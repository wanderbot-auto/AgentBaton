import { describe, expect, it } from 'vitest';
import { referenceShape, selectionShape, sequenceInput, sameSelection } from './history-export-types';
const s={version:1 as const,project_id:'p',from_sequence:0,to_sequence:null,tasks:true,messages:false,events:false};
describe('history selection bounds',()=>{
 it('rejects lossy and malformed sequence input',()=>{for(const v of ['-1','1.1','1e3','9007199254740992',' 1','01'])expect(()=>sequenceInput(v)).toThrow();expect(sequenceInput('9007199254740991')).toBe(Number.MAX_SAFE_INTEGER);});
 it('requires categories and matches the whole selection',()=>{expect(selectionShape(s)).toBe(true);expect(selectionShape({...s,tasks:false})).toBe(false);expect(selectionShape({...s,from_sequence:4,to_sequence:3})).toBe(false);expect(sameSelection(s,{...s,messages:true})).toBe(false);});
 it('rejects unsafe references',()=>{expect(referenceShape({preview_id:'../outside',selection_digest:'a'.repeat(64),package_sha256:'a'.repeat(64)})).toBe(false);});
});
