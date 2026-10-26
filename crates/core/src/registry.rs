//! Persistent owner-selected projects, isolated workspace intents, and fenced workspace leases.
use crate::storage::{db_error, decode, encode, now, Event, Store};
use crate::{CoreError, Result};
use agentbaton_workspaces::{
    inspect, reassociate, WorkspaceError, WorkspaceIdentity, WorkspaceKind,
};
use rusqlite::{params, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub identity: WorkspaceIdentity,
    pub revision: u64,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ManagedWorkspace {
    pub id: String,
    pub project_id: String,
    pub identity: Option<WorkspaceIdentity>,
    pub root: PathBuf,
    pub base_revision: String,
    pub created_at: String,
    pub state: String,
    pub revision: u64,
}

/// Durable Baton write-boundary fencing, not process termination or OS write isolation.
/// UTC expiry survives restart but is sensitive to wall-clock changes; it must never serve as
/// a monotonic execution budget. Before reusing a workspace, the scheduler must reconcile or
/// stop the previous Provider process: an expired database lease does not stop its file writes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceLease {
    pub resource: String,
    pub holder: String,
    pub fencing_token: u64,
    pub expires_at_ms: i64,
}

fn workspace_error(error: WorkspaceError) -> CoreError {
    match error {
        WorkspaceError::InvalidPath | WorkspaceError::NotRepository => CoreError::new(
            "INVALID_REQUEST",
            "Select a valid existing canonical workspace root or exact commit",
        ),
        WorkspaceError::UnsafeRepository => CoreError::new(
            "PERMISSION_DENIED",
            "Repository checkout filters require a supported explicit policy; no scripts were run",
        ),
        WorkspaceError::Dirty => {
            CoreError::new("CONFIG_CONFLICT", "Workspace contains uncommitted changes")
        }
        WorkspaceError::StaleHead => {
            CoreError::new("INPUT_CHANGED", "Source HEAD changed; refresh the project")
        }
        WorkspaceError::ResourceLimit | WorkspaceError::TooLarge => CoreError::new(
            "RESOURCE_LIMIT",
            "Workspace operation exceeded its bounded resources",
        ),
        _ => CoreError::new(
            "INVALID_REQUEST",
            "Workspace cannot be inspected; verify the selected root and permissions",
        ),
    }
}

