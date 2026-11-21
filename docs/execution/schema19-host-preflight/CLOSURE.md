# Accepted-a4 schema19 compatibility closure

Static contract, not implementation or tested compatibility. `SOURCE_MANIFEST.json` binds complete accepted source bytes/Git blobs to base `a4e03303b54bd5a5544131e95ad39662f8310e29`. `CLOSURE_MATRIX.json` lists literal file/line anchors, role, owner proposal and action; this document supplies semantic exceptions and indirect call edges. The original B95 candidate list is retained as a discovery input, not a completeness proof. A changed target/base requires rediscovery and independent review; no global18→19 edit is authorized.

## Current runtime and typed readers

| Literal file and accepted region | Current19 contract / preserved control |
| --- | --- |
| `crates/core/src/storage.rs:98–136,332–363` | Upper bound19, additional independent pre-v19 backup, Host migration after Conversation18 and before recovery. Original pre-v18 and all earlier backup branches stay. Historical exact16 reference744–764 stops at16. Whole legacy migration was segmented; only each new Host segment is atomic. |
| `crates/core/src/integration_store.rs:5–82,140–217` | Current reopen `matches!(current,17\|18)` and59/63-table trusted references need new exact19 authority-schema branch. Preserve original17/18 branches, original16→17 DDL and safe ordinary opaque-table/index compatibility.64 is known reference count, not a limit on arbitrary user DB tables. Construct reference without recursive `Store::open` or recovery. |
| `crates/core/src/background_status.rs:33–46,113` | Exact current integer19; old18 and future20 refusal. Actual read is not an inferred capability. |
| `crates/core/src/factory_reads.rs:1265–1281` | Worker exact19 read gate; keep query-only, bounds/cancellation/deadline semantics. |
| `crates/core/src/factory_history_reads.rs:143` | Same exact19 gate for composed history. |
| `crates/core/src/integration_targets.rs:32`, `integration_budget.rs:181`, `integration_fences.rs:100` | All three strict current19 gates must move together; no target/budget/fence authority from old/future schema. |
| `crates/core/src/integration_reads.rs:535,951` | Both worker and post-validator fresh-read gates become19; failure has no new side effect. |
| `crates/core/src/ipc.rs:1691` | Snapshot reports actual SQLite PRAGMA, no literal version substitution. API protocol1 remains1. |
| `apps/desktop/src-tauri/src/lib.rs:2378–2400,2998–3018` | Native exact19 authenticated handshake, type tests and error text. Existing live daemon18 yields incompatibility through the existing error return; it must not trigger another daemon or mutate its DB. |
| `apps/desktop/src-tauri/src/background_policy.rs:42,158–170` | Current19 status and exact typed fixture; preserve no-ownership/no-race policy. |
| `apps/desktop/src/api.ts:116` | Snapshot parser accepts exact19 only. Task accepted projection checks remain. |
| `apps/desktop/src/background-service-api.ts:2,6` | Both TypeScript literal `database_schema_version:18` and runtime parser become19. Type-only edits are insufficient. |
| `apps/desktop/src/TaskIntegration.tsx:47`, `integration-i18n.ts` | Current integration UI admission and bilingual unsupported-schema wording match exact19. API1/record1 remain distinct. |

`crates/core/src/conversations.rs:118–137` is **HISTORICAL18 migration**, including original DDL and `PRAGMA user_version=18`; its `version>=18` exit is a migration boundary, not a current reader to replace. Existing data-workspace/Goal/Factory/task-schema migrations retain their old SQL/version boundaries. `crates/remote-host/src/store.rs` and foundation protocol/store schemas are independent companion state; no Core19 replacement and no Host registry in that DB.

## Current tests, including anchors B95 missed

