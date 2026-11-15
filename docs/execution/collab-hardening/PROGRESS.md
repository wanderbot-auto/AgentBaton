# Progress

Local Apple Silicon/Bash 3.2: 12 actual CLI regressions and all 877 infrastructure tests pass. Design validation, static infrastructure policy, 93 bootstrap digests and syntax checks pass. Independent reviewer reproduced 12/12, additionally verified SIGINT exit130 and wrapper-SIGKILL/direct-child lease retention in separate /tmp fixtures, and found no must-fix issue. Source review only for Linux until remote CI runs. No product/native acceptance claimed.

Existing Environment workflow unittest discovery includes the new tests without changing CI triggers, permissions or budgets. PR #27 created at implementation 5b703b1d. All five expected check names observed from GitHub Actions app15368. Appended Product / Rust, Product / UI and Product / macos-15 to ruleset25893455, retaining all existing constraints. Final candidate CI pending; no GitHub approval-count change.


Candidate d3bb: remote Environment Linux passes; macOS runs all 12 collaboration tests successfully, but an unchanged synthetic bound-provider test fails. Retained run43698278274/job132763616353 and local raw log. Both root and independent reviewer reproduced the same guard refusal with a delayed 4ms sleep; CI lacks exact clock samples, so its precise delay remains unknown. Only the simulated test namespace now uses logical time, with an additional >=20ms refusal control. Product/browser code unchanged. Focused 52 tests and full 878 tests pass locally; independent reviewer reproduced both changed controls and accepted the actual diff. Final candidate remote checks still pending.
