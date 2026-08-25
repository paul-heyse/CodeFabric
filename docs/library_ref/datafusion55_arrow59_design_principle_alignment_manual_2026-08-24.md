# DataFusion 55 + Arrow 59 Design-Principle Alignment Manual

## Agent workflow for a model-first, contract-driven, authority-centered, provenance-native data fabric

**Version baseline:** Apache DataFusion `55.0.0`; Apache Arrow Rust / Parquet `59.2.0`; `object_store` `0.13.2` where DataFusion storage integration is relevant.

**Primary audience:** LLM programming agents and human reviewers translating high-level requirements into a coherent architecture that makes disciplined use of DataFusion and Arrow rather than merely compiling against them.

**Source design constitution:** *Model-First, Contract-Driven, Provenance-Native Data Fabric*. Its governing objective is that meaning, authority, state, execution, and history remain explicit; important semantics are modeled once, compiled through controlled lifecycle phases, executed over a common data fabric, and preserved through inspectable lineage.

---

## 0. Purpose and scope

This document is a **design-alignment manual**, not a general API encyclopedia. The companion DataFusion 55 and Arrow 59 references remain the syntax and capability sources. This manual answers a different question:

> Given a high-level requirement, how should an agent use Arrow and DataFusion so that the resulting design advances the stated architectural principles rather than merely producing working code?

It therefore maps each design principle to:

1. the relevant Arrow and DataFusion abstractions;
2. the correct way to use those abstractions;
3. application-owned responsibilities that the libraries do not supply automatically;
4. optimizer, schema, state, governance, provenance, interoperability, and testing consequences;
5. evidence an agent must produce before implementation is considered aligned.

The document deliberately avoids equating “library feature exists” with “architectural principle is satisfied.” For example, Arrow metadata can carry a provenance reference, but it is not a provenance system; a DataFusion catalog can be an authority boundary, but it is not automatically a durable governed catalog; `datafusion-proto` can serialize plans, but its bytes are not a stable cross-version semantic fingerprint.

### 0.1 Capability-status legend

Every recommendation should be interpreted through the following status classes.

| Status | Meaning | Agent implication |
| --- | --- | --- |
| **NATIVE MODEL** | Arrow or DataFusion directly represents the concept as a typed object. | Use the native object as the compiled/runtime representation; do not recreate it as an application DTO without a demonstrated need. |
| **NATIVE ENFORCEMENT** | The library validates or enforces the property in the relevant phase. | Depend on the enforcement only within its documented scope and add boundary tests. |
| **EXTENSION CONTRACT** | The library supplies a trait, registry, hook, or planner interface, but the application owns the implementation. | Document invariants, legal variation, capability truthfulness, lifecycle, and tests for the implementation. |
| **COMPOSITION PATTERN** | The principle is achieved by composing native features in a disciplined way. | Preserve the prescribed boundaries and do not collapse the pattern into ad hoc procedural logic. |
| **APPLICATION OVERLAY** | Arrow/DataFusion provide useful artifacts but not the complete capability. | Build an explicit application-owned model, registry, artifact store, policy layer, or provenance system and link it to native objects. |
| **CAUTION** | The feature is advisory, version-coupled, optimizer-sensitive, or not a stable authority. | Do not treat it as stronger than documented; record uncertainty and test assumptions. |

### 0.2 What Arrow and DataFusion should be authoritative for

Within the combined architecture, use the libraries as authorities only for the concepts they actually own:

- **Arrow** should be the canonical runtime representation of typed columnar data: `DataType`, `Field`, `Schema`, arrays, buffers, `RecordBatch`, readers, streams, and standard interoperability protocols.
- **DataFusion logical objects** should be the canonical compiled representation of relational meaning inside the query subsystem: `Expr`, `DFSchema`, and `LogicalPlan`.
- **DataFusion physical objects** should be the canonical compiled representation of a selected execution strategy: `PhysicalExpr`, `ExecutionPlan`, `PlanProperties`, physical partitioning, ordering, resource behavior, and operator metrics.
- **Catalog/provider traits** should be the canonical contract by which consumers discover and access tables and by which backend variability is localized.
- **Session/runtime objects** should own scoped planning and execution state, not global application semantics.

Application domain meaning—such as a `CalculationSpec`, `SchemaContract`, `PlanSpec`, policy set, provenance graph, semantic version, or stable fingerprint—normally remains an application-owned authority that compiles to or references the native objects above.

### 0.3 Non-goals and explicit gaps

The following are not supplied as complete built-in systems by Arrow/DataFusion and must not be implied by metadata or naming conventions:

- a durable semantic-model registry;
- a governed enterprise catalog with durable identity and workflow;
- a complete authorization or tenancy system;
- a stable cross-version logical-plan fingerprint;
- a provenance graph or lineage catalog;
- transaction history and durable table versions;
- a universal schema-evolution policy;
- a reproducible build/environment registry;
- automatic enforcement of arbitrary Arrow field/schema metadata.

DataFusion and Arrow supply the compiler IRs, runtime contracts, extension points, data plane, diagnostics, and protocol boundaries from which these capabilities can be built coherently.

---

# 1. How an LLM agent should use this manual

## 1.1 Required input

Before consulting feature details, the agent should have a high-level requirement statement that identifies, at minimum:

- the semantic outcome;
- inputs and outputs;
- affected authorities and state;
- required correctness and governance properties;
- expected scale, latency, and execution posture;
- interoperability boundaries;
- reproducibility and provenance expectations.

The requirement need not yet prescribe DataFusion or Arrow APIs. Prematurely naming a physical operator or storage implementation is itself a design smell unless the requirement is genuinely physical.

## 1.2 Mandatory review flow

For every material subsystem or capability, execute this sequence.

| Step | Agent action | Required output |
| --- | --- | --- |
| 1. **Extract semantics** | Separate domain/query meaning from implementation mechanics. | `SemanticRequirement` and explicit invariants. |
| 2. **Assign authority** | Name the single source of truth for each concept and every derived representation. | `AuthorityMap` with mutation, derivation, and staleness rules. |
| 3. **Select canonical representations** | Choose the Arrow/DataFusion objects that will represent data, schema, expressions, plans, and execution. | `RepresentationMap`. |
| 4. **Choose the highest viable extension level** | Attempt built-ins and transparent `Expr` composition before UDFs, providers, custom logical nodes, physical operators, or planners. | `ExtensionDecisionRecord`. |
| 5. **Define contracts and capability truth** | State what is invariant, what may vary, and which optimizer/runtime claims are exact, inexact, absent, or unknown. | `ContractAndCapabilityMatrix`. |
| 6. **Define lifecycle phases** | Expose declaration, resolution, validation, compilation, optimization, authorization, execution, verification, and persistence/observation boundaries. | `LifecycleArtifactMap`. |
| 7. **Preserve optimizer visibility** | Keep predicates, types, ordering, constraints, and calculation structure visible wherever possible. | `OptimizerVisibilityReview`. |
| 8. **Design provenance and reproducibility** | Specify identities, versions, fingerprints, configuration, plans, source snapshots, and result references produced by construction. | `ProvenanceClosureMap` and `ReproducibilityStatus`. |
| 9. **Define state ownership and resource behavior** | Assign session/runtime/query/task/partition scopes and cache invalidation. | `StateOwnershipMap` and `ResourcePlan`. |
| 10. **Derive evidence from contracts** | Generate tests for every advertised property and lifecycle boundary. | `TestEvidenceMatrix`. |
| 11. **Run anti-pattern review** | Reject hidden semantics, duplicate authorities, backend leakage, opaque UDFs, premature physicalization, metadata theater, and capability overclaiming. | `AntiPatternDisposition`. |
| 12. **Produce the implementation packet** | Only after the preceding artifacts are coherent, specify modules, traits, APIs, migrations, and ordered work. | `ImplementationPacket`. |

## 1.3 Stop conditions

The agent should stop at design—not proceed to code—when any of the following remains unresolved:

- two representations both appear authoritative;
- schema compatibility or null semantics are implicit;
- a provider cannot truthfully state pushdown or ordering behavior;
- a UDF is proposed only to hide transparent expression logic;
- physical choices are embedded in the semantic model without semantic necessity;
- provenance depends only on logs or human convention;
- state scope, ownership, or invalidation is unclear;
- a claimed metadata property is not consumed or enforced anywhere;
- tests cannot be derived for a claimed invariant.

---

# 2. Canonical architecture and representation map

## 2.1 Preferred compilation chain

```text
high-level requirement
    ↓
application semantic model
    ├─ SchemaContract
    ├─ CalculationSpec / ExprSpec
    ├─ PlanSpec / QuerySpec
    ├─ SourceSpec / TableSpec
    ├─ PolicySpec
    └─ ProvenanceSpec
    ↓ validate / normalize / fingerprint
bound semantic model
    ↓ compile
Arrow Schema + DataFusion Expr / DFSchema / LogicalPlan
    ↓ logical optimization and policy validation
optimized LogicalPlan
    ↓ physical planning with PhysicalPlanningContext
ExecutionPlan + PhysicalExpr + PlanProperties
    ↓ physical optimization / resource configuration
RecordBatch streams over Arrow arrays
    ↓ protocol / sink / persistence boundary
result plus provenance, diagnostics, metrics, and state-transition identity
```

## 2.2 Authority and derivation table

| Concept | Preferred authority | Native compiled/runtime form | Derived forms that must point back to authority |
| --- | --- | --- | --- |
| Domain calculation | Application `CalculationSpec` or equivalent | DataFusion `Expr`, built-in function call, or registered UDF | SQL rendering, physical expression, documentation, tests, plan fragments, metrics labels. |
| Schema contract | Application `SchemaContract` with version/fingerprint | Arrow `Schema` / `Field`; DataFusion `DFSchema` after qualification | Provider schema, projected schema, plan schema, `RecordBatch` schema, IPC/Parquet schema, API schema. |
| Relational intent | Application `PlanSpec`/query request or SQL text under a declared dialect | DataFusion `LogicalPlan` | Optimized plan, physical plan, serialized plan, explain output. |
| Execution strategy | DataFusion physical planner for the pinned environment | `ExecutionPlan`, `PhysicalExpr`, `PlanProperties` | Metrics, explain-analyze output, execution traces, spill artifacts. |
| Table access contract | `TableProvider` implementation registered under a catalog hierarchy | Provider-produced `ExecutionPlan` and Arrow batches | Table scan node, projections, filters, statistics, write plans. |
| Runtime data | Arrow memory model | Arrays and `RecordBatch` / batch streams | IPC, Parquet, C Data/C Stream, Flight payloads, Python capsules. |
| Planning configuration | Versioned application config resolved into `SessionConfig` / `ConfigOptions` | Session/query configuration snapshot | Explain bundle, cache keys, provenance references. |
| Resource environment | `RuntimeEnv` plus application deployment configuration | Memory pool, disk manager, object-store registry, caches | Task-level handles and metrics. |
| Function availability | Versioned function-package manifest / allowlist | DataFusion function registries | SQL visibility, docs, signatures, optimizer behavior. |
| Provenance | Application provenance record or graph | References from schema metadata, plan artifact, task/session identity, metrics | Audit views, lineage UI, commit/write metadata, reproducibility bundle. |

## 2.3 Extension-selection hierarchy

Use the first level that fully preserves required semantics:

```text
Arrow built-in array/kernel
    ↓ if relational semantics are required
DataFusion built-in SQL / DataFrame / Expr
    ↓ if reusable transparent composition is needed
application expression builder
    ↓ if a true custom scalar kernel is required
ScalarUDF / AsyncScalarUDF / HigherOrderUDF
    ↓ if aggregate/window/table semantics are required
AggregateUDF / WindowUDF / TableFunction / TableProvider
    ↓ if parser/binder syntax requires extension
ExprPlanner / TypePlanner / RelationPlanner / SQL planner hook
    ↓ if a new relational operator is required
LogicalPlan::Extension + ExtensionPlanner
    ↓ if no built-in physical operator can execute it
custom ExecutionPlan / PhysicalExpr
    ↓ only for global physical-planning replacement
custom QueryPlanner / PhysicalPlanner
```

Storage-specific progression should similarly prefer an existing `TableProvider`, `FileSource`, `ObjectStore`, `ParquetFileReaderFactory`, or expression-adapter seam before a new query operator.

---

# Part I — Principle-by-principle Arrow/DataFusion alignment

## P1 — Model semantics before implementing behavior

Important meaning should exist as inspectable typed models before it becomes control flow. Arrow and DataFusion should be treated as the compiled semantic/runtime vocabulary, not as an excuse to encode domain rules directly in service methods.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Arrow type system | `DataType`, `Field`, `Schema`, nested types, extension types | Represent data and schema meaning explicitly rather than inferring it from values or maps. |
| DataFusion expression IR | `Expr`, `SortExpr`, logical operators, typed literals | Represent calculations and predicates as traversable semantic trees. |
| Relational IR | `LogicalPlan`, `LogicalPlanBuilder`, DataFrame lazy plans | Represent relational intent independently of execution strategy. |
| Contract models | `DFSchema`, `Constraints`, `FunctionalDependencies`, `Statistics` | Make planning-relevant guarantees explicit and typed. |
| Typed configuration | `SessionConfig`, `ConfigOptions` and config extensions | Prevent scattered string/config lookups from becoming hidden semantics. |

### Required utilization rules

- Create application semantic models such as `SchemaContract`, `CalculationSpec`, `PlanSpec`, or `SourceSpec` when meaning must survive across operations or versions.
- Compile those authorities into Arrow schemas, DataFusion expressions, and logical plans through one controlled compiler/binder layer.
- Use native enums, typed literals, column identities, and schema objects; do not use display strings or generated SQL fragments as the internal source of truth.
- Traverse `Expr` and `LogicalPlan` to derive referenced columns, dependencies, policy scope, and diagnostics rather than maintaining parallel hand-written lists.
- Keep physical details—partition counts, join algorithms, batch size, spill policy—out of semantic models unless they change externally observable meaning.

### Application-owned overlay

- Arrow/DataFusion do not define the application semantic model, its versioning, or its stable identity.
- The application must define validation, serialization, evolution, and compiler ownership for those models.

### Required evidence

- A serialized semantic model exists independently of executable Rust control flow.
- One compiler path produces `Expr`/`LogicalPlan` and all consumers use it.
- Dependency extraction from the compiled tree matches declared inputs.
- No duplicated business rule appears in provider, writer, API, and test code.

### Reject these implementations

- Runtime strings as durable expression identity.
- Procedural branches that independently restate domain semantics.
- Example-data inference used as the schema contract.

**Primary utilization-pattern references:** MOD-01–MOD-08, SCH-01–SCH-04, EXP-01–EXP-03, LOG-01–LOG-03

---

## P2 — Make models executable, not merely descriptive

A semantic model should behave like a declarative program: it should validate, bind, compile, explain, fingerprint, and produce tests through controlled interpreters rather than being unpacked into unrelated procedural implementations.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Expression compilation | `Expr` plus `ExprSchemable` type/nullability inference | Compile calculation models into executable and statically inspectable expressions. |
| Plan compilation | `LogicalPlanBuilder`, DataFrame API, `SessionState::create_logical_plan` | Compile relational models into a common logical plan. |
| Optimization | Analyzer and logical optimizer rules | Transform compiled meaning without changing semantics. |
| Physical lowering | `PhysicalPlanner`, `PhysicalPlanningContext`, physical-expression creation | Select execution only after semantic validation. |
| Serialization | `datafusion-proto`, Substrait, Arrow IPC schema/data | Persist or exchange controlled derived representations. |

### Required utilization rules

- Give each model one compiler/interpreter interface that can emit the appropriate `Expr`, `LogicalPlan`, Arrow schema, documentation, dependency list, and test fixture.
- Use DataFusion plan and expression traversal to implement derived operations rather than re-reading the original request in each consumer.
- Validate types and nullability before physical planning; use return-field inference for functions and plan schema inspection for relational outputs.
- Make compilation deterministic under a pinned registry, catalog snapshot, and configuration.
- Treat DataFusion proto/Substrait as derived transport artifacts, not as the application semantic authority.

### Application-owned overlay

- Application models need their own semantic-version and compiler-version policy.
- Cross-version compatibility of serialized DataFusion plans must be managed explicitly.

### Required evidence

- The same model can generate execution, documentation, dependency inventory, and tests.
- Round-trip tests prove model serialization preserves meaning.
- Optimized and unoptimized execution return equivalent results.

### Reject these implementations

- DTOs immediately destructured into bespoke code paths.
- Separate SQL, DataFrame, and test implementations of the same calculation.
- Treating a physical plan as the durable business specification.

**Primary utilization-pattern references:** MOD-02–MOD-08, EXP-02, LOG-01–LOG-07, INT-06, TST-06–TST-08

---

## P3 — One authoritative owner for every concept

