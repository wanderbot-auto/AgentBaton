# Independent review · project registry and foundation CI follow-up

Reviewer: native `/root/review`. Active branch `codex/project-registry`, initial base `03878121b649bcc90d0f35ace740f5bfbc10fe82`. Project registry/core/UI have been independently reviewed and tested as detailed below. Capability and installation follow-up review is complete for the source hashes below; no complete-product or native acceptance is implied.

## PR #3 actual CI findings

Read GitHub API job evidence for PR #3 exact head `03878121b649bcc90d0f35ace740f5bfbc10fe82`.

- Product run `40362148029`: Linux Rust and UI checks succeeded; both macOS product checks failed during core IPC tests, before Tauri host build/package steps.
- ARM job `121710139626`: `oversized_frame_does_not_kill_daemon` failed at client `write_all` with BrokenPipe errno32.
- Intel job `121710139576`: same test failed at client `write_all` with NotConnected errno57.
- Root cause under investigation at this point: the oversized socket fixture assumes guaranteed completion of its large client write and rejection-response delivery even though the bounded reader is permitted to reject and close. Initial suspicion focused on an unread trailing newline. This is not evidence the daemon died.
- Independently reviewed isolated `/workspace/.worktrees/foundation-ci` test-only correction: send exactly `MAX_FRAME+1` bytes without trailing newline. The exact oversize rejection and subsequent authenticated daemon-liveness assertions remain unchanged. Approved for resubmission after targeted test; fresh remote checks remain required. No product assertion is deleted or skipped.
- Environment run `40362147943` at observation: Ubuntu and Intel succeeded; ARM queued. Queued is not failure or success; final result must be read independently.

Raw CI logs retained under ignored local artifacts (not fabricated platform outcomes):

- `artifacts/product-foundation/ci-arm-failure.log` SHA-256 `f640b608dec803d9488d230efe8dd77cd541c207b5bf6901b818ab748b3396b3`
- `artifacts/product-foundation/ci-intel-failure.log` SHA-256 `0ba5dd40cc0642acb6db1f04f77bf8011f5241e8f014195177d5ad83a1a9875a`

## PR #3 second candidate and corrected transport test contract

Candidate `8dd441c1fbaa1342e546291ab3a69ec94defe32f`, product run `40362744480`: Linux Rust/UI succeeded, Intel job `121712240151` still failed at the oversized fixture client `write_all` with BrokenPipe, before native build. ARM was queued at observation. This disproves the hypothesis that removing the extra newline alone suffices on Darwin; the failure evidence is retained, not replaced by a successful Linux result.

Independently reviewed the next isolated fix: a direct `read_frame` UnixStream-pair unit test asserts `RESOURCE_LIMIT` at its exact bound independently of socket error delivery. The integration test accepts only documented peer-close errors (BrokenPipe/ConnectionReset/NotConnected) or EOF, validates any delivered response as `RESOURCE_LIMIT`, rejects timeouts/malformed responses, and always requires a subsequent authenticated snapshot to prove daemon liveness. Approved for fresh CI: parser correctness and daemon availability assertions are retained while removing an invalid delivery guarantee for an oversized sender. This approval is not a claim of fresh macOS success.

## Project registry/core/UI review

No remaining blocking defect identified in the reviewed project slice after live managed-workspace identity checks were added. Suitable for candidate integration and exact-SHA tests; native and remote CI are still separate prerequisites.

