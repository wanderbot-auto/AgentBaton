use super::*;
use crate::CodeWorkspacePreparationRequest;
use rusqlite::types::Value as SqlValue;
use std::{
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    process::Command,
};
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Generated fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(output.status.success(), "own temporary Git fixture failed");
    String::from_utf8(output.stdout).unwrap().trim().into()
}
struct Fixture {
    store: Store,
    task: Task,
    project: Project,
    source: PathBuf,
    managed: PathBuf,
    path: PathBuf,
    root: TempDir,
}
impl Fixture {
    fn new(repository: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let managed = root.path().join("managed");
        let path = root.path().join("store.sqlite3");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(source.join("input"), "baseline\n").unwrap();
        fs::create_dir(source.join("empty")).unwrap();
        if repository {
            git(&source, &["init", "--initial-branch=main"]);
            git(&source, &["add", "."]);
            git(&source, &["commit", "-m", "baseline"]);
        }
        let mut store = Store::open(&path).unwrap();
        let project = store
            .import_project(&source, "Generated project", "import")
            .unwrap();
        let task = store
            .create_registered(&project.id, "Code with isolated writes", "task")
            .unwrap();
        Self {
            store,
            task,
            project,
            source,
            managed,
            path,
            root,
        }
    }
    fn request_for(&self, task: &Task) -> TaskGitWorkspacePreparation {
        let prep = self
            .store
            .resolve_code_workspace_preparation(&CodeWorkspacePreparationRequest {
                task_id: task.id.clone(),
                expected_task_revision: task.revision,
                expected_project_revision: self.project.revision,
                host_id: "local".into(),
            })
            .unwrap();
        TaskGitWorkspacePreparation {
            task_id: task.id.clone(),
            expected_task_revision: task.revision,
            expected_spec_revision: task.spec_revision,
            expected_project_revision: self.project.revision,
            host_id: "local".into(),
            expected_source_pin: GitWorkspacePin::observed(prep.source.unwrap()).unwrap(),
            preparation_key: prep.preparation_key.unwrap(),
        }
    }
    fn request(&self) -> TaskGitWorkspacePreparation {
        self.request_for(&self.task)
    }
    fn reserve(
        &mut self,
        request: &TaskGitWorkspacePreparation,
    ) -> TaskWorkspaceReservationReceipt {
        match self
            .store
            .reserve_task_git_workspace(request, &self.managed, "prepare")
            .unwrap()
        {
            WorkspaceReservation::Pending(receipt) => *receipt,
            WorkspaceReservation::Existing(_) => panic!("fresh fixture already bound"),
        }
    }
    fn checkout(&self, receipt: &TaskWorkspaceReservationReceipt) -> WorkspaceIdentity {
        self.store.checkout_task_workspace(receipt).unwrap()
    }
}
type Image = Vec<(String, Vec<Vec<SqlValue>>)>;
fn image(db: &Connection) -> Image {
    let mut stmt=db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_stat%' ORDER BY name").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            assert!(name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_'));
            let mut stmt = db.prepare(&format!("SELECT * FROM {name}")).unwrap();
            let n = stmt.column_count();
            let rows = stmt
                .query_map([], |r| {
                    (0..n)
                        .map(|i| r.get::<_, SqlValue>(i))
                        .collect::<std::result::Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap();
            (name, rows)
        })
        .collect()
}
fn files(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
        let mut paths: Vec<_> = fs::read_dir(current)
            .unwrap()
            .map(|v| v.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            let m = fs::symlink_metadata(&path).unwrap();
            assert!(!m.file_type().is_symlink());
            let rel = path.strip_prefix(root).unwrap().into();
            if m.is_dir() {
                out.push((rel, true, vec![]));
                walk(root, &path, out);
            } else {
                out.push((rel, false, fs::read(path).unwrap()));
            }
        }
    }
    let mut out = vec![];
    walk(root, root, &mut out);
    out
}
fn unchanged_error(
    f: &mut Fixture,
    request: &TaskGitWorkspacePreparation,
    key: &str,
    expected: &str,
) {
    let before = image(&f.store.conn);
    let source = files(&f.source);
    let managed = files(&f.managed);
    let e = f
        .store
        .prepare_task_git_workspace(request, &f.managed, key)
        .unwrap_err();
    assert_eq!(e.code, expected);
    assert!(
        before == image(&f.store.conn),
        "refusal changed durable rows/sequence"
    );
    assert!(source == files(&f.source), "refusal changed source tree");
    assert!(
        managed == files(&f.managed),
        "refusal changed managed content"
    );
}
fn assert_no_execution(db: &Connection) {
    for table in [
        "execution_grants",
        "budgets",
        "attempts",
        "dispatch_intents",
        "sessions",
        "workspace_leases",
        "factory_runs",
    ] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "preparation minted execution state");
    }
}

