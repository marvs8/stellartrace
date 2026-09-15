#!/usr/bin/env bash
# Calls the audit integrity endpoint and exits non-zero if the chain is
# not intact — suitable for a monitoring cron job or CI health check.
# Usage: STELLARTRACE_API=http://localhost:8080 ./scripts/verify-audit.sh
set -euo pipefail

API="${STELLARTRACE_API:-http://localhost:8080}"

result="$(curl -sf "$API/api/audit/verify")"
echo "$result"

intact="$(echo "$result" | python3 -c 'import json,sys; print(json.load(sys.stdin)["intact"])')"

if [ "$intact" != "True" ]; then
  echo "AUDIT INTEGRITY FAILURE — see docs/runbook.md#audit-integrity-failure" >&2
  exit 1
fi

echo "Audit chain intact."
