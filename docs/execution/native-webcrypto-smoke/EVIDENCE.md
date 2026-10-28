# Evidence

Source fingerprints:

- `apps/desktop/src-tauri/src/bin/native_smoke.rs` SHA256 `93777843bb2d5caf7eac5e42a569a859d5f04153896d0e53c1baee771d370d82`
- `scripts/native-run.py` SHA256 `8fa23e107fe92542b95fc5d398aa38a20236d29b2e3fbbe7eb87b92efb0197c1`
- `apps/desktop/src-tauri/tests/test_native_runner.py` SHA256 `50433be6bd43681342d5e247bc0386561ce5f4c070684824c1ca92dffb217b2e`

Actually executed in this isolated tree:

- `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p test_native_runner.py -v`:exit0,29 tests passed,0 failures/errors/skips,2.608s. Log `/workspace/.onboarding/native-webcrypto-smoke/python-tests.log`.
- `RUSTUP_HOME=/workspace/.toolchains/rustup /workspace/.toolchains/cargo/bin/rustfmt --edition 2021 --check apps/desktop/src-tauri/src/bin/native_smoke.rs`:exit0, syntax/format only.
- `python3 -m py_compile scripts/native-run.py apps/desktop/src-tauri/tests/test_native_runner.py`:exit0.
- `git diff --check`:exit0.
- Node v24.19.0 `new Function` parsed extracted actual FIRST/SECOND scripts; native Node globalThis.crypto.subtle SHA256 independently processed109 original UTF8 bytes: d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892. JSON.parse/stringify numeric rounding changed the text and digest to0c68e740a81ddcf1832d9db0a9fdc5a426dfd4a08b52cee2708b728f887e0cea. Node log explicitly labels NOT Native WebView/macOS; no global crypto monkeypatch used. `/workspace/.onboarding/native-webcrypto-smoke/node-vector.log`.
- Python source comparison to652c5f0: entire original22 Rust test-module prefix,25 Python test-method bodies, all three readonly SQL literals identical. Removing only new crypto instrumentation restores full original FIRST/SECOND bytes. Product lib/approval/parent/Cargo manifests/lock/macOS runner script unchanged. `/workspace/.onboarding/native-webcrypto-smoke/preservation.json`.

One inspector attempt failed ValueError substring-not-found because it assumed mod tests instead of actual mod fixture_tests. Corrected inspector only, no source/tests weakened; retained `preservation-first-attempt.log`. No product or test failure was hidden. Independent evidence hashes are in `.onboarding/native-webcrypto-smoke/manifest.json`.

New negative controls reject missing crypto/proof fields, false/string/integer availability, boolean version, wrong algorithm/hash/input/encoding/source, integer-type forgery, extra fields, missing/swapped/replayed checkpoint tags, using first proof as reopened proof, and attempts to replace existing database/sequence checks with the known digest. Four Rust bodies cover corresponding typed/order/reopened controls but are not executed here.

NOT RUN: Cargo compile/tests/Clippy, actual Linux Native WebView digest, macOS Intel/Apple Silicon WebCrypto/NSAlert/GUI, real Provider/model calls. This only adds fixed-vector platform capability evidence; it does not claim real Graph execution/OSOwner acceptance or infer that macOS lacks crypto. No production fallback/credential/account/fee/permission/signing/release change.
