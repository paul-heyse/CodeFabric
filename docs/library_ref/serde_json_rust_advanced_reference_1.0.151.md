# `serde_json` in Rust — advanced technical reference / parsing, token-preservation, and Serde JSON catalog for LLM coding agents

This reference targets `serde_json = 1.0.151` and emphasizes the features that matter to a strict cross-language canonicalization pipeline while still cataloging the broader library surface.

## Version / source anchors

**Release anchor:** `serde_json = 1.0.151` (released 2026-07-20). **MSRV:** Rust 1.71. **CodeFabric feature requirement:** `arbitrary_precision` is enabled so validation can inspect JSON numeric tokens without first collapsing them to ordinary machine-number representations.

Primary anchors:

- https://docs.rs/serde_json/1.0.151/serde_json/
- https://docs.rs/crate/serde_json/1.0.151/features
- https://docs.rs/crate/serde_json/1.0.151/source/Cargo.toml.orig
- https://github.com/serde-rs/json/tree/v1.0.151
- https://serde.rs/

## Capability inventory

`serde_json` provides four overlapping JSON layers:

1. direct typed deserialization/serialization through Serde;
2. dynamic `Value`/`Map`/`Number` manipulation;
3. streaming/deserializer APIs for custom visitors, token-aware validation, and multiple values;
4. optional raw and representation-preserving facilities controlled by Cargo features.

Core public surfaces include `from_str`, `from_slice`, `from_reader`, `to_string`, `to_vec`, `to_writer`, pretty variants, `to_value`/`from_value`, `Deserializer`, `Serializer`, `StreamDeserializer`, `Value`, `Map`, `Number`, `Error`, `json!`, and optional `RawValue`.

## Cargo features in 1.0.151

| Feature | Default? | Meaning | CodeFabric posture |
|---|---:|---|---|
| `std` | yes | standard-library I/O and normal runtime support | normal server/CLI default |
| `alloc` | no | heap-backed JSON without full `std` | only for no-std+alloc targets |
| `arbitrary_precision` | no | stores `Number` representation so arbitrary-size/precision JSON numbers can round-trip as `Number` | **required at strict ingress** |
| `float_roundtrip` | no | more expensive float parsing to preserve `f64 -> JSON -> f64` | canonicalizer brings its own use; not a substitute for token validation |
| `preserve_order` | no | backs `Map` with `IndexMap` instead of default sorted map | not needed for JCS ordering |
| `raw_value` | no | enables `RawValue` to borrow/retain an unprocessed JSON value | useful in specialized token-preserving paths |
| `unbounded_depth` | no | allows disabling the built-in recursion limit | avoid for untrusted input without another stack defense |
| `indexmap` | no | internal optional dependency | normally reached via `preserve_order` |

Recommended CodeFabric dependency shape:

```toml
serde_json = { version = "=1.0.151", features = ["arbitrary_precision"] }
```

Enable `raw_value` only where a concrete design requires verbatim sub-value retention.

## Typed JSON APIs

### Deserialize from in-memory text or bytes

```rust
let model: Model = serde_json::from_str(text)?;
let model: Model = serde_json::from_slice(bytes)?;
```

`from_str`/`from_slice` can borrow from the input when target lifetimes permit. They are preferable to `from_reader` when zero-copy borrowed strings or borrowed `RawValue` are desired.

### Deserialize from a reader

```rust
let model: Model = serde_json::from_reader(reader)?;
```

`Deserializer::from_reader` does not itself buffer file I/O and reader-based deserialization cannot borrow string slices from the source in the same way as `from_str`/`from_slice`. Wrap files/sockets in an appropriate buffered reader when needed.

### Serialize

```rust
let text = serde_json::to_string(&model)?;
let bytes = serde_json::to_vec(&model)?;
serde_json::to_writer(&mut writer, &model)?;
```

Pretty variants exist for human-facing output. **None of these functions implement RFC 8785.** Use `serde_json_canonicalizer` for fingerprint bytes.

## `Value`, `Map`, and `json!`

