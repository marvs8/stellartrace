pub mod auth;
pub mod error;
pub mod handlers;
pub mod metrics;
pub mod persistence;
pub mod state;

use axum::routing::{get, post};
use axum::Router;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use state::AppState;

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(handlers::health))
        .route("/metrics", get(handlers::metrics))
        .route("/api/transactions", post(handlers::ingest_and_evaluate))
        .route("/api/transactions/:tx_hash", get(handlers::get_transaction))
        .route("/api/transactions/:tx_hash/evaluate", post(handlers::evaluate_transaction))
        .route("/api/alerts", get(handlers::list_alerts))
        .route("/api/alerts/:alert_id", get(handlers::get_alert))
        .route("/api/alerts/:alert_id/ai-investigation", get(handlers::get_ai_investigation))
        .route("/api/alerts/:alert_id/decision", post(handlers::submit_decision))
        .route("/api/alerts/:alert_id/audit", get(handlers::get_alert_audit))
        .route("/api/audit/verify", get(handlers::verify_audit_integrity))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
