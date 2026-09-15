# Threat Model

This is a lightweight, focused threat model — enough to justify the security decisions elsewhere in `docs/`, not a full STRIDE exercise.

## Assets

1. **Alert and audit data** — evidence of who was investigated, what was found, and what was decided. Integrity and non-repudiation matter more than confidentiality for most of it (an investigator's decision should be provably theirs and provably unaltered), though notes may contain sensitive personal or account information.
2. **The decision boundary itself** — the guarantee that only an authorized human, never the AI, can change an alert's status. This is the system's core compliance property.
3. **API credentials** (bearer tokens) — compromise lets an attacker read alert data or, worse, submit fraudulent investigator decisions.
4. **The on-chain flagged-accounts registry's admin key** — compromise lets an attacker flag/unflag arbitrary addresses publicly.

## Threats and mitigations

| Threat | Mitigation |
|---|---|
| A compromised or manipulated AI response tries to get treated as a decision | Type-level boundary: no AI-facing type can write `Alert::status` (see [ai-boundary.md](ai-boundary.md)). Output sanitization neutralizes action-instruction language regardless. |
| A prompt-injection payload embedded in transaction metadata reaches the AI and tries to leak unrelated data or alter its output's meaning | `AiContext` is a narrow, explicit allowlist of fields (see [ai-boundary.md](ai-boundary.md#what-the-ai-receives)) — there is no free-form "notes" or "memo" field passed through verbatim into a context that could carry a payload beyond what's already validated transaction data. |
| A stolen Viewer token is used to attempt a decision | `403 Forbidden` — role check on the one state-changing endpoint, tested in `crates/api/tests/integration.rs`. |
| An attacker with storage access rewrites audit history to hide an earlier decision | `AuditLog::verify_integrity()` detects the tampered hash chain. This does not *prevent* the tampering (see [audit-model.md](audit-model.md#what-this-does-and-doesnt-guarantee)) — it makes it detectable, which is the achievable guarantee for an off-chain, single-process log. |
| The AI provider is down, rate-limited, or returns garbage during an active investigation | `AdvisorService` fallback ensures the investigator still gets a labeled, honest "AI unavailable" response rather than a stalled request or a fabricated-looking success. |
| The on-chain registry's admin key is compromised | `require_auth` scopes all writes to a single address; rotate via `set_admin` (itself `require_auth`-gated by the *current* admin) the moment compromise is suspected. The registry stores no sensitive data, so the worst case is public mislabeling of addresses, not a data breach. |
| A malformed rules config file is deployed | Fails open to `RulesConfig::default()` with a logged warning rather than refusing to start (see [configuration.md](configuration.md)) — detection availability is prioritized over enforcing a specific tuning. |

## Out of scope for this reference implementation

- Network-layer threats (TLS termination, DDoS protection) — assumed to be handled by the deployment environment (load balancer, reverse proxy, CDN).
- Multi-tenant isolation — this implementation assumes one operator's investigation team, not multiple mutually-untrusting organizations sharing one deployment.
- Formal verification of the Soroban contract — it is intentionally small enough (roughly a dozen lines of actual logic) to be reviewed by inspection, but has not been through a formal audit process.
