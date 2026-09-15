# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in StellarTrace, please report it privately rather than opening a public GitHub issue. Use GitHub's private vulnerability reporting feature on this repository (Security tab → "Report a vulnerability"), or contact the maintainers directly.

Please include:
- A description of the vulnerability and its potential impact.
- Steps to reproduce (a minimal repro is very helpful).
- Any relevant logs or stack traces (redact secrets/tokens first).

We will acknowledge receipt as quickly as we can and keep you updated as we investigate and remediate.

## Scope

Areas of particular interest for security review, given this project's design constraints (see [docs/threat-model.md](docs/threat-model.md)):
- Any path by which AI-generated content could influence `Alert::status` or otherwise be treated as an executed action rather than advisory text (see [docs/ai-boundary.md](docs/ai-boundary.md)).
- Authorization bypasses on `POST /api/alerts/:id/decision` or any future state-changing endpoint.
- Audit log tampering that `AuditLog::verify_integrity()` fails to detect.
- Issues in the `contracts/flagged_accounts` Soroban contract, particularly around the `require_auth` admin gating.

## Supported Versions

This is a reference implementation under active development; security fixes are applied to the `main` branch. There is no separate long-term-support branch at this time.
