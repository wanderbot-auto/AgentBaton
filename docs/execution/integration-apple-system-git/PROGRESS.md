# Implementation checkpoint

The fixed Apple source/environment policy is shared between Core and the actual workspace command builder. A private Apple proof is additive to `LeafPin`; generic capture still rejects non-regular files and `nlink != 1`. Context/program serialization and existing positive/refusal/runtime tests are unchanged.

New authored tests: 6 portable Core policy/private-file tests, 1 Darwin-only mount-constant test, and 4 workspace child-command tests. They cover wrong path/owner/type/mode/device, readonly and mount proof failures, changed links/identity/volume facts, ancestor mismatch, generic hardlink/symlink refusals, selector cleaning and Linux source/environment preservation. All 11 are NOT_RUN by this author; none prove a real Mac mount or runtime.

Completed validation: 10 Python source preservation/wiring controls, rustfmt check for four changed Rust files, and `git diff --check`. No Cargo, Rust compilation, SQL, actual Git child execution, Node, browser, network or CI was run.

OWNER-001/002 authority read and preserved. Overall product/native acceptance remains open. Root owns independent review, tests, composition and CI.
