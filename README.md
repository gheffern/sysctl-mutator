# Kubernetes Sysctl Admission Mutator

A high-performance Kubernetes Mutating Admission Webhook written in Rust, utilizing `mimalloc` and Link-Time Optimization (LTO). It dynamically configures sysctl values for Pods based on a hierarchical merge of cluster-wide defaults, namespace annotations, and pod specifications.

## Features
- **Hierarchical Merge**: Combines sysctls from three levels (Pod spec > Namespace annotation > Cluster fallback default) where more specific settings override less specific ones.
- **In-Memory Watch Cache**: Watches Namespace resources using Kubernetes reflectors to achieve sub-millisecond mutations during pod admission.
- **Minimal Docker Footprint**: Compiled with LTO and `mimalloc`, packaged inside a minimal Google Distroless CC runtime container.
- **Safety Exclusions**: Safe configurations excluding critical namespaces (e.g., `kube-system`, `kube-public`, and the webhook's own namespace) to prevent cluster bootstrapping locks.

## Quick Start

### 1. Build and Load Image
Build the container image using Podman:
```bash
podman build -t sysctl-mutator:latest .
```
Load it into a local `kind` cluster:
```bash
kind load docker-image sysctl-mutator:latest
```

### 2. Generate TLS Certificates
Mutating webhooks require HTTPS. Run the bootstrap script to generate self-signed TLS certificates, create the TLS secret, and configure the mutating webhook manifest:
```bash
./scripts/generate-certs.sh
```

### 3. Deploy
Deploy the RBAC resources, webhook service, and MutatingWebhookConfiguration:
```bash
kubectl apply -f k8s/rbac.yaml
kubectl apply -f k8s/deployment.yaml
kubectl apply -f k8s/webhook-config.yaml
```

## Detailed Documentation
- [Configuration and Merging Strategy](docs/configuration.md)
- [Example Use Cases and Configurations](docs/use-cases.md)
