//! Form endpoints: `POST /v1/forms/{fields,fill}`.
//!
//! `fill` is the project's hero feature (issue #611): AcroForm field values are
//! passed to `pdf_oxide` VERBATIM as UTF-8 — no transcoding, no normalisation —
//! so Japanese/CJK text round-trips without mojibake.

use axum::{extract::State, response::Response};
use pdf_oxide::editor::form_fields::FormFieldValue;
use pdf_oxide::editor::DocumentEditor;
use pdf_oxide::extractors::forms::{FieldType, FieldValue, FormExtractor};
use pdf_oxide::PdfDocument;
use serde_json::{json, Value};

use super::{json_response, open_doc, param_bool, pdf_response};
use crate::{error::ApiError, io::PdfInput, AppState};

/// PDF field flag: ReadOnly is bit position 1 (value `1`), per ISO 32000 §12.7.
const FLAG_READ_ONLY: u32 = 1;

fn field_type_str(t: &FieldType) -> &'static str {
    match t {
        FieldType::Button => "button",
        FieldType::Text => "text",
        FieldType::Choice => "choice",
        FieldType::Signature => "signature",
        FieldType::Unknown(_) => "unknown",
    }
}

fn field_value_json(v: &FieldValue) -> Value {
    match v {
        FieldValue::Text(s) => Value::String(s.clone()),
        FieldValue::Boolean(b) => Value::Bool(*b),
        FieldValue::Name(s) => Value::String(s.clone()),
        FieldValue::Array(items) => {
            Value::Array(items.iter().map(|s| Value::String(s.clone())).collect())
        }
        FieldValue::None => Value::Null,
    }
}

/// Introspect the AcroForm: list every field with its type, current value, and
/// read-only flag.
pub fn fields_value(doc: &PdfDocument) -> Result<Value, ApiError> {
    let fields = FormExtractor::extract_fields(doc).map_err(ApiError::from_pdf)?;
    let list: Vec<Value> = fields
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "full_name": f.full_name,
                "type": field_type_str(&f.field_type),
                "value": field_value_json(&f.value),
                "read_only": f.flags.map(|fl| fl & FLAG_READ_ONLY != 0).unwrap_or(false),
            })
        })
        .collect();
    Ok(json!({ "field_count": list.len(), "fields": list }))
}

/// Convert a JSON field value into a `pdf_oxide` `FormFieldValue`.
fn to_form_value(v: &Value) -> Result<FormFieldValue, ApiError> {
    match v {
        Value::String(s) => Ok(FormFieldValue::Text(s.clone())),
        Value::Bool(b) => Ok(FormFieldValue::Boolean(*b)),
        Value::Number(n) => Ok(FormFieldValue::Text(n.to_string())),
        Value::Array(items) => {
            let mut choices = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    Value::String(s) => choices.push(s.clone()),
                    _ => {
                        return Err(ApiError::BadRequest(
                            "array field values must be strings".into(),
                        ))
                    }
                }
            }
            Ok(FormFieldValue::MultiChoice(choices))
        }
        Value::Null => Ok(FormFieldValue::None),
        _ => Err(ApiError::BadRequest(
            "field values must be string, number, boolean, array, or null".into(),
        )),
    }
}

/// Fill the given fields and return the resulting PDF bytes. Reused by the
/// pipeline endpoint. `fields` must be a JSON object `{ name: value }`.
pub fn fill_bytes(bytes: Vec<u8>, fields: &Value, flatten: bool) -> Result<Vec<u8>, ApiError> {
    let map = fields
        .as_object()
        .ok_or_else(|| ApiError::BadRequest("'fields' must be a JSON object".into()))?;
    let mut editor = DocumentEditor::from_bytes(bytes).map_err(ApiError::from_pdf)?;
    for (name, value) in map {
        // Field NAMES are user-supplied structure (safe to act on); field
        // VALUES are passed through verbatim and never logged.
        let fv = to_form_value(value)?;
        editor
            .set_form_field_value(name, fv)
            .map_err(ApiError::from_pdf)?;
    }
    if flatten {
        editor.flatten_forms().map_err(ApiError::from_pdf)?;
    }
    editor.save_to_bytes().map_err(ApiError::from_pdf)
}

pub async fn fields(State(state): State<AppState>, input: PdfInput) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let value = state
        .cpu
        .run(move || {
            let doc = open_doc(bytes)?;
            fields_value(&doc)
        })
        .await?;
    Ok(json_response(value))
}

pub async fn fill(State(state): State<AppState>, input: PdfInput) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let fields = input
        .fields
        .clone()
        .ok_or_else(|| ApiError::BadRequest("missing 'fields' object".into()))?;
    let flatten = param_bool(&input.params, "flatten").unwrap_or(false);
    let out = state
        .cpu
        .run(move || fill_bytes(bytes, &fields, flatten))
        .await?;
    Ok(pdf_response(out))
}
