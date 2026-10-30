//! Real generated local Git operations. These tests do not approve a Task,
//! validate correctness, adopt a ref, contact a Provider or use private files.
use agentbaton_workspaces::{
    inspect,
    source_capture::*,
    validation::{sha256, ArtifactRequirement, BoundArtifact},
    WorkspaceError, WorkspaceIdentity,
};
use std::{
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "fixture Git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .trim_end_matches('\n')
        .into()
}
struct Repo {
    _temp: tempfile::TempDir,
    root: PathBuf,
    base: String,
}
impl Repo {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        git(&root, &["init", "-b", "source"]);
        git(&root, &["config", "user.name", "Generated fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("selected.txt"), b"base selected\n").unwrap();
        fs::write(root.join("unselected.txt"), b"base unselected\n").unwrap();
        fs::write(root.join("executable"), b"base executable\n").unwrap();
        fs::set_permissions(root.join("executable"), fs::Permissions::from_mode(0o755)).unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "base"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        Self {
            _temp: temp,
            root,
            base,
        }
    }
    fn artifact(&self, id: &str, path: &str, bytes: &[u8], max: u64) -> BoundArtifact {
        BoundArtifact::from_approved_root(
            &self.root,
            ArtifactRequirement {
                artifact_id: id.into(),
                relative_path: path.into(),
                max_bytes: max,
                expected_sha256: Some(sha256(bytes)),
            },
        )
        .unwrap()
    }
    fn authorize(&self, items: Vec<BoundArtifact>) -> AuthorizedSourceCapture {
        AuthorizedSourceCapture::from_verified_selection(
            &inspect(&self.root).unwrap(),
            self.base.clone(),
            items,
            1_700_000_000,
            "selection-v1".into(),
            "independent-owner-policy".into(),
        )
        .unwrap()
    }
    fn original_state(&self) -> (Vec<u8>, String, String, String) {
        (
            fs::read(self.root.join(".git/index")).unwrap(),
            git(&self.root, &["show-ref"]),
            git(
                &self.root,
                &["status", "--porcelain=v1", "--untracked-files=all"],
            ),
            git(&self.root, &["diff"]),
        )
    }
}
#[test]
fn accepted_dirty_files_make_exact_commit_preserving_user_index_refs_and_unselected_bytes() {
    let r = Repo::new();
    fs::write(r.root.join("selected.txt"), b"accepted selected\n").unwrap();
    fs::write(r.root.join("new.txt"), b"accepted new\n").unwrap();
    fs::set_permissions(r.root.join("new.txt"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(r.root.join("executable"), b"accepted executable\n").unwrap();
    fs::write(r.root.join("unselected.txt"), b"dirty user edit\n").unwrap();
    fs::write(
        r.root.join("not-selected"),
        b"private untracked user output\n",
    )
    .unwrap();
    git(&r.root, &["add", "unselected.txt"]);
    let items = vec![
        r.artifact("a", "selected.txt", b"accepted selected\n", 1024),
        r.artifact("b", "new.txt", b"accepted new\n", 1024),
        r.artifact("c", "executable", b"accepted executable\n", 1024),
    ];
    let before = r.original_state();
    let result = capture_source_commit(r.authorize(items)).unwrap();
    assert_eq!(result.base(), r.base);
    assert_eq!(result.selected().len(), 3);
    assert_eq!(result.selected()[0].sha256, sha256(b"accepted selected\n"));
    assert_eq!(
        git(
            &r.root,
            &["show", &format!("{}:selected.txt", result.commit())]
        ),
        "accepted selected"
    );
    assert_eq!(
        git(&r.root, &["show", &format!("{}:new.txt", result.commit())]),
        "accepted new"
    );
    assert_eq!(
        git(
            &r.root,
            &["show", &format!("{}:unselected.txt", result.commit())]
        ),
        "base unselected"
    );
    assert!(git(&r.root, &["ls-tree", result.commit(), "not-selected"]).is_empty());
    assert!(git(&r.root, &["ls-tree", result.commit(), "new.txt"]).starts_with("100644 blob "));
    assert!(git(&r.root, &["ls-tree", result.commit(), "executable"]).starts_with("100755 blob "));
    assert_eq!(
        git(&r.root, &["show", "-s", "--format=%P", result.commit()]),
        r.base
    );
    assert_eq!(
        git(
            &r.root,
            &["rev-parse", &format!("{}^{{tree}}", result.commit())]
        ),
        result.tree()
    );
    assert_eq!(r.original_state(), before);
    assert_eq!(
        fs::read(r.root.join("unselected.txt")).unwrap(),
        b"dirty user edit\n"
    );
    assert_eq!(
        fs::read(r.root.join("not-selected")).unwrap(),
        b"private untracked user output\n"
    );
}
#[test]
fn identical_contract_and_bytes_have_reproducible_commit_tree_and_digest() {
    let r = Repo::new();
    let a = capture_source_commit(r.authorize(vec![r.artifact(
        "a",
        "selected.txt",
        b"base selected\n",
        64,
    )]))
    .unwrap();
    let b = capture_source_commit(r.authorize(vec![r.artifact(
        "a",
        "selected.txt",
        b"base selected\n",
        64,
    )]))
    .unwrap();
    assert_eq!(a.commit(), b.commit());
    assert_eq!(a.tree(), b.tree());
    assert_eq!(a.selection_digest(), b.selection_digest());
}
#[test]
fn wrong_sha_or_changed_selected_bytes_or_original_head_is_rejected_without_ref_updates() {
    let r = Repo::new();
    let refs = git(&r.root, &["show-ref"]);
    let wrong = r.authorize(vec![r.artifact("a", "selected.txt", b"wrong bytes", 64)]);
    assert!(capture_source_commit(wrong).is_err());
    let drift = r.authorize(vec![r.artifact(
        "a",
        "selected.txt",
        b"base selected\n",
        64,
    )]);
    fs::write(r.root.join("selected.txt"), b"replacement\n").unwrap();
    assert!(capture_source_commit(drift).is_err());
    assert_eq!(git(&r.root, &["show-ref"]), refs);
    let head_drift = r.authorize(vec![r.artifact("a", "selected.txt", b"replacement\n", 64)]);
    git(&r.root, &["commit", "--allow-empty", "-m", "advance"]);
    let after = git(&r.root, &["show-ref"]);
    assert!(matches!(
        capture_source_commit(head_drift),
        Err(WorkspaceError::StaleHead)
    ));
    assert_eq!(git(&r.root, &["show-ref"]), after);
}
fn selection(
    r: &Repo,
    root: &WorkspaceIdentity,
    items: Vec<BoundArtifact>,
) -> Result<AuthorizedSourceCapture, WorkspaceError> {
    AuthorizedSourceCapture::from_verified_selection(
        root,
        r.base.clone(),
        items,
        1_700_000_000,
        "selection".into(),
        "owner-policy".into(),
    )
}
#[test]
fn wrong_root_identity_missing_sha_ids_paths_case_aliases_and_counts_are_rejected() {
    let r = Repo::new();
    let original = inspect(&r.root).unwrap();
    let mut wrong = original.clone();
    wrong.inode += 1;
    assert!(selection(
        &r,
        &wrong,
        vec![r.artifact("a", "selected.txt", b"base selected\n", 64)]
    )
    .is_err());
    let other = Repo::new();
    assert!(selection(
        &r,
        &original,
        vec![other.artifact("a", "selected.txt", b"base selected\n", 64)]
    )
    .is_err());
    let missing = BoundArtifact::from_approved_root(
        &r.root,
        ArtifactRequirement {
            artifact_id: "a".into(),
            relative_path: "selected.txt".into(),
            max_bytes: 64,
            expected_sha256: None,
        },
    )
    .unwrap();
    assert!(selection(&r, &original, vec![missing]).is_err());
    for pairs in [
        [("a", "selected.txt"), ("a", "other")],
        [("a", "selected.txt"), ("b", "selected.txt")],
        [("a", "selected.txt"), ("b", "SELECTED.TXT")],
        [("a", "dir"), ("b", "dir/child")],
        [("a", ".Git/config"), ("b", "other")],
    ] {
        let items = pairs
            .iter()
            .map(|(id, path)| r.artifact(id, path, b"unused", 64))
            .collect();
        assert!(selection(&r, &original, items).is_err());
    }
    for path in ["../escape", "/absolute", "a/../escape"] {
        assert!(BoundArtifact::from_approved_root(
            &r.root,
            ArtifactRequirement {
                artifact_id: "a".into(),
                relative_path: path.into(),
                max_bytes: 64,
                expected_sha256: Some(sha256(b"unused"))
            }
        )
        .is_err());
    }
    let items = (0..257)
        .map(|i| r.artifact(&format!("id{i}"), &format!("file{i}"), b"unused", 64))
        .collect();
    assert!(matches!(
        selection(&r, &original, items),
        Err(WorkspaceError::TooLarge)
    ));
}
#[test]
fn symlink_hardlink_fifo_and_oversize_selected_files_are_never_imported() {
    let r = Repo::new();
    symlink("selected.txt", r.root.join("link")).unwrap();
    fs::hard_link(r.root.join("selected.txt"), r.root.join("hard")).unwrap();
    fs::write(r.root.join("large"), b"more than four bytes").unwrap();
    let fifo = r.root.join("fifo");
    let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let before = r.original_state();
    for (path, max) in [("link", 64), ("hard", 64), ("fifo", 64), ("large", 4)] {
        let token = r.authorize(vec![r.artifact("a", path, b"unused", max)]);
        assert!(capture_source_commit(token).is_err());
    }
    assert_eq!(r.original_state(), before);
}
#[test]
fn selected_base_symlink_gitlink_and_directory_types_cannot_be_silently_replaced() {
    for kind in ["symlink", "gitlink", "directory"] {
        let mut r = Repo::new();
        match kind {
            "symlink" => {
                symlink("selected.txt", r.root.join("special")).unwrap();
                git(&r.root, &["add", "special"]);
            }
            "gitlink" => {
                git(
                    &r.root,
                    &[
                        "update-index",
                        "--add",
                        "--cacheinfo",
                        "160000",
                        &r.base,
                        "special",
                    ],
                );
            }
            _ => {
                fs::create_dir(r.root.join("special")).unwrap();
                fs::write(r.root.join("special/file"), b"base").unwrap();
                git(&r.root, &["add", "special"]);
            }
        }
        git(&r.root, &["commit", "-m", "special type"]);
        r.base = git(&r.root, &["rev-parse", "HEAD"]);
        if kind == "directory" {
            fs::remove_file(r.root.join("special/file")).unwrap();
            fs::remove_dir(r.root.join("special")).unwrap();
        } else if kind == "symlink" {
            fs::remove_file(r.root.join("special")).unwrap();
        }
        fs::write(r.root.join("special"), b"selected regular\n").unwrap();
        let before = r.original_state();
        assert!(capture_source_commit(r.authorize(vec![r.artifact(
            "a",
            "special",
            b"selected regular\n",
            64
        )]))
        .is_err());
        assert_eq!(r.original_state(), before);
    }
}
#[test]
fn executable_filter_include_alternates_and_promisor_configuration_fail_closed_without_execution() {
    for kind in ["filter", "include", "alternates", "promisor", "merge"] {
        let r = Repo::new();
        let artifact = r.artifact("a", "selected.txt", b"base selected\n", 64);
        let identity = inspect(&r.root).unwrap();
        let marker = r.root.join("must-not-execute");
        match kind {
            "filter" => {
                git(
                    &r.root,
                    &[
                        "config",
                        "filter.evil.clean",
                        &format!("touch {}", marker.display()),
                    ],
                );
                fs::write(r.root.join(".gitattributes"), b"*.txt filter=evil\n").unwrap();
            }
            "include" => {
                git(&r.root, &["config", "include.path", "missing-config"]);
            }
            "alternates" => {
                fs::write(
                    r.root.join(".git/objects/info/alternates"),
                    "/not-approved/object-source\n",
                )
                .unwrap();
            }
            "promisor" => {
                git(&r.root, &["config", "remote.evil.promisor", "true"]);
                git(
                    &r.root,
                    &[
                        "config",
                        "remote.evil.url",
                        &format!("ext::touch {}", marker.display()),
                    ],
                );
            }
            _ => {
                git(
                    &r.root,
                    &[
                        "config",
                        "merge.evil.driver",
                        &format!("touch {}", marker.display()),
                    ],
                );
            }
        }
        let index = fs::read(r.root.join(".git/index")).unwrap();
        assert!(selection(&r, &identity, vec![artifact]).is_err());
        assert!(!marker.exists());
        assert_eq!(fs::read(r.root.join(".git/index")).unwrap(), index);
    }
}
#[test]
fn hooks_are_disabled_while_dirty_capture_and_original_hooks_config_are_preserved() {
    let r = Repo::new();
    let marker = r.root.join("must-not-run-hook");
    let hook = r.root.join(".git/hooks/post-commit");
    fs::write(&hook, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let hook_bytes = fs::read(&hook).unwrap();
    fs::write(r.root.join("selected.txt"), b"accepted\n").unwrap();
    capture_source_commit(r.authorize(vec![r.artifact("a", "selected.txt", b"accepted\n", 64)]))
        .unwrap();
    assert!(!marker.exists());
    assert_eq!(fs::read(&hook).unwrap(), hook_bytes);
}
#[test]
fn replaced_source_directory_or_common_directory_after_authorization_is_rejected() {
    for common in [false, true] {
        let r = Repo::new();
        let token = r.authorize(vec![r.artifact(
            "a",
            "selected.txt",
            b"base selected\n",
            64,
        )]);
        if common {
            fs::rename(r.root.join(".git"), r.root.join("old-git")).unwrap();
            fs::create_dir(r.root.join(".git")).unwrap();
        } else {
            let moved = r.root.with_extension("held-original");
            fs::rename(&r.root, &moved).unwrap();
            fs::create_dir(&r.root).unwrap();
            // Restore the generated source after checking so TempDir still owns cleanup.
            assert!(capture_source_commit(token).is_err());
            fs::remove_dir(&r.root).unwrap();
            fs::rename(moved, &r.root).unwrap();
            continue;
        }
        assert!(capture_source_commit(token).is_err());
    }
}

#[test]
fn ascii_case_aliases_and_unicode_selections_or_base_paths_fail_without_user_changes() {
    let mut case = Repo::new();
    fs::write(case.root.join("BaseCase.txt"), b"base case\n").unwrap();
    git(&case.root, &["add", "BaseCase.txt"]);
    git(&case.root, &["commit", "-m", "case path"]);
    case.base = git(&case.root, &["rev-parse", "HEAD"]);
    fs::write(case.root.join("basecase.txt"), b"accepted case\n").unwrap();
    let before = case.original_state();
    let original = fs::read(case.root.join("BaseCase.txt")).unwrap();
    assert!(capture_source_commit(case.authorize(vec![case.artifact(
        "a",
        "basecase.txt",
        b"accepted case\n",
        64
    )]))
    .is_err());
    assert_eq!(case.original_state(), before);
    assert_eq!(fs::read(case.root.join("BaseCase.txt")).unwrap(), original);
    for name in ["unicode-é.txt", "unicode-e\u{301}.txt"] {
        let mut r = Repo::new();
        fs::write(r.root.join(name), b"base named file\n").unwrap();
        git(&r.root, &["add", "--", name]);
        git(&r.root, &["commit", "-m", "path guard"]);
        r.base = git(&r.root, &["rev-parse", "HEAD"]);
        fs::write(r.root.join("selected.txt"), b"accepted\n").unwrap();
        let before = r.original_state();
        assert!(capture_source_commit(r.authorize(vec![r.artifact(
            "a",
            "selected.txt",
            b"accepted\n",
            64
        )]))
        .is_err());
        assert_eq!(r.original_state(), before);
        assert_eq!(fs::read(r.root.join(name)).unwrap(), b"base named file\n");
    }
    let r = Repo::new();
    fs::write(r.root.join("é.txt"), b"selected unicode\n").unwrap();
    let before = r.original_state();
    assert!(selection(
        &r,
        &inspect(&r.root).unwrap(),
        vec![r.artifact("a", "é.txt", b"selected unicode\n", 64)]
    )
    .is_err());
    assert_eq!(r.original_state(), before);
}
#[test]
fn total_selected_byte_budget_is_shared_and_not_reset_per_artifact() {
    let r = Repo::new();
    const SIZE: u64 = 33 * 1024 * 1024;
    for name in ["first", "second"] {
        fs::File::create(r.root.join(name))
            .unwrap()
            .set_len(SIZE)
            .unwrap();
    }
    let zeros = vec![0u8; SIZE as usize];
    let items = vec![
        r.artifact("a", "first", &zeros, SIZE),
        r.artifact("b", "second", &zeros, SIZE),
    ];
    drop(zeros);
    let before = r.original_state();
    assert!(matches!(
        capture_source_commit(r.authorize(items)),
        Err(WorkspaceError::TooLarge)
    ));
    assert_eq!(r.original_state(), before);
    assert_eq!(fs::metadata(r.root.join("first")).unwrap().len(), SIZE);
    assert_eq!(fs::metadata(r.root.join("second")).unwrap().len(), SIZE);
}
#[test]
fn symlinked_object_storage_is_rejected_before_any_object_write() {
    let r = Repo::new();
    let identity = inspect(&r.root).unwrap();
    let object_store = tempfile::tempdir().unwrap();
    let prefix = (0..=255)
        .map(|n| format!("{n:02x}"))
        .find(|p| !r.root.join(".git/objects").join(p).exists())
        .unwrap();
    symlink(
        object_store.path(),
        r.root.join(".git/objects").join(prefix),
    )
    .unwrap();
    let before = r.original_state();
    assert!(selection(
        &r,
        &identity,
        vec![r.artifact("a", "selected.txt", b"base selected\n", 64)]
    )
    .is_err());
    assert_eq!(fs::read_dir(object_store.path()).unwrap().count(), 0);
    assert_eq!(r.original_state(), before);
}

#[test]
fn unrepresentable_empty_base_subtree_is_rejected_instead_of_silently_removed() {
    let mut r = Repo::new();
    let empty = git(&r.root, &["mktree"]);
    let mut raw = b"40000 empty\0".to_vec();
    for pair in empty.as_bytes().chunks_exact(2) {
        raw.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap());
    }
    fs::write(r.root.join("raw-generated-tree"), raw).unwrap();
    let tree = git(
        &r.root,
        &[
            "hash-object",
            "-w",
            "--no-filters",
            "-t",
            "tree",
            "raw-generated-tree",
        ],
    );
    let commit = git(
        &r.root,
        &[
            "commit-tree",
            &tree,
            "-p",
            &r.base,
            "-m",
            "empty subtree fixture",
        ],
    );
    git(&r.root, &["update-ref", "HEAD", &commit, &r.base]);
    r.base = commit;
    let before = r.original_state();
    assert!(matches!(
        capture_source_commit(r.authorize(vec![r.artifact(
            "a",
            "selected.txt",
            b"base selected\n",
            64
        )])),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert_eq!(r.original_state(), before);
    assert_eq!(
        git(&r.root, &["rev-parse", &format!("{}^{{tree}}", r.base)]),
        tree
    );
}
