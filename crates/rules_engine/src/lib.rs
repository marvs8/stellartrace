//! Deterministic, explainable fraud-detection rules engine.
//!
//! Every rule is a pure function of `(transaction, context, config)` — no
//! hidden state, no randomness, no ML. Given the same inputs it always
//! produces the same output, and every trigger carries a structured
//! `evidence` map plus a human-readable `reason` string so an investigator
//! (or an auditor months later) can see exactly why it fired without
//! trusting a black box.
//!
//! New rules are added by implementing the `Rule` trait and registering
//! them in `RulesEngine::with_default_rules` (or a custom set) — the
//! engine itself never needs to change.

pub mod config;
pub mod context;
pub mod rules;

pub use config::RulesConfig;
pub use context::RuleContext;

use stellartrace_common::{NormalizedTransaction, TriggeredRule};

/// A single, self-contained detection rule.
pub trait Rule: Send + Sync {
    /// Stable identifier, e.g. `"large_transfer"`. Used in audit records
    /// and dashboards, so once shipped it should not change meaning.
    fn id(&self) -> &'static str;

    /// Evaluate the rule against one transaction. Returns `Some` with a
    /// fully explained trigger, or `None` if the rule did not fire.
    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule>;
}

/// Runs a transaction through every registered rule and collects all
/// triggers. Order of rules does not affect the result set (each rule is
/// independent), which is what keeps the engine deterministic and easy to
/// extend.
pub struct RulesEngine {
    rules: Vec<Box<dyn Rule>>,
}

impl RulesEngine {
    pub fn new(rules: Vec<Box<dyn Rule>>) -> Self {
        Self { rules }
    }

    pub fn with_default_rules() -> Self {
        Self::new(rules::default_rule_set())
    }

    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Evaluate all rules and return every trigger, sorted by rule id for
    /// deterministic output ordering (important for reproducible audit
    /// records and tests).
    pub fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Vec<TriggeredRule> {
        let mut triggered: Vec<TriggeredRule> = self
            .rules
            .iter()
            .filter_map(|rule| {
                let result = rule.evaluate(tx, ctx, config);
                if let Some(t) = &result {
                    tracing::info!(
                        tx_hash = %tx.tx_hash,
                        rule_id = %t.rule_id,
                        severity = ?t.severity,
                        "rule triggered"
                    );
                }
                result
            })
            .collect();
        triggered.sort_by(|a, b| a.rule_id.cmp(&b.rule_id));
        triggered
    }
}
