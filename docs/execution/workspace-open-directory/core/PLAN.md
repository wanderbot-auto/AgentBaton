# AB-SES-009 Core directory observation

Base `e874b123d3e7f329a551e21922baf1a7a643548f`; authoritative interface: Root's `/workspace/.onboarding/workspace-open-contract-v1.json`.

Implement only the Core leg of the complete directory-opening chain: strict typed IDs/revisions -> registered local source or ready managed directory -> bounded read snapshot -> root-to-leaf NOFOLLOW descriptor observations -> opaque decimal identity context. Export the same descriptor pin/recheck helper for Native use. This neither opens a terminal nor creates/assigns a task, workspace, session, approval or execution authority.

Root owns existing lib.rs/ipc.rs hooks. Supply a private narrow patch, including authenticated method-only raw decoding to prevent Value from hiding duplicate keys. Existing routes/Factory companion allowlist remain unchanged. Desktop owns its separate new modules. No lock/schema/migration/budget/permission changes.

Author meaningful pure/wire, actual filesystem, registry and real Unix IPC test sources. This author runs only Python source checks, formatting and patch applicability; Root composes hooks, independently reviews and executes all Rust/SQLite/process/native tests.
