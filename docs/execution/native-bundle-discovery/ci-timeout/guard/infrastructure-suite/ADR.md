# ADR: regression tests follow the exact authorized job policy

The60-minute desktop job and matching validator were separately authorized, but an existing infrastructure regression still expected45 and rejected46. All three actual Environment lanes failed; the saved Linux log shows the complete34-test run's two failures. Prior static/finite checks did not cover this existing suite and cannot substitute for it.

Change only the exact desktop expectation to60 and its first over-bound case to61, plus the case name. Keep ordinary30-minute bounds and every other assertion unchanged. Update only this test's registered bootstrap hash; preserve the other92. This aligns tests with the explicit finite job-resource decision and does not enlarge general resources, runner permissions or product/per-test deadlines.

Completion requires the complete original Environment workflow validation commands, including the existing Rust fixture test/clippy under Root's resource lease. Local Python/Node/static successes do not claim new CI or product/native success. Preserve both old failures and all pending dimensions until actually executed.
