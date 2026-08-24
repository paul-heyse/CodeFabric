#!/usr/bin/env bash
# Run the WP01 persisted-state and benchmark contracts at two committed revisions.
# All writes are confined to validated temporary namespaces and ignored Cargo targets.
set -euo pipefail

cd "$(dirname "$0")/.."
repo_root="$PWD"
mode="${1:-}"
baseline_ref="${2:-}"
target_ref="${3:-}"

usage() {
  printf 'usage: %s <compat|benchmark> <baseline-ref> <target-ref>\n' "$0" >&2
  exit 2
}

[ -n "$baseline_ref" ] && [ -n "$target_ref" ] || usage
case "$mode" in
  compat|benchmark) ;;
  *) usage ;;
esac

baseline_commit="$(git rev-parse --verify "${baseline_ref}^{commit}")"
target_commit="$(git rev-parse --verify "${target_ref}^{commit}")"
run_root="$(mktemp -d)"
baseline_tree="$run_root/baseline"
target_tree="$run_root/target"
baseline_target="$repo_root/target/data-fabric-revisions"
target_target="$baseline_target"

cleanup() {
  if [ -d "$target_tree" ] && [ "$target_tree" != "$baseline_tree" ]; then
    git worktree remove --force "$target_tree" >/dev/null 2>&1 || true
  fi
  if [ -d "$baseline_tree" ]; then
    git worktree remove --force "$baseline_tree" >/dev/null 2>&1 || true
  fi
  case "$run_root" in
    /tmp/*|/private/tmp/*|/var/folders/*|/private/var/folders/*) rm -rf "$run_root" ;;
    *) printf 'refusing to remove unexpected temporary path: %s\n' "$run_root" >&2 ;;
  esac
}
trap cleanup EXIT

git worktree add --detach "$baseline_tree" "$baseline_commit" >/dev/null
if [ "$target_commit" = "$baseline_commit" ]; then
  target_tree="$baseline_tree"
else
  git worktree add --detach "$target_tree" "$target_commit" >/dev/null
fi

run_fixture_mode() {
  local tree="$1" target_dir="$2" fixture_mode="$3" fixture="$4"
  (
    cd "$tree"
    CARGO_TARGET_DIR="$target_dir" \
      CODEFABRIC_CROSS_REVISION_MODE="$fixture_mode" \
      CODEFABRIC_CROSS_REVISION_FIXTURE="$fixture" \
      cargo test --locked --test integration \
        integration::data_fabric_upgrade::data_fabric_cross_revision_fixture_mode \
        -- --exact
  )
}

fixture_digest() {
  local fixture="$1"
  find "$fixture" -type f -print0 \
    | LC_ALL=C sort -z \
    | xargs -0 shasum -a 256 \
    | shasum -a 256 \
    | awk '{print $1}'
}

run_compat() {
  local old_fixture="$run_root/old-produced"
  local target_fixture="$run_root/target-produced"

  run_fixture_mode "$baseline_tree" "$baseline_target" produce "$old_fixture"
  local old_before old_after
  old_before="$(fixture_digest "$old_fixture")"
  run_fixture_mode "$target_tree" "$target_target" consume "$old_fixture"
  old_after="$(fixture_digest "$old_fixture")"
  [ "$old_before" = "$old_after" ] || {
    printf 'target reader changed the old-stack namespace\n' >&2
    exit 1
  }

  run_fixture_mode "$target_tree" "$target_target" produce "$target_fixture"
  local target_before target_after
  target_before="$(fixture_digest "$target_fixture")"
  # This is deliberately consume-only: the old revision never writes the target namespace.
  run_fixture_mode "$baseline_tree" "$baseline_target" consume "$target_fixture"
  target_after="$(fixture_digest "$target_fixture")"
  [ "$target_before" = "$target_after" ] || {
    printf 'old reader changed the target-stack namespace\n' >&2
    exit 1
  }

  printf 'data-fabric stack compatibility passed: %s -> %s and read-only %s -> %s\n' \
    "$baseline_commit" "$target_commit" "$target_commit" "$baseline_commit"
}

emit_benchmark() {
  local tree="$1" target_dir="$2" output="$3"
  cp "$repo_root/tooling/data_fabric_revision_benchmark.rs" \
    "$tree/tests/data_fabric_revision_benchmark.rs"
  (
    cd "$tree"
    CARGO_TARGET_DIR="$target_dir" \
      CODEFABRIC_BENCHMARK_REPORT="$output" \
      cargo test --locked --test data_fabric_revision_benchmark \
        data_fabric_revision_benchmark_emit \
        -- --exact
  )
}

run_benchmark() {
  local baseline_report="$run_root/baseline-benchmark.json"
  local target_report="$run_root/target-benchmark.json"
  emit_benchmark "$baseline_tree" "$baseline_target" "$baseline_report"
  if [ "$target_commit" = "$baseline_commit" ]; then
    cp "$baseline_report" "$target_report"
  else
    emit_benchmark "$target_tree" "$target_target" "$target_report"
  fi
  (
    cd "$target_tree"
    CARGO_TARGET_DIR="$target_target" \
      CODEFABRIC_BENCHMARK_BASELINE="$baseline_report" \
      CODEFABRIC_BENCHMARK_TARGET="$target_report" \
      cargo test --locked --test integration \
        integration::data_fabric_upgrade::wp06_operational_performance_rollback \
        -- --exact
  )
  jq -n \
    --arg baseline_commit "$baseline_commit" \
    --arg target_commit "$target_commit" \
    --slurpfile baseline "$baseline_report" \
    --slurpfile target "$target_report" \
    '{baseline_commit: $baseline_commit, target_commit: $target_commit,
      baseline: $baseline[0], target: $target[0]}'
}

case "$mode" in
  compat) run_compat ;;
  benchmark) run_benchmark ;;
esac
