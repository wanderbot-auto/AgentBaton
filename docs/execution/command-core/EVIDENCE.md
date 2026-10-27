# Command Core evidence

Managed Linux x86_64 Cloud. No Provider account, model request, network/service fixture, credential, production deployment or signing operation. All command fixtures are generated locally. The real lifecycle fixture is an account-free executable whose version probe and ACP initialize succeed and session/new deliberately fails; no prompt/model call is issued. Other storage fixtures are explicitly synthetic never-dispatched stopped sessions and are not claimed as live Provider validation.

Final exact-source commands:

```sh
export CARGO_HOME=/workspace/.toolchains/cargo
export RUSTUP_HOME=/workspace/.toolchains/rustup
export PATH="$CARGO_HOME/bin:$PATH"
export CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target
export CARGO_BUILD_JOBS=2
export CARGO_INCREMENTAL=0
umask 022
cargo test --locked -p agentbaton-core
cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings
cargo fmt --all -- --check
```

**128 unique Core tests PASS**: base111, new13 command integration +3 command migration +1 pre-cancel unit. Current library unit binary has37 cases, and existing integration groups have3 approval,7 execution,5 IPC,2 migration_v3,2 migration_v5,2 migration_v6,3 migration_v7,13 policy,3 projectIPC,10 registry,17 sessionIPC,8 storage. New command groups have13+3. Core doctests0. The existing FIFO isolation case invokes itself once in a child; nested output1 is an additional execution of that same case and is excluded from unique counts. No assertions were removed/lowered and no failure timeout was increased. Earlier informal126 was a nested-output counting mistake, corrected to125 for that intermediate source; final source adds3 unique cases and is128.

New actual evidence covers private owner proposal/type/digest/revision binding; one-run/same-key history; ignored report output from committed script/input; exact input/script/HEAD changes; no arbitrary environment; real script timeout and cancel; real Unix socket status responsiveness, stale observed revision cancellation and TaskCancelled only after owned receipts; policy revocation; durable prepared restart→unknown; no new-key/policy/Provider writer fence bypass; ordinary integrity/data proposal and unconsumed approval rejection while a command is active; own normal terminal rule reconstruction still passes; unknown reader quarantine; actual daemon shutdown→writer rebind; prepare and post-execution terminal journal faults; private migration backups/raw document and blob preservation; schema8 transaction rollback/unknown data retention; future9 entire database bytes unchanged; raw4MiB/count guarded history. Capture streams are never stored as raw text in command receipts.

The terminal journal fault is deliberately after the real script appends its marker and exits. The test proves the marker exists exactly once, command remains running with a durable fence after rollback, restart changes it to unknown, and same-key replay does not append again. An unreaped actual original Child remains owned in CommandWorkers for reconciliation and daemon lock-before-reap shutdown; thread completion alone is not stop proof. No-start receipts are separately verified through a real pre-cancel library result. Kernel stalls can delay graceful shutdown; there is no promise to forcibly interrupt them or to contain all escaped descendants.

The initial actual ACP fixture emitted malformed JSON during initialize. The transport returned unknown without typed process-reap proof, so the guard correctly refused independent validation; the failed log is retained. The corrected positive lifecycle uses actual successful initialize then session/new failure, where the driver still owns its process and explicitly terminates/reaps it. Production APIs record intentStopped/empty lease and TaskOutcomeUnknown; the new independent command passes and leaves TaskOutcomeUnknown, never TaskAccepted. This is not replay of the original Provider action and not a real Provider/account test.

Migration fixtures now reconstruct real old schemas by removing newly added schema8 tables/triggers before setting older version numbers. Top target assertions advance7→8 and future8→9; historical pre-v7 files retain original version/content semantics. A new private pre-v8 backup captures actual schema7 before the command migration. Tests compare raw serialized task-document bytes and binary BLOB bytes in live/backup databases, prove reopenability/0600 permissions and rollback retention. Only the future9 test claims entire database raw-file bytes unchanged; SQLite backup is not claimed to have an identical physical header/file layout.

Logs are retained under ignored artifacts/command-core; all source hashes are in CHECKPOINT:

| Log | Actual result | SHA-256 |
| --- | --- | --- |
| core-final-closed-guards.log | 128 unique Core PASS; existing nested FIFO child separately reruns1 | `61dd5acbe5c630f27fec30c60b8d6568c0c924d94c53182772ea083ca9f2254a` |
| clippy-final-closed-guards.log | all-targets -Dwarnings PASS | `4757a0f5cafa2cc04b2e5ffa67915ec418408df65c8605e57550017771d0cfd5` |
| fmt-final-closed-guards.log | PASS empty output | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| real-confirmed-stop-targeted.log | actual generated ACP session-start failure/reap→new independent command PASS; Task still Unknown | `e3a10bdcb9011497f0cc01c9f6090c71ef007af28c2a2647c58cfbbecaf4a0a7` |
| precancel-unit.log | actual no-start receipt classified without phantom child | `984f2762a67509d8421762a00237587835ad08bb47a50fd7ef488ebaa53a9516` |
| closed-guards-targeted.log | Intermediate12PASS/1FAIL: early initialize failure had no typed stop proof | `37d467d864e7d7cd904d0222992b55120ed7ca0b1d36c5598c896602bbf65b3e` |
| migration-v7-before-fixture-upgrade.log | Original legacy fixture1PASS/2FAIL before version advancement | `0e3c44e4f311eee3ac7265fa808bd5a6bdbbd7a9f805a0ea527bfc39e3df1925` |
| clippy-first.log | FAIL in test while-let style, subsequently fixed | `ca0f4b1a789fcb761cccd2c125a1ba60ba830c691e04f3c2d3901a2d2cdd9ee2` |
| check-initial.log | FAIL type typo and shared stale dependency metadata | `0b112e99d2a4c0f042a7537a358ced0bd42b0f7663aa81b9d8f4c403fc0e79f2` |
| core-first.log | FAIL new fixture wrong field, fixed | `f1bddce853752903c2d8602773891be5fb4e1e2a1e33ce8bc161b82866699078` |
| core-expanded-full.log | FAIL shared stale dependency metadata after another branch used target | `d6237bf70c77a28b0b31a55abd702f3600d1e5b309fc59ceceaf3169433045f2` |