fn key_valid(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "A bounded idempotency key is required",
        ));
    }
    Ok(())
}
fn event(tx: &Transaction<'_>, id: &str, revision: u64, kind: &str, payload: Value) -> Result<()> {
    let event = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: id.into(),
        revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload,
        source: "user_action".into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
        .map_err(db_error)?;
    Ok(())
}
fn replay<T: for<'a> Deserialize<'a>>(
    tx: &Transaction<'_>,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<T>> {
    let old: Option<(String, String)> = tx
        .query_row(
            "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
            params![scope, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    if let Some((old_payload, result)) = old {
        if old_payload != payload {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Idempotency key already used with another request",
            ));
        }
        return Ok(Some(decode(&result)?));
    }
    Ok(None)
}
fn remember<T: Serialize>(
    tx: &Transaction<'_>,
    scope: &str,
    key: &str,
    payload: &str,
    result: &T,
) -> Result<()> {
    tx.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, encode(result)?],
    )
    .map_err(db_error)?;
    Ok(())
}
fn project_from(conn: &rusqlite::Connection, id: &str) -> Result<Project> {
    let raw: Option<String> = conn
        .query_row("SELECT document FROM projects WHERE id=?1", [id], |r| {
            r.get(0)
        })
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Project does not exist"))?)
}
fn workspace_from(conn: &rusqlite::Connection, id: &str) -> Result<ManagedWorkspace> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT document FROM managed_workspaces WHERE id=?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(|| CoreError::new("NOT_FOUND", "Managed workspace does not exist"))?)
}
fn current_project(project: &Project, revision: u64) -> Result<()> {
    if project.revision != revision {
        return Err(CoreError::new(
            "STALE_REVISION",
            "Project changed; refresh before writing",
        ));
    }
    if project.archived {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Project is archived; no new workspace operations are allowed",
        ));
    }
    Ok(())
}
fn project_save(tx: &Transaction<'_>, project: &Project) -> Result<()> {
    tx.execute(
        "UPDATE projects SET document=?1,device=?2,inode=?3,root=?4 WHERE id=?5",
        params![
            encode(project)?,
            project.identity.device.to_string(),
            project.identity.inode.to_string(),
            project.identity.canonical_root.to_string_lossy(),
            project.id
        ],
    )
    .map_err(db_error)?;
    Ok(())
}
fn milliseconds() -> Result<i64> {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| CoreError::new("INTERNAL_ERROR", "Clock outside supported range"))
}
fn lease_expiry(ttl_ms: u64) -> Result<i64> {
    if ttl_ms == 0 || ttl_ms > 3_600_000 {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "Lease duration must be within one hour",
        ));
    }
    milliseconds()?
        .checked_add(ttl_ms as i64)
        .ok_or_else(|| CoreError::new("INVALID_REQUEST", "Lease expiry overflow"))
}
fn lease_from(conn: &rusqlite::Connection, resource: &str) -> Result<Option<WorkspaceLease>> {
    conn.query_row(
        "SELECT holder,fencing_token,expires_at FROM workspace_leases WHERE resource=?1",
        [resource],
        |r| {
            Ok(WorkspaceLease {
                resource: resource.into(),
                holder: r.get(0)?,
                fencing_token: u64::try_from(r.get::<_, i64>(1)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                expires_at_ms: r.get(2)?,
            })
        },
    )
    .optional()
    .map_err(db_error)
}

fn check_workspace_identity(workspace: &ManagedWorkspace, check_base: bool) -> Result<()> {
    let pinned = workspace.identity.as_ref().ok_or_else(|| {
        CoreError::new(
            "OUTCOME_UNKNOWN",
            "Workspace identity has not been confirmed",
        )
    })?;
    let current = inspect(&workspace.root).map_err(|_| {
        CoreError::new(
            "INPUT_CHANGED",
            "Managed workspace is missing or cannot be safely inspected",
        )
    })?;
    if current.canonical_root != pinned.canonical_root
        || current.device != pinned.device
        || current.inode != pinned.inode
    {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Managed workspace directory was moved or replaced",
        ));
    }
    match (&pinned.kind, &current.kind) {
        (
            WorkspaceKind::Git {
                common_dir: pinned_git,
                ..
            },
            WorkspaceKind::Git {
                common_dir: current_git,
                head,
                ..
            },
        ) if pinned_git == current_git => {
            if check_base && head != &workspace.base_revision {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Managed workspace HEAD changed; revalidate before a new writer",
                ));
            }
            Ok(())
        }
        _ => Err(CoreError::new(
            "INPUT_CHANGED",
            "Managed workspace repository identity changed",
        )),
    }
}

