# PR16 final composition review — fixed9448918

Subject: `9448918d17ba3a1159edf7051e140ea04e04e6df`, baseline `641be86e8b055920787cfebdd48270716d509309`.

**No blocking finding for this final test-maintenance/evidence composition.** The two narrow independently reviewed leaves are combined without additional production changes. This conclusion permits ordinary preparation of the next candidate within existing authority; it is not GitHub Approve, merge authorization despite failed checks, proven remediation of the historical Mac failures, Mac/installer acceptance or complete-product acceptance.

This reviewer exported the fixed Git tree into its own directory and audited that export, not moving author source. All1134 exported files remained byte-identical to the export manifest after checks. Root HEAD was still9448918 and its worktree clean when read at review completion. No source/Root/lock/workflow editing, Cargo/target writing, daemon, product tests, browser, remote API, push, PR mutation, formal Approve, Provider, credential or spending action occurred. Existing leaf/CI tests below were inspected, not rerun.

## Composition and retained acceptance standards

Git comparison with641 contains exactly three non-document/bootstrap source paths:

| Path | Exact independently reviewed leaf | SHA256 |
| --- | --- | --- |
| `apps/desktop/src/CommandValidation.test.tsx` | `bdb2a8860c38abe4e42c67b173a3452c06a760b1` | `56fd14320f4d53d9e88d6bfcb12c57534cf082296d644506e3fd108ab4e41bd8` |
| `scripts/native-owner-dialog-smoke.py` | `a43cbb181a52b9ebcc0c72a838f68497101759b8` | `872b1ba6e7dc0d006b96561bbc36627ac2868bb13407f10768a80e4336f6611a` |
| `apps/desktop/src-tauri/tests/test_native_owner_dialog.py` | same Owner leaf | `bb5f2c9bdc4a9567e6886b94b67de229965b641247cd2f80030c2a5bec1457b1` |

Every composed source file matches its frozen leaf Git object. Other differences are records, archives, requirements evidence and the single bootstrap hash. Production Core/Native/UI, dependency manifests/locks, workflows, bootstrap verifier and original validation algorithms are unchanged from641. Schema13/Factory source was not introduced. The original three Environment names and four Product job names remain exactly the same; the retained branch-rules read still names the original three required Environment contexts. This does not waive the project's requirement to obtain all seven successful checks and relevant product evidence before integration.

Independent final static checks establish:

- The Command test's prefix/helpers/fixtures and every byte after the first case are unchanged. Its first case retains all10 original literal expect assertions verbatim, with8 additional visible/enabled guards (18); full-file expect-call text counts are89→97. The imported independent AST89→97 proof is hash-verified, not newly executed. A new cached-TypeScript AST attempt could not load the absent parser dependency; no dependency was installed and no AST execution is claimed. The literal comparison is limited to the manually inspected semicolon-terminated first-case assertions, with byte equality providing preservation of the rest.
- The seven changed click call sites now activate actual React handlers through DOM events after availability assertions; original policy/IPC/revision/key, no-auto-run, prepared/terminal, owned-reap/unknown-containment, no-scientific/acceptance and failed Task assertions remain. There is no new timeout, retry, fake timer or CI override. The test is explicitly mocked transport lifecycle evidence, not actual Tauri execution or pointer/keyboard/native interaction evidence.
- Python AST source spans for all11 original runner declarations other than the modified `bootstrap` are byte-identical, including NativeCoreClient, stop_owned, expected_message and the complete OS run driver. The entire original11-test OwnerRunnerControls class is byte-identical. Both10-second bootstrap deadlines,19-table observations, fixed RPC/Grant/invoke flow and actual OS approval driver remain. The new private canonical/short path and stdout/stderr capture prepare only owned fixture leaves and preserve strict failure/cleanup refusal; no native production path/authorization guard is relaxed.

The two leaf review reports clearly delimit their subjects and retain failed evidence. Node author26/full564/37/type/build and independent26 are separate executions of overlapping coverage, not additional unique product behavior. Historical Intel5000ms timeout was not reproduced locally; neither root cause, guaranteed remediation nor whole-suite speedup is established. Owner author15 and independent15 cover the same15 cases, not30 unique tests. Original11 already include one actual Core/readonly-SQLite test. The new4 mix two actual Core bootstrap controls, generated own-process exit0/23 refusal, and private/empty evidence-directory refusal. The six separately retained bootstrap controls overlap coverage; they do not add six unique unittests or real macOS approvals.

## Bootstrap, scope and repository checks actually executed here

The manifest has the same93 paths as641. Only `docs/execution/STATUS.md`'s registered hash changes:

`532f2d5edf3f4533e8e02558c626bd7d6cecd1ea123992d920a6444a9d8e0c02` → `ff1cd0199804c7aa0a580053e50b5f05dae340a6039610cc8323efdf8f7fcb8d`.

All other92 current file bytes and registered hashes equal641. Current STATUS bytes match the new hash. No bootstrap file/path was removed and the checker algorithm is unchanged.

Actual independent commands, against the exported source except the read-only Git range check:

| Command | Result |
| --- | --- |
| `python3 scripts/verify_bootstrap_manifest.py` | exit0,93 files |
| `python3 scripts/validate_design.py` | exit0,130 requirements/14 packages/27 answers/7 positive examples/10 negative examples/41 links; design-only |
| `python3 scripts/validate_infrastructure.py` | exit0,2 workflows/15 pinned action references; static-only |
| `git diff --check 641be86e8b055920787cfebdd48270716d509309 9448918d17ba3a1159edf7051e140ea04e04e6df` | exit0 |
| Own `audit.py` | exit0; static/hash/AST-source-span/archive checks, no product execution |

Raw structured command output is in `validation-checks.json`; reproducible composition/import checks and output are `audit.py`, `audit.log`, `audit-results.json`; the limited literal assertion proof is `command-assertion-audit.json`.

The original130 requirement identities and all previous contribution data remain. Exactly nine records append one `PR16_641_exact_CI_partial_dimensions_not_accepted` contribution each: AB-TSK-003/004/007/008/009/010/011/012 and AB-SEC-002. No requirement is deleted, promoted to full completion or narrowed; the new records explicitly retain no-new-package/Owner/Provider/Factory/full-Design limits.

## Imported evidence roundtrip independently verified

For each of these six archives, this reviewer checked archive bytes/SHA against its fixed import record, unique bounded relative paths, exclusively regular files (no symlink/hardlink/directory/device members), exact declared manifest/path mapping and every payload SHA (and size when declared). Data was then written only beneath this review's `imports/`; source was not changed.

| Import | Payload files | Archive members including metadata | Archive SHA256 |
| --- | ---: | ---: | --- |
| CI641 Linux |129|130|`ffe4be49979acb01a11ef3a867efe0217e9e3f92508efcb1447cbcc3058c40fb`|
| CI641 ARM |27|28|`56c0439ed0a04675115046cac1b07ea43bd97c4d4762867c43e61ea219b9f31d`|
| Command author |24|25|`17e558d07a12079b95e688be3921550f30f2108922c2f39f78678382a9c78f82`|
| Command reviewer |19|20|`6aad3190a79a94a9e5098ccc1e877427fd0b8f067c95c22c1694aef65bc23806`|
| Owner author |58|59|`83cd60c9d4efd15ccd452562e3fef8f511b7407d9d9bc469bf2be3033ff57bf4`|
| Owner reviewer |50|51|`2ab4161e1fe86e5584606632605d652312b0a139e4ce4a972bb49a1b2cff01b6`|

Total307 payload files/313 members are retained with six metadata manifests/evidence records. External manifest bytes equal their embedded counterpart where provided. Command-author's24 explicit original→archive path mappings match exactly, with its additional AUTHOR_EVIDENCE metadata bounded by the checked archive SHA. Original failures/audit-script diagnostics and independent reviewer reports are preserved. The three compressed `ci-641` logs and all retained API records also roundtrip to their declared uncompressed SHA/size. No new installer member exists in these imports.

## Accurate CI and platform boundary

Retained exact API/log evidence binds feature641, actual checkout `d03313d5b9b1b575e1eb4a4ae35ec29c49ff446d`, tree `67038b21828745f79905a169abb6e57e98bca5a0`, Product40749637133 and Environment40749637046. The seven retained jobs show five successes (all three Environment plus Rust/UI) and two failed Mac Product jobs. These are historical immutable API records, not a newly fetched live CI status and not checks for9448918.

Linux independent review evidence describes593 unique outer Rust tests +7 compilefail docs, Node564/37, actual UnixIPC/browser114 checks/70PNG (Graph subset34/14) at that exact candidate. ARM independent audit records real readonly WebView/WebCrypto/schema12/lifecycle evidence, followed by actual Owner bootstrap daemon exit1 with empty transcript. Release audit/package steps were skipped. Intel stops at Node563pass/1timeout5000ms; Core/native/package steps were skipped. The raw metadata and logs do not establish the old Owner exit's cause. The109-byte socket probe/87-byte short restore is a separate observed Mac control, not proof of that historical cause. A single native PNG and representative Linux pictures were viewed by the recorded leaf reviewers/Root; this final reviewer checked their files/hashes and reports, not a new visual or OS run.

The fc5/9065 sidecar provenance is explicitly corrected to5541 compile-only assets, not executed70a binaries. Owner author/reviewer execution records bind the actual frozen70a/schema12 Linux daemon SHA `40322bff82aa8ec0f1c019e5d723f42694a6266bcaa27b153f535d0d21dbadcd`. Existing historical manifests are unchanged. No compile asset is promoted into new execution or Mac evidence.

No new exact944 CI, successful Mac Owner NSAlert/full approval acceptance, default release audit, dual-architecture installer pair, signing/notarization, real Provider, Factory13 runtime or full Design completion is established. Those remain necessary follow-up or separately gated work. A normal next candidate push must receive its own exact checks/evidence; existing failed jobs or older packages cannot be borrowed for merge.
