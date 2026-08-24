# DataFusion 55 + Arrow 59 task map

Current authority:
`docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`.

| Task | Read first | Proof emphasis |
|---|---|---|
| Pin or resolve the stack | §1, §34, §40A, V1–V2 | exact 55.0.0/59.2.0 family, source and feature graph |
| Build or decode Arrow IPC | §4, §14, schema S1–S7, V3 | schema, nullability, values, deterministic bytes |
| Canonicalize rows/batches | §4, §32, V3 | pre-upgrade KATs; no silent `RowConverter` drift |
| Implement a table provider | §18, planning §47/§51, V3–V4 | projection, filter, limit, `ScanArgs`, statistics disposition |
| Configure Parquet reads | §15, §27–§29, V4–V5 | explicit pushdown posture, pruning, resource limits |
| Compare query behavior | §19–§23, §30, §32, V4–V5 | ordered rows/schema/checksum before plan diagnostics |
| Add a calculation | §24, C1–C13 | prefer expressions/built-ins; justify custom state and null policy |
| Diagnose an upgrade | §33–§35, §40A, Part III | exact-source compiler/runtime probe, not illustrative prose |

## Operating rules

1. Match Arrow 59.2.0 across every crate that exchanges Arrow types; Parquet is 59.2.0 and
   DataFusion is 55.0.0.
2. Treat `DataFrame` as lazy and bound terminal materialization and memory use.
3. Distinguish plan-side `DFSchema` from runtime Arrow `Schema` and preserve field metadata and
   nullability at boundaries.
4. Use typed `Expr` construction; never splice untrusted strings into predicates.
5. Make provider pushdown and statistics behavior executable. DataFusion default methods are not
   evidence that wrapper semantics were preserved.
6. Compare result meaning before optimizer/operator names. Rebaseline plan diagnostics only after
   rows, schema, checksum, snapshot, pruning intent, cancellation, and resource ceilings pass.
7. The FastMCP adapter has no Arrow/DataFusion processing role.

## CodeFabric source map

- `src/fabric/snapshot_catalog.rs`: identity provider wrapper and exact provider caching.
- `src/fabric/overlay.rs`: effective overlay provider and explicit statistics disposition.
- `src/fabric/serving.rs`: query evidence, pruning, resources, cancellation, plan diagnostics.
- `src/fabric/mutation.rs` and `publication.rs`: durable checksum consumers.
- `rustc-extractor/src/wrapper.rs`: Arrow IPC producer boundary.
- `scripts/stable_graph_check.sh`: exact family/source/feature enforcement.

For Delta snapshots, writes, transactions, protocol features, checkpoints, optimize, or vacuum,
use `deltalake-rust-ref`.
