# orjson in Python — advanced technical reference / feature-category catalog

This reference is designed for **LLM coding agents and engineers using orjson as a high-performance JSON boundary**, particularly in FastMCP/Pydantic/gRPC systems where some data remains JSON while RPC contracts use Protobuf. Unlike a generic JSON tutorial, it focuses on the exact `orjson` type/option/error model, zero-ambiguity integration boundaries, performance behavior, and the places where “faster `json`” is an unsafe mental model.

## Version / source anchors

**Primary deployable target: `orjson==3.12.0`**, released **2026-08-14**, and the latest stable release as of 2026-08-20.

Release 3.12.0 substantially rewrote the serialization implementation and ships CPython **3.10 through 3.15** wheels on supported platforms. For Python >=3.15, Linux wheel baseline details change for some architectures; release notes state that 3.12.0 no longer provides ppc64le/s390x wheels and uses a newer manylinux baseline for Python 3.15+ than older CPython wheels.

The project explicitly targets CPython, does **not** support PyPy, embedded Android/iOS Python builds, or PEP 554 subinterpreters, and may add free-threading support once that platform is stable. The current implementation holds the **GIL for the duration of `dumps()` and `loads()`**.

Primary sources:

- PyPI: https://pypi.org/project/orjson/
- GitHub/tag README: https://github.com/ijl/orjson/tree/3.12.0
- Releases: https://github.com/ijl/orjson/releases/tag/3.12.0

---

## Feature inventory

This reference covers installation/platform constraints, `dumps`, `loads`, native type support, strict UTF-8, integer/float rules, datetime/date/time, UUID, dataclasses, TypedDict, NumPy, `default`, every stable option flag, non-string keys, deterministic sorting caveats, `Fragment`, JSON Lines patterns, errors, nesting/input limits, GIL/concurrency, performance, bytes/IO boundaries, FastAPI/Pydantic/FastMCP integration, protobuf boundaries, security/trust, testing, release migration, and dense decision matrices.

---

# Proposed comprehensive documentation map

0. Scope, versioning, and orjson mental model
1. Installation, platform, and deployment constraints
2. Core API: `dumps()` and `loads()`
3. Supported native Python types
4. Strings and strict UTF-8
5. Integers, floats, booleans, and null
6. `datetime`, `date`, and `time`
7. UUID
8. Dataclasses and TypedDict
9. NumPy serialization
10. `default` callback contract
11. Option-bitmask model and full inventory
12. Non-string dictionary keys
13. Formatting: indent, sorting, newline
14. Date/time option combinations
15. Passthrough options
16. Strict integer policy
17. `orjson.Fragment`
18. `loads()` input types, parser limits, and errors
19. `dumps()` errors and unsupported values
20. JSON Lines / NDJSON patterns
21. Bytes, files, sockets, and HTTP boundaries
22. GIL, concurrency, and async applications
23. Performance engineering and benchmarking
24. Pydantic integration
25. FastAPI integration
26. FastMCP integration
27. Protobuf/gRPC integration
28. Security and trust boundaries
29. Testing and compatibility fixtures
30. Upgrade guidance for 3.12
31. Anti-pattern inventory
32. Dense API/option matrices
33. Agent implementation checklist

---

# orjson Advanced — 0) Scope, versioning, and mental model

## 0.0 What orjson is

`orjson` is a **high-performance JSON serializer/deserializer for CPython**, implemented primarily in Rust/native code and exposed through a deliberately small Python API:

```python
orjson.dumps(obj, default=None, option=None) -> bytes
orjson.loads(obj) -> Any
```

The compact surface is deceptive: the supported native type set and option flags encode important semantic choices around datetime formatting, integer precision, non-string keys, dataclasses, NumPy, subclass handling, and raw JSON fragments.

## 0.1 What it is not

orjson is not:

- a drop-in behavioral clone of `json` in every corner case;
- a schema validator;
- a streaming JSON parser/writer;
- a file I/O API;
- a protobuf ProtoJSON implementation;
- automatically safe for arbitrary pre-serialized fragments;
- automatically parallel merely because it is native/Rust code.

## 0.2 Primary architectural value

Use orjson when the application has a **real JSON contract** and serialization/deserialization cost is meaningful. Do not insert JSON into a protobuf/gRPC boundary that was already binary solely to use orjson.

## 0.3 Key invariants

```text
dumps() returns bytes, not str.
loads() accepts bytes/bytearray/memoryview/str.
UTF-8 is strict.
The GIL is held during dumps/loads.
JSON object keys are strings on the wire.
NaN/Infinity serialize as null; invalid non-standard numeric tokens are rejected on input.
Fragment bypasses validation for embedded raw JSON.
```

---

# 1) Installation, platform, and deployment constraints

## 1.1 Pin

```bash
python -m pip install "orjson==3.12.0"
```

or:

```toml
[project]
dependencies = [
  "orjson==3.12.0",
]
```

## 1.2 CPython support

3.12.0 supports current CPython releases 3.10–3.15 on its documented wheel platforms. Check PyPI wheel availability for the exact architecture/container baseline used in deployment.

