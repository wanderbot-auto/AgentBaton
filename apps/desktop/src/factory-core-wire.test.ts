// @vitest-environment node
import { describe,it,expect } from 'vitest';
import raw from '../src-tauri/tests/fixtures/factory-wire-bundle-0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64.json?raw';
import { factoryScopeShape,factoryDetailsShape,factorySelectionSupported } from './factory-guards';
import type { FactoryScope,FactoryRunDetails,FactorySelection } from './factory-types';
// Actual Store finalizer fixture + durable queued run; no Provider or OS dialog.
const wire=JSON.parse(raw) as {scope:FactoryScope;selection:FactorySelection;run_details:FactoryRunDetails;proposal:{snapshot:{bindings:FactoryScope['bindings']}}};
describe('immutable Core-generated Factory wire bundle',()=>{
 it('retains the original UTF8 bytes and validates the current Project/scope/queued-run DTOs',async()=>{const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(raw))),x=>x.toString(16).padStart(2,'0')).join('');expect(digest).toBe('0c6fbd065df4a49997acca4fc898a3f952721bfbc60b57229ca36571db0c0c64');expect(factorySelectionSupported(wire.selection)).toBe(true);expect(factoryScopeShape(wire.scope,wire.scope.bindings.root_task)).toBe(true);expect(factoryDetailsShape(wire.run_details,wire.scope.bindings.root_task,wire.scope.id)).toBe(true);expect(wire.run_details.run.state).toBe('queued');expect(wire.run_details.tasks[0].parent_task_id).toBeNull();});
 it('keeps unsafe server filesystem identity opaque while independently rejecting foreign Project or hosts',()=>{const scope=structuredClone(wire.scope);scope.bindings.project.identity.inode=2**63;expect(factoryScopeShape(scope,scope.bindings.root_task)).toBe(true);scope.bindings.connections[0].host_id='outside';expect(factoryScopeShape(scope,scope.bindings.root_task)).toBe(false);scope.bindings.connections=wire.scope.bindings.connections;scope.bindings.project.id='foreign';expect(factoryScopeShape(scope,scope.bindings.root_task)).toBe(false);});
});
