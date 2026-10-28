# Goal execution UI and Native approval

Base822c3bb46c8ec8ba506e7682b07e2a1384ed0674; branch codex/goal-runtime-ui; OWNER001/002 and AB-START-001 ACK. No credentials, paid models, production or signing. Partial contributions: AB-TSK-003/004/007/008/009/010/011/012, AB-SEC-001/002/006/010; full acceptance remains open.

1. Coordinate exact goal_runs wire with Core owner, preserving prior preview-only scope semantics. New goal_execution Native confirmation is the only renderer approval route; no generic finalize, filesystem, SQL or shell permission.
2. Default-folded GoalExecution uses the existing focused Goal model. Read only after expansion, scoped/monotonic run metadata and ledger observations, bounded polling, exact-key/payload/revision recovery across navigation. Cancellation acknowledgement is not stop; candidates are not accepted tasks; null measurements remain unknown.
3. Five fixed Native handlers: independent approval, list, get, cancellation and explicit adoption of a newer persisted/validated same-scope plan. Daemon dispatch remains internal. The last action consumes the newly approved in-scope-version capability, never an old preview-only scope, resets no budget/history and never adopts unsaved local edits.
4. Complete server intent/scope/root_grant/existing_connection/plan and permissions are retained under the unchanged16KiB review bound. GoalExecution-only compact JSON may remove formatting whitespace but no fields or values. Oversize still refuses. Other approval kinds keep their prior behavior.
5. Preserve all367 baseline UI test-source bytes, original22 Native test bodies and12 harness bytes. New normal/negative/uncertain/version/history tests and actual Core-issued proposal callback controls precede source freeze.
6. Root owns independent review, current Core11/frontend real IPC/browser/macOS/OSdialog/default-release/package integration. Local mocks and callbacks prove only their recorded dimensions.
