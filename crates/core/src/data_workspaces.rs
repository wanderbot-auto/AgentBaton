//! Native-owner-approved non-Git dataset copies and private input authority.
//! SQLite/journal mutations occur only on the daemon actor, never an I/O worker.
use crate::{
    registry::{ManagedWorkspace, Project},
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store},
    CoreError, Result,
};
use agentbaton_workspaces::{
    data_workspace::{DataLimits, DataWorkspaceContract, DatasetSnapshotView, DirectoryPin},
    validation::ArtifactRequirement,
    WorkspaceKind,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use uuid::Uuid;
const MAX_DOCUMENT: i64 = 4 * 1024 * 1024;
const MAX_RECORDS: i64 = 100;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DataWorkspaceSelection {
    pub project_id: String,
    pub files: Vec<ArtifactRequirement>,
    pub limits: DataLimits,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataWorkspacePreview {
    pub id: String,
    pub project_id: String,
    pub project_revision: u64,
    pub revision: u64,
    pub selection: DataWorkspaceSelection,
    pub state: String,
    pub snapshot: Option<DatasetSnapshotView>,
    pub error: Option<CoreError>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataWorkspaceCreation {
    pub id: String,
    pub preview_id: String,
    pub project_id: String,
    pub project_revision: u64,
    pub workspace_id: String,
    pub approval_id: String,
    pub revision: u64,
    pub state: String,
    pub dataset_revision: String,
    pub managed_root: DirectoryPin,
    pub destination: PathBuf,
    pub snapshot: DatasetSnapshotView,
    pub contract: Option<DataWorkspaceContract>,
    pub observation: Option<agentbaton_workspaces::data_workspace::DataCreationView>,
    pub cancel_requested: bool,
    #[serde(default)]
    pub io_completion: Option<String>,
    pub error: Option<CoreError>,
    pub created_at: String,
    pub updated_at: String,
    pub scope: String,
    pub filesystem_sandbox: bool,
    pub atomic_directory_freeze: bool,
}
fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST","Select a registered data project and bounded relative regular files; roots, verified flags and copy authority are server-owned")
}
fn validate_selection(selection: &DataWorkspaceSelection) -> Result<()> {
    let limits = &selection.limits;
    if selection.project_id.is_empty()
        || selection.project_id.len() > 128
        || selection.files.is_empty()
        || selection.files.len() > 64
        || limits.max_files == 0
        || limits.max_files > 64
        || selection.files.len() > limits.max_files as usize
        || limits.max_file_bytes == 0
        || limits.max_file_bytes > 64 * 1024 * 1024
        || limits.max_total_bytes == 0
        || limits.max_total_bytes > 256 * 1024 * 1024
    {
        return Err(invalid());
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut paths = Vec::new();
    for file in &selection.files {
        if file.artifact_id.is_empty()
            || file.artifact_id.len() > 256
            || !file
                .artifact_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._:-/".contains(&b))
            || !ids.insert(file.artifact_id.as_str())
            || file.max_bytes == 0
            || file.max_bytes > limits.max_file_bytes
            || file.expected_sha256.as_ref().is_some_and(|hash| {
                hash.len() != 64
                    || !hash
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
        {
            return Err(invalid());
        }
        let path = file
            .relative_path
            .to_str()
            .filter(|p| !p.is_empty() && p.len() <= 4096 && !p.chars().any(char::is_control))
            .ok_or_else(invalid)?;
        if file.relative_path.components().count() > 16
            || !file
                .relative_path
                .components()
                .all(|p| matches!(p, std::path::Component::Normal(_)))
            || file
                .relative_path
                .components()
                .collect::<PathBuf>()
                .as_os_str()
                != file.relative_path.as_os_str()
        {
            return Err(invalid());
        }
        paths.push(path.to_lowercase());
    }
    if paths.iter().enumerate().any(|(i, a)| {
        paths.iter().skip(i + 1).any(|b| {
            a == b || a.starts_with(&(b.clone() + "/")) || b.starts_with(&(a.clone() + "/"))
        })
    }) {
        return Err(invalid());
    }
    Ok(())
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Data project, approved file snapshot or managed root changed; review a fresh snapshot",
    )
}
fn limit() -> CoreError {
    CoreError::new("RESOURCE_LIMIT","Data workspace record exceeds its bounded byte/count budget; retained local history was not truncated")
}
fn key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 128 {
        Err(invalid())
    } else {
        Ok(())
    }
}
fn event(db: &Connection, id: &str, revision: u64, kind: &str, payload: Value) -> Result<()> {
    let e = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: if kind == "approval.confirmed" {
            "local_owner_confirmation"
        } else {
            "data_workspace_core"
        }
        .into(),
    };
    db.execute("INSERT INTO events(document) VALUES(?1)", [encode(&e)?])
        .map_err(db_error)?;
    Ok(())
}
fn sql(table: &str) -> Result<&'static str> {
    match table {"data_workspace_previews"=>Ok("SELECT document FROM data_workspace_previews WHERE id=?1 AND octet_length(document)<=?2"),"data_workspace_creations"=>Ok("SELECT document FROM data_workspace_creations WHERE id=?1 AND octet_length(document)<=?2"),_=>Err(invalid())}
}
fn read<T: for<'a> Deserialize<'a>>(db: &Connection, table: &str, id: &str) -> Result<T> {
    let q = sql(table)?;
    let size_sql = match table {
        "data_workspace_previews" => {
            "SELECT octet_length(document) FROM data_workspace_previews WHERE id=?1"
        }
        "data_workspace_creations" => {
            "SELECT octet_length(document) FROM data_workspace_creations WHERE id=?1"
        }
        _ => return Err(invalid()),
    };
    let size: Option<i64> = db
        .query_row(size_sql, [id], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    if size.is_some_and(|size| !(0..=MAX_DOCUMENT).contains(&size)) {
        return Err(limit());
    }
    let document: Option<String> = db
        .query_row(q, params![id, MAX_DOCUMENT], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    let document = document.ok_or_else(|| {
        CoreError::new(
            "NOT_FOUND",
            "Data workspace record unavailable or exceeds byte bound",
        )
    })?;
    decode(&document)
}
fn list<T: for<'a> Deserialize<'a>>(db: &Connection, table: &str, project: &str) -> Result<Vec<T>> {
    let query=match table{"data_workspace_previews"=>"SELECT document,octet_length(document) FROM data_workspace_previews WHERE project_id=?1 AND octet_length(document)<=4194304 ORDER BY rowid DESC", "data_workspace_creations"=>"SELECT document,octet_length(document) FROM data_workspace_creations WHERE project_id=?1 AND octet_length(document)<=4194304 ORDER BY rowid DESC",_=>return Err(invalid())};
    let budget=match table{"data_workspace_previews"=>"SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM data_workspace_previews WHERE project_id=?1",_=>"SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM data_workspace_creations WHERE project_id=?1"};
    let (n, bytes): (i64, i64) = db
        .query_row(budget, [project], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(db_error)?;
    if n > MAX_RECORDS || bytes > MAX_DOCUMENT {
        return Err(limit());
    }
    let mut s = db.prepare(query).map_err(db_error)?;
    let rows = s
        .query_map([project], |r| r.get::<_, String>(0))
        .map_err(db_error)?;
    rows.map(|r| decode(&r.map_err(db_error)?)).collect()
}
pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version < 10 {
        let tx = db.transaction().map_err(db_error)?;
        tx.execute_batch("CREATE TABLE data_workspace_previews(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),state TEXT NOT NULL,document TEXT NOT NULL,idempotency_key TEXT NOT NULL,payload TEXT NOT NULL,UNIQUE(project_id,idempotency_key)); CREATE TABLE data_workspace_creations(id TEXT PRIMARY KEY,preview_id TEXT NOT NULL UNIQUE REFERENCES data_workspace_previews(id),project_id TEXT NOT NULL REFERENCES projects(id),workspace_id TEXT NOT NULL UNIQUE REFERENCES managed_workspaces(id),approval_id TEXT NOT NULL UNIQUE REFERENCES approval_proposals(id),state TEXT NOT NULL,document TEXT NOT NULL); CREATE TABLE data_workspace_receipts(creation_id TEXT PRIMARY KEY REFERENCES data_workspace_creations(id),owner_sequence INTEGER NOT NULL,prepared_sequence INTEGER NOT NULL,finished_sequence INTEGER); PRAGMA user_version=10;").map_err(db_error)?;
        tx.commit().map_err(db_error)?;
    }
    Ok(())
}
impl Store {
    pub fn get_data_workspace_preview(&self, id: &str) -> Result<DataWorkspacePreview> {
        read(&self.conn, "data_workspace_previews", id)
    }
    pub fn list_data_workspace_previews(&self, project: &str) -> Result<Vec<DataWorkspacePreview>> {
        list(&self.conn, "data_workspace_previews", project)
    }
    pub fn get_data_workspace_creation(&self, id: &str) -> Result<DataWorkspaceCreation> {
        read(&self.conn, "data_workspace_creations", id)
    }
    pub fn list_data_workspace_creations(
        &self,
        project: &str,
    ) -> Result<Vec<DataWorkspaceCreation>> {
        list(&self.conn, "data_workspace_creations", project)
    }
    pub(crate) fn prepare_data_preview(
        &mut self,
        selection: &DataWorkspaceSelection,
        revision: u64,
        idempotency: &str,
    ) -> Result<DataWorkspacePreview> {
        key(idempotency)?;
        validate_selection(selection)?;
        let payload = encode(&json!({"selection":selection,"expected_revision":revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = preview_replay_from(&tx, &selection.project_id, idempotency, &payload)? {
            return Ok(old);
        }
        let project: Project = decode(
            &tx.query_row(
                "SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=4194304",
                [&selection.project_id],
                |r| r.get::<_, String>(0),
            )
            .map_err(db_error)?,
        )?;
        if project.archived
            || project.revision != revision
            || !matches!(project.identity.kind, WorkspaceKind::Directory)
        {
            return Err(changed());
        }
        let timestamp = now()?;
        let preview = DataWorkspacePreview {
            id: Uuid::new_v4().to_string(),
            project_id: project.id,
            project_revision: revision,
            revision: 1,
            selection: selection.clone(),
            state: "checking".into(),
            snapshot: None,
            error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let reserved = encode(&preview)?
            .len()
            .checked_mul(5)
            .and_then(|n| n.checked_add(65_536))
            .ok_or_else(limit)?;
        history_budget(
            &tx,
            "data_workspace_previews",
            &preview.project_id,
            reserved,
            None,
        )?;
        tx.execute(
            "INSERT INTO data_workspace_previews VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                preview.id,
                preview.project_id,
                preview.state,
                encode(&preview)?,
                idempotency,
                payload
            ],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &preview.id,
            1,
            "data_workspace.preview_prepared",
            json!({"project_id":preview.project_id,"source_write":false,"copy_authorized":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(preview)
    }
}
impl Store {
    pub(crate) fn finish_data_preview(
        &mut self,
        id: &str,
        snapshot: Result<DatasetSnapshotView>,
        cancelled: bool,
    ) -> Result<DataWorkspacePreview> {
        let mut record = self.get_data_workspace_preview(id)?;
        if !matches!(record.state.as_str(), "checking" | "cancel_requested") {
            return Ok(record);
        }
        let project = self.get_project(&record.project_id)?;
        let cancelled = cancelled || record.state == "cancel_requested";
        let snapshot = if project.archived || project.revision != record.project_revision {
            Err(changed())
        } else {
            snapshot
        };
        match snapshot {
            Ok(view) if !cancelled => {
                record.snapshot = Some(view);
                record.state = "verified".into();
                record.error = None
            }
            Ok(_) => record.state = "cancelled".into(),
            Err(error) => {
                record.state = if cancelled { "cancelled" } else { "failed" }.into();
                record.error = Some(error)
            }
        }
        record.revision += 1;
        record.updated_at = now()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if history_budget(
            &tx,
            "data_workspace_previews",
            &record.project_id,
            encode(&record)?.len(),
            Some(id),
        )
        .is_err()
        {
            record.snapshot = None;
            record.state = "failed".into();
            record.error = Some(limit());
        }
        history_budget(
            &tx,
            "data_workspace_previews",
            &record.project_id,
            encode(&record)?.len(),
            Some(id),
        )?;
        tx.execute(
            "UPDATE data_workspace_previews SET state=?1,document=?2 WHERE id=?3",
            params![record.state, encode(&record)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            record.revision,
            "data_workspace.preview_finished",
            json!({"state":record.state,"copy_authorized":false}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }
    pub fn propose_data_workspace(
        &mut self,
        preview_id: &str,
        revision: u64,
        key_value: &str,
        managed: &Path,
    ) -> Result<ApprovalProposal> {
        key(key_value)?;
        if let Some(old) =
            self.data_workspace_proposal_replay(preview_id, revision, key_value, managed)?
        {
            return Ok(old);
        }
        let preview = self.get_data_workspace_preview(preview_id)?;
        if self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM data_workspace_creations WHERE preview_id=?1)",
                [preview_id],
                |r| r.get::<_, bool>(0),
            )
            .map_err(db_error)?
        {
            return Err(CoreError::new("OUTCOME_UNKNOWN", "This preview already has a copy intent; inspect that existing creation instead of replaying it with a new key"));
        }
        let project = self.get_project(&preview.project_id)?;
        if preview.revision != revision
            || preview.state != "verified"
            || project.revision != preview.project_revision
            || project.archived
        {
            return Err(changed());
        }
        let snapshot = preview.snapshot.as_ref().ok_or_else(changed)?;
        let managed =
            agentbaton_workspaces::data_workspace::ApprovedManagedRoot::from_verified_host(managed)
                .map_err(data_error)?
                .view();
        let creation_id = Uuid::new_v4().to_string();
        let workspace_id = Uuid::new_v4().to_string();
        let destination = managed.canonical_root.join(format!("data-{creation_id}"));
        if destination.as_os_str().len() > 4096
            || snapshot.files.iter().any(|file| {
                destination
                    .join(&file.destination_relative_path)
                    .as_os_str()
                    .len()
                    > 4096
            })
        {
            return Err(limit());
        }

        let intent = json!({"preview_id":preview_id,"preview_revision":revision,"project_id":project.id,"project_revision":project.revision,"creation_id":creation_id,"workspace_id":workspace_id,"managed_root":managed,"destination":destination});
        let owner = json!({"intent":intent,"project_name":project.name,"snapshot":snapshot,"scope":"explicit_selected_dataset_copy_only","guarantees":{"filesystem_sandbox":false,"atomic_directory_freeze":false,"continuous_input_immutability":false,"credential_detection":"host_selection_required_not_dlp"},"warning":"Copies only the displayed exact dataset files to the private host-managed destination. No scripts, Git initialization, model calls or original source writes. Hashes are observed versions; this is not an OS sandbox or whole-directory freeze."});
        self.persist_proposal("data_workspace", owner, key_value)
    }
    pub(crate) fn finalize_data_workspace_proposal(
        &mut self,
        id: &str,
        revision: u64,
    ) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let(document,result):(String,Option<String>)=tx.query_row("SELECT document,result FROM approval_proposals WHERE id=?1 AND octet_length(document)<=4194304 AND (result IS NULL OR octet_length(result)<=4194304)",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
        if let Some(result) = result {
            return decode(&result);
        }
        let mut proposal: ApprovalProposal = decode(&document)?;
        let now_ms = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64;
        if proposal.kind != "data_workspace"
            || proposal.revision != revision
            || proposal.expires_at_ms <= now_ms
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Data copy owner proposal changed or expired",
            ));
        }
        let intent = &proposal.snapshot["intent"];
        let get = |name: &str| intent[name].as_str().ok_or_else(invalid);
        let mut preview: DataWorkspacePreview =
            read(&tx, "data_workspace_previews", get("preview_id")?)?;
        let project: Project = decode(
            &tx.query_row(
                "SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=4194304",
                [&preview.project_id],
                |r| r.get::<_, String>(0),
            )
            .map_err(db_error)?,
        )?;
        if project.archived
            || project.revision != preview.project_revision
            || intent["project_id"] != project.id
            || intent["project_revision"] != project.revision
            || intent["preview_revision"] != preview.revision
            || preview.state != "verified"
            || proposal.snapshot["snapshot"]
                != json!(preview.snapshot.as_ref().ok_or_else(changed)?)
        {
            return Err(changed());
        }
        let snapshot = preview.snapshot.take().ok_or_else(changed)?;
        let managed_root: DirectoryPin =
            serde_json::from_value(intent["managed_root"].clone()).map_err(|_| invalid())?;
        let creation_id = get("creation_id")?.to_owned();
        let workspace_id = get("workspace_id")?.to_owned();
        if Uuid::parse_str(&creation_id).is_err() || Uuid::parse_str(&workspace_id).is_err() {
            return Err(invalid());
        }
        let destination = managed_root
            .canonical_root
            .join(format!("data-{creation_id}"));
        if json!(destination) != intent["destination"] {
            return Err(changed());
        }
        let timestamp = now()?;
        let workspace = ManagedWorkspace {
            id: workspace_id.clone(),
            project_id: project.id.clone(),
            identity: None,
            root: destination.clone(),
            base_revision: snapshot.dataset_revision.clone(),
            created_at: timestamp.clone(),
            state: "creating".into(),
            revision: 1,
        };
        let record = DataWorkspaceCreation {
            id: creation_id.clone(),
            preview_id: preview.id,
            project_id: project.id,
            project_revision: project.revision,
            workspace_id,
            approval_id: id.into(),
            revision: 1,
            state: "creating".into(),
            dataset_revision: snapshot.dataset_revision.clone(),
            managed_root,
            destination,
            snapshot,
            contract: None,
            observation: None,
            cancel_requested: false,
            io_completion: None,
            error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            scope: "explicit_selected_dataset_copy_only".into(),
            filesystem_sandbox: false,
            atomic_directory_freeze: false,
        };
        if record.destination.as_os_str().len() > 4096
            || record.snapshot.files.iter().any(|file| {
                record
                    .destination
                    .join(&file.destination_relative_path)
                    .as_os_str()
                    .len()
                    > 4096
            })
        {
            return Err(limit());
        }
        let reservation = creation_record_reservation(&record)?;
        history_budget(
            &tx,
            "data_workspace_creations",
            &record.project_id,
            reservation,
            None,
        )?;
        tx.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,payload,root) VALUES(?1,?2,?3,'creating',?4,?5,?6)",params![workspace.id,workspace.project_id,encode(&workspace)?,format!("data-proposal:{id}"),encode(&proposal.snapshot)?,workspace.root.to_string_lossy()]).map_err(db_error)?;
        tx.execute(
            "INSERT INTO data_workspace_creations VALUES(?1,?2,?3,?4,?5,'creating',?6)",
            params![
                record.id,
                record.preview_id,
                record.project_id,
                record.workspace_id,
                id,
                encode(&record)?
            ],
        )
        .map_err(db_error)?;
        proposal.consumed = true;
        proposal.revision += 1;
        let result = json!(record);
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
            json!({"kind":"data_workspace","creation_id":creation_id,"consumed":true,"snapshot_digest":digest(&proposal.snapshot)?}),
        )?;
        let owner_sequence = tx.last_insert_rowid();
        event(
            &tx,
            &record.id,
            1,
            "data_workspace.copy_prepared",
            json!({"workspace_id":workspace.id,"dataset_revision":record.dataset_revision,"copy_started":false,"approval_id":record.approval_id,"preview_id":record.preview_id,"scope":record.scope,"snapshot_digest":digest(&record.snapshot)?}),
        )?;
        let prepared_sequence = tx.last_insert_rowid();
        tx.execute("INSERT INTO data_workspace_receipts(creation_id,owner_sequence,prepared_sequence) VALUES(?1,?2,?3)",params![record.id,owner_sequence,prepared_sequence]).map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(result)
    }
    pub(crate) fn recover_data_workspace_operations(&mut self) -> Result<()> {
        for (table, states) in [
            ("data_workspace_previews", "checking','cancel_requested"),
            ("data_workspace_creations", "creating','cancel_requested"),
        ] {
            let q = format!("SELECT id FROM {table} WHERE state IN('{states}') LIMIT 100");
            loop {
                let ids: Vec<String> = self
                    .conn
                    .prepare(&q)
                    .map_err(db_error)?
                    .query_map([], |r| r.get(0))
                    .map_err(db_error)?
                    .map(|r| r.map_err(db_error))
                    .collect::<Result<_>>()?;
                if ids.is_empty() {
                    break;
                }
                let tx = self.conn.transaction().map_err(db_error)?;
                for id in ids {
                    if table == "data_workspace_previews" {
                        let mut r: DataWorkspacePreview = read(&tx, table, &id)?;
                        r.state = "outcome_unknown".into();
                        r.revision += 1;
                        r.updated_at = now()?;
                        tx.execute(
                            "UPDATE data_workspace_previews SET state=?1,document=?2 WHERE id=?3",
                            params![r.state, encode(&r)?, id],
                        )
                        .map_err(db_error)?;
                        event(
                            &tx,
                            &id,
                            r.revision,
                            "data_workspace.preview_interrupted",
                            json!({"automatic_replay":false}),
                        )?;
                    } else {
                        let mut r: DataWorkspaceCreation = read(&tx, table, &id)?;
                        r.io_completion = Some("interrupted_no_join_receipt".into());
                        r.state = "outcome_unknown".into();
                        r.revision += 1;
                        r.updated_at = now()?;
                        r.error=Some(CoreError{side_effect_state:"unknown".into(),..CoreError::new("OUTCOME_UNKNOWN","Interrupted copy has no trusted completed root; partial files remain and are not replayed")});
                        tx.execute(
                            "UPDATE data_workspace_creations SET state=?1,document=?2 WHERE id=?3",
                            params![r.state, encode(&r)?, id],
                        )
                        .map_err(db_error)?;
                        event(
                            &tx,
                            &id,
                            r.revision,
                            "data_workspace.copy_interrupted",
                            json!({"automatic_replay":false,"partial_deleted":false}),
                        )?;
                    }
                }
                tx.commit().map_err(db_error)?;
            }
        }
        Ok(())
    }
}
pub(crate) fn data_error(
    error: agentbaton_workspaces::data_workspace::DataWorkspaceError,
) -> CoreError {
    use agentbaton_workspaces::data_workspace::DataWorkspaceError::*;
    match error {
        ResourceLimit => limit(),
        ExcludedSource => CoreError::new(
            "PERMISSION_DENIED",
            "Data source traverses the known private runtime; no dataset hash/copy authorized",
        ),
        CancelRequested => CoreError::new(
            "CANCEL_REQUESTED",
            "Data I/O cancellation was observed; partial files are retained",
        ),
        DeadlineExceeded => CoreError::new(
            "DEADLINE_EXCEEDED",
            "The data I/O deadline was observed; partial files are retained",
        ),
        InvalidSelection | UnauthorizedPath => invalid(),
        _ => changed(),
    }
}
impl Store {
    pub(crate) fn finish_data_copy(
        &mut self,
        id: &str,
        result: &Result<agentbaton_workspaces::data_workspace::DataWorkspaceCreation>,
        worker_joined: bool,
    ) -> Result<DataWorkspaceCreation> {
        let mut record = self.get_data_workspace_creation(id)?;
        if !matches!(record.state.as_str(), "creating" | "cancel_requested") {
            return Ok(record);
        }
        let mut workspace:ManagedWorkspace=decode(&self.conn.query_row("SELECT document FROM managed_workspaces WHERE id=?1 AND octet_length(document)<=4194304",[&record.workspace_id],|r|r.get::<_,String>(0)).map_err(db_error)?)?;
        record.io_completion = Some(
            if worker_joined {
                "worker_joined"
            } else {
                "not_scheduled"
            }
            .into(),
        );
        match result {
            Ok(created) => {
                let view = created.view();
                record.state = match view.state {
                    agentbaton_workspaces::data_workspace::CreationState::Ready => "ready",
                    agentbaton_workspaces::data_workspace::CreationState::Failed => "failed",
                    _ => "outcome_unknown",
                }
                .into();
                record.error = view.error.map(|error| {
                    let mut error = data_error(error);
                    error.side_effect_state = if view.state
                        == agentbaton_workspaces::data_workspace::CreationState::OutcomeUnknown
                    {
                        "unknown"
                    } else {
                        "confirmed"
                    }
                    .into();
                    error
                });
                if worker_joined && view.state==agentbaton_workspaces::data_workspace::CreationState::Failed && view.error==Some(agentbaton_workspaces::data_workspace::DataWorkspaceError::CancelRequested) {
                    record.state="cancelled".into();record.cancel_requested=true;
                }
                record.observation = Some(view);
                if let Some(verified) = created.verified() {
                    let contract = verified.contract_view();
                    if contract.creation_id != record.id
                        || contract.snapshot != record.snapshot
                        || contract.managed_root != record.managed_root
                        || contract.destination.canonical_root != record.destination
                    {
                        return Err(changed());
                    }
                    record.contract = Some(contract.clone());
                    workspace.identity = Some(agentbaton_workspaces::WorkspaceIdentity {
                        canonical_root: contract.destination.canonical_root,
                        device: contract.destination.device,
                        inode: contract.destination.inode,
                        kind: WorkspaceKind::Directory,
                    });
                    workspace.state = "ready".into();
                } else {
                    workspace.state = "outcome_unknown".into();
                }
            }
            Err(error) => {
                record.state =
                    if error.side_effect_state == "unknown" || error.code == "OUTCOME_UNKNOWN" {
                        "outcome_unknown"
                    } else {
                        "failed"
                    }
                    .into();
                let mut error = error.clone();
                if worker_joined && error.code == "CANCEL_REQUESTED" {
                    record.state = "cancelled".into();
                    record.cancel_requested = true;
                    error.side_effect_state = "confirmed".into();
                }
                record.error = Some(error);
                workspace.state = "outcome_unknown".into();
            }
        }
        record.revision += 1;
        workspace.revision += 1;
        record.updated_at = now()?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        history_budget(
            &tx,
            "data_workspace_creations",
            &record.project_id,
            encode(&record)?.len(),
            Some(id),
        )?;
        tx.execute(
            "UPDATE managed_workspaces SET document=?1,state=?2 WHERE id=?3",
            params![encode(&workspace)?, workspace.state, workspace.id],
        )
        .map_err(db_error)?;
        tx.execute(
            "UPDATE data_workspace_creations SET document=?1,state=?2 WHERE id=?3",
            params![encode(&record)?, record.state, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            record.revision,
            "data_workspace.copy_finished",
            json!({"state":record.state,"workspace_id":workspace.id,"cancel_requested":record.cancel_requested,"automatic_retry":false,"source_write":false,"approval_id":record.approval_id,"dataset_revision":record.dataset_revision,"contract_digest":record.contract.as_ref().map(|c|&c.contract_digest),"io_completion":record.io_completion}),
        )?;
        let finished_sequence = tx.last_insert_rowid();
        tx.execute(
            "UPDATE data_workspace_receipts SET finished_sequence=?1 WHERE creation_id=?2",
            params![finished_sequence, record.id],
        )
        .map_err(db_error)?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }
    pub fn cancel_data_workspace_preview(
        &mut self,
        id: &str,
        revision: u64,
        key_value: &str,
    ) -> Result<DataWorkspacePreview> {
        key(key_value)?;
        let scope = format!("data_preview_cancel:{id}");
        let payload = encode(&json!({"id":id,"revision":revision}))?;
        if let Some(record) = cancel_replay(&self.conn, &scope, key_value, &payload)? {
            return Ok(record);
        }
        let mut record = self.get_data_workspace_preview(id)?;
        if revision == 0 || revision > record.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data preview revision is unavailable",
            ));
        }
        let changed = record.state == "checking";
        if changed {
            record.state = "cancel_requested".into();
            record.revision += 1;
            record.updated_at = now()?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if changed {
            tx.execute(
                "UPDATE data_workspace_previews SET document=?1,state=?2 WHERE id=?3",
                params![encode(&record)?, record.state, id],
            )
            .map_err(db_error)?;
            event(
                &tx,
                id,
                record.revision,
                "data_workspace.preview_cancel_requested",
                json!({"confirmed_stop":false}),
            )?;
        }
        remember_cancel(&tx, &scope, key_value, &payload, &record)?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }
    pub fn cancel_data_workspace_creation(
        &mut self,
        id: &str,
        revision: u64,
        key_value: &str,
    ) -> Result<DataWorkspaceCreation> {
        key(key_value)?;
        let scope = format!("data_copy_cancel:{id}");
        let payload = encode(&json!({"id":id,"revision":revision}))?;
        if let Some(record) = cancel_replay(&self.conn, &scope, key_value, &payload)? {
            return Ok(record);
        }
        let mut record = self.get_data_workspace_creation(id)?;
        if revision == 0 || revision > record.revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Data creation revision is unavailable",
            ));
        }
        let changed = matches!(record.state.as_str(), "creating" | "cancel_requested")
            && !record.cancel_requested;
        if changed {
            record.cancel_requested = true;
            record.state = "cancel_requested".into();
            record.revision += 1;
            record.updated_at = now()?;
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if changed {
            tx.execute(
                "UPDATE data_workspace_creations SET document=?1,state=?2 WHERE id=?3",
                params![encode(&record)?, record.state, id],
            )
            .map_err(db_error)?;
            event(
                &tx,
                id,
                record.revision,
                "data_workspace.copy_cancel_requested",
                json!({"confirmed_stop":false,"partial_deleted":false}),
            )?;
        }
        remember_cancel(&tx, &scope, key_value, &payload, &record)?;
        tx.commit().map_err(db_error)?;
        Ok(record)
    }
}

/// No Deserialize/Clone authority. Built only from the native-confirmed DB
/// creation and a fresh worker restore; consumed by one exact deferred request.
pub(crate) struct DataInputContext {
    creation: DataWorkspaceCreation,
    workspace: ManagedWorkspace,
    project_revision: u64,
    record_hash: String,
}
pub(crate) struct DataInputWitness {
    pub(crate) graph: Option<crate::goal_store::GraphReadWitness>,
    request_hash: String,
    deadline: std::time::Instant,
    inputs: Vec<VerifiedDataInput>,
}
struct VerifiedDataInput {
    context: DataInputContext,
    verified: agentbaton_workspaces::data_workspace::VerifiedDataWorkspace,
}
fn digest<T: Serialize>(value: &T) -> Result<String> {
    Ok(agentbaton_workspaces::validation::sha256(
        encode(value)?.as_bytes(),
    ))
}
fn workspace_record(db: &Connection, id: &str) -> Result<ManagedWorkspace> {
    let raw:Option<String>=db.query_row("SELECT document FROM managed_workspaces WHERE id=?1 AND octet_length(document)<=4194304",[id],|r|r.get(0)).optional().map_err(db_error)?;
    decode(&raw.ok_or_else(changed)?)
}
fn project_record(db: &Connection, id: &str) -> Result<Project> {
    let raw: Option<String> = db
        .query_row(
            "SELECT document FROM projects WHERE id=?1 AND octet_length(document)<=4194304",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(changed)?)
}
fn creation_for_workspace(db: &Connection, id: &str) -> Result<DataWorkspaceCreation> {
    let id: Option<String> = db
        .query_row(
            "SELECT id FROM data_workspace_creations WHERE workspace_id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    read(
        db,
        "data_workspace_creations",
        &id.ok_or_else(|| {
            CoreError::new(
                "AUTH_REQUIRED",
                "Directory workspaces require a native-confirmed actual data copy",
            )
        })?,
    )
}
fn confirmed_copy(db: &Connection, workspace: &ManagedWorkspace) -> Result<DataWorkspaceCreation> {
    let c = creation_for_workspace(db, &workspace.id)?;
    if c.workspace_id != workspace.id
        || c.project_id != workspace.project_id
        || c.state != "ready"
        || workspace.state != "ready"
        || c.dataset_revision != workspace.base_revision
        || c.scope != "explicit_selected_dataset_copy_only"
        || c.filesystem_sandbox
        || c.atomic_directory_freeze
    {
        return Err(changed());
    }
    let contract = c.contract.as_ref().ok_or_else(changed)?;
    let pin = workspace.identity.as_ref().ok_or_else(changed)?;
    if !matches!(pin.kind, WorkspaceKind::Directory)
        || pin.canonical_root != c.destination
        || workspace.root != c.destination
        || contract.destination.canonical_root != c.destination
        || contract.destination.device != pin.device
        || contract.destination.inode != pin.inode
        || contract.creation_id != c.id
        || contract.managed_root != c.managed_root
        || contract.snapshot != c.snapshot
    {
        return Err(changed());
    }
    confirmed_intent(db, &c)?;
    let (_, _, finished) = receipt_sequences(db, &c.id)?;
    let ready = required_receipt(
        db,
        finished.ok_or_else(changed)?,
        &c.id,
        "data_workspace.copy_finished",
    )?;
    if ready.revision != c.revision
        || ready.payload["state"] != "ready"
        || ready.payload["workspace_id"] != c.workspace_id
        || ready.payload["approval_id"] != c.approval_id
        || ready.payload["dataset_revision"] != c.dataset_revision
        || ready.payload["contract_digest"] != contract.contract_digest
    {
        return Err(CoreError::new(
            "AUTH_REQUIRED",
            "Ready input lacks its actual completed-copy journal commitment",
        ));
    }
    Ok(c)
}
fn confirmed_intent(db: &Connection, c: &DataWorkspaceCreation) -> Result<()> {
    let row:Option<(String,String)>=db.query_row("SELECT document,result FROM approval_proposals WHERE id=?1 AND octet_length(document)<=4194304 AND octet_length(result)<=4194304",[&c.approval_id],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    let (document, result) = row.ok_or_else(changed)?;
    let proposal: ApprovalProposal = decode(&document)?;
    let result: Value = decode(&result)?;
    if proposal.id != c.approval_id
        || proposal.kind != "data_workspace"
        || !proposal.consumed
        || proposal.revision < 2
        || result["id"] != c.id
        || result["workspace_id"] != c.workspace_id
        || result["project_id"] != c.project_id
        || result["approval_id"] != c.approval_id
        || result["dataset_revision"] != c.dataset_revision
        || proposal.snapshot["snapshot"] != json!(c.snapshot)
        || proposal.snapshot["intent"]["creation_id"] != c.id
        || proposal.snapshot["intent"]["preview_id"] != c.preview_id
        || proposal.snapshot["intent"]["project_id"] != c.project_id
        || proposal.snapshot["intent"]["project_revision"] != c.project_revision
        || proposal.snapshot["intent"]["workspace_id"] != c.workspace_id
        || proposal.snapshot["intent"]["managed_root"] != json!(c.managed_root)
        || proposal.snapshot["intent"]["destination"] != json!(c.destination)
    {
        return Err(CoreError::new("AUTH_REQUIRED","Data copy lacks the matching native owner confirmation and immutable creation contract"));
    }
    let (owner_sequence, prepared_sequence, _) = receipt_sequences(db, &c.id)?;
    let owner = required_receipt(db, owner_sequence, &c.approval_id, "approval.confirmed")?;
    let prepared = required_receipt(db, prepared_sequence, &c.id, "data_workspace.copy_prepared")?;
    if owner.source != "local_owner_confirmation"
        || owner.revision != proposal.revision
        || owner.payload["kind"] != "data_workspace"
        || owner.payload["creation_id"] != c.id
        || owner.payload["consumed"] != true
        || owner.payload["snapshot_digest"] != digest(&proposal.snapshot)?
        || prepared.revision != 1
        || prepared.payload["copy_started"] != false
        || prepared.payload["workspace_id"] != c.workspace_id
        || prepared.payload["approval_id"] != c.approval_id
        || prepared.payload["preview_id"] != c.preview_id
        || prepared.payload["dataset_revision"] != c.dataset_revision
        || prepared.payload["scope"] != c.scope
        || prepared.payload["snapshot_digest"] != digest(&c.snapshot)?
    {
        return Err(CoreError::new(
            "AUTH_REQUIRED",
            "Required data-copy owner/intent journal receipt is missing or inconsistent",
        ));
    }
    Ok(())
}
impl DataInputWitness {
    pub(crate) fn restore(
        contexts: Vec<DataInputContext>,
        request_hash: String,
        deadline: std::time::Instant,
    ) -> Result<Self> {
        use agentbaton_workspaces::data_workspace::{
            restore_data_workspace, ApprovedManagedRoot, ApprovedRestoreContract,
        };
        let mut inputs = Vec::new();
        for context in contexts {
            if std::time::Instant::now() >= deadline {
                return Err(CoreError::new(
                    "DEADLINE_EXCEEDED",
                    "Data input request expired before the next bounded restore",
                ));
            }
            let c = &context.creation;
            let restore = ApprovedRestoreContract::from_verified_host(
                c.contract.clone().ok_or_else(changed)?,
            )
            .map_err(data_error)?;
            let managed = ApprovedManagedRoot::from_verified_host(&c.managed_root.canonical_root)
                .map_err(data_error)?;
            if managed.view() != c.managed_root {
                return Err(changed());
            }
            let verified = restore_data_workspace(&restore, &managed).map_err(data_error)?;
            inputs.push(VerifiedDataInput { context, verified });
        }
        Ok(Self {
            graph: None,
            request_hash,
            deadline,
            inputs,
        })
    }
    pub(crate) fn for_request(&self, raw: &str) -> bool {
        std::time::Instant::now() < self.deadline
            && self.request_hash == agentbaton_workspaces::validation::sha256(raw.as_bytes())
    }
    pub(crate) fn selected_input_hash(
        &self,
        db: &Connection,
        workspace: &ManagedWorkspace,
        requirement: &ArtifactRequirement,
    ) -> Result<String> {
        self.authorize(db, workspace)?;
        let input = self
            .inputs
            .iter()
            .find(|i| i.context.workspace.id == workspace.id)
            .ok_or_else(changed)?;
        let contract = input
            .context
            .creation
            .contract
            .as_ref()
            .ok_or_else(changed)?;
        let file=contract.files.iter().find(|f|f.input.destination_relative_path==requirement.relative_path).ok_or_else(||CoreError::new("CAPABILITY_UNSUPPORTED","Goal data inputs currently select only the actual native-confirmed dataset copy, not arbitrary later output files"))?;
        if file.input.bytes > requirement.max_bytes
            || requirement
                .expected_sha256
                .as_ref()
                .is_some_and(|h| h != &file.input.sha256)
        {
            return Err(changed());
        }
        Ok(file.input.sha256.clone())
    }
    pub(crate) fn authorize(&self, db: &Connection, workspace: &ManagedWorkspace) -> Result<()> {
        if std::time::Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Private data observation expired before admission",
            ));
        }
        let input = self
            .inputs
            .iter()
            .find(|input| input.context.workspace.id == workspace.id)
            .ok_or_else(|| {
                CoreError::new(
                    "WORKSPACE_BUSY",
                    "A fresh private data input check is required for this request",
                )
            })?;
        let project = project_record(db, &workspace.project_id)?;
        if project.archived
            || project.revision != input.context.project_revision
            || workspace.revision != input.context.workspace.revision
            || workspace.root != input.context.workspace.root
            || workspace.base_revision != input.context.workspace.base_revision
            || workspace.identity != input.context.workspace.identity
        {
            return Err(changed());
        }
        let c = confirmed_copy(db, workspace)?;
        if digest(&c)? != input.context.record_hash {
            return Err(changed());
        }
        input
            .verified
            .recheck_observed_identities()
            .map_err(data_error)
    }
}
impl Store {
    pub(crate) fn prepare_data_input_contexts(
        &self,
        ids: &[String],
    ) -> Result<Vec<DataInputContext>> {
        if ids.len() > 16 {
            return Err(limit());
        }
        let mut contexts = Vec::new();
        let mut bytes = 0usize;
        for id in ids {
            let workspace = workspace_record(&self.conn, id)?;
            if !matches!(
                workspace.identity.as_ref().map(|i| &i.kind),
                Some(WorkspaceKind::Directory)
            ) {
                continue;
            }
            let project = self.get_project(&workspace.project_id)?;
            if project.archived {
                return Err(CoreError::new(
                    "PERMISSION_DENIED",
                    "Data project is archived",
                ));
            }
            let creation = confirmed_copy(&self.conn, &workspace)?;
            let document = encode(&creation)?;
            bytes = bytes.checked_add(document.len()).ok_or_else(limit)?;
            if bytes > MAX_DOCUMENT as usize {
                return Err(limit());
            }
            let record_hash = agentbaton_workspaces::validation::sha256(document.as_bytes());
            contexts.push(DataInputContext {
                creation,
                workspace,
                project_revision: project.revision,
                record_hash,
            });
        }
        Ok(contexts)
    }
    pub(crate) fn require_data_input(&self, workspace: &ManagedWorkspace) -> Result<()> {
        let witness = self.data_input.as_ref().ok_or_else(|| {
            CoreError::new(
                "WORKSPACE_BUSY",
                "Data input hashing must finish on the bounded I/O worker before admission",
            )
        })?;
        witness.authorize(&self.conn, workspace)
    }
}
pub(crate) fn require_data_input_from(
    db: &Connection,
    workspace: &ManagedWorkspace,
    witness: Option<&DataInputWitness>,
) -> Result<()> {
    witness.ok_or_else(||CoreError::new("WORKSPACE_BUSY","Fresh private data input witness required; Directory labels and disk markers are not authority"))?.authorize(db,workspace)
}
pub(crate) fn data_live_workspace(db: &Connection, workspace: &ManagedWorkspace) -> Result<()> {
    let c = confirmed_copy(db, workspace)?;
    let actual = std::fs::symlink_metadata(&workspace.root).map_err(|_| changed())?;
    use std::os::unix::fs::MetadataExt;
    let canonical = std::fs::canonicalize(&workspace.root).map_err(|_| changed())?;
    let pin = c.contract.as_ref().ok_or_else(changed)?.destination.clone();
    if !actual.is_dir()
        || actual.dev() != pin.device
        || actual.ino() != pin.inode
        || canonical != pin.canonical_root
        || actual.mode() & 0o7777 != 0o700
        || actual.uid() != rustix::process::geteuid().as_raw()
    {
        return Err(changed());
    }
    Ok(())
}

