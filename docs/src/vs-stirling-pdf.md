# pdf_oxide_api vs Stirling-PDF

Both are self-hostable PDF tools, but they target different needs.
pdf_oxide_api is a lean, automation-first **API** built on a Rust engine;
Stirling-PDF is a feature-broad **web app** (Java/Spring) with a UI.

| | pdf_oxide_api | Stirling-PDF |
|---|---|---|
| Primary interface | REST + JSON API (OpenAPI 3.1) | Web UI (+ API) |
| Engine | Rust (`pdf_oxide`) | Java + external tools |
| Image size | ~8–16 MB (static, distroless) | hundreds of MB |
| State | Stateless, nothing persisted | Stateless core, larger surface |
| Japanese / CJK form-fill | First-class, values passed verbatim as UTF-8 | Reported encoding issues (the reason this project exists — see [#611](https://github.com/yfedoseev/pdf_oxide/issues/611)) |
| Footprint / startup | Single binary, instant start | JVM warm-up |
| Scope | Focused: extract, forms, merge/split, pipeline | Very broad toolbox |

## When to choose pdf_oxide_api
- You want a small, fast, easily-scaled **API** to embed in n8n / Dify /
  automation, not a UI.
- You need reliable **Japanese / CJK** text and form handling.
- You care about a minimal, hardened, auditable container for **confidential**
  documents.

## When Stirling-PDF fits better
- You want a rich browser UI and a very large catalogue of one-off PDF tools
  for interactive use.

> Filling a form value is correct-by-construction here: the API passes your
> UTF-8 field values straight to the engine with no transcoding.
