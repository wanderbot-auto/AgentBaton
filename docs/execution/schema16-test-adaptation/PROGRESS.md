# Progress

Frozen source: `84e6969edc46eba385c6a9bd1473ddc93a4ee51a`; 19 test files (17 top-level targets plus two explicitly authorized fixtures), 292 insertions/56 deletions.

18 current Store assertions now expect16. Ten future-negative functions now reject17; the initial estimate of nine missed a function without “future” in its name. `ASSERTION_CHANGES.json` records each original location and reason; original function names in that inventory identify pre-change source. Unrelated numeric15 assertions remain unchanged.

Strict inverse adds bounded typed Task validation, rejects any workspace-binding key including null/false, requires complete named index sets and active transactions. Thirty Rust negative variants retain all12 original variants. Full WAL schema/typed rows/rowids/sequence/pragmas/autocommit snapshots and the original caller late-DDL rollback test remain. Positive preservation now covers raw Task bytes and SQLite NULL/BLOB/TEXT/INTEGER/REAL values.

Authentic14 consumer targets16, retains original pre-v15 backup checks and adds pre-v16 backup checks against original14 raw rows/FKs. The immutable source14 generator, fixture input and manifest bytes are untouched. The separately authorized runtime fixture has exactly one version assertion changed.

Completed before recovery: 35 static checks; 22 generated SQLite/WAL model controls; rustfmt check of19 changed files. No Rust compilation or migration execution. First own checker failed due to a Python chained-comparison assertion, preserved separately; corrected checker passed. Recovery only verified clean source and packaged existing records.
