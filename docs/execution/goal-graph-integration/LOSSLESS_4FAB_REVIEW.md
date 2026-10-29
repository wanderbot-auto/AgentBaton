# Independent lossless observation review — PASS, source and Node scope

Reviewed exact application source **4fab558fe126543c23b1410949e6fcd922b770d6** in detached `/workspace/.worktrees/goal-observation-lossless-review`, following independently reviewed733 and test-only8a. Final documentation **2f31fcc7f88d9a1d90442ed5b1120903d5850467** changes documentation only; apps bytes match4fab exactly. No blocking finding remains in this lossless-display iteration. This conclusion does not borrow earlier runtime/CI green and is not actual Core/Native/macOS acceptance.

## Actual independent verification

Using existing dependencies with reviewer-owned symlinks, private config/cache and a **new dedicated build output** under this directory:

| Command | Actual result |
| --- | --- |
| `vitest run --config /workspace/.onboarding/goal-validation-observation-ui-review/lossless-4fab/vitest.config.mts --reporter=verbose` | exit0, **564 tests /37 files**, 31.41s |
| Same config, selected `src/goal-graph-observation-text.test.ts src/GoalGraphExecution.lossless-observation.test.tsx` | exit0, **14 tests /2 files**, subset of564 |
| `tsc --noEmit` | exit0 |
| `vite build --config .../lossless-4fab/vitest.config.mts` | exit0; dedicated `lossless-4fab/build/`; existing >500kB chunk warning retained |
| `git diff --check 87b4434a032fc2995facdee4761d877fa79b7770 4fab558fe126543c23b1410949e6fcd922b770d6` | exit0 |
| `rustfmt --edition 2021 --check .../approval.rs` | exit0, standalone parse/format only, no compilation |
| Independent extra exact-module tests via `independent.config.mts` | exit0, **18 reviewer unit controls** |
| `preservation_audit.py` and author evidence audit | exit0 |

564 includes the preserved547 tests, test-only8a's3 and lossless4fab's14. Selected14 is not counted a second time. All original **34** renderer files from733 and six original fixtures are byte-identical; the additional8a test file and its two provided fixtures also match8a exactly, so **35 preceding renderer test files** are preserved. The original Graph execution component/intent reconciliation paths are unchanged; old R1/R2 and unknown outcome/mode/floor controls execute in the full suite. Candidate precision, artifact/summary/CAS/task-binding and node/reference consistency source segments are byte-identical; graphDetailsShape and graphViewSupported normalize back to733 by replacing only their observation-specific predicate. There is no installed dependency, lock, CI, parent, target or authority modification by this reviewer.

## Correct boundary for the raw text

The new `observation_json` is a **derived readonly GraphDetails display field**. It is optional for older safe projections. The UI never sends it as Owner authority or writes it to storage. The exact provided Core graph source reference was located in Git history **364fc3cbc8a417914f1e9c98bdfcc1ae10e53ae9**, file SHA **14ab9d6a578142356bdd20aa227a9d0c88bc2a7656c88b9d2de05f8b0cebd780**, matching both new packets' build-source hashes. `Core-derived-view-reference.rs` is a read-only contract reference: GraphValidationView flattens the original receipt, adds encoded complete observation text and checks its raw SHA against result_digest during view construction. Original receipt/storage/event/Owner snapshots retain the original Value. Current live Core source has advanced; it was not substituted for that frozen reference, compiled or rerun in this review.

`goalGraphApi.get` now awaits validation of each supplied raw field before returning the same view object. It preserves the existing 4MiB view cap and65536 validation count cap. Verification hashes **the original UTF-8 text** with WebCrypto SHA-256, compares the Core-provided result_digest, parses only to check agreement with the renderer's rounded JSON projection, then rechecks text/digest/projection after the await. JS re-encoding is never the digest input. Invalid JSON, changed raw bytes, wrong digest, projection disagreement, unavailable/rejecting/wrong-result crypto and changes during await fail closed. No additional RPC or mutation is issued on verification failure. Summary list and narrowing cancellation stay independent of crypto availability.

The module-private WeakMap stores only a successful **current-object/raw/digest/projection relation**. It is not a credential, permission or substitute for an authenticated Core response. Clones cannot inherit it. Every supported/shape check rechecks that relation; changing or deleting raw, changing digest or observation invalidates support. Failed re-verification deletes an earlier cached proof, even if the caller subsequently restores old bytes. The independent18 controls exercise shallow and structured clones, restoring/reverifying, every async raw/digest/projection mutation, missing/rejecting/wrong crypto, and strict candidate FilePin/CAS/sequence precision. An opaque observation may contain an unsafe integer only with verified original raw; this does not permit unsafe origin pins, budget counters used as authority, run revisions or sequence numbers.

No new action endpoint exists. Existing approve/reject/advance/cancel calls still carry fixed references/CAS/keys only; Native/Core fetch and recheck the original server proposal. The unchanged Graph component awaits API get, runs shape/origin/floor checks, and retains its verified live detail object for actions. Its structured clones are evidence floors, not portable verification credentials; they cannot silently authenticate new objects. Thus asynchronous display verification does not grant renderer authority.

## Raw history and immutable evidence floors

History shows the exact raw string through React text content in `<pre data-testid="goal-graph-original-observation">`, rather than pretty-printing its parsed rounded observation. The envelope retains receipt metadata/digest and the derived string while omitting the redundant rounded observation from that envelope display. Existing folds remain readonly. Actual jsdom tests verify exact original text, no buttons/links, and literal HTML-shaped metadata stays text with no img/script element; this is DOM execution, not a screenshot or WebView run.

