# Self-hosting

The recommended way to run pdf_oxide_api is Docker Compose with a hardened,
resource-limited container.

```yaml
# docker-compose.yml (shipped in the repo)
services:
  pdf-oxide-api:
    image: ghcr.io/yfedoseev/pdf_oxide:latest
    ports: ["8080:8080"]
    read_only: true
    cap_drop: ["ALL"]
    security_opt: ["no-new-privileges:true"]
    mem_limit: 1g
    environment:
      RUST_LOG: info
    restart: unless-started
```

```bash
docker compose up -d
curl -s http://localhost:8080/healthz
```

## Behind a reverse proxy (TLS + auth)

For any network-exposed deployment, set an API key and terminate TLS at a proxy:

```bash
docker run -d -p 127.0.0.1:8080:8080 \
  -e PDF_OXIDE_API_KEY="$(openssl rand -hex 32)" \
  ghcr.io/yfedoseev/pdf_oxide:latest
```

Then front it with Caddy:

```
pdf.example.com {
    reverse_proxy 127.0.0.1:8080
}
```

See [Deployment](deployment.md) for Kubernetes and nginx examples, and
[Configuration](configuration.md) for all tunables.
