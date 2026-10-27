# Independent exact-head CI/native verification — 58b5

**Verdict: this exact CI/native evidence passes independent verification.** All seven checks are successful for PR head `58b5d329ff009f07eef5ec9776b15a2676504dc6`. Both newly downloaded native artifacts match API digests and actual checkout. This supports Primary's guarded integration decision after its separate DMG verification/current-head/protection checks; this reviewer did not merge, push, run Cargo, edit source/locks/index or execute downloaded artifacts. It does not complete the overall product goal or unrun manual/provider dimensions.

## Exact subject, API and actual checkout

API read **2026-10-27 06:28:42 UTC**: Product run `40458672955` and Environment run `40458672868` are pull_request, completed/success, with head_sha exactly58b5. All jobs below are completed/success:

| Required check | Actual job ID |
| --- | --- |
| Product / Rust |122037851303 |
| Product / UI |122037850985 |
| Product / macos-15 |122037851006 |
| Product / macos-15-intel |122037850702 |
| Environment / macos-15-intel |122037850036 |
| Environment / macos-15 |122037850193 |
| Environment / ubuntu-24.04 |122037850248 |

Both Mac logs and the Linux Rust log explicitly checkout **`8f19d8f7a4c6db032351a28c7629d8df7882c383`**, GitHub's actual merge of58b5 into protected main `66061439c79fa44b66a33782febc1fb0f31c73df`. Both artifacts' workflow_run.id/head_sha match40458672955/58b5, and their native JSON commits match8f19. Artifact names/report commits are not incorrectly equated with PR head. No e373/ada4/201f/7ba pass or package was borrowed.

## Actual tests, unchanged budget and release audit

Completed raw logs bind each result to this checkout. Exact Core command integration sections show **14 passed, zero failed/ignored/filtered** on Linux, ARM and Intel. Each Mac additionally shows **native host11** and **native feature harness6** passed, actual WebView/daemon acceptance passed, and the named-smoke-marker scan of the actual default release executable passed. The audit proves its named identifiers absent in that binary; it is not a formal proof about all possible development code.

| Observed field | Apple Silicon | Intel |
| --- | --- | --- |
| Core command14 duration |75.59s |72.68s |
| Native host11 / harness6 |passed / passed |passed / passed |
| Original grant remaining/request interval |999ms / 0ms |998ms / 0ms |
| Original sessions.create RPC |**587ms** |**665ms** |
| Profile readonly/reservation/drifted-launch/launch (µs) |226297 / 199861 / 226512 / 235509 |205708 / 308327 / 227319 / 348989 |
| Actual WebView/daemon + default release audit |passed |passed |

The original **1000ms** grant/expired negative,1s/2s/3s thresholds and strong initialize→session/new failure/owned reap/stopped intent/lease release/TaskOutcomeUnknown/separate command_passed-not-TaskAccepted assertions remain. Passed suites do not replace those assertions with a cancelled/ready/stopped-only fallback. The diagnostic60s profile fixture is not substituted for the original1s case.

Linux and Mac workspace steps explicitly run **RUST_TEST_THREADS=1**. This isolates independent fixtures; test-internal real worker/client/daemon threads and explicit concurrency cases remain. Earlier Linux cross-fixture pressure failure stays recorded as unmet; serialized success is not a claim that this pressure or arbitrary-load1s startup has been demonstrated. These are observations, not latency percentiles or a universal budget guarantee.

## Independently downloaded native artifacts

Only new native ZIPs from run40458672955 were fetched. API SHA-256 equals local bytes before extraction. The independent helper rejects traversal/absolute/backslash paths, normalized duplicate names, symlink/special/encrypted entries and oversized archives/files, then checks ZIP CRC; each archive has5 files. Nothing from the archive was executed.

| Native artifact | ID / architecture | ZIP SHA-256 (API = local) |
| --- | --- | --- |
| native-webview-macos-15-8f19d8f… |12713534720 / aarch64 |2062d773992c348eace5ad6c133a59f4cb9676f0f0fe0ed6c3cc680fa14f2e8a |
| native-webview-macos-15-intel-8f19d8f… |12714245741 / x86_64 |d25822b942357c78c1aec9d228eba63fd8053a4bf705402db785ee4c751876e8 |

