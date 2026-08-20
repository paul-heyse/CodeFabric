# Protocol Buffers (`protobuf`) in Python — advanced technical reference / feature-category catalog

This reference is designed for **LLM coding agents and engineers using Protocol Buffers as a durable Python/RPC schema layer**, especially where generated messages back gRPC/FastMCP daemon interfaces. The emphasis is not only “how to serialize a message,” but **schema semantics, field presence, generated-code behavior, runtime/compiler compatibility, binary/JSON stability, reflection, dynamic messages, performance, and safe schema evolution**.

## Version / source anchors

**Primary deployable Python runtime target: `protobuf==7.36.0`**, released on PyPI **2026-08-20**, and the latest stable Python package as of this reference date. PyPI declares **Python >=3.10**.

### The critical version-numbering distinction

Protocol Buffers uses **two related version surfaces**:

```text
protobuf repository / protoc release line: 36.x
Python protobuf runtime package line:       7.36.x
```

The shared release number is the **36.x** portion; languages have language-specific major numbers. Therefore:

```text
"protobuf 7.36.0" == Python runtime package version
"protoc 36.x"     == compiler/monorepo release family
```

Do not compare these numerically as if `7.36.0` and `36.0` were unrelated or as if the Python package should be named `36.0`. Official compatibility/version pages may lag a newly published package—the Python 7.36.0 release landed on the date of this reference—so use PyPI/tagged artifacts for the deployable package boundary and official compatibility docs for the policy.

Other important compatibility anchors:

- Python generated code since **3.20.0** uses the modern thin-descriptor style and has an unusually long compatibility window compared with many other protobuf languages.
- Generated code produced by a **newer** compiler must not in general be paired with an older runtime unless the language-specific compatibility guarantee says it is supported.
- The binary wire format has extremely strong compatibility guarantees when schemas evolve according to protobuf rules.
- ProtoJSON has different compatibility hazards; many wire-safe schema changes are **not** JSON-safe.

Primary sources:

- PyPI: https://pypi.org/project/protobuf/
- Version support: https://protobuf.dev/support/version-support/
- Cross-version runtime guarantee: https://protobuf.dev/support/cross-version-runtime-guarantee/
- Python generated code: https://protobuf.dev/reference/python/python-generated/
- Python tutorial: https://protobuf.dev/getting-started/pythontutorial/
- Field presence: https://protobuf.dev/programming-guides/field_presence/
- ProtoJSON: https://protobuf.dev/programming-guides/json/
- Proto3 language guide: https://protobuf.dev/programming-guides/proto3/
- Editions overview: https://protobuf.dev/editions/overview/
- Encoding/wire guide: https://protobuf.dev/programming-guides/encoding/
- Releases: https://github.com/protocolbuffers/protobuf/releases

---

## Feature inventory

This reference covers: compiler/runtime/package versioning, `.proto` language/Edition selection, generated Python modules and `.pyi`, messages, scalars, explicit/implicit presence, nested messages, repeated fields, maps, oneofs, enums, unknown fields, binary wire encoding, deterministic serialization caveats, merge/copy semantics, ProtoJSON, text format, descriptors/reflection, descriptor pools, message factories, dynamic messages, `Any`, well-known types, custom options/extensions, schema evolution, wire-safe vs JSON-safe changes, Python gencode/runtime compatibility, packaging/imports, the upb-backed runtime, performance/memory, testing/security, gRPC integration, Pydantic/orjson boundaries, Editions 2026 transition, and migration/upgrade policy.

---

# Proposed comprehensive documentation map

0. Scope, versioning, and protobuf mental model
1. Installation, compiler/runtime topology, and project layout
2. Syntax/edition selection: Editions, proto3, proto2
3. First `.proto` and generation pipeline
4. Generated Python module anatomy and type stubs
5. `Message` base API and object lifecycle
6. Scalar fields, defaults, type assignment
7. Field presence: explicit vs implicit
8. Singular message fields and ownership
9. Repeated scalar/message fields
10. Maps and map-entry semantics
11. `oneof` and mutually exclusive state
12. Enums and unknown enum values
13. Strings, bytes, numeric ranges, floats
14. Unknown fields and forward compatibility
15. Binary wire format mental model
16. Serialization and deterministic output
17. Parsing, merge, copy, equality, and clear operations
18. ProtoJSON mapping and compatibility
19. TextFormat and why it is not a stable interchange wire format
20. Descriptors and generated reflection
21. `DescriptorPool`, symbol lookup, and file descriptors
22. `message_factory` and dynamic message classes
23. `Any` and type URLs
24. Well-known types: Timestamp, Duration, FieldMask, Struct, wrappers
25. Extensions and custom options
26. Schema evolution and binary compatibility
27. JSON-specific schema evolution hazards
28. Package/module/import rules
29. Python type stubs and static typing
30. Runtime/compiler/generated-code compatibility
31. Python runtime architecture and upb
32. Performance and serialization sizing
33. Memory behavior and object reuse
34. Threading/concurrency and mutation rules
35. Validation limitations and semantic invariants
36. Security and untrusted payloads
37. Testing contracts and compatibility
38. gRPC integration
39. Pydantic/orjson/FastMCP integration boundaries
40. Editions 2023/2024/2026 and feature lifecycle
41. Upgrade/migration guidance for 7.36
42. Anti-pattern inventory
43. Dense API/reference matrices
44. Agent schema-authoring checklist

---

# Protocol Buffers Advanced — 0) Scope, versioning, and mental model

## 0.0 What Protocol Buffers gives you

Protocol Buffers is a **schema language + compiler + generated runtime model + binary/JSON mappings**. In Python the normal flow is:

```text
.proto source
  -> protoc 36.x (+ plugins such as grpc_python)
      -> *_pb2.py descriptors/message bindings
      -> *_pb2.pyi type stubs (when requested)
      -> *_pb2_grpc.py for gRPC service bindings
          -> protobuf 7.36.0 runtime
              -> binary / JSON / text / reflection operations
```

The `.proto` file—not the Python class definition—is the canonical durable contract.

## 0.1 What protobuf is not

Protocol Buffers is not:

- a semantic validation framework like Pydantic;
- a database schema/migration system;
- an authorization contract;
- a canonical hashing/signature format by default;
- arbitrary JSON schema;
- a guarantee that every source-level change is wire compatible.

Generated messages enforce protobuf field types and structural semantics, but domain constraints such as “end must be after start” or “exactly one of these repeated collections must be nonempty” need application validation.

## 0.2 Three compatibility layers

Always reason about separately:

```text
1. binary wire compatibility
2. ProtoJSON compatibility
3. generated-code/runtime/toolchain compatibility
```

A schema change can be safe in one and unsafe in another.

## 0.3 Field-number identity

