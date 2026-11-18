//! Internal trusted-host observations only. These tests never contact a CI
//! service, approve a production rule, reserve resources or dispatch a repair.
use agentbaton_core::{
    execution::ExecutionBinding,
    goal_plan::{AttemptReservation, BudgetLimits},
    integration::{ci_repair::*, IntegrationBudgetUsage, IntegrationSharedUsage},
    policy::{GrantScope, NetworkScope},
    storage::Store,
};

const NOW: i64 = 1_000;

fn cap(attempts: u32, parallel: u32, cost: u64) -> BudgetLimits {
    BudgetLimits {
        attempts,
        parallel,
        runtime_ms: 10_000,
        output_bytes: 10_000,
        children: 0,
        reworks: attempts,
        tokens: Some(10_000),
        cost_microunits: Some(cost),
    }
}

struct InternalHost {
    candidate: CiCandidate,
    source: ApprovedCiSource,
    observation: CiObservation,
    rule: CiRepairRule,
    authority: CiRepairAuthority,
    history: CiRepairHistory,
}

impl InternalHost {
    fn example() -> Self {
        let candidate = CiCandidate {
            project_id: "project-fixture".into(),
            project_revision: 2,
            root_task_id: "root-task-fixture".into(),
            task_id: "child-task-fixture".into(),
            spec_revision: 3,
            integration_id: "integration-fixture".into(),
            integration_revision: 4,
            repository_digest: "a".repeat(64),
            target_ref: "refs/heads/integration".into(),
            commit: "a".repeat(40),
        };
        let source = CiSource {
            service_id: "ci-service-fixture".into(),
            repository_id: "ci-repository-fixture".into(),
            workflow_id: "ci-workflow-fixture".into(),
        };
        let actor = ExecutionBinding {
            connection_id: "connection-fixture".into(),
            environment_id: "environment-fixture".into(),
            identity_ref: "identity-fixture".into(),
            host_id: "local".into(),
            workspace_id: "workspace-fixture".into(),
            config_revision: "config-fixture:1".into(),
            input_revision: candidate.commit.clone(),
            capability_snapshot_id: "capability-fixture".into(),
            policy_revision: "grant-fixture:1".into(),
        };
        let usage = IntegrationBudgetUsage {
            attempts: 1,
            active: 0,
            children: 0,
            runtime_ms: Some(100),
            output_bytes: Some(100),
            tokens: Some(100),
            cost_microunits: Some(100),
        };
        Self {
            source: ApprovedCiSource {
                approval_ref: "owner-ci-repository".into(),
                source: source.clone(),
                current_candidate: candidate.clone(),
                expires_at_ms: 2_000,
            },
            observation: CiObservation {
                candidate: candidate.clone(),
                run: CiRunIdentity {
                    source: source.clone(),
                    run_id: "run-fixture".into(),
                    attempt: 1,
                },
                current_run: CiRunIdentity {
                    source: source.clone(),
                    run_id: "run-fixture".into(),
                    attempt: 1,
                },
                status: CiStatus::Failed,
                observed_at_ms: 950,
                finished_at_ms: Some(900),
                expires_at_ms: 1_100,
                report_ref: Some("report-fixture".into()),
            },
            rule: CiRepairRule {
                id: "ci-rule-fixture".into(),
                revision: 1,
                approval_ref: "owner-ci-repair".into(),
                candidate: candidate.clone(),
                source,
                actor: actor.clone(),
                approved_at_ms: 850,
                expires_at_ms: 1_200,
                revoked: false,
                budget: cap(2, 1, 200),
                reservation: AttemptReservation {
                    runtime_ms: 1_000,
                    output_bytes: 1_000,
                    tokens: Some(50),
                    cost_microunits: Some(50),
                },
            },
            authority: CiRepairAuthority {
                actor,
                scope: GrantScope {
                    workspace_ids: vec!["workspace-fixture".into()],
                    connection_ids: vec!["connection-fixture".into()],
                    host_ids: vec!["local".into()],
                    max_attempts: 8,
                    max_concurrency: 2,
                    max_duration_ms: 5_000,
                    recipients: vec![],
                    network: NetworkScope::Deny,
                    max_depth: 0,
                    max_children: 0,
                },
                root_budget: cap(3, 2, 400),
                project_budget: cap(4, 2, 300),
                shared_usage: IntegrationSharedUsage {
                    root: usage.clone(),
                    project: usage,
                },
            },
            history: CiRepairHistory {
                project_id: candidate.project_id.clone(),
                root_task_id: candidate.root_task_id.clone(),
                entries: vec![],
            },
            candidate,
        }
    }

