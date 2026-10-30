//! Owner-only preview of existing local branch slots. No approval or execution
//! authority is derived from this observation; policy proposal reads again.
use super::{authority::*, fs::*, reads::DatabasePin, *};
use crate::{
    registry::{ManagedWorkspace, Project},
    storage::{db_error, Store, Task},
    CoreError, Result,
};
use agentbaton_workspaces::WorkspaceKind;
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    ffi::OsStr,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

#[derive(Serialize)]
struct TargetMaterial {
    task: Task,
    project: Project,
    workspace: ManagedWorkspace,
    ownership: IntegrationWorkspaceMaterial,
}
impl Store {
    fn integration_target_material(&self, task_id: &str) -> Result<TargetMaterial> {
        let version: u32 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version != 17 {
            return Err(changed());
        }
        let task: Task = raw(&self.conn, "tasks", task_id)?;
        let project: Project = raw(&self.conn, "projects", &task.project_id)?;
        if task.id != task_id || project.id != task.project_id || project.archived {
            return Err(changed());
        }
        let (workspace_id, delegated) = if let Some(binding) = &task.code_workspace_binding {
            (binding.receipt.workspace_id.clone(), None)
        } else {
            let scope: Option<String> = self.conn.query_row(
                "SELECT CASE WHEN octet_length(scope_id)<=128 THEN scope_id END FROM factory_task_lineage WHERE task_id=?1",
                [task_id], |r| r.get(0),
            ).optional().map_err(db_error)?.flatten();
            let scope = scope.ok_or_else(|| {
                CoreError::new(
                    "CAPABILITY_UNSUPPORTED",
                    "Integration targets require genuine managed Task or Factory ownership",
                )
            })?;
            let delegated = self.integration_factory_workspace_material(&scope, task_id)?;
            (delegated.workspace_id.clone(), Some(delegated))
        };
        let workspace: ManagedWorkspace = raw(&self.conn, "managed_workspaces", &workspace_id)?;
        if workspace.id != workspace_id
            || workspace.project_id != project.id
            || workspace.state != "ready"
        {
            return Err(changed());
        }
        let ownership = match delegated {
            Some(delegated) => {
                if delegated.source_base != workspace.base_revision {
                    return Err(changed());
                }
                IntegrationWorkspaceMaterial::FactoryDelegated(delegated)
            }
            None => IntegrationWorkspaceMaterial::TaskManaged(
                crate::task_workspace_binding::verify_task_binding_for_integration(
                    &self.conn, &task, &workspace,
                )?,
            ),
        };
        let identity = workspace.identity.as_ref().ok_or_else(changed)?;
        if workspace.root != identity.canonical_root
            || !matches!(identity.kind, WorkspaceKind::Git { .. })
        {
            return Err(changed());
        }
        super::fences::check_workspace_fences(&self.conn, identity, None)?;
        super::fences::check_workspace_fences(&self.conn, &project.identity, None)?;
        Ok(TargetMaterial {
            task,
            project,
            workspace,
            ownership,
        })
    }
    pub(crate) fn prepare_integration_targets_read(
        &self,
        query: IntegrationTargetQuery,
        raw_request: &str,
    ) -> Result<IntegrationTargetPlan> {
        query_shape(&query)?;
        let material = self.integration_target_material(&query.task_id)?;
        Ok(IntegrationTargetPlan {
            query,
            material_digest: sha(&material)?,
            database: DatabasePin::capture(&self.conn)?,
            request_hash: sha(&raw_request)?,
        })
    }
}
fn query_shape(query: &IntegrationTargetQuery) -> Result<()> {
    if !id(&query.task_id)
        || !(1..=MAX_INTEGRATION_PAGE).contains(&query.limit.unwrap_or(MAX_INTEGRATION_PAGE))
        || query.after_ref.as_ref().is_some_and(|r| !preview_ref(r))
    {
        return Err(invalid());
    }
    Ok(())
}
// Listing is a preview, including UTF-8 branch spellings. The current approved
// integration implementation separately rejects unsupported non-ASCII targets.
fn preview_ref(reference: &str) -> bool {
    reference.starts_with("refs/heads/")
        && reference.len() <= 1024
        && !reference.ends_with('.')
        && !reference.ends_with('/')
        && !reference.contains("..")
        && !reference.contains("@{")
        && reference
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
        && reference
            .chars()
            .all(|c| !c.is_control() && !c.is_whitespace() && !"~^:?*[\\".contains(c))
}