On the binary wire, a field's durable identity is primarily its **field number + wire type**, not its source name. Renaming a field can therefore be binary-compatible while breaking JSON consumers because ProtoJSON exposes field names.

## 0.4 Agent invariant

Never modify a published `.proto` by “cleaning up” field numbers. Once a field number has shipped, it is part of the wire history. Removed field numbers/names should generally be `reserved` to prevent accidental reuse.

---

# 1) Installation, compiler/runtime topology, and project layout

## 1.1 Runtime

```bash
python -m pip install "protobuf==7.36.0"
```

The Python runtime imports under `google.protobuf`:

```python
from google.protobuf import json_format
from google.protobuf.message import Message
```

## 1.2 Compiler

The standalone compiler is `protoc`. For a gRPC Python project, `grpcio-tools` provides a Python-invokable protoc/toolchain:

```bash
python -m pip install "grpcio-tools==1.83.0"
python -m grpc_tools.protoc ...
```

Do not assume the compiler version equals the Python runtime version string. Record both.

## 1.3 Recommended layout

```text
repo/
  proto/
    codefabric/v1/*.proto
  src/package/rpc/generated/
    codefabric/v1/*_pb2.py
    codefabric/v1/*_pb2.pyi
    codefabric/v1/*_pb2_grpc.py
  scripts/
    generate_protos.sh
  tests/
    proto/
      test_wire_compat.py
      test_json_compat.py
      test_descriptor_contract.py
```

## 1.4 Build-vs-runtime dependencies

Production runtime usually needs `protobuf` and `grpcio`, not the code generator. Generate artifacts in CI/build. This improves reproducibility and avoids shipping compiler/toolchain dependencies to a runtime container.

---

# 2) Syntax/edition selection: Editions, proto3, proto2

## 2.1 New schema stance

Modern protobuf development is moving toward **Editions**, where language features are controlled through edition/feature settings rather than periodically creating `proto4`, `proto5`, etc.

Examples:

```proto
edition = "2023";
package codefabric.v1;
```

Legacy syntax declarations remain common:

```proto
syntax = "proto3";
```

```proto
syntax = "proto2";
```

## 2.2 Presence difference is central

Editions use explicit presence by default for singular fields. Proto3 historically used implicit presence for ordinary scalar fields unless marked `optional`. This difference affects patch/update semantics, JSON behavior, merge behavior, and whether `HasField()` is valid.

## 2.3 Recommendation for new contracts

If your deployed toolchain fully supports the target Edition, prefer Editions for new long-lived schemas because feature behavior is more explicit. If ecosystem compatibility requires proto3, use `optional` deliberately where presence matters.

Do not convert a mature proto2/proto3 schema to Editions as a formatting exercise; migration is a contract/toolchain change and should follow official Editions migration guidance.

---

# 3) First `.proto` and generation pipeline

```proto
syntax = "proto3";

package codefabric.v1;

message Node {
  string id = 1;
  string kind = 2;
  optional string documentation = 3;
  repeated string tags = 4;
  map<string, string> attributes = 5;
}

message GetNodeRequest {
  string id = 1;
}

message GetNodeResponse {
  Node node = 1;
}
```

Generate:

```bash
python -m grpc_tools.protoc \
  -I proto \
  --python_out=src/package/rpc/generated \
  --pyi_out=src/package/rpc/generated \
  --grpc_python_out=src/package/rpc/generated \
  proto/codefabric/v1/graph.proto
```

Use `.pyi` generation when type-checking generated messages with Pyright/mypy. Do not manually wrap every protobuf class in a second handwritten typing class solely because older code generation lacked stubs.

---

# 4) Generated Python module anatomy and type stubs

## 4.1 `_pb2.py`

The generated module embeds/constructs descriptors for the source `.proto`. Modern Python generated code is intentionally compact: the runtime/metaclass machinery creates concrete message classes from descriptors.

```python
from package.rpc.generated.codefabric.v1 import graph_pb2

node = graph_pb2.Node(id="n1", kind="function")
```

Generated classes subclass `google.protobuf.message.Message` conceptually, but **are not designed for user subclassing**.

## 4.2 `_pb2.pyi`

Contains static type information for generated classes and fields. It does not affect runtime.

## 4.3 `_pb2_grpc.py`

This is generated by the gRPC plugin, not core protobuf. It contains service stubs/servicers and uses the `_pb2.py` message classes as serializers/deserializers.

## 4.4 Do not edit generated files

Put custom helpers in handwritten modules:

```python
# graph_converters.py

def node_to_domain(msg: graph_pb2.Node) -> Node:
    ...
```

Generated-code changes should be made in `.proto` and regenerated.

---

# 5) `Message` base API and object lifecycle

Generated message classes share the `Message` interface. High-value methods include:

```text
SerializeToString(**kwargs)
SerializePartialToString(**kwargs)
ParseFromString(serialized)
MergeFromString(serialized)
ByteSize()
Clear()
ClearField(name)
HasField(name)
ListFields()
WhichOneof(name)
CopyFrom(other)
MergeFrom(other)
IsInitialized()
FindInitializationErrors()
DiscardUnknownFields()
UnknownFields() / runtime-specific unknown field access surfaces
```

## 5.1 Construction

```python
msg = graph_pb2.Node(
    id="n1",
    kind="function",
    tags=["public", "python"],
    attributes={"module": "pkg.mod"},
)
```

## 5.2 `FromString`

Generated classes expose `FromString`:

```python
msg = graph_pb2.Node.FromString(payload)
```

Equivalent explicit pattern:

```python
msg = graph_pb2.Node()
msg.ParseFromString(payload)
```

## 5.3 Messages are mutable

Do not use a protobuf object as if it were an immutable value object. If one part of the program keeps a reference while another mutates it, the observer sees mutations.

## 5.4 Copy before ownership transfer when needed

```python
copy = graph_pb2.Node()
copy.CopyFrom(original)
```

This is especially important when caching, sharing across tasks, or retaining request messages after a handler returns.

---

# 6) Scalar fields, defaults, and type assignment

## 6.1 Scalar assignment

```python
msg.id = "n1"
msg.count = 42
msg.enabled = True
```

Wrong Python types generally raise `TypeError`/range errors through the runtime.

## 6.2 Default values are observable values, not necessarily presence

Reading an unset scalar returns its protobuf default:

```text
string -> ""
bytes  -> b""
bool   -> False
numeric -> 0
first enum value -> default enum
```

This does **not** mean the field is explicitly present under every presence discipline.

## 6.3 Numeric ranges

Integer fields correspond to protobuf bounded integer types. Python's arbitrary-precision `int` does not mean an `int32` field accepts any integer. The runtime enforces field range.

## 6.4 Float semantics

Binary protobuf can represent IEEE special values; ProtoJSON represents special float values with defined string encodings. Do not assume JSON-number behavior and protobuf binary behavior are identical.

