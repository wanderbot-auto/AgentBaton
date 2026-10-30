//! Request-owned read plans. Only authenticated Owner code constructs these;
//! a result is consumed with the original request, nonce and monotonic deadline.
use super::{authority::*, fs::*, records, *};
use crate::{
    storage::{db_error, decode, encode, Store},
    CoreError, Result,
};
use agentbaton_workspaces::{WorkspaceIdentity, WorkspaceKind};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
struct DatabasePin {
    path: PathBuf,
    file: File,
    parent: File,
    device: u64,
    inode: u64,
    parent_device: u64,
    parent_inode: u64,
}
impl DatabasePin {
    fn capture(db: &Connection) -> Result<Self> {
        let path = PathBuf::from(db.path().ok_or_else(changed)?);
        if !path.is_absolute()
            || path.as_os_str().len() > 4096
            || path.canonicalize().ok().as_deref() != Some(path.as_path())
        {
            return Err(changed());
        }
        let flags = (rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::CLOEXEC)
            .bits() as i32;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(flags | rustix::fs::OFlags::DIRECTORY.bits() as i32)
            .open(path.parent().ok_or_else(changed)?)
            .map_err(|_| changed())?;
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(flags)
            .open(&path)
            .map_err(|_| changed())?;
        let p = parent.metadata().map_err(|_| changed())?;
        let m = file.metadata().map_err(|_| changed())?;
        if !p.is_dir()
            || p.mode() & 0o7777 != 0o700
            || !m.is_file()
            || m.nlink() != 1
            || m.mode() & 0o7777 != 0o600
            || m.uid() != rustix::process::geteuid().as_raw()
            || p.uid() != m.uid()
        {
            return Err(denied());
        }
        Ok(Self {
            path,
            file,
            parent,
            device: m.dev(),
            inode: m.ino(),
            parent_device: p.dev(),
            parent_inode: p.ino(),
        })
    }
    fn open_readonly(&self, deadline: Instant) -> Result<Connection> {
        self.recheck()?;
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_ONLY
                | OpenFlags::SQLITE_OPEN_NO_MUTEX
                | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(db_error)?;
        connection
            .busy_timeout(
                Duration::from_secs(3).min(deadline.saturating_duration_since(Instant::now())),
            )
            .map_err(db_error)?;
        connection
            .execute_batch("PRAGMA query_only=ON; BEGIN;")
            .map_err(db_error)?;
        Ok(connection)
    }
    fn recheck(&self) -> Result<()> {
        let held = self.file.metadata().map_err(|_| changed())?;
        let p = self.parent.metadata().map_err(|_| changed())?;
        let current = fs::symlink_metadata(&self.path).map_err(|_| changed())?;
        let current_parent =
            fs::symlink_metadata(self.path.parent().ok_or_else(changed)?).map_err(|_| changed())?;
        if !current.is_file()
            || current.nlink() != 1
            || held.nlink() != 1
            || current.mode() & 0o7777 != 0o600
            || !current_parent.is_dir()
            || current_parent.mode() & 0o7777 != 0o700
            || self.path.canonicalize().ok().as_deref() != Some(self.path.as_path())
            || [
                (&held, self.device, self.inode),
                (&current, self.device, self.inode),
                (&p, self.parent_device, self.parent_inode),
                (&current_parent, self.parent_device, self.parent_inode),
            ]
            .iter()
            .any(|(m, d, i)| {
                m.dev() != *d || m.ino() != *i || m.uid() != rustix::process::geteuid().as_raw()
            })
        {
            return Err(changed());
        }
        Ok(())
    }
}

