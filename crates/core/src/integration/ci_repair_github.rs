//! Maps independently read GitHub Actions REST responses into an advisory CI
//! observation. A trusted host actor supplies read context for api.github.com.
use super::{
    authority, git_oid, ApprovedCiSource, CiCandidate, CiObservation, CiRunIdentity, CiSource,
    CiStatus,
};
use serde::{de::DeserializeOwned, Deserialize};
use std::collections::BTreeSet;
use time::{Date, Month};

pub const SERVICE_ID: &str = "github-actions";
const MAX_RESPONSE_BYTES: usize = 4 * 1024 * 1024;
const RUN_SEARCH_CAP: usize = 1_000;

/// Body from a successful GET and its host-recorded receipt time. The actor
/// supplies the endpoint context and uses an approved GitHub installation.
#[derive(Clone, Copy)]
pub struct TrustedRead<'a> {
    pub body: &'a [u8],
    pub received_at_ms: i64,
}

/// The run and attempt in the actor's request path.
#[derive(Clone, Copy)]
pub struct SelectedAttempt<'a> {
    pub run_id: u64,
    pub attempt: u32,
    pub response: TrustedRead<'a>,
}

/// The actor's independent branch-filtered workflow lookup. The full ref
/// identifies a branch, while GitHub's branch query uses its short name.
#[derive(Clone, Copy)]
pub struct LatestCandidateRuns<'a> {
    pub source: &'a CiSource,
    pub target_ref: &'a str,
    pub head_sha: &'a str,
    pub response: TrustedRead<'a>,
}

/// The actor obtains the selected attempt and its complete job list, then
/// looks up the candidate's latest run across all statuses and attempts.
#[derive(Clone, Copy)]
pub struct TrustedReads<'a> {
    pub selected_attempt: SelectedAttempt<'a>,
    pub attempt_jobs: Option<TrustedRead<'a>>,
    pub latest_candidate_runs: LatestCandidateRuns<'a>,
    /// Trusted lower bound for acceptable reads and terminal completion times.
    pub fresh_after_ms: i64,
    pub expires_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationError {
    InvalidApproval,
    StaleRead,
    InvalidResponse,
    BindingMismatch,
    UnsupportedEvent,
    IncompleteList,
    Superseded,
    Rerun,
    UnverifiedCompletion,
}

#[derive(Deserialize)]
struct Repository {
    id: u64,
}

#[derive(Deserialize)]
struct WorkflowRun {
    id: u64,
    run_number: u64,
    run_attempt: u32,
    workflow_id: u64,
    event: String,
    repository: Repository,
    head_repository: Repository,
    head_branch: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct RunList {
    total_count: usize,
    workflow_runs: Vec<WorkflowRun>,
}

#[derive(Deserialize)]
struct Job {
    id: u64,
    run_id: u64,
    run_attempt: u32,
    head_branch: String,
    head_sha: String,
    status: String,
    conclusion: Option<String>,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
}

#[derive(Deserialize)]
struct JobList {
    total_count: usize,
    jobs: Vec<Job>,
}

fn decode<T: DeserializeOwned>(read: TrustedRead<'_>) -> Result<T, ObservationError> {
    if read.body.len() > MAX_RESPONSE_BYTES {
        return Err(ObservationError::InvalidResponse);
    }
    serde_json::from_slice(read.body).map_err(|_| ObservationError::InvalidResponse)
}

fn timestamp_ms(value: &str) -> Option<i64> {
    // GitHub REST Actions exposes UTC timestamps with whole-second precision.
    let bytes: &[u8; 20] = value.as_bytes().try_into().ok()?;
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return None;
    }
    let number = |start, end| {
        let part = value.get(start..end)?;
        if !part.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        part.parse::<i32>().ok()
    };
    let day = Date::from_calendar_date(
        number(0, 4)?,
        Month::try_from(number(5, 7)? as u8).ok()?,
        number(8, 10)? as u8,
    )
    .ok()?;
    day.with_hms(
        number(11, 13)? as u8,
        number(14, 16)? as u8,
        number(17, 19)? as u8,
    )
    .ok()?
    .assume_utc()
    .unix_timestamp()
    .checked_mul(1_000)
    .filter(|value| *value > 0)
}

