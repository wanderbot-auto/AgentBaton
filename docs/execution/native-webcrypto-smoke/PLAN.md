# Native WebView WebCrypto evidence

Isolated branch `codex/native-webcrypto-smoke`, baseline `652c5f0` of Root PR16. ACK OWNER001/002 and AB-START-001; narrow test contribution to AB-SEC-002 and AB-REL-001/008. This is platform acceptance instrumentation, not a production fallback, new authority or new product scope.

Only native_smoke.rs, native-run.py, test_native_runner.py and these four records. No production lib/approval/parent/UI/schema/lock/CI/timeout changes. Root/Core retain Cargo/Native target lease; do not compile or use targets without handoff.

Run actual window.crypto.subtle.digest(SHA-256) on TextEncoder UTF8 original constant text in first WebView before/after readonly interval, first completion and reopened WebView. The original JSON includes Chinese, emoji, inode2^60+3, u64=2^63+1 and u64 max as literal text, never parsed/stringified through Number. No substitute crypto/global mocks or fallback is used in the actual harness.

Rust validates the complete typed proof against exact algorithm/input/109byte length/digest/source/availability/tag; ordered native checkpoints reject missing/repeated/mixed contexts before preserving all existing readonly database checks and report commit. Reopened proof requires first-window evidence and its own tag. Python independently checks identical exact fields/types and four required contexts. Keep original schema12/zero Graph counts/sequence/lifecycle/cleanup checks, tests and assertions intact.

Verify Python controls, Node syntax and native NodeWebCrypto fixed-vector comparison, Rust formatting and source preservation. These are not actual Native/Mac evidence. Freeze for independent review; Root will schedule Cargo and actual Mac CI. Missing crypto/digest/typed evidence makes the actual harness fail rather than claiming unsupported/supported in advance.
