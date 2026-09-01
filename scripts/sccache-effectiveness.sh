#!/usr/bin/env bash
# Repeated Cargo-shaped cold-target/warm-cache measurements with hyperfine.

set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"
mode="${1:-both}"
if [ "$#" -gt 0 ]; then shift; fi
runs="${CODEFABRIC_BENCHMARK_RUNS:-5}"
case "$runs" in *[!0-9]* | 0 | 1 | 2) printf 'CODEFABRIC_BENCHMARK_RUNS must be an integer >= 3\n' >&2; exit 64 ;; esac

case "$mode" in
  client) modes=(client) ;;
  server) modes=(server) ;;
  both) modes=(client server) ;;
  *) printf 'usage: %s {client|server|both} [cargo build args...]\n' "$0" >&2; exit 64 ;;
esac

command -v hyperfine >/dev/null 2>&1 || {
  printf 'hyperfine is required; run `just setup-tools`\n' >&2
  exit 1
}
"$repo_root/scripts/sccache-service.sh" doctor >/dev/null

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
scratch_root="$repo_root/target/agent/sccache-effectiveness/${timestamp}-$$"
report_dir="$repo_root/target/sccache-measurements"
mkdir -p "$scratch_root" "$report_dir"

cleanup() {
  case "$scratch_root" in
    "$repo_root"/target/agent/sccache-effectiveness/*) rm -rf -- "$scratch_root" ;;
  esac
}
trap cleanup EXIT HUP INT TERM

cache_dir="$("$repo_root/scripts/sccache-service.sh" paths | sed -n 's/^cache=//p')"
escaped_cache_dir="${cache_dir//\\/\\\\}"
escaped_cache_dir="${escaped_cache_dir//\"/\\\"}"

write_config() {
  local selected_mode="$1" config_file="$2" client_side_mode
  [ "$selected_mode" = client ] && client_side_mode=true || client_side_mode=false
  {
    printf 'server_startup_timeout_ms = 10000\n'
    printf 'client_side_mode = %s\n\n' "$client_side_mode"
    printf '[cache.disk]\n'
    printf 'dir = "%s"\n' "$escaped_cache_dir"
    printf 'size = 42949672960\n'
  } >"$config_file"
}

before_stats="$scratch_root/before.json"
after_stats="$scratch_root/after.json"
hyperfine_raw="$scratch_root/hyperfine.json"
"$repo_root/scripts/sccache-service.sh" stats-json >"$before_stats"

hyperfine_args=(--warmup 1 --runs "$runs" --style basic --export-json "$hyperfine_raw")
for selected_mode in "${modes[@]}"; do
  config_file="$scratch_root/sccache-${selected_mode}.toml"
  target_dir="$scratch_root/cargo-target-${selected_mode}"
  write_config "$selected_mode" "$config_file"
  printf -v benchmark_command '%q ' \
    "$repo_root/scripts/sccache-benchmark-command.sh" "$config_file" "$target_dir" "$@"
  hyperfine_args+=(--command-name "$selected_mode" "$benchmark_command")
done

hyperfine "${hyperfine_args[@]}"
"$repo_root/scripts/sccache-service.sh" stats-json >"$after_stats"

rust_hits() { jq -r '.stats.cache_hits.counts.Rust // 0' "$1"; }
cache_errors() {
  jq -r '([.stats.cache_errors.counts[]?] | add // 0) + (.stats.cache_read_errors // 0) + (.stats.cache_write_errors // 0)' "$1"
}
hit_delta=$(( $(rust_hits "$after_stats") - $(rust_hits "$before_stats") ))
error_delta=$(( $(cache_errors "$after_stats") - $(cache_errors "$before_stats") ))
timeout_delta=$(( $(jq -r '.stats.cache_timeouts // 0' "$after_stats") - $(jq -r '.stats.cache_timeouts // 0' "$before_stats") ))

report="$report_dir/${timestamp}-${mode}.json"
jq -n \
  --arg schema codefabric-sccache-effectiveness-v2 \
  --arg created_at "$timestamp" \
  --arg mode "$mode" \
  --arg checkout_root "$repo_root" \
  --arg revision "$(git -C "$repo_root" rev-parse HEAD)" \
  --arg cargo "$("$repo_root/scripts/cargo" --version)" \
  --arg rustc "$(rustup run "$CODEFABRIC_STABLE_TOOLCHAIN" rustc -vV)" \
  --argjson runs "$runs" \
  --argjson rust_hit_delta "$hit_delta" \
  --argjson cache_error_delta "$error_delta" \
  --argjson cache_timeout_delta "$timeout_delta" \
  --slurpfile hyperfine "$hyperfine_raw" \
  --slurpfile before "$before_stats" \
  --slurpfile after "$after_stats" \
  '{
    schema: $schema,
    created_at: $created_at,
    mode: $mode,
    checkout_root: $checkout_root,
    revision: $revision,
    cargo: $cargo,
    rustc: $rustc,
    runs_per_mode: $runs,
    cargo_incremental: 0,
    cargo_target_state: "cold before every warmup and measured sample",
    cache_state: "preserved before and across all samples",
    rust_hit_delta: $rust_hit_delta,
    cache_error_delta: $cache_error_delta,
    cache_timeout_delta: $cache_timeout_delta,
    hyperfine: $hyperfine[0],
    stats: {before: $before[0], after: $after[0]}
  }' >"$report"

if [ "$hit_delta" -lt 1 ] || [ "$error_delta" -ne 0 ] || [ "$timeout_delta" -ne 0 ]; then
  printf 'sccache effectiveness probe failed; inspect %s\n' "$report" >&2
  exit 1
fi

printf 'sccache %s-mode benchmark passed: %s measured run(s) per mode, Rust hits +%s; report=%s\n' \
  "$mode" "$runs" "$hit_delta" "$report"
