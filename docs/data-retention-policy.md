# Data Retention Policy (Template)

This is a template for operators to adapt, not a policy StellarTrace enforces in code. Retention decisions belong to the deploying organization's legal/compliance function, but the system is designed to make each of the following straightforward.

## Categories

| Data | Where it lives | Suggested minimum retention | Notes |
|---|---|---|---|
| Alerts (`Alert` records) | `AlertManager` / `stellartrace-storage` snapshots | Per applicable financial-crime record-keeping regulation in your jurisdiction (commonly 5+ years) | Contains account addresses, amounts, triggered-rule reasons. |
| Audit trail (`AuditRecord`s) | `AuditLog` / `stellartrace-storage` snapshots | Same as alerts, and never shorter — the audit trail is what proves the alert was handled correctly | Append-only; do not implement a "delete old records" job without first exporting to cold, compliant storage. |
| AI advisory recommendations | Embedded in `AuditRecord::AiRecommendationReceived` (metadata only) and transiently in `AppState::ai_recommendations` | Same as the alert it's attached to | Full AI recommendation text is not persisted in the reference in-memory store beyond the current process lifetime unless you extend `stellartrace-storage` to snapshot it — decide whether your compliance requirements need the full text retained. |
| Raw ingested transactions | `AppState::transactions` (in-memory) | Public Stellar ledger data is already permanently available via Horizon; this cache exists for rule-context lookups, not as a system of record | Safe to bound/evict aggressively; the ledger itself is the durable source. |

## Recommendations

- Export alerts and audit records to your organization's long-term compliant storage (e.g. a write-once object store) on a schedule, rather than relying solely on the in-memory/JSON-snapshot reference persistence.
- If you extend `stellartrace-storage` to a real database, apply row-level retention/legal-hold flags at that layer rather than in application code, so retention policy changes don't require a deploy.
- Never delete an `AuditRecord` to "free up space" — see [audit-model.md](audit-model.md). Archive instead.
