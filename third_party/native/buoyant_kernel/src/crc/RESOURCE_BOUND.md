# CRC native allocation boundary

This amendment preserves `CrcRaw`, native Serde, native CRC validation, and native
replay/transaction decisions. It admits requested allocation layouts before the
selected allocation phases and retains receipts on the original native owners.
It does not certify complete Kernel, Delta, or workspace resource closure.

The source baseline is buoyant_kernel 0.25.1, serde/serde_core 1.0.229,
serde_json 1.0.151, bytes 1.12.1, and Rust 1.98.0. Rust's std HashMap is backed by
hashbrown 0.17.1. Executable evidence uses x86_64, Arrow/Parquet 59.2.0, and the
coherent staged leaf resource amendments. This is an allocation-layout claim,
not allocator arena/RSS accounting. A pin, target-layout, or native implementation
change requires renewing the source audit.

## Decode phases and source geometry

`Crc::try_from_json_bytes_admitted` resolves the required native worker policy
before inspection. `JsonResourceLimits::inspect` performs the allocation-free
source scan. The resulting bytes/tokens/container counts describe the actual
uncompressed CRC body. They are not derived from compressed bytes, rows, or an
application multiplier. All bound arithmetic rejects overflow and values above
`isize::MAX` before Serde runs.

`CrcRaw` in `mod.rs` has Metadata, Protocol, optional vectors of SetTransaction
and DomainMetadata, and a validated FileSizeHistogram. Metadata's owned fields
are strings, String maps, and a String vector. Protocol's feature vector has a
tagged TableFeature enum with an untagged Unknown(String) fallback. Unknown CRC
fields use native ignored-value parsing; unsupported Crc `all_files` is not part
of this wire DTO.

`resource::decode_bytes` adds the following source-specific families:

| Phase | Native allocations and bound |
| --- | --- |
| Serde owned vectors | SetTransaction, DomainMetadata, TableFeature, String, i64, Serde Content, and `(Content, Content)` slots are summed using their actual Rust sizes. Content uses `serde::__private229::de::Content`, so a serde patch-version change fails compilation instead of silently retaining a stale enum-layout estimate. |
| Vector growth | Serde's sequence visitor and RawVec grow geometrically. The sum of full new layouts is bounded by four times actual token count plus eight initial slots per source container, for each explicit vector family. The token count includes object keys and scalar values. |
| Raw and converted maps | The three native bucket families are `(String,String)`, `(String,DomainMetadata)`, `(String,SetTransaction)`. The latter two are built when original CrcRaw vectors move into CRC keyed maps. The sum of growing layouts includes power-of-two bucket rounding, the native 7/8 load rule, each control byte, and the <=16-byte SIMD tail/alignment. The bound allows all three families for every counted element; it therefore covers duplicate-key replacement and both construction phases. |
| Strings | Four complete source-byte spans cover native decoded strings, unknown-feature/Content owned conversion, and separately cloned application/domain map keys. Escaped JSON does not expand beyond its encoded UTF-8 source span. The whole byte count is used because arbitrary_precision feature unification can make a malformed feature's numeric lexeme an owned String even when the scanner counted no quoted string. |
| Parser scratch | SliceRead's escaped-string scratch and arbitrary-precision numeric scratch use checked `4 * (source_bytes + 8)` full growth geometry. |
| Diagnostics | Native static field/variant diagnostics plus Debug escapes and error wrapping are admitted before parsing. TableFeature's audited serialized names are <=64 bytes; the variant-list term derives its count from the native EnumCount. Ten ASCII bytes per input byte bounds Rust Debug escaping; four owned formatting copies cover the source-derived message and Serde/native wrappers. Scoped native errors do not capture an unbounded backtrace. |
| Owner cells | CrcRaw/Crc and the error owner wrapper, including Arc/box bookkeeping, are included explicitly. |

The current decode request retains this conservative combined phase allowance in
the scope until its last owner drops. Scratch has not yet been split into a
shorter-lived transient receipt. This is safe over-retention and a performance
follow-up; it is not a statement that parser scratch remains physically live.

The native TableFeature tagged attempt examines borrowed Content; Unknown String
conversion can copy that original string. Malformed nested Content does not
recursively deserialize more TableFeature values. No recursive `serde_json::Value`
schema path is covered here: the embedded metadata.schemaString has a separate
decoded-schema admission boundary.

