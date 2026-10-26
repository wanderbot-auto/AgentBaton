# Data contract frontend independent review

Reviewed frozen eight-file snapshot specified by UI_EVIDENCE.json; starting HEAD `8da2bee3ee6d070cb4e4e46562fc8829a1fd6f93`, primary checkpoint during review `d0c714b69c9114ab38cbeb702ebf815e9a6a501f`. All eight source hashes were verified before and after independent checks and remained unchanged. Baseline PR10 `3c1cd1a` plus reviewed deterministic library `cbd5509` remain separate prerequisites. Read current OWNER-001/002/gate and latest local origin/main inbox `c4e72c641e3edc3f3c0f665ffde5217933c2895f`; no new owner authorization was inferred.

Verdict: no unresolved blocking frontend finding in this snapshot. Scope is AB-VAL-003/004/006/009/010/011 and AB-DOM-004/008 frontend behavior, not acceptance of their full workflows. Core/schema7/migrations/native implementation, actual backend/browser composition, macOS and whole-product acceptance require their separate evidence. Reviewer made no source/test/lock/permission edits or commits.

## Contract and authority review

The typed selection contains one session ID,1–16 SHA-bound input contracts, exactly three outputs with fixed table/script/environment role IDs and a typed JSON table DTO. It cannot submit roots, resolved subject/attempt/spec/input identities, directory identity, approval flags, finalize, TaskAccepted or adoption authority. The five dedicated Tauri-facing commands match the current native/Core names and payloads. Approval and run are separate actions; opening Artifacts performs read-only task-scoped lists and no implicit mutation. Native approval kind data_validation and server-resolved full snapshot are separate from the proposal DTO; native code was read for shape/boundary only, not executed by this frontend reviewer.

Form parsing enforces unique bounded relative paths (240 UTF-8 bytes), rejects traversal/absolute/backslash/control paths, requires full SHA-256 for each input, enforces fixed role IDs and no path overlap. It accepts1–64 distinct ASCII column names with at least one numeric column. Scalar ranges require finite JSON binary64 numbers or exact decimal safe signed integers; counts/seed are nonnegative safe integers and do not pass through rounded u64/i64 identities. String columns are bounded by the selected string limit. Each numeric column has explicit nonblank units limited128 UTF-8 bytes; null-prototype units storage preserves prototype-named columns as own data.

Limits match the inspected library DTO:1..4194304 bytes/file, depth1..32, values1..200000, string bytes1..16384, rows1..20000, total declared file caps <=16MiB. Full rows must fit the selected row limit; sample rows are positive and <=declared population/row limit, with nonblank <=4096 UTF-8 byte limitation. Full/sample scope is retained, never upgraded. Timezone is bounded ASCII metadata, not validated geographic/timezone science. Selection preflight <=16KiB is explicitly distinct from Rust's independently bounded complete approval snapshot, which can reject a selection that fits its preflight. No field truncation, script/analysis engine, Provider/model/account call or scientific/algorithm correctness claim was added.

The old ArtifactValidation component/API and both existing test files were independently compared byte-for-byte to PR10 and are unchanged. The sole new App mount is alongside integrity in the task Artifacts tab, with an initially closed contextual panel. CSS additions are restricted to .data-validation and its responsive children, preserve existing canvas/composer rules and shared palette, and use restrained14/18px-compatible typography and44px mobile controls. Labels and English/Chinese warnings retain raw paths/hashes/failure pointers as text. No visual/native screenshot execution for this new panel is claimed by this review.

## Failure, reconciliation and evidence semantics

Policies/records are checked against exact current task/project, data scope/validator, data-version subject and safe revisions. Elevated correctness, task acceptance or integration flags are refused. Only exact contract_passed/contract_failed state spellings receive their respective labels; prototype names, unknown/future or case-varied states do not become passed or TaskAccepted. Known pass requires both data and file-integrity pass flags. Returned failure pointers/path/code content renders through React text, with no HTML execution, hyperlink or file opening. Dataset rows, units and timezone remain owner declarations and exact metadata checks; receipts clearly state no algorithm correctness, whole-task acceptance or integration authorization, and old receipts are historical evidence only.

Session filtering is only UI usability: matching task/project and stopped/failed are offered, while server confirmed-stop, attempt/root/input/spec and fencing/lease checks remain mandatory. Stale spec or missing current task spec disables run. Approval does not run automatically. Revocation uses exact observed revision and preserves historical evidence.

