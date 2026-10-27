//! Versioned product goal contracts and deterministic admission decisions.
//! No model, worker, filesystem mutation, Store transaction or scheduler runs here.
//! Host-only constructors require already authenticated, current Store evidence;
//! their labels are not signatures and must never be populated from Agent JSON.
use crate::{
    execution::ExecutionBinding,
    policy::{ExecutionGrant, GrantedConnection},
    storage::Task,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_NODES: usize = 256;
const MAX_REFS: usize = 256;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    InvalidContract,
    AuthorityExpansion,
    StaleRevision,
    InputChanged,
    MissingReference,
    Cycle,
    Duplicate,
    Limit,
    StaleEvidence,
}
pub type PlanResult<T> = Result<T, PlanError>;
fn id(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 256
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
}
fn hash(v: &str) -> bool {
    v.len() == 64
        && v.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn ids(v: &[String]) -> bool {
    v.len() <= MAX_REFS
        && v.iter().all(|x| id(x))
        && v.iter().collect::<BTreeSet<_>>().len() == v.len()
}
fn digest<T: Serialize>(v: &T) -> PlanResult<String> {
    let b = serde_json::to_vec(v).map_err(|_| PlanError::InvalidContract)?;
    Ok(format!("{:x}", Sha256::digest(b)))
}
fn path(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 4096
        && !v.starts_with('/')
        && !v.contains('\\')
        && !v.chars().any(char::is_control)
        && v.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}
fn beneath(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}
fn path_overlap(a: &str, b: &str) -> bool {
    if !a.is_ascii() || !b.is_ascii() {
        return true;
    }
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    beneath(&a, &b) || beneath(&b, &a)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalBinding {
    pub project_id: String,
    pub root_task_id: String,
    pub task_id: String,
    pub spec_revision: u64,
    pub input_revision: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BudgetLimits {
    pub attempts: u32,
    pub parallel: u32,
    pub runtime_ms: u64,
    pub output_bytes: u64,
    pub children: u32,
    pub reworks: u32,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
}
impl BudgetLimits {
    fn valid(&self) -> bool {
        self.attempts > 0
            && self.attempts <= 100_000
            && self.parallel > 0
            && self.parallel <= self.attempts
            && self.runtime_ms > 0
            && self.runtime_ms <= 604_800_000
            && self.output_bytes > 0
            && self.children <= 1000
            && self.reworks <= 1000
    }
    fn narrower(&self, other: &Self) -> bool {
        self.valid()
            && other.valid()
            && self.attempts <= other.attempts
            && self.parallel <= other.parallel
            && self.runtime_ms <= other.runtime_ms
            && self.output_bytes <= other.output_bytes
            && self.children <= other.children
            && self.reworks <= other.reworks
            && bounded(self.tokens, other.tokens)
            && bounded(self.cost_microunits, other.cost_microunits)
    }
}
fn bounded(child: Option<u64>, parent: Option<u64>) -> bool {
    match (parent, child) {
        (None, _) => true,
        (Some(p), Some(c)) => c <= p,
        _ => false,
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Work,
    Validate,
    OwnerApproval,
    Summary,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Executor {
    Connection {
        binding: Box<ExecutionBinding>,
        program_sha256: String,
    },
    Validator {
        rule_id: String,
    },
    Owner {
        rule_id: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    AdvisoryReadOnly,
    WorkspaceWrite,
    ExternalWrite,
    Production,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WriteTarget {
    Workspace { path: String },
    External { resource_id: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum InputSource {
    ApprovedExternal {
        sha256: String,
    },
    Node {
        node_id: String,
        candidate_for_validation: bool,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RequiredInput {
    pub goal_task_id: String,
    pub artifact_id: String,
    pub contract_id: String,
    pub source: InputSource,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlannedOutput {
    pub artifact_id: String,
    pub contract_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    BeforeDispatchTransient,
    CrashedNoSideEffect,
    ValidationFailed,
    Authentication,
    Permission,
    Unsupported,
    InputChanged,
    DisconnectedUnknown,
    Permanent,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub max_reworks: u32,
    pub retry_classes: Vec<FailureClass>,
}
/// Maximum resources reserved for one dispatch; BudgetLimits are cumulative.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttemptReservation {
    pub runtime_ms: u64,
    pub output_bytes: u64,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: NodeKind,
    pub goal: String,
    pub dependencies: Vec<String>,
    pub workspace_id: String,
    pub executor: Executor,
    pub recipients: Vec<String>,
    pub required_inputs: Vec<RequiredInput>,
    pub outputs: Vec<PlannedOutput>,
    pub required_validators: Vec<String>,
    pub side_effect: SideEffect,
    pub write_set: Vec<WriteTarget>,
    pub budget: BudgetLimits,
    pub reservation: AttemptReservation,
    pub retry: RetryPolicy,
    pub idempotency_key: String,
    pub external_action_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GoalPlanProposal {
    pub schema_version: u32,
    pub plan_id: String,
    pub goal: GoalBinding,
    pub scope_revision: u64,
    pub revision: u64,
    pub expected_revision: u64,
    pub acceptance_rules: Vec<String>,
    pub nodes: Vec<PlanNode>,
}

/// Host policy references are resolved from owner-approved records, not from a
/// proposed validator name or an Agent's completion message.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum RuleKind {
    Deterministic,
    Owner,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ApprovedRuleRef {
    pub kind: RuleKind,
    pub policy_digest: String,
    pub goal_acceptance: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ArtifactContract {
    pub required_validators: BTreeSet<String>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExternalInput {
    pub contract_id: String,
    pub sha256: String,
    pub verified: bool,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ConnectionPermit {
    pub provider_quota_key: String,
    pub binding: GrantedConnection,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct WorkspacePermit {
    pub host_id: String,
    pub canonical_root: String,
    pub device: u64,
    pub inode: u64,
    pub writable_prefixes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExternalWritePermit {
    pub recipient: String,
    pub conflict_group: String,
}
/// Exact structured request approved by the host. No model-authored request or
/// digest is accepted as authority. Dispatch must send/recheck these exact bytes.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ExternalActionRequest {
    pub operation: String,
    pub recipient: String,
    pub resource_ids: BTreeSet<String>,
    pub payload: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ApprovedExternalAction {
    pub executor: Executor,
    pub request: ExternalActionRequest,
    pub payload_sha256: String,
}
impl ApprovedExternalAction {
    /// Caller must resolve actual owner-approved request/connection records.
    /// Computing this digest authenticates neither the caller nor owner approval.
    pub fn from_verified_host(
        executor: Executor,
        request: ExternalActionRequest,
    ) -> PlanResult<Self> {
        if serde_json::to_vec(&request)
            .map_err(|_| PlanError::InvalidContract)?
            .len()
            > 16384
        {
            return Err(PlanError::Limit);
        }
        let payload_sha256 = digest(&(&executor, &request))?;
        Ok(Self {
            executor,
            request,
            payload_sha256,
        })
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ResourceLimits {
    pub global: u32,
    pub hosts: BTreeMap<String, u32>,
    pub providers: BTreeMap<String, u32>,
    pub workspaces: BTreeMap<String, u32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct HostGoalPolicy {
    pub root_task_id: String,
    pub input_revision: String,
    pub max_nodes: u32,
    pub max_depth: u32,
    pub root_budget: BudgetLimits,
    pub project_budget: BudgetLimits,
    pub resources: ResourceLimits,
    pub connections: BTreeMap<String, ConnectionPermit>,
    pub workspaces: BTreeMap<String, WorkspacePermit>,
    pub rules: BTreeMap<String, ApprovedRuleRef>,
    pub contracts: BTreeMap<String, ArtifactContract>,
    pub external_inputs: BTreeMap<String, ExternalInput>,
    pub external_writes: BTreeMap<String, ExternalWritePermit>,
    pub external_actions: BTreeMap<String, ApprovedExternalAction>,
    pub required_goal_outputs: BTreeMap<String, String>,
    pub acceptance_rules: BTreeSet<String>,
}
/// Non-deserializable, host-resolved authority. This type prevents accidental
/// Agent-DTO authority mixing; it does not authenticate an arbitrary Rust caller.
#[derive(Clone, Debug)]
pub struct ApprovedGoalScope {
    goal: GoalBinding,
    revision: u64,
    approval_id: String,
    expires_at_ms: i64,
    grant: ExecutionGrant,
    policy: HostGoalPolicy,
    digest: String,
}
impl ApprovedGoalScope {
    /// Caller must verify owner approval, grant authenticity/current revision,
    /// program/config/identity, project rights and actual rule/receipt ownership.
    /// `now_ms` must be host time. No Agent-supplied caller/approval string is read.
    pub fn from_verified_host(
        task: &Task,
        grant: &ExecutionGrant,
        policy: HostGoalPolicy,
        now_ms: i64,
    ) -> PlanResult<Self> {
        if grant.revoked
            || grant.parent_id.is_some()
            || grant.depth != 0
            || grant.root_grant_id != grant.id
            || grant.project_id != task.project_id
            || grant.expires_at_ms <= now_ms
        {
            return Err(PlanError::AuthorityExpansion);
        }
        grant
            .scope
            .validate()
            .map_err(|_| PlanError::AuthorityExpansion)?;
        if !id(&task.id)
            || !id(&task.project_id)
            || !id(&policy.root_task_id)
            || !id(&policy.input_revision)
            || task.spec_revision == 0
            || grant.revision == 0
            || !id(&grant.approval_id)
        {
            return Err(PlanError::InvalidContract);
        }
        if policy.max_nodes == 0
            || policy.max_nodes as usize > MAX_NODES
            || policy.max_depth > grant.scope.max_depth
            || policy.max_depth > 32
            || grant
                .scope
                .max_children
                .checked_add(1)
                .is_none_or(|n| policy.max_nodes > n)
        {
            return Err(PlanError::Limit);
        }
        if !policy.root_budget.narrower(&policy.project_budget)
            || policy.root_budget.attempts > grant.scope.max_attempts
            || policy.root_budget.parallel > grant.scope.max_concurrency
            || policy.root_budget.runtime_ms > grant.scope.max_duration_ms
            || policy.root_budget.children > grant.scope.max_children
        {
            return Err(PlanError::AuthorityExpansion);
        }
        if policy.connections.is_empty()
            || policy.workspaces.is_empty()
            || policy.rules.is_empty()
            || policy.contracts.is_empty()
            || policy.resources.global == 0
            || policy.resources.global > 1000
            || policy.acceptance_rules.is_empty()
            || policy.required_goal_outputs.is_empty()
        {
            return Err(PlanError::InvalidContract);
        }
        if [
            policy.connections.len(),
            policy.workspaces.len(),
            policy.rules.len(),
            policy.contracts.len(),
            policy.external_inputs.len(),
            policy.external_writes.len(),
            policy.external_actions.len(),
            policy.required_goal_outputs.len(),
        ]
        .into_iter()
        .any(|n| n > MAX_REFS)
        {
            return Err(PlanError::Limit);
        }
        for (key, c) in &policy.connections {
            if !id(key)
                || !id(&c.provider_quota_key)
                || !grant.scope.connection_ids.contains(key)
                || grant.connection_bindings.get(key) != Some(&c.binding)
                || !hash(&c.binding.executable_sha256)
                || !policy
                    .resources
                    .providers
                    .contains_key(&c.provider_quota_key)
            {
                return Err(PlanError::AuthorityExpansion);
            }
        }
        for (key, w) in &policy.workspaces {
            let pinned = grant
                .workspace_bindings
                .get(key)
                .ok_or(PlanError::AuthorityExpansion)?;
            if !id(key)
                || !grant.scope.workspace_ids.contains(key)
                || !grant.scope.host_ids.contains(&w.host_id)
                || pinned.canonical_root.to_str() != Some(w.canonical_root.as_str())
                || pinned.device != w.device
                || pinned.inode != w.inode
                || !w.canonical_root.starts_with('/')
                || w.canonical_root.contains('\\')
                || w.writable_prefixes.len() > MAX_REFS
                || w.writable_prefixes.iter().any(|p| !path(p))
                || !policy.resources.hosts.contains_key(&w.host_id)
                || !policy.resources.workspaces.contains_key(key)
            {
                return Err(PlanError::AuthorityExpansion);
            }
        }
        for (key, rule) in &policy.rules {
            if !id(key) || !hash(&rule.policy_digest) {
                return Err(PlanError::InvalidContract);
            }
        }
        for (key, c) in &policy.contracts {
            if !id(key)
                || c.required_validators.is_empty()
                || c.required_validators.len() > MAX_REFS
                || c.required_validators
                    .iter()
                    .any(|r| !policy.rules.contains_key(r))
            {
                return Err(PlanError::MissingReference);
            }
        }
        for (key, input) in &policy.external_inputs {
            if !id(key)
                || !hash(&input.sha256)
                || !input.verified
                || !policy.contracts.contains_key(&input.contract_id)
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        for (key, w) in &policy.external_writes {
            if !id(key) || !id(&w.conflict_group) || !grant.scope.recipients.contains(&w.recipient)
            {
                return Err(PlanError::AuthorityExpansion);
            }
        }
        let mut canonical_actions = BTreeSet::new();
        for (key, action) in &policy.external_actions {
            let Executor::Connection {
                binding,
                program_sha256,
            } = &action.executor
            else {
                return Err(PlanError::AuthorityExpansion);
            };
            let c = &policy
                .connections
                .get(&binding.connection_id)
                .ok_or(PlanError::AuthorityExpansion)?
                .binding;
            let w = policy
                .workspaces
                .get(&binding.workspace_id)
                .ok_or(PlanError::AuthorityExpansion)?;
            if action.payload_sha256 != digest(&(&action.executor, &action.request))?
                || !id(&action.request.operation)
                || !grant.scope.recipients.contains(&action.request.recipient)
                || serde_json::to_vec(&action.request)
                    .map_err(|_| PlanError::InvalidContract)?
                    .len()
                    > 16384
                || program_sha256 != &c.executable_sha256
                || binding.environment_id != c.environment_id
                || binding.identity_ref != c.identity_ref
                || binding.host_id != c.host_id
                || binding.host_id != w.host_id
                || binding.config_revision != c.config_revision
                || binding.capability_snapshot_id != c.capability_snapshot_id
                || binding.input_revision != policy.input_revision
                || binding.policy_revision != format!("{}:{}", grant.id, grant.revision)
                || action.request.resource_ids.iter().any(|id| {
                    policy
                        .external_writes
                        .get(id)
                        .is_none_or(|p| p.recipient != action.request.recipient)
                })
            {
                return Err(PlanError::AuthorityExpansion);
            }
            if !id(key)
                || !hash(&action.payload_sha256)
                || action.request.resource_ids.is_empty()
                || action.request.resource_ids.len() > MAX_REFS
                || action
                    .request
                    .resource_ids
                    .iter()
                    .any(|r| !policy.external_writes.contains_key(r))
            {
                return Err(PlanError::AuthorityExpansion);
            }
        }
        for action in policy.external_actions.values() {
            // Conservative canonicalization: renaming a host action or using a
            // resource alias cannot turn the same payload into another action.
            let targets: BTreeSet<_> = action
                .request
                .resource_ids
                .iter()
                .map(|r| &policy.external_writes[r].conflict_group)
                .collect();
            let semantic = digest(&(
                &action.request.operation,
                &action.request.recipient,
                targets,
                &action.request.payload,
            ))?;
            if !canonical_actions.insert(semantic) {
                return Err(PlanError::Duplicate);
            }
        }
        for (key, contract) in &policy.required_goal_outputs {
            if !id(key) || !policy.contracts.contains_key(contract) {
                return Err(PlanError::MissingReference);
            }
        }
        if policy
            .acceptance_rules
            .iter()
            .any(|r| !policy.rules.get(r).is_some_and(|r| r.goal_acceptance))
        {
            return Err(PlanError::AuthorityExpansion);
        }
        if policy
            .resources
            .hosts
            .values()
            .chain(policy.resources.providers.values())
            .chain(policy.resources.workspaces.values())
            .any(|v| *v == 0 || *v > 1000)
        {
            return Err(PlanError::InvalidContract);
        }
        let goal = GoalBinding {
            project_id: task.project_id.clone(),
            root_task_id: policy.root_task_id.clone(),
            task_id: task.id.clone(),
            spec_revision: task.spec_revision,
            input_revision: policy.input_revision.clone(),
        };
        let scope_digest = digest(&(&goal, grant, &policy))?;
        Ok(Self {
            goal,
            revision: grant.revision,
            approval_id: grant.approval_id.clone(),
            expires_at_ms: grant.expires_at_ms,
            grant: grant.clone(),
            policy,
            digest: scope_digest,
        })
    }
    pub fn goal(&self) -> &GoalBinding {
        &self.goal
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    /// Product Store only: call after checking its separate native Goal proposal,
    /// consumed confirmation/result and exact persisted grant revision. This is
    /// not a renderer approval constructor or a label authentication mechanism.
    pub(crate) fn bind_native_goal_approval(mut self, approval: &str) -> PlanResult<Self> {
        if !id(approval) {
            return Err(PlanError::InvalidContract);
        }
        self.digest = digest(&(&self.digest, approval))?;
        self.approval_id = approval.into();
        Ok(self)
    }
    pub fn approval_id(&self) -> &str {
        &self.approval_id
    }
}
/// A structural/semantic plan, never a dispatched or accepted result.
#[derive(Clone, Debug)]
pub struct ValidatedPlan {
    proposal: GoalPlanProposal,
    scope: ApprovedGoalScope,
    digest: String,
    order: Vec<String>,
}
impl ValidatedPlan {
    pub fn initial(scope: &ApprovedGoalScope, proposal: GoalPlanProposal) -> PlanResult<Self> {
        validate(scope, proposal, None)
    }
    pub fn revise(
        &self,
        scope: &ApprovedGoalScope,
        proposal: GoalPlanProposal,
    ) -> PlanResult<Self> {
        validate(scope, proposal, Some(self))
    }
    pub fn proposal(&self) -> &GoalPlanProposal {
        &self.proposal
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn topological_order(&self) -> &[String] {
        &self.order
    }
    pub fn approved_external_actions(&self) -> &BTreeMap<String, ApprovedExternalAction> {
        &self.scope.policy.external_actions
    }
    pub fn evidence_binding(&self) -> EvidenceBinding {
        EvidenceBinding {
            goal: self.proposal.goal.clone(),
            plan_digest: self.digest.clone(),
            plan_revision: self.proposal.revision,
        }
    }
}
fn validate(
    scope: &ApprovedGoalScope,
    p: GoalPlanProposal,
    previous: Option<&ValidatedPlan>,
) -> PlanResult<ValidatedPlan> {
    if p.schema_version != 1
        || !id(&p.plan_id)
        || p.goal != scope.goal
        || p.scope_revision != scope.revision
    {
        return Err(PlanError::InputChanged);
    }
    match previous {
        None if p.revision == 1 && p.expected_revision == 0 => {}
        Some(old)
            if old.scope.digest == scope.digest
                && p.plan_id == old.proposal.plan_id
                && p.expected_revision == old.proposal.revision
                && old.proposal.revision.checked_add(1) == Some(p.revision) => {}
        _ => return Err(PlanError::StaleRevision),
    }
    if p.nodes.is_empty()
        || p.nodes.len() > scope.policy.max_nodes as usize
        || !ids(&p.acceptance_rules)
        || p.acceptance_rules.iter().cloned().collect::<BTreeSet<_>>()
            != scope.policy.acceptance_rules
    {
        return Err(PlanError::AuthorityExpansion);
    }
    let mut nodes = BTreeMap::new();
    let mut outputs = BTreeMap::new();
    let mut keys = BTreeSet::new();
    let mut action_ids = BTreeSet::new();
    for n in &p.nodes {
        if !id(&n.id)
            || n.goal.trim().is_empty()
            || n.goal.len() > 16384
            || !id(&n.idempotency_key)
            || !ids(&n.dependencies)
            || !ids(&n.recipients)
            || !ids(&n.required_validators)
            || n.required_inputs.len() > MAX_REFS
            || n.outputs.len() > MAX_REFS
            || n.write_set.len() > MAX_REFS
        {
            return Err(PlanError::InvalidContract);
        }
        if nodes.insert(n.id.clone(), n).is_some() || !keys.insert(n.idempotency_key.clone()) {
            return Err(PlanError::Duplicate);
        }
        if n.reservation.runtime_ms == 0
            || n.reservation.runtime_ms > n.budget.runtime_ms
            || n.reservation.output_bytes == 0
            || n.reservation.output_bytes > n.budget.output_bytes
            || !bounded(n.reservation.tokens, n.budget.tokens)
            || !bounded(n.reservation.cost_microunits, n.budget.cost_microunits)
            || !n.budget.narrower(&scope.policy.root_budget)
            || n.retry.max_attempts == 0
            || n.retry.max_attempts > n.budget.attempts
            || n.retry.max_reworks > n.budget.reworks
            || n.retry.retry_classes.len() > 3
            || n.retry.retry_classes.iter().any(|r| {
                !matches!(
                    r,
                    FailureClass::BeforeDispatchTransient
                        | FailureClass::CrashedNoSideEffect
                        | FailureClass::ValidationFailed
                )
            })
        {
            return Err(PlanError::AuthorityExpansion);
        }
        if n.side_effect == SideEffect::Production
            || n.recipients
                .iter()
                .any(|r| !scope.grant.scope.recipients.contains(r))
        {
            return Err(PlanError::AuthorityExpansion);
        }
        let workspace = scope
            .policy
            .workspaces
            .get(&n.workspace_id)
            .ok_or(PlanError::AuthorityExpansion)?;
        match (&n.kind, &n.executor) {
            (
                NodeKind::Work | NodeKind::Summary,
                Executor::Connection {
                    binding,
                    program_sha256,
                },
            ) => {
                let permit = scope
                    .policy
                    .connections
                    .get(&binding.connection_id)
                    .ok_or(PlanError::AuthorityExpansion)?;
                let c = &permit.binding;
                if binding.workspace_id != n.workspace_id
                    || binding.input_revision != scope.goal.input_revision
                    || binding.policy_revision != format!("{}:{}", scope.grant.id, scope.revision)
                    || binding.environment_id != c.environment_id
                    || binding.identity_ref != c.identity_ref
                    || binding.host_id != c.host_id
                    || binding.host_id != workspace.host_id
                    || binding.config_revision != c.config_revision
                    || binding.capability_snapshot_id != c.capability_snapshot_id
                    || program_sha256 != &c.executable_sha256
                {
                    return Err(PlanError::AuthorityExpansion);
                }
            }
            (NodeKind::Validate, Executor::Validator { rule_id })
                if scope
                    .policy
                    .rules
                    .get(rule_id)
                    .is_some_and(|r| r.kind == RuleKind::Deterministic)
                    && n.required_validators.contains(rule_id) => {}
            (NodeKind::OwnerApproval, Executor::Owner { rule_id })
                if scope
                    .policy
                    .rules
                    .get(rule_id)
                    .is_some_and(|r| r.kind == RuleKind::Owner)
                    && n.required_validators.contains(rule_id) => {}
            _ => return Err(PlanError::AuthorityExpansion),
        }
        for r in &n.required_validators {
            if !scope.policy.rules.contains_key(r) {
                return Err(PlanError::MissingReference);
            }
        }
        if n.side_effect == SideEffect::AdvisoryReadOnly && !n.write_set.is_empty() {
            return Err(PlanError::InvalidContract);
        }
        if n.side_effect == SideEffect::WorkspaceWrite && n.write_set.is_empty() {
            return Err(PlanError::InvalidContract);
        }
        if n.side_effect == SideEffect::ExternalWrite
            && !n
                .write_set
                .iter()
                .any(|w| matches!(w, WriteTarget::External { .. }))
        {
            return Err(PlanError::InvalidContract);
        }
        if matches!(n.kind, NodeKind::Validate | NodeKind::OwnerApproval)
            && n.side_effect != SideEffect::AdvisoryReadOnly
        {
            return Err(PlanError::InvalidContract);
        }
        match (&n.side_effect, &n.external_action_id) {
            (SideEffect::ExternalWrite, Some(action_id)) => {
                let action = scope
                    .policy
                    .external_actions
                    .get(action_id)
                    .ok_or(PlanError::AuthorityExpansion)?;
                let resources: BTreeSet<_> = n
                    .write_set
                    .iter()
                    .filter_map(|w| match w {
                        WriteTarget::External { resource_id } => Some(resource_id.clone()),
                        _ => None,
                    })
                    .collect();
                if resources != action.request.resource_ids
                    || n.executor != action.executor
                    || !action_ids.insert(action_id.clone())
                {
                    return Err(PlanError::AuthorityExpansion);
                }
            }
            (SideEffect::ExternalWrite, None) => return Err(PlanError::AuthorityExpansion),
            (_, Some(_)) => return Err(PlanError::InvalidContract),
            (_, None) => {}
        }
        for w in &n.write_set {
            match w {
                WriteTarget::Workspace { path: p }
                    if path(p)
                        && workspace.writable_prefixes.iter().any(|r| beneath(p, r))
                        && n.side_effect != SideEffect::AdvisoryReadOnly => {}
                WriteTarget::External { resource_id }
                    if n.side_effect == SideEffect::ExternalWrite
                        && scope
                            .policy
                            .external_writes
                            .get(resource_id)
                            .is_some_and(|w| n.recipients.contains(&w.recipient)) => {}
                _ => return Err(PlanError::AuthorityExpansion),
            }
        }
        match &n.executor {
            Executor::Connection { .. } => {
                if n.outputs.is_empty() {
                    return Err(PlanError::InvalidContract);
                }
                for o in &n.outputs {
                    let c = scope
                        .policy
                        .contracts
                        .get(&o.contract_id)
                        .ok_or(PlanError::MissingReference)?;
                    if c.required_validators
                        .iter()
                        .any(|r| !n.required_validators.contains(r))
                    {
                        return Err(PlanError::MissingReference);
                    }
                }
            }
            Executor::Validator { rule_id } | Executor::Owner { rule_id } => {
                if n.side_effect != SideEffect::AdvisoryReadOnly
                    || !n.write_set.is_empty()
                    || n.required_validators != [rule_id.clone()]
                    || n.required_inputs.is_empty()
                    || !n.outputs.is_empty()
                {
                    return Err(PlanError::InvalidContract);
                }
            }
        }
        for o in &n.outputs {
            if !id(&o.artifact_id) || !scope.policy.contracts.contains_key(&o.contract_id) {
                return Err(PlanError::MissingReference);
            }
            if scope.policy.external_inputs.contains_key(&o.artifact_id) {
                return Err(PlanError::Duplicate);
            }
            if outputs
                .insert(o.artifact_id.clone(), (&n.id, &o.contract_id))
                .is_some()
            {
                return Err(PlanError::Duplicate);
            }
        }
    }
    let mut edges: BTreeMap<String, BTreeSet<String>> =
        nodes.keys().map(|k| (k.clone(), BTreeSet::new())).collect();
    for n in &p.nodes {
        let mut parents = BTreeSet::new();
        let mut current = n;
        let mut depth = 0;
        while let Some(parent) = &current.parent_id {
            if !parents.insert(parent) {
                return Err(PlanError::Cycle);
            }
            current = nodes.get(parent).ok_or(PlanError::MissingReference)?;
            depth += 1;
            if depth > scope.policy.max_depth {
                return Err(PlanError::Limit);
            }
        }
        if let Some(parent) = &n.parent_id {
            let parent = nodes[parent];
            if !n.budget.narrower(&parent.budget)
                || n.recipients.iter().any(|r| !parent.recipients.contains(r))
            {
                return Err(PlanError::AuthorityExpansion);
            }
        }
        for dep in n.dependencies.iter().chain(n.parent_id.iter()) {
            if !nodes.contains_key(dep) {
                return Err(PlanError::MissingReference);
            }
            edges.get_mut(&n.id).unwrap().insert(dep.clone());
        }
        let mut inputs = BTreeSet::new();
        for input in &n.required_inputs {
            if input.goal_task_id != scope.goal.task_id
                || !inputs.insert(input.artifact_id.clone())
                || !scope.policy.contracts.contains_key(&input.contract_id)
            {
                return Err(PlanError::InputChanged);
            }
            match &input.source {
                InputSource::ApprovedExternal { sha256 } => {
                    let e = scope
                        .policy
                        .external_inputs
                        .get(&input.artifact_id)
                        .ok_or(PlanError::MissingReference)?;
                    if !e.verified || &e.sha256 != sha256 || e.contract_id != input.contract_id {
                        return Err(PlanError::InputChanged);
                    }
                }
                InputSource::Node {
                    node_id,
                    candidate_for_validation,
                } => {
                    if outputs.get(&input.artifact_id) != Some(&(node_id, &input.contract_id)) {
                        return Err(PlanError::MissingReference);
                    }
                    if *candidate_for_validation
                        && !((n.kind == NodeKind::Validate || n.kind == NodeKind::OwnerApproval)
                            && matches!(&n.executor,Executor::Validator{rule_id}|Executor::Owner{rule_id} if scope.policy.contracts[&input.contract_id].required_validators.contains(rule_id)))
                    {
                        return Err(PlanError::AuthorityExpansion);
                    }
                    edges.get_mut(&n.id).unwrap().insert(node_id.clone());
                }
            }
        }
    }
    for (artifact, contract) in &scope.policy.required_goal_outputs {
        if outputs.get(artifact).map(|(_, c)| *c) != Some(contract) {
            return Err(PlanError::MissingReference);
        }
    }
    // Kahn's algorithm over both data/dependency and lineage edges. Rework is a
    // bounded new attempt, never a backwards graph edge.
    let mut ready: VecDeque<_> = edges
        .iter()
        .filter(|(_, deps)| deps.is_empty())
        .map(|(id, _)| id.clone())
        .collect();
    let mut order = Vec::new();
    while let Some(id) = ready.pop_front() {
        order.push(id.clone());
        for (node, deps) in &mut edges {
            if deps.remove(&id) && deps.is_empty() {
                ready.push_back(node.clone());
            }
        }
    }
    if order.len() != nodes.len() {
        return Err(PlanError::Cycle);
    }
    let plan_digest = digest(&(scope.digest.as_str(), &p))?;
    Ok(ValidatedPlan {
        proposal: p,
        scope: scope.clone(),
        digest: plan_digest,
        order,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopProof {
    ConfirmedNotStarted,
    ConfirmedStopped,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeState {
    NotStarted,
    Running,
    WaitingChild,
    WaitingApproval,
    ExecutionSucceeded,
    Failed {
        class: FailureClass,
        stop: StopProof,
    },
    CancelRequested,
    Cancelled,
    OutcomeUnknown,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceBinding {
    pub goal: GoalBinding,
    pub plan_digest: String,
    pub plan_revision: u64,
}
/// An actual attempt's resolved input identity, also returned for atomic admission.
/// This host-only representation never deserializes proposed Agent evidence.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedInputSource {
    NodeOutput {
        node_id: String,
        attempt_id: String,
        sha256: String,
    },
    ApprovedExternal {
        scope_revision: u64,
        sha256: String,
    },
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ResolvedInput {
    pub artifact_id: String,
    pub contract_id: String,
    pub source: ResolvedInputSource,
}
#[derive(Clone, Debug)]
pub struct ConsumedInputReceipt {
    pub evidence: EvidenceBinding,
    pub consumer_node_id: String,
    pub consumer_attempt_id: String,
    pub input: ResolvedInput,
}
#[derive(Clone, Debug)]
pub struct NodeObservation {
    pub evidence: EvidenceBinding,
    pub state: NodeState,
    pub attempt_id: Option<String>,
    pub attempts_started: u32,
    pub reworks_used: u32,
    pub consumed_inputs: Vec<ConsumedInputReceipt>,
}
#[derive(Clone, Debug)]
pub struct ArtifactReceipt {
    pub evidence: EvidenceBinding,
    pub node_id: String,
    pub attempt_id: String,
    pub artifact_id: String,
    pub sha256: String,
}
#[derive(Clone, Debug)]
pub struct ValidationReceipt {
    pub evidence: EvidenceBinding,
    pub input: ResolvedInput,
    pub rule_id: String,
    pub policy_digest: String,
    pub passed: bool,
}
#[derive(Clone, Debug, Default)]
pub struct BudgetUsage {
    pub attempts_started: u32,
    pub active_slots: u32,
    pub runtime_ms_reserved: u64,
    pub output_bytes_reserved: u64,
    pub children_started: u32,
    pub reworks_used: u32,
    pub tokens: Option<u64>,
    pub cost_microunits: Option<u64>,
}
#[derive(Clone, Debug, Default)]
pub struct ResourceUsage {
    pub global: u32,
    pub hosts: BTreeMap<String, u32>,
    pub providers: BTreeMap<String, u32>,
    pub workspaces: BTreeMap<String, u32>,
}
/// All fields are host-derived current observations; this input is deliberately
/// not Deserialize. Missing/old receipts are not synthesized from Agent prose.
#[derive(Clone, Debug)]
pub struct UnresolvedAction {
    pub root_task_id: String,
    pub action_id: String,
    pub attempt_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalActionState {
    NeverStarted,
    /// Reconciled against the receiver/actual dispatch journal, not process exit.
    NoEffect,
    InFlight,
    OutcomeUnknown,
    EffectConfirmed,
}
/// Complete durable root journal includes known effects as well as unknown ones.
#[derive(Clone, Debug)]
pub struct ExternalActionObservation {
    pub project_id: String,
    pub root_task_id: String,
    pub action_id: String,
    pub payload_sha256: String,
    pub state: ExternalActionState,
}
/// Host-verified physical isolation, bound to this exact plan and both pinned roots.
#[derive(Clone, Debug)]
pub struct IsolationProof {
    pub evidence: EvidenceBinding,
    pub left_workspace: String,
    pub right_workspace: String,
    pub left: WorkspacePermit,
    pub right: WorkspacePermit,
}
/// Host-verified actual write footprint, not an Agent's read-only label.
#[derive(Clone, Debug)]
pub struct WriteSetProof {
    pub evidence: EvidenceBinding,
    pub node_id: String,
    pub workspace_id: String,
    pub write_set: Vec<WriteTarget>,
}
#[derive(Clone, Debug)]
pub struct HostPlanSnapshot {
    pub goal: GoalBinding,
    pub plan_digest: String,
    pub scope_revision: u64,
    pub ledger_revision: u64,
    pub now_ms: i64,
    pub revoked: bool,
    pub cancelled: bool,
    pub nodes: BTreeMap<String, NodeObservation>,
    pub artifacts: Vec<ArtifactReceipt>,
    pub validations: Vec<ValidationReceipt>,
    pub root_usage: BudgetUsage,
    pub project_usage: BudgetUsage,
    /// Complete cumulative node-plus-descendants ledgers, including older revisions.
    pub subtree_usage: BTreeMap<String, BudgetUsage>,
    pub resources: ResourceUsage,
    /// Complete root-goal journal projection, including older plans/actions.
    pub unresolved_actions: Vec<UnresolvedAction>,
    pub external_actions: BTreeMap<String, ExternalActionObservation>,
    /// Ordered pairs from an actual host isolation check, never a proposal flag.
    pub verified_isolation: Vec<IsolationProof>,
    /// Nodes whose actual bounded write set has been independently verified.
    pub verified_write_sets: Vec<WriteSetProof>,
}
#[derive(Clone, Debug)]
pub struct TrustedPlanSnapshot {
    host: HostPlanSnapshot,
}
impl TrustedPlanSnapshot {
    /// Caller must obtain these values from one consistent, authenticated host
    /// journal/Store view: all root actions (including old plans), current grant
    /// revocation, cumulative ledgers, exact attempt-owned artifact receipts and
    /// independently executed validator/owner decisions. This constructor checks
    /// consistency; it cannot authenticate invented Rust values or Agent prose.
    /// Isolation includes shared repository metadata, subprocess writes and alias
    /// resources. Distinct strings, directories or a read-only label are no proof.
    pub fn from_verified_host(plan: &ValidatedPlan, host: HostPlanSnapshot) -> PlanResult<Self> {
        if host.goal != plan.proposal.goal
            || host.plan_digest != plan.digest
            || host.scope_revision != plan.scope.revision
            || host.ledger_revision == 0
        {
            return Err(PlanError::StaleEvidence);
        }
        if host.nodes.len() != plan.proposal.nodes.len()
            || host
                .nodes
                .keys()
                .any(|id| !plan.proposal.nodes.iter().any(|n| &n.id == id))
            || host.artifacts.len() > 4096
            || host.validations.len() > 4096
        {
            return Err(PlanError::StaleEvidence);
        }
        let expected = plan.evidence_binding();
        if host.unresolved_actions.len() > 4096
            || host.unresolved_actions.iter().any(|a| {
                a.root_task_id != plan.proposal.goal.root_task_id
                    || !id(&a.action_id)
                    || !id(&a.attempt_id)
            })
        {
            return Err(PlanError::StaleEvidence);
        }
        if host.external_actions.len() > 4096
            || plan.scope.policy.external_actions.iter().any(|(k, p)| {
                host.external_actions
                    .get(k)
                    .is_none_or(|a| a.payload_sha256 != p.payload_sha256)
            })
            || host.external_actions.iter().any(|(k, a)| {
                k != &a.action_id
                    || !id(k)
                    || !hash(&a.payload_sha256)
                    || a.project_id != plan.proposal.goal.project_id
                    || a.root_task_id != plan.proposal.goal.root_task_id
            })
        {
            return Err(PlanError::StaleEvidence);
        }
        let mut attempts = 0u32;
        let mut reworks = 0u32;
        let mut local = ResourceUsage::default();
        for (id, o) in &host.nodes {
            if o.evidence != expected {
                return Err(PlanError::StaleEvidence);
            }
            attempts = attempts
                .checked_add(o.attempts_started)
                .ok_or(PlanError::Limit)?;
            reworks = reworks
                .checked_add(o.reworks_used)
                .ok_or(PlanError::Limit)?;
            if matches!(
                o.state,
                NodeState::Running
                    | NodeState::CancelRequested
                    | NodeState::OutcomeUnknown
                    | NodeState::Failed {
                        stop: StopProof::Unknown,
                        ..
                    }
            ) {
                let node = plan
                    .proposal
                    .nodes
                    .iter()
                    .find(|n| &n.id == id)
                    .ok_or(PlanError::StaleEvidence)?;
                add_resource(plan, &mut local, node).ok_or(PlanError::Limit)?;
            }
            if o.attempts_started > 100_000
                || o.reworks_used > 1000
                || (o.state == NodeState::NotStarted
                    && (o.attempts_started != 0 || o.attempt_id.is_some()))
                || (o.state != NodeState::NotStarted
                    && (o.attempts_started == 0
                        || !o.attempt_id.as_deref().is_some_and(crate::goal_plan::id)))
            {
                return Err(PlanError::StaleEvidence);
            }
            let node = plan
                .proposal
                .nodes
                .iter()
                .find(|n| &n.id == id)
                .ok_or(PlanError::StaleEvidence)?;
            let never_started = matches!(
                o.state,
                NodeState::NotStarted
                    | NodeState::Failed {
                        stop: StopProof::ConfirmedNotStarted,
                        ..
                    }
            );
            if (o.state == NodeState::NotStarted && !o.consumed_inputs.is_empty())
                || (!never_started || !o.consumed_inputs.is_empty())
                    && (o.consumed_inputs.len() != node.required_inputs.len()
                        || node
                            .required_inputs
                            .iter()
                            .zip(&o.consumed_inputs)
                            .any(|(i, c)| {
                                c.evidence != expected
                                    || c.consumer_node_id != node.id
                                    || Some(&c.consumer_attempt_id) != o.attempt_id.as_ref()
                                    || !consumption_matches_contract(plan, i, &c.input)
                            }))
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        if host.root_usage.attempts_started < attempts
            || host.root_usage.reworks_used < reworks
            || host.root_usage.active_slots < local.global
            || host.project_usage.attempts_started < host.root_usage.attempts_started
            || host.project_usage.active_slots < host.root_usage.active_slots
            || host.project_usage.runtime_ms_reserved < host.root_usage.runtime_ms_reserved
            || host.project_usage.output_bytes_reserved < host.root_usage.output_bytes_reserved
            || host.project_usage.children_started < host.root_usage.children_started
            || host.project_usage.reworks_used < host.root_usage.reworks_used
            || host.resources.global < local.global
        {
            return Err(PlanError::StaleEvidence);
        }
        for (actual, required) in [
            (&host.resources.hosts, &local.hosts),
            (&host.resources.providers, &local.providers),
            (&host.resources.workspaces, &local.workspaces),
        ] {
            if actual.len() > 4096
                || required
                    .iter()
                    .any(|(k, v)| actual.get(k).copied().unwrap_or(0) < *v)
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        if host.subtree_usage.len() != host.nodes.len()
            || host
                .subtree_usage
                .keys()
                .any(|k| !host.nodes.contains_key(k))
        {
            return Err(PlanError::StaleEvidence);
        }
        let mut minimum: BTreeMap<String, BudgetUsage> = host
            .nodes
            .keys()
            .map(|k| (k.clone(), BudgetUsage::default()))
            .collect();
        let mut root_children = 0u32;
        for n in &plan.proposal.nodes {
            let o = &host.nodes[&n.id];
            if n.parent_id.is_some() && o.attempts_started > 0 {
                root_children = root_children.checked_add(1).ok_or(PlanError::Limit)?;
            }
            for a in ancestors(plan, n) {
                let m = minimum.get_mut(&a.id).unwrap();
                m.attempts_started = m
                    .attempts_started
                    .checked_add(o.attempts_started)
                    .ok_or(PlanError::Limit)?;
                m.reworks_used = m
                    .reworks_used
                    .checked_add(o.reworks_used)
                    .ok_or(PlanError::Limit)?;
                if a.id != n.id && o.attempts_started > 0 {
                    m.children_started =
                        m.children_started.checked_add(1).ok_or(PlanError::Limit)?;
                }
                if matches!(
                    o.state,
                    NodeState::Running
                        | NodeState::CancelRequested
                        | NodeState::OutcomeUnknown
                        | NodeState::Failed {
                            stop: StopProof::Unknown,
                            ..
                        }
                ) {
                    m.active_slots = m.active_slots.checked_add(1).ok_or(PlanError::Limit)?;
                }
            }
        }
        if host.root_usage.children_started < root_children {
            return Err(PlanError::StaleEvidence);
        }
        for (k, m) in minimum {
            let u = &host.subtree_usage[&k];
            if u.attempts_started < m.attempts_started
                || u.reworks_used < m.reworks_used
                || u.children_started < m.children_started
                || u.active_slots < m.active_slots
                || u.attempts_started > host.root_usage.attempts_started
                || u.active_slots > host.root_usage.active_slots
                || u.runtime_ms_reserved > host.root_usage.runtime_ms_reserved
                || u.output_bytes_reserved > host.root_usage.output_bytes_reserved
                || u.children_started > host.root_usage.children_started
                || u.reworks_used > host.root_usage.reworks_used
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        let mut artifacts = BTreeSet::new();
        for a in &host.artifacts {
            let o = host.nodes.get(&a.node_id).ok_or(PlanError::StaleEvidence)?;
            if a.evidence != expected
                || o.attempt_id.as_ref() != Some(&a.attempt_id)
                || !hash(&a.sha256)
                || !plan.proposal.nodes.iter().any(|n| {
                    n.id == a.node_id && n.outputs.iter().any(|x| x.artifact_id == a.artifact_id)
                })
                || !artifacts.insert(a.artifact_id.clone())
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        let mut validations = BTreeSet::new();
        for v in &host.validations {
            if v.evidence != expected
                || resolve_artifact(plan, &host, &v.input.artifact_id, &v.input.contract_id)
                    .as_ref()
                    != Some(&v.input)
                || plan
                    .scope
                    .policy
                    .rules
                    .get(&v.rule_id)
                    .map(|r| &r.policy_digest)
                    != Some(&v.policy_digest)
                || !validations.insert((v.input.artifact_id.clone(), v.rule_id.clone()))
            {
                return Err(PlanError::StaleEvidence);
            }
        }
        if host.verified_isolation.len() > MAX_NODES * MAX_NODES
            || host.verified_isolation.iter().any(|p| {
                p.evidence != expected
                    || p.left_workspace >= p.right_workspace
                    || plan.scope.policy.workspaces.get(&p.left_workspace) != Some(&p.left)
                    || plan.scope.policy.workspaces.get(&p.right_workspace) != Some(&p.right)
            })
            || host.verified_write_sets.len() > MAX_NODES
            || host.verified_write_sets.iter().any(|p| {
                p.evidence != expected
                    || !plan.proposal.nodes.iter().any(|n| {
                        n.id == p.node_id
                            && n.workspace_id == p.workspace_id
                            && n.write_set == p.write_set
                    })
            })
        {
            return Err(PlanError::StaleEvidence);
        }
        Ok(Self { host })
    }
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum BlockReason {
    RevokedOrExpired,
    Cancelled,
    Dependencies,
    InputsNotValidated,
    ConsumedInputsChanged,
    ExternalEffectAlreadyApplied,
    ParentNotReady,
    ReconcileUnknown,
    StoppedEvidenceMissing,
    RetryNotAllowed,
    RetryLimit,
    AwaitingOwner,
    RootBudget,
    SubtreeBudget,
    ProjectBudget,
    MeteringUnknown,
    ResourceLimit,
    WriteConflict,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ResolvedExternalAction {
    pub action_id: String,
    pub payload_sha256: String,
    pub request: ExternalActionRequest,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Admission {
    pub node_id: String,
    pub next_attempt: u32,
    pub rework: bool,
    pub workspace_id: String,
    pub executor: Executor,
    pub reservation: AttemptReservation,
    pub idempotency_key: String,
    pub resolved_inputs: Vec<ResolvedInput>,
    pub external_action: Option<ResolvedExternalAction>,
}
#[derive(Clone, Debug, Serialize)]
pub struct ReadyDecision {
    pub plan_digest: String,
    pub expected_plan_revision: u64,
    pub expected_ledger_revision: u64,
    pub admissions: Vec<Admission>,
    pub blocked: BTreeMap<String, BlockReason>,
    pub required_checks_satisfied: bool,
    pub requires_atomic_store_commit: bool,
}
fn consumption_matches_contract(
    plan: &ValidatedPlan,
    i: &RequiredInput,
    c: &ResolvedInput,
) -> bool {
    if i.artifact_id != c.artifact_id || i.contract_id != c.contract_id {
        return false;
    }
    match (&i.source, &c.source) {
        (
            InputSource::Node { node_id, .. },
            ResolvedInputSource::NodeOutput {
                node_id: actual,
                attempt_id,
                sha256,
            },
        ) => node_id == actual && id(attempt_id) && hash(sha256),
        (
            InputSource::ApprovedExternal { sha256 },
            ResolvedInputSource::ApprovedExternal {
                scope_revision,
                sha256: actual,
            },
        ) => sha256 == actual && *scope_revision == plan.scope.revision,
        _ => false,
    }
}
fn resolve_artifact(
    plan: &ValidatedPlan,
    snapshot: &HostPlanSnapshot,
    artifact: &str,
    contract: &str,
) -> Option<ResolvedInput> {
    if let Some(e) = plan.scope.policy.external_inputs.get(artifact) {
        if e.contract_id != contract {
            return None;
        }
        return Some(ResolvedInput {
            artifact_id: artifact.into(),
            contract_id: contract.into(),
            source: ResolvedInputSource::ApprovedExternal {
                scope_revision: plan.scope.revision,
                sha256: e.sha256.clone(),
            },
        });
    }
    let a = snapshot
        .artifacts
        .iter()
        .find(|a| a.artifact_id == artifact)?;
    let n = plan.proposal.nodes.iter().find(|n| n.id == a.node_id)?;
    if !n
        .outputs
        .iter()
        .any(|o| o.artifact_id == artifact && o.contract_id == contract)
    {
        return None;
    }
    Some(ResolvedInput {
        artifact_id: artifact.into(),
        contract_id: contract.into(),
        source: ResolvedInputSource::NodeOutput {
            node_id: a.node_id.clone(),
            attempt_id: a.attempt_id.clone(),
            sha256: a.sha256.clone(),
        },
    })
}
fn checked_input(plan: &ValidatedPlan, snapshot: &HostPlanSnapshot, input: &ResolvedInput) -> bool {
    plan.scope.policy.contracts[&input.contract_id]
        .required_validators
        .iter()
        .all(|r| {
            snapshot
                .validations
                .iter()
                .any(|v| &v.input == input && &v.rule_id == r && v.passed)
        })
}
fn checked_artifact(
    plan: &ValidatedPlan,
    snapshot: &HostPlanSnapshot,
    artifact: &str,
    contract: &str,
) -> bool {
    resolve_artifact(plan, snapshot, artifact, contract)
        .is_some_and(|i| checked_input(plan, snapshot, &i))
}
/// Freshness propagates through data/dependency edges without requiring a
/// candidate's yet-to-run validator; execution success alone never upgrades refs.
fn input_freshness(plan: &ValidatedPlan, snapshot: &HostPlanSnapshot) -> BTreeMap<String, bool> {
    let mut fresh = BTreeMap::new();
    for id in &plan.order {
        let n = plan.proposal.nodes.iter().find(|n| &n.id == id).unwrap();
        let o = &snapshot.nodes[id];
        let expected: Option<Vec<_>> = n
            .required_inputs
            .iter()
            .map(|i| resolve_artifact(plan, snapshot, &i.artifact_id, &i.contract_id))
            .collect();
        let current = expected
            .as_ref()
            .is_some_and(|inputs| inputs.iter().eq(o.consumed_inputs.iter().map(|c| &c.input)))
            && n.required_inputs.iter().all(|i| match &i.source {
                InputSource::ApprovedExternal { .. } => true,
                InputSource::Node { node_id, .. } => {
                    snapshot.nodes[node_id].state == NodeState::ExecutionSucceeded && fresh[node_id]
                }
            })
            && n.dependencies
                .iter()
                .all(|d| snapshot.nodes[d].state == NodeState::ExecutionSucceeded && fresh[d])
            && n.parent_id.as_ref().is_none_or(|p| {
                matches!(
                    snapshot.nodes[p].state,
                    NodeState::Running | NodeState::WaitingChild | NodeState::ExecutionSucceeded
                ) && fresh[p]
            });
        fresh.insert(id.clone(), current);
    }
    fresh
}
fn finished(
    plan: &ValidatedPlan,
    snapshot: &HostPlanSnapshot,
    id: &str,
    fresh: &BTreeMap<String, bool>,
) -> bool {
    if snapshot.nodes[id].state != NodeState::ExecutionSucceeded || !fresh[id] {
        return false;
    }
    let node = plan.proposal.nodes.iter().find(|n| n.id == id).unwrap();
    if node
        .external_action_id
        .as_ref()
        .is_some_and(|a| snapshot.external_actions[a].state != ExternalActionState::EffectConfirmed)
    {
        return false;
    }
    let rules_pass = |i: &ResolvedInput, rules: &[String]| {
        rules.iter().all(|r| {
            snapshot
                .validations
                .iter()
                .any(|v| &v.input == i && &v.rule_id == r && v.passed)
        })
    };
    match &node.executor {
        Executor::Connection { .. } => node.outputs.iter().all(|o| {
            resolve_artifact(plan, snapshot, &o.artifact_id, &o.contract_id).is_some_and(|i| {
                checked_input(plan, snapshot, &i) && rules_pass(&i, &node.required_validators)
            })
        }),
        Executor::Validator { .. } | Executor::Owner { .. } => snapshot.nodes[id]
            .consumed_inputs
            .iter()
            .all(|i| rules_pass(&i.input, &node.required_validators)),
    }
}
fn budget_admits(
    limit: &BudgetLimits,
    usage: &BudgetUsage,
    node: &PlanNode,
    rework: bool,
    new_child: bool,
) -> Result<(), BlockReason> {
    if usage
        .attempts_started
        .checked_add(1)
        .is_none_or(|n| n > limit.attempts)
        || usage
            .active_slots
            .checked_add(1)
            .is_none_or(|n| n > limit.parallel)
        || usage
            .runtime_ms_reserved
            .checked_add(node.reservation.runtime_ms)
            .is_none_or(|n| n > limit.runtime_ms)
        || usage
            .output_bytes_reserved
            .checked_add(node.reservation.output_bytes)
            .is_none_or(|n| n > limit.output_bytes)
        || usage
            .children_started
            .checked_add(u32::from(new_child))
            .is_none_or(|n| n > limit.children)
        || usage
            .reworks_used
            .checked_add(u32::from(rework))
            .is_none_or(|n| n > limit.reworks)
    {
        return Err(BlockReason::RootBudget);
    }
    for (cap, used, requested) in [
        (limit.tokens, usage.tokens, node.reservation.tokens),
        (
            limit.cost_microunits,
            usage.cost_microunits,
            node.reservation.cost_microunits,
        ),
    ] {
        if let Some(cap) = cap {
            let used = used.ok_or(BlockReason::MeteringUnknown)?;
            let requested = requested.ok_or(BlockReason::MeteringUnknown)?;
            if used.checked_add(requested).is_none_or(|v| v > cap) {
                return Err(BlockReason::RootBudget);
            }
        }
    }
    Ok(())
}
fn add_budget(
    usage: &mut BudgetUsage,
    node: &PlanNode,
    rework: bool,
    new_child: bool,
) -> Option<()> {
    usage.attempts_started = usage.attempts_started.checked_add(1)?;
    usage.active_slots = usage.active_slots.checked_add(1)?;
    usage.runtime_ms_reserved = usage
        .runtime_ms_reserved
        .checked_add(node.reservation.runtime_ms)?;
    usage.output_bytes_reserved = usage
        .output_bytes_reserved
        .checked_add(node.reservation.output_bytes)?;
    usage.children_started = usage.children_started.checked_add(u32::from(new_child))?;
    usage.reworks_used = usage.reworks_used.checked_add(u32::from(rework))?;
    usage.tokens = match (usage.tokens, node.reservation.tokens) {
        (Some(a), Some(b)) => Some(a.checked_add(b)?),
        _ => None,
    };
    usage.cost_microunits = match (usage.cost_microunits, node.reservation.cost_microunits) {
        (Some(a), Some(b)) => Some(a.checked_add(b)?),
        _ => None,
    };
    Some(())
}
fn provider<'a>(plan: &'a ValidatedPlan, node: &PlanNode) -> Option<&'a str> {
    match &node.executor {
        Executor::Connection { binding, .. } => Some(
            plan.scope.policy.connections[&binding.connection_id]
                .provider_quota_key
                .as_str(),
        ),
        _ => None,
    }
}
fn resources_admit(plan: &ValidatedPlan, usage: &ResourceUsage, node: &PlanNode) -> bool {
    let limits = &plan.scope.policy.resources;
    let host = &plan.scope.policy.workspaces[&node.workspace_id].host_id;
    usage.global < limits.global
        && usage.hosts.get(host).copied().unwrap_or(0) < limits.hosts[host]
        && usage
            .workspaces
            .get(&node.workspace_id)
            .copied()
            .unwrap_or(0)
            < limits.workspaces[&node.workspace_id]
        && provider(plan, node)
            .is_none_or(|p| usage.providers.get(p).copied().unwrap_or(0) < limits.providers[p])
}
fn add_resource(plan: &ValidatedPlan, usage: &mut ResourceUsage, node: &PlanNode) -> Option<()> {
    usage.global = usage.global.checked_add(1)?;
    let host = usage
        .hosts
        .entry(
            plan.scope.policy.workspaces[&node.workspace_id]
                .host_id
                .clone(),
        )
        .or_default();
    *host = host.checked_add(1)?;
    let workspace = usage
        .workspaces
        .entry(node.workspace_id.clone())
        .or_default();
    *workspace = workspace.checked_add(1)?;
    if let Some(p) = provider(plan, node) {
        let provider = usage.providers.entry(p.into()).or_default();
        *provider = provider.checked_add(1)?
    }
    Some(())
}
fn conflict(plan: &ValidatedPlan, snapshot: &HostPlanSnapshot, a: &PlanNode, b: &PlanNode) -> bool {
    if a.workspace_id == b.workspace_id {
        return true;
    }
    let wa = &plan.scope.policy.workspaces[&a.workspace_id];
    let wb = &plan.scope.policy.workspaces[&b.workspace_id];
    let pair = if a.workspace_id < b.workspace_id {
        (a.workspace_id.clone(), b.workspace_id.clone())
    } else {
        (b.workspace_id.clone(), a.workspace_id.clone())
    };
    if !snapshot
        .verified_isolation
        .iter()
        .any(|p| p.left_workspace == pair.0 && p.right_workspace == pair.1)
        || !snapshot
            .verified_write_sets
            .iter()
            .any(|p| p.node_id == a.id)
        || !snapshot
            .verified_write_sets
            .iter()
            .any(|p| p.node_id == b.id)
        || wa.device == wb.device && wa.inode == wb.inode
        || path_overlap(&wa.canonical_root, &wb.canonical_root)
    {
        return true;
    }
    a.write_set.iter().any(|a| {
        b.write_set.iter().any(|b| match (a, b) {
            (
                WriteTarget::External { resource_id: a },
                WriteTarget::External { resource_id: b },
            ) => {
                plan.scope.policy.external_writes[a].conflict_group
                    == plan.scope.policy.external_writes[b].conflict_group
            }
            _ => false,
        })
    })
}
fn ancestors<'a>(plan: &'a ValidatedPlan, node: &'a PlanNode) -> Vec<&'a PlanNode> {
    let mut result = vec![node];
    let mut current = node;
    while let Some(parent) = &current.parent_id {
        current = plan
            .proposal
            .nodes
            .iter()
            .find(|n| &n.id == parent)
            .expect("validated parent");
        result.push(current);
    }
    result
}
/// A deterministic candidate batch only. The daemon must atomically compare plan
/// and ledger revisions, reserve all quotas and recheck authority before dispatch.
/// Waiting parents release execution slots, but remain workspace conflict owners.
pub fn decide_ready(
    plan: &ValidatedPlan,
    snapshot: &TrustedPlanSnapshot,
) -> PlanResult<ReadyDecision> {
    let host = &snapshot.host;
    if host.goal != plan.proposal.goal
        || host.plan_digest != plan.digest
        || host.scope_revision != plan.scope.revision
    {
        return Err(PlanError::StaleEvidence);
    }
    let mut result = ReadyDecision {
        plan_digest: plan.digest.clone(),
        expected_plan_revision: plan.proposal.revision,
        expected_ledger_revision: host.ledger_revision,
        admissions: Vec::new(),
        blocked: BTreeMap::new(),
        required_checks_satisfied: false,
        requires_atomic_store_commit: true,
    };
    let mut root = host.root_usage.clone();
    let mut project = host.project_usage.clone();
    let mut resources = host.resources.clone();
    let mut subtrees = host.subtree_usage.clone();
    let fresh = input_freshness(plan, host);
    let mut occupied: Vec<&PlanNode> = plan
        .proposal
        .nodes
        .iter()
        .filter(|n| {
            matches!(
                host.nodes[&n.id].state,
                NodeState::Running
                    | NodeState::WaitingChild
                    | NodeState::WaitingApproval
                    | NodeState::CancelRequested
                    | NodeState::OutcomeUnknown
            )
        })
        .collect();
    let unresolved = host.external_actions.values().any(|a| {
        matches!(
            a.state,
            ExternalActionState::InFlight | ExternalActionState::OutcomeUnknown
        )
    }) || !host.unresolved_actions.is_empty()
        || host.nodes.values().any(|o| {
            matches!(
                o.state,
                NodeState::OutcomeUnknown
                    | NodeState::CancelRequested
                    | NodeState::Failed {
                        stop: StopProof::Unknown,
                        ..
                    }
            )
        });
    for id in &plan.order {
        let node = plan.proposal.nodes.iter().find(|n| &n.id == id).unwrap();
        let observed = &host.nodes[id];
        let reason = if host.revoked || host.now_ms >= plan.scope.expires_at_ms {
            Some(BlockReason::RevokedOrExpired)
        } else if host.cancelled {
            Some(BlockReason::Cancelled)
        } else if unresolved {
            Some(BlockReason::ReconcileUnknown)
        } else {
            None
        };
        if let Some(reason) = reason {
            result.blocked.insert(id.clone(), reason);
            continue;
        }
        if matches!(
            observed.state,
            NodeState::ExecutionSucceeded
                | NodeState::Running
                | NodeState::WaitingChild
                | NodeState::WaitingApproval
        ) && !fresh[id]
        {
            result
                .blocked
                .insert(id.clone(), BlockReason::ConsumedInputsChanged);
            continue;
        }
        if let Some(action_id) = &node.external_action_id {
            let action = &host.external_actions[action_id];
            if observed.state != NodeState::ExecutionSucceeded
                && action.state == ExternalActionState::EffectConfirmed
            {
                result
                    .blocked
                    .insert(id.clone(), BlockReason::ExternalEffectAlreadyApplied);
                continue;
            }
            if let NodeState::Failed { stop, .. } = &observed.state {
                if action.state != ExternalActionState::NoEffect
                    && !(action.state == ExternalActionState::NeverStarted
                        && *stop == StopProof::ConfirmedNotStarted)
                {
                    result
                        .blocked
                        .insert(id.clone(), BlockReason::ReconcileUnknown);
                    continue;
                }
            }
        }
        let rework = match &observed.state {
            NodeState::NotStarted => false,
            NodeState::OutcomeUnknown | NodeState::CancelRequested => {
                result
                    .blocked
                    .insert(id.clone(), BlockReason::ReconcileUnknown);
                continue;
            }
            NodeState::Failed { class, stop } => {
                if *stop == StopProof::Unknown || matches!(class, FailureClass::DisconnectedUnknown)
                {
                    result
                        .blocked
                        .insert(id.clone(), BlockReason::ReconcileUnknown);
                    continue;
                }
                if matches!(class, FailureClass::InputChanged) {
                    result
                        .blocked
                        .insert(id.clone(), BlockReason::InputsNotValidated);
                    continue;
                }
                if !node.retry.retry_classes.contains(class) {
                    result
                        .blocked
                        .insert(id.clone(), BlockReason::RetryNotAllowed);
                    continue;
                }
                let rework = *class == FailureClass::ValidationFailed;
                if observed.attempts_started >= node.retry.max_attempts
                    || rework && observed.reworks_used >= node.retry.max_reworks
                {
                    result.blocked.insert(id.clone(), BlockReason::RetryLimit);
                    continue;
                }
                rework
            }
            _ => continue,
        };
        if node.parent_id.as_ref().is_some_and(|p| {
            !fresh[p]
                || !matches!(
                    host.nodes[p].state,
                    NodeState::Running | NodeState::WaitingChild | NodeState::ExecutionSucceeded
                )
        }) {
            result
                .blocked
                .insert(id.clone(), BlockReason::ParentNotReady);
            continue;
        }
        if node.dependencies.iter().any(|d| {
            // A validator consumes an explicitly identified unvalidated candidate.
            // Requiring that candidate's final checks here would deadlock validation.
            let candidate = node.required_inputs.iter().any(|i| matches!(&i.source, InputSource::Node {node_id,candidate_for_validation:true} if node_id == d));
            if candidate { host.nodes[d].state != NodeState::ExecutionSucceeded || !fresh[d] } else { !finished(plan, host, d, &fresh) }
        }) {
            result.blocked.insert(id.clone(), BlockReason::Dependencies);
            continue;
        }
        let resolved_inputs: Option<Vec<_>> = node
            .required_inputs
            .iter()
            .map(|i| resolve_artifact(plan, host, &i.artifact_id, &i.contract_id))
            .collect();
        let inputs_ready = resolved_inputs.as_ref().is_some_and(|resolved| node.required_inputs.iter().zip(resolved).all(|(i,r)| {
            let candidate=match &i.source {
                InputSource::Node {node_id,candidate_for_validation} => {
                    if host.nodes[node_id].state != NodeState::ExecutionSucceeded || !fresh[node_id] {return false;}
                    *candidate_for_validation
                }
                InputSource::ApprovedExternal {..} => matches!(&node.executor,Executor::Validator {rule_id}|Executor::Owner {rule_id} if plan.scope.policy.contracts[&i.contract_id].required_validators.contains(rule_id)),
            };
            candidate || checked_input(plan,host,r)
        }));
        if !inputs_ready {
            result
                .blocked
                .insert(id.clone(), BlockReason::InputsNotValidated);
            continue;
        }
        if node.kind == NodeKind::OwnerApproval {
            result
                .blocked
                .insert(id.clone(), BlockReason::AwaitingOwner);
            continue;
        }
        if let Err(reason) = budget_admits(
            &plan.scope.policy.root_budget,
            &root,
            node,
            rework,
            node.parent_id.is_some() && observed.attempts_started == 0,
        ) {
            result.blocked.insert(id.clone(), reason);
            continue;
        }
        if let Err(reason) = budget_admits(
            &plan.scope.policy.project_budget,
            &project,
            node,
            rework,
            node.parent_id.is_some() && observed.attempts_started == 0,
        ) {
            result.blocked.insert(
                id.clone(),
                if reason == BlockReason::MeteringUnknown {
                    reason
                } else {
                    BlockReason::ProjectBudget
                },
            );
            continue;
        }
        let ancestors = ancestors(plan, node);
        if let Some(reason) = ancestors.iter().find_map(|a| {
            budget_admits(
                &a.budget,
                &subtrees[&a.id],
                node,
                rework,
                a.id != node.id && observed.attempts_started == 0,
            )
            .err()
        }) {
            result.blocked.insert(
                id.clone(),
                if reason == BlockReason::MeteringUnknown {
                    reason
                } else {
                    BlockReason::SubtreeBudget
                },
            );
            continue;
        }
        if !resources_admit(plan, &resources, node) {
            result
                .blocked
                .insert(id.clone(), BlockReason::ResourceLimit);
            continue;
        }
        if occupied
            .iter()
            .any(|other| conflict(plan, host, node, other))
        {
            result
                .blocked
                .insert(id.clone(), BlockReason::WriteConflict);
            continue;
        }
        let mut next_root = root.clone();
        let mut next_project = project.clone();
        let mut next_resources = resources.clone();
        let mut next_subtrees = subtrees.clone();
        let mut subtree_overflow = false;
        for a in &ancestors {
            if add_budget(
                next_subtrees.get_mut(&a.id).unwrap(),
                node,
                rework,
                a.id != node.id && observed.attempts_started == 0,
            )
            .is_none()
            {
                subtree_overflow = true;
                break;
            }
        }
        if subtree_overflow
            || add_budget(
                &mut next_root,
                node,
                rework,
                node.parent_id.is_some() && observed.attempts_started == 0,
            )
            .is_none()
            || add_budget(
                &mut next_project,
                node,
                rework,
                node.parent_id.is_some() && observed.attempts_started == 0,
            )
            .is_none()
            || add_resource(plan, &mut next_resources, node).is_none()
        {
            result.blocked.insert(id.clone(), BlockReason::RootBudget);
            continue;
        }
        root = next_root;
        project = next_project;
        resources = next_resources;
        subtrees = next_subtrees;
        occupied.push(node);
        result.admissions.push(Admission {
            node_id: id.clone(),
            next_attempt: observed
                .attempts_started
                .checked_add(1)
                .ok_or(PlanError::Limit)?,
            rework,
            workspace_id: node.workspace_id.clone(),
            executor: node.executor.clone(),
            reservation: node.reservation.clone(),
            resolved_inputs: resolved_inputs.unwrap(),
            external_action: node.external_action_id.as_ref().map(|id| {
                let a = &plan.scope.policy.external_actions[id];
                ResolvedExternalAction {
                    action_id: id.clone(),
                    payload_sha256: a.payload_sha256.clone(),
                    request: a.request.clone(),
                }
            }),
            idempotency_key: if let Some(id) = &node.external_action_id {
                digest(&(
                    &plan.proposal.goal.project_id,
                    &plan.proposal.goal.root_task_id,
                    id,
                    &plan.scope.policy.external_actions[id].payload_sha256,
                ))?
            } else {
                digest(&(
                    plan.digest.as_str(),
                    node.idempotency_key.as_str(),
                    observed
                        .attempts_started
                        .checked_add(1)
                        .ok_or(PlanError::Limit)?,
                ))?
            },
        });
    }
    result.required_checks_satisfied = !unresolved
        && !host.revoked
        && !host.cancelled
        && host.now_ms < plan.scope.expires_at_ms
        && plan
            .proposal
            .nodes
            .iter()
            .all(|n| finished(plan, host, &n.id, &fresh))
        && plan
            .scope
            .policy
            .required_goal_outputs
            .iter()
            .all(|(artifact, contract)| {
                checked_artifact(plan, host, artifact, contract)
                    && plan.scope.policy.acceptance_rules.iter().all(|r| {
                        host.validations.iter().any(|v| {
                            v.input.artifact_id == *artifact && v.rule_id == *r && v.passed
                        })
                    })
            });
    Ok(result)
}
