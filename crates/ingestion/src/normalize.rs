//! Event normalization: converts raw, source-specific payloads (Horizon
//! JSON payment/invoke-host-function operations) into
//! `stellartrace_common::NormalizedTransaction`.
//!
//! This is kept separate from the HTTP polling logic so unit tests can
//! feed it fixtures without any network access, and so a future Soroban
//! RPC event source can reuse the same normalization for shared shapes.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use stellartrace_common::{Asset, NormalizedTransaction};

/// Shape of a single Horizon `/operations` payment-type record, trimmed to
/// the fields StellarTrace actually needs.
#[derive(Debug, Deserialize)]
pub struct HorizonPaymentOperation {
    pub transaction_hash: String,
    pub source_account: String,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub asset_type: String,
    #[serde(default)]
    pub asset_code: Option<String>,
    #[serde(default)]
    pub asset_issuer: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub type_field_ledger: Option<u32>,
    #[serde(default)]
    pub ledger_sequence: Option<u32>,
    #[serde(default)]
    pub function_name: Option<String>,
    #[serde(default)]
    pub contract_id: Option<String>,
    #[serde(rename = "type", default)]
    pub op_type: String,
}

#[derive(Debug, thiserror::Error)]
pub enum NormalizeError {
    #[error("unparseable timestamp: {0}")]
    BadTimestamp(String),
    #[error("unsupported operation type: {0}")]
    UnsupportedOperation(String),
}

/// Normalize one Horizon operation record into the canonical transaction
/// shape used throughout the pipeline.
pub fn normalize_horizon_operation(
    op: HorizonPaymentOperation,
) -> Result<NormalizedTransaction, NormalizeError> {
    let timestamp: DateTime<Utc> = op
        .created_at
        .parse()
        .map_err(|_| NormalizeError::BadTimestamp(op.created_at.clone()))?;

    let asset = match op.asset_type.as_str() {
        "native" | "" => Asset::Native,
        _ => Asset::Credit {
            code: op.asset_code.unwrap_or_default(),
            issuer: op.asset_issuer.unwrap_or_default(),
        },
    };

    let is_soroban = op.op_type == "invoke_host_function" || op.contract_id.is_some();

    let mut metadata = HashMap::new();
    metadata.insert("op_type".to_string(), op.op_type.clone());
    if let Some(func) = &op.function_name {
        metadata.insert("function_name".to_string(), func.clone());
    }

    Ok(NormalizedTransaction {
        tx_hash: op.transaction_hash,
        ledger_sequence: op.ledger_sequence.or(op.type_field_ledger).unwrap_or(0),
        source_account: if is_soroban {
            op.from.unwrap_or(op.source_account)
        } else {
            op.from.clone().unwrap_or(op.source_account)
        },
        destination_account: op.to,
        asset,
        amount: op.amount.unwrap_or_else(|| "0".to_string()),
        timestamp,
        is_soroban_invocation: is_soroban,
        contract_id: op.contract_id,
        metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_op() -> HorizonPaymentOperation {
        HorizonPaymentOperation {
            transaction_hash: "abc123".into(),
            source_account: "GSOURCE".into(),
            to: Some("GDEST".into()),
            from: Some("GSOURCE".into()),
            asset_type: "native".into(),
            asset_code: None,
            asset_issuer: None,
            amount: Some("1000.0000000".into()),
            created_at: "2026-01-01T00:00:00Z".into(),
            type_field_ledger: None,
            ledger_sequence: Some(42),
            function_name: None,
            contract_id: None,
            op_type: "payment".into(),
        }
    }

    #[test]
    fn normalizes_native_payment() {
        let tx = normalize_horizon_operation(base_op()).unwrap();
        assert_eq!(tx.asset, Asset::Native);
        assert_eq!(tx.source_account, "GSOURCE");
        assert_eq!(tx.destination_account.as_deref(), Some("GDEST"));
        assert!(!tx.is_soroban_invocation);
    }

    #[test]
    fn normalizes_credit_asset() {
        let mut op = base_op();
        op.asset_type = "credit_alphanum4".into();
        op.asset_code = Some("USDC".into());
        op.asset_issuer = Some("GISSUER".into());
        let tx = normalize_horizon_operation(op).unwrap();
        assert_eq!(
            tx.asset,
            Asset::Credit {
                code: "USDC".into(),
                issuer: "GISSUER".into()
            }
        );
    }

    #[test]
    fn detects_soroban_invocation() {
        let mut op = base_op();
        op.op_type = "invoke_host_function".into();
        op.contract_id = Some("CCONTRACT".into());
        let tx = normalize_horizon_operation(op).unwrap();
        assert!(tx.is_soroban_invocation);
        assert_eq!(tx.contract_id.as_deref(), Some("CCONTRACT"));
    }

    #[test]
    fn rejects_bad_timestamp() {
        let mut op = base_op();
        op.created_at = "not-a-date".into();
        assert!(normalize_horizon_operation(op).is_err());
    }
}
