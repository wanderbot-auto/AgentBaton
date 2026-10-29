//! Explicit required-feature genuine15->16 migration target; no synthetic old schema.
//! Root must register required-features=["actual-schema15-migration-tests"].
use agentbaton_core::storage::{Store, Task};
use rusqlite::{params, types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const SOURCE: &str = "baa72ac04944ccd1a1abc659084f5bc6cd148f33";
const TREE: &str = "fabfe01e74189f4986ea8c133f32a5f1ed52006b";
const FACTORY: [&str; 12] = [
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
    "factory_child_goal_runs",
];
type Rows = BTreeMap<String, Vec<Vec<SqlValue>>>;
type Schema = Vec<(String, String, String, Option<String>)>;
#[derive(PartialEq)]
struct Image {
    schema: Schema,
    rows: Rows,
    version: u32,
    schema_version: u32,
    application_id: u32,
}

fn equal<T: PartialEq>(a: &T, b: &T, path: &Path, context: &str) {
    assert!(
        a == b,
        "{context}; retained private evidence at {}",
        path.display()
    );
}
fn sha(raw: &[u8]) -> String {
    agentbaton_workspaces::validation::sha256(raw)
}
fn digest(value: &Value) -> bool {
    value.as_str().is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
fn private_read(path: &Path, maximum: u64) -> Vec<u8> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)
        .unwrap();
    let info = file.metadata().unwrap();
    assert!(info.is_file() && info.nlink() == 1);
    assert_eq!(info.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(info.permissions().mode() & 0o7777, 0o600);
    assert!(info.len() > 0 && info.len() <= maximum);
    let mut raw = Vec::new();
    Read::by_ref(&mut file)
        .take(maximum + 1)
        .read_to_end(&mut raw)
        .unwrap();
    let after = file.metadata().unwrap();
    let pin = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
            m.nlink(),
            m.uid(),
            m.mode(),
        )
    };
    assert!(pin(&info) == pin(&after));
    assert_eq!(raw.len() as u64, info.len());
    raw
}
fn ro(path: &Path) -> Connection {
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    db.pragma_update(None, "query_only", true).unwrap();
    db
}
fn image(db: &Connection) -> Image {
    let schema: Schema = db
        .prepare("SELECT type,name,tbl_name,sql FROM sqlite_schema ORDER BY type,name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut rows = BTreeMap::new();
    for (_, name, _, _) in schema.iter().filter(|r| r.0 == "table") {
        let quoted = name.replace('"', "\"\"");
        let mut statement = db
            .prepare(&format!("SELECT rowid,* FROM \"{quoted}\" ORDER BY rowid"))
            .unwrap();
        let width = statement.column_count();
        let values = statement
            .query_map([], |r| {
                (0..width)
                    .map(|i| r.get::<_, SqlValue>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        rows.insert(name.clone(), values);
    }
    Image {
        schema,
        rows,
        version: db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap(),
        schema_version: db
            .pragma_query_value(None, "schema_version", |r| r.get(0))
            .unwrap(),
        application_id: db
            .pragma_query_value(None, "application_id", |r| r.get(0))
            .unwrap(),
    }
}
fn clean_fk(db: &Connection) {
    assert!(!db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
}
fn backup_equal(before: &Image, after: &Image, path: &Path) {
    equal(&before.schema, &after.schema, path, "Backup schema objects");
    equal(
        &before.rows,
        &after.rows,
        path,
        "Backup all raw rows/rowids/receipts",
    );
    assert_eq!(before.version, after.version);
    assert_eq!(before.application_id, after.application_id);
    // SQLite's backup API changes the destination schema cookie to invalidate
    // prepared statements. Compare complete DDL, not that internal counter.
    assert!(after.schema_version > 0);
}
fn backups(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root)
        .unwrap()
        .map(Result::unwrap)
        .map(|r| r.path())
        .filter(|p| {
            // A query-only SQLite connection can still create -wal/-shm
            // sidecars. Preserve those files, but count actual backup DBs.
            let name = p.file_name().unwrap().to_str().unwrap();
            if !name.starts_with("actual.pre-v16-") || !name.ends_with(".sqlite3") {
                return false;
            }
            let id = name
                .strip_prefix("actual.pre-v16-")
                .unwrap()
                .strip_suffix(".sqlite3")
                .unwrap();
            let uuid = uuid::Uuid::parse_str(id).expect("Backup name must contain a UUID");
            assert_eq!(uuid.to_string(), id);
            assert_eq!(uuid.get_version_num(), 4);
            assert_eq!(fs::canonicalize(p).unwrap(), *p);
            private_read(p, 16_777_216);
            true
        })
        .collect();
    paths.sort();
    paths
}
struct Retained(Option<tempfile::TempDir>);
impl Drop for Retained {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(root) = self.0.take() {
                eprintln!(
                    "Retained failed actual15 migration: {}",
                    root.keep().display()
                );
            }
        }
    }
}
struct Fixture {
    _root: Retained,
    db: PathBuf,
    manifest: Value,
    original: PathBuf,
    original_sha: String,
}
impl Fixture {
    fn root(&self) -> &Path {
        self.db.parent().unwrap()
    }
    fn original_unchanged(&self) {
        assert_eq!(
            sha(&private_read(&self.original, 16_777_216)),
            self.original_sha
        );
    }
}
fn verify_provenance(input: &Path, m: &Value) {
    let raw = private_read(&input.join("tool-provenance.json"), 4_194_304);
    assert_eq!(sha(&raw), m["tool_provenance_sha256"]);
    let p: Value = serde_json::from_slice(&raw).unwrap();
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let architecture = if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "arm64"
    } else {
        std::env::consts::ARCH
    };
    assert_eq!(m["platform"], platform);
    assert_eq!(m["architecture"], architecture);
    if m["provenance_kind"] == "immutable_frozen_pair" {
        assert_eq!(platform, "linux");
        assert_eq!(architecture, "x86_64");
        assert_eq!(
            sha(&raw),
            "51949baa8e19abcd3832d8c2c2e63739055d04df2a0405ac09b0b0e05a1125ca"
        );
        assert_eq!(p["git_head"], SOURCE);
        assert_eq!(p["git_tree"], TREE);
        assert_eq!(p["schema"].as_u64(), Some(15));
        assert_eq!(p["tracked_clean"], true);
        for (name, digest) in [
            (
                "batond",
                "5f6e31504ad7e94c4c29cbe550da20506432be4a1034c8059e6ddf4b2bea3984",
            ),
            (
                "baton",
                "0c36b0d8d0eb01143e3a176e383ee15147443940a721712501dca146b083170f",
            ),
        ] {
            assert_eq!(p["binaries"][name], digest);
            assert_eq!(m["binaries"][name], digest);
        }
        let build = private_read(&input.join("original-build-evidence.json"), 4_194_304);
        assert_eq!(
            sha(&build),
            "2dc019b3e49d111088f0d75cf431bec9142f030c76fa0641d03bb760b8a04f99"
        );
        assert_eq!(sha(&build), m["original_build_evidence_sha256"]);
        assert_eq!(p["probe"]["schema"].as_u64(), Some(15));
        assert_eq!(
            p["probe"]["original_daemon_SIGTERM_wait_exit"].as_i64(),
            Some(0)
        );
        assert_eq!(p["probe"]["forced_kill"], false);
    } else {
        assert_eq!(m["provenance_kind"], "reviewed_source_build");
        assert_eq!(p["contract"], "agentbaton.schema15-source-build.v1");
        assert_eq!(p["status"], "compiled");
        assert_eq!(p["schema"].as_u64(), Some(15));
        assert_eq!(p["source"]["commit"], SOURCE);
        assert_eq!(p["source"]["tree"], TREE);
        assert_eq!(
            p["source"]["repository"],
            "https://github.com/wanderbot-auto/AgentBaton.git"
        );
        assert_eq!(p["source"]["compiled_source_clean"], true);
        assert!(digest(&p["source"]["archive_sha256"]));
        let files = p["source"]["files_sha256"].as_object().unwrap();
        assert!((2..=4096).contains(&files.len()));
        assert!(files.contains_key("Cargo.toml") && files.contains_key("Cargo.lock"));
        assert!(files
            .iter()
            .all(|(name, value)| !Path::new(name).is_absolute()
                && !Path::new(name)
                    .components()
                    .any(|p| matches!(p, std::path::Component::ParentDir))
                && digest(value)));
        assert_eq!(files["Cargo.lock"], p["build"]["lockfile_sha256"]);
        assert_eq!(
            p["builder_sha256"],
            sha(include_bytes!(
                "../../../scripts/prepare-schema15-migration-fixture.py"
            ))
        );
        assert_eq!(
            p["common_helper_sha256"],
            sha(include_bytes!(
                "../../../scripts/prepare-legacy-migration-fixture.py"
            ))
        );
        assert_eq!(p["platform"], platform);
        assert_eq!(p["architecture"], architecture);
        for name in ["cargo", "rustc"] {
            assert!(p["toolchain"][name]
                .as_str()
                .is_some_and(|v| !v.is_empty() && v.len() <= 512));
        }
        let argv = p["build"]["argv"].as_array().unwrap();
        assert_eq!(argv.len(), 10);
        assert!(argv[0].as_str().is_some_and(|v| Path::new(v).is_absolute()));
        equal(
            &argv[1..]
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect::<Vec<_>>(),
            &vec![
                "build",
                "--offline",
                "--locked",
                "-j",
                "2",
                "--bin",
                "batond",
                "--bin",
                "baton",
            ],
            input,
            "Exact source build command",
        );
        assert_eq!(p["build"]["exit_code"].as_i64(), Some(0));
        assert_eq!(p["build"]["default_features"], true);
        assert_eq!(p["build"]["features"], json!([]));
        equal(
            &p["binaries"],
            &m["tool_metadata"],
            input,
            "Measured source-built tool records",
        );
    }
    equal(
        &m["tool_metadata"],
        &m["tool_metadata_after"],
        input,
        "Frozen inode/metadata/byte records changed",
    );
    for name in ["batond", "baton"] {
        let record = &m["tool_metadata"][name];
        assert_eq!(record["sha256"], m["binaries"][name]);
        assert!(record["bytes"]
            .as_u64()
            .is_some_and(|n| n > 0 && n <= 268_435_456));
        assert_eq!(record["mode"], "0o500");
        assert_eq!(record["nlink"].as_u64(), Some(1));
        assert_eq!(
            record["uid"].as_u64(),
            Some(u64::from(rustix::process::geteuid().as_raw()))
        );
        for field in ["device", "inode", "mtime_ns", "ctime_ns"] {
            assert!(record[field]
                .as_str()
                .is_some_and(|v| v.parse::<u64>().is_ok_and(|n| n.to_string() == v)));
        }
        assert_eq!(record["architecture"], architecture);
        assert_eq!(
            record["format"],
            if platform == "darwin" {
                "mach-o-64"
            } else {
                "elf-64"
            }
        );
    }
}
fn fixture() -> Fixture {
    let input = PathBuf::from(
        std::env::var_os("AGENTBATON_ACTUAL_SCHEMA15_FIXTURE")
            .expect("Explicit genuine15 fixture is required for this mandatory dedicated target"),
    );
    let info = fs::symlink_metadata(&input).unwrap();
    assert!(info.is_dir() && !info.file_type().is_symlink());
    assert_eq!(info.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(info.permissions().mode() & 0o7777, 0o700);
    let input = input.canonicalize().unwrap();
    let m: Value =
        serde_json::from_slice(&private_read(&input.join("manifest.json"), 4_194_304)).unwrap();
    assert_eq!(m["contract"], "agentbaton.actual-task-schema15.v1");
    assert_eq!(m["status"], "passed");
    assert_eq!(m["origin"], "actual_source15_core_rpc");
    assert_eq!(m["schema"].as_u64(), Some(15));
    assert_eq!(m["source_commit"], SOURCE);
    assert_eq!(m["source_tree"], TREE);
    assert_eq!(
        m["generator_sha256"],
        sha(include_bytes!(
            "fixtures/task_workspace_actual15_migration/generate.py"
        ))
    );
    assert_eq!(
        m["helper_sha256"],
        sha(include_bytes!(
            "../../../scripts/prepare-schema15-migration-fixture.py"
        ))
    );
    assert_eq!(
        m["common_helper_sha256"],
        sha(include_bytes!(
            "../../../scripts/prepare-legacy-migration-fixture.py"
        ))
    );
    assert_eq!(
        m["native_helper_sha256"],
        sha(include_bytes!("../../../scripts/native-run.py"))
    );
    verify_provenance(&input, &m);
    assert_eq!(m["initial_probe"]["schema"].as_u64(), Some(15));
    assert_eq!(m["initial_probe"]["sequence"].as_u64(), Some(0));
    assert_eq!(m["initial_probe"]["counts"]["tasks"].as_u64(), Some(0));
    assert_eq!(m["initial_probe"]["query_only"], true);
    assert_eq!(m["actual_cli_probe"]["exit"].as_i64(), Some(0));
    assert_eq!(m["actual_cli_probe"]["api_version"].as_u64(), Some(1));
    assert_eq!(m["daemon"]["daemon_wait_exit"].as_i64(), Some(0));
    assert_eq!(m["daemon"]["sigterm_sent"], true);
    assert_eq!(m["daemon"]["forced_kill"], false);
    assert_eq!(m["daemon"]["identity_gone_after_join"], true);
    assert_eq!(m["daemon"]["leader_unreaped_until_signalling"], true);
    assert_eq!(m["daemon"]["group_quiescent_before_reaping"], true);
    assert_eq!(m["daemon"]["cleanup_errors"], json!([]));
    assert_eq!(m["backup_logical_exact"], true);
    let original = input.join("actual-schema15.sqlite3");
    let raw = private_read(&original, 16_777_216);
    let original_sha = sha(&raw);
    assert_eq!(original_sha, m["database_sha256"]);
    assert_eq!(raw.len() as u64, m["database_bytes"].as_u64().unwrap());
    let temp = tempfile::Builder::new()
        .prefix("actual15-migration-")
        .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
        .unwrap();
    let retained = Retained(Some(temp));
    let root = retained.0.as_ref().unwrap().path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    let db = root.join("actual.sqlite3");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&db)
        .unwrap();
    output.write_all(&raw).unwrap();
    output.sync_all().unwrap();
    drop(output);
    let fixture = Fixture {
        _root: retained,
        db,
        manifest: m,
        original,
        original_sha,
    };
    let connection = ro(&fixture.db);
    let old = image(&connection);
    assert_eq!(old.version, 15);
    clean_fk(&connection);
    assert_eq!(old.rows["tasks"].len(), 3);
    assert!(old.rows["events"].len() >= 5 && old.rows["idempotency"].len() >= 4);
    for table in FACTORY {
        assert!(old.rows[table].is_empty());
        assert_eq!(
            fixture.manifest["final_probe"]["counts"][table].as_u64(),
            Some(0)
        );
    }
    for table in [
        "approval_proposals",
        "execution_grants",
        "sessions",
        "attempts",
        "managed_workspaces",
        "goal_scopes",
        "goal_runs",
        "goal_graph_capabilities",
        "connections",
    ] {
        assert!(old.rows[table].is_empty());
    }
    assert_eq!(fixture.manifest["tasks"].as_array().unwrap().len(), 3);
    for record in fixture.manifest["tasks"].as_array().unwrap() {
        let raw: String = connection
            .query_row(
                "SELECT document FROM tasks WHERE id=?1",
                [record["id"].as_str().unwrap()],
                |r| r.get(0),
            )
            .unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert!(value.get("code_workspace_binding").is_none());
        for field in ["id", "project_id", "goal", "state", "revision"] {
            equal(
                &value[field],
                &record[field],
                &fixture.db,
                "Real old Task row does not match retained RPC metadata",
            );
        }
        let task: Task = serde_json::from_str(&raw).unwrap();
        assert!(task.code_workspace_binding.is_none());
    }
    drop(connection);
    fixture
}
fn reject_unchanged(f: &Fixture, expected_code: &str) {
    let before = image(&ro(&f.db));
    let raw = fs::read(&f.db).unwrap();
    let error = Store::open(&f.db)
        .err()
        .expect("Corrupt genuine15 copy must reject");
    assert_eq!(error.code, expected_code);
    let after = image(&ro(&f.db));
    equal(
        &before,
        &after,
        &f.db,
        "Rejected migration changed logical old history",
    );
    equal(
        &raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Rejected migration changed main file bytes",
    );
    let copies = backups(f.root());
    assert_eq!(copies.len(), 1);
    let backup = &copies[0];
    private_read(backup, 16_777_216);
    backup_equal(&before, &image(&ro(backup)), backup);
    f.original_unchanged();
}

#[test]
fn genuine15_task_history_migrates16_without_binding_or_raw_history_rewrite() {
    let f = fixture();
    let before = image(&ro(&f.db));
    let store = Store::open(&f.db).unwrap();
    let after = image(&ro(&f.db));
    assert_eq!(after.version, 16);
    equal(
        &before.rows,
        &after.rows,
        &f.db,
        "Every predecessor row/raw document/receipt/sequence",
    );
    equal(
        &before.schema,
        &after.schema,
        &f.db,
        "Schema16 adds no table or DDL",
    );
    assert_eq!(before.schema_version, after.schema_version);
    assert_eq!(before.application_id, after.application_id);
    for record in f.manifest["tasks"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        let task = store.get(id).unwrap();
        assert!(task.code_workspace_binding.is_none());
        let raw: String = ro(&f.db)
            .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        equal(
            &serde_json::to_string(&task).unwrap(),
            &raw,
            &f.db,
            "Typed old None serializes without a new field",
        );
    }
    let copies = backups(f.root());
    assert_eq!(copies.len(), 1);
    private_read(&copies[0], 16_777_216);
    backup_equal(&before, &image(&ro(&copies[0])), &copies[0]);
    clean_fk(&ro(&f.db));
    drop(store);
    drop(Store::open(&f.db).unwrap());
    assert_eq!(backups(f.root()).len(), 1);
    equal(
        &after,
        &image(&ro(&f.db)),
        &f.db,
        "Reopen must not bind/recover unchanged old Tasks",
    );
    f.original_unchanged();
}
#[test]
fn genuine15_original_receipts_replay_after16_without_mutation_or_new_authority() {
    let f = fixture();
    let mut store = Store::open(&f.db).unwrap();
    let before = image(&ro(&f.db));
    for record in f.manifest["tasks"].as_array().unwrap() {
        let project = record["project_id"].as_str().unwrap();
        let goal = record["goal"].as_str().unwrap();
        let key = record["create_key"].as_str().unwrap();
        let original: String = ro(&f.db)
            .query_row(
                "SELECT result FROM idempotency WHERE scope=?1 AND key=?2",
                params![format!("local:{project}:tasks.create"), key],
                |r| r.get(0),
            )
            .unwrap();
        let replay = store.create_registered(project, goal, key).unwrap();
        assert!(replay.code_workspace_binding.is_none());
        equal(
            &serde_json::to_string(&replay).unwrap(),
            &original,
            &f.db,
            "Original create receipt must remain immutable",
        );
    }
    equal(
        &before,
        &image(&ro(&f.db)),
        &f.db,
        "Historical replay must not mutate events/receipts/authority",
    );
    f.original_unchanged();
}
#[test]
fn genuine15_corrupt_task_validation_rejects_preserving_all_old_rows_version_and_backup() {
    for variant in [
        "field_null",
        "field_object",
        "wrong_doc_id",
        "zero_revision",
        "zero_spec",
        "oversize",
    ] {
        let f = fixture();
        let db = Connection::open(&f.db).unwrap();
        let (id, raw): (String, String) = db
            .query_row(
                "SELECT id,document FROM tasks ORDER BY rowid DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let mut value: Value = serde_json::from_str(&raw).unwrap();
        match variant {
            "field_null" => value["code_workspace_binding"] = Value::Null,
            "field_object" => value["code_workspace_binding"] = json!({}),
            "wrong_doc_id" => value["id"] = json!("corrupt-copy-only"),
            "zero_revision" => value["revision"] = json!(0),
            "zero_spec" => value["spec_revision"] = json!(0),
            "oversize" => {}
            _ => unreachable!(),
        }
        let corrupt = if variant == "oversize" {
            raw + &" ".repeat(262_145)
        } else {
            serde_json::to_string(&value).unwrap()
        };
        db.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![corrupt, id],
        )
        .unwrap();
        drop(db);
        // The highest-rowid record is a controlled negative copy. Core's
        // unordered validation SELECT does not prove it was validated last.
        reject_unchanged(&f, "INPUT_CHANGED");
    }
}
#[test]
fn genuine15_foreign_key_corruption_is_rejected_without_erasing_unknown_rows() {
    let f = fixture();
    let db = Connection::open(&f.db).unwrap();
    clean_fk(&db);
    // Only the disposable negative copy is corrupted. New rusqlite
    // connections enforce FKs by default; restore enforcement before handing
    // the retained orphan to the unchanged production Store::open checks.
    db.pragma_update(None, "foreign_keys", false).unwrap();
    assert!(!db
        .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
        .unwrap());
    assert_eq!(db.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,payload,root) VALUES('corrupt-copy','missing-project','{}','creating','negative','{}','/unopened-negative-only')",[]).unwrap(), 1);
    db.pragma_update(None, "foreign_keys", true).unwrap();
    assert!(db
        .pragma_query_value(None, "foreign_keys", |r| r.get::<_, bool>(0))
        .unwrap());
    let violations: Vec<(String, i64, String, i64)> = db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].0, "managed_workspaces");
    assert_eq!(violations[0].2, "projects");
    assert_eq!(
        db.query_row(
            "SELECT rowid FROM managed_workspaces WHERE id='corrupt-copy'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        violations[0].1
    );
    drop(db);
    reject_unchanged(&f, "INPUT_CHANGED");
}
#[test]
fn genuine15_copy_labeled_future17_is_refused_before_backup_or_recovery() {
    let f = fixture();
    let db = Connection::open(&f.db).unwrap();
    db.pragma_update(None, "user_version", 17u32).unwrap();
    drop(db);
    let before = image(&ro(&f.db));
    let raw = fs::read(&f.db).unwrap();
    let error = Store::open(&f.db).err().unwrap();
    assert_eq!(error.code, "VERSION_UNSUPPORTED");
    equal(
        &before,
        &image(&ro(&f.db)),
        &f.db,
        "Future17 refusal must not mutate logical state",
    );
    equal(
        &raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Future17 refusal must not touch raw file",
    );
    assert!(backups(f.root()).is_empty());
    f.original_unchanged();
}
