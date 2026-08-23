#!/usr/bin/env bash
# Exercise current-tree, fallback, Git-state, and linked-worktree repository modeling.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
task_tmp_base="${TMPDIR:-/tmp}"
task_tmp_base="${task_tmp_base%/}"
fixture_root="$(mktemp -d "$task_tmp_base/codefabric-model-inventory.XXXXXX")"
linked_parent="$(mktemp -d "$task_tmp_base/codefabric-model-linked.XXXXXX")"
linked_root="$linked_parent/worktree"
conflict_root="$(mktemp -d "$task_tmp_base/codefabric-model-conflict.XXXXXX")"

cleanup() {
  git -C "$fixture_root" worktree remove --force "$linked_root" >/dev/null 2>&1 || true
  for path in "$fixture_root" "$linked_parent" "$conflict_root"; do
    case "$path" in
      "$task_tmp_base"/codefabric-model-inventory.*|\
      "$task_tmp_base"/codefabric-model-linked.*|\
      "$task_tmp_base"/codefabric-model-conflict.*)
        find "$path" -depth -delete 2>/dev/null || true
        ;;
      *)
        printf 'refusing to clean unexpected inventory fixture: %s\n' "$path" >&2
        ;;
    esac
  done
}
trap cleanup EXIT

fail() {
  printf 'model inventory check failed: %s\n' "$1" >&2
  exit 1
}

model() {
  "$repo_root/scripts/model_exec.sh" "$@"
}

write_authority() {
  local path="$1" id="$2"
  mkdir -p "$(dirname "$path")"
  printf 'artifact_id: %s\nartifact_kind: yaml-contract\nversion: "1.0"\ncompatible_suite_major: 1\nstatus: released\n' \
    "$id" > "$path"
}

initialize_repository() {
  local root="$1"
  git -C "$root" init -q
  git -C "$root" config user.name 'CodeFabric Model Test'
  git -C "$root" config user.email 'codefabric-model@example.invalid'
}

model_gix_failure_falls_back_without_semantic_drift() {
  local accelerated fallback
  accelerated="$(model inventory "$repo_root")"
  fallback="$(model inventory --no-gix "$repo_root")"
  jq -e '.summary.topology.git_available and .summary.diagnostic_count == 0 and .shadow.missing_paths == []' \
    <<<"$accelerated" >/dev/null || fail 'live accelerated model or shadow parity is invalid'
  [ "$(jq -r '.summary.semantic_digest' <<<"$accelerated")" = \
    "$(jq -r '.summary.semantic_digest' <<<"$fallback")" ] || \
    fail 'gix and filesystem fallback changed semantic model identity'
  jq -e '.summary.topology.git_available | not' <<<"$fallback" >/dev/null || \
    fail 'fallback model incorrectly reports Git acceleration'
}

model_inventory_classifies_tracked_staged_untracked_and_ignored() {
  initialize_repository "$fixture_root"
  printf '%s\n' 'contracts/identity/ignored.yaml' > "$fixture_root/.gitignore"
  for name in tracked staged deleted; do
    write_authority \
      "$fixture_root/contracts/identity/$name.yaml" \
      "codefabric.identity.$name"
  done
  git -C "$fixture_root" add .
  git -C "$fixture_root" commit -qm fixture

  printf 'note: staged\n' >> "$fixture_root/contracts/identity/staged.yaml"
  git -C "$fixture_root" add contracts/identity/staged.yaml
  write_authority \
    "$fixture_root/contracts/identity/untracked.yaml" \
    'codefabric.identity.untracked'
  write_authority \
    "$fixture_root/contracts/identity/ignored.yaml" \
    'codefabric.identity.ignored'
  mv "$fixture_root/contracts/identity/deleted.yaml" "$fixture_root/deleted.yaml.backup"

  local summary
  summary="$(model inventory "$fixture_root")"
  jq -e '
    .summary.classifications.tracked > 0
    and .summary.classifications.staged > 0
    and .summary.classifications.untracked > 0
    and .summary.classifications.ignored > 0
    and .summary.classifications.deleted > 0
  ' <<<"$summary" >/dev/null || fail 'Git state census omitted a required state'

  initialize_repository "$conflict_root"
  mkdir -p "$conflict_root/src/generated"
  printf 'pub const VALUE: &str = "base";\n' > "$conflict_root/src/generated/conflict.rs"
  git -C "$conflict_root" add .
  git -C "$conflict_root" commit -qm base
  git -C "$conflict_root" checkout -qb conflict-side
  printf 'pub const VALUE: &str = "side";\n' > "$conflict_root/src/generated/conflict.rs"
  git -C "$conflict_root" commit -qam side
  git -C "$conflict_root" checkout -q master
  printf 'pub const VALUE: &str = "main";\n' > "$conflict_root/src/generated/conflict.rs"
  git -C "$conflict_root" commit -qam main
  git -C "$conflict_root" merge conflict-side >/dev/null 2>&1 || true
  jq -e '.summary.classifications.conflicted > 0' \
    <<<"$(model inventory "$conflict_root")" >/dev/null || \
    fail 'conflicted index stages were not classified'
}

model_linked_worktree_inventory_uses_current_bytes() {
  git -C "$fixture_root" worktree add -qb fixture-linked "$linked_root"
  printf 'note: linked-current-bytes\n' >> "$linked_root/contracts/identity/tracked.yaml"

  local accelerated fallback main_explanation linked_explanation
  accelerated="$(model inventory "$linked_root")"
  fallback="$(model inventory --no-gix "$linked_root")"
  jq -e '.summary.topology.linked_worktree and .summary.topology.git_available' \
    <<<"$accelerated" >/dev/null || fail 'linked worktree topology was not detached correctly'
  [ "$(jq -r '.summary.semantic_digest' <<<"$accelerated")" = \
    "$(jq -r '.summary.semantic_digest' <<<"$fallback")" ] || \
    fail 'linked-worktree acceleration changed current-byte semantics'

  main_explanation="$(model explain contracts/identity/tracked.yaml "$fixture_root")"
  linked_explanation="$(model explain contracts/identity/tracked.yaml "$linked_root")"
  [ "$(jq -r '.model[0].claim.source_digest' <<<"$main_explanation")" != \
    "$(jq -r '.model[0].claim.source_digest' <<<"$linked_explanation")" ] || \
    fail 'linked worktree did not use its current source bytes'
}

model_gix_failure_falls_back_without_semantic_drift
model_inventory_classifies_tracked_staged_untracked_and_ignored
model_linked_worktree_inventory_uses_current_bytes

printf 'model inventory check passed\n'
