# Library routing for the relational suite

This index routes implementation questions to exact pinned references. Version
pins come from the current FAB dependency section and the resolved manifests,
never from this derived file.

## 1. Data-fabric routing

| Question | Primary reference |
|---|---|
| Arrow arrays, schemas, metadata, builders, kernels, IPC, Parquet | arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md |
| DataFusion catalogs, sessions, expressions, logical/physical plans, providers, functions, statistics, resources | datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md |
| DataFusion/Arrow alignment with v2 principles | datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md plus full_data_fabric_design_principles_v2.md |
| Delta snapshots, exact versions, writes, DML, constraints, optimize/vacuum | deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md |
| Delta durability, transaction, CDF, statistics, recovery, and retention alignment | deltalake_1.0.0_43a0cf10_design_principle_alignment_manual_2026-08-26.md plus full_data_fabric_design_principles_v2.md |
| Graph projection and bounded algorithms | petgraph.md |

## 2. Provider routing

| Provider boundary | Primary reference |
|---|---|
| Tree-sitter CST, queries, error recovery, coordinates | tree_sitter_rust_python.md |
| Ruff lexer/AST/trivia/semantic scopes and bindings | ruff_python_crates_advanced_reference_2026-08-18.md |
| Pyrefly Query/TSP/module resolver/Glean/LSP exact surfaces | pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md |
| rustc public/private MIR, identity, ownership, dataflow, compiler trust | rust_mir_cpg_continuous_reference_2026-08-18.md |

## 3. Lifecycle and serving routing

| Boundary | Primary reference |
|---|---|
| repository/worktree authority and Git acceleration | gix_rust_advanced_reference.md |
| filesystem event loss, debounce, rename, rescan | notify_debouncer_full_rust_reference.md |
| FastMCP server, tools/resources, transport, lifecycle | fastmcp_python_advanced_reference_3.4.7.md |
| Python public contract validation and serialization | pydantic_python_advanced_reference_2.13.4.md |
| gRPC behavior and async transport | grpcio_python_advanced_reference_1.83.0.md |
| Protobuf wire/presence/evolution | protobuf_python_advanced_reference_7.36.0.md |

## 4. Operating rule

Consult the current manifest/lockfile, then the exact reference, then compile
or import the API when load-bearing. A future library change is an explicit
migration. No semantic facade is added merely because an API might change.
