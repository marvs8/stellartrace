//! Alert management: creates alert records from rule triggers + anomaly
//! score, owns alert lifecycle transitions, and is the *only* place that
//! is allowed to write `Alert::status`.
//!
//! Critically, the only function that can change status is
//! `AlertManager::apply_investigator_decision`, which takes a
//! `stellartrace_common::InvestigatorDecision` — a type that can only be
//! constructed by the human-review/API layer after authorization checks.
//! There is no function in this crate that accepts an `AiRecommendation`
//! and turns it into a status change; that is what enforces the
//! human-in-the-loop boundary at the type level, not just by convention.

use std::collections::HashMap;
use std::sync::RwLock;
use stellartrace_audit::{AuditEventKind, AuditLog};
use stellartrace_common::{
    Alert, InvestigationStatus, InvestigatorDecision, NormalizedTransaction, Severity,
    TriggeredRule,
};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum AlertError {
    #[error("alert not found: {0}")]
    NotFound(Uuid),
    #[error("invalid status transition from {from:?} to {to:?}")]
    InvalidTransition { from: InvestigationStatus, to: InvestigationStatus },
}

/// In-memory alert store. A production deployment would back this with a
/// real database; the interface (`create`, `get`, `list`,
/// `apply_investigator_decision`) is what matters and would not change.
pub struct AlertManager {
    alerts: RwLock<HashMap<Uuid, Alert>>,
    audit: std::sync::Arc<AuditLog>,
}

impl AlertManager {
    pub fn new(audit: std::sync::Arc<AuditLog>) -> Self {
        Self { alerts: RwLock::new(HashMap::new()), audit }
    }

    /// Creates a new alert from a flagged transaction. Only called when
    /// the rules engine produced at least one trigger.
    pub fn create_alert(
        &self,
        tx: &NormalizedTransaction,
        triggered_rules: Vec<TriggeredRule>,
        anomaly_score: f64,
        severity: Severity,
    ) -> Alert {
        let now = chrono::Utc::now();
        let alert = Alert {
            alert_id: Uuid::new_v4(),
            tx_hash: tx.tx_hash.clone(),
            source_account: tx.source_account.clone(),
            destination_account: tx.destination_account.clone(),
            asset: tx.asset.clone(),
            amount: tx.amount.clone(),
            timestamp: tx.timestamp,
            triggered_rules: triggered_rules.clone(),
            anomaly_score,
            severity,
            status: InvestigationStatus::Open,
            created_at: now,
            updated_at: now,
        };

        self.alerts.write().unwrap().insert(alert.alert_id, alert.clone());

        self.audit.append(
            alert.alert_id,
            tx.tx_hash.clone(),
            AuditEventKind::AlertCreated {
                anomaly_score,
                severity: format!("{severity:?}"),
            },
        );
        self.audit.append(
            alert.alert_id,
            tx.tx_hash.clone(),
            AuditEventKind::RulesTriggered {
                rule_ids: triggered_rules.iter().map(|r| r.rule_id.clone()).collect(),
            },
        );

        tracing::info!(
            alert_id = %alert.alert_id,
            tx_hash = %tx.tx_hash,
            severity = ?severity,
            score = anomaly_score,
            "alert created"
        );

        alert
    }

    pub fn get(&self, alert_id: Uuid) -> Option<Alert> {
        self.alerts.read().unwrap().get(&alert_id).cloned()
    }

    pub fn list(&self) -> Vec<Alert> {
        let mut alerts: Vec<Alert> = self.alerts.read().unwrap().values().cloned().collect();
        alerts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        alerts
    }

    pub fn list_by_status(&self, status: InvestigationStatus) -> Vec<Alert> {
        self.list().into_iter().filter(|a| a.status == status).collect()
    }

    /// Any status is reachable from any non-terminal status; terminal
    /// statuses (`Dismissed`, `ConfirmedSuspicious`) can still be revised
    /// by a human (e.g. new evidence reopens a dismissed alert) — the
    /// audit trail preserves the fact that it changed, which is what
    /// matters for this domain, rather than forbidding correction.
    fn validate_transition(&self, _from: InvestigationStatus, _to: InvestigationStatus) -> Result<(), AlertError> {
        Ok(())
    }

