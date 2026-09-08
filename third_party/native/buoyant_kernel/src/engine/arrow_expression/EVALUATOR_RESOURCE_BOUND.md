# Native evaluator allocation boundary

This amendment governs existing selected Arrow operations; it does not introduce
an expression engine or substitute a retained handle for allocation admission.

Implemented boundaries:

- ARRAY and COALESCE admit column vectors, `ArrayData` descriptors and borrowed
  descriptor vectors before allocation. They invoke native `try_to_data`,
  `MutableArrayData::try_new`, `try_extend`/`try_extend_nulls`, `try_freeze`, and
  `try_make_array`. ARRAY admits its exact i32 offset vector, immutable buffer
  owner, list field and final concrete Arc. Existing offsets and values remain
  native Arrow layouts.
- `null_row` uses borrowed kernel types and the scalar resource preflight before
  constructing each native builder. `create_many` validates and preadmits the
  complete original row set before appending, with one aggregate append request.
  Both use selected native `ArrayBuilder::try_finish`, retain fresh buffers and
  admit schema/column/engine-data descriptors. Empty row/field counts are explicit.
- Expression/predicate evaluator Arc allocations are admitted before construction.
  Evaluators retain original NativeDataOwners and the actual neutral Arrow owner
  after payload fields. Use under an absent required scope is rejected. Output
  engine-data owners inherit input and evaluator owners. Selected one-column
  output schema/field/column descriptors and the empty StructPatch input clone
  are admitted before allocation.
- ParseJson resource exhaustion returns before the optional malformed-input
  null-stats fallback. Resource pressure cannot become a successful fallback.
- NativeDataOwners has a clone-stable cached inherited depth, a hard pair-tree
  depth cap of 64, and an explicit stricter merge API accepting 1..=64. Depth is
  checked before admission or allocation across fresh scope generations. Debug
  does not recurse through the pair tree. This cap is separate from an
  application's immutable original-owner group DAG limit.

The exact 59.2 native probe observes System allocator calls and full realloc
layouts. It checks allocation order for public numeric/string COALESCE, ARRAY,
null_row and create_many, typed descriptor refusal before allocation, evaluator
owner lifetime, and pair-depth refusal before a new receipt or Arc. The scalar
producer has separate constructor/growth/finalization probes. These observations
are requested-layout evidence, not measured RSS or allocator-overhead accounting.

Schema reconstruction now borrows original Struct/List/Map columns and fields,
admits actual column/FieldRef vectors, Field names/Arcs, Fields Arc slices, metadata
maps and nested paths before allocation, and transfers native offsets/nulls. The
consuming Arrow RecordBatch-to-StructArray conversion moves its actual columns
Vec instead of copying it. Struct and StructPatch admit descriptors and final
Arcs; patch capacity is checked before a retained input column is pushed. Struct
nullability uses the original native bitmap operation after admitting a finite
sum of its exact aligned-Vec, unaligned-buffer/replacement and normalization
allocation branches. Null fallback uses borrowed schema/scalar preflight and
native try_finish, with an admitted retained null bitmap.

The added public probes cover nested schema metadata and preserved owner lifetime,
nullable bitmap offsets and lengths, descriptor denial before column allocation,
and malformed JSON fallback. The native batch transfer probe checks the original
Vec pointer and observes zero allocations.

MapToStruct now admits its original field/type/builder vectors, bounded native
HashMap index, match/offset census and output vectors before construction. Each
selected primitive parse admits raw-input copies, native Decimal concatenation
and timestamp/decimal diagnostics before parsing; each native append uses the
scalar cumulative replacement-allocation bound. String/Binary offsets are checked
across all rows before each parse/append. Native try_finish retains actual output
buffers. Duplicate-key rightmost selection and null-map propagation are unchanged.

The JSON stats no-change path scans borrowed Struct primitive leaves rather than
running the allocating generic Cow transform. Arrow target schemas are admitted
before conversion. The changed path uses the specific SchemaRelaxationAdmission
for the installed primitive-to-String transform and retains its original schema
owner. Empty JSON batches use admitted native builders and explicit zero rows.

