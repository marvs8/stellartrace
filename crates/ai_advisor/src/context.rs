//! Builds the minimal context sent to the AI advisor.
//!
//! Deliberately a narrow, explicit struct rather than "pass the whole
//! alert object": every field here was chosen because the AI needs it to
//! produce a useful assessment. Anything not listed here (investigator
//! notes, other investigators' identities, unrelated account activity,
//! full audit history) is never sent, which is both a privacy control and
//! a prompt-injection surface reduction.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stellartrace_common::Alert;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiContext {
    pub alert_id: Uuid,
    pub tx_hash: String,
    pub source_account: String,
    pub destination_account: Option<String>,
    pub asset_description: String,
    pub amount: String,
    pub timestamp: DateTime<Utc>,
    pub triggered_rule_reasons: Vec<String>,
    pub anomaly_score: f64,
    pub severity: String,
    /// A short, pre-aggregated summary of relevant prior activity (e.g.
    /// "4 transactions to this destination in the last 10 minutes; none
    /// previously flagged"), not raw account history dumps.
    pub recent_related_history_summary: String,
}

pub fn build_context(alert: &Alert, recent_related_history_summary: impl Into<String>) -> AiContext {
    AiContext {
        alert_id: alert.alert_id,
        tx_hash: alert.tx_hash.clone(),
        source_account: alert.source_account.clone(),
        destination_account: alert.destination_account.clone(),
        asset_description: alert.asset.to_string(),
        amount: alert.amount.clone(),
        timestamp: alert.timestamp,
        triggered_rule_reasons: alert.triggered_rules.iter().map(|r| r.reason.clone()).collect(),
        anomaly_score: alert.anomaly_score,
        severity: format!("{:?}", alert.severity),
        recent_related_history_summary: recent_related_history_summary.into(),
    }
}
