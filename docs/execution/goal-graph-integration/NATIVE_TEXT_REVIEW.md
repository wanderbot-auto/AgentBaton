# Independent review of fixed native sheet diagnostics

Reviewed 2026-10-28 UTC. Candidate `7a13cddef4316d520361071ff5e2a263ea058580`, tree `c8f099d3f0285ddb07507f59361f4d18c33320c8`, baseline `0aa20c968d493cad895e4a3853bdf106d6b54acf`. Read only fixed Git source, saved dual-mac failure evidence, installed bindings and Root's exact Linux lease result. This reviewer did not author or change candidate source, run Cargo, invoke a daemon/model, mutate CI, approve or merge. Author's later documentation is outside this fixed-source review.

No mandatory source correction was found. This is a bounded, test-only evidence improvement for an unresolved observed failure, not proof that the Owner flow succeeds or that its root cause is known. Actual macOS conditional code compilation and new diagnostic/screenshot execution remain not_run.

## Scope and preservation

Exactly four files change: test-only macos.rs, pure controller.rs, standalone Native Cargo.toml and its Cargo.lock. Optional sha2 is enabled only by native-smoke; lock changes only add the existing locked sha2 package as a direct desktop dependency, no package/version/checksum change. Root Cargo.lock is unchanged. Native bins still require native-smoke. Production lib.rs, approval.rs, native_parent.rs, original native_smoke.rs and both Python runners are byte-identical to baseline; no new renderer command, external driver, credential or permission route is added.

Independent `check_fixed.py` compares fixed Git blobs and saves before/candidate source. All 16 reported mechanical assertions pass, including composite assertions: controller.validate and original four tests exact; write/invoke_script/report/contains/view_visible exact; OwnedCapture and capture exact; caller/parent/visibility/same-sheet500ms prefix exact; title/message exact equality and complete measured cell visibility gate exact; ready/permitted exact proposal-revision-sequence to real button.performClick path exact. record_report and run differ only by initializing observation_failure=None. Original 4-second capture, 75-second watchdog, 100-ms polling and Python85-second outer deadline remain unchanged. The 4-second child capture owns kill/wait on timeout/drop as before.

## Diagnostic bounds and failure behavior

Candidate macos.rs:215–338 adds a separate diagnostic object after live framework caller, attached-sheet parent identity, visible sheet and stable observed window checks. It records phase/stage, same window number and parent number, frames, bounds/visible rectangles, hidden/visibility/window relations, each actual view class, full NSTextField string with UTF-16 units, UTF-8 bytes and SHA-256, match counts and button metadata. Expected title/message are separately full hashed text. Nonmatching actual text is recorded as text, not accepted as a partial match; the original title/message vectors still require each exactly once.

Traversal rejects over256 visited/queued views. Class source bytes over256 reject before conversion. NSTextField conversion first rejects over32768 UTF-16 units to bound preallocation, then diagnostic_text checks exact UTF-8 field size32768 and accumulated actual field text65536 with checked_add. Over-limit/overflow returns an error without truncating text, advancing accumulated budget, or reaching original acceptance. Buttons have both256-unit preflight and256-UTF8-byte bounds. Expected title/message use independent bounded fields; actual aggregate counts actual field strings. Output includes finite structural overhead, expected fields and bounded button/class metadata as well as the65536-byte actual field-text budget; it is not claiming the whole JSON file is <=65536 bytes. These are evidence limits and add refusal conditions, never authority.

In poll's error branch, the earliest observation rejection is placed in phase.observation_failure and saved before capture. poll immediately diverts into poll_failure on later iterations; it does not publish ready or request a database permit or click. Secondary capture/write/join failures are annotations; fail chooses the original observation reason even if watchdog/other reporting fails later. When pre-parent/visibility checks cannot verify a positive own-sheet window number, no screenshot is started. Failure remains failed; no fallback text, modal return, Core finalize or successful OS receipt is synthesized.

