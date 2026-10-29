#[path = "fixtures/pre_factory_schema.rs"]
mod pre_factory_schema;
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
    pre_factory_schema::remove_empty_factory_increment(&c).unwrap();
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
        15
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
fn future_schema16_refused_without_touching_bytes() {
    let (_d, p) = fixture();
    let c = Connection::open(&p).unwrap();
    c.execute_batch("PRAGMA user_version=16;").unwrap();
    drop(c);
    let raw = fs::read(&p).unwrap();
    assert_eq!(Store::open(&p).err().unwrap().code, "VERSION_UNSUPPORTED");
    assert_eq!(fs::read(&p).unwrap(), raw);
}

fn fresh_factory_fixture() -> (tempfile::TempDir, PathBuf, Connection) {
    let directory = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let database = directory.path().join("fresh-schema15.sqlite3");
    drop(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&database)
            .unwrap(),
    );
    drop(Store::open(&database).unwrap());
    let connection = Connection::open(&database).unwrap();
    (directory, database, connection)
}

#[test]
fn fixture_inverse_restores_native_identity_fks_and_preserves_other_schema_and_bytes() {
    let (_directory, _database, connection) = fresh_factory_fixture();
    connection.execute_batch("CREATE TABLE fixture_unknown(raw BLOB);INSERT INTO fixture_unknown VALUES(x'00ffdeadbeef');PRAGMA foreign_keys=ON;").unwrap();
    type SchemaObject = (String, String, String, Option<String>);
    type Column = (i64, String, String, i64, Option<String>, i64);
    type ForeignKey = (String, String, Option<String>, String, String, String);
    let native_shape = |table: &str| -> (Vec<Column>, Vec<ForeignKey>) {
        let columns = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        let mut foreign: Vec<ForeignKey> = connection
            .prepare(&format!("PRAGMA foreign_key_list({table})"))
            .unwrap()
            .query_map([], |r| {
                Ok((
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            })
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        foreign.sort();
        (columns, foreign)
    };
    let before_native = ["goal_scopes", "goal_runs"].map(|table| native_shape(table));
    let before: Vec<SchemaObject> = connection
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?)))
        .unwrap()
        .map(|row| row.unwrap())
        .filter(|row| !pre_factory_schema::FACTORY_TABLES.contains(&row.2.as_str()))
        .map(|mut row| {
            // Preserve the complete sqlite_master comparison, changing ONLY
            // the two intentional Native id->proposal FK restorations.
            if row.0 == "table" && matches!(row.1.as_str(), "goal_scopes" | "goal_runs") {
                let sql = row.3.as_ref().unwrap();
                assert!(sql.starts_with(&format!("CREATE TABLE \"{}\"(id TEXT PRIMARY KEY,",row.1)));
                row.3 = Some(sql.replacen("(id TEXT PRIMARY KEY,", "(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),", 1));
            }
            row
        })
        .collect();
    pre_factory_schema::remove_empty_factory_increment(&connection).unwrap();
    assert_eq!(
        connection
            .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
            .unwrap(),
        12
    );
    let after: Vec<SchemaObject> = connection
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_master WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name")
        .unwrap()
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?)))
        .unwrap().map(|row|row.unwrap()).collect();
    assert_eq!(after, before);
    for (table, (columns, mut foreign)) in
        ["goal_scopes", "goal_runs"].into_iter().zip(before_native)
    {
        assert!(!foreign.iter().any(|f| f.1 == "id"));
        foreign.push((
            "approval_proposals".into(),
            "id".into(),
            Some("id".into()),
            "NO ACTION".into(),
            "NO ACTION".into(),
            "NONE".into(),
        ));
        foreign.sort();
        assert_eq!(native_shape(table), (columns, foreign), "{table}");
    }
    assert!(connection
        .pragma_query_value::<bool, _>(None, "foreign_keys", |r| r.get(0))
        .unwrap());
    assert!(!connection
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    assert_eq!(
        connection
            .query_row("SELECT raw FROM fixture_unknown", [], |row| row
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        [0, 255, 222, 173, 190, 239]
    );
}

#[test]
fn fixture_inverse_refuses_nonempty_missing_or_wrong_version_without_deleting_any_bytes() {
    for variant in [
        "nonempty_legacy_reservation",
        "nonempty_child_goal",
        "missing_table",
        "missing_child_goal_table",
        "wrong_version",
        "old_current14",
        "factory_extra_column",
        "factory_extra_index",
        "native_scope_extra_column",
        "native_run_extra_index",
        "late_rebuild_collision",
        "missing_native_proposal_fk",
    ] {
        let (_directory, database, connection) = fresh_factory_fixture();
        connection
            .pragma_update(None, "foreign_keys", true)
            .unwrap();
        match variant {
            // Explicit private malformed-data fixtures only. Restore FK before
            // asking the helper to reject, never manufacture a successful run.
            "nonempty_legacy_reservation" => connection.execute_batch("PRAGMA foreign_keys=OFF;INSERT INTO factory_budget_reservations(attempt_id,scope_id,root_task_id,project_id,document) VALUES('generated-unreconciled',NULL,'generated-root','generated-project',json_object('authority_kind','legacy_shared_session'));PRAGMA foreign_keys=ON;").unwrap(),
            "nonempty_child_goal" => connection.execute_batch("PRAGMA foreign_keys=OFF;INSERT INTO factory_child_goal_runs VALUES('run','scope','factory-scope','factory-run','task','root',NULL,'plan',1,1,printf('%064d',0),'{}');PRAGMA foreign_keys=ON;").unwrap(),
            "missing_table" => connection.execute_batch("DROP TABLE factory_scoped_handles;").unwrap(),
            "missing_child_goal_table" => connection.execute_batch("DROP TABLE factory_child_goal_runs;").unwrap(),
            "wrong_version" => connection.execute_batch("PRAGMA user_version=12;").unwrap(),
            "old_current14" => connection.execute_batch("PRAGMA user_version=14;").unwrap(),
            "factory_extra_column" => connection.execute_batch("ALTER TABLE factory_budget_reservations ADD COLUMN unknown_fixture_field TEXT;").unwrap(),
            "factory_extra_index" => connection.execute_batch("CREATE INDEX unknown_factory_index ON factory_runs(state);").unwrap(),
            "native_scope_extra_column" => connection.execute_batch("ALTER TABLE goal_scopes ADD COLUMN unknown_fixture_field TEXT;").unwrap(),
            "native_run_extra_index" => connection.execute_batch("CREATE INDEX unknown_fixture_index ON goal_runs(task_id);").unwrap(),
            // Inert raw schema corruption, NOT a Native approval or a usable
            // Store fixture. All existing15 FKs hold; restoring the native
            // proposal FK must fail late and roll back the whole inverse.
            "missing_native_proposal_fk" => connection.execute_batch("INSERT INTO tasks VALUES('inert-task','{}');INSERT INTO projects VALUES('inert-project','{}','fixture-dev','fixture-ino','/inert-private-fixture');INSERT INTO execution_grants VALUES('inert-grant','inert-project',NULL,'{}');INSERT INTO goal_scopes VALUES('missing-proposal','inert-task','inert-grant',1,0,'{}');").unwrap(),
            "late_rebuild_collision" => connection.execute_batch("CREATE TABLE goal_runs_pre_factory(sentinel BLOB);").unwrap(),
            _ => unreachable!(),
        }
        if variant == "missing_native_proposal_fk" {
            assert!(!connection
                .prepare("PRAGMA foreign_key_check")
                .unwrap()
                .exists([])
                .unwrap());
        }
        let before = fs::read(&database).unwrap();
        assert!(
            pre_factory_schema::remove_empty_factory_increment(&connection).is_err(),
            "{variant}"
        );
        assert_eq!(fs::read(&database).unwrap(), before, "{variant}");
        assert!(connection.is_autocommit(), "{variant}");
        assert!(
            connection
                .pragma_query_value::<bool, _>(None, "foreign_keys", |r| r.get(0))
                .unwrap(),
            "{variant}"
        );
    }
}
