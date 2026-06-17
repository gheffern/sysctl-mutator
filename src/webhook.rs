use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use k8s_openapi::api::core::v1::{Namespace, Pod, Sysctl};
use kube::core::admission::{AdmissionResponse, AdmissionReview};
use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

/// Core business logic: hierarchically merges default, namespace-level, and pod-level sysctls.
fn calculate_merged_sysctls(
    pod: &Pod,
    ns_opt: Option<&Namespace>,
    default_sysctls: &HashMap<String, String>,
) -> Vec<Sysctl> {
    let mut merged = default_sysctls.clone();

    // 1. Namespace overrides defaults
    if let Some(ns) = ns_opt {
        if let Some(annotations) = &ns.metadata.annotations {
            if let Some(ann_val) = annotations.get("sysctl-mutator.gromware.com/sysctls") {
                match serde_json::from_str::<HashMap<String, String>>(ann_val) {
                    Ok(ns_sysctls) => {
                        for (k, v) in ns_sysctls {
                            merged.insert(k, v);
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to parse namespace annotation 'sysctl-mutator.gromware.com/sysctls' in namespace {}: {}",
                            ns.metadata.name.as_deref().unwrap_or("unknown"),
                            e
                        );
                    }
                }
            }
        }
    }

    // 2. Pod overrides both
    let mut pod_sysctls = HashMap::new();
    if let Some(spec) = &pod.spec {
        if let Some(sec_ctx) = &spec.security_context {
            if let Some(sysctls) = &sec_ctx.sysctls {
                for s in sysctls {
                    pod_sysctls.insert(s.name.clone(), s.value.clone());
                }
            }
        }
    }

    for (k, v) in &pod_sysctls {
        merged.insert(k.clone(), v.clone());
    }

    // Convert to sorted vector
    let mut target_sysctls: Vec<Sysctl> = merged
        .into_iter()
        .map(|(name, value)| Sysctl { name, value })
        .collect();
    target_sysctls.sort_by(|a, b| a.name.cmp(&b.name));
    target_sysctls
}

#[allow(clippy::too_many_lines)]
pub async fn mutate_handler(
    State(state): State<Arc<AppState>>,
    Json(review): Json<AdmissionReview<Pod>>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let mut operation = "UNKNOWN".to_string();
    let mut namespace = "unknown".to_string();
    let mut allowed = false;

    let (status, res) = mutate_handler_inner(
        &state,
        &review,
        &mut operation,
        &mut namespace,
        &mut allowed,
    );

    if let Some(metrics) = &state.metrics {
        metrics
            .requests_total
            .with_label_values(&[&operation, &allowed.to_string(), &namespace])
            .inc();
        metrics
            .request_duration_seconds
            .with_label_values(&[&operation, &allowed.to_string()])
            .observe(start.elapsed().as_secs_f64());
    }

    (status, res)
}

