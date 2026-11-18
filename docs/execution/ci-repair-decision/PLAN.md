# AB-REL-006 plan

1. Bind the Core decision to live Integration, Task and Project records and to separate approved source and observed CI facts. Keep the source and policy trust boundary outside renderer IPC.
2. Classify nonterminal, unknown, stale and mismatched observations. Report continuation for a current terminal failure without an approved repair rule.
3. Gate eligibility on exact scope, live identity, stable deduplication, bounded history, blocked cycles, known metering and shared capacity. Keep decision evaluation read-only.
4. Check the existing Integration regression slice, focused cases, formatting, check and strict Clippy. Record exact source, integration preview, and producer, persistence and UI dependencies for review.

## Production sources and remaining wiring

The Store reads registered Project and accepted Task state, the authenticated Integration policy and run, the confirmed local ref journal entry and joined CAS attempt, and the original Goal acceptance. For eligibility it rechecks registered connection/workspace scope and live grant ancestry, uses verified shared usage, and refuses active work when reservation amounts cannot be safely projected. The policy must predate the failed result. The ledger covers the stable root across candidate and rule revisions.

A production CI adapter must provide an approved project-to-remote source mapping, independently checked current candidate and latest CI run, a fresh terminal observation and an opaque report reference. An effective-settings/native approval path must supply the explicit rule and its identity and limits. Neither source currently has a production producer in this slice. The history adapter must supply complete durable repair history. A future dispatcher needs an atomic unique attempt insert, meter/fence reservation, revalidation and unknown-outcome recovery before dispatch. The advisory decision itself cannot supply that authority.

Normal UI needs a read-only Core projection to render the typed status, including `continue_required`, and an existing separately authorized continuation entry. Persisted CI and repair lifecycle records, effective rule approval, and the normal IPC/UI routing need leader coordination before any shared-file changes. The task leaves existing workflow triggers, real PRs and Provider operations untouched.