## 1.3 Unsupported runtimes

Do not design around orjson if the product requires:

- PyPy;
- PEP 554 subinterpreters;
- embedded Android/iOS Python;
- an unsupported CPU/libc/wheel platform without an acceptable source-build toolchain.

## 1.4 Source builds

Source builds require Rust/native build machinery and are slower/more complex than wheel installation. In minimal production containers, prefer a supported prebuilt wheel unless supply-chain/build policy requires compiling from source.

## 1.5 Platform change in 3.12.0

The 3.12 release added Python 3.15 wheels and changed Linux wheel baseline/architecture availability for that interpreter generation. If your fleet uses older glibc distributions or less-common architectures, test deployment artifacts rather than only local import behavior.

---

# 2) Core API: `dumps()` and `loads()`

## 2.1 Exact conceptual signatures

```python
orjson.dumps(obj, default=None, option=None) -> bytes
orjson.loads(obj) -> Any
```

## 2.2 `dumps()` returns `bytes`

```python
payload = orjson.dumps({"a": 1})
assert payload == b'{"a":1}'
```

Do not immediately call `.decode()` unless the consumer truly requires a Python `str`. Network/file interfaces often accept bytes directly.

## 2.3 `loads()` accepts multiple buffer forms

```python
orjson.loads(b'{"a":1}')
orjson.loads(bytearray(b'{"a":1}'))
orjson.loads(memoryview(b'{"a":1}'))
orjson.loads('{"a":1}')
```

Passing bytes-like input directly avoids a needless UTF-8 decode before parsing.

## 2.4 No file methods

There is no `orjson.dump(fp)` / `orjson.load(fp)` file API. Compose with Python I/O:

```python
path.write_bytes(orjson.dumps(data))
data = orjson.loads(path.read_bytes())
```

## 2.5 Option bitmask

Multiple options are OR'ed:

```python
payload = orjson.dumps(
    data,
    option=orjson.OPT_SORT_KEYS | orjson.OPT_APPEND_NEWLINE,
)
```

Do not pass a Python list/set of options.

---

# 3) Supported native Python types

orjson 3.12 serializes a broad set natively, including:

```text
None
bool
int
float
str
dict
list
tuple
dataclass instances
TypedDict-shaped dict objects (ordinary runtime dicts)
datetime/date/time
UUID
enum.Enum values/subclasses
orjson.Fragment
NumPy ndarray/scalars when OPT_SERIALIZE_NUMPY is enabled
subclasses of several supported types unless passthrough policy changes behavior
```

It also accepts combinations recursively subject to key/type rules.

## 3.1 Do not confuse typing constructs with runtime classes

A `TypedDict` instance is an ordinary `dict` at runtime; support refers to its value shape, not runtime schema validation.

## 3.2 Namedtuple/tuple subclasses

The project deliberately does not promise generic tuple-subclass/namedtuple serialization as ordinary tuples. Use explicit conversion/default handling when the type is not in the supported native matrix.

## 3.3 Schema validation is outside orjson

orjson serializes what it supports; it does not validate that a dict conforms to a Pydantic model or TypedDict annotation. Validate before serialization when the boundary requires it.

---

# 4) Strings and strict UTF-8

## 4.1 Valid Unicode only

orjson enforces valid UTF-8/Unicode and rejects invalid surrogate-containing Python strings rather than silently emitting malformed JSON.

```python
try:
    orjson.dumps("\ud800")
except orjson.JSONEncodeError:
    ...
```

## 4.2 Why this is desirable

JSON requires Unicode text. Silently preserving invalid surrogate data pushes interoperability bugs into downstream systems. Treat encode failure as upstream data-quality failure.

## 4.3 Loads

Invalid UTF-8 bytes or invalid JSON strings raise `JSONDecodeError`.

## 4.4 Do not sanitize by `errors="ignore"` automatically

Dropping characters to force serialization mutates user/data semantics. Fix the source encoding or make a deliberate replacement policy before orjson.

---

# 5) Integers, floats, booleans, and null

## 5.1 Integers

By default orjson serializes integers within the supported 64-bit range. Python's arbitrary precision does not imply arbitrary JSON integer output.

```python
orjson.dumps(2**63 - 1)
```

Use `OPT_STRICT_INTEGER` to enforce the interoperable 53-bit integer range commonly safe for IEEE-754 JavaScript number consumers.

## 5.2 Floats

Finite floats serialize as JSON numbers.

Special non-finite values:

```python
orjson.dumps(float("nan"))  # b"null"
orjson.dumps(float("inf"))  # b"null"
```

This differs from Python stdlib configurations that may emit non-standard `NaN`/`Infinity` tokens.

## 5.3 Input rejects non-standard numeric tokens

`loads()` rejects invalid JSON tokens such as bare `NaN`/`Infinity` rather than accepting JavaScript-like extensions.

## 5.4 Bool vs int

Python `bool` is a subclass of `int`, but JSON semantics distinguish booleans. orjson emits `true`/`false` correctly.

