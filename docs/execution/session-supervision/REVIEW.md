# Independent review · session supervision

Reviewer: native `/root/review`. Active work base `4e1efd378e03cbfeddf11351299da88e911df85c`, branch `codex/session-supervision`. Final source review covers the fingerprinted partial unit below; no acceptance of unfinished requirements or the complete product is implied.

## PR #3 native CI follow-up

Exact head `30a70d8ad0f1aeff26346242bdd4ec4b3bd5be38`, product run `40364802580`:

- Both ARM job `121719495484` and Intel job `121719495659` passed the direct oversized-frame parser test and the oversized-socket/no-mutation/liveness integration test. This is actual native evidence that the prior malformed-client fixture issue was passed at this candidate.
- Both then failed nine of eleven ACP stdio fixture tests during session creation with `Closed`; initialization succeeded. Intentional malformed-session test also received a close before its intended malformed reply. Neither job reached native desktop compilation or package production.
- Diagnosis confirmed independently with the exact candidate's Python fixture: a symlinked working directory sent as a noncanonical `cwd` makes the fixture's strict `request.params == {cwd: os.getcwd(), mcpServers: []}` assertion fail. Linux controlled reproduction: alias pathname yielded exit1, only initialize response, AssertionError; canonical pathname yielded exit0, initialize plus session/new responses.
- Independently reviewed isolated correction: canonicalize the fixture descriptor's workspace before sending, preserving the exact protocol assertion. Corrected real stdio roundtrip also passed locally with a symlinked TMPDIR. Production transport unchanged; this is not proof of a subsequent macOS pass.
- Previous candidate failure evidence and test-contract corrections remain in product-foundation/project-registry review records. Do not relabel the prior failed runs successful.

Raw failed native-job logs retained locally (ignored artifacts):
- `artifacts/product-foundation/ci-arm-30a70d8-failure.log` SHA-256 `3e8641e18cc6b0071b36d7a900fbdba0127ff3c3d1a2ece1d5229f4b463b8f17`
- `artifacts/product-foundation/ci-intel-30a70d8-failure.log` SHA-256 `d9ee059e9544eaa1ce3bd23a4e55ddfb7f280793f7ba2f002de3fd8bf6080181`

## Subsequent PR #3 evidence and reviewed corrections

- Head `e732be151c78ba47bc9a7d4a80f13bb2652f2d51`, run `40366339701`, ARM job `121724960080`: canonical cwd fixture passed; stderr flood then timed out. The same 4MiB/500ms workload with a deliberately 4096-byte Linux pipe reproduced the production scheduling issue. Removing sleeps only after useful work was insufficient; readiness polling is required for true-idle wakeups. Independently reviewed `libc::poll` correction retains 16-read fairness, original deadlines and original flood volume; EOF stderr is disabled to avoid a HUP busy loop. Isolated foundation stdio suite: 12 PASS. This was a production scheduling correction, not evidence of a complete native run.
- Head `d64d57cd59ca34aeb4a63e68bc1a6ceaac0c9a11`, run `40368611277`: Rust/UI and Intel product job `121732958753` succeeded; ARM job `121732958660` failed in `socket_workflow_single_writer_and_restart_history` with `Null` versus `draft`. Its log does not print the response error. The historical ARM root cause remains unproved.
- Independently approved the next three-file correction at `7521b38d509facccd081d25dc1ed0009c98b3038`: drop the auxiliary database fd before opening SQLite (a later non-SQLite close can release process-scoped POSIX locks), expose only numeric SQLite extended codes, and assert absence of an IPC error before checking the result. Independent isolated `cargo test --locked -p agentbaton-core`: 19 PASS. This is a justified lock-lifecycle repair plus stronger diagnostics; it does not prove that lock release caused the historical ARM failure. Product run `40376998625` was in progress when checked.

## Session-unit independent findings and verification