---

# 7) Field presence: explicit vs implicit

This is one of the most important protobuf concepts for agent-authored schema design.

## 7.1 Explicit presence

Explicit presence distinguishes:

```text
field unset
vs
field explicitly set to its default value
```

```python
assert not msg.HasField("documentation")
msg.documentation = ""
assert msg.HasField("documentation")
msg.ClearField("documentation")
```

Proto3 `optional`, proto2 optional/required, singular message fields, and Editions defaults provide explicit-presence behavior as applicable.

## 7.2 Implicit scalar presence

For ordinary implicit-presence proto3 scalar fields, `HasField("field")` is not available. Setting the field to its default is effectively indistinguishable from unset in ordinary serialized presence semantics.

## 7.3 Why it matters for patch/update APIs

Suppose `enabled=false` is a legitimate requested update. With implicit presence, a patch message cannot distinguish “do not modify enabled” from “set enabled to false” without another mechanism such as FieldMask or explicit optional presence.

## 7.4 Recommended rule

For fields used in patch/partial-update/request-intent semantics, use explicit presence or a well-designed FieldMask/oneof model. Do not rely on default-value guessing.

## 7.5 Presence matrix

| Field shape | Presence? |
|---|---|
| singular message | explicit |
| oneof member | explicit through oneof |
| proto3 ordinary scalar | implicit unless `optional` |
| proto3 `optional` scalar | explicit |
| Editions singular field | explicit by default |
| repeated | no singular presence; empty/nonempty collection |
| map | no singular presence; empty/nonempty mapping |

---

# 8) Singular message fields and ownership

## 8.1 Cannot assign a submessage directly

Bad:

```python
response.node = graph_pb2.Node(id="n1")  # generally invalid
```

Use `CopyFrom`:

```python
response.node.CopyFrom(graph_pb2.Node(id="n1"))
```

Or mutate child fields:

```python
response.node.id = "n1"
response.node.kind = "function"
```

## 8.2 Presence is activated by mutation

Merely reading a child does not necessarily mark it present. Mutating the child or calling `SetInParent()` establishes presence.

```python
assert not response.HasField("node")
_ = response.node.id
assert not response.HasField("node")
response.node.id = "n1"
assert response.HasField("node")
```

## 8.3 Parent owns submessage

Treat returned child message handles as views into the parent object. Copy if you need an independent object lifetime.

---

# 9) Repeated scalar/message fields

## 9.1 Repeated scalars

```python
msg.tags.append("python")
msg.tags.extend(["public", "indexed"])
msg.tags[:] = ["a", "b"]
del msg.tags[:]
```

Repeated containers emulate Python sequences but are protobuf-owned containers, not ordinary lists.

## 9.2 Repeated messages

Use `.add()` or append/extend according to supported container semantics:

```python
child = response.nodes.add()
child.id = "n1"
child.kind = "function"
```

For clarity/performance, construct directly into the owned container instead of creating many temporary messages then copying when unnecessary.

## 9.3 No `HasField` for repeated

Empty vs never-populated is not singular presence. If the distinction matters, model it explicitly.

## 9.4 Clearing may retain capacity

Clearing repeated fields resets logical contents but may not immediately release all underlying capacity. See memory behavior in §33.

---

# 10) Maps and map-entry semantics

## 10.1 Scalar map

```proto
map<string, string> attributes = 5;
```

Python:

```python
msg.attributes["module"] = "pkg.mod"
msg.attributes.update({"language": "python"})
```

## 10.2 Message-valued map surprise

Accessing a missing message-valued map key can create an entry, unlike a normal read-only dictionary lookup. Generated Python provides `get_or_create()` on message maps to make this mutating behavior explicit in code.

Prefer:

```python
child = msg.message_map.get_or_create(key)
```

when creation is intended.

## 10.3 Map ordering

Do not treat protobuf map iteration or binary ordering as a durable canonical order. If ordering is part of the business contract, model an ordered repeated message explicitly.

## 10.4 JSON object mapping

ProtoJSON maps protobuf maps to JSON objects subject to key conversion rules. This has different duplicate/order behavior than binary messages; do not use map ordering as a signature basis.

---

# 11) `oneof` and mutually exclusive state

```proto
message Subject {
  oneof target {
    string node_id = 1;
    string file_path = 2;
    string symbol = 3;
  }
}
```

## 11.1 Selection

```python
subject.node_id = "n1"
assert subject.WhichOneof("target") == "node_id"

subject.file_path = "src/a.py"
assert subject.WhichOneof("target") == "file_path"
assert not subject.HasField("node_id")
```

Setting one member clears the previous member.

## 11.2 Group presence

`HasField()` and `ClearField()` can use the oneof group name:

```python
subject.ClearField("target")
assert subject.WhichOneof("target") is None
```

## 11.3 Design use

Use oneof when **exactly zero/one of several representations** should be active and that exclusivity is part of the wire schema. Do not use three optional fields and rely on application convention when the protocol can express the invariant directly.

---

# 12) Enums and unknown enum values

## 12.1 Define zero value intentionally

Proto3/Edition designs commonly require/encourage a zero-valued first enum. Make it semantically safe:

```proto
enum NodeKind {
  NODE_KIND_UNSPECIFIED = 0;
  NODE_KIND_FUNCTION = 1;
  NODE_KIND_CLASS = 2;
}
```

Avoid making the zero default mean an active business state like `DELETED`.

## 12.2 Unknown values

Modern protobuf runtimes preserve unknown enum numeric values in many scenarios rather than failing, enabling forward compatibility. Application logic should have a fallback for values not known to the current generated enum vocabulary.

## 12.3 JSON mapping

ProtoJSON normally uses enum names (with numeric handling rules/options). Renaming enum values can therefore be JSON-breaking even when numeric binary representation remains stable.

## 12.4 Never reuse enum numbers casually

As with field numbers, reserve removed enum values/names according to protobuf evolution guidance.

---

# 13) Strings, bytes, numeric ranges, and floats

## 13.1 `string` vs `bytes`

Use `string` for Unicode text. Use `bytes` for arbitrary binary payloads such as serialized Arrow fragments, hashes, compressed blobs, or opaque binary tokens.

## 13.2 JSON behavior

ProtoJSON encodes bytes as base64 text. If human-facing JSON APIs need meaningful structure, wrapping an entire domain object in `bytes` makes the JSON representation opaque.

## 13.3 Signed/unsigned types

Choose a type based on semantic range, not Python convenience. IDs that are textual/stable across languages often belong as string/bytes rather than integers chosen only for compactness.

## 13.4 64-bit integers in JSON

ProtoJSON has special mappings for 64-bit integer precision across JavaScript/JSON ecosystems. Never assume a protobuf `int64` becomes a normal unquoted JSON number in every mapping.

---

