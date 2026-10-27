# Data workspace real IPC candidate evidence

## Exact candidate and source

- Inherited product base `aa1c2814a30f43405f3c545ec178cf665f96e673`, reviewed DataUI a436 unchanged.
- Harness source `ca35dc82200033feb15d106839c29762b1d85284`, script SHA-256 `a736ee8b125effe304d59f15ed3c042913773126e69a4029c9b11100162c5d02`.
- Coherent **WIP candidate**, not final review/CI acceptance: binary `636e41324108da50195585149a7e5730ea7a247a27e5f74c499f5d30c5381a5c`, manifest `01ce170843e65f7826e33d540d8f2bc82c902561c2256559a1b3ff20e155f17c`, dirty HEAD `152725b1423482cda318724d5af2d171e0f1c602`, sourceTree `b4b0c3071f31cb750351bb0c1fb9715734daed68a4aaf12510965a01a11e3263`.
- Frontend `/workspace/.worktrees/data-workspace-ui/apps/desktop/dist`, actual assets below; no source/build edits by this unit.

## Actual final command

```sh
python3 apps/desktop/tests/data-workspace-real-ipc.py \
  --batond /workspace/.onboarding/data-workspace-candidate-binaries/schema10-b4b0c3071f31/batond \
  --candidate-manifest /workspace/.onboarding/data-workspace-candidate-binaries/schema10-b4b0c3071f31/manifest.json \
  --frontend /workspace/.worktrees/data-workspace-ui/apps/desktop/dist \
  --chromium /usr/bin/chromium \
  --output /workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5
```

Actual exit0 / `passed` / **8 checks /6 screenshots/no page errors**. Python syntax compilation and `git diff --check` pass. No Cargo/native/macOS/model execution.

| Requirement | Actual candidate observation | Limit |
| --- | --- | --- |
| AB-PRJ-002/003 WP05 | Directory import; UI relative files/limits; actual count2/bytes39/SHA; host-approved real Ready copy; exact inputs/bytes and Directory registry | Host fixture, not OS native OwnerDialog |
| AB-VAL-002 WP09 | Source SHA/mtime unchanged; wrong expected hashes fail; deliberate source drift cannot become Ready resnapshot | Not algorithm/Task acceptance |
| AB-TSK-007 WP08 | Preview actual cancelled; response lost after real receipt; remount/read recovery without new mutation. Copy cancellation-specific failure before reservation | No in-copy/partial-output stop proof |
| AB-SEC-004/010 WP12 | HTTP403 approve/propose/finalize/generic/execution; forged source/foreign refs refused; benign runtime sentinel denied without persistence | No OS sandbox/DLP claim |
| Idempotency/history | Same-key preview/finalize sameIDs/no secondcopy; readonly UI refresh leaves counts/sequence unchanged; failed records rendered | Not process-restart UI-cache proof |
| OWNER-002 | Actual built React/Chromium screenshots1488x981 and390x844 inspected; no mobile horizontal overflow | Not native WebView/Mac |

Baseline WP06 covers capabilities; no spec mapping changed.

## Actual receipts and cancellation layers

Normal Ready `c405974e-284e-456d-a116-8e6ccda62994` has two copied files and Directory registry `c6f2c80b-8fb2-4775-9d7d-4489b987e9c4`. Host fixture verifies actual source/managed directory canonical path/dev/inode and exact managed destination before private finalization. Independent readonly SQL checks consumed proposal/original result, immutable snapshot agreement and `data_workspace_receipts` sequences **8<9<10**: owner snapshot digest `8a0c2cd93e7e7d55d25860db3c33e507f7e57961e6f5a6f88381ce0925473b55`, prepared snapshot digest `d680cda55e62929d0c2bfa14009dea9d11aea0d7e49990084c6708334167fc61`, Ready contract digest `b24e4f8dc3fe4e4d0ab328d746c4b1989bcc60489d86dcfea72ca57ef75be398`. Contract commitment is additionally recomputed independently from the actual stored struct preserving its field order and emptying only contract_digest. Public persisted observations are not JS private authority or a reconstructed witness.

Preview: real cancel_requested revision2/sequence12 response is retained then HTTP connection is closed before headers. Actual UI unknown state, dialog remount and read reconciliation lead to cancelled revision3; no repeated cancellation/new copy. Copy: requested through real UI, actual failed revision3/CANCEL_REQUESTED/not_started, observation null and destination absent. Report explicitly records `cancellation_stage=before_destination_reservation`, `in_copy_cancellation_observation=false`, `partial_copy_stop_not_established=true`. A cancellation-specific receipt is not proof of chunk I/O/partial stop/thread join. New Core `io_completion`/cancelled change is not in this candidate.

## Preserved chronology and deliberate negative

