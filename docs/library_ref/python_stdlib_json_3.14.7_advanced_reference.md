# Python `json` (CPython 3.14.7) — advanced technical reference / strict JSON ingress and decoder-hook catalog for LLM coding agents

## Version / source anchors

**Runtime anchor:** CPython 3.14.7 stdlib `json`, matching the CodeFabric design probe. This is a standard-library module rather than a separately pinned PyPI dependency.

Primary anchors:

- https://docs.python.org/3.14/library/json.html
- https://github.com/python/cpython/tree/v3.14.7/Lib/json
- https://www.rfc-editor.org/rfc/rfc8259

## CodeFabric role

Use stdlib `json.loads` as the **strict source boundary** before RFC 8785 canonicalization. Its `object_pairs_hook`, `parse_int`, `parse_float`, and `parse_constant` hooks expose exactly the points needed to reject duplicates and preserve/validate number tokens before ordinary Python object materialization erases source-level information.

Do **not** use `json.dumps` as the CodeFabric JCS serializer.

## Top-level API inventory

Serialization:

- `json.dump(obj, fp, ...)` — write JSON `str` to text sink;
- `json.dumps(obj, ...) -> str` — return JSON text.

Deserialization:

- `json.load(fp, ...)` — parse text/binary file-like input;
- `json.loads(s, ...)` — parse `str`, `bytes`, or `bytearray`.

Extensibility:

- `JSONEncoder` / `default`;
- `JSONDecoder` / hooks;
- `JSONDecoder.raw_decode` for a JSON prefix plus ending index;
- `python -m json` CLI for validation/pretty printing.

## `json.loads` hooks

Signature-relevant hooks in 3.14:

```python
json.loads(
    s,
    *,
    cls=None,
    object_hook=None,
    parse_float=None,
    parse_int=None,
    parse_constant=None,
    object_pairs_hook=None,
    **kw,
)
```

### `object_pairs_hook`

Called for every decoded object with an **ordered list of `(name, value)` pairs**. If also supplied, it takes precedence over `object_hook`.

This is the key duplicate-rejection hook because repeated names are still visible:

```python
class DuplicateKeyError(ValueError):
    pass

def reject_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise DuplicateKeyError(key)
        out[key] = value
    return out
```

Default `json.loads` behavior accepts repeated names and keeps the last value, which is not acceptable for CodeFabric canonical ingress.

### `parse_int`

Receives the exact string spelling of each JSON integer token. Use it to validate safe-integer bounds **before** constructing the final Python `int`.

```python
SAFE = 9_007_199_254_740_991

def parse_int_strict(token: str) -> int:
    value = int(token)
    if not -SAFE <= value <= SAFE:
        raise ValueError("unsafe JSON integer")
    return value
```

Python 3.11+ also applies the interpreter's integer-string conversion length limit to default `int()` as a denial-of-service mitigation. Application-level token length limits may still be appropriate.

### `parse_float`

Receives the string spelling of each JSON real/exponent token. This can construct `decimal.Decimal`, a token wrapper, or a validated float depending on the contract.

For JCS parity, the ultimate accepted numeric value must be representable in the RFC 8785/ECMAScript finite-double domain. If exact source-token classification matters, preserve the token until validation is complete.

### `parse_constant`

Called for the non-standard tokens `NaN`, `Infinity`, and `-Infinity`. The stdlib decoder accepts these by default even though they are outside strict JSON. CodeFabric should reject them:

```python
def reject_constant(token: str):
    raise ValueError(f"non-finite JSON number: {token}")
```

## Recommended strict loader

```python
import json

SAFE = 9_007_199_254_740_991

class DuplicateKeyError(ValueError):
    pass


def object_pairs(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise DuplicateKeyError(key)
        out[key] = value
    return out


def parse_int_token(token: str):
    value = int(token)
    if value < -SAFE or value > SAFE:
        raise ValueError("integer outside JCS safe range")
    return value


def parse_float_token(token: str):
    value = float(token)
    if not math.isfinite(value):
        raise ValueError("non-finite number")
    return value


def reject_constant(token: str):
    raise ValueError(f"invalid JSON numeric constant: {token}")


def strict_loads(source: str | bytes | bytearray):
    return json.loads(
        source,
        object_pairs_hook=object_pairs,
        parse_int=parse_int_token,
        parse_float=parse_float_token,
        parse_constant=reject_constant,
    )
```

The production implementation may use a richer number-token wrapper if it needs to distinguish domain decisions more precisely before converting to `float`.

## `object_hook` vs `object_pairs_hook`

`object_hook` sees an already-created dict and therefore cannot recover duplicate names. It is useful for semantic object conversion but should not replace `object_pairs_hook` in strict input validation.

