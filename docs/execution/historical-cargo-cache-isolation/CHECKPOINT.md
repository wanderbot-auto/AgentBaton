# Checkpoint

Branch codex/historical-cargo-cache-isolation; isolated worktree /workspace/.worktrees/historical-cargo-cache-isolation; fixed baseline 82773a3b78e9fbf0a5224b054c64d06ef90037f9. Source ready for independent review and Root integration. Only product.yml, test_schema17_migration_workflow.py and these four unit records changed.

Actual source validation:9 workflow pure tests +6 runtime-policy pure tests passed (15 unique, separate suites), YAML parsed, git diff --check passed. Test repeats during evidence collection are not extra unique tests. Full inverse-normalization baseline product SHA256 is4b7543d61e8846248e1b7474ff6fe15d64068612fadbd44f6fe7585933a3378d; only six exact historical target argv differences are allowed. Future workflow changes require an explicit reviewed baseline update.

Next: Root independently review/cherry-pick; retain its in-progress genuine10 result without duplicate runs; actually run exact-commit CI and measure cold historical rebuild time/storage on both Mac architectures. No timeout expansion or acceptance reduction is authorized by this source change.
