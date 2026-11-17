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
