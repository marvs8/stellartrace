//! End-to-end integration tests against the real router: ingest a
//! transaction, evaluate it, fetch AI advisory context, and confirm that
//! only an authenticated investigator/admin can submit a decision — and
//! that the AI recommendation step never touches alert status.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use stellartrace_ai_advisor::{AdvisorService, FallbackAdvisor};
use stellartrace_alerts::AlertManager;
use stellartrace_audit::AuditLog;
use stellartrace_common::{Alert, InvestigationStatus};
use stellartrace_rules_engine::{RulesConfig, RulesEngine};
use tower::ServiceExt;

use stellartrace_api::auth::AuthRegistry;
use stellartrace_api::state::AppState;

fn test_app() -> axum::Router {
    std::env::set_var(
        "STELLARTRACE_API_TOKENS",
        "inv-token:investigator:inv-1,viewer-token:viewer:view-1",
    );
    let audit = Arc::new(AuditLog::new());
    let alerts = Arc::new(AlertManager::new(audit.clone()));
    let state = Arc::new(AppState {
        metrics: stellartrace_api::metrics::Metrics::default(),
        rules_engine: RulesEngine::with_default_rules(),
        rules_config: RwLock::new(RulesConfig::default()),
        alerts,
        audit,
        advisor: AdvisorService::new(FallbackAdvisor),
        auth: AuthRegistry::from_env(),
        tx_history: RwLock::new(HashMap::new()),
        transactions: RwLock::new(HashMap::new()),
        flagged_accounts: RwLock::new(HashSet::new()),
        ai_recommendations: RwLock::new(HashMap::new()),
    });
    stellartrace_api::build_router(state)
}

fn large_tx_json() -> serde_json::Value {
    serde_json::json!({
        "tx_hash": "deadbeef01",
        "ledger_sequence": 100,
        "source_account": "GALICE",
        "destination_account": "GBOB",
        "asset": { "type": "native" },
        "amount": "50000",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "is_soroban_invocation": false,
        "contract_id": null,
        "metadata": {}
    })
}

async fn body_json(response: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_check_ok() {
    let app = test_app();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn ingest_evaluate_and_create_alert_flow() {
    let app = test_app();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(large_tx_json().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert!(
        json["alert"].is_object(),
        "large transfer should create an alert: {json:?}"
    );
    assert!(json["triggered_rules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["rule_id"] == "large_transfer"));
}

#[tokio::test]
async fn ai_investigation_endpoint_never_changes_alert_status() {
    let app = test_app();

    let eval_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(large_tx_json().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let eval_json = body_json(eval_response).await;
    let alert_id = eval_json["alert"]["alert_id"].as_str().unwrap().to_string();
    assert_eq!(eval_json["alert"]["status"], "open");

    // Call the AI advisory endpoint repeatedly.
    for _ in 0..3 {
        let ai_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/alerts/{alert_id}/ai-investigation"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ai_response.status(), StatusCode::OK);
        let rec = body_json(ai_response).await;
        assert!(rec["label"].as_str().unwrap().contains("ADVISORY"));
    }

    // Status must still be "open" — no AI call can have changed it.
    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/alerts/{alert_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let alert: Alert = serde_json::from_value(body_json(get_response).await).unwrap();
    assert_eq!(alert.status, InvestigationStatus::Open);
}

#[tokio::test]
async fn decision_requires_authorization() {
    let app = test_app();

    let eval_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(large_tx_json().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let eval_json = body_json(eval_response).await;
    let alert_id = eval_json["alert"]["alert_id"].as_str().unwrap().to_string();

    // No token at all -> unauthorized.
    let unauth = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{alert_id}/decision"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"decision": "dismissed", "notes": "n/a"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

    // Viewer role -> forbidden (cannot decide).
    let forbidden = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{alert_id}/decision"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer viewer-token")
                .body(Body::from(
                    serde_json::json!({"decision": "dismissed", "notes": "n/a"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

    // Investigator role -> allowed.
    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{alert_id}/decision"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer inv-token")
                .body(Body::from(
                    serde_json::json!({"decision": "confirmed_suspicious", "notes": "verified off-chain"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    let updated_alert = body_json(allowed).await;
    assert_eq!(updated_alert["status"], "confirmed_suspicious");
}

#[tokio::test]
async fn audit_trail_records_every_step_and_is_intact() {
    let app = test_app();

    let eval_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(large_tx_json().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let eval_json = body_json(eval_response).await;
    let alert_id = eval_json["alert"]["alert_id"].as_str().unwrap().to_string();

    app.clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/alerts/{alert_id}/ai-investigation"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{alert_id}/decision"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer inv-token")
                .body(Body::from(
                    serde_json::json!({"decision": "escalated", "notes": "sending to compliance"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let audit_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/alerts/{alert_id}/audit"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let audit_records = body_json(audit_response).await;
    let events: Vec<String> = audit_records
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["event"]["event_type"].as_str().unwrap().to_string())
        .collect();

    assert!(events.contains(&"alert_created".to_string()));
    assert!(events.contains(&"rules_triggered".to_string()));
    assert!(events.contains(&"ai_context_sent".to_string()));
    assert!(events.contains(&"ai_recommendation_received".to_string()));
    assert!(events.contains(&"investigator_decision".to_string()));
    assert!(events.contains(&"status_changed".to_string()));

    let verify_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/audit/verify")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let verify_json = body_json(verify_response).await;
    assert_eq!(verify_json["intact"], true);
}
