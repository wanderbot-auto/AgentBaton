#!/usr/bin/env bash
# Local coordination for worktrees that share this repository's Git directory.
set -euo pipefail

die() { printf 'collab: %s\n' "$*" >&2; exit 2; }
usage() {
  cat <<'USAGE'
Usage (run from the repository or a task worktree):
  collab.sh new ID origin/BASE PATH...  create collab/ID and reserve literal paths
  collab.sh list                       list worktrees and active reservations
  collab.sh scope                      check committed and local edits against this task's paths
  collab.sh preview BASE [HEAD]         simulate merging two committed refs
  collab.sh rebase --private            rebase a clean, unpublished, unshared task
  collab.sh finish --merged|--handoff   release paths after merge or documented handoff

Fetch the intended base before new, scope, preview or rebase. No command pushes.
USAGE
}
[[ ${1:-} == -h || ${1:-} == --help || $# == 0 ]] && { usage; exit 0; }
cmd=$1; shift
top=$(git rev-parse --show-toplevel) || die 'run this command inside the intended repository'
common=$(git rev-parse --path-format=absolute --git-common-dir)
root=$(dirname "$common")
[[ "$common" == "$root/.git" && -d "$common" ]] || die 'requires a regular checkout with a .git directory'
cd "$top"
state="$common/agentbaton-collab"
active="$state/active"
trees="$root/artifacts/collab-worktrees"

commit() { git rev-parse --verify --end-of-options "$1^{commit}"; }
base_commit() {
  [[ "$1" == origin/* ]] && git check-ref-format "refs/remotes/$1" ||
    die 'base must be an explicit origin/<branch>'
  commit "refs/remotes/$1"
}
take_lock() {
  mkdir -p "$active" "$state/closed"
  mkdir "$state/lock" 2>/dev/null ||
    die "another task is registering or finishing (or recovery is needed): $state/lock"
  trap 'rmdir "$state/lock"' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
}
read_task() {
  branch=$(git symbolic-ref --quiet --short HEAD) || die 'detached HEAD'
  [[ "$branch" == collab/* ]] || die 'run from a registered collab/<id> worktree'
  id=${branch#collab/}
  [[ "$top" == "$trees/$id" && -f "$active/$id" ]] || die 'no active reservation for this worktree'
  claim="$active/$id"
  base=$(sed -n '1p' "$claim")
  initial=$(sed -n '2p' "$claim")
}
task_base() {
  tip=$(base_commit "$base") || die "cannot resolve $base"
  git merge-base --is-ancestor "$initial" "$tip" ||
    die 'base history changed; review the PR stack and plan a new branch or an explicit --onto'
  git merge-base --is-ancestor "$initial" HEAD ||
    die 'task history no longer includes its recorded starting commit'
}
require_idle() {
  for marker in rebase-merge rebase-apply MERGE_HEAD CHERRY_PICK_HEAD REVERT_HEAD sequencer BISECT_START; do
    [[ ! -e "$(git rev-parse --git-path "$marker")" ]] || die "complete or abort the current Git operation ($marker)"
  done
  status=$(git status --porcelain=v1 --untracked-files=all)
  [[ -z "$status" ]] || die 'worktree has local changes; review and checkpoint them first'
}
fold() { printf '%s' "$1" | LC_ALL=C tr '[:upper:]' '[:lower:]'; }
within() { [[ "$1" == "$2" || "$1" == "$2/"* ]]; }

case "$cmd" in
  new)
    [[ $# -ge 3 ]] || die 'new requires ID, origin/BASE and one or more paths'
    id=$1; base=$2; shift 2
    [[ "$id" =~ ^[a-z0-9][a-z0-9-]*$ ]] || die 'use a lowercase task ID with letters, digits and hyphens'
    for path in "$@"; do
      [[ "$path" =~ ^[a-zA-Z0-9_.-]+(/[a-zA-Z0-9_.-]+)*$ &&
         "/$path/" != *'/../'* && "/$path/" != *'/./'* ]] ||
        die "use literal repository-relative paths with no trailing slash: $path"
      key=$(fold "$path")
      [[ "$key" != .git && "$key" != .git/* && "$key" != artifacts && "$key" != artifacts/* ]] ||
        die "path is reserved for repository metadata or local artifacts: $path"
    done
    tip=$(base_commit "$base") || die "cannot resolve $base; fetch the intended branch first"
    take_lock
    [[ ! -e "$active/$id" && ! -e "$state/closed/$id" && ! -e "$trees/$id" && ! -L "$trees/$id" ]] ||
      die "task ID or worktree already exists: $id"
    if git show-ref --verify --quiet "refs/heads/collab/$id"; then die "branch already exists: collab/$id"; fi
    for claim in "$active"/*; do
      [[ -f "$claim" ]] || continue
      while IFS= read -r reserved; do
        for path in "$@"; do
          a=$(fold "$path"); b=$(fold "$reserved")
          if within "$a" "$b" || within "$b" "$a"; then
            die "$path overlaps $reserved (task ${claim##*/}); narrow the work or agree on a handoff"
          fi
        done
      done < <(tail -n +3 "$claim")
    done
    mkdir -p "$trees"
    printf '%s\n' "$base" "$tip" "$@" > "$active/$id"
    git worktree add --no-track -b "collab/$id" "$trees/$id" "$tip" ||
      die "worktree creation did not finish; reservation retained for recovery: $active/$id"
    printf 'Worktree: %s\nBranch: collab/%s\nBase: %s %s\n' "$trees/$id" "$id" "$base" "$tip"
    ;;
  list)
    [[ $# == 0 ]] || die 'list takes no arguments'
    git worktree list
    for claim in "$active"/*; do
      [[ -f "$claim" ]] || continue
      printf '\n[%s] base, starting SHA, reserved paths:\n' "${claim##*/}"
      cat "$claim"
    done
    ;;
  scope)
    [[ $# == 0 ]] || die 'scope takes no arguments'
    read_task
    task_base
    ancestor=$(git merge-base "$tip" HEAD)
    # Keep both sides of renames in scope. NUL delimiters preserve unusual filenames.
    paths=$(mktemp -t ab-collab-scope.XXXXXXXX)
    trap 'rm -- "$paths"' EXIT
    git diff --no-renames --name-only -z "$ancestor" HEAD > "$paths"
    git diff --no-renames --name-only -z HEAD >> "$paths"
    git diff --cached --no-renames --name-only -z HEAD >> "$paths"
    git ls-files --others --exclude-standard -z >> "$paths"
    outside=0
    while IFS= read -r -d '' path; do
      covered=0
      while IFS= read -r reserved; do
        if within "$path" "$reserved"; then covered=1; break; fi
      done < <(tail -n +3 "$claim")
      if [[ "$covered" == 0 ]]; then printf 'Outside task scope: %q\n' "$path" >&2; outside=1; fi
    done < "$paths"
    [[ "$outside" == 0 ]] || exit 1
    printf 'Scope OK against merge base %s.\n' "$ancestor"
    ;;
  preview)
    [[ $# == 1 || $# == 2 ]] || die 'preview requires BASE and optionally a source ref'
    left=$(commit "$1"); right=$(commit "${2:-HEAD}")
    printf 'Previewing target %s + source %s (committed snapshots only).\n' "$left" "$right"
    if git merge-tree --write-tree --name-only "$left" "$right"; then
      printf 'No Git merge conflict. Run tests; rebase can still conflict while replaying commits.\n'
    else
      status=$?
      if [[ "$status" == 1 ]]; then printf 'Git merge conflict; resolve in the source worktree.\n' >&2; fi
      exit "$status"
    fi
    ;;
  rebase)
    [[ $# == 1 && "$1" == --private ]] || die 'rebase requires --private: unpublished branch, no downstream users'
    read_task
    require_idle
    task_base
    [[ -z "$(git rev-list --merges "$tip..HEAD")" ]] || die 'merge commits require an explicit integration plan'
    [[ -z "$(git for-each-ref --format='%(refname)' "refs/remotes/*/$branch")" ]] ||
      die 'branch has a remote tracking ref; use a regular base merge for published branches'
    published=$(git ls-remote --heads origin "refs/heads/$branch") || die 'cannot verify publish status on origin'
    [[ -z "$published" ]] || die 'branch exists on origin; use a regular base merge'
    if git merge-base --is-ancestor "$tip" HEAD; then
      printf 'Already based on %s (%s).\n' "$base" "$tip"
      exit 0
    fi
    before=$(commit HEAD)
    git update-ref "refs/agentbaton-collab-backup/$id/$before" "$before"
    printf 'Backup: refs/agentbaton-collab-backup/%s/%s\n' "$id" "$before"
    if git -c rebase.updateRefs=false -c rebase.autoStash=false -c rerere.enabled=false \
        rebase --no-fork-point --no-rebase-merges "$tip"; then
      printf 'Rebased on %s; rerun scope, preview and affected tests.\n' "$tip"
    else
      status=$?
      printf 'Inspect git status in this worktree. Resolve deliberately and continue, or git rebase --abort.\n' >&2
      exit "$status"
    fi
    ;;
  finish)
    [[ $# == 1 && ( "$1" == --merged || "$1" == --handoff ) ]] ||
      die 'finish requires --merged or --handoff; confirm the handoff before releasing ownership'
    read_task
    require_idle
    take_lock
    [[ -f "$claim" && ! -e "$state/closed/$id" ]] || die "reservation changed before release: $id"
    mv "$claim" "$state/closed/$id"
    printf 'Released paths for %s; kept the worktree and branch.\n' "$id"
    ;;
  *) usage >&2; die "unknown command: $cmd" ;;
esac
