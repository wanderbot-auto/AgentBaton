# Narrow independent review — schema12 legacy harness adaptation

No blocking finding for the **three schema-only changes** observed at Root HEAD `dd0d9d2ca4abb47513987db9f5f96e1156501012` in `/workspace/.worktrees/goal-graph-integration`. This review does not accept the unfinished Graph implementation or authorize a merge/release.

Independent inverse-byte comparison confirms exactly:

- `.github/workflows/product.yml`: candidate metadata schema11→12, one numeric replacement. Every other workflow byte is identical to HEAD.
- `data-workspace-real-ipc.py`: two description strings and the exact manifest/SQLite user_version assertions11→12.
- `goal-execution-real-ipc.py`: exact manifest/SQLite user_version assertions and its refusal message11→12.

All former flow code, assertions, limits, process controls, private16KiB Owner review, stop-source/order/value bindings, HTTP denial, request bounds and evidence behavior remain byte-identical after reversing only those explicit replacements. The guards require exactly12; no11|12 compatibility alternative was introduced. The Environment preflight workflow and execution gate are byte-identical to HEAD. Remote branch-protection settings were not queried or changed.

Independent Python AST/py_compile and `git diff --check` pass. Compiled Python artifacts were written only into this reviewer directory. No Cargo, target cache or source write was performed.

Both current harnesses were independently executed through their real preflight using the immutable schema11 manifest and daemon SHA-256 `d1099e7ee128d4cb5ea16668b7f5fbde83d482c4c6b11616fecf8775ea7e5f48`. Both raised AssertionError at the strict `manifest['schema'] == 12` assertion. An audit hook observed **zero subprocess/process launches** and would have stopped any attempted launch. The original binary hash is unchanged afterward. Existing Root refusal JSON/raw logs were also read and matched their recorded script and trace hashes. Each independent traceback is retained beside verification.json. The frontend path was used only to satisfy preflight existence/read checks; no browser, daemon, HTTP host or product scenario ran.

Exact reviewed source SHA-256:

| File | SHA-256 |
| --- | --- |
| Product workflow | `ef238f4a2e78ead6c20414b348825e129083903d2b4bec012524ba0cb1580363` |
| Data harness | `75108a947b237db13e47495d0ae2e0b176217361e7d4b37b9ceedafbed83e493` |
| Goal execution harness | `0ce135bd4ca07f5e4aca117bf1d57c9754da44eca56fd1add158597d0bdafe0f` |

All three hashes were rechecked after verification. The evidence proves source-preserving strict schema12 adaptation and actual rejection of the old coherent schema11 candidate. It does **not** establish schema12 browser success, re-execution of the legacy80 checks, new Graph validation/acceptance, CI success, macOS/native dialog or real Provider behavior. Those positive checks remain bound to a future final frozen candidate and actual execution. Old EARLY/WIP graph review records were untouched.