Floor comparison strips **only observation_json** from validation receipt comparisons and final same-revision equality. It still compares the original observation, complete receipt, result_digest, unknown receipt fields, origin references and other evidence. This allows the same-CAS derived display upgrade without rewriting a persisted receipt. Independent controls reject real observation/digest/unknown-envelope forks. They also explicitly show that a raw-only tamper can meet the display-independent floor **in isolation**, but shape/consistency then fail without verified raw, and no Owner action becomes available. Floor equality alone is not verification. This separation preserves the original R1/R2 intent and evidence safeguards.

## Exact actual packets, including the unsafe system Command

All four supplied packets were independently byte-compared against their original Core artifacts; no inode/program path was edited to produce a safe fixture:

| Packet | Bytes | SHA-256 |
| --- | ---: | --- |
| Original Data without raw | 119716 | `33c63615f18d89b6201e2a603f16066ce72d1e415868a90403ee3f3aaa9867cd` |
| Original Command without raw | 94218 | `592ac8036eb9d1c59ffbdf4119612eb3ffcf6bac960b92e8cf8fc3092be04f89` |
| Lossless Data | 127780 | `97c07926adf05f4bc04bcbeed97adc6d6aea59cbbcaa10d6aaeebad19a456d99` |
| Lossless system Command | 102705 | `526a42e1775785b82eaf420af9dd6fd9a3f3c299f885f1535ab23835a812ed19` |

Lossless Data raw text is7355 UTF-8 bytes, SHA/result_digest **cb872993e99b37b383682a8ca4421bfc0ef59a3974677c91e4301c790bdbc17f**. System Command raw is7818 bytes, SHA/result_digest **0db7e71a370495d6f92cce98ebe5fe0118a361f5fc28d4c23dca09d6245c510d**. The Command still describes canonical_root `/usr/bin`, program relative_path `printf`, and original u64 root_inode **9223372036855074700**. Its parsed Number is unsafe and its re-encoded JSON differs from the original raw text. It passes the API/shape/Task-binding tests only after actual raw-byte digest verification. The previous592 packet **without raw remains an unsupported negative**. This is not a safe-inode/program-location workaround.

Python independently hashes the provided original raw UTF-8 and checks full parsed Value equality, while Node executes the new API verification. Each provided GraphDetails receipt, excluding only its new display field, equals the original complete validation_ref in the matching Owner proposal. That proposal has no observation_json; the actual first Owner receipt remains bound to its original approval ID. Validation remains `source=baton_goal_validator`, `task_acceptance=not_established`, stop=true/worker_joined, with bounded scope/correctness: Data `data_contract_only` / `approved_schema_ranges_metadata_only`; Command `approved_deterministic_command_only` / `approved_fixed_command_observations_only`. These checks do not independently establish general semantic correctness or authorize GoalAcceptance without the separate required Owner criteria and Core acceptance chain.

Original f64/u64 **text** can be hashed and shown exactly. Parsed JS Value agreement is only projection consistency; JSON.parse→JSON.stringify is not a general Rust original-byte algorithm. The earlier Integrity-only digest claim remains limited to its known values and is not silently broadened.

## Native, failures and artifact incident

approval.rs SHA **9e5e68431d43a3435f539bac465af998fe300457984ea953a2d402c982399983** is byte-identical to8a. Relative to baseline87 there are **four new callback functions total**:733's three Integrity functions plus8a's one function iterating actual Data/Command packets.4fab adds none. All four remain **source-only / NOT_RUN** in this review. Original36 tests remain, total40 source test functions. The extra8a callback reviews full nonempty validation refs/observation Values, checks the exact fixed finalize frame, returns the corresponding preserved actual first Owner receipt, and checks Cancel does not write a rejection. It does not create a new Core success, invoke an OS dialog or establish Native compilation.

Preserved failures include733's old missing/null/unsafe controls,8a's actual592 initial3PASS/1FAIL with original source, and the lossless asynchronous-mutation bug's actual1FAIL/10 diagnostic-filtered skips plus pre-fix guards. Final full564 has no skipped acceptance tests. Relevant raw failure files are retained here; the original733 review and its53 listed files still rehash exactly.

The author **did touch** the older private build index when the first lossless build reused that output directory. The shared-output build command/log and explicit correction record are retained. The index was restored against its previously recorded SHA, and this reviewer independently rehashed **all47** original frozen Integrity artifacts to their original hashes/sizes. This proves current byte restoration; it does not claim the old directory was never touched. The author and reviewer final builds use separate new output directories. No product source/dist was affected by that evidence-output incident.

Final author manifest SHA **e754ed7d36dce62f6019dfcebb5a9c7270e53908968797f456e3e71db7be9a7e**, verification **a34c984d98551ac2b342c00b3f0441bff062d444609ae855d7e72607d9be56ce**: all **39** listed files independently rehashed. Exact final docs2f31 accurately state source-only Native, author-supplied Core packets, raw-text hashing, async failure correction, output restoration and no old green borrowing.

Not run/accepted here: fresh composed Core binary/real Unix IPC with this UI, actual browser or macOS WebView crypto availability, OS/native Owner dialogs, Native/Rust compilation or callback execution, full Graph failure/recovery matrix, live Provider/model, CI, release audits, dual installers/signatures, or full Design v0.2. No Cargo/target, dependency install, source/CI/remote action, formal approval or permission expansion occurred. Root must execute the fresh composed runtime/Native/macOS/CI/package gates.
