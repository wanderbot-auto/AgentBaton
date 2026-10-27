# Provider protocol and owned-process layer

This crate implements a bounded protocol-v1 client state machine and pure wire
mapping, plus an owned Unix stdio subprocess transport. It accesses no credentials
and claims no live Provider compatibility. The daemon connects the ACP layer to its persisted session supervisor; native-route integration is a separate higher-level checkpoint.

Reference checked: the official
[`schema/v1/schema.json`](https://github.com/agentclientprotocol/agent-client-protocol/blob/6bf1e6dda543af80c1b993f3a3eeb392c58a6c63/schema/v1/schema.json)
at commit `6bf1e6dda543af80c1b993f3a3eeb392c58a6c63`, SHA-256
`9da900d14f5a6764b97be01b28940c3e90e2b2574a4bf8f3a87ec9cd0d08a547`.
These are checked reference facts, not a claim of compatibility with every SDK
release or Provider version. No SDK dependency is adopted in this increment.

- Only protocol version 1 becomes ready. Loading requires advertised
  `loadSession=true`. Filesystem, terminal and terminal-auth capabilities remain
  false because no controlled host implementation is present.
- Newline JSON-RPC input is bounded to 1 MiB. Transport owners must set read
  deadlines and close failed transports; they must not keep reading a partially
  rejected oversized message. Outgoing prompts are conservatively limited to
  one eighth of that bound, leaving room for JSON escaping and the envelope.
- Local Attempt/Connection associations are supplied by trusted transport code;
  Provider-supplied identity strings never replace them. Session creation
  correlation is the caller's responsibility before registration.
- Text/tool details are normalized only for known shapes. Unknown update kinds
  and unsupported content remain opaque, with the original wire value retained.
  Raw data is untrusted and may contain secrets. Apply the application's privacy,
  retention and redaction policies before durable storage or UI rendering.
- Permission responses require a scope-checked decision from trusted application
  policy. The mapping does not authorize operations. Only exact offered
  `allow_once`/`reject_once` selections are mapped. Persistent grants and unknown
  kinds fail closed; cancelled turns answer pending requests as cancelled.
- Cancelling sends a notification and records `cancel_requested`. Only a correlated
  prompt result with `stopReason=cancelled` confirms cancellation. Disconnects,
  protocol errors and unrecognized stop reasons never imply success or absence of
  effects. `end_turn` completes the Provider turn, not task acceptance.

The `transport::ProviderProcess` API accepts an explicitly approved installation
with an absolute executable, argv, workspace and explicit environment. It clears
inherited environment variables, uses no shell or RC files, negotiates ACP, creates
sessions with `session/new`, and sends/awaits prompt and cancellation messages.
The approval reference is provenance only; the application's authorization store
must validate it before constructing the descriptor. The required expected SHA-256
is checked against the selected canonical executable immediately before spawn.
There is no auto-discovery-to-execution path and no automatic retry. Executable
selection does not establish a filesystem/network sandbox or prevent native
Provider code from accessing external configuration on its own.

Nonblocking Unix pipes enforce bounded frames and read/write deadlines. Stderr is
drained fairly but only its byte count is retained. Idle waiting uses pipe
readiness, so a small stderr pipe does not impose a fixed sleep on every refill.
Linux fixtures explicitly shrink that pipe to 4096 bytes while retaining the
4 MiB flood volume and original 500 ms process deadline. Raw protocol evidence is capped
at 1 MiB/128 records with an explicit dropped-record count; this diagnostic buffer
is not a durable control-event journal. On transport failure, active outcomes become
unknown and the owned process group is killed before its direct child is reaped.
Processes that deliberately escape that group require separate OS supervision.
Session associations from a disconnected transport cannot be reused after a new
initialize, even if the prior turn was idle or completed. Permission requests are
cancelled until a trusted application-policy callback is implemented. Transport
supports one outstanding prompt per owned process; this is not full orchestration
scheduling. Separate owned processes maintain separate local bindings, even if
Providers reuse the same native session ID.

Workers can call `begin_prompt`, then `poll_prompt(session, quantum)` while checking
their cancellation/budget state between quanta. Quantum expiry returns `None` and
retains the process. A turn keeps its original monotonic deadline; repeated polls
cannot extend it. `set_io_timeout` configures only the next operation and rejects
pending-turn extensions. `cancel` queues one notification and remains a request
until a correlated native stop result. Bounded nonblocking outbound queues avoid
blocking the worker when the child stops reading stdin.

`take_updates` yields raw-free typed summaries with local binding. These summaries
can still contain private user text: downstream storage must apply privacy and
retention rules. Text/opaque overflow increments explicit dropped-record counters;
a critical permission-decision overflow fails closed instead of silently dropping
it. `PermissionDenied` records the local decision to send a cancelled permission
response; it does not assert that native tools are OS-sandboxed or that external
side effects stopped. The diagnostic update buffer is not a durable journal.

See [installation provenance and connection contracts](PROVENANCE.md) for canonical
selection, drift rejection, bounded version probes and immutable named bindings.

Verification dimensions: 20 protocol contracts, 8 connection contracts, 9 actual
selected fixture executable version probes and 19 actual Unix stdio fixture
subprocess tests. The latter exercise real pipe I/O/processes, without calling a
model or claiming real Provider compatibility. Session/load, MCP/host tools,
Provider authentication/model calls, positive native-approval policy callbacks, durable
raw-event storage/redaction and live native Provider compatibility remain unimplemented
or unverified here. Trusted application approval-store integration is owned by the daemon.

Run from the repository root:

```sh
cargo test -p agentbaton-providers
cargo clippy -p agentbaton-providers --all-targets -- -D warnings
cargo fmt --all --check
```

## Explicit vendor routes and native Codex dialect

`driver::ManagedProviderProcess::start(route, approved_installation, timeout)`
selects a codec explicitly. The caller supplies already approved, exact argv and
environment; this factory never installs an adapter, starts a login flow, chooses
a new account/model/billing mode, or falls back to another route. Codex App Server
uses its own `{id,method,params}` dialect and `initialized`, `thread/start`,
`turn/start`, `turn/interrupt`, and correlated `turn/completed` handling. ACP
routes use JSON-RPC2.0/protocolVersion1. Claude means the official selected ACP
adapter executable, not an invented `claude --acp` invocation; Qoder uses its
explicit documented ACP launch configured by the caller.

Both dialects share one codec-independent owned-process pump with the original
frame/output bounds, fair16-read stderr batches, readiness polling, fixed monotonic
deadlines, outbound backpressure and owned process-group reap. A native
`turn/start` result (even with status completed) and an interrupt RPC ACK are not
completion evidence. Only a matching terminal native notification ends that turn;
an early terminal waits for its correlated submission response. Old terminal IDs
cannot complete a new turn. Tracking is bounded at4096 completed turn IDs per
process; exhaustion is explicit, never silent history forgetting. Turn completion
does not accept the Baton task. Late opaque metadata/control ACKs do not acquire
current-turn authority.

Native command/file approval requests are declined, or cancelled after local cancel;
permission expansion receives an empty turn-scoped grant. Unknown client/auth/tool
requests receive a correlated unsupported error; no credential reader or generic host
execution is implemented. Native initialization fields, configuration home, declared
workspace-write/on-request policy and network/root restrictions are consistency
checks only. They do not prove native account identity, sandbox enforcement, or
absence of hidden inherited configuration. Nondefault native policies and resume
remain explicit follow-up scope. No native Provider executable was run for these tests.

`configure_session_configuration(&Value)` takes only the trusted server's approved
snapshot projection before the first ACP session. It cannot itself prove approval.
MCP arrays must currently be empty; nonempty arrays fail as unsupported rather than
being ignored or implicitly connected. Claude transmits exactly the whitelisted
`settingSources:[]`, generated three-key `settings`,
`allowDangerouslySkipPermissions:false`, and `strictMcpConfig:true`. Only bounded
project-MCP disable names are supported; enabling project servers remains unsupported
until scoped MCP invocation is implemented. Unknown metadata, dangerous SDK settings,
ambient setting sources and post-session mutation are rejected. Native Codex rejects
this ACP metadata API. These requested options are not runtime-loading or enforcement
proof.

The native source is pinned in
[protocol evidence](../../docs/execution/provider-adapters/PROTOCOL_EVIDENCE.md).
`tests/validate-native-schema.py` validates a captured generated-child roundtrip
against the hash-verified complete official schema using the existing Python
jsonschema development dependency; it refuses external schema references. Set
`AGENTBATON_TEST_SCHEMA_CAPTURE` only when running the corresponding Rust test
binary to export its generated fixture wire. That hook exists only in test code.
The actual eight-message schema validation is separate from Rust fixture tests and
from real vendor/platform compatibility.
