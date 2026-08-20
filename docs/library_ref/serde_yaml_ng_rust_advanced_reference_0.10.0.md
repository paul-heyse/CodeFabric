# `serde_yaml_ng` in Rust — advanced technical reference / YAML 1.1 registry ingestion catalog for LLM coding agents

## Version / source anchors

**Release anchor:** `serde_yaml_ng = 0.10.0`, released 2024-05-26. **MSRV:** Rust 1.64. **Specification scope:** YAML 1.1. The crate is a maintained fork of `serde_yaml` designed for API compatibility with the original project.

Primary anchors:

- https://docs.rs/serde_yaml_ng/0.10.0/serde_yaml_ng/
- https://docs.rs/crate/serde_yaml_ng/0.10.0/source/Cargo.toml.orig
- https://github.com/acatton/serde-yaml-ng
- https://yaml.org/spec/1.1/

Dependencies in 0.10.0 include `serde`, `indexmap`, `itoa`, `ryu`, and `unsafe-libyaml`.

## CodeFabric role

Use `serde_yaml_ng` for **registry YAML ingestion** into validated Rust structures. YAML is an authoring/input format, not the canonical fingerprint format. After parsing and semantic validation, the contract model is converted to canonical JSON semantics and serialized through `serde_json_canonicalizer`.

Never hash raw YAML text as equivalent to `codefabric-jcs-v1`; YAML has many representationally distinct spellings for the same data.

## Public API inventory

Deserialization helpers:

- `from_str<T>`;
- `from_slice<T>`;
- `from_reader<T>`;
- `from_value<T>`.

Serialization helpers:

- `to_string<T>`;
- `to_writer<T>`;
- `to_value<T>`.

Core types:

- `Deserializer`;
- `Serializer`;
- `Value`;
- `Mapping`;
- `Number`;
- `Error` and source `Location`;
- `Sequence` alias;
- `Index` trait for `Value` access.

`with` contains Serde adapter modules such as singleton-map enum representation helpers.

## Installation

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_yaml_ng = "=0.10.0"
```

The crate's Rust 1.64 floor is comfortably below the CodeFabric verified Rust toolchain floor; exact pinning is still required by the canonicalization design's upgrade policy.

## Typed deserialization

```rust
#[derive(serde::Deserialize)]
struct Registry { /* ... */ }

