# Independent session portability review

Reviewer: native `/root/integration_review`, independent of implementation. Reviewed source `e261740f9e102142f3be47219dc495d26909a8b1` and documentation candidate `51ed39d`, compared with `80e6431`, at 2026-10-26T20:39:25.276358+00:00. Existing `INTEGRATION_REVIEW.md` is preserved byte-for-byte with SHA-256 `cca900cf1aad333f97d76bbbe5b5e79798ce50dbe932b4c0c17e408c5d70e493`. Only this new review document was written; no source/lock edits, commits, pushes or merges were performed.

Verdict: no unresolved blocker in the three-file portability delta. Native macOS and exact-candidate required CI remain separate; neither historical ARM causation nor full-product acceptance is established by these Linux tests.

The production change explicitly clears inherited O_NONBLOCK on the accepted worker-owned UnixStream before existing bounded first-frame IO. A reset failure closes only that worker/client. Listener, actor, authentication, schema4/session/grant interfaces, MAX_FRAME and three-second timeouts are unchanged. The added delayed-first-frame regression deliberately sets the input descriptor nonblocking and verifies the bounded worker receives a complete later frame and sends its response. Its identical old-code control fails with BrokenPipe while the corrected candidate passes; this supports the mechanism rather than claiming the old ARM failure cause.

Only fresh test-owned policy/registry managed roots now request explicit 0700 creation permissions. Production root admission is byte-unchanged from 80e6431; ordinary imported/source directories are not chmodded. The registry helper asserts 0700. Intentional 0755-root refusal, dirty/replaced source, lease, rollback, idempotency and original NOT_FOUND assertions are preserved. No schema5/runtime-plan/native-route/UI feature was copied into this correction.

## Independent commands and results

Ran in `/workspace/.worktrees/session-integration` under `umask 022`, with `CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup PATH=/workspace/.toolchains/cargo/bin:$PATH CARGO_BUILD_JOBS=2`:

| Command | Actual result |
| --- | --- |
| `cargo test -p agentbaton-core --locked --test policy --test registry` | PASS: policy 13 + registry 10. |
| `cargo test -p agentbaton-core --locked --lib inherited_nonblocking_stream_waits_for_the_delayed_complete_first_frame` | PASS: 1, other unit cases filtered. |

Independent targeted total: **24 tests**. These supplement the prior history/merge review; they are not added to the implementation suite count.

## Evidence verified

All **67 current source hashes** and **18 log/evidence hashes** in current EVIDENCE.json match actual files. The preserved `EVIDENCE.history-bounds-80e.json` matches previous-checkpoint SHA-256 `88fcfc4eaa7c9e4732dba7727bbf37e2a81ebb2a89d396c50ce1356f8f789772`. Read actual failure logs: old socket control fails one regression and the old default-permission policy fixture under 022 fails 13 tests. They remain preserved.

Read the actual full Rust log: 193 passing result-line tests include one nested FIFO child, hence **192 top-level tests** (core 69/config 45/providers 58/workspaces 18/daemon signals 2). Actual UI log reports 35 tests; logs record successful restart smoke, clippy/fmt/build/typecheck, design/infrastructure 28 and 92-file bootstrap validation. Those full commands were observed and hash-verified, not independently rerun in full here. Current EVIDENCE.json SHA-256: `a2e7d58d850666e1ba309a4fd6320565cfe0ec3c2f8da223256260f61973cf06`.

No real vendor/account/model request, private credentials, production operation, signing, additional paid service or native window was used. No current-candidate macOS/remote CI success is claimed; an unrelated PR4 ARM CI result is not inherited by this branch. The existing bounded/partial history and default product limitations remain as documented. Full Design v0.2 remains incomplete.

## Reviewed source fingerprints

| File | SHA-256 |
| --- | --- |
| `crates/core/src/ipc.rs` | `7e11083700413641bdafa8d375162c468540ea5357ad0495d04b6df33f73bf02` |
| `crates/core/tests/policy.rs` | `52eb621f116060ec2e7447d0375de14ebd81f7d6043beeef5e1bf0474fa8ed4f` |
| `crates/core/tests/registry.rs` | `97efcad183a1f150bfd7b3273a1ffded1dae3765569f15cde6e0a666eb027aa7` |
