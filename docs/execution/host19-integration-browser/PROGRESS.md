# Progress

Fixed base cde501db847d1bbfc27a298e16cd772afc1a41e9, clean worktree, gates and eight exact claims confirmed.
Scope passed. A repeated in-app handoff message hit a transport failure, so the
ACK is recorded here. The two verified arm64 tools have byte-identical portable
Rust build inputs between their prior build and this base. Baseline starts next.

The unchanged driver refused the real integer 19 snapshot and database. It
accepted only the temporary SQLite version-18 copy and refused version 20.
The repeated baseline included durable SQLite and WAL comparisons after closing
fixture writers. Both private daemons exited zero and were reaped.

Only the two runtime guards changed. Reversing the two exact current anchors
recovered the parent Git blob, and reversing only the input plumbing recovered
all four original test files. The old source tests failed in 11 cases with the
new driver and the original readers. All 34 existing tests and seven added
controls then passed together.

Candidate 86c7cd20a7528d5b7e3dabcfd9738a64dd60275c passed the same real daemon and SQLite probe, including
read-only mode, transaction, SQL write refusal, file and WAL hashes, and
the unchanged daemon sequence. Full infra passed with 927 passes and 16
existing skips. The real locked desktop frontend built locally.

The unchanged workflow schema probe observed actual integer 19 and refused.
The standalone driver entry also returned exit 2 at candidate_schema_guard
using the actual schema19 tool provenance. Source attribution separately read
the exact tracked live file. Browser UI and CI were outside the successful
validation. The evidence and conflict-free preview are ready for leader review.