let registry: Registry = serde_yaml_ng::from_str(source)?;
```

Typed deserialization is preferable for governed registries because it keeps schema intent close to Rust types and allows Serde attributes/defaults/deny-unknown-fields policies.

However, Serde type success is not sufficient for CodeFabric semantic validation. Apply format constraints, normalization rules, identity constraints, and cross-record checks after decoding.

## Dynamic `Value`

`serde_yaml_ng::Value` can represent arbitrary YAML values and can be converted to typed data with `from_value` or produced from typed data with `to_value`.

Use dynamic values when schema dispatch or staged validation genuinely needs them. Prefer typed models for normal registry ingestion to avoid ambiguous type coercions surviving too long.

## `Mapping`

YAML mappings can have keys that are not strings, unlike JSON objects. This is central to the CodeFabric boundary: non-string-keyed semantic maps cannot be emitted directly as JSON objects. The design requires canonical conversion to arrays of key/value records with explicit deterministic sorting when such structures are allowed by the schema.

Do not rely on YAML mapping iteration order as canonical identity.

## YAML 1.1 semantics

This crate explicitly implements YAML 1.1 rather than YAML 1.2. YAML 1.1 has historically surprising scalar-resolution rules and a richer type system than JSON. Authoring policy should therefore constrain registry YAML to the subset the schema expects.

For any scalar whose spelling could be ambiguous across YAML implementations, prefer explicit quoting or typed schema validation. Treat cross-language YAML parsing as an ingestion concern; canonical identity begins only after conversion to the shared JSON-domain model.

## Enums and tags

Serde enum values can serialize using YAML `!Tag` syntax. Unit variants may also appear as plain strings in supported forms. The `with` module includes adapters such as `singleton_map` for alternative representations.

If registry syntax is cross-language or long-lived, define one accepted representation in schema/examples rather than accepting every Serde-supported encoding accidentally.

## Multi-document streams

`Deserializer` supports YAML stream/document semantics. If a registry file is required to contain exactly one document, enforce that explicitly. Do not silently concatenate or ignore additional YAML documents.

## Anchors, aliases, merges, tags, and application semantics

YAML has features with no direct JSON lexical equivalent. The parser may resolve some of these into ordinary data structures. Canonicalization should operate on the resulting validated semantic model, not on YAML syntax nodes.

If aliases/merge keys/custom tags are not needed, consider rejecting or constraining them at the registry policy layer to reduce authoring ambiguity and attack surface.

## Numbers

`serde_yaml_ng::Number` represents YAML integer/float values. Before canonical JSON emission, enforce CodeFabric's JSON/JCS numeric domain. A YAML integer valid in Rust/YAML may still be outside the JCS safe-integer domain and must then be represented through an explicit string format (`codefabric-int64`/`codefabric-uint64`) or rejected according to schema.

Do not depend on YAML's wider numeric lexical space to expand the canonical JSON number contract.

## Strings and Unicode

Preserve semantic Unicode strings unchanged into the JSON-domain model. YAML escaping/quoting is an authoring syntax concern. Do not normalize Unicode as part of YAML ingestion unless a schema field explicitly demands it outside JCS.

## Error handling

`serde_yaml_ng::Error` can expose source location information. Wrap parsing failures with file/path/registry context while retaining the original location. Separate parse/type errors from semantic registry validation errors.

Recommended diagnostic dimensions:

- file/path;
- YAML line/column when available;
- schema field / logical record identifier;
- stable CodeFabric validation code;
- human-readable explanation.

## Security / resource policy

YAML parsers process more grammar than JSON. Bound input size, nesting, alias expansion behavior, document count, collection sizes, and string sizes according to deployment risk. Do not accept arbitrary remote YAML as trusted configuration merely because it deserializes into a Rust type.

`unsafe-libyaml` is a dependency beneath the crate. Treat parser upgrades as security-relevant even if the Rust-facing API is unchanged.

## Conversion into canonical JSON

Recommended architecture:

```text
YAML source
  -> serde_yaml_ng typed decode
  -> registry semantic validation
  -> explicit conversion into JSON-domain contract model
     - no non-string JSON map keys
     - wide integers represented by canonical string formats where required
     - bytes represented by canonical URL_SAFE_NO_PAD strings where required
     - deterministic array sorting for set/map-record semantics
  -> serde_json_canonicalizer::to_vec
  -> BLAKE3-256 framing
```

Do not serialize `serde_yaml_ng::Value` generically to JSON and hope every YAML type has the intended protocol meaning.

## Serialization back to YAML

`to_string`/`to_writer` are useful for generated registries, fixtures, or migrations, but emitted YAML text is not stable canonical identity. Formatting changes between versions are acceptable only where files are authoring artifacts; never fingerprint the YAML emission.

## Anti-patterns

- raw-YAML hashes as semantic fingerprints;
- assuming YAML map keys are JSON-compatible strings;
- permitting wide YAML integers to flow into JCS numbers unchecked;
- treating mapping insertion order as canonical;
- allowing extra YAML documents accidentally;
- using YAML serialization formatting as a stable protocol;
- overloading YAML tags/aliases with semantics not represented in the validated contract model;
- ignoring YAML 1.1 vs 1.2 scalar-resolution differences.

## Agent checklist

- [ ] exact crate 0.10.0 remains pinned;
- [ ] parser output is validated semantically before canonicalization;
- [ ] YAML-only types/features are converted explicitly into JSON-domain contract forms;
- [ ] non-string keyed maps become sorted record arrays when the schema permits them;
- [ ] wide integers/bytes use their canonical string formats;
- [ ] raw YAML bytes are never treated as JCS fingerprint material;
- [ ] source locations are preserved in diagnostics;
- [ ] parser/resource limits are considered for untrusted inputs.
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

## 1) Role of YAML in the canonicalization architecture

`serde_yaml_ng` is an **ingress format** for human-authored registry/configuration data. YAML bytes are never fingerprint bytes. The pipeline must make this separation explicit:

```text
YAML source
 -> serde_yaml_ng parse
 -> typed/domain validation
 -> deterministic CodeFabric normalization
 -> JSON-compatible semantic model
 -> RFC 8785 serializer
 -> BLAKE3
