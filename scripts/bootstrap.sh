#!/usr/bin/env bash
#
# CodeFabric environment bootstrap. Works under bash and zsh, in interactive and
# non-interactive shells.
#
#   source scripts/bootstrap.sh      apply the environment to the current shell
#   ./scripts/bootstrap.sh           check the environment and print a report
#   ./scripts/bootstrap.sh --quiet   print only problems; silent when healthy
#   ./scripts/bootstrap.sh --context compact one-block summary for agent context
#
# Why this exists alongside .envrc: direnv only hooks interactive shells, and
# LLM agent harnesses typically run each command in a fresh non-interactive
# shell that inherits nothing from the previous one. This file is the mechanism
# both can share -- .envrc applies it for humans, agents source it per-command
# or invoke tools through `uv run` / `direnv exec .`.

# ---------------------------------------------------------------- detect mode

_cf_sourced=0
if [ -n "${ZSH_VERSION:-}" ]; then
  case "${ZSH_EVAL_CONTEXT:-}" in *:file) _cf_sourced=1 ;; esac
elif [ -n "${BASH_VERSION:-}" ]; then
  (return 0 2>/dev/null) && _cf_sourced=1
fi

# Repo root, resolved from this file rather than the caller's cwd.
if [ -n "${ZSH_VERSION:-}" ]; then
  _cf_self="${(%):-%x}"
else
  _cf_self="${BASH_SOURCE[0]:-$0}"
fi
CF_ROOT="$(cd "$(dirname "$_cf_self")/.." && pwd)"

# --------------------------------------------------------------- environment

cf_apply_env() {
  export CF_ROOT
  export UV_PROJECT_ENVIRONMENT="${CF_ROOT}/.venv"

  if [ -d "${UV_PROJECT_ENVIRONMENT}/bin" ]; then
    export VIRTUAL_ENV="${UV_PROJECT_ENVIRONMENT}"
    case ":${PATH}:" in
      *":${VIRTUAL_ENV}/bin:"*) ;;
      *) PATH="${VIRTUAL_ENV}/bin:${PATH}"; export PATH ;;
    esac
  fi
}

# ------------------------------------------------------------------- checks

# Anchors. Sources:
#   Python floor        pyproject.toml requires-python
#   Rust channel        rust-toolchain.toml (stable-first, repo spec section 10)
#
# There is deliberately no nightly pin and no rustc-dev requirement. Repo spec section 76:
# nightly is a *targeted analysis toolchain* for `just miri` and `just udeps` only, and
# declaring rustc-dev would couple this repository to compiler-private APIs. Its absence
# is reported below as information, never as a failure.
CF_PY_MIN="3.14"
CF_STABLE_COMPONENTS="rustfmt clippy rust-src llvm-tools"

# Tools the repository contract depends on. sccache is required because
# .cargo/config.toml commits `rustc-wrapper = "sccache"` (repo spec section 13.1), so a
# missing sccache breaks every cargo invocation rather than merely slowing it.
CF_REQUIRED_TOOLS="just sccache cargo-nextest maturin typos rg ast-grep"
CF_GATE_TOOLS="cargo-deny cargo-audit cargo-shear cargo-machete"

_cf_ok=0; _cf_warn=0; _cf_bad=0
_cf_lines=""

_cf_say() { _cf_lines="${_cf_lines}$1
"; }
_cf_pass() { _cf_ok=$((_cf_ok + 1)); [ "$CF_MODE" = quiet ] || _cf_say "  ok    $1"; }
_cf_warns() { _cf_warn=$((_cf_warn + 1)); _cf_say "  warn  $1"; }
_cf_fail() { _cf_bad=$((_cf_bad + 1)); _cf_say "  FAIL  $1"; }

# True when $1 >= $2 under version ordering.
_cf_vge() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -1)" = "$2" ]; }

