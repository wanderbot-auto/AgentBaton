//! Real private Git contracts and malformed observations for only the fresh
//! isolated-checkout declaration boundary. No Provider, Core or new authority.
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

struct Repository {
    _temporary: tempfile::TempDir,
    source: PathBuf,
    managed: PathBuf,
    config: PathBuf,
    original_config: Vec<u8>,
}
impl Repository {
    fn new() -> Self {
        let temporary = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let root = temporary.path().canonicalize().unwrap();
        let source = root.join("source");
        let managed = root.join("managed");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&managed).unwrap();
        let result = bounded_output(git_command(&source).args([
            "-c",
            "init.templateDir=",
            "init",
            "-q",
            "--initial-branch=source",
        ]))
        .unwrap();
        assert!(result.status.success());
        let config = source.join(".git/config");
        let original_config = fs::read(&config).unwrap();
        Self {
            _temporary: temporary,
            source,
            managed,
            config,
            original_config,
        }
    }
    fn config(&self, extra: &[u8]) -> Vec<u8> {
        let raw = [self.original_config.as_slice(), extra].concat();
        fs::write(&self.config, &raw).unwrap();
        raw
    }
    fn commit(&self) -> String {
        fs::write(self.source.join("file.txt"), b"original source\n").unwrap();
        for args in [vec!["add", "file.txt"], vec!["commit", "-qm", "baseline"]] {
            let result = bounded_output(
                git_command(&self.source)
                    .args([
                        "-c",
                        "user.name=Generated fixture",
                        "-c",
                        "user.email=fixture@example.invalid",
                        "-c",
                        "commit.gpgsign=false",
                    ])
                    .args(args),
            )
            .unwrap();
            assert!(result.status.success());
        }
        git_text(&self.source, &["rev-parse", "--verify", "HEAD"]).unwrap()
    }
}
fn kind(result: Result<(), WorkspaceError>) -> &'static str {
    match result {
        Ok(()) => "safe",
        Err(WorkspaceError::UnsafeRepository) => "unsafe",
        Err(WorkspaceError::NotRepository) => "not_repository",
        Err(WorkspaceError::ResourceLimit) => "limited",
        other => panic!("unexpected private config result: {other:?}"),
    }
}
// Fixed original three-query body; same argv, order and status categories as
// the exact ab4 predecessor. This oracle never performs a checkout or repair.
fn previous_three_query_guard(root: &Path) -> Result<(), WorkspaceError> {
    let includes = bounded_output(git_command(root).args([
        "config",
        "--no-includes",
        "--get-regexp",
        "^include(if\\..*)?\\.path$",
    ]))?;
    if includes.status.success() {
        return Err(WorkspaceError::UnsafeRepository);
    }
    if includes.status.code() != Some(1) {
        return Err(WorkspaceError::NotRepository);
    }
    let partial = bounded_output(git_command(root).args([
        "config",
        "--get-regexp",
        "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
    ]))?;
    if partial.status.success() || partial.status.code() != Some(1) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    let filters = bounded_output(git_command(root).args(["config", "--get-regexp", "^filter\\."]))?;
    if filters.status.success() || filters.status.code() != Some(1) {
        return Err(WorkspaceError::UnsafeRepository);
    }
    Ok(())
}
fn counts(control: &adoption::AdoptionControl<'_>) -> (u64, u64) {
    // Existing exact read-only Serialize; no privacy widening or default0.
    let value = serde_json::to_value(control.preparation_io()).unwrap();
    (
        value["git_calls_attempted"].as_u64().unwrap(),
        value["git_calls_complete"].as_u64().unwrap(),
    )
}
fn observation(raw: &[u8], status: i32) -> Output {
    Output {
        status: std::process::ExitStatus::from_raw(status),
        stdout: raw.to_vec(),
        stderr: vec![],
    }
}