## 5.5 None

```python
orjson.dumps(None) == b"null"
```

Beware a `default` callback that forgets to raise and implicitly returns `None`: the unsupported object will silently become `null`. See §10.

---

# 6) `datetime`, `date`, and `time`

## 6.1 Native serialization

orjson natively serializes Python datetime/date/time values using RFC 3339-like formatting rules documented by the project.

```python
from datetime import datetime, timezone

payload = orjson.dumps({"ts": datetime.now(timezone.utc)})
```

## 6.2 Naive datetime

Default naive datetime output has no timezone offset. `OPT_NAIVE_UTC` treats naive datetime as UTC for serialization.

## 6.3 UTC `Z`

`OPT_UTC_Z` formats UTC offsets as `Z` rather than `+00:00` where applicable.

## 6.4 Microseconds

`OPT_OMIT_MICROSECONDS` removes the fractional-second component.

## 6.5 `time` and timezone

`time` values with unsupported `tzinfo` combinations are rejected. Do not use time-of-day with timezone semantics as if it were an instant; use `datetime`/domain modeling when offset/zone meaning is required.

## 6.6 `zoneinfo`

Prefer standard-library `zoneinfo` for time zones in modern Python rather than relying on arbitrary custom tzinfo objects with unsupported serialization behavior.

---

# 7) UUID

UUID serializes natively to its standard string representation:

```python
from uuid import uuid4
orjson.dumps({"id": uuid4()})
```

`OPT_SERIALIZE_UUID` exists for backwards compatibility but is deprecated/no-op because UUID is serialized by default in modern orjson.

Use UUID native support instead of `default=str` globally, which can mask unsupported types unexpectedly.

---

# 8) Dataclasses and TypedDict

## 8.1 Dataclass native serialization

```python
from dataclasses import dataclass

@dataclass
class Node:
    id: str
    kind: str

payload = orjson.dumps(Node("n1", "function"))
```

## 8.2 Performance semantics

Native dataclass serialization avoids an explicit `dataclasses.asdict()` deep conversion step and can be substantially more efficient.

## 8.3 Dataclass passthrough

`OPT_PASSTHROUGH_DATACLASS` disables native dataclass serialization so your `default` callback can control it.

Use only when custom field filtering/redaction/formatting is required.

## 8.4 Deprecated serialization option

`OPT_SERIALIZE_DATACLASS` is a deprecated no-op retained for compatibility. Do not add it to new code.

## 8.5 TypedDict

At runtime TypedDict values are dicts and serialize as dicts. The annotation is not consulted for validation or field exclusion.

---

# 9) NumPy serialization

## 9.1 Enable explicitly

```python
payload = orjson.dumps(
    array,
    option=orjson.OPT_SERIALIZE_NUMPY,
)
```

Supported NumPy types/dtypes are documented by orjson and cover common numeric arrays/scalars across NumPy 1.x/2.x environments.

## 9.2 Contiguity and native endianness

Arrays generally need a supported contiguous/native-endian representation. Unsupported memory layout or dtype raises rather than silently copying every exotic array shape.

Normalize deliberately when needed:

```python
arr = np.ascontiguousarray(arr)
```

## 9.3 Precision/format differences

Native NumPy serialization can differ from `array.tolist()` followed by Python float serialization because conversion paths/precision may differ. If exact JSON numeric text is contract-critical, golden-test your actual path.

## 9.4 Large arrays

JSON is inefficient for very large numerical arrays. In a code-intelligence/data-fabric system, prefer Arrow/Parquet/protobuf bytes or another binary format for bulk vectors/matrices; use orjson for control/metadata payloads.

---

# 10) `default` callback contract

## 10.1 Purpose

`default` handles unsupported object types:

```python
def default(obj):
    if isinstance(obj, Decimal):
        return str(obj)
    raise TypeError

payload = orjson.dumps(value, default=default)
```

## 10.2 Critical rule: raise `TypeError`

If the callback cannot serialize an object, it **must raise `TypeError`**.

Bad:

```python
def default(obj):
    if isinstance(obj, Decimal):
        return str(obj)
    # implicit return None -> unsupported object becomes JSON null
```

Good:

```python
def default(obj):
    if isinstance(obj, Decimal):
        return str(obj)
    raise TypeError
```

## 10.3 Return must itself be serializable

The returned object is recursively serialized and may invoke `default` again. orjson limits recursive `default` handling to **254 levels** before raising `JSONEncodeError`. Avoid recursive conversions that return the original unsupported object.

## 10.4 Do not use `default=str`

`default=str` turns arbitrary unsupported objects into implementation-dependent strings, hiding bugs and potentially leaking sensitive representations.

Prefer an explicit type allowlist.

---

# 11) Option-bitmask model and full inventory

Stable option constants in the current reference surface include:

