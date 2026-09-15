//! Concrete rule implementations. Each one is small, independent, and
//! documents its own logic — that independence is what lets new rules be
//! added without touching the engine or any existing rule.

use crate::config::RulesConfig;
use crate::context::RuleContext;
use crate::Rule;
use std::collections::HashMap;
use stellartrace_common::{NormalizedTransaction, Severity, TriggeredRule};

pub fn default_rule_set() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(LargeTransferRule),
        Box::new(RepeatedTransactionRule),
        Box::new(AbnormalFrequencyRule),
        Box::new(FlaggedAccountInteractionRule),
        Box::new(UnusualAssetMovementRule),
        Box::new(ConfigurableThresholdRule),
        Box::new(DormantAccountReactivationRule),
        Box::new(RoundTripWashTradingRule),
        Box::new(NewAccountHighValueOutflowRule),
        Box::new(CrossAssetRapidConversionRule),
    ]
}

/// Flags any single transfer whose amount exceeds the configured
/// large-transfer threshold.
pub struct LargeTransferRule;
impl Rule for LargeTransferRule {
    fn id(&self) -> &'static str {
        "large_transfer"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        _ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let amount = tx.amount_f64();
        if amount > config.large_transfer_threshold {
            let mut evidence = HashMap::new();
            evidence.insert("amount".into(), tx.amount.clone());
            evidence.insert(
                "threshold".into(),
                config.large_transfer_threshold.to_string(),
            );
            evidence.insert("asset".into(), tx.asset.to_string());
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Large Transfer".into(),
                severity: if amount > config.large_transfer_threshold * 10.0 {
                    Severity::Critical
                } else {
                    Severity::High
                },
                reason: format!(
                    "Transfer of {} {} exceeds the large-transfer threshold of {}",
                    tx.amount, tx.asset, config.large_transfer_threshold
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags repeated transactions between the same source/destination pair
/// within a short window — a common structuring or automated-drain
/// pattern.
pub struct RepeatedTransactionRule;
impl Rule for RepeatedTransactionRule {
    fn id(&self) -> &'static str {
        "repeated_transaction"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let window = ctx.within_window(tx.timestamp, config.repeated_tx_window_secs);
        let matching = window
            .iter()
            .filter(|h| h.destination_account == tx.destination_account)
            .count()
            + 1; // include the current transaction

        if matching >= config.repeated_tx_count_threshold {
            let mut evidence = HashMap::new();
            evidence.insert("count".into(), matching.to_string());
            evidence.insert(
                "threshold".into(),
                config.repeated_tx_count_threshold.to_string(),
            );
            evidence.insert(
                "window_secs".into(),
                config.repeated_tx_window_secs.to_string(),
            );
            if let Some(dest) = &tx.destination_account {
                evidence.insert("destination_account".into(), dest.clone());
            }
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Repeated Transaction".into(),
                severity: Severity::Medium,
                reason: format!(
                    "{} transactions to the same destination within {} seconds (threshold: {})",
                    matching, config.repeated_tx_window_secs, config.repeated_tx_count_threshold
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags an account transacting far more frequently than the configured
/// baseline in a rolling window (e.g. bot-driven activity).
pub struct AbnormalFrequencyRule;
impl Rule for AbnormalFrequencyRule {
    fn id(&self) -> &'static str {
        "abnormal_frequency"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let window = ctx.within_window(tx.timestamp, config.frequency_window_secs);
        let count = window.len() + 1;

        if count >= config.frequency_count_threshold {
            let mut evidence = HashMap::new();
            evidence.insert("count".into(), count.to_string());
            evidence.insert(
                "threshold".into(),
                config.frequency_count_threshold.to_string(),
            );
            evidence.insert(
                "window_secs".into(),
                config.frequency_window_secs.to_string(),
            );
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Abnormal Transaction Frequency".into(),
                severity: Severity::Medium,
                reason: format!(
                    "Account {} made {} transactions within {} seconds (threshold: {})",
                    tx.source_account,
                    count,
                    config.frequency_window_secs,
                    config.frequency_count_threshold
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags any transaction touching an account already on the
/// flagged-accounts list (from prior confirmed investigations or an
/// on-chain flagged-account registry).
pub struct FlaggedAccountInteractionRule;
impl Rule for FlaggedAccountInteractionRule {
    fn id(&self) -> &'static str {
        "flagged_account_interaction"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        _config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let source_flagged = ctx.flagged_accounts.contains(&tx.source_account);
        let dest_flagged = tx
            .destination_account
            .as_ref()
            .map(|d| ctx.flagged_accounts.contains(d))
            .unwrap_or(false);

        if source_flagged || dest_flagged {
            let mut evidence = HashMap::new();
            evidence.insert("source_flagged".into(), source_flagged.to_string());
            evidence.insert("destination_flagged".into(), dest_flagged.to_string());
            evidence.insert("source_account".into(), tx.source_account.clone());
            if let Some(dest) = &tx.destination_account {
                evidence.insert("destination_account".into(), dest.clone());
            }
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Flagged Account Interaction".into(),
                severity: Severity::High,
                reason: format!(
                    "Transaction involves a previously flagged account ({})",
                    if source_flagged {
                        "source"
                    } else {
                        "destination"
                    }
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags a transaction whose amount deviates sharply from the account's
/// own historical average, indicating unusual asset movement even when
/// the absolute amount isn't large enough to trip `LargeTransferRule`.
pub struct UnusualAssetMovementRule;
impl Rule for UnusualAssetMovementRule {
    fn id(&self) -> &'static str {
        "unusual_asset_movement"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let same_asset_history: Vec<f64> = ctx
            .account_history
            .iter()
            .filter(|h| h.asset == tx.asset)
            .map(|h| h.amount_f64())
            .collect();

        if same_asset_history.len() < config.unusual_asset_min_history {
            return None;
        }

        let avg = same_asset_history.iter().sum::<f64>() / same_asset_history.len() as f64;
        if avg <= 0.0 {
            return None;
        }

        let amount = tx.amount_f64();
        let ratio = amount / avg;
        if ratio >= config.unusual_asset_movement_multiplier {
            let mut evidence = HashMap::new();
            evidence.insert("amount".into(), tx.amount.clone());
            evidence.insert("historical_average".into(), format!("{avg:.7}"));
            evidence.insert("ratio".into(), format!("{ratio:.2}"));
            evidence.insert(
                "multiplier_threshold".into(),
                config.unusual_asset_movement_multiplier.to_string(),
            );
            evidence.insert("history_size".into(), same_asset_history.len().to_string());
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Unusual Asset Movement".into(),
                severity: Severity::Medium,
                reason: format!(
                    "Amount {} is {:.1}x this account's historical average of {:.7} for {} (threshold: {}x)",
                    tx.amount, ratio, avg, tx.asset, config.unusual_asset_movement_multiplier
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Evaluates operator-configurable named thresholds against transaction
/// metadata/amount. This is the extension point the spec calls out for
/// "configurable threshold violations" that don't warrant a bespoke rule.
pub struct ConfigurableThresholdRule;
impl Rule for ConfigurableThresholdRule {
    fn id(&self) -> &'static str {
        "configurable_threshold"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        _ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let amount = tx.amount_f64();
        for (name, threshold) in &config.custom_thresholds {
            if amount > *threshold {
                let mut evidence = HashMap::new();
                evidence.insert("threshold_name".into(), name.clone());
                evidence.insert("threshold_value".into(), threshold.to_string());
                evidence.insert("amount".into(), tx.amount.clone());
                return Some(TriggeredRule {
                    rule_id: self.id().into(),
                    rule_name: format!("Configurable Threshold: {name}"),
                    severity: Severity::Low,
                    reason: format!(
                        "Amount {} exceeds configured threshold '{}' ({})",
                        tx.amount, name, threshold
                    ),
                    evidence,
                });
            }
        }
        None
    }
}

/// Flags a transaction from an account that had no activity for an
/// extended gap and then suddenly moves a significant amount — a pattern
/// consistent with a compromised, sold, or otherwise hijacked dormant
/// account being drained.
pub struct DormantAccountReactivationRule;
impl Rule for DormantAccountReactivationRule {
    fn id(&self) -> &'static str {
        "dormant_account_reactivation"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        if tx.amount_f64() < config.dormant_reactivation_min_amount {
            return None;
        }

        let last_prior = ctx
            .account_history
            .iter()
            .filter(|h| h.source_account == tx.source_account && h.timestamp < tx.timestamp)
            .max_by_key(|h| h.timestamp)?;

        let gap_secs = (tx.timestamp - last_prior.timestamp).num_seconds();
        if gap_secs >= config.dormant_reactivation_gap_secs {
            let mut evidence = HashMap::new();
            evidence.insert("gap_secs".into(), gap_secs.to_string());
            evidence.insert(
                "gap_threshold_secs".into(),
                config.dormant_reactivation_gap_secs.to_string(),
            );
            evidence.insert("amount".into(), tx.amount.clone());
            evidence.insert(
                "min_amount_threshold".into(),
                config.dormant_reactivation_min_amount.to_string(),
            );
            evidence.insert("last_prior_tx_hash".into(), last_prior.tx_hash.clone());
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Dormant Account Reactivation".into(),
                severity: Severity::High,
                reason: format!(
                    "Account was inactive for {} seconds (threshold: {}) before this {} {} transaction",
                    gap_secs, config.dormant_reactivation_gap_secs, tx.amount, tx.asset
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags a transaction that completes a round trip: the current
/// destination account previously sent funds back to the current source
/// account within a short window, a pattern associated with wash trading
/// or artificially inflating transaction volume.
pub struct RoundTripWashTradingRule;
impl Rule for RoundTripWashTradingRule {
    fn id(&self) -> &'static str {
        "round_trip_wash_trading"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let destination = tx.destination_account.as_ref()?;

        let window = ctx.within_window(tx.timestamp, config.wash_trading_window_secs);
        let round_trip = window.iter().find(|h| {
            &h.source_account == destination
                && h.destination_account.as_deref() == Some(tx.source_account.as_str())
                && h.timestamp < tx.timestamp
        });

        if let Some(prior) = round_trip {
            let mut evidence = HashMap::new();
            evidence.insert(
                "window_secs".into(),
                config.wash_trading_window_secs.to_string(),
            );
            evidence.insert("return_leg_tx_hash".into(), prior.tx_hash.clone());
            evidence.insert("return_leg_amount".into(), prior.amount.clone());
            evidence.insert("outbound_amount".into(), tx.amount.clone());
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Round-Trip Wash Trading".into(),
                severity: Severity::High,
                reason: format!(
                    "Funds round-tripped between {} and {} within {} seconds",
                    tx.source_account, destination, config.wash_trading_window_secs
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags a high-value outflow from an account with little to no prior
/// sending history — common in account-takeover or mule-account drains
/// where the account is used once immediately after being compromised.
pub struct NewAccountHighValueOutflowRule;
impl Rule for NewAccountHighValueOutflowRule {
    fn id(&self) -> &'static str {
        "new_account_high_value_outflow"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        tx.destination_account.as_ref()?;
        if tx.amount_f64() < config.new_account_outflow_threshold {
            return None;
        }

        let outgoing_count = ctx
            .account_history
            .iter()
            .filter(|h| h.source_account == tx.source_account && h.timestamp < tx.timestamp)
            .count();

        if outgoing_count <= config.new_account_history_threshold {
            let mut evidence = HashMap::new();
            evidence.insert("prior_outgoing_count".into(), outgoing_count.to_string());
            evidence.insert(
                "history_threshold".into(),
                config.new_account_history_threshold.to_string(),
            );
            evidence.insert("amount".into(), tx.amount.clone());
            evidence.insert(
                "amount_threshold".into(),
                config.new_account_outflow_threshold.to_string(),
            );
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "New Account High-Value Outflow".into(),
                severity: Severity::Medium,
                reason: format!(
                    "Account has only {} prior outgoing transaction(s) (threshold: {}) but is sending {} {}",
                    outgoing_count, config.new_account_history_threshold, tx.amount, tx.asset
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

/// Flags an account moving through several distinct assets in rapid
/// succession — a layering pattern often used to obscure the origin of
/// funds before a final cash-out.
pub struct CrossAssetRapidConversionRule;
impl Rule for CrossAssetRapidConversionRule {
    fn id(&self) -> &'static str {
        "cross_asset_rapid_conversion"
    }

    fn evaluate(
        &self,
        tx: &NormalizedTransaction,
        ctx: &RuleContext,
        config: &RulesConfig,
    ) -> Option<TriggeredRule> {
        let window = ctx.within_window(tx.timestamp, config.cross_asset_conversion_window_secs);
        let mut distinct_assets: std::collections::HashSet<&stellartrace_common::Asset> =
            window.iter().map(|h| &h.asset).collect();
        distinct_assets.insert(&tx.asset);

        if distinct_assets.len() >= config.cross_asset_conversion_count_threshold {
            let mut evidence = HashMap::new();
            evidence.insert(
                "distinct_asset_count".into(),
                distinct_assets.len().to_string(),
            );
            evidence.insert(
                "count_threshold".into(),
                config.cross_asset_conversion_count_threshold.to_string(),
            );
            evidence.insert(
                "window_secs".into(),
                config.cross_asset_conversion_window_secs.to_string(),
            );
            Some(TriggeredRule {
                rule_id: self.id().into(),
                rule_name: "Cross-Asset Rapid Conversion".into(),
                severity: Severity::Medium,
                reason: format!(
                    "Account touched {} distinct assets within {} seconds (threshold: {}), consistent with layering",
                    distinct_assets.len(), config.cross_asset_conversion_window_secs, config.cross_asset_conversion_count_threshold
                ),
                evidence,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use std::collections::{HashMap as Map, HashSet};
    use stellartrace_common::Asset;

    fn tx(amount: &str, ts_offset_secs: i64, dest: Option<&str>) -> NormalizedTransaction {
        tx_from("GALICE", amount, ts_offset_secs, dest, Asset::Native)
    }

    fn tx_from(
        source: &str,
        amount: &str,
        ts_offset_secs: i64,
        dest: Option<&str>,
        asset: Asset,
    ) -> NormalizedTransaction {
        NormalizedTransaction {
            tx_hash: format!("h-{source}-{ts_offset_secs}"),
            ledger_sequence: 1,
            source_account: source.into(),
            destination_account: dest.map(|s| s.to_string()),
            asset,
            amount: amount.into(),
            timestamp: Utc::now() + Duration::seconds(ts_offset_secs),
            is_soroban_invocation: false,
            contract_id: None,
            metadata: Map::new(),
        }
    }

    #[test]
    fn large_transfer_fires_above_threshold() {
        let rule = LargeTransferRule;
        let config = RulesConfig::default();
        let t = tx("20000", 0, Some("GBOB"));
        let ctx = RuleContext::default();
        let result = rule.evaluate(&t, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "large_transfer");
        assert_eq!(result.severity, Severity::High);
    }

    #[test]
    fn large_transfer_silent_below_threshold() {
        let rule = LargeTransferRule;
        let config = RulesConfig::default();
        let t = tx("100", 0, Some("GBOB"));
        let ctx = RuleContext::default();
        assert!(rule.evaluate(&t, &ctx, &config).is_none());
    }

    #[test]
    fn repeated_transaction_counts_within_window() {
        let rule = RepeatedTransactionRule;
        let config = RulesConfig::default();
        let history = vec![tx("10", -10, Some("GBOB")), tx("10", -20, Some("GBOB"))];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx("10", 0, Some("GBOB"));
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "repeated_transaction");
    }

    #[test]
    fn repeated_transaction_ignores_outside_window() {
        let rule = RepeatedTransactionRule;
        let config = RulesConfig::default();
        let history = vec![tx("10", -1000, Some("GBOB")), tx("10", -2000, Some("GBOB"))];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx("10", 0, Some("GBOB"));
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn flagged_account_interaction_detects_destination() {
        let rule = FlaggedAccountInteractionRule;
        let config = RulesConfig::default();
        let mut flagged = HashSet::new();
        flagged.insert("GBOB".to_string());
        let ctx = RuleContext::new(vec![], flagged);
        let current = tx("10", 0, Some("GBOB"));
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.severity, Severity::High);
    }

    #[test]
    fn unusual_asset_movement_requires_min_history() {
        let rule = UnusualAssetMovementRule;
        let config = RulesConfig::default();
        let ctx = RuleContext::new(vec![tx("10", -10, Some("GBOB"))], HashSet::new());
        let current = tx("1000", 0, Some("GBOB"));
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn unusual_asset_movement_fires_with_enough_history() {
        let rule = UnusualAssetMovementRule;
        let config = RulesConfig::default();
        let history = vec![
            tx("10", -10, Some("GBOB")),
            tx("10", -20, Some("GBOB")),
            tx("10", -30, Some("GBOB")),
        ];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx("1000", 0, Some("GBOB"));
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "unusual_asset_movement");
    }

    #[test]
    fn configurable_threshold_uses_named_limits() {
        let rule = ConfigurableThresholdRule;
        let mut config = RulesConfig::default();
        config
            .custom_thresholds
            .insert("max_single_asset_exposure".into(), 500.0);
        let current = tx("600", 0, Some("GBOB"));
        let ctx = RuleContext::default();
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert!(result.rule_name.contains("max_single_asset_exposure"));
    }

    #[test]
    fn dormant_reactivation_fires_after_long_gap() {
        let rule = DormantAccountReactivationRule;
        let config = RulesConfig::default();
        // last activity ~40 days ago, well beyond the 30-day default gap
        let history = vec![tx_from(
            "GALICE",
            "10",
            -(40 * 24 * 60 * 60),
            Some("GBOB"),
            Asset::Native,
        )];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "5000", 0, Some("GBOB"), Asset::Native);
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "dormant_account_reactivation");
    }

    #[test]
    fn dormant_reactivation_silent_with_recent_activity() {
        let rule = DormantAccountReactivationRule;
        let config = RulesConfig::default();
        let history = vec![tx_from("GALICE", "10", -60, Some("GBOB"), Asset::Native)];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "5000", 0, Some("GBOB"), Asset::Native);
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn dormant_reactivation_silent_with_no_history() {
        let rule = DormantAccountReactivationRule;
        let config = RulesConfig::default();
        let ctx = RuleContext::default();
        let current = tx_from("GALICE", "5000", 0, Some("GBOB"), Asset::Native);
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn round_trip_wash_trading_detects_return_leg() {
        let rule = RoundTripWashTradingRule;
        let config = RulesConfig::default();
        // GBOB sent to GALICE 10 minutes ago; now GALICE sends to GBOB.
        let history = vec![tx_from("GBOB", "100", -600, Some("GALICE"), Asset::Native)];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "100", 0, Some("GBOB"), Asset::Native);
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "round_trip_wash_trading");
    }

    #[test]
    fn round_trip_wash_trading_silent_without_return_leg() {
        let rule = RoundTripWashTradingRule;
        let config = RulesConfig::default();
        let ctx = RuleContext::default();
        let current = tx_from("GALICE", "100", 0, Some("GBOB"), Asset::Native);
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn new_account_high_value_outflow_fires_with_little_history() {
        let rule = NewAccountHighValueOutflowRule;
        let config = RulesConfig::default();
        let ctx = RuleContext::default();
        let current = tx_from("GNEWACCOUNT", "6000", 0, Some("GBOB"), Asset::Native);
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "new_account_high_value_outflow");
    }

    #[test]
    fn new_account_high_value_outflow_silent_with_established_history() {
        let rule = NewAccountHighValueOutflowRule;
        let config = RulesConfig::default();
        let history = vec![
            tx_from("GALICE", "10", -1000, Some("GBOB"), Asset::Native),
            tx_from("GALICE", "10", -2000, Some("GBOB"), Asset::Native),
            tx_from("GALICE", "10", -3000, Some("GBOB"), Asset::Native),
        ];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "6000", 0, Some("GBOB"), Asset::Native);
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn cross_asset_rapid_conversion_fires_across_distinct_assets() {
        let rule = CrossAssetRapidConversionRule;
        let config = RulesConfig::default();
        let history = vec![
            tx_from(
                "GALICE",
                "10",
                -100,
                Some("GBOB"),
                Asset::Credit {
                    code: "USDC".into(),
                    issuer: "GISSUER1".into(),
                },
            ),
            tx_from(
                "GALICE",
                "10",
                -200,
                Some("GBOB"),
                Asset::Credit {
                    code: "EURC".into(),
                    issuer: "GISSUER2".into(),
                },
            ),
        ];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "10", 0, Some("GBOB"), Asset::Native);
        let result = rule.evaluate(&current, &ctx, &config).unwrap();
        assert_eq!(result.rule_id, "cross_asset_rapid_conversion");
    }

    #[test]
    fn cross_asset_rapid_conversion_silent_with_single_asset() {
        let rule = CrossAssetRapidConversionRule;
        let config = RulesConfig::default();
        let history = vec![tx_from("GALICE", "10", -100, Some("GBOB"), Asset::Native)];
        let ctx = RuleContext::new(history, HashSet::new());
        let current = tx_from("GALICE", "10", 0, Some("GBOB"), Asset::Native);
        assert!(rule.evaluate(&current, &ctx, &config).is_none());
    }

    #[test]
    fn engine_evaluates_all_rules_and_sorts_output() {
        let engine = crate::RulesEngine::with_default_rules();
        let config = RulesConfig::default();
        let mut flagged = HashSet::new();
        flagged.insert("GBOB".to_string());
        let ctx = RuleContext::new(vec![], flagged);
        let current = tx("20000", 0, Some("GBOB"));
        let triggered = engine.evaluate(&current, &ctx, &config);
        assert!(triggered.iter().any(|t| t.rule_id == "large_transfer"));
        assert!(triggered
            .iter()
            .any(|t| t.rule_id == "flagged_account_interaction"));
        let ids: Vec<&str> = triggered.iter().map(|t| t.rule_id.as_str()).collect();
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        assert_eq!(ids, sorted_ids);
    }
}