#[test]
fn actual_names_match_original_include_partial_filter_and_case_refusals() {
    let repository = Repository::new();
    for extra in [
        b"".as_slice(),
        b"\n[fixture]\n note = false\n",
        b"\n[include]\n path = /no/such/private/include\n",
        b"\n[IncludeIf \"gitdir:/never/\"]\n PaTh = /no/such/private/include\n",
        b"\n[extensions]\n partialclone = false\n",
        b"\n[REMOTE \"MiXeD\"]\n ProMiSoR = false\n",
        b"\n[Filter \"MiXeD\"]\n PrOcEsS = false\n",
        b"\n[filter \"MiXeD\"]\n clean = false\n[include]\n path = /no/such/include\n",
        b"\n[remote \"\"]\n promisor = false\n",
        b"\n[includeIf \"\"]\n path = /no/such/include\n",
        b"\n[remote]\n promisor = false\n[includeIf]\n path = /no/such/include\n",
    ] {
        let before = repository.config(extra);
        let cancel = AtomicBool::new(false);
        let control = adoption::AdoptionControl::for_request(
            &cancel,
            Instant::now() + Duration::from_secs(15),
        );
        assert_eq!(
            kind(audit_isolated_config(&repository.source, &control)),
            kind(previous_three_query_guard(&repository.source))
        );
        assert_eq!(counts(&control), (1, 1));
        assert!(!control.preparation_io().effects_may_have_started());
        assert_eq!(fs::read(&repository.config).unwrap(), before);
        assert_eq!(fs::read_dir(&repository.managed).unwrap().count(), 0);
    }
}

#[test]
fn actual_duplicate_names_keep_declaration_refusal_even_for_false_values() {
    let repository = Repository::new();
    for (extra, expected) in [
        (
            b"\n[fixture]\n note = first\n note = second\n".as_slice(),
            "safe",
        ),
        (
            b"\n[remote \"Mixed\"]\n promisor = false\n promisor = false\n".as_slice(),
            "unsafe",
        ),
        (
            b"\n[filter \"Mixed\"]\n clean = false\n clean = false\n".as_slice(),
            "unsafe",
        ),
    ] {
        let before = repository.config(extra);
        let cancel = AtomicBool::new(false);
        let control = adoption::AdoptionControl::for_request(
            &cancel,
            Instant::now() + Duration::from_secs(15),
        );
        assert_eq!(
            kind(audit_isolated_config(&repository.source, &control)),
            expected
        );
        assert_eq!(
            kind(previous_three_query_guard(&repository.source)),
            expected
        );
        assert_eq!(counts(&control), (1, 1));
        assert_eq!(fs::read(&repository.config).unwrap(), before);
    }
}

#[test]
fn actual_multiline_value_is_not_parsed_as_a_name_or_executed() {
    let repository = Repository::new();
    let before = repository.config(
        b"\n[fixture]\n note = \"include.path\\nremote.Mixed.promisor\\nfilter.Mixed.clean\"\n",
    );
    let cancel = AtomicBool::new(false);
    let control =
        adoption::AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(audit_isolated_config(&repository.source, &control)),
        "safe"
    );
    assert_eq!(kind(previous_three_query_guard(&repository.source)), "safe");
    assert_eq!(counts(&control), (1, 1));
    assert_eq!(fs::read(&repository.config).unwrap(), before);
    assert_eq!(fs::read_dir(&repository.managed).unwrap().count(), 0);
}

#[test]
fn actual_separate_boundary_observes_new_config_without_a_cache() {
    let repository = Repository::new();
    let cancel = AtomicBool::new(false);
    let control =
        adoption::AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(audit_isolated_config(&repository.source, &control)),
        "safe"
    );
    let before = repository.config(b"\n[filter \"changed\"]\n clean = false\n");
    assert_eq!(
        kind(audit_isolated_config(&repository.source, &control)),
        "unsafe"
    );
    assert_eq!(counts(&control), (2, 2));
    assert_eq!(fs::read(&repository.config).unwrap(), before);
}

#[test]
fn actual_malformed_config_keeps_first_query_failure_priority_and_bytes() {
    let repository = Repository::new();
    let before = repository.config(b"\n[include]\n path = /no/such/include\n[broken\n");
    let cancel = AtomicBool::new(false);
    let control =
        adoption::AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(audit_isolated_config(&repository.source, &control)),
        "not_repository"
    );
    assert_eq!(
        kind(previous_three_query_guard(&repository.source)),
        "not_repository"
    );
    assert_eq!(fs::read(&repository.config).unwrap(), before);
    assert_eq!(counts(&control), (1, 1));
}

