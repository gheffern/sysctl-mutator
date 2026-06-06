use axum::{
    routing::{get, post},
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use futures::StreamExt;
use kube::{Api, Client};
use kube::runtime::{reflector, watcher::{watcher, Config as WatcherConfig}};
use k8s_openapi::api::core::v1::Namespace;
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
    // 1. Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    tracing::info!("Starting sysctl-mutator admission webhook...");

    // 2. Parse configuration
    let cfg = config::Config::parse();
    let default_sysctls = cfg.parse_default_sysctls()
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
}
