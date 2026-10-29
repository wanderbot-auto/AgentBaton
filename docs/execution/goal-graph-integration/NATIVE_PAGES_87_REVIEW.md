# Independent Native presentation review — fixed 87ec9ad

**Disposition: one open P2 (NP87-01).** The fixed source implements reversible bounded pages through the shipping caller-owned confirmation path and preserves the original approval and test gates. It omits U+061C from its explicit bidi-control escape set. No new macOS presentation execution or Rust compilation is certified by this review. This report is not formal approval or product acceptance.

Revision: `87ec9ad8647b549311fc90077e9757bc257c4b70`; baseline: `e3ccf86a8b12e7ed1371074de095bb454c0b62aa`. Requirements: SEC003/010, UX003 and readable Chinese/English content within this presentation delta. Fixed Git objects were exported to `source/`; later author metadata or source changes are outside this report. The prior actual double-Mac e3cc review is retained unchanged at `/workspace/.onboarding/pr16-e3cc-mac-review`.

No Cargo, daemon, shared target, remote, product source, STATUS, permission or approval operation was performed by the reviewer. Only review-local files, fixed Git reads, existing artifact hashing and finite Python controls were used. Export discovery initially tried two nonexistent supporting paths; those read failures produced no source changes and were corrected using the fixed Git tree. The final export manifest covers 15 existing files, including all five changed files and shipping approval/lifecycle wiring.

## Open finding

**NP87-01 — P2 — U+061C remains an invisible active bidi control in displayed review rows.**

- Paths: `apps/desktop/src-tauri/src/native_parent.rs:43`–`:54`; `scripts/native-owner-dialog-smoke.py:180`–`:184` in the fixed export.
- Trigger: a reviewed value contains U+061C ARABIC LETTER MARK, e.g. `recipient: abc<U+061C>123`. This is a Unicode Bidi_Control character, separate from ordinary Arabic/RTL text.
- Evidence: the Rust explicit ranges include the other bidi controls but not U+061C; its fallback serializes the scalar with serde_json rather than visibly escaping this character. The independently executed Python mirror emits raw U+061C and no visible `\u061c`; reverse decoding still restores the original bytes. The independent requirement test over all 12 Unicode Bidi_Control scalars **fails only for U+061C**, exit **1**, in `boundary.stderr`. `bidi-control-repro.json` preserves the smaller reproduction without rendering the invisible character in the report.
- Impact: byte losslessness alone does not meet this change's promise that bidi controls become visible escapes. A control-bearing value can still influence visual ordering inside the displayed JSON row. This is a source/mirror-level result; no macOS exploit or actual rendered reordering was demonstrated, and the original exact-text/visibility guards remain intact.
- Required repair: explicitly escape U+061C in both implementations, preserve ordinary Chinese/English/emoji/RTL characters, and add production plus mirror regression coverage for all 12 Bidi_Control scalars with exact reverse-byte reconstruction. Re-review a new fixed revision; this frozen 87 report keeps the finding open.

Root's requested finite separator check is **not another finding**: both implementations explicitly escape U+2028/U+2029 through their U+2028..U+202E range. `left<separator>right` independently contains visible `\u2028`/`\u2029`, no raw separator, and reverses exactly. The second independent boundary test passes; `separator-controls.json` records both vectors. No generalized Unicode framework is requested.

## Shipping wiring and consent boundary

`lib.rs:43` routes the real ordinary approval through `native_parent::with_caller` and `parent.confirm`, followed by `parent.finalize`. The Goal/graph/node approval and decline callers similarly use that same CallerOwner (`lib.rs:629`, `:660`, `:692`). `native_parent.rs:318` now splits every supplied message before invoking the original per-sheet caller-owned path. This is not a smoke-only formatter.

The new representation retains the complete original UTF-8 message, original byte ranges, ordered JSON-string rows with non-whitespace `L` prefixes, at most 40 encoded UTF-8 bytes per row and eight rows per content page, followed by a separate Final display confirmation. It checks reverse decoding before opening the first page. Chinese and emoji remain readable rather than being converted wholesale to ASCII escapes; new page instructions are English and the original bilingual warning/message remains byte-preserved. This does not certify full Chinese-localized product UX.

`present_complete` uses ordered short-circuiting; a false response on any content page or Final stops immediately. Only all true responses return to the existing approval formatter. The original `approval.rs` is byte-identical to baseline: the target proposal ID/revision/idempotency key, exact finalize/decline frame construction, compact/full-field rules, 16KiB limits and 1MiB graph snapshot limit have not changed. The added presentation envelope is independently bounded at 1MiB+32KiB and never truncates the message to fit.

The existing caller/main-thread/parent validity checks, review startup timeout, native response callback, close invalidation, finalization linearization and unknown-transport reporting are retained in `confirm_page`. Each page creates its own response channel/start flag, invokes the real plugin dialog and rechecks the caller. There is no new renderer approval flag or permission endpoint. Closing before private finalization still denies dispatch; a close after the existing finalization linearization is not relabeled as definitely not started. Existing Rust lifecycle tests were retained but not independently executed here.

