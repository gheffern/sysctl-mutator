# Configuration & Merging Strategy

The `sysctl-mutator` admission webhook resolves sysctl settings dynamically by merging values defined at three different scopes.

## Precedence and Merge Order

1. **Pod Specification (Highest Priority)**: Any sysctl explicitly defined in `spec.securityContext.sysctls` inside the Pod manifest.
2. **Namespace Annotation (Medium Priority)**: Any sysctl defined in the target Namespace's `sysctl-mutator.gromware.com/sysctls` annotation.
3. **Cluster-wide Default (Lowest Priority)**: The default fallback sysctl settings passed to the mutator binary via arguments or environment variables.

Sysctls are merged key-by-key. For duplicate keys, the value defined at the higher priority level overrides the lower level.

---

## Configuration Scopes

### 1. Cluster-wide Defaults
Configure cluster-wide fallback sysctls by editing the `DEFAULT_SYSCTLS` environment variable (or passing the `--default-sysctls` argument) in the mutator deployment:

```yaml
env:
  - name: DEFAULT_SYSCTLS
    value: '{"net.ipv4.ip_local_port_range": "1024 65000", "net.core.somaxconn": "1024"}'
```

### 2. Namespace Annotations
To enable namespaced configuration, annotate a namespace with a JSON map under the `sysctl-mutator.gromware.com/sysctls` key:

```bash
kubectl annotate namespace production sysctl-mutator.gromware.com/sysctls='{"net.core.somaxconn": "4096", "net.ipv4.tcp_rmem": "4096 87380 16777216"}'
```

### 3. Pod Specifications
Any standard sysctl specified in the Pod spec takes top priority:

```yaml
spec:
  securityContext:
    sysctls:
      - name: net.core.somaxconn
        value: "8192"
```

### 4. Low-Privilege Mode (Default)
By default, `sysctl-mutator` runs in **Low-Privilege Mode**, which requires no cluster-wide permissions (completely bypassing namespace read/watch scopes). This is ideal for secure, multi-tenant environments where `ClusterRole` and `ClusterRoleBinding` resources are restricted.

Enable this by setting the `DISABLE_NAMESPACE_REFLECTOR` environment variable to `"true"` (or using the `--disable-namespace-reflector` command-line argument):

```yaml
env:
  - name: DISABLE_NAMESPACE_REFLECTOR
    value: "true"
```

**Impact:**
* **No Namespace RBAC Required**: The webhook no longer queries the API server. By default, the `ClusterRole` and `ClusterRoleBinding` defined in `k8s/rbac.yaml` are commented out and not created.
* **Simplified Precedence**: The hierarchical merge simplifies to:
  1. **Pod Specification (Highest Priority)**
  2. **Cluster-wide Default (Lowest Priority)**
  *(Namespace-level annotations are ignored by default since the webhook has no access to Namespace resources).*

To run in **Namespace-Reflector Mode** (enabling namespace annotations):
1. Set `DISABLE_NAMESPACE_REFLECTOR` to `"false"` in `k8s/deployment.yaml` (or set `disableNamespaceReflector=false` in Helm).
2. Uncomment and apply the `ClusterRole` and `ClusterRoleBinding` resources in `k8s/rbac.yaml` (or Helm will automatically create them).

---

## HTTP/2 Connection Tuning

To prevent connection pinning (ensuring balanced traffic across webhook replicas) and avoid silent connection drops by middleboxes, `sysctl-mutator` supports HTTP/2 configuration options.

| Environment Variable | CLI Argument | Default | Description |
| :--- | :--- | :--- | :--- |
| `HTTP2_KEEP_ALIVE_INTERVAL_SECS` | `--http2-keep-alive-interval-secs` | `0` (Disabled) | Interval in seconds to send HTTP/2 PING frames to keep connections alive. |
| `HTTP2_KEEP_ALIVE_TIMEOUT_SECS` | `--http2-keep-alive-timeout-secs` | `20` | Timeout in seconds to wait for a ping response before terminating the connection. |
| `HTTP2_MAX_CONCURRENT_STREAMS` | `--http2-max-concurrent-streams` | `0` (Uses default: `200`) | Maximum simultaneous streams allowed per connection. |

### Helm Overrides

These settings can be tuned in Helm's `values.yaml` under the `http2` block:

```yaml
http2:
  keepAliveIntervalSecs: 60
  keepAliveTimeoutSecs: 20
  maxConcurrentStreams: 200
```

---

## Prometheus Metrics

`sysctl-mutator` exposes standard Prometheus metrics on a separate HTTP port (unauthenticated, defaults to port `9090`).

| Environment Variable | CLI Argument | Default | Description |
| :--- | :--- | :--- | :--- |
| `DISABLE_METRICS` | `--disable-metrics` | `false` | Set to `true` to disable the metrics endpoint. |
| `METRICS_PORT` | `--metrics-port` | `9090` | The HTTP port to expose `/metrics` on. |
| `METRICS_BIND_ADDRESS` | `--metrics-bind-address` | `0.0.0.0` | The IP address the metrics server binds to. |

### Exposed Metrics

* **`webhook_requests_total`** (Counter): Total number of mutation requests processed.
  * *Labels:* `operation` (`CREATE`/`UPDATE`/`UNKNOWN`), `allowed` (`true`/`false`), `namespace` (target pod namespace).
* **`webhook_request_duration_seconds`** (Histogram): Duration of mutation requests processed.
  * *Labels:* `operation`, `allowed`.
* **`reflector_namespace_count`** (Gauge): Number of namespaces currently held in the in-memory reflector store (only relevant if namespace reflector is enabled).

### Helm Overrides

Configure metrics inside Helm's `values.yaml`:

```yaml
metrics:
  enabled: true
  port: 9090
  bindAddress: 0.0.0.0
```

---

## Mutation Mechanics

When a Pod is created:
1. The webhook checks if the final merged sysctl set differs from what the Pod already has. If they match, no mutation is made.
2. If they differ, a JSONPatch is generated:
   - If `spec.securityContext` is completely absent, it initializes it:
     `[{"op": "add", "path": "/spec/securityContext", "value": {"sysctls": [...]}}]`
   - If `spec.securityContext` exists but has no `sysctls` field, it adds the field:
     `[{"op": "add", "path": "/spec/securityContext/sysctls", "value": [...]}]`
   - If `spec.securityContext.sysctls` already exists but contains outdated/missing entries, it replaces it:
     `[{"op": "replace", "path": "/spec/securityContext/sysctls", "value": [...]}]`
