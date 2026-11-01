//! Required-feature genuine source16 -> Store17 migration; no synthetic old-schema success.
//! Root registers required-features=["actual-schema16-migration-tests"].
use agentbaton_core::{
    storage::{Store, Task},
    TaskGitWorkspacePreparation,
};
use rusqlite::{params, types::Value as SqlValue, Connection, OpenFlags};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const SOURCE: &str = "0644ea9d8c10d075fef6b139bd9d7d363d5cc4fb";
const TREE: &str = "107895dfd43c996c668017d556f938d01fa17d3d";
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
    journal: String,
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
    let path_after = fs::symlink_metadata(path).unwrap();
    assert!(!path_after.file_type().is_symlink() && pin(&after) == pin(&path_after));
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
        journal: db
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
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
            if !name.starts_with("actual.pre-v17-") || !name.ends_with(".sqlite3") {
                return false;
            }
            let id = name
                .strip_prefix("actual.pre-v17-")
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
                    "Retained failed actual16 migration: {}",
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

const SOURCE_FILES: [(&str, &str); 5] = [
    (
        "Cargo.toml",
        "565d77f9b3e5a08bd7ef67b7f21cd53bfafe3f23b35e0f8e8a1286fdc1a82949",
    ),
    (
        "Cargo.lock",
        "b22c148dd432426f70586093394eeb9f04c0aeca938764be503e1145659f7d42",
    ),
    (
        "crates/core/src/storage.rs",
        "5fe4b98b2321ecf26f35a5d825d8bfc5baf3b74e78d96927e90a2ceacd090aec",
    ),
    (
        "crates/core/src/task_workspace_binding.rs",
        "5aa76ce8e535f2bf6fc5d26aeaa520136d417149df6d087c7f18b2796817b7ba",
    ),
    (
        "crates/core/src/lib.rs",
        "2044c7f67d1a314d8feb9f2e26016073710e897dc374c410e520547ad68935ec",
    ),
];

const OLD_TABLES: [&str; 55] = [
    "approval_proposals",
    "attempts",
    "budgets",
    "command_validation_policies",
    "command_validation_runs",
    "command_workspace_fences",
    "connections",
    "data_validation_policies",
    "data_validation_runs",
    "data_workspace_creations",
    "data_workspace_previews",
    "data_workspace_receipts",
    "dispatch_intents",
    "events",
    "execution_grants",
    "factory_attempts",
    "factory_budget_reservations",
    "factory_child_goal_runs",
    "factory_child_plan_heads",
    "factory_child_plan_revisions",
    "factory_delegations",
    "factory_handoffs",
    "factory_runs",
    "factory_scoped_handles",
    "factory_scopes",
    "factory_task_bindings",
    "factory_task_lineage",
    "goal_external_action_journal",
    "goal_grant_revisions",
    "goal_graph_acceptances",
    "goal_graph_candidates",
    "goal_graph_capabilities",
    "goal_graph_jobs",
    "goal_graph_owner_decisions",
    "goal_graph_validations",
    "goal_ledgers",
    "goal_node_evidence",
    "goal_plan_heads",
    "goal_plan_proposals",
    "goal_plan_revisions",
    "goal_run_nodes",
    "goal_run_reservations",
    "goal_runs",
    "goal_scopes",
    "idempotency",
    "managed_workspaces",
    "projects",
    "session_runtime_snapshots",
    "session_turns",
    "sessions",
    "sqlite_sequence",
    "tasks",
    "validation_policies",
    "validation_runs",
    "workspace_leases",
];
const NEW_TABLES: [&str; 4] = [
    "integration_policies",
    "integrations",
    "integration_attempts",
    "integration_ref_fences",
];