fn run_bounds(run: &WorkflowRun, received_at_ms: i64) -> Result<(i64, i64), ObservationError> {
    let invalid = ObservationError::InvalidResponse;
    let created = timestamp_ms(&run.created_at).ok_or(invalid)?;
    let updated = timestamp_ms(&run.updated_at).ok_or(invalid)?;
    if created > updated || updated > received_at_ms {
        return Err(invalid);
    }
    Ok((created, updated))
}

fn bind_run(
    run: &WorkflowRun,
    approval: &ApprovedCiSource,
    branch: &str,
) -> Result<(), ObservationError> {
    if run.id == 0
        || run.run_number == 0
        || run.run_attempt == 0
        || run.workflow_id == 0
        || run.repository.id == 0
        || run.workflow_id.to_string() != approval.source.workflow_id
        || run.repository.id.to_string() != approval.source.repository_id
        || run.head_repository.id != run.repository.id
        || run.head_branch != branch
        || run.head_sha != approval.current_candidate.commit
    {
        return Err(ObservationError::BindingMismatch);
    }
    // pull_request can execute a merge SHA even when REST head_sha names the
    // branch candidate. A PR head value cannot certify those execution bytes.
    if !matches!(run.event.as_str(), "push" | "workflow_dispatch") {
        return Err(ObservationError::UnsupportedEvent);
    }
    Ok(())
}

fn status(run: &WorkflowRun) -> CiStatus {
    match (run.status.as_str(), run.conclusion.as_deref()) {
        ("queued" | "requested" | "waiting" | "pending", None) => CiStatus::Pending,
        ("in_progress", None) => CiStatus::Running,
        ("completed", Some("success")) => CiStatus::Passed,
        ("completed", Some("failure")) => CiStatus::Failed,
        ("completed", Some("cancelled")) => CiStatus::Cancelled,
        _ => CiStatus::Unknown,
    }
}

fn verify_completion(
    run: &WorkflowRun,
    jobs: TrustedRead<'_>,
    created: i64,
    run_updated_ms: i64,
    fresh_after_ms: i64,
    status: CiStatus,
) -> Result<(i64, Option<String>), ObservationError> {
    let list: JobList = decode(jobs)?;
    if list.total_count == 0 || list.jobs.len() != list.total_count {
        return Err(ObservationError::IncompleteList);
    }
    let mut ids = BTreeSet::new();
    let mut finished = 0;
    let mut last_failure = None;
    let invalid = ObservationError::UnverifiedCompletion;
    for job in list.jobs {
        if job.id == 0
            || !ids.insert(job.id)
            || job.run_id != run.id
            || job.run_attempt != run.run_attempt
            || job.head_branch != run.head_branch
            || job.head_sha != run.head_sha
        {
            return Err(ObservationError::BindingMismatch);
        }
        let job_created = timestamp_ms(&job.created_at).ok_or(invalid)?;
        let job_started = match job.started_at {
            Some(value) => timestamp_ms(&value).ok_or(invalid)?,
            None if job.conclusion.as_deref() == Some("skipped") => job_created,
            None => return Err(invalid),
        };
        let completed = job
            .completed_at
            .as_deref()
            .and_then(timestamp_ms)
            .ok_or(invalid)?;
        if job.status != "completed"
            || !matches!(
                job.conclusion.as_deref(),
                Some(
                    "success"
                        | "failure"
                        | "cancelled"
                        | "skipped"
                        | "neutral"
                        | "timed_out"
                        | "action_required"
                        | "stale"
                        | "startup_failure"
                )
            )
            || job_created < created
            || job_started < job_created
            || completed < job_started
            || completed > run_updated_ms
            || completed > jobs.received_at_ms
        {
            return Err(invalid);
        }
        finished = finished.max(completed);
        if job.conclusion.as_deref() == Some("failure") {
            last_failure = last_failure.max(Some((completed, job.id)));
        }
    }
    let report = if status == CiStatus::Failed {
        let (failed_at, id) = last_failure.ok_or(invalid)?;
        if failed_at < fresh_after_ms {
            return Err(ObservationError::StaleRead);
        }
        Some(format!("github-actions-job:{id}"))
    } else {
        None
    };
    Ok((finished, report))
}

