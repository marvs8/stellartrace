# Audit Model

`stellartrace-audit::AuditLog` is **append-only** by construction — there is no update or delete method on `AuditRecord` anywhere in the crate.

## What gets recorded

Every meaningful action in the pipeline appends a new `AuditEventKind`:

| Event | Recorded when | Fields |
|---|---|---|
| `AlertCreated` | An alert is filed | anomaly score, severity |
| `RulesTriggered` | Alongside creation | which rule ids fired |
| `AiContextSent` | The AI advisory endpoint is called | a summary of what was sent (not the raw payload, to avoid duplicating potentially large data in the log while still recording scope) |
| `AiRecommendationReceived` | The AI (or fallback) responds | confidence, whether the real model was available |
| `InvestigatorDecision` | A human submits a decision | investigator id, decision, notes |
| `StatusChanged` | Alongside a decision | from/to status |

Every record also carries the `alert_id`, the transaction hash, and a timestamp — the correlation identifiers needed to reconstruct "what happened to this alert, in order" without cross-referencing anything else.

## Tamper evidence

Each `AuditRecord` embeds:
- `prev_hash`: the SHA-256 hash of the previous record in the log (or a fixed genesis value for the first record),
- `this_hash`: SHA-256 of this record's own content chained with `prev_hash`.

This forms a hash chain, similar in spirit to a blockchain's linking but entirely off-chain and dependency-free. `AuditLog::verify_integrity()` recomputes the chain from scratch and returns an error identifying the first record whose stored hash doesn't match its recomputed hash, or whose `prev_hash` doesn't match its predecessor's `this_hash` — either case means a record was altered, reordered, or removed after the fact. This is exposed operationally via `GET /api/audit/verify`.

See [ADR 0004](adr/0004-append-only-hash-chained-audit.md) for why a hash chain was chosen over, e.g., relying solely on database-level append-only permissions.

## What this does and doesn't guarantee

- **Does**: make undetected silent edits to *recorded* history practically impossible, given the chain is checked.
- **Does**: preserve every investigator decision, even superseded ones, so "who said what, when" is always answerable.
- **Does not** (in this reference implementation): provide Byzantine fault tolerance or external anchoring — the log lives in one process's memory (or, with the storage crate, one JSON snapshot file). A determined operator with direct access to the storage backend could still replace the entire log wholesale. Practical mitigations for a production deployment: write to an append-only-grant database table, ship records to a separate write-once log store (e.g. object storage with object-lock/WORM), or periodically checkpoint the chain's latest hash somewhere independent (including, if desired, on-chain — the hash chain's interface would not need to change).