type Check = std::result::Result<(), &'static str>;
macro_rules! require {
    ($condition:expr, $message:literal) => {
        if !$condition {
            return Err($message);
        }
    };
}
fn platform() -> (&'static str, &'static str) {
    (
        if cfg!(target_os = "macos") {
            "darwin"
        } else {
            "linux"
        },
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "arm64"
        } else {
            std::env::consts::ARCH
        },
    )
}
fn verify_provenance(m: &Value, p: &Value, provenance_bytes: &[u8]) -> Check {
    let (platform, architecture) = platform();
    require!(
        m["provenance_kind"] == "reviewed_source_build",
        "No unbound frozen-pair fallback"
    );
    require!(
        m["tool_provenance_sha256"] == sha(provenance_bytes),
        "Provenance bytes changed"
    );
    require!(
        p["contract"] == "agentbaton.schema16-source-build.v1"
            && p["status"] == "compiled"
            && p["schema"].as_u64() == Some(16),
        "Exact completed16 build required"
    );
    require!(
        p["source"]["commit"] == SOURCE
            && p["source"]["tree"] == TREE
            && p["source"]["repository"] == "https://github.com/wanderbot-auto/AgentBaton.git"
            && p["source"]["compiled_source_clean"] == true,
        "Wrong historical source"
    );
    require!(
        digest(&p["source"]["archive_sha256"]),
        "Missing source archive hash"
    );
    let files = p["source"]["files_sha256"]
        .as_object()
        .ok_or("Missing source file map")?;
    require!(
        (2..=4096).contains(&files.len()),
        "Unbounded source file map"
    );
    require!(
        files
            .iter()
            .all(|(name, value)| !Path::new(name).is_absolute()
                && Path::new(name)
                    .components()
                    .all(|v| matches!(v, std::path::Component::Normal(_)))
                && digest(value)),
        "Invalid source path/hash"
    );
    for (name, expected) in SOURCE_FILES {
        require!(
            files.get(name).is_some_and(|v| v == expected),
            "Critical historical source bytes differ"
        );
    }
    require!(
        p["build"]["lockfile_sha256"] == files["Cargo.lock"],
        "Lock hash mismatch"
    );
    require!(
        p["builder_sha256"]
            == sha(include_bytes!(
                "../../../scripts/prepare-schema16-migration-fixture.py"
            ))
            && p["common_helper_sha256"]
                == sha(include_bytes!(
                    "../../../scripts/prepare-legacy-migration-fixture.py"
                )),
        "Unreviewed build helpers"
    );
    require!(
        m["platform"] == platform
            && m["architecture"] == architecture
            && p["platform"] == platform
            && p["architecture"] == architecture,
        "Wrong native platform"
    );
    for name in ["cargo", "rustc"] {
        require!(
            p["toolchain"][name]
                .as_str()
                .is_some_and(|v| !v.is_empty() && v.len() <= 512),
            "Missing measured toolchain"
        );
    }
    let argv = p["build"]["argv"]
        .as_array()
        .ok_or("Missing build command")?;
    require!(
        argv.len() == 10
            && argv[0].as_str().is_some_and(|v| Path::new(v).is_absolute())
            && argv[1..]
                == json!([
                    "build",
                    "--offline",
                    "--locked",
                    "-j",
                    "2",
                    "--bin",
                    "batond",
                    "--bin",
                    "baton"
                ])
                .as_array()
                .unwrap()[..],
        "Wrong locked default build command"
    );
    require!(
        p["build"]["exit_code"].as_i64() == Some(0)
            && p["build"]["default_features"] == true
            && p["build"]["features"] == json!([]),
        "Build did not finish normally"
    );
    require!(
        p["binaries"] == m["tool_metadata"] && m["tool_metadata"] == m["tool_metadata_after"],
        "Measured tool records changed"
    );
    require!(
        m["binaries"].as_object().is_some_and(|v| v.len() == 2)
            && m["tool_metadata"].as_object().is_some_and(|v| v.len() == 2),
        "Exact dual tools required"
    );
    for name in ["batond", "baton"] {
        let record = &m["tool_metadata"][name];
        require!(
            digest(&m["binaries"][name]) && record["sha256"] == m["binaries"][name],
            "Tool SHA mismatch"
        );
        require!(
            record["bytes"]
                .as_u64()
                .is_some_and(|v| v > 0 && v <= 268_435_456)
                && record["mode"] == "0o500"
                && record["nlink"].as_u64() == Some(1)
                && record["uid"].as_u64() == Some(u64::from(rustix::process::geteuid().as_raw())),
            "Nonprivate tool metadata"
        );
        for field in ["device", "inode", "mtime_ns", "ctime_ns"] {
            require!(
                record[field]
                    .as_str()
                    .is_some_and(|v| v.parse::<u64>().is_ok_and(|n| n.to_string() == v)),
                "Noncanonical lossless tool pin"
            );
        }
        require!(
            record["architecture"] == architecture
                && record["format"]
                    == if platform == "darwin" {
                        "mach-o-64"
                    } else {
                        "elf-64"
                    },
            "Tool architecture mismatch"
        );
    }
    Ok(())
}
fn verify_manifest(m: &Value) -> Check {
    require!(
        m["contract"] == "agentbaton.actual-task-schema16.v1"
            && m["status"] == "passed"
            && m["origin"] == "actual_source16_core_rpc",
        "Actual product RPC fixture required"
    );
    require!(
        m["schema"].as_u64() == Some(16)
            && m["source_commit"] == SOURCE
            && m["source_tree"] == TREE,
        "Wrong predecessor provenance"
    );
    require!(
        m["generator_sha256"]
            == sha(include_bytes!(
                "../../../scripts/generate-schema16-migration-fixture.py"
            ))
            && m["helper_sha256"]
                == sha(include_bytes!(
                    "../../../scripts/prepare-schema16-migration-fixture.py"
                ))
            && m["common_helper_sha256"]
                == sha(include_bytes!(
                    "../../../scripts/prepare-legacy-migration-fixture.py"
                ))
            && m["native_helper_sha256"] == sha(include_bytes!("../../../scripts/native-run.py")),
        "Producer/helper source mismatch"
    );
    require!(
        m["initial_probe"]["schema"].as_u64() == Some(16)
            && m["initial_probe"]["sequence"].as_u64() == Some(0)
            && m["initial_probe"]["query_only"] == true
            && m["initial_probe"]["foreign_key_errors"].as_u64() == Some(0),
        "Not a fresh actual16 runtime"
    );
    let counts = m["initial_probe"]["counts"]
        .as_object()
        .ok_or("Missing initial table counts")?;
    require!(
        counts.len() == 55
            && OLD_TABLES
                .iter()
                .all(|name| counts.get(*name).is_some_and(|v| v.as_u64() == Some(0))),
        "Incomplete/nonempty initial16 tables"
    );
    require!(
        m["actual_cli_probe"]["exit"].as_i64() == Some(0)
            && m["actual_cli_probe"]["api_version"].as_u64() == Some(1)
            && m["actual_cli_probe"]["sequence"].as_u64() == Some(0),
        "Companion did not probe real empty16"
    );
    require!(
        m["prepared_task_count"].as_u64() == Some(1)
            && m["source_git_unchanged"] == true
            && m["backup_logical_exact"] == true,
        "Preparation/backup did not preserve source"
    );
    require!(
        m["runtime_mode"] == "0o700"
            && m["socket_utf8_bytes"]
                .as_u64()
                .is_some_and(|v| v > 0 && v <= 100),
        "Private bounded Unix runtime required"
    );
    let daemon = &m["daemon"];
    require!(
        daemon["sigterm_sent"] == true
            && daemon["daemon_wait_exit"].as_i64() == Some(0)
            && daemon["forced_kill"] == false
            && daemon["identity_gone_after_join"] == true
            && daemon["leader_unreaped_until_signalling"] == true
            && daemon["group_quiescent_before_reaping"] == true
            && daemon["cleanup_errors"] == json!([]),
        "Owned daemon did not really join cleanly"
    );
    let pid = daemon["owned_identity"]["pid"]
        .as_u64()
        .ok_or("Missing owned PID")?;
    require!(
        pid > 1 && daemon["owned_identity"]["pgid"].as_u64() == Some(pid),
        "Unbound owned group identity"
    );
    require!(
        digest(&m["database_sha256"])
            && m["database_bytes"]
                .as_u64()
                .is_some_and(|v| v > 0 && v <= 16_777_216),
        "Missing bounded actual database hash"
    );
    Ok(())
}
fn verify_database(m: &Value, db: &Connection) -> Check {
    let old = image(db);
    require!(
        old.version == 16
            && old.rows.len() == 55
            && old.rows.keys().map(String::as_str).eq(OLD_TABLES),
        "Not complete original55 schema16 tables"
    );
    require!(
        !db.prepare("PRAGMA foreign_key_check")
            .map_err(|_| "FK query failed")?
            .exists([])
            .map_err(|_| "FK query failed")?,
        "Old foreign keys corrupt"
    );
    require!(
        m["final_probe"]["schema"].as_u64() == Some(16)
            && m["final_probe"]["query_only"] == true
            && m["final_probe"]["foreign_key_errors"].as_u64() == Some(0),
        "Missing final read-only16 observation"
    );
    let counts = m["final_probe"]["counts"]
        .as_object()
        .ok_or("Missing final table counts")?;
    require!(
        counts.len() == 55
            && old.rows.iter().all(|(name, rows)| counts
                .get(name)
                .is_some_and(|v| v.as_u64() == Some(rows.len() as u64))),
        "Rehashed database/count mismatch"
    );
    require!(
        old.rows["tasks"].len() == 3
            && old.rows["managed_workspaces"].len() == 1
            && old.rows["events"].len() >= 5
            && old.rows["idempotency"].len() >= 5,
        "Missing actual history"
    );
    for name in FACTORY.into_iter().chain([
        "approval_proposals",
        "execution_grants",
        "sessions",
        "attempts",
        "goal_scopes",
        "goal_runs",
        "goal_graph_capabilities",
        "connections",
    ]) {
        require!(
            old.rows[name].is_empty(),
            "Unexpected old execution/Provider authority"
        );
    }
    let sequence: i64 = db
        .query_row("SELECT coalesce(max(sequence),0) FROM events", [], |r| {
            r.get(0)
        })
        .map_err(|_| "Event query failed")?;
    let sequence = u64::try_from(sequence).map_err(|_| "Negative event sequence")?;
    require!(
        m["final_api_sequence"].as_u64() == Some(sequence)
            && m["final_probe"]["sequence"].as_u64() == Some(sequence),
        "Event counter differs from actual RPC"
    );
    let records = m["tasks"].as_array().ok_or("Missing real task metadata")?;
    require!(records.len() == 3, "Wrong actual task count");
    let mut ids = std::collections::BTreeSet::new();
    let mut ready = 0;
    let mut cancelled = 0;
    for record in records {
        let id = record["id"].as_str().ok_or("Missing Task id")?;
        require!(!id.is_empty() && ids.insert(id), "Duplicate Task metadata");
        let raw: String = db
            .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .map_err(|_| "Missing actual Task row")?;
        let value: Value = serde_json::from_str(&raw).map_err(|_| "Invalid old Task JSON")?;
        let task: Task = serde_json::from_str(&raw).map_err(|_| "Invalid old Task fields")?;
        for field in ["id", "project_id", "goal", "state", "revision"] {
            require!(
                value[field] == record[field],
                "Old Task differs from RPC observation"
            );
        }
        require!(
            task.revision > 0
                && task.spec_revision == 1
                && task.acceptance_ref.is_none()
                && task.acceptance_sequence.is_none(),
            "Task must not imply acceptance"
        );
        if let Some(binding) = task.code_workspace_binding {
            ready += 1;
            let raw_binding =
                serde_json::to_value(&binding).map_err(|_| "Binding serialization failed")?;
            require!(
                raw_binding == record["workspace_binding"]
                    && raw_binding["state"] == "ready"
                    && raw_binding["receipt"]["task_id"] == id,
                "Genuine ready binding changed"
            );
            let receipt = &binding.receipt;
            let stored: String = db
                .query_row(
                    "SELECT result FROM idempotency WHERE scope=?1 AND key=?2",
                    params![
                        format!("local:{id}:task-workspace.prepare_git"),
                        receipt.request_key
                    ],
                    |r| r.get(0),
                )
                .map_err(|_| "Missing original preparation receipt")?;
            require!(
                serde_json::from_str::<Value>(&stored)
                    .map_err(|_| "Invalid preparation receipt")?
                    == raw_binding["receipt"],
                "Preparation receipt not exact original"
            );
        } else {
            require!(
                record.get("workspace_binding").is_none(),
                "Lost original ready binding"
            );
        }
        if value["state"] == "cancelled" {
            cancelled += 1;
        }
        let key = record["create_key"]
            .as_str()
            .ok_or("Missing original create key")?;
        let original: String = db
            .query_row(
                "SELECT result FROM idempotency WHERE scope=?1 AND key=?2",
                params![format!("local:{}:tasks.create", task.project_id), key],
                |r| r.get(0),
            )
            .map_err(|_| "Missing original create receipt")?;
        let receipt: Value =
            serde_json::from_str(&original).map_err(|_| "Invalid original create receipt")?;
        require!(
            receipt["id"] == id
                && receipt["goal"] == task.goal
                && receipt["project_id"] == task.project_id
                && receipt["state"] == "draft"
                && receipt["revision"].as_u64() == Some(1)
                && receipt.get("code_workspace_binding").is_none(),
            "Original create receipt was rewritten"
        );
    }
    require!(
        ready == 1 && cancelled == 1,
        "Missing ready/cancelled history"
    );
    Ok(())
}

