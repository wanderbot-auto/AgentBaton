//! Read-only CI repair eligibility. External observations and rule history must
//! come from a trusted host adapter. These Rust inputs are deliberately not
//! deserializable, and a decision cannot reserve budget or dispatch a repair.
use super::{
    authority, budget, records, IntegrationIoCompletion, IntegrationSharedUsage,
    IntegrationStageKind, IntegrationState,
};
use crate::{
    execution::ExecutionBinding,
    goal_plan::{AttemptReservation, BudgetLimits},
    policy::GrantScope,
    storage::{db_error, decode, Event, Store, TaskState},
    Result,
};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::BTreeSet;

const MAX_HISTORY: usize = 1_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CiCandidate {
    pub project_id: String,
    pub project_revision: u64,
    pub root_task_id: String,
    pub task_id: String,
    pub spec_revision: u64,
    pub integration_id: String,
    pub integration_revision: u64,
    pub repository_digest: String,
    pub target_ref: String,
    pub commit: String,
}

/// Stable identifiers from the verified CI service, independent of any URL
/// or workflow text found in the workspace or in an Agent's response.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CiSource {
    pub service_id: String,
    pub repository_id: String,
    pub workflow_id: String,
}

impl CiSource {
    fn valid(&self) -> bool {
        [&self.service_id, &self.repository_id, &self.workflow_id]
            .into_iter()
            .all(|value| authority::id(value))
    }
}

