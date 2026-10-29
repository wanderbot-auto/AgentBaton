# PR16 a2201 — independent Linux CI failure evidence

Verdict: **UI FAILED; Rust SUCCESS.** The new UI result is a real failure, not superseded by prior bb success. This review only diagnoses and archives the observed run; it does not approve a fix, rerun products, or establish new macOS/package results.

Subject head `a2201fe2729eddb0d76bf3995667d3692126ee0f`; Product run **40806944290**. Both independently downloaded UI/Rust job logs explicitly record checkout **7f359dc8807e4d11f8e2dedbf4b0a5cc83c81f16** after `git log -1 --format=%H`. The commit API confirms tree **520d7aab6abdd4d659d0a9d301d9e28bdfffafd2**, parents `[a1be94d26d10bb245bb35eb50226b82012c967da, a2201fe2729eddb0d76bf3995667d3692126ee0f]`. This checkout was read from original logs, not guessed from the PR head or copied from older runs.

## Exact failure

UI job **123228044786**, step 8 **Actual frontend with real Unix IPC and responsive screenshots**, failed at 02:36:34 UTC. The command was the existing browser-venv Python running `apps/desktop/tests/goal-graph-real-ipc.py` with actual `target/debug/batond`, built `apps/desktop/dist`, `goal-execution-candidate.json`, cached Chromium and **`--validation-case command-timeout`**. The child returned **exit 1**; the surrounding `subprocess.run(..., check=True)` raised CalledProcessError and the step exited 1. The always-upload step succeeded.

The retained failed report is `ui-artifact/goal-graph-command-timeout/report.json`. Primary AssertionError is **`Complete bounded private argv required`**, at `owned_command_child`, line **854**:

```python
assert len(raw) <= 1024 and raw.endswith(b'\0'), 'Complete bounded private argv required'
```

It was called by line **1512**, the actual owned-command observer with its original **5-second** limit. This failure occurs before validating the expected timeout receipt. The exact checkout source came from the GitHub contents API and hashes to **4bc91fde76a61c4fdb829f3844f06cca20f94238add92d5b60bedb0edd00adc5**, matching the report. No equivalence assumption with bb was needed.

The last sample records the observer error but **does not save the failing PID, raw length, trailing-NUL flag or pre/post identities**. Therefore the record cannot distinguish an empty argv, oversized/incomplete bytes, or a lifecycle/exec race. Earlier samples used actual `task_children`, not the historical Cloud missing-task-children fallback, and recorded only generated ACP PID 10557. These samples do not prove it was the failing PID.

The separate failure-time Graph query is revision/ledger **82**: producer is candidate with confirmed stop, the validator is **running**, validations are empty, and Owner/Summary nodes are not_started. It does not contain a final deadline_exceeded receipt, worker-joined result or completed negative-path budget proof. Those intended assertions were **not reached**.

Cleanup subsequently calls the observer again if no original command identity was saved (source lines 1757–1759), and identifies actual `/usr/bin/sleep 3` PID **10581**, start ticks **26767**, parent **10315**, PGID **10581**. Its original identity was later gone; no cleanup signal was sent to it. The owned daemon exits 0 without forced SIGKILL and HTTP thread/handlers join. This later cleanup identity cannot be backfilled as the original failing PID or evidence that the original deadline assertion passed. One later generated marker is explicitly unmatched to a retained real task request. The report correctly stays failed.

**Minimal reviewable next step:** add bounded test-observer diagnostics at the existing failure guard: already-owned parent/PID and before/after identity, bytes read, trailing-NUL boolean and bounded-byte digest, without arbitrary argv/credential/environment dumps. Keep the complete-argv/identity/PGID checks, no acceptance of malformed/unknown processes, and the current product/observer limits. If later evidence proves a transient pre-exec state, a separate narrow test-only handling change can be reviewed with exact own-process negative controls. This evidence alone does not justify increasing TTL/runtime, skipping the timeout test, accepting zombies/partial argv, or changing production Core. The precise underlying cause and whether any production defect exists remain **unproven**.

