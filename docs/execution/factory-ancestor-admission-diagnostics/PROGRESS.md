# Progress

Source implementation: `9e7ce2c031a2800cf2bbb42b2a84e987ed23ddd8`, based on Parent `3e882a8d516707b40c991706714eaa4b81885fc8`.

The optional failure sample distinguishes attempts, parallel, cumulative runtime, cumulative output, runtime addition overflow and output addition overflow. It contains the original run/plan revisions, relevant/active/accounted counts, original caps and request, and separate settled-actual/unsettled-reserved subtotals. Count guards leave unobserved aggregates null. Overflow preserves partial counts and null subtotals rather than inventing zero or a full aggregation.

The existing `RESOURCE_LIMIT` error keeps its message, retryability and side-effect state. Optional serialization is bounded to 1024 bytes after a fixed prefix; failure to serialize omits the sample and preserves the refusal. The existing Actor blocked-event path accepts only the closed typed detail matching the same current run/plan. Stale, malformed, unrelated or overlong details are omitted without changing the block. Collector projection additionally binds the original run revision to the immediately following blocked event and the observed plan.

Twelve new pure field/generated-SQLite controls and the original unchanged eighty collector controls passed. Rustfmt and diff whitespace passed. Two new Rust unit sources are written, not run. No Cargo, Actor, Node, Provider, Native/Mac or installation validation was performed by this author. Independent review and exact integrating-candidate tests remain required.
