//! Historical context a rule needs to evaluate a transaction. This is
//! supplied by the caller (typically the alert-management layer backed by
//! a transaction store) so the rules engine itself stays free of storage
//! concerns and easy to unit test with hand-built fixtures.

use chrono::{DateTime, Utc};
use std::collections::HashSet;
use stellartrace_common::NormalizedTransaction;

#[derive(Debug, Clone, Default)]
pub struct RuleContext {
    /// Prior transactions involving the current transaction's source
    /// account, most-recent-first, bounded to whatever window the caller
    /// deems useful (the rules apply their own time-window filtering on
    /// top of this).
    pub account_history: Vec<NormalizedTransaction>,

    /// Accounts previously flagged (e.g. confirmed suspicious in a prior
    /// investigation, or synced from an on-chain flagged-account
    /// registry). Interaction with any of these is itself a signal.
    pub flagged_accounts: HashSet<String>,
}

impl RuleContext {
    pub fn new(account_history: Vec<NormalizedTransaction>, flagged_accounts: HashSet<String>) -> Self {
        Self { account_history, flagged_accounts }
    }

    /// Transactions in `account_history` that occurred within `window_secs`
    /// before `at`.
    pub fn within_window(&self, at: DateTime<Utc>, window_secs: i64) -> Vec<&NormalizedTransaction> {
        self.account_history
            .iter()
            .filter(|tx| (at - tx.timestamp).num_seconds().abs() <= window_secs && tx.timestamp <= at)
            .collect()
    }
}
