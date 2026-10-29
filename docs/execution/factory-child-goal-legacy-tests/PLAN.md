# Factory child Goal legacy migration tests

Authorization: AB-START-001, Design v0.2. Assigned WP01/WP08/SEC/COR migration evidence only. Baseline `5491c94cffbe40292ff977e0d9cfedcb55026c9e`; current merged main `3fd60f354236ddc5ab0ea2f7665428a8496f4a64`. Latest merged OWNER_INBOX entries 001 and 002 acknowledged; no 003 observed. No authorization expansion.

Allowed files: new `crates/core/tests/factory_child_goal_legacy_migration.rs`, its new `fixtures/child_goal_legacy_migration/` subtree and these records. Coordinator subsequently expanded this list solely for `crates/core/Cargo.toml`'s empty test feature/dedicated target and `scripts/prepare-legacy-migration-fixture.py` plus its pure tests. No dependency/lock changes. Core implementation/migrations/protocol/existing fixtures/CLI/UI/CI/locks/global STATUS are excluded. Core author is sole schema writer. No remote writes.

1. Prepare fail-closed tests against real nonempty schema14 input and a bounded generator using frozen f032 tools. Use actual private authenticated Core approvals, not synthesized Native authority.
2. Obtain the coordinator's process window; generate real Native Goal scope, Graph execution approval/capability and terminal joined Graph, plus separate real Factory14 scope/run/child plan. Retain exact RPC proposal/result bytes and ordered original database rows. Capture old daemon source/hash/manifest and clean shutdown.
3. Freeze original input. Run actual schema15 Store::open integration tests in the coordinator's Cargo window. Require original raw rows, original Native proof FKs, valid foreign keys, private original14 backup, rollback, corruption refusal and future16 refusal.
4. Record separate active/unknown recovery and derived-issuer cases as outstanding until backed by actual input/execution. Review independently, fix, and return isolated commit to coordinator. No overall product completion claim.

There is no public migration-only entry point. This suite deliberately calls Store::open, which performs migration and legitimate recovery. Its immutable-history fixture requires terminal joined work and expects zero recovery changes. The pre-v15 backup proves original14 raw history separately. It does not claim an independent DDL-only execution or active-worker recovery evidence.

## Dedicated target integration (PENDING, coordinator owns Cargo/CI)

The six tests have no ignore/skip/missing-input success path. They consume only an explicit private0700 `AGENTBATON_ACTUAL_SCHEMA14_FIXTURE` directory, require original source/pair hashes, real initial14 probe, clean joined processes, actual nonempty proofs, data SHA and foreign keys. Original generated DB/RPC/capabilities are not committed or public-uploaded.

Coordinator explicitly added the following `crates/core/Cargo.toml` gate to this agent's allowlist; it is now implemented, without dependency/lock changes:

```toml
[features]
actual-legacy-migration-tests = []

[[test]]
name = "factory_child_goal_legacy_migration"
path = "tests/factory_child_goal_legacy_migration.rs"
required-features = ["actual-legacy-migration-tests"]
```

Ordinary suites explicitly report these six as NOT_RUN; they are not included in ordinary659+7 counts. A separate required CI step must generate genuine14 input and run this exact target with the feature and private fixture env. CI integration is PENDING; this is not permission to omit the migration gate from acceptance.

Rebuilding f032 on another platform/path/toolchain produces new measured hashes. A separate explicit source-build contract is prepared below; the original frozen-pair mode and run2 data remain unchanged. New build hashes cannot be relabelled as the original frozen hashes. No CI reproducibility execution success is currently claimed.

## Linux reproducible input contract (SOURCE_READY, actual build/CI NOT_RUN)

`prepare-legacy-migration-fixture.py` fetches only public historical commit f0322aa2a098c50ebc86e2fc5d52c3f62cb1c701 and checks tree93ce75df286770a9f196994ed9f5c9a11528cc5e. It archives root Cargo.toml/lock, crates and daemon/CLI sources, avoiding all historical docs/installers. Exact f032 manifests confirm package names batond/baton; the normal build command uses the established `--bin batond --bin baton`. Source inspection found library include_str/bytes references only within archived crates/fixtures and no workspace build.rs.

The builder uses a caller-specified shared target BEFORE normal15 builds, locks dependencies, disables incremental/dev debug, records actual cargo/rustc and source/archive/file hashes, verifies compiled-source cleanliness, copies distinct0500/nlink1 normal tools, then actually probes schema14/zero tables/FK/companion snapshot and clean owned SIGTERM join. Failure records and private runtime survive; no prebuilt download, invented hash or synthetic DDL/approval. Its `agentbaton.schema14-source-build.v1` manifest contains measured platform-specific hashes. Generator requires explicit `--build-manifest` to accept that mode; Rust independently rechecks source identity, clean build, tool/command/probe/join metadata and exact hashed manifest. Legacy no-flag mode still pins the old actual Linux pair.

Unexecuted CI call contract (Root owns YAML and mandatory gate):

```sh
python3 scripts/prepare-legacy-migration-fixture.py --repository "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/agentbaton-legacy14-build" --target-dir "$CARGO_TARGET_DIR" --cargo "$(command -v cargo)" --rustc "$(command -v rustc)"
python3 crates/core/tests/fixtures/child_goal_legacy_migration/generate.py --frozen "$RUNNER_TEMP/agentbaton-legacy14-build/frozen" --build-manifest "$RUNNER_TEMP/agentbaton-legacy14-build/frozen/source-build-manifest.json" --repository "$GITHUB_WORKSPACE" --output "$RUNNER_TEMP/agentbaton-legacy14-fixture"
AGENTBATON_ACTUAL_SCHEMA14_FIXTURE="$RUNNER_TEMP/agentbaton-legacy14-fixture" cargo test --locked -p agentbaton-core --features actual-legacy-migration-tests --test factory_child_goal_legacy_migration -- --nocapture --test-threads=1
```

Caller must provide an absolute shared CARGO_TARGET_DIR and explicit real cargo/rustc paths, clear compiler flags/wrappers, and preserve each command's real nonzero status. Builder is Linux-only (/proc identities); Mac true14 rebuild/migration is NOT_RUN. Root must build/freeze14 before normal15 tools, then verify normal15 preparation after shared-target overwrites. CI artifacts may contain redacted counts/hashes/build records and test results only; never raw fixture DB/WAL, original RPC/capability manifest, runtime token or scoped handle. Rust raw-row equality failures retain private DBs and print only diagnostic references, not capability rows.
