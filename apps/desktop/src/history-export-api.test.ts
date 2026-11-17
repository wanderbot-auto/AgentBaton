import { beforeEach, expect, it, vi } from 'vitest';
import { request } from './api';
import { historyExportApi } from './history-export-api';
vi.mock('./api',()=>({request:vi.fn()}));
const selection={version:1 as const,project_id:'p',from_sequence:0,to_sequence:null,tasks:true,messages:false,events:false};
beforeEach(()=>vi.clearAllMocks());
it('forwards only selection and rejects a mismatched preview',async()=>{
 vi.mocked(request).mockResolvedValue({});await expect(historyExportApi.preview(selection)).rejects.toThrow('INVALID_EXPORT_RESPONSE');
 expect(request).toHaveBeenCalledWith('history_export_preview',{selection});
});
it('forwards only fixed reference for native retrieval',async()=>{
 const reference={preview_id:'469fcb86-cd29-4c49-b647-99f884d12f87',selection_digest:'a'.repeat(64),package_sha256:'b'.repeat(64)};
 vi.mocked(request).mockResolvedValue({status:'requested'});await historyExportApi.reveal(reference);
 expect(request).toHaveBeenCalledWith('history_export_reveal',{reference});
 vi.mocked(request).mockResolvedValue({state:'ready',filename:'/arbitrary'});await expect(historyExportApi.get(reference)).rejects.toThrow('INVALID_EXPORT_RESPONSE');
});
