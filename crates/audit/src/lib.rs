//! Append-only, tamper-evident audit trail.
//!
//! Every action that matters for an investigation — alert creation, rule
//! triggers, what was sent to the AI, what the AI said, and every
//! investigator decision — is recorded as an immutable `AuditRecord`.
//! Records are never edited or deleted; a changed decision is a *new*
//! record referencing the same alert, so the full history is always
//! reconstructable.
//!
//! Tamper evidence is implemented as a hash chain: each record embeds the
//! SHA-256 hash of the previous record plus its own content, so altering
//! or removing any past record breaks every subsequent hash and is
//! detectable via `AuditLog::verify_integrity`. This is a practical,
//! dependency-free approximation of a tamper-evident log; it does not
//! require or imply an on-chain anchor, though the resulting hash chain
//! could optionally be checkpointed on-chain later without redesigning
//! this crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AuditEventKind {
    AlertCreated { anomaly_score: f64, severity: String },
    RulesTriggered { rule_ids: Vec<String> },
    AiContextSent { context_summary: String },
    AiRecommendationReceived { confidence: f64, model_available: bool },
    InvestigatorDecision { investigator_id: String, decision: String, notes: String },
    StatusChanged { from: String, to: String },
    Note { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub record_id: Uuid,
    pub alert_id: Uuid,
    pub tx_hash: String,
    pub timestamp: DateTime<Utc>,
    pub event: AuditEventKind,
    /// Hash of the previous record in the chain (hex-encoded), or a fixed
    /// genesis value for the first record.
    pub prev_hash: String,
    /// SHA-256 of this record's content chained with `prev_hash`.
    pub this_hash: String,
}

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

fn compute_hash(prev_hash: &str, record_id: &Uuid, alert_id: &Uuid, tx_hash: &str, timestamp: &DateTime<Utc>, event: &AuditEventKind) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prev_hash.as_bytes());
    hasher.update(record_id.as_bytes());
    hasher.update(alert_id.as_bytes());
    hasher.update(tx_hash.as_bytes());
    hasher.update(timestamp.to_rfc3339().as_bytes());
    hasher.update(serde_json::to_vec(event).unwrap_or_default());
    hex::encode(hasher.finalize())
}

/// Thread-safe, append-only audit log. Backed by an in-memory `Vec` for
/// this reference implementation; swapping to a persistent append-only
/// store (e.g. a WAL-backed table with no UPDATE/DELETE grants) is a
/// drop-in replacement behind the same `append`/`for_alert` interface.
#[derive(Default)]
pub struct AuditLog {
    records: RwLock<Vec<AuditRecord>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self { records: RwLock::new(Vec::new()) }
    }

    pub fn append(&self, alert_id: Uuid, tx_hash: impl Into<String>, event: AuditEventKind) -> AuditRecord {
        let tx_hash = tx_hash.into();
        let mut records = self.records.write().unwrap();
        let prev_hash = records.last().map(|r| r.this_hash.clone()).unwrap_or_else(|| GENESIS_HASH.to_string());
        let record_id = Uuid::new_v4();
        let timestamp = Utc::now();
        let this_hash = compute_hash(&prev_hash, &record_id, &alert_id, &tx_hash, &timestamp, &event);

        tracing::info!(
            alert_id = %alert_id,
            tx_hash = %tx_hash,
            event = ?event,
            "audit event recorded"
        );

        let record = AuditRecord { record_id, alert_id, tx_hash, timestamp, event, prev_hash, this_hash };
        records.push(record.clone());
        record
    }

    pub fn for_alert(&self, alert_id: Uuid) -> Vec<AuditRecord> {
        self.records
            .read()
            .unwrap()
            .iter()
            .filter(|r| r.alert_id == alert_id)
            .cloned()
            .collect()
    }

    pub fn all(&self) -> Vec<AuditRecord> {
        self.records.read().unwrap().clone()
    }

    /// Recomputes the hash chain over the full log and returns `Ok(())`
    /// if every record's stored hash matches its recomputed hash and
    /// correctly chains to its predecessor. Any mismatch means a record
    /// was altered, reordered, or removed after the fact.
    pub fn verify_integrity(&self) -> Result<(), IntegrityError> {
        let records = self.records.read().unwrap();
        let mut expected_prev = GENESIS_HASH.to_string();
        for (i, record) in records.iter().enumerate() {
            if record.prev_hash != expected_prev {
                return Err(IntegrityError::ChainBroken { index: i });
            }
            let recomputed = compute_hash(
                &record.prev_hash,
                &record.record_id,
                &record.alert_id,
                &record.tx_hash,
                &record.timestamp,
                &record.event,
            );
            if recomputed != record.this_hash {
                return Err(IntegrityError::HashMismatch { index: i });
            }
            expected_prev = record.this_hash.clone();
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IntegrityError {
    #[error("audit chain broken at record index {index}")]
    ChainBroken { index: usize },
    #[error("audit record hash mismatch at index {index}")]
    HashMismatch { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_only_records_preserve_history() {
        let log = AuditLog::new();
        let alert_id = Uuid::new_v4();
        log.append(alert_id, "tx1", AuditEventKind::AlertCreated { anomaly_score: 80.0, severity: "high".into() });
        log.append(alert_id, "tx1", AuditEventKind::InvestigatorDecision {
            investigator_id: "inv1".into(),
            decision: "under_investigation".into(),
            notes: "looking into it".into(),
        });
        log.append(alert_id, "tx1", AuditEventKind::InvestigatorDecision {
            investigator_id: "inv1".into(),
            decision: "confirmed_suspicious".into(),
            notes: "confirmed after review".into(),
        });

        let history = log.for_alert(alert_id);
        assert_eq!(history.len(), 3);
        // both decisions remain present; the earlier one was never deleted
        let decisions: Vec<_> = history
            .iter()
            .filter_map(|r| match &r.event {
                AuditEventKind::InvestigatorDecision { decision, .. } => Some(decision.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(decisions, vec!["under_investigation", "confirmed_suspicious"]);
    }

    #[test]
    fn verify_integrity_passes_on_untampered_log() {
        let log = AuditLog::new();
        let alert_id = Uuid::new_v4();
        for _ in 0..5 {
            log.append(alert_id, "tx1", AuditEventKind::Note { message: "x".into() });
        }
        assert!(log.verify_integrity().is_ok());
    }

    #[test]
    fn verify_integrity_detects_tampering() {
        let log = AuditLog::new();
        let alert_id = Uuid::new_v4();
        log.append(alert_id, "tx1", AuditEventKind::Note { message: "a".into() });
        log.append(alert_id, "tx1", AuditEventKind::Note { message: "b".into() });

        {
            let mut records = log.records.write().unwrap();
            records[0].event = AuditEventKind::Note { message: "tampered".into() };
        }

        assert!(log.verify_integrity().is_err());
    }
}
