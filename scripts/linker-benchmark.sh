#!/usr/bin/env bash
# Compare the pinned toolchain's default Linux linker with mold; never changes defaults.

set -euo pipefail
umask 077

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"
[ "$(uname -s)" = Linux ] || { printf 'the mold comparison applies only to Linux\n' >&2; exit 1; }
command -v mold >/dev/null 2>&1 || { printf 'mold is missing; install it before benchmarking\n' >&2; exit 1; }
command -v hyperfine >/dev/null 2>&1 || { printf 'hyperfine is missing; run `just setup-tools`\n' >&2; exit 1; }
runs="${CODEFABRIC_BENCHMARK_RUNS:-5}"
case "$runs" in *[!0-9]* | 0 | 1 | 2) printf 'CODEFABRIC_BENCHMARK_RUNS must be an integer >= 3\n' >&2; exit 64 ;; esac

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
scratch_root="$repo_root/target/agent/linker-benchmark/${timestamp}-$$"
report_dir="$repo_root/target/linker-measurements"
mkdir -p "$scratch_root" "$report_dir"
cleanup() {
  case "$scratch_root" in
    "$repo_root"/target/agent/linker-benchmark/*) rm -rf -- "$scratch_root" ;;
  esac
}
trap cleanup EXIT HUP INT TERM
package_name="$(RUSTC_WRAPPER= "$repo_root/scripts/cargo" metadata --locked --no-deps --format-version 1 | jq -r '.root_package.name')"

printf -v default_command '%q ' "$repo_root/scripts/linker-benchmark-command.sh" \
  "$scratch_root/cargo-target-default" default "$package_name" "$@"
printf -v mold_command '%q ' "$repo_root/scripts/linker-benchmark-command.sh" \
  "$scratch_root/cargo-target-mold" mold "$package_name" "$@"

raw_report="$scratch_root/hyperfine.json"
hyperfine --warmup 1 --runs "$runs" --style basic --export-json "$raw_report" \
  --command-name default "$default_command" \
  --command-name mold "$mold_command"

report="$report_dir/${timestamp}.json"
jq -n \
  --arg schema codefabric-linker-benchmark-v1 \
  --arg created_at "$timestamp" \
  --arg revision "$(git -C "$repo_root" rev-parse HEAD)" \
  --arg rustc "$(rustup run "$CODEFABRIC_STABLE_TOOLCHAIN" rustc -vV)" \
  --arg mold "$(mold --version | head -1)" \
  --argjson runs "$runs" \
  --slurpfile hyperfine "$raw_report" \
  '{schema: $schema, created_at: $created_at, revision: $revision, rustc: $rustc,
    mold: $mold, runs_per_mode: $runs,
    workload: "package relink with dependency artifacts retained and sccache disabled",
    hyperfine: $hyperfine[0]}' >"$report"

printf 'linker benchmark complete; no default changed: %s\n' "$report"
