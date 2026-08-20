# `rfc8785` in Python — advanced technical reference / JCS canonical byte serialization for LLM coding agents

## Version / source anchors

**Release anchor:** `rfc8785 == 0.1.4`, released 2024-09-27 and still current as of 2026-08-20. Pure Python, no runtime dependencies, Python >= 3.8, Apache-2.0.

Primary anchors:

- https://pypi.org/project/rfc8785/0.1.4/
- https://trailofbits.github.io/rfc8785.py/
- https://github.com/trailofbits/rfc8785.py
- https://www.rfc-editor.org/rfc/rfc8785

## CodeFabric role

`rfc8785` is the authoritative Python serializer for RFC 8785 canonical bytes. Python stdlib `json` remains responsible for strict ingress decoding because canonicalizing an already-materialized `dict` cannot reveal whether the original JSON source contained duplicate member names.

## Capability inventory

The package intentionally exposes a small, strong contract:

- `dumps(obj) -> bytes` canonicalizes an in-memory Python value;
- `dump(obj, sink)` writes canonical UTF-8 bytes directly to a binary sink;
- output is always minimal RFC 8785 JSON: no indentation / pretty mode;
- APIs operate in bytes, not `str` output;
- dictionary keys must already be `str`; non-string keys are not silently coerced;
- domain failures raise `CanonicalizationError` or subclasses;
- unsafe integers, non-finite floats, invalid string encoding/domain cases, and unsupported types are rejected.

## Installation

```bash
uv add 'rfc8785==0.1.4'
# or
python -m pip install 'rfc8785==0.1.4'
```

Keep the exact pin in a byte-level canonicalization protocol.

## `dumps`

```python
import rfc8785
canonical: bytes = rfc8785.dumps(value)
```

The `bytes` return type is exactly what should be passed into `blake3.blake3(...)` for CodeFabric fingerprints.

```python
canonical = rfc8785.dumps({"b": 1, "a": 2})
assert canonical == b'{"a":2,"b":1}'
```

Do not `.decode()` and re-encode unless a text API genuinely requires it.

## `dump`

```python
with open(path, "wb") as f:
    rfc8785.dump(value, f)
```

The sink must accept bytes. This API can avoid returning a final byte object to the caller, but JCS object sorting still implies internal work proportional to object content.

## Accepted Python data model

Use ordinary JSON-domain values:

- `None`;
- `bool`;
- `int` within the JCS safe/in-domain constraints;
- finite `float` values;
- `str`;
- list/tuple-like arrays supported by the implementation;
- dictionaries with string keys.

Do not rely on implicit conversion of custom classes, `Decimal`, dataclasses, UUIDs, paths, bytes, sets, or non-string dict keys. Convert application types into the canonical contract model explicitly before calling `dumps`.

## Number semantics

Python integers are unbounded, but JCS interoperability is not. The package rejects integers outside the safe interoperable domain rather than silently emitting arbitrary-width JSON integer text.

Python floats must be finite. NaN and infinity are invalid in RFC 8785.

CodeFabric is stricter in how input number tokens are admitted. Validate raw number tokens at the stdlib `json.loads` boundary before `rfc8785` receives materialized Python numbers.

## String and key semantics

`rfc8785` implements RFC string escaping and object-key sorting. Do not call `unicodedata.normalize`; JCS does not normalize Unicode. Strings that are not valid for the UTF-8/JCS domain should fail rather than be repaired.

Do not sort Python dictionaries yourself. Dict insertion order is not the RFC 8785 key order.

## Error model

All serialization failures derive from `rfc8785.CanonicalizationError`. The package exposes more specific domain errors for invalid integer/float/string situations in its implementation/API.

Application code should catch the narrowest useful exception at the canonicalization boundary and translate it into a stable validation error type. Keep original exception chaining for diagnostics.

```python
try:
    canonical = rfc8785.dumps(value)
except rfc8785.CanonicalizationError as exc:
    raise CanonicalizationFailure(...) from exc
```

## Why stdlib `json` is still required

This is invalid for strict protocol ingress:

```python
obj = json.loads(raw_text)  # duplicate keys may already be lost
canonical = rfc8785.dumps(obj)
```

