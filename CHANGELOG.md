# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The API release version **mirrors the embedded `pdf_oxide` engine version** it
links: each engine bump re-versions the API to the same number. The engine
version is also surfaced as metadata by `GET /version` and the
`fyi.oxide.pdf_oxide.version` image label.

## [Unreleased]

## [0.3.64] - 2026-06-14

> Engine bump to `pdf_oxide` 0.3.64 (version-mirroring release). The API
> release version, the pinned engine, the OpenAPI contract, `llms.txt`, and
> this changelog move in lock-step, verified by the CI version-sync gate.

### Changed
- Bumped the embedded `pdf_oxide` engine to **0.3.64**; the mirrored API
  release version moves `0.3.63` → `0.3.64` in step.

## [0.3.63] - 2026-06-11

> Engine bump to `pdf_oxide` 0.3.63 (version-mirroring release). The API
> release version, the pinned engine, the OpenAPI contract, `llms.txt`, and
> this changelog are now verified in lock-step by a CI version-sync gate so a
> bump can never again ship with a half-updated set of files.

### Changed
- Bumped the embedded `pdf_oxide` engine to **0.3.63**; the mirrored API
  release version moves `0.3.61` → `0.3.63` in step.

### Added
- CI **version-sync gate** (`.github/scripts/check-version-sync.sh`, run as the
  `version-sync` job in `ci.yml`): asserts that `Cargo.toml` (package version +
  `pdf_oxide` dependency), `Cargo.lock`, `openapi.yaml`, `openapi.json`,
  `llms.txt`, and the top `CHANGELOG.md` entry all carry the **same** version
  string. Catches the drift class from the 0.3.61 bump, where the manifest was
  updated but `openapi.*` and `llms.txt` were left behind (fixed in #6).

## [0.3.61] - 2026-06-09

> Engine bump to `pdf_oxide` 0.3.61 and adoption of version mirroring: the API
> release version now tracks the embedded engine version. `GET /version` and
> extract responses report the exact linked engine version instead of a
> hardcoded major-only string.

### Changed
- Bumped the embedded `pdf_oxide` engine to **0.3.61**.
- Adopted a version-mirroring policy: the API release version now tracks the
  `pdf_oxide` version it links (this release jumps `0.1.0` → `0.3.61`).

### Fixed
- `GET /version` and extract responses now report the **exact** linked engine
  version (e.g. `0.3.61`) instead of a hardcoded major-only `"0.3"` string;
  `PDF_OXIDE_VERSION` is sourced from `pdf_oxide::VERSION`. (#4)

## [0.1.0] - 2026-06-03

> First release — a stateless single-shot PDF REST API over the `pdf_oxide`
> engine: extract text/markdown/html, fill AcroForm fields (any UTF-8 script —
> CJK, Arabic, Hebrew), merge/split, and chain ops in one request. PDF in →
> result out, nothing persisted. Ships as a hardened, signed, ~14.5 MB
> distroless image.

### Added
- HTTP service on axum 0.8 + tokio, with a bounded rayon CPU pool + semaphore
  admission control for all `pdf_oxide` work (no `spawn_blocking`); panics in a
  worker are isolated via `catch_unwind`.
- **Extraction:** `POST /v1/extract/text`, `/v1/extract/markdown` (heading
  detection), `/v1/extract/html` — with an optional `pages` selection
  (`"1-3,5"`).
- **Forms (the issue #611 hero feature):** `POST /v1/forms/fields` (introspect
  AcroForm fields) and `POST /v1/forms/fill` (fill from a JSON map, optional
  `flatten`). Field values are passed to `pdf_oxide` **verbatim as UTF-8** and
  written as UTF-16BE, so **CJK, Arabic, Hebrew, and any Unicode round-trip with
  no mojibake** — covered by a gating acceptance test against `pdf_oxide` 0.3.59.
- **Document ops:** `POST /v1/docs/merge`, `/v1/docs/split` (one PDF per page,
  returned as a ZIP), `/v1/docs/metadata`, `/v1/docs/page-info`.
- **`POST /v1/pipeline`** — chain ops over one in-memory parse (e.g. fill →
  extract); a data-producing op must be last. `max_pipeline_steps` enforced.
- Dual request encoding on every data endpoint: `multipart/form-data` (`file`
  parts), `application/json` (`pdf_base64` / `pdfs_base64`), and raw body.
- Operational endpoints: `GET /healthz`, `GET /readyz` (503 while draining),
  `GET /version` (reports the embedded `pdf_oxide` version), `GET /metrics`.
- `healthcheck` subcommand for the no-shell container `HEALTHCHECK`.
- RFC 9457 `application/problem+json` error envelope via a single `ApiError`
  with a **variant-aware `pdf_oxide::Error` mapping** that never leaks document
  content (regression-tested).
- Hardening: env-configurable limits (max body 32 MiB, request timeout 30 s,
  max pages 2000, max in-flight 8, max pipeline steps 16); `Cache-Control:
  no-store` on results; optional bearer auth; a **loud startup warning** on a
  non-loopback bind without an API key (opt into hard fail-closed with
  `PDF_OXIDE_API_REQUIRE_AUTH=true`); graceful-drain readiness.
- Hardened multi-stage `Dockerfile` (static musl on Chainguard `static`,
  cargo-chef caching, mimalloc) and a hardened `docker-compose.yml`.
- CI (fmt, clippy `-D warnings`, test, cargo-deny, cargo-audit, MSRV, Docker
  build + Trivy + smoke test), release workflow (multi-arch buildx, cosign
  keyless sign, SBOM + SLSA provenance attest), and the cross-repo
  `pdf-oxide-released` rebuild trigger with a crates.io poll fallback.
- SEO/GEO docs assets: `README.md`, `llms.txt`, `.devin/wiki.json`,
  `openapi.yaml` (OpenAPI 3.1) + served `/openapi.json`, and an mdBook docs
  site.

[Unreleased]: https://github.com/yfedoseev/pdf_oxide_api/compare/v0.3.64...HEAD
[0.3.64]: https://github.com/yfedoseev/pdf_oxide_api/compare/v0.3.63...v0.3.64
[0.3.63]: https://github.com/yfedoseev/pdf_oxide_api/compare/v0.3.61...v0.3.63
[0.3.61]: https://github.com/yfedoseev/pdf_oxide_api/compare/v0.1.0...v0.3.61
[0.1.0]: https://github.com/yfedoseev/pdf_oxide_api/releases/tag/v0.1.0
