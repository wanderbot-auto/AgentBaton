# e3cc CI stage1: exact candidate, incomplete jobs

Featuree3ccf86a8b12e7ed1371074de095bb454c0b62aa, Product40771243767/Environment40771243761, PR16 draft/basea1be94d26d10bb245bb35eb50226b82012c967da. Read-only metadata/log/source-object audit; no product/Mac success is asserted.

Completed Environment Linux123108703097 and ARM123108704372 original logs independently record actual checkout4a9ade282b3babefbc3cdbe2e93fd037f89bcdaa. Git commit API supplies parents[a1be94,e3ccf86] and tree144ab4e890c06b537c018b609016268513051a56, equal to local feature tree. Signed raw commit was reconstructed and its full Git object SHA1 verified, without shared refs/objects mutation or a separate PGP trust check. Product checkout is pending its own completed raw logs and must not be inferred from Environment.

Both original Environment logs show93 integrity files,34 infrastructure Python,4 environment Node and2 environment Rust smoke tests passed. These are infrastructure checks, not product tests. Intel Environment123108702561 is still in_progress in the one stage1 API snapshot; no incomplete log was requested. Product UI123108702939/Rust123108703316/ARM123108703256/Intel123108703489 are all in_progress in that snapshot. No repeated status polling.

Fixed Git comparison against0aa confirms exactly four non-document source paths: Native Cargo.toml/Cargo.lock and test-only Owner driver controller.rs/macos.rs, each byte-identical to7a13cddef4316d520361071ff5e2a263ea058580 and the prior reviewed composition's exact hashes. Four source snapshots and the prior FINAL_232 review are retained. Production Core/UI/root lock/workflows have no byte changes; old success is not reused as new CI proof. Native optional/default feature semantics are the prior review's finding, not new actual Mac compilation or GUI evidence.

Pending: Intel Environment terminal API/log; own completed Linux UI/Rust logs, artifacts and independent decoders; Mac work belongs to the separate reviewer. Stage1 files stay unchanged; later files will be additional evidence. No Cargo/target, product rerun/browser, source/remote mutation, retry/cancel/Approve/merge, Provider/private account/credential or spending.
