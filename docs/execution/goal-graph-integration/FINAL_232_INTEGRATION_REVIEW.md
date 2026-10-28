# PR16 bounded native-text diagnostic composition review — fixed232

Subject `232b53a832b247964510b8660a7231d167fa9c90`; baseline `0aa20c968d493cad895e4a3853bdf106d6b54acf`.

**No source or evidence-integrity blocker found in this limited final composition review.** One already-identified metadata correction remains: `EVIDENCE.current_candidate.Root_final_record_failure_and_correction.final_check` still says pending. Root will replace only that value when importing this review with the accurate range-check exit0 already executed by Root and this reviewer. This correction must preserve the initial failed staged check,15e4 history and raw bytes; it requires no source/STATUS/bootstrap change or product retest.

The purpose of the candidate is to obtain bounded actual Mac failure diagnostics, not to claim a fix or known root cause. This report is not formal Approve, permission to merge failed checks, Mac/installer acceptance or full-product completion.

## Actual independent review and commands

The fixed Git tree was exported into this review's `source/`; all1175 source files still match `manifest.json` after review. Root HEAD232 and clean worktree were reconfirmed read-only. No source/STATUS/target write, Cargo, daemon, product test, new dependency installation, remote/CI/PR action, credential/model call, recursive agent or formal approval occurred. Existing product/leaf tests below were inspected, not rerun.

| Command actually executed | Result |
| --- | --- |
| `python3 scripts/verify_bootstrap_manifest.py` against export | exit0,93 paths |
| `python3 scripts/validate_design.py` against export | exit0,130 requirements/27 answers/14 packages/41 links |
| `python3 scripts/validate_infrastructure.py` against export | exit0,2 workflows/15 pinned action references, static only |
| `git diff --check 0aa20c968d493cad895e4a3853bdf106d6b54acf 232b53a832b247964510b8660a7231d167fa9c90` | exit0 |
| Own `audit.py` | exit0, fixed-source/lock/bootstrap/archive/hash/record comparisons only |
| Own offline formatter/WebCrypto comparison | passed against original retained bytes; no OS/process execution |

Exact structured command results: `validation-checks.json`. Reproducible composition/import checks: `audit.py`, `audit.log`, `audit-results.json`. Additional byte-level actual fixture checks: `offline-owner-checks.json`. Initial tool-wrapper syntax and archive-manifest shape assumptions failed during inspection. Root also caught that the first command-evidence writer had persisted only scope metadata; this was corrected by directly recording four fresh narrow static subprocess invocations with command/cwd/exit/raw stdout/stderr. These were review-tool errors, not product checks or modified evidence. The corrected validation-checks.json has four actual exit0 records.

## Source, limits and bootstrap

Exactly four non-document/bootstrap paths differ from0aa, and all four match frozen diagnostic source `7a13cddef4316d520361071ff5e2a263ea058580` byte-for-byte:

| Path | SHA256 |
| --- | --- |
| `apps/desktop/src-tauri/src/bin/native_owner_dialog/controller.rs` | `4deff52ca01c91f10958ee0ef833f1aab09d0ccaf52c349e03e67fd2a87d2c5e` |
| `apps/desktop/src-tauri/src/bin/native_owner_dialog/macos.rs` | `bb78a3201ed730a6eb87983fc0a91ae1e93081ad4de103949a1b0faa41018f39` |
| `apps/desktop/src-tauri/Cargo.toml` | `f0ad32ea0b400d4a7e0edb8655c0102c38469d512d7d3cc14e6cc6a74ac1bb49` |
| `apps/desktop/src-tauri/Cargo.lock` | `4fd256434d2bb3c0903919bc6474f8a3712be2a27eab1651d42e63f7b876a93c` |

The first two are test-driver code. Native sha2 is optional and activated only by native-smoke. Native lock package/version/checksum set is identical: its root desktop package dependency list adds the already-locked sha2 name only. Repository-root Cargo.lock is byte-identical. Production Core/Native/UI, default features/permissions, both Python runners, workflows, validators, original three Environment/four Product names and acceptance thresholds are unchanged. No Factory13 source is introduced.

Independent final comparison confirms controller.validate and all four original controller tests remain verbatim; the complete four-file leaf subject is unchanged from its peer review. Peer report SHA `3c465e82308236f771d47a3fb207a615438bd6fd9f168b7687d0c9510d1596f6` and original manifest SHA `9d9bc3ed93169c27144397b36efc7b4fbbc042e55e3ea03a593f8b5760a88472` match the imported files and external copies. Its16 mechanical preservation assertions, installed-binding evidence and clear uncompiled-AppKit limit remain separately attributed. Diagnostic bounds add refusal, never partial text matching or click authority. Parent/caller/visibility, exact one full title/message,500ms stability,4s owned capture,75s watchdog,100ms polling, Python85s outer limit and DBpermit→real button click gates are preserved. Failure paths retain the first observation reason and use the one owned/joined capture worker; no screenshot or diagnostic JSON grants approval.

BOOTSTRAP_MANIFEST retains the same93 registered paths. Only STATUS changes hash from `ff1cd0199804c7aa0a580053e50b5f05dae340a6039610cc8323efdf8f7fcb8d` to `4ac42f0fce63ed9f8b797893665bdb7a2f2144ee8777a9d68434cd672ed6fb32`. All other92 actual bytes and registered hashes match0aa, and the checker algorithm is unchanged.

