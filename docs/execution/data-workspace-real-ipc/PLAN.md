# Real selected-file data workspace browser plan

ACK OWNER-001 / OWNER-002 at approved base `aa1c2814a30f43405f3c545ec178cf665f96e673`. Worktree `/workspace/.worktrees/data-workspace-real-ipc`, branch `codex/data-workspace-real-ipc`. Only a new standalone Python harness and these four records are owned; no UI/native/Core/library/locks/CI/global status or existing tests/harness changes.

Accurate baseline mappings read from `specs/requirements.json`: AB-PRJ-002/003 are WP05; AB-TSK-007 is WP08; AB-VAL-002 is WP09; AB-SEC-004/010 are WP12. The supplied informal WP06 name is not used to change the specification (baseline WP06 covers capabilities).

1. Coordinate actual schema10 namespaces/DTOs and runtime exclusion with Core. Use generated sibling `tempBase/runtime` and `tempBase/sources/...`, regular single-link files only. No Git initialization, user directories, accounts, sessions, models or SQL state writes.
2. Serve the exact built React supplied by `--frontend` behind a test-only strict fixed localhost adapter: required readonly metadata, preview start/get/list/cancel and creation get/list/cancel. HTTP denies approve/propose/finalize/generic IPC, arbitrary sources, permission flags and execution.
3. Through the real UI, enter relative file references and limits; start preview and observe actual count/hash/bytes/dataset version. Outside HTTP, the trusted host fixture reads the actual preview ID/version, validates and reviews complete kind=data_workspace snapshot, then calls the real private finalizer. Label this explicitly as host-fixture approval, not an OS OwnerDialog.
4. Verify actual ready destination `inputs/` bytes, registry Directory identity, dataset revision and source bytes/mtime. Exercise same-key replay/readonly history without duplicate copies or journal writes.
5. Exercise real32MiB regular sparse-file preview/copy cancellation. Only after actual cancel IPC returns, a Python-local one-shot control may drop that HTTP reply. Retain the actual downstream receipt and sequence; remount actual UI and reconcile through real readonly state, never a new copy key or forged terminal record. A completed race remains actual verified/ready and cancellation coverage is reported unrun, never relabeled successful cancellation.
6. Exercise invalid expected hashes, post-preview source drift, retained failure history and the private-runtime boundary with an own benign sentinel (never session.token as dataset). Preserve all failures/screenshots and exact binary/assets/harness hashes.

Run **only after primary supplies an immutable final schema10 daemon**. Do not build or borrow an older executable. `--batond`, `--frontend`, `--chromium`, `--output` are explicit. The test-only35s observation ceiling includes Core's unchanged30s I/O deadline; it does not extend product budgets or wait for a cancellation scheduling win.

Runtime evidence, independent review, native OwnerDialog/WebView, macOS and complete-product acceptance remain separate. A cancellation race without confirmed coverage yields explicit `incomplete_race_coverage`, exit2; a failed assertion exits1; fully executed planned coverage exits0.