Multiple representations are expected, but every representation must declare the authority from which it derives and the mechanism by which staleness or incompatibility is detected.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Schema chain | Arrow `Schema` → `DFSchema` → plan schemas → `RecordBatch` schema | Maintain one semantic contract through qualified, projected, physical, and runtime forms. |
| Catalog registry | `CatalogProviderList` / `CatalogProvider` / `SchemaProvider` / `TableProvider` | Assign namespace and table access ownership. |
| Function registry | Session scalar/aggregate/window/table/higher-order registries | Centralize callable function definitions. |
| Object-store registry | `RuntimeEnv` object-store registry | Centralize storage-scheme resolution and credentials. |
| Resource authority | `RuntimeEnv` memory, disk, cache, and object-store handles | Prevent each operator from inventing its own resource environment. |

### Required utilization rules

- Name the application authority separately from its Arrow/DataFusion compiled representation.
- Cache `SchemaRef` and provider metadata as immutable snapshots tied to a source version or fingerprint.
- Resolve tables and functions through registries rather than constructing alternatives in arbitrary consumers.
- Make projected schemas, physical schemas, and batch schemas traceable to their parent contract and projection mapping.
- Invalidate plan, schema, and metadata caches when catalog, function registry, configuration, or source versions change.

### Application-owned overlay

- DataFusion in-memory catalogs are not automatically durable or governed.
- Stable authority IDs and cache invalidation versions are application responsibilities.

### Required evidence

- An `AuthorityMap` lists owner, mutation route, derivations, and invalidation for every major concept.
- No duplicate schema definition or function implementation is authoritative.
- Cache entries carry source version/fingerprint.

### Reject these implementations

- Rust struct, SQL DDL, JSON config, and writer each defining schema independently.
- Provider schema fetched live and changing during a query.
- Caches treated as unversioned truth.

**Primary utilization-pattern references:** MOD-04, SCH-01–SCH-03, CAT-01–CAT-04, RUN-01–RUN-06, OBS-08

---

## P4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation

Hierarchy should encode responsibility and substitutability. Consumers should rely on common contracts while implementations vary only along explicitly legal dimensions.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Catalog hierarchy | `CatalogProviderList` → `CatalogProvider` → `SchemaProvider` → `TableProvider` | Separate namespace ownership from table behavior. |
| Function hierarchy | Scalar, async scalar, aggregate, window, higher-order, and table functions | Use a semantic function family rather than forcing all calculations into one abstraction. |
| Plan hierarchy | `Expr` / `LogicalPlan` / `PhysicalExpr` / `ExecutionPlan` | Separate expression, relational, and execution responsibilities. |
| Data hierarchy | `DataType` / `Field` / `Schema` / `Array` / `RecordBatch` / reader or stream | Encode type, column, relation, and stream responsibilities. |
| State hierarchy | `SessionContext` / `SessionState` / `RuntimeEnv` / `TaskContext` / planning contexts | Assign scope and lifetime. |

### Required utilization rules

- Select the trait whose responsibility matches the requirement; do not use a lower-level trait merely because it is more powerful.
- Document universal guarantees at the trait boundary and backend-specific variation in the implementation.
- Keep namespaces, table contracts, scan execution, and data transport as separate levels.
- Use `TableFunction` when invocation produces a table provider; use aggregate/window abstractions for their actual semantics.
- Introduce a new hierarchy only when existing Arrow/DataFusion hierarchies cannot express the shared contract.

### Application-owned overlay

- Application hierarchies should mirror native responsibility boundaries, not duplicate them with competing layers.

### Required evidence

- Each interface states invariant methods and legal variability.
- Adding a new backend changes registration and one implementation, not consumers.
- Function types match their cardinality/state semantics.

### Reject these implementations

- A universal “plugin” trait that erases semantic differences.
- Consumers downcasting to concrete providers for normal behavior.
- Scalar UDFs used to simulate tables, aggregates, or windows.

**Primary utilization-pattern references:** CAT-01–CAT-10, EXP-03–EXP-10, LOG-01, PHY-01–PHY-03, RUN-01

---

## P5 — Encode variability behind contracts, not throughout consumers

Backend knowledge belongs at the adapter/provider boundary. The rest of the system should operate against Arrow and DataFusion contracts, not branch on storage or implementation type.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Table abstraction | `TableProvider` and `scan_with_args` | Localize source-specific planning and pushdown behavior. |
| File abstraction | `FileSource`, `DataSource`, `FileScanConfig` | Localize file-format and scan behavior. |
| Storage abstraction | `object_store::ObjectStore` registered in `RuntimeEnv` | Localize cloud/local storage operations. |
| Schema adaptation | `PhysicalExprAdapterFactory` and projection mapping | Localize physical-file-to-table schema differences. |
| Reader customization | `ParquetFileReaderFactory` and cache interfaces | Localize custom I/O without changing query consumers. |
| Planner extension | `ExtensionPlanner`, `RelationPlanner`, `ExprPlanner` | Localize syntax or relational variation at planning boundaries. |

### Required utilization rules

- Return Arrow batches and DataFusion plans from adapters; do not leak backend row objects into query code.
- Translate backend field paths, types, filters, projections, limits, and ordering in the provider/adapter that owns the backend.
- Advertise only capabilities the implementation can uphold exactly or inexactly.
- Register new providers, stores, readers, or planners through existing registries and builders.
- Keep retry, pagination, credential, and remote-schema logic out of consumers.

### Application-owned overlay

- Application-specific source identity, credential policy, and refresh lifecycle still require explicit models.

### Required evidence

- A new backend passes the same provider contract suite.
- No normal consumer branches on Delta/Parquet/API/memory source type.
- Projection and predicate mapping tests cover reordered and hidden backend fields.

### Reject these implementations

- Backend enums threaded through every service.
- Direct object-store calls scattered throughout operators.
- Provider returning batches that differ from its declared schema.

**Primary utilization-pattern references:** CAT-03–CAT-10, SRC-01–SRC-10, GOV-01, TST-02–TST-04

---

## P6 — Separate semantic meaning from execution strategy

Relational and domain intent should remain stable while DataFusion is free to optimize partitioning, ordering, algorithms, streaming, spill, and resource use.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Logical meaning | `Expr`, `DFSchema`, `LogicalPlan` | Describe what computation means. |
| Physical strategy | `PhysicalExpr`, `ExecutionPlan`, `PlanProperties` | Describe how it runs. |
| Lowering seam | `PhysicalPlanner`, `ExtensionPlanner`, `PhysicalPlanningContext` | Make the semantic-to-physical transition explicit. |
| Physical optimization | `EnsureRequirements`, repartitioning, sort pushdown, join selection | Change strategy without changing logical meaning. |
| Resource configuration | memory pools, spill, batch size, target partitions | Keep operational choices downstream. |

### Required utilization rules

- Compile high-level requirements to logical plans before selecting physical operators.
- Express required ordering/distribution semantically only when correctness depends on them; let physical planning choose implementations.
- Use `PlanProperties` and input requirements to communicate physical guarantees, not domain models.
- Keep runtime batching, object-store access, spill, and work stealing outside the semantic model.
- Test logical equivalence across alternative physical plans.

### Application-owned overlay

- Application models may expose non-semantic performance preferences, but they must be clearly classified as hints or policies.

### Required evidence

- A logical plan can be inspected without physical details.
- Changing partition count or join strategy leaves semantic fingerprints and results unchanged.
- Physical requirements are declared on operators, not embedded in domain specs.

### Reject these implementations

- PlanSpec names `HashJoinExec` or fixed partition counts without necessity.
- Business logic implemented inside custom stream polling.
- Physical plan persisted as the sole semantic authority.

**Primary utilization-pattern references:** MOD-05, LOG-01–LOG-08, PHY-01–PHY-10, RUN-07–RUN-09

---

## P7 — Build a shared canonical data fabric

Subsystems should compose through a small set of canonical representations: Arrow for data, DataFusion for expressions/plans, provider hierarchies for access, and standard protocols for persistence and transport.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Common data plane | Arrow arrays, schemas, `RecordBatch`, batch readers/streams | One typed columnar representation across operators and boundaries. |
| Logical plane | `Expr`, `DFSchema`, `LogicalPlan` | One relational representation across SQL, DataFrame, and programmatic builders. |
| Physical plane | `ExecutionPlan` and `SendableRecordBatchStream` | One executable streaming contract. |
| Authority plane | catalog/schema/table/function registries | One discovery and contract plane. |
| Persistence/transport | Parquet, IPC, Flight, C Data/C Stream, Substrait | Standard boundaries rather than pairwise formats. |

### Required utilization rules

- Adopt `RecordBatch` and `SchemaRef` as the default internal tabular boundary.
- Compile all query entry paths into `LogicalPlan` rather than maintaining separate SQL and programmatic engines.
- Require providers and physical operators to emit the same Arrow stream contract.
- Use standard protocols at process/language/durable boundaries.
- Create pairwise adapters only when a common Arrow/DataFusion/protocol boundary cannot express the requirement.

### Application-owned overlay

- Domain models and provenance identities sit above the fabric and should reference, not replace, its canonical objects.

### Required evidence

- A representation map shows few canonical types and explicit conversion points.
- Copies and row-materialization boundaries are inventoried.
- Interop tests prove schema and metadata behavior at every protocol boundary.

### Reject these implementations

- A separate row DTO for every subsystem.
- SQL and API query engines with unrelated semantics.
- Pairwise conversions among every component.

**Primary utilization-pattern references:** ARR-01–ARR-10, LOG-01, CAT-01, INT-01–INT-09

---

## P8 — Treat the common representation as infrastructure

Arrow is not a serialization afterthought; it is the in-memory and interoperability substrate. Designs should preserve columnar representation, ownership, schema, and streaming through the core pipeline.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Memory model | buffers, arrays, null buffers, offsets, `ArrayRef` | Efficient shared ownership and zero-copy operations. |
| Batch model | `RecordBatch` and readers/streams | Typed relation chunks with bounded memory. |
| Compute kernels | cast, filter, take, sort, arithmetic, comparison, string, temporal, nested kernels | Vectorized behavior instead of row loops. |
| Encoding | dictionary arrays, view types, nested arrays | Preserve useful physical encodings when semantics permit. |
| Scalar bridge | DataFusion `ColumnarValue` | Avoid expanding constants to arrays unnecessarily. |

### Required utilization rules

- Keep arrays and batches Arrow-native through calculation and query layers.
- Prefer Arrow kernels or DataFusion expressions over `Vec<Row>` loops.
- Use slicing, projection, dictionary preservation, and scalar fast paths deliberately to avoid copies.
- Expose streaming readers rather than eager tables where size is unbounded.
- Document every unavoidable conversion with ownership, copy, null, and schema consequences.

### Application-owned overlay

- Memory budgets and copy-boundary telemetry must be supplied by the application/runtime design.

### Required evidence

- A copy-boundary inventory exists.
- Benchmarks compare kernels/expressions to row implementations.
- Large-input tests prove bounded streaming and cancellation.

### Reject these implementations

- Pandas or row objects as the core internal model.
- Collecting all batches solely for API convenience.
- Rebuilding arrays from scalar rows after every operation.

**Primary utilization-pattern references:** ARR-01–ARR-10, EXP-01–EXP-02, PHY-09, INT-01–INT-04

---

## P9 — Make provenance intrinsic to every meaningful transformation

Plans, schemas, source identities, configuration, software versions, execution IDs, and outputs should be captured as normal products of an operation. Arrow/DataFusion provide artifacts and attachment points; the application must compose them into provenance.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Plan artifacts | logical/optimized/physical plans, `EXPLAIN`, `EXPLAIN ANALYZE` | Record what meaning and strategy were selected. |
| Schema metadata | Arrow schema/field metadata and extension metadata | Carry stable references to semantic contracts and lineage records. |
| Expression metadata | deterministic aliases, `with_metadata`, field-aware outputs | Preserve calculation/output identity where supported. |
| Execution identity | `TaskContext` session/task IDs and application trace IDs | Tie runtime work to a provenance record. |
| Metrics | `ExecutionPlanMetricsSet`, baseline/operator metrics | Record observed execution behavior. |
| Serialization | proto/Substrait and config/catalog snapshots | Persist reconstructible planning artifacts. |

### Required utilization rules

- Create a provenance envelope before execution and pass its IDs through planning, task context, tracing, output metadata, and write boundaries.
- Capture semantic model ID/version, schema fingerprint, input/source snapshot IDs, function registry version, config fingerprint, library versions, logical and physical plan artifacts, and result identity.
- Use Arrow metadata for stable references and compact annotations—not as the sole provenance store.
- Preserve plans and metrics in an artifact bundle keyed by execution/request identity.
- Mark volatile functions and external dependencies explicitly.

### Application-owned overlay

- Neither library automatically constructs a provenance graph or durable artifact store.
- Source versions and durable result identities must come from providers/persistence systems.

### Required evidence

- Every durable result resolves to an execution record.
- The execution record links inputs, schemas, calculations, configuration, plans, metrics, and software versions.
- Missing provenance is a validation failure for governed operations.

### Reject these implementations

- Only unstructured logs explain lineage.
- Embedding large provenance documents in Arrow metadata.
- Recording provenance after successful execution as a best-effort step.

**Primary utilization-pattern references:** OBS-01–OBS-12, SCH-06, EXP-11, RUN-05, TST-11

---

## P10 — Seek provenance closure

A durable result should recursively resolve the material facts needed to explain its creation. DataFusion/Arrow artifacts become nodes in that chain, while stable application identities connect them.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Schema identity | canonical Arrow schema plus application fingerprint/version | Resolve output and input contracts. |
| Plan identity | captured logical/physical plan or version-coupled serialized form | Resolve selected computation and strategy. |
| Function identity | registry snapshot, signatures, UDF package versions | Resolve calculations invoked by plans. |
| Source identity | provider metadata and application snapshot/version references | Resolve actual inputs. |
| Environment identity | crate versions, feature flags, Rust/toolchain, config | Resolve executable environment. |

### Required utilization rules

- Store stable references from results to provenance records; do not require every artifact to embed every ancestor.
- Define canonical IDs and versioned fingerprint algorithms for schemas, specs, registries, configs, and plans.
- Record DataFusion/Arrow versions because native serialized forms and display strings are version-coupled.
- Ensure provider snapshots and function registry contents can be reconstructed or archived.
- Model closure status and missing links explicitly.

### Application-owned overlay

- Closure requires an application artifact/lineage registry and retention policy.

### Required evidence

- A traversal from output ID reaches every required input and semantic artifact.
- Fingerprints are revalidated during replay.
- The system reports partial/non-reproducible closure rather than guessing.

### Reject these implementations

- Human-readable names as the only links.
- Plan text without environment/version context.
- Source URI without snapshot/version semantics.

**Primary utilization-pattern references:** OBS-01, OBS-05–OBS-12, INT-06, SCH-10, RUN-10

---

## P11 — Prefer immutable snapshots and explicit state transitions

Arrow’s immutable arrays and DataFusion’s immutable plan transformations should be used as exemplars. Semantically significant changes should produce new identified states, while mutable runtime state remains local and non-authoritative.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Immutable data | Arrow arrays, buffers, `RecordBatch`, `Arc` ownership | Share data safely and derive new arrays/batches without mutating authority. |
| Immutable plans | DataFrame transformations, `LogicalPlan` rewrites, `ExecutionPlan::replace_children` | Produce new plan trees rather than mutating hidden state. |
| Scoped runtime state | `TaskContext`, `PhysicalPlanningContext`, dynamic filters, accumulators | Confine mutation to execution scope. |
| Snapshot registries | cloned/built `SessionState`, catalog/function/config snapshots | Make planning inputs stable for a query. |

### Required utilization rules

- Use `Arc`-backed immutable schema, array, batch, and plan objects across shared boundaries.
- Represent semantic changes as new model versions or provider snapshots.
- Implement `replace_children` by constructing a semantically equivalent new operator and recomputing properties when needed.
- Implement `reset_state` for reusable physical plans that own dynamic runtime state.
- Never let a cache, accumulator, dynamic filter, or task-local object become a semantic authority.

### Application-owned overlay

- Application semantic state transitions require version IDs, validation, and audit records.

### Required evidence

- Before/operation/after identities exist for semantic changes.
- Concurrent queries see stable provider/schema/function snapshots.
- Re-execution tests reset all runtime state.

### Reject these implementations

- Arbitrary callers mutate a shared canonical schema.
- Plan properties retained after child replacement without proof.
- Query-scoped mutable state stored globally.

**Primary utilization-pattern references:** ARR-04, ARR-08, LOG-05, PHY-04–PHY-06, RUN-01–RUN-09

---

## P12 — Schemas are executable contracts, not documentation

