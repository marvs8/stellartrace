//! Horizon polling client. Polls `/operations?cursor=...` in order and
//! hands normalized transactions to the pipeline. Kept intentionally
//! simple (cursor-based polling) rather than streaming SSE, since polling
//! is easier to make resilient to transient failures and duplicate-safe
//! via cursor persistence.

use crate::normalize::{normalize_horizon_operation, HorizonPaymentOperation};
use async_trait::async_trait;
use serde::Deserialize;
use stellartrace_common::NormalizedTransaction;

#[derive(Debug, thiserror::Error)]
pub enum HorizonError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("normalization error: {0}")]
    Normalize(#[from] crate::normalize::NormalizeError),
}

#[derive(Debug, Deserialize)]
struct HorizonOperationsResponse {
    #[serde(rename = "_embedded")]
    embedded: Embedded,
}

#[derive(Debug, Deserialize)]
struct Embedded {
    records: Vec<HorizonPaymentOperation>,
}

/// Abstraction over "a thing that can fetch a page of operations since a
/// cursor," so tests can substitute a fixture-backed fake instead of
/// hitting the network.
#[async_trait]
pub trait HorizonClient: Send + Sync {
    async fn fetch_operations(
        &self,
        cursor: &str,
    ) -> Result<(Vec<NormalizedTransaction>, String), HorizonError>;
}

/// Real Horizon HTTP client.
pub struct HttpHorizonClient {
    base_url: String,
    http: reqwest::Client,
    page_limit: u32,
}

impl HttpHorizonClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: reqwest::Client::new(),
            page_limit: 50,
        }
    }
}

#[async_trait]
impl HorizonClient for HttpHorizonClient {
    async fn fetch_operations(
        &self,
        cursor: &str,
    ) -> Result<(Vec<NormalizedTransaction>, String), HorizonError> {
        let url = format!(
            "{}/operations?cursor={}&order=asc&limit={}&join=transactions",
            self.base_url.trim_end_matches('/'),
            cursor,
            self.page_limit
        );
        let resp: HorizonOperationsResponse = self
            .http
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let mut normalized = Vec::new();
        let mut next_cursor = cursor.to_string();
        for record in resp.embedded.records {
            next_cursor = record.transaction_hash.clone();
            match normalize_horizon_operation(record) {
                Ok(tx) => normalized.push(tx),
                Err(e) => {
                    tracing::warn!(error = %e, "skipping unnormalizable operation");
                }
            }
        }
        Ok((normalized, next_cursor))
    }
}
