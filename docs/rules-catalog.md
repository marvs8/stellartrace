# Rules Catalog

Every rule lives in `crates/rules_engine/src/rules.rs`, implements the `Rule` trait, and is registered in `default_rule_set()`. Each is a pure function of `(transaction, context, config)` — deterministic, independent of the others, and independently testable.

| Rule id | Name | Fires when | Default severity |
|---|---|---|---|
| `large_transfer` | Large Transfer | A single transfer exceeds `large_transfer_threshold` | High (Critical if >10x threshold) |
| `repeated_transaction` | Repeated Transaction | N+ transactions to the same counterparty within `repeated_tx_window_secs` | Medium |
| `abnormal_frequency` | Abnormal Transaction Frequency | An account transacts more than `frequency_count_threshold` times within `frequency_window_secs` | Medium |
| `flagged_account_interaction` | Flagged Account Interaction | Either party is on the flagged-accounts set | High |
| `unusual_asset_movement` | Unusual Asset Movement | Amount is `unusual_asset_movement_multiplier`x+ the account's own historical average for that asset (requires `unusual_asset_min_history` prior transactions) | Medium |
| `configurable_threshold` | Configurable Threshold: `<name>` | Amount exceeds any operator-defined entry in `RulesConfig::custom_thresholds` | Low |
| `dormant_account_reactivation` | Dormant Account Reactivation | An account with no activity for `dormant_reactivation_gap_secs` suddenly transacts above `dormant_reactivation_min_amount` | High |
| `round_trip_wash_trading` | Round-Trip Wash Trading | Funds return to an account that sent to the same counterparty within `wash_trading_window_secs`, suggesting a wash/round-trip pattern | High |
| `new_account_high_value_outflow` | New Account High-Value Outflow | An account with a short observed history (`new_account_history_threshold` or fewer prior transactions) sends an amount above `new_account_outflow_threshold` | Medium |
| `cross_asset_rapid_conversion` | Cross-Asset Rapid Conversion | An account moves through `cross_asset_conversion_count_threshold`+ distinct assets within `cross_asset_conversion_window_secs`, a pattern associated with layering | Medium |

All thresholds are configurable — see [configuration.md](configuration.md).

## Adding a new rule

1. Implement `Rule` for a new struct in `crates/rules_engine/src/rules.rs` (or a new module if it's substantial).
2. Add it to `default_rule_set()`.
3. Add unit tests covering: the boundary condition, a case that should *not* fire, and (if it uses history) window-filtering behavior.
4. Add a row to the table above.

No other crate needs to change — this is the entire point of keeping the engine and rules decoupled (see [architecture.md](architecture.md#design-principles)).
