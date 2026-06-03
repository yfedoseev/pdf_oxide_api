# Dify

Dify can call pdf_oxide_api as a **Custom Tool** via its OpenAPI schema.

## Add the tool

1. In Dify, go to **Tools → Custom → Create Custom Tool**.
2. Paste the contents of `http://your-host:8080/openapi.json` (or the
   repo's `openapi.yaml`) as the schema.
3. If you set `PDF_OXIDE_API_KEY`, configure **API Key / Bearer** auth with your
   key.
4. The endpoints become callable tools in your workflows/agents.

## JSON-first

Dify works best with JSON, which every endpoint supports natively:

```json
POST /v1/extract/markdown
{ "pdf_base64": "<base64 of the file>", "detect_headings": true }
```

The Markdown output (with heading detection) is ideal for feeding RAG /
chunking steps directly inside a Dify pipeline.

## One-shot pipelines

Chain operations server-side to keep your Dify graph simple:

```json
POST /v1/pipeline
{ "pdf_base64": "<...>",
  "operations": [ { "op": "fill", "fields": { "name": "..." } },
                  { "op": "extract_text" } ] }
```
