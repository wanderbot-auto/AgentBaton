# PR16 c6f6 Linux terminal CI evidence review

Verdict: **PASS for the recorded Linux Rust/UI CI evidence and artifact integrity.** This does not approve a merge, macOS build, installer, live Provider, Factory/schema13 or the complete product.

PR: https://github.com/wanderbot-auto/AgentBaton/pull/16 . Product run: https://github.com/wanderbot-auto/AgentBaton/actions/runs/40790894775 . Feature `c6f6b3c2be18ecc339f1cba6a9603657739e4cd4`, base `a1be94d26d10bb245bb35eb50226b82012c967da`. The API jobs' head_sha is the feature, while **both original job logs explicitly show `git log -1 --format=%H` returning synthetic PR checkout `e963523c04b27d99d44a7893b161b5180b9818e3`** (UI lines123–124, Rust122–123). Reused immutable commit API maps it to tree `32b2747d4541b7f09d05cf21332f6232b081d8a5`, with exact base/feature parents. UI's archived candidate independently records that checkout/tree, dirty=false, integer schema12. These identities are not conflated.

## Actual terminal results

- UI job **123175557548 completed/success**; all recorded steps successful. Actual typecheck, **564 tests /37 files**, Vite build and 12 real browser/Unix-IPC fixture flows completed.
- Rust job **123175557757 completed/success**, including final all-target workspace Clippy with `-D warnings`; all recorded steps successful. Raw cargo output has **593 outer unit/integration tests +7 doctests**, all passed, zero failed/ignored; **two additional nested subprocess test executions** are separately printed. There are51 result summaries summing602; do not inflate the outer count to602. `rust-test-summaries.json` retains every result line; nearest preceding Cargo context can be interleaved stderr/stdout and is not a definitive per-summary association.
- The two Linux Python suites each report **29 tests / OK**, at Rust log1300 and1331. The latter uses the actual built Core daemon for Owner Grant response/persistence contracts. These are Linux runner/Core contracts, not AppKit dialogs.
- Provider official-schema validation reports **8 messages**, pinned schema SHA-256 `f50e158c34ac55c5cc67c5493df539f1c07b603bd5dd2100a7393eefb62a3e20`. Independently reran the exact c6 source's read-only validator against downloaded official artifact bytes using the existing Python dependency: exit0, same8. This checks captured generated-child wire, not vendor/live/account/model compatibility.
- Existing saved Environment run40790894815 records all **three required Environment jobs success** at exact feature SHA. Files were copied and hashed from the Mac reviewer evidence; **no Environment API request was repeated**.

## Official artifacts

Each original terminal Linux/UI log and each artifact ZIP was requested **once**, with no retry, polling or workflow trigger. Product jobs/artifact listing read once each. Request/exit/time/bytes/SHA records are adjacent `.attempt.json` files.

| Artifact | ID | API/actual ZIP bytes | Verified ZIP SHA-256 | Files |
|---|---:|---:|---|---:|
| provider-contract-e963523… |12861917193|74555|768b312b42a29966079aa053ce3882f9f348eac4c9c8c16734aeb4f434468256|2|
| frontend-real-ipc-e963523… |12861233284|9108992|8bc2752a885c7914af477fc435812b5ca4c4cab2e9cbbb18bce5198c10f7ba4f|95|

Both API size/digest and workflow_run ID/head bindings match original bytes. ZIP CRC passed; bounded unique normalized member paths, no traversal, backslashes, links or special modes. Extracted files are individually hashed in `artifact-inventories.json` (provider739873 uncompressed bytes; UI19186866). Original ZIPs/logs remain unchanged. UI contains reports/screenshots/logs, **not raw daemon ELF or frontend JS/CSS**: recorded hashes are cross-checked for consistency, not independently recomputed from absent build bytes.

## Twelve real UI flows

| Flow | Passed checks | PNG |
|---|---:|---:|
| Quiet canvas/home|38|24|
| Approved Command|5|2|
| Goal planning|9|6|
| Structured plan editor|10|9|
| Data workspace|8|6|
| Goal execution|10|9|
| Graph Owner-only|6|3|
| Graph Integrity|6|3|
| Graph Data|6|3|
| Graph Command|6|3|
| Graph Data-range negative|5|1|
| Graph Command-timeout negative|5|1|
| Total|**114**|**70**|

