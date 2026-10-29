//! A Task-owned Git preparation intent. Neither this record nor a worktree grants execution.
use crate::registry::{finalize_workspace_row, reserve_workspace_row, ManagedWorkspace, Project};
use crate::storage::{db_error, decode, encode, now, Event, Store, Task, TaskState};
use crate::{CoreError, Result};
use agentbaton_workspaces::{inspect, WorkspaceIdentity, WorkspaceKind};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_RECORD: i64 = 262_144;
const BINDING_VERSION: u32 = 1;

/// Persisted and future-renderer identity integers are canonical lossless decimal strings.
mod decimal_u64 {
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        value: &u64,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.is_empty() || text.len() > 20 || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err(D::Error::custom(
                "A canonical decimal u64 string is required",
            ));
        }
        let value = text
            .parse::<u64>()
            .map_err(|_| D::Error::custom("Filesystem identity exceeds u64"))?;
        if value.to_string() != text {
            return Err(D::Error::custom("Filesystem identity is not canonical"));
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GitWorkspacePin {
    pub canonical_root: PathBuf,
    #[serde(with = "decimal_u64")]
    pub device: u64,
    #[serde(with = "decimal_u64")]
    pub inode: u64,
    pub common_dir: PathBuf,
    pub head: String,
}
impl GitWorkspacePin {
    fn observed(identity: WorkspaceIdentity) -> Result<Self> {
        let WorkspaceKind::Git {
            common_dir, head, ..
        } = identity.kind
        else {
            return Err(unsupported());
        };
        Ok(Self {
            canonical_root: identity.canonical_root,
            device: identity.device,
            inode: identity.inode,
            common_dir,
            head,
        })
    }
    fn same_physical_repository(&self, other: &Self) -> bool {
        self.canonical_root == other.canonical_root
            && self.device == other.device
            && self.inode == other.inode
            && self.common_dir == other.common_dir
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskGitWorkspacePreparation {
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_spec_revision: u64,
    pub expected_project_revision: u64,
    pub host_id: String,
    pub expected_source_pin: GitWorkspacePin,
    pub preparation_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskWorkspaceBindingState {
    Pending,
    Ready,
    OutcomeUnknown,
}

/// Immutable result of the original reservation transaction, never a current Task snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskWorkspaceReservationReceipt {
    pub version: u32,
    pub binding_id: String,
    pub workspace_id: String,
    pub task_id: String,
    pub project_id: String,
    pub host_id: String,
    pub original_task_revision: u64,
    pub reserved_task_revision: u64,
    pub spec_revision: u64,
    pub project_revision: u64,
    pub source_pin: GitWorkspacePin,
    pub managed_root: PathBuf,
    pub preparation_key: String,
    pub request_key: String,
    pub request_digest: String,
    pub created_at: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskWorkspaceBinding {
    pub receipt: TaskWorkspaceReservationReceipt,
    pub state: TaskWorkspaceBindingState,
    pub revision: u64,
    pub managed_pin: Option<GitWorkspacePin>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TaskWorkspaceBindingView {
    pub original_receipt: TaskWorkspaceReservationReceipt,
    pub binding: TaskWorkspaceBinding,
    pub task_revision: u64,
    pub observed_managed_pin: Option<GitWorkspacePin>,
    pub identity_revalidated: bool,
    pub execution_authorized: bool,
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Task workspace binding references changed; revalidate explicitly",
    )
}
fn unsupported() -> CoreError {
    CoreError::new(
        "CAPABILITY_UNSUPPORTED",
        "A registered Git source is required; Directory inputs require selected Dataset approval",
    )
}
fn unknown() -> CoreError {
    let mut e=CoreError::new("OUTCOME_UNKNOWN", "Task workspace preparation is unresolved; do not create or replay its filesystem operation");
    e.side_effect_state = "unknown".into();
    e
}
fn conflict() -> CoreError {
    CoreError::new(
        "CONFIG_CONFLICT",
        "Task workspace preparation key or existing binding differs",
    )
}
fn bump(value: u64) -> Result<u64> {
    value
        .checked_add(1)
        .ok_or_else(|| CoreError::new("RESOURCE_LIMIT", "Workspace binding revision is exhausted"))
}
fn scope(task_id: &str) -> String {
    format!("local:{task_id}:task-workspace.prepare_git")
}
fn creation_key(id: &str) -> String {
    format!("task-binding:{id}")
}
fn digest(raw: &str) -> String {
    format!("{:x}", Sha256::digest(raw.as_bytes()))
}
fn bounded(raw: Option<String>) -> Result<String> {
    raw.ok_or_else(|| {
        CoreError::new(
            "RESOURCE_LIMIT",
            "Task workspace preparation record exceeds its read bound",
        )
    })
}
fn task_from(db: &Connection, id: &str) -> Result<Task> {
    if id.is_empty() || id.len() > 128 {
        return Err(changed());
    }
    let raw:Option<Option<String>>=db.query_row("SELECT CASE WHEN length(CAST(document AS BLOB))<=262144 THEN document END FROM tasks WHERE id=?1",[id],|row|row.get(0)).optional().map_err(db_error)?;
    let task: Task =
        decode(&bounded(raw.ok_or_else(|| {
            CoreError::new("NOT_FOUND", "Task does not exist")
        })?)?)?;
    if task.id != id || task.revision == 0 || task.spec_revision == 0 {
        return Err(changed());
    }
    Ok(task)
}
fn project_from(db: &Connection, id: &str) -> Result<Project> {
    let row:Option<(Option<String>,Option<String>,Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN length(CAST(document AS BLOB))<=262144 THEN document END,CASE WHEN length(CAST(root AS BLOB))<=262144 THEN root END,CASE WHEN length(CAST(device AS BLOB))<=20 THEN device END,CASE WHEN length(CAST(inode AS BLOB))<=20 THEN inode END FROM projects WHERE id=?1",[id],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).optional().map_err(db_error)?;
    let (raw, root, device, inode) =
        row.ok_or_else(|| CoreError::new("NOT_FOUND", "Project does not exist"))?;
    let p: Project = decode(&bounded(raw)?)?;
    if p.id != id
        || p.revision == 0
        || root.as_deref() != p.identity.canonical_root.to_str()
        || root.is_none()
        || device.as_deref() != Some(p.identity.device.to_string().as_str())
        || inode.as_deref() != Some(p.identity.inode.to_string().as_str())
    {
        return Err(changed());
    }
    Ok(p)
}
fn source_from(
    db: &Connection,
    task: &Task,
    receipt: &TaskWorkspaceReservationReceipt,
) -> Result<GitWorkspacePin> {
    if task.project_id != receipt.project_id
        || task.spec_revision != receipt.spec_revision
        || task.revision < receipt.reserved_task_revision
        || receipt.task_id != task.id
        || receipt.host_id != "local"
    {
        return Err(changed());
    }
    let p = project_from(db, &task.project_id)?;
    if p.revision != receipt.project_revision {
        return Err(CoreError::new(
            "STALE_REVISION",
            "Project changed before Task workspace reuse",
        ));
    }
    if p.archived {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Archived projects cannot prepare or admit Task workspaces",
        ));
    }
    let registered = GitWorkspacePin::observed(p.identity)?;
    let observed =
        GitWorkspacePin::observed(inspect(&registered.canonical_root).map_err(|_| changed())?)?;
    if !registered.same_physical_repository(&observed) || observed != receipt.source_pin {
        return Err(changed());
    }
    Ok(observed)
}
fn managed_root(root: &Path, source: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(root).map_err(|_| {
        CoreError::new(
            "PERMISSION_DENIED",
            "Managed root must be an existing private host directory",
        )
    })?;
    let meta = fs::metadata(&canonical).map_err(|_| changed())?;
    if !meta.is_dir()
        || meta.mode() & 0o077 != 0
        || meta.uid() != rustix::process::getuid().as_raw()
        || canonical.starts_with(source)
    {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Managed root must be private, owned and outside the source",
        ));
    }
    Ok(canonical)
}
fn request_valid(request: &TaskGitWorkspacePreparation, key: &str) -> Result<()> {
    let p = &request.expected_source_pin;
    if request.task_id.is_empty()
        || request.task_id.len() > 128
        || key.is_empty()
        || key.len() > 256
        || request.expected_task_revision == 0
        || request.expected_spec_revision == 0
        || request.expected_project_revision == 0
        || request.host_id != "local"
        || !p.canonical_root.is_absolute()
        || !p.common_dir.is_absolute()
        || p.canonical_root.to_str().is_none()
        || p.common_dir.to_str().is_none()
        || p.canonical_root.as_os_str().len() > MAX_RECORD as usize
        || p.common_dir.as_os_str().len() > MAX_RECORD as usize
        || !matches!(p.head.len(), 40 | 64)
        || !p
            .head
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        || request.preparation_key.len() != 74
        || !request.preparation_key.starts_with("code-prep:")
        || !request.preparation_key[10..]
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "A bounded local Task/Git preparation and exact revisions are required",
        ));
    }
    Ok(())
}
fn event(
    tx: &Transaction<'_>,
    task: &Task,
    kind: &str,
    binding: &TaskWorkspaceBinding,
) -> Result<()> {
    let event = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: task.id.clone(),
        revision: task.revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload: json!({"binding_id":binding.receipt.binding_id,"workspace_id":binding.receipt.workspace_id,"state":binding.state,"execution_authorized":false}),
        source: "user_action".into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&event)?])
        .map_err(db_error)?;
    Ok(())
}
fn save_task(tx: &Transaction<'_>, task: &Task) -> Result<()> {
    let raw = encode(task)?;
    if raw.len() > MAX_RECORD as usize {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Task workspace binding exceeds its retained record bound",
        ));
    }
    if tx
        .execute(
            "UPDATE tasks SET document=?1 WHERE id=?2",
            params![raw, task.id],
        )
        .map_err(db_error)?
        != 1
    {
        return Err(changed());
    }
    Ok(())
}
fn original_receipt(
    db: &Connection,
    binding: &TaskWorkspaceBinding,
) -> Result<TaskWorkspaceReservationReceipt> {
    let r = &binding.receipt;
    if r.version != BINDING_VERSION
        || binding.revision == 0
        || r.binding_id.is_empty()
        || r.workspace_id.is_empty()
        || r.task_id.is_empty()
        || r.project_id == "local"
        || r.host_id != "local"
        || r.original_task_revision == 0
        || r.reserved_task_revision != bump(r.original_task_revision)?
    {
        return Err(changed());
    }
    let row:Option<(Option<String>,Option<String>)>=db.query_row("SELECT CASE WHEN length(CAST(payload AS BLOB))<=262144 THEN payload END,CASE WHEN length(CAST(result AS BLOB))<=262144 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope(&r.task_id),r.request_key],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(db_error)?;
    let (payload, result) = row.ok_or_else(changed)?;
    let payload = bounded(payload)?;
    let original: TaskWorkspaceReservationReceipt = decode(&bounded(result)?)?;
    let request: TaskGitWorkspacePreparation = decode(&payload)?;
    request_valid(&request, &r.request_key)?;
    if original != *r
        || digest(&payload) != r.request_digest
        || request.task_id != r.task_id
        || request.expected_task_revision != r.original_task_revision
        || request.expected_spec_revision != r.spec_revision
        || request.expected_project_revision != r.project_revision
        || request.expected_source_pin != r.source_pin
        || request.preparation_key != r.preparation_key
    {
        return Err(changed());
    }
    Ok(original)
}
fn workspace_from(db: &Connection, binding: &TaskWorkspaceBinding) -> Result<ManagedWorkspace> {
    let r = &binding.receipt;
    let row:Option<(String,String,String,String,Option<String>,Option<String>)>=db.query_row("SELECT id,project_id,state,root,CASE WHEN length(CAST(document AS BLOB))<=262144 THEN document END,CASE WHEN length(CAST(payload AS BLOB))<=262144 THEN payload END FROM managed_workspaces WHERE id=?1 AND idempotency_key=?2",params![r.workspace_id,creation_key(&r.binding_id)],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?))).optional().map_err(db_error)?;
    let (id, project, state, root, raw, payload) = row.ok_or_else(unknown)?;
    let workspace: ManagedWorkspace = decode(&bounded(raw)?)?;
    if id != workspace.id
        || project != workspace.project_id
        || project != r.project_id
        || state != workspace.state
        || Path::new(&root) != workspace.root
        || workspace.root != r.managed_root.join(&r.workspace_id)
        || digest(&bounded(payload)?) != r.request_digest
        || workspace.revision == 0
    {
        return Err(changed());
    }
    Ok(workspace)
}
fn verify_bound_rows(
    db: &Connection,
    task: &Task,
    binding: &TaskWorkspaceBinding,
    workspace: &ManagedWorkspace,
) -> Result<()> {
    original_receipt(db, binding)?;
    if encode(&task_from(db, &task.id)?)? != encode(task)?
        || encode(&workspace_from(db, binding)?)? != encode(workspace)?
    {
        return Err(changed());
    }
    Ok(())
}

