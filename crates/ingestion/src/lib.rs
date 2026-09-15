//! Stellar/Soroban transaction ingestion.
//!
//! This crate is the only part of StellarTrace that talks to the network
//! (Horizon REST API today; a Soroban RPC event stream fits the same
//! `TransactionSource` trait). Everything downstream consumes
//! `stellartrace_common::NormalizedTransaction`, so swapping or adding a
//! source never touches the rules engine, scoring, or alerting code.

pub mod horizon;
pub mod normalize;
pub mod pipeline;

pub use pipeline::{IngestionError, IngestionPipeline, TransactionSource};
