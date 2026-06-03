# Security model

pdf_oxide_api is built to process **confidential** documents safely.

## No persistence
- PDFs are processed entirely in memory. No database, no temp files held
  between requests.
- Results carry `Cache-Control: no-store`.
- Run with a read-only root filesystem (the bundled compose file does).

## Hardened by default
- **Fail closed:** binding a non-loopback address without `PDF_OXIDE_API_KEY`
  is refused at startup.
- **Optional bearer auth** with a constant-time key comparison.
- **Request limits:** max body size, per-request timeout, max page count, max
  pipeline steps — all configurable. These bound PDF-bomb and resource-
  exhaustion attacks.
- **Admission control:** a bounded worker pool caps concurrent PDF jobs, which
  bounds peak memory regardless of request rate.
- **Panic isolation:** a pathological PDF that panics the parser is caught and
  returns a clean `500` without taking down the process.
- **Locked-down CORS** (off unless explicitly enabled) and conservative
  security headers (`X-Content-Type-Options`, `X-Frame-Options`,
  `Referrer-Policy`).
- **No content in logs or errors:** the RFC 9457 error envelope never echoes
  document content; logs never include PDF bytes/field values/filenames.

## Container
- Distroless, **non-root** (UID 65532), no shell.
- Single static binary; minimal attack surface.
- Images are signed (cosign keyless) with an SBOM + SLSA provenance attestation.

## TLS
Terminate TLS at a reverse proxy (Caddy / Traefik / nginx) in front of the
service. See [Deployment](deployment.md).

## Reporting
Please report vulnerabilities privately — see
[`SECURITY.md`](https://github.com/yfedoseev/pdf_oxide_api/blob/main/SECURITY.md).
