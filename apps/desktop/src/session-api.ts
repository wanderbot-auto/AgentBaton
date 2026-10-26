import { request } from './api';

export interface NamedConnection {
  id: string;
  name: string;
  provider: string;
  executable: string;
  arguments: string[];
  executable_sha256: string;
  environment_id: string;
  identity_ref: string;
  config_revision: string;
  host_id: string;
  approval_id: string;
  probe_state: 'pending' | 'ready' | 'failed';
  revision: number;
  capability_snapshot_id: string;
  identity_status: string;
  config_status: string;
  created_at: string;
  version_evidence: null | {
    source: string;
    executable_sha256: string;
    observed_at_unix_ms: number;
    state: string;
    version_line: string | null;
    exit_code: number | null;
    stdout_bytes: number;
    stderr_bytes: number;
    process_reaped: boolean;
  };
}
export interface ManagedSession {
  id: string;
  task_id: string;
  project_id: string;
  workspace_id: string;
  connection_id: string;
  grant_id: string;
  state: 'starting' | 'ready' | 'running' | 'cancel_requested' | 'stopped' | 'outcome_unknown' | 'failed';
  revision: number;
  native_session_id: string | null;
  created_at: string;
  updated_at: string;
  last_error: string | null;
  turn_state: string | null;
}
export interface ExecutionGrant {
  id: string;
  project_id: string;
  root_grant_id: string;
  parent_id: string | null;
  depth: number;
  revision: number;
  approval_id: string;
  approved_at: string;
  expires_at_ms: number;
  revoked: boolean;
  scope: { workspace_ids: string[]; connection_ids: string[]; host_ids: string[]; max_attempts: number; max_concurrency: number; max_duration_ms: number; recipients: string[]; network: { mode: string }; max_depth: number; max_children: number };
  enforcement: { baton_mediated_scope: boolean; os_filesystem_sandbox: boolean; os_network_sandbox: boolean; native_identity_isolation: boolean };
}
export interface GrantRequest {
  project_id: string;
  workspace_id: string;
  connection_id: string;
  max_attempts: number;
  max_concurrency: number;
  max_duration_ms: number;
}
export const sessionApi = {
  connectionsList: () => request<NamedConnection[]>('connections_list'),
  connectionsRegister: (name: string, executable: string, arguments_: string[], idempotencyKey: string) => request<NamedConnection>('connections_register', { name, executable, arguments: arguments_, provider: 'custom_acp', idempotencyKey }),
  grantsCreate: (scope: GrantRequest, idempotencyKey: string) => request<ExecutionGrant>('grants_create', { projectId: scope.project_id, workspaceId: scope.workspace_id, connectionId: scope.connection_id, maxAttempts: scope.max_attempts, maxConcurrency: scope.max_concurrency, maxDurationMs: scope.max_duration_ms, idempotencyKey }),
  grantsList: (projectId: string) => request<ExecutionGrant[]>('grants_list', { projectId }),
  grantsRevoke: (grant: ExecutionGrant, idempotencyKey: string) => request<ExecutionGrant>('grants_revoke', { grantId: grant.id, expectedRevision: grant.revision, idempotencyKey }),
  sessionsCreate: (taskId: string, expectedRevision: number, workspaceId: string, connectionId: string, grantId: string, idempotencyKey: string) => request<ManagedSession>('sessions_create', { taskId, expectedRevision, workspaceId, connectionId, grantId, idempotencyKey }),
  sessionsList: () => request<ManagedSession[]>('sessions_list'),
  sessionsGet: (sessionId: string) => request<ManagedSession>('sessions_get', { sessionId }),
  sessionsSend: (session: ManagedSession, text: string, idempotencyKey: string) => request<ManagedSession>('sessions_send', { sessionId: session.id, expectedRevision: session.revision, text, idempotencyKey }),
  sessionsCancel: (session: ManagedSession, idempotencyKey: string) => request<ManagedSession>('sessions_cancel', { sessionId: session.id, expectedRevision: session.revision, idempotencyKey }),
};
