# PDFOxide API — a fast, tiny, stateless self-hosted PDF REST API in Docker

> **PDFOxide API** (`pdf_oxide_api`) is a stateless single-shot PDF REST API
> over the [`pdf_oxide`](https://crates.io/crates/pdf_oxide)
> Rust engine. **Extract** text / Markdown / HTML, **read & fill** AcroForm
> fields (any UTF-8 script — CJK, Arabic, Hebrew, …), **merge**, **split**, read
> **metadata** and **page geometry**, and **chain** all of those in one
> `POST /v1/pipeline` call. PDF in → result out; nothing is stored between
> requests. One small Docker image, no database, no telemetry — self-host in
> one command.

The fastest, smallest, most secure self-hostable PDF REST API. A single static
Rust binary in a distroless container exposes a clean JSON HTTP API. Stateless
by design: every request is self-contained, nothing is persisted, which makes
it a good fit for confidential documents and automation platforms like **n8n**
and **Dify**.

[![GHCR Image](https://img.shields.io/badge/ghcr.io-pdf__oxide-2496ED?logo=docker)](https://github.com/yfedoseev/pdf_oxide_api/pkgs/container/pdf_oxide)
[![Image Size](https://img.shields.io/badge/image%20size-14.5MB-brightgreen?logo=docker)](https://github.com/yfedoseev/pdf_oxide_api/pkgs/container/pdf_oxide)
[![CI](https://github.com/yfedoseev/pdf_oxide_api/workflows/CI/badge.svg)](https://github.com/yfedoseev/pdf_oxide_api/actions)
[![Release](https://github.com/yfedoseev/pdf_oxide_api/workflows/Release/badge.svg)](https://github.com/yfedoseev/pdf_oxide_api/actions)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](https://opensource.org/licenses)
[![Engine: pdf_oxide](https://img.shields.io/crates/v/pdf_oxide.svg?label=engine%3A%20pdf__oxide)](https://crates.io/crates/pdf_oxide)

> Image size is the measured `linux/amd64` build (distroless static base);
> arm64 is within a few hundred KB.

## Quick Start

```bash
# Run the API (zero config; binds 0.0.0.0:8080 inside the container)
docker run --rm -p 8080:8080 ghcr.io/yfedoseev/pdf_oxide:latest

# Liveness / versions
curl -s http://localhost:8080/healthz   # {"status":"ok"}
curl -s http://localhost:8080/version   # API + engine versions
```

### Every data endpoint accepts three input encodings

The examples below use `multipart/form-data` (best for `curl` and n8n), but you
can send the same request as JSON-with-base64 (best for Dify / JSON-only tools)
or as a raw PDF body:

```bash
# 1) multipart — the PDF rides in a `file` part
curl -s -F file=@doc.pdf http://localhost:8080/v1/extract/text

# 2) application/json — PDF as base64 in `pdf_base64`
curl -s -X POST http://localhost:8080/v1/extract/text \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(base64 -w0 doc.pdf)\"}"

# 3) raw body — the whole body is the PDF
curl -s --data-binary @doc.pdf \
  -H 'content-type: application/pdf' \
  http://localhost:8080/v1/extract/text
```

### Common operations

```bash
# Extract Markdown (heading detection on by default) — great for RAG / LLMs
curl -s -F file=@paper.pdf http://localhost:8080/v1/extract/markdown | jq -r .markdown

# Extract a page range only
curl -s -F file=@paper.pdf -F pages=1-3,5 http://localhost:8080/v1/extract/text

# List AcroForm fields (name, type, value, read-only)
curl -s -F file=@form.pdf http://localhost:8080/v1/forms/fields | jq .

# Fill a form with UTF-8 values (CJK, Arabic, Hebrew, …), get the filled PDF back.
# The field map is a separate `fields` part (a JSON object), NOT `options=`.
curl -s -F file=@form.pdf \
  -F 'fields={"full_name":"山田太郎","city":"東京都千代田区"}' \
  http://localhost:8080/v1/forms/fill -o filled.pdf

# Merge two or more PDFs (in upload order)
curl -s -F file=@a.pdf -F file=@b.pdf http://localhost:8080/v1/docs/merge -o merged.pdf

# Split into one PDF per page, returned as a ZIP (use `pages` to subset)
curl -s -F file=@doc.pdf http://localhost:8080/v1/docs/split -o pages.zip

# Metadata and per-page geometry
curl -s -F file=@doc.pdf http://localhost:8080/v1/docs/metadata  | jq .
curl -s -F file=@doc.pdf http://localhost:8080/v1/docs/page-info | jq .

# Chain ops in one request: fill, then extract Markdown.
# The steps live in an `operations` array; data ops (extract_*, metadata,
# page_info) must be the LAST step.
curl -s -F file=@form.pdf \
  -F 'operations=[{"op":"fill","fields":{"full_name":"山田太郎"}},{"op":"extract_markdown"}]' \
  http://localhost:8080/v1/pipeline | jq -r .markdown
```

The interactive API reference is served by the running container at
`http://localhost:8080/docs`, and the machine contract at
`http://localhost:8080/openapi.json` (or `/openapi.yaml`).

## Why pdf_oxide_api?

- **Fast** — a single static Rust binary, sub-second cold start; latency is
  dominated by the PDF, not the HTTP layer.
- **Small** — distroless image, no JVM, no shell, no package manager.
- **Secure** — non-root, read-only rootfs, no persistence, bounded body size,
  per-request timeout, RFC 9457 errors, supply-chain-hardened releases.
- **Stateless** — PDF in, result out; nothing stored between requests.
- **Three input encodings** — multipart, JSON+base64, or raw body, so it drops
  into both binary-upload (`curl`/n8n) and JSON-only (Dify) clients.
- **Any-script form fill** — AcroForm values in any UTF-8 script (CJK, Arabic,
  Hebrew, …) round-trip verbatim with no mojibake — the case where Stirling-PDF
  historically struggled (validated end-to-end against `pdf_oxide` 0.3.59).

## Endpoints

| Method | Path | Purpose | Returns |
|---|---|---|---|
| POST | `/v1/extract/text` | Extract plain text (optional `pages`) | JSON |
| POST | `/v1/extract/markdown` | Convert to Markdown (`detect_headings`, `pages`) | JSON |
| POST | `/v1/extract/html` | Convert to HTML (`detect_headings`, `pages`) | JSON |
| POST | `/v1/forms/fields` | List AcroForm fields, types, values, read-only flag | JSON |
| POST | `/v1/forms/fill` | Fill AcroForm fields (any UTF-8 script); optional `flatten` | PDF |
| POST | `/v1/docs/merge` | Merge ≥2 PDFs in upload order | PDF |
| POST | `/v1/docs/split` | One PDF per selected page (optional `pages`) | ZIP |
| POST | `/v1/docs/metadata` | Title, author, subject, keywords, producer, creation date | JSON |
| POST | `/v1/docs/page-info` | Per-page index, width, height, rotation | JSON |
| POST | `/v1/pipeline` | Chain ops in one request (`operations` array) | PDF or JSON |
| GET | `/healthz` | Liveness probe | JSON |
| GET | `/readyz` | Readiness probe (503 while draining) | JSON |
| GET | `/version` | API + `pdf_oxide` engine versions | JSON |
| GET | `/metrics` | Prometheus metrics (RED on matched routes) | text |
| GET | `/openapi.json`, `/openapi.yaml` | OpenAPI 3.1 contract | JSON / YAML |
| GET | `/docs` | Interactive API reference (Scalar; `swagger-ui` feature, on by default) | HTML |

Operational endpoints (`/healthz`, `/readyz`, `/version`, `/metrics`) are always
unauthenticated. Auth, when enabled, applies to `/v1/*` only.

### Request parameters

- **`pages`** (extract\*, split): absent → all pages; a number `n` → page `n`
  (1-based); a string like `"1-3,5"` → those 1-based pages/ranges.
- **`detect_headings`** (markdown/html): boolean, default `true`.
- **`fields`** (forms/fill): a JSON object `{ "field name": value }`, sent as
  its own multipart `fields` part or a top-level `fields` key in JSON. Values
  may be string, number, boolean, array of strings (multi-choice), or `null`.
- **`flatten`** (forms/fill): boolean, default `false` — flatten the form after
  filling so values are no longer editable.
- **`operations`** (pipeline): a JSON array of steps. `fill` is a transform
  (PDF → PDF); `extract_text` / `extract_markdown` / `extract_html` /
  `metadata` / `page_info` are terminal data ops and must be the last step.

## Form Filling (any UTF-8 script — CJK, Arabic, Hebrew, …)

`POST /v1/forms/fill` takes the PDF plus a `fields` JSON object and returns the
filled PDF. Field **values** are passed to the engine verbatim as UTF-8 — no
transcoding, no ASCII assumption — and are written as proper UTF-16BE text
strings, so CJK, Arabic, Hebrew, and any other Unicode values round-trip
without mojibake (verified end-to-end against `pdf_oxide` 0.3.59). Add
`flatten=true` to bake the values in.

> Note: correct on-screen rendering of RTL/complex scripts also depends on the
> form's embedded font including those glyphs. The engine currently does not
> preserve field `/T` names across a fill+save, so a re-read of a filled form
> returns the correct values but not their original field names.

```bash
# multipart
curl -s -F file=@form.pdf \
  -F 'fields={"氏名":"山田太郎","住所":"東京都千代田区"}' \
  -F flatten=true \
  http://localhost:8080/v1/forms/fill -o filled.pdf

# or JSON+base64
curl -s -X POST http://localhost:8080/v1/forms/fill \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(base64 -w0 form.pdf)\",\"fields\":{\"氏名\":\"山田太郎\"}}" \
  -o filled.pdf
```

## Self-Hosting

A hardened `docker-compose.yml` ships in the repo (read-only rootfs, nonroot,
`cap_drop: ALL`, `no-new-privileges`, tmpfs `/tmp`). Put a reverse proxy
(Caddy / Traefik / nginx) in front for TLS.

```bash
docker compose up -d
curl -s http://localhost:8080/healthz
```

### Configuration

All knobs use the `PDF_OXIDE_API_` prefix and have safe defaults — none are
required.

| Variable | Default | Meaning |
|---|---|---|
| `PDF_OXIDE_API_ADDR` | `0.0.0.0:8080` | Bind address |
| `PDF_OXIDE_API_KEY` | _(unset)_ | Bearer token; when set, all `/v1/*` calls need `Authorization: Bearer <key>` |
| `PDF_OXIDE_API_REQUIRE_AUTH` | `false` | Refuse to start on a non-loopback bind that has no API key (hard fail-closed) |
| `PDF_OXIDE_API_ENABLE_CORS` | `false` | Send permissive CORS headers when `true` |
| `PDF_OXIDE_API_CPU_THREADS` | num CPUs | rayon CPU pool size |
| `PDF_OXIDE_API_MAX_INFLIGHT` | `8` | Concurrent-job admission cap |
| `PDF_OXIDE_API_MAX_BODY_BYTES` | `33554432` | Max upload (32 MiB) |
| `PDF_OXIDE_API_REQUEST_TIMEOUT_SECS` | `30` | Per-request timeout |
| `PDF_OXIDE_API_MAX_PAGES` | `2000` | Reject documents larger than this (PDF-bomb guard) |
| `PDF_OXIDE_API_MAX_PIPELINE_STEPS` | `16` | Max steps in one `/v1/pipeline` request |
| `RUST_LOG` | `info` | Log filter |

Peak memory is bounded roughly by `MAX_BODY_BYTES × MAX_INFLIGHT`, so tune those
two together for your memory budget.

## Authentication

Auth is **off by default** for localhost/trusted-network use. Set
`PDF_OXIDE_API_KEY` to require a bearer token on every `/v1/*` request:

```bash
docker run --rm -p 8080:8080 -e PDF_OXIDE_API_KEY=s3cret ghcr.io/yfedoseev/pdf_oxide:latest
curl -s -H 'Authorization: Bearer s3cret' -F file=@doc.pdf http://localhost:8080/v1/extract/text
```

The key is compared in constant time and never logged. A non-loopback bind with
no key emits a loud startup warning; set `PDF_OXIDE_API_REQUIRE_AUTH=true` to
turn that into a hard startup failure instead.

## Errors

Errors are RFC 9457 `application/problem+json` (a `type`, `title`, `status`, and
`detail`). Typical statuses: `400` (bad request / malformed input), `401`
(missing or wrong bearer token), `408` (request timeout), `413` (body over
`MAX_BODY_BYTES`), and a limit-exceeded response when a document exceeds
`MAX_PAGES` or a pipeline exceeds `MAX_PIPELINE_STEPS`. PDF bytes, filenames, and
field values are never logged.

## Security

Stateless and no-persistence by design: PDFs are processed in memory and never
written to disk; the container runs non-root on a read-only filesystem with no
shell. Request body size and per-request timeout are bounded. See
[SECURITY.md](SECURITY.md).

Verify a published image (keyless cosign):

```bash
cosign verify ghcr.io/yfedoseev/pdf_oxide:0.1.0 \
  --certificate-identity-regexp 'https://github.com/yfedoseev/pdf_oxide_api/.+' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

## Integrations

- **n8n** — use an HTTP Request node with `multipart/form-data` and a binary
  `file` field; add `Authorization: Bearer` if you enabled a key. Import
  `/openapi.json` for typed nodes.
- **Dify** — add a custom tool by importing the OpenAPI 3.1 spec
  (`/openapi.json`); the JSON+base64 encoding suits Dify's JSON-only tool calls.

## pdf_oxide_api vs Stirling-PDF

Choose pdf_oxide_api when you need a small, stateless, programmatic PDF API for
automation; choose Stirling-PDF when you need a broad interactive browser UI.

| | pdf_oxide_api | Stirling-PDF |
|---|---|---|
| Runtime | Single static Rust binary | JVM (Java) |
| Image size | ~14.5MB (distroless) | hundreds of MB |
| State | Stateless, nothing stored | Server app, broader surface |
| Primary use | Programmatic JSON API for automation | Interactive browser UI + API |
| UTF-8 / CJK / RTL form fill | First-class (verbatim, no mojibake) | Historically problematic |
| Scope | Focused: extract, fill, merge/split, pipeline | 50+ tools |
| Cold start | Sub-second | Slower (JVM warmup) |
| License | MIT OR Apache-2.0 | MIT |

> Confirm Stirling-PDF specifics against their repo before relying on any cell.

## Documentation

Full documentation (Quick Start, Self-Hosting, Auth, API Reference, Errors,
Architecture, Integrations) lives in [`docs/`](docs/src/SUMMARY.md) and is
published at <https://pdf.oxide.fyi/>.

## FAQ

**What is the smallest self-hosted PDF API?** pdf_oxide_api ships a single
static Rust binary in a distroless image, far smaller than JVM-based tools.

**How do I fill a non-Latin (CJK / Arabic / Hebrew) PDF form via an API?** POST
the PDF and a `fields` JSON object to `/v1/forms/fill`; any UTF-8 value
round-trips verbatim, written as UTF-16BE so it renders without mojibake.

**Is there a stateless PDF API that doesn't store my documents?** Yes — this
one. Nothing is persisted between requests: no database, no temp files.

**Does pdf_oxide_api need a database?** No — it is fully stateless.

**How do I convert PDF to Markdown for a RAG pipeline over HTTP?** POST the PDF
to `/v1/extract/markdown`.

**How do I run a PDF processing API in Docker with one command?**
`docker run --rm -p 8080:8080 ghcr.io/yfedoseev/pdf_oxide:latest`.

## License

Dual-licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option. Use freely — no copyleft.

## Citation

```bibtex
@software{pdf_oxide_api,
  title  = {PDFOxide API: a fast, tiny, stateless self-hostable PDF REST API},
  author = {Fedoseev, Yury},
  year   = {2026},
  url    = {https://github.com/yfedoseev/pdf_oxide_api}
}
```
