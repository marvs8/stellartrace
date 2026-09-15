use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use stellartrace_ai_advisor::{AdvisorService, ClaudeAdvisor, FallbackAdvisor};
use stellartrace_alerts::AlertManager;
use stellartrace_audit::AuditLog;
use stellartrace_rules_engine::{RulesConfig, RulesEngine};

use stellartrace_api::auth::AuthRegistry;
use stellartrace_api::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let audit = Arc::new(AuditLog::new());
    let alerts = Arc::new(AlertManager::new(audit.clone()));

    // If the AI service is unavailable or unconfigured, fall back
    // transparently — monitoring and rules evaluation never depend on it.
    let advisor = match ClaudeAdvisor::from_env() {
        Ok(claude) => {
            tracing::info!("AI advisor configured with Claude backend");
            AdvisorService::new(claude)
        }
        Err(_) => {
            tracing::warn!(
                "ANTHROPIC_API_KEY not set; AI advisory endpoint will return \
                 fallback (model_available=false) responses only"
            );
            AdvisorService::new(FallbackAdvisor)
        }
    };

    let state = Arc::new(AppState {
        rules_engine: RulesEngine::with_default_rules(),
        rules_config: RwLock::new(RulesConfig::default()),
        alerts,
        audit,
        advisor,
        auth: AuthRegistry::from_env(),
        tx_history: RwLock::new(HashMap::new()),
        transactions: RwLock::new(HashMap::new()),
        flagged_accounts: RwLock::new(HashSet::new()),
        ai_recommendations: RwLock::new(HashMap::new()),
    });

    let app = stellartrace_api::build_router(state);

    let addr = std::env::var("STELLARTRACE_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    tracing::info!(%addr, "starting StellarTrace API");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