Both native reports and external-runner records pass exact OS/architecture/commit, actual product composer/DOM, idempotency, invalid-input/journal, same persisted task on close/reopen, **Command3 + Data3 read-only** flags, UI-close daemon survival, rejected second writer, owned daemon restart/cleanup, reopened cancellation/stop evidence, exit0/no-timeout/no-error/owned-group cleanup. Task binding:

- ARM first/reopened: `2cf2fdc8-7bda-4bd2-b11b-e19c953cb352`.
- Intel first/reopened: `c9c2d2c1-de1f-4eca-8922-d10e0e7ba43e`.

The three flags per category establish real list/empty-controls/read-only-no-mutation behavior; they do not mean native command/data execution or native owner-dialog approval was exercised. Both reports explicitly preserve `native_dialog=not_run_manual_acceptance_required` and `provider_models=not_run_not_authorized`.

Both **new** PNGs were decoded/visually inspected with view_image: actual native task windows, ARM1136×793 and Intel1292×912. ARM captures Conversation; Intel retains an Artifacts physical frame despite the scripted return to Conversation. Keep that paint-timing limitation: these images do not prove an identical final tab or all below-fold command controls visible. The separate exact DOM/IPC assertions establish the read-only operations.

| Native file | ARM SHA-256 | Intel SHA-256 |
| --- | --- | --- |
| native-smoke.json |2c6f4343b4d5dfd17b736fa57d1d5dcddcea5e16b612eb9ac7aa001beca44409 |1e913bf6ad33a03904e5cc9e0fdb8623aaf40e5884b04cb384275a0d52f3e863 |
| native-screen.png |148db13e708f7965f7042ed737200dc55047305052c042d21b1f17e47c4891f1 |7740e75cf2d3c07770395049113a1c45c00d976ae7e8cc8f890077feec0bd57f |

## Evidence and limits

Raw API/job logs, ZIPs, extracted files and independent audit JSON are preserved under ignored `/workspace/.onboarding/command-native-58b5/`. Each audit JSON includes all5 extracted file hashes. All results here were read from this run; no private account/model/remote-host/signing/production operation was enabled. Development-package jobs succeeded, but **dual DMG/build-record/signature/checksum verification belongs to Primary's separate helper and is not asserted by this native-only report**. Real native owner-dialog acceptance and real Provider/model validation remain unrun as declared.

| Evidence file | SHA-256 |
| --- | --- |
| `product-run.json` | `36a344e083f843a64c9b2bdf7e1b202f9ce4500e82141d91276a0f1f0be6661f` |
| `product-jobs.json` | `e67cb6d6a6454f83dcff32b594dceb143ff500dacd4d7607fa5aeed8bf527c39` |
| `env-run.json` | `77fe3b62cce9d226a255afe23b57907301653d78590775cbf231fe9adce997a2` |
| `env-jobs.json` | `2a35c675f8cc5effb21e3d67f8940bdee2ee5763f46f7778bba292c03d99a4f5` |
| `artifacts.json` | `db9474ba702979be56bec0b43d8dfa35239f76af0c70aea25dd6a1b1dfe43419` |
| `arm-job-122037851006.log` | `0abd168f53a9477dd6f1dabc271eee4823bb48e0fd7ba3b3003ed4944b51e6b8` |
| `intel-job-122037850702.log` | `03b375876dddc3e17a33df83e27276ff0d50ba9aa1a0009d2c7893f845dff793` |
| `rust-job-122037851303.log` | `9d9b13fefaa696c699a9cce859b3280a4ff618953eb17a82f2790b896f53dced` |
| `arm-audit.json` | `f1c969e75b1e892083aa3a5325eaf29f681cffecd768b4163c3e4d8688bc5909` |
| `intel-audit.json` | `6cea926e3eb8ad90fcc1d7fc3fd06e8065504523c63e976e8c98c0b222f45ff9` |
| `audit-native-artifact.py` | `0b182023209a33030945aaf2e48643dad7c8d92756d6c265058fe45f47e0b7d7` |

Actual operations: gh api exact run/jobs/completed-job logs/artifacts metadata and ZIP-by-ID; independent Python API-binding/hash/safe-ZIP/report checks; view_image of both new PNGs. Current expected PR head, current main, required check state and branch protection must still be rechecked by Primary at integration. This report is evidence, not an authority bypass, a merge operation, a license choice, a production release or overall product completion.