```text
OPT_APPEND_NEWLINE
OPT_INDENT_2
OPT_NAIVE_UTC
OPT_NON_STR_KEYS
OPT_OMIT_MICROSECONDS
OPT_PASSTHROUGH_DATACLASS
OPT_PASSTHROUGH_DATETIME
OPT_PASSTHROUGH_SUBCLASS
OPT_SERIALIZE_DATACLASS   # deprecated no-op
OPT_SERIALIZE_NUMPY
OPT_SERIALIZE_UUID        # deprecated no-op
OPT_SORT_KEYS
OPT_STRICT_INTEGER
OPT_UTC_Z
```

Combine with bitwise OR:

```python
option = (
    orjson.OPT_SORT_KEYS
    | orjson.OPT_APPEND_NEWLINE
    | orjson.OPT_UTC_Z
)
```

Avoid global “standard option bundles” unless every API using the bundle truly shares the same semantic requirements.

---

# 12) Non-string dictionary keys

JSON object keys are strings. By default orjson requires string keys. `OPT_NON_STR_KEYS` converts supported non-string key types.

```python
payload = orjson.dumps(
    {1: "one", None: "none"},
    option=orjson.OPT_NON_STR_KEYS,
)
```

## 12.1 Collision hazard

Supported non-string keys include `int`, `float`, `bool`, `None`, `datetime.datetime`, `datetime.date`, `datetime.time`, `enum.Enum`, and `uuid.UUID` in addition to `str`. Different Python keys can stringify to the same JSON key:

```python
{"1": "a", 1: "b"}
```

With non-string conversion this can produce duplicate JSON object keys. Many JSON parsers keep only the last value, creating data loss/ambiguity.

## 12.2 Sorting caveat

`OPT_SORT_KEYS` with converted duplicate keys cannot establish a meaningful semantic canonical object because duplicate-key ordering/value retention is ambiguous.

## 12.3 Recommendation

Prefer normalizing keys to a unique string schema *before* serialization for public APIs. Use `OPT_NON_STR_KEYS` for controlled internal data where collision semantics have been analyzed.

---

# 13) Formatting: indent, sorting, newline

## 13.1 Compact default

```python
orjson.dumps({"b": 1, "a": 2})
# compact bytes; no pretty whitespace
```

## 13.2 Two-space indent

```python
orjson.dumps(data, option=orjson.OPT_INDENT_2)
```

Only two-space pretty printing is exposed as an option; orjson intentionally avoids a huge formatting configuration API.

## 13.3 Sorted keys

```python
orjson.dumps(data, option=orjson.OPT_SORT_KEYS)
```

Sorting incurs a material performance cost. It is not locale-aware and should not be enabled unless deterministic human output/testing/cache keys require it.

## 13.4 Append newline

```python
orjson.dumps(data, option=orjson.OPT_APPEND_NEWLINE)
```

Useful for JSON Lines output and text-file ergonomics.

## 13.5 Sorting is not full canonical JSON

If cryptographic canonicalization/signatures require a formal canonical JSON specification, `OPT_SORT_KEYS` alone is insufficient. Number formatting, duplicate keys, Unicode normalization, and other rules need an explicit canonicalization standard.

---

# 14) Date/time option combinations

| Requirement | Options |
|---|---|
| naive datetimes treated as UTC | `OPT_NAIVE_UTC` |
| UTC rendered with `Z` | `OPT_UTC_Z` |
| remove microseconds | `OPT_OMIT_MICROSECONDS` |
| handle datetime yourself in `default` | `OPT_PASSTHROUGH_DATETIME` |

Example:

```python
option = orjson.OPT_NAIVE_UTC | orjson.OPT_UTC_Z
payload = orjson.dumps({"ts": ts}, option=option)
```

Choose one application datetime contract and test it. Mixing endpoints where naive datetimes sometimes mean UTC and sometimes mean local/unspecified time creates subtle API bugs.

---

# 15) Passthrough options

## 15.1 `OPT_PASSTHROUGH_DATETIME`

Stops native datetime/date/time serialization so `default` can receive those values.

Use for custom epoch formats, mandatory timezone validation, or specialized date strings.

## 15.2 `OPT_PASSTHROUGH_DATACLASS`

Routes dataclasses to `default` instead of native field serialization.

## 15.3 `OPT_PASSTHROUGH_SUBCLASS`

Routes subclasses of supported built-ins through `default`, useful for redaction/semantic wrapper types.

Example security wrapper:

```python
class Secret(str):
    pass

def default(obj):
    if isinstance(obj, Secret):
        return "***"
    raise TypeError

orjson.dumps(
    {"token": Secret("abc")},
    default=default,
    option=orjson.OPT_PASSTHROUGH_SUBCLASS,
)
```

Without passthrough, native subclass serialization can bypass your intended custom handling.

---

# 16) Strict integer policy

`OPT_STRICT_INTEGER` rejects integers outside the **53-bit** range generally safe for exact JavaScript/IEEE-754 number representation.

```python
orjson.dumps(
    {"id": 2**60},
    option=orjson.OPT_STRICT_INTEGER,
)  # raises
```

Use when JSON consumers include JavaScript/browser systems and numeric identity precision matters.

Alternative: model large IDs as strings intentionally.

