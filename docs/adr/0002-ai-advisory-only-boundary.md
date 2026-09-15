# ADR 0002: AI Is Strictly Advisory, Enforced at the Type Level

## Status
Accepted

## Context
An LLM can meaningfully help an investigator by summarizing a transaction, surfacing signals, and suggesting next steps — but LLM output can be wrong, can be influenced by adversarial input embedded in transaction metadata (prompt injection), and its stated confidence is not a reliable proxy for correctness. Fraud-triage decisions carry asymmetric cost: incorrectly freezing or flagging a legitimate account is real harm to a real person, so the system needs a boundary that holds even under a compromised or malfunctioning model, not just under a well-behaved one.

## Decision
Three independent, redundant layers enforce that the AI can never become the decision-maker:

1. **Type-level**: `AiAdvisor::investigate` can only return `AiRecommendation`, a type with no field capable of writing `Alert::status`. The only function that writes status, `AlertManager::apply_investigator_decision`, takes a different type (`InvestigatorDecision`) that the AI-advisor crate does not depend on.
2. **Prompt-level**: the system prompt explicitly forbids action language and requires a fixed schema with no decision field.
3. **Output-handling**: every AI response is treated as untrusted input — sanitized, relabeled, and clamped before storage or display (`crates/ai_advisor/src/sanitize.rs`).

We chose the type-level guarantee as the *primary* control, not the prompt, because prompts can be bypassed by a sufficiently adversarial or simply buggy model response, while a type that structurally cannot carry a status value cannot be bypassed by any response content whatsoever — the compiler enforces it independent of what the model says.

## Consequences
- **Positive**: the human-in-the-loop guarantee holds even if the AI provider is fully compromised or hallucinating; this property is testable and tested (`ai_investigation_endpoint_never_changes_alert_status` in `crates/api/tests/integration.rs`) rather than merely asserted in documentation.
- **Negative**: no "auto-triage" fast path exists for even the most obviously-confirmed cases; every alert requires a human decision regardless of AI confidence. This is a deliberate tradeoff, not an oversight — see [human-in-the-loop.md](../human-in-the-loop.md#why-not-let-the-ai-recommend-and-auto-apply-if-confidence-is-high).
