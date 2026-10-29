# Factory Data/private-ticket review — fixed053

Subject `053b4327bb580cf42cb0fe28688f1465d6c8ce22`, baseline `137e24698090c73349c9560868c2a38ad97c627b`. Source was exported only from fixed Git objects into this review's `source/`, pinned in `manifest.json`. Moving author Data/cooperative-restore changes are excluded.

Scope: Data authorization freshness, non-Deserialize private dispatch tickets, request/deadline association, Launch/fixed Prompt/scoped-tool boundaries, witness-independent Owner cancellation/internal stop, owned join and accounting. Original FR01–04 and53e scope fixes remain preserved; this is not a broader architecture/product acceptance review.

**One new P2 accounting correction is required in fixed053. No new wire-authority bypass was found within the inspected normal-path wiring.** The known cooperative restore cancellation/deadline gap also remains open and prevents a full responsiveness claim. The Core author accepted the P2 and reports a later untested WIP fix; neither that source nor its future tests count for053.

No Cargo, daemon/companion, model/Provider, product tests, source/target/STATUS/DTO edit, new dependency, remote/CI/PR action, formal approval or recursive agent occurred. Python exported/hashed source and existing logs only. This reviewer independently executed zero Rust/product tests.

## FD053-01 — P2 — failed worker-boundary observation drops its own elapsed time

Path: `crates/core/src/sessions.rs:1587`–1598; normal accounting tail at1613–1615; consumer `persist_receipt`→`settle_shared_session`.

Trigger: a Data witness is admitted and the owned launch worker starts. Its initial `check_data_boundary()` spends time rechecking the held dataset descriptors, then rejects a later changed/missing identity or expired observation. This does not require a Provider or any new credentials; it can use generated dataset drift and a controlled delayed failure.

Static evidence: this outer branch directly returns `WorkerReceipt { never_started:true, elapsed_ms:prior, ... }`. It bypasses the common `started.elapsed() + prior` assignment. The receipt therefore retains queued/configuration time from `prior` but omits time spent inside this worker's initial failed boundary check. Subsequent shared settlement uses that receipt elapsed value as the available actual runtime. Failure must not refund already spent owned-worker work merely because no Provider started.

Necessary fix: route this failure through the same common elapsed finalization as all other receipts, without double-charging `prior`. Add a controlled delayed boundary failure proving elapsed includes the delay and prior, no Provider starts, and shared runtime/resource records are retained through actual worker join. Core explicitly confirmed the omission and accepted this fix; at review time its next-WIP closure change was not tested and is not reviewed here.

## Normal-path authority wiring inspected

### Base observations do not replace Data authorization

`factory_runtime.rs:160` `base_live()` checks current scope/expiry/revocation, exact approved Grant and full original Grant references through `policy::observe_grant_bindings`. The latter checks every original connection/workspace, including references omitted from the narrower Factory selection. Directory liveness here checks the native-confirmed copy/root identity only; this is intentionally a cheap read-preflight observation, not a full byte-authorizing witness.

`live_with_input` at223 then invokes the existing full `validate_grant_live_from_with_data`. Each scoped call's final `authenticate_factory_handle` at1926 reaches that predicate. Its `FactoryBaseCaller` wrapper is private and used only to obtain Data contexts; it is not returned as the full caller or accepted as consent. Thus valid handle/token/base metadata alone cannot authorize the four tool operations on a Directory dataset.

The scoped preflight obtains contexts for the **original Grant's entire workspace set**, not only selected workspace IDs. `DataInputContext`/`DataInputWitness` are crate-private, with private identity/hash/deadline fields, actual VerifiedDataWorkspace/held descriptors and no Deserialize/Clone authority. Native-confirmed creation records and current project/workspace/copy receipt consistency are rechecked before final action. Unknown/missing proof is denied, not replaced by a Directory label or boolean.

### Exact request and deadline association

