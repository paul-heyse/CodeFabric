#!/usr/bin/env bash
#
# CodeFabric environment verifier and session-context reporter.
#
#   ./scripts/bootstrap.sh           check the environment and print a report
#   ./scripts/bootstrap.sh --quiet   print only problems; silent when healthy
#   ./scripts/bootstrap.sh --context compact one-block summary for agent context
#   ./scripts/bootstrap.sh --baseline run the gate and cache the verdict for --context
#
# It deliberately does not activate Python, alter PATH, or select a persistent Rust
# toolchain. `just` recipes use scripts/repo-shell.sh and are self-contained in fresh
# non-interactive shells; .envrc only adds interactive conveniences.

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
CF_ADAPTER_ROOT="${CF_ROOT}/codefabric-cpg-mcp"
CF_UV_CACHE_DIR="${CF_ROOT}/target/uv-cache"

# ------------------------------------------------------------------- checks

# The root is the stable daemon/data plane. The nightly extractor, Pyrefly sidecar, and
# FastMCP adapter are independent build domains added by WP02-WP04.
CF_STABLE_COMPONENTS="rustfmt clippy rust-src llvm-tools"

# Tools the repository contract depends on. sccache is required because
# .cargo/config.toml commits the repository sccache wrapper (repo spec section 13.1), so
# a missing or unhealthy supervised service breaks Cargo rather than silently slowing it.
CF_REQUIRED_TOOLS="just sccache cargo-nextest typos rg ast-grep jq uv"
CF_GATE_TOOLS="cargo-deny cargo-audit cargo-shear cargo-machete"

_cf_ok=0; _cf_warn=0; _cf_bad=0
_cf_lines=""

