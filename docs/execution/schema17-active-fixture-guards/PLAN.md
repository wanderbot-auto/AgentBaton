# Strict current Factory schema17 guards

Requirements: AB-TSK-008, AB-TSK-009, AB-TSK-010, AB-PRJ-004, AB-PRJ-005 and AB-VAL-009; partial test maintenance under AB-START-001.

The current Store migrates a complete schema16 predecessor to schema17. Two active Factory fixture checks still demand schema16. Bind these current consumers to exactly17 while keeping the original typed SQLite u32 extraction and every operational assertion unchanged. Do not accept a version range, modify production migration/schema, relabel a historical producer or change a budget/permission.

1. Distinguish current daemon/post-run headers from historical predecessor inverse fixtures.
2. Change only the two expected current versions and explanatory messages.
3. Prove complete-file inverse/byte preservation, strict typed contract and formatting; obtain independent review.
4. Root executes all30 actual Factory cases using schema17 tools and records precise source/results. Root owns Cargo/Actor/browser execution and integration.

Broader migration consumers remain separate because safely removing the new integration domain requires a controlled inverse, not a search-and-replace of16.
