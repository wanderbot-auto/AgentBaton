//! Real generated Git/FS controls. Host-verified constructor inputs below are
//! fixture authority only; complete IO never means Task/Goal acceptance or CAS.
use agentbaton_workspaces::{
    adoption::{
        prepare_candidate, prepare_candidate_controlled, AdoptionControl, AdoptionError,
        AuthorizedIntegration, ControlledPreparationIo,
    },
    create_isolated_controlled, inspect,
    source_capture::{
        capture_source_commit, capture_source_commit_controlled, AuthorizedSourceCapture,
    },
    validation::{sha256, ArtifactRequirement, BoundArtifact},
    WorkspaceError, WorkspaceIdentity,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

struct KeepOnFailure(Option<tempfile::TempDir>);
impl Drop for KeepOnFailure {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(root) = self.0.take() {
                eprintln!(
                    "Retained controlled preparation test: {}",
                    root.keep().display()
                );
            }
        }
    }
}
struct Repository {
    root: PathBuf,
    source: PathBuf,
    managed: PathBuf,
    base: String,
    candidate: String,
    _retained: KeepOnFailure,
}
fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "init.templateDir=",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "protocol.allow=never",
            "-c",
            "user.name=Generated preparation fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "own Git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}
fn text(root: &Path, args: &[&str]) -> String {
    String::from_utf8(git(root, args))
        .unwrap()
        .trim()
        .to_owned()
}
impl Repository {
    fn new() -> Self {
        let temporary = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(std::env::temp_dir().canonicalize().unwrap())
            .unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let retained = KeepOnFailure(Some(temporary));
        let source = root.join("source");
        let managed = root.join("managed");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        git(&source, &["init", "-q", "--initial-branch=source"]);
        fs::write(source.join("file.txt"), b"baseline\n").unwrap();
        git(&source, &["add", "file.txt"]);
        git(&source, &["commit", "-qm", "generated baseline"]);
        let base = text(&source, &["rev-parse", "HEAD"]);
        git(&source, &["branch", "integration"]);
        fs::write(source.join("file.txt"), b"candidate\n").unwrap();
        git(&source, &["commit", "-qam", "generated candidate"]);
        let candidate = text(&source, &["rev-parse", "HEAD"]);
        Self {
            root,
            source,
            managed,
            base,
            candidate,
            _retained: retained,
        }
    }
    fn authorize(
        &self,
        identity: &WorkspaceIdentity,
        control: &AdoptionControl<'_>,
    ) -> AuthorizedIntegration {
        AuthorizedIntegration::from_verified_grant_controlled(
            identity,
            "refs/heads/integration".into(),
            self.base.clone(),
            self.candidate.clone(),
            "owner:generated-controlled-preparation".into(),
            1_700_000_000,
            control,
        )
        .unwrap()
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    directory: bool,
    mode: u32,
    bytes: Vec<u8>,
}
fn files(root: &Path) -> BTreeMap<PathBuf, Entry> {
    fn walk(root: &Path, current: &Path, result: &mut BTreeMap<PathBuf, Entry>) {
        let mut children: Vec<_> = fs::read_dir(current)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        children.sort();
        for path in children {
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "own Git fixture unexpectedly contains a symlink"
            );
            assert!(metadata.is_dir() || metadata.is_file());
            let directory = metadata.is_dir();
            result.insert(
                path.strip_prefix(root).unwrap().to_path_buf(),
                Entry {
                    directory,
                    mode: metadata.permissions().mode() & 0o7777,
                    bytes: if directory {
                        Vec::new()
                    } else {
                        fs::read(&path).unwrap()
                    },
                },
            );
            if directory {
                walk(root, &path, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}
#[derive(Debug, PartialEq, Eq)]
struct UserState {
    files: BTreeMap<PathBuf, Entry>,
    index: Vec<u8>,
    head_file: Vec<u8>,
    head: Vec<u8>,
    refs: Vec<u8>,
    ref_files: BTreeMap<PathBuf, Entry>,
}
fn user_state(source: &Path) -> UserState {
    let mut worktree = files(source);
    // Objects and owned detached-worktree registration can legitimately grow;
    // original user paths/index/HEAD/branch refs must not change.
    worktree.retain(|path, _| !path.starts_with(".git"));
    UserState {
        files: worktree,
        index: fs::read(source.join(".git/index")).unwrap(),
        head_file: fs::read(source.join(".git/HEAD")).unwrap(),
        head: git(source, &["rev-parse", "HEAD"]),
        refs: git(
            source,
            &["for-each-ref", "--format=%(refname) %(objectname)"],
        ),
        ref_files: files(&source.join(".git/refs")),
    }
}
fn counters(control: &AdoptionControl<'_>) -> Value {
    serde_json::to_value(control.preparation_io()).unwrap()
}
fn no_elapsed(mut value: Value) -> Value {
    assert!(value
        .as_object_mut()
        .unwrap()
        .remove("elapsed_ms")
        .is_some());
    value
}
fn no_git_started(io: &ControlledPreparationIo) {
    let view = serde_json::to_value(io).unwrap();
    assert_eq!(view["git_calls_attempted"].as_u64(), Some(0));
    assert_eq!(view["git_calls_complete"].as_u64(), Some(0));
    assert_eq!(io.git_stdout_observed_bytes(), None);
    assert_eq!(io.git_stderr_observed_bytes(), None);
    assert!(!io.effects_may_have_started());
    // io_complete can be vacuously true for zero attempts. It is neither a
    // started/joined process nor authorization. Never convert None to Some(0).
}
fn unchanged_git_cost(before: &Value, after: &Value) {
    assert_eq!(before["git_calls_attempted"], after["git_calls_attempted"]);
    assert_eq!(before["git_calls_complete"], after["git_calls_complete"]);
    assert_eq!(
        before["git_stdout_observed_bytes"],
        after["git_stdout_observed_bytes"]
    );
    assert_eq!(
        before["git_stderr_observed_bytes"],
        after["git_stderr_observed_bytes"]
    );
    assert!(after["elapsed_ms"].as_u64().unwrap() >= before["elapsed_ms"].as_u64().unwrap());
}

#[test]
fn controlled_isolation_and_candidate_preserve_dirty_original_index_and_refs() {
    let repo = Repository::new();
    fs::write(repo.source.join("file.txt"), b"user staged\n").unwrap();
    fs::write(repo.source.join("staged.txt"), b"staged new file\n").unwrap();
    git(&repo.source, &["add", "file.txt", "staged.txt"]);
    fs::write(repo.source.join("file.txt"), b"user dirty after staging\n").unwrap();
    fs::write(repo.source.join("untracked.txt"), b"user untracked\n").unwrap();
    let before = user_state(&repo.source);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let control = AdoptionControl::for_request(&cancel, deadline);
    let execution = create_isolated_controlled(
        &repo.source,
        &repo.managed,
        "execution",
        &repo.candidate,
        &control,
    )
    .unwrap();
    assert_eq!(
        fs::read(execution.canonical_root.join("file.txt")).unwrap(),
        b"candidate\n"
    );
    let authorization = repo.authorize(&execution, &control);
    let constructed = counters(&control);
    assert!(constructed["git_calls_attempted"].as_u64().unwrap() > 0);
    let narrowed = control.narrowed(deadline - Duration::from_millis(1));
    let candidate =
        prepare_candidate_controlled(authorization, &repo.managed, "prepared", &narrowed).unwrap();
    assert_ne!(candidate.workspace().canonical_root, repo.source);
    assert!(candidate
        .workspace()
        .canonical_root
        .starts_with(&repo.managed));
    assert_eq!(candidate.expected_target_head(), repo.base);
    assert_eq!(
        text(
            &candidate.workspace().canonical_root,
            &["rev-parse", "HEAD"]
        ),
        candidate.candidate_commit()
    );
    assert_eq!(
        fs::read(candidate.workspace().canonical_root.join("file.txt")).unwrap(),
        b"candidate\n"
    );
    assert_eq!(
        text(&repo.source, &["rev-parse", "refs/heads/integration"]),
        repo.base
    );
    assert_eq!(before, user_state(&repo.source));
    let final_io = control.preparation_io();
    assert!(final_io.io_complete());
    assert!(final_io.effects_may_have_started());
    assert!(!final_io.interrupted());
    assert!(final_io.git_stdout_observed_bytes().is_some());
    assert!(final_io.git_stderr_observed_bytes().is_some());
    let after = counters(&control);
    assert!(
        after["git_calls_attempted"].as_u64().unwrap()
            > constructed["git_calls_attempted"].as_u64().unwrap()
    );
    assert_eq!(no_elapsed(after), no_elapsed(counters(&narrowed)));
    // No validation, acceptance, update-ref/CAS, push or task permission is
    // inferred from PreparedCandidate or its complete preparation IO.
}

fn refuse_before_io(cancelled: bool) {
    let repo = Repository::new();
    let identity = inspect(&repo.source).unwrap();
    let before = files(&repo.root);
    let cancel = AtomicBool::new(cancelled);
    let deadline = if cancelled {
        Instant::now() + Duration::from_secs(15)
    } else {
        Instant::now() - Duration::from_millis(1)
    };
    let create_control = AdoptionControl::for_request(&cancel, deadline);
    assert!(matches!(
        create_isolated_controlled(
            &repo.source,
            &repo.managed,
            "must-not-exist",
            &repo.candidate,
            &create_control
        ),
        Err(WorkspaceError::ResourceLimit)
    ));
    no_git_started(&create_control.preparation_io());
    assert!(create_control.preparation_io().interrupted());
    assert_eq!(before, files(&repo.root));
    let constructor_control = AdoptionControl::for_request(&cancel, deadline);
    assert!(matches!(
        AuthorizedIntegration::from_verified_grant_controlled(
            &identity,
            "refs/heads/integration".into(),
            repo.base.clone(),
            repo.candidate.clone(),
            "owner:generated-controlled-preparation".into(),
            1_700_000_000,
            &constructor_control,
        ),
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
    ));
    no_git_started(&constructor_control.preparation_io());
    assert!(constructor_control.preparation_io().interrupted());
    assert_eq!(before, files(&repo.root));
}
#[test]
fn already_expired_controls_refuse_create_and_constructor_without_git_or_writes() {
    refuse_before_io(false);
}
#[test]
fn already_cancelled_controls_refuse_create_and_constructor_without_git_or_writes() {
    refuse_before_io(true);
}

#[test]
fn original_and_narrowed_controls_keep_cancellation_and_constructor_cost() {
    for use_narrowed in [false, true] {
        let repo = Repository::new();
        let identity = inspect(&repo.source).unwrap();
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(15);
        let control = AdoptionControl::for_request(&cancel, deadline);
        let authorization = repo.authorize(&identity, &control);
        let before = files(&repo.root);
        let cost = counters(&control);
        assert!(cost["git_calls_attempted"].as_u64().unwrap() > 0);
        let narrowed = control.narrowed(deadline - Duration::from_millis(1));
        cancel.store(true, Ordering::Release);
        let consumed = if use_narrowed { &narrowed } else { &control };
        assert!(matches!(
            prepare_candidate_controlled(authorization, &repo.managed, "cancelled", consumed),
            Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
        ));
        assert_eq!(before, files(&repo.root));
        unchanged_git_cost(&cost, &counters(&control));
        assert!(control.preparation_io().interrupted());
        assert!(!control.preparation_io().effects_may_have_started());
        assert_eq!(
            no_elapsed(counters(&control)),
            no_elapsed(counters(&narrowed))
        );
    }
}

#[test]
fn constructor_cancel_epoch_cannot_be_replaced_by_fresh_false_flag_at_same_deadline() {
    let repo = Repository::new();
    let original_cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let original = AdoptionControl::for_request(&original_cancel, deadline);
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &original);
    let before = files(&repo.root);
    let cost = counters(&original);
    original_cancel.store(true, Ordering::Release);
    let replacement_cancel = AtomicBool::new(false);
    let replacement = AdoptionControl::for_request(&replacement_cancel, deadline);
    assert!(matches!(
        prepare_candidate_controlled(authorization, &repo.managed, "changed-cancel", &replacement),
        Err(AdoptionError::InvalidGrant)
    ));
    assert_eq!(before, files(&repo.root));
    unchanged_git_cost(&cost, &counters(&original));
    no_git_started(&replacement.preparation_io());
}
#[test]
fn same_cancel_flag_and_deadline_cannot_replace_original_metrics_epoch() {
    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let original = AdoptionControl::for_request(&cancel, deadline);
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &original);
    let before = files(&repo.root);
    let cost = counters(&original);
    assert!(cost["git_calls_attempted"].as_u64().unwrap() > 0);
    let replacement = AdoptionControl::for_request(&cancel, deadline);
    no_git_started(&replacement.preparation_io());
    assert!(matches!(
        prepare_candidate_controlled(authorization, &repo.managed, "changed-cost", &replacement),
        Err(AdoptionError::InvalidGrant)
    ));
    assert_eq!(before, files(&repo.root));
    unchanged_git_cost(&cost, &counters(&original));
    no_git_started(&replacement.preparation_io());
}
#[test]
fn fresh_later_deadline_cannot_renew_constructor_budget() {
    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let original = AdoptionControl::for_request(&cancel, deadline);
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &original);
    let before = files(&repo.root);
    let replacement = AdoptionControl::for_request(&cancel, deadline + Duration::from_secs(1));
    assert!(matches!(
        prepare_candidate_controlled(authorization, &repo.managed, "renewed", &replacement),
        Err(AdoptionError::InvalidGrant)
    ));
    assert_eq!(before, files(&repo.root));
    no_git_started(&replacement.preparation_io());
}
#[test]
fn narrowed_past_deadline_preserves_constructor_cost_and_refuses_private_writes() {
    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &control);
    let before = files(&repo.root);
    let cost = counters(&control);
    let expired = control.narrowed(Instant::now() - Duration::from_millis(1));
    assert!(matches!(
        prepare_candidate_controlled(authorization, &repo.managed, "expired", &expired),
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
    ));
    assert_eq!(before, files(&repo.root));
    unchanged_git_cost(&cost, &counters(&control));
    assert!(control.preparation_io().interrupted());
    assert_eq!(
        no_elapsed(counters(&control)),
        no_elapsed(counters(&expired))
    );
}
#[test]
fn attempted_later_narrowing_cannot_refresh_the_expired_original_deadline() {
    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let control = AdoptionControl::for_request(&cancel, deadline);
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &control);
    let before = files(&repo.root);
    let cost = counters(&control);
    let attempted_renewal = control.narrowed(deadline + Duration::from_secs(1));
    // Wait for this known original deadline, not an assumed Git latency or a
    // large-file race. The maximum single wait is the unchanged original15s.
    while Instant::now() < deadline {
        std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
    }
    assert!(matches!(
        prepare_candidate_controlled(
            authorization,
            &repo.managed,
            "past-original",
            &attempted_renewal
        ),
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
    ));
    assert_eq!(before, files(&repo.root));
    unchanged_git_cost(&cost, &counters(&control));
    assert!(control.preparation_io().interrupted());
}

