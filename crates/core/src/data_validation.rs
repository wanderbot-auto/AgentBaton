//! Owner-approved deterministic JSON contracts. Separate from integrity policies,
//! task acceptance and adoption; neither scripts nor Provider calls run here.
use crate::{
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store},
    validation::bounded_documents,
    CoreError, Result,
};
use agentbaton_workspaces::{
    deterministic::{
        validate_json_table, ApprovedJsonTableRule, JsonTableContract, JsonTablePolicyView,
        VALIDATOR_ID,
    },
    validation::{
        ArtifactContractView, ArtifactRequirement, BoundArtifact, ValidationError,
        ValidationPolicyContractView, ValidationSubject, VersionBinding,
    },
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const MAX_RECORD_BYTES: i64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationSelection {
    pub session_id: String,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<ArtifactRequirement>,
    pub table: JsonTableContract,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationPolicy {
    pub id: String,
    pub task_id: String,
    pub project_id: String,
    pub session_id: String,
    pub attempt_id: String,
    pub workspace_id: String,
    pub approval_id: String,
    pub revision: u64,
    pub revoked: bool,
    pub scope: String,
    pub validator_id: String,
    pub file_contract: ValidationPolicyContractView,
    pub table: JsonTableContract,
    pub contract_digest: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationRecord {
    pub id: String,
    pub policy_id: String,
    pub task_id: String,
    pub state: String,
    pub scope: String,
    pub correctness: String,
    pub task_acceptance: String,
    pub integration_authorized: bool,
    /// Serializable view only; never deserialize this into a trusted acceptance.
    pub evidence: Value,
    pub created_at: String,
}
fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Select a bounded JSON data contract from one registered, confirmed-stopped session",
    )
}
fn changed(_: ValidationError) -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Data subject, root or contract changed; review a fresh owner policy",
    )
}
fn selection_error(error: ValidationError) -> CoreError {
    match error {
        ValidationError::InvalidContract => invalid(),
        other => changed(other),
    }
}
fn key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn limited() -> CoreError {
    CoreError::new(
        "RESOURCE_LIMIT",
        "Stored data validation document exceeds its raw-byte bound; history remains local",
    )
}
fn event(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    revision: u64,
    kind: &str,
    payload: Value,
    source: &str,
) -> Result<()> {
    let event = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: source.into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
        .map_err(db_error)?;
    Ok(())
}
/// Only these fixed tables can be selected. Check raw size before decoding even
/// a single legacy/corrupt policy or owner proposal.
fn bounded_document(connection: &rusqlite::Connection, table: &str, id: &str) -> Result<String> {
    let (size_sql, document_sql) = match table {
        "data_validation_policies" => ("SELECT octet_length(document) FROM data_validation_policies WHERE id=?1", "SELECT document FROM data_validation_policies WHERE id=?1 AND octet_length(document)<=?2"),
        "approval_proposals" => ("SELECT octet_length(document) FROM approval_proposals WHERE id=?1", "SELECT document FROM approval_proposals WHERE id=?1 AND octet_length(document)<=?2"),
        _ => return Err(CoreError::new("INTERNAL_ERROR", "Unknown data validation document")),
    };
    let size: Option<i64> = connection
        .query_row(size_sql, [id], |row| row.get(0))
        .optional()
        .map_err(db_error)?;
    let size = size.ok_or_else(|| {
        CoreError::new("NOT_FOUND", "Data policy or owner proposal is unavailable")
    })?;
    if !(0..=MAX_RECORD_BYTES).contains(&size) {
        return Err(limited());
    }
    connection
        .query_row(document_sql, params![id, MAX_RECORD_BYTES], |row| {
            row.get(0)
        })
        .optional()
        .map_err(db_error)?
        .ok_or_else(limited)
}
impl Store {
    fn resolve_data_validation_selection(
        &self,
        selection: &DataValidationSelection,
    ) -> Result<(JsonTablePolicyView, Value)> {
        if selection.inputs.is_empty()
            || selection.inputs.len() > 16
            || selection.outputs.len() != 3
            || selection
                .inputs
                .iter()
                .chain(&selection.outputs)
                .any(|a| a.relative_path.as_os_str().len() > 4096)
        {
            return Err(invalid());
        }
        let (session, workspace, binding) = self.validation_workspace(&selection.session_id)?;
        let task = self.get(&session.task_id)?;
        let bind = |requirements: &[ArtifactRequirement]| -> Result<Vec<BoundArtifact>> {
            requirements
                .iter()
                .cloned()
                .map(|requirement| {
                    BoundArtifact::from_approved_root(&workspace.root, requirement)
                        .map_err(selection_error)
                })
                .collect()
        };
        let subject = ValidationSubject {
            task_id: task.id.clone(),
            attempt_id: session.attempt_id.clone(),
            spec_revision: task.spec_revision,
            input_revision: binding.input_revision.clone(),
            // This immutable revision is server-derived. Population rows are an
            // owner-approved declaration, not proof from parsing input datasets.
            version: VersionBinding::Data {
                dataset_revision: binding.input_revision.clone(),
            },
        };
        let rule = ApprovedJsonTableRule::from_verified_policy(
            "proposal-preview".into(),
            "1".into(),
            subject,
            bind(&selection.inputs)?,
            bind(&selection.outputs)?,
            selection.table.clone(),
        )
        .map_err(selection_error)?;
        let project = self.get_project(&session.project_id)?;
        let refs = json!({"project_id":project.id,"project_name":project.name,"project_revision":project.revision,"task_goal":task.goal,"task_revision":task.revision,"session_revision":session.revision,"attempt_id":session.attempt_id,"workspace_id":workspace.id,"workspace_revision":workspace.revision,"workspace_identity":workspace.identity,"binding":binding});
        Ok((rule.contract_view(), refs))
    }
    pub fn propose_data_validation_policy(
        &mut self,
        selection: &DataValidationSelection,
        idempotency_key: &str,
    ) -> Result<ApprovalProposal> {
        let (contract, refs) = self.resolve_data_validation_selection(selection)?;
        let digest = contract.digest().map_err(selection_error)?;
        self.persist_proposal("data_validation",json!({"intent":selection,"refs":refs,"file_contract":contract.artifact_policy,"table":contract.table,"validator_id":VALIDATOR_ID,"contract_digest":digest,"scope":"data_contract_only","correctness":"approved_schema_ranges_metadata_only","warning":"Approves local JSON schema/ranges/exact metadata and hash-bound artifact checks only. Dataset population, units and timezone are approved declarations, not algorithmic or scientific proof. No scripts, Provider calls, TaskAccepted, Git adoption or production release are authorized."}),idempotency_key)
    }
    pub(crate) fn approve_data_validation_policy(
        &mut self,
        proposal: &ApprovalProposal,
    ) -> Result<Value> {
        let selection: DataValidationSelection =
            serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_| invalid())?;
        let (contract, refs) = self.resolve_data_validation_selection(&selection)?;
        let digest = contract.digest().map_err(selection_error)?;
        if proposal.snapshot["refs"] != refs
            || proposal.snapshot["file_contract"] != json!(contract.artifact_policy)
            || proposal.snapshot["table"] != json!(contract.table)
            || proposal.snapshot["contract_digest"] != json!(digest)
            || proposal.snapshot["validator_id"] != json!(VALIDATOR_ID)
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Data validation selection changed after native owner proposal",
            ));
        }
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM data_validation_policies WHERE approval_id=?1",
                [&proposal.id],
                |r| r.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(id) = existing {
            return Ok(json!(self.get_data_validation_policy(&id)?));
        }
        let session = self.get_session(&selection.session_id)?;
        let policy = DataValidationPolicy {
            id: Uuid::new_v4().to_string(),
            task_id: session.task_id,
            project_id: session.project_id,
            session_id: session.id,
            attempt_id: session.attempt_id,
            workspace_id: session.workspace_id,
            approval_id: proposal.id.clone(),
            revision: 1,
            revoked: false,
            scope: "data_contract_only".into(),
            validator_id: VALIDATOR_ID.into(),
            file_contract: contract.artifact_policy,
            table: contract.table,
            contract_digest: digest,
            created_at: now()?,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "INSERT INTO data_validation_policies VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                policy.id,
                policy.task_id,
                policy.session_id,
                policy.attempt_id,
                policy.workspace_id,
                policy.approval_id,
                encode(&policy)?
            ],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &policy.id,
            1,
            "data_validation.policy_approved",
            json!({"policy":policy,"scope":"data_contract_only"}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(policy))
    }
    pub fn get_data_validation_policy(&self, id: &str) -> Result<DataValidationPolicy> {
        let policy: DataValidationPolicy = decode(&bounded_document(
            &self.conn,
            "data_validation_policies",
            id,
        )?)?;
        if policy.id != id {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Stored data policy identity is inconsistent",
            ));
        }
        Ok(policy)
    }
    pub fn list_data_validation_policies(
        &self,
        task_id: &str,
    ) -> Result<Vec<DataValidationPolicy>> {
        self.get(task_id)?;
        bounded_documents(&self.conn, "data_validation_policies", task_id)
    }
    pub fn list_data_validation_records(&self, task_id: &str) -> Result<Vec<DataValidationRecord>> {
        self.get(task_id)?;
        bounded_documents(&self.conn, "data_validation_runs", task_id)
    }
    pub fn revoke_data_validation_policy(
        &mut self,
        policy_id: &str,
        expected_revision: u64,
    ) -> Result<DataValidationPolicy> {
        let mut policy = self.get_data_validation_policy(policy_id)?;
        if policy.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data validation policy changed",
            ));
        }
        if policy.revoked {
            return Ok(policy);
        }
        policy.revoked = true;
        policy.revision = policy.revision.checked_add(1).ok_or_else(invalid)?;
        let sql_revision = i64::try_from(expected_revision).map_err(|_| invalid())?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let affected=tx.execute("UPDATE data_validation_policies SET document=?1 WHERE id=?2 AND json_extract(document,'$.revision')=?3",params![encode(&policy)?,policy.id,sql_revision]).map_err(db_error)?;
        if affected != 1 {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data validation policy changed",
            ));
        }
        event(
            &tx,
            &policy.id,
            policy.revision,
            "data_validation.policy_revoked",
            json!({"policy_id":policy.id,"existing_evidence_retained":true}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(policy)
    }
    pub fn run_data_validation(
        &mut self,
        policy_id: &str,
        expected_revision: u64,
        idempotency_key: &str,
    ) -> Result<DataValidationRecord> {
        key(idempotency_key)?;
        let payload = encode(&json!({"policy_id":policy_id,"revision":expected_revision}))?;
        let old:Option<(i64,i64)>=self.conn.query_row("SELECT octet_length(payload),octet_length(document) FROM data_validation_runs WHERE policy_id=?1 AND idempotency_key=?2",params![policy_id,idempotency_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((p, d)) = old {
            if !(0..=MAX_RECORD_BYTES).contains(&p) || !(0..=MAX_RECORD_BYTES).contains(&d) {
                return Err(limited());
            }
            let (original,record):(String,String)=self.conn.query_row("SELECT payload,document FROM data_validation_runs WHERE policy_id=?1 AND idempotency_key=?2 AND octet_length(payload)<=?3 AND octet_length(document)<=?3",params![policy_id,idempotency_key,MAX_RECORD_BYTES],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?.ok_or_else(limited)?;
            if original != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Data validation key was already used with another request",
                ));
            }
            // Historical result, never a new check after revocation or drift.
            return decode(&record);
        }
        let policy = self.get_data_validation_policy(policy_id)?;
        if policy.revoked {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Data validation policy was revoked",
            ));
        }
        if policy.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data validation policy changed",
            ));
        }
        let (session, workspace, binding) = self.validation_workspace(&policy.session_id)?;
        let task = self.get(&session.task_id)?;
        let subject = ValidationSubject {
            task_id: task.id.clone(),
            attempt_id: session.attempt_id.clone(),
            spec_revision: task.spec_revision,
            input_revision: binding.input_revision.clone(),
            version: VersionBinding::Data {
                dataset_revision: binding.input_revision.clone(),
            },
        };
        if policy.validator_id != VALIDATOR_ID
            || policy.scope != "data_contract_only"
            || policy.task_id != session.task_id
            || policy.project_id != session.project_id
            || policy.attempt_id != session.attempt_id
            || policy.workspace_id != workspace.id
            || policy.file_contract.subject != subject
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Data validation subject or attempt binding changed",
            ));
        }
        let proposal: ApprovalProposal = decode(&bounded_document(
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
            || proposal.kind != "data_validation"
            || proposal.snapshot["file_contract"] != json!(policy.file_contract)
            || proposal.snapshot["table"] != json!(policy.table)
            || proposal.snapshot["contract_digest"] != json!(policy.contract_digest)
            || proposal.snapshot["validator_id"] != json!(VALIDATOR_ID)
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Data policy no longer matches its confirmed owner snapshot",
            ));
        }
        let bind = |contracts: &[ArtifactContractView]| -> Result<Vec<BoundArtifact>> {
            contracts
                .iter()
                .map(|contract| {
                    if contract.canonical_root != workspace.root {
                        return Err(CoreError::new(
                            "INPUT_CHANGED",
                            "Approved data root no longer matches its workspace",
                        ));
                    }
                    BoundArtifact::from_verified_contract(contract).map_err(changed)
                })
                .collect()
        };
        let rule = ApprovedJsonTableRule::from_verified_policy(
            policy.approval_id.clone(),
            policy.revision.to_string(),
            subject,
            bind(&policy.file_contract.inputs)?,
            bind(&policy.file_contract.outputs)?,
            policy.table.clone(),
        )
        .map_err(changed)?;
        let canonical = rule.contract_view();
        if canonical.artifact_policy != policy.file_contract
            || canonical.table != policy.table
            || rule.contract_digest() != policy.contract_digest
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Persisted data contract digest is inconsistent",
            ));
        }
        let evidence = validate_json_table(&rule).map_err(changed)?;
        evidence.verify_current().map_err(changed)?;
        let view = evidence.view();
        if view.passed {
            let _contract_only = evidence.accept_contract(&rule).map_err(changed)?;
        }
        let record = DataValidationRecord {
            id: Uuid::new_v4().to_string(),
            policy_id: policy.id.clone(),
            task_id: policy.task_id.clone(),
            state: if view.passed {
                "contract_passed"
            } else {
                "contract_failed"
            }
            .into(),
            scope: "data_contract_only".into(),
            correctness: "approved_schema_ranges_metadata_only".into(),
            task_acceptance: "not_established".into(),
            integration_authorized: false,
            evidence: serde_json::to_value(view).map_err(|_| invalid())?,
            created_at: now()?,
        };
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        // Recheck revocation/revision at the commit boundary as well.
        let current: DataValidationPolicy = decode(&bounded_document(
            &tx,
            "data_validation_policies",
            policy_id,
        )?)?;
        if current.revoked
            || current.revision != expected_revision
            || encode(&current)? != encode(&policy)?
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data policy changed during validation",
            ));
        }
        tx.execute(
            "INSERT INTO data_validation_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                record.id,
                record.policy_id,
                record.task_id,
                record.state,
                encode(&record)?,
                idempotency_key,
                payload
            ],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &record.id,
            1,
            "data_validation.contract_recorded",
            json!({"record":record}),
            "deterministic_builtin_data_validator",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        execution::{ExecutionBinding, StopEvidence},
        registry::ManagedWorkspace,
        sessions::ManagedSession,
    };
    use std::{
        fs,
        os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt},
        path::{Path, PathBuf},
        process::Command,
    };
    use tempfile::TempDir;
    // Synthetic, explicitly never-dispatched storage fixtures. No real Provider
    // credentials, account, session, model or native dialog is invoked.
    struct Fixture {
        _directory: TempDir,
        database: PathBuf,
        store: Store,
        workspace: ManagedWorkspace,
        session: ManagedSession,
        selection: DataValidationSelection,
    }
    fn git(path: &Path, args: &[&str]) -> String {
        let mut command = Command::new("git");
        for (name, _) in std::env::vars_os() {
            if name.to_string_lossy().starts_with("GIT_") {
                command.env_remove(name);
            }
        }
        let result = command
            .args(["-c", "core.hooksPath=/dev/null", "-C"])
            .arg(path)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Generated fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@invalid")
            .env("GIT_COMMITTER_NAME", "Generated fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@invalid")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap().trim().into()
    }
    // The stopped session/connection are explicit storage fixtures. No Provider
    // process, native owner dialog or model is executed by these core tests.
    fn fixture(stopped: bool) -> Fixture {
        let directory = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let database = directory.path().join("baton.sqlite3");
        // Match the daemon's private-file preparation even with umask022;
        // Store itself also serves explicitly selected non-runtime databases.
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&database)
            .unwrap();
        assert_eq!(
            fs::metadata(&database).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let source = directory.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "--initial-branch=main"]);
        fs::write(source.join("input.txt"), "fixed input\n").unwrap();
        git(&source, &["add", "input.txt"]);
        git(&source, &["commit", "-m", "generated input"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let managed = directory.path().join("managed");
        fs::create_dir(&managed).unwrap();
        fs::set_permissions(&managed, fs::Permissions::from_mode(0o700)).unwrap();
        let mut store = Store::open(&database).unwrap();
        let project = store
            .import_project(&source, "Generated project", "import")
            .unwrap();
        let workspace = store
            .create_workspace(&project.id, project.revision, &head, &managed, "workspace")
            .unwrap();
        let task = store
            .create_registered(&project.id, "Validate recorded output", "task")
            .unwrap();
        let connection = json!({"id":"connection-fixture","name":"No Provider invoked","provider":"custom_acp","probe_state":"ready","host_id":"local","revision":1,"environment_id":"fixture-environment","identity_ref":"unknown-fixture","config_revision":"fixture-config","executable_sha256":"0".repeat(64),"capability_snapshot_id":"fixture-capability"});
        store
            .conn
            .execute(
                "INSERT INTO connections VALUES(?1,?2)",
                params!["connection-fixture", connection.to_string()],
            )
            .unwrap();
        let grant = store
            .create_grant(
                &project.id,
                &workspace.id,
                "connection-fixture",
                4,
                1,
                60_000,
                true,
                "grant",
            )
            .unwrap();
        let granted = &grant.connection_bindings["connection-fixture"];
        let binding = ExecutionBinding {
            connection_id: "connection-fixture".into(),
            environment_id: granted.environment_id.clone(),
            identity_ref: granted.identity_ref.clone(),
            host_id: "local".into(),
            workspace_id: workspace.id.clone(),
            config_revision: granted.config_revision.clone(),
            input_revision: workspace.base_revision.clone(),
            capability_snapshot_id: granted.capability_snapshot_id.clone(),
            policy_revision: format!("{}:1", grant.id),
        };
        store.configure_budget(true, &project.id, 4, 1).unwrap();
        store.configure_budget(false, &task.id, 4, 1).unwrap();
        let lease = store
            .acquire_workspace_lease(&workspace.id, "session-fixture", 0, 60_000)
            .unwrap();
        let intent = store
            .prepare_attempt(&task.id, task.revision, binding, "attempt")
            .unwrap();
        if stopped {
            store
                .confirm_stop(&intent.id, StopEvidence::NeverDispatched)
                .unwrap();
            store
                .release_workspace_lease(&workspace.id, "session-fixture", lease.fencing_token)
                .unwrap();
        }
        let timestamp = now().unwrap();
        let session = ManagedSession {
            id: "session-fixture".into(),
            task_id: task.id,
            project_id: project.id,
            workspace_id: workspace.id.clone(),
            connection_id: "connection-fixture".into(),
            grant_id: grant.id.clone(),
            state: "failed".into(),
            revision: 1,
            native_session_id: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            last_error: Some("Synthetic never-started storage fixture".into()),
            turn_state: None,
            attempt_id: intent.attempt_id,
            intent_id: intent.id,
            fencing_token: lease.fencing_token,
            runtime_snapshot: None,
        };
        store
            .conn
            .execute(
                "INSERT INTO sessions VALUES(?1,?2,?3,?4,?5,?6,?6,?7,?8,?9,?10)",
                params![
                    session.id,
                    session.task_id,
                    session.project_id,
                    session.workspace_id,
                    session.connection_id,
                    session.grant_id,
                    session.state,
                    encode(&session).unwrap(),
                    "session-key",
                    "fixture-only"
                ],
            )
            .unwrap();
        fs::write(
            workspace.root.join("reproduce.sh"),
            format!(
                "#!/bin/sh\n: > '{}'\n",
                workspace.root.join("SCRIPT_MUST_NOT_RUN").display()
            ),
        )
        .unwrap();
        fs::write(
            workspace.root.join("environment.json"),
            "{\"engine\":\"synthetic\"}\n",
        )
        .unwrap();
        let input_hash = agentbaton_workspaces::validation::sha256(b"fixed input\n");
        let script_hash = agentbaton_workspaces::validation::sha256(
            &fs::read(workspace.root.join("reproduce.sh")).unwrap(),
        );
        let environment_hash = agentbaton_workspaces::validation::sha256(
            &fs::read(workspace.root.join("environment.json")).unwrap(),
        );
        let analysis = agentbaton_workspaces::validation::AnalysisReproducibility {
            script_artifact_id: "script".into(),
            environment_artifact_id: "environment".into(),
            units: std::collections::BTreeMap::from([("x".into(), "m".into())]),
            timezone: "UTC".into(),
            seed: Some(7),
            scope: agentbaton_workspaces::validation::AnalysisScope::Full,
        };
        let table = JsonTableContract {
            table_artifact_id: "table".into(),
            dataset_rows: 1,
            columns: vec![agentbaton_workspaces::deterministic::Column {
                name: "x".into(),
                required: true,
                nullable: false,
                scalar: agentbaton_workspaces::deterministic::ScalarType::Number {
                    min: 0.0,
                    max: 10.0,
                },
            }],
            analysis: analysis.clone(),
            limits: agentbaton_workspaces::deterministic::JsonLimits {
                max_bytes: 4096,
                ..Default::default()
            },
        };
        let document = json!({"metadata":{"input_revision":workspace.base_revision,"dataset_revision":workspace.base_revision,"input_digests":{"input":input_hash},"table_artifact_id":"table","row_count":1,"dataset_rows":1,"units":analysis.units,"timezone":"UTC","seed":7,"scope":{"kind":"full"},"script_sha256":script_hash,"environment_sha256":environment_hash},"rows":[{"x":1.5}]});
        fs::write(
            workspace.root.join("table.json"),
            serde_json::to_vec(&document).unwrap(),
        )
        .unwrap();
        let selection = DataValidationSelection {
            session_id: session.id.clone(),
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "input.txt".into(),
                max_bytes: 1024,
                expected_sha256: Some(input_hash),
            }],
            outputs: vec![
                ArtifactRequirement {
                    artifact_id: "table".into(),
                    relative_path: "table.json".into(),
                    max_bytes: 4096,
                    expected_sha256: None,
                },
                ArtifactRequirement {
                    artifact_id: "script".into(),
                    relative_path: "reproduce.sh".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                },
                ArtifactRequirement {
                    artifact_id: "environment".into(),
                    relative_path: "environment.json".into(),
                    max_bytes: 1024,
                    expected_sha256: None,
                },
            ],
            table,
        };
        Fixture {
            _directory: directory,
            database,
            store,
            workspace,
            session,
            selection,
        }
    }
    fn approve(f: &mut Fixture) -> DataValidationPolicy {
        let proposal = f
            .store
            .propose_data_validation_policy(&f.selection, "propose-data")
            .unwrap();
        assert_eq!(proposal.kind, "data_validation");
        assert_eq!(proposal.snapshot["scope"], "data_contract_only");
        serde_json::from_value(f.store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap()
    }
    fn update_table(f: &Fixture, update: impl FnOnce(&mut Value)) {
        let path = f.workspace.root.join("table.json");
        let mut document: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        update(&mut document);
        fs::write(path, serde_json::to_vec(&document).unwrap()).unwrap();
    }
    #[test]
    fn real_contract_pass_and_historical_replay_are_durable_and_namespaces_stay_separate() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let before = f.store.get(&f.session.task_id).unwrap();
        let record = f.store.run_data_validation(&policy.id, 1, "run").unwrap();
        assert_eq!(record.state, "contract_passed");
        assert_eq!(record.scope, "data_contract_only");
        assert_eq!(record.correctness, "approved_schema_ranges_metadata_only");
        assert_eq!(record.task_acceptance, "not_established");
        assert!(!record.integration_authorized);
        assert_eq!(record.evidence["validator_id"], VALIDATOR_ID);
        assert_eq!(
            record.evidence["contract"]["artifact_policy"]["subject"]["version"]["kind"],
            "data"
        );
        assert_eq!(
            record.evidence["contract"]["artifact_policy"]["subject"]["version"]
                ["dataset_revision"],
            f.workspace.base_revision
        );
        assert_eq!(
            f.store.get(&f.session.task_id).unwrap().revision,
            before.revision
        );
        assert!(!f.workspace.root.join("SCRIPT_MUST_NOT_RUN").exists());
        assert!(f
            .store
            .list_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
        assert!(f
            .store
            .list_validation_records(&f.session.task_id)
            .unwrap()
            .is_empty());
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "run")
                .unwrap()
                .id,
            record.id
        );
        drop(f.store);
        let mut restored = Store::open(&f.database).unwrap();
        assert_eq!(
            restored
                .list_data_validation_records(&f.session.task_id)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            restored
                .run_data_validation(&policy.id, 1, "run")
                .unwrap()
                .id,
            record.id
        );
        assert_eq!(
            restored
                .list_data_validation_policies(&f.session.task_id)
                .unwrap()[0]
                .contract_digest,
            policy.contract_digest
        );
    }
    #[test]
    fn schema_failure_and_bad_metadata_persist_actual_failed_receipts_not_integrity_acceptance() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        update_table(&f, |value| value["rows"][0]["x"] = json!(99));
        let failed = f
            .store
            .run_data_validation(&policy.id, 1, "schema-failed")
            .unwrap();
        assert_eq!(failed.state, "contract_failed");
        assert_eq!(failed.evidence["passed"], false);
        assert_eq!(failed.evidence["file_integrity"]["passed"], true);
        assert_eq!(failed.evidence["failure"]["code"], "numeric_range");
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "schema-failed")
                .unwrap()
                .id,
            failed.id
        );
        update_table(&f, |value| {
            value["rows"][0]["x"] = json!(1.5);
            value["metadata"]["units"] = json!({"x":"km"});
        });
        let bad_units = f
            .store
            .run_data_validation(&policy.id, 1, "bad-units")
            .unwrap();
        assert_eq!(bad_units.state, "contract_failed");
        assert_eq!(bad_units.evidence["failure"]["code"], "metadata_mismatch");
        assert_eq!(
            f.store
                .list_data_validation_records(&f.session.task_id)
                .unwrap()
                .len(),
            2
        );
    }
    #[test]
    fn stopped_receipt_is_required_even_if_session_string_claims_stopped() {
        let mut f = fixture(false);
        let mut fake = f.session.clone();
        fake.state = "stopped".into();
        f.store
            .conn
            .execute(
                "UPDATE sessions SET document=?1 WHERE id=?2",
                params![encode(&fake).unwrap(), fake.id],
            )
            .unwrap();
        assert_eq!(
            f.store
                .propose_data_validation_policy(&f.selection, "forged-stop")
                .unwrap_err()
                .code,
            "WORKSPACE_BUSY"
        );
        assert!(f
            .store
            .list_data_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn owner_confirmation_detects_root_change_and_never_approves_mutated_snapshot() {
        let mut f = fixture(true);
        let proposal = f
            .store
            .propose_data_validation_policy(&f.selection, "proposal")
            .unwrap();
        let mut changed = f.store.get(&f.session.task_id).unwrap();
        changed.spec_revision += 1;
        f.store
            .conn
            .execute(
                "UPDATE tasks SET document=?1 WHERE id=?2",
                params![encode(&changed).unwrap(), changed.id],
            )
            .unwrap();
        assert_eq!(
            f.store.finalize_proposal(&proposal.id, 1).unwrap_err().code,
            "INPUT_CHANGED"
        );
        assert!(f
            .store
            .list_data_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
        let mut root = fixture(true);
        let proposal = root
            .store
            .propose_data_validation_policy(&root.selection, "proposal")
            .unwrap();
        let moved = root.workspace.root.with_file_name("old-root");
        fs::rename(&root.workspace.root, moved).unwrap();
        fs::create_dir(&root.workspace.root).unwrap();
        assert_eq!(
            root.store
                .finalize_proposal(&proposal.id, 1)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
    }
    #[test]
    fn persisted_contract_and_validator_drift_cannot_reuse_owner_confirmation() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        for mode in 0..5 {
            let mut changed = policy.clone();
            match mode {
                0 => changed
                    .table
                    .analysis
                    .units
                    .insert("x".into(), "km".into())
                    .map(|_| ()),
                1 => {
                    changed.file_contract.subject.spec_revision += 1;
                    None
                }
                2 => {
                    changed.contract_digest = "0".repeat(64);
                    None
                }
                3 => {
                    changed.validator_id = "unapproved-script".into();
                    None
                }
                4 => {
                    changed.file_contract.subject.version = VersionBinding::Data {
                        dataset_revision: "forged".into(),
                    };
                    None
                }
                _ => unreachable!(),
            };
            f.store
                .conn
                .execute(
                    "UPDATE data_validation_policies SET document=?1 WHERE id=?2",
                    params![encode(&changed).unwrap(), policy.id],
                )
                .unwrap();
            assert_eq!(
                f.store
                    .run_data_validation(&policy.id, 1, &format!("drift-{mode}"))
                    .unwrap_err()
                    .code,
                "INPUT_CHANGED"
            );
        }
        assert!(f
            .store
            .list_data_validation_records(&f.session.task_id)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn task_spec_input_and_later_writer_epoch_are_server_side_read_guards() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let original = f
            .store
            .run_data_validation(&policy.id, 1, "original")
            .unwrap();
        let lease = f
            .store
            .acquire_workspace_lease(
                &f.workspace.id,
                "later-writer",
                f.session.fencing_token,
                60_000,
            )
            .unwrap();
        f.store
            .release_workspace_lease(&f.workspace.id, "later-writer", lease.fencing_token)
            .unwrap();
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "later")
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "original")
                .unwrap()
                .id,
            original.id
        );
        let mut spec = fixture(true);
        let policy = approve(&mut spec);
        spec.store
            .conn
            .execute(
                "UPDATE tasks SET document=json_set(document,'$.spec_revision',2) WHERE id=?1",
                [&spec.session.task_id],
            )
            .unwrap();
        assert_eq!(
            spec.store
                .run_data_validation(&policy.id, 1, "spec")
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        let mut input = fixture(true);
        let policy = approve(&mut input);
        input.store.conn.execute("UPDATE attempts SET binding=json_set(binding,'$.input_revision','changed') WHERE id=?1",[&input.session.attempt_id]).unwrap();
        assert_eq!(
            input
                .store
                .run_data_validation(&policy.id, 1, "input")
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
    }
    #[test]
    fn revocation_and_key_conflict_preserve_history_without_running_again() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let original = f
            .store
            .run_data_validation(&policy.id, 1, "original")
            .unwrap();
        let revoked = f
            .store
            .revoke_data_validation_policy(&policy.id, 1)
            .unwrap();
        assert_eq!(revoked.revision, 2);
        assert!(revoked.revoked);
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 2, "fresh")
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "original")
                .unwrap()
                .id,
            original.id
        );
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 2, "original")
                .unwrap_err()
                .code,
            "CONFIG_CONFLICT"
        );
        assert_eq!(
            f.store
                .revoke_data_validation_policy(&policy.id, 1)
                .unwrap_err()
                .code,
            "STALE_REVISION"
        );
        assert_eq!(
            f.store
                .list_data_validation_records(&f.session.task_id)
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn failed_run_journal_rolls_back_receipt_and_keeps_key_retryable() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let before = f.store.sequence().unwrap();
        f.store.conn.execute_batch("CREATE TRIGGER reject_data_events BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(f
            .store
            .run_data_validation(&policy.id, 1, "atomic")
            .is_err());
        assert_eq!(f.store.sequence().unwrap(), before);
        assert!(f
            .store
            .list_data_validation_records(&f.session.task_id)
            .unwrap()
            .is_empty());
        f.store
            .conn
            .execute_batch("DROP TRIGGER reject_data_events")
            .unwrap();
        let record = f
            .store
            .run_data_validation(&policy.id, 1, "atomic")
            .unwrap();
        assert_eq!(record.state, "contract_passed");
        assert_eq!(f.store.sequence().unwrap(), before + 1);
    }
    #[test]
    fn owner_policy_and_its_journal_are_atomic_and_unconsumed_proposals_cannot_run() {
        let mut f = fixture(true);
        let proposal = f
            .store
            .propose_data_validation_policy(&f.selection, "proposal")
            .unwrap();
        f.store.conn.execute_batch("CREATE TRIGGER reject_data_approval BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='data_validation.policy_approved' BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").unwrap();
        assert!(f.store.finalize_proposal(&proposal.id, 1).is_err());
        assert!(f
            .store
            .list_data_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
        f.store.conn.execute_batch("DROP TRIGGER reject_data_approval;CREATE TRIGGER reject_confirmation BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='approval.confirmed' BEGIN SELECT RAISE(ABORT,'synthetic finalization failure'); END;").unwrap();
        assert!(f.store.finalize_proposal(&proposal.id, 1).is_err());
        let pending = f
            .store
            .list_data_validation_policies(&f.session.task_id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            f.store
                .run_data_validation(&pending[0].id, 1, "not-consumed")
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        f.store
            .conn
            .execute_batch("DROP TRIGGER reject_confirmation")
            .unwrap();
        let policy: DataValidationPolicy =
            serde_json::from_value(f.store.finalize_proposal(&proposal.id, 1).unwrap()).unwrap();
        assert_eq!(policy.id, pending[0].id);
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "confirmed")
                .unwrap()
                .state,
            "contract_passed"
        );
    }
    #[test]
    fn path_and_size_failures_are_real_failed_receipts_not_permission_widening() {
        let mut f = fixture(true);
        let mut unsafe_selection = f.selection.clone();
        unsafe_selection.outputs[0].relative_path = "../escape".into();
        assert!(f
            .store
            .propose_data_validation_policy(&unsafe_selection, "unsafe")
            .is_err());
        let policy = approve(&mut f);
        let path = f.workspace.root.join("table.json");
        let original_bytes = fs::read(&path).unwrap();
        let saved = f.workspace.root.join("saved-table");
        fs::rename(&path, &saved).unwrap();
        symlink(&saved, &path).unwrap();
        let failed = f
            .store
            .run_data_validation(&policy.id, 1, "symlink")
            .unwrap();
        assert_eq!(failed.state, "contract_failed");
        assert_eq!(failed.evidence["failure"]["code"], "artifact_integrity");
        fs::remove_file(&path).unwrap();
        fs::write(&path, vec![b'x'; 4097]).unwrap();
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "oversize")
                .unwrap()
                .state,
            "contract_failed"
        );
        assert_eq!(fs::read(saved).unwrap(), original_bytes);
    }
    #[test]
    fn raw_document_caps_precede_decode_for_policies_runs_and_historical_replay() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let record = f.store.run_data_validation(&policy.id, 1, "old").unwrap();
        let giant = "x".repeat(4 * 1024 * 1024 + 1);
        f.store
            .conn
            .execute(
                "UPDATE data_validation_runs SET document=?1 WHERE id=?2",
                params![giant, record.id],
            )
            .unwrap();
        assert_eq!(
            f.store
                .list_data_validation_records(&f.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            f.store
                .run_data_validation(&policy.id, 1, "old")
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        f.store
            .conn
            .execute(
                "UPDATE data_validation_policies SET document=?1 WHERE id=?2",
                params![giant, policy.id],
            )
            .unwrap();
        assert_eq!(
            f.store
                .list_data_validation_policies(&f.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            f.store
                .get_data_validation_policy(&policy.id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM data_validation_runs", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    #[test]
    fn listing_aggregate_and_count_limits_reject_before_decoding_and_keep_history() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let record = f.store.run_data_validation(&policy.id, 1, "base").unwrap();
        let mut large = record.clone();
        large.evidence = json!({"legacy":"x".repeat(2*1024*1024+1)});
        f.store
            .conn
            .execute(
                "UPDATE data_validation_runs SET document=?1 WHERE id=?2",
                params![encode(&large).unwrap(), large.id],
            )
            .unwrap();
        large.id = "second-legacy".into();
        f.store
            .conn
            .execute(
                "INSERT INTO data_validation_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    large.id,
                    policy.id,
                    policy.task_id,
                    large.state,
                    encode(&large).unwrap(),
                    "second-legacy",
                    "{}"
                ],
            )
            .unwrap();
        assert_eq!(
            f.store
                .list_data_validation_records(&f.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        let mut many = fixture(true);
        let policy = approve(&mut many);
        for n in 0..101 {
            many.store.conn.execute("INSERT INTO data_validation_runs VALUES(?1,?2,?3,'contract_failed','{}',?4,'{}')",params![format!("legacy-{n}"),policy.id,policy.task_id,format!("legacy-key-{n}")]).unwrap();
        }
        assert_eq!(
            many.store
                .list_data_validation_records(&many.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            many.store
                .conn
                .query_row("SELECT count(*) FROM data_validation_runs", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            101
        );
    }
    #[test]
    fn actual_authenticated_unix_ipc_records_normal_failed_and_rejects_forged_authority() {
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        struct Running {
            stop: Arc<AtomicBool>,
            thread: Option<std::thread::JoinHandle<()>>,
        }
        impl Drop for Running {
            fn drop(&mut self) {
                self.stop.store(true, Ordering::Relaxed);
                self.thread.take().unwrap().join().unwrap();
            }
        }
        let f = fixture(true);
        let runtime = f._directory.path().canonicalize().unwrap();
        drop(f.store);
        let daemon = crate::ipc::Daemon::bind(&runtime).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let run = stop.clone();
        let _running = Running {
            stop,
            thread: Some(std::thread::spawn(move || daemon.run(run).unwrap())),
        };
        let call = |request| crate::ipc::call(&runtime, request).unwrap();
        for name in ["approved", "root", "binding", "task_id"] {
            let mut forged = json!(f.selection);
            forged[name] = json!(true);
            assert_eq!(
                call(
                    json!({"method":"data_validation_policies.propose","params":forged,"idempotency_key":format!("forged-{name}")})
                )["error"]["code"],
                "INVALID_REQUEST"
            );
        }
        let proposed = call(
            json!({"method":"data_validation_policies.propose","params":f.selection,"idempotency_key":"data-proposal"}),
        );
        assert!(proposed.get("error").is_none(), "{proposed}");
        let proposal = &proposed["result"];
        assert_eq!(proposal["kind"], "data_validation");
        // Authenticated private-CLI confirmation fixture; not a native dialog or renderer permission.
        let confirmed = call(
            json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":1,"idempotency_key":"owner-confirm"}),
        );
        assert!(confirmed.get("error").is_none(), "{confirmed}");
        let policy = &confirmed["result"];
        let policies = call(
            json!({"method":"data_validation_policies.list","params":{"task_id":f.session.task_id}}),
        );
        assert_eq!(policies["result"].as_array().unwrap().len(), 1);
        assert_eq!(policies["result"][0]["id"], policy["id"]);
        assert_eq!(
            call(
                json!({"method":"validations.run","params":{"policy_id":policy["id"]},"expected_revision":1,"idempotency_key":"wrong-namespace"})
            )["error"]["code"],
            "NOT_FOUND"
        );
        assert_eq!(
            call(
                json!({"method":"data_validations.run","params":{"policy_id":policy["id"],"passed":true},"expected_revision":1,"idempotency_key":"forged-result"})
            )["error"]["code"],
            "INVALID_REQUEST"
        );
        let passed = call(
            json!({"method":"data_validations.run","params":{"policy_id":policy["id"]},"expected_revision":1,"idempotency_key":"actual"}),
        );
        assert!(passed.get("error").is_none(), "{passed}");
        assert_eq!(passed["result"]["state"], "contract_passed");
        assert_eq!(passed["result"]["integration_authorized"], false);
        assert_eq!(passed["result"]["task_acceptance"], "not_established");
        let file = f.workspace.root.join("table.json");
        let mut document: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        document["rows"][0]["x"] = json!(99);
        fs::write(&file, serde_json::to_vec(&document).unwrap()).unwrap();
        let failed = call(
            json!({"method":"data_validations.run","params":{"policy_id":policy["id"]},"expected_revision":1,"idempotency_key":"actual-failed"}),
        );
        assert_eq!(failed["result"]["state"], "contract_failed");
        let records =
            call(json!({"method":"data_validations.list","params":{"task_id":f.session.task_id}}));
        assert_eq!(records["result"].as_array().unwrap().len(), 2);
        assert_eq!(
            call(json!({"method":"validations.list","params":{"task_id":f.session.task_id}}))
                ["result"],
            json!([])
        );
        assert_eq!(
            call(
                json!({"method":"validation_policies.list","params":{"task_id":f.session.task_id}})
            )["result"],
            json!([])
        );
        let revoked = call(
            json!({"method":"data_validation_policies.revoke","params":{"policy_id":policy["id"]},"expected_revision":1}),
        );
        assert_eq!(revoked["result"]["revoked"], true);
        assert_eq!(
            call(
                json!({"method":"data_validations.run","params":{"policy_id":policy["id"]},"expected_revision":2,"idempotency_key":"revoked"})
            )["error"]["code"],
            "PERMISSION_DENIED"
        );
        assert!(!f.workspace.root.join("SCRIPT_MUST_NOT_RUN").exists());
    }
}