fn check_idle(db: &Connection, workspace_id: &str) -> Result<()> {
    let busy:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM workspace_leases WHERE resource=?1 AND holder!='') OR EXISTS(SELECT 1 FROM sessions WHERE workspace_id=?1 AND state NOT IN('stopped','failed')) OR EXISTS(SELECT 1 FROM command_workspace_fences WHERE workspace_id=?1)",[workspace_id],|row|row.get(0)).map_err(db_error)?;
    if busy {
        return Err(CoreError::new(
            "WORKSPACE_BUSY",
            "Task workspace has an active or unreconciled writer; expiry is not stop evidence",
        ));
    }
    Ok(())
}
fn fresh_view(
    db: &Connection,
    task: &Task,
    binding: &TaskWorkspaceBinding,
) -> Result<TaskWorkspaceBindingView> {
    let original = original_receipt(db, binding)?;
    if binding.state != TaskWorkspaceBindingState::Ready {
        return Err(unknown());
    }
    source_from(db, task, &original)?;
    if managed_root(&original.managed_root, &original.source_pin.canonical_root)?
        != original.managed_root
    {
        return Err(changed());
    }
    let workspace = workspace_from(db, binding)?;
    if workspace.state != "ready" {
        return Err(unknown());
    }
    let pinned = binding.managed_pin.as_ref().ok_or_else(changed)?;
    let recorded = GitWorkspacePin::observed(workspace.identity.ok_or_else(unknown)?)?;
    let current = GitWorkspacePin::observed(inspect(&workspace.root).map_err(|_| changed())?)?;
    // HEAD and dirty bytes are legitimate writable output. The existing Grant/
    // exact-input admission rules must separately validate the selected attempt.
    if !pinned.same_physical_repository(&recorded)
        || !pinned.same_physical_repository(&current)
        || current.common_dir != original.source_pin.common_dir
    {
        return Err(changed());
    }
    check_idle(db, &workspace.id)?;
    Ok(TaskWorkspaceBindingView {
        original_receipt: original,
        binding: binding.clone(),
        task_revision: task.revision,
        observed_managed_pin: Some(current),
        identity_revalidated: true,
        execution_authorized: false,
    })
}

