# Checkpoint

Code freeze: `8902da6a7f3fe130d82d068cc0ff018e6cd2d0fc`, branch `codex/integration-release-audit`, baseline `b452e4a31dd4d1c9cce7e35282a9c2c9dcf0ff39`.

Only scripts/native-audit-release.sh, new tests/infrastructure/test_native_release_audit.py and these four records changed. The previous Runtime source/evidence branch is preserved. No release binary or shared target was accessed, changed or leased.

Recheck command: `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests/infrastructure -p test_native_release_audit.py -v`. Actual local result:8PASS; before fix7PASS1FAIL, retained separately. This command executes only the real audit shell, local inspection tools and controlled fixed failure tools against private non-executable byte files. It does not audit an actual product binary.

Pending: third-party review, full infrastructure regression and actual default-feature Apple Silicon/Intel release audits. No Mac/native/production-delivery claim, new accounts/fees, CI trigger, PR, merge, push or formal approval follows from these canaries. Keep the old missing-marker failure and limits when integrating.