## Original retained ownership and copies

The final field on Crc and private CrcDelta is a CrcResourceOwner holding the
original scope. Normal Arc clones preserve original object/string identity and
the receipt. Compatibility deep Clone deliberately clears this field; it cannot
pretend a newly allocated deep copy was admitted by cloning the receipt.

`try_clone_admitted` traverses borrowed original fields before cloning. It charges
all original strings, Vec output slots, histograms, supported and unsupported
owned fields, and HashMap layout using current capacity, including an oversized
empty map. `try_apply_admitted` additionally admits moved predecessor/delta backing
into the selected owner, map growth, and histogram copying before native apply.
Version-zero conversion preadmits its filtered domain map and new CRC owner.
Scope selection rejects a foreign explicit owner while an owned worker policy is
required. An ungoverned compatibility call remains ungoverned.

Errors that retain native formatted strings keep the scope in ScopedCrcError.
Static typed ResourceExhausted requires no retained variable-sized message.
Arrow JSON and Parquet pressure remains typed through Arrow ExternalError source
chains and the Kernel Error conversion; display text is never classification.

## Writer

`try_write_crc_file` admits native CrcRaw conversion and validation diagnostics
before serialization. A native serde_json::to_writer pass writes to an
allocation-free checked counter, obtaining the exact output length. A second
conversion and exact-capacity output Vec are admitted before allocation. A
bounded writer rejects unexpected growth. This preserves native serialization
and conditional `overwrite=false` storage semantics.

`Bytes::from_owner` carries the actual Vec and scope into StorageHandler::put.
Clones and slices share that original backing owner. The test deletes the stored
object, drops CRC/engine/scope/whole Bytes, and verifies that the remaining slice
still holds the resource charge until its own final drop.

## Governed caller inventory

* reader::try_read_crc_file -> admitted original CrcRaw parse; optional malformed
  CRC fallback propagates ResourceExhausted instead of replaying around pressure.
* LogSegment::build_incremental_crc_from_base -> histogram seed admission,
  original CRC fallible clone, and fallible admitted apply.
* LogSegment::build_incremental_crc_delta -> checked filtered Vec growth and exact
  FileMeta/URL clone geometry before collection; original borrowed getter lengths
  before String/list/map materialization. Map bucket admission occurs before
  each batch's insertions. Diagnostic errors keep their scope.
* Transaction::commit -> synchronous native resource context for the whole call;
  FileStatsDelta histogram admission precedes original vectors; build_crc_delta
  preadmits native key/value conversions and Metadata/Protocol clones; the
  version-zero and post-commit paths use admitted CRC conversion/apply.
* Snapshot::new_post_commit -> admitted original CRC clone and apply.
* Snapshot::write_checksum -> admitted native CRC writer.
* Engine/DefaultEngine/StorageHandler policy getters inherit the active required
  worker policy when no explicit field is installed; MeteredDeltaEngine forwards
  the original owner.

## Evidence and remaining boundaries

The external exact-source native probe at `probe/tests/crc_resource.rs` covers
denial before Serde, original Arc identity/drop, denied and successful deep clones,
large empty HashMap capacity, error lifetime, long numeric lexemes, byte slice
lifetime after deletion, denial before storage mutation, optional CRC pressure,
engine forwarding, nested Arrow/Parquet typed pressure, required worker policy,
native incremental replay admission, and native create-table version-zero CRC.

This boundary does not charge all production work. Open or separately owned
paths include LogSegment/list/path owners,
schema/TableConfiguration copies, Arrow backing and descriptors, native engine
tasks, storage ownership, static lazy schema initialization, and non-CRC
transaction preparations. RowVisitor/EngineData's own extraction descriptor
allocations require their separate native admission seam. The original CRC Metadata/Protocol fields now each retain their original scope
when moved from the enclosing CRC; admitted deep clones request a new complete
allocation bound. Legacy deep Clone deliberately clears admission. Independently
moving the CRC's other public map/vector fields still needs an owned output
boundary; no enclosing CRC receipt is claimed to follow such moves. Native action
Scalar conversion preadmits its own descriptors and formatted feature strings,
but generic create_one/expression evaluation allocations remain a separate
obligation. Normal Protocol/Metadata replay uses borrowed getters and preadmits
original owned strings, lists, and maps before materialization.
