#!/usr/bin/env bash
#
# Chapter-level outline of the CodeFabric library references.
#
#   lib-outline                                    map every reference by chapter
#   lib-outline <path>                             map one reference or directory
#   lib-outline <path> --view expanded             include subsections
#   lib-outline <path> --match '^Appendix M'       zoom to one chapter
#   lib-outline <path> --json=compact              machine-readable
#
# Items are `# Chapter`; members are `## N.M Subsection`. All ast-grep outline
# flags are forwarded, so --view/--items/--match/--type/--json work as documented
# in docs/library_ref/ast-grep_0.45.1_advanced_reference.md section 5.
#
# Sibling of spec-outline.sh, which maps the h2-rooted specs in docs/authoritative_design.
# The two corpora are rooted differently and are not interchangeable; each script
# refuses the other's tree rather than emitting a misleading outline.
#
# Note the coordinate convention: the rendered `NNNN:` prefix is 1-based and can be
# passed straight to Read/sed, but `range.start.line` in --json output is 0-based.
#
# This wrapper exists because markdown is a built-in language and `outlineRules`
# is only an sgconfig customLanguages field -- there is no registration path, so
# --outline-rules has to be passed on every invocation.

set -euo pipefail

CF_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
RULES="${CF_ROOT}/tooling/ast-grep/outline/library-ref.yml"
DEFAULT_PATH="docs/library_ref"

if [ ! -f "$RULES" ]; then
  echo "lib-outline: missing extractor at $RULES" >&2
  exit 2
fi

if ! command -v ast-grep >/dev/null 2>&1; then
  echo "lib-outline: ast-grep not on PATH (run scripts/bootstrap.sh)" >&2
  exit 2
fi

# Forward everything, but supply the default path when the caller passed only
# flags -- ast-grep would otherwise default to the current directory.
#
# Detecting "did they give a path" means skipping the values of flags that take
# one separately (`--view expanded`, not just `--view=expanded`), or the value
# would be mistaken for a path.
has_path=0
has_view=0
has_threads=0
skip_next=0
for arg in "$@"; do
  if [ "$skip_next" -eq 1 ]; then
    skip_next=0
    continue
  fi
  case "$arg" in
  --view | --view=*) has_view=1; [ "$arg" = "--view" ] && skip_next=1 ;;
  --threads | --threads=* | -j) has_threads=1; case "$arg" in --threads | -j) skip_next=1 ;; esac ;;
  --items | --match | --type | --lang | -l | --outline-rules | --globs | \
    --color | --config | -c)
    skip_next=1
    ;;
  -*) ;; # --flag=value, or a value-less flag such as --pub-members
  *)
    has_path=1
    # Pointing this at the h2-rooted specs produces items for their `###`
    # subsections and nothing for their `##` sections -- structurally wrong in
    # the same way specs.yml is wrong here. Refuse instead of misleading.
    case "$arg" in
    *upfront_design*)
      echo "lib-outline: $arg is an h2-rooted design spec -- use 'spec-outline' instead" >&2
      exit 2
      ;;
    esac
    ;;
  esac
done

# ast-grep defaults a directory to --view names, which is tuned for short code
# symbols. Reference chapter headings are long sentences, so `names` collapses
# them into comma-walls. `signatures` gives one line per chapter with the line
# number to seek to, which is what makes the outline navigable.
defaults=()
[ "$has_view" -eq 0 ] && defaults+=(--view signatures)

# ast-grep walks files in parallel, so the order of files in the output varies
# between runs on the same input. Over two dozen references that unpredictability
# costs more (spurious churn when diffing two outlines) than serial traversal
# saves. Override with -j if you only care about wall-clock.
[ "$has_threads" -eq 0 ] && defaults+=(--threads 1)

if [ "$has_path" -eq 1 ]; then
  exec ast-grep outline --outline-rules "$RULES" -l markdown "${defaults[@]}" "$@"
fi

# No path given: run from the repo root so the default resolves relatively and
# the reported paths stay short.
cd "$CF_ROOT"
exec ast-grep outline --outline-rules "$RULES" -l markdown "${defaults[@]}" "$@" \
  "$DEFAULT_PATH"
