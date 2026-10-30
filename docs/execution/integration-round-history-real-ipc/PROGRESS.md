# Source progress

Added only a new optional driver and its pure tests. The original integration-real-ipc.py, product workflow, integration browser CI guard, strict publisher and existing driver/projector tests remain byte-identical to the baseline; a whole-file hash manifest verifies this. No product code, schema, lockfile or migration changed.

Actual author checks:9 new pure controls PASS0.005s,35 original finite boundary controls PASS0.102s,18 original strict publisher controls PASS0.541s,62 distinct tests total. Wrapper times and full logs are retained separately. These tests use controlled symbolic DTOs, filesystem FIFOs and injected subprocess/proc observations; they start no product process, no real Git child, no browser and no Core. Source compilation without execution and git diff --check passed.

The source driver includes original image/assets/source pins before and after runtime, full private proposal and consumed receipt verification, real event/receipt digest links, approved command/image and per-round file subject checks, exact source/index preservation, true joined stop/EOF/reap assertions, no-refund sums, original Graph acceptance, one sole final CAS and released leases/intents. Failure retains private transcripts, gate trace, database and generated roots after owned cleanup attempts. No authority/pass/acceptance/stop is inserted using SQL.

Actual mixed-round runtime, DOM selectors, screenshots and cleanup have not been executed by this author. Existing Mock UI53 or earlier Core real IPC results do not count as this new browser case passing. No CI wiring or publication change is included.
