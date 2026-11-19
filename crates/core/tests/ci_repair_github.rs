//! Read-only response projections and negative controls. No network in tests.
use agentbaton_core::integration::ci_repair::{
    evaluate_ci_repair,
    github::{
        map_observation, LatestCandidateRuns, ObservationError, SelectedAttempt, TrustedRead,
        TrustedReads, SERVICE_ID,
    },
    ApprovedCiSource, CiCandidate, CiObservation, CiRepairDecision, CiRepairInputs, CiSource,
    CiStatus,
};
use serde_json::{json, Value};

const FINISHED: i64 = 1_794_658_737_000;
const NOW: i64 = FINISHED + 10_000;
const EXPIRES: i64 = NOW + 60_000;

#[derive(Clone)]
struct Fixture {
    candidate: CiCandidate,
    approval: ApprovedCiSource,
    requested_run: u64,
    requested_attempt: u32,
    selected: Value,
    latest: Value,
    jobs: Option<Value>,
    fresh_after: i64,
    expires: i64,
    attempt_read_at: i64,
    jobs_read_at: i64,
    latest_read_at: i64,
}

fn observed() -> Value {
    serde_json::from_str(include_str!(
        "../../../docs/execution/ci-repair-hosted-observation/github-observed.json"
    ))
    .unwrap()
}

impl Fixture {
    fn observed_dispatch() -> Self {
        let actual = observed();
        let candidate = CiCandidate {
            project_id: "project".into(),
            project_revision: 1,
            root_task_id: "root".into(),
            task_id: "task".into(),
            spec_revision: 1,
            integration_id: "integration".into(),
            integration_revision: 1,
            repository_digest: "a".repeat(64),
            target_ref: "refs/heads/dev/macmini-agentbaton".into(),
            commit: "e8abd6bbc939bc24cf07ba5793aadfd173313b13".into(),
        };
        let approval = ApprovedCiSource {
            approval_ref: "trusted-fixture".into(),
            source: CiSource {
                service_id: SERVICE_ID.into(),
                repository_id: "1434422493".into(),
                workflow_id: "397305968".into(),
            },
            current_candidate: candidate.clone(),
            expires_at_ms: EXPIRES,
        };
        Self {
            candidate,
            approval,
            requested_run: 43_592_293_854,
            requested_attempt: 1,
            selected: actual["attempt"].clone(),
            latest: actual["latest"].clone(),
            jobs: Some(actual["jobs"].clone()),
            fresh_after: FINISHED - 60_000,
            expires: EXPIRES,
            attempt_read_at: NOW - 3_000,
            jobs_read_at: NOW - 2_000,
            latest_read_at: NOW - 1_000,
        }
    }

    fn map(&self) -> Result<CiObservation, ObservationError> {
        let selected = serde_json::to_vec(&self.selected).unwrap();
        let latest = serde_json::to_vec(&self.latest).unwrap();
        let jobs = self
            .jobs
            .as_ref()
            .map(|value| serde_json::to_vec(value).unwrap());
        self.map_body(&selected, &latest, jobs.as_deref())
    }

    fn map_body(
        &self,
        selected: &[u8],
        latest: &[u8],
        jobs: Option<&[u8]>,
    ) -> Result<CiObservation, ObservationError> {
        map_observation(
            &self.candidate,
            &self.approval,
            self.reads(selected, latest, jobs),
            NOW,
        )
    }

    fn reads<'a>(
        &'a self,
        selected: &'a [u8],
        latest: &'a [u8],
        jobs: Option<&'a [u8]>,
    ) -> TrustedReads<'a> {
        TrustedReads {
            selected_attempt: SelectedAttempt {
                run_id: self.requested_run,
                attempt: self.requested_attempt,
                response: TrustedRead {
                    body: selected,
                    received_at_ms: self.attempt_read_at,
                },
            },
            attempt_jobs: jobs.map(|body| TrustedRead {
                body,
                received_at_ms: self.jobs_read_at,
            }),
            latest_candidate_runs: LatestCandidateRuns {
                source: &self.approval.source,
                target_ref: &self.candidate.target_ref,
                head_sha: &self.candidate.commit,
                response: TrustedRead {
                    body: latest,
                    received_at_ms: self.latest_read_at,
                },
            },
            fresh_after_ms: self.fresh_after,
            expires_at_ms: self.expires,
        }
    }

    fn existing_decision(&self, observation: &CiObservation) -> CiRepairDecision {
        evaluate_ci_repair(
            &self.candidate,
            CiRepairInputs {
                source: Some(&self.approval),
                observation: Some(observation),
                ..Default::default()
            },
            None,
            NOW,
        )
    }

    fn set_status(&mut self, status: &str, conclusion: Value) {
        self.selected["status"] = json!(status);
        self.selected["conclusion"] = conclusion.clone();
        self.latest["workflow_runs"][0]["status"] = json!(status);
        self.latest["workflow_runs"][0]["conclusion"] = conclusion;
    }
}

