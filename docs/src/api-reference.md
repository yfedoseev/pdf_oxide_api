# API reference

The full machine-readable contract is **OpenAPI 3.1**, served by the running
service and version-controlled in the repo:

- Interactive viewer: `GET /docs`
- JSON: `GET /openapi.json` — import this into n8n / Dify / Postman
- YAML: `GET /openapi.yaml`
- In the repo: [`openapi.yaml`](https://github.com/yfedoseev/pdf_oxide_api/blob/main/openapi.yaml)

## Conventions

- All data endpoints are `POST` under `/v1` (stateless — there is no resource
  URL to `GET`).
- Input is one of: `multipart/form-data` (`file` parts), `application/json`
  (`pdf_base64` / `pdfs_base64`), or a raw body. See [Quick start](quick-start.md).
- PDF-producing ops return `application/pdf`; `split` returns
  `application/zip`; data ops return JSON. All results are `Cache-Control:
  no-store`.
- Errors use the [error envelope](errors.md).

## Endpoints

| Method | Path | Returns |
|---|---|---|
| POST | `/v1/extract/text` | `{ text, page_count, pages_extracted }` |
| POST | `/v1/extract/markdown` | `{ markdown, page_count, ... }` |
| POST | `/v1/extract/html` | `{ html, page_count, ... }` |
| POST | `/v1/forms/fields` | `{ field_count, fields:[{name,type,value,read_only}] }` |
| POST | `/v1/forms/fill` | `application/pdf` (filled) |
| POST | `/v1/docs/merge` | `application/pdf` (merged) |
| POST | `/v1/docs/split` | `application/zip` (one PDF/page) |
| POST | `/v1/docs/metadata` | `{ title, author, subject, keywords, producer, creation_date }` |
| POST | `/v1/docs/page-info` | `{ page_count, pages:[{index,width,height,rotation}] }` |
| POST | `/v1/pipeline` | PDF or JSON (depends on the last op) |
| GET | `/healthz` `/readyz` `/version` `/metrics` | operational |

## Common parameters

- `pages` — a 1-based selection: a number (`3`), a list/range string
  (`"1-3,5"`), or omitted for all pages. (extract\*, split)
- `detect_headings` — bool, default `true`. (markdown/html)
- `flatten` — bool, default `false`. (forms/fill)
