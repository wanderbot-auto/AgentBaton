import { expect,it } from 'vitest';
import raw from './goal-graph-actual-accepted-fixture.json?raw';
import { taskHasAcceptanceProjection,type Task } from './api';
import { graphSummaryShape,graphTaskAcceptanceMatchesTask,graphViewConsistent,graphArtifactPrecision } from './goal-graph-guards';
import type { GraphRunDetails } from './goal-graph-types';
it('parses the immutable actual Core Owner-only accepted packet and binds the trusted Task receipt exactly',()=>{
  // Actual Core50a Owner-only Work/stop/capture/Owner result, SHA da00c1ca...
  // These are static renderer checks against its serialized data. This UI
  // branch does not run that Core, a Provider, OS dialog or Native finalizer.
  const packet=JSON.parse(raw) as {task:Task;graph_details:GraphRunDetails;final_owner_proposal:{id:string;snapshot:{run_binding:{scope_id:string;root_grant_id:string;input_revision:string;workspace_id:string}}};final_owner_decision:{approval_id:string}};
  const {task,graph_details:view}=packet,binding=packet.final_owner_proposal.snapshot.run_binding;
  const scopeReferences={id:binding.scope_id,root_grant_id:binding.root_grant_id,input_revision:binding.input_revision,workspace_id:binding.workspace_id};
  expect(taskHasAcceptanceProjection(task)).toBe(true);expect(graphSummaryShape(view,task)).toBe(true);expect(graphViewConsistent(view)).toBe(true);expect(view.candidates.flatMap(c=>c.artifacts).every(graphArtifactPrecision)).toBe(true);expect(graphTaskAcceptanceMatchesTask(view.task_acceptance,task,scopeReferences,view)).toBe(true);expect(packet.final_owner_decision.approval_id).toBe(packet.final_owner_proposal.id);
  expect(graphTaskAcceptanceMatchesTask(view.task_acceptance,{...task,acceptance_sequence:task.acceptance_sequence!+1},scopeReferences,view)).toBe(false);
});