#[test]
fn real_dispatch_response_binds_the_candidate_and_preserves_advisory_decision() {
    let fixture = Fixture::observed_dispatch();
    let value = fixture.map().unwrap();
    assert_eq!(value.candidate, fixture.candidate);
    assert_eq!(value.run.source, fixture.approval.source);
    assert_eq!(value.run, value.current_run);
    assert_eq!(value.run.run_id, "43592293854");
    assert_eq!(value.run.attempt, 1);
    assert_eq!(value.status, CiStatus::Failed);
    assert_eq!(value.finished_at_ms, Some(FINISHED));
    assert_eq!(value.observed_at_ms, fixture.latest_read_at);
    assert_eq!(value.expires_at_ms, EXPIRES);
    assert_eq!(
        value.report_ref.as_deref(),
        Some("github-actions-job:132439465437")
    );
    assert_eq!(
        fixture.existing_decision(&value),
        CiRepairDecision::ContinueRequired
    );
}

#[test]
fn requires_approval_and_exact_selected_latest_and_job_bindings() {
    for (path, changed) in [
        ("/repository/id", json!(1)),
        ("/head_repository/id", json!(1)),
        ("/workflow_id", json!(1)),
        ("/head_sha", json!("a".repeat(40))),
        ("/head_branch", json!("dev/other")),
        ("/id", json!(0)),
        ("/run_number", json!(0)),
        ("/run_attempt", json!(0)),
    ] {
        let mut fixture = Fixture::observed_dispatch();
        *fixture.selected.pointer_mut(path).unwrap() = changed.clone();
        assert_eq!(
            fixture.map().unwrap_err(),
            ObservationError::BindingMismatch,
            "{path}"
        );
        let mut fixture = Fixture::observed_dispatch();
        *fixture.latest["workflow_runs"][0]
            .pointer_mut(path)
            .unwrap() = changed;
        assert_eq!(
            fixture.map().unwrap_err(),
            ObservationError::BindingMismatch,
            "{path}"
        );
    }
    for (path, changed) in [
        ("/id", json!(0)),
        ("/run_id", json!(42)),
        ("/run_attempt", json!(2)),
        ("/head_branch", json!("dev/other")),
        ("/head_sha", json!("b".repeat(40))),
    ] {
        let mut fixture = Fixture::observed_dispatch();
        *fixture.jobs.as_mut().unwrap()["jobs"][0]
            .pointer_mut(path)
            .unwrap() = changed;
        assert_eq!(
            fixture.map().unwrap_err(),
            ObservationError::BindingMismatch,
            "{path}"
        );
    }
    let mut fixture = Fixture::observed_dispatch();
    fixture.candidate.integration_revision += 1;
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::InvalidApproval
    );
    let mut fixture = Fixture::observed_dispatch();
    fixture.approval.source.service_id = "other-service".into();
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::InvalidApproval
    );
}

#[test]
fn requires_an_independent_query_for_the_approved_full_ref_and_exact_attempt_path() {
    let fixture = Fixture::observed_dispatch();
    let selected = serde_json::to_vec(&fixture.selected).unwrap();
    let latest = serde_json::to_vec(&fixture.latest).unwrap();
    let jobs = serde_json::to_vec(&fixture.jobs).unwrap();
    let expected = fixture.reads(&selected, &latest, Some(&jobs));
    let foreign_source = CiSource {
        workflow_id: "another-workflow".into(),
        ..fixture.approval.source.clone()
    };
    let mut controls = [expected; 5];
    controls[0].latest_candidate_runs.target_ref = "refs/tags/dev/macmini-agentbaton";
    controls[1].latest_candidate_runs.head_sha = "the wrong SHA";
    controls[2].latest_candidate_runs.source = &foreign_source;
    controls[3].selected_attempt.run_id += 1;
    controls[4].selected_attempt.attempt += 1;
    for reads in controls {
        assert_eq!(
            map_observation(&fixture.candidate, &fixture.approval, reads, NOW).unwrap_err(),
            ObservationError::BindingMismatch
        );
    }
}

