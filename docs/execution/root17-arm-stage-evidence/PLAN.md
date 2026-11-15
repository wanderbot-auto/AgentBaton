# Root17 ARM stage failure evidence

Base: `e874b123d3e7f329a551e21922baf1a7a643548f`. AB-START-001 / OWNER-001 and OWNER-002 acknowledged; no OWNER-003. This unit implements test evidence only, within the authorized existing CI allocation. No product behavior fix or formal approval.

Product run 41176611805 ARM integration IPC retained 8 passes and 12 failures in 238.32s. Seven FIFO observers expired before the fixture queried the current stage, two normal cases reported durable `outcome_unknown`, and preparing, validating and stopped-leader observers failed once each. Raw log SHA256: `0a5b90545a83f6b15b7b7a3c5e72833dcf21909278afcd4e7fdef79c0340225b`. Apple held-pin/system-volume policy passed; no old Git-pin root cause is carried forward. The official 419-byte Native integration summary is Owner-gate evidence, not the missing runtime stage history.

1. Preserve original test-host stop/join calls, count, order, panic behavior, runtime fixtures, assertions, budgets, grants, executable/source guards and production code.
2. Observe only the original Actor thread join result and actual successful fixture integration RPC run/task/policy linkage. Only an observed successful Actor join permits read-only inspection.
3. After that original shutdown, use one bounded immutable/read-only SQLite transaction over the exact fixture database. Refuse journals, unsafe metadata, inconsistent column/JSON associations and ambiguous aliases. Project only the existing attempts, `integration.stage_unknown` and `integration.preparation_io_observed` facts.
4. Write a fixed closed JSON line to existing test stdout on a failing fixture. No workflow, upload platform, raw database publication, new RPC, Store open/migration/recovery, production phase latch, process action, authority or acceptance changes.
5. Run source/inverse checks here; leave compilation, Rust controls, real SQL snapshots and actual unchanged Linux/dual-Mac integration cases to Root and independent review.

Collection success is not test success, a process stop receipt, proof of complete I/O, a no-effects finding, or a failure-instant snapshot. Missing/invalid evidence remains unobserved. Root must bind any new CI output to its exact candidate before assigning a cause.

## b8e independent P2 follow-up

The initial candidate `b8e9874f7f082d5a5404234a061174b4bebc09b7` was rejected for reopening SQLite through a pathname while holding only runtime/DB leaf pins. An ancestor swap-and-restore can leave both old pins unchanged while the pathname reader sees a foreign database. Preserve that rejection and its filesystem model. The follow-up uses bounded held/named pins for every directory from `/` to runtime, retaining each parent's identity, mode and nanosecond mtime/ctime across the original pathname open/read. No shared-directory exemption, FD-reader claim, new FFI/dependency, production change or behavioral repair is introduced.
