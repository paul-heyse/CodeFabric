#!/usr/bin/env bash
# Codex SessionStart hook — CodeFabric environment report.
#
# Mirrors the Claude Code SessionStart hook in .claude/settings.json: both run
# scripts/bootstrap.sh --context and inject the result as additional context, so
# a session opens knowing the toolchain state without probing for it.
#
# Contract (Codex hooks reference): read the hook payload from stdin, write hook
# JSON to stdout. `hookSpecificOutput.additionalContext` is added as developer
# context. Plain stdout is also accepted, which is the fallback path below.
#
# Never fail the session: any error exits 0 with no output.

set -uo pipefail

# Codex runs hooks with the session cwd as the working directory. Resolve the
# repo root from this script's own location so the hook also works when Codex is
# started from a subdirectory.
here="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
repo_root="$(cd "${here}/../.." && pwd)"
bootstrap="${repo_root}/scripts/bootstrap.sh"

[ -x "$bootstrap" ] || exit 0

context="$("$bootstrap" --context 2>/dev/null)" || exit 0
[ -n "$context" ] || exit 0

if command -v python3 >/dev/null 2>&1; then
  CF_CONTEXT="$context" python3 -c '
import json, os
print(json.dumps({"hookSpecificOutput": {
    "hookEventName": "SessionStart",
    "additionalContext": os.environ["CF_CONTEXT"],
}}))' 2>/dev/null || printf '%s\n' "$context"
else
  # Plain stdout is accepted as additionalContext.
  printf '%s\n' "$context"
fi