## New-run results and counts

- Node typecheck, test and build step succeeded: **564 tests /37 files**. The actual browser failure is separate from those checks.
- Eleven complete UI reports passed **109 checks /69 PNGs**. The failed Command timeout report retains **2 passed preliminary checks /1 failure PNG**. There are twelve reports, **111 recorded checks /70 PNGs total**, **not 114 successful checks** and not a full UI pass. The five preceding Graph flows passed 6/6/6/6/5 checks; the sixth failed after 2.
- Rust job **123228044793** was already completed/success at its first API read. Its exact log records **593 unique non-doc tests +7 doctests**. Two one-test nested filtered runs overlap full suites and are not added again. Core lib69, command14 and sessions55 are successful in this new run. Format, workspace Clippy `-D warnings`, actual build/local product smoke, official native-wire contract audit, native runner Python **29**, and Owner runner Python **29** all have successful steps/logs. These Python contracts are not macOS GUI evidence.
- Provider artifact contains 9 generated-child wire frames; independent validation actually checked **8 schema-bearing messages** against pinned official schema SHA **f50e158c34ac55c5cc67c5493df539f1c07b603bd5dd2100a7393eefb62a3e20**. This is a generated native contract, **not live official Provider/model compatibility**.
- This schema12 workflow did not wire Factory cases. Factory local runs or a future Factory branch are not counted as this CI's results. The failed timeout branch's final validation/budget/Owner nonstart assertions were not reached, even though earlier bounded observations exist.

## Acquisition and integrity

Only the new two Linux artifacts and two job logs were downloaded, each **once**. Disk was checked first (274 MiB free); no unknown files were deleted. Sizes were inspected before extraction; no Cargo/cache or desktop artifact was touched.

| Artifact | ID | ZIP bytes | SHA-256 / API digest |
|---|---:|---:|---|
| frontend-real-ipc-7f359dc… | 12869160663 | 9129490 | `4a1a0e67e2c2602c48bc750a078a5b1f70ea4515a77ce4508263cfa04fd3178e` |
| provider-contract-7f359dc… | 12868443501 | 74556 | `e19b7ccec86408376b9ce8ed91ff8fcf537835a92c062245b80ba4bad8e1d075` |

Both exact downloaded SHA/size values match API metadata. All ZIP CRCs pass; every member is bounded, unique, relative and has no traversal/backslash/drive separator or symlink. UI has 95 members /18954307 expanded bytes; Provider has 2 /739873. Every extracted file is SHA-inventoried, including all 70 PNGs. Original ZIPs, metadata and logs are retained. UI raw log SHA **67b87cebcb74512158f67a0bb2ae10df43480d0d9afc4f1d7a9687a3fda47281**, 75828 bytes. Rust raw log SHA and all other exact bytes are in MANIFEST.json.

The failure screenshot SHA **164b0bb224d4150839e0dc139d0b2d37d56568670f18b6a57640fadd1605e1cd** was genuinely viewed with view_image plus image forwarding. It shows a nonblank AgentBaton page, approved planning scope/revised plan and Goal execution section; it is not proof of a final blocked timeout result. The other 69 screenshots were hashed, not visually inspected.

`extract.py`, `audit.py`, `validate-native-schema.py` were actually executed with bytecode disabled and returned 0; raw output is retained. The audit verifies candidate/head/tree/daemon/hash bindings and each report's recorded status/count, not a fresh semantic replay of every successful flow. The first attempted local Git source export could not resolve the path in this environment's retained commit snapshot; the exact source was then read via GitHub API and SHA-checked. ACQUISITION_NOTES.json records that observation accurately, without inventing a separately captured raw shell log.

No browser/daemon/Cargo reexecution, source/STATUS/lock edit, remote/CI mutation, formal Approve, new subagent, account, key or fee occurred. Desktop/macOS, Environment results, packages, previous bb green and future fixes remain separate evidence. New Linux UI failure remains an integration blocker until an authorized repair and new exact verification close it.
