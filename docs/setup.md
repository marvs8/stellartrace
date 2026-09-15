# Setup

## Requirements

- Rust stable (edition 2021).
- For the Soroban contract: the `wasm32-unknown-unknown` target — `rustup target add wasm32-unknown-unknown`.

## Build & run

```bash
cargo build --workspace
cargo run -p stellartrace-api
```

Then open `dashboard/index.html` directly in a browser (or serve it with any static file server, e.g. `python3 -m http.server` from the `dashboard/` directory) and point it at the API base URL shown in the header.

## Environment variables

| Variable | Purpose | Default |
|---|---|---|
| `STELLARTRACE_BIND_ADDR` | API bind address | `0.0.0.0:8080` |
| `STELLARTRACE_API_TOKENS` | `token:role:investigator_id,...` — see [authorization.md](authorization.md) | falls back to a single insecure `dev-investigator-token` (Investigator role), with a loud warning logged |
| `ANTHROPIC_API_KEY` | Enables the real Claude-backed advisor | unset → AI endpoint returns fallback (`model_available: false`) advisory responses only |
| `STELLARTRACE_RULES_CONFIG` | Path to a TOML file overriding rule thresholds — see [configuration.md](configuration.md) | built-in defaults |
| `STELLARTRACE_DATA_DIR` | Directory for periodic alert/audit snapshot persistence — see [persistence.md](persistence.md) | unset → in-memory only, no snapshots |
| `RUST_LOG` | `tracing_subscriber::EnvFilter` string | `info` |

A template is provided at [`.env.example`](../.env.example).

## Quick smoke test

```bash
curl -s -X POST localhost:8080/api/transactions \
  -H 'content-type: application/json' \
  -d '{"tx_hash":"demo1","ledger_sequence":1,"source_account":"GALICE","destination_account":"GBOB","asset":{"type":"native"},"amount":"25000","timestamp":"2026-01-01T00:00:00Z","is_soroban_invocation":false,"contract_id":null,"metadata":{}}'
```

This should return a `202`-style JSON body with `triggered_rules` including `large_transfer` and a created `alert`. See `examples/requests.http` for a fuller walkthrough including the AI advisory and decision endpoints.
