#!/usr/bin/env bash
#
# Clean-wheel validation (spec section 45).
#
# A development install is not packaging evidence. This script proves that the artifact
# in dist/ installs and imports on its own, in an environment that has never seen this
# repository's source tree (spec sections 44.2 and 62.3).
#
# Implements all seven semantics from section 45. Two of them are gaps in the
# specification's own shell example, closed here:
#   - it takes the first of however many wheels it finds; this requires exactly one, so a
#     stale artifact from an earlier build cannot silently become the thing under test;
#   - it prints neither the Python version nor the wheel filename, which section 45 item
#     6 requires as part of the record.
#
# Invoke through `just wheel-test`, which builds a fresh wheel first.

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

# 1. Require exactly the intended freshly built wheel.
shopt -s nullglob
wheels=(dist/*.whl)
shopt -u nullglob

if [ "${#wheels[@]}" -eq 0 ]; then
  echo "wheel_test: no wheel in dist/ -- run 'just wheel' first" >&2
  exit 1
fi
if [ "${#wheels[@]}" -gt 1 ]; then
  echo "wheel_test: expected exactly one wheel in dist/, found ${#wheels[@]}:" >&2
  printf '  %s\n' "${wheels[@]}" >&2
  echo "wheel_test: refusing to guess which artifact is under test" >&2
  exit 1
fi
wheel="${wheels[0]}"

# 2. Create a temporary isolated virtual environment.
tmp="$(mktemp -d)"
if [ "${WHEEL_TEST_KEEP:-}" = "1" ]; then
  # 7. ...unless explicitly retained for debugging.
  trap 'echo "wheel_test: retained ${tmp}"' EXIT
else
  trap 'rm -rf "$tmp"' EXIT
fi

interpreter="$(uv run python -c 'import sys; print(sys.executable)')"
uv venv --quiet --python "$interpreter" "$tmp/venv"

# 3. Install that wheel, not the repository source tree.
uv pip install --quiet --python "$tmp/venv/bin/python" "$wheel" pytest

# 6. Record what was actually tested.
echo "wheel_test: wheel      $(basename "$wheel")"
echo "wheel_test: sha256     $(shasum -a 256 "$wheel" | awk '{print $1}')"
echo "wheel_test: python     $("$tmp/venv/bin/python" --version 2>&1)"
echo "wheel_test: env        $tmp/venv"

# 5. Confirm the import resolves inside the temporary environment, not the source tree.
#
# Both paths are resolved before comparison: on macOS `mktemp -d` yields /var/folders/...
# while Path.resolve() yields /private/var/folders/..., and an unresolved comparison
# would fail even on a correct install.
WHEEL_TEST_ROOT="$tmp/venv" PYTHONPATH= "$tmp/venv/bin/python" - <<'PY'
import os
import pathlib

import codefabric

path = pathlib.Path(codefabric.__file__).resolve()
root = pathlib.Path(os.environ["WHEEL_TEST_ROOT"]).resolve()
print(f"wheel_test: imported  {path}")
if not path.is_relative_to(root):
    raise SystemExit(
        f"wheel_test: import escaped the test environment\n"
        f"  imported from: {path}\n"
        f"  expected under: {root}\n"
        f"  the repository source tree or a stale editable install shadowed the wheel"
    )
PY

# 4. Run the public-API suite against the installed package.
PYTHONPATH= "$tmp/venv/bin/python" -m pytest python_tests -q