```

A formatting-only YAML edit that preserves the parsed model should therefore not change the canonical fingerprint, while any semantic-model change should.

## 2) Specification version: YAML 1.1

The project states that it follows YAML 1.1. This matters because scalar resolution and accepted syntax can differ from YAML 1.2 and from JSON expectations.

Do not write validation rules based on the assumption “YAML is just JSON with comments.” YAML adds tags, block styles, aliases, richer scalar syntax, non-string mapping keys, and other semantics.

For registry formats intended to be portable and unsurprising, define an application-level subset even if the parser accepts more YAML.

## 3) Deserialization entry points

Top-level functions mirror `serde_json`:

```rust
let t: T = serde_yaml_ng::from_str(text)?;
let t: T = serde_yaml_ng::from_slice(bytes)?;
let t: T = serde_yaml_ng::from_reader(reader)?;
let t: T = serde_yaml_ng::from_value(value)?;
```

Use typed deserialization for stable schemas; use `Value` when generic registry transformations or validation need to inspect YAML-native structure first.

## 4) Serialization entry points

```rust
let text = serde_yaml_ng::to_string(&model)?;
serde_yaml_ng::to_writer(&mut writer, &model)?;
let value = serde_yaml_ng::to_value(&model)?;
```

YAML output is for human-facing configuration/export. Never hash this emitted YAML as the canonical object fingerprint unless a future profile explicitly defines YAML-byte canonicalization (the current one does not).

## 5) `Deserializer` and multiple YAML documents

The YAML deserializer supports parsing document streams. A YAML source may contain multiple documents separated by `---`.

Application policy must answer explicitly:

- is exactly one document allowed?
- are empty documents allowed?
- if multiple documents are accepted, what semantic collection do they produce?
- in what order are documents incorporated into the canonical model?

Do not accidentally accept only the first document while silently ignoring the rest.

For CodeFabric registry ingestion, “exactly one logical registry document” is generally the safest default unless the schema deliberately defines a stream.

## 6) `Value` data model

The dynamic YAML model supports YAML's scalar and collection categories, including null, bool, number, string, sequence, mapping, and tagged values.

This is broader than JSON. Conversion into canonical JSON therefore requires an explicit projection policy for anything that is not natively a JSON value.

### Agent rule

Never assume this conversion is total:

```rust
serde_yaml_ng::Value -> serde_json::Value
```

without validating YAML-specific constructs and map-key domains first.

## 7) `Mapping`

YAML mappings can use keys outside JSON's string-key-only model. CodeFabric should not stringify arbitrary keys implicitly because different language/toolchains can choose different textual representations.

If a registry concept logically uses non-string keys, project it into the design's explicit record-array representation, validate key type, and sort records deterministically before JCS serialization.

### Ordering

YAML author order can be useful for presentation, but it is not JCS object order and should not be allowed to influence a semantic fingerprint unless the schema says an array is ordered.

## 8) `Number`

YAML numeric syntax and JSON numeric syntax are not identical. Parse results still need CodeFabric numeric-domain validation before producing JCS.

Do not use “the YAML parser accepted it as a number” as evidence that:

- it is a safe JCS integer;
- it can be represented identically in Python and Rust;
- it should canonicalize as a JSON number rather than a string-encoded typed format.

The application owns `codefabric-int64` / `codefabric-uint64` semantics and the safe-integer policy.

## 9) Tagged values and Rust enums

`serde_yaml_ng` uses YAML `!tag` syntax naturally for many enum representations. Tagged values can therefore enter the dynamic model even when a JSON-only mental model would not expect them.

Policy options:

```text
A. reject application-unknown tags at the registry boundary;
B. deserialize directly into a typed enum whose accepted tags are schema-defined;
C. explicitly convert a tag into an ordinary JSON field/value representation.
```

Do not discard an unknown tag and retain only its payload; that changes semantics invisibly.

## 10) Enum compatibility helpers

The `with` module includes singleton-map representations for enums, including recursive forms. These adapters are useful when compatibility requires enum values represented as maps instead of YAML tags.

Using them changes source representation and sometimes interoperability. Treat `#[serde(with = ...)]` changes on registry models as schema/interface changes.

