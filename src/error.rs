//! The single typed application error. Implements `IntoResponse`, mapping each
//! variant to an HTTP status + an RFC 9457-compatible `application/problem+json`
//! body nested under `error` (the stable, machine-branchable envelope):
//!
//! ```json
//! { "error": { "code": "MALFORMED_PDF", "message": "...", "details": null,
//!              "request_id": null } }
//! ```
//!
//! Privacy rule (LOAD-BEARING): error messages are curated constants — they
//! NEVER echo raw PDF contents, field values, filenames, internal paths, or a
//! `pdf_oxide::Error` `Display` string (which can embed document fragments).
//! `details` may carry only non-sensitive structured hints (e.g. a byte
//! offset, which is a position, not content).

use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    /// Malformed request / multipart / parameters (400).
    #[error("the request was malformed")]
    BadRequest(String),
    /// Missing/invalid credentials when auth is enabled (401).
    #[error("authentication required")]
    Unauthorized,
    /// Body exceeded the configured cap (413).
    #[error("the uploaded payload is too large")]
    PayloadTooLarge,
    /// The PDF is encrypted / password-protected; v0.1 does not accept
    /// passwords, so it cannot be processed (422).
    #[error("the PDF is encrypted and cannot be processed")]
    Encrypted,
    /// The bytes are not a parseable PDF (bad header, broken xref, truncated,
    /// corrupt objects) (422).
    #[error("the file is not a valid or parseable PDF")]
    MalformedPdf { details: Option<String> },
    /// Well-formed PDF, but it uses a feature this version does not support
    /// (e.g. an unsupported filter or PDF version) (422).
    #[error("the PDF uses an unsupported feature")]
    UnsupportedPdf,
    /// The PDF parsed, but the requested operation could not be completed
    /// (e.g. font/layout/encode failure during conversion) (422).
    #[error("the PDF could not be processed for this operation")]
    ProcessingFailed,
    /// A request limit was exceeded (page count, pipeline steps) (422).
    #[error("a request limit was exceeded")]
    LimitExceeded { details: Option<String> },
    /// A panic was caught inside the CPU worker (500).
    #[error("PDF processing failed unexpectedly")]
    PdfPanic,
    /// Service is draining / pool closed (503).
    #[error("the service is shutting down")]
    ShuttingDown,
    /// Unexpected internal error (500); never leaks internals.
    #[error("internal error")]
    Internal,
}

impl ApiError {
    /// Centralised, variant-aware mapping of `pdf_oxide::Error` -> `ApiError`.
    ///
    /// The match is exhaustive-by-category so a new upstream variant degrades
    /// to the safe `Internal` default rather than leaking. We never embed the
    /// error's `Display` text; for `ParseError` we surface only the byte
    /// offset (a position, not document content) as a hint.
    pub fn from_pdf(err: pdf_oxide::Error) -> Self {
        use pdf_oxide::Error as E;
        match err {
            // Encrypted / password-protected.
            E::EncryptedPdf => ApiError::Encrypted,

            // Not-a-PDF / structurally broken / truncated / corrupt graph.
            E::InvalidHeader(_)
            | E::InvalidPdf(_)
            | E::InvalidXref
            | E::UnexpectedEof
            | E::ObjectNotFound(_, _)
            | E::InvalidObjectType { .. }
            | E::Decode(_)
            | E::Utf8Error(_)
            | E::CircularReference(_)
            | E::RecursionLimitExceeded(_) => ApiError::MalformedPdf { details: None },

            // Parse error carries a byte offset — safe to surface as a hint.
            E::ParseError { offset, .. } | E::ParseWarning { offset, .. } => {
                ApiError::MalformedPdf {
                    details: Some(format!("parse failure near byte offset {offset}")),
                }
            }

            // Parseable, but uses something we don't support.
            E::UnsupportedVersion(_) | E::Unsupported(_) | E::UnsupportedFilter(_) => {
                ApiError::UnsupportedPdf
            }

            // Parsed OK, operation failed (content-level).
            E::Font(_)
            | E::Image(_)
            | E::Encode(_)
            | E::LayoutAnalysis(_)
            | E::Barcode(_)
            | E::InvalidOperation(_) => ApiError::ProcessingFailed,

            // I/O on in-memory data, ML/OCR (unused in the slim image), or any
            // future variant: treat as internal — never leak the Display.
            _ => ApiError::Internal,
        }
    }

