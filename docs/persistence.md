# Persistence

By default, StellarTrace's `AlertManager` and `AuditLog` hold state entirely in memory, guarded by `RwLock`s. This is intentional for a reference implementation focused on the detection/AI/human-review logic rather than storage engineering — but it means a process restart loses all alerts and audit history unless persistence is enabled.

## `stellartrace-storage`

`crates/storage` provides a minimal, dependency-light snapshot mechanism:

- `SnapshotStore<T>` — a trait for "write the current state of `T` to durable storage" / "read it back."
- `JsonFileStore` — a concrete implementation that serializes to a JSON file on disk via `serde_json`, with an atomic write (write to a temp file, then rename) so a crash mid-write cannot corrupt the existing snapshot.

## How the API uses it

If `STELLARTRACE_DATA_DIR` is set, `stellartrace-api`:
1. On startup, attempts to load `alerts.json` and `audit.json` from that directory and seed `AppState` with their contents before accepting traffic.
2. Spawns a background task that snapshots current alerts and audit records to those files on a fixed interval.

If `STELLARTRACE_DATA_DIR` is unset, the server behaves exactly as it did before this feature existed: pure in-memory, no snapshot files touched.

## What this is not

This is a durability *aid*, not a transactional database. It does not provide concurrent multi-writer consistency, point-in-time recovery, or partial-failure atomicity across the two snapshot files. For a production deployment, replace `AlertManager`'s and `AuditLog`'s internal storage with a real database (e.g. Postgres) behind the same public interfaces — `create_alert`, `apply_investigator_decision`, `append`, `for_alert`, `verify_integrity` — none of which need to change shape to support that swap.
