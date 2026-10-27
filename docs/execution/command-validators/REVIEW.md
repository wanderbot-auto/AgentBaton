# Independent fixed-command library review

Final reviewed source `1dae4566d09e469306a3f225f3c78e5cd448f124`, following initial source `eec524c559ec5e99ac8434fa45655a170bbbc0ad`, based on Data ddd6707. Separate context from implementation author. Review-only: no source, lock, index, commit, push or subagents. AB-START-001 and existing OWNER-001/002/resource/Provider restrictions remain. This library is not Core/native approval/IPC/UI integration or full Design v0.2 acceptance.

## Final verdict

No unresolved blocking finding in this fixed-command library scope at1dae4566. F1 is independently closed by the exact differential tests below. Ready for parent integration review and exact-head CI; actual macOS execution and Core/native-owner/IPC/UI wiring remain unverified.

## F1 initial finding — closed after independent correction verification

**F1 (P2): late exit observation can bypass the approved monotonic budget.** In the initial run loop, the deadline is checked before nonblocking capture/feed and `OwnedChild::finished()`. `Ok(true)` then selects Exited without another clock check. A parent scheduling/syscall pause between these points can make the observed exit occur after the approved wall budget while the receipt and command-only acceptance still pass. The documented inability to interrupt arbitrary kernel stalls does not justify reporting a late observation as inside its budget.

Actual independent Linux control used an isolated local LD_PRELOAD shim only in the test harness, delaying its first WNOWAIT|WNOHANG waitid1500ms. The generated executable and C compiler both use env_clear and do not inherit this shim. No shared/system library, source or product process was patched. The existing normal fixed-command case uses wall_time_ms1000 and asserts both run.passed and successful accept_command; under this delay it still passed, with waitid observing the real exited child/si_status0 and total test1.65s. This is a controlled synthetic observation stall, not evidence of a historical CI/Mac failure. Raw source/binary/log fingerprints are retained below. Author and integrator were notified for a post-observation clock/cancel check and regression.

A related cleanup boundary was reported by static inspection only: cleanup timing begins after the initial kill_owned call, and the loop checks completed reaping/EOF before testing the cleanup clock. No independent second reproduction is claimed for that boundary. At that initial checkpoint integration was blocked pending the author's correction and independent differential verification; that verification is now recorded below.

## Initial independent checks

Explicit umask022 and the existing validation-integration target:

- `cargo test --locked -p agentbaton-workspaces --test command_validation`:26 pass,1.34s.
- `cargo test --locked -p agentbaton-workspaces --doc`:4 compile-fail doctests pass.
- `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`:exit0.

All three initial source fingerprints match the supplied freeze. Seven explicitly hashed implementation evidence logs independently match, including Linux89+4, both Mac cross-target Clippy logs, formatting and the preserved initial Clippy failure. Cross-target checks compile; they do not run the13 Mac cases. No Mac runtime/native dialog/Provider/account/network/production verification was performed here.

| Initial reviewed file | SHA-256 |
| --- | --- |
| `crates/workspaces/src/command_validation.rs` | `fdc83ed40e68e4de3a590f69fa272dcb6a112b47d396dd598003623b80697f04` |
| `crates/workspaces/src/lib.rs` | `e396e03e5fbdf99ee4ffa2016240c1771da2892977651340cef2b4f3afdd45a5` |
| `crates/workspaces/tests/command_validation.rs` | `8896fd4c1062724771b01b5cd1b847491b0df0b225218ce13c9a35b04520f866` |

| Independent evidence | SHA-256 |
| --- | --- |
| `/tmp/ab-review-command-tests.log` | `ecd4fbefe90d74f701ae5a3f485b609e3f823ee04f9bb8d9696744f2235f2814` |
| `/tmp/ab-review-command-doc.log` | `901c55d586d664c1df623f16979cb7c2fd71098b0f7d5d0c4434b54aaa0dd5ef` |
| `/tmp/ab-review-command-clippy.log` | `8c233a54790c4800d3dd585436af679aa838da259bfdb129412e2d51cdb75cdc` |
| `/tmp/ab-command-deadline-review/waitid_delay.c` | `7d056f0d0975bd0a9cb0de510bae2ea9845983dd689f6215ffed7d7177c56eaf` |
| `/tmp/ab-command-deadline-review/waitid_delay.so` | `0eff0e99b4f304ce3095c1e5ea511a8d46415c0f233290633332bdbe24e1c814` |
| `/tmp/ab-command-deadline-review/baseline-delay.log` | `7a90bc7ae2b2456399ffdec2cff5f0d284f8d46761ce759295419a549a3d9e14` |


