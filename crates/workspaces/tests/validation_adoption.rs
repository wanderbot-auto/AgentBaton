use agentbaton_workspaces::{adoption::*, inspect, validation::*, WorkspaceError};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
};
fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Git fixture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}
struct Repository {
    _temp: tempfile::TempDir,
    root: PathBuf,
    managed: tempfile::TempDir,
    base: String,
    source: String,
}
impl Repository {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        git(&root, &["init", "-b", "source"]);
        git(&root, &["config", "user.name", "Fixture"]);
        git(&root, &["config", "user.email", "fixture@example.invalid"]);
        fs::write(root.join("file.txt"), b"baseline\n").unwrap();
        git(&root, &["add", "file.txt"]);
        git(&root, &["commit", "-m", "baseline"]);
        let base = git(&root, &["rev-parse", "HEAD"]);
        git(&root, &["branch", "integration"]);
        fs::write(root.join("file.txt"), b"candidate\n").unwrap();
        git(&root, &["commit", "-am", "candidate"]);
        let source = git(&root, &["rev-parse", "HEAD"]);
        Self {
            _temp: temp,
            root,
            managed: tempfile::tempdir().unwrap(),
            base,
            source,
        }
    }
    fn grant(&self, target: &str) -> AuthorizedIntegration {
        AuthorizedIntegration::from_verified_grant(
            &inspect(&self.root).unwrap(),
            target.into(),
            self.base.clone(),
            self.source.clone(),
            "owner:integration-policy-v1".into(),
            1_700_000_000,
        )
        .unwrap()
    }
    fn prepare(&self) -> PreparedCandidate {
        prepare_candidate(
            self.grant("refs/heads/integration"),
            &self.managed.path().canonicalize().unwrap(),
            "validated",
        )
        .unwrap()
    }
}
fn data_subject() -> ValidationSubject {
    ValidationSubject {
        task_id: "task-1".into(),
        attempt_id: "attempt-1".into(),
        spec_revision: 1,
        input_revision: "input-revision-1".into(),
        version: VersionBinding::Data {
            dataset_revision: "dataset-v1".into(),
        },
    }
}
fn artifact(root: &Path, id: &str, path: &str, expected: Option<String>) -> BoundArtifact {
    BoundArtifact::from_approved_root(
        root,
        ArtifactRequirement {
            artifact_id: id.into(),
            relative_path: path.into(),
            max_bytes: 4096,
            expected_sha256: expected,
        },
    )
    .unwrap()
}
fn rule(
    subject: ValidationSubject,
    inputs: &[BoundArtifact],
    outputs: &[BoundArtifact],
    analysis: Option<AnalysisReproducibility>,
) -> ApprovedRule {
    ApprovedRule::from_verified_policy(
        "owner:acceptance-v1".into(),
        "policy:v1".into(),
        subject,
        inputs,
        outputs,
        analysis,
    )
    .unwrap()
}
fn candidate_validation(
    candidate: &PreparedCandidate,
) -> (ApprovedRule, ValidationEvidence, Acceptance) {
    let outputs = vec![artifact(
        &candidate.workspace().canonical_root,
        "code",
        "file.txt",
        None,
    )];
    let approval = rule(
        ValidationSubject {
            version: candidate.version_binding(),
            ..data_subject()
        },
        &[],
        &outputs,
        None,
    );
    let evidence = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    let accepted = evidence.accept(&approval).unwrap();
    (approval, evidence, accepted)
}
#[test]
fn actual_isolated_candidate_validation_acceptance_and_local_ref_cas_preserve_source_index() {
    let repo = Repository::new();
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let head = git(&repo.root, &["rev-parse", "HEAD"]);
    let bytes = fs::read(repo.root.join("file.txt")).unwrap();
    let candidate = repo.prepare();
    assert_ne!(candidate.workspace().canonical_root, repo.root);
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let record = adopt_local_reference(&candidate, &evidence, &acceptance).unwrap();
    assert_eq!(record.state, IntegrationState::ConfirmedLocalRef);
    assert_eq!(
        git(&repo.root, &["rev-parse", "integration"]),
        candidate.candidate_commit()
    );
    assert_eq!(git(&repo.root, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(repo.root.join("file.txt")).unwrap(), bytes);
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::StaleHead)
    ));
}
#[test]
fn dirty_source_or_advanced_target_blocks_candidate_without_changing_user_files() {
    let repo = Repository::new();
    let grant = repo.grant("refs/heads/integration");
    fs::write(repo.root.join("file.txt"), b"user edit\n").unwrap();
    assert!(matches!(
        prepare_candidate(grant, &repo.managed.path().canonicalize().unwrap(), "dirty"),
        Err(AdoptionError::Workspace(WorkspaceError::Dirty))
    ));
    assert_eq!(
        fs::read(repo.root.join("file.txt")).unwrap(),
        b"user edit\n"
    );
    let repo = Repository::new();
    let grant = repo.grant("refs/heads/integration");
    git(
        &repo.root,
        &[
            "update-ref",
            "refs/heads/integration",
            &repo.source,
            &repo.base,
        ],
    );
    assert!(matches!(
        prepare_candidate(grant, &repo.managed.path().canonicalize().unwrap(), "stale"),
        Err(AdoptionError::StaleHead)
    ));
}
#[test]
fn checked_out_target_is_never_moved_even_with_valid_evidence() {
    let repo = Repository::new();
    git(&repo.root, &["branch", "checked", &repo.base]);
    let user = repo.managed.path().join("user-checkout");
    git(
        &repo.root,
        &["worktree", "add", user.to_str().unwrap(), "checked"],
    );
    let candidate = prepare_candidate(
        repo.grant("refs/heads/checked"),
        &repo.managed.path().canonicalize().unwrap(),
        "candidate",
    )
    .unwrap();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let index_path = git(&user, &["rev-parse", "--git-path", "index"]);
    let index = fs::read(&index_path).unwrap();
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::TargetCheckedOut)
    ));
    assert_eq!(git(&user, &["rev-parse", "HEAD"]), repo.base);
    assert_eq!(fs::read(index_path).unwrap(), index);
    assert_eq!(fs::read(user.join("file.txt")).unwrap(), b"baseline\n");
}
#[test]
fn target_advance_or_candidate_drift_invalidates_previously_passed_acceptance() {
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    fs::write(
        candidate.workspace().canonical_root.join("file.txt"),
        b"changed after validation\n",
    )
    .unwrap();
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::Validation(ValidationError::StaleEvidence))
    ));
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    git(
        &repo.root,
        &[
            "update-ref",
            "refs/heads/integration",
            &repo.source,
            &repo.base,
        ],
    );
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::StaleHead)
    ));
}
#[test]
fn custom_merge_drivers_are_rejected_before_any_repository_code_executes() {
    let repo = Repository::new();
    let marker = repo.managed.path().join("executed");
    git(
        &repo.root,
        &[
            "config",
            "merge.evil.driver",
            &format!("touch {}", marker.display()),
        ],
    );
    assert!(matches!(
        prepare_candidate(
            repo.grant("refs/heads/integration"),
            &repo.managed.path().canonicalize().unwrap(),
            "unsafe"
        ),
        Err(AdoptionError::UnsafeMergeDriver)
    ));
    assert!(!marker.exists());
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
}
#[test]
fn missing_or_symlink_deliverable_never_passes_from_agent_claims() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let outputs = vec![artifact(&root, "report", "missing.md", None)];
    let approval = rule(data_subject(), &[], &outputs, None);
    let missing = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    assert!(!missing.view().passed);
    assert_eq!(missing.view().outputs[0].state, CheckState::Missing);
    assert!(matches!(
        missing.accept(&approval),
        Err(ValidationError::ValidationFailed)
    ));
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), b"outside").unwrap();
    symlink(outside.path().join("secret"), root.join("report.md")).unwrap();
    let outputs = vec![artifact(&root, "report", "report.md", None)];
    let approval = rule(data_subject(), &[], &outputs, None);
    let result = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    assert!(!result.view().passed);
    assert_eq!(
        result.view().outputs[0].state,
        CheckState::UnsafeOrUnreadable
    );
    assert!(BoundArtifact::from_approved_root(
        &root,
        ArtifactRequirement {
            artifact_id: "escape".into(),
            relative_path: "../secret".into(),
            max_bytes: 4096,
            expected_sha256: None
        }
    )
    .is_err());
}
#[test]
fn evidence_is_deterministic_and_bound_to_input_output_attempt_rule_versions() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    fs::write(root.join("input"), b"x\n1\n").unwrap();
    fs::write(root.join("report"), b"report").unwrap();
    let inputs = vec![artifact(&root, "input", "input", Some(sha256(b"x\n1\n")))];
    let outputs = vec![artifact(&root, "report", "report", None)];
    let approval = rule(data_subject(), &inputs, &outputs, None);
    let first = validate_artifacts(&approval, inputs.clone(), outputs.clone(), None).unwrap();
    let second = validate_artifacts(&approval, inputs.clone(), outputs.clone(), None).unwrap();
    assert_eq!(first.view().digest, second.view().digest);
    let mut changed = data_subject();
    changed.attempt_id = "attempt-2".into();
    assert!(matches!(
        first.accept(&rule(changed, &inputs, &outputs, None)),
        Err(ValidationError::ApprovalMismatch)
    ));
    fs::write(root.join("input"), b"x\n2\n").unwrap();
    assert!(matches!(
        first.verify_current(),
        Err(ValidationError::StaleEvidence)
    ));
    assert!(first.accept(&approval).is_err());
}
#[test]
fn reproducibility_requires_fixed_inputs_scripts_environment_units_and_sample_scope() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    for (name, bytes) in [
        ("input", b"x".as_slice()),
        ("script", b"print(1)".as_slice()),
        ("environment", b"python=3.12".as_slice()),
    ] {
        fs::write(root.join(name), bytes).unwrap();
    }
    let inputs = vec![artifact(&root, "input", "input", Some(sha256(b"x")))];
    let outputs = vec![
        artifact(&root, "script", "script", None),
        artifact(&root, "environment", "environment", None),
    ];
    let analysis = AnalysisReproducibility {
        script_artifact_id: "script".into(),
        environment_artifact_id: "environment".into(),
        units: BTreeMap::from([("distance".into(), "m".into())]),
        timezone: "UTC".into(),
        seed: Some(7),
        scope: AnalysisScope::Sample {
            rows: 1,
            limitation: "one-row synthetic sample only".into(),
        },
    };
    let approval = rule(data_subject(), &inputs, &outputs, Some(analysis.clone()));
    let evidence = validate_artifacts(
        &approval,
        inputs.clone(),
        outputs.clone(),
        Some(analysis.clone()),
    )
    .unwrap();
    assert!(evidence.view().passed);
    assert_eq!(evidence.view().analysis, Some(analysis.clone()));
    assert!(matches!(
        validate_artifacts(&approval, vec![], outputs.clone(), Some(analysis.clone())),
        Err(ValidationError::ApprovalMismatch)
    ));
    let mut changed = analysis;
    changed.scope = AnalysisScope::Full;
    assert!(matches!(
        validate_artifacts(&approval, inputs, outputs, Some(changed)),
        Err(ValidationError::ApprovalMismatch)
    ));
}
#[test]
fn production_confirmation_cannot_be_replaced_by_development_or_changed_bindings() {
    let subject = ReleaseSubject {
        artifact_sha256: sha256(b"artifact"),
        commit: "a".repeat(40),
        environment_id: "production-eu".into(),
        class: EnvironmentClass::Production,
    };
    let grant = ConfirmedReleaseGrant::from_verified_human_confirmation(
        "human:approval-7".into(),
        subject.clone(),
        200,
    )
    .unwrap();
    assert_eq!(grant.verify(&subject, 100).unwrap(), "human:approval-7");
    for change in 0..4 {
        let mut changed = subject.clone();
        match change {
            0 => changed.artifact_sha256 = sha256(b"changed"),
            1 => changed.commit = "b".repeat(40),
            2 => changed.environment_id = "other".into(),
            _ => changed.class = EnvironmentClass::Development,
        };
        assert!(grant.verify(&changed, 100).is_err());
    }
    assert!(grant.verify(&subject, 200).is_err());
    assert!(plan_development_delivery(
        "owner:dev".into(),
        subject.commit.clone(),
        "refs/heads/development".into(),
        "test".into(),
        EnvironmentClass::Production
    )
    .is_err());
    assert_eq!(
        plan_development_delivery(
            "owner:dev".into(),
            subject.commit,
            "refs/heads/development".into(),
            "test".into(),
            EnvironmentClass::Development
        )
        .unwrap()
        .effect_state,
        DeliveryEffectState::NotDispatched
    );
}
#[test]
fn hardlinked_and_oversized_artifacts_are_not_accepted_as_deliverables() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("source"), b"outside").unwrap();
    fs::hard_link(outside.path().join("source"), root.join("linked")).unwrap();
    let outputs = vec![artifact(&root, "report", "linked", None)];
    let approval = rule(data_subject(), &[], &outputs, None);
    let result = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    assert_eq!(
        result.view().outputs[0].state,
        CheckState::UnsafeOrUnreadable
    );
    assert!(!result.view().passed);
    fs::write(root.join("huge"), vec![b'x'; 4097]).unwrap();
    let outputs = vec![artifact(&root, "report", "huge", None)];
    let approval = rule(data_subject(), &[], &outputs, None);
    let result = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    assert_eq!(result.view().outputs[0].state, CheckState::TooLarge);
    assert!(result.accept(&approval).is_err());
}
#[test]
fn actual_merge_conflict_preserves_original_index_and_content() {
    let repo = Repository::new();
    git(&repo.root, &["switch", "integration"]);
    fs::write(repo.root.join("file.txt"), b"target different edit\n").unwrap();
    git(&repo.root, &["commit", "-am", "target edit"]);
    let target = git(&repo.root, &["rev-parse", "HEAD"]);
    git(&repo.root, &["switch", "source"]);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let grant = AuthorizedIntegration::from_verified_grant(
        &inspect(&repo.root).unwrap(),
        "refs/heads/integration".into(),
        target.clone(),
        repo.source.clone(),
        "owner:merge".into(),
        1_700_000_000,
    )
    .unwrap();
    assert!(matches!(
        prepare_candidate(
            grant,
            &repo.managed.path().canonicalize().unwrap(),
            "conflict"
        ),
        Err(AdoptionError::Conflict)
    ));
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read(repo.root.join("file.txt")).unwrap(),
        b"candidate\n"
    );
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), target);
    assert!(!repo.managed.path().join("conflict").exists());
}
#[test]
fn two_adopters_cannot_overwrite_each_other_after_same_expected_head() {
    let repo = Repository::new();
    let first = repo.prepare();
    fs::write(repo.root.join("file.txt"), b"second candidate\n").unwrap();
    git(&repo.root, &["commit", "-am", "second candidate"]);
    let source = git(&repo.root, &["rev-parse", "HEAD"]);
    let grant = AuthorizedIntegration::from_verified_grant(
        &inspect(&repo.root).unwrap(),
        "refs/heads/integration".into(),
        repo.base.clone(),
        source,
        "owner:second".into(),
        1_700_000_000,
    )
    .unwrap();
    let second = prepare_candidate(
        grant,
        &repo.managed.path().canonicalize().unwrap(),
        "second",
    )
    .unwrap();
    let first_commit = first.candidate_commit().to_owned();
    let second_commit = second.candidate_commit().to_owned();
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let (_, first_evidence, first_acceptance) = candidate_validation(&first);
    let (_, second_evidence, second_acceptance) = candidate_validation(&second);
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let other = barrier.clone();
    let a = std::thread::spawn(move || {
        other.wait();
        adopt_local_reference(&first, &first_evidence, &first_acceptance)
    });
    let b = std::thread::spawn(move || {
        barrier.wait();
        adopt_local_reference(&second, &second_evidence, &second_acceptance)
    });
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result|matches!(result,Ok(record)if record.state==IntegrationState::ConfirmedLocalRef)).count(),1);
    let adopted = git(&repo.root, &["rev-parse", "integration"]);
    assert!(adopted == first_commit || adopted == second_commit);
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read(repo.root.join("file.txt")).unwrap(),
        b"second candidate\n"
    );
}
#[test]
fn retargeted_gitdir_with_same_objects_cannot_mutate_another_repository() {
    let repo = Repository::new();
    let approved = repo.managed.path().join("approved");
    git(
        &repo.root,
        &[
            "worktree",
            "add",
            "--detach",
            approved.to_str().unwrap(),
            &repo.source,
        ],
    );
    let grant = AuthorizedIntegration::from_verified_grant(
        &inspect(&approved).unwrap(),
        "refs/heads/integration".into(),
        repo.base.clone(),
        repo.source.clone(),
        "owner:repository-a".into(),
        1_700_000_000,
    )
    .unwrap();
    let candidate = prepare_candidate(
        grant,
        &repo.managed.path().canonicalize().unwrap(),
        "candidate",
    )
    .unwrap();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let other = tempfile::tempdir().unwrap();
    let b = other.path().join("repository-b");
    git(
        &repo.root,
        &[
            "clone",
            "--no-hardlinks",
            repo.root.to_str().unwrap(),
            b.to_str().unwrap(),
        ],
    );
    git(&b, &["branch", "integration", &repo.base]);
    git(
        &b,
        &[
            "cat-file",
            "-e",
            &format!("{}^{{commit}}", candidate.candidate_commit()),
        ],
    );
    let index_a_path = PathBuf::from(git(&approved, &["rev-parse", "--git-path", "index"]));
    let index_a = fs::read(&index_a_path).unwrap();
    let index_b = fs::read(b.join(".git/index")).unwrap();
    let pointer = fs::read(approved.join(".git")).unwrap();
    fs::write(
        approved.join(".git"),
        format!("gitdir: {}\n", b.join(".git").display()),
    )
    .unwrap();
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::SubjectMismatch)
    ));
    assert_eq!(git(&b, &["rev-parse", "integration"]), repo.base);
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
    assert_eq!(fs::read(index_a_path).unwrap(), index_a);
    assert_eq!(fs::read(b.join(".git/index")).unwrap(), index_b);
    assert_eq!(fs::read(approved.join("file.txt")).unwrap(), b"candidate\n");
    fs::write(approved.join(".git"), pointer).unwrap();
}
fn copy_fixture(source: &Path, target: &Path) {
    fs::create_dir(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        assert!(!entry.file_type().unwrap().is_symlink());
        if entry.file_type().unwrap().is_dir() {
            copy_fixture(&entry.path(), &target.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).unwrap();
        }
    }
}
#[test]
fn replacement_common_git_directory_at_same_path_requires_new_authorization() {
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let backup = repo.managed.path().join("original-metadata");
    let replacement = repo.managed.path().join("replacement-metadata");
    copy_fixture(&repo.root.join(".git"), &replacement);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    fs::rename(repo.root.join(".git"), &backup).unwrap();
    fs::rename(replacement, repo.root.join(".git")).unwrap();
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &acceptance),
        Err(AdoptionError::SubjectMismatch)
    ));
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(backup.join("index")).unwrap(), index);
}
#[test]
fn approved_policy_binds_roots_paths_digests_and_resource_limits_not_only_ids() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let other = tempfile::tempdir().unwrap();
    let other_root = other.path().canonicalize().unwrap();
    for path in [
        root.join("approved"),
        root.join("different-path"),
        other_root.join("approved"),
    ] {
        fs::write(path, b"same bytes").unwrap();
    }
    let outputs = vec![artifact(&root, "report", "approved", None)];
    let approval = rule(data_subject(), &[], &outputs, None);
    let evidence = validate_artifacts(&approval, vec![], outputs.clone(), None).unwrap();
    for replacement in [
        artifact(&root, "report", "different-path", None),
        artifact(&other_root, "report", "approved", None),
        artifact(&root, "report", "approved", Some(sha256(b"same bytes"))),
        BoundArtifact::from_approved_root(
            &root,
            ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "approved".into(),
                max_bytes: 2048,
                expected_sha256: None,
            },
        )
        .unwrap(),
    ] {
        assert!(matches!(
            validate_artifacts(&approval, vec![], vec![replacement.clone()], None),
            Err(ValidationError::ApprovalMismatch)
        ));
        assert!(matches!(
            evidence.accept(&rule(data_subject(), &[], &[replacement], None)),
            Err(ValidationError::ApprovalMismatch)
        ));
    }
    let policy_view = approval.contract_view();
    let reconstructed = BoundArtifact::from_verified_contract(&policy_view.outputs[0]).unwrap();
    assert_eq!(reconstructed.contract_view(), outputs[0].contract_view());
}
#[test]
fn conditional_common_git_config_cannot_hide_executable_drivers_from_a_linked_root() {
    let repo = Repository::new();
    let approved = repo.managed.path().join("approved");
    git(
        &repo.root,
        &[
            "worktree",
            "add",
            "--detach",
            approved.to_str().unwrap(),
            &repo.source,
        ],
    );
    // Pin the legitimate authorization before repository configuration drifts.
    // The shared include guard now rejects inspection itself, while adoption
    // must still independently reject a previously authorized linked root.
    let grant = AuthorizedIntegration::from_verified_grant(
        &inspect(&approved).unwrap(),
        "refs/heads/integration".into(),
        repo.base.clone(),
        repo.source.clone(),
        "owner:conditional".into(),
        1_700_000_000,
    )
    .unwrap();
    let marker = repo.managed.path().join("driver-executed");
    let included = repo.managed.path().join("conditional-config");
    fs::write(
        &included,
        format!(
            "[merge \"generated\"]\n driver = touch {}\n",
            marker.display()
        ),
    )
    .unwrap();
    let condition = format!("includeIf.gitdir:{}.path", repo.root.join(".git").display());
    git(
        &repo.root,
        &["config", &condition, included.to_str().unwrap()],
    );
    let linked = Command::new("git")
        .arg("-C")
        .arg(&approved)
        .args(["config", "--get-regexp", "^merge\\."])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert_eq!(linked.status.code(), Some(1));
    let common = Command::new("git")
        .arg("-C")
        .arg(&approved)
        .arg("--git-dir")
        .arg(repo.root.join(".git"))
        .args(["config", "--get-regexp", "^merge\\."])
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(common.status.success());
    let index_path = git(&approved, &["rev-parse", "--git-path", "index"]);
    let index = fs::read(&index_path).unwrap();
    let source_index = fs::read(repo.root.join(".git/index")).unwrap();
    let source_bytes = fs::read(repo.root.join("file.txt")).unwrap();
    let approved_bytes = fs::read(approved.join("file.txt")).unwrap();
    let config = fs::read(repo.root.join(".git/config")).unwrap();
    let included_bytes = fs::read(&included).unwrap();
    assert!(matches!(
        inspect(&approved),
        Err(WorkspaceError::UnsafeRepository)
    ));
    assert!(matches!(
        prepare_candidate(
            grant,
            &repo.managed.path().canonicalize().unwrap(),
            "conditional"
        ),
        Err(AdoptionError::Workspace(WorkspaceError::UnsafeRepository))
    ));
    assert!(!marker.exists());
    assert_eq!(fs::read(index_path).unwrap(), index);
    assert_eq!(
        fs::read(repo.root.join(".git/index")).unwrap(),
        source_index
    );
    assert_eq!(fs::read(repo.root.join("file.txt")).unwrap(), source_bytes);
    assert_eq!(fs::read(approved.join("file.txt")).unwrap(), approved_bytes);
    assert_eq!(fs::read(repo.root.join(".git/config")).unwrap(), config);
    assert_eq!(fs::read(included).unwrap(), included_bytes);
    assert_eq!(git(&repo.root, &["rev-parse", "HEAD"]), repo.source);
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
    assert!(!repo.managed.path().join("conditional").exists());
}
#[test]
fn workspace_artifact_binding_is_honest_and_cannot_drive_git_adoption() {
    let repo = Repository::new();
    let candidate = repo.prepare();
    let outputs = vec![artifact(
        &candidate.workspace().canonical_root,
        "code",
        "file.txt",
        None,
    )];
    let subject = ValidationSubject {
        version: VersionBinding::WorkspaceArtifacts {
            workspace_id: "workspace-1".into(),
            base_revision: repo.source.clone(),
        },
        ..data_subject()
    };
    let approval = rule(subject, &[], &outputs, None);
    let evidence = validate_artifacts(&approval, vec![], outputs, None).unwrap();
    assert!(evidence.view().passed);
    let integrity = evidence.accept(&approval).unwrap();
    assert!(matches!(
        adopt_local_reference(&candidate, &evidence, &integrity),
        Err(AdoptionError::SubjectMismatch)
    ));
    assert_eq!(git(&repo.root, &["rev-parse", "integration"]), repo.base);
}

