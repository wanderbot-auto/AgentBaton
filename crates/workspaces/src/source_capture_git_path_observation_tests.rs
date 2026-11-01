//! Actual Git contracts plus malformed-observation controls. No model/provider.
use super::*;
use std::sync::{atomic::AtomicBool, Arc};

fn repository(prefix: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    for args in [
        vec!["init", "-b", "source"],
        vec!["config", "user.name", "Generated fixture"],
        vec!["config", "user.email", "fixture@example.invalid"],
    ] {
        assert!(git_command(&root)
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(root.join("file"), b"base\n").unwrap();
    assert!(git_command(&root)
        .args(["add", "file"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(git_command(&root)
        .args(["commit", "-m", "base"])
        .output()
        .unwrap()
        .status
        .success());
    (directory, root)
}
fn fields(root: &Path) -> (PathBuf, PathBuf, Vec<u8>) {
    let git = root.join(".git").canonicalize().unwrap();
    let common = git.clone();
    let raw = format!(
        "{}\n{}\n{}\n",
        root.display(),
        git.display(),
        common.display()
    )
    .into_bytes();
    (git, common, raw)
}
fn authorized(root: &Path) -> AuthorizedSourceCapture {
    let workspace = crate::inspect(root).unwrap();
    let WorkspaceKind::Git { head, .. } = &workspace.kind else {
        panic!("actual Git required")
    };
    let artifact = BoundArtifact::from_approved_root(
        root,
        crate::validation::ArtifactRequirement {
            artifact_id: "file".into(),
            relative_path: "file".into(),
            max_bytes: 32,
            expected_sha256: Some(sha256(b"base\n")),
        },
    )
    .unwrap();
    AuthorizedSourceCapture::from_verified_selection(
        &workspace,
        head.clone(),
        vec![artifact],
        1_700_000_000,
        "selection".into(),
        "owner-policy".into(),
    )
    .unwrap()
}

// Existing Serialize is a read-only observation, not a new field accessor.
// Counts remain exact u64; missing, negative or non-integer fields fail tests.
fn git_counts(io: &crate::adoption::ControlledPreparationIo) -> (u64, u64) {
    let value = serde_json::to_value(io).unwrap();
    (
        value["git_calls_attempted"].as_u64().unwrap(),
        value["git_calls_complete"].as_u64().unwrap(),
    )
}

#[test]
fn actual_batch_matches_all_three_original_observations_with_one_counted_child() {
    let (_directory, root) = repository("source-paths space-雪-");
    let (git, common, _) = fields(&root);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = crate::adoption::AdoptionControl::for_request(&cancel, deadline);
    recheck_git_paths(deadline, &root, [&root, &git, &common], Some(&control)).unwrap();
    let observed = control.preparation_io();
    assert_eq!(git_counts(&observed).0, 1);
    assert_eq!(git_counts(&observed).1, 1);
    assert!(observed.io_complete() && !observed.interrupted());
    assert!(!observed.effects_may_have_started());
    for (arg, expected) in [
        ("--show-toplevel", &root),
        ("--absolute-git-dir", &git),
        ("--git-common-dir", &common),
    ] {
        assert_eq!(git_path(deadline, &root, arg, None).unwrap(), *expected);
    }
}

#[test]
fn actual_linked_worktree_keeps_distinct_git_dir_and_common_dir() {
    let (_directory, root) = repository("source-paths-linked-");
    let managed = tempfile::tempdir().unwrap();
    let linked = managed.path().canonicalize().unwrap().join("linked");
    assert!(git_command(&root)
        .args(["worktree", "add", "--detach", "--"])
        .arg(&linked)
        .arg("HEAD")
        .output()
        .unwrap()
        .status
        .success());
    let deadline = Instant::now() + Duration::from_secs(5);
    let git = git_path(deadline, &linked, "--absolute-git-dir", None).unwrap();
    let common = git_path(deadline, &linked, "--git-common-dir", None).unwrap();
    assert_ne!(git, common);
    recheck_git_paths(deadline, &linked, [&linked, &git, &common], None).unwrap();
    assert!(recheck_git_paths(deadline, &linked, [&linked, &common, &git], None).is_err());
}

#[test]
fn malformed_extra_short_non_utf8_control_and_oversized_fields_are_rejected() {
    let (_directory, root) = repository("source-paths-shape-");
    let (git, common, raw) = fields(&root);
    let deadline = Instant::now() + Duration::from_secs(5);
    let extra = [raw.as_slice(), b"extra\n"].concat();
    let short = format!("{}\n{}\n", root.display(), git.display()).into_bytes();
    let blank = format!("{}\n\n{}\n", root.display(), common.display()).into_bytes();
    let carriage = [b"/root\r\n".as_slice(), raw.as_slice()].concat();
    let oversized = [vec![b'x'; MAX_PATH_BYTES + 1], b"\n/a\n/b\n".to_vec()].concat();
    for invalid in [
        extra,
        short,
        blank,
        raw[..raw.len() - 1].to_vec(),
        b"\xff\n/a\n/b\n".to_vec(),
        carriage,
        oversized,
    ] {
        assert!(
            verify_git_path_observation(&invalid, [&root, &git, &common], deadline, None).is_err()
        );
    }
    let mismatched = format!(
        "{}\n{}\n{}\n",
        root.display(),
        root.display(),
        common.display()
    );
    assert!(verify_git_path_observation(
        mismatched.as_bytes(),
        [&root, &git, &common],
        deadline,
        None
    )
    .is_err());
}

#[test]
fn real_newline_path_cannot_be_reframed_into_three_pinned_fields() {
    let (_directory, root) = repository("source-paths-new\nline-");
    let (git, common, _) = fields(&root);
    assert!(recheck_git_paths(
        Instant::now() + Duration::from_secs(5),
        &root,
        [&root, &git, &common],
        None
    )
    .is_err());
}

#[test]
fn canonical_symlink_alias_is_rejected_instead_of_normalized_into_pin() {
    let (_directory, root) = repository("source-paths-alias-");
    let (git, common, _) = fields(&root);
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).unwrap();
    let raw = format!(
        "{}\n{}\n{}\n",
        alias.display(),
        git.display(),
        common.display()
    );
    assert!(verify_git_path_observation(
        raw.as_bytes(),
        [&root, &git, &common],
        Instant::now() + Duration::from_secs(5),
        None
    )
    .is_err());
}

#[test]
fn actual_non_repository_nonzero_exit_never_returns_a_path_observation() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = crate::adoption::AdoptionControl::for_request(&cancel, deadline);
    assert!(recheck_git_paths(deadline, &root, [&root, &root, &root], Some(&control)).is_err());
    assert_eq!(git_counts(&control.preparation_io()).0, 1);
}

#[test]
fn original_cancelled_and_expired_clocks_reject_before_spawn() {
    let (_directory, root) = repository("source-paths-expired-");
    let (git, common, _) = fields(&root);
    for (cancelled, expired) in [(true, false), (false, true)] {
        let cancel = AtomicBool::new(cancelled);
        let deadline = if expired {
            Instant::now() - Duration::from_millis(1)
        } else {
            Instant::now() + Duration::from_secs(5)
        };
        let control = crate::adoption::AdoptionControl::for_request(&cancel, deadline);
        assert!(matches!(
            recheck_git_paths(deadline, &root, [&root, &git, &common], Some(&control)),
            Err(WorkspaceError::ResourceLimit)
        ));
        assert_eq!(git_counts(&control.preparation_io()).0, 0);
    }
}

#[test]
fn actual_spawn_then_cancel_still_rejects_and_keeps_real_counted_io() {
    let (_directory, root) = repository("source-paths-cancel-");
    let (git, common, _) = fields(&root);
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = crate::adoption::AdoptionControl::for_request(&cancel, deadline);
    let _hook = crate::controlled_spawn_hook::install(move |_| flag.store(true, Ordering::Release));
    assert!(matches!(
        recheck_git_paths(deadline, &root, [&root, &git, &common], Some(&control)),
        Err(WorkspaceError::ResourceLimit)
    ));
    let observed = control.preparation_io();
    assert_eq!(git_counts(&observed).0, 1);
    assert!(observed.interrupted());
    assert!(!observed.effects_may_have_started());
}

#[test]
fn actual_spawn_elapsed_past_original_deadline_never_accepts_finished_output() {
    let (_directory, root) = repository("source-paths-timeout-");
    let (git, common, _) = fields(&root);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    let control = crate::adoption::AdoptionControl::for_request(&cancel, deadline);
    let _hook = crate::controlled_spawn_hook::install(move |_| {
        // Test-only barrier advances the original clock; production has no delay.
        std::thread::sleep(
            deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(1),
        );
    });
    assert!(matches!(
        recheck_git_paths(deadline, &root, [&root, &git, &common], Some(&control)),
        Err(WorkspaceError::ResourceLimit)
    ));
    let observed = control.preparation_io();
    assert_eq!(git_counts(&observed).0, 1);
    assert!(observed.interrupted());
}

#[test]
fn independent_bound_head_recheck_still_refuses_new_commit_after_same_paths() {
    let (_directory, root) = repository("source-paths-head-");
    let capture = authorized(&root);
    assert!(git_command(&root)
        .args(["commit", "--allow-empty", "-m", "advance"])
        .output()
        .unwrap()
        .status
        .success());
    assert!(matches!(
        capture.recheck(None),
        Err(WorkspaceError::StaleHead)
    ));
}