Schema must govern planning, projection, execution, interoperability, and validation. Arrow and DataFusion provide strong schema objects, but compatibility, semantic annotations, versioning, and evolution policy must be explicit.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Arrow contract | `DataType`, `Field`, `Schema`, `SchemaBuilder`, metadata | Represent ordered fields, types, nullability, nested structure, and annotations. |
| Compatibility | `Schema::try_merge`, `Field::try_merge`, `Schema::contains`, projections and datatype comparisons | Implement explicit compatibility classes. |
| Logical schema | `DFSchema` qualifiers and functional dependencies | Support column resolution and relational reasoning. |
| Provider schema | `TableProvider::schema` and `TableSchema` file/partition/full separation | Make source-facing schema authoritative during planning. |
| Runtime validation | `RecordBatch::try_new`, plan/stream schema checks, DataFusion invariant checks | Reject mismatched runtime output. |
| Evolution/adaptation | `PhysicalExprAdapterFactory`, cast and projection mappings | Reconcile physical file schemas under declared policy. |
| Extension semantics | Arrow extension metadata and canonical extension types | Preserve domain logical types over valid storage types. |

### Required utilization rules

- Compile the application `SchemaContract` to one canonical Arrow `Schema` and use it to derive provider, plan, output, and protocol schemas.
- Define exact, backward, forward, mergeable, coercible, and incompatible classes explicitly; never equate `try_merge` success with policy approval.
- Freeze provider schema for a query and ensure projection indices refer to its stable field order.
- Validate every emitted batch against the plan/stream schema, including nested type and nullability expectations.
- Use deterministic aliases for derived expressions; do not persist display-generated field names.
- Classify metadata and extension types separately from enforced types and constraints.
- Round-trip schema contracts through every required IPC/Parquet/FFI boundary.

### Application-owned overlay

- Schema version, semantic field IDs, units, compatibility policy, and fingerprints require application ownership.
- Most arbitrary metadata is not runtime enforcement.

### Required evidence

- Schema contract tests cover exact, compatible, and rejected changes.
- Provider projection order and runtime batches are validated.
- Interop round trips preserve required metadata/extension semantics.
- Evolution changes generate explicit migration artifacts.

### Reject these implementations

- Silent widening, nullability changes, or reordering.
- Schema inferred from a sample as authority.
- Field display names used as durable IDs.
- Metadata key assumed to enforce a rule.

**Primary utilization-pattern references:** SCH-01–SCH-12, CAT-03, SRC-06–SRC-08, TST-01, TST-08–TST-09

---

## P13 — Put governance at the authoritative boundary

Policy should be enforced where namespace, table, function, plan, resource, or mutation authority resides. DataFusion exposes structural boundaries, but the application must supply the actual authorization model.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Namespace governance | catalog and schema providers | Control discovery and visibility. |
| Table governance | `TableProvider::schema`, `scan`, `scan_with_args`, DML methods | Control visible columns, tenant filters, and mutations. |
| Function governance | function registries and packages | Control callable calculations. |
| Plan governance | logical-plan traversal/validation and optimizer hooks | Reject disallowed operations before execution. |
| Resource governance | tenant/session-scoped `RuntimeEnv`, object stores, memory pools | Control credentials and resource budgets. |

### Required utilization rules

- Create tenant/user-scoped catalog, schema, table, and function views rather than filtering results after unrestricted planning.
- Apply row/tenant policy in the provider or a mandatory logical-plan rewrite/validation layer; prove residual predicates cannot be bypassed.
- Expose only authorized columns in the provider schema or through a controlled projection/masking layer.
- Gate update/delete/insert/merge/truncate methods at the target provider or transaction boundary.
- Use function allowlists and registry snapshots for executable calculation policy.
- Scope object-store credentials and memory/resource limits to the appropriate runtime/session.

### Application-owned overlay

- Authentication, authorization decisions, policy versioning, audit retention, and secret management are application responsibilities.

### Required evidence

- Policy tests use SQL, DataFrame, direct provider, and serialized-plan entry paths.
- Exact pushdown claims include enforced tenant predicates.
- Unauthorized functions/tables/columns cannot be resolved.

### Reject these implementations

- Policy only in UI or API handlers.
- A hidden column remains retrievable by direct DataFrame/plan APIs.
- Provider claims exact filtering but only uses it as file pruning.

**Primary utilization-pattern references:** GOV-01–GOV-10, CAT-01–CAT-08, LOG-07, RUN-02–RUN-04, TST-12

---

## P14 — Prefer the highest-level extension that preserves the semantics

Higher-level DataFusion and Arrow features retain more optimization, validation, explainability, portability, and governance. Drop lower only when the required semantics cannot be represented above.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Built-ins | Arrow kernels and DataFusion built-in functions/operators | Maximum native visibility and maintenance leverage. |
| Transparent composition | reusable `Expr` and `LogicalPlanBuilder` helpers | Code reuse without hiding semantics. |
| Function extensions | scalar/async/higher-order/aggregate/window/table functions | Add the narrowest missing semantic unit. |
| Data-source extensions | `TableProvider`, `FileSource`, `ObjectStore` | Add data access without inventing query semantics. |
| Planner extensions | `ExprPlanner`, `TypePlanner`, `RelationPlanner`, `ExtensionPlanner` | Extend syntax or logical lowering. |
| Physical extensions | `ExecutionPlan`, `PhysicalExpr`, `QueryPlanner` | Use only for genuinely new physical behavior. |

### Required utilization rules

- Search built-ins and Arrow kernels before writing custom code.
- Prefer an expression builder over a UDF when the expression tree remains transparent.
- Select UDAF/UDWF/UDTF/higher-order function according to actual cardinality and state semantics.
- Use `TableProvider` for a source, not a custom plan root exposed directly to consumers.
- Use logical extension nodes before custom physical nodes; let `ExtensionPlanner` lower them.
- Replace the global query planner only when local extension seams cannot express the requirement.

### Application-owned overlay

- Maintain an extension decision record documenting rejected higher-level alternatives and the semantic necessity for the chosen level.

### Required evidence

- The implementation packet cites the built-ins/features reviewed.
- Custom extensions implement all contract and optimizer hooks appropriate to their level.
- A lower-level choice has a written semantic justification.

### Reject these implementations

- Custom physical operator for code organization.
- Opaque UDF around simple predicates.
- Custom parser/planner when SQL/DataFrame/Expr already express the requirement.

**Primary utilization-pattern references:** EXT-01–EXT-10, ARR-06–ARR-07, EXP-01–EXP-10, CAT-09, LOG-08, PHY-01

---

## P15 — Preserve optimizer visibility

Encapsulation is valuable only if it does not hide facts needed for type inference, pruning, join reasoning, ordering, constraints, null propagation, or placement.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Transparent expressions | built-in `Expr` nodes and expression builders | Expose predicates, columns, literals, and operators. |
| UDF semantic hooks | `simplify`, `preimage`, `is_strict`, `evaluate_bounds`, `propagate_constraints`, ordering hooks, `placement` | Restore optimizer knowledge when custom functions are necessary. |
| Signature semantics | `Signature`, typed coercions, volatility, return-field inference | Expose accepted types, determinism, and output schema. |
| Provider metadata | pushdown support, statistics, constraints, defaults, functional dependencies | Expose source-side planning facts. |
| Physical properties | `EquivalenceProperties`, partitioning, ordering, `InputDistributionRequirements` | Expose execution guarantees. |
| Pruning/dynamic filters | `PruningPredicateBuilder`, `DynamicFilterTracking`, Parquet metadata pruning | Keep filter semantics available near data. |

### Required utilization rules

- Use built-in `Expr` composition for calculations that can be represented transparently.
- When introducing a UDF, implement every truthful optimizer hook and leave unsupported facts conservative.
- Declare `Volatility` and `is_strict` accurately; incorrect declarations can change results or rewrites.
- Expose exact/inexact/unsupported filter pushdown per predicate and provide statistics with correct `Precision`.
- Keep ordering and partitioning claims synchronized with actual physical output.
- Use physical-expression adapters and source rewrites without replacing logical semantics with opaque source code.

### Application-owned overlay

- Application-level semantic models should retain an optimizer-visibility review for every abstraction boundary.

### Required evidence

- `EXPLAIN` demonstrates expected pushdown, pruning, and operator selection.
- UDF hook tests compare optimized and unoptimized behavior.
- False metadata injection tests are rejected by invariants or contract tests.

### Reject these implementations

- Opaque convenience UDFs.
- Statistics fabricated for performance.
- Overclaiming order preservation or strictness.
- Filter translation that loses residual semantics.

**Primary utilization-pattern references:** EXP-01–EXP-08, CAT-05–CAT-07, LOG-04–LOG-06, PHY-03, PHY-07–PHY-10, SRC-03–SRC-05

---

## P16 — Treat lifecycle phases as first-class architecture

Declare, resolve, validate, normalize, compile, optimize, authorize, execute, verify, and observe should be visible phases with distinct artifacts and errors rather than one opaque method.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Parsing/binding | SQL parser, `PlannerContext`, `ContextProvider` | Resolve names, CTEs, parameters, outer scopes, and lambda variables. |
| Logical compilation | DataFrame/`LogicalPlanBuilder`/`SqlToRel` to `LogicalPlan` | Create the semantic relational artifact. |
| Logical optimization | analyzer and optimizer rule chains | Normalize and optimize meaning. |
| Physical planning | `PhysicalPlanner`, `PhysicalPlanningContext` | Lower a validated plan to execution. |
| Physical optimization | `EnsureRequirements`, sort/filter pushdown, repartitioning | Finalize strategy and requirements. |
| Execution | `ExecutionPlan::execute` and batch streams | Produce incremental Arrow results. |
| Observation | explain, metrics, traces, output schema | Expose what happened. |

### Required utilization rules

- Expose phase-specific functions and artifacts in the application orchestration layer.
- Run governance and semantic validation after binding and before physical execution; run runtime schema validation during execution.
- Use `ExecutionProps` only for query-wide optimization/expression properties and `PhysicalPlanningContext` for subtree physical-planning state.
- Keep `TableProvider::scan`/`scan_with_args` as planning-time plan construction; perform I/O in `execute`/stream polling.
- Tag errors with lifecycle phase and preserve causal context.
- Capture plans/config/schemas before executing so failures remain explainable.

### Application-owned overlay

- The application must define the complete operation lifecycle, write/commit phases, artifact retention, and error taxonomy.

### Required evidence

- Each phase has a named input/output artifact.
- Tests inject failures at binding, logical planning, physical planning, execution, and verification.
- No remote full-table read occurs during provider `scan`.

### Reject these implementations

- One “run query” method hides all intermediate artifacts.
- Authorization after data has been read.
- Heavy I/O in planning callbacks.

**Primary utilization-pattern references:** MOD-05, LOG-01–LOG-09, PHY-01–PHY-06, RUN-05–RUN-07, OBS-01–OBS-04

---

## P17 — Make intermediate artifacts inspectable and reproducible

The system should preserve or reconstruct the semantic and physical artifacts between request and result. DataFusion exposes rich plan and metric artifacts that should be captured systematically.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Logical inspection | `logical_plan`, optimized plan, tree traversal, display/Graphviz | Inspect bound and optimized semantics. |
| Physical inspection | physical-plan display, `PlanProperties`, operator tree | Inspect chosen strategy and guarantees. |
| Explain surfaces | `EXPLAIN`, `EXPLAIN VERBOSE`, `EXPLAIN ANALYZE`, PG JSON format | Create human/machine-readable diagnostics. |
| Serialization | `datafusion-proto`, Substrait | Store version-coupled or interoperable plan artifacts. |
| Execution evidence | operator metrics, output schema, row counts, result sample | Record observed behavior. |

### Required utilization rules

- Create a planning artifact bundle containing request/spec, catalog and schema snapshot, registry/config fingerprints, logical plans, physical plan, explain output, metrics, and output contract.
- Capture both unoptimized and optimized logical plans when upgrades or custom rules matter.
- Normalize plan snapshots for tests while retaining raw artifacts for diagnosis.
- Version the artifact schema and redact secrets/literals according to policy.
- Do not promise cross-version replay from native plan serialization without a compatibility contract.

### Application-owned overlay

- Artifact storage, redaction, retention, indexing, and cross-version migration are application-owned.

### Required evidence

- A failed query still has a partial artifact bundle through the failure phase.
- An upgrade harness can compare old/new logical and physical artifacts.
- Artifacts identify exact library/config/catalog/function versions.

### Reject these implementations

- Only final SQL and error text retained.
- Golden tests depend on unstable raw formatting without normalization.
- Serialized plan cached without dependency fingerprints.

**Primary utilization-pattern references:** OBS-01–OBS-04, OBS-07–OBS-12, INT-06, TST-06, TST-11

---

## P18 — Fingerprint anything whose identity matters

Names do not prove semantic identity. Application-owned canonical encodings should fingerprint schemas, specs, registries, configuration, source snapshots, and plan dependencies while treating native hashes and displays as version-local.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Schema material | ordered Arrow fields, datatypes, nullability, selected metadata | Canonical input for a schema fingerprint. |
| Expression/plan material | `Expr`/`LogicalPlan` traversal or versioned proto | Canonical input for a version-scoped calculation/plan fingerprint. |
| Registry material | function names, aliases, signatures, volatility, package versions | Fingerprint callable semantics. |
| Configuration material | `ConfigOptions`, feature flags, crate/toolchain versions | Fingerprint planning/execution environment. |
| Source material | provider snapshot/version, object metadata, schema fingerprint | Fingerprint data dependencies. |

### Required utilization rules

- Define a versioned canonicalization algorithm for every fingerprint domain.
- Exclude unstable pointer identities, debug output, and unordered maps unless normalized.
- Include semantic metadata selectively; classify non-semantic metadata separately.
- Use DataFusion proto bytes only within a pinned compatibility domain and include DataFusion/Arrow version in the fingerprint namespace.
- Use fingerprints in cache keys and provenance references, never names alone.

### Application-owned overlay

- Neither Arrow nor DataFusion promises stable cross-version semantic hashes.

### Required evidence

- Canonicalization tests are deterministic across process runs.
- Material semantic changes alter the fingerprint; irrelevant metadata does not unless policy says otherwise.
- Cache invalidation tests cover catalog, registry, config, and schema drift.

### Reject these implementations

- Hashing `Debug` or `Display` text as a timeless ID.
- Using `Arc` pointer identity.
- Cache key equals SQL text only.

**Primary utilization-pattern references:** MOD-06, SCH-10, OBS-07–OBS-10, RUN-08, TST-14

---

## P19 — Make reproducibility a normal operating mode

Reproduction should be designed into execution by pinning semantic inputs and recording environment, configuration, volatility, plans, and source versions. The system should model when exact reproduction is impossible.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Version pinning | DataFusion 55, Arrow/Parquet 59.2, Cargo lock and feature set | Fix the executable type/plan universe. |
| Function semantics | `Volatility`, strictness, start-time handling in `ExecutionProps` | Classify deterministic and time-sensitive calculations. |
| Config snapshot | `SessionConfig` / `ConfigOptions` | Record planner/runtime choices. |
| Plan artifacts | logical/physical plans and serialization | Record compiled computation. |
| Input snapshot hooks | provider/source version metadata | Pin data dependencies where the backend supports it. |

### Required utilization rules

- Record crate versions, feature flags, Rust/toolchain, `Cargo.lock` fingerprint, config, function registry, and catalog snapshot.
- Require input providers to expose immutable snapshot/version identity for reproducible operations; otherwise mark the operation partially reproducible.
- Record or prohibit volatile functions and external service calls according to policy.
- Stabilize time-sensitive functions with query start time and capture that time.
- Define deterministic output comparison separately from physical row order; add an explicit sort when order is part of the contract.
- Model reproducibility status with booleans/reasons rather than an undocumented promise.

### Application-owned overlay

- Durable source versioning and environment artifact retention require application/platform support.

### Required evidence

- Replay under the same environment reproduces contractually deterministic outputs.
- The provenance record explains every non-deterministic dependency.
- Upgrade tests distinguish semantic drift from physical-plan drift.

### Reject these implementations

- “Same SQL” treated as reproducible.
- Ignoring UDF volatility or external dependencies.
- Golden result tests relying on unspecified partition output order.

**Primary utilization-pattern references:** RUN-10, OBS-05–OBS-12, EXP-05, INT-06, TST-06, TST-14

---

## P20 — Be conservative about claimed capabilities

Optimizer and runtime metadata must be truthful. Unknown, absent, or inexact is preferable to a false guarantee that can corrupt results or invalidate optimization.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Filter capability | `TableProviderFilterPushDown::{Exact, Inexact, Unsupported}` | State per-predicate source enforcement truth. |
| Statistics precision | `Precision::{Exact, Inexact, Absent}` and `Statistics` | State certainty explicitly. |
| Physical properties | partitioning, ordering, boundedness, emission, order preservation | Communicate execution guarantees. |
| Function properties | `Volatility`, `is_strict`, nullability, ordering/constraint hooks | Communicate calculation semantics. |
| Constraints/FDs | `Constraints`, `FunctionalDependencies` | Enable reasoning only when true. |
| Pruning | `FilePruner` optionality and `PruningPredicateBuilder` | Do not imply pruning is possible without usable metadata. |

### Required utilization rules

