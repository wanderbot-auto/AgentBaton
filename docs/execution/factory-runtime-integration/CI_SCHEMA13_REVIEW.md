# Factory schema13 CI delta: independent review

Verdict: **PASS within the static/finite-control dimension; no blocking finding.** This is independent code review evidence, not a GitHub Approve, runtime acceptance or a product-completion claim.

Exact subject: source `98bc23ee10529ed290eaaaa68412e4e3ff971aef`, documentation HEAD `1f6c69a3c884c023fdb56f3bfa21c5a02fb66e2a`, author worktree `/workspace/.worktrees/factory-ci-schema13`, baseline `8c6ea33c493d1936dfe7521d1bb96f0e410319ad`. The preapproved async Playwright lookup is local `435ac7b6dfc8810646109c414c244cb403a12954`, picked from `65f7fd26bec8a88307178a69381389de6188efaa`. Source was read-only throughout; tracked tree clean at start/end and seven source before/after hashes equal. Bindings are in `bindings.json`.

Scope: AB-TSK-002/003/004/007/008/012 and SEC003/010 wiring. No requirement marked fully accepted. Five authored paths are product workflow, Goal execution/Graph/Data harnesses and native macOS smoke shell. Integrated Factory driver is byte-identical to upstream `744bf29d255b618c54a905dff72a784942903f83`, SHA-256 `4aa84fb34529e6712923e5783e7824d3965adbce7742ca8386b22a9d45264162`; no local driver variation was substituted.

## Findings and preservation

No must-fix finding in this delta. The schema guards require `type(value) is int` and exact 13 before binary/runtime work; 12, 14, 13.0, fractional floats, booleans, string and null fail closed. Live SQLite assertions require user_version 13. SQLite returns integers; synthetic in-memory controls test 12/13/14, not migration execution. Native postcheck delegates the existing strict Factory nine-field validator and strengthens its two legacy schema asserts without dropping original assertions.

The workflow builds both sibling tools with locked Cargo and hashes raw bytes of both `batond` and `baton`, with checkout HEAD/tree and tracked-clean check. Existing shell prefix and three Python subprocess callsites retain the original 12 logical flows (five shell flows, Goal execution, Graph Owner and five validation modes). Each Factory case gets its own output path. Its exact aggregate AST was executed with recording fake subprocesses for all eight normal/cancel/unknown failure subsets: all three calls occurred every time; a nonempty failure set gave nonzero SystemExit. These are control tests, not actual Factory runs. Earlier legacy flows retain existing fail-fast semantics; this loop is reached only after they succeed.

YAML outside the two authorized frontend run blocks is equal to the approved lookup baseline. Both action pins and artifact-always upload, permissions, triggers, concurrency, runners, job names, matrices, max parallelism and 25/20/45-minute limits remain intact. The Environment workflow is byte-identical to baseline, preserving its three required names. No default production release audit line was altered. Async public Playwright executable lookup remains exact. No new daemon options, permissions, models, accounts or credentials are introduced.

Full Goal/Data source equality holds after only their enumerated schema/type/description replacements. Graph's cumulative diff is limited to schema13 guards, live checks, attribution/refusal descriptions and extra rejecting self-check values; HTTP allowlists, authority, time limits and assertions are unchanged. Every original Native non-schema assert is AST-identical and ordered, and the complete shell prefix is byte-identical. The integration-only driver's bound adopted-child reaper is inherited exactly from the reviewed upstream: Linux-only, verifies previously bound process/file identities and waits a specific adopted zombie, sends no signal, restores subreaper state and labels the wait as controller evidence, not Core descendant join.

## Actual checks

- Author finite script independently replayed with `PYTHONDONTWRITEBYTECODE=1`, `python3 -B author-replay-files/integrated-744/check_static.py`: exit 0, **81 controls**, no known dependency failures. Actual invocation used the original `author-archive/...` location before renaming the replay working copy; the untouched archive was then extracted separately. `author-replay.stdout/stderr` and working outputs are retained.
- Independently authored `python3 -B independent-check.py`: exit 0, **87 controls**, recorded in `independent-results.json`. These include eight aggregate failure subsets, strict manifest types in four harnesses, generated in-memory SQLite guards, raw two-tool hashing, old-flow/Native preservation and exact workflow metadata equality. No fake binaries were executed.
- `python3 -B scripts/validate_infrastructure.py`: exit 0; **2 workflows / 15 pinned references**.
- `bash -n scripts/native-macos-smoke.sh`: exit 0.
- `git diff --check 8c6ea33c493d1936dfe7521d1bb96f0e410319ad 98bc23ee10529ed290eaaaa68412e4e3ff971aef`: exit 0.
- `git status --porcelain --untracked-files=no`: exit 0, empty; exact source/hash and HEAD before/after equality passed.

Commands and logs: `final-checks.json`, `independent-check.py`, `independent.stdout/stderr`, `author-replay.stdout/stderr`. YAML parsing/Python AST compilation do not execute workflow commands.

## Evidence and retained failures

Author archive SHA-256 `302a4303e47281ba1de613bd79a4b44589cd613c31b17d34c631d557d7060ecd`; external manifest SHA-256 `beb9718738241c69ee87c4a6c1f96d9af865742f99f928972963ec71b60fdce5`. Independently verified all **33 regular members** (32 payload plus separately hashed MEMBERS.json), lengths/hashes, unique names, no traversal or links. All old 77 control names remain in final 81.

Archived exit1/77 controls and the old driver's acceptance of float13.0 are unchanged. The old schema assert was also evaluated independently with 13.0 and demonstrably accepts it; this is finite assertion reproduction, not historical runtime reproduction. Reviewed driver rejects it. Two reviewer setup errors remain separately recorded: the first archive verifier incorrectly treated the separately hashed internal manifest as payload; the first independent script guessed a nonexistent Environment filename. Corrected verification passes; neither is an author/product defect. No failure evidence removed or acceptance lowered.

## Not run / remaining boundaries

No Cargo, target writes, build, binary/daemon launch, browser/Chromium, actual Core migration, Factory normal/cancel/unknown, Native/macOS/WebView/OS dialog, real Provider, GitHub workflow or remote mutation. No push/Approve/merge/dependency installation/recursive delegation. The 12 old and three new flows are statically retained and wired, **not claimed runtime-passed**. Root/Core's separately observed wrong-start-CAS failure is neither resolved nor waived by this review. Exact composed candidate runtime, dual macOS CI, artifacts and the remaining product criteria still require their own real evidence.
