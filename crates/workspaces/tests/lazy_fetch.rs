//! Controlled, non-network regression: an unsafe Git remote helper would only
//! create a synthetic marker, then fail. Import/isolation must never execute it.
use agentbaton_workspaces::{
    check_adoption, create_isolated, inspect, read_artifact, WorkspaceError, WorkspaceKind,
};
use std::{
    fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    let output = command
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.fsmonitor=false",
        ])
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture Git: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
fn fixture() -> (
    tempfile::TempDir,
    PathBuf,
    tempfile::TempDir,
    String,
    PathBuf,
) {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path().canonicalize().unwrap();
    let managed = tempfile::tempdir().unwrap();
    git(&root, &["init", "-b", "fixture"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(root.join("file.txt"), b"synthetic object\n").unwrap();
    git(&root, &["add", "file.txt"]);
    git(&root, &["commit", "-m", "fixture"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    let blob = git(&root, &["rev-parse", "HEAD:file.txt"]);
    let marker = managed.path().join("helper-executed");
    let helper = managed.path().join("generated-remote-helper");
    fs::write(
        &helper,
        format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
    )
    .unwrap();
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o700)).unwrap();
    git(&root, &["config", "remote.origin.promisor", "true"]);
    git(
        &root,
        &[
            "config",
            "remote.origin.url",
            &format!("ext::{}", helper.display()),
        ],
    );
    git(
        &root,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
    );
    git(&root, &["config", "protocol.ext.allow", "always"]);
    fs::remove_file(root.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
    (repository, root, managed, head, marker)
}
#[test]
fn promisor_missing_blob_cannot_execute_remote_helper_during_import_or_checkout() {
    let (_repository, root, managed, head, marker) = fixture();
    let index_before = fs::read(root.join(".git/index")).unwrap();
    assert!(matches!(
        inspect(&root),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(matches!(
        create_isolated(
            &root,
            &managed.path().canonicalize().unwrap(),
            "isolated",
            &head
        ),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(!marker.exists());
    assert!(!managed.path().join("isolated").exists());
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index_before);
    assert_eq!(
        fs::read(root.join("file.txt")).unwrap(),
        b"synthetic object\n"
    );
}
#[test]
fn partial_clone_extension_is_rejected_without_external_configuration_repair() {
    let (_repository, root, _managed, _head, marker) = fixture();
    git(&root, &["config", "--unset", "remote.origin.promisor"]);
    git(&root, &["config", "extensions.partialClone", "origin"]);
    let config_before = fs::read(root.join(".git/config")).unwrap();
    assert!(matches!(
        inspect(&root),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(!marker.exists());
    assert_eq!(fs::read(root.join(".git/config")).unwrap(), config_before);
}

#[test]
fn replacement_refs_cannot_change_the_approved_commit_content() {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path().canonicalize().unwrap();
    let managed = tempfile::tempdir().unwrap();
    git(&root, &["init", "-b", "fixture"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.invalid"]);
    fs::write(root.join("file.txt"), b"approved\n").unwrap();
    git(&root, &["add", "file.txt"]);
    git(&root, &["commit", "-m", "approved"]);
    let approved = git(&root, &["rev-parse", "HEAD"]);
    let approved_tree = git(&root, &["rev-parse", "HEAD^{tree}"]);
    fs::write(root.join("file.txt"), b"replacement\n").unwrap();
    git(&root, &["commit", "-am", "replacement"]);
    let replacement = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["replace", &approved, &replacement]);
    git(&root, &["checkout", "--detach", &approved]);
    // Ordinary Git reports the approved hash and a clean worktree, but reads B.
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), approved);
    assert_ne!(git(&root, &["rev-parse", "HEAD^{tree}"]), approved_tree);
    assert!(git(&root, &["status", "--porcelain"]).is_empty());
    assert_eq!(fs::read(root.join("file.txt")).unwrap(), b"replacement\n");
    let replacement_ref = root.join(".git/refs/replace").join(&approved);
    let ref_before = fs::read(&replacement_ref).unwrap();
    let index_before = fs::read(root.join(".git/index")).unwrap();
    assert!(matches!(
        inspect(&root).unwrap().kind,
        WorkspaceKind::Git { head, dirty: true, .. } if head == approved
    ));
    assert!(matches!(
        check_adoption(&root, &approved),
        Err(WorkspaceError::Dirty)
    ));
    let isolated = create_isolated(&root, managed.path(), "raw-approved", &approved).unwrap();
    assert_eq!(
        read_artifact(&isolated.canonical_root, Path::new("file.txt"), 1024).unwrap(),
        b"approved\n"
    );
    assert!(
        matches!(isolated.kind, WorkspaceKind::Git { head, dirty: false, .. } if head == approved)
    );
    assert_eq!(fs::read(&replacement_ref).unwrap(), ref_before);
    assert_eq!(fs::read(root.join(".git/index")).unwrap(), index_before);
    assert_eq!(fs::read(root.join("file.txt")).unwrap(), b"replacement\n");
}
