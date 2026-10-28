# Independent static review — production Native approval parent

No new blocking static code finding was found in `e942d566baa6647fee98ba244b3a2c24b4f0309b` versus baseline `f094d217f6ca62e1b753e6d6a13ddc43021487ea`. **Compilation, tests and actual macOS approval/lifecycle acceptance remain pending.** Reviewer did not run Cargo or use any target, edit product/CI/JS/permissions/locks, or invoke an approval. This is separate from earlier IO and Native evidence.

Exact source SHA-256: lib.rs `b1889f27230fa1612fd4fac3fa5210d15dbc54ac3506dab7b8b8637dc6c28c0d`; native_parent.rs `4cf7125cd41c9dcbca816cb6e8de75e8d353838784c504a524cbce159f9b71e3`; unchanged approval.rs `ed15fe18bca38f102c41f77cd87c31b2a644f23201652d367670465ebc7bb80a`. Immutable copies and cached dependency hashes are in source-binding.json.

## Caller, handles and actual backend path

All11 fixed approval commands inject Tauri WebviewWindow, obtained by the framework CommandArg from the actual invoking webview window (Tauri webview_window.rs1606–1617). The renderer does not provide a label or handle. The label used by Slot is only a mutual-exclusion/invalidation key; no lookup selects a replacement parent. Six with_caller wrappers validate the real visible caller before initial Core proposal.

native_parent.rs142–190 acquires both borrowed handles fallibly on the main thread and requires AppKit window onMac. CheckedParent returns the exact stored successful pair. Plugin.parent(lib.rs303–317) therefore takes its success branch and storesSome, eliminating its ordinary silent HandleError→None route. It does not re-query the caller.

The real plugin.show runs from the main-thread closure(native_parent.rs248–286). Cached Wry send_user_message(lib.rs263–282) executes nested main-thread Task inline. Plugin desktop.show_message_dialog215–255 constructs rfd AsyncMessageDialog there; rfd message_dialog171–181 uses parentSome, utils34–46 derives and retains the original NSWindow from that NSView, and ModalFuture38–105 constructs the actual AppKit sheet. This supports the intended production parented route, rather than the previous parentless CF notification or a feature-only replacement. No extra dependency was added; cached Tauri/Wry reexports the locked raw_window_handle crate.

This is source-supported behavior, **not a Mac observation**. Real NSWindow retention, sheet attachment/visibility, close/destroy timing and AppKit completion still need Mac compilation and execution.

## Lifecycle and waits

Main-thread validation/actual dialog construction is separated from off-main spawn_blocking waiting. check() and queued startup have5s bounds; expiry invalidates later queued callbacks. The per-page review_started flag is monotonic, preventing a genuine completed human response from being mistaken for a startup timeout after lifecycle returns idle. There is intentionally no5s limit on human review. A fatal/stalledGUI after review begins is not proven bounded by that startup deadline; real teardown/callback tests remain necessary.

Close/destroy before dispatch invalidates consent, with checks before each page and the final private gate. The final main-thread CAS toSTARTED is the declared linearization boundary(native_parent.rs317–328). Ordinary close after it is deferred, and Drop marksFINISHED then completes the deferred close. Forced destruction afterSTARTED preserves the actual Core result/transport uncertainty; finish does not relabel it not_started. There is no atomic transaction spanning that GUI check and Core RPC, and this code does not claim one.

BeforeSTARTED, NATIVE_PARENT_UNAVAILABLE/not_started means no privatefinalize authority was dispatched. It does **not** prove no pending Core proposal was created or that a lost proposal reply was rolled back. Original request keys and pending-record reconciliation still apply. Window closure cannot revoke a possibly committed Core action.

One shared Builder.on_window_event hook routes framework events; there are no asynchronous per-operation listeners. SlotDrop removes the weak entry. Stale/reused labels may conservatively invalidate/fail closed, but cannot select a foreign parent or approve.

## Preserved approval rules and actual review checks

Independently compared oldlib entire test suffix and approval.rs wholefile: byte-identical. Old16KiB kinds, complete1MiB new-kind review, fullsnapshot/CAS/origin checks, fixed privatefinalize/decline frames and realCancel0finalize behavior are not weakened. Parent.confirm forwards the unchanged message/title into the genuine pluginOkCancel callback; it does not manufacture a confirmation result. Seven originallib plus36approval testdefinitions remain43; ten newstate bodies are present but **none were compiled or run by this review**. Cargo.toml and lock are unchanged. Git diff --check passed. SOURCE_CHECK_NOTES records two reviewer counting-regex corrections; they were not product/Rust failures.

## Required next evidence

Under an explicit target lease, compile production/default and native-smoke, run the preserved43 plus new10state tests and existing harness, fmt/Clippy. Then actualMac must prove: exact caller sheet and no parentless fallback on handle failure; human review beyond5s; queued startup expiry; realCancel with unconsumed proposal/0finalize; close before/during/betweenpages; postgate deferredclose and forcedestroy preserving actual Core result; teardown and retained parent/callback behavior. A future host-owned OSdialog driver must use real buttons/completion and original production path; it is not implemented here.

No actualCore12 run, NSAlert button/screenshot, native Owner review, Mac desktop validation, CI or completeGraph/product acceptance is claimed. EarlierIO41 or oldNative65 passes cannot be borrowed for this newlib. The branch’s Core11 baseline is not the active Graph runtime.
