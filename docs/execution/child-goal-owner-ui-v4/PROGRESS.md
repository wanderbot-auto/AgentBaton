# Progress

Implemented the isolated v4 extension, read-only DOM attributes, two non-waiting endpoint reads and strict finite projection. Preserved the old source through an exact full-file inverse. Added 14 pure v4 controls and five mounted controls.

Actual author validation: 53 pure Python tests PASS in 3.638s (existing finite metadata, v3 metadata, bounded readonly wait, and new v4 controls); `git diff --check` PASS. The new v4 controls alone passed 14/14 in 0.113s before the combined run. Python source was compiled in memory during test loading; no .pyc was written.

Mounted tests, TypeScript checking/build, browser JavaScript execution, actual Core, SQL, Native, macOS and CI were NOT RUN by this author. The retained old CI failure is not fixed or reclassified by this observation change. Independent review and Root execution are required before acceptance.