fn fixture() -> Fixture {
    let input = PathBuf::from(
        std::env::var_os("AGENTBATON_ACTUAL_SCHEMA16_FIXTURE")
            .expect("Exact actual16 manifest FILE is mandatory for this required-feature target"),
    );
    assert!(input.is_absolute());
    assert_eq!(input.canonicalize().unwrap(), input);
    let parent = input.parent().unwrap();
    let directory = fs::symlink_metadata(parent).unwrap();
    assert!(directory.is_dir() && !directory.file_type().is_symlink());
    assert_eq!(directory.uid(), rustix::process::geteuid().as_raw());
    assert_eq!(directory.mode() & 0o7777, 0o700);
    let m: Value = serde_json::from_slice(&private_read(&input, 4_194_304)).unwrap();
    assert!(
        verify_manifest(&m).is_ok(),
        "Actual16 producer manifest contract rejected"
    );
    let provenance_bytes = private_read(&parent.join("tool-provenance.json"), 4_194_304);
    let provenance: Value = serde_json::from_slice(&provenance_bytes).unwrap();
    assert!(
        verify_provenance(&m, &provenance, &provenance_bytes).is_ok(),
        "Historical16 tool source provenance rejected"
    );
    let original = parent.join("actual-schema16.sqlite3");
    let raw = private_read(&original, 16_777_216);
    let original_sha = sha(&raw);
    assert_eq!(original_sha, m["database_sha256"]);
    assert_eq!(Some(raw.len() as u64), m["database_bytes"].as_u64());
    let temp = tempfile::Builder::new()
        .prefix("actual16-migration-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
        .unwrap();
    let retained = Retained(Some(temp));
    let db = retained.0.as_ref().unwrap().path().join("actual.sqlite3");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&db)
        .unwrap();
    output.write_all(&raw).unwrap();
    output.sync_all().unwrap();
    drop(output);
    let f = Fixture {
        _root: retained,
        db,
        manifest: m,
        original,
        original_sha,
    };
    assert!(
        verify_database(&f.manifest, &ro(&f.db)).is_ok(),
        "Genuine16 raw database and RPC history disagree"
    );
    f.original_unchanged();
    f
}
fn zero_integration(store: &Store, path: &Path) {
    assert_eq!(
        serde_json::to_value(store.integration_record_counts().unwrap()).unwrap(),
        json!({"policies":0,"runs":0,"attempts":0,"ref_fences":0})
    );
    let db = ro(path);
    for name in NEW_TABLES.into_iter().chain(FACTORY).chain([
        "approval_proposals",
        "execution_grants",
        "sessions",
        "attempts",
        "goal_scopes",
        "goal_runs",
        "goal_graph_capabilities",
        "connections",
    ]) {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {name}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(u64::try_from(count).expect("Negative domain row count"), 0);
    }
    clean_fk(&db);
}
fn assert_additive(before: &Image, after: &Image, path: &Path) {
    assert_eq!(before.version, 16);
    assert_eq!(after.version, 17);
    assert_eq!(after.journal, "wal");
    assert_eq!(before.rows.len(), 55);
    assert_eq!(after.rows.len(), 59);
    let old_schema: Schema = after
        .schema
        .iter()
        .filter(|r| !NEW_TABLES.contains(&r.2.as_str()))
        .cloned()
        .collect();
    equal(
        &before.schema,
        &old_schema,
        path,
        "Every original55 table/index/trigger DDL",
    );
    let old_rows: Rows = after
        .rows
        .iter()
        .filter(|(name, _)| !NEW_TABLES.contains(&name.as_str()))
        .map(|(name, rows)| (name.clone(), rows.clone()))
        .collect();
    equal(
        &before.rows,
        &old_rows,
        path,
        "Every old rowid/raw document/receipt/event/counter",
    );
    for table in NEW_TABLES {
        assert!(after.rows[table].is_empty());
    }
    assert_eq!(before.application_id, after.application_id);
}
fn reject_unchanged(f: &Fixture, expected_code: Option<&str>) {
    let before = image(&ro(&f.db));
    let raw = fs::read(&f.db).unwrap();
    let error = Store::open(&f.db)
        .err()
        .expect("Malformed genuine16 disposable copy must reject");
    if let Some(code) = expected_code {
        assert_eq!(error.code, code);
    }
    equal(
        &before,
        &image(&ro(&f.db)),
        &f.db,
        "Rejected migration changed old image/version/DDL",
    );
    equal(
        &raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Rejected migration changed original main bytes",
    );
    let copies = backups(f.root());
    assert_eq!(copies.len(), 1);
    backup_equal(&before, &image(&ro(&copies[0])), &copies[0]);
    f.original_unchanged();
}

