//! Inbound request decoding, shared by every data endpoint.
//!
//! The service accepts a PDF in two encodings so it is friendly to both
//! `curl`/n8n binary uploads and JSON-only automation platforms (Dify):
//!
//! - `multipart/form-data` (default): one or more `file` parts carry the
//!   PDF(s); other text parts and the URL query carry parameters; an optional
//!   `fields` text part carries a JSON object (used by form-fill).
//! - `application/json`: `{ "pdf_base64": "...", ... }` (single) or
//!   `{ "pdfs_base64": ["...", ...], ... }` (multiple); remaining keys are
//!   parameters; an optional `fields` key carries the form-fill map.
//! - raw body (`application/pdf` / `application/octet-stream` / no type): the
//!   whole body is the single PDF; parameters come from the URL query.
//!
//! Decoding never logs PDF bytes, field values, or filenames.

use axum::{
    body::Bytes,
    extract::{rejection::BytesRejection, FromRequest, Multipart, Request},
    http::header,
};
use base64::Engine as _;
use serde_json::{Map, Value};

use crate::error::ApiError;

/// Map a `Bytes` extractor rejection, preserving the over-limit case as a 413
/// (`PayloadTooLarge`) instead of collapsing every read error into a 400. We
/// key off the rejection's own HTTP status (length-limit -> 413) rather than a
/// (non-exhaustive, version-sensitive) variant name.
fn map_bytes_rejection(rej: BytesRejection) -> ApiError {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    if rej.into_response().status() == StatusCode::PAYLOAD_TOO_LARGE {
        ApiError::PayloadTooLarge
    } else {
        ApiError::BadRequest("could not read request body".into())
    }
}

/// A normalized inbound request: the PDF payload(s), a JSON params object, and
/// an optional `fields` object (form-fill map).
pub struct PdfInput {
    pub pdfs: Vec<Vec<u8>>,
    pub params: Value,
    pub fields: Option<Value>,
}

impl PdfInput {
    /// The single primary PDF, or a 400 if none was supplied.
    pub fn single(&self) -> Result<&[u8], ApiError> {
        match self.pdfs.first() {
            Some(b) if !b.is_empty() => Ok(b),
            _ => Err(ApiError::BadRequest("no PDF supplied".into())),
        }
    }
}

impl<S: Send + Sync> FromRequest<S> for PdfInput {
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        // Query params are merged for every encoding (with light type coercion
        // so `?detect_headings=true` deserializes into a bool field).
        let mut params = query_to_params(req.uri().query());

        if content_type.starts_with("multipart/form-data") {
            decode_multipart(req, state, &mut params).await
        } else if content_type.starts_with("application/json") {
            decode_json(req, state).await
        } else {
            let bytes = Bytes::from_request(req, state)
                .await
                .map_err(map_bytes_rejection)?;
            Ok(PdfInput {
                pdfs: if bytes.is_empty() {
                    vec![]
                } else {
                    vec![bytes.to_vec()]
                },
                params: Value::Object(params),
                fields: None,
            })
        }
    }
}

async fn decode_multipart<S: Send + Sync>(
    req: Request,
    state: &S,
    params: &mut Map<String, Value>,
) -> Result<PdfInput, ApiError> {
    let mut multipart = Multipart::from_request(req, state)
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart body: {e}")))?;

    let mut pdfs = Vec::new();
    let mut fields = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("invalid multipart field: {e}")))?
    {
        match field.name().map(str::to_owned).as_deref() {
            Some("file") | Some("files") | Some("pdf") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("could not read file part: {e}")))?;
                if !bytes.is_empty() {
                    pdfs.push(bytes.to_vec());
                }
            }
            Some("fields") => {
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::BadRequest("could not read 'fields' part".into()))?;
                fields = Some(serde_json::from_str(&text).map_err(|e| {
                    ApiError::BadRequest(format!("'fields' must be a JSON object: {e}"))
                })?);
            }
            Some(name) => {
                let name = name.to_owned();
                let text = field
                    .text()
                    .await
                    .map_err(|_| ApiError::BadRequest("could not read text part".into()))?;
                params.insert(name, coerce(&text));
            }
            None => {}
        }
    }

    Ok(PdfInput {
        pdfs,
        params: Value::Object(std::mem::take(params)),
        fields,
    })
}

async fn decode_json<S: Send + Sync>(req: Request, state: &S) -> Result<PdfInput, ApiError> {
    let bytes = Bytes::from_request(req, state)
        .await
        .map_err(map_bytes_rejection)?;
    let mut obj: Map<String, Value> = serde_json::from_slice(&bytes)
        .map_err(|e| ApiError::BadRequest(format!("invalid JSON body: {e}")))?;

    let mut pdfs = Vec::new();
    if let Some(Value::String(b64)) = obj.remove("pdf_base64") {
        pdfs.push(decode_b64(&b64)?);
    }
    if let Some(Value::Array(arr)) = obj.remove("pdfs_base64") {
        for item in arr {
            if let Value::String(b64) = item {
                pdfs.push(decode_b64(&b64)?);
            } else {
                return Err(ApiError::BadRequest(
                    "pdfs_base64 must be an array of base64 strings".into(),
                ));
            }
        }
    }
    let fields = obj.remove("fields");

    Ok(PdfInput {
        pdfs,
        params: Value::Object(obj),
        fields,
    })
}

fn decode_b64(s: &str) -> Result<Vec<u8>, ApiError> {
    base64::engine::general_purpose::STANDARD
        .decode(s.trim())
        .map_err(|_| ApiError::BadRequest("invalid base64 PDF data".into()))
}

/// Parse a URL query string into a JSON object with light type coercion.
fn query_to_params(query: Option<&str>) -> Map<String, Value> {
    let mut map = Map::new();
    if let Some(q) = query {
        for (k, v) in form_urlencoded_pairs(q) {
            map.insert(k, coerce(&v));
        }
    }
    map
}

/// Minimal `application/x-www-form-urlencoded` pair parser (keys/values are
/// percent-decoded; `+` becomes space). Avoids pulling in an extra crate.
fn form_urlencoded_pairs(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|s| !s.is_empty())
        .map(|pair| {
            let mut it = pair.splitn(2, '=');
            let k = percent_decode(it.next().unwrap_or(""));
            let v = percent_decode(it.next().unwrap_or(""));
            (k, v)
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let s = s.replace('+', " ");
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Coerce a textual value into the most specific JSON type so typed param
/// structs deserialize cleanly: `"true"`/`"false"` -> bool, integers -> number,
/// everything else -> string.
fn coerce(s: &str) -> Value {
    match s {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    if let Ok(n) = s.parse::<i64>() {
        return Value::Number(n.into());
    }
    Value::String(s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerce_types() {
        assert_eq!(coerce("true"), Value::Bool(true));
        assert_eq!(coerce("42"), Value::Number(42.into()));
        assert_eq!(coerce("hello"), Value::String("hello".into()));
    }

    #[test]
    fn query_parsing_and_coercion() {
        let m = query_to_params(Some("detect_headings=true&pages=3&q=a%20b"));
        assert_eq!(m["detect_headings"], Value::Bool(true));
        assert_eq!(m["pages"], Value::Number(3.into()));
        assert_eq!(m["q"], Value::String("a b".into()));
    }
}