cf_check() {
  local v

  # --- Python -------------------------------------------------------------
  if command -v uv >/dev/null 2>&1; then
    _cf_pass "uv $(uv --version 2>/dev/null | awk '{print $2}')"
  else
    _cf_fail "uv not on PATH -- required to build the Python environment"
  fi

  if [ -x "${CF_ROOT}/.venv/bin/python" ]; then
    v="$("${CF_ROOT}/.venv/bin/python" -c 'import platform;print(platform.python_version())' 2>/dev/null)"
    if _cf_vge "$v" "$CF_PY_MIN"; then
      _cf_pass ".venv python $v"
    else
      _cf_fail ".venv python $v is below the ${CF_PY_MIN} floor in pyproject.toml"
    fi
  else
    _cf_fail ".venv missing -- run 'uv sync'"
  fi

  # --- Rust: stable is the working toolchain ------------------------------
  if command -v rustup >/dev/null 2>&1; then
    v="$(rustup show active-toolchain 2>/dev/null | head -1)"
    case "$v" in
      stable*) _cf_pass "active toolchain ${v%% *} ($(rustc --version 2>/dev/null | awk '{print $2}'))" ;;
      "") _cf_fail "no active toolchain resolved -- is rust-toolchain.toml readable?" ;;
      *) _cf_warns "active toolchain is ${v%% *}; rust-toolchain.toml pins stable" ;;
    esac

    local installed missing=""
    installed="$(rustup component list --toolchain stable --installed 2>/dev/null)"
    for c in $CF_STABLE_COMPONENTS; do
      case "$installed" in *"$c"*) ;; *) missing="${missing} ${c}" ;; esac
    done
    if [ -n "$missing" ]; then
      _cf_fail "stable missing components:${missing} -- see rust-toolchain.toml"
    else
      _cf_pass "stable components: ${CF_STABLE_COMPONENTS// /, }"
    fi

    # Informational only. Nightly is needed for `just miri` and `just udeps`; ordinary
    # development never touches it.
    if rustup run nightly rustc --version >/dev/null 2>&1; then
      _cf_pass "nightly available for miri/udeps ($(rustup run nightly rustc --version 2>/dev/null | awk '{print $2}'))"
    else
      _cf_say "  note  no nightly toolchain -- only 'just miri' and 'just udeps' need one"
    fi
  else
    _cf_fail "rustup not on PATH -- the Rust core cannot be built"
  fi

  # --- Repository tooling contract ----------------------------------------
  local absent=""
  for t_ in $CF_REQUIRED_TOOLS; do
    command -v "$t_" >/dev/null 2>&1 || absent="${absent} ${t_}"
  done
  if [ -n "$absent" ]; then
    _cf_fail "required tools missing:${absent} -- install with 'cargo binstall'"
  else
    _cf_pass "required tools present: ${CF_REQUIRED_TOOLS// /, }"
  fi

  absent=""
  for t_ in $CF_GATE_TOOLS; do
    command -v "$t_" >/dev/null 2>&1 || absent="${absent} ${t_}"
  done
  if [ -n "$absent" ]; then
    _cf_warns "gate tools missing:${absent} -- 'just policy' and 'just deps-fast' will fail"
  else
    _cf_pass "gate tools present: ${CF_GATE_TOOLS// /, }"
  fi

  # sccache is enabled by a committed wrapper; report whether it is actually earning its
  # place rather than assuming it is (repo spec section 13.2).
  if command -v sccache >/dev/null 2>&1; then
    local rate
    rate="$(sccache --show-stats 2>/dev/null | awk -F'  +' '/Cache hits rate/ {print $2; exit}')"
    [ -n "$rate" ] && _cf_say "  note  sccache hit rate ${rate}  ('just cache-stats' for detail)"
  fi

  # --- Research tooling (see _shared/code-intelligence.md) ----------------
  # The skills' navigation guidance depends on `outline`, added in 0.44. Its
  # absence means an older CLI surface than the guidance assumes.
  if command -v ast-grep >/dev/null 2>&1 &&
    ! ast-grep outline --help >/dev/null 2>&1; then
    _cf_fail "ast-grep lacks 'outline' -- pre-0.44 CLI, navigation guidance will not work"
  fi

  # --- direnv -------------------------------------------------------------
  if command -v direnv >/dev/null 2>&1; then
    if [ ! -f "${CF_ROOT}/.envrc" ]; then
      _cf_warns ".envrc missing"
    else
      # direnv status prints "Found RC allowed <n>": 0 allowed, 1 not allowed, 2 denied.
      local allowed
      allowed="$(cd "$CF_ROOT" && direnv status 2>/dev/null |
        awk '/Found RC allowed/ {print $NF; exit}')"
      case "$allowed" in
        0 | true) _cf_pass "direnv $(direnv --version) (.envrc allowed)" ;;
        2) _cf_warns ".envrc is denied in direnv -- run 'direnv allow' to re-enable" ;;
        *) _cf_warns ".envrc not yet allowed -- run 'direnv allow' (interactive shells only)" ;;
      esac
    fi
  fi
}

# ---------------------------------------------------------------- dispatch

if [ "$_cf_sourced" = 1 ]; then
  cf_apply_env
  unset _cf_sourced _cf_self
  return 0 2>/dev/null || true
else
  set -uo pipefail
  CF_MODE=report
  case "${1:-}" in
    --quiet) CF_MODE=quiet ;;
    --context) CF_MODE=context ;;
    --help | -h)
      sed -n '3,16p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
  esac

  cf_apply_env
  cf_check

  if [ "$CF_MODE" = context ]; then
    printf 'CodeFabric environment (%s):\n%s' "$CF_ROOT" "$_cf_lines"
    printf 'Shell state does not persist between agent commands. Run project tools as\n'
    printf '`uv run <cmd>` (Python) or `direnv exec . <cmd>` (full .envrc env); or source\n'
    printf 'scripts/bootstrap.sh within a single compound command.\n'
    exit 0
  fi

  if [ "$CF_MODE" = quiet ] && [ "$_cf_bad" = 0 ] && [ "$_cf_warn" = 0 ]; then
    exit 0
  fi

  printf 'CodeFabric environment  %s\n%s' "$CF_ROOT" "$_cf_lines"
  printf '  --\n  %d ok, %d warn, %d fail\n' "$_cf_ok" "$_cf_warn" "$_cf_bad"
  [ "$_cf_bad" = 0 ] || exit 1
  exit 0
fi
