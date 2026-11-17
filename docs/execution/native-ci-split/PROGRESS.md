# Progress

Exact c3 base and clean worktree verified. Baseline tracked-file hashes and
original workflow bytes saved under ignored artifacts/collab/native-ci-split.
OWNER-001/002 read; no new scope changes. Implementation is in progress.

Verified the official actions/download-artifact v4 tag through GitHub API:
`d3f86a106a0bac45b974a628896c90dbdf5c8093`, commit object; read its exact
action.yml and artifact-ids/current-run semantics. Existing action pins stay.

Source abf5d1ad was committed before validation. Its 56 targeted checks had
55 pass and one remaining byte-reader adapter error; raw infrastructure
validator passed. Corrected the explicit read_bytes entry in 7cd7ce55.

Full infrastructure on clean 7cd7ce55 ran 907 cases: 16 skipped, six errors,
two failures. Five errors trace through the still-frozen background reader;
the workspace diagnostic inverse needs a literal read-entry adapter. One
failure is missing jsonschema in the initial interpreter. Tracked input hashes
were identical before/after. Raw failures remain in ignored task artifacts.

Independent reviewer found cancelled() in unsupported env expression locations
and matrix.os in a job lacking a matrix. Both are corrected using supported
step conditions and a single ARM matrix within the ordinary 30-minute cap.
Additional filesystem tests cover actual reconstruction, probes and packages.

Created task-local .venv using the unchanged requirements-infra.txt. Initial
configured mirror failed TLS verification; retained that failure. Official
PyPI with system CA verification succeeded without disabling TLS or changing
global configuration. Full rerun waits for a new committed source snapshot.

07f8dc531db8fdf17c69d025a09d30278cde2315 passed all 66 targeted tests and
the unmodified raw infrastructure validator, with identical tracked inputs
before/after. Independent re-review identified late cancellation after artifact
download as a remaining edge case; 2a05f9cc added a fixed verifier id and final
always-running outcome/cancellation refusal. d4905e12 then applied the one
additional literal diagnostic reader authorized by integration.

Full infrastructure at clean d4905e12370b571c698a865fed9533ff3c77d590:
912 total / 891 passed / 16 skipped / five errors / zero failures. The remaining
five errors all flow through the still-frozen background workflow reader.
Independent review of this exact source reported no remaining blocking source
finding. No native or real CI timing acceptance is implied.

Integration review subsequently found that read_text() normalizes actual CRLF
bytes before strict string hashing. 272dcf38 introduced an explicit
read_bytes().decode('utf-8') reader at authorized entries, preserving CRLF/BOM
without stripping, and a real temporary-file refusal test. A read-only review
confirmed the correction. Integration granted four additional exact-source
reader files; 479f3dcc73e94b400b6b28e25976651b7fa28ca7 changes only their
imports and workflow reads. All 112 targeted tests pass with identical tracked
inputs before/after. Semantic AST/YAML readers and runtime policy stay frozen.
Full regression waits for the background reader's explicit serial transfer.

After B's handoff, integration explicitly granted the background workflow
reader. 76fdc57bef072cf70a02f8d247e88cf2d6508b10 adds only three raw workflow
reads and two strict inverse entry calls on this branch's c3 version, retaining
all its original Rust assertions/pins. One final full run passed913 cases
(897 passed,16 skipped), as did the unmodified infrastructure validator,
scope, diff check and preview. Tracked inputs were identical before/after.

Feature tracking locally still resolved c3 although leader's actual target is
56746af61789e3e7121e7f43d39b4a5d2b567576. A separate explicit exact-target
preview passed; no parent/tracking ref was mutated. Its tree is
a7f49706279b07acf518d78b436352240400f891, which remains subject to integration
regression and genuine Native/default CI. Final independent source review at
exact76fdc57b found no blocking implementation issue. Evidence archives retain
all earlier errors and TLS failure. No product build/native/CI execution or
remote mutation occurred in this package.

Integration clarified the first CI combination: PR29 performance c3 normally
fast-forwards to this final package, against main
a4e03303b54bd5a5544131e95ad39662f8310e29. The unaccepted Host parent56746af6 is
excluded from that run. Its separately recorded preview is only future
compatibility evidence; no Host merge, source change or extra test was requested.