#[derive(Debug)]
enum WorkspaceReservation {
    Existing(Box<TaskWorkspaceBindingView>),
    Pending(Box<TaskWorkspaceReservationReceipt>),
}

impl Store {
    pub fn prepare_task_git_workspace(
        &mut self,
        request: &TaskGitWorkspacePreparation,
        root: &Path,
        key: &str,
    ) -> Result<TaskWorkspaceBindingView> {
        let receipt = match self.reserve_task_git_workspace(request, root, key)? {
            WorkspaceReservation::Existing(view) => return Ok(*view),
            WorkspaceReservation::Pending(receipt) => *receipt,
        };
        let created = self.checkout_task_workspace(&receipt);
        self.finish_task_workspace(&receipt, created.ok())
            .map_err(|mut error| {
                error.side_effect_state = "unknown".into();
                error
            })
    }

    fn checkout_task_workspace(
        &self,
        receipt: &TaskWorkspaceReservationReceipt,
    ) -> Result<WorkspaceIdentity> {
        // Recheck after the durable reservation and immediately before the sole
        // filesystem call. These observations are not an OS sandbox/atomic FS lock.
        let task = task_from(&self.conn, &receipt.task_id)?;
        let binding = task.code_workspace_binding.as_ref().ok_or_else(changed)?;
        if binding.receipt != *receipt
            || binding.state != TaskWorkspaceBindingState::Pending
            || task.revision != receipt.reserved_task_revision
            || task.state != TaskState::Draft
        {
            return Err(changed());
        }
        original_receipt(&self.conn, binding)?;
        let source = source_from(&self.conn, &task, receipt)?;
        let workspace = workspace_from(&self.conn, binding)?;
        if workspace.state != "creating"
            || workspace.identity.is_some()
            || workspace.base_revision != receipt.source_pin.head
            || managed_root(&receipt.managed_root, &source.canonical_root)? != receipt.managed_root
        {
            return Err(changed());
        }
        check_idle(&self.conn, &workspace.id)?;
        // The only production create. Replay and startup do not call this helper.
        agentbaton_workspaces::create_isolated(
            &source.canonical_root,
            &receipt.managed_root,
            &receipt.workspace_id,
            &receipt.source_pin.head,
        )
        .map_err(|_| unknown())
    }