This review uses a separate native agent context, reads the actual shared source, and runs checks independently. It does not rely solely on implementation-agent reports. AB-START-001 authorizes this repository work; private Provider accounts, model calls, signing and production remain closed.

Findings corrected during review:

1. A full bounded event channel could drop terminal process-stop evidence. Authoritative `JoinHandle<WorkerReceipt>` now carries explicit reaped/not-started proof independently of the event queue; thread completion alone is not proof. The full-channel regression passes.
2. Streaming revision increments could indefinitely stale a cancellation observation. Cancel now uses immutable session identity, accepts an older observed revision and rejects future revisions; rapid-stream cancellation passes. A restarted unknown session without an owned worker returns `OUTCOME_UNKNOWN` instead of falsely entering a controllable state.
3. Delayed ready/turn/failure events could resurrect a confirmed stopped session. Transitions now require the matching active state; late failures preserve stopped status. The queued late-events regression preserves released lease and budget evidence.
4. A raw socket read and heavy Git admission blocked the sole SQLite actor. Bounded reader/reply workers isolate slow sockets, and new heavy admission returns explicit `WORKSPACE_BUSY` while owned sessions exist. The concurrent slow-client/heavy-registry/cancel regression passes. This is a bounded admission restriction; background Git admission and general concurrent scheduling are not complete.
5. Native renderer input originally risked granting execution authority through a consent boolean. Connection/grant actions now produce a server-resolved proposal, show its complete escaped snapshot in a Rust-only native dialog, and internally finalize only the server proposal ID/revision. No finalize or dialog plugin permission is granted to JavaScript. Independently compiled and ran the actual production `approval.rs` test module using rustc: 4 PASS on Linux. This does not compile Tauri or prove a visible native dialog.
6. Skill YAML parsing expanded aliases through a generic value tree. A reviewer-generated 30,276-byte fixture with a 30KB scalar and five levels of eight-way aliases aborted with allocation failure under a 128MiB address-space limit. The implementation now uses bounded scalar/key/metadata visitors and consumes unknown trees with `IgnoredAny`. Recompiled the original independent helper against the fixed library and reran the unchanged fixture and limit: exit 0, `returned true`, no stderr. The subprocess regression, known alias-sequence rejection and literal `*`/`&` handling pass. The original failure is preserved in `crates/config/evidence/native-sources.json` rather than erased.
7. Codex's documented 64-character skill-name limit was incorrectly combined with a generic UTF-8 byte limit; exact 64/65 CJK regressions now pass. Claude settings with non-string environment values now report malformed without exposing values.
8. Git assume-unchanged/skip-worktree and sparse checkout could hide modified input. Inspection now rejects these before status, overrides stat shortcuts without changing the index, and preserves source/index bytes. Five regressions demonstrate the ordinary status false negatives and fail-closed admission.
9. Normalized updates omit raw protocol/stderr history. The durable launch/ready metadata now explicitly records `completeness: partial`, `raw_protocol: false`, `stderr: false`. Complete-history requirements remain unfinished; normalized fixtures must not claim them.

Independently executed on the reviewed working tree before the final signal/live-policy additions:

- `cargo test --locked -p agentbaton-core`: 59 PASS, including real fixture-child IPC, crash/unknown recovery, scope/budget lineage, receipt backpressure, late events, cancellation and bounded admission.
- `cargo test --locked -p agentbaton-config`: 45 PASS, including 19 native-source/Skill parser tests and transactional config regressions.
- `cargo test --locked -p agentbaton-providers`: 56 PASS, including real fixture-child stdio, fixed turn deadlines, blocked stdin, streamed events, control backpressure and readiness polling.
- `cargo test --locked -p agentbaton-workspaces`: 18 PASS.
- `npm test -- --reporter=dot` in desktop: 28 PASS. These are mocked UI/transport tests.
- Independent production approval-helper module: 4 PASS; unchanged malicious YAML helper replay: PASS under 128MiB.