impl Store {
    fn data_workspace_proposal_replay(
        &self,
        preview_id: &str,
        revision: u64,
        key_value: &str,
        managed: &Path,
    ) -> Result<Option<ApprovalProposal>> {
        let scope = "local:approvals.propose:data_workspace";
        let old = proposal_replay_document(&self.conn, scope, key_value)?;
        if let Some(old) = old {
            let proposal: ApprovalProposal = decode(&old)?;
            let intent = &proposal.snapshot["intent"];
            if intent["preview_id"] != preview_id
                || intent["preview_revision"] != revision
                || intent["managed_root"]["canonical_root"] != json!(managed)
            {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Data copy proposal key already used with another exact request",
                ));
            }
            return Ok(Some(proposal));
        }
        Ok(None)
    }
    pub(crate) fn data_preview_replay(
        &self,
        selection: &DataWorkspaceSelection,
        revision: u64,
        key_value: &str,
    ) -> Result<Option<DataWorkspacePreview>> {
        key(key_value)?;
        validate_selection(selection)?;
        let payload = encode(&json!({"selection":selection,"expected_revision":revision}))?;
        preview_replay_from(&self.conn, &selection.project_id, key_value, &payload)
    }
}
impl Store {
    pub(crate) fn data_proposal_replay(
        &self,
        kind: &str,
        intent: &Value,
        key_value: &str,
    ) -> Result<Option<ApprovalProposal>> {
        key(key_value)?;
        let scope = format!("local:approvals.propose:{kind}");
        let old = proposal_replay_document(&self.conn, &scope, key_value)?;
        let Some(old) = old else { return Ok(None) };
        let proposal: ApprovalProposal = decode(&old)?;
        let workspace = if kind == "grant" {
            proposal.snapshot["intent"]["workspace_id"].as_str()
        } else if kind == "goal" {
            proposal.snapshot["resolved"]["workspace"]["id"].as_str()
        } else {
            proposal.snapshot["refs"]["workspace_id"].as_str()
        };
        let Some(workspace) = workspace else {
            return Ok(None);
        };
        let data: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM data_workspace_creations WHERE workspace_id=?1)",
                [workspace],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if !data {
            return Ok(None);
        }
        if &proposal.snapshot["intent"] != intent {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Data-bound proposal key already used with another exact intent",
            ));
        }
        Ok(Some(proposal))
    }
}
fn history_budget(
    db: &Connection,
    table: &str,
    project: &str,
    bytes: usize,
    exclude: Option<&str>,
) -> Result<()> {
    let query=match table {
        "data_workspace_previews"=>"SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM data_workspace_previews WHERE project_id=?1 AND (?2 IS NULL OR id<>?2)",
        "data_workspace_creations"=>"SELECT count(*),COALESCE(sum(octet_length(document)),0) FROM data_workspace_creations WHERE project_id=?1 AND (?2 IS NULL OR id<>?2)",_=>return Err(invalid())};
    let (count, current): (i64, i64) = db
        .query_row(query, params![project, exclude], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(db_error)?;
    if count >= MAX_RECORDS
        || current < 0
        || bytes > MAX_DOCUMENT as usize
        || current as u64 + bytes as u64 > MAX_DOCUMENT as u64
    {
        return Err(limit());
    }
    Ok(())
}
// Reserve the largest serialized completed record BEFORE creating a directory.
// This uses fixed path contracts/maximum integer encodings, never input bytes.
fn creation_record_reservation(record: &DataWorkspaceCreation) -> Result<usize> {
    use agentbaton_workspaces::data_workspace::{
        CopiedDataFile, CreationState, DataCreationView, FilePin,
    };
    if record.snapshot.files.len() > 64
        || record.snapshot.files.iter().any(|f| {
            f.destination_relative_path.as_os_str().len() > 4103
                || f.destination_relative_path.components().count() > 17
        })
    {
        return Err(limit());
    }
    let mut upper = record.clone();
    let mut directories = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for source in &record.snapshot.files {
        let pin = FilePin {
            device: u64::MAX,
            inode: u64::MAX,
            bytes: source.bytes,
            uid: u32::MAX,
            gid: u32::MAX,
            mode: u32::MAX,
            mtime: i64::MIN,
            mtime_nsec: i64::MIN,
            ctime: i64::MIN,
            ctime_nsec: i64::MIN,
        };
        files.push(CopiedDataFile {
            input: source.clone(),
            destination_identity: pin,
        });
        let mut parent = source.destination_relative_path.parent();
        while let Some(path) = parent.filter(|p| !p.as_os_str().is_empty()) {
            directories.insert(record.destination.join(path));
            parent = path.parent();
        }
    }
    upper.contract = Some(DataWorkspaceContract {
        format_version: 1,
        creation_id: record.id.clone(),
        snapshot: record.snapshot.clone(),
        managed_root: record.managed_root.clone(),
        destination: DirectoryPin {
            canonical_root: record.destination.clone(),
            device: u64::MAX,
            inode: u64::MAX,
        },
        files,
        directories: directories
            .into_iter()
            .map(|canonical_root| DirectoryPin {
                canonical_root,
                device: u64::MAX,
                inode: u64::MAX,
            })
            .collect(),
        contract_digest: "f".repeat(64),
    });
    upper.observation = Some(DataCreationView {
        creation_id: record.id.clone(),
        destination: record.destination.clone(),
        state: CreationState::OutcomeUnknown,
        dataset_revision: record.dataset_revision.clone(),
        selection_digest: record.snapshot.selection_digest.clone(),
        copied_files: u32::MAX,
        error: Some(agentbaton_workspaces::data_workspace::DataWorkspaceError::ManagedRootInvalid),
        source_observation_scope: record.snapshot.source_observation_scope.clone(),
        atomic_directory_freeze: false,
        filesystem_sandbox: false,
    });
    encode(&upper)?
        .len()
        .checked_add(8192)
        .filter(|n| *n <= MAX_DOCUMENT as usize)
        .ok_or_else(limit)
}

impl Store {
    pub(crate) fn authorized_data_copy_context(
        &self,
        id: &str,
    ) -> Result<(DataWorkspaceCreation, Project)> {
        let creation = self.get_data_workspace_creation(id)?;
        if creation.id != id || creation.state != "creating" {
            return Err(changed());
        }
        confirmed_intent(&self.conn, &creation)?;
        let project = project_record(&self.conn, &creation.project_id)?;
        if project.archived || project.revision != creation.project_revision {
            return Err(changed());
        }
        Ok((creation, project))
    }
}
pub(crate) fn data_input_approval_view(
    db: &Connection,
    workspace: &ManagedWorkspace,
) -> Result<Value> {
    let c = confirmed_copy(db, workspace)?;
    let contract = c.contract.as_ref().ok_or_else(changed)?;
    let files:Vec<Value>=contract.files.iter().map(|file|json!({"artifact_id":file.input.requirement.artifact_id,"relative_path":file.input.destination_relative_path,"sha256":file.input.sha256,"bytes":file.input.bytes})).collect();
    Ok(
        json!({"creation_id":c.id,"approval_id":c.approval_id,"dataset_revision":c.dataset_revision,"contract_digest":contract.contract_digest,"files":files,"scope":"selected_dataset_observations_only","continuous_immutability":false}),
    )
}

fn cancel_replay<T: for<'de> Deserialize<'de>>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<T>> {
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN octet_length(payload)<=4194304 AND octet_length(result)<=4194304 THEN payload END, CASE WHEN octet_length(payload)<=4194304 AND octet_length(result)<=4194304 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    match row {
        None => Ok(None),
        Some((Some(original), Some(result))) => {
            if original != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Cancel key was already used with another observed revision",
                ));
            }
            Ok(Some(decode(&result)?))
        }
        Some(_) => Err(limit()),
    }
}
fn remember_cancel<T: Serialize>(
    db: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
    result: &T,
) -> Result<()> {
    db.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, encode(result)?],
    )
    .map_err(db_error)?;
    Ok(())
}

