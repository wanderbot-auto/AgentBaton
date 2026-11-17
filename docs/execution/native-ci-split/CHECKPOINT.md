# Checkpoint

Implementation base c3a001f8b248d79302317689f86ed7703e59e077.
Final reviewed/validated source 76fdc57bef072cf70a02f8d247e88cf2d6508b10.
Latest full infrastructure: **913 cases, 897 passed, 16 skipped, zero failures
or errors**. The unmodified raw validator, scope, diff check and merge previews
passed. Complete tracked-input maps are identical before/after validation.

Integration explicitly transferred the background reader after B finished.
This branch changed only its workflow raw read entries and two strict inverse
entry calls. Old Rust lock assertions/pins remain untouched; B's Host-dependent
file was not copied. Final independent source review reports no blocker.

The first CI target remains PR29: normally fast-forward the performance feature
from c3 to this package's final head, testing against main
a4e03303b54bd5a5544131e95ad39662f8310e29. It does not include the unaccepted Host
package. The c3 preview covers that feature ancestry. A separate explicit
compatibility preview against Host parent
56746af61789e3e7121e7f43d39b4a5d2b567576 passed without conflict, producing tree
a7f49706279b07acf518d78b436352240400f891. This is future compatibility evidence,
not the first CI combination or candidate/native acceptance. No refs changed.
Leader owns final review, the normal feature fast-forward and exact new-head CI.

No Cargo/native product process, push, dispatch, rerun, cancel or merge was
performed by this package. Cold ARM Native/default completion within30 minutes
and the full default App audit/package/upload still require real new-head CI.

The following records describe earlier checkpoints and retained failures.

Independent source review passed after fixing unsupported status expressions,
the Native matrix context and the late-cancellation verifier-skip edge case.
The protected aggregate has a final always-running refusal if cancellation is
observed or its fixed verifier outcome is anything except success.

Full infrastructure: 912 total, 891 passed, 16 skipped, five errors, zero
failures. All five errors are the frozen background source-reader entry and
its integration-setup downstream readers. The explicitly authorized workspace
diagnostic read adapter passed with all original assertions/pins retained.
Before/after hashes for all tracked inputs are identical at the tested source.

Background smoke source guard still belongs to host-pragma-compat and remains
byte-identical to c3. Integration must transfer it before the strict read-entry
adapter can be applied. No integration-setup reader write is currently needed.

Raw failures and successful targeted checks are gzip archived losslessly under
raw/ with byte counts and SHA256 mappings. The original 60-minute cancellation
is unchanged. No Cargo/native process, push, new CI run or merge was performed.
Real cold Native/default acceptance within 30 minutes remains unverified.

Follow-up source: 479f3dcc73e94b400b6b28e25976651b7fa28ca7. Actual workflow
read entries now preserve raw bytes before hashing; a real filesystem test
refuses CRLF/BOM/appended bytes. Integration explicitly granted four further
exact-source reader imports/read entries. 112 targeted tests pass, identical
tracked before/after inputs. Full regression remains pending the background
entry transfer. No repeated known-failing full run was started while waiting.