This would retain the raw observations missing from both actual0aa Mac failures, which previously stopped at `Exact full native title/message not uniquely present` before first Cancel click. The candidate has not yet produced such observations on actual macOS. Previous native task screenshots do not depict this new diagnostic sheet.

## Thread, process and capture ownership

Only start_capture spawns a capture thread. Under the phase mutex it creates a result slot/JoinHandle only if capture is None. A normal in-progress/finished capture is reused on later observation failure, never overwritten by a second worker. Capture takes only the number observed from the exact caller-attached sheet; no whole-desktop capture, foreign-window enumeration or OS permissions change is added.

Capture completion publishes its result before poll can act. Normal success takes/joins the worker before publishing ready and before clicking. Failure polling joins a completed worker before returning its retained reason. A normal capture error returns to fail but the remaining JoinHandle is taken/joined in the unchanged run epilogue; watchdog exit likewise joins the poller then any capture worker. Before phase reset to approve, a permitted click can only have occurred after normal worker join, so no previous-phase worker is lost by resetting Phase. OwnedCapture::Drop and the actual screencapture timeout still kill/wait their owned child; no process/thread is deliberately detached.

No new deadline extension is present. A capture operation is bounded by its original4-second deadline, and existing run epilogue joins remain real joins rather than fabricated completion. Filesystem IO/main-thread scheduling are not newly claimed hard real-time bounds.

## Installed API and borrow review

`installed-api-proof.json` retains exact installed file hashes and source-line excerpts, not guessed framework APIs. Locked objc2 is0.6.5, AppKit/Foundation0.3.2. AnyObject::class(&self)->&'static AnyClass exists; AnyClass::name(&self)->&CStr exists, so the class name borrow lives through to_bytes/to_string_lossy and JSON serialization. NSString::len_utf16(&self)->usize exists without an additional gate on that method, and NSString is already enabled. NSControl::stringValue and NSButton::title return Retained<NSString>, so preflight and conversion refer to an owned retained value. NSView window/subviews/hidden/frame/bounds/visibleRect APIs and geometry feature are present. The queue parent index infers usize from Some(index); JSON construction borrows serializable strings/vectors rather than consuming later button controls. Mutable diagnostic/value borrows are sequential; no new aliasing unsafe code was added.

This static inspection found no obvious signature or borrow mismatch. Linux cfg excludes macos.rs and cannot establish actual Apple-target compilation; actual Mac CI remains necessary.

## Actual checks and evidence limits

Reviewer command `python3 /workspace/.onboarding/owner-native-text-diagnostics-independent-review/check_fixed.py` passed fixed-Git mechanical comparisons. It executes no product code or Cargo. It is source evidence, not runtime verification.

Root's copied `root-lease-proof/RESULT.json`, before.json and controller-tests.log bind the same four source SHA-256 values before/after and exact command:

`cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --bin agentbaton-native-owner-dialog-smoke`

Root executed under its sole Native Cargo lease: exit0, **6 passed, 0 failed, 0 ignored** (original4 + new2), build26.17s. Log SHA-256 `05a3613d05059b8c549c781e19900c067b2620cb562862f9053658d2250d8adf` is independently matched to retained bytes. New tests exercise actual SHA-256/UTF-8 length, exact per-field/aggregate boundaries, overflow, unchanged rejected budget and inability to treat diagnostic JSON as click authority. They do not execute AppKit traversal or screenshot threads. Root recorded old compile-only dist/5541 sidecar resources as macro assets, never executed them, removed exact own temporary links and returned lease to Core; this review did not reuse a lease or run tests.

No macOS new-code compilation, real sheet diagnostics/PNG, actual Cancel/OK rerun, default release-binary audit/package or real Provider model evidence is supplied by this candidate. Prior PR16 actual readonly/WebCrypto successes remain valid separately; both actual Owner failures remain preserved. No formal approval is issued by this review.
