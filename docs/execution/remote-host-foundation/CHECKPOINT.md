# Checkpoint

status=review_preparing_commit
base/head-before-commit=b78ffbb5bcbe741de2e0cd8fcf2e568c4c7ef28c
branch=collab/remote-host-foundation; target=main
Dirty paths: crates/remote-host/**, scripts/verify-remote-host.py, tests/infrastructure/test_remote_host.py, docs/execution/remote-host-foundation/** (new task files only).
Complete: ADR, production-default bounded stdio companion and strict SSH adapter; local trusted provisioning; persistent journal/receipts/budget/fencing; worker filesystem effects; offline cancel/event ledger; truthful wait/unknown recovery; read-only independent review fixes.
Verification: latest EVIDENCE.json links 16 Rust +16 real subprocess tests, fmt, locked build, strict clippy; all passed. Failed runs retained. No surviving owned companion groups per suite.
Next: commit implementation/evidence, fetch/preview main, bind verification to submitted implementation commit, normal push and draft PR; leader handles global state and acceptance.
Unverified: real SSH handshake/changed-key rejection; Linux and macOS↔Linux remote runs; Provider; Core/UI/shared root-budget and retention integration; production signing/release.
leader_actions: review exact submitted source/PR; integrate via dedicated follow-ups and existing required checks. B does not merge main or write global status.
