# Independent bounded large-preview fixture review

Verdict: **PASS for the narrow test-only preparation-observation correction, with stronger RPC responsiveness checks; no blocking finding remains.** Suitable for integration and fresh exact-head CI. Reviewed0db994786b3e70e21b7c0c57a15cf210fd9e9fdf/docsd1c29611b63588ebb9f967222b5e9baf2b779125 againstde150, independently by `/root/data_native_integration_review`. Source SHA17178a8c943fa0c5225bb4fd543a8ef50281808458da031b59fe5ea5c54043dd. This is not new Mac/native or full-product acceptance.

## Boundary review

Only the one32MiB cancellation test's preview **preparation observer** changes from the generic5-second wait to an explicit35-second absolute deadline measured **before the start RPC**. The product's30-second I/O budget is unchanged; the extra5seconds is a finite test observation margin. OrdinaryFixture.wait5, copy/cancel/join wait5, serverRPC3/clientordinary5,32MiB size, source count/limits and all production code/workflows/locks remain byte-identical. The increase is explicitly disclosed, not described as all fixturetime literals unchanged.

The absolute clock assertion executes at the top of each observation loop **before checking/breaking on a terminal result**. It therefore cannot accept a late verified result merely because the last poll began before35seconds or returned quickly. Preparing is captured beforestart, and the sameabsolute35 bound includes bothparallelreads and everylaterpoll. Everyterminal result must still satisfy the original unconditional state=verified assertion. A failed/deadline/cancelled/unknown terminal cannot become passingpreparation. No forced35-second expiredclock control was run; this ordering and identity were reviewed directly, and actual successful timing is separately measured below.

New assertions require start<3seconds, two actual parallel system.snapshot/preview.get calls each<3seconds, and every subsequent poll<3seconds. This adds responsiveness evidence while the worker operates, rather than allowing a blocking actor to pass through a larger completion window. It does not claim OS/kernel hard interruption or scheduler guarantees. Preview types do not have creation io_completion; diagnostic null there is not a missing copy completion receipt.

## Source and acceptance preservation

Replacing only the target function body with exactbaseline bytes restores the entire original file byte-for-byte. All21 testnames and20other testfunctions are retained. The original tail from verified through proposal/nativehostfixtureapproval/cancel/5-secondjoinwait, strict worker_joined, cancelled/Ready branch constraints, nullcontract/cancelerror semantics, exactoldkeyreplay and original33554432byte source-size assertion is byte-identical after removing only the new diagnostic timer/print. Source32MiB is not reduced and no assertion is deleted or replaced with a genericfalse check. No product budget or authority is enlarged.

Historical de150 ARM and Intel logs both genuinely show previewchecking/rev1/snapshotnull/errornull at the old5-second observer; copy/cancel/join was not reached. Both original raw hashes remain unchanged. The old unmodified case passed in the author's Linux run. Consequently local success neither reproduces nor identifies the historical Mac reason; hashing speed/load/kernel failure is not inferred.

## Independent actual verification

Using only the parent's handed-off Native target as a **Core test cache** (not a Native GUI proof), incremental0/jobs2/CIserial1/umask022. Core/workspaces entrypoints received mtime-only refresh with bytes checked unchanged to avoid stale cross-worktree metadata.

- `cargo test --locked --offline -p agentbaton-core --test data_workspaces --test data_workspace_migration -- --nocapture --test-threads=1`: **21 Data +3 migration tests passed**, zero failed/ignored. One real process-exit nested child overlaps an existing case and is not a25th unique test. Raw log SHA `e129ac9f1f23d44ee60093302461870113f908471f794d9aba1ca0c85efb8544`.
- All-target Core Clippy -Dwarnings, package fmt and exact source diffcheck passed. Clippy SHA `9bda7e5311efb18adbe62d5108cc51a9965391b23e275041d07b5399bb902801`.
- Actual start13ms; parallel system.snapshot4ms/preview.get5ms with statechecking; preparation2244ms→verified/rev2; copy phase1060ms (includingapproval/request)→cancelled/worker_joined. The unchanged5-second copyjoin path and all original receipt/Ready/nullcontract/replay assertions really executed.
- All12author manifest artifacts and both historicalMac rawlogs were independently hash-verified. Author manifest SHAe7fa39c1465ab632a4fad2fdd8aedf2d4e50f0087473415dfc4040961e6a414c.

## Handoff

Native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target` is formally **RELEASED**, with no further Cargo writer from this review. Main/private targets were not accessed. Source/index remain clean. All new evidence persists under `/workspace/.onboarding/data-preview-observation-review/`, manifest SHA `3197e4f06d5ea400c0208a95700d9932962ee9d416211b30047c6e7a655eeb66`. No source/index/remote/CItrigger/cancel/retry/account/model/signing action was performed.

This correction preserves actualde150 failures and does not turn them green. Fresh exact7, realMac Corecase, Native22+12/Goal3+DB/Command3/Data3/lifecycle/newsidebarimages/defaultreleaseaudits and realdualDMGs remain necessary. Linux DataUI8/cancelledjoined-before-reservation and full64displaycallback evidence cannot substitute for OSOwnerconfirmation, in-copy partialstop, actualCore64copy, liveProvider, minimumOS/userinstall or wholeDesign130 acceptance.
