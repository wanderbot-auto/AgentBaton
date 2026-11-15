# Source checkpoint

Baseline `9b695892a1814dc9daf7e09486e94cbefdc25b2f`; source `185fcfae6d19f1828506e16fdfe316fad5e7c1fc`; branch `codex/schema17-inverse-current-migrations`; worktree `/workspace/.worktrees/schema17-inverse-current-migrations`.

Source chain: `dd69cdc5bae0d270de4006244d00dc8e63732728`, `13a467c9a40ffaa2f9b77fb2c96e96a7f66eb3b7`, `185fcfae6d19f1828506e16fdfe316fad5e7c1fc`. Shared read-only signatures `INTEGRATION17_TABLES`, `verify_empty_integration17(&Connection)->Result<(),String>` are available to the separate genuine-consumer author; that author does not own this helper.

All actual Cargo/SQLite/Actor execution belongs to Root. Next: independent source review, integrate once with genuine-consumer dependency, then full affected Rust targets and strict checks. Preserve initial Root schema17 Factory21/9 failures, external-capacity interruptions and every historical fixture/negative; do not count source work as passed migration or Mac acceptance.


## Independent session-projection finding correction

Independent review identified a static P2 in the current session history compatibility test: its historical row projection removed only Factory tables, so a fresh17 before-image also contained the four empty Integration tables that the inverse later removed. This was a source-review finding, not an executed migration failure.

Source `7f55a4f583902b4cf5f6618198a034bed1889b22` now verifies the complete exact empty Integration17 domain before capturing the historical projection and excludes only the four named `INTEGRATION17_TABLES`, preserving the original full old-row/fee/counter equality assertion and raw evidence suffix. The after-inverse snapshot never invokes the17-only verifier; unknown/changed/nonempty domains are still rejected by the proof before any exclusion. No wildcard exclusion or weaker equality was introduced.

Eight lightweight inverse/byte/order controls, configured rustfmt and diff checks pass. Root actual Cargo/SQLite/session execution and independent closure remain pending; earlier source manifests and47-control static record remain historical and unchanged.