_cf_say() { _cf_lines="${_cf_lines}$1
"; }
_cf_pass() { _cf_ok=$((_cf_ok + 1)); [ "$CF_MODE" = quiet ] || _cf_say "  ok    $1"; }
_cf_warns() { _cf_warn=$((_cf_warn + 1)); _cf_say "  warn  $1"; }
_cf_fail() { _cf_bad=$((_cf_bad + 1)); _cf_say "  FAIL  $1"; }

cf_check() {
  local v

  # --- Rust: stable is the working toolchain ------------------------------
  if command -v rustup >/dev/null 2>&1; then
    v="$(cd "$CF_ROOT" && rustup show active-toolchain 2>/dev/null | head -1)"
    case "$v" in
      stable*) _cf_pass "active toolchain ${v%% *} ($(rustup run stable rustc --version 2>/dev/null | awk '{print $2}'))" ;;
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

    # WP02 turns the dated nightly into the extractor domain's production toolchain.
    # Until that root exists, nightly availability is informational only.
    if rustup run nightly rustc --version >/dev/null 2>&1; then
      _cf_pass "nightly available for extractor/assurance ($(rustup run nightly rustc --version 2>/dev/null | awk '{print $2}'))"
    else
      _cf_say "  note  no nightly toolchain -- WP02 will install the dated extractor pin"
    fi

    local stable_cargo extractor_rustc
    stable_cargo="$(rustup which cargo --toolchain stable 2>/dev/null)"
    extractor_rustc="$(rustup which rustc --toolchain nightly-2026-08-18 2>/dev/null)"
    if [ -n "$stable_cargo" ] && [ -n "$extractor_rustc" ]; then
      _cf_say "  note  Rust executables: stable cargo=${stable_cargo}; extractor rustc=${extractor_rustc}"
    fi
  else
    _cf_fail "rustup not on PATH -- the Rust core cannot be built"
  fi

  # --- Four independent build domains ------------------------------------
  _cf_pass "stable domain: root Cargo package"
  if [ -f "${CF_ROOT}/rustc-extractor/Cargo.toml" ]; then
    local extractor_rust
    extractor_rust="$(rustup run nightly-2026-08-18 rustc --version 2>/dev/null)"
    case "$extractor_rust" in
      *nightly*2026*) _cf_pass "rustc extractor domain: ${extractor_rust}" ;;
      *) _cf_fail "rustc extractor toolchain did not resolve its dated nightly" ;;
    esac
  else
    _cf_say "  note  rustc extractor domain arrives in WP02"
  fi
  if [ -f "${CF_ROOT}/pyrefly-sidecar/Cargo.toml" ]; then
    if grep -q '"pyrefly_version"' "${CF_ROOT}/pyrefly-sidecar/toolchain-identity.json" 2>/dev/null; then
      _cf_pass "Pyrefly sidecar domain and identity present"
    else
      _cf_fail "Pyrefly sidecar identity is missing"
    fi
  else
    _cf_say "  note  Pyrefly sidecar domain arrives in WP03"
  fi
  if [ -f "${CF_ROOT}/codefabric-cpg-mcp/pyproject.toml" ]; then
    local adapter_python
    adapter_python="$(env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT -u PYTHONPATH \
      UV_CACHE_DIR="$CF_UV_CACHE_DIR" uv run --frozen --project "$CF_ADAPTER_ROOT" \
      python --version 2>/dev/null)"
    case "$adapter_python" in
      "Python 3.14.7")
        _cf_pass "FastMCP adapter domain: ${adapter_python}"
        _cf_say "  note  uv project=${CF_ADAPTER_ROOT}; cache=${CF_UV_CACHE_DIR}"
        ;;
      *) _cf_fail "FastMCP adapter did not resolve locked Python 3.14.7" ;;
    esac
  else
    _cf_say "  note  FastMCP adapter domain arrives in WP04"
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

  # Structural and textual claims are only valid for the tool version that produced
  # them, and the skills require that version to be recorded alongside the claim
  # (.claude/skills/_shared/evidence-policy.md section 5). Report it so no session
  # has to shell out for it.
  local sg_v rg_v
  sg_v="$(ast-grep --version 2>/dev/null | awk '{print $NF}')"
  rg_v="$(rg --version 2>/dev/null | awk 'NR==1{print $2}')"
  [ -n "${sg_v}${rg_v}" ] && _cf_say "  note  search tooling: ast-grep ${sg_v:-?}, rg ${rg_v:-?}"

  absent=""
  for t_ in $CF_GATE_TOOLS; do
    command -v "$t_" >/dev/null 2>&1 || absent="${absent} ${t_}"
  done
  if [ -n "$absent" ]; then
    _cf_warns "gate tools missing:${absent} -- 'just policy' and 'just deps-fast' will fail"
  else
    _cf_pass "gate tools present: ${CF_GATE_TOOLS// /, }"
  fi

  # Validate current service health, then report cumulative lookup and non-cacheable
  # telemetry without presenting a hit percentage as repository performance proof.
  if [ -x "${CF_ROOT}/scripts/sccache-service.sh" ]; then
    local sccache_health sccache_json rust_hits rust_misses rust_rate non_cacheable_calls
    if sccache_health="$("${CF_ROOT}/scripts/sccache-service.sh" doctor 2>&1)"; then
      _cf_pass "$sccache_health"
      sccache_json="$("${CF_ROOT}/scripts/sccache-service.sh" stats-json 2>/dev/null)"
      rust_hits="$(printf '%s' "$sccache_json" | jq -r '.stats.cache_hits.counts.Rust // 0')"
      rust_misses="$(printf '%s' "$sccache_json" | jq -r '.stats.cache_misses.counts.Rust // 0')"
      non_cacheable_calls="$(printf '%s' "$sccache_json" | jq -r '.stats.requests_not_cacheable // 0')"
      if [ $((rust_hits + rust_misses)) -gt 0 ]; then
        rust_rate="$(awk -v h="$rust_hits" -v m="$rust_misses" 'BEGIN { printf "%.2f", 100*h/(h+m) }')"
        _cf_say "  note  sccache cumulative Rust lookups ${rust_hits} hit / ${rust_misses} miss (${rust_rate}% among lookups); non-cacheable calls ${non_cacheable_calls}"
      else
        _cf_say "  note  sccache cumulative Rust lookups pending; non-cacheable calls ${non_cacheable_calls}"
      fi
      _cf_say "  note  use 'just sccache-effectiveness' for Cargo-shaped performance evidence"
    else
      _cf_fail "$sccache_health"
    fi
  fi

  # Randomly named test sockets cannot be safely allowlisted. Confirm the three
  # exact, non-mutating recipes that are permitted to leave the Codex sandbox.
  if command -v codex >/dev/null 2>&1 && [ -f "${CF_ROOT}/.codex/rules/codefabric.rules" ]; then
    local uds_recipe uds_decision uds_rules_ok=1
    for uds_recipe in adapter-test root-test ci-fast environment-regression; do
      uds_decision="$(codex execpolicy check \
        --rules "${CF_ROOT}/.codex/rules/codefabric.rules" \
        -- just "$uds_recipe" 2>/dev/null | jq -r '.decision // ""')"
      [ "$uds_decision" = allow ] || uds_rules_ok=0
    done
    if [ "$uds_rules_ok" = 1 ]; then
      _cf_pass "Codex UDS rules apply only to: adapter-test, root-test, ci-fast, environment-regression"
    else
      _cf_fail "Codex UDS test rules are missing or do not resolve to allow"
    fi
  else
    _cf_say "  note  Codex UDS rules not checked (Codex CLI unavailable)"
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

