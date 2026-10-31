//! Persisted local supervision. The daemon is the sole database writer; owned workers
//! perform only approved subprocess IO and report through bounded channels.
//! Private HOME and cleared environment are not OS or native-account isolation.
use crate::{
    execution::{DispatchIntent, ExecutionBinding, IntentState, StopEvidence},
    policy::{validate_grant_from_with_data, validate_grant_live_from_with_data},
    registry::ManagedWorkspace,
    storage::{db_error, decode, encode, now, Event, Store, Task, TaskState},
    CoreError, Result,
};
use agentbaton_config::runtime::RuntimePlan;
use agentbaton_config::{
    runtime::{RuntimeRequirements, RuntimeRoot},
    Ownership,
};
use agentbaton_providers::{
    connections::ProtocolRoute,
    driver::ManagedProviderProcess,
    installation::{
        executable_digest, probe_version, verify_executable, BinaryDigest, VersionEvidence,
        VersionProbeRequest, VersionProbeState,
    },
    transport::ApprovedInstallation,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashMap},
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use time::OffsetDateTime;
use uuid::Uuid;

// Unit-test-only, one-shot fault injection at the exact atomic boundary. No
// product build, feature, IPC request or Provider can install this callback.
#[cfg(test)]
pub(crate) mod goal_reservation_fault_fixture {
    use super::{Connection, Task};
    type Fault = Box<dyn FnOnce(&Connection, &mut Task)>;
    std::thread_local! {
        static FAULT: std::cell::RefCell<Option<Fault>> = std::cell::RefCell::new(None);
    }
    pub(crate) struct Guard(std::marker::PhantomData<std::rc::Rc<()>>);
    pub(crate) fn install(fault: impl FnOnce(&Connection, &mut Task) + 'static) -> Guard {
        FAULT.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "no nested reservation fault fixture");
            *slot = Some(Box::new(fault));
        });
        Guard(std::marker::PhantomData)
    }
    pub(super) fn apply(db: &Connection, task: &mut Task) {
        let fault = FAULT.with(|slot| slot.borrow_mut().take());
        if let Some(fault) = fault {
            fault(db, task);
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            FAULT.with(|slot| {
                let _ = slot.borrow_mut().take();
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConnection {
    pub id: String,
    pub name: String,
    pub provider: String,
    #[serde(default = "crate::routes::default_route")]
    pub protocol_route: ProtocolRoute,
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub executable_sha256: String,
    pub environment_id: String,
    pub identity_ref: String,
    pub config_revision: String,
    #[serde(default)]
    pub runtime_plan: Option<RuntimePlan>,
    pub host_id: String,
    pub approval_id: String,
    pub probe_state: String,
    pub revision: u64,
    pub capability_snapshot_id: String,
    pub identity_status: String,
    pub config_status: String,
    pub created_at: String,
    pub version_evidence: Option<VersionEvidence>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSession {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub workspace_id: String,
    pub connection_id: String,
    pub grant_id: String,
    pub state: String,
    pub revision: u64,
    pub native_session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_error: Option<String>,
    pub turn_state: Option<String>,
    pub attempt_id: String,
    pub intent_id: String,
    pub fencing_token: u64,
    #[serde(default)]
    pub runtime_snapshot: Option<crate::RuntimeBinding>,
}
fn invalid(message: &str) -> CoreError {
    CoreError::new("INVALID_REQUEST", message)
}
fn key_valid(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 {
        Err(invalid("A bounded idempotency key is required"))
    } else {
        Ok(())
    }
}
fn ms() -> Result<i64> {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| CoreError::new("INTERNAL_ERROR", "Clock outside supported range"))
}
fn event(
    tx: &Transaction<'_>,
    id: &str,
    revision: u64,
    kind: &str,
    payload: Value,
    source: &str,
) -> Result<()> {
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
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(())
}
fn connection_from(conn: &Connection, id: &str) -> Result<ProviderConnection> {
    let raw: Option<String> = conn
        .query_row("SELECT document FROM connections WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Connection does not exist"))?)
}
fn session_from(conn: &Connection, id: &str) -> Result<ManagedSession> {
    let raw: Option<String> = conn
        .query_row("SELECT document FROM sessions WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Session does not exist"))?)
}
fn save_session(
    tx: &Transaction<'_>,
    s: &mut ManagedSession,
    kind: &str,
    payload: Value,
    source: &str,
) -> Result<()> {
    s.revision += 1;
    s.updated_at = now()?;
    tx.execute(
        "UPDATE sessions SET document=?1,state=?2 WHERE id=?3",
        params![encode(s)?, s.state, s.id],
    )
    .map_err(db_error)?;
    event(tx, &s.id, s.revision, kind, payload, source)
}
struct ExpectedConnectionSnapshot<'a> {
    executable_sha256: &'a str,
    runtime_plan_digest: &'a str,
}
impl Store {
    /// A local authenticated registration authorizes only one bounded version probe.
    /// A separate persisted execution grant is mandatory before session launch.
    pub fn register_connection(
        &mut self,
        name: &str,
        provider: &str,
        executable: &Path,
        arguments: &[String],
        key: &str,
    ) -> Result<ProviderConnection> {
        self.register_connection_exact(name, provider, executable, arguments, key, None)
    }
    fn register_connection_exact(
        &mut self,
        name: &str,
        provider: &str,
        executable: &Path,
        arguments: &[String],
        key: &str,
        expected: Option<ExpectedConnectionSnapshot<'_>>,
    ) -> Result<ProviderConnection> {
        key_valid(key)?;
        let protocol_route = crate::routes::validate_route_arguments(provider, arguments)?;
        let runtime_plan = crate::runtime_config::plan_for_provider(provider)?;
        let plan_digest = runtime_plan
            .plan_digest()
            .map_err(crate::runtime_config::config_error)?;
        if expected
            .as_ref()
            .is_some_and(|expected| expected.runtime_plan_digest != plan_digest)
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Runtime configuration changed since the owner proposal",
            ));
        }
        if name.trim().is_empty()
            || name.len() > 256
            || arguments.len() > 32
            || arguments.iter().any(|a| a.len() > 4096)
        {
            return Err(invalid("Connection name and arguments must be bounded"));
        }
        let payload = encode(
            &json!({"name":name,"provider":provider,"executable":executable,"arguments":arguments}),
        )?;
        let old:Option<(String,String)>=self.conn.query_row("SELECT payload,result FROM idempotency WHERE scope='local:connections.register' AND key=?1",[key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((old, result)) = old {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Connection key already used with another registration",
                ));
            }
            let c: ProviderConnection = decode(&result)?;
            return self.get_connection(&c.id);
        }
        let digest = executable_digest(executable).map_err(|_| {
            invalid("Select an existing canonical executable; digest verification failed")
        })?;
        if expected
            .as_ref()
            .is_some_and(|expected| expected.executable_sha256 != digest.as_str())
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Selected executable changed since the owner confirmation proposal",
            ));
        }
        let id = Uuid::new_v4().to_string();
        let config_revision =
            crate::runtime_config::configuration_revision(arguments, protocol_route, &plan_digest)?;
        let c = ProviderConnection {
            id: id.clone(),
            name: name.into(),
            provider: provider.into(),
            protocol_route,
            executable: executable.into(),
            arguments: arguments.to_vec(),
            executable_sha256: digest.as_str().into(),
            environment_id: Uuid::new_v4().to_string(),
            identity_ref: format!("unknown:{id}"),
            config_revision,
            runtime_plan: Some(runtime_plan),
            host_id: "local".into(),
            approval_id: Uuid::new_v4().to_string(),
            probe_state: "pending".into(),
            revision: 1,
            capability_snapshot_id: format!("client-no-tools:{id}"),
            identity_status: "unknown".into(),
            config_status: "private_environment_not_sandbox".into(),
            created_at: now()?,
            version_evidence: None,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "INSERT INTO connections VALUES(?1,?2)",
            params![c.id, encode(&c)?],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO idempotency VALUES('local:connections.register',?1,?2,?3)",
            params![key, payload, encode(&c)?],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &c.id,
            c.revision,
            "connection.probe_authorized",
            json!({"connection_id":c.id,"executable_sha256":c.executable_sha256,"approval_id":c.approval_id,"scope":"version_probe_only","identity_status":"unknown"}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(c)
    }
    pub fn get_connection(&self, id: &str) -> Result<ProviderConnection> {
        connection_from(&self.conn, id)
    }
    pub fn list_connections(&self) -> Result<Vec<ProviderConnection>> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM connections", [], |r| r.get(0))
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Connection listing exceeds its bounded limit",
            ));
        }
        let mut stmt = self
            .conn
            .prepare("SELECT document FROM connections ORDER BY rowid DESC")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub fn get_session(&self, id: &str) -> Result<ManagedSession> {
        session_from(&self.conn, id)
    }
    pub fn list_sessions(&self) -> Result<Vec<ManagedSession>> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Session listing exceeds its bounded limit",
            ));
        }
        let mut stmt = self
            .conn
            .prepare("SELECT document FROM sessions ORDER BY rowid DESC")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub fn create_session(
        &mut self,
        task_id: &str,
        workspace_id: &str,
        connection_id: &str,
        grant_id: &str,
        expected_revision: u64,
        key: &str,
    ) -> Result<ManagedSession> {
        self.create_session_inner(
            (
                task_id,
                workspace_id,
                connection_id,
                grant_id,
                expected_revision,
                key,
            ),
            None,
            None,
        )
    }
    pub(crate) fn create_session_for_goal(
        &mut self,
        request: (&str, &str, &str, &str, u64, &str),
        context: crate::goal_store::GoalDispatchContext,
    ) -> Result<ManagedSession> {
        self.create_session_inner(request, Some(context), None)
    }
    pub(crate) fn create_session_for_factory(
        &mut self,
        request: (&str, &str, &str, &str, u64, &str),
        context: crate::factory::FactoryDispatchContext,
    ) -> Result<ManagedSession> {
        self.create_session_inner(request, None, Some(context))
    }
    fn create_session_inner(
        &mut self,
        request: (&str, &str, &str, &str, u64, &str),
        goal: Option<crate::goal_store::GoalDispatchContext>,
        factory: Option<crate::factory::FactoryDispatchContext>,
    ) -> Result<ManagedSession> {
        #[cfg(debug_assertions)]
        let mut factory_phase_clock = factory.as_ref().map(|_| Instant::now());
        let (task_id, workspace_id, connection_id, grant_id, expected_revision, key) = request;
        key_valid(key)?;
        let payload = encode(
            &json!({"task_id":task_id,"workspace_id":workspace_id,"connection_id":connection_id,"grant_id":grant_id,"expected_revision":expected_revision}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT payload,document FROM sessions WHERE task_id=?1 AND idempotency_key=?2",
                params![task_id, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((old, document)) = old {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Session key already used with another request",
                ));
            }
            return decode(&document);
        }
        let command_fenced: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1)",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if command_fenced {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace has an active or unreconciled command writer",
            ));
        }
        let raw: Option<String> = tx
            .query_row("SELECT document FROM tasks WHERE id=?1", [task_id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(db_error)?;
        let mut task: Task =
            decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Task does not exist"))?)?;
        let factory_goal_root = goal
            .as_ref()
            .map(|context| crate::goal_store::factory_goal_session_root(&tx, context, &task.id))
            .transpose()?
            .flatten();
        if factory.is_none()
            && factory_goal_root.is_none()
            && crate::factory::is_factory_task(&tx, &task.id)?
        {
            return Err(CoreError::new(
                "AUTH_REQUIRED",
                "Factory descendants require their private Factory dispatch context",
            ));
        }
        let canonical_root = if let Some(context) = factory.as_ref() {
            context.root_task_id().to_owned()
        } else {
            factory_goal_root.unwrap_or_else(|| task.id.clone())
        };
        if task.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task changed before session creation",
            ));
        }
        if task.state != TaskState::Draft
            && !((goal.is_some() || factory.is_some())
                && matches!(task.state, TaskState::Running | TaskState::OutcomeUnknown))
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Only a draft task or current privately bound Goal node can start a managed session",
            ));
        }
        let validate = if goal.is_some() || factory.is_some() {
            validate_grant_live_from_with_data
        } else {
            validate_grant_from_with_data
        };
        let grant = validate(
            &tx,
            grant_id,
            &task.project_id,
            workspace_id,
            connection_id,
            self.data_input.as_ref(),
        )?;
        let connection = connection_from(&tx, connection_id)?;
        let digest = BinaryDigest::parse(&connection.executable_sha256)
            .map_err(|_| invalid("Connection digest is invalid"))?;
        verify_executable(&connection.executable, &digest).map_err(|_| {
            CoreError::new(
                "INPUT_CHANGED",
                "Selected executable changed; register and approve it again",
            )
        })?;
        #[cfg(debug_assertions)]
        if let Some(clock) = factory_phase_clock.as_mut() {
            crate::ipc::goal_diagnostics::emit(
                "factory_admission_phase",
                Some(clock.elapsed()),
                [false; 6],
                5,
                None,
            );
            *clock = Instant::now();
        }
        let busy:i64=tx.query_row("SELECT count(*) FROM sessions WHERE workspace_id=?1 AND state NOT IN('stopped','failed')",[workspace_id],|r|r.get(0)).map_err(db_error)?;
        let lease: Option<(String, i64)> = tx
            .query_row(
                "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
                [workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if busy != 0 || lease.as_ref().is_some_and(|(holder, _)| !holder.is_empty()) {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Workspace has an unreconciled writer; lease expiry is not proof of stop",
            ));
        }
        crate::policy::validate_ancestor_grant_budgets_from(&tx, grant_id)?;
        let total: i64 = tx
            .query_row(
                "SELECT count(*) FROM sessions WHERE root_grant_id=?1",
                [&grant.root_grant_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let active:i64=tx.query_row("SELECT count(*) FROM sessions WHERE root_grant_id=?1 AND state NOT IN('stopped','failed')",[&grant.root_grant_id],|r|r.get(0)).map_err(db_error)?;
        let root = crate::policy::grant_from_for_supervisor(&tx, &grant.root_grant_id)?;
        if total >= i64::from(root.scope.max_attempts)
            || active >= i64::from(root.scope.max_concurrency)
        {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Shared root grant attempt or concurrency limit reached",
            ));
        }
        let own_total: i64 = tx
            .query_row(
                "SELECT count(*) FROM sessions WHERE grant_id=?1",
                [grant_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let own_active:i64=tx.query_row("SELECT count(*) FROM sessions WHERE grant_id=?1 AND state NOT IN('stopped','failed')",[grant_id],|r|r.get(0)).map_err(db_error)?;
        if own_total >= i64::from(grant.scope.max_attempts)
            || own_active >= i64::from(grant.scope.max_concurrency)
        {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Execution grant attempt or concurrency limit reached",
            ));
        }
        let running: i64 = tx
            .query_row(
                "SELECT count(*) FROM sessions WHERE state NOT IN('stopped','failed')",
                [],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if running >= 4 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Local supervisor concurrency limit reached",
            ));
        }
        if let Some(context) = goal.as_ref() {
            crate::goal_store::preflight_factory_goal_reservation(
                &tx,
                context,
                self.data_input.as_ref(),
            )?;
        }
        for scope in [
            format!("project:{}", task.project_id),
            format!("root:{}", canonical_root),
        ] {
            tx.execute(
                "INSERT OR IGNORE INTO budgets(scope,total_limit,parallel_limit) VALUES(?1,?2,?3)",
                params![scope, root.scope.max_attempts, root.scope.max_concurrency],
            )
            .map_err(db_error)?;
            if tx.execute("UPDATE budgets SET consumed=consumed+1,active=active+1 WHERE scope=?1 AND consumed<total_limit AND active<parallel_limit",[scope]).map_err(db_error)?!=1{return Err(CoreError::new("RESOURCE_LIMIT","Shared project or task budget exhausted"));}
        }
        let workspace_raw: String = tx
            .query_row(
                "SELECT document FROM managed_workspaces WHERE id=?1",
                [workspace_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let workspace: ManagedWorkspace = decode(&workspace_raw)?;
        let connection_binding = &grant.connection_bindings[connection_id];
        let binding = ExecutionBinding {
            connection_id: connection_id.into(),
            environment_id: connection.environment_id.clone(),
            identity_ref: connection.identity_ref.clone(),
            host_id: connection.host_id.clone(),
            workspace_id: workspace_id.into(),
            config_revision: connection.config_revision.clone(),
            input_revision: workspace.base_revision.clone(),
            capability_snapshot_id: connection_binding.capability_snapshot_id.clone(),
            policy_revision: format!("{}:{}", grant.id, grant.revision),
        };
        let intent = DispatchIntent {
            id: Uuid::new_v4().to_string(),
            attempt_id: Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            root_task_id: canonical_root.clone(),
            state: IntentState::Prepared,
            executor_instance: None,
            binding,
        };
        tx.execute(
            "INSERT INTO attempts VALUES(?1,?2,?3,?4,?5)",
            params![
                intent.attempt_id,
                task.id,
                canonical_root,
                encode(&intent.binding)?,
                format!("session:{key}")
            ],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO dispatch_intents VALUES(?1,?2,'prepared',NULL,?3)",
            params![intent.id, intent.attempt_id, encode(&intent)?],
        )
        .map_err(db_error)?;
        let id = Uuid::new_v4().to_string();
        let timestamp = now()?;
        let fencing_token = lease
            .map_or(0, |(_, token)| token)
            .checked_add(1)
            .ok_or_else(|| CoreError::new("RESOURCE_LIMIT", "Lease generation exhausted"))?;
        let expiry = ms()?
            .checked_add(grant.scope.max_duration_ms.min(3_600_000) as i64)
            .ok_or_else(|| invalid("Duration overflow"))?;
        tx.execute("INSERT INTO workspace_leases VALUES(?1,?2,?3,?4) ON CONFLICT(resource) DO UPDATE SET holder=excluded.holder,fencing_token=excluded.fencing_token,expires_at=excluded.expires_at",params![workspace_id,id,fencing_token,expiry]).map_err(db_error)?;
        let session = ManagedSession {
            id,
            task_id: task.id.clone(),
            project_id: task.project_id.clone(),
            workspace_id: workspace_id.into(),
            connection_id: connection_id.into(),
            grant_id: grant_id.into(),
            state: "starting".into(),
            revision: 1,
            native_session_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp.clone(),
            last_error: None,
            turn_state: None,
            attempt_id: intent.attempt_id.clone(),
            intent_id: intent.id.clone(),
            fencing_token: fencing_token as u64,
            runtime_snapshot: None,
        };
        tx.execute(
            "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                session.id,
                task.id,
                task.project_id,
                workspace_id,
                connection_id,
                grant_id,
                grant.root_grant_id,
                session.state,
                encode(&session)?,
                key,
                payload
            ],
        )
        .map_err(db_error)?;
        if let Some(context) = goal.as_ref() {
            crate::goal_store::reserve_session(
                &tx,
                context,
                &session,
                &intent.binding,
                self.data_input.as_ref(),
                self.graph_input.as_ref(),
            )?;
            #[cfg(test)]
            goal_reservation_fault_fixture::apply(&tx, &mut task);
            if task.state == TaskState::OutcomeUnknown
                && crate::goal_store::reserved_factory_goal_task_continuation(
                    &tx,
                    context,
                    &session,
                    self.data_input.as_ref(),
                )?
            {
                // The old generic unknown projection represented an actually
                // joined, normally completed Factory handoff. Only this fully
                // verified new derived reservation continues the Task; this is
                // neither a worker-start receipt nor a general unknown retry.
                task.state = TaskState::Running;
            }
        }
        if let Some(context) = factory.as_ref() {
            #[cfg(debug_assertions)]
            if let Some(clock) = factory_phase_clock.as_mut() {
                crate::ipc::goal_diagnostics::emit(
                    "factory_admission_phase",
                    Some(clock.elapsed()),
                    [false; 6],
                    6,
                    None,
                );
                *clock = Instant::now();
            }
            crate::factory::reserve_factory_session(
                &tx,
                context,
                &session,
                &intent.binding,
                self.data_input.as_ref(),
            )?;
            #[cfg(debug_assertions)]
            if let Some(clock) = factory_phase_clock.as_mut() {
                crate::ipc::goal_diagnostics::emit(
                    "factory_admission_phase",
                    Some(clock.elapsed()),
                    [false; 6],
                    7,
                    None,
                );
                *clock = Instant::now();
            }
        } else if goal.is_none() {
            Store::reserve_ordinary_shared_budget(&tx, &session, &grant, &canonical_root)?;
        }
        task.revision += 1;
        task.updated_at = timestamp;
        tx.execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![encode(&task)?, task.id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &session.id,
            session.revision,
            "session.launch_reserved",
            json!({"task_id":task.id,"attempt_id":intent.attempt_id,"intent_id":intent.id,"binding":intent.binding,"fencing_token":session.fencing_token,"identity_status":"unknown","history_capture":{"normalized_updates":true,"raw_protocol":false,"stderr":false,"completeness":"partial"}}),
            "user_action",
        )?;
        if let Some(context) = goal.as_ref() {
            crate::goal_store::seal_factory_goal_read(
                &tx,
                context,
                &session.attempt_id,
                &session.intent_id,
                Some(&session),
                self.data_input.as_ref(),
            )?;
        }
        tx.commit().map_err(db_error)?;
        #[cfg(debug_assertions)]
        if let Some(clock) = factory_phase_clock {
            crate::ipc::goal_diagnostics::emit(
                "factory_admission_phase",
                Some(clock.elapsed()),
                [false; 6],
                8,
                None,
            );
        }
        Ok(session)
    }
    pub fn send_session(
        &mut self,
        id: &str,
        text: &str,
        revision: u64,
        key: &str,
    ) -> Result<(ManagedSession, bool)> {
        self.send_session_inner(id, text, revision, key, false, false)
    }
    pub(crate) fn send_goal_session(
        &mut self,
        id: &str,
        text: &str,
        revision: u64,
        key: &str,
    ) -> Result<(ManagedSession, bool)> {
        self.send_session_inner(id, text, revision, key, true, false)
    }
    pub(crate) fn send_factory_session(
        &mut self,
        id: &str,
        text: &str,
        revision: u64,
        key: &str,
    ) -> Result<(ManagedSession, bool)> {
        self.send_session_inner(id, text, revision, key, false, true)
    }
    fn send_session_inner(
        &mut self,
        id: &str,
        text: &str,
        revision: u64,
        key: &str,
        goal: bool,
        factory: bool,
    ) -> Result<(ManagedSession, bool)> {
        key_valid(key)?;
        if crate::goal_store::owns_session(&self.conn, id)? != goal
            || crate::factory::owns_session(&self.conn, id)? != factory
        {
            return Err(CoreError::new(
                "AUTH_REQUIRED",
                "Goal-owned turns require their private fixed-node dispatcher",
            ));
        }
        if text.trim().is_empty()
            || text.len()
                > if factory {
                    agentbaton_providers::MAX_FRAME_BYTES / 8
                } else {
                    65536
                }
        {
            return Err(invalid("Message must be nonempty and at most 64 KiB"));
        }
        let payload = encode(&json!({"text":text,"expected_revision":revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old:Option<(String,String)>=tx.query_row("SELECT payload,document FROM session_turns WHERE session_id=?1 AND idempotency_key=?2",params![id,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((old, _)) = old {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Turn key already used with another message",
                ));
            }
            return Ok((session_from(&tx, id)?, false));
        }
        let mut session = session_from(&tx, id)?;
        if session.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Session changed; refresh before sending",
            ));
        }
        if session.state != "ready" {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Session is not ready; unknown outcomes are never replayed",
            ));
        }
        validate_grant_live_from_with_data(
            &tx,
            &session.grant_id,
            &session.project_id,
            &session.workspace_id,
            &session.connection_id,
            self.data_input.as_ref(),
        )?;
        let lease: Option<(String, i64)> = tx
            .query_row(
                "SELECT holder,fencing_token FROM workspace_leases WHERE resource=?1",
                [&session.workspace_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if lease != Some((session.id.clone(), session.fencing_token as i64)) {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Session writer lease no longer matches",
            ));
        }
        let turn_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO session_turns VALUES(?1,?2,'pending',?3,?4,?5)",
            params![
                turn_id,
                id,
                encode(&json!({"turn_id":turn_id,"text":text}))?,
                key,
                payload
            ],
        )
        .map_err(db_error)?;
        session.state = "running".into();
        session.turn_state = Some("running".into());
        save_session(
            &tx,
            &mut session,
            "session.message_queued",
            json!({"turn_id":turn_id,"text":text}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok((session, true))
    }
    pub fn cancel_session(
        &mut self,
        id: &str,
        revision: u64,
        key: &str,
        owned: bool,
    ) -> Result<ManagedSession> {
        key_valid(key)?;
        let scope = format!("local:{id}:sessions.cancel");
        let payload = encode(&json!({"expected_revision":revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
                params![scope, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((old, _)) = old {
            if old != payload {
                return Err(CoreError::new("CONFIG_CONFLICT", "Cancel key was reused"));
            }
            return session_from(&tx, id);
        }
        let mut s = session_from(&tx, id)?;
        if revision == 0 || revision > s.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Cancel revision is ahead of this immutable session",
            ));
        }
        if !owned && s.state != "stopped" {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "No owned process control exists; reconcile before claiming cancellation",
            ));
        }
        if s.state != "stopped" {
            s.state = "cancel_requested".into();
            save_session(
                &tx,
                &mut s,
                "session.cancel_requested",
                json!({}),
                "user_action",
            )?;
        }
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key, payload, encode(&s)?],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(s)
    }
    pub(crate) fn record_turn_queue_failure(&mut self, id: &str) -> Result<()> {
        self.session_failure(id, "turn_queue_failure_requires_reconciliation")
    }
    pub(crate) fn recover_sessions(&mut self) -> Result<()> {
        let ids: Vec<String> = {
            let mut stmt=self.conn.prepare("SELECT id FROM sessions WHERE state NOT IN('stopped','failed','outcome_unknown')").map_err(db_error)?;
            let rows = stmt.query_map([], |r| r.get(0)).map_err(db_error)?;
            rows.collect::<std::result::Result<Vec<_>, _>>()
                .map_err(db_error)?
        };
        for id in ids {
            self.session_failure(&id, "daemon_restart_requires_reconciliation")?;
        }
        let pending: Vec<ProviderConnection> = {
            let mut stmt=self.conn.prepare("SELECT document FROM connections WHERE json_extract(document,'$.probe_state')='pending'").map_err(db_error)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(db_error)?;
            rows.map(|r| decode(&r.map_err(db_error)?))
                .collect::<Result<Vec<_>>>()?
        };
        for c in pending {
            self.finish_probe(&c.id, Err("probe_interrupted".into()))?;
        }
        Ok(())
    }
    fn finish_probe(
        &mut self,
        id: &str,
        result: std::result::Result<VersionEvidence, String>,
    ) -> Result<()> {
        let mut c = self.get_connection(id)?;
        c.revision += 1;
        c.probe_state = if result
            .as_ref()
            .is_ok_and(|r| r.state == VersionProbeState::Succeeded)
        {
            "ready"
        } else {
            "failed"
        }
        .into();
        c.version_evidence = result.ok();
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "UPDATE connections SET document=?1 WHERE id=?2",
            params![encode(&c)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            c.revision,
            "connection.probe_finished",
            json!({"probe_state":c.probe_state,"version_evidence":c.version_evidence,"identity_status":"unknown"}),
            "provider_runtime",
        )?;
        tx.commit().map_err(db_error)
    }
    fn session_failure(&mut self, id: &str, reason: &str) -> Result<()> {
        let mut s = self.get_session(id)?;
        if s.state == "stopped" {
            let tx = self.conn.transaction().map_err(db_error)?;
            save_session(
                &tx,
                &mut s,
                "session.late_failure",
                json!({"reason":reason,"received_after_stop":true}),
                "baton_supervisor",
            )?;
            return tx.commit().map_err(db_error);
        }
        if s.state != "cancel_requested" {
            s.state = "outcome_unknown".into();
        }
        s.last_error = Some(reason.into());
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute("UPDATE session_turns SET state='outcome_unknown' WHERE session_id=?1 AND state='pending'",[id]).map_err(db_error)?;
        save_session(
            &tx,
            &mut s,
            "session.outcome_unknown",
            json!({"reason":reason,"automatic_replay":false}),
            "baton_supervisor",
        )?;
        tx.commit().map_err(db_error)
    }
}

