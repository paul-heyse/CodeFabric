#!/usr/bin/env bash
# Cargo compiler wrapper for the mandatory CodeFabric sccache service.

set -euo pipefail
unset SCCACHE_BASEDIRS

# Cargo persists compiler-discovery output, including failures, in
# target/.rustc_info.json. A transient cache-service failure during one of these
# non-compilation queries can therefore remain replayable after the service is repaired.
# Run informational queries directly through rustc before consulting any sccache state;
# they produce no reusable compiler artifact and do not belong on the cache path.
compiler_argument_index=0
for argument in "$@"; do
  if [ "$compiler_argument_index" -eq 0 ]; then
    compiler_argument_index=1
    continue
  fi
  case "$argument" in
    -V | -vV | --version | --print | --print=*) exec "$@" ;;
  esac
done

# Cargo's profile default can request incremental compilation even when no
# CARGO_INCREMENTAL variable is present. sccache 0.17.0 rejects those invocations rather
# than compiling them uncached. Local incremental work is an explicit no-wrapper mode, so
# identify it before consulting the supervised service. CI remains cache-eligible and
# fail-closed because its contract sets CARGO_INCREMENTAL=0.
if [ -z "${CI:-}" ] && [ "${SCCACHE_GHA_ENABLED:-}" != "true" ]; then
  if [ "${CARGO_INCREMENTAL:-}" = "1" ]; then
    exec "$@"
  fi
  previous_argument=""
  for argument in "$@"; do
    if [ "$previous_argument" = "-C" ] && [[ "$argument" = incremental=* ]]; then
      exec "$@"
    fi
    case "$argument" in
      -Cincremental=*) exec "$@" ;;
    esac
    previous_argument="$argument"
  done
fi

if [ -n "${CI:-}" ] || [ "${SCCACHE_GHA_ENABLED:-}" = "true" ]; then
  unset SCCACHE_SERVER_UDS SCCACHE_CONF SCCACHE_ERROR_LOG
  export SCCACHE_CLIENT_SIDE="${SCCACHE_CLIENT_SIDE:-1}"
  sccache_bin="${SCCACHE_PATH:-$(command -v sccache || true)}"
else
  repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
  # shellcheck source=sccache-paths.sh
  source "$repo_root/scripts/sccache-paths.sh"
  export SCCACHE_SERVER_UDS="$CF_SCCACHE_SOCKET_FILE"
  default_sccache_conf="$CF_SCCACHE_CONFIG_FILE"
  if [ -n "${CODEFABRIC_SCCACHE_CONF:-}" ]; then
    export SCCACHE_CONF="$CODEFABRIC_SCCACHE_CONF"
    unset SCCACHE_CLIENT_SIDE
  else
    export SCCACHE_CONF="$default_sccache_conf"
    export SCCACHE_CLIENT_SIDE="${SCCACHE_CLIENT_SIDE:-1}"
  fi
  unset SCCACHE_ERROR_LOG
  sccache_bin="$(command -v sccache || true)"

  if [ ! -S "$SCCACHE_SERVER_UDS" ]; then
    printf 'CodeFabric: mandatory sccache service is unavailable at %s.\n' "$SCCACHE_SERVER_UDS" >&2
    printf 'Run `just setup-sccache`, then retry the Cargo command.\n' >&2
    exit 1
  fi
  if [ -L "$SCCACHE_SERVER_UDS" ]; then
    printf 'CodeFabric: the sccache endpoint is an unsupported symlink at %s.\n' \
      "$SCCACHE_SERVER_UDS" >&2
    printf 'Run `just setup-sccache` once from a host user session.\n' >&2
    exit 1
  fi
fi

if [ -z "$sccache_bin" ]; then
  printf 'CodeFabric: the pinned sccache is not installed. Run `just setup-tools`.\n' >&2
  exit 127
fi

exec "$sccache_bin" "$@"
