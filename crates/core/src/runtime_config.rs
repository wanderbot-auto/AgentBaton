//! Persist the exact materialized configuration before a Provider can execute.
//! The approved logical plan and per-attempt byte snapshot have different IDs.
use crate::sessions::{ManagedSession, ProviderConnection};
use crate::storage::{db_error, decode, encode, now, Event, Store};
use crate::{CoreError, Result};
use agentbaton_config::runtime::{RuntimePlan, RuntimeProvider, SnapshotDescriptor};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBinding {
    pub session_id: String,
    pub attempt_id: String,
    pub connection_config_revision: String,
    pub plan_digest: String,
    pub snapshot: SnapshotDescriptor,
}

pub(crate) fn plan_for_provider(provider: &str) -> Result<RuntimePlan> {
    let provider = match provider {
        "custom_acp" => RuntimeProvider::CustomAcp,
        "codex" => RuntimeProvider::Codex,
        "qoder" => RuntimeProvider::Qoder,
        "claude_code" => RuntimeProvider::ClaudeAcp,
        _ => {
            return Err(CoreError::new(
                "CAPABILITY_UNSUPPORTED",
                "Unknown configuration adapter",
            ))
        }
    };
    Ok(RuntimePlan::empty(provider))
}

pub(crate) fn config_error(error: agentbaton_config::ConfigError) -> CoreError {
    use agentbaton_config::ErrorCode;
    let code = match error.code {
        ErrorCode::ResourceLimit => "RESOURCE_LIMIT",
        ErrorCode::PermissionDenied => "PERMISSION_DENIED",
        ErrorCode::InvalidPath => "INVALID_REQUEST",
        ErrorCode::Conflict => "INPUT_CHANGED",
        ErrorCode::Io | ErrorCode::OutcomeUnknown => "INTERNAL_ERROR",
    };
    let mut result = CoreError::new(
        code,
        "Managed configuration cannot be verified; no Provider fallback or replay was performed",
    );
    if error.code == ErrorCode::OutcomeUnknown {
        result.side_effect_state = "unknown".into();
    }
    result
}

pub(crate) fn read_plan(record: Option<&RuntimePlan>, provider: &str) -> Result<RuntimePlan> {
    let plan = record
        .ok_or_else(|| {
            CoreError::new(
        "INPUT_CHANGED",
        "Legacy connection has no approved configuration plan; prepare a new connection proposal",
    )
        })?
        .clone();
    plan.validate().map_err(config_error)?;
    if plan.provider() != plan_for_provider(provider)?.provider() {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Provider and approved configuration plan differ",
        ));
    }
    Ok(plan)
}

pub(crate) fn configuration_revision(
    arguments: &[String],
    route: agentbaton_providers::connections::ProtocolRoute,
    plan_digest: &str,
) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(
            encode(&json!({
                "arguments": arguments,
                "protocol_route": route,
                "runtime_plan_digest": plan_digest,
                "host_capabilities": {"fs":false,"terminal":false}
            }))?
            .as_bytes()
        )
    ))
}

pub(crate) fn approved_plan(connection: &ProviderConnection) -> Result<RuntimePlan> {
    let route =
        crate::routes::validate_route_arguments(&connection.provider, &connection.arguments)?;
    let plan = read_plan(connection.runtime_plan.as_ref(), &connection.provider)?;
    let digest = plan.plan_digest().map_err(config_error)?;
    if route != connection.protocol_route
        || configuration_revision(&connection.arguments, route, &digest)?
            != connection.config_revision
    {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Approved runtime route or configuration plan changed",
        ));
    }
    // The current connection proposal explicitly grants the generated default
    // profile. Broader selected Skills/MCP/model profiles need their own scope
    // and exact driver configuration bridge, not an implicit DB-field upgrade.
    if digest
        != plan_for_provider(&connection.provider)?
            .plan_digest()
            .map_err(config_error)?
    {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "This connection route currently requires its explicitly approved default profile",
        ));
    }
    Ok(plan)
}

impl Store {
    pub fn runtime_binding(&self, session_id: &str) -> Result<Option<RuntimeBinding>> {
        let record: Option<String> = self
            .conn
            .query_row(
                "SELECT document FROM session_runtime_snapshots WHERE session_id=?1",
                [session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        record.as_deref().map(decode).transpose()
    }

    pub(crate) fn bind_runtime(&mut self, binding: &RuntimeBinding) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT document FROM session_runtime_snapshots WHERE session_id=?1",
                [&binding.session_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(previous) = previous {
            if decode::<RuntimeBinding>(&previous)? != *binding {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Attempt configuration snapshot is immutable",
                ));
            }
            return Ok(());
        }
        let document: String = tx
            .query_row(
                "SELECT document FROM sessions WHERE id=?1",
                [&binding.session_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let mut session: ManagedSession = decode(&document)?;
        let intent_state: String = tx
            .query_row(
                "SELECT state FROM dispatch_intents WHERE id=?1",
                [&session.intent_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let connection_raw: String = tx
            .query_row(
                "SELECT document FROM connections WHERE id=?1",
                [&session.connection_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let connection: ProviderConnection = decode(&connection_raw)?;
        let plan = approved_plan(&connection)?;
        if session.state != "starting"
            || intent_state != "prepared"
            || session.attempt_id != binding.attempt_id
            || connection.config_revision != binding.connection_config_revision
            || plan.plan_digest().map_err(config_error)? != binding.plan_digest
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Only the exact prepared attempt can bind a runtime snapshot",
            ));
        }
        session.runtime_snapshot = Some(binding.clone());
        session.revision += 1;
        session.updated_at = now()?;
        tx.execute("INSERT INTO session_runtime_snapshots(session_id,attempt_id,config_revision,document) VALUES(?1,?2,?3,?4)", params![binding.session_id,binding.attempt_id,binding.connection_config_revision,encode(binding)?]).map_err(db_error)?;
        tx.execute(
            "UPDATE sessions SET document=?1 WHERE id=?2",
            params![encode(&session)?, session.id],
        )
        .map_err(db_error)?;
        let event = Event {
            sequence: 0,
            event_id: Uuid::new_v4().to_string(),
            aggregate_id: session.id.clone(),
            revision: session.revision,
            kind: "session.runtime_bound".into(),
            occurred_at: session.updated_at,
            payload: json!({"binding":binding,"native_runtime_loading":"unknown","os_enforcement":"not_proven"}),
            source: "baton_supervisor".into(),
        };
        tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
            .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(())
    }
}
