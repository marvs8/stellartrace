# Operational Runbook

## Audit integrity failure

**Symptom:** `GET /api/audit/verify` returns `{"intact": false, "detail": "..."}`.

This means a previously-written audit record was altered, reordered, or removed. Treat it as a security incident:

1. Freeze write access to the audit storage backend immediately (in the reference in-memory/JSON-snapshot implementation: stop the API process to prevent further writes over the corrupted state).
2. Identify the affected record from the `detail` field's index.
3. Restore from the most recent known-good snapshot (`crates/storage`, if `STELLARTRACE_DATA_DIR` is configured) or backup, and re-run `verify_integrity()` against the restored copy before resuming traffic.
4. Investigate access logs for who had write access to the storage backend in the affected window.
5. Once root-caused, document the incident — the irony of losing audit trail *about* an audit trail failure is exactly why step 4 matters.

## AI service degraded or down

**Symptom:** `stellartrace_ai_fallback_total` rate rising, or investigators reporting AI summaries all say "AI investigation service was unavailable."

1. Check `ANTHROPIC_API_KEY` is set and valid, and that outbound network access to `api.anthropic.com` is not blocked.
2. This is not an emergency for detection: the rules engine and alerting are fully unaffected (see [ai-boundary.md](ai-boundary.md#failure-handling)). Communicate to investigators that AI summaries are temporarily unavailable and manual review of triggered rules/evidence is required in the interim.
3. Once restored, no backfill is needed — the AI advisory endpoint generates on demand per alert, not proactively.

## Ingestion appears stalled

**Symptom:** `stellartrace_transactions_ingested_total` flat for longer than the expected Horizon polling interval.

1. Check ingestion logs for repeated `ingestion source exhausted retries` errors.
2. Check `IngestionPipeline::dead_letters()` (if wired to an inspection endpoint in your deployment) for batches that failed all retry attempts.
3. Verify the configured Horizon base URL is reachable and not rate-limiting the polling account.
4. Restart the ingestion task once the upstream issue is resolved; polling resumes from the last successfully advanced cursor, so no manual replay is normally needed.

## Rotating the on-chain registry admin key

1. Sign a `set_admin(new_admin)` call from the *current* admin key against the deployed `flagged_accounts` contract.
2. Update the operational signing key used by whatever off-chain process calls `flag`/`unflag` (this is not automated by the API in the reference implementation — see [human-in-the-loop.md](human-in-the-loop.md#effects-of-a-confirmedsuspicious-decision)).
3. Confirm the old key can no longer call `flag`/`unflag` by attempting a read-only dry run (`is_flagged`, which doesn't require auth, will still work — verify the *write* path specifically fails for the old key).
