#!/usr/bin/env bash
# Measure Cargo-shaped cold-target/warm-cache reuse without touching the normal target.

set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
mode="${1:-client}"
if [ "$#" -gt 0 ]; then
  shift
fi

case "$mode" in
  client) client_side_mode=true ;;
  server) client_side_mode=false ;;
  *)
    printf 'usage: %s {client|server} [cargo build args...]\n' "$0" >&2
    exit 64
    ;;
esac

"$repo_root/scripts/sccache-service.sh" doctor >/dev/null

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
scratch_root="$repo_root/target/agent/sccache-effectiveness/${timestamp}-$$"
target_dir="$scratch_root/cargo-target"
report_dir="$repo_root/target/sccache-measurements"
benchmark_config="$scratch_root/sccache-config"
mkdir -p "$scratch_root" "$report_dir"

cleanup() {
  case "$scratch_root" in
    "$repo_root"/target/agent/sccache-effectiveness/*)
      rm -rf -- "$scratch_root"
      ;;
  esac
}
trap cleanup EXIT HUP INT TERM

cache_dir="$("$repo_root/scripts/sccache-service.sh" paths | sed -n 's/^cache=//p')"
escaped_cache_dir="${cache_dir//\\/\\\\}"
escaped_cache_dir="${escaped_cache_dir//\"/\\\"}"
{
  printf 'server_startup_timeout_ms = 10000\n'
  printf 'client_side_mode = %s\n\n' "$client_side_mode"
  printf '[cache.disk]\n'
  printf 'dir = "%s"\n' "$escaped_cache_dir"
  printf 'size = 42949672960\n'
} >"$benchmark_config"

before_stats="$scratch_root/before.json"
first_stats="$scratch_root/first.json"
second_stats="$scratch_root/second.json"
"$repo_root/scripts/sccache-service.sh" stats-json >"$before_stats"

run_build() {
  local elapsed_variable="$1"
  shift
  case "$target_dir" in
    "$repo_root"/target/agent/sccache-effectiveness/*) rm -rf -- "$target_dir" ;;
    *) printf 'refusing to clear unexpected target path: %s\n' "$target_dir" >&2; exit 1 ;;
  esac
  SECONDS=0
  CODEFABRIC_SCCACHE_CONF="$benchmark_config" \
    CARGO_INCREMENTAL=0 \
    CARGO_TARGET_DIR="$target_dir" \
    "$repo_root/scripts/cargo" build --locked "$@"
  printf -v "$elapsed_variable" '%s' "$SECONDS"
}

run_build first_seconds "$@"
"$repo_root/scripts/sccache-service.sh" stats-json >"$first_stats"
run_build second_seconds "$@"
"$repo_root/scripts/sccache-service.sh" stats-json >"$second_stats"

rust_hits() {
  jq -r '.stats.cache_hits.counts.Rust // 0' "$1"
}
cache_errors() {
  jq -r '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)' "$1"
}

first_hit_delta=$(( $(rust_hits "$first_stats") - $(rust_hits "$before_stats") ))
second_hit_delta=$(( $(rust_hits "$second_stats") - $(rust_hits "$first_stats") ))
error_delta=$(( $(cache_errors "$second_stats") - $(cache_errors "$before_stats") ))
timeout_delta=$(( $(jq -r '.stats.cache_timeouts // 0' "$second_stats") - $(jq -r '.stats.cache_timeouts // 0' "$before_stats") ))

printf -v command_display '%q ' "$repo_root/scripts/cargo" build --locked "$@"
report="$report_dir/${timestamp}-${mode}.json"
jq -n \
  --arg schema "codefabric-sccache-effectiveness-v1" \
  --arg created_at "$timestamp" \
  --arg mode "$mode" \
  --arg command "$command_display" \
  --arg checkout_root "$repo_root" \
  --arg revision "$(git -C "$repo_root" rev-parse HEAD)" \
  --arg cargo "$("$repo_root/scripts/cargo" --version)" \
  --arg rustc "$("$HOME/.cargo/bin/rustup" run stable rustc -vV)" \
  --argjson first_seconds "$first_seconds" \
  --argjson second_seconds "$second_seconds" \
  --argjson first_rust_hit_delta "$first_hit_delta" \
  --argjson second_rust_hit_delta "$second_hit_delta" \
  --argjson cache_error_delta "$error_delta" \
  --argjson cache_timeout_delta "$timeout_delta" \
  --slurpfile before "$before_stats" \
  --slurpfile after_first "$first_stats" \
  --slurpfile after_second "$second_stats" \
  '{
    schema: $schema,
    created_at: $created_at,
    mode: $mode,
    command: $command,
    checkout_root: $checkout_root,
    revision: $revision,
    cargo: $cargo,
    rustc: $rustc,
    cargo_incremental: 0,
    cargo_target_state: "cold before each run",
    cache_state: "preserved before and between runs",
    first: {seconds: $first_seconds, rust_hit_delta: $first_rust_hit_delta},
    second: {seconds: $second_seconds, rust_hit_delta: $second_rust_hit_delta},
    cache_error_delta: $cache_error_delta,
    cache_timeout_delta: $cache_timeout_delta,
    stats: {before: $before[0], after_first: $after_first[0], after_second: $after_second[0]}
  }' >"$report"

if [ "$second_hit_delta" -lt 1 ] || [ "$error_delta" -ne 0 ] || [ "$timeout_delta" -ne 0 ]; then
  printf 'sccache effectiveness probe failed; inspect %s\n' "$report" >&2
  exit 1
fi

printf 'sccache %s-mode effectiveness: first=%ss (+%s Rust hits), second=%ss (+%s Rust hits); report=%s\n' \
  "$mode" "$first_seconds" "$first_hit_delta" "$second_seconds" "$second_hit_delta" "$report"
