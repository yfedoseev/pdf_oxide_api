# Error model

Errors use a single, stable, machine-branchable envelope
(`application/problem+json`, RFC 9457-compatible):

```json
{
  "error": {
    "code": "MALFORMED_PDF",
    "message": "the file is not a valid or parseable PDF",
    "details": "parse failure near byte offset 4242",
    "request_id": null
  }
}
```

- `code` — a stable `SCREAMING_SNAKE` string; branch on this, not the message.
- `message` — a human-readable, **content-free** summary.
- `details` — optional, non-sensitive structured hint (e.g. a byte offset).
- `request_id` — correlation id when present.

The `message`/`details` never contain PDF content, field values, filenames, or
raw engine error text.

## Codes

| HTTP | `code` | Meaning |
|---|---|---|
| 400 | `INVALID_REQUEST` | Malformed request, missing PDF, bad parameters. |
| 401 | `UNAUTHORIZED` | Auth enabled and the bearer token is missing/wrong. |
| 413 | `PAYLOAD_TOO_LARGE` | Body exceeded `MAX_BODY_BYTES`. |
| 422 | `PDF_ENCRYPTED` | The PDF is password-protected (not supported in v0.1). |
| 422 | `MALFORMED_PDF` | Not a parseable PDF. |
| 422 | `UNSUPPORTED_PDF_FEATURE` | Parseable but uses an unsupported feature. |
| 422 | `PDF_PROCESSING_FAILED` | Parsed, but the operation could not complete. |
| 422 | `LIMIT_EXCEEDED` | Too many pages / pipeline steps. |
| 500 | `INTERNAL_ERROR` | Unexpected failure (no internals leaked). |
| 503 | `NOT_READY` | Service is draining / shutting down. |
