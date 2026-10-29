//! Factory rows and authority are server-derived; public DTOs are views only.
use super::*;
use crate::{
    storage::{db_error, decode, encode, now, Event, Store},
    Result,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use uuid::Uuid;
pub(super) const MAX_FACTORY_BYTES: usize = 1024 * 1024;
pub(super) const MAX_FACTORY_ROWS: i64 = 256;
pub(super) fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST","Select bounded Factory task/grant references and explicit capabilities; caller identity, root and authority are server-derived")
}
pub(super) fn denied() -> CoreError {
    CoreError::new(
        "AUTH_REQUIRED",
        "A live independent Factory owner approval and matching scoped caller are required",
    )
}
pub(super) fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Factory task, input, grant, configuration or workspace binding changed",
    )
}
pub(super) fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Factory byte/count/shared-resource bound reached; local history was not truncated",
    )
}
pub(super) fn key(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
    {
        Err(invalid())
    } else {
        Ok(())
    }
}
pub(super) fn sha<T: Serialize>(value: &T) -> Result<String> {
    Ok(agentbaton_workspaces::validation::sha256(
        encode(value)?.as_bytes(),
    ))
}
pub(super) fn event(
    db: &Connection,
    id: &str,
    revision: u64,
    kind: &str,
    payload: Value,
    source: &str,
) -> Result<i64> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: source.into(),
    };
    // Bound the complete escaped event/IPC form, not snapshot text alone.
    check_wire(
        &json!({"request_id":"\0".repeat(128),"result":&e,"revision":u64::MAX,"warnings":[]}),
    )?;
    db.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(db.last_insert_rowid())
}
pub(super) fn check_wire<T: Serialize>(value: &T) -> Result<()> {
    if serde_json::to_vec(value).map_err(|_| invalid())?.len() > MAX_FACTORY_BYTES {
        Err(limited())
    } else {
        Ok(())
    }
}
pub(super) fn raw<T: for<'de> Deserialize<'de>>(
    db: &Connection,
    table: &str,
    id: &str,
) -> Result<T> {
    key(id)?;
    let query=match table{
 "factory_scopes"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_scopes WHERE id=?1",
 "factory_runs"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_runs WHERE id=?1",
 "factory_delegations"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM factory_delegations WHERE id=?1",
 _=>return Err(invalid())};
    let row: Option<Option<String>> = db
        .query_row(query, [id], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    match row {
        None => Err(CoreError::new("NOT_FOUND", "Factory record not found")),
        Some(None) => Err(limited()),
        Some(Some(raw)) => decode(&raw),
    }
}
pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 13 {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute_batch("CREATE TABLE factory_scopes(id TEXT PRIMARY KEY REFERENCES approval_proposals(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),project_id TEXT NOT NULL REFERENCES projects(id),root_grant_id TEXT NOT NULL REFERENCES execution_grants(id),revision INTEGER NOT NULL CHECK(revision>0),revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),owner_sequence INTEGER NOT NULL REFERENCES events(sequence),document TEXT NOT NULL);
CREATE TABLE factory_runs(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL UNIQUE REFERENCES factory_scopes(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),project_id TEXT NOT NULL REFERENCES projects(id),revision INTEGER NOT NULL CHECK(revision>0),state TEXT NOT NULL,document TEXT NOT NULL);
CREATE TABLE factory_task_lineage(task_id TEXT PRIMARY KEY REFERENCES tasks(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),parent_task_id TEXT REFERENCES tasks(id),scope_id TEXT NOT NULL REFERENCES factory_scopes(id),delegation_id TEXT,depth INTEGER NOT NULL CHECK(depth>=0),document TEXT NOT NULL);
CREATE TABLE factory_delegations(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES factory_scopes(id),run_id TEXT NOT NULL REFERENCES factory_runs(id),parent_task_id TEXT NOT NULL REFERENCES tasks(id),child_task_id TEXT NOT NULL UNIQUE REFERENCES tasks(id),grant_id TEXT NOT NULL REFERENCES execution_grants(id),state TEXT NOT NULL,document TEXT NOT NULL);
CREATE TABLE factory_task_bindings(task_id TEXT NOT NULL REFERENCES tasks(id),scope_id TEXT NOT NULL REFERENCES factory_scopes(id),run_id TEXT NOT NULL REFERENCES factory_runs(id),grant_id TEXT NOT NULL REFERENCES execution_grants(id),state TEXT NOT NULL,document TEXT NOT NULL,PRIMARY KEY(run_id,task_id));
CREATE TABLE factory_attempts(session_id TEXT PRIMARY KEY REFERENCES sessions(id),attempt_id TEXT NOT NULL UNIQUE REFERENCES attempts(id),scope_id TEXT NOT NULL REFERENCES factory_scopes(id),run_id TEXT NOT NULL REFERENCES factory_runs(id),task_id TEXT NOT NULL REFERENCES tasks(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),reservation TEXT NOT NULL,state TEXT NOT NULL,document TEXT NOT NULL);
CREATE TABLE factory_scoped_handles(id TEXT PRIMARY KEY,scope_id TEXT NOT NULL REFERENCES factory_scopes(id),session_id TEXT NOT NULL UNIQUE REFERENCES sessions(id),attempt_id TEXT NOT NULL REFERENCES attempts(id),task_id TEXT NOT NULL REFERENCES tasks(id),token_digest TEXT NOT NULL UNIQUE,expires_at_ms INTEGER NOT NULL,revoked INTEGER NOT NULL CHECK(revoked IN(0,1)),fencing_token INTEGER NOT NULL,document TEXT NOT NULL);
CREATE TABLE factory_handoffs(id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES factory_runs(id),parent_task_id TEXT NOT NULL REFERENCES tasks(id),child_task_id TEXT NOT NULL REFERENCES tasks(id),parent_session_id TEXT NOT NULL REFERENCES sessions(id),child_attempt_id TEXT REFERENCES attempts(id),state TEXT NOT NULL,document TEXT NOT NULL);
CREATE TABLE factory_budget_reservations(attempt_id TEXT PRIMARY KEY REFERENCES attempts(id),scope_id TEXT REFERENCES factory_scopes(id),root_task_id TEXT NOT NULL REFERENCES tasks(id),project_id TEXT NOT NULL REFERENCES projects(id),document TEXT NOT NULL,CHECK(COALESCE((scope_id IS NULL AND json_extract(document,'$.authority_kind')='legacy_shared_session') OR (scope_id IS NOT NULL AND json_extract(document,'$.authority_kind')='factory_task'),0)));
PRAGMA user_version=13;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    super::runtime::migrate_child_planning(db, version)?;
    super::runtime::migrate_child_goals(db, version)
}

pub(super) fn ms() -> Result<i64> {
    i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| invalid())
}
pub(super) fn limits(selection: &FactoryScopeSelection, grant: &ExecutionGrant) -> Result<()> {
    key(&selection.root_task_id)?;
    key(&selection.grant_id)?;
    if selection.connection_ids.is_empty()
        || selection.connection_ids.len() > 4
        || selection.workspace_ids.len() != 1
        || selection.inputs.is_empty()
        || selection.inputs.len() > 16
        || selection.outputs.is_empty()
        || selection.outputs.len() > 16
        || selection.recipients.len() > 64
        || selection.max_depth == 0
        || selection.max_depth > 32
        || selection.max_children == 0
        || selection.max_children > 1000
        || selection.max_total_nodes < 2
        || selection.max_total_nodes > 1000
        || selection.max_children >= selection.max_total_nodes
        || selection.ttl_ms == 0
        || selection.ttl_ms > 3_600_000
    {
        return Err(invalid());
    }
    if !selection.capabilities.delegate_task || !selection.capabilities.resume_parent_as_new_attempt
    {
        return Err(invalid());
    }
    if selection.capabilities.create_workspace {
        return Err(CoreError::new("CAPABILITY_UNSUPPORTED","This Factory contract only dispatches fixed handoffs in existing approved workspaces; child Goal/planning and new-workspace authority are not implemented"));
    }
    if selection.capabilities.child_goal_execution
        != selection.child_goal_execution_policy.is_some()
    {
        return Err(invalid());
    }
    if let Some(policy) = &selection.child_goal_execution_policy {
        super::runtime::child_goal_limits(selection, policy, grant)?;
    }
    if selection.capabilities.child_planning && selection.child_planning_policy.is_none() {
        return Err(CoreError::new("CAPABILITY_UNSUPPORTED","Old Factory snapshots grant no child planning policy; review a new complete Native Factory scope"));
    }
    if !selection.capabilities.child_planning && selection.child_planning_policy.is_some() {
        return Err(invalid());
    }
    if let Some(policy) = &selection.child_planning_policy {
        super::runtime::child_planning_limits(selection, policy)?;
    }
    if !selection.root_budget.narrower(&selection.project_budget)
        || selection.root_budget.attempts > grant.scope.max_attempts
        || selection.root_budget.parallel > grant.scope.max_concurrency
        || selection.root_budget.runtime_ms
            > grant
                .scope
                .max_duration_ms
                .saturating_mul(u64::from(grant.scope.max_attempts))
        || selection.root_budget.children > selection.max_total_nodes - 1
        || selection.root_budget.reworks != 0
        || selection.project_budget.reworks != 0
        || selection.root_budget.tokens.is_some()
        || selection.root_budget.cost_microunits.is_some()
        || selection.project_budget.tokens.is_some()
        || selection.project_budget.cost_microunits.is_some()
    {
        return Err(CoreError::new("ENFORCEMENT_UNAVAILABLE","Factory must fit the existing shared grant and known runtime/output/attempt bounds; unmeasured token/cost or automatic rework is not authorized"));
    }
    for (selected, allowed) in [
        (&selection.connection_ids, &grant.scope.connection_ids),
        (&selection.workspace_ids, &grant.scope.workspace_ids),
        (&selection.recipients, &grant.scope.recipients),
    ] {
        let mut seen = std::collections::BTreeSet::new();
        for id in selected {
            key(id)?;
            if !seen.insert(id) || !allowed.contains(id) {
                return Err(denied());
            }
        }
    }
    Ok(())
}
pub(super) fn companion(path: &std::path::Path) -> Result<FactoryCompanionBinding> {
    companion_controlled(path, None)
}
pub(super) fn companion_controlled(
    path: &std::path::Path,
    control: Option<&crate::factory::FactoryReadControl>,
) -> Result<FactoryCompanionBinding> {
    if let Some(control) = control {
        control.check()?;
    }
    use sha2::{Digest, Sha256};
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err(invalid());
    }
    let canonical = std::fs::canonicalize(path).map_err(|_| {
        CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "The host-selected Baton MCP companion is not installed",
        )
    })?;
    if canonical != path {
        return Err(changed());
    }
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC)
                .bits() as i32,
        )
        .open(path)
        .map_err(|_| changed())?;
    let before = file.metadata().map_err(|_| changed())?;
    if !before.is_file()
        || before.nlink() != 1
        || before.len() > 128 * 1024 * 1024
        || before.mode() & 0o022 != 0
        || before.mode() & 0o111 == 0
    {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Baton MCP companion must be a bounded non-writable-by-group/world regular executable",
        ));
    }
    let mut observed = 0u64;
    let mut hasher = Sha256::new();
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if let Some(control) = control {
            control.check()?;
        }
        let n = file.read(&mut chunk).map_err(|_| changed())?;
        if let Some(control) = control {
            control.check()?;
        }
        if n == 0 {
            break;
        }
        observed = observed.checked_add(n as u64).ok_or_else(limited)?;
        if observed > 128 * 1024 * 1024 {
            return Err(limited());
        }
        hasher.update(&chunk[..n]);
    }
    let current = std::fs::symlink_metadata(path).map_err(|_| changed())?;
    if observed != before.len()
        || !current.is_file()
        || (
            current.dev(),
            current.ino(),
            current.len(),
            current.mode(),
            current.mtime(),
            current.mtime_nsec(),
            current.ctime(),
            current.ctime_nsec(),
        ) != (
            before.dev(),
            before.ino(),
            before.len(),
            before.mode(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec(),
        )
    {
        return Err(changed());
    }
    Ok(FactoryCompanionBinding {
        executable: canonical,
        executable_sha256: format!("{:x}", hasher.finalize()),
        argument_template: vec!["factory-mcp".into(), "${SESSION_HANDLE_FILE}".into()],
        credential_transport: "private_0600_file_reference".into(),
        global_daemon_token_available: false,
    })
}
impl Store {
    pub(super) fn resolve_factory_bindings(
        &self,
        selection: &FactoryScopeSelection,
        companion_path: &std::path::Path,
    ) -> Result<FactoryScopeBindings> {
        if let Some(proof) = self.data_input.as_ref().and_then(|d| d.factory.as_deref()) {
            return proof.recheck(self, selection, companion_path);
        }
        if let Some(control) = &self.factory_read_control {
            control.check()?;
        }
        let task = self.get(&selection.root_task_id)?;
        let grant = crate::policy::checked_lineage(&self.conn, &selection.grant_id)?;
        if crate::factory::task_budget_root(&self.conn, &task.id)? != task.id {
            return Err(CoreError::new("AUTH_REQUIRED","An existing Factory child cannot be relabeled as a new root; its original ancestry and cumulative budgets remain"));
        }

        limits(selection, &grant)?;
        if task.revision != selection.expected_task_revision
            || grant.revision != selection.expected_grant_revision
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Factory task or grant changed before owner review",
            ));
        }
        if task.project_id != grant.project_id
            || grant.parent_id.is_some()
            || grant.root_grant_id != grant.id
            || !matches!(task.state, crate::storage::TaskState::Draft)
        {
            return Err(denied());
        }
        let historical_busy:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM factory_runs WHERE root_task_id=?1 AND state NOT IN('completed','cancelled'))",[&task.id],|r|r.get(0)).map_err(db_error)?;
        if historical_busy {
            return Err(CoreError::new("OUTCOME_UNKNOWN","A previous Factory run remains live, blocked or unknown; revocation is not stop or reconciliation"));
        }
        for (project_scope, id, selected) in [
            (true, task.project_id.as_str(), &selection.project_budget),
            (false, task.id.as_str(), &selection.root_budget),
        ] {
            if let Some(existing) = self.budget(project_scope, id)? {
                if selected.attempts > existing.total_limit
                    || selected.parallel > existing.parallel_limit
                {
                    return Err(denied());
                }
                if existing.consumed >= selected.attempts || existing.active >= selected.parallel {
                    return Err(limited());
                }
            }
        }
        for (scope_key, selected) in [
            (
                format!("project:{}", task.project_id),
                &selection.project_budget,
            ),
            (format!("root-grant:{}", grant.id), &selection.root_budget),
        ] {
            let prior:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM goal_ledgers WHERE scope=?1",[scope_key],|r|r.get(0)).optional().map_err(db_error)?;
            if let Some(prior) = prior {
                let old: Value = decode(&prior.ok_or_else(limited)?)?;
                let limits: crate::goal_plan::BudgetLimits =
                    serde_json::from_value(old["limits"].clone()).map_err(|_| changed())?;
                if !selected.narrower(&limits) {
                    return Err(denied());
                }
                if old["runtime_ms"].is_null() || old["output_bytes"].is_null() {
                    return Err(CoreError::new("BUDGET_UNKNOWN","Historical shared runtime/output usage is unknown; a Factory scope cannot reset it"));
                }
            }
        }
        let project = self.get_project(&task.project_id)?;
        if project.archived {
            return Err(denied());
        }
        let unresolved:bool=self.conn.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE a.task_id=?1 AND d.state!='stopped')",[&task.id],|r|r.get(0)).map_err(db_error)?;
        if unresolved {
            return Err(CoreError::new("OUTCOME_UNKNOWN","Factory root has an active or unreconciled original attempt; it cannot be reused by relabeling it waiting"));
        }
        let mut connections = Vec::new();
        for id in &selection.connection_ids {
            let c = self.get_connection(id)?;
            if c.host_id != "local"
                || c.probe_state != "ready"
                || c.protocol_route != agentbaton_providers::connections::ProtocolRoute::CustomAcp
                || c.runtime_plan.is_none()
            {
                return Err(CoreError::new("CAPABILITY_UNSUPPORTED","The first Factory runtime requires an existing approved local managed custom ACP connection with explicit fixed MCP support; no external/native configuration is changed"));
            }
            if let Some(plan) = &c.runtime_plan {
                plan.validate().map_err(|_| changed())?;
            }
            connections.push(c);
        }
        let mut workspaces = Vec::new();
        for id in &selection.workspace_ids {
            let row:Option<Option<String>>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM managed_workspaces WHERE id=?1",[id],|r|r.get(0)).optional().map_err(db_error)?;
            let w: crate::registry::ManagedWorkspace =
                decode(&row.ok_or_else(changed)?.ok_or_else(limited)?)?;
            workspaces.push(w);
        }
        for workspace in &workspaces {
            for connection in &connections {
                crate::policy::validate_grant_from_with_data(
                    &self.conn,
                    &grant.id,
                    &task.project_id,
                    &workspace.id,
                    &connection.id,
                    self.data_input.as_ref(),
                )?;
            }
        }
        let root = &workspaces[0].root;
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut total = 0usize;
        for r in &selection.inputs {
            if let Some(control) = &self.factory_read_control {
                control.check()?;
            }
            key(&r.artifact_id)?;
            if !seen.insert(&r.artifact_id) || r.max_bytes == 0 || r.max_bytes > 16 * 1024 * 1024 {
                return Err(invalid());
            }
            let bound = agentbaton_workspaces::validation::BoundArtifact::from_approved_root(
                root,
                r.clone(),
            )
            .map_err(|_| invalid())?;
            let (digest, observed) = if matches!(
                workspaces[0].identity.as_ref().map(|i| &i.kind),
                Some(agentbaton_workspaces::WorkspaceKind::Directory)
            ) {
                self.data_input
                    .as_ref()
                    .ok_or_else(denied)?
                    .selected_input_observation(&self.conn, &workspaces[0], r)?
            } else {
                let bytes =
                    agentbaton_workspaces::read_artifact(root, &r.relative_path, r.max_bytes)
                        .map_err(|_| changed())?;
                (
                    agentbaton_workspaces::validation::sha256(&bytes),
                    bytes.len() as u64,
                )
            };
            if let Some(control) = &self.factory_read_control {
                control.check()?;
            }
            total = total
                .checked_add(usize::try_from(observed).map_err(|_| limited())?)
                .ok_or_else(limited)?;
            if total > 64 * 1024 * 1024 {
                return Err(limited());
            }
            if r.expected_sha256
                .as_ref()
                .is_some_and(|expected| expected != &digest)
            {
                return Err(changed());
            }
            inputs.push(FactoryInputBinding {
                contract: bound.contract_view(),
                sha256: digest,
                bytes: observed,
            });
        }
        for r in &selection.outputs {
            key(&r.artifact_id)?;
            if !seen.insert(&r.artifact_id) || r.max_bytes == 0 || r.max_bytes > 64 * 1024 * 1024 {
                return Err(invalid());
            }
            outputs.push(
                agentbaton_workspaces::validation::BoundArtifact::from_approved_root(
                    root,
                    r.clone(),
                )
                .map_err(|_| invalid())?
                .contract_view(),
            );
        }
        let root_request = json!({"schema_version":1,"kind":"agentbaton_factory_task_request","goal":task.goal,"root_task_id":task.id,"inputs":inputs,"outputs":outputs,"root_budget":selection.root_budget,"capabilities":selection.capabilities});
        let child_planning =
            super::runtime::resolve_child_planning_binding(&self.conn, selection, &workspaces[0])?;
        let child_goal_execution =
            super::runtime::resolve_child_goal_binding(selection, &grant, child_planning.as_ref())?;
        Ok(FactoryScopeBindings {
            child_goal_execution,
            child_planning,
            project,
            root_task: task,
            approved_grant: grant,
            connections,
            workspaces,
            inputs,
            outputs,
            companion: companion_controlled(companion_path, self.factory_read_control.as_ref())?,
            guarantees: FactoryGuarantees {
                filesystem_sandbox: false,
                network_sandbox: false,
                account_isolation: false,
                output_quota: false,
                continuous_input_immutability: false,
                complete_process_tree_containment: false,
            },
            start_contract: FactoryStartContract {
                mode: "single_scope_single_run".into(),
                root_task_request_digest: sha(&root_request)?,
                context_continuation: "new_attempt_explicit_handoff_only".into(),
                implicit_start_on_approval: false,
                new_workspace_authority: false,
            },
        })
    }
}

