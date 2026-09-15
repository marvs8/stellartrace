# Security Considerations

- **AI output is untrusted input**, always sanitized before storage/display, and never executed as code or interpreted as a state-changing command. See [ai-boundary.md](ai-boundary.md).
- **Least-privilege AI context**: the AI receives only what's needed to assess one transaction — no other investigators' notes, no unrelated PII, no full audit history (`crates/ai_advisor/src/context.rs`).
- **No sensitive data on-chain**: the Soroban contract stores only an address, a reason code, and a timestamp. All investigation detail, AI output, and audit history stay off-chain. See [stellar-soroban-integration.md](stellar-soroban-integration.md).
- **Authorization on the sole state-changing action**: `POST /api/alerts/:id/decision` requires an authenticated Investigator/Admin; this is the only path capable of changing `Alert::status`, and it's covered by tests. See [authorization.md](authorization.md).
- **Fail-open for monitoring, fail-closed for decisions**: if the AI is down, triage continues with a labeled fallback; if auth is missing/invalid, the decision endpoint refuses rather than defaulting to allow.
- **Tamper-evident audit**: the hash-chained log makes silent post-hoc edits to investigation history detectable. See [audit-model.md](audit-model.md).
- **Default credentials are loud, not silent**: running without `STELLARTRACE_API_TOKENS` logs an explicit warning and uses an obviously-named dev token rather than failing to start — but should never be relied on outside local development.
- **Amounts as strings**: `NormalizedTransaction::amount` is kept as a decimal string end-to-end to avoid floating-point precision loss on Stellar's 7-decimal amounts; parsing to `f64` happens only inside rule evaluation math, never for storage or display of the canonical amount.

## Failure handling

| Failure | Behavior |
|---|---|
| AI service unavailable/unconfigured/malformed response | `AdvisorService` returns a labeled fallback recommendation (`model_available: false`); rules-engine detection and alerting are unaffected. |
| Ingestion source fails transiently | Bounded exponential backoff retry; on exhaustion, the batch is logged and (if applicable) retained for manual reprocessing rather than dropped — see `IngestionPipeline::dead_letters`. |
| Missing/invalid auth on a decision request | `401`/`403`, no state change. |
| Malformed or tampered audit record | Detected by `AuditLog::verify_integrity()`; treat any non-`intact` result as a security incident (see [runbook.md](runbook.md#audit-integrity-failure)), not a transient bug. |

## Reporting a vulnerability

See [`SECURITY.md`](../SECURITY.md) at the repository root.

## Known limitations of this reference implementation

- Default persistence is in-memory (`RwLock`-guarded collections in `AppState`); the optional `stellartrace-storage` snapshotting (see [persistence.md](persistence.md)) is a lightweight durability aid, not a replacement for a real transactional database in a production deployment.
- The bearer-token auth scheme is intentionally minimal; integrate a real identity provider for production (see [authorization.md](authorization.md)).
- The in-memory dead-letter buffer for failed ingestion batches is unbounded; add persistence and a size cap before relying on it as a durability guarantee at scale.
- Metrics counters reset on process restart (they are in-memory); export to a time-series database if long-term trend analysis is required.
