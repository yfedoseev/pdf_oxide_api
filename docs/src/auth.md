# Authentication

Auth is **optional and off by default** for loopback use, but **required** the
moment you bind a non-loopback address.

## Enable it

Set `PDF_OXIDE_API_KEY`:

```bash
docker run -p 8080:8080 -e PDF_OXIDE_API_KEY=my-long-random-key \
  ghcr.io/yfedoseev/pdf_oxide:latest
```

Then every `/v1/*` request must present the bearer token:

```bash
export PDF_OXIDE_API_KEY=my-long-random-key   # the value you started the server with
curl -s -F file=@doc.pdf \
  -H "authorization: Bearer $PDF_OXIDE_API_KEY" \
  http://localhost:8080/v1/extract/text
```

- Missing/incorrect token → `401 UNAUTHORIZED` (RFC 9457 envelope).
- Operational endpoints (`/healthz`, `/readyz`, `/version`, `/metrics`) are
  always unauthenticated so orchestrators can probe them.
- The key is compared in constant time and is never logged.

## Non-loopback without a key

If `PDF_OXIDE_API_ADDR` is **not** loopback (e.g. `0.0.0.0:8080` — the norm in
a container) and no key is set, the service logs a **loud startup warning** but
still starts, so the zero-config `docker run` quick-start keeps working. For a
network-exposed deployment you should set a key.

To make this a **hard failure** instead (refuse to start), set
`PDF_OXIDE_API_REQUIRE_AUTH=true` — recommended for production so a missing key
can never silently expose confidential-document processing.

Generate a strong key:

```bash
openssl rand -hex 32
```
