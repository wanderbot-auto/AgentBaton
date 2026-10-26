# Independent session FIFO fixture review

Reviewer: native `/root/integration_review`, independent implementation context. Reviewed `3b94d1c..cf00c849e39977eb5bcbc83532866168da663d82`, tested source `5e4771de4992fe58bdcc612a583b9448f510ab32`, at 2026-10-26T20:55:30.374516+00:00. Only this review document was written; no sources, locks, earlier reviews, staging, commits or remote operations were changed.

Verdict: no unresolved blocker in this test-only correction. The production policy prefix before cfg(test) is byte-identical to 3b94d1c and matches its recorded digest. The real FIFO fixture now invokes fixed `/usr/bin/mkfifo` with literal `-m`, `600` and a direct path argument, asserts command success, then asserts actual FIFO type. Both gitdir/commondir cases and all original validation assertions remain. The killable nested child and original three-second bound are unchanged; no skip or looser deadline was introduced.

Independently executed under `umask 022` with the configured CARGO_HOME/RUSTUP_HOME/PATH and CARGO_BUILD_JOBS=2:

| Command | Actual result |
| --- | --- |
| `cargo test -p agentbaton-core --locked --lib policy::live_workspace_tests::live_writer_fifo_pointer_cannot_block_actor -- --exact` | PASS: one parent test and its actual nested child; the child is not counted as a second top-level test. |
| `cargo fmt --all -- --check` | PASS, exit 0. |

Verified the single source fingerprint and all **7 current log/evidence fingerprints** in EVIDENCE.json. The raw Apple Silicon job 121838837098 log actually reports compile E0425 for rustix mkfifoat; it is preserved as a failure. Actual corrected local core log has 71 passing result-line tests including one nested child, hence 70 top-level tests. The complete 193-workspace/35-UI suite was not rerun for this fixture-only delta; prior evidence is preserved at EVIDENCE.pre-first-ci-3b94.json and its recorded hash matches. Current clippy/core-test/manifest logs were observed and hash-verified, not independently repeated in full by this reviewer.

No fresh macOS/remote CI, native GUI, real vendor/account/model call, private identity, production action or additional paid service is claimed. Linux FIFO execution verifies the unchanged rejection behavior; the exact candidate still needs native CI. Full Design v0.2 remains incomplete.

| Reviewed bytes | SHA-256 |
| --- | --- |
| `crates/core/src/policy.rs` | `77a775f2befa7c70afa9ba5e8928ecb28d490da9de2c9bdef67b79469b4ca449` |
| Production policy prefix | `ee20ebba6daeeaf3c0e2883f532b83919d7add40ad3f5474c66bbde0131d58b5` |
| Current EVIDENCE.json | `d320f704bb6060f3358f2ca0b04c6be7658ce3e9c6ea79b5d0d1c33aa7eb2ae7` |
| Actual ARM failure log | `3dfe65fb33be4cdac79523d0b9a5d64d2559a50917fbe7566be5cac33c6f781a` |