- Reviewed persistent project import/archive/reassociation, idempotency and expected revision, directory identity/replacement protection, host-selected private worktree root, persisted worktree creation reservations, restart-to-unknown recovery, non-replayed external side effects, and registered-project task binding.
- Reviewed schema-v3 migration with private pre-upgrade backups and transactional new tables; future schema refusal remains in effect. Backup/failed-upgrade tests keep prior records recoverable.
- Finding resolved: lease grant initially trusted only persisted `ready` state. The implementation now inspects actual canonical root, device/inode, Git common-directory identity and pinned base HEAD for a new grant; renew/validate check identity while permitting the current writer's own commits. Replacement-root regression passes.
- Renderer commands do not select managed destinations or supply lease/dispatch authority. UI shows recorded source/head/dirty observations, archived controls, workspace creation/unknown state and explicit lack of execution-lease evidence. Device/inode numeric fields are never compared or roundtripped by the renderer.
- `cargo test -p agentbaton-core --locked` independently passed 33 tests: execution7, IPC5, migration2, registry IPC3, registry10, storage6. Real local Git repositories and Unix socket calls validate source dirty/untracked preservation, isolated checkout, concurrent idempotent clients, archive and restart.
- `npm test -- --reporter=dot` independently passed 15 tests across three files. These are mocked Tauri transport/component serialization tests, not native desktop evidence.

Limitations: legacy `local` tasks are drafts; the low-level trusted storage helpers are not authorization grants. Lease expiry and fencing only govern cooperating Baton operations, do not stop an old Provider or external editor, and do not create an OS sandbox. Future dispatch must reconcile/stop previous executors before workspace reuse. UTC persisted TTL is not a monotonic runtime budget or a guarantee across wall-clock changes. Project create/import operations currently run serially through the daemon, and full scheduling/configuration/Provider/remote/retention behavior is not accepted from this slice.

Additional raw failed-job log: `artifacts/product-foundation/ci-intel-8dd441c-failure.log` SHA-256 `218d4d43bb7baca9de45707bc585ac38de46bb36578b8c43c2c4e9d534dca981`.

## PR #3 third candidate and refined oversized-send regression

Candidate `84adc45`, run `40363883855`, Intel job `121716221969`: the direct parser unit test passed, but the integration client received `PROVIDER_DISCONNECTED` instead of `RESOURCE_LIMIT`. The generic transport classification comes from an underlying read failure before a complete oversized frame was processed. No native build result was reached. Raw log retained below.

Independently reviewed fourth causal correction in isolated foundation-ci: stream in bounded 4096-byte writes and half-close; distinguish fully written payload from interrupted send. A delivered response after a fully written frame still must be `RESOURCE_LIMIT`; `PROVIDER_DISCONNECTED` is accepted only after a known interrupted client write. Unexpected transport errors, timeouts and malformed responses still fail. A fresh authenticated snapshot must show both empty tasks and sequence zero. Direct parser-bound rejection remains independently asserted. This is approved for resubmission; it does not establish a successful remote run.

## Capability and installation follow-up verdict

No remaining blocking finding in the reviewed, unintegrated capability/installation/connection primitives after the malformed-session fix. They may be included in the next checkpoint candidate, with applicable integration and native verification still pending.

- Finding fixed: capability assessment previously allowed an empty binding session ID and empty observation session ID to agree and become `Observed`. `assess` now rejects malformed/empty/oversized bindings and marks missing/malformed runtime observation associations as mismatch. The new regression passes. Binding IDs are explicitly documented as Baton-global trusted-registry identities, not vendor-native IDs shared across accounts.
- Reviewed selected-file private no-follow reads; explicit source/version-specific merge policy and mandatory organization constraints; immutable source/snapshot hashes and full provenance; unknown raw values preserved privately; separate configured/enabled, diagnostic, native availability/load/use and freshness evidence. Successful independent diagnostics never prove runtime use. No scripts or MCPs are executed by these snapshot primitives.
- Reviewed selected canonical executable SHA-256 binding before process spawn; bounded exact-argv version probes with explicit environment, fair bounded pipe reads, timeout and owned unreaped process-group cleanup. Contradictory success/nonzero-exit and missing provenance cannot bind an execution. Typed named connections preserve program/configuration/identity/host distinctions and prohibit fallback/writeback modes. Documentation/version/fixture evidence cannot satisfy live runtime capability prerequisites.
- `cargo test -p agentbaton-config --locked` independently passed 26 tests: one transaction failure unit, 13 transaction integration, 12 capability tests including malformed session associations.
- `cargo test -p agentbaton-providers --locked` independently passed 48 tests: 20 protocol, 11 actual fixture-child stdio, nine actual selected-fixture version probes and eight typed connection contracts. No live vendor/model/account call occurred.
- After integrating the added parser unit, `cargo test -p agentbaton-core --locked` independently passed 34 tests (the earlier 33 plus one direct bounded-parser regression). UI15 result remains the separate mocked transport verification above.

