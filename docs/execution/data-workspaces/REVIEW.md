# Independent data workspace review

Reviewed frozen `a10c573592af85067bd60a7d2f17aff56e6ccb0f`, base `7ba60e0822129d474be1ade845640d1d72e79b21`. Verdict: PASS for this library unit; both review findings are closed, with no remaining blocker. This is dependency evidence for the approved data-project/validation scope, not complete product or requirement acceptance.

The reviewer edited only this review. Source, locks, manifests and index were unchanged; touching the workspace library timestamp forced correct cross-worktree metadata without changing bytes. Cargo ownership was handed over explicitly and released to the primary after verification.

## Findings closed

**R1: publication metadata was not rechecked before returning Ready.** Original0fba semantics could return a private verified token after Creating or Ready metadata was overwritten during creation. The final flow checks the actual complete Creating binding before writing Ready, then rereads both records after syncing and before returning the token. Failure retains the reserved directory, partial copies and Failed/OutcomeUnknown observation; it does not delete, overwrite, resume or silently reuse contents. The deterministic injection is private cfg(test) code, absent from production. This adds a missing observation, not continuous protection against later same-UID changes.

**R2: resource/path validation order.** Selection count/path/limit checks now precede sorting and root I/O; relative-path UTF-8/length/control/depth checks precede path collection; every checked root enforces canonical bounded shape before canonicalize/I/O. Restore snapshot/count/path/hash-shape checks precede revision-vector construction and completed-record cloning. A missing-source count control and existing private control-character root/path controls verify rejection. The old factory accepted that control-character managed root; the old completed contract already rejected it later, so this is not an old Ready acceptance or a measured allocation/DoS claim.

Both old controls were independently replayed from the preserved actual Linux binaries, without source replacement or Cargo-cache dependence:

| Control | Actual result |
| --- | --- |
| artifacts/data-workspaces/marker-old-source-control publication_tests --nocapture | exit101: two assertion failures, old flow still returned verified tokens |
| artifacts/data-workspaces/root-bound-old-source-control --exact managed_and_relative_path_shapes_are_bounded_before_allocation_or_root_io --nocapture | exit101: existing control-character root factory returned a token instead of error |

Marker control source/binary SHA-256: `7b606f17fd548b02a6f3ab6ae4dd330e352700719d8b936774c51f446b96baf7` / `352096ee70d9add96e8c7a7224aa2275e713bc2457b9b5289ea8dbdbcb3cbf9a`. Root control exact168 source/binary: `3b248f692a3ae34efcb03dfc4d57601be98aac9a22a812a7e64f007b392b94c6` / `be67a5c408db899e81df6c193be80a7eb72ee9f78d33df20733adfe058cce8e1`. The reviewer checked these hashes and the author's retained old/final logs. Historical0fba/168 passing results and the initial Clippy failure remain separate evidence.

## Contracts inspected

Approved selection, managed root, verified snapshot/workspace and host-approved restore tokens are private-field types without Deserialize. Serializable contracts/views, approval strings and disk labels cannot authenticate a caller. Future Core must authenticate the owner decision and fixed persisted contract before invoking these trusted host constructors; this library does not do that product wiring.

Selection is explicit, not recursive: at most64 regular files,64MiB per file and256MiB selected total, with tighter host limits allowed. Root descriptors/device/inode pins, no-symlink/nonblocking component opens, regular-file/single-link checks, bounded reads, file stamps and hashes bind the actual observations. Path traversal, noncanonical shapes, duplicate IDs, case/prefix/inode aliases, known native-auth names and whole HOME are rejected. Filesystem normalization collisions fail exclusive creation rather than reuse unknown entries.

Managed roots must be current-UID mode0700 and pinned. Equal/nested-under-source managed roots are rejected using paths and ancestor identities. An existing source inside the chosen destination would collide with exclusive reservation rather than be overwritten. New directory reservation is atomic mkdir; post-mkdir open/pin failure is OutcomeUnknown, not a no-effect error. New directories are0700 and files0600 with executable bits removed; no source content/config/index/permission write is requested. Normal access-time updates remain possible. No production external program or Git command is invoked.

Ready restoration requires the independently authenticated full saved contract, actual Creating/Ready bindings, every selected destination byte/hash/file pin and intermediate-directory pin/permission. It does not reopen original source and remains valid after source content change, movement or deletion. Unrelated later outputs may remain without becoming approved dataset inputs. Incomplete inspection never promotes a Ready label into authority. Resource bounds and no-overwrite behavior apply across drift, FIFO, symlink, hardlink, replacement and concurrent-ID cases.

## Independent verification

