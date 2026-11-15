Current Root17/main composition is pending new runtime verification. Both historical source records below are retained with their original scope and timestamps.

# Progress

Source implementation: `9e7ce2c031a2800cf2bbb42b2a84e987ed23ddd8`, based on Parent `3e882a8d516707b40c991706714eaa4b81885fc8`.

The optional failure sample distinguishes attempts, parallel, cumulative runtime, cumulative output, runtime addition overflow and output addition overflow. It contains the original run/plan revisions, relevant/active/accounted counts, original caps and request, and separate settled-actual/unsettled-reserved subtotals. Count guards leave unobserved aggregates null. Overflow preserves partial counts and null subtotals rather than inventing zero or a full aggregation.

The existing `RESOURCE_LIMIT` error keeps its message, retryability and side-effect state. Optional serialization is bounded to 1024 bytes after a fixed prefix; failure to serialize omits the sample and preserves the refusal. The existing Actor blocked-event path accepts only the closed typed detail matching the same current run/plan. Stale, malformed, unrelated or overlong details are omitted without changing the block. Collector projection additionally binds the original run revision to the immediately following blocked event and the observed plan.

Twelve new pure field/generated-SQLite controls and the original unchanged eighty collector controls passed. Rustfmt and diff whitespace passed. Two new Rust unit sources are written, not run. No Cargo, Actor, Node, Provider, Native/Mac or installation validation was performed by this author. Independent review and exact integrating-candidate tests remain required.

## Root actual verification — 2026-10-30T18:18:22.756351+00:00

Exact source937b426 passes167 unique Core units (new2 included; nested helper not additive),256 infrastructure controls, strict workspace/all-target Clippy and format. Real generated ACP/Command/UnixIPC failed-validator→rework→Owner→parent-resume passes32.57s;29 other Factory cases were filtered, not run here. Optional typed original ancestor-budget refusal details preserve all limits/error semantics and have independent source/pure review. [Root results](ROOT_RESULTS.json) retain commands/source/logSHA and earlier evidence.

Original fbeb Environment3/Rust/ARM passed but wholeProduct failed: legacy UI cancelled-provider empty argv, Intel26/1 blocked/resource-limit at runrev14. Capture2 was3919/4000; postshutdown child15589+newreservation6000 exceeds20k but is not the original admission snapshot. New code adds exact guarded finite observations, not a claimed original rootcause/fix or budget increase. Legacy cancel4checks localpass uses new harness and explicitly older415 compiledCore/frontend; empty-argv branch not actually counted. Exact new remoteCI, current Mac/Native/development packages and full130 remain unverified; no oldpass transfer/noformalApprove/no production/signing/credentials or additional spend.
