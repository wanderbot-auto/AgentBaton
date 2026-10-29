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

## Post-b01c message-only clarification

Root actual composed b01c source passed 53 library tests (including ten new lifecycle tests), 22 Native harness tests, Clippy with -D warnings, and fmt. Read exact results `/workspace/.onboarding/native-approval-parent-composition-b01c/results.json` (SHA-256 `168caaf635e5213226a75e48b48d1b9a268ef2ae5e703c3f19bc3f486e6b350b`) and test log counts: 53 passed/0 failed/0 ignored; 22 passed/0 failed/0 ignored. Test log SHA `6507fe0e6da6abe13167815cad7c1db5535c6434816c1b93c553be50508c6ab9`; Clippy SHA `da74a76ed548aaa4f66d549070f3d19026a39321498c7775912bc087a9afa3a7`. This was Root's actual Core12/strict12 composition, not this branch's old Core baseline or a macOS dialog run. Original failed E0433 evidence remains.

Root then authorized only error-message clarification: this invocation did not dispatch private finalize/decline; pending proposals and previously committed history were not rolled back; verify the original idempotency key. Error shape, guard logic, all old/new tests, lib, approval and both Cargo files are byte-identical to tested b01c. Exact source comparison confirmed removing the single format-message line restores b01c native_parent.rs bytes. New native_parent.rs SHA `a4a0b2f9feba3848c2a9cd46510abb89e8da7d9f6c0b70dcc0344e8da545fc9f`. Direct rustfmt --check and git diff --check exit 0 after this message-only change; Cargo was not rerun for the new message. No tests are attributed to this new source revision beyond those checks.

Native cache lease is released to Core; no agent writer here. Pause this leaf after normal backup push. Actual macOS NSAlert presentation, real buttons, window-close races, OS driver and Core effects remain separate unrun acceptance work; Linux 75 tests do not establish them.