`Value` models `Null`, `Bool`, `Number`, `String`, `Array(Vec<Value>)`, and `Object(Map<String, Value>)`.

```rust
use serde_json::{json, Value};
let v = json!({"name": "codefabric", "enabled": true});
```

By default `Map` uses a sorted map representation; `preserve_order` changes it to insertion-preserving `IndexMap`. Neither representation should be treated as the JCS ordering algorithm for all Unicode keys.

Indexing with `value["key"]` is ergonomic but can hide absence by yielding `Value::Null` in some indexing contexts. Validation code should prefer `get`, `get_mut`, and explicit type accessors.

## `Number` and the `arbitrary_precision` feature

Without `arbitrary_precision`, a JSON number is represented internally in the normal signed integer / unsigned integer / finite-float domains. With `arbitrary_precision`, the internal representation retains the numeric text in string form sufficiently for JSON→`Number`→JSON round-trip.

Useful inspection methods include:

- `is_i64`, `is_u64`, `is_f64`;
- `as_i64`, `as_u64`, `as_f64`;
- `from_f64` (returns `None` for NaN/infinity);
- `Display` to recover the represented JSON number spelling under `arbitrary_precision`.

### CodeFabric use

The feature exists in this pipeline so validation can reject integer tokens outside `[-9007199254740991, 9007199254740991]` **before** JCS output converts numeric values to the interoperable double domain. Do not use `as_f64` as the first validation operation because it may erase exactly the distinction that needs to be rejected.

A robust validator should distinguish lexical integer tokens from lexical floating/exponent tokens according to the contract, then enforce the appropriate domain rules.

## Custom `Deserializer` and Visitor path

For protocol ingress where duplicate member names are forbidden, ordinary `Value` parsing is insufficient because a map representation cannot prove whether the source contained a duplicate. Use a custom `DeserializeSeed`/`Visitor`/`MapAccess` path (or an equivalent strict pre-parser) that records names as they arrive and rejects repeats.

Conceptual pattern:

```rust
struct StrictObjectVisitor;

impl<'de> serde::de::Visitor<'de> for StrictObjectVisitor {
    type Value = /* your representation */;
    // in visit_map:
    // 1. read key in source order
    // 2. reject if already seen
    // 3. parse value with number-aware representation
}
```

Do not deserialize first into `serde_json::Value` and attempt to detect duplicates afterward.

## Streaming and multiple values

`Deserializer` can be converted into an iterator over sequential JSON values with `into_iter::<T>()`, yielding `StreamDeserializer`. This is useful for JSON streams/NDJSON-like inputs when framing is handled by the caller. A canonicalization contract should still define whether multiple values are allowed; JCS itself canonicalizes one JSON value at a time.

## Recursion limits

The deserializer maintains a recursion-depth guard (128 levels in the implementation). The `unbounded_depth` feature exposes `disable_recursion_limit`; the crate documentation warns that callers then need an alternate stack-overflow defense such as `serde_stacker`, and later operations like `Debug`, `Display`, or `Drop` may still recurse.

For untrusted CodeFabric registry/input data, keep bounded depth unless a reviewed resource policy replaces it.

## `RawValue` (`raw_value` feature)

`RawValue` refers to the exact byte/text range of one valid JSON value and can defer parsing or splice the value verbatim into another serialization. Borrowed `&RawValue` works with `from_str`/`from_slice`; reader-based decoding generally needs owned `Box<RawValue>`.

This is useful when:

- preserving original number spelling for a specialized validation path;
- forwarding opaque JSON without parse/re-serialize costs;
- deferring parsing of large subtrees.

It is dangerous in canonicalization if verbatim output accidentally bypasses JCS normalization. Never embed arbitrary `RawValue` directly into fingerprint serialization unless its semantics are explicitly canonicalized by the outer serializer and tested.

## Serialization model and Serde annotations

Serde attributes are wire-format policy:

- `rename` / `rename_all` changes object member names;
- `skip_serializing_if` changes member presence;
- `default` affects deserialization semantics;
- `flatten` changes object shape and can introduce name collisions;
- tagged/untagged enum strategies change representation;
- custom `serialize_with` / `deserialize_with` can bypass normal type semantics.

