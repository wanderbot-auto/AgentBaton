//! Real private Git config reads and a fixed historical query oracle. No
//! external repository, helper execution, changed budget or authority.
use super::*;
use std::{
    process::Command,
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn git(root: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    crate::configure_local_git_environment(&mut command);
    let output = command
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "private Git fixture failed");
}
fn repository() -> (tempfile::TempDir, AuthorizedIntegration) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    git(&root, &["init", "-q", "--initial-branch=source"]);
    fs::write(root.join("file"), "baseline").unwrap();
    git(&root, &["add", "."]);
    git(&root, &["commit", "-qm", "baseline"]);
    git(&root, &["branch", "integration"]);
    let workspace = inspect(&root).unwrap();
    let WorkspaceKind::Git { head, .. } = &workspace.kind else {
        panic!("fixture is not Git")
    };
    let grant = AuthorizedIntegration::from_verified_grant(
        &workspace,
        "refs/heads/integration".into(),
        head.clone(),
        head.clone(),
        "owner:fixed-fixture".into(),
        1_700_000_000,
    )
    .unwrap();
    (temp, grant)
}
fn kind(result: Result<(), AdoptionError>) -> &'static str {
    match result {
        Ok(()) => "safe",
        Err(AdoptionError::UnsafeMergeDriver) => "merge",
        Err(AdoptionError::Workspace(WorkspaceError::UnsafeRepository)) => "unsafe",
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit)) => "limited",
        other => panic!("unexpected audit result: {other:?}"),
    }
}
fn previous_four_query_guard(grant: &AuthorizedIntegration) -> Result<(), AdoptionError> {
    for (pattern, merge) in [
        ("^include(if\\..*)?\\.path$", false),
        ("^merge\\.", true),
        ("^filter\\.", false),
        (
            "^(extensions\\.partialclone|remote\\..*\\.promisor)$",
            false,
        ),
    ] {
        let observed = bound_output(
            grant,
            &["config", "--no-includes", "--get-regexp", pattern],
            None,
        )?;
        if observed.interrupted
            || observed.output.status.success()
            || observed.output.status.code() != Some(1)
        {
            return Err(if merge {
                AdoptionError::UnsafeMergeDriver
            } else {
                WorkspaceError::UnsafeRepository.into()
            });
        }
    }
    Ok(())
}
#[test]
fn one_fresh_read_matches_all_original_query_refusals_and_precedence() {
    for keys in [
        vec![],
        vec!["include.path"],
        vec!["includeIf.gitdir:/never/.path"],
        vec!["merge.fixture.driver"],
        vec!["filter.fixture.process"],
        vec!["extensions.partialclone"],
        vec!["remote.fixture.promisor"],
        vec!["filter.fixture.clean", "merge.fixture.driver"],
        vec!["merge.fixture.driver", "include.path"],
        vec!["remote.fixture.promisor", "filter.fixture.clean"],
    ] {
        let (_temp, grant) = repository();
        for key in &keys {
            git(&grant.root, &["config", key, "false"]);
        }
        let config = fs::read(grant.common_dir.join("config")).unwrap();
        let cancel = AtomicBool::new(false);
        let control =
            AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
        let current = kind(reject_bound_unsafe_config_controlled(
            &grant,
            Some(&control),
        ));
        assert_eq!(
            current,
            kind(previous_four_query_guard(&grant)),
            "keys {keys:?}"
        );
        let facts = control.preparation.lock().unwrap();
        assert_eq!(facts.git_calls_attempted, 1, "keys {keys:?}");
        assert_eq!(facts.git_calls_complete, 1);
        assert!(!facts.effects_may_have_started);
        assert_eq!(config, fs::read(grant.common_dir.join("config")).unwrap());
    }
}
#[test]
fn separate_boundaries_reread_config_and_keep_original_error_priority() {
    let (_temp, grant) = repository();
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(reject_bound_unsafe_config_controlled(
            &grant,
            Some(&control)
        )),
        "safe"
    );
    git(&grant.root, &["config", "merge.fixture.driver", "false"]);
    assert_eq!(
        kind(reject_bound_unsafe_config_controlled(
            &grant,
            Some(&control)
        )),
        "merge"
    );
    git(
        &grant.root,
        &["config", "includeIf.gitdir:/absent/.path", "/no/such/file"],
    );
    assert_eq!(
        kind(reject_bound_unsafe_config_controlled(
            &grant,
            Some(&control)
        )),
        "unsafe"
    );
    assert_eq!(control.preparation.lock().unwrap().git_calls_attempted, 3);
}
#[test]
fn malformed_config_and_original_cancel_or_deadline_never_admit_a_read() {
    let (_temp, grant) = repository();
    fs::write(grant.common_dir.join("config"), b"[malformed\n").unwrap();
    assert_eq!(
        kind(reject_bound_unsafe_config_controlled(&grant, None)),
        kind(previous_four_query_guard(&grant))
    );
    for cancelled in [true, false] {
        let cancel = AtomicBool::new(cancelled);
        let control = AdoptionControl::for_request(
            &cancel,
            if cancelled {
                Instant::now() + Duration::from_secs(15)
            } else {
                Instant::now()
            },
        );
        assert_eq!(
            kind(reject_bound_unsafe_config_controlled(
                &grant,
                Some(&control)
            )),
            "limited"
        );
        assert_eq!(control.preparation.lock().unwrap().git_calls_attempted, 0);
    }
}
#[test]
fn nul_separated_key_subsections_keep_original_meaning() {
    assert_eq!(
        bound_config_categories(b"core.note\0user.name\0").unwrap(),
        [false; 4]
    );
    assert_eq!(bound_config_categories(b"includeif.gitdir:/unused/.path\0merge.Mixed.driver\0filter.Mixed.clean\0remote.Mixed.promisor\0").unwrap(),[true;4]);
    assert_eq!(
        bound_config_categories(b"remote.notpromisor\0x.include.path\0").unwrap(),
        [false; 4]
    );
    assert_eq!(bound_config_categories(b"").unwrap(), [false; 4]);
    assert!(bound_config_categories(b"core.note-missing-final-NUL").is_err());
}
#[test]
fn actual_multiline_values_are_never_mistaken_for_executable_config_keys() {
    let (_temp, grant) = repository();
    git(
        &grant.root,
        &[
            "config",
            "fixture.note",
            "include.path\nmerge.fixture.driver\nfilter.fixture.clean\nremote.fixture.promisor",
        ],
    );
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(15));
    assert_eq!(
        kind(reject_bound_unsafe_config_controlled(
            &grant,
            Some(&control)
        )),
        "safe"
    );
    assert_eq!(kind(previous_four_query_guard(&grant)), "safe");
    assert_eq!(control.preparation.lock().unwrap().git_calls_attempted, 1);
}
#[test]
fn actual_empty_subsection_config_declarations_preserve_the_original_refusal() {
    use std::io::Write;
    for raw in [
        b"\n[remote \"\"]\n promisor = false\n".as_slice(),
        b"\n[includeIf \"\"]\n path = /no/such/file\n".as_slice(),
    ] {
        let (_temp, grant) = repository();
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(grant.common_dir.join("config"))
            .unwrap();
        file.write_all(raw).unwrap();
        file.sync_all().unwrap();
        assert_eq!(
            kind(reject_bound_unsafe_config_controlled(&grant, None)),
            "unsafe"
        );
        assert_eq!(kind(previous_four_query_guard(&grant)), "unsafe");
    }
}
