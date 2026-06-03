# Architecture

pdf_oxide_api is deliberately small: a thin, stateless HTTP layer over the
`pdf_oxide` Rust engine.

## Request lifecycle

```
client ──HTTP──▶ axum router
                   │  (tower layers: trace, catch-panic, compression,
                   │   timeout, body-limit, CORS, security headers, metrics)
                   ▼
              auth (on /v1 only, if a key is set)
                   ▼
              PdfInput extractor  (multipart / JSON base64 / raw)
                   ▼
              handler ── state.cpu.run(closure) ──▶ bounded rayon pool
                   │                                   │  (semaphore admission
                   │                                   │   control + catch_unwind)
                   ◀──────────── result ──────────────┘
                   ▼
              JSON / application/pdf / application/zip   (Cache-Control: no-store)
```

## Why stateless

PDF in → result out, nothing held between requests. This is the simplest thing
that meets the issue-#611 workflow and is **safe by default** for confidential
documents: no sessions, no shared storage, no temp files. It also scales
trivially — run N replicas behind a plain load balancer.

## The one rule

Every `pdf_oxide` call goes through `state.cpu.run(...)`. `pdf_oxide` work is
synchronous and CPU-bound; running it directly on the tokio runtime would
starve the async workers, and `spawn_blocking` (an unbounded ~512-thread pool)
would let a burst of large PDFs exhaust memory. Instead a fixed-size **rayon**
pool fronted by a tokio **semaphore** gives real admission control (bounding
peak memory) and bridges the result back over a `oneshot`. A panic inside the
worker is caught (`catch_unwind`) and surfaced as a clean `500`.

## Engine boundary

The service depends on the published `pdf_oxide` crate as a library and never
forks or vendors it. The compiled engine version is reported by `GET /version`
and stamped as an image label — the contract the auto-rebuild pipeline keys on.