Do not enable strict integer globally if the API legitimately transfers 64-bit counters to consumers capable of preserving them; choose per contract.

---

# 17) `orjson.Fragment`

## 17.1 Purpose

`Fragment` embeds **already serialized JSON bytes** directly into a larger document without parsing/reformatting them.

Conceptual example:

```python
fragment = orjson.Fragment(b'{"precomputed":true}')
payload = orjson.dumps({"data": fragment})
```

## 17.2 Trust boundary

orjson does **not validate** the fragment. Invalid/malicious fragment bytes can make the final output invalid JSON or inject structure you did not intend.

Only use Fragment with bytes produced by a trusted JSON serializer or stored under a strong validated invariant.

## 17.3 Formatting interaction

Embedded fragment formatting is preserved; outer indentation/sorting does not rewrite the fragment's internal whitespace/key order.

## 17.4 Ideal use

- cached pre-serialized JSON subdocuments;
- database columns guaranteed to contain validated JSON;
- composing large responses without repeated parse/serialize.

Not appropriate for arbitrary user-provided bytes.

---

# 18) `loads()` input types, parser limits, and errors

## 18.1 Inputs

```text
bytes
bytearray
memoryview
str
```

Prefer bytes-like inputs when data arrives from disk/network to avoid an unnecessary UTF-8 decode.

## 18.2 Return types

Returns ordinary Python JSON types:

```text
dict[str, Any]
list[Any]
str
int
float
bool
None
```

There is no schema/model construction. Feed the result into Pydantic/domain validation when required.

## 18.3 Nesting limit

orjson enforces a maximum nesting depth (documented as 1024 nested arrays/objects). This protects the parser/stack from extreme recursive structures.

## 18.4 Key cache

The parser maintains a bounded cache for frequently repeated short map keys (documented around 2048 entries and key length constraints), improving allocation/performance for typical JSON object schemas.

Do not rely on cache behavior semantically.

## 18.5 Decode errors

`orjson.JSONDecodeError` subclasses the standard JSON decode/value error hierarchy so callers can catch it specifically or more generally as `ValueError` where appropriate.

```python
try:
    data = orjson.loads(payload)
except orjson.JSONDecodeError as exc:
    ...
```

## 18.6 Input size

Nesting limit does not cap total bytes/elements. Apply request/body size limits before `loads()` at untrusted boundaries.

---

# 19) `dumps()` errors and unsupported values

`orjson.JSONEncodeError` is raised for unsupported/unserializable conditions such as:

- unsupported object type without usable `default`;
- invalid Unicode/surrogates;
- integers outside supported range;
- unsupported dict key without `OPT_NON_STR_KEYS`;
- problematic datetime/tzinfo;
- unsupported NumPy layout/dtype when NumPy serialization is requested.

Normalize encode failures at API boundaries rather than exposing raw reprs of the failing object, which may include secrets.

---

# 20) JSON Lines / NDJSON patterns

orjson does not provide a special JSONL parser/writer, but JSON Lines is easy to compose.

## 20.1 Write

```python
with path.open("wb") as f:
    for item in items:
        f.write(orjson.dumps(item, option=orjson.OPT_APPEND_NEWLINE))
```

## 20.2 Read line-by-line

```python
with path.open("rb") as f:
    for line in f:
        if line.strip():
            item = orjson.loads(line)
```

## 20.3 Streaming benefit

JSONL lets you process one record at a time without building a giant JSON array.

For high-throughput structured internal data, Arrow/Parquet may still be superior. Use JSONL when line-oriented interoperability/debuggability is valuable.

---

# 21) Bytes, files, sockets, and HTTP boundaries

## 21.1 File output

```python
path.write_bytes(orjson.dumps(data))
```

## 21.2 ASGI/HTTP

ASGI response bodies are bytes, matching `dumps()` naturally. Avoid converting `bytes -> str -> bytes`.

## 21.3 Sockets

Frame JSON explicitly if writing multiple objects to a raw byte stream. JSON itself has no built-in message framing:

```text
length-prefix
newline-delimited JSON
HTTP message boundaries
WebSocket message boundaries
```

Do not concatenate compact JSON values and hope `loads()` knows where one ends.

## 21.4 gRPC

gRPC already frames protobuf messages. If one protobuf field contains JSON bytes, that is an **application sub-format** and should be explicitly named/versioned.

---

# 22) GIL, concurrency, and async applications

## 22.1 GIL behavior

The project states that the GIL is held for the duration of `dumps()` and `loads()` in the current implementation.

Therefore:

```text
Rust/native implementation != automatic Python thread parallelism
```

Multiple threads serializing large JSON documents do not necessarily scale linearly across CPU cores.

## 22.2 AsyncIO

Calling orjson is synchronous CPU work. Small/normal payloads are usually fast enough to execute inline. For very large JSON payloads, serialization can block the event loop and should be profiled.

Do not automatically offload every `orjson.dumps()` to a thread; thread scheduling overhead can exceed serialization time and the GIL limits benefit.