| Literal file / accepted region | Required adaptation, with original values that remain |
| --- | --- |
| `crates/core/src/factory_history_reads_tests.rs:392–442,480–550` | Current assertions399/487→19; old/future arrays405/508 retain15/16/17, add18 and use20 as future. Preserve revision0, not_started and raw-history equality controls. |
| `crates/core/src/factory_reads.rs:2033–2108` | Current readonly schema2080→19, negative versions2035 include18/20.2096 `PRAGMA user_version=18` is a **write-denial attack**, keep it and add19 attack. |
| `crates/core/tests/fixtures/integration_runtime.rs:289–292,758–769` | Current daemon and independent RO DB require19 (including missed768). |
| `crates/core/tests/integration_ipc.rs:54–74` | Missed `Server::image` current version64→19. Compare all preserved rows/schema on refusals. |
| `crates/core/tests/sessions_ipc.rs:3682–3686,4039–4067` | Current schema3684/4055→19, including missed graph-owner-clock evidence. Preserve exact grant/scope/revision/budget facts. |
| `crates/core/tests/fixtures/integration_stage_evidence.rs:497,702` | Both producer output and actual RO schema guard19. No schema constant without actual SQLite read. |
| `crates/core/tests/fixtures/integration_stage_evidence_tests.rs:421–476` | Current success branch19; old18/future20 refuse along with malformed/unknown source. Preserve original collector bounds/typed counts. |
| `crates/core/tests/admission_performance.rs:110`, `factory_runtime_ipc.rs:2021–2022`, `command_validation.rs:101–102`, `data_workspaces.rs:356`, `workspace_open_ipc.rs:101` | Current successful Store/daemon assertion19. Do not change deadlines, resource caps or original failure assertions. |
| `crates/core/src/conversations_history_tests.rs:106`, `crates/core/tests/fixtures/factory_child_goal_runtime.rs:427–428` | Current fixture DB19, not a historical producer. |
| `crates/core/tests/execution.rs:330–358` | Preserve generated malformed intermediate version16: the earlier migration segment commits16, then integration rejects corrupt input. This is **not** successful current output. |

## Genuine inputs, projection, backups and transaction chains

| Literal path / region | Classification and closure |
| --- | --- |
| `crates/core/tests/fixtures/pre_conversation_schema.rs:28–158` | Entire original exact18→17 inverse retained, including original SQL slices, four-table empty proof, FK/dependency checks, standalone/in-transaction distinction. It must continue rejecting19. New Host inverse runs first only at explicit current19 callers. |
| `crates/core/tests/fixtures/pre_integration_schema.rs:28–168` | Original17→16 inverse and exact current18 readonly composition stay strict. `verify_empty_integration17_current18:53–61` cannot directly accept19. Add a separate readonly current19 composition. |
| `crates/core/tests/fixtures/pre_factory_schema.rs:82–217,224–283` | Keep original16 inverse and existing18 wrappers. New19 wrappers prepend Host inverse in the same original caller transaction; do not turn `current18:bool` into a range. Preserve autocommit/FK-off-before-BEGIN/restore-after-commit-or-rollback behavior. |
| `crates/core/tests/conversation_schema18.rs:44–109,111–155` | Fresh Store19 must first become exact18 through Host inverse, then old18→17. Current table count with sentinel64→65; predecessor17 count60 stays. New current output19; old helper negative versions/rollback semantics unchanged. Verify original pre-v18 plus new pre-v19 separately. |
| `crates/core/tests/integration_schema17.rs:169–215,294–378,458–502,568–608` | Current18/63→19/64; current future19→20;18→17 helper call gets Host inverse first. Late caller collision rolls the entire19→18→17→16→older chain back to original19. Historical helper's wrong-version18 stays18. Keep old16 output/reference and original pre-v18 content. |
| `crates/core/src/task_workspace_binding_tests.rs:782–865` | Current788/801/834→19; new Host inverse before803 within same tx; old15 field/authority constraints stay; future19→20, original pre-v16/pre-v18 checks plus new pre-v19. |
| `crates/core/tests/task_workspace_actual15_migration.rs:164–218,582–644,810` | Genuine15/before55 preserved; current19/64; verify empty Host19 before excluding it from complete rows/DDL projections. Original12 explicit DDL cookie delta becomes14 for proposed table+index. Original pre-v16/pre-v18 exact input15 and all source/build/archive/helper hashes stay; new pre-v19 checked independently. |
| `crates/core/tests/integration_actual16_migration.rs:170–197,820–864,1002,1133` | Genuine16/before55 and manifest rejection types unchanged. Current19/64; separate readonly current19 domain proof, exact Host exclusion and original rowid/raw/DDL equality; future20; original pre-v18 + new pre-v19. |
| `crates/core/tests/factory_child_goal_legacy_migration.rs:487–590,661–820,973–988` | Genuine14/raw opaque input and original pre-v15/16/18 content stay. Current19 output/read/reopen, strict empty Host before projection exclusion, new backup separate; future20. No extra backups on reopen. |
| `crates/core/tests/sessions_ipc.rs:3807–3861` | Legacy projection/caller drop to11 must prepend exact empty Host19 handling within existing rollback boundary; compare all old raw rows. Never leave Host objects under old version labels. |
| `crates/core/tests/command_validation_migration.rs:157–181` | Current generated16 helper input needs explicit19→18→17→16 construction. Original16 inverse's15/17 negatives and collision controls remain. |

