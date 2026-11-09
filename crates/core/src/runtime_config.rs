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

/// Explicit source selection only. No HOME discovery and no native writes.
/// The source file is bounded/read through the existing descriptor-relative reader.
pub(crate) fn selected_claude_plan(path: &str) -> Result<(RuntimePlan, String)> {
    use agentbaton_config::capabilities::{
        native::{NativeFormat, NativeSourceRoot},
        ProviderRevision, SourceDescriptor, SourceKind,
    };
    use agentbaton_config::runtime::SourceProvenance;
    use std::path::Path;
    if path.len() > 4096
        || path.is_empty()
        || path.ends_with('/')
        || path.chars().any(char::is_control)
    {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "Select an explicit canonical configuration file",
        ));
    }
    let file = Path::new(path);
    let parent = file
        .parent()
        .ok_or_else(|| CoreError::new("INVALID_REQUEST", "Invalid selected configuration file"))?;
    let name = file
        .file_name()
        .ok_or_else(|| CoreError::new("INVALID_REQUEST", "Invalid selected configuration file"))?;
    let root = NativeSourceRoot::open_selected(parent).map_err(config_error)?;
    let source = root
        .read_native(
            SourceDescriptor {
                source_id: "selected-claude-restrictions".into(),
                kind: SourceKind::Native,
                locator_ref: "owner-selected-file".into(),
                provider: ProviderRevision {
                    provider_id: "claude-code".into(),
                    version: "static-restrictions-v1".into(),
                },
                mandatory: false,
            },
            Path::new(name),
            NativeFormat::ClaudeSettingsJson,
        )
        .map_err(config_error)?;
    let disabled = source.restrictive_claude_settings().map_err(|_| CoreError::new("CAPABILITY_UNSUPPORTED", "Only nonempty disabledMcpjsonServers with optional false enableAllProjectMcpServers and empty enabledMcpjsonServers can be imported; other settings, hooks, credentials and active capabilities are unsupported"))?;
    if !root.verify_unchanged(&source).map_err(config_error)? {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Selected configuration changed while reading",
        ));
    }
    let digest = source.preview().source_digest;
    let mut plan = RuntimePlan::empty(RuntimeProvider::ClaudeAcp);
    plan.claude.disabled_project_mcp_names = disabled;
    plan.source_refs.push(SourceProvenance {
        source_ref: "claude-settings-restrictions".into(),
        revision: "1".into(),
        sha256: digest.clone(),
    });
    plan.validate().map_err(config_error)?;
    Ok((plan, digest))
}

