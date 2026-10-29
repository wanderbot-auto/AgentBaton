# Progress

The async Playwright lookup source-only change65f7 was applied as435ac7b. Checkpoint00c26dc preserved the five schema13 wiring files before importing the complete reviewed Factory driver from744bf29, including its prerequisite reaper changes. Source98bc23e freezes these six files; the driver SHA-256 is `4aa84fb34529e6712923e5783e7824d3965adbce7742ca8386b22a9d45264162`, also byte-identical to Root integrationc23ee5.

UI CI now builds baton and batond and records both raw hashes plus actual Git HEAD/tree with schema13. Goal execution, Graph and Data retain their original behavior with strict schema13 preflight/live checks. The native shell additionally calls the existing strict nine-field Factory readonly validator. The three Factory modes each have a distinct output directory; all three are attempted and any child failure makes the aggregate fail. Original 12 flows, upload-always behavior and workflow controls remain unchanged.

The original finite check batch passed77 controls but exited1 because the old Factory driver accepted13.0. That failed batch is preserved. With the reviewed driver,81 finite controls pass, including all original77 and four added strict-type rejection controls. Infrastructure validation reports2 workflows/15 pinned references; bash syntax and cumulative diff checks exit0. Portable evidence round-trips every member byte-for-byte.

These are author checks on wiring and generated fixtures. No actual Core/Factory/browser/macOS/CI run occurred here. Independent review is pending. The separate actual wrong-start-CAS failure under Root/Core remains outside this leaf and is not waived by these results.
