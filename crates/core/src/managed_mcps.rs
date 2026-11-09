//! Explicit executable-pinned managed MCP versions. Native files remain read-only;
//! deriving a named connection never mutates its base or active runtime.
use super::*;
use agentbaton_config::runtime::{
    managed_codex_mcp_digest, managed_codex_mcp_metadata, CodexMcpPolicy, ManagedMcpServer,
    ManagedMcpTransport, NonSecretText, SourceProvenance,
};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
const MARKER: &str = "baton-managed-codex-mcp";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedMcpOperation {
    Replace,
    Remove,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpRequest {
    pub base_connection_id: String,
    pub expected_base_revision: u64,
    pub new_name: String,
    pub operation: ManagedMcpOperation,
    pub server: Option<ManagedMcpServerSelection>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpSelection {
    pub request: ManagedMcpRequest,
    pub expected_preview_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpServerSelection {
    pub name: String,
    pub executable: String,
    pub enabled_tools: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpSummary {
    pub name: String,
    pub executable: String,
    pub executable_sha256: String,
    pub enabled_tools: Vec<String>,
    pub approval_mode: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpProfile {
    pub connection_id: String,
    pub connection_revision: u64,
    pub config_revision: String,
    pub runtime_plan_digest: String,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub has_managed_skill: bool,
    pub managed_mcp: Option<ManagedMcpSummary>,
    pub runtime_connected: Option<bool>,
    pub runtime_used: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpPreview {
    pub base: ManagedMcpProfile,
    pub operation: ManagedMcpOperation,
    pub new_name: String,
    pub server: Option<ManagedMcpSummary>,
    pub policy_unchanged: bool,
    pub preview_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpConfirmation {
    pub connection: ProviderConnection,
    pub profile: ManagedMcpProfile,
    pub request_key: String,
    pub preview_digest: String,
    pub base_connection_id: String,
    pub base_connection_revision: u64,
    pub operation: ManagedMcpOperation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpFind {
    pub request_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpConnectionId {
    pub connection_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpUnavailable {
    pub connection_id: String,
    pub name: String,
    pub code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedMcpCatalog {
    pub profiles: Vec<ManagedMcpProfile>,
    pub unavailable: Vec<ManagedMcpUnavailable>,
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "MCP source or base connection changed; review a new managed copy",
    )
}
fn digest<T: Serialize>(v: &T) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(encode(v)?.as_bytes())))
}
fn read_connection(db: &Connection, id: &str) -> Result<ProviderConnection> {
    key_valid(id)?;
    let raw:String=db.query_row("SELECT CASE WHEN octet_length(document)<=1048576 THEN document END FROM connections WHERE id=?1",[id],|r|r.get(0)).map_err(db_error)?;
    let c: ProviderConnection = decode(&raw)?;
    if c.id != id {
        return Err(changed());
    }
    Ok(c)
}
pub(crate) fn summary(plan: &RuntimePlan) -> Result<Option<ManagedMcpSummary>> {
    Ok(managed_codex_mcp_metadata(plan)
        .map_err(crate::runtime_config::config_error)?
        .map(|m| ManagedMcpSummary {
            name: m.name,
            executable: m.command,
            executable_sha256: m.executable_sha256,
            enabled_tools: m.enabled_tools,
            approval_mode: "prompt".into(),
        }))
}
/// Hash only an explicitly selected executable. No execution, environment or credentials reads.
fn source_binding(path: &str) -> Result<Value> {
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err(changed());
    }
    let p = Path::new(path);
    let before = std::fs::symlink_metadata(p).map_err(|_| changed())?;
    if !before.is_file()
        || before.permissions().mode() & 0o111 == 0
        || before.permissions().mode() & 0o022 != 0
    {
        return Err(changed());
    }
    let sha = executable_digest(p).map_err(|_| changed())?;
    let after = std::fs::symlink_metadata(p).map_err(|_| changed())?;
    let identity = |m: &std::fs::Metadata| json!({"device":m.dev(),"inode":m.ino(),"mode":m.mode(),"uid":m.uid(),"size":m.size(),"mtime_seconds":m.mtime(),"mtime_nanoseconds":m.mtime_nsec(),"ctime_seconds":m.ctime(),"ctime_nanoseconds":m.ctime_nsec()});
    if identity(&before) != identity(&after) {
        return Err(changed());
    }
    Ok(json!({"executable_sha256":sha.as_str(),"identity":identity(&after)}))
}
fn profile(c: &ProviderConnection) -> Result<ManagedMcpProfile> {
    if c.provider != "codex" || c.protocol_route != ProtocolRoute::CodexAppServer {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Managed MCP servers currently require a Codex connection",
        ));
    }
    let plan = crate::runtime_config::approved_plan(c)?;
    let enum_text = |v: Value| v.as_str().map(str::to_owned).ok_or_else(changed);
    Ok(ManagedMcpProfile {
        connection_id: c.id.clone(),
        connection_revision: c.revision,
        config_revision: c.config_revision.clone(),
        runtime_plan_digest: plan
            .plan_digest()
            .map_err(crate::runtime_config::config_error)?,
        sandbox_mode: enum_text(json!(plan.codex.sandbox))?,
        approval_policy: enum_text(json!(plan.codex.approval_policy))?,
        has_managed_skill: !plan.skills.is_empty(),
        managed_mcp: summary(&plan)?,
        runtime_connected: None,
        runtime_used: None,
    })
}
struct Prepared {
    base: ProviderConnection,
    plan: RuntimePlan,
    preview: ManagedMcpPreview,
    source_binding: Value,
}
fn prepare(db: &Connection, r: &ManagedMcpRequest) -> Result<Prepared> {
    key_valid(&r.base_connection_id)?;
    if r.new_name.trim().is_empty()
        || r.new_name.len() > 128
        || r.new_name.chars().any(char::is_control)
    {
        return Err(invalid("Select a bounded new connection name"));
    }
    let base = read_connection(db, &r.base_connection_id)?;
    if base.revision != r.expected_base_revision {
        return Err(CoreError::new("STALE_REVISION", "Base connection changed"));
    }
    if base.probe_state != "ready" || base.host_id != "local" {
        return Err(CoreError::new(
            "PERMISSION_DENIED",
            "Use a ready base connection",
        ));
    }
    let base_profile = profile(&base)?;
    let mut plan = crate::runtime_config::approved_plan(&base)?;
    let had_server = summary(&plan)?.is_some();
    plan.mcp_servers.clear();
    plan.source_refs.retain(|s| s.source_ref != MARKER);
    let source_binding = match r.operation {
        ManagedMcpOperation::Remove => {
            if r.server.is_some() || !had_server {
                return Err(invalid(
                    "Remove requires an existing managed MCP server and no replacement",
                ));
            }
            Value::Null
        }
        ManagedMcpOperation::Replace => {
            let server = r
                .server
                .as_ref()
                .ok_or_else(|| invalid("Select an explicit executable and finite tools"))?;
            let binding = source_binding(&server.executable)?;
            let sha = binding["executable_sha256"]
                .as_str()
                .ok_or_else(changed)?
                .to_owned();
            let hash = managed_codex_mcp_digest(
                &server.name,
                &server.executable,
                &sha,
                &server.enabled_tools,
            )
            .map_err(crate::runtime_config::config_error)?;
            let provenance = SourceProvenance {
                source_ref: MARKER.into(),
                revision: "1".into(),
                sha256: hash,
            };
            plan.mcp_servers.push(ManagedMcpServer {
                name: server.name.clone(),
                transport: ManagedMcpTransport::Stdio {
                    command: NonSecretText::approved(server.executable.clone())
                        .map_err(crate::runtime_config::config_error)?,
                    arguments: vec![],
                },
                codex_policy: Some(CodexMcpPolicy {
                    executable_sha256: sha,
                    enabled_tools: server.enabled_tools.clone(),
                }),
                provenance: provenance.clone(),
            });
            plan.source_refs.push(provenance);
            binding
        }
    };
    plan.validate()
        .map_err(crate::runtime_config::config_error)?;
    let mut preview = ManagedMcpPreview {
        base: base_profile,
        operation: r.operation.clone(),
        new_name: r.new_name.clone(),
        server: summary(&plan)?,
        policy_unchanged: true,
        preview_digest: String::new(),
    };
    preview.preview_digest = digest(
        &json!({"request":r,"preview":preview,"base_digest":digest(&base)?,"after_plan_digest":plan.plan_digest().map_err(crate::runtime_config::config_error)?,"source_binding":source_binding}),
    )?;
    Ok(Prepared {
        base,
        plan,
        preview,
        source_binding,
    })
}
fn proposal_row(db: &Connection, id: &str) -> Result<(ApprovalProposal, Option<String>)> {
    let(raw,result):(String,Option<String>)=db.query_row("SELECT document,result FROM approval_proposals WHERE id=?1 AND octet_length(document)<=1048576 AND COALESCE(octet_length(result),0)<=1048576",[id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(db_error)?;
    let p: ApprovalProposal = decode(&raw)?;
    if p.kind != "managed_mcp" || p.id != id {
        return Err(changed());
    }
    Ok((p, result))
}
fn committed(db: &Connection, p: &ApprovalProposal, raw: &str) -> Result<ManagedMcpConfirmation> {
    let mut c: ManagedMcpConfirmation = decode(raw)?;
    let selection: ManagedMcpSelection =
        serde_json::from_value(p.snapshot["intent"].clone()).map_err(|_| changed())?;
    if !p.consumed
        || c.request_key != p.snapshot["request_key"]
        || c.preview_digest != selection.expected_preview_digest
        || c.base_connection_id != selection.request.base_connection_id
        || c.base_connection_revision != selection.request.expected_base_revision
        || c.operation != selection.request.operation
    {
        return Err(changed());
    }
    let event:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='connection.managed_mcp_created' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.payload.proposal_id')=?2 AND json_extract(document,'$.payload.preview_digest')=?3)",params![c.connection.id,p.id,c.preview_digest],|r|r.get(0)).map_err(db_error)?;
    if !event {
        return Err(changed());
    }
    let current = read_connection(db, &c.connection.id)?;
    let current_profile = profile(&current)?;
    if current.config_revision != c.connection.config_revision
        || current_profile.runtime_plan_digest != c.profile.runtime_plan_digest
        || current.executable_sha256 != c.connection.executable_sha256
    {
        return Err(changed());
    }
    let mut immutable = current.clone();
    immutable.revision = c.connection.revision;
    immutable.probe_state = c.connection.probe_state.clone();
    immutable.version_evidence = c.connection.version_evidence.clone();
    if encode(&immutable)? != encode(&c.connection)? {
        return Err(changed());
    }
    c.connection = current;
    c.profile = current_profile;
    Ok(c)
}
impl Store {
    pub fn preview_managed_mcp(&self, r: &ManagedMcpRequest) -> Result<ManagedMcpPreview> {
        Ok(prepare(&self.conn, r)?.preview)
    }
    pub fn get_managed_mcp_profile(&self, id: &str) -> Result<ManagedMcpProfile> {
        profile(&read_connection(&self.conn, id)?)
    }
    pub fn list_managed_mcp_profiles(&self) -> Result<ManagedMcpCatalog> {
        let mut q=self.conn.prepare("SELECT id FROM connections WHERE json_extract(document,'$.provider')='codex' ORDER BY rowid LIMIT 257").map_err(db_error)?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if ids.len() > 256 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Managed MCP connection list exceeds bound",
            ));
        }
        let mut catalog = ManagedMcpCatalog {
            profiles: vec![],
            unavailable: vec![],
        };
        for id in ids {
            let c = read_connection(&self.conn, &id)?;
            match profile(&c) {
                Ok(p) => catalog.profiles.push(p),
                Err(e) if matches!(e.code.as_str(), "INPUT_CHANGED" | "CAPABILITY_UNSUPPORTED") => {
                    catalog.unavailable.push(ManagedMcpUnavailable {
                        connection_id: c.id,
                        name: c.name,
                        code: e.code,
                    })
                }
                Err(e) => return Err(e),
            }
        }
        Ok(catalog)
    }
    pub fn find_managed_mcp(&self, r: &ManagedMcpFind) -> Result<Option<ManagedMcpConfirmation>> {
        key_valid(&r.request_key)?;
        let id:Option<String>=self.conn.query_row("SELECT id FROM approval_proposals WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='managed_mcp' AND json_extract(document,'$.snapshot.request_key')=?1",[&r.request_key],|row|row.get(0)).optional().map_err(db_error)?;
        let Some(id) = id else { return Ok(None) };
        let (p, result) = proposal_row(&self.conn, &id)?;
        result
            .map(|raw| committed(&self.conn, &p, &raw))
            .transpose()
    }
    pub fn propose_managed_mcp(
        &mut self,
        s: &ManagedMcpSelection,
        key: &str,
    ) -> Result<ApprovalProposal> {
        key_valid(key)?;
        let old:Option<String>=self.conn.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope='local:approvals.propose:managed_mcp' AND key=?1",[key],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(raw) = old {
            let p: ApprovalProposal = decode(&raw)?;
            if p.snapshot["intent"] != json!(s) {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Managed MCP key belongs to another selection",
                ));
            }
            return Ok(p);
        }
        let p = prepare(&self.conn, &s.request)?;
        if p.preview.preview_digest != s.expected_preview_digest {
            return Err(changed());
        }
        let snapshot = json!({"intent":s,"preview":p.preview,"base_connection":p.base,"base_connection_digest":digest(&p.base)?,"after_plan":p.plan,"after_plan_digest":p.plan.plan_digest().map_err(crate::runtime_config::config_error)?,"source_binding":p.source_binding,"request_key":key,"scope":"new_managed_connection_version_probe_only","warning":"This native stdio executable may perform local or network I/O outside Baton isolation. Only selected tools use prompt approval; unsupported interactive approvals are denied. No credentials or model settings are imported. A new execution grant is required; connected and used remain unknown."});
        self.persist_proposal("managed_mcp", snapshot, key)
    }
    pub fn finalize_managed_mcp(&mut self, id: &str, revision: u64) -> Result<Value> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let (mut proposal, result) = proposal_row(&tx, id)?;
        if let Some(raw) = result {
            return committed(&tx, &proposal, &raw)
                .map(|c| json!(c))
                .map_err(|mut e| {
                    e.side_effect_state = "confirmed".into();
                    e
                });
        }
        if proposal.revision != revision {
            return Err(CoreError::new(
                "STALE_REVISION",
                "MCP owner proposal changed",
            ));
        }
        if proposal.expires_at_ms <= ms()? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "MCP owner proposal expired",
            ));
        }
        let selection: ManagedMcpSelection =
            serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_| changed())?;
        let p = prepare(&tx, &selection.request)?;
        if p.preview.preview_digest != selection.expected_preview_digest
            || json!(p.preview) != proposal.snapshot["preview"]
            || digest(&p.base)? != proposal.snapshot["base_connection_digest"]
            || json!(p.base) != proposal.snapshot["base_connection"]
            || p.source_binding != proposal.snapshot["source_binding"]
            || json!(p.plan) != proposal.snapshot["after_plan"]
            || p.plan
                .plan_digest()
                .map_err(crate::runtime_config::config_error)?
                != proposal.snapshot["after_plan_digest"]
        {
            return Err(changed());
        }
        verify_executable(
            &p.base.executable,
            &BinaryDigest::parse(&p.base.executable_sha256).map_err(|_| changed())?,
        )
        .map_err(|_| changed())?;
        let mut c = p.base.clone();
        c.id = Uuid::new_v4().to_string();
        c.name = selection.request.new_name.clone();
        c.environment_id = Uuid::new_v4().to_string();
        c.identity_ref = format!("unknown:{}", c.id);
        c.approval_id = Uuid::new_v4().to_string();
        c.revision = 1;
        c.probe_state = "pending".into();
        c.version_evidence = None;
        c.capability_snapshot_id = format!("client-no-tools:{}", c.id);
        c.created_at = now()?;
        c.config_revision = crate::runtime_config::configuration_revision(
            &c.arguments,
            c.protocol_route,
            &p.plan
                .plan_digest()
                .map_err(crate::runtime_config::config_error)?,
        )?;
        c.runtime_plan = Some(p.plan);
        let result = ManagedMcpConfirmation {
            profile: profile(&c)?,
            connection: c.clone(),
            request_key: proposal.snapshot["request_key"]
                .as_str()
                .ok_or_else(changed)?
                .into(),
            preview_digest: selection.expected_preview_digest,
            base_connection_id: selection.request.base_connection_id,
            base_connection_revision: selection.request.expected_base_revision,
            operation: selection.request.operation,
        };
        tx.execute(
            "INSERT INTO connections VALUES(?1,?2)",
            params![c.id, encode(&c)?],
        )
        .map_err(db_error)?;
        event(
            &tx,
            &c.id,
            c.revision,
            "connection.probe_authorized",
            json!({"connection_id":c.id,"executable_sha256":c.executable_sha256,"approval_id":c.approval_id,"scope":"version_probe_only","identity_status":"unknown"}),
            "user_action",
        )?;
        event(
            &tx,
            &c.id,
            c.revision,
            "connection.managed_mcp_created",
            json!({"proposal_id":id,"base_connection_id":result.base_connection_id,"preview_digest":result.preview_digest,"runtime_plan_digest":result.profile.runtime_plan_digest}),
            "local_owner_confirmation",
        )?;
        proposal.consumed = true;
        proposal.revision += 1;
        tx.execute(
            "UPDATE approval_proposals SET document=?1,result=?2 WHERE id=?3 AND result IS NULL",
            params![encode(&proposal)?, encode(&result)?, id],
        )
        .map_err(db_error)?;
        event(
            &tx,
            id,
            proposal.revision,
            "approval.confirmed",
            json!({"kind":"managed_mcp","proposal_id":id,"consumed":true}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(result))
    }
}

