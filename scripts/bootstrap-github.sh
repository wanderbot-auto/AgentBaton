#!/usr/bin/env bash
# Creates only a NEW PRIVATE personal repository. No secrets, billing or releases.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OWNER=""; APPLY=0; RUN_CI=0
usage(){ echo 'Usage: bash scripts/bootstrap-github.sh --owner GITHUB_LOGIN [--apply] [--run-preflight]'; }
while (($#)); do
  case "$1" in
    --owner) [[ $# -ge 2 ]] || { usage; exit 2; }; OWNER="$2"; shift 2;;
    --apply) APPLY=1; shift;;
    --run-preflight) RUN_CI=1; shift;;
    -h|--help) usage; exit 0;;
    *) usage; exit 2;;
  esac
done
[[ "$OWNER" =~ ^[A-Za-z0-9][A-Za-z0-9-]*$ ]] || { echo 'An explicit valid owner is required.' >&2; exit 2; }
# Do not silently substitute a different connected account for the approved owner.
EXPECTED_OWNER="$(python3 - "$ROOT/infra/environment-plan.json" <<'PY_OWNER'
import json,sys
owner=json.load(open(sys.argv[1], encoding='utf-8')).get('repository_owner')
if not owner:
    raise SystemExit('Repository owner has not been approved.')
print(owner)
PY_OWNER
)"
[[ "$OWNER" == "$EXPECTED_OWNER" ]] || {
  echo "Approved owner is $EXPECTED_OWNER, not $OWNER. No changes made." >&2
  exit 2
}
REPO="$OWNER/AgentBaton"
if (( ! APPLY )); then
  echo "DRY RUN: create private $REPO from $ROOT; no existing repository is modified."
  echo "Product development remains on hold. CI runs only with --run-preflight."
  exit 0
fi
command -v gh >/dev/null || { echo 'GitHub CLI is unavailable; no remote changes made.' >&2; exit 2; }
LOGIN="$(gh api user --jq .login)"
[[ "$LOGIN" == "$OWNER" ]] || { echo "Authenticated as $LOGIN, not requested owner $OWNER. No changes made." >&2; exit 2; }
cd "$ROOT"
python3 scripts/check_gate.py --scope preparation
if ! git rev-parse --git-dir >/dev/null 2>&1; then
  python3 scripts/verify_bootstrap_manifest.py
  git init -b main
  python3 scripts/verify_bootstrap_manifest.py --paths0 | git add --pathspec-from-file=- --pathspec-file-nul
  git -c user.name='AgentBaton Preparation' -c user.email='agentbaton-preparation@example.invalid' -c commit.gpgsign=false commit -m 'chore: prepare design baseline and environment checks; implementation on hold'
fi
[[ "$(git branch --show-current)" == main ]] || { echo 'Expected local main branch.' >&2; exit 2; }
[[ -z "$(git remote)" ]] || { echo 'Existing Git remote detected. Stop for review; not overwritten.' >&2; exit 2; }
[[ -z "$(git status --porcelain)" ]] || { echo 'Working tree has uncommitted changes. Review and commit first.' >&2; exit 2; }
# No overwrite/update path: GitHub create fails safely if the name exists.
gh repo create "$REPO" --private --source . --remote origin --push \
  --description 'AgentBaton — design and infrastructure preparation; implementation on hold'
[[ "$(gh api "repos/$REPO" --jq .private)" == true ]] || { echo 'Unexpected visibility; inspect repository immediately.' >&2; exit 3; }
echo "CREATED: $REPO. Default branch and setup files are ready for inspection."
echo 'Branch protection, Codex Cloud publishing, native GUI tests and code signing are NOT configured by this script.'
if (( RUN_CI )); then
  # Hosted runner usage can consume account minutes. Explicit flag is required.
  gh workflow run environment-preflight.yml --repo "$REPO" --ref main
  echo 'Preflight was requested, not yet verified. Review Actions status and logs.'
fi
echo 'STOP: do not launch product development without the user approval.'
