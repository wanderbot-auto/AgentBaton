# Independent macOS CI build-envelope review

Verdict: **no blocking finding; suitable for new exact-head CI.** Reviewed against a2c5bcf47fd60d66207e2cc93c94db4f7f648057 by `/root/data_native_integration_review`. Only `.github/workflows/product.yml` is modified: the matrix desktop job's overall timeout changes30→45 minutes, with two explanatory comments. This is a bounded build/test/audit/package execution envelope, not a product or per-test deadline change.

Actual prior Intel run exceeded the overall30-minute job limit while still compiling the release application, after Core/nativehost and actual Native WebView/lifecycle had passed. The check-run annotation explicitly says maximum30m0s; old raw evidence and cancelled result remain preserved. This adjustment does not retrospectively make that run green or supply its absent Intel DMG.

Mechanical inverse proof: remove exactly the two new comments and restore desktop45→30, and the entire workflow becomes byte-identical to the exact old Git object. All runners, matrix/parallelism, tests, steps, timeouts inside product/native runners, scopes, failure propagation, default release audit, packaging and check names are unchanged. Critical command/provider/native runner/browser/audit/package source bytes were additionally compared with exact Git objects and remain unchanged. YAML syntax result: {'parser': 'PyYAML', 'version': '6.0.3', 'parsed': True, 'only_semantic_change': 'jobs.desktop.timeout-minutes30→45'}.

This consumes only the already authorized hostedCI quota, creates no new resource or account, and buys nothing.45 minutes remains finite. No paid provisioning, real Provider calls, signing, production or new permissions are introduced. Current quota sufficiency is not asserted.

No functional test was rerun for this workflow envelope-only edit. Existing exact A2C5 results are historical evidence, not the upcoming source's checks. Root must normalpush a new reviewed subject and inspect its fresh exact seven checks, both Native reports, both default-release audits and real dual DMGs before guarded merge. This reviewer did not push, trigger/retry/cancelCI, edit source/index or merge.

Manifest SHA `055cf7517faaf71b5150f325aa571fb75a40c83870dada123ff0ffd36a8d14e4`; full before/after workflow hashes and every critical unchanged source hash are in CI_BUILD_BUDGET_VERIFICATION.json.
