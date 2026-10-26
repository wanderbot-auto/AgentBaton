# Independent macOS native success review — 9edf14e

Reviewer: native `/root/integration_review`, separate execution context. Independently queried GitHub API, downloaded/inspected both native artifacts and full macOS job logs, parsed reports with the actual native-runner validator, and visually inspected both PNGs at 2026-10-26T22:51:38.588000+00:00. Source was read only; only this review document was added. Evidence downloads are isolated at `/workspace/.onboarding/provider-native-9edf/`; old failure artifacts/reports were not changed.

Verdict: **PASS for the defined scripted native WebView/daemon lifecycle on Apple Silicon and Intel at exact commit `9edf14e152e59f98971ffce925814a520a33a0a7`, plus both actual default-release marker audits.** This is actual macOS CI evidence, distinct from prior Linux/browser/host tests. It is not full Design v0.2, manual acceptance or production-release approval.

## Exact CI and artifact binding

Product [run 40412628973](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40412628973) and Environment [run 40412633797](https://github.com/wanderbot-auto/AgentBaton/actions/runs/40412633797) are completed/success from independently fetched API records. All four Product jobs and all three Environment jobs succeeded, with the same exact 9edf head. The Environment names remain Ubuntu/macOS ARM/macOS Intel and remain infrastructure checks. These are workflow_dispatch checkout results; this review does not independently declare a PR merge candidate or branch-protection approval.

Mac job logs explicitly fetch and report checkout `9edf14e152e59f98971ffce925814a520a33a0a7`; native reports, artifact API metadata and uploaded artifact names agree. Runner images are macos-15-arm64 for ARM and macos-15 for the Intel matrix; binary-reported architectures are aarch64/x86_64.

| Architecture | Job | Native artifact | ZIP SHA-256 |
| --- | --- | --- | --- |
| Apple Silicon / aarch64 | `121886914385` | `12693114929` | `4808cedf558cf72630309ae64d7728737294328e1ae3c905e7605695c5dbb930` |
| Intel / x86_64 | `121886914498` | `12693384755` | `1508a7ca9269dc03619d5495a4b9438aca0c19627cba0f9ea363d8b07e1f6547` |

Both ZIP digests match both the supplied expected values and independently retrieved GitHub artifact digest metadata. Safe relative member names/no symlinks were checked before extraction. Each archive contains fresh lane.log, native-run.log, native-runner.json, native-screen.png and native-smoke.json; all bytes are fingerprinted in the verification manifest.

## Native checks actually established

Both native reports have status passed, os macos and the exact checkout. First-window DOM mount, actual product composer submission/task rendering, idempotency, invalid-input rejection and journal checks are true. The native reopened window reports DOM mount, the same task ID, restarted-daemon persistence, actual draft cancellation and no-attempt stop evidence. All nine diagnostic stages are present once, through reopened window build, script dispatch and report receipt; stages alone were not accepted as success.

| Architecture | First and reopened task ID | Visually inspected first-window PNG |
| --- | --- | --- |
| Apple Silicon / aarch64 | `29e952e2-3541-49b0-8bd6-c9f97f497e77` | 1136×793 |
| Intel / x86_64 | `3cecad86-b15e-451d-af57-ddbb7983bba7` | 1292×912 |

Both screenshots show the actual connected native AgentBaton window and the UI-created draft. They are first-window screenshots; no reopened-window screenshot is claimed. Reopened behavior is established by the correlated native report/invoke checks and stages, not by an invented second image.

Both runs confirm window-close daemon survival, refusal of a second writer process, daemon restart and owned daemon cleanup. Both external runner records report exit 0, no timeout, owned_group_cleanup true and status passed, without observation/cleanup/evidence errors. Both SIGTERM/SIGKILL results are observed_quiescent, exercising the bounded original-zombie-group proof on actual Darwin. Native lane logs also show all six native-bin units passed on each architecture and the final actual-macOS WebView/IPC/persistence/cleanup PASS line. The real native result is therefore no longer merely the partial first-window evidence seen at 4f4415f.

## Actual default-release audit

In both macOS jobs, building the default application succeeded before the step `Audit the actual default release application for test controls`. That step is success in API job metadata and the full logs show the actual command:

`bash scripts/native-audit-release.sh apps/desktop/src-tauri/target/release/bundle/macos/AgentBaton.app/Contents/MacOS/agentbaton-desktop`

Both commands print `PASS no native harness identifiers in default application executable` (ARM 22:37:31 UTC; Intel 22:40:26 UTC). This is an audit of the actual default release-bundle executable, not the debug smoke binary. The source scanner fails closed on inspection errors and checks the defined harness markers. The reviewer read these real CI results; release executables were not independently downloaded/reinspected here. Marker exclusion is a specific check, not a general binary-security or signing proof.

## Fingerprints

| Evidence file | SHA-256 |
| --- | --- |
| `arm/verified/native-smoke.json` | `ab725f85cc2d759f65afbab40b9879a9f821b5b9971dfdaf940a3b62fcce7c19` |
| `arm/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `arm/verified/native-screen.png` | `5710a543e2ba5ae1c7cc07a86e83341b108653b050e3cb0529968c8fe05b06db` |
| `arm/verified/lane.log` | `328973f65744d9fcf8d707c4fd2dab5c0d76249f35d954cdd510964d4fd7818f` |
| `arm/job.log` | `8052bd563ca34b346a9a6b0e38991268bf42b7db5192cb0da052d1928d952023` |
| `intel/verified/native-smoke.json` | `dd3b0c62aa5f3b9444c1a5a17894882606de209da1d1a759f6a73fcc22b8b0ad` |
| `intel/verified/native-runner.json` | `420778039d52fa4b7d743caaa832b6d26957a1b5578e61c7c8345fb65130a2b0` |
| `intel/verified/native-screen.png` | `1e64dca91ed47f539780ba7f03fe6524b8c02eb97b104669f716ba3aff75534d` |
| `intel/verified/lane.log` | `d7e039f00e2e36cea7d09b11ed31abee8bfe2f392d848ff081c83bd48ae620bd` |
| `intel/job.log` | `5a3866b87cb0a15bbfe124d01f88225838e5e11c125637d6dc80da7ac6b187e3` |

All evidence-relative paths above are under `/workspace/.onboarding/provider-native-9edf/`. The complete 20-file download fingerprint set and exact-source fingerprints are saved in `verification.json`, SHA-256 `e58178a9ee4f5a19280e95f228c3e5746f9ca5d93a9b853844887ac904902eae`.

| Source at exact checkout | SHA-256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `8eb1d8114d60a86eaabf4d316a9f4d7931b1c463a13b1624dee7ed65af19d31f` |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `scripts/native-macos-smoke.sh` | `d2228f8a325a386bf83171d41d5616c625fdfd5c16fa17494488e10329fc0886` |
| `scripts/native-audit-release.sh` | `dd60a45803ebc66c8dc5a19bba24da35b6e20716589b332314c7d1ecc4fa52d0` |
| `.github/workflows/product.yml` | `dae8b93309c42038015946557454fe45e1709ea3248cb766f5731c2decb56220` |

## Limits and retained failures

- The native run is a separate debug-only harness driving the actual built frontend and production typed IPC/private daemon. It covers a bounded scripted draft workflow, not active Provider work surviving ordinary UI quit, sleep/login or remote-host interruption.
- Native approval dialogs are explicitly not_run/manual_acceptance_required. Real Provider/account/model calls remain not_run/not_authorized; no such call was made during this verification. Native credential isolation/OS sandbox enforcement and vendor compatibility are not established by these tests.
- Only the tested macOS 15 runner images and two architectures are covered. Minimum macOS, user-hardware installation, accessibility/manual use, formal signing/notarization and production release remain separate gates. Full Design v0.2 remains incomplete.
- Earlier first-CI compiler/startup/cleanup failures and the 4f reopened-watchdog failure remain failures. New success does not retroactively prove their exact historical cause or relabel them. Their prior evidence is retained unchanged; the source-supported reentrancy correction and independently closed terminal-race finding remain documented separately.

No source, lock, commit, stage, push or remote mutation was performed. This report establishes the narrow current native success and its provenance; it does not authorize release or bypass exact-candidate review/protection requirements.
