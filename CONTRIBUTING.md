# Contributing to StellarTrace

Thanks for considering a contribution. This project prioritizes explainability, deterministic detection, auditability, and a strict AI/human decision boundary — please read [docs/architecture.md](docs/architecture.md) and [docs/ai-boundary.md](docs/ai-boundary.md) before proposing changes that touch detection or the AI advisor.

## Development setup

```bash
git clone <your fork>
cd stellartrace
cargo build --workspace
cargo test --workspace
```

For the Soroban contract:
```bash
rustup target add wasm32-unknown-unknown
cd contracts/flagged_accounts
cargo test
```

## Before opening a pull request

- `cargo fmt --all` (see [`rustfmt.toml`](rustfmt.toml)).
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo test --workspace` — all tests must pass.
- If you added a new fraud rule, add it to [docs/rules-catalog.md](docs/rules-catalog.md) and cover it with unit tests (boundary case, non-firing case, and window-filtering if applicable — see [docs/testing-strategy.md](docs/testing-strategy.md)).
- If you touched the AI advisor or its output handling, make sure `crates/ai_advisor/src/sanitize.rs`'s tests still cover your change and that no new field on `AiRecommendation` could plausibly be read as an executable action.

## Commit style

Small, atomic commits with an imperative-mood subject line (`Add X`, `Fix Y`, not `Added X`). Reference the relevant crate or doc in the subject when it helps scanning history, e.g. `rules_engine: add dormant account reactivation rule`.

## Design constraints that pull requests must not violate

1. **The AI must never be able to write `Alert::status`.** Any change that adds a code path from `stellartrace-ai-advisor` output into alert state will be rejected regardless of how well-intentioned the confidence heuristic is. See [ADR 0002](docs/adr/0002-ai-advisory-only-boundary.md).
2. **The audit log is append-only.** Do not add an update or delete method to `AuditRecord`/`AuditLog`.
3. **No sensitive investigation data goes on-chain.** Changes to `contracts/flagged_accounts` should be reviewed against [ADR 0003](docs/adr/0003-on-chain-vs-off-chain-data-split.md).
4. **Rules must stay pure and deterministic.** A `Rule::evaluate` implementation must not perform I/O, use wall-clock "now" beyond what's passed in via the transaction/context, or otherwise be non-reproducible given the same inputs.

## Reporting security issues

Do not open a public issue for a security vulnerability — see [SECURITY.md](SECURITY.md).
