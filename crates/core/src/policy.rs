//! Server-owned execution grants. Scope limits apply to Baton-mediated actions, not an OS sandbox.
//! UTC admission expiry is separate from monotonic runtime timeout accounting in the supervisor.
use crate::execution::ExecutionBinding;
use crate::registry::{ManagedWorkspace, Project};
use crate::storage::{db_error, decode, encode, now, Event, Store};
use crate::{CoreError, Result};
use agentbaton_providers::connections::ProtocolRoute;
use agentbaton_workspaces::{inspect, WorkspaceIdentity, WorkspaceKind};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub enum NetworkScope {
    #[default]
    Deny,
    Allowlisted {
        hosts: Vec<String>,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantScope {
    pub workspace_ids: Vec<String>,
    pub connection_ids: Vec<String>,
    pub host_ids: Vec<String>,
    pub max_attempts: u32,
    pub max_concurrency: u32,
    pub max_duration_ms: u64,
    #[serde(default)]
    pub recipients: Vec<String>,
    #[serde(default)]
    pub network: NetworkScope,
    #[serde(default)]
    pub max_depth: u32,
    #[serde(default)]
    pub max_children: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantedConnection {
    #[serde(default = "crate::routes::default_route")]
    pub protocol_route: ProtocolRoute,
    pub revision: u64,
    pub environment_id: String,
    pub identity_ref: String,
    pub host_id: String,
    pub config_revision: String,
    pub executable_sha256: String,
    pub capability_snapshot_id: String,
}
/// A generated approval record never asserts native account identity or OS enforcement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionGrant {
    pub id: String,
    pub project_id: String,
    pub root_grant_id: String,
    pub parent_id: Option<String>,
    pub depth: u32,
    pub revision: u64,
    pub approval_id: String,
    pub approved_at: String,
    pub expires_at_ms: i64,
    pub revoked: bool,
    pub scope: GrantScope,
    pub connection_bindings: BTreeMap<String, GrantedConnection>,
    pub workspace_bindings: BTreeMap<String, WorkspaceIdentity>,
    pub enforcement: GrantEnforcement,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantEnforcement {
    pub baton_mediated_scope: bool,
    pub os_filesystem_sandbox: bool,
    pub os_network_sandbox: bool,
    pub native_identity_isolation: bool,
}
impl Default for GrantEnforcement {
    fn default() -> Self {
        Self {
            baton_mediated_scope: true,
            os_filesystem_sandbox: false,
            os_network_sandbox: false,
            native_identity_isolation: false,
        }
    }
}
fn denied(message: &str) -> CoreError {
    CoreError::new("PERMISSION_DENIED", message)
}
fn milliseconds() -> Result<i64> {
    i64::try_from(OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
        .map_err(|_| CoreError::new("INTERNAL_ERROR", "Clock outside supported range"))
}
fn valid_list(values: &[String], required: bool) -> bool {
    (!required || !values.is_empty())
        && values.len() <= 256
        && values
            .iter()
            .all(|v| !v.trim().is_empty() && v.len() <= 512)
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn subset(child: &[String], parent: &[String]) -> bool {
    child.iter().all(|v| parent.contains(v))
}
fn valid_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}
impl GrantScope {
    pub fn validate(&self) -> Result<()> {
        if !valid_list(&self.workspace_ids, true)
            || !valid_list(&self.connection_ids, true)
            || !valid_list(&self.host_ids, true)
            || !valid_list(&self.recipients, false)
            || self.max_attempts == 0
            || self.max_attempts > 100_000
            || self.max_concurrency == 0
            || self.max_concurrency > self.max_attempts
            || self.max_duration_ms == 0
            || self.max_duration_ms > 3_600_000
            || self.max_depth > 32
            || self.max_children > 1000
        {
            return Err(CoreError::new("INVALID_REQUEST","Grant scope and budgets must be explicit, unique, nonempty where required, and bounded"));
        }
        if let NetworkScope::Allowlisted { hosts } = &self.network {
            if !valid_list(hosts, true) || hosts.iter().any(|h| !valid_hostname(h)) {
                return Err(CoreError::new("INVALID_REQUEST","Network destinations must be explicit lowercase hostnames without wildcards or URL parts"));
            }
        }
        Ok(())
    }
    pub fn is_narrower_than(&self, parent: &Self) -> Result<()> {
        self.validate()?;
        parent.validate()?;
        let network_subset = match (&self.network, &parent.network) {
            (NetworkScope::Deny, _) => true,
            (NetworkScope::Allowlisted { hosts: a }, NetworkScope::Allowlisted { hosts: b }) => {
                subset(a, b)
            }
            _ => false,
        };
        if !subset(&self.workspace_ids, &parent.workspace_ids)
            || !subset(&self.connection_ids, &parent.connection_ids)
            || !subset(&self.host_ids, &parent.host_ids)
            || !subset(&self.recipients, &parent.recipients)
            || !network_subset
            || self.max_attempts > parent.max_attempts
            || self.max_concurrency > parent.max_concurrency
            || self.max_duration_ms > parent.max_duration_ms
            || self.max_depth > parent.max_depth
            || self.max_children > parent.max_children
        {
            return Err(denied("Delegation cannot add identities, connections, hosts, workspaces, recipients, network destinations or budget"));
        }
        Ok(())
    }
}
pub fn require_enforcement(
    grant: &ExecutionGrant,
    filesystem: bool,
    network: bool,
    identity: bool,
) -> Result<()> {
    if (filesystem && !grant.enforcement.os_filesystem_sandbox)
        || (network && !grant.enforcement.os_network_sandbox)
        || (identity && !grant.enforcement.native_identity_isolation)
    {
        return Err(CoreError::new("ENFORCEMENT_UNAVAILABLE","Required OS or native identity enforcement has not been verified; advisory scope is insufficient"));
    }
    Ok(())
}
fn read_document<T: for<'a> Deserialize<'a>>(
    conn: &Connection,
    table: &str,
    id: &str,
) -> Result<T> {
    let sql = match table {
        "execution_grants" => "SELECT document FROM execution_grants WHERE id=?1",
        "projects" => "SELECT document FROM projects WHERE id=?1",
        "managed_workspaces" => "SELECT document FROM managed_workspaces WHERE id=?1",
        "connections" => "SELECT document FROM connections WHERE id=?1",
        "tasks" => "SELECT document FROM tasks WHERE id=?1",
        _ => return Err(CoreError::new("INTERNAL_ERROR", "Unknown policy table")),
    };
    let raw: Option<String> = conn
        .query_row(sql, [id], |r| r.get(0))
        .optional()
        .map_err(db_error)?;
    decode(&raw.ok_or_else(|| {
        CoreError::new(
            "NOT_FOUND",
            "A required registered policy reference does not exist",
        )
    })?)
}
fn connection_binding(conn: &Connection, id: &str) -> Result<GrantedConnection> {
    let connection: Value = read_document(conn, "connections", id)?;
    let provider = connection["provider"]
        .as_str()
        .ok_or_else(|| denied("Connection Provider is invalid"))?;
    let expected_route = crate::routes::route_for_provider(provider)?;
    let actual_route: ProtocolRoute = match connection.get("protocol_route") {
        Some(route) => serde_json::from_value(route.clone())
            .map_err(|_| denied("Connection protocol route is invalid"))?,
        None if provider == "custom_acp" => ProtocolRoute::CustomAcp,
        None => {
            return Err(denied(
                "Native connection must explicitly select its protocol route",
            ))
        }
    };
    if connection["id"] != id
        || connection["probe_state"] != "ready"
        || connection["host_id"] != "local"
        || actual_route != expected_route
    {
        return Err(denied(
            "Connection must be a ready registered local connection with an explicit matching protocol route",
        ));
    }
    let text = |name: &str| -> Result<String> {
        connection[name]
            .as_str()
            .filter(|v| !v.is_empty() && v.len() <= 512)
            .map(str::to_owned)
            .ok_or_else(|| {
                CoreError::new(
                    "INPUT_CHANGED",
                    "Registered connection binding is incomplete",
                )
            })
    };
    Ok(GrantedConnection {
        protocol_route: actual_route,
        revision: connection["revision"]
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or_else(|| CoreError::new("INPUT_CHANGED", "Connection revision is invalid"))?,
        environment_id: text("environment_id")?,
        identity_ref: text("identity_ref")?,
        host_id: text("host_id")?,
        config_revision: text("config_revision")?,
        executable_sha256: text("executable_sha256")?,
        capability_snapshot_id: text("capability_snapshot_id")?,
    })
}

// Existing writers may have changed tracked files. Re-running Git status here
// can both reject their own edits and hold the single-writer actor while a Git
// command runs. Check the pinned directory and Git linkage using bounded local
// filesystem reads instead; this is not a fresh clean-input/adoption check.
pub(crate) fn live_workspace_matches(
    root: &Path,
    pinned: &WorkspaceIdentity,
) -> std::io::Result<bool> {
    fn pointer(path: &Path) -> std::io::Result<String> {
        let mut bytes = Vec::new();
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            )
            .open(path)?;
        if !file.metadata()?.is_file() {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        String::from_utf8(bytes).map_err(|_| std::io::ErrorKind::InvalidData.into())
    }
    fn resolve(base: &Path, value: &str) -> std::io::Result<PathBuf> {
        let value = value.trim_end_matches(['\n', '\r']);
        if value.is_empty() || value.contains(['\0', '\n', '\r']) {
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        fs::canonicalize(base.join(value))
    }
    let canonical = fs::canonicalize(root)?;
    let metadata = fs::metadata(&canonical)?;
    if !metadata.is_dir()
        || canonical != pinned.canonical_root
        || metadata.dev() != pinned.device
        || metadata.ino() != pinned.inode
    {
        return Ok(false);
    }
    let WorkspaceKind::Git { common_dir, .. } = &pinned.kind else {
        return Ok(false);
    };
    let dotgit = canonical.join(".git");
    let git_metadata = fs::symlink_metadata(&dotgit)?;
    let gitdir = if git_metadata.is_dir() {
        fs::canonicalize(&dotgit)?
    } else if git_metadata.is_file() {
        let text = pointer(&dotgit)?;
        let Some(target) = text.strip_prefix("gitdir: ") else {
            return Ok(false);
        };
        let gitdir = resolve(&canonical, target)?;
        // A linked worktree must still point back to this exact working tree.
        if resolve(&gitdir, &pointer(&gitdir.join("gitdir"))?)? != dotgit {
            return Ok(false);
        }
        gitdir
    } else {
        return Ok(false);
    };
    let common_pointer = gitdir.join("commondir");
    let actual_common = match fs::symlink_metadata(&common_pointer) {
        Ok(_) => resolve(&gitdir, &pointer(&common_pointer)?)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => gitdir,
        Err(error) => return Err(error),
    };
    Ok(&actual_common == common_dir)
}
fn validate_registered(
    conn: &Connection,
    project_id: &str,
    scope: &GrantScope,
    check_input: bool,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<(
    BTreeMap<String, GrantedConnection>,
    BTreeMap<String, WorkspaceIdentity>,
)> {
    scope.validate()?;
    let project: Project = read_document(conn, "projects", project_id)?;
    if project.id != project_id || project.archived {
        return Err(denied("Execution project is archived or inconsistent"));
    }
    let project_budget: Option<(u32, u32)> = conn
        .query_row(
            "SELECT total_limit,parallel_limit FROM budgets WHERE scope=?1",
            [format!("project:{project_id}")],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    if project_budget.is_some_and(|(total, parallel)| {
        scope.max_attempts > total || scope.max_concurrency > parallel
    }) {
        return Err(denied("Execution scope cannot widen the existing shared project budget; a separate approved project policy revision is required"));
    }
    if scope.host_ids.iter().any(|h| h != "local") {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Only the registered local host is currently supported",
        ));
    }
    let mut workspaces = BTreeMap::new();
    for id in &scope.workspace_ids {
        let workspace: ManagedWorkspace = read_document(conn, "managed_workspaces", id)?;
        if workspace.id != *id || workspace.project_id != project_id || workspace.state != "ready" {
            return Err(denied("Workspace is not a ready member of this project"));
        }
        let pinned = workspace.identity.clone().ok_or_else(|| {
            CoreError::new(
                "OUTCOME_UNKNOWN",
                "Workspace identity has not been confirmed",
            )
        })?;
        if matches!(pinned.kind, WorkspaceKind::Directory) {
            crate::data_workspaces::require_data_input_from(conn, &workspace, data)?;
            workspaces.insert(id.clone(), pinned);
            continue;
        }
        if !check_input {
            if !live_workspace_matches(&workspace.root, &pinned).unwrap_or(false) {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Managed workspace directory or repository linkage changed",
                ));
            }
            workspaces.insert(id.clone(), pinned);
            continue;
        }
        let current = inspect(&workspace.root)
            .map_err(|_| CoreError::new("INPUT_CHANGED", "Workspace cannot be safely inspected"))?;
        if current.device != pinned.device
            || current.inode != pinned.inode
            || current.canonical_root != pinned.canonical_root
        {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Workspace was moved or replaced",
            ));
        }
        match (&current.kind, &pinned.kind) {
            (
                WorkspaceKind::Git {
                    common_dir: a,
                    head,
                    dirty,
                    ..
                },
                WorkspaceKind::Git { common_dir: b, .. },
            ) if a == b => {
                if check_input && (head != &workspace.base_revision || *dirty) {
                    return Err(CoreError::new("INPUT_CHANGED","Managed workspace input changed; snapshot and validate a new attempt before dispatch"));
                }
            }
            _ => {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Managed repository identity changed",
                ))
            }
        }
        workspaces.insert(id.clone(), pinned);
    }
    let mut connections = BTreeMap::new();
    for id in &scope.connection_ids {
        let binding = connection_binding(conn, id)?;
        if !scope.host_ids.contains(&binding.host_id) {
            return Err(denied("Connection host is outside the approved scope"));
        }
        connections.insert(id.clone(), binding);
    }
    Ok((connections, workspaces))
}