#[test]
fn genuine16_all55_tables_raw_history_and_ready_git_receipt_survive_additive17_and_restart() {
    let f = fixture();
    let before = image(&ro(&f.db));
    let store = Store::open(&f.db).unwrap();
    let after = image(&ro(&f.db));
    assert_additive(&before, &after, &f.db);
    zero_integration(&store, &f.db);
    for record in f.manifest["tasks"].as_array().unwrap() {
        let id = record["id"].as_str().unwrap();
        let task = store.get(id).unwrap();
        let raw: String = ro(&f.db)
            .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        equal(
            &serde_json::to_string(&task).unwrap(),
            &raw,
            &f.db,
            "Typed Task must not normalize/lose raw fields",
        );
    }
    let copies = backups(f.root());
    assert_eq!(copies.len(), 1);
    backup_equal(&before, &image(&ro(&copies[0])), &copies[0]);
    drop(store);
    let reopened = Store::open(&f.db).unwrap();
    equal(
        &after,
        &image(&ro(&f.db)),
        &f.db,
        "Restart must not rewrite history or mint integration state",
    );
    zero_integration(&reopened, &f.db);
    assert_eq!(backups(f.root()).len(), 1);
    f.original_unchanged();
}

#[test]
fn genuine16_three_create_receipts_and_ready_preparation_replay_without_new_writes_or_authority() {
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
            "Original draft receipt must survive later ready/cancelled states",
        );
        assert_eq!(
            store
                .create_registered(project, "Conflicting same old key", key)
                .err()
                .unwrap()
                .code,
            "CONFIG_CONFLICT"
        );
    }
    let record = f.manifest["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v.get("workspace_binding").is_some())
        .unwrap();
    let task = store.get(record["id"].as_str().unwrap()).unwrap();
    let receipt = task.code_workspace_binding.unwrap().receipt;
    let request = TaskGitWorkspacePreparation {
        task_id: receipt.task_id.clone(),
        expected_task_revision: receipt.original_task_revision,
        expected_spec_revision: receipt.spec_revision,
        expected_project_revision: receipt.project_revision,
        host_id: receipt.host_id.clone(),
        expected_source_pin: receipt.source_pin.clone(),
        preparation_key: receipt.preparation_key.clone(),
    };
    let replay = store
        .prepare_task_git_workspace(&request, &receipt.managed_root, &receipt.request_key)
        .unwrap();
    assert_eq!(replay.original_receipt, receipt);
    assert!(replay.identity_revalidated && !replay.execution_authorized);
    assert_eq!(
        serde_json::to_value(replay.binding).unwrap(),
        record["workspace_binding"]
    );
    equal(
        &before,
        &image(&ro(&f.db)),
        &f.db,
        "Historical replays/conflicts must preserve every raw row and sequence",
    );
    zero_integration(&store, &f.db);
    f.original_unchanged();
}