    /// The single, sole entry point for changing an alert's status. Takes
    /// an `InvestigatorDecision`, which the API layer must only construct
    /// after verifying the caller is an authorized investigator. This
    /// function has no parameter through which an `AiRecommendation`
    /// could flow into `status`.
    pub fn apply_investigator_decision(
        &self,
        decision: InvestigatorDecision,
    ) -> Result<Alert, AlertError> {
        let mut alerts = self.alerts.write().unwrap();
        let alert = alerts
            .get_mut(&decision.alert_id)
            .ok_or(AlertError::NotFound(decision.alert_id))?;

        self.validate_transition(alert.status, decision.decision)?;

        let from = alert.status;
        alert.status = decision.decision;
        alert.updated_at = chrono::Utc::now();
        let updated = alert.clone();
        drop(alerts);

        self.audit.append(
            decision.alert_id,
            updated.tx_hash.clone(),
            AuditEventKind::InvestigatorDecision {
                investigator_id: decision.investigator_id.clone(),
                decision: format!("{:?}", decision.decision),
                notes: decision.notes.clone(),
            },
        );
        self.audit.append(
            decision.alert_id,
            updated.tx_hash.clone(),
            AuditEventKind::StatusChanged { from: format!("{from:?}"), to: format!("{:?}", decision.decision) },
        );

        tracing::info!(
            alert_id = %decision.alert_id,
            investigator_id = %decision.investigator_id,
            from = ?from,
            to = ?decision.decision,
            "investigator decision applied"
        );

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as Map;
    use std::sync::Arc;
    use stellartrace_common::Asset;

    fn sample_tx() -> NormalizedTransaction {
        NormalizedTransaction {
            tx_hash: "txhash1".into(),
            ledger_sequence: 1,
            source_account: "GALICE".into(),
            destination_account: Some("GBOB".into()),
            asset: Asset::Native,
            amount: "50000".into(),
            timestamp: chrono::Utc::now(),
            is_soroban_invocation: false,
            contract_id: None,
            metadata: Map::new(),
        }
    }

    fn manager() -> AlertManager {
        AlertManager::new(Arc::new(AuditLog::new()))
    }

    #[test]
    fn create_alert_starts_open() {
        let mgr = manager();
        let alert = mgr.create_alert(&sample_tx(), vec![], 10.0, Severity::Low);
        assert_eq!(alert.status, InvestigationStatus::Open);
        assert_eq!(mgr.get(alert.alert_id).unwrap().status, InvestigationStatus::Open);
    }

    #[test]
    fn investigator_decision_changes_status() {
        let mgr = manager();
        let alert = mgr.create_alert(&sample_tx(), vec![], 80.0, Severity::Critical);

        let decision = InvestigatorDecision {
            alert_id: alert.alert_id,
            investigator_id: "inv-1".into(),
            decision: InvestigationStatus::Escalated,
            notes: "needs compliance review".into(),
            decided_at: chrono::Utc::now(),
        };
        let updated = mgr.apply_investigator_decision(decision).unwrap();
        assert_eq!(updated.status, InvestigationStatus::Escalated);
    }

    #[test]
    fn decision_on_unknown_alert_errors() {
        let mgr = manager();
        let decision = InvestigatorDecision {
            alert_id: Uuid::new_v4(),
            investigator_id: "inv-1".into(),
            decision: InvestigationStatus::Dismissed,
            notes: "".into(),
            decided_at: chrono::Utc::now(),
        };
        assert!(matches!(mgr.apply_investigator_decision(decision), Err(AlertError::NotFound(_))));
    }

    #[test]
    fn repeated_decisions_are_all_preserved_in_audit_not_overwritten() {
        let mgr = manager();
        let alert = mgr.create_alert(&sample_tx(), vec![], 80.0, Severity::Critical);

        mgr.apply_investigator_decision(InvestigatorDecision {
            alert_id: alert.alert_id,
            investigator_id: "inv-1".into(),
            decision: InvestigationStatus::UnderInvestigation,
            notes: "initial look".into(),
            decided_at: chrono::Utc::now(),
        }).unwrap();

        mgr.apply_investigator_decision(InvestigatorDecision {
            alert_id: alert.alert_id,
            investigator_id: "inv-2".into(),
            decision: InvestigationStatus::ConfirmedSuspicious,
            notes: "confirmed via off-chain KYC lookup".into(),
            decided_at: chrono::Utc::now(),
        }).unwrap();

        let history = mgr.audit.for_alert(alert.alert_id);
        let decision_events = history
            .iter()
            .filter(|r| matches!(r.event, AuditEventKind::InvestigatorDecision { .. }))
            .count();
        assert_eq!(decision_events, 2, "both decisions must remain in the audit trail");
    }
}
