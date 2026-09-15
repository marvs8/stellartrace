#!/usr/bin/env bash
# Runs the StellarTrace API locally with development-friendly defaults.
# Not for production use — see docs/authorization.md before deploying.
set -euo pipefail

cd "$(dirname "$0")/.."

export STELLARTRACE_BIND_ADDR="${STELLARTRACE_BIND_ADDR:-0.0.0.0:8080}"
export STELLARTRACE_API_TOKENS="${STELLARTRACE_API_TOKENS:-dev-investigator-token:investigator:dev,dev-admin-token:admin:dev-admin,dev-viewer-token:viewer:dev-viewer}"
export RUST_LOG="${RUST_LOG:-info}"

echo "Starting StellarTrace API on ${STELLARTRACE_BIND_ADDR}"
echo "Investigator token: dev-investigator-token"
cargo run -p stellartrace-api
