# Observability

## Structured logging

All logging goes through `tracing`, emitted as JSON (`tracing_subscriber` with the `json` feature) so it is directly consumable by log aggregators (e.g. Loki, Elasticsearch, CloudWatch Logs Insights). Every log line that touches an alert or transaction includes correlation identifiers — `tx_hash`, `alert_id`, `investigator_id` — without logging secrets or full account PII beyond the public Stellar address already inherent to the transaction.

Set `RUST_LOG` (any `tracing_subscriber::EnvFilter` string, e.g. `info`, `debug`, `stellartrace_rules_engine=debug,info`) to change verbosity.

## Metrics

The API exposes `GET /metrics` in Prometheus text exposition format (`crates/api/src/metrics.rs`), backed by plain `AtomicU64` counters in `AppState` — no external metrics crate dependency, kept intentionally simple for the reference implementation. Counters exposed:

| Metric | Meaning |
|---|---|
| `stellartrace_transactions_ingested_total` | Transactions received via the ingest/evaluate endpoints |
| `stellartrace_alerts_created_total` | Alerts created by the rules engine |
| `stellartrace_ai_recommendations_total` | AI advisory recommendations generated |
| `stellartrace_ai_fallback_total` | Of those, how many were fallback responses (AI unavailable) |
| `stellartrace_investigator_decisions_total` | Final human decisions submitted |

Wire this into a real Prometheus/Grafana stack by pointing a scrape config at `/metrics`; the text format is standard and needs no adapter.

## Suggested alerts for operators

- `stellartrace_ai_fallback_total` rate rising sharply → the AI provider is degraded; triage continues but investigators are seeing fewer AI-assisted summaries.
- No increase in `stellartrace_transactions_ingested_total` for longer than your expected polling interval → the ingestion pipeline may be stuck; check `dead_letters()` and the ingestion error logs.
- `GET /api/audit/verify` returning `intact: false` → treat as a security incident, not a bug — see [runbook.md](runbook.md#audit-integrity-failure).