impl Store {
    pub fn import_project(&mut self, path: &Path, name: &str, key: &str) -> Result<Project> {
        key_valid(key)?;
        if name.trim().is_empty() || name.len() > 256 || path.to_str().is_none() {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Project name and UTF-8 path are required",
            ));
        }
        let payload = encode(&json!({"path":path,"name":name}))?;
        let scope = "local:projects.import";
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = replay(&tx, scope, key, &payload)? {
            return Ok(old);
        }
        let identity = inspect(path).map_err(workspace_error)?;
        let duplicate: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE root=?1 OR (device=?2 AND inode=?3))",
                params![
                    identity.canonical_root.to_string_lossy(),
                    identity.device.to_string(),
                    identity.inode.to_string()
                ],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if duplicate {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Directory is already registered; use the existing project or reassociate it",
            ));
        }
        let timestamp = now()?;
        let project = Project {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            identity,
            revision: 1,
            archived: false,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        tx.execute(
            "INSERT INTO projects(id,document,device,inode,root) VALUES(?1,?2,?3,?4,?5)",
            params![
                project.id,
                encode(&project)?,
                project.identity.device.to_string(),
                project.identity.inode.to_string(),
                project.identity.canonical_root.to_string_lossy()
            ],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &project.id,
            project.revision,
            "project.imported",
            json!({"root":project.identity.canonical_root}),
        )?;
        remember(&tx, scope, key, &payload, &project)?;
        tx.commit().map_err(db_error)?;
        Ok(project)
    }
    pub fn get_project(&self, id: &str) -> Result<Project> {
        project_from(&self.conn, id)
    }
    pub fn project(&self, id: &str) -> Result<Project> {
        project_from(&self.conn, id)
    }
    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let count: i64 = self
            .conn
            .query_row("SELECT count(*) FROM projects", [], |r| r.get(0))
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Project listing exceeds 1000 records",
            ));
        }
        let mut statement = self
            .conn
            .prepare("SELECT document FROM projects ORDER BY rowid DESC LIMIT 1000")
            .map_err(db_error)?;
        let rows = statement
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub fn archive_project(
        &mut self,
        id: &str,
        expected_revision: u64,
        key: &str,
    ) -> Result<Project> {
        key_valid(key)?;
        let scope = format!("local:{id}:projects.archive");
        let payload = encode(&json!({"expected_revision":expected_revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = replay(&tx, &scope, key, &payload)? {
            return Ok(old);
        }
        let mut project = project_from(&tx, id)?;
        if project.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Project changed; refresh before archiving",
            ));
        }
        if !project.archived {
            project.archived = true;
            project.revision += 1;
            project.updated_at = now()?;
            project_save(&tx, &project)?;
            event(
                &tx,
                id,
                project.revision,
                "project.archived",
                json!({"source_files_deleted":false}),
            )?;
        }
        remember(&tx, &scope, key, &payload, &project)?;
        tx.commit().map_err(db_error)?;
        Ok(project)
    }
    pub fn reassociate_project(
        &mut self,
        id: &str,
        path: &Path,
        expected_revision: u64,
        key: &str,
    ) -> Result<Project> {
        key_valid(key)?;
        if path.to_str().is_none() {
            return Err(CoreError::new("INVALID_REQUEST", "UTF-8 path required"));
        }
        let scope = format!("local:{id}:projects.reassociate");
        let payload = encode(&json!({"path":path,"expected_revision":expected_revision}))?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = replay(&tx, &scope, key, &payload)? {
            return Ok(old);
        }
        let mut project = project_from(&tx, id)?;
        current_project(&project, expected_revision)?;
        project.identity = reassociate(&project.identity, path).map_err(workspace_error)?;
        project.revision += 1;
        project.updated_at = now()?;
        project_save(&tx, &project)?;
        event(
            &tx,
            id,
            project.revision,
            "project.reassociated",
            json!({"root":project.identity.canonical_root}),
        )?;
        remember(&tx, &scope, key, &payload, &project)?;
        tx.commit().map_err(db_error)?;
        Ok(project)
    }
    pub fn create_workspace(
        &mut self,
        id: &str,
        expected_project_revision: u64,
        expected_head: &str,
        managed_root: &Path,
        key: &str,
    ) -> Result<ManagedWorkspace> {
        key_valid(key)?;
        let payload = encode(
            &json!({"project_id":id,"expected_project_revision":expected_project_revision,"expected_head":expected_head}),
        )?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let old:Option<(String,String)>=tx.query_row("SELECT payload,document FROM managed_workspaces WHERE project_id=?1 AND idempotency_key=?2",params![id,key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((old_payload, raw)) = old {
            if old_payload != payload {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Workspace creation key already used with another request",
                ));
            }
            let existing: ManagedWorkspace = decode(&raw)?;
            if existing.state == "ready" {
                return Ok(existing);
            }
            let mut error=CoreError::new("OUTCOME_UNKNOWN","Workspace creation was already attempted; inspect its persisted reservation before retrying");
            error.side_effect_state = "unknown".into();
            return Err(error);
        }
        let project = project_from(&tx, id)?;
        current_project(&project, expected_project_revision)?;
        let current = inspect(&project.identity.canonical_root).map_err(workspace_error)?;
        if current.device != project.identity.device || current.inode != project.identity.inode {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Source directory was replaced; reassociate before use",
            ));
        }
        let WorkspaceKind::Git { ref head, .. } = current.kind else {
            return Err(CoreError::new(
                "CAPABILITY_UNSUPPORTED",
                "Coding workspaces require a committed Git repository",
            ));
        };
        if head != expected_head {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Source HEAD changed; refresh before workspace creation",
            ));
        }
        let managed_root = fs::canonicalize(managed_root).map_err(|_| {
            CoreError::new(
                "PERMISSION_DENIED",
                "Managed root must be a private existing host-selected directory",
            )
        })?;
        let metadata = fs::metadata(&managed_root)
            .map_err(|_| CoreError::new("PERMISSION_DENIED", "Managed root unavailable"))?;
        if !metadata.is_dir()
            || metadata.mode() & 0o077 != 0
            || metadata.uid() != rustix::process::getuid().as_raw()
        {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Managed root must be private and owned by this user",
            ));
        }
        if managed_root.starts_with(&current.canonical_root) {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "Managed workspaces must be outside the original project root",
            ));
        }
        let workspace_id = Uuid::new_v4().to_string();
        let mut workspace = ManagedWorkspace {
            id: workspace_id.clone(),
            project_id: id.into(),
            identity: None,
            root: managed_root.join(&workspace_id),
            base_revision: expected_head.into(),
            created_at: now()?,
            state: "creating".into(),
            revision: 1,
        };
        tx.execute("INSERT INTO managed_workspaces(id,project_id,document,state,idempotency_key,root,payload) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![workspace.id,id,encode(&workspace)?,workspace.state,key,workspace.root.to_string_lossy(),payload]).map_err(db_error)?;
        event(
            &tx,
            &workspace.id,
            workspace.revision,
            "workspace.creation_reserved",
            json!({"project_id":id,"root":workspace.root,"base_revision":expected_head}),
        )?;
        tx.commit().map_err(db_error)?;
        let created = agentbaton_workspaces::create_isolated(
            &current.canonical_root,
            &managed_root,
            &workspace_id,
            expected_head,
        );
        workspace.revision += 1;
        match created {
            Ok(identity) => {
                workspace.identity = Some(identity);
                workspace.state = "ready".into();
            }
            Err(_) => {
                workspace.state = "outcome_unknown".into();
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        tx.execute(
            "UPDATE managed_workspaces SET document=?1,state=?2 WHERE id=?3",
            params![encode(&workspace)?, workspace.state, workspace.id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &workspace.id,
            workspace.revision,
            if workspace.state == "ready" {
                "workspace.created"
            } else {
                "workspace.creation_outcome_unknown"
            },
            json!({"project_id":id,"root":workspace.root}),
        )?;
        tx.commit().map_err(db_error)?;
        if workspace.state != "ready" {
            let mut error = CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace creation failed after a durable reservation; inspect it before retrying",
            );
            error.side_effect_state = "unknown".into();
            return Err(error);
        }
        Ok(workspace)
    }
    pub fn list_workspaces(&self, project_id: &str) -> Result<Vec<ManagedWorkspace>> {
        project_from(&self.conn, project_id)?;
        let count: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM managed_workspaces WHERE project_id=?1",
                [project_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Workspace listing exceeds 1000 records",
            ));
        }
        let mut statement=self.conn.prepare("SELECT document FROM managed_workspaces WHERE project_id=?1 ORDER BY rowid LIMIT 1000").map_err(db_error)?;
        let rows = statement
            .query_map([project_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub(crate) fn recover_workspace_creations(&mut self) -> Result<()> {
        // Bound retained recovery input while committing each intent and its event atomically.
        loop {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let records = {
                let mut statement = tx
                    .prepare(
                        "SELECT document FROM managed_workspaces WHERE state='creating' LIMIT 100",
                    )
                    .map_err(db_error)?;
                let rows = statement
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(db_error)?;
                rows.collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(db_error)?
            };
            if records.is_empty() {
                tx.commit().map_err(db_error)?;
                return Ok(());
            }
            for raw in records {
                let mut workspace: ManagedWorkspace = decode(&raw)?;
                workspace.state = "outcome_unknown".into();
                workspace.revision += 1;
                tx.execute(
                    "UPDATE managed_workspaces SET document=?1,state='outcome_unknown' WHERE id=?2",
                    params![encode(&workspace)?, workspace.id],
                )
                .map_err(db_error)?;
                event(
                    &tx,
                    &workspace.id,
                    workspace.revision,
                    "workspace.recovery_outcome_unknown",
                    json!({"root":workspace.root,"automatic_replay":false}),
                )?;
            }
            tx.commit().map_err(db_error)?;
        }
    }
    pub fn workspace_lease(&self, resource: &str) -> Result<Option<WorkspaceLease>> {
        lease_from(&self.conn, resource)
    }
    pub fn acquire_workspace_lease(
        &mut self,
        resource: &str,
        holder: &str,
        expected_fencing_token: u64,
        ttl_ms: u64,
    ) -> Result<WorkspaceLease> {
        if holder.is_empty() || holder.len() > 128 {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "A bounded lease holder is required",
            ));
        }
        let expiry = lease_expiry(ttl_ms)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let workspace = workspace_from(&tx, resource)?;
        if workspace.state != "ready" {
            return Err(CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace is not reconciled and ready",
            ));
        }
        let project = project_from(&tx, &workspace.project_id)?;
        if project.archived {
            return Err(CoreError::new("PERMISSION_DENIED", "Project is archived"));
        }
        check_workspace_identity(&workspace, true)?;
        let previous = lease_from(&tx, resource)?;
        let token = previous.as_ref().map_or(0, |l| l.fencing_token);
        if token != expected_fencing_token {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Lease generation changed; refresh before acquiring",
            ));
        }
        let current_ms = milliseconds()?;
        if previous
            .as_ref()
            .is_some_and(|l| !l.holder.is_empty() && l.expires_at_ms > current_ms)
        {
            return Err(CoreError::new(
                "WORKSPACE_BUSY",
                "Workspace already has an unexpired writer lease",
            ));
        }
        let token = token
            .checked_add(1)
            .filter(|v| *v <= i64::MAX as u64)
            .ok_or_else(|| CoreError::new("RESOURCE_LIMIT", "Lease generation exhausted"))?;
        let lease = WorkspaceLease {
            resource: resource.into(),
            holder: holder.into(),
            fencing_token: token,
            expires_at_ms: expiry,
        };
        tx.execute("INSERT INTO workspace_leases(resource,holder,fencing_token,expires_at) VALUES(?1,?2,?3,?4) ON CONFLICT(resource) DO UPDATE SET holder=excluded.holder,fencing_token=excluded.fencing_token,expires_at=excluded.expires_at",params![resource,holder,token as i64,expiry]).map_err(db_error)?;
        event(
            &tx,
            resource,
            token,
            "workspace.lease_acquired",
            json!({"holder":holder,"fencing_token":token,"expires_at_ms":expiry}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(lease)
    }
    pub fn renew_workspace_lease(
        &mut self,
        resource: &str,
        holder: &str,
        fencing_token: u64,
        ttl_ms: u64,
    ) -> Result<WorkspaceLease> {
        let expiry = lease_expiry(ttl_ms)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut lease = lease_from(&tx, resource)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Workspace lease does not exist"))?;
        if lease.holder != holder
            || holder.is_empty()
            || lease.fencing_token != fencing_token
            || lease.expires_at_ms <= milliseconds()?
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Writer lease expired or was superseded",
            ));
        }
        let workspace = workspace_from(&tx, resource)?;
        check_workspace_identity(&workspace, false)?;
        if project_from(&tx, &workspace.project_id)?.archived {
            return Err(CoreError::new("PERMISSION_DENIED", "Project is archived"));
        }
        lease.expires_at_ms = expiry;
        tx.execute(
            "UPDATE workspace_leases SET expires_at=?1 WHERE resource=?2",
            params![expiry, resource],
        )
        .map_err(db_error)?;
        event(
            &tx,
            resource,
            fencing_token,
            "workspace.lease_renewed",
            json!({"holder":holder,"expires_at_ms":expiry}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(lease)
    }
    pub fn release_workspace_lease(
        &mut self,
        resource: &str,
        holder: &str,
        fencing_token: u64,
    ) -> Result<WorkspaceLease> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut lease = lease_from(&tx, resource)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Workspace lease does not exist"))?;
        if lease.holder != holder || holder.is_empty() || lease.fencing_token != fencing_token {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Writer lease was superseded",
            ));
        }
        lease.holder.clear();
        lease.expires_at_ms = 0;
        tx.execute(
            "UPDATE workspace_leases SET holder='',expires_at=0 WHERE resource=?1",
            [resource],
        )
        .map_err(db_error)?;
        event(
            &tx,
            resource,
            fencing_token,
            "workspace.lease_released",
            json!({"previous_holder":holder,"fencing_token":fencing_token}),
        )?;
        tx.commit().map_err(db_error)?;
        Ok(lease)
    }
    /// Fencing validation at a Baton write boundary; external editors do not honor this lease.
    pub fn validate_workspace_lease(
        &self,
        resource: &str,
        holder: &str,
        fencing_token: u64,
    ) -> Result<WorkspaceLease> {
        let lease = lease_from(&self.conn, resource)?
            .ok_or_else(|| CoreError::new("NOT_FOUND", "Workspace lease does not exist"))?;
        if holder.is_empty()
            || lease.holder != holder
            || lease.fencing_token != fencing_token
            || lease.expires_at_ms <= milliseconds()?
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Writer lease expired or was superseded",
            ));
        }
        let workspace = workspace_from(&self.conn, resource)?;
        check_workspace_identity(&workspace, false)?;
        if workspace.state != "ready" || project_from(&self.conn, &workspace.project_id)?.archived {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Workspace is not available for new writes",
            ));
        }
        Ok(lease)
    }
}