Integration limits: static capability inventory parsing is not vendor-native discovery, runtime use or configuration application. Private inventories are fixtures/custom normalized metadata, not proof arbitrary existing native/global files are supported. Provenance references/observation origins must be authenticated by future registry/adapter code. Executable path verification has a documented hash-to-spawn TOCTOU window and does not pin interpreters, argv scripts, loaded libraries or native config. Public Rust metadata types do not confer authorization or OS isolation. Core and provider execution-binding representations still need an explicit validated persisted bridge before real dispatch; this review does not claim that bridge exists. Real vendor compatibility, native secret-store behavior, full scheduling, remote execution, UI wiring for capability/installation modules and macOS acceptance remain unverified/incomplete.

Third-candidate raw log: `artifacts/product-foundation/ci-intel-84adc45-failure.log` SHA-256 `bf3ae8acc568f191df60ffdae7cf52b366512f3ca0bd71f39241ec02e23b3fcd`.

## Exact reviewed working-tree sources

Hashes bind this review to the code and lockfiles before the checkpoint commit, not to an imagined remote result. Later changes require follow-up. Documentation/evidence metadata is listed when it belongs to the reviewed crate; status/requirements tracking is the parent's integration responsibility.

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `0a5e18a75333e4a068d9c63d43fe5ac176ebf6247ff1c0a12eb91d6b859d0557` |
| `apps/cli/src/main.rs` | `a2701f6ea5a377cca6a542df4eea4dd67b6314476dbb421d6e423d1f4801f67c` |
| `apps/desktop/README.md` | `af8a2b187e8379536ae63a1aba38883eaf63ee869906f94b341e859ffed86fed` |
| `apps/desktop/src-tauri/Cargo.lock` | `61cb7b8cbb02b15cf7b6d72a49c2fecaac291a2f3e037599e68e496d7538ff3d` |
| `apps/desktop/src-tauri/src/main.rs` | `100b33b06394d93df1d6e5245461f85d4602d574ff92031e6c23c42b0c1113aa` |
| `apps/desktop/src/App.test.tsx` | `079afe463dc56e79dfc7e3c3c90fdcae15d98aef2627c1b89beb0af69540852c` |
| `apps/desktop/src/App.tsx` | `4e1f9334efc513715fcb3d2e50f05dd12ec3af762077a2b6a4d5d2d45a9ba023` |
| `apps/desktop/src/Projects.test.tsx` | `409b00cfd1621c8f8cbffe2f86d278e8069077a0404947f606ff5e0d4709dfa8` |
| `apps/desktop/src/Projects.tsx` | `64cc627ac2275f1f7b21ab6018db6f5d64688bd6becdee18faf62f208f69d75a` |
| `apps/desktop/src/api.test.ts` | `3321a66e98bfbd4e264756cd1d34e7c84ab00c5c80709ac443e5821a3993278c` |
| `apps/desktop/src/api.ts` | `b626501476e73480d15cda5655f691aa702e66867009c3272f41ea26e8ff47cd` |
| `apps/desktop/src/i18n.ts` | `e3623d51040dbbecfb7633abea4838a26415800893b7d245357cd816fe9aa8f3` |
| `apps/desktop/src/style.css` | `cb62f42aa003d7e1244f7187eeaee48750b0ae08738cc515a9236e32a83da265` |
| `crates/config/README.md` | `96009bbd83d2aadcec190814bdb37edb48667005ede1b278c2165c90ea7b96ff` |
| `crates/config/src/capabilities.rs` | `2c8975a02d2183354c2e2a55426bb9ccf80eb431b73c8089ed2754db9bf69a13` |
| `crates/config/src/lib.rs` | `da5b00909547862fff1583b6593a40b874e5f7eb36b04ed27f925069e7a2df13` |
| `crates/config/tests/capabilities.rs` | `b27c0a7460acd0433734aed8db3ad7a99b577ca003c1a6a270d44416df576acc` |
| `crates/core/Cargo.toml` | `7b8bf9c6ae3912ae744c26cdb08dc9b9378506aecd10eebc88d554271e9127eb` |
| `crates/core/src/execution.rs` | `3d1992d1a0ae2d0b4484ec8faf44dee612970c75e63db37c9e6254b82106f9df` |
| `crates/core/src/ipc.rs` | `9b8328810fe639200957df7b48ca003014f790851456020d11891487599205f7` |
| `crates/core/src/lib.rs` | `968df12fe9fe65569775f4fb6531e037c9e31e2da1c8d626b57a941cfe9d6907` |
| `crates/core/src/registry.rs` | `6574e4dad21094279da3069ce6659c5c7d8a070205f06fd61b8ae0f247da2bff` |
| `crates/core/src/storage.rs` | `0c264d5499637eecf50c333830d996336f30b28e87a4e8f573dc58800fa0ab1c` |
| `crates/core/tests/execution.rs` | `e322291303da63d5ef04f0094c7a38e333b1c69ccc44cf9f998b9c33cb391e8d` |
| `crates/core/tests/ipc.rs` | `44e59d276006628543b0579c3ffb17e6bd2f9a491f2ede95cca1fed1e1aa8711` |
| `crates/core/tests/migration_v3.rs` | `c3cb5af07f7e48abd8bf63d2885108d68ce196fd7ff60e20f5e39f3cd47fcd7b` |
| `crates/core/tests/project_ipc.rs` | `9b38de8f283a706730edb49060c11124ad6341111ac0e76abdb07930884205f8` |
| `crates/core/tests/registry.rs` | `7a35c34e4557cad8e350b6ebade5b880dd9e4c0cb320c3101468b9155acc3041` |
| `crates/core/tests/storage.rs` | `492b192a251c15c5a1f5210e07cf957b4618115074c664139006bdae09d2cbf6` |
| `crates/providers/Cargo.toml` | `ebcef2dbe3294cf341990be4843da8e9e20fe2f4f65501f90fff06e038ef37c0` |
| `crates/providers/EVIDENCE.json` | `0f10199ea3ab61d10f11fe566b6caca0d4c9ecdda97f35f58e7dbe9dcf7340b3` |
| `crates/providers/PROVENANCE.md` | `8aca242a321a0a512e2dbe09722b991fac0f59b08922e6530e7462df0d44e818` |
| `crates/providers/README.md` | `5335ac7340c48e4a77aa9b0adfff3d183025b0b692f238e7bf6efd645a525e97` |
| `crates/providers/documentation-evidence.json` | `d97e8c6040900c542ba3c2e579d6f53b74aa005938a0cf1ceb8aef5a55b05c52` |
| `crates/providers/src/connections.rs` | `37fe79be1974b4af019722ea9c4b6d73acecc8d21f96c9d4bd0c3b4add941eb1` |
| `crates/providers/src/installation.rs` | `0710fbe74027bc2dfe812a99a53052a40af842b09200d8c74c827f28970da889` |
| `crates/providers/src/lib.rs` | `d7c4772de18ee6c802c2d29bcda09415a36d67e3664c5fe65dceba532dc8cac1` |
| `crates/providers/src/transport.rs` | `e8b140c850e8141a3ec5783d685a43c9943b27d84ed3161f6253c4a126dbbc76` |
| `crates/providers/tests/connections.rs` | `31aa177dff7ad010fa147c68f3e338ef5c33f1fdb430322a95cde9b65d4199c3` |
| `crates/providers/tests/installation.rs` | `2d7d4a6ebe2663c624b9b56992359611efb40d4e6f75b4561dd5d38825ffaf52` |
| `crates/providers/tests/stdio.rs` | `ec2d71a9092eae0f3792dafa2ee3a1bb8cf6343e600a2caf3edbafb34247eeb5` |
