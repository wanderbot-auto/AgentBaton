# Evidence — actual Integrity observation

Tested source: 733fff526f88d3438f7533a0b474ba0474954e4f; baseline 87b4434a032fc2995facdee4761d877fa79b7770; source tree be34148f7c83633d718ea5d480a95905d909f988.
Evidence directory: /workspace/.onboarding/goal-validation-observation-ui.
Author worktree: /workspace/.worktrees/goal-validation-observation-ui.

## Provided real Core bundles and verified source bytes

- Original actual bundle: 7cc70a97cac37d97967a292e519e5d6a5eaecf2267eec9a449d8f0a43e39a0f6; 71,707 bytes; original fields unchanged. It has no TaskProjection.
- Interim real TaskProjection bundle: 74c44f411cd8dc719427d1cd624be5b27c8b50a0ab5401c4acc0fb6f6ca7d88f; 73,333 bytes; preserved in local evidence.
- Final portable actual bundle: 5821176997487a594416b740e7e40538ae7d1dfa1c9e133a1531a5e4e093a0d2; 73,731 bytes. Includes actual tasks.get projection, complete original observation/goal_io and first Owner decision bound to the nonempty validation_refs proposal. Same exact bytes as the Core author's generated artifact; no fields synthesized or overwritten.
- Complete observation digests were actually checked for both included Integrity packets using standard WebCrypto. This verifies only these actual Integrity serialized values; it is not a generic parse/stringify equivalence claim for Rust f64 or future Data/Command receipts.
- Core fixture execution belongs to the Core author. This leaf executes static DTO/DOM tests against those bytes and does not claim another live Core or Provider run.

## Actual tests and commands

Complete Vitest: 547 tests / 34 files passed (the unchanged 537 existing tests plus 10 new tests). Selected new tests: 10 / 2 files passed. Typecheck, Vite build, diff check passed.
- full-suite: /workspace/AgentBaton/apps/desktop/node_modules/.bin/vitest run --config /workspace/.onboarding/goal-validation-observation-ui/vitest.config.mts --reporter=verbose → exit 0; 35.116 s; log SHA-256 b01bc9bbadcf34cae1b6c2d928222e3541f1911ed6e09730f35388abc164d69a
- selected: /workspace/AgentBaton/apps/desktop/node_modules/.bin/vitest run src/goal-graph-integrity-observation.test.ts src/GoalGraphExecution.integrity-observation.test.tsx --config /workspace/.onboarding/goal-validation-observation-ui/vitest.config.mts --reporter=verbose → exit 0; 2.494 s; log SHA-256 7ecc1abd9d218c2cb086ef245efa740205613e619a1c824709fd1bbf87a808db
- typecheck: /workspace/AgentBaton/apps/desktop/node_modules/.bin/tsc --noEmit → exit 0; 1.637 s; log SHA-256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
- build: /workspace/AgentBaton/apps/desktop/node_modules/.bin/vite build --config /workspace/.onboarding/goal-validation-observation-ui/vitest.config.mts → exit 0; 1.108 s; log SHA-256 e0e46afa3503baf72f1350c1753c0a70e215be552359ca892d4d94a30a893f6c
- diff-check: git diff --check 87b4434a032fc2995facdee4761d877fa79b7770 733fff526f88d3438f7533a0b474ba0474954e4f → exit 0; 0.007 s; log SHA-256 e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

Standalone Rust parser/format check: /workspace/.toolchains/rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustfmt --edition 2021 --check apps/desktop/src-tauri/src/approval.rs → exit 0. This does not compile or run Rust. The three new Native callback tests remain NOT_RUN; Root must assign an explicit Cargo window.

## Preserved failures and source

- Baseline unchanged real positive consistency check: 1 passed.
- Baseline missing/null/unsafe-integer observation controls: 1 positive passed / 3 controls failed, exit 1. Log baseline-missing-observation-actual.log retained; no original failure is deleted.
- First wrong-relative-path invocation found no tests; preserved separately as a tool error, not a product failure.
- First typecheck found absent Node type definitions and a readonly assignment. A direct types reference still failed because those definitions are absent. Both logs remain. Final tests use existing standard WebCrypto and immutable copies; no dependency, tsconfig or lock modification.
- All original 32 renderer test files, four existing actual/failure JSON fixtures and 36 approval Rust test blocks remain byte-identical.
- Source inventory, exact log hashes and actual receipt/proposal binding checks are in verification.json. Existing bundle-size warning remains unsuppressed.

## Requirement evidence and limits

AB-TSK-003/004/008/011: old authority, original key/CAS and version/evidence floors retained by unchanged tests; no new IPC.
AB-TSK-007: passed validator receipts require actual stop/join; new Native review Cancel source tests cover each page and final confirmation with zero private decision writes, pending real Rust execution.
AB-TSK-010/012: full observation preserves actual goal_io/counters as observations without inventing token, cost, quota or semantic success.
AB-SEC-002/004: readonly UI and original fixed host-private actions, with no new cloud upload, generic command or credential.
AB-REL-001: Node/jsdom, supplied actual Core data, callback source, real Core execution, macOS and Provider dimensions remain separate.
All requirement mappings remain partial at project level.

NOT_RUN / NOT_CLAIMED: Native compilation/execution for this revision, actual macOS WebView or OS dialog, full Data/Command graphs, real Provider, CI, installers/signing and full Design v0.2. Independent review is pending Root's separate reviewer. No production release or new authority.