pub(crate) struct IntegrationTargetPlan {
    query: IntegrationTargetQuery,
    material_digest: String,
    database: DatabasePin,
    request_hash: String,
}
pub(crate) struct IntegrationTargetProof {
    query: IntegrationTargetQuery,
    material_digest: String,
    database: DatabasePin,
    observation: FsObservation,
    request_hash: String,
    deadline: Instant,
    targets: Vec<IntegrationTargetView>,
}
impl IntegrationTargetPlan {
    pub(crate) fn execute(
        self,
        cancel: Arc<AtomicBool>,
        deadline: Instant,
    ) -> Result<IntegrationTargetProof> {
        let control = IntegrationReadControl { cancel, deadline };
        control.check()?;
        let connection = self.database.open_readonly(deadline)?;
        let reader = Store {
            conn: connection,
            data_input: None,
            graph_input: None,
            graph_capture_clocks: Default::default(),
            factory_recovery_queue_epochs: vec![],
            factory_read_control: None,
        };
        let material = reader.integration_target_material(&self.query.task_id)?;
        if sha(&material)? != self.material_digest {
            return Err(changed());
        }
        let roots = vec![
            material.workspace.root.clone(),
            material.project.identity.canonical_root.clone(),
        ];
        let mut observation = FsObservation::capture(&roots, &control)?;
        let (program, pin) = system_git(&control)?;
        observation.add_leaf(pin);
        super::reads::inspect_exact(
            material.workspace.identity.as_ref().ok_or_else(changed)?,
            &material.workspace.base_revision,
            true,
            &program,
            &control,
        )?;
        let WorkspaceKind::Git { head, .. } = &material.project.identity.kind else {
            return Err(changed());
        };
        super::reads::inspect_exact(&material.project.identity, head, true, &program, &control)?;
        let branches = git(
            &material.workspace.root,
            &program,
            &[
                OsStr::new("for-each-ref"),
                OsStr::new("--sort=refname"),
                OsStr::new("--format=%(refname)%00%(objectname)%00%(objecttype)%00%(symref)%00"),
                OsStr::new("refs/heads/"),
            ],
            &control,
        )?;
        let worktrees = git(
            &material.workspace.root,
            &program,
            &[
                OsStr::new("worktree"),
                OsStr::new("list"),
                OsStr::new("--porcelain"),
                OsStr::new("-z"),
            ],
            &control,
        )?;
        let targets = parse_targets(&branches, &worktrees, &self.query)?;
        observation.recheck()?;
        if sha(&reader.integration_target_material(&self.query.task_id)?)? != self.material_digest {
            return Err(changed());
        }
        control.check()?;
        reader.conn.execute_batch("ROLLBACK;").map_err(db_error)?;
        self.database.recheck()?;
        Ok(IntegrationTargetProof {
            query: self.query,
            material_digest: self.material_digest,
            database: self.database,
            observation,
            request_hash: self.request_hash,
            deadline,
            targets,
        })
    }
}
impl IntegrationTargetProof {
    pub(crate) fn verify(
        self,
        store: &Store,
        raw_request: &str,
        deadline: Instant,
    ) -> Result<Vec<IntegrationTargetView>> {
        if Instant::now() >= deadline || self.deadline != deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration targets observation exceeded its original request deadline",
            ));
        }
        if sha(&raw_request)? != self.request_hash {
            return Err(denied());
        }
        self.database.recheck()?;
        if sha(&store.integration_target_material(&self.query.task_id)?)? != self.material_digest {
            return Err(changed());
        }
        self.observation.recheck()?;
        if Instant::now() >= deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration targets metadata seal exceeded its original request deadline",
            ));
        }
        Ok(self.targets)
    }
}
fn parse_targets(
    branches: &[u8],
    worktrees: &[u8],
    query: &IntegrationTargetQuery,
) -> Result<Vec<IntegrationTargetView>> {
    query_shape(query)?;
    if branches
        .len()
        .checked_add(worktrees.len())
        .is_none_or(|bytes| bytes > 4_194_304)
        || (!branches.is_empty() && branches.last() != Some(&b'\n'))
        || (!worktrees.is_empty() && worktrees.last() != Some(&0))
    {
        return Err(limited());
    }
    let mut checked = BTreeSet::new();
    for field in worktrees.split(|b| *b == 0) {
        if let Some(reference) = field.strip_prefix(b"branch ") {
            let reference = std::str::from_utf8(reference).map_err(|_| changed())?;
            if !preview_ref(reference) || !checked.insert(reference.to_owned()) {
                return Err(changed());
            }
        }
    }
    let mut targets = vec![];
    let mut previous = None::<String>;
    for record in branches.split(|b| *b == b'\n').filter(|r| !r.is_empty()) {
        let fields = record.split(|b| *b == 0).collect::<Vec<_>>();
        if fields.len() != 5 || !fields[4].is_empty() {
            return Err(changed());
        }
        let reference = std::str::from_utf8(fields[0]).map_err(|_| changed())?;
        let head = std::str::from_utf8(fields[1]).map_err(|_| changed())?;
        if !preview_ref(reference)
            || !super::reads::oid(head)
            || fields[2] != b"commit"
            || previous.as_deref().is_some_and(|last| last >= reference)
        {
            return Err(changed());
        }
        previous = Some(reference.to_owned());
        // Symbolic aliases are not independent preapprovable branch slots.
        if !fields[3].is_empty() {
            if !preview_ref(std::str::from_utf8(fields[3]).map_err(|_| changed())?) {
                return Err(changed());
            }
            continue;
        }
        if query
            .after_ref
            .as_deref()
            .is_some_and(|after| reference <= after)
        {
            continue;
        }
        if targets.len() < query.limit.unwrap_or(MAX_INTEGRATION_PAGE) as usize {
            targets.push(IntegrationTargetView {
                target_ref: reference.to_owned(),
                head: head.to_owned(),
                checked_out: checked.contains(reference),
            });
        }
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> IntegrationTargetQuery {
        IntegrationTargetQuery {
            task_id: "task".into(),
            after_ref: None,
            limit: Some(1),
        }
    }
    fn branch(reference: &str, kind: &str, symbolic: &str) -> Vec<u8> {
        format!("{reference}\0{}\0{kind}\0{symbolic}\0\n", "a".repeat(40)).into_bytes()
    }
    #[test]
    fn direct_targets_page_without_approving_symbolic_aliases() {
        let mut branches = branch("refs/heads/alias", "commit", "refs/heads/main");
        branches.extend(branch("refs/heads/main", "commit", ""));
        branches.extend(branch("refs/heads/next", "commit", ""));
        let worktrees = b"worktree /not-published\0HEAD ignored\0branch refs/heads/main\0\0";
        let first = parse_targets(&branches, worktrees, &query()).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].target_ref, "refs/heads/main");
        assert!(first[0].checked_out);
        let mut next = query();
        next.after_ref = Some(first[0].target_ref.clone());
        let second = parse_targets(&branches, worktrees, &next).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].target_ref, "refs/heads/next");
        assert!(!second[0].checked_out);
    }
    #[test]
    fn malformed_or_ambiguous_git_records_are_not_silently_skipped() {
        for branches in [
            branch("refs/heads/main", "blob", ""),
            branch("refs/heads/bad..name", "commit", ""),
            branch("refs/heads/main", "commit", "not-a-head"),
            b"refs/heads/main\0bad\0commit\0\0\n".to_vec(),
            b"unterminated".to_vec(),
        ] {
            assert!(parse_targets(&branches, b"", &query()).is_err());
        }
        let mut duplicate = branch("refs/heads/main", "commit", "");
        duplicate.extend(branch("refs/heads/main", "commit", ""));
        assert!(parse_targets(&duplicate, b"", &query()).is_err());
        let mut late_bad = branch("refs/heads/main", "commit", "");
        late_bad.extend(branch("refs/heads/z", "blob", ""));
        assert!(
            parse_targets(&late_bad, b"", &query()).is_err(),
            "a full page must not hide later malformed records"
        );
    }
    #[test]
    fn invalid_limits_and_duplicate_worktree_branch_bindings_are_rejected() {
        let branches = branch("refs/heads/main", "commit", "");
        for limit in [0, 101] {
            let mut q = query();
            q.limit = Some(limit);
            assert!(parse_targets(&branches, b"", &q).is_err());
        }
        assert!(parse_targets(
            &branches,
            b"branch refs/heads/main\0branch refs/heads/main\0",
            &query()
        )
        .is_err());
    }
    #[test]
    fn detached_worktree_has_no_invented_branch_and_unicode_is_only_preview() {
        let branches = branch("refs/heads/branĉh", "commit", "");
        let targets = parse_targets(
            &branches,
            b"worktree /not-published\0detached\0\0",
            &query(),
        )
        .unwrap();
        assert_eq!(targets[0].target_ref, "refs/heads/branĉh");
        assert!(!targets[0].checked_out);
        assert!(!super::super::reads::valid_ref(&targets[0].target_ref));
    }
}
