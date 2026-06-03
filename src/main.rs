//! pdf_oxide_api — a stateless, single-shot REST service wrapping the
//! `pdf_oxide` Rust library.
//!
//! Design (see `docs/releases/plans/v0.1.0/`):
//! - Stateless: PDF in -> result out. Nothing is persisted between requests.
//! - The ONE non-negotiable rule: every `pdf_oxide` call goes through
//!   `state.cpu.run(...)`, a bounded rayon pool fronted by a tokio semaphore.
//!   We deliberately do NOT use `spawn_blocking` (unbounded 512-thread pool ->
//!   OOM under large-PDF bursts).
//! - Errors map to RFC 9457 `application/problem+json` via `ApiError`.

// musl allocator perf for the static image; harmless elsewhere.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::{from_fn, from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use metrics_exporter_prometheus::PrometheusHandle;
use serde::Serialize;
use tower_http::{
    catch_panic::CatchPanicLayer, compression::CompressionLayer, cors::CorsLayer,
    timeout::TimeoutLayer, trace::TraceLayer,
};

mod auth;
mod blocking;
mod config;
mod error;
mod handlers;
mod io;
mod metrics;
#[cfg(test)]
mod tests;

use blocking::CpuPool;
use config::Config;
use handlers::PDF_OXIDE_VERSION;

/// Shared, cheap-to-clone application state.
#[derive(Clone)]
pub struct AppState {
    pub cpu: CpuPool,
    pub cfg: Arc<Config>,
    /// Flipped to `false` on shutdown so `/readyz` returns 503 while draining.
    pub ready: Arc<AtomicBool>,
    /// Prometheus render handle. `None` in tests (no global recorder installed).
    pub metrics: Option<Arc<PrometheusHandle>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `anyhow` is permitted only here in startup. Handlers return ApiError.

    // Support the no-shell-image `healthcheck` subcommand (exec form):
    //   ENTRYPOINT ["/usr/local/bin/pdf_oxide_api"]; HEALTHCHECK CMD [..., "healthcheck"]
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(run_healthcheck());
    }

    init_tracing();

    let cfg = Config::from_env()?;

    // A non-loopback bind without an API key would serve confidential-document
    // processing unauthenticated. Containers bind 0.0.0.0 by design, so the
    // DEFAULT is a loud warning (keeps zero-config `docker run` working);
    // operators who want hard fail-closed set PDF_OXIDE_API_REQUIRE_AUTH=true.
    if !cfg.is_loopback_bind() && cfg.api_key.is_none() {
        if cfg.require_auth {
            anyhow::bail!(
                "PDF_OXIDE_API_REQUIRE_AUTH is set but no PDF_OXIDE_API_KEY is configured \
                 for the non-loopback bind {}; set an API key or bind to loopback",
                cfg.addr
            );
        }
        tracing::warn!(
            addr = %cfg.addr,
            "binding a non-loopback address WITHOUT an API key — anyone who can reach this \
             port can process PDFs. Set PDF_OXIDE_API_KEY (and front it with TLS) for any \
             network-exposed deployment, or PDF_OXIDE_API_REQUIRE_AUTH=true to hard-fail."
        );
    }

    let addr: SocketAddr = cfg.addr.parse()?;
    let cpu = CpuPool::new(cfg.cpu_threads, cfg.max_inflight)?;
    let state = AppState {
        cpu,
        cfg: Arc::new(cfg),
        ready: Arc::new(AtomicBool::new(true)),
        metrics: Some(Arc::new(metrics::init_recorder())),
    };

    let app = build_router(state.clone());

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "pdf_oxide_api listening");

    let ready = state.ready.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(ready))
        .await?;

    Ok(())
}