- Return one pushdown result for each predicate and preserve residual filters for `Inexact` cases.
- Use `Absent` or unknown statistics instead of guessed values unless an explicit estimation policy marks them inexact.
- Declare ordering/partitioning only if every emitted batch/partition satisfies it.
- Default custom UDF optimizer properties to conservative values and opt in only with tests.
- Override `child_stats_requests` only for needed children and propagate statistics with correct partition semantics.
- Use invariant checks and adversarial tests to validate claims.

### Application-owned overlay

- Capability claims should be versioned as part of provider/function/operator contracts.

### Required evidence

- Contract tests falsify each claim with boundary cases.
- `EXPLAIN` and result tests verify residual predicates and sort/repartition insertion.
- Statistics and property tests cover empty, null, skewed, and partitioned inputs.

### Reject these implementations

- Exact pushdown used for file pruning only.
- Fake NDV/min/max to force a join plan.
- Claimed order preservation after a reordering operator.

**Primary utilization-pattern references:** CAT-05–CAT-07, EXP-04–EXP-07, PHY-03, PHY-07–PHY-09, SRC-03–SRC-05, TST-03, TST-07

---

## P21 — Separate enforced semantics from advisory metadata

Types, runtime validation, policies, planner hints, contractual annotations, governance tags, lineage references, and display metadata are distinct semantic classes. Metadata must never masquerade as enforcement.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Enforced data structure | Arrow datatypes, array/batch construction, DataFusion plan schema checks | Runtime structural validation. |
| Planner-consumed facts | statistics, ordering, partitioning, constraints, pushdown support | Optimization and reasoning inputs. |
| Contractual annotations | field/schema metadata and extension types | Semantic references and logical types when explicitly consumed. |
| Governance/lineage annotations | metadata keys and external IDs | References to policies/provenance records. |
| Documentation | UDF `Documentation`, aliases, descriptions | Human-facing advisory information. |

### Required utilization rules

- Classify every metadata field as enforced, planner-consumed, contractual, governance, lineage, or advisory.
- Name the component that consumes/enforces each non-advisory field.
- Use Arrow extension types only with valid storage types and registered interpretation; unknown consumers must still handle storage safely.
- Do not assume `Constraints` or arbitrary metadata are automatically enforced by DataFusion ingestion/writes.
- Promote a requirement from metadata to validation/policy code when correctness depends on it.

### Application-owned overlay

- Metadata governance schema, registry, validation, and retention are application responsibilities.

### Required evidence

- A metadata dictionary documents semantic class and consumer.
- Tests prove enforced rules fail when violated.
- Unknown metadata/extension consumers degrade safely.

### Reject these implementations

- A `unit=kg` tag assumed to prevent incompatible arithmetic.
- A classification tag assumed to enforce masking.
- A constraint declared but never validated.

**Primary utilization-pattern references:** SCH-05–SCH-07, EXP-11, GOV-06–GOV-08, OBS-05, TST-01

---

## P22 — Use protocols and canonical boundaries for interoperability

Language, process, and storage boundaries should use standard Arrow/DataFusion protocols before bespoke pairwise conversions. Each protocol should have an explicit schema, metadata, version, and streaming contract.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| In-process language boundary | Arrow C Data/C Stream, PyCapsule, `pyo3-arrow` | Zero-copy or low-copy typed exchange. |
| Process/network boundary | Arrow Flight and Flight SQL | Streaming Arrow-native RPC. |
| Durable columnar boundary | Parquet | Analytics-oriented persisted data with pruning metadata. |
| Ephemeral/stream boundary | Arrow IPC file/stream and sans-I/O stream encoding | Schema-preserving Arrow exchange. |
| Plan boundary | Substrait and DataFusion native proto | Interoperable semantic plans or pinned DataFusion plans. |
| Internal streaming boundary | `RecordBatchReader` and DataFusion batch streams | Backpressure and bounded memory. |

### Required utilization rules

- Select protocol according to boundary semantics, not convenience.
- Preserve schema, nullability, dictionary/extension metadata, ordering assumptions, and batch semantics through tests.
- Use C Stream/PyCapsule for in-process Python tables/readers instead of converting through Python lists or pandas.
- Use Substrait for cross-engine logical intent and native proto only within a tightly pinned DataFusion compatibility domain.
- Version custom protocol metadata and negotiate unsupported extension types.
- Maintain streaming rather than eager materialization where the protocol permits it.

### Application-owned overlay

- Protocol compatibility matrices, authentication, transport governance, and schema negotiation require application/platform design.

### Required evidence

- Round-trip fixtures exist across each required language/engine/version.
- Large streams remain bounded and cancellable.
- Unsupported extension/schema cases fail explicitly.

### Reject these implementations

- JSON rows for high-volume internal tabular exchange.
- Native plan proto treated as universal portable semantics.
- Pairwise adapters bypassing an available standard protocol.

**Primary utilization-pattern references:** INT-01–INT-10, ARR-03, SCH-12, TST-08–TST-10

---

## P23 — Keep state ownership local and explicit

Planning, runtime, task, expression, cache, and operator state should have named owners, scopes, lifetimes, mutability, and invalidation policies. DataFusion’s state stack should be used rather than replaced by globals.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Session scope | `SessionContext` and `SessionState` | Catalogs, functions, config, planners, and query creation. |
| Runtime scope | `RuntimeEnv` | Memory, disk, caches, object stores. |
| Task scope | `TaskContext` | Physical execution resources and IDs. |
| Query-wide planning scope | `ExecutionProps` | Start time, aliases, config snapshot, variable providers. |
| Subtree planning scope | `PhysicalPlanningContext` | Scalar-subquery slots and lambda-variable qualification. |
| Operator scope | accumulators, dynamic filters, streams, reservations, spill files | Mutable execution-local state. |

### Required utilization rules

- Use separate contexts per tenant/security/config boundary unless sharing is explicitly safe.
- Build `SessionState` through builders and capture its registry/config version before planning.
- Register object stores and resource managers in `RuntimeEnv`, not static globals.
- Pass `PhysicalPlanningContext` through extension planners and use an empty context only for standalone expressions without subqueries.
- Account operator memory with reservations and release it on completion/cancellation.
- Key caches by authority version, schema, config, and function/catalog fingerprints; document invalidation.
- Reset dynamic/operator state before plan reuse.

### Application-owned overlay

- Tenant lifecycle, cache governance, distributed state, and durable registry versions are application responsibilities.

### Required evidence

- A state ownership table covers process/runtime/session/query/task/partition/batch scopes.
- Concurrency and cancellation tests show no leaked state/resources.
- Cache invalidation tests cover every authority dependency.

### Reject these implementations

- Global mutable `SessionContext` for all tenants without policy isolation.
- Custom operator allocating outside the memory pool.
- Physical-planning subquery state stored in `ExecutionProps` or globals.

**Primary utilization-pattern references:** RUN-01–RUN-10, PHY-01, PHY-09–PHY-10, SRC-01, TST-10

---

## P24 — Make observability semantic, not merely operational

Runtime latency and memory are necessary but insufficient. Observability should expose bound schemas, source versions, function/spec identities, logical plans, physical strategies, pushdowns, configuration, and output identity.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Plan observability | `EXPLAIN`, plan displays, Graphviz, PG JSON explain | Expose semantic and physical plan choices. |
| Execution observability | `EXPLAIN ANALYZE`, operator metrics, baseline metrics | Expose rows, time, spill, pruning, and operator behavior. |
| Schema observability | Arrow/DFSchema snapshots, metadata, output schema | Expose contracts used and produced. |
| Source observability | provider/file scan metrics, pushed filters/projections, statistics | Expose data-access decisions. |
| Trace integration | application `tracing` spans plus task/session/request IDs | Tie runtime events to semantic artifacts. |

### Required utilization rules

- Emit semantic IDs/fingerprints as trace and metric dimensions through stable low-cardinality identifiers.
- Capture logical and physical plans, selected pushdowns, source snapshot IDs, schema fingerprints, config fingerprint, and output identity.
- Instrument custom operators/providers with DataFusion metrics and structured tracing.
- Separate sensitive SQL/literals/data samples from safe fingerprints and redacted artifacts.
- Define semantic SLOs such as pruning effectiveness, schema drift, plan drift, or source-version lag in addition to latency.

### Application-owned overlay

- Cross-system trace storage, dashboards, provenance joins, and redaction policy are application/platform responsibilities.

### Required evidence

- A request ID joins traces, plan bundle, metrics, and result provenance.
- Custom providers/operators expose meaningful metrics.
- Plan/pushdown drift alerts can be tested on upgrades.

### Reject these implementations

- Only function timings and error counts.
- High-cardinality raw SQL or customer values used as metric labels.
- Metrics unconnected to semantic execution identity.

**Primary utilization-pattern references:** OBS-01–OBS-06, OBS-11–OBS-12, PHY-11, SRC-03–SRC-05, TST-11

---

## P25 — Make testing derive from contracts and invariants

Every claimed schema, provider, function, optimizer, physical-property, state, interoperability, and reproducibility contract should generate evidence. Tests should follow the semantic models and lifecycle, not accidental implementation methods.

### Applicable Arrow and DataFusion mechanisms

| Feature family | Native mechanism | Alignment value |
| --- | --- | --- |
| Arrow boundary tests | schema/array/batch construction, IPC/Parquet/FFI round trips | Prove structural and interoperability contracts. |
| Provider tests | schema, projection, filter/limit pushdown, statistics, DML behavior | Prove advertised table capabilities. |
| Expression/UDF tests | type coercion, nullability, strictness, volatility, optimizer hooks | Prove calculation contracts. |
| Aggregate/window tests | partial/merge/final state, retract, group ordering, frames | Prove stateful semantics. |
| Plan tests | SQL logic tests, plan snapshots, optimizer equivalence | Prove logical and physical transformations. |
| Physical invariant tests | `check_invariants`, properties, distribution/order, cancellation/spill | Prove execution contracts. |
| Compatibility tests | version matrices, serialization, protocol fixtures | Prove upgrade and interop posture. |

### Required utilization rules

- Generate a test row for every invariant and capability claim in the design matrix.
- Compare results before/after logical and physical optimization.
- Test exact/inexact/unsupported pushdown with adversarial predicates and residual-filter checks.
- Test UDAF state partition independence and `GroupsAccumulator` conversion/merge semantics.
- Test custom execution plans for schema, expression traversal, child replacement, properties, statistics, state reset, streaming, cancellation, memory, spill, and serialization.
- Use SQL logic tests and golden fixtures for semantic coverage, but normalize unstable plan formatting.
- Fuzz malformed IPC/Parquet/JSON and unsafe/FFI boundaries where applicable.
- Run a pinned dependency/feature compatibility matrix and duplicate-Arrow/DataFusion checks.

### Application-owned overlay

- The application must maintain model-derived fixture generation, contract traceability, and release gates.

### Required evidence

- `TestEvidenceMatrix` links each contract to specific tests and CI jobs.
- Every custom extension has negative and adversarial tests.
- Upgrade tests distinguish API compilation, plan drift, performance drift, and semantic drift.

### Reject these implementations

- Tests only around public service methods.
- Plan snapshot alone treated as semantic correctness.
- No test for optimizer metadata truthfulness.

**Primary utilization-pattern references:** TST-01–TST-14 and every feature pattern’s evidence column

---

# Part II — Feature-utilization pattern catalogue

This catalogue gives stable identifiers to the recommended ways Arrow and DataFusion should be used. In the next design stage, functional building blocks can be mapped to these identifiers rather than to vague statements such as “use DataFusion.” Selecting a pattern means accepting its contract, lifecycle, provenance, and evidence obligations.

## 3. Semantic modeling and compilation patterns

## MOD — Semantic modeling and compilation

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| MOD-01 | `DataType`, `Field`, `Schema`, typed application models | Represent domain concepts with typed fields/enums/models before execution code. | P1, P3, P12 | Model serialization and invariant tests. |
| MOD-02 | `Expr` compiler | Compile calculation/predicate models into transparent DataFusion expressions through one owned compiler. | P1, P2, P15 | Model→Expr golden and dependency-extraction tests. |
| MOD-03 | `LogicalPlanBuilder` / DataFrame / SQL binder | Compile relational models into a common `LogicalPlan` rather than separate engines. | P1, P2, P6, P7 | Equivalent SQL/DataFrame/model results. |
| MOD-04 | Authority-to-derived representation map | Record which semantic authority produced each Arrow schema, expression, plan, provider, or batch. | P3, P9, P10 | Authority and staleness audit. |
| MOD-05 | Explicit validation/bind/compile phases | Do not combine request parsing, catalog resolution, type checking, and execution in one procedure. | P2, P6, P16 | Phase-specific failure injection. |
| MOD-06 | Versioned canonical encoding | Fingerprint application models using an owned stable encoding; namespace any DataFusion-plan fingerprint by engine version. | P10, P18, P19 | Determinism and semantic-change tests. |
| MOD-07 | Tree-derived information | Traverse `Expr`/`LogicalPlan` for columns, dependencies, volatility, policy scope, and documentation inputs. | P1, P2, P15 | Derived inventory equals actual plan references. |
| MOD-08 | Deterministic naming | Assign explicit stable aliases/field IDs; never use expression display output as durable identity. | P1, P12, P18 | Name stability and collision tests. |



## ARR — Arrow canonical data-plane utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| ARR-01 | `SchemaRef` / `FieldRef` / `DataType` | Use Arrow schema objects as the canonical runtime data contract. | P7, P8, P12 | Boundary schema equality/compatibility tests. |
| ARR-02 | `ArrayRef` and typed arrays | Keep columns in Arrow arrays; avoid subsystem-specific row containers. | P7, P8 | Copy and allocation benchmarks. |
| ARR-03 | `RecordBatch` | Use batches as the standard internal relation fragment across providers, operators, services, and tests. | P7, P8, P22 | Batch schema/length/null tests. |
| ARR-04 | `Arc`, immutable arrays, zero-copy slice | Share immutable buffers and derive slices/projections without mutation where possible. | P8, P11, P23 | Pointer/buffer-sharing and lifetime tests. |
| ARR-05 | Arrow null buffers and validity semantics | Model null behavior explicitly; never use sentinel values as hidden nulls. | P1, P12, P25 | All-null/no-null/mixed-null tests. |
| ARR-06 | Arrow compute kernels | Prefer vectorized cast/filter/take/sort/arithmetic/string/temporal/nested kernels over row loops. | P8, P14 | Correctness and throughput benchmarks. |
| ARR-07 | Dictionary, view, and nested encodings | Preserve useful encodings when semantics and consumers support them; materialize only at deliberate boundaries. | P8, P15, P20 | Encoding-preservation and fallback tests. |
| ARR-08 | `RecordBatchReader` / iterator / async stream | Expose bounded incremental consumption rather than eager full-table materialization. | P7, P8, P22, P23 | Backpressure, cancellation, and peak-memory tests. |
| ARR-09 | Builders and controlled unchecked paths | Use builders with capacity planning; reserve unchecked APIs for proven hot paths behind validation. | P8, P20, P25 | Fuzz/validation and benchmark evidence. |
| ARR-10 | Explicit conversion boundaries | Document every Arrow↔row/pandas/tensor/other conversion with copy, null, schema, and ownership semantics. | P7, P8, P22 | Conversion inventory and round-trip tests. |



## SCH — Schema, type, metadata, and evolution utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| SCH-01 | Application `SchemaContract` → Arrow `Schema` | Compile one versioned semantic schema authority into the runtime schema. | P1, P3, P12 | Compiler and fingerprint tests. |
| SCH-02 | Arrow `Schema` → DataFusion `DFSchema` | Add qualifiers and functional dependencies for planning without creating a second semantic authority. | P3, P6, P12 | Qualification and resolution tests. |
| SCH-03 | `TableProvider::schema` snapshot | Return a cheap, immutable, query-stable schema whose field order defines projection indices. | P3, P5, P12, P23 | Concurrent refresh and projection tests. |
| SCH-04 | Exact/contains/merge/equality modes | Define compatibility policy explicitly; use `Schema::contains`/`try_merge` as mechanisms, not policy decisions. | P1, P12, P20 | Compatibility classification matrix. |
| SCH-05 | Schema/field metadata classification | Classify each key as contractual, governance, lineage, advisory, or planner-consumed and name its consumer. | P9, P12, P21 | Metadata registry validation. |
| SCH-06 | Extension types | Use canonical extension types where available; custom types use stable names, valid storage types, versioned metadata, and graceful fallback. | P7, P12, P21, P22 | Known/unknown consumer round trips. |
| SCH-07 | `Constraints` and `FunctionalDependencies` | Declare only truthful relationships and distinguish planner reasoning from runtime enforcement. | P12, P15, P20, P21 | Optimizer and enforcement-boundary tests. |
| SCH-08 | `TableSchema` file/partition/full distinction | Keep physical file fields, partition columns, virtual columns, and table output schema explicit. | P3, P5, P12 | Mixed-schema/partition projection tests. |
| SCH-09 | Runtime plan/stream/batch schema validation | Validate every emitted batch against the plan and stream contract. | P12, P20, P25 | Negative malformed-batch tests. |
| SCH-10 | Schema fingerprints and versions | Fingerprint canonical ordered schema content with a versioned algorithm and include it in provenance/cache keys. | P9, P10, P18, P19 | Stable canonicalization tests. |
| SCH-11 | Physical schema adaptation | Use `PhysicalExprAdapterFactory`, casts, and projection mappings under an explicit evolution policy. | P5, P12, P15 | File-version and nested-evolution tests. |
| SCH-12 | IPC/Parquet/C interface schema round trips | Verify required names, nullability, metadata, dictionaries, and extension semantics at each boundary. | P12, P22, P25 | Golden cross-language/version fixtures. |



