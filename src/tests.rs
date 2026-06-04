//! Integration tests for the HTTP surface. These run in-process against the
//! real `pdf_oxide` engine (no mocking) over the committed fixtures, so a
//! `pdf_oxide` bump that changes behaviour forces a human diff review.
//!
//! Coverage is deliberately broad — every endpoint, the error envelope, the
//! limits, and the issue-#611 Japanese/CJK form-fill round-trip — so the suite
//! is a regression net for v0.1.0.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use axum::http::{header, StatusCode};
use base64::Engine as _;
use serde_json::{json, Value};

use crate::config::Config;
use crate::{blocking::CpuPool, build_router, AppState};

const HELLO_PDF: &[u8] = include_bytes!("../tests/fixtures/hello.pdf");
const JP_FORM_PDF: &[u8] = include_bytes!("../tests/fixtures/japanese_form.pdf");

fn server() -> axum_test::TestServer {
    server_from(Config::for_tests())
}

fn server_from(cfg: Config) -> axum_test::TestServer {
    let cpu = CpuPool::new(cfg.cpu_threads, cfg.max_inflight).unwrap();
    let state = AppState {
        cpu,
        cfg: Arc::new(cfg),
        ready: Arc::new(AtomicBool::new(true)),
        metrics: None,
    };
    axum_test::TestServer::new(build_router(state)).unwrap()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A real 2-page PDF, built by merging the 1-page fixture with itself via the
/// engine — gives multi-page coverage without a second binary fixture.
fn two_page_pdf() -> Vec<u8> {
    let mut ed = pdf_oxide::editor::DocumentEditor::from_bytes(HELLO_PDF.to_vec()).unwrap();
    ed.merge_from_bytes(HELLO_PDF).unwrap();
    ed.save_to_bytes().unwrap()
}

// --------------------------------------------------------------------------
// Multi-page selection, limits, and operational output
// --------------------------------------------------------------------------

#[tokio::test]
async fn extract_text_page_selection_on_multipage() {
    let r = server()
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(&two_page_pdf()), "pages": "2" }))
        .await;
    r.assert_status_ok();
    let v: Value = r.json();
    assert_eq!(v["page_count"], 2);
    assert_eq!(v["pages_extracted"], 1);
}

#[tokio::test]
async fn split_multipage_yields_one_pdf_per_page() {
    let r = server()
        .post("/v1/docs/split")
        .json(&json!({ "pdf_base64": b64(&two_page_pdf()) }))
        .await;
    r.assert_status_ok();
    let zip_bytes = r.as_bytes().to_vec();
    let archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).unwrap();
    assert_eq!(archive.len(), 2, "expected one PDF per page");
}

