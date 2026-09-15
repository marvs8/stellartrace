//! HTTP handlers. Each one is a thin adapter: parse/authorize the
//! request, call into the appropriate domain crate, serialize the
//! result. No business logic lives here beyond that glue, so the
//! rules/scoring/alert/audit/AI behavior is identical whether it's
//! exercised through this API or directly in a Rust test.

use axum::extract::{Path, Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use stellartrace_ai_advisor::build_context;
use stellartrace_audit::{AuditEventKind, AuditRecord};
use stellartrace_common::{
    AiRecommendation, Alert, InvestigationStatus, InvestigatorDecision, NormalizedTransaction,
    TriggeredRule,
};
use stellartrace_rules_engine::RuleContext;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn health() -> &'static str {
    "ok"
}

pub async fn metrics(State(state): State<Arc<AppState>>) -> String {
    state.metrics.render_prometheus_text()
}

// ---------- Transactions ----------

pub async fn ingest_transaction(
    State(state): State<Arc<AppState>>,
    Json(tx): Json<NormalizedTransaction>,
) -> Result<Json<NormalizedTransaction>, ApiError> {
    tracing::info!(tx_hash = %tx.tx_hash, source = %tx.source_account, "ingesting transaction");
    state.record_transaction(tx.clone());
    state.metrics.record_transaction_ingested();
    Ok(Json(tx))
}

pub async fn get_transaction(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> Result<Json<NormalizedTransaction>, ApiError> {
    state
        .transactions
        .read()
        .unwrap()
        .get(&tx_hash)
        .cloned()
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("transaction not found: {tx_hash}")))
}

#[derive(Debug, Serialize)]
pub struct EvaluationResult {
    pub triggered_rules: Vec<TriggeredRule>,
    pub anomaly_score: f64,
    pub severity: stellartrace_common::Severity,
    pub alert: Option<Alert>,
}

/// Evaluates an already-ingested transaction (by hash) against the rules
/// engine using the account's stored history and the current
/// flagged-accounts set, creating an alert if anything triggers.
pub async fn evaluate_transaction(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> Result<Json<EvaluationResult>, ApiError> {
    let tx = state
        .transactions
        .read()
        .unwrap()
        .get(&tx_hash)
        .cloned()
        .ok_or_else(|| ApiError::not_found(format!("transaction not found: {tx_hash}")))?;

    let result = run_evaluation(&state, &tx);
    Ok(Json(result))
}

/// Ingests a transaction inline and immediately evaluates it — the
/// common path for the ingestion pipeline feeding the engine in
/// real time, without a separate round trip.
pub async fn ingest_and_evaluate(
    State(state): State<Arc<AppState>>,
    Json(tx): Json<NormalizedTransaction>,
) -> Result<Json<EvaluationResult>, ApiError> {
    let result = run_evaluation(&state, &tx);
    state.record_transaction(tx);
    state.metrics.record_transaction_ingested();
    Ok(Json(result))
}

fn run_evaluation(state: &AppState, tx: &NormalizedTransaction) -> EvaluationResult {
    let history: Vec<NormalizedTransaction> = state
        .history_for(&tx.source_account)
        .into_iter()
        .filter(|h| h.tx_hash != tx.tx_hash)
        .collect();
    let flagged: HashSet<String> = state.flagged_accounts_snapshot();
    let ctx = RuleContext::new(history, flagged);
    let config = state.rules_config.read().unwrap().clone();

    let triggered_rules = state.rules_engine.evaluate(tx, &ctx, &config);
    let weights = stellartrace_scoring::ScoringWeights::default();
    let anomaly_score = stellartrace_scoring::compute_score(&triggered_rules, &weights);
    let severity = stellartrace_scoring::severity_for_score(anomaly_score);

    let alert = if !triggered_rules.is_empty() {
        state.metrics.record_alert_created();
        Some(
            state
                .alerts
                .create_alert(tx, triggered_rules.clone(), anomaly_score, severity),
        )
    } else {
        None
    };

    EvaluationResult {
        triggered_rules,
        anomaly_score,
        severity,
        alert,
    }
}

// ---------- Alerts ----------

#[derive(Debug, Deserialize)]
pub struct ListAlertsQuery {
    pub status: Option<String>,
}

pub async fn list_alerts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ListAlertsQuery>,
) -> Result<Json<Vec<Alert>>, ApiError> {
    let alerts = match q.status.as_deref() {
        Some("open") => state.alerts.list_by_status(InvestigationStatus::Open),
        Some("under_investigation") => state
            .alerts
            .list_by_status(InvestigationStatus::UnderInvestigation),
        Some("escalated") => state.alerts.list_by_status(InvestigationStatus::Escalated),
        Some("dismissed") => state.alerts.list_by_status(InvestigationStatus::Dismissed),
        Some("confirmed_suspicious") => state
            .alerts
            .list_by_status(InvestigationStatus::ConfirmedSuspicious),
        Some(other) => {
            return Err(ApiError::bad_request(format!(
                "unknown status filter: {other}"
            )))
        }
        None => state.alerts.list(),
    };
    Ok(Json(alerts))
}

