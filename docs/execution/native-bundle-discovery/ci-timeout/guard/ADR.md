# ADR: match the explicit desktop exception to the authorized job budget

Status: implemented candidate, pending supplemental independent review.

The earlier one-line job change to60 left `validate_job_timeout` enforcing45 for the same exact desktop job. Root actually ran the validator and received `Unbounded CI job`; this failure is retained rather than removed or called passed.

Change only that existing exception to60. It still applies solely to product.yml/desktop, the original matrix host selector, exactly macos-15 and macos-15-intel, max-parallel2 and fail-fast false. General jobs remain bounded at30; all permissions, secrets, pins and required-Environment checks remain byte-identical. Existing integer/string parsing rules remain unchanged, and invalid values are tested. This does not relax any product/per-test deadline or authorize another runner/resource/account.

The validator is one of93 registered bootstrap files. Update only its SHA-256 after reviewable source change; preserve the other92 entries and actual bytes. Root owns any later STATUS rebaseline. Real infrastructure and bootstrap validators must run successfully. Passing these static checks establishes guard consistency, not a successful new CI run, build or installation package.