#[test]
fn actual_positive_checkout_preserves_source_head_index_and_managed_binding() {
    let repository = Repository::new();
    let head = repository.commit();
    let config = fs::read(&repository.config).unwrap();
    let index = fs::read(repository.source.join(".git/index")).unwrap();
    let cancel = AtomicBool::new(false);
    let control =
        adoption::AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    let identity = create_isolated_controlled(
        &repository.source,
        &repository.managed,
        "reviewed",
        &head,
        &control,
    )
    .unwrap();
    assert_eq!(identity.canonical_root, repository.managed.join("reviewed"));
    assert!(
        matches!(identity.kind, WorkspaceKind::Git { head: ref actual, dirty: false, .. } if actual == &head)
    );
    assert_eq!(
        git_text(&repository.source, &["rev-parse", "HEAD"]).unwrap(),
        head
    );
    assert_eq!(
        fs::read(repository.source.join(".git/index")).unwrap(),
        index
    );
    assert_eq!(fs::read(repository.config).unwrap(), config);
    assert_eq!(
        fs::read(repository.source.join("file.txt")).unwrap(),
        b"original source\n"
    );
    assert_eq!(
        fs::read(identity.canonical_root.join("file.txt")).unwrap(),
        b"original source\n"
    );
    assert!(control.preparation_io().effects_may_have_started());
    assert!(control.preparation_io().io_complete());
    assert!(!control.preparation_io().interrupted());
}

#[test]
fn original_cancel_or_expired_deadline_never_starts_a_config_child() {
    let repository = Repository::new();
    for cancelled in [true, false] {
        let cancel = AtomicBool::new(cancelled);
        let control = adoption::AdoptionControl::for_request(
            &cancel,
            if cancelled {
                Instant::now() + Duration::from_secs(15)
            } else {
                Instant::now()
            },
        );
        assert_eq!(
            kind(audit_isolated_config(&repository.source, &control)),
            "limited"
        );
        assert_eq!(counts(&control), (0, 0));
        assert!(control.preparation_io().interrupted());
        assert!(!control.preparation_io().effects_may_have_started());
    }
}

#[test]
fn actual_cancel_after_spawn_cannot_admit_the_joined_config_observation() {
    let repository = Repository::new();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let _hook = controlled_spawn_hook::install(move |_| signal.store(true, Ordering::Release));
    let control =
        adoption::AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(audit_isolated_config(&repository.source, &control)),
        "limited"
    );
    assert_eq!(counts(&control).0, 1);
    assert!(counts(&control).1 <= 1);
    assert!(control.preparation_io().interrupted());
    assert!(!control.preparation_io().effects_may_have_started());
}

#[test]
fn framing_names_duplicates_and_subsection_bytes_cannot_hide_a_refusal() {
    for raw in [b"core.note\0".as_slice(), b"core.note\0core.note\0", b""] {
        assert_eq!(
            isolated_config_categories(&observation(raw, 0)).unwrap(),
            [false; 3]
        );
    }
    assert_eq!(
        isolated_config_categories(&observation(
            b"includeif..path\0remote..promisor\0filter.Mixed.clean\0",
            0
        ))
        .unwrap(),
        [true; 3]
    );
    assert_eq!(
        isolated_config_categories(&observation(
            b"remote.Mixed\nsub.promisor\0filter.\xff.clean\0",
            0
        ))
        .unwrap(),
        [false, true, true]
    );
    assert_eq!(
        isolated_config_categories(&observation(
            b"remote.promisor\0includeif.path\0x.filter.a\0",
            0
        ))
        .unwrap(),
        [false; 3]
    );
    for raw in [b"core.note".as_slice(), b"\0", b"core.note\0\0"] {
        assert!(matches!(
            isolated_config_categories(&observation(raw, 0)),
            Err(WorkspaceError::UnsafeRepository)
        ));
    }
}

#[test]
fn failed_or_signalled_list_is_not_a_no_match_and_output_bound_stays_fixed() {
    for status in [1 << 8, 2 << 8, 128 << 8, libc::SIGTERM] {
        assert!(matches!(
            isolated_config_categories(&observation(b"include.path\0", status)),
            Err(WorkspaceError::NotRepository)
        ));
    }
    let mut huge = observation(&[], 0);
    huge.stdout = vec![b'x'; 4_194_305];
    assert!(matches!(
        isolated_config_categories(&huge),
        Err(WorkspaceError::ResourceLimit)
    ));
    huge.stdout.clear();
    huge.stderr = vec![b'x'; 4_194_305];
    assert!(matches!(
        isolated_config_categories(&huge),
        Err(WorkspaceError::ResourceLimit)
    ));
}
