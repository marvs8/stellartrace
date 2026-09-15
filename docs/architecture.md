# Architecture

StellarTrace is a Rust Cargo workspace with one crate per concern. No crate reaches into another's internals — they only share the plain data types in `stellartrace-common`. This keeps the detection pipeline testable in isolation and lets any single layer (e.g. the AI backend, or the storage backend) be swapped without touching the others.

```
                 ┌─────────────────────────┐
                 │   Stellar Horizon /      │
                 │   Soroban RPC / events   │
                 └────────────┬─────────────┘
                              │  raw JSON
                    (1) stellartrace-ingestion
                    - Horizon polling client
                    - retry + dead-letter handling
                              │
                    (2) event normalization
                    - normalize.rs: raw → NormalizedTransaction
                              │
                    (3) stellartrace-rules-engine
                    - deterministic rules, structured reasons
                              │  Vec<TriggeredRule>
                    (4) stellartrace-scoring
                    - weighted anomaly score → severity
                              │
                    (5) stellartrace-alerts
                    - Alert record, lifecycle, status transitions
                              │
              ┌───────────────┼────────────────────┐
              │                                     │
   (6) stellartrace-ai-advisor              (8) stellartrace-audit
   - advisory-only recommendation           - append-only, hash-chained
   - untrusted-output sanitization            audit trail
   - graceful fallback if AI is down                │
              │                                     │
              └──────────────┬──────────────────────┘
                              │
                 (9) stellartrace-api (axum)
                 - REST endpoints, auth/authz, structured logging,
                   metrics, periodic snapshot persistence
                              │
                 (7) human review workflow
                 - InvestigatorDecision is the ONLY thing that can
                   change Alert::status (enforced at the type level)
                              │
                (10) dashboard/index.html
                 - investigator UI, AI vs. human boundary made explicit
```

## Crate map

| # | Concern | Crate | Key types |
|---|---|---|---|
| 1 | Stellar/Soroban ingestion | `stellartrace-ingestion` | `HttpHorizonClient`, `IngestionPipeline`, `TransactionSource` |
| 2 | Event normalization | `stellartrace-ingestion::normalize` | `normalize_horizon_operation` |
| 3 | Rules engine | `stellartrace-rules-engine` | `Rule`, `RulesEngine`, `RulesConfig` |
| 4 | Anomaly scoring | `stellartrace-scoring` | `compute_score`, `severity_for_score` |
| 5 | Alert management | `stellartrace-alerts` | `AlertManager`, `Alert` |
| 6 | AI investigation assistant | `stellartrace-ai-advisor` | `AiAdvisor`, `AdvisorService`, `ClaudeAdvisor`, `sanitize` |
| 7 | Human review workflow | `stellartrace-alerts` + `stellartrace-api` | `InvestigatorDecision`, `apply_investigator_decision` |
| 8 | Audit logging | `stellartrace-audit` | `AuditLog`, `AuditRecord` (hash-chained) |
| 9 | API / backend | `stellartrace-api` | axum router, `AuthUser`, `AppState`, metrics |
| 10 | Investigator interface | `dashboard/index.html` | static HTML/JS dashboard |
| — | Persistence | `stellartrace-storage` | `SnapshotStore`, JSON file-backed repositories |
| — | On-chain | `contracts/flagged_accounts` | Soroban flagged-accounts registry |

## Design principles

- **Determinism over inference** in the detection path: every rule is a pure function of `(transaction, context, config)`. The AI layer is additive and advisory, never a replacement for the deterministic engine.
- **Explainability**: every triggered rule carries a human-readable `reason` and a machine-readable `evidence` map — nothing about *why* a transaction was flagged should require reverse-engineering.
- **Explicit boundaries**: the AI cannot write alert state (see [ai-boundary.md](ai-boundary.md)); only an authorized human can (see [human-in-the-loop.md](human-in-the-loop.md)).
- **Fail safe, not fail closed on monitoring**: if the AI or a downstream integration is unavailable, detection and alerting keep working (see [security.md](security.md#failure-handling)).
- **Auditable by construction**: every state transition is recorded in an append-only, hash-chained log (see [audit-model.md](audit-model.md)).

See also: [Stellar/Soroban integration](stellar-soroban-integration.md), [fraud detection flow](fraud-detection-flow.md), [API reference](api-reference.md).
