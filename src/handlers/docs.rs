//! Document-operation endpoints: `POST /v1/docs/{merge,split,metadata,page-info}`.

use std::io::{Cursor, Write as _};

use axum::{extract::State, response::Response};
use pdf_oxide::editor::DocumentEditor;
use pdf_oxide::PdfDocument;
use serde_json::{json, Value};

use super::{json_response, open_doc, pdf_response, selected_pages, zip_response};
use crate::{error::ApiError, io::PdfInput, AppState};

/// Merge two or more PDFs (in upload order) into one document.
pub fn merge_bytes(pdfs: Vec<Vec<u8>>) -> Result<Vec<u8>, ApiError> {
    if pdfs.len() < 2 {
        return Err(ApiError::BadRequest(
            "merge requires at least two PDF files".into(),
        ));
    }
    let mut iter = pdfs.into_iter();
    let first = iter.next().unwrap();
    let mut editor = DocumentEditor::from_bytes(first).map_err(ApiError::from_pdf)?;
    for next in iter {
        editor.merge_from_bytes(&next).map_err(ApiError::from_pdf)?;
    }
    editor.save_to_bytes().map_err(ApiError::from_pdf)
}

/// Split into one PDF per selected page, packaged as a ZIP archive. Each page
/// is extracted from a fresh editor over the original bytes so the operation
/// is order-independent and non-destructive.
pub fn split_zip(bytes: Vec<u8>, pages: Vec<usize>) -> Result<Vec<u8>, ApiError> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for &page in &pages {
        let mut editor = DocumentEditor::from_bytes(bytes.clone()).map_err(ApiError::from_pdf)?;
        let page_pdf = editor
            .extract_pages_to_bytes(&[page])
            .map_err(ApiError::from_pdf)?;
        let name = format!("page-{:04}.pdf", page + 1);
        zip.start_file(name, opts).map_err(|_| ApiError::Internal)?;
        zip.write_all(&page_pdf).map_err(|_| ApiError::Internal)?;
    }
    let cursor = zip.finish().map_err(|_| ApiError::Internal)?;
    Ok(cursor.into_inner())
}

/// Read document metadata into a JSON object (missing entries are `null`).
pub fn metadata_value(bytes: Vec<u8>) -> Result<Value, ApiError> {
    let mut editor = DocumentEditor::from_bytes(bytes).map_err(ApiError::from_pdf)?;
    Ok(json!({
        "title": editor.title().map_err(ApiError::from_pdf)?,
        "author": editor.author().map_err(ApiError::from_pdf)?,
        "subject": editor.subject().map_err(ApiError::from_pdf)?,
        "keywords": editor.keywords().map_err(ApiError::from_pdf)?,
        "producer": editor.producer().map_err(ApiError::from_pdf)?,
        "creation_date": editor.creation_date().map_err(ApiError::from_pdf)?,
    }))
}

/// Per-page geometry: size (PDF points) and rotation.
pub fn page_info_value(doc: &PdfDocument, max_pages: usize) -> Result<Value, ApiError> {
    let page_count = doc.page_count().map_err(ApiError::from_pdf)?;
    if page_count > max_pages {
        return Err(ApiError::LimitExceeded {
            details: Some(format!(
                "document has {page_count} pages; limit is {max_pages}"
            )),
        });
    }
    let mut pages = Vec::with_capacity(page_count);
    for i in 0..page_count {
        let (x0, y0, x1, y1) = doc.get_page_media_box(i).map_err(ApiError::from_pdf)?;
        let rotation = doc.get_page_rotation(i).map_err(ApiError::from_pdf)?;
        pages.push(json!({
            "index": i,
            "width": (x1 - x0).abs(),
            "height": (y1 - y0).abs(),
            "rotation": rotation,
        }));
    }
    Ok(json!({ "page_count": page_count, "pages": pages }))
}

pub async fn merge(State(state): State<AppState>, input: PdfInput) -> Result<Response, ApiError> {
    let pdfs = input.pdfs.clone();
    let out = state.cpu.run(move || merge_bytes(pdfs)).await?;
    Ok(pdf_response(out))
}

pub async fn split(State(state): State<AppState>, input: PdfInput) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let params = input.params.clone();
    let max_pages = state.cfg.max_pages;
    let out = state
        .cpu
        .run(move || {
            let doc = open_doc(bytes.clone())?;
            let page_count = doc.page_count().map_err(ApiError::from_pdf)?;
            let pages = selected_pages(&params, page_count, max_pages)?;
            split_zip(bytes, pages)
        })
        .await?;
    Ok(zip_response(out))
}

pub async fn metadata(
    State(state): State<AppState>,
    input: PdfInput,
) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let value = state.cpu.run(move || metadata_value(bytes)).await?;
    Ok(json_response(value))
}

pub async fn page_info(
    State(state): State<AppState>,
    input: PdfInput,
) -> Result<Response, ApiError> {
    let bytes = input.single()?.to_vec();
    let max_pages = state.cfg.max_pages;
    let value = state
        .cpu
        .run(move || {
            let doc = open_doc(bytes)?;
            page_info_value(&doc, max_pages)
        })
        .await?;
    Ok(json_response(value))
}
