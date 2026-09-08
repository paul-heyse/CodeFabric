# Selected scalar and null-row native allocation boundary

This amendment covers the existing scalar/ArrayData conversion path on Arrow
59.2.0, using the installed required native owner. It does not substitute a
serialized scalar, clone a kernel schema to estimate it, or reconstruct output
from observed rows. Ungoverned callers retain the existing native behavior.

## Phase inventory

1. Borrow the original scalar/type/fields. Check recursive type/value/metadata
   depth before descending, validate values against their declared native types,
   and calculate aggregate output values and variable bytes with checked
   arithmetic. The selected list/map/string/binary builders use i32 offsets;
   their aggregate cap also bounds every individual descendant offset. A null
   struct produces null child rows; a null list/map produces no child entries.
2. Admit kernel-to-Arrow schema conversion. The bound enumerates the concrete
   Vec<Field>, Vec<FieldRef>, Field Arc, Fields slice Arc, names, metadata
   HashMap<String,String>, serialized non-string metadata, synthesized list/map
   fields, nested-ID paths and diagnostics. Hashbrown's pinned 7/8 load,
   power-of-two bucket/control geometry is included. Metadata serialization
   writes borrowed values to an allocation-free counting sink after a depth
   preflight. It does not allocate a serialized source or deserialize it again.
   Timestamp UTC Arc<str> uses Layout::extend and pad_to_align, including the
   header's padding. Borrowed scalar-to-Arrow conversion avoids the old
   Scalar::data_type deep-copy intermediary for containers.
3. Admit the selected make_builder constructor before it runs. Each supported
   branch uses the size of its concrete builder Box and actual initial Vec
   capacities. String/binary constructors allocate 1024 data bytes. List/map
   offsets start at rows+1; child constructors inherit native make_builder's
   original row capacity. Struct constructors allocate one Box reference per
   field. Map field-name copies are charged separately. Lazy validity and
   Boolean backing use native 64-byte-rounded bitmap growth geometry.
4. Admit cumulative append growth before append_to. Exact borrowed values and
   repetition counts determine appended primitive widths, variable data bytes,
   offsets, validity and every descendant contribution. A growing RawVec's
   sum of requested replacement layouts is below four times its final/minimum
   requirement; constructors are charged separately, so the original and full
   replacement backing remain simultaneously admitted. Zero rows and nullable
   containers retain their native behavior. This is a sum over the concrete
   allocation families, not a largest-DTO coefficient or RSS estimate.
5. Call the amended ArrayBuilder::try_finish. That native seam inspects actual
   builder backing/capacities and preadmits immutable Bytes Arc descriptors,
   output array Arcs, offset reset vectors, recursive child finalization and
   map conversion/factory work. This phase is independent of the earlier
   payload bound. Unknown custom builders reject required ownership before
   allocating; the selected supported native branches retain their behavior.
6. Visit original output buffers through Array::try_visit_buffers, without
   allocating an ArrayData graph. The fresh-buffer helper fallibly admits its
   claim object before attachment, preserving an existing original claim.
   Concrete output array/schema roots retain the same owner. A final bare
   Buffer clone consequently retains the actual admitted original backing.

The bound is cumulative, conservatively retaining old allocations through the
operation's owner. Later work may use a smaller measured admission profile but
cannot weaken preallocation, required ownership, or joined lifetime rules.

## Evidence and limits

The private native probe `scalar_layout` observes System's requested layouts
without changing allocation outcomes. It checks admission before each observed
allocation, total requested layouts, zero-allocation denial, offset overflow,
original Buffer escape lifetime, primitive/nullable/date/timestamp/decimal,
string/binary, nested map/list/struct, repeated rows and ArrayData elements.
The timestamp padding falsifier detected and corrected the Arc<str> layout.
Arrow's separate finalization tests inspect native current capacities and
preservation on denial. These checks do not certify unrelated expression
operators, provider inputs, default engine construction, task geometry or
production integration. Those consumers retain their separate WP79 obligations.