# 14) Unknown fields and forward compatibility

## 14.1 Binary preservation

A core protobuf property is that a newer sender can add fields and an older binary-aware receiver can preserve unknown fields while parsing/reserializing, enabling forward compatibility.

## 14.2 Unknown fields can be lost through transformations

The proto3 guide warns that unknown fields can be lost when:

- serializing through JSON;
- iterating known fields and manually copying only them;
- transforming into another representation that lacks unknown-field preservation.

Prefer `CopyFrom`, `MergeFrom`, or direct binary round trips when preserving unknown fields matters.

## 14.3 `DiscardUnknownFields()`

Explicitly discarding unknowns can reduce retained payload/normalize data when intentional, but destroys forward-compatible information.

```python
msg.DiscardUnknownFields()
```

Do not call this automatically at every boundary.

---

# 15) Binary wire format mental model

## 15.1 Tags

Each encoded field key combines:

```text
field_number << 3 | wire_type
```

This is why field numbers are durable identities.

## 15.2 Wire types

Conceptually:

```text
varint             integers/bools/enums
64-bit             fixed64/sfixed64/double
length-delimited   strings/bytes/messages/packed repeated
32-bit             fixed32/sfixed32/float
(groups legacy wire types also exist)
```

## 15.3 Field order is not semantic

Binary parsers accept fields in varying order. Repeated order is semantic for repeated values, but serialized field ordering overall should not be used as a canonical identity.

## 15.4 Unknown fields work because tags are self-describing enough to skip

The decoder can retain/skip fields it does not know because the wire key tells it how to parse their encoded extent.

---

# 16) Serialization and deterministic output

## 16.1 Basic serialization

```python
payload: bytes = msg.SerializeToString()
```

Partial serialization exists for schemas with initialization requirements:

```python
payload = msg.SerializePartialToString()
```

Prefer normal serialization unless you explicitly understand required-field initialization semantics.

## 16.2 Deterministic serialization

```python
payload = msg.SerializeToString(deterministic=True)
```

**Deterministic does not mean canonical across all builds/languages/versions.** The Python `Message` API explicitly warns that deterministic serialization requests consistency for a current build and is not a universal canonical representation.

Therefore do not make protobuf deterministic bytes a permanent cross-version cryptographic canonicalization scheme without a stronger canonical-format policy.

## 16.3 `ByteSize()`

```python
size = msg.ByteSize()
```

Use to measure serialized message size before transport/admission decisions where appropriate.

## 16.4 Length framing

Raw protobuf messages are not intrinsically self-delimiting when concatenated. gRPC supplies its own message framing. For custom files/streams, use an explicit length-delimited framing scheme rather than concatenating `SerializeToString()` outputs blindly.

---

# 17) Parsing, merge, copy, equality, and clear operations

## 17.1 Parse replaces

`ParseFromString()` clears then parses:

```python
msg.ParseFromString(payload)
```

## 17.2 Merge accumulates

`MergeFromString()` merges encoded fields into the existing message. Likewise:

```python
msg.MergeFrom(other)
```

For singular scalar fields, present source values overwrite; for repeated fields, values append; for submessages, merge semantics apply recursively.

## 17.3 Copy replaces

```python
msg.CopyFrom(other)
```

Copies the entire message state of the same type.

## 17.4 Equality

Message equality compares message content/semantics, not raw serialized byte equality. Binary bytes are not a safe equality proxy because serialization order can vary without changing message meaning.

## 17.5 `ListFields()`

Returns present/nonempty fields as `(FieldDescriptor, value)` pairs ordered by field number. Useful for reflection, diagnostics, generic conversion, and tests.

## 17.6 Clear

```python
msg.Clear()
msg.ClearField("field")
```

Logical clear does not guarantee all underlying allocated capacity is returned immediately.


---

# 18) ProtoJSON mapping and compatibility

## 18.1 ProtoJSON is a defined protobuf mapping, not arbitrary `json.dumps`

Use `google.protobuf.json_format` rather than converting generated messages by inspecting `__dict__` or dataclass-style fields.

Typical operations:

```python
from google.protobuf import json_format

text = json_format.MessageToJson(msg)
data = json_format.MessageToDict(msg)

out = graph_pb2.Node()
json_format.Parse(text, out)
json_format.ParseDict(data, out)
```

Exact optional parameters vary by runtime release; check the 7.36.0 API when using advanced name/enum/default formatting controls.

## 18.2 Name mapping

ProtoJSON normally emits lowerCamelCase JSON names derived from protobuf field names unless configured/prescribed otherwise. Field names are visible in JSON, unlike binary wire data where field numbers are primary.

Renaming a protobuf field can therefore be:

```text
binary wire-compatible
but JSON-breaking
```

## 18.3 Unknown fields

ProtoJSON does not provide binary-style unknown-field preservation. A JSON parser generally rejects unknown fields unless configured to ignore them, and a binary message round-tripped through JSON can lose unknown data.

## 18.4 Null

JSON `null` has protobuf-specific mapping behavior. Do not treat null as a universal substitute for “field absent” without checking the field/type mapping.

## 18.5 64-bit numbers

Several 64-bit integer protobuf types map to JSON strings to preserve precision in common JSON ecosystems. This is intentional, not an orjson/Python bug.

## 18.6 Enum names

Enum names are often emitted rather than only numbers. Renaming enum values is therefore a JSON compatibility event.

## 18.7 Agent rule

If your gRPC system also exposes the same messages through JSON/HTTP, maintain a **separate JSON compatibility test suite**. Binary schema compatibility tests are insufficient.

---

# 19) TextFormat and why it is not a stable interchange wire format

Protocol Buffers includes a human-readable text representation through `google.protobuf.text_format`.

Use cases:

- fixtures;
- debugging;
- human-edited internal configuration where explicitly supported;
- golden tests when stable enough for your pinned toolchain.

Do **not** choose TextFormat as a long-term network/storage wire format merely because it is readable. Official compatibility guidance does not give it the same wire-stability guarantees as binary protobuf; field names are part of the representation and parser/printer behavior can evolve.

```python
from google.protobuf import text_format

text = text_format.MessageToString(msg)
parsed = graph_pb2.Node()
text_format.Parse(text, parsed)
```

For durable external interchange, prefer the binary format or a consciously supported ProtoJSON contract.

---

# 20) Descriptors and generated reflection

## 20.1 Every generated message has a descriptor

```python
desc = graph_pb2.Node.DESCRIPTOR
print(desc.full_name)
print(desc.fields_by_name["id"].number)
```

Descriptors expose schema metadata at runtime: fields, field numbers/types, nested messages, enums, oneofs, file/package information, options, and more.

## 20.2 File descriptor

Generated modules expose a module-level `DESCRIPTOR` representing the `.proto` file.

