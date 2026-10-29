//! Actual schema14 Core history, not a current database relabelled as legacy.
//!
//! The companion generator records its frozen old binaries and original RPC
//! results. Missing input is a test failure; synthetic approvals are never a
//! substitute. This tests Store::open (including recovery), not a public
//! migration-only API, which does not exist.
use agentbaton_core::storage::Store;
use rusqlite::{types::ValueRef, Connection};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

type Rows = BTreeMap<String, Vec<Vec<Value>>>;

struct FixtureDir(Option<tempfile::TempDir>);
impl FixtureDir {
    fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}
impl Drop for FixtureDir {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(dir) = self.0.take() {
                // Retain the exact failing copy, WAL and migration backup;
                // the immutable real14 source input is never edited either.
                let path = dir.keep();
                eprintln!(
                    "Retained private failed migration database: {}",
                    path.display()
                );
            }
        }
    }
}

fn fixture() -> (FixtureDir, PathBuf, Value) {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/child_goal_legacy_migration");
    let manifest: Value = serde_json::from_slice(
        &fs::read(source.join("manifest.json"))
            .expect("Generate and retain the real schema14 fixture first"),
    )
    .unwrap();
    assert_eq!(manifest["schema"], 14);
    assert_eq!(manifest["status"], "passed");
    assert_eq!(manifest["origin"], "actual_frozen_schema14_core_rpc");
    assert_eq!(
        manifest["native_owner"],
        "trusted_private_host_fixture_not_os_dialog"
    );
    assert_eq!(
        manifest["provider"],
        "generated_local_acp_no_model_or_credentials"
    );
    assert_eq!(manifest["daemon_exit"], 0);
    assert_eq!(manifest["forced_kill"], false);
    let owned = manifest["owned_processes"].as_array().unwrap();
    assert!(owned.len() >= 2 && owned.iter().all(|p| p["gone_after_join"] == true));
    assert_eq!(
        manifest["source_commit"],
        "f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701"
    );
    assert_eq!(
        manifest["binaries"]["batond"],
        "1c835dcd2b925baeb2f0131b225c1047bdc89c4452417a302fae34a133c4cf73"
    );
    assert_eq!(
        manifest["binaries"]["baton"],
        "d90cf23761b83dc7eeeb92291b411a573e9cfb6ecde3750bc3bb376d2fe66797"
    );
    assert_ne!(
        manifest["native_goal_grant_id"],
        manifest["factory_grant_id"]
    );
    assert_ne!(
        manifest["native_goal_workspace_id"],
        manifest["factory_workspace_id"]
    );
    let bytes = fs::read(source.join("actual-schema14.sqlite3")).unwrap();
    assert_eq!(
        agentbaton_workspaces::validation::sha256(&bytes),
        manifest["database_sha256"]
    );
    let dir = FixtureDir(Some(
        tempfile::Builder::new()
            .prefix("legacy14-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap(),
    ));
    let path = dir.path().join("history.sqlite3");
    fs::write(&path, bytes).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), 14);
    assert!(fk_errors(&db).is_empty());
    for table in [
        "goal_scopes",
        "goal_runs",
        "goal_graph_capabilities",
        "factory_scopes",
        "factory_runs",
        "factory_child_plan_revisions",
        "factory_child_plan_heads",
    ] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert!(count > 0, "Real legacy fixture must contain {table}");
        assert_eq!(json!(count), manifest["counts"][table]);
    }
    let pending: i64 = db.query_row("SELECT count(*) FROM goal_runs WHERE state IN ('queued','running','cancel_requested','outcome_unknown')", [], |r|r.get(0)).unwrap();
    assert_eq!(pending, 0, "This immutable-history fixture must have joined terminal workers; active recovery is a separate case");
    for kind in ["goal", "goal_graph_execution", "factory"] {
        let count: i64 = db.query_row("SELECT count(*) FROM approval_proposals WHERE json_extract(document,'$.kind')=?1 AND json_extract(document,'$.consumed')=1", [kind], |r|r.get(0)).unwrap();
        assert!(count > 0, "Missing real consumed {kind} approval");
    }
    // A capability must retain its original Native proposal foreign key.
    let invalid: i64 = db.query_row("SELECT count(*) FROM goal_graph_capabilities c LEFT JOIN approval_proposals p ON p.id=c.approval_id WHERE p.id IS NULL OR json_extract(p.document,'$.kind')!='goal_graph_execution' OR json_extract(p.document,'$.consumed')!=1", [], |r|r.get(0)).unwrap();
    assert_eq!(invalid, 0);
    drop(db);
    (dir, path, manifest)
}

