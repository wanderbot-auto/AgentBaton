# Independent Command Core/schema8 review

Verdict: **no blocking correctness finding remains in the reviewed Core increment at `88ca6bcc4d9cdb9a78cf0e46ccd905ec323bfeda`**, based on `6b19d1c1633c9e52e95f83c1d45c0c40427ee84c` through initial `8009824eaf2b83b37e65fe63f773bcf59143fc01`. Independent final **129 unique Core tests, all-target Clippy and formatting pass**. This accepts the scoped Core implementation/recovery checks, not combined Native/UI/macOS, project-wide budget enforcement, TaskAccepted, Git adoption, scientific correctness or full Design v0.2.

The reviewer read AGENTS/gate/OWNER001/002, snapshotted initial in-progress Core source at `/tmp/command-core-static-20261027T021133Z` (manifest SHA `7b186bf1bcfed0ea07b9a5118a9c26ed60c91e3c90aa13c8146f8f744a9a1339`), reviewed the final deltas and ran tests only after coordinated target/source freezes. The reviewer changed only this document and ignored logs/manifests/snapshot files; no source/lock, index, commit, push, merge, new agent, paid service or real Provider/model/account operation occurred. Native/UI/Goal work is separately owned.

## Authority, execution and recovery

Selection/policy/record DTOs are serializable data, not execution authority. Fixed Core routes resolve the current authenticated owner's persisted proposal; the private library ApprovedCommandRule is reconstructed from the exact subject, program/script/input hashes, approved root identities, cwd/base, literal argv, bounded settings and current policy. Contract content and separately checked approval/revision/refs are all bound; a source-agent DTO or caller flag cannot construct the private owned-run read exception. The nonsecret environment accepts only fixed LANG/LC_ALL values and TZ=UTC. Arguments are displayed/retained, and must not contain credentials. Captured stdout/stderr bytes are retained in the bounded library result, while persisted Core receipts contain observations/hashes rather than raw stream text.

Prepared run, original key/payload, journal and no-TTL workspace fence commit together before thread/program dispatch. The immediate transaction rechecks policy/consumption/fence. Same-key replay reads the original run; changing keys cannot evade max_runs=1, including failed/no-start/unknown intents. The actor owns at most one command worker at a time and excludes concurrent Provider workers. Actual Unix-IPC get/cancel/revoke responsiveness while a generated command runs is tested. Preflight/file/Git checks are still synchronous observations; execution responsiveness does not mean every filesystem syscall is interruptible.

Critical CommandRun results come through retained JoinHandles, not a lossy event queue. A started unreaped original Child remains retained and reconciled; no-start receipts are not phantom children. Error/drop/shutdown paths retire owned workers before the daemon socket/lock is relinquished. Unknown cleanup remains unknown and fenced even after later leader reap; restart never reexecutes a durable unresolved intent and does not release a fence by TTL. Library `Err` paths were traced to before successful spawn; post-spawn observations return CommandRun, including cleanup uncertainty. Worker panic/restart records do not become stopped proof or automatic retry permission.

Ordinary integrity/Data propose, unconsumed approve and fresh run paths use the shared stopped-workspace reader and refuse a command fence. Historical receipt replay/listing is metadata, not a fresh file validation. Only the command module's private terminal-read authority may pass its own exact running fence, with matching run/policy/task/session/attempt/workspace, current policy revision and lease epoch. New Provider attempts/lease inserts/updates are also blocked by durable SQL fencing. Current spec/input/root/lease/project/policy drift prevents final command acceptance. No command result is converted to task or integration acceptance.

Raw document/payload/result lengths are checked in SQL before loading/decoding command history and owner finalization; command history is also count/aggregate-byte bounded (100 rows/4 MiB). The schema8 migration creates separate policy/run/fence tables and triggers, keeps earlier backup history and adds a private0600 schema7 backup before command migration. New and existing tests preserve raw serialized old documents and BLOBs, rollback unknown-data failures, reopen backups and refuse future9 without changing database-file bytes. Old fixture edits reconstruct the genuine older schema by dropping new tables/triggers and advance expected target/future versions; original preservation assertions remain.

## Findings closed on final source

**Task cancellation did not converge:** initial static review found that a command could stop and release its fence while its task stayed CancelRequested forever. This was a source-level finding, not a claimed independent pre-fix failing run. Final source now atomically records the terminal command, releases a known-clean fence and marks TaskCancelled/journals only when that task has no active/unreconciled command and all Provider intents are stopped. Unknown/escaped cleanup does not qualify. The task event explicitly limits proof to Baton-owned runtimes and retains descendant-containment unknown. Direct and actual Unix-IPC cancellation regressions pass independently.

