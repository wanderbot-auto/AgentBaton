# ADR 0005 — One config-key read within each fresh boundary

Status: implemented locally, independently source reviewed; Mac validation pending.

Actual old8b macOS evidence showed costly preparation with143 completed Git calls. That does not identify a single cause of its5s observation failure. Source review identified four config subprocesses repeated within one original validation boundary. Each asks whether config can execute helpers, load includes or enable partial-clone behavior.

Use one bounded `git config --no-includes --null --name-only --list` at that same boundary. Parse only complete NUL-delimited key names and preserve all four existing predicates and refusal priority. Values are not read, so multiline values cannot become fake executable keys. Every later boundary performs its own fresh read; do not cache authority, filesystem pins, target head, lineage or prior config results. Original cancellation, deadline, environment and output caps apply.

Six meaningful controls compare against the historical four-query oracle, exercise fresh mutation, error priority, malformed data, deadlines/cancel, multiline values and empty subsections. Full Workspaces/Core/integration checks and independent predicate comparisons are source-bound in `docs/execution/fresh-bound-config/EVIDENCE.json`.

The unchanged output cap now covers all names; a repository with extremely many benign keys may be refused sooner. This conservative limitation is accepted for this reversible change; no helper/config/output allowance is expanded. No claim is made that Linux timing proves the previous Mac failure fixed. Separate846 capture queue and ARM missing-entry failures remain open under original budgets/refusals.
