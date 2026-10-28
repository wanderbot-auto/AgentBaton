# Independent Goal editing-context review

Initial reviewed Core source `5f4b0dea34d04d9bc9ea6f1e17305aee87f3b2c1`, combined HEAD `76ec9b1ea4cff3ea09549ee2aa2a1eb414edb61c`. Status: BLOCKED pending the real persisted-legacy-result compatibility fix described below. This is a Core DTO review, not UI/native dialog, browser, dispatch or macOS acceptance. The reviewer changed only this review and temporary probe/log artifacts; no repository source, lock or index was changed.

The new optional display DTO projects max_nodes, writable_prefixes and complete output requirements from the recorded selection and owner/validator executor kinds from the fixed host policy. It does not expose a private scope token and is not accepted as authority by the raw plan submission API. Historical projection and current live-action checks remain separate. Missing local-cache fields deserialize as None, but that alone does not make old persisted approval results readable.

## Blocking finding: persisted legacy results fail the full-result comparison

recorded_scope_view now produces edit_context=Some for every stored scope, while stored_scope_view still compares its complete projected JSON with the original native approval_proposals.result. Legitimate schema9 results produced before this additive DTO field lack the key. The full comparison rejects them with INPUT_CHANGED. The supplied test removed the field only from an in-memory DTO and tested deserialization; it did not exercise this private persisted-result path.

Independent initial command `cargo test --locked --offline -p agentbaton-core --test goal_store -- --test-threads=2` actually passed all32 tests under umask022, CARGO_INCREMENTAL=0/JOBS2 and the explicitly handed-over shared validation target. The reviewer then compiled a temporary standalone rustc --test probe against that freshly tested library, reusing fixture setup only. It approved a scope and persisted a plan, removed only result.edit_context from that actual Goal proposal row to reproduce the old result shape, and checked scope get/list, historical plan list and ready preview. All four returned INPUT_CHANGED; the probe failed101 and confirmed the event sequence had not changed. No original selection, permission, policy, digest, grant or journal was altered by that injector.

The primary has accepted the finding and is preparing a minimal comparison compatibility change: only a genuinely absent additive field may be omitted from the expected comparison. All original fields and present context must still match; null must not be treated as absence. No old record, cached body, authorization or digest is to be rewritten. Real persisted legacy and corruption controls will be reviewed after the new source freeze. No corrected-source test result is claimed yet; the shared target was formally released to the primary after the failed probe.

| Initial source/evidence | SHA-256 |
| --- | --- |
| crates/core/src/goal_store.rs | 6bb8f618d7c56a47855b431e3e908250a2b3abc441a1c68b8ccad48a8e5fa2e0 |
| crates/core/tests/goal_store.rs | d2181ef025a3b98f6ff4ddb94329970da8776469b33616b13d2ff6a5da4e8228 |
| /tmp/goal-editing-context-independent-initial-tests.log | eff6de8acd8094eff4502d1440ccd9d13f161fc5227752b8b941843ff3942893 |
| /tmp/goal-editing-context-independent-legacy-probe.rs | 1478206e9773359af38a60b5a2c7a94098433627e192b17212888a44f211a0f7 |
| /tmp/goal-editing-context-independent-legacy-old.log | 7da05717af981b79b2921bbe5dab11302c399c95320c3431c155a905aadac3e5 |
| /tmp/goal-editing-context-independent-legacy-old-binary | cc9a92e8eb57428e8153c14fbdf667e190641536e653b0dfb3afca9f367b8379 |

Exact old source is also preserved as /tmp/goal-editing-context-initial-source.rs. The old binary remains independently replayable without a source swap. Other existing untracked UI/node_modules/review paths belong to other authors and were untouched. Existing Backend trust/validation limits remain: private SQLite and trusted host confirmation are fixture premises; no real Provider/account/model, Owner OS dialog, actual editor-to-Core UI mode, new macOS runtime/CI, reservation, dispatch or TaskAccepted was tested here.

## Compatibility follow-up: source 589b1a8

Final reviewed freeze `589b1a82d5def8302b9ea6a585695cfe95b7e892`. Verdict: PASS for this Core DTO/compatibility unit; the blocking persisted-legacy-result finding above is closed, with no remaining blocker. This supersedes the initial candidate's blocked verdict without changing its failure evidence.

