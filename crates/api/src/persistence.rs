//! Wires `stellartrace-storage` into the API: restores alerts and audit
//! records from disk at startup (if `STELLARTRACE_DATA_DIR` is set), and
//! periodically snapshots current state back to disk. See
//! `docs/persistence.md` for what this does and doesn't guarantee.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use stellartrace_audit::AuditRecord;
use stellartrace_common::Alert;
use stellartrace_storage::{JsonFileStore, SnapshotStore};

use crate::state::AppState;

const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(30);

pub struct PersistencePaths {
    pub alerts: PathBuf,
    pub audit: PathBuf,
}

impl PersistencePaths {
    pub fn new(data_dir: &str) -> Self {
        let base = PathBuf::from(data_dir);
        Self {
            alerts: base.join("alerts.json"),
            audit: base.join("audit.json"),
        }
    }
}

/// Restores state from disk into `state`, if snapshot files exist. Safe
/// to call even if the directory or files don't exist yet — that's just
/// treated as "nothing to restore," not an error, so a fresh deployment
/// starts cleanly.
pub fn restore_on_startup(state: &Arc<AppState>, paths: &PersistencePaths) {
    let alert_store: JsonFileStore<Vec<Alert>> = JsonFileStore::new(&paths.alerts);
    match alert_store.load() {
        Ok(Some(alerts)) => {
            let count = alerts.len();
            state.alerts.restore(alerts);
            tracing::info!(count, "restored alerts from snapshot");
        }
        Ok(None) => tracing::info!("no alert snapshot found; starting empty"),
        Err(e) => tracing::warn!(error = %e, "failed to load alert snapshot; starting empty"),
    }

    let audit_store: JsonFileStore<Vec<AuditRecord>> = JsonFileStore::new(&paths.audit);
    match audit_store.load() {
        Ok(Some(records)) => {
            let count = records.len();
            state.audit.restore(records);
            match state.audit.verify_integrity() {
                Ok(()) => tracing::info!(
                    count,
                    "restored audit log from snapshot; integrity verified"
                ),
                Err(e) => tracing::error!(
                    error = %e,
                    "restored audit log FAILED integrity verification — see docs/runbook.md#audit-integrity-failure"
                ),
            }
        }
        Ok(None) => tracing::info!("no audit snapshot found; starting empty"),
        Err(e) => tracing::warn!(error = %e, "failed to load audit snapshot; starting empty"),
    }
}

/// Spawns a background task that periodically snapshots current alerts
/// and audit records to disk. Runs for the lifetime of the process.
pub fn spawn_periodic_snapshot(state: Arc<AppState>, paths: PersistencePaths) {
    tokio::spawn(async move {
        let alert_store: JsonFileStore<Vec<Alert>> = JsonFileStore::new(&paths.alerts);
        let audit_store: JsonFileStore<Vec<AuditRecord>> = JsonFileStore::new(&paths.audit);

        loop {
            tokio::time::sleep(SNAPSHOT_INTERVAL).await;

            let alerts = state.alerts.list();
            if let Err(e) = alert_store.save(&alerts) {
                tracing::warn!(error = %e, "failed to snapshot alerts to disk");
            }

            let records = state.audit.all();
            if let Err(e) = audit_store.save(&records) {
                tracing::warn!(error = %e, "failed to snapshot audit log to disk");
            }
        }
    });
}
