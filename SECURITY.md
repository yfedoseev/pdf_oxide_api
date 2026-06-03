# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

Use GitHub's private vulnerability reporting (Security tab → "Report a
vulnerability") for `yfedoseev/pdf_oxide_api`, or email the maintainers
(contact in [CONTRIBUTING.md](CONTRIBUTING.md)).

### What to Include

* Type of issue (e.g. SSRF, path traversal, resource exhaustion, parser bug)
* The affected endpoint / source file and the version or commit
* Configuration required to reproduce
* Step-by-step reproduction and, if possible, a proof of concept
* Impact, including how an attacker might exploit it

### What to Expect

* Acknowledgement within 48 hours
* A more detailed response within 7 days with next steps
* Progress updates through to a fix and coordinated disclosure (crediting you
  if desired)

## Security Model

pdf_oxide_api is designed safe-by-default for self-hosting confidential
documents (founding Issue #611, "Option A").

* **Stateless, no persistence.** PDFs are processed in memory and never written
  to disk. Nothing is held between requests: no database, no temp files, no
  caches of document content.
* **Hardened container.** Non-root (UID 65532), read-only root filesystem, no
  shell, no package manager (distroless/static base). `cap_drop: ALL`,
  `no-new-privileges`, tmpfs `/tmp`.
* **Bounded resources.** Configurable max request body (default 32 MiB),
  per-request timeout (default 30 s), and a bounded CPU worker pool with
  admission control to cap concurrent in-flight PDF jobs (bounds peak memory).
* **Panic isolation.** Each PDF operation runs inside `catch_unwind` on a
  worker thread, so a pathological PDF cannot crash the process or contaminate
  other requests.
* **No content logging.** PDF bytes, extracted text, filenames, and form field
  values are never logged or used as metric labels.
* **Minimal default attack surface.** Only the `pdf_oxide` default features are
  built (pure-Rust); no OCR/ML/GPU/rendering native dependencies.

## Deploying Untrusted-PDF Workloads

PDFs can be hostile. When processing untrusted input:

1. Keep the default body-size and timeout limits (or tighten them).
2. Run with the shipped hardened `docker-compose.yml` (read-only rootfs,
   `--memory`, `cap_drop: ALL`, `no-new-privileges`); the container cgroup
   bounds memory so a decompression bomb is killed, not the host.
3. Bind to localhost and front the service with a reverse proxy that
   terminates TLS and (if exposed) enforces authentication.
4. Keep the image current — releases auto-rebuild when the `pdf_oxide` engine
   ships a new version.

## Disclosure Policy

We confirm the problem, audit for similar issues, prepare a fix for supported
versions, and release as soon as practical. We ask researchers to give us
reasonable time before public disclosure and to avoid privacy violations or
service disruption.
