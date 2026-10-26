# Independent session schema4 integration review

Reviewer: native `/root/integration_review`, independent of the implementation context. Candidate `80e6431b99c24bc2f86f83dc9a7eabd70b3ef23b`, worktree `/workspace/.worktrees/session-integration`; reviewed and verified at 2026-10-26T20:05:26.147550+00:00. AGENTS, execution gate, OWNER-001/002 and current integration/evidence records were read. No sources, locks, commits, pushes or merges were changed by this reviewer. Only generated local children were invoked; no real vendor/account/model, production, signing or additional paid service was used.

Verdict: no unresolved blocking source finding in these scoped schema4 integration changes. This is review of the preserved session/registry merge, readable execution recipient and bounded-history backport; it is not full Design v0.2 acceptance or approval of remote/native CI results.

## Merge and behavior assessment

- Confirmed protected main `1f34b444821640bf88f32a8bb39c6434d4ca26cc` and registry checkpoint `a429cb3` are ancestors. Merge `a159aed` retains the newer session owned-transport/polling implementation, and `8f94696` preserves registry review/checkpoint documents. Compared with reviewed session `7a6a46b`, production transport, session supervisor, core IPC and workspace implementation have no diff. Registry tests remain preserved; the registry error bridge additionally maps the existing UnverifiedIndex refusal to INPUT_CHANGED.
- Both fixture families survive the merge: original stderr modes retain 4MiB workloads, original version-probe deadline remains 500ms, canonical workspace identity stays pinned, and the original Linux stdio capacity assertion still checks actual 4096-byte pipes. Additional small-pipe tests do not replace or weaken those original assertions. The independently rerun generated-child tests passed.
- `bc47ea1` adds server-resolved project name and readable recipient name/provider/route/executable/argv/hash/status to the owner proposal. Schema4 only permits custom ACP; absent legacy protocol_route is displayed as custom_acp. Checked finalization compares the complete current references. The independent policy suite includes the two-recipient metadata drift rejection.
- The history backport obtains octet_length before fetching/decoding a full journal document, caps raw pages at 4MiB, preserves ordered last-returned next_cursor and refuses an oversized first record without skipping it. Its invalid-JSON oversized fixture demonstrates the size check precedes decoding.
- The existing frontend keeps 8MiB encoded-JSON and 500-record visible bounds, including array delimiters. An over-budget merge replaces the display with the requested bounded earlier page and an honest local-retention message; an oversized incoming display page is rejected. Byte-limited pages are labeled. A history-only RESOURCE_LIMIT no longer disables authenticated task cancellation, and the regression passed.
- This branch deliberately retains its old layout/navigation, custom-ACP-only routing and schema4. It contains no schema5 runtime-snapshot migration or native Provider route copy. Full cursor navigation remains limited and is explicitly disclosed in EVIDENCE/INTEGRATION; the separate Quiet Canvas/UI pagination work remains necessary. No persistent journal record is removed by display-window bounds.

## Independently executed validation

Rust commands used `CARGO_HOME=/workspace/.toolchains/cargo RUSTUP_HOME=/workspace/.toolchains/rustup PATH=/workspace/.toolchains/cargo/bin:$PATH CARGO_BUILD_JOBS=2` in the actual candidate worktree.

| Command | Actual result |
| --- | --- |
| `cargo test -p agentbaton-core --locked --test storage --test policy` | PASS: storage 8 + policy 13. |
| `cargo test -p agentbaton-core --locked --test registry --test project_ipc` | PASS: registry 10 + real project IPC 3. |
| `cargo test -p agentbaton-providers --locked small_` | PASS: two generated-child small-pipe regressions; other tests were filtered, not counted. |
| `cargo test -p agentbaton-workspaces --locked --test safety` | PASS: 18, including hidden index/stat-trust/dirty protection and isolation. |
| `npm run typecheck && npm test -- --reporter=dot` at `apps/desktop` | PASS: typecheck +35 tests /6 files; transport mocks/helper tests, not native GUI. |
| `git diff --check` | PASS. |

Independent targeted Rust total is **54** (21+13+2+18); these are a subset of the implementer's full-suite evidence and are not added to its total.

## Evidence verification and limits

