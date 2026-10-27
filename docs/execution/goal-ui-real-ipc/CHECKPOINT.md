# Goal UI real IPC checkpoint

Branch: `codex/goal-planning-ui`; inherited base `4953abe4e9607551d0694016be35b1d52158fac8`. Product source remains `98a38d4cd486afa2a4fe476d0f464f1763d0805a`, previously independently reviewed with 229 frontend tests/types/build and 13 Linux native unit tests. Those prior tests are historical, not new schema9 RPC/native-dialog proof.

Only this new harness and these four docs belong to this unit. The existing untracked `docs/execution/goal-ui/REVIEW.md` is the independent reviewer's document and is not staged or overwritten. No node_modules link was created. No push, PR, merge, Cargo or other-worktree edit was performed.

Current executable evidence is a deliberate **failure**, exit 1, against primary's old schema9 binary. It confirms the real empty-history blocker. No successful original-plan/preview/revision/revoke end-to-end claim is made. The same script must be run unchanged against a supplied repaired binary, with a new output directory. If later fixture problems arise, retain the old script hashes/output and distinguish those from product defects.

Next: primary/backend author fixes and independently reviews historical reads/initial empty history, supplies exact frozen binary/hash; run this harness, inspect screenshots and actual read-only DB invariants, resolve genuine failures without weakening assertions, then freeze evidence for independent review. Native OwnerDialog/WebView, macOS and dispatch remain separate validation dimensions.
