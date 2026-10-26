# ADR 0001: Local authority and first vertical integration

Status: accepted reversible implementation choice under AB-START-001; not full product acceptance.

Keep a Rust workspace for core, authenticated Unix-socket daemon, CLI, protocol and filesystem libraries. The desktop is Tauri 2/React/TypeScript with its own lockfiles and a narrow command bridge. SQLite and atomic domain events live behind one daemon writer lock. UI exit does not imply daemon/task shutdown. There is no production TCP control listener.

A task created by this first API is a durable draft, not an execution request. No provider identity or executable is chosen, no model is called, and no attempt is fabricated. Cancelling a draft can confirm that no attempt was dispatched; cancelling an active attempt in later work requires explicit stop evidence. The full schema TaskRequest and later attempt/execution-envelope binding remain separate from this draft API.

IPC v1 rejects unknown fields, authenticates using a private local runtime credential, checks expected revisions on cancellation and requires create idempotency keys. The shared local user is the current caller boundary; additional per-session grants must precede MCP/remote access. Private filesystem permissions do not protect against an attacker already controlling that user account.

The three existing Environment check names are preserved. Product Rust/UI checks and dual-architecture macOS build jobs are additive. Packaging produces development DMGs with exact commit and SHA-256; signature inspection and manual desktop acceptance remain separate evidence. No minimum supported macOS is declared from the configured deployment target alone.

Tradeoffs and remaining work: the initial storage schema is a foundation, not all logical domain tables; protocol fixtures do not establish installed Provider compatibility; safe worktree/path primitives do not establish an OS sandbox. Full requirement statuses remain in requirements-progress.json.

The original design validator recursively scanned installed npm documentation and failed on package README links whose source docs are not distributed. It now discovers repository-authored Markdown through NUL-delimited Git tracked/untracked non-ignored paths. Link/source assertions are unchanged; a regression checks ignored dependency docs versus broken authored docs. BOOTSTRAP_MANIFEST explicitly tracks the reviewed validator implementation update; reference design hashes and immutable requirements/decisions/work-packages remain unchanged.
