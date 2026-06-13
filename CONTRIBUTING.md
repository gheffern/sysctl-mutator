# Developer & Contributing Guide

Thank you for contributing to `sysctl-mutator`! This document outlines how to build, test, and run the admission webhook locally during development.

## Prerequisites

To develop on this codebase, you need:
- **Rust Toolchain**: Stable compiler, `cargo`, `rustfmt`, and `clippy`.
- **Container Engine**: `podman` or `docker` (for building container images).
- **Kubernetes Cluster**: A local development cluster like `kind` or `k3s`.
- **Helm**: For testing Helm chart template modifications.

---

## Local Development & Testing

### Running Unit Tests
Unit tests verify the parsing, config logic, and hierarchical merge of sysctl values:
```bash
cargo test --all-targets
```

### Checking Formatting and Lints
CI will fail if your code is not formatted or has clippy warnings. Run the following checks locally:
```bash
# Verify formatting
cargo fmt --all -- --check

# Check for compiler warnings and common lints
cargo clippy --all-targets -- -D warnings
```

---

## Local Container Builds

### 1. Build Image
Build the container image using Podman or Docker. The build compiles the binary in release mode using Link-Time Optimization (LTO) and package it in a minimal Google Distroless CC runtime container:
```bash
podman build -t sysctl-mutator:latest .
```

### 2. Load into Local Cluster
If you are using `kind`, load the freshly built image directly into your development cluster:
```bash
kind load docker-image sysctl-mutator:latest
```

---

## TLS Certificate Bootstrapping

Kubernetes mutating webhooks require HTTPS communication. A local shell helper script is provided under `scripts/` to generate self-signed certificates and bootstrap secrets/webhook configurations for testing.

To generate certs, create the secret in the cluster, and populate the `caBundle` in your raw manifests:
```bash
./scripts/generate-certs.sh
```

---

## Linting the Helm Chart

If you modify the Helm chart templates in `k8s/charts/sysctl-mutator`, ensure the templates lint successfully:
```bash
helm lint k8s/charts/sysctl-mutator
```

You can render and verify the rendered templates locally:
```bash
helm template sysctl-mutator k8s/charts/sysctl-mutator --debug
```