Linux x86_64, umask022. CARGO_HOME=/workspace/.toolchains/cargo; RUSTUP_HOME=/workspace/.toolchains/rustup; its bin on PATH; CARGO_TARGET_DIR=/workspace/.worktrees/validation-integration/target; CARGO_INCREMENTAL=0; CARGO_BUILD_JOBS=2. No concurrent writer or new target/dependency was used.

| Command | Observed result |
| --- | --- |
| cargo test --locked --offline -p agentbaton-workspaces | 111 unique unit/integration tests +7 compile-fail doctests passed |
| cargo clippy --locked --offline -p agentbaton-workspaces --all-targets -- -D warnings | PASS |
| cargo fmt --all -- --check; git diff --check | Both PASS |
| cargo check --locked --offline -p agentbaton-workspaces --all-targets --target aarch64-apple-darwin | PASS, cross-compilation only |
| cargo check --locked --offline -p agentbaton-workspaces --all-targets --target x86_64-apple-darwin | PASS, cross-compilation only |

Count:4 unit+27 Command+19 data+21 JSON+5 Git guard+18 safety+17 validation=111. One FIFO case reexecutes in an owned killable child and appears again in raw output; it is not a112th unique test. All7 doctests really ran. No failures, ignored cases or test-filter-zero passes are substituted for final success.

| Frozen source | SHA-256 |
| --- | --- |
| crates/workspaces/src/data_workspace.rs | d98e0d02af175adc035d97822efbd2008a87b61788ee63d4b60be6d762557e40 |
| crates/workspaces/src/lib.rs | 438827e89aa1bbcb2cf74d118e76569eb18e6be94fe1b6253da98db513defe25 |
| crates/workspaces/tests/data_workspace.rs | c126f32dc951826e8f24f88496dfa6286e74c751e97c09c85e23cf88d6c210cb |
| Cargo.lock | b22c148dd432426f70586093394eeb9f04c0aeca938764be503e1145659f7d42 |
| Cargo.toml | 5dc88c14683dc23feac5ee1d6076ee96fc8f4a8eb149fa62cf35818553522c29 |
| crates/workspaces/Cargo.toml | 303ae03186afbb4da35f45b954efbb91155f6d241c174f130ad89c36df9aa897 |

All source bytes were compared against the frozen Git objects after testing. Locks/manifests also match the base. Author verification.json SHA-256 `4eae9e91df18c9c7c494f406328630a16d9f7a358a70265a6e019442209b5957`; all19 listed logs and both old source/binary pairs matched. Independent manifest `/tmp/data-workspaces-independent-verification.json` SHA-256 `9c28e183344d381697013aad0ed3a5d5a2ac49dae32298068e41175d7df5b68d`.

| Independent local log | SHA-256 |
| --- | --- |
| /tmp/data-workspaces-independent-marker-old.log | 7b532f456c27f18e7c33f552a1f396314a02e881eb01f8750e8f03699631dfe3 |
| /tmp/data-workspaces-independent-root-old.log | eebd0ae1868c8ac88085b7f83082b71fa3a2750bf8b8453b4b843f4f099453cd |
| /tmp/data-workspaces-independent-all-tests.log | 197983747cb6d7c03780867b61bdf29a0d3b2ea0de71fb4bb1d79dff84a559e8 |
| /tmp/data-workspaces-independent-clippy.log | 015371e9352a9ecee0452aa06355f9fa2e76d85130a434942c0f206e44aefbfb |
| /tmp/data-workspaces-independent-fmt.log | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| /tmp/data-workspaces-independent-darwin-arm.log | 56f5f59a307c0e0c567db15f702f7fc87f0f5d026c2e2d04ef52b60f069597c1 |
| /tmp/data-workspaces-independent-darwin-intel.log | 2c7e9edde6a2df484404ff89d68176b8d0cf070f23f482f2ec830478894df624 |

## Remaining validation limits

This is selected-file, before/after observation and host-trusted persistence, not an atomic multi-file snapshot, ongoing freeze, secret-content detector/DLP or OS sandbox. Defensive auth-name rules are not universal credential detection; the host must choose legitimate dataset paths and authenticate saved contracts. Same-UID/privileged actors can change files after observations. Hashes establish versions, not caller authenticity or scientific correctness.

No Core registry/IPC/leases/UI/Goal or non-Git runtime integration was exercised here. No TaskAccepted, Git adoption, complete data-execution/validation closure, actual macOS filesystem/runtime/package or new CI success is established. Darwin checks compiled source/tests only. All fixtures were generated locally; no real credential/auth content, Provider/model/account, extra paid service or production action was used. These remaining integrations continue the approved scope rather than being declared complete by this unit.
