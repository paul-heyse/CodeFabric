#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: run_fastmcp4_modern_client.sh <installed-wheel-python> <scenario.json|->" >&2
  exit 2
fi

installed_python="$1"
scenario="$2"

if [[ "$installed_python" != /* || ! -x "$installed_python" ]]; then
  echo "installed-wheel-python must be an absolute executable path" >&2
  exit 2
fi
if [[ "$scenario" != "-" && ! -f "$scenario" ]]; then
  echo "scenario must be a readable JSON file or -" >&2
  exit 2
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

unset PYTHONHOME PYTHONPATH VIRTUAL_ENV CONDA_PREFIX CONDA_DEFAULT_ENV
export PYTHONNOUSERSITE=1

exec "$installed_python" -I \
  "$repository_root/tooling/fastmcp4_modern_client_driver.py" \
  "$scenario"
