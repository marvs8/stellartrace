# API Reference

All endpoints are prefixed `/api` except `/health` and `/metrics`. The server is implemented in `crates/api` (axum).

| Method | Path | Purpose | Auth |
|---|---|---|---|
| GET | `/health` | Liveness check | none |
| GET | `/metrics` | Prometheus-text exposition of internal counters | none* |
| POST | `/api/transactions` | Ingest a `NormalizedTransaction` and immediately evaluate it against the rules engine (creates an alert if triggered) | none* |
| GET | `/api/transactions/:tx_hash` | Retrieve a previously ingested transaction | none* |
| POST | `/api/transactions/:tx_hash/evaluate` | Re-evaluate an already-ingested transaction against current rules/config | none* |
| GET | `/api/alerts` | List alerts, optional `?status=open\|under_investigation\|escalated\|dismissed\|confirmed_suspicious` | none* |
| GET | `/api/alerts/:alert_id` | Retrieve one alert | none* |
| GET | `/api/alerts/:alert_id/ai-investigation` | Get (generating if needed) the advisory AI recommendation | none* |
| POST | `/api/alerts/:alert_id/decision` | Submit the final human decision | **Investigator/Admin** |
| GET | `/api/alerts/:alert_id/audit` | Full audit trail for one alert | none* |
| GET | `/api/audit/verify` | Verify the whole audit chain's integrity | none* |

\* In this reference implementation, only the state-changing decision endpoint enforces authorization, matching the requirement that only authorized investigators can change alert status. In a production deployment, put read endpoints behind at-least-Viewer auth too — the `AuthUser` extractor (`crates/api/src/auth.rs`) is already reusable on any handler; add it as a parameter and the handler is authenticated.

## Example requests

See `examples/requests.http` for a ready-to-run collection (compatible with the VS Code REST Client extension and JetBrains HTTP client), and `examples/openapi.yaml` for a machine-readable OpenAPI 3.0 description. A Postman collection is also provided at `examples/postman_collection.json`.

## Error shape

Errors are returned as `{"error": "<message>"}` with an appropriate HTTP status code (`400`, `401`, `403`, `404`, `500`). See `crates/api/src/error.rs`.
