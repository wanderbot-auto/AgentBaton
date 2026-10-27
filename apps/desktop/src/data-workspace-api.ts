import { request, type Project } from './api';
import type { ArtifactRequirement } from './validation-api';

export interface DataLimits { max_file_bytes:number; max_total_bytes:number; max_files:number }
export interface DataSelection { project_id:string; files:ArtifactRequirement[]; limits:DataLimits }
// Host identities are read-only observations, never permission tokens or values
// to roundtrip through JavaScript's narrower integer representation.
export interface DataDirectoryPin { canonical_root:string; device:unknown; inode:unknown }
export interface DatasetSnapshot {
  contract:{format_version:number;approval_reference:string;source:DataDirectoryPin;files:ArtifactRequirement[];limits:DataLimits};
  selection_digest:string;dataset_revision:string;files:{requirement:ArtifactRequirement;source_identity:Record<string,unknown>;sha256:string;bytes:number;destination_relative_path:string}[];
  total_bytes:number;source_observation_scope:string;atomic_directory_freeze:boolean;
}
export interface DataOperationError { code:string;message:string;retryable:boolean;side_effect_state:string }
export interface DataPreview { id:string;project_id:string;project_revision:number;revision:number;selection:DataSelection;state:string;snapshot:DatasetSnapshot|null;error:DataOperationError|null;created_at:string;updated_at:string }
export interface DataCopiedFile { input:DatasetSnapshot['files'][number]; destination_identity:Record<string,unknown> }
export interface DataCopyContract { format_version:number;creation_id:string;snapshot:DatasetSnapshot;managed_root:DataDirectoryPin;destination:DataDirectoryPin;files:DataCopiedFile[];directories:DataDirectoryPin[];contract_digest:string }
export interface DataCreationObservation { creation_id:string;destination:string;state:string;dataset_revision:string;selection_digest:string;copied_files:number;error:unknown;source_observation_scope:string;atomic_directory_freeze:boolean;filesystem_sandbox:boolean }
export interface DataCreation {
  id:string;preview_id:string;project_id:string;project_revision:number;workspace_id:string;approval_id:string;revision:number;state:string;dataset_revision:string;managed_root:DataDirectoryPin;destination:string;
  snapshot:DatasetSnapshot;contract:DataCopyContract|null;observation:DataCreationObservation|null;cancel_requested:boolean;error:DataOperationError|null;created_at:string;updated_at:string;
  scope:string;filesystem_sandbox:boolean;atomic_directory_freeze:boolean;
  readonly io_completion?: 'worker_joined'|'not_scheduled'|'interrupted_no_join_receipt'|null;
}
// A historical cancellation error alone is not a stopped-worker receipt.
export const dataCopyCancellationUnconfirmed=(value:DataCreation)=>value.state==='failed'&&value.error?.code==='CANCEL_REQUESTED';
export interface DataWorkspaceForm { files:string;maxFileBytes:string;maxTotalBytes:string;maxFiles:string }
export const newDataForm=():DataWorkspaceForm=>({files:'',maxFileBytes:'8388608',maxTotalBytes:'33554432',maxFiles:'16'});
export class DataFormError extends Error {constructor(public reason:'path'|'integer'|'hash'|'duplicate'|'limit'|'selection'){super(reason);}}
export const dataSafeInteger=(value:unknown,min=0,max=Number.MAX_SAFE_INTEGER):value is number=>typeof value==='number'&&Number.isSafeInteger(value)&&value>=min&&value<=max;
const hash=(value:unknown):value is string=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
const id=(value:unknown):value is string=>typeof value==='string'&&/^[A-Za-z0-9._:-]{1,256}$/.test(value);
const relative=(value:unknown):value is string=>typeof value==='string'&&!!value&&!value.startsWith('/')&&!/^[A-Za-z]:/.test(value)&&!/[\\\u0000-\u001f\u007f]/.test(value)&&new TextEncoder().encode(value).length<=4096&&value.split('/').length<=16&&value.split('/').every(part=>!!part&&part!=='.'&&part!=='..');
const sameLimits=(a:DataLimits,b:DataLimits)=>a.max_file_bytes===b.max_file_bytes&&a.max_total_bytes===b.max_total_bytes&&a.max_files===b.max_files;
const sameFile=(a:ArtifactRequirement,b:ArtifactRequirement)=>!!a&&!!b&&a.artifact_id===b.artifact_id&&a.relative_path===b.relative_path&&a.max_bytes===b.max_bytes&&a.expected_sha256===b.expected_sha256;
export const dataSelectionEqual=(a:DataSelection,b:DataSelection)=>a.project_id===b.project_id&&sameLimits(a.limits,b.limits)&&a.files.length===b.files.length&&a.files.every((file,index)=>sameFile(file,b.files[index]));
function integer(text:string,max:number){if(!/^[1-9]\d*$/.test(text)||!dataSafeInteger(Number(text),1,max))throw new DataFormError('integer');return Number(text);}
function requirement(value:ArtifactRequirement):boolean{return !!value&&id(value.artifact_id)&&relative(value.relative_path)&&dataSafeInteger(value.max_bytes,1,67108864)&&(value.expected_sha256===null||hash(value.expected_sha256));}
export function parseDataSelection(project:Project,form:DataWorkspaceForm):DataSelection {
  if(project.archived||project.identity.kind.kind!=='directory'||!dataSafeInteger(project.revision,1))throw new DataFormError('selection');
  const limits={max_file_bytes:integer(form.maxFileBytes,67108864),max_total_bytes:integer(form.maxTotalBytes,268435456),max_files:integer(form.maxFiles,64)};
  const lines=form.files.split(/\r?\n/).map(line=>line.trim()).filter(Boolean);
  if(!lines.length||lines.length>limits.max_files)throw new DataFormError('limit');
  const paths:string[]=[];
  const files=lines.map((line,index)=>{
    const parts=line.split('|').map(part=>part.trim());if(parts.length>2)throw new DataFormError('selection');
    const [path,expected='']=parts;if(!relative(path))throw new DataFormError('path');
    const lower=path.toLowerCase();if(paths.some(old=>old===lower||old.startsWith(lower+'/')||lower.startsWith(old+'/')))throw new DataFormError('duplicate');paths.push(lower);
    if(expected&&!/^[a-fA-F0-9]{64}$/.test(expected))throw new DataFormError('hash');
    return {artifact_id:`file-${index+1}`,relative_path:path,max_bytes:limits.max_file_bytes,expected_sha256:expected?expected.toLowerCase():null};
  });
  const selection={project_id:project.id,files,limits};if(new TextEncoder().encode(JSON.stringify(selection)).length>16384)throw new DataFormError('limit');return selection;
}
export function dataSelectionShape(value:DataSelection,projectId:string):boolean {
  return !!value&&value.project_id===projectId&&Array.isArray(value.files)&&value.files.length>0&&value.files.length<=64&&value.files.every(requirement)&&new Set(value.files.map(file=>file.artifact_id)).size===value.files.length&&new Set(value.files.map(file=>file.relative_path)).size===value.files.length
    &&!!value.limits&&dataSafeInteger(value.limits.max_file_bytes,1,67108864)&&dataSafeInteger(value.limits.max_total_bytes,1,268435456)&&dataSafeInteger(value.limits.max_files,1,64)&&value.files.length<=value.limits.max_files&&value.files.every(file=>file.max_bytes<=value.limits.max_file_bytes);
}
const sameObservation=(a:DatasetSnapshot['files'][number],b:DatasetSnapshot['files'][number])=>!!a&&!!b&&sameFile(a.requirement,b.requirement)&&a.sha256===b.sha256&&a.bytes===b.bytes&&a.destination_relative_path===b.destination_relative_path;
export const dataSnapshotEqual=(a:DatasetSnapshot,b:DatasetSnapshot)=>dataSnapshotShape(a)&&dataSnapshotShape(b)&&a.dataset_revision===b.dataset_revision&&a.selection_digest===b.selection_digest&&a.contract.approval_reference===b.contract.approval_reference&&a.contract.source.canonical_root===b.contract.source.canonical_root&&sameLimits(a.contract.limits,b.contract.limits)&&a.files.length===b.files.length&&a.files.every((file,index)=>sameObservation(file,b.files[index]));
const pin=(value:DataDirectoryPin)=>!!value&&typeof value.canonical_root==='string'&&value.canonical_root.startsWith('/')&&'device' in value&&'inode' in value;
const operationError=(value:DataOperationError|null)=>value===null||!!value&&typeof value.code==='string'&&typeof value.message==='string'&&typeof value.retryable==='boolean'&&typeof value.side_effect_state==='string';
export function dataSnapshotShape(value:DatasetSnapshot,selection?:DataSelection):boolean {
  if(!value||!value.contract||value.contract.format_version!==1||!id(value.contract.approval_reference)||!pin(value.contract.source)||!hash(value.selection_digest)||!hash(value.dataset_revision)||value.atomic_directory_freeze!==false||value.source_observation_scope!=='selected_files_before_after_only'||!dataSafeInteger(value.total_bytes)||!Array.isArray(value.files)||value.files.length<1||value.files.length>64)return false;
  const declared={project_id:selection?.project_id??'history',files:value.contract.files,limits:value.contract.limits};
  if(!dataSelectionShape(declared,declared.project_id)||(selection&&(value.contract.files.length!==selection.files.length||!value.contract.files.every(file=>selection.files.some(selected=>sameFile(file,selected)))||!sameLimits(value.contract.limits,selection.limits)))||value.files.length!==value.contract.files.length)return false;
  const total=value.files.reduce((sum,file)=>sum+(dataSafeInteger(file?.bytes)?file.bytes:Number.NaN),0);
  return total===value.total_bytes&&total<=value.contract.limits.max_total_bytes&&value.files.every((file,index)=>!!file&&sameFile(file.requirement,value.contract.files[index])&&requirement(file.requirement)&&hash(file.sha256)&&(!file.requirement.expected_sha256||file.requirement.expected_sha256===file.sha256)&&dataSafeInteger(file.bytes,0,file.requirement.max_bytes)&&file.destination_relative_path==='inputs/'+file.requirement.relative_path&&!!file.source_identity&&typeof file.source_identity==='object');
}
export function dataPreviewShape(value:DataPreview,projectId:string):boolean {
  return !!value&&id(value.id)&&value.project_id===projectId&&dataSafeInteger(value.project_revision,1)&&dataSafeInteger(value.revision,1)&&typeof value.state==='string'&&dataSelectionShape(value.selection,projectId)&&typeof value.created_at==='string'&&typeof value.updated_at==='string'&&operationError(value.error)
    &&(value.snapshot===null?value.state!=='verified':dataSnapshotShape(value.snapshot,value.selection)&&value.snapshot.contract.approval_reference===value.id);
}
export function dataCreationShape(value:DataCreation,projectId:string):boolean {
  if(!value||!id(value.id)||!id(value.preview_id)||value.project_id!==projectId||!dataSafeInteger(value.project_revision,1)||!dataSafeInteger(value.revision,1)||!id(value.workspace_id)||!id(value.approval_id)||typeof value.state!=='string'||value.scope!=='explicit_selected_dataset_copy_only'||value.filesystem_sandbox!==false||value.atomic_directory_freeze!==false||typeof value.cancel_requested!=='boolean'||typeof value.destination!=='string'||!value.destination.startsWith('/')||!pin(value.managed_root)||!dataSnapshotShape(value.snapshot)||value.dataset_revision!==value.snapshot.dataset_revision||value.snapshot.contract.approval_reference!==value.preview_id||!operationError(value.error)||typeof value.created_at!=='string'||typeof value.updated_at!=='string')return false;
  const observation=value.observation;
  const io=value.io_completion;
  if(io!==undefined&&io!==null&&!['worker_joined','not_scheduled','interrupted_no_join_receipt'].includes(io))return false;
  if(io==='worker_joined'&&!['ready','failed','outcome_unknown','cancelled'].includes(value.state))return false;
  if(io==='not_scheduled'&&(value.state!=='failed'||observation!==null||value.contract!==null))return false;
  if(io==='interrupted_no_join_receipt'&&(value.state!=='outcome_unknown'||value.contract!==null))return false;
  if(observation!==null&&(!observation||observation.creation_id!==value.id||observation.destination!==value.destination||observation.dataset_revision!==value.dataset_revision||observation.selection_digest!==value.snapshot.selection_digest||!dataSafeInteger(observation.copied_files,0,value.snapshot.files.length)||typeof observation.state!=='string'||observation.filesystem_sandbox!==false||observation.atomic_directory_freeze!==false||observation.source_observation_scope!=='selected_files_before_after_only'))return false;
  if(value.state==='cancelled'&&(io!=='worker_joined'||!value.cancel_requested||value.contract!==null||value.error?.code!=='CANCEL_REQUESTED'||value.error.side_effect_state!=='confirmed'||observation!==null&&(observation.state!=='failed'||observation.error!=='cancel_requested')))return false;
  if(value.state==='ready'&&(!observation||observation.state!=='ready'||observation.error!==null||value.error!==null||observation.copied_files!==value.snapshot.files.length))return false;
  if(value.contract===null)return value.state!=='ready';
  return !!value.contract&&value.contract.format_version===1&&value.contract.creation_id===value.id&&hash(value.contract.contract_digest)&&!!value.contract.snapshot&&dataSnapshotEqual(value.contract.snapshot,value.snapshot)&&pin(value.contract.managed_root)&&value.contract.managed_root.canonical_root===value.managed_root.canonical_root&&pin(value.contract.destination)&&value.contract.destination.canonical_root===value.destination&&Array.isArray(value.contract.files)&&value.contract.files.length===value.snapshot.files.length&&value.contract.files.every((file,index)=>!!file&&sameObservation(file.input,value.snapshot.files[index])&&!!file.destination_identity&&typeof file.destination_identity==='object'&&!Array.isArray(file.destination_identity))&&Array.isArray(value.contract.directories);
}
export const dataWorkspaceApi={
  previewStart:(selection:DataSelection,expectedRevision:number,idempotencyKey:string)=>request<DataPreview>('data_workspace_previews_start',{selection,expectedRevision,idempotencyKey}),
  previewsList:(projectId:string)=>request<DataPreview[]>('data_workspace_previews_list',{projectId}),
  previewGet:(previewId:string)=>request<DataPreview>('data_workspace_previews_get',{previewId}),
  previewCancel:(previewId:string,expectedRevision:number,idempotencyKey:string)=>request<DataPreview>('data_workspace_previews_cancel',{previewId,expectedRevision,idempotencyKey}),
  approve:(previewId:string,expectedRevision:number,idempotencyKey:string)=>request<DataCreation>('data_workspace_approve',{previewId,expectedRevision,idempotencyKey}),
  creationsList:(projectId:string)=>request<DataCreation[]>('data_workspaces_list',{projectId}),
  creationGet:(creationId:string)=>request<DataCreation>('data_workspaces_get',{creationId}),
  creationCancel:(creationId:string,expectedRevision:number,idempotencyKey:string)=>request<DataCreation>('data_workspaces_cancel',{creationId,expectedRevision,idempotencyKey}),
};
