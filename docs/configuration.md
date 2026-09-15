# Configuration

Rule thresholds live in `stellartrace_rules_engine::RulesConfig` (`crates/rules_engine/src/config.rs`), which implements `Default` with sensible starting values and derives `serde::{Serialize, Deserialize}` so it can be loaded from a file.

## Loading from TOML

Set `STELLARTRACE_RULES_CONFIG` to the path of a TOML file (see `config/default_rules.toml` for a fully-commented example) and the API loads it at startup via `stellartrace_rules_engine::config::load_from_file`, falling back to `RulesConfig::default()` (with a logged warning) if the path is unset, missing, or unparseable — a misconfigured threshold file should never prevent the service from starting.

```toml
# config/default_rules.toml
large_transfer_threshold = 10000.0

repeated_tx_window_secs = 60
repeated_tx_count_threshold = 3

frequency_window_secs = 300
frequency_count_threshold = 10

unusual_asset_movement_multiplier = 5.0
unusual_asset_min_history = 3

dormant_reactivation_gap_secs = 2592000     # 30 days
dormant_reactivation_min_amount = 1000.0

wash_trading_window_secs = 3600

new_account_history_threshold = 2
new_account_outflow_threshold = 5000.0

cross_asset_conversion_window_secs = 600
cross_asset_conversion_count_threshold = 3

[custom_thresholds]
max_single_asset_exposure = 250000.0
```

## Why TOML, and why fail open to defaults

- TOML is easy for a non-Rust-fluent compliance/risk analyst to read and edit directly, without touching code.
- Failing open to `RulesConfig::default()` on a bad config file matches the project's general failure-handling posture (see [security.md](security.md#failure-handling)): a config problem should degrade to "using conservative defaults" and log loudly, not take detection offline entirely.

## Hot-reloading

Not implemented in this reference implementation — `RulesConfig` is loaded once at startup and held behind a `RwLock` in `AppState`, which means it *can* be swapped at runtime (e.g. via an admin-only reload endpoint) without a restart; wiring that endpoint up is a natural next step and requires no changes to the rules engine itself.
