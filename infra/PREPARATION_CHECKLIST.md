# AgentBaton environment preparation — public repository update

**Product development remains on hold. Repository and Cloud preparation do not authorize product work.**

| Item | Verified state / evidence |
|---|---|
| Repository | `wanderbot-auto/AgentBaton`; public, confirmed through GitHub API after the user's visibility change |
| Design baseline | Design v0.2, 27 questionnaire answers and 130 requirements preserved |
| Prior three-platform CI | Run `40336413916`, tested commit `02508d4eb1c32d35cc9deda546f334938abdd357`; Linux, macOS arm64 and Intel succeeded |
| Cloud preparation | User-reported setup success: Python 11, Node 4, Rust 2 tests plus design/infrastructure/integrity/Clippy; cloud EVIDENCE.json not independently read |
| Cloud publication | User screenshot shows published, Only me, no network keys or environment variables; environment ID not independently retrieved |
| PR CI | This change adds checks to every main-targeting PR; success must be read from the new PR's actual run, not inferred from previous CI |
| codesign diagnostic | This change checks executable presence only; it no longer runs unsupported `codesign --version`; signing remains untested |
| Main protection | API returned an empty ruleset list and `protected=false`; importable active policy is prepared, not applied |
| Execution authorization | `infra/execution-gate.json` unchanged; product, real Provider calls, production release and paid resource authorization remain false |

## Remaining activation

Import `infra/github-main-ruleset.json` in GitHub Settings > Rules > Rulesets, review and Create. Verify the resulting active rules via API before declaring this item complete. The current connector has no Administration-write/ruleset creation operation; public visibility removes the plan restriction, not this tool limitation.

## Evidence boundaries

Preparation smoke tests are not product tests or minimum-macOS validation. There is no Tauri application build, signed installer, real Agent integration or production credential setup. Failed or unrun checks remain visible. The original CI evidence is tied to its exact commit; the new PR must have its own check results.

The publicly visible repository still has no final approved open-source license. Keep the existing license-review decision; do not silently select a license.