#[tokio::test]
async fn page_out_of_range_is_400() {
    let r = server()
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF), "pages": "9" }))
        .await;
    r.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn payload_too_large_returns_413() {
    let mut cfg = Config::for_tests();
    cfg.max_body_bytes = 16; // tiny cap; any real body exceeds it
    let app = server_from(cfg);
    // ~hundreds of bytes of JSON, well over the 16-byte cap -> 413 from the
    // RequestBodyLimitLayer (Content-Length is known, so it short-circuits).
    let r = app
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status(StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn metrics_endpoint_serves_prometheus_text() {
    use axum::http::HeaderName;
    let r = server().get("/metrics").await;
    r.assert_status_ok();
    let ct = r
        .header(HeaderName::from_static("content-type"))
        .to_str()
        .unwrap()
        .to_string();
    assert!(ct.starts_with("text/plain"), "got {ct}");
}

// --------------------------------------------------------------------------
// OpenAPI contract, auth, and response hardening
// --------------------------------------------------------------------------

#[tokio::test]
async fn openapi_json_served_and_covers_all_routes() {
    let r = server().get("/openapi.json").await;
    r.assert_status_ok();
    let spec: Value = r.json();
    assert!(spec["openapi"].as_str().unwrap().starts_with("3.1"));
    // Drift guard: every wired data route must be documented.
    for path in [
        "/v1/extract/text",
        "/v1/extract/markdown",
        "/v1/extract/html",
        "/v1/forms/fields",
        "/v1/forms/fill",
        "/v1/docs/merge",
        "/v1/docs/split",
        "/v1/docs/metadata",
        "/v1/docs/page-info",
        "/v1/pipeline",
    ] {
        assert!(
            spec["paths"].get(path).is_some(),
            "openapi.json missing documented path {path}"
        );
    }
}

#[tokio::test]
async fn security_headers_present() {
    use axum::http::HeaderName;
    let r = server().get("/healthz").await;
    assert_eq!(
        r.header(HeaderName::from_static("x-content-type-options")),
        "nosniff"
    );
    assert_eq!(r.header(HeaderName::from_static("x-frame-options")), "DENY");
}

#[tokio::test]
async fn auth_required_when_key_set() {
    let mut cfg = Config::for_tests();
    cfg.api_key = Some("s3cr3t-key".into());
    let app = server_from(cfg);

    // No token -> 401 on /v1.
    let no_tok = app
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    no_tok.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(no_tok.json::<Value>()["error"]["code"], "UNAUTHORIZED");

    // Operational endpoints stay open even with auth enabled.
    app.get("/healthz").await.assert_status_ok();

    // Correct token -> allowed.
    let ok = app
        .post("/v1/extract/text")
        .add_header(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer s3cr3t-key"),
        )
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    ok.assert_status_ok();
}

// --------------------------------------------------------------------------
// Operational
// --------------------------------------------------------------------------

#[tokio::test]
async fn healthz_ok() {
    let r = server().get("/healthz").await;
    r.assert_status_ok();
    r.assert_json(&json!({ "status": "ok" }));
}

#[tokio::test]
async fn version_reports_engine() {
    let r = server().get("/version").await;
    r.assert_status_ok();
    let v: Value = r.json();
    assert_eq!(v["service"], "pdf_oxide_api");
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["pdf_oxide_version"].is_string());
}

// --------------------------------------------------------------------------
// Extraction
// --------------------------------------------------------------------------

#[tokio::test]
async fn extract_text_json_base64() {
    let r = server()
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status_ok();
    let v: Value = r.json();
    assert!(
        v["text"].as_str().unwrap().contains("Hello, world!"),
        "got: {v}"
    );
    assert_eq!(v["page_count"], 1);
}

#[tokio::test]
async fn extract_text_multipart() {
    use axum_test::multipart::{MultipartForm, Part};
    let form = MultipartForm::new().add_part(
        "file",
        Part::bytes(HELLO_PDF.to_vec())
            .file_name("hello.pdf")
            .mime_type("application/pdf"),
    );
    let r = server().post("/v1/extract/text").multipart(form).await;
    r.assert_status_ok();
    assert!(r.json::<Value>()["text"]
        .as_str()
        .unwrap()
        .contains("Hello"));
}

#[tokio::test]
async fn extract_text_no_pdf_is_400_problem_json() {
    let r = server().post("/v1/extract/text").json(&json!({})).await;
    r.assert_status(StatusCode::BAD_REQUEST);
    assert_eq!(r.header(header::CONTENT_TYPE), "application/problem+json");
    assert_eq!(r.json::<Value>()["error"]["code"], "INVALID_REQUEST");
}

#[tokio::test]
async fn extract_text_garbage_is_422_malformed() {
    let r = server()
        .post("/v1/extract/text")
        .json(&json!({ "pdf_base64": b64(b"not a pdf at all") }))
        .await;
    r.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.json::<Value>()["error"]["code"], "MALFORMED_PDF");
}

#[tokio::test]
async fn extract_markdown_and_html() {
    let md = server()
        .post("/v1/extract/markdown")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    md.assert_status_ok();
    assert!(md.json::<Value>()["markdown"]
        .as_str()
        .unwrap()
        .contains("Hello"));

    let html = server()
        .post("/v1/extract/html")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    html.assert_status_ok();
    assert!(html.json::<Value>()["html"]
        .as_str()
        .unwrap()
        .contains("Hello"));
}

// --------------------------------------------------------------------------
// Forms — the issue-#611 hero feature
// --------------------------------------------------------------------------

