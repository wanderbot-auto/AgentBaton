//! Native-owner approved fixed-command policies. Results remain command-only;
//! no task acceptance, Git adoption, account or production authority is inferred.
use crate::{
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store, Task, TaskState},
    CoreError, Result,
};
use agentbaton_workspaces::{
    command_validation::{
        ApprovedCommandRule, CommandMode, CommandRun, CommandSettings, RequiredEnforcement, SCOPE,
        VALIDATOR_ID,
    },
    validation::{sha256, ArtifactRequirement, BoundArtifact, ValidationSubject, VersionBinding},
    WorkspaceKind,
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, os::unix::fs::PermissionsExt, path::Path};
use uuid::Uuid;
const MAX_BYTES: i64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnforcement {
    pub filesystem_isolation: bool,
    pub network_isolation: bool,
    pub process_tree_containment: bool,
    pub execution_time_input_immutability: bool,
    pub auxiliary_source_immutability: bool,
    pub sealed_executable_image: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandValidationSelection {
    pub session_id: String,
    pub executable: String,
    pub mode: String,
    pub script: Option<ArtifactRequirement>,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<ArtifactRequirement>,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub expected_exit_codes: Vec<i32>,
    pub wall_time_ms: u64,
    pub cleanup_time_ms: u64,
    pub stdout_limit: u32,
    pub stderr_limit: u32,
    pub require: CommandEnforcement,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandValidationPolicy {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub session_id: String,
    pub attempt_id: String,
    pub workspace_id: String,
    pub fencing_token: u64,
    pub approval_id: String,
    pub revision: u64,
    pub revoked: bool,
    pub validator_id: String,
    pub scope: String,
    pub max_runs: u32,
    pub selection: CommandValidationSelection,
    pub contract: Value,
    pub contract_digest: String,
    pub refs: Value,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandValidationRecord {
    pub id: String,
    pub policy_id: String,
    pub task_id: String,
    pub session_id: String,
    pub workspace_id: String,
    pub attempt_id: String,
    pub revision: u64,
    pub state: String,
    pub scope: String,
    pub correctness: String,
    pub task_acceptance: String,
    pub integration_authorized: bool,
    pub evidence: Option<Value>,
    pub last_error: Option<CoreError>,
    pub created_at: String,
    pub updated_at: String,
}
pub(crate) fn has_unreaped_child(run: &CommandRun) -> bool {
    let view = run.view();
    view.process_started && !view.cleanup.owned_leader_reaped
}
fn ambiguous_cleanup(run: &CommandRun) -> bool {
    let view = run.view();
    view.process_started
        && (!view.cleanup.owned_leader_reaped
            || !view.cleanup_completed_within_budget
            || matches!(
                view.reason,
                agentbaton_workspaces::command_validation::RunReason::CleanupUnknown
            ))
}
fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Select one bounded fixed command and a confirmed-stopped session",
    )
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Command policy, program, workspace, input or approval snapshot changed",
    )
}
fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Command history exceeds its raw byte/count bounds; records remain local",
    )
}
pub(crate) fn command_error(
    error: agentbaton_workspaces::command_validation::CommandError,
) -> CoreError {
    let code = match error {
        agentbaton_workspaces::command_validation::CommandError::EnforcementUnavailable => {
            "ENFORCEMENT_UNAVAILABLE"
        }
        agentbaton_workspaces::command_validation::CommandError::InvalidContract => {
            "INVALID_REQUEST"
        }
        agentbaton_workspaces::command_validation::CommandError::UnsupportedProgram => {
            "CAPABILITY_UNSUPPORTED"
        }
        _ => "INPUT_CHANGED",
    };
    CoreError::new(code, &error.to_string())
}
fn key(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 256 {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(crate) fn event(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    revision: u64,
    kind: &str,
    payload: Value,
) -> Result<()> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: "fixed_command_supervisor".into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(())
}
fn document(c: &rusqlite::Connection, table: &str, id: &str) -> Result<String> {
    let sql=match table{"command_validation_policies"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=4194304 THEN document END FROM command_validation_policies WHERE id=?1","command_validation_runs"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=4194304 THEN document END FROM command_validation_runs WHERE id=?1","approval_proposals"=>"SELECT octet_length(document),CASE WHEN octet_length(document)<=4194304 THEN document END FROM approval_proposals WHERE id=?1",_=>return Err(invalid())};
    let row: Option<(i64, Option<String>)> = c
        .query_row(sql, [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .map_err(db_error)?;
    let (size, raw) =
        row.ok_or_else(|| CoreError::new("NOT_FOUND", "Command record is unavailable"))?;
    if !(0..=MAX_BYTES).contains(&size) {
        return Err(limited());
    }
    raw.ok_or_else(limited)
}
fn listed<T: for<'d> Deserialize<'d>>(
    c: &rusqlite::Connection,
    table: &str,
    task: &str,
) -> Result<Vec<T>> {
    let sql=match table{"command_validation_policies"=>"SELECT id,octet_length(document) FROM command_validation_policies WHERE task_id=?1 ORDER BY rowid LIMIT 101","command_validation_runs"=>"SELECT id,octet_length(document) FROM command_validation_runs WHERE task_id=?1 ORDER BY rowid LIMIT 101",_=>return Err(invalid())};
    let mut q = c.prepare(sql).map_err(db_error)?;
    let rows = q
        .query_map([task], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        })
        .map_err(db_error)?;
    let mut ids = vec![];
    let mut bytes = 0i64;
    for row in rows {
        let (id, size) = row.map_err(db_error)?;
        bytes = bytes.checked_add(size).ok_or_else(limited)?;
        if size < 0 || bytes > MAX_BYTES || ids.len() == 100 {
            return Err(limited());
        }
        ids.push(id);
    }
    ids.iter()
        .map(|id| decode(&document(c, table, id)?))
        .collect()
}
fn replay_receipt<T: for<'d> Deserialize<'d>>(
    c: &rusqlite::Connection,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<T>> {
    let row:Option<(i64,i64,Option<String>,Option<String>)>=c.query_row("SELECT octet_length(payload),octet_length(result),CASE WHEN octet_length(payload)<=4194304 THEN payload END,CASE WHEN octet_length(result)<=4194304 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope,key],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(db_error)?;
    if let Some((p, r, old, result)) = row {
        if !(0..=MAX_BYTES).contains(&p) || !(0..=MAX_BYTES).contains(&r) {
            return Err(limited());
        }
        if old.as_deref() != Some(payload) {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Command idempotency key belongs to another request",
            ));
        }
        return Ok(Some(decode(&result.ok_or_else(limited)?)?));
    }
    Ok(None)
}
fn nonsecret_environment(env: &BTreeMap<String, String>) -> bool {
    env.iter().all(|(name, value)| match name.as_str() {
        "LANG" | "LC_ALL" => matches!(value.as_str(), "C" | "C.UTF-8" | "en_US.UTF-8"),
        "TZ" => value == "UTC",
        _ => false,
    })
}
fn settings(
    selection: &CommandValidationSelection,
    script: Option<BoundArtifact>,
) -> Result<CommandSettings> {
    let mode = match (selection.mode.as_str(), script) {
        ("native_executable", None) => CommandMode::NativeExecutable,
        ("posix_sh_stdin", Some(script)) => CommandMode::PosixShStdin { script },
        _ => return Err(invalid()),
    };
    let r = &selection.require;
    Ok(CommandSettings {
        mode,
        arguments: selection.arguments.clone(),
        environment: selection.environment.clone(),
        expected_exit_codes: selection.expected_exit_codes.clone(),
        wall_time_ms: selection.wall_time_ms,
        cleanup_time_ms: selection.cleanup_time_ms,
        stdout_limit: selection.stdout_limit,
        stderr_limit: selection.stderr_limit,
        require: RequiredEnforcement {
            filesystem_isolation: r.filesystem_isolation,
            network_isolation: r.network_isolation,
            process_tree_containment: r.process_tree_containment,
            execution_time_input_immutability: r.execution_time_input_immutability,
            auxiliary_source_immutability: r.auxiliary_source_immutability,
            sealed_executable_image: r.sealed_executable_image,
        },
    })
}
/// Internal only: never Deserialize, never constructed from a renderer flag.
/// Its private fields bind the one actual run whose terminal receipt is checked.
pub(crate) struct CommandReadAuthority {
    run_id: String,
    policy_id: String,
    task_id: String,
    session_id: String,
    attempt_id: String,
    workspace_id: String,
    fencing_token: u64,
    policy_revision: u64,
}
impl Store {
    fn resolve_command(
        &self,
        selection: &CommandValidationSelection,
        approval: &str,
        revision: u64,
        owned: Option<&CommandReadAuthority>,
    ) -> Result<(ApprovedCommandRule, Value)> {
        if !nonsecret_environment(&selection.environment) {
            return Err(CoreError::new("CAPABILITY_UNSUPPORTED","Only approved LANG/LC_ALL locale constants and TZ=UTC are supported; credentials and arbitrary environment values are not accepted"));
        }
        if selection.executable.len() > 4096
            || selection.inputs.len() > 15
            || selection.outputs.len() > 16
            || selection.arguments.len() > 64
            || selection
                .inputs
                .iter()
                .chain(&selection.outputs)
                .chain(selection.script.iter())
                .any(|a| a.relative_path.as_os_str().len() > 4096)
        {
            return Err(invalid());
        }
        let (session, workspace, binding) = match owned {
            Some(authority) => {
                self.validation_workspace_for_owned_command(&selection.session_id, authority)?
            }
            None => self.validation_workspace(&selection.session_id)?,
        };
        let task = self.get(&session.task_id)?;
        if matches!(
            task.state,
            TaskState::CancelRequested | TaskState::Cancelled
        ) {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Task cancellation prevents a new command validation",
            ));
        }
        let cwd = agentbaton_workspaces::inspect(&workspace.root).map_err(|_| changed())?;
        let base = match &cwd.kind {
            WorkspaceKind::Git {
                head, dirty: false, ..
            } => head.clone(),
            WorkspaceKind::Directory => binding.input_revision.clone(),
            _ => return Err(changed()),
        };
        let path = std::fs::canonicalize(&selection.executable).map_err(|_| invalid())?;
        if !Path::new(&selection.executable).is_absolute() || path.to_str().is_none() {
            return Err(invalid());
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| invalid())?;
        if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
            return Err(invalid());
        }
        let parent = path.parent().ok_or_else(invalid)?;
        let relative = Path::new(path.file_name().ok_or_else(invalid)?);
        let bytes = agentbaton_workspaces::read_artifact(parent, relative, 16 * 1024 * 1024)
            .map_err(|_| changed())?;
        let program = BoundArtifact::from_approved_root(
            parent,
            ArtifactRequirement {
                artifact_id: "baton-command-program".into(),
                relative_path: relative.into(),
                max_bytes: 16 * 1024 * 1024,
                expected_sha256: Some(sha256(&bytes)),
            },
        )
        .map_err(|_| changed())?;
        let bind = |a: &ArtifactRequirement| {
            BoundArtifact::from_approved_root(&workspace.root, a.clone()).map_err(|_| invalid())
        };
        let inputs = selection
            .inputs
            .iter()
            .map(bind)
            .collect::<Result<Vec<_>>>()?;
        let outputs = selection
            .outputs
            .iter()
            .map(bind)
            .collect::<Result<Vec<_>>>()?;
        let script = selection.script.as_ref().map(bind).transpose()?;
        let subject = ValidationSubject {
            task_id: task.id.clone(),
            attempt_id: session.attempt_id.clone(),
            spec_revision: task.spec_revision,
            input_revision: binding.input_revision.clone(),
            version: VersionBinding::WorkspaceArtifacts {
                workspace_id: workspace.id.clone(),
                base_revision: base,
            },
        };
        let rule = ApprovedCommandRule::from_verified_policy(
            approval.into(),
            revision.to_string(),
            subject,
            program,
            cwd,
            inputs,
            outputs,
            settings(selection, script)?,
        )
        .map_err(command_error)?;
        let project = self.get_project(&session.project_id)?;
        let refs = json!({"project_id":project.id,"project_name":project.name,"project_revision":project.revision,"task_goal":task.goal,"task_revision":task.revision,"task_state":task.state,"session_revision":session.revision,"workspace_revision":workspace.revision,"fencing_token":session.fencing_token,"binding":binding});
        Ok((rule, refs))
    }
    pub fn propose_command_validation_policy(
        &mut self,
        selection: &CommandValidationSelection,
        key: &str,
    ) -> Result<ApprovalProposal> {
        let (rule, refs) = self.resolve_command(selection, "proposal-preview", 1, None)?;
        self.persist_proposal("command_validation",json!({"intent":selection,"contract":rule.contract_view(),"contract_digest":rule.contract_digest(),"refs":refs,"max_runs":1,"scope":SCOPE,"warning":"Executes exactly this approved local program/argv or SHA-bound script from memory once. Arguments are displayed and retained: do not supply credentials. No arbitrary environment values, OS sandbox, continuous input freeze, descendant containment, TaskAccepted, Git adoption or production authority is established."}),key)
    }
    pub(crate) fn approve_command_validation_policy(
        &mut self,
        proposal: &ApprovalProposal,
    ) -> Result<Value> {
        let selection: CommandValidationSelection =
            serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_| invalid())?;
        let (rule, refs) = self.resolve_command(&selection, "proposal-preview", 1, None)?;
        if proposal.snapshot["contract"] != json!(rule.contract_view())
            || proposal.snapshot["contract_digest"] != json!(rule.contract_digest())
            || proposal.snapshot["refs"] != refs
            || proposal.snapshot["max_runs"] != json!(1)
        {
            return Err(changed());
        }
        if let Some(id) = self
            .conn
            .query_row(
                "SELECT id FROM command_validation_policies WHERE approval_id=?1",
                [&proposal.id],
                |r| r.get::<_, String>(0),
            )
            .optional()
            .map_err(db_error)?
        {
            return Ok(json!(self.get_command_validation_policy(&id)?));
        }
        let session = self.get_session(&selection.session_id)?;
        let policy = CommandValidationPolicy {
            id: Uuid::new_v4().to_string(),
            task_id: session.task_id,
            project_id: session.project_id,
            session_id: session.id,
            attempt_id: session.attempt_id,
            workspace_id: session.workspace_id,
            fencing_token: session.fencing_token,
            approval_id: proposal.id.clone(),
            revision: 1,
            revoked: false,
            validator_id: VALIDATOR_ID.into(),
            scope: SCOPE.into(),
            max_runs: 1,
            selection,
            contract: json!(rule.contract_view()),
            contract_digest: rule.contract_digest().into(),
            refs,
            created_at: now()?,
        };
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("INSERT INTO command_validation_policies(id,task_id,session_id,attempt_id,workspace_id,approval_id,document) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![policy.id,policy.task_id,policy.session_id,policy.attempt_id,policy.workspace_id,policy.approval_id,encode(&policy)?]).map_err(db_error)?;
        event(
            &tx,
            &policy.id,
            1,
            "command_validation.policy_approved",
            json!({"policy":policy}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(policy))
    }
    pub fn get_command_validation_policy(&self, id: &str) -> Result<CommandValidationPolicy> {
        let p: CommandValidationPolicy =
            decode(&document(&self.conn, "command_validation_policies", id)?)?;
        if p.id != id {
            return Err(changed());
        }
        Ok(p)
    }
    pub fn get_command_validation_record(&self, id: &str) -> Result<CommandValidationRecord> {
        let r: CommandValidationRecord =
            decode(&document(&self.conn, "command_validation_runs", id)?)?;
        if r.id != id {
            return Err(changed());
        }
        Ok(r)
    }
    pub fn list_command_validation_policies(
        &self,
        task: &str,
    ) -> Result<Vec<CommandValidationPolicy>> {
        self.get(task)?;
        listed(&self.conn, "command_validation_policies", task)
    }
    pub fn list_command_validation_records(
        &self,
        task: &str,
    ) -> Result<Vec<CommandValidationRecord>> {
        self.get(task)?;
        listed(&self.conn, "command_validation_runs", task)
    }
    pub fn revoke_command_validation_policy(
        &mut self,
        id: &str,
        expected: u64,
        key_value: &str,
    ) -> Result<CommandValidationPolicy> {
        key(key_value)?;
        let scope = format!("command-policy-revoke:{id}");
        let payload = encode(&json!({"id":id,"revision":expected}))?;
        if let Some(result) = replay_receipt(&self.conn, &scope, key_value, &payload)? {
            return Ok(result);
        }
        let mut policy = self.get_command_validation_policy(id)?;
        if policy.revision != expected {
            return Err(CoreError::new("STALE_REVISION", "Command policy changed"));
        }
        policy.revoked = true;
        policy.revision = policy.revision.checked_add(1).ok_or_else(invalid)?;
        let tx = self.conn.transaction().map_err(db_error)?;
        let affected=tx.execute("UPDATE command_validation_policies SET document=?1 WHERE id=?2 AND json_extract(document,'$.revision')=?3",params![encode(&policy)?,id,i64::try_from(expected).map_err(|_|invalid())?]).map_err(db_error)?;
        if affected != 1 {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Command policy changed during revocation",
            ));
        }
        event(
            &tx,
            id,
            policy.revision,
            "command_validation.policy_revoked",
            json!({"policy_id":id,"stop_confirmed":false}),
        )?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key_value, payload, encode(&policy)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(policy)
    }
    pub(crate) fn command_rule(
        &self,
        policy: &CommandValidationPolicy,
        owned: Option<&CommandReadAuthority>,
    ) -> Result<ApprovedCommandRule> {
        if policy.revoked
            || policy.validator_id != VALIDATOR_ID
            || policy.scope != SCOPE
            || policy.max_runs != 1
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Command policy is revoked or unsupported",
            ));
        }
        let proposal: ApprovalProposal = decode(&document(
            &self.conn,
            "approval_proposals",
            &policy.approval_id,
        )?)?;
        let confirmed: bool = self
            .conn
            .query_row(
                "SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",
                [&policy.approval_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if !confirmed
            || !proposal.consumed
            || proposal.kind != "command_validation"
            || proposal.snapshot["contract"] != policy.contract
            || proposal.snapshot["contract_digest"] != json!(policy.contract_digest)
            || proposal.snapshot["refs"] != policy.refs
            || proposal.snapshot["intent"] != json!(policy.selection)
        {
            return Err(changed());
        }
        let (rule, refs) = self.resolve_command(
            &policy.selection,
            &policy.approval_id,
            policy.revision,
            owned,
        )?;
        let session = self.get_session(&policy.session_id)?;
        if json!(rule.contract_view()) != policy.contract
            || rule.contract_digest() != policy.contract_digest
            || refs != policy.refs
            || session.fencing_token != policy.fencing_token
            || session.task_id != policy.task_id
            || session.project_id != policy.project_id
            || session.attempt_id != policy.attempt_id
            || session.workspace_id != policy.workspace_id
        {
            return Err(changed());
        }
        Ok(rule)
    }
    pub(crate) fn command_replay(
        &self,
        policy: &str,
        revision: u64,
        key_value: &str,
    ) -> Result<Option<CommandValidationRecord>> {
        key(key_value)?;
        let old:Option<(String,i64,Option<String>)>=self.conn.query_row("SELECT id,octet_length(payload),CASE WHEN octet_length(payload)<=4194304 THEN payload END FROM command_validation_runs WHERE policy_id=?1 AND idempotency_key=?2",params![policy,key_value],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
        if let Some((id, size, payload)) = old {
            if size > MAX_BYTES {
                return Err(limited());
            }
            if payload.as_deref()
                != Some(&encode(&json!({"policy_id":policy,"revision":revision}))?)
            {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Command run key was used with another request",
                ));
            }
            return Ok(Some(self.get_command_validation_record(&id)?));
        }
        Ok(None)
    }
    pub(crate) fn prepare_command_validation(
        &mut self,
        id: &str,
        expected: u64,
        key_value: &str,
    ) -> Result<(CommandValidationRecord, ApprovedCommandRule)> {
        let policy = self.get_command_validation_policy(id)?;
        if policy.revision != expected {
            return Err(CoreError::new("STALE_REVISION", "Command policy changed"));
        }
        let consumed: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM command_validation_runs WHERE policy_id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if consumed {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "This single-run approval was already consumed; replay its original key",
            ));
        }
        let fenced: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1)",
                [&policy.workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if fenced {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace has an active or unreconciled command writer",
            ));
        }
        let rule = self.command_rule(&policy, None)?;
        let time = now()?;
        let record = CommandValidationRecord {
            id: Uuid::new_v4().to_string(),
            policy_id: id.into(),
            task_id: policy.task_id.clone(),
            session_id: policy.session_id.clone(),
            workspace_id: policy.workspace_id.clone(),
            attempt_id: policy.attempt_id.clone(),
            revision: 1,
            state: "prepared".into(),
            scope: SCOPE.into(),
            correctness: "approved_fixed_command_observations_only".into(),
            task_acceptance: "not_established".into(),
            integration_authorized: false,
            evidence: None,
            last_error: None,
            created_at: time.clone(),
            updated_at: time,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let current: CommandValidationPolicy =
            decode(&document(&tx, "command_validation_policies", id)?)?;
        if current.revoked || encode(&current)? != encode(&policy)? {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Command policy changed before intent commit",
            ));
        }
        let count: i64 = tx
            .query_row(
                "SELECT count(*) FROM command_validation_runs WHERE policy_id=?1",
                [id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if count != 0 {
            return Err(CoreError::new("PERMISSION_DENIED","This one-run command approval was already consumed; replay the same key or review a new policy"));
        }
        let busy: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1)",
                [&policy.workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if busy {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace has an active or unreconciled command writer",
            ));
        }
        tx.execute("INSERT INTO command_validation_runs(id,policy_id,task_id,workspace_id,state,document,idempotency_key,payload) VALUES(?1,?2,?3,?4,'prepared',?5,?6,?7)",params![record.id,id,record.task_id,record.workspace_id,encode(&record)?,key_value,encode(&json!({"policy_id":id,"revision":expected}))?]).map_err(db_error)?;
        tx.execute(
            "INSERT INTO command_workspace_fences(workspace_id,run_id) VALUES(?1,?2)",
            params![record.workspace_id, record.id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &record.id,
            1,
            "command_validation.prepared",
            json!({"record":record,"external_dispatch_started":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok((record, rule))
    }
    pub(crate) fn save_command_record(
        &mut self,
        record: &mut CommandValidationRecord,
        kind: &str,
        release: bool,
    ) -> Result<()> {
        record.revision = record.revision.checked_add(1).ok_or_else(invalid)?;
        record.updated_at = now()?;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "UPDATE command_validation_runs SET state=?1,document=?2 WHERE id=?3",
            params![record.state, encode(record)?, record.id],
        )
        .map_err(db_error)?;
        if record.state == "outcome_unknown" {
            // Reuse the existing shared validation/admission guard: a workspace
            // with an unreconciled command writer is not a ready artifact source.
            tx.execute("UPDATE managed_workspaces SET state='outcome_unknown',document=json_set(document,'$.state','outcome_unknown','$.revision',json_extract(document,'$.revision')+1) WHERE id=?1 AND state!='outcome_unknown'",[&record.workspace_id]).map_err(db_error)?;
        }
        if release {
            tx.execute(
                "DELETE FROM command_workspace_fences WHERE run_id=?1",
                [&record.id],
            )
            .map_err(db_error)?;
        }
        if release {
            let unresolved:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM attempts a JOIN dispatch_intents d ON d.attempt_id=a.id WHERE a.task_id=?1 AND d.state!='stopped') OR EXISTS(SELECT 1 FROM command_validation_runs WHERE task_id=?1 AND state IN('prepared','running','cancel_requested','outcome_unknown'))",[&record.task_id],|r|r.get(0)).map_err(db_error)?;
            let raw: String = tx
                .query_row(
                    "SELECT document FROM tasks WHERE id=?1",
                    [&record.task_id],
                    |r| r.get(0),
                )
                .map_err(db_error)?;
            let mut task: Task = decode(&raw)?;
            if task.state == TaskState::CancelRequested && !unresolved {
                task.state = TaskState::Cancelled;
                task.revision = task.revision.checked_add(1).ok_or_else(invalid)?;
                task.updated_at = now()?;
                tx.execute(
                    "UPDATE tasks SET document=?1 WHERE id=?2",
                    params![encode(&task)?, task.id],
                )
                .map_err(db_error)?;
                event(
                    &tx,
                    &task.id,
                    task.revision,
                    "task.cancelled",
                    json!({"stop_evidence":"all_baton_owned_runtimes_confirmed_stopped","scope":"owned_runtime_only","descendant_containment":"not_enforced_unknown"}),
                )?;
            }
        }
        event(
            &tx,
            &record.id,
            record.revision,
            kind,
            json!({"record":record}),
        )?;
        tx.commit().map_err(db_error)
    }
    pub(crate) fn finish_command_validation(&mut self, id: &str, run: &CommandRun) -> Result<()> {
        let mut record = self.get_command_validation_record(id)?;
        let view = run.view();
        let unknown = record.state == "outcome_unknown" || ambiguous_cleanup(run);
        let mut accepted = false;
        let mut last_error = None;
        if !unknown && view.passed && record.state == "running" {
            match self
                .get_command_validation_policy(&record.policy_id)
                .and_then(|p| {
                    let authority = CommandReadAuthority {
                        run_id: record.id.clone(),
                        policy_id: p.id.clone(),
                        task_id: record.task_id.clone(),
                        session_id: record.session_id.clone(),
                        attempt_id: record.attempt_id.clone(),
                        workspace_id: record.workspace_id.clone(),
                        fencing_token: p.fencing_token,
                        policy_revision: p.revision,
                    };
                    self.command_rule(&p, Some(&authority))
                }) {
                Ok(rule) => match run.accept_command(&rule) {
                    Ok(_) => accepted = true,
                    Err(e) => last_error = Some(command_error(e)),
                },
                Err(e) => last_error = Some(e),
            }
        }
        if view.passed && record.state == "cancel_requested" {
            last_error = Some(CoreError::new(
                "PERMISSION_DENIED",
                "Command cancellation was requested before result commit",
            ));
        }
        record.state = if unknown {
            "outcome_unknown"
        } else if accepted {
            "command_passed"
        } else {
            "command_failed"
        }
        .into();
        record.evidence = Some(json!(view));
        if let Some(error) = &mut last_error {
            if view.process_started {
                error.side_effect_state = "confirmed".into();
                error.retryable = false;
                error.details_ref = Some(format!("command-run:{}", record.id));
            }
        }
        record.last_error = last_error;
        self.save_command_record(&mut record, "command_validation.result_recorded", !unknown)
    }
    pub(crate) fn command_failed_before_receipt(
        &mut self,
        id: &str,
        error: CoreError,
        unknown: bool,
    ) -> Result<()> {
        let mut r = self.get_command_validation_record(id)?;
        r.state = if unknown {
            "outcome_unknown"
        } else {
            "command_failed"
        }
        .into();
        r.last_error = Some(error);
        self.save_command_record(&mut r, "command_validation.worker_failed", !unknown)
    }
    pub(crate) fn request_command_cancel(
        &mut self,
        id: &str,
        expected: u64,
        key_value: &str,
        owned: bool,
    ) -> Result<CommandValidationRecord> {
        key(key_value)?;
        let mut r = self.get_command_validation_record(id)?;
        if expected == 0 || expected > r.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Refresh the command run before cancellation",
            ));
        }
        let scope = format!("command-cancel:{id}");
        let payload = encode(&json!({"run_id":id,"revision":expected}))?;
        if let Some(result) = replay_receipt(&self.conn, &scope, key_value, &payload)? {
            return Ok(result);
        }
        if owned
            && matches!(
                r.state.as_str(),
                "prepared" | "running" | "cancel_requested"
            )
        {
            r.state = "cancel_requested".into();
        }
        let tx = self.conn.transaction().map_err(db_error)?;
        r.revision = r.revision.checked_add(1).ok_or_else(invalid)?;
        r.updated_at = now()?;
        tx.execute(
            "UPDATE command_validation_runs SET state=?1,document=?2 WHERE id=?3",
            params![r.state, encode(&r)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            r.revision,
            "command_validation.cancel_requested",
            json!({"owned_control":owned,"stop_confirmed":false,"state":r.state}),
        )?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key_value, payload, encode(&r)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(r)
    }
    pub(crate) fn check_command_read_fence(
        &self,
        session: &crate::sessions::ManagedSession,
        owned: Option<&CommandReadAuthority>,
    ) -> Result<()> {
        let fence: Option<String> = self
            .conn
            .query_row(
                "SELECT run_id FROM command_workspace_fences WHERE workspace_id=?1",
                [&session.workspace_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        match (fence,owned){
            (None,None)=>Ok(()),
            (Some(id),Some(a))=>{let r=self.get_command_validation_record(&id)?;let p=self.get_command_validation_policy(&a.policy_id)?;
                if id!=a.run_id||r.state!="running"||r.id!=a.run_id||r.policy_id!=a.policy_id||r.task_id!=a.task_id||r.session_id!=a.session_id||r.attempt_id!=a.attempt_id||r.workspace_id!=a.workspace_id||session.id!=a.session_id||session.task_id!=a.task_id||session.attempt_id!=a.attempt_id||session.workspace_id!=a.workspace_id||session.fencing_token!=a.fencing_token||p.fencing_token!=a.fencing_token||p.revoked||p.revision!=a.policy_revision{return Err(changed());}Ok(())
            },
            (Some(_),None)=>Err(CoreError::new("WORKSPACE_BUSY","A prepared, running or unreconciled command writer fences this workspace; ordinary validation cannot read/approve it")),
            (None,Some(_))=>Err(changed()),
        }
    }
    pub(crate) fn recover_command_validations(&mut self) -> Result<()> {
        loop {
            let ids: Vec<String> = {
                let mut q=self.conn.prepare("SELECT id FROM command_validation_runs WHERE state IN('prepared','running','cancel_requested') LIMIT 100").map_err(db_error)?;
                let rows = q.query_map([], |r| r.get(0)).map_err(db_error)?;
                rows.collect::<std::result::Result<_, _>>()
                    .map_err(db_error)?
            };
            if ids.is_empty() {
                return Ok(());
            }
            for id in ids {
                self.command_failed_before_receipt(&id,CoreError::new("OUTCOME_UNKNOWN","Daemon restarted with a durable command intent; no replay or stop confirmation is inferred"),true)?;
            }
        }
    }
}

impl std::fmt::Debug for CommandValidationSelection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandValidationSelection")
            .field("session_id", &self.session_id)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for CommandValidationPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandValidationPolicy")
            .field("id", &self.id)
            .field("revision", &self.revision)
            .field("revoked", &self.revoked)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod cancellation_classification {
    use super::*;
    #[test]
    fn real_precancelled_library_receipt_has_no_child_to_reconcile_or_fence_as_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join("input"), "fixed").unwrap();
        let program = std::fs::canonicalize("/bin/sh").unwrap();
        let bind = |id: &str, base: &Path, path: &Path, hash: Option<String>| {
            BoundArtifact::from_approved_root(
                base,
                ArtifactRequirement {
                    artifact_id: id.into(),
                    relative_path: path.into(),
                    max_bytes: if id == "program" {
                        16 * 1024 * 1024
                    } else {
                        4096
                    },
                    expected_sha256: hash,
                },
            )
            .unwrap()
        };
        let rule = ApprovedCommandRule::from_verified_policy(
            "owner".into(),
            "1".into(),
            ValidationSubject {
                task_id: "task".into(),
                attempt_id: "attempt".into(),
                spec_revision: 1,
                input_revision: "input".into(),
                version: VersionBinding::Data {
                    dataset_revision: "input".into(),
                },
            },
            bind(
                "program",
                program.parent().unwrap(),
                Path::new(program.file_name().unwrap()),
                Some(sha256(&std::fs::read(&program).unwrap())),
            ),
            agentbaton_workspaces::inspect(&root).unwrap(),
            vec![bind(
                "input",
                &root,
                Path::new("input"),
                Some(sha256(b"fixed")),
            )],
            vec![bind("output", &root, Path::new("out"), None)],
            CommandSettings {
                mode: CommandMode::NativeExecutable,
                arguments: vec![],
                environment: BTreeMap::new(),
                expected_exit_codes: vec![0],
                wall_time_ms: 50,
                cleanup_time_ms: 50,
                stdout_limit: 1,
                stderr_limit: 1,
                require: Default::default(),
            },
        )
        .unwrap();
        let run = agentbaton_workspaces::command_validation::run_approved_command(
            &rule,
            &std::sync::atomic::AtomicBool::new(true),
        )
        .unwrap();
        assert!(!run.view().process_started);
        assert!(!has_unreaped_child(&run));
        assert!(!ambiguous_cleanup(&run));
        assert!(!run.view().passed);
        assert!(!root.join("out").exists());
    }
}