# ------------------------------------------------------- agent session context

# Everything below is emitted only by --context. It exists so an agent session opens
# already holding the facts it would otherwise spend its first several tool calls
# re-deriving -- the repository specification's section 59 "mandatory session
# bootstrap" list, the working-tree state three skills require before any change, the
# invariants that were previously restated in README.md, AGENTS.md and CLAUDE.md at
# once, and the dependency pins three library skills insist be read from the spec
# rather than from a quoted copy.
#
# Two rules govern what may go here:
#   1. Derived, never hand-copied. Every count, version and pin below is computed at
#      run time. A hand-maintained copy in a script is the same drift hazard as a
#      hand-maintained copy in prose.
#   2. Facts, not judgment. Decision procedures -- the section 60 change-risk table,
#      the doctrine, the design-corpus map -- stay in the prose that owns them.

CF_BASELINE_FILE="${CF_ROOT}/target/agent/baseline.json"

# JSON scalar by key, without requiring jq.
_cf_json() {
  sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$1" 2>/dev/null | head -1
}

_cf_ctx_repo() {
  command -v git >/dev/null 2>&1 || return 0
  git -C "$CF_ROOT" rev-parse --git-dir >/dev/null 2>&1 || return 0

  local branch head count mod uns del state
  branch="$(git -C "$CF_ROOT" rev-parse --abbrev-ref HEAD 2>/dev/null)"
  head="$(git -C "$CF_ROOT" log -1 --format='%h %s' 2>/dev/null | cut -c1-64)"
  count="$(git -C "$CF_ROOT" rev-list --count HEAD 2>/dev/null)"
  mod="$(git -C "$CF_ROOT" status --porcelain 2>/dev/null | grep -c '^ *M' || true)"
  uns="$(git -C "$CF_ROOT" status --porcelain 2>/dev/null | grep -c '^??' || true)"
  del="$(git -C "$CF_ROOT" status --porcelain 2>/dev/null | grep -c '^ *D' || true)"

  if [ "$mod" = 0 ] && [ "$uns" = 0 ] && [ "$del" = 0 ]; then
    state="clean"
  else
    state="${mod} modified, ${uns} untracked, ${del} deleted -- PRE-EXISTING, not yours"
  fi

  printf 'REPO  %s at %s (%s commits)\n  tree  %s\n' \
    "${branch:-?}" "${head:-?}" "${count:-?}" "$state"

  # Section 59.1 wants a gate baseline with pre-existing failures recorded separately.
  # Report the cached verdict rather than making every session pay for the gate.
  if [ -f "$CF_BASELINE_FILE" ]; then
    local v when at now age
    v="$(_cf_json "$CF_BASELINE_FILE" verdict)"
    at="$(_cf_json "$CF_BASELINE_FILE" head)"
    when="$(_cf_json "$CF_BASELINE_FILE" utc)"
    now="$(date -u +%s 2>/dev/null)"
    age="$(_cf_json "$CF_BASELINE_FILE" epoch)"
    if [ -n "$now" ] && [ -n "$age" ]; then
      age=$(( (now - age) / 60 ))
      if [ "$age" -lt 90 ]; then age="${age}m ago"; else age="$(( age / 60 ))h ago"; fi
    else
      age="$when"
    fi
    local tree_at tree_now
    tree_at="$(_cf_json "$CF_BASELINE_FILE" tree)"
    tree_now="$({ git -C "$CF_ROOT" diff HEAD 2>/dev/null; git -C "$CF_ROOT" ls-files --others --exclude-standard 2>/dev/null; } | shasum -a 256 2>/dev/null | cut -c1-12)"
    if [ -n "$at" ] && [ "$at" != "${head%% *}" ]; then
      printf '  base  %s (%s) -- STALE, HEAD has moved since\n' "${v:-?}" "$age"
    elif [ -n "$tree_at" ] && [ "$tree_at" != "$tree_now" ]; then
      printf '  base  %s (%s) -- STALE, the working tree changed since\n' "${v:-?}" "$age"
    else
      printf '  base  %s (%s)\n' "${v:-?}" "$age"
    fi
  else
    printf '  base  not run -- ./scripts/bootstrap.sh --baseline\n'
  fi
}

