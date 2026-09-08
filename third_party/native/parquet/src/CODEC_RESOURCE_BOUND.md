# Selected codec allocation contract

This amendment covers the governed UNCOMPRESSED, SNAPPY and ZSTD codec
entrypoints. Outside a required/current native policy the original factory and
codec implementations remain selected. Other governed codecs are rejected with
an inline typed resource failure. The bound is not a claim about all Parquet
writer encodings, statistics, Thrift, encryption, or Arrow conversion.

The exact source assumptions are Parquet59.2.0, snap1.1.1, zstd0.13.3,
zstd-safe7.2.4 and zstd-sys2.0.16+zstd.1.5.7. The final resolved graph must
confirm these identities. The ZSTD experimental static API is deliberately
enabled in the amended native package. Upstream code, checksums and the ordered
patch belong in the immutable native artifact, not a modified Cargo cache.

## Admission and ownership

`create_codec` checks the effective policy before constructing the selected
native codec. Its concrete Box layout is reserved first. ZSTD workspace
creation is lazy and fallible; a reader never constructs encoder scratch.
Every call validates the required worker's original owner, so moving a codec
to another governed operation cannot relabel its existing allocation.

Full new output capacity is reserved before `reserve_exact` and resize. Prior
receipts remain held across growth and context reuse. Encoded and decoded input
sizes are checked against the finite page profile, and output lengths against
the finite output profile. Checked Vec arithmetic rejects overflow before native
work. These are requested-allocation layout bounds, not allocator or RSS bounds.

Native error values are kept inline until the exact `ScopedCodecError<E>` Box
layout has been admitted. The diagnostic owns the original policy, including
after codec destruction. Malformed data remains a normal native error; admission
denial stays an inline resource error. Unsupported legacy ZSTD frames are a
typed unavailable bounded profile. No formatted String is allocated first.

The low-level codec trait writes into a caller-owned Vec. Its contract does not
magically place an owner in that Vec. Production writers must retain the original
policy in their page container and in any original Bytes backing escaping the
writer; the separately amended native writer seam owns that obligation. The
codec, diagnostic, and original output owner release after their payload fields.

## ZSTD fixed workspace

`ZSTD_estimateCCtxSize(level)` in `lib/compress/zstd_compress.c` takes the maximum
across all compression levels through the selected level and the native 16KiB,
128KiB, 256KiB and unknown input-size tiers. Its documented bound applies to
single-shot compression at that level for any source size, and excludes the
streaming and multithreaded APIs. We use that bound, round upward to the native
eight-byte alignment, check the finite codec ceiling and admit it before any
Vec allocation. The native default level is1.

The backing is a fixed Vec of `repr(C, align(8))` eight-byte words. Moving the Vec
does not move its allocation. `ZSTD_initStaticCCtx` checks alignment and size,
zeroes the context and installs only pointers into this backing. Initialization
is repeated for every independent page. The prior one-shot operation installed
no owned dictionary, worker or dynamic backing requiring a destructor; no prior
caller input/output pointer survives that reinitialization. The Vec is never
resized afterwards, and no native free is called on a static context.

`ZSTD_compressCCtx` delegates to `compress_usingDict` with no dictionary and
single-shot source size, resetting advanced options and selecting single-thread
compression. `ZSTD_resetCCtx_internal` refuses static workspace expansion. The
destination is the disjoint live Rust slice of exactly `compress_bound(input)`
bytes, already admitted, not unused Vec capacity. Error inspection allocates
nothing. Static workspace and destination exhaustion return typed failures.

`ZSTD_estimateDCtxSize` is the compiled `sizeof(ZSTD_DCtx)`. Its fixed workspace
uses the same alignment and admission rules. Decode requires the caller's exact
finite output bound. Every concatenated frame is inspected before native decode:
only standard and skippable modern frames are accepted. This excludes legacy
decoder allocation paths. In the pinned source `decompressMultiFrame` calls
`decompressBegin_usingDict(NULL,0)` before `checkContinuity`; `decompressBegin`
resets the previous output/dictionary pointers. Earlier outputs may therefore
drop before the next independent decode. No external dictionary or streaming
window is created.

## Snappy fixed scratch

The snap1.1.1 Encoder contains an inline `[u16;1024]` small table and an initially
empty Vec. Its only dynamic scratch in `compress.rs::Encoder::block_table` is
the first allocation of `MAX_TABLE_SIZE` (`1 << 14`) u16 entries. It never grows
that table again. The entire 32768-byte layout is reserved before the first
compression, while the small table was covered by the original codec Box.
`max_compress_len` bounds the admitted destination. The native encoder writes
only to that provided slice, block by block.

The decoder is zero-sized, uses no Encoder, and writes only into the admitted
caller destination after checking the stream's declared length. Native snap
errors use the admitted diagnostic owner described above.

## Evidence and limits

The focused native tests cover static encode/decode round trips, context reuse
after each old input/output drops, lazy read construction, scratch/output denial
before output growth, foreign worker rejection, malformed diagnostic admission
and final-owner retention. A private System-allocator observer additionally
checks admission order for Rust allocations at original public codec calls; its
receipt bank is preallocated outside observation and its allocator forwards all
original pointers/layouts unchanged. C allocation closure depends on the static
source analysis and independent FFI review, not that Rust observer.

No ASAN, Miri, exhaustive malformed-frame result, production sizing, complete
writer allocation closure or terminal packet certification is implied.
