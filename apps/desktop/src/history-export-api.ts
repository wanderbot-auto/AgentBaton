import { request } from './api';
import { previewShape, receiptShape, type HistorySelection, type HistoryReference, type HistoryPreview, type HistoryReceipt } from './history-export-types';
export const historyExportApi={
  async preview(selection:HistorySelection,idempotencyKey:string){const p=await request<HistoryPreview>('history_export_preview',{selection,idempotencyKey});if(!previewShape(p,selection)||p.preview_id!==idempotencyKey)throw new Error('INVALID_EXPORT_RESPONSE');return p;},
  async publish(reference:HistoryReference){const r=await request<HistoryReceipt>('history_export_publish',{reference});if(!receiptShape(r,reference)||r.state!=='ready')throw new Error('INVALID_EXPORT_RESPONSE');return r;},
  async get(reference:HistoryReference){const r=await request<HistoryReceipt>('history_export_get',{reference});if(!receiptShape(r,reference))throw new Error('INVALID_EXPORT_RESPONSE');return r;},
  reveal:(reference:HistoryReference)=>request<{status:string}>('history_export_reveal',{reference}),
};
