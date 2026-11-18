# Progress

Confirmed exact clean base, branch, active claim, gate and upstream source-pin evidence. Four authorized callers identified. Capturing a baseline before wiring.

Reproduced 15 failures on the exact task base. Fixed source caf445487fd622b3ae71f7a86e0b7ea570332e39 closed all 15 and passed all 37 targeted tests. Full infrastructure ran 921 tests with 905 passes, 16 existing skips and zero failures. Byte controls bind reviewed current inputs, exact predecessor outputs, final legacy outputs, unchanged old helpers and reader-wiring-only old consumer diffs. Logs and manifest collected; all controllers joined. No Rust/native/CI or historical producer runs in this slice. Ready for independent leader review.

Leader identified that the first live reader also recognized the exact predecessor on disk, allowing each old test module to go green against a rolled-back fixture. The new disk regression reproduced both wrong acceptances at fixed test source 67611e5fe6a00a9d0f9f8028d7550ac66f0df911. It ran one test with two failing subtests. Log SHA 78b541a9a42cb82bfec514b8ad34699ef2fd73b7755c7e70ea284084a65557c6. Remove predecessor pass-through entirely. No caller needs it since old helpers receive the restored bytes directly. The original current and predecessor pins and output checks are retained. Rerun the five modules and complete infrastructure from fixed source.