#[allow(clippy::too_many_lines)]
fn mutate_handler_inner(
    state: &AppState,
    review: &AdmissionReview<Pod>,
    operation: &mut String,
    namespace: &mut String,
    allowed: &mut bool,
) -> (StatusCode, Json<AdmissionReview<Pod>>) {
    let Some(req) = &review.request else {
        tracing::error!("Received AdmissionReview without request");
        return (
            StatusCode::BAD_REQUEST,
            Json(AdmissionReview::<Pod> {
                types: review.types.clone(),
                request: None,
                response: Some(AdmissionResponse::invalid("Missing request field")),
            }),
        );
    };

    *operation = match req.operation {
        kube::core::admission::Operation::Create => "CREATE".to_string(),
        kube::core::admission::Operation::Update => "UPDATE".to_string(),
        kube::core::admission::Operation::Delete => "DELETE".to_string(),
        kube::core::admission::Operation::Connect => "CONNECT".to_string(),
    };
    *namespace = req
        .namespace
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let Some(pod) = &req.object else {
        tracing::error!("Received AdmissionRequest without Pod object");
        let mut response = AdmissionResponse::from(req);
        response.allowed = false;
        response.result =
            kube::core::Status::failure("Missing Pod object in request", "InvalidRequest")
                .with_code(400);
        return (
            StatusCode::OK,
            Json(AdmissionReview {
                types: review.types.clone(),
                request: None,
                response: Some(response),
            }),
        );
    };

    // Skip mutation for pods sharing host namespaces (hostNetwork or hostIPC) to prevent API server validation failures
    let spec = pod.spec.as_ref();
    let is_host_network = spec.and_then(|s| s.host_network).unwrap_or(false);
    let is_host_ipc = spec.and_then(|s| s.host_ipc).unwrap_or(false);

    if is_host_network || is_host_ipc {
        let mut response = AdmissionResponse::from(req);
        *allowed = true;
        response.allowed = true;
        return (
            StatusCode::OK,
            Json(AdmissionReview {
                types: review.types.clone(),
                request: None,
                response: Some(response),
            }),
        );
    }

    // 1. Determine namespace annotations
    let ns_name = &req.namespace;
    let ns_opt = ns_name.as_ref().and_then(|name| {
        let ns_ref = kube::runtime::reflector::ObjectRef::new(name);
        state.ns_store.get(&ns_ref).map(|ns| ns.as_ref().clone())
    });

    // 2. Calculate target sysctls
    let target_sysctls = calculate_merged_sysctls(pod, ns_opt.as_ref(), &state.default_sysctls);

    // Get existing sysctls
    let mut existing_sysctls = pod
        .spec
        .as_ref()
        .and_then(|spec| spec.security_context.as_ref())
        .and_then(|sec_ctx| sec_ctx.sysctls.as_ref())
        .cloned()
        .unwrap_or_default();
    existing_sysctls.sort_by(|a, b| a.name.cmp(&b.name));

    // If no change, allowed = true, no patch
    if target_sysctls == existing_sysctls {
        let mut response = AdmissionResponse::from(req);
        *allowed = true;
        response.allowed = true;
        return (
            StatusCode::OK,
            Json(AdmissionReview {
                types: review.types.clone(),
                request: None,
                response: Some(response),
            }),
        );
    }

    // Build JSON Patch
    let has_security_context = pod
        .spec
        .as_ref()
        .and_then(|s| s.security_context.as_ref())
        .is_some();
    let has_sysctls = pod
        .spec
        .as_ref()
        .and_then(|s| s.security_context.as_ref())
        .and_then(|sc| sc.sysctls.as_ref())
        .is_some();

    let patch_val = if !has_security_context {
        serde_json::json!([
            {
                "op": "add",
                "path": "/spec/securityContext",
                "value": {
                    "sysctls": target_sysctls
                }
            }
        ])
    } else if !has_sysctls {
        serde_json::json!([
            {
                "op": "add",
                "path": "/spec/securityContext/sysctls",
                "value": target_sysctls
            }
        ])
    } else {
        serde_json::json!([
            {
                "op": "replace",
                "path": "/spec/securityContext/sysctls",
                "value": target_sysctls
            }
        ])
    };

    let patch: json_patch::Patch = match serde_json::from_value(patch_val) {
        Ok(p) => p,
        Err(err) => {
            tracing::error!("Failed to parse patch JSON: {:?}", err);
            let mut response = AdmissionResponse::from(req);
            *allowed = false;
            response.allowed = false;
            let err_msg = format!("Failed to parse mutation patch: {err}");
            response.result = kube::core::Status::failure(&err_msg, "InternalError").with_code(500);
            return (
                StatusCode::OK,
                Json(AdmissionReview {
                    types: review.types.clone(),
                    request: None,
                    response: Some(response),
                }),
            );
        }
    };

    let mut response = AdmissionResponse::from(req);
    *allowed = true;
    response.allowed = true;

    response = match response.with_patch(patch) {
        Ok(res) => res,
        Err(err) => {
            tracing::error!("Failed to apply patch to admission response: {:?}", err);
            let mut response = AdmissionResponse::from(req);
            *allowed = false;
            response.allowed = false;
            let err_msg = format!("Failed to apply mutation patch: {err}");
            response.result = kube::core::Status::failure(&err_msg, "InternalError").with_code(500);
            return (
                StatusCode::OK,
                Json(AdmissionReview {
                    types: review.types.clone(),
                    request: None,
                    response: Some(response),
                }),
            );
        }
    };

    (
        StatusCode::OK,
        Json(AdmissionReview {
            types: review.types.clone(),
            request: None,
            response: Some(response),
        }),
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::PodSecurityContext;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn create_test_pod(sysctls: Option<Vec<Sysctl>>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some("test-pod".to_string()),
                ..Default::default()
            },
            spec: Some(k8s_openapi::api::core::v1::PodSpec {
                security_context: sysctls.map(|s| PodSecurityContext {
                    sysctls: Some(s),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn create_test_ns(annotation_val: Option<&str>) -> Namespace {
        let mut annotations = std::collections::BTreeMap::new();
        if let Some(val) = annotation_val {
            annotations.insert(
                "sysctl-mutator.gromware.com/sysctls".to_string(),
                val.to_string(),
            );
        }
        Namespace {
            metadata: ObjectMeta {
                name: Some("test-namespace".to_string()),
                annotations: Some(annotations),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_hierarchical_merge() {
        // 1. Defaults setup
        let mut defaults = HashMap::new();
        defaults.insert(
            "net.ipv4.ip_local_port_range".to_string(),
            "1024 65000".to_string(),
        );
        defaults.insert("net.core.somaxconn".to_string(), "1024".to_string());

        // Scenario A: Pod with no annotations, NS with no annotations -> inherits only defaults
        let pod_a = create_test_pod(None);
        let ns_a = create_test_ns(None);
        let merged_a = calculate_merged_sysctls(&pod_a, Some(&ns_a), &defaults);
        assert_eq!(merged_a.len(), 2);
        assert_eq!(merged_a[0].name, "net.core.somaxconn");
        assert_eq!(merged_a[0].value, "1024");
        assert_eq!(merged_a[1].name, "net.ipv4.ip_local_port_range");
        assert_eq!(merged_a[1].value, "1024 65000");

        // Scenario B: NS overrides a default and adds a new one
        let ns_b = create_test_ns(Some(
            r#"{"net.core.somaxconn": "2048", "net.ipv4.tcp_rmem": "4096 87380 16777216"}"#,
        ));
        let merged_b = calculate_merged_sysctls(&pod_a, Some(&ns_b), &defaults);
        assert_eq!(merged_b.len(), 3);
        // net.core.somaxconn should be overridden by Namespace to 2048
        let somaxconn = merged_b
            .iter()
            .find(|s| s.name == "net.core.somaxconn")
            .unwrap();
        assert_eq!(somaxconn.value, "2048");
        // net.ipv4.tcp_rmem should be added
        let tcp_rmem = merged_b
            .iter()
            .find(|s| s.name == "net.ipv4.tcp_rmem")
            .unwrap();
        assert_eq!(tcp_rmem.value, "4096 87380 16777216");
        // net.ipv4.ip_local_port_range should still be default
        let port_range = merged_b
            .iter()
            .find(|s| s.name == "net.ipv4.ip_local_port_range")
            .unwrap();
        assert_eq!(port_range.value, "1024 65000");

        // Scenario C: Pod overrides default and Namespace
        let pod_c = create_test_pod(Some(vec![Sysctl {
            name: "net.core.somaxconn".to_string(),
            value: "4096".to_string(),
        }]));
        let merged_c = calculate_merged_sysctls(&pod_c, Some(&ns_b), &defaults);
        let somaxconn_c = merged_c
            .iter()
            .find(|s| s.name == "net.core.somaxconn")
            .unwrap();
        // Pod value 4096 should override NS value 2048 and default value 1024
        assert_eq!(somaxconn_c.value, "4096");
    }
}
