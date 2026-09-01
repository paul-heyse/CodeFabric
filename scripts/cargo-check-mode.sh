#!/usr/bin/env bash
# Preserve native rustc incremental feedback for explicit local edit-loop commands.

set -euo pipefail

if [ "$#" -lt 1 ]; then
  printf 'usage: %s <cargo-or-command> [args...]\n' "$0" >&2
  exit 64
fi

if [ -n "${CI:-}" ]; then
  export CARGO_INCREMENTAL=0
else
  export CARGO_INCREMENTAL=1
  # sccache 0.17.0 exits rather than passing through an incremental Rust invocation.
  # Check/Clippy units omit `link`, while focused test builds benefit from preserving
  # the local incremental graph. Neither path is terminal reproducibility evidence.
  export RUSTC_WRAPPER=
fi

exec "$@"