#[test]
fn controlled_cas_preexpired_preserves_original_ref_index_and_files() {
    use std::{sync::atomic::AtomicBool, time::Instant};
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let bytes = fs::read(repo.root.join("file.txt")).unwrap();
    let reflog = git(
        &repo.root,
        &[
            "reflog",
            "show",
            "--format=%H %gs",
            "refs/heads/integration",
        ],
    );
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now());
    assert!(matches!(
        adopt_local_reference_controlled(&candidate, &evidence, &acceptance, &control),
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
    ));
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/heads/integration"]),
        repo.base
    );
    assert_eq!(
        git(
            &repo.root,
            &[
                "reflog",
                "show",
                "--format=%H %gs",
                "refs/heads/integration"
            ]
        ),
        reflog
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(repo.root.join("file.txt")).unwrap(), bytes);
}
#[test]
fn controlled_cas_cancelled_before_start_preserves_original_ref() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let cancel = AtomicBool::new(true);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(5));
    assert!(matches!(
        adopt_local_reference_controlled(&candidate, &evidence, &acceptance, &control),
        Err(AdoptionError::Workspace(WorkspaceError::ResourceLimit))
    ));
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/heads/integration"]),
        repo.base
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
}
#[test]
fn controlled_cas_retains_original_custom_merge_driver_refusal() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    git(&repo.root, &["config", "merge.selected.driver", "false"]);
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(5));
    assert!(matches!(
        adopt_local_reference_controlled(&candidate, &evidence, &acceptance, &control),
        Err(AdoptionError::UnsafeMergeDriver)
    ));
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/heads/integration"]),
        repo.base
    );
}

#[test]
fn controlled_actual_cas_records_distinct_complete_io_without_changing_user_files() {
    use std::{
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    let repo = Repository::new();
    let candidate = repo.prepare();
    let (_, evidence, acceptance) = candidate_validation(&candidate);
    let index = fs::read(repo.root.join(".git/index")).unwrap();
    let bytes = fs::read(repo.root.join("file.txt")).unwrap();
    let cancel = AtomicBool::new(false);
    let control = AdoptionControl::for_request(&cancel, Instant::now() + Duration::from_secs(5));
    let result =
        adopt_local_reference_controlled(&candidate, &evidence, &acceptance, &control).unwrap();
    assert_eq!(result.record().state, IntegrationState::ConfirmedLocalRef);
    assert!(result.io().io_complete());
    assert_eq!(result.io().owned_leader_reaped(), Some(true));
    assert_eq!(result.io().read_failed(), Some(false));
    assert!(!result.io().interrupted());
    assert_eq!(
        git(&repo.root, &["rev-parse", "refs/heads/integration"]),
        candidate.candidate_commit()
    );
    assert_eq!(fs::read(repo.root.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read(repo.root.join("file.txt")).unwrap(), bytes);
}
