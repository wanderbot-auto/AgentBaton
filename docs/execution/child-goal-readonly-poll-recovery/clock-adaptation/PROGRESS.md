# Test-only clock adaptation authored; execution pending

The new temporal block no longer installs Vitest fake globals. A scoped timeout spy holds 1000ms function callbacks and forwards other timers unchanged. Its restore hook clears its own queue and restores both spies after each test. User-event uses normal real scheduling; real timer/performance captures let async SHA and zero-delay drains complete while preserving the original 1000ms observation bound. No global Jest bridge, configuration timeout change or product hook was added.

Only explicit test advances move the polling clock. Each ordinary positive step first advances 999ms and checks the read count is unchanged, then advances the final 1ms. Observations use real zero-delay turns and cannot consume a polling slot. Existing twenty-read, manual-reset, failure/unknown/identity/epoch/partial-evidence and no-write assertions remain.

Pure source inspection passed: every original case declaration name/order and all 231 original assertion occurrences are preserved; the old 55-case prefix is byte-identical except the scoped clock cleanup hook; product component, API and API test are byte-identical to 127. `git diff --check` passed. No Node/typecheck execution of this adaptation occurred here, so no improved test result is claimed.
