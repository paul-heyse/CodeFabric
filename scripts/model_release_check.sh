#!/usr/bin/env bash
# Exhaustive read-only certification of the current model-owned repository tree.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

repository_identity() {
  {
    git status --porcelain=v1 --untracked-files=all
    git diff --binary --no-ext-diff
    git ls-files --others --exclude-standard -z \
      | LC_ALL=C sort -z \
      | while IFS= read -r -d '' path; do
          shasum -a 256 "$path"
        done
  } | shasum -a 256 | awk '{print $1}'
}

before="$(repository_identity)"

# The transaction reader performs exclusive recovery before compiling the read-only plan.
reconciliation="$($repo_root/scripts/model_exec.sh reconcile-check "$repo_root")"
jq -e '
  .added == 0
  and .replaced == 0
  and .deleted_stale == 0
  and .transaction_applied == false
  and .unchanged > 0
' <<<"$reconciliation" >/dev/null || {
  printf 'model release check requires a zero-action DesiredTree: %s\n' \
    "$reconciliation" >&2
  exit 1
}

just model-bootstrap-check
just model-zero-state-check
CODEFABRIC_MODEL_CACHE_MODE=disabled \
  "$repo_root/scripts/model_exec.sh" check release --root "$repo_root" >/dev/null
CODEFABRIC_MODEL_CACHE_MODE=disabled just _model-profile-release

after="$(repository_identity)"
[ "$before" = "$after" ] || {
  printf 'model release check changed the repository worktree\n' >&2
  exit 1
}

printf 'model release check passed: %s\n' \
  "$(jq -r '.desired_tree_identity' <<<"$reconciliation")"
