#!/usr/bin/env bash
# Launch the already-built rustc_private extractor with its pinned sysroot libraries.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
extractor_root="${root}/rustc-extractor"
binary="${root}/target/extractor/debug/codefabric-rustc-extractor"
test -x "$binary" || {
  printf 'extractor binary is absent; run just extractor-check first\n' >&2
  exit 1
}

# Cargo exports its invoking toolchain to child processes. A stable-root daemon
# therefore carries `RUSTUP_TOOLCHAIN=stable` even after changing into the
# extractor directory, which would defeat the extractor's dated-nightly file.
# Resolve the governed compiler-private sysroot explicitly.
sysroot="$(rustup run nightly-2026-08-18 rustc --print sysroot)"
case "$(uname -s)" in
  Darwin)
    export DYLD_LIBRARY_PATH="${sysroot}/lib${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
    ;;
  Linux)
    export LD_LIBRARY_PATH="${sysroot}/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
    ;;
  *)
    printf 'unsupported extractor launch platform: %s\n' "$(uname -s)" >&2
    exit 1
    ;;
esac

exec "$binary" "$@"
