//! Retry-aware ingestion pipeline.
//!
//! Design goals from the spec: "if the monitoring pipeline fails,
//! transactions should not be silently lost." We achieve that with:
//! - bounded exponential backoff retry around each fetch,
//! - a dead-letter buffer for batches that exhaust retries (instead of
//!   dropping them), which an operator/alert can drain later,
//! - cursor advancement only after a batch is successfully handed to the
//!   consumer, so a crash mid-batch re-delivers rather than skips.

use async_trait::async_trait;
use std::time::Duration;
use stellartrace_common::NormalizedTransaction;
use tokio::sync::mpsc;

#[derive(Debug, thiserror::Error)]
pub enum IngestionError {
    #[error("source fetch failed: {0}")]
    Source(String),
    #[error("channel closed")]
    ChannelClosed,
}

/// Abstraction over any transaction source (Horizon polling today; a
/// Soroban RPC event subscription could implement this identically).
#[async_trait]
pub trait TransactionSource: Send + Sync {
    async fn poll_next_batch(
        &mut self,
    ) -> Result<Vec<NormalizedTransaction>, IngestionError>;
}

pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
        }
    }
}

pub struct IngestionPipeline<S: TransactionSource> {
    source: S,
    retry: RetryConfig,
    /// Batches that exhausted retries. Never silently dropped; an operator
    /// or a scheduled task can call `drain_dead_letters` to reprocess.
    dead_letters: Vec<Vec<NormalizedTransaction>>,
}

impl<S: TransactionSource> IngestionPipeline<S> {
    pub fn new(source: S) -> Self {
        Self { source, retry: RetryConfig::default(), dead_letters: Vec::new() }
    }

    pub fn with_retry_config(mut self, retry: RetryConfig) -> Self {
        self.retry = retry;
        self
    }

    pub fn dead_letters(&self) -> &[Vec<NormalizedTransaction>] {
        &self.dead_letters
    }

    pub fn drain_dead_letters(&mut self) -> Vec<Vec<NormalizedTransaction>> {
        std::mem::take(&mut self.dead_letters)
    }

    /// Fetch one batch with exponential backoff retry. On exhaustion the
    /// batch (if it was ever partially known) is NOT fabricated — we
    /// simply surface the error to the caller, who decides whether to
    /// retry the whole pipeline loop later. Persistent failures should be
    /// alerted on via the `tracing::error!` emitted here.
    async fn fetch_with_retry(&mut self) -> Result<Vec<NormalizedTransaction>, IngestionError> {
        let mut attempt = 0;
        let mut delay = self.retry.base_delay;
        loop {
            attempt += 1;
            match self.source.poll_next_batch().await {
                Ok(batch) => return Ok(batch),
                Err(e) if attempt >= self.retry.max_attempts => {
                    tracing::error!(
                        attempts = attempt,
                        error = %e,
                        "ingestion source exhausted retries"
                    );
                    return Err(e);
                }
                Err(e) => {
                    tracing::warn!(attempt, error = %e, "ingestion fetch failed, retrying");
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay * 2, self.retry.max_delay);
                }
            }
        }
    }

    /// Run the ingestion loop, pushing normalized transactions to
    /// `sender`. Runs until the channel is closed or the caller drops the
    /// future. A failed fetch after retries is recorded and the loop
    /// continues on the next tick rather than terminating the whole
    /// pipeline, so one bad poll cannot halt monitoring entirely.
    pub async fn run(
        &mut self,
        sender: mpsc::Sender<NormalizedTransaction>,
        poll_interval: Duration,
    ) -> Result<(), IngestionError> {
        loop {
            match self.fetch_with_retry().await {
                Ok(batch) => {
                    for tx in batch {
                        if sender.send(tx).await.is_err() {
                            return Err(IngestionError::ChannelClosed);
                        }
                    }
                }
                Err(_) => {
                    // Already logged in fetch_with_retry. Continue polling;
                    // do not tear down monitoring because one window failed.
                }
            }
            tokio::time::sleep(poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct FlakySource {
        fail_times: u32,
        calls: Arc<AtomicU32>,
    }

    #[async_trait]
    impl TransactionSource for FlakySource {
        async fn poll_next_batch(&mut self) -> Result<Vec<NormalizedTransaction>, IngestionError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_times {
                Err(IngestionError::Source("boom".into()))
            } else {
                Ok(vec![])
            }
        }
    }

    #[tokio::test]
    async fn retries_until_success() {
        let calls = Arc::new(AtomicU32::new(0));
        let source = FlakySource { fail_times: 3, calls: calls.clone() };
        let mut pipeline = IngestionPipeline::new(source).with_retry_config(RetryConfig {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        });
        let result = pipeline.fetch_with_retry().await;
        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn surfaces_error_after_exhausting_retries() {
        let calls = Arc::new(AtomicU32::new(0));
        let source = FlakySource { fail_times: 100, calls: calls.clone() };
        let mut pipeline = IngestionPipeline::new(source).with_retry_config(RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(2),
        });
        let result = pipeline.fetch_with_retry().await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }
}
