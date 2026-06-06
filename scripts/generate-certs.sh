#!/usr/bin/env bash
set -euo pipefail

NAMESPACE="sysctl-mutator"
SERVICE="sysctl-mutator"
SECRET="sysctl-mutator-certs"
TMP_DIR=$(mktemp -d)

echo "Generating TLS certificates in ${TMP_DIR}..."

# 1. Create a CA
openssl genrsa -out "${TMP_DIR}/ca.key" 2048
openssl req -x509 -new -nodes -key "${TMP_DIR}/ca.key" -subj "/CN=${SERVICE}-ca" -days 365 -out "${TMP_DIR}/ca.crt"

# 2. Create server key and certificate signing request
openssl genrsa -out "${TMP_DIR}/tls.key" 2048
openssl req -new -key "${TMP_DIR}/tls.key" -subj "/CN=${SERVICE}.${NAMESPACE}.svc" -out "${TMP_DIR}/tls.csr" -config <(
cat <<EOF
[req]
req_extensions = v3_req
distinguished_name = req_distinguished_name
[req_distinguished_name]
[ v3_req ]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names
[alt_names]
DNS.1 = ${SERVICE}
DNS.2 = ${SERVICE}.${NAMESPACE}
DNS.3 = ${SERVICE}.${NAMESPACE}.svc
EOF
)

# 3. Sign the server certificate with the CA
openssl x509 -req -in "${TMP_DIR}/tls.csr" -CA "${TMP_DIR}/ca.crt" -CAkey "${TMP_DIR}/ca.key" -CAcreateserial -out "${TMP_DIR}/tls.crt" -days 365 -extensions v3_req -extfile <(
cat <<EOF
[v3_req]
basicConstraints = CA:FALSE
keyUsage = nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = @alt_names
[alt_names]
DNS.1 = ${SERVICE}
DNS.2 = ${SERVICE}.${NAMESPACE}
DNS.3 = ${SERVICE}.${NAMESPACE}.svc
EOF
)

echo "Creating namespace ${NAMESPACE} if not exists..."
kubectl create namespace "${NAMESPACE}" || true

echo "Creating TLS secret '${SECRET}' in namespace '${NAMESPACE}'..."
kubectl delete secret "${SECRET}" -n "${NAMESPACE}" || true
kubectl create secret tls "${SECRET}" \
  -n "${NAMESPACE}" \
  --cert="${TMP_DIR}/tls.crt" \
  --key="${TMP_DIR}/tls.key"

# 4. Inject CA Bundle into webhook config
CA_BUNDLE=$(cat "${TMP_DIR}/ca.crt" | base64 | tr -d '\n')
echo "Injecting CA bundle into k8s/webhook-config.yaml..."
sed -i "s/caBundle: \".*\"/caBundle: \"${CA_BUNDLE}\"/g" k8s/webhook-config.yaml

echo "Cleanup temporary cert directory..."
rm -rf "${TMP_DIR}"

echo "TLS certificates successfully generated, secret created, and webhook config patched!"