Current EVIDENCE.json binds tested source `695144ba9f97886051eb7db54827c72017ecb1c1`; subsequent `80e6431` changes are documentation only. All **86 current product-source fingerprints** match actual candidate files. All **12 current logs** match their recorded SHA-256. Inspected actual logs report 191 top-level Rust tests (core 68/config 45/providers 58/workspaces 18/daemon 2), 35 UI tests, successful fmt/clippy/build, actual daemon/CLI SIGKILL/restart smoke, design 130/14, infrastructure 28 and bootstrap 92 validation. The nested FIFO child is not counted twice. These full-suite results were read and hash-verified, not independently rerun in full by this reviewer.

The exact preserved `EVIDENCE.integration-8302.json` matches its recorded previous-checkpoint SHA-256 `10fa92fc3c433c072e06903ad85dfb265a2d8dbef8576b3a9f28d1f3fedadf31`. All **84 historical source hashes** match source at commit8302; the eight expected history-backport changes are not falsely treated as historical source matches. All **14 historical logs** match their recorded hashes. Current EVIDENCE.json SHA-256 at review: `88fcfc4eaa7c9e4732dba7727bbf37e2a81ebb2a89d396c50ce1356f8f789772`.

Actual macOS native host/GUI/confirmation, current-candidate remote required CI and installation evidence remain not_run. Earlier host tests do not become acceptance of this merged candidate. Browser/mocked UI, Linux unit tests and generated children do not prove native macOS or live vendor/model compatibility. Full Design v0.2 and OWNER-002 product adoption remain incomplete; push/retarget/guarded merge are integrator actions subject to the exact candidate's required checks.

## Selected reviewed fingerprints

The complete 86-file set is in EVIDENCE.json and was verified; these identify the integration interfaces and conflict-sensitive files.

| File | SHA-256 |
| --- | --- |
| `apps/desktop/src/App.tsx` | `6d5eb2ac670900c4ab275de31133d5f5ed14115d56abe3ad4449ffb32a406215` |
| `apps/desktop/src/App.test.tsx` | `940ef6fa4aee4791074093f9e472f12ef950e3a0cdc39d65bd4b276c0ffee717` |
| `apps/desktop/src/ManagedSessions.tsx` | `7bbf112301e10c40b6ff5bf6222002c75984c641bea2164699dc4badc82387a5` |
| `apps/desktop/src/event-window.ts` | `73074b1d53281d6f738614f292b3f8a7cbfd95f3694c3ed3491cda9d637ad7a2` |
| `apps/desktop/src/event-window.test.ts` | `08be8fd17b0b30ab83acbb9c1536d4edf453e034b697a20cf8a04459c0c713cb` |
| `crates/core/src/policy.rs` | `d604dd00946ea1d2ffdfe09dfadefea48d1d9e34af3a134adc227e61a528c386` |
| `crates/core/src/storage.rs` | `905de3f02f01465c07690d3f88c5940e52e68531ad82acb13090bf91096a2c5a` |
| `crates/core/tests/policy.rs` | `595896299ce2045e78db2198222aedb35eda981fb3bdceb84a4b212194655cbb` |
| `crates/core/tests/storage.rs` | `714bfa25bd3c57bb795dce77aa645963e993fbbfd3b15aeaea2aa2773ef5a329` |
| `crates/providers/src/transport.rs` | `68a953ad51567ec9570f72c1d3c5893dba69d814e7b8352d75d3e9a38f1b0dc4` |
| `crates/providers/tests/installation.rs` | `90724f1d9d6c3fe5641ae7076d9c9442293a6550a4b20bb7baecbfcd6e6b1531` |
| `crates/providers/tests/stdio.rs` | `1be8c06e63a6107e94fc903bcdbcbddf662f77d4d96cf475f3eddb6485972dfd` |
| `crates/core/src/registry.rs` | `4c339c794bdf2cb67e3320bafa81ae789eb9b5b34b4da9ca65ab650b3065d27d` |
| `crates/core/src/ipc.rs` | `2354e556d4d52d6b2301b3fa8b51da192b2bd8828fc785e0717b11a811cf965f` |
| `crates/core/src/sessions.rs` | `56ca698c6a6b02f626e79aa820d9a3f95a9f595808b2a905f1753ce60a177700` |
| `crates/workspaces/src/lib.rs` | `31340e7eddcc414e497a54d5f5066994d0fb77bc46d774b915c5667afc556f75` |