## 22.3 Process parallelism

For bulk independent JSON encoding where CPU is truly dominant, process-level parallelism can scale, but IPC serialization/copy overhead may negate the gain. Benchmark the full pipeline.

---

# 23) Performance engineering and benchmarking

## 23.1 Benchmark your shape

orjson's README includes comparative benchmarks, but application performance depends on:

- object shape/depth;
- string size/Unicode;
- dataclasses/NumPy;
- sorting/indent options;
- custom `default`;
- output copy/write behavior;
- validation before serialization.

## 23.2 Avoid needless conversions

Bad:

```python
payload = orjson.dumps(json.loads(stdlib_json_string))
```

unless normalization is actually required.

## 23.3 Keep bytes

Bad:

```python
text = orjson.dumps(data).decode("utf-8")
body = text.encode("utf-8")
```

Prefer bytes end-to-end where downstream supports it.

## 23.4 Sorting cost

`OPT_SORT_KEYS` is a deliberate performance tradeoff. Do not enable globally for “clean JSON” in latency-sensitive endpoints.

## 23.5 Dataclass/NumPy native paths

Native serialization is generally more efficient than converting to intermediate Python dict/list structures first. Use native paths unless custom semantics require otherwise.

## 23.6 Optimize architecture before serializer microseconds

If a FastMCP request spends 100 ms querying a daemon and 0.2 ms serializing JSON, replacing orjson options will not matter. Profile end-to-end.

---

# 24) Pydantic integration

## 24.1 Pydantic V2 already has Rust-backed serialization

Pydantic V2 uses `pydantic-core` and provides `model_dump_json()`. Do not assume a mandatory performance benefit from converting every model to dict then using orjson.

## 24.2 Clean integration

When an external API specifically needs orjson bytes and model validation/filtering must happen first:

```python
validated = Model.model_validate(input_data)
python_data = validated.model_dump(mode="json")
payload = orjson.dumps(python_data)
```

`mode="json"` converts many Pydantic-supported Python types into JSON-compatible representations before orjson.

## 24.3 Avoid raw `__dict__`

```python
orjson.dumps(model.__dict__)  # bad contract
```

This bypasses aliases, serializers, field exclusion, computed fields, subclass exposure rules, and Pydantic's serialization contract.

## 24.4 Custom serializer precedence

Decide which layer owns special formatting:

```text
Pydantic field/model serializers -> semantic API representation
orjson default/options            -> final JSON encoding concerns
```

Prefer semantic transformation in Pydantic/domain code and keep orjson configuration simple.

---

# 25) FastAPI integration

FastAPI/Starlette ecosystems expose ORJSON response integrations in common deployments. The key architecture rule is unchanged: FastAPI response-model filtering/validation should occur before final JSON encoding.

Do not return pre-serialized orjson bytes from a normal endpoint merely to “make it faster” if doing so bypasses:

- response model filtering;
- status/media-type behavior;
- middleware expectations;
- OpenAPI-consistent serialization.

Use an explicit response class/raw response only when taking ownership of those semantics deliberately.

For SSE/JSONL streaming, encode each structured event/item independently rather than building one giant JSON document first.

---

# 26) FastMCP integration

## 26.1 Good uses

- compact serialization of internal cached tool metadata;
- persistence/transport of genuinely JSON-defined auxiliary data;
- logging/event payloads after redaction;
- JSON Lines diagnostics/export;
- custom HTTP side routes where bytes are the response body.

## 26.2 MCP tool results

FastMCP already maps Python/Pydantic return values into MCP content/structured content. Do not turn every tool result into orjson bytes; that hides structure from MCP clients/LLMs.

Prefer:

```python
@mcp.tool
def get_node(...) -> NodeResult:
    return NodeResult(...)
```

not:

```python
return orjson.dumps({...})
```

unless the tool contract explicitly returns a binary/JSON file payload.

## 26.3 Daemon boundary

If FastMCP talks to a daemon via gRPC/protobuf, keep that boundary protobuf. Use orjson only where a field or separate interface is intentionally JSON.

---

# 27) Protobuf/gRPC integration

## 27.1 Do not serialize protobuf with orjson directly

ProtoJSON has specific field-name, bytes, enum, 64-bit integer, well-known-type, and presence mappings. Use `google.protobuf.json_format`.

Bad:

```python
orjson.dumps(proto.__dict__)
```

## 27.2 If you need orjson after protobuf mapping

```python
from google.protobuf import json_format

data = json_format.MessageToDict(proto)
payload = orjson.dumps(data)
```

Validate with golden ProtoJSON fixtures because `MessageToDict` options control semantic mapping and orjson controls only final JSON text encoding.

## 27.3 JSON bytes inside protobuf

If a protobuf `bytes`/`string` field carries JSON, define:

```text
content meaning
schema/version
UTF-8 requirement
maximum size
validation owner
whether Fragment/cache storage assumes already validated JSON
```

Otherwise you have created an undocumented protocol inside a protocol.

---

# 28) Security and trust boundaries

