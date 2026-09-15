## Summary

What does this change do, and why?

## Checklist

- [ ] `cargo fmt --all` run
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` passes
- [ ] If a new fraud rule was added: unit tests cover the firing boundary, a non-firing case, and window filtering (if applicable); `docs/rules-catalog.md` updated
- [ ] If the AI advisor or its output handling changed: confirmed no new field on `AiRecommendation` could be read as an executable action, and sanitize tests still pass
- [ ] If the audit log changed: confirmed no update/delete path was introduced
- [ ] If `contracts/flagged_accounts` changed: confirmed no sensitive investigation data was added on-chain

## Design constraints affected (if any)

Reference the relevant [ADR](../docs/adr/) if this touches a documented design decision.

## Test plan

How did you verify this works?