## Real-controller/host changes

The debug-only macOS fixture observes each exact expected presentation page, page index/count, same proposal/revision and actual attached parent. Full-cell fit, complete visibility, exact title/message uniqueness, positive window identity, enabled/unique buttons and on-screen containment remain required. It does not trim, normalize, partially compare or accept clipped text.

For each page it waits for the existing 500ms stabilization, joins the owned screenshot worker, writes a per-page ready record, and requires the host's per-page database permit. Permit identity includes phase, index/count, proposal/revision and original before-click sequence. The host verifies page order and the unchanged pending database before every permit. A permit from another page/phase/proposal/revision/sequence is refused. The final Cancel phase uses the real Cancel button only on Final after earlier real OKs; the approve phase requires the prior genuine Cancel denial and then real OK on every page including Final. There is no direct harness finalize of the target Grant or manufactured native callback.

The controller marks progress before `performClick`, blocks reuse of the previous sheet number, re-observes text/parent/layout while awaiting capture/permit, and resets per-page capture/stability state. These are static observations. Whether AppKit recycles sheet window numbers, actual page timing/geometry and new presentation success require a new Mac run; this reviewer does not infer success from offline bounds. Failure reports/captures remain diagnostic-only and unknown/nonzero outcomes stay failures.

The original **75s** native harness deadline, **85s** host deadline, **4s** screenshot deadline and **500ms** sheet stabilization are unchanged. No threshold was increased to fit the extra pages. All original Python test bodies remain byte-identical; only four PresentationControls were appended. Locks, Native manifest, shipping lib wiring, approval formatter and Core/schema/CI authority are unchanged by this five-file delta.

## Independently executed checks

Actual commands, exit codes and full stdout/stderr are in `commands.json`; executable review scripts are retained.

1. Four fixed-source `PresentationControls`: **4 passed**, 1.956s. They cover UTF-8/indent/control reconstruction, canonical Rust/Python vector, unchanged large-message bounds and foreign/reordered/partial/offscreen page refusal.
2. Eleven additional deterministic vectors: passed exact UTF-8 reconstruction, encoded row/page bounds and mixed Chinese/emoji/control content. This exercises the Python mirror, not compiled Rust or native GUI.
3. Two independent security-boundary tests: **1 passed / 1 failed**, exit **1**. Separator preservation passes; the all-12-bidi-control test fails for U+061C, preserving NP87-01.
4. Evidence/source verification: exit **0**. All 15 exported files match fixed Git; all **104** author source-map entries match fixed 87; seven author evidence files match the author manifest; unchanged source/lock comparisons pass. Existing frozen `40322bff82aa8ec0f1c019e5d723f42694a6266bcaa27b153f535d0d21dbadcd` daemon file was hashed without execution.
5. `git diff --check e3ccf86a8b12e7ed1371074de095bb454c0b62aa 87ec9ad8647b549311fc90077e9757bc257c4b70`: exit **0**.

Author evidence, kept separate: `python-utf8-final.log` records **19 tests passed, 6.169s**, including the original three real-Core/bootstrap/SQLite controls with the frozen schema12 daemon. Those controls do not execute the new native presentation. The first attempt retained three errors due to the missing explicit fixture-daemon environment variable; intermediate ASCII presentation and final UTF-8 logs are preserved separately. Author syntax/format/py_compile claims are not relabeled as reviewer-run Rust typechecks. Expected **61 lib +8 controller** Rust tests and actual AppKit presentation remain **not run by this reviewer**; Root's concurrent compilation lease is not borrowed as a result for this report.

## Original actual Mac evidence and remaining validation

Both original e3cc artifacts were re-read locally, without downloads or mutation. Their actual native body equals its expected message only after removing each line's leading spaces/tabs: **252 UTF-8 bytes** difference, independently recomputed. ARM expected/actual are **3132/2880** bytes; Intel **3136/2884**. Each original body hash matches its raw saved string; expected message equals the saved fixture. Both first fail exact full title/message uniqueness before any click; both separately record `on_screen=false`, sheet **520×1329**. The old exact check remains failed; whitespace normalization is not adopted.

Using each actual expected body, the new Python representation yields **12 prompts including Final** and exact reverse reconstruction with short bounded pages. This is offline format evidence only. It does not establish the cause of AppKit's original whitespace handling, cure original clipping experimentally, prove new Cancel/OK/finalization, produce an after-DB result, or deliver a package. Original Mac reports/manifests and both raw diagnostic/fixture pairs are hash-bound in this review's evidence/provenance.

New dual-Mac actual page text/cell/onscreen/capture/DB-before-permit/real-click evidence, independent image review, lifecycle failure controls, corrected U+061C source tests and compiled shipping Native checks remain required before claiming this presentation fix verified. No Factory schema13, real Provider credentials/model call, package/signing, broader UX completion or formal owner acceptance is asserted.