fn version(db: &Connection) -> u32 {
    db.query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap()
}
fn fk_errors(db: &Connection) -> Vec<String> {
    let mut s = db.prepare("PRAGMA foreign_key_check").unwrap();
    let values = s
        .query_map([], |r| {
            Ok(format!(
                "{}:{}:{}",
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    values
}
fn tables(db: &Connection) -> Vec<String> {
    let mut s = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap();
    let values = s
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    values
}
fn rows(db: &Connection) -> Rows {
    tables(db)
        .into_iter()
        .map(|table| {
            assert!(table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
            let mut s = db.prepare(&format!("SELECT * FROM {table}")).unwrap();
            let columns = s.column_count();
            let mut values: Vec<Vec<Value>> = s
                .query_map([], |r| {
                    (0..columns)
                        .map(|i| {
                            Ok(match r.get_ref(i)? {
                                ValueRef::Null => json!(["null"]),
                                ValueRef::Integer(v) => json!(["integer", v]),
                                ValueRef::Real(v) => json!(["real", v.to_bits()]),
                                ValueRef::Text(v) => {
                                    json!(["text", String::from_utf8(v.to_vec()).unwrap()])
                                }
                                ValueRef::Blob(v) => json!(["blob", v]),
                            })
                        })
                        .collect()
                })
                .unwrap()
                .map(Result::unwrap)
                .collect();
            values.sort_by_key(|r| serde_json::to_string(r).unwrap());
            (table, values)
        })
        .collect()
}
fn backups(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v15-")
        })
        .collect()
}
fn schema(db: &Connection) -> Vec<(String, String)> {
    let mut s = db
        .prepare("SELECT name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
        .unwrap();
    let values = s
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    values
}
fn reject_unchanged(path: &Path) {
    let before = Connection::open(path).unwrap();
    let data = rows(&before);
    let ddl = schema(&before);
    drop(before);
    assert!(
        Store::open(path).is_err(),
        "Corrupt provenance must never fall back to Native or derived authority"
    );
    let after = Connection::open(path).unwrap();
    assert_eq!(version(&after), 14);
    assert_eq!(
        schema(&after),
        ddl,
        "Failed migration must roll back schema"
    );
    assert_eq!(
        rows(&after),
        data,
        "Failed migration must not erase rejected proof"
    );
}

#[test]
fn actual_nonempty_native_and_factory14_history_survives_store15_and_private_backup() {
    let (dir, path, manifest) = fixture();
    let db = Connection::open(&path).unwrap();
    let original = rows(&db);
    let old_cap_fk: Vec<(String, String)> = {
        let mut s=db.prepare("SELECT \"table\",\"from\" FROM pragma_foreign_key_list('goal_graph_capabilities') ORDER BY id").unwrap();
        let values = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        values
    };
    drop(db);
    let store = Store::open(&path).unwrap();
    let scope_id = manifest["native_goal_scope_id"].as_str().unwrap();
    let view = store.get_goal_scope(scope_id).unwrap();
    assert_eq!(view.id, scope_id);
    assert!(serde_json::to_value(&view)
        .unwrap()
        .get("factory_origin")
        .is_none());
    let details = store
        .goal_graph_details(manifest["native_graph_run_id"].as_str().unwrap())
        .unwrap();
    assert_eq!(details.task_accepted, false);
    drop(store);
    let after = Connection::open(&path).unwrap();
    assert_eq!(version(&after), 15);
    assert!(fk_errors(&after).is_empty());
    let mut new_rows = rows(&after);
    assert_eq!(
        new_rows.remove("factory_child_goal_runs"),
        Some(vec![]),
        "Migration cannot upgrade historical Factory scopes to execution authority"
    );
    assert_eq!(new_rows,original,"Fixture terminal/joined precondition means recovery has no active work to alter; every old raw JSON, ID, result, capability and event must survive");
    let new_cap_fk: Vec<(String, String)> = {
        let mut s=after.prepare("SELECT \"table\",\"from\" FROM pragma_foreign_key_list('goal_graph_capabilities') ORDER BY id").unwrap();
        let values = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        values
    };
    assert_eq!(new_cap_fk, old_cap_fk);
    assert!(new_cap_fk.contains(&("approval_proposals".into(), "approval_id".into())));
    for table in ["goal_scopes", "goal_runs"] {
        let count:i64=after.query_row(&format!("SELECT count(*) FROM pragma_foreign_key_list('{table}') WHERE \"table\"='approval_proposals'"),[],|r|r.get(0)).unwrap();
        assert_eq!(
            count, 0,
            "Only the overbroad ID-is-Native-proposal assumption is removed"
        );
    }
    let saved = backups(dir.path());
    assert_eq!(saved.len(), 1);
    assert_eq!(
        fs::metadata(&saved[0]).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let backup = Connection::open(&saved[0]).unwrap();
    assert_eq!(version(&backup), 14);
    assert_eq!(
        rows(&backup),
        original,
        "Pre-v15 SQLite backup retains actual original14 rows, not recovered15 rows"
    );
    assert!(fk_errors(&backup).is_empty());
    drop(after);
    drop(Store::open(&path).unwrap());
    assert_eq!(
        backups(dir.path()).len(),
        1,
        "Reopen cannot create another migration backup"
    );
}

#[test]
fn actual14_migration_collision_rolls_back_without_losing_proof() {
    let (_dir, path, _) = fixture();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE factory_child_goal_runs(sentinel TEXT);INSERT INTO factory_child_goal_runs VALUES('retain failed DDL evidence');").unwrap();
    drop(db);
    reject_unchanged(&path);
}

#[test]
fn actual14_orphan_scope_is_rejected_without_disabling_native_capability_fks() {
    let (_dir, path, _) = fixture();
    let db = Connection::open(&path).unwrap();
    db.execute(
        "UPDATE goal_scopes SET root_grant_id='missing-original-grant'",
        [],
    )
    .unwrap();
    assert!(!fk_errors(&db).is_empty());
    drop(db);
    reject_unchanged(&path);
}

#[test]
fn actual14_missing_or_wrong_native_capability_proposal_cannot_fallback() {
    for missing in [true, false] {
        let (_dir, path, manifest) = fixture();
        let db = Connection::open(&path).unwrap();
        if missing {
            db.execute(
                "UPDATE goal_graph_capabilities SET approval_id='missing-native-approval'",
                [],
            )
            .unwrap();
        } else {
            // A real consumed Goal scope proposal is still the wrong issuer for
            // a Graph execution capability; retain it rather than forging one.
            db.execute(
                "UPDATE goal_graph_capabilities SET approval_id=?1",
                [manifest["native_goal_scope_id"].as_str().unwrap()],
            )
            .unwrap();
            assert!(fk_errors(&db).is_empty());
        }
        drop(db);
        reject_unchanged(&path);
    }
}

#[test]
fn actual14_corrupt_native_result_capability_or_ordered_receipt_is_rejected() {
    for mutation in ["result", "capability", "receipt"] {
        let (_dir, path, manifest) = fixture();
        let db = Connection::open(&path).unwrap();
        match mutation {
            "result" => {
                let raw: String = db
                    .query_row(
                        "SELECT result FROM approval_proposals WHERE id=?1",
                        [manifest["native_graph_approval_id"].as_str().unwrap()],
                        |r| r.get(0),
                    )
                    .unwrap();
                let mut v: Value = serde_json::from_str(&raw).unwrap();
                v["run"]["plan_digest"] = json!("0".repeat(64));
                db.execute(
                    "UPDATE approval_proposals SET result=?1 WHERE id=?2",
                    [
                        serde_json::to_string(&v).unwrap(),
                        manifest["native_graph_approval_id"]
                            .as_str()
                            .unwrap()
                            .into(),
                    ],
                )
                .unwrap();
            }
            "capability" => {
                db.execute("UPDATE goal_graph_capabilities SET document='{}'", [])
                    .unwrap();
            }
            "receipt" => {
                // Mutate the original receipt, not a synthetic new authority.
                let n=db.execute("DELETE FROM events WHERE sequence=(SELECT sequence FROM events WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='goal.run_authorized' LIMIT 1)",[manifest["native_graph_run_id"].as_str().unwrap()]).unwrap();
                assert_eq!(
                    n, 1,
                    "Expected actual original ordered authorization receipt"
                );
            }
            _ => unreachable!(),
        }
        drop(db);
        reject_unchanged(&path);
    }
}

#[test]
fn actual14_fixture_relabeled_future16_is_refused_before_backup_or_recovery() {
    let (dir, path, _) = fixture();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=16").unwrap();
    let before = rows(&db);
    drop(db);
    let error = match Store::open(&path) {
        Ok(_) => panic!("Future16 must not open"),
        Err(e) => e,
    };
    assert_eq!(error.code, "VERSION_UNSUPPORTED");
    let db = Connection::open(&path).unwrap();
    assert_eq!(version(&db), 16);
    assert_eq!(rows(&db), before);
    assert!(backups(dir.path()).is_empty());
}
