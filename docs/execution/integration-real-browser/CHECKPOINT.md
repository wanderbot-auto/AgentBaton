# Checkpoint

Branch codex/integration-real-browser, baselinebbac3a113bc0ae5024b0709bea335e3da9ac7d35. Six new files only; no prior branch/work was overwritten. Source freeze commit is supplied in the handoff rather than self-referencing this file's future commit.

Implemented harness/source;34 unique pure driver tests PASS. Initial symbolic test failure and all later observations remain recorded in PROGRESS/EVIDENCE. Actual browser/Core/Native/CI evidence is NOT_RUN, independent review PENDING. No whole product or verification percentage is claimed.

Root next steps: independently review the frozen six-file delta; integrate it on Root's source branch; build Core batond+baton and frontend with matching exact-source metadata, then run using existing dependencies:

    env PATH=/workspace/.toolchains/cargo/bin:/usr/bin:/bin python3 apps/desktop/tests/integration-real-ipc.py --batond <frozen-batond> --candidate-manifest <schema17-manifest> --frontend <built-dist> --frontend-build-record <actual-build-RESULT.json> --expected-source-head <exact-Root-commit> --chromium <existing-chromium> --output <new-private-output> --case all --run

Without --run preflight returns2/NOT_RUN; missing/mismatched source/schema/tool/build/dependency returns2 with no process/token/listener. --case normal and --case command-failure are independently selectable, fresh runtimes. The source guard requires the actual driver and clean tracked source to match that exact Root commit; do not relabel earlier binaries/builds. Record real report/commands/results/hashes and review failures separately. The finite30s authority can expire on loaded machines: preserve genuine refusal/unknown; no clock changes, cap extension, synthetic rows or new-key retry of the old unknown run. Missing OS Native/macOS credentials/hosts block only those separate gates.