fn proposal_replay_document(db: &Connection, scope: &str, key: &str) -> Result<Option<String>> {
    let row:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(result) BETWEEN 0 AND 4194304 THEN result ELSE NULL END FROM idempotency WHERE scope=?1 AND key=?2",params![scope,key],|r|r.get(0)).optional().map_err(db_error)?;
    match row {
        None => Ok(None),
        Some(Some(document)) => Ok(Some(document)),
        Some(None) => Err(limit()),
    }
}
fn preview_replay_from(
    db: &Connection,
    project: &str,
    key: &str,
    payload: &str,
) -> Result<Option<DataWorkspacePreview>> {
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN octet_length(payload) BETWEEN 0 AND 4194304 AND octet_length(document) BETWEEN 0 AND 4194304 THEN payload ELSE NULL END, CASE WHEN octet_length(payload) BETWEEN 0 AND 4194304 AND octet_length(document) BETWEEN 0 AND 4194304 THEN document ELSE NULL END FROM data_workspace_previews WHERE project_id=?1 AND idempotency_key=?2",params![project,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
    match row {
        None => Ok(None),
        Some((Some(previous), Some(document))) => {
            if previous != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Data preview key already used with another exact selection",
                ));
            }
            Ok(Some(decode(&document)?))
        }
        Some(_) => Err(limit()),
    }
}

