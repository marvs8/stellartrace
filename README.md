# StellarTrace

A production-oriented fraud and anomaly detection triage system for the Stellar ecosystem: it ingests Stellar/Soroban transaction activity, runs it through a deterministic and explainable rules engine, scores and files alerts, offers an **advisory-only** AI investigation assistant, and enforces a strict human-in-the-loop decision workflow with a tamper-evident audit trail.

StellarTrace is not a simple CRUD app. Its design center is: **explainability, deterministic detection, auditability, graceful failure handling, and a hard boundary between AI recommendations and human decisions.**

---

## 1. Architecture overview

The system is a Rust Cargo workspace with one crate per concern. No crate reaches into another's internals — they only share the plain data types in `stellartrace-common`.

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
                 - REST endpoints, auth/authz, structured logging
                              │
                 (7) human review workflow
                 - InvestigatorDecision is the ONLY thing that can
                   change Alert::status (enforced at the type level)
                              │
                (10) dashboard/index.html
                 - investigator UI, AI vs. human boundary made explicit
```

### Crate map

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
| 9 | API / backend | `stellartrace-api` | axum router, `AuthUser`, `AppState` |
| 10 | Investigator interface | `dashboard/index.html` | static HTML/JS dashboard |

On-chain: `contracts/flagged_accounts` — a Soroban contract (see §3).

---

## 2. Fraud detection flow

1. **Ingestion** (`stellartrace-ingestion`): polls Horizon `/operations`, with bounded exponential-backoff retry. If a batch keeps failing after retries, it's kept in an in-memory dead-letter buffer rather than discarded — the ingestion loop logs the failure and keeps polling the next window rather than crashing, so one bad request never stalls monitoring for good.

2. **Normalization**: raw Horizon JSON (or, once wired up, Soroban RPC events) is converted into `NormalizedTransaction` — one canonical shape the rest of the system understands, regardless of source.

3. **Rules engine** (`stellartrace-rules-engine`): every `NormalizedTransaction` is run through independent, pure `Rule` implementations:
   - `LargeTransferRule` — amount over a configurable threshold.
   - `RepeatedTransactionRule` — N+ transactions to the same counterparty within a short window.
   - `AbnormalFrequencyRule` — an account transacting far more often than its configured baseline.
   - `FlaggedAccountInteractionRule` — either party is on the flagged-accounts list.
   - `UnusualAssetMovementRule` — amount is an outlier vs. the account's own historical average for that asset.
   - `ConfigurableThresholdRule` — arbitrary named numeric thresholds, addable via config with no code change.

   Every trigger produces a `TriggeredRule` with a `reason` (human-readable) and `evidence` (machine-readable key/value map of the numbers behind that reason) — nothing is a black box. Rules are deterministic: same inputs, same output, every time. New rules are added by implementing the four-method `Rule` trait and registering it; the engine and existing rules never change.

4. **Anomaly scoring** (`stellartrace-scoring`): triggered rules are combined into a single 0–100 score via severity-weighted, diminishing-returns aggregation (stacking many low-severity rules doesn't outrank one critical rule). The score maps to a `Severity` band (`Low`/`Medium`/`High`/`Critical`).

5. **Alert creation** (`stellartrace-alerts`): if anything triggered, an `Alert` is created holding the transaction id, account addresses, asset, amount, timestamp, triggered rules, score, severity, and status (`Open` initially). This — and *only* `apply_investigator_decision` afterward — is what writes `Alert::status`.

6. **AI-assisted investigation** (`stellartrace-ai-advisor`) — advisory only, see §4.

7. **Human decision** (`stellartrace-api` + `stellartrace-alerts`) — see §5.

8. **Audit** (`stellartrace-audit`) — every step above is recorded, see §6.

---

## 3. Stellar / Soroban integration

- **Ingestion** talks to a Horizon-compatible REST endpoint (`HttpHorizonClient` in `crates/ingestion/src/horizon.rs`), polling `/operations` by cursor. The same `TransactionSource` trait used by the retry pipeline can be implemented for a Soroban RPC event subscription without touching any downstream code — ingestion is the only layer that knows about network transport.
- **Soroban is used only where it earns its place.** The rest of the system (rules, scoring, alerting, AI, audit, human decisions) is intentionally off-chain: it needs mutable, richly structured, potentially sensitive investigation state and fast iteration, none of which belongs on a ledger.
- The one on-chain piece is `contracts/flagged_accounts` — a minimal **Flagged Accounts Registry** Soroban contract:
  - Stores, per address: a small numeric `ReasonCode` (large-transfer pattern, repeated-transaction abuse, confirmed fraud, etc.) and the ledger timestamp it was flagged.
  - Writes (`flag`, `unflag`, `set_admin`) are gated by `require_auth` on a configured admin address (the StellarTrace backend's operational key) — they're only ever called *after* a human investigator confirms an account suspicious off-chain (typically via `ConfirmedSuspicious` in the API, see `handlers::submit_decision`).
  - Reads (`is_flagged`, `get_flag`) are public and unauthenticated, so any other Soroban contract or off-chain service in the ecosystem can cheaply check an address without needing access to StellarTrace's private investigation database.
  - **No sensitive investigation data ever goes on-chain** — no transaction details, no AI analysis, no investigator identity or notes. Those live only in the off-chain audit log; the contract only answers "is this address currently flagged, and under which broad category."

Build/test the contract:
```bash
cd contracts/flagged_accounts
cargo test                                   # native unit tests
cargo build --release --target wasm32-unknown-unknown   # wasm artifact for deployment
```

---

## 4. The AI boundary (critical design constraint)

**The AI can never approve, reject, freeze, reverse, or block a transaction, and it can never change an alert's status.** This is enforced in three independent ways, not just by prompting:

1. **Type-level:** `AiAdvisor::investigate` can only return an `AiRecommendation` (`crates/common/src/lib.rs`). That type has no field that maps to alert status. The only function anywhere in the codebase that writes `Alert::status` is `AlertManager::apply_investigator_decision`, which takes an `InvestigatorDecision` — a different type the AI advisor crate does not depend on and cannot construct.
2. **Prompt-level:** the system prompt sent to Claude (`crates/ai_advisor/src/claude.rs`) explicitly and repeatedly states the advisory-only constraint and requires a fixed JSON schema with no "action" field.
3. **Output-handling level:** every AI response is treated as **untrusted input** (`crates/ai_advisor/src/sanitize.rs`) before it is stored or shown:
   - the `label` field is always force-overwritten to the canonical `AI_ADVISORY_LABEL`, regardless of what the model said;
   - HTML/markup is stripped;
   - text resembling a direct action instruction ("approve this transaction", "freeze the account", "set status to…") is neutralized in place;
   - `confidence` is clamped to `[0, 1]`, non-finite values are zeroed;
   - all free-text fields are length-capped.

   None of this text is ever interpreted as a command or executed — it is only ever rendered as a labeled string.

For every flagged transaction, the AI receives a narrow `AiContext` (`crates/ai_advisor/src/context.rs`) built specifically for it — the transaction, the triggered-rule reasons, the anomaly score, and an aggregated (not raw) history summary. It does **not** receive other investigators' identities, private notes, or unrelated account history. It produces:
- a concise transaction summary,
- the suspicious signals detected,
- relevant context from available transaction history,
- a risk assessment,
- recommended next investigative steps,
- a confidence/explanation field.

**Failure handling:** if the AI service is unavailable, unconfigured, or returns something unparseable, `AdvisorService` (`crates/ai_advisor/src/lib.rs`) returns a clearly labeled fallback recommendation with `model_available: false` instead of erroring out. Rules-engine detection and alerting never depend on the AI being up.

---

## 5. Human-in-the-loop workflow

- The only way to change an alert's status is `POST /api/alerts/:id/decision`, which requires an authenticated **Investigator** or **Admin** (see §9). A **Viewer** token gets `403 Forbidden`; no token gets `401 Unauthorized`.
- Decisions map to `InvestigationStatus`: `Open` (initial) → `UnderInvestigation`, `Escalated`, `Dismissed` (false positive), or `ConfirmedSuspicious`.
- A decision is never silently overwritten. Each decision appends a **new** audit record; the previous decision stays in the trail. Re-deciding an alert (e.g. new evidence reopens a dismissed one) is allowed, but the full history is always reconstructable — see `AlertManager::apply_investigator_decision` and its test `repeated_decisions_are_all_preserved_in_audit_not_overwritten`.
- Confirming an alert `ConfirmedSuspicious` adds the involved accounts to the in-memory flagged-accounts set (and, operationally, would call the on-chain registry's `flag`), which future rule evaluations pick up via `FlaggedAccountInteractionRule`.

---

## 6. Audit model

`stellartrace-audit::AuditLog` is **append-only**: there is no update or delete method on `AuditRecord`, by construction. Every meaningful action records a new `AuditEventKind`:

- `AlertCreated` — anomaly score + severity at creation.
- `RulesTriggered` — which rule ids fired.
- `AiContextSent` — a summary of what was sent to the AI (not the raw prompt, to avoid duplicating potentially large/sensitive payloads in the log, but enough to reconstruct scope).
- `AiRecommendationReceived` — confidence and whether the real model was available.
- `InvestigatorDecision` — who, what decision, and their notes.
- `StatusChanged` — from/to status.

**Tamper evidence:** each record embeds the SHA-256 hash of the previous record plus its own content (`prev_hash`/`this_hash`), forming a hash chain. `AuditLog::verify_integrity()` recomputes the chain and detects any alteration, reordering, or deletion of a past record — exposed via `GET /api/audit/verify`. This is a practical, dependency-free approximation of tamper evidence; it doesn't require an on-chain anchor, though the resulting hash chain could be checkpointed on-chain later without changing this crate's interface.

---

## 7. API

All endpoints are prefixed `/api` except `/health`.

| Method | Path | Purpose | Auth |
|---|---|---|---|
| GET | `/health` | Liveness check | none |
| POST | `/api/transactions` | Ingest a `NormalizedTransaction` and immediately evaluate it against the rules engine (creates an alert if triggered) | none* |
| GET | `/api/transactions/:tx_hash` | Retrieve a previously ingested transaction | none* |
| POST | `/api/transactions/:tx_hash/evaluate` | Re-evaluate an already-ingested transaction against current rules/config | none* |
| GET | `/api/alerts` | List alerts, optional `?status=open\|under_investigation\|escalated\|dismissed\|confirmed_suspicious` | none* |
| GET | `/api/alerts/:alert_id` | Retrieve one alert | none* |
| GET | `/api/alerts/:alert_id/ai-investigation` | Get (generating if needed) the advisory AI recommendation | none* |
| POST | `/api/alerts/:alert_id/decision` | Submit the final human decision | **Investigator/Admin** |
| GET | `/api/alerts/:alert_id/audit` | Full audit trail for one alert | none* |
| GET | `/api/audit/verify` | Verify the whole audit chain's integrity | none* |

\* In this reference implementation, only the state-changing decision endpoint enforces authorization, matching the spec's requirement that only authorized investigators can change alert status. In a production deployment you would put read endpoints behind at-least-Viewer auth too (the `AuthUser` extractor is already reusable on any handler — add it to a handler's signature and it is authenticated).

---

## 8. Authentication & authorization

Minimal, dependency-light bearer-token scheme (`crates/api/src/auth.rs`), configured via `STELLARTRACE_API_TOKENS` (see §10). Three roles:

- `Viewer` — read-only.
- `Investigator` — may submit decisions.
- `Admin` — may submit decisions (and, operationally, manage tokens/config).

`AuthUser` is an axum extractor; any handler that takes it as a parameter is authenticated, and `role.can_decide()` gates the one state-changing endpoint. Swapping this for OAuth2/OIDC means replacing `AuthRegistry::authenticate` only.

---

## 9. Structured logging

All logging goes through `tracing`, emitted as JSON (`tracing_subscriber` with the `json` feature) so it's directly consumable by log aggregators. Every log line that touches an alert or transaction includes correlation identifiers — `tx_hash`, `alert_id`, `investigator_id` — without logging secrets or full account PII beyond the public Stellar address already inherent to the transaction. Set `RUST_LOG=debug` (or any `tracing_subscriber::EnvFilter` string) to change verbosity.

---

## 10. Setup & environment variables

Requirements: Rust stable (edition 2021), and for the contract, the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`).

