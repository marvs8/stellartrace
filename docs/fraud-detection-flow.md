# Fraud Detection Flow

1. **Ingestion** (`stellartrace-ingestion`) — polls Horizon `/operations` with retry/backoff; failures are logged and retried, never silently dropped.

2. **Normalization** — raw source-specific JSON becomes a `NormalizedTransaction`, the one canonical shape every downstream layer understands.

3. **Rules engine** (`stellartrace-rules-engine`) — every transaction runs through independent, pure `Rule` implementations. See [rules-catalog.md](rules-catalog.md) for the full list and the reasoning behind each one. Every trigger produces a `TriggeredRule` with a `reason` and an `evidence` map — deterministic and fully explainable, never a black box.

4. **Anomaly scoring** (`stellartrace-scoring`) — triggered rules combine into a single 0–100 score via severity-weighted, diminishing-returns aggregation, then map to a `Severity` band.

5. **Alert creation** (`stellartrace-alerts`) — if anything triggered, an `Alert` is created with status `Open`. This function, and *only* `apply_investigator_decision` afterward, may write `Alert::status`.

6. **AI-assisted investigation** (`stellartrace-ai-advisor`) — advisory only. See [ai-boundary.md](ai-boundary.md).

7. **Human decision** (`stellartrace-api` + `stellartrace-alerts`) — see [human-in-the-loop.md](human-in-the-loop.md).

8. **Audit** (`stellartrace-audit`) — every step above is recorded. See [audit-model.md](audit-model.md).

9. **Persistence** (`stellartrace-storage`, optional) — the API can periodically snapshot alerts and audit records to disk and restore them at startup, so an in-memory reference deployment survives a restart. See [persistence.md](persistence.md).

## Extending detection

New rules are added by implementing the four-method `Rule` trait in `crates/rules_engine/src/rules.rs` and registering it in `default_rule_set()` (or a custom `RulesEngine::new(vec![...])`). The engine, scoring, and every existing rule are untouched by this — that is the whole point of the trait boundary. Thresholds that don't warrant a bespoke rule can instead be added to `RulesConfig::custom_thresholds` with no code change at all, or loaded from `config/default_rules.toml` (see [configuration.md](configuration.md)).
