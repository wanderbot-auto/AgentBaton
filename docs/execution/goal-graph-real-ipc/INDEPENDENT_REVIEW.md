# Independent Owner-only harness review — PASS within assigned scope

Reviewed detached worktree `/workspace/.worktrees/goal-graph-owner-harness-review`, HEAD `f9943b2e7ccf6f413edfdcd7d2774f1486d6334f` (four documentation records), source commits `f0deb2790eb39c9b8d2cb3fadfa5ab797089fa94` and `5b7e401780ab393a13fccd1c54b64362cdbef452`. New driver `apps/desktop/tests/goal-graph-real-ipc.py` SHA-256 `84e90a6bf6250441c39da2f4ebf09ca59dc75401b10d870859452f81d5315129`. Baseline protected main `a1be94d26d10bb245bb35eb50226b82012c967da`. Exactly this new script and its four records differ from baseline; all five original harness files match baseline and the author’s recorded hashes. Source snapshots and both narrow patch diffs are retained here. No product, Core, old harness, authority, Native parent, lock, CI, target or remote was modified. No Cargo, native target lease, formal GitHub approval or recursive agent was used.

There are no blocking findings for this Owner-only driver/transport/cleanup iteration. This is independent code review plus a fresh actual Linux browser/Unix IPC run, not full Graph or whole-product acceptance.

## Actual independent execution

From the detached worktree, this exact command exited **0**, with **6 checks and 3 newly captured PNGs**, writing a fresh output directory rather than copying the author's run:

```sh
/workspace/AgentBaton/artifacts/browser-venv/bin/python apps/desktop/tests/goal-graph-real-ipc.py --owner-only --batond /workspace/.onboarding/goal-graph-owner5541-native94157/batond --candidate-manifest /workspace/.onboarding/goal-graph-owner5541-native94157/manifest.json --frontend /workspace/.worktrees/goal-graph-integration/apps/desktop/dist --output /workspace/.onboarding/goal-graph-owner-harness-review/actual-owner-only
```

`actual-owner-only/report.json` SHA-256 **`f6d55cb0689069882f734d3cbdd649de1c60fcac925beb3c758d8768cd88f91f`**. Core daemon SHA **`fc5d33dca48d4b46dc7c5aa8eaf2235be8222f04ea959ba139561c0fa380d77e`**; candidate manifest SHA **`dd76425e46e99fad9e6fdd58c17f232f992ffecde13b8cdb2bd002f546e4b325`**. Independently rehashed all **84** source objects against the manifest's exact composed Git head `94157aed2cb69c6b503c5b6b12af5aff554476af`; this binds the supplied Core5541 binary, not whatever source is currently in a different worktree. No compiler was invoked.

Corrected UI4ead frontend assets were independently hashed before the run, compared with both the author’s positive report and Root’s corrected legacy report, and checked unchanged by the actual driver after execution. Exact JS SHA `4472889886faf0fc9f9d1f39faa0e0053ca47a22be7d7e21c191e67a08fa0bd8`, CSS `785c729e596607549ed99b807d29608310cf49dc4660f473492ffc17a27647ac`, index `3d83ddee4f79c6d56431ffb6c32f2de936f03cb40a73226360485731d30fac99`. No UI build/edit occurred in this review.

## Chain and durable evidence

The actual driver generated its own local account-free ACP program and project, authenticated to its own Core Unix socket, stored complete plan revisions 1/2 via the built UI, and ran the fixed **Work → Owner → dependent Summary → Owner** chain: four nodes, two owned ACP processes and **zero deterministic Validator nodes/results**. Planning did not launch processes. Before first Owner approval only the first candidate existed, no dependent session/output existed, and TaskAccepted remained false. Summary consumed the exact accepted upstream artifact; final acceptance waited for the second separate Owner decision.

Both complete actual TaskRequests are retained, independently UTF-8 decoded and rehashed, including exact node, workspace, run/plan digest, input candidate, outputs, limits, lineage and return-location bindings:

| Node | Request bytes | Request SHA-256 | Output bytes | reservation → intent stop → supervisor stop → capture |
| --- | ---: | --- | ---: | --- |
| produce-report | 3702 | `7dd98414e438e5fb81afad249beafcbe4ee0e12615952604c4c32b95d7f0cbfe` | 12 | 24 → 34 → 36 → 39 |
| independent-2 | 4484 | `9880146b598f397d7140cbb279c41dbc162d1816eb70ecb6d35b4974a6ea01e8` | 14 | 46 → 56 → 58 → 61 |

The independent decoder checks actual sequence numbers, not assumed timing. The two candidate SHA values bind `fixed report` and `extra report 2`; physical FD observations in the executed driver verified exact root device/inode and file pins, no-follow traversal, regular single-link files, stable before/after stat and full bytes. `baton_goal_evidence` candidates remain `task_acceptance=not_established`; `baton_supervisor` stop events prove reaping before `goal_core` capture with `io_completion=worker_joined`. Owner nodes remain process-free (`session_id/attempt_id` null, stop_confirmed false), while connection nodes retain candidate state/confirmed stop; all required acceptance projections are accepted.

Private host Owner review checked exact proposal kinds and full bounded proposal/snapshot, without digest-only review or widening the legacy 16 KiB cap. Both actual decisions were `local_owner_confirmation`, same-action same-key ordinary replay returned the same receipt, opposite action returned CONFIG_CONFLICT, and same proposal key recovered the consumed original proposal. These returned-response replays are **not** actual dropped-response recovery tests.