_cf_ctx_gates() {
  local n
  n="$(command -v just >/dev/null 2>&1 && just --justfile "${CF_ROOT}/justfile" --summary 2>/dev/null | wc -w | tr -d ' ')"
  printf 'GATES  just --list is the operational API (%s recipes); prefer a recipe to raw flags\n' "${n:-?}"
  cat <<'EOF'
  ci-fast  all four domains + governance: run when the change risk warrants the aggregate
  root-check  default local + featureless stable root   root-test  nextest + doctests
  stable-graph-check  exact pins/features/local-vs-S3 activation boundary
  [mutating] root-fmt-write proto-gen typos-write snapshots-accept deps-fix
EOF
}

_cf_ctx_traps() {
  cat <<'EOF'
TRAPS  cargo nextest does not run doctests -- `just root-test` covers both
  --all-features is not a feature matrix -- use `just features-each`
  sccache is mandatory for compile-producing recipes; check/incremental modes are explicit
  adapter commands own codefabric-cpg-mcp/.venv; never reuse the root as a Python project
SEARCH  .claude/ is hidden: rg --hidden -g '!.git/**'
  docs/library_ref/ is large prose: exclude with -g '!docs/library_ref/**'
  exit codes differ: `ast-grep run` = 1 on clean no-match; `outline` = 0 on empty
  rg -uu / ast-grep --no-ignore reach .envrc.local, which holds a capability token
  invoke ast-grep, never the deprecated `sg` shim
EOF
}