## CAT — Catalog, provider, and table-contract utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| CAT-01 | Catalog→schema→table hierarchy | Use native provider hierarchy as namespace and authority boundaries. | P3, P4, P7, P13 | Visibility and registration tests. |
| CAT-02 | Provider registration through session/catalog APIs | Resolve tables through registries rather than constructing source-specific plans in consumers. | P3, P5, P7 | Resolution and replacement/version tests. |
| CAT-03 | `TableProvider` contract | Own schema, table type, scan planning, pushdowns, statistics, constraints, defaults, and write posture in one provider. | P3, P4, P5, P12 | Full provider contract suite. |
| CAT-04 | Stable backend schema snapshot and mapping | Freeze remote/backend metadata and map provider field identity to backend paths/types. | P3, P5, P23 | Refresh/invalidation and reordered-field tests. |
| CAT-05 | `supports_filters_pushdown` | Return exact/inexact/unsupported per filter and preserve residual semantics. | P15, P20 | Adversarial pushdown truth tests. |
| CAT-06 | `scan_with_args` / `ScanArgs` | Use structured projection/filter/limit/statistics requests and keep scan planning cheap. | P5, P16, P20 | Planning-time I/O and argument tests. |
| CAT-07 | `StatisticsRequest` and provider statistics | Answer only cheap truthful requests; expose exact/inexact/absent precision. | P15, P20 | Cost/precision and stale-stat tests. |
| CAT-08 | Provider DML methods | Enforce insert/delete/update/truncate/merge policy and output-count contract at the table authority. | P13, P16, P20 | Authorization, atomicity-boundary, and count-schema tests. |
| CAT-09 | `TableFunction` / UDTF | Use table-producing function contracts instead of scalar or custom-plan workarounds. | P4, P14 | Schema and cardinality tests. |
| CAT-10 | Backend-specific implementation isolation | Keep pagination, retries, credentials, field mapping, and remote query translation inside provider/adapters. | P5, P7 | New-backend substitution test. |



## EXP — Expression and calculation utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| EXP-01 | Built-in DataFusion `Expr` and functions | Represent transparent arithmetic, predicates, casts, conditionals, arrays, and temporal logic with built-ins first. | P1, P14, P15 | Explain and result tests. |
| EXP-02 | Reusable expression builders | Encapsulate construction without wrapping visible logic in opaque UDFs. | P2, P14, P15 | Tree-shape/dependency tests. |
| EXP-03 | `ScalarUDFImpl` | Use only for true custom vector kernels or domain semantics not cleanly expressible by built-ins. | P4, P14 | Reference/differential and type tests. |
| EXP-04 | `Signature`, coercion, encoding preservation | Declare accepted post-coercion types precisely and preserve dictionary encoding only when implementation supports it. | P12, P15, P20 | Overload/coercion/encoding matrix. |
| EXP-05 | `Volatility`, `is_strict`, nullability | Expose determinism and null propagation truthfully for planning and reproducibility. | P15, P19, P20 | Null/volatile optimization tests. |
| EXP-06 | UDF optimizer hooks | Implement simplification, preimage, bounds, constraint propagation, ordering, placement, and struct-field mapping when truthful. | P15, P20 | Optimized/unoptimized equivalence tests. |
| EXP-07 | `conditional_arguments` / `short_circuits` | Declare lazy/conditional evaluation semantics to preserve correctness and side-effect behavior. | P15, P20 | Error/side-effect branch tests. |
| EXP-08 | Higher-order functions and lambdas | Use `HigherOrderUDFImpl` for true lambda semantics; model parameters, captures, coercion, and result fields explicitly. | P4, P14, P15 | Nested lambda/capture/list-type tests. |
| EXP-09 | `AggregateUDF`, `Accumulator`, `GroupsAccumulator` | Model algebraic state, partial/final merge, conversion-to-state, memory, null, and emit semantics. | P4, P14, P25 | Partition/merge/emit equivalence tests. |
| EXP-10 | Window UDF / `PartitionEvaluator` | Use window abstractions for frame/order/partition-aware semantics. | P4, P14 | Frame, peer, retract, and ordering tests. |
| EXP-11 | Return-field metadata and deterministic aliasing | Attach contractual references and stable names at expression output; do not rely on display names. | P9, P12, P21 | Output field/metadata tests. |
| EXP-12 | Async scalar UDF | Use only for unavoidable async per-value/batch work with concurrency, retry, timeout, and reproducibility policy. | P14, P19, P23 | Cancellation/backpressure/external-dependency tests. |



## LOG — Logical planning and optimization utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| LOG-01 | SQL/DataFrame/builder → `LogicalPlan` | Converge all query entry paths on one logical IR. | P2, P6, P7, P16 | Cross-entry equivalence tests. |
| LOG-02 | `PlannerContext` | Use explicit CTE, parameter, outer-scope, set-schema, and lambda planning state. | P1, P16, P23 | Scope/shadowing/correlation tests. |
| LOG-03 | `ContextProvider` | Resolve tables, functions, variables, options, and types through a controlled planning authority. | P3, P13, P16 | Authorized/unauthorized resolution tests. |
| LOG-04 | Analyzer and logical optimizer | Normalize and optimize only after binding; preserve semantic equivalence. | P2, P6, P15, P16 | Rule-by-rule equivalence tests. |
| LOG-05 | TreeNode traversal and transforms | Inspect/rewrite plans structurally and preserve schemas/expressions rather than string matching. | P1, P11, P15 | Transformation invariant tests. |
| LOG-06 | Plan metadata: schema, constraints, FDs, statistics requests | Carry planning facts explicitly through nodes. | P12, P15, P20 | Propagation tests. |
| LOG-07 | Logical plan policy validator | Authorize/reject tables, functions, DML, literals, joins, and resource-risk patterns before physical planning. | P13, P16 | Bypass-path policy tests. |
| LOG-08 | `LogicalPlan::Extension` | Model genuinely new relational meaning as a typed logical node with expressions, inputs, schema, and identity. | P6, P14, P15 | Rewrite/serialization/lowering tests. |
| LOG-09 | Native proto/Substrait logical artifacts | Persist derived plans with explicit compatibility and dependency context. | P17, P18, P22 | Round-trip and version-gate tests. |
| LOG-10 | DML logical models including `MERGE INTO` | Keep mutation semantics, predicates, clauses, and assignments in logical structures before target-provider execution. | P1, P6, P13, P16 | Clause ordering, schema, and authorization tests. |



## PHY — Physical planning, execution, and optimizer-contract utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| PHY-01 | `PhysicalPlanningContext` | Thread subtree scalar-subquery/lambda scope through physical planning and extension planners. | P6, P16, P23 | Subquery/lambda extension tests. |
| PHY-02 | `ExecutionPlan` as physical contract | Implement schema/properties/children/expressions/execution/metrics/state consistently. | P4, P6, P16 | Full custom-plan contract suite. |
| PHY-03 | `PlanProperties` and equivalence properties | Declare output schema, partitioning, ordering, boundedness, and emission conservatively. | P15, P20 | Property/invariant/adversarial tests. |
| PHY-04 | `apply_expressions` | Expose every owned/evaluated dynamic or static physical-expression root to traversal. | P11, P15, P25 | Expression-count and rewrite tests. |
| PHY-05 | `replace_children` plus compatibility `with_new_children` | Rebuild operators with new children and recompute properties unless preservation is proven. | P11, P16, P20 | Child replacement and property tests. |
| PHY-06 | `check_invariants`, `reset_state`, cancellation-safe execution | Make reusable plan correctness and state lifecycle explicit. | P11, P23, P25 | Reuse/cancel/invariant tests. |
| PHY-07 | `InputDistributionRequirements` and range/co-partitioning | Express child distribution and compatible layout requirements, not hard-coded consumer branches. | P6, P15, P20 | Satisfied/unsatisfied layout tests. |
| PHY-08 | Ordering requirements and order preservation | Declare required input ordering and `maintains_input_order` only when guaranteed. | P15, P20 | Sorted/unsorted/multipartition tests. |
| PHY-09 | `StatisticsContext`, `child_stats_requests`, `statistics_from_inputs` | Propagate statistics bottom-up with explicit partition requests and memoization. | P15, P20, P23 | Unary/join/partition stats tests. |
| PHY-10 | Dynamic filters and pruning | Use dynamic-filter tracking and pruning builders to adapt predicates without hiding their semantics. | P15, P20, P24 | Generation/update/complete/reset and pruning tests. |
| PHY-11 | Metrics, memory reservations, spill files | Instrument custom operators and account resources through runtime-managed contracts. | P23, P24 | Memory/spill/metric/cancel tests. |
| PHY-12 | Physical-plan serialization hooks | Use self-serialization/proto codecs only for self-contained or explicitly supported nodes under pinned compatibility. | P17, P18, P22 | Encode/decode and dependency tests. |



## SRC — Data source, file, Parquet, and object-store utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| SRC-01 | `RuntimeEnv` object-store registry | Register schemes/stores once and resolve through runtime state; isolate credentials by scope. | P3, P5, P13, P23 | Store resolution and isolation tests. |
| SRC-02 | `FileSource` / `FileScanConfig` / `DataSourceExec` | Use native file-scan planning/execution contracts rather than custom loops. | P5, P7, P14 | Multi-file/partition/limit tests. |
| SRC-03 | Projection/filter/limit pushdown | Push only what the source can implement while preserving logical evaluation order and residual semantics. | P15, P20 | Projection-only-filter-column and inexact-limit tests. |
| SRC-04 | Parquet row-group/page/bloom/row-filter pruning | Exploit metadata and late materialization while treating pruning as file/row avoidance, not semantic filtering unless exact. | P8, P15, P20 | Stats/no-stats/page/bloom tests. |
| SRC-05 | Sort pushdown, Top-K, dynamic early stopping | Expose ordering and dynamic thresholds to avoid unnecessary I/O/decoding under correct order semantics. | P6, P15, P24 | Ordered-file and early-stop metrics tests. |
| SRC-06 | `file_row_index()` and virtual columns | Use source-dependent virtual expressions for file-relative provenance/identity and push them to the scan. | P9, P12, P15 | Multi-file uniqueness-scope and placement tests. |
| SRC-07 | Schema evolution and physical-expression adapters | Map table predicates/projections to each file schema and fill/cast only under policy. | P5, P12, P15 | Historical-file schema matrix. |
| SRC-08 | Custom Parquet readers and metadata caches | Use reader factories/caches behind source contracts; include schema/source version in cache keys. | P5, P23 | Stale-cache and range-read tests. |
| SRC-09 | File-stream work stealing and output partitioning | Treat work stealing/order preservation/declared partitioning as physical policy; disable sharing when partition assignment is semantically required. | P6, P20, P23 | Distributed/preserve-order tests. |
| SRC-10 | CSV/JSON/Avro and sink boundaries | Use explicit schemas/options and stream/batch interfaces; treat weaker formats as controlled ingress/egress boundaries. | P7, P12, P22 | Malformed input and schema drift tests. |



## RUN — Session, runtime, state, cache, and resource utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| RUN-01 | State-scope taxonomy | Assign process/runtime/session/tenant/query/task/partition/batch scope before choosing an object. | P4, P23 | State ownership table. |
| RUN-02 | `SessionContext` | Use as the public session/tenant boundary for catalogs, functions, SQL/DataFrame APIs, and shared state. | P3, P13, P23 | Tenant/session isolation tests. |
| RUN-03 | `SessionState` and builder | Create a query-planning snapshot with explicit rules, registries, planners, config, and runtime. | P3, P16, P23 | Builder/config/registry fingerprint tests. |
| RUN-04 | `RuntimeEnv` | Own memory pools, disk manager, object stores, and caches centrally. | P3, P13, P23 | Resource isolation and limit tests. |
| RUN-05 | `TaskContext` | Pass physical execution resources and session/task identities into operators. | P9, P23, P24 | Task identity and cancellation tests. |
| RUN-06 | `ExecutionProps` | Use only for query-wide optimization/expression state such as start time, aliases, config snapshot, and variables. | P16, P19, P23 | Time/variable/alias tests. |
| RUN-07 | `PhysicalPlanningContext` | Use for subtree scalar-subquery result slots and lambda qualifiers; do not rebuild it as global mutable state. | P16, P23 | Nested planning scope tests. |
| RUN-08 | Cache entries with dependency fingerprints | Key and invalidate caches using schema, source, catalog, registry, config, and engine versions. | P3, P18, P23 | Dependency invalidation matrix. |
| RUN-09 | Memory reservations, spill, and bounded channels | Account dynamic operator buffers and implement bounded/cancellable execution. | P8, P23, P24 | Peak-memory/spill/drop tests. |
| RUN-10 | Environment snapshot | Record config, features, crate/toolchain versions, allocator/runtime, and resource policy for reproducibility. | P10, P18, P19 | Environment-fingerprint and replay tests. |



## INT — Interoperability and serialization utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| INT-01 | Arrow IPC file/stream | Use for Arrow-native schema/data interchange and bounded streams; version custom metadata. | P7, P22 | File/stream compression and metadata round trips. |
| INT-02 | Parquet | Use as durable analytical columnar interchange with explicit writer properties and schema-evolution tests. | P7, P12, P22 | Cross-engine/read-write fixtures. |
| INT-03 | Arrow C Data Interface | Use for zero-copy array/schema ownership transfer across native languages. | P8, P22 | Lifetime/release/malformed FFI tests. |
| INT-04 | Arrow C Stream / PyCapsule / `pyo3-arrow` | Use for Python in-process arrays, batches, and readers without pandas/list materialization. | P8, P22 | Python/Rust zero-copy and stream tests. |
| INT-05 | Arrow Flight / Flight SQL | Use for networked Arrow batch streaming and SQL service contracts. | P7, P22 | Auth, cancellation, schema, and backpressure tests. |
| INT-06 | DataFusion native proto | Use for pinned DataFusion plan/debug/cache transport with dependency fingerprints; not universal semantics. | P17, P18, P19, P22 | Same-version and rejection tests. |
| INT-07 | Substrait | Use for cross-engine logical-plan exchange with explicit function/type/table extension mapping. | P7, P22 | Cross-engine equivalence and unsupported-feature tests. |
| INT-08 | `RecordBatchReader` / `SendableRecordBatchStream` | Use as internal protocol boundaries to retain streaming and backpressure. | P7, P8, P22 | Large-data bounded-memory tests. |
| INT-09 | Extension-type negotiation | Preserve canonical/custom logical types where supported and degrade to valid storage type otherwise. | P12, P21, P22 | Known/unknown consumer tests. |
| INT-10 | Compatibility matrix | Pin and test Arrow/DataFusion/Parquet/Python/protocol versions and feature flags at each public boundary. | P18, P19, P22, P25 | CI version matrix and duplicate-crate gate. |



## OBS — Provenance, observability, identity, and reproducibility utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| OBS-01 | Planning/execution artifact bundle | Capture request/spec, authorities, schemas, registry/config, logical/physical plans, metrics, output, and errors. | P9, P10, P17, P24 | Complete/partial bundle tests. |
| OBS-02 | Logical-plan capture | Store bound and optimized logical artifacts with engine/catalog/function context. | P9, P17, P24 | Upgrade diff tests. |
| OBS-03 | Physical-plan and properties capture | Store selected operators, partitioning, ordering, boundedness, and requirements. | P9, P17, P24 | Physical drift tests. |
| OBS-04 | `EXPLAIN` / `EXPLAIN ANALYZE` | Produce human/machine diagnostics including optimizer and execution evidence. | P17, P24 | Format/redaction/snapshot tests. |
| OBS-05 | Schema/field metadata references | Attach compact IDs for schema contract, calculation, producer, run, and lineage records. | P9, P10, P21 | Metadata consumer and round-trip tests. |
| OBS-06 | Task/session/request/trace IDs | Propagate one execution identity through context, tracing, metrics, artifacts, and outputs. | P9, P24 | Cross-system correlation test. |
| OBS-07 | Dependency environment record | Capture versions, features, lockfile, toolchain, config, catalog, and registry fingerprints. | P10, P18, P19 | Environment replay validation. |
| OBS-08 | Source/provider snapshot identity | Require providers to expose immutable input identity or mark the operation non-reproducible. | P3, P9, P10, P19 | Snapshot freshness and missing-ID tests. |
| OBS-09 | Versioned semantic fingerprints | Fingerprint specs, schemas, registries, configs, and source snapshots with owned algorithms. | P10, P18 | Canonicalization tests. |
| OBS-10 | Reproducibility status model | Record determinism, pinned inputs, external dependencies, volatility, and environment completeness. | P10, P19 | Status derivation tests. |
| OBS-11 | Operator/provider metrics | Expose rows, bytes, pruning, spill, waits, retries, and source actions through structured metrics. | P24 | Metric semantics and cardinality tests. |
| OBS-12 | Provenance closure traversal | Resolve output→execution→plans/specs→inputs/schemas/environment recursively through stable references. | P9, P10 | Closure completeness audit. |