```python
file_desc = graph_pb2.DESCRIPTOR
print(file_desc.name)
print(file_desc.package)
```

## 20.3 Reflection vs application API

Use descriptors for generic tooling, contract validation, dynamic adapters, schema inspection, and code generation. Ordinary business logic should access generated fields directly rather than resolving `FieldDescriptor`s for every call.

## 20.4 Deprecation awareness

Descriptor APIs evolve. For example, older `FieldDescriptor.label`-style checks have modern convenience properties such as `is_repeated`, `is_required`, and presence-related helpers in newer releases. When writing reflection libraries, target current public descriptor properties rather than copying old generated-code examples.

---

# 21) `DescriptorPool`, symbol lookup, and file descriptors

## 21.1 Purpose

A `DescriptorPool` stores and resolves protobuf schema descriptors. Generated code registers descriptors into pools; dynamic systems can load descriptor sets at runtime.

Conceptual API:

```python
from google.protobuf import descriptor_pool

pool = descriptor_pool.Default()
message_desc = pool.FindMessageTypeByName("codefabric.v1.Node")
service_desc = pool.FindServiceByName("codefabric.v1.GraphService")
file_desc = pool.FindFileByName("codefabric/v1/graph.proto")
```

## 21.2 Custom pool

Use a separate pool when loading untrusted/isolated descriptor universes or when you intentionally need multiple schema sets not registered globally.

## 21.3 Serialized descriptor sets

Build systems can emit `FileDescriptorSet` artifacts. These are valuable for:

- runtime reflection without source `.proto` files;
- compatibility analysis;
- dynamic tooling;
- API registries;
- gRPC reflection tooling.

## 21.4 Conflict policy

Descriptor full names are global within a pool. Do not dynamically load two incompatible definitions of the same fully qualified symbol into one pool and expect version dispatch.

---

# 22) `message_factory` and dynamic message classes

Dynamic messages are useful when the schema is discovered at runtime rather than imported as generated Python.

Conceptual flow:

```text
serialized FileDescriptorSet
 -> DescriptorPool
 -> message Descriptor
 -> message_factory / GetMessageClass
 -> dynamic Message class
 -> parse/serialize like generated message
```

Use dynamic messages for generic tooling, proxies, schema registries, or data migration utilities. Prefer generated classes for normal application paths because they provide stronger static typing, clearer imports, and easier refactoring.

Do not generate dynamic classes repeatedly on hot paths; cache by descriptor/full name when appropriate.

---

# 23) `Any` and type URLs

`google.protobuf.Any` packages an arbitrary protobuf message plus a type URL.

```python
from google.protobuf.any_pb2 import Any

wrapped = Any()
wrapped.Pack(node_msg)

assert wrapped.Is(graph_pb2.Node.DESCRIPTOR)

node = graph_pb2.Node()
if wrapped.Unpack(node):
    ...
```

Generated Python also provides helpers such as `TypeName()` on `Any`.

## 23.1 Use case

`Any` is appropriate when the protocol intentionally supports an extensible heterogeneous message slot.

## 23.2 Cost

`Any` moves schema choice from compile-time field type to runtime type URL. Overusing it makes contracts opaque and forces clients to carry descriptor/type registries.

Prefer a normal field/oneof when the known variant set is small and stable.

## 23.3 Security

Never instantiate/dispatch arbitrary types from an untrusted type URL without an allowlist. The payload is data, but runtime type dispatch can turn it into an application capability boundary.

---

# 24) Well-known types

## 24.1 `Timestamp`

`google.protobuf.Timestamp` models an instant. Python helpers convert to/from datetime-like forms depending on API.

Use it instead of ad-hoc epoch integers when cross-language semantic clarity matters.

## 24.2 `Duration`

`Duration` models elapsed time and supports helpers such as conversion to/from `datetime.timedelta` and nanosecond/microsecond units.

## 24.3 `FieldMask`

A `FieldMask` identifies selected fields/paths and is valuable for update/patch APIs:

```proto
message UpdateNodeRequest {
  Node node = 1;
  google.protobuf.FieldMask update_mask = 2;
}
```

This solves many “default value vs not supplied” patch semantics cleanly when combined with explicit field rules.

## 24.4 `Struct`, `Value`, `ListValue`

These represent JSON-like dynamic data. Use them only when the contract truly requires unstructured JSON semantics. A `Struct` everywhere is effectively abandoning most protobuf schema value.

## 24.5 Wrapper types

Legacy wrapper messages (`Int32Value`, etc.) historically provided presence around primitive values. With modern explicit presence/optional/editions, prefer native field presence unless wrappers are needed for compatibility or a specific well-known-type integration.

---

# 25) Extensions and custom options

## 25.1 Custom options

Protobuf descriptors can carry custom options defined via extensions of descriptor option messages. These are useful for codegen/schema metadata such as:

```text
authorization annotations
validation metadata
API exposure hints
storage mapping
semantic type tags
```

Do not make runtime-critical behavior depend on undocumented custom options without validating that every toolchain preserves/understands them.

## 25.2 Extensions vs normal fields

Proto2/extensions remain part of protobuf capability but are more specialized than ordinary fields/Any. For new application protocols, use normal messages/oneofs unless extension points are a deliberate ecosystem design.

## 25.3 Descriptor option mutation

Recent runtime release notes warn/deprecate mutating objects returned by descriptor `GetOptions()`-style APIs. Treat descriptor/options objects as schema metadata, not mutable application configuration.

---

# 26) Schema evolution and binary compatibility

## 26.1 Core safe evolution habits

- Never change an existing field number.
- Never reuse a removed field number.
- Reserve deleted field names and numbers.
- Add new fields using fresh numbers.
- Preserve compatible wire types when changing field types; consult the official compatibility guide for allowed numeric/string-like changes.
- Do not turn a singular field into repeated or vice versa without explicit compatibility analysis.
- Avoid changing oneof membership casually.
- Keep package/full message names stable when `Any`, reflection, or APIs depend on them.

## 26.2 Delete safely

```proto
message Node {
  reserved 6, 7;
  reserved "old_path", "old_kind";

  string id = 1;
  ...
}
```

Reserve to prevent future authors/agents from accidentally assigning the old wire identity to a different semantic field.

## 26.3 Additive changes

Adding optional/repeated/new explicit-presence fields using new numbers is the classic forward/backward compatible path. Old readers ignore/preserve unknowns; new readers see defaults/absence when reading old messages.

## 26.4 Required fields

Proto2 required fields create deployment-order and initialization hazards. Do not add required fields to an already-deployed message.

## 26.5 “Wire-compatible” is not “semantically compatible”

Changing `int32 count` to another wire-compatible numeric field may preserve decoding while altering valid range/meaning. Compatibility tests need semantic fixtures, not only “parse succeeded.”

---

# 27) JSON-specific schema evolution hazards