#[test]
fn legacy_none_is_omitted_and_persisted_filesystem_integers_are_lossless_strings() {
    let f = Fixture::new(false);
    let raw: String = f
        .store
        .conn
        .query_row(
            "SELECT document FROM tasks WHERE id=?1",
            [&f.task.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!serde_json::from_str::<serde_json::Value>(&raw)
        .unwrap()
        .as_object()
        .unwrap()
        .contains_key("code_workspace_binding"));
    assert_eq!(encode(&f.task).unwrap(), raw);
    let pin = GitWorkspacePin {
        canonical_root: "/source".into(),
        device: 9_007_199_254_740_993,
        inode: u64::MAX,
        common_dir: "/source/.git".into(),
        head: "a".repeat(40),
    };
    let value = serde_json::to_value(&pin).unwrap();
    assert_eq!(value["device"], "9007199254740993");
    assert_eq!(value["inode"], "18446744073709551615");
    assert_eq!(
        serde_json::from_value::<GitWorkspacePin>(value.clone()).unwrap(),
        pin
    );
    for bad in [
        json!(1),
        json!(1.0),
        json!(true),
        json!("01"),
        json!("1\n"),
        json!("18446744073709551616"),
    ] {
        let mut altered = value.clone();
        altered["inode"] = bad;
        assert!(serde_json::from_value::<GitWorkspacePin>(altered).is_err());
    }
    let mut zero = pin;
    zero.device = 0;
    assert_eq!(serde_json::to_value(zero).unwrap()["device"], "0");
}

#[test]
fn durable_default_binding_is_same_key_only_and_two_tasks_get_distinct_worktrees() {
    let mut f = Fixture::new(true);
    fs::write(f.source.join("input"), "user dirty input\n").unwrap();
    fs::write(f.source.join("untracked"), "keep\n").unwrap();
    let head = git(&f.source, &["rev-parse", "HEAD"]);
    let index = fs::read(f.source.join(".git/index")).unwrap();
    let request = f.request();
    let first = f
        .store
        .prepare_task_git_workspace(&request, &f.managed, "prepare")
        .unwrap();
    assert_eq!(first.binding.state, TaskWorkspaceBindingState::Ready);
    assert!(!first.execution_authorized);
    assert!(first.identity_revalidated);
    assert_eq!(first.original_receipt.original_task_revision, 1);
    assert_eq!(first.original_receipt.reserved_task_revision, 2);
    assert_eq!(first.task_revision, 3);
    let before = image(&f.store.conn);
    let again = f
        .store
        .prepare_task_git_workspace(&request, &f.managed, "prepare")
        .unwrap();
    assert_eq!(again, first);
    assert!(before == image(&f.store.conn));
    unchanged_error(&mut f, &request, "new-key", "CONFIG_CONFLICT");
    let mut altered = request.clone();
    altered.expected_spec_revision += 1;
    unchanged_error(&mut f, &altered, "prepare", "CONFIG_CONFLICT");
    let second_task = f
        .store
        .create_registered(&f.project.id, "Independent task", "task-2")
        .unwrap();
    let second_request = f.request_for(&second_task);
    let second = f
        .store
        .prepare_task_git_workspace(&second_request, &f.managed, "prepare")
        .unwrap();
    assert_ne!(
        first.binding.receipt.workspace_id,
        second.binding.receipt.workspace_id
    );
    let first_path = f.managed.join(&first.original_receipt.workspace_id);
    let second_path = f.managed.join(&second.original_receipt.workspace_id);
    fs::write(first_path.join("input"), "only first\n").unwrap();
    assert_eq!(
        fs::read_to_string(second_path.join("input")).unwrap(),
        "baseline\n"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("input")).unwrap(),
        "user dirty input\n"
    );
    assert_eq!(
        fs::read_to_string(f.source.join("untracked")).unwrap(),
        "keep\n"
    );
    assert_eq!(git(&f.source, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.source.join(".git/index")).unwrap(), index);
    assert_no_execution(&f.store.conn);
    let receipt = first.original_receipt.clone();
    drop(f.store);
    f.store = Store::open(&f.path).unwrap();
    assert_eq!(
        f.store
            .prepare_task_git_workspace(&request, &f.managed, "prepare")
            .unwrap()
            .original_receipt,
        receipt
    );
}