## Final F1 source/differential verification

The fixed loop rechecks cancellation and time after capture/feed and after the actual WNOWAIT observation, before classifying Exited. Exit observation within budget is an explicit receipt fact. Cleanup's deadline now starts before ownership observation/signalling, is never restarted after a delayed call, and is checked after waitid/reap and each pipe observation before accepting completion. The fixed receipt separately records exit_observed_within_budget, cleanup_completed_within_budget and cancellation_observed; both passed and accept_command require the appropriate facts. Cancellation is rechecked at the final observation boundary. Artifact/Git validation remains outside the declared process/cleanup timing interval; no approved deadline was enlarged.

The new27th top-level command test runs three controlled subcases in isolated test-harness children. Independently observed final output:

- Deadline: first parent waitid delayed1500ms under unchanged1000ms command budget → DeadlineExceeded, passed=false, no command acceptance, recorded elapsed1500ms and actual exit0.
- Cleanup: delay after a first positive exit observation,150ms under unchanged100ms cleanup budget → CleanupUnknown, passed=false and no acceptance. The original leader stays owned until nonblocking reconciliation. Later reaping does not change a failed timing fact or restore acceptance.
- Cancellation during a150ms observation → CancelRequested, passed=false, no acceptance.

The author also saved a real old-source failure of the new regression (Exited/passed=true/elapsed1501ms); that log was read and hash-verified. An intermediate cleanup-test phase selection failure is preserved and expressly not described as a reproduced production cleanup bug.

This reviewer additionally replayed **the exact original reviewer shim** and original normal positive case against the fixed binary. The earlier injected run passed0; the fixed injected run fails101 at its old run.passed assertion after the same1500ms delayed exit observation. That failure is the expected controlled rejection, not a failing normal CI test. The fixed27-test run without that external override passes, including its three negative observation subcases. Raw initial and corrected differential logs are retained separately.

## Audited boundaries and remaining integration limits

- Rule and command-only acceptance have private fields and no Deserialize. A cloned/serialized contract/view is not authority; trusted host code must authenticate owner approval/revocation and bind the subject before calling the explicit constructor. The complete digest binds subject/task/attempt/spec/input/version, program/script/root/path/hash/limits, literal actual argv, cwd/Git identity, environment value hashes, expected exit set, mode, platform image policy, UID/GID identity and enforcement/time/capture settings. Inputs/program/script are fixed-hash preflight reads; output validation and current observations are rechecked before scoped acceptance.
- Linux executes sealed approved ELF bytes from a memfd, avoiding substitution of the mutable original main-program path. macOS checks a canonical root-owned Mach-O path and every ancestor, denies group/world/current-nonroot write access including its ACL-sensitive access check, rejects root or mismatched execution identities/set-ID programs, and compares protected observations/hash before/after and at acceptance. This is a narrowly documented protected-path assumption, not Linux-style sealing or protection against privileged OS changes, dynamic libraries, hidden native config or transitive executable code. Mac sealed-image requests are refused.
- Script mode uses only the selected fixed system sh, actual -s/-- prefix, literal arguments and SHA-approved script bytes fed from memory. It does not execute a script pathname. NativeExecutable refuses shebang input/known shell paths, but this is not a universal interpreter classifier: trusted host approval must audit arbitrary binaries/arguments and cannot infer auxiliary-source integrity from an ELF/Mach-O hash.
- Unsupported filesystem/network/tree containment, execution-time input immutability or auxiliary-source immutability requirements fail before spawn. Actual input change-and-restore intentionally demonstrates that equal before/after hashes do not prove continuous immutability. Cwd uses a pinned open descriptor for fchdir; its root/Git candidate/tree/target bindings are checked. These are observations rather than a multi-file transaction or OS sandbox.
- Environment is cleared; only approved explicit values are passed and only names/value hashes enter contract evidence. Captured streams are separately byte-bounded, and receipts contain size/hash/truncation/EOF facts, not automatic raw output. Raw getters are trusted-caller data requiring appropriate redaction before future history; literal argv are intentionally visible in the approved contract and must not be treated as an automatic secret-redaction mechanism. No private credential was read or used in these fixtures.
- Nonblocking capture/feed/poll avoids unbounded reader-thread joins. Every post-spawn ordinary error path produces a failed/unknown CommandRun rather than discarding its direct Child. WNOWAIT preserves the original leader's PID until original-group signals finish; only that owned group/direct Child is signalled, with no PID discovery/scan. The trusted host must preserve SIGCHLD disposition and avoid an independent global child reaper throughout the run.
- A still-unreaped child remains privately owned by CommandRun for nonblocking reconciliation. The caller must retain such an unknown run; dropping it is not a durable supervisor/restart/replay strategy. CleanupUnknown and late/cancelled outcomes cannot obtain a command-only token, even after later reap. Descendant containment is always honestly not_enforced_unknown: all-pipe EOF or leader reaping cannot prove every escaped descendant stopped. The release-file/short self-termination escape fixture demonstrates bounded capture refusal, not containment or a general escaped-process reaper.
- Matching exit0 and artifacts permits only approved_deterministic_command_only acceptance. It does not prove reproducibility, code/scientific correctness, a whole task, Git adoption or production authorization. No Core policy tables/outbox/lease ownership, native approval, public command entry, UI or global acceptance is wired here; those integrations still must enforce their own authenticated policy, idempotency/recovery and acceptance gates.

