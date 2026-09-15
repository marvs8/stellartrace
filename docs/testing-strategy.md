# Testing Strategy

```bash
cargo test --workspace                        # unit + integration tests across all crates
cd contracts/flagged_accounts && cargo test   # Soroban contract tests
cargo bench -p stellartrace-rules-engine      # rule evaluation throughput
cargo bench -p stellartrace-scoring           # scoring throughput
```

## Coverage by area

- **Rules engine** (`crates/rules_engine/src/rules.rs`): each rule fires/doesn't fire at its boundary, window filtering, and a full-engine test asserting deterministic, sorted output. See [rules-catalog.md](rules-catalog.md) for the full rule list.
- **Anomaly scoring** (`crates/scoring/src/lib.rs`): zero-trigger baseline, severity ordering, diminishing returns on stacked triggers, and a single-critical-outranks-many-lows guarantee.
- **Alert lifecycle** (`crates/alerts/src/lib.rs`): creation starts `Open`, decisions transition status, unknown alerts error, and repeated decisions are all preserved (never overwritten) in the audit trail.
- **AI response validation** (`crates/ai_advisor/src/sanitize.rs`, `src/lib.rs`): a `MaliciousAdvisor` test double returns script tags, out-of-range confidence, and embedded "approve/freeze/ignore instructions" text — the suite asserts all of it is neutralized/clamped/relabeled before leaving the crate, and that a failing primary advisor falls back cleanly.
- **Authorization** (`crates/api/tests/integration.rs`): no token → 401; Viewer token → 403 on the decision endpoint; Investigator token → 200.
- **AI cannot change final status** (`crates/api/tests/integration.rs::ai_investigation_endpoint_never_changes_alert_status`): calls the AI advisory endpoint repeatedly and asserts the alert's status is untouched — the only thing that can change it is the decision endpoint, exercised separately.
- **Audit trail** (`crates/audit/src/lib.rs`, plus the API integration test): append-only history, hash-chain integrity verification, and tamper detection when a record is mutated in place.
- **Ingestion resilience** (`crates/ingestion/src/pipeline.rs`): retry-until-success and retry-exhaustion behavior with a flaky fake source.
- **Persistence** (`crates/storage/src/lib.rs`): snapshot round-trip (write then read back byte-identical state) for both alerts and audit records.
- **Metrics** (`crates/api/tests/metrics.rs`): counters increment on the expected events and the `/metrics` endpoint emits valid Prometheus text exposition format.
- **Soroban contract** (`contracts/flagged_accounts/src/lib.rs`): admin-only writes, public reads, double-initialize rejection.

## Why not property-based / fuzz testing (yet)

The rules engine's pure-function design (`Rule::evaluate`) is a natural fit for property-based testing (e.g. with `proptest`) — a good next step would be generating randomized transaction/history fixtures and asserting invariants like "a transaction below every threshold never triggers any rule" or "the score is monotonic non-decreasing in triggered-rule severity." This is left as a documented extension point rather than implemented here, to keep the reference implementation's dependency surface small.
