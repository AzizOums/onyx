{{/*
Expand the name of the chart.
*/}}
{{- define "lumen.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "lumen.fullname" -}}
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
{{- define "lumen.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Build a child resource name as `<fullname>-<suffix>`, capped at 63 chars to
satisfy the Kubernetes DNS-1123 label limit that applies to Services, Pods,
Deployments, HPAs, etc. Names that fit are used as-is. Longer names are
truncated and get a short hash of the full name, so two suffixes that share a
prefix (e.g. celery-worker-docfetching / celery-worker-docprocessing) cannot
collapse onto the same truncated name. The result is deterministic, so every
caller that passes the same suffix renders the same name and cross-references
stay consistent. Always use this instead of
  {{ include "lumen.fullname" . }}-<suffix>
Callers must pass `(list . "<suffix>")`.
*/}}
{{- define "lumen.resourceName" -}}
{{- $ctx := index . 0 -}}
{{- $suffix := index . 1 -}}
{{- $name := printf "%s-%s" (include "lumen.fullname" $ctx) $suffix -}}
{{- if gt (len $name) 63 -}}
{{- printf "%s-%s" ($name | trunc 54 | trimSuffix "-") (sha256sum $name | trunc 8) -}}
{{- else -}}
{{- $name -}}
{{- end -}}
{{- end }}

{{/*
Common labels
*/}}
{{- define "lumen.labels" -}}
helm.sh/chart: {{ include "lumen.chart" . }}
{{ include "lumen.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "lumen.selectorLabels" -}}
app.kubernetes.io/name: {{ include "lumen.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "lumen.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "lumen.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Set secret name
*/}}
{{- define "lumen.secretName" -}}
{{- default .secretName .existingSecret }}
{{- end }}

{{/*
Create env vars from secrets (global secrets only — skips entries with allPods: false)
*/}}
{{- define "lumen.envSecrets" -}}
    {{- range $secretSuffix, $secretContent := .Values.auth }}
    {{- $allPods := or (not (hasKey $secretContent "allPods")) (ne (toString $secretContent.allPods) "false") }}
    {{- if and (ne $secretSuffix "metricsAuth") (ne (toString $secretContent.enabled) "false") ($secretContent.secretKeys) $allPods }}
    {{- range $name, $key := $secretContent.secretKeys }}
- name: {{ $name | upper | replace "-" "_" | quote }}
  valueFrom:
    secretKeyRef:
      name: {{ include "lumen.secretName" $secretContent }}
      key: {{ default $name $key }}
    {{- end }}
    {{- end }}
    {{- end }}
{{- end }}

{{/*
Create env vars from secrets restricted to specific pods (entries with allPods: false)
*/}}
{{- define "lumen.envSecretsRestricted" -}}
    {{- range $secretSuffix, $secretContent := .Values.auth }}
    {{- $restricted := and (hasKey $secretContent "allPods") (eq (toString $secretContent.allPods) "false") }}
    {{- if and (ne $secretSuffix "metricsAuth") (ne (toString $secretContent.enabled) "false") ($secretContent.secretKeys) $restricted }}
    {{- range $name, $key := $secretContent.secretKeys }}
- name: {{ $name | upper | replace "-" "_" | quote }}
  valueFrom:
    secretKeyRef:
      name: {{ include "lumen.secretName" $secretContent }}
      key: {{ default $name $key }}
    {{- end }}
    {{- end }}
    {{- end }}
{{- end }}

{{/*
Inject metrics auth only into pods that expose the protected endpoint.
*/}}
{{- define "lumen.metricsAuthEnv" -}}
    {{- $metricsAuth := .Values.auth.metricsAuth | default dict }}
    {{- if dig "enabled" false $metricsAuth }}
- name: METRICS_AUTH_TOKEN
  valueFrom:
    secretKeyRef:
      name: {{ include "lumen.secretName" $metricsAuth }}
      key: {{ default "METRICS_AUTH_TOKEN" (dig "secretKeys" "METRICS_AUTH_TOKEN" "" $metricsAuth) }}
    {{- end }}
{{- end }}

{{/*
Helpers for mounting a psql convenience script into pods.
*/}}
{{- define "lumen.pgInto.enabled" -}}
{{- if and .Values.tooling .Values.tooling.pgInto .Values.tooling.pgInto.enabled }}true{{- end }}
{{- end }}

{{- define "lumen.pgInto.configMapName" -}}
{{- include "lumen.resourceName" (list . "pginto") -}}
{{- end }}

{{- define "lumen.pgInto.checksumAnnotation" -}}
{{- if (include "lumen.pgInto.enabled" .) }}
checksum/pginto: {{ include (print $.Template.BasePath "/tooling-pginto-configmap.yaml") . | sha256sum }}
{{- end }}
{{- end }}

{{- define "lumen.pgInto.volumeMount" -}}
{{- if (include "lumen.pgInto.enabled" .) }}
- name: pginto-script
  mountPath: {{ default "/usr/local/bin/pginto" .Values.tooling.pgInto.mountPath }}
  subPath: pginto
  readOnly: true
{{- end }}
{{- end }}

{{- define "lumen.pgInto.volume" -}}
{{- if (include "lumen.pgInto.enabled" .) }}
- name: pginto-script
  configMap:
    name: {{ include "lumen.pgInto.configMapName" . }}
    defaultMode: 0755
{{- end }}
{{- end }}

{{- define "lumen.renderVolumeMounts" -}}
{{- $pginto := include "lumen.pgInto.volumeMount" .ctx -}}
{{- $ca := include "lumen.customCACerts.volumeMount" .ctx -}}
{{- $postgresTls := include "lumen.postgresTls.volumeMount" .ctx -}}
{{- $redisTls := include "lumen.redisTls.volumeMount" .ctx -}}
{{- $existing := .volumeMounts -}}
{{- if or $pginto $ca $postgresTls $redisTls $existing -}}
volumeMounts:
{{- if $pginto }}
{{ $pginto | nindent 2 }}
{{- end }}
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- if $postgresTls }}
{{ $postgresTls | nindent 2 }}
{{- end }}
{{- if $redisTls }}
{{ $redisTls | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}

{{- define "lumen.renderVolumes" -}}
{{- $pginto := include "lumen.pgInto.volume" .ctx -}}
{{- $ca := include "lumen.customCACerts.volume" .ctx -}}
{{- $postgresTls := include "lumen.postgresTls.volume" .ctx -}}
{{- $redisTls := include "lumen.redisTls.volume" .ctx -}}
{{- $existing := .volumes -}}
{{- if or $pginto $ca $postgresTls $redisTls $existing -}}
volumes:
{{- if $pginto }}
{{ $pginto | nindent 2 }}
{{- end }}
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- if $postgresTls }}
{{ $postgresTls | nindent 2 }}
{{- end }}
{{- if $redisTls }}
{{ $redisTls | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}

{{/*
Return the configured autoscaling engine; defaults to HPA when unset.
*/}}
{{- define "lumen.autoscaling.engine" -}}
{{- $engine := default "hpa" .Values.autoscaling.engine -}}
{{- $engine | lower -}}
{{- end }}

{{/*
"true" when ENABLE_CRAFT is set in configMap, empty otherwise.
*/}}
{{- define "lumen.craftEnabled" -}}
{{- if eq (toString (index .Values.configMap "ENABLE_CRAFT" | default "")) "true" -}}true{{- end -}}
{{- end }}

{{/*
Sandbox container image and pull policy. Shared by the sandbox-pod PodTemplate
and the image-prepull DaemonSet: the prepull only pins layers the sandbox pods
actually use if both resolve to the identical reference, and a drift here is
invisible (the DaemonSet looks healthy while every sandbox still cold-pulls).
*/}}
{{- define "lumen.sandboxImage" -}}
{{- (index .Values.configMap "SANDBOX_CONTAINER_IMAGE") | default (printf "lumendotapp/sandbox:%s" (.Values.global.version | default .Chart.AppVersion)) -}}
{{- end }}

{{- define "lumen.sandboxImagePullPolicy" -}}
{{- (index .Values.configMap "SANDBOX_IMAGE_PULL_POLICY") | default .Values.global.pullPolicy -}}
{{- end }}

{{- define "lumen.sandboxProxyHost" -}}
{{- (index .Values.configMap "SANDBOX_PROXY_HOST") | default (printf "%s.%s.svc.cluster.local" (include "lumen.resourceName" (list . "sandbox-proxy")) .Release.Namespace) -}}
{{- end }}

{{- define "lumen.sandboxProxyPort" -}}
8080
{{- end }}

{{- define "lumen.sandboxPushDaemonPort" -}}
8731
{{- end }}

{{/* Sandbox egress-proxy env. Mirrors _proxy_main_container_env_vars(). */}}
{{- define "lumen.sandboxProxyEnv" -}}
{{- $proxyUrl := printf "http://sandbox-proxy:%v" .proxyPort }}
- { name: HTTPS_PROXY, value: "{{ $proxyUrl }}" }
- { name: HTTP_PROXY, value: "{{ $proxyUrl }}" }
- { name: https_proxy, value: "{{ $proxyUrl }}" }
- { name: http_proxy, value: "{{ $proxyUrl }}" }
- { name: NO_PROXY, value: "127.0.0.1,localhost" }
- { name: no_proxy, value: "127.0.0.1,localhost" }
- { name: NODE_EXTRA_CA_CERTS, value: "{{ .caBundleFile }}" }
- { name: REQUESTS_CA_BUNDLE, value: "{{ .caBundleFile }}" }
- { name: SSL_CERT_FILE, value: "{{ .caBundleFile }}" }
- { name: AWS_CA_BUNDLE, value: "{{ .caBundleFile }}" }
- { name: CURL_CA_BUNDLE, value: "{{ .caBundleFile }}" }
- { name: GIT_SSL_CAINFO, value: "{{ .caBundleFile }}" }
{{- end }}

{{/*
"true" when custom CA certificates are configured, empty otherwise.
*/}}
{{- define "lumen.customCACerts.enabled" -}}
{{- if and .Values.customCACerts .Values.customCACerts.enabled -}}true{{- end -}}
{{- end }}

{{/*
Volume sourcing the custom CA certificates from a Secret or ConfigMap.
*/}}
{{- define "lumen.customCACerts.volume" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
{{- if and .Values.customCACerts.secretName .Values.customCACerts.configMapName -}}
{{- fail "customCACerts.secretName and customCACerts.configMapName are mutually exclusive; set exactly one" -}}
{{- end -}}
{{- if .Values.customCACerts.secretName -}}
- name: custom-ca-certs
  secret:
    secretName: {{ .Values.customCACerts.secretName }}
{{- else if .Values.customCACerts.configMapName -}}
- name: custom-ca-certs
  configMap:
    name: {{ .Values.customCACerts.configMapName }}
{{- else -}}
{{- fail "customCACerts.enabled is true but neither customCACerts.secretName nor customCACerts.configMapName is set" -}}
{{- end -}}
{{- end -}}
{{- end }}

{{/*
Mount for the custom CA certificates. update-ca-certificates picks up
PEM files with a .crt suffix from /usr/local/share/ca-certificates.
*/}}
{{- define "lumen.customCACerts.volumeMount" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
- name: custom-ca-certs
  mountPath: /usr/local/share/ca-certificates
  readOnly: true
{{- end -}}
{{- end }}

{{/*
Env vars so Python HTTP clients (which default to certifi's bundle) trust
the system bundle regenerated by update-ca-certificates.
*/}}
{{- define "lumen.customCACerts.env" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
- name: REQUESTS_CA_BUNDLE
  value: /etc/ssl/certs/ca-certificates.crt
- name: SSL_CERT_FILE
  value: /etc/ssl/certs/ca-certificates.crt
{{- end -}}
{{- end }}

{{/*
Items to prepend to an exec-form container command so update-ca-certificates
runs first; sh passes the original command through via "$0" "$@".
Emits a single line ending with a comma.
*/}}
{{- define "lumen.customCACerts.commandPrefix" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
"/bin/sh", "-c", "update-ca-certificates && exec \"$0\" \"$@\"",
{{- end -}}
{{- end }}

{{/*
"true" when PostgreSQL TLS settings and a CA certificate source are configured.
*/}}
{{- define "lumen.postgresTls.enabled" -}}
{{- if and .Values.postgresTls .Values.postgresTls.enabled }}true{{- end -}}
{{- end }}

{{/*
Volume sourcing the PostgreSQL server CA. The backend validates the configured
path at startup, so mount exactly the configured key at its expected filename.
*/}}
{{- define "lumen.postgresTls.volume" -}}
{{- if include "lumen.postgresTls.enabled" . -}}
{{- $tls := .Values.postgresTls -}}
{{- if and $tls.caSecretName $tls.caConfigMapName -}}
{{- fail "postgresTls.caSecretName and postgresTls.caConfigMapName are mutually exclusive; set exactly one" -}}
{{- end -}}
{{- $caPath := $tls.caMountPath | default "/etc/postgres-ca/ca.crt" -}}
{{- $caKey := $tls.caKey | default "ca.crt" -}}
- name: postgres-ca
  {{- if $tls.caSecretName }}
  secret:
    secretName: {{ $tls.caSecretName }}
    items:
      - key: {{ $caKey }}
        path: {{ base $caPath }}
  {{- else if $tls.caConfigMapName }}
  configMap:
    name: {{ $tls.caConfigMapName }}
    items:
      - key: {{ $caKey }}
        path: {{ base $caPath }}
  {{- else -}}
  {{- fail "postgresTls.enabled is true but neither postgresTls.caSecretName nor postgresTls.caConfigMapName is set" -}}
  {{- end }}
{{- end -}}
{{- end }}

{{/* Mount for the PostgreSQL server CA. */}}
{{- define "lumen.postgresTls.volumeMount" -}}
{{- if include "lumen.postgresTls.enabled" . -}}
{{- $caPath := .Values.postgresTls.caMountPath | default "/etc/postgres-ca/ca.crt" -}}
- name: postgres-ca
  mountPath: {{ dir $caPath }}
  readOnly: true
{{- end -}}
{{- end }}

{{/*
"true" when Redis TLS settings and a CA certificate source are configured.
*/}}
{{- define "lumen.redisTls.enabled" -}}
{{- $tls := .Values.redisTls | default dict -}}
{{- if hasKey $tls "enabled" -}}
{{- if eq (toString (index $tls "enabled")) "true" }}true{{- end -}}
{{- end -}}
{{- end }}

{{/*
Redis hostname verification defaults to enabled. Only an explicit Boolean false
disables it; missing, null, and invalid values keep the secure default.
*/}}
{{- define "lumen.redisTls.checkHostname" -}}
{{- $tls := .Values.redisTls | default dict -}}
{{- $checkHostname := index $tls "checkHostname" -}}
{{- if kindIs "bool" $checkHostname -}}
{{- $checkHostname -}}
{{- else -}}
true
{{- end -}}
{{- end }}

{{/*
Volume sourcing the Redis server CA. The backend uses the configured path for
REDIS_SSL_CA_CERTS, so mount exactly the configured key at its expected name.
*/}}
{{- define "lumen.redisTls.volume" -}}
{{- if include "lumen.redisTls.enabled" . -}}
{{- $tls := .Values.redisTls | default dict -}}
{{- if and $tls.caSecretName $tls.caConfigMapName -}}
{{- fail "redisTls.caSecretName and redisTls.caConfigMapName are mutually exclusive; set exactly one" -}}
{{- end -}}
{{- $caPath := $tls.caMountPath | default "/etc/ssl/certs/redis-ca.crt" -}}
{{- $caKey := $tls.caKey | default "ca.crt" -}}
- name: redis-ca
  {{- if $tls.caSecretName }}
  secret:
    secretName: {{ $tls.caSecretName }}
    items:
      - key: {{ $caKey }}
        path: {{ base $caPath }}
  {{- else if $tls.caConfigMapName }}
  configMap:
    name: {{ $tls.caConfigMapName }}
    items:
      - key: {{ $caKey }}
        path: {{ base $caPath }}
  {{- else -}}
  {{- fail "redisTls.enabled is true but neither redisTls.caSecretName nor redisTls.caConfigMapName is set" -}}
  {{- end }}
{{- end -}}
{{- end }}

{{/* Mount for the Redis server CA. */}}
{{- define "lumen.redisTls.volumeMount" -}}
{{- if include "lumen.redisTls.enabled" . -}}
{{- $tls := .Values.redisTls | default dict -}}
{{- $caPath := $tls.caMountPath | default "/etc/ssl/certs/redis-ca.crt" -}}
- name: redis-ca
  mountPath: {{ $caPath }}
  subPath: {{ base $caPath }}
  readOnly: true
{{- end -}}
{{- end }}

{{/*
Model-server variant of the custom-CA env. The model servers run on a distroless
image with no shell to run update-ca-certificates, so instead of pointing at the
merged system store they hand the mount directory to the Python entrypoint, which
concatenates the mounted roots with certifi's public roots at startup. Any key
name(s) under the mount work, and the roots stay additive to the public roots.
*/}}
{{- define "lumen.customCACerts.modelServerEnv" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
- name: LUMEN_CUSTOM_CA_CERTS_DIR
  value: /etc/lumen/certs
{{- end -}}
{{- end }}

{{/*
Model-server variant of the custom-CA mount: mounts the bundle directory the
entrypoint reads (no update-ca-certificates step) at the path the env above names.
*/}}
{{- define "lumen.customCACerts.modelServerVolumeMount" -}}
{{- if include "lumen.customCACerts.enabled" . -}}
- name: custom-ca-certs
  mountPath: /etc/lumen/certs
  readOnly: true
{{- end -}}
{{- end }}

{{/*
Render a volumeMounts block for the model servers, combining pod-specific mounts
with the model-server custom-CA mount.
Usage: include "lumen.modelServer.volumeMountsWithCA" (dict "ctx" . "volumeMounts" <list>)
*/}}
{{- define "lumen.modelServer.volumeMountsWithCA" -}}
{{- $ca := include "lumen.customCACerts.modelServerVolumeMount" .ctx -}}
{{- $existing := .volumeMounts -}}
{{- if or $ca $existing -}}
volumeMounts:
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}

{{/*
Render a volumes block combining pod-specific volumes with the model-server
custom-CA volume. Usage: include "lumen.modelServer.volumesWithCA" (dict "ctx" . "volumes" <list>)
*/}}
{{- define "lumen.modelServer.volumesWithCA" -}}
{{- $ca := include "lumen.customCACerts.volume" .ctx -}}
{{- $existing := .volumes -}}
{{- if or $ca $existing -}}
volumes:
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}

{{/*
Render a volumeMounts block combining pod-specific mounts with the custom CA
mount. Usage: include "lumen.volumeMountsWithCA" (dict "ctx" . "volumeMounts" <list>)
*/}}
{{- define "lumen.volumeMountsWithCA" -}}
{{- $ca := include "lumen.customCACerts.volumeMount" .ctx -}}
{{- $postgresTls := include "lumen.postgresTls.volumeMount" .ctx -}}
{{- $redisTls := include "lumen.redisTls.volumeMount" .ctx -}}
{{- $existing := .volumeMounts -}}
{{- if or $ca $postgresTls $redisTls $existing -}}
volumeMounts:
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- if $postgresTls }}
{{ $postgresTls | nindent 2 }}
{{- end }}
{{- if $redisTls }}
{{ $redisTls | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}

{{/*
Render a volumes block combining pod-specific volumes with the custom CA
volume. Usage: include "lumen.volumesWithCA" (dict "ctx" . "volumes" <list>)
*/}}
{{- define "lumen.volumesWithCA" -}}
{{- $ca := include "lumen.customCACerts.volume" .ctx -}}
{{- $postgresTls := include "lumen.postgresTls.volume" .ctx -}}
{{- $redisTls := include "lumen.redisTls.volume" .ctx -}}
{{- $existing := .volumes -}}
{{- if or $ca $postgresTls $redisTls $existing -}}
volumes:
{{- if $existing }}
{{ toYaml $existing | nindent 2 }}
{{- end }}
{{- if $ca }}
{{ $ca | nindent 2 }}
{{- end }}
{{- if $postgresTls }}
{{ $postgresTls | nindent 2 }}
{{- end }}
{{- if $redisTls }}
{{ $redisTls | nindent 2 }}
{{- end }}
{{- end -}}
{{- end }}