#[test]
fn the_reported_pr_head_or_merge_event_does_not_certify_candidate_execution() {
    let actual = observed();
    let mut fixture = Fixture::observed_dispatch();
    fixture.selected = actual["pull_request_run"].clone();
    fixture.requested_run = fixture.selected["id"].as_u64().unwrap();
    fixture.latest = json!({"total_count": 1, "workflow_runs": [fixture.selected.clone()]});
    fixture.jobs = Some(json!({"total_count": 1, "jobs": [actual["pull_request_job"].clone()]}));
    fixture.candidate.target_ref = "refs/heads/collab/host-registration-integration".into();
    fixture.candidate.commit = fixture.selected["head_sha"].as_str().unwrap().into();
    fixture.approval.current_candidate = fixture.candidate.clone();
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::UnsupportedEvent
    );
    fixture.candidate.commit = fixture.selected["pull_requests"][0]["head"]["sha"]
        .as_str()
        .unwrap()
        .into();
    fixture.approval.current_candidate = fixture.candidate.clone();
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::BindingMismatch
    );
    fixture.candidate.target_ref = "refs/pull/33/merge".into();
    fixture.approval.current_candidate = fixture.candidate.clone();
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::InvalidApproval
    );

    for event in [
        "pull_request_target",
        "merge_group",
        "workflow_run",
        "schedule",
    ] {
        let mut fixture = Fixture::observed_dispatch();
        fixture.selected["event"] = json!(event);
        assert_eq!(
            fixture.map().unwrap_err(),
            ObservationError::UnsupportedEvent
        );
    }
}

#[test]
fn independent_latest_selection_rejects_truncation_supersession_and_reruns() {
    let mut fixture = Fixture::observed_dispatch();
    let mut older = fixture.selected.clone();
    older["id"] = json!(10);
    older["run_number"] = json!(120);
    fixture.latest["workflow_runs"]
        .as_array_mut()
        .unwrap()
        .insert(0, older);
    fixture.latest["total_count"] = json!(2);
    assert_eq!(fixture.map().unwrap().status, CiStatus::Failed);
    fixture.latest["workflow_runs"][0]["run_number"] = json!(122);
    assert_eq!(fixture.map().unwrap_err(), ObservationError::Superseded);
    fixture.latest["total_count"] = json!(3);
    assert_eq!(fixture.map().unwrap_err(), ObservationError::IncompleteList);

    let mut fixture = Fixture::observed_dispatch();
    fixture.latest["workflow_runs"][0]["conclusion"] = json!("success");
    assert_eq!(fixture.map().unwrap_err(), ObservationError::Superseded);
    fixture.latest["workflow_runs"][0]["run_attempt"] = json!(2);
    assert_eq!(fixture.map().unwrap_err(), ObservationError::Superseded);
    fixture.latest["workflow_runs"][0]["conclusion"] = json!("failure");
    fixture.selected["run_attempt"] = json!(2);
    fixture.requested_attempt = 2;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::Rerun);
}

