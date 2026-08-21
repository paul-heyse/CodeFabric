#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
normative_pattern='contracts/fixtures/(jcs/vectors|projections/vectors|proto/wave0_probe)\.json'

if rg -n "$normative_pattern" \
  "$repository_root/justfile" "$repository_root/scripts" \
  -g '*.sh' | rg -n '(write|accept|update|generate)' >/dev/null; then
  echo "a gate or accept path appears able to write a normative KAT" >&2
  exit 1
fi

if rg -n 'fixture-candidates' "$repository_root/justfile" | rg -n '^[^#]*:.*(ci-fast|governance|contracts-verify)' >/dev/null; then
  echo "fixture-candidates must not be a gate dependency" >&2
  exit 1
fi

echo "normative KAT paths have no generator or gate write path"