All report check statuses pass and counts agree with original job output. Every PNG has a real PNG signature/dimensions and inventoried SHA; **68 report-declared image hashes match**, while the two older Approved Command images have no declared per-image hash and are bound by the official ZIP digest plus independent inventory. This absence is not silently invented into a hash declaration. All reports record coherent daemon SHA `a07b00de7eb58128f2dde32397e9c49b3918ff0413195c574f9eb8feae4208c3`; applicable candidate/raw-manifest/harness source hashes match exact c6 and archived candidate bytes. Reported frontend asset maps agree across applicable flows. This is schema12 PR16 evidence; the separately reviewed schema13 Factory leaf was not part of this run.

Four Graph positive flows bind real Task state/acceptance_ref/sequence to their stored acceptance receipt and `task.accepted` event's digest/source/run. Five deterministic-validator runs (Integrity/Data/Command and two negatives) retain full original observation strings whose UTF8 hashes match result_digest and parsed values match the receipt. Workers report joined/confirmed stop and no Provider session created for validation. Negative Data-range/Command-timeout outcomes remain failed validation, blocked run and pending acceptance, with Owner/Summary not started. Their diagnostic failure state is the expected negative-path evidence, not a failed CI run to erase.

Goal execution retains seven owned-stop receipt chains, partial14-byte cancellation versus timeout/unknown distinction, and same-initial-plan Work12B+Summary13B outputs with zero additional advance requests. It also retains two actual dropped-response records, revision2 history/budget preservation, Data inputs and stale-CAS refusal evidence. Data's preview/copy cancellation and deliberate drift records remain in the official reports; bounded copy/preview receipts do not establish generic process containment.

Six Graph reports explicitly show joined HTTP serve/handler threads, exact daemon exit0 without forced SIGKILL, and recorded generated original process identities gone without signals; Command timeout additionally records its exact `/usr/bin/sleep` child gone. Other flows have their original assertion/exit/observation evidence, not a uniform whole-process census. **No general “no leaked processes” claim** is made from finite generated identities or successful job teardown.

## Actual image inspection

Three original artifact PNGs were loaded through `view_image` and actually returned as images:

1. `goal-graph-integrity/03-task-accepted-five-node-integrity.png`: Task accepted; five nodes visible; Validate explicitly says checks complete / acceptance separate; Owner nodes accepted; two Owner decisions.
2. `goal-graph-command-timeout/01-five-node-command-timeout-blocked-before-owner.png`: Blocked, acceptance pending, Validate failed, Owner/Summary not started, zero Owner decisions.
3. `goal-execution/08-timeout-mobile-unknown.png`: mobile outcome_unknown, owned stop confirmed Yes,1200ms reservation and tokens/cost unknown.

**Remaining67 PNGs were hash/dimension checked, not visually inspected.** `audit-results.json` marks viewed images individually.

## Warnings, audit commands and limits

The new async Playwright executable lookup actually ran in the original UI job. No `Event loop is closed`, `Task exception was never retrieved`, coroutine-not-awaited or old PlaywrightContextManager warning matches exist in that raw UI log. The run is **not warning-free**: Vite's >500kB chunk warning, Node punycode/url.parse deprecations and action Node20→24 deprecation remain. Rust Clippy itself succeeds under deny-warnings.

Independent `python3 -B audit.py` exits0 with **245 evidence consistency checks**; `extract.py` exits0; exact source `validate-native-schema.py <downloaded schema> <downloaded wire>` exits0 with8 messages. These are artifact/log/schema analysis, no new product execution. Initial audit exit1 is preserved (`audit-initial.py/.stderr`): the reviewer incorrectly required a per-image hash in the old Approved Command report. Corrected audit accurately records the existing absence and relies on the verified official ZIP and per-file inventory, with no source/test/acceptance change.

No Cargo, target, browser, daemon, models/accounts, dependency install, source/STATUS write, Mac artifact download, workflow trigger, push/merge/Approve or delegation. Mac jobs and installers remain the separate reviewer's scope. No prior failure evidence or other review records modified. Whole-product acceptance remains incomplete.