```bash
# Build everything (backend workspace)
cargo build --workspace

# Run the API server
cargo run -p stellartrace-api
```

Then open `dashboard/index.html` directly in a browser (or serve it with any static file server) and point it at the API base URL shown in the header.

### Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `STELLARTRACE_BIND_ADDR` | API bind address | `0.0.0.0:8080` |
| `STELLARTRACE_API_TOKENS` | `token:role:investigator_id,...` — see §8 | falls back to a single insecure `dev-investigator-token` (Investigator role), with a loud warning logged — **never use the default in production** |
| `ANTHROPIC_API_KEY` | Enables the real Claude-backed advisor | unset → AI endpoint returns fallback (`model_available: false`) advisory responses only |
| `RUST_LOG` | `tracing_subscriber::EnvFilter` string | `info` |

Example:
```bash
export STELLARTRACE_API_TOKENS="s3cr3t-inv:investigator:alice,s3cr3t-admin:admin:bob,s3cr3t-view:viewer:carol"
export ANTHROPIC_API_KEY="sk-ant-..."
cargo run -p stellartrace-api
```

---

## 11. Testing strategy

```bash
cargo test --workspace          # unit + integration tests across all crates
cd contracts/flagged_accounts && cargo test   # Soroban contract tests
```

Coverage by area:
- **Rules engine** (`crates/rules_engine/src/rules.rs`): each rule fires/doesn't fire at its boundary, window filtering, and a full-engine test asserting deterministic, sorted output.
- **Anomaly scoring** (`crates/scoring/src/lib.rs`): zero-trigger baseline, severity ordering, diminishing returns on stacked triggers, and a single-critical-outranks-many-lows guarantee.
- **Alert lifecycle** (`crates/alerts/src/lib.rs`): creation starts `Open`, decisions transition status, unknown alerts error, and repeated decisions are all preserved (never overwritten) in the audit trail.
- **AI response validation** (`crates/ai_advisor/src/sanitize.rs`, `src/lib.rs`): a `MaliciousAdvisor` test double returns script tags, out-of-range confidence, and embedded "approve/freeze/ignore instructions" text — the suite asserts all of it is neutralized/clamped/relabeled before leaving the crate, and that a failing primary advisor falls back cleanly.
- **Authorization** (`crates/api/tests/integration.rs`): no token → 401; Viewer token → 403 on the decision endpoint; Investigator token → 200.
- **AI cannot change final status** (`crates/api/tests/integration.rs::ai_investigation_endpoint_never_changes_alert_status`): calls the AI advisory endpoint repeatedly and asserts the alert's status is untouched — the only thing that can change it is the decision endpoint, which is exercised separately.
- **Audit trail** (`crates/audit/src/lib.rs`, plus the API integration test): append-only history, hash-chain integrity verification, and tamper detection when a record is mutated in place.
- **Ingestion resilience** (`crates/ingestion/src/pipeline.rs`): retry-until-success and retry-exhaustion behavior with a flaky fake source.

