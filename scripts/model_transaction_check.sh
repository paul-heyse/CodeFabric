#!/usr/bin/env bash
# Prove worktree-local locking, crash recovery, and exact model reconciliation semantics.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"

cargo nextest run \
  --manifest-path "$repo_root/Cargo.toml" \
  --locked \
  --no-default-features \
  --features model-compiler \
  --bin codefabric-model \
  -E 'test(transaction::tests)'

rg -n 'transaction::read_guard' "$repo_root/src/bin/codefabric_model/main.rs" >/dev/null
if rg -n 'consumer-overlays|PATCH_PATH|join\("tooling/model-transition' \
  "$repo_root/src/bin/codefabric_model" \
  "$repo_root/tooling/model" \
  -g '!target/**'; then
  printf 'supported model runtime still reads the one-time transition root\n' >&2
  exit 1
fi

printf 'model transaction check passed\n'