    /// (status, stable machine code, optional safe details).
    fn parts(&self) -> (StatusCode, &'static str, Option<String>) {
        match self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", None),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", None),
            ApiError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE", None),
            ApiError::Encrypted => (StatusCode::UNPROCESSABLE_ENTITY, "PDF_ENCRYPTED", None),
            ApiError::MalformedPdf { details } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "MALFORMED_PDF",
                details.clone(),
            ),
            ApiError::UnsupportedPdf => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "UNSUPPORTED_PDF_FEATURE",
                None,
            ),
            ApiError::ProcessingFailed => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "PDF_PROCESSING_FAILED",
                None,
            ),
            ApiError::LimitExceeded { details } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "LIMIT_EXCEEDED",
                details.clone(),
            ),
            ApiError::ShuttingDown => (StatusCode::SERVICE_UNAVAILABLE, "NOT_READY", None),
            ApiError::PdfPanic | ApiError::Internal => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", None)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, details) = self.parts();
        let body = json!({
            "error": {
                "code": code,
                "message": self.to_string(),
                "details": details,
                // Populated by the request-id response layer (see middleware).
                "request_id": serde_json::Value::Null,
            }
        });
        let mut resp = (status, Json(body)).into_response();
        resp.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn envelope(err: ApiError) -> (StatusCode, serde_json::Value) {
        let resp = err.into_response();
        let status = resp.status();
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert_eq!(ct, "application/problem+json");
        let bytes = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        // Every envelope has the four stable keys nested under `error`.
        let e = &v["error"];
        assert!(e["code"].is_string());
        assert!(e["message"].is_string());
        assert!(e.get("details").is_some());
        assert!(e.get("request_id").is_some());
        (status, v)
    }

    #[tokio::test]
    async fn encrypted_maps_to_422_pdf_encrypted() {
        let (status, v) = envelope(ApiError::Encrypted).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(v["error"]["code"], "PDF_ENCRYPTED");
    }

    #[tokio::test]
    async fn malformed_surfaces_offset_but_no_content() {
        let (status, v) = envelope(ApiError::MalformedPdf {
            details: Some("parse failure near byte offset 4242".into()),
        })
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(v["error"]["code"], "MALFORMED_PDF");
        assert_eq!(v["error"]["details"], "parse failure near byte offset 4242");
    }

    #[tokio::test]
    async fn payload_too_large_is_413() {
        let (status, v) = envelope(ApiError::PayloadTooLarge).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(v["error"]["code"], "PAYLOAD_TOO_LARGE");
    }

    /// Regression guard for the privacy rule: mapping must NOT carry the raw
    /// `pdf_oxide::Error` Display (which can contain document fragments). We
    /// build an error whose Display embeds a secret marker and assert it never
    /// appears in the serialized envelope.
    #[tokio::test]
    async fn from_pdf_never_leaks_display_text() {
        let secret = "SECRET_DOCUMENT_FRAGMENT_9f3a";
        let mapped = ApiError::from_pdf(pdf_oxide::Error::InvalidPdf(secret.into()));
        let (_status, v) = envelope(mapped).await;
        let serialized = v.to_string();
        assert!(
            !serialized.contains(secret),
            "leaked raw pdf_oxide Display into the client envelope: {serialized}"
        );
        assert_eq!(v["error"]["code"], "MALFORMED_PDF");
    }

    #[tokio::test]
    async fn from_pdf_unsupported_category() {
        let mapped = ApiError::from_pdf(pdf_oxide::Error::UnsupportedFilter("JBIG2".into()));
        let (status, v) = envelope(mapped).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(v["error"]["code"], "UNSUPPORTED_PDF_FEATURE");
    }
}
