#!/usr/bin/env bash
#
# Fixture test for the spec outline extractors.
#
# The markdown outline surface is alpha-era and grammar-dependent: an ast-grep or
# tree-sitter-markdown upgrade can silently change node kinds, nesting, or
# extraction and leave the outline quietly returning less. This asserts the exact
# shape the project depends on so that fails loudly instead.
#
#   ./tooling/ast-grep/outline/specs.test.sh
#
# Exits 0 on pass, 1 on any assertion failure.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/../../.." && pwd)"
RULES="${ROOT}/tooling/ast-grep/outline/specs.yml"
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

# The fixture is inline rather than a committed .md so it cannot be picked up by
# spec-outline itself or by any repository-wide markdown scan.
FIXTURE=$(
  cat <<'MD'
# Document Title

Preamble prose that belongs to no section.

## 1. Purpose

Body.

### 1.1 First sub

Body.

### 1.2 Second sub

Body.

#### 1.2.1 Deep

Body.

## 2. Scope

Body with a fenced block that must not be mistaken for a heading:

```python
## not a heading
### also not a heading
```

## 3. `Quoted` and — punctuated

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

echo "spec outline extractor"

# Items are h2 only: h1 title, h3 subsections, h4, and fenced-code lines excluded.
check "item names" \
  "1. Purpose|2. Scope|3. \`Quoted\` and — punctuated" \
  "$(items '[.[].items[].name] | join("|")')"

check "all items are role=item" \
  "item" \
  "$(items '[.[].items[].role] | unique | join(",")')"

check "item symbolType" \
  "namespace" \
  "$(items '[.[].items[].symbolType] | unique | join(",")')"

# Members are the h3 subsections, attached to their containing h2.
check "members of section 1" \
  "1.1 First sub|1.2 Second sub" \
  "$(items '[.[].items[] | select(.name=="1. Purpose") | .members[]?.name] | join("|")')"

check "section 2 has no members" \
  "0" \
  "$(items '[.[].items[] | select(.name=="2. Scope") | .members[]?] | length')"

check "member symbolType" \
  "property" \
  "$(items '[.[].items[].members[]?.symbolType] | unique | join(",")')"

# h4 must not surface: outline exposes item + direct member only.
check "h4 not emitted as a member" \
  "false" \
  "$(items '[.[].items[].members[]?.name] | any(. == "1.2.1 Deep")')"

# Fenced code containing ## must not produce items.
check "fenced ## lines ignored" \
  "false" \
  "$(items '[.[].items[].name] | any(startswith("not a heading"))')"

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