All earlier passing/intermediate logs remain local. Shared dependency cache sometimes reused an older branch's workspace metadata despite the exact module/export being present; refreshing only this clone's dependency mtimes forced its source rebuild, without modifying library contents or deleting errors. Independent review/CI on the exact source remains required. The primary removed only rebuilding incremental cache at an idle boundary; deps, binaries, logs and source were preserved.

Limits/unrun: no combined Native/UI/browser check in this unit, no current-candidate macOS run, no production/signing or model validation. The standalone library's older two-target cross compilation does not prove this Core candidate runs on macOS. Commands verify only approved fixed-command observations; no TaskAccepted/scientific correctness/Git adoption token is created. Only clean pinned WorkspaceArtifacts are supported; uncommitted Provider edits and exact target integration-candidate adoption remain another unit. Nonsecret environment is intentionally limited to LANG/LC_ALL fixed locale constants and TZ=UTC. Arguments are displayed/retained and must not contain credentials. Strict filesystem/network/input/auxiliary-source/tree guarantees unavailable in the library are rejected before spawn, not silently weakened. Root bindings are observations, not OS write isolation. Thread-spawn failure handling is source-reviewed but not forced with an OS resource-limit fault here; its durable one-run intent remains consumed and historical replay must not treat it as absent. Early initialize failures lacking a typed cleanup receipt remain unknown and fenced, never fabricated as stopped.


## Post-commit owned-control repair

Initial source8009824 independently passed128 unique Core tests, all-target Clippy and formatting; the independent full log is `/workspace/.onboarding/command-core-independent-tests.log`, SHA35607907ed4e301068a98f049c4b9d6a700c9208e78a2299ea7f93db5c6482ba. This remains valid historical evidence, not evidence for later source.

A new real Unix IPC test `committed_revoke_and_task_cancel_never_become_not_started_errors_from_owned_record_corruption` installs a generated SQLite trigger during an actual command run. On revoke's idempotency INSERT it corrupts only the owned run document beyond4MiB. Old source actually committed policy.revoked=true, then its post-commit cancel_policy decoded that corrupted run and returned RESOURCE_LIMIT/side_effect_state=not_started. Task.cancel's analogous control already returned its committed result on old source; no equivalent old post-commit lookup existed there. Both old controls, including the failing revoke, remain in the preserved log.

The fixed worker caches immutable policy_id/task_id from the actual durable prepared record. cancel_policy and cancel_task perform pure-memory matching/setAtomicBool operations; no caller-supplied metadata, PID scan or fallible database decode occurs after commit. The same corruption controls now return committed success for revoke/task cancel. Subsequent actor raw-limit reconciliation still fails conservatively and the persistent workspace fence remains; fatal-path teardown retires owned work. This does not repair corrupt history by deleting it or manufacture stop evidence.

Final commands are the same CARGO_INCREMENTAL=0/JOBS2/umask022 fullCore/Clippy/fmt commands above. New exact-source result: **129 unique Core tests PASS**, base111 +18new (14 command integration,3migration,1unit); existing nested FIFO child1 is still excluded from unique counts. No failure timeout or guard was relaxed.

| Follow-up log in artifacts/command-core | Actual result | SHA-256 |
| --- | --- | --- |
| postcommit-old-source-control.log | Old-source trigger reproduces committed revoke misreportednot_started; expectedFAIL101 | `cceb147856b20405ec467a442217e13c58d7e6ac922d6e5d361b1871e6e694f4` |
| postcommit-fixed-source-control.log | Intermediate compileFAIL omitted Worker metadata initializer, corrected | `0d99de46c57486e4c216eab2e230ee36ca2fd7bc59ec41bcebec4164febf5fa1` |
| postcommit-final-source-control.log | Both committed revoke/task_cancel controls PASS with result, actor conservatively fails raw limit and fence remains | `abede85615e59c49b54feb2d247b676ed2ec79f6d453b71621f2e7599f0b6567` |
| core-postcommit-final.log | 129 unique Core PASS | `a394e5da2f3e3b6ac54e0899e26bb4b7d247749dd602d0eea974852d7210ad27` |
| clippy-postcommit-final.log | all-targets -Dwarnings PASS | `ec7e553abc13a9de335d4d91e193da7eb7fc35aa26f3f6a8e176be045605f0e3` |
| fmt-postcommit-final.log | PASS empty output | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The independent reviewer is retesting this fixed source; no new independent approval is claimed until their verdict. No Native/macOS/combined browser result is added by this follow-up. Latest merged main66061439c79fa44b66a33782febc1fb0f31c73df was read for OWNER001/002 ACK; it changes no execution scope for this unit.