#[test]
fn genuine16_copy_labelled_future18_is_refused_without_backup_or_any_byte_change() {
    let f = fixture();
    let db = Connection::open(&f.db).unwrap();
    db.pragma_update(None, "user_version", 18u32).unwrap();
    drop(db);
    let before = image(&ro(&f.db));
    let raw = fs::read(&f.db).unwrap();
    assert_eq!(
        Store::open(&f.db).err().unwrap().code,
        "VERSION_UNSUPPORTED"
    );
    equal(
        &before,
        &image(&ro(&f.db)),
        &f.db,
        "Future18 logical image changed",
    );
    equal(
        &raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Future18 raw bytes changed",
    );
    assert!(backups(f.root()).is_empty());
    f.original_unchanged();
}

#[test]
fn genuine16_orphan_foreign_key_rejects_without_rewriting_history_or_partial17() {
    let f = fixture();
    let db = Connection::open(&f.db).unwrap();
    clean_fk(&db);
    db.pragma_update(None, "foreign_keys", false).unwrap();
    assert_eq!(
        db.execute(
            "UPDATE managed_workspaces SET project_id='missing-negative-project'",
            []
        )
        .unwrap(),
        1
    );
    db.pragma_update(None, "foreign_keys", true).unwrap();
    assert!(db
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .exists([])
        .unwrap());
    drop(db);
    reject_unchanged(&f, Some("INPUT_CHANGED"));
}

