//! Owner-approved artifact integrity. These receipts never accept a task, prove
//! code correctness, execute repository checks or authorize Git integration.
use crate::{
    execution::ExecutionBinding,
    registry::ManagedWorkspace,
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store},
    CoreError, Result,
};
use agentbaton_workspaces::validation::{
    validate_artifacts, AnalysisReproducibility, ApprovedRule, ArtifactRequirement, BoundArtifact,
    ValidationPolicyContractView, ValidationSubject, ValidationView, VersionBinding,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationSelection {
    pub session_id: String,
    #[serde(default)]
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<ArtifactRequirement>,
    pub analysis: Option<AnalysisReproducibility>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::StopEvidence;
    use crate::sessions::ManagedSession;
    use std::{
        fs,
        os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt},
        path::{Path, PathBuf},
        process::Command,
    };
    use tempfile::TempDir;

    struct Fixture {
        _directory: TempDir,
        database: PathBuf,
        store: Store,
        workspace: ManagedWorkspace,
        session: ManagedSession,
        selection: ValidationSelection,
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
        fs::write(workspace.root.join("report.txt"), "generated output").unwrap();
        let selection = ValidationSelection {
            session_id: session.id.clone(),
            inputs: vec![ArtifactRequirement {
                artifact_id: "input".into(),
                relative_path: "input.txt".into(),
                max_bytes: 1024,
                expected_sha256: Some(agentbaton_workspaces::validation::sha256(b"fixed input\n")),
            }],
            outputs: vec![ArtifactRequirement {
                artifact_id: "report".into(),
                relative_path: "report.txt".into(),
                max_bytes: 1024,
                expected_sha256: Some(agentbaton_workspaces::validation::sha256(
                    b"generated output",
                )),
            }],
            analysis: None,
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
    fn approve(fixture: &mut Fixture) -> ValidationPolicy {
        let proposal = fixture
            .store
            .propose_validation_policy(&fixture.selection, "propose")
            .unwrap();
        assert_eq!(proposal.kind, "validation");
        assert_eq!(proposal.snapshot["scope"], "artifact_integrity_only");
        serde_json::from_value(
            fixture
                .store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn integrity_receipt_is_durable_idempotent_and_never_claims_task_acceptance() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let before = f.store.get(&f.session.task_id).unwrap();
        assert!(before.revision > before.spec_revision);
        assert_eq!(before.spec_revision, 1);
        let record = f
            .store
            .run_artifact_validation(&policy.id, 1, "check")
            .unwrap();
        assert_eq!(record.state, "integrity_passed");
        assert_eq!(record.correctness, "not_established");
        assert_eq!(record.task_acceptance, "not_established");
        assert!(!record.integration_authorized);
        assert_eq!(record.evidence.subject.attempt_id, f.session.attempt_id);
        assert_eq!(record.evidence.subject.spec_revision, 1);
        assert_eq!(
            f.store.get(&f.session.task_id).unwrap().revision,
            before.revision
        );
        let sequence = f.store.sequence().unwrap();
        assert_eq!(
            f.store
                .run_artifact_validation(&policy.id, 1, "check")
                .unwrap()
                .id,
            record.id
        );
        assert_eq!(f.store.sequence().unwrap(), sequence);
        drop(f.store);
        let reopened = Store::open(&f.database).unwrap();
        assert_eq!(
            reopened
                .list_validation_records(&f.session.task_id)
                .unwrap()[0]
                .id,
            record.id
        );
    }
    #[test]
    fn unsafe_output_is_recorded_as_failed_and_original_file_is_untouched() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let outside = f._directory.path().join("original.txt");
        fs::write(&outside, "must remain").unwrap();
        fs::remove_file(f.workspace.root.join("report.txt")).unwrap();
        symlink(&outside, f.workspace.root.join("report.txt")).unwrap();
        let record = f
            .store
            .run_artifact_validation(&policy.id, 1, "unsafe")
            .unwrap();
        assert_eq!(record.state, "failed");
        assert!(!record.evidence.passed);
        assert!(!record.integration_authorized);
        assert_eq!(fs::read_to_string(outside).unwrap(), "must remain");
    }
    #[test]
    fn session_status_without_a_confirmed_attempt_stop_cannot_authorize_reading() {
        let mut f = fixture(false);
        assert_eq!(
            f.store
                .propose_validation_policy(&f.selection, "propose")
                .unwrap_err()
                .code,
            "WORKSPACE_BUSY"
        );
        assert!(f
            .store
            .list_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn matching_bytes_from_a_later_writer_do_not_borrow_an_old_attempt_policy() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        fs::remove_file(f.workspace.root.join("report.txt")).unwrap();
        let next = f
            .store
            .acquire_workspace_lease(
                &f.workspace.id,
                "later-writer",
                f.session.fencing_token,
                60_000,
            )
            .unwrap();
        f.store
            .release_workspace_lease(&f.workspace.id, "later-writer", next.fencing_token)
            .unwrap();
        fs::write(f.workspace.root.join("report.txt"), "generated output").unwrap();
        assert_eq!(
            f.store
                .run_artifact_validation(&policy.id, 1, "after-later-writer")
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert!(f
            .store
            .list_validation_records(&f.session.task_id)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn validation_lists_reject_large_raw_documents_before_deserializing_them() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let record = f
            .store
            .run_artifact_validation(&policy.id, 1, "original")
            .unwrap();
        // A legacy/corrupted oversized single document is deliberately invalid
        // JSON. RESOURCE_LIMIT (not decode error) proves the predecode guard.
        f.store
            .conn
            .execute(
                "UPDATE validation_runs SET document=?1 WHERE id=?2",
                params!["x".repeat(4 * 1024 * 1024 + 1), record.id],
            )
            .unwrap();
        assert_eq!(
            f.store
                .list_validation_records(&f.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        f.store
            .conn
            .execute(
                "UPDATE validation_runs SET document=?1 WHERE id=?2",
                params![encode(&record).unwrap(), record.id],
            )
            .unwrap();
        // Multiple individually bounded legacy documents still cannot exceed
        // the aggregate budget. Existing history is retained in the database.
        let mut large = policy.clone();
        large.contract.outputs[0].requirement.relative_path =
            "x".repeat(2 * 1024 * 1024 + 1).into();
        f.store
            .conn
            .execute(
                "UPDATE validation_policies SET document=?1 WHERE id=?2",
                params![encode(&large).unwrap(), policy.id],
            )
            .unwrap();
        let mut second = large.clone();
        second.id = "second-policy-fixture".into();
        second.approval_id = "second-proposal-fixture".into();
        f.store
            .conn
            .execute(
                "INSERT INTO approval_proposals(id,document) VALUES(?1,?2)",
                params![second.approval_id, "{}"],
            )
            .unwrap();
        f.store
            .conn
            .execute(
                "INSERT INTO validation_policies VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    second.id,
                    second.task_id,
                    second.session_id,
                    second.attempt_id,
                    second.workspace_id,
                    second.approval_id,
                    encode(&second).unwrap()
                ],
            )
            .unwrap();
        assert_eq!(
            f.store
                .list_validation_policies(&f.session.task_id)
                .unwrap_err()
                .code,
            "RESOURCE_LIMIT"
        );
        assert_eq!(
            f.store
                .conn
                .query_row("SELECT count(*) FROM validation_policies", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            2
        );
    }
    #[test]
    fn owner_confirmation_rejects_changed_workspace_identity() {
        let mut f = fixture(true);
        let proposal = f
            .store
            .propose_validation_policy(&f.selection, "propose")
            .unwrap();
        fs::rename(
            &f.workspace.root,
            f.workspace.root.with_extension("preserved"),
        )
        .unwrap();
        fs::create_dir(&f.workspace.root).unwrap();
        assert_eq!(
            f.store
                .finalize_proposal(&proposal.id, proposal.revision)
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
        assert!(f
            .store
            .list_validation_policies(&f.session.task_id)
            .unwrap()
            .is_empty());
    }
    #[test]
    fn revocation_blocks_new_checks_without_rewriting_existing_evidence() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        let first = f
            .store
            .run_artifact_validation(&policy.id, 1, "original")
            .unwrap();
        assert!(
            f.store
                .revoke_validation_policy(&policy.id, 1)
                .unwrap()
                .revoked
        );
        assert_eq!(
            f.store
                .run_artifact_validation(&policy.id, 2, "new")
                .unwrap_err()
                .code,
            "PERMISSION_DENIED"
        );
        assert_eq!(
            f.store
                .run_artifact_validation(&policy.id, 1, "original")
                .unwrap()
                .id,
            first.id
        );
        assert_eq!(
            f.store
                .list_validation_records(&f.session.task_id)
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn journal_failure_rolls_back_receipt_and_allows_safe_same_key_retry() {
        let mut f = fixture(true);
        let policy = approve(&mut f);
        f.store.conn.execute_batch("CREATE TRIGGER refuse_validation_journal BEFORE INSERT ON events WHEN json_extract(NEW.document,'$.kind')='validation.artifact_integrity_recorded' BEGIN SELECT RAISE(ABORT,'generated fault'); END;").unwrap();
        assert!(f
            .store
            .run_artifact_validation(&policy.id, 1, "retry")
            .is_err());
        assert!(f
            .store
            .list_validation_records(&f.session.task_id)
            .unwrap()
            .is_empty());
        f.store
            .conn
            .execute_batch("DROP TRIGGER refuse_validation_journal")
            .unwrap();
        assert_eq!(
            f.store
                .run_artifact_validation(&policy.id, 1, "retry")
                .unwrap()
                .state,
            "integrity_passed"
        );
    }
    #[test]
    fn actual_authenticated_ipc_uses_server_policy_and_rejects_forged_result_fields() {
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
        let mut forged = json!(f.selection);
        forged["approved"] = json!(true);
        assert_eq!(
            call(
                json!({"method":"validation_policies.propose","params":forged,"idempotency_key":"forged"})
            )["error"]["code"],
            "INVALID_REQUEST"
        );
        let proposed = call(
            json!({"method":"validation_policies.propose","params":f.selection,"idempotency_key":"owner-proposal"}),
        );
        assert!(proposed.get("error").is_none(), "{proposed}");
        let proposal = &proposed["result"];
        // Trusted private-CLI confirmation fixture, not a renderer approval or a
        // real native dialog. The production renderer has no finalize command.
        let confirmed = call(
            json!({"method":"approvals.finalize","params":{"proposal_id":proposal["id"]},"expected_revision":proposal["revision"],"idempotency_key":"owner-confirm"}),
        );
        assert!(confirmed.get("error").is_none(), "{confirmed}");
        let policy = &confirmed["result"];
        let forged = call(
            json!({"method":"validations.run","params":{"policy_id":policy["id"],"passed":true,"integration_authorized":true},"expected_revision":1,"idempotency_key":"forged-result"}),
        );
        assert_eq!(forged["error"]["code"], "INVALID_REQUEST");
        let checked = call(
            json!({"method":"validations.run","params":{"policy_id":policy["id"]},"expected_revision":1,"idempotency_key":"actual-check"}),
        );
        assert!(checked.get("error").is_none(), "{checked}");
        assert_eq!(checked["result"]["state"], "integrity_passed");
        assert_eq!(checked["result"]["task_acceptance"], "not_established");
        assert_eq!(checked["result"]["integration_authorized"], false);
        let records =
            call(json!({"method":"validations.list","params":{"task_id":f.session.task_id}}));
        assert_eq!(records["result"].as_array().unwrap().len(), 1);
        assert_eq!(records["result"][0]["id"], checked["result"]["id"]);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationPolicy {
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
    pub contract: ValidationPolicyContractView,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationRecord {
    pub id: String,
    pub policy_id: String,
    pub task_id: String,
    pub state: String,
    pub scope: String,
    pub correctness: String,
    pub task_acceptance: String,
    pub integration_authorized: bool,
    pub evidence: ValidationView,
    pub created_at: String,
}

fn invalid() -> CoreError {
    CoreError::new(
        "INVALID_REQUEST",
        "Select bounded artifact contracts from one registered stopped session",
    )
}
fn validation_error(_: agentbaton_workspaces::validation::ValidationError) -> CoreError {
    CoreError::new("INPUT_CHANGED", "Artifact policy or selected root changed; review a fresh policy instead of reusing approval")
}
fn key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 {
        Err(invalid())
    } else {
        Ok(())
    }
}

fn validation_event(
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

// Bound stored/raw JSON bytes before decoding any record. This Vec API refuses
// an oversized listing explicitly; it never silently drops history or contracts.
pub(crate) fn bounded_documents<T: for<'de> Deserialize<'de>>(
    connection: &rusqlite::Connection,
    table: &str,
    task_id: &str,
) -> Result<Vec<T>> {
    const MAX_BYTES: i64 = 4 * 1024 * 1024;
    let (metadata_sql, document_sql) = match table {
        "validation_policies" => ("SELECT id,octet_length(document) FROM validation_policies WHERE task_id=?1 ORDER BY rowid DESC LIMIT 101", "SELECT document FROM validation_policies WHERE id=?1 AND task_id=?2 AND octet_length(document)<=?3"),
        "validation_runs" => ("SELECT id,octet_length(document) FROM validation_runs WHERE task_id=?1 ORDER BY rowid DESC LIMIT 101", "SELECT document FROM validation_runs WHERE id=?1 AND task_id=?2 AND octet_length(document)<=?3"),
        "data_validation_policies" => ("SELECT id,octet_length(document) FROM data_validation_policies WHERE task_id=?1 ORDER BY rowid DESC LIMIT 101", "SELECT document FROM data_validation_policies WHERE id=?1 AND task_id=?2 AND octet_length(document)<=?3"),
        "data_validation_runs" => ("SELECT id,octet_length(document) FROM data_validation_runs WHERE task_id=?1 ORDER BY rowid DESC LIMIT 101", "SELECT document FROM data_validation_runs WHERE id=?1 AND task_id=?2 AND octet_length(document)<=?3"),
        _ => return Err(CoreError::new("INTERNAL_ERROR", "Unknown validation record collection")),
    };
    let limited = || {
        CoreError::new("RESOURCE_LIMIT", "Validation listing exceeds its count or raw-byte bound; records remain local and were not silently omitted")
    };
    let metadata: Vec<(String, i64)> = {
        let mut statement = connection.prepare(metadata_sql).map_err(db_error)?;
        let rows = statement
            .query_map([task_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_error)?;
        rows.collect::<std::result::Result<_, _>>()
            .map_err(db_error)?
    };
    if metadata.len() > 100 {
        return Err(limited());
    }
    let total = metadata
        .iter()
        .try_fold(0i64, |sum, (_, bytes)| {
            if *bytes < 0 {
                None
            } else {
                sum.checked_add(*bytes)
            }
        })
        .ok_or_else(limited)?;
    if total > MAX_BYTES {
        return Err(limited());
    }
    let mut result = Vec::with_capacity(metadata.len());
    let mut remaining = MAX_BYTES;
    for (id, _) in metadata {
        let raw: String = connection
            .query_row(document_sql, params![id, task_id, remaining], |row| {
                row.get(0)
            })
            .optional()
            .map_err(db_error)?
            .ok_or_else(limited)?;
        remaining = remaining
            .checked_sub(raw.len() as i64)
            .ok_or_else(limited)?;
        result.push(decode(&raw)?);
    }
    Ok(result)
}

impl Store {
    pub(crate) fn validation_workspace(
        &self,
        session_id: &str,
    ) -> Result<(
        crate::sessions::ManagedSession,
        ManagedWorkspace,
        ExecutionBinding,
    )> {
        self.validation_workspace_inner(session_id, None)
    }
    pub(crate) fn validation_workspace_for_owned_command(
        &self,
        session_id: &str,
        owned: &crate::command_validation::CommandReadAuthority,
    ) -> Result<(
        crate::sessions::ManagedSession,
        ManagedWorkspace,
        ExecutionBinding,
    )> {
        self.validation_workspace_inner(session_id, Some(owned))
    }
    fn validation_workspace_inner(
        &self,
        session_id: &str,
        owned: Option<&crate::command_validation::CommandReadAuthority>,
    ) -> Result<(
        crate::sessions::ManagedSession,
        ManagedWorkspace,
        ExecutionBinding,
    )> {
        let session = self.get_session(session_id)?;
        self.check_command_read_fence(&session, owned)?;
        let intent = self.intent(&session.intent_id)?;
        if intent.state != crate::execution::IntentState::Stopped
            || intent.attempt_id != session.attempt_id
            || intent.task_id != session.task_id
        {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "The attempt has no matching confirmed stop receipt",
            ));
        }

        let project = self.get_project(&session.project_id)?;
        if project.archived {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Archived project cannot approve or run new validation",
            ));
        }
        if !matches!(session.state.as_str(), "stopped" | "failed") {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Stop and reconcile the session before validating its files",
            ));
        }
        let raw: String = self
            .conn
            .query_row(
                "SELECT document FROM managed_workspaces WHERE id=?1 AND project_id=?2",
                params![session.workspace_id, session.project_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Session workspace is unavailable"))?;
        let workspace: ManagedWorkspace = decode(&raw)?;
        let lease = self.workspace_lease(&workspace.id)?.ok_or_else(|| {
            CoreError::new("INPUT_CHANGED", "Workspace write epoch is unavailable")
        })?;
        if lease.fencing_token != session.fencing_token {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "A later writer superseded this attempt's workspace output; select its own session",
            ));
        }
        if workspace.state != "ready" || !lease.holder.is_empty() {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Workspace still has an unreconciled writer",
            ));
        }
        let identity = workspace.identity.as_ref().ok_or_else(invalid)?;
        if !crate::policy::live_workspace_matches(&workspace.root, identity).map_err(|_| {
            validation_error(agentbaton_workspaces::validation::ValidationError::StaleEvidence)
        })? {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Registered workspace identity changed",
            ));
        }
        let raw: String = self
            .conn
            .query_row(
                "SELECT binding FROM attempts WHERE id=?1 AND task_id=?2",
                params![session.attempt_id, session.task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Session attempt is unavailable"))?;
        let binding: ExecutionBinding = decode(&raw)?;
        if binding.workspace_id != session.workspace_id {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Attempt workspace binding is inconsistent",
            ));
        }
        Ok((session, workspace, binding))
    }

    fn resolve_validation_selection(
        &self,
        selection: &ValidationSelection,
    ) -> Result<(ValidationPolicyContractView, Value)> {
        let count = selection
            .inputs
            .len()
            .checked_add(selection.outputs.len())
            .ok_or_else(invalid)?;
        let bytes = selection
            .inputs
            .iter()
            .chain(&selection.outputs)
            .try_fold(0u64, |sum, requirement| {
                sum.checked_add(requirement.max_bytes)
            })
            .ok_or_else(invalid)?;
        if count > 64 || bytes > 128 * 1024 * 1024 {
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
                        .map_err(validation_error)
                })
                .collect()
        };
        let inputs = bind(&selection.inputs)?;
        let outputs = bind(&selection.outputs)?;
        let subject = ValidationSubject {
            task_id: task.id.clone(),
            attempt_id: session.attempt_id.clone(),
            spec_revision: task.spec_revision,
            input_revision: binding.input_revision.clone(),
            version: VersionBinding::WorkspaceArtifacts {
                workspace_id: workspace.id.clone(),
                base_revision: workspace.base_revision.clone(),
            },
        };
        // Temporary labels construct and validate a proposal DTO, not authority.
        // Actual execution reconstructs the rule from its persisted owner policy.
        let rule = ApprovedRule::from_verified_policy(
            "proposal-preview".into(),
            "1".into(),
            subject,
            &inputs,
            &outputs,
            selection.analysis.clone(),
        )
        .map_err(validation_error)?;
        let refs = json!({"project_id":session.project_id,"project_name":self.get_project(&session.project_id)?.name,"project_revision":self.get_project(&session.project_id)?.revision,"task_goal":task.goal,"task_revision":task.revision,"session_revision":session.revision,"attempt_id":session.attempt_id,"workspace_id":workspace.id,"workspace_revision":workspace.revision,"workspace_identity":workspace.identity,"binding":binding});
        Ok((rule.contract_view(), refs))
    }

    pub fn propose_validation_policy(
        &mut self,
        selection: &ValidationSelection,
        idempotency_key: &str,
    ) -> Result<ApprovalProposal> {
        let (contract, refs) = self.resolve_validation_selection(selection)?;
        self.persist_proposal("validation", json!({"intent":selection,"refs":refs,"contract":contract,"contract_digest":contract.digest(),"scope":"artifact_integrity_only","warning":"Approves local file existence/hash/reproducibility metadata checks only. Does not execute tests, establish code correctness, accept a task, authorize integration or production release. No file contents are sent to a Provider."}), idempotency_key)
    }

    pub(crate) fn approve_validation_policy(
        &mut self,
        proposal: &ApprovalProposal,
    ) -> Result<Value> {
        let selection: ValidationSelection =
            serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_| invalid())?;
        let (contract, refs) = self.resolve_validation_selection(&selection)?;
        if proposal.snapshot["refs"] != refs
            || proposal.snapshot["contract"] != json!(contract)
            || proposal.snapshot["contract_digest"] != json!(contract.digest())
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Validation subject, root or artifact contract changed after proposal",
            ));
        }
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT document FROM validation_policies WHERE approval_id=?1",
                [&proposal.id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?;
        if let Some(existing) = existing {
            return decode(&existing);
        }
        let session = self.get_session(&selection.session_id)?;
        let policy = ValidationPolicy {
            id: Uuid::new_v4().to_string(),
            task_id: session.task_id,
            project_id: session.project_id,
            session_id: session.id,
            attempt_id: session.attempt_id,
            workspace_id: session.workspace_id,
            approval_id: proposal.id.clone(),
            revision: 1,
            revoked: false,
            scope: "artifact_integrity_only".into(),
            contract,
            created_at: now()?,
        };
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "INSERT INTO validation_policies VALUES(?1,?2,?3,?4,?5,?6,?7)",
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
        validation_event(
            &tx,
            &policy.id,
            policy.revision,
            "validation.policy_approved",
            json!({"policy":policy,"scope":"artifact_integrity_only"}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(policy))
    }

    pub fn get_validation_policy(&self, id: &str) -> Result<ValidationPolicy> {
        let raw: String = self
            .conn
            .query_row(
                "SELECT document FROM validation_policies WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(db_error)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Validation policy does not exist"))?;
        decode(&raw)
    }

    pub fn list_validation_policies(&self, task_id: &str) -> Result<Vec<ValidationPolicy>> {
        self.get(task_id)?;
        bounded_documents(&self.conn, "validation_policies", task_id)
    }

    pub fn run_artifact_validation(
        &mut self,
        policy_id: &str,
        expected_revision: u64,
        idempotency_key: &str,
    ) -> Result<ValidationRecord> {
        key(idempotency_key)?;
        let payload = encode(&json!({"policy_id":policy_id,"revision":expected_revision}))?;
        let old: Option<(String,String)> = self.conn.query_row("SELECT payload,document FROM validation_runs WHERE policy_id=?1 AND idempotency_key=?2",params![policy_id,idempotency_key],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(db_error)?;
        if let Some((original, record)) = old {
            if original != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Validation key was already used with another request",
                ));
            }
            return decode(&record);
        }
        let policy = self.get_validation_policy(policy_id)?;
        if policy.revoked {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Validation policy was revoked",
            ));
        }
        if policy.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Validation policy changed",
            ));
        }
        let (session, workspace, binding) = self.validation_workspace(&policy.session_id)?;
        if self.get(&policy.task_id)?.spec_revision != policy.contract.subject.spec_revision {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Task specification changed after validation policy approval",
            ));
        }

        if session.task_id != policy.task_id
            || session.attempt_id != policy.attempt_id
            || workspace.id != policy.workspace_id
            || binding.input_revision != policy.contract.subject.input_revision
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Validation attempt binding changed",
            ));
        }
        let bind = |contracts: &[agentbaton_workspaces::validation::ArtifactContractView]| -> Result<Vec<BoundArtifact>> {
            contracts.iter().map(|contract| {
                if contract.canonical_root != workspace.root { return Err(invalid()); }
                BoundArtifact::from_verified_contract(contract).map_err(validation_error)
            }).collect()
        };
        let inputs = bind(&policy.contract.inputs)?;
        let outputs = bind(&policy.contract.outputs)?;
        let rule = ApprovedRule::from_verified_policy(
            policy.approval_id.clone(),
            policy.revision.to_string(),
            policy.contract.subject.clone(),
            &inputs,
            &outputs,
            policy.contract.analysis.clone(),
        )
        .map_err(validation_error)?;
        if rule.contract_view() != policy.contract {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Persisted validation contract is inconsistent",
            ));
        }
        let result = validate_artifacts(&rule, inputs, outputs, policy.contract.analysis.clone())
            .map_err(validation_error)?;
        result.verify_current().map_err(validation_error)?;
        let view = result.view();
        let record = ValidationRecord {
            id: Uuid::new_v4().to_string(),
            policy_id: policy.id.clone(),
            task_id: policy.task_id.clone(),
            state: if view.passed {
                "integrity_passed"
            } else {
                "failed"
            }
            .into(),
            scope: "artifact_integrity_only".into(),
            correctness: "not_established".into(),
            task_acceptance: "not_established".into(),
            integration_authorized: false,
            evidence: view,
            created_at: now()?,
        };
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "INSERT INTO validation_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",
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
        validation_event(
            &tx,
            &record.id,
            1,
            "validation.artifact_integrity_recorded",
            json!({"record":record}),
            "deterministic_builtin_validator",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }

    pub fn list_validation_records(&self, task_id: &str) -> Result<Vec<ValidationRecord>> {
        self.get(task_id)?;
        bounded_documents(&self.conn, "validation_runs", task_id)
    }

    pub fn revoke_validation_policy(
        &mut self,
        policy_id: &str,
        expected_revision: u64,
    ) -> Result<ValidationPolicy> {
        let mut policy = self.get_validation_policy(policy_id)?;
        if policy.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Validation policy changed",
            ));
        }
        if policy.revoked {
            return Ok(policy);
        }
        policy.revoked = true;
        policy.revision += 1;
        let tx = self.conn.transaction().map_err(db_error)?;
        tx.execute(
            "UPDATE validation_policies SET document=?1 WHERE id=?2",
            params![encode(&policy)?, policy.id],
        )
        .map_err(db_error)?;
        validation_event(
            &tx,
            &policy.id,
            policy.revision,
            "validation.policy_revoked",
            json!({"policy_id":policy.id,"existing_evidence_retained":true}),
            "user_action",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(policy)
    }
}
