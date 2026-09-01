#!/usr/bin/env bash
# Reject drift between the one tool manifest and operational Rust/CI configuration.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"

root_channel="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$repo_root/rust-toolchain.toml")"
extractor_channel="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$repo_root/rustc-extractor/rust-toolchain.toml")"
[ "$root_channel" = "$CODEFABRIC_STABLE_TOOLCHAIN" ] || {
  printf 'root Rustup pin drifted: %s\n' "${root_channel:-missing}" >&2; exit 1;
}
[ "$extractor_channel" = "$CODEFABRIC_ASSURANCE_TOOLCHAIN" ] || {
  printf 'extractor Rustup pin drifted: %s\n' "${extractor_channel:-missing}" >&2; exit 1;
}

if rg -n 'toolchain: stable|toolchain: nightly$|cargo \+nightly( |$)|rustc \+nightly( |$)' \
  "$repo_root/.github/workflows/ci.yml" "$repo_root/justfile" "$repo_root/scripts" \
  -g '!tool-version-contract-check.sh'; then
  printf 'operational configuration contains a rolling Rustup identity\n' >&2
  exit 1
fi

grep -Fq "toolchain: $CODEFABRIC_STABLE_TOOLCHAIN" "$repo_root/.github/workflows/ci.yml"
grep -Fq "toolchain: $CODEFABRIC_ASSURANCE_TOOLCHAIN" "$repo_root/.github/workflows/ci.yml"
grep -Fq "version: \"v$SCCACHE_VERSION\"" "$repo_root/.github/workflows/ci.yml"
grep -Fq "version: \"$UV_VERSION\"" "$repo_root/.github/workflows/ci.yml"
grep -Fq "bacon $BACON_VERSION" "$repo_root/bacon.toml"

printf 'tool version configuration contract passed\n'