    fn inputs(&self) -> CiRepairInputs<'_> {
        CiRepairInputs {
            source: Some(&self.source),
            observation: Some(&self.observation),
            rule: Some(&self.rule),
            history: Some(&self.history),
        }
    }

    fn decision(&self) -> CiRepairDecision {
        evaluate_ci_repair(&self.candidate, self.inputs(), Some(&self.authority), NOW)
    }

    fn earlier(&self, progress: CiRepairProgress) -> CiRepairHistoryEntry {
        CiRepairHistoryEntry {
            run: CiRunIdentity {
                run_id: "earlier-run".into(),
                ..self.observation.run.clone()
            },
            input_commit: "b".repeat(40),
            progress,
        }
    }
}

fn blocked(reason: CiRepairBlock) -> CiRepairDecision {
    CiRepairDecision::Blocked { reason }
}

fn key(decision: CiRepairDecision) -> String {
    match decision {
        CiRepairDecision::Eligible {
            deduplication_key, ..
        } => deduplication_key,
        other => panic!("expected advisory eligibility, got {other:?}"),
    }
}

#[test]
fn current_terminal_failure_without_rule_explicitly_requires_continuation() {
    let f = InternalHost::example();
    let decision = evaluate_ci_repair(
        &f.candidate,
        CiRepairInputs {
            rule: None,
            history: None,
            ..f.inputs()
        },
        None,
        NOW,
    );
    assert_eq!(decision, CiRepairDecision::ContinueRequired);
    assert_eq!(
        serde_json::to_value(decision).unwrap()["status"],
        "continue_required"
    );
}

#[test]
fn stable_eligibility_changes_with_ci_attempt_and_does_not_reset_with_policy_revision() {
    let mut f = InternalHost::example();
    let original = key(f.decision());
    assert_eq!(key(f.decision()), original);
    f.rule.revision += 1;
    assert_eq!(key(f.decision()), original);
    f.observation.run.attempt = 2;
    f.observation.current_run.attempt = 2;
    assert_ne!(key(f.decision()), original);
}

#[test]
fn missing_sources_or_observations_and_wrong_repo_workflow_or_commit_are_distinct() {
    let mut f = InternalHost::example();
    for (input, reason) in [
        (
            CiRepairInputs {
                source: None,
                ..f.inputs()
            },
            CiRepairBlock::MissingApprovedSource,
        ),
        (
            CiRepairInputs {
                observation: None,
                ..f.inputs()
            },
            CiRepairBlock::MissingObservation,
        ),
    ] {
        assert_eq!(
            evaluate_ci_repair(&f.candidate, input, Some(&f.authority), NOW),
            blocked(reason)
        );
    }
    for (source, repo, workflow) in [
        (
            "other-service",
            "ci-repository-fixture",
            "ci-workflow-fixture",
        ),
        (
            "ci-service-fixture",
            "other-repository",
            "ci-workflow-fixture",
        ),
        (
            "ci-service-fixture",
            "ci-repository-fixture",
            "other-workflow",
        ),
    ] {
        f.observation.run.source = CiSource {
            service_id: source.into(),
            repository_id: repo.into(),
            workflow_id: workflow.into(),
        };
        assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationMismatch));
    }
    f.observation.run.source = f.source.source.clone();
    f.observation.candidate.commit = "c".repeat(40);
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationMismatch));
    f.observation.candidate = f.candidate.clone();
    f.source.current_candidate.integration_revision += 1;
    assert_eq!(f.decision(), blocked(CiRepairBlock::CandidateNotCurrent));
    f.source.current_candidate = f.candidate.clone();
    f.observation.candidate.project_revision -= 1;
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationMismatch));
}