#[test]
fn stale_foreign_source_and_directory_requests_have_no_reservation_side_effects() {
    for case in [
        "task_revision",
        "spec_revision",
        "project_revision",
        "host",
        "source_inode",
        "source_root",
        "preparation_key",
        "archived",
        "project_column",
    ] {
        let mut f = Fixture::new(true);
        let mut r = f.request();
        let expected = match case {
            "task_revision" => {
                r.expected_task_revision += 1;
                "STALE_REVISION"
            }
            "spec_revision" => {
                r.expected_spec_revision += 1;
                "STALE_REVISION"
            }
            "project_revision" => {
                r.expected_project_revision += 1;
                "STALE_REVISION"
            }
            "host" => {
                r.host_id = "remote".into();
                "INVALID_REQUEST"
            }
            "source_inode" => {
                r.expected_source_pin.inode ^= 1;
                "INPUT_CHANGED"
            }
            "source_root" => {
                r.expected_source_pin.canonical_root = f.managed.clone();
                "INPUT_CHANGED"
            }
            "preparation_key" => {
                r.preparation_key = "code-prep:".to_owned() + &"a".repeat(64);
                "INPUT_CHANGED"
            }
            "archived" => {
                f.project = f
                    .store
                    .archive_project(&f.project.id, f.project.revision, "archive")
                    .unwrap();
                r.expected_project_revision = f.project.revision;
                "PERMISSION_DENIED"
            }
            _ => {
                f.store
                    .conn
                    .execute(
                        "UPDATE projects SET inode=?1 WHERE id=?2",
                        params![
                            if f.project.identity.inode == 0 {
                                "1"
                            } else {
                                "0"
                            },
                            f.project.id
                        ],
                    )
                    .unwrap();
                "INPUT_CHANGED"
            }
        };
        unchanged_error(&mut f, &r, "prepare", expected);
        assert_no_execution(&f.store.conn);
    }
    let mut f = Fixture::new(false);
    let r = TaskGitWorkspacePreparation {
        task_id: f.task.id.clone(),
        expected_task_revision: f.task.revision,
        expected_spec_revision: f.task.spec_revision,
        expected_project_revision: f.project.revision,
        host_id: "local".into(),
        expected_source_pin: GitWorkspacePin {
            canonical_root: f.source.clone(),
            device: f.project.identity.device,
            inode: f.project.identity.inode,
            common_dir: f.source.join(".git"),
            head: "a".repeat(40),
        },
        preparation_key: "code-prep:".to_owned() + &"b".repeat(64),
    };
    unchanged_error(&mut f, &r, "prepare", "CAPABILITY_UNSUPPORTED");
    assert!(!f.source.join(".git").exists());
}

