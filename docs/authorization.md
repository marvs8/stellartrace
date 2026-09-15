# Authentication & Authorization

StellarTrace uses a minimal, dependency-light bearer-token scheme (`crates/api/src/auth.rs`), configured via the `STELLARTRACE_API_TOKENS` environment variable.

## Roles

| Role | Can read | Can submit decisions |
|---|---|---|
| `Viewer` | yes | no |
| `Investigator` | yes | yes |
| `Admin` | yes | yes |

`Role::can_decide()` is the single predicate gating the one state-changing endpoint, `POST /api/alerts/:id/decision`.

## Configuring tokens

```
STELLARTRACE_API_TOKENS="token1:investigator:alice,token2:admin:bob,token3:viewer:carol"
```

Each entry is `token:role:investigator_id`. Malformed entries are skipped with a logged warning rather than crashing startup.

If `STELLARTRACE_API_TOKENS` is unset, the server falls back to a single, loudly-logged, obviously-named development token (`dev-investigator-token`, role `Investigator`). **This fallback must never be relied on outside local development** — there is no other credential rotation, expiry, or revocation mechanism attached to it.

## Extending to a real identity provider

`AuthUser` is an axum `FromRequestParts` extractor. Any handler that takes it as a parameter is authenticated. Swapping the bearer-token scheme for OAuth2/OIDC, mTLS, or an internal SSO integration means replacing `AuthRegistry::authenticate` (and the `Authorization` header parsing in the extractor) only — no handler code changes.
