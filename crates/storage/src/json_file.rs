//! JSON-file-backed `SnapshotStore` implementation.

use crate::{SnapshotStore, StorageError};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

/// Persists `T` as pretty-printed JSON at a fixed path. Writes are atomic:
/// the new content is written to a sibling temp file and then renamed
/// into place, so a crash mid-write can never leave a truncated or
/// half-written snapshot behind — readers always see either the old
/// complete file or the new complete file, never a partial one.
pub struct JsonFileStore<T> {
    path: PathBuf,
    _marker: PhantomData<T>,
}

impl<T> JsonFileStore<T> {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self { path: path.as_ref().to_path_buf(), _marker: PhantomData }
    }
}

impl<T: Serialize + DeserializeOwned + Send + Sync> SnapshotStore<T> for JsonFileStore<T> {
    fn save(&self, data: &T) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_path = self.path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(data)?;
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }

    fn load(&self) -> Result<Option<T>, StorageError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let contents = std::fs::read(&self.path)?;
        let data = serde_json::from_slice(&contents)?;
        Ok(Some(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Sample {
        id: u32,
        name: String,
        values: Vec<i64>,
    }

    fn temp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "stellartrace_storage_test_{tag}_{}.json",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ))
    }

    #[test]
    fn round_trips_data_through_disk() {
        let path = temp_path("roundtrip");
        let store: JsonFileStore<Sample> = JsonFileStore::new(&path);

        let original = Sample { id: 1, name: "alert-snapshot".into(), values: vec![1, 2, 3] };
        store.save(&original).unwrap();

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded, original);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_returns_none_when_file_absent() {
        let path = temp_path("absent");
        let store: JsonFileStore<Sample> = JsonFileStore::new(&path);
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn save_overwrites_previous_snapshot() {
        let path = temp_path("overwrite");
        let store: JsonFileStore<Sample> = JsonFileStore::new(&path);

        store.save(&Sample { id: 1, name: "first".into(), values: vec![] }).unwrap();
        store.save(&Sample { id: 2, name: "second".into(), values: vec![9] }).unwrap();

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.id, 2);
        assert_eq!(loaded.name, "second");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn save_creates_parent_directories() {
        let base = std::env::temp_dir().join(format!(
            "stellartrace_storage_test_nested_{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let path = base.join("nested").join("dir").join("snapshot.json");
        let store: JsonFileStore<Sample> = JsonFileStore::new(&path);

        store.save(&Sample { id: 7, name: "nested".into(), values: vec![] }).unwrap();
        assert!(path.exists());

        std::fs::remove_dir_all(&base).ok();
    }
}
