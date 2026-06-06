# Example Use Cases & Configurations

This guide provides configurations for typical workload scenarios and notes on Kubernetes sysctl safety.

> [!WARNING]
> **Safe vs. Unsafe Sysctls**:
> Kubernetes categorizes sysctls into **safe** and **unsafe**:
> - **Safe sysctls** (e.g., `net.ipv4.ping_group_range`) are fully namespaced and do not impact other pods or the host node.
> - **Unsafe sysctls** (e.g., `net.core.somaxconn`, `net.ipv4.ip_local_port_range`, `kernel.shmmax`) are disabled on the kubelet by default.
>
> If the mutator applies an **unsafe** sysctl to a Pod, the kubelet will reject that Pod at scheduling time unless the host kubelet has been explicitly started with the flag:
> `--allowed-unsafe-sysctls=<comma-separated-list>`
> Ensure you configure your cluster nodes' kubelet flags to allow the sysctls you plan to mutate.

---

## Workload Examples

### 1. High-Throughput HTTP / Web Services
To handle large volumes of concurrent TCP connections (e.g., for Nginx, HAProxy, or Envoy proxies), apply the following configuration to open port ranges and allow larger queue sizes.

**Namespace Annotation**:
```bash
kubectl annotate namespace web-services sysctl-mutator.gromware.com/sysctls='{
  "net.core.somaxconn": "8192",
  "net.ipv4.ip_local_port_range": "1024 65535",
  "net.ipv4.tcp_fin_timeout": "15"
}'
```

### 2. Database Workloads (PostgreSQL / Redis)
High-performance databases often require adjustments to socket queues and keepalive configurations to manage connections reliably over long periods.

**Namespace Annotation**:
```bash
kubectl annotate namespace database sysctl-mutator.gromware.com/sysctls='{
  "net.core.somaxconn": "4096",
  "net.ipv4.tcp_keepalive_time": "600",
  "net.ipv4.tcp_keepalive_intvl": "10",
  "net.ipv4.tcp_keepalive_probes": "9"
}'
```
