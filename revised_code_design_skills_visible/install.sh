#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 [--force] /path/to/repository" >&2
  exit 2
}

force=0
if [[ ${1:-} == "--force" ]]; then
  force=1
  shift
fi
[[ $# -eq 1 ]] || usage

repo=$(cd "$1" && pwd)
src=$(cd "$(dirname "$0")" && pwd)/skills
dst="$repo/.claude/skills"

[[ -d "$src" ]] || { echo "Missing source directory: $src" >&2; exit 1; }
mkdir -p "$repo/.claude"

if [[ -e "$dst" && $force -ne 1 ]]; then
  echo "Refusing to overwrite existing $dst. Re-run with --force after reviewing backups." >&2
  exit 1
fi

if [[ -e "$dst" ]]; then
  backup="$dst.backup.$(date +%Y%m%d-%H%M%S)"
  mv "$dst" "$backup"
  echo "Existing skills moved to: $backup"
fi

cp -R "$src" "$dst"
echo "Installed revised skills at: $dst"
find "$dst" -type f -name SKILL.md -print | sort