Socket readers construct `ReadyRequest.factory_dispatch = None`. The private `FactoryDispatchTicket` at `factory_runtime.rs:1947` has private fields and no Deserialize/Serialize path; only the scheduler consumes a trusted `FactoryDispatchContext` to create it. Its raw request includes a fresh nonce plus scope/run/task/revisions/contract digest. Prompt tickets additionally bind exact session and prompt digest. `for_request` checks exact raw SHA, so a wire-shaped JSON object or copied IDs cannot attach the Rust ticket.

The bounded Data worker (`data_workspace_workers.rs:386`) carries the original ReadyRequest unchanged, hashes its exact raw bytes, and uses `min(original request deadline, original worker deadline)`. It does not renew that deadline on completion. Actor completion at `ipc.rs:1757` requires both current deadline and witness.for_request(raw), installs the witness only for that single continuation, and clears data_input/graph_input on the relevant completion paths. Scoped methods are reparsed/reauthenticated after the observation. Unknown/malformed/expired requests cannot silently become a new-key or automatically retried mutation.

### Launch and fixed Prompt both receive actual final authorization

The private route verifies ticket raw association before starting a read and again before execution. `factory_ticket_data_contexts` checks current run/task revisions, cancellation, active session and ready state for the prompt variant. Launch uses the private context through `create_session_for_factory`, complete Grant+witness validation, original lease/budget reservation, Factory-specific context checks and mark-dispatch validation. The witness is moved from the Actor into the actual launch permit and checked at worker entry and immediately before Provider spawn.

Fixed Prompt is not an ordinary renderer-supplied text endpoint: the ticket stores the scheduler's exact text/session and raw digest. After Data observation, the Actor rechecks ticket context and `send_factory_session` performs original full Grant+witness, exact session revision/readiness and lease/fence validation before queuing the fixed turn. Only the approved text is sent to the existing owned worker. These are admission point checks, not a claim of continuous immutability after dispatch or an OS sandbox.

The public Factory DTO prefix in factory.rs is byte-identical to137. The only file change there is a crate-private re-export of FactoryDispatchTicket. Public Owner methods and four Agent tools gain no ticket/witness/approval flag field or wire constructor. Existing unknown-field/method restrictions remain.

### Cancellation, true join and capture

Owner Factory run cancel/scope revoke/Grant revoke and internal stop do not require a new Data restore/witness. Scope tokens still authenticate through the full scoped boundary, including cancel_task; this distinction does not turn an Agent token into Owner withdrawal authority. Base-only supervisor policy/tick/internal-stop predicates are liveness/cleanup, not new launch/Prompt permission.

The Data worker publishes a retained admission result only after its actual thread.join. The original Provider worker likewise terminates/reaps then joins before authoritative receipt/resource release. No lease expiry, cancel response, private ticket or nonce is substituted for actual stop/join. Failed private launch preflight records owned queue/preflight work and blocks/cancels rather than replaying; pending/fatal errors do not fabricate a successful dispatch.

Post-stop capture consumes request admission proof into immutable native creation observation references. A new worker observation uses the already bounded execution deadline and a private capture-only subject, then Actor rechecks FD metadata and native DB authority before accepting observed bytes. It is not a reusable witness for a later request. Captures/results remain selected-artifact observations with acceptance not_established, not TaskAccepted/Git adoption or full filesystem/Provider-output accounting. FD053-01 remains an elapsed failure-path omission.

## Known restore gap — retained, not a duplicate task

`DataInputWitness::restore` (`data_workspaces.rs:1076`–1103) checks deadline before each dataset restore, but calls the full restore without a cooperative cancel/deadline control inside its read chunks. Deferred request handling checks cancellation and deadline again after restore, and Actor final admission checks deadline/hash once more. Consequently late observations are refused before a new action, but a large/slow restore cannot currently promptly stop at a read-chunk checkpoint.

This is the author's explicitly recorded existing WIP. Keep its resources/results owned until real worker join; do not claim full cancellation latency or hard restore-time enforcement from the normal test. Dedicated large restore/capture cancel, revoke/deadline/drift, private ticket/raw association negatives, actual authorized queue-expiry and mixed ordinary/Goal/Factory ledger tests remain unrun for this commit. Owner Git-input/companion scope/start preflight is still synchronously bounded and was not expanded into a new review task.