#[test]
fn only_explicit_completed_failure_can_supply_a_repair_report() {
    for (state, conclusion, expected, decision) in [
        (
            "queued",
            Value::Null,
            CiStatus::Pending,
            CiRepairDecision::CiPending,
        ),
        (
            "in_progress",
            Value::Null,
            CiStatus::Running,
            CiRepairDecision::CiPending,
        ),
        (
            "in_progress",
            json!("failure"),
            CiStatus::Unknown,
            CiRepairDecision::CiOutcomeUnknown,
        ),
        (
            "completed",
            Value::Null,
            CiStatus::Unknown,
            CiRepairDecision::CiOutcomeUnknown,
        ),
        (
            "completed",
            json!("timed_out"),
            CiStatus::Unknown,
            CiRepairDecision::CiOutcomeUnknown,
        ),
        (
            "completed",
            json!("new-conclusion"),
            CiStatus::Unknown,
            CiRepairDecision::CiOutcomeUnknown,
        ),
        (
            "new-state",
            json!("failure"),
            CiStatus::Unknown,
            CiRepairDecision::CiOutcomeUnknown,
        ),
        (
            "completed",
            json!("success"),
            CiStatus::Passed,
            CiRepairDecision::CiPassed,
        ),
        (
            "completed",
            json!("cancelled"),
            CiStatus::Cancelled,
            CiRepairDecision::CiCancelled,
        ),
    ] {
        let mut fixture = Fixture::observed_dispatch();
        fixture.set_status(state, conclusion);
        let result = fixture.map().unwrap();
        assert_eq!(result.status, expected);
        assert!(result.report_ref.is_none());
        assert_eq!(fixture.existing_decision(&result), decision);
    }
    let mut fixture = Fixture::observed_dispatch();
    for job in fixture.jobs.as_mut().unwrap()["jobs"]
        .as_array_mut()
        .unwrap()
    {
        job["conclusion"] = json!("success");
    }
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::UnverifiedCompletion
    );
}

#[test]
fn completion_is_from_verified_jobs_and_must_be_fresh_and_ordered() {
    for completion in [
        Value::Null,
        json!("2026-02-30T12:18:57Z"),
        json!("2026-11-14T03:18:57Z"),
        json!("2026-11-14T12:18:59Z"),
        json!("2026-11-14T12:18:57+00:00"),
    ] {
        let mut fixture = Fixture::observed_dispatch();
        fixture.jobs.as_mut().unwrap()["jobs"][0]["completed_at"] = completion;
        assert_eq!(
            fixture.map().unwrap_err(),
            ObservationError::UnverifiedCompletion
        );
    }
    let mut fixture = Fixture::observed_dispatch();
    fixture.jobs.as_mut().unwrap()["total_count"] = json!(5);
    assert_eq!(fixture.map().unwrap_err(), ObservationError::IncompleteList);
    fixture.jobs = None;
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::UnverifiedCompletion
    );
    let mut fixture = Fixture::observed_dispatch();
    fixture.approval.expires_at_ms = NOW;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);
    let mut fixture = Fixture::observed_dispatch();
    fixture.expires = NOW;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);
    let mut fixture = Fixture::observed_dispatch();
    fixture.fresh_after = FINISHED + 1;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);

    let mut fixture = Fixture::observed_dispatch();
    fixture.fresh_after = FINISHED - 1;
    let jobs = fixture.jobs.as_mut().unwrap();
    jobs["jobs"][0]["completed_at"] = json!("2026-11-14T12:18:56Z");
    jobs["jobs"][2]["completed_at"] = json!("2026-11-14T12:18:57Z");
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);
    let mut fixture = Fixture::observed_dispatch();
    fixture.latest_read_at = NOW - 5_000;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);
    let mut fixture = Fixture::observed_dispatch();
    fixture.latest_read_at = NOW + 1;
    assert_eq!(fixture.map().unwrap_err(), ObservationError::StaleRead);
}

#[test]
fn refuses_unverified_response_bytes_and_duplicate_identities() {
    let fixture = Fixture::observed_dispatch();
    let latest = serde_json::to_vec(&fixture.latest).unwrap();
    let selected = serde_json::to_string(&fixture.selected).unwrap();
    let duplicate_field = selected.replacen('{', r#"{"id":1,"#, 1);
    for body in [b"{}".as_slice(), duplicate_field.as_bytes(), &[b' '; 16]] {
        assert_eq!(
            fixture.map_body(body, &latest, None).unwrap_err(),
            ObservationError::InvalidResponse
        );
    }
    let mut fixture = Fixture::observed_dispatch();
    let duplicate = fixture.latest["workflow_runs"][0].clone();
    fixture.latest = json!({"total_count": 2, "workflow_runs": [duplicate.clone(), duplicate]});
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::InvalidResponse
    );
    let mut fixture = Fixture::observed_dispatch();
    let jobs = fixture.jobs.as_mut().unwrap();
    jobs["jobs"][1]["id"] = jobs["jobs"][0]["id"].clone();
    assert_eq!(
        fixture.map().unwrap_err(),
        ObservationError::BindingMismatch
    );
}
