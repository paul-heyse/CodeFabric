#!/usr/bin/env bash
# Prove predecessor stack identities survive only in reviewed historical or
# negative-assurance locations, and current agent routes name the target docs.
set -euo pipefail

cd "$(dirname "$0")/.."

old_pattern='54\.1\.0|58\.4\.0|9f9223197469897ef05ae4369eb4fd1390174e65|arrow-58|DataFusion 54|Arrow 58'
scopes=(
  AGENTS.md
  Cargo.toml
  Cargo.lock
  rustc-extractor/Cargo.toml
  rustc-extractor/Cargo.lock
  src
  tests
  contracts
  scripts
  tooling
  docs/upfront_design
  docs/spec_index
  docs/designs
  .claude/skills
)

allowed_historical_hit() {
  local path="$1" content="$2"
  case "$path" in
    docs/designs/*_v1_*) return 0 ;;
    tests/fixtures/data_fabric_upgrade/old_stack/*) return 0 ;;
    tests/integration/data_fabric_upgrade.rs) return 0 ;;
    tests/integration/compatibility.rs) return 0 ;;
    scripts/stable_graph_check.sh)
      [[ "$content" == *'forbidden'* || "$content" == *'rg -qx'* ]] && return 0
      ;;
    scripts/data_fabric_old_authority_check.sh)
      [[ "$content" == old_pattern=* ]] && return 0
      ;;
    .claude/skills/deltalake-rust-ref/SKILL.md)
      [[ "$content" == *'known typo'* ]] && return 0
      ;;
    docs/designs/*_v2_*)
      [[ "$content" =~ historical|predecessor|supersed|never|different|prior ]] && return 0
      ;;
  esac
  return 1
}

unapproved=()
while IFS=: read -r path line content; do
  [ -n "$path" ] || continue
  if ! allowed_historical_hit "$path" "$content"; then
    unapproved+=("$path:$line:$content")
  fi
done < <(git grep -n -E "$old_pattern" -- "${scopes[@]}" ':!docs/library_ref/**' || true)

if [ "${#unapproved[@]}" -ne 0 ]; then
  printf 'unapproved live predecessor authority:\n' >&2
  printf '  %s\n' "${unapproved[@]}" >&2
  exit 1
fi

df_skill=.claude/skills/datafusion-pyarrow-rust-ref/SKILL.md
delta_skill=.claude/skills/deltalake-rust-ref/SKILL.md
routing=docs/spec_index/library-routing.md
df_ref=datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md
delta_ref=deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md

for route in "$df_skill:$df_ref" "$delta_skill:$delta_ref" "$routing:$df_ref" "$routing:$delta_ref"; do
  path="${route%%:*}"
  reference="${route#*:}"
  rg -Fq "$reference" "$path" || {
    printf 'current reference route missing from %s: %s\n' "$path" "$reference" >&2
    exit 1
  }
done

if rg -n 'Use one current authority:.*(datafusion_rust\.md|deltalake_rust_1\.0\.0_9f922319)' \
  "$df_skill" "$delta_skill"; then
  printf 'legacy document remains a current agent authority\n' >&2
  exit 1
fi

printf 'data-fabric predecessor authority zero-state passed with reviewed historical exclusions\n'
