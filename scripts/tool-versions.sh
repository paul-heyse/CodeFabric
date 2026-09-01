#!/usr/bin/env bash
# Check or install the exact repository CLI contract without reinstalling matching tools.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
# shellcheck source=../tooling/rust-tool-versions.env
source "$repo_root/tooling/rust-tool-versions.env"

mode="${1:-check}"
case "$mode" in
  check | install | report) ;;
  *) printf 'usage: %s {check|install|report}\n' "$0" >&2; exit 64 ;;
esac

tool_rows() {
  cat <<EOF
just|just|just|$JUST_VERSION
sccache|sccache|sccache|$SCCACHE_VERSION
cargo-nextest|cargo nextest|cargo-nextest|$CARGO_NEXTEST_VERSION
cargo-deny|cargo deny|cargo-deny|$CARGO_DENY_VERSION
cargo-audit|cargo audit|cargo-audit|$CARGO_AUDIT_VERSION
cargo-shear|cargo shear|cargo-shear|$CARGO_SHEAR_VERSION
cargo-machete|cargo machete|cargo-machete|$CARGO_MACHETE_VERSION
cargo-msrv|cargo msrv|cargo-msrv|$CARGO_MSRV_VERSION
typos|typos|typos-cli|$TYPOS_CLI_VERSION
ast-grep|ast-grep|ast-grep|$AST_GREP_VERSION
rg|rg|ripgrep|$RIPGREP_VERSION
bacon|bacon|bacon|$BACON_VERSION
hyperfine|hyperfine|hyperfine|$HYPERFINE_VERSION
cargo-hack|cargo hack|cargo-hack|$CARGO_HACK_VERSION
EOF
}

command_version() {
  local command_name="$1"
  case "$command_name" in
    "cargo nextest") "$repo_root/scripts/cargo" nextest --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    "cargo deny") "$repo_root/scripts/cargo" deny --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    "cargo audit") "$repo_root/scripts/cargo" audit --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    "cargo shear") "$repo_root/scripts/cargo" shear --version 2>/dev/null | sed -n 's/^Version: \([^ ]*\).*/\1/p' ;;
    "cargo machete") "$repo_root/scripts/cargo" machete --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    "cargo msrv") "$repo_root/scripts/cargo" msrv --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    "cargo hack") "$repo_root/scripts/cargo" hack --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p' ;;
    *)
      command -v "$command_name" >/dev/null 2>&1 || return 0
      "$command_name" --version 2>/dev/null | sed -n '1s/^[^0-9]*\([0-9][^ ]*\).*/\1/p'
      ;;
  esac
}

ensure_rustup_contract() {
  local root_channel extractor_channel
  root_channel="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$repo_root/rust-toolchain.toml")"
  extractor_channel="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$repo_root/rustc-extractor/rust-toolchain.toml")"
  [ "$root_channel" = "$CODEFABRIC_STABLE_TOOLCHAIN" ] || {
    printf 'root toolchain mismatch: manifest=%s rust-toolchain.toml=%s\n' \
      "$CODEFABRIC_STABLE_TOOLCHAIN" "${root_channel:-missing}" >&2
    return 1
  }
  [ "$extractor_channel" = "$CODEFABRIC_ASSURANCE_TOOLCHAIN" ] || {
    printf 'dated-nightly mismatch: manifest=%s extractor=%s\n' \
      "$CODEFABRIC_ASSURANCE_TOOLCHAIN" "${extractor_channel:-missing}" >&2
    return 1
  }
}

check_rustup_install() {
  local toolchain components component missing=0
  for toolchain in "$CODEFABRIC_STABLE_TOOLCHAIN" "$CODEFABRIC_ASSURANCE_TOOLCHAIN"; do
    if ! (cd /tmp && rustup run "$toolchain" rustc --version >/dev/null 2>&1); then
      printf 'missing Rustup toolchain: %s\n' "$toolchain" >&2
      missing=1
      continue
    fi
    components="$(cd /tmp && rustup component list --toolchain "$toolchain" --installed 2>/dev/null)"
    if [ "$toolchain" = "$CODEFABRIC_STABLE_TOOLCHAIN" ]; then
      required_components='rustfmt clippy rust-analyzer rust-src llvm-tools'
    else
      required_components='rustc-dev rust-src llvm-tools miri'
    fi
    for component in $required_components; do
      printf '%s\n' "$components" | grep -Eq "^${component}(-|$)" || {
        printf '%s missing component %s\n' "$toolchain" "$component" >&2
        missing=1
      }
    done
    [ "$mode" = report ] && printf '%-16s %s\n' rustup "$toolchain"
  done
  return "$missing"
}

if [ "$mode" = install ]; then
  command -v cargo-binstall >/dev/null 2>&1 || {
    printf 'cargo-binstall is required to run `just setup-tools`\n' >&2
    exit 1
  }
fi

failures=0
while IFS='|' read -r binary command_name package expected; do
  actual="$(command_version "$command_name" || true)"
  if [ "$actual" = "$expected" ]; then
    [ "$mode" = report ] && printf '%-16s %s\n' "$binary" "$actual"
    continue
  fi
  if [ "$mode" = install ]; then
    printf 'installing %s %s (found %s)\n' "$package" "$expected" "${actual:-missing}"
    cargo binstall --no-confirm --force "${package}@${expected}"
    actual="$(command_version "$command_name" || true)"
  fi
  if [ "$actual" != "$expected" ]; then
    printf '%s: expected %s, found %s\n' "$binary" "$expected" "${actual:-missing}" >&2
    failures=$((failures + 1))
  elif [ "$mode" = report ]; then
    printf '%-16s %s\n' "$binary" "$actual"
  fi
done < <(tool_rows)

uv_actual="$(command -v uv >/dev/null 2>&1 && uv --version 2>/dev/null | awk 'NR==1 {print $2}' || true)"
if [ "$uv_actual" != "$UV_VERSION" ]; then
  printf 'uv: expected %s, found %s; install the exact uv release before retrying\n' \
    "$UV_VERSION" "${uv_actual:-missing}" >&2
  failures=$((failures + 1))
elif [ "$mode" = report ]; then
  printf '%-16s %s\n' uv "$uv_actual"
fi

ensure_rustup_contract || failures=$((failures + 1))

if [ "$mode" = install ]; then
  rustup toolchain install "$CODEFABRIC_STABLE_TOOLCHAIN" --profile default \
    --component rustfmt,clippy,rust-analyzer,rust-src,llvm-tools-preview
  rustup toolchain install "$CODEFABRIC_ASSURANCE_TOOLCHAIN" --profile minimal \
    --component rustc-dev,rust-src,llvm-tools-preview,miri
fi

check_rustup_install || failures=$((failures + 1))

if [ "$failures" -ne 0 ]; then
  [ "$mode" = check ] && printf 'Run `just setup-tools` from a host shell to reconcile this contract.\n' >&2
  exit 1
fi

[ "$mode" = report ] || printf 'tool version contract passed\n'
