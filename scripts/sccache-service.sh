#!/usr/bin/env bash
# Install, supervise, inspect, and prove CodeFabric's per-user sccache service.

set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
required_version="0.17.0"
cache_size_bytes="42949672960"
service_label="com.codefabric.sccache"
configuration_changed=0

case "$(uname -s)" in
  Darwin)
    socket_dir="/private/tmp/codefabric-sccache"
    cache_dir="$HOME/Library/Caches/CodeFabric/sccache"
    config_dir="$HOME/Library/Application Support/CodeFabric/sccache"
    config_file="$config_dir/config"
    service_file="$HOME/Library/LaunchAgents/${service_label}.plist"
    log_dir="$HOME/Library/Logs/CodeFabric"
    ;;
  Linux)
    socket_dir="/tmp/codefabric-sccache"
    cache_dir="${XDG_CACHE_HOME:-$HOME/.cache}/codefabric/sccache"
    config_dir="${XDG_CONFIG_HOME:-$HOME/.config}/codefabric/sccache"
    config_file="$config_dir/config"
    service_file="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/${service_label}.service"
    log_dir="${XDG_STATE_HOME:-$HOME/.local/state}/codefabric"
    ;;
  *)
    printf 'CodeFabric sccache service supports macOS and Linux.\n' >&2
    exit 1
    ;;
esac

socket_file="$socket_dir/server.sock"

sccache_bin="$(command -v sccache || true)"
rustup_bin="$HOME/.cargo/bin/rustup"
if [ ! -x "$rustup_bin" ]; then
  rustup_bin="$(command -v rustup || true)"
fi

require_tools() {
  if [ -z "$sccache_bin" ]; then
    printf 'sccache is missing; install version %s first.\n' "$required_version" >&2
    exit 1
  fi
  actual_version="$($sccache_bin --version | awk '{print $2}')"
  if [ "$actual_version" != "$required_version" ]; then
    printf 'expected sccache %s, found %s at %s\n' \
      "$required_version" "$actual_version" "$sccache_bin" >&2
    exit 1
  fi
  if [ -z "$rustup_bin" ]; then
    printf 'rustup is missing\n' >&2
    exit 1
  fi
}

prepare_directories() {
  mkdir -p "$socket_dir" "$cache_dir" "$config_dir" "$log_dir" "$(dirname "$service_file")"
  chmod 700 "$socket_dir" "$cache_dir" "$config_dir" "$log_dir"
}

toml_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
}

replace_if_changed() {
  local temporary_file="$1"
  local destination="$2"
  if [ -f "$destination" ] && cmp -s "$temporary_file" "$destination"; then
    rm -f -- "$temporary_file"
    return 0
  fi
  mv "$temporary_file" "$destination"
  configuration_changed=1
}

write_config() {
  local temporary_config="${config_file}.tmp.$$"
  {
    printf 'server_startup_timeout_ms = 10000\n'
    printf 'client_side_mode = true\n'
    printf '\n'
    printf '[cache.disk]\n'
    printf 'dir = "%s"\n' "$(toml_escape "$cache_dir")"
    printf 'size = %s\n' "$cache_size_bytes"
  } >"$temporary_config"
  chmod 600 "$temporary_config"
  replace_if_changed "$temporary_config" "$config_file"
}

xml_escape() {
  printf '%s' "$1" | sed 's/&/\&amp;/g; s/</\&lt;/g; s/>/\&gt;/g'
}

