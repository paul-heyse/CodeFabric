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

sysroot="$(cd "$extractor_root" && rustc --print sysroot)"
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