#[test]
fn controlled_preparation_keeps_dirty_and_advanced_target_refusals() {
    for case in ["dirty", "advanced"] {
        let repo = Repository::new();
        let cancel = AtomicBool::new(false);
        let control =
            AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
        let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &control);
        if case == "dirty" {
            fs::write(repo.source.join("file.txt"), b"new user edit\n").unwrap();
            git(&repo.source, &["add", "file.txt"]);
            fs::write(repo.source.join("untracked.txt"), b"untracked user edit\n").unwrap();
        } else {
            git(
                &repo.source,
                &[
                    "update-ref",
                    "refs/heads/integration",
                    &repo.candidate,
                    &repo.base,
                ],
            );
        }
        let before = files(&repo.root);
        let error =
            match prepare_candidate_controlled(authorization, &repo.managed, "forbidden", &control)
            {
                Ok(_) => panic!("dirty or stale target must not yield a candidate"),
                Err(error) => error,
            };
        if case == "dirty" {
            assert!(matches!(
                error,
                AdoptionError::Workspace(WorkspaceError::Dirty)
            ));
        } else {
            assert!(matches!(error, AdoptionError::StaleHead));
        }
        assert_eq!(before, files(&repo.root));
        assert!(!control.preparation_io().effects_may_have_started());
    }
}
#[test]
fn controlled_paths_keep_merge_driver_and_checkout_filter_guards() {
    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    let authorization = repo.authorize(&inspect(&repo.source).unwrap(), &control);
    let marker = repo.managed.join("must-not-execute");
    git(
        &repo.source,
        &[
            "config",
            "merge.fixture.driver",
            &format!("touch {}", marker.display()),
        ],
    );
    let before = files(&repo.root);
    assert!(matches!(
        prepare_candidate_controlled(authorization, &repo.managed, "unsafe-merge", &control),
        Err(AdoptionError::UnsafeMergeDriver)
    ));
    assert!(!marker.exists());
    assert_eq!(before, files(&repo.root));
    assert!(!control.preparation_io().effects_may_have_started());
    let repo = Repository::new();
    git(&repo.source, &["config", "filter.fixture.clean", "cat"]);
    let before = files(&repo.root);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert!(matches!(
        create_isolated_controlled(
            &repo.source,
            &repo.managed,
            "unsafe-filter",
            &repo.candidate,
            &control
        ),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert_eq!(before, files(&repo.root));
    assert!(!control.preparation_io().effects_may_have_started());
}
#[test]
fn default_uncontrolled_wrappers_keep_existing_candidate_semantics() {
    let repo = Repository::new();
    let before = user_state(&repo.source);
    let authorization = AuthorizedIntegration::from_verified_grant(
        &inspect(&repo.source).unwrap(),
        "refs/heads/integration".into(),
        repo.base.clone(),
        repo.candidate.clone(),
        "owner:generated-legacy-preparation".into(),
        1_700_000_000,
    )
    .unwrap();
    let candidate = prepare_candidate(authorization, &repo.managed, "legacy").unwrap();
    assert_eq!(
        fs::read(candidate.workspace().canonical_root.join("file.txt")).unwrap(),
        b"candidate\n"
    );
    assert_eq!(candidate.expected_target_head(), repo.base);
    assert_eq!(
        text(&repo.source, &["rev-parse", "refs/heads/integration"]),
        repo.base
    );
    assert_eq!(before, user_state(&repo.source));
}

fn selected_artifact(repo: &Repository, id: &str, path: &str, expected: &[u8]) -> BoundArtifact {
    BoundArtifact::from_approved_root(
        &repo.source,
        ArtifactRequirement {
            artifact_id: id.into(),
            relative_path: path.into(),
            max_bytes: 1024,
            expected_sha256: Some(sha256(expected)),
        },
    )
    .unwrap()
}
fn source_authorization(
    repo: &Repository,
    base: &str,
    selected: Vec<BoundArtifact>,
    control: &AdoptionControl<'_>,
) -> AuthorizedSourceCapture {
    AuthorizedSourceCapture::from_verified_selection_controlled(
        &inspect(&repo.source).unwrap(),
        base.into(),
        selected,
        1_700_000_000,
        "accepted-selection:generated".into(),
        "independent-owner:generated".into(),
        control,
    )
    .unwrap()
}

#[test]
fn controlled_source_capture_has_exact_selected_tree_and_preserves_all_user_state() {
    let repo = Repository::new();
    fs::write(repo.source.join("unselected.txt"), b"base unselected\n").unwrap();
    git(&repo.source, &["add", "unselected.txt"]);
    git(
        &repo.source,
        &["commit", "-qm", "generated unselected base"],
    );
    let base = text(&repo.source, &["rev-parse", "HEAD"]);
    fs::write(repo.source.join("file.txt"), b"accepted selected\n").unwrap();
    fs::write(repo.source.join("new-selected.txt"), b"accepted new\n").unwrap();
    fs::write(repo.source.join("unselected.txt"), b"user staged\n").unwrap();
    git(&repo.source, &["add", "unselected.txt"]);
    fs::write(
        repo.source.join("unselected.txt"),
        b"user dirty after staging\n",
    )
    .unwrap();
    fs::write(repo.source.join("not-selected.txt"), b"user untracked\n").unwrap();
    let before = user_state(&repo.source);
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let control = AdoptionControl::for_request(&cancel, deadline);
    let authorization = source_authorization(
        &repo,
        &base,
        vec![
            selected_artifact(&repo, "accepted-file", "file.txt", b"accepted selected\n"),
            selected_artifact(&repo, "accepted-new", "new-selected.txt", b"accepted new\n"),
        ],
        &control,
    );
    let constructed = counters(&control);
    let narrowed = control.narrowed(deadline - Duration::from_millis(1));
    let captured = capture_source_commit_controlled(authorization, &narrowed).unwrap();
    assert_eq!(captured.base(), base);
    assert_eq!(captured.selected().len(), 2);
    for (id, path, expected) in [
        (
            "accepted-file",
            "file.txt",
            b"accepted selected\n".as_slice(),
        ),
        (
            "accepted-new",
            "new-selected.txt",
            b"accepted new\n".as_slice(),
        ),
    ] {
        let actual = git(
            &repo.source,
            &["cat-file", "blob", &format!("{}:{path}", captured.commit())],
        );
        assert_eq!(actual, expected);
        let selected = captured
            .selected()
            .iter()
            .find(|item| item.artifact_id == id)
            .unwrap();
        assert_eq!(selected.relative_path, Path::new(path));
        assert_eq!(selected.sha256, sha256(expected));
        assert_eq!(selected.bytes, expected.len() as u64);
        assert_eq!(selected.mode, "100644");
    }
    assert_eq!(
        git(
            &repo.source,
            &[
                "cat-file",
                "blob",
                &format!("{}:unselected.txt", captured.commit())
            ]
        ),
        b"base unselected\n"
    );
    assert_eq!(
        text(
            &repo.source,
            &["ls-tree", "-r", "--name-only", captured.commit()]
        ),
        "file.txt\nnew-selected.txt\nunselected.txt"
    );
    assert_eq!(
        text(
            &repo.source,
            &["show", "-s", "--format=%P", captured.commit()]
        ),
        base
    );
    assert_eq!(
        text(
            &repo.source,
            &["rev-parse", &format!("{}^{{tree}}", captured.commit())]
        ),
        captured.tree()
    );
    assert_eq!(before, user_state(&repo.source));
    let after = counters(&control);
    assert!(
        after["git_calls_attempted"].as_u64().unwrap()
            > constructed["git_calls_attempted"].as_u64().unwrap()
    );
    assert_eq!(no_elapsed(after), no_elapsed(counters(&narrowed)));
    assert!(control.preparation_io().effects_may_have_started());
    assert!(control.preparation_io().io_complete());
    // This raw commit/selection digest is still no correctness, acceptance,
    // independent integration-policy approval, ref CAS or remote authority.
}

#[test]
fn controlled_source_capture_precancelled_construction_and_consume_never_write() {
    let repo = Repository::new();
    let identity = inspect(&repo.source).unwrap();
    let artifact = selected_artifact(&repo, "selected", "file.txt", b"candidate\n");
    let before = files(&repo.root);
    let cancel = AtomicBool::new(true);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert!(matches!(
        AuthorizedSourceCapture::from_verified_selection_controlled(
            &identity,
            repo.candidate.clone(),
            vec![artifact],
            1_700_000_000,
            "accepted-selection:generated".into(),
            "independent-owner:generated".into(),
            &control,
        ),
        Err(WorkspaceError::ResourceLimit)
    ));
    assert_eq!(before, files(&repo.root));
    no_git_started(&control.preparation_io());
    assert!(control.preparation_io().interrupted());

    let repo = Repository::new();
    let cancel = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(15);
    let original = AdoptionControl::for_request(&cancel, deadline);
    let authorization = source_authorization(
        &repo,
        &repo.candidate,
        vec![selected_artifact(
            &repo,
            "selected",
            "file.txt",
            b"candidate\n",
        )],
        &original,
    );
    let before = files(&repo.root);
    let cost = counters(&original);
    let narrowed = original.narrowed(deadline - Duration::from_millis(1));
    cancel.store(true, Ordering::Release);
    assert!(matches!(
        capture_source_commit_controlled(authorization, &narrowed),
        Err(WorkspaceError::ResourceLimit)
    ));
    assert_eq!(before, files(&repo.root));
    unchanged_git_cost(&cost, &counters(&original));
    assert_eq!(
        no_elapsed(counters(&original)),
        no_elapsed(counters(&narrowed))
    );
    assert!(original.preparation_io().interrupted());
    assert!(!original.preparation_io().effects_may_have_started());
}

#[test]
fn controlled_source_capture_epoch_or_legacy_consumer_cannot_reset_original_control() {
    for case in ["new-cancel", "new-metrics", "legacy-consumer"] {
        let repo = Repository::new();
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(15);
        let original = AdoptionControl::for_request(&cancel, deadline);
        let authorization = source_authorization(
            &repo,
            &repo.candidate,
            vec![selected_artifact(
                &repo,
                "selected",
                "file.txt",
                b"candidate\n",
            )],
            &original,
        );
        let before = files(&repo.root);
        let cost = counters(&original);
        if case == "legacy-consumer" {
            assert!(matches!(
                capture_source_commit(authorization),
                Err(WorkspaceError::InvalidPath)
            ));
        } else {
            let foreign_cancel = AtomicBool::new(false);
            if case == "new-cancel" {
                cancel.store(true, Ordering::Release);
            }
            let replacement = AdoptionControl::for_request(
                if case == "new-cancel" {
                    &foreign_cancel
                } else {
                    &cancel
                },
                deadline,
            );
            assert!(matches!(
                capture_source_commit_controlled(authorization, &replacement),
                Err(WorkspaceError::InvalidPath)
            ));
            no_git_started(&replacement.preparation_io());
        }
        assert_eq!(before, files(&repo.root));
        unchanged_git_cost(&cost, &counters(&original));
        assert!(!original.preparation_io().effects_may_have_started());
    }
}
