import { taskHasAcceptanceProjection,type Task } from './api';
import { safeInteger, utf8Bytes, type GoalScope } from './goal-api';
import type { GraphArtifactReference, GraphRunSummary, GraphRunDetails, GraphOwnerDecisionResult,GraphTaskAcceptance } from './goal-graph-types';
export const graphRunStates=['queued','running','cancel_requested','cancelled','awaiting_acceptance','blocked','failed','outcome_unknown','accepted'];
export const graphNodeStates=['not_started','reserved','running','stopping','capturing','candidate','validating','validated','awaiting_owner','accepted','rejected','failed','cancelled','outcome_unknown'];
export const graphAcceptanceStates=['not_applicable','pending','validated','accepted','rejected','stale','unknown'];
const object=(x:unknown):x is Record<string,any>=>!!x&&typeof x==='object'&&!Array.isArray(x);
const text=(x:unknown,max=16384):x is string=>typeof x==='string'&&x.length>0&&utf8Bytes(x)<=max;
const id=(x:unknown):x is string=>typeof x==='string'&&/^[A-Za-z0-9._:-]{1,256}$/.test(x);
const hash=(x:unknown):x is string=>typeof x==='string'&&/^[a-f0-9]{64}$/.test(x);
const ids=(x:unknown):x is string[]=>Array.isArray(x)&&x.length<=65536&&x.every(id)&&new Set(x).size===x.length;
const meter=(x:unknown)=>x===null||safeInteger(x);
const optionalId=(x:unknown)=>x===null||id(x);
const relative=(x:unknown):x is string=>text(x,4096)&&!x.startsWith('/')&&!/[\\\u0000-\u001f\u007f]/.test(x)&&x.split('/').every(part=>!!part&&part!=='.'&&part!=='..');
export function graphArtifactPrecision(value:unknown):boolean {
  if(!object(value)||!object(value.root)||!object(value.file_pin))return false;
  const root=value.root,pin=value.file_pin;
  return safeInteger(root.device)&&safeInteger(root.inode)&&['device','inode','bytes'].every(key=>safeInteger(pin[key]))&&['uid','gid','mode'].every(key=>safeInteger(pin[key],0,4294967295))&&['mtime','ctime'].every(key=>safeInteger(pin[key],Number.MIN_SAFE_INTEGER))&&['mtime_nsec','ctime_nsec'].every(key=>safeInteger(pin[key],0,999999999));
}
export function graphArtifactShape(x:unknown,scope:GoalScope):x is GraphArtifactReference {
  if(!object(x)||!id(x.candidate_id)||!id(x.artifact_id)||!id(x.contract_id)||!id(x.producer_node_id)||!id(x.producer_attempt_id)||!hash(x.sha256)||!safeInteger(x.bytes,0,67108864)||!relative(x.relative_path)||!safeInteger(x.writer_epoch)||scope.outputs[x.artifact_id]!==x.contract_id)return false;
  // JavaScript may round a Rust u64 before comparison or display. Such a
  // projection cannot support artifact trust or an Owner action. Native/Core
  // retain complete original integer Values; no wire field is rewritten.
  const root=x.root,pin=x.file_pin;
  return graphArtifactPrecision(x)&&object(root)&&text(root.canonical_root,4096)&&root.canonical_root.startsWith('/')&&object(root.kind)&&['directory','git'].includes(root.kind.kind)&&object(pin)&&pin.bytes===x.bytes;
}
export function graphSummaryShape(x:unknown,task:Task,scope?:GoalScope):x is GraphRunSummary {
  if(!object(x)||x.graph_contract_version!==1||!safeInteger(x.ledger_revision)||typeof x.task_accepted!=='boolean'||!object(x.run))return false;
  const r=x.run;
  return id(r.id)&&id(r.scope_id)&&id(r.root_grant_id)&&r.task_id===task.id&&r.project_id===task.project_id&&safeInteger(r.plan_revision,1)&&hash(r.plan_digest)&&safeInteger(r.revision,1)&&text(r.state,128)&&typeof r.cancel_requested==='boolean'&&typeof r.task_accepted==='boolean'&&r.task_accepted===x.task_accepted&&text(r.created_at,128)&&text(r.updated_at,128)&&(!scope||r.scope_id===scope.id&&r.root_grant_id===scope.root_grant_id)&&(r.state!=='cancel_requested'&&r.state!=='cancelled'||r.cancel_requested)&&(!r.task_accepted||r.state==='accepted'&&!r.cancel_requested)&&(r.state!=='accepted'||r.task_accepted);
}
export function graphDetailsShape(value:unknown,task:Task,scope:GoalScope):value is GraphRunDetails {
  if(!object(value)||!graphSummaryShape(value,task,scope))return false;
  const x:Record<string,any>=value;
  if(!Array.isArray(x.nodes)||!Array.isArray(x.candidates)||!Array.isArray(x.validations)||!Array.isArray(x.owner_decisions)||!x.nodes.length||x.nodes.length>65536||[x.candidates,x.validations,x.owner_decisions].some(rows=>rows.length>65536)||utf8Bytes(JSON.stringify(x))>4194304)return false;
  const r=x.run,keys=new Set<string>(),nodeKey=(rev:number,node:string)=>JSON.stringify([rev,node]);
  const nodes=new Map<string,Record<string,any>[]>();
  for(const record of x.nodes){
    if(!object(record)||!object(record.node)||!safeInteger(record.plan_revision,1)||record.plan_revision>r.plan_revision||!id(record.node.id)||!text(record.node.goal)||!['work','validate','owner_approval','summary'].includes(record.node.kind)||record.node.workspace_id!==scope.workspace_id||!text(record.state,128)||!text(record.execution_kind,128)||!text(record.acceptance_state,128)||typeof record.stop_confirmed!=='boolean'||![record.session_id,record.attempt_id,record.job_id].every(optionalId)||![record.actual_runtime_ms,record.actual_output_bytes,record.tokens,record.cost_microunits].every(meter)||![record.candidate_ids,record.validation_ids,record.owner_decision_ids].every(ids)||!(record.blocked_reason===null||text(record.blocked_reason)))return false;
    if(!Array.isArray(record.node.dependencies)||!record.node.dependencies.every(id)||!object(record.node.reservation)||!safeInteger(record.node.reservation.runtime_ms,1)||!safeInteger(record.node.reservation.output_bytes,1)||![record.node.reservation.tokens,record.node.reservation.cost_microunits].every(meter))return false;
    const key=JSON.stringify([record.plan_revision,record.node.id,record.attempt_id??record.job_id]);if(keys.has(key))return false;keys.add(key);
    const key2=nodeKey(record.plan_revision,record.node.id);nodes.set(key2,[...(nodes.get(key2)??[]),record]);
  }
  const candidates=new Map<string,Record<string,any>>(),validations=new Map<string,Record<string,any>>(),decisions=new Map<string,Record<string,any>>();
  const base=(record:any)=>object(record)&&id(record.id)&&record.graph_contract_version===1&&record.run_id===r.id&&safeInteger(record.plan_revision,1)&&record.plan_revision<=r.plan_revision&&hash(record.plan_digest)&&(record.plan_revision!==r.plan_revision||record.plan_digest===r.plan_digest)&&id(record.node_id)&&nodes.has(nodeKey(record.plan_revision,record.node_id))&&text(record.created_at,128)&&text(record.source,128);
  for(const c of x.candidates){
    if(!base(c)||candidates.has(c.id)||c.scope_id!==scope.id||c.task_id!==task.id||c.spec_revision!==scope.goal.spec_revision||c.input_revision!==scope.input_revision||c.workspace_id!==scope.workspace_id||!id(c.attempt_id)||!id(c.session_id)||!hash(c.capability_digest)||!safeInteger(c.stop_receipt_sequence,1)||c.stop_receipt_sequence>x.ledger_revision||!id(c.capture_job_id)||!safeInteger(c.capture_receipt_sequence,c.stop_receipt_sequence+1)||c.capture_receipt_sequence>x.ledger_revision||!text(c.integrity,128)||!text(c.task_acceptance,128)||!Array.isArray(c.artifacts)||c.artifacts.length>64||!Array.isArray(c.consumed_inputs)||c.consumed_inputs.length>64)return false;
    const origin=nodes.get(nodeKey(c.plan_revision,c.node_id))!.find(n=>n.attempt_id===c.attempt_id&&n.session_id===c.session_id&&n.stop_confirmed&&n.candidate_ids.includes(c.id));if(!origin)return false;
    const artifacts=new Set<string>();
    for(const ref of c.artifacts){if(!graphArtifactShape(ref,scope)||ref.candidate_id!==c.id||ref.producer_node_id!==c.node_id||ref.producer_attempt_id!==c.attempt_id||artifacts.has(ref.artifact_id))return false;artifacts.add(ref.artifact_id);}
    for(const input of c.consumed_inputs)if(!object(input)||!id(input.artifact_id)||!id(input.contract_id)||!hash(input.sha256)||!(input.candidate===null||graphArtifactShape(input.candidate,scope)&&input.candidate.sha256===input.sha256&&input.candidate.artifact_id===input.artifact_id&&input.candidate.contract_id===input.contract_id))return false;
    candidates.set(c.id,c);
  }
  const refMatches=(ref:any)=>graphArtifactShape(ref,scope)&&candidates.get(ref.candidate_id)?.artifacts.some((actual:any)=>actual.artifact_id===ref.artifact_id&&actual.contract_id===ref.contract_id&&actual.sha256===ref.sha256&&actual.bytes===ref.bytes&&actual.relative_path===ref.relative_path&&actual.producer_node_id===ref.producer_node_id&&actual.producer_attempt_id===ref.producer_attempt_id&&actual.writer_epoch===ref.writer_epoch&&canonical(actual)===canonical(ref));
  for(const v of x.validations){
    if(!base(v)||validations.has(v.id)||!id(v.attempt_id)||!id(v.rule_id)||!hash(v.definition_digest)||!Array.isArray(v.candidate_refs)||v.candidate_refs.length>64||!v.candidate_refs.every(refMatches)||!Array.isArray(v.input_bindings)||v.input_bindings.length>64||v.input_bindings.some((input:any)=>!object(input)||!id(input.artifact_id)||!id(input.contract_id)||!hash(input.sha256)||!(input.candidate===null||refMatches(input.candidate)&&input.candidate.sha256===input.sha256&&input.candidate.artifact_id===input.artifact_id&&input.candidate.contract_id===input.contract_id))||!text(v.state,128)||typeof v.passed!=='boolean'||!text(v.scope,128)||!text(v.correctness,128)||!text(v.task_acceptance,128)||!hash(v.result_digest)||!text(v.details_ref,4096)||typeof v.stop_confirmed!=='boolean'||!text(v.io_completion,128)||!meter(v.actual_runtime_ms))return false;
    validations.set(v.id,v);
  }
  for(const d of x.owner_decisions){
    if(!base(d)||decisions.has(d.id)||!id(d.approval_id)||!id(d.rule_id)||!hash(d.criterion_digest)||!Array.isArray(d.candidate_refs)||d.candidate_refs.length>64||!d.candidate_refs.every(refMatches)||!ids(d.validation_ids)||!d.validation_ids.every((ref:string)=>validations.has(ref))||!text(d.decision,128))return false;decisions.set(d.id,d);
  }
  for(const node of x.nodes){
    if(!node.candidate_ids.every((key:string)=>candidates.get(key)?.node_id===node.node.id&&candidates.get(key)?.plan_revision===node.plan_revision)||!node.validation_ids.every((key:string)=>validations.get(key)?.node_id===node.node.id&&validations.get(key)?.plan_revision===node.plan_revision)||!node.owner_decision_ids.every((key:string)=>decisions.get(key)?.node_id===node.node.id&&decisions.get(key)?.plan_revision===node.plan_revision))return false;
  }
  const accepted=x.task_acceptance;
  if(x.task_accepted){
    if(!object(accepted)||!id(accepted.id)||accepted.graph_contract_version!==1||accepted.task_id!==task.id||accepted.project_id!==task.project_id||accepted.run_id!==r.id||accepted.scope_id!==scope.id||accepted.root_grant_id!==scope.root_grant_id||accepted.plan_revision!==r.plan_revision||accepted.plan_digest!==r.plan_digest||accepted.spec_revision!==scope.goal.spec_revision||accepted.input_revision!==scope.input_revision||accepted.workspace_id!==scope.workspace_id||!safeInteger(accepted.task_revision,1)||!hash(accepted.capability_digest)||!hash(accepted.criterion_digest)||!safeInteger(accepted.sequence,1)||accepted.sequence>x.ledger_revision||accepted.source!=='baton_goal_acceptance'||!text(accepted.accepted_at,128)||!Array.isArray(accepted.artifact_refs)||accepted.artifact_refs.length>64||!accepted.artifact_refs.every(refMatches)||!ids(accepted.validation_ids)||!accepted.validation_ids.every((id:string)=>validations.has(id))||!ids(accepted.owner_decision_ids)||!accepted.owner_decision_ids.every((id:string)=>decisions.has(id)))return false;
    for(const rule of scope.required_acceptance_rules){
      const owner=accepted.owner_decision_ids.some((id:string)=>{const d=decisions.get(id);return d?.rule_id===rule&&d.decision==='accepted'&&d.plan_revision===r.plan_revision&&d.plan_digest===r.plan_digest;});
      const validator=scope.edit_context?.rule_executors[rule]==='validator'&&accepted.validation_ids.some((id:string)=>{const v=validations.get(id);return v?.rule_id===rule&&v.state==='passed'&&v.passed&&v.plan_revision===r.plan_revision&&v.plan_digest===r.plan_digest&&v.stop_confirmed&&v.io_completion==='worker_joined';});
      if(!owner&&!validator)return false;
    }
  }else if(accepted!==null)return false;
  return true;
}
export function graphTaskAcceptanceMatchesTask(receipt:GraphTaskAcceptance|null,task:Task,scope:Pick<GoalScope,'id'|'root_grant_id'|'input_revision'|'workspace_id'>,view:GraphRunDetails):boolean {
  return taskHasAcceptanceProjection(task)&&graphViewConsistent(view)&&!!receipt&&receipt.graph_contract_version===1&&receipt.source==='baton_goal_acceptance'&&receipt.id===task.acceptance_ref&&receipt.sequence===task.acceptance_sequence&&receipt.task_id===task.id&&receipt.project_id===task.project_id&&receipt.task_revision===task.revision&&receipt.spec_revision===task.spec_revision&&receipt.scope_id===scope.id&&receipt.root_grant_id===scope.root_grant_id&&receipt.input_revision===scope.input_revision&&receipt.workspace_id===scope.workspace_id&&receipt.run_id===view.run.id&&receipt.plan_revision===view.run.plan_revision&&receipt.plan_digest===view.run.plan_digest&&receipt.sequence<=view.ledger_revision;
}
export function graphDecisionShape(x:unknown,runId:string,nodeId:string,planRevision:number,revision:number,decision:'accepted'|'rejected'):x is GraphOwnerDecisionResult {
  return object(x)&&x.graph_contract_version===1&&id(x.approval_id)&&x.run_id===runId&&x.node_id===nodeId&&x.plan_revision===planRevision&&x.decision===decision&&id(x.owner_decision_id)&&safeInteger(x.run_revision,revision+1)&&typeof x.task_accepted==='boolean'&&(decision!=='rejected'||!x.task_accepted);
}