---

## 12. Security considerations

- **AI output is untrusted input**, always sanitized before storage/display, and never executed as code or interpreted as a state-changing command (§4).
- **Least-privilege AI context**: the AI receives only what's needed to assess one transaction — no other investigators' notes, no unrelated PII, no full audit history (`crates/ai_advisor/src/context.rs`).
- **No sensitive data on-chain**: the Soroban contract stores only an address, a reason code, and a timestamp (§3). All investigation detail, AI output, and audit history stay off-chain.
- **Authorization on the sole state-changing action**: `POST /api/alerts/:id/decision` requires an authenticated Investigator/Admin; this is the only path capable of changing `Alert::status`, and it's covered by tests.
- **Fail-open for monitoring, fail-closed for decisions**: if the AI is down, triage continues with a labeled fallback; if auth is missing/invalid, the decision endpoint refuses rather than defaulting to allow.
- **Tamper-evident audit**: the hash-chained log makes silent post-hoc edits to investigation history detectable (§6).
- **Default credentials are loud, not silent**: running without `STELLARTRACE_API_TOKENS` logs an explicit warning and uses an obviously-named dev token rather than failing to start — but should never be relied on outside local development.
- **Amounts as strings**: `NormalizedTransaction::amount` is kept as a decimal string end-to-end to avoid floating-point precision loss on Stellar's 7-decimal amounts; parsing to `f64` happens only inside rule evaluation math, never for storage or display of the canonical amount.

### Known limitations of this reference implementation (call out before production use)

- Storage is in-memory (`RwLock`-guarded collections in `AppState`); swap in a real database behind the same `AlertManager`/`AuditLog` interfaces for persistence.
- The bearer-token auth scheme is intentionally minimal; integrate a real identity provider for production.
- The dead-letter buffer for failed ingestion batches is in-memory and unbounded; add persistence and a size cap before relying on it as a durability guarantee.
