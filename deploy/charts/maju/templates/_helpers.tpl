{{/* Standard naming/labels helpers. */}}

{{- define "maju.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "maju.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- if contains $name .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{- define "maju.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "maju.labels" -}}
helm.sh/chart: {{ include "maju.chart" . }}
{{ include "maju.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: maju
{{- end -}}

{{- define "maju.selectorLabels" -}}
app.kubernetes.io/name: {{ include "maju.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* Relay-specific selector: scopes the relay Deployment + Service so they do
     not also match the quickstart MinIO pods, which share the base
     selectorLabels but carry their own component label. */}}
{{- define "maju.relaySelectorLabels" -}}
{{ include "maju.selectorLabels" . }}
app.kubernetes.io/component: relay
{{- end -}}

{{- define "maju.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "maju.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "maju.image" -}}
{{- $tag := default .Chart.AppVersion .Values.image.tag -}}
{{- printf "%s:%s" .Values.image.repository $tag -}}
{{- end -}}

{{/*
Name of the chart-managed Secret holding relay-identity material and any
chart-composed connection strings.
*/}}
{{- define "maju.chartSecretName" -}}
{{- printf "%s-relay" (include "maju.fullname" .) -}}
{{- end -}}

{{/*
The Secret name the relay should pull env from. If the operator supplied
secrets.existingSecret, use that. Otherwise use the chart-managed one.
*/}}
{{- define "maju.envSecretName" -}}
{{- if .Values.secrets.existingSecret -}}
{{- .Values.secrets.existingSecret -}}
{{- else -}}
{{- include "maju.chartSecretName" . -}}
{{- end -}}
{{- end -}}

{{/* Host derived from relayUrl, used as ingress default + media domain. */}}
{{- define "maju.relayHost" -}}
{{- $url := required "relayUrl is required: set --set relayUrl=wss://your.domain" .Values.relayUrl -}}
{{- $stripped := $url | replace "wss://" "" | replace "ws://" "" | replace "https://" "" | replace "http://" "" -}}
{{- first (splitList "/" $stripped) -}}
{{- end -}}

{{/* Default media base URL: https://<host>/media derived from relayUrl. */}}
{{- define "maju.mediaBaseUrl" -}}
{{- if .Values.mediaBaseUrl -}}
{{- .Values.mediaBaseUrl -}}
{{- else -}}
{{- printf "https://%s/media" (include "maju.relayHost" .) -}}
{{- end -}}
{{- end -}}

{{/* Quickstart-only in-cluster service hostnames (eval profile). */}}
{{- define "maju.minioFullname" -}}
{{- printf "%s-minio" (include "maju.fullname" .) -}}
{{- end -}}

{{/* In-cluster MinIO endpoint, used when minio.enabled and s3.endpoint unset. */}}
{{- define "maju.minioEndpoint" -}}
{{- printf "http://%s.%s.svc.cluster.local:9000" (include "maju.minioFullname" .) .Release.Namespace -}}
{{- end -}}

{{/* Minimum number of relay replicas the release can run. */}}
{{- define "maju.minimumReplicas" -}}
{{- if .Values.autoscaling.enabled -}}
{{- .Values.autoscaling.minReplicas -}}
{{- else -}}
{{- .Values.replicaCount -}}
{{- end -}}
{{- end -}}

{{/* Effective huddle-audio availability. Nil means safe chart default: on for
     one replica, off for multi-pod until an SFU/shared-room story exists. */}}
{{- define "maju.huddleAudioAvailable" -}}
{{- if kindIs "invalid" .Values.relay.huddleAudioAvailable -}}
{{- if gt (include "maju.minimumReplicas" . | int) 1 -}}false{{- else -}}true{{- end -}}
{{- else -}}
{{- .Values.relay.huddleAudioAvailable -}}
{{- end -}}
{{- end -}}

{{/* Effective S3 endpoint: explicit s3.endpoint wins, else bundled MinIO. */}}
{{- define "maju.s3Endpoint" -}}
{{- if .Values.s3.endpoint -}}
{{- .Values.s3.endpoint -}}
{{- else if .Values.minio.enabled -}}
{{- include "maju.minioEndpoint" . -}}
{{- end -}}
{{- end -}}

{{- define "maju.pairingRelaySelectorLabels" -}}
{{ include "maju.selectorLabels" . }}
app.kubernetes.io/component: pairing-relay
{{- end -}}
