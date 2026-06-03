//! Extraction endpoints: `POST /v1/extract/{text,markdown,html}`.
//!
//! Core logic lives in the `*_value` functions (operating on an open
//! `PdfDocument` and returning JSON) so the pipeline endpoint can reuse them
//! inside a single in-memory parse.

use axum::{extract::State, response::Response};
use pdf_oxide::{converters::ConversionOptions, PdfDocument};
use serde_json::{json, Value};

use super::{json_response, open_doc, param_bool, selected_pages, PDF_OXIDE_VERSION};
use crate::{error::ApiError, io::PdfInput, AppState};

fn conversion_options(params: &Value) -> ConversionOptions {
    let mut opts = ConversionOptions::default(); // detect_headings = true
    if let Some(b) = param_bool(params, "detect_headings") {
        opts.detect_headings = b;
    }
    opts
}

/// Whether the caller asked for a specific page subset.
fn has_page_selection(params: &Value) -> bool {
    !matches!(params.get("pages"), None | Some(Value::Null))
}

pub fn text_value(doc: &PdfDocument, params: &Value, max_pages: usize) -> Result<Value, ApiError> {
    let page_count = doc.page_count().map_err(ApiError::from_pdf)?;
    let pages = selected_pages(params, page_count, max_pages)?;
    let mut text = String::new();
    for (i, &p) in pages.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        text.push_str(&doc.extract_text(p).map_err(ApiError::from_pdf)?);
    }
    Ok(json!({
        "text": text,
        "page_count": page_count,
        "pages_extracted": pages.len(),
        "pdf_oxide_version": PDF_OXIDE_VERSION,
    }))
}

pub fn markdown_value(
    doc: &PdfDocument,
    params: &Value,
    max_pages: usize,
) -> Result<Value, ApiError> {
    let page_count = doc.page_count().map_err(ApiError::from_pdf)?;
    let pages = selected_pages(params, page_count, max_pages)?;
    let opts = conversion_options(params);
    let markdown = if has_page_selection(params) {
        let mut parts = Vec::with_capacity(pages.len());
        for &p in &pages {
            parts.push(doc.to_markdown(p, &opts).map_err(ApiError::from_pdf)?);
        }
        parts.join("\n\n---\n\n")
    } else {
        doc.to_markdown_all(&opts).map_err(ApiError::from_pdf)?
    };
    Ok(json!({
        "markdown": markdown,
        "page_count": page_count,
        "pages_extracted": pages.len(),
        "pdf_oxide_version": PDF_OXIDE_VERSION,
    }))
}

pub fn html_value(doc: &PdfDocument, params: &Value, max_pages: usize) -> Result<Value, ApiError> {
    let page_count = doc.page_count().map_err(ApiError::from_pdf)?;
    let pages = selected_pages(params, page_count, max_pages)?;
    let opts = conversion_options(params);
    let html = if has_page_selection(params) {
        let mut parts = Vec::with_capacity(pages.len());
        for &p in &pages {
            parts.push(doc.to_html(p, &opts).map_err(ApiError::from_pdf)?);
        }
        parts.join("\n")
    } else {
        doc.to_html_all(&opts).map_err(ApiError::from_pdf)?
    };
    Ok(json!({
        "html": html,
        "page_count": page_count,
        "pages_extracted": pages.len(),
        "pdf_oxide_version": PDF_OXIDE_VERSION,
    }))
}

macro_rules! extract_handler {
    ($name:ident, $core:ident) => {
        pub async fn $name(
            State(state): State<AppState>,
            input: PdfInput,
        ) -> Result<Response, ApiError> {
            let bytes = input.single()?.to_vec();
            let params = input.params.clone();
            let max_pages = state.cfg.max_pages;
            let value = state
                .cpu
                .run(move || {
                    let doc = open_doc(bytes)?;
                    $core(&doc, &params, max_pages)
                })
                .await?;
            Ok(json_response(value))
        }
    };
}

extract_handler!(text, text_value);
extract_handler!(markdown, markdown_value);
extract_handler!(html, html_value);
