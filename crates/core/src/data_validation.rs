//! Owner-approved deterministic JSON contracts. Separate from integrity policies,
//! task acceptance and adoption; neither scripts nor Provider calls run here.
use crate::{
    sessions::ApprovalProposal,
    storage::{db_error, decode, encode, now, Event, Store},
    validation::bounded_documents,
    CoreError, Result,
};
use agentbaton_workspaces::{
    deterministic::{validate_json_table, ApprovedJsonTableRule, JsonTableContract, JsonTablePolicyView, VALIDATOR_ID},
    validation::{ArtifactContractView, ArtifactRequirement, BoundArtifact, ValidationError, ValidationPolicyContractView, ValidationSubject, VersionBinding},
};
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

const MAX_RECORD_BYTES: i64 = 4 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationSelection {
    pub session_id: String,
    pub inputs: Vec<ArtifactRequirement>,
    pub outputs: Vec<ArtifactRequirement>,
    pub table: JsonTableContract,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationPolicy {
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
    pub validator_id: String,
    pub file_contract: ValidationPolicyContractView,
    pub table: JsonTableContract,
    pub contract_digest: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataValidationRecord {
    pub id: String,
    pub policy_id: String,
    pub task_id: String,
    pub state: String,
    pub scope: String,
    pub correctness: String,
    pub task_acceptance: String,
    pub integration_authorized: bool,
    /// Serializable view only; never deserialize this into a trusted acceptance.
    pub evidence: Value,
    pub created_at: String,
}
fn invalid() -> CoreError {
    CoreError::new("INVALID_REQUEST", "Select a bounded JSON data contract from one registered, confirmed-stopped session")
}
fn changed(_: ValidationError) -> CoreError {
    CoreError::new("INPUT_CHANGED", "Data subject, root or contract changed; review a fresh owner policy")
}
fn selection_error(error: ValidationError) -> CoreError {
    match error { ValidationError::InvalidContract => invalid(), other => changed(other) }
}
fn key(key: &str) -> Result<()> {
    if key.is_empty() || key.len() > 256 { Err(invalid()) } else { Ok(()) }
}
fn limited() -> CoreError {
    CoreError::new("RESOURCE_LIMIT", "Stored data validation document exceeds its raw-byte bound; history remains local")
}
fn event(tx: &rusqlite::Transaction<'_>, id: &str, revision: u64, kind: &str, payload: Value, source: &str) -> Result<()> {
    let event = Event { sequence:0, event_id:Uuid::new_v4().to_string(), aggregate_id:id.into(), revision, kind:kind.into(), occurred_at:now()?, payload, source:source.into() };
    tx.execute("INSERT INTO events(document) VALUES(?1)",[encode(&event)?]).map_err(db_error)?;
    Ok(())
}
/// Only these fixed tables can be selected. Check raw size before decoding even
/// a single legacy/corrupt policy or owner proposal.
fn bounded_document(connection: &rusqlite::Connection, table: &str, id: &str) -> Result<String> {
    let (size_sql, document_sql) = match table {
        "data_validation_policies" => ("SELECT octet_length(document) FROM data_validation_policies WHERE id=?1", "SELECT document FROM data_validation_policies WHERE id=?1 AND octet_length(document)<=?2"),
        "approval_proposals" => ("SELECT octet_length(document) FROM approval_proposals WHERE id=?1", "SELECT document FROM approval_proposals WHERE id=?1 AND octet_length(document)<=?2"),
        _ => return Err(CoreError::new("INTERNAL_ERROR", "Unknown data validation document")),
    };
    let size: Option<i64> = connection.query_row(size_sql,[id],|row|row.get(0)).optional().map_err(db_error)?;
    let size=size.ok_or_else(||CoreError::new("NOT_FOUND","Data policy or owner proposal is unavailable"))?;
    if !(0..=MAX_RECORD_BYTES).contains(&size) { return Err(limited()); }
    connection.query_row(document_sql,params![id,MAX_RECORD_BYTES],|row|row.get(0)).optional().map_err(db_error)?.ok_or_else(limited)
}
impl Store {
    fn resolve_data_validation_selection(&self, selection: &DataValidationSelection) -> Result<(JsonTablePolicyView, Value)> {
        if selection.inputs.is_empty() || selection.inputs.len()>16 || selection.outputs.len()!=3
            || selection.inputs.iter().chain(&selection.outputs).any(|a| a.relative_path.as_os_str().len()>4096)
        { return Err(invalid()); }
        let (session, workspace, binding)=self.validation_workspace(&selection.session_id)?;
        let task=self.get(&session.task_id)?;
        let bind=|requirements:&[ArtifactRequirement]| -> Result<Vec<BoundArtifact>> {
            requirements.iter().cloned().map(|requirement| BoundArtifact::from_approved_root(&workspace.root,requirement).map_err(selection_error)).collect()
        };
        let subject=ValidationSubject {
            task_id:task.id.clone(), attempt_id:session.attempt_id.clone(), spec_revision:task.spec_revision,
            input_revision:binding.input_revision.clone(),
            // This immutable revision is server-derived. Population rows are an
            // owner-approved declaration, not proof from parsing input datasets.
            version:VersionBinding::Data { dataset_revision:binding.input_revision.clone() },
        };
        let rule=ApprovedJsonTableRule::from_verified_policy("proposal-preview".into(),"1".into(),subject,bind(&selection.inputs)?,bind(&selection.outputs)?,selection.table.clone()).map_err(selection_error)?;
        let project=self.get_project(&session.project_id)?;
        let refs=json!({"project_id":project.id,"project_name":project.name,"project_revision":project.revision,"task_goal":task.goal,"task_revision":task.revision,"session_revision":session.revision,"attempt_id":session.attempt_id,"workspace_id":workspace.id,"workspace_revision":workspace.revision,"workspace_identity":workspace.identity,"binding":binding});
        Ok((rule.contract_view(),refs))
    }
    pub fn propose_data_validation_policy(&mut self, selection: &DataValidationSelection, idempotency_key: &str) -> Result<ApprovalProposal> {
        let (contract,refs)=self.resolve_data_validation_selection(selection)?;
        let digest=contract.digest().map_err(selection_error)?;
        self.persist_proposal("data_validation",json!({"intent":selection,"refs":refs,"file_contract":contract.artifact_policy,"table":contract.table,"validator_id":VALIDATOR_ID,"contract_digest":digest,"scope":"data_contract_only","correctness":"approved_schema_ranges_metadata_only","warning":"Approves local JSON schema/ranges/exact metadata and hash-bound artifact checks only. Dataset population, units and timezone are approved declarations, not algorithmic or scientific proof. No scripts, Provider calls, TaskAccepted, Git adoption or production release are authorized."}),idempotency_key)
    }
    pub(crate) fn approve_data_validation_policy(&mut self, proposal: &ApprovalProposal) -> Result<Value> {
        let selection:DataValidationSelection=serde_json::from_value(proposal.snapshot["intent"].clone()).map_err(|_|invalid())?;
        let (contract,refs)=self.resolve_data_validation_selection(&selection)?;
        let digest=contract.digest().map_err(selection_error)?;
        if proposal.snapshot["refs"]!=refs || proposal.snapshot["file_contract"]!=json!(contract.artifact_policy)
            || proposal.snapshot["table"]!=json!(contract.table) || proposal.snapshot["contract_digest"]!=json!(digest)
            || proposal.snapshot["validator_id"]!=json!(VALIDATOR_ID)
        { return Err(CoreError::new("INPUT_CHANGED","Data validation selection changed after native owner proposal")); }
        let existing:Option<String>=self.conn.query_row("SELECT id FROM data_validation_policies WHERE approval_id=?1",[&proposal.id],|r|r.get(0)).optional().map_err(db_error)?;
        if let Some(id)=existing { return Ok(json!(self.get_data_validation_policy(&id)?)); }
        let session=self.get_session(&selection.session_id)?;
        let policy=DataValidationPolicy { id:Uuid::new_v4().to_string(),task_id:session.task_id,project_id:session.project_id,session_id:session.id,attempt_id:session.attempt_id,workspace_id:session.workspace_id,approval_id:proposal.id.clone(),revision:1,revoked:false,scope:"data_contract_only".into(),validator_id:VALIDATOR_ID.into(),file_contract:contract.artifact_policy,table:contract.table,contract_digest:digest,created_at:now()? };
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        tx.execute("INSERT INTO data_validation_policies VALUES(?1,?2,?3,?4,?5,?6,?7)",params![policy.id,policy.task_id,policy.session_id,policy.attempt_id,policy.workspace_id,policy.approval_id,encode(&policy)?]).map_err(db_error)?;
        event(&tx,&policy.id,1,"data_validation.policy_approved",json!({"policy":policy,"scope":"data_contract_only"}),"local_owner_confirmation")?;
        tx.commit().map_err(db_error)?;
        Ok(json!(policy))
    }
    pub fn get_data_validation_policy(&self,id:&str)->Result<DataValidationPolicy> {
        let policy:DataValidationPolicy=decode(&bounded_document(&self.conn,"data_validation_policies",id)?)?;
        if policy.id!=id { return Err(CoreError::new("INPUT_CHANGED","Stored data policy identity is inconsistent")); }
        Ok(policy)
    }
    pub fn list_data_validation_policies(&self,task_id:&str)->Result<Vec<DataValidationPolicy>> {
        self.get(task_id)?; bounded_documents(&self.conn,"data_validation_policies",task_id)
    }
    pub fn list_data_validation_records(&self,task_id:&str)->Result<Vec<DataValidationRecord>> {
        self.get(task_id)?; bounded_documents(&self.conn,"data_validation_runs",task_id)
    }
    pub fn revoke_data_validation_policy(&mut self,policy_id:&str,expected_revision:u64)->Result<DataValidationPolicy> {
        let mut policy=self.get_data_validation_policy(policy_id)?;
        if policy.revision!=expected_revision { return Err(CoreError::new("STALE_REVISION","Data validation policy changed")); }
        if policy.revoked { return Ok(policy); }
        policy.revoked=true;policy.revision=policy.revision.checked_add(1).ok_or_else(invalid)?;
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        let affected=tx.execute("UPDATE data_validation_policies SET document=?1 WHERE id=?2 AND json_extract(document,'$.revision')=?3",params![encode(&policy)?,policy.id,expected_revision]).map_err(db_error)?;
        if affected!=1 { return Err(CoreError::new("STALE_REVISION","Data validation policy changed")); }
        event(&tx,&policy.id,policy.revision,"data_validation.policy_revoked",json!({"policy_id":policy.id,"existing_evidence_retained":true}),"user_action")?;
        tx.commit().map_err(db_error)?;Ok(policy)
    }
    pub fn run_data_validation(&mut self,policy_id:&str,expected_revision:u64,idempotency_key:&str)->Result<DataValidationRecord> {
        key(idempotency_key)?;
        let payload=encode(&json!({"policy_id":policy_id,"revision":expected_revision}))?;
        let old:Option<(i64,i64)>=self.conn.query_row("SELECT octet_length(payload),octet_length(document) FROM data_validation_runs WHERE policy_id=?1 AND idempotency_key=?2",params![policy_id,idempotency_key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?;
        if let Some((p,d))=old {
            if !(0..=MAX_RECORD_BYTES).contains(&p) || !(0..=MAX_RECORD_BYTES).contains(&d) { return Err(limited()); }
            let (original,record):(String,String)=self.conn.query_row("SELECT payload,document FROM data_validation_runs WHERE policy_id=?1 AND idempotency_key=?2 AND octet_length(payload)<=?3 AND octet_length(document)<=?3",params![policy_id,idempotency_key,MAX_RECORD_BYTES],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_error)?.ok_or_else(limited)?;
            if original!=payload { return Err(CoreError::new("CONFIG_CONFLICT","Data validation key was already used with another request")); }
            // Historical result, never a new check after revocation or drift.
            return decode(&record);
        }
        let policy=self.get_data_validation_policy(policy_id)?;
        if policy.revoked { return Err(CoreError::new("PERMISSION_DENIED","Data validation policy was revoked")); }
        if policy.revision!=expected_revision { return Err(CoreError::new("STALE_REVISION","Data validation policy changed")); }
        let (session,workspace,binding)=self.validation_workspace(&policy.session_id)?;
        let task=self.get(&session.task_id)?;
        let subject=ValidationSubject { task_id:task.id.clone(),attempt_id:session.attempt_id.clone(),spec_revision:task.spec_revision,input_revision:binding.input_revision.clone(),version:VersionBinding::Data { dataset_revision:binding.input_revision.clone() } };
        if policy.validator_id!=VALIDATOR_ID || policy.scope!="data_contract_only" || policy.task_id!=session.task_id
            || policy.project_id!=session.project_id || policy.attempt_id!=session.attempt_id || policy.workspace_id!=workspace.id
            || policy.file_contract.subject!=subject
        { return Err(CoreError::new("INPUT_CHANGED","Data validation subject or attempt binding changed")); }
        let proposal:ApprovalProposal=decode(&bounded_document(&self.conn,"approval_proposals",&policy.approval_id)?)?;
        let confirmed:bool=self.conn.query_row("SELECT result IS NOT NULL FROM approval_proposals WHERE id=?1",[&policy.approval_id],|r|r.get(0)).map_err(db_error)?;
        if !confirmed || !proposal.consumed || proposal.kind!="data_validation"
            || proposal.snapshot["file_contract"]!=json!(policy.file_contract) || proposal.snapshot["table"]!=json!(policy.table)
            || proposal.snapshot["contract_digest"]!=json!(policy.contract_digest) || proposal.snapshot["validator_id"]!=json!(VALIDATOR_ID)
        { return Err(CoreError::new("INPUT_CHANGED","Data policy no longer matches its confirmed owner snapshot")); }
        let bind=|contracts:&[ArtifactContractView]|->Result<Vec<BoundArtifact>> {
            contracts.iter().map(|contract| {
                if contract.canonical_root!=workspace.root { return Err(CoreError::new("INPUT_CHANGED","Approved data root no longer matches its workspace")); }
                BoundArtifact::from_verified_contract(contract).map_err(changed)
            }).collect()
        };
        let rule=ApprovedJsonTableRule::from_verified_policy(policy.approval_id.clone(),policy.revision.to_string(),subject,bind(&policy.file_contract.inputs)?,bind(&policy.file_contract.outputs)?,policy.table.clone()).map_err(changed)?;
        let canonical=rule.contract_view();
        if canonical.artifact_policy!=policy.file_contract || canonical.table!=policy.table || rule.contract_digest()!=policy.contract_digest {
            return Err(CoreError::new("INPUT_CHANGED","Persisted data contract digest is inconsistent"));
        }
        let evidence=validate_json_table(&rule).map_err(changed)?;
        evidence.verify_current().map_err(changed)?;
        let view=evidence.view();
        if view.passed { let _contract_only=evidence.accept_contract(&rule).map_err(changed)?; }
        let record=DataValidationRecord { id:Uuid::new_v4().to_string(),policy_id:policy.id.clone(),task_id:policy.task_id.clone(),state:if view.passed {"contract_passed"}else{"contract_failed"}.into(),scope:"data_contract_only".into(),correctness:"approved_schema_ranges_metadata_only".into(),task_acceptance:"not_established".into(),integration_authorized:false,evidence:serde_json::to_value(view).map_err(|_|invalid())?,created_at:now()? };
        let tx=self.conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        // Recheck revocation/revision at the commit boundary as well.
        let current:String=tx.query_row("SELECT document FROM data_validation_policies WHERE id=?1",[policy_id],|r|r.get(0)).map_err(db_error)?;
        let current:DataValidationPolicy=decode(&current)?;
        if current.revoked || current.revision!=expected_revision || current.contract_digest!=policy.contract_digest {
            return Err(CoreError::new("STALE_REVISION","Data policy changed during validation"));
        }
        tx.execute("INSERT INTO data_validation_runs VALUES(?1,?2,?3,?4,?5,?6,?7)",params![record.id,record.policy_id,record.task_id,record.state,encode(&record)?,idempotency_key,payload]).map_err(db_error)?;
        event(&tx,&record.id,1,"data_validation.contract_recorded",json!({"record":record}),"deterministic_builtin_data_validator")?;
        tx.commit().map_err(db_error)?;Ok(record)
    }
}
