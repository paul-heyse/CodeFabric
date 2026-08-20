#!/usr/bin/env bash
#
# Fixture test for the library-reference outline extractors.
#
# Sibling of specs.test.sh. The markdown outline surface is alpha-era and
# grammar-dependent: an ast-grep or tree-sitter-markdown upgrade can silently
# change node kinds, nesting, or extraction and leave the outline quietly
# returning less. This asserts the exact shape the project depends on so that
# fails loudly instead.
#
#   ./tooling/ast-grep/outline/library-ref.test.sh
#
# Exits 0 on pass, 1 on any assertion failure.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/../../.." && pwd)"
RULES="${ROOT}/tooling/ast-grep/outline/library-ref.yml"
fail=0

check() { # check <label> <expected> <actual>
  if [ "$2" = "$3" ]; then
    echo "  ok    $1"
  else
    echo "  FAIL  $1"
    echo "          expected: $2"
    echo "          actual:   $3"
    fail=1
  fi
}

# The fixture reproduces the four structural traits of docs/library_ref that this
# extractor exists to handle: h1-rooted chapters, a prefixed chapter naming scheme,
# an internal table of contents whose entries look like body subsections, and
# fenced code containing lines that begin with `#`.
FIXTURE=$(
  cat <<'MD'
# Some Reference — advanced technical reference

## Version / source anchors

Pinned to 1.2.3.

```toml
# rust-toolchain.toml
[toolchain]
channel = "nightly-2026-08-18"
```

# Proposed comprehensive documentation map

## 0) Scope and mental model

Abstract for chapter 0.

## 25) Mapping into a CPG

Abstract for chapter 25.

# Ref Advanced — 0) Scope and mental model

## 0.0 Identity

Body.

## 0.1 Canonical pipeline

Body.

### 0.1.1 Deep

Body.

# Ref Advanced — 25) Mapping into a CPG

## 25.0 Raw facts

Body with a fenced block that must not be mistaken for a chapter:

```bash
# Cargo.toml
# not a chapter
```

# Appendix R — Implementation checklist

## R.1 Milestone 1

Body.
MD
)

json="$(printf '%s\n' "$FIXTURE" |
  ast-grep outline --stdin -l markdown --outline-rules "$RULES" --json=compact 2>/dev/null)"

if [ -z "$json" ]; then
  echo "  FAIL  extractor produced no output at all"
  exit 1
fi

items() { printf '%s' "$json" | jq -r "$1"; }

echo "library-ref outline extractor"

# Items are h1 only -- including the appendix, which is the heading level
# specs.yml cannot see at all.
check "item names" \
  "Some Reference — advanced technical reference|Proposed comprehensive documentation map|Ref Advanced — 0) Scope and mental model|Ref Advanced — 25) Mapping into a CPG|Appendix R — Implementation checklist" \
  "$(items '[.[].items[].name] | join("|")')"

check "appendix chapters are extracted" \
  "true" \
  "$(items '[.[].items[].name] | any(startswith("Appendix R"))')"

check "all items are role=item" \
  "item" \
  "$(items '[.[].items[].role] | unique | join(",")')"

check "item symbolType" \
  "namespace" \
  "$(items '[.[].items[].symbolType] | unique | join(",")')"

# Members are the h2 subsections, attached to their containing h1.
check "members of chapter 0" \
  "0.0 Identity|0.1 Canonical pipeline" \
  "$(items '[.[].items[] | select(.name | startswith("Ref Advanced — 0)")) | .members[]?.name] | join("|")')"

check "member symbolType" \
  "property" \
  "$(items '[.[].items[].members[]?.symbolType] | unique | join(",")')"

# The disambiguation this mapping buys: `25)` appears twice in the document, once
# as a TOC entry and once as a chapter. They land at different levels under
# different parents, which a flat heading grep cannot distinguish.
check "TOC entries attach to the doc map, not the body" \
  "0) Scope and mental model|25) Mapping into a CPG" \
  "$(items '[.[].items[] | select(.name=="Proposed comprehensive documentation map") | .members[]?.name] | join("|")')"

check "body chapter 25 is an item, not a doc-map member" \
  "true" \
  "$(items '[.[].items[].name] | any(. == "Ref Advanced — 25) Mapping into a CPG")')"

check "body chapter 25 owns its own subsection" \
  "25.0 Raw facts" \
  "$(items '[.[].items[] | select(.name | startswith("Ref Advanced — 25)")) | .members[]?.name] | join("|")')"

# h3 must not surface: outline exposes item + direct member only, and with h1 as
# item the member level is h2.
check "h3 not emitted as a member" \
  "false" \
  "$(items '[.[].items[].members[]?.name] | any(. == "0.1.1 Deep")')"

# Fenced code containing `#` must not produce chapters. This is the property that
# makes the extractor safer than `rg "^# "` on these files.
check "fenced # lines ignored" \
  "false" \
  "$(items '[.[].items[].name] | any(test("rust-toolchain|Cargo\\.toml|not a chapter"))')"

# Ranges must be usable as seek coordinates.
check "items carry a start line" \
  "true" \
  "$(items '[.[].items[].range.start.line] | all(type == "number")')"

echo
if [ "$fail" -eq 0 ]; then
  echo "  all assertions passed"
else
  echo "  FAILURES -- the extractor or the markdown grammar has drifted"
fi
exit "$fail"
