# Checkpoint

The original Root 6a cancel execution remains failed. Its line 830 composite assertion checks nonempty, <=8192-byte and NUL-terminated cmdline; failure does not identify which condition failed. The old report retained neither the failing proc bytes nor state, so empty argv or zombie state cannot be established retrospectively. Its weak verified Graph record contains expected identity/argv, not the new stable initial proof. Do not backfill it with new metadata or attribute the same cause to Parent 7af.

One actual cancel call and one committed reply deliberately dropped were retained. The harness failed before the second exact-key recovery call and actual cancelled view required for CG04. Daemon exit 0 and later absence of the recorded identity do not supply those missing business checks. Cleanup_complete remains false and no old outcome is reclassified.

Candidate is source/pure verified only. Root must independently review, then run fresh actual normal/cancel/lost-start fixtures with real paired inputs. Strict initial capture may now expose a short-lived worker that was never observed live; retain not_verified rather than fabricating proof, adding sleep or enlarging original budgets. The helpers do not signal/wait/retry or interpret cancel_requested as stopped. Actual Mac diagnostics CI may proceed independently; this patch does not establish any Mac root cause or new CI pass.

Private process metadata/paths and original raw fixture records are not public artifacts. These records contain only hashes, source boundaries and finite facts; all failed roots/unknown outcomes remain preserved.
