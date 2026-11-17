# Checkpoint

Implementation base c3a001f8b248d79302317689f86ed7703e59e077.
Reviewed/validated source d4905e12370b571c698a865fed9533ff3c77d590.

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
