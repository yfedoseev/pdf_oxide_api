# Contributing to pdf_oxide_api

Thank you for your interest in contributing! pdf_oxide_api is a stateless,
single-shot REST service wrapping the [pdf_oxide](https://github.com/yfedoseev/pdf_oxide)
Rust library.

## Code of Conduct

This project adheres to the [Contributor Covenant](CODE_OF_CONDUCT.md). By
participating, you are expected to uphold it. Report unacceptable behavior by
opening an issue or contacting the maintainers.

## Getting Started

### Prerequisites

- **Rust** — the toolchain is pinned in `rust-toolchain.toml` (channel
  `1.88.0`); `rustup` will install it automatically. MSRV floor is 1.82.
- **Git**.
- Optional: **Docker** (to build/run the production image),
  **cargo-nextest** (`cargo install cargo-nextest`) for isolated test runs,
  **cargo-deny** and **cargo-audit** for the supply-chain gates.

### Build, test, lint

```bash
cargo build
cargo test                 # or: cargo nextest run
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo deny check
cargo audit
```

## Architecture (read this before adding an endpoint)

The single non-negotiable rule: **every `pdf_oxide` call must go through
`state.cpu.run(...)`** (the bounded rayon pool + tokio semaphore in
`src/blocking.rs`). Never call `pdf_oxide` directly in an `async fn` handler,
and never use `spawn_blocking` for it — that path is unbounded and will OOM
under large-PDF bursts. See `docs/releases/plans/v0.1.0/framework-decision.md`.

Other invariants:

- **Stateless / no persistence.** Do not write PDFs to disk or hold document
  state between requests.
- **No content logging.** Never log PDF bytes, extracted text, filenames, or
  field values — only sizes, counts, durations, route templates, sanitized
  error categories.
- **Errors** return through the single `ApiError` enum (`src/error.rs`) as
  RFC 9457 `application/problem+json`.
- **Profile** keeps `panic = "unwind"` — `catch_unwind` in the worker depends
  on it. Do not switch to `abort`.
- **OpenAPI** is the contract for n8n/Dify; keep handler annotations and
  `docs/openapi.yaml` in lockstep.

## Submitting Changes

1. Branch from `main`.
2. Keep changes additive and focused; follow SOLID/KISS/DRY and write tests
   first where practical.
3. Ensure `fmt`, `clippy -D warnings`, tests, `cargo deny`, and `cargo audit`
   all pass locally — CI enforces them.
4. Use Conventional Commit messages (e.g. `feat(forms): add /v1/forms/fill`).
5. Open a pull request describing the change and its rationale.

## License

By contributing, you agree your contributions are dual-licensed under
[MIT](LICENSE-MIT) and [Apache-2.0](LICENSE-APACHE), matching the project.
