# Provider adapter protocol evidence

Research recorded at 2026-10-26T17:49:44.018710+00:00. Repository starting point: `4e1efd378e03cbfeddf11351299da88e911df85c`; workstream `codex/session-supervision`. Authority: AB-START-001.

This is source/documentation inspection for the next implementation branch. No vendor executable, model request, login/authentication command or credential file was accessed. No native compatibility, authentication, runtime enforcement or platform success is claimed. The immutable GitHub revisions below are source snapshots, not assertions that a released installed binary matches them. Qoder web pages have no immutable version URL; their hashes identify the exact observed document bytes.

## Authoritative source ledger

| ID | Immutable source / observed web page | SHA-256 |
| --- | --- | --- |
| `native-schema` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server-protocol/schema/json/codex_app_server_protocol.schemas.json) | `f50e158c34ac55c5cc67c5493df539f1c07b603bd5dd2100a7393eefb62a3e20` |
| `native-server` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/src/lib.rs) | `314e9b2fe129e053b8c7cf261c366a19f9df681f9bc2c7288761d695152fad2f` |
| `native-transport` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/src/transport.rs) | `5b4cb5599deae1af2153976fee7586be3f87df53a2e2816f2011bddf638bda06` |
| `claude-agent` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/src/acp-agent.ts) | `e7f0e6d6783c33dbf7c8b5b2a6b8bfd35a0df4909a1667e716a01fbfab320fcd` |
| `claude-paths` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/src/paths.ts) | `6a6f9c9f89b034bd10aa981ec957c39d72b87165214903ca42389ee3bbb1f8d5` |
| `claude-settings` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/src/settings.ts) | `a9bce3449644f12c1b3258d38fd2ca93ee95534a3d7e043581bc3abca56b3fe1` |
| `claude-auth` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/src/auth-status.ts) | `9c19935f206795fbd52ac790ade786fda0ecca5af62dac8e2fca626fcf98b1e8` |
| `native-cli` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/cli/src/main.rs) | `ac51c153916132c0be2c37ea1136562685b832957bc2e43e53d71608d0b28a4a` |
| `native-stdio` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server-transport/src/lib.rs) | `7aa7f940a38d02e96bcd1bcf21ae37e67dfb3cffa762e1d55dbca22306e463b0` |
| `native-config` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/core/src/config/mod.rs) | `8ea0ac0bbd62e4961401fd45d5be59758c5995d52f94bb185cd117806b6e5b28` |
| `native-stdio-impl` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server-transport/src/transport/stdio.rs) | `e4f83304498aec2e9ccad9ad3a0af66965ab93508a4a6923f993731adcd69c19` |
| `acp-v1-schema` | [source](https://raw.githubusercontent.com/agentclientprotocol/agent-client-protocol/6bf1e6dda543af80c1b993f3a3eeb392c58a6c63/schema/v1/schema.json) | `9da900d14f5a6764b97be01b28940c3e90e2b2574a4bf8f3a87ec9cd0d08a547` |
| `codex-acp-readme` | [source](https://raw.githubusercontent.com/agentclientprotocol/codex-acp/6cce03545eaaf5daef849aa9647d4ccfdc22eb41/README.md) | `4dcbade129cbe32acfff276b7147a0b365ea894c57479fdf58e75ba3a29dd87d` |
| `claude-readme` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/README.md) | `ad3e8086954af6a1ce005ffe7cc59d1775188207c84b148455c735ac46d174c9` |
| `claude-entrypoint` | [source](https://raw.githubusercontent.com/agentclientprotocol/claude-agent-acp/782e463ff418a6093e09cef3f3f5ce813f4ee270/src/index.ts) | `8798fd99816e9a4d0c4a408de4206736cfe297ff7a2668e6f287c08c571daa98` |
| `qoder-acp` | [source](https://docs.qoder.com/cli/acp) | `89e38ae7d8b492e38a41c943cdb9d36dde626269bf67bed2b253798da98c5947` |
| `qoder-cli-reference` | [source](https://docs.qoder.com/cli/cli-reference) | `9bad1965f7ec25acf0d3a3004a034fe53544632ec3bdee73c98a04f305a48486` |
| `native-readme` | [source](https://raw.githubusercontent.com/openai/codex/970111f6cb62cc02fe36a82ea7d71b37a2889c97/codex-rs/app-server/README.md) | `b4a92cfab25bd7b96a16b8f9503a8aeae71fad9bad786ee0662efb2cddd1aeb6` |

The initial guessed native stdio source path `src/transport.rs` returned 404. The implementation was then located through the pinned repository tree at `src/transport/stdio.rs` (`native-stdio-impl`). This is a corrected source lookup, not a runtime failure.

## Codex native App Server: a separate wire dialect

Launch the explicitly selected and digest-verified Codex executable with parameterized argv `app-server --listen stdio://`. The pinned CLI also exposes `--stdio` as an alternative; do not combine it with `--listen`. The CLI documents stdio as default, but explicit selection avoids ambient transport assumptions. Do not enable remote control, a shared daemon, Unix listeners or WebSocket listeners as a side effect of desktop connection.

`native-stdio-impl` reads newline-delimited UTF-8 JSON on stdin. Its output transport serializes records and explicitly appends a newline before writing stdout. The generated native schema has request `{id,method,params?,trace?}`, response `{id,result}` and error `{id,error:{code,message,data?}}`. It does **not require a `jsonrpc` field**. It also has no ACP `protocolVersion` negotiation. Do not run the existing strict ACP envelope validator over native messages, or treat app-server thread APIs as ACP session APIs. Preserve unknown native fields/extensions as opaque evidence, without dispatching them as authority.

Initialize first, correlate its response, then send the `initialized` notification:

```json
{"id":1,"method":"initialize","params":{"clientInfo":{"name":"agentbaton","title":"AgentBaton","version":"<build-version>"},"capabilities":{"experimentalApi":false}}}
{"id":1,"result":{"codexHome":"<effective-home>","platformFamily":"unix","platformOs":"<observed-os>","userAgent":"<observed-agent>"}}
{"method":"initialized"}
```

`InitializeParams.clientInfo` is required; its `name` and `version` are required, `title` is optional/nullable. `InitializeCapabilities` currently contains `experimentalApi`, `explicitGatewayOauth`, `extensions`, `mcpServerOpenaiFormElicitation`, `optOutNotificationMethods`, `requestAttestation`; absence of an ACP-like global capability manifest is not proof all later methods exist. Do not opt into experimental APIs, custom gateways, attestation or auth flows merely because the source defines them.

### Exact schema pointers and field sets

Pointers below resolve within the complete hashed `native-schema` artifact, using JSON Pointer (`N` = `#/definitions`, `V` = `#/definitions/v2`). These are complete top-level property sets for the selected messages; referenced nested schemas remain authoritative in that artifact. They are not invented Baton request contracts.

| Schema pointer | Required fields | Complete declared top-level properties |
| --- | --- | --- |
| `V/ThreadStartParams` | (none) | `approvalPolicy`, `approvalsReviewer`, `baseInstructions`, `config`, `cwd`, `developerInstructions`, `ephemeral`, `model`, `modelProvider`, `personality`, `sandbox`, `serviceName`, `serviceTier`, `sessionStartSource`, `threadSource` |
| `V/ThreadStartResponse` | `approvalPolicy`, `approvalsReviewer`, `cwd`, `model`, `modelProvider`, `sandbox`, `thread` | `approvalPolicy`, `approvalsReviewer`, `cwd`, `disabledPluginIds`, `instructionSources`, `model`, `modelProvider`, `reasoningEffort`, `sandbox`, `serviceTier`, `thread` |
| `V/ThreadResumeParams` | `threadId` | `approvalPolicy`, `approvalsReviewer`, `baseInstructions`, `config`, `cwd`, `developerInstructions`, `excludeTurns`, `model`, `modelProvider`, `personality`, `sandbox`, `serviceTier`, `threadId` |
| `V/ThreadResumeResponse` | `approvalPolicy`, `approvalsReviewer`, `cwd`, `model`, `modelProvider`, `sandbox`, `thread` | `approvalPolicy`, `approvalsReviewer`, `collaborationMode`, `cwd`, `disabledPluginIds`, `instructionSources`, `itemsBackwardsCursor`, `model`, `modelProvider`, `reasoningEffort`, `sandbox`, `serviceTier`, `thread`, `turnsBackwardsCursor` |
| `V/TurnStartParams` | `input`, `threadId` | `approvalPolicy`, `approvalsReviewer`, `clientUserMessageId`, `cwd`, `disabledPluginIds`, `effort`, `input`, `model`, `outputSchema`, `parentTurnId`, `personality`, `rootTurnId`, `sandboxPolicy`, `serviceTier`, `serviceTierForTurn`, `summary`, `threadId`, `toolOutput`, `turnTrigger` |
| `V/TurnStartResponse` | `turn` | `turn` |
| `V/TurnInterruptParams` | `threadId`, `turnId` | `threadId`, `turnId` |
| `V/TurnInterruptResponse` | (none) | (none) |
| `V/Turn` | `id`, `items`, `status` | `completedAt`, `durationMs`, `error`, `id`, `items`, `itemsView`, `rootTurnId`, `startedAt`, `status` |
| `V/AgentMessageDeltaNotification` | `delta`, `itemId`, `threadId`, `turnId` | `delta`, `itemId`, `threadId`, `turnId` |
| `V/TurnStartedNotification` | `threadId`, `turn` | `threadId`, `turn` |
| `V/TurnCompletedNotification` | `threadId`, `turn` | `threadId`, `turn` |
| `V/ItemStartedNotification` | `item`, `startedAtMs`, `threadId`, `turnId` | `item`, `startedAtMs`, `threadId`, `turnId` |
| `V/ItemCompletedNotification` | `completedAtMs`, `item`, `threadId`, `turnId` | `completedAtMs`, `item`, `threadId`, `turnId` |
| `V/CommandExecutionOutputDeltaNotification` | `delta`, `itemId`, `threadId`, `turnId` | `delta`, `itemId`, `threadId`, `turnId` |
| `N/CommandExecutionRequestApprovalParams` | `itemId`, `startedAtMs`, `threadId`, `turnId` | `approvalId`, `command`, `commandActions`, `cwd`, `environmentId`, `itemId`, `kind`, `networkApprovalContext`, `proposedExecpolicyAmendment`, `proposedNetworkPolicyAmendments`, `reason`, `startedAtMs`, `threadId`, `turnId` |
| `N/CommandExecutionRequestApprovalResponse` | `decision` | `decision` |
| `N/FileChangeRequestApprovalParams` | `itemId`, `startedAtMs`, `threadId`, `turnId` | `grantRoot`, `itemId`, `reason`, `startedAtMs`, `threadId`, `turnId` |
| `N/FileChangeRequestApprovalResponse` | `decision` | `decision` |
| `N/PermissionsRequestApprovalParams` | `cwd`, `itemId`, `permissions`, `startedAtMs`, `threadId`, `turnId` | `cwd`, `environmentId`, `itemId`, `permissions`, `reason`, `startedAtMs`, `threadId`, `turnId` |
| `N/PermissionsRequestApprovalResponse` | `permissions` | `permissions`, `scope`, `strictAutoReview` |

Selected exact type projections below omit only prose metadata (`description`, `title`, `$schema`); constraints, required fields, wire names and references are retained. All referenced types are in the source ledger schema.

`V/TurnInterruptParams`:

```json
{"properties":{"threadId":{"type":"string"},"turnId":{"type":"string"}},"required":["threadId","turnId"],"type":"object"}
```

`V/AgentMessageDeltaNotification`:

```json
{"properties":{"delta":{"type":"string"},"itemId":{"type":"string"},"threadId":{"type":"string"},"turnId":{"type":"string"}},"required":["delta","itemId","threadId","turnId"],"type":"object"}
```

`V/TurnCompletedNotification`:

```json
{"properties":{"threadId":{"type":"string"},"turn":{"$ref":"#/definitions/v2/Turn"}},"required":["threadId","turn"],"type":"object"}
```

`V/TurnStatus`:

```json
{"enum":["completed","interrupted","failed","inProgress"],"type":"string"}
```

`V/UserInput`:

```json
{"oneOf":[{"properties":{"text":{"type":"string"},"text_elements":{"default":[],"items":{"$ref":"#/definitions/v2/TextElement"},"type":"array"},"type":{"enum":["text"],"type":"string"}},"required":["text","type"],"type":"object"},{"anyOf":[{"properties":{"url":{"type":"string"}},"required":["url"],"type":"object"},{"properties":{"fileId":{"type":"string"}},"required":["fileId"],"type":"object"}],"properties":{"detail":{"anyOf":[{"$ref":"#/definitions/v2/ImageDetail"},{"type":"null"}],"default":null},"type":{"enum":["image"],"type":"string"}},"required":["type"],"type":"object"},{"properties":{"detail":{"anyOf":[{"$ref":"#/definitions/v2/ImageDetail"},{"type":"null"}],"default":null},"path":{"type":"string"},"type":{"enum":["localImage"],"type":"string"}},"required":["path","type"],"type":"object"},{"properties":{"type":{"enum":["audio"],"type":"string"},"url":{"type":"string"}},"required":["type","url"],"type":"object"},{"properties":{"path":{"type":"string"},"type":{"enum":["localAudio"],"type":"string"}},"required":["path","type"],"type":"object"},{"properties":{"name":{"type":"string"},"path":{"type":"string"},"type":{"enum":["skill"],"type":"string"}},"required":["name","path","type"],"type":"object"},{"properties":{"name":{"type":"string"},"path":{"type":"string"},"type":{"enum":["mention"],"type":"string"}},"required":["name","path","type"],"type":"object"}]}
```

### Native operations, streaming and approval semantics

| Operation | Exact native call / observation | Baton mapping and boundary |
| --- | --- | --- |
| Create | `thread/start` with explicit `cwd`, `approvalPolicy`, `sandbox`, resolved config/model fields where approved. Response supplies `thread.id` and effective `cwd`, model/provider, sandbox and approval settings. | Pin returned effective settings against the Connection/Attempt snapshot. A response is not proof the requested policy was enforced. Omitted/null config fields inherit native behavior. |
| Prompt | `turn/start` requires `threadId` and `input:[UserInput]`. Plain text is `{type:"text",text:"…",text_elements:[]}`; `text_elements` defaults to `[]`. | Its response contains a `turn` with native `id` and status; submission acceptance is not completion. Do not confuse the camelCase RPC object with UserInput's exact snake_case `text_elements`. |
| Stream | `turn/started`, `item/started`, `item/agentMessage/delta`, `item/commandExecution/outputDelta`, `item/completed`, `turn/completed`; exact schema pointers above. | Bind every delta to its native thread/turn/item and trusted local attempt. A delta is not an accepted artifact; keep source and sequence/gap evidence. |
| Cancel | `turn/interrupt` requires **both** `threadId` and `turnId`; response is an empty object schema. | Response acknowledges the interrupt RPC. Observe correlated `turn/completed` with `turn.status="interrupted"` before native turn-stop confirmation. Disconnect/timeouts remain unknown. Owned process termination is a different stop-evidence source. |
| Resume | `thread/resume` requires `threadId`. Optional `excludeTurns` can omit history from the response; config/cwd/model/approval overrides exist. | Verify stored account/installation/config/workspace/input binding first. No silent account or model-provider replacement; revalidate returned effective settings. A saved thread ID does not prove a live old process is controllable. |

Native `TurnStatus` is exactly `completed | interrupted | failed | inProgress` in this pinned schema. `completed` ends a native turn; it does not accept the Baton task. An `error` notification or RPC error must not imply no side effects. The event state is separate from artifact validation, acceptance and integration.

Native `AskForApproval` is `untrusted | on-request | never`, or `{granular:{mcp_elicitations:bool,rules:bool,sandbox_approval:bool,request_permissions?:bool,skill_approval?:bool}}` with the first three granular fields required. `SandboxMode` is `read-only | workspace-write | danger-full-access`. The richer `SandboxPolicy` used by turn requests has variants `readOnly`, `workspaceWrite`, `externalSandbox`, `dangerFullAccess`, with different fields. These are native settings, not an automatic OS-sandbox guarantee on every host. Do not map a Baton permission grant to `never` or full access to suppress approvals.

Server-initiated requests have their own RPC IDs:

- `item/commandExecution/requestApproval`: response `{decision:...}`. Native decisions are `accept`, `acceptForSession`, `decline`, `cancel`, or the distinct objects `{acceptWithExecpolicyAmendment:{execpolicy_amendment:[string]}}` / `{applyNetworkPolicyAmendment:{network_policy_amendment:...}}`. `decline` permits the agent to continue; `cancel` also requests interrupt. Persistent/session amendments require matching explicit authority; they are not equivalent to one-time acceptance. Preserve `approvalId` when supplied: multiple callbacks may share an `itemId`.
- `item/fileChange/requestApproval`: response `{decision:"accept"|"acceptForSession"|"decline"|"cancel"}`. `grantRoot` is explicitly unstable and its honoring is unclear in the pinned schema; it cannot be used to claim a filesystem boundary.
- `item/permissions/requestApproval`: response `{permissions:GrantedPermissionProfile,scope?:"turn"|"session",strictAutoReview?:bool|null}`. `permissions` is required; `scope` defaults to `turn`; the grant profile has optional `fileSystem`/`network`. This is a different authority-expansion request, not a generic command approval. Deny/unavailable policy must not manufacture a broad grant.
- Also defined: `item/tool/requestUserInput`, `mcpServer/elicitation/request`, `item/tool/call`, token refresh and attestation requests. Do not advertise or silently implement these through a generic shell, credential reader or arbitrary UI RPC. Unknown requests fail closed with correlated errors and preserved opaque evidence.

### Native configuration and identity

`native-config` defines `CODEX_HOME`, defaulting to `~/.codex`; an explicit value must exist and is canonicalized. Managed mode must deliberately materialize its own namespace; native-connected mode must bind an explicitly approved external source without writing it. Native defaults may include repository/organization settings and native secret stores; private `HOME` alone does not exclude them.

The pinned native README describes account/workspace-routing discovery at startup and notifications after initialization; launching/handshaking a real server can read native account state or contact routing services. It is not an authorization-free smoke test.

The source distinguishes file/keyring/auto CLI auth credential stores, and file/keyring/auto MCP OAuth stores. It defines `forced_login_method` and `forced_chatgpt_workspace_id` restrictions, not a generic arbitrary account-alias selector. Do not invent CLI flags for account switching, copy `auth.json`, or use a keyring identity from another named connection. Authentication readiness needs a consented native operation and trusted local identity mapping; it is not established by `initialize.userAgent` or self-reported Provider names. Existing native App Server must remain the same runtime for capability/account queries; launching another diagnostic server cannot be labeled session-runtime evidence.

## ACP v1 contract used by Claude adapter, Qoder and custom ACP

The pinned `acp-v1-schema` is authoritative for the wire version **1**, distinct from its crate/artifact version. ACP messages use newline-delimited JSON-RPC 2.0, including `jsonrpc:"2.0"`. This is the existing ACP dialect; it is not the Codex native dialect above.

`A` below means `#/$defs` in the hashed ACP schema. `_meta` is optional/nullable metadata; unknown extension contents remain opaque and never grant authority.

| Method / schema pointer | Exact required fields | Optional fields / result shape |
| --- | --- | --- |
| `initialize` / `A/InitializeRequest` | `protocolVersion` (use integer `1`) | `clientCapabilities`, `clientInfo`, `_meta`; result `A/InitializeResponse` requires `protocolVersion`, optionally `agentCapabilities`, `authMethods`, `agentInfo`, `_meta`. |
| `session/new` / `A/NewSessionRequest` | `cwd` absolute string, `mcpServers` array | `additionalDirectories`, `_meta`; response `A/NewSessionResponse` requires `sessionId`, optionally `modes`, `configOptions`, `_meta`. |
| `session/prompt` / `A/PromptRequest` | `sessionId`, `prompt:[ContentBlock]` | `_meta`; `A/PromptResponse` requires `stopReason`, optional `_meta`. Text is `{type:"text",text:"…"}`. Image/audio/embedded context require the corresponding negotiated prompt capability. |
| `session/update` notification / `A/SessionNotification` | `sessionId`, `update:SessionUpdate` | `_meta`; `update.sessionUpdate` discriminates content, tools, plan and other update variants. |
| `session/request_permission` server request / `A/RequestPermissionRequest` | `sessionId`, `toolCall:ToolCallUpdate`, `options:[PermissionOption]` | `_meta`; result `A/RequestPermissionResponse` requires `outcome`. |
| `session/cancel` notification / `A/CancelNotification` | `sessionId` | `_meta`; **no request ID or direct acknowledgement response**. Correlate the pending prompt response. |
| `session/load` / `A/LoadSessionRequest` | `sessionId`, `cwd`, `mcpServers` | `additionalDirectories`, `_meta`; result `A/LoadSessionResponse` has optional `modes`, `configOptions`, `_meta`, no new required ID. Requires negotiated `loadSession=true`; history is replayed through updates. |
| `session/resume` / `A/ResumeSessionRequest` | `sessionId`, `cwd` | `additionalDirectories`, optional `mcpServers`, `_meta`; result `A/ResumeSessionResponse` has optional `modes`, `configOptions`, `_meta`. Requires advertised non-null `sessionCapabilities.resume`; resumes **without** previous-message replay. Do not equate it with load. |

Initialization example (host filesystem, terminal and terminal-auth methods are deliberately unimplemented):

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,"clientInfo":{"name":"AgentBaton","version":"<build-version>"},"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false,"auth":{"terminal":false}}}}
```

ACP permission option fields are `optionId`, `name`, `kind`; `kind` is `allow_once | allow_always | reject_once | reject_always`. A selected reply is exactly `{outcome:{outcome:"selected",optionId:"<offered-id>"}}`; cancellation is `{outcome:{outcome:"cancelled"}}`, wrapped in the correlated JSON-RPC result. Do not translate an untrusted tool title or option label into policy authority. Match the offered ID, action scope, actor and lifetime against the trusted grant; unknown or unrepresentable semantics block rather than broadening permissions. Sending cancel requires answering pending permission requests as cancelled.

ACP stop reasons are `end_turn | max_tokens | max_turn_requests | refusal | cancelled`. Sending `session/cancel` only records `cancel_requested`; correlated prompt `stopReason="cancelled"` confirms the native turn cancellation. `end_turn` after a cancellation race is completed, not falsely cancelled. Acknowledgements, transport EOF, and OS process termination are separate evidence classes.

Known update shapes include `agent_message_chunk` and `agent_thought_chunk` with `content:{type:"text",text:string}`, and `tool_call`/`tool_call_update` with `toolCallId`; initial tool calls require `title`. The complete pinned `SessionUpdate` also has user messages, plan, available commands, modes, config options, session metadata, usage, notices and compaction variants. Unknown content or update extensions must retain bounded raw evidence and downgrade to opaque UI detail rather than being executed or silently treated as understood.

## Claude Code: choose the official ACP adapter path explicitly

The inspected route is `@agentclientprotocol/claude-agent-acp`, around Claude Agent SDK. It is **not** a hypothetical `claude --acp` flag. The pinned entrypoint handles `--version` before normal SDK startup and `--cli` separately delegates flags to the wrapped native CLI. Launch the exact selected adapter executable with its explicit ACP argv; do not use `--cli` on the ACP route. Do not auto-install a preview through `npx` or treat adapter/package/underlying SDK/native CLI versions as interchangeable. `CLAUDE_CODE_EXECUTABLE` can select the underlying CLI in the inspected implementation; both the adapter and underlying executable/dependency provenance must be pinned and tested.

Source observations, **not live compatibility**:

- `acp-agent.ts::initialize` returns wire version 1, text/image/embedded-context support, HTTP/SSE MCP support, `loadSession:true`, and session capabilities including resume/list/close/additional directories. Extra capabilities/metadata also exist, including providers, steering, auth status and AIR extensions. The client still must negotiate the actual running binary and preserve unsupported extensions as opaque.
- **Initialize itself starts `probeCliAuthStatus()` in the background**, then publishes `_auth/status_update`; it does not wait for auth or place identity in initialize's response. An initialize smoke test is therefore not necessarily free of private-account access. Real adapter startup/initialization needs the separately approved identity test context. This research did not execute it.
- `newSession` calls `createSession`; `loadSession` can replay history before its response; `resumeSession` calls `getOrCreateSession` without the same history replay. The implementation rejects non-absolute/nonexistent cwd. Its SDK `query()` starts the native CLI during session creation, not only at first prompt.
- Default SDK `settingSources` is `["user","project","local"]`; `_meta.claudeCode.options` is filtered through an explicit whitelist, with `settingSources`, `env`, settings and selected other options. Some fields are adapter-owned and rejected/ignored (e.g. arbitrary callback objects, cwd and permissionMode). The code later sets the actual cwd, MCP merge, permission mode and `canUseTool` itself. Do not blindly forward renderer `_meta` or assume every SDK option passes through.
- `paths.ts` resolves `CLAUDE_CONFIG_DIR`, else `~/.claude`. This scopes a native configuration directory; it does not prove all organization policies, project files, CLI keychain state or billing selectors are excluded. Managed mode needs tested materialization and explicit source manifests; native-connected mode remains read-only externally. No native auth directories may be copied or symlinked to create apparent account isolation.
- Cancellation uses the SDK interrupt path; the pinned README/source explicitly distinguish foreground subagents, background subagents, workflows and background commands. Certain declared subagent capabilities spare background children from parent cancellation; background commands can survive an interrupt. A prompt `cancelled` result is not a proof every spawned background process stopped. Record the native cancellation scope and reject requirements whose enforcement is unavailable.
- Browser/terminal login and gateway methods are capability-dependent. Do not advertise terminal auth or opt into custom gateways to work around unavailable credentials. No fallback identity, API key, endpoint or billing mode is authorized by protocol support.

## Qoder: documented launch, negotiated runtime capabilities

The official observed pages document:

- `qoder --acp` exposes ACP over standard input/output.
- `qoder --version` / `-v` prints version and exits.
- `--config-dir <directory>` overrides the user configuration directory (default `~/.qoder`); `--cwd <directory>` chooses the working directory.
- General CLI history flags include `--resume`, `--session-id`, `--continue`; these are **not substitutes** for an ACP `session/load` or `session/resume` capability/contract. Do not combine interactive CLI continuation with a managed ACP attempt.
- ACP authentication reuses native login or the optional `QODER_PERSONAL_ACCESS_TOKEN` mechanism. No real login, token, authentication path or native state was touched here.
- Permission flags include modes and dangerous bypass flags. These are not proof ACP host-side enforcement exists; never add bypass/yolo flags to make a test pass.

Candidate managed argv is an explicitly selected Qoder executable with `--acp` and approved config/cwd selectors; the documented combination, actual config inheritance and credential-store binding remain **unrun** until a pinned binary fixture/native integration confirms them. The pages do not publish a per-version complete Qoder initialize response or proprietary stream schema. Consequently Qoder load/resume, prompt types, tool/approval mappings, auth identity and feature support remain `unknown/untested` until actual negotiated runtime evidence. Implement the common ACP dialect with strict capability checks; do not hardcode the Claude adapter's capability response into Qoder.

## Proposed shared supervisor interface for immediate implementation

Reuse the owned-process pump, monotonic operation/turn deadlines, nonblocking outbound queue, readiness polling, bounded raw/typed evidence and stop-identity checks. Keep **different codecs and native correlation/state machines** for ACP and native Codex. Do not route native messages through ACP `AcpClient`, and do not add another business daemon or private task state machine.

The following is a proposed internal Rust interface, not an existing vendor SDK API:

```rust
trait SupervisedProviderDriver: Send {
    fn evidence(&self) -> &NegotiatedSupport;
    fn open_managed(&mut self, binding: &VerifiedExecutionBinding)
        -> Result<NativeSessionRef, ProviderFailure>;
    fn begin_turn(&mut self, session: &NativeSessionRef, turn: &BoundTurnInput)
        -> Result<NativeTurnRef, ProviderFailure>;
    fn poll(&mut self, quantum: Duration)
        -> Result<Vec<ProviderObservation>, ProviderFailure>;
    fn answer_approval(&mut self, key: &NativeApprovalKey, decision: &VerifiedDecision)
        -> Result<DecisionDelivery, ProviderFailure>;
    fn request_cancel(&mut self, turn: &NativeTurnRef)
        -> Result<CancelDelivery, ProviderFailure>;
    fn resume_managed(&mut self, recovery: &VerifiedRecoveryBinding)
        -> Result<NativeSessionRef, ProviderFailure>;
    fn terminate_owned(&mut self) -> Result<OwnedStopEvidence, ProviderFailure>;
}
```

- Constructors take a server-validated launch grant and exact binary/argv/environment/config/identity/host snapshot; no deserialization or raw string path upgrades a renderer request into `VerifiedExecutionBinding` authority.
- `NativeSessionRef` distinguishes `Acp {session_id}` from `Codex {thread_id}`. `NativeTurnRef` distinguishes ACP's pending prompt request from Codex's native thread/turn IDs. `NativeApprovalKey` retains protocol, request ID and native session/thread/turn/item/approval IDs as applicable; stale callbacks cannot be answered on a replacement runtime.
- Observations distinguish turn submission, text/tool updates, permission requests, cancel delivery, native confirmed turn stop, owned process stop, transport failure, output gaps and unknown effects. They carry source/version/binding evidence and bounded payload references. Completion never implies task acceptance or permission enforcement.
- Keep `supported/unsupported/unknown/untested`, evidence source, declared/requested/effective/enforced settings and verification dimension separate. Documentation and fixture success cannot satisfy a live-provider support requirement.
- Replay during `session/load` can precede its response. A future recovery codec needs a **provisional replay-only binding** derived from an already trusted stored attempt, with no prompt/approval/cancel authority until native load/resume is confirmed. The current invalidated session ID must not simply become attached after reinitialize.
- Preserve intent/outbox state before sending; no automatic retry on submission timeout, unknown model charge or dropped connection. Resume only the approved account/host/config/workspace/input revision. An unsupported resume creates an explicit new attempt with selected handoff, not a fabricated restored session.
- Native account introspection, vendor startup, real prompts, remote hosts and signature operations remain separately gated. Code/fixture implementation can proceed immediately; docs do not authorize private identities or additional billing.

Next implementation order: extract/reuse the transport pump without changing its tested cancellation/backpressure semantics; add a native Codex codec with pinned schema fixtures (handshake, thread/turn, approvals, interrupt, resume); add adapter-specific configuration/identity handling and actual ACP capability fixtures; wire each explicit connection route into the existing supervisor. Keep live vendors blocked until the approved identity context and matching binary/runtime versions are available. This research is preparation for that code, not a completion claim for WP04.