/// Accepts only host-produced reads of the approved repository and workflow.
/// An error leaves the CI observation unavailable to the repair decision.
pub fn map_observation(
    candidate: &CiCandidate,
    approval: &ApprovedCiSource,
    reads: TrustedReads<'_>,
    now_ms: i64,
) -> Result<CiObservation, ObservationError> {
    use ObservationError::*;
    let source = &approval.source;
    if source.service_id != SERVICE_ID
        || !source.valid()
        || !authority::id(&approval.approval_ref)
        || approval.current_candidate != *candidate
        || !git_oid(&candidate.commit)
        || !super::super::reads::valid_ref(&candidate.target_ref)
    {
        return Err(InvalidApproval);
    }
    let branch = candidate
        .target_ref
        .strip_prefix("refs/heads/")
        .ok_or(InvalidApproval)?;
    let latest_read = reads.latest_candidate_runs;
    if latest_read.source != source
        || latest_read.target_ref != candidate.target_ref
        || latest_read.head_sha != candidate.commit
    {
        return Err(BindingMismatch);
    }
    let selected_read = reads.selected_attempt;
    let expires_at_ms = approval.expires_at_ms.min(reads.expires_at_ms);
    let observed_at_ms = latest_read.response.received_at_ms;
    if reads.fresh_after_ms <= 0
        || expires_at_ms <= now_ms
        || observed_at_ms > now_ms
        || [Some(selected_read.response), reads.attempt_jobs]
            .into_iter()
            .flatten()
            .any(|read| !(reads.fresh_after_ms..=observed_at_ms).contains(&read.received_at_ms))
    {
        return Err(StaleRead);
    }

    let selected: WorkflowRun = decode(selected_read.response)?;
    bind_run(&selected, approval, branch)?;
    if selected.id != selected_read.run_id || selected.run_attempt != selected_read.attempt {
        return Err(BindingMismatch);
    }
    let (created, selected_updated) = run_bounds(&selected, selected_read.response.received_at_ms)?;
    let list: RunList = decode(latest_read.response)?;
    if list.total_count == 0
        || list.total_count >= RUN_SEARCH_CAP
        || list.workflow_runs.len() != list.total_count
    {
        return Err(IncompleteList);
    }
    let mut ids = BTreeSet::new();
    let mut numbers = BTreeSet::new();
    for run in &list.workflow_runs {
        bind_run(run, approval, branch)?;
        run_bounds(run, observed_at_ms)?;
        if !ids.insert(run.id) || !numbers.insert(run.run_number) {
            return Err(InvalidResponse);
        }
    }
    let latest = list
        .workflow_runs
        .iter()
        .max_by_key(|run| run.run_number)
        .ok_or(IncompleteList)?;
    if selected.id != latest.id
        || selected.run_number != latest.run_number
        || selected.run_attempt != latest.run_attempt
        || selected.event != latest.event
        || selected.status != latest.status
        || selected.conclusion != latest.conclusion
        || selected.created_at != latest.created_at
    {
        return Err(Superseded);
    }
    if latest.run_attempt != 1 {
        return Err(Rerun);
    }

    let status = status(&selected);
    let (finished_at_ms, report_ref) = match status {
        CiStatus::Failed | CiStatus::Passed | CiStatus::Cancelled => {
            let updated = timestamp_ms(&latest.updated_at).ok_or(InvalidResponse)?;
            let (finished, report) = verify_completion(
                &selected,
                reads.attempt_jobs.ok_or(UnverifiedCompletion)?,
                created,
                selected_updated.min(updated),
                reads.fresh_after_ms,
                status,
            )?;
            if finished < reads.fresh_after_ms {
                return Err(StaleRead);
            }
            (Some(finished), report)
        }
        _ => (None, None),
    };
    let identity = |run: &WorkflowRun| CiRunIdentity {
        source: source.clone(),
        run_id: run.id.to_string(),
        attempt: run.run_attempt,
    };
    Ok(CiObservation {
        candidate: candidate.clone(),
        run: identity(&selected),
        current_run: identity(latest),
        status,
        observed_at_ms,
        finished_at_ms,
        expires_at_ms,
        report_ref,
    })
}
