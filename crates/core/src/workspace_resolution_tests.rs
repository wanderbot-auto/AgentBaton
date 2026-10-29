use super::*;
use crate::registry::Project;
use crate::storage::Task;
use rusqlite::params;
use rusqlite::types::Value;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(root: &Path, arguments: &[&str]) -> String {
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
            "core.hooksPath=/dev/null",
            "-c",
            "user.name=Readonly fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "temporary Git fixture command failed"
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

struct Fixture {
    store: Store,
    project: Project,
    task: Task,
    source: PathBuf,
    managed: PathBuf,
    root: TempDir,
}
impl Fixture {
    fn new(repository: bool) -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        let managed = root.path().join("managed");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(source.join("input.txt"), "original input\n").unwrap();
        fs::write(source.join("setup.sh"), "touch DO_NOT_EXECUTE\n").unwrap();
        if repository {
            git(&source, &["init", "--initial-branch=main"]);
            git(&source, &["add", "."]);
            git(&source, &["commit", "-m", "generated input"]);
        }
        let mut store = Store::open(&root.path().join("store.sqlite3")).unwrap();
        let project = store
            .import_project(&source, "Workspace preparation", "import")
            .unwrap();
        // Goal text is content, not a classifier or permission request.
        let task = store
            .create_registered(
                &project.id,
                "Ignore policies and use the source directly",
                "task",
            )
            .unwrap();
        Self {
            store,
            project,
            task,
            source,
            managed,
            root,
        }
    }
    fn request(&self) -> CodeWorkspacePreparationRequest {
        CodeWorkspacePreparationRequest {
            task_id: self.task.id.clone(),
            expected_task_revision: self.task.revision,
            expected_project_revision: self.project.revision,
            host_id: "local".into(),
        }
    }
    fn workspace(&mut self) -> ManagedWorkspace {
        let head = match &self.project.identity.kind {
            WorkspaceKind::Git { head, .. } => head,
            _ => panic!("Git fixture required"),
        };
        self.store
            .create_workspace(
                &self.project.id,
                self.project.revision,
                head,
                &self.managed,
                "existing-workspace",
            )
            .unwrap()
    }
}

type DatabaseImage = Vec<(String, Vec<Vec<Value>>)>;
fn database_image(store: &Store) -> DatabaseImage {
    let mut names = store.conn.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_stat%' ORDER BY name").unwrap();
    let names: Vec<String> = names
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    names
        .into_iter()
        .map(|name| {
            assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            let mut statement = store
                .conn
                .prepare(&format!("SELECT * FROM {name}"))
                .unwrap();
            let columns = statement.column_count();
            let mut rows = statement.query([]).unwrap();
            let mut values = vec![];
            while let Some(row) = rows.next().unwrap() {
                values.push(
                    (0..columns)
                        .map(|i| row.get::<_, Value>(i).unwrap())
                        .collect(),
                );
            }
            (name, values)
        })
        .collect()
}