All current-to-old Factory generator callers need an explicit19 composition path; the original helpers remain unchanged:

- `crates/core/tests/command_validation_migration.rs:30`
- `crates/core/tests/data_workspace_migration.rs:29`
- `crates/core/tests/factory_child_plan_migration.rs:35,242`
- `crates/core/tests/factory_migration.rs:31`
- `crates/core/tests/goal_graph_migration.rs:29`
- `crates/core/tests/goal_runtime_migration.rs:21`
- `crates/core/tests/goal_store_migration.rs:31`
- `crates/core/tests/migration_v3.rs:16,78`
- `crates/core/tests/migration_v5.rs:17,95`
- `crates/core/tests/migration_v6.rs:17,138`
- `crates/core/tests/migration_v7.rs:36`
- `crates/core/tests/integration_schema17.rs:471`
- `crates/core/tests/sessions_ipc.rs:3851`

The corresponding successful current outputs (command-validation69; v7:91; child-plan132; Goal-store66/runtime46; data-workspace55; v6:51; v5:35; graph59; Factory68) move to19. Old input/backup versions4/5/6/7/8/9/10/11/12/13/14/15/16 remain. Current Store future19 rejection cases become future20, without changing old-helper exact negative inputs. Exact lower-version refusal controls are preserved in current reader tests; Store itself still legitimately migrates old supported DBs.

## Current native, scripts, UI and source pins

Native typed/current sites include `apps/desktop/src-tauri/src/bin/native_smoke.rs:123,226,360,497`, its current JSON count fixtures2050/2298/2689/2921, independent SQLite counts2580/3088 and old/future controls2104/2375/2747/2906/2947/3097. `native_owner_dialog_test/controller.rs:440,1031,1059–1060` and `integration_database.rs:29` are current19 producers/readers. New old18/future20/type controls must accompany the schema19 data; old original18 source restored by inverse still contains its original19 negative cases.

Current consumers not all captured by the B95 regex:

