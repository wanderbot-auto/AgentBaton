# Evidence

Exact failure: PR #18 f55a805a952f84dc65e83c5e33f98e975871c3f1, Product 41143055493, Intel job 124361646237. 29 passed, 1 failed at rework observation exhaustion; daemon cleanup ok/joined. Finite artifact 13019844978: 4464 bytes, SHA-256 `8d177b06cb29ac9423eddd1998c0cd28a44d2285cec7deb3d4e17db34c12b04c`.

Sealed local diagnostic JSON SHA-256 `5a45b54b0e32946c3e577de661c399d9895bf4e5d7be5bbad9025e501c7d85ce`; diagnostic manifest SHA-256 `8044fb0abd77576d613061ae2d4d66ded8661cb13f1244c9be04678c07a9a442`. Local evidence directory: `/workspace/.onboarding/pr18-f55-intel-read-limit-independent-diagnosis`. These are local retained artifacts, not checked-in private runtime data.

Actual source-author checks:

- `rustfmt --edition 2021 --config skip_children=true` and `--check` on the two changed Rust files, using `/workspace/.toolchains/{cargo,rustup}`. The first default-home invocation failed because `/home/agent/.rustup` is read-only; rerun with the existing explicit toolchain homes succeeded. No toolchain was installed.
- Static Python extraction compared the f55 original post-request body with the helper after removing exactly eight indentation bytes: identical statements, comments and ordering. Normalized SHA-256 `cc2842e480048117e004e9451bb8ce771610de71f86c3e7effd10231f8a16ba1`.
- Original `handle_ready_request` byte-identical SHA-256 `6b6da775802fe1cc002cdbb0690e711ff7f73189a125fff5f8db406b6c5d8500`.
- Original `socket_worker` byte-identical SHA-256 `160c64786ae62a1fc8581f8441303af141b5ae71019a93c91574e35ed875d47a`.
- Original run shutdown byte-identical SHA-256 `b58a87f099416ac90f463452ddeef02a97995e05aa3a2a6d7ef765b8278b1673`.
- Static run-order check: original fatal guard, one head observation, early post-request service, original Factory/Goal ticks, original drain phase, original accept phase. No unrelated source/profile/budget changes.

Rust source tests are in `crates/core/src/ipc_owner_read_queue_tests.rs`. Root should run `cargo test -p agentbaton-core --lib ipc::owner_read_queue_tests` in its complete controlled workspace, then the relevant existing IPC, Goal stop, Factory history/runtime/rework suites. This author did not execute Cargo or any test. The retained-fatal tests inject an explicit error at the actual post-request boundary; they do not claim to manufacture a real failing data-admission worker. End-to-end data admission and real cancellation/join behavior require the existing runtime suites.

Independent review, Rust compilation/tests, native/UI validation and same-candidate Intel/ARM CI remain pending. Static evidence does not prove responsiveness or product acceptance.


## Root actual checkpoint 2026-10-31T00:21:23.878308+00:00

Actual source cbb16c7: Core180, Factory33, strictClippy/fmt and same-source normal browser4checks/4PNG pass. Test-only b183c86 fullinfra284 pass after retained NameError. Exact commands, immutable log/result/input hashes and f55 failed CI are in ROOT_RESULTS.json (SHA256 3a2e5cd21f2a3b00967b69e0543ab8348484f1c2af69ace2626a2760a7ebf627). New exact CI, dualMac/Native/audit/installer and full130 acceptance remain open; no deadline, retry, budget or authority expansion.
