# Progress

Locked tempfile3.27 source confirms tempdir_in delegates through default Builder with permissions=None. Unix create uses DirBuilder::new and only specifies mode when permissions=Some; crate documentation states default directories0777 subject to umask. Production evidence root_fd requires Directory mode&07777==0700 and euid. Thus022→0755 has a concrete source mechanism; actual failing CI root mode was not printed and is not yet directly observed by this author.

Preparing minimal explicit-private fixture and one root0755 rejection control. Root localbefore reproduction pending. No Cargo/Actor/Node/browser or CI execution by author.

Source frozen f6074de9510cc8ccb35b930ba397547641737bf8. Only constructor requests Builder.permissions0700 at creation, with original canonical temp parent; import PermissionsExt and append one negative. Inverse-normalizing just the import/constructor and removing the one appended test reproduces the entire4344 test file byte for byte. All9 original test functions/thresholds remain. Total test functions10, all NOT_RUN by this author.

New negative starts from the corrected private fixture, verifies0700/euid and real report bytes, changes only its test-owned root to0755, verifies unchanged dev/ino/uid and bytes, and calls the actual capture execute path expecting exact INPUT_CHANGED. No global umask change, fake clock, root identity substitution, production exception or deadline change. The positive original threaded capture remains intact and must pass under Root's022/002/077 actual matrix.

Actual direct rustfmt --check and git diff --check exit0. Static inverse-byte proof and locked tempfile source tracing PASS; static record outsideGit. No Cargo/rustc/Actor/Node/browser/CI execution. Root localbefore and new Mac/CI execution still pending; source mechanism is confirmed but this record does not claim a directly observed CI directory mode or an actual fixed CI result. Original41004930335 CI137PASS1FAIL and secondary collector outcome are not rewritten.