const LEASE_PROJECTION:&str="SELECT json_array(resource,holder,fencing_token,expires_at) AS value FROM workspace_leases WHERE resource IN (SELECT value FROM json_each(?1)) ORDER BY resource";
struct QueryProof {
    sql: &'static str,
    args: Vec<String>,
    rows: Vec<String>,
}
impl QueryProof {
    fn capture(
        db: &Connection,
        sql: &'static str,
        args: Vec<String>,
        remaining: &mut (usize, usize),
    ) -> Result<Self> {
        let check = format!("SELECT count(*),COALESCE(sum(octet_length(value)),0) FROM ({sql})");
        let (count, bytes): (i64, i64) = db
            .query_row(&check, rusqlite::params_from_iter(&args), |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(db_error)?;
        let count = usize::try_from(count).map_err(|_| limited())?;
        let bytes = usize::try_from(bytes).map_err(|_| limited())?;
        if count > remaining.1 || bytes > remaining.0 {
            return Err(limited());
        }
        remaining.0 -= bytes;
        remaining.1 -= count;
        let mut statement = db.prepare(sql).map_err(db_error)?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(&args), |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        Ok(Self { sql, args, rows })
    }
    fn recheck(&self, db: &Connection, remaining: &mut (usize, usize)) -> Result<()> {
        if Self::capture(db, self.sql, self.args.clone(), remaining)?.rows != self.rows {
            return Err(changed());
        }
        Ok(())
    }
}
#[derive(Clone)]
pub(crate) enum IntegrationReadIntent {
    Context(IntegrationContextQuery),
    Proposal(IntegrationPolicySelection),
    Finalize { proposal_id: String, revision: u64 },
    Start { policy_id: String, revision: u64 },
}
pub(crate) struct IntegrationReadPlan {
    intent: IntegrationReadIntent,
    material: IntegrationScopeMaterial,
    available: Vec<IntegrationValidatorBinding>,
    database: DatabasePin,
    queries: Vec<QueryProof>,
    request_hash: String,
    as_of_sequence: u64,
}
pub(crate) struct IntegrationReadProof {
    pub(super) intent: IntegrationReadIntent,
    pub(super) context: IntegrationContextView,
    material_digest: String,
    available_digest: String,
    database: DatabasePin,
    queries: Vec<QueryProof>,
    observation: FsObservation,
    request_hash: String,
    deadline: Instant,
}
fn projection(db: &Connection, material: &IntegrationScopeMaterial) -> Result<Vec<QueryProof>> {
    let mut result = vec![];
    let mut remaining = (4_194_304, 100_000);
    for grant in &material.grant_lineage {
        for id in &grant.scope.connection_ids {
            result.push(QueryProof::capture(
                db,
                "SELECT document AS value FROM connections WHERE id=?1",
                vec![id.clone()],
                &mut remaining,
            )?);
        }
        for id in &grant.scope.workspace_ids {
            result.push(QueryProof::capture(
                db,
                "SELECT document AS value FROM managed_workspaces WHERE id=?1",
                vec![id.clone()],
                &mut remaining,
            )?);
        }
    }
    for(scope,sql)in[(format!("root:{}",material.root_task_id),"SELECT json_array(total_limit,parallel_limit,consumed,active) AS value FROM budgets WHERE scope=?1"),(format!("project:{}",material.project.id),"SELECT json_array(total_limit,parallel_limit,consumed,active) AS value FROM budgets WHERE scope=?1")] {
  result.push(QueryProof::capture(db,sql,vec![scope],&mut remaining)?);
 }
    result.push(QueryProof::capture(db,"SELECT json_array(id,state,document) AS value FROM dispatch_intents WHERE json_extract(document,'$.root_task_id')=?1 ORDER BY id",vec![material.root_task_id.clone()],&mut remaining)?);
    result.push(QueryProof::capture(
        db,
        LEASE_PROJECTION,
        vec![encode(&material.grant.scope.workspace_ids)?],
        &mut remaining,
    )?);
    Ok(result)
}
impl Store {
    pub(crate) fn prepare_integration_scope_read(
        &self,
        intent: IntegrationReadIntent,
        raw_request: &str,
    ) -> Result<IntegrationReadPlan> {
        if raw_request.len() > MAX_INTEGRATION_BYTES {
            return Err(limited());
        }
        let (task, scope) = match &intent {
            IntegrationReadIntent::Context(q) => (q.task_id.clone(), q.goal_scope_id.clone()),
            IntegrationReadIntent::Proposal(s) => {
                selection_shape(s)?;
                (s.task_id.clone(), s.goal_scope_id.clone())
            }
            IntegrationReadIntent::Finalize {
                proposal_id,
                revision,
            } => {
                let p: crate::sessions::ApprovalProposal =
                    raw(&self.conn, "approval_proposals", proposal_id)?;
                if p.kind != "integration" {
                    return Err(denied());
                }
                if p.revision != *revision {
                    return Err(CoreError::new(
                        "STALE_REVISION",
                        "Integration proposal changed",
                    ));
                }
                if p.consumed {
                    return Err(changed());
                }
                let s: IntegrationPolicySnapshot =
                    serde_json::from_value(p.snapshot).map_err(|_| changed())?;
                (s.intent.task_id, s.intent.goal_scope_id)
            }
            IntegrationReadIntent::Start {
                policy_id,
                revision,
            } => {
                let p = records::policy(&self.conn, policy_id)?;
                if p.revision != *revision {
                    return Err(CoreError::new(
                        "STALE_REVISION",
                        "Integration policy changed",
                    ));
                }
                (p.selection.task_id, p.selection.goal_scope_id)
            }
        };
        key(&task)?;
        key(&scope)?;
        let material = self.integration_scope_material(&scope, &task)?;
        if let IntegrationReadIntent::Proposal(s) = &intent {
            selection_cas(s, &material)?;
        }
        let available = self.integration_available_validators(&material)?;
        let mut queries = projection(&self.conn, &material)?;
        if let IntegrationReadIntent::Finalize { proposal_id, .. } = &intent {
            queries.push(QueryProof::capture(&self.conn,"SELECT json_array(document,COALESCE(result,'null')) AS value FROM approval_proposals WHERE id=?1",vec![proposal_id.clone()],&mut(1_048_576,2))?);
        }
        if let IntegrationReadIntent::Start { policy_id, .. } = &intent {
            queries.push(QueryProof::capture(
                &self.conn,
                "SELECT document AS value FROM integration_policies WHERE id=?1",
                vec![policy_id.clone()],
                &mut (1_048_576, 2),
            )?);
        }
        Ok(IntegrationReadPlan {
            intent,
            queries,
            database: DatabasePin::capture(&self.conn)?,
            request_hash: sha(&raw_request)?,
            as_of_sequence: records::sequence(&self.conn)?,
            material,
            available,
        })
    }
}
pub(super) fn selection_shape(s: &IntegrationPolicySelection) -> Result<()> {
    if !id(&s.task_id)
        || !id(&s.goal_scope_id)
        || s.expected_task_revision == 0
        || s.expected_spec_revision == 0
        || s.expected_scope_revision == 0
        || s.host_id != "local"
        || !valid_ref(&s.target_ref)
        || !oid(&s.expected_target_head)
        || !(1..=8).contains(&s.max_rounds)
        || !(1..=3_600_000).contains(&s.ttl_ms)
        || s.stage_reservations.validators.is_empty()
        || s.stage_reservations.validators.len() > 64
    {
        return Err(invalid());
    }
    let mut refs = std::collections::BTreeSet::new();
    for v in &s.stage_reservations.validators {
        if !refs.insert(encode(&v.validator)?) {
            return Err(invalid());
        }
    }
    for r in std::iter::once(&s.stage_reservations.preparation)
        .chain(
            s.stage_reservations
                .validators
                .iter()
                .map(|v| &v.reservation),
        )
        .chain(std::iter::once(&s.stage_reservations.cas))
    {
        if r.runtime_ms == 0
            || r.output_bytes == 0
            || r.tokens.is_some()
            || r.cost_microunits.is_some()
        {
            return Err(invalid());
        }
    }
    if encode(s)?.len() > MAX_INTEGRATION_BYTES {
        return Err(limited());
    }
    Ok(())
}
fn selection_cas(s: &IntegrationPolicySelection, m: &IntegrationScopeMaterial) -> Result<()> {
    if s.expected_task_revision != m.task.revision
        || s.expected_spec_revision != m.task.spec_revision
        || s.expected_scope_revision != m.scope_revision
    {
        return Err(CoreError::new(
            "STALE_REVISION",
            "Integration Task, spec or Goal scope changed before read admission",
        ));
    }
    Ok(())
}
pub(super) fn oid(s: &str) -> bool {
    matches!(s.len(), 40 | 64)
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
pub(super) fn valid_ref(s: &str) -> bool {
    s.starts_with("refs/heads/")
        && s.len() <= 1024
        && s.is_ascii()
        && !s.ends_with('.')
        && !s.ends_with('/')
        && !s.contains("..")
        && !s.contains("@{")
        && s.split('/')
            .all(|c| !c.is_empty() && !c.starts_with('.') && !c.ends_with(".lock"))
        && s.bytes()
            .all(|b| b > 32 && b < 127 && !b"~^:?*[\\".contains(&b))
}
fn git_pin(identity: &WorkspaceIdentity) -> Result<IntegrationGitPin> {
    let WorkspaceKind::Git {
        common_dir, head, ..
    } = &identity.kind
    else {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Integration requires a registered managed Git workspace",
        ));
    };
    let common = super::fs::directory_identity(common_dir)?;
    Ok(IntegrationGitPin {
        canonical_root: identity.canonical_root.clone(),
        device: identity.device.to_string(),
        inode: identity.inode.to_string(),
        common_dir: common_dir.clone(),
        common_device: common.0.to_string(),
        common_inode: common.1.to_string(),
        head: head.clone(),
    })
}
fn inspect_exact(
    expected: &WorkspaceIdentity,
    base: &str,
    allow_dirty: bool,
    program: &IntegrationProgramPin,
    control: &IntegrationReadControl,
) -> Result<WorkspaceIdentity> {
    let current = super::fs::inspect_git(&expected.canonical_root, program, control)?;
    let (
        WorkspaceKind::Git {
            common_dir: a,
            head,
            dirty,
        },
        WorkspaceKind::Git { common_dir: b, .. },
    ) = (&current.kind, &expected.kind)
    else {
        return Err(changed());
    };
    if current.canonical_root != expected.canonical_root
        || current.device != expected.device
        || current.inode != expected.inode
        || a != b
        || head != base
        || (!allow_dirty && *dirty)
    {
        return Err(changed());
    }
    Ok(current)
}
impl IntegrationReadPlan {
    pub(crate) fn execute(
        self,
        cancel: Arc<AtomicBool>,
        deadline: Instant,
    ) -> Result<IntegrationReadProof> {
        let control = IntegrationReadControl { cancel, deadline };
        control.check()?;
        let connection = self.database.open_readonly(deadline)?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version != 17 {
            return Err(changed());
        }
        let mut remaining = (4_194_304, 100_000);
        for q in &self.queries {
            control.check()?;
            q.recheck(&connection, &mut remaining)?;
        }
        let reader = Store {
            conn: connection,
            data_input: None,
            graph_input: None,
            graph_capture_clocks: Default::default(),
            factory_recovery_queue_epochs: vec![],
            factory_read_control: None,
        };
        let material =
            reader.integration_scope_material(&self.material.scope_id, &self.material.task.id)?;
        let available = reader.integration_available_validators(&material)?;
        if sha(&material)? != sha(&self.material)? || sha(&available)? != sha(&self.available)? {
            return Err(changed());
        }
        let mut roots = std::collections::BTreeSet::new();
        roots.insert(material.project.identity.canonical_root.clone());
        for grant in &material.grant_lineage {
            for workspace in grant.workspace_bindings.values() {
                if !matches!(workspace.kind, WorkspaceKind::Git { .. }) {
                    return Err(CoreError::new(
                        "CAPABILITY_UNSUPPORTED",
                        "An original Grant Directory binding needs a separate genuine data witness",
                    ));
                }
                roots.insert(workspace.canonical_root.clone());
            }
        }
        let mut observed =
            FsObservation::capture(&roots.into_iter().collect::<Vec<_>>(), &control)?;
        let (program, program_pin) = system_git(&control)?;
        observed.add_leaf(program_pin);
        for grant in &material.grant_lineage {
            control.check()?;
            let first_ws = grant.scope.workspace_ids.first().ok_or_else(denied)?;
            let first_conn = grant.scope.connection_ids.first().ok_or_else(denied)?;
            if crate::policy::validate_grant_live_from_with_data(
                &reader.conn,
                &grant.id,
                &grant.project_id,
                first_ws,
                first_conn,
                None,
            )? != *grant
            {
                return Err(changed());
            }
            for c in &grant.scope.connection_ids {
                let connection = reader.get_connection(c)?;
                crate::runtime_config::approved_plan(&connection)?;
                let pin = LeafPin::capture(&connection.executable)?;
                let (hash, _) = pin.digest(268435456, &control)?;
                if hash != connection.executable_sha256 {
                    return Err(changed());
                }
                observed.add_leaf(pin);
            }
            for (id, pinned) in &grant.workspace_bindings {
                let workspace: crate::registry::ManagedWorkspace =
                    raw(&reader.conn, "managed_workspaces", id)?;
                inspect_exact(
                    pinned,
                    &workspace.base_revision,
                    id == &material.workspace.id,
                    &program,
                    &control,
                )?;
            }
        }
        let project_head = match &material.project.identity.kind {
            WorkspaceKind::Git { head, .. } => head,
            _ => return Err(changed()),
        };
        let project = inspect_exact(
            &material.project.identity,
            project_head,
            false,
            &program,
            &control,
        )?;
        let source = inspect_exact(
            material.workspace.identity.as_ref().ok_or_else(changed)?,
            &material.workspace.base_revision,
            true,
            &program,
            &control,
        )?;
        for input in &material.inputs {
            artifact_pin(&source, input, &control, &mut observed)?;
        }
        for validator in material.validators.iter().chain(&available) {
            for file in [validator.program.as_ref(), validator.script.as_ref()]
                .into_iter()
                .flatten()
            {
                let identity = super::fs::directory_identity(&file.canonical_root)?;
                if identity.0.to_string() != file.root_device
                    || identity.1.to_string() != file.root_inode
                {
                    return Err(changed());
                }
                let pin =
                    LeafPin::capture(&file.canonical_root.join(&file.requirement.relative_path))?;
                let (hash, _) = pin.digest(file.requirement.max_bytes, &control)?;
                if file.requirement.expected_sha256.as_deref() != Some(hash.as_str()) {
                    return Err(changed());
                }
                observed.add_leaf(pin);
            }
        }
        let selected = match &self.intent {
            IntegrationReadIntent::Proposal(s) => Some(s.clone()),
            IntegrationReadIntent::Finalize { proposal_id, .. } => {
                let p: crate::sessions::ApprovalProposal =
                    raw(&reader.conn, "approval_proposals", proposal_id)?;
                Some(
                    serde_json::from_value::<IntegrationPolicySnapshot>(p.snapshot)
                        .map_err(|_| changed())?
                        .intent,
                )
            }
            IntegrationReadIntent::Start { policy_id, .. } => {
                Some(records::policy(&reader.conn, policy_id)?.selection)
            }
            IntegrationReadIntent::Context(_) => None,
        };
        if let Some(selection) = selected {
            observe_target(&material.workspace.root, &program, &selection, &control)?;
        }
        let source = git_pin(&source)?;
        let workspace_binding = match &material.workspace_ownership {
            IntegrationWorkspaceMaterial::TaskManaged(r) => {
                IntegrationWorkspaceBinding::TaskManaged { receipt: r.clone() }
            }
            IntegrationWorkspaceMaterial::FactoryDelegated(d) => {
                IntegrationWorkspaceBinding::FactoryDelegated {
                    factory_scope_id: d.factory_scope_id.clone(),
                    factory_scope_digest: d.factory_scope_digest.clone(),
                    factory_run_id: d.factory_run_id.clone(),
                    child_spec_revision: d.child_spec_revision,
                    lineage: d.lineage.clone(),
                    delegation_event_sequence: d.delegation_event_sequence,
                    delegation_document_digest: d.delegation_document_digest.clone(),
                    original_request: d.original_request.clone(),
                    original_handle: d.original_handle.clone(),
                    workspace_id: d.workspace_id.clone(),
                    registered_pin: source.clone(),
                    source_base: d.source_base.clone(),
                    managed_creation_ref: d.managed_creation_ref.clone(),
                }
            }
        };
        let mut lineage = vec![];
        for grant in &material.grant_lineage {
            let mut workspace_bindings = std::collections::BTreeMap::new();
            for (id, w) in &grant.workspace_bindings {
                workspace_bindings.insert(id.clone(), git_pin(w)?);
            }
            lineage.push(IntegrationGrantBinding {
                id: grant.id.clone(),
                root_grant_id: grant.root_grant_id.clone(),
                parent_id: grant.parent_id.clone(),
                revision: grant.revision,
                approval_id: grant.approval_id.clone(),
                approved_at: grant.approved_at.clone(),
                expires_at_ms: grant.expires_at_ms,
                scope: grant.scope.clone(),
                connection_bindings: grant.connection_bindings.clone(),
                workspace_bindings,
                original_document_sha256: sha(grant)?,
            });
        }
        let mut bindings = IntegrationStaticBindings {
            task_id: material.task.id.clone(),
            task_goal: material.task.goal.clone(),
            spec_revision: material.task.spec_revision,
            project_id: material.project.id.clone(),
            project_revision: material.project.revision,
            project_source: git_pin(&project)?,
            workspace_id: material.workspace.id.clone(),
            workspace_revision: material.workspace.revision,
            source: source.clone(),
            workspace_binding,
            goal_scope_id: material.scope_id.clone(),
            goal_scope_revision: material.scope_revision,
            goal_scope_digest: material.scope_digest.clone(),
            goal_scope_expires_at_ms: material.expires_at_ms,
            root_task_id: material.root_task_id.clone(),
            root_grant_id: material.grant.root_grant_id.clone(),
            factory_scope_id: material.factory_scope_id.clone(),
            ancestors: material.ancestors.clone(),
            grant_lineage: lineage,
            root_budget: material.root_budget.clone(),
            project_budget: material.project_budget.clone(),
            inputs: material.inputs.clone(),
            outputs: material.outputs.clone(),
            validators: material.validators.clone(),
            system_git: program,
            repository_digest: sha(
                &json!({"host":"local","project":material.project.id,"common_dir":source.common_dir,"common_device":source.common_device,"common_inode":source.common_inode}),
            )?,
            binding_digest: String::new(),
        };
        bindings.binding_digest = sha(&bindings)?;
        control.check()?;
        observed.recheck()?;
        self.database.recheck()?;
        reader.conn.execute_batch("ROLLBACK;").map_err(db_error)?;
        let context = IntegrationContextView {
            task_revision: material.task.revision,
            bindings,
            as_of_sequence: self.as_of_sequence,
            shared_usage: material.usage.clone(),
            available_validators: available.clone(),
            preview_only: true,
        };
        if encode(&context)?.len() > MAX_INTEGRATION_BYTES {
            return Err(limited());
        }
        Ok(IntegrationReadProof {
            intent: self.intent,
            context,
            material_digest: sha(&material)?,
            available_digest: sha(&available)?,
            database: self.database,
            queries: self.queries,
            observation: observed,
            request_hash: self.request_hash,
            deadline,
        })
    }
}
fn artifact_pin(
    root: &WorkspaceIdentity,
    requirement: &agentbaton_workspaces::validation::ArtifactRequirement,
    control: &IntegrationReadControl,
    observed: &mut FsObservation,
) -> Result<()> {
    if requirement.relative_path.as_os_str().len() > 4096
        || requirement
            .relative_path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
        || requirement.max_bytes == 0
        || requirement.max_bytes > 67_108_864
    {
        return Err(invalid());
    }
    let pin = LeafPin::capture(&root.canonical_root.join(&requirement.relative_path))?;
    let (hash, _) = pin.digest(requirement.max_bytes, control)?;
    if requirement.expected_sha256.as_deref() != Some(hash.as_str()) {
        return Err(changed());
    }
    observed.add_leaf(pin);
    Ok(())
}
impl IntegrationReadProof {
    pub(crate) fn intent(&self) -> &IntegrationReadIntent {
        &self.intent
    }
    pub(crate) fn context(&self) -> &IntegrationContextView {
        &self.context
    }
    pub(super) fn seal_observation(&self) -> Result<()> {
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration observation expired before commit",
            ));
        }
        self.database.recheck()?;
        self.observation.recheck()?;
        if Instant::now() >= self.deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration observation expired during its final metadata recheck",
            ));
        }
        Ok(())
    }
    pub(crate) fn verify(&self, store: &Store, raw_request: &str, deadline: Instant) -> Result<()> {
        if Instant::now() >= self.deadline
            || Instant::now() >= deadline
            || self.deadline != deadline
        {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration read arrived after its original request deadline",
            ));
        }
        if sha(&raw_request)? != self.request_hash {
            return Err(denied());
        }
        self.database.recheck()?;
        let material = store.integration_scope_material(
            &self.context.bindings.goal_scope_id,
            &self.context.bindings.task_id,
        )?;
        if sha(&material)? != self.material_digest
            || sha(&store.integration_available_validators(&material)?)? != self.available_digest
        {
            return Err(changed());
        }
        let mut remaining = (4_194_304, 100_000);
        for q in &self.queries {
            q.recheck(&store.conn, &mut remaining)?;
        }
        for grant in &material.grant_lineage {
            crate::policy::observe_grant_bindings(&store.conn, grant)?;
        }
        self.observation.recheck()?;
        if Instant::now() >= deadline {
            return Err(CoreError::new(
                "DEADLINE_EXCEEDED",
                "Integration proof expired at admission",
            ));
        }
        Ok(())
    }
}

