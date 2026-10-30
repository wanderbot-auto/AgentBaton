# Accepted source capture — AB-VAL-009 prerequisite

Base `bc10ee94c667da9535018c37fdf5b128d63126af`; branch `codex/accepted-source-capture`. OWNER-001/002 ACK. AB-START-001 permits implementation; it does not manufacture product Owner approval or authorize this library to merge a branch.

Implement only a trusted-host selected-files → raw Git source commit primitive. The host must already validate the accepted Task/Graph receipt and separate integration policy before constructing the private capability. No Core, schema, IPC, Native, UI, adoption/validation implementation, dependency or lock changes.

Allowed source: new `crates/workspaces/src/source_capture.rs`, one lib export, new `crates/workspaces/tests/source_capture.rs`. Private result getters expose commit/tree/base/selection digest and selected file SHA256/bytes/mode; neither result nor authorization is Deserialize.

ADR: use pinned canonical root/git-dir/common-dir FDs, existing bounded Git runner, private 0700 scratch and create-new 0600 raw files. The scratch index is explicitly selected through GIT_INDEX_FILE for every index operation. Base read-tree/write-tree must roundtrip exactly; this refuses unrepresentable raw trees instead of dropping untouched base entries. No original index/working files/refs are written. Git object writes are allowed and may remain unreferenced on failure.

ADR: this slice supports ASCII selected paths and ASCII base-tree paths only. Reject the entire capture if base contains non-ASCII names; do not claim Unicode/macOS normalization support. Root explicitly approved this conservative boundary; later native-dirent/FD semantics and real Mac evidence remain required, not removed from full product scope. Case-fold aliases, .git components, traversal, aliases/prefix conflicts and nonregular selected base entries reject.

Limits: 1–256 selected files, each original BoundArtifact byte cap, shared 64MiB selected bytes, 4096-byte relative paths, globally bounded 65536 object metadata entries and 65536 base entries, existing 4MiB Git output cap. One private original monotonic 15s deadline begins at constructor entry and covers handoff/capture; stage/file/Git checks never renew it. An already-spawned Git still uses existing 15s kill/reap. Blocking kernel I/O cannot be forcibly interrupted; future owned Core workers must join and account actual elapsed time. No OS sandbox is claimed.

Existing base regular-file modes100644/100755 are preserved; additions use100644. No deletion/rename/chmod contract exists. Source may be dirty; unselected bytes and index stay untouched. Partial/success scratch directories are retained privately; no recursive deletion or complete retention/cleanup service is claimed. A host must later reconcile/clean known scratch under its own retention policy.

Root will independently review and execute real generated Git tests after source freeze. Full AB-VAL-009 still needs independent native policy, durable intents/fences, correctness validation of the merged candidate, exact target CAS and unknown reconciliation. No TaskAccepted, code correctness, Grant, adoption, push or remote protection authority is added here.
