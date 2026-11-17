# Stage2 archived evidence

`report.json` and `command-0.log` through `command-3.log` are byte-for-byte copies of the raw local report and four command logs. Original local paths and digests remain in that report; EVIDENCE.json maps each raw path to this archive.

`inputs-identical.json` is a byte-for-byte copy of both original `inputs-before.json` and `inputs-after.json`, which were equal. It retains all 2,758 tracked input entries. Both original manifest digests and this archive digest equal `a51827cb0e06a88fa11aafb96dfa21e14977f41615cc001012ff4564d1d450eb`. Deduplicating these equal copies changes neither the recorded executions nor their input hashes.

Actual clean execution HEAD was f95db43f78def107aabc4afb403e2927a82915e3, with source equal to reviewed code commit 8b298fde82931928bbd22f532dfe50df1f2921b7. Evidence documentation was edited after the run completed. The ARM64 binaries remain in the task-local target directory; their hashes and marker inspection results are in the report. They are development daemon binaries, not native app release artifacts.

No original hosted timeout is resolved by these diagnostic controls. The original Stage1 pre-commit provenance limitation remains documented. No actual native release application audit or new hosted CI run is included.