## Existing evidence independently verified, not executed

All12 hashes in `EVIDENCE.data_normal_checkpoint.sources` match053 Git objects. Supporting policy/Data-worker/validation sources were also exported and rehashed. `provenance.json` records those matches and all seven retained log SHA/size/result lines; raw logs are copied unchanged in `evidence/`.

- `data-and-git-seven-runtime.log`, SHA `7b7bd07d24194c532aea41a33f5e7dced901278a2f5d80aea4b9c11f187e8085`: **7 unique runtime tests passed**,0 failed/ignored,11.77s. This includes the prior6 plus the new actual Directory Data normal path. It is not7+6 or a new pass for the old14 scope/migration or593 workspace suite.
- `data-factory-first-test.log`, SHA `b478f1407184bf05f68d1993e27bd565fe6151608f64441cc46216c7635b899f`: the same Data case alone passed2.18s; it is a subset, not an eighth runtime case.
- `data-ticket-queue-unit.log`, SHA `8478821eddc757fedcbceeeb91c2099b4c4a222005182981c44ca1668cb9f431`: **1 queue unit passed**,69 filtered. It uses a fake/missing handle to prove deadline gate ordering, not an authorized expired delegate.
- First E0502 check failure and subsequent check/binary-build logs are preserved with the recorded hashes. No failed evidence is relabelled green.

The new Data test uses generated local files, actual private installed daemon/CLI copies, real Unix IPC, trusted-host Owner finalizer fixtures for Data copy/connection/Grant/Factory, and actual generated ACP→fixed MCP parent→child→new-parent flow. It checks the original Directory source bytes unchanged/no `.git`, completed Factory run, two task handles/three actual attempts, no live scoped handles and root Task remaining outcome_unknown rather than accepted. It does not run a macOS Owner dialog, Vendor model/account or production service. Its owned outer daemon is killed/waited at fixture cleanup; that is not proof of graceful OS daemon shutdown. The internal Provider true-join logic is a separate boundary.

The test copies already-built target/debug batond/baton and strips only private installed copies. This reviewer neither rebuilt nor executed those binaries; the12-file source map/logs are not a complete build dependency/binary provenance closure. No independent product-test PASS is claimed.

## Later shared-target tool provenance incident (separate from the fixed seven-test evidence)

Root subsequently reported that its21:26–27 Native compile borrowed old5541 sidecars, and tauri-build copy_binaries copied them into shared target/debug, replacing baton/batond. A later supposed053 frozen-binary manifest therefore actually referred to old batond `fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e` and baton `9065fa79d6381b51364d9304eb98aea16cf355d0cb81024134a07fb6ddef7c73`, not053/schema13 tools. Root acknowledges the build-script side effect; Core retained the erroneous manifest and is correcting it through a separate exact rebuild. This is a Root-reported later incident, not a new binary execution/hash observation by this reviewer.

The fixed053 source and retained7-runtime/queue logs remain the review subject; Root reports those tests occurred before this replacement. This report does not certify the later erroneous binaries as053, use them to establish schema13 or real UI behavior, or treat Root's106 pure Native compilation cases as execution of either sidecar. No shared target was read/changed by this reviewer. A later genuine schema13 frozen binary/manifest and real UI run require their own accurate build/source/hash proof; real schema13 UI remains NOT_RUN.

## Requirements and disposition

This is partial implementation/review evidence relevant to AB-TSK-002/003/004/007/008/012 and AB-SEC-003/010. Those requirements are not marked complete. Original FR01–04 fixes remain structurally in place, now with real request-local Directory proof instead of a witness-free bypass.

FD053-01 and the already-owned cooperative restore controls must be fixed/tested in a later exact source snapshot before accepting complete accounting/responsiveness. Negative authorization/cancel/drift/fault matrices, mixed ledgers, full regression/migration/lints, native UI/macOS/dual installers and full Factory/Design acceptance remain incomplete. No source/DTO/STATUS/permission or formal approval change follows from this review.
