# pdf_oxide_api

**The fastest, smallest, most secure self-hostable PDF REST API.** A stateless
single-shot HTTP service that wraps the [`pdf_oxide`](https://github.com/yfedoseev/pdf_oxide)
Rust engine: send a PDF, get a result back, nothing is kept.

```bash
docker run --rm -p 8080:8080 ghcr.io/yfedoseev/pdf_oxide:latest
curl -s -F file=@invoice.pdf http://localhost:8080/v1/extract/text
```

## Why

- **Self-hosted & private.** PDFs with confidential or personal data never
  leave your infrastructure. No database, no temp files held between requests,
  no telemetry. Results are sent `Cache-Control: no-store`.
- **Tiny & hardened.** A single static Rust binary in a distroless,
  non-root, read-only-rootfs container (~8–16 MB).
- **Built for automation.** A clean REST + JSON contract (OpenAPI 3.1) that
  drops straight into n8n, Dify, and any HTTP-capable workflow tool.
- **Japanese / CJK form-fill as a first-class feature** — values are passed
  through verbatim as UTF-8.

## What it does (v0.1.0)

| Family | Endpoints |
|---|---|
| Extraction | `text`, `markdown`, `html` |
| Forms | `fields` (introspect), `fill` |
| Document ops | `merge`, `split`, `metadata`, `page-info` |
| Pipeline | `pipeline` (chain ops in one request) |

See the [Quick start](quick-start.md) to make your first request, or the
[API reference](api-reference.md) for the full contract.