Remaining independent allocation families require their own selected-path proof:
unselected encoded-array native compute branches and visitor/constructor diagnostics. Ordinary diagnostic allocations in remaining
unamended native validation paths also require their source-bound admission. Carrying an input owner or returning an owned batch
does not itself admit those newly allocated outputs. Any of these selected by a
production phase remains an implementation obligation.


Concrete remaining source inventory (all exact Arrow 59.2):

- Array/array numeric, selected plain primitive/string/binary comparisons and
  predicate bitmaps now have source-specific admission below. Dictionary,
  RunEndEncoded and Union logical-null materialization explicitly reject under
  required ownership until their native expansion geometry is separately proved.
- Native view conversion currently admits the direct reused-block branch with
  absolute source buffer and offsets below u32::MAX. The larger-block native copy
  fallback is guarded out before work. Application profiles must prove selected
  original buffers meet this geometry; it is not a semantic CPG type exclusion.
- ToJson and arrow_utils::to_json_bytes now use the prepared native encoder
  below. Other legacy generic Writer<W> consumers are not certified by these
  selected call sites; an arbitrary Write implementation is not an admission
  boundary.

Prepared native JSON encoding:

- Encoder Boxes, Struct FieldEncoder vectors and serde-escaped field names are
  admitted before allocation. Exact row byte lengths are computed by the
  original native encoder's borrowed values and nonallocating serde/Display
  counting sinks. Output growth admits each complete new Vec capacity before
  try_reserve_exact. ToJson additionally enforces i32 aggregate offsets before
  encoding; offsets, validity, immutable owners and final StringArray Arc have
  separate admission.
- Decimal/temporal private ArrayFormatter Boxes are admitted from their actual
  state/reference layouts. Default formatter scratch admits both counting and
  writing passes. Native decimal conversion materializes a signed integer of
  at most 79 bytes, then one format_decimal_str output with at most 128 padding
  bytes. Each geometrically growing String admits a cumulative bound four times
  its maximum capacity (including a minimum initial capacity). RFC3339 uses
  chrono's initial capacity38 and at most a six-digit year, fractional nine
  digits and offset; invalid temporal diagnostics include the bounded numeric
  value/type plus the actual borrowed timezone length. Custom temporal formats
  and custom encoder factories explicitly reject under required ownership before
  constructing an encoder, since their native growth is not this default bound.
  RunEndEncoded has the same guarded-path rejection until its geometry is proved;
  selected Delta schemas do not request these paths.
- Delta line writing prepares one encoder per actual filtered batch. Its map-only
  explicit-null option retains null partition map entries while ordinary null
  Struct fields remain omitted, preserving the old custom encoder's semantics.
  Encoded Vec backing transfers directly into Bytes::from_owner with an admitted
  exact private Bytes Owned<T> allocation. The backing owner retains native and
  Arrow banks after payload fields across nonempty clones/slices and async IO.
  Empty Bytes slices contain no backing and need not retain it.
- Selection setup admits a full replacement selection Vec before extension and
  the actual native BooleanBuffer before filtering. An empty/all-true mask takes
  the native identity path, preserving implicit-true tail semantics.
- Native tests cover required-thread owner capture, old-reader refusal, finite
  output refusal before Vec mutation and unchanged pointer/capacity, map-null
  semantics and custom-format rejection. External exact probes compare native
  writer bytes for selected scalar/list/map/temporal/decimal inputs, observe
  actual allocation order, preserve preexisting bytes on refusal, and check
  original output owners through Bytes clones/slices.

Selected compute output boundary:

- Stats safe casts retain native cast_with_options(safe=true), per-cell NULL and
  native decimal rounding. Preadmission covers trusted_len_unzip's two original
  aligned buffers, both immutable owners, concrete Arc, nested Struct child Vecs
  and parent Arcs. Date32's native Parser can fall back to timestamp parsing;
  this allocated error path is explicitly included. Decimal parsing collects a
  borrowed split Vec before validation; its exact delimiter count, full doubling
  series, padded strings, signed i256 text and escaped diagnostics are admitted.
