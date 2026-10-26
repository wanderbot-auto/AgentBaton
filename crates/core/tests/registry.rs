use agentbaton_core::storage::Store;
use rusqlite::Connection;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn private_managed_root() -> TempDir {
    // Only fresh test-owned execution roots are made private. Imported source
    // fixtures retain their ordinary directory permissions.
    let root = tempfile::Builder::new()
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    assert_eq!(
        fs::metadata(root.path()).unwrap().permissions().mode() & 0o777,
        0o700
    );
    root
}

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Baton Test")
        .env("GIT_AUTHOR_EMAIL", "test@invalid")
        .env("GIT_COMMITTER_NAME", "Baton Test")
        .env("GIT_COMMITTER_EMAIL", "test@invalid")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn repository() -> (TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    git(root.path(), &["init", "--initial-branch=main"]);
    fs::write(root.path().join("source.txt"), "original\n").unwrap();
    git(root.path(), &["add", "source.txt"]);
    git(root.path(), &["commit", "-m", "initial"]);
    let head = git(root.path(), &["rev-parse", "HEAD"]);
    (root, head)
}
fn store() -> (TempDir, PathBuf, Store) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("store.sqlite3");
    let store = Store::open(&path).unwrap();
    (dir, path, store)
}

#[test]
fn directory_import_is_persistent_idempotent_and_does_not_execute_contents() {
    let (_data, path, mut store) = store();
    let source = tempfile::tempdir().unwrap();
    fs::write(source.path().join("setup.sh"), "touch PWNED").unwrap();
    let project = store
        .import_project(source.path(), "Data project", "import-1")
        .unwrap();
    assert_eq!(
        project,
        store
            .import_project(source.path(), "Data project", "import-1")
            .unwrap()
    );
    assert_eq!(
        store
            .import_project(source.path(), "Changed request", "import-1")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert_eq!(
        store
            .import_project(source.path(), "Duplicate alias", "import-2")
            .unwrap_err()
            .code,
        "CONFIG_CONFLICT"
    );
    assert!(!source.path().join("PWNED").exists());
    assert!(!source.path().join(".git").exists());
    assert_eq!(store.sequence().unwrap(), 1);
    drop(store);
    let store = Store::open(&path).unwrap();
    assert_eq!(store.list_projects().unwrap(), vec![project]);
}

#[test]
fn reassociation_requires_directory_identity_and_revision_archive_preserves_files() {
    let (_data, path, mut store) = store();
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("old");
    let moved = parent.path().join("moved");
    fs::create_dir(&old).unwrap();
    fs::write(old.join("user.txt"), "precious source").unwrap();
    let project = store.import_project(&old, "Move", "import").unwrap();
    fs::rename(&old, &moved).unwrap();
    fs::create_dir(&old).unwrap();
    assert_eq!(
        store
            .reassociate_project(&project.id, &old, 1, "replace")
            .unwrap_err()
            .code,
        "INVALID_REQUEST"
    );
    let project = store
        .reassociate_project(&project.id, &moved, 1, "move")
        .unwrap();
    assert_eq!(
        project.identity.canonical_root,
        fs::canonicalize(&moved).unwrap()
    );
    assert_eq!(project.revision, 2);
    assert_eq!(
        store
            .archive_project(&project.id, 1, "stale")
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let archived = store.archive_project(&project.id, 2, "archive").unwrap();
    assert!(archived.archived);
    assert_eq!(
        store.archive_project(&project.id, 2, "archive").unwrap(),
        archived
    );
    assert_eq!(
        fs::read_to_string(moved.join("user.txt")).unwrap(),
        "precious source"
    );
    drop(store);
    assert!(
        Store::open(&path)
            .unwrap()
            .project(&project.id)
            .unwrap()
            .archived
    );
}

#[test]
fn project_and_journal_are_rolled_back_together_on_injected_failure() {
    let (_data, path, mut store) = store();
    let source = tempfile::tempdir().unwrap();
    let db = Connection::open(path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_project_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='project.imported' BEGIN SELECT RAISE(ABORT,'injected event failure'); END;").unwrap();
    assert_eq!(
        store
            .import_project(source.path(), "Project", "import")
            .unwrap_err()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(store.list_projects().unwrap().is_empty());
    assert_eq!(store.sequence().unwrap(), 0);
    db.execute_batch("DROP TRIGGER fail_project_event").unwrap();
    store
        .import_project(source.path(), "Project", "import")
        .unwrap();
    assert_eq!(store.sequence().unwrap(), 1);
}

#[test]
fn workspace_creation_protects_source_head_dirty_edits_and_retries() {
    let (_data, _path, mut store) = store();
    let (source, head) = repository();
    fs::write(source.path().join("source.txt"), "user dirty work").unwrap();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    assert_eq!(
        store
            .create_workspace(&project.id, 2, &head, managed.path(), "stale")
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(
        store
            .create_workspace(&project.id, 1, &"0".repeat(40), managed.path(), "bad-head")
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    let first = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace-1")
        .unwrap();
    assert_eq!(first.state, "ready");
    assert!(first.identity.is_some());
    assert_eq!(
        first,
        store
            .create_workspace(&project.id, 1, &head, managed.path(), "workspace-1")
            .unwrap()
    );
    assert_eq!(
        fs::read_to_string(first.root.join("source.txt")).unwrap(),
        "original\n"
    );
    fs::write(first.root.join("source.txt"), "agent edit").unwrap();
    let second = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace-2")
        .unwrap();
    assert_eq!(
        fs::read_to_string(second.root.join("source.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(source.path().join("source.txt")).unwrap(),
        "user dirty work"
    );
    assert_eq!(store.list_workspaces(&project.id).unwrap().len(), 2);
    store.archive_project(&project.id, 1, "archive").unwrap();
    assert_eq!(
        store
            .create_workspace(&project.id, 2, &head, managed.path(), "blocked")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    assert!(source.path().join("source.txt").exists());
}

#[test]
fn replaced_root_or_shared_managed_root_cannot_receive_new_writes() {
    use std::os::unix::fs::PermissionsExt;
    let (_data, _path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    fs::set_permissions(managed.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        store
            .create_workspace(&project.id, 1, &head, managed.path(), "shared")
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
    fs::set_permissions(managed.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let moved = source.path().with_extension("moved-for-test");
    fs::rename(source.path(), &moved).unwrap();
    fs::create_dir(source.path()).unwrap();
    assert!(store
        .create_workspace(&project.id, 1, &head, managed.path(), "replaced")
        .is_err());
    fs::remove_dir(source.path()).unwrap();
    fs::rename(moved, source.path()).unwrap();
    assert!(store.list_workspaces(&project.id).unwrap().is_empty());
}

#[test]
fn failed_final_persistence_recovers_unknown_and_never_replays_checkout() {
    let (_data, path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_workspace_commit BEFORE UPDATE ON managed_workspaces WHEN NEW.state='ready' BEGIN SELECT RAISE(ABORT,'injected final persistence failure'); END;").unwrap();
    let error = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap_err();
    assert_eq!(error.side_effect_state, "unknown");
    assert_eq!(fs::read_dir(managed.path()).unwrap().count(), 1);
    assert_eq!(
        store.list_workspaces(&project.id).unwrap()[0].state,
        "creating"
    );
    db.execute_batch("DROP TRIGGER fail_workspace_commit")
        .unwrap();
    drop(store);
    let mut store = Store::open(&path).unwrap();
    assert_eq!(
        store.list_workspaces(&project.id).unwrap()[0].state,
        "outcome_unknown"
    );
    assert_eq!(
        store
            .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
            .unwrap_err()
            .code,
        "OUTCOME_UNKNOWN"
    );
    assert_eq!(fs::read_dir(managed.path()).unwrap().count(), 1);
    let events = store.events(0, 100).unwrap();
    assert!(events["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["kind"] == "workspace.recovery_outcome_unknown"));
}

#[test]
fn leases_survive_restart_and_stale_fences_cannot_write_or_renew() {
    let (_data, path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    let workspace = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap();
    let first = store
        .acquire_workspace_lease(&workspace.id, "attempt-1", 0, 60_000)
        .unwrap();
    assert_eq!(first.fencing_token, 1);
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt-2", 1, 60_000)
            .unwrap_err()
            .code,
        "WORKSPACE_BUSY"
    );
    store
        .release_workspace_lease(&workspace.id, "attempt-1", 1)
        .unwrap();
    let second = store
        .acquire_workspace_lease(&workspace.id, "attempt-2", 1, 60_000)
        .unwrap();
    assert_eq!(second.fencing_token, 2);
    assert_eq!(
        store
            .validate_workspace_lease(&workspace.id, "attempt-1", 1)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    assert_eq!(
        store
            .release_workspace_lease(&workspace.id, "attempt-1", 1)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    drop(store);
    let mut store = Store::open(&path).unwrap();
    assert_eq!(
        store.workspace_lease(&workspace.id).unwrap().unwrap(),
        second
    );
    store
        .renew_workspace_lease(&workspace.id, "attempt-2", 2, 60_000)
        .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute(
        "UPDATE workspace_leases SET expires_at=0 WHERE resource=?1",
        [&workspace.id],
    )
    .unwrap();
    assert_eq!(
        store
            .renew_workspace_lease(&workspace.id, "attempt-2", 2, 60_000)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    let third = store
        .acquire_workspace_lease(&workspace.id, "attempt-3", 2, 60_000)
        .unwrap();
    assert_eq!(third.fencing_token, 3);
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt-4", 2, 60_000)
            .unwrap_err()
            .code,
        "STALE_REVISION"
    );
    store.archive_project(&project.id, 1, "archive").unwrap();
    assert_eq!(
        store
            .validate_workspace_lease(&workspace.id, "attempt-3", 3)
            .unwrap_err()
            .code,
        "PERMISSION_DENIED"
    );
}

#[test]
fn lease_state_and_journal_rollback_together() {
    let (_data, path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    let workspace = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap();
    let db = Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_lease_event BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='workspace.lease_acquired' BEGIN SELECT RAISE(ABORT,'injected lease event failure'); END;").unwrap();
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt", 0, 60_000)
            .unwrap_err()
            .code,
        "INTERNAL_ERROR"
    );
    assert!(store.workspace_lease(&workspace.id).unwrap().is_none());
    db.execute_batch("DROP TRIGGER fail_lease_event").unwrap();
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt", 0, 60_000)
            .unwrap()
            .fencing_token,
        1
    );
}

#[test]
fn lease_grants_and_renewals_reject_replaced_managed_directory() {
    let (_data, _path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    let workspace = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap();
    let lease = store
        .acquire_workspace_lease(&workspace.id, "attempt", 0, 60_000)
        .unwrap();
    let moved = workspace.root.with_extension("moved");
    fs::rename(&workspace.root, &moved).unwrap();
    fs::create_dir(&workspace.root).unwrap();
    assert_eq!(
        store
            .validate_workspace_lease(&workspace.id, "attempt", lease.fencing_token)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    assert_eq!(
        store
            .renew_workspace_lease(&workspace.id, "attempt", lease.fencing_token, 60_000)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    store
        .release_workspace_lease(&workspace.id, "attempt", lease.fencing_token)
        .unwrap();
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt-2", lease.fencing_token, 60_000)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
    fs::remove_dir(&workspace.root).unwrap();
    fs::rename(moved, &workspace.root).unwrap();
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt-2", lease.fencing_token, 60_000)
            .unwrap()
            .fencing_token,
        2
    );
}

#[test]
fn new_lease_requires_pinned_base_but_existing_writer_can_commit() {
    let (_data, _path, mut store) = store();
    let (source, head) = repository();
    let project = store
        .import_project(source.path(), "Coding", "import")
        .unwrap();
    let managed = private_managed_root();
    let workspace = store
        .create_workspace(&project.id, 1, &head, managed.path(), "workspace")
        .unwrap();
    store
        .acquire_workspace_lease(&workspace.id, "attempt", 0, 60_000)
        .unwrap();
    fs::write(workspace.root.join("candidate.txt"), "candidate").unwrap();
    git(&workspace.root, &["add", "candidate.txt"]);
    git(&workspace.root, &["commit", "-m", "candidate"]);
    store
        .validate_workspace_lease(&workspace.id, "attempt", 1)
        .unwrap();
    store
        .renew_workspace_lease(&workspace.id, "attempt", 1, 60_000)
        .unwrap();
    store
        .release_workspace_lease(&workspace.id, "attempt", 1)
        .unwrap();
    assert_eq!(
        store
            .acquire_workspace_lease(&workspace.id, "attempt-2", 1, 60_000)
            .unwrap_err()
            .code,
        "INPUT_CHANGED"
    );
}
