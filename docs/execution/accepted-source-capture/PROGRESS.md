# Progress

Source frozen at `2784897058dbf81b29432bac096892b01f0fa98d`; only three allowed source paths changed. Source and test implementation complete for review; Rust compilation/runtime not run by this author.

Implemented explicit expected SHA256/root/devino validation, descriptor-safe BoundArtifact reads, before/after selection/root/common/HEAD checks, literal exact-path raw-object capture, private index, base roundtrip, fixed commit metadata, unsafe config/alternate/object symlink rejection, case/prefix guards, total bytes and one global monotonic deadline. Fixed metadata disables commit signing and encoding drift. Scope/approval references are host inputs bound into the digest, not authenticated here.

Added 13 integration-test sources using generated local repositories/files (not executed): exact dirty selected capture, reproducibility, SHA/file/HEAD drift, identity/path/ID/count contracts, symlink/hardlink/FIFO/oversize, base special types, unsafe filter/include/merge/promisor/alternates, hooks, replacement root/common, ASCII/Unicode aliases, total byte budget, object-store symlink, and empty base subtree. One cfg-unit source creates a real Git fixture then injects an expired private deadline before capture; no public test knob or sleep.

Executed only direct rustfmt check, git diff check and source-only mechanical checks, all exit0. The lib inverse is byte-equal to base after removing its one export. Static checks are pattern/scope checks, not compilation or security/runtime proof. The first shell attempt targeted the not-yet-created tree and failed before execution; then the explicitly authorized tree/branch was created from the exact base. No failed product test was run or erased.

No Cargo, rustc, generated Git test, Actor, Clippy, macOS, UI, Provider/model, remote, workflow or budget changes/actions. Disk was approximately284MB at implementation start; no caches or evidence were deleted and no large test data was created. The total-byte test, when Root runs it, uses sparse33MiB files and can retain approximately33MiB scratch on expected rejection; Root must budget that disk.