enum Work {
    Prompt(String),
}
enum Outcome {
    Ready(String, String),
    Update(String, Value),
    Turn(String, String),
    Failure(String, String),
}
struct WorkerReceipt {
    capture: Option<crate::factory::FactoryCapture>,
    elapsed_ms: u64,
    reason: String,
    process_reaped: bool,
    never_started: bool,
}
struct Worker {
    commands: SyncSender<Work>,
    cancel: Arc<AtomicBool>,
    thread: JoinHandle<WorkerReceipt>,
}
pub struct Supervisor {
    workers: HashMap<String, Worker>,
    probes: HashMap<String, JoinHandle<std::result::Result<VersionEvidence, String>>>,
    events: Receiver<Outcome>,
    sender: SyncSender<Outcome>,
    runtime: PathBuf,
    launch_clocks: HashMap<String, Instant>,
    // Process-local service hint, never permission or a stop/join receipt.
    goal_stop_hint: bool,
}
impl Supervisor {
    pub fn new(runtime: &Path) -> Self {
        let (sender, events) = mpsc::sync_channel(64);
        Self {
            workers: HashMap::new(),
            probes: HashMap::new(),
            events,
            sender,
            runtime: runtime.into(),
            launch_clocks: HashMap::new(),
            goal_stop_hint: false,
        }
    }
    pub(crate) fn take_goal_stop_hint(&mut self) -> bool {
        std::mem::take(&mut self.goal_stop_hint)
    }
    fn environment(&self, c: &ProviderConnection) -> Result<BTreeMap<OsString, OsString>> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        let base = self.runtime.join("environments");
        for path in [&base, &base.join(&c.environment_id)] {
            if !path.exists() {
                let mut b = std::fs::DirBuilder::new();
                b.mode(0o700).create(path).map_err(|_| {
                    CoreError::new(
                        "PERMISSION_DENIED",
                        "Cannot create private runtime environment",
                    )
                })?;
            }
            let m = std::fs::symlink_metadata(path)
                .map_err(|_| invalid("Environment directory inaccessible"))?;
            if !m.is_dir()
                || m.file_type().is_symlink()
                || m.permissions().mode() & 0o077 != 0
                || m.uid() != rustix::process::geteuid().as_raw()
            {
                return Err(CoreError::new("PERMISSION_DENIED","Runtime environment must be private and owned; no existing permissions changed"));
            }
        }
        let home = base.join(&c.environment_id);
        Ok(BTreeMap::from([
            (OsString::from("HOME"), home.as_os_str().to_owned()),
            (
                OsString::from("XDG_CONFIG_HOME"),
                home.join("config").into_os_string(),
            ),
            (
                OsString::from("XDG_STATE_HOME"),
                home.join("state").into_os_string(),
            ),
        ]))
    }
    fn configuration_root(&self) -> Result<RuntimeRoot> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        let directory = self.runtime.join("runtime-config");
        match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => {
                return Err(CoreError::new(
                    "RESOURCE_LIMIT",
                    "Cannot create private configuration staging directory",
                ))
            }
        }
        let meta = std::fs::symlink_metadata(&directory)
            .map_err(|_| invalid("Configuration namespace is inaccessible"))?;
        if !meta.is_dir()
            || meta.file_type().is_symlink()
            || meta.permissions().mode() & 0o077 != 0
            || meta.uid() != rustix::process::geteuid().as_raw()
        {
            return Err(CoreError::new("PERMISSION_DENIED", "Configuration namespace must be private and owned; existing permissions are not changed"));
        }
        RuntimeRoot::open(&directory).map_err(crate::runtime_config::config_error)
    }
    pub fn probe(&mut self, store: &Store, id: &str) -> Result<()> {
        let c = store.get_connection(id)?;
        if c.probe_state != "pending" || self.probes.contains_key(id) {
            return Ok(());
        }
        if self.probes.len() >= 4 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Version probe concurrency limit reached",
            ));
        }
        let environment = self.environment(&c)?;
        let id = c.id.clone();
        let request = VersionProbeRequest {
            executable: c.executable,
            expected_sha256: BinaryDigest::parse(&c.executable_sha256)
                .map_err(|_| invalid("Invalid connection digest"))?,
            working_directory: self.runtime.clone(),
            approval_reference: c.approval_id,
            environment,
            timeout: Duration::from_secs(3),
        };
        let thread = std::thread::Builder::new()
            .name("baton-version-probe".into())
            .spawn(move || probe_version(request).map_err(|_| "version_probe_failed".to_owned()))
            .map_err(|_| {
                CoreError::new(
                    "RESOURCE_LIMIT",
                    "Cannot start bounded version probe worker",
                )
            })?;
        self.probes.insert(id, thread);
        Ok(())
    }
    pub fn launch(&mut self, store: &mut Store, id: &str) -> Result<()> {
        if self.workers.contains_key(id) {
            return Ok(());
        }
        self.launch_clocks
            .entry(id.to_owned())
            .or_insert_with(Instant::now);
        let s = store.get_session(id)?;
        if s.state != "starting" {
            return Ok(());
        }
        let c = store.get_connection(&s.connection_id)?;
        let plan = crate::runtime_config::approved_plan(&c)?;
        let goal_duration = store
            .goal_session_duration(id)?
            .or(store.shared_session_duration(id)?);
        let grant = if goal_duration.is_some() {
            crate::policy::validate_grant_live_from_with_data(
                &store.conn,
                &s.grant_id,
                &s.project_id,
                &s.workspace_id,
                &s.connection_id,
                store.data_input.as_ref(),
            )?
        } else {
            store.validate_grant(
                &s.grant_id,
                &s.project_id,
                &s.workspace_id,
                &s.connection_id,
            )?
        };
        let workspace = store
            .list_workspaces(&s.project_id)?
            .into_iter()
            .find(|w| w.id == s.workspace_id)
            .ok_or_else(|| invalid("Workspace unavailable"))?;
        let root = self.configuration_root()?;
        let snapshot = if let Some(binding) = store.runtime_binding(id)? {
            if binding.attempt_id != s.attempt_id
                || binding.connection_config_revision != c.config_revision
            {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Attempt runtime binding changed",
                ));
            }
            root.verify_before_launch(&binding.snapshot, RuntimeRequirements::default())
                .map_err(crate::runtime_config::config_error)?
        } else {
            let snapshot = root
                .materialize(plan.clone(), Ownership::BatonManaged)
                .map_err(crate::runtime_config::config_error)?;
            let binding = crate::RuntimeBinding {
                session_id: s.id.clone(),
                attempt_id: s.attempt_id.clone(),
                connection_config_revision: c.config_revision.clone(),
                plan_digest: snapshot.plan_digest().into(),
                snapshot: snapshot.descriptor(),
            };
            store.bind_runtime(&binding)?;
            snapshot
        };
        if snapshot.plan_digest()
            != plan
                .plan_digest()
                .map_err(crate::runtime_config::config_error)?
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Materialized runtime differs from the approved plan",
            ));
        }
        // Recheck after the durable binding and immediately before committing
        // dispatch. This detects drift; it does not claim an OS race-free sandbox.
        let snapshot = root
            .verify_before_launch(&snapshot.descriptor(), RuntimeRequirements::default())
            .map_err(crate::runtime_config::config_error)?;
        let configuration = if c.protocol_route == ProtocolRoute::CodexAppServer {
            None
        } else {
            Some(
                snapshot
                    .acp_configuration()
                    .map_err(crate::runtime_config::config_error)?,
            )
        };
        let arguments: Vec<OsString> = match c.protocol_route {
            ProtocolRoute::CustomAcp => c.arguments.iter().map(OsString::from).collect(),
            ProtocolRoute::CodexAppServer => ["app-server", "--listen", "stdio://"]
                .into_iter()
                .map(OsString::from)
                .collect(),
            ProtocolRoute::QoderNativeAcp => vec![
                "--acp".into(),
                "--config-dir".into(),
                snapshot.provider_configuration_root().into_os_string(),
            ],
            ProtocolRoute::ClaudeAgentAcp => vec![],
            ProtocolRoute::CodexAcpAdapter => {
                return Err(CoreError::new(
                    "CAPABILITY_UNSUPPORTED",
                    "This connection does not authorize an alternative Codex adapter route",
                ))
            }
        };
        let environment = snapshot.environment();
        let installation = ApprovedInstallation {
            executable: c.executable,
            expected_sha256: BinaryDigest::parse(&c.executable_sha256)
                .map_err(|_| invalid("Invalid connection digest"))?,
            arguments,
            workspace: workspace.root,
            approval_reference: grant.approval_id,
            environment,
        };
        let mut factory_mcp = store.factory_mcp_for_launch(id, &self.runtime)?;
        // The durable sent marker precedes worker spawn and any provider code.
        store.mark_dispatch_started(&s.intent_id, &s.id)?;
        if let Some(mcp) = factory_mcp.as_mut() {
            mcp.attach_data(store.data_input.take());
        }
        let (commands, receiver) = mpsc::sync_channel(8);
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let sender = self.sender.clone();
        let id = s.id.clone();
        let prelaunch_ms = u64::try_from(
            self.launch_clocks
                .get(&id)
                .ok_or_else(|| invalid("Missing owned launch clock"))?
                .elapsed()
                .as_millis(),
        )
        .unwrap_or(u64::MAX)
        .saturating_add(store.shared_session_queue(&id)?.unwrap_or(0));
        let duration = Duration::from_millis(
            goal_duration
                .unwrap_or(grant.scope.max_duration_ms)
                .min(grant.scope.max_duration_ms)
                .saturating_sub(prelaunch_ms),
        );
        if duration.is_zero() {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Queued/configuration preparation exhausted the original attempt runtime",
            ));
        }
        let thread = std::thread::Builder::new()
            .name("baton-provider".into())
            .spawn(move || {
                run_worker(
                    id,
                    WorkerLaunch {
                        route: c.protocol_route,
                        installation,
                        configuration,
                        factory_mcp,
                        prelaunch_ms,
                        duration,
                    },
                    receiver,
                    worker_cancel,
                    sender,
                )
            })
            .map_err(|_| {
                CoreError::new(
                    "RESOURCE_LIMIT",
                    "Cannot start owned provider worker; reconcile intent",
                )
            })?;
        self.launch_clocks.remove(&s.id);
        self.workers.insert(
            s.id,
            Worker {
                commands,
                cancel,
                thread,
            },
        );
        Ok(())
    }
    pub fn send(&mut self, id: &str, text: &str) -> Result<()> {
        self.workers
            .get(id)
            .ok_or_else(|| {
                CoreError::new(
                    "OUTCOME_UNKNOWN",
                    "No owned worker exists; reconcile before retry",
                )
            })?
            .commands
            .try_send(Work::Prompt(text.into()))
            .map_err(|_| {
                CoreError::new(
                    "RESOURCE_LIMIT",
                    "Worker command queue unavailable; reconcile queued turn",
                )
            })
    }
    /// Only `launch` errors reach this path: no worker was inserted and a
    /// successful thread spawn has no fallible operation before insertion.
    pub(crate) fn record_local_launch_failure(
        &mut self,
        store: &mut Store,
        id: &str,
    ) -> Result<()> {
        if self.workers.contains_key(id) {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "An owned worker exists; local no-start proof is unavailable",
            ));
        }
        let elapsed_ms = u64::try_from(
            self.launch_clocks
                .remove(id)
                .map(|s| s.elapsed().as_millis())
                .unwrap_or(0),
        )
        .unwrap_or(u64::MAX)
        .saturating_add(store.shared_session_queue(id)?.unwrap_or(0));
        self.persist_receipt(
            store,
            id,
            WorkerReceipt {
                capture: None,
                reason: "local_launch_failed_before_worker_start".into(),
                process_reaped: false,
                never_started: true,
                elapsed_ms,
            },
        )
    }
    pub(crate) fn has_owned_io(&self) -> bool {
        !self.workers.is_empty() || !self.probes.is_empty()
    }
    pub fn has_workers(&self) -> bool {
        !self.workers.is_empty()
    }
    pub fn owns(&self, id: &str) -> bool {
        self.workers.contains_key(id)
    }
    pub fn cancel(&self, id: &str) {
        if let Some(w) = self.workers.get(id) {
            w.cancel.store(true, Ordering::Relaxed);
        }
    }
    fn persist_receipt(
        &mut self,
        store: &mut Store,
        id: &str,
        receipt: WorkerReceipt,
    ) -> Result<()> {
        let mut s = store.get_session(id)?;
        if receipt.process_reaped || receipt.never_started {
            let task = store.get(&s.task_id)?;
            if (receipt.reason == "cancelled" || s.state == "cancel_requested")
                && task.state != TaskState::CancelRequested
                && !crate::goal_store::owns_session(&store.conn, id)?
                && !store.factory_internal_stop_is_current(id)?
            {
                store.cancel(&task.id, task.revision)?;
            }
            let proof = if receipt.never_started
                && store.intent(&s.intent_id)?.state == IntentState::Prepared
            {
                StopEvidence::NeverDispatched
            } else if receipt.never_started {
                StopEvidence::ConfirmedNotStarted {
                    executor_instance: s.id.clone(),
                    receipt_ref: format!("owned-worker-no-start:{id}"),
                }
            } else {
                StopEvidence::ConfirmedStopped {
                    executor_instance: s.id.clone(),
                    receipt_ref: format!("owned-worker-stop:{id}"),
                }
            };
            store.confirm_stop(&s.intent_id, proof)?;
            store.release_workspace_lease(&s.workspace_id, &s.id, s.fencing_token)?;
            s.state = "stopped".into();
        } else {
            s.state = "outcome_unknown".into();
        }
        s.last_error = if receipt.reason == "cancelled" {
            s.last_error.clone()
        } else {
            Some(receipt.reason.clone())
        };
        let tx = store.conn.transaction().map_err(db_error)?;
        tx.execute("UPDATE session_turns SET state='outcome_unknown' WHERE session_id=?1 AND state='pending'",[id]).map_err(db_error)?;
        save_session(
            &tx,
            &mut s,
            "session.stopped",
            json!({"reason":receipt.reason,"process_reaped":receipt.process_reaped,"no_process_started":receipt.never_started,"task_accepted":false}),
            "baton_supervisor",
        )?;
        tx.commit().map_err(db_error)?;
        store.settle_goal_worker(
            id,
            receipt.elapsed_ms,
            receipt.process_reaped || receipt.never_started,
            receipt.never_started,
        )?;
        store.settle_shared_session(
            id,
            receipt.elapsed_ms,
            receipt.process_reaped || receipt.never_started,
            receipt.never_started,
            receipt.capture.as_ref(),
        )?;
        store.settle_factory_worker(
            id,
            receipt.process_reaped,
            receipt.never_started,
            &receipt.reason,
            receipt.capture.as_ref(),
        )
    }
    pub fn shutdown(&mut self, store: &mut Store) -> Result<()> {
        for worker in self.workers.values() {
            worker.cancel.store(true, Ordering::Relaxed);
        }
        while !self.workers.is_empty() || !self.probes.is_empty() {
            self.tick(store)?;
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
    pub fn tick(&mut self, store: &mut Store) -> Result<()> {
        let complete: Vec<String> = self
            .probes
            .iter()
            .filter(|(_, thread)| thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in complete {
            let thread = self.probes.remove(&id).unwrap();
            let result = thread
                .join()
                .unwrap_or_else(|_| Err("version_probe_worker_failed".into()));
            store.finish_probe(&id, result)?;
        }

        for (id, worker) in &self.workers {
            let s = store.get_session(id)?;
            let task = store.get(&s.task_id)?;
            let policy_invalid = crate::policy::checked_lineage(&store.conn, &s.grant_id).is_err()
                || !store.factory_session_policy_current(id)?;
            if policy_invalid && s.state != "cancel_requested" {
                let mut stopped = s.clone();
                stopped.state = "cancel_requested".into();
                stopped.last_error = Some("grant_revoked_or_expired".into());
                let tx = store.conn.transaction().map_err(db_error)?;
                save_session(
                    &tx,
                    &mut stopped,
                    "session.policy_stop_requested",
                    json!({"reason":"grant_revoked_or_expired","stop_confirmed":false}),
                    "baton_policy",
                )?;
                tx.commit().map_err(db_error)?;
            }
            if task.state == TaskState::CancelRequested || policy_invalid {
                worker.cancel.store(true, Ordering::Relaxed);
            }
        }
        for _ in 0..64 {
            match self.events.try_recv() {
                Ok(Outcome::Ready(id, native)) => {
                    let mut s = store.get_session(&id)?;
                    if s.state == "starting" {
                        s.state = "ready".into();
                    }
                    s.native_session_id = Some(native);
                    let tx = store.conn.transaction().map_err(db_error)?;
                    save_session(
                        &tx,
                        &mut s,
                        "session.ready",
                        json!({"identity_status":"unknown","host_tools":false,"history_capture":{"normalized_updates":true,"raw_protocol":false,"stderr":false,"completeness":"partial"}}),
                        "provider_runtime",
                    )?;
                    tx.commit().map_err(db_error)?;
                }
                Ok(Outcome::Update(id, update)) => {
                    let mut s = store.get_session(&id)?;
                    let tx = store.conn.transaction().map_err(db_error)?;
                    save_session(
                        &tx,
                        &mut s,
                        "session.update",
                        json!({"update":update}),
                        "provider_runtime",
                    )?;
                    tx.commit().map_err(db_error)?;
                }
                Ok(Outcome::Turn(id, state)) => {
                    let mut s = store.get_session(&id)?;
                    if s.state == "running" {
                        s.state = "ready".into();
                    }
                    s.turn_state = Some(state.clone());
                    let tx = store.conn.transaction().map_err(db_error)?;
                    tx.execute("UPDATE session_turns SET state='ended' WHERE session_id=?1 AND state='pending'",[&id]).map_err(db_error)?;
                    save_session(
                        &tx,
                        &mut s,
                        "session.turn_ended",
                        json!({"turn_state":state,"task_accepted":false}),
                        "provider_runtime",
                    )?;
                    tx.commit().map_err(db_error)?;
                    // Only an owned worker's correlated, committed Goal turn
                    // can request earlier service. The Actor must still run
                    // the complete fresh Goal stop policy before signalling it.
                    if self.workers.contains_key(&id)
                        && matches!(
                            state.as_str(),
                            "completed" | "cancelled" | "interrupted" | "outcome_unknown"
                        )
                        && crate::goal_store::owns_session(&store.conn, &id)?
                    {
                        self.goal_stop_hint = true;
                    }
                }
                Ok(Outcome::Failure(id, reason)) => {
                    store.session_failure(&id, &reason)?;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        let done: Vec<String> = self
            .workers
            .iter()
            .filter(|(_, w)| w.thread.is_finished())
            .map(|(id, _)| id.clone())
            .collect();
        for id in done {
            let w = self.workers.remove(&id).unwrap();
            match w.thread.join() {
                Ok(receipt) => self.persist_receipt(store, &id, receipt)?,
                Err(_) => store.session_failure(&id, "worker_panic_requires_reconciliation")?,
            }
        }
        Ok(())
    }
}
impl Supervisor {
    /// No database mutation: after a fatal actor error, stop/join owned IO before
    /// releasing daemon authority; persisted ambiguous outcomes remain fenced.
    pub(crate) fn reap_owned_workers(&mut self) {
        for w in self.workers.values() {
            w.cancel.store(true, Ordering::Relaxed);
        }
        for (_, w) in self.workers.drain() {
            let _ = w.thread.join();
        }
        for (_, probe) in self.probes.drain() {
            let _ = probe.join();
        }
    }
    #[cfg(test)]
    pub(crate) fn install_drop_fixture(&mut self, check: impl FnOnce() + Send + 'static) {
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (commands, _receiver) = mpsc::sync_channel(1);
        let thread = std::thread::spawn(move || {
            while !worker_cancel.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(1));
            }
            check();
            WorkerReceipt {
                capture: None,
                reason: "generated_drop_fixture".into(),
                process_reaped: true,
                never_started: true,
                elapsed_ms: 0,
            }
        });
        self.workers.insert(
            "generated-drop-fixture".into(),
            Worker {
                commands,
                cancel,
                thread,
            },
        );
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.reap_owned_workers();
    }
}

struct WorkerLaunch {
    route: ProtocolRoute,
    installation: ApprovedInstallation,
    configuration: Option<Value>,
    factory_mcp: Option<crate::factory::FactoryMcpLaunch>,
    prelaunch_ms: u64,
    duration: Duration,
}
fn run_worker(
    id: String,
    mut launch: WorkerLaunch,
    commands: Receiver<Work>,
    cancel: Arc<AtomicBool>,
    events: SyncSender<Outcome>,
) -> WorkerReceipt {
    let started = Instant::now();
    let prior = launch.prelaunch_ms;
    let mut receipt = (|| {
        if launch
            .factory_mcp
            .as_ref()
            .is_some_and(|m| m.check_data_boundary().is_err())
        {
            return WorkerReceipt {
                capture: None,
                reason: "data_observation_expired_before_worker".into(),
                process_reaped: false,
                never_started: true,
                elapsed_ms: 0,
            };
        }
        let data = launch.factory_mcp.as_mut().and_then(|m| m.take_data());
        let execution_deadline = started + launch.duration;
        let mut capture_plan = launch
            .factory_mcp
            .as_mut()
            .and_then(|m| m.take_capture_plan());
        let mut receipt = run_worker_inner(id, launch, commands, cancel, events, data.as_ref());
        if let (Some(plan), Some(data)) = (capture_plan.as_mut(), data) {
            plan.attach_observation_contexts(data.into_observation_contexts());
        }
        if receipt.process_reaped && !receipt.never_started {
            receipt.capture = capture_plan.and_then(|p| p.capture(execution_deadline).ok());
        }
        receipt
    })();
    receipt.elapsed_ms = u64::try_from(started.elapsed().as_millis())
        .unwrap_or(u64::MAX)
        .saturating_add(prior);
    receipt
}
fn run_worker_inner(
    id: String,
    launch: WorkerLaunch,
    commands: Receiver<Work>,
    cancel: Arc<AtomicBool>,
    events: SyncSender<Outcome>,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> WorkerReceipt {
    let WorkerLaunch {
        route,
        installation,
        configuration,
        factory_mcp,
        prelaunch_ms: _,
        duration,
    } = launch;
    let started = Instant::now();
    let notify = |outcome| events.try_send(outcome).is_ok();
    if cancel.load(Ordering::Relaxed) {
        return WorkerReceipt {
            capture: None,
            reason: "cancelled".into(),
            process_reaped: false,
            never_started: true,
            elapsed_ms: 0,
        };
    }
    let factory_mcp = match factory_mcp.map(|permit| permit.authorize()).transpose() {
        Ok(mcp) => mcp,
        Err(_) => {
            return WorkerReceipt {
                capture: None,
                reason: "factory_launch_binding_changed".into(),
                process_reaped: false,
                never_started: true,
                elapsed_ms: 0,
            }
        }
    };
    if cancel.load(Ordering::Relaxed) || started.elapsed() >= duration {
        return WorkerReceipt {
            capture: None,
            reason: "factory_preflight_cancelled_or_deadline".into(),
            process_reaped: false,
            never_started: true,
            elapsed_ms: 0,
        };
    }
    if data.is_some_and(|d| d.check_worker_boundary().is_err()) {
        return WorkerReceipt {
            capture: None,
            reason: "data_changed_or_request_deadline_before_spawn".into(),
            process_reaped: false,
            never_started: true,
            elapsed_ms: 0,
        };
    }
    let mut process = match ManagedProviderProcess::start(
        route,
        installation,
        duration.min(Duration::from_secs(3)),
    ) {
        Ok(p) => p,
        Err(failure) => {
            notify(Outcome::Failure(
                id.clone(),
                "provider_initialization_failed".into(),
            ));
            return WorkerReceipt {
                capture: None,
                reason: "initialization_failed".into(),
                process_reaped: false,
                never_started: failure.side_effect_state
                    == agentbaton_providers::transport::SideEffectState::None,
                elapsed_ms: 0,
            };
        }
    };
    if cancel.load(Ordering::Relaxed) || started.elapsed() >= duration {
        process.terminate();
        return WorkerReceipt {
            capture: None,
            reason: "launch_cancelled_or_timeout".into(),
            process_reaped: process.diagnostics().process_reaped,
            never_started: false,
            elapsed_ms: 0,
        };
    }
    let _ = process.set_io_timeout(
        duration
            .saturating_sub(started.elapsed())
            .min(Duration::from_secs(3)),
    );
    if let Some(configuration) = configuration {
        let configured = if let Some(mcp) = factory_mcp {
            process.configure_factory_session_configuration(&configuration, &mcp)
        } else {
            process.configure_session_configuration(&configuration)
        };
        if configured.is_err() {
            process.terminate();
            return WorkerReceipt {
                capture: None,
                reason: "provider_configuration_unsupported".into(),
                process_reaped: process.diagnostics().process_reaped,
                never_started: false,
                elapsed_ms: 0,
            };
        }
    }
    let native = match process.new_session(&id) {
        Ok(n) => n,
        Err(_) => {
            process.terminate();
            notify(Outcome::Failure(
                id.clone(),
                "provider_session_creation_failed".into(),
            ));
            return WorkerReceipt {
                capture: None,
                reason: "session_creation_failed".into(),
                process_reaped: process.diagnostics().process_reaped,
                never_started: false,
                elapsed_ms: 0,
            };
        }
    };
    if !notify(Outcome::Ready(id.clone(), native.clone())) {
        process.terminate();
        return WorkerReceipt {
            capture: None,
            reason: "event_backpressure".into(),
            process_reaped: process.diagnostics().process_reaped,
            never_started: false,
            elapsed_ms: 0,
        };
    }
    let mut pending = false;
    let mut dropped = 0;
    let reason = loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = process.cancel(&native);
            break "cancelled";
        }
        if started.elapsed() >= duration {
            break "duration_limit";
        }
        match commands.try_recv() {
            Ok(Work::Prompt(text)) => {
                if pending {
                    break "duplicate_worker_turn";
                }
                let remaining = duration.saturating_sub(started.elapsed());
                let _ = process.set_io_timeout(remaining.min(Duration::from_secs(60)));
                if process.begin_prompt(&native, &text).is_err() {
                    break "provider_prompt_failed";
                }
                pending = true;
            }
            Err(TryRecvError::Disconnected) => break "supervisor_disconnected",
            Err(TryRecvError::Empty) => {}
        }
        let mut turn_completed = None;
        if pending {
            match process.poll_prompt(&native, Duration::from_millis(25)) {
                Ok(Some(state)) => {
                    pending = false;
                    turn_completed = Some(state);
                }
                Ok(None) => {}
                Err(error) => {
                    #[cfg(debug_assertions)]
                    crate::ipc::goal_diagnostics::emit(
                        "provider_prompt_failure",
                        Some(started.elapsed()),
                        [
                            error.kind == agentbaton_providers::transport::FailureKind::Timeout,
                            started.elapsed() >= duration,
                            pending,
                            false,
                            false,
                            false,
                        ],
                        crate::ipc::goal_diagnostics::provider_failure_tag(error.kind),
                        None,
                    );
                    // Preserve the original public reason/state and deadline.
                    let _ = error;
                    break "provider_protocol_or_timeout_failure";
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
        let updates = process.take_updates();
        for update in updates {
            if !notify(Outcome::Update(id.clone(), json!(update))) {
                process.terminate();
                notify(Outcome::Failure(id.clone(), "event_backpressure".into()));
                return WorkerReceipt {
                    capture: None,
                    reason: "event_backpressure".into(),
                    process_reaped: process.diagnostics().process_reaped,
                    never_started: false,
                    elapsed_ms: 0,
                };
            }
        }
        // Raw evidence is intentionally not forwarded or persisted. Normalized updates are retained.
        process.take_evidence();
        let diagnostics = process.diagnostics();
        if diagnostics.update_records_dropped > dropped {
            let count = diagnostics.update_records_dropped - dropped;
            dropped = diagnostics.update_records_dropped;
            if !notify(Outcome::Update(
                id.clone(),
                json!({"kind":{"kind":"stream_gap","dropped_records":count}}),
            )) {
                break "event_backpressure";
            }
        }
        if let Some(state) = turn_completed {
            let state = serde_json::to_value(state)
                .ok()
                .and_then(|s| s.as_str().map(str::to_owned))
                .unwrap_or_else(|| "outcome_unknown".into());
            if !notify(Outcome::Turn(id.clone(), state)) {
                break "event_backpressure";
            }
        }
    };
    process.terminate();
    if reason != "cancelled" {
        notify(Outcome::Failure(id.clone(), reason.into()));
    }
    WorkerReceipt {
        capture: None,
        reason: reason.into(),
        process_reaped: process.diagnostics().process_reaped,
        never_started: false,
        elapsed_ms: 0,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalProposal {
    pub id: String,
    pub kind: String,
    pub revision: u64,
    pub snapshot: Value,
    pub created_at: String,
    pub expires_at_ms: i64,
    pub consumed: bool,
}
impl Store {
    pub fn propose_connection(
        &mut self,
        name: &str,
        provider: &str,
        executable: &Path,
        arguments: &[String],
        key: &str,
    ) -> Result<ApprovalProposal> {
        let protocol_route = crate::routes::validate_route_arguments(provider, arguments)?;
        let plan_digest = crate::runtime_config::plan_for_provider(provider)?
            .plan_digest()
            .map_err(crate::runtime_config::config_error)?;
        if name.trim().is_empty()
            || name.len() > 256
            || arguments.len() > 32
            || arguments.iter().any(|s| s.len() > 4096)
        {
            return Err(invalid("Connection selection must be bounded"));
        }
        let digest = executable_digest(executable)
            .map_err(|_| invalid("Select an existing canonical executable"))?;
        let snapshot = json!({"intent":{"name":name,"provider":provider,"executable":executable,"arguments":arguments},"protocol_route":protocol_route,"runtime_plan_digest":plan_digest,"executable_sha256":digest.as_str(),"scope":"version_probe_only","identity_status":"unknown","enforcement":{"filesystem_sandbox":false,"network_sandbox":false,"native_identity_isolation":false},"warning":"Executes selected program with --version only; no session/model invocation approved"});
        self.persist_proposal("connection", snapshot, key)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn propose_grant(
        &mut self,
        project_id: &str,
        workspace_id: &str,
        connection_id: &str,
        max_attempts: u32,
        max_concurrency: u32,
        max_duration_ms: u64,
        key: &str,
    ) -> Result<ApprovalProposal> {
        let intent = json!({"project_id":project_id,"workspace_id":workspace_id,"connection_id":connection_id,"max_attempts":max_attempts,"max_concurrency":max_concurrency,"max_duration_ms":max_duration_ms});
        if let Some(old) = self.data_proposal_replay("grant", &intent, key)? {
            return Ok(old);
        }
        let refs = crate::policy::grant_proposal_refs_with_data(
            &self.conn,
            project_id,
            workspace_id,
            connection_id,
            self.data_input.as_ref(),
        )?;
        let snapshot = json!({"intent":{"project_id":project_id,"workspace_id":workspace_id,"connection_id":connection_id,"max_attempts":max_attempts,"max_concurrency":max_concurrency,"max_duration_ms":max_duration_ms},"refs":refs,"scope":"bounded_local_session_execution","enforcement":{"filesystem_sandbox":false,"network_sandbox":false,"native_identity_isolation":false},"warning":"Selected process may use its native capabilities. Baton-mediated tools deny additional roots/recipients; private environment is not an OS sandbox. Native account identity is unknown."});
        self.persist_proposal("grant", snapshot, key)
    }
    pub(crate) fn persist_proposal(
        &mut self,
        kind: &str,
        snapshot: Value,
        key: &str,
    ) -> Result<ApprovalProposal> {
        key_valid(key)?;
        let scope = format!("local:approvals.propose:{kind}");
        let payload = encode(&snapshot)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
                params![scope, key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        if let Some((old, result)) = old {
            if old != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Proposal key already used with another snapshot",
                ));
            }
            return decode(&result);
        }
        let proposal = ApprovalProposal {
            id: Uuid::new_v4().to_string(),
            kind: kind.into(),
            revision: 1,
            snapshot,
            created_at: now()?,
            expires_at_ms: ms()?
                .checked_add(300_000)
                .ok_or_else(|| invalid("Clock overflow"))?,
            consumed: false,
        };
        tx.execute(
            "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
            params![proposal.id, encode(&proposal)?],
        )
        .map_err(db_error)?;
        tx.execute(
            "INSERT INTO idempotency VALUES(?1,?2,?3,?4)",
            params![scope, key, payload, encode(&proposal)?],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &proposal.id,
            1,
            "approval.proposed",
            json!({"kind":kind,"snapshot":proposal.snapshot}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(proposal)
    }
    /// Only the private authenticated CLI or a Rust host following native owner confirmation
    /// calls this method. It is deliberately absent from renderer Tauri commands.
    pub fn finalize_proposal(&mut self, id: &str, revision: u64) -> Result<Value> {
        let sizes:Option<(i64,i64)>=self.conn.query_row("SELECT octet_length(document),COALESCE(octet_length(result),0) FROM approval_proposals WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if sizes.is_some_and(|(document, result)| {
            !(0..=4_194_304).contains(&document) || !(0..=4_194_304).contains(&result)
        }) {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Stored owner proposal exceeds its raw-byte bound",
            ));
        }
        let old: Option<(String, Option<String>)> = self
            .conn
            .query_row(
                "SELECT document,result FROM approval_proposals WHERE id=?1 AND octet_length(document)<=4194304 AND (result IS NULL OR octet_length(result)<=4194304)",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        let (document, result) = old
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Owner approval proposal does not exist"))?;
        let mut proposal: ApprovalProposal = decode(&document)?;
        if proposal.kind == "factory" {
            let companion: crate::factory::FactoryCompanionBinding =
                serde_json::from_value(proposal.snapshot["bindings"]["companion"].clone())
                    .map_err(|_| {
                        CoreError::new("INPUT_CHANGED", "Factory companion contract is invalid")
                    })?;
            return self.finalize_factory_scope(id, revision, &companion.executable);
        }
        if proposal.kind == "goal_node_acceptance" {
            return self.decide_goal_node(id, revision, true);
        }
        if proposal.kind == "goal_graph_execution" {
            return self.finalize_goal_graph_run(id, revision);
        }
        if proposal.kind == "goal_execution" {
            return self.finalize_goal_run(id, revision);
        }
        if proposal.kind == "data_workspace" {
            return self.finalize_data_workspace_proposal(id, revision);
        }
        if proposal.kind == "goal" {
            return self.finalize_goal_scope_proposal(id, revision);
        }
        if let Some(result) = result {
            return decode(&result);
        }
        if proposal.revision != revision {
            return Err(CoreError::new("STALE_REVISION", "Owner proposal changed"));
        }
        if proposal.expires_at_ms <= ms()? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Owner proposal expired; prepare and review a fresh snapshot",
            ));
        }
        let intent = &proposal.snapshot["intent"];
        let text = |field: &str| -> Result<&str> {
            intent[field]
                .as_str()
                .ok_or_else(|| invalid("Stored proposal intent is invalid"))
        };
        let key = format!("proposal:{}", proposal.id);
        let result = match proposal.kind.as_str() {
            "connection" => {
                let arguments: Vec<String> = serde_json::from_value(intent["arguments"].clone())
                    .map_err(|_| invalid("Stored arguments invalid"))?;
                json!(self.register_connection_exact(
                    text("name")?,
                    text("provider")?,
                    Path::new(text("executable")?),
                    &arguments,
                    &key,
                    Some(ExpectedConnectionSnapshot {
                        executable_sha256: proposal.snapshot["executable_sha256"].as_str().ok_or_else(|| invalid("Stored proposal executable digest is missing"))?,
                        runtime_plan_digest: proposal.snapshot["runtime_plan_digest"].as_str().ok_or_else(|| CoreError::new("INPUT_CHANGED", "Stored proposal predates the current runtime plan; prepare a fresh proposal"))?,
                    })
                )?)
            }
            "grant" => {
                let max_attempts = u32::try_from(
                    intent["max_attempts"]
                        .as_u64()
                        .ok_or_else(|| invalid("Stored attempt limit invalid"))?,
                )
                .map_err(|_| invalid("Attempt limit overflow"))?;
                let max_concurrency = u32::try_from(
                    intent["max_concurrency"]
                        .as_u64()
                        .ok_or_else(|| invalid("Stored concurrency invalid"))?,
                )
                .map_err(|_| invalid("Concurrency overflow"))?;
                let duration = intent["max_duration_ms"]
                    .as_u64()
                    .ok_or_else(|| invalid("Stored duration invalid"))?;
                json!(self.create_grant_checked(
                    text("project_id")?,
                    text("workspace_id")?,
                    text("connection_id")?,
                    max_attempts,
                    max_concurrency,
                    duration,
                    true,
                    &key,
                    Some(&proposal.snapshot["refs"])
                )?)
            }
            "validation" => self.approve_validation_policy(&proposal)?,
            "data_validation" => self.approve_data_validation_policy(&proposal)?,
            "command_validation" => self.approve_command_validation_policy(&proposal)?,
            _ => return Err(invalid("Unknown owner proposal kind")),
        };
        proposal.consumed = true;
        proposal.revision += 1;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3",
            params![encode(&proposal)?, encode(&result)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            proposal.revision,
            "approval.confirmed",
            json!({"kind":proposal.kind,"proposal_id":id,"consumed":true}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_owned_receipt_survives_a_completely_full_event_channel() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let python = std::process::Command::new("python3")
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            .unwrap();
        let exe = root.join("fixture");
        let body="import json,sys\nfor line in sys.stdin:\n r=json.loads(line);m=r['method'];print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'protocolVersion':1,'agentCapabilities':{}} if m=='initialize' else {'sessionId':'fixture'}}),flush=True)\n";
        std::fs::write(
            &exe,
            format!(
                "#!{}\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                body
            ),
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let digest = executable_digest(&exe).unwrap();
        let (sender, _receiver) = mpsc::sync_channel(1);
        sender
            .send(Outcome::Failure("full".into(), "queue".into()))
            .unwrap();
        let (_commands, receiver) = mpsc::sync_channel(1);
        let receipt = run_worker(
            "session".into(),
            WorkerLaunch {
                route: ProtocolRoute::CustomAcp,
                installation: ApprovedInstallation {
                    executable: exe,
                    expected_sha256: digest,
                    arguments: vec![],
                    workspace: root,
                    approval_reference: "fixture-only".into(),
                    environment: BTreeMap::new(),
                },
                configuration: None,
                factory_mcp: None,
                prelaunch_ms: 0,
                duration: Duration::from_secs(2),
            },
            receiver,
            Arc::new(AtomicBool::new(false)),
            sender,
        );
        assert_eq!(receipt.reason, "event_backpressure");
        assert!(
            receipt.process_reaped,
            "Authoritative stop proof must survive log-channel exhaustion"
        );
    }
    #[test]
    fn queued_late_ready_turn_and_failure_cannot_resurrect_a_confirmed_stopped_session() {
        let mut store = Store::open(Path::new(":memory:")).unwrap();
        let task = store.create("p", "fixture domain task", "k").unwrap();
        store.conn.execute_batch("INSERT INTO projects VALUES('p','{}','1','1','/fixture');INSERT INTO managed_workspaces VALUES('w','p','{}','ready','k','{}','/fixture-workspace');INSERT INTO connections VALUES('c','{}');INSERT INTO execution_grants VALUES('g','p',NULL,'{}');INSERT INTO workspace_leases VALUES('w','',1,0);").unwrap();
        let session = ManagedSession {
            id: "s".into(),
            task_id: task.id.clone(),
            project_id: "p".into(),
            workspace_id: "w".into(),
            connection_id: "c".into(),
            grant_id: "g".into(),
            state: "stopped".into(),
            revision: 5,
            native_session_id: None,
            created_at: now().unwrap(),
            updated_at: now().unwrap(),
            last_error: None,
            turn_state: None,
            attempt_id: "a".into(),
            intent_id: "i".into(),
            fencing_token: 1,
            runtime_snapshot: None,
        };
        store
            .conn
            .execute(
                "INSERT INTO sessions VALUES('s',?1,'p','w','c','g','g','stopped',?2,'k','{}')",
                params![task.id, encode(&session).unwrap()],
            )
            .unwrap();
        let mut supervisor = Supervisor::new(Path::new("/unused-fixture"));
        assert!(supervisor
            .sender
            .try_send(Outcome::Ready("s".into(), "native".into()))
            .is_ok());
        assert!(supervisor
            .sender
            .try_send(Outcome::Turn("s".into(), "completed".into()))
            .is_ok());
        assert!(supervisor
            .sender
            .try_send(Outcome::Failure("s".into(), "late_failure".into()))
            .is_ok());
        supervisor.tick(&mut store).unwrap();
        assert_eq!(store.get_session("s").unwrap().state, "stopped");
        assert_eq!(store.workspace_lease("w").unwrap().unwrap().holder, "");
        assert_eq!(store.get(&task.id).unwrap().state, TaskState::Draft);
        assert_eq!(
            store
                .conn
                .query_row("SELECT count(*) FROM budgets", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