Instead, configure `json.loads` with an `object_pairs_hook` that rejects duplicate names while each source object is still represented as a list of pairs, plus strict number hooks / `parse_constant` behavior.

The separation of responsibilities is deliberate:

```text
stdlib json decoder = source-text validation / duplicate evidence / token hooks
rfc8785            = RFC canonical byte serialization
```

## CodeFabric pipeline

```python
import json
import rfc8785
from blake3 import blake3

# pseudocode: strict_loads owns duplicate and numeric-token rejection
obj = strict_loads(source)
validate_codefabric_formats(obj)
canonical = rfc8785.dumps(obj)
digest = "b3:" + blake3(canonical).hexdigest()
```

## Cross-language parity

Rust `serde_json_canonicalizer` and Python `rfc8785` must produce identical bytes for the shared accepted value domain. Build fixtures around semantic values and expected canonical bytes, not just “Rust output equals Python output”; otherwise a correlated defect could pass.

Include RFC/JCS independent vectors, especially Unicode key ordering and Appendix B number cases.

## Performance guidance

The package is pure Python. For normal registry/config payload sizes, correctness dominates. If canonicalization becomes a measured bottleneck:

1. first eliminate redundant canonicalizations and dynamic conversions;
2. canonicalize only at identity/fingerprint boundaries;
3. cache only immutable semantic objects;
4. profile object sorting and conversion overhead separately;
5. do not replace the implementation with a merely sorted/minified JSON serializer.

## Anti-patterns

- `json.dumps(..., sort_keys=True, separators=(",", ":"))` as a substitute for RFC 8785;
- `orjson.dumps(..., OPT_SORT_KEYS)` as the canonical fingerprint serializer;
- canonicalizing after duplicate keys have collapsed into a dict;
- converting large integers to float to “make them work”;
- Unicode normalization before JCS;
- converting output bytes to text and adding newlines before hashing;
- custom serialization of unsupported application types inside the canonicalization function.

## Agent checklist

- [ ] pin is exactly 0.1.4;
- [ ] `dumps` output bytes are fed directly to BLAKE3;
- [ ] strict stdlib JSON decoding precedes canonicalization;
- [ ] duplicate names and unsafe number tokens are rejected at ingress;
- [ ] only JSON-domain values reach `rfc8785`;
- [ ] Unicode is not normalized;
- [ ] parity fixtures compare against Rust and independent RFC vectors.
## Testing matrix for agent-authored changes

At minimum, exercise:

- empty, scalar, nested-object, nested-array, and mixed-value cases;
- ASCII and non-ASCII object keys, including supplementary-plane code points;
- characters requiring JSON escaping and strings containing combining sequences;
- `0`, `-0.0`, integral floats, exponent forms, subnormals, largest finite doubles, and values adjacent to safe-integer boundaries;
- duplicate JSON member names at multiple nesting levels;
- malformed UTF-8 / malformed JSON where the API accepts raw bytes;
- deterministic repeated execution and cross-process execution;
- Rust↔Python byte equality for canonical bytes and BLAKE3 digest framing;
- intentionally malformed `codefabric-bytes`, `codefabric-int64`, `codefabric-uint64`, ID, and digest values;
- very deep / large inputs according to the caller's resource-limit policy.
## Upgrade and compatibility policy

For CodeFabric canonicalization dependencies, a dependency update is **not** an ordinary implementation-only change. Replay the complete shared positive and negative fixture corpus before accepting any upgrade. Positive fixtures must preserve canonical bytes and `b3:` digests byte-for-byte. Negative fixtures must preserve rejection of duplicate names, unsafe integer tokens, non-finite values, malformed typed-format strings, non-canonical base64, and uppercase IDs/digests. If a serializer upgrade changes canonical bytes for any previously accepted value, introduce a new canonicalization profile/version rather than silently changing `codefabric-jcs-v1`.

Agent rule: do not infer compatibility from SemVer alone when a dependency participates in a byte-level protocol.


---

# Extended capability catalog

## 1) Why this package exists in the stack

`rfc8785` is intentionally narrow: convert a JSON-compatible Python value into the unique byte representation required by RFC 8785 / JSON Canonicalization Scheme (JCS). It is not a parser, schema validator, duplicate-key detector, hashing package, or generic “fast JSON” library.