fn is_restrictive_claude_plan(plan: &RuntimePlan) -> Result<bool> {
    if plan.provider() != RuntimeProvider::ClaudeAcp
        || plan.claude.disabled_project_mcp_names.is_empty()
        || plan.source_refs.len() != 1
    {
        return Ok(false);
    }
    let source = &plan.source_refs[0];
    if source.source_ref != "claude-settings-restrictions"
        || source.revision != "1"
        || source.sha256.len() != 64
        || !source
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Ok(false);
    }
    let mut permitted = RuntimePlan::empty(RuntimeProvider::ClaudeAcp);
    permitted.claude.disabled_project_mcp_names = plan.claude.disabled_project_mcp_names.clone();
    permitted.source_refs = plan.source_refs.clone();
    Ok(permitted.plan_digest().map_err(config_error)?
        == plan.plan_digest().map_err(config_error)?)
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
    // Only the default or explicitly imported restrictive Claude subset is
    // supported. Broader capabilities still require separate scoped authority.
    if digest
        != plan_for_provider(&connection.provider)?
            .plan_digest()
            .map_err(config_error)?
        && !is_restrictive_claude_plan(&plan)?
    {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "This connection route requires an approved default profile or the supported restrictive Claude import",
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

#[cfg(test)]
mod import_tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    fn fixture() -> (
        tempfile::TempDir,
        Store,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::open(&root.join("store.sqlite")).unwrap();
        let exe = root.join("generated-adapter");
        fs::write(&exe, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
        let source = root.join("settings.json");
        fs::write(&source, br#"{"disabledMcpjsonServers":["private-tools"]}"#).unwrap();
        (dir, store, exe, source)
    }
    #[test]
    fn import_is_owner_confirmed_immutable_and_replay_survives_source_drift() {
        let (_dir, mut store, exe, source) = fixture();
        let original = fs::read(&source).unwrap();
        let default_connection = store
            .register_connection("existing", "claude_code", &exe, &[], "existing-default")
            .unwrap();
        let default_before = serde_json::to_value(&default_connection).unwrap();
        let proposal = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import",
            )
            .unwrap();
        assert_eq!(store.list_connections().unwrap().len(), 1);
        assert_eq!(
            proposal.snapshot["configuration_import"]["disabled_mcp_names_before"],
            json!([])
        );
        assert_eq!(
            proposal.snapshot["configuration_import"]["disabled_mcp_names_after"],
            json!(["private-tools"])
        );
        let result = store
            .finalize_proposal(&proposal.id, proposal.revision)
            .unwrap();
        let connection: ProviderConnection = serde_json::from_value(result.clone()).unwrap();
        let plan = approved_plan(&connection).unwrap();
        assert_eq!(
            plan.claude.disabled_project_mcp_names,
            vec!["private-tools"]
        );
        assert_eq!(fs::read(&source).unwrap(), original);
        fs::write(&source, br#"{"disabledMcpjsonServers":["changed"]}"#).unwrap();
        let replay = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import",
            )
            .unwrap();
        assert_eq!(replay.id, proposal.id);
        assert_eq!(
            store
                .finalize_proposal(&replay.id, replay.revision)
                .unwrap(),
            result
        );
        fs::remove_file(&source).unwrap();
        assert_eq!(
            store
                .propose_connection_with_configuration(
                    "imported",
                    "claude_code",
                    &exe,
                    &[],
                    Some(source.to_str().unwrap()),
                    "import"
                )
                .unwrap()
                .id,
            proposal.id
        );
        // Simulate the real registration-committed/proposal-result-not-recorded
        // crash boundary: reconcile the existing connection, never import drift.
        let mut expired = serde_json::to_value(&proposal).unwrap();
        expired["expires_at_ms"] = json!(0);
        store
            .conn
            .execute(
                "UPDATE approval_proposals SET document=?1,result=NULL WHERE id=?2",
                params![serde_json::to_string(&expired).unwrap(), proposal.id],
            )
            .unwrap();
        assert_eq!(
            store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
            result
        );
        assert_eq!(store.list_connections().unwrap().len(), 2);
        assert_eq!(
            serde_json::to_value(store.get_connection(&default_connection.id).unwrap()).unwrap(),
            default_before
        );
    }
    #[test]
    fn first_confirmation_rejects_source_change_and_other_providers() {
        let (_dir, mut store, exe, source) = fixture();
        let proposal = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import",
            )
            .unwrap();
        fs::write(&source, br#"{"disabledMcpjsonServers":["changed"]}"#).unwrap();
        assert_eq!(
            store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert!(store.list_connections().unwrap().is_empty());
        assert_eq!(
            store
                .propose_connection_with_configuration(
                    "wrong",
                    "codex",
                    &exe,
                    &[],
                    Some(source.to_str().unwrap()),
                    "wrong"
                )
                .err()
                .unwrap()
                .code,
            "CAPABILITY_UNSUPPORTED"
        );
        assert_eq!(
            store
                .propose_connection_with_configuration(
                    "imported",
                    "claude_code",
                    &exe,
                    &[],
                    Some(source.to_str().unwrap()),
                    "import"
                )
                .unwrap()
                .id,
            proposal.id
        );
        assert!(store
            .propose_connection_with_configuration(
                "different",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import"
            )
            .is_err());
        let renewed = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "fresh-import",
            )
            .unwrap();
        assert_ne!(renewed.id, proposal.id);
        assert!(store
            .finalize_proposal(&renewed.id, renewed.revision)
            .is_ok());
    }
    #[test]
    fn imported_snapshot_reaches_a_real_generated_acp_child() {
        use agentbaton_config::{runtime::RuntimeRoot, Ownership};
        use agentbaton_providers::{
            driver::ManagedProviderProcess, installation::executable_digest,
            transport::ApprovedInstallation,
        };
        let (dir, mut store, exe, source) = fixture();
        let root = dir.path().canonicalize().unwrap();
        let python = std::process::Command::new("python3")
            .args(["-c", "import sys;print(sys.executable)"])
            .output()
            .unwrap();
        assert!(python.status.success());
        let body = r#"
import json,sys,os
for line in sys.stdin:
 r=json.loads(line)
 if r['method']=='initialize': v={'protocolVersion':1,'agentCapabilities':{}}
 elif r['method']=='session/new':
  assert r['params']['mcpServers']==[]
  options=r['params']['_meta']['claudeCode']['options']
  expected={'enableAllProjectMcpServers':False,'enabledMcpjsonServers':[],'disabledMcpjsonServers':['private-tools']}
  assert options['settings']==expected
  assert options['settingSources']==[] and options['strictMcpConfig'] is True and options['allowDangerouslySkipPermissions'] is False
  native=json.load(open(os.path.join(os.environ['CLAUDE_CONFIG_DIR'],'settings.json')))
  assert native==expected
  open('generated-import-observed.json','w').write(json.dumps(r['params']))
  v={'sessionId':'generated-import-child'}
 else: raise AssertionError('No prompt or model call is expected')
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':v}),flush=True)
"#;
        fs::write(
            &exe,
            format!(
                "#!{}\n{}",
                String::from_utf8(python.stdout).unwrap().trim(),
                body
            ),
        )
        .unwrap();
        let proposal = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import",
            )
            .unwrap();
        let connection: ProviderConnection = serde_json::from_value(
            store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap();
        let managed = root.join("managed");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let snapshot = RuntimeRoot::open(&managed)
            .unwrap()
            .materialize(approved_plan(&connection).unwrap(), Ownership::BatonManaged)
            .unwrap();
        let installation = ApprovedInstallation {
            expected_sha256: executable_digest(&exe).unwrap(),
            executable: exe,
            workspace: root.clone(),
            arguments: vec![],
            approval_reference: connection.approval_id,
            environment: snapshot.environment(),
        };
        let mut child = ManagedProviderProcess::start(
            connection.protocol_route,
            installation,
            std::time::Duration::from_secs(2),
        )
        .unwrap();
        child
            .configure_session_configuration(&snapshot.acp_configuration().unwrap())
            .unwrap();
        assert_eq!(
            child.new_session("generated-import-only").unwrap(),
            "generated-import-child"
        );
        child.terminate();
        assert!(child.diagnostics().process_reaped);
        let observed: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("generated-import-observed.json")).unwrap())
                .unwrap();
        assert_eq!(
            observed["_meta"]["claudeCode"]["options"]["settings"]["disabledMcpjsonServers"],
            json!(["private-tools"])
        );
        assert_eq!(
            fs::read(&source).unwrap(),
            br#"{"disabledMcpjsonServers":["private-tools"]}"#
        );
    }

    #[test]
    fn imported_plan_cannot_smuggle_additional_model_or_active_configuration() {
        let (_dir, mut store, exe, source) = fixture();
        let proposal = store
            .propose_connection_with_configuration(
                "imported",
                "claude_code",
                &exe,
                &[],
                Some(source.to_str().unwrap()),
                "import",
            )
            .unwrap();
        let mut connection: ProviderConnection = serde_json::from_value(
            store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap();
        connection
            .runtime_plan
            .as_mut()
            .unwrap()
            .claude
            .enabled_project_mcp_names
            .push("unapproved".into());
        let digest = connection
            .runtime_plan
            .as_ref()
            .unwrap()
            .plan_digest()
            .unwrap();
        connection.config_revision =
            configuration_revision(&connection.arguments, connection.protocol_route, &digest)
                .unwrap();
        assert!(approved_plan(&connection).is_err());
    }
}
