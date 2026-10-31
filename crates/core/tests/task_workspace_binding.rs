//! Public Rust backend contract only; no IPC/Native/Provider is exercised.
use agentbaton_core::{
    storage::Store, CodeWorkspacePreparationRequest, GitWorkspacePin, TaskGitWorkspacePreparation,
    TaskWorkspaceBindingState,
};
use agentbaton_workspaces::WorkspaceKind;
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::Path,
    process::Command,
};

fn git(root: &Path, args: &[&str]) {
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
            "user.name=Public contract fixture",
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
    assert!(
        output.status.success(),
        "own generated Git operation failed"
    );
}

#[test]
fn public_backend_binding_survives_restart_without_source_writes_or_grant_authority() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let managed = root.path().join("managed");
    let db = root.path().join("store.sqlite3");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&managed).unwrap();
    fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(source.join("input"), "baseline\n").unwrap();
    git(&source, &["init", "--initial-branch=main"]);
    git(&source, &["add", "."]);
    git(&source, &["commit", "-m", "initial"]);
    fs::write(source.join("input"), "user dirty input\n").unwrap();
    let mut store = Store::open(&db).unwrap();
    let project = store
        .import_project(&source, "Generated project", "project")
        .unwrap();
    let task = store
        .create_registered(&project.id, "Code task", "task")
        .unwrap();
    let sibling = store
        .create_registered(&project.id, "Unbound sibling", "sibling")
        .unwrap();
    let preparation = store
        .resolve_code_workspace_preparation(&CodeWorkspacePreparationRequest {
            task_id: task.id.clone(),
            expected_task_revision: task.revision,
            expected_project_revision: project.revision,
            host_id: "local".into(),
        })
        .unwrap();
    let identity = preparation.source.unwrap();
    let WorkspaceKind::Git {
        common_dir, head, ..
    } = identity.kind
    else {
        panic!("Git fixture expected")
    };
    let request = TaskGitWorkspacePreparation {
        task_id: task.id.clone(),
        expected_task_revision: task.revision,
        expected_spec_revision: task.spec_revision,
        expected_project_revision: project.revision,
        host_id: "local".into(),
        expected_source_pin: GitWorkspacePin {
            canonical_root: identity.canonical_root,
            device: identity.device,
            inode: identity.inode,
            common_dir,
            head,
        },
        preparation_key: preparation.preparation_key.unwrap(),
    };
    let view = store
        .prepare_task_git_workspace(&request, &managed, "prepare")
        .unwrap();
    assert_eq!(view.binding.state, TaskWorkspaceBindingState::Ready);
    assert!(!view.execution_authorized);
    assert!(view.identity_revalidated);
    assert!(store
        .get(&sibling.id)
        .unwrap()
        .code_workspace_binding
        .is_none());
    let task_json = serde_json::to_value(store.get(&task.id).unwrap()).unwrap();
    assert!(task_json["code_workspace_binding"]["receipt"]["source_pin"]["device"].is_string());
    assert!(task_json["code_workspace_binding"]["managed_pin"]["inode"].is_string());
    let receipt = view.original_receipt;
    let workspace = managed.join(&receipt.workspace_id);
    assert_eq!(
        fs::read_to_string(workspace.join("input")).unwrap(),
        "baseline\n"
    );
    assert_eq!(
        fs::read_to_string(source.join("input")).unwrap(),
        "user dirty input\n"
    );
    drop(store);
    let mut reopened = Store::open(&db).unwrap();
    let again = reopened
        .prepare_task_git_workspace(&request, &managed, "prepare")
        .unwrap();
    assert_eq!(again.original_receipt, receipt);
    assert!(!again.execution_authorized);
    // The binding supplies no connection, Grant, Session, stop receipt or TaskAccepted.
    assert!(reopened
        .create_session(
            &task.id,
            &receipt.workspace_id,
            "missing-connection",
            "missing-grant",
            again.task_revision,
            "no-authority"
        )
        .is_err());
}