That narrowness is an advantage. Keep the canonicalization boundary small and fixture-driven.

## 2) Public serialization surfaces

### `dumps`

```python
canonical: bytes = rfc8785.dumps(value)
```

The return type is **bytes**, already suitable for hashing/signing. Do not encode it again.

### `dump`

```python
with open(path, "wb") as f:
    rfc8785.dump(value, f)
```

The sink is bytes-oriented. Use this when writing canonical bytes directly to an output abstraction. For checksums, `dumps` is usually simpler because the same bytes can be passed directly to BLAKE3.

## 3) Input domain

JCS canonicalizes the JSON data model. Python values therefore need to be JSON-compatible:

```text
None
bool
int within allowed interoperable domain
finite float
str
list/tuple-like supported sequence form
mapping with string keys
```

The package deliberately rejects non-string map keys rather than applying Python-specific coercions.

## 4) Integer-domain failures

Python integers are arbitrary precision, but RFC 8785 interoperability is bounded by the IEEE-754/ECMAScript numeric model. The implementation rejects integers outside its supported safe domain.

CodeFabric still validates integer tokens at **parse time** because:

- the source may need a more specific typed failure;
- the original integer-vs-float lexical category can matter to application validation;
- canonicalizer rejection is not a substitute for duplicate detection or schema formats.

## 5) Float-domain failures

Non-finite values (`nan`, `inf`, `-inf`) are not valid JCS values. The package reports typed canonicalization/domain failures instead of emitting non-standard JSON tokens.

Do not catch these errors and stringify the float; that would silently alter the profile.

## 6) Strings and UTF-8

JCS emits UTF-8 canonical JSON. The Python implementation validates/serializes strings according to RFC 8785 requirements and does not provide an application Unicode-normalization policy.

Preserve CodeFabric's “unchanged Unicode” rule. Do not call `unicodedata.normalize()` as a hidden pre-canonicalization step unless a schema field explicitly defines it outside JCS.

## 7) Object member ordering

RFC 8785 requires sorting property names by UTF-16 code units. This is not equivalent in all cases to Python's normal Unicode string ordering.

Therefore none of these is a replacement:

```python
sorted(d)
json.dumps(d, sort_keys=True)
collections.OrderedDict(...)
```

Let `rfc8785` own JCS property sorting.

## 8) Number rendering

JCS number output follows ECMAScript-compatible finite-double serialization. Python's `repr(float)`, `format`, `json.dumps`, or Decimal formatting should not be substituted.

A cross-language fixture should include exponent thresholds, `-0.0`, subnormal values, and numbers near representability boundaries so Rust's `ryu-js` path and Python's implementation stay byte-identical.

## 9) String escaping

The canonicalizer owns exactly which JSON characters are escaped and how. Do not post-process canonical bytes with `.replace`, ASCII escaping, whitespace removal, or HTML-safe escaping.

Any transformation after `rfc8785.dumps` changes the byte contract.

## 10) Error hierarchy

The package exposes canonicalization errors and typed domain-specific subclasses for unsupported numeric/string conditions. Application code should wrap these with semantic context while retaining the original exception.

Recommended failure routing:

```text
strict JSON decoder errors -> ingress layer
schema/format errors       -> validation layer
rfc8785 errors             -> canonicalization layer
blake3/framing mismatch    -> checksum layer
```

This makes it much easier for an agent to diagnose whether a cross-language fixture failed before or during canonicalization.

## 11) Why `rfc8785` does not replace `json.loads`

The input is a materialized Python object. Once a source object is a dict, duplicate JSON member names are gone.

Correct pipeline:

```python
source_text
 -> json.loads(... object_pairs_hook=..., parse_int=..., parse_constant=...)
 -> schema/domain validation
 -> deterministic CodeFabric transformations
 -> rfc8785.dumps(model)
```

Incorrect pipeline:

```python
source_text -> json.loads(source_text) -> rfc8785.dumps(...)
# duplicate keys may already have collapsed
```

## 12) Direct bytes-to-checksum pipeline

```python
from blake3 import blake3
import rfc8785

canonical = rfc8785.dumps(model)
digest = "b3:" + blake3(canonical).hexdigest()
```