ProtoJSON has weaker evolution properties than binary protobuf because **names are on the wire** and unknown fields are not retained like binary unknowns.

Potentially JSON-breaking changes include:

- renaming fields;
- renaming enum values;
- removing fields while old JSON producers still send them;
- changing representation of numeric/string types;
- changing JSON names/custom JSON enum strings;
- changing oneof/shape expected by external JSON clients.

If JSON is an external API, version and test it as an API in its own right. Do not claim “protobuf backward compatibility” as proof that JSON clients are safe.

---

# 28) Package/module/import rules

## 28.1 Protobuf `package`

```proto
package codefabric.v1;
```

Defines fully qualified protobuf names such as:

```text
codefabric.v1.Node
codefabric.v1.GraphService
```

This affects descriptors, service paths, `Any` type names, and cross-language identity.

## 28.2 Python module path

Python module paths follow generated output/import layout, not directly the protobuf `package` statement.

## 28.3 Imports

```proto
import "google/protobuf/timestamp.proto";
import "codefabric/v1/common.proto";
```

Configure `-I/--proto_path` roots deterministically. Generated Python imports must resolve in the installed package—not only when executed from the repository root.

## 28.4 Version namespace

For externally consumed APIs, package namespace versioning such as `codefabric.v1` is often worthwhile when a future change may require an incompatible `v2`. Do not bump package version for every additive field.

---

# 29) Python type stubs and static typing

Generate `.pyi`:

```bash
protoc --python_out=... --pyi_out=... file.proto
```

Benefits:

- generated field types visible to Pyright/mypy;
- autocomplete for nested/repeated/map fields;
- method parameter/response types at protobuf boundary;
- less need for handwritten wrappers purely for typing.

Static type checking does not validate cross-field semantics or whether a message is wire-compatible with another deployed version. Keep schema contract tests.

Generated containers have specialized container types; code that annotates every repeated field as ordinary `list[T]` can misrepresent mutation/ownership semantics.

---

# 30) Runtime/compiler/generated-code compatibility

## 30.1 General rule

Do not pair generated code from a newer unsupported compiler with an older runtime. Protobuf uses generated-code version checks/compatibility policies to prevent unsupported combinations.

## 30.2 Python's extended window

Python is a special case because since 3.20 generated code is largely a thin wrapper around embedded descriptors. Official compatibility policy provides a long support window for Python gencode from 3.20 onward relative to newer runtimes.

This does **not** mean “any protoc + any protobuf runtime forever.” Pin/test your actual pairing.

## 30.3 One runtime major in a process

Do not attempt to load multiple incompatible major protobuf runtimes in one Python process. Python packaging/import machinery and native/runtime implementation do not provide application-level multi-runtime isolation.

## 30.4 gRPC pairing

Your effective stack is:

```text
protoc 36.x
protobuf runtime 7.36.0
grpcio-tools 1.83.0 plugin/tool wrapper
grpcio 1.83.0 transport runtime
```

Regenerate and run tests when any of these boundaries change.

---

# 31) Python runtime architecture and upb

Modern Python protobuf uses a high-performance native implementation built around **upb** for core parsing/serialization/message behavior, with Python wrappers presenting the generated `Message` API.

Practical implications:

- performance is substantially better than naive pure-Python field walking;
- message objects have runtime-managed storage semantics different from dataclasses;
- internal implementation details are not a public extension surface;
- descriptor/message behavior can be implemented in native code even though APIs look Pythonic.

Do not depend on undocumented `_message`, upb/C-extension internals, generated descriptor implementation details, or private module objects.

---

# 32) Performance and serialization sizing

## 32.1 Use binary for RPC/hot paths

Binary protobuf is compact and fast relative to JSON in the intended use case. gRPC already uses protobuf serializers/deserializers generated into the stub path.

## 32.2 Measure `ByteSize()`

```python
if msg.ByteSize() > MAX_LOGICAL_MESSAGE:
    raise ValueError("message too large")
```

Use application-specific caps before the gRPC transport limit when message size has semantic/resource significance.

## 32.3 Avoid needless intermediate dict/JSON

Bad hot path:

```text
protobuf -> dict -> orjson -> bytes -> dict -> protobuf
```

If both sides speak protobuf, serialize protobuf directly.

## 32.4 Reuse schema/classes, not mutable messages indiscriminately

Generated classes/descriptors are reusable. Mutable message instances should generally be request/operation scoped unless an explicit object pool is proven beneficial and correctly reset.

## 32.5 Packed repeated numeric fields

Modern protobuf can encode repeated numeric primitive fields in packed length-delimited form, improving size. Let schema/runtime defaults/features manage this unless compatibility requires a particular setting.

---

# 33) Memory behavior and object reuse

## 33.1 Clear may retain capacity

Official Python generated-code docs note that clearing a field may reset logical size without returning all capacity immediately. For a repeatedly reused message that once held a huge repeated field, memory can remain elevated.

A compaction technique is to copy the logically cleared message into a fresh instance:

```python
msg.ClearField("big_field")
compact = type(msg)()
compact.CopyFrom(msg)
msg = compact
```

Use only when profiling shows retained capacity matters.

## 33.2 Avoid long-lived giant messages

For streams/ingestion, process chunks and release message references promptly instead of accumulating a list of every request message.

## 33.3 Submessage references

Submessages are owned by parents. Retaining a child reference can keep the parent/storage graph alive. Copy if you need detached long-term state.

---

# 34) Threading/concurrency and mutation rules

Treat **message mutation as single-owner** unless you have strong guarantees. Even if independent reads are safe in your runtime, concurrently mutating the same protobuf object is not a good application contract.

Recommended:

```text
parse/build message in one request/task
 -> convert to immutable/domain representation if shared
 -> or CopyFrom into independent message per owner
```

Generated descriptors/classes can be shared widely; mutable instances should not be used as global scratch buffers across gRPC workers.

For AsyncIO, never mutate a request message from a background task after the handler logically hands ownership elsewhere without an explicit copy/ownership design.

---

# 35) Validation limitations and semantic invariants

Protobuf validates structural types/ranges, but not rich domain constraints.

Schema:

```proto
message Range {
  int64 start = 1;
  int64 end = 2;
}
```

Protobuf will not enforce `end >= start`.

Recommended boundary:

```python
def range_from_proto(msg: api_pb2.Range) -> Range:
    if msg.end < msg.start:
        raise InvalidArgument("end must be >= start")
    return Range(start=msg.start, end=msg.end)
```

Pydantic can be useful in internal JSON/config boundaries, but do not automatically convert every protobuf to Pydantic simply to recover validation. A focused domain constructor can be cheaper and clearer.

---

# 36) Security and untrusted payloads

## 36.1 Binary protobuf is not “safe because typed”

Untrusted payloads can still cause resource pressure through:

- deeply nested messages;
- large length-delimited fields;
- huge repeated collections;
- expensive semantic processing after parse;
- maliciously selected dynamic `Any` types.

## 36.2 Apply transport and application limits

In gRPC, set receive/send limits and request deadlines. In direct parsing, cap input size before parsing where feasible.

## 36.3 Do not dynamically dispatch arbitrary descriptors/types

Reflection + `Any` + dynamic message factory is powerful. If untrusted callers can choose arbitrary type names and trigger arbitrary handlers, you have created a dynamic capability surface. Use allowlists.

## 36.4 Error handling

Catch `DecodeError` at untrusted binary boundaries and map it to a controlled failure rather than allowing raw exceptions/bytes into logs.

---

# 37) Testing contracts and compatibility

## 37.1 Golden binary fixtures

Keep representative old-version serialized fixtures and assert the current generated runtime can parse them with expected semantics.

## 37.2 Forward-unknown fixture

Generate a message with a future/extra field using a newer schema; parse/reserialize with the older schema; verify the newer reader recovers the unknown field if that preservation path matters.

## 37.3 JSON fixtures separately

Test canonical JSON field names/enums/64-bit mappings separately. Binary compatibility does not prove JSON compatibility.

## 37.4 Descriptor contract

Assert critical field numbers/full names:

```python
assert graph_pb2.Node.DESCRIPTOR.fields_by_name["id"].number == 1
assert graph_pb2.Node.DESCRIPTOR.full_name == "codefabric.v1.Node"
```

This catches accidental renumbering/package changes immediately.

## 37.5 Regeneration test

Regenerate generated files in CI and fail on diff.

## 37.6 Cross-version test matrix

For a public service, test new client↔old server and old client↔new server for representative additive changes, not only “latest/latest.”

---

# 38) gRPC integration

Generated `_pb2_grpc.py` binds protobuf serializers/deserializers to gRPC methods. A unary stub conceptually uses:

```python
channel.unary_unary(
    "/codefabric.v1.GraphService/GetNode",
    request_serializer=GetNodeRequest.SerializeToString,
    response_deserializer=GetNodeResponse.FromString,
)
```

Therefore:

- protobuf parse/serialize cost is inside RPC latency;
- field presence/unknown fields directly influence gRPC contract behavior;
- gRPC status/metadata are outside protobuf response schema unless modeled explicitly;
- request/response message evolution should follow protobuf compatibility rules;
- RPC method cardinality/name changes are additional gRPC-level compatibility concerns.

For rich errors, use `google.rpc.Status` detail messages rather than inventing a parallel JSON blob.

---

# 39) Pydantic/orjson/FastMCP integration boundaries

## 39.1 Pydantic

Use Pydantic where Python-facing data enters through JSON/config/tool schemas. Use protobuf where RPC/wire contracts require generated cross-language schema.

A clean architecture is:

```text
FastMCP/Pydantic request model
 -> domain request
 -> protobuf request
 -> gRPC daemon
 -> protobuf response
 -> domain result
 -> FastMCP/Pydantic/JSON output
```

Do not make generated protobuf messages the public FastMCP tool schema merely because both are typed; FastMCP/Pydantic schema ergonomics and protobuf wire semantics are different.

## 39.2 orjson

Do not use `orjson.dumps(msg)` as a protobuf JSON serializer. Use `json_format.MessageToDict/MessageToJson` because ProtoJSON has specific mappings.

If performance demands JSON bytes:

```python
proto_dict = json_format.MessageToDict(msg)
json_bytes = orjson.dumps(proto_dict)
```

But understand that this is **ProtoJSON-derived Python data serialized by orjson**, and test that the resulting JSON exactly preserves required protobuf mapping semantics/options. Prefer `MessageToJson` unless you have a measured reason to split the pipeline.

## 39.3 Opaque JSON payloads

If protobuf has a `bytes json_payload = ...`, document the payload's JSON schema/version explicitly. Otherwise protobuf only knows “opaque bytes,” and schema evolution has moved into an undocumented secondary protocol.

---

# 40) Editions 2023/2024/2026 and feature lifecycle

## 40.1 Editions model

Editions decouple protobuf language evolution from `syntax = "protoN"` generations. The file declares an edition and features can have edition-specific defaults/lifecycles.

## 40.2 Edition 2023

Edition 2023 established the modern Editions model and explicit-presence defaults.

## 40.3 Later Editions

The protobuf project continues adding feature changes through later Editions. As of the 36.x/7.36 release period, Edition 2026 work includes language/style/default visibility and JSON enum customization capabilities in the compiler/repository release stream.

Because the Python 7.36.0 runtime was released on the date of this reference while some official version-support pages still show 35.x examples, **verify the exact `protoc 36.x` artifact and feature support before enabling Edition 2026-only syntax in production schemas**.

## 40.4 Do not mix runtime release with syntax availability casually

A Python runtime can parse descriptors generated for features it supports, but schema authoring availability is primarily a compiler/toolchain concern. Pin/test compiler, plugins, and runtime together.

---

# 41) Upgrade/migration guidance for 7.36

## 41.1 Stable runtime target

Use:

```text
protobuf==7.36.0
Python >=3.10
```

and record the matching `protoc`/gRPC toolchain used to produce generated files.

## 41.2 From 6.x to 7.x

Treat major runtime changes as a compatibility event. Run:

```text
import/generated-code tests
binary fixture round trips
JSON fixture tests
descriptor/reflection tests
gRPC integration tests
Any/custom options tests
type-checking
performance/memory baseline
```

## 41.3 Descriptor API deprecations

Recent release work continues tightening/deprecating older descriptor mutation/comparison APIs. Reflection-heavy code is more upgrade-sensitive than ordinary generated-message code; isolate it and target public current properties.

## 41.4 Python 3.15/toolchain

The surrounding 2026 Python/gRPC/protobuf release line includes Python 3.15 support work. Do not infer interpreter support only from another package; use PyPI metadata for each pinned package.

## 41.5 Regenerate or not?

Python's long gencode compatibility window means a runtime upgrade does not necessarily require regenerating every file. However, if you want new compiler features, updated generated typing, Editions support, or gRPC plugin changes, regenerate intentionally and diff/test.

---

# 42) Anti-pattern inventory

