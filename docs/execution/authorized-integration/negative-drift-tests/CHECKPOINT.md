# Source checkpoint

Source commit `246955d43786e124be7d88c3071a49e7a7813e6c`; original test addition `012f4a55c4feb21c6d5b54ad6f4b25bc733dca45`, baseline `bfcc942b554113ea6c18133a9a998b22ad574dbe`. The later source revision only clarifies the independent root-commit ancestry comment. Author worktree branch `codex/integration-validator-target-drift`.

Source-only handoff: new tests need independent review and Root execution. No push, PR approval, merge, CI trigger, paid Provider call or product process occurred in this author task. Root retains sole integration and heavy-execution ownership.

The original fixture has 124496 bytes / 3150 lines, SHA-256 `b256ffec06a24da4c0119959efde792b092c779d2c8cec0848a158cbbaa0c83c`. Prefix equality was checked against the actual Git object after formatting; only the appended suffix changed. Full source SHA-256 `e8968ca75e5de1bb2bf5058e3dc26e5f260c16bcfd3f7d2be20f05d54022f7d4`.

Author ran rustfmt with the configured toolchain on the fixture, restored the original prefix byte-for-byte, and ran `git diff --check` successfully. An initial rustfmt call lacked RUSTUP_HOME and failed read-only home creation before formatting; the configured invocation succeeded. No Rust tests were run.
