#!/usr/bin/env bash
#
# Section-level outline of the CodeFabric design specifications.
#
#   spec-outline                              map every spec by section
#   spec-outline <path>                       map one spec or directory
#   spec-outline <path> --view expanded       include subsections
#   spec-outline <path> --match '^36\.'       zoom to one section
#   spec-outline <path> --json=compact        machine-readable
#
# Items are `## N. Section`; members are `### N.N Subsection`. All ast-grep
# outline flags are forwarded, so --view/--items/--match/--type/--json work as
# documented in docs/library_ref/ast-grep_0.45.1_advanced_reference.md section 5.
#
# Sibling of lib-outline.sh, which maps the h1-rooted references in
# docs/library_ref. The two corpora are rooted differently and are not
# interchangeable; each script refuses the other's tree rather than emitting a
# misleading outline.
#
# This wrapper exists because markdown is a built-in language and `outlineRules`
# is only an sgconfig customLanguages field -- there is no registration path, so
# --outline-rules has to be passed on every invocation.

set -euo pipefail

CF_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
RULES="${CF_ROOT}/tooling/ast-grep/outline/specs.yml"
# The environment override exists only so the conformance test can prove that
# missing and empty default roots fail closed without mutating the repository.
DEFAULT_TARGET="${CODEFABRIC_SPEC_OUTLINE_DEFAULT:-docs/authoritative_design}"

if [ ! -f "$RULES" ]; then
  echo "spec-outline: missing extractor at $RULES" >&2
  exit 2
fi

if ! command -v ast-grep >/dev/null 2>&1; then
  echo "spec-outline: ast-grep not on PATH (run scripts/bootstrap.sh)" >&2
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
    # The library references are h1-rooted, so this extractor matches none of
    # their chapters and promotes their `##` subsections to items -- a flat wall
    # that silently omits whole appendix ranges. Refuse instead of misleading.
    case "$arg" in
    *library_ref*)
      echo "spec-outline: $arg is an h1-rooted library reference -- use 'lib-outline' instead" >&2
      exit 2
      ;;
    esac
    ;;
  esac
done

# ast-grep defaults a directory to --view names, which is tuned for short code
# symbols. Spec headings are long sentences, so `names` collapses 688 of them
# into comma-walls. `signatures` costs ~24% more but gives one line per section
# with the line number to seek to, which is what makes the outline navigable.
defaults=()
[ "$has_view" -eq 0 ] && defaults+=(--view signatures)

# ast-grep walks files in parallel, so the order of files in the output varies
# between runs on the same input. Over six specs that unpredictability costs more
# (spurious churn when diffing two outlines) than the ~60ms that serial traversal
# saves. Override with -j if pointing this at a large tree.
[ "$has_threads" -eq 0 ] && defaults+=(--threads 1)

if [ "$has_path" -eq 1 ]; then
  exec ast-grep outline --outline-rules "$RULES" -l markdown "${defaults[@]}" "$@"
fi

# No path given: run from the repo root so the default resolves relatively and
# the reported paths stay short.
cd "$CF_ROOT"
if [ ! -d "$DEFAULT_TARGET" ]; then
  echo "spec-outline: authoritative design root does not exist: $DEFAULT_TARGET" >&2
  exit 2
fi
shopt -s nullglob
specs=("$DEFAULT_TARGET"/*.md)
shopt -u nullglob
if [ "${#specs[@]}" -eq 0 ]; then
  echo "spec-outline: authoritative design root contains no Markdown masters: $DEFAULT_TARGET" >&2
  exit 2
fi
exec ast-grep outline --outline-rules "$RULES" -l markdown "${defaults[@]}" "$@" \
  "$DEFAULT_TARGET"
