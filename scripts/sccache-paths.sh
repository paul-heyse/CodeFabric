#!/usr/bin/env bash
# Shared per-user paths for the supervised local sccache service and its clients.

case "$(uname -s)" in
  Darwin)
    CF_SCCACHE_SOCKET_DIR="/private/tmp/codefabric-sccache"
    CF_SCCACHE_CACHE_DIR="$HOME/Library/Caches/CodeFabric/sccache"
    CF_SCCACHE_CONFIG_DIR="$HOME/Library/Application Support/CodeFabric/sccache"
    CF_SCCACHE_SERVICE_FILE="$HOME/Library/LaunchAgents/com.codefabric.sccache.plist"
    CF_SCCACHE_LOG_DIR="$HOME/Library/Logs/CodeFabric"
    ;;
  Linux)
    CF_SCCACHE_SOCKET_DIR="/tmp/codefabric-sccache"
    CF_SCCACHE_CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/codefabric/sccache"
    CF_SCCACHE_CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/codefabric/sccache"
    CF_SCCACHE_SERVICE_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/com.codefabric.sccache.service"
    CF_SCCACHE_LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/codefabric"
    ;;
  *)
    printf 'CodeFabric sccache service supports macOS and Linux.\n' >&2
    return 1 2>/dev/null || exit 1
    ;;
esac

CF_SCCACHE_SOCKET_FILE="$CF_SCCACHE_SOCKET_DIR/server.sock"
CF_SCCACHE_CONFIG_FILE="$CF_SCCACHE_CONFIG_DIR/config"
