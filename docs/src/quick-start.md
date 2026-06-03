# Quick start

## Run

```bash
docker run --rm -p 8080:8080 ghcr.io/yfedoseev/pdf_oxide:latest
```

Check it's up:

```bash
curl -s http://localhost:8080/healthz      # {"status":"ok"}
curl -s http://localhost:8080/version      # API + engine versions
```

## Every endpoint accepts three input encodings

1. **multipart/form-data** (great for `curl`, n8n binary):
   ```bash
   curl -s -F file=@doc.pdf http://localhost:8080/v1/extract/text
   ```
2. **application/json with base64** (great for Dify / JSON-only tools):
   ```bash
   curl -s -X POST http://localhost:8080/v1/extract/text \
     -H 'content-type: application/json' \
     -d "{\"pdf_base64\":\"$(base64 -w0 doc.pdf)\"}"
   ```
3. **raw body**:
   ```bash
   curl -s --data-binary @doc.pdf \
     -H 'content-type: application/pdf' \
     http://localhost:8080/v1/extract/text
   ```

## Examples

Extract Markdown (heading detection on by default):

```bash
curl -s -F file=@paper.pdf http://localhost:8080/v1/extract/markdown
```

Fill a form (Japanese works verbatim) and save the result:

```bash
curl -s -X POST http://localhost:8080/v1/forms/fill \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(base64 -w0 form.pdf)\",
       \"fields\":{\"full_name\":\"山田太郎\",\"city\":\"東京\"}}" \
  -o filled.pdf
```

List form fields:

```bash
curl -s -F file=@form.pdf http://localhost:8080/v1/forms/fields
```

Merge and split:

```bash
curl -s -F file=@a.pdf -F file=@b.pdf http://localhost:8080/v1/docs/merge -o merged.pdf
curl -s -F file=@doc.pdf http://localhost:8080/v1/docs/split -o pages.zip
```

Chain operations in one request:

```bash
curl -s -X POST http://localhost:8080/v1/pipeline \
  -H 'content-type: application/json' \
  -d "{\"pdf_base64\":\"$(base64 -w0 form.pdf)\",
       \"operations\":[{\"op\":\"fill\",\"fields\":{\"full_name\":\"山田太郎\"}},
                       {\"op\":\"extract_text\"}]}"
```

Browse the interactive reference at `http://localhost:8080/docs`, or fetch the
machine contract at `http://localhost:8080/openapi.json`.
