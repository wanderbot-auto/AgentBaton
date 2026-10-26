# Native CI failure follow-up

Scope: AB-REL-001/002/003/008 and the narrow window-close lifecycle evidence for AB-SES-008. AB-START-001; OWNER-001/002 remain acknowledged by the integration owner. This is a test-harness correction, not native/full-product acceptance.

Actual Product run40401378109 at f3eb83ebf9022544730e8ad6be578f5bfaf3efa1, ARM job121847819111, compiled the native smoke binary but failed before a native report/window result. Downloaded artifact12689352960 and verified its GitHub ZIP digest **f4324ed37e6981d55fce03033d35285fb45a8ce84aa748d67e13f0f39102294d**. It contains only lane.log and native-run.log. The latter says `owned daemon exited before authentication`; the former shows PermissionError EPERM during external-runner group cleanup. It contains no permission diagnostic or native-smoke.json. Do not claim that artifact proves the historical startup cause or any WebView check.

A controlled test under umask022 invokes real core `Daemon::bind` using the original native fixture Builder behavior. It fails101 with the production private-runtime PERMISSION_DENIED error. Explicit0700 creation fixes the same regression, which passes. No existing directory is chmodded and production admission stays intact. The native harness now also preserves the exited daemon status in its startup diagnostic.

The external runner retains the original child unreaped through all group signals. An EPERM is treated as quiescent only after a bounded query of that original private group proves the original zombie leader is present and every returned member is a zombie; no discovered PID is a signal target. Live, missing/unknown leader, malformed proof and other permission errors fail closed. Observation/cleanup errors still reap the original child where permitted, produce failed structured evidence and cannot accept a passing native report. The external timeout80s and native watchdog60s stay unchanged; cleanup is bounded to3s and screenshot capture remains3s.

Actual Linux checks: **15 Python runner/audit tests** (all original10 retained, five additional contracts), **one native-bin fixture/real daemon-admission test under022**, native feature/all-target Clippy, native format check, Python compile and scoped diff check pass. The EPERM test injects the syscall result on a real owned Linux zombie group; it is a synthetic Darwin failure contract and does not prove Darwin execution. No new skip, wider timeout, relaxed acceptance assertion or system sandbox bypass was added. The first exact filter mistakenly selected zero tests; that log is preserved separately and not counted. An initial format failure is also retained.

Current evidence: `artifacts/native-ci-fix/evidence.json` and downloaded/verified artifact contents under `artifacts/native-ci-fix/arm-f3`. The evidence JSON contains actual local source/log fingerprints. New macOS ARM/Intel CI, native screenshots/dialogs, release bundle audit and manual installation still require their own actual results. No Provider/model request, private auth/key, production action, workflow/lock/core/UI change, staging, commit or push was performed by this worker.

| Frozen source | SHA-256 |
| --- | --- |
| `scripts/native-run.py` | `b1aa421c9b98f3efe449a64b317566e131385f26f65b4c07dca01203c6f0d46e` |
| `apps/desktop/src-tauri/src/bin/native_smoke.rs` | `4952d4cdf7cb78909264092a45c46c7035f2c2f0ea3e2fcef5936e0934ec2b2c` |
| `apps/desktop/src-tauri/tests/test_native_runner.py` | `4e8165bfd0df98a5db63d5ce19ed429625773e2e93fba35ad07c272b43d63a3b` |
