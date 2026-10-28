# Evidence

Baseline `f094d217f6ca62e1b753e6d6a13ddc43021487ea`. Locked feasibility source `/workspace/.onboarding/goal-native-owner-dialog-feasibility/REPORT.md` was read; that report itself is source-only, not OS proof. Tauri 2.12.5, dialog plugin 2.8.3, Wry 0.57.1, rfd 0.16.1 and raw-window-handle 0.6.2 were inspected directly in the existing Cargo registry.

Actual API evidence: WebviewWindow CommandArg reads command.message.webview().window(), rather than JSON fields; Initial inspection incorrectly treated tauri reexports wry as desktop-available: Root actual compiler E0433 exposed its Android-only cfg. Wry itself reexports raw_window_handle, but this is unavailable through desktop tauri; corrected to a direct exact-pinned raw-window-handle dependency after Root approval. Plugin.parent ignores failed handle queries, so the new adapter supplies the already-checked pair. Plugin show constructs AsyncMessageDialog inside app.run_on_main_thread; Wry runs a nested main-thread task inline. rfd's parented async branch retains NSWindow from the AppKit NSView before ModalFuture starts the sheet. The old parentless branch uses CFUserNotification. Builder.on_window_event is installed before event processing and avoids per-operation asynchronous listener registration.

Commands actually executed in this tree:

- `RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 --check apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/native_parent.rs`: exit 0 (syntax/format only).
- `git diff --check`: exit 0.
- Python comparison against git-show baseline: original entire `#[cfg(test)] mod tests` suffix in lib is byte-identical; approval.rs byte-identical; exactly eleven fixed command signatures take framework WebviewWindow; six private wrappers and six parent.finalize adapters; shared builder has one global lifecycle hook; no blocking_show remains in lib. Exit 0. Ten new unit bodies counted; not executed.

Current source SHA-256:

- lib.rs `b1889f27230fa1612fd4fac3fa5210d15dbc54ac3506dab7b8b8637dc6c28c0d`
- native_parent.rs `f64da3d0b31ddfe3bc07e5387d3a4414cd379bba4b61343dd37df0e50b783683`
- unchanged approval.rs `ed15fe18bca38f102c41f77cd87c31b2a644f23201652d367670465ebc7bb80a`

Not run: Rust compilation/tests/lints, macOS build, actual NSAlert/Owner buttons, close/destroy on a real GUI, native integration smoke or Core approval dispatch. Earlier 43-test evidence applies to the old approval/lib source, not this revision. No account, Provider/model, production secret, fee, signing or release activity.

Root actual composition first compile failed exit 101/E0433 for tauri::wry on desktop; original log is `/workspace/.onboarding/native-approval-parent-composition/native-tests.log`, old e942 source retained in git. Source feasibility missed the cfg immediately above lib.rs193. Corrected native_parent import to direct raw_window_handle. No Cargo was run by this agent. Python TOML comparison confirmed exactly one pre-existing raw-window-handle version (0.6.2), and only agentbaton-desktop.dependencies gained the edge; all package/version/checksum metadata identical. Direct rustfmt and diff check passed after correction. Manifest SHA23816032a352ef88077fbf77e42bf445da4df1b8ca6822afb5ca7d5a6c2e785d; lock SHAe134ac4c4ff071aecc03a51dfb6522e782def7a52cbf3252d8c9c19226cb9424. Await Root recompile; no green Rust claim.