Canonical bytes are deterministic only after the application representation is deterministic. Review changes to Serde annotations as protocol changes.

## Error model

`serde_json::Error` carries line/column information where available and can be classified into I/O, syntax, data, or EOF categories. Typical application practice is to attach path/context while preserving the source error.

For validation services, separate:

- malformed JSON syntax;
- duplicate-key contract failure;
- number-domain contract failure;
- schema/format failure;
- canonical serialization failure;
- downstream I/O failure.

Do not make clients reverse-engineer all failures from one generic “invalid JSON” string.

## JSON compliance details relevant to strict systems

`serde_json` is intentionally a general JSON library, not the CodeFabric canonical contract. Key concerns:

- duplicate member policy must be supplied by the target/custom visitor;
- JSON object order is semantically separate from canonical JCS order;
- non-finite Rust floats are not representable as valid `serde_json::Number` (`Number::from_f64` rejects them);
- integer/float representability depends on enabled features and target type;
- deep or enormous inputs are a resource-exhaustion concern independent of syntactic validity.

## Performance guidance

- Prefer typed deserialization when schema is known; it avoids a dynamic `Value` pass.
- Prefer `from_slice`/`from_str` when input is already resident and borrowing matters.
- Buffer file/network readers externally.
- Avoid `Value -> String -> Value` round trips as a validation technique.
- `float_roundtrip` intentionally costs more float-parse CPU; enable only when required.
- `arbitrary_precision` changes number representation and is justified here by protocol validation, not throughput.
- Do not enable `preserve_order` solely for JCS; it adds an `IndexMap` dependency and does not implement UTF-16 JCS sorting.

## Security and robustness

Treat input size, nesting, string length, array length, and object-member count as separate limits. Parser correctness does not prevent memory exhaustion. When duplicate detection uses a `HashSet`, bound object size and consider adversarial hashing only through standard hardened collections.

Never call `disable_recursion_limit` on untrusted data without a reviewed stack strategy.

## Canonicalization-specific anti-patterns

- `serde_json::to_vec(&value)` as the fingerprint serializer.
- `Value` parsing before duplicate detection.
- `as_f64()` before safe-integer/token validation.
- assuming `BTreeMap` sorting equals RFC 8785.
- `preserve_order` as a canonicalization feature.
- pretty JSON or whitespace stripping as “canonical JSON”.
- normalizing Unicode to make two strings “equivalent”.
- relying on the lexical spelling of a number after converting it to a Rust numeric primitive.

## Agent checklist

- [ ] exact version and `arbitrary_precision` feature remain pinned;
- [ ] duplicate names are rejected before map materialization;
- [ ] numeric token validation occurs before JCS conversion;
- [ ] resource limits are explicit;
- [ ] `serde_json` serialization is used for ordinary JSON only, not fingerprint bytes;
- [ ] Serde annotation changes are covered by canonical fixtures;
- [ ] `RawValue` or recursion-limit features are enabled only for an explicit reviewed need.
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

## 1) Data model, ownership, and where `serde_json` sits in a protocol stack

`serde_json` is a Serde data-format implementation, not merely a pair of `parse` / `stringify` helpers. Coding agents should distinguish four layers:

| Layer | Principal API | Preserves source spelling? | Best use |
|---|---|---:|---|
| typed | `from_*::<T>`, `to_*(&T)` | generally no | validated application models |
| dynamic | `Value`, `Map`, `Number` | partly, feature-dependent | generic JSON manipulation |
| streaming | `Deserializer`, `StreamDeserializer`, `Serializer` | depends on target | large/sequential inputs, custom visitors |
| raw | `RawValue` with `raw_value` | yes, for one value slice | deferred/verbatim sub-values |

For `codefabric-jcs-v1`, the recommended architecture is deliberately asymmetric: use the **deserialization side** of `serde_json` for strict ingress and token-aware validation, then use `serde_json_canonicalizer` for protocol bytes. Normal `serde_json` serialization remains useful for logs, APIs, fixtures, and non-fingerprint JSON.

