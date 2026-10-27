# Read-only command harness independent review

Scope: three executable harness files added in `0f3d94502331bd0e1e1c464617a30df7464b7c14` relative to parent `019a318`, plus one-line selector correction `b4a758c62d4797c5d9379a63b5cfa3ddc2108ec7`. Read exact git objects; other documentation/control changes in0f3 are outside this small review. No Cargo/full application or Core8/real native/browser integration execution was performed by reviewer.

Verdict on corrected b4a758c: no unresolved blocking source finding. Ready for actual composed schema8/backend/browser and macOS runs. Neither old c4f CI nor these static checks proves the new harness passes on actual native UI.

## Finding and closure

F1 at0f3: `command_panel.locator('details').evaluate('(e)=>e.open')` matched the outer details plus environment and limits details already present in the product DOM. Playwright's strict evaluator therefore fails before any acceptance claim. Reported to primary; primary changed only this selector to `:scope > details`, retaining the closed assertion. No scope assertion, deadline or prior check was removed.

Independent isolated DOM control (not the product/browser IPC harness): a synthetic panel with the same outer/environment/limits details structure has3 total matches; the old exact selector actually raises strict-mode violation. The corrected direct-child selector has1 match, observes false initially, and true only after summary click. Old failure and fixed-control logs remain in /tmp. Reparsed exact corrected Python source successfully. This demonstrates selector mechanics only; it is not a schema8, native WebView, built-product visual or real-daemon test.

## Source audit

Native FIRST adds only two fixed production list invocations for the created task: command_validation_policies_list and command_validations_list. Both must return actual empty arrays. It locates the real product panel, requires default closed state, opens its summary, waits until its actual form fieldset is enabled after successful scope-valid list reconciliation, requires no alert, and requires approval disabled for this draft with no stopped session. A real system snapshot sequence must remain unchanged. All3 new report flags are emitted as literal booleans only after those checks and successful return to conversation; the catch emits ok:false without success flags. No new native invoke for approval/run/cancel/finalize was added to this injected acceptance path.

The shell parser adds exact `is True` requirements for commandContractLists, commandContractEmptyControls and readonlyCommandNoMutation. Independently extracted and executed the literal parser against synthetic JSON fixtures: all3 booleans true accepted; each individual false, string"true", integer1, null or missing flag refused (15 negative controls). This executes only the parser; fixture os=macos/status=passed is synthetic and is not macOS evidence. Existing Data/goal/restart/cleanup requirements remain unchanged.

The manually invoked localhost browser adapter gains exactly two explicit read-only mappings, each restricted to taskId. It does not add command approve/propose/run/cancel/revoke, approvals.finalize or generic IPC dispatch. Existing origin/content type/body/key restrictions remain. Shipping application controls, permissions, Cargo feature definitions and native-run.py are unchanged; the test server remains a manually invoked test-only script, not a bundled product capability.

Across each of1488x981,1440x900,1024x768 and390x844, the new browser path folds the Data panel, opens actual default-folded Command controls, waits for successful actual list reconciliation, checks refresh enabled/approval disabled/no alert, requires successful recorded list calls, checks no horizontal overflow, records a new PNG, asserts unchanged journal sequence and rejects recorded proposal/finalize/run/cancel mutation calls. Existing integrity/Data and all previous layout/focus/cancellation/history assertions remain. These are future runtime assertions, not reported passed runtime results.

Compared deadline expressions and SECOND script to019a318: all existing expressions and the complete SECOND body are byte-identical. Original10s page deadlines,60s native integration deadline and unchanged80s native-run.py wrapper remain; no test timeout or retry loop was expanded. Native smoke source and shell script are identical between0f3 and b4a; only the browser selector differs.

## Independent checks

Exported exact sources to `/tmp/agentbaton-command-harness-review-0f3d945/` and ran:

- Extract FIRST and SECOND raw JS from native_smoke.rs; `node --check FIRST.js` and `node --check SECOND.js`: exit0.
- `ast.parse` on exported browser Python source, including corrected b4a version: pass.
- `bash -n` on exact native-macos-smoke.sh: exit0.
- Extracted shell Python report parser: all-true control exit0;15 malformed/missing flag controls nonzero as required.
- Isolated Playwright synthetic-DOM old-selector failure/new-selector unique closed/open control: pass, expressly not actual product browser acceptance.

No Core/native application build, sidecar execution, real owner dialog/IPC, schema8 command list call, screenshot of actual new product, macOS acceptance or full GOAL result is claimed. No source/lock/index change, stage, commit, push or subagent by reviewer. Only this document and independent temporary source/logs were written; unrelated NATIVE_C4F9_REVIEW.md work was preserved.

## SHA-256 evidence

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/agentbaton-command-harness-review-0f3d945/native_smoke.rs` | `3f18a3623727e18df8188d1257ad3947fc73b0c65d0e1a04278eada1813ce5d9` |
| `/tmp/agentbaton-command-harness-review-0f3d945/quiet-canvas-real-ipc.py` | `3881f58f968704ebc387542edaa9e54390933907f50fb9877b920c788fff7525` |
| `/tmp/agentbaton-command-harness-review-0f3d945/quiet-canvas-real-ipc-fixed.py` | `feccc553932de9056da22bfdb0719eaf0dca73ce3bad9cda5edfe90d2cab4b97` |
| `/tmp/agentbaton-command-harness-review-0f3d945/native-macos-smoke.sh` | `87d252b41782a90181f6b2b6fd354d39a421230f768fc321a8b668b32b336060` |
| `/tmp/agentbaton-command-harness-review-0f3d945/FIRST.js` | `0c2c4bdd5145b3deaba657c754350413dfe098282aefdd99b005388eeeb64333` |
| `/tmp/agentbaton-command-harness-review-0f3d945/SECOND.js` | `067585ef62566bedc2069ebf983d9964f94ca6fa27ce4ebf9764c2514527f5cc` |
| `/tmp/agentbaton-command-harness-review-0f3d945/report-parser.py` | `a025b185ef2c7f8c720eb4d4fc32eae9b06a3fbd80f492a668c4579cd7f091c0` |
| `/tmp/agentbaton-command-harness-review-0f3d945/static-results.log` | `b43059b444c06e40172aae9b555c78a2f3e229a6f8c5603489855e2a80eb0098` |
| `/tmp/agentbaton-command-harness-review-0f3d945/old-selector-failure.log` | `4a5817f723919c5c177654d63256619f2265ce5e227cbc73724aca142dbb226a` |
| `/tmp/agentbaton-command-harness-review-0f3d945/fixed-selector-control.log` | `80ddd8eb5e6db77606a457fa99ed8528cb442f9f478adbb539ed448481ef57ed` |