#[test]
fn genuine16_malformed_task_json_and_boolean_fields_refuse_without_partial_commit() {
    for variant in ["invalid_json", "boolean_revision", "boolean_ready_inode"] {
        let f = fixture();
        let db = Connection::open(&f.db).unwrap();
        let id = f.manifest["tasks"][0]["id"].as_str().unwrap();
        let raw: String = db
            .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        let mut value: Value = serde_json::from_str(&raw).unwrap();
        let corrupt = match variant {
            "invalid_json" => "not valid JSON".into(),
            "boolean_revision" => {
                value["revision"] = json!(true);
                serde_json::to_string(&value).unwrap()
            }
            "boolean_ready_inode" => {
                value["code_workspace_binding"]["managed_pin"]["inode"] = json!(true);
                serde_json::to_string(&value).unwrap()
            }
            _ => unreachable!(),
        };
        db.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![corrupt, id],
        )
        .unwrap();
        drop(db);
        reject_unchanged(&f, None);
    }
}

#[test]
fn genuine16_committed_wal_and_late_ddl_conflict_roll_back_all17_changes() {
    let f = fixture();
    let keeper = Connection::open(&f.db).unwrap();
    keeper.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE integration_ref_fences(sentinel BLOB); INSERT INTO integration_ref_fences VALUES(x'00ffdeadbeef');").unwrap();
    let before = image(&keeper);
    let raw = fs::read(&f.db).unwrap();
    let wal = f.db.with_file_name("actual.sqlite3-wal");
    let wal_raw = fs::read(&wal).unwrap();
    assert!(wal_raw.len() > 32);
    assert!(Store::open(&f.db).is_err());
    equal(
        &before,
        &image(&keeper),
        &f.db,
        "Late DDL failure changed committed WAL image",
    );
    equal(
        &raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Failure changed main file",
    );
    equal(
        &wal_raw,
        &fs::read(&wal).unwrap(),
        &f.db,
        "Failure changed WAL frames",
    );
    for table in [
        "integration_policies",
        "integrations",
        "integration_attempts",
    ] {
        assert!(!image(&keeper).rows.contains_key(table));
    }
    let copies = backups(f.root());
    assert_eq!(copies.len(), 1);
    backup_equal(&before, &image(&ro(&copies[0])), &copies[0]);
    f.original_unchanged();
}