fn receipt_sequences(db: &Connection, id: &str) -> Result<(i64, i64, Option<i64>)> {
    let receipt:(i64,i64,Option<i64>)=db.query_row("SELECT owner_sequence,prepared_sequence,finished_sequence FROM data_workspace_receipts WHERE creation_id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?.ok_or_else(||CoreError::new("AUTH_REQUIRED","Data copy journal linkage is missing"))?;
    if receipt.0 <= 0
        || receipt.1 <= receipt.0
        || receipt.2.is_some_and(|finished| finished <= receipt.1)
    {
        return Err(CoreError::new("AUTH_REQUIRED","Owner, prepared-copy and finished-copy receipts must retain their actual positive commit ordering"));
    }
    Ok(receipt)
}
fn required_receipt(db: &Connection, sequence: i64, id: &str, kind: &str) -> Result<Event> {
    if sequence <= 0 {
        return Err(changed());
    }
    let document:Option<Option<String>>=db.query_row("SELECT CASE WHEN octet_length(document) BETWEEN 0 AND 4194304 THEN document ELSE NULL END FROM events WHERE sequence=?1",[sequence],|r|r.get(0)).optional().map_err(db_error)?;
    let document = document.flatten().ok_or_else(|| {
        CoreError::new(
            "AUTH_REQUIRED",
            "Required bounded data-copy journal receipt is missing",
        )
    })?;
    let event: Event = decode(&document)?;
    let expected_source = if kind == "approval.confirmed" {
        "local_owner_confirmation"
    } else {
        "data_workspace_core"
    };
    if event.aggregate_id != id || event.kind != kind || event.source != expected_source {
        return Err(CoreError::new(
            "AUTH_REQUIRED",
            "Data journal receipt belongs to another operation",
        ));
    }
    Ok(event)
}

// Metadata handoff for the exact native-approved validator artifacts. Original
// file descriptors remain owned so inode reuse cannot impersonate a prior file.
type ArtifactObservation = (
    agentbaton_workspaces::validation::ArtifactContractView,
    std::fs::File,
    Option<(
        std::fs::File,
        agentbaton_workspaces::data_workspace::FilePin,
    )>,
);
pub(crate) struct DataArtifactPins {
    entries: Vec<ArtifactObservation>,
}

fn artifact_root(root: &Path) -> Result<std::fs::File> {
    use rustix::fs::{open, openat, Mode, OFlags};
    let mut file: std::fs::File = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| changed())?
    .into();
    for part in root.components() {
        match part {
            std::path::Component::RootDir => {}
            std::path::Component::Normal(name) => {
                file = openat(
                    &file,
                    name,
                    OFlags::RDONLY
                        | OFlags::DIRECTORY
                        | OFlags::NOFOLLOW
                        | OFlags::NONBLOCK
                        | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|_| changed())?
                .into();
            }
            _ => return Err(changed()),
        }
    }
    Ok(file)
}
fn artifact_leaf(
    root: &std::fs::File,
    path: &Path,
) -> std::result::Result<std::fs::File, rustix::io::Errno> {
    use rustix::fs::{openat, Mode, OFlags};
    let mut file = root.try_clone().map_err(|_| rustix::io::Errno::IO)?;
    let parts: Vec<_> = path.components().collect();
    for (i, part) in parts.iter().enumerate() {
        let std::path::Component::Normal(name) = part else {
            return Err(rustix::io::Errno::INVAL);
        };
        let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        if i + 1 < parts.len() {
            flags |= OFlags::DIRECTORY;
        }
        file = openat(&file, *name, flags, Mode::empty())?.into();
    }
    Ok(file)
}
fn file_pin(m: &std::fs::Metadata) -> agentbaton_workspaces::data_workspace::FilePin {
    use std::os::unix::fs::MetadataExt;
    agentbaton_workspaces::data_workspace::FilePin {
        device: m.dev(),
        inode: m.ino(),
        bytes: m.len(),
        uid: m.uid(),
        gid: m.gid(),
        mode: m.mode(),
        mtime: m.mtime(),
        mtime_nsec: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_nsec: m.ctime_nsec(),
    }
}
impl DataArtifactPins {
    pub(crate) fn capture(
        contracts: &[agentbaton_workspaces::validation::ArtifactContractView],
    ) -> Result<Self> {
        use std::os::unix::fs::MetadataExt;
        if contracts.len() > 64 {
            return Err(limit());
        }
        let mut entries = Vec::new();
        for contract in contracts {
            if contract.canonical_root.as_os_str().len() > 4096
                || contract.requirement.relative_path.as_os_str().len() > 4096
            {
                return Err(limit());
            }
            let root = artifact_root(&contract.canonical_root)?;
            let m = root.metadata().map_err(|_| changed())?;
            if (m.dev(), m.ino()) != (contract.root_device, contract.root_inode) {
                return Err(changed());
            }
            let file = match artifact_leaf(&root, &contract.requirement.relative_path) {
                Ok(file) => {
                    let m = file.metadata().map_err(|_| changed())?;
                    Some((file, file_pin(&m)))
                }
                Err(_) => None,
            };
            entries.push((contract.clone(), root, file));
        }
        Ok(Self { entries })
    }
    pub(crate) fn verify(&self, passed: bool) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        for (contract, root, observed) in &self.entries {
            let current_root = artifact_root(&contract.canonical_root)?;
            let m = current_root.metadata().map_err(|_| changed())?;
            if (m.dev(), m.ino()) != (contract.root_device, contract.root_inode) {
                return Err(changed());
            }
            match (
                observed,
                artifact_leaf(root, &contract.requirement.relative_path),
            ) {
                (Some((held, pin)), Ok(current)) => {
                    let old = held.metadata().map_err(|_| changed())?;
                    let new = current.metadata().map_err(|_| changed())?;
                    if (passed
                        && (old.nlink() != 1
                            || new.nlink() != 1
                            || !old.is_file()
                            || !new.is_file()))
                        || file_pin(&old) != *pin
                        || file_pin(&new) != *pin
                    {
                        return Err(changed());
                    }
                }
                (None, Err(_)) if !passed => {}
                _ => return Err(changed()),
            }
        }
        Ok(())
    }
}
impl DataInputWitness {
    pub(crate) fn refresh(self) -> Result<Self> {
        let contexts = self.inputs.into_iter().map(|input| input.context).collect();
        Self::restore(contexts, self.request_hash, self.deadline)
    }
    pub(crate) fn deadline(&self) -> std::time::Instant {
        self.deadline
    }
}

