# Evidence and interface

Actual Linux pure commands:

```sh
python3 -B tests/infrastructure/test_legacy_migration_platform.py -v
python3 -B crates/core/tests/fixtures/child_goal_legacy_migration/test_source_builder.py -v
python3 -B tests/infrastructure/test_legacy_migration_ci_evidence.py -v
```

Results:11/11/8 passed respectively. New controls cover both synthetic Mach-O headers/architecture mismatch, complete Darwin proof versus unknown/unjoined/forced/premature/bad-wait states, no pre-signal poll, conservative PID presence/zombie/reuse/error refusal, synthetic same-session/attempt matching, generated ACP Darwin branch without `/proc`, old Linux v1 compatibility and no private nested publication. These use mocks/synthetic metadata only, not fake actual Core history. AST parsing of builder/generator/generated ACP/serializer and rustfmt/diff checks pass. Logs/source hashes at private evidence directory in CHECKPOINT.

Same reviewed-source-build v1 additions:

- Build root: platform `linux|darwin`, new architecture (`arm64|x86_64` on Darwin), numeric Darwin `os_release`, `native_helper_sha256` digest. Linux historical records may lack new fields. Each Darwin binary adds format `mach-o-64` and matching architecture, derived from actual header.
- Private builder probe: identity_method `darwin_owned_unreaped_group`, original owned pid/pgid (private), leader_unreaped_until_signalling, group_quiescent_before_reaping, cleanup_errors; existing real schema/query/FK/counts/CLI/SIGTERM/wait/forcedkill guards retained.
- Private fixture: Darwin platform/architecture/release, daemon identity/SIGTERM/group/cleanup fields; provider markers carry original pid/pgid and task/attempt references. Core_join retains same session/attempt stopped/confirmed references privately; strict ps proof separately records no-match. Summary provider_stop_proof has fixed method `core_stop_confirmed_plus_pid_absence`, observed positive count, two true booleans.
- Public build: explicit host fields, helper digest, finite process_proof (fixed method, SIGTERM true, group quiescent true, wait0, forcedfalse); no raw probe or process IDs. Public fixture: bounded provider_stop_proof only, no private tuple/rows/RPC/capabilities/Grant. Never copy source dicts wholesale. Failed records still publish fixed failed status only.

Primary Core stop/join plus secondary PID absence does not claim process birth identity, OS sandbox, all descendants, Native OS dialog, Provider/model, or whole-product completion. Both real Mac architectures, rebuilt14/generator and updated Rust six-function/ten-case execution are NOT_RUN. Root owns subsequent workflow integration and actual CI evidence. No timeout increase or new fee/host/credential.

## Required Product dual-Mac workflow source

The unchanged desktop matrix runs `[macos-15, macos-15-intel]` under60 minutes. A new required builder/generator step uses the same explicit RUNNER_TEMP directories and shared workspace target as Linux, before the existing schema15 tool preparation. A new required step runs `cargo test --locked -p agentbaton-core --features actual-legacy-migration-tests --test factory_child_goal_legacy_migration -- --nocapture --test-threads=1` with the actual private fixture env, then feature-target `cargo clippy ... -- -D warnings`. Neither step has continue-on-error or optional conditions.

An `if: always()` finite serializer step writes only `artifacts/native-smoke/legacy-migration/summary.json`; the existing pinned native artifact upload retains that summary. No raw build/fixture/RPC/database/runtime directory is added to artifacts. Producer source/native proof validation remains in the reviewed scripts. Existing job names, matrix, action pins, product deadlines and timeout stay unchanged.

YAML/ordering/invariant/bash-source checks are source-only. Actual Apple Silicon/Intel builds, six-test counts/results, strict Clippy and timing remain NOT_RUN. Additional oldCore/dependency builds may exceed remaining Intel headroom even with shared target reuse; only real CI can establish fit. This change does not raise the timeout or waive later desktop checks.

Actual source verification: PyYAML parse; `bash -n` for each added run body; exact existing-job and existing-step equality against4a21fb64; unchanged matrix/job names/security pins/60-minute value; builder before15 tools before required consumer; private fixture env, explicit feature target and strict Clippy; `always()` finite projection and no new upload action/path. All PASS; full record `/workspace/.onboarding/legacy-migration-macos-portability/WORKFLOW_SOURCE_CHECKS.json`, workflow SHA `f5e3fdb73b16a8db1a24ebbf33632c16b8ad2f85538f61cf9fab504b53b54435`. This does not execute any of the validated run bodies.
