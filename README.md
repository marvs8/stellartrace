# StellarTrace

[![CI](https://github.com/marvs8/stellartrace/actions/workflows/ci.yml/badge.svg)](https://github.com/marvs8/stellartrace/actions/workflows/ci.yml)
[![Soroban Contract CI](https://github.com/marvs8/stellartrace/actions/workflows/contract-ci.yml/badge.svg)](https://github.com/marvs8/stellartrace/actions/workflows/contract-ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A production-oriented fraud and anomaly detection triage system for the Stellar ecosystem: it ingests Stellar/Soroban transaction activity, runs it through a deterministic and explainable rules engine, scores and files alerts, offers an **advisory-only** AI investigation assistant, and enforces a strict human-in-the-loop decision workflow with a tamper-evident audit trail.

StellarTrace is not a simple CRUD app. Its design center is: **explainability, deterministic detection, auditability, graceful failure handling, and a hard boundary between AI recommendations and human decisions.**

## Quick start

```bash
cargo build --workspace
cargo run -p stellartrace-api
```

Then open `dashboard/index.html` in a browser and point it at `http://localhost:8080`. See [docs/setup.md](docs/setup.md) for environment variables and a full smoke test.

## Documentation

| Doc | Covers |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Crate map, data flow diagram, design principles |
| [docs/stellar-soroban-integration.md](docs/stellar-soroban-integration.md) | Horizon ingestion, the on-chain flagged-accounts registry, why Soroban is used only there |
| [docs/fraud-detection-flow.md](docs/fraud-detection-flow.md) | End-to-end pipeline from ingestion to audit |
| [docs/rules-catalog.md](docs/rules-catalog.md) | Every detection rule, what it does, how to add a new one |
| [docs/configuration.md](docs/configuration.md) | Tuning thresholds via TOML |
| [docs/ai-boundary.md](docs/ai-boundary.md) | Why and how the AI can never make a final decision |
| [docs/human-in-the-loop.md](docs/human-in-the-loop.md) | Investigator decision workflow |
| [docs/audit-model.md](docs/audit-model.md) | Append-only, hash-chained audit trail |
| [docs/api-reference.md](docs/api-reference.md) | Every endpoint |
| [docs/authorization.md](docs/authorization.md) | Roles, tokens, auth extension points |
| [docs/observability.md](docs/observability.md) | Structured logging and metrics |
| [docs/persistence.md](docs/persistence.md) | Optional snapshot-based durability |
| [docs/setup.md](docs/setup.md) | Build, run, environment variables |
| [docs/testing-strategy.md](docs/testing-strategy.md) | What's tested and why |
| [docs/security.md](docs/security.md) | Security posture, failure handling, known limitations |
| [docs/threat-model.md](docs/threat-model.md) | Assets, threats, mitigations |
| [docs/runbook.md](docs/runbook.md) | Operational procedures for common incidents |
| [docs/data-retention-policy.md](docs/data-retention-policy.md) | Retention policy template |
| [docs/adr/](docs/adr/) | Architecture decision records |

## Repository layout

```
crates/
  common/          shared domain types (Alert, TriggeredRule, AiRecommendation, ...)
  ingestion/       Stellar/Soroban transaction ingestion + retry handling
  rules_engine/    deterministic, explainable fraud rules
  scoring/         anomaly scoring
  alerts/          alert lifecycle management
  ai_advisor/      advisory-only AI investigation layer
  audit/           append-only, tamper-evident audit log
  storage/         optional snapshot-based persistence
  api/             axum backend: routes, auth, metrics
contracts/
  flagged_accounts/  Soroban on-chain flagged-accounts registry
dashboard/         investigator UI (static HTML/JS)
docs/              architecture, security, ops, and ADR documentation
config/            sample rule-threshold configuration
examples/          HTTP request collection, OpenAPI spec, Postman collection
scripts/           local development helper scripts
deploy/            Docker, systemd, and deployment notes
.github/           CI workflows, issue/PR templates
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Please review [SECURITY.md](SECURITY.md) before reporting a vulnerability.

## License

[MIT](LICENSE)