## Final independent commands and evidence

Explicit Linux umask022, managed toolchain and CARGO_BUILD_JOBS=2, existing CARGO_TARGET_DIR `/workspace/.worktrees/validation-integration/target`:

- `cargo test --locked -p agentbaton-workspaces --test command_validation -- --nocapture`:27 top-level tests pass,2.25s; the three subprocess controls belong to one test and are not counted again.
- `cargo test --locked -p agentbaton-workspaces --doc`:4 compile-fail tests pass.
- `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings`:exit0.
- `cargo fmt --all --check`:exit0.

Final hashes were checked after execution and match the frozen1dae4566 source. All15 explicitly hashed implementation logs in EVIDENCE.md independently match. The author's final full90+4 and both Apple-target Clippy checks remain attributed to the author, not claimed as this reviewer's full-library or Mac execution. Both Mac paths compile only; actual13 Mac tests/native evidence must come from fresh exact-source CI. The process/cleanup budget classifies late observations; it cannot forcibly interrupt arbitrary kernel/scheduler stalls or bound synchronous artifact/Git pre/postflight. No vendor/model/account/network/signing/production or whole-product result is claimed.

| Final reviewed file | SHA-256 |
| --- | --- |
| `crates/workspaces/src/command_validation.rs` | `d5fa7b28d6a694c4fa8f767a021499fb5c984a68937cfbf939fcf553cbd4e870` |
| `crates/workspaces/src/lib.rs` | `e396e03e5fbdf99ee4ffa2016240c1771da2892977651340cef2b4f3afdd45a5` |
| `crates/workspaces/tests/command_validation.rs` | `1ec8708817bb72197be4fb44afec25155810e70dc4c0fc6e3973f86fe912cfd5` |

| Final independent log | SHA-256 |
| --- | --- |
| `/tmp/ab-review-command-final-tests.log` | `82f2c5bedafbe38ab0d216b950fa831911692eda48323cf6ae99ab5ec9ded5b0` |
| `/tmp/ab-review-command-final-doc.log` | `204cb5b7a302e220c1d9fbdae49b170f1bca9dad41bd0e4ebc511579cf86f115` |
| `/tmp/ab-review-command-final-clippy.log` | `99f71393b9903dca098310bb446022a4bdee195ea9996f351a787195555210a6` |
| `/tmp/ab-review-command-final-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/tmp/ab-command-deadline-review/fixed-delay.log` | `aa74c87dd5e7ececfd7ebfd8ec4d8e67e35718393988a1acdf61fdc4029d9c5c` |
