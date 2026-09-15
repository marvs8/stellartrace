#!/usr/bin/env bash
# Template for flagging an account on a deployed flagged_accounts contract.
# This should only ever be run as the automated final step of a human
# investigator's ConfirmedSuspicious decision (see docs/human-in-the-loop.md)
# — never as an independent or automatic process.
set -euo pipefail

NETWORK="${STELLAR_NETWORK:-testnet}"
SOURCE_IDENTITY="${STELLAR_SOURCE_IDENTITY:?Set STELLAR_SOURCE_IDENTITY to the admin CLI identity}"
CONTRACT_ID="${FLAGGED_ACCOUNTS_CONTRACT_ID:?Set FLAGGED_ACCOUNTS_CONTRACT_ID to the deployed contract id}"
ACCOUNT_TO_FLAG="${1:?Usage: invoke-flag.sh <account_address> [reason_code]}"
REASON_CODE="${2:-ConfirmedFraud}"

echo "Flagging $ACCOUNT_TO_FLAG with reason $REASON_CODE on $NETWORK..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOURCE_IDENTITY" \
  --network "$NETWORK" \
  -- flag --account "$ACCOUNT_TO_FLAG" --reason "$REASON_CODE"

echo "Done."