## 11) Anchors and aliases

YAML permits anchors (`&name`) and aliases (`*name`) to reuse nodes. Even if the parser resolves aliases into a resulting value, application logic should reason about the **resolved semantic model**, not source token identity.

Security/resource considerations include alias-heavy documents that expand into large structures. Establish source size and resulting-model limits and include alias stress cases in parser tests if untrusted YAML is accepted.

Do not make checksums depend on anchor names or whether a value was written inline versus through an alias unless the schema explicitly preserves source syntax—which `codefabric-jcs-v1` does not.

## 12) Merge-key semantics

YAML ecosystems commonly use merge-key syntax such as `<<: *defaults`, but support and processing details are parser/application-sensitive. Do not rely on merge-key expansion implicitly in a canonical registry format without an executable fixture proving the selected pinned parser's behavior.

If the registry allows merges, normalize the **fully resolved mapping** into the canonical semantic model and define duplicate/override precedence. If it does not, reject merge constructs rather than accepting environment-dependent interpretation.

## 13) Comments and presentation information

Serde-oriented YAML parsing is for data, not round-trip preservation of every comment, quote style, indentation, or source position. If the product later needs comment-preserving editing, use a dedicated round-trip syntax representation rather than treating `serde_yaml_ng::Value` as a lossless editor tree.

This is advantageous for fingerprints: comments and stylistic whitespace should normally have no semantic effect.

## 14) Strings and Unicode

YAML offers plain, single-quoted, double-quoted, folded, and literal scalar styles that can yield equivalent string values.

Canonicalization occurs over the **resulting Unicode string value**. Preserve the design rule that Unicode is not normalized merely to make visually similar strings equal. A folded block and a quoted scalar that parse to the same string should fingerprint the same; NFC versus NFD strings remain distinct unless the schema separately normalizes them.

## 15) YAML booleans/nulls and scalar surprises

Because YAML 1.1 has broader scalar-resolution behavior than JSON, human-friendly plain scalars can resolve to types an author did not intend.

A robust registry schema should:

- deserialize into typed fields whenever possible;
- require quotes for strings that are ambiguous under the accepted YAML subset;
- reject type mismatches early with field paths;
- document any scalars where YAML 1.1 behavior is surprising.

Do not “repair” unexpected scalar types silently during canonicalization.

## 16) Dynamic-to-typed conversion

A useful validation flow is:

```rust
let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(source)?;
validate_yaml_native_constraints(&raw)?;
let model: Registry = serde_yaml_ng::from_value(raw)?;
validate_domain(&model)?;
```

Alternatively, deserialize directly to the typed model when no YAML-native pre-check is needed. Avoid redundant parse/serialize cycles merely for validation.

## 17) Error model

`serde_yaml_ng::Error` can expose a location where available. Wrap errors with the registry/file context while preserving the source error.

Separate:

```text
YAML_SYNTAX
YAML_UNSUPPORTED_CONSTRUCT
SCHEMA_TYPE
SCHEMA_FORMAT
CANONICALIZATION
CHECKSUM
```

A source location is useful for authoring diagnostics, but line/column is not stable application identity and should not enter the canonical checksum.

## 18) Error-path enrichment

For large registry models, errors should ideally identify semantic paths (`profiles[3].id`) in addition to YAML locations. Serde itself does not guarantee the application-level path contract you may want, so wrap/augment where needed.

Keep diagnostics deterministic enough for tests without making exact parser wording part of the public protocol.