#[test]
fn startup_pending_and_failed_checkout_become_unknown_without_filesystem_replay() {
    let mut f = Fixture::new(true);
    let r = f.request();
    let receipt = f.reserve(&r);
    let path = f.managed.join(&receipt.workspace_id);
    assert!(!path.exists());
    let result: String = f
        .store
        .conn
        .query_row(
            "SELECT result FROM idempotency WHERE scope=?1 AND key='prepare'",
            [scope(&f.task.id)],
            |row| row.get(0),
        )
        .unwrap();
    drop(f.store);
    f.store = Store::open(&f.path).unwrap();
    let binding = f
        .store
        .get(&f.task.id)
        .unwrap()
        .code_workspace_binding
        .unwrap();
    assert_eq!(binding.state, TaskWorkspaceBindingState::OutcomeUnknown);
    assert_eq!(binding.receipt, receipt);
    assert!(!path.exists());
    unchanged_error(&mut f, &r, "prepare", "OUTCOME_UNKNOWN");
    unchanged_error(&mut f, &r, "another-key", "CONFIG_CONFLICT");
    assert_eq!(
        f.store
            .conn
            .query_row(
                "SELECT result FROM idempotency WHERE scope=?1 AND key='prepare'",
                [scope(&f.task.id)],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
        result
    );
    let before = image(&f.store.conn);
    drop(f.store);
    f.store = Store::open(&f.path).unwrap();
    assert!(before == image(&f.store.conn));
    assert!(!path.exists());
    let mut collision = Fixture::new(true);
    let r = collision.request();
    let receipt = collision.reserve(&r);
    let path = collision.managed.join(&receipt.workspace_id);
    fs::create_dir(&path).unwrap();
    fs::write(path.join("existing"), "do not overwrite").unwrap();
    let failed = agentbaton_workspaces::create_isolated(
        &receipt.source_pin.canonical_root,
        &receipt.managed_root,
        &receipt.workspace_id,
        &receipt.source_pin.head,
    );
    assert!(failed.is_err());
    assert_eq!(
        collision
            .store
            .finish_task_workspace(&receipt, None)
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    unchanged_error(&mut collision, &r, "prepare", "OUTCOME_UNKNOWN");
    assert_eq!(
        fs::read_to_string(path.join("existing")).unwrap(),
        "do not overwrite"
    );
}

#[test]
fn late_event_failure_rolls_back_both_ready_records_and_preserves_unknown_on_restart() {
    let mut f = Fixture::new(true);
    let r = f.request();
    let receipt = f.reserve(&r);
    let identity = f.checkout(&receipt);
    f.store.conn.execute_batch("CREATE TRIGGER fail_binding_finish BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='task.workspace_preparation_finished' BEGIN SELECT RAISE(ABORT,'controlled failure'); END;").unwrap();
    let before = image(&f.store.conn);
    let error = f
        .store
        .finish_task_workspace(&receipt, Some(identity))
        .unwrap_err();
    assert_eq!(error.details_ref.as_deref(), Some("sqlite-code:1811"));
    assert!(before == image(&f.store.conn));
    assert!(f.managed.join(&receipt.workspace_id).exists());
    f.store
        .conn
        .execute_batch("DROP TRIGGER fail_binding_finish;")
        .unwrap();
    drop(f.store);
    f.store = Store::open(&f.path).unwrap();
    assert_eq!(
        f.store
            .get(&f.task.id)
            .unwrap()
            .code_workspace_binding
            .unwrap()
            .state,
        TaskWorkspaceBindingState::OutcomeUnknown
    );
    unchanged_error(&mut f, &r, "prepare", "OUTCOME_UNKNOWN");
}

#[test]
fn task_cancellation_spec_and_source_changes_before_finalize_never_become_ready() {
    for case in ["cancel", "spec", "source"] {
        let mut f = Fixture::new(true);
        let request = f.request();
        let receipt = f.reserve(&request);
        let identity = f.checkout(&receipt);
        match case {
            "cancel" => {
                f.store
                    .cancel(&f.task.id, receipt.reserved_task_revision)
                    .unwrap();
            }
            "spec" => {
                let mut task = f.store.get(&f.task.id).unwrap();
                task.spec_revision += 1;
                task.revision += 1;
                task.goal = "Preserve the new specification".into();
                f.store
                    .conn
                    .execute(
                        "UPDATE tasks SET document=?1 WHERE id=?2",
                        params![encode(&task).unwrap(), task.id],
                    )
                    .unwrap();
            }
            _ => {
                fs::write(f.source.join("input"), "source moved forward\n").unwrap();
                git(&f.source, &["add", "input"]);
                git(&f.source, &["commit", "-m", "source change"]);
            }
        }
        assert_eq!(
            f.store
                .finish_task_workspace(&receipt, Some(identity))
                .unwrap_err()
                .code,
            "OUTCOME_UNKNOWN"
        );
        let task = f.store.get(&f.task.id).unwrap();
        assert_eq!(
            task.code_workspace_binding.unwrap().state,
            TaskWorkspaceBindingState::OutcomeUnknown
        );
        if case == "cancel" {
            assert_eq!(task.state, TaskState::Cancelled);
        }
        if case == "spec" {
            assert_eq!(task.goal, "Preserve the new specification");
            assert_eq!(task.spec_revision, 2);
        }
        assert_no_execution(&f.store.conn);
    }
}

#[test]
fn replay_and_new_admission_revalidate_physical_pins_without_locking_legal_output_head() {
    let mut f = Fixture::new(true);
    let r = f.request();
    let view = f
        .store
        .prepare_task_git_workspace(&r, &f.managed, "prepare")
        .unwrap();
    let root = f.managed.join(&view.original_receipt.workspace_id);
    let receipt = view.original_receipt.clone();
    fs::write(root.join("result"), "legal output\n").unwrap();
    git(&root, &["add", "result"]);
    git(&root, &["commit", "-m", "isolated result"]);
    let live_head = git(&root, &["rev-parse", "HEAD"]);
    assert_ne!(live_head, receipt.source_pin.head);
    let task = f.store.get(&f.task.id).unwrap();
    require_task_binding_for_session(&f.store.conn, &task, &task.id, &receipt.workspace_id)
        .unwrap();
    let fresh = f
        .store
        .prepare_task_git_workspace(&r, &f.managed, "prepare")
        .unwrap();
    assert_eq!(fresh.observed_managed_pin.unwrap().head, live_head);
    assert_eq!(fresh.original_receipt, receipt);
    assert!(!fresh.execution_authorized);
    let error = f
        .store
        .create_session(
            &task.id,
            "foreign-workspace",
            "missing-connection",
            "missing-grant",
            task.revision,
            "wrong-workspace",
        )
        .unwrap_err();
    assert_eq!(error.code, "PERMISSION_DENIED");
    // A passed binding guard is still not a Grant. No Session/attempt is minted.
    assert!(f
        .store
        .create_session(
            &task.id,
            &receipt.workspace_id,
            "missing-connection",
            "missing-grant",
            task.revision,
            "no-grant"
        )
        .is_err());
    assert_no_execution(&f.store.conn);
    // Real Task transition advances revision; original reservation revision is historical.
    f.store.cancel(&task.id, task.revision).unwrap();
    assert!(f
        .store
        .prepare_task_git_workspace(&r, &f.managed, "prepare")
        .is_ok());
    f.store.conn.execute("INSERT INTO workspace_leases(resource,holder,fencing_token,expires_at) VALUES(?1,'unreconciled-generated',1,0)",[&receipt.workspace_id]).unwrap();
    unchanged_error(&mut f, &r, "prepare", "WORKSPACE_BUSY");
}

#[test]
fn replaced_workspace_and_cleared_binding_history_cannot_reassign_or_repeat_creation() {
    for case in ["replaced", "source_head", "cleared_binding"] {
        let mut f = Fixture::new(true);
        let r = f.request();
        let view = f
            .store
            .prepare_task_git_workspace(&r, &f.managed, "prepare")
            .unwrap();
        let (key, expected) = match case {
            "replaced" => {
                let root = f.managed.join(&view.original_receipt.workspace_id);
                fs::rename(&root, f.root.path().join("retained-old-workspace")).unwrap();
                fs::create_dir(&root).unwrap();
                ("prepare", "INPUT_CHANGED")
            }
            "source_head" => {
                fs::write(f.source.join("input"), "new source\n").unwrap();
                git(&f.source, &["add", "input"]);
                git(&f.source, &["commit", "-m", "source update"]);
                ("prepare", "INPUT_CHANGED")
            }
            _ => {
                let mut task = f.store.get(&f.task.id).unwrap();
                task.code_workspace_binding = None;
                f.store
                    .conn
                    .execute(
                        "UPDATE tasks SET document=?1 WHERE id=?2",
                        params![encode(&task).unwrap(), task.id],
                    )
                    .unwrap();
                ("new-key", "OUTCOME_UNKNOWN")
            }
        };
        unchanged_error(&mut f, &r, key, expected);
    }
}

#[test]
fn controlled_v15_shape_migration_retains_rows_backup_and_refuses_future17() {
    // A local shape control, NOT a genuine old15-binary fixture; Root supplies that separately.
    let f = Fixture::new(false);
    let before = image(&f.store.conn);
    f.store
        .conn
        .pragma_update(None, "user_version", 15u32)
        .unwrap();
    let path = f.path.clone();
    let parent = f.root.path().to_path_buf();
    drop(f.store);
    let store = Store::open(&path).unwrap();
    assert_eq!(
        store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        16
    );
    assert!(before == image(&store.conn));
    let backups: Vec<_> = fs::read_dir(&parent)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .contains("pre-v16-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::metadata(&backups[0]).unwrap().mode() & 0o777, 0o600);
    let backup =
        Connection::open_with_flags(&backups[0], rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert_eq!(
        backup
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        15
    );
    assert!(before == image(&backup));
    drop(backup);
    store
        .conn
        .pragma_update(None, "user_version", 17u32)
        .unwrap();
    drop(store);
    let original = fs::read(&path).unwrap();
    let error = match Store::open(&path) {
        Ok(_) => panic!("future database accepted"),
        Err(e) => e,
    };
    assert_eq!(error.code, "VERSION_UNSUPPORTED");
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn version15_cannot_hide_a_new_semantic_field_even_when_null() {
    let f = Fixture::new(false);
    f.store.conn.execute("UPDATE tasks SET document=json_set(document,'$.code_workspace_binding',json('null')) WHERE id=?1",[&f.task.id]).unwrap();
    f.store
        .conn
        .pragma_update(None, "user_version", 15u32)
        .unwrap();
    let path = f.path.clone();
    let before = image(&f.store.conn);
    drop(f.store);
    let error = match Store::open(&path) {
        Ok(_) => panic!("hidden new semantic field accepted"),
        Err(e) => e,
    };
    assert_eq!(error.code, "INPUT_CHANGED");
    let db = Connection::open(&path).unwrap();
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap(),
        15
    );
    assert!(before == image(&db));
}

#[test]
fn reservation_event_failure_rolls_back_task_workspace_receipt_and_never_checks_out() {
    let mut f = Fixture::new(true);
    let request = f.request();
    f.store.conn.execute_batch("CREATE TRIGGER fail_binding_reserve BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='task.workspace_preparation_reserved' BEGIN SELECT RAISE(ABORT,'controlled reserve failure'); END;").unwrap();
    let before = image(&f.store.conn);
    let source = files(&f.source);
    let managed = files(&f.managed);
    let error = f
        .store
        .prepare_task_git_workspace(&request, &f.managed, "prepare")
        .unwrap_err();
    assert_eq!(error.details_ref.as_deref(), Some("sqlite-code:1811"));
    assert!(before == image(&f.store.conn));
    assert!(source == files(&f.source));
    assert!(managed == files(&f.managed));
    assert!(f
        .store
        .get(&f.task.id)
        .unwrap()
        .code_workspace_binding
        .is_none());
    assert_no_execution(&f.store.conn);
}

#[test]
fn recovery_rejects_a_copied_document_under_another_task_primary_key_without_cross_writes() {
    let mut f = Fixture::new(true);
    let request = f.request();
    let receipt = f.reserve(&request);
    let original = f.store.get(&f.task.id).unwrap();
    let other = f
        .store
        .create_registered(&f.project.id, "Different real Task", "other-task")
        .unwrap();
    // Deliberate raw-row corruption, not an application-produced capability.
    f.store
        .conn
        .execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&original).unwrap(), other.id],
        )
        .unwrap();
    let mut cleared = original;
    cleared.code_workspace_binding = None;
    f.store
        .conn
        .execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&cleared).unwrap(), cleared.id],
        )
        .unwrap();
    let before = image(&f.store.conn);
    let error = f.store.recover_workspace_creations().unwrap_err();
    assert_eq!(error.code, "INPUT_CHANGED");
    assert!(before == image(&f.store.conn));
    assert!(!f.managed.join(&receipt.workspace_id).exists());
    assert_no_execution(&f.store.conn);
}

#[test]
fn post_reservation_source_change_is_refused_before_the_filesystem_call() {
    let mut f = Fixture::new(true);
    let request = f.request();
    let receipt = f.reserve(&request);
    fs::write(f.source.join("input"), "next source revision\n").unwrap();
    git(&f.source, &["add", "input"]);
    git(&f.source, &["commit", "-m", "advance before checkout"]);
    let before = image(&f.store.conn);
    let error = f.store.checkout_task_workspace(&receipt).unwrap_err();
    assert_eq!(error.code, "INPUT_CHANGED");
    assert!(before == image(&f.store.conn));
    assert!(!f.managed.join(&receipt.workspace_id).exists());
    assert_eq!(
        f.store
            .finish_task_workspace(&receipt, None)
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    unchanged_error(&mut f, &request, "prepare", "OUTCOME_UNKNOWN");
}
