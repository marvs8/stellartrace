# Stellar / Soroban Integration

## Ingestion

`stellartrace-ingestion` talks to a Horizon-compatible REST endpoint (`HttpHorizonClient` in `crates/ingestion/src/horizon.rs`), polling `/operations` by cursor. It is the *only* layer that knows about network transport — everything downstream consumes `stellartrace_common::NormalizedTransaction`.

The `TransactionSource` trait (`crates/ingestion/src/pipeline.rs`) abstracts "a thing that can fetch a page of operations." The same trait can be implemented for a Soroban RPC event subscription (e.g. `getEvents` polling, or a websocket stream) without touching the rules engine, scoring, alerting, or API code — swapping or adding a source is a single new struct.

## Retry & failure handling

`IngestionPipeline::run` wraps every fetch in bounded exponential backoff (`RetryConfig`). If a batch exhausts its retries, the pipeline:
- logs an `error!`-level event so it can be alerted on operationally,
- does **not** fabricate or skip data — the caller can inspect `dead_letters()` for anything that needs manual reprocessing,
- continues polling on the next tick rather than crashing the whole monitoring loop.

This satisfies the "transactions should not be silently lost" requirement without introducing a full message-queue dependency in the reference implementation.

## Why Soroban only where it earns its place

The bulk of StellarTrace's logic — rules, scoring, alerting, AI advisory, audit, human decisions — needs mutable, richly structured, sometimes sensitive state, and fast iteration. None of that belongs on a ledger. Soroban is used for exactly one thing: a small, public, high-value primitive that benefits from being globally verifiable and independent of StellarTrace's own uptime.

## The Flagged Accounts Registry contract

`contracts/flagged_accounts` is a minimal Soroban contract (see also [ADR 0003](adr/0003-on-chain-vs-off-chain-data-split.md)):

- **Storage**: per address, a small numeric `ReasonCode` enum (large-transfer pattern, repeated-transaction abuse, abnormal frequency, flagged-counterparty interaction, unusual asset movement, confirmed fraud, other) plus the ledger timestamp the flag was set.
- **Writes** (`flag`, `unflag`, `set_admin`) require `require_auth` from a single configured admin address — in practice, StellarTrace's operational signing key. They are only ever invoked *after* a human investigator has confirmed an account suspicious off-chain (see `handlers::submit_decision` in the API, which is the trigger point).
- **Reads** (`is_flagged`, `get_flag`) are public and unauthenticated, so any other Soroban contract or off-chain service in the ecosystem can cheaply check an address without needing access to StellarTrace's private investigation database.
- **No sensitive data on-chain.** No transaction details, AI analysis, investigator identities, or notes are ever written to this contract. All of that lives only in the off-chain, tamper-evident audit log (see [audit-model.md](audit-model.md)).

### Building and testing the contract

```bash
cd contracts/flagged_accounts
cargo test                                                # native unit tests
cargo build --release --target wasm32-unknown-unknown     # wasm artifact
```

### Deploying (outline)

See `contracts/flagged_accounts/scripts/deploy.sh` for a template using `soroban-cli` / `stellar-cli` against a configured network (local, testnet, or futurenet). The script is intentionally a starting point — production deployment should go through your organization's standard signing and change-management process, not be run ad hoc.
