# Fixed-command validation plan

Baseline: `ddd6707e5c67ff66f65f1ab243f868ee35366959`, isolated branch `codex/deterministic-command-validators`. This is a dependency-ready WP09 library unit under AB-START-001, not completion of WP09 or the product. OWNER-001/002 ACK: retain full approved scope, the Quiet Canvas direction, and existing credentials/production/resource limits. Latest protected-main owner inbox read at `afb52be241f7c4cef261f99c04f56caf21f12083`; only 001/002 are present. No product Provider/model/account call is authorized or used.

Read `AGENTS.md`, execution gate/approval, `docs/06-orchestration.md`, `docs/08-security.md`, the existing artifact-validation/adoption contracts, and Provider-owned process/poll cleanup patterns. The approved scope is one new workspaces module, its tests, one module export and an explicitly authorized top-level comment clarification, and this unit's documents. No dependency, lock, Core, UI, workflow or other worktree edits.

1. Define a non-Deserialize trusted-host rule binding the complete subject, program/script hashes, artifact roots and identities, argv, cwd, environment digests, expected exits, runtime/capture budgets, mode, platform image policy and process identity. Reject unsupported strict enforcement before spawn.
2. Execute approved native ELF bytes from a sealed Linux memfd. Support macOS root-protected Mach-O paths only for nonroot execution with canonical, root-owned, non-writable ancestor chains and repeated identity/hash/permission observations. Do not imply verified dynamic dependencies or native configuration.
3. For approved scripts, use a fixed system sh and reviewed in-memory SHA-bound script bytes over stdin, with literal positional arguments. Do not execute a mutable script path. Known sh programs cannot use NativeExecutable as a bypass; this is not a classifier proving arbitrary binaries never interpret other code.
4. Poll bounded nonblocking stdin/stdout/stderr and the owned original Child. Check monotonic timeout/cancellation without reader threads. Signal only the created process group/direct child while preserving its unreaped leader with WNOWAIT. Record unknown cleanup/escaped descendants honestly.
5. Recheck safe BoundArtifact observations and cwd/Git candidate/target bindings after execution and at scoped acceptance. Produce only `approved_deterministic_command_only` evidence/token, never TaskAccepted, Git adoption, scientific correctness or production authorization.
6. Run generated local fixture programs and fixed system sh positive/negative tests, full workspaces tests, formatting and Clippy. Cross-check both macOS targets; actual macOS execution remains for existing approved two-architecture CI. Preserve failures and arrange independent review before integration.

| Requirement | This unit's evidence | Remaining integration/assurance |
| --- | --- | --- |
| AB-VAL-003 | Non-Deserialize approval and scoped acceptance, fixed contracts, true command/script execution | Core-native owner approval/proposal, persisted policy/run IPC, UI are not wired here |
| AB-VAL-004 | Program/script/input SHA, literal argv/environment digests, platform/UID and before/after artifacts | No units/population/scientific interpretation; transitive dependencies/native configuration remain unverified |
| AB-VAL-006 | BoundArtifact nofollow/single-link/bounded reads; root replacement, FIFO, symlink, hardlink and size negatives | No OS filesystem isolation or guarantee that an approved command cannot itself write elsewhere |
| AB-VAL-007 | Private command-only acceptance tied to exact rule/approval/revision and live artifact observations | No automatic TaskAccepted promotion or full task rules in this library |
| AB-VAL-009 | Approved Git candidate/tree/target-head checked before/after and again at acceptance | No merge/adoption side effects; caller still needs the separate authorized adoption contract |
| AB-VAL-011 | `integration_authorized=false`, task acceptance not established | No production approval, signing or publishing operation |
