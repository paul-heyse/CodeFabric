#!/usr/bin/env bash
# Preserve native rustc incremental feedback for local check-oriented commands.

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
  # Ordinary check/Clippy units also omit `link`, so bypassing the wrapper is both
  # required and avoids cache lookup overhead on a non-cacheable workflow.
  export RUSTC_WRAPPER=
fi

exec "$@"