/// Build the full router. Exposed (`pub`) as the `build_router(state) -> Router`
/// factory the integration-test suite depends on.
pub fn build_router(state: AppState) -> Router {
    let body_limit = state.cfg.max_body_bytes;
    let timeout = Duration::from_secs(state.cfg.request_timeout_secs);
    let cors = if state.cfg.enable_cors {
        CorsLayer::permissive()
    } else {
        CorsLayer::new() // no Access-Control-* headers -> cross-origin denied
    };

    // Operational endpoints — unversioned, dependency-free, unauthenticated.
    let ops = Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/version", get(version))
        .route("/metrics", get(metrics_handler));

    // OpenAPI contract (served verbatim from the version-controlled spec).
    let api_docs = Router::new()
        .route("/openapi.json", get(openapi_json))
        .route("/openapi.yaml", get(openapi_yaml));
    #[cfg(feature = "swagger-ui")]
    let api_docs = api_docs.route("/docs", get(docs_ui));

    // Versioned data API (stateless single-shot). Auth applies to /v1 only.
    let v1 = Router::new()
        .route("/v1/extract/text", post(handlers::extract::text))
        .route("/v1/extract/markdown", post(handlers::extract::markdown))
        .route("/v1/extract/html", post(handlers::extract::html))
        .route("/v1/forms/fields", post(handlers::forms::fields))
        .route("/v1/forms/fill", post(handlers::forms::fill))
        .route("/v1/docs/merge", post(handlers::docs::merge))
        .route("/v1/docs/split", post(handlers::docs::split))
        .route("/v1/docs/metadata", post(handlers::docs::metadata))
        .route("/v1/docs/page-info", post(handlers::docs::page_info))
        .route("/v1/pipeline", post(handlers::pipeline::pipeline))
        .route_layer(from_fn_with_state(state.clone(), auth::require_api_key));

    Router::new()
        .merge(ops)
        .merge(api_docs)
        .merge(v1)
        // RED metrics on matched routes (MatchedPath available under route_layer).
        .route_layer(from_fn(metrics::track))
        .layer(from_fn(security_headers))
        .layer(cors)
        // Configurable body cap. Using axum's DefaultBodyLimit (not tower-http's
        // RequestBodyLimitLayer) so the Bytes/Json/Multipart extractors surface
        // an over-limit body as a 413-mapped LengthLimitError even when no
        // Content-Length is sent (see io::map_bytes_rejection).
        .layer(DefaultBodyLimit::max(body_limit))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            timeout,
        ))
        .layer(CompressionLayer::new())
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Operational handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Status {
    status: &'static str,
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, Json(Status { status: "ok" }))
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    if state.ready.load(Ordering::Relaxed) {
        (StatusCode::OK, Json(Status { status: "ready" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(Status { status: "draining" }),
        )
    }
}

#[derive(Serialize)]
struct VersionResponse {
    service: &'static str,
    version: &'static str,
    pdf_oxide_version: &'static str,
}

async fn version() -> impl IntoResponse {
    Json(VersionResponse {
        service: "pdf_oxide_api",
        version: env!("CARGO_PKG_VERSION"),
        pdf_oxide_version: PDF_OXIDE_VERSION,
    })
}

async fn metrics_handler(State(state): State<AppState>) -> Response {
    let body = state
        .metrics
        .as_ref()
        .map(|h| h.render())
        .unwrap_or_else(|| "# metrics recorder not installed\n".to_string());
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4"),
        )],
        body,
    )
        .into_response()
}

async fn openapi_json() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        include_str!("../openapi.json"),
    )
}

async fn openapi_yaml() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/yaml"),
        )],
        include_str!("../openapi.yaml"),
    )
}

/// Self-contained API docs viewer (Scalar). Behind the `swagger-ui` feature so
/// a fully air-gapped image can drop it with `--no-default-features`.
#[cfg(feature = "swagger-ui")]
async fn docs_ui() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!doctype html><html><head><meta charset="utf-8">
<title>pdf_oxide_api — API reference</title>
<meta name="viewport" content="width=device-width,initial-scale=1"></head>
<body><script id="api-reference" data-url="/openapi.json"></script>
<script src="https://cdn.jsdelivr.net/npm/@scalar/api-reference"></script>
</body></html>"#,
    )
}

/// Add conservative security headers to every response.
async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    resp
}

// ---------------------------------------------------------------------------
// Startup helpers
// ---------------------------------------------------------------------------

fn init_tracing() {
    use tracing_subscriber::{prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // JSON in containers, pretty in a TTY. Never log PDF bytes / extracted text.
    let fmt = tracing_subscriber::fmt::layer();
    tracing_subscriber::registry().with(filter).with(fmt).init();
}

/// Localhost self-probe for the no-shell Docker `HEALTHCHECK`. Returns a process
/// exit code. Kept minimal and dependency-free (uses a blocking TCP connect).
fn run_healthcheck() -> i32 {
    let addr = std::env::var("PDF_OXIDE_API_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let probe = addr.replace("0.0.0.0", "127.0.0.1");
    match std::net::TcpStream::connect(probe) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

async fn shutdown_signal(ready: Arc<AtomicBool>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    // Flip readiness FIRST so load balancers drain us before connections close.
    ready.store(false, Ordering::Relaxed);
    tracing::info!("shutdown signal received; draining");
}
