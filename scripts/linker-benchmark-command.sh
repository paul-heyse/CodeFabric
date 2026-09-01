#!/usr/bin/env bash
# One controlled relink sample with dependencies retained in an isolated target tree.

set -euo pipefail

[ "$#" -ge 3 ] || { printf 'usage: %s <target-dir> {default|mold} <package> [cargo build args...]\n' "$0" >&2; exit 64; }
target_dir="$1"
linker_mode="$2"
package_name="$3"
shift 3
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"

case "$target_dir" in
  "$repo_root"/target/agent/linker-benchmark/*/cargo-target-*) ;;
  *) printf 'refusing to clean unexpected benchmark target: %s\n' "$target_dir" >&2; exit 1 ;;
esac

RUSTC_WRAPPER= CARGO_INCREMENTAL=0 \
  "$repo_root/scripts/cargo" clean --target-dir "$target_dir" -p "$package_name"

case "$linker_mode" in
  default) rustflags= ;;
  mold) rustflags='-C link-arg=-fuse-ld=mold' ;;
  *) printf 'unknown linker mode: %s\n' "$linker_mode" >&2; exit 64 ;;
esac

RUSTC_WRAPPER= CARGO_INCREMENTAL=0 RUSTFLAGS="$rustflags" \
  "$repo_root/scripts/cargo" build --locked --target-dir "$target_dir" --bins "$@"
