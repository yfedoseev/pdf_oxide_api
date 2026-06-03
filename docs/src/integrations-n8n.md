# n8n

pdf_oxide_api is a plain HTTP service, so the built-in **HTTP Request** node is
all you need — no community node required.

## Extract text from a binary file

1. Add an **HTTP Request** node.
2. Method `POST`, URL `http://pdf-oxide-api:8080/v1/extract/text`.
3. Body Content-Type: **Form-Data Multipart**.
4. Add a parameter named `file`, type **n8n Binary File**, pointing at the
   incoming binary property (e.g. `data`).
5. The response JSON has `text`, `page_count`.

## Fill a form (Japanese works)

Use JSON mode:

- Method `POST`, URL `.../v1/forms/fill`, Content-Type **JSON**.
- Body:
  ```json
  {
    "pdf_base64": "={{ $binary.data.toString('base64') }}",
    "fields": { "full_name": "山田太郎", "city": "東京" }
  }
  ```
- Set **Response Format** to *File* so the returned `application/pdf` becomes a
  binary you can write out or email.

## Auth

If you set `PDF_OXIDE_API_KEY`, add a **Header Auth** credential with
`Authorization: Bearer <key>`.

## Tip

Import `/openapi.json` into n8n's HTTP node generators or any OpenAPI tooling to
scaffold requests for every endpoint.