If semantic conversion is required, perform it after duplicate-safe object construction or combine the behavior in the pairs hook deliberately.

## Decoder compliance caveats

The stdlib docs explicitly note:

- decoder accepts `NaN`/`Infinity` by default;
- repeated names are accepted by default and only the last value survives;
- `parse_constant` can override non-finite token handling;
- `object_pairs_hook` can override duplicate-name handling;
- input ordering is preserved by normal Python containers, but ordering preservation is not canonical ordering.

Strict systems must opt into the desired policy.

## Input encodings

`loads` accepts `str`, `bytes`, or `bytearray`; bytes input may be UTF-8/UTF-16/UTF-32 according to the decoder. If the CodeFabric source contract requires UTF-8 specifically, enforce that at the boundary rather than relying on the decoder's broader convenience behavior.

## `JSONDecodeError`

Malformed input raises `JSONDecodeError`, which provides message, document position, line, and column information. Preserve this location information in diagnostics while converting it to stable application error codes.

Unicode decoding of binary input can separately raise `UnicodeDecodeError`.

## `JSONDecoder.raw_decode`

`raw_decode(s)` returns `(value, end_index)` and can parse one JSON value from a string with extra trailing data. Useful for custom framing/protocol parsers. Ordinary CodeFabric documents should normally require a single complete JSON value; do not use `raw_decode` to accidentally tolerate trailing junk.

## Encoder surface and why it is not JCS

`json.dumps` supports `ensure_ascii`, `allow_nan`, indentation, separators, `sort_keys`, custom encoders, and key skipping. Even with:

```python
json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False, allow_nan=False)
```

the result is **not** a guarantee of RFC 8785 semantics. Python key sorting is not the full UTF-16 JCS ordering contract and float rendering/escaping semantics are not specified as a JCS implementation. Use `rfc8785.dumps`.

## Resource limits

The stdlib module does not impose comprehensive application-level limits on document size, nesting, number magnitude, or string/array/object counts beyond interpreter/datatype constraints. Add explicit limits when parsing untrusted large inputs.

The default `int()` conversion-length limit provides one defense but is not a complete JSON DoS policy.

## Security and correctness anti-patterns

- default `json.loads` followed by duplicate detection;
- assuming Python dict insertion order proves source uniqueness;
- allowing default NaN/Infinity parsing;
- converting number tokens before validating contract bounds;
- accepting UTF-16/UTF-32 when the external protocol declares UTF-8 only;
- using `sort_keys=True` as JCS;
- using `object_hook` where duplicate evidence is needed;
- ignoring trailing data with `raw_decode` in a single-document protocol.

## Agent checklist

- [ ] `object_pairs_hook` rejects duplicates at every nesting level;
- [ ] `parse_constant` rejects NaN and infinities;
- [ ] integer and float hooks enforce CodeFabric/JCS numeric policy before information loss;
- [ ] source encoding policy is explicit;
- [ ] successful output is then passed to schema/format validation and `rfc8785.dumps`;
- [ ] `json.dumps` is used only for non-canonical JSON output.
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

## 1) Decoder/encoder architecture

Python's `json` module provides:

- `load` / `loads` convenience decoding;
- `dump` / `dumps` convenience encoding;
- `JSONDecoder` and `JSONEncoder` classes for configured/reusable behavior;
- decoder hooks that are unusually valuable for strict protocol ingress;
- a module CLI (`python -m json`) for validation/pretty-printing.

For CodeFabric, the key value is **decoder hooks before ordinary dict semantics erase source evidence**.

## 2) `loads` and `load`

```python
obj = json.loads(text, ...)
obj = json.load(file_obj, ...)
```

`loads` accepts text and supported bytes-like encoded inputs. `load` consumes a text/binary file-like object as documented by the stdlib.

A strict wrapper should own all hook arguments centrally. Do not let call sites independently choose permissive decoder settings for fingerprinted input.

## 3) `object_pairs_hook`

The decoder calls `object_pairs_hook` with the object's members as an **ordered list of `(name, value)` pairs**. This is the only normal stdlib hook that still exposes duplicate source member names before they collapse into a dict.

```python
def reject_duplicates(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise DuplicateKeyError(key)
        out[key] = value
    return out
```

If both `object_pairs_hook` and `object_hook` are provided, `object_pairs_hook` takes priority. CodeFabric should therefore build its strict object logic around `object_pairs_hook` rather than layering duplicate detection onto `object_hook`.

## 4) `parse_int`

`parse_int` receives the source integer token as a string. This makes it ideal for enforcing safe-integer rules **before** a Python arbitrary-precision `int` makes every syntactically valid integer appear lossless locally.

