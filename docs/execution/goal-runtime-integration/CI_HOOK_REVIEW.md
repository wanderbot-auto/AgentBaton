# Independent CI hook review

Verdict: **PASS for static/preparation dimension only; no GitHub workflow executed by this reviewer.**

Final Root workflow SHA2569189ad4eea3d7d0b4f36ab6735aae2b2db06cacf8ec053f9e3cd069942deedfd, against baseline78b317a4dc42f088f752852bf04028eb0bb51d9c/old SHA25699a8f28ec9900401fb548abeefdfaf3edb04847f9179f36e355050e3cba897f0. Final append26 lines. Goal harness503c/SHA6c75746a2402710ccb7eb58c9c2e9e91c0bc697a5d39974652b830d9adcf8114.

## Finding closed

Initial16-line hook, observed workflow SHA77d84e4818f259c2dc08e860f538f830ff91150441e99530c2e4a5a3784903ad, inherited harness default /usr/bin/chromium. Workflow installs Playwright-managed Chromium and did not establish that system path. This was a portability concern, not an observed remote failure. Root revised only the append to run inside the existing browser-venv, get installed Playwright chromium.executable_path and pass it explicitly. Resolved.

First reviewer verifier expected the old16-line marker after Root had already changed it; substring-not-found log is preserved. It was not a product/CI failure. Final verifier binds the final hash.

## Actual static verification

- PyYAML6.0.3 BaseLoader parse passed. Replacing only the new text in one existing UI run block restores identical parsed YAML.
- Inverse removal of26 appended lines restores every old workflow byte. Environment workflow/lane, Rust/UI/macOS steps, permissions/action pins/matrices/fail-fast/max-parallel and25/20/45-minute timeouts remain unchanged.
- Actual extracted run text passes bash -n; quoted heredoc delimits correctly. Embedded Python ast.parse/compile passes without executing it.
- Exact tracked-source assertion accepts empty status and rejects dirty tracked status. Untracked build artifacts are excluded as intended.
- AST confirms structured subprocess.run argv/check=True/no shell=True, same sys.executable in browser-venv and same Playwright-derived executable path. A harness failure fails the step.
- Extracted actual harness output.mkdir(parents=True, exist_ok=False) guard rejects an existing review-owned directory with FileExistsError, preserving sentinel bytes. Goal output directory and candidate JSON are distinct sibling paths from all prior outputs; earlier browser steps create the artifact parent. Existing output directories are not overwritten.
- Independently ran PYTHONDONTWRITEBYTECODE=1 python3 scripts/validate_infrastructure.py: PASS2 workflows/15 pinned actions, explicitly static_infrastructure_only/remote_ci_executed=false/product_tests_executed=false. Workflow git diff --check passed. No pycache/source/target writes.
- Exact argv, shell/Python copies and controls are in ci-hook-final-static.json and accompanying files. No actionlint/shellcheck result is claimed.

## Attribution and remaining gates

The hook reads actual current Git HEAD/tree, rejects modified tracked source and hashes the daemon just built by the same job. HEAD may correctly be an Actions PR merge checkout. Declared schema11 is attribution, not forged acceptance: harness independently checks real SQLite user_version11 and exact binary SHA. Candidate scope explicitly says schema checked by SQLite and acceptance pending.

Existing set -euo pipefail and always-upload artifact behavior remain. No extra installs, permissions, tokens, private identities, spending or release actions. Candidate JSON/report/screenshots fall within existing UI artifact upload.

Real Actions execution/result/artifact review is pending. ACTUAL_REVIEW.md separately proves an independent scoped Linux browser/Unix Core11/generated ACP run, not this CI hook, macOS, real Provider, TaskAccepted or whole-product completion.