Remaining scope and evidence limits: local custom ACP only; native Codex/Claude/Qoder runtime protocols and actual identity/authentication remain unverified and are not silently ACP-fallbacked. HOME/XDG directories, declared recipient/network scope and worktrees are not OS sandboxes. Permission requests are denied, not forwarded as authority. Raw protocol history, complete capability application, full planner/delegation/verification/adoption/remote/lifecycle workflows and the full design remain partial. A Provider turn ending does not accept a task. No real model calls, private credentials, GUI desktop validation, notarization or signed release was performed by this review. Current session changes have no Mac CI/artifact evidence yet; PR #3 native results belong to its exact older source.

Additional final findings and checks:

- Static review found `live_workspace_matches` opened a pointer before testing regular-file metadata. A FIFO at `commondir` or the backpointer could block the actor indefinitely. Added `O_NONBLOCK | O_NOFOLLOW` and retained descriptor regular-file validation; the killable-child regression exercises both FIFO locations. I did not execute the vulnerable version as a FIFO repro. Independently ran all four live-filesystem tests: PASS. Independently reran the now 13-test `sessions_ipc` suite: PASS, including actual IPC send rejection, responsive status and owned cancellation with hostile FIFO metadata. This checks live repository linkage, not clean inputs or an OS sandbox; new admission still performs full Git inspection.
- Independently ran `cargo test --locked -p batond --test signals`: 2 PASS. The real daemon binary receives SIGTERM with a live generated fixture child, durably records explicit reap evidence and releases its lease; restart plus SIGINT exits cleanly and removes the socket. A pending generated version probe is reaped after its bounded timeout. These are Linux subprocess/signal tests, not macOS sleep/login-startup/full desktop lifecycle acceptance. DB-error cleanup still relies on conservative unknown recovery rather than claiming a durable stop if persistence fails.
- PR #3 exact head `7521b38`, Intel job `121762663775`: IPC passes but both original-volume stderr tests fail with Timeout (10/12 stdio tests pass). Thus the prior readiness correction is not yet proved sufficient on both Mac architectures. Raw log is retained as `artifacts/product-foundation/ci-intel-7521b38-failure.log`. The ARM job was still running at this check. A residual useful-progress `yield_now` is a scheduling hypothesis, not a proven historical root cause. This native failure remains a merge/acceptance blocker; local passes do not supersede it.
- Primary's actual Linux Tauri `cargo check --locked` log was read independently at `/workspace/.onboarding/linux-native-check-final.log`: finished successfully with actual dialog/core/native dependencies. This is reported primary execution with log inspection, separate from this reviewer's independent helper test. The initial native-host test attempt subsequently failed at linking; its failure is preserved. See the final integration evidence below for the corrected Linux native-host result. No GUI claim.

Final verdict: no unresolved source-level blocking issue found in the fingerprinted local session unit after the fixes above. It is suitable for a reviewable feature-branch checkpoint/PR and continuing the remaining authorized goal. Merge still requires its exact mandatory CI and prerequisite branches; native failures are not waived. Full-product acceptance is not granted.

Final independent command `cargo test --locked -p agentbaton-core -p baton -p batond`: 65 core tests and 2 real daemon signal tests PASS (do not double-count the nested FIFO child harness). The current session transport and installation probe have no `yield_now` or fixed sleep after useful progress; only true-idle readiness polling remains. The residual-yield hypothesis applies to the isolated older PR #3 source, not the current session implementation.

## Reviewed source fingerprints

SHA-256 of actual shared working files at 2026-10-26T18:27:54.027675+00:00. Base remains `4e1efd3`; the primary agent assigns the final commit. Any subsequent source change requires checking its corresponding fingerprint and test scope. Review/status documents are deliberately excluded from this table.