Initial syntax-only preparation is preserved in commit6eff. Run1/source996/hash444 reported an overbroad `cooperative_io_cancel_observed`; its untouched original JSON is retained and superseded semantically by split fields in source001e/run2. Run3/source0b7 only improved screenshot framing. Run4/source61a adds own-root/receipt guards and exclusive evidence directories. Run5/sourceca35 independently hashes the persisted contract commitment. No assertion deleted, deadline extended, DTO fabricated, fake Ready inserted, or HTTP authority added.

All five actual runs exit0/eight checks/six screenshots; no candidate assertion failure occurred. Separate deliberate reuse of the run4 output directory exits1/FileExistsError **before starting a daemon**; independently compared hashes of every existing report/log/PNG remain identical (`exclusive-output-negative.log`). This is not a product failure or suppression.

| Artifact | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run1/report.json` | `e983cf2d42d334b8c07a6900e377a6a1d43db49b70e06058bd41901eb514f450` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run1.log` | `2ef4c863bae3bbb86340d056af1201a48883c4738af0d59c04a9e2d28aeb7e9e` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run2/report.json` | `ec493eb8a856d8085f89f56c5291af658188e25f51b7a4675cee96703c8bbed0` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run2.log` | `a2499b657a0ee692d6ddc68c4ba1e57e654e17167871c111ffafaec0077a140a` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run3/report.json` | `cc71f5f7853a98c3b7a3c52cc3f81db4748227dae3225c4629fa38b4586e7003` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run3.log` | `7a729b3819e6ddcfae544cb6e3787d246566d0032a92a5b92cdd9426768c4558` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run4/report.json` | `0cd54b33b012b6c74fd2fc2c30c58cd02d08ccf1ce5dface1983ce677f97c91d` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run4.log` | `d9c100da6903b07d28dc3d5aaa0d691a0a9389ae285eae2bd53bb1ed8a2c48fb` |
| `/workspace/.onboarding/data-workspace-real-ipc/exclusive-output-negative.log` | `1a6e6fe6bb482e066f7c936a73b9b974e9711e5627e29cc80f0df02cc853dc19` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5.log` | `025c66e77bb6e1578f613bbf1e320dbeb9102fce51eccf3ad1a3f400a1d19a3e` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/report.json` | `d9fb4365889c305b0140434216fdaf254e51d88d08bf852f07b86de6d4c7b5f7` |

## Frontend and inspected actual screenshots

| Frontend file | SHA-256 |
| --- | --- |
| `assets/index-Cj0Bu_O8.js` | `700b2d52f14affd5a82f5884b5644fef8a9639bef28ec7e632be8f198ee8f001` |
| `assets/index-D6UB0Jo6.css` | `1208bc54eb8aeea54bca803a4d78dc7435688106f6b6eb0224dfbf1b5b390595` |
| `index.html` | `71318408d4586c00df756b7f4da5eaf39cd90f8a1961b978bef302965a9bef18` |

All final six images were opened and inspected; desktop1488x981, mobile390x844. Real product UI, not prototype/native/Mac.

| Screenshot | SHA-256 |
| --- | --- |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/01-real-selected-file-preview.png` | `8356d864fabb95b7b96ca47bf0ffcc77ace4241eeafff84145cc396f3e96ac80` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/02-real-ready-data-copy.png` | `f139cdb8dc31713e72f0d697268830e56d96d6fbcf1ff88fba12ec302ae32043` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/03-real-ready-mobile.png` | `083d00790943144b19e4864d9aaf9b56b199ebceffd9fd50b204b369e67bdce3` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/04-real-cancel-response-lost.png` | `7c5dcd0fe2b9fc83c20c46cdd5158ac8d4f5adf790c7f57646d97b526ac7251a` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/05-real-copy-terminal-observation.png` | `c4b37495feb909f40c7f0aebd8a32303af956d991c19dc29c9caa64e0254efc1` |
| `/workspace/.onboarding/data-workspace-real-ipc/candidate-b4b0-run5/06-real-historical-failure-and-drift.png` | `8ad519b728278389236595c33f3725daad69a77ded0799cb9707ce7cc4d976f9` |

## Boundaries and remaining work

Strict adapter binds loopback/exactOrigin/Host/JSON/key/count/bytes/reference scope. Only generated sibling sources/runtime; only own new runtime token inside host authenticated Unix closure, never dataset/HTTP/report/log/screenshot. Fault only drops one actual cancellation response; real receipt/sequence retained. SQLite is readonly/query_only, no fake Ready/authority/connection rows. Owner fixture privately calls actual proposal/finalize outside HTTP; no native OwnerDialog claim. Generated temporary trees cleaned only after actual source/copy/retention inspection.

Core full schema10 migration/crash/cancel/regression/independent review, actual chunk/partial-output stop, later io_completion/cancelled contract integration, native OwnerDialog/WebView/macOS/final CI remain separate/pending. No model/account/private identity/extra spending/signing/production/Task acceptance claim. Independent harness REVIEW is reviewer-owned and not staged by this author.
