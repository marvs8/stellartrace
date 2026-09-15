#!/usr/bin/env bash
# Template for deploying the flagged_accounts contract with the Stellar CLI
# (`stellar` / `soroban` CLI). Review network, identity, and admin address
# before running against anything beyond a local/testnet sandbox — this is
# a starting point, not a change-managed production deploy pipeline.
set -euo pipefail

NETWORK="${STELLAR_NETWORK:-testnet}"
SOURCE_IDENTITY="${STELLAR_SOURCE_IDENTITY:?Set STELLAR_SOURCE_IDENTITY to your configured CLI identity}"
ADMIN_ADDRESS="${FLAGGED_ACCOUNTS_ADMIN:?Set FLAGGED_ACCOUNTS_ADMIN to the admin account's public key}"

cd "$(dirname "$0")/.."

echo "Building contract..."
cargo build --release --target wasm32-unknown-unknown

WASM_PATH="target/wasm32-unknown-unknown/release/flagged_accounts_registry.wasm"

echo "Deploying to network: $NETWORK"
CONTRACT_ID="$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --source "$SOURCE_IDENTITY" \
  --network "$NETWORK")"

echo "Deployed contract id: $CONTRACT_ID"

echo "Initializing with admin: $ADMIN_ADDRESS"
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --source "$SOURCE_IDENTITY" \
  --network "$NETWORK" \
  -- initialize --admin "$ADMIN_ADDRESS"

echo "Done. Save this contract id for scripts/invoke-flag.sh: $CONTRACT_ID"