write_launch_agent() {
  local temporary_service="${service_file}.tmp.$$"
  local escaped_bin escaped_socket escaped_config escaped_stdout escaped_stderr
  escaped_bin="$(xml_escape "$sccache_bin")"
  escaped_socket="$(xml_escape "$socket_file")"
  escaped_config="$(xml_escape "$config_file")"
  escaped_stdout="$(xml_escape "$log_dir/sccache.stdout.log")"
  escaped_stderr="$(xml_escape "$log_dir/sccache.stderr.log")"
  {
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    printf '%s\n' '<plist version="1.0"><dict>'
    printf '  <key>Label</key><string>%s</string>\n' "$service_label"
    printf '  <key>ProgramArguments</key><array><string>%s</string></array>\n' "$escaped_bin"
    printf '%s\n' '  <key>EnvironmentVariables</key><dict>'
    printf '%s\n' '    <key>SCCACHE_START_SERVER</key><string>1</string>'
    printf '%s\n' '    <key>SCCACHE_NO_DAEMON</key><string>1</string>'
    printf '%s\n' '    <key>SCCACHE_IDLE_TIMEOUT</key><string>0</string>'
    printf '    <key>SCCACHE_SERVER_UDS</key><string>%s</string>\n' "$escaped_socket"
    printf '    <key>SCCACHE_CONF</key><string>%s</string>\n' "$escaped_config"
    printf '%s\n' '  </dict>'
    printf '%s\n' '  <key>RunAtLoad</key><true/>'
    printf '%s\n' '  <key>KeepAlive</key><true/>'
    printf '%s\n' '  <key>ProcessType</key><string>Background</string>'
    printf '%s\n' '  <key>ThrottleInterval</key><integer>2</integer>'
    printf '  <key>StandardOutPath</key><string>%s</string>\n' "$escaped_stdout"
    printf '  <key>StandardErrorPath</key><string>%s</string>\n' "$escaped_stderr"
    printf '%s\n' '</dict></plist>'
  } >"$temporary_service"
  plutil -lint "$temporary_service" >/dev/null
  chmod 600 "$temporary_service"
  replace_if_changed "$temporary_service" "$service_file"
}

write_systemd_user_service() {
  local temporary_service="${service_file}.tmp.$$"
  {
    printf '%s\n' '[Unit]'
    printf '%s\n' 'Description=CodeFabric sccache service'
    printf '%s\n' 'After=default.target'
    printf '\n[Service]\n'
    printf 'ExecStart=%s\n' "$sccache_bin"
    printf 'Environment=SCCACHE_START_SERVER=1\n'
    printf 'Environment=SCCACHE_NO_DAEMON=1\n'
    printf 'Environment=SCCACHE_IDLE_TIMEOUT=0\n'
    printf 'Environment="SCCACHE_SERVER_UDS=%s"\n' "$socket_file"
    printf 'Environment="SCCACHE_CONF=%s"\n' "$config_file"
    printf '%s\n' 'Restart=on-failure' 'RestartSec=2'
    printf '\n[Install]\nWantedBy=default.target\n'
  } >"$temporary_service"
  chmod 600 "$temporary_service"
  replace_if_changed "$temporary_service" "$service_file"
}

restart_service() {
  if [ "$(uname -s)" = Darwin ]; then
    launchctl bootout "gui/$(id -u)" "$service_file" >/dev/null 2>&1 || true
    [ ! -S "$socket_file" ] || unlink "$socket_file"
    launchctl bootstrap "gui/$(id -u)" "$service_file"
    launchctl kickstart -k "gui/$(id -u)/$service_label"
  else
    systemctl --user daemon-reload
    systemctl --user enable --now "$service_label"
    systemctl --user restart "$service_label"
  fi

  local attempt
  for attempt in 1 2 3 4 5 6 7 8 9 10; do
    [ -S "$socket_file" ] && return 0
    sleep 0.2
  done
  printf 'sccache service did not create %s\n' "$socket_file" >&2
  exit 1
}

service_env() {
  env -u SCCACHE_BASEDIRS -u SCCACHE_ERROR_LOG \
    SCCACHE_SERVER_UDS="$socket_file" SCCACHE_CONF="$config_file" "$@"
}

stats_json() {
  service_env "$sccache_bin" --show-adv-stats --stats-format json
}

doctor() {
  require_tools
  [ -f "$config_file" ] || { printf 'missing sccache config: %s\n' "$config_file" >&2; exit 1; }
  [ -S "$socket_file" ] || { printf 'missing sccache socket: %s\n' "$socket_file" >&2; exit 1; }
  grep -Fxq 'client_side_mode = true' "$config_file" || {
    printf 'sccache client-side default is not configured\n' >&2
    exit 1
  }
  if grep -Eq '^basedirs[[:space:]]*=' "$config_file"; then
    printf 'sccache basedirs must not be configured for Rust 0.17.0\n' >&2
    exit 1
  fi

  local stats cache_size max_size percent errors timeouts capacity_state
  stats="$(stats_json)"
  cache_size="$(printf '%s' "$stats" | jq -r '.cache_size // 0')"
  max_size="$(printf '%s' "$stats" | jq -r '.max_cache_size')"
  errors="$(printf '%s' "$stats" | jq '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)')"
  timeouts="$(printf '%s' "$stats" | jq -r '.stats.cache_timeouts')"
  percent=$(( cache_size * 100 / max_size ))
  capacity_state="within-limit"
  if [ "$percent" -ge 95 ]; then
    capacity_state="bounded-lru-near-limit"
  fi

  [ "$(printf '%s' "$stats" | jq -r '.version')" = "$required_version" ]
  [ "$max_size" = "$cache_size_bytes" ]
  [ "$(printf '%s' "$stats" | jq -r '.basedirs | length')" -eq 0 ]

  printf 'sccache %s healthy: UDS=%s cache=%s%% (%s) client-side=default historical-errors=%s timeouts=%s\n' \
    "$required_version" "$socket_file" "$percent" "$capacity_state" "$errors" "$timeouts"
}

