//! Optional, lightweight persistence for StellarTrace.
//!
//! By default, `stellartrace-alerts` and `stellartrace-audit` hold state
//! purely in memory. This crate provides a minimal snapshot mechanism so
//! a reference deployment can survive a restart without pulling in a full
//! database dependency: serialize the current state to a JSON file on a
//! schedule, and restore it at startup.
//!
//! This is a durability *aid*, not a transactional database — see
//! `docs/persistence.md` for what it does and doesn't guarantee.

pub mod json_file;

pub use json_file::JsonFileStore;

/// A place `T` can be durably written to and read back from. Kept generic
/// and minimal so a future backend (e.g. a real database) can implement
/// the same trait without changing any caller.
pub trait SnapshotStore<T>: Send + Sync {
    fn save(&self, data: &T) -> Result<(), StorageError>;
    fn load(&self) -> Result<Option<T>, StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("io error accessing snapshot: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to (de)serialize snapshot: {0}")]
    Serde(#[from] serde_json::Error),
}
