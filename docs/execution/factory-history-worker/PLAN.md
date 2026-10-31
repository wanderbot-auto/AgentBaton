Current Root17/main composition is pending new runtime verification. Both historical source records below are retained with their original scope and timestamps.

# Fixed Factory history worker

Requirements: AB-TSK-009/003/008 readonly Factory progress, AB-TSK-007 confirmed stop/cancellation responsiveness, AB-VAL-009 retained proof integrity. Partial implementation only. This is not a schema migration (AB-SEC-007), new authority, budget change or Intel repair acceptance.

Base `61c0fce36c1819d136b91c6349e8d86031156f8d`. Root authorized four product paths, two new cfg test files, two append-only private cfg registrations and these four records. No external integration Cargo target was created.

## Evidence and bounded goal

Exact Parent61 Intel run41063677236/job124089485764 failed Factory26/1: plan2/run12 blocked, same-revision capture4164/4000ms, producer3745ms, IO not_started/no joined capture and four worker phase values null. Last list839ms;28 list calls30849ms client wall time. These facts neither measure CPU nor prove overlap/causality. Previous059 plan2/run11 node4013/4000 outcome_unknown remains a different failure. Diagnosis seals: REVIEW ed89f0d33ad7a7410103296824ceb0d9a364391de171eebb4d11890240e5f28f / MANIFEST95aafa06c354c133366ed4f98dd5b7a46d034817447cabeb46d826109c6a1e04.

The source-proven issue is structural: ready Owner get/list invokes complete SQL history verification synchronously before the next Supervisor/Graph service opportunity. Offload only those two reads to one owned readonly thread. No claim that this establishes the measured overrun cause, reduces CPU cost or guarantees4000ms.

## ADR: one flight, zero pending, no authority cache

The existing Owner Request type parses the exact envelope. Only the original token/API/nonempty request_id<=128 checks plus fixed factory_child_goals.get/list and typed query can select this route. The original arms already reject any Some expected_revision/idempotency_key; omission/null remain allowed, malformed types and unknown fields retain original refusal. Direct Store APIs, direct dispatch and all effects remain unchanged. Wrong/scoped/ticket/effect requests never acquire this route's authority.

One active worker, zero additional waiting requests. Occupied or non-autocommit Actor state yields WORKSPACE_BUSY with retryable=true and side_effect_state=not_started; no synchronous fallback. The original ready16/socket8 bounds remain. This separate slot is never included in DataWorkers.has_workers or Goal/Factory admission/setup gates.

Actor retains ReadyRequest, original deadline, response channel and the strong raw DatabasePin owner. The closure receives only parsed fixed query, Arc borrow of identity resources, cancellation/deadline and a one-shot interrupt-handle sender. No token/raw request, arbitrary method/SQL/path, dispatch ticket, serialized capability or live permission cache is transferred. Retaining the DB pin across reads caches only file identity/ownership resources; every request opens a fresh connection/snapshot and revalidates all history.

## Read connection and reply

Reuse the existing DatabasePin protocol with only four internal visibility changes: canonical owned700 parent, owned600 regular single-link main, held/path identity checks, READ_ONLY|NO_MUTEX|NOFOLLOW, bounded busy timeout, query_only and BEGIN. Read active WAL normally, never immutable=1/main copy. Assert exact current schema15 for this fixed source; a Core17 port requires separate exact-version adaptation, not permissive dual-schema reading.

Never Store::open: it migrates/backs up/recovers and is forbidden on a live read worker. Construct the private Store with readonly connection and all ephemeral authority/clock fields empty. An explicit SELECT sequence establishes the snapshot; unchanged public get/list then uses its original strict existing-transaction fallback. Complete raw-row/approval/source/ref/stop/validator/Owner/acceptance and original JSON bounds remain. The earlier29 summary dedup is deliberately not reused in this path: correctness/fairness first, no CPU or latency saving is assumed.

Reply DTO and revision, including an empty list, come from that same SQL transaction. Never stamp old data with Actor's later sequence. Query errors retain original strict Core errors and cannot become domain success. Check original deadline/cancel before/after read/commit and again before reply; no restart/reset/renewal. Existing client5s/server60s and all4s/20s/30s/55s/60s domain clocks, reservations and fee formulas remain unchanged.

## Interrupt and owned cleanup

Worker sends only its own SQLite InterruptHandle to Actor via bounded channel1. Deadline/shutdown interrupts that connection, not Actor's. Ordinary poll joins only is_finished and outside Actor transactions. Worker panic becomes a failed query, never a successful receipt or Task state change. No detached thread is allowed.

The Actor's strong raw FD pin outlives all worker Arc clones, including panic. Normal run/Drop signals read cancellation, retains the original process shutdown/reap ordering, then really joins the reader. Only after join and Actor autocommit does release_database close the last raw pin on Actor. Exceptional raw BEGIN retains the pin until Store closes first (Daemon field order Store then history service). No worker closes the last raw FD during Actor writing. This reuses the existing path/held-FD pin protocol; it does not claim a new atomic proof of SQLite's internal FD against arbitrary same-UID ABA manipulation.

Normal ticks keep all original Supervisor/capture/frontier/admission/effect order; only a finished-history reply poll is added. Shutdown/unwind keeps writer ownership through actual join. Kernel stalls remain non-forcibly-interruptible, as in existing I/O; no hard thread-kill guarantee.

## Test and acceptance boundary

10 cfg Core/SQLite/owned-thread controls plus2 real Unix/socket_worker/generated ACP/manually driven Actor controls are source-only here. The latter uses an honestly missing Factory scope for strict error/held-read fairness, and separate genuine Native Goal approval/owned session/real stop; it is not one full derived Factory Goal flow or Daemon.run/Mac. Normal complete derived DTO/WAL behavior is covered by the10 service controls. Original fixture bodies/assertions remain unchanged.

Root owns compilation, selected tests, complete Core/Factory/sessions IPC/strict Clippy/fmt and actual exact Mac comparison. Require independent source review first. No original failure or old green can transfer to this source. No budget/timeout/authority reduction, performance success, Mac delivery or formal Approve is implied.


## Root actual checkpoint 2026-10-30T15:23:35.312072+00:00

Source6acd/d649 compiled and passed165 unique Core tests (new10+2/nestedhelper included),30Factory123.57s,58sessions206.66s, strictworkspace/alltargetsClippy/fmt and203infrastructure. Two held-history Unix/ACP tests are manualActor+strictmissingFactoryScope; fullnormalDaemon coverage is the separately attributed fullFactory/sessions targets. Original61 Env/Rust/UI/ARM pass and Intelcapture4164/4000 failure remain, no CPUrootcause/speedup/repair claim. Independent42 sourcechecks are not another productexecution. New exact sevenCI/Native/Mac/packages pending; full130 incomplete. ROOT_RESULTS.json binds argv/source/INPUTS/RESULT/logSHA and limitations.
