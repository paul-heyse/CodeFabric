# Selected Arrow schema hint admission

`ArrowWriter::try_new_with_options` calls the fallible
`try_add_encoded_arrow_schema_to_metadata` before the original native hint
encoder. The admission walk borrows the original Schema and WriterProperties;
it allocates no model, serialization or recursive work list. It preserves the
original IPC/Base64 bytes, top-level run-end flattening, and hint replacement.
The existing compatibility String encoder is not an independently owned output
API. The selected writer retains the original policy through its metadata;
returned metadata lifetime remains part of the native writer ownership proof.

The exact inspected sources are Arrow IPC59.2.0 `convert.rs` and
`writer.rs::IpcDataGenerator::schema_to_bytes_with_dictionary_tracker`,
FlatBuffers25.12.19 `builder.rs`, Base64 0.22.1's standard padded encoder, and
Rust1.98 `alloc/{raw_vec,slice,sync}` plus `core/slice/sort/stable/mod.rs`.
This is an allocation-layout upper bound derived from those algorithms, not
a compressed-byte multiplier, measured RSS budget, or substitute encoder.

The walk counts every native Field/type/DictionaryEncoding/Int/KeyValue table,
Schema and Message, string and child vector. Each generated table has at most
eight slots with scalars/offsets at most eight bytes. Its bound gives every
slot seven bytes of alignment, a signed table offset, and all ten two-byte
vtable entries. Strings add their byte length, terminator, u32 length and
alignment. Vectors add every element, u32 length and alignment. Root offset
and final alignment are included. Duplicate vtables may reduce the result.
Flattened run-end fields have no more tables/strings than the original tree.

Before native encoding, admission includes:

- Every full geometric replacement of the FlatBuffer's original byte Vec.
  DefaultAllocator doubles its logical size; RawVec's initial u8 capacity is
  included. The sum is bounded by four times max(required bytes, eight).
- `field_locs` (native u16/u32 pair, eight slots), the original Vec of written
  vtable positions, and DictionaryTracker's i64 ID Vec. The schema-only path
  never inserts into its `written` HashMap or the builder's shared-string pool.
- Schema/Struct/Union field-offset Vecs, Union type IDs, metadata key and
  KeyValue-offset Vecs. Each uses the corresponding full RawVec replacement
  sum. Metadata stable sorting has a separate n-reference scratch bound;
  the small-sort minimum fits its 4096-byte stack scratch.
- The complete IPC message copy, legacy prefix Vec, two temporary four-byte
  prefix Vecs, exact padded Base64 capacity and Arrow metadata key String.
- WriterProperties' metadata Vec growth, considering its actual spare capacity
  and whether replacing the existing hint requires any growth at all.
- If top-level run-end flattening occurs: every cloned top-level field/name,
  original-capacity HashMap clone with all key/value Strings, recursive boxed
  Dictionary DataTypes, the Field Vec, per-field Arc layouts and Fields Arc
  slice. Nested fields, UnionFields and timezone strings clone original Arcs.

All arithmetic, total fields, per-collection entries, string bytes and schema
depth are checked before native work. The calculated wire bound must also fit
the explicit footer ceiling and the native FlatBuffer 2GiB limit. The complete
allocation bound must fit the output ceiling before the policy reserves it.
Denial uses the inline native ResourceExhausted variant before any mutation.

The native package tests run the original encoder under a forwarding System
allocator observer with preallocated receipt slots. They check cumulative
allocation-before-admission, exact metadata equality with the ungoverned
encoder, dictionary/nested/temporal/run-end cases, wide schemas, retained map
capacity and replacement of an existing hint. A refused reservation allocates
zero bytes and leaves original metadata unchanged. These tests falsify the
source bound; they do not establish platform allocator overhead or RSS.