- Arithmetic uses original arrow-arith array/array dispatch. Its checked path
  creates one aligned buffer; the infallible path creates an exact Vec. Empty
  paths create ArrayData's one Buffer descriptor and null owner. Validity union
  uses the existing bitmap bound. The largest operand width, at least eight for
  Date32 subtraction's duration result, bounds native payload width. Primitive
  array generic T is phantom in the owned descriptor. Native i256/type diagnostics
  and eagerly allocated timestamp interval messages are admitted separately.
- Plain comparisons construct at most one values and one final validity/distinct
  bitmap. Native aligned/unaligned and padded-word collection branches are each
  included. NOT, IS NULL, IS NOT NULL and Kleene conjunction/disjunction count
  their actual bitmap nodes. Final buffers retain the current owner. IN also
  admits each original valid-row concrete primitive/String slice Arc. Empty
  junction and literal IN use native BooleanBufferBuilder after admission.
- Mixed byte/view normalization admits native view vectors, completed-block Vec,
  immutable view-owner and buffer Arc tails. View-to-byte additionally admits
  the original ArrayData round-trip, exact aggregate bytes, i32 offsets, builder
  reset and final descriptors. List field metadata is copied after admission.
  ListView-to-List first checks expanded row ranges against the native offset
  limit, then uses amended MutableArrayData range extension in original row order
  (including overlap), skips null rows, and preserves the original parent nulls.
  It does not allocate take indices or invoke an unguarded native take kernel.
- External tests compare original native results on zero/nonzero/sliced nullable
  arrays, malformed safe casts, decimals and arithmetic overflow; mixed view
  comparisons and overlapping list views; and observe System allocation order.
  Admission refusal is typed and checked before original output/parser allocation.

Append and diagnostic boundary:

- append_columns admits schema conversion, complete old+new field/column vectors,
  Fields tail, schema Arc and engine-data Box; new payload remains the selected
  scalar ArrayData::to_arrow producer. Original columns are cloned as handles,
  preserving their actual backing. Row/field mismatch is checked before native
  construction and formatted after admission. The zero-existing-column behavior
  still derives row count from the first new column, matching RecordBatch.
- Selected evaluator diagnostic sites now use a nonallocating fmt counting pass
  followed by one admitted String allocation and consuming error enum constructor.
  They avoid Error::generic/unsupported's ToString copy of an existing String.
  A refusal remains inline ResourceExhausted/ResourceOwnerError, including error
  closures whose return type cannot itself use `?`. Native external probes deny
  the diagnostic allocation and observe zero replacement allocations.

Validation and visitor setup boundary:

- ensure_data_types compares MetadataValue's original Display text using a
  nonallocating streaming equality sink, including nested serde_json metadata.
  It admits actual native schema conversion and diagnostics; missing-field text
  uses a repeatable borrowed iterator and checked exact String allocation, with
  no temporary HashSet or uncharged join buffer.
- ArrowEngineData::visit_rows admits the finite map bucket allocation, each leaf
  and prefix key, traversal path and getter Vec. Keys borrow original column-name
  strings; traversal borrows original Arrow field names. An explicit native path
  ceiling of 64 is checked before map allocation. Borrowed path formatting shares
  ColumnName's exact escaping algorithm without copying paths or text.
- Successful scalar/list/map getter extraction borrows original arrays and does
  not slice/clone concrete arrays. New native probes cover nested escaped paths,
  nullable 0/1/65-row inputs, missing-field diagnostics, zero-allocation metadata
  equality, pre-map refusal and later getter-storage refusal without callbacks.
- Visitor-selected process-global LazyLocks and owned action materialization are
  separate lifecycle boundaries: warming static fixtures outside an allocation
  observer does not prove their production initialization or output ownership.

