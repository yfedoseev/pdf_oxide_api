# Configuration

All configuration is via environment variables with the `PDF_OXIDE_API_`
prefix. Nothing is read from disk; no secrets are logged.

| Variable | Default | Purpose |
|---|---|---|
| `PDF_OXIDE_API_ADDR` | `0.0.0.0:8080` | Bind address. A non-loopback bind **requires** `PDF_OXIDE_API_KEY` (fail-closed). |
| `PDF_OXIDE_API_KEY` | _(unset)_ | If set, all `/v1/*` calls need `Authorization: Bearer <key>`. |
| `PDF_OXIDE_API_REQUIRE_AUTH` | `false` | If `true`, refuse to start on a non-loopback bind without a key (hard fail-closed). Default just warns. |
| `PDF_OXIDE_API_CPU_THREADS` | CPU count | Size of the rayon pool that runs PDF work. |
| `PDF_OXIDE_API_MAX_INFLIGHT` | `8` | Max concurrent PDF jobs (bounds peak memory). |
| `PDF_OXIDE_API_MAX_BODY_BYTES` | `33554432` (32 MiB) | Hard request-body ceiling. |
| `PDF_OXIDE_API_REQUEST_TIMEOUT_SECS` | `30` | Per-request wall-clock timeout. |
| `PDF_OXIDE_API_MAX_PAGES` | `2000` | Reject documents with more pages (PDF-bomb guard). |
| `PDF_OXIDE_API_MAX_PIPELINE_STEPS` | `16` | Max ops in a `/v1/pipeline` request. |
| `PDF_OXIDE_API_ENABLE_CORS` | `false` | Enable permissive CORS (off by default). |
| `RUST_LOG` | `info` | Tracing filter (e.g. `pdf_oxide_api=debug,tower_http=info`). |

## Memory budget

Peak memory is roughly `MAX_BODY_BYTES × MAX_INFLIGHT × parse-overhead`. With the
defaults (32 MiB × 8) the raw input ceiling is ~256 MiB; the bundled
`docker-compose.yml` sets a 1 GiB limit for parse headroom. Lower `MAX_INFLIGHT`
on memory-constrained hosts.

## Logging

Logs are structured (JSON in containers) and **never** contain PDF bytes,
extracted text, field values, or filenames — only sizes, counts, route
templates, status codes, and sanitized error categories.
