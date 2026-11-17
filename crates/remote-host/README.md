# AgentBaton remote Host foundation

Independent development workspace, not wired into Core or the desktop. Read [ADR](../../docs/execution/remote-host-foundation/ADR-001.md) for authority, protocol, limits and remaining integration.

```sh
bash scripts/collab.sh run -- python3 scripts/verify-remote-host.py
bash scripts/collab.sh run -- cargo test --manifest-path crates/remote-host/Cargo.toml --locked
```

`baton-host-companion --serve /absolute/private/trusted.json` serves private stdio. Configuration is provisioned locally by an already authorized installer/Core, never by an RPC. `Config` and `Grant` in `src/protocol.rs` define the strict versioned schema. Create separate canonical 0700 mapped-record/state directories and a 0600 config in a 0700 directory; generate a scoped credential and provision only its SHA-256. Host/recipient/mapping/binding are explicit, not inferred from a local project path. Lease is at most 300 seconds. Same-lease process reopen permits reconciliation/cancel but safely pauses new effects; a fresh higher fence requires trusted local reprovision and cannot reset budget or broaden ceilings. Never put live credentials or real project data into checked-in examples.

Public Rust entry points: `transport::Client::{local,ssh,request,close}`, `controller::Ledger` (offline cancellation/cursor persistence), typed protocol and local `store::Store`. Local transport is for controlled two-process development. SSH registration must explicitly bind Host ID to known_hosts digest/alias and user identity. No actual remote handshake is run in this task.

The only executor is the restricted built-in record worker: exclusive leaf file creation within its approved mapping. It proves real bounded child communication, persistence and failure semantics; it is not an Agent/Provider implementation, terminal or arbitrary execution service. Internal `--worker` accepts the same trusted config and an existing durable intent only. There are no test-control servers, clock injection endpoints or production test permissions.

Version 1 is a new isolated journal schema; unknown future versions fail closed. Do not replace or erase a journal to recover an uncertain attempt. Record outcomes and consumed budget survive reconnect. Host cache is pinned pending Core retention integration. Product release/signing, real Hosts/Provider identities and root integration remain separately gated.
