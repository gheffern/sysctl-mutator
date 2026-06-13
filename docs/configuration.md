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
