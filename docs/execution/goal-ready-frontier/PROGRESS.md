# Progress

Read current source and confirmed candidate immediately sets awaiting_acceptance while all scheduling entrypoints allow queued/running only. No implementation change yet. Baseline78b source matches prior9f runtime. New isolated tree and main target acquired without orphan Cargo writer; library entrypoint touched only after before/after SHA equality. Adding actual old-source regression now.

Actual old-source regression failed twice under unchanged runtimeSHA65c953ca…a9220b1. old-source-two-node-count-fail.log directly queries SQLite and records first=candidate,second=not_started,attempts=1,run=awaiting_acceptance; original observer bound remains unchanged. Both FAIL logs retained. New fixture uses full fixed TaskRequest and two approved disjoint output mappings; no product code changed before the failure. Next: narrow candidate frontier and once-only pending-version pause, then actual positives/negatives.
