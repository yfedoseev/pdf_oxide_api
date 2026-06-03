//! `POST /v1/pipeline` — chain operations over ONE in-memory PDF.
//!
//! The PDF is parsed/edited inside a single `state.cpu.run(...)` closure and
//! threaded op-to-op, preserving statelessness (nothing is persisted between
//! requests). Transform ops (`fill`) produce a new PDF; a data op
//! (`extract_*`, `metadata`, `page_info`) ends the pipeline and must be last.

use axum::{extract::State, response::Response};
use serde_json::{Map, Value};

use super::{docs, extract, forms, json_response, open_doc, pdf_response};
use crate::{error::ApiError, io::PdfInput, AppState};

enum PipelineOutput {
    Pdf(Vec<u8>),
    Json(Value),
}

/// Pull the `operations` array out of the params (accepting either a JSON array
/// or a JSON-encoded string, the latter arriving via a multipart text part).
fn operations(params: &Value) -> Result<Vec<Value>, ApiError> {
    let raw = params
        .get("operations")
        .ok_or_else(|| ApiError::BadRequest("missing 'operations' array".into()))?;
    let arr = match raw {
        Value::Array(a) => a.clone(),
        Value::String(s) => serde_json::from_str::<Value>(s)
            .ok()
            .and_then(|v| v.as_array().cloned())
            .ok_or_else(|| ApiError::BadRequest("'operations' must be a JSON array".into()))?,
        _ => {
            return Err(ApiError::BadRequest(
                "'operations' must be a JSON array".into(),
            ))
        }
    };
    if arr.is_empty() {
        return Err(ApiError::BadRequest(
            "'operations' must not be empty".into(),
        ));
    }
    Ok(arr)
}

/// An op object minus its `op` discriminator, reusable as an extract params map.
fn op_params(op: &Value) -> Value {
    let mut m: Map<String, Value> = op.as_object().cloned().unwrap_or_default();
    m.remove("op");
    Value::Object(m)
}

fn run_pipeline(
    mut bytes: Vec<u8>,
    ops: Vec<Value>,
    max_pages: usize,
) -> Result<PipelineOutput, ApiError> {
    let n = ops.len();
    for (i, op) in ops.into_iter().enumerate() {
        let is_last = i + 1 == n;
        let kind = op
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::BadRequest("each operation needs an 'op' field".into()))?
            .to_owned();

        match kind.as_str() {
            // ---- transform: PDF -> PDF ----
            "fill" => {
                let fields = op
                    .get("fields")
                    .cloned()
                    .ok_or_else(|| ApiError::BadRequest("'fill' op needs 'fields'".into()))?;
                let flatten = op.get("flatten").and_then(Value::as_bool).unwrap_or(false);
                bytes = forms::fill_bytes(bytes, &fields, flatten)?;
                if is_last {
                    return Ok(PipelineOutput::Pdf(bytes));
                }
            }
            // ---- terminal: PDF -> data (must be the last step) ----
            "extract_text" | "extract_markdown" | "extract_html" | "metadata" | "page_info" => {
                if !is_last {
                    return Err(ApiError::BadRequest(
                        "a data-producing operation must be the last step".into(),
                    ));
                }
                let params = op_params(&op);
                let value = match kind.as_str() {
                    "extract_text" => extract::text_value(&open_doc(bytes)?, &params, max_pages)?,
                    "extract_markdown" => {
                        extract::markdown_value(&open_doc(bytes)?, &params, max_pages)?
                    }
                    "extract_html" => extract::html_value(&open_doc(bytes)?, &params, max_pages)?,
                    "metadata" => docs::metadata_value(bytes)?,
                    "page_info" => docs::page_info_value(&open_doc(bytes)?, max_pages)?,
                    _ => unreachable!(),
                };
                return Ok(PipelineOutput::Json(value));
            }
            other => {
                // The op name is caller-supplied, not document content -> safe to echo.
                return Err(ApiError::BadRequest(format!(
                    "unknown pipeline op: {other}"
                )));
            }
        }
    }
    // Reachable only if the final op was a transform, which already returned.
    Err(ApiError::Internal)
}

pub async fn pipeline(
    State(state): State<AppState>,
    input: PdfInput,
) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let ops = operations(&input.params)?;
    if ops.len() > state.cfg.max_pipeline_steps {
        return Err(ApiError::LimitExceeded {
            details: Some(format!(
                "pipeline has {} steps; limit is {}",
                ops.len(),
                state.cfg.max_pipeline_steps
            )),
        });
    }
    let max_pages = state.cfg.max_pages;
    let out = state
        .cpu
        .run(move || run_pipeline(bytes, ops, max_pages))
        .await?;
    Ok(match out {
        PipelineOutput::Pdf(b) => pdf_response(b),
        PipelineOutput::Json(v) => json_response(v),
    })
}
