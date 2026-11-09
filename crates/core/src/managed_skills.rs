//! Explicit one-file managed Skill versions. Native files remain read-only;
//! deriving a named connection never mutates its base or active runtime.
use super::*;
use agentbaton_config::capabilities::native::{
    instruction_only_skill_metadata, InstructionOnlySkill,
};
use agentbaton_config::runtime::{NonSecretText, SelectedSkill, SourceProvenance};
const MARKER: &str = "baton-managed-codex-skill";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ManagedSkillOperation {
    Replace,
    Remove,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillRequest {
    pub base_connection_id: String,
    pub expected_base_revision: u64,
    pub new_name: String,
    pub operation: ManagedSkillOperation,
    pub skill_folder: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillSelection {
    pub request: ManagedSkillRequest,
    pub expected_preview_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillSummary {
    pub directory_name: String,
    pub name: String,
    pub description: String,
    pub sha256: String,
    pub instruction_bytes: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillProfile {
    pub connection_id: String,
    pub connection_revision: u64,
    pub config_revision: String,
    pub runtime_plan_digest: String,
    pub sandbox_mode: String,
    pub approval_policy: String,
    pub managed_skill: Option<ManagedSkillSummary>,
    pub runtime_loaded: Option<bool>,
    pub runtime_used: Option<bool>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillPreview {
    pub base: ManagedSkillProfile,
    pub operation: ManagedSkillOperation,
    pub new_name: String,
    pub skill: Option<ManagedSkillSummary>,
    pub instruction: Option<String>,
    pub policy_unchanged: bool,
    pub source_folder: Option<String>,
    pub preview_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillConfirmation {
    pub connection: ProviderConnection,
    pub profile: ManagedSkillProfile,
    pub request_key: String,
    pub preview_digest: String,
    pub base_connection_id: String,
    pub base_connection_revision: u64,
    pub operation: ManagedSkillOperation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillFind {
    pub request_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillConnectionId {
    pub connection_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillUnavailable {
    pub connection_id: String,
    pub name: String,
    pub code: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedSkillCatalog {
    pub profiles: Vec<ManagedSkillProfile>,
    pub unavailable: Vec<ManagedSkillUnavailable>,
}
fn changed() -> CoreError {
    CoreError::new(
        "INPUT_CHANGED",
        "Skill source or base connection changed; review a new managed copy",
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
fn text(skill: &SelectedSkill) -> Result<String> {
    Ok(skill.instruction_text().to_owned())
}
fn summary(plan: &RuntimePlan) -> Result<Option<ManagedSkillSummary>> {
    if plan.skills.is_empty() {
        return Ok(None);
    }
    if !crate::runtime_config::is_managed_codex_skill_plan(plan)? {
        return Err(changed());
    }
    let s = &plan.skills[0];
    let instruction = text(s)?;
    let (name, description) = instruction_only_skill_metadata(&s.directory_name, &instruction)
        .map_err(crate::runtime_config::config_error)?;
    Ok(Some(ManagedSkillSummary {
        directory_name: s.directory_name.clone(),
        name,
        description,
        sha256: s.provenance.sha256.clone(),
        instruction_bytes: instruction.len() as u64,
    }))
}
fn profile(c: &ProviderConnection) -> Result<ManagedSkillProfile> {
    if c.provider != "codex" {
        return Err(CoreError::new(
            "CAPABILITY_UNSUPPORTED",
            "Managed Skills currently require a Codex connection",
        ));
    }
    let plan = crate::runtime_config::approved_plan(c)?;
    let enum_text = |v: Value| v.as_str().map(str::to_owned).ok_or_else(changed);
    Ok(ManagedSkillProfile {
        connection_id: c.id.clone(),
        connection_revision: c.revision,
        config_revision: c.config_revision.clone(),
        runtime_plan_digest: plan
            .plan_digest()
            .map_err(crate::runtime_config::config_error)?,
        sandbox_mode: enum_text(json!(plan.codex.sandbox))?,
        approval_policy: enum_text(json!(plan.codex.approval_policy))?,
        managed_skill: summary(&plan)?,
        runtime_loaded: None,
        runtime_used: None,
    })
}
struct Prepared {
    base: ProviderConnection,
    plan: RuntimePlan,
    preview: ManagedSkillPreview,
    source_binding: Value,
}
fn prepare(db: &Connection, r: &ManagedSkillRequest) -> Result<Prepared> {
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
    let had_skill = crate::runtime_config::is_managed_codex_skill_plan(&plan)?;
    plan.skills.clear();
    plan.source_refs.retain(|s| s.source_ref != MARKER);
    let (source_binding, instruction) = match r.operation {
        ManagedSkillOperation::Remove => {
            if r.skill_folder.is_some() || !had_skill {
                return Err(invalid(
                    "Remove requires exactly one existing managed Skill and no source folder",
                ));
            }
            (Value::Null, None)
        }
        ManagedSkillOperation::Replace => {
            let folder = r
                .skill_folder
                .as_ref()
                .filter(|p| !p.is_empty() && p.len() <= 4096 && !p.chars().any(char::is_control))
                .ok_or_else(|| invalid("Select the exact one-file Skill folder"))?;
            let source = InstructionOnlySkill::read_selected(Path::new(folder))
                .map_err(crate::runtime_config::config_error)?;
            let m = source.metadata();
            let provenance = SourceProvenance {
                source_ref: MARKER.into(),
                revision: "1".into(),
                sha256: m.sha256.clone(),
            };
            plan.skills.push(SelectedSkill {
                directory_name: m.directory_name.clone(),
                instruction: NonSecretText::approved(source.instruction().into())
                    .map_err(crate::runtime_config::config_error)?,
                assets: vec![],
                provenance: provenance.clone(),
            });
            plan.source_refs.push(provenance);
            source
                .verify_unchanged()
                .map_err(crate::runtime_config::config_error)?;
            let binding = json!(m);
            (binding, Some(source.instruction().to_owned()))
        }
    };
    plan.validate()
        .map_err(crate::runtime_config::config_error)?;
    let mut preview = ManagedSkillPreview {
        base: base_profile,
        operation: r.operation.clone(),
        new_name: r.new_name.clone(),
        skill: summary(&plan)?,
        instruction,
        policy_unchanged: true,
        source_folder: r.skill_folder.clone(),
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
    if p.kind != "managed_skill" || p.id != id {
        return Err(changed());
    }
    Ok((p, result))
}
fn committed(db: &Connection, p: &ApprovalProposal, raw: &str) -> Result<ManagedSkillConfirmation> {
    let mut c: ManagedSkillConfirmation = decode(raw)?;
    let selection: ManagedSkillSelection =
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
    let event:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM events WHERE octet_length(document)<=1048576 AND json_extract(document,'$.aggregate_id')=?1 AND json_extract(document,'$.kind')='connection.managed_skill_created' AND json_extract(document,'$.source')='local_owner_confirmation' AND json_extract(document,'$.payload.proposal_id')=?2 AND json_extract(document,'$.payload.preview_digest')=?3)",params![c.connection.id,p.id,c.preview_digest],|r|r.get(0)).map_err(db_error)?;
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
    pub fn preview_managed_skill(&self, r: &ManagedSkillRequest) -> Result<ManagedSkillPreview> {
        Ok(prepare(&self.conn, r)?.preview)
    }
    pub fn get_managed_skill_profile(&self, id: &str) -> Result<ManagedSkillProfile> {
        profile(&read_connection(&self.conn, id)?)
    }
    pub fn list_managed_skill_profiles(&self) -> Result<ManagedSkillCatalog> {
        let mut q=self.conn.prepare("SELECT id FROM connections WHERE json_extract(document,'$.provider')='codex' ORDER BY rowid LIMIT 257").map_err(db_error)?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db_error)?;
        if ids.len() > 256 {
            return Err(CoreError::new(
                "RESOURCE_LIMIT",
                "Managed Skill connection list exceeds bound",
            ));
        }
        let mut catalog = ManagedSkillCatalog {
            profiles: vec![],
            unavailable: vec![],
        };
        for id in ids {
            let c = read_connection(&self.conn, &id)?;
            match profile(&c) {
                Ok(p) => catalog.profiles.push(p),
                Err(e) if matches!(e.code.as_str(), "INPUT_CHANGED" | "CAPABILITY_UNSUPPORTED") => {
                    catalog.unavailable.push(ManagedSkillUnavailable {
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
    pub fn find_managed_skill(
        &self,
        r: &ManagedSkillFind,
    ) -> Result<Option<ManagedSkillConfirmation>> {
        key_valid(&r.request_key)?;
        let id:Option<String>=self.conn.query_row("SELECT id FROM approval_proposals WHERE octet_length(document)<=1048576 AND json_extract(document,'$.kind')='managed_skill' AND json_extract(document,'$.snapshot.request_key')=?1",[&r.request_key],|row|row.get(0)).optional().map_err(db_error)?;
        let Some(id) = id else { return Ok(None) };
        let (p, result) = proposal_row(&self.conn, &id)?;
        result
            .map(|raw| committed(&self.conn, &p, &raw))
            .transpose()
    }
    pub fn propose_managed_skill(
        &mut self,
        s: &ManagedSkillSelection,
        key: &str,
    ) -> Result<ApprovalProposal> {
        key_valid(key)?;
        let old:Option<String>=self.conn.query_row("SELECT CASE WHEN octet_length(result)<=1048576 THEN result END FROM idempotency WHERE scope='local:approvals.propose:managed_skill' AND key=?1",[key],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(raw) = old {
            let p: ApprovalProposal = decode(&raw)?;
            if p.snapshot["intent"] != json!(s) {
                return Err(CoreError::new(
                    "CONFIG_CONFLICT",
                    "Managed Skill key belongs to another selection",
                ));
            }
            return Ok(p);
        }
        let p = prepare(&self.conn, &s.request)?;
        if p.preview.preview_digest != s.expected_preview_digest {
            return Err(changed());
        }
        let snapshot = json!({"intent":s,"preview":p.preview,"base_connection":p.base,"base_connection_digest":digest(&p.base)?,"after_plan_digest":p.plan.plan_digest().map_err(crate::runtime_config::config_error)?,"source_binding":p.source_binding,"request_key":key,"scope":"new_managed_connection_version_probe_only","warning":"Skill instructions may affect future Agent behavior; not an OS sandbox or new execution grant. No scripts, assets, MCP, credentials, model or billing changes are imported. Runtime loaded and used remain unknown."});
        self.persist_proposal("managed_skill", snapshot, key)
    }
    pub fn finalize_managed_skill(&mut self, id: &str, revision: u64) -> Result<Value> {
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
                "Skill owner proposal changed",
            ));
        }
        if proposal.expires_at_ms <= ms()? {
            return Err(CoreError::new(
                "PERMISSION_DENIED",
                "Skill owner proposal expired",
            ));
        }
        let selection: ManagedSkillSelection =
            serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_| changed())?;
        let p = prepare(&tx, &selection.request)?;
        if p.preview.preview_digest != selection.expected_preview_digest
            || json!(p.preview) != proposal.snapshot["preview"]
            || digest(&p.base)? != proposal.snapshot["base_connection_digest"]
            || json!(p.base) != proposal.snapshot["base_connection"]
            || p.source_binding != proposal.snapshot["source_binding"]
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
        let result = ManagedSkillConfirmation {
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
            "connection.managed_skill_created",
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
            json!({"kind":"managed_skill","proposal_id":id,"consumed":true}),
            "local_owner_confirmation",
        )?;
        tx.commit().map_err(db_error)?;
        Ok(json!(result))
    }
}

#[cfg(test)]
#[path = "managed_skills_tests.rs"]
mod tests;

impl std::fmt::Debug for ManagedSkillPreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedSkillPreview")
            .field("preview_digest", &self.preview_digest)
            .field("instruction", &"[redacted]")
            .finish()
    }
}
