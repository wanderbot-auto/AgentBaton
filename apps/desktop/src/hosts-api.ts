import { request } from './api';
import {
  hostId, hostUuid, maxHostPage, parseHostAcknowledgement, parseHostPage, parseHostPending,
  parseHostReconciliation, parseHostRegistration, parseHostSubmission, parseHostView, positiveHostInteger,
  type HostIntent, type HostReceipt, type HostRegistration,
} from './hosts-types';

export const hostsApi = {
  async register(registration: HostRegistration, idempotencyKey: string) {
    const input = parseHostRegistration(registration);
    return parseHostSubmission(await request<unknown>('hosts_register', {
      registration: input, idempotencyKey: hostUuid(idempotencyKey),
    }), input);
  },
  async list(after: number | null = null, limit = maxHostPage) {
    const cursor = after === null ? null : positiveHostInteger(after);
    const size = positiveHostInteger(limit, maxHostPage);
    return parseHostPage(await request<unknown>('hosts_list', {
      request: { limit: size, after_created_sequence: cursor },
    }), cursor, size);
  },
  async get(id: string) {
    const selected = hostId(id);
    return parseHostView(await request<unknown>('hosts_get', { hostId: selected }), selected);
  },
  async pending() {
    return parseHostPending(await request<unknown>('hosts_registration_pending', {}));
  },
  async reconcile(intent: HostIntent) {
    return parseHostReconciliation(await request<unknown>('hosts_registration_reconcile', {
      entryId: hostUuid(intent.entry_id),
    }), intent);
  },
  async retry(intent: HostIntent) {
    const entry = hostUuid(intent.entry_id);
    return parseHostSubmission(await request<unknown>('hosts_registration_retry', {
      entryId: entry,
    }), intent.registration, entry);
  },
  async acknowledge(entry: string, receipt: HostReceipt) {
    const entryId = hostUuid(entry);
    return parseHostAcknowledgement(await request<unknown>('hosts_registration_acknowledge', {
      entryId,
      hostId: hostId(receipt.host_id),
      createdSequence: positiveHostInteger(receipt.created_sequence),
    }), entryId);
  },
};
