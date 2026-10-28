# Actual progress

Source frozen at `0db994786b3e70e21b7c0c57a15cf210fd9e9fdf` (SHA-256 `17178a8c943fa0c5225bb4fd543a8ef50281808458da031b59fe5ea5c54043dd`). Only the assigned test function changed. Static reverse comparison proves the other 20 tests, Fixture.wait and all bytes outside the function unchanged. Removing the added cancel-stage timer/log restores the complete original verified/Owner/copy/cancel/Ready-race/receipt/source-size assertion tail byte for byte. Production worker, IPC, manifests, locks and CI remain identical.

Original exact local case passed in 2.45 seconds. Historical ARM log is preserved, SHA-256 `3ab69d42611f550cb14fb2a5a4b7c3a1634771b694852385c0b3f285d594df72`. It proves only a 5-second observer failure while checking. Hash slowness, scheduling, product timeout and other root causes remain unproven.

Actual new suite: 21 unique Data tests plus one nested process execution, 3 unique migration tests, zero failures/ignored. Start 11 ms; concurrently requested system.snapshot and preview.get each 5 ms, preview still checking; terminal verified at 2196 ms; copy cancellation reached cancelled/worker_joined at 39 ms. Core all-targets Clippy, package format and diff checks passed. Exact commands and logs are in EVIDENCE.json / manifest. Initial bare rustfmt lookup hit the default read-only RUSTUP home; pinned toolchain invocation passed before final checks.

No real Provider, remote host, signing, purchase or remote repository operation. No macOS result claimed for this source. Independent review and fresh exact CI remain pending.