### Planning rule

```text
Need ordinary typed JSON?            -> from_* / to_*
Need unknown JSON shape?             -> Value
Need duplicate-aware object ingress? -> custom Visitor / MapAccess
Need multiple sequential values?     -> Deserializer::into_iter
Need verbatim sub-value?              -> RawValue + raw_value
Need RFC 8785 bytes?                  -> serde_json_canonicalizer, not Serializer
```

## 2) Deserialization entry points and lifetime behavior

### `from_str`

```rust
pub fn from_str<'a, T>(s: &'a str) -> Result<T>
where
    T: serde::de::Deserialize<'a>;
```

Use when input is validated UTF-8 text already resident in memory. Borrowing targets such as `&str` or `Cow<'a, str>` may borrow from the source where the JSON representation permits it.

### `from_slice`

```rust
pub fn from_slice<'a, T>(v: &'a [u8]) -> Result<T>
where
    T: serde::de::Deserialize<'a>;
```

Use when the network/file layer owns bytes. It avoids an application-owned intermediate `String` merely to invoke the parser and can support borrowing from the slice.

### `from_reader`

```rust
pub fn from_reader<R, T>(rdr: R) -> Result<T>
where
    R: std::io::Read,
    T: serde::de::DeserializeOwned;
```

The owned target bound is important: a reader is consumed incrementally, so returned values cannot borrow arbitrary source slices. The JSON deserializer is not a replacement for I/O buffering; wrap small-read sources in `BufReader` where appropriate.

### `from_value`

```rust
pub fn from_value<T>(value: Value) -> Result<T>
where
    T: serde::de::DeserializeOwned;
```

This is useful after dynamic transformation, but it is **too late** for source-level duplicate detection. Once an object is a `Value::Object`, duplicate member provenance is gone.

## 3) `Deserializer`: custom ingress and parser control

Constructors mirror the convenience functions:

```rust
let mut de = serde_json::Deserializer::from_str(input);
let mut de = serde_json::Deserializer::from_slice(bytes);
let mut de = serde_json::Deserializer::from_reader(reader);
```

Deserialize explicitly when you need custom seeds/visitors or post-parse framing checks:

```rust
use serde::Deserialize;

let mut de = serde_json::Deserializer::from_str(input);
let value = Model::deserialize(&mut de)?;
de.end()?; // reject trailing non-whitespace input
```

### Why `end()` matters

A protocol endpoint that intends to accept exactly one JSON value should verify that the deserializer reached the end after trailing whitespace. Otherwise an integration can accidentally validate a prefix and ignore unwanted trailing input.

### Recursion control

The normal deserializer keeps its built-in recursion limit. `unbounded_depth` exists for applications that consciously replace this defense. Do not enable the feature and call `disable_recursion_limit()` just to accommodate an accidental deeply nested fixture. Deep nesting is an input-complexity dimension with stack implications across parsing, traversal, validation, display, and drop.

## 4) `StreamDeserializer`: sequential JSON values

`Deserializer::into_iter::<T>()` creates a `StreamDeserializer`, useful for streams such as:

```text
{"id":1}\n
{"id":2}\n
{"id":3}\n
```

or concatenated JSON values where framing is otherwise defined.

Operational rules:

- the iterator yields `Result<T>` one value at a time;
- parsing state and byte offsets belong to the stream, not a fresh parser per record;
- a malformed record terminates normal progression unless the application has a documented resynchronization strategy;
- NDJSON line framing and generic sequential-JSON parsing are related but not identical contracts;
- canonicalization still happens per accepted logical JSON value.

Do not use stream parsing as an implicit way to accept multiple top-level values in an endpoint whose contract says “one JSON document.”

## 5) `Value`: complete dynamic navigation mental model

`Value` variants are:

```rust
Null
Bool(bool)
Number(Number)
String(String)
Array(Vec<Value>)
Object(Map<String, Value>)
```

### Type predicates and typed accessors

Use paired predicates/accessors when writing generic validators:

| Domain | Predicate | Accessor |
|---|---|---|
| null | `is_null` | `as_null` |
| boolean | `is_boolean` | `as_bool` |
| number | `is_number` | `as_number` |
| integer-like | `is_i64`, `is_u64` | `as_i64`, `as_u64` |
| float-representable | `is_f64` | `as_f64` |
| string | `is_string` | `as_str` |
| array | `is_array` | `as_array`, `as_array_mut` |
| object | `is_object` | `as_object`, `as_object_mut` |

For validation, prefer these APIs to pattern-insensitive indexing because an absent member must not be confused with an explicit JSON `null`.

### `get` / `get_mut`

`Value::get` and `get_mut` support object keys and array indices through the indexing abstraction. They return `Option` and therefore make absence explicit.

```rust
let name = value.get("name").and_then(Value::as_str);
```

### JSON Pointer

`pointer` and `pointer_mut` implement JSON Pointer-style navigation. `/` separates tokens, `~1` escapes slash, and `~0` escapes tilde.

```rust
if let Some(v) = value.pointer("/registry/items/0/id") {
    // ...
}
```

Use pointers for generic diagnostics/config lookups; for hot typed paths, normal structs are clearer and safer.

### Moving values with `take`

`Value::take` replaces the source with `Null` and returns the old value. This avoids cloning a subtree when restructuring a dynamic document. Treat the mutation semantics as explicit—after `take`, the old location is no longer unchanged input.

## 6) `Map<String, Value>` and object-order policy

The default map implementation is sorted; with `preserve_order` it uses `IndexMap`. This is an in-memory implementation detail, not a canonical JSON policy.

Useful map operations follow ordinary map idioms: lookup, mutable lookup, contains/remove, iteration, entry insertion, length/capacity where supported by the backing representation, and conversion/collection through Serde.

### Canonicalization warning

RFC 8785 compares property names by UTF-16 code units. A Rust map's own ordering is not a substitute. This remains true even if test fixtures containing only ASCII keys happen to match canonical output.

### Duplicate-name warning

A map cannot represent the statement “the source had the same member name twice.” If that statement must be rejected, enforce it while consuming `MapAccess`.

## 7) `Number`: representability, inspection, and lexical provenance

JSON has one syntactic “number” category; Rust has many numeric types. `serde_json::Number` mediates this mismatch.

Without `arbitrary_precision`, the usual internal domains are signed 64-bit, unsigned 64-bit, or finite floating point. With `arbitrary_precision`, the crate can preserve arbitrary JSON number spellings through the `Number` representation sufficiently for exact JSON `Number` round-tripping.

### Important methods

- `is_i64`, `is_u64`, `is_f64` — domain predicates;
- `as_i64`, `as_u64`, `as_f64` — conversion if representable;
- `from_f64` — returns `None` for NaN or infinity;
- formatting / serialization — emits a JSON number representation.

### `codefabric-jcs-v1` rule

Do not treat `arbitrary_precision` as permission to canonicalize arbitrarily large integers. Its job here is to **delay information loss long enough to reject them** according to the safe-integer contract. Validation order is therefore semantic:

```text
JSON numeric token
  -> retain enough representation to classify token
  -> reject invalid / unsafe numeric domain
  -> only then convert into the canonicalizable JSON data model
```

A successful `as_f64()` call does not mean the original source token was an allowed integer token.

## 8) `RawValue`: exact sub-document retention

With feature `raw_value`, `RawValue` represents one syntactically valid JSON value in its original JSON spelling.

Typical uses:

- defer parsing of a large/opaque extension field;
- retain number or whitespace spelling for diagnostics;
- forward a validated opaque JSON payload;
- parse an envelope without materializing every nested value.

### Borrowed vs owned

In-memory parsing can deserialize borrowed `&RawValue` from source text/slices. Reader-based parsing generally needs owned `Box<RawValue>` because source storage does not outlive the parse in borrowable form.

### Canonicalization safety