#[derive(Clone, Debug)]
pub struct ApprovedCiSource {
    pub approval_ref: String,
    pub source: CiSource,
    /// The adapter's independently obtained current subject, including the
    /// approved project binding and current remote candidate. It must not be
    /// reconstructed from the run report being judged.
    pub current_candidate: CiCandidate,
    pub expires_at_ms: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct CiRunIdentity {
    pub source: CiSource,
    pub run_id: String,
    pub attempt: u32,
}

impl CiRunIdentity {
    fn valid(&self) -> bool {
        self.attempt > 0 && authority::id(&self.run_id) && self.source.valid()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiStatus {
    Pending,
    Running,
    Failed,
    Passed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct CiObservation {
    pub candidate: CiCandidate,
    pub run: CiRunIdentity,
    /// Latest service-observed run and attempt for the candidate. The adapter
    /// must obtain this independently of the report selected for review.
    pub current_run: CiRunIdentity,
    pub status: CiStatus,
    pub observed_at_ms: i64,
    pub finished_at_ms: Option<i64>,
    pub expires_at_ms: i64,
    pub report_ref: Option<String>,
}

/// An enabled, current Owner-approved policy from trusted effective settings.
/// A project file, CI output or Integration's local-only approval cannot
/// supply this record.
#[derive(Clone, Debug)]
pub struct CiRepairRule {
    pub id: String,
    pub revision: u64,
    pub approval_ref: String,
    pub candidate: CiCandidate,
    pub source: CiSource,
    pub actor: ExecutionBinding,
    pub approved_at_ms: i64,
    pub expires_at_ms: i64,
    pub revoked: bool,
    pub budget: BudgetLimits,
    pub reservation: AttemptReservation,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CiRepairMeters {
    pub runtime_ms: u64,
    pub output_bytes: u64,
    pub tokens: u64,
    pub cost_microunits: u64,
}

#[derive(Clone, Debug)]
pub enum CiRepairProgress {
    Running,
    CancelRequested,
    OutcomeUnknown,
    /// Definitively stopped, with an optional measured usage and result commit.
    Settled {
        produced_commit: Option<String>,
        meters: Option<CiRepairMeters>,
    },
}

#[derive(Clone, Debug)]
pub struct CiRepairHistoryEntry {
    pub run: CiRunIdentity,
    pub input_commit: String,
    pub progress: CiRepairProgress,
}

/// Complete authoritative history for the stable root across rule/candidate revisions.
#[derive(Clone, Debug)]
pub struct CiRepairHistory {
    pub project_id: String,
    pub root_task_id: String,
    pub entries: Vec<CiRepairHistoryEntry>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CiRepairInputs<'a> {
    pub source: Option<&'a ApprovedCiSource>,
    pub observation: Option<&'a CiObservation>,
    pub rule: Option<&'a CiRepairRule>,
    pub history: Option<&'a CiRepairHistory>,
}

/// Core-derived current grant and shared usage. The advisory evaluator can
/// also consume an explicitly constructed snapshot in internal host tests.
#[derive(Clone, Debug)]
pub struct CiRepairAuthority {
    pub actor: ExecutionBinding,
    pub scope: GrantScope,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub shared_usage: IntegrationSharedUsage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CiRepairBlock {
    CandidateNotCurrent,
    CandidateOutcomeUnknown,
    CandidateCancelled,
    MissingApprovedSource,
    InvalidApprovedSource,
    MissingObservation,
    ObservationMismatch,
    ObservationStale,
    ObservationUnverified,
    RuleUnavailable,
    AuthorityUnavailable,
    HistoryUnavailable,
    HistoryLimit,
    Duplicate,
    RepairInProgress,
    RepairOutcomeUnknown,
    RepairCycle,
    AttemptLimit,
    Capacity,
    BudgetUnknown,
    BudgetExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CiRepairDecision {
    Blocked {
        reason: CiRepairBlock,
    },
    CiPending,
    CiPassed,
    CiCancelled,
    CiOutcomeUnknown,
    ContinueRequired,
    Eligible {
        deduplication_key: String,
        run: CiRunIdentity,
        report_ref: String,
        rule_id: String,
        rule_revision: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CiRepairDecisionView {
    pub as_of_sequence: u64,
    pub decision: CiRepairDecision,
}

fn blocked(reason: CiRepairBlock) -> CiRepairDecision {
    CiRepairDecision::Blocked { reason }
}

fn git_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && value.bytes().any(|b| b != b'0')
}

fn applicable_failure<'a>(
    candidate: &CiCandidate,
    input: CiRepairInputs<'a>,
    now_ms: i64,
) -> std::result::Result<(&'a CiObservation, &'a CiRepairRule, &'a str), CiRepairDecision> {
    use CiRepairBlock::*;
    let source = input.source.ok_or_else(|| blocked(MissingApprovedSource))?;
    if !source.source.valid()
        || !authority::id(&source.approval_ref)
        || source.expires_at_ms <= now_ms
    {
        return Err(blocked(InvalidApprovedSource));
    }
    if !git_oid(&candidate.commit) || source.current_candidate != *candidate {
        return Err(blocked(CandidateNotCurrent));
    }
    let observation = input
        .observation
        .ok_or_else(|| blocked(MissingObservation))?;
    if observation.candidate != *candidate || observation.run.source != source.source {
        return Err(blocked(ObservationMismatch));
    }
    if !observation.run.valid()
        || !observation.current_run.valid()
        || observation.current_run.source != source.source
        || observation.observed_at_ms <= 0
        || observation.observed_at_ms > now_ms
        || (observation.run.run_id == observation.current_run.run_id
            && observation.run.attempt > observation.current_run.attempt)
    {
        return Err(blocked(ObservationUnverified));
    }
    if observation.run != observation.current_run || observation.expires_at_ms <= now_ms {
        return Err(blocked(ObservationStale));
    }
    match observation.status {
        CiStatus::Pending | CiStatus::Running => return Err(CiRepairDecision::CiPending),
        CiStatus::Unknown => return Err(CiRepairDecision::CiOutcomeUnknown),
        CiStatus::Failed | CiStatus::Passed | CiStatus::Cancelled => {}
    }
    let Some(finished) = observation
        .finished_at_ms
        .filter(|when| *when > 0 && *when <= observation.observed_at_ms)
    else {
        return Err(blocked(ObservationUnverified));
    };
    match observation.status {
        CiStatus::Passed => return Err(CiRepairDecision::CiPassed),
        CiStatus::Cancelled => return Err(CiRepairDecision::CiCancelled),
        _ => {}
    }
    let Some(report) = observation
        .report_ref
        .as_deref()
        .filter(|value| authority::id(value))
    else {
        return Err(blocked(ObservationUnverified));
    };
    let rule = input.rule.ok_or(CiRepairDecision::ContinueRequired)?;
    if rule.revoked
        || rule.candidate != *candidate
        || rule.source != source.source
        || rule.revision == 0
        || !authority::id(&rule.id)
        || !authority::id(&rule.approval_ref)
        || rule.approved_at_ms <= 0
        || rule.approved_at_ms > finished
        || rule.expires_at_ms <= now_ms
    {
        return Err(blocked(RuleUnavailable));
    }
    Ok((observation, rule, report))
}

fn configured(
    candidate: &CiCandidate,
    observation: &CiObservation,
    rule: &CiRepairRule,
    report: &str,
    input: CiRepairInputs<'_>,
    current: Option<&CiRepairAuthority>,
) -> std::result::Result<CiRepairDecision, CiRepairBlock> {
    use CiRepairBlock::*;
    let history = input.history.ok_or(HistoryUnavailable)?;
    if history.project_id != candidate.project_id || history.root_task_id != candidate.root_task_id
    {
        return Err(HistoryUnavailable);
    }
    if history.entries.len() > MAX_HISTORY {
        return Err(HistoryLimit);
    }
    let mut attempts = BTreeSet::new();
    for entry in &history.entries {
        if !entry.run.valid() || !git_oid(&entry.input_commit) || !attempts.insert(&entry.run) {
            return Err(HistoryUnavailable);
        }
        if let CiRepairProgress::Settled {
            produced_commit: Some(commit),
            ..
        } = &entry.progress
        {
            if !git_oid(commit) {
                return Err(HistoryUnavailable);
            }
        }
    }
    if attempts.contains(&observation.run) {
        return Err(Duplicate);
    }
    let mut spent = CiRepairMeters::default();
    for entry in &history.entries {
        let CiRepairProgress::Settled {
            produced_commit,
            meters,
        } = &entry.progress
        else {
            return Err(match entry.progress {
                CiRepairProgress::OutcomeUnknown | CiRepairProgress::CancelRequested => {
                    RepairOutcomeUnknown
                }
                _ => RepairInProgress,
            });
        };
        if entry.run.source.repository_id == observation.run.source.repository_id
            && entry.run.source.service_id == observation.run.source.service_id
            && (entry.input_commit == candidate.commit
                || produced_commit.as_ref() == Some(&candidate.commit))
        {
            return Err(RepairCycle);
        }
        let meters = meters.ok_or(BudgetUnknown)?;
        spent.runtime_ms = spent
            .runtime_ms
            .checked_add(meters.runtime_ms)
            .ok_or(BudgetExceeded)?;
        spent.output_bytes = spent
            .output_bytes
            .checked_add(meters.output_bytes)
            .ok_or(BudgetExceeded)?;
        spent.tokens = spent
            .tokens
            .checked_add(meters.tokens)
            .ok_or(BudgetExceeded)?;
        spent.cost_microunits = spent
            .cost_microunits
            .checked_add(meters.cost_microunits)
            .ok_or(BudgetExceeded)?;
    }
    let current = current.ok_or(AuthorityUnavailable)?;
    let reservation = &rule.reservation;
    if rule.actor != current.actor
        || rule.actor.input_revision != candidate.commit
        || reservation.runtime_ms > current.scope.max_duration_ms
        || rule.budget.attempts > current.scope.max_attempts
        || rule.budget.parallel > current.scope.max_concurrency
        || !rule.budget.narrower(&current.root_budget)
        || !rule.budget.narrower(&current.project_budget)
    {
        return Err(AuthorityUnavailable);
    }
    let count = history.entries.len() as u32;
    if count >= rule.budget.attempts
        || count >= rule.budget.reworks
        || current.shared_usage.root.attempts >= current.root_budget.attempts
        || current.shared_usage.project.attempts >= current.project_budget.attempts
    {
        return Err(AttemptLimit);
    }
    if current.shared_usage.root.active >= current.root_budget.parallel
        || current.shared_usage.project.active >= current.project_budget.parallel
    {
        return Err(Capacity);
    }
    // The read model exposes settled usage and active counts, but does not
    // expose each active reservation's complete token/output/cost amounts.
    // This advisory decision cannot treat available slots as available meters.
    if current.shared_usage.root.active != 0 || current.shared_usage.project.active != 0 {
        return Err(RepairInProgress);
    }
    if reservation.runtime_ms == 0
        || reservation.output_bytes == 0
        || rule.budget.cost_microunits.is_none()
    {
        return Err(RuleUnavailable);
    }
    let tokens = reservation.tokens.ok_or(BudgetUnknown)?;
    let cost = reservation.cost_microunits.ok_or(BudgetUnknown)?;
    let check = |spent: CiRepairMeters, limit: &BudgetLimits| {
        let fits = |used: u64, next: u64, cap: Option<u64>| {
            used.checked_add(next)
                .is_some_and(|total| cap.is_none_or(|max| total <= max))
        };
        if fits(
            spent.runtime_ms,
            reservation.runtime_ms,
            Some(limit.runtime_ms),
        ) && fits(
            spent.output_bytes,
            reservation.output_bytes,
            Some(limit.output_bytes),
        ) && fits(spent.tokens, tokens, limit.tokens)
            && fits(spent.cost_microunits, cost, limit.cost_microunits)
        {
            Ok(())
        } else {
            Err(BudgetExceeded)
        }
    };
    check(spent, &rule.budget)?;
    for (usage, limit) in [
        (&current.shared_usage.root, &current.root_budget),
        (&current.shared_usage.project, &current.project_budget),
    ] {
        check(
            CiRepairMeters {
                runtime_ms: usage.runtime_ms.ok_or(BudgetUnknown)?,
                output_bytes: usage.output_bytes.ok_or(BudgetUnknown)?,
                tokens: usage.tokens.ok_or(BudgetUnknown)?,
                cost_microunits: usage.cost_microunits.ok_or(BudgetUnknown)?,
            },
            limit,
        )?;
    }
    // Scope by stable project/root/repository/run attempt. Rule revisions and
    // transport retries must never mint a new identifier for the same failure.
    let key = authority::sha(&(
        "ci-repair-v1",
        &candidate.project_id,
        &candidate.root_task_id,
        &observation.run,
    ))
    .map_err(|_| ObservationUnverified)?;
    Ok(CiRepairDecision::Eligible {
        deduplication_key: format!("ci-repair:{key}"),
        run: observation.run.clone(),
        report_ref: report.into(),
        rule_id: rule.id.clone(),
        rule_revision: rule.revision,
    })
}

/// Deterministic policy evaluation of an already verified snapshot. Production
/// readers should use Store::decide_ci_repair to acquire the Core-owned facts.
pub fn evaluate_ci_repair(
    candidate: &CiCandidate,
    input: CiRepairInputs<'_>,
    current: Option<&CiRepairAuthority>,
    now_ms: i64,
) -> CiRepairDecision {
    match applicable_failure(candidate, input, now_ms) {
        Ok((observation, rule, report)) => {
            configured(candidate, observation, rule, report, input, current).unwrap_or_else(blocked)
        }
        Err(decision) => decision,
    }
}

// This is a single read snapshot. No result, even Eligible, authorizes a
// command. A future dispatcher must reacquire the trusted inputs and commit a
// unique attempt plus metering reservations before creating any side effect.
impl Store {
    pub fn decide_ci_repair(
        &self,
        integration_id: &str,
        input: CiRepairInputs<'_>,
    ) -> Result<CiRepairDecisionView> {
        authority::key(integration_id)?;
        let tx = self.conn.unchecked_transaction().map_err(db_error)?;
        let as_of_sequence = records::sequence(&tx)?;
        let view = |decision| CiRepairDecisionView {
            as_of_sequence,
            decision,
        };
        let run = records::run(&tx, integration_id)?;
        let policy = records::policy(&tx, &run.policy_id)?;
        let task = self.get(&run.task_id)?;
        let project = self.get_project(&policy.bindings.project_id)?;
        if run.state == IntegrationState::OutcomeUnknown || task.state == TaskState::OutcomeUnknown
        {
            return Ok(view(blocked(CiRepairBlock::CandidateOutcomeUnknown)));
        }
        if run.cancel_requested
            || run.state == IntegrationState::Cancelled
            || matches!(
                task.state,
                TaskState::CancelRequested | TaskState::Cancelled
            )
        {
            return Ok(view(blocked(CiRepairBlock::CandidateCancelled)));
        }
        let bindings = &policy.bindings;
        let Some(commit) = run
            .integration_commit
            .as_ref()
            .filter(|value| git_oid(value))
        else {
            return Ok(view(blocked(CiRepairBlock::CandidateNotCurrent)));
        };
        if task.project_id != project.id
            || project.archived
            || project.revision != bindings.project_revision
            || task.spec_revision != run.spec_revision
            || task.state != TaskState::Accepted
            || task.acceptance_ref.as_deref() != Some(&run.acceptance_ref)
            || run.state != IntegrationState::ConfirmedLocalRef
            || run.active_stage.is_some()
            || !confirmed_candidate(&tx, &run, &policy)?
        {
            return Ok(view(blocked(CiRepairBlock::CandidateNotCurrent)));
        }
        let Some(acceptance) =
            self.integration_verified_acceptance(&run.goal_scope_id, &task.id)?
        else {
            return Ok(view(blocked(CiRepairBlock::CandidateNotCurrent)));
        };
        if acceptance.id != run.acceptance_ref
            || authority::sha(&acceptance)? != run.acceptance_digest
        {
            return Ok(view(blocked(CiRepairBlock::CandidateNotCurrent)));
        }
        let candidate = CiCandidate {
            project_id: project.id,
            project_revision: project.revision,
            root_task_id: bindings.root_task_id.clone(),
            task_id: task.id,
            spec_revision: task.spec_revision,
            integration_id: run.id.clone(),
            integration_revision: run.revision,
            repository_digest: bindings.repository_digest.clone(),
            target_ref: run.target_ref,
            commit: commit.clone(),
        };
        let now_ms = authority::ms()?;
        let (observation, rule, report) = match applicable_failure(&candidate, input, now_ms) {
            Ok(ready) => ready,
            Err(decision) => return Ok(view(decision)),
        };
        let current = self.ci_repair_authority(&policy, &candidate, rule, now_ms);
        Ok(view(match current {
            Ok(authority) => configured(
                &candidate,
                observation,
                rule,
                report,
                input,
                Some(&authority),
            )
            .unwrap_or_else(blocked),
            Err(error) if error.code == "BUDGET_UNKNOWN" => blocked(CiRepairBlock::BudgetUnknown),
            Err(error) if error.code == "RESOURCE_LIMIT" => blocked(CiRepairBlock::Capacity),
            Err(error) if error.code == "WORKSPACE_BUSY" => {
                blocked(CiRepairBlock::RepairInProgress)
            }
            Err(error) if error.code == "INTERNAL_ERROR" => return Err(error),
            Err(_) => blocked(CiRepairBlock::AuthorityUnavailable),
        }))
    }

    fn ci_repair_authority(
        &self,
        policy: &super::IntegrationPolicy,
        candidate: &CiCandidate,
        rule: &CiRepairRule,
        now_ms: i64,
    ) -> Result<CiRepairAuthority> {
        if policy.revoked || policy.expires_at_ms <= now_ms {
            return Err(authority::denied());
        }
        let bindings = &policy.bindings;
        let material =
            self.integration_scope_material(&bindings.goal_scope_id, &candidate.task_id)?;
        if material.task.goal != bindings.task_goal
            || material.task.spec_revision != candidate.spec_revision
            || material.scope_revision != bindings.goal_scope_revision
            || material.scope_digest != bindings.goal_scope_digest
            || material.root_task_id != candidate.root_task_id
            || material.project.id != candidate.project_id
            || material.project.revision != candidate.project_revision
            || material.workspace.id != bindings.workspace_id
            || material.workspace.revision != bindings.workspace_revision
            || material.grant.root_grant_id != bindings.root_grant_id
        {
            return Err(authority::changed());
        }
        let connection_id = &rule.actor.connection_id;
        let connection = material
            .grant
            .connection_bindings
            .get(connection_id)
            .ok_or_else(authority::denied)?;
        let originally_bound = bindings
            .grant_lineage
            .first()
            .ok_or_else(authority::denied)?;
        if originally_bound.id != material.grant.id
            || originally_bound.revision != material.grant.revision
            || originally_bound.connection_bindings.get(connection_id) != Some(connection)
            || connection.host_id != policy.selection.host_id
        {
            return Err(authority::denied());
        }
        crate::policy::validate_ancestor_grant_budgets_from(&self.conn, &material.grant.id)?;
        crate::policy::validate_grant_live_from_with_data(
            &self.conn,
            &material.grant.id,
            &candidate.project_id,
            &bindings.workspace_id,
            connection_id,
            None,
        )?;
        super::fences::check_conversation_writers(&self.conn, &bindings.source)?;
        super::check_workspace_fences(
            &self.conn,
            material
                .workspace
                .identity
                .as_ref()
                .ok_or_else(authority::changed)?,
            None,
        )?;
        let writer: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id
              WHERE d.state!='stopped' AND (a.root_task_id=?1 OR json_extract(a.binding,'$.workspace_id')=?2))
              OR EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?2)
              OR EXISTS(SELECT 1 FROM goal_graph_jobs WHERE workspace_id=?2 AND state IN('prepared','running','cancel_requested','outcome_unknown'))
              OR EXISTS(SELECT 1 FROM workspace_leases WHERE resource=?2 AND holder!='')",
            [&candidate.root_task_id, &bindings.workspace_id], |row| row.get(0)
        ).map_err(db_error)?;
        if writer {
            return Err(crate::CoreError::new(
                "WORKSPACE_BUSY",
                "An owned writer or unresolved attempt still exists",
            ));
        }
        Ok(CiRepairAuthority {
            actor: ExecutionBinding {
                connection_id: connection_id.clone(),
                environment_id: connection.environment_id.clone(),
                identity_ref: connection.identity_ref.clone(),
                host_id: connection.host_id.clone(),
                workspace_id: material.workspace.id,
                config_revision: connection.config_revision.clone(),
                input_revision: candidate.commit.clone(),
                capability_snapshot_id: connection.capability_snapshot_id.clone(),
                policy_revision: format!("{}:{}", material.grant.id, material.grant.revision),
            },
            scope: material.grant.scope,
            root_budget: material.root_budget,
            project_budget: material.project_budget,
            shared_usage: material.usage,
        })
    }
}

fn confirmed_candidate(
    db: &Connection,
    run: &super::IntegrationRun,
    policy: &super::IntegrationPolicy,
) -> Result<bool> {
    let mut query = db.prepare(
        "SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events
         WHERE json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.source')='integration_core'
         AND json_extract(document,'$.kind')='integration.cas_observed'
         AND json_type(document,'$.payload.historical_cas_confirmed')='true' ORDER BY sequence LIMIT 2",
    ).map_err(db_error)?;
    let mut rows = query.query([&run.id]).map_err(db_error)?;
    let Some(row) = rows.next().map_err(db_error)? else {
        return Ok(false);
    };
    let document: Option<String> = row.get(0).map_err(db_error)?;
    if rows.next().map_err(db_error)?.is_some() || document.is_none() {
        return Ok(false);
    }
    let event: Event = decode(document.as_deref().ok_or_else(authority::changed)?)?;
    let Some(attempt_id) = event.payload["attempt_id"]
        .as_str()
        .filter(|value| authority::id(value))
    else {
        return Ok(false);
    };
    let attempt = budget::attempt(db, attempt_id)?;
    let record = &event.payload["record"];
    Ok(attempt.integration_id == run.id
        && attempt.view.kind == IntegrationStageKind::Cas
        && attempt.view.round == run.round
        && attempt.view.settled
        && attempt.view.io_completion == Some(IntegrationIoCompletion::WorkerJoined)
        && event.revision <= run.revision
        && event.payload["round"] == run.round
        && record["state"] == "confirmed_local_ref"
        && record["grant_ref"] == policy.approval_id
        && record["target_ref"] == run.target_ref
        && record["expected_target_head"] == run.expected_target_head
        && record["candidate_commit"].as_str() == run.integration_commit.as_deref())
}