/// Canonical server-generated references for a native-owner confirmation proposal. Knowing this
/// snapshot does not grant execution authority; finalization must verify owner confirmation too.
pub(crate) fn grant_proposal_refs_with_data(
    conn: &Connection,
    project_id: &str,
    workspace_id: &str,
    connection_id: &str,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<Value> {
    let scope = GrantScope {
        workspace_ids: vec![workspace_id.into()],
        connection_ids: vec![connection_id.into()],
        host_ids: vec!["local".into()],
        max_attempts: 1,
        max_concurrency: 1,
        max_duration_ms: 1,
        recipients: vec![],
        network: NetworkScope::Deny,
        max_depth: 0,
        max_children: 0,
    };
    let (connections, _) = validate_registered(conn, project_id, &scope, true, data)?;
    let project: Project = read_document(conn, "projects", project_id)?;
    let workspace: ManagedWorkspace = read_document(conn, "managed_workspaces", workspace_id)?;
    let connection: Value = read_document(conn, "connections", connection_id)?;
    let identity = if matches!(
        workspace.identity.as_ref().map(|i| &i.kind),
        Some(WorkspaceKind::Directory)
    ) {
        workspace
            .identity
            .clone()
            .ok_or_else(|| CoreError::new("INPUT_CHANGED", "Data root identity unavailable"))?
    } else {
        inspect(&workspace.root).map_err(|_| {
            CoreError::new(
                "INPUT_CHANGED",
                "Workspace changed before approval snapshot",
            )
        })?
    };
    let mut refs = json!({"project_revision":project.revision,"project_name":project.name,"workspace_revision":workspace.revision,"workspace_identity":identity,"connection":connections[connection_id],"execution_recipient":{"connection_id":connection_id,"name":connection["name"],"provider":connection["provider"],"protocol_route":connection["protocol_route"],"executable":connection["executable"],"arguments":connection["arguments"],"executable_sha256":connection["executable_sha256"],"identity_status":connection["identity_status"],"config_status":connection["config_status"]}});
    if matches!(identity.kind, WorkspaceKind::Directory) {
        refs["data_input"] = crate::data_workspaces::data_input_approval_view(conn, &workspace)?;
    }
    Ok(refs)
}
fn event(tx: &Transaction<'_>, grant: &ExecutionGrant, kind: &str, source: &str) -> Result<()> {
    let value = Event {
        sequence: 0,
        event_id: Uuid::new_v4().to_string(),
        aggregate_id: grant.id.clone(),
        revision: grant.revision,
        kind: kind.into(),
        occurred_at: now()?,
        payload: json!({"grant_id":grant.id,"project_id":grant.project_id,"root_grant_id":grant.root_grant_id,"approval_id":grant.approval_id,"scope":grant.scope,"enforcement":grant.enforcement}),
        source: source.into(),
    };
    tx.execute("INSERT INTO events(document) VALUES(?1)", [encode(&value)?])
        .map_err(db_error)?;
    Ok(())
}
fn replay(
    conn: &Connection,
    scope: &str,
    key: &str,
    payload: &str,
) -> Result<Option<ExecutionGrant>> {
    if key.is_empty() || key.len() > 256 {
        return Err(CoreError::new(
            "INVALID_REQUEST",
            "A bounded idempotency key is required",
        ));
    }
    let old: Option<(String, String)> = conn
        .query_row(
            "SELECT payload,result FROM idempotency WHERE scope=?1 AND key=?2",
            params![scope, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    if let Some((previous, result)) = old {
        if previous != payload {
            return Err(CoreError::new(
                "CONFIG_CONFLICT",
                "Grant request key was already used with another request",
            ));
        }
        return Ok(Some(decode(&result)?));
    }
    Ok(None)
}
fn remember(
    tx: &Transaction<'_>,
    scope: &str,
    key: &str,
    payload: &str,
    grant: &ExecutionGrant,
) -> Result<()> {
    tx.execute(
        "INSERT INTO idempotency(scope,key,payload,result) VALUES(?1,?2,?3,?4)",
        params![scope, key, payload, encode(grant)?],
    )
    .map_err(db_error)?;
    Ok(())
}
pub(crate) fn checked_lineage(conn: &Connection, id: &str) -> Result<ExecutionGrant> {
    let first: ExecutionGrant = read_document(conn, "execution_grants", id)?;
    let mut current = first.clone();
    let mut seen = BTreeSet::new();
    let now_ms = milliseconds()?;
    loop {
        if !seen.insert(current.id.clone()) || seen.len() > 33 {
            return Err(denied(
                "Grant ancestry is cyclic or exceeds the maximum depth",
            ));
        }
        if current.revoked || current.expires_at_ms <= now_ms {
            return Err(denied(
                "Execution grant or an ancestor is revoked or expired",
            ));
        }
        current.scope.validate()?;
        if let Some(parent_id) = &current.parent_id {
            let parent: ExecutionGrant = read_document(conn, "execution_grants", parent_id)?;
            if parent.id != *parent_id
                || current.project_id != parent.project_id
                || current.root_grant_id != parent.root_grant_id
                || current.depth != parent.depth.saturating_add(1)
                || current.depth > parent.scope.max_depth
                || current.expires_at_ms > parent.expires_at_ms
                || current.approval_id != parent.approval_id
            {
                return Err(denied("Delegated grant lineage is inconsistent"));
            }
            current.scope.is_narrower_than(&parent.scope)?;
            current = parent;
        } else {
            if current.id != first.root_grant_id
                || current.root_grant_id != current.id
                || current.depth != 0
            {
                return Err(denied("Root grant identity is inconsistent"));
            }
            break;
        }
    }
    Ok(first)
}

/// Supervisor budget lookup uses persisted ancestry but does not apply new-admission clean-input
/// checks to an existing writer. This must not be used as proof that a process has stopped.
pub(crate) fn grant_from_for_supervisor(conn: &Connection, id: &str) -> Result<ExecutionGrant> {
    read_document(conn, "execution_grants", id)
}

/// Call in the same IMMEDIATE transaction as session insertion. Every ancestor's narrowed
/// allowance covers its whole subtree, so a descendant cannot obtain a fresh parent budget.
pub(crate) fn validate_ancestor_grant_budgets_from(conn: &Connection, id: &str) -> Result<()> {
    let mut current = checked_lineage(conn, id)?;
    let mut seen = BTreeSet::new();
    loop {
        if !seen.insert(current.id.clone()) || seen.len() > 33 {
            return Err(denied("Grant budget lineage is cyclic or too deep"));
        }
        let (total, active): (i64, i64) = conn.query_row(
            "WITH RECURSIVE descendants(id) AS (SELECT ?1 UNION SELECT g.id FROM execution_grants g JOIN descendants d ON g.parent_id=d.id) SELECT count(*),COALESCE(SUM(CASE WHEN s.state NOT IN('stopped','failed') THEN 1 ELSE 0 END),0) FROM sessions s WHERE s.grant_id IN (SELECT id FROM descendants)",
            [&current.id], |r|Ok((r.get(0)?,r.get(1)?))
        ).map_err(db_error)?;
        if total >= i64::from(current.scope.max_attempts)
            || active >= i64::from(current.scope.max_concurrency)
        {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Shared grant ancestor attempt or concurrency budget exhausted",
            ));
        }
        let Some(parent_id) = &current.parent_id else {
            return Ok(());
        };
        current = read_document(conn, "execution_grants", parent_id)?;
    }
}
/// Validate within the dispatch reservation transaction; caller cannot supply approved snapshots.
pub(crate) fn validate_grant_from(
    conn: &Connection,
    grant_id: &str,
    project_id: &str,
    workspace_id: &str,
    connection_id: &str,
) -> Result<ExecutionGrant> {
    validate_grant_from_with_data(
        conn,
        grant_id,
        project_id,
        workspace_id,
        connection_id,
        None,
    )
}
pub(crate) fn validate_grant_from_with_data(
    conn: &Connection,
    grant_id: &str,
    project_id: &str,
    workspace_id: &str,
    connection_id: &str,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<ExecutionGrant> {
    validate_grant_inner(
        conn,
        grant_id,
        project_id,
        workspace_id,
        connection_id,
        true,
        data,
    )
}

/// Admission of another turn for an existing managed writer permits its own filesystem edits.
/// This still enforces active ancestry, project, identity and connection snapshots. Cancellation
/// must remain available independently and must not call this admission predicate.
pub(crate) fn validate_grant_live_from_with_data(
    conn: &Connection,
    grant_id: &str,
    project_id: &str,
    workspace_id: &str,
    connection_id: &str,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<ExecutionGrant> {
    validate_grant_inner(
        conn,
        grant_id,
        project_id,
        workspace_id,
        connection_id,
        false,
        data,
    )
}

fn validate_grant_inner(
    conn: &Connection,
    grant_id: &str,
    project_id: &str,
    workspace_id: &str,
    connection_id: &str,
    check_input: bool,
    data: Option<&crate::data_workspaces::DataInputWitness>,
) -> Result<ExecutionGrant> {
    let grant = checked_lineage(conn, grant_id)?;
    if grant.id != grant_id
        || grant.project_id != project_id
        || !grant.scope.workspace_ids.iter().any(|v| v == workspace_id)
        || !grant
            .scope
            .connection_ids
            .iter()
            .any(|v| v == connection_id)
    {
        return Err(denied(
            "Dispatch is outside the approved project, workspace or connection",
        ));
    }
    let (connections, workspaces) =
        validate_registered(conn, project_id, &grant.scope, check_input, data)?;
    if connections != grant.connection_bindings || workspaces != grant.workspace_bindings {
        return Err(CoreError::new("INPUT_CHANGED","Approved connection, account, environment or workspace binding changed; obtain a new grant"));
    }
    Ok(grant)
}

impl Store {
    /// A read-only budget preview. Actual admission must check again in its reservation transaction.
    pub fn validate_grant_budget(&self, id: &str) -> Result<()> {
        validate_ancestor_grant_budgets_from(&self.conn, id)
    }
    /// Trusted-core convenience wrapper. The IPC surface must use native-owner-confirmed proposal
    /// finalization and `create_grant_checked`, rather than accepting renderer consent booleans.
    /// No Provider-supplied approval ID, scope snapshot or approved flag is authority.
    #[allow(clippy::too_many_arguments)]
    pub fn create_grant(
        &mut self,
        project_id: &str,
        workspace_id: &str,
        connection_id: &str,
        max_attempts: u32,
        max_concurrency: u32,
        max_duration_ms: u64,
        consent: bool,
        key: &str,
    ) -> Result<ExecutionGrant> {
        self.create_grant_checked(
            project_id,
            workspace_id,
            connection_id,
            max_attempts,
            max_concurrency,
            max_duration_ms,
            consent,
            key,
            None,
        )
    }
    /// Trusted-core finalization only: expected references originate in a persisted server proposal,
    /// and `consent` is supplied only after native-owner confirmation, never renderer claims.
    #[allow(clippy::too_many_arguments)]
    pub fn create_grant_checked(
        &mut self,
        project_id: &str,
        workspace_id: &str,
        connection_id: &str,
        max_attempts: u32,
        max_concurrency: u32,
        max_duration_ms: u64,
        consent: bool,
        key: &str,
        expected_refs: Option<&Value>,
    ) -> Result<ExecutionGrant> {
        if !consent {
            return Err(denied("Explicit local user execution consent is required"));
        }
        let scope = GrantScope {
            workspace_ids: vec![workspace_id.into()],
            connection_ids: vec![connection_id.into()],
            host_ids: vec!["local".into()],
            max_attempts,
            max_concurrency,
            max_duration_ms,
            recipients: vec![],
            network: NetworkScope::Deny,
            max_depth: 0,
            max_children: 0,
        };
        let payload =
            encode(&json!({"project_id":project_id,"scope":scope,"approval_refs":expected_refs}))?;
        let idem_scope = format!("local:{project_id}:grants.create");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(expected) = expected_refs {
            let current = grant_proposal_refs_with_data(
                &tx,
                project_id,
                workspace_id,
                connection_id,
                self.data_input.as_ref(),
            )?;
            if &current != expected {
                return Err(CoreError::new("INPUT_CHANGED","Owner-confirmed project, workspace or connection snapshot changed; prepare a new proposal"));
            }
        }
        if let Some(old) = replay(&tx, &idem_scope, key, &payload)? {
            return Ok(old);
        }
        let (connection_bindings, workspace_bindings) =
            validate_registered(&tx, project_id, &scope, true, self.data_input.as_ref())?;
        let id = Uuid::new_v4().to_string();
        let expires_at_ms = milliseconds()?
            .checked_add(scope.max_duration_ms as i64)
            .ok_or_else(|| CoreError::new("INVALID_REQUEST", "Grant deadline overflow"))?;
        let grant = ExecutionGrant {
            id: id.clone(),
            project_id: project_id.into(),
            root_grant_id: id,
            parent_id: None,
            depth: 0,
            revision: 1,
            approval_id: Uuid::new_v4().to_string(),
            approved_at: now()?,
            expires_at_ms,
            revoked: false,
            scope,
            connection_bindings,
            workspace_bindings,
            enforcement: GrantEnforcement::default(),
        };
        tx.execute(
            "INSERT INTO execution_grants(id,project_id,parent_id,document) VALUES(?1,?2,NULL,?3)",
            params![grant.id, project_id, encode(&grant)?],
        )
        .map_err(db_error)?;
        event(&tx, &grant, "execution_grant.approved", "user_action")?;
        remember(&tx, &idem_scope, key, &payload, &grant)?;
        tx.commit().map_err(db_error)?;
        Ok(grant)
    }
    pub fn grant_proposal_snapshot(
        &self,
        project_id: &str,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<Value> {
        grant_proposal_refs_with_data(
            &self.conn,
            project_id,
            workspace_id,
            connection_id,
            self.data_input.as_ref(),
        )
    }
    pub fn get_grant(&self, id: &str) -> Result<ExecutionGrant> {
        read_document(&self.conn, "execution_grants", id)
    }
    pub fn list_grants(&self, project_id: &str) -> Result<Vec<ExecutionGrant>> {
        let _: Project = read_document(&self.conn, "projects", project_id)?;
        let count: i64 = self
            .conn
            .query_row(
                "SELECT count(*) FROM execution_grants WHERE project_id=?1",
                [project_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        if count > 1000 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Grant listing exceeds 1000 records",
            ));
        }
        let mut statement=self.conn.prepare("SELECT document FROM execution_grants WHERE project_id=?1 ORDER BY rowid DESC LIMIT 1000").map_err(db_error)?;
        let rows = statement
            .query_map([project_id], |r| r.get::<_, String>(0))
            .map_err(db_error)?;
        rows.map(|r| decode(&r.map_err(db_error)?)).collect()
    }
    pub fn validate_grant(
        &self,
        id: &str,
        project_id: &str,
        workspace_id: &str,
        connection_id: &str,
    ) -> Result<ExecutionGrant> {
        validate_grant_from_with_data(
            &self.conn,
            id,
            project_id,
            workspace_id,
            connection_id,
            self.data_input.as_ref(),
        )
    }
    pub fn validate_execution_grant(
        &self,
        id: &str,
        expected_revision: u64,
        task_id: &str,
        binding: &ExecutionBinding,
    ) -> Result<ExecutionGrant> {
        let task: crate::storage::Task = read_document(&self.conn, "tasks", task_id)?;
        let grant = validate_grant_from_with_data(
            &self.conn,
            id,
            &task.project_id,
            &binding.workspace_id,
            &binding.connection_id,
            self.data_input.as_ref(),
        )?;
        if grant.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Execution grant changed before dispatch",
            ));
        }
        let approved = grant
            .connection_bindings
            .get(&binding.connection_id)
            .ok_or_else(|| denied("Connection binding missing"))?;
        if binding.environment_id != approved.environment_id
            || binding.identity_ref != approved.identity_ref
            || binding.host_id != approved.host_id
            || binding.config_revision != approved.config_revision
            || binding.capability_snapshot_id != approved.capability_snapshot_id
            || binding.policy_revision != format!("{}:{}", grant.id, grant.revision)
        {
            return Err(denied("Attempt binding must exactly match the server-resolved grant revision and connection snapshot"));
        }
        let workspace: ManagedWorkspace =
            read_document(&self.conn, "managed_workspaces", &binding.workspace_id)?;
        if binding.input_revision != workspace.base_revision {
            return Err(CoreError::new(
                "INPUT_CHANGED",
                "Attempt input revision differs from the fixed workspace base",
            ));
        }
        Ok(grant)
    }
    pub fn revoke_grant(
        &mut self,
        id: &str,
        expected_revision: u64,
        key: &str,
    ) -> Result<ExecutionGrant> {
        let payload = encode(&json!({"expected_revision":expected_revision}))?;
        let scope = format!("local:{id}:grants.revoke");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = replay(&tx, &scope, key, &payload)? {
            return Ok(old);
        }
        let mut grant: ExecutionGrant = read_document(&tx, "execution_grants", id)?;
        if grant.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Grant changed; refresh before revocation",
            ));
        }
        if !grant.revoked {
            grant.revoked = true;
            grant.revision += 1;
            tx.execute(
                "UPDATE execution_grants SET document=?1 WHERE id=?2",
                params![encode(&grant)?, id],
            )
            .map_err(db_error)?;
            event(&tx, &grant, "execution_grant.revoked", "user_action")?;
        }
        remember(&tx, &scope, key, &payload, &grant)?;
        tx.commit().map_err(db_error)?;
        Ok(grant)
    }
    /// Trusted scheduler-only delegation; no IPC method exposes broad scope construction.
    /// Children inherit the same approval and shared root grant budget, never a fresh allowance.
    pub fn delegate_grant(
        &mut self,
        parent_id: &str,
        expected_revision: u64,
        scope: GrantScope,
        key: &str,
    ) -> Result<ExecutionGrant> {
        let payload = encode(&json!({"expected_revision":expected_revision,"scope":scope}))?;
        let idem_scope = format!("local:{parent_id}:grants.delegate");
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        if let Some(old) = replay(&tx, &idem_scope, key, &payload)? {
            return Ok(old);
        }
        let parent = checked_lineage(&tx, parent_id)?;
        if parent.revision != expected_revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "Parent grant changed before delegation",
            ));
        }
        scope.is_narrower_than(&parent.scope)?;
        if parent.depth >= parent.scope.max_depth {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Root-approved delegation depth reached",
            ));
        }
        let root: ExecutionGrant = read_document(&tx, "execution_grants", &parent.root_grant_id)?;
        let siblings: i64 = tx
            .query_row(
                "SELECT count(*) FROM execution_grants WHERE parent_id=?1",
                [parent_id],
                |r| r.get(0),
            )
            .map_err(db_error)?;
        let descendants:i64=tx.query_row("SELECT count(*) FROM execution_grants WHERE project_id=?1 AND id<>?2 AND json_extract(document,'$.root_grant_id')=?2",params![parent.project_id,parent.root_grant_id],|r|r.get(0)).map_err(db_error)?;
        if siblings >= parent.scope.max_children as i64
            || descendants >= root.scope.max_children as i64
        {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Shared root or parent delegation fanout exhausted",
            ));
        }
        let (connection_bindings, workspace_bindings) = validate_registered(
            &tx,
            &parent.project_id,
            &scope,
            true,
            self.data_input.as_ref(),
        )?;
        for (id, binding) in &connection_bindings {
            if parent.connection_bindings.get(id) != Some(binding) {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Delegation cannot silently change account or connection snapshots",
                ));
            }
        }
        for (id, binding) in &workspace_bindings {
            if parent.workspace_bindings.get(id) != Some(binding) {
                return Err(CoreError::new(
                    "INPUT_CHANGED",
                    "Delegation cannot silently replace workspace identity",
                ));
            }
        }
        let expires_at_ms = milliseconds()?
            .checked_add(scope.max_duration_ms as i64)
            .ok_or_else(|| CoreError::new("INVALID_REQUEST", "Child deadline overflow"))?
            .min(parent.expires_at_ms);
        let grant = ExecutionGrant {
            id: Uuid::new_v4().to_string(),
            project_id: parent.project_id,
            root_grant_id: parent.root_grant_id,
            parent_id: Some(parent_id.into()),
            depth: parent.depth + 1,
            revision: 1,
            approval_id: parent.approval_id,
            approved_at: now()?,
            expires_at_ms,
            revoked: false,
            scope,
            connection_bindings,
            workspace_bindings,
            enforcement: parent.enforcement,
        };
        tx.execute(
            "INSERT INTO execution_grants(id,project_id,parent_id,document) VALUES(?1,?2,?3,?4)",
            params![grant.id, grant.project_id, parent_id, encode(&grant)?],
        )
        .map_err(db_error)?;
        event(&tx, &grant, "execution_grant.delegated", "baton_policy")?;
        remember(&tx, &idem_scope, key, &payload, &grant)?;
        tx.commit().map_err(db_error)?;
        Ok(grant)
    }
}

