# Progress

Frozen source `9c3850d40d78d2317142f604043fd0aaaceb2aa7` moves `controller.rs` and `macos.rs` from `src/bin/native_owner_dialog/` to `src/native_owner_dialog_test/`, with100% byte identity. The smoke entry changes only its two `#[path]` attributes. No other source changed.

Pinned Tauri CLI2.12.4 resolves to official source commit993038c49185307dfc895ab7657d77ca0d5ed7c7. `get_binaries` at rust.rs928–1014 scans every entry of `src/bin` by file stem. It excludes the two explicitly feature-gated smoke entries, but treats their helper directory as another binary despite Cargo metadata declaring no such target. The source-derived project default list changes from `[agentbaton-desktop,native_owner_dialog]` to `[agentbaton-desktop]`; the metadata targets remain byte-identical before/after.

The installed pinned CLI reproduces the old missing-helper failure on generated fixture input with the same two feature-gated smoke declarations. Moving the fixture helper outside `src/bin` yields exit0; pure ar/tar inspection finds only the main binary in its tiny generated Debian package. These inert fixture bytes were never executed or installed and are not an AgentBaton product package. The original ARM release failure is losslessly preserved.

24 finite checks and cumulative source diff validation pass. Eight protected files, including release audit and manifests/locks, are unchanged. Rust tests/lint, actual Core, macOS bundle/GUI and new CI remain not_run; Root owns those later windows. Independent review remains pending.
