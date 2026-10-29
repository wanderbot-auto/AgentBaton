//! Read-only preparation for a new code task. This is not execution or copy authority.
use crate::registry::ManagedWorkspace;
use crate::storage::{db_error, decode, encode, Store, TaskState};
use crate::{CoreError, Result};
use agentbaton_workspaces::{inspect, WorkspaceError, WorkspaceIdentity, WorkspaceKind};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const MAX_DOCUMENT_BYTES: i64 = 262_144;
const MAX_CANDIDATE_BYTES: i64 = 4_194_304;
const MAX_CANDIDATES: i64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CodeWorkspacePreparationRequest {
    pub task_id: String,
    pub expected_task_revision: u64,
    pub expected_project_revision: u64,
    pub host_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeWorkspacePreparationState {
    NeedsRegisteredProject,
    NeedsGitWorkspace,
    NeedsDatasetSelection,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CodeWorkspacePreparationReason {
    RegisteredProjectRequired,
    IsolatedGitWorkspaceRequired,
    SelectedDatasetRequired,
    TaskNotDraft,
}

/// Recorded metadata only: no Task assignment, freshness or availability proof.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CodeWorkspaceCandidateInfo {
    pub workspace_id: String,
    pub revision: u64,
    pub root: PathBuf,
    pub recorded_state: String,
    pub has_unreconciled_lease: bool,
    pub assigned_to_task: bool,
    pub identity_revalidated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CodeWorkspacePreparation {
    pub task_id: String,
    pub task_revision: u64,
    pub spec_revision: u64,
    pub project_id: String,
    pub project_revision: Option<u64>,
    pub host_id: String,
    /// Exact Rust filesystem integers; future JS consumers must keep these opaque.
    pub source: Option<WorkspaceIdentity>,
    pub source_head: Option<String>,
    /// Deduplication suggestion, never a Grant, Task assignment or filesystem capability.
    pub preparation_key: Option<String>,
    pub state: CodeWorkspacePreparationState,
    pub reason: CodeWorkspacePreparationReason,
    pub candidates: Vec<CodeWorkspaceCandidateInfo>,
    pub execution_authorized: bool,
    pub creates_workspace: bool,
}

fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Code preparation references are inconsistent; refresh before preparing",
    )
}

fn bounded_document(db: &Connection, sql: &str, id: &str) -> Result<()> {
    let size: Option<i64> = db
        .query_row(sql, [id], |row| row.get(0))
        .map_err(db_error)?;
    if size.is_some_and(|n| n <= 0 || n > MAX_DOCUMENT_BYTES) {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Code preparation document exceeds its read bound",
        ));
    }
    Ok(())
}

fn source_error(error: WorkspaceError) -> CoreError {
    match error {
        WorkspaceError::UnsafeRepository => CoreError::new(
            "PERMISSION_DENIED",
            "Repository inspection requires an explicit supported policy",
        ),
        WorkspaceError::TooLarge | WorkspaceError::ResourceLimit => CoreError::new(
            "RESOURCE_LIMIT",
            "Source inspection exceeded its bounded resources",
        ),
        _ => changed(),
    }
}

fn candidates(db: &Connection, project_id: &str) -> Result<Vec<CodeWorkspaceCandidateInfo>> {
    let (count, bytes, maximum): (i64, i64, i64) = db.query_row(
        "SELECT count(*),COALESCE(sum(length(CAST(document AS BLOB))),0),COALESCE(max(length(CAST(document AS BLOB))),0) FROM managed_workspaces WHERE project_id=?1",
        [project_id], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).map_err(db_error)?;
    if count > MAX_CANDIDATES || bytes > MAX_CANDIDATE_BYTES || maximum > MAX_DOCUMENT_BYTES {
        return Err(CoreError::new(
            "RESOURCE_LIMIT",
            "Code preparation candidate metadata exceeds its read bound",
        ));
    }
    let mut statement = db.prepare(
        "SELECT m.id,m.project_id,m.state,m.root,m.document,EXISTS(SELECT 1 FROM workspace_leases l WHERE l.resource=m.id AND l.holder!='') FROM managed_workspaces m WHERE m.project_id=?1 ORDER BY m.id LIMIT 1000",
    ).map_err(db_error)?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })
        .map_err(db_error)?;
    rows.map(|row| {
        let (id, owner, state, root, raw, held) = row.map_err(db_error)?;
        let workspace: ManagedWorkspace = decode(&raw)?;
        if workspace.id != id
            || workspace.project_id != owner
            || owner != project_id
            || workspace.state != state
            || workspace.root != PathBuf::from(root)
            || !matches!(state.as_str(), "ready" | "creating" | "outcome_unknown")
            || workspace.revision == 0
            || !workspace.root.is_absolute()
            || workspace
                .identity
                .as_ref()
                .is_some_and(|pin| pin.canonical_root != workspace.root)
            || (state == "ready" && workspace.identity.is_none())
        {
            return Err(changed());
        }
        Ok(CodeWorkspaceCandidateInfo {
            workspace_id: id,
            revision: workspace.revision,
            root: workspace.root,
            recorded_state: state,
            has_unreconciled_lease: held,
            assigned_to_task: false,
            identity_revalidated: false,
        })
    })
    .collect()
}

