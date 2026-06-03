# Deployment

## Docker

```bash
docker run -d --name pdf-oxide-api \
  -p 127.0.0.1:8080:8080 \
  --read-only --cap-drop ALL --security-opt no-new-privileges \
  -m 1g \
  -e PDF_OXIDE_API_KEY="$(openssl rand -hex 32)" \
  ghcr.io/yfedoseev/pdf_oxide:latest
```

Pin a digest for reproducibility:
`ghcr.io/yfedoseev/pdf_oxide@sha256:...`.

## Kubernetes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata: { name: pdf-oxide-api }
spec:
  replicas: 2
  selector: { matchLabels: { app: pdf-oxide-api } }
  template:
    metadata: { labels: { app: pdf-oxide-api } }
    spec:
      containers:
        - name: api
          image: ghcr.io/yfedoseev/pdf_oxide:latest
          ports: [{ containerPort: 8080 }]
          envFrom: [{ secretRef: { name: pdf-oxide-api } }]  # PDF_OXIDE_API_KEY
          resources:
            limits: { memory: "1Gi", cpu: "1" }
          readinessProbe: { httpGet: { path: /readyz, port: 8080 } }
          livenessProbe:  { httpGet: { path: /healthz, port: 8080 } }
          securityContext:
            runAsNonRoot: true
            readOnlyRootFilesystem: true
            allowPrivilegeEscalation: false
            capabilities: { drop: ["ALL"] }
```

The service is stateless, so scale horizontally with plain replicas behind any
load balancer — no sticky sessions, no shared storage. `/readyz` returns 503
during graceful shutdown so the LB drains in-flight requests.

## Reverse proxy (nginx)

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
    client_max_body_size 32m;   # match PDF_OXIDE_API_MAX_BODY_BYTES
}
```
