//! Trusted project configuration approval, publication and immutable Task pins.
use crate::registry::Project;
use crate::sessions::ApprovalProposal;
use crate::storage::{db_error, decode, encode, Store};
use crate::{CoreError, Result};
use agentbaton_config::capabilities::ProviderRevision;
use agentbaton_config::project_import::{
    apply_project_config_update, apply_project_import, inspect_project_config,
    prepare_project_config_update, prepare_project_config_update_binding, prepare_project_import,
    read_imported_project_plan, read_project_config_version, recover_project_config_update,
    recover_project_import, ConfirmedProjectConfigUpdate, ConfirmedProjectImport,
    ProjectCapabilityKey, ProjectCapabilityKind, ProjectConfigUpdateBinding,
    ProjectConfigUpdatePreview, ProjectConfigUpdateRequest, ProjectConfigUpdateRule,
    ProjectConfigVersionBinding, ProjectImportBinding, ProjectImportChoice, ProjectImportDecision,
    ProjectImportPolicy, ProjectImportRequest, ProjectImportScope, ProjectImportState,
    ProjectSource,
};
use agentbaton_config::runtime::{RuntimePlan, SourceProvenance};
use agentbaton_workspaces::{inspect, WorkspaceKind};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use rustix::fs::{self as fs, Mode, OFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

pub(crate) const KIND: &str = "project_config";
const PROPOSALS: &str = "local:approvals.propose:project_config";
const PINS: &str = "local:project-config:task-pins";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Provider {
    Codex,
    ClaudeCode,
}

impl Provider {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }

    fn policy(self) -> ProjectImportPolicy {
        use ProjectSource::*;
        let (provider_id, sources) = match self {
            Self::Codex => ("codex", vec![AgentSkills, CodexSkills, CodexConfig]),
            Self::ClaudeCode => (
                "claude-code",
                vec![ClaudeSkills, ClaudeMcp, ClaudeSettings, ClaudeLocalSettings],
            ),
        };
        ProjectImportPolicy {
            revision: "core-project-instructions-1".into(),
            provider: ProviderRevision {
                provider_id: provider_id.into(),
                version: "project-source-1".into(),
            },
            sources,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preview {
    pub project_id: String,
    pub provider: Provider,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    key: ProjectCapabilityKey,
    sha256: String,
    approved_update_sha256: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Propose {
    project_id: String,
    provider: Provider,
    expected_config_revision: u64,
    instructions: Vec<Selection>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Refresh {
    project_id: String,
    expected_config_revision: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Reconcile {
    project_id: String,
    operation_id: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Proposal {
    selection: Propose,
    scope: ProjectImportScope,
    binding: ProjectImportBinding,
    plan_sha256: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Active {
    proposal: Proposal,
    approval_id: String,
    rule: Option<ProjectConfigUpdateBinding>,
    update: Option<ProjectConfigVersionBinding>,
}

impl Active {
    fn first(&self) -> ConfirmedProjectImport<'_> {
        ConfirmedProjectImport {
            approval_ref: &self.approval_id,
            binding: &self.proposal.binding,
        }
    }

    fn rule(&self) -> ProjectConfigUpdateRule {
        ProjectConfigUpdateRule {
            revision: self.approval_id.clone(),
            instruction_keys: self
                .proposal
                .selection
                .instructions
                .iter()
                .filter(|entry| !entry.approved_update_sha256.is_empty())
                .map(|entry| entry.key.clone())
                .collect(),
        }
    }

    fn snapshot(&self) -> &str {
        self.update
            .as_ref()
            .map_or(&self.proposal.binding.snapshot_sha256, |update| {
                &update.snapshot_sha256
            })
    }

    fn path(&self, db: &Connection) -> Result<PathBuf> {
        target(
            db,
            &self.proposal.scope.project_id,
            &self.proposal.binding.proposal_id,
            false,
        )
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    id: String,
    next: Active,
    is_update: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    serial: u64,
    revision: u64,
    active: Option<Active>,
    pending: Option<Pending>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    revision: u64,
    active: Active,
}

fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST", "Invalid project configuration request")
}

fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Project configuration inputs changed; review the current project",
    )
}

fn stale() -> CoreError {
    CoreError::new(
        "STALE_REVISION",
        "Project configuration or approval revision changed; refresh before continuing",
    )
}

fn missing() -> CoreError {
    CoreError::new(
        "NOT_FOUND",
        "Project configuration operation does not exist",
    )
}

fn unknown(id: &str) -> CoreError {
    let mut error = CoreError::new(
        "OUTCOME_UNKNOWN",
        "Read the exact project configuration operation before retrying",
    );
    error.side_effect_state = "unknown".into();
    error.details_ref = Some(format!("project-config:{id}"));
    error
}

fn uuid(value: &str) -> Result<()> {
    if Uuid::parse_str(value).is_ok_and(|id| id.hyphenated().to_string() == value) {
        Ok(())
    } else {
        Err(invalid())
    }
}

fn key(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 256 {
        return Err(invalid());
    }
    Ok(())
}

fn state_scope(project_id: &str) -> String {
    format!("local:project-config:state:{project_id}")
}

fn operation_scope(project_id: &str) -> String {
    format!("local:project-config:operations:{project_id}")
}

fn refresh_scope(project_id: &str) -> String {
    format!("local:project-config:refresh:{project_id}")
}

fn read(db: &Connection, scope: &str, key: &str) -> Result<Option<(String, String)>> {
    db.query_row(
        "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
        params![scope, key],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(db_error)
}

fn insert<T: Serialize>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
    value: &T,
) -> Result<()> {
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, encode(value)?],
    )
    .map_err(db_error)?;
    Ok(())
}

fn state(db: &Connection, project_id: &str) -> Result<State> {
    let last: Option<String> = db
        .query_row(
            "SELECT result FROM idempotency WHERE scope=?1 ORDER BY key DESC LIMIT 1",
            [state_scope(project_id)],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    last.as_deref()
        .map(decode)
        .transpose()
        .map(Option::unwrap_or_default)
}

fn append(db: &Connection, project_id: &str, state: &mut State) -> Result<()> {
    state.serial = state.serial.checked_add(1).ok_or_else(changed)?;
    insert(
        db,
        &state_scope(project_id),
        &format!("{:020}", state.serial),
        "",
        state,
    )
}

fn view(state: &State) -> Value {
    json!({
        "config_revision": state.revision,
        "active": state.active.as_ref().map(|active| json!({
            "provider": active.proposal.selection.provider,
            "project_revision": active.proposal.scope.project_revision,
            "snapshot_sha256": active.snapshot(),
            "approval_id": active.approval_id,
            "update_rule_bound": active.rule.is_some(),
        })),
        "pending_operation_id": state.pending.as_ref().map(|pending| &pending.id),
    })
}

fn check_ready(state: &State, revision: u64) -> Result<()> {
    if let Some(pending) = &state.pending {
        return Err(unknown(&pending.id));
    }
    if state.revision != revision {
        return Err(stale());
    }
    Ok(())
}

fn project_scope(project: &Project, expected: u64) -> Result<ProjectImportScope> {
    if project.revision != expected {
        return Err(stale());
    }
    if project.archived {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Archived projects cannot update configuration",
        ));
    }
    let prior = &project.identity;
    let current = inspect(&prior.canonical_root).map_err(|_| changed())?;
    if current.canonical_root != prior.canonical_root
        || current.device != prior.device
        || current.inode != prior.inode
    {
        return Err(changed());
    }
    let input_revision = match (&prior.kind, &current.kind) {
        (WorkspaceKind::Directory, WorkspaceKind::Directory) => {
            format!("directory:{}:{}", current.device, current.inode)
        }
        (
            WorkspaceKind::Git {
                common_dir: old, ..
            },
            WorkspaceKind::Git {
                common_dir, head, ..
            },
        ) if old == common_dir => head.clone(),
        _ => return Err(changed()),
    };
    Ok(ProjectImportScope {
        project_id: project.id.clone(),
        project_revision: project.revision,
        input_revision,
        root: current.canonical_root,
        root_device: current.device,
        root_inode: current.inode,
    })
}

fn runtime_root(db: &Connection) -> Result<&Path> {
    db.path()
        .and_then(|file| Path::new(file).parent())
        .filter(|path| path.is_absolute())
        .ok_or_else(changed)
}

fn target(db: &Connection, project: &str, epoch: &str, create: bool) -> Result<PathBuf> {
    uuid(project)?;
    uuid(epoch)?;
    let root = runtime_root(db)?;
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut fd = fs::open(root, flags, Mode::empty()).map_err(|_| changed())?;
    for component in ["project-config", project, epoch] {
        if create {
            match fs::mkdirat(&fd, component, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(_) => return Err(changed()),
            }
        }
        let next = fs::openat(&fd, component, flags, Mode::empty()).map_err(|_| changed())?;
        let stat = fs::fstat(&next).map_err(|_| changed())?;
        if stat.st_mode & 0o777 != 0o700 || stat.st_uid != rustix::process::geteuid().as_raw() {
            return Err(changed());
        }
        fd = next;
    }
    Ok(root.join("project-config").join(project).join(epoch))
}

fn choices(selection: &Propose) -> Vec<ProjectImportChoice<'_>> {
    selection
        .instructions
        .iter()
        .map(|item| ProjectImportChoice::Skill {
            key: &item.key,
            nonsecret_sha256: &item.sha256,
        })
        .collect()
}

fn validate_selections(selection: &Propose) -> Result<()> {
    let digest = |value: &str| {
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    };
    if selection.instructions.len() > 32 {
        return Err(invalid());
    }
    let mut keys = std::collections::BTreeSet::new();
    for item in &selection.instructions {
        if item.key.kind != ProjectCapabilityKind::Skill
            || item.key.locator.len() > 256
            || !keys.insert(&item.key)
            || !digest(&item.sha256)
            || item.approved_update_sha256.len() > 16
            || !item
                .approved_update_sha256
                .iter()
                .all(|value| digest(value))
        {
            return Err(invalid());
        }
    }
    Ok(())
}

impl Store {
    pub(crate) fn project_config_get(&self, project_id: &str) -> Result<Value> {
        uuid(project_id)?;
        self.get_project(project_id)?;
        Ok(view(&state(&self.conn, project_id)?))
    }

    pub(crate) fn project_config_preview(&self, request: Preview) -> Result<Value> {
        uuid(&request.project_id)?;
        let project = self.get_project(&request.project_id)?;
        let scope = project_scope(&project, project.revision)?;
        let inspection = inspect_project_config(&scope, &request.provider.policy())
            .map_err(crate::runtime_config::config_error)?;
        Ok(json!({
            "project_id": project.id,
            "project_revision": project.revision,
            "provider": request.provider,
            "input_revision": scope.input_revision,
            "inspection": inspection,
            "configuration": view(&state(&self.conn, &project.id)?),
        }))
    }

    pub(crate) fn project_config_propose(
        &mut self,
        offer: Propose,
        project_revision: u64,
        idempotency_key: &str,
    ) -> Result<ApprovalProposal> {
        key(idempotency_key)?;
        uuid(&offer.project_id)?;
        validate_selections(&offer)?;
        if let Some((payload, receipt)) = read(&self.conn, PROPOSALS, idempotency_key)? {
            let original: Proposal = decode(&payload)?;
            if original.selection != offer || original.scope.project_revision != project_revision {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Proposal key is bound to another request",
                ));
            }
            return decode(&receipt);
        }
        let current = state(&self.conn, &offer.project_id)?;
        check_ready(&current, offer.expected_config_revision)?;
        let scope = project_scope(&self.get_project(&offer.project_id)?, project_revision)?;
        let runtime = runtime_root(&self.conn)?;
        if runtime.starts_with(&scope.root) || scope.root.starts_with(runtime) {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Project and daemon directories must be disjoint",
            ));
        }
        let epoch = Uuid::new_v4().hyphenated().to_string();
        let path = target(&self.conn, &offer.project_id, &epoch, true)?;
        let review = prepare_project_import(&ProjectImportRequest {
            proposal_id: &epoch,
            scope: &scope,
            policy: &offer.provider.policy(),
            managed_project_root: &path,
            choices: &choices(&offer),
        })
        .map_err(crate::runtime_config::config_error)?;
        let proposal = Proposal {
            selection: offer,
            scope,
            binding: review.binding,
            plan_sha256: review.runtime_plan_sha256,
        };
        self.persist_proposal(KIND, json!(proposal), idempotency_key)
    }

    pub(crate) fn is_project_config_proposal(&self, approval_id: &str) -> Result<bool> {
        self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM approval_proposals WHERE id=?1 AND json_valid(document) AND json_extract(document,'$.kind')=?2)",
            params![approval_id, KIND],
            |row| row.get(0),
        ).map_err(db_error)
    }

    pub(crate) fn finalize_project_config(
        &mut self,
        mut approval: ApprovalProposal,
        revision: u64,
        consumed: Option<String>,
    ) -> Result<Value> {
        let proposal: Proposal =
            serde_json::from_value(approval.snapshot.clone()).map_err(|_| changed())?;
        let project_id = proposal.scope.project_id.clone();
        if let Some(operation) = consumed {
            let request: Reconcile = decode(&operation)?;
            if !approval.consumed
                || approval.revision != revision.saturating_add(1)
                || request.project_id != project_id
            {
                return Err(stale());
            }
            return self.project_config_reconcile(request);
        }
        let now = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        if approval.consumed
            || approval.revision != revision
            || now >= i128::from(approval.expires_at_ms)
        {
            return Err(stale());
        }
        let mut current = state(&self.conn, &project_id)?;
        check_ready(&current, proposal.selection.expected_config_revision)?;
        let scope = project_scope(
            &self.get_project(&project_id)?,
            proposal.scope.project_revision,
        )?;
        if scope != proposal.scope {
            return Err(changed());
        }
        let mut next = Active {
            proposal,
            approval_id: approval.id.clone(),
            rule: None,
            update: None,
        };
        let path = next.path(&self.conn)?;
        let id = next.proposal.binding.proposal_id.clone();
        let policy = next.proposal.selection.provider.policy();
        let selection = next.proposal.selection.clone();
        let request = ProjectImportRequest {
            proposal_id: &id,
            scope: &scope,
            policy: &policy,
            managed_project_root: &path,
            choices: &choices(&selection),
        };
        let review =
            prepare_project_import(&request).map_err(crate::runtime_config::config_error)?;
        if review.binding != next.proposal.binding
            || review.runtime_plan_sha256 != next.proposal.plan_sha256
        {
            return Err(changed());
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if state(&tx, &project_id)?.serial != current.serial {
            return Err(stale());
        }
        current.pending = Some(Pending {
            id: id.clone(),
            next: next.clone(),
            is_update: false,
        });
        append(&tx, &project_id, &mut current)?;
        let confirmed_revision = i64::try_from(approval.revision).map_err(|_| stale())?;
        approval.revision += 1;
        approval.consumed = true;
        let receipt = json!({"project_id":project_id,"operation_id":id});
        let written = tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL AND json_extract(document,'$.revision')=?4 AND json_extract(document,'$.consumed')=0",
            params![encode(&approval)?, encode(&receipt)?, approval.id, confirmed_revision],
        ).map_err(db_error)?;
        if written != 1 {
            return Err(stale());
        }
        crate::sessions::event(
            &tx,
            &approval.id,
            approval.revision,
            "approval.confirmed",
            receipt,
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        let outcome = apply_project_import(&request, ProjectImportDecision::Confirm(next.first()));
        if outcome.is_ok_and(|value| value == ProjectImportState::Applied) {
            let rule = next.rule();
            if !rule.instruction_keys.is_empty() {
                next.rule = prepare_project_config_update_binding(&path, &next.first(), &rule).ok();
            }
            return self.project_config_settle(&project_id, &id, Some(next));
        }
        self.project_config_reconcile(Reconcile {
            project_id,
            operation_id: id,
        })
    }

    pub(crate) fn project_config_refresh(
        &mut self,
        input: Refresh,
        revision: u64,
        idempotency_key: &str,
    ) -> Result<Value> {
        uuid(&input.project_id)?;
        key(idempotency_key)?;
        let project_id = &input.project_id;
        let idem = refresh_scope(project_id);
        let payload = encode(&(revision, &input))?;
        if let Some((old, result)) = read(&self.conn, &idem, idempotency_key)? {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Refresh key is bound to another request",
                ));
            }
            let receipt: Value = decode(&result)?;
            return match receipt["operation_id"].as_str() {
                Some(id) => self.project_config_reconcile(Reconcile {
                    project_id: project_id.clone(),
                    operation_id: id.into(),
                }),
                None => Ok(receipt),
            };
        }
        let mut current = state(&self.conn, project_id)?;
        check_ready(&current, input.expected_config_revision)?;
        let scope = project_scope(&self.get_project(project_id)?, revision)?;
        let active = current.active.clone().ok_or_else(missing)?;
        let Some(binding) = &active.rule else {
            let reply = json!({"status":"review_required","reason":"no_confirmed_update_rule","configuration":view(&current)});
            insert(&self.conn, &idem, idempotency_key, &payload, &reply)?;
            return Ok(reply);
        };
        let policy = active.proposal.selection.provider.policy();
        let inspection =
            inspect_project_config(&scope, &policy).map_err(crate::runtime_config::config_error)?;
        let mut classified = BTreeMap::new();
        for source in inspection.candidates {
            if active.proposal.selection.instructions.iter().any(|item| {
                source.key == item.key
                    && (item.sha256 == source.source_sha256
                        || item.approved_update_sha256.contains(&source.source_sha256))
            }) {
                classified.insert(source.key, source.source_sha256);
            }
        }
        let rule = active.rule();
        let authorization = ConfirmedProjectConfigUpdate {
            approval_ref: &active.approval_id,
            binding,
            rule: &rule,
        };
        let id = Uuid::new_v4().hyphenated().to_string();
        let path = active.path(&self.conn)?;
        let request = ProjectConfigUpdateRequest {
            transaction_id: &id,
            managed_project_root: &path,
            scope: &scope,
            policy: &policy,
            base: active.update.as_ref(),
            classified_instructions: &classified,
        };
        let version = match prepare_project_config_update(&request, &authorization)
            .map_err(crate::runtime_config::config_error)?
        {
            ProjectConfigUpdatePreview::Ready(version) => version,
            other => {
                let reply = match other {
                    ProjectConfigUpdatePreview::Unchanged => {
                        json!({"status":"unchanged","configuration":view(&current)})
                    }
                    ProjectConfigUpdatePreview::ReviewRequired(reason) => {
                        json!({"status":"review_required","review":reason,"configuration":view(&current)})
                    }
                    ProjectConfigUpdatePreview::Ready(_) => unreachable!(),
                };
                insert(&self.conn, &idem, idempotency_key, &payload, &reply)?;
                return Ok(reply);
            }
        };
        let mut next = active.clone();
        next.update = Some(version.clone());
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if state(&tx, project_id)?.serial != current.serial {
            return Err(stale());
        }
        current.pending = Some(Pending {
            id: id.clone(),
            next: next.clone(),
            is_update: true,
        });
        append(&tx, project_id, &mut current)?;
        insert(
            &tx,
            &idem,
            idempotency_key,
            &payload,
            &json!({"operation_id":id}),
        )?;
        tx.commit().map_err(db_error)?;
        if apply_project_config_update(&request, &authorization, &version)
            .is_ok_and(|value| value == ProjectImportState::Applied)
        {
            return self.project_config_settle(project_id, &id, Some(next));
        }
        self.project_config_reconcile(Reconcile {
            project_id: project_id.clone(),
            operation_id: id,
        })
    }

    pub(crate) fn project_config_reconcile(&mut self, input: Reconcile) -> Result<Value> {
        let Reconcile {
            project_id,
            operation_id,
        } = input;
        uuid(&project_id)?;
        uuid(&operation_id)?;
        if let Some((_, receipt)) = read(&self.conn, &operation_scope(&project_id), &operation_id)?
        {
            return decode(&receipt);
        }
        let current = state(&self.conn, &project_id)?;
        let pending = current
            .pending
            .filter(|pending| pending.id == operation_id)
            .ok_or_else(missing)?;
        let mut next = pending.next;
        let path = next.path(&self.conn).map_err(|_| unknown(&operation_id))?;
        let observed = if pending.is_update {
            let rule = next.rule();
            let binding = next.rule.as_ref().ok_or_else(changed)?;
            let expected = next.update.as_ref().ok_or_else(changed)?;
            recover_project_config_update(
                &path,
                &ConfirmedProjectConfigUpdate {
                    approval_ref: &next.approval_id,
                    binding,
                    rule: &rule,
                },
                expected,
            )
        } else {
            recover_project_import(&path, &next.first())
        }
        .map_err(|_| unknown(&operation_id))?;
        match observed {
            ProjectImportState::Applied => {
                let rule = next.rule();
                if !pending.is_update && !rule.instruction_keys.is_empty() {
                    next.rule =
                        prepare_project_config_update_binding(&path, &next.first(), &rule).ok();
                }
                self.project_config_settle(&project_id, &operation_id, Some(next))
            }
            ProjectImportState::NotApplied => {
                self.project_config_settle(&project_id, &operation_id, None)
            }
            _ => Err(unknown(&operation_id)),
        }
    }

    fn project_config_settle(
        &mut self,
        project_id: &str,
        id: &str,
        active: Option<Active>,
    ) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut current = state(&tx, project_id)?;
        if current
            .pending
            .as_ref()
            .is_none_or(|pending| pending.id != id)
        {
            return Err(unknown(id));
        }
        current.pending = None;
        let status = if let Some(active) = active {
            current.revision = current.revision.checked_add(1).ok_or_else(changed)?;
            current.active = Some(active);
            "applied"
        } else {
            "not_applied"
        };
        let result = json!({"status":status,"operation_id":id,"configuration":view(&current)});
        append(&tx, project_id, &mut current).map_err(|_| unknown(id))?;
        insert(&tx, &operation_scope(project_id), id, "", &result).map_err(|_| unknown(id))?;
        crate::sessions::event(
            &tx,
            project_id,
            current.revision,
            "project.config_settled",
            result.clone(),
            "project_config",
        )
        .map_err(|_| unknown(id))?;
        tx.commit().map_err(|_| unknown(id))?;
        Ok(result)
    }
}

