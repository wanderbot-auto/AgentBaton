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
#[path = "fixtures/child_goal_legacy_migration/private_assert.rs"]
mod private_assert;
use private_assert::assert_private_equal;

fn digest(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    })
}

fn verify_tool_provenance(source: &Path, manifest: &Value) {
    if manifest.get("provenance_kind").is_none() {
        // Preserve the original immutable local input: do not relabel a fresh
        // CI build as this original Linux pair.
        assert_eq!(
            manifest["binaries"]["batond"],
            "1c835dcd2b925baeb2f0131b225c1047bdc89c4452417a302fae34a133c4cf73"
        );
        assert_eq!(
            manifest["binaries"]["baton"],
            "d90cf23761b83dc7eeeb92291b411a573e9cfb6ecde3750bc3bb376d2fe66797"
        );
        return;
    }
    assert_eq!(manifest["provenance_kind"], "reviewed_source_build");
    let path = source.join("source-build-manifest.json");
    let info = fs::symlink_metadata(&path).unwrap();
    assert!(info.is_file() && !info.file_type().is_symlink());
    assert_eq!(info.permissions().mode() & 0o777, 0o600);
    assert!(info.len() > 0 && info.len() <= 4_194_304);
    let raw = fs::read(&path).unwrap();
    let hash = agentbaton_workspaces::validation::sha256(&raw);
    assert_eq!(hash, manifest["source_build_manifest_sha256"]);
    assert_eq!(hash, manifest["frozen_manifest_sha256"]);
    let m: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(m["contract"], "agentbaton.schema14-source-build.v1");
    assert_eq!(m["status"], "passed");
    assert_eq!(m["schema"], 14);
    let host = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    assert_eq!(m["platform"], host);
    assert_eq!(
        manifest
            .get("platform")
            .and_then(Value::as_str)
            .unwrap_or("linux"),
        host
    );
    if host == "darwin" {
        let architecture = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        assert_eq!(m["architecture"], architecture);
        assert_eq!(manifest["architecture"], architecture);
        assert!(digest(&m["native_helper_sha256"]));
        assert!(
            m["native_helper_sha256"]
                == agentbaton_workspaces::validation::sha256(include_bytes!(
                    "../../../scripts/native-run.py"
                )),
            "Native lifecycle helper source is not the reviewed checkout"
        );
        assert!(!m["os_release"].as_str().unwrap().is_empty());
    }
    assert_eq!(
        m["source"]["commit"],
        "f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701"
    );
    assert_eq!(
        m["source"]["tree"],
        "93ce75df286770a9f196994ed9f5c9a11528cc5e"
    );
    assert_eq!(
        m["source"]["repository"],
        "https://github.com/wanderbot-auto/AgentBaton.git"
    );
    assert_eq!(m["source"]["compiled_source_clean"], true);
    assert!(digest(&m["source"]["archive_sha256"]));
    let current_builder = agentbaton_workspaces::validation::sha256(include_bytes!(
        "../../../scripts/prepare-legacy-migration-fixture.py"
    ));
    let historical_linux = host == "linux"
        && m.get("architecture").is_none()
        && m.get("os_release").is_none()
        && m.get("native_helper_sha256").is_none()
        && m["probe"].get("identity_method").is_none()
        && [
            "owned_identity",
            "leader_unreaped_until_signalling",
            "group_quiescent_before_reaping",
            "cleanup_errors",
        ]
        .iter()
        .all(|&k| m["probe"].get(k).is_none())
        && ["batond", "baton"].iter().all(|&n| {
            m["binaries"][n].get("format").is_none()
                && m["binaries"][n].get("architecture").is_none()
        })
        && m["builder_sha256"]
            == "b3ce52eb0c32791db9532e9cdd1a4e194d700cebd82c3120b76dfa3e20b81e5b";
    assert!(
        m["builder_sha256"] == current_builder || historical_linux,
        "Unreviewed builder source or relabelled historical Linux proof"
    );
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "crates/core/Cargo.toml",
        "apps/daemon/Cargo.toml",
        "apps/cli/Cargo.toml",
    ] {
        assert!(digest(&m["source"]["files_sha256"][name]));
    }
    let b = &m["build"];
    assert_eq!(b["exit_code"], 0);
    assert_eq!(b["packages"], json!(["batond", "baton"]));
    assert_eq!(b["features"], json!([]));
    assert_eq!(b["default_features"], true);
    assert_eq!(
        b["lockfile_sha256"],
        m["source"]["files_sha256"]["Cargo.lock"]
    );
    let argv = b["argv"].as_array().unwrap();
    assert_eq!(argv.len(), 9);
    assert!(Path::new(argv[0].as_str().unwrap()).is_absolute());
    assert_eq!(
        &argv[1..],
        &[
            json!("build"),
            json!("--locked"),
            json!("-j"),
            json!("2"),
            json!("--bin"),
            json!("batond"),
            json!("--bin"),
            json!("baton")
        ]
    );
    for name in ["cargo", "rustc"] {
        assert!(!m["toolchain"][name].as_str().unwrap().is_empty());
    }
    let p = &m["probe"];
    assert_eq!(p["schema"], 14);
    assert_eq!(p["query_only"], true);
    assert_eq!(p["foreign_key_errors"], json!([]));
    let names = [
        "goal_scopes",
        "goal_runs",
        "goal_graph_capabilities",
        "factory_scopes",
        "factory_runs",
        "factory_task_lineage",
        "factory_delegations",
        "factory_task_bindings",
        "factory_attempts",
        "factory_scoped_handles",
        "factory_handoffs",
        "factory_budget_reservations",
        "factory_child_plan_heads",
        "factory_child_plan_revisions",
    ];
    assert_eq!(p["counts"].as_object().unwrap().len(), names.len());
    for name in names {
        assert_eq!(p["counts"][name], 0);
    }
    for (key, value) in [
        ("snapshot_api_version", 1),
        ("snapshot_sequence", 0),
        ("cli_exit", 0),
        ("daemon_wait_exit", 0),
    ] {
        assert_eq!(p[key], value);
    }
    assert_eq!(p["sigterm_sent"], true);
    assert_eq!(p["forced_kill"], false);
    assert_eq!(p["identity_gone_after_join"], true);
    if host == "darwin" {
        assert_eq!(p["identity_method"], "darwin_owned_unreaped_group");
        assert_eq!(p["leader_unreaped_until_signalling"], true);
        assert_eq!(p["group_quiescent_before_reaping"], true);
        assert_eq!(p["cleanup_errors"], json!([]));
    }
    for name in ["batond", "baton"] {
        let binary = &m["binaries"][name];
        assert!(digest(&binary["sha256"]));
        assert_eq!(binary["sha256"], manifest["binaries"][name]);
        assert_eq!(binary["mode"], "0o500");
        assert_eq!(binary["nlink"], 1);
        if host == "darwin" {
            assert_eq!(binary["format"], "mach-o-64");
            assert_eq!(binary["architecture"], m["architecture"]);
        }
        assert!(binary["bytes"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 134_217_728));
        assert_eq!(
            binary["bytes"],
            manifest["tool_metadata_before"][name]["bytes"]
        );
    }
    assert_eq!(
        manifest["tool_metadata_before"],
        manifest["tool_metadata_after"]
    );
}

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
    let source = PathBuf::from(std::env::var_os("AGENTBATON_ACTUAL_SCHEMA14_FIXTURE")
        .expect("Explicit private actual14 fixture is required; run the dedicated migration target after generating it"));
    let info = fs::symlink_metadata(&source).unwrap();
    assert!(info.is_dir() && !info.file_type().is_symlink());
    assert_eq!(info.permissions().mode() & 0o777, 0o700);
    let source = source.canonicalize().unwrap();
    for (name, maximum) in [
        ("manifest.json", 4_194_304),
        ("actual-schema14.sqlite3", 16_777_216),
    ] {
        let info = fs::symlink_metadata(source.join(name)).unwrap();
        assert!(info.is_file() && !info.file_type().is_symlink());
        assert_eq!(info.permissions().mode() & 0o777, 0o600);
        assert!(info.len() > 0 && info.len() <= maximum);
    }
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
    assert_eq!(manifest["actual_initial_probe"]["schema"], 14);
    assert_eq!(manifest["actual_initial_probe"]["goal_scopes"], 0);
    assert_eq!(manifest["actual_initial_probe"]["factory_scopes"], 0);
    assert_eq!(manifest["actual_initial_probe"]["foreign_keys"], "clean");
    assert_eq!(manifest["actual_initial_probe"]["query_only"], true);
    let owned = manifest["owned_processes"].as_array().unwrap();
    assert!(owned.len() >= 2 && owned.iter().all(|p| p["gone_after_join"] == true));
    if cfg!(target_os = "macos") {
        assert_eq!(manifest["platform"], "darwin");
        assert_eq!(
            manifest["daemon_identity_method"],
            "darwin_owned_unreaped_group"
        );
        for key in [
            "daemon_sigterm_sent",
            "daemon_group_quiescent_before_reaping",
            "daemon_leader_unreaped_until_signalling",
        ] {
            assert_eq!(manifest[key], true);
        }
        assert_eq!(manifest["daemon_cleanup_errors"], json!([]));
        assert_eq!(
            manifest["provider_stop_proof"]["method"],
            "core_stop_confirmed_plus_pid_absence"
        );
        assert_eq!(manifest["provider_stop_proof"]["observed"], owned.len());
        assert_eq!(manifest["provider_stop_proof"]["core_join_confirmed"], true);
        assert_eq!(
            manifest["provider_stop_proof"]["pid_absence_confirmed"],
            true
        );
    }
    assert_eq!(
        manifest["source_commit"],
        "f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701"
    );
    verify_tool_provenance(&source, &manifest);
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
    if cfg!(target_os = "macos") {
        for process in owned {
            assert!(process["pid"].as_u64().is_some_and(|n| n > 1));
            assert!(
                process["pgid"].as_u64() == process["pid"].as_u64(),
                "Private provider group tuple differs"
            );
            assert_eq!(
                process["identity_method"],
                "darwin_pid_absence_after_core_join"
            );
            assert_eq!(process["pid_query_exit"], 1);
            assert_eq!(process["pid_query_stdout_empty"], true);
            assert_eq!(process["pid_query_stderr_empty"], true);
            let join = &process["core_join"];
            assert!(
                join["stop_confirmed"] == true,
                "Private Core stop receipt is unconfirmed"
            );
            assert!(
                join["session_state"] == "stopped",
                "Private Core session is not stopped"
            );
            let session = join["session_id"].as_str().unwrap();
            let attempt = join["attempt_id"].as_str().unwrap();
            let count: i64 = db.query_row("SELECT count(*) FROM sessions WHERE id=?1 AND json_extract(document,'$.attempt_id')=?2 AND json_extract(document,'$.state')='stopped'",[session,attempt],|r|r.get(0)).unwrap();
            assert_eq!(count, 1);
            let count: i64 = match join["source"].as_str().unwrap() {
                "native_goal_node" => db.query_row("SELECT count(*) FROM goal_run_nodes WHERE json_extract(document,'$.session_id')=?1 AND json_extract(document,'$.attempt_id')=?2 AND json_extract(document,'$.stop_confirmed')=1",[session,attempt],|r|r.get(0)).unwrap(),
                "factory_binding_attempt" => {
                    let count: i64 = db.query_row("SELECT count(*) FROM factory_task_bindings WHERE json_extract(document,'$.session_id')=?1 AND json_extract(document,'$.handle.attempt_id')=?2 AND json_extract(document,'$.handle.confirmed_stop')=1",[session,attempt],|r|r.get(0)).unwrap();
                    assert_eq!(count,1);
                    db.query_row("SELECT count(*) FROM factory_attempts WHERE session_id=?1 AND attempt_id=?2 AND json_extract(document,'$.stop_confirmed')=1",[session,attempt],|r|r.get(0)).unwrap()
                },
                _ => panic!("Unsupported private Core stop receipt source"),
            };
            assert_eq!(count, 1);
        }
    }
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
    let legacy_factory: i64 = db.query_row("SELECT count(*) FROM factory_scopes WHERE json_extract(document,'$.selection.capabilities.child_goal_execution')=0 AND json_type(document,'$.selection.child_goal_execution_policy') IS NULL", [], |r|r.get(0)).unwrap();
    assert!(
        legacy_factory > 0,
        "Actual14 Factory scope has no child Goal execution authority"
    );
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
                && p.extension().is_some_and(|v| v == "sqlite3")
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
    assert_private_equal(
        &rows(&after),
        &data,
        path,
        "Failed migration changed rejected proof",
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
    assert!(
        new_rows.remove("factory_child_goal_runs") == Some(vec![]),
        "Historical Factory cannot gain execution authority; inspect retained private evidence"
    );
    assert_private_equal(
        &new_rows,
        &original,
        &path,
        "Terminal/joined original raw rows must survive",
    );
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
    assert_private_equal(
        &rows(&backup),
        &original,
        &path,
        "Pre-v15 backup must retain original14 rows",
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
    // Intentionally corrupt this private negative-test copy, never the real14
    // generator/input or production Store connection. Store::open must reject
    // it under its unchanged integrity validation.
    db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
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
            db.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
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
    for mutation in ["result", "capability", "receipt", "relational_version"] {
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
            "relational_version" => {
                let run_id = manifest["native_graph_run_id"].as_str().unwrap();
                let original: String = db
                    .query_row(
                        "SELECT document FROM goal_graph_capabilities WHERE run_id=?1",
                        [run_id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    serde_json::from_str::<Value>(&original).unwrap()["version"],
                    1
                );
                // The original schema has CHECK(version=1). Only this private
                // corruption copy bypasses the SQLite writer CHECK, without
                // changing DDL. Restore it before the production Store opens;
                // the real Native approval/result/document remain untouched.
                db.execute_batch("PRAGMA ignore_check_constraints=ON")
                    .unwrap();
                assert_eq!(
                    db.execute(
                        "UPDATE goal_graph_capabilities SET version=2 WHERE run_id=?1",
                        [run_id]
                    )
                    .unwrap(),
                    1
                );
                db.execute_batch("PRAGMA ignore_check_constraints=OFF")
                    .unwrap();
                let after: String = db
                    .query_row(
                        "SELECT document FROM goal_graph_capabilities WHERE run_id=?1",
                        [run_id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(
                    agentbaton_workspaces::validation::sha256(original.as_bytes()),
                    agentbaton_workspaces::validation::sha256(after.as_bytes())
                );
                assert!(fk_errors(&db).is_empty());
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
    assert_private_equal(&rows(&db), &before, &path, "Future database changed");
    assert!(backups(dir.path()).is_empty());
}
