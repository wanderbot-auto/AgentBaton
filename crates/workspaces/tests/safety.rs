use agentbaton_workspaces::{
    check_adoption, create_isolated, inspect, read_artifact, reassociate, WorkspaceError,
    WorkspaceKind,
};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
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
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
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

#[test]
fn imports_data_without_executing_scripts_or_initializing_git() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("setup.sh"), "touch PWNED").unwrap();
    assert_eq!(inspect(root.path()).unwrap().kind, WorkspaceKind::Directory);
    assert!(!root.path().join(".git").exists());
    assert!(!root.path().join("PWNED").exists());
}

#[test]
fn import_does_not_invoke_configured_fsmonitor() {
    let (root, _) = repository();
    let marker = root.path().join("PWNED");
    git(
        root.path(),
        &[
            "config",
            "core.fsmonitor",
            &format!("touch {}", marker.display()),
        ],
    );
    inspect(root.path()).unwrap();
    assert!(!marker.exists());
}

#[test]
fn import_and_isolation_reject_clean_filters_before_status_can_run_them() {
    let (root, head) = repository();
    let managed = tempfile::tempdir().unwrap();
    let marker = root.path().join("PWNED");
    fs::write(
        root.path().join(".gitattributes"),
        "source.txt filter=unapproved\n",
    )
    .unwrap();
    fs::write(root.path().join("source.txt"), "modified before import\n").unwrap();
    git(
        root.path(),
        &[
            "config",
            "filter.unapproved.clean",
            &format!("touch '{}'; cat", marker.display()),
        ],
    );
    assert!(matches!(
        inspect(root.path()),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(matches!(
        create_isolated(root.path(), managed.path(), "attempt", &head),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(!marker.exists());
}

#[test]
fn rejects_invalid_repository_instead_of_hiding_it_as_data() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join(".git"), "bad metadata").unwrap();
    assert!(matches!(
        inspect(root.path()),
        Err(WorkspaceError::NotRepository)
    ));
    let (unborn, _) = repository();
    let nested = unborn.path().join("nested");
    fs::create_dir(&nested).unwrap();
    assert!(matches!(inspect(&nested), Err(WorkspaceError::InvalidPath)));
}

#[test]
fn detached_workspaces_preserve_dirty_source_and_do_not_share_writes() {
    let (root, head) = repository();
    fs::write(root.path().join("source.txt"), "unsaved user work").unwrap();
    fs::write(root.path().join("untracked.txt"), "untracked").unwrap();
    let managed = tempfile::tempdir().unwrap();
    let first = create_isolated(root.path(), managed.path(), "attempt-1", &head).unwrap();
    let second = create_isolated(root.path(), managed.path(), "attempt-2", &head).unwrap();
    assert_eq!(
        fs::read_to_string(first.canonical_root.join("source.txt")).unwrap(),
        "original\n"
    );
    fs::write(first.canonical_root.join("source.txt"), "agent edit").unwrap();
    assert_eq!(
        fs::read_to_string(second.canonical_root.join("source.txt")).unwrap(),
        "original\n"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("source.txt")).unwrap(),
        "unsaved user work"
    );
    assert_eq!(
        fs::read_to_string(root.path().join("untracked.txt")).unwrap(),
        "untracked"
    );
    assert!(matches!(
        check_adoption(root.path(), &head),
        Err(WorkspaceError::Dirty)
    ));
    assert!(matches!(
        create_isolated(root.path(), managed.path(), "attempt-1", &head),
        Err(WorkspaceError::InvalidPath)
    ));
}

#[test]
fn checkout_hooks_and_filters_cannot_execute_during_isolation() {
    use std::os::unix::fs::PermissionsExt;
    let (root, head) = repository();
    let marker = root.path().join("PWNED");
    let hook = root.path().join(".git/hooks/post-checkout");
    fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let managed = tempfile::tempdir().unwrap();
    create_isolated(root.path(), managed.path(), "without-hooks", &head).unwrap();
    assert!(!marker.exists());
    git(
        root.path(),
        &[
            "config",
            "filter.unapproved.smudge",
            &format!("touch {}", marker.display()),
        ],
    );
    assert!(matches!(
        create_isolated(root.path(), managed.path(), "with-filter", &head),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(!marker.exists());
}

#[test]
fn expected_head_and_dirty_guard_require_revalidation() {
    let (root, head) = repository();
    check_adoption(root.path(), &head).unwrap();
    fs::write(root.path().join("source.txt"), "new revision").unwrap();
    git(root.path(), &["add", "source.txt"]);
    git(root.path(), &["commit", "-m", "changed"]);
    assert!(matches!(
        check_adoption(root.path(), &head),
        Err(WorkspaceError::StaleHead)
    ));
    assert!(matches!(
        check_adoption(root.path(), "HEAD"),
        Err(WorkspaceError::InvalidPath)
    ));
}

#[test]
fn workspace_reassociation_distinguishes_move_from_replacement() {
    let parent = tempfile::tempdir().unwrap();
    let old = parent.path().join("old");
    let moved = parent.path().join("moved");
    fs::create_dir(&old).unwrap();
    let original = inspect(&old).unwrap();
    fs::rename(&old, &moved).unwrap();
    assert_eq!(
        reassociate(&original, &moved).unwrap().inode,
        original.inode
    );
    fs::create_dir(&old).unwrap();
    assert!(matches!(
        reassociate(&original, &old),
        Err(WorkspaceError::InvalidPath)
    ));
}

#[test]
fn hostile_names_and_nested_managed_paths_are_rejected() {
    let (root, head) = repository();
    let managed = tempfile::tempdir().unwrap();
    for name in [
        "../escape",
        "--exec=bad",
        "a/b",
        "a;touch PWNED",
        "",
        ".hidden",
    ] {
        assert!(matches!(
            create_isolated(root.path(), managed.path(), name, &head),
            Err(WorkspaceError::InvalidPath)
        ));
    }
    assert!(matches!(
        create_isolated(root.path(), root.path(), "attempt", &head),
        Err(WorkspaceError::InvalidPath)
    ));
}

#[test]
fn artifact_read_checks_content_size_and_regular_file() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("nested")).unwrap();
    fs::write(root.path().join("nested/output.txt"), "data").unwrap();
    assert_eq!(
        read_artifact(
            &fs::canonicalize(root.path()).unwrap(),
            Path::new("nested/output.txt"),
            4
        )
        .unwrap(),
        b"data"
    );
    assert!(matches!(
        read_artifact(
            &fs::canonicalize(root.path()).unwrap(),
            Path::new("nested/output.txt"),
            3
        ),
        Err(WorkspaceError::TooLarge)
    ));
    assert!(matches!(
        read_artifact(
            &fs::canonicalize(root.path()).unwrap(),
            Path::new("nested"),
            100
        ),
        Err(WorkspaceError::NotRegularFile)
    ));
}

#[test]
fn artifact_read_rejects_traversal_and_symlinks_at_each_component() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let external = tempfile::tempdir().unwrap();
    fs::write(external.path().join("secret"), "must not read").unwrap();
    symlink(external.path(), root.path().join("linked-directory")).unwrap();
    symlink(
        external.path().join("secret"),
        root.path().join("linked-file"),
    )
    .unwrap();
    for path in [
        Path::new("../secret"),
        Path::new("/etc/passwd"),
        Path::new("linked-file"),
        Path::new("linked-directory/secret"),
        Path::new(""),
    ] {
        assert!(
            read_artifact(&fs::canonicalize(root.path()).unwrap(), path, 100).is_err(),
            "{path:?}"
        );
    }
    let named_pipe = root.path().join("fifo");
    let name = std::ffi::CString::new(named_pipe.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: pathname is a valid NUL-terminated filename in the temporary test directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        read_artifact(
            &fs::canonicalize(root.path()).unwrap(),
            Path::new("fifo"),
            100
        ),
        Err(WorkspaceError::NotRegularFile)
    ));
}

#[test]
fn artifact_read_rejects_symlinked_approved_root_ancestors() {
    use std::os::unix::fs::symlink;
    let parent = tempfile::tempdir().unwrap();
    fs::create_dir_all(parent.path().join("real/nested")).unwrap();
    fs::write(parent.path().join("real/nested/output"), "data").unwrap();
    symlink(parent.path().join("real"), parent.path().join("alias")).unwrap();
    assert!(read_artifact(&parent.path().join("alias/nested"), Path::new("output"), 4).is_err());
    let approved = fs::canonicalize(parent.path().join("real/nested")).unwrap();
    assert_eq!(
        read_artifact(&approved, Path::new("output"), 4).unwrap(),
        b"data"
    );
}

#[test]
fn gitlinks_block_adoption_without_inspecting_nested_repository_code() {
    let (root, head) = repository();
    git(
        root.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{head},nested-repository"),
        ],
    );
    git(root.path(), &["commit", "-m", "gitlink"]);
    let head = git(root.path(), &["rev-parse", "HEAD"]);
    assert!(matches!(
        check_adoption(root.path(), &head),
        Err(WorkspaceError::Dirty)
    ));
}