pub async fn get_alert(
    State(state): State<Arc<AppState>>,
    Path(alert_id): Path<Uuid>,
) -> Result<Json<Alert>, ApiError> {
    state
        .alerts
        .get(alert_id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("alert not found: {alert_id}")))
}

// ---------- AI investigation context (advisory only) ----------

/// Retrieves (generating if not yet cached) the AI advisory
/// recommendation for an alert. The response is explicitly labeled as
/// advisory and can never itself change alert status — only
/// `submit_decision` below can do that, and it requires investigator
/// authorization.
pub async fn get_ai_investigation(
    State(state): State<Arc<AppState>>,
    Path(alert_id): Path<Uuid>,
) -> Result<Json<AiRecommendation>, ApiError> {
    let alert = state
        .alerts
        .get(alert_id)
        .ok_or_else(|| ApiError::not_found(format!("alert not found: {alert_id}")))?;

    let history = state.history_for(&alert.source_account);
    let history_summary = format!(
        "{} prior transactions on record for source account {} (not shown in full; aggregated only).",
        history.len(),
        alert.source_account
    );
    let context = build_context(&alert, history_summary);

    state.audit.append(
        alert_id,
        alert.tx_hash.clone(),
        AuditEventKind::AiContextSent {
            context_summary: format!(
                "{} triggered rule reasons sent",
                context.triggered_rule_reasons.len()
            ),
        },
    );

    let recommendation = state.advisor.investigate(&context).await;
    state
        .metrics
        .record_ai_recommendation(recommendation.model_available);

    state.audit.append(
        alert_id,
        alert.tx_hash.clone(),
        AuditEventKind::AiRecommendationReceived {
            confidence: recommendation.confidence,
            model_available: recommendation.model_available,
        },
    );

    state
        .ai_recommendations
        .write()
        .unwrap()
        .insert(alert_id, recommendation.clone());

    tracing::info!(
        alert_id = %alert_id,
        model_available = recommendation.model_available,
        "AI advisory recommendation generated (advisory only, not a decision)"
    );

    Ok(Json(recommendation))
}

// ---------- Human investigator decisions ----------

#[derive(Debug, Deserialize)]
pub struct DecisionRequest {
    pub decision: InvestigationStatus,
    #[serde(default)]
    pub notes: String,
}

/// The only endpoint capable of changing an alert's status. Requires an
/// authenticated investigator or admin — enforced here via the `AuthUser`
/// extractor and an explicit role check, not merely by the client's
/// intent.
pub async fn submit_decision(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(alert_id): Path<Uuid>,
    Json(req): Json<DecisionRequest>,
) -> Result<Json<Alert>, ApiError> {
    if !auth.role.can_decide() {
        return Err(ApiError::forbidden(
            "only an authorized investigator or admin may submit a final decision",
        ));
    }

    let decision = InvestigatorDecision {
        alert_id,
        investigator_id: auth.investigator_id.clone(),
        decision: req.decision,
        notes: req.notes,
        decided_at: chrono::Utc::now(),
    };

    let updated = state.alerts.apply_investigator_decision(decision)?;
    state.metrics.record_investigator_decision();

    if updated.status == InvestigationStatus::ConfirmedSuspicious {
        let mut flagged = state.flagged_accounts.write().unwrap();
        flagged.insert(updated.source_account.clone());
        if let Some(dest) = &updated.destination_account {
            flagged.insert(dest.clone());
        }
    }

    tracing::info!(
        alert_id = %alert_id,
        investigator_id = %auth.investigator_id,
        decision = ?req.decision,
        "investigator decision recorded"
    );

    Ok(Json(updated))
}

// ---------- Audit ----------

pub async fn get_alert_audit(
    State(state): State<Arc<AppState>>,
    Path(alert_id): Path<Uuid>,
) -> Json<Vec<AuditRecord>> {
    Json(state.audit.for_alert(alert_id))
}

#[derive(Debug, Serialize)]
pub struct IntegrityResponse {
    pub intact: bool,
    pub detail: Option<String>,
}

pub async fn verify_audit_integrity(State(state): State<Arc<AppState>>) -> Json<IntegrityResponse> {
    match state.audit.verify_integrity() {
        Ok(()) => Json(IntegrityResponse {
            intact: true,
            detail: None,
        }),
        Err(e) => Json(IntegrityResponse {
            intact: false,
            detail: Some(e.to_string()),
        }),
    }
}
