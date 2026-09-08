# Native schema decode admission

This is the source inventory for `StructType::try_from_json_with_resources`.
It covers that entry point and `Metadata::parse_schema` when a native scope is
installed. The additional fixed transformations, serializer and configuration
phases covered below have separate admission. This is not certification of
arbitrary SchemaTransform visitors, generic deep clones, Arrow conversion, or
the whole native operation.

## Pinned source assumptions

The implementation is tied to Kernel 0.25.1, serde/serde_core 1.0.229,
serde_json 1.0.151, indexmap 2.14.0, Rust 1.98.0 and its std hashbrown 0.17.1.
The actual serde private Content type is used in `size_of`; a changed serde
private version does not silently reuse its layout. Both serde_json Map backends
are included, so `preserve_order` feature unification does not remove a charge.
Numeric bytes use the whole encoded input, including `arbitrary_precision`
number strings and malformed numeric lexemes.

Relevant upstream paths:

- Kernel `schema/mod.rs`: DataType and PrimitiveType Deserialize,
  StructTypeSerDeHelper/StructField Deserialize, MetadataValue's untagged
  Content, StructType::try_new, and unshredded_variant.
- serde `src/private/de.rs`: ContentDeserializer, ContentRefDeserializer,
  `__deserialize_content_v1`, and generated untagged variant handling.
- serde_core `src/private/content.rs`: original enum and sequence/map layouts.
- serde_json `src/value/de.rs`, `src/number.rs`, `src/read.rs`, `src/ser.rs`:
  owned Value conversion, both number representations, unescape scratch and
  error display serialization.
- Rust `alloc/src/raw_vec/mod.rs`: geometric Vec/String growth (minimum at
  most eight elements, doubling); `alloc/src/collections/btree/node.rs`: B=6,
  eleven key/value slots, parent pointer, two u16 and twelve child pointers;
  `alloc/src/str.rs`: lowercase emits at most three chars per source scalar.
- indexmap `src/lib.rs`/`src/inner.rs`: separate Bucket vector and hash index.
- std hashbrown `raw/mod.rs`: power-of-two buckets, seven-eighths load,
  control bytes, alignment and group tail.

## Phase accounting

An allocation-free source scan first checks input bytes, tokens, depth, source
string spans and per-container items. This scan is repeated over the decoded
`schemaString`: limits on the enclosing log/CRC do not cover embedded JSON.
Native serde retains grammar and semantic authority.

DataType first builds a Value, then consumes it through the selected schema
deserializer. A nested DataType can therefore revisit one source subtree for
each ancestor. The structural JSON depth bounds the number of such ancestors.
For each ancestor the bound includes four graph visits: the Value buffer,
typed schema consumption, MetadataValue's Content buffer and its conversion to
Other(Value). The latter includes ContentRefDeserializer's deep content clone;
the Number, String and Boolean alternatives reject containers by type and do
not each reconstruct a successful container graph. PrimitiveType now consumes
the existing Value instead of cloning it. These terms are retained in the
bound despite that reduction. Strings equal to `variant` synthesize two fields
and a struct; treating every token as such a shorthand bounds this expansion.

The implementation adds each concrete allocation family: Value and field
vectors, actual Content and Content-pair vectors, IndexMap Bucket vectors and
their separate hash indexes, metadata and duplicate-name hash maps/sets, both
Value map backends and recursive Box payloads. It does not infer allocation
size from the largest application DTO. B-tree node geometry allows padding
before each field and at the tail, without assuming Rust's private field order.
For each growing vector it reserves the sum of old and new capacities; for hash
tables it includes bucket/control/tail geometry over geometric growth.

String terms cover unescape scratch, owned Value/Content/native strings,
primitive names and duplicate map keys. UTF-8 lowercase expands by at most
twelve output bytes per source byte: at most three chars, each at most four
bytes, for an input scalar occupying at least one byte. Its moving reallocations
are included separately. Synthetic names are `metadata`, `value`, and `struct`
(nineteen bytes total).

Malformed input remains bounded. Debug escapes use at most ten output bytes per
input byte. Schema serde's fixed expected-field/type descriptions, primitive
and decimal messages, duplicate-name messages and line/column suffixes together
fit the separate 1024-byte allowance for each token/pass; the dynamic name or
Value content is charged in the source-byte term. Error strings can be wrapped
at each ancestor, already included in the pass count. Automatic kernel
backtrace capture is disabled inside a governed native scope.

## Lifetime and proof boundary

Both final backing and transient decode capacity are admitted before serde.
Successful decode releases transient capacity and stores the retained scope in
the original StructType. Arc clones preserve original backing and the scope.
An error stores both owners until the error is dropped. Compatibility deep
Clone intentionally clears the owner: it is not a fallible admitted-copy API.

These deliberately conservative source bounds are requested-allocation layout
bounds, not RSS or allocator overhead measurements. The external native probes
defend reject-before-parse ordering, shared original ownership, transient
release, error lifetime and context restoration. They do not by themselves prove
all possible input geometry; source review and allocation-observation falsifiers
are also required before the artifact is eligible for adoption.

## Fixed schema transformations and serialization

`SchemaRelaxationAdmission` additionally covers the installed
StringifyFailureProneLeaves transformation used by native JSON statistics:
Timestamp/TimestampNtz/Date/Decimal primitive leaves become the inline String
variant. Names and metadata are preserved; arrays, maps and variants are not
traversed. It adds no field or variable backing beyond the fixed physical
transform's existing output/scratch bound. Admission begins before the visitor
and its actual owned StructType receives the original scope on completion.
This internal seam is not a bound for arbitrary user-supplied visitors.

`SchemaTransformAdmission` covers native MakePhysical and with_fields_filtered.
It reserves output before transformation, including additional field-id metadata
and copied physical names. Its transient reservation includes the original
backing copied at each possible ancestor, both Cow/field descriptor vectors,
path and duplicate-id/name visitor sets, and schema diagnostics. MakePhysical
mutates an owned field's metadata instead of deep-copying the complete field
through successive with_name/with_metadata helpers. The output StructType keeps
the original new reservation; metadata semantics and field order are unchanged.
Caller-provided predicate allocations are outside this native bound.

StructType serialization traverses borrowed original fields. The former helper
deep-copied every field before writing. The admitted metadata serializer runs a
nonallocating counting sink, reserves its exact new byte length, then serializes
again into a fallibly reserved Vec whose writer refuses to grow past that
length. Both visits share an admitted fixed validation/diagnostic bound. The
resulting String moves into an independently admitted Metadata copy. No
serialized copy is used as evidence of the original schema's ownership.

## Table configuration

`table_configuration_resource.rs` covers the fixed try_new_inner path before
TableProperties parsing and physical/filtered schema creation. Property source
bytes bound ColumnName parsing and its path vectors, recognized String-valued
properties, unknown-property HashMap growth and invalid-value diagnostics.
The separate transient reservation covers partition HashSets and the fixed
timestamp/variant/Iceberg V3 validation visitors. The latter includes their
Vec<String> paths, path copies/joining and rendered offending DataTypes.

The original TableConfiguration retains its property charge; its schema Arcs
retain their separate original schema owners. try_clone_admitted shares those
Arcs but admits new property maps/paths/strings, URL and Metadata/Protocol copies
before cloning. The update and snapshot callers use this native fallible copy.
This does not certify later stats-schema construction or arbitrary feature
validators. Those remain separate selected-operation admission obligations.

External native allocation observers exercise both mapping modes, field/map
growth boundaries and unknown/parsed properties. All requested layouts observed
in those cases fit the reservations made before those phases. This falsifier
supplements the source bound; passing samples are not a universal proof.