fn observe_target(
    root: &Path,
    program: &IntegrationProgramPin,
    s: &IntegrationPolicySelection,
    control: &IntegrationReadControl,
) -> Result<()> {
    use std::ffi::OsStr;
    selection_shape(s)?;
    super::fs::git(
        root,
        program,
        &[OsStr::new("check-ref-format"), OsStr::new(&s.target_ref)],
        control,
    )?;
    let reference = format!("{}^{{commit}}", s.target_ref);
    let bytes = super::fs::git(
        root,
        program,
        &[
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&reference),
        ],
        control,
    )?;
    let head = std::str::from_utf8(&bytes)
        .map_err(|_| changed())?
        .trim_end_matches('\n');
    if head != s.expected_target_head {
        return Err(CoreError::new(
            "INPUT_CHANGED",
            "Integration target head changed before approval or stage admission",
        ));
    }
    let worktrees = super::fs::git(
        root,
        program,
        &[
            OsStr::new("worktree"),
            OsStr::new("list"),
            OsStr::new("--porcelain"),
            OsStr::new("-z"),
        ],
        control,
    )?;
    for field in worktrees.split(|b| *b == 0) {
        if field.strip_prefix(b"branch ") == Some(s.target_ref.as_bytes()) {
            return Err(CoreError::new(
                "CAPABILITY_UNSUPPORTED",
                "Integration target is checked out; the local ref-only path cannot update it",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod lease_projection_tests {
    use super::*;
    #[test]
    fn existing_schema_lease_expiry_column_is_projected_and_rechecked() {
        // Actual predecessor DDL, with only an inert isolated row. This is a
        // SQL/proof regression, not a session/permission or UnixIPC substitute.
        let db = crate::storage::integration_schema16_reference().unwrap();
        db.execute_batch("PRAGMA foreign_keys=OFF;INSERT INTO workspace_leases(resource,holder,fencing_token,expires_at) VALUES('fixture-workspace','',7,12345);").unwrap();
        let proof = QueryProof::capture(
            &db,
            LEASE_PROJECTION,
            vec!["[\"fixture-workspace\"]".into()],
            &mut (4_194_304, 100_000),
        )
        .unwrap();
        assert_eq!(proof.rows, vec!["[\"fixture-workspace\",\"\",7,12345]"]);
        proof.recheck(&db, &mut (4_194_304, 100_000)).unwrap();
        db.execute(
            "UPDATE workspace_leases SET expires_at=12346 WHERE resource='fixture-workspace'",
            [],
        )
        .unwrap();
        assert_eq!(
            proof
                .recheck(&db, &mut (4_194_304, 100_000))
                .unwrap_err()
                .code,
            "INPUT_CHANGED"
        );
    }
}