#[cfg(test)]
mod live_workspace_tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn fixture() -> (tempfile::TempDir, WorkspaceIdentity) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("worktree");
        let common = dir.path().join("repository.git");
        let gitdir = common.join("worktrees/managed");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&gitdir).unwrap();
        fs::write(root.join(".git"), format!("gitdir: {}\n", gitdir.display())).unwrap();
        fs::write(gitdir.join("commondir"), "../..\n").unwrap();
        fs::write(
            gitdir.join("gitdir"),
            format!("{}\n", root.join(".git").display()),
        )
        .unwrap();
        let canonical_root = root.canonicalize().unwrap();
        let metadata = fs::metadata(&canonical_root).unwrap();
        let identity = WorkspaceIdentity {
            canonical_root,
            device: metadata.dev(),
            inode: metadata.ino(),
            kind: WorkspaceKind::Git {
                common_dir: common.canonicalize().unwrap(),
                head: "not_revalidated_by_live_check".into(),
                dirty: false,
            },
        };
        (dir, identity)
    }

    #[test]
    fn live_writer_can_edit_without_git_or_clean_input_validation() {
        let (_dir, pinned) = fixture();
        fs::write(
            pinned.canonical_root.join("owned-output"),
            "changed by writer",
        )
        .unwrap();
        assert!(live_workspace_matches(&pinned.canonical_root, &pinned).unwrap());
    }

    #[test]
    fn live_writer_rejects_replaced_directory_or_repository_backpointer() {
        let (dir, pinned) = fixture();
        let gitdir = dir.path().join("repository.git/worktrees/managed");
        fs::write(gitdir.join("gitdir"), "/different-worktree/.git\n").unwrap();
        assert!(!live_workspace_matches(&pinned.canonical_root, &pinned).unwrap_or(false));
        fs::rename(&pinned.canonical_root, dir.path().join("old-worktree")).unwrap();
        fs::create_dir(&pinned.canonical_root).unwrap();
        assert!(!live_workspace_matches(&pinned.canonical_root, &pinned).unwrap());
    }

    #[test]
    fn live_writer_rejects_symlink_and_oversized_git_pointers() {
        let (dir, pinned) = fixture();
        let dotgit = pinned.canonical_root.join(".git");
        fs::rename(&dotgit, dir.path().join("saved-pointer")).unwrap();
        symlink(dir.path().join("saved-pointer"), &dotgit).unwrap();
        assert!(!live_workspace_matches(&pinned.canonical_root, &pinned).unwrap());
        fs::remove_file(&dotgit).unwrap();
        fs::write(&dotgit, "x".repeat(4097)).unwrap();
        assert!(live_workspace_matches(&pinned.canonical_root, &pinned).is_err());
    }

    #[test]
    fn live_writer_fifo_pointer_cannot_block_actor() {
        const CHILD: &str = "AGENTBATON_POLICY_FIFO_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            for name in ["gitdir", "commondir"] {
                let (dir, pinned) = fixture();
                let path = dir
                    .path()
                    .join("repository.git/worktrees/managed")
                    .join(name);
                fs::remove_file(&path).unwrap();
                // The portable POSIX fixture utility also exists on macOS,
                // where rustix deliberately does not expose mkfifoat.
                assert!(std::process::Command::new("/usr/bin/mkfifo")
                    .args(["-m", "600"])
                    .arg(&path)
                    .status()
                    .unwrap()
                    .success());
                use std::os::unix::fs::FileTypeExt;
                assert!(fs::symlink_metadata(&path).unwrap().file_type().is_fifo());
                assert!(live_workspace_matches(&pinned.canonical_root, &pinned).is_err());
            }
            return;
        }
        // Run the malicious filesystem case in a killable child: a regression
        // must fail with evidence instead of hanging the whole test harness.
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "policy::live_workspace_tests::live_writer_fifo_pointer_cannot_block_actor",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "FIFO pointer validation child failed: {status}"
                );
                break;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("FIFO pointer validation blocked longer than three seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}