    fn reserve_task_git_workspace(
        &mut self,
        request: &TaskGitWorkspacePreparation,
        root: &Path,
        key: &str,
    ) -> Result<WorkspaceReservation> {
        request_valid(request, key)?;
        let payload = encode(request)?;
        if payload.len() > MAX_RECORD as usize {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Task preparation payload exceeds its limit",
            ));
        }
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut task = task_from(&tx, &request.task_id)?;
        let prior:Option<(Option<String>,Option<String>)>=tx.query_row("SELECT CASE WHEN length(CAST(payload AS BLOB))<=262144 THEN payload END,CASE WHEN length(CAST(result AS BLOB))<=262144 THEN result END FROM idempotency WHERE scope=?1 AND key=?2",params![scope(&task.id),key],|row|Ok((row.get(0)?,row.get(1)?))).optional().map_err(db_error)?;
        if let Some((prior, result)) = prior {
            let prior = bounded(prior)?;
            let result = bounded(result)?;
            if prior != payload {
                return Err(conflict());
            }
            let receipt: TaskWorkspaceReservationReceipt = decode(&result)?;
            let binding = task.code_workspace_binding.as_ref().ok_or_else(changed)?;
            if binding.receipt != receipt
                || managed_root(root, &receipt.source_pin.canonical_root)? != receipt.managed_root
            {
                return Err(changed());
            }
            let view = fresh_view(&tx, &task, binding)?;
            tx.rollback().map_err(db_error)?;
            return Ok(WorkspaceReservation::Existing(Box::new(view)));
        }
        if task.code_workspace_binding.is_some() {
            return Err(conflict());
        }
        let prior_binding: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM idempotency WHERE scope=?1)",
                [scope(&task.id)],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if prior_binding {
            return Err(unknown());
        }

        if task.project_id == "local" {
            return Err(unsupported());
        }
        if task.revision != request.expected_task_revision
            || task.spec_revision != request.expected_spec_revision
        {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task or specification changed before workspace preparation",
            ));
        }
        if task.state != TaskState::Draft || crate::factory::is_factory_task(&tx, &task.id)? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Only an unbound non-Factory draft Task can prepare a new Git workspace",
            ));
        }
        let project = project_from(&tx, &task.project_id)?;
        if project.archived {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Archived projects cannot prepare Task workspaces",
            ));
        }
        if project.revision != request.expected_project_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Project changed before workspace preparation",
            ));
        }
        let registered = GitWorkspacePin::observed(project.identity)?;
        let observed =
            GitWorkspacePin::observed(inspect(&registered.canonical_root).map_err(|_| changed())?)?;
        if !registered.same_physical_repository(&observed)
            || observed != request.expected_source_pin
        {
            return Err(changed());
        }
        let expected_key = format!(
            "code-prep:{:x}",
            Sha256::digest(
                encode(&(
                    "code-preparation.v1",
                    &task.id,
                    task.revision,
                    task.spec_revision,
                    &project.id,
                    project.revision,
                    "local",
                    &observed.canonical_root,
                    observed.device,
                    observed.inode,
                    &observed.common_dir,
                    &observed.head
                ))?
                .as_bytes()
            )
        );
        if expected_key != request.preparation_key {
            return Err(changed());
        }
        let root = managed_root(root, &observed.canonical_root)?;
        let receipt = TaskWorkspaceReservationReceipt {
            version: BINDING_VERSION,
            binding_id: Uuid::new_v4().to_string(),
            workspace_id: Uuid::new_v4().to_string(),
            task_id: task.id.clone(),
            project_id: task.project_id.clone(),
            host_id: "local".into(),
            original_task_revision: task.revision,
            reserved_task_revision: bump(task.revision)?,
            spec_revision: task.spec_revision,
            project_revision: project.revision,
            source_pin: observed,
            managed_root: root,
            preparation_key: request.preparation_key.clone(),
            request_key: key.into(),
            request_digest: digest(&payload),
            created_at: now()?,
        };
        let binding = TaskWorkspaceBinding {
            receipt: receipt.clone(),
            state: TaskWorkspaceBindingState::Pending,
            revision: 1,
            managed_pin: None,
        };
        let workspace = ManagedWorkspace {
            id: receipt.workspace_id.clone(),
            project_id: receipt.project_id.clone(),
            identity: None,
            root: receipt.managed_root.join(&receipt.workspace_id),
            base_revision: receipt.source_pin.head.clone(),
            created_at: receipt.created_at.clone(),
            state: "creating".into(),
            revision: 1,
        };
        task.revision = receipt.reserved_task_revision;
        task.updated_at = now()?;
        task.code_workspace_binding = Some(binding.clone());
        reserve_workspace_row(
            &tx,
            &workspace,
            &creation_key(&receipt.binding_id),
            &payload,
        )?;
        tx.execute(
            "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
            params![scope(&task.id), key, payload, encode(&receipt)?],
        )
        .map_err(db_error)?;
        save_task(&tx, &task)?;
        event(&tx, &task, "task.workspace_preparation_reserved", &binding)?;
        verify_bound_rows(&tx, &task, &binding, &workspace)?;
        tx.commit().map_err(db_error)?;
        Ok(WorkspaceReservation::Pending(Box::new(receipt)))
    }
    fn finish_task_workspace(
        &mut self,
        receipt: &TaskWorkspaceReservationReceipt,
        identity: Option<WorkspaceIdentity>,
    ) -> Result<TaskWorkspaceBindingView> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let mut task = task_from(&tx, &receipt.task_id)?;
        let mut binding = task.code_workspace_binding.clone().ok_or_else(changed)?;
        if binding.receipt != *receipt || binding.state != TaskWorkspaceBindingState::Pending {
            return Err(changed());
        }
        original_receipt(&tx, &binding)?;
        let mut workspace = workspace_from(&tx, &binding)?;
        if workspace.state != "creating" {
            return Err(unknown());
        }
        let valid = task.revision == receipt.reserved_task_revision
            && task.state == TaskState::Draft
            && source_from(&tx, &task, receipt).is_ok()
            && check_idle(&tx, &workspace.id).is_ok();
        let observed = identity
            .clone()
            .map(GitWorkspacePin::observed)
            .transpose()?;
        let current = inspect(&workspace.root)
            .ok()
            .and_then(|value| GitWorkspacePin::observed(value).ok());
        let valid = valid
            && observed.as_ref().is_some_and(|pin| {
                pin.canonical_root == workspace.root
                    && pin.common_dir == receipt.source_pin.common_dir
                    && pin.head == receipt.source_pin.head
            })
            && current == observed;
        workspace.revision = bump(workspace.revision)?;
        workspace.state = if valid { "ready" } else { "outcome_unknown" }.into();
        workspace.identity = identity;
        binding.state = if valid {
            TaskWorkspaceBindingState::Ready
        } else {
            TaskWorkspaceBindingState::OutcomeUnknown
        };
        binding.managed_pin = observed;
        binding.revision = bump(binding.revision)?;
        task.revision = bump(task.revision)?;
        task.updated_at = now()?;
        task.code_workspace_binding = Some(binding.clone());
        finalize_workspace_row(&tx, &workspace)?;
        save_task(&tx, &task)?;
        event(&tx, &task, "task.workspace_preparation_finished", &binding)?;
        verify_bound_rows(&tx, &task, &binding, &workspace)?;
        tx.commit().map_err(db_error)?;
        if !valid {
            return Err(unknown());
        }
        // Final identity is inspected again; a race after commit is a refusal,
        // never permission to reset a directory or rerun creation.
        fresh_view(&self.conn, &task, &binding)
    }
    pub(crate) fn recover_task_workspace_bindings(&mut self) -> Result<()> {
        loop {
            let tx = self
                .conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(db_error)?;
            let rows: Vec<String> = {
                let mut stmt=tx.prepare("SELECT CASE WHEN length(CAST(id AS BLOB))<=128 THEN id END FROM tasks WHERE json_extract(document,'$.code_workspace_binding.state')='pending' LIMIT 100").map_err(db_error)?;
                let values = stmt
                    .query_map([], |row| row.get::<_, Option<String>>(0))
                    .map_err(db_error)?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(db_error)?;
                values
                    .into_iter()
                    .map(bounded)
                    .collect::<Result<Vec<_>>>()?
            };
            if rows.is_empty() {
                tx.commit().map_err(db_error)?;
                return Ok(());
            }
            for id in rows {
                let mut task = task_from(&tx, &id)?;
                let mut b = task.code_workspace_binding.clone().ok_or_else(changed)?;
                original_receipt(&tx, &b)?;
                if task.id != b.receipt.task_id
                    || task.project_id != b.receipt.project_id
                    || task.revision < b.receipt.reserved_task_revision
                {
                    return Err(changed());
                }
                match workspace_from(&tx, &b) {
                    Ok(mut workspace) => {
                        workspace.state = "outcome_unknown".into();
                        workspace.revision = bump(workspace.revision)?;
                        finalize_workspace_row(&tx, &workspace)?;
                    }
                    Err(error) if error.code == "OUTCOME_UNKNOWN" => {}
                    Err(error) => return Err(error),
                }
                b.state = TaskWorkspaceBindingState::OutcomeUnknown;
                b.revision = bump(b.revision)?;
                task.revision = bump(task.revision)?;
                task.updated_at = now()?;
                task.code_workspace_binding = Some(b.clone());
                save_task(&tx, &task)?;
                event(&tx, &task, "task.workspace_recovery_outcome_unknown", &b)?;
            }
            tx.commit().map_err(db_error)?;
        }
    }
}

