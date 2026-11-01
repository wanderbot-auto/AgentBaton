# Current schema guards in command and data-workspace tests

Baseline: `9549441a0de2b84c57e2302191ca96017b2d73d1`; AB-START-001, Design v0.2, requirement `AB-SEC-007`.

1. Search current Rust/Python/Tauri workflow paths for exact schema16 remnants and classify every potentially relevant guard before editing.
2. Correct only the two verified fresh/restarted-current Store header assertions to typed schema17, including the command fixture mismatch message.
3. Independently review whole-file inverse proofs and preserved historical/negative controls.
4. Root runs complete `command_validation` and `data_workspaces` targets and retains source-bound results, then exact-candidate product checks.

No product behavior, test skipping, timeout, budget or safety threshold changes.
