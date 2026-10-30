# Evidence

Source `6853844c87a54de41fdd86fd34d96e6ed2482817`, baseline de0b911328c6703491902970ff608b908e3bf643.

| File | SHA-256 |
| --- | --- |
| apps/desktop/tests/factory-real-ipc.py | 04a780636166dd21beaac082a9724b5c60998a5360a7ee651ee269c1f299f016 |
| tests/infrastructure/test_factory_bound_provider_exit.py | 2947f20a5c826ad416673068d5b623927e22e13430cd1b4caa06170670f3e335 |
| .onboarding/factory-bound-provider-exit-source/BEFORE.json | 3d27b90c2cf8a67222457a7b28f1c89bb4ea13739849006237bc5c4a82e49b80 |
| .onboarding/factory-bound-provider-exit-source/old-control.log | 62e118223ba9bc2a203b0e78796ebcbfc63000d8c59cb1b9ac1c020b4a4e8f2a |
| .onboarding/factory-bound-provider-exit-source/fixed-control.log | 9f98417a7cd9c22b6100d0bcc2a06930ef5ce9f2f65b43b97336598eb687ac76 |
| .onboarding/factory-bound-provider-exit-source/SOURCE_CHECKS.json | 819d7cd3861d41a6145c65a3965be78d0661bd6e639135d4b105468fd4445113 |

Observed old CI failure: field owned_provider, previously_bound=true; expected PID/PGID12124/start23884, before stateR/parent11750/samebirth. cmdline read was present but0bytes, no trailingNUL, empty SHA e3b0c442…b855. Diagnostic immediately after it reported identity_after=null without a separate identity_after_error. This directly supports an observer exit interleaving; it does not establish the precise kernel state at the exact read instant or any pre-exec cause. Do not use later cleanup PID observations to fill an earlier missing fact. CP01/02 passed; workflow remained failed_or_interrupted and cleanup.complete=false even though all six originally verified identities were absent after daemon wait0. Original full failure evidence remains private/unchanged.

The exact same20-test regression source ran before/after. Old14PASS/4FAIL/2ERROR,1.242s/exit1; fixed20PASS,1.162s/exit0. Unique count20, not40 and not actual Actor/Store/Core tests. Real own SQLite tables are relation surrogates for the actual collector's SELECT/document checks, not issued approvals or real product session data. A normal initial pass through actual collector establishes the unit verified binding before injecting the race; it is not a manually mirrored new acceptance implementation.

Controls include empty/missing known provider followed by exact absence; still-live/Z/reused refusal; initial foreign birth before argv; unbound empty/missing refusal; malformed/oversized/nonNUL/invalidUTF8/suffix mismatch; PGID mismatch; missing getpgid/final stat vs unbound/live/reused; permission exceptions; changed prior record; program hash and Core-turn mismatch before proc; normal complete argv still checking both process groups/final identities; unchanged MCP early/late-reaper call sequence. The MCP control stubs reaper calls and claims no real OS wait. Positive cases preserve exactly the old verified object/list, keep request acceptance_state=not_established and create no stop_confirmed/task_accepted field.

Source checks: complete prior file restored by reversing only the new predicate and precise collector guards; all old other functions/classes byte-identical. Thus unchanged real MCP known-PID wait, nonce/FD/program/settings/companion/Core joins, diagnostic cap8192 plus8193 overread/NUL/suffix, HTTP allowlist, Owner authority, subsequent runtime/final checks, timeout and cleanup code. Present current=None historical handling is unchanged; only present foreign birth is additionally rejected and only previously bound exact metadata may survive the narrowly observed disappearance. New helper does not accept a different start_ticks as absence and never signals/reaps or records a Core stop receipt.

No raw private DB/RPC/Grant/token/provider credential is copied or uploaded. No product/Actor/daemon/browser/Cargo/Node/OS process helper ran. Independent review/new actual run/new CI/macOS remain pending, not inferred from pure green. Original Graph/Factory green and other platform failures are not substituted for this slice.