#[test]
fn fresh_pending_success_cancelled_and_unknown_are_never_repairable() {
    let mut f = InternalHost::example();
    f.observation.report_ref = None;
    for (status, expected) in [
        (CiStatus::Pending, CiRepairDecision::CiPending),
        (CiStatus::Running, CiRepairDecision::CiPending),
        (CiStatus::Passed, CiRepairDecision::CiPassed),
        (CiStatus::Cancelled, CiRepairDecision::CiCancelled),
        (CiStatus::Unknown, CiRepairDecision::CiOutcomeUnknown),
    ] {
        f.observation.status = status;
        assert_eq!(f.decision(), expected);
    }
}

#[test]
fn stale_attempts_and_expired_or_incomplete_evidence_fail_closed() {
    let mut f = InternalHost::example();
    f.observation.current_run.run_id = "newer-workflow-run".into();
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationStale));
    f.observation.current_run.run_id = f.observation.run.run_id.clone();
    f.observation.current_run.attempt = 2;
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationStale));
    f.observation.run.attempt = 3;
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationUnverified));
    f.observation.run.attempt = 2;
    f.observation.expires_at_ms = NOW;
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationStale));
    f.observation.expires_at_ms = NOW + 1;
    f.observation.finished_at_ms = None;
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationUnverified));
    f.observation.finished_at_ms = Some(NOW + 1);
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationUnverified));
    f.observation.finished_at_ms = Some(900);
    f.observation.report_ref = Some("https://untrusted.example/output".into());
    assert_eq!(f.decision(), blocked(CiRepairBlock::ObservationUnverified));
    f.observation.report_ref = Some("report-fixture".into());
    f.source.expires_at_ms = NOW;
    assert_eq!(f.decision(), blocked(CiRepairBlock::InvalidApprovedSource));
}

#[test]
fn policy_requires_unrevoked_explicit_approval_and_exact_live_actor() {
    let mut f = InternalHost::example();
    f.rule.expires_at_ms = NOW;
    assert_eq!(f.decision(), blocked(CiRepairBlock::RuleUnavailable));
    f.rule.expires_at_ms = NOW + 1;
    f.rule.revoked = true;
    assert_eq!(f.decision(), blocked(CiRepairBlock::RuleUnavailable));
    f.rule.revoked = false;
    f.rule.approved_at_ms = 901;
    assert_eq!(f.decision(), blocked(CiRepairBlock::RuleUnavailable));
    f.rule.approved_at_ms = 850;
    f.rule.actor.identity_ref = "different-account".into();
    assert_eq!(f.decision(), blocked(CiRepairBlock::AuthorityUnavailable));
    f.rule.actor = f.authority.actor.clone();
    f.authority.actor.config_revision = "changed-config".into();
    assert_eq!(f.decision(), blocked(CiRepairBlock::AuthorityUnavailable));
}

