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
