# The AI Boundary

**The AI can never approve, reject, freeze, reverse, or block a transaction, and it can never change an alert's status.** This is enforced in three independent, redundant ways — not by prompting alone.

## 1. Type-level enforcement

`AiAdvisor::investigate` (`crates/ai_advisor/src/lib.rs`) can only return an `AiRecommendation` (`crates/common/src/lib.rs`). That type has no field that maps to alert status.

The only function anywhere in the codebase that writes `Alert::status` is `AlertManager::apply_investigator_decision` (`crates/alerts/src/lib.rs`), which takes an `InvestigatorDecision` — a distinct type the `stellartrace-ai-advisor` crate does not depend on and structurally cannot construct a path to. See [ADR 0002](adr/0002-ai-advisory-only-boundary.md) for the reasoning behind choosing a type-level guarantee over a runtime check alone.

## 2. Prompt-level constraint

The system prompt sent to Claude (`crates/ai_advisor/src/claude.rs`) explicitly and repeatedly states the advisory-only constraint, and requires a fixed JSON schema with no "action" or "decision" field for the model to fill in.

## 3. Output-handling: AI output is untrusted input

Every AI response — regardless of whether it came from the real model or a fallback — passes through `crates/ai_advisor/src/sanitize.rs` before it is stored or shown:

- The `label` field is always force-overwritten to the canonical `AI_ADVISORY_LABEL`, regardless of what the model claimed about itself.
- HTML/markup is stripped from every free-text field.
- Text resembling a direct action instruction ("approve this transaction", "freeze the account", "set status to…", "ignore previous instructions") is neutralized in place rather than silently dropped, so an investigator can still see that the model said something inappropriate.
- `confidence` is clamped to `[0, 1]`; non-finite values are zeroed.
- All free-text fields are length-capped to prevent unbounded storage growth from a runaway response.

None of this text is ever interpreted as a command or executed. It is only ever rendered as a labeled string in the dashboard or API response.

## What the AI receives

For every flagged transaction, `crates/ai_advisor/src/context.rs::build_context` constructs a narrow `AiContext`:

- the transaction (hash, accounts, asset, amount, timestamp),
- the triggered-rule *reasons* (not raw evidence dumps),
- the anomaly score and severity,
- an aggregated (not raw) history summary — e.g. "4 prior transactions on record," never a dump of another account's full activity.

It does **not** receive: other investigators' identities, private investigation notes, or unrelated account history. This is a deliberate minimization, both for privacy and to reduce the prompt-injection surface — there is simply less untrusted or sensitive material in context to begin with.

## What the AI produces

- a concise transaction summary,
- the suspicious signals detected,
- relevant context from available transaction history,
- a risk assessment,
- recommended next investigative steps,
- a confidence/explanation field.

Every one of these fields is labeled advisory in both the API response (`label` field) and the dashboard UI (a persistent purple banner reading "ADVISORY ONLY").

## Failure handling

If the AI service is unavailable, unconfigured (no `ANTHROPIC_API_KEY`), or returns something unparseable, `AdvisorService` (`crates/ai_advisor/src/lib.rs`) returns a clearly labeled fallback recommendation with `model_available: false` instead of propagating an error. Rules-engine detection and alerting never depend on the AI being up — see [security.md](security.md#failure-handling).