## 19) Serialization policy

`serde_yaml_ng::to_string` is suitable for generated config, snapshots, and diagnostics. It is not designed as a source-preserving formatter.

If a generated YAML file is committed to source control:

- pin the crate;
- snapshot output if formatting matters;
- do not equate stable YAML formatting with canonical fingerprint stability;
- prefer canonical JSON fixture bytes for byte-level protocol tests.

## 20) Serde model attributes

All normal Serde representation decisions apply:

- renamed fields;
- defaults;
- optional/skipped fields;
- flattening;
- enum tagging/with adapters;
- custom deserializers.

The same Rust type may therefore have materially different YAML and JSON representations. Canonicalization should operate on the validated semantic model, not assume format representations are identical.

## 21) YAML-to-JSON projection checklist

Before a dynamic YAML value becomes JCS input, verify:

```text
[ ] exactly the permitted document count
[ ] no unsupported tags
[ ] no unsupported merge semantics
[ ] mapping keys satisfy the schema
[ ] non-string logical maps are converted to explicit records
[ ] numbers satisfy the target JSON/domain rules
[ ] binary/typed bytes use the explicit codefabric-bytes format
[ ] Unicode is preserved without undocumented normalization
[ ] arrays have defined ordered/unordered semantics
[ ] unordered record arrays are deterministically sorted
```

## 22) Duplicate-key policy

Canonical registries should reject ambiguous duplicate mapping keys rather than relying on “first wins” or “last wins.” Verify the selected parser path and add explicit fixtures for duplicates at multiple nesting levels.

If CodeFabric's strict duplicate rule is specified primarily for JSON source, decide separately whether YAML ingestion enforces the same semantic uniqueness rule; for a registry compiler, symmetric rejection is usually preferable.

## 23) Resource limits

Bound:

- source bytes;
- document count;
- nesting;
- sequence/mapping size;
- scalar length;
- alias expansion / resulting model size;
- downstream sorting/canonicalization work.

YAML's richer grammar makes parser-level resource testing especially important for untrusted registries.

## 24) Threading and concurrency

Parse independent files/documents independently; do not share mutable deserializers. If parallel registry ingestion merges results, define a deterministic merge/sort phase before canonicalization so scheduler order cannot affect arrays or diagnostics.

## 25) Testing matrix

```text
[ ] plain/quoted/folded/literal strings resolving to expected semantic text
[ ] Unicode BMP and supplementary-plane strings
[ ] YAML 1.1 scalar-resolution edge cases
[ ] null/bool/integer/float boundaries
[ ] tags accepted by typed enums
[ ] unknown tag rejection
[ ] anchors/aliases where policy permits
[ ] merge-key fixture according to explicit allow/reject policy
[ ] non-string mapping keys
[ ] duplicate mapping keys
[ ] multi-document input
[ ] comments/whitespace changes do not alter canonical semantic fingerprint
[ ] equivalent YAML and JSON semantic models produce identical JCS bytes
[ ] Rust and Python downstream canonical outputs agree
```

## 26) Deployment advisory

| Deployment | Recommended stance |
|---|---|
| developer-edited registry | typed model, rich location diagnostics |
| CI registry compiler | exact pin, strict subset, deterministic normalization |
| untrusted upload service | explicit size/nesting/document limits, reject exotic constructs unless needed |
| source formatter | do not expect comment/style round-trip preservation |
| canonical checksum path | hash JCS output only, never YAML output |

## 27) Agent execution playbook

```text
1. Treat YAML as source syntax, not canonical bytes.
2. Check YAML 1.1 semantics before assuming JSON-like scalar behavior.
3. Reject or explicitly model YAML-only constructs before JSON projection.
4. Never stringify arbitrary non-string mapping keys implicitly.
5. Preserve Unicode string values without hidden normalization.
6. Make document-count, tag, alias, merge, and duplicate policies explicit.
7. Canonicalize only after producing the deterministic JSON-compatible semantic model.
8. Replay YAML->model->JCS cross-language fixtures for dependency upgrades.
```