canary() {
  require_tools
  [ -S "$socket_file" ] || { printf 'run `just setup-sccache` first\n' >&2; exit 1; }
  local before after hit_delta error_delta timeout_delta rustc_bin output_root
  before="$(stats_json | jq -r '.stats.cache_hits.counts.Rust // 0')"
  local errors_before timeouts_before errors_after timeouts_after
  errors_before="$(stats_json | jq '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)')"
  timeouts_before="$(stats_json | jq -r '.stats.cache_timeouts // 0')"
  rustc_bin="$($rustup_bin which rustc --toolchain stable)"
  output_root="$repo_root/target/agent/sccache-canary"
  mkdir -p "$output_root/one" "$output_root/two"
  "$repo_root/scripts/sccache-wrapper.sh" "$rustc_bin" \
    --crate-name codefabric_sccache_canary --edition=2024 \
    "$repo_root/tooling/sccache/cache_canary.rs" --crate-type rlib \
    --emit=dep-info,metadata,link --out-dir "$output_root/one"
  "$repo_root/scripts/sccache-wrapper.sh" "$rustc_bin" \
    --crate-name codefabric_sccache_canary --edition=2024 \
    "$repo_root/tooling/sccache/cache_canary.rs" --crate-type rlib \
    --emit=dep-info,metadata,link --out-dir "$output_root/two"
  after="$(stats_json | jq -r '.stats.cache_hits.counts.Rust // 0')"
  errors_after="$(stats_json | jq '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)')"
  timeouts_after="$(stats_json | jq -r '.stats.cache_timeouts // 0')"
  hit_delta=$((after - before))
  error_delta=$((errors_after - errors_before))
  timeout_delta=$((timeouts_after - timeouts_before))
  if [ "$hit_delta" -lt 1 ]; then
    printf 'sccache canary failed: two identical compilations produced no Rust hit\n' >&2
    exit 1
  fi
  if [ "$error_delta" -ne 0 ] || [ "$timeout_delta" -ne 0 ]; then
    printf 'sccache canary failed: errors increased by %s and timeouts by %s\n' \
      "$error_delta" "$timeout_delta" >&2
    exit 1
  fi
  printf 'sccache transport/storage canary passed: Rust hits +%s, errors +0, timeouts +0\n' "$hit_delta"
}

install_service() {
  local initial_install=0
  if [ ! -f "$config_file" ] || [ ! -f "$service_file" ]; then
    initial_install=1
  fi
  require_tools
  prepare_directories
  configuration_changed=0
  write_config
  if [ "$(uname -s)" = Darwin ]; then
    write_launch_agent
  else
    write_systemd_user_service
  fi
  if [ "$configuration_changed" -ne 0 ] || ! doctor >/dev/null 2>&1; then
    restart_service
  fi
  doctor
  if [ "$initial_install" -ne 0 ]; then
    canary
  fi
}

case "${1:-}" in
  install | setup) install_service ;;
  refresh) install_service ;;
  restart) require_tools; prepare_directories; restart_service; doctor ;;
  doctor) doctor ;;
  canary) canary ;;
  stats) service_env "$sccache_bin" --show-adv-stats ;;
  stats-json) stats_json ;;
  zero-stats) service_env "$sccache_bin" --zero-stats ;;
  paths)
    printf 'socket=%s\nconfig=%s\ncache=%s\nservice=%s\n' \
      "$socket_file" "$config_file" "$cache_dir" "$service_file"
    ;;
  *)
    printf 'usage: %s {install|refresh|restart|doctor|canary|stats|stats-json|zero-stats|paths}\n' "$0" >&2
    exit 64
    ;;
esac
