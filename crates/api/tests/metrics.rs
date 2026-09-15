//! Confirms the /metrics endpoint reflects real activity end-to-end
//! through the router, not just the unit-level counter logic.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use stellartrace_ai_advisor::{AdvisorService, FallbackAdvisor};
use stellartrace_alerts::AlertManager;
use stellartrace_audit::AuditLog;
use stellartrace_rules_engine::{RulesConfig, RulesEngine};
use tower::ServiceExt;

use stellartrace_api::auth::AuthRegistry;
use stellartrace_api::state::AppState;

fn test_app() -> axum::Router {
    std::env::set_var("STELLARTRACE_API_TOKENS", "inv-token:investigator:inv-1");
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

async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[tokio::test]
async fn metrics_endpoint_reflects_ingestion_and_decision_activity() {
    let app = test_app();

    let before = app
        .clone()
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);
    let before_text = body_text(before).await;
    assert!(before_text.contains("stellartrace_transactions_ingested_total 0"));

    let tx = serde_json::json!({
        "tx_hash": "metrics-tx-1",
        "ledger_sequence": 1,
        "source_account": "GALICE",
        "destination_account": "GBOB",
        "asset": { "type": "native" },
        "amount": "50000",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "is_soroban_invocation": false,
        "contract_id": null,
        "metadata": {}
    });

    let eval = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/transactions")
                .header("content-type", "application/json")
                .body(Body::from(tx.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let eval_json: serde_json::Value =
        serde_json::from_slice(&axum::body::to_bytes(eval.into_body(), usize::MAX).await.unwrap()).unwrap();
    let alert_id = eval_json["alert"]["alert_id"].as_str().unwrap().to_string();

    app.clone()
        .oneshot(Request::builder().uri(format!("/api/alerts/{alert_id}/ai-investigation")).body(Body::empty()).unwrap())
        .await
        .unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/alerts/{alert_id}/decision"))
                .header("content-type", "application/json")
                .header("authorization", "Bearer inv-token")
                .body(Body::from(serde_json::json!({"decision": "dismissed", "notes": "reviewed"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let after = app
        .clone()
        .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let after_text = body_text(after).await;

    assert!(after_text.contains("stellartrace_transactions_ingested_total 1"));
    assert!(after_text.contains("stellartrace_alerts_created_total 1"));
    assert!(after_text.contains("stellartrace_ai_recommendations_total 1"));
    assert!(after_text.contains("stellartrace_ai_fallback_total 1")); // FallbackAdvisor is always unavailable
    assert!(after_text.contains("stellartrace_investigator_decisions_total 1"));
}