**Committed revoke could be misreported not_started:** after the initial independent128 pass, review found post-commit cancel_policy still decoded owned history. The author then installed a real SQLite trigger during a running generated command: old source8009824 committed revoked=true, but returned RESOURCE_LIMIT/not_started after its owned record exceeded4MiB. The old log was directly read/rehashed and remains FAIL101. Task.cancel's old analogous control already returned success; it is not falsely described as the same original bug. Final88 caches actual prepared-run policy/task IDs in private Worker metadata and makes post-commit cancellation pure memory/AtomicBool, without a fallible DB read. The unchanged fault now returns committed success for both controls; subsequent actor raw-limit failure still retains the fence and performs owned teardown. It does not delete corrupt history or invent cleanup proof. The final regression ran independently as part of all129.

The previously identified shared-reader/real TaskUnknown/no-start issues were also checked, rather than re-counted as new independent findings. A real account-free ACP fixture successfully initializes, fails session/new, is actually terminated/reaped and records intentStopped/free lease while task result stays OutcomeUnknown. Independent owner-approved command validation now works for that session without changing TaskOutcomeUnknown to TaskAccepted. An earlier initialize-failure fixture lacked typed cleanup proof; its refusal remains preserved, and the guard was not weakened to accept it. Current task state/revision is pinned, so a later cancellation/revision change requests stop instead. The actual pre-cancel library receipt has process_started=false and no child/ambiguous cleanup to retain.

The nested last_error clarification is deliberately scoped: when a post-execution rule/acceptance check fails and process_started is true, last_error records confirmed, nonretryable and command-run:<id>. This confirms observed execution, **not** validator PASS, descendant containment, whole-task completion or a new execution grant. Before-spawn errors and genuinely unknown DB/cleanup paths keep their own meanings; no blanket not_started downgrade was added.

## Independent final execution

Linux managed Cloud; `umask 022`, `CARGO_HOME=/workspace/.toolchains/cargo`, `RUSTUP_HOME=/workspace/.toolchains/rustup`, Cargo bin on PATH, `CARGO_BUILD_JOBS=2`, `CARGO_INCREMENTAL=0`, coordinated `CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target`. Root/Goal writers were serialized; target was handed to GoalStore after this review's final checks. No threshold/assertion was relaxed.

| Actual independent command | Result | Log SHA-256 |
| --- | --- | --- |
| `cargo test --locked -p agentbaton-core` | 129 unique pass; zero failures/ignored; doctests0 | `b56f7a5eb5afd4573b32a410295f0525f3af4a5ce87bd9c91705ab3027c10a84` |
| `cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings` | PASS, exit0 | `a5f4c585ee974ca44916ac30a98bbc189e067a7e0a6bc6d2e8d6bc525be724af` |
| `cargo fmt --all -- --check` | PASS, exit0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Independent logs are `/workspace/.onboarding/command-core-final-independent-{tests,clippy,fmt}.log`. Unique count is37 library +3 approval +14 command +3 command-migration +7 execution +5 IPC +2 migration_v3 +2 migration_v5 +2 migration_v6 +3 migration_v7 +13 policy +3 project IPC +10 registry +17 sessions IPC +8 storage =129. The existing nested FIFO child runs1 selected case again and is excluded. This includes actual generated programs, ACP startup/cleanup, real Unix IPC, task cancellation convergence, prepare/terminal/post-commit journal faults, restart/fence/non-replay and migration negatives. No real vendor executable/account/model request is used.

All14 final source/test hashes were checked against CHECKPOINT and committed88 before/after testing. The workspaces library and both locks are byte-identical to baseline6b19; the library hashes remain command source d5fa7b28..., tests1ec87088..., lib e396e03e.... `git diff --check` also passed. Earlier independent128 at8009824 remains historical evidence (log SHA35607907ed4e301068a98f049c4b9d6a700c9208e78a2299ea7f93db5c6482ba), not relabeled as final129.

