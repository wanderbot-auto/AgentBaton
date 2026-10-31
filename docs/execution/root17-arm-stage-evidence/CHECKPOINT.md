# Checkpoint

Status: implemented test-only diagnostic candidate, awaiting independent source review and Root compilation/runtime validation. Not a behavioral fix, CI pass, Mac acceptance, merge approval or full130 completion.

Finite stdout contract: `INTEGRATION_STAGE_FAILURE ` followed by one canonical compact JSON object, then newline. Contract `agentbaton.integration-stage-failure.v1`; status is `collected`, `unobserved_actor_join`, `unobserved_run_binding` or `unobserved_records`. Every output states `post_owned_shutdown_not_failure_instant`, `measurement_not_authority=true` and `process_stop_proven=false`. Collected output adds actual checked schema17 and nullable original event facts. Unknown error categories map to `unrecognized`; raw error text never appears. Serialization/I/O failure does not replace the original test panic.

The existing fixture lifetime and failed-temp-directory retention are unchanged. Construction failures before a `RuntimeFixture` exists still have only the original retained-directory behavior. If the original `finish()` itself panics before the diagnostic call, no line is promised; absence remains unobserved. A successful Actor join permits collection but is not reinterpreted as complete worker/process cleanup. Journal presence, failed/missing join, ambiguous run, corrupt/oversize/private metadata or any selected association error leaves the entire diagnostic unobserved. All observed values are after shutdown; teardown may already have changed the run and attempts.

Whole-file inverses restore e874 exactly:

- integration runtime fixture SHA256 `149693770c6c6bce56def9b9b8a8fe43f8fc7c0657fb31adef45595b64813e06`.
- integration test Server SHA256 `90fe84ccdecccddcdb6b8d0530fcbf29dad8d9390c0ac3bfc7200e93a469aabb`.

Root next: run the nine `integration_runtime::stage_evidence::tests` controls, including the original test-host finish/panic lifecycle; validate actual immutable SQL collection on an owned joined schema17 fixture; run the original unchanged Linux integration cases, then an exact dual-Mac candidate. Keep the ARM 8/12 failure and official Native summary unchanged. Add no timeout, Git fallback, grant/stop/source-pin relaxation, or speculative common cause. Product/Provider/remote/signing/full130 dimensions remain as previously recorded.
