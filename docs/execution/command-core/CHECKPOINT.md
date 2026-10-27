# Command Core source freeze

Branch `codex/command-core-runtime`; worktree `/workspace/.worktrees/command-core`; baseline `6b19d1c1633c9e52e95f83c1d45c0c40427ee84c`. OWNER-001/002 ACK at protected main `febe7c74f62aad1f767aa60a0eab5e3fc0529dcb`, no new authorization. AB-START-001 covers this product unit; private credentials/model calls/new paid resources/production/signing remain closed.

Actual final result: **129 unique Core tests**, zero failures/skips, plus the existing FIFO child's extra selected rerun noted separately; Clippy and format checks pass. The source is frozen; this checkpoint and EVIDENCE bind its exact hashes. A local feature commit is authorized; no push/main update from this worktree. Independent reviewer owns REVIEW.md and is testing this exact source before parent integration.

| Source/test | SHA-256 |
| --- | --- |
| crates/core/src/command_validation.rs | `a268db26f883ca6c9dbb64dd42ea45f07955512394bf9b800e4f09d2b814bae0` |
| crates/core/src/command_workers.rs | `73c26d9f2db4e5e59cdbe13655af536a64d7ceb9f95977f8f90da9a9cc2ddd84` |
| crates/core/src/ipc.rs | `4bc30ac4cd4c400586ef4d9c968a6ab71f6250c741eb61e3d97933fe1371dee4` |
| crates/core/src/lib.rs | `b944881ec6353c80eca87994a2bb5e5edc569ebbc51fc2a2071330a01043a547` |
| crates/core/src/sessions.rs | `200de9b12eb403743392c2dc972c3a5b60dfca4bd60a720dd7363beb67087cb7` |
| crates/core/src/storage.rs | `667f7e5ef76c85e97abe3ff502611792f9e95e1dbcce2acdfff5c50434c2c749` |
| crates/core/src/validation.rs | `0192be28e8f25de0f5c410ed6822ee07b8fff9dce118f6eff0c504d481312278` |
| crates/core/tests/command_validation.rs | `272a2dfa2b6a038809ef4d6f23fe422c7a4b6a78c2861280ece8eb4d9b1945cc` |
| crates/core/tests/command_validation_migration.rs | `8b7777dad8ce0e65bea564e32239b60e867783144c21c1273b0f153c5e609365` |
| crates/core/tests/migration_v3.rs | `e5a19679f57fc695f75b06416f1e712b07101b82b00ba67db33e4bf7f2cbd47a` |
| crates/core/tests/migration_v5.rs | `c2309db6b4cada2a4f71de7c67b342a5b2cacf9083a23cb556a240d7c137d31f` |
| crates/core/tests/migration_v6.rs | `2a47de014322788fbc305465740b17b191e7a8ef2da9376e561bc0f2c22c2774` |
| crates/core/tests/migration_v7.rs | `5fbf8ce5c28be4dfe9710874628feadd321c431a273d163b7acdcd7b4a73033e` |
| crates/core/tests/storage.rs | `09bdc5ad52e6ecb10c1a753714d88518df8ce4dd96f605257c53a0b73d78bad9` |

The reviewed workspace command library is unchanged in content: src/command_validation.rs `d5fa7b28d6a694c4fa8f767a021499fb5c984a68937cfbf939fcf553cbd4e870`, lib.rs `e396e03e5fbdf99ee4ffa2016240c1771da2892977651340cef2b4f3afdd45a5`, tests/command_validation.rs `1ec8708817bb72197be4fb44afec25155810e70dc4c0fc6e3973f86fe912cfd5`. Only dependency mtimes were refreshed to avoid shared-target stale metadata after branch handoff; failures are preserved.

Schema8 adds separate command policy/run/fence tables and attempt/lease fencing triggers, preserving old pre-v7 snapshots and adding private schema7 pre-v8 backups. A downstream schema9 must keep those histories, upgrade real schema8 fixtures and preserve unknown recovery. Ordinary validation has no fence exception; the command module's private typed terminal-run authority is narrowly checked. It must not become a caller-controlled/global bypass.

Next: independent exact-fingerprint Core review/tests; address concrete findings. Parent then integrates Native/UI changes on its separate branch, runs combined build/browser/restart and exact-candidate Linux/macOS CI. No macOS or UI integration success is claimed here. Current clean pinned WorkspaceArtifacts verification is not a Git integration-candidate registry/adoption loop. Missing typed cleanup proof for early Provider initialize errors remains conservatively unknown, not silently treated as stopped.

Shared target is released to independent integration_review for its test window. No background executor/continued run is promised; resume only in an actual execution context. Retain all errors, traces, unknown fences and original assertions. Never rerun an unknown command with a new key or release a writer fence based on elapsed TTL/thread exit alone.


Post-commit control follow-up supersedes the initial8009824 source only in command_workers.rs, ipc.rs and command_validation.rs tests. Independent review actually passed initial128 before finding the revoke fault; that evidence is preserved. The new actual trigger control first reproduced revoked=true with RESOURCE_LIMIT/not_started on8009824. Fixed owned workers cache policy/task IDs from their true durable prepared record; post-commit revoke/task cancellation sets flags entirely in memory, without decoding potentially corrupted history. Updated exact-source fullCore129/Clippy/fmt pass. Independent delta review is pending, not self-declared complete. Latest protected main66061439c79fa44b66a33782febc1fb0f31c73df OWNER_INBOX was actually read; ACK001/002 only, no scope change.
