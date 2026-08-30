#!/usr/bin/env bash
# Cargo compiler wrapper for the mandatory CodeFabric sccache service.

set -euo pipefail
unset SCCACHE_BASEDIRS

if [ -n "${CI:-}" ] || [ "${SCCACHE_GHA_ENABLED:-}" = "true" ]; then
  unset SCCACHE_SERVER_UDS SCCACHE_CONF SCCACHE_ERROR_LOG
  export SCCACHE_CLIENT_SIDE="${SCCACHE_CLIENT_SIDE:-1}"
  sccache_bin="${SCCACHE_PATH:-$(command -v sccache || true)}"
else
  case "$(uname -s)" in
    Darwin)
      export SCCACHE_SERVER_UDS="/private/tmp/codefabric-sccache/server.sock"
      default_sccache_conf="$HOME/Library/Application Support/CodeFabric/sccache/config"
      ;;
    Linux)
      export SCCACHE_SERVER_UDS="/tmp/codefabric-sccache/server.sock"
      default_sccache_conf="${XDG_CONFIG_HOME:-$HOME/.config}/codefabric/sccache/config"
      ;;
    *)
      printf 'CodeFabric: the supervised sccache service supports macOS and Linux.\n' >&2
      exit 1
      ;;
  esac
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
fi

if [ -z "$sccache_bin" ]; then
  printf 'CodeFabric: sccache 0.17.0 is not installed. Run `just doctor`.\n' >&2
  exit 127
fi

# Cargo's profile default can request incremental compilation even when no
# CARGO_INCREMENTAL variable is present. sccache 0.17.0 rejects those invocations rather
# than compiling them uncached, so route only that documented incompatible shape directly
# to the real rustc. Cache-eligible invocations remain fail-closed through sccache.
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

exec "$sccache_bin" "$@"