/// In the same transaction as the new Task. Missing pins on older Tasks remain
/// the original connection-only plan after later project approvals.
pub(crate) fn pin_task(db: &Connection, project_id: &str, task_id: &str) -> Result<()> {
    let current = state(db, project_id)?;
    check_ready(&current, current.revision)?;
    if let Some(active) = current.active {
        let project: String = db
            .query_row(
                "SELECT document FROM projects WHERE id=?1",
                [project_id],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let project: Project = decode(&project)?;
        if project.archived || project.revision != active.proposal.scope.project_revision {
            return Err(changed());
        }
        insert(
            db,
            PINS,
            task_id,
            "",
            &Pin {
                revision: current.revision,
                active,
            },
        )?;
    }
    Ok(())
}

pub(crate) fn task_plan(
    db: &Connection,
    task_id: &str,
    project_id: &str,
    provider: &str,
) -> Result<Option<RuntimePlan>> {
    // The Factory delegation transaction fixes descendants to the root Task.
    // The lookup inherits its immutable pin or its original no-config behavior.
    let receipt: Option<String> = db
        .query_row(
            "SELECT result FROM idempotency WHERE scope=?1 AND key IN (
            ?2,(SELECT root_task_id FROM factory_task_lineage
                WHERE task_id=?2 AND parent_task_id IS NOT NULL)
         ) ORDER BY key=?2 DESC LIMIT 1",
            params![PINS, task_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let Some(receipt) = receipt else {
        return Ok(None);
    };
    let pin: Pin = decode(&receipt)?;
    let active = pin.active;
    if active.proposal.scope.project_id != project_id {
        return Err(changed());
    }
    let path = active.path(db)?;
    let mut plan = if let Some(update) = &active.update {
        let rule = active.rule();
        read_project_config_version(
            &path,
            &ConfirmedProjectConfigUpdate {
                approval_ref: &active.approval_id,
                binding: active.rule.as_ref().ok_or_else(changed)?,
                rule: &rule,
            },
            update,
        )
    } else {
        read_imported_project_plan(&path, &active.first())
    }
    .map_err(crate::runtime_config::config_error)?
    .ok_or_else(changed)?;
    // A pin also binds provenance for an independently approved connection
    // using another provider. Only the matching provider receives instructions.
    if active.proposal.selection.provider.id() != provider {
        plan = crate::runtime_config::plan_for_provider(provider)?;
    }
    plan.source_refs.push(SourceProvenance {
        source_ref: "baton-project-config".into(),
        revision: pin.revision.to_string(),
        sha256: active.snapshot().into(),
    });
    Ok(Some(plan))
}