## 28.1 `Fragment` is highest-risk surface

Never wrap untrusted bytes in `orjson.Fragment`. It bypasses JSON validation.

## 28.2 `default` can leak data

Avoid generic `str(obj)`/`repr(obj)` fallbacks. Objects may reveal tokens, file paths, SQL, private fields, or memory-like identifiers.

## 28.3 Non-string key collisions

`OPT_NON_STR_KEYS` can create duplicate wire keys with lossy downstream interpretation. Normalize/validate keys for security-sensitive mappings.

## 28.4 Payload size

Nesting depth is bounded, but total input size/elements still need HTTP/gRPC/application limits.

## 28.5 JSON is not canonical signing format by default

`OPT_SORT_KEYS` does not solve duplicate keys, Unicode normalization, number canonicalization, or formal canonical JSON requirements. Use a defined canonicalization standard for signatures.

## 28.6 Sensitive fields

Redact at the model/domain layer before serialization. A serializer should not be your primary secret-access-control mechanism.

---

# 29) Testing and compatibility fixtures

## 29.1 Round trip

```python
payload = orjson.dumps(data)
assert orjson.loads(payload) == expected
```

Be careful with tuples (JSON arrays decode to lists), datetimes (decode to strings), UUID (string), and other typed values; JSON roundtrip preserves JSON data, not original Python runtime types.

## 29.2 Golden bytes

Use golden bytes only when exact JSON text is part of the contract (sorted keys, formatting, canonical-ish cache key). Otherwise compare parsed JSON structure.

## 29.3 Option matrix tests

Test the exact configured combination:

```text
naive UTC
UTC Z
microsecond omission
strict integer
non-string keys
sort keys
passthrough/default
NumPy
```

## 29.4 Error fixtures

Test:

- invalid UTF-8;
- surrogate strings;
- oversized integer;
- unsupported object;
- `default` recursion/failure;
- invalid JSON input;
- NaN/Infinity input rejection;
- nesting limit as appropriate;
- Fragment trust invariant.

## 29.5 Cross-version output

For externally consumed exact-text JSON, compare representative 3.11.x→3.12.0 output before deployment because the 3.12 serializer implementation was substantially rewritten even though semantic versioning should preserve supported behavior.

---

# 30) Upgrade guidance for 3.12

## 30.1 Stable target

```text
orjson==3.12.0
```

## 30.2 Serializer rewrite

3.12.0 substantially rewrites serialization internals. Even with API compatibility, performance and obscure edge cases can shift. Run correctness plus benchmark suites.

## 30.3 Python 3.15/platform packaging

If adopting Python 3.15, verify Linux base image/architecture wheel availability. The 3.12 release changes manylinux/platform support for the newest interpreter and drops some architecture wheels.

## 30.4 Deprecation cleanup

Remove legacy no-op flags from new code:

```text
OPT_SERIALIZE_DATACLASS
OPT_SERIALIZE_UUID
```

Their presence can mislead reviewers/agents into thinking those types require opt-in.

## 30.5 Upgrade checklist

```text
[ ] import on every deployment architecture
[ ] dumps/loads golden semantic tests
[ ] datetime option tests
[ ] large integer tests
[ ] default callback tests
[ ] dataclass/NumPy tests if used
[ ] JSONL/Fragment tests if used
[ ] representative latency/throughput benchmark
[ ] memory benchmark for large payloads
[ ] Python 3.15 wheel/base-image validation if applicable
```

---

# 31) Anti-pattern inventory

- Calling orjson a drop-in clone of `json` without noting behavior differences.
- Decoding `dumps()` bytes to str only to re-encode them for HTTP/file output.
- Using `default=str` for arbitrary objects.
- Forgetting to raise `TypeError` in `default`, silently producing `null`.
- Enabling `OPT_NON_STR_KEYS` without checking key collisions.
- Enabling `OPT_SORT_KEYS` everywhere for aesthetics.
- Treating sorted keys as cryptographic canonical JSON.
- Using `Fragment` with user/database bytes that were never validated.
- Assuming native Rust means the GIL is released/parallel threads scale.
- Blocking an AsyncIO event loop with enormous JSON documents without profiling.
- Serializing huge NumPy arrays as JSON when a binary format is appropriate.
- Calling `dataclasses.asdict()` first when native dataclass serialization is sufficient.
- Serializing Pydantic `__dict__` and bypassing model serializers/exclusion/security.
- Serializing protobuf internals with orjson instead of ProtoJSON mapping.
- Returning orjson bytes from FastMCP tools and thereby hiding structured content.
- Assuming JSON roundtrip preserves Python runtime types like datetime/UUID/tuple.
- Accepting unbounded request bodies because the parser has a nesting limit.
- Emitting 64-bit numeric identifiers to JavaScript clients without precision policy.
- Ignoring wheel/platform constraints during Python 3.15 upgrades.

---

# 32) Dense API/option matrices

## 32.1 Core API

