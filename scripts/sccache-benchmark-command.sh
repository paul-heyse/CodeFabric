#!/usr/bin/env bash
# One hyperfine sample: cold Cargo target, preserved sccache storage.

set -euo pipefail

[ "$#" -ge 2 ] || { printf 'usage: %s <sccache-config> <target-dir> [cargo build args...]\n' "$0" >&2; exit 64; }
benchmark_config="$1"
target_dir="$2"
shift 2

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
case "$target_dir" in
  "$repo_root"/target/agent/sccache-effectiveness/*/cargo-target-*) ;;
  *) printf 'refusing to clear unexpected benchmark target: %s\n' "$target_dir" >&2; exit 1 ;;
esac
rm -rf -- "$target_dir"

CODEFABRIC_SCCACHE_CONF="$benchmark_config" \
  CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR="$target_dir" \
  "$repo_root/scripts/cargo" build --locked "$@"
