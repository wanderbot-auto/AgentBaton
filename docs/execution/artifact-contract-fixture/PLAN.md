# Artifact contract fixture correction

Baseline bf67629f7911531fec52ec31e59c9508f814f9fd, branch codex/artifact-contract-fixture. AB-START-001 / OWNER001/002 ACK. Scope is one test file and this unit records; no production/lock/CI/authority changes. Requirements AB-VAL-001/002/003/006 and AB-REL-001 remain partial whole-requirement contributions.

1. Preserve exact original source and Intel raw failure identity; do not invent its unprinted reason.
2. Separate normal-exit artifact validation from the original30ms unbounded-writer negative control. Missing and deterministic5000-byte output must actually exit0 and fail their exact file contract under existing Fixture default1000ms; keep all original assertions and the30ms writer. Add safe structured reason/observation/reap diagnostics.
3. Exercise an explicitly generated delayed-script mechanism under30ms versus the unchanged1000ms default. This demonstrates the mixed-test intent only, not historical macOS cause.
4. Run the complete command-validation suite, workspace all-target Clippy/fmt/diff using only the released private Provider target. Save source/log hashes and freeze for independent review; actual new dual-Mac CI belongs to Root.
