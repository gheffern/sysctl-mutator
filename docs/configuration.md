# Configuration & Merging Strategy

The `sysctl-mutator` admission webhook resolves sysctl settings dynamically by merging values defined at three different scopes.

## Precedence and Merge Order

1. **Pod Specification (Highest Priority)**: Any sysctl explicitly defined in `spec.securityContext.sysctls` inside the Pod manifest.
2. **Namespace Annotation (Medium Priority)**: Any sysctl defined in the target Namespace's `sysctl-mutator.elotl.co/sysctls` annotation.
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
To enable namespaced configuration, annotate a namespace with a JSON map under the `sysctl-mutator.elotl.co/sysctls` key:

```bash
kubectl annotate namespace production sysctl-mutator.elotl.co/sysctls='{"net.core.somaxconn": "4096", "net.ipv4.tcp_rmem": "4096 87380 16777216"}'
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