## GOV — Governance, policy, and capability-truth utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| GOV-01 | Catalog/schema/table visibility | Build authorized namespace views at provider boundaries. | P13 | Discovery bypass tests. |
| GOV-02 | Provider row and column enforcement | Apply tenant filters, masking, and visible schema at the table authority. | P13, P20 | Direct-provider and SQL bypass tests. |
| GOV-03 | Logical-plan policy validation | Inspect tables, functions, DML, joins, literals, and resource risk before execution. | P13, P16 | Serialized/direct plan policy tests. |
| GOV-04 | Function registry allowlist | Expose only approved built-ins/UDF packages and version registry contents. | P3, P13 | Unknown/disallowed function tests. |
| GOV-05 | Scoped runtime credentials/resources | Bind object stores, secrets, memory, and spill policies to tenant/session/runtime scope. | P13, P23 | Cross-tenant isolation tests. |
| GOV-06 | Capability truth table | Document exact/inexact/unsupported/absent semantics for provider, function, and operator claims. | P20, P21 | Claim-to-test traceability. |
| GOV-07 | Metadata semantic-class registry | Document consumers and enforcement level for every governance/lineage/contract key. | P21 | Unknown/misclassified key tests. |
| GOV-08 | Policy version/fingerprint | Include policy identity in plan/cache/provenance dependencies. | P10, P13, P18 | Policy drift invalidation tests. |
| GOV-09 | DML/write authority | Authorize mutation at target provider/sink boundary and capture affected-row/output contract. | P13, P16 | Unauthorized and partial-failure tests. |
| GOV-10 | Audit-ready decisions | Record policy decision, authority, version, subject, and execution/result IDs without exposing secrets. | P9, P13, P24 | Audit completeness/redaction tests. |



## EXT — Extension-level selection and implementation utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| EXT-01 | Built-in Arrow kernel | Use when the requirement is one/few array operations without relational planning. | P14 | Built-in review and benchmark. |
| EXT-02 | Built-in DataFusion expression/operator | Use when relational semantics are already expressible. | P14, P15 | `EXPLAIN` visibility evidence. |
| EXT-03 | Expression-builder library | Use for reusable transparent domain logic. | P14, P15 | Expr tree and dependency tests. |
| EXT-04 | Scalar/async/higher-order UDF | Use for a missing function semantic, selecting the precise function family. | P4, P14, P15 | Signature/null/optimizer tests. |
| EXT-05 | UDAF/UDWF/UDTF | Use when aggregate state, window frames, or table cardinality are intrinsic. | P4, P14 | State/frame/schema tests. |
| EXT-06 | `TableProvider` / `FileSource` / `ObjectStore` | Use for new data access, storage, or file behavior. | P5, P14 | Provider/source contract suite. |
| EXT-07 | SQL expression/type/relation planner hooks | Use when syntax/binding requires extension but existing logical nodes suffice. | P14, P16 | Parser/binder/round-trip tests. |
| EXT-08 | `LogicalPlan::Extension` + `ExtensionPlanner` | Use for genuinely new relational semantics with a typed logical node. | P6, P14 | Logical identity/rewrite/lowering tests. |
| EXT-09 | Custom `ExecutionPlan` / `PhysicalExpr` | Use only for missing physical algorithms or runtime behavior after logical semantics are defined. | P6, P14 | Full physical contract suite. |
| EXT-10 | Custom `QueryPlanner` / `PhysicalPlanner` | Use only when global lowering policy cannot be achieved through local extension planners/rules. | P14, P16 | Fallback, compatibility, and full-plan tests. |



## TST — Contract-derived testing and validation utilization

| ID | Feature(s) | Required leverage | Primary principles | Minimum evidence |
| --- | --- | --- | --- | --- |
| TST-01 | Schema contract tests | Cover exact equality, compatibility, nullability, nested fields, metadata, extension types, and rejected evolution. | P12, P21, P25 | Unit/property/golden schema suite. |
| TST-02 | Provider contract tests | Verify schema stability, projection order, filter-only columns, limits, streams, errors, and DML outputs. | P5, P12, P25 | Reusable provider harness. |
| TST-03 | Pushdown truth tests | Prove exact/inexact/unsupported behavior and residual filters with adversarial predicates. | P15, P20, P25 | Plan plus result assertions. |
| TST-04 | Source schema-adaptation tests | Cover physical schema reorder/add/remove/cast/nested evolution and virtual/partition fields. | P5, P12, P25 | Historical-file matrix. |
| TST-05 | UDF semantic tests | Cover signature, coercion, nulls, volatility, strictness, bounds, simplification, ordering, and errors. | P15, P20, P25 | Differential/property tests. |
| TST-06 | Optimizer equivalence tests | Compare unoptimized/optimized/serialized/deserialized execution results. | P2, P6, P17, P25 | Result equivalence harness. |
| TST-07 | Physical property/invariant tests | Validate schema, child arity, expression traversal, distribution, ordering, boundedness, emission, and statistics. | P20, P25 | `check_invariants` plus adversarial plans. |
| TST-08 | Serialization round-trip tests | Round-trip logical/physical plans and expressions within supported compatibility domains. | P17, P22, P25 | Codec/version tests. |
| TST-09 | Arrow protocol interoperability tests | Cross-read/write IPC, Parquet, C Data/C Stream, Python, and Flight fixtures. | P12, P22, P25 | Cross-language/version CI. |
| TST-10 | Resource/state tests | Test memory reservations, spill, cancellation, stream drop, reset, concurrency, and cache invalidation. | P11, P23, P25 | Stress/leak tests. |
| TST-11 | Plan/metric semantic snapshots | Capture normalized plan shape and required metric semantics without overfitting formatting. | P17, P24, P25 | Snapshot normalization tests. |
| TST-12 | Governance bypass tests | Exercise SQL, DataFrame, direct provider, function registry, serialized plans, and DML entry paths. | P13, P25 | Negative security suite. |
| TST-13 | Fuzz/malformed-input tests | Fuzz IPC, Parquet, JSON, FFI, builders, and unsafe/unchecked paths. | P8, P20, P25 | Fuzz targets and corpus. |
| TST-14 | Version/dependency migration matrix | Compile and execute under pinned feature/version profiles; reject duplicate Arrow/DataFusion type universes. | P18, P19, P22, P25 | CI matrix and cargo-tree gate. |



# Part III — Requirement-to-feature decision flows

These flows are the operational bridge between high-level requirements and the utilization catalogue. An agent should select every applicable flow, record its decisions, and reference the chosen pattern IDs.

## 4. Schema and data-contract flow

```text
Does the requirement introduce or alter tabular meaning?
    ├─ no → reuse the existing SchemaContract and Arrow Schema
    └─ yes
        ↓
Define semantic fields, identity, types, nullability, nested structure,
units/annotations, constraints, compatibility, version, fingerprint
        ↓
Compile to Arrow Schema
        ↓
Is qualification / relational reasoning required?
    ├─ yes → derive DFSchema + functional dependencies
    └─ no  → remain Arrow-native
        ↓
Does data come from a physical/backend schema?
    ├─ yes → define TableSchema + field/projection/cast mapping
    └─ no
        ↓
Validate provider schema, plan schema, stream schema, and every RecordBatch
        ↓
Round-trip through every required IPC/Parquet/FFI boundary
```

### Required selections

- `SCH-01` through `SCH-04` for every new schema authority.
- `SCH-08` and `SCH-11` for file, partition, virtual, or backend adaptation.
- `SCH-05` through `SCH-07` when metadata, extension types, constraints, or FDs are present.
- `SCH-09`, `SCH-10`, and `SCH-12` for runtime, identity, and interoperability.
- `TST-01`, and usually `TST-04`/`TST-09`.

### Agent questions

1. What distinguishes field identity from display name?
2. Which changes are backward/forward/merge compatible?
3. Which properties are enforced, planner-consumed, contractual, or advisory?
4. What is the physical-source schema, and how is it mapped to the table contract?
5. Where is the schema fingerprint stored and validated?

## 5. Calculation and expression flow

```text
Can Arrow kernel(s) express the operation without relational planning?
    ├─ yes → ARR-06 / EXT-01
    └─ no
        ↓
Can DataFusion built-in Expr/functions express it transparently?
    ├─ yes → EXP-01, optionally EXP-02
    └─ no
        ↓
What is the true semantic family?
    ├─ scalar vector kernel       → ScalarUDF
    ├─ asynchronous calculation  → AsyncScalarUDF
    ├─ lambda/list calculation   → HigherOrderUDF
    ├─ aggregate state           → AggregateUDF / Accumulator / GroupsAccumulator
    ├─ window/frame semantics    → WindowUDF / PartitionEvaluator
    └─ table-producing operation → TableFunction / TableProvider
        ↓
Declare signature, coercion, output field, null policy, volatility,
strictness, optimizer hooks, state/memory, documentation, provenance
        ↓
Compile/register under a versioned function package
        ↓
Test reference equivalence, optimization, serialization, and edge cases
```

### Required selections

- Always review `EXP-01` and `EXP-02` before any custom function.
- Apply `EXP-04` through `EXP-07` to every scalar UDF.
- Apply the semantic-family-specific patterns `EXP-08` through `EXP-10`.
- Apply `EXP-11` for stable output identity and provenance references.
- Apply `OBS-07`, `OBS-09`, and `GOV-04` to governed/versioned function packages.
- Apply `TST-05`; stateful functions also require `TST-10`.

### Agent questions

1. What optimizer-visible structure would a UDF hide?
2. What input types exist after DataFusion coercion?
3. Is the function strict, volatile, order-preserving, monotonic, or invertible over predicates?
4. What state must merge across partitions, and is the merge algebra correct?
5. How is the function implementation/version included in provenance and cache keys?

## 6. Table, source, and provider flow

```text
Is this merely a new location/scheme for an existing file/source implementation?
    ├─ yes → register ObjectStore or configure existing provider
    └─ no
        ↓
Is it a new file-format scan implementation?
    ├─ yes → FileSource / DataSource / FileScanConfig
    └─ no
        ↓
Is it a queryable logical table backed by API/DB/domain storage?
    ├─ yes → TableProvider
    └─ no → reconsider whether it belongs in DataFusion
        ↓
Freeze backend schema snapshot and field mapping
        ↓
Define projection/filter/limit/statistics/write capabilities truthfully
        ↓
Build ExecutionPlan during scan; perform I/O in execute/stream polling
        ↓
Emit Arrow RecordBatch streams matching projected schema
```

### Required selections

- `CAT-03` through `CAT-07` for every custom provider.
- `SRC-01` for storage; `SRC-02` through `SRC-10` as applicable.
- `CAT-08` and `GOV-09` for writes/mutations.
- `SCH-03`, `SCH-08`, `SCH-09`, and often `SCH-11`.
- `TST-02` through `TST-04` plus `TST-10`.

### Agent questions

1. Is `schema()` cheap and stable for the entire query?
2. Do projection indices refer to provider field order, and are filter-only columns handled?
3. Does `Exact` mean the source enforces all SQL null/type semantics?
4. Can limit be safely pushed with inexact predicates?
5. Which source snapshot/version and schema fingerprint identify the read?

## 7. Relational plan and query flow

```text
What is the authoritative query/PlanSpec or SQL request?
        ↓
Resolve catalog, functions, variables, parameters, scopes, and policy
        ↓
Compile all entry paths to LogicalPlan
        ↓
Validate schema, types, function/table access, DML, and resource policy
        ↓
Run analyzer and logical optimizer
        ↓
Does requirement need new relational meaning?
    ├─ no → use built-in logical nodes
    └─ yes → LogicalPlan::Extension + typed node
        ↓
Capture bound and optimized logical artifacts
        ↓
Lower via PhysicalPlanner / ExtensionPlanner
```

### Required selections

- `LOG-01` through `LOG-07` for ordinary governed planning.
- `LOG-08` only for genuinely new relational semantics.
- `LOG-09` when plans are serialized or exchanged.
- `LOG-10` for DML/mutation plans.
- `OBS-01` through `OBS-04` for important executions.
- `TST-06`, `TST-08`, and `TST-11`.

### Agent questions

1. Are SQL, DataFrame, and model compilation semantically identical?
2. Which names/scopes are resolved by `PlannerContext` and `ContextProvider`?
3. Where is policy validation mandatory?
4. What semantic information must remain visible to optimizer rules?
5. What dependency fingerprints invalidate a cached plan?

## 8. Physical execution and performance flow

```text
Optimized LogicalPlan
        ↓
PhysicalPlanningContext + session/config
        ↓
ExecutionPlan tree
        ↓
For every custom node:
  schema/properties/children/apply_expressions/replace_children
  distribution/order requirements/statistics/state/metrics/execute
        ↓
EnsureRequirements and other physical optimizer rules
        ↓
Memory reservation, spill, bounded channels, object-store/file behavior
        ↓
Incremental RecordBatch streams
        ↓
Metrics, explain-analyze, cancellation, output validation
```

### Required selections

- `PHY-01` through `PHY-09` for all custom physical nodes.
- `PHY-10` where dynamic filters or pruning exist.
- `PHY-11` for metrics/resources and `PHY-12` for serialization.
- `RUN-04`, `RUN-05`, `RUN-09`, and `RUN-10`.
- `TST-07`, `TST-10`, and `TST-11`.

### Agent questions

1. What output properties are guaranteed versus merely desired?
2. Are children co-partitioned or independently partitioned?
3. Does child replacement recompute every affected property?
4. Which child statistics are requested at which partitions?
5. Does `execute()` return quickly and leave work to stream polling?
6. What happens on stream drop, recursive/repeated execution, or memory pressure?

## 9. Interoperability flow

```text
Identify boundary semantics
    ├─ in-process Rust/Python arrays/readers → C Data/C Stream / PyCapsule
    ├─ Arrow-native process/network stream  → Flight / Flight SQL
    ├─ durable analytics file               → Parquet
    ├─ Arrow-native file/message stream     → IPC
    ├─ cross-engine logical plan            → Substrait
    └─ pinned DataFusion plan transport     → native proto
        ↓
Declare schema/metadata/extension/version/ownership/streaming contract
        ↓
Implement capability negotiation and explicit unsupported cases
        ↓
Cross-language/engine/version round-trip testing
```

### Required selections

- One or more of `INT-01` through `INT-09`.
- Always `INT-10` for a public or durable boundary.
- `SCH-12`, `ARR-10`, `TST-08`, and `TST-09`.
- `OBS-07` if reproducibility or audit matters.

### Agent questions

1. Is the boundary in-process, networked, durable, or semantic-plan exchange?
2. Does it preserve streaming/backpressure?
3. What metadata and extension types survive?
4. Who owns and releases buffers?
5. What compatibility versions are supported and tested?

## 10. Governance and policy flow

```text
Identify governed authority
    ├─ namespace → CatalogProvider / SchemaProvider
    ├─ table/rows/columns → TableProvider
    ├─ calculation → function registry
    ├─ query semantics → logical-plan validator/rewrite
    ├─ mutation → provider/sink DML boundary
    └─ credentials/resources → scoped RuntimeEnv/session
        ↓
Compile policy into structural visibility/enforcement
        ↓
Include policy version/fingerprint in planning and provenance dependencies
        ↓
Test every alternate entry path and bypass attempt
```

### Required selections

- `GOV-01` through `GOV-10` as applicable.
- `CAT-01`, `CAT-03`, `LOG-03`, `LOG-07`, `RUN-02` through `RUN-05`.
- `TST-12` is mandatory for governed systems.

### Agent questions

1. At which authority boundary can the policy be made unavoidable?
2. Does visible schema itself reflect column policy?
3. Are tenant predicates semantically exact, or merely pruning hints?
4. Can a direct provider, DataFrame, serialized plan, or DML call bypass policy?
5. Is the policy version in cache/provenance dependencies?

## 11. Provenance, observability, and reproducibility flow

```text
Before planning:
  allocate execution/request/run identity
  resolve semantic/schema/policy/function/config/source identities
        ↓
During planning:
  capture bound + optimized logical plans and dependency fingerprints
        ↓
During physical planning:
  capture physical plan/properties/resource configuration
        ↓
During execution:
  propagate IDs; capture metrics, pushed predicates, source/file actions,
  spills, errors, output schema, row counts
        ↓
At result/write boundary:
  create result/state-transition identity and link to execution record
        ↓
Validate provenance closure and reproducibility status
```

