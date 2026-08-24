---
name: deltalake-rust-ref
description: "Reference navigator for CodeFabric's Rust deltalake 1.0.0 exact revision 43a0cf10 on DataFusion 55.0.0 and Arrow/Parquet 59.2.0. Use for Delta table loading, exact versions, schema adaptation, TableProvider construction, writes and DML, commit properties/application transactions, checkpoints, protocol features, pruning, optimize, and vacuum."
allowed-tools: Read, Grep, Glob, Bash
---

# delta-rs 43a0cf10 reference navigator

Use one current authority:

`docs/library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md`

The exact source is commit `43a0cf10a313e5077c48637ad786a05359136bbb`; a branch name,
declared crate version, or nearby `main` is not interchangeable. The aligned CodeFabric stack is
DataFusion 55.0.0, Arrow/Parquet 59.2.0, and `object_store` 0.13.2. Where the document's alignment
matrix says Arrow 58, treat it as a known typo: the target banner, Cargo snippets, exact source,
and FAB §2.1 are authoritative.

## Chapter routing

- §0: exact version, features, compatibility, API stability zones.
- §2: deployment, local/cloud storage, TLS, URLs, and feature posture.
- §3: loading, snapshots, versions, time travel, lazy/eager state, freshness, checkpoints.
- §4: Delta↔Arrow schema mapping and metadata governance.
- §5: Arrow/DataFusion writes, save/schema modes, commit properties, idempotency.
- §6–§7: DataFusion providers, sessions, structured scans, statistics, pruning, IPC/batches.
- §8: create-table and Parquet-conversion workflows.
- §9: delete/update/merge, transactions, conflicts, retries, metrics.
- §10–§11: CDF, constraints, table properties, protocol and feature governance.
- §12–§13: layout, optimize, checkpoints, retention, vacuum, restore.

Use `just lib-outline <path> --view names` before opening a large chapter.

## CodeFabric constraints

1. Keep `CommitProperties::with_max_retries(0)`: CodeFabric owns coordination, retries,
   application transactions, predecessor checks, and unknown-outcome reconciliation.
2. A `DeltaTable`/provider is a pinned snapshot. Every query binds exact table versions and never
   mixes publications.
3. Read Delta through delta-rs schema and physical adaptation; never bypass the log with raw
   Parquet reads.
4. Query-serving handles retain full statistics. Metadata-only and `without_files` profiles are
   separate deliberate constructions.
5. Reopen validation fails closed on unapproved CDF, deletion vectors, type widening, protocol,
   or table features.
6. Local workstation authority excludes `deltalake-aws` and AWS SDK packages; only `s3-storage`
   activates that implementation. Kernel-forced latent `object_store` cloud features are reported,
   not mistaken for runtime authority.
7. Vacuum is destructive to retained versions. Dry-run and reference/lease safety remain
   mandatory; this skill does not authorize production vacuum orchestration.

The predecessor document
`deltalake_rust_1.0.0_9f922319_advanced_reference_2026-08-20.md` is historical comparison
material only and is not a current API authority.

DataFusion/Arrow APIs that are not Delta-specific route to `datafusion-pyarrow-rust-ref`.
