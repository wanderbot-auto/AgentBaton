# Progress

Implemented four real WebView async digest calls, test-only report witnesses, strict native/Python validators, ordered before/after witnesses, own first/reopened tags and additive failures. No Node crypto is installed or substituted into WebView. Existing three readonly SQL literals and entire FIRST/SECOND scripts restore byte-for-byte to baseline after removing only new crypto instrumentation. Entire original22 Rust test-module prefix and all original25 Python test method bodies remain byte-identical.

Actually passed:29 Python tests (original25+new4), Rustfmt syntax/format check, py_compile, git diff check, source preservation. FIRST/SECOND parse in Node24.19.0; native Node globalThis.crypto.subtle hashes the109 original UTF8 bytes to the known digest, and parse/stringify rounding produces a different digest. This is a pure Node comparison, not a WebView/macOS success.

Four new Rust unit tests written, not run. Native old22/new26 harness Rust tests, compilation/Clippy, actual Linux Native WebView and actual macOS ARM/Intel WebView remain not_run for this revision. Existing Root Product/Environment successes are separate and do not prove this new instrumentation or macOS WebCrypto support.
