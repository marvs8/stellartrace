# Changelog

All notable changes to this project are documented in this file. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added
- Expanded `docs/` tree (architecture, Stellar/Soroban integration, AI boundary, human-in-the-loop, audit model, API reference, authorization, observability, configuration, persistence, testing strategy, security, threat model, runbook, data retention) plus architecture decision records under `docs/adr/`.
- `stellartrace-storage` crate: optional JSON-file snapshot persistence for alerts and audit records.
- New fraud detection rules: dormant account reactivation, round-trip wash trading, new-account high-value outflow, cross-asset rapid conversion.
- TOML-based rule threshold configuration (`config/default_rules.toml`, `STELLARTRACE_RULES_CONFIG`).
- `/metrics` endpoint (Prometheus text exposition) and internal counters.
- Criterion benchmarks for the rules engine and scoring.
- CI workflows (build/test, fmt/clippy, Soroban contract build), issue/PR templates, CODEOWNERS.
- Governance and community files: `LICENSE`, `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`.
- Developer tooling: `rustfmt.toml`, `clippy.toml`, `Makefile`, `.editorconfig`, `.pre-commit-config.yaml`, `.env.example`.
- Deployment assets: `Dockerfile`, `docker-compose.yml`, systemd unit, Soroban contract deploy/invoke scripts.
- `examples/` directory: HTTP request collection, OpenAPI 3.0 spec, Postman collection.

## [0.1.0] - 2026-09-15

### Added
- Initial release: deterministic rules engine, anomaly scoring, alert management, advisory-only AI investigation layer, strict human-in-the-loop decision workflow, tamper-evident audit trail, Soroban flagged-accounts registry, axum API, and static investigator dashboard.
