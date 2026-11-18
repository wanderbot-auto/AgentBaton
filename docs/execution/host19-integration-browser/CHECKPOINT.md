# Checkpoint

- Status is review. Branch `collab/host19-integration-browser`. Fixed base
  `cde501db847d1bbfc27a298e16cd772afc1a41e9`.
- Tested source `86c7cd20a7528d5b7e3dabcfd9738a64dd60275c`. The following local commit adds evidence only.
  No tracked files were dirty during testing. Only this evidence directory was
  pending at archive time.
- Scope passes. The latest fetched integration target at preview is
  `cde501db847d1bbfc27a298e16cd772afc1a41e9`. The source-only merge preview tree
  `2aaa8f6be782f9c1d4114e26363dee88af44f41d` has no Git conflict.
- Same-tools private IPC and real SQLite baseline and candidate recorded.
  Candidate schema 19 passes. Versions 18 and 20 refuse. Raw database and WAL
  hashes and the daemon sequence are unchanged by the reads.
- Focus 41/41, full infrastructure 927 passed with 16 existing skips. The
  actual browser entry returned exit 2 at `candidate_schema_guard`.
  The original outer CI function refused the actual daemon schema 19.
- All owned processes were joined. Actual browser UI, native App and CI are
  unverified by this task. The schema18 outer contract, manifest/report labeling
  and projection stay in the separate integration stage.
- Leader review pending. The in-app task messaging transport failed repeatedly.
  Claims remain active until a documented handoff.
