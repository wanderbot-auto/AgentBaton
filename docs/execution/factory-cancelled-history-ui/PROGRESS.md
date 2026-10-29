# Progress

The actual first-summary-cancel1 case passed CG01/CG02/CG04, including committed cancellation, deliberately lost response, exact-key recovery and final GoalCancelled. The whole case failed later while capturing final parent-cancelled history; cleanup_complete=false and CG06 was not run. Its original report and failures remain unchanged.

Wires197/204/255 show Factory cancelled/r7 with no active task/session, a joined real parent Factory attempt and a derived child cancelled/r5 with null Factory attempt, confirmed_stop=true, result_available=false and cancel_requested=true. Every handle and waiting parent reports confirmed stop. The old accepted-only derived-result guard rejects that child, FactoryRuntime clears detail and unmounts child history. The retained DOM contains the foreign/stale evidence alert and no child Goal Refresh control. This is a product readonly-shape defect, not a cancellation failure or a target-selection error.

Source ac03b997623036a1313dd64f810a03a6e9b8cc0d adds a separate scope-aware terminal cancellation read branch. It requires the complete validated execution Scope, exact cancelled/requested run, null active references, all stopped handles and waiting parents, plus all original complete-root/lineage/duplicate/foreign/terminal checks. Only a non-root cancelled/no-result/requested derived child can use the null-attempt exception. An unconfirmed null-attempt child in a cancelled execution read is rejected. Original three-argument behavior and accepted-result behavior remain intact. No permission to launch or retry is introduced.

Added 28 synthetic guard/component cases, including deferred parent refresh, live/expired/revoked readonly display and contradictory/foreign/unknown/unjoined negative controls. These new tests have not been executed.

## Supported continuation follow-up — 2026-10-29T18:12:03.606088+00:00

Review found a P2 in ac03: factoryRunShape accepts a nonempty future context_continuation, while derivedCancelled did not require factoryRunSupported. Fixed by adding that predicate. Added an independent negative whose scalar shape is valid but continuation is unsupported. All original28 tests are byte-preserved; new total29 source cases. No actual Node result is claimed.

## Subsequent actual Root validation — 2026-10-29T18:17:21.626112+00:00

Root source2d12dff1cbf6748c2f9c2fef51600ff49d4d23b3 includes exact reviewed guard/test bytes6b8c. Actual typecheck0, full901tests/52files0 and build0. All29 new cases were part of the real full-suite run. New actual cancelled-history1 case passed in20.525272s with CG01/CG02/CG04/CG06 allPASS,3PNG and cleanup_complete=true. Cancel committed/dropped/same-key recovery and final cancelled history now both passed; final Graph and Factory are cancelled and Task is not accepted. This new result does not replace earlier failed runs.