The fix changes only comparison of the newly added display projection. If and only if the actual persisted result lacks the edit_context key, that field is removed from the freshly derived expected JSON for comparison. Every original result field still compares exactly. A present edited context or explicit null is not treated as an old missing field. Scope records, original native approvals/cached bytes, policy/digests, grant permissions and journals are not rewritten. The returned current projection is derived from the already verified recorded selection/policy; it is not authority reconstructed from cached JSON.

Real persisted-result regression coverage now includes scope get/list, plan history, readonly preview, consumed finalize replay, byte-for-byte equality of SELECTed cached result before/after, old cached DTO decoding to None, historical reads after revocation and continued preview rejection. Separate corruption cases cover an altered legacy root budget, a present forged write prefix and present null context; all reject with INPUT_CHANGED and no event-sequence change. Existing forged-client-context write-set rejection and original history/approval/CAS controls remain in the full GoalStore batch.

The primary independently ran the new regression against the old code and preserved an actual INPUT_CHANGED/exit101 failure. Its first corrective run also remains:33 passes/1 failure because the assertion examined a regenerated readonly finalize response instead of the actual persisted cache. The production fix was not changed for that test mistake; the final regression explicitly SELECTs and compares the stored JSON bytes. The reviewer read those logs and verified their hashes, distinguishing both from the final independent success.

Independent final validation, same explicitly coordinated environment as the initial run (umask022, CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2 and shared validation target):

| Command/control | Observed result |
| --- | --- |
| cargo test --locked --offline -p agentbaton-core --test goal_store -- --test-threads=2 | 34 passed,0 failed/ignored/filtered |
| Original /tmp legacy probe source recompiled with rustc --test against the freshly tested fixed library, then executed unchanged | 1 passed; all four formerly failing APIs return Ok; event sequence unchanged |
| cargo clippy --locked --offline -p agentbaton-core --all-targets -- -D warnings | PASS |
| cargo fmt --all -- --check; git diff --check | Both PASS |

The independent probe source SHA remains `1478206e9773359af38a60b5a2c7a94098433627e192b17212888a44f211a0f7`; the old failed binary/log remain intact. It was not rewritten to fit the fix. The additional probe is separate from the34 repository-test count. Cargo was formally released to the primary after all checks, with no remaining writer.

| Frozen source | SHA-256 |
| --- | --- |
| crates/core/src/goal_store.rs | 1b040978a6de912a69e6c7d5d82588965fc7291a48774b60e0019157f377d539 |
| crates/core/tests/goal_store.rs | f2262514d84f51cba76253c060ed47580a5b3efb4ec4e04d25598d031e94e454 |

These source bytes were checked against the frozen Git objects before/after verification. Root/native locks and relevant Cargo manifests remain unchanged from76ec9b1. The author's four final Store/Clippy/fmt/sidecar log hashes were verified; their UI/build claims are separate evidence, not independently executed by this reviewer. Independent manifest `/tmp/goal-editing-context-independent-fixed-verification.json` SHA-256 `a5bbc4b4f70e83963753c6a66f1438c94c5e795df8e9c788745654034db8a66f`.

| Final independent / preserved control log | SHA-256 |
| --- | --- |
| /tmp/goal-editing-context-independent-fixed-tests.log | 06f9e24441ca5f1462b9f4b9f0ebf90275866e854408da037f7fb1ad05587714 |
| /tmp/goal-editing-context-independent-fixed-clippy.log | 132c791a1583385177cf54549e5d6018cb6396a1db2ecfe4bd17be38b33e1d8c |
| /tmp/goal-editing-context-independent-fixed-fmt.log | e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |
| /tmp/goal-editing-context-independent-legacy-fixed.log | 8be8ebb90a8ec541fe304692c41b9810b0d9dc638e5214cb1372da045f54aca4 |
| /workspace/.onboarding/goal-editing-context-persisted-legacy-old.log | cb36492d7265df966f5174e2bb3bba6324f1a3214c093f4a3255637f2cd92614 |
| /workspace/.onboarding/goal-editing-context-compatible-store-tests.log | 20f8abb479786878778ff3a32438619d77562fe0d9ca507dddb7f802bbfa31f7 |

No full-Core total, UI259/browser/editor mode, native Owner dialog, actual macOS runtime/CI or sidecar build is newly claimed by this review. The narrow34-test batch includes the relevant history/approval/live-action regressions and all-target Clippy compiles the remaining Core targets; these are appropriate to the additive DTO and comparison change. Existing private-host/SQLite trust and the unimplemented planner/dispatch/reservation/TaskAccepted boundaries remain. No real Provider/model/account call, private credential use or paid/production action occurred.
