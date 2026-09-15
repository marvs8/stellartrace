# ADR 0004: Append-Only, Hash-Chained Audit Log

## Status
Accepted

## Context
The audit trail is the system's proof that detection, AI advisory, and human decisions happened the way they're claimed to have happened. It must never allow a status change or a triggered rule to be quietly rewritten after the fact — especially not a superseded investigator decision, which is exactly the kind of record someone under scrutiny might be motivated to alter.

Relying solely on "the database doesn't grant UPDATE/DELETE to the application role" is a reasonable operational control, but it's invisible to the application itself — there's no way for the system (or an auditor holding an export of the log) to *verify* that no tampering occurred without trusting the database administrator's configuration to have never changed.

## Decision
`AuditLog` (`crates/audit/src/lib.rs`) never exposes a mutation or deletion method on `AuditRecord` — this is enforced by the crate's public API, not just a database grant. Additionally, each record embeds a SHA-256 hash chain (`prev_hash`/`this_hash`) over its own content and its predecessor's hash, so `verify_integrity()` can detect — from the data itself, independent of storage-layer permissions — whether any past record was altered, reordered, or removed.

## Consequences
- **Positive**: tamper evidence is a property of the data, checkable by anyone holding a copy of the log (e.g. an exported snapshot handed to an external auditor), not just an operational promise about database configuration.
- **Negative**: the hash chain does not by itself *prevent* tampering by someone with direct write access to the underlying storage — it makes such tampering detectable after the fact. Preventing it outright would require append-only storage guarantees (e.g. object-lock/WORM storage, or an on-chain checkpoint of the chain's latest hash) layered on top; the interface (`append`, `for_alert`, `verify_integrity`) does not need to change to add that layer later.
- **Negative**: verifying integrity is O(n) in the number of records; acceptable for a per-alert audit trail and periodic full-log checks, but a very large single log should be checkpointed/rotated rather than verified from genesis every time in a hot path.
