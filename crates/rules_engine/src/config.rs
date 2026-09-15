//! Configurable thresholds for the rules engine. Kept as plain, serde-able
//! data so operators can tune detection sensitivity (per the spec's
//! "configurable threshold violations" rule) without recompiling, e.g. by
//! loading this from a config file or the API's admin endpoints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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

    /// Gap, in seconds, of inactivity after which a subsequent transaction
    /// above `dormant_reactivation_min_amount` is treated as a suspicious
    /// reactivation (e.g. a compromised or sold dormant account).
    pub dormant_reactivation_gap_secs: i64,
    pub dormant_reactivation_min_amount: f64,

    /// Window, in seconds, within which funds returning to a prior sender
    /// are treated as a possible round-trip / wash-trading pattern.
    pub wash_trading_window_secs: i64,

    /// An account with this many or fewer prior transactions on record is
    /// considered "new" for the purposes of the new-account high-value
    /// outflow rule.
    pub new_account_history_threshold: usize,
    pub new_account_outflow_threshold: f64,

    /// Window and distinct-asset count used to detect rapid conversion
    /// across multiple assets (a layering pattern).
    pub cross_asset_conversion_window_secs: i64,
    pub cross_asset_conversion_count_threshold: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read rules config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse rules config TOML: {0}")]
    Parse(#[from] toml::de::Error),
}

/// Loads a `RulesConfig` from a TOML file. Callers should treat any error
/// here as non-fatal and fall back to `RulesConfig::default()` with a
/// logged warning — a misconfigured threshold file must never prevent the
/// service from starting (see docs/configuration.md).
pub fn load_from_file(path: impl AsRef<Path>) -> Result<RulesConfig, ConfigError> {
    let contents = std::fs::read_to_string(path)?;
    let config: RulesConfig = toml::from_str(&contents)?;
    Ok(config)
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
            dormant_reactivation_gap_secs: 30 * 24 * 60 * 60,
            dormant_reactivation_min_amount: 1_000.0,
            wash_trading_window_secs: 3_600,
            new_account_history_threshold: 2,
            new_account_outflow_threshold: 5_000.0,
            cross_asset_conversion_window_secs: 600,
            cross_asset_conversion_count_threshold: 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_file_parses_valid_toml() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "stellartrace_test_config_{}.toml",
            uuid_like_suffix()
        ));
        std::fs::write(&path, "large_transfer_threshold = 42.0\n").unwrap();
        let config = load_from_file(&path).unwrap();
        assert_eq!(config.large_transfer_threshold, 42.0);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_from_file_errors_on_missing_file() {
        let result = load_from_file("/nonexistent/path/does-not-exist.toml");
        assert!(result.is_err());
    }

    #[test]
    fn load_from_file_errors_on_malformed_toml() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "stellartrace_test_bad_config_{}.toml",
            uuid_like_suffix()
        ));
        std::fs::write(&path, "this is not valid toml {{{").unwrap();
        let result = load_from_file(&path);
        assert!(result.is_err());
        std::fs::remove_file(&path).ok();
    }

    fn uuid_like_suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