Fresh run `d3944f36-475a-45cf-93cd-fe03c6c918f6` accepted task `4f65c1b1-7539-4895-9f8e-3d27bb004586`, acceptance ref `a04cc68d-f318-4e58-929a-10b90a45faae`, sequence **66**, task revision **8**. The driver executed read-only SQLite assertions of schema12, persisted candidate/Owner/TaskAcceptance Value equality and raw-document SHA versus exactly sourced event digests, full acceptance refs/Owner decision IDs, and task revision/acceptance sequence. Both root/project attempt budgets read consumed=2/active=0; both root-grant/project ledgers read observed_attempts=2/output_bytes=26. Terminal cancel returned the accepted task unchanged; ordinary session reopen was refused, and three subsequent graph reads kept run/ledger stable. The script's nine SQL calls are PRAGMA query-only/user-version, BEGIN and SELECT only; connect uses `mode=ro`, no SQL writes/forged authority rows.

The temporary DB and workspace were removed by the driver's normal cleanup. The retained report contains complete semantic values and recorded digests, not the original SQL document serialization. The independent decoder cross-binds those recorded fields; it does not pretend the removed database still exists. Its first checker incorrectly attempted to reproduce the SQL byte digest by reserializing parsed IPC Values, whose key ordering differs from Rust typed-document serialization. That reviewer error and checker are preserved as `integrity-audit-first-failure.log` / `integrity_audit_first.py`; only the reviewer checker was corrected. The unmodified actual driver had already hashed the original persisted bytes within the read transaction.

## HTTP/transport and cleanup

Actual browser requests denied all six Graph approve/Owner approve/reject/finalize/decline/generic commands with HTTP403. Successful actual plan store/revise requests retained exact generated idempotency keys and expected revisions 0/1. Source permits only the finite fixed read/plan-store/cancel/advance set, strict task/scope/run ownership, safe integer CAS and exact fields/Origin/Host/body caps; it exposes no private Owner or arbitrary execution route.

Independent actual static checks on the final frozen script passed: **58** outgoing boundary cases, **15** constructor/schema/readonly-FD cases, **9** disconnect-classifier cases. In addition, **11 independent unit controls** execute AST-extracted actual HTTP Handler/error recording and full-review code with fake IPC/socket outputs. They verify readonly response BrokenPipe classification, mutation lost reply rejection, arbitrary handler error/Core reset rejection, foreign nested task/scope rejection, and wrong-kind/digest-only/missing-field/oversized review refusal. This is unit fault injection, not live network-drop or actual Core stale-CAS/failure-matrix validation. Syntax, source preservation and baseline diff whitespace checks passed.

After actual browser shutdown, HTTP serve thread joined with `thread_alive=false`, non-daemon request handlers joined with `active_handlers=[]`, and only then were errors inspected. Daemon **595394** waited with exit0, no forced kill, original process gone. Both real TaskRequest-bound generated workers **595856** / **595878**, exact PGID/start-ticks identities, were observed absent (`observed_start_ticks=null`, `pid_exists=false`, signal_sent=false). This proves more than an empty cleanup list. Both HTTP error and readonly-abort lists are empty: no real reply-abort occurred in this positive run. Source ensures mutation reply loss/unknown exceptions poison the final verdict even if the browser path previously appeared successful.

## Actual original images

All **three fresh exact PNGs** were forwarded through `view_image` plus `image` and independently viewed at original 1488×1200 resolution. Images 01/02 show pending acceptance, four node states, complete Owner approve/reject controls visibly above the composer. Actual DOM corner/center `elementFromPoint` samples are unobscured for both controls. Image03 shows Task accepted, two accepted Owner nodes, two candidate outputs and result checks 0. Linux built-UI screenshots are not Native/macOS/OS Owner-dialog proof.

The first image happens to render byte-identically to the author's first image; it was captured by the fresh independent run, not copied. Images 02/03 have new SHA values and the final image shows this run's own ID. All three source-path/hash records are in the fresh report and full manifest.

## Preserved failures and limits

Author f0deb actual first run remains failed/interrupted, one check/one failure PNG, exact NOT_FOUND from `goal_plans.get` before UI asynchronous plan storage. Report SHA `2110f6c8161ff627581df69540c89a1cd434cbe2fc3f989164f7844913c17902`, raw log/PNG copied unchanged into retained inputs. Subsequent real propose success remains in that raw report. Source5b switches only that observer to readonly list-empty-until-target-revision, preserving the original 15-second wait and exact full plan/state/dispatched assertions. No timeout/acceptance relaxation or product repair is hidden here.

Author final actual report/manifest remain separately identified (`ebd3e204…b39` / `80decddfe05f397b873e16e6edf3ca94fd06df6fe5337de2809a8938e2b1e1a9`); all five author run-manifest raw files were independently rehashed. Root's two earlier d291 positive reports are preserved as earlier 5-check/3-image runs, not reused as this run's result; corrected-ui raw log retains its real BrokenPipe traceback.

Not run/accepted here: deterministic Validate bootstrap/reuse/subject rebinding or full five-node flow; fresh full Reject/drift/cancel/unknown/stale-CAS/ledger matrix; actual dropped-mutation-response recovery; complete product/Rust/Native regression; Native OS Owner dialog; macOS; live Provider/model; OS sandbox enforcement; new CI/release audit/development packages/signing or full Design v0.2 completion. The generated local ACP/host Owner fixture adds no account/model credentials or production permission. Root retains integration and future acceptance decisions.