#[tokio::test]
async fn forms_fields_lists_acroform_fields() {
    let r = server()
        .post("/v1/forms/fields")
        .json(&json!({ "pdf_base64": b64(JP_FORM_PDF) }))
        .await;
    r.assert_status_ok();
    let v: Value = r.json();
    assert_eq!(v["field_count"], 2);
    let names: Vec<&str> = v["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"full_name"));
    assert!(names.contains(&"city"));
}

/// Contract test for the fill endpoint: a fill request is accepted and returns
/// a PDF. (The API layer is correct regardless of the upstream encoding bug.)
#[tokio::test]
async fn forms_fill_returns_pdf() {
    let r = server()
        .post("/v1/forms/fill")
        .json(&json!({
            "pdf_base64": b64(JP_FORM_PDF),
            "fields": { "full_name": "山田太郎", "city": "東京" }
        }))
        .await;
    r.assert_status_ok();
    assert_eq!(r.header(header::CONTENT_TYPE), "application/pdf");
    assert!(r.as_bytes().starts_with(b"%PDF"));
}

/// THE acceptance gate for issue #611: fill an AcroForm with non-ASCII text in
/// several scripts (CJK, Arabic, Hebrew), read it back, and assert exact UTF-8
/// equality — proving the engine emits proper UTF-16BE+BOM text strings (no
/// mojibake) and the filled form is re-readable. The mojibake failure (the
/// issue-#611 / Stirling-PDF case) was fixed upstream in `pdf_oxide` 0.3.59.
///
/// We assert by NAME *and* value. 0.3.59 round-tripped values verbatim but
/// dropped the field `/T` names on save (orphaning a bare object — issue #1),
/// so the read-back fields carried the right values under empty names. 0.3.60
/// writes the value in place on the existing field/widget object, preserving
/// its `/T` name, so a reader keying widgets by name (PyMuPDF, the #1 repro)
/// sees the value on the correct field. Both must now hold.
#[tokio::test]
async fn form_fill_roundtrip_no_mojibake() {
    // (full_name, city) samples whose PDF text strings all require UTF-16BE.
    let cases = [
        ("山田太郎", "東京都渋谷区"), // Japanese / CJK
        ("محمد علي", "القاهرة"),      // Arabic (RTL)
        ("דוד כהן", "תל אביב"),       // Hebrew (RTL)
    ];
    for (full_name, city) in cases {
        // 1. Fill -> get the filled PDF bytes back.
        let fill = server()
            .post("/v1/forms/fill")
            .json(&json!({
                "pdf_base64": b64(JP_FORM_PDF),
                "fields": { "full_name": full_name, "city": city }
            }))
            .await;
        fill.assert_status_ok();
        assert_eq!(fill.header(header::CONTENT_TYPE), "application/pdf");
        let filled = fill.as_bytes().to_vec();
        assert!(filled.starts_with(b"%PDF"), "fill did not return a PDF");

        // 2. Read the values back and assert verbatim round-trip (no mojibake).
        let fields = server()
            .post("/v1/forms/fields")
            .json(&json!({ "pdf_base64": b64(&filled) }))
            .await;
        fields.assert_status_ok();
        let v: Value = fields.json();
        let by_name: std::collections::HashMap<String, String> = v["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (
                    f["name"].as_str().unwrap_or("").to_string(),
                    f["value"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        // issue #1: the value must land on the correctly-NAMED field (the real
        // widget a reader displays), not on an orphaned, name-less object.
        assert_eq!(
            by_name.get("full_name").map(String::as_str),
            Some(full_name),
            "full_name {full_name:?} did not round-trip on its named field; got {by_name:?}"
        );
        assert_eq!(
            by_name.get("city").map(String::as_str),
            Some(city),
            "city {city:?} did not round-trip on its named field; got {by_name:?}"
        );
    }
}

// --------------------------------------------------------------------------
// Document ops
// --------------------------------------------------------------------------

#[tokio::test]
async fn merge_two_pdfs_returns_pdf() {
    let r = server()
        .post("/v1/docs/merge")
        .json(&json!({ "pdfs_base64": [b64(HELLO_PDF), b64(HELLO_PDF)] }))
        .await;
    r.assert_status_ok();
    assert_eq!(r.header(header::CONTENT_TYPE), "application/pdf");
    assert!(r.as_bytes().starts_with(b"%PDF"));
}

#[tokio::test]
async fn merge_single_pdf_is_400() {
    let r = server()
        .post("/v1/docs/merge")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn split_returns_zip() {
    let r = server()
        .post("/v1/docs/split")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status_ok();
    assert_eq!(r.header(header::CONTENT_TYPE), "application/zip");
    // ZIP local-file-header magic.
    assert_eq!(&r.as_bytes()[..2], b"PK");
}

#[tokio::test]
async fn metadata_returns_object() {
    let r = server()
        .post("/v1/docs/metadata")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status_ok();
    let v: Value = r.json();
    // Keys always present (value may be null for our minimal fixture).
    for k in [
        "title",
        "author",
        "subject",
        "keywords",
        "producer",
        "creation_date",
    ] {
        assert!(v.get(k).is_some(), "missing key {k}");
    }
}

#[tokio::test]
async fn page_info_reports_geometry() {
    let r = server()
        .post("/v1/docs/page-info")
        .json(&json!({ "pdf_base64": b64(HELLO_PDF) }))
        .await;
    r.assert_status_ok();
    let v: Value = r.json();
    assert_eq!(v["page_count"], 1);
    let p0 = &v["pages"][0];
    assert_eq!(p0["index"], 0);
    assert!((p0["width"].as_f64().unwrap() - 612.0).abs() < 1.0);
    assert!((p0["height"].as_f64().unwrap() - 792.0).abs() < 1.0);
}

// --------------------------------------------------------------------------
// Pipeline
// --------------------------------------------------------------------------

#[tokio::test]
async fn pipeline_terminal_data_op() {
    let r = server()
        .post("/v1/pipeline")
        .json(&json!({
            "pdf_base64": b64(HELLO_PDF),
            "operations": [{ "op": "page_info" }]
        }))
        .await;
    r.assert_status_ok();
    assert_eq!(r.json::<Value>()["page_count"], 1);
}

#[tokio::test]
async fn pipeline_fill_then_fields_via_two_steps_is_rejected_if_data_not_last() {
    // page_info (data) before fill (transform) -> data op must be last.
    let r = server()
        .post("/v1/pipeline")
        .json(&json!({
            "pdf_base64": b64(JP_FORM_PDF),
            "operations": [{ "op": "page_info" }, { "op": "fill", "fields": {} }]
        }))
        .await;
    r.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn pipeline_fill_then_extract_chains_in_one_parse() {
    // fill (transform) -> metadata (terminal). Proves threading works.
    let r = server()
        .post("/v1/pipeline")
        .json(&json!({
            "pdf_base64": b64(JP_FORM_PDF),
            "operations": [
                { "op": "fill", "fields": { "full_name": "テスト" } },
                { "op": "metadata" }
            ]
        }))
        .await;
    r.assert_status_ok();
    assert!(r.json::<Value>().is_object());
}