Exactly nine existing requirements append one explicitly partial0aa evidence contribution each (AB-TSK-003/004/007/008/009/010/011/012 and AB-SEC-002). All130 IDs, prior states and contributions remain intact. Previous current-candidate records were moved unchanged into a labelled historical stage, and the641 evidence object remains unchanged. Full Design scope is not reduced or declared complete.

## Five portable archives and raw output

Archive SHA/size, every original manifest declaration/path mapping, every payload SHA/size, unique relative safe paths and regular member types were independently verified before copying only into this review's `imports/`. There are no symlink, hardlink, directory, device or traversal members.

| Archive | Payload union | Members including original metadata | SHA256 |
| --- | ---: | ---: | --- |
| Initial0aa evidence |16|17|`9640962fea7c63f926b1937581c08b5de339823ac54e8b30cfec52d3c9b4dafd`|
| Linux0aa evidence |148|149|`beb3a30a4cb22ad83f2dc0e6a2eb2b0ae4083da94478fb9ada828d7be6fa0f59`|
| Dual-Mac union |71|74|`42c4ed7366d43358ed3ca664dce4b28c9ad9ae5781315c7c9f71ab6d3089bd33`|
| Diagnostic author source-stage |22|23|`a2eb58da89b3da9c6ca6d82984668185f5db5e1fbbcb9045d37c6ca812d8ec87`|
| Diagnostic independent review |17|18|`5708e1abe36c21bf9632a13f0c3c634438252c7f96c5942a87524175b9d85c90`|

Mac ARM44/Intel28/summary7 manifest entries overlap; their union plus3 manifests is74 members, not a sum of independent payload counts. All external original manifest copies equal their embedded archive bytes. Original failures/logs/observations remain; this storage change does not replace or rewrite them.

Root's original Controller log raw SHA is `05a3613d05059b8c549c781e19900c067b2620cb562862f9053658d2250d8adf`,1525 bytes. `NATIVE_TEXT_ROOT_TESTS.log.gz` SHA `9b829a10cdaed5241276315579539a3d326e5fe4ac75f9478ef567c2d619be70` decompresses **byte-identically** to both the raw log in historical15e4a7ee5b5f291dceb943588bdc9241ac1f7457 and the independent-review archive's raw controller-tests.log. The original trailing blank line is preserved, not trimmed. The initial staged diffcheck exit2 and the newline-batch commit mistake are explicitly recorded. Final cumulative0aa→232 diffcheck passes with no checker weakened. A copy of the decompressed original is retained here as `controller-original.log`.

The raw log records Root's actual6 pure Controller tests (original4+new2),0 failures/ignored,26.17s compile, for exact source before/after hashes matching the four-file composition. This final reviewer did not rerun them. AppKit cfg code was not compiled on Linux and GUI was not executed. Root before/after records accurately identify borrowed old5541 fc5/906 sidecars and previous dist as compile-only resources, not70a/current runtime proof. All three exact temporary link paths are now absent/non-symlinks, independently checked without deleting anything.

## Exact0aa CI and unresolved Mac observation

The retained actual job records bind feature `0aa20c968d493cad895e4a3853bdf106d6b54acf`, checkout `5caecb5f9945b5fdb548f0e071a78e2d97537403`, tree `1943596e0bd12177d852117dab86c81ca30e6858`, Product40761824261 and Environment40761824141: five successes (Environment3+Rust+UI), two failed Mac Owner jobs. These are saved records, not newly polled status and not checks for232.

Both actual Mac native-smoke reports bind that checkout, architecture aarch64/x86_64, statuspassed and the original readonly/lifecycle dimension. This reviewer additionally checked each of the four real WebCrypto records per architecture: original raw UTF-8 string109 bytes, SHA `d29817e7338bcd0db0c7f22a98e3faa82802bb73f11d67ad2c65e789d1dd2892`, source window.crypto.subtle.digest. None of this makes the later Owner step successful.

Both actual owner-dialog.json files fail `Exact full native title/message not uniquely present` in initial cancel observation before controller eligibility, DB permit or performClick. Independent offline reconstruction from the unchanged production formatter equals each saved expected message exactly: ARM3132 bytes/SHA `c78bd560b64451be46a7df94653a5d72ffea77675c895b54a9f6d3293e3ca17c`, Intel3136 bytes/SHA `af80eba01bebcc28e39ebc77b6d946813db5ab7e98be9acf3fc95607b337e2c2`. Actual failing OS field strings/counts were not retained; matching error text is not proof of the same cause, clipping, wrong view type or a production defect.

The actual `before.json` files independently contain schema12/sequence8/grants0/exact19 zero counters. No Owner after.json exists, so no after-failure zero/unchanged-DB claim is made. The retained PNG on each architecture is the successful readonly task page, not a failed Owner-sheet capture. Both release build/audit/package stages were skipped; there are no current new development packages. New diagnostic Mac compilation, actual captured text/counts/geometry and Owner Cancel+OK acceptance are still not_run for232.

The next ordinary candidate can collect those facts only through its own exact checks. No threshold reduction, unverified root-cause fix, hidden failure, old green/package borrowing, Factory13 acceptance or full-product completion is authorized or established here.
