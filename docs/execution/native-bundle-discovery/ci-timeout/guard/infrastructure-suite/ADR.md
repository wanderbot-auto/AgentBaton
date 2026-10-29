# ADR: regression coverage follows the exact finite job policy

The60-minute desktop job and matching validator were separately authorized, but an existing regression expected45 and rejected46. Root reported all three Environment lanes failed; the preserved Linux log independently shows34 tests with two failures. Previous static/finite checks did not exercise this existing suite and cannot substitute for it.

Change only exact desktop expectation60, first forbidden61 and case name. Ordinary30-minute bounds and every other assertion remain byte-identical. Update only this registered test hash; preserve the other92 and STATUS. This aligns tests with the explicit bounded job-resource decision without enlarging general resources, runners, permissions or product/per-test deadlines.

Require the full original Environment validation-command set. Python/Node/static commands passed on source9d; the existing Rust fixture test/clippy passed under an exclusive lease on Root3a91. Fixed-Git hashes bridge21 relevant inputs; counts and commit provenance remain distinct. This is complete local command coverage, not independent newCI replay or macOS/product acceptance. Original failures remain preserved, and new remote checks must establish their own results.
