# Data workspace real IPC evidence — preparation only

## Exact inputs and actual command

- Approved inherited base `aa1c2814a30f43405f3c545ec178cf665f96e673`; no product source changes.
- New harness source `996815991489dec0d50c492a1a1e5865855a0e1e`.
- Script SHA-256 `444fe020a4fde10831161d6d98cb226fee34b318cc499e206347dc3a371f9ae9`.
- Read-only contract sources: `/workspace/.worktrees/data-workspace-core/crates/core/src/data_workspaces.rs`, `data_workspace_workers.rs`, `ipc.rs`, actual `tests/data_workspaces.rs`, library `data_workspace.rs`. Their source is still independently owned; these reads are not a build or runtime result.

Actually executed in own worktree:

```sh
python3 - <<'PY'
import py_compile
py_compile.compile('apps/desktop/tests/data-workspace-real-ipc.py',
                   cfile='/tmp/agentbaton-data-workspace-real-ipc.pyc',
                   doraise=True)
print('py_compile PASS: syntax only; no daemon/browser execution')
PY
git diff --check
```

**PASS, syntax only.** No daemon/browser execution, report, screenshot, source-mtime test, cancellation, private owner finalization or real copy occurred. No test failure has occurred in this preparation-only unit. There is no final binary/assets hash yet; those will be captured from primary's supplied artifacts during the actual run.

## Implemented but unexecuted checks

| Area | Script behavior | Current result |
| --- | --- | --- |
| AB-PRJ-002/003, WP05 | Actual Directory import, user-entered relative files/limits, observed hashes/bytes, host-approved real copy, inputs/ content and registry identity/base version | NOT RUN |
| AB-VAL-002, WP09 | Exact dataset/hash binding, source bytes/mtime, invalid expected hash and deliberate fixture mutation after original snapshot, no silent ready resnapshot | NOT RUN |
| AB-TSK-007, WP08 | UI cancel request, actual reply-loss injection, preserved response/journal sequence, remount/read reconciliation, truthful completed race and retained partial/unknown history | NOT RUN |
| AB-SEC-004/010, WP12 | Fixed HTTP scope, no approve/finalize/generic/execution, real host-only owner fixture, runtime sentinel exclusion, no private credentials or cloud/model | NOT RUN |
| Idempotency/history | Original keys/revisions, no second copy or readonly journal mutation, actual source unchanged outside explicitly recorded fixture drift | NOT RUN |
| OWNER-002 | Actual built React with desktop/mobile screenshot/overflow checks in Projects; no fake terminal/dashboard | NOT RUN |

No specification ID/work-package mapping was changed; WP06 is capabilities in the immutable baseline and is not used as normative data-workspace mapping.

## Runtime design boundaries

The adapter binds IPv4 loopback and exact Origin/Host/JSON/body/argument keys. Dataset references are restricted to explicitly generated file names and known fixture project IDs; record IDs enter allowlists only from actual scoped daemon responses. Readonly metadata is fixed. Data native approval, proposal, finalization, generic IPC and execution commands receive HTTP403. No test controls enter the production build.

Authentication is only this run's newly generated runtime token in a Python closure, never a dataset, HTTP field, log/report/screenshot or assertion failure value. Reports additionally redact that authentication string before writing. The negative runtime test selects only an own benign sentinel and checks actual permission denial/no preview persistence; it never reads token bytes as test data. No private Agent credential, other runtime or production key is accessed.

The one-shot fault forwards a real cancellation first, retains the actual downstream receipt/sequence in evidence, then closes HTTP without a fabricated result. Browser uncertainty/recovery is observed against actual get/list state. No stale or fake terminal records are supplied. If the worker completed first, actual verified/ready is recorded; no cooperative stop PASS is inferred. Unexecuted race coverage returns explicit exit2, failed assertions exit1, fully executed covered run exit0. Observation waits35s include the unchanged30s product I/O deadline, not a deadline increase.

SQL is strictly readonly count/sequence observation with query_only. No SQL Ready/connection/authority insertion, fixture success DTO, Git initialization, account, session, model or script execution. Native approval is clearly an external host fixture (actual Core proposal/private finalize), **not OS OwnerDialog proof**. Generated temporary trees are cleaned after inspection; product-side retention/non-replay is checked before fixture cleanup. No user source or unknown production output is deleted.

Pending: final immutable schema10 daemon/frontend; actual run and preserved failure artifacts; screenshot inspection; independent harness review; separately native OwnerDialog/WebView/macOS/CI. No entire-product, permission, execution, model, sandbox, DLP or acceptance completion claim.