#[test]
fn genuine16_wrong_provenance_boolean_and_rehashed_build_claims_fail_closed() {
    let f = fixture();
    let parent = f.original.parent().unwrap();
    let raw = private_read(&parent.join("tool-provenance.json"), 4_194_304);
    let p: Value = serde_json::from_slice(&raw).unwrap();
    assert!(
        verify_manifest(&f.manifest).is_ok() && verify_provenance(&f.manifest, &p, &raw).is_ok()
    );
    for version in [json!(15), json!(17), json!(16.0), json!("16"), json!(true)] {
        let mut bad_m = f.manifest.clone();
        bad_m["schema"] = version.clone();
        assert!(verify_manifest(&bad_m).is_err());
        let mut bad_p = p.clone();
        bad_p["schema"] = version;
        let rehashed = serde_json::to_vec(&bad_p).unwrap();
        bad_m = f.manifest.clone();
        bad_m["tool_provenance_sha256"] = json!(sha(&rehashed));
        assert!(verify_provenance(&bad_m, &bad_p, &rehashed).is_err());
    }
    for field in [
        "schema",
        "source_commit",
        "source_tree",
        "source_git_unchanged",
        "prepared_task_count",
    ] {
        let mut bad = f.manifest.clone();
        bad[field] = if field == "source_commit" || field == "source_tree" {
            json!("0".repeat(40))
        } else {
            json!(true)
        };
        if field == "source_git_unchanged" {
            bad[field] = json!(1);
        }
        assert!(verify_manifest(&bad).is_err());
    }
    for variant in [
        "schema_boolean",
        "clean_number",
        "exit_boolean",
        "rehashed_critical_source",
        "helper_hash",
    ] {
        let mut bad_p = p.clone();
        match variant {
            "schema_boolean" => bad_p["schema"] = json!(true),
            "clean_number" => bad_p["source"]["compiled_source_clean"] = json!(1),
            "exit_boolean" => bad_p["build"]["exit_code"] = json!(false),
            "rehashed_critical_source" => {
                bad_p["source"]["files_sha256"]["crates/core/src/storage.rs"] =
                    json!("0".repeat(64))
            }
            "helper_hash" => bad_p["builder_sha256"] = json!("0".repeat(64)),
            _ => unreachable!(),
        }
        let rehashed = serde_json::to_vec(&bad_p).unwrap();
        let mut bad_m = f.manifest.clone();
        bad_m["tool_provenance_sha256"] = json!(sha(&rehashed));
        assert_eq!(bad_m["tool_provenance_sha256"], sha(&rehashed));
        assert!(verify_provenance(&bad_m, &bad_p, &rehashed).is_err());
    }
    f.original_unchanged();
}

#[test]
fn genuine16_rehashed_database_tamper_cannot_replace_actual_rpc_task_receipts() {
    let f = fixture();
    let db = Connection::open(&f.db).unwrap();
    let id = f.manifest["tasks"][0]["id"].as_str().unwrap();
    db.execute("UPDATE tasks SET document=json_set(document,'$.code_workspace_binding.receipt.request_digest',?1) WHERE id=?2",params!["0".repeat(64),id]).unwrap();
    drop(db);
    let raw = private_read(&f.db, 16_777_216);
    let mut rehashed = f.manifest.clone();
    rehashed["database_sha256"] = json!(sha(&raw));
    rehashed["database_bytes"] = json!(raw.len());
    assert_eq!(rehashed["database_sha256"], sha(&raw));
    assert!(
        verify_database(&rehashed, &ro(&f.db)).is_err(),
        "New checksum must not bless changed binding/immutable receipt"
    );
    assert!(backups(f.root()).is_empty());
    f.original_unchanged();
}