| Need | API |
|---|---|
| serialize | `orjson.dumps(obj, default=None, option=None) -> bytes` |
| deserialize | `orjson.loads(bytes|bytearray|memoryview|str) -> Any` |
| raw trusted embedded JSON | `orjson.Fragment(bytes_or_str)` |
| encode error | `orjson.JSONEncodeError` |
| decode error | `orjson.JSONDecodeError` |

## 32.2 Options

| Option | Purpose | Main caveat |
|---|---|---|
| `OPT_APPEND_NEWLINE` | append `\n` | useful JSONL/text output |
| `OPT_INDENT_2` | pretty print 2 spaces | larger/slower output |
| `OPT_NAIVE_UTC` | treat naive datetime as UTC | changes semantic interpretation |
| `OPT_NON_STR_KEYS` | serialize supported non-string dict keys | collision/duplicate-key risk |
| `OPT_OMIT_MICROSECONDS` | remove microseconds | loses precision |
| `OPT_PASSTHROUGH_DATACLASS` | send dataclasses to `default` | gives up native fast path |
| `OPT_PASSTHROUGH_DATETIME` | send datetime/date/time to `default` | custom callback owns format |
| `OPT_PASSTHROUGH_SUBCLASS` | send supported subclasses to `default` | useful for semantic wrappers |
| `OPT_SERIALIZE_DATACLASS` | deprecated no-op | don't use new code |
| `OPT_SERIALIZE_NUMPY` | NumPy native support | dtype/layout constraints |
| `OPT_SERIALIZE_UUID` | deprecated no-op | UUID already native |
| `OPT_SORT_KEYS` | lexicographically sort object keys | performance; not canonical spec |
| `OPT_STRICT_INTEGER` | enforce 53-bit integer range | rejects valid 64-bit use cases |
| `OPT_UTC_Z` | UTC offset as `Z` | formatting contract change |

## 32.3 Native-type behavior

| Type | Dumps | Loads returns |
|---|---|---|
| dict | JSON object | dict |
| list | array | list |
| tuple | array | list |
| str | string | str |
| bytes | not generic JSON value; use encoding/default | — |
| int | number within range | int |
| float | number; non-finite -> null | float/int depending token |
| bool | boolean | bool |
| None | null | None |
| datetime/date/time | formatted string | str |
| UUID | string | str |
| dataclass | object | dict |
| NumPy | with option | list/number JSON types |

## 32.4 Integration decision

| Boundary | Preferred serializer |
|---|---|
| gRPC RPC message | Protobuf binary |
| protobuf -> official JSON | `google.protobuf.json_format` |
| validated Pydantic API output where bytes wanted | `model_dump(mode="json")` + orjson if justified |
| FastMCP structured tool result | Python/Pydantic object, not raw orjson bytes |
| large tabular/vector data | Arrow/Parquet/binary, not JSON by default |
| JSONL diagnostics/export | orjson + `OPT_APPEND_NEWLINE` |

---

# 33) Agent implementation checklist

```text
VERSION / PLATFORM
[ ] Pin orjson 3.12.0.
[ ] Confirm CPython and wheel platform support.
[ ] Validate Python 3.15 base-image compatibility if applicable.

ENCODING
[ ] Keep dumps() output as bytes where possible.
[ ] Define datetime timezone/microsecond contract.
[ ] Define integer precision contract for JSON consumers.
[ ] Use UTF-8 strictly; fix invalid source text rather than dropping bytes/chars.

CUSTOM TYPES
[ ] Prefer native dataclass/UUID/datetime support.
[ ] Enable NumPy only when needed.
[ ] `default` uses explicit type allowlist.
[ ] `default` always raises TypeError for unknown types.
[ ] Never use default=str as generic fallback.

OPTIONS
[ ] Combine options with bitwise OR.
[ ] OPT_NON_STR_KEYS only after collision analysis.
[ ] OPT_SORT_KEYS only when exact ordering is required.
[ ] Remove deprecated no-op serialization flags.
[ ] Passthrough options only when custom semantics are necessary.

TRUST / SECURITY
[ ] Fragment only contains trusted validated JSON.
[ ] Bound total input/request size.
[ ] Redact secrets before serialization.
[ ] Do not treat sorted JSON as canonical signature format.

INTEGRATION
[ ] Pydantic serialization semantics happen before orjson when models are involved.
[ ] Protobuf JSON uses protobuf json_format, not orjson on message internals.
[ ] FastMCP structured results stay structured.
[ ] gRPC wire payload stays protobuf unless JSON sub-format is intentional/documented.

PERFORMANCE
[ ] Benchmark representative object shapes/options.
[ ] Avoid bytes->str->bytes conversions.
[ ] Avoid dict/list intermediate conversions where native support exists.
[ ] Remember GIL is held during dumps/loads.
[ ] Use binary formats for large numerical/tabular payloads where appropriate.
```

---

# Source index

[ORJSON-PYPI]: https://pypi.org/project/orjson/
[ORJSON-README]: https://github.com/ijl/orjson/tree/3.12.0
[ORJSON-RELEASE]: https://github.com/ijl/orjson/releases/tag/3.12.0

