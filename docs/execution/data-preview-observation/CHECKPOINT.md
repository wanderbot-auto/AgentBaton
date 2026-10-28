# Checkpoint

Branch `codex/data-preview-observation`; source `0db994786b3e70e21b7c0c57a15cf210fd9e9fdf` on baseline `de150679f7af9915f00cae58e0686a3954041dee`; test source SHA-256 `17178a8c943fa0c5225bb4fd543a8ef50281808458da031b59fe5ea5c54043dd`. Unit docs committed separately. Local author checks complete, independent review / fresh PR14 macOS CI pending. No remote action by this child.

Evidence: `/workspace/.onboarding/data-preview-observation/verification-manifest.json`, SHA-256 `e7fa39c1465ab632a4fad2fdd8aedf2d4e50f0087473415dfc4040961e6a414c`. It binds preserved original ARM failure/source and old/new binaries, complete local logs, command/results and mechanical unchanged-byte proof. Manifest is local author evidence, not independent review.

The parent authorized native target `/workspace/.worktrees/validation-integration/apps/desktop/src-tauri/target` was exclusively used after checking no writer; it is now RELEASED for parent/reviewer allocation. Main Core and private Provider targets were not used. Do not reuse the native target without a new ownership handoff. Core lib entrypoint mtime was refreshed without byte changes to avoid stale Core10 metadata.

Next: independent reviewer verifies frozen source and reproduces scoped checks using a parent-allocated target; parent cherry-picks source/docs, runs exact CI and reads the actual outcomes. If preview still exceeds the unchanged 30-second product deadline or exposes a worker fault, retain failure and escalate to parent for a separately scoped production fix. Do not broaden all waits or waive cancellation assertions.
