# Human-in-the-Loop Workflow

## The only path to a status change

`POST /api/alerts/:id/decision` is the sole endpoint capable of changing an alert's status. It requires an authenticated **Investigator** or **Admin** (see [authorization.md](authorization.md)):

- No token → `401 Unauthorized`.
- A **Viewer** token → `403 Forbidden`.
- An **Investigator** or **Admin** token → the decision is applied and the alert transitions.

This is tested end-to-end in `crates/api/tests/integration.rs::decision_requires_authorization` and, from the AI side, `ai_investigation_endpoint_never_changes_alert_status` — which repeatedly calls the AI advisory endpoint and asserts the alert's status is untouched by it.

## Decision states

Decisions map to `InvestigationStatus`:

| Status | Meaning |
|---|---|
| `Open` | Initial state on alert creation. No investigator has acted yet. |
| `UnderInvestigation` | An investigator has picked this up and is actively reviewing it. |
| `Escalated` | Sent to a higher tier of review (e.g. compliance, legal). |
| `Dismissed` | Determined to be a false positive. |
| `ConfirmedSuspicious` | Confirmed fraudulent/suspicious activity. |

`Dismissed` and `ConfirmedSuspicious` are treated as investigation outcomes, not hard locks — a human can still reopen and re-decide an alert if new evidence surfaces. What matters for compliance is that the change is always visible, never silent (see below).

## Decisions are never silently overwritten

Each call to `apply_investigator_decision` appends a **new** audit record; the previous decision is never deleted or mutated. Re-deciding an alert is allowed, but the full history — who decided what, when, and why — is always reconstructable from the audit trail. This is exercised directly in `crates/alerts/src/lib.rs::tests::repeated_decisions_are_all_preserved_in_audit_not_overwritten`.

## Effects of a `ConfirmedSuspicious` decision

Confirming an alert adds the involved accounts to the in-memory flagged-accounts set consulted by `FlaggedAccountInteractionRule`, so future transactions touching those accounts are caught immediately. Operationally, this is also the trigger point for calling the on-chain [Flagged Accounts Registry](stellar-soroban-integration.md#the-flagged-accounts-registry-contract)'s `flag` entry point — deliberately a manual/reviewed step in this reference implementation rather than an automatic on-chain write, so a human is always the one who puts an address on a public registry.

## Why not let the AI "recommend and auto-apply if confidence is high"?

This was deliberately rejected. See [ADR 0002](adr/0002-ai-advisory-only-boundary.md) for the full reasoning: false positives in fraud detection carry asymmetric cost (a wrongly frozen account is a real harm to a real user), and a model's stated confidence is not a reliable proxy for correctness. The human reviewer is the accountable decision-maker in every case, full stop.
