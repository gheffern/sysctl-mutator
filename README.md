# Kubernetes Sysctl Admission Mutator

A high-performance Kubernetes Mutating Admission Webhook written in Rust, utilizing `mimalloc` and Link-Time Optimization (LTO). It dynamically configures sysctl values for Pods based on a hierarchical merge of cluster-wide defaults, namespace annotations, and pod specifications.

## Features
- **Hierarchical Merge**: Combines sysctls from three levels (Pod spec > Namespace annotation > Cluster fallback default) where more specific settings override less specific ones.
- **In-Memory Watch Cache**: Watches Namespace resources using Kubernetes reflectors to achieve sub-millisecond mutations during pod admission.
- **Low-Privilege Mode**: Optional zero-cluster-RBAC mode (`DISABLE_NAMESPACE_REFLECTOR=true`) to run without any namespace watching/reading permissions, ideal for restricted multi-tenant clusters.
- **Minimal Docker Footprint**: Compiled with LTO and `mimalloc`, packaged inside a minimal Google Distroless CC runtime container.
- **Safety Exclusions**: Safe configurations excluding critical namespaces (e.g., `kube-system`, `kube-public`, and the webhook's own namespace) to prevent cluster bootstrapping locks.

---

## Deployment Modes

Before deploying, choose the mode that matches your security and feature requirements:

| Mode | Namespace Annotations | Required RBAC Permissions | Recommended For |
| :--- | :--- | :--- | :--- |
| **Standard Mode** (Default) | **Enabled** (Merges default + Namespace overrides + Pod spec) | Cluster-wide `get`, `list`, `watch` on `namespaces` | Standard clusters where namespace-level overrides are desired. |
| **Low-Privilege Mode** | **Disabled** (Merges default + Pod spec only) | **None** (Zero cluster-scoped permissions) | Secure, multi-tenant clusters where cluster-wide roles are restricted. |

---

## Deployment

### Option 1: Helm Chart (Preferred)

Helm is the recommended deployment method because it automatically handles self-signed TLS certificate generation, namespace exclusions, and modular configurations.

1. **Deploy with Standard Mode (Default):**
   ```bash
   helm install sysctl-mutator k8s/charts/sysctl-mutator \
     --namespace sysctl-mutator \
     --create-namespace
   ```

2. **Deploy with Low-Privilege Mode:**
   Disable the namespace watcher to remove all cluster-scoped RBAC requirements:
   ```bash
   helm install sysctl-mutator k8s/charts/sysctl-mutator \
     --namespace sysctl-mutator \
     --create-namespace \
     --set disableNamespaceReflector=true
   ```

3. **Configure Custom Defaults:**
   Pass your desired default sysctls as structured values:
   ```bash
   helm install sysctl-mutator k8s/charts/sysctl-mutator \
     --namespace sysctl-mutator \
     --create-namespace \
     --set defaultSysctls."net.ipv4.ip_local_port_range"="1024 65000"
   ```

For advanced settings (e.g., using cert-manager instead of self-signed certs), see [values.yaml](k8s/charts/sysctl-mutator/values.yaml).

---

### Option 2: Static Manifests

If you prefer deploying raw manifests, they are located under the `k8s/` directory.

> [!NOTE]
> Mutating webhooks require HTTPS. When using static manifests, you must generate your own TLS certificates, create a TLS secret named `sysctl-mutator-certs` in the `sysctl-mutator` namespace, and populate the `caBundle` in the webhook configuration.

1. **Deploy standard resources:**
   ```bash
   kubectl apply -f k8s/rbac.yaml
   kubectl apply -f k8s/deployment.yaml
   ```

2. **Configure your TLS secret and apply webhook configuration:**
   ```bash
   kubectl apply -f k8s/webhook-config.yaml
   ```

To run static manifests in Low-Privilege mode, uncomment the `DISABLE_NAMESPACE_REFLECTOR` environment variable in `k8s/deployment.yaml` and omit `rbac.yaml`'s `ClusterRole` and `ClusterRoleBinding`.

---

## Detailed Documentation
- [Configuration and Merging Strategy](docs/configuration.md)
- [Example Use Cases and Configurations](docs/use-cases.md)
- [Developer & Contributing Guide](CONTRIBUTING.md)
