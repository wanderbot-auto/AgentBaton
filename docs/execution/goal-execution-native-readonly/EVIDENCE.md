# Evidence

Source `776e02e1a61a82b676972b9d40c9317524984c56`; baseline `78b317a4dc42f088f752852bf04028eb0bb51d9c`. Partial requirements AB-SEC-002, AB-TSK-003/007/008, AB-REL-001/008. Persistent manifest `/workspace/.onboarding/goal-execution-native-readonly/verification.json` SHA-256 `ea41b5f406d37fa7102058a7379cd668da405df841845b1439475926e8d16f27` includes four source hashes, exact commands, environment, provenance and preservation hashes.

Actual commands from this worktree:

- `cargo test --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol`:28lib+17harness,0failed/ignored/filtered. `native-first.log` SHA-256 `3cab5db181a99d886d1dccab09d7a950b5aaf7e69348f595728d59fc75c6554f`.
- `cargo clippy --locked --offline --manifest-path apps/desktop/src-tauri/Cargo.toml --features native-smoke,custom-protocol --all-targets -- -D warnings`:PASS. `clippy.log` SHA-256 `8c6071c336ef84069072f1bda1c5dc435a7508e734f7546e4d8dd5bb387d51de`.
- `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml -- --check`:PASS.
- `python3 apps/desktop/src-tauri/tests/test_native_runner.py`:21tests PASS,2.509s; original17 bodies unchanged.
- `bash -n scripts/native-macos-smoke.sh` and `node --check` for each actual extracted FIRST/SECOND script:PASS; syntax only.
- Mechanical preservation: original Rust test module prefix/12 tests, old4-field functions/SQL/bounded query, all17Python functions unchanged; both scripts retain all old lines in order. `preservation.json` records hashes.

New real Linux tests observe schema11 and three zero execution tables, actual journal sequence, actual authenticated private daemon list `[]` and fixed missing-get `NOT_FOUND/not_started` without result. Actual schema10/12 and each table row mutation are rejected; malformed generated SQL metadata is a negative control, never an approved Goal. Phase/replay/type/missing/fake flags/crossinterval controls fail closed.

No positive-path command failed in this leaf; negative controls pass by requiring rejection. The exec-server interruption preceded Cargo. Frozen schema11 Linuxdebug9f833738 sidecars and existing UI dist are compile resources only; resource hashes in manifest. They do not prove Mac, G19 repair, release or signing. Entry-point mtime refresh preserved bytes.

Not run: new macOS Tauri WebView invokes, native OS Owner dialog, actual Goal execution, Provider/model calls or production delivery. New PR15 macOS CI is required; old Goal/DataPR14 checks cannot substitute. Native target released after checks; no Main target access.
