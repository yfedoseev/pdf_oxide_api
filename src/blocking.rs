//! Bounded CPU offload for synchronous, CPU-bound `pdf_oxide` work.
//!
//! Every `pdf_oxide` call MUST go through [`CpuPool::run`] — the project's one
//! non-negotiable concurrency rule. See `framework-decision.md` §4.
//!
//! Why not `tokio::task::spawn_blocking`? Its pool defaults to ~512 threads and
//! is meant for blocking I/O, not sustained CPU crunching. A burst of large
//! PDFs would spawn dozens-to-hundreds of threads each holding a parsed PDF in
//! memory -> OOM, with no back-pressure. Instead we use a fixed-size rayon pool
//! fronted by a tokio semaphore (admission control = the real concurrency cap),
//! bridging the result back over a oneshot so the tokio worker is freed
//! immediately.

use std::sync::Arc;

use tokio::sync::{oneshot, Semaphore};

use crate::error::ApiError;

/// Process-wide handle to the CPU pool + admission gate. Cheap to clone.
#[derive(Clone)]
pub struct CpuPool {
    pool: Arc<rayon::ThreadPool>,
    permits: Arc<Semaphore>,
}

impl CpuPool {
    /// `threads` sizes the rayon pool; `max_in_flight` caps concurrent PDF jobs
    /// (bounds peak memory).
    pub fn new(threads: usize, max_in_flight: usize) -> anyhow::Result<Self> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .thread_name(|i| format!("pdf-cpu-{i}"))
            .build()?;
        Ok(Self {
            pool: Arc::new(pool),
            permits: Arc::new(Semaphore::new(max_in_flight.max(1))),
        })
    }

    /// Run a blocking, CPU-bound closure off the async runtime. Back-pressures
    /// via the semaphore; never blocks a tokio worker. Panics in the closure
    /// (e.g. a pathological PDF) are caught so they cannot poison a pool thread.
    pub async fn run<F, T>(&self, job: F) -> Result<T, ApiError>
    where
        F: FnOnce() -> Result<T, ApiError> + Send + 'static,
        T: Send + 'static,
    {
        // Admission control: bounds in-flight jobs -> bounds peak memory.
        let _permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::ShuttingDown)?;

        let (tx, rx) = oneshot::channel();
        self.pool.spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(job));
            let _ = tx.send(result);
        });

        match rx.await {
            Ok(Ok(value)) => value, // the job's own Result
            Ok(Err(_panic)) => Err(ApiError::PdfPanic),
            Err(_recv) => Err(ApiError::Internal), // worker vanished
        }
        // `_permit` dropped here -> next queued request admitted.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn run_returns_value() {
        let pool = CpuPool::new(2, 2).unwrap();
        let out = pool.run(|| Ok(2 + 2)).await.unwrap();
        assert_eq!(out, 4);
    }

    #[tokio::test]
    async fn run_isolates_panics() {
        let pool = CpuPool::new(2, 2).unwrap();
        let err = pool
            .run(|| -> Result<(), ApiError> { panic!("bad pdf") })
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::PdfPanic));
        // Pool still usable after a panic.
        assert_eq!(pool.run(|| Ok(1)).await.unwrap(), 1);
    }
}
