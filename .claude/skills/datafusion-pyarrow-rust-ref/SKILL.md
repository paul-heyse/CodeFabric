---
name: datafusion-pyarrow-rust-ref
description: "Reference navigator for CodeFabric's Rust DataFusion 55.0.0 and Arrow/Parquet 59.2.0 stack. Routes the comprehensive 2026-08-23 reference for sessions, RecordBatch and schema APIs, expressions, plans, TableProvider/ScanArgs, statistics, Parquet, object_store, execution, UDFs, and upgrade validation. Use for Rust data-fabric code or Cargo changes touching datafusion, arrow, arrow_*, parquet, or object_store."
allowed-tools: Read, Grep, Glob, Bash
---

# DataFusion 55 + Arrow 59 Rust reference navigator

Use one current authority:

`docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`

The active CodeFabric baseline is DataFusion 55.0.0, Arrow/Parquet 59.2.0, and
`object_store` 0.13.2. Read the exact pins from FAB §2.1 and the resolved graph; do not infer
them from examples. The document's source-verified §40A and Part III V1–V5 upgrade gate take
precedence over stale illustrative version strings in imported deep dives.

## Routing

- Dependency selection, features, MSRV, and version alignment: §1, §34, §40A, Part III V1–V2.
- Arrow `Schema`, `Field`, arrays, `RecordBatch`, IPC, Parquet, and canonical bytes: §4,
  §7, §13–§15, schema chapters S1–S15, Part III V3.
- Sessions, SQL/DataFrame/Expr, configuration, memory, cancellation, and metrics: §3,
  §5–§12, §27–§30, Part III V3–V5.
- `TableProvider`, `scan`, `scan_with_args`, `ScanArgs`, pushdown, and statistics: §18,
  planning §47 and §51, §40A, Part III V3–V4.
- Logical/physical planning and optimizer behavior: §19–§23, planning §41–§56.
- UDF/UDAF and calculation placement: §24 and C1–C13.

Use [REFERENCE.md](REFERENCE.md) for the compact task matrix and CodeFabric-specific
invariants. Use `just lib-outline <path> --view names` before reading a large chapter.

## CodeFabric constraints

- Preserve one Arrow/Parquet/DataFusion public type universe across the stable root and the
  extractor IPC boundary.
- `RowConverter` output participates in durable application checksums. Never refresh a checksum
  merely because a library version changed.
- A query pins one immutable snapshot and exact Delta versions. Plan text and metrics are
  diagnostics, not semantic identity.
- The two application-owned `TableProvider` wrappers must make DataFusion 55 structured scan and
  statistics behavior explicit.
- Python/FastMCP remains presentation-only; do not introduce Python Arrow/DataFusion processing.

Rust Delta APIs, Delta protocol/state, and mutation/maintenance behavior route to the sibling
`deltalake-rust-ref` skill.

## Historical references

`datafusion_rust.md`, `datafusion_planning_rust.md`, `datafusion_schemas_rust.md`,
`datafusion_calculations_rust.md`, and `pyarrow_rust.md` document the predecessor stack. Read them
only for an explicitly historical comparison; they are not API authorities for current work.
