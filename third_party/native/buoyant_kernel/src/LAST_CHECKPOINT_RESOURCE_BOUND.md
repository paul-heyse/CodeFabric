# Native checkpoint hint allocation and ownership

The exact Kernel 0.25.1/serde 1.0.229/serde_json 1.0.151/Rust 1.98 source is
governed by last_checkpoint_hint_resource.rs. The initial allocation-free JSON
scan bounds bytes, tokens, depth and collection sizes before serde. Native hint
thresholds still drop an oversized optimization field after decoding; those
thresholds are not an allocation bound.

The decode inventory includes the separate CRC typed-action/feature geometry,
the schema decoder's complete recursive Value/Content/StructType geometry,
hint-specific Sidecar and HintAction vector layouts, String map bucket/control
growth, path/checksum/tag strings, parser scratch and error owners. The bound
uses the whole original hint shape for each subset, deliberately conservative.
Final and transient decode reservations precede serde. It includes the original
hint Arc header used by LogSegment.

The original decoded hint stores its scope. Its originally decoded embedded
schema Arc is unique at attachment; no schema clone or reparse substitutes for
that allocation. Original embedded Metadata and Protocol actions receive the
same pre-admitted decode owner. Their move/copy APIs retain or freshly admit
those actual allocations. Normal LogSegment clones share the original hint
Arc; explicit deep copies reserve the complete stored geometry first and share
the original embedded schema Arc while owning newly copied action backing.

Native malformed-hint fallback is preserved, but resource failures propagate
and cannot become a missing optimization. Invalid input errors retain both
reservation owners until dropped. Hint diagnostics log borrowed fields without
building a summary String. URL and FileSlice setup use the native path admission
helpers before their allocations; storage reads have their own admission.

Native allocation observers cover tags and sidecars at 0/7/30/31/64/255 entries,
embedded recursive schema and typed protocol, threshold drops and deep copies.
A denied retained reservation causes zero native parser allocations. Original
schema/action lifetime probes and typed-denial probes accompany the source
inventory. These checks do not certify checkpoint writers, sidecar evaluation,
or arbitrary raw-field extraction outside the owned native interfaces.
