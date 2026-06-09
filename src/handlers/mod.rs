//! HTTP handlers for the data API. Every handler:
//!  - decodes its input via [`crate::io::PdfInput`] (multipart / JSON / raw),
//!  - runs ALL `pdf_oxide` work inside `state.cpu.run(...)` (the one rule),
//!  - returns either JSON or a binary PDF/ZIP, never persisting anything.

pub mod docs;
pub mod extract;
pub mod forms;
pub mod pipeline;

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::Value;

use crate::error::ApiError;

/// The exact pdf_oxide version this binary is built against. Surfaced by
/// `/version` and in some responses. Sourced directly from the linked engine
/// (`env!("CARGO_PKG_VERSION")` inside pdf_oxide) so it always reports the
/// resolved patch version (e.g. `0.3.61`), never a stale major-only string.
pub const PDF_OXIDE_VERSION: &str = pdf_oxide::VERSION;

/// Open a PDF from bytes, mapping any parse error to the safe `ApiError`.
pub fn open_doc(bytes: Vec<u8>) -> Result<pdf_oxide::PdfDocument, ApiError> {
    pdf_oxide::PdfDocument::from_bytes(bytes).map_err(ApiError::from_pdf)
}

/// Build a JSON `200 OK` response with `Cache-Control: no-store` (confidential
/// data must never be cached by a proxy).
pub fn json_response<T: serde::Serialize>(value: T) -> Response {
    let mut resp = (StatusCode::OK, axum::Json(value)).into_response();
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// Build an `application/pdf` response from owned bytes.
pub fn pdf_response(bytes: Vec<u8>) -> Response {
    binary_response(bytes, "application/pdf")
}

/// Build an `application/zip` response from owned bytes.
pub fn zip_response(bytes: Vec<u8>) -> Response {
    binary_response(bytes, "application/zip")
}

fn binary_response(bytes: Vec<u8>, content_type: &'static str) -> Response {
    let mut resp = (StatusCode::OK, bytes).into_response();
    let headers = resp.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    // Confidential data: never let a proxy/browser cache a result.
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

/// Read an optional boolean param (already type-coerced by the IO layer).
pub fn param_bool(params: &Value, key: &str) -> Option<bool> {
    params.get(key).and_then(Value::as_bool)
}

/// Resolve the set of 0-based page indices to operate on, enforcing the
/// `max_pages` PDF-bomb guard.
///
/// `pages` param: absent -> all pages; a number `n` -> page `n` (1-based);
/// a string like `"1-3,5"` -> those 1-based pages/ranges.
pub fn selected_pages(
    params: &Value,
    page_count: usize,
    max_pages: usize,
) -> Result<Vec<usize>, ApiError> {
    if page_count > max_pages {
        return Err(ApiError::LimitExceeded {
            details: Some(format!(
                "document has {page_count} pages; limit is {max_pages}"
            )),
        });
    }
    match params.get("pages") {
        None | Some(Value::Null) => Ok((0..page_count).collect()),
        Some(Value::Number(n)) => {
            let one_based = n.as_u64().ok_or_else(bad_pages)? as usize;
            let idx = one_based.checked_sub(1).ok_or_else(bad_pages)?;
            check_in_range(idx, page_count)?;
            Ok(vec![idx])
        }
        Some(Value::String(spec)) => parse_page_spec(spec, page_count),
        Some(_) => Err(bad_pages()),
    }
}

fn parse_page_spec(spec: &str, page_count: usize) -> Result<Vec<usize>, ApiError> {
    let mut out = Vec::new();
    for part in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((a, b)) = part.split_once('-') {
            let start: usize = a.trim().parse().map_err(|_| bad_pages())?;
            let end: usize = b.trim().parse().map_err(|_| bad_pages())?;
            if start == 0 || end == 0 || end < start {
                return Err(bad_pages());
            }
            for one_based in start..=end {
                let idx = one_based - 1;
                check_in_range(idx, page_count)?;
                out.push(idx);
            }
        } else {
            let one_based: usize = part.parse().map_err(|_| bad_pages())?;
            let idx = one_based.checked_sub(1).ok_or_else(bad_pages)?;
            check_in_range(idx, page_count)?;
            out.push(idx);
        }
    }
    if out.is_empty() {
        return Err(bad_pages());
    }
    Ok(out)
}

fn check_in_range(idx: usize, page_count: usize) -> Result<(), ApiError> {
    if idx >= page_count {
        return Err(ApiError::BadRequest(format!(
            "page out of range (document has {page_count} pages)"
        )));
    }
    Ok(())
}

fn bad_pages() -> ApiError {
    ApiError::BadRequest("invalid 'pages' selection".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pages_default_all() {
        let p = json!({});
        assert_eq!(selected_pages(&p, 3, 100).unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn pages_single_number_is_one_based() {
        let p = json!({ "pages": 2 });
        assert_eq!(selected_pages(&p, 3, 100).unwrap(), vec![1]);
    }

    #[test]
    fn pages_range_and_list() {
        let p = json!({ "pages": "1-2,4" });
        assert_eq!(selected_pages(&p, 5, 100).unwrap(), vec![0, 1, 3]);
    }

    #[test]
    fn pages_out_of_range_rejected() {
        let p = json!({ "pages": "9" });
        assert!(selected_pages(&p, 3, 100).is_err());
    }

    #[test]
    fn too_many_pages_is_limit_exceeded() {
        let p = json!({});
        let err = selected_pages(&p, 5000, 2000).unwrap_err();
        assert!(matches!(err, ApiError::LimitExceeded { .. }));
    }
}
