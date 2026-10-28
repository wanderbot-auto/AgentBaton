# Compile erratum — e942d56

Root actual desktop compilation failed with **E0433 / exit101**, before any tests executed. Exact command/result/log hash are preserved in e942-actual-compile-results.json and e942-actual-compile-failure.log.

I missed the cfg directly above Tauri lib.rs193: `#[cfg(all(feature = "wry", target_os = "android"))]`. Therefore `tauri::wry` cannot be used by the desktop parent adapter. The earlier static claim that this reexport avoids a direct dependency is withdrawn; e942 is compile-blocked. Wry itself reexporting raw_window_handle does not make the Android-gated Tauri path available on desktop.

Original REVIEW.md and VERIFICATION.json remain complete and byte-identical, with their exact hashes in CURRENT_REVIEW_STATUS.json; they are superseded on this feasibility point, not silently corrected. The real compiler failure is retained. Lifecycle analysis remains source-only and cannot be used as a compile/runtime pass.

A direct dependency on the existing locked raw-window-handle0.6.2 and direct import is the narrow repair candidate authorized by Root. It requires a new immutable source revision and actual compilation/tests. No dependency version/checksum change, Mac dialog proof or approval permission follows from this repair. Reviewer did not run Cargo or modify product files.