- Reusing a removed field number.
- Renumbering fields to make a `.proto` “look cleaner.”
- Assuming field rename is universally compatible because binary protobuf ignores source names.
- Using implicit scalar presence for patch semantics that need “unset vs default.”
- Calling `HasField()` on implicit proto3 scalars.
- Directly assigning a submessage instead of using `CopyFrom`/child mutation.
- Treating repeated/map containers as ordinary detached `list`/`dict` objects.
- Reading a missing message-map key without realizing it may create an entry.
- Using map iteration order as a stable business order.
- Making enum zero value an active dangerous state.
- Comparing serialized bytes for message semantic equality.
- Treating deterministic serialization as canonical cross-version bytes.
- Passing protobuf through JSON and assuming unknown fields survive.
- Using `json.dumps`/orjson directly on message internals instead of ProtoJSON mapping.
- Using TextFormat as a durable public wire format.
- Overusing `Any`/`Struct` until the schema becomes effectively dynamic.
- Dynamic type dispatch from untrusted `Any` type URLs without allowlists.
- Mutating one message instance across threads/tasks.
- Expecting protobuf to enforce cross-field domain invariants.
- Loading giant untrusted payloads without transport/application limits.
- Hand-editing generated `_pb2.py`.
- Conflating Python runtime 7.36.0 with compiler version 7.36.0; compiler release line is 36.x.
- Assuming “wire safe” implies ProtoJSON safe.

---

# 43) Dense API/reference matrices

## 43.1 Message operations

| Need | API |
|---|---|
| serialize | `msg.SerializeToString(deterministic=...)` |
| serialize partial | `msg.SerializePartialToString(...)` |
| parse replacing existing | `msg.ParseFromString(payload)` |
| parse merge | `msg.MergeFromString(payload)` |
| copy message | `dst.CopyFrom(src)` |
| merge message | `dst.MergeFrom(src)` |
| encoded size | `msg.ByteSize()` |
| clear all | `msg.Clear()` |
| clear one field/oneof | `msg.ClearField(name)` |
| check explicit presence | `msg.HasField(name)` |
| active oneof member | `msg.WhichOneof(group)` |
| present field reflection | `msg.ListFields()` |
| drop future fields | `msg.DiscardUnknownFields()` |

## 43.2 Container operations

| Shape | Common operations |
|---|---|
| repeated scalar | append, extend, slice assign, del |
| repeated message | add, append/copy semantics, iteration |
| scalar map | `m[k]=v`, update, del |
| message map | `get_or_create(k)` when creation intended |

## 43.3 Presence decision matrix

| Need | Modeling choice |
|---|---|
| distinguish absent vs scalar default | Edition explicit presence / proto3 `optional` |
| exactly one of variants | `oneof` |
| update selected paths | `FieldMask` + message |
| arbitrary optional dynamic fields | carefully consider `Struct`/map/Any; avoid by default |

## 43.4 Serialization-format matrix

| Format | Schema-aware | Unknown preservation | Human-readable | Stability posture |
|---|---|---|---|---|
| binary protobuf | yes | strong | no | strongest wire compatibility |
| ProtoJSON | yes, mapped | no binary-style preservation | yes | separate JSON compatibility rules |
| TextFormat | yes | not public-wire equivalent | yes | debug/config, not durable wire default |
| arbitrary orjson of custom dict | only your mapping | only your mapping | yes | entirely application-defined |

## 43.5 Compatibility matrix

| Change | Binary tendency | ProtoJSON tendency |
|---|---|---|
| add fresh optional field | safe | generally okay for tolerant/new readers, unknown handling differs |
| rename field, same number | binary-safe | JSON-breaking name change |
| remove field and reserve | binary-compatible with caveats | old JSON producers may break new parser |
| reuse number | dangerous/invalid evolution | dangerous |
| rename enum value same number | binary-safe-ish | JSON-breaking enum name |
| add enum numeric value | forward-compatible when unknown values handled | clients must tolerate new names/values |
| change package/full message name | wire field bytes may parse, but type/service/Any identities can break | API/schema identity break |

## 43.6 Toolchain matrix

| Component | Stable target in this reference |
|---|---|
| Python runtime package | `protobuf==7.36.0` |
| Python version floor | >=3.10 |
| compiler/repository release family | 36.x |
| gRPC Python runtime | `grpcio==1.83.0` |
| gRPC codegen wrapper/plugin | `grpcio-tools==1.83.0` |

---

# 44) Agent schema-authoring checklist

```text
VERSION / TOOLCHAIN
[ ] Pin protobuf Python runtime 7.36.0.
[ ] Record protoc 36.x compiler version used.
[ ] Record grpcio-tools version when generating gRPC code.
[ ] Generate .pyi files for static typing.
[ ] Regenerate in CI and fail on drift.

SCHEMA DESIGN
[ ] Never change/reuse published field numbers.
[ ] Reserve deleted field names and numbers.
[ ] Use explicit presence where absent vs default matters.
[ ] Use oneof for mutually exclusive variants.
[ ] Use semantically safe enum zero values.
[ ] Prefer typed fields over Struct/Any when schema is known.
[ ] Add package/version namespace deliberately.

COMPATIBILITY
[ ] Evaluate binary compatibility.
[ ] Evaluate ProtoJSON compatibility separately.
[ ] Keep old/new fixture round-trip tests.
[ ] Test old client/new server and new client/old server where relevant.
[ ] Preserve unknown fields when forward compatibility requires it.

PYTHON
[ ] Do not subclass generated messages.
[ ] Do not hand-edit generated files.
[ ] Do not concurrently mutate one message across tasks/threads.
[ ] Copy messages/submessages when ownership must be independent.
[ ] Use descriptors/message_factory only for genuinely dynamic tooling.

RPC / FASTMCP
[ ] Keep protobuf at the gRPC boundary.
[ ] Convert to domain models in one adapter layer.
[ ] Use google.rpc.Status/details for typed RPC failures if needed.
[ ] Do not use orjson directly as a protobuf JSON mapping.
[ ] Keep FastMCP/Pydantic schemas separate from wire schemas unless intentionally unified.

SECURITY / PERFORMANCE
[ ] Bound payload/message sizes.
[ ] Avoid giant monolithic repeated messages when streaming/chunking fits.
[ ] Allowlist Any/dynamic type dispatch.
[ ] Measure ByteSize and protobuf encode/decode on hot paths.
[ ] Do not assume Clear() immediately returns retained capacity.
```

---

# Source index

[PB-PYPI]: https://pypi.org/project/protobuf/
[PB-VERSION]: https://protobuf.dev/support/version-support/
[PB-CROSS]: https://protobuf.dev/support/cross-version-runtime-guarantee/
[PB-PY-GEN]: https://protobuf.dev/reference/python/python-generated/
[PB-PY-TUTORIAL]: https://protobuf.dev/getting-started/pythontutorial/
[PB-PRESENCE]: https://protobuf.dev/programming-guides/field_presence/
[PB-JSON]: https://protobuf.dev/programming-guides/json/
[PB-PROTO3]: https://protobuf.dev/programming-guides/proto3/
[PB-ENCODING]: https://protobuf.dev/programming-guides/encoding/
[PB-EDITIONS]: https://protobuf.dev/editions/overview/
[PB-RELEASES]: https://github.com/protocolbuffers/protobuf/releases

