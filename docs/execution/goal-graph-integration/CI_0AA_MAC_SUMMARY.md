# PR16 0aa dual-mac evidence summary

Feature `0aa20c968d493cad895e4a3853bdf106d6b54acf`, actual merge checkout `5caecb5f9945b5fdb548f0e071a78e2d97537403`, tree `1943596e0bd12177d852117dab86c81ca30e6858`. Product run40761824261 attempt1. Original ARM and Intel logs/ZIPs each downloaded once, both ZIP API digests match and CRC/safe extraction passed. Reports and independent manifests retain exact timestamps, phases, source hashes and limitations.

| Check or dimension | Result |
| --- | --- |
| Environment / ubuntu-24.04 | success |
| Environment / macos-15 | success |
| Environment / macos-15-intel | success |
| Product / UI | success |
| Product / Rust | success |
| Product / macos-15 | failure, actual Owner step |
| Product / macos-15-intel | failure, actual Owner step |
| Actual readonly WebView/lifecycle and four WebCrypto contexts | success on each actual mac architecture |
| Actual Owner Grant Cancel then OK + final DB acceptance | failed before first click on each architecture |
| Default release build, release binary audit, development packages | skipped on both architectures; no current packages |

Job-level five successes and two failures come from Root's saved API responses, retained as `summary-root-environment-jobs-snapshot.json` and `intel-root-product-jobs-snapshot.json`; these counts do not change the two mac jobs to success. The actual readonly dimension succeeded inside each failed job and is retained as independent valid evidence. Both are strict schema12/sequence3 evidence, not new Factory schema13 validation.

Both Owner reports independently say `Exact full native title/message not uniquely present` at the original strict NSTextField uniqueness guard in initial cancel phase. Actual field values/counts and Owner failure screenshots were not retained, so the specific causes remain unresolved; matching failure text alone does not prove a shared root cause. Bootstrap completed and daemons were ready; stale retained session.token readiness errors are not terminal causes. No acceptance threshold, timeout, permission or production code was changed by this review. Root's separate test-only diagnostic task must preserve full-text/parent/visibility/DB/button gates and cannot accept partial text.

Provider models, signing/notarization, graph private multipage OS approvals and production release remain not_run. No formal approval or merge recommendation is given. Detailed evidence is in `ARM_REVIEW.md` / `ARM_MANIFEST.json` and `INTEL_REVIEW.md` / `INTEL_MANIFEST.json`. The ARM sealed files remain historical snapshots; Intel's later completed status is additive here.
