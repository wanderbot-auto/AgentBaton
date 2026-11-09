//! Process-local background identity. Reading this view performs no recovery,
//! dispatch, stop, reconciliation or database write.
use crate::{
    storage::{db_error, now, Store},
    CoreError, Result,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(crate) struct BackgroundIdentity {
    instance_id: String,
    started_at: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundStatus {
    pub instance_id: String,
    pub started_at: String,
    pub api_version: u32,
    pub database_schema_version: u32,
    pub sequence: u64,
    pub owned_provider_workers: bool,
    pub store_recovery: String,
}
impl BackgroundIdentity {
    /// Call only after Store::open recovery succeeds and before scheduling.
    pub(crate) fn after_store_open() -> Result<Self> {
        Ok(Self {
            instance_id: Uuid::new_v4().to_string(),
            started_at: now()?,
        })
    }
    pub(crate) fn read(
        &self,
        store: &Store,
        owned_provider_workers: bool,
    ) -> Result<BackgroundStatus> {
        let schema: u32 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(db_error)?;
        if schema != 18 {
            return Err(CoreError::new(
                "VERSION_UNSUPPORTED",
                "Background status requires current database schema18",
            ));
        }
        Ok(BackgroundStatus {
            instance_id: self.instance_id.clone(),
            started_at: self.started_at.clone(),
            api_version: crate::ipc::API_VERSION,
            database_schema_version: schema,
            sequence: store.sequence()?,
            owned_provider_workers,
            store_recovery: "completed_on_start".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{encode, TaskState};
    #[test]
    fn two_clients_observe_same_identity_without_database_or_unknown_changes() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(&dir.path().join("db")).unwrap();
        let mut task = store
            .create("p", "unknown remains unresolved", "fixture")
            .unwrap();
        task.state = TaskState::OutcomeUnknown;
        let document = encode(&task).unwrap();
        store
            .conn
            .execute(
                "UPDATE tasks SET document=?1 WHERE id=?2",
                rusqlite::params![document, task.id],
            )
            .unwrap();
        let identity = BackgroundIdentity::after_store_open().unwrap();
        let changes = store.conn.total_changes();
        let sequence = store.sequence().unwrap();
        let first = identity.read(&store, false).unwrap();
        let second = identity.read(&store, false).unwrap();
        assert_eq!(first, second);
        assert!(Uuid::parse_str(&first.instance_id).is_ok());
        assert!(!first.started_at.is_empty());
        assert_eq!(first.sequence, sequence);
        assert_eq!(first.store_recovery, "completed_on_start");
        let owned = identity.read(&store, true).unwrap();
        assert_eq!(owned.instance_id, first.instance_id);
        assert!(owned.owned_provider_workers);
        assert_eq!(store.conn.total_changes(), changes);
        assert_eq!(store.sequence().unwrap(), sequence);
        assert_eq!(encode(&store.get(&task.id).unwrap()).unwrap(), document);
        drop(store);
        let store = Store::open(&dir.path().join("db")).unwrap();
        let restarted = BackgroundIdentity::after_store_open()
            .unwrap()
            .read(&store, false)
            .unwrap();
        assert_ne!(first.instance_id, restarted.instance_id);
        assert_eq!(
            store.get(&task.id).unwrap().state,
            TaskState::OutcomeUnknown
        );
    }
    #[test]
    fn unexpected_schema_is_not_reported_as_current() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("db")).unwrap();
        let identity = BackgroundIdentity::after_store_open().unwrap();
        store.conn.pragma_update(None, "user_version", 19).unwrap();
        assert_eq!(
            identity.read(&store, false).unwrap_err().code,
            "VERSION_UNSUPPORTED"
        );
    }
}