| File | SHA-256 |
| --- | --- |
| `Cargo.lock` | `3f518d6cfeb5cdf126a24708120b4ec8349c5aab1d80ff4512b8296efcd5a33c` |
| `apps/cli/src/main.rs` | `7d2003574a945a52864593ba857819c1ecae348ab366a8a09b499c0d95663a17` |
| `apps/daemon/Cargo.toml` | `20d92630b9f65de42c0913c2fa93749084d1d9eeebc1a01e0d2bafe27683297c` |
| `apps/daemon/src/main.rs` | `372f71e1b9a4cefbb158324e7d73207e892056736e5557d25723fb64259eea7c` |
| `apps/daemon/tests/signals.rs` | `150a794d8659152bcd93d42c9870026d98287e320b7772e9dde719e7364027ff` |
| `apps/desktop/README.md` | `cee3e5500b74988f7fbf9dcf4b1627c4f44d13d67ee7b3d9e0b1f6c2dc107e1d` |
| `apps/desktop/src-tauri/Cargo.lock` | `b8bc038fff4ed7c4f4571a108fe6839cb097fd0660142da7376225ff39c83694` |
| `apps/desktop/src-tauri/Cargo.toml` | `2b66acb381d724d709654e0d5733fa43f1ccdd0854a3a707bca40eeeb0cb2bb6` |
| `apps/desktop/src-tauri/src/approval.rs` | `be13845668be4f3c0ac01ae9adf8bffc850fb22138d143fb906fb2eef58d308c` |
| `apps/desktop/src-tauri/src/main.rs` | `ed7f60412bc00c11b2e3d48a3dd1bed2c7e1e5b80425e373d9a7573c28ddbeb6` |
| `apps/desktop/src/App.test.tsx` | `09ba24e67dff8df253df83d1e5670c593a3deba6234276edf9820988bb38fada` |
| `apps/desktop/src/App.tsx` | `dfc49a9c609aef27f7c83ab28a228e1c0376dbf7f58f6be27583356712757b1c` |
| `apps/desktop/src/Connections.tsx` | `f1be7b4978449b7bc8fcb50e9056b9939bb5abfc2c2384fb9ac104e07003d32e` |
| `apps/desktop/src/ManagedSessions.tsx` | `5e2da6e4cd7289eb654f5ac1f73de31b33168e7f99ad4f035c87c72994a1090e` |
| `apps/desktop/src/SessionWorkbench.test.tsx` | `61d6ce4f0fc7546576e2969396615dcc2824a1bdfd7c6889408b08c4f3e48822` |
| `apps/desktop/src/api.ts` | `0275296ca30f79089ef83d729f4a111929a5bb42196b3db1775b9c18d858711b` |
| `apps/desktop/src/session-api.test.ts` | `6072418acd558b80b0000e51d3739b4feee56b188f8036dd32fcc3f6fb77818a` |
| `apps/desktop/src/session-api.ts` | `9bcf228b9ff9db2f44407355ba8d43ab2bc5809e510429d735c3428cd3c11de8` |
| `apps/desktop/src/session-i18n.ts` | `3bb6af5dd2f65371b4990694b8f3a6eaedd285d4d95b80ef5a81da485c6511e7` |
| `apps/desktop/src/style.css` | `10cccaa8267507a8fdaaa6eacc2e64cc850a6d7cf38d2ba465a47ab9a1330caf` |
| `crates/config/Cargo.toml` | `17843f041172110202c01843bacfd2ae96ddfe0b4ac4ac4aa07a4c30efdc5690` |
| `crates/config/README.md` | `82803cb56052bcbc70e79da8f779a526a6e4b0e910e3e2fc5f393ade9b21211c` |
| `crates/config/evidence/native-sources.json` | `47beac151869c62e1da654be9a22e96bb57e8d1ece899550f380da69c3677bd7` |
| `crates/config/src/capabilities.rs` | `8a5bb9a9814c981c353f89f6cba041d8157e8488c75bb88e10f10136ed16cee5` |
| `crates/config/src/capabilities/native.rs` | `3fec218349c9e6b5fe18e288150e276d581711427bf9c558cfcb057caab5ce6a` |
| `crates/config/tests/native_sources.rs` | `13e67f49c4bb434d5655e97ea09234ad17a3af2e89d4d7c2d0af2ced8251aa15` |
| `crates/core/Cargo.toml` | `e5debd15030a5e2c49b28d5c4c9fc061b18a39fd0b90d08225f946e0d5c3e323` |
| `crates/core/src/execution.rs` | `8ca281e755206dae172299892c65299b884383961ff4af2219141f4353f841b5` |
| `crates/core/src/ipc.rs` | `2354e556d4d52d6b2301b3fa8b51da192b2bd8828fc785e0717b11a811cf965f` |
| `crates/core/src/lib.rs` | `26db2de6d9015c40d2c2bfbed65ce53ba334b87d82eb4c398b3fc5d7a93ab53f` |
| `crates/core/src/policy.rs` | `5b4e6df91543d0df3ee727e9a1a590be4a125a94ae04b440d13b3aa2ce148d45` |
| `crates/core/src/registry.rs` | `4c339c794bdf2cb67e3320bafa81ae789eb9b5b34b4da9ca65ab650b3065d27d` |
| `crates/core/src/sessions.rs` | `56ca698c6a6b02f626e79aa820d9a3f95a9f595808b2a905f1753ce60a177700` |
| `crates/core/src/storage.rs` | `5179d5da6e6ae2b769a72cbae5de340a1c0409dc03363b7e4006cc16030db43f` |
| `crates/core/tests/execution.rs` | `70f4121685c332562ee92632eaebbb4fdb440692d7dd0b89b5aecea5c73ecc06` |
| `crates/core/tests/migration_v3.rs` | `68e45867d5a77b634eb6a7f271008143b52c4e7f5742187cf631d43c0c1691bf` |
| `crates/core/tests/policy.rs` | `b9bfae2833bbe4e6ac4dc85437b1d99e1931ae33ceff91bae39ed6945aa3f92f` |
| `crates/core/tests/sessions_ipc.rs` | `512d0bd901ca91fe492b7c63c9589d31a8d8aec5eef06d84ddcae47d38f3c31d` |
| `crates/core/tests/storage.rs` | `8a15c63d78e3ba84b5e7e49bed4ee5f6cce985ae5fbe00aefe14b5d4ff3ddee5` |
| `crates/providers/EVIDENCE.json` | `78d33d6ed5f655d03b7a9e803866c259f12243004bf5df6842587491983b370c` |
| `crates/providers/PROVENANCE.md` | `9a5134d403e39060b28b5678797c16bbed17f10f02f6ceb4354ba7836c1bc455` |
| `crates/providers/README.md` | `81a74c60f119fab99d6e763a5102ec1c12a119558200c7439da758187769eaa2` |
| `crates/providers/src/installation.rs` | `c623fcb56a356977eb406c93693c30504c7aff1ece153f713d090f5e3c323d87` |
| `crates/providers/src/lib.rs` | `9237c5a3f765a71dd97d7f49897c24cc06db85926e36b5579af0c4ca47de2fd5` |
| `crates/providers/src/transport.rs` | `68a953ad51567ec9570f72c1d3c5893dba69d814e7b8352d75d3e9a38f1b0dc4` |
| `crates/providers/tests/installation.rs` | `cd6c75e9c5320938b4ba3b50529773f7494a806c51253091449a486547f10e9b` |
| `crates/providers/tests/stdio.rs` | `f3b4d84acec039e4568bdbf8e2cb25e646843e5d436de89e1857104916ba33e2` |
| `crates/workspaces/README.md` | `084a39c356117def7599a6459e79967190e8bbcaad1a13213763322015c06224` |
| `crates/workspaces/src/lib.rs` | `31340e7eddcc414e497a54d5f5066994d0fb77bc46d774b915c5667afc556f75` |
| `crates/workspaces/tests/safety.rs` | `aea419fc8d4f4640d1522170063b034a958c2be2951714cbe1cf2cf8bc990420` |