### Required selections

- `OBS-01` through `OBS-12` according to criticality.
- `SCH-10`, `MOD-06`, `RUN-10`, `INT-06`/`INT-07` as applicable.
- `GOV-10` for audited systems.
- `TST-11` and `TST-14`.

### Agent questions

1. Can any durable result resolve its semantic spec, schema, inputs, plans, config, functions, environment, and execution?
2. Which links are embedded references versus external artifacts?
3. Which native artifacts are version-coupled?
4. What is redacted, and how can authorized operators retrieve the full artifact?
5. Does the operation explicitly report non-determinism or missing source pins?

---

# Part IV — Required agent design artifacts

The following artifact set operationalizes the design constitution. An agent should produce these before an implementation packet for any material subsystem.

## 12. `SemanticRequirement`

```yaml
semantic_requirement:
  id: stable requirement id
  objective: externally meaningful outcome
  inputs:
    - semantic input and required version/snapshot semantics
  outputs:
    - output contract and identity
  invariants:
    - machine-testable invariant
  non_semantic_preferences:
    - latency, scale, deployment, physical hints
  prohibited_shortcuts:
    - hidden assumptions or disallowed implementation classes
```

## 13. `AuthorityMap`

| Concept | Authority | Mutable by | Derived representations | Derivation | Staleness/invalidation | Provenance identity |
|---|---|---|---|---|---|---|
| schema | `SchemaContract` | schema governance workflow | Arrow Schema, DFSchema, provider/plan/batch schemas | schema compiler | version/fingerprint mismatch | schema ID/version/fingerprint |
| calculation | `CalculationSpec` | calculation package owner | Expr/UDF/SQL/docs/tests | calculation compiler | spec/package/registry fingerprint | calculation ID/version |
| table access | registered provider snapshot | provider/catalog owner | TableScan/ExecutionPlan/RecordBatch stream | scan planning | source/catalog/schema version | table/source snapshot ID |

The agent should add every substantive concept; three rows are only illustrative.

## 14. `RepresentationMap`

```yaml
representations:
  semantic:
    - application model and authority
  runtime_data:
    - Arrow Schema / Array / RecordBatch
  logical_query:
    - Expr / DFSchema / LogicalPlan
  physical_execution:
    - PhysicalExpr / ExecutionPlan / PlanProperties
  persistence_or_transport:
    - IPC / Parquet / Flight / C Stream / Substrait / native proto
  provenance:
    - external record IDs plus compact metadata references
  conversions:
    - source, target, copy behavior, null behavior, metadata behavior
```

## 15. `FeatureUtilizationPlan`

For each requirement or future functional building block, create rows of this form:

| Requirement/building block | Selected pattern IDs | Native features | Application overlay | Why highest viable level | Key contracts | Evidence |
|---|---|---|---|---|---|---|
| example calculation | EXP-01, EXP-02, SCH-02, TST-05 | built-in Expr + DFSchema | CalculationSpec registry | transparent expression retains pruning | types/nulls/alias | model→Expr + optimized equivalence |

A building block is not considered mapped merely because a crate is named. It must select concrete utilization patterns.

## 16. `ContractAndCapabilityMatrix`

| Claim | Semantic class | Exact/inexact/absent | Owner | Consumer/enforcer | Failure consequence | Test |
|---|---|---|---|---|---|---|
| filter pushdown for predicate X | planner/runtime | Exact | provider | provider scan | wrong rows if false | adversarial pushdown test |
| output ordering | physical property | Exact | execution node | optimizer/downstream operators | wrong algorithm/result if false | multipartition order test |
| schema metadata unit | contractual annotation | Advisory unless calculation layer consumes | schema contract | calculation validator | semantic mismatch | metadata-consumer test |

## 17. `LifecycleArtifactMap`

| Phase | Input | Native Arrow/DataFusion artifact | Application artifact | Validation/policy gate | Failure code |
|---|---|---|---|---|---|
| declare | high-level request | none | semantic spec | model validation | `declaration.*` |
| resolve | semantic spec | catalog/function lookup | bound dependencies | access policy | `resolution.*` |
| compile | bound model | Expr/DFSchema/LogicalPlan | compiler diagnostics | schema/type validation | `logical_compilation.*` |
| optimize | logical plan | optimized LogicalPlan | plan diff | optimizer invariants | `logical_optimization.*` |
| physical plan | optimized plan | ExecutionPlan | physical summary | property/resource policy | `physical_planning.*` |
| execute | physical plan | RecordBatch stream/metrics | execution record | runtime schema/resource checks | `execution.*` |
| verify/result | batches | output schema/results | result identity/provenance | output contract | `verification.*` |

## 18. `ProvenanceClosureMap`

```yaml
provenance:
  result_id: ...
  execution_id: ...
  semantic_spec:
    id: ...
    version: ...
    fingerprint: ...
  schema_contracts:
    - id/version/fingerprint
  functions:
    registry_fingerprint: ...
    packages: [...]
  sources:
    - provider_id: ...
      snapshot_id: ...
      schema_fingerprint: ...
  planning:
    logical_artifact: ...
    physical_artifact: ...
    config_fingerprint: ...
    policy_fingerprint: ...
  environment:
    datafusion: 55.0.0
    arrow: 59.2.0
    parquet: 59.2.0
    rust_toolchain: ...
    cargo_lock_fingerprint: ...
  observations:
    metrics_artifact: ...
    output_schema_fingerprint: ...
  reproducibility:
    deterministic: ...
    inputs_pinned: ...
    volatile_functions: ...
    external_dependencies_pinned: ...
    missing_links: [...]
```

## 19. `StateOwnershipMap`

| State | Scope | Owner | Mutable? | Lifetime | Authority relationship | Refresh/reset | Concurrency/invalidation |
|---|---|---|---|---|---|---|---|
| catalog/function registry | session/tenant | SessionState owner | controlled | session version | derived from governed registry | rebuild/version | invalidate plans/caches |
| object stores/memory/disk | runtime | RuntimeEnv | controlled | runtime | resource authority, not semantic authority | deployment policy | tenant isolation |
| scalar subquery slots | physical-plan subtree/query | PhysicalPlanningContext/result container | yes | execution | runtime only | clear/reset | scoped shared container |
| accumulator | operator/partition/group | execution node | yes | execution | derived runtime state | emit/reset/drop | memory reservation |

## 20. `OptimizerVisibilityReview`

For every custom abstraction answer:

1. Which columns, predicates, literals, types, ordering, constraints, and null properties remain visible?
2. Which become opaque?
3. Can a transparent `Expr` builder replace the abstraction?
4. Which UDF/provider/operator hooks restore truthful optimizer knowledge?
5. What `EXPLAIN` evidence proves the intended visibility?

## 21. `TestEvidenceMatrix`

| Contract ID | Claim | Positive tests | Negative/adversarial tests | Property/differential tests | Serialization/interop tests | Upgrade tests | CI gate |
|---|---|---|---|---|---|---|---|

Every row in `ContractAndCapabilityMatrix` must have a corresponding evidence row.

## 22. `ExtensionDecisionRecord`

```yaml
extension_decision:
  requirement: ...
  candidates_reviewed:
    - built-in Arrow kernel
    - built-in DataFusion expression/operator
    - expression builder
    - scalar/aggregate/window/table/higher-order function
    - provider/source/store
    - planner hook
    - logical extension
    - physical extension
    - custom planner
  selected_level: ...
  why_higher_levels_fail_semantically: ...
  optimizer_visibility_preserved: ...
  additional_contracts_introduced: ...
  required_tests: ...
```

---

# Part V — Crosswalks for future functional building blocks

## 23. Principle-to-pattern crosswalk

| Principle | Primary utilization-pattern ranges |
| --- | --- |
| P1 — Model semantics before implementing behavior | MOD-01–MOD-08, SCH-01–SCH-04, EXP-01–EXP-03, LOG-01–LOG-03 |
| P2 — Make models executable, not merely descriptive | MOD-02–MOD-08, EXP-02, LOG-01–LOG-07, INT-06, TST-06–TST-08 |
| P3 — One authoritative owner for every concept | MOD-04, SCH-01–SCH-03, CAT-01–CAT-04, RUN-01–RUN-06, OBS-08 |
| P4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation | CAT-01–CAT-10, EXP-03–EXP-10, LOG-01, PHY-01–PHY-03, RUN-01 |
| P5 — Encode variability behind contracts, not throughout consumers | CAT-03–CAT-10, SRC-01–SRC-10, GOV-01, TST-02–TST-04 |
| P6 — Separate semantic meaning from execution strategy | MOD-05, LOG-01–LOG-08, PHY-01–PHY-10, RUN-07–RUN-09 |
| P7 — Build a shared canonical data fabric | ARR-01–ARR-10, LOG-01, CAT-01, INT-01–INT-09 |
| P8 — Treat the common representation as infrastructure | ARR-01–ARR-10, EXP-01–EXP-02, PHY-09, INT-01–INT-04 |
| P9 — Make provenance intrinsic to every meaningful transformation | OBS-01–OBS-12, SCH-06, EXP-11, RUN-05, TST-11 |
| P10 — Seek provenance closure | OBS-01, OBS-05–OBS-12, INT-06, SCH-10, RUN-10 |
| P11 — Prefer immutable snapshots and explicit state transitions | ARR-04, ARR-08, LOG-05, PHY-04–PHY-06, RUN-01–RUN-09 |
| P12 — Schemas are executable contracts, not documentation | SCH-01–SCH-12, CAT-03, SRC-06–SRC-08, TST-01, TST-08–TST-09 |
| P13 — Put governance at the authoritative boundary | GOV-01–GOV-10, CAT-01–CAT-08, LOG-07, RUN-02–RUN-04, TST-12 |
| P14 — Prefer the highest-level extension that preserves the semantics | EXT-01–EXT-10, ARR-06–ARR-07, EXP-01–EXP-10, CAT-09, LOG-08, PHY-01 |
| P15 — Preserve optimizer visibility | EXP-01–EXP-08, CAT-05–CAT-07, LOG-04–LOG-06, PHY-03, PHY-07–PHY-10, SRC-03–SRC-05 |
| P16 — Treat lifecycle phases as first-class architecture | MOD-05, LOG-01–LOG-09, PHY-01–PHY-06, RUN-05–RUN-07, OBS-01–OBS-04 |
| P17 — Make intermediate artifacts inspectable and reproducible | OBS-01–OBS-04, OBS-07–OBS-12, INT-06, TST-06, TST-11 |
| P18 — Fingerprint anything whose identity matters | MOD-06, SCH-10, OBS-07–OBS-10, RUN-08, TST-14 |
| P19 — Make reproducibility a normal operating mode | RUN-10, OBS-05–OBS-12, EXP-05, INT-06, TST-06, TST-14 |
| P20 — Be conservative about claimed capabilities | CAT-05–CAT-07, EXP-04–EXP-07, PHY-03, PHY-07–PHY-09, SRC-03–SRC-05, TST-03, TST-07 |
| P21 — Separate enforced semantics from advisory metadata | SCH-05–SCH-07, EXP-11, GOV-06–GOV-08, OBS-05, TST-01 |
| P22 — Use protocols and canonical boundaries for interoperability | INT-01–INT-10, ARR-03, SCH-12, TST-08–TST-10 |
| P23 — Keep state ownership local and explicit | RUN-01–RUN-10, PHY-01, PHY-09–PHY-10, SRC-01, TST-10 |
| P24 — Make observability semantic, not merely operational | OBS-01–OBS-06, OBS-11–OBS-12, PHY-11, SRC-03–SRC-05, TST-11 |
| P25 — Make testing derive from contracts and invariants | TST-01–TST-14 and every feature pattern’s evidence column |

## 24. Feature-family-to-principle crosswalk

| Feature family | Primary principles advanced | Typical requirement/building-block classes |
| --- | --- | --- |
| Arrow schema/types/metadata | P1, P3, P7, P8, P12, P21, P22, P25 | data models, schema contracts, typed interfaces, nested/domain types, protocol schemas |
| Arrow arrays/buffers/kernels | P7, P8, P11, P14, P22, P23, P25 | calculations, batch transforms, zero-copy data movement, streaming |
| RecordBatch/readers/streams | P7, P8, P16, P22, P23, P24, P25 | source scans, pipelines, RPC, sinks, bounded execution |
| Expr and built-in functions | P1, P2, P6, P14, P15, P16, P25 | filters, projections, calculations, validation expressions |
| UDF families | P2, P4, P14, P15, P19, P20, P25 | domain kernels, async calculations, aggregates, windows, lambdas, table functions |
| DFSchema/constraints/FDs/statistics | P3, P12, P15, P20, P21, P25 | binding, compatibility, relational reasoning, optimizer metadata |
| Catalog/schema/table providers | P3, P4, P5, P7, P13, P20, P23, P25 | data sources, namespace, governance, write authority |
| LogicalPlan/planner/optimizer | P2, P6, P14, P15, P16, P17, P18, P24, P25 | query models, plan specs, policy validation, optimization |
| ExecutionPlan/physical optimizer | P6, P11, P15, P16, P17, P20, P23, P24, P25 | custom algorithms, distribution/order, streaming execution |
| Parquet/FileSource/object_store | P5, P7, P8, P12, P15, P20, P22, P23, P24, P25 | lake/file sources, pruning, remote storage, schema adaptation |
| Session/runtime/task contexts | P3, P4, P6, P13, P16, P19, P23, P24, P25 | tenancy, configuration, resources, query/task state |
| Proto/Substrait/IPC/Flight/C interfaces | P2, P7, P9, P10, P17, P18, P19, P22, P25 | plan artifacts, cross-engine/language/process integration |
| Explain/metrics/tracing | P9, P10, P17, P19, P24, P25 | semantic observability, provenance, regression and performance diagnosis |

## 25. Preparation for the next-stage “functional building block” catalogue

The next design exercise can define an exhaustive set of functional capabilities—such as ingest, project, filter, join, aggregate, validate, version, query, write, expose, authorize, observe, or interoperate—and map each building block to this manual. Each building block should eventually carry at least:

```yaml
functional_building_block:
  id: ...
  semantic_purpose: ...
  input_contracts: ...
  output_contracts: ...
  authority_touched: ...
  lifecycle_phases: ...
  selected_utilization_patterns:
    - MOD-..
    - ARR-..
    - SCH-..
    - CAT/EXP/LOG/PHY/SRC/RUN/INT/OBS/GOV/EXT/TST-..
  legal_variation: ...
  optimizer_visibility: ...
  provenance_outputs: ...
  capability_claims: ...
  state_and_resource_scope: ...
  interoperability_boundaries: ...
  test_evidence: ...
```

This keeps the future capability catalogue **functional** while this document remains the architectural and library-utilization authority.

---

# Part VI — Comprehensive agent review checklist

## 26. Semantic and authority review

- [ ] Important meaning is represented in a typed application model or a documented reason says why no model is needed.
- [ ] One authority is named for every schema, calculation, plan request, table contract, policy, configuration set, and provenance record.
- [ ] Arrow/DataFusion objects are classified as authorities or derived compiled/runtime forms.
- [ ] Derived representations carry authority identity/version/fingerprint.
- [ ] No cache, debug string, SQL rendering, physical plan, or metadata tag silently becomes an alternative authority.

## 27. Arrow data-fabric review

- [ ] Arrow `Schema`, arrays, and `RecordBatch` are the default tabular boundary.
- [ ] Row materialization and non-Arrow conversions are explicitly justified and inventoried.
- [ ] Streaming readers/streams are used for unbounded or large results.
- [ ] Null, dictionary, view, nested, timestamp, decimal, and extension semantics are explicit.
- [ ] Copy and ownership behavior is documented at every boundary.
- [ ] Built-in kernels are reviewed before custom loops.

## 28. Schema-contract review

- [ ] Schema version, fingerprint, field identity, names, types, nullability, nested structure, ordering, annotations, and compatibility are defined.
- [ ] Source/physical schema is separated from canonical table schema.
- [ ] `DFSchema` qualifiers and functional dependencies are derived, not competing authorities.
- [ ] Projection field order and hidden/filter-only fields are tested.
- [ ] Runtime batches are validated against stream/plan schema.
- [ ] Metadata semantic classes and consumers are documented.
- [ ] IPC/Parquet/FFI round trips preserve all required schema semantics.

## 29. Calculation and optimizer review

- [ ] Built-in Arrow/DataFusion functionality and transparent expression builders were reviewed first.
- [ ] The selected function family matches scalar/async/lambda/aggregate/window/table semantics.
- [ ] Signature, coercion, return field, null policy, strictness, volatility, ordering, bounds, and simplification are truthful.
- [ ] UDF state is mergeable, memory-accounted, and resettable where applicable.
- [ ] `EXPLAIN` proves important predicates/columns/order remain visible.
- [ ] Function package and registry versions are in provenance/cache dependencies.