Raw JSON spelling is intentionally *noncanonical*. A `RawValue` retained for diagnostics must not be spliced directly into fingerprint bytes unless the serialization path proves it passes through canonical transformation. Keep “verbatim source” and “canonical protocol output” as separate types/variables.

## 9) Serialization entry points

The top-level serializer functions are convenience wrappers:

```rust
let compact = serde_json::to_string(&value)?;
let bytes = serde_json::to_vec(&value)?;
serde_json::to_writer(&mut writer, &value)?;

let pretty = serde_json::to_string_pretty(&value)?;
let pretty_bytes = serde_json::to_vec_pretty(&value)?;
serde_json::to_writer_pretty(&mut writer, &value)?;
```

`to_value` serializes a Serde value into the dynamic JSON data model instead of text.

### Protocol boundary

Compact JSON is not canonical JSON. Pretty JSON is not canonical JSON. A sorted Rust map serialized compactly is still not a general JCS implementation.

## 10) `Serializer` and `Formatter`

`Serializer<W>` writes compact JSON to an `io::Write`; `Serializer::pretty` uses the pretty formatter; `Serializer::with_formatter` accepts a formatter implementation.

```rust
let mut ser = serde_json::Serializer::new(writer);
model.serialize(&mut ser)?;
```

Custom formatters are appropriate for presentation/protocol formats explicitly defined in terms of `serde_json` formatting hooks. They are a poor place to recreate JCS: canonical object sorting requires semantic handling beyond merely changing whitespace or token punctuation.

`into_inner()` returns the underlying writer after serialization.

## 11) `json!` macro

`json!` is excellent for tests, small dynamic payloads, and examples:

```rust
let fixture = serde_json::json!({
    "id": "abc",
    "enabled": true,
    "values": [1, 2, 3],
});
```

Agent rules:

- do not migrate large typed protocol models to `json!` merely because it is concise;
- values embedded in the macro still serialize according to Serde semantics;
- dynamic creation does not add duplicate-key detection or schema validation;
- compile-time-looking syntax does not mean runtime protocol validity.

## 12) Error model and diagnostics

`serde_json::Error` is used by both deserialization and serialization. Preserve:

- human-readable source error;
- line and column when available;
- error category (`Io`, `Syntax`, `Data`, `Eof`) where application routing benefits;
- a semantic path when validation occurs after parsing.

For deeply nested typed models, consider adding a path-aware layer in application code rather than replacing the underlying error text. Stable machine-facing validation error codes should be application-owned.

### Failure taxonomy for CodeFabric

```text
JSON_SYNTAX
JSON_DUPLICATE_KEY
JSON_NUMBER_DOMAIN
SCHEMA_SHAPE
SCHEMA_FORMAT
CANONICAL_SERIALIZE
CHECKSUM_FRAMING
IO
```

Do not collapse all of these into a single `serde_json::Error` contract just because parsing begins with `serde_json`.

## 13) Strict duplicate-key detection with a Visitor

The invariant must be checked as source members are observed:

```rust
use serde::de::{MapAccess, Visitor};
use std::collections::HashSet;

// Sketch only: application result type omitted for clarity.
// In visit_map:
// while let Some(key) = map.next_key::<String>()? {
//     if !seen.insert(key.clone()) {
//         return Err(serde::de::Error::custom("duplicate JSON member"));
//     }
//     let value = map.next_value::<...>()?;
// }
```

The production implementation should preserve a structured path and duplicate member name so diagnostics can identify the offending object.

### Why `flatten` deserves review

Serde `flatten` changes how map keys are associated with struct fields. If strict source-object duplicate semantics matter, verify behavior at the raw object boundary rather than assuming derives will expose every collision the way the protocol needs.

## 14) Serde derive policy as wire-schema policy

Attributes that change JSON representation include:

- `rename`, `rename_all`;
- `skip`, `skip_serializing`, `skip_deserializing`, `skip_serializing_if`;
- `default`;
- `flatten`;
- enum tagging strategies (`tag`, `content`, `untagged`);
- custom `serialize_with`, `deserialize_with`, module `with`;
- field-level adapters for formats such as `codefabric-bytes`.