```python
SAFE = 9_007_199_254_740_991

def parse_int_strict(token: str) -> int:
    value = int(token)
    if not -SAFE <= value <= SAFE:
        raise UnsafeIntegerError(token)
    return value
```

If specific fields use string-encoded `codefabric-int64` / `uint64`, validate those as format strings in the schema layer rather than relaxing generic JSON numeric policy.

## 5) `parse_float`

`parse_float` receives the JSON floating token as source text and returns the desired Python object.

Possible uses:

- reject excessive token lengths before expensive conversion;
- preserve a custom lexical wrapper during a validation pass;
- use `decimal.Decimal` in non-JCS financial/application contexts.

For JCS, using `Decimal` does **not** change RFC 8785's ECMAScript-double output contract. Do not turn a strict JCS profile into decimal canonical JSON by changing this hook.

## 6) `parse_constant`

CPython's decoder recognizes the non-standard tokens `NaN`, `Infinity`, and `-Infinity` and routes them to `parse_constant`. Strict JSON must reject them:

```python
def reject_constant(token: str):
    raise NonFiniteJSONNumberError(token)
```

This hook is mandatory for a strict boundary because the module's default behavior accepts these non-RFC JSON extensions.

## 7) Repeated member names: default behavior

The stdlib accepts repeated object names by default; when decoded to an ordinary dict, later values win. This is precisely why an already-materialized dict cannot prove source uniqueness.

Protocol rule:

```text
Never canonicalize a dict from untrusted source unless the source decoder already enforced duplicate rejection.
```

## 8) `object_hook`

`object_hook` receives a dict after normal object construction. It is useful for application-specific reconstruction, such as recognizing tagged object shapes, but cannot be used to recover duplicates.

Use either:

- `object_pairs_hook` alone to return the desired dict/model; or
- a pairs hook that explicitly invokes a second reconstruction function after uniqueness checks.

## 9) `JSONDecoder`

A configured decoder can centralize hooks:

```python
decoder = json.JSONDecoder(
    object_pairs_hook=reject_duplicates,
    parse_int=parse_int_strict,
    parse_constant=reject_constant,
)
obj = decoder.decode(text)
```

Prefer a thin application-owned `strict_json_loads()` API so dependency behavior does not leak throughout the repository.

## 10) `raw_decode`

`JSONDecoder.raw_decode` decodes a JSON value from the beginning of a string and returns both value and the index where parsing stopped. This supports custom framing/protocols where JSON is embedded in a larger buffer.

It should not be used to accidentally accept garbage after a document. If the endpoint promises one JSON document, verify that only permitted whitespace follows.

## 11) `dump` / `dumps`

Encoder options include:

- `skipkeys`;
- `ensure_ascii`;
- `check_circular`;
- `allow_nan`;
- `cls`;
- `indent`;
- `separators`;
- `default`;
- `sort_keys`.

These make `json.dumps` flexible for APIs and human output. They do **not** make it RFC 8785.

### Why `sort_keys=True` is insufficient

JCS requires UTF-16 property ordering plus its own number/string serialization contract. `sort_keys=True`, compact separators, and `ensure_ascii=False` are still a collection of Python encoder settings, not a standards-complete RFC 8785 implementation.

## 12) `allow_nan`

Default encoder behavior permits non-finite float spellings compatible with JavaScript tokens rather than strict JSON. Set `allow_nan=False` for ordinary strict JSON output when using stdlib encoding.

For CodeFabric fingerprint bytes, use `rfc8785.dumps` instead of configuring `json.dumps` into a near-canonical shape.

## 13) `JSONEncoder`

Subclass or provide `default()` only for presentation/adapters with a clear object-to-JSON policy.

`iterencode()` emits JSON text fragments incrementally and can reduce peak intermediate string construction. It still uses stdlib JSON semantics, not RFC 8785 semantics.

Do not build JCS by subclassing `JSONEncoder`; use the selected RFC implementation.

## 14) `ensure_ascii`

With default `ensure_ascii=True`, non-ASCII characters are escaped into ASCII-safe sequences. With `False`, Unicode characters can be emitted directly.

Both can represent equivalent JSON string values, illustrating why ordinary serialization bytes are not suitable as fingerprints. RFC 8785 decides the canonical escape representation independently.

## 15) Whitespace controls

`indent` and `separators` alter presentation. Whitespace outside strings is insignificant to JSON semantics but obviously changes source bytes.

Use pretty JSON for diagnostics and source artifacts; use RFC 8785 bytes for equality/fingerprint semantics.

## 16) Key coercion and `skipkeys`

JSON object names are strings. The stdlib encoder can accept some non-string basic keys and convert them, or skip unsupported ones if `skipkeys=True`.

For a canonical cross-language model:

- do not enable `skipkeys=True`—dropping data silently is unacceptable;
- do not rely on Python-specific key coercion;
- represent logical non-string-keyed maps as explicit sorted records according to the schema.

## 17) Circular references

`check_circular=True` guards against recursive Python containers during encoding. Disabling it can lead to recursion failure on cyclic structures.

A valid JSON semantic model is a tree, not an arbitrary cyclic object graph. Detect/fail rather than trying to invent a canonical representation for cycles.

## 18) Input encoding behavior

`loads` accepts supported Unicode encodings when given bytes/bytearray and detects encoding according to the JSON module behavior. At a network/file boundary, it is usually cleaner for the protocol layer to require UTF-8 explicitly and reject unexpected encodings before or during decode.

RFC 8785 output is UTF-8 bytes. Keep input transport decoding policy distinct from canonical output encoding.

## 19) `JSONDecodeError`

`JSONDecodeError` extends `ValueError` with useful position information such as message, document, absolute position, line and column.

Wrap it in an application exception containing source/file identity when appropriate, but avoid storing entire sensitive input strings inside telemetry merely because the exception references the document.

## 20) Strict loader reference implementation

```python
from __future__ import annotations
import json

SAFE = 9_007_199_254_740_991

class StrictJsonError(ValueError): ...
class DuplicateKeyError(StrictJsonError): ...
class UnsafeIntegerError(StrictJsonError): ...
class NonFiniteNumberError(StrictJsonError): ...

def _pairs(pairs):
    out = {}
    for key, value in pairs:
        if key in out:
            raise DuplicateKeyError(key)
        out[key] = value
    return out

def _int(token: str) -> int:
    value = int(token)
    if value < -SAFE or value > SAFE:
        raise UnsafeIntegerError(token)
    return value

def _constant(token: str):
    raise NonFiniteNumberError(token)

def strict_loads(text: str):
    return json.loads(
        text,
        object_pairs_hook=_pairs,
        parse_int=_int,
        parse_constant=_constant,
    )
```

The production version should add numeric-token-length and document-size controls appropriate to the trust boundary.

## 21) Decoder extension policy

Keep strict JSON decoder policy in one module. A library/application component that needs permissive data ingestion should have a separately named adapter, not flags on the canonical loader such as `strict=False` that are easy to pass incorrectly.

## 22) Resource limits

The stdlib warns that malicious JSON can consume substantial CPU/memory. Bound externally as needed:

```text
raw bytes / characters
nesting
member count
array length
string length
numeric token length
aggregate materialized object size
```

Python's unlimited-size integers are especially relevant: modern CPython has safeguards around integer string conversion, but a protocol should retain its own much smaller semantic bounds.

## 23) CLI tooling

`python -m json` is useful for quick syntax validation and pretty printing. It uses stdlib JSON semantics; do not use it as a JCS conformance checker or duplicate-key validator.

## 24) Determinism and dict ordering

Modern Python dicts preserve insertion order, but RFC 8785 requires canonical key ordering independent of source insertion. Dict insertion order must therefore not be treated as fingerprint ordering.

Similarly, application transformations that build arrays from sets/dicts must sort according to schema semantics before JCS; RFC 8785 does not reorder arrays.

## 25) Testing matrix

```text
[ ] duplicate names at root and nested object
[ ] NaN / Infinity / -Infinity rejected
[ ] safe integer boundaries accepted
[ ] one-past safe boundaries rejected
[ ] exponent and decimal float tokens handled consistently with profile
[ ] trailing junk rejected for single-document endpoint
[ ] UTF-8 and Unicode edge cases
[ ] huge numeric token rejected/bounded
[ ] ordinary dict insertion order cannot affect JCS bytes
[ ] strict-loaded Python model canonicalizes to same bytes as Rust model
```

## 26) Deployment advisory

| Deployment | Recommended stance |
|---|---|
| canonical JSON API | one application `strict_loads` wrapper + `rfc8785.dumps` |
| developer scripts | stdlib defaults acceptable only for non-protocol data |
| large untrusted uploads | pre-parse size limits + strict hooks + semantic limits |
| human export | `json.dump(..., indent=...)` |
| fingerprint serialization | never stdlib encoder |

## 27) Agent execution playbook

```text
1. For canonical ingress, use the repository strict wrapper; do not call bare json.loads.
2. Ensure object_pairs_hook is present wherever duplicate rejection is required.
3. Ensure parse_constant rejects the stdlib's non-finite extensions.
4. Enforce safe integer bounds before RFC 8785 serialization.
5. Never replace rfc8785.dumps with json.dumps(sort_keys=True, ...).
6. Treat decoder-hook changes as protocol validation changes.
7. Replay the Rust/Python shared fixture corpus after CPython upgrades.
```
