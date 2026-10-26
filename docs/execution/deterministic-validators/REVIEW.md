# Independent deterministic JSON contract review

Reviewed source commit `cbd55093d58a95c5876591e1f57f44977f919fe9` against `9895ba6`, after the implementer froze source and released the shared Cargo target. No blocking correctness finding remains in this library increment. This review does not accept a whole task, scientific result, Git integration candidate or the complete Design v0.2.

The reviewer changed only this document and ignored independent test logs. Source, locks, index and Git refs were not changed by the reviewer. AB-START-001 and OWNER-001/002 were read; no Provider, model, credential, production or paid-service call was made.

The six-file source diff is confined to the new module/tests, one public module declaration, a workspace `serde_json` dependency edge using the already locked package, and making the existing `BoundArtifact::read` method `pub(crate)`. Its descriptor-reading implementation is unchanged.

| File | SHA-256 at reviewed source |
| --- | --- |
| `crates/workspaces/src/deterministic.rs` | `44e8feda9a9dbd91d7fb5db1ed0c38424ef380fbb8fc3ec565ea3f1591a97c26` |
| `crates/workspaces/tests/deterministic.rs` | `340f91af93a8550712b2c96daa1e890065843e4ddcf09c4c40907073c55cb286` |
| `crates/workspaces/src/validation.rs` | `92ea96d82f7711bca1e5112a59fdbd9f0617df0ce887ce9f55e2c8ac25cd4c83` |
| `crates/workspaces/src/lib.rs` | `c1757055bb2c5adbaf8ab50aab25f533c1f2e64ecfd549084f1defbcd6de435b` |
| `crates/workspaces/Cargo.toml` | `303ae03186afbb4da35f45b954efbb91155f6d241c174f130ad89c36df9aa897` |
| `Cargo.lock` | `b22c148dd432426f70586093394eeb9f04c0aeca938764be503e1145659f7d42` |

Authority remains a trusted-host responsibility. The serializable proposal is not approval; `ApprovedJsonTableRule`, the actual evidence and `JsonTableContractAcceptance` have private fields and no `Deserialize`. The policy-content digest binds the validator version, complete schema/limits/analysis declarations, task/attempt/spec/input/dataset subject and all root identities, paths and expected input hashes. The receipt digest and explicit acceptance checks additionally bind the approval ID and policy revision; those two fields are not directly included in the policy-content digest. The constructor validates proposal counts/string lengths before sorting or cloning its table/analysis strings. A public constructor named `from_verified_policy` authenticates no caller itself; future Core wiring must resolve a current authenticated owner grant and approved root mapping before calling it, and bound proposal transport before deserialization.

The reader pins the opened root device/inode, traverses components with `openat`/`O_NOFOLLOW`/`O_NONBLOCK`, checks regular files and single links, bounds bytes and checks file metadata around reading. New checks reject input/output path aliases even across nested approved roots. The JSON table is read again and its hash compared with the integrity observation; all bound inputs/outputs are rehashed after validation and before contract acceptance. Schema/policy/subject/root/path/hash drift cannot reuse that receipt. This is content verification at observed times, not a multi-file transaction or filesystem sandbox.

Parsing uses the existing `serde_json` parser with a bounded visitor. Maximum policy ceilings are 4 MiB per artifact, 16 MiB aggregate declared artifact limits, 16 inputs, exactly three outputs, 64 columns, depth 32, 200,000 values plus object keys, 16 KiB decoded strings and 20,000 rows. Unknown columns/root fields, duplicate decoded keys (including escaped aliases), malformed Unicode/JSON, trailing input and non-finite numbers are rejected. Diagnostics contain fixed pointers or approved column names rather than observed values. The 21 new tests exercise exact byte/depth/value/string boundaries, numeric boundaries, required/optional/null fields, metadata, alias/path/file failures, receipt mutation and stale files; existing workspaces regressions also passed independently.

`Integer` checks exact signed-i64 representations without float coercion. During review, the conservative rejection of JSON `-0` was raised; the final source explicitly documents it and tests `-0`, `0.0` and `0e0` rejection. `Number` deliberately uses finite IEEE-754 binary64 semantics, including its rounding/underflow behavior, rather than arbitrary-precision decimal comparisons. Units and timezone are approved strings matched exactly, not dimension analysis or timezone-database validation. Full/sample scope, limitation text, population/checked row counts, seed, all observed input digests and script/environment hashes must match the contract metadata. A sample is not upgraded to population evidence. These declarations do not prove representative sampling, correct algorithms, effective dependency versions or successful reproduction. Script/environment artifacts are hashed and never executed.

Independent commands ran on Linux under `umask 022` with `CARGO_HOME=/workspace/.toolchains/cargo`, `RUSTUP_HOME=/workspace/.toolchains/rustup`, its Cargo bin on `PATH`, `CARGO_BUILD_JOBS=2`, and coordinated `CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target`:

| Actual command | Result | Independent log SHA-256 |
| --- | --- | --- |
| `cargo test --locked -p agentbaton-workspaces` | PASS: 61 unit/integration tests (2 library + 21 deterministic + 3 lazy-fetch + 18 safety + 17 validation/adoption), plus 2 compile-fail doctests; no failures/ignored/filtered tests | `47ce36686a545f370ce9332a7c694743ac5df47e7eb35e51a815ca1f79ac9219` |
| `cargo fmt --all -- --check` | PASS, exit 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |
| `cargo clippy --locked -p agentbaton-workspaces --all-targets -- -D warnings` | PASS, exit 0 | `8c233a54790c4800d3dd585436af679aa838da259bfdb129412e2d51cdb75cdc` |

Logs are `/workspace/.onboarding/deterministic-review-{tests,fmt,clippy}.log`. The six source hashes were checked both before and after those commands and did not change. `git diff --check` also passed. The implementer's original failed string-budget fixture run remains in `artifacts/deterministic-validators/initial-tests.log`; it is historical failed evidence, not a passing final run.

This module has no Core/IPC/storage/UI/native integration in this diff. This review therefore proves neither current-policy authentication/revocation in a running app nor durable receipt replay, UI labels, macOS portability of the new increment, real Provider behavior, automatic TaskAccepted, Git adoption or production release. Follow-up integration must retain the explicit data-contract-only acceptance meaning and rerun checks against its actual composite source.
