import { describe,expect,it } from 'vitest';
import { graphSummaryShape,graphDetailsShape,graphDetailsMeetFloor,graphSummaryMeetsFloor,graphArtifactShape,graphDecisionShape,graphViewSupported } from './goal-graph-guards';
import { goalGraphFixture,graphCandidateView } from './goal-graph-test-fixtures';
type Mutable<T>={-readonly[K in keyof T]:Mutable<T[K]>};
const mutable=<T,>(value:T):Mutable<T>=>structuredClone(value) as Mutable<T>;
describe('Goal graph draft DTO guards; generated fixtures only',()=>{
  it('reads generated current and historical candidate evidence without declaring acceptance',()=>{
    const f=goalGraphFixture(),v=graphCandidateView(f);expect(graphDetailsShape(f.graphView,f.task,f.scope)).toBe(true);expect(graphDetailsShape(v,f.task,f.scope)).toBe(true);expect(v.task_accepted).toBe(false);
  });
  it.each(['version','foreignscope','foreignroot','ledger','accepted','cancelled'])('rejects malformed or mismatched run summary %s',kind=>{
    const f=goalGraphFixture(),v=mutable(f.summary);if(kind==='version')v.graph_contract_version=2;if(kind==='foreignscope')v.run.scope_id='foreign';if(kind==='foreignroot')v.run.root_grant_id='foreign';if(kind==='ledger')v.ledger_revision=Number.MAX_SAFE_INTEGER+1;if(kind==='accepted')v.run.task_accepted=true;if(kind==='cancelled')v.run.state='cancelled';expect(graphSummaryShape(v,f.task,f.scope)).toBe(false);
  });
  it.each(['attempt','hash','bytes','root','path','pin','origin','missing'])('rejects broken candidate and original artifact binding %s',kind=>{
    const f=goalGraphFixture(),v=graphCandidateView(f),ref=v.candidates[0].artifacts[0];
    if(kind==='attempt')ref.producer_attempt_id='foreign';if(kind==='hash')ref.sha256='missing';if(kind==='bytes')ref.bytes=-1;if(kind==='root')ref.root.canonical_root='relative';if(kind==='path')ref.relative_path='../secret';if(kind==='pin')ref.file_pin.bytes=13;if(kind==='origin')v.nodes[0].stop_confirmed=false;if(kind==='missing')v.nodes[0].candidate_ids=[];
    expect(graphDetailsShape(v,f.task,f.scope)).toBe(false);
  });
  it('rejects unsafe artifact identities before trusting a renderer projection',()=>{
    const f=goalGraphFixture(),ref=graphCandidateView(f).candidates[0].artifacts[0];ref.root.inode=Number.MAX_SAFE_INTEGER+1;ref.file_pin.inode=Number.MAX_SAFE_INTEGER+1;expect(graphArtifactShape(ref,f.scope)).toBe(false);ref.bytes=Number.MAX_SAFE_INTEGER+1;expect(graphArtifactShape(ref,f.scope)).toBe(false);
  });
  it('allows future execution state for display but does not mark the view supported',()=>{
    const f=goalGraphFixture(),v=graphCandidateView(f);v.nodes[0].state='future_unknown_effect';expect(graphDetailsShape(v,f.task,f.scope)).toBe(true);expect(graphViewSupported(v)).toBe(false);
  });
  it('rejects ledger rollback, same revision fork and changed original plan digest',()=>{
    const f=goalGraphFixture(),before=graphCandidateView(f);for(const mode of ['ledger','state','digest']){const next=mutable(before);if(mode==='ledger')--next.ledger_revision;if(mode==='state')next.run.state='blocked';if(mode==='digest')next.run.plan_digest='f'.repeat(64);expect(graphSummaryMeetsFloor(next,before)).toBe(false);}
  });
  it('retains original candidate bytes and attempt history across plan adoption',()=>{
    const f=goalGraphFixture(),before=graphCandidateView(f),floor={summary:before,details:before};const next=mutable(before);next.run.revision++;next.run.plan_revision=2;next.run.plan_digest='f'.repeat(64);next.ledger_revision++;expect(graphDetailsMeetFloor(next,floor)).toBe(true);
    const changed=mutable(next);changed.candidates[0].artifacts[0].sha256='f'.repeat(64);expect(graphDetailsMeetFloor(changed,floor)).toBe(false);const missing=mutable(next);missing.candidates=[];expect(graphDetailsMeetFloor(missing,floor)).toBe(false);const lost=mutable(next);lost.nodes[0].attempt_id=null;expect(graphDetailsMeetFloor(lost,floor)).toBe(false);
  });
  it('does not allow a newer global ledger to conceal changed same-CAS result evidence',()=>{
    const f=goalGraphFixture(),before=graphCandidateView(f),next=mutable(before);next.ledger_revision++;expect(graphDetailsMeetFloor(next,{summary:before,details:before})).toBe(true);next.nodes[0].acceptance_state='accepted';expect(graphDetailsMeetFloor(next,{summary:before,details:before})).toBe(false);
  });
  it('checks exact Owner action receipt references and never promotes rejection into acceptance',()=>{
    const value={graph_contract_version:1,approval_id:'approval',run_id:'run',node_id:'owner',plan_revision:1,decision:'accepted',owner_decision_id:'decision',run_revision:4,task_accepted:false};expect(graphDecisionShape(value,'run','owner',1,3,'accepted')).toBe(true);expect(graphDecisionShape({...value,run_revision:3},'run','owner',1,3,'accepted')).toBe(false);expect(graphDecisionShape({...value,node_id:'foreign'},'run','owner',1,3,'accepted')).toBe(false);expect(graphDecisionShape({...value,decision:'rejected',task_accepted:true},'run','owner',1,3,'rejected')).toBe(false);
  });
});
it('requires the complete accepted receipt and exact criterion decisions instead of graph status booleans',async()=>{
  const {graphAcceptedFixture}=await import('./goal-graph-test-fixtures');const f=goalGraphFixture(),accepted=graphAcceptedFixture(f);expect(graphDetailsShape(accepted.view,accepted.task,f.scope)).toBe(true);const missing=mutable(accepted.view);missing.task_acceptance=null;expect(graphDetailsShape(missing,accepted.task,f.scope)).toBe(false);const missingOwner=mutable(accepted.view);missingOwner.task_acceptance!.owner_decision_ids=[];expect(graphDetailsShape(missingOwner,accepted.task,f.scope)).toBe(false);const foreign=mutable(accepted.view);foreign.task_acceptance!.input_revision='foreign';expect(graphDetailsShape(foreign,accepted.task,f.scope)).toBe(false);const ahead=mutable(accepted.view);ahead.task_acceptance!.sequence=23;expect(graphDetailsShape(ahead,accepted.task,f.scope)).toBe(false);
});
it.each(['device','inode','bytes','uid','gid','mode','mtime','mtime_nsec','ctime','ctime_nsec'])('rejects unsafe or invalid file pin %s instead of treating rounded JSON as exact evidence',field=>{
  const f=goalGraphFixture(),ref=graphCandidateView(f).candidates[0].artifacts[0];ref.file_pin[field]=Number.MAX_SAFE_INTEGER+1;expect(graphArtifactShape(ref,f.scope)).toBe(false);
});
it('enforces signed seconds, nanosecond and unsigned pin ranges without changing wire values',()=>{
  const f=goalGraphFixture(),ref=graphCandidateView(f).candidates[0].artifacts[0];ref.file_pin.mtime=-1;expect(graphArtifactShape(ref,f.scope)).toBe(true);ref.file_pin.mtime_nsec=-1;expect(graphArtifactShape(ref,f.scope)).toBe(false);ref.file_pin.mtime_nsec=1_000_000_000;expect(graphArtifactShape(ref,f.scope)).toBe(false);ref.file_pin.mtime_nsec=999_999_999;ref.file_pin.uid=4_294_967_296;expect(graphArtifactShape(ref,f.scope)).toBe(false);
});
it.each(['task','owner'])('rejects %s acceptance refs whose complete safe origin differs from the captured candidate',async kind=>{
  const {graphAcceptedFixture}=await import('./goal-graph-test-fixtures');const {graphTaskAcceptanceMatchesTask}=await import('./goal-graph-guards');const f=goalGraphFixture(),accepted=graphAcceptedFixture(f),view=mutable(accepted.view);expect(graphDetailsShape(view,accepted.task,f.scope)).toBe(true);
  if(kind==='task'){view.task_acceptance!.artifact_refs=structuredClone(view.task_acceptance!.artifact_refs);view.task_acceptance!.artifact_refs[0].root.device++;view.task_acceptance!.artifact_refs[0].file_pin.inode=999;}
  else{view.owner_decisions[0].candidate_refs=structuredClone(view.owner_decisions[0].candidate_refs);view.owner_decisions[0].candidate_refs[0].root.inode++;view.owner_decisions[0].candidate_refs[0].file_pin.ctime_nsec=1;}
  expect(graphDetailsShape(view,accepted.task,f.scope)).toBe(false);expect(graphTaskAcceptanceMatchesTask(view.task_acceptance,accepted.task,f.scope,view)).toBe(false);
});