- `scripts/prepare-factory-test-tools.py:53,155,170` actual DB + emitted frozen current tool manifest + verifier.
- `scripts/record-child-goal-ci-evidence.py:57,65` compact `m['schema']==18` + output; `record-factory-rework-ci-evidence.py:20` alias `SCHEMA=18` feeding downstream reads.
- `scripts/record-integration-owner-ci-evidence.py:97,103`, `record-task-workspace-ci-evidence.py:214,221`, `record-integration-browser-ci-evidence.py:215,225,257,493,503` current evidence producers/consumers.
- `scripts/native-owner-dialog-smoke.py:141,179,892,919,1164`, `integration-owner-fixture.py:225,295–313,326`, `native-macos-smoke.sh:26–40,60,71` current report/table/read gates. Integration owner's exact table union must add only approved Host table; old `prepare-schema16` SCHEMA_TABLES tuple remains55.
- `scripts/native-linux-smoke.py:59–62` and macOS runner import/run need the new explicit19 entry, while `scripts/native-run.py` is **fully frozen** for historical-helper provenance (ADR-001). Original `test_native_runner.py` still tests original18; add new19 tests.
- `apps/desktop/tests/data-workspace-real-ipc.py`, `factory-child-goal-real-ipc.py`, `factory-child-planning-real-ipc.py`, `factory-real-ipc.py`, `goal-execution-real-ipc.py`, `goal-graph-real-ipc.py`, `integration-real-ipc.py`, `task-workspace-binding-real-ipc.py` and `task_notifications_real_ipc_probe.py` require current19 manifests/RO reads/outputs while preserving counts/no-execution/privacy/owned cleanup. Their current synthetic fixtures/collector tests need19 input plus18/20 rejection. An old17 mock that bypasses parser is not a genuine predecessor and not actual19 evidence; classify its test intent before editing.
- `.github/workflows/product.yml:196,510,548` are exactly the three current manifest/probe sites. Preserve genuine14/15/16 producer steps and all required checks, permissions, timeouts and failure evidence. Any workflow adaptation is leader-owned and source-inverse guarded; no CI operation ran here.
- UI current fixtures are enumerated literally in `CLOSURE_MATRIX.json` (including App/BackgroundService/FactoryChildGoals/GoalGraphExecution/TaskIntegration/TaskActions/notification/entry tests). Current18 fixtures and typed parser tests move to19; wrong-schema18/20 remain negatives. Do not replace every17 mock, API1 or unrelated format number.

Original historical producers/helper/source archives and their manifest assertions remain byte-identical: `scripts/prepare-{legacy,schema15,schema16}-migration-fixture.py`, `generate-schema16-migration-fixture.py`, `record-legacy-migration-ci-evidence.py`, genuine14/15 fixture generators and original16/17/18 references. Their historical reports remain reports of those older versions. New current evidence must have a fresh exact candidate/source/binary binding, not relabel those reports19.

`SOURCE_INVERSE_GRAPH.json` gives the existing9-file schema18 source-layer hashes and literal consumers. Preserve original `schema18_source_fixture.py`, `fixtures/schema18_source_inverse.json`, `schema18_host_fixture.py`, inner `test_schema17_migration_workflow.py` inverse function/constants and collector17/18/phase pins. Add a new, separately pinned source19 wrapper at current source-read boundaries, then compose the original layers in their actual dependency order. Some caller test files also contain frozen-source functions; only reviewed outer adapters/current data regions may change. Original pins themselves do not become19. Unknown bytes/path/span/hash/order must still reject.

Particular source-composition chains: native background/workflow/matrix/dev-profile guards; schema18 source→notification screenshot→planning/capture/fee-kind/fixed-script layers; integration host18→17→original diagnostics; collector source18→17→phase/preparation source pins. Multiple wrappers reading the same file need the same complete current19→a4 normalization, not separately updated broad hashes. Every exact path proposed for later writing appears in `PROPOSED_SCOPE.json`; none is reserved now.

## Scope and remaining uncertainty

This closes the static known baseline contract using lexical anchors, typed checks, alias constants, current helper call edges, migration projections, source-pin consumers and independent review.174 inventoried files/207 hashed context files are navigation counts only; they do not prove arbitrary future source complete. Pending: leader/B agree final DDL/wire/namespace, reserve one writer, choose actual integration base and reconcile any later accepted PR29 changes. No current schema19 code, product test, native GUI, CI, SSH or remote capability has been exercised. HEXP local626d/remote0feb and unaccepted PR29 fixed499 remain separate named dependencies; neither is used as the accepted source baseline. No other worktree was modified.
