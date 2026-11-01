# Progress

- Preserved the clean integration-runtime-tests tree/branch. Created a separate tree at the fixed b452 base; current permissions/gate and OWNER-001/002 remain unchanged.
- Confirmed the Integration Native test-control kind in fixed controller source and the missing literal in the shell regex. Read immutable requirement wording: REL003 is the direct mapping, REL001 governs evidence attribution.
- Ran the real unmodified shell with the new canaries before fixing it:8 tests,7PASS1FAIL in0.113s; the new Integration marker was wrongly accepted with exit0. The original raw output, source hashes and command are retained in EVIDENCE.json. All old marker and fail-closed controls passed in that run.
- Source8902da6 adds only the omitted regex alternative and one new test module. Removing exactly that new alternative makes the entire shell byte-identical to b452; no old marker or branch was changed.
- Ran the same8 test methods against fixed shell:8PASS in0.141s. The old-marker test contains11 independent subcases. Legitimate production integration names pass. strings exit37/38, grep exit23, missing strings and missing input all reject without PASS. Every existing byte fixture remained0600, single-link and unchanged, and the script's private inspection files were removed.
- `bash -n`, Python compile() without bytecode output, and git diff --check passed. No actual Mach-O/release executable, Cargo, Node, Actor, CI or Mac execution occurred. These are script canaries and controlled tool-failure tests only.
