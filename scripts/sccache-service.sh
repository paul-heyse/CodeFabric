#!/usr/bin/env bash
# Install, supervise, inspect, and prove CodeFabric's per-user sccache service.

set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"
# shellcheck source=sccache-paths.sh
source "$repo_root/scripts/sccache-paths.sh"
required_version="$SCCACHE_VERSION"
cache_size_bytes="42949672960"
service_label="com.codefabric.sccache"
configuration_changed=0
socket_dir="$CF_SCCACHE_SOCKET_DIR"
socket_file="$CF_SCCACHE_SOCKET_FILE"
cache_dir="$CF_SCCACHE_CACHE_DIR"
config_dir="$CF_SCCACHE_CONFIG_DIR"
config_file="$CF_SCCACHE_CONFIG_FILE"
service_file="$CF_SCCACHE_SERVICE_FILE"
log_dir="$CF_SCCACHE_LOG_DIR"
entrypoint="$repo_root/scripts/sccache-service-entrypoint.sh"

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
  command -v jq >/dev/null 2>&1 || { printf 'jq is missing\n' >&2; exit 1; }
}

prepare_directories() {
  mkdir -p "$socket_dir" "$cache_dir" "$config_dir" "$log_dir" "$(dirname "$service_file")"
  chmod 700 "$socket_dir" "$cache_dir" "$config_dir" "$log_dir"
}

validate_socket_directory() {
  local owner_id
  if [ "$(uname -s)" = Darwin ]; then
    owner_id="$(stat -f '%u' "$socket_dir")"
  else
    owner_id="$(stat -c '%u' "$socket_dir")"
  fi
  [ "$owner_id" = "$(id -u)" ] || {
    printf 'refusing socket directory owned by uid %s: %s\n' "$owner_id" "$socket_dir" >&2
    exit 1
  }

  if [ -L "$socket_file" ]; then
    # Recover from a stale or previous symlink endpoint. Stop the exact managed unit
    # before replacing only that link with the allowlisted socket itself.
    if [ "$(uname -s)" = Darwin ]; then
      launchctl bootout "gui/$(id -u)" "$service_file" >/dev/null 2>&1 || true
    else
      systemctl --user stop "$service_label" >/dev/null 2>&1 || true
    fi
    unlink "$socket_file"
  elif [ -e "$socket_file" ] && [ ! -S "$socket_file" ]; then
    printf 'refusing to replace non-socket sccache endpoint: %s\n' "$socket_file" >&2
    exit 1
  fi
}

