# Integration browser driver schema 19 evidence

## Source and artifacts

- Base `cde501db847d1bbfc27a298e16cd772afc1a41e9`.
- Directly tested source `86c7cd20a7528d5b7e3dabcfd9738a64dd60275c`.
- Current raw driver SHA-256
  `e5036fd15afa431228a9b11553b0602e144636dda58fecf4fbbbcaac8633f6f3`.
- The two-anchor byte inverse restores the exact base Git driver, SHA-256
  `0131bee1842f3856507def3f41d4d45f0a9b38511167886461fc96d8a32d37fe`. Restoring only the wiring in the four historical
  readers also reproduces the base Git files byte for byte. Original helpers,
  fixtures, pins and live source attribution are preserved.
  [Source byte receipt](logs/source-bytes.json)
- ARM64 `batond` SHA-256 `f75eca51ec0ab4384e24a42143ac5d455e554bf5be4928428ecb24c39b976d84`. ARM64
  `baton` SHA-256 `6f70be052ebe1c072b8cc23f62a0b75f768d598ecae36f55311b4752f42f201c`. Both were reused
  from locked Rust build `cf04dc19e0e225d831d8c337c7ecf11e12f307fb` after comparing the Rust build
  inputs to this base and verifying the bytes of both tools.
  [Tool provenance](logs/tools.json)
- Locked local npm install and frontend build passed. [Frontend output hashes](logs/frontend-artifacts.json)

## Runtime and regression checks

| Check | Baseline | Candidate |
| --- | --- | --- |
| Actual private Core Unix IPC via the driver's Host and bootstrap | Core supplies integer 19, old guard refuses | Same integer 19 passes the guard |
| Actual daemon SQLite via the driver's database context | Schema 19 refused | Schema 19 opens in a read-only transaction, four Integration tables read, attempted write rejected |
| Isolated copies with real PRAGMA version 18/20 | Accepts 18, rejects 20 | Rejects both |
| Original historical readers | 11 failures with the new raw driver and unchanged readers | 34 existing cases and seven new controls pass |
| Full infrastructure | | 943 total, 927 pass and 16 existing skips |
| Exact existing workflow schema probe function against the real Core | | Refuses integer 19 with `Actual Core schema probe refused` |
| Unmodified browser CLI preflight using the honest schema19 manifest | | Exit 2, `candidate_schema_guard`, runtime `NOT_RUN` |

The same private probe ran the real driver up to the first project import. The
probe stops at that boundary after executing the real snapshot guard. It then
exercises the actual reader over the daemon SQLite file and isolated backup
copies with PRAGMA labels 18 and 20. File and WAL hashes remain equal across
each read, and Core sequence stays at zero. Both daemons returned zero and
their private runtimes were removed. The label variants exercise version
refusal. The probe did not migrate a legacy store. The first capture and the
repeated capture that explicitly closed SQLite writers and compared WAL hashes
are retained. [Baseline receipt](logs/baseline--receipt.json) and
[candidate receipt](logs/candidate--receipt.json)

The exact CI schema probe was replayed as a function on macOS against the same
verified private daemon, with the same resolved private path setup as the
workflow. Its existing check refuses the actual Core schema19 observation
after making the read. The first local replay used an unresolved temporary path
and failed startup before that check. The subsequent run reached the schema
mismatch and the daemon cleaned up. The standard local browser CLI refused
at its separate manifest schema18 check before starting any browser or runtime.
The live `source_guard` separately passed on the tested commit and produced
the SHA of the current driver. The evidence projector's separate schema18
probe checks and the report dimension label remain a follow-up.
[Workflow probe](logs/outer.json) and [browser CLI receipt](logs/browser-preflight-command.json)

Actual browser UI, Native App, CI and real Provider are untested here. The
integration preview used target `cde501db847d1bbfc27a298e16cd772afc1a41e9`. There was no Git conflict.
[Archive index with command receipts and SHA-256 values](LOGS.json)