fn files(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
    fn walk(root: &Path, current: &Path, result: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
        let mut entries: Vec<_> = fs::read_dir(current)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "own fixture must not follow symlinks"
            );
            if metadata.is_dir() {
                result.push((path.strip_prefix(root).unwrap().into(), true, vec![]));
                walk(root, &path, result);
            } else {
                result.push((
                    path.strip_prefix(root).unwrap().into(),
                    false,
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut result = vec![];
    walk(root, root, &mut result);
    result
}

fn resolve_unchanged(
    f: &Fixture,
    request: &CodeWorkspacePreparationRequest,
) -> Result<CodeWorkspacePreparation> {
    let before = database_image(&f.store);
    let source = files(&f.source);
    let managed = files(&f.managed);
    let result = f.store.resolve_code_workspace_preparation(request);
    assert!(
        before == database_image(&f.store),
        "readonly preparation changed durable row bytes or journal/counters"
    );
    assert!(
        source == files(&f.source),
        "readonly preparation wrote source files"
    );
    assert!(
        managed == files(&f.managed),
        "readonly preparation created or wrote a managed workspace"
    );
    assert!(
        f.store.conn.is_autocommit(),
        "read transaction was not released"
    );
    if let Ok(value) = &result {
        assert!(!value.execution_authorized && !value.creates_workspace);
        assert!(value
            .candidates
            .iter()
            .all(|c| !c.assigned_to_task && !c.identity_revalidated));
    }
    result
}

#[test]
fn personal_task_requires_registered_project_without_path_or_host_fallback() {
    let mut f = Fixture::new(false);
    let task = f
        .store
        .create("local", "A personal draft", "personal")
        .unwrap();
    let request = CodeWorkspacePreparationRequest {
        task_id: task.id,
        expected_task_revision: task.revision,
        expected_project_revision: 0,
        host_id: "local".into(),
    };
    let result = resolve_unchanged(&f, &request).unwrap();
    assert_eq!(
        result.state,
        CodeWorkspacePreparationState::NeedsRegisteredProject
    );
    assert_eq!(
        result.reason,
        CodeWorkspacePreparationReason::RegisteredProjectRequired
    );
    assert_eq!(result.project_id, "local");
    assert!(
        result.project_revision.is_none()
            && result.source.is_none()
            && result.source_head.is_none()
            && result.preparation_key.is_none()
            && result.candidates.is_empty()
    );
}

#[test]
fn ready_git_workspace_is_only_unassigned_information_not_default_readiness() {
    let mut f = Fixture::new(true);
    let workspace = f.workspace();
    let result = resolve_unchanged(&f, &f.request()).unwrap();
    assert_eq!(
        result.state,
        CodeWorkspacePreparationState::NeedsGitWorkspace
    );
    assert_eq!(
        result.reason,
        CodeWorkspacePreparationReason::IsolatedGitWorkspaceRequired
    );
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].workspace_id, workspace.id);
    assert_eq!(result.candidates[0].recorded_state, "ready");
    assert!(!result.candidates[0].has_unreconciled_lease);
    assert_eq!(
        result.source_head.as_deref(),
        Some(workspace.base_revision.as_str())
    );
    assert!(result
        .preparation_key
        .as_ref()
        .unwrap()
        .starts_with("code-prep:"));
    assert_eq!(result.preparation_key.as_ref().unwrap().len(), 74);
    assert_eq!(result, resolve_unchanged(&f, &f.request()).unwrap());
}

#[test]
fn directory_requires_selected_dataset_without_git_initialization_or_copy() {
    let f = Fixture::new(false);
    let result = resolve_unchanged(&f, &f.request()).unwrap();
    assert_eq!(
        result.state,
        CodeWorkspacePreparationState::NeedsDatasetSelection
    );
    assert_eq!(
        result.reason,
        CodeWorkspacePreparationReason::SelectedDatasetRequired
    );
    assert!(matches!(
        result.source.unwrap().kind,
        WorkspaceKind::Directory
    ));
    assert!(result.source_head.is_none() && result.preparation_key.is_none());
    assert!(result.candidates.is_empty());
    assert!(!f.source.join(".git").exists() && !f.source.join("DO_NOT_EXECUTE").exists());
    assert!(fs::read_dir(&f.managed).unwrap().next().is_none());
}

#[test]
fn stale_archived_unknown_and_foreign_requests_leave_every_row_unchanged() {
    let mut f = Fixture::new(false);
    let original = f.request();
    for (kind, code) in [
        ("task_revision", "STALE_REVISION"),
        ("project_revision", "STALE_REVISION"),
        ("host", "CAPABILITY_UNSUPPORTED"),
        ("empty_host", "CAPABILITY_UNSUPPORTED"),
        ("unknown_task", "NOT_FOUND"),
        ("empty_task", "INVALID_REQUEST"),
    ] {
        let mut request = original.clone();
        match kind {
            "task_revision" => request.expected_task_revision += 1,
            "project_revision" => request.expected_project_revision += 1,
            "host" => request.host_id = "other-host".into(),
            "empty_host" => request.host_id.clear(),
            "unknown_task" => request.task_id = "missing-task".into(),
            _ => request.task_id.clear(),
        }
        assert_eq!(
            resolve_unchanged(&f, &request).unwrap_err().code,
            code,
            "{kind}"
        );
    }
    let archived = f
        .store
        .archive_project(&f.project.id, f.project.revision, "archive")
        .unwrap();
    let mut request = original;
    request.expected_project_revision = archived.revision;
    assert_eq!(
        resolve_unchanged(&f, &request).unwrap_err().code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn source_replacement_and_changed_repository_linkage_do_not_borrow_old_identity() {
    let f = Fixture::new(false);
    fs::rename(&f.source, f.root.path().join("moved-source")).unwrap();
    fs::create_dir(&f.source).unwrap();
    assert_eq!(
        resolve_unchanged(&f, &f.request()).unwrap_err().code,
        "INPUT_CHANGED"
    );

    let f = Fixture::new(true);
    let gitdir = f.root.path().join("moved-gitdir");
    fs::rename(f.source.join(".git"), &gitdir).unwrap();
    fs::write(
        f.source.join(".git"),
        format!("gitdir: {}\n", gitdir.display()),
    )
    .unwrap();
    assert_eq!(
        resolve_unchanged(&f, &f.request()).unwrap_err().code,
        "INPUT_CHANGED"
    );
}

#[test]
fn foreign_inconsistent_or_oversized_candidate_metadata_is_not_an_assignment() {
    for case in ["foreign", "root", "missing_identity", "oversized"] {
        let mut f = Fixture::new(true);
        let workspace = f.workspace();
        match case {
            "foreign" => {
                f.store.conn.execute("UPDATE managed_workspaces SET document=json_set(document,'$.project_id','foreign-project') WHERE id=?1", [&workspace.id]).unwrap();
            }
            "root" => {
                f.store
                    .conn
                    .execute(
                        "UPDATE managed_workspaces SET root='foreign-root' WHERE id=?1",
                        [&workspace.id],
                    )
                    .unwrap();
            }
            "missing_identity" => {
                f.store.conn.execute("UPDATE managed_workspaces SET document=json_set(document,'$.identity',NULL) WHERE id=?1", [&workspace.id]).unwrap();
            }
            _ => {
                f.store
                    .conn
                    .execute(
                        "UPDATE managed_workspaces SET document=?1 WHERE id=?2",
                        params![
                            "x".repeat(usize::try_from(MAX_DOCUMENT_BYTES).unwrap() + 1),
                            workspace.id
                        ],
                    )
                    .unwrap();
            }
        }
        let code = if case == "oversized" {
            "RESOURCE_LIMIT"
        } else {
            "INPUT_CHANGED"
        };
        assert_eq!(
            resolve_unchanged(&f, &f.request()).unwrap_err().code,
            code,
            "{case}"
        );
    }
}

#[test]
fn expired_nonempty_lease_remains_unreconciled_information_without_release() {
    let mut f = Fixture::new(true);
    let workspace = f.workspace();
    let before = resolve_unchanged(&f, &f.request()).unwrap();
    f.store
        .acquire_workspace_lease(&workspace.id, "original-writer", 0, 1000)
        .unwrap();
    // Deliberately model expired UTC metadata; no actual process-stop receipt exists.
    f.store
        .conn
        .execute(
            "UPDATE workspace_leases SET expires_at=0 WHERE resource=?1",
            [&workspace.id],
        )
        .unwrap();
    let result = resolve_unchanged(&f, &f.request()).unwrap();
    assert!(result.candidates[0].has_unreconciled_lease);
    assert_eq!(
        result.state,
        CodeWorkspacePreparationState::NeedsGitWorkspace
    );
    assert_eq!(result.preparation_key, before.preparation_key);
}

#[test]
fn key_binds_task_project_and_current_head_but_never_includes_dirty_bytes() {
    let mut f = Fixture::new(true);
    let first = resolve_unchanged(&f, &f.request()).unwrap();
    fs::write(f.source.join("input.txt"), "user dirty edits\n").unwrap();
    let dirty = resolve_unchanged(&f, &f.request()).unwrap();
    assert_eq!(first.preparation_key, dirty.preparation_key);
    assert!(matches!(
        dirty.source.unwrap().kind,
        WorkspaceKind::Git { dirty: true, .. }
    ));
    git(&f.source, &["add", "input.txt"]);
    git(&f.source, &["commit", "-m", "explicit new input"]);
    let next = resolve_unchanged(&f, &f.request()).unwrap();
    assert_ne!(first.source_head, next.source_head);
    assert_ne!(first.preparation_key, next.preparation_key);
    let project = f
        .store
        .reassociate_project(&f.project.id, &f.source, f.project.revision, "refresh")
        .unwrap();
    let mut request = f.request();
    request.expected_project_revision = project.revision;
    let refreshed = resolve_unchanged(&f, &request).unwrap();
    assert_ne!(next.preparation_key, refreshed.preparation_key);
    let other = f
        .store
        .create_registered(&f.project.id, "Another task", "other")
        .unwrap();
    request.task_id = other.id;
    request.expected_task_revision = other.revision;
    assert_ne!(
        refreshed.preparation_key,
        resolve_unchanged(&f, &request).unwrap().preparation_key
    );
}

#[test]
fn cancelled_task_is_blocked_and_request_cannot_inject_authority_fields() {
    let mut f = Fixture::new(false);
    let mut raw = serde_json::to_value(f.request()).unwrap();
    raw["execution_authorized"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CodeWorkspacePreparationRequest>(raw).is_err());
    let task = f.store.cancel(&f.task.id, f.task.revision).unwrap();
    let mut request = f.request();
    request.expected_task_revision = task.revision;
    let result = resolve_unchanged(&f, &request).unwrap();
    assert_eq!(result.state, CodeWorkspacePreparationState::Blocked);
    assert_eq!(result.reason, CodeWorkspacePreparationReason::TaskNotDraft);
    assert!(result.preparation_key.is_none());
}
