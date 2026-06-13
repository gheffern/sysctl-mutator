use axum::{
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Namespace;
use kube::runtime::{
    reflector,
    watcher::{watcher, Config as WatcherConfig},
};
use kube::{Api, Client};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod config;
mod webhook;

pub struct AppState {
    pub ns_store: reflector::Store<Namespace>,
    pub default_sysctls: std::collections::HashMap<String, String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install default crypto provider for rustls
    let _ = rustls::crypto::ring::default_provider().install_default();

    // 1. Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting sysctl-mutator admission webhook...");

    // 2. Parse configuration
    let cfg = config::Config::parse();
    let default_sysctls = cfg
        .parse_default_sysctls()
        .expect("Failed to parse DEFAULT_SYSCTLS env/arg as JSON");

    // 3. Initialize Kubernetes Client
    let client = Client::try_default().await?;
    let namespaces: Api<Namespace> = Api::all(client);

    // 4. Set up Namespace Reflector (In-memory cache)
    let (reader, writer) = reflector::store();
    let stream = watcher(namespaces, WatcherConfig::default());
    let rf = reflector(writer, stream);

    // Spawn Reflector task to watch namespaces
    tokio::spawn(async move {
        let mut stream = std::pin::pin!(rf);
        while let Some(event) = stream.next().await {
            if let Err(err) = event {
                tracing::error!("Informer watcher error: {:?}", err);
            }
        }
    });

    // 5. Build Axum Router
    let state = Arc::new(AppState {
        ns_store: reader,
        default_sysctls,
    });

    let app = build_app(state);

    // 6. Bind Axum Server with TLS
    let tls_config = RustlsConfig::from_pem_file(&cfg.tls_cert, &cfg.tls_key)
        .await
        .expect("Failed to load TLS certificates");

    let addr = format!("{}:{}", cfg.bind_address, cfg.port)
        .parse()
        .expect("Invalid bind address/port");

    tracing::info!("Webhook server listening on HTTPS at {}", addr);

    axum_server::bind_rustls(addr, tls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

pub fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/mutate", post(webhook::mutate_handler))
        .route("/healthz", get(|| async { "OK" }))
        .route("/readyz", get(|| async { "OK" }))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_healthz() {
        let (reader, _) = reflector::store();
        let state = Arc::new(AppState {
            ns_store: reader,
            default_sysctls: std::collections::HashMap::new(),
        });
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_readyz() {
        let (reader, _) = reflector::store();
        let state = Arc::new(AppState {
            ns_store: reader,
            default_sysctls: std::collections::HashMap::new(),
        });
        let app = build_app(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/readyz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn test_mutate_handler_success() {
        use k8s_openapi::api::core::v1::Pod;
        use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
        use std::collections::BTreeMap;

        // 1. Setup mock reflector store and write Namespace with annotation
        let (reader, mut writer) = reflector::store::<Namespace>();
        let ns = Namespace {
            metadata: ObjectMeta {
                name: Some("test-ns".to_string()),
                annotations: Some(BTreeMap::from([(
                    "sysctl-mutator.gromware.com/sysctls".to_string(),
                    r#"{"net.core.somaxconn": "2048"}"#.to_string(),
                )])),
                ..Default::default()
            },
            ..Default::default()
        };
        writer.apply_watcher_event(&kube::runtime::watcher::Event::Apply(ns));

        // 2. Setup AppState
        let state = Arc::new(AppState {
            ns_store: reader,
            default_sysctls: std::collections::HashMap::new(),
        });
        let app = build_app(state);

        // 3. Construct AdmissionReview request with a Pod
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some("test-pod".to_string()),
                namespace: Some("test-ns".to_string()),
                ..Default::default()
            },
            spec: Some(k8s_openapi::api::core::v1::PodSpec {
                ..Default::default()
            }),
            ..Default::default()
        };

        let review_req: kube::core::admission::AdmissionReview<Pod> =
            serde_json::from_value(serde_json::json!({
                "apiVersion": "admission.k8s.io/v1",
                "kind": "AdmissionReview",
                "request": {
                    "uid": "test-uid-1234",
                    "kind": {
                        "group": "",
                        "version": "v1",
                        "kind": "Pod"
                    },
                    "resource": {
                        "group": "",
                        "version": "v1",
                        "resource": "pods"
                    },
                    "requestKind": {
                        "group": "",
                        "version": "v1",
                        "kind": "Pod"
                    },
                    "requestResource": {
                        "group": "",
                        "version": "v1",
                        "resource": "pods"
                    },
                    "name": "test-pod",
                    "namespace": "test-ns",
                    "operation": "CREATE",
                    "userInfo": {
                        "username": "admin",
                        "groups": ["system:masters"]
                    },
                    "object": pod,
                    "dryRun": false
                }
            }))
            .unwrap();

        // 4. Send request to /mutate
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/mutate")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&review_req).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 5. Parse and assert on response AdmissionReview
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let review_res: kube::core::admission::AdmissionReview<Pod> =
            serde_json::from_slice(&body_bytes).unwrap();
        let res = review_res.response.unwrap();

        assert!(res.allowed);
        assert_eq!(res.uid, "test-uid-1234");

        // Patch should be present representing the mutation
        let patch_bytes = res.patch.unwrap();
        let patch_val: serde_json::Value = serde_json::from_slice(&patch_bytes).unwrap();
        assert_eq!(
            patch_val,
            serde_json::json!([
                {
                    "op": "add",
                    "path": "/spec/securityContext",
                    "value": {
                        "sysctls": [
                            {"name": "net.core.somaxconn", "value": "2048"}
                        ]
                    }
                }
            ])
        );
    }
}
