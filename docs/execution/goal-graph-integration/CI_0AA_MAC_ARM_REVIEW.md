# Independent ARM macOS CI evidence diagnosis

Recorded 2026-10-28 UTC. Read-only diagnosis; no code change, Cargo, daemon/model invocation, CI retry, remote mutation or approval. Execution boundary and OWNER-001/002 read and acknowledged for this diagnostic scope. Managed environment running, current observations and network policy enforced.

## Binding and original evidence

Product run `40761824261`, attempt 1, ARM job `123075460314`, feature `0aa20c968d493cad895e4a3853bdf106d6b54acf`. Checkout log lines 93–106 identifies PR merge `5caecb5f9945b5fdb548f0e071a78e2d97537403`. One commit API read independently verifies its tree `1943596e0bd12177d852117dab86c81ca30e6858`, identical to the local feature tree used for source inspection. Sources and Git blob/SHA-256 map are retained under `source-snapshot/` and `source-map.json`.

One ARM job-log download: `arm-job.log`, 386396 bytes, SHA-256 `69d678d64aa0170870d327a276d2f9361e593f4175377f78c0bd0aaa42845edc`. One artifact-list read and one ARM native artifact download: ID `12849482814`, `arm-native-artifact.zip`, 173721 bytes, SHA-256 `8274f44b8a6884fcf7d6fcfd4f05bd0d28472018edbb548b823ea241e6b5619d`, matching GitHub's digest. All 16 members pass ZIP CRC; extraction rejects absolute/traversal/backslash/symlink entries and caps member size. `arm-artifact-inventory.json` retains ZIP original modes, CRC, member SHA/size; local extracted modes are not claims about original filesystem state.

## Failure, stage and what it does prove

`arm-native-artifact/owner-dialog/owner-dialog.json:4`: **`Exact full native title/message not uniquely present`**, status `failed`, dimension `macos_own_process_real_sheet`.

Job log line 3393: `Error: "OS dialog smoke failed; inspect preserved evidence"`. Lines 3394–3397 retain the first Python exception, `RuntimeError: Real dialog process failed; inspect retained native report` at runner line 455. Lines 3401–3407 show cleanup's later exception `RuntimeError: Native app did not exit cleanly; no successful result` at runner line 475. The latter is the message recorded by `runner-failure.json` and `bootstrap-state.json.original_failure`; it must not erase the earlier native failure or be treated as independent bootstrap cause.

Source `native_owner_dialog/macos.rs:251–252` emits the exact native failure after a sheet is attached to the actual caller NSWindow, parent identity checked, visible, same sheet number stable for at least 500 ms, and a traversal of its content. The condition requires exactly one NSTextField matching the exact title and exactly one matching the complete expected message. This is the initial **cancel** phase (`macos.rs:431–436`). It fails before `controller::validate` at 259, before capture at 326, before the ready/database-permit handshake, and before `NSControl.performClick` at 383.

There are no cancel/approve sheet-ready files, replies, sheet PNGs, per-click database records/permits, after.json, or result.json in the uploaded inventory. No actual Cancel or OK success, Grant finalization, or final database acceptance is proved by this run. The screenshot present is from the separate successful readonly WebView step, not this Owner dialog.

The original report does **not** save actual matched counts, traversed text, view classes or a failure sheet image. Consequently it cannot distinguish missing title, missing/changed/split message, ambiguous duplicate fields, or an AppKit view-type/layout change. It does not establish text clipping, production formatting defect, or any specific OS behavior as root cause. Exact view/text diagnostics and a failure screenshot while retaining all original gates are the narrow next evidence needed; fixes are outside this task.

## Bootstrap and cleanup observations

`daemon-stderr.log:1` says `batond API v1 ready` at the canonical private runtime. `bootstrap-paths.json` records root/runtime owned by uid 501, both mode decimal 448 (`0700`), neither symlink; original socket path is 92 UTF-8 bytes, below the explicit 103-byte bound, no short fallback used. Snapshot's absent socket is from before spawn, not proof readiness failed. Captured daemon stream metadata in bootstrap-state records modes decimal 384 (`0600`).