_cf_ctx_corpus() {
  local specs current historical parts refs idx plans skills absent
  specs="$(ls "${CF_ROOT}"/docs/authoritative_design/*.md 2>/dev/null | wc -l | tr -d ' ')"
  current="$(rg -l '^authority_status: current$' "${CF_ROOT}"/docs/authoritative_design/*.md 2>/dev/null | wc -l | tr -d ' ')"
  historical="$((specs - current))"
  parts="$(awk '/^```/{f=!f;next} !f && /^# (Part|Appendix)/{p++} END{print p+0}' \
    "${CF_ROOT}"/docs/authoritative_design/*.md 2>/dev/null)"
  refs="$(ls "${CF_ROOT}"/docs/library_ref/*.md 2>/dev/null | wc -l | tr -d ' ')"
  idx="$(ls "${CF_ROOT}"/docs/spec_index/*.md 2>/dev/null | wc -l | tr -d ' ')"
  plans="$(ls "${CF_ROOT}"/docs/plans/*.md 2>/dev/null | wc -l | tr -d ' ')"
  skills="$(find "${CF_ROOT}/.claude/skills" -name SKILL.md 2>/dev/null | wc -l | tr -d ' ')"

  printf 'CORPUS  do not read these whole -- navigate them\n'
  printf '  docs/authoritative_design/  %s current + %s historical masters, %s `# Part`/`# Appendix` headings that\n' "$current" "$historical" "$parts"
  printf '        spec-outline structurally cannot emit (docs/spec_index/README.md §3.1)\n'
  printf '  docs/library_ref/  %s refs   docs/spec_index/  %s   docs/plans/  %s   skills  %s\n' \
    "$refs" "$idx" "$plans" "$skills"

  # Cited by the routing layer but absent. Derived, so it cannot go stale.
  if command -v rg >/dev/null 2>&1; then
    absent="$(rg -oI 'docs/library_ref/[A-Za-z0-9_.-]+\.md' --hidden -g '!.git/**' \
      "${CF_ROOT}/.claude/skills" "${CF_ROOT}/docs/spec_index" "${CF_ROOT}/docs/library_ref" 2>/dev/null |
      sed 's|.*docs/library_ref/||' | sort -u |
      while read -r f; do [ -f "${CF_ROOT}/docs/library_ref/$f" ] || printf '%s ' "${f%.md}"; done)"
    absent="$(printf '%s' "$absent" | sed 's/ *$//')"
    [ -n "$absent" ] && printf '  cited but ABSENT from docs/library_ref/: %s\n' "$absent"
  fi

  cat <<'EOF'
  spec-outline (~0.3s) · lib-outline -j 8 (~0.5s) · ast-grep outline <dir> for code
  cite specs as TAG §N with the section title, never a line number, and confirm with
  `spec-outline <spec>.md --match '^N\.'` -- section numbers move between revisions
  docs/spec_index/ is navigation only, NEVER normative: cite the section it points at
EOF
}

_cf_ctx_pins() {
  local f blk candidate arrow datafusion object_store delta rust edition
  f=""
  for candidate in "${CF_ROOT}"/docs/authoritative_design/*.md; do
    if awk '
      /^artifact_tag: FAB$/ { tag = 1 }
      /^authority_status: current$/ { current = 1 }
      END { exit !(tag && current) }
    ' "$candidate"; then
      f="$candidate"
      break
    fi
  done
  if [ -z "$f" ]; then
    printf 'PINS  read them from the data-fabric spec §2.1 (that spec was not found here)\n'
    return 0
  fi
  blk="$(awk '/^### 2\.1 /{s=1; next} s&&/^### /{exit} s' "$f" 2>/dev/null)"
  # Degrade to a pointer rather than print a stale or partial pin set.
  case "$blk" in
    *DataFusion*Arrow*Parquet*) ;;
    *) printf 'PINS  read them from the data-fabric spec §2.1 (extraction failed here)\n'; return 0 ;;
  esac
  arrow="$(printf '%s\n' "$blk" | awk -F'|' '/Arrow and Parquet/{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $3); print $3}')"
  datafusion="$(printf '%s\n' "$blk" | awk -F'|' '/DataFusion/{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $3); print $3}')"
  object_store="$(printf '%s\n' "$blk" | awk -F'|' '/`object_store`/{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $3); print $3}')"
  delta="$(printf '%s\n' "$blk" | sed -n 's/.*revision `\([0-9a-f][0-9a-f]*\)`.*/\1/p')"
  rust="$(printf '%s\n' "$blk" | awk -F'|' '/Rust toolchain floor/{gsub(/^[[:space:]]+|[[:space:]]+$/, "", $3); sub(/,.*/, "", $3); print $3}')"
  edition="$(printf '%s\n' "$blk" | sed -n 's/.*edition \([0-9][0-9]*\).*/\1/p')"
  if [ -z "$arrow" ] || [ -z "$datafusion" ] || [ -z "$object_store" ] || [ -z "$delta" ] || [ -z "$rust" ] || [ -z "$edition" ]; then
    printf 'PINS  read them from the data-fabric spec §2.1 (extraction failed here)\n'
    return 0
  fi
  printf 'PINS  from the data-fabric spec §2.1 -- authoritative; never trust a quoted copy\n'
  printf '  arrow/parquet =%s · datafusion =%s · object_store =%s\n' "$arrow" "$datafusion" "$object_store"
  printf '  deltalake git %.8s (pre-release pin) · rust %s · edition %s · resolver 3\n' "$delta" "$rust" "$edition"
}

