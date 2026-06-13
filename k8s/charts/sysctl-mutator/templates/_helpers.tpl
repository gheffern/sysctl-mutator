{{/*
Expand the name of the chart.
*/}}
{{- define "sysctl-mutator.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "sysctl-mutator.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "sysctl-mutator.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "sysctl-mutator.labels" -}}
helm.sh/chart: {{ include "sysctl-mutator.chart" . }}
{{ include "sysctl-mutator.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "sysctl-mutator.selectorLabels" -}}
app.kubernetes.io/name: {{ include "sysctl-mutator.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "sysctl-mutator.serviceAccountName" -}}
{{- include "sysctl-mutator.fullname" . }}
{{- end }}

{{/*
Determine the TLS secret name.
*/}}
{{- define "sysctl-mutator.tlsSecretName" -}}
{{- if eq .Values.tls.mode "manual" }}
{{- required "tls.secretName is required when tls.mode is manual" .Values.tls.secretName }}
{{- else if .Values.tls.secretName }}
{{- .Values.tls.secretName }}
{{- else }}
{{- printf "%s-certs" (include "sysctl-mutator.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Generate self-signed certificate once and cache it in the root context
*/}}
{{- define "sysctl-mutator.certs" -}}
  {{- if not (hasKey $ "sysctlMutatorCerts") -}}
    {{- $secretName := include "sysctl-mutator.tlsSecretName" . -}}
    {{- $secret := lookup "v1" "Secret" .Release.Namespace $secretName -}}
    {{- $ca := "" -}}
    {{- $cert := "" -}}
    {{- $key := "" -}}
    {{- if and $secret (not .Values.tls.forceRegenerate) -}}
      {{- $ca = index $secret.data "ca.crt" -}}
      {{- $cert = index $secret.data "tls.crt" -}}
      {{- $key = index $secret.data "tls.key" -}}
    {{- else -}}
      {{- $altNames := list (include "sysctl-mutator.fullname" .) (printf "%s.%s" (include "sysctl-mutator.fullname" .) .Release.Namespace) (printf "%s.%s.svc" (include "sysctl-mutator.fullname" .) .Release.Namespace) (printf "%s.%s.svc.cluster.local" (include "sysctl-mutator.fullname" .) .Release.Namespace) -}}
      {{- $caGen := genCA "sysctl-mutator-ca" 3650 -}}
      {{- $certGen := genSignedCert (include "sysctl-mutator.fullname" .) nil $altNames 3650 $caGen -}}
      {{- $ca = $caGen.Cert | b64enc -}}
      {{- $cert = $certGen.Cert | b64enc -}}
      {{- $key = $certGen.Key | b64enc -}}
    {{- end -}}
    {{- $_ := set $ "sysctlMutatorCerts" (dict "ca" $ca "cert" $cert "key" $key) -}}
  {{- end -}}
{{- end -}}