## 30. Provider/source review

- [ ] Provider schema is stable and cheap during a query.
- [ ] Backend variation is contained within provider/source/store/adapters.
- [ ] Projection, filters, limits, statistics, and DML have explicit contracts.
- [ ] Exact/inexact/unsupported pushdown is truthful per predicate.
- [ ] Scan planning is cheap; I/O occurs in execution/stream polling.
- [ ] Source snapshot/version and schema fingerprint are available or reproducibility is downgraded explicitly.
- [ ] File/Parquet pruning is distinguished from exact row filtering.

## 31. Logical/physical planning review

- [ ] All entry paths converge on `LogicalPlan`.
- [ ] Binding, policy, logical optimization, physical planning, and physical optimization are separate phases.
- [ ] Custom logical meaning uses a typed logical extension before custom physical code.
- [ ] Every custom `ExecutionPlan` correctly implements expression traversal, child replacement, properties, requirements, statistics, state, metrics, and streaming.
- [ ] Distribution and ordering claims are conservative and tested.
- [ ] Memory, spill, cancellation, and repeated execution are designed.
- [ ] Plan serialization is used only within its compatibility contract.

## 32. Governance and state review

- [ ] Policy is enforced at catalog/schema/table/function/plan/write/resource authority boundaries.
- [ ] Direct provider, DataFrame, serialized-plan, and DML paths cannot bypass policy.
- [ ] Session/runtime/query/task/operator state scopes are explicit.
- [ ] Credentials, object stores, memory pools, spill, and caches are scoped correctly.
- [ ] Cache keys include every authority dependency and have explicit invalidation.
- [ ] Mutable runtime state cannot become semantic authority.

## 33. Provenance, observability, and reproducibility review

- [ ] Execution identity is allocated before planning.
- [ ] Semantic spec, schema, policy, function registry, config, source snapshots, plans, environment, metrics, and output identity are linked.
- [ ] Logical and physical plan artifacts are captured with version context.
- [ ] Semantic observability includes pushdowns, source/file actions, and schema/plan fingerprints.
- [ ] Reproducibility status explicitly records volatility and missing pins.
- [ ] Provenance closure can be traversed from durable output.
- [ ] Redaction and artifact-retention policy are defined.

## 34. Test-evidence review

- [ ] Every capability claim maps to a test.
- [ ] Optimized/unoptimized and serialized/deserialized results are compared.
- [ ] Provider pushdown and physical properties have adversarial tests.
- [ ] Stateful functions/operators have partition, merge, reset, memory, and cancellation tests.
- [ ] Protocol boundaries have cross-language/version fixtures.
- [ ] Malformed input and FFI/unsafe paths are fuzzed where relevant.
- [ ] Version/feature/dependency CI rejects duplicate Arrow/DataFusion universes.

---

# Part VII — Anti-pattern diagnosis and prescribed correction

| Anti-pattern | Arrow/DataFusion symptom | Why it violates the constitution | Prescribed correction |
|---|---|---|---|
| Hidden semantic logic | business rule implemented in provider scan branches or stream polling | meaning cannot be inspected, compiled, or reused | create typed spec and compile to `Expr`/`LogicalPlan` |
| Multiple authorities | schema duplicated in Arrow, SQL, config, and writer structs | independent drift | one `SchemaContract`; derive all native forms |
| Backend leakage | consumers downcast providers or branch on source type | destroys substitutability/fabric | localize in `TableProvider`/`FileSource`/adapter |
| Opaque abstraction | simple predicate wrapped in UDF | hides optimizer semantics | expression builder using built-ins |
| Premature physicalization | requirement names partition count or exec node | blocks optimization and portability | logical intent first; physical policy downstream |
| Provenance afterthought | only logs and SQL retained | no closure or reliable replay | create artifact/provenance envelope before planning |
| Mutable authority | shared schema/catalog object changes mid-query | unstable meaning | immutable versioned snapshot and explicit transition |
| Metadata theater | tag claims unit/security/constraint without consumer | false assurance | classify metadata; add enforcement or mark advisory |
| Pairwise integration explosion | custom DTO conversion for every component pair | no common fabric | Arrow/protocol boundary |
| Capability overclaiming | exact pushdown/order/stats not guaranteed | may produce wrong results | conservative exact/inexact/absent declarations |
| State-scope collapse | global mutable context holds tenant/query/operator state | leakage and unreproducibility | use Session/Runtime/Task/Planning/operator scopes |
| Eager materialization | `collect()` at every boundary | memory/cancellation loss | readers and batch streams |
| Unstable identity | plan display/hash used as durable ID | version/format drift | application canonicalization + version namespace |
| Test-by-method | tests mirror functions but not contracts | gaps survive refactors | derive evidence from invariants/capability matrix |

---

# Part VIII — Compact LLM-agent instruction block

> **Use Arrow and DataFusion as a semantic compiler and common data fabric, not merely as utility libraries.** Begin from explicit application-owned semantic authorities, validate and bind them, then compile them into Arrow schemas/data and DataFusion expressions/logical plans. Keep domain and relational meaning separate from physical execution strategy. Use catalog/provider and function hierarchies as authority and variability boundaries; consumers should not branch on backend type.
>
> Select the highest-level built-in or extension point that fully expresses the semantics. Prefer Arrow kernels and transparent DataFusion expressions before UDFs; choose the correct scalar/aggregate/window/higher-order/table function family; use providers for sources; use logical extensions before physical operators; replace the global planner only as a last resort. Preserve optimizer visibility through built-in expressions, truthful UDF hooks, provider pushdown contracts, statistics, constraints, functional dependencies, and physical properties.
>
> Treat Arrow `Schema`, arrays, `RecordBatch`, readers, and streams as infrastructure. Treat schemas as executable contracts with explicit identity, version, fingerprint, compatibility, nullability, metadata class, and evolution policy. Validate provider, plan, stream, batch, and protocol schemas. Do not silently widen, reorder, change nullability, or treat metadata as enforcement.
>
> Use DataFusion lifecycle phases explicitly: resolve/bind, validate, compile to `LogicalPlan`, optimize, authorize, lower with `PhysicalPlanningContext`, physically optimize, execute as Arrow streams, validate outputs, and observe. Capture inspectable artifacts at each material phase. Custom `ExecutionPlan`s must truthfully implement schema/properties, expression traversal, child replacement, distribution/order requirements, statistics, state reset, memory/spill, metrics, serialization posture, streaming, and cancellation.
>
> Make provenance and reproducibility application-owned but native to the flow. Before execution allocate identity and record semantic/schema/policy/function/config/source/environment fingerprints; capture logical and physical plans, metrics, output schema, and result identity; link them through stable references. Treat DataFusion plan serialization and hashes as version-coupled unless an explicit compatibility contract says otherwise.
>
> Be conservative: unknown is safer than false. Exact/inexact/unsupported pushdown, statistics precision, ordering, partitioning, constraints, nullability, strictness, volatility, determinism, and idempotency must be truthful and tested. Classify metadata as enforced, planner-consumed, contractual, governance, lineage, or advisory and name its consumer.
>
> Derive tests from contracts. Every claimed property must have positive, negative, adversarial, serialization/interoperability, resource/state, and upgrade evidence as applicable. Do not implement until semantic authority, legal variation, lifecycle, state scope, provenance closure, feature-utilization pattern IDs, and test evidence are explicit.

---


# Appendix A — DataFusion 55 and Arrow 59 version-specific leverage map

This appendix identifies features that are especially important in the pinned 55/59 environment. They do not replace the general patterns above; they sharpen how those patterns should be implemented against these versions.

## A.1 DataFusion 55 features with direct design-principle value

| DataFusion 55 capability | How it should be leveraged | Principles advanced | Required caution/evidence |
|---|---|---|---|
| `PhysicalPlanningContext` | Keep scalar-subquery result slots and lambda-variable qualification in a subtree-scoped physical-planning context; forward it through extension planners and physical-expression creation. | P6, P16, P23 | Do not reconstruct this state in `ExecutionProps` or globals; test nested scalar subqueries and lambdas. |
| Required `ExecutionPlan::apply_expressions` | Make every physical expression root owned/evaluated by a custom node traversable by optimizer, serialization, diagnostics, and dynamic-expression logic. | P15, P17, P25 | Shallow traversal must include all owned roots but not child-plan expressions; add count/rewrite tests. |
| `ExecutionPlan::replace_children` | Treat physical plan rewrites as immutable reconstruction with explicit property recomputation/preservation policy. | P6, P11, P20 | The deprecated compatibility `with_new_children` remains part of the trait surface; delegate it to the modern replacement contract and test both property modes. |
| `StatisticsContext`, `child_stats_requests`, `StatisticsArgs`, `statistics_from_inputs` | Compute statistics bottom-up, request only needed child/partition statistics, and memoize within a walk. | P15, P20, P23 | The default child request skips children; custom nodes reading child stats must request them explicitly. |
| `scan_with_args`, `ScanArgs`, `StatisticsRequest` | Use structured provider scan arguments and permit custom optimizer/provider cooperation on cheap granular statistics. | P5, P15, P16, P20 | Built-in providers may ignore granular requests; custom providers must ignore anything they cannot answer cheaply/truthfully. |
| Range partitioning and `InputDistributionRequirements::co_partitioned` | Represent range boundaries and compatible child layouts explicitly for range-aware exchanges and joins. | P6, P15, P20 | Split points, sort options, types, and partition counts must be compatible; test exact boundary and null-order semantics. |
| Unified `EnsureRequirements` | Let physical optimization coordinate distribution and sorting so they do not invalidate each other; inspect resulting repartition/sort/merge plans. | P6, P15, P17, P24 | Custom nodes must state input requirements and output properties accurately; verify idempotence and parallel sort behavior. |
| Scalar-UDF `is_strict` | Expose true null propagation so nullability and null-rejecting/outer-join reasoning remain visible. | P15, P19, P20 | Default false is conservative; opt in only when every argument-null case produces null. |
| Dictionary encoding preservation in typed coercion | Preserve low-cardinality dictionary inputs only when the UDF implementation handles dictionary physical types and benefits from them. | P8, P15, P20 | Coerced argument and return types change; test dictionary and materialized paths. |
| `DynamicFilterTracking` / tracker APIs | Classify dynamic predicates, detect changes cheaply, and coordinate scan/pruning reevaluation. | P15, P20, P23, P24 | Dynamic filters require state-reset and completion/update tests; do not infer dynamic status from deprecated helpers. |
| `PruningPredicateBuilder` and optional `FilePruner` | Build pruning with explicit file schema and accept that no useful pruner exists without usable statistics or dynamics. | P15, P20 | Pruning is an optimization, not exact filtering; test no-stat and type-coercion cases. |
| `file_row_index()` source-dependent expression | Provide file-relative row provenance/identity as a source-rewritten virtual expression that moves toward scans. | P9, P12, P15 | It is not globally unique and errors outside a supporting file context; pair with file identity where durable row identity is required. |
| `MERGE INTO` logical types and `TableProvider::merge_into` | Keep merge predicate/clauses/assignments as logical semantics and delegate target-specific mutation to the table authority. | P1, P6, P13, P16 | Generic DataFusion types do not supply transactional semantics; target/provider implementation and audit remain application/storage responsibilities. |
| `UnnestOptions::NullHandling` | Model `NULL` and empty-list cardinality behavior explicitly (`Drop`, `Preserve`, `PreserveAndExpandEmpty`). | P1, P12, P20, P25 | Cardinality semantics must be part of the contract and tested separately for null and empty lists. |
| Mandatory `GroupsAccumulator::convert_to_state` and revised merge contract | Implement the high-cardinality partial-aggregation bypass path explicitly; merge intermediate state without a raw-row filter. | P4, P14, P25 | State arrays, filters, emit order, memory, and FFI compatibility must be tested. |
| Pluggable `SpillFile` / `TempFileFactory` | Put spill backend variability behind runtime resource contracts instead of hard-wiring local temp files. | P5, P23, P24 | Spill state is operational, not semantic authority; account size, errors, cleanup, and custom disk-manager modes. |
| File-stream work stealing configuration | Treat cross-partition file reassignment as a physical optimization that must be disabled when order or declared file-group partitioning is semantically/operationally required. | P6, P20, P23 | Distributed executors that poll isolated partitions require explicit tests and usually disable stealing. |
| Physical-plan self-serialization hooks | Let supported self-contained physical nodes serialize through native hooks while preserving fallback codecs for others. | P17, P18, P22 | Feature-gated and version-coupled; capture session/provider dependencies separately. |
| Multiple external-table locations | Represent a multi-location table as one logical contract only when schemas and object-store authority are compatible. | P3, P5, P12 | Location lists are source configuration, not multiple semantic authorities; validate schema and store consistency. |

## A.2 Arrow 59 features with direct design-principle value

| Arrow 59 capability/change | How it should be leveraged | Principles advanced | Required caution/evidence |
|---|---|---|---|
| Canonical extension-type support | Prefer standardized Arrow logical types such as UUID/JSON/tensor forms where they match domain meaning, retaining valid storage types for unknown consumers. | P7, P12, P21, P22 | Gate the feature, version metadata, and test known/unknown IPC/Parquet/FFI consumers. |
| Custom `ExtensionType` APIs | Encode domain logical meaning in field metadata with storage-type validation rather than inventing incompatible physical arrays. | P1, P12, P21 | Metadata is not automatic calculation/governance enforcement. |
| Improved nested and dictionary casting | Preserve columnar encodings and nested schema semantics through controlled cast paths instead of row materialization. | P8, P12, P15 | Cast policy remains application-owned; test nullability, nested fields, and dictionary order. |
| Product aggregate kernel and expanded compute surfaces | Reuse vectorized built-ins before custom loops/UDFs. | P8, P14 | Verify numeric overflow/null semantics for the contract. |
| CSV header validation against supplied schema | Treat text ingestion as validation against an authority rather than schema inference alone. | P12, P16, P25 | CSV remains a weakly typed boundary; test malformed headers and conversion failures. |
| Configurable IPC writer compression and sans-I/O stream encoding | Separate Arrow message encoding from transport/backpressure and make compression part of an explicit protocol profile. | P7, P22, P23 | Record codec/profile versions and test fragmented/streaming transport. |
| Stronger FFI alignment and metadata preservation fixes | Use C Data/C Stream boundaries as first-class infrastructure while validating ownership, release, alignment, and metadata semantics. | P8, P22, P25 | FFI remains a high-risk boundary; retain malformed-input and lifetime tests. |
| `FixedSizeBinaryArray` fallible construction (`TryFrom`) | Propagate construction errors as contract-validation failures instead of assuming infallible data shape. | P12, P16, P20 | Audit older `From` call sites and test invalid widths. |
| `RowSelection` backed by `BooleanBuffer` and improved mask filtering | Keep Parquet row-selection/filter representations Arrow-native and compositional. | P8, P15 | Selection remains physical pruning/filter machinery; preserve exact semantic predicates separately. |
| `force_validate` and controlled unchecked builder methods | Use additional validation in tests/fuzzing/unsafe boundaries; permit unchecked hot paths only behind proven invariants. | P20, P25 | Never use unchecked APIs to bypass contract validation for convenience. |
| `StructArray::field_*` symmetry and builder capacity accessors | Build generic schema-aware/nested tooling without hand-maintained column/field parallel logic and with deliberate capacity planning. | P1, P8, P12 | Maintain field/array positional invariants. |
| Parquet `object_store` bridge deprecation direction | Keep object-store operations in a storage adapter or DataFusion datasource layer; use Parquet async reader/writer traits for custom low-level services. | P5, P7, P22 | Do not deepen coupling to deprecated `ParquetObjectReader`/`Writer` in new architecture. |
| Stronger JSON/map/nullability validation | Treat malformed nested inputs as boundary contract failures rather than tolerating invalid Arrow state. | P12, P20, P25 | Include nullable-map-key and nested-nullability negative fixtures. |
| Performance improvements for nested lists, decimals, map/list interleave, strings, ZSTD, and Flight buffers | Benefit automatically when canonical Arrow types and kernels are preserved; avoid conversions that erase these gains. | P7, P8, P14 | Benchmark representative workloads rather than assuming every encoding wins. |

## A.3 Version-specific dependency invariant

For code that directly imports public Arrow/DataFusion types, maintain one coherent type universe:

```toml
[dependencies]
datafusion = "=55.0.0"
arrow = "=59.2.0"
arrow-array = "=59.2.0"
arrow-schema = "=59.2.0"
parquet = "=59.2.0"
object_store = "=0.13.2"
```

Use the narrowest direct dependency set your public interfaces require, but reject duplicate Arrow/DataFusion majors crossing public boundaries. Record the resolved dependency graph and feature set in `OBS-07`, and enforce it through `TST-14`.

# Closing maxim

> **Model the truth once; compile it through Arrow and DataFusion; keep the optimizer able to see it; execute it through truthful contracts; and preserve enough identity, state, evidence, and lineage to explain it forever.**
