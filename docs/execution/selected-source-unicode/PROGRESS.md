# Source progress

Source8d8f24c adds exact UTF-8 support, full casefold ordering and descriptor-relative actual spelling/identity guards. Root's separate949cf1e changes only the direct focaccia dependency and lockfile. No public API, schema, migration, Core, Command or UI code was edited by this author.

Eight new actual-owned-Git fixture test declarations cover raw Chinese/emoji/space/combining paths and exact blob/tree bytes; NFC/NFD distinct-byte success versus real observed same-inode host refusal; previously rejected unselected Unicode base preservation; full casefold aliases with base entries absent from checkout; selected case/dir aliases; changed pins plus Unicode symlink/hardlink/control/traversal/.git rejection; invalid UTF-8 base/selection; and byte-counted path limits. No generated Git fixture was executed by the author.

The old combined ASCII/Unicode restriction test retains its ASCII safety body verbatim under a narrower name. Its valid Unicode blanket-refusal portions are replaced by explicit positive/raw-byte and real-alias negative tests. The original historical implementation/tests remain in the prior Git commits. All other old test entrypoints remain unchanged. Source-only rustfmt and git diff --check passed;12 static/source byte comparisons passed. The complete original raw-object capture function and private unit-test tail are byte-identical.

Downstream source inspection: validation::safe_relative accepts valid UTF-8 normal components, and Core integration artifact/held-file pin guards use normal components and4096/64MiB checks rather than ASCII restrictions. Command policy uses these bound artifact paths and preserves original argument/image contracts. This inspection is not an executed Unicode Core/Command/UI chain. Branch-ref ASCII validation is a different existing target-ref restriction and was not changed. macOS Unicode normalization/collation, actual FFI behavior, performance within the original deadline, end-to-end integration and browser display all remain unverified.

## Independent review corrections, source0fa6dad

The retained8d8f24c independent review is CHANGES_REQUIRED, not runtime acceptance. P2-01 found existing_identity checked the original control only around the complete multi-component walk. Source0fa6dad9bc474731c0d8778745b458b8dbc47945 passes the same capture.deadline/control into the walk, checking before/after root descriptor clone and before each component plus after openat/fstatat; no new clock or cap is introduced.

P2-02 found the invalid-UTF8 fixture assumed the host filesystem could create a0xff filename. The replacement builds an actual raw Git tree with hash-object, ls-tree and mktree -z input, then commit-tree/update-ref binds the generated base without checking out the bad name. Each setup command must succeed; capability construction must succeed; capture must specifically return WorkspaceError::InvalidPath. Invalid selection bytes are tested directly at BoundArtifact with exact InvalidContract, without writing an invalid host filename. No preliminary host refusal counts as a passing product guard.

Author source inspection also found the new control-path test combined ValidationError and WorkspaceError using incompatible and_then error types. It now matches the two existing guard layers explicitly and accepts only the intended InvalidContract or failed source selection; other errors fail the fixture. This was found statically, not reported as an executed compiler failure.

Standalone rustfmt/diff checks and8 additional source comparisons passed. Author did not compile or run this revision; actual Git/macOS/typed error behavior, performance and full product Unicode still remain NOT_RUN. The original review and earlier source are preserved; neither is relabelled as passed.

Independent review of0fa6dad closed both P2 findings and the explicit test error-type correction as SOURCE_READY (review SHA7043000ec1adf89a8f62bdb002d4deb4ce22c73d50467a3cca723c47c06ad290). This is source closure only; original8d CHANGES_REQUIRED remains, and no runtime/Mac result is inferred.