Additional retained raw-log hashes (local ignored artifacts, not embedded credentials):

- `artifacts/product-foundation/ci-arm-e732be1-failure.log` SHA-256 `8b4a639ad04dc4d49d2ec8b7569daf68f34936c63a334f7cb553231ba38e21f2`
- `artifacts/product-foundation/ci-arm-d64d57c-failure.log` SHA-256 `e4652c506bea5c36c6c2e18ca3c537902af3b413f8d2a4750155da715d10aa72`
- `artifacts/product-foundation/ci-intel-7521b38-failure.log` SHA-256 `e3d06d5acc8525ffcbc3d782d01fdee3bf2371569da28dc52c425663d922e90a`

## Final native follow-up at the reviewed boundary

PR #3 run `40376998625`, head `7521b38d509facccd081d25dc1ed0009c98b3038`: ARM job `121762663832` subsequently **succeeded**. Its actual log includes all 12 stdio tests, three old native host tests, a release app build and DMG packaging/upload. Intel job `121762663775` still failed both stderr deadline tests. The overall run therefore does not satisfy both architectures.

Downloaded actual artifact `12676265263` and independently verified its ZIP SHA-256 `328fb6cc85d81a940f8fad98723e52fd22ceb81ed694ec7588e38a5b16ec1f63` and contained DMG SHA-256 `28a40cfaafe819456655c3dd14cea31ed932a1402ce3e3900f450ea620074089`. The DMG filename and build record bind to GitHub's PR merge-test commit `5c539035c57abab5077cfa0252343dbdee90669a`, not directly the head SHA. The build record reports macOS 15.7.9, arm64, Xcode 16.4 and Rust 1.99.1. Signature inspection says `adhoc,linker-signed`, no TeamIdentifier, no Developer ID or notarization. This real foundation development package does not prove the current session source, complete product or manual GUI acceptance. Artifact URL: https://github.com/wanderbot-auto/AgentBaton/actions/runs/40376998625/artifacts/12676265263 .