#[test]
fn history_is_required_and_duplicates_cycles_and_unknown_stop_are_refused() {
    let mut f = InternalHost::example();
    assert_eq!(
        evaluate_ci_repair(
            &f.candidate,
            CiRepairInputs {
                history: None,
                ..f.inputs()
            },
            Some(&f.authority),
            NOW
        ),
        blocked(CiRepairBlock::HistoryUnavailable)
    );
    let states = [
        (CiRepairProgress::Running, CiRepairBlock::RepairInProgress),
        (
            CiRepairProgress::CancelRequested,
            CiRepairBlock::RepairOutcomeUnknown,
        ),
        (
            CiRepairProgress::OutcomeUnknown,
            CiRepairBlock::RepairOutcomeUnknown,
        ),
        (
            CiRepairProgress::Settled {
                produced_commit: None,
                meters: None,
            },
            CiRepairBlock::BudgetUnknown,
        ),
        (
            CiRepairProgress::Settled {
                produced_commit: Some(f.candidate.commit.clone()),
                meters: Some(CiRepairMeters::default()),
            },
            CiRepairBlock::RepairCycle,
        ),
    ];
    for (progress, expected) in states {
        f.history.entries = vec![f.earlier(progress)];
        assert_eq!(f.decision(), blocked(expected));
    }
    f.history.entries[0].run = f.observation.run.clone();
    assert_eq!(f.decision(), blocked(CiRepairBlock::Duplicate));
    f.observation.run.attempt = 2;
    f.observation.current_run.attempt = 2;
    f.history.entries[0].input_commit = f.candidate.commit.clone();
    f.history.entries[0].progress = CiRepairProgress::Settled {
        produced_commit: None,
        meters: Some(CiRepairMeters::default()),
    };
    assert_eq!(f.decision(), blocked(CiRepairBlock::RepairCycle));
}

#[test]
fn limits_use_known_shared_and_repair_usage_without_resetting_budget() {
    let mut f = InternalHost::example();
    f.authority.shared_usage.project.active = f.authority.project_budget.parallel;
    assert_eq!(f.decision(), blocked(CiRepairBlock::Capacity));
    f.authority.shared_usage.project.active = 1;
    assert_eq!(f.decision(), blocked(CiRepairBlock::RepairInProgress));
    f.authority.shared_usage.project.active = 0;
    f.authority.shared_usage.root.attempts = f.authority.root_budget.attempts;
    assert_eq!(f.decision(), blocked(CiRepairBlock::AttemptLimit));
    f.authority.shared_usage.root.attempts = 1;
    f.authority.shared_usage.project.cost_microunits = None;
    assert_eq!(f.decision(), blocked(CiRepairBlock::BudgetUnknown));
    f.authority.shared_usage.project.cost_microunits = Some(u64::MAX);
    assert_eq!(f.decision(), blocked(CiRepairBlock::BudgetExceeded));
    f.authority.shared_usage.project.cost_microunits = Some(251);
    assert_eq!(f.decision(), blocked(CiRepairBlock::BudgetExceeded));
    f.authority.shared_usage.project.cost_microunits = Some(250);
    assert!(matches!(f.decision(), CiRepairDecision::Eligible { .. }));
    f.rule.reservation.cost_microunits = None;
    assert_eq!(f.decision(), blocked(CiRepairBlock::BudgetUnknown));
    f.rule.reservation.cost_microunits = Some(50);
    f.history.entries = vec![f.earlier(CiRepairProgress::Settled {
        produced_commit: None,
        meters: Some(CiRepairMeters {
            cost_microunits: 151,
            ..CiRepairMeters::default()
        }),
    })];
    assert_eq!(f.decision(), blocked(CiRepairBlock::BudgetExceeded));
    f.rule.budget.reworks = 1;
    assert_eq!(f.decision(), blocked(CiRepairBlock::AttemptLimit));
    f.history.entries = vec![f.history.entries[0].clone(); 1001];
    assert_eq!(f.decision(), blocked(CiRepairBlock::HistoryLimit));
}

#[test]
fn real_store_cannot_accept_a_forged_candidate_and_does_not_write_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("baton.sqlite3");
    let store = Store::open(&db).unwrap();
    let independent =
        rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let state = || {
        independent.query_row(
        "SELECT (SELECT count(*) FROM events), (SELECT count(*) FROM integrations), (SELECT count(*) FROM idempotency)",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?)),
    ).unwrap()
    };
    let f = InternalHost::example();
    let before = state();
    assert_eq!(
        store
            .decide_ci_repair(&f.candidate.integration_id, f.inputs())
            .unwrap_err()
            .code,
        "NOT_FOUND"
    );
    assert_eq!(state(), before);
}
