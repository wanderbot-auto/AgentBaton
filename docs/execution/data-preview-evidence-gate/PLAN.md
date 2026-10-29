# Data preview evidence gate

Base `3ae9160ac88f5e147c2478900599db3ccc356376`, schema15; branch `codex/data-preview-evidence-gate`. This is a bounded test-evidence adapter for the PR18 preview-cancel blocker, not workspace creation, schema16/WSI3 or new product permission. Keep Design v0.2 / AB-START-001 / OWNER-001/002 and all130 requirements unchanged.

Requirement links: AB-TSK-007 (request versus confirmed stop), AB-TSK-008 (unknown/recovery), AB-VAL-004/006 (data/path evidence), AB-SEC-006/010 (failure and source/data policy). The requirement baseline contains no AB-DAT IDs; “Data” here describes the existing preview domain and does not invent requirement IDs.

The old actual driver still finished its preview without a cancel hit (exit2/eight checks). Do not retry for luck, mislabel verified as cancelled, relax32MiB/64MiB/four-files/30s, or substitute a fake snapshot/stop.

1. Read actual worker/control/finish contract and agree exact private config/ready fields with Desktop.
2. Add non-default debug-only Core feature: one immutable private startup selector and one worker-only pre-scan delay; original cancel/deadline and join/receipt paths remain authoritative.
3. Add feature-only normal/negative/worker source tests and extend existing release audit's exclusive markers. Run only source/format and inert audit controls under this lease.
4. Freeze source/evidence for independent review. Root owns serial feature/default/release compilation, combined real driver run, current-source manifests, exact CI and both Mac observations.

## ADR: deterministic delay at the real worker boundary

Use `ApprovedDataSelection::contract_view` after original source identity validation and before `verify_dataset_controlled`; no workspaces cfg(test) hooks, public IPC, generic test server, Owner/Agent enable flag, scanner rewrite or Actor wait. The exact descriptor is only a match restriction; its source paths are never opened. Matching delays once until the existing cancel or original timeout; no external release or success override. Therefore the only claim is cooperative **pre-scan** cancellation, not mid-read cancellation or OS interruptibility.

Feature-off compiles none of the module/config/marker code; feature+release compile_error prevents accidental inclusion. Existing script `scripts/native-audit-release.sh` is extended instead of creating a parallel .py audit platform. Same-user hostile mutation and blocking kernel I/O remain outside this engineering fixture guarantee.
