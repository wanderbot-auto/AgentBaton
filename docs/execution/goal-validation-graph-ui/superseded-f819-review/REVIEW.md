# Graph UI and Native independent review — in progress

Frozen original subject: f81979c45e7d86f5b21b045f9b5329d433aec19a.
Author docs: 9d8e5ffdfa0a28fa0ee865d938e553258bfe04f0.
Independent detached tree: /workspace/.worktrees/goal-graph-ui-review.
This reviewer did not author this UI, lib.rs or approval.rs. No product source was edited. Existing Node dependencies were temporarily symlinked. All generated cache and build output are under this evidence directory.

Current disposition: changes requested. Original subject is not green. Awaiting author source revision and complete independent revalidation.

## Confirmed original findings

R1 — goal-graph-guards.ts:55. Captured-artifact reference equality omits full root and file_pin identity. Two independent controls change safe numeric root/file-pin identity while retaining content and path. Both incorrectly return true. The Task acceptance and Owner-reference controls fail; origin-controls-f819.log preserves the first run. Author confirmed and is fixing complete canonical identity, without changing the old precision rejection.

R2 — GoalGraphExecution.tsx:70 and 93. Owner reconciliation only looks up the returned decision ID. A shape-valid response claiming the target owner-node can point at an existing decision for another Owner node in the same run. A fully shape-valid and consistent next detail retaining the still-pending target clears the hold and enables a new Owner action. The component control expects the action to stay disabled and actually fails. foreign-decision-controls-f819-v2.log and foreign-decision.review.test.tsx preserve the reproduction; the earlier configuration error is preserved separately. Reported to author and Root.

## Actual original-subject checks

- Full Vitest: exit 1; 528 passed, 1 failed, 529 total; 31 passing files and 1 failing file. The only suite failure is the previously known old actual-Core da00 accepted projection positive, not either independent control.
- Independent artifact origin controls: 2 failures, expected false but observed true.
- Component foreign decision control: 1 failure, expected disabled but observed enabled. Its combined run also replays the two origin failures.
- TypeScript tsc --noEmit: passed, empty diagnostic log.
- Vite production bundle to private evidence outDir: passed; existing over-500kB chunk warning preserved.
- Native preservation: all baseline 22 approval + 6 lib Rust test blocks remain exactly byte-identical, unique, in order; both complete old source line sequences are retained; source additions give 36 approval + 7 lib tests = 43. This is source evidence only.
- All 24 baseline renderer test files are byte-identical at f819, independently checked against 3b99f2.
- Root later supplied exact Linux Native composition 94157aed2cb69c6b503c5b6b12af5aff554476af. Independently rehashed its manifest dd76425e46e99fad9e6fdd58c17f232f992ffecde13b8cdb2bd002f546e4b325, all 84 source bindings, five command logs and two frozen binaries; its approval/lib source hashes exactly match f819. The Root-executed log really contains 43 library + 22 harness tests passed, Clippy and fmt passed. This is independent evidence verification of Root execution, not a reviewer Cargo run or macOS proof; it cannot be reused for a later Native revision.
- git diff --check with correct repository cwd: passed. A first invocation from /workspace failed with Could not access 3b99f2 and remains in the old log; it was not a code failure.

## Source review boundaries

Fixed frontend Graph commands and Native bridge actions remain separate: list/get, exact scoped approval, cancellation, plan advancement, Owner acceptance and explicit rejection. Renderer cannot select generic private approval methods. Native paged review retains all snapshot bytes through UTF-8-safe 12KiB chunks and 16KiB per-message bounds, with a separate complete 1MiB envelope cap; all pages and final confirmation precede a fixed private finalize/decline reference. Cancel returns not_started and no Owner decision, not explicit rejection. Old single-dialog kinds remain unchanged.

The caller preserves trusted summary cancellation when unsafe artifact detail fails closed. Historical exact-key advancement recovery retains newer run/ledger floors. Accepted Task presentation binds receipt/projection references, sequence, revision/spec, project/scope/grant/input/workspace, run and plan instead of treating a candidate as TaskAccepted.

Open source questions sent to author: consumed-input reference membership in the same captured candidate set; Native validation refs membership and joined/passed relationship. These are not presented as executed Rust failures.

## Not run / not claimed

No Cargo, shared target, package installation, daemon, real Provider, account, fee, CI trigger, GitHub Approve, merge, or production release action. Native 43 tests were not compiled or executed by this reviewer. The separately attributed Root exact-source Linux execution is recorded above. Browser/jsdom mock tests do not establish real Core IPC, real macOS WebView, actual Owner OS dialog, Apple Silicon/Intel installer or overall Goal/product acceptance. The separate finished OS-parent feasibility report remains source-only/Linux and does not change this reviewed source.

Final new-source positive, full suite, independent regressions, typecheck/build and immutable manifest remain pending.