Independently reviewed the isolated two-file next correction: remove scheduler yields after useful stderr progress, retain readiness polling only on no progress, retain the same 16-read fairness and original monotonic deadline, and add only failure diagnostics. The 4MiB/500ms workload and Completed/observed-byte assertions are unchanged. Independent corrected stdio suite: 12 PASS. Read the diagnostic worker's actual synthetic experiment logs: a shim adding 2ms to `sched_yield` makes the unchanged old small-pipe test fail in 0.53s and the corrected source pass in 0.05s. This proves sensitivity to modeled handoff latency; it does not establish the historical Darwin root cause. Fresh native CI remains required.

Retained successful ARM log SHA-256: `0af840167c3ce0a8259bde0fafec5d1008724c6954179cb08f9ba56873cb6ee7`.

## Final integration evidence audit

The primary completed the Linux native-host link environment after the earlier missing-library failure. Independently read the actual final logs: `linux-native-check-final.log` finishes successfully, and `linux-native-tests-final.log` runs all seven production host/helper tests with 7 PASS, 0 failures. These include the native-only approval helper, fixed sibling daemon selection, stale-socket handling and bounded readiness probe. This is a successful Linux Tauri/native bridge build and host test run, not a visible dialog/window, macOS session source, vendor-model test or full lifecycle acceptance.

The original linker failure remains in `linux-native-tests-link-missing.log`; it is not reclassified as passing. The primary's private development sysroot repaired 88 missing runtime link targets using already-installed runtime libraries. The reviewed overlay inventory contains 88 entries. This review inspected the resulting logs/inventory; it does not claim an independent forensic proof of every environment mutation. No system-directory write was performed by this reviewer.

