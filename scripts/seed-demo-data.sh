#!/usr/bin/env bash
# Seeds a running StellarTrace API with a handful of demo transactions that
# trigger different rules, so the dashboard has something to show.
# Usage: STELLARTRACE_API=http://localhost:8080 ./scripts/seed-demo-data.sh
set -euo pipefail

API="${STELLARTRACE_API:-http://localhost:8080}"
NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

post_tx() {
  curl -s -X POST "$API/api/transactions" \
    -H 'content-type: application/json' \
    -d "$1" | python3 -m json.tool
  echo "---"
}

echo "Seeding a large-transfer alert..."
post_tx '{
  "tx_hash": "demo-large-transfer",
  "ledger_sequence": 1001,
  "source_account": "GDEMOALICE000000000000000000000000000000000000000000000",
  "destination_account": "GDEMOBOB0000000000000000000000000000000000000000000000",
  "asset": {"type": "native"},
  "amount": "50000",
  "timestamp": "'"$NOW"'",
  "is_soroban_invocation": false,
  "contract_id": null,
  "metadata": {}
}'

echo "Seeding a Soroban invocation with a moderate amount..."
post_tx '{
  "tx_hash": "demo-soroban-invocation",
  "ledger_sequence": 1002,
  "source_account": "GDEMOCAROL00000000000000000000000000000000000000000000",
  "destination_account": null,
  "asset": {"type": "native"},
  "amount": "250",
  "timestamp": "'"$NOW"'",
  "is_soroban_invocation": true,
  "contract_id": "CDEMOCONTRACT000000000000000000000000000000000000000000",
  "metadata": {"function_name": "swap"}
}'

echo "Done. Open dashboard/index.html and connect to $API"