acquire_setup_lock() {
  local attempt
  setup_lock="$config_dir/.setup-lock"
  for attempt in $(seq 1 100); do
    if mkdir "$setup_lock" 2>/dev/null; then
      trap 'rmdir "$setup_lock" 2>/dev/null || true' EXIT HUP INT TERM
      return 0
    fi
    sleep 0.1
  done
  printf 'another CodeFabric sccache setup is active: %s\n' "$setup_lock" >&2
  exit 1
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
  local escaped_bin escaped_entrypoint escaped_socket escaped_config escaped_stdout escaped_stderr
  escaped_bin="$(xml_escape "$sccache_bin")"
  escaped_entrypoint="$(xml_escape "$entrypoint")"
  escaped_socket="$(xml_escape "$socket_file")"
  escaped_config="$(xml_escape "$config_file")"
  escaped_stdout="$(xml_escape "$log_dir/sccache.stdout.log")"
  escaped_stderr="$(xml_escape "$log_dir/sccache.stderr.log")"
  {
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>'
    printf '%s\n' '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    printf '%s\n' '<plist version="1.0"><dict>'
    printf '  <key>Label</key><string>%s</string>\n' "$service_label"
    printf '  <key>ProgramArguments</key><array><string>%s</string><string>%s</string><string>%s</string></array>\n' \
      "$escaped_entrypoint" "$escaped_bin" "$escaped_socket"
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

emit_systemd_user_service() {
  {
    printf '%s\n' '[Unit]'
    printf '%s\n' 'Description=CodeFabric sccache service'
    printf '\n[Service]\n'
    printf '%s\n' 'Type=simple'
    printf '%s\n' 'UMask=0077'
    printf 'ExecStart=%s %s %s\n' "$entrypoint" "$sccache_bin" "$socket_file"
    printf 'Environment=SCCACHE_START_SERVER=1\n'
    printf 'Environment=SCCACHE_NO_DAEMON=1\n'
    printf 'Environment=SCCACHE_IDLE_TIMEOUT=0\n'
    printf 'Environment="SCCACHE_SERVER_UDS=%s"\n' "$socket_file"
    printf 'Environment="SCCACHE_CONF=%s"\n' "$config_file"
    printf '%s\n' 'Restart=on-failure' 'RestartSec=2'
    printf '\n[Install]\nWantedBy=default.target\n'
  }
}

write_systemd_user_service() {
  local temporary_service="${service_file}.tmp.$$"
  emit_systemd_user_service >"$temporary_service"
  chmod 600 "$temporary_service"
  replace_if_changed "$temporary_service" "$service_file"
}

validate_installed_service_definition() {
  [ -f "$service_file" ] || {
    printf 'sccache is not provisioned: missing service definition %s\n' "$service_file" >&2
    return 1
  }

  if [ "$(uname -s)" = Linux ] && ! cmp -s "$service_file" <(emit_systemd_user_service); then
    printf 'installed sccache service definition does not match the repository contract: %s\n' \
      "$service_file" >&2
    printf 'Run `just setup-sccache` from a host user session to migrate it atomically.\n' >&2
    return 1
  fi
}

restart_service() {
  if [ "$(uname -s)" = Darwin ]; then
    launchctl bootout "gui/$(id -u)" "$service_file" >/dev/null 2>&1 || true
    launchctl bootstrap "gui/$(id -u)" "$service_file"
    launchctl kickstart -k "gui/$(id -u)/$service_label"
  else
    systemctl --user daemon-reload
    systemctl --user enable "$service_label"
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

check_cargo_probe_cache() {
  local cargo_probe_cache="${1:-$repo_root/target/.rustc_info.json}"
  [ -f "$cargo_probe_cache" ] || return 0

  # Cargo caches both successful and failed compiler-discovery output. The outer
  # rustc fingerprint follows compiler/wrapper identity, not the mutable health of the
  # supervised sccache endpoint, so a repaired service does not invalidate an earlier
  # wrapper failure. Invalid JSON is not treated as poison because Cargo discards an
  # unreadable cache itself; only a well-formed cached sccache failure is actionable.
  if jq -e '
    [
      (.outputs // {})[]?
      | select(.success == false)
      | ((.stdout // "") + "\n" + (.stderr // ""))
      | select(test("sccache|sccache-wrapper\\.sh"; "i"))
    ]
    | any
  ' "$cargo_probe_cache" >/dev/null 2>&1; then
    printf 'Cargo cached a failed sccache compiler probe in %s.\n' "$cargo_probe_cache" >&2
    printf 'Remove only this generated probe cache, then retry:\n' >&2
    printf '  rm -- %q\n' "$cargo_probe_cache" >&2
    printf 'Do not run `cargo clean`; no other Cargo artifacts need removal.\n' >&2
    return 1
  fi
}

doctor() {
  require_tools
  [ -f "$config_file" ] || {
    printf 'sccache is not provisioned: missing config %s\n' "$config_file" >&2
    printf 'Run `just setup-sccache` from a host user session with access to the service manager.\n' >&2
    exit 1
  }
  validate_installed_service_definition || exit 1
  [ -S "$socket_file" ] || {
    printf 'sccache service is not running: missing socket %s\n' "$socket_file" >&2
    printf 'Run `just setup-sccache` from a host user session; if already installed, run `just sccache-restart`.\n' >&2
    exit 1
  }
  [ ! -L "$socket_file" ] || {
    printf 'sccache has an unsupported symlink endpoint at %s\n' "$socket_file" >&2
    printf 'Run `just setup-sccache` once from a host user session.\n' >&2
    exit 1
  }
  grep -Fxq 'client_side_mode = true' "$config_file" || {
    printf 'sccache client-side default is not configured\n' >&2
    exit 1
  }
  grep -Fxq "dir = \"$(toml_escape "$cache_dir")\"" "$config_file" || {
    printf 'sccache cache directory contract drifted: expected %s\n' "$cache_dir" >&2
    exit 1
  }
  grep -Fxq "size = $cache_size_bytes" "$config_file" || {
    printf 'sccache configured cache size drifted: expected %s\n' "$cache_size_bytes" >&2
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
  [ "$max_size" = "$cache_size_bytes" ] || {
    printf 'sccache cache-size contract drifted: expected %s, found %s\n' \
      "$cache_size_bytes" "$max_size" >&2; exit 1;
  }
  percent=$(( cache_size * 100 / max_size ))
  capacity_state="within-limit"
  if [ "$percent" -ge 95 ]; then
    capacity_state="bounded-lru-near-limit"
  fi

  [ "$(printf '%s' "$stats" | jq -r '.version')" = "$required_version" ] || {
    printf 'running sccache server is not version %s\n' "$required_version" >&2; exit 1;
  }
  [ "$(printf '%s' "$stats" | jq -r '.basedirs | length')" -eq 0 ] || {
    printf 'sccache basedirs must remain empty for Rust 0.17.0\n' >&2; exit 1;
  }

  printf 'sccache %s healthy: UDS=%s cache=%s%% (%s) client-side=default historical-errors=%s timeouts=%s\n' \
    "$required_version" "$socket_file" "$percent" "$capacity_state" "$errors" "$timeouts"
}

canary() {
  doctor >/dev/null
  check_cargo_probe_cache
  local before after hit_delta error_delta timeout_delta rustc_bin output_root
  before="$(stats_json | jq -r '.stats.cache_hits.counts.Rust // 0')"
  local errors_before timeouts_before errors_after timeouts_after
  errors_before="$(stats_json | jq '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)')"
  timeouts_before="$(stats_json | jq -r '.stats.cache_timeouts // 0')"
  rustc_bin="$($rustup_bin which rustc --toolchain "$CODEFABRIC_STABLE_TOOLCHAIN")"
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
  local initial_install=0 should_canary=0
  if [ ! -f "$config_file" ] || [ ! -f "$service_file" ]; then
    initial_install=1
  fi
  require_tools
  prepare_directories
  acquire_setup_lock
  validate_socket_directory
  configuration_changed=0
  write_config
  if [ "$(uname -s)" = Darwin ]; then
    write_launch_agent
  else
    write_systemd_user_service
  fi
  if [ "$configuration_changed" -ne 0 ] || ! doctor >/dev/null 2>&1; then
    should_canary=1
    restart_service
  fi
  doctor
  if [ "$initial_install" -ne 0 ] || [ "$should_canary" -ne 0 ]; then
    canary
  fi
}

case "${1:-}" in
  install | setup) install_service ;;
  refresh) install_service ;;
  restart) require_tools; prepare_directories; acquire_setup_lock; validate_socket_directory; restart_service; doctor ;;
  doctor) doctor ;;
  canary) canary ;;
  stats) service_env "$sccache_bin" --show-adv-stats ;;
  stats-json) stats_json ;;
  zero-stats) service_env "$sccache_bin" --zero-stats ;;
  cargo-probe-cache-check) check_cargo_probe_cache "${2:-}" ;;
  render-systemd)
    [ "$(uname -s)" = Linux ] || { printf 'render-systemd applies only to Linux\n' >&2; exit 64; }
    emit_systemd_user_service
    ;;
  paths)
    printf 'socket=%s\nconfig=%s\ncache=%s\nservice=%s\n' \
      "$socket_file" "$config_file" "$cache_dir" "$service_file"
    ;;
  *)
    printf 'usage: %s {install|refresh|restart|doctor|canary|stats|stats-json|zero-stats|cargo-probe-cache-check|paths|render-systemd}\n' "$0" >&2
    exit 64
    ;;
esac