pub(crate) fn require_task_binding_for_session(
    db: &Connection,
    task: &Task,
    requested_task_id: &str,
    workspace_id: &str,
) -> Result<()> {
    if let Some(binding) = task.code_workspace_binding.as_ref() {
        if task.id != requested_task_id {
            return Err(changed());
        }
        if binding.receipt.workspace_id != workspace_id {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "A bound Task must use its own prepared workspace",
            ));
        }
        fresh_view(db, task, binding)?;
    }
    Ok(())
}

pub(crate) fn recover_for_workspace(
    tx: &Transaction<'_>,
    workspace: &ManagedWorkspace,
) -> Result<()> {
    let rows: Vec<String> = {
        let mut stmt=tx.prepare("SELECT CASE WHEN length(CAST(id AS BLOB))<=128 THEN id END FROM tasks WHERE json_extract(document,'$.code_workspace_binding.receipt.workspace_id')=?1 LIMIT 2").map_err(db_error)?;
        let values = stmt
            .query_map([&workspace.id], |row| row.get::<_, Option<String>>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        values
            .into_iter()
            .map(bounded)
            .collect::<Result<Vec<_>>>()?
    };
    if rows.is_empty() {
        return Ok(());
    }
    if rows.len() != 1 {
        return Err(changed());
    }
    let mut task = task_from(tx, &rows[0])?;
    let mut binding = task.code_workspace_binding.clone().ok_or_else(changed)?;
    original_receipt(tx, &binding)?;
    if task.id != binding.receipt.task_id
        || task.project_id != binding.receipt.project_id
        || task.revision < binding.receipt.reserved_task_revision
    {
        return Err(changed());
    }
    if binding.state != TaskWorkspaceBindingState::Pending
        || workspace_from(tx, &binding)?.id != workspace.id
    {
        return Err(changed());
    }
    binding.state = TaskWorkspaceBindingState::OutcomeUnknown;
    binding.revision = bump(binding.revision)?;
    task.revision = bump(task.revision)?;
    task.updated_at = now()?;
    task.code_workspace_binding = Some(binding.clone());
    save_task(tx, &task)?;
    event(
        tx,
        &task,
        "task.workspace_recovery_outcome_unknown",
        &binding,
    )
}

pub(crate) fn migrate(db: &mut Connection, version: u32) -> Result<()> {
    if version >= 16 {
        return Ok(());
    }
    let current: u32 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(db_error)?;
    if current != 15 {
        return Err(CoreError::new(
            "VERSION_UNSUPPORTED",
            "Task workspace migration requires the complete schema15 predecessor",
        ));
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    if tx
        .prepare("PRAGMA foreign_key_check")
        .map_err(db_error)?
        .exists([])
        .map_err(db_error)?
    {
        return Err(changed());
    }
    // Preserve every predecessor row byte. A version15 file cannot already carry
    // the version16 semantic field, even when a forged field is null.
    let mut stmt = tx
        .prepare("SELECT length(CAST(document AS BLOB)),document,id FROM tasks")
        .map_err(db_error)?;
    let mut rows = stmt.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let size: i64 = row.get(0).map_err(db_error)?;
        if size > MAX_RECORD {
            return Err(changed());
        }
        let raw: String = row.get(1).map_err(db_error)?;
        let value: serde_json::Value = decode(&raw)?;
        if value.get("code_workspace_binding").is_some() {
            return Err(changed());
        }
        let task: Task = decode(&raw)?;
        let id: String = row.get(2).map_err(db_error)?;
        if task.id != id || task.revision == 0 || task.spec_revision == 0 {
            return Err(changed());
        }
    }
    drop(rows);
    drop(stmt);
    tx.pragma_update(None, "user_version", 16u32)
        .map_err(db_error)?;
    tx.commit().map_err(db_error)
}

#[cfg(test)]
#[path = "task_workspace_binding_tests.rs"]
mod tests;