The Core transcript has seven successful requests: initial snapshot; project import; managed workspace create; connection proposal; finalize **only the generated --version connection**; connections.list with ready/reaped successful version probe; Grant proposal. No target Grant finalize appears. The generated executable only reports version and rejects other invocation; this is not a real Provider model test.

Saved before.json has schema 12, sequence 8, zero execution Grants, all original 19 Goal/attempt/session tables zero and the exact pending proposal unconsumed. An offline call to the unchanged pending validator returns no error. Offline recomputation from the actual production formatter equals fixture.message byte-for-byte: 3132 UTF-8 bytes, SHA-256 `c78bd560b64451be46a7df94653a5d72ffea77675c895b54a9f6d3293e3ca17c`. This validates fixture/source agreement, not what AppKit rendered. No post-failure database snapshot was uploaded, so zero-after/unchanged-after is not inferred.

bootstrap-state phase `yielded_before_native_owner` means preparation completed and control yielded into the native body. Its `last_readiness_error` is a retained earlier missing session.token error; later successful requests disprove treating that stale observation as the terminal cause. Daemon cleanup is `ok:true`, SIGTERM, exit 0, owned group quiescent. App cleanup observes an already exited code-1 leader and quiescent group, no errors/signals; `ok:false` correctly preserves unsuccessful app execution. This does not prove lingering processes, and must not be weakened into success.

## Independent validation dimensions

- **Succeeded on actual ARM macOS:** Core/native host step; 57 host tests (job line 3300); 26 native-smoke tests (3367); actual readonly WebView/lifecycle acceptance. The latter's saved report binds aarch64/checkout5ca, original readonly schema12 Graph six tables and Goal execution counts zero, journal sequence 3 unchanged, actual product composer and real restricted IPC, close/surviving daemon/restart/reopen/cancel evidence and owned cleanup.
- **Actual WebCrypto succeeded in four real WebView contexts:** before, after, first report, reopened report; original UTF-8 109 bytes including Chinese/emoji and unsafe-u64 literal text, SHA-256 `d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892`. This is the schema12 PR16 run, not Factory schema13 validation.
- **Owner test compilation/pure controls succeeded:** 4 tests, 0 failures (3389). **Actual Owner Grant OS flow failed** before either click; final OS/database acceptance not reached.
- **Skipped, not failed compilation:** default release application build, default release test-control audit, development package/evidence steps and package upload after Owner failure. No current release package or binary audit exists from this ARM job.
- **Not run/not authorized:** real Provider model calls, graph private multipage OS Owner decisions, signing/notarization, production release. No new checks were run in this diagnostic task; only saved evidence/source comparison.

## Screenshot inspection

`native-screen.png` SHA-256 `10349267f14f3af41b74292aed4f628f3aaf5858c936729c12821e776ddf528d` was opened with view_image. It shows AgentBaton's actual task page titled `原生 WebView → IPC persistent task`, draft state, local daemon connected, native readonly Goal project selected, empty execution evidence and configure-execution controls. No Owner sheet or approval message is visible. This image cannot supply missing Owner visual acceptance.

## Preservation and Intel boundary

`source-preservation.json` confirms inspected feature sources unchanged and clean, Factory readonly leaf still `c8bafaec1cae4c953230b53c56ab4ae711e168aa` and clean. An offline import created one Python bytecode file in the inspected source tree; only that exact own file was removed immediately, preexisting bytecode untouched; cleanup is separately recorded. No source, old evidence, checks, timeouts, authorization, target or c8 leaf modified.

Intel job `123075460377` was read once at 20:45:42 UTC: in progress, Core/native host step active. Its later actual Native/Owner/release outcome remains pending until Root completion notification; no polling and no Intel artifact/log download yet.
