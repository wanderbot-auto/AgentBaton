# Main branch protection — import required

This file and `github-main-ruleset.json` describe the intended policy; committing them does NOT activate GitHub rules.

## Policy

Target only `refs/heads/main`; active enforcement; no bypass actors; require a pull request; block deletion and non-fast-forward updates. Require all three existing `Environment / ...` checks from GitHub Actions (app ID 15368, verified from the repository's successful check runs), with the branch up to date. Resolve review conversations before merge.

Approving review count is 0 because this is a single-maintainer preparation workflow. A PR and successful CI are still required. This does not authorize product development, production deployment, real Provider calls or paid resources. Add independent human review requirements only when another reviewer is available and the policy is approved.

## One-time activation

In this repository's Settings > Rules > Rulesets, use New ruleset > Import a ruleset, select `infra/github-main-ruleset.json`, review, keep enforcement Active, then Create. If a matching ruleset already exists, compare/update it instead of creating duplicates. Do not add an administrator or integration bypass merely to make CI or merging pass.

After saving, read `GET /repos/wanderbot-auto/AgentBaton/rulesets` and `GET /repos/wanderbot-auto/AgentBaton/rules/branches/main`; verify the exact contexts, expected integration, PR requirement and block rules. No destructive force-push or delete test is required. Do not mark enforcement as verified based only on this JSON or static tests.

## PR checks and public repository safety

All PRs targeting main run the infrastructure checks without path filters. The workflow uses `pull_request`, a read-only token, no retained checkout credentials, no secrets and GitHub-hosted disposable runners. It does not use a privileged PR trigger, production credentials or a personal self-hosted runner. Public forks should continue to require the account's normal workflow approval policy; this change does not relax repository-level approval settings.

The existing explicit push trigger is limited to `infra/ci-request.json` on main. The runs remain preparation tests, not product test evidence. Publishing the repository does not select an open-source license; the specific license is still awaiting the established review gate.

Sources: GitHub Docs, REST API endpoints for rules (create ruleset); Managing rulesets for a repository (JSON import); Troubleshooting required status checks. Read access to rules is not Administration:write capability.