Unknown approval/run/revoke intents preserve deep-copied form, exact selection/key/policy revision per task across tab/task unmount/remount. The cache is process-local, not a persistent outbox across application restart. Reconciliation performs reads only and never automatically mutates, re-grants or replaces a request key. Every refresh first clears freshness; only both successful scope-valid list results restore it. Later failed or foreign reads re-block all mutations. Explicit same-request retry reuses the original payload/key/revision even if the new policy projection has a newer revision. Remount may perform actual successful read-only reconciliation before an explicit retry; an extra manual refresh click is not required. Unresolved revoke can clear only after a valid read confirms that same policy revoked. Source bounds the pending map at32 entries and refuses a new task entry instead of evicting or bypassing an unknown request.

## Independent checks

Cwd `/workspace/.worktrees/data-contract-validation/apps/desktop`, all exit0:

- `npm run typecheck`
- `npm test`:129 tests /11 files (existing78 plus51 new data UI/transport/parser cases).
- `npm run build`

Meaningful executed negative assertions cover unsafe paths, hashes/duplicate roles, unsafe rounded counts/seed, signed-integer bounds, nonfinite number bounds, column/UTF-8/units/string and total-byte/snapshot limits; full/sample bounds; native denial; exact unknown request retry across remount/task changes; old run revision retained despite newer projection; successful-read then later failed/foreign-read retry blocking; independent foreign/elevated history rejection; unknown constructor/failure markup rendered as text; stopped-session and stale-spec checks; read-only revoke reconciliation; unsupported policy/foreign approval response; localized/disconnected behavior. All prior tests remain. No test assertion or acceptance threshold was weakened by reviewer.

Audited8 current source hashes and5 worker log hashes: all match. Retained initial TS1117 duplicate i18n property failure remains in the evidence and was not relabeled passed. A temporary exact symlink to the main locked node_modules enabled the independent commands; only that reviewer-created symlink was removed afterward, leaving actual installed dependencies unchanged. No dependencies/lock edits, staging, commit, push, recursive agents or paid APIs.

Limits of this evidence: jsdom and mocked Tauri-facing transport, not real daemon/native owner confirmation or filesystem/symlink enforcement. The32-entry saturation branch and explicit case-varied state fixture were source-reviewed, not separately exercised by a dedicated new test; existing unknown-constructor behavior was executed. Current Core/native/lock/browser changes by primary were preserved and are outside this frontend approval. Fresh integrated backend/browser and exact-commit protected CI/native/macOS evidence remain required; no full GOAL completion is implied.

## Reviewed frontend SHA-256

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/DataContractValidation.tsx` | `6baa6fab529327597da6467462c9732b3a7c679cb3251f5924af6208b6b73721` |
| `apps/desktop/src/data-validation-api.ts` | `18d9b4ee9096ce9628964331cf2f646d58a0edb375dd63ade0da4579f3710ce4` |
| `apps/desktop/src/data-validation-i18n.ts` | `96160edfb341b04237d3483f1c3f75b7c8e92d386c4c442271f5afbeb289f123` |
| `apps/desktop/src/DataContractValidation.test.tsx` | `27255002e65611f0caa4c16e1b2de6118fa71c6ea14de9968036d1da66ca9798` |
| `apps/desktop/src/data-validation-api.test.ts` | `3baaba560995820394324e1000414d26ed0905b11c130ddea3e1ef66d356cf92` |
| `apps/desktop/src/App.tsx` | `92fc3f5a70f1322663047aec2e2869e329a8461a441651c571ed5a254c2b095a` |
| `apps/desktop/src/QuietCanvas.test.tsx` | `fd5361c142365db06c5c0e4c67f734771281e8f43ceb407bf1406ea31343c1e3` |
| `apps/desktop/src/style.css` | `0beaa70ea841aef88b65fca6dc486cd21885031f49e9f1f13fe1f7cc489693f3` |

## Independent logs SHA-256

Relative to `/workspace/.onboarding/data-validation-ui-independent/`.

| Artifact | SHA-256 |
| --- | --- |
| `build.log` | `135bcdd3f9370544bbb3d9bd81a200bf3e73642938cfde610818aaf9a7be9791` |
| `source-before.json` | `18ab06c356e93b5683c6c1f4d08047d7d969b510bb487b24b4b68b5ffdf2be11` |
| `tests.log` | `cd2ea862cd10c46dd2c86f49acec89134efffd156ca82457e159895aa5f7df9c` |
| `typecheck.log` | `642536308317e611561144cad25fb98c96caafe3b35d43f613b546901ea6d74e` |