// This floor detects rollback/forked UI evidence. It never turns renderer
// observations into approval authority; Core rechecks every original reference.
export interface GraphEvidenceFloor {summary:GraphRunSummary;details:GraphRunDetails|null}
const canonical=(value:unknown):string=>JSON.stringify(value,(_key,item)=>object(item)?Object.fromEntries(Object.entries(item).sort(([a],[b])=>a.localeCompare(b))):item);
export function graphSummaryMeetsFloor(next:GraphRunSummary,floor:GraphRunSummary):boolean {
  const a=next.run,b=floor.run;
  return next.graph_contract_version===floor.graph_contract_version&&next.ledger_revision>=floor.ledger_revision&&a.id===b.id&&a.scope_id===b.scope_id&&a.project_id===b.project_id&&a.task_id===b.task_id&&a.root_grant_id===b.root_grant_id&&a.created_at===b.created_at&&a.revision>=b.revision&&a.plan_revision>=b.plan_revision&&(a.plan_revision!==b.plan_revision||a.plan_digest===b.plan_digest)&&(!b.cancel_requested||a.cancel_requested)&&(!b.task_accepted||a.task_accepted)&&(a.revision!==b.revision||canonical(a)===canonical(b));
}
// A replayed exact-key result is historical evidence, not a new observation.
// It may acknowledge the original request while preserving all newer floors.
export const graphReceiptFitsFloor=(receipt:GraphRunSummary,observed:GraphRunSummary,exactUncertainReplay:boolean)=>graphSummaryMeetsFloor(receipt,observed)||exactUncertainReplay&&graphSummaryMeetsFloor(observed,receipt);
export function graphDetailsMeetFloor(next:GraphRunDetails,floor:GraphEvidenceFloor):boolean {
  if(!graphSummaryMeetsFloor(next,floor.summary))return false;
  const before=floor.details;if(!before)return true;
  if(before.task_acceptance!==null&&canonical(before.task_acceptance)!==canonical(next.task_acceptance))return false;
  for(const name of ['candidates','validations','owner_decisions'] as const){
    const records=new Map(next[name].map(item=>[item.id,canonical(item)]));
    if(before[name].some(item=>records.get(item.id)!==canonical(item)))return false;
  }
  for(const original of before.nodes){
    const sameNode=next.nodes.filter(item=>item.plan_revision===original.plan_revision&&item.node.id===original.node.id);
    if(!sameNode.length||sameNode.some(item=>canonical(item.node)!==canonical(original.node)))return false;
    // The initial no-attempt placeholder may become an attempt. A real attempt
    // or intrinsic job is retained forever rather than overwritten on rework.
    if(original.attempt_id!==null||original.job_id!==null){
      const retained=sameNode.find(item=>original.attempt_id!==null?item.attempt_id===original.attempt_id:item.attempt_id===null&&item.job_id===original.job_id);
      if(!retained||retained.session_id!==original.session_id||original.job_id!==null&&retained.job_id!==original.job_id||original.stop_confirmed&&!retained.stop_confirmed||original.candidate_ids.some(id=>!retained.candidate_ids.includes(id))||original.validation_ids.some(id=>!retained.validation_ids.includes(id))||original.owner_decision_ids.some(id=>!retained.owner_decision_ids.includes(id)))return false;
    }
  }
  return next.run.revision!==before.run.revision||canonical({...next,ledger_revision:0})===canonical({...before,ledger_revision:0});
}
export const graphExecutionKinds:readonly string[]=['connection','validator','owner'];
export function graphViewSupported(view:GraphRunDetails):boolean {
  return graphRunStates.includes(view.run.state)&&view.nodes.every(node=>graphNodeStates.includes(node.state)&&graphAcceptanceStates.includes(node.acceptance_state)&&graphExecutionKinds.includes(node.execution_kind)&&(node.execution_kind==='connection'?['work','summary'].includes(node.node.kind):node.execution_kind==='validator'?node.node.kind==='validate':node.node.kind==='owner_approval'))
    &&view.candidates.every(c=>c.source==='baton_goal_evidence'&&c.integrity==='verified'&&c.task_acceptance==='not_established'&&c.artifacts.every(graphArtifactPrecision))
    &&view.validations.every(v=>v.source==='baton_goal_validator'&&['passed','failed','cancelled','outcome_unknown'].includes(v.state)&&['worker_joined','not_started','outcome_unknown'].includes(v.io_completion)&&['artifact_integrity_only','data_contract_only','approved_deterministic_command_only'].includes(v.scope)&&v.correctness===(v.scope==='artifact_integrity_only'?'not_established':v.scope==='data_contract_only'?'approved_schema_ranges_metadata_only':'approved_fixed_command_observations_only')&&v.task_acceptance==='not_established'&&(v.state==='passed')===v.passed)
    &&view.validations.every(v=>v.candidate_refs.every(graphArtifactPrecision))&&view.owner_decisions.every(d=>d.candidate_refs.every(graphArtifactPrecision))&&(!view.task_acceptance||view.task_acceptance.artifact_refs.every(graphArtifactPrecision))
    &&view.owner_decisions.every(d=>d.decision==='accepted'&&d.source==='local_owner_confirmation'||d.decision==='rejected'&&d.source==='local_owner_rejection');
}
export function graphNodeClosed(view:GraphRunDetails,node:GraphRunDetails['nodes'][number]):boolean {
  if(!graphViewSupported(view)||node.acceptance_state==='unknown'||node.acceptance_state==='stale')return false;
  if(node.state==='not_started')return node.session_id===null&&node.attempt_id===null&&node.job_id===null&&!node.stop_confirmed;
  if(node.execution_kind==='owner')return node.session_id===null&&node.attempt_id===null&&node.job_id===null&&['accepted','rejected'].includes(node.state)&&node.owner_decision_ids.some(id=>view.owner_decisions.some(d=>d.id===id&&d.node_id===node.node.id&&d.plan_revision===node.plan_revision&&d.decision===node.state));
  if(node.execution_kind==='connection')return ['candidate','failed','cancelled'].includes(node.state)&&node.session_id!==null&&node.attempt_id!==null&&node.stop_confirmed&&(node.state!=='candidate'||node.candidate_ids.some(id=>view.candidates.some(c=>c.id===id&&c.attempt_id===node.attempt_id&&c.node_id===node.node.id&&c.plan_revision===node.plan_revision)));
  return ['validated','failed','cancelled'].includes(node.state)&&node.session_id===null&&node.validation_ids.some(id=>view.validations.some(v=>v.id===id&&v.node_id===node.node.id&&v.plan_revision===node.plan_revision&&(v.io_completion==='worker_joined'&&v.stop_confirmed&&node.stop_confirmed&&node.attempt_id!==null&&node.job_id!==null||v.io_completion==='not_started'&&v.state!=='passed')&&(node.state!=='validated'||v.state==='passed'&&v.passed)));
}
export function graphReferencesBound(view:GraphRunDetails):boolean {
  const captured=new Map<string,string>();
  for(const candidate of view.candidates)for(const ref of candidate.artifacts){
    if(!graphArtifactPrecision(ref)||ref.candidate_id!==candidate.id||ref.producer_node_id!==candidate.node_id||ref.producer_attempt_id!==candidate.attempt_id)return false;
    const key=JSON.stringify([ref.candidate_id,ref.artifact_id]);if(captured.has(key))return false;captured.set(key,canonical(ref));
  }
  const refs=[...view.candidates.flatMap(c=>c.consumed_inputs.flatMap(input=>input.candidate?[input.candidate]:[])),...view.validations.flatMap(v=>[...v.candidate_refs,...v.input_bindings.flatMap(input=>input.candidate?[input.candidate]:[])]),...view.owner_decisions.flatMap(d=>d.candidate_refs),...(view.task_acceptance?.artifact_refs??[])];
  return refs.every(ref=>graphArtifactPrecision(ref)&&captured.get(JSON.stringify([ref.candidate_id,ref.artifact_id]))===canonical(ref));
}
export function graphViewConsistent(view:GraphRunDetails):boolean {
  if(!graphViewSupported(view)||!graphReferencesBound(view))return false;
  const nodesKnown=view.nodes.every(node=>{
    if(node.state==='not_started')return node.session_id===null&&node.attempt_id===null&&node.job_id===null&&!node.stop_confirmed;
    if(node.execution_kind==='owner')return node.session_id===null&&node.attempt_id===null&&node.job_id===null&&(node.state==='awaiting_owner'&&node.acceptance_state==='pending'||graphNodeClosed(view,node));
    if(node.execution_kind==='connection')return node.session_id!==null&&node.attempt_id!==null&&(node.state==='outcome_unknown'||['reserved','running','stopping','capturing'].includes(node.state)&&!node.stop_confirmed||graphNodeClosed(view,node));
    return node.session_id===null&&(node.state==='outcome_unknown'||['reserved','running','stopping','validating'].includes(node.state)&&node.attempt_id!==null&&node.job_id!==null&&!node.stop_confirmed||graphNodeClosed(view,node));
  });
  return nodesKnown&&(!['accepted','cancelled','failed'].includes(view.run.state)||view.nodes.every(node=>graphNodeClosed(view,node)));
}
export const graphViewClosed=(view:GraphRunDetails)=>graphViewConsistent(view)&&view.nodes.every(node=>graphNodeClosed(view,node));
export const graphCanAdvanceView=(view:GraphRunDetails)=>!view.run.cancel_requested&&['queued','running','blocked','awaiting_acceptance'].includes(view.run.state)&&graphViewClosed(view);
export const graphCanReviewOwner=(view:GraphRunDetails,node:GraphRunDetails['nodes'][number])=>graphViewConsistent(view)&&!view.run.cancel_requested&&!['accepted','cancelled','failed','outcome_unknown','cancel_requested'].includes(view.run.state)&&!view.nodes.some(record=>record.state==='outcome_unknown'||record.acceptance_state==='unknown'||record.acceptance_state==='stale')&&!view.validations.some(check=>check.state==='outcome_unknown'||check.io_completion==='outcome_unknown')&&node.plan_revision===view.run.plan_revision&&node.node.kind==='owner_approval'&&node.execution_kind==='owner'&&node.state==='awaiting_owner'&&node.acceptance_state==='pending'&&node.session_id===null&&node.attempt_id===null&&node.job_id===null;

export const graphRunPolling=(summary:GraphRunSummary)=>['queued','running','cancel_requested'].includes(summary.run.state);
export const graphRunCanCancel=(summary:GraphRunSummary)=>!summary.run.cancel_requested&&['queued','running','blocked','outcome_unknown','awaiting_acceptance'].includes(summary.run.state);
