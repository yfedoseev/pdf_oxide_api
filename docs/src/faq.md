# FAQ

**Is my PDF stored anywhere?**
No. PDFs are processed in memory and discarded when the request ends. There is
no database and no temp file held between requests, and results are sent
`Cache-Control: no-store`.

**Does it phone home / send telemetry?**
No. The default image emits nothing externally. OpenTelemetry export is an
opt-in build feature.

**How big is the image?**
Roughly 8–16 MB — a single static Rust binary on a distroless base.

**Can it fill Japanese / CJK form fields?**
Yes. Field values are passed to the engine verbatim as UTF-8 — no transcoding
or normalisation — so CJK text is handled correctly.

**Does it do OCR / rendering?**
Not in the default slim image (those pull heavy native dependencies). v0.1
focuses on extraction, forms, and document ops.

**How do I process password-protected PDFs?**
v0.1 returns `PDF_ENCRYPTED` for encrypted documents; password support is on
the roadmap.

**How does it scale?**
It's stateless — run multiple replicas behind any load balancer. Concurrency is
bounded per instance by `PDF_OXIDE_API_MAX_INFLIGHT`.

**What's the difference between this and the `pdf_oxide` library?**
`pdf_oxide` is the Rust engine (also available for Python, Go, JS, C#, Java).
pdf_oxide_api is a self-hostable HTTP service that exposes it over REST for
tools that integrate via HTTP.

**Which versions am I running?**
`GET /version` reports both the API version and the embedded `pdf_oxide`
engine version.
