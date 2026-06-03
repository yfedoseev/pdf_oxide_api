//! Optional bearer-token auth for the `/v1/*` data API.
//!
//! When `PDF_OXIDE_API_KEY` is set, every `/v1` request must carry
//! `Authorization: Bearer <key>`. Operational endpoints (`/healthz`, `/readyz`,
//! `/version`, `/metrics`) are always unauthenticated. The key is never logged.
//! A non-loopback bind with no key is refused at startup (fail closed) — see
//! `main::main`.

use axum::extract::Request;
use axum::{extract::State, http::header::AUTHORIZATION, middleware::Next, response::Response};

use crate::{error::ApiError, AppState};

pub async fn require_api_key(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if let Some(expected) = state.cfg.api_key.as_deref() {
        let presented = req
            .headers()
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "));
        let ok = presented
            .map(|tok| constant_time_eq(tok.as_bytes(), expected.as_bytes()))
            .unwrap_or(false);
        if !ok {
            return Err(ApiError::Unauthorized);
        }
    }
    Ok(next.run(req).await)
}

/// Length-then-content compare with no early-out on a content mismatch, so a
/// matching prefix does not leak via timing. (Length is not secret here.)
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn ct_eq_basic() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreu"));
        assert!(!constant_time_eq(b"secret", b"secre"));
    }
}