#[test]
fn genuine16_whitespace_colliding_declared_type_cannot_remove_not_null_before17() {
    let f = fixture();
    let keeper = Connection::open(&f.db).unwrap();
    keeper
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    let before = image(&keeper);
    assert_eq!(before.version, 16);
    let original_sql: String = keeper
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name='tasks'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let original_column: (String, i64) = keeper
        .query_row(
            "SELECT type,\"notnull\" FROM pragma_table_info('tasks') WHERE name='document'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(original_column, ("TEXT".to_string(), 1i64));
    assert_eq!(original_sql.matches("document TEXT NOT NULL").count(), 1);
    let weak_sql = original_sql.replacen("document TEXT NOT NULL", "document TEXTNOTNULL", 1);
    // The old whitespace-deleting comparator equated these byte-distinct DDLs.
    // SQLite instead parses TEXTNOTNULL as one legal declared type token.
    let collapsed = |sql: &str| {
        sql.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
    };
    assert_eq!(collapsed(&original_sql), collapsed(&weak_sql));
    assert_ne!(original_sql, weak_sql);
    let editor = Connection::open(&f.db).unwrap();
    editor.pragma_update(None, "writable_schema", true).unwrap();
    assert!(editor
        .pragma_query_value(None, "writable_schema", |r| r.get::<_, bool>(0))
        .unwrap());
    assert_eq!(
        editor
            .execute(
                "UPDATE sqlite_schema SET sql=?1 WHERE type='table' AND name='tasks'",
                [&weak_sql]
            )
            .unwrap(),
        1
    );
    editor
        .pragma_update(None, "writable_schema", false)
        .unwrap();
    assert!(!editor
        .pragma_query_value(None, "writable_schema", |r| r.get::<_, bool>(0))
        .unwrap());
    drop(editor);
    // A fresh connection must parse the actual edited sqlite_schema. The separate
    // original owner connection remains alive solely to preserve committed WAL for comparison.
    let observer = ro(&f.db);
    let weak_column: (String, i64) = observer
        .query_row(
            "SELECT type,\"notnull\" FROM pragma_table_info('tasks') WHERE name='document'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(weak_column, ("TEXTNOTNULL".to_string(), 0i64));
    clean_fk(&observer);
    let mutated = image(&observer);
    let mut expected_schema = before.schema.clone();
    let tasks = expected_schema
        .iter_mut()
        .find(|r| r.0 == "table" && r.1 == "tasks")
        .unwrap();
    assert_eq!(tasks.3.as_ref(), Some(&original_sql));
    tasks.3 = Some(weak_sql);
    equal(
        &expected_schema,
        &mutated.schema,
        &f.db,
        "Only the owned Task DDL text changed in the negative copy",
    );
    equal(
        &before.rows,
        &mutated.rows,
        &f.db,
        "All55 original raw tables/counters/receipts remain exact before rejection",
    );
    assert_eq!(mutated.version, before.version);
    assert_eq!(mutated.schema_version, before.schema_version);
    assert_eq!(mutated.application_id, before.application_id);
    assert_eq!(mutated.journal, before.journal);
    let wal = f.db.with_file_name("actual.sqlite3-wal");
    let wal_raw = fs::read(&wal).unwrap();
    let main_raw = fs::read(&f.db).unwrap();
    assert!(wal_raw.len() > 32);
    // Do not rehash or certify the weaker copy as a new producer fixture.
    reject_unchanged(&f, Some("INPUT_CHANGED"));
    equal(
        &main_raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Refusal changed main file bytes",
    );
    equal(
        &wal_raw,
        &fs::read(&wal).unwrap(),
        &f.db,
        "Refusal changed committed weak-DDL WAL frames",
    );
    equal(
        &mutated,
        &image(&observer),
        &f.db,
        "Refusal changed the weak predecessor image",
    );
    f.original_unchanged();
}

#[test]
fn genuine16_duplicate_known_task_field_in_original_json_text_is_not_collapsed_for17() {
    let f = fixture();
    let keeper = Connection::open(&f.db).unwrap();
    keeper
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;")
        .unwrap();
    let before = image(&keeper);
    let id = f.manifest["tasks"][2]["id"].as_str().unwrap();
    let raw: String = keeper
        .query_row("SELECT document FROM tasks WHERE id=?1", [id], |r| r.get(0))
        .unwrap();
    assert_eq!(raw.matches("\"id\":").count(), 1);
    let original: Task = serde_json::from_str(&raw).unwrap();
    assert_eq!(original.id, id);
    assert!(original.code_workspace_binding.is_none());
    // Insert directly into the original raw text. Only the ID string literal
    // is encoded; the document is never Value-normalized or reserialized.
    let id_literal = serde_json::to_string(id).unwrap();
    let duplicate = format!("{{\"id\":{id_literal},{}", raw.strip_prefix('{').unwrap());
    assert_eq!(duplicate.matches("\"id\":").count(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(&duplicate).unwrap(),
        serde_json::from_str::<Value>(&raw).unwrap()
    );
    assert!(
        serde_json::from_str::<Task>(&duplicate).is_err(),
        "Typed Task decoding must reject a duplicate known field"
    );
    assert_eq!(
        keeper
            .execute(
                "UPDATE tasks SET document=?1 WHERE id=?2",
                params![duplicate, id]
            )
            .unwrap(),
        1
    );
    let mutated = image(&keeper);
    let mut expected_rows = before.rows.clone();
    let row = expected_rows
        .get_mut("tasks")
        .unwrap()
        .iter_mut()
        .find(|row| row.get(1) == Some(&SqlValue::Text(id.into())))
        .unwrap();
    assert_eq!(row.get(2), Some(&SqlValue::Text(raw)));
    row[2] = SqlValue::Text(duplicate);
    equal(
        &expected_rows,
        &mutated.rows,
        &f.db,
        "Only one raw Task document cell changed; other55-table history stays exact",
    );
    equal(
        &before.schema,
        &mutated.schema,
        &f.db,
        "Duplicate JSON negative changed no DDL",
    );
    assert_eq!(mutated.version, before.version);
    assert_eq!(mutated.schema_version, before.schema_version);
    assert_eq!(mutated.application_id, before.application_id);
    assert_eq!(mutated.journal, before.journal);
    clean_fk(&keeper);
    let wal = f.db.with_file_name("actual.sqlite3-wal");
    let wal_raw = fs::read(&wal).unwrap();
    let main_raw = fs::read(&f.db).unwrap();
    assert!(wal_raw.len() > 32);
    // The authentic producer manifest/input remains untouched. This is solely
    // a disposable malformed genuine16 copy, never a synthetic success input.
    reject_unchanged(&f, Some("INPUT_CHANGED"));
    equal(
        &main_raw,
        &fs::read(&f.db).unwrap(),
        &f.db,
        "Duplicate refusal changed main bytes",
    );
    equal(
        &wal_raw,
        &fs::read(&wal).unwrap(),
        &f.db,
        "Duplicate refusal changed committed WAL",
    );
    equal(
        &mutated,
        &image(&keeper),
        &f.db,
        "Duplicate refusal changed old image/version",
    );
    f.original_unchanged();
}
