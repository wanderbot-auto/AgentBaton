// @vitest-environment node
import {expect,it} from 'vitest';
import raw from './goal-graph-actual-integrity-fixture.json?raw';
import taskRaw from './goal-graph-actual-integrity-task-fixture.json?raw';
import {graphViewConsistent,graphViewSupported,graphReferencesBound,graphObservationSupported} from './goal-graph-guards';
import type {GraphRunDetails} from './goal-graph-types';
const packet=()=>JSON.parse(raw) as {graph_details:GraphRunDetails};
it.each([{label:'original 7cc70a',value:raw},{label:'actual task packet',value:taskRaw}])('preserves $label Core Integrity receipt and complete observation digest without claiming semantic acceptance',async({value})=>{
  const view=(JSON.parse(value) as {graph_details:GraphRunDetails}).graph_details,receipt=view.validations[0];
  expect(graphViewSupported(view)).toBe(true);
  expect(graphReferencesBound(view)).toBe(true);
  expect(graphViewConsistent(view)).toBe(true);
  expect(receipt.scope).toBe('artifact_integrity_only');
  expect(receipt.correctness).toBe('not_established');
  expect(receipt.task_acceptance).toBe('not_established');
  const encoded=new TextEncoder().encode(JSON.stringify(receipt.observation));
  const digest=await crypto.subtle.digest('SHA-256',encoded);
  expect(Array.from(new Uint8Array(digest),byte=>byte.toString(16).padStart(2,'0')).join('')).toBe(receipt.result_digest);
});
it.each(['missing','null','missing-goal-io','unsafe-integer'])('refuses trusted validator projection with %s complete observation',mode=>{
  const view=packet().graph_details;
  const receipt=view.validations[0] as unknown as Record<string,any>;
  if(mode==='missing')delete receipt.observation;
  if(mode==='null')receipt.observation=null;
  if(mode==='missing-goal-io')delete receipt.observation.goal_io;
  if(mode==='unsafe-integer')receipt.observation.outputs[0].root_inode=Number.MAX_SAFE_INTEGER+1;
  expect(graphViewSupported(view)).toBe(false);
  expect(graphViewConsistent(view)).toBe(false);
});
it('preserves finite fractional JSON observations without pretending they are actual Data validation',()=>{
  const unitOnly={goal_io:{state:'observed'},numeric_measurements:[0.5,1.25,-0.5],unknown_optional_metadata:null};
  expect(graphObservationSupported(unitOnly)).toBe(true);
  expect(graphObservationSupported({...unitOnly,numeric_measurements:[Infinity]})).toBe(false);
});
