# PR #33 partition checkpoint

The recommended first implementation boundary is after original macOS step 12. Keep the existing `desktop` ID and 60m cap, with an ARM64 `desktop_release` job capped at 30m and owning the original `Product / macos-15` required name. A first-step dependency gate explicitly rejects anything other than success. The contract keeps the other four required check names and matches evidence to the same PR merge checkout and run attempt. [Contract](CONTRACT.md)

The original attempts finished step 12 at 50:49 and 47:37, then cancelled in background registration and Native Integration respectively. Neither reached packaging. The contract retains both observed outcomes and the exact artifact IDs so a partial rerun cannot reuse older evidence by name. Cold downstream timing and actual account headroom remain open. [Timing](TIMING.md)

Local proof accepted three ZIP-based layouts and one direct-output case. It rejected 99 bad ZIP cases and 7 bad direct outputs. Three staging controls confirmed byte reuse by the actual sidecar staging script, a missing-input refusal and a compiler-call refusal. The exact workflow splice passed the original inverse functions and rejected seven malformed splice cases. The existing guard/provenance suites passed 84 tests. [Replay commands](RUN.md)

The implementation still needs a cold hosted run on its exact PR merge checkout, actual blocking dependency behavior, all original native/package gates and evidence, artifact transport or output validation, and a fresh read of the live branch rules. This task delivered only local contract evidence and performed no hosted submission. [Evidence](EVIDENCE.json)
