# Same-refresh immutable plan reuse

Base: `c67569856050fd40195b540b4bb71d3406e0e907`.

The exact c675 Product CI run41188874038 failed the original normal-case Owner5s expectation at driver line1732. The retained official finite summary has SHA256 `8dd4a2794b180f39d01e4e1c184a8ce94da0dd368245694e3b71263c114b91d5`. Its end renderer sample reports an enabled Owner control, but does not establish when it became enabled. The cause of that failed expectation remains unproved. The original failed report, cleanup=false and unknown timing remain failed evidence.

`refresh` first obtains and validates the current saved planning head against its local context/scope/task, including raw SHA-256. After the exact Goal read, it reads the immutable initial plan again; it can also read an adopted plan already equal to that head. These serial duplicate Core reads can be removed without treating a summary as raw evidence.

Within that one refresh invocation only, a complete `childGoalRefSame(head.reference,wantedReference)` match selects the already loaded head DTO. This uses all immutable reference fields, including scope/run/task, id/revision, plan digest, raw SHA and receipt sequence. Different references use the original API request. Existing initial==adopted reuse remains. No DTO is stored as a reusable cross-refresh cache.

Both selected DTOs still pass the original shape/reference/raw verification loop and `childGoalSavedReferencesMatch`. The original context/floor/origin/graph/epoch/pending/authority/Owner gates and all write handlers remain. There is no fallback to a summary, no extra retry, no changed deadline or poll budget, and no Python harness change. Repeated raw checks also reject mutation of a head object during the intervening Goal await.

Three existing v4 delayed-initial tests now use head r2 and initial r1. Two existing polling partial-evidence tests use a different head and historical references so the expected late Core await remains present. Their previous assertions remain byte exact. The new mounted file covers both reuse branches, every-refresh reads, full-reference mismatch, raw/shape rejection before and after the Goal await, foreign binding rejection, and a superseded epoch. All mounted controls and type/build/runtime checks are NOT_RUN by this author.

This is a bounded read deduplication optimization. It does not establish the old CI root cause or prove a future CI/browser pass. Root owns actual Node/type checks and any fresh runtime verification.

The inherited Python v4 full-source inverse still pins the earlier UI body. Its actual pure run retained13PASS/1 source-boundary FAIL with zero errors; it must acquire a separately reviewed outer reuse inverse before full infrastructure acceptance. Python source is outside this unit and is unchanged. The new SOURCE_INVERSE.json restores the complete c675 files and rejects unrelated/guard changes.
