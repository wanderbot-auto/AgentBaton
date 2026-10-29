use agentbaton_core::storage::Store;
use rusqlite::{params, Connection};
use std::{
    fs,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let p = d.path().join("state.sqlite3");
    drop(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&p)
            .unwrap(),
    );
    let mut s = Store::open(&p).unwrap();
    let t = s
        .create("local", "Preserve raw schema7", "original")
        .unwrap();
    drop(s);
    let c = Connection::open(&p).unwrap();
    c.execute_batch("DROP TRIGGER graph_fence_lease_insert;DROP TRIGGER graph_fence_lease_update;DROP TABLE goal_graph_acceptances;DROP TABLE goal_graph_owner_decisions;DROP TABLE goal_graph_validations;DROP TABLE goal_graph_candidates;DROP TABLE goal_graph_jobs;DROP TABLE goal_graph_capabilities;DROP TABLE goal_run_reservations;DROP TABLE goal_run_nodes;DROP TABLE goal_runs;DROP TABLE data_workspace_receipts;DROP TABLE data_workspace_creations;DROP TABLE data_workspace_previews;DROP TABLE goal_node_evidence;DROP TABLE goal_external_action_journal;DROP TABLE goal_plan_proposals;DROP TABLE goal_plan_revisions;DROP TABLE goal_plan_heads;DROP TABLE goal_scopes;DROP TABLE goal_grant_revisions;DROP TABLE goal_ledgers;DROP TRIGGER command_fence_attempt;DROP TRIGGER command_fence_lease_insert;DROP TRIGGER command_fence_lease_update;DROP TABLE command_workspace_fences;DROP TABLE command_validation_runs;DROP TABLE command_validation_policies;CREATE TABLE original_blob(bytes BLOB);INSERT INTO original_blob VALUES(x'00ffdeadbeef');PRAGMA user_version=7;").unwrap();
    let raw: String = c
        .query_row("SELECT document FROM tasks WHERE id=?1", [&t.id], |r| {
            r.get(0)
        })
        .unwrap();
    let raw =
        serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&raw).unwrap())
            .unwrap()
            + "\n";
    c.execute(
        "UPDATE tasks SET document=?1 WHERE id=?2",
        params![raw, t.id],
    )
    .unwrap();
    drop(c);
    (d, p)
}
fn backups(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(root)
        .unwrap()
        .map(|p| p.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().contains("pre-v8-"))
        .collect()
}
#[test]
fn real_schema7_upgrade_has_private_reopenable_backup_preserving_raw_documents_and_blob() {
    let (d, p) = fixture();
    let before = Connection::open(&p)
        .unwrap()
        .query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
        .unwrap();
    let s = Store::open(&p).unwrap();
    drop(s);
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        13
    );
    assert_eq!(
        c.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        before.as_bytes()
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM command_workspace_fences", [], |r| r
            .get::<_, u32>(
            0
        ))
        .unwrap(),
        0
    );
    let b = backups(d.path());
    assert_eq!(b.len(), 1);
    assert_eq!(
        fs::metadata(&b[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let b = Connection::open(&b[0]).unwrap();
    assert_eq!(
        b.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        b.query_row("SELECT document FROM tasks", [], |r| r.get::<_, String>(0))
            .unwrap()
            .as_bytes(),
        before.as_bytes()
    );
    assert_eq!(
        b.query_row("SELECT bytes FROM original_blob", [], |r| r
            .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
    assert_eq!(
        b.query_row(
            "SELECT count(*) FROM sqlite_master WHERE name='command_validation_policies'",
            [],
            |r| r.get::<_, u32>(0)
        )
        .unwrap(),
        0
    );
    drop(c);
    drop(b);
    drop(Store::open(&p).unwrap());
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn command_migration_failure_rolls_back_tables_and_version_retains_unknown_data() {
    let (d, p) = fixture();
    let c = Connection::open(&p).unwrap();
    c.execute_batch("CREATE TABLE command_validation_runs(sentinel TEXT);INSERT INTO command_validation_runs VALUES('preserve schema7 unknown data');").unwrap();
    drop(c);
    assert!(Store::open(&p).is_err());
    let c = Connection::open(&p).unwrap();
    assert_eq!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        7
    );
    assert_eq!(
        c.query_row("SELECT sentinel FROM command_validation_runs", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        "preserve schema7 unknown data"
    );
    assert_eq!(c.query_row("SELECT count(*) FROM sqlite_master WHERE name IN('command_validation_policies','command_workspace_fences')",[],|r|r.get::<_,u32>(0)).unwrap(),0);
    assert_eq!(backups(d.path()).len(), 1);
}
#[test]
fn future_schema14_refused_without_touching_bytes() {
    let (_d, p) = fixture();
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA user_version=14;").unwrap();
    drop(c);
    let raw = fs::read(&p).unwrap();
    assert_eq!(Store::open(&p).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(fs::read(&p).unwrap(), raw);
}