Audited `EVIDENCE.json` against the actual primary execution files: all 14 recorded log hashes match, including Rust integration, Clippy, formatting, build, real SIGKILL/restart smoke, UI checks, native success/failure and infrastructure/bootstrap records. Summing the Rust log's test summaries gives 187 harness tests, of which one is the nested FIFO child: the stated 186 top-level tests is correct (65 core +45 config +56 providers +18 workspaces +2 daemon signals). UI's 28 tests remain mocked-transport evidence. These aggregate primary commands are distinguished from this reviewer's independently executed component commands recorded above.

Rechecked all 50 source fingerprints: no mismatches. Audited the current `STATUS.md`, `CHECKPOINT.md`, `EVIDENCE.json` and 130-entry requirements progress table: 68 entries are implementing with partial coverage, 62 are not started, and no complete/verified requirement acceptance is asserted. The documents preserve open model/account/signing/production gates, current-unit macOS/GUI limitations and the fact that older ARM/Intel artifacts come from different candidates. A few inherited capability notes still say native parsing remains; these conservatively understate the new selected-source libraries and can be clarified without changing acceptance.

Final checkpoint verdict remains: locally verified partial implementation may be committed and proposed as a stacked draft PR; no source-level review blocker remains in this fingerprinted unit. Exact required CI, prerequisite integration, a jointly passing macOS candidate, current-source artifacts and all unfinished design requirements remain necessary. The primary must continue the authorized goal after this checkpoint.

- `linux-native-check-final.log` SHA-256 `c0fa7b2f72186d49b8286785edaa9c7d2d8a726753c89ceeeaf56f03ae829da0`

- `linux-native-tests-final.log` SHA-256 `bc7b0938db973ab7c3a9c07b24b46464b99b2b4ac4e62ee685bcca2f675b7108`

- `linux-native-tests-link-missing.log` SHA-256 `ee9dfa57b3147b0235e50270af6ac182f937520ed7ad3f20eec5f610a8e161f5`

## OWNER-002 integration boundary

Independently read protected-main commit `2d9361df71aed03cde47c363b8c575636d177c1a`, its OWNER-002 inbox entry and the complete Quiet Canvas implementation/acceptance overlay. GitHub PR #5 is actually merged at that commit. The directive replaces default UI presentation with the specified light 2×N task canvas and focused conversation; it preserves full v0.2 capability/safety scope and does not grant extra credentials, spending, Provider/model calls or production authority. Synthetic prototype records remain reference material, not live product evidence. The primary ACK is recorded at `a5324a6` on the next runtime adapter branch; UI work has a dedicated owner/context.

Independently checked foundation integration commit `23de87e73c2d0cc896e8f7f332d227b71fa77c63`, parents `e5e0ec1` and `2d9361d`. Relative to the previous foundation candidate, changes are restricted to BOOTSTRAP_MANIFEST, OWNER_INBOX and five Quiet Canvas reference documents. No product source, app, script, workflow, infrastructure gate or immutable specification changes were introduced by that merge. `python3 scripts/verify_bootstrap_manifest.py` passes all 92 entries. The merged manifest preserves the union of both parents' entries; all five differing paths bind to the actual merged bytes, and every nonconflicting parent hash is retained. No content-loss or authorization-expansion blocker found.

Actual GitHub APIs confirm `e5e0ec1f56660cc1c322af59e82d9dbb0bc3e48b` Product run `40378120708`: Rust, UI, macOS ARM and macOS Intel all SUCCESS; Environment run `40378120759`: all three required Environment checks SUCCESS. This is genuine same-candidate dual-architecture success, superseding earlier failures only for this precise source candidate. The merge from updated main changes the head: push it, require exact-new-head checks/review and respect branch protection before merging. Prior green results are not silently inherited by the new commit. Current runtime-adapter/Quiet Canvas edits remain separately unreviewed until their own freeze and evidence.
