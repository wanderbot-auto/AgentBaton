# Checkpoint

Code source: `ea9bcd12fc78016922c86b40ed84108dbf918b70`, parent `b22b6d4835ecb86ea52a8e2c194ff28ff6e4b0e2`.

Implementation is ready for a **separate independent source review and actual dual-Mac CI**, not accepted as a Native/CI pass. The code commit modifies only .github/workflows/product.yml and tests/infrastructure/test_integration_native_ci.py. These four records are a separate documentation commit.

The existing helper builds a generated legacy Work resource to an unconsumed independent integration proposal. It has no TaskAccepted Graph chain and does not run the integration coordinator or a real ref CAS. This slice verifies intended full Native policy confirmation and phase-specific Cancel semantics only when actually run. It does not establish full AB-VAL-009, Unicode support, provider/model behavior, OS sandboxing, human Owner review, signing or production release.

Cancel phase expectations: pending/unconsumed proposal, exact original rows/sequence unchanged, no finalize/approval, typed PERMISSION_DENIED/not_started. Approval phase expectations: all original bounded Native pages/buttons, genuine private finalize, consumed proposal/result/current policy binding, schema17 policy1 with run/attempt/fence0 and exact ordered events. All are enforced by existing runner/Rust code, not fabricated by workflow glue. The runner retains its original85s per-phase clock, 1MiB envelope and cleanup rules. The outer Mac job remains60min; actual added duration/capacity is unknown until CI, without automatic timeout increase.

Two projected JSON reports are explicitly native_report_asserted, not independent execution attestation. The added finite summary gates on separately retained driver exit codes and projection exit codes; neither alone washes the other. New raw paths/files are not uploaded. Visual evidence stays private and visually unreviewed by this slice.

Root should review the frozen code, include the new infrastructure test in its normal suite, then run both exact Mac lanes. Preserve any original failure and report actual counts/architecture/source. Do not transfer earlier Linux/native tests or old CI results to this new workflow revision.
