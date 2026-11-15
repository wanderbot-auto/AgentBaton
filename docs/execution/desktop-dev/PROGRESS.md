# Progress

- Reproduced raw `npm run tauri -- dev` on Apple Silicon: tauri-build exits 1 because binaries/batond-aarch64-apple-darwin is absent. Vite itself ready in 157ms; original failure retained in EVIDENCE.
- Added debug sidecar preparation to beforeDevCommand, helper lifecycle supervisor and matching separate-workspace optimization overrides.
- Actual cold helper build and targeted verification in progress. No release or provider calls.

- 12 helper tests, 179 native library tests (2 existing ignores), frontend type/build and static checks pass. Independent review passed after four lifecycle/target/concurrency findings fixed. Real ARM64 native process/daemon IPC, restart and stop with no recorded process leftovers verified.

- Default-hook raw Tauri command also starts successfully (5.532s warm launch, explicit owned daemon); cleanup complete. Source 005c391e pushed; PR/CI next.