A code review that changes these on a fingerprinted type should be reviewed like a protocol/schema change, even if the Rust field types are unchanged.

## 15) Object keys and non-string maps

JSON object member names are strings. Serde can serialize certain primitive map-key domains by textual conversion in normal JSON, but do not build a canonical cross-language contract around language-specific map-key coercion.

For CodeFabric's non-string-keyed logical maps, use the design's explicit array-of-record representation and sort those records according to the contract **before** JCS serialization.

## 16) Resource limits and denial-of-service posture

Parser validity is not a resource policy. Bound, as appropriate:

- raw input bytes;
- nesting depth;
- object member count;
- array length;
- individual string length;
- aggregate decoded string bytes;
- numeric token length;
- total model/DOM allocation;
- time spent in downstream validation and sorting.

Canonicalization may sort every object, so “valid but enormous object with huge member count” is especially relevant to CPU and allocation planning.

## 17) `no_std` / `alloc` posture

`serde_json` can be built with `alloc` instead of full `std` for constrained environments. The practical CodeFabric server/CLI deployment should normally use `std`; reader/writer integration and conventional diagnostics assume it.

Do not remove default features in shared Cargo configuration without confirming every call site. `serde_json_canonicalizer` itself also needs a compatible environment.

## 18) Performance decision table

| Situation | Preferred API | Why |
|---|---|---|
| typed request already in memory | `from_slice` / `from_str` | low overhead, borrowing possible |
| typed request from file/socket | buffered reader + `from_reader` | avoids whole-input copy when suitable |
| unknown shape | `Value` | dynamic traversal |
| duplicate-aware strict ingress | custom `Deserializer` visitor | sees members before map collapse |
| repeated sequential objects | `StreamDeserializer` | parser reuse / bounded record handling |
| opaque sub-value forwarding | `RawValue` | avoids parse/materialize of sub-tree |
| human JSON output | `to_writer` / pretty variants | ordinary Serde JSON |
| fingerprint output | canonicalizer `to_vec` | RFC 8785 semantics |

## 19) Concurrency and state ownership

Top-level parse/serialize calls are independent and require no global mutable parser state. Prefer request-local deserializers and serializers. Share immutable schema/config state rather than parser instances.

When parallelizing validation of independent records, preserve deterministic ordering at any stage that later becomes an array in canonical output. Parallel execution is safe only if collection order is explicitly reconstructed according to the protocol.

## 20) Fuzzing and property-based testing

High-value properties:

```text
parse(serialize(valid_typed_value)) preserves modeled semantics
strict_parse rejects duplicate members at every nesting level
safe integer boundary -9007199254740991 is accepted
safe integer boundary +9007199254740991 is accepted
one step beyond either boundary is rejected where token is integer
canonicalize(strict_parse(x)) is deterministic across repeated runs
Rust canonical bytes == Python canonical bytes for shared fixtures
```

Fuzz the **strict ingress wrapper**, not only `serde_json::from_str`, because the application invariants live above the generic parser.

## 21) Deployment advisory

| Deployment | Recommended stance |
|---|---|
| developer CLI | `Value` acceptable for inspection; strict wrapper for fingerprint operations |
| registry compiler | strict ingress + typed model + canonicalizer; exact dependency pins |
| long-running service | explicit size/depth limits, bounded allocations, structured errors |
| untrusted batch scanner | per-document limits and failure isolation |
| test/fixture generator | `json!`/pretty output acceptable; canonical fixture bytes from canonicalizer only |

## 22) Agent execution playbook

```text
1. Identify whether the call site is parsing, validating, presenting, or fingerprinting.
2. If fingerprinting, refuse to use ordinary serde_json serialization as the final byte emitter.
3. If duplicate rejection matters, stay before Value/map materialization for that check.
4. If numeric-token provenance matters, keep arbitrary_precision and validate before f64 conversion.
5. Make every Serde representation attribute change visible in protocol review.
6. Keep resource limits separate from syntactic validation.
7. Replay the cross-language fixture corpus for every dependency/configuration change.
```
