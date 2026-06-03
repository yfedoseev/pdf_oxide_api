//! Runtime configuration, sourced from environment variables with safe
//! defaults. All knobs are documented in the README / docs `configuration`
//! page and use the single `PDF_OXIDE_API_` prefix (decisions-locked.md §1).
//! No field here is logged; the API key is a secret and is never emitted.

use anyhow::Context;

/// Default maximum request body: 32 MiB.
pub const DEFAULT_MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
/// Default per-request timeout, seconds.
pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
/// Default concurrent-job admission cap (decisions-locked.md §2: bounds peak
/// memory at MAX_BODY_BYTES x MAX_INFLIGHT so it fits the 1 GiB compose limit).
pub const DEFAULT_MAX_INFLIGHT: usize = 8;
/// Default maximum page count accepted for processing (PDF-bomb guard).
pub const DEFAULT_MAX_PAGES: usize = 2000;
/// Default maximum number of steps in a `/v1/pipeline` request.
pub const DEFAULT_MAX_PIPELINE_STEPS: usize = 16;

#[derive(Debug, Clone)]
pub struct Config {
    /// Bind address, e.g. `0.0.0.0:8080`.
    pub addr: String,
    /// rayon CPU pool size.
    pub cpu_threads: usize,
    /// Semaphore permits = the real concurrency cap.
    pub max_inflight: usize,
    /// Hard request-body ceiling, bytes.
    pub max_body_bytes: usize,
    /// Per-request timeout, seconds.
    pub request_timeout_secs: u64,
    /// Reject documents with more than this many pages (PDF-bomb guard).
    pub max_pages: usize,
    /// Maximum number of ops in a single pipeline request.
    pub max_pipeline_steps: usize,
    /// Optional bearer API key. When set, all `/v1/*` calls must present
    /// `Authorization: Bearer <key>`. Never logged.
    pub api_key: Option<String>,
    /// Enable permissive CORS (off by default; locked down).
    pub enable_cors: bool,
    /// When true, refuse to start on a non-loopback bind without an API key
    /// (hard fail-closed). Default false: a containerised `0.0.0.0` bind is the
    /// norm, so by default we only emit a loud startup warning.
    pub require_auth: bool,
}

impl Config {
    /// Load configuration from the environment, applying defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        let cpu_threads = parse_env("PDF_OXIDE_API_CPU_THREADS")?.unwrap_or_else(num_cpus);
        let api_key = std::env::var("PDF_OXIDE_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        Ok(Self {
            addr: std::env::var("PDF_OXIDE_API_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            cpu_threads,
            max_inflight: parse_env("PDF_OXIDE_API_MAX_INFLIGHT")?.unwrap_or(DEFAULT_MAX_INFLIGHT),
            max_body_bytes: parse_env("PDF_OXIDE_API_MAX_BODY_BYTES")?
                .unwrap_or(DEFAULT_MAX_BODY_BYTES),
            request_timeout_secs: parse_env("PDF_OXIDE_API_REQUEST_TIMEOUT_SECS")?
                .unwrap_or(DEFAULT_REQUEST_TIMEOUT_SECS),
            max_pages: parse_env("PDF_OXIDE_API_MAX_PAGES")?.unwrap_or(DEFAULT_MAX_PAGES),
            max_pipeline_steps: parse_env("PDF_OXIDE_API_MAX_PIPELINE_STEPS")?
                .unwrap_or(DEFAULT_MAX_PIPELINE_STEPS),
            api_key,
            enable_cors: parse_env("PDF_OXIDE_API_ENABLE_CORS")?.unwrap_or(false),
            require_auth: parse_env("PDF_OXIDE_API_REQUIRE_AUTH")?.unwrap_or(false),
        })
    }

    /// True when the bind address resolves only to loopback (no auth required
    /// by default; a non-loopback bind without an API key fails closed).
    pub fn is_loopback_bind(&self) -> bool {
        use std::net::ToSocketAddrs;
        match self.addr.to_socket_addrs() {
            Ok(mut addrs) => {
                let mut any = false;
                let all_loopback = addrs.all(|sa| {
                    any = true;
                    sa.ip().is_loopback()
                });
                any && all_loopback
            }
            // Unresolvable (e.g. "0.0.0.0:8080" resolves fine; a hostname may
            // not at config time) -> treat as non-loopback (safer).
            Err(_) => false,
        }
    }

    /// Small, deterministic config for tests.
    #[cfg(test)]
    pub fn for_tests() -> Self {
        Self {
            addr: "127.0.0.1:0".into(),
            cpu_threads: 2,
            max_inflight: 4,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            request_timeout_secs: DEFAULT_REQUEST_TIMEOUT_SECS,
            max_pages: DEFAULT_MAX_PAGES,
            max_pipeline_steps: DEFAULT_MAX_PIPELINE_STEPS,
            api_key: None,
            enable_cors: false,
            require_auth: false,
        }
    }
}

fn parse_env<T>(key: &str) -> anyhow::Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) => v
            .parse::<T>()
            .map(Some)
            .map_err(|e| anyhow::anyhow!("invalid {key}: {e}")),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {key}")),
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}
