# Canonical approval executable fixture follow-up

Source `8441fe6e509283c904e70ffc0a65767e3ffe5412`, based on frozen83f0d88. Test file only; production IPC/Store/sessions/policy, locks, all original outcome/replay assertions, permissions and time bounds are unchanged. No Root/Data shared-tree edits, push or merge by this worker. Parent assigns separate independent review; these are implementation notes.

## Actual failure and control

Read the retained real ARM log `/workspace/.onboarding/provider-2da7-job-121917839562.log`: all3 new approval_outcomes fixtures fail during connection proposal/registration with INVALID_REQUEST requiring an existing canonical executable, before reaching their outcome assertions. This is not a failure of the post-approval confirmed mapping. The actual run API identifies head2da7da2d5fd24bc46b0c39967b2118b2bf975181, run40421813961; the overall run was still in_progress when inspected, so no final aggregate conclusion is asserted.

The helper had returned the path assembled from TempDir directly. A temporary-root alias such as Darwin's /var→/private/var remains noncanonical despite referring to the same generated file. To exercise this on Linux, the existing connection-confirmation case now supplies an explicitly created selected-root symlink pointing only to its own generated temporary directory; assert_ne verifies the alias is genuinely noncanonical. With the original helper this control actually fails101 at propose_connection with the same canonical-executable rejection. Original failure log is retained. This demonstrates the alias class locally; fresh Mac CI still must confirm the correction there, and no unlogged runner path was invented.

## Minimal correction and retained meaning

The helper now returns `path.canonicalize().unwrap()` after creating the same0700 generated file. Only the fixture selects the canonical target; the production canonical/no-follow executable checks remain untouched. The controlled alias remains permanently in the existing connection case. No skip, permissive canonical guard or OS-only conditional was added. The helper's program remains unexecuted: all original absolute no-execution marker assertions are preserved.

All3 tests actually pass after the correction, including the real connection proposal, rejected approval-confirmation journal, unknown result, same-proposal original-ID recovery and single confirmation receipt. The grant and authenticated probe-refusal cases retain all original assertions, including confirmed existing-connection details, before-effect not_started and no duplicate approval/key. This fixes test input preparation rather than accepting the negative proposal response as success.

## Commands and limits

Explicit umask022, existing quiet-canvas Cargo target, Linux:

- Original-helper alias control, exact existing connection test: exit101,1 failed/2 filtered, retained as failure.
- `cargo test --locked -p agentbaton-core --test approval_outcomes -- --nocapture`: exit0,3 passed,1.22s test execution.
- `cargo fmt --all --check`: exit0.
- `cargo clippy --locked -p agentbaton-core --all-targets -- -D warnings`: exit0.
- `git diff --check`: exit0.

Earlier full Core83 results are retained as historical evidence on identical production code. No redundant full Core run, fresh native Mac pass, installer, Provider/account call or production change is claimed for this fixture-only commit. Parent must review and integrate before exact-head CI retry.

## Fingerprints

| File/log | SHA-256 |
| --- | --- |
| `crates/core/tests/approval_outcomes.rs` | `dbc6adc0de7e2c086e31a022f52df374cf7fc3c7c6d3f71e1ccc53e3acf96a67` |
| `/workspace/.onboarding/provider-2da7-job-121917839562.log` | `a0d6729503530ef816413cb9ebe0614c6bcb2533a8c52b35bebc14e93914cc9b` |
| `/workspace/.onboarding/approval-outcome-guard/alias-before.log` | `1f29a82cf941858aaca30d9e29e353413fa2d0e8de7e4c557ae9edc11376de24` |
| `/workspace/.onboarding/approval-outcome-guard/alias-after.log` | `44ffa4db2b000059a3513567320b2481a613e1a02b0e7a136af546d2bbf5e575` |
| `/workspace/.onboarding/approval-outcome-guard/alias-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `/workspace/.onboarding/approval-outcome-guard/alias-clippy.log` | `dcfb54745bfeb86d5469232b57cb45b251a17b7217730acc72d6e04be431e256` |
