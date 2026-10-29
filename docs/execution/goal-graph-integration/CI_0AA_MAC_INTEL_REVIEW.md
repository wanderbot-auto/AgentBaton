# Independent Intel macOS CI evidence diagnosis

Recorded 2026-10-28 UTC after Root completion notification. This report and manifest are additive; the earlier sealed ARM report/manifest remain byte-identical. No source change, Cargo, daemon/model invocation, remote mutation, CI retry, timeout or authorization change.

## Identity and downloads

Product run `40761824261`, attempt 1; Intel job `123075460377`, macos-15-intel, completed failure at 20:48:16 UTC. Feature `0aa20c968d493cad895e4a3853bdf106d6b54acf`; actual checkout `5caecb5f9945b5fdb548f0e071a78e2d97537403` is independently in Intel job log lines 93–143. The prior single commit API record binds that checkout to tree `1943596e0bd12177d852117dab86c81ca30e6858`, identical to inspected feature source. Reuse `source-snapshot/` and `source-map.json`; no second commit API read.

One Intel log download: `intel-job.log`, 403866 bytes, SHA-256 `7b9106d126cc7c9433aee76dfe266d12b1928a8d93c27133b8f980a5d4480888`. One refreshed artifact-list read after completion; one Intel native ZIP download, artifact ID `12849993665`, 227398 bytes, SHA-256 `1e821b06164644439813d8755e561f55e4364aa49ac5911116bc590383944aa8`, exactly matching API digest. ZIP CRC all 16 members passed, no absolute/traversal/backslash/symlink members, bounded member sizes. `intel-artifact-inventory.json` lists every original ZIP mode, CRC, member hash/size. The 0600 stream ZIP modes and separately reported remote metadata are evidence; local extracted permission defaults are not substituted for them.

## Intel's original failure and phase

`intel-native-artifact/owner-dialog/owner-dialog.json:4` independently reports **`Exact full native title/message not uniquely present`**, status failed, dimension macos_own_process_real_sheet. Intel log line 3525 is the native `OS dialog smoke failed; inspect preserved evidence`; 3526–3529 retain Python's first exception `Real dialog process failed; inspect retained native report` at runner line 455. Lines 3533–3539 then show the cleanup exception `Native app did not exit cleanly; no successful result` at line 475. The latter overwrites the runner-level saved error, so the native report and first traceback must remain part of causal evidence.

Same-source guard `native_owner_dialog/macos.rs:251–252` checks exactly one title NSTextField and one complete message NSTextField after traversing a visible, caller-attached sheet with matching parent and stable sheet number for at least 500 ms. Initial phase is cancel (`431–436`). This fails before controller acceptance (`259`), exact-window capture (`326`), ready/per-click database permit publication, and real button click (`383`). Intel's archive contains no ready/reply/permit/per-click database files, no cancel/approve sheet PNGs, no after.json or result.json. The actual Cancel and OK paths and final database acceptance were **not reached**.

Intel independently reaches the same guard as ARM. This does not prove the same underlying cause: neither archive preserves actual matching counts, text values, view classes, or a failure-sheet screenshot. It cannot establish which exact field was missing/ambiguous, whether text was split/changed, or whether OS view behavior differed. No diagnosis of clipping, truncation, account permissions or a production defect is justified by this run. Root has separately assigned additive test-only failure observations and an exact own-sheet image; this diagnostic task changes nothing.

## Intel bootstrap, saved database and cleanup

`daemon-stderr.log:1` reports API v1 ready at `/private/var/folders/20/jp1_0n3n7kndh6rnbqb5344m0000gn/T/ab-own-fnvnri6i/runtime`. Path report records root/runtime uid 501, mode 448 (`0700`), directories and not symlinks; socket path is 92 UTF-8 bytes under limit 103, no short fallback. Daemon stream metadata records 384 (`0600`). The missing socket in bootstrap-paths is a pre-spawn observation. The last_readiness_error missing session.token in bootstrap-state is retained earlier state, followed by actual successful IPC, not terminal failure.

All seven retained Core bootstrap calls succeed: snapshot; project import; isolated workspace create; generated connection propose; finalize only that version-only connection; list showing ready/succeeded/reaped version probe; Grant propose. No target Grant finalize appears in this transcript. The generated executable version probe is not real model execution.

Saved before.json is schema12, sequence8, grants0 and all original19 Goal/attempt/session counts0; pending proposal and result are unconsumed. Running the unchanged pending validator only against saved evidence yields no error. Offline production-template recomputation exactly equals Intel fixture.message, **3136 UTF-8 bytes**, SHA-256 `af80eba01bebcc28e39ebc77b6d946813db5ab7e98be9acf3fc95607b337e2c2`. Intel and ARM include different real references and are not assumed to have equal complete message hashes. Source/fixture agreement does not reveal actual NSTextField values. No after snapshot was uploaded, so post-failure database emptiness/unchanged journal is not newly established.

bootstrap-state phase yielded_before_native_owner means preparation yielded into native work, not failed startup. App cleanup: pid56677, exited1, already quiescent group, no signals/errors; okfalse correctly preserves failed app exit. Daemon cleanup: pid56423, was running, SIGTERM, exit0, group quiescent, oktrue. This is not evidence of surviving owned processes, and must not be relabeled a successful Owner flow.

## Actual Intel validation dimensions

- Core/native step succeeded. Native host tests: 57 passed/0 failed (log3431). Native-smoke: 26 passed/0 failed (3499). Owner harness pure observation controls: 4 passed/0 failed (3521).
- Actual x86_64 WebView/private daemon acceptance succeeded. Saved native report binds checkout5ca and records real product composer/submitted task, restricted IPC, schema12 Goal execution and Graph six zero counts, journal sequence3 unchanged, exact missing-reference not_started errors, original nine lifecycle stages, background survival/restart/reopened task/cancel evidence, and owned daemon/group cleanup.
- Actual WebView crypto.subtle.digest/TextEncoder checkpoints before/after/first/reopened each preserve original109-byte Chinese/emoji/unsafe-u64 literal input, SHA-256 `d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892`. This is PR16 schema12 evidence, not Factory schema13 or Node simulation.
- Actual Owner Grant dialog flow failed20:47:56–20:48:09, before first click; postclick database acceptance not reached.
- Default application release build, default release test-control audit, development package/evidence and package upload were skipped after that failure. No Intel current development install package or release-binary audit is delivered from this job.
- Real Provider models, real graph private multipage Owner flow, signing/notarization and production release remain not_run. No validation process was rerun locally by this review.

## Actual screenshot inspection and preservation

Opened `intel-native-artifact/native-screen.png` with view_image, SHA-256 `294d4dc48443b730d256bd072a40671f442fd5ec860ebc2a9b2b8b9549fb0ada`. It shows the actual task Details tab, draft state, task ID matching saved report, local daemon connected, native readonly Goal project, revision1, request-cancellation control and warning that a request is not proof of stopped process. No Owner sheet is visible. It cannot supply Owner visual acceptance.

`intel-source-preservation.json` confirms original source-map bytes remain unchanged, source tree clean, c8 Factory leaf clean and unchanged. Offline saved-evidence checks used compile/exec of the retained Python module source, without import bytecode writes. `ARM_REVIEW.md` and `ARM_MANIFEST.json` are not changed; this report supersedes only their then-pending Intel status through additional evidence.
