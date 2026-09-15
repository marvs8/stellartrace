//! Shared application state wiring every layer together. This is the one
//! place that composes ingestion history, the rules engine, scoring,
//! alert management, the audit log, and the AI advisor — each of those
//! stays a separate crate with no knowledge of the others.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use stellartrace_ai_advisor::AdvisorService;
use stellartrace_alerts::AlertManager;
use stellartrace_audit::AuditLog;
use stellartrace_common::{AiRecommendation, NormalizedTransaction};
use stellartrace_rules_engine::{RulesConfig, RulesEngine};

use crate::auth::AuthRegistry;
use crate::metrics::Metrics;

pub struct AppState {
    pub metrics: Metrics,
    pub rules_engine: RulesEngine,
    pub rules_config: RwLock<RulesConfig>,
    pub alerts: Arc<AlertManager>,
    pub audit: Arc<AuditLog>,
    pub advisor: AdvisorService,
    pub auth: AuthRegistry,

    /// Recent transactions per account, indexed under both the source and
    /// destination account so rules that need bidirectional context (e.g.
    /// round-trip / wash-trading detection) can see money flowing both in
    /// and out of an account, not just out. Bounded per account to avoid
    /// unbounded growth in this reference (in-memory) implementation.
    pub tx_history: RwLock<HashMap<String, Vec<NormalizedTransaction>>>,
    /// All ingested transactions keyed by hash, for lookup/evaluate-by-hash
    /// endpoints.
    pub transactions: RwLock<HashMap<String, NormalizedTransaction>>,
    /// Accounts previously confirmed suspicious (or otherwise flagged),
    /// consulted by `FlaggedAccountInteractionRule`.
    pub flagged_accounts: RwLock<HashSet<String>>,
    /// Latest AI recommendation produced per alert.
    pub ai_recommendations: RwLock<HashMap<uuid::Uuid, AiRecommendation>>,
}

const MAX_HISTORY_PER_ACCOUNT: usize = 500;

impl AppState {
    pub fn record_transaction(&self, tx: NormalizedTransaction) {
        self.transactions.write().unwrap().insert(tx.tx_hash.clone(), tx.clone());
        let mut history = self.tx_history.write().unwrap();

        Self::push_bounded(&mut history, tx.source_account.clone(), tx.clone());
        if let Some(dest) = tx.destination_account.clone() {
            if dest != tx.source_account {
                Self::push_bounded(&mut history, dest, tx);
            }
        }
    }

    fn push_bounded(history: &mut HashMap<String, Vec<NormalizedTransaction>>, account: String, tx: NormalizedTransaction) {
        let entry = history.entry(account).or_default();
        entry.push(tx);
        if entry.len() > MAX_HISTORY_PER_ACCOUNT {
            let excess = entry.len() - MAX_HISTORY_PER_ACCOUNT;
            entry.drain(0..excess);
        }
    }

    pub fn history_for(&self, account: &str) -> Vec<NormalizedTransaction> {
        self.tx_history.read().unwrap().get(account).cloned().unwrap_or_default()
    }

    pub fn flagged_accounts_snapshot(&self) -> HashSet<String> {
        self.flagged_accounts.read().unwrap().clone()
    }
}
