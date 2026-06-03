//! Prometheus metrics. RED metrics (Rate, Errors, Duration) by route template +
//! status, plus an in-flight gauge. Privacy rule: labels carry only the matched
//! ROUTE TEMPLATE (e.g. `/v1/extract/text`) and method/status — never the PDF,
//! filenames, or field values.
//!
//! The recorder is a process-global singleton installed once in `main`; the
//! render handle lives in `AppState`. Tests do not install it (the metrics
//! macros are cheap no-ops without a recorder), keeping many test servers safe.

use std::time::Instant;

use axum::{extract::MatchedPath, extract::Request, middleware::Next, response::Response};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// Install the global Prometheus recorder and return the render handle.
pub fn init_recorder() -> PrometheusHandle {
    PrometheusBuilder::new()
        .install_recorder()
        .expect("install prometheus recorder")
}

/// Middleware: count + time every request, labelled by route template.
pub async fn track(req: Request, next: Next) -> Response {
    let method = req.method().as_str().to_owned();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|m| m.as_str().to_owned())
        .unwrap_or_else(|| "<unmatched>".to_owned());

    let start = Instant::now();
    metrics::gauge!("pdf_oxide_api_requests_in_flight").increment(1.0);

    let response = next.run(req).await;

    metrics::gauge!("pdf_oxide_api_requests_in_flight").decrement(1.0);
    let status = response.status().as_u16().to_string();
    let elapsed = start.elapsed().as_secs_f64();

    metrics::counter!(
        "pdf_oxide_api_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status,
    )
    .increment(1);
    metrics::histogram!(
        "pdf_oxide_api_request_duration_seconds",
        "method" => method,
        "path" => path,
    )
    .record(elapsed);

    response
}