impl DataInputWitness {
    /// A consumed request/attempt observation, never a reusable authorization
    /// cache. Worker handoff rechecks the original deadline and held descriptors.
    pub(crate) fn check_worker_boundary(&self) -> Result<()> {
        if std::time::Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Original data observation deadline expired before worker boundary",
            ));
        }
        for input in &self.inputs {
            input
                .verified
                .recheck_observed_identities()
                .map_err(data_error)?;
        }
        Ok(())
    }
    pub(crate) fn recheck_all_authority(&self, db: &Connection) -> Result<()> {
        for input in &self.inputs {
            let workspace = workspace_record(db, &input.context.workspace.id)?;
            self.authorize(db, &workspace)?;
        }
        Ok(())
    }
    pub(crate) fn selected_input_observation(
        &self,
        db: &Connection,
        workspace: &ManagedWorkspace,
        requirement: &ArtifactRequirement,
    ) -> Result<(String, u64)> {
        let sha = self.selected_input_hash(db, workspace, requirement)?;
        let input = self
            .inputs
            .iter()
            .find(|i| i.context.workspace.id == workspace.id)
            .ok_or_else(changed)?;
        let file = input
            .context
            .creation
            .contract
            .as_ref()
            .ok_or_else(changed)?
            .files
            .iter()
            .find(|f| f.input.destination_relative_path == requirement.relative_path)
            .ok_or_else(changed)?;
        Ok((sha, file.input.bytes))
    }
}
impl DataInputWitness {
    /// Drops the request admission proof. These immutable creation references
    /// may only support a fresh execution observation; they are not a witness
    /// for any later scoped request or dispatch.
    pub(crate) fn into_observation_contexts(self) -> Vec<DataInputContext> {
        self.inputs.into_iter().map(|i| i.context).collect()
    }
}