pub(super) fn replay<T: for<'de> Deserialize<'de>>(
    db: &Connection,
    scope: &str,
    id: &str,
    payload: &str,
) -> Result<Option<T>> {
    key(id)?;
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN octet_length(payload)<=1048576 AND octet_length(result)<=1048576 THEN payload END, CASE WHEN octet_length(payload)<=1048576 AND octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope,id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    match row {
        None => Ok(None),
        Some((Some(old), Some(result))) => {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Factory idempotency key already identifies another exact request",
                ));
            }
            Ok(Some(decode(&result)?))
        }
        Some(_) => Err(limited()),
    }
}
pub(super) fn remember<T: Serialize>(
    db: &Connection,
    scope: &str,
    id: &str,
    payload: &str,
    result: &T,
) -> Result<()> {
    check_wire(
        &json!({"request_id":"\0".repeat(128),"result":result,"revision":u64::MAX,"warnings":[]}),
    )?;
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, id, payload, encode(result)?],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(super) fn scope(db: &Connection, id: &str) -> Result<FactoryScope> {
    let s: FactoryScope = raw(db, "factory_scopes", id)?;
    let columns:(String,String,String,i64,bool,i64)=db.query_row("SELECT root_task_id,project_id,root_grant_id,revision,revoked,owner_sequence FROM factory_scopes WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(db_error)?;
    if s.id != id
        || s.approval_id != id
        || columns.0 != s.root_task_id
        || columns.1 != s.project_id
        || columns.2 != s.root_grant_id
        || columns.3 != i64::try_from(s.revision).map_err(|_| changed())?
        || columns.4 != s.revoked
    {
        return Err(changed());
    }
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,CASE WHEN octet_length(result)<=1048576 THEN result END FROM approval_proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let (p, r) = row.ok_or_else(denied)?;
    let p: crate::sessions::ApprovalProposal = decode(&p.ok_or_else(limited)?)?;
    let result: FactoryScope = decode(&r.ok_or_else(limited)?)?;
    let mut immutable_current = serde_json::to_value(&s).map_err(|_| changed())?;
    immutable_current["revision"] = json!(1);
    immutable_current["revoked"] = json!(false);
    if immutable_current != serde_json::to_value(&result).map_err(|_| changed())?
        || result.revision != 1
        || result.revoked
        || s.revision != if s.revoked { 2 } else { 1 }
        || s.root_task_id != s.selection.root_task_id
        || s.root_task_id != s.bindings.root_task.id
        || s.project_id != s.bindings.project.id
        || s.project_id != s.bindings.root_task.project_id
        || s.root_grant_id != s.selection.grant_id
        || s.root_grant_id != s.bindings.approved_grant.id
        || s.root_grant_revision != s.selection.expected_grant_revision
        || s.root_grant_revision != s.bindings.approved_grant.revision
        || s.expires_at_ms > s.bindings.approved_grant.expires_at_ms
    {
        return Err(denied());
    }
    let original_digest =
        sha(&json!({"schema_version":1,"selection":s.selection,"bindings":s.bindings}))?;
    if p.kind != "factory"
        || !p.consumed
        || p.id != s.approval_id
        || result.id != s.id
        || result.contract_digest != s.contract_digest
        || s.contract_digest != original_digest
        || p.snapshot["selection"] != json!(s.selection)
        || p.snapshot["bindings"] != json!(s.bindings)
        || p.snapshot["contract_digest"] != s.contract_digest
    {
        return Err(denied());
    }
    let raw_event:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM events WHERE sequence=?1",[columns.5],|r|r.get(0)).optional().map_err(db_error)?;
    let receipt: Event = decode(&raw_event.flatten().ok_or_else(denied)?)?;
    if columns.5 <= 0
        || receipt.aggregate_id != s.id
        || receipt.kind != "approval.confirmed"
        || receipt.source != "local_owner_confirmation"
        || receipt.payload["kind"] != "factory"
        || receipt.payload["contract_digest"] != s.contract_digest
        || receipt.payload["factory_scope_id"] != s.id
        || receipt.payload["spawned"] != false
    {
        return Err(denied());
    }
    Ok(s)
}
impl Store {
    pub fn get_factory_scope(&self, id: &str) -> Result<FactoryScope> {
        scope(&self.conn, id)
    }
    pub fn list_factory_scopes(&self, root_task_id: &str) -> Result<Vec<FactoryScope>> {
        key(root_task_id)?;
        let(count,bytes):(i64,i64)=self.conn.query_row("SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM factory_scopes WHERE root_task_id=?1",[root_task_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if count > MAX_FACTORY_ROWS || bytes > MAX_FACTORY_BYTES as i64 {
            return Err(limited());
        }
        let mut q = self
            .conn
            .prepare("SELECT id FROM factory_scopes WHERE root_task_id=?1 ORDER BY rowid")
            .map_err(db_error)?;
        let ids = q
            .query_map([root_task_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        let records = ids
            .iter()
            .map(|id| scope(&self.conn, id))
            .collect::<Result<Vec<_>>>()?;
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":records,"revision":u64::MAX,"warnings":[]}),
        )?;
        Ok(records)
    }
    pub fn propose_factory_scope(
        &mut self,
        selection: &FactoryScopeSelection,
        key_id: &str,
        companion_path: &std::path::Path,
    ) -> Result<crate::sessions::ApprovalProposal> {
        let payload = encode(selection)?;
        if payload.len() > MAX_FACTORY_BYTES / 4 {
            return Err(limited());
        }
        if let Some(old) = replay(&self.conn, "factory:propose", key_id, &payload)? {
            return Ok(old);
        }
        let bindings = self.resolve_factory_bindings(selection, companion_path)?;
        let active: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM factory_scopes WHERE root_task_id=?1 AND revoked=0)",
                [&selection.root_task_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if active {
            return Err(CoreError::new("CONFIG_CONFLICT","Revoke the existing Factory authority before approving a replacement; its consumed budget and lineage remain"));
        }
        let snapshot_base = json!({"schema_version":1,"selection":selection,"bindings":bindings});
        let digest = sha(&snapshot_base)?;
        let mut snapshot = snapshot_base;
        snapshot["contract_digest"] = json!(digest);
        let created = now()?;
        let expires = ms()?.checked_add(300_000).ok_or_else(invalid)?;
        let proposal = crate::sessions::ApprovalProposal {
            id: Uuid::new_v4().to_string(),
            kind: "factory".into(),
            revision: 1,
            snapshot,
            created_at: created,
            expires_at_ms: expires,
            consumed: false,
        };
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":proposal,"revision":u64::MAX,"warnings":[]}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
            params![proposal.id, encode(&proposal)?],
        )
        .map_err(db_error)?;
        remember(&tx, "factory:propose", key_id, &payload, &proposal)?;
        event(
            &tx,
            &proposal.id,
            1,
            "approval.proposed",
            json!({"kind":"factory","snapshot":proposal.snapshot}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(proposal)
    }
    pub(crate) fn finalize_factory_scope(
        &mut self,
        id: &str,
        revision: u64,
        companion_path: &std::path::Path,
    ) -> Result<Value> {
        key(id)?;
        let row:Option<(Option<String>,Option<String>,bool)>=self.conn.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END,CASE WHEN COALESCE(octet_length(result),0)<=1048576 THEN result END,result IS NOT NULL FROM approval_proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
        let (document, result, has_result) = row.ok_or_else(denied)?;
        let mut p: crate::sessions::ApprovalProposal = decode(&document.ok_or_else(limited)?)?;
        if p.kind != "factory" || p.id != id {
            return Err(denied());
        }
        if has_result {
            let original: FactoryScope = decode(&result.ok_or_else(limited)?)?;
            let _ = scope(&self.conn, id)?;
            return Ok(json!(original));
        }
        if p.consumed || p.revision != revision || p.expires_at_ms <= ms()? {
            return Err(denied());
        }
        let selection: FactoryScopeSelection =
            serde_json::from_value(p.snapshot["selection"].clone()).map_err(|_| invalid())?;
        let bindings = self.resolve_factory_bindings(&selection, companion_path)?;
        if p.snapshot["bindings"] != json!(bindings) {
            return Err(changed());
        }
        let digest = sha(&json!({"schema_version":1,"selection":selection,"bindings":bindings}))?;
        if p.snapshot["contract_digest"] != digest {
            return Err(changed());
        }
        let record = FactoryScope {
            id: p.id.clone(),
            approval_id: p.id.clone(),
            project_id: bindings.root_task.project_id.clone(),
            root_task_id: selection.root_task_id.clone(),
            root_grant_id: bindings.approved_grant.id.clone(),
            root_grant_revision: bindings.approved_grant.revision,
            revision: 1,
            revoked: false,
            expires_at_ms: ms()?
                .checked_add(i64::try_from(selection.ttl_ms).map_err(|_| invalid())?)
                .ok_or_else(invalid)?
                .min(bindings.approved_grant.expires_at_ms),
            selection,
            bindings,
            contract_digest: digest,
            created_at: now()?,
        };
        check_wire(
            &json!({"request_id":"\0".repeat(128),"result":record,"revision":u64::MAX,"warnings":[]}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        check_admission_db(&tx, &record.bindings)?;
        let(current_task,current_grant):(String,String)=tx.query_row("SELECT t.document,g.document FROM tasks t JOIN execution_grants g ON g.id=?1 WHERE t.id=?2",params![record.root_grant_id,record.root_task_id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if encode(&decode::<Task>(&current_task)?)? != encode(&record.bindings.root_task)?
            || encode(&decode::<ExecutionGrant>(&current_grant)?)?
                != encode(&record.bindings.approved_grant)?
        {
            return Err(changed());
        }
        let active: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM factory_scopes WHERE root_task_id=?1 AND revoked=0)",
                [&record.root_task_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if active {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Another Factory scope became active before native confirmation",
            ));
        }
        p.consumed = true;
        p.revision += 1;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3",
            params![encode(&p)?, encode(&record)?, p.id],
        )
        .map_err(db_error)?;
        let owner_sequence = event(
            &tx,
            id,
            p.revision,
            "approval.confirmed",
            json!({"kind":"factory","factory_scope_id":id,"contract_digest":record.contract_digest,"root_task_id":record.root_task_id,"root_grant_id":record.root_grant_id,"original_grant_revision_unchanged":record.root_grant_revision,"spawned":false,"authority":"independent_task_delegation_only"}),
            "local_owner_confirmation",
        )?;
        tx.execute("INSERT INTO factory_scopes(id,root_task_id,project_id,root_grant_id,revision,revoked,owner_sequence,document) VALUES(?1,?2,?3,?4,1,0,?5,?6)",params![record.id,record.root_task_id,record.project_id,record.root_grant_id,owner_sequence,encode(&record)?]).map_err(db_error)?;
        check_admission_db(&tx, &record.bindings)?;
        tx.commit().map_err(db_error)?;
        Ok(json!(record))
    }
    pub fn revoke_factory_scope(
        &mut self,
        id: &str,
        revision: u64,
        key_id: &str,
    ) -> Result<FactoryScope> {
        key(id)?;
        let payload = encode(&json!({"scope_id":id,"revision":revision}))?;
        let idem = format!("factory:{id}:revoke");
        if let Some(old) = replay(&self.conn, &idem, key_id, &payload)? {
            return Ok(old);
        }
        let mut s = scope(&self.conn, id)?;
        if s.revision != revision {
            return Err(CoreError::new("STALE_REVISION", "Factory scope changed"));
        }
        if s.revoked {
            return Err(denied());
        }
        s.revoked = true;
        s.revision += 1;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE factory_scopes SET document=?1,revision=?2,revoked=1 WHERE id=?3",
            params![
                encode(&s)?,
                i64::try_from(s.revision).map_err(|_| invalid())?,
                id
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE factory_scoped_handles SET revoked=1 WHERE scope_id=?1",
            [id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            s.revision,
            "factory.scope_revoked",
            json!({"stop_confirmed":false,"old_grant_unchanged":true}),
            "factory_core",
        )?;
        remember(&tx, &idem, key_id, &payload, &s)?;
        tx.commit().map_err(db_error)?;
        Ok(s)
    }
}
impl Store {
    /// Read-only test/host projection. Never returns credential digests or secrets.
    pub fn factory_record_counts(&self, root_task_id: &str) -> Result<FactoryRecordCounts> {
        key(root_task_id)?;
        let scopes = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_scopes WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)? as u64;
        let runs = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_runs WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)? as u64;
        let task_lineage = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_task_lineage WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)? as u64;
        let task_bindings=self.conn.query_row("SELECT count(*) FROM factory_task_bindings t JOIN factory_scopes s ON s.id=t.scope_id WHERE s.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let delegations=self.conn.query_row("SELECT count(*) FROM factory_delegations d JOIN factory_scopes s ON s.id=d.scope_id WHERE s.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let attempts = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_attempts WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)? as u64;
        let scoped_handles=self.conn.query_row("SELECT count(*) FROM factory_scoped_handles h JOIN factory_scopes s ON s.id=h.scope_id WHERE s.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let handoffs=self.conn.query_row("SELECT count(*) FROM factory_handoffs h JOIN factory_runs r ON r.id=h.run_id WHERE r.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let budget_reservations = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_budget_reservations WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)? as u64;
        let child_plan_heads=self.conn.query_row("SELECT count(*) FROM factory_child_plan_heads p JOIN factory_scopes s ON s.id=p.scope_id WHERE s.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let child_plan_revisions=self.conn.query_row("SELECT count(*) FROM factory_child_plan_revisions p JOIN factory_scopes s ON s.id=p.scope_id WHERE s.root_task_id=?1",[root_task_id],|r|r.get::<_,i64>(0)).map_err(db_error)? as u64;
        let child_goal_runs = self
            .conn
            .query_row(
                "SELECT count(*) FROM factory_child_goal_runs WHERE root_task_id=?1",
                [root_task_id],
                |r| r.get::<_, i64>(0),
            )
            .map_err(db_error)?;
        Ok(FactoryRecordCounts {
            child_goal_runs: u64::try_from(child_goal_runs).map_err(|_| changed())?,
            child_plan_heads,
            child_plan_revisions,
            scopes,
            runs,
            task_lineage,
            task_bindings,
            delegations,
            attempts,
            scoped_handles,
            handoffs,
            budget_reservations,
        })
    }
}

/// Recheck all server DB predicates under the caller's IMMEDIATE transaction.
/// External bytes/identities are observed separately; this is not an OS freeze.
pub(super) fn check_admission_db(db: &Connection, b: &FactoryScopeBindings) -> Result<()> {
    let task: Task = read_bound(db, "tasks", &b.root_task.id)?;
    let project: crate::registry::Project = read_bound(db, "projects", &b.project.id)?;
    if task.state != crate::storage::TaskState::Draft
        || encode(&task)? != encode(&b.root_task)?
        || project.archived
        || encode(&project)? != encode(&b.project)?
        || encode(&crate::policy::checked_lineage(db, &b.approved_grant.id)?)?
            != encode(&b.approved_grant)?
    {
        return Err(changed());
    }
    let unresolved:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM dispatch_intents d JOIN attempts a ON a.id=d.attempt_id WHERE a.task_id=?1 AND d.state!='stopped')",[&task.id],|r|r.get(0)).map_err(db_error)?;
    if unresolved {
        return Err(changed());
    }
    for c in &b.connections {
        let actual: crate::sessions::ProviderConnection = read_bound(db, "connections", &c.id)?;
        if encode(&actual)? != encode(c)? {
            return Err(changed());
        }
    }
    for w in &b.workspaces {
        let actual: crate::registry::ManagedWorkspace =
            read_bound(db, "managed_workspaces", &w.id)?;
        if encode(&actual)? != encode(w)? {
            return Err(changed());
        }
    }
    Ok(())
}
fn read_bound<T: for<'de> Deserialize<'de>>(db: &Connection, table: &str, id: &str) -> Result<T> {
    let query=match table {
      "tasks"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM tasks WHERE id=?1",
      "projects"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM projects WHERE id=?1",
      "connections"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM connections WHERE id=?1",
      "managed_workspaces"=>"SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM managed_workspaces WHERE id=?1",
      _=>return Err(invalid()),
    };
    let raw: Option<Option<String>> = db
        .query_row(query, [id], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    decode(&raw.flatten().ok_or_else(changed)?)
}
