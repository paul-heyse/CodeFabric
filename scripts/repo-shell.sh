#!/usr/bin/env bash
# Clean, repository-owned shell boundary for every linewise Just recipe.

set -euo pipefail

if [ "$#" -lt 1 ]; then
  printf 'usage: repo-shell.sh [shell-options] <command>\n' >&2
  exit 64
fi

# Just passes the recipe line, recipe name for `$0`, and positional recipe arguments.
# Direct compatibility callers may prepend shell options before the command.
case "${1:-}" in
  -*)
    [ "$#" -ge 2 ] || { printf 'missing command after %s\n' "$1" >&2; exit 64; }
    command_text="$2"
    shell_name="${3:-repo-shell}"
    if [ "$#" -ge 4 ]; then
      command_arguments=("${@:4}")
    else
      command_arguments=()
    fi
    ;;
  *)
    command_text="$1"
    shell_name="${2:-repo-shell}"
    if [ "$#" -ge 3 ]; then
      command_arguments=("${@:3}")
    else
      command_arguments=()
    fi
    ;;
esac

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
export CF_ROOT="$repo_root"

# Repository commands never inherit another Python environment, an ambient Rust
# toolchain override, or caller-owned build output locations. Recipes may set a
# value again after this boundary when that value is part of their contract.
unset VIRTUAL_ENV UV_PROJECT_ENVIRONMENT PYTHONPATH CONDA_PREFIX
unset RUSTUP_TOOLCHAIN RUSTC_WRAPPER CARGO_TARGET_DIR

while IFS= read -r variable_name; do
  unset "$variable_name"
done < <(compgen -A variable | LC_ALL=C sort | sed -n '/^CONDA_/p; /^DIRENV_/p')

# Remove the adapter venv inherited by an already-running app, then make the
# repository's Rustup/Cargo router and user-installed tools win over Homebrew.
clean_path=""
old_ifs="$IFS"
IFS=:
for path_entry in ${PATH:-}; do
  case "$path_entry" in
    "$repo_root/scripts" | "$HOME/.cargo/bin" | "$repo_root/codefabric-cpg-mcp/.venv/bin")
      continue
      ;;
  esac
  if [ -n "$path_entry" ]; then
    clean_path="${clean_path:+${clean_path}:}${path_entry}"
  fi
done
IFS="$old_ifs"
export PATH="$repo_root/scripts:$HOME/.cargo/bin:$HOME/.local/bin${clean_path:+:$clean_path}"

export UV_CACHE_DIR="$repo_root/target/uv-cache"
mkdir -p "$UV_CACHE_DIR"

# Just recipes are the reproducible shared-reuse boundary. Compile-producing recipes and
# the scripts they invoke therefore emit non-incremental outputs that sccache can reuse.
# The explicit cargo-check-mode helper restores local incremental feedback for check and
# Clippy while preserving CARGO_INCREMENTAL=0 in CI. Raw Cargo outside Just keeps Cargo's
# normal profile policy.
export CARGO_INCREMENTAL=0

exec /bin/bash -euo pipefail -c "$command_text" "$shell_name" "${command_arguments[@]}"
