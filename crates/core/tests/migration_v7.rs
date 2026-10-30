#[path = "fixtures/pre_factory_schema.rs"]
pub mod pre_factory_schema;
use agentbaton_core::storage::Store;
use rusqlite::{params, Connection};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

fn private_database() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let database = directory.path().join("state.sqlite3");
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&database)
        .unwrap();
    assert_eq!(
        fs::metadata(&database).unwrap().permissions().mode() & 0o777,
        0o600
    );
    (directory, database)
}
fn v6_fixture(database: &Path) -> (String, String) {
    let mut store = Store::open(database).unwrap();
    let task = store
        .create("local", "Preserve raw v6 state", "original")
        .unwrap();
    drop(store);
    let connection = Connection::open(database).unwrap();
    pre_factory_schema::remove_empty_current_factory_increment(&connection).unwrap();
    connection.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;DROP TABLE data_validation_runs;DROP TABLE data_validation_policies;CREATE TABLE legacy_raw(id TEXT PRIMARY KEY,payload BLOB NOT NULL);PRAGMA user_version=6;").unwrap();
    let value: serde_json::Value = serde_json::from_str(
        &connection
            .query_row("SELECT document FROM tasks WHERE id=?1", [&task.id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
    )
    .unwrap();
    let raw = serde_json::to_string_pretty(&value).unwrap() + "\n";
    connection
        .execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![raw, task.id],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO legacy_raw VALUES('raw',?1)",
            [b"\0\xfforiginal\nbytes".as_slice()],
        )
        .unwrap();
    (task.id, raw)
}
fn backups(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| {
            p.file_name().unwrap().to_string_lossy().contains("pre-v7")
                && p.extension().is_some_and(|x| x == "sqlite3")
        })
        .collect()
}
#[test]
fn real_v6_upgrade_keeps_raw_documents_and_blobs_in_private_reopenable_backup() {
    let (directory, database) = private_database();
    let (task, raw) = v6_fixture(&database);
    let store = Store::open(&database).unwrap();
    assert_eq!(store.get(&task).unwrap().goal, "Preserve raw v6 state");
    assert!(store
        .list_data_validation_policies(&task)
        .unwrap()
        .is_empty());
    assert!(store
        .list_data_validation_records(&task)
        .unwrap()
        .is_empty());
    drop(store);
    let connection = Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        connection
            .query_row("SELECT document FROM tasks WHERE id=?1", [&task], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    assert_eq!(
        connection
            .query_row("SELECT payload FROM legacy_raw WHERE id='raw'", [], |r| r
                .get::<_, Vec<
                u8,
            >>(
                0
            ))
            .unwrap(),
        b"\0\xfforiginal\nbytes"
    );
    drop(connection);
    let backups = backups(directory.path());
    assert_eq!(backups.len(), 1);
    assert_eq!(
        fs::metadata(&backups[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&backups[0]).unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        backup
            .query_row("SELECT document FROM tasks WHERE id=?1", [&task], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
            .as_bytes(),
        raw.as_bytes()
    );
    assert_eq!(
        backup
            .query_row("SELECT payload FROM legacy_raw WHERE id='raw'", [], |r| r
                .get::<_, Vec<
                u8,
            >>(
                0
            ))
            .unwrap(),
        b"\0\xfforiginal\nbytes"
    );
    assert_eq!(backup.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('data_validation_policies','data_validation_runs')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    drop(backup);
    drop(Store::open(&database).unwrap());
    assert_eq!(self::backups(directory.path()).len(), 1);
}
#[test]
fn failed_data_migration_rolls_back_new_table_and_version_without_deleting_unknown_data() {
    let (_directory, database) = private_database();
    let (task, raw) = v6_fixture(&database);
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE data_validation_runs(sentinel TEXT);INSERT INTO data_validation_runs VALUES('preserve unknown v6 table');").unwrap();
    drop(connection);
    assert!(Store::open(&database).is_err());
    let connection = Connection::open(&database).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        6
    );
    assert_eq!(
        connection
            .query_row("SELECT sentinel FROM data_validation_runs", [], |r| r
                .get::<_, String>(0))
            .unwrap(),
        "preserve unknown v6 table"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name='data_validation_policies'",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT document FROM tasks WHERE id=?1", [&task], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
        raw
    );
}
#[test]
fn future_v18_database_is_refused_without_changing_its_raw_bytes() {
    let (_directory, database) = private_database();
    let connection = Connection::open(&database).unwrap();
    connection.execute_batch("CREATE TABLE sentinel(raw BLOB);INSERT INTO sentinel VALUES(x'00ff');PRAGMA user_version=18;").unwrap();
    drop(connection);
    let before = fs::read(&database).unwrap();
    assert_eq!(
        Store::open(&database).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    assert_eq!(fs::read(&database).unwrap(), before);
}