/// Only Core can construct this authority from its approved plan and verified
/// runtime snapshot. Every worker receives one, including no-MCP runtimes.
pub(crate) struct ManagedMcpLaunch {
    environment: BTreeMap<OsString, OsString>,
    configuration: Option<(PathBuf, String)>,
    server: Option<(String, Value)>,
    #[cfg(test)]
    cancel_after_verify: Option<Arc<AtomicBool>>,
}
impl ManagedMcpLaunch {
    pub(crate) fn new(
        plan: &RuntimePlan,
        snapshot: &agentbaton_config::runtime::RuntimeSnapshot,
    ) -> Result<Self> {
        if plan
            .plan_digest()
            .map_err(crate::runtime_config::config_error)?
            != snapshot.plan_digest()
        {
            return Err(changed());
        }
        let server = summary(plan)?
            .map(|s| {
                let binding = source_binding(&s.executable)?;
                if binding["executable_sha256"] != s.executable_sha256 {
                    return Err(changed());
                }
                Ok((s.executable, binding))
            })
            .transpose()?;
        let configuration = if plan.provider() == agentbaton_config::runtime::RuntimeProvider::Codex
        {
            let entry = snapshot
                .manifest()
                .files
                .iter()
                .find(|f| f.relative_path == Path::new("codex/config.toml"))
                .ok_or_else(changed)?;
            Some((
                snapshot.provider_configuration_root().join("config.toml"),
                entry.sha256.clone(),
            ))
        } else {
            None
        };
        Ok(Self {
            environment: snapshot.environment(),
            configuration,
            server,
            #[cfg(test)]
            cancel_after_verify: None,
        })
    }
    pub(crate) fn verify(&self, installation: &ApprovedInstallation) -> Result<()> {
        use sha2::{Digest, Sha256};
        use std::io::Read;
        if self.environment != installation.environment {
            return Err(changed());
        }
        if let Some((path, sha)) = &self.configuration {
            let m = std::fs::symlink_metadata(path).map_err(|_| changed())?;
            if !m.is_file()
                || m.len() > 1048576
                || std::fs::canonicalize(path).map_err(|_| changed())? != *path
            {
                return Err(changed());
            }
            let file = std::fs::File::open(path).map_err(|_| changed())?;
            let mut bytes = Vec::new();
            file.take(1048577)
                .read_to_end(&mut bytes)
                .map_err(|_| changed())?;
            if bytes.len() > 1048576 || format!("{:x}", Sha256::digest(&bytes)) != *sha {
                return Err(changed());
            }
        }
        if let Some((path, binding)) = &self.server {
            if source_binding(path)? != *binding {
                return Err(changed());
            }
        }
        #[cfg(test)]
        if let Some(cancel) = &self.cancel_after_verify {
            cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn test_empty() -> Self {
        Self {
            environment: BTreeMap::new(),
            configuration: None,
            server: None,
            cancel_after_verify: None,
        }
    }
}

#[cfg(test)]
#[path = "managed_mcps_tests.rs"]
mod tests;