cf_context_extras() {
  printf '\n'
  _cf_ctx_repo
  printf '\n'
  _cf_ctx_gates
  _cf_ctx_traps
  _cf_ctx_corpus
  _cf_ctx_pins
  cat <<'EOF'
NEXT  repo-spec §59's session-bootstrap list is answered above. Classify the change
  against its §60 risk table before reaching for an expensive tool.
EOF
}

# Run the routine gate once and cache the verdict, so every later session can read
# section 59.1's "pre-existing failures" without paying for the gate again.
cf_baseline() {
  command -v just >/dev/null 2>&1 || { printf 'just is not on PATH\n' >&2; return 1; }
  printf 'Running `just ci-fast` against the current stable-root tree.\n\n'

  local log rc head verdict
  mkdir -p "${CF_ROOT}/target/agent"
  log="${CF_ROOT}/target/agent/baseline.log"
  ( cd "$CF_ROOT" && just ci-fast ) >"$log" 2>&1
  rc=$?
  if [ "$rc" -ge 128 ]; then
    printf 'baseline cancelled (exit %s); no verdict was recorded\n' "$rc" >&2
    return "$rc"
  fi
  [ "$rc" = 0 ] && verdict=green || verdict=red
  head="$(git -C "$CF_ROOT" rev-parse --short HEAD 2>/dev/null)"
  # A baseline is only evidence about the tree it ran against, so fingerprint the
  # working tree too -- HEAD alone would call a baseline current across uncommitted edits.
  local tree
  tree="$({ git -C "$CF_ROOT" diff HEAD 2>/dev/null; git -C "$CF_ROOT" ls-files --others --exclude-standard 2>/dev/null; } | shasum -a 256 2>/dev/null | cut -c1-12)"

  cat >"$CF_BASELINE_FILE" <<EOF
{
  "verdict": "$verdict",
  "recipe": "just ci-fast",
  "exit": $rc,
  "head": "${head:-unknown}",
  "tree": "${tree:-unknown}",
  "utc": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "epoch": "$(date -u +%s)",
  "log": "target/agent/baseline.log"
}
EOF
  printf 'baseline %s (exit %s) -- %s\n' "$verdict" "$rc" "$CF_BASELINE_FILE"
  [ "$verdict" = red ] && printf 'failures are PRE-EXISTING for later sessions; see %s\n' "$log"
  return 0
}

# ---------------------------------------------------------------- dispatch

if [ "$_cf_sourced" = 1 ]; then
  unset _cf_sourced _cf_self
  return 0 2>/dev/null || true
else
  set -uo pipefail
  CF_MODE=report
  case "${1:-}" in
    --quiet) CF_MODE=quiet ;;
    --context) CF_MODE=context ;;
    --baseline) CF_MODE=baseline ;;
    --help | -h)
      # Print the leading comment block. Delimiter-driven, not line-numbered, so
      # editing the header cannot silently corrupt --help.
      awk 'NR>1 && /^#/ {sub(/^# ?/, ""); print; next} NR>1 {exit}' "$0"
      exit 0
      ;;
  esac

  cf_check

  if [ "$CF_MODE" = baseline ]; then
    cf_baseline
    exit $?
  fi

  if [ "$CF_MODE" = context ]; then
    printf 'CodeFabric session context (%s)\nENVIRONMENT  %d ok, %d warn, %d fail\n%s' \
      "$CF_ROOT" "$_cf_ok" "$_cf_warn" "$_cf_bad" "$_cf_lines"
    printf 'SHELL  repository recipes are self-contained in fresh non-interactive shells\n'
    printf '  use `just <recipe>` directly; do not source bootstrap or invoke direnv routinely\n'
    cf_context_extras
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