| Exact final source/test | SHA-256 |
| --- | --- |
| `crates/core/src/command_validation.rs` | `a268db26f883ca6c9dbb64dd42ea45f07955512394bf9b800e4f09d2b814bae0` |
| `crates/core/src/command_workers.rs` | `73c26d9f2db4e5e59cdbe13655af536a64d7ceb9f95977f8f90da9a9cc2ddd84` |
| `crates/core/src/ipc.rs` | `4bc30ac4cd4c400586ef4d9c968a6ab71f6250c741eb61e3d97933fe1371dee4` |
| `crates/core/src/lib.rs` | `b944881ec6353c80eca87994a2bb5e5edc569ebbc51fc2a2071330a01043a547` |
| `crates/core/src/sessions.rs` | `200de9b12eb403743392c2dc972c3a5b60dfca4bd60a720dd7363beb67087cb7` |
| `crates/core/src/storage.rs` | `667f7e5ef76c85e97abe3ff502611792f9e95e1dbcce2acdfff5c50434c2c749` |
| `crates/core/src/validation.rs` | `0192be28e8f25de0f5c410ed6822ee07b8fff9dce118f6eff0c504d481312278` |
| `crates/core/tests/command_validation.rs` | `272a2dfa2b6a038809ef4d6f23fe422c7a4b6a78c2861280ece8eb4d9b1945cc` |
| `crates/core/tests/command_validation_migration.rs` | `8b7777dad8ce0e65bea564e32239b60e867783144c21c1273b0f153c5e609365` |
| `crates/core/tests/migration_v3.rs` | `e5a19679f57fc695f75b06416f1e712b07101b82b00ba67db33e4bf7f2cbd47a` |
| `crates/core/tests/migration_v5.rs` | `c2309db6b4cada2a4f71de7c67b342a5b2cacf9083a23cb556a240d7c137d31f` |
| `crates/core/tests/migration_v6.rs` | `2a47de014322788fbc305465740b17b191e7a8ef2da9376e561bc0f2c22c2774` |
| `crates/core/tests/migration_v7.rs` | `5fbf8ce5c28be4dfe9710874628feadd321c431a273d163b7acdcd7b4a73033e` |
| `crates/core/tests/storage.rs` | `09bdc5ad52e6ecb10c1a753714d88518df8ce4dd96f605257c53a0b73d78bad9` |

The independent manifest `/workspace/.onboarding/command-core-review/verification.json`, SHA-256 **5b6c92fc2a1eede19683ec8357647db37e38aa9731adafde6821e3e4614f182f**, records all14 source hashes, unchanged library/locks, independent logs and rehashed author evidence. Preserved key controls include:

| Author artifact in `artifacts/command-core/` | SHA-256 |
| --- | --- |
| `postcommit-old-source-control.log` — actual committed-revoke not_started FAIL101 | `cceb147856b20405ec467a442217e13c58d7e6ac922d6e5d361b1871e6e694f4` |
| `postcommit-final-source-control.log` — both fixed controls pass; raw history still fails conservatively | `abede85615e59c49b54feb2d247b676ed2ec79f6d453b71621f2e7599f0b6567` |
| `closed-guards-targeted.log` — initial early-ACP case correctly lacked stop proof | `37d467d864e7d7cd904d0222992b55120ed7ca0b1d36c5598c896602bbf65b3e` |
| `migration-v7-before-fixture-upgrade.log` — old fixture failure retained | `0e3c44e4f311eee3ac7265fa808bd5a6bdbbd7a9f805a0ea527bfc39e3df1925` |

## Material limits

Raw stdout/stderr are not durably retained by this Core increment; receipts establish stream counts/hashes, so complete retained command-output history is not established here. Per-policy single-use/time/capture bounds and one owned worker are implemented; **existing project/root-task shared budget accounting is not connected to command runs**. This unit must not be labeled as full shared-budget/scheduler completion. Root/workspace pins are observations, not OS filesystem/network isolation; continuous input/auxiliary-source immutability, dynamic/transitive dependency integrity and escaped-descendant containment are not established. Unavailable requested enforcement is rejected before spawn. Kernel stalls can delay owned shutdown. Thread-spawn resource exhaustion and arbitrary kernel/unwind failures were not fault-forced in this review; unknown states cannot be cleared based only on thread exit or elapsed time.

Only clean pinned WorkspaceArtifacts are admitted here; dirty Provider edits and exact target-head integration-candidate/Git adoption remain separate work. There is no current-source macOS, native owner-dialog, combined UI/browser, signed installer, production or real Provider/model compatibility result in this Core review. The older c4f library Mac13 and Data/native CI do not validate schema8 or later Command UI/Core/native compositions. Those require their own exact candidate source review and Linux/macOS CI, preserving all previous failures. Full Design v0.2 remains incomplete.
