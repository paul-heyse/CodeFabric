#!/usr/bin/env bash
# Reconcile bounded RustSec exceptions with deny.toml, Cargo.lock, and the live advisory DB.
set -euo pipefail

cd "$(dirname "$0")/.."

registry="tooling/security/advisory-exceptions.json"
run_audit=false
if [ "${1:-}" = "--audit" ]; then
  run_audit=true
elif [ "$#" -ne 0 ]; then
  printf 'usage: %s [--audit]\n' "$0" >&2
  exit 2
fi

fail() {
  printf 'advisory policy check failed: %s\n' "$1" >&2
  exit 1
}

jq -e '
  .schema_version == 1
  and .review_owner_packet == "WP19"
  and .review_trigger == "before_wp19_completion"
  and (.exceptions | length > 0)
  and ([.exceptions[].advisory_id] | length == (unique | length))
  and all(
    .exceptions[];
    (.advisory_id | test("^RUSTSEC-[0-9]{4}-[0-9]{4}$"))
    and (.package | length > 0)
    and (.version | length > 0)
    and (.classification == "vulnerability" or .classification == "unmaintained")
    and (.affected_surface | length > 0)
    and (.rationale | length > 0)
    and .owner_packet == "WP19"
    and .review_trigger == "before_wp19_completion"
  )
' "$registry" >/dev/null || fail 'registry shape, uniqueness, owner, or review trigger is invalid'

registry_ids="$(jq -r '.exceptions[].advisory_id' "$registry" | sort)"
deny_ids="$(awk '
  $0 == "[advisories]" { in_advisories = 1; next }
  in_advisories && /^\[/ { exit }
  in_advisories && /^ignore = \[/ { in_ignore = 1; next }
  in_ignore && /^\]/ { exit }
  in_ignore && /"RUSTSEC-/ {
    line = $0
    sub(/^[^\"]*\"/, "", line)
    sub(/\".*/, "", line)
    print line
  }
' deny.toml | sort)"
[ "$registry_ids" = "$deny_ids" ] || fail 'registry IDs and deny.toml advisory ignores differ'

metadata="$(cargo metadata --locked --format-version 1)"
while IFS=$'\t' read -r package version; do
  printf '%s' "$metadata" | jq -e --arg package "$package" --arg version "$version" '
    any(.packages[]; .name == $package and .version == $version)
  ' >/dev/null || fail "$package $version is not selected by Cargo.lock"
done < <(jq -r '.exceptions[] | [.package, .version] | @tsv' "$registry" | sort -u)

audit_json="$(cargo audit --json 2>/dev/null || true)"
observed="$(printf '%s' "$audit_json" | jq -r '
  (.vulnerabilities.list[]?, (.warnings | to_entries[] | .value[]?))
  | [.advisory.id, .package.name, .package.version]
  | @tsv
' | sort)"
registered="$(jq -r '
  .exceptions[] | [.advisory_id, .package, .version] | @tsv
' "$registry" | sort)"
[ "$observed" = "$registered" ] || fail 'live RustSec findings differ from the exact exception registry'

if $run_audit; then
  audit_args=()
  while IFS= read -r advisory_id; do
    audit_args+=(--ignore "$advisory_id")
  done <<< "$registry_ids"
  cargo audit "${audit_args[@]}"
fi

printf 'advisory policy check passed (%s exact exceptions; WP19 review)\n' \
  "$(jq '.exceptions | length' "$registry")"
