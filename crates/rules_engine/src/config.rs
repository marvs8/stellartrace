//! Configurable thresholds for the rules engine. Kept as plain, serde-able
//! data so operators can tune detection sensitivity (per the spec's
//! "configurable threshold violations" rule) without recompiling, e.g. by
//! loading this from a config file or the API's admin endpoints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesConfig {
    /// Amount (in the asset's display units) above which a single
    /// transfer is considered unusually large.
    pub large_transfer_threshold: f64,

    /// Window, in seconds, within which repeated transactions to/from the
    /// same counterparty are considered suspicious.
    pub repeated_tx_window_secs: i64,
    /// Number of transactions to the same counterparty within the window
    /// that triggers the repeated-transaction rule.
    pub repeated_tx_count_threshold: usize,

    /// Window, in seconds, used to measure overall account activity
    /// frequency.
    pub frequency_window_secs: i64,
    /// Number of transactions from one account within the frequency
    /// window that is considered abnormal.
    pub frequency_count_threshold: usize,

    /// A transaction whose amount is this many multiples of the account's
    /// historical average (over the same window) is flagged as unusual
    /// asset movement. Requires at least `unusual_asset_min_history` prior
    /// transactions to avoid flagging a thin history.
    pub unusual_asset_movement_multiplier: f64,
    pub unusual_asset_min_history: usize,

    /// Named, operator-configurable numeric thresholds for the generic
    /// "configurable threshold violation" rule, keyed by threshold name
    /// (e.g. "max_single_asset_exposure"). This lets new limits be added
    /// via config alone, no code change, for simple magnitude checks.
    pub custom_thresholds: HashMap<String, f64>,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            large_transfer_threshold: 10_000.0,
            repeated_tx_window_secs: 60,
            repeated_tx_count_threshold: 3,
            frequency_window_secs: 300,
            frequency_count_threshold: 10,
            unusual_asset_movement_multiplier: 5.0,
            unusual_asset_min_history: 3,
            custom_thresholds: HashMap::new(),
        }
    }
}