Keep `canonical` as bytes for tests and hash input. Decode to text only for a human display need.

## 13) No pretty-print mode by design

Canonical serialization has one representation; “pretty canonical JSON” is a contradiction for the fingerprint byte stream.

If humans need readability, render a separate ordinary JSON/YAML view. Never overwrite stored expected canonical bytes with a pretty representation.

## 14) Schema-adapter boundary

Python application objects such as dataclasses, Pydantic models, UUIDs, datetime, Decimal, bytes, and enums are not automatically the RFC JSON data model. Convert them through an **explicit schema adapter** into strings/numbers/bools/lists/dicts according to the CodeFabric contract before canonicalization.

Do not add a generic `default=str`-style fallback around the RFC package.

## 15) `codefabric-bytes`

Raw Python `bytes` are not JSON. The schema layer must encode them as URL-safe unpadded base64 text, validate canonical form, and only then place the resulting `str` into the model that `rfc8785` receives.

This matches Rust's explicit `base64::URL_SAFE_NO_PAD` representation.

## 16) Non-string-keyed logical maps

Do not convert integer/tuple/etc. keys to strings opportunistically. Transform a logical map into the designated array of key/value records and sort the records according to the schema before JCS.

RFC 8785 sorts **object properties**, not arrays. Therefore CodeFabric must own ordering of record arrays that represent unordered maps.

## 17) Determinism boundary

After the model is ready for `rfc8785.dumps`, no nondeterministic data should remain:

- sets converted/sorted;
- logical maps converted and sorted;
- generated IDs/timestamps fixed by upstream semantics;
- no iteration over hash-randomized containers when emitting arrays;
- no environment-dependent locale formatting.

JCS cannot repair nondeterminism already encoded as array order or scalar values.

## 18) Performance guidance

For typical registry/config objects, correctness dominates micro-optimization. Useful practices:

- canonicalize once per final semantic value;
- keep returned bytes and hash them directly;
- avoid `dumps(...).decode().encode()` cycles;
- do not pre-sort object keys—let the library perform required ordering;
- benchmark only after fixtures prove equivalence.

For extremely large objects, object sorting and Python model allocation may dominate; consider architecture changes before reimplementing JCS.

## 19) Error handling example

```python
try:
    canonical = rfc8785.dumps(model)
except rfc8785.CanonicalizationError as exc:
    raise CanonicalJsonError("RFC 8785 serialization failed") from exc
```

Catch narrower subclasses when the application can map them to stable typed failures without losing context.

## 20) Differential testing against Rust

For every positive fixture:

```text
strict Python decode -> normalize -> rfc8785.dumps
strict Rust decode   -> normalize -> serde_json_canonicalizer::to_vec

assert bytes identical
assert BLAKE3 raw digest identical
assert `b3:` text identical
```

For negative fixtures, compare **failure class/contract outcome**, not exact Python-vs-Rust exception strings.

## 21) RFC appendix/edge vectors to retain

The fixture suite should include representative RFC edge behavior:

- property names whose UTF-16 order differs from naïve code-point assumptions;
- control characters and escaping;
- supplementary-plane Unicode;
- `-0.0` and exponent formatting;
- smallest/largest/subnormal finite doubles where practical;
- unsafe integer rejection;
- non-finite rejection;
- non-string mapping-key rejection.

## 22) Deployment matrix

| Deployment | Recommended stance |
|---|---|
| canonical checksum service | `dumps` -> Python `blake3` |
| canonical file emitter | `dump` to binary sink |
| human JSON export | stdlib JSON, not RFC bytes |
| untrusted JSON ingress | strict stdlib decoder before RFC package |
| typed app models | explicit adapter -> JSON-compatible primitives |

## 23) Agent execution playbook

```text
Use exactly rfc8785==0.1.4 for codefabric-jcs-v1.
Feed it only a fully validated deterministic JSON-compatible model.
Treat returned bytes as final canonical bytes; do not post-process.
Do not use dict sorting or stdlib JSON as a substitute.
Do not expect it to detect duplicates from source JSON.
Keep application formats and BLAKE3 framing outside the package.
Replay shared Rust/Python fixtures for any upgrade.
```
