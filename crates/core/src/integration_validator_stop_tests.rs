//! Real opaque library CommandRun receipts; no JSON/serde stop or PASS token.
//! Trusted Rust approval fixtures, not Native OS/Actor/Provider evidence.
use super::*;
use agentbaton_workspaces::command_validation::{
    run_approved_command_with_deadline, ApprovedCommandRule, CommandMode, CommandRun,
    CommandSettings, RequiredEnforcement, RunReason,
};
use std::{
    collections::BTreeMap,
    os::unix::fs::{MetadataExt, PermissionsExt},
    time::Duration,
};

struct Fixture {
    _directory: tempfile::TempDir,
    root: std::path::PathBuf,
    rule: ApprovedCommandRule,
}
fn artifact(root: &Path, name: &str, id: &str) -> BoundArtifact {
    let bytes = fs::read(root.join(name)).unwrap();
    BoundArtifact::from_approved_root(
        root,
        ArtifactRequirement {
            artifact_id: id.into(),
            relative_path: name.into(),
            max_bytes: bytes.len().max(1) as u64,
            expected_sha256: Some(agentbaton_workspaces::validation::sha256(&bytes)),
        },
    )
    .unwrap()
}
impl Fixture {
    fn new(script: &[u8]) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.join("input"), b"fixed input").unwrap();
        fs::write(root.join("output"), b"fixed output").unwrap();
        fs::write(root.join("script"), script).unwrap();
        let shell = fs::canonicalize("/bin/sh").unwrap();
        let program = artifact(
            shell.parent().unwrap(),
            shell.file_name().unwrap().to_str().unwrap(),
            "fixed-system-shell",
        );
        let metadata = fs::metadata(&root).unwrap();
        let rule = ApprovedCommandRule::from_verified_policy(
            "owner:integration-stop-fixture".into(),
            "definition:fixed".into(),
            ValidationSubject {
                task_id: "task".into(),
                attempt_id: "attempt".into(),
                spec_revision: 1,
                input_revision: "base".into(),
                version: agentbaton_workspaces::validation::VersionBinding::WorkspaceArtifacts {
                    workspace_id: "workspace".into(),
                    base_revision: "base".into(),
                },
            },
            program,
            WorkspaceIdentity {
                canonical_root: root.clone(),
                device: metadata.dev(),
                inode: metadata.ino(),
                kind: WorkspaceKind::Directory,
            },
            vec![artifact(&root, "input", "input")],
            vec![artifact(&root, "output", "output")],
            CommandSettings {
                mode: CommandMode::PosixShStdin {
                    script: artifact(&root, "script", "fixed-script"),
                },
                arguments: vec![],
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 5000,
                cleanup_time_ms: 500,
                stdout_limit: 128,
                stderr_limit: 128,
                require: RequiredEnforcement::default(),
            },
        )
        .unwrap();
        Self {
            _directory: directory,
            root,
            rule,
        }
    }
}
fn observation(run: CommandRun) -> GoalValidatorObservation {
    let view = run.view();
    GoalValidatorObservation {
        view: serde_json::json!(view),
        passed: view.passed,
        elapsed: Duration::from_millis(view.process_and_cleanup_elapsed_ms),
        scope: "approved_deterministic_command_only",
        correctness: "approved_fixed_command_observations_only",
        command: Some(run),
    }
}

#[test]
fn actual_natural_exit_remains_passed_and_known_stopped_without_changing_its_receipt() {
    let f = Fixture::new(b"exit 0\n");
    let run = run_approved_command_with_deadline(
        &f.rule,
        &AtomicBool::new(false),
        Instant::now() + Duration::from_secs(10),
    )
    .unwrap();
    assert!(run.view().passed);
    assert!(run.view().exit_observed_within_budget);
    let before = serde_json::to_vec(&run.view()).unwrap();
    let observed = observation(run);
    assert!(IntegrationPreparedContext::validation_is_stopped(&observed));
    assert!(observed.passed);
    assert_eq!(
        serde_json::to_vec(&observed.command.as_ref().unwrap().view()).unwrap(),
        before
    );
}

#[test]
fn actual_before_spawn_cancel_and_deadline_are_no_child_without_inventing_reap_or_pass() {
    let f = Fixture::new(b"printf forbidden > must-not-start\n");
    for cancelled in [true, false] {
        let cancel = AtomicBool::new(cancelled);
        let deadline = if cancelled {
            Instant::now() + Duration::from_secs(10)
        } else {
            Instant::now()
        };
        let run = run_approved_command_with_deadline(&f.rule, &cancel, deadline).unwrap();
        let view = run.view();
        assert!(!view.process_started);
        assert_eq!(
            view.reason,
            if cancelled {
                RunReason::CancelledBeforeSpawn
            } else {
                RunReason::DeadlineExceeded
            }
        );
        assert!(!view.cleanup.owned_leader_reaped);
        assert!(!view.cleanup_completed_within_budget);
        assert!(!view.exit_observed_within_budget);
        assert!(!view.passed);
        let before = serde_json::to_vec(&view).unwrap();
        let observed = observation(run);
        assert!(IntegrationPreparedContext::validation_is_stopped(&observed));
        assert!(!observed.passed);
        assert_eq!(
            serde_json::to_vec(&observed.command.as_ref().unwrap().view()).unwrap(),
            before
        );
        assert!(!f.root.join("must-not-start").exists());
    }
}

