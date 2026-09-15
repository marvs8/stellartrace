# ADR 0003: What Goes On-Chain vs. Off-Chain

## Status
Accepted

## Context
StellarTrace is a Stellar/Soroban-native system, but not everything it does benefits from a ledger. Storing investigation data on-chain would make it permanent, public, and expensive to change — the opposite of what a working investigation needs (draft findings, private notes, evolving AI context, revisable decisions).

## Decision
Only one thing lives on-chain: a minimal Flagged Accounts Registry (`contracts/flagged_accounts`) storing, per address, a small numeric reason code and a timestamp. Everything else — transaction detail beyond what's already public on the ledger, rule evaluations, anomaly scores, AI recommendations, investigator notes, and the full audit trail — stays off-chain in StellarTrace's own storage.

The registry exists on-chain specifically because its value comes from being globally, cheaply, and trustlessly checkable by *other* parties (other Soroban contracts, other off-chain services in the ecosystem) without needing access to or trust in StellarTrace's own infrastructure or API.

## Consequences
- **Positive**: no sensitive investigation data is ever permanently and publicly exposed on a ledger; the on-chain contract is small enough to reason about and cheap to call; other ecosystem participants get a genuinely decentralized-verification benefit from the one piece that's actually suited to it.
- **Negative**: the registry is a separate system from the off-chain audit log, so "why was this address flagged" requires querying StellarTrace's off-chain records, not just the chain — this is intentional (see [audit-model.md](../audit-model.md)) but does mean the two systems must be kept operationally consistent (a `ConfirmedSuspicious` decision should be the trigger for calling `flag`, not an independent, potentially-drifting process).