impl Store {
    /// Explicit code preparation only; no IPC endpoint, routing dispatch or new authority.
    /// The read transaction keeps relational references consistent. Source observations and
    /// candidate metadata must be revalidated by the existing create/approval/dispatch paths.
    pub fn resolve_code_workspace_preparation(
        &self,
        request: &CodeWorkspacePreparationRequest,
    ) -> Result<CodeWorkspacePreparation> {
        if request.task_id.is_empty()
            || request.task_id.len() > 128
            || request.expected_task_revision == 0
        {
            return Err(CoreError::new(
                "INVALID_REQUEST",
                "A bounded Task reference and revision are required",
            ));
        }
        if request.host_id != "local" {
            return Err(CoreError::new(
                "CAPABILITY_UNSUPPORTED",
                "Code preparation supports only the existing local host",
            ));
        }
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Deferred)
            .map_err(db_error)?;
        bounded_document(
            &tx,
            "SELECT (SELECT length(CAST(document AS BLOB)) FROM tasks WHERE id=?1)",
            &request.task_id,
        )?;
        let task = self.get(&request.task_id)?;
        if task.id != request.task_id || task.spec_revision == 0 {
            return Err(changed());
        }
        if task.revision != request.expected_task_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Task changed; refresh code preparation",
            ));
        }
        let mut result = CodeWorkspacePreparation {
            task_id: task.id.clone(),
            task_revision: task.revision,
            spec_revision: task.spec_revision,
            project_id: task.project_id.clone(),
            project_revision: None,
            host_id: "local".into(),
            source: None,
            source_head: None,
            preparation_key: None,
            state: CodeWorkspacePreparationState::NeedsRegisteredProject,
            reason: CodeWorkspacePreparationReason::RegisteredProjectRequired,
            candidates: vec![],
            execution_authorized: false,
            creates_workspace: false,
        };
        // A personal draft's project marker is not a Host, path, HOME or cwd.
        if task.project_id == "local" {
            tx.rollback().map_err(db_error)?;
            return Ok(result);
        }
        bounded_document(
            &tx,
            "SELECT (SELECT length(CAST(document AS BLOB)) FROM projects WHERE id=?1)",
            &task.project_id,
        )?;
        let project = self.get_project(&task.project_id)?;
        if project.id != task.project_id || project.revision == 0 {
            return Err(changed());
        }
        if project.revision != request.expected_project_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Project changed; refresh code preparation",
            ));
        }
        if project.archived {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Archived project cannot prepare code execution",
            ));
        }
        let current = inspect(&project.identity.canonical_root).map_err(source_error)?;
        if current.canonical_root != project.identity.canonical_root
            || current.device != project.identity.device
            || current.inode != project.identity.inode
        {
            return Err(changed());
        }
        match (&project.identity.kind, &current.kind) {
            (WorkspaceKind::Directory, WorkspaceKind::Directory) => {
                result.state = CodeWorkspacePreparationState::NeedsDatasetSelection;
                result.reason = CodeWorkspacePreparationReason::SelectedDatasetRequired;
            }
            (
                WorkspaceKind::Git {
                    common_dir: old, ..
                },
                WorkspaceKind::Git {
                    common_dir, head, ..
                },
            ) if old == common_dir => {
                result.state = CodeWorkspacePreparationState::NeedsGitWorkspace;
                result.reason = CodeWorkspacePreparationReason::IsolatedGitWorkspaceRequired;
                result.source_head = Some(head.clone());
                // Deliberately exclude dirty bytes: an exact HEAD worktree never includes them.
                let key = encode(&(
                    "code-preparation.v1",
                    &task.id,
                    task.revision,
                    task.spec_revision,
                    &project.id,
                    project.revision,
                    "local",
                    &current.canonical_root,
                    current.device,
                    current.inode,
                    common_dir,
                    head,
                ))?;
                result.preparation_key =
                    Some(format!("code-prep:{:x}", Sha256::digest(key.as_bytes())));
            }
            _ => return Err(changed()),
        }
        result.project_revision = Some(project.revision);
        result.source = Some(current);
        result.candidates = candidates(&tx, &project.id)?;
        if !matches!(task.state, TaskState::Draft) {
            result.state = CodeWorkspacePreparationState::Blocked;
            result.reason = CodeWorkspacePreparationReason::TaskNotDraft;
            result.preparation_key = None;
        }
        tx.rollback().map_err(db_error)?;
        Ok(result)
    }
}

#[cfg(test)]
#[path = "workspace_resolution_tests.rs"]
mod tests;
