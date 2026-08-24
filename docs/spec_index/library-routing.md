# Library routing

Which library reference chapter covers the functionality a given spec section depends on, and
which skill routes you there.

Each spec carries a document-level `## 2. Source basis` table — `GEN §2`, `FAB §2`, `LIFE §2`,
`SRV §2`. Those name the reference; they do not say *which chapter*. This file does.

`ONT` and `QRY` have no source-basis table at all: they are deliberately library-agnostic.
Their few dependency-bearing sections are in §7 below.

See [`README.md §2`](./README.md#2-citation-convention) for spec tags.

## 1. Reference shorthands

| Short | Document (under `docs/library_ref/`) | Chapter scheme | Skill |
|---|---|---|---|
| `ts` | `tree_sitter_rust_python.md` | §0–§45 | `code-facts-lib-ref` |
| `ruff` | `ruff_python_crates_advanced_reference_2026-08-18.md` | §0–§25 | `code-facts-lib-ref` |
| `pyrefly` | `pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md` | §0–§38 + §15A + App. A–E | `code-facts-lib-ref` |
| `mir` | `rust_mir_cpg_continuous_reference_2026-08-18.md` | §0–§58 + App. A–R | `code-facts-lib-ref` |
| `pg` | `petgraph.md` | §0, §2–§30 | `petgraph-ref` |
| `df` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | §0–§40A and §40 | `datafusion-pyarrow-rust-ref` |
| `df-plan` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | §41–§56 | `datafusion-pyarrow-rust-ref` |
| `df-schema` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | S1–S15 | `datafusion-pyarrow-rust-ref` |
| `df-calc` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | C1–C13 | `datafusion-pyarrow-rust-ref` |
| `arrow` | `datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md` | Arrow/Parquet integration throughout §0–§40A, S1–S15, and C1–C13 | `datafusion-pyarrow-rust-ref` |
| `delta` | `deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md` | §0, §2–§13 (no §1) | `deltalake-rust-ref` |
| `gix` | `gix_rust_advanced_reference.md` | §0–§50 + §1A + App. A–F | `gix-notify-ref` |
| `notify` | `notify_debouncer_full_rust_reference.md` | §0–§40 | `gix-notify-ref` |
| `fastmcp` | `fastmcp_python_advanced_reference_3.4.7.md` | §0–§37 | `fastmcp-pydantic-ref` |
| `pydantic` | `pydantic_python_advanced_reference_2.13.4.md` | §0–§51 | `fastmcp-pydantic-ref` |
| `canon` | `README_canonicalization_library_reference_pack.md` and its version-pinned linked references | per-library sections | `canonicalization-lib-ref` |
| `grpcio` | `grpcio_python_advanced_reference_1.83.0.md` | §0–§40 | `grpcio-orjson-protobuf-ref` |
| `protobuf` | `protobuf_python_advanced_reference_7.36.0.md` | §0–§44 | `grpcio-orjson-protobuf-ref` |
| `orjson` | `orjson_python_advanced_reference_3.12.0.md` | §0–§33 | `grpcio-orjson-protobuf-ref` |
| `conc` | `rust_parallel_concurrency_stack_reference_2026-08-19.md` | §0–§51 | **none — unrouted** |
| `tooling` | `rust_development_environment_tooling_agent_reference_2026-08-19.md` | §0–§68 | **none — unrouted** |
| `ast-grep` | `ast-grep_0.45.1_advanced_reference.md` | §0–§25 | `ast-grep-ripgrep-ref` |
| `ripgrep` | `ripgrep_advanced_reference_15.2.0_pcre2_10.47.md` | §0–§49 | `ast-grep-ripgrep-ref` |

The skills already carry dense per-document section indexes with line numbers in their
`REFERENCE.md`. This file routes *into* them; it does not duplicate them.

The last two rows are agent tooling rather than dependencies: no spec section cites them, so
they appear in this table and nowhere else in this file.

The target DataFusion/Arrow document consolidates the former engine, planning, schema,
calculation, and Arrow routes. Its source-verified §40A and Part III upgrade gate take precedence
over stale illustrative version strings in its imported deep dives. Navigate it with
`just lib-outline <ref>.md --match '^30\)' --view expanded`.

## 2. `LIFE` — watcher, Git state, scheduling

The heaviest dependency surface in the suite: 199 gix mentions and the whole of Part V.

| LIFE § | Subject | Library |
|---|---|---|
| §2.2 | Version and security stance | `gix` §48 API stability/MSRV/feature drift |
| §27 | Watcher and Git-state roles | `notify` §0 mental model · `gix` §0 mental model |
| §28 | Debounce policy | `notify` §4 debounce timing semantics |
| §29 | Application event facade | `notify` §3 core type system · §10 event taxonomy |
| §30 | Bounded ingress and overflow recovery | `notify` §21 backpressure/burst control · §11 rescan and loss recovery |
| §31 | Dirty registry | `conc` §32–§36 DashMap |
| §33 | Source image capture | `canon` BLAKE3 Rust reference |
| §34 | Source inventory | `gix` §18 directory walking · §20 status |
| §35 | Rename and identity policy | `notify` §6 rename normalization · §7 file-ID caches · `gix` §21 rename tracking |
| §36 | Rescan generation fence | `notify` §11 rescan semantics and loss recovery |
| §37 | New `codefabric-git-state` subsystem | `gix` §45 plumbing-crate catalog · App. B selection guide |
| §38 | Read-only repository policy | `gix` §43 security and governance · §33 push workflow gap |
| §39 | Recommended gix feature profile | `gix` §1 Cargo features · **§1A release-pinned feature flag catalog** |
| §41 | Startup discovery | `gix` §4 discovery/open/init/trust |
| §42 | Bare repositories | `gix` §4 bare repositories |
| §43 · §44 | Canonical path representations and normalization | `gix` §17 pathspecs and path normalization · §42 cross-platform behavior |
| §45 | File identity hierarchy | `gix` §7 object IDs · `notify` §23 path identity and races |
| §46 | Inclusion policy | `gix` §17 pathspecs |
| §47 | Git-aware directory walking | `gix` §18 attributes, ignores, excludes, directory walking |
| §48 | Attribute policy | `gix` §18 worktree stacks |
| §49 | Ignore-policy fingerprint | `gix` §18 excludes |
| §50 · §51 | Operational Git state vector, Git operation state | `gix` §6 repository layout and state · §13 references/HEAD |
| §52 · §53 | Metadata watch set, Git metadata event facade | `notify` §8 watch roots · §19 filtering and scope reduction |
| §54 · §55 | Status as an accelerator, status-based reconcile | `gix` §20 repository status |
| §56 · §57 | Index integration, sparse index | `gix` §16 the Git index |
| §58 | HEAD and tracked baseline | `gix` §13 references and HEAD · §10 tree objects |
| §59 | Branch switch / checkout acceleration | `gix` §21 tree diff · §26 checkout semantics · §20 status |
| §60 | Rename detection policy | `gix` §21 rewrite/rename tracking and diff caches |
| §61 | Mode-only and symlink changes | `gix` §42 symlinks and executable bits |
| §63 · §64 | Blob OID as auxiliary cache key, cache hierarchy | `gix` §8 object database and caching · §40 performance tuning |
| §65 | gix cache configuration | `gix` §40 object cache, pack caches, commit graph |
| §66 · §67 | Submodules, nested repositories | `gix` §24 submodules |
| §68 · §69 | Linked-worktree scheduling, repository handle model | `gix` §25 main and linked worktrees · §3 handle model and thread safety |
| §70 | Execution placement | `conc` §4 executor-selection · §13 Tokio blocking boundary · `gix` §39 blocking/async integration |
| §71 | Parallelism policy | `conc` §16–§24 Rayon · §5 whole-process thread topology · `gix` §39 `parallel` |
| §72 | Cancellation | `gix` §37 interruption and cancellation · `conc` §12 cancellation safety |
| §74 | gix lock/tempfile integration | `gix` §36 locks, tempfiles, atomicity boundaries |
| §77 | Clean/smudge filters | `gix` §19 filter pipelines and external process boundaries |
| §94 | Fast syntax lane | `ts` §10 incremental parsing · §11 changed ranges and invalidation |
| §95 | Python semantic lane | `ruff` §15 incrementality and cache boundaries · `pyrefly` §6 state/transactions/epochs · §23 incremental file-change workflow |
| §96 | Rust semantic lane | `mir` §41 continuous-update architecture · §43 rustc incremental reuse · §46 failure isolation and stale-state policy |
| §97 · §98 | Owner-local and interprocedural derived lanes | `pg` §13 traversal system · §16 connectivity/components · `df` §26 custom operators |
| §105 | Overlay-aware DataFusion providers | `df` §17 catalogs/schemas/tables · §18 custom `TableProvider` |
| §107 | Durable overlay flush | `delta` §5 writing from Arrow and DataFusion |
| §109 | Runtime responsibility split | `conc` §6–§15 Tokio · §16–§24 Rayon · §25–§31 Crossbeam · §32–§36 DashMap · §37–§38 tokio-rayon |
| §110 | Actor-owned coordinator | `conc` §10 Tokio channels · §42 actor and staged-compute patterns |
| §112 · §113 | Admission control, thread-budget policy | `conc` §40 CPU admission control and overload · §5 oversubscription budget |
| §114 | Supersession and cancellation | `conc` §43 errors, panics, cancellation, graceful shutdown · `gix` §37 interruption |
| §115 | Backpressure policy | `conc` §40 backpressure · `notify` §21 burst control |
| §116 | Cache policy | `gix` §40 caches · `mir` §43 query reuse |
| §130 · §131 | Operational-state schemas and backing store | *(SQLite — no reference, no crate named)* |
| §151 · §152 · §153 | Shutdown ordering, drain policy, cancel policy | `conc` §43 errors, panics, cancellation, graceful shutdown · `notify` §30 shutdown and resource lifecycle |
| §145–§147 | gix conformance suite, CLI parity oracle, upgrade gate | `gix` §41 fixtures and Git parity · §49 CLI as debugging tool · §48 upgrade migration |
| §155 | Recommended crates | every reference's chapter 1 (installation and crate selection) |
| App. E | Read-only gix dependency profile | `gix` §1A feature flag catalog · App. D Cargo recipes |
| `AC-G-27` | Operational-state persistence | *(SQLite — gap)* |

## 3. `FAB` — Arrow, DataFusion, Delta

| FAB § | Subject | Library |
|---|---|---|
| §2 · §2.1 | Source basis, workspace dependency baseline | `delta` §0 version/feature/compatibility baseline · `arrow` §2 Cargo features |
| §3.1 · §3.2 · §3.3 | Technology responsibility model | `arrow` §0 · `df` §0 · `delta` §0 |
| §7 | Canonical physical types and identity | `arrow` §3 data model: types, fields, schemas, metadata |
| §10 · §11 | Schema metadata conventions, schema registry | `arrow` §3 metadata · `df-schema` S1–S15 · `delta` §4 schema and Arrow type mapping |
| §13 | Control-plane schemas and operational-state store | `df` §17 catalogs · *(SQLite — gap)* |
| §63 | Provider-observation to Arrow contract | `arrow` §6 `RecordBatch` and streaming readers · §10 IPC and streams |
| §64 · §65 | Batch-size policy, builder policy | `arrow` §5 arrays, builders, scalars · §4 buffers and zero-copy |
| §66 | Batch validation | `arrow` §3 schemas · `df` §4 data model |
| §67 | Delta table creation | `delta` §8 create-table workflows · §4 metadata governance |
| §68 · §69 | Table mutation classes, owner replacement protocol | `delta` §9 DML delete/update/merge · §5 writing from Arrow |
| §70 | Idempotency and retry | `delta` §9 conflict and retry · §5 idempotency |
| §71 | Durable publication and active-snapshot algorithm | `delta` §3 table loading, snapshots, versions · §5 write |
| §12.5–§12.9 | Delta engine snapshot boundary · snapshot-scoped provider set · ephemeral caches · checkpoint identity · validation vs construction | `delta` §3 snapshots and state · §7.1 `TableProvider` integration · §6 `DeltaScanConfig` |
| §75 | Integrity validation queries | `df` §5 SQL API · §11 expression API |
| §77 | Arrow kernel catalog | `arrow` §7 compute kernels · §8 compute expressions |
| §78 | DataFusion scalar UDFs | `df` §24 user-defined functions · `df-calc` C1 placement decision tree |
| §79 | DataFusion aggregate UDFs | `df` §24 UDAF · `df-calc` C1–C3 |
| §79A | Derivation registry and single-authority matrix | `df-calc` C1.7 multi-axis decision matrix · C1.12 calculation manifest |
| §80 | Relationally expressible derived facts | `df` §22 query optimizer · §23 join algorithms |
| §81 | Custom logical operators | `df` §19 logical plans · §26 custom logical and physical operators |
| §82 | Custom physical graph representation | `arrow` §4 buffers and zero-copy — CSR over Arrow buffers, **not** petgraph |
| §83–§89 | Reachability · SCC · dominators · control dependence · reaching definitions/liveness · points-to · summary fixed point | `df` §20 physical plans and execution operators · §21 streaming execution · `df-plan` §41–§60 |
| §90 | Custom-operator execution requirements | `df` §20 `ExecutionPlan` · §28 memory management and spilling |
| §91 | `ServingSnapshot`-pinned overlay-aware catalog provider | `df` §17 catalogs · §18 custom `TableProvider` · `delta` §6 reading through DataFusion · §7.1 `TableProvider` integration |
| §92 · §93 | Stable serving views, table functions | `df` §17 tables · §24 UDTF · `delta` §7.3 SQL API path |
| §94 | Query-planning policy | `df` §22 optimizer · `df-plan` planning track |
| §95 · §96 | Partitioning policy, Z-order and clustering | `delta` §12 partitioning, layout, file skipping · §13 Z-order |
| §97 | Parquet writer policy | `arrow` §11 Parquet core · §12 advanced Parquet |
| §98 | DataFusion runtime policy | `df` §27 configuration system · §28 memory and spilling |
| §98.1–§98.3 | Delta provider access profiles · query-serving statistics profile · warm-up policy | `delta` §6 reading and querying, `DeltaScanConfig` · §7.8 file skipping, pruning, performance |
| §100 · §101 | Compaction thresholds, vacuum policy | `delta` §13 optimize, compaction, vacuum |
| §100.1 | Nested-schema optimize obligations | `delta` §13 optimize · §4 Delta↔Arrow schema mapping · §12 partitioning and layout |
| §101.1 | Delta action paths are opaque URI identities | `delta` §2 storage options and paths · §12 file skipping |
| §102 | Delta constraints | `delta` §11 constraints, properties, governance |
| §103 | Schema compatibility policy | `delta` §4 schema mapping · `arrow` §3 schema metadata |
| §103.4 | Physical nested-schema adaptation | `delta` §4 Delta↔Arrow schema mapping and metadata governance · §6 read-path schema adaptation |
| §108 · §109 | Schema migration, maintenance workflow | `delta` §11 protocol governance · §13 maintenance |
| §112 | Testing strategy | `delta` §7.12 agent checklist · `df` §32 testing and correctness |
| §112.6 | delta-rs upgrade gate | `delta` §0 baseline, feature-flag matrix, **API stability zones** · §11 protocol governance |
| §113 | Recommended crates | `arrow` §1 crate topology · `df` §1 crate selection · `delta` §2 deployment and project setup |
| `AC-G-20` | Hot-overlay physical schemas | `arrow` §3 · §5 builders |
| `AC-G-23` | Snapshot leases and Delta vacuum | `delta` §13 vacuum · §3 time travel |

## 4. `GEN` — providers

| GEN § | Subject | Library |
|---|---|---|
| §2 | Source basis and version anchors | each provider reference's chapter 0 |
| §5.1 · §5.2 | Python and Rust authority order | `pyrefly` §18 Ruff + Pyrefly division of responsibility · `mir` §4 `rustc_public` vs `rustc_private` |
| §7.1 | Tree-sitter isolation | `ts` §34 raw FFI and ownership boundaries · §7 `Node` is a borrowed view |
| §7.2 | Ruff isolation | `ruff` §11 cross-crate ownership and lifetime model |
| §7.3 | Pyrefly isolation | `pyrefly` §21 stable semantic-sidecar protocol design · App. B application-owned DTO schema |
| §7.4 | rustc isolation | `mir` §4 extraction surface · §55 reference extractor/daemon architecture |
| §7.5 | petgraph isolation | `pg` §11 indexing, identity, mutation safety |
| §8 · §9 | Immutable source-image contract, canonical source coordinates | `ruff` §3 source coordinates and newline semantics · `ts` §6 text input and encodings |
| §11 · §12 | Two-stage batch and Arrow boundary, raw/normalized preservation | `arrow` §10 IPC and streams · `ts` §22 static node types as CST schema |
| §15 | Python source and lexical generation | `ruff` §4 lexer, parser, tokens · §6 `ruff_python_trivia` |
| §16 | Python syntax generation | `ruff` §5 `ruff_python_ast` · `ts` **§45 parsing Python** · §20 error recovery |
| §17 | Python semantic-entity generation | `ruff` §8 `ruff_python_semantic` |
| §18 | Python scope and binding generation | `ruff` §8 scopes, bindings, references |
| §19 | Python module/import/export generation | `ruff` §8 import semantics · `pyrefly` §26 imports, modules, exports, re-exports |
| §20 | Python type generation | `pyrefly` §10 whole-file deduplicated type-table extraction · §7 type model · §8 narrowing |
| §21 | Python object/member generation | `pyrefly` §12 class attributes, properties, member semantics · §27 inheritance and MRO |
| §22 | Python callable-contract generation | `ruff` §5 typed AST · `pyrefly` §7 type model |
| §23 | Python call-site and dispatch generation | `pyrefly` §11 type-aware call/callee extraction · §13 qualified targets and subtype queries |
| §24 | Python CFG generation | `ruff` §5 typed AST traversal — CodeFabric owns the CFG builder |
| §25 · §26 | Python value/dataflow, memory/alias generation | `pyrefly` §8 flow-sensitive inference · §28 dynamic Python and uncertainty |
| §33 | Python explicit-unknown generation | `pyrefly` §28 uncertainty modeling · §17 what a complete Python CPG can and cannot mean |
| §35 | Rust source and lexical generation | `ts` §45-analogue for Rust; `mir` §17 source spans and anchoring |
| §36 | Rust semantic-definition generation | `mir` §7 crate and item discovery · §16 types, generics, traits |
| §37 | Rust type and generic generation | `mir` §16 type normalization · §19 generic vs monomorphized MIR |
| §38 | Rust MIR-body generation | `mir` §8 `Body` anatomy · §9 locals and debug variables · §18 visitor APIs |
| §39 | Rust CFG generation | `mir` §10 basic blocks and CFG · §12 terminators, unwind edges · §25 mapping MIR CFG into a CPG |
| §40 | Rust place, memory, access-event generation | `mir` §14 places and projections · §28 place abstraction and access paths |
| §41 | Rust call and instance generation | `mir` §20 `Instance` resolution · §21 direct call edges · §22 fn pointers, closures, indirect calls |
| §42 | Rust trait and dynamic-dispatch generation | `mir` §23 trait dispatch, vtables, unsizing |
| §43 | Rust macro and generated-code generation | `mir` §17 macro expansion and source anchoring |
| §44 | Rust move/initialization/ownership-state generation | `mir` §29 move paths and initialization · §27 borrows and ownership edges |
| §44.3 | Exact borrowck facts (`rustc_private` escape hatch) | `mir` §4 extraction-surface choice · §51 nightly/API upgrade gates · `tooling` `rustc-dev` chapter |
| §45 · §46 | Rust def-use/liveness, alias/points-to generation | `mir` §30 reaching definitions and SSA-like overlays · §28 alias domains |
| §47 | Rust drop and resource generation | `mir` §24 drop glue, shims, intrinsics |
| §48 | Rust async/coroutine generation | `mir` §32 closures, captures, async, coroutines |
| §49 | Rust constants/statics/CTFE generation | `mir` §33 constants, statics, CTFE |
| §50 | Rust unsafe/FFI/inline-assembly generation | `mir` §34 unsafe operations, FFI, inline asm, trust boundaries |
| §52 | Petgraph role | `pg` §2 graph type decision guide · §4 `Graph` · §5 `StableGraph` · §8 `Csr` |
| §53 | Projection construction | `pg` §12 construction patterns and graph loading · §10 weights as domain data |
| §54 | Reachability generation | `pg` §13 traversal system · §16 connectivity |
| §55 | SCC and recursion | `pg` §16 components, cycles, DAGs |
| §56 · §57 | Dominance, control dependence | `pg` §20 graph analytics and specialized routines |
| §58 | Loop generation | `pg` §16 cycles · §20 analytics |
| §59 · §60 | Reaching definitions, liveness | `pg` §13 visitors and walkers — the worklist engine is application-owned |
| §61 | Points-to and alias analysis | `pg` §14 trait-based graph abstraction |
| §62 · §63 · §64 | Shortest distance, connected components, transitive reduction/closure | `pg` §15 shortest paths · §16 connectivity · §20 analytics |
| §66 | Interprocedural summary generation | `pg` §16 SCC and condensation · `mir` §35 interprocedural summaries |
| §89 | Recommended crates | each provider reference's chapter 1 |
| §90 | Provider job interfaces | `conc` §8 Tokio tasks · §13 blocking boundary · §20 Rayon scope |
| §91 | Canonical graph-projection DTO | `pg` §10 weights as domain data · `arrow` §5 arrays and builders |
| §97 | Capability gaps | `pyrefly` §17 completeness boundaries · `mir` §58 capability gaps · `pg` catalog-only chapters |
| `AC-G-30` | Pyrefly sidecar wire protocol | `pyrefly` §21 sidecar protocol · App. A pinned-source sidecar skeleton · `arrow` §10 IPC · `protobuf` descriptors and schema evolution |
| `AC-G-31` | rustc extractor protocol | `mir` §6 compiler-wrapper integration with Cargo · §39 extraction protocol and transaction boundaries · §55 daemon architecture |
| `AC-G-32` | Common asynchronous provider execution interface | `conc` §8 tasks · §12 cancellation safety · §40 admission control |
| `AC-G-33` | Immutable source snapshot transport | `arrow` §10 IPC · `pyrefly` §22 bootstrap/indexing workflow |
| `AC-G-34` | Build and project-configuration discovery | `mir` §5 cargo metadata, packages, targets · `pyrefly` §4 module identity and search paths |
| `AC-G-35` | Provider sandbox and trust model | `mir` §53 security and resource governance · `ts` §42 security and resource governance |
| `AC-G-39` | Derived-analysis precision profiles | `pg` §2 graph type decision guide |

## 5. `SRV` — FastMCP, Pydantic, gRPC

| SRV § | Subject | Library |
|---|---|---|
| §8 · §9 | gRPC over Unix domain socket, Protobuf service | `grpcio` channels/servers/UDS · `protobuf` generated API/descriptors/evolution |
| §18 | Framework and package posture | `fastmcp` §1 installation and dependency policy · `pydantic` §1 version pinning |
| §19 | Pydantic adapter-contract architecture | `pydantic` §4 `BaseModel` · §9 `ConfigDict` · §10 strict mode · §21 `TypeAdapter` |
| §20 | Server construction | `fastmcp` §4 server construction and lifecycle · §2 first executable server |
| §21 | Public MCP component catalog | `fastmcp` §3 core API map and object model |
| §22–§25 | The four public tools | `fastmcp` §5 tool definition and execution contract · §6 typing, validation, outputs, content blocks |
| §26 | Tool annotations | `fastmcp` §15 transforms, visibility, discovery shaping |
| §27 | Resources | `fastmcp` §7 resources and resource templates |
| §28 | Prompts | `fastmcp` §8 prompts and prompt rendering |
| §31 | Deliberate FastMCP exclusions | `fastmcp` §12 background tasks · §16 search transforms, Code Mode, gateways |
| §32 | Query and static-resource caching policy | `fastmcp` §33 performance and large-catalog engineering |
| §33 | Lifespan and immutable settings | `fastmcp` §11 lifespans, session state, state ownership · `pydantic` §38 `pydantic-settings` fundamentals |
| §34 | Dependency injection boundary | `fastmcp` §10 dependency injection · §9 MCP Context |
| §35 | Middleware stack | `fastmcp` §13 middleware and the server policy layer |
| §36 | Internal vs client-visible logging | `fastmcp` §29 observability and operational diagnostics |
| §37 | Validation layers | `pydantic` §5 validation entry points · §40 validation hot paths and `FailFast` |
| §40 | Error registry and Pydantic translation | `pydantic` §36 `ValidationError`, custom errors, locations |
| §41 | No silent fallback or unrestricted serialization | `pydantic` §19 `SerializeAsAny` and external-contract safety · §18 include/exclude semantics |
| §43 | Inline/resource delivery as a discriminated union | `pydantic` §26 unions, discriminators, callable discriminators |
| §44 · §45 | One logical response, result subresources | `fastmcp` §6 outputs and content blocks · §7 resource templates |
| §55 | Settings implementation | `pydantic` §38 source priority · §39 nested env, dotenv, secrets, CLI · §31 `SecretStr` |
| §56 | Public contracts, daemon DTOs, reusable adapters | `pydantic` §21 `TypeAdapter` reuse · §7 fields and `Annotated` |
| §57 | Daemon client interface | `grpcio` §10 `grpc.aio` · deadlines/status/metadata/channel lifecycle |
| §58 | Primary tool implementation pattern | `fastmcp` §5 tools · §9 Context · canonicalization pack for RFC 8785 request bytes |
| §59 | Result resources, status, and references | `fastmcp` §7 resources |
| §60 | Schema generation and STDIO launch | `fastmcp` §27 project configuration and `fastmcp.json` · §28 CLI · `pydantic` §34 validation-vs-serialization schemas |
| §61 | Security model | `fastmcp` §32 security hardening and governance · `pydantic` §49 serialization exposure and trust boundaries |
| §62 | Threat and misuse controls | `fastmcp` §18 advanced security policy |
| §63–§65 | Trace topology, adapter and daemon metrics | `fastmcp` §29 telemetry and inspection |
| §68 | Test layers | `fastmcp` §30 testing, contract verification, tool fingerprinting · §21 programmatic client |
| §70 | Contract fingerprinting | `fastmcp` §30 fingerprinting · `pydantic` §48 schema snapshots and compatibility contracts |
| §73–§76 | Process ownership, startup, shutdown, multiple agents | `fastmcp` §19 running and deploying servers · §34 production architecture patterns |
| §77 · §78 | Dependency and daemon upgrade policy | `fastmcp` §35 upgrade discipline · §36 FastMCP 4 prerelease · `pydantic` §46 2.12→2.13.4 delta · §47 2.14 prerelease |
| `AC-G-58` | Complete Protobuf service and state machine | `grpcio` RPC cardinalities/streaming/deadlines/status · `protobuf` generated API/descriptors/evolution |
| `AC-G-63` | Immutable result artifact store | `canon` BLAKE3 references · *(SQLite metadata gap)* |

## 6. `RM` — the roadmap

| RM § | Subject | Library |
|---|---|---|
| §5 W0 WP1 | Stable Rust daemon/data-plane domain | `delta` §0 compatibility baseline · `arrow` §2 Cargo features · `tooling` cargo and workspace chapters |
| §5 W0 WP2 | Nightly rustc extractor domain | `mir` §1 toolchain installation and compiler-library setup · §51 nightly upgrade gates · `tooling` `rustc-dev` chapter |
| §5 W0 WP3 | Pyrefly sidecar domain | `pyrefly` §1 installation, build, deployment surfaces · §2 workspace and crate architecture |
| §5 W0 WP4 | Python FastMCP adapter domain | `fastmcp` §1 · `pydantic` §1 |
| §5 W0 WP5 | Repository command and CI contract | `tooling` nextest, insta, and CI chapters |

## 7. `ONT` and `QRY` — the library-agnostic specs

Neither has a source-basis table. Their dependency-bearing sections are few and mostly concern
generated artifacts rather than runtime APIs.

| Spec § | Subject | Library |
|---|---|---|
| `ONT §57` · `ONT AC-G-17` | Rust unsafe/FFI ontology, cross-language FFI linking | PyO3 and Maturin appear as **analysis subjects**, not dependencies — `arrow` §21 PyO3 interop and `tooling` §Maturin cover the mechanics if you need to model expansions |
| `ONT §64` · `AC-G-12` · `AC-G-13` | Identity and public encoding rules | `canon` BLAKE3 Rust/Python references |
| `ONT AC-G-70` · `AC-G-71` | Machine ontology registry, property schema | generates Arrow fields, Delta constraints and Pydantic schemas — `arrow` §3 · `delta` §11 · `pydantic` §34 |
| `QRY AC-G-46` | Typed internal `PlanSpec` | `df` §19 logical plans · `df-plan` planning track |
| `QRY AC-G-52` | Query cost model, defaults, hard limits | `df` §22 optimizer · §28 memory management and spilling |
| `QRY AC-G-53` | Canonical JSON and checksum contract | canonicalization pack (`rfc8785`, `serde_json_canonicalizer`, BLAKE3, strict ingress) |
| `QRY AC-G-56` | Streaming, chunk interning, resumability | `df` §21 streaming execution · `grpcio` streaming/flow control/cancellation |
| `QRY AC-G-57` | Query plan cache contract | `df` §22 optimizer · `df-plan` plan caching |

## 8. Version-pin ledger

Pins as the specs state them. Where a spec names a technology without a crate, that is recorded
too — it is a decision still open.

| Library | Pin | Stated at |
|---|---|---|
| Arrow Rust | `=59.2.0` family (`arrow`, `-array`, `-buffer`, `-schema`, `-cast`, `-select`, `-ord`, `-string`, `-row`) | `FAB §2` · `FAB §2.1` |
| Parquet Rust | `=59.2.0`, features `["arrow","async","object_store"]` | `FAB §2.1` |
| DataFusion | `=55.0.0` | `FAB §2` |
| `deltalake` / delta-rs | `1.0.0` at git rev `43a0cf10a313e5077c48637ad786a05359136bbb` — **a pinned pre-release revision, not a tagged stable release**; `default-features=false`, features `["rustls","datafusion"]`. Object-store backends are feature-gated, not default | `FAB §2` · `FAB §2.1` |
| `object_store` | `=0.13.2` | `FAB §2.1` |
| Rust toolchain | `rust-version = "1.95.0"`, `edition = "2024"`, `resolver = "3"` — floor set by the Ruff 0.0.7 provider train above delta-rs's 1.94.1 minimum | `FAB §2` · `FAB §2.1` |
| Delta kernel | `buoyant_kernel 0.25.1` + `buoyant_kernel_engine 0.25.0`, `arrow-59`, **selected transitively**; CodeFabric does not declare it directly | `FAB §2` · `FAB §2.2` |
| tree-sitter | `0.26.12`; `tree-sitter-python 0.25.0` | `GEN §2` |
| Ruff | `0.16.1`; component crates `0.0.7` | `GEN §2` |
| Pyrefly | `1.2.0` | `GEN §2` |
| `rustc_public` | `1.100.0-nightly` on nightly `2026-08-18` | `GEN §2` |
| petgraph | `0.8.3` | `GEN §2` |
| gix | `=0.86.0` — floor is security-driven; `<=0.85.0` rejected | `LIFE §2.2` · `LIFE §39` |
| `notify-debouncer-full` | **no version pin stated** — only a 75 ms debounce timeout budget in `LIFE` Appendix B | `LIFE §2` · `LIFE §28` |
| fastmcp | `== 3.4.7` (exact) | `SRV §18` |
| pydantic | `== 2.13.4` (exact); `pydantic-core` must **not** be pinned separately | `SRV §18` |
| pydantic-settings | `== 2.15.0` (exact) | `SRV §18` |
| tokio | `"1"`, features `["rt-multi-thread","macros","sync","time"]` | `FAB §2.1` |
| serde / serde_json / futures / tracing / url / blake3 | `"1"` / `"1"` / `"0.3"` / `"0.1"` / `"2"` / `"1"` | `FAB §2.1` |
| zstd | codec choice, not a direct dependency — Parquet compression and RPC payload compression; covered by `arrow` §11–§12 | `FAB §97` Parquet writer policy · `SRV AC-G-58` |
| rayon · crossbeam · dashmap · tokio-rayon | named, **unpinned** | `LIFE §109` · `LIFE §155` |
| gRPC / Protobuf | Python `grpcio==1.83.0`, `protobuf==7.36.0`; build-only `grpcio-tools==1.83.0`; Rust consumes the same descriptor IR through pinned Prost/Tonic generation APIs | `SRV §18` · `SUITE AC-G-05` |
| SQLite | embedded, WAL mode; **no crate named** — `LIFE AC-G-27` only excludes RocksDB, redb, and independent append journals | `FAB §13` · `LIFE §130` · `LIFE AC-G-27` |

`FAB §2.2` states the version-alignment invariant these pins serve: one Arrow major/minor
family, one matching Parquet family, one DataFusion family, one `object_store` family, one
pinned delta-rs revision — and CI rejects duplicates that cross public type boundaries.

`SUITE AC-G-02` requires a normative version and separate semantic/source identities for
every artifact; `SUITE AC-G-03` requires a fail-fast compatibility matrix. Remaining
unnamed implementation crates must be closed by the owning wave before adoption.

## 9. Grep hazards in this corpus

- `rg -i arrow` over `ONT`/`QRY` is roughly 80% false positives — `NARROWS_TO`, "narrowing".
  Use `rg -w 'Arrow'`.
- `rg -i chrono` matches "asynchronous" and "synchronous". `chrono` has **zero** real mentions.
- `rg -i tonic` is dominated by false-positive "monotonic" matches. Use `rg -w 'Tonic|tonic'`
  for the intentional Prost/Tonic descriptor-generation references.
- `docs/library_ref/` is ~9 MB of prose. Scope searches with `-g '!docs/library_ref/**'` unless
  that is what you are searching.

## 10. What has no coverage

Summarized here, detailed in [`README.md §7.4`](./README.md#74-library-coverage-gaps):

- **No reference document**: embedded SQLite.
- **Reference exists, no skill routes it**: `conc`
  (`rust_parallel_concurrency_stack_reference_2026-08-19.md`) and `tooling`
  (`rust_development_environment_tooling_agent_reference_2026-08-19.md`). Between them they
  carry `LIFE §70`–`§71`, `§109`–`§116`, `§151`–`§153`, `GEN AC-G-32`, `GEN §44.3`, and all of
  `RM W0` — a substantial surface reachable only by opening the files directly.