#[cfg(target_os = "linux")]
fn cancelled_owned_stopped_shell(hold_capture_writer: bool) {
    use std::{io::Read, os::fd::OwnedFd, process::Command, sync::atomic::Ordering};
    let f = Fixture::new(b"printf '%s' \"$$\" > child.pid\nkill -STOP \"$$\"\nexit 0\n");
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let rule = f.rule.clone();
    let worker = std::thread::spawn(move || {
        run_approved_command_with_deadline(&rule, &flag, Instant::now() + Duration::from_secs(10))
    });
    let control = (|| -> Option<fs::File> {
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if Instant::now() >= until {
                return None;
            }
            if let Ok(pid) = fs::read_to_string(f.root.join("child.pid")) {
                if !pid.is_empty() && pid.bytes().all(|c| c.is_ascii_digit()) {
                    // Retain this particular /proc directory before probing;
                    // subsequent fd lookup cannot follow a reused numeric PID.
                    if let Ok(directory) = fs::File::open(format!("/proc/{pid}")) {
                        let stat: OwnedFd = rustix::fs::openat(
                            &directory,
                            "stat",
                            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                            rustix::fs::Mode::empty(),
                        )
                        .ok()?;
                        let mut body = String::new();
                        fs::File::from(stat).read_to_string(&mut body).ok()?;
                        let (_, fields) = body.rsplit_once(") ")?;
                        let fields = fields.split_whitespace().collect::<Vec<_>>();
                        if fields.len() >= 2
                            && fields[0] == "T"
                            && fields[1].parse::<u32>().ok() == Some(std::process::id())
                        {
                            let output = Command::new("/bin/ps")
                                .args(["-o", "stat=", "-p", &pid])
                                .output()
                                .ok()?;
                            if output.status.success()
                                && String::from_utf8(output.stdout)
                                    .ok()?
                                    .trim()
                                    .starts_with('T')
                            {
                                if hold_capture_writer {
                                    let fd = rustix::fs::openat(
                                        &directory,
                                        "fd/1",
                                        rustix::fs::OFlags::WRONLY
                                            | rustix::fs::OFlags::NONBLOCK
                                            | rustix::fs::OFlags::CLOEXEC,
                                        rustix::fs::Mode::empty(),
                                    )
                                    .ok()?;
                                    return Some(fs::File::from(fd));
                                }
                                return Some(directory);
                            }
                        }
                    }
                }
            }
            std::thread::yield_now();
        }
    })();
    // Always cancel through the original opaque owner, never signal a claimed
    // PID; always consume the real worker/child even on fixture-control failure.
    cancel.store(true, Ordering::Release);
    let mut run = worker.join().unwrap().unwrap();
    run.stop_and_reap_owned_leader().unwrap();
    assert!(
        control.is_some(),
        "the original leader was not observed stopped under our parent"
    );
    let view = run.view();
    assert!(view.process_started);
    assert!(view.cancellation_observed);
    assert_eq!(view.reason, RunReason::CancelRequested);
    assert!(!view.exit_observed_within_budget);
    assert!(!view.passed);
    assert!(view.cleanup.owned_leader_reaped);
    assert_eq!(view.cleanup.descendant_containment, "not_enforced_unknown");
    let observed = observation(run);
    if hold_capture_writer {
        assert!(!view.cleanup_completed_within_budget);
        assert!(!view.cleanup.all_capture_pipes_observed_eof);
        assert!(!view.stdout.eof_observed);
        assert!(!IntegrationPreparedContext::validation_is_stopped(
            &observed
        ));
    } else {
        assert!(view.cleanup_completed_within_budget);
        assert!(view.cleanup.all_capture_pipes_observed_eof);
        assert!(view.stdout.eof_observed && view.stderr.eof_observed);
        assert!(IntegrationPreparedContext::validation_is_stopped(&observed));
    }
    assert!(!observed.passed);
    drop(control);
}

#[cfg(target_os = "linux")]
#[test]
fn real_cancel_kill_reap_and_eof_is_known_stop_but_never_pass() {
    cancelled_owned_stopped_shell(false);
}

#[cfg(target_os = "linux")]
#[test]
fn real_cancel_reap_with_parent_retained_capture_writer_stays_unknown() {
    cancelled_owned_stopped_shell(true);
}
