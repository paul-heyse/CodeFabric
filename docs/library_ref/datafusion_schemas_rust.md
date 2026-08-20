## High-level assessment

The attached documentation already has **strong schema coverage**, but it is distributed across many chapters rather than organized as a single schema lifecycle. It covers Arrow `Schema` / `Field` / `DataType`, `RecordBatch` schema validation, `DFSchema` qualification and equivalence, SQL type mapping, DDL-driven schema creation, external-table schema inference, nested schemas, catalog/schema/table hierarchy, `TableProvider::schema()`, and schema-related testing/governance patterns. The main gap is not “missing basics”; it is the absence of a **schema-first architecture spine** that ties creation, inference, cataloging, evolution, provider contracts, logical-plan propagation, runtime batch validation, and governance into one coherent deployment model.

Current DataFusion docs align with that interpretation: `DFSchema` is the logical schema wrapper around Arrow schema plus relation qualification, while `SchemaProvider` and `TableProvider` are the catalog/provider surfaces through which table schemas are exposed and resolved during planning. ([Docs.rs][1]) ([Docs.rs][2]) ([Docs.rs][3])

---

## What the attached document already details well

| Schema area                                 | Current coverage quality | Evidence                                                                                                                                                                                                                                       |
| ------------------------------------------- | -----------------------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Arrow physical schema model**             |                   Strong | The document explicitly models the physical layer as `RecordBatch → Arc<Schema> + Vec<ArrayRef>`, with `ArrayRef = Arc<dyn Array>`, and separates that from the logical `DFSchema` layer.                                                      |
| **`DataType`, `Field`, `Schema` creation**  |                   Strong | It covers primitive, timestamp, decimal, dictionary, list, and struct `DataType`s; `Field` name/type/nullability/metadata; and `Schema` as ordered fields plus metadata.                                                                       |
| **`DFSchema` vs Arrow `Schema`**            |    Strong, but scattered | It distinguishes Arrow `Schema` for physical data from `DFSchema` for planning, expression resolution, joins, SQL binding, and ambiguity detection.                                                                                            |
| **Qualifiers and ambiguity resolution**     |                   Strong | It describes qualifiers such as `table.column`, `schema.table.column`, and `catalog.schema.table.column`, plus ambiguity-safe lookup patterns after joins.                                                                                     |
| **Schema equality / compatibility helpers** |                 Moderate | It lists `matches_arrow_schema`, `has_equivalent_names_and_types`, `datatype_is_logically_equal`, `datatype_is_semantically_equal`, `strip_qualifiers`, and `replace_qualifier`, but does not build this into a full compatibility framework.  |
| **SQL type mapping to Arrow types**         |                   Strong | It maps SQL type declarations into Arrow `DataType`s, including `Utf8View`, decimal precision limits, temporal types, booleans, binary, and unsupported SQL types.                                                                             |
| **DDL and catalog mutation**                |                   Strong | It models SQL DDL as catalog mutation through `CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider`, and lists current DDL support.                                                                                         |
| **External-table schema inference**         |                 Moderate | It covers `CREATE EXTERNAL TABLE`, Parquet schema inference, CSV inference, explicit schema, and `LOCATION`, but needs more edge-case depth across formats and directories.                                                                    |
| **Nested schemas**                          |        Strong foundation | It covers `ARRAY` / `LIST`, `STRUCT`, maps, nested Parquet/Arrow I/O, nested field access, and nested design patterns.                                                                                                                         |
| **Catalog/schema/table management**         |                   Strong | Section 17 provides the hierarchy, default catalog/schema behavior, `information_schema`, custom catalog patterns, remote catalog caveats, security, and testing.                                                                              |
| **`TableProvider` schema contract**         |        Strong foundation | The custom provider section identifies `schema()` as the authoritative logical output schema and ties projection indices and stream schema correctness to that contract.                                                                       |

---

## Core gap diagnosis

The largest missing layer is a **schema lifecycle architecture chapter**. The current document explains the pieces, but an agent still has to infer how schemas move through the system:

```text
source file / in-memory batch / remote table
  → inferred or declared Arrow Schema
  → TableProvider::schema()
  → catalog/schema/table registration
  → DFSchema with qualifiers
  → LogicalPlan output schema
  → optimized LogicalPlan schema preservation
  → physical ExecutionPlan schema
  → RecordBatch runtime schema validation
  → sink / Parquet / Arrow / JSON / SQL result schema
  → information_schema / DESCRIBE / audit / tests
```

Without that spine, schema rules are correct locally but hard to apply globally. That matters for your likely use case because agents need deterministic contracts: schema creation, schema qualification, plan schema propagation, provider output validation, type coercion, evolution policy, metadata preservation, and catalog governance all need to be coordinated rather than learned section-by-section.

---

# Proposed schema deep-dive backlog

## S1) Schema lifecycle and invariants across DataFusion

**Purpose:** make schemas a first-class lifecycle artifact, not a scattered concept.

**Current attachment coverage:** Strong ingredients exist: Arrow/DFSchema stack, DDL catalog graph, catalog hierarchy, provider schema contract, and plan/batch execution model.  

**Gap to fill:** A single end-to-end schema flow from source declaration/inference through planning, optimization, execution, writing, introspection, and tests.

**Deep dive:**

* Schema lifecycle diagram:

  * declared SQL schema
  * inferred file schema
  * explicit Arrow `Schema`
  * `TableProvider::schema()`
  * `DFSchema`
  * `LogicalPlan::schema()`
  * physical plan schema
  * `RecordBatch::schema()`
  * sink/output schema
* Hard invariants:

  * provider schema must match execution output
  * logical schema qualifiers do not survive Arrow conversion
  * output field names must be deterministic
  * RecordBatch array order/type/nullability must match schema
* Schema ownership:

  * source-owned schema
  * catalog-owned schema
  * provider-owned schema
  * query-derived schema
  * sink-owned schema
* Agent contract:

  * where to validate
  * where to normalize
  * where to attach metadata
  * where to reject schema drift

---

## S2) Schema creation surfaces and factory patterns

**Purpose:** enumerate every supported schema creation path and define when each is appropriate.

**Current attachment coverage:** It covers Rust `Schema::new`, `Field::new`, SQL DDL schemas, `read_batch`, external-table inference, and provider schemas, but not as a single creation matrix.  

**Deep dive:**

* Rust-native creation:

  * `Schema::new`
  * `Schema::new_with_metadata`
  * `Field::new`
  * nested `Field` constructors
  * `RecordBatch::try_new`
* SQL DDL creation:

  * `CREATE EXTERNAL TABLE (...)`
  * `CREATE TABLE AS SELECT`
  * `CREATE VIEW`
  * `CREATE SCHEMA`
* Source-driven creation:

  * Parquet footer schema
  * Arrow IPC schema
  * Avro schema
  * CSV/JSON inference
  * Hive partition-derived columns
* Provider-driven creation:

  * `TableProvider::schema()`
  * `ListingTable` provided schema
  * dynamic/remote schema snapshots
* Derived schemas:

  * projection output
  * aggregate output
  * join output
  * union/coercion output
  * nested `unnest` output
* Factory policy:

  * `schema_factory::from_sql_decl`
  * `schema_factory::from_arrow`
  * `schema_factory::from_provider`
  * `schema_factory::from_contract`
  * `schema_factory::for_sink`

---

## S3) Schema inference, explicit overrides, and multi-file drift

**Purpose:** make file/dataset schema inference predictable and auditable.

**Current attachment coverage:** Good baseline for Parquet and CSV; less complete for multi-file merge behavior, JSON/Avro edge cases, schema drift, and directory-level compatibility. 

**Deep dive:**

* Format inference matrix:

  * CSV sampling
  * JSON inference
  * Parquet metadata
  * Avro schema
  * Arrow IPC schema
* Directory datasets:

  * compatible schemas
  * file-level schema mismatch
  * nullability widening
  * type conflict
  * missing/extra columns
  * partition-column discovery
* Explicit override policy:

  * when to supply explicit schema
  * when to trust Parquet footer
  * when to disable inference
  * when to use all-`Utf8` staging
* Failure taxonomy:

  * inconsistent file schema
  * incompatible partition schema
  * unsupported logical type
  * inferred wrong numeric type
  * nullable/non-nullable mismatch
* Agent tests:

  * `DESCRIBE`
  * `arrow_typeof`
  * `df.schema()`
  * first-batch schema vs full dataset schema
  * negative tests for drift

---

## S4) Naming, identifier normalization, qualifiers, and output field names

**Purpose:** define a deterministic naming policy across SQL, DataFrame, Arrow, and catalog layers.

**Current attachment coverage:** It covers SQL lowercasing/quoted identifiers and DFSchema qualifiers, but a dedicated schema naming standard would prevent many agent-generated-plan errors. 

**Deep dive:**

* Naming policy:

  * lowercase snake_case physical columns
  * quoted identifiers only for external compatibility
  * alias capitalized source fields immediately
  * avoid spaces/special characters in stable contracts
* Qualifier policy:

  * table alias vs table name
  * schema-qualified table names
  * catalog-qualified table names
  * multi-tenant qualification
* Output names:

  * computed expression aliases
  * aggregate aliases
  * window aliases
  * join duplicate-column handling
  * `SELECT *` expansion rules
  * `unnest` field naming
* DataFrame/Expr naming:

  * `col("x")`
  * `col("t.x")`
  * `ident("A")`
  * alias propagation
* Anti-patterns:

  * unqualified `id` after join
  * expression-derived output names
  * relying on case-preserving unquoted SQL
  * duplicate aliases
  * stripping qualifiers too early

---

## S5) Type compatibility, coercion, and schema equality

**Purpose:** move from “these types exist” to “these schemas are compatible under these policies.”

**Current attachment coverage:** SQL-to-Arrow type mapping and DFSchema equality helpers are covered, but there is no end-to-end compatibility decision framework.  

**Deep dive:**

* Compatibility levels:

  * physical exact equality
  * logical equality
  * semantic equality
  * coercible equality
  * user-facing contract equality
* Type coercion cases:

  * integer widening
  * float promotion
  * decimal precision/scale
  * `Utf8` / `Utf8View` / `LargeUtf8`
  * dictionary strings
  * timestamp units/time zones
  * list element coercion
  * struct field-name mapping
* Operator-specific schema compatibility:

  * `UNION`
  * `JOIN`
  * `CASE`
  * `COALESCE`
  * aggregate outputs
  * CTAS/view outputs
* Validation API:

  * `DFSchema` equality helpers
  * `ExprSchemable::get_type`
  * `ExprSchemable::nullable`
  * `Schema::try_merge`
  * `RecordBatch::try_new`
* Agent output:

  * compatibility report
  * proposed casts
  * lossy vs lossless conversion flag
  * rejected fields

---

## S6) Schema evolution and migration lifecycle

**Purpose:** define how schemas can change safely over time.

**Current attachment coverage:** Schema evolution is mentioned, but not treated as a standalone lifecycle discipline. The current doc says to use explicit compatibility checks and choose physical/logical/semantic equivalence deliberately, but this is only a short deployment note. 

**Deep dive:**

* Evolution actions:

  * add nullable column
  * add non-nullable column with default
  * drop column
  * rename column
  * reorder columns
  * widen type
  * narrow type
  * change nullability
  * change metadata
  * evolve nested struct field
  * evolve list element type
  * evolve partition columns
* Compatibility policy:

  * backward compatible
  * forward compatible
  * read compatible
  * write compatible
  * query compatible
  * Parquet/Arrow compatible
* Migration artifacts:

  * schema version
  * compatibility report
  * migration plan
  * backfill expression
  * rejected migration reason
* Testing:

  * old data with new schema
  * new data with old query
  * mixed-partition schema
  * old view over new table
  * CTAS output stability
* Agent rules:

  * never infer compatibility from names alone
  * never silently tighten nullability
  * never reorder physical columns without explicit mapping
  * always emit migration diffs

---

## S7) Schema metadata, Arrow extension types, and semantic annotations

**Purpose:** clarify which metadata is preserved, ignored, propagated, or consumed.

**Current attachment coverage:** It shows `Field::with_metadata` and schema-level metadata, but does not establish metadata governance or extension-type semantics. 

**Deep dive:**

* Metadata locations:

  * schema metadata
  * field metadata
  * Parquet key-value metadata
  * Arrow IPC metadata
  * DataFusion expression alias metadata
* Arrow extension type conventions:

  * extension name
  * extension metadata
  * logical vs physical type
  * downstream compatibility
* Propagation questions:

  * projection
  * alias
  * cast
  * join
  * aggregate
  * CTAS
  * Parquet write
  * Arrow IPC write
* Semantic annotations:

  * unit of measure
  * semantic type
  * source lineage
  * quality flag
  * confidentiality class
  * display format
  * domain enum
* Agent rules:

  * metadata is not optimizer semantics unless explicitly consumed
  * do not rely on metadata preservation without tests
  * expose metadata preservation matrix by operator/sink

---

## S8) Constraints, functional dependencies, defaults, and table contracts

**Purpose:** make table schema more than column names and types.

**Current attachment coverage:** `DFSchema` functional dependencies and `TableProvider` constraints are mentioned, but not integrated into a schema contract model. The current official `TableProvider` surface also includes `constraints()` and `statistics()` as planning-relevant metadata. ([Docs.rs][3])

**Deep dive:**

* Schema contract components:

  * fields
  * nullability
  * constraints
  * functional dependencies
  * uniqueness
  * primary-key-like metadata
  * default values
  * generated columns
  * check constraints
  * partition columns
  * ordering
* DataFusion support boundary:

  * what is enforced
  * what is optimizer metadata
  * what is provider-specific
  * what must be application-enforced
* Custom provider patterns:

  * declare constraints accurately
  * expose statistics safely
  * inject tenant predicates
  * enforce write constraints in `insert_into`
* Agent rules:

  * distinguish declared vs enforced constraints
  * do not advertise exact pushdown/constraints unless actually guaranteed
  * surface constraint violations as schema/contract diagnostics

---

## S9) Catalog schema management, remote metastores, and `information_schema`

**Purpose:** turn catalog/schema/table organization into an operational metadata system.

**Current attachment coverage:** This is one of the strongest existing areas: it covers catalog hierarchy, default catalog/schema, custom catalogs, remote metadata caching, `information_schema`, security, and deployment patterns. The gap is a more operational, migration-grade treatment.  

**Deep dive:**

* Namespace policy:

  * catalog per tenant
  * schema per workspace
  * schema per lifecycle layer
  * raw/curated/semantic/temp schemas
* Remote metastore pattern:

  * metadata snapshot
  * refresh interval
  * invalidation
  * stale-read semantics
  * async `table(name)` boundary
  * deterministic `schema_names` / `table_names`
* `information_schema`:

  * enable/disable
  * tables
  * columns
  * views
  * settings
  * leakage risks
  * filtered information schema
* DDL semantics:

  * read-only catalogs
  * mutable catalogs
  * concurrent DDL
  * audit log
  * schema ownership
  * object lifecycle
* Agent rules:

  * no network calls in hot planning loops
  * deterministic listing order for tests
  * qualified names for multi-tenant systems
  * explicit persistence semantics for every catalog provider

---

## S10) Custom `TableProvider` schema adaptation and projection mapping

**Purpose:** make custom providers safe under projection, filter pushdown, schema drift, and remote API schemas.

**Current attachment coverage:** Strong baseline: `schema()` is authoritative, projection indices come from that schema, and stream batches must match the projected schema. 

**Gap to fill:** Provider schema adaptation needs deeper treatment: dynamic schemas, remote schema caching, mapping requested projections to backend fields, converting backend types to Arrow types, and diagnosing mismatches.

**Deep dive:**

* Provider schema contract:

  * authoritative output schema
  * stable field order
  * cheap `schema()` call
  * no remote call in `schema()` unless cached
* Projection mapping:

  * projection index → field
  * backend field name
  * nested projection
  * physical vs logical projection
  * hidden columns
* Schema adaptation:

  * backend type → Arrow type
  * missing backend column
  * extra backend column
  * nullable mismatch
  * dictionary/string normalization
  * timestamp unit normalization
* Dynamic schemas:

  * snapshot schema
  * schema refresh
  * versioned provider
  * invalidation
  * query-start schema freeze
* Runtime validation:

  * stream schema equals projected schema
  * every batch validates
  * first batch absent/empty stream behavior
  * negative tests

---

## S11) Logical-plan schema propagation and operator output contracts

**Purpose:** define exactly how every logical operator transforms schema.

**Current attachment coverage:** The document has logical-plan material and notes schema preservation, but a schema-specific operator contract table would materially improve agent reliability.

**Deep dive:**

* Operator schema rules:

  * table scan
  * projection
  * filter
  * aggregate
  * window
  * sort
  * limit
  * join
  * union
  * distinct
  * repartition
  * unnest
  * extension node
* Output field naming:

  * aliases
  * expression display names
  * aggregate names
  * duplicate join names
  * unnest names
* Qualifier propagation:

  * scan qualifiers
  * projection qualifier retention/loss
  * join qualifier retention
  * alias qualifier changes
  * final output stripping
* Validation hooks:

  * `df.schema()`
  * `logical_plan().schema()`
  * optimized plan schema
  * physical plan schema
* Agent rules:

  * aliases are schema contracts
  * never depend on display-derived expression names
  * snapshot output schema alongside output rows
  * test schema after optimizer rewrites

---

## S12) Nested, partition, and virtual-column schemas

**Purpose:** consolidate all non-flat schema behavior.

**Current attachment coverage:** Nested data support is robust, including arrays, structs, maps, nested Parquet/Arrow reading/writing, introspection, and design guidance.   

**Gap to fill:** Partition columns and virtual columns should be treated as schema elements with lifecycle and compatibility rules.

**Deep dive:**

* Nested nullability:

  * null struct vs null field
  * null list vs empty list
  * null list element
  * missing map key vs null map value
* Nested evolution:

  * add struct field
  * reorder struct field
  * rename struct field
  * list element type widening
  * map key/value type change
* Partition schema:

  * Hive partition discovery
  * partition column type inference
  * partition/file column conflict
  * partition evolution
  * write-time `PARTITIONED BY`
  * keeping/dropping partition columns in file payload
* Virtual columns:

  * path-derived columns
  * tenant columns
  * row provenance columns
  * ingestion timestamp columns
* Agent tests:

  * `arrow_typeof(payload['x'])`
  * `DESCRIBE`
  * partition pruning explain
  * mixed partition schemas
  * nested Parquet round-trip

---

## S13) View, CTAS, and derived-table schema stability

**Purpose:** make derived catalog objects schema-stable.

**Current attachment coverage:** The DDL section covers `CREATE VIEW`, `DROP VIEW`, and CTAS as catalog objects, but not their schema-stability implications. 

**Deep dive:**

* View schema derivation:

  * output names
  * output types
  * nullability
  * qualifiers
  * dependency tracking
  * underlying table drift
* CTAS schema derivation:

  * inferred output field names
  * aliases required
  * in-memory default provider
  * durable custom provider behavior
* Stable view contracts:

  * explicit aliases
  * explicit casts
  * no `SELECT *`
  * versioned views
  * compatibility views
* Drift handling:

  * source column renamed
  * source type changed
  * source nullability changed
  * source column dropped
* Agent rules:

  * every view select item should be aliased
  * avoid `SELECT *` in stable views
  * snapshot view schema after creation
  * treat view schema as API surface

---

## S14) Schema testing, diagnostics, and error cookbook

**Purpose:** provide agents with deterministic schema validation and remediation paths.

**Current attachment coverage:** Testing and diagnostics exist, but schema tests are distributed across general testing, error handling, table provider tests, and catalog tests.

**Deep dive:**

* Schema test types:

  * Arrow schema equality
  * DFSchema equality
  * logical vs physical schema
  * RecordBatch schema
  * `DESCRIBE`
  * `information_schema.columns`
  * `arrow_typeof`
  * sqllogictest schema assertions
* Error taxonomy:

  * ambiguous column
  * missing column
  * duplicate field
  * incompatible type
  * invalid nullability
  * invalid RecordBatch
  * unsupported SQL type
  * schema inference failure
  * file schema drift
  * provider output mismatch
* Remediation:

  * qualify column
  * alias output
  * cast expression
  * widen nullability
  * add explicit schema
  * reject incompatible file
  * rewrite projection
* Agent artifacts:

  * schema diff report
  * compatibility matrix
  * suggested cast plan
  * generated failing fixture
  * golden schema snapshot

---

## S15) Schema security, governance, and tenant isolation

**Purpose:** treat schema visibility and metadata as security-sensitive.

**Current attachment coverage:** Catalog security and governance are discussed, including `TableProvider::schema()` governance hooks such as hiding unauthorized columns, renaming fields, exposing stable contracts, injecting tenant filters, and protecting sensitive statistics. 

**Deep dive:**

* Schema visibility:

  * column allowlist
  * hidden columns
  * masked columns
  * tenant-specific schemas
  * filtered `information_schema`
* Catalog isolation:

  * catalog per tenant
  * schema per workspace
  * object-store registry per credential domain
  * no direct URL table access
* Metadata leakage:

  * column names
  * table names
  * statistics
  * row counts
  * partition values
  * comments/metadata keys
* Audit schema:

  * user
  * tenant
  * catalog/schema/table
  * projected columns
  * filters
  * output row count
  * denied schema access
* Agent rules:

  * provider `schema()` is a security boundary
  * `information_schema` is not automatically safe
  * schema metadata may leak business semantics
  * statistics can be sensitive

---

## Recommended expansion order

1. **S1 — Schema lifecycle and invariants**
   This should become the root schema chapter.

2. **S2 + S3 — Schema creation and inference**
   These close the biggest practical gap for real datasets and agent-built providers.

3. **S4 + S5 — Naming and type compatibility**
   These prevent most agent-generated SQL/DataFrame schema bugs.

4. **S6 + S7 + S8 — Evolution, metadata, constraints**
   These turn schemas into governed contracts rather than passive Arrow objects.

5. **S9 + S10 + S11 — Catalog/provider/plan propagation**
   These are highest value for custom engines and semantic-platform work.

6. **S12 + S13 — Nested, partition, view, and CTAS schemas**
   These address the non-flat/lakehouse cases that become painful later.

7. **S14 + S15 — Testing, diagnostics, and governance**
   These make the schema system safe for LLM programming agents and production query services.

## Bottom line

The attached document already contains most of the raw schema facts. The gap is that schemas need a **dedicated schema-governance architecture**, with explicit lifecycle invariants, compatibility policies, evolution rules, provider contracts, and diagnostic artifacts. I would add the proposed schema topics as a new contiguous chapter block, likely after Section 4 or after Section 17, so agents can treat schema as a first-class compiled contract rather than a detail scattered across Arrow, SQL, catalogs, sources, plans, and providers.

[1]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html "DFSchema in datafusion::common - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.SchemaProvider.html "SchemaProvider in datafusion::catalog - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.TableProvider.html "TableProvider in datafusion::catalog - Rust"


# DataFusion Advanced — S1) Schema lifecycle and invariants across DataFusion

## S1.0 Objective

Treat schema as a **compiled contract** that crosses every DataFusion layer:

```text
external data / declared SQL / in-memory Arrow
  → Arrow Schema
  → catalog/table provider contract
  → DFSchema for logical planning
  → LogicalPlan output schema
  → optimized LogicalPlan output schema
  → ExecutionPlan output schema
  → RecordBatch runtime schema
  → sink/output schema
  → introspection + tests + diagnostics
```

The attached document already covers the main ingredients: Arrow `Schema` / `Field` / `DataType`, `RecordBatch`, `DFSchema`, schema qualifiers, catalog/provider hierarchy, and provider output requirements. This chapter’s added value is the **end-to-end invariant system**: where schemas originate, who owns them, where they are validated, how they are normalized, when metadata is preserved or discarded, and where drift must be rejected.  

DataFusion’s released docs.rs surface currently renders `datafusion 54.1.0`, uses Apache Arrow as the in-memory format, exposes SQL/DataFrame APIs, and allows customization across data sources, functions, operators, query languages, and planning/execution layers. ([Docs.rs][1])

---

## S1.1 Schema vocabulary: object taxonomy

```text
Arrow Schema
  = physical / interchange / runtime batch schema

DFSchema
  = logical planning schema
  = Arrow fields + optional qualifiers + logical resolution helpers

TableProvider::schema()
  = authoritative table/source contract exposed to planner

LogicalPlan::schema()
  = operator-derived logical output schema

ExecutionPlan::schema()
  = physical operator output schema communicated to physical optimizer/executor

RecordBatch::schema()
  = concrete runtime batch schema paired with actual Arrow arrays

Sink/output schema
  = persisted / serialized / API result schema after execution
```

Arrow `Schema` is metadata for an ordered sequence of field types and is not part of the physical memory layout; Arrow `Field` describes one column with name, data type, nullability, and custom metadata. ([Docs.rs][2]) ([Docs.rs][3])

`DFSchema` wraps Arrow schema information and adds relation/table qualification; it can contain qualified and unqualified fields, supports qualified schema creation, and can convert back to Arrow schema, at which point qualifier information is not retained as Arrow table/catalog metadata. ([Docs.rs][4])

`RecordBatch` is the runtime unit: a two-dimensional columnar dataset whose arrays must match the schema’s field count, field order, field data types, and equal column lengths. ([Docs.rs][5])

---

## S1.2 End-to-end schema lifecycle diagram

```text
┌────────────────────────────────────────────────────────────────────────────┐
│ 0. Source schema origin                                                    │
│                                                                            │
│    SQL DDL schema                                                          │
│      CREATE EXTERNAL TABLE t (id BIGINT, name VARCHAR, ...)                │
│                                                                            │
│    File-inferred schema                                                    │
│      Parquet footer / Arrow IPC schema / Avro schema / CSV sampling         │
│                                                                            │
│    Rust Arrow schema                                                       │
│      Arc<Schema> = Schema::new(vec![Field::new(...)])                      │
│                                                                            │
│    Dynamic provider schema                                                 │
│      remote DB/API/metastore snapshot                                      │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 1. Source registration / catalog binding                                   │
│                                                                            │
│    SessionContext::register_*                                              │
│    CREATE EXTERNAL TABLE                                                   │
│    SchemaProvider::register_table                                          │
│    CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider  │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 2. TableProvider contract                                                  │
│                                                                            │
│    TableProvider::schema() -> Arc<Schema>                                  │
│    TableProvider::scan(projection, filters, limit) -> ExecutionPlan         │
│                                                                            │
│    projection indices are interpreted against TableProvider::schema()       │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 3. Logical planning schema                                                 │
│                                                                            │
│    Arrow Schema → DFSchema                                                 │
│    add relation/table qualifiers                                           │
│    resolve col("x"), col("t.x"), aliases, casts, aggregates, joins          │
│    LogicalPlan::schema() records each operator output                      │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 4. Optimized logical plan                                                  │
│                                                                            │
│    projection pushdown                                                     │
│    filter pushdown                                                         │
│    expression simplification                                               │
│    join / aggregate rewrites                                               │
│                                                                            │
│    invariant: semantic schema contract preserved unless rewrite explicitly  │
│    changes projection / alias / output operator                            │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 5. Physical plan schema                                                    │
│                                                                            │
│    ExecutionPlan::schema() -> Arc<Schema>                                  │
│    ExecutionPlan::properties() -> PlanProperties                           │
│    physical expressions compiled against Arrow arrays                      │
│                                                                            │
│    invariant: physical output schema must match logical output schema       │
│    modulo planned projection/coercion/materialization boundaries            │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 6. Runtime streams                                                         │
│                                                                            │
│    ExecutionPlan::execute(partition, TaskContext)                          │
│      -> SendableRecordBatchStream                                          │
│      -> RecordBatch, RecordBatch, ...                                      │
│                                                                            │
│    invariant: every batch emitted by a stream must match stream/plan schema │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 7. Sink / output schema                                                    │
│                                                                            │
│    collect() -> Vec<RecordBatch>                                           │
│    execute_stream() -> RecordBatch stream                                  │
│    write_parquet / write_csv / write_json / write_table                    │
│    DataSink::schema() / TableProvider::insert_into                         │
│                                                                            │
│    invariant: persisted/API output schema is a deliberate contract          │
└────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────┐
│ 8. Introspection / validation / regression                                 │
│                                                                            │
│    df.schema()                                                             │
│    df.logical_plan()                                                       │
│    physical_plan.schema()                                                  │
│    DESCRIBE t                                                              │
│    information_schema.columns                                              │
│    arrow_typeof(expr)                                                      │
│    RecordBatch::try_new                                                    │
│    golden schema snapshots                                                 │
└────────────────────────────────────────────────────────────────────────────┘
```

The catalog hierarchy is explicitly `CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider`, and DataFusion’s catalog guide states that DDL such as `CREATE TABLE` is supported through this catalog API. ([Apache DataFusion][6])

---

## S1.3 Schema surfaces matrix

| Layer               | Schema object                           |                                         Owner |                             Mutability | Primary validation                 | Failure mode                                                |
| ------------------- | --------------------------------------- | --------------------------------------------: | -------------------------------------: | ---------------------------------- | ----------------------------------------------------------- |
| SQL DDL             | SQL column list                         |                      user / migration / agent |                    mutable through DDL | SQL planner + Arrow type mapping   | unsupported type, invalid nullability, duplicate column     |
| File source         | inferred Arrow `Schema`                 | file format / object store / listing provider |                       source-dependent | file metadata / inference / merge  | incompatible files, bad inference, partition conflict       |
| Rust in-memory data | `Arc<Schema>` + arrays                  |                              application code |           immutable after construction | `RecordBatch::try_new`             | field count/type/length/nullability mismatch                |
| Catalog             | schema/table namespace                  |          `CatalogProvider` / `SchemaProvider` |                       provider-defined | registration policy                | duplicate table, stale schema, unauthorized schema exposure |
| Table provider      | `TableProvider::schema()`               |                       provider implementation | should be stable per planning snapshot | provider tests + scan output tests | projected output mismatch                                   |
| Logical planner     | `DFSchema`                              |                            DataFusion planner |                                derived | binder/analyzer/type coercion      | missing/ambiguous column, type coercion failure             |
| Logical plan        | `LogicalPlan::schema()` / `DFSchemaRef` |                  DataFusion logical operators |                     immutable per plan | plan construction                  | invalid operator output schema                              |
| Physical plan       | `ExecutionPlan::schema()`               |                     physical planner/operator |                immutable per plan node | physical planning + invariants     | logical/physical mismatch                                   |
| Runtime stream      | `RecordBatch::schema()`                 |                     execution operator/stream |                  batch-level immutable | Arrow constructor + adapter checks | bad batch emitted                                           |
| Sink                | sink schema / output files              |                          writer/sink/provider |                           sink-defined | sink compatibility check           | persisted schema drift                                      |

DataFusion’s custom table-provider guide describes three cooperating layers: `TableProvider` describes the table and produces a plan, `ExecutionPlan` describes how to compute results, and `SendableRecordBatchStream` yields `RecordBatch` values during execution. ([Apache DataFusion][7])

---

## S1.4 Source-origin schemas

### S1.4.1 Declared SQL schema

```sql
CREATE EXTERNAL TABLE refinery_streams (
    stream_id       VARCHAR NOT NULL,
    case_id         VARCHAR NOT NULL,
    unit_id         VARCHAR NOT NULL,
    component       VARCHAR NOT NULL,
    mass_flow_kg_h  DOUBLE  NOT NULL,
    sulfur_wt_pct   DOUBLE,
    event_ts        TIMESTAMP
)
STORED AS PARQUET
LOCATION 's3://lake/refinery/streams/';
```

SQL type declarations are mapped into Arrow data types during `CREATE EXTERNAL TABLE` and `CAST`; string declarations map to `Utf8View` by default unless `datafusion.sql_parser.map_string_types_to_utf8view` is set to false. ([Apache DataFusion][8])

Agent rules:

```text
DDL schema = public contract.
Use explicit aliases / lowercase names.
Reject unsupported source dialect types before SQL planning.
Attach source metadata outside SQL where possible.
Snapshot DESCRIBE output after registration.
```

### S1.4.2 Inferred file schema

```sql
CREATE EXTERNAL TABLE taxi
STORED AS PARQUET
LOCATION '/mnt/nyctaxi/tripdata.parquet';

CREATE EXTERNAL TABLE raw_csv
STORED AS CSV
LOCATION '/path/to/raw.csv'
OPTIONS ('has_header' 'true');
```

Parquet external tables do not require an explicit schema because schema information is obtained from file metadata, while CSV tables infer schema by scanning a subset of the file unless a schema is supplied. DataFusion also incorporates Hive partition columns into schema/data when directory paths use Hive-compliant `key=value` partitioning. ([Apache DataFusion][9])

Agent rules:

```text
Parquet/Arrow/Avro:
  infer from embedded schema, then validate.

CSV/JSON:
  declare explicit schema for production.
  use inference for exploration only.
  route dirty input through staging schema + arrow_try_cast.

Directories:
  validate all files are schema-compatible.
  validate partition columns are expected.
  reject mixed schemas unless a deliberate merge/coercion policy exists.
```

### S1.4.3 Explicit Arrow schema

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, Schema, TimeUnit};

pub fn stream_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("stream_id", DataType::Utf8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("mass_flow_kg_h", DataType::Float64, false),
        Field::new("sulfur_wt_pct", DataType::Float64, true),
        Field::new(
            "event_ts",
            DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
            true,
        ),
    ]))
}
```

Use explicit Arrow schemas for in-memory tests, generated data, custom providers, UDF outputs, adapter boundaries, and sink contracts. Use `datafusion::arrow` re-exports to prevent Arrow version bifurcation across DataFusion and application code; the attached document highlights this as a repeated invariant. 

---

## S1.5 Catalog and provider binding

```text
CatalogProviderList
  └─ CatalogProvider
      └─ SchemaProvider
          └─ TableProvider
              ├─ schema()      -> Arc<Schema>
              ├─ constraints() -> optional constraint metadata
              ├─ statistics()  -> optional planning metadata
              └─ scan(...)     -> Arc<dyn ExecutionPlan>
```

`TableProvider` represents a source of Arrow `RecordBatch`es; its documented planning information includes `schema`, filter-pushdown support, and `scan`, with `schema()` returning the columns and types of the table. ([Docs.rs][10])

```rust
use std::sync::Arc;
use datafusion::catalog::{Session, TableProvider};
use datafusion::datasource::TableType;
use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;
use datafusion::error::Result;

#[derive(Debug)]
pub struct ContractTable {
    schema: SchemaRef,
}

#[async_trait::async_trait]
impl TableProvider for ContractTable {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // projection indices are against self.schema()
        // filters are logical Exprs already bound against this table contract
        // returned ExecutionPlan::schema() must match requested projection
        todo!()
    }
}
```

DataFusion 54 removed the `fn as_any(&self)` boilerplate from the catalog traits: `TableProvider`, `SchemaProvider`, `CatalogProvider`, and `CatalogProviderList` now declare `Any` as a supertrait (e.g. `TableProvider: Any + Debug + Sync + Send`), so implementations no longer write an `as_any` method. Downcasting uses the inherent helpers on the trait object via trait upcasting: `provider.downcast_ref::<ContractTable>()` and `provider.is::<ContractTable>()` work directly on `&dyn TableProvider` and on `Arc<dyn TableProvider>` via auto-deref.

Hard provider invariant:

```text
TableProvider::schema()
  = full unprojected table schema

scan(projection = Some(indices))
  = ExecutionPlan with schema = schema.project(indices)

every emitted RecordBatch
  = exactly ExecutionPlan::schema()
```

The planning-time `TableSource` trait exists to expose a subset of `TableProvider` information such as schema and filter-pushdown capability without depending on the execution engine; this separation is useful when building logical-plan-only systems or alternate execution engines. ([Docs.rs][11])

---

## S1.6 Logical schema: `DFSchema`

### Role

```text
Arrow Schema
  = physical ordered fields

DFSchema
  = physical fields
  + optional table/relation qualifiers
  + logical column resolution
  + ambiguity detection
  + functional-dependency space
```

```rust
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::{Column, DFSchema};

let arrow_schema = Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    Field::new("amount", DataType::Float64, true),
]);

let df_schema = DFSchema::try_from_qualified_schema("orders", &arrow_schema)?;

assert!(df_schema.has_column(&Column::from_qualified_name("orders.id")));
assert!(df_schema.has_column(&Column::from_qualified_name("id"))); // if unambiguous
```

`DFSchema` is the correct object for binding and validating expressions, not raw Arrow `Schema`, because logical expressions can reference qualified table columns. The attached document correctly frames `DFSchema` as the planning schema and Arrow `Schema` as the physical/runtime schema. 

### Qualifier-loss invariant

```rust
let arrow_only = df_schema.as_arrow();
// arrow_only contains fields + metadata
// arrow_only does not encode catalog/schema/table qualifiers as planner qualifiers
```

Hard invariant:

```text
DFSchema qualifiers are logical planner state.
Arrow Schema has no native DataFusion catalog/table qualifier semantics.
Converting DFSchema → Arrow Schema is a boundary crossing.
Do not expect qualifiers to survive RecordBatch / Parquet / Arrow IPC output.
```

---

## S1.7 Logical plan schema propagation

```text
Scan
  input: TableProvider::schema()
  output: DFSchema qualified by table/reference

Projection
  input: child DFSchema
  output: selected Expr::to_field results
  contract: aliases determine durable output names

Filter
  input: child DFSchema
  output: child DFSchema unchanged
  contract: predicate type must coerce to Boolean

Aggregate
  input: child DFSchema
  output: grouping fields + aggregate fields
  contract: aggregate aliases define stable schema

Join
  input: left DFSchema + right DFSchema
  output: combined DFSchema
  contract: duplicate names require qualification or aliasing

Union
  input: two compatible schemas
  output: coerced/merged schema
  contract: position or name semantics depend on API/operator

Sort / Limit
  input: child DFSchema
  output: child DFSchema unchanged

Window
  input: child DFSchema
  output: child fields + window expression fields
  contract: window expression aliases required for stable names

Unnest
  input: list/struct/map fields
  output: cardinality/field-shape transformed schema
  contract: output names must be explicit in stable APIs
```

`DataFrame::schema()` returns the `DFSchema` describing the output of a DataFrame, including name, data type, and nullability; `logical_plan()` returns the unoptimized logical plan, while `into_unoptimized_plan` / `into_optimized_plan` are documented as testing-oriented because they lose the attached `SessionState` snapshot. ([Docs.rs][12])

Agent rules:

```text
Validate after each schema-changing operation:
  select
  with_column
  aggregate
  join
  union
  unnest
  cast
  view creation
  CTAS
  sink write

Do not validate only after execution.
Most schema errors are planning-time errors and should be caught before collect().
```

---

## S1.8 Physical plan schema

```rust
use datafusion::prelude::*;
use datafusion::physical_plan::ExecutionPlan;

async fn inspect_physical_schema(ctx: &SessionContext) -> datafusion::error::Result<()> {
    let df = ctx.sql("SELECT 1 AS x").await?;
    let physical = df.create_physical_plan().await?;
    let schema = physical.schema();

    for field in schema.fields() {
        println!("{}: {:?}, nullable={}", field.name(), field.data_type(), field.is_nullable());
    }

    Ok(())
}
```

`ExecutionPlan::execute(partition, context)` returns an async stream of `RecordBatch`es, and `ExecutionPlan::schema()` communicates the output schema of that physical node to the optimizer/executor. Execution should be streaming: most plans should not perform heavy work before the first batch is requested. ([Docs.rs][13])

Hard invariant:

```text
physical_plan.schema()
  == schema of every RecordBatch yielded by physical_plan.execute(...)
```

Recommended physical validation:

```rust
use datafusion::physical_plan::{ExecutionPlan, InvariantLevel};

fn validate_physical_plan(plan: &dyn ExecutionPlan) -> datafusion::error::Result<()> {
    plan.check_invariants(InvariantLevel::Always)?;
    Ok(())
}
```

Use `ExecutionPlan::check_invariants` in custom physical operators and regression tests when available in the pinned version. The exact invariant levels and method availability must be verified against the version-pinned docs.rs API. ([Docs.rs][13])

---

## S1.9 Runtime `RecordBatch` schema invariants

`RecordBatch::try_new(schema, columns)` expects non-empty columns, field count equal to column count, each field data type equal to the corresponding array data type, and all columns having equal length. ([Docs.rs][5])

```rust
use std::sync::Arc;
use datafusion::arrow::{
    array::{ArrayRef, Int64Array, StringArray},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};

let schema = Arc::new(Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    Field::new("name", DataType::Utf8, true),
]));

let columns: Vec<ArrayRef> = vec![
    Arc::new(Int64Array::from(vec![1, 2, 3])),
    Arc::new(StringArray::from(vec![Some("a"), None, Some("c")])),
];

let batch = RecordBatch::try_new(schema.clone(), columns)?;
assert_eq!(batch.schema(), schema);
```

Runtime contract:

```text
RecordBatch.schema().fields()[i].data_type() == RecordBatch.column(i).data_type()
RecordBatch.schema().fields().len() == RecordBatch.num_columns()
RecordBatch.column(i).len() == RecordBatch.num_rows()
RecordBatch.schema().fields()[i].is_nullable() == false implies no nulls in column i
RecordBatch column order is semantic
```

Do not use `RecordBatch::new_unchecked` in application/provider code unless a narrow, proven, internally audited hot path justifies skipping validation.

---

## S1.10 Sink and output schema

```text
Output paths:
  collect()
    -> Vec<RecordBatch>
    -> same schema for each batch

  execute_stream()
    -> SendableRecordBatchStream
    -> stream schema + every yielded batch schema

  write_parquet()
    -> Parquet schema / physical file schema

  write_csv()
    -> header / serialized scalar field names

  write_json()
    -> serialized field names/types inferred from Arrow values

  write_table()
    -> TableProvider::insert_into target schema contract

  DataSink
    -> sink.schema()
    -> write_all(stream, task_context)
```

Sink schema policy:

```text
Sink-owned schema must be explicit before write.
Never infer output schema from first batch only.
Empty result sets still require schema.
File writers should use plan/schema contract, not sampled runtime rows.
Partition columns require explicit policy:
  kept in payload
  removed from payload
  duplicated in path + payload
```

For service APIs, the output schema is an API contract:

```json
{
  "schema_fingerprint": "sha256:...",
  "fields": [
    {"name": "stream_id", "data_type": "Utf8", "nullable": false},
    {"name": "mass_flow_kg_h", "data_type": "Float64", "nullable": false}
  ],
  "qualifiers": "stripped",
  "source_plan_hash": "..."
}
```

---

## S1.11 Hard invariants

### Invariant 1 — Provider schema must match execution output

```text
TableProvider::schema()
  → scan(projection)
  → ExecutionPlan::schema()
  → RecordBatchStream::schema()
  → every RecordBatch::schema()
```

Violation examples:

```text
provider schema says [id:Int64, name:Utf8]
scan(projection=[0]) emits [name:Utf8]
stream schema says [id:Int64] but batch emits [id:Int32]
projection index maps against backend order instead of TableProvider::schema order
```

Required test:

```rust
fn assert_batch_schema_eq(expected: &Schema, batch: &RecordBatch) {
    assert_eq!(expected.fields(), batch.schema().fields());
}
```

### Invariant 2 — Logical qualifiers do not survive Arrow conversion

```text
DFSchema:
  orders.id
  customers.id

Arrow Schema:
  id
  id
```

Required policy:

```text
Before Arrow boundary:
  preserve qualifiers for binding/ambiguity checks.

At output boundary:
  alias/rename columns into deterministic unique names.

After Arrow boundary:
  do not attempt to recover qualifiers unless stored explicitly in custom metadata.
```

### Invariant 3 — Output field names must be deterministic

Bad:

```sql
SELECT SUM(amount), amount * 1.08, ROW_NUMBER() OVER (...)
FROM t
GROUP BY amount;
```

Good:

```sql
SELECT
  SUM(amount) AS total_amount,
  amount * 1.08 AS amount_with_tax,
  ROW_NUMBER() OVER (ORDER BY amount DESC) AS amount_rank
FROM t
GROUP BY amount
ORDER BY amount_rank;
```

Agent rule:

```text
Every computed expression must have an alias:
  arithmetic
  cast
  aggregate
  window
  case
  scalar function
  nested extraction
  unnest
  generated expression
```

### Invariant 4 — `RecordBatch` array order/type/nullability must match schema

```text
Schema fields are ordered.
Column vectors are ordered.
Names do not remap runtime arrays.
```

Bad:

```rust
Schema:  [id:Int64, name:Utf8]
Columns: [StringArray(name), Int64Array(id)]
```

Good:

```rust
Schema:  [id:Int64, name:Utf8]
Columns: [Int64Array(id), StringArray(name)]
```

### Invariant 5 — Planning schema is not execution proof

```text
df.schema()
  proves planner output contract

collect().await?
  proves execution succeeded for this run

RecordBatch::try_new tests
  prove provider/adaptor emitted valid batches

physical_plan.check_invariants()
  proves some physical plan structural invariants
```

Agent rule:

```text
Validate at planning boundary and runtime boundary.
Do not treat one as replacement for the other.
```

---

## S1.12 Schema ownership model

| Ownership class       | Owns                                 | Examples                                                          | Authority               | Drift policy                       |
| --------------------- | ------------------------------------ | ----------------------------------------------------------------- | ----------------------- | ---------------------------------- |
| Source-owned schema   | physical storage contract            | Parquet footer, Avro schema, Arrow IPC schema, remote DB metadata | storage/source          | reject or adapt at ingestion       |
| Catalog-owned schema  | namespace binding and table identity | `prod.analytics.events` table schema                              | catalog/metastore       | version, cache, refresh            |
| Provider-owned schema | DataFusion scan contract             | `TableProvider::schema()`                                         | provider implementation | freeze per query planning snapshot |
| Query-derived schema  | operator output                      | projection/aggregate/join/window/union                            | planner/analyzer        | deterministic derivation           |
| Sink-owned schema     | persisted/API output                 | Parquet file, JSON API, target table insert schema                | writer/target provider  | enforce compatibility before write |

### Source-owned schema

```text
Schema source:
  Parquet footer
  Avro file schema
  Arrow IPC schema
  CSV inferred header + sampled data
  remote database INFORMATION_SCHEMA
```

Policy:

```text
Do not mutate source schema in place.
Create normalized DataFusion-facing schema.
Record source fingerprint.
Reject incompatible changes unless migration policy exists.
```

### Catalog-owned schema

```text
catalog.schema.table → TableProvider
```

Policy:

```text
Catalog controls visibility, naming, tenancy, persistence, refresh.
Catalog should not perform heavy scans during listing.
Catalog should expose stable TableProvider snapshot per query.
```

### Provider-owned schema

```text
TableProvider::schema() = contract exposed to DataFusion.
```

Policy:

```text
Cheap clone of Arc<Schema>.
No surprise network calls in schema().
No per-batch shape drift.
Projection indices are interpreted against this schema.
```

### Query-derived schema

```text
SELECT a AS x, b + c AS y FROM t
  → [x:type(a), y:type(b+c)]
```

Policy:

```text
Aliases stabilize field names.
Casts stabilize field types.
Qualifiers stabilize resolution before output.
```

### Sink-owned schema

```text
write_parquet(path)
  → final Parquet schema

write_table(target)
  → target TableProvider insert contract

execute_stream()
  → stream schema contract
```

Policy:

```text
Validate before write.
Treat empty output schema as meaningful.
Snapshot output schema in CI.
```

---

## S1.13 Validation checkpoints

```text
Checkpoint 0 — schema construction
  Rust: Schema::new, Field::new
  SQL: CREATE EXTERNAL TABLE explicit columns
  File: DESCRIBE / schema inference audit

Checkpoint 1 — registration
  catalog.table_exist()
  table_provider.schema()
  information_schema.columns

Checkpoint 2 — expression binding
  ExprSchemable::get_type(df_schema)
  ExprSchemable::nullable(df_schema)
  missing/ambiguous column rejection

Checkpoint 3 — logical plan
  df.schema()
  df.logical_plan().schema()
  optimized plan schema snapshot

Checkpoint 4 — physical plan
  df.create_physical_plan().await?.schema()
  ExecutionPlan::check_invariants(...)

Checkpoint 5 — runtime batch
  RecordBatch::try_new(...)
  stream schema vs batch schema
  all batches same schema

Checkpoint 6 — sink
  writer schema
  target TableProvider insert compatibility
  Parquet/Arrow metadata inspection
```

### Minimal Rust validation harness

```rust
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::DFSchema;
use datafusion::prelude::*;

pub fn assert_arrow_schema_eq(expected: &SchemaRef, actual: &SchemaRef) {
    assert_eq!(expected.fields(), actual.fields(), "field mismatch");
    assert_eq!(expected.metadata(), actual.metadata(), "schema metadata mismatch");
}

pub fn assert_batch_schema_eq(expected: &SchemaRef, batch: &RecordBatch) {
    assert_arrow_schema_eq(expected, &batch.schema());
}

pub fn assert_df_schema_has_unique_names(df_schema: &DFSchema) {
    let mut seen = std::collections::HashSet::new();

    for (_qualifier, field) in df_schema.iter() {
        let inserted = seen.insert(field.name().to_string());
        assert!(inserted, "duplicate output field name: {}", field.name());
    }
}

pub async fn validate_dataframe_schema(df: DataFrame) -> datafusion::error::Result<Vec<RecordBatch>> {
    let logical_schema = df.schema().clone();

    assert_df_schema_has_unique_names(&logical_schema);

    let batches = df.collect().await?;

    if let Some(first) = batches.first() {
        let output_schema = first.schema();
        for batch in &batches {
            assert_batch_schema_eq(&output_schema, batch);
        }
    }

    Ok(batches)
}
```

---

## S1.14 Normalization checkpoints

### Normalize at source boundary

```text
external field: "Stream ID"
normalized field: stream_id

external type: VARCHAR
normalized Arrow type: Utf8 or Utf8View, according to config

external nullability: unknown
normalized nullability: true unless proven/enforced
```

SQL pattern:

```sql
WITH normalized AS (
  SELECT
    "Stream ID" AS stream_id,
    "Unit ID" AS unit_id,
    CAST("Mass Flow" AS DOUBLE) AS mass_flow_kg_h
  FROM raw_streams
)
SELECT *
FROM normalized;
```

Rust schema normalization pattern:

```rust
use datafusion::arrow::datatypes::{DataType, Field};

#[derive(Debug, Clone)]
pub struct FieldPolicy {
    pub source_name: String,
    pub canonical_name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

pub fn canonical_field(p: &FieldPolicy) -> Field {
    Field::new(&p.canonical_name, p.data_type.clone(), p.nullable)
}
```

### Normalize before catalog registration

```text
Source schema:
  raw names, source-specific types, source nullability

Catalog schema:
  canonical names, DataFusion-supported Arrow types, explicit nullability
```

### Normalize before output/sink

```text
Planning qualifiers:
  orders.id
  customers.id

Output fields:
  order_id
  customer_id
```

Agent rule:

```text
Normalize early for source fields.
Alias late for output fields.
Do not strip qualifiers before name resolution is complete.
```

---

## S1.15 Metadata attachment points

| Metadata location          | Attach syntax                  | Best use                        | Risk                                           |
| -------------------------- | ------------------------------ | ------------------------------- | ---------------------------------------------- |
| Arrow `Field` metadata     | `Field::with_metadata`         | semantic type, units, source id | may be dropped by operators/sinks              |
| Arrow `Schema` metadata    | `Schema::new_with_metadata`    | dataset-level lineage/version   | not physical memory layout                     |
| `DFSchema` metadata        | `DFSchema::new_with_metadata`  | logical schema annotations      | qualifier/metadata semantics are planner-local |
| Parquet key-value metadata | writer options / file metadata | durable file lineage            | reader compatibility                           |
| Catalog metadata           | custom provider                | ownership, ACLs, table version  | not exposed uniformly                          |
| Output manifest            | app-owned JSON/YAML            | stable audit contract           | extra artifact lifecycle                       |

Attached guidance already notes that metadata should not be assumed to affect optimization unless DataFusion specifically consumes it. 

Recommended metadata keys:

```text
schema.contract.version
schema.contract.fingerprint
source.system
source.schema.fingerprint
source.extracted_at
semantic.unit
semantic.type
governance.classification
lineage.plan_hash
lineage.created_by
```

Rust pattern:

```rust
use std::collections::HashMap;
use datafusion::arrow::datatypes::{DataType, Field, Schema};

let mut field_meta = HashMap::new();
field_meta.insert("semantic.unit".to_string(), "kg/h".to_string());
field_meta.insert("semantic.type".to_string(), "mass_flow".to_string());

let flow = Field::new("mass_flow_kg_h", DataType::Float64, false)
    .with_metadata(field_meta);

let mut schema_meta = HashMap::new();
schema_meta.insert("schema.contract.version".to_string(), "streams.v1".to_string());

let schema = Schema::new_with_metadata(vec![flow], schema_meta);
```

Policy:

```text
Use metadata for advisory semantics.
Use explicit schema fields/types/nullability for enforced semantics.
Test metadata preservation across:
  projection
  alias
  cast
  join
  aggregate
  write_parquet
  read_parquet
```

---

## S1.16 Schema drift rejection

### Drift classes

```text
Name drift:
  missing column
  extra column
  renamed column
  duplicate column
  case-only rename

Type drift:
  exact mismatch
  coercible widening
  lossy narrowing
  decimal scale/precision change
  timestamp unit/timezone change
  Utf8 vs Utf8View vs LargeUtf8

Nullability drift:
  nullable → non-nullable
  non-nullable → nullable
  runtime nulls in non-nullable field

Nested drift:
  struct field added/dropped/renamed
  list element type changed
  map key/value type changed
  struct field order changed

Catalog drift:
  table disappeared
  table provider changed
  schema cache stale
  remote metastore version changed

Sink drift:
  target schema incompatible
  output schema changed unexpectedly
  partition column changed
```

### Drift policy table

| Drift                   | Default agent action               | Optional override                   |
| ----------------------- | ---------------------------------- | ----------------------------------- |
| missing required column | reject                             | derive/backfill if rule exists      |
| extra source column     | ignore or warn                     | include if schema evolution allowed |
| type widening           | allow only if policy says lossless | emit cast                           |
| type narrowing          | reject                             | require explicit lossy cast         |
| nullable → non-nullable | reject unless validated            | filter/reject null rows             |
| non-nullable → nullable | allow with warning                 | tighten only after runtime proof    |
| nested field add        | allow if nullable                  | version contract                    |
| nested field drop       | reject                             | compatibility view                  |
| partition column change | reject                             | new table version                   |
| metadata-only change    | warn                               | accept if non-contract metadata     |

### Drift report schema

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDriftReport {
    pub contract_name: String,
    pub expected_fingerprint: String,
    pub actual_fingerprint: String,
    pub findings: Vec<SchemaDriftFinding>,
    pub decision: DriftDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDriftFinding {
    pub path: String,
    pub kind: DriftKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub severity: Severity,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DriftKind {
    MissingField,
    ExtraField,
    TypeMismatch,
    NullabilityMismatch,
    MetadataMismatch,
    OrderMismatch,
    NestedMismatch,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DriftDecision {
    Accept,
    AcceptWithWarning,
    Reject,
    RequiresMigration,
}
```

---

## S1.17 Schema fingerprinting

Purpose:

```text
Detect schema drift deterministically.
Cache plans against schema versions.
Audit output compatibility.
Invalidate provider/catalog caches.
Regression-test plan output contracts.
```

Canonical fingerprint ingredients:

```text
field index
field path
field name
data type canonical string
nullability
field metadata subset
schema metadata subset
qualifier policy
partition-column flag
generated-column flag
contract version
```

Rust sketch:

```rust
use datafusion::arrow::datatypes::{DataType, Field, Schema};

pub fn canonical_schema_string(schema: &Schema) -> String {
    let mut out = String::new();

    for (idx, field) in schema.fields().iter().enumerate() {
        out.push_str(&format!(
            "{}|{}|{:?}|nullable={}\n",
            idx,
            field.name(),
            field.data_type(),
            field.is_nullable()
        ));

        // Optional: include sorted metadata keys from an allowlist.
        let mut keys: Vec<_> = field.metadata().keys().collect();
        keys.sort();
        for key in keys {
            if key.starts_with("schema.contract.") || key.starts_with("semantic.") {
                out.push_str(&format!(
                    "  meta:{}={}\n",
                    key,
                    field.metadata().get(key).unwrap()
                ));
            }
        }
    }

    out
}
```

Use a cryptographic hash such as SHA-256 over the canonical string in deployment code. Do not hash raw `Debug` output if it is not stable across versions.

---

## S1.18 Schema contract object for agents

```rust
use std::collections::HashMap;
use datafusion::arrow::datatypes::SchemaRef;

#[derive(Debug, Clone)]
pub struct SchemaContract {
    pub name: String,
    pub version: String,
    pub owner: SchemaOwner,
    pub schema: SchemaRef,
    pub fingerprint: String,
    pub qualifier_policy: QualifierPolicy,
    pub compatibility_policy: CompatibilityPolicy,
    pub metadata_policy: MetadataPolicy,
}

#[derive(Debug, Clone)]
pub enum SchemaOwner {
    SourceOwned,
    CatalogOwned,
    ProviderOwned,
    QueryDerived,
    SinkOwned,
}

#[derive(Debug, Clone)]
pub enum QualifierPolicy {
    PreserveDuringPlanning,
    StripAtOutput,
    PersistInMetadata,
}

#[derive(Debug, Clone)]
pub enum CompatibilityPolicy {
    ExactPhysical,
    LogicalDataFusion,
    SemanticAllowMetadataDrift,
    CoercibleWithExplicitCasts,
}

#[derive(Debug, Clone)]
pub struct MetadataPolicy {
    pub preserve_schema_keys: Vec<String>,
    pub preserve_field_keys: Vec<String>,
    pub reject_unknown_contract_keys: bool,
}
```

Agent lifecycle:

```text
create SchemaContract at source/provider boundary
validate SchemaContract at catalog registration
convert SchemaContract.schema to DFSchema during planning
derive QueryDerived SchemaContract after logical plan
derive SinkOwned SchemaContract before write
snapshot all contract fingerprints in CI
```

---

## S1.19 Deployment architecture pattern

```text
crates/query-core/src/schema/
  mod.rs
  contract.rs          # SchemaContract, SchemaOwner, policies
  normalize.rs         # source → canonical schema
  fingerprint.rs       # stable schema hashing
  diff.rs              # expected vs actual
  validate.rs          # RecordBatch / DFSchema / sink validation
  metadata.rs          # metadata allowlist + preservation checks
  registry.rs          # known contracts by table/view/sink
  diagnostics.rs       # schema error payloads for agents
```

Recommended module interfaces:

```rust
pub trait SchemaNormalizer {
    fn normalize_source_schema(&self, source: SchemaRef) -> datafusion::error::Result<SchemaRef>;
}

pub trait SchemaValidator {
    fn validate_arrow_schema(&self, actual: &SchemaRef) -> datafusion::error::Result<()>;
    fn validate_batch(&self, batch: &RecordBatch) -> datafusion::error::Result<()>;
    fn diff(&self, actual: &SchemaRef) -> SchemaDriftReport;
}

pub trait SchemaCatalog {
    fn contract_for_table(&self, table_ref: &str) -> Option<SchemaContract>;
    fn register_contract(&mut self, contract: SchemaContract);
}
```

---

## S1.20 Provider implementation pattern with schema validation

```rust
use std::sync::Arc;
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::error::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct ProviderSchemaGuard {
    full_schema: SchemaRef,
}

impl ProviderSchemaGuard {
    pub fn new(full_schema: SchemaRef) -> Self {
        Self { full_schema }
    }

    pub fn full_schema(&self) -> SchemaRef {
        self.full_schema.clone()
    }

    pub fn project_schema(&self, projection: Option<&Vec<usize>>) -> Result<SchemaRef> {
        match projection {
            Some(indices) => {
                let projected = self
                    .full_schema
                    .project(indices)
                    .map_err(|e| DataFusionError::External(Box::new(e)))?;
                Ok(Arc::new(projected))
            }
            None => Ok(self.full_schema.clone()),
        }
    }

    pub fn validate_batch(&self, expected: &SchemaRef, batch: &RecordBatch) -> Result<()> {
        if expected.fields() != batch.schema().fields() {
            return Err(DataFusionError::Execution(format!(
                "provider emitted batch with schema mismatch: expected={:?}, actual={:?}",
                expected.fields(),
                batch.schema().fields()
            )));
        }

        Ok(())
    }
}
```

Use this guard inside custom `ExecutionPlan` stream construction:

```rust
let output_schema = schema_guard.project_schema(projection)?;
let stream = make_stream(output_schema.clone(), backend_request);

let stream = stream.map(move |batch_result| {
    let batch = batch_result?;
    schema_guard.validate_batch(&output_schema, &batch)?;
    Ok(batch)
});
```

In production, gate runtime batch validation with a config flag:

```text
dev/test/staging:
  validate every batch

production hot path:
  validate first batch per partition
  validate all batches behind debug/strict mode
  always validate after schema-changing provider upgrades
```

---

## S1.21 Query-derived schema stabilization

### SQL generation rule

```sql
SELECT
  o.id AS order_id,
  c.id AS customer_id,
  CAST(o.amount AS DOUBLE) AS amount_f64,
  o.amount * 1.08 AS amount_with_tax,
  ROW_NUMBER() OVER (
    PARTITION BY c.id
    ORDER BY o.event_ts DESC, o.id ASC
  ) AS customer_order_rank
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.id;
```

### DataFrame generation rule

```rust
use datafusion::prelude::*;

let df = orders
    .join(
        customers,
        datafusion::logical_expr::JoinType::Inner,
        &["customer_id"],
        &["id"],
        None,
    )?
    .select(vec![
        col("orders.id").alias("order_id"),
        col("customers.id").alias("customer_id"),
        col("orders.amount").cast_to(
            &datafusion::arrow::datatypes::DataType::Float64,
            orders.schema(),
        )?.alias("amount_f64"),
    ])?;
```

Schema stabilization checklist:

```text
[ ] all computed fields aliased
[ ] all duplicate join fields renamed
[ ] all casts explicit where output type matters
[ ] all source case/special identifiers normalized
[ ] no SELECT * in stable view/API/sink contracts
[ ] output schema snapshot stored
```

---

## S1.22 Introspection patterns

### SQL introspection

```sql
DESCRIBE prod.analytics.streams;

SELECT
  table_catalog,
  table_schema,
  table_name,
  column_name,
  ordinal_position,
  data_type,
  is_nullable
FROM information_schema.columns
WHERE table_schema = 'analytics'
  AND table_name = 'streams'
ORDER BY ordinal_position;

SELECT
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM prod.analytics.streams
LIMIT 1;
```

`DESCRIBE` works across regular tables, external tables, views, and qualified table names. ([Apache DataFusion][9])

### Rust introspection

```rust
use datafusion::prelude::*;

pub async fn schema_audit(ctx: &SessionContext, table: &str) -> datafusion::error::Result<()> {
    let df = ctx.table(table).await?;
    let df_schema = df.schema();

    println!("logical schema: {df_schema:#?}");

    let physical = df.create_physical_plan().await?;
    println!("physical schema: {:#?}", physical.schema());

    Ok(())
}
```

---

## S1.23 Testing matrix

```text
Unit tests:
  Schema::new / Field::new expected schema
  canonical schema fingerprint
  diff report examples
  normalization rules

Provider tests:
  TableProvider::schema()
  scan(None) schema
  scan(Some(projection)) schema
  every emitted batch schema
  empty stream schema
  filter/projection pushdown schema preservation

Planner tests:
  df.schema()
  df.logical_plan()
  into_optimized_plan() schema
  expression get_type / nullable
  ambiguous column rejection
  missing column rejection

SQL tests:
  CREATE EXTERNAL TABLE explicit schema
  inferred schema audit
  DESCRIBE output
  arrow_typeof outputs
  quoted identifier behavior
  aliases for computed expressions

Runtime tests:
  collect() batch schemas all equal
  execute_stream() stream batches all equal
  partitioned stream schemas all equal
  sink output schema verified

Regression tests:
  golden Arrow schema JSON
  golden DFSchema debug-normalized artifact
  golden SQL DESCRIBE output
  golden Parquet schema after write/read roundtrip
```

Example test:

```rust
#[tokio::test]
async fn output_schema_is_stable() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();

    ctx.register_csv("streams", "tests/fixtures/streams.csv", CsvReadOptions::new())
        .await?;

    let df = ctx
        .sql(
            r#"
            SELECT
              stream_id AS stream_id,
              CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h
            FROM streams
            ORDER BY stream_id
            "#
        )
        .await?;

    let schema = df.schema();

    let names: Vec<_> = schema.fields().iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, vec!["stream_id", "mass_flow_kg_h"]);

    let batches = df.collect().await?;
    assert!(!batches.is_empty());

    let output_schema = batches[0].schema();
    for batch in &batches {
        assert_eq!(batch.schema().fields(), output_schema.fields());
    }

    Ok(())
}
```

---

## S1.24 Diagnostics payloads for agents

Schema error payload:

```json
{
  "error_class": "schema_mismatch",
  "phase": "provider_runtime",
  "owner": "provider_owned",
  "table": "prod.analytics.streams",
  "node_path": "DataSourceExec[streams]",
  "expected_schema_fingerprint": "sha256:...",
  "actual_schema_fingerprint": "sha256:...",
  "field_path": "mass_flow_kg_h",
  "expected_type": "Float64",
  "actual_type": "Utf8",
  "expected_nullable": false,
  "actual_nullable": true,
  "candidate_resolutions": [
    "cast Utf8 to Float64 during source normalization",
    "mark field nullable in contract and add rejected-row audit",
    "reject source file as schema drift"
  ],
  "suggested_fix": "Add explicit CAST in staging query or update provider type adapter"
}
```

Recommended error classes:

```text
schema_inference_failure
schema_missing_field
schema_extra_field
schema_duplicate_field
schema_type_mismatch
schema_nullability_mismatch
schema_order_mismatch
schema_qualifier_loss
schema_ambiguous_column
schema_provider_output_mismatch
schema_sink_incompatible
schema_metadata_mismatch
schema_partition_conflict
```

---

## S1.25 Deployment advisory

### Service / API deployment

```text
Use explicit table registration.
Disable direct URL table access for untrusted SQL.
Use read-only SQLOptions unless DDL/DML is a product feature.
Expose schema endpoint backed by df.schema() / information_schema.
Return schema fingerprint with every streamed result.
Validate output schema before first response chunk.
```

### Batch / ETL deployment

```text
Pin DataFusion version.
Pin Arrow version via DataFusion re-exports.
Explicitly declare CSV/JSON schemas.
Trust Parquet/Arrow/Avro embedded schemas only after audit.
Write output schema manifest next to output data.
Run read-after-write schema roundtrip tests.
Reject drift before expensive execution.
```

### Custom provider deployment

```text
schema() must be cheap, stable, and Arc-cloneable.
scan() must interpret projection indices against schema().
execute() must construct streams, not perform heavy work eagerly.
stream polling performs IO/compute and emits valid RecordBatch values.
provider tests must cover projection, empty output, nullability, and backend drift.
```

### Catalog / metastore deployment

```text
Catalog owns namespace and visibility.
SchemaProvider table(name) may be async; cache remote metadata deliberately.
Avoid expensive table_names/schema_names hot-path calls.
Use versioned table contracts.
Separate tenant catalogs or schemas when visibility differs.
Filter information_schema for sensitive deployments.
```

---

## S1.26 Anti-pattern inventory

```text
Schema lifecycle anti-patterns:
  treating schema as a local Arrow object only
  validating schema only after collect()
  inferring stable API schema from first batch
  using SELECT * in views/sinks/APIs
  relying on expression-derived output names
  stripping DFSchema qualifiers before joins are resolved
  assuming qualifiers survive Arrow/Parquet/RecordBatch boundaries
  using direct arrow::* crate versions that differ from DataFusion’s Arrow version
  building RecordBatch columns in name order instead of schema index order
  marking fields non-nullable before runtime data quality is proven
  trusting CSV inference in production
  hiding schema drift behind permissive casts
  accepting nullable → non-nullable changes without validation
  using metadata as if DataFusion enforces it
  implementing TableProvider::schema() with live remote IO on every call
  emitting projected batches in backend order rather than requested projection order
  returning ExecutionPlan::schema() that differs from actual stream batches
  writing output files without schema manifest / read-after-write audit
  exposing information_schema without tenant/governance filtering
```

---

## S1.27 Agent checklist

```text
[ ] Determine schema origin:
    declared SQL / inferred file / Arrow explicit / provider dynamic / query-derived / sink-owned.

[ ] Normalize at source boundary:
    names, types, nullability, metadata, partition columns.

[ ] Register only validated schemas:
    catalog/schema/table provider binding must expose stable Arc<Schema>.

[ ] Treat TableProvider::schema() as authoritative:
    projection indices and filters are bound against it.

[ ] Use DFSchema for planning:
    column resolution, qualifiers, type/nullability inference, ambiguity detection.

[ ] Preserve qualifiers until output boundary:
    alias duplicate fields before Arrow/sink boundary.

[ ] Alias all computed outputs:
    arithmetic, casts, aggregates, windows, CASE, functions, nested extraction.

[ ] Validate logical schema:
    df.schema(), expression get_type/nullable, logical plan schema.

[ ] Validate physical schema:
    create_physical_plan().schema(), check_invariants where available.

[ ] Validate runtime batches:
    RecordBatch::try_new in adapters; batch schema equality in tests.

[ ] Validate sink schema:
    target table compatibility, writer schema, read-after-write schema.

[ ] Generate schema fingerprint:
    store with plan/output artifacts.

[ ] Reject schema drift explicitly:
    missing required fields, lossy casts, nullability tightening, partition conflicts.

[ ] Attach metadata deliberately:
    advisory unless explicitly enforced by application/provider.

[ ] Snapshot schema contracts in CI:
    DESCRIBE, information_schema, arrow_typeof, df.schema(), Parquet roundtrip.

[ ] Pin DataFusion + Arrow versions:
    use datafusion::arrow re-exports in application/provider code.
```

## S1.28 Minimal “schema-first” reference flow

```rust
use std::sync::Arc;

use datafusion::arrow::{
    array::{ArrayRef, Float64Array, StringArray},
    datatypes::{DataType, Field, Schema, SchemaRef},
    record_batch::RecordBatch,
};
use datafusion::prelude::*;

fn declared_contract_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("stream_id", DataType::Utf8, false),
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("mass_flow_kg_h", DataType::Float64, false),
    ]))
}

fn make_valid_batch(schema: SchemaRef) -> datafusion::error::Result<RecordBatch> {
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec!["s1", "s2"])),
        Arc::new(StringArray::from(vec!["CDU", "VDU"])),
        Arc::new(Float64Array::from(vec![1000.0, 500.0])),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}

#[tokio::main]
async fn main() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();

    let schema = declared_contract_schema();
    let batch = make_valid_batch(schema.clone())?;

    let df = ctx
        .read_batch(batch)?
        .select(vec![
            col("stream_id").alias("stream_id"),
            col("unit_id").alias("unit_id"),
            col("mass_flow_kg_h").alias("mass_flow_kg_h"),
        ])?;

    // Planning-time schema validation.
    let df_schema = df.schema();
    assert_eq!(df_schema.fields().len(), schema.fields().len());

    // Physical plan schema validation.
    let physical = df.clone().create_physical_plan().await?;
    assert_eq!(physical.schema().fields().len(), schema.fields().len());

    // Runtime batch validation.
    let batches = df.collect().await?;
    for batch in &batches {
        assert_eq!(batch.schema().fields(), schema.fields());
    }

    Ok(())
}
```

This is the canonical mental model for agents: **declare/normalize schema first, register it through the provider/catalog layer, validate it during logical planning, validate it again at physical/runtime boundaries, and treat sink output schema as a versioned contract.**

[1]: https://docs.rs/datafusion/latest/datafusion/ "datafusion - Rust"
[2]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[3]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Field.html "Field in arrow::datatypes - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html "DFSchema in datafusion::common - Rust"
[5]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html "RecordBatch in arrow::record_batch - Rust"
[6]: https://datafusion.apache.org/library-user-guide/catalogs.html "Catalogs, Schemas, and Tables — Apache DataFusion  documentation"
[7]: https://datafusion.apache.org/_sources/library-user-guide/custom-table-providers.md.txt "datafusion.apache.org"
[8]: https://datafusion.apache.org/user-guide/sql/data_types.html "Data Types — Apache DataFusion  documentation"
[9]: https://datafusion.apache.org/user-guide/sql/ddl.html "DDL — Apache DataFusion  documentation"
[10]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.TableProvider.html "TableProvider in datafusion::catalog - Rust"
[11]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.TableSource.html "TableSource in datafusion_expr - Rust"
[12]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html "DataFrame in datafusion::dataframe - Rust"
[13]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html "ExecutionPlan in datafusion::physical_plan - Rust"


# DataFusion Advanced — S2) Schema creation surfaces and factory patterns

## S2.0 Objective

Make **schema creation** explicit, enumerable, policy-governed, and factory-backed. DataFusion schema creation is not one API. It is a union of:

```text
Rust-native Arrow schema construction
SQL DDL schema declaration
file/source schema discovery
catalog/provider schema exposure
logical-plan-derived schema construction
sink/output schema materialization
```

The attached documentation already covers the raw pieces—Arrow schemas/batches, DataFusion `DFSchema`, DDL, file formats, catalogs, custom providers, and DataFrame/SQL plan construction—but not as one unified schema factory taxonomy. 

Core invariant:

```text
Every schema must have:
  origin
  owner
  normalization policy
  compatibility policy
  metadata policy
  validation checkpoint
  drift behavior
  factory function
```

---

## S2.1 Schema creation surface taxonomy

```text
SchemaCreationSurface
  ├─ RustNative
  │   ├─ Schema::new
  │   ├─ Schema::new_with_metadata
  │   ├─ Field::new
  │   ├─ Field::new_list / new_struct / new_map / new_list_field
  │   └─ RecordBatch::try_new
  │
  ├─ SqlDdl
  │   ├─ CREATE SCHEMA
  │   ├─ CREATE EXTERNAL TABLE (...)
  │   ├─ CREATE TABLE AS SELECT
  │   └─ CREATE VIEW
  │
  ├─ SourceDriven
  │   ├─ Parquet footer schema
  │   ├─ Arrow IPC embedded schema
  │   ├─ Avro embedded schema
  │   ├─ CSV inference
  │   ├─ JSON inference
  │   └─ Hive partition-derived columns
  │
  ├─ ProviderDriven
  │   ├─ TableProvider::schema()
  │   ├─ ListingTable provided schema
  │   ├─ MemTable batch schema
  │   ├─ ViewTable logical-plan schema
  │   └─ dynamic remote schema snapshot
  │
  ├─ Derived
  │   ├─ projection schema
  │   ├─ aggregate schema
  │   ├─ join schema
  │   ├─ union/coercion schema
  │   ├─ window schema
  │   ├─ nested get_field schema
  │   └─ unnest schema
  │
  └─ SinkOutput
      ├─ write_parquet output schema
      ├─ write_csv header/schema
      ├─ write_json serialized object schema
      ├─ write_table target schema
      └─ service/API response schema
```

Arrow `Schema` is an ordered sequence of `Field`s plus schema metadata, and the Arrow docs state that this metadata is not part of the physical memory layout. `Schema::new`, `Schema::new_with_metadata`, `Schema::project`, `Schema::try_merge`, and `Schema::normalize` are the primary Rust-native schema operations relevant to DataFusion schema factories. ([Docs.rs][1])

---

## S2.2 Decision matrix: which schema creation path to use

| Scenario                 | Preferred factory                  | Primary API                                            | Validation                                     | Deployment note                                          |
| ------------------------ | ---------------------------------- | ------------------------------------------------------ | ---------------------------------------------- | -------------------------------------------------------- |
| In-memory test fixture   | `from_arrow`                       | `Schema::new` + `RecordBatch::try_new`                 | batch constructor                              | strongest small-test path                                |
| App-owned data model     | `from_contract`                    | generated `Arc<Schema>`                                | contract fingerprint                           | source of truth should be code/data contract             |
| CSV production ingestion | `from_sql_decl` or `from_contract` | explicit DDL schema                                    | `DESCRIBE`, `arrow_typeof`, rejected-row tests | avoid inference for durable contract                     |
| CSV exploration          | `from_source_inferred`             | `read_csv` / `CREATE EXTERNAL TABLE` inferred          | schema snapshot                                | not stable enough for API/sink                           |
| Parquet lake table       | `from_source_footer`               | Parquet metadata / `read_parquet` / `register_parquet` | footer schema + directory compatibility        | usually trust footer, then validate                      |
| Arrow IPC                | `from_source_embedded`             | `read_arrow` / `register_arrow`                        | embedded schema                                | good Arrow-native interchange                            |
| Avro ingestion           | `from_source_embedded`             | `read_avro` / `register_avro`                          | Avro logical-type mapping tests                | feature-gated; validate nested/union semantics           |
| Remote database/API      | `from_provider_snapshot`           | custom `TableProvider::schema()`                       | schema snapshot + drift report                 | freeze schema per query/session                          |
| SQL view                 | `from_plan`                        | `CREATE VIEW AS SELECT`                                | view output schema                             | aliases/casts mandatory                                  |
| CTAS                     | `from_plan_for_table`              | `CREATE TABLE AS SELECT`                               | output schema snapshot                         | default table may be memory-backed unless custom catalog |
| API result stream        | `for_sink`                         | `df.schema()` + `execute_stream`                       | pre-first-chunk schema                         | include schema/fingerprint in response                   |
| Parquet export           | `for_sink`                         | `write_parquet`                                        | read-after-write schema                        | attach manifest                                          |

DataFusion’s `SessionContext` is the main interface for creating DataFrames, registering data sources, registering custom tables, and executing SQL; its API also exposes direct file reads, table registration, SQL, and table-provider retrieval, which makes it the orchestration boundary for multiple schema creation surfaces. ([Docs.rs][2])

---

## S2.3 Rust-native creation

### S2.3.1 Imports and dependency rule

```rust
use std::sync::Arc;
use std::collections::HashMap;

use datafusion::arrow::{
    array::{ArrayRef, Float64Array, Int64Array, StringArray},
    datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit},
    record_batch::RecordBatch,
};
```

Agent rule:

```text
Use datafusion::arrow re-exports in DataFusion applications.
Avoid direct arrow::* imports unless the Arrow version is deliberately aligned.
```

The attached document repeatedly flags Arrow version matching and `datafusion::arrow` re-exports as a core deployment invariant. 

---

### S2.3.2 `Field::new`: scalar field factory

```rust
let stream_id = Field::new("stream_id", DataType::Utf8, false);
let mass_flow = Field::new("mass_flow_kg_h", DataType::Float64, false);
let sulfur = Field::new("sulfur_wt_pct", DataType::Float64, true);
let event_ts = Field::new(
    "event_ts",
    DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
    true,
);
```

`Field` describes a single column in a `Schema` and stores name, data type, nullability, and metadata; Arrow extension types are encoded through `Field` metadata. `Field::new(name, data_type, nullable)` creates the basic field object. ([Docs.rs][3])

Policy:

```text
nullable=false only when enforced upstream or validated during ingestion
nullable=true for inferred / dirty / optional data
field name = canonical output/API/schema-contract name
metadata = advisory unless your platform enforces it
```

---

### S2.3.3 Field metadata factory

```rust
fn field_with_semantics(
    name: &str,
    data_type: DataType,
    nullable: bool,
    semantic_type: &str,
    unit: Option<&str>,
) -> Field {
    let mut metadata = HashMap::new();
    metadata.insert("semantic.type".to_string(), semantic_type.to_string());

    if let Some(unit) = unit {
        metadata.insert("semantic.unit".to_string(), unit.to_string());
    }

    Field::new(name, data_type, nullable).with_metadata(metadata)
}

let mass_flow = field_with_semantics(
    "mass_flow_kg_h",
    DataType::Float64,
    false,
    "mass_flow",
    Some("kg/h"),
);
```

Metadata policy:

```text
Allowed field metadata:
  semantic.type
  semantic.unit
  source.name
  source.path
  governance.classification
  schema.contract.field_id

Rejected / avoid:
  executable logic
  SQL snippets
  secret material
  optimizer assumptions not consumed by DataFusion
```

---

### S2.3.4 Nested `Field` constructors

#### List field

```rust
let tags = Field::new_list(
    "tags",
    Arc::new(Field::new_list_field(DataType::Utf8, true)),
    true,
);
```

#### Struct field

```rust
let assay = Field::new_struct(
    "assay",
    vec![
        Arc::new(Field::new("api_gravity", DataType::Float64, true)),
        Arc::new(Field::new("sulfur_wt_pct", DataType::Float64, true)),
        Arc::new(Field::new("tanf_mg_koh_g", DataType::Float64, true)),
    ],
    true,
);
```

#### Explicit `DataType::Struct`

```rust
let point_type = DataType::Struct(vec![
    Field::new("x", DataType::Float64, false).into(),
    Field::new("y", DataType::Float64, false).into(),
].into());

let point = Field::new("point", point_type, false);
```

Nested nullability policy:

```text
list column nullable != list item nullable
struct column nullable != struct field nullable
map column nullable != key/value nullable
```

Agent rule:

```text
Always state parent nullability and child nullability separately in schema contracts.
```

---

### S2.3.5 `Schema::new`

```rust
pub fn stream_schema_v1() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("stream_id", DataType::Utf8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("mass_flow_kg_h", DataType::Float64, false),
        Field::new("sulfur_wt_pct", DataType::Float64, true),
    ]))
}
```

`Schema::new` creates a schema from a sequence of `Field` values, and schema field order is semantically important because Arrow arrays in a `RecordBatch` are position-aligned with fields. ([Docs.rs][1])

Factory rule:

```text
Schema::new = canonical ordered physical schema.
Do not use maps/sets as schema source without deterministic ordering.
```

---

### S2.3.6 `Schema::new_with_metadata`

```rust
pub fn stream_schema_v1_with_metadata() -> SchemaRef {
    let mut metadata = HashMap::new();
    metadata.insert("schema.contract.name".to_string(), "refinery.streams".to_string());
    metadata.insert("schema.contract.version".to_string(), "1".to_string());
    metadata.insert("schema.owner".to_string(), "simulation-engine".to_string());

    Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("stream_id", DataType::Utf8, false),
            Field::new("case_id", DataType::Utf8, false),
            Field::new("mass_flow_kg_h", DataType::Float64, false),
        ],
        metadata,
    ))
}
```

`Schema::new_with_metadata` creates a schema from fields and attaches schema-level key-value metadata. `Schema::project` returns a projected schema while carrying parent metadata, and `Schema::try_merge` merges compatible schemas, including recursively merging struct fields. ([Docs.rs][1])

Metadata governance:

```text
schema-level metadata:
  dataset/schema contract identity
  schema version
  lineage
  owner
  compatibility policy

field-level metadata:
  semantic unit
  semantic type
  source field name
  classification
```

---

### S2.3.7 `RecordBatch::try_new`: data + schema factory

```rust
pub fn example_stream_batch() -> datafusion::error::Result<RecordBatch> {
    let schema = stream_schema_v1();

    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec!["S1", "S2"])),
        Arc::new(StringArray::from(vec!["CASE_A", "CASE_A"])),
        Arc::new(StringArray::from(vec!["CDU", "VDU"])),
        Arc::new(Float64Array::from(vec![1000.0, 500.0])),
        Arc::new(Float64Array::from(vec![Some(0.5), None])),
    ];

    Ok(RecordBatch::try_new(schema, columns)?)
}
```

`RecordBatch::try_new` validates that columns are non-empty, field count equals column count, every array data type matches the corresponding field data type, and all arrays have equal length; `new_unchecked` bypasses validation and can cause undefined behavior if schema/data do not match. ([Docs.rs][4])

Factory rule:

```text
RecordBatch::try_new is a validation boundary, not just a constructor.
Use it in all adapters, tests, custom providers, and source simulators.
```

---

### S2.3.8 Rust-native creation value case

| Feature                     | Value                                                           |
| --------------------------- | --------------------------------------------------------------- |
| `Field::new`                | precise column contract: name, type, nullability                |
| nested field constructors   | first-class Arrow nested model; avoids JSON-string anti-pattern |
| `Schema::new`               | deterministic physical field order                              |
| `Schema::new_with_metadata` | contract identity + lineage + semantic annotations              |
| `Schema::project`           | projection schema derivation for custom providers               |
| `Schema::try_merge`         | controlled compatibility/evolution utility                      |
| `RecordBatch::try_new`      | runtime schema/data validation                                  |

---

## S2.4 SQL DDL creation

### S2.4.1 `CREATE SCHEMA`

```sql
CREATE SCHEMA IF NOT EXISTS analytics;
CREATE SCHEMA IF NOT EXISTS prod.analytics;
```

Use for catalog namespace creation, not data storage. In a default/in-memory catalog, persistence is process/session scoped unless a custom catalog implementation persists metadata.

Agent policy:

```text
CREATE SCHEMA = namespace creation.
Do not assume durable metastore behavior unless custom CatalogProvider supplies it.
```

---

### S2.4.2 `CREATE EXTERNAL TABLE (...)`: explicit schema

```sql
CREATE EXTERNAL TABLE refinery_streams (
    stream_id       VARCHAR NOT NULL,
    case_id         VARCHAR NOT NULL,
    unit_id         VARCHAR NOT NULL,
    mass_flow_kg_h  DOUBLE  NOT NULL,
    sulfur_wt_pct   DOUBLE,
    event_ts        TIMESTAMP
)
STORED AS PARQUET
LOCATION 's3://lake/refinery/streams/';
```

`CREATE EXTERNAL TABLE` registers a local or remote file/object-store location as a named table. The DDL page documents CSV examples with inferred and explicit schemas, compressed CSV, Parquet external tables, and Hive partition discovery. ([Apache DataFusion][5])

Use explicit DDL schema when:

```text
CSV/JSON production ingestion
external users depend on field names/types
source inference is unstable
nullability must be governed
column order must be fixed
dirty data requires staged parsing
```

---

### S2.4.3 `CREATE EXTERNAL TABLE`: inferred schema

```sql
CREATE EXTERNAL TABLE raw_csv
STORED AS CSV
LOCATION '/data/raw/events.csv'
OPTIONS ('has_header' 'true');

CREATE EXTERNAL TABLE lake_facts
STORED AS PARQUET
LOCATION 's3://lake/facts/';
```

DataFusion’s DDL docs state that CSV schemas can be inferred by scanning a subset of the file and that Parquet table schema information can be inferred from the file metadata; Hive-style partition directories are also incorporated into schema/data. ([Apache DataFusion][5])

Inference policy:

```text
Parquet:
  usually infer; validate footer schema and directory compatibility.

Arrow IPC:
  infer embedded schema; validate contract if crossing service boundary.

Avro:
  infer embedded schema; test logical/nested/union mapping.

CSV/JSON:
  use inference for exploration only.
  for production, declare schema explicitly or ingest as all-text staging.
```

---

### S2.4.4 CTAS: `CREATE TABLE AS SELECT`

```sql
CREATE TABLE curated_streams AS
SELECT
    stream_id AS stream_id,
    case_id AS case_id,
    unit_id AS unit_id,
    CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h,
    CAST(sulfur_wt_pct AS DOUBLE) AS sulfur_wt_pct
FROM raw_streams
WHERE stream_id IS NOT NULL;
```

CTAS schema source:

```text
input table schema
  → SELECT expression schema
  → aliases/casts
  → query-derived output schema
  → table provider registration
```

Agent rules:

```text
Never use CTAS with SELECT * for stable contracts.
Alias every expression.
Cast every field whose output type matters.
Snapshot DESCRIBE curated_table after creation.
Default CREATE TABLE behavior may be in-memory unless catalog/provider changes semantics.
```

---

### S2.4.5 `CREATE VIEW`

```sql
CREATE OR REPLACE VIEW analytics.streams_v1 AS
SELECT
    stream_id AS stream_id,
    case_id AS case_id,
    unit_id AS unit_id,
    mass_flow_kg_h AS mass_flow_kg_h,
    sulfur_wt_pct AS sulfur_wt_pct
FROM prod.raw_streams;
```

View schema source:

```text
SELECT output schema
  → view table logical-plan schema
  → catalog-bound virtual table schema
```

Use views for:

```text
stable compatibility layer
source-field renaming
column masking/projection
type-normalized interface
schema-version facade
semantic objects over raw tables
```

View anti-patterns:

```text
CREATE VIEW v AS SELECT *
CREATE VIEW v AS SELECT SUM(x) FROM t
CREATE VIEW v AS SELECT "Bad Column Name" FROM raw
```

Preferred:

```sql
CREATE VIEW v AS
SELECT
  SUM(x) AS total_x,
  "Bad Column Name" AS bad_column_name
FROM raw;
```

---

### S2.4.6 DDL execution in Rust

```rust
use datafusion::prelude::*;

pub async fn create_schema_objects(ctx: &SessionContext) -> datafusion::error::Result<()> {
    ctx.sql("CREATE SCHEMA IF NOT EXISTS analytics")
        .await?
        .collect()
        .await?;

    ctx.sql(
        r#"
        CREATE EXTERNAL TABLE IF NOT EXISTS analytics.streams (
            stream_id VARCHAR NOT NULL,
            case_id VARCHAR NOT NULL,
            mass_flow_kg_h DOUBLE NOT NULL
        )
        STORED AS PARQUET
        LOCATION 's3://lake/refinery/streams/'
        "#
    )
    .await?
    .collect()
    .await?;

    Ok(())
}
```

DDL returns a `DataFrame` and must be executed through an action such as `collect()` for the side effect to occur. `SessionContext::enable_url_table` is security-sensitive and permits direct SQL access to arbitrary files such as `SELECT * FROM 'my_file.parquet'`; do not enable it in public or multi-tenant systems. ([Docs.rs][6])

---

## S2.5 Source-driven schema creation

### S2.5.1 Parquet footer schema

```rust
let df = ctx
    .read_parquet("s3://lake/refinery/streams/", ParquetReadOptions::default())
    .await?;

let schema = df.schema().clone();
```

```sql
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
LOCATION 's3://lake/refinery/streams/';
```

Parquet path policy:

```text
Use directory root.
Avoid wildcard paths.
Ensure all files under directory have compatible schemas.
Validate Hive partition columns.
Snapshot DESCRIBE table.
```

DataFusion’s CLI/source docs state that direct quoted paths can query files/directories/remote locations, directories can be queried when files have compatible schemas, and CSV schema can be inferred or specified explicitly. ([Apache DataFusion][7])

---

### S2.5.2 Arrow IPC schema

```rust
let df = ctx
    .read_arrow("data/events.arrow", ArrowReadOptions::default())
    .await?;
```

```sql
CREATE EXTERNAL TABLE events
STORED AS ARROW
LOCATION '/data/events.arrow';
```

Policy:

```text
Arrow IPC schema is embedded and Arrow-native.
Use for local high-speed columnar interchange.
Validate Arrow version compatibility and metadata preservation.
Prefer Parquet for compressed long-term lake storage.
```

---

### S2.5.3 Avro schema

```rust
let df = ctx
    .read_avro("data/events.avro", AvroReadOptions::default())
    .await?;
```

```sql
CREATE EXTERNAL TABLE events
STORED AS AVRO
LOCATION '/data/events.avro';
```

Policy:

```text
Avro schema is source-owned and schema-bearing.
Enable avro feature in minimized builds.
Test logical type mapping.
Test union/nullability behavior.
Prefer conversion to Parquet for repeated analytics.
```

---

### S2.5.4 CSV inference

```rust
let df = ctx
    .read_csv("data/raw_streams.csv", CsvReadOptions::new().has_header(true))
    .await?;
```

```sql
CREATE EXTERNAL TABLE raw_streams
STORED AS CSV
LOCATION '/data/raw_streams.csv'
OPTIONS ('has_header' 'true');
```

CSV inference risk classes:

```text
integer inferred but later float appears
float inferred but decimal intended
date string inferred as Utf8
nullability inferred from sample only
header case/special characters preserved
wide sparse file under-sampled
```

Production alternative:

```sql
CREATE EXTERNAL TABLE raw_streams (
    stream_id       VARCHAR,
    case_id         VARCHAR,
    mass_flow_raw   VARCHAR,
    sulfur_raw      VARCHAR,
    event_ts_raw    VARCHAR
)
STORED AS CSV
LOCATION '/data/raw_streams.csv'
OPTIONS ('has_header' 'true');
```

Then parse:

```sql
CREATE VIEW parsed_streams AS
SELECT
    stream_id,
    case_id,
    arrow_try_cast(mass_flow_raw, 'Float64') AS mass_flow_kg_h,
    arrow_try_cast(sulfur_raw, 'Float64') AS sulfur_wt_pct,
    arrow_try_cast(event_ts_raw, 'Timestamp(ns)') AS event_ts
FROM raw_streams;
```

---

### S2.5.5 JSON inference

```rust
let df = ctx
    .read_json("data/events.json", JsonReadOptions::default())
    .await?;
```

```sql
CREATE EXTERNAL TABLE events (
    event_id VARCHAR,
    payload VARCHAR
)
STORED AS JSON
LOCATION '/data/events.json';
```

JSON policy:

```text
Use inferred JSON schema for discovery.
Use explicit JSON schema for production ingestion.
Promote hot nested fields to typed struct/list/map or scalar columns.
Round-trip nested JSON-derived schema in tests.
```

---

### S2.5.6 Hive partition-derived columns

```text
s3://lake/streams/event_date=2026-05-24/site=baytown/part-0.parquet
s3://lake/streams/event_date=2026-05-24/site=rotterdam/part-1.parquet
```

```sql
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
LOCATION 's3://lake/streams/';

SELECT count(*)
FROM streams
WHERE site = 'baytown';
```

Hive partition policy:

```text
Partition columns are path-derived virtual/data columns.
Partition names are part of table schema.
Partition values are strings/coerced according to DataFusion behavior/version.
Avoid high-cardinality partition keys.
Reject table roots with inconsistent partition layouts.
Validate partition columns through DESCRIBE and query tests.
```

---

## S2.6 Provider-driven schema creation

### S2.6.1 `TableProvider::schema()`

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::catalog::{Session, TableProvider};
use datafusion::datasource::TableType;
use datafusion::error::Result;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct RemoteStreamsProvider {
    schema: SchemaRef,
    snapshot_id: String,
}

#[async_trait::async_trait]
impl TableProvider for RemoteStreamsProvider {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        todo!("return ExecutionPlan with schema matching projection")
    }
}
```

`TableProvider` requires `schema()`, `table_type()`, and `scan(...)`; it also exposes optional planning/write metadata such as constraints, table definition, insert/update/delete/truncate, statistics, and filter pushdown. ([Docs.rs][8]) Since DataFusion 54 the trait is declared as `TableProvider: Any + Debug + Sync + Send` — there is no `as_any` method to implement; downcast with `provider.downcast_ref::<T>()` via trait upcasting when concrete access is needed.

Provider schema rules:

```text
schema() must be stable for a query-planning snapshot.
schema() should be cheap: Arc clone, no hot-path network scan.
scan(projection) must project against schema() field order.
returned ExecutionPlan schema must match projection.
every emitted RecordBatch must match ExecutionPlan schema.
```

---

### S2.6.2 `ListingTable` provided schema

Conceptual registration:

```rust
ctx.register_listing_table(
    "streams",
    "s3://lake/refinery/streams/",
    listing_options,
    Some(provided_schema),
    None,
).await?;
```

Use provided schema when:

```text
file inference is too expensive
CSV/JSON inference is unstable
directory has empty partitions
schema must include virtual/partition columns
schema contract is governed externally
cold-start latency matters
```

Listing table policy:

```text
Provided schema must match actual file data after projection/decoding.
If provided schema lies, failures occur at scan/decode/batch boundary.
Cache listing/schema metadata with TTL and explicit invalidation.
Use source fingerprint to detect data/layout drift.
```

---

### S2.6.3 Dynamic / remote schema snapshots

Remote schema snapshot pattern:

```rust
#[derive(Debug, Clone)]
pub struct RemoteSchemaSnapshot {
    pub source_system: String,
    pub source_object: String,
    pub version: String,
    pub fetched_at_epoch_ms: i64,
    pub schema: SchemaRef,
    pub fingerprint: String,
}

#[async_trait::async_trait]
pub trait RemoteSchemaLoader {
    async fn load_schema_snapshot(&self, object: &str) -> datafusion::error::Result<RemoteSchemaSnapshot>;
}
```

Deployment pattern:

```text
startup / refresh task:
  load remote schema
  normalize to Arrow Schema
  fingerprint
  register provider with frozen SchemaRef

query planning:
  TableProvider::schema() returns frozen snapshot

refresh:
  compute diff against current
  reject incompatible drift
  swap provider/catalog binding atomically if accepted
```

Avoid:

```text
TableProvider::schema() performs remote API call per query
scan() silently changes schema relative to schema()
stream emits batches using newer remote schema than planning snapshot
```

---

## S2.7 Derived schemas

Derived schema = output schema computed by a relational/logical operator.

### S2.7.1 Projection output

```rust
let df = df.select(vec![
    col("stream_id").alias("stream_id"),
    (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d"),
])?;

let schema = df.schema();
```

Projection rule:

```text
output fields = selected expression fields
field names = aliases or expression-derived names
stable contract requires aliases
```

Bad:

```sql
SELECT mass_flow_kg_h * 24.0 FROM streams;
```

Good:

```sql
SELECT mass_flow_kg_h * 24.0 AS mass_flow_kg_d FROM streams;
```

---

### S2.7.2 Aggregate output

```rust
use datafusion::functions_aggregate::expr_fn::sum;

let df = df.aggregate(
    vec![col("unit_id")],
    vec![sum(col("mass_flow_kg_h")).alias("total_mass_flow_kg_h")],
)?;
```

Aggregate schema rule:

```text
output fields = group expressions + aggregate expressions
aggregate expression names must be aliased
aggregate output nullability depends on aggregate semantics and input nullability
```

SQL:

```sql
SELECT
  unit_id,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h
FROM streams
GROUP BY unit_id;
```

---

### S2.7.3 Join output

```sql
SELECT
  s.stream_id AS stream_id,
  s.unit_id AS stream_unit_id,
  u.unit_id AS unit_dim_id,
  u.unit_name AS unit_name
FROM streams AS s
JOIN units AS u
  ON s.unit_id = u.unit_id;
```

Join schema rule:

```text
output fields = left fields + right fields unless projection selects/renames
duplicate names require qualification during planning
stable output requires aliasing duplicate fields
```

Bad:

```sql
SELECT *
FROM streams s
JOIN units u ON s.unit_id = u.unit_id;
```

Good:

```sql
SELECT
  s.stream_id AS stream_id,
  s.unit_id AS stream_unit_id,
  u.unit_name AS unit_name
FROM streams s
JOIN units u ON s.unit_id = u.unit_id;
```

---

### S2.7.4 Union/coercion output

```rust
let combined = current.union_by_name(history)?;
```

Union schema rules:

```text
position-based union:
  schemas must match by field position/order

name-based union:
  fields aligned by name
  missing fields may be null-filled depending API/operator

coercion:
  output type may be supertype/common type
  lossy coercions must be rejected by factory policy
```

Factory policy:

```text
Use union_by_name for schema-evolution ingestion.
Use position union only for identical schema contracts.
Snapshot post-union schema.
Reject implicit output type changes without explicit cast plan.
```

---

### S2.7.5 Nested `unnest` output

Array unnest:

```sql
SELECT
  stream_id,
  unnest(tags) AS tag
FROM streams;
```

Struct unnest:

```sql
SELECT
  unnest(assay) 
FROM streams;
```

Unnest schema rules:

```text
array/list unnest:
  row cardinality changes
  output element field type = list element type

struct unnest:
  column shape changes
  output fields = struct child fields

map unnest:
  output shape depends on map representation/version
```

Agent rules:

```text
Always alias unnested scalar output.
Always snapshot unnest output schema.
Never expose unbounded unnest without cardinality controls.
```

---

## S2.8 `DFSchema` creation and conversion

```rust
use datafusion::common::{Column, DFSchema};

let arrow_schema = Schema::new(vec![
    Field::new("stream_id", DataType::Utf8, false),
    Field::new("mass_flow_kg_h", DataType::Float64, false),
]);

let qualified = DFSchema::try_from_qualified_schema("streams", &arrow_schema)?;
let unqualified = DFSchema::try_from(arrow_schema)?;
```

`DFSchema` supports qualified and unqualified schema creation, conversion back to Arrow schema, column lookup, qualifier replacement/stripping, joins, merges, Arrow compatibility checks, and logical/semantic type comparison helpers. ([Docs.rs][9])

Policy:

```text
Use Arrow Schema for physical data and provider contracts.
Use DFSchema for expression binding and query-derived schema validation.
Use qualified DFSchema for joins and multi-table generated queries.
Strip qualifiers only at output boundary.
```

---

## S2.9 Factory architecture

### S2.9.1 Module layout

```text
src/schema_factory/
  mod.rs
  contract.rs
  rust_native.rs
  sql_decl.rs
  source.rs
  provider.rs
  derived.rs
  sink.rs
  normalize.rs
  diff.rs
  fingerprint.rs
  validate.rs
```

### S2.9.2 Core types

```rust
use std::sync::Arc;
use std::collections::HashMap;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::DFSchema;

#[derive(Debug, Clone)]
pub struct SchemaContract {
    pub name: String,
    pub version: String,
    pub owner: SchemaOwner,
    pub schema: SchemaRef,
    pub fingerprint: String,
    pub metadata_policy: MetadataPolicy,
    pub compatibility_policy: CompatibilityPolicy,
}

#[derive(Debug, Clone)]
pub enum SchemaOwner {
    SourceOwned,
    CatalogOwned,
    ProviderOwned,
    QueryDerived,
    SinkOwned,
}

#[derive(Debug, Clone)]
pub enum CompatibilityPolicy {
    ExactPhysical,
    DataFusionLogical,
    CoercibleWithExplicitCasts,
    SemanticMetadataTolerant,
}

#[derive(Debug, Clone)]
pub struct MetadataPolicy {
    pub schema_keys_allowlist: Vec<String>,
    pub field_keys_allowlist: Vec<String>,
    pub reject_unknown_contract_keys: bool,
}

#[derive(Debug, Clone)]
pub struct SchemaFactoryOptions {
    pub canonicalize_names: bool,
    pub lowercase_names: bool,
    pub preserve_source_names_in_metadata: bool,
    pub default_nullable_for_inferred: bool,
    pub attach_fingerprint_metadata: bool,
}
```

---

## S2.10 `schema_factory::from_arrow`

### Purpose

Convert an existing Arrow `Schema` into a governed `SchemaContract`.

```rust
pub fn from_arrow(
    name: impl Into<String>,
    version: impl Into<String>,
    schema: SchemaRef,
    owner: SchemaOwner,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let normalized = if options.canonicalize_names {
        Arc::new(normalize_arrow_schema(schema.as_ref(), options)?)
    } else {
        schema
    };

    let fingerprint = fingerprint_schema(normalized.as_ref());

    Ok(SchemaContract {
        name: name.into(),
        version: version.into(),
        owner,
        schema: normalized,
        fingerprint,
        metadata_policy: default_metadata_policy(),
        compatibility_policy: CompatibilityPolicy::ExactPhysical,
    })
}
```

Normalization sketch:

```rust
pub fn normalize_arrow_schema(
    schema: &Schema,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<Schema> {
    let fields = schema
        .fields()
        .iter()
        .map(|field| normalize_field(field.as_ref(), options))
        .collect::<datafusion::error::Result<Vec<_>>>()?;

    let mut metadata = schema.metadata().clone();

    if options.attach_fingerprint_metadata {
        metadata.insert(
            "schema.normalized".to_string(),
            "true".to_string(),
        );
    }

    Ok(Schema::new_with_metadata(fields, metadata))
}

fn normalize_field(
    field: &Field,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<Field> {
    let name = if options.lowercase_names {
        canonical_name(field.name())
    } else {
        field.name().clone()
    };

    let mut metadata = field.metadata().clone();
    if options.preserve_source_names_in_metadata && name != *field.name() {
        metadata.insert("source.name".to_string(), field.name().clone());
    }

    Ok(Field::new(name, field.data_type().clone(), field.is_nullable())
        .with_metadata(metadata))
}
```

Use when:

```text
input is already Arrow-native
custom provider supplies Arrow schema
test fixture schema is code-authored
Parquet/Arrow/Avro schema has already been decoded externally
```

---

## S2.11 `schema_factory::from_sql_decl`

### Purpose

Create a schema contract from a DataFusion SQL DDL declaration.

Input options:

```text
full CREATE EXTERNAL TABLE statement
column-definition list from DDL
domain-specific schema DSL compiled to SQL types
```

Recommended implementation options:

```text
Option A — DataFusion planner path:
  execute CREATE EXTERNAL TABLE in isolated SessionContext
  retrieve table provider
  read provider.schema()
  convert to contract

Option B — sqlparser + DataFusion type adapter:
  parse DDL AST
  convert SQL data types to Arrow DataType using DataFusion-compatible mapping
  create Schema directly

Option C — contract DSL:
  avoid SQL as source of truth
  generate both SQL DDL and Arrow Schema from same typed contract
```

Factory sketch using isolated context:

```rust
pub async fn from_sql_decl_via_context(
    ddl: &str,
    table_name: &str,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let ctx = SessionContext::new();

    ctx.sql(ddl).await?.collect().await?;

    let provider = ctx.table_provider(table_name).await?;
    let schema = provider.schema();

    from_arrow(
        table_name,
        "ddl-derived",
        schema,
        SchemaOwner::CatalogOwned,
        options,
    )
}
```

Caveats:

```text
DDL execution may touch filesystem/object store for schema inference.
Use isolated temporary context.
Disable DDL in untrusted user contexts; only run trusted migration DDL.
If DDL points at remote files, schema creation can be async/IO-bound.
```

Use when:

```text
SQL DDL is the canonical interface
migration scripts define schema
CLI/admin users create tables
external SQL compatibility matters
```

Avoid when:

```text
schema contract is app-owned
query service is read-only
DDL contains credentials/paths
low-latency schema factory is required
```

---

## S2.12 `schema_factory::from_source`

### Purpose

Create schema from file/source metadata or inference.

```rust
#[derive(Debug, Clone)]
pub enum SourceSchemaOrigin {
    ParquetFooter,
    ArrowIpcEmbedded,
    AvroEmbedded,
    CsvInferred { sample_rows: Option<usize> },
    JsonInferred,
    HivePartitionColumns,
}

#[derive(Debug, Clone)]
pub struct SourceSchemaRequest {
    pub source_uri: String,
    pub format: SourceFormat,
    pub explicit_schema: Option<SchemaRef>,
    pub infer_options: HashMap<String, String>,
    pub origin: SourceSchemaOrigin,
}
```

Factory policy:

```rust
pub async fn from_source(
    ctx: &SessionContext,
    request: SourceSchemaRequest,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let df = match request.format {
        SourceFormat::Parquet => {
            ctx.read_parquet(
                request.source_uri,
                ParquetReadOptions::default(),
            ).await?
        }
        SourceFormat::Csv => {
            ctx.read_csv(
                request.source_uri,
                CsvReadOptions::new(),
            ).await?
        }
        SourceFormat::Json => {
            ctx.read_json(
                request.source_uri,
                JsonReadOptions::default(),
            ).await?
        }
        SourceFormat::Arrow => {
            ctx.read_arrow(
                request.source_uri,
                ArrowReadOptions::default(),
            ).await?
        }
        SourceFormat::Avro => {
            ctx.read_avro(
                request.source_uri,
                AvroReadOptions::default(),
            ).await?
        }
    };

    let arrow_schema = df.schema().as_arrow().clone();

    from_arrow(
        "source-derived",
        "inferred",
        Arc::new(arrow_schema),
        SchemaOwner::SourceOwned,
        options,
    )
}
```

Production caveat:

```text
The exact read_* signatures and option types should be verified against the pinned DataFusion version.
Use this as a factory pattern, not a stable cross-version API guarantee.
```

---

## S2.13 `schema_factory::from_provider`

### Purpose

Freeze and govern a `TableProvider::schema()` output.

```rust
use datafusion::catalog::TableProvider;

pub fn from_provider(
    table_name: impl Into<String>,
    provider: &dyn TableProvider,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let schema = provider.schema();

    from_arrow(
        table_name,
        "provider-snapshot",
        schema,
        SchemaOwner::ProviderOwned,
        options,
    )
}
```

Provider factory validation:

```rust
pub async fn validate_provider_projection_schema(
    provider: &dyn TableProvider,
    state: &dyn datafusion::catalog::Session,
    projection: Vec<usize>,
) -> datafusion::error::Result<()> {
    let full_schema = provider.schema();
    let expected = Arc::new(full_schema.project(&projection)?);

    let plan = provider
        .scan(state, Some(&projection), &[], None)
        .await?;

    if plan.schema().fields() != expected.fields() {
        return Err(datafusion::error::DataFusionError::Execution(format!(
            "provider projection schema mismatch: expected={expected:?}, actual={:?}",
            plan.schema()
        )));
    }

    Ok(())
}
```

Use when:

```text
custom provider is schema owner
remote schema is normalized into provider
catalog exposes provider-created table
agent wants table contract without executing scan
```

---

## S2.14 `schema_factory::from_contract`

### Purpose

Use a domain/schema DSL as the canonical schema source and emit Arrow schema, SQL DDL, provider contract, and sink contract from the same object.

```rust
#[derive(Debug, Clone)]
pub struct FieldContract {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub semantic_type: Option<String>,
    pub unit: Option<String>,
    pub source_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TableContract {
    pub catalog: Option<String>,
    pub schema: Option<String>,
    pub table: String,
    pub version: String,
    pub fields: Vec<FieldContract>,
    pub partition_by: Vec<String>,
    pub metadata: HashMap<String, String>,
}

pub fn from_contract(
    contract: &TableContract,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let fields = contract
        .fields
        .iter()
        .map(|f| {
            let mut metadata = HashMap::new();

            if let Some(v) = &f.semantic_type {
                metadata.insert("semantic.type".to_string(), v.clone());
            }
            if let Some(v) = &f.unit {
                metadata.insert("semantic.unit".to_string(), v.clone());
            }
            if let Some(v) = &f.source_name {
                metadata.insert("source.name".to_string(), v.clone());
            }

            Field::new(&f.name, f.data_type.clone(), f.nullable)
                .with_metadata(metadata)
        })
        .collect::<Vec<_>>();

    let schema = Arc::new(Schema::new_with_metadata(fields, contract.metadata.clone()));

    from_arrow(
        format!(
            "{}{}{}",
            contract.schema.as_deref().unwrap_or(""),
            if contract.schema.is_some() { "." } else { "" },
            contract.table
        ),
        contract.version.clone(),
        schema,
        SchemaOwner::CatalogOwned,
        options,
    )
}
```

Value case:

```text
One source of truth produces:
  Arrow Schema
  CREATE EXTERNAL TABLE DDL
  SQL view projection
  TableProvider schema
  sink schema
  JSON manifest
  test fixtures
```

---

## S2.15 `schema_factory::for_sink`

### Purpose

Produce a sink/output schema that is explicit, stable, and validated before write/stream.

```rust
#[derive(Debug, Clone)]
pub enum SinkKind {
    Parquet,
    Csv,
    Json,
    ArrowIpc,
    TableInsert { target_table: String },
    ApiResponse,
}

#[derive(Debug, Clone)]
pub struct SinkSchemaRequest {
    pub sink_kind: SinkKind,
    pub input_df_schema: DFSchema,
    pub strip_qualifiers: bool,
    pub require_unique_names: bool,
    pub metadata_policy: MetadataPolicy,
}
```

Factory sketch:

```rust
pub fn for_sink(
    name: impl Into<String>,
    request: SinkSchemaRequest,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let mut df_schema = request.input_df_schema;

    if request.strip_qualifiers {
        df_schema = df_schema.strip_qualifiers();
    }

    if request.require_unique_names {
        ensure_unique_field_names(&df_schema)?;
    }

    let arrow_schema = Arc::new(df_schema.as_arrow().clone());

    from_arrow(
        name,
        "sink-output",
        arrow_schema,
        SchemaOwner::SinkOwned,
        options,
    )
}
```

Sink policy matrix:

| Sink         | Schema concern                                                    | Required action                   |
| ------------ | ----------------------------------------------------------------- | --------------------------------- |
| Parquet      | Arrow logical/physical metadata, nested fields, partition columns | read-after-write schema test      |
| CSV          | header names, scalar serialization, loss of nested fidelity       | flatten or reject nested columns  |
| JSON         | object keys, nested serialization, null behavior                  | snapshot sample + schema manifest |
| Arrow IPC    | Arrow schema fidelity                                             | version compatibility test        |
| Table insert | target provider compatibility                                     | preflight target schema diff      |
| API response | unique names, no qualifiers, stable field order                   | include schema/fingerprint        |

---

## S2.16 Derived-schema factory examples

### Projection factory

```rust
pub async fn projected_contract(
    ctx: &SessionContext,
    sql: &str,
    name: &str,
    options: &SchemaFactoryOptions,
) -> datafusion::error::Result<SchemaContract> {
    let df = ctx.sql(sql).await?;
    let arrow_schema = Arc::new(df.schema().as_arrow().clone());

    from_arrow(
        name,
        "query-derived",
        arrow_schema,
        SchemaOwner::QueryDerived,
        options,
    )
}
```

### Aggregate stable schema SQL

```sql
SELECT
  unit_id AS unit_id,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h,
  AVG(sulfur_wt_pct) AS avg_sulfur_wt_pct
FROM streams
GROUP BY unit_id;
```

### Join stable schema SQL

```sql
SELECT
  s.stream_id AS stream_id,
  s.unit_id AS stream_unit_id,
  u.unit_name AS unit_name
FROM streams AS s
JOIN units AS u
  ON s.unit_id = u.unit_id;
```

### Union stable schema SQL

```sql
SELECT
  stream_id,
  case_id,
  CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h
FROM current_streams

UNION ALL

SELECT
  stream_id,
  case_id,
  CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h
FROM historical_streams;
```

---

## S2.17 Schema factory validation phases

```text
Phase 0 — parse / build
  validate field names
  validate data types
  validate nullability policy
  validate duplicate fields

Phase 1 — normalize
  canonical names
  canonical metadata keys
  canonical timestamp/timezone policy
  canonical decimal policy
  canonical nested field order policy

Phase 2 — fingerprint
  stable schema hash
  metadata subset hash
  field-id hash if available

Phase 3 — compatibility
  compare against registered contract
  emit drift report
  reject/accept/warn/migrate

Phase 4 — bind
  register catalog/provider/table
  expose TableProvider::schema()
  derive DFSchema for planning

Phase 5 — execution validation
  validate RecordBatch
  validate stream schema
  validate sink schema

Phase 6 — persist/audit
  write schema manifest
  snapshot DESCRIBE / information_schema
  record source/output fingerprints
```

---

## S2.18 Schema drift report

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiff {
    pub expected_name: String,
    pub expected_fingerprint: String,
    pub actual_fingerprint: String,
    pub findings: Vec<SchemaDiffFinding>,
    pub decision: SchemaDiffDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiffFinding {
    pub path: String,
    pub kind: SchemaDiffKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub severity: Severity,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SchemaDiffKind {
    MissingField,
    ExtraField,
    TypeMismatch,
    NullabilityMismatch,
    FieldOrderMismatch,
    MetadataMismatch,
    NestedFieldMismatch,
    PartitionColumnMismatch,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SchemaDiffDecision {
    Accept,
    AcceptWithWarning,
    Reject,
    RequiresMigration,
}
```

Rejection defaults:

```text
missing required column      -> reject
type narrowing              -> reject
nullable to non-nullable     -> reject unless data proof exists
partition column change      -> reject
duplicate output field       -> reject
unknown SQL type             -> reject
metadata-only drift          -> warn unless metadata is contract key
extra source column          -> ignore/warn depending policy
type widening                -> allow only if declared lossless
```

---

## S2.19 Factory-driven deployment pattern

```text
startup:
  load TableContract registry
  build SchemaContract objects
  register object stores
  register providers with frozen SchemaRef
  enable information_schema only if allowed

ingestion:
  source schema discovery
  normalize
  diff against expected contract
  reject drift or create migration artifact
  register staging table

query planning:
  build SQL/DataFrame from registered tables
  validate derived df.schema()
  build sink contract

execution:
  stream RecordBatch
  validate schema under strict/debug mode
  write output with schema manifest

upgrade:
  rerun factory tests under new DataFusion version
  compare schema fingerprints and DESCRIBE output
  approve/deny migration
```

---

## S2.20 Best-practice deployment advisory

```text
Rust-native schemas:
  use generated functions for application-owned contracts
  keep schema factories deterministic
  use metadata allowlists
  use RecordBatch::try_new in every test/adaptor

SQL DDL:
  treat DDL as migration/admin surface
  do not run untrusted DDL
  collect() DDL DataFrames to apply side effects
  snapshot DESCRIBE after creation

Source-driven:
  trust Parquet/Arrow/Avro embedded schemas only after validation
  avoid CSV/JSON inference in production
  use all-text staging for dirty CSV/JSON
  reject incompatible multi-file directories

Provider-driven:
  schema() must be cheap/stable
  dynamic remote schemas require snapshots
  projection indices must map against provider schema
  provider tests must validate projected ExecutionPlan schema

Derived:
  alias every computed expression
  cast every contract-sensitive output
  avoid SELECT * in CTAS/views/APIs
  validate df.schema() before execution

Sink:
  create explicit sink schema
  reject duplicate output names
  include output schema fingerprint
  read-after-write Parquet/Arrow schema tests
```

---

## S2.21 Anti-pattern inventory

```text
Schema creation anti-patterns:
  hand-written SQL DDL and Rust Schema drift apart
  generated schemas from HashMap iteration with nondeterministic field order
  CSV inference used as production contract
  JSON inferred nested shape treated as stable API
  `SELECT *` in CTAS or CREATE VIEW
  unaliased aggregate / arithmetic / window expressions
  TableProvider::schema() performs live remote scan
  scan(projection) returns backend order rather than schema projection order
  source partition columns conflict with file columns
  sink schema inferred from first non-empty batch
  empty result creates no schema manifest
  metadata keys accepted without allowlist
  direct arrow crate imports incompatible with DataFusion’s Arrow version
  RecordBatch::new_unchecked used in provider code
  DDL accepted from public SQL endpoint
  direct URL-table access enabled in multi-tenant service
```

---

## S2.22 Agent checklist

```text
[ ] Identify schema creation surface:
    RustNative / SqlDdl / SourceDriven / ProviderDriven / Derived / SinkOutput.

[ ] Assign schema owner:
    source / catalog / provider / query / sink.

[ ] Choose factory:
    from_sql_decl
    from_arrow
    from_provider
    from_contract
    for_sink
    from_source

[ ] Normalize:
    names
    types
    nullability
    metadata
    nested field policy
    partition fields

[ ] Validate:
    no duplicate fields
    no unsupported types
    deterministic field order
    RecordBatch compatibility where data exists
    provider projection compatibility
    sink compatibility

[ ] Stabilize derived schema:
    aliases
    explicit casts
    qualified columns before join output
    no SELECT * for stable contracts

[ ] Fingerprint:
    canonical schema string
    field order
    data types
    nullability
    contract metadata subset

[ ] Register:
    catalog/schema/table only after validation.

[ ] Snapshot:
    DESCRIBE
    information_schema.columns
    df.schema()
    output sink schema
    read-after-write schema

[ ] Reject drift:
    missing required fields
    lossy type change
    nullability tightening
    partition column drift
    duplicate output names
```

## S2.23 Minimal canonical schema factory skeleton

```rust
pub mod schema_factory {
    use std::collections::HashMap;
    use std::sync::Arc;

    use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
    use datafusion::catalog::TableProvider;
    use datafusion::common::DFSchema;
    use datafusion::error::{DataFusionError, Result};

    #[derive(Debug, Clone)]
    pub enum SchemaOwner {
        SourceOwned,
        CatalogOwned,
        ProviderOwned,
        QueryDerived,
        SinkOwned,
    }

    #[derive(Debug, Clone)]
    pub struct SchemaContract {
        pub name: String,
        pub version: String,
        pub owner: SchemaOwner,
        pub schema: SchemaRef,
        pub fingerprint: String,
    }

    #[derive(Debug, Clone)]
    pub struct SchemaFactoryOptions {
        pub lowercase_names: bool,
        pub preserve_source_names: bool,
        pub attach_contract_metadata: bool,
    }

    pub fn from_arrow(
        name: impl Into<String>,
        version: impl Into<String>,
        owner: SchemaOwner,
        schema: SchemaRef,
        options: &SchemaFactoryOptions,
    ) -> Result<SchemaContract> {
        let normalized = Arc::new(normalize_schema(schema.as_ref(), options)?);
        let fingerprint = fingerprint_schema(normalized.as_ref());

        Ok(SchemaContract {
            name: name.into(),
            version: version.into(),
            owner,
            schema: normalized,
            fingerprint,
        })
    }

    pub fn from_provider(
        name: impl Into<String>,
        provider: &dyn TableProvider,
        options: &SchemaFactoryOptions,
    ) -> Result<SchemaContract> {
        from_arrow(
            name,
            "provider-snapshot",
            SchemaOwner::ProviderOwned,
            provider.schema(),
            options,
        )
    }

    pub fn for_sink(
        name: impl Into<String>,
        df_schema: &DFSchema,
        options: &SchemaFactoryOptions,
    ) -> Result<SchemaContract> {
        let stripped = df_schema.clone().strip_qualifiers();
        ensure_unique_names(&stripped)?;

        from_arrow(
            name,
            "sink-output",
            SchemaOwner::SinkOwned,
            Arc::new(stripped.as_arrow().clone()),
            options,
        )
    }

    fn normalize_schema(schema: &Schema, options: &SchemaFactoryOptions) -> Result<Schema> {
        let fields = schema
            .fields()
            .iter()
            .map(|f| normalize_field(f.as_ref(), options))
            .collect::<Result<Vec<_>>>()?;

        let mut metadata = schema.metadata().clone();

        if options.attach_contract_metadata {
            metadata.insert("schema.factory.normalized".to_string(), "true".to_string());
        }

        Ok(Schema::new_with_metadata(fields, metadata))
    }

    fn normalize_field(field: &Field, options: &SchemaFactoryOptions) -> Result<Field> {
        let original = field.name().clone();
        let normalized = if options.lowercase_names {
            canonical_name(&original)
        } else {
            original.clone()
        };

        let mut metadata = field.metadata().clone();

        if options.preserve_source_names && normalized != original {
            metadata.insert("source.name".to_string(), original);
        }

        Ok(Field::new(normalized, field.data_type().clone(), field.is_nullable())
            .with_metadata(metadata))
    }

    fn ensure_unique_names(df_schema: &DFSchema) -> Result<()> {
        let mut seen = std::collections::HashSet::new();

        for field in df_schema.fields() {
            if !seen.insert(field.name().clone()) {
                return Err(DataFusionError::Execution(format!(
                    "duplicate output field name in sink schema: {}",
                    field.name()
                )));
            }
        }

        Ok(())
    }

    fn canonical_name(name: &str) -> String {
        name.trim()
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    }

    fn fingerprint_schema(schema: &Schema) -> String {
        // Replace with SHA-256 over this canonical string in production.
        let mut s = String::new();

        for (idx, field) in schema.fields().iter().enumerate() {
            s.push_str(&format!(
                "{idx}|{}|{:?}|nullable={}\n",
                field.name(),
                field.data_type(),
                field.is_nullable()
            ));
        }

        format!("debug-fingerprint:{}", s.len())
    }
}
```

The operating principle: **schema creation should be centralized, typed, normalized, fingerprinted, and policy-checked before a schema is registered, queried, written, or exposed to agents.**

[1]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html?utm_source=chatgpt.com "SessionContext in datafusion::execution::context - Rust"
[3]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Field.html "Field in arrow::datatypes - Rust"
[4]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html "RecordBatch in arrow::record_batch - Rust"
[5]: https://datafusion.apache.org/user-guide/sql/ddl.html?utm_source=chatgpt.com "DDL — Apache DataFusion documentation"
[6]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
[7]: https://datafusion.apache.org/user-guide/cli/datasources.html "Local Files / Directories — Apache DataFusion  documentation"
[8]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html "TableProvider in datafusion::datasource - Rust"
[9]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html "DFSchema in datafusion::common - Rust"


# DataFusion Advanced — S3) Schema inference, explicit overrides, and multi-file drift

## S3.0 Objective

Make file/dataset schema inference **predictable, auditable, reproducible, and rejectable**. Inference is useful for exploration and file-format convenience; it is not a sufficient production contract by itself.

```text
source files / directories / object-store prefixes
  → file-format schema discovery
  → optional explicit override
  → optional partition-column discovery
  → schema normalization
  → compatibility merge / drift check
  → TableProvider schema
  → DataFrame / SQL query schema
  → batch-level and sink-level validation
```

The attached document already covers the baseline: built-in CSV/Parquet/JSON/Avro/Arrow sources, schema inference vs explicit schemas, directory datasets, Hive-style partitions, Arrow `Schema::try_merge`, and provider tests, but S3 fills the missing operational layer: **how agents decide whether to trust inference, override it, stage as text, reject drift, or create a migration artifact.** 

---

## S3.1 Inference posture

```text
Inference modes:
  Exploratory inference
    → acceptable for ad hoc analysis, CLI, local notebooks, schema discovery.

  Verified inference
    → acceptable when source format is schema-bearing and schema is snapshot-tested.

  Governed explicit schema
    → required for production CSV/JSON ingestion, public APIs, table contracts, writes.

  All-text staging
    → required when raw data quality is unknown and typed parsing must be audited.

  Provider snapshot
    → required for remote/dynamic schemas, multi-tenant catalogs, remote APIs.

  Contract-first schema
    → required for stable platform APIs, semantic objects, generated plans, sinks.
```

DataFusion can register external files/directories through `CREATE EXTERNAL TABLE`; supported file types in the documented syntax are `CSV`, `ARROW`, `PARQUET`, `AVRO`, and `JSON`, and `LOCATION` can point to a file or directory on a local filesystem or object store. Parquet does not require explicit schema information, while CSV schema is inferred by scanning a subset unless manually specified. ([Apache DataFusion][1])

Core agent rule:

```text
Schema inference creates an initial hypothesis.
Only validation, normalization, and compatibility policy turn that hypothesis into a contract.
```

---

## S3.2 Format inference matrix

| Format          | Schema source                            | Inference stability | Multi-file risk | Production stance                                   |
| --------------- | ---------------------------------------- | ------------------: | --------------: | --------------------------------------------------- |
| CSV             | sampling + options + optional header     |          low/medium |            high | explicit schema or all-`Utf8` staging               |
| JSON            | sampling / object-shape discovery        |          low/medium |            high | explicit schema or staged normalization             |
| Parquet         | file footer metadata                     |                high |          medium | trust footer, validate directory compatibility      |
| Avro            | embedded Avro schema                     |                high |          medium | trust embedded schema, test logical/union mapping   |
| Arrow IPC       | embedded Arrow schema                    |           very high |          medium | trust embedded schema, validate Arrow compatibility |
| Hive partitions | directory path `key=value`               |              medium |            high | validate partition layout and column conflicts      |
| Remote provider | remote metadata snapshot                 |            variable |            high | freeze snapshot and diff on refresh                 |
| ListingTable    | file metadata + optional provided schema |            variable |     medium/high | provide schema when inference is expensive/unstable |

DataFusion’s `SessionContext` has direct reader APIs for CSV, JSON, Parquet, Avro, and Arrow; `read_csv` and `read_json` accept `DataFilePaths`, and docs note that for more control, including multiple-file cases, `read_table` with `ListingTable` is available. `read_parquet` is feature-gated by `parquet`. ([Docs.rs][2])

---

## S3.3 CSV sampling

### S3.3.1 SQL inference

```sql
CREATE EXTERNAL TABLE raw_streams
STORED AS CSV
LOCATION '/data/raw/streams.csv'
OPTIONS ('has_header' 'true');
```

DataFusion’s DDL docs state that CSV external-table schema is inferred based on scanning a subset of the file; the docs also show explicit schema declaration for CSV and compressed CSV registration. ([Apache DataFusion][1])

### S3.3.2 Rust inference

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();

let df = ctx
    .read_csv("data/raw/streams.csv", CsvReadOptions::new().has_header(true))
    .await?;

let inferred = df.schema();
println!("{inferred:#?}");
```

### S3.3.3 CSV inference failure modes

```text
Numeric drift:
  sample: 1, 2, 3
  later: 4.5
  inferred: Int64/Int32
  runtime/scan: parse error or null/failed cast depending path

Mixed integer/string:
  sample: 1001, 1002
  later: "N/A"
  inferred: numeric
  production expectation: staged string + typed parse with rejection query

Date/time ambiguity:
  "01/02/2026"
  could mean Jan 2 or Feb 1
  inferred as Utf8 or misparsed if format configured incorrectly

Sparse columns:
  first N rows empty
  later non-empty values
  inferred nullable Utf8 or Null-like behavior depending options/version

Header instability:
  "Mass Flow kg/h"
  spaces/slashes/case preserved
  SQL needs quoting unless normalized

Null token ambiguity:
  "", "NULL", "N/A", "-", "nan"
  must be configured or staged

Delimiter/quote/newline ambiguity:
  inconsistent quoting
  embedded delimiters
  newline-in-values
  malformed rows
```

### S3.3.4 Production CSV policy

```text
Production CSV ingestion options:
  A. explicit typed schema + fail-fast parsing
  B. all-Utf8 staging schema + arrow_try_cast validation
  C. custom provider/adapter with row-level rejection records
```

Fail-fast typed external table:

```sql
CREATE EXTERNAL TABLE raw_streams_typed (
    stream_id       VARCHAR,
    case_id         VARCHAR,
    mass_flow_kg_h  DOUBLE,
    sulfur_wt_pct   DOUBLE,
    event_ts        TIMESTAMP
)
STORED AS CSV
LOCATION '/data/raw/streams.csv'
OPTIONS (
  'has_header' 'true',
  'format.delimiter' ',',
  'null_value' ''
);
```

All-text staging:

```sql
CREATE EXTERNAL TABLE raw_streams_stage (
    stream_id_raw   VARCHAR,
    case_id_raw     VARCHAR,
    mass_flow_raw   VARCHAR,
    sulfur_raw      VARCHAR,
    event_ts_raw    VARCHAR
)
STORED AS CSV
LOCATION '/data/raw/streams.csv'
OPTIONS ('has_header' 'true');
```

Typed accepted rows:

```sql
CREATE VIEW streams_accepted AS
SELECT
    stream_id_raw AS stream_id,
    case_id_raw AS case_id,
    arrow_try_cast(mass_flow_raw, 'Float64') AS mass_flow_kg_h,
    arrow_try_cast(sulfur_raw, 'Float64') AS sulfur_wt_pct,
    arrow_try_cast(event_ts_raw, 'Timestamp(ns)') AS event_ts
FROM raw_streams_stage
WHERE arrow_try_cast(mass_flow_raw, 'Float64') IS NOT NULL
  AND arrow_try_cast(event_ts_raw, 'Timestamp(ns)') IS NOT NULL;
```

Rejected rows:

```sql
CREATE VIEW streams_rejected AS
SELECT
    *,
    CASE
      WHEN mass_flow_raw IS NOT NULL
       AND arrow_try_cast(mass_flow_raw, 'Float64') IS NULL
      THEN 'invalid_mass_flow'
      WHEN event_ts_raw IS NOT NULL
       AND arrow_try_cast(event_ts_raw, 'Timestamp(ns)') IS NULL
      THEN 'invalid_event_ts'
      ELSE 'unknown'
    END AS reject_reason
FROM raw_streams_stage
WHERE (mass_flow_raw IS NOT NULL AND arrow_try_cast(mass_flow_raw, 'Float64') IS NULL)
   OR (event_ts_raw IS NOT NULL AND arrow_try_cast(event_ts_raw, 'Timestamp(ns)') IS NULL);
```

### S3.3.5 CSV format options governance

DataFusion format options can be specified in `CREATE EXTERNAL TABLE`, `COPY`, or session-level config defaults, with documented precedence from DDL syntax to `COPY` option tuples to session-level defaults. Use table-specific options for durable ingestion contracts, not ambient session defaults. ([Apache DataFusion][3])

Agent rules:

```text
CSV:
  use explicit schema for production
  configure header/delimiter/null/quote/escape/date/timestamp options deliberately
  use all-Utf8 staging when dirty data must not abort whole ingestion
  snapshot DESCRIBE output
  snapshot arrow_typeof parsed outputs
  maintain rejected-row query
```

---

## S3.4 JSON inference

### S3.4.1 Rust inference

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();

let df = ctx
    .read_json("data/raw/events.json", JsonReadOptions::default())
    .await?;

println!("{:#?}", df.schema());
```

### S3.4.2 SQL explicit JSON table

```sql
CREATE EXTERNAL TABLE raw_events (
    event_id     VARCHAR,
    event_type   VARCHAR,
    payload      VARCHAR,
    event_ts_raw VARCHAR
)
STORED AS JSON
LOCATION '/data/raw/events.json';
```

### S3.4.3 JSON inference hazards

```text
Object-shape drift:
  file1: {"a": 1, "b": "x"}
  file2: {"a": 2, "c": "y"}
  risk: missing/extra fields, nullable widening, inconsistent nested struct shape

Scalar type drift:
  row1: {"amount": 1}
  row2: {"amount": 1.25}
  row3: {"amount": "N/A"}

Array element drift:
  row1: {"tags": ["a", "b"]}
  row2: {"tags": [1, 2]}
  row3: {"tags": null}

Nested drift:
  row1: {"device": {"id": "d1"}}
  row2: {"device": "unknown"}

Null/missing ambiguity:
  missing field != explicit null != empty object != empty list

Numeric ambiguity:
  integer vs float vs decimal-intended money
```

### S3.4.4 JSON production patterns

#### Pattern A — explicit scalar schema

```sql
CREATE EXTERNAL TABLE raw_events (
    event_id   VARCHAR,
    event_type VARCHAR,
    user_id    VARCHAR,
    event_ts   TIMESTAMP
)
STORED AS JSON
LOCATION 's3://lake/raw/events/';
```

#### Pattern B — raw payload + typed views

```sql
CREATE EXTERNAL TABLE raw_events (
    event_id     VARCHAR,
    payload_json VARCHAR
)
STORED AS JSON
LOCATION 's3://lake/raw/events/';
```

#### Pattern C — nested contract, version-pinned tests

```sql
CREATE VIEW events_normalized AS
SELECT
    event_id AS event_id,
    payload['user']['id'] AS user_id,
    payload['device']['os'] AS device_os,
    arrow_try_cast(payload['metrics']['latency_ms'], 'Float64') AS latency_ms
FROM raw_events;
```

Agent rules:

```text
JSON:
  use inference for discovery only
  avoid making inferred nested shape a public contract without tests
  prefer explicit schema or raw-payload staging
  promote hot fields to typed columns/views
  test null/missing/nested drift explicitly
```

---

## S3.5 Parquet metadata inference

### S3.5.1 SQL registration

```sql
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
LOCATION 's3://lake/refinery/streams/';
```

Parquet tables can be registered without supplying schema information because schema is derived from Parquet file metadata; DataFusion may read file metadata/statistics during table creation, and `datafusion.execution.collect_statistics = false` can disable the statistics collection before creating the table. ([Apache DataFusion][1])

### S3.5.2 Rust direct read

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();

let df = ctx
    .read_parquet("s3://lake/refinery/streams/", ParquetReadOptions::default())
    .await?;

let df_schema = df.schema();
```

### S3.5.3 Parquet inference strengths

```text
file stores schema in footer
column types are explicit
nested schema is explicit
statistics can support planning/pruning
schema discovery does not depend on row sampling
```

### S3.5.4 Parquet inference risks

```text
multi-file schema drift:
  part-000.parquet: amount Float64
  part-001.parquet: amount Utf8

metadata-only mismatch:
  same field/type, different field metadata

nullability drift:
  old file non-nullable
  new file nullable

timestamp drift:
  ns vs us vs ms
  timezone vs no timezone
  INT96 legacy timestamp handling

decimal drift:
  precision/scale differs

nested drift:
  struct field added/dropped
  list item type changed
  map representation differs

partition conflict:
  path has year=2026
  file also contains year with conflicting type/value

extension/logical type drift:
  Arrow extension metadata inconsistent
  Parquet logical annotation unsupported or mapped unexpectedly
```

### S3.5.5 Trust policy

```text
Trust Parquet footer when:
  data is produced by governed writer
  directory files share compatible schemas
  partition layout is stable
  read-after-write tests exist
  schema fingerprint is tracked

Do not blindly trust when:
  prefix is externally supplied
  mixed producers write same directory
  append-only object-store prefix lacks transaction/manifest layer
  schema has evolved without compatibility policy
  producer changed Arrow/Parquet versions
```

### S3.5.6 Recommended Parquet audit

```sql
DESCRIBE streams;

SELECT
  arrow_typeof(stream_id) AS stream_id_type,
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM streams
LIMIT 1;
```

Rust:

```rust
let df = ctx
    .read_parquet("s3://lake/refinery/streams/", ParquetReadOptions::default())
    .await?;

let schema = df.schema();
println!("{schema:#?}");
```

---

## S3.6 Avro schema inference

### S3.6.1 Rust read

```rust
use datafusion::prelude::*;

let df = ctx
    .read_avro("data/events.avro", AvroReadOptions::default())
    .await?;
```

### S3.6.2 SQL registration

```sql
CREATE EXTERNAL TABLE events
STORED AS AVRO
LOCATION '/data/events.avro';
```

`CREATE EXTERNAL TABLE` supports `AVRO` as a documented file type, while DataFusion’s Rust direct readers include `read_avro`; feature-minimized builds must enable Avro support explicitly. ([Apache DataFusion][1]) ([Docs.rs][2])

### S3.6.3 Avro inference hazards

```text
Avro union mapping:
  ["null", "double"] → nullable Float64-like semantic
  complex unions may map awkwardly or require validation

logical types:
  decimal
  date
  timestamp-millis / timestamp-micros
  uuid-like strings
  duration

schema evolution:
  default values
  aliases
  reader/writer schema compatibility
  missing/extra fields

nested records:
  record namespace/name ignored or preserved differently from field names
```

### S3.6.4 Avro policy

```text
Use Avro inference when:
  upstream Avro schema is governed
  logical-type mapping is test-covered
  union/nullability behavior is accepted
  conversion to Arrow schema is audited

Prefer conversion/staging when:
  unions are complex
  logical types are domain-critical
  producer schema evolution is active
  downstream output is Parquet/Arrow contract
```

---

## S3.7 Arrow IPC schema inference

### S3.7.1 Rust read

```rust
let df = ctx
    .read_arrow("data/events.arrow", ArrowReadOptions::default())
    .await?;
```

### S3.7.2 SQL registration

```sql
CREATE EXTERNAL TABLE events
STORED AS ARROW
LOCATION '/data/events.arrow';
```

Arrow IPC is the most direct schema-carrying path because the file is already Arrow-native. The core risk is less “inference quality” and more **version/interoperability/metadata-preservation**.

### S3.7.3 Arrow IPC policy

```text
Trust Arrow IPC schema when:
  producer uses compatible Arrow version/metadata conventions
  DataFusion consumes same Arrow type universe via datafusion::arrow
  schema fingerprint matches expected contract

Validate:
  nested field representation
  extension metadata
  dictionary/string fields
  timestamp/timezone fields
  nullable flags
```

---

## S3.8 Directory datasets

### S3.8.1 Directory model

```text
directory table
  ├─ file-000.parquet
  ├─ file-001.parquet
  ├─ file-002.parquet
  └─ optional Hive partitions:
      event_date=2026-05-24/site=baytown/file.parquet
```

DataFusion supports external-table locations that point to directories of partitioned files; for Parquet directory registration, all files should be valid compatible Parquet files. The CLI docs also state that wildcards are not supported and recommend directory paths so DataFusion reads compatible files within that directory; Hive-compliant partition directories are parsed and incorporated into table schema/data. ([Apache DataFusion][1]) ([Apache DataFusion][4])

### S3.8.2 Compatibility dimensions

```text
Field names:
  same set
  same case
  same aliases
  no duplicates

Field order:
  same order for strict physical contract
  name-based merge if policy allows

Types:
  exact Arrow type
  coercible widening
  lossy conflict
  unsupported logical type

Nullability:
  identical
  widening false→true
  tightening true→false

Metadata:
  identical
  advisory drift
  contract metadata drift

Nested:
  same struct field names
  same list item type/nullability
  same map key/value type
  compatible nested metadata

Partitions:
  same partition keys
  same partition depth
  partition values parse consistently
  no conflict with file columns
```

### S3.8.3 Multi-file schema matrix

| Case                   | Example                                          | Default production decision                            |
| ---------------------- | ------------------------------------------------ | ------------------------------------------------------ |
| exact match            | all files `amount: Float64 nullable=true`        | accept                                                 |
| nullability widening   | old `nullable=false`, new `nullable=true`        | accept with warning or migrate                         |
| nullability tightening | old `nullable=true`, new `nullable=false`        | do not tighten global contract without full validation |
| extra nullable field   | new file adds `quality_code: Utf8 nullable=true` | accept only if evolution policy allows                 |
| missing required field | file lacks `stream_id`                           | reject                                                 |
| type widening          | `Int32` → `Int64`                                | accept only with explicit coercion policy              |
| type conflict          | `Float64` vs `Utf8`                              | reject or all-`Utf8` staging                           |
| decimal drift          | `Decimal128(20,2)` vs `Decimal128(20,4)`         | reject unless explicit scale policy                    |
| timestamp drift        | `Timestamp(us)` vs `Timestamp(ns)`               | explicit cast/migration required                       |
| nested field add       | struct adds nullable child                       | accept only if nested evolution allowed                |
| partition conflict     | file has `year:Int32`, path has `year=2026`      | reject or rename partition column                      |

### S3.8.4 Schema merge semantics

Arrow `Schema::try_merge` merges compatible schemas and recursively merges struct fields; the docs example shows nullability widening when one schema has `c1` non-nullable and another has `c1` nullable, and it adds a new field `c3`. Use this carefully: Arrow-level compatibility does not automatically equal platform-level compatibility. ([Docs.rs][5])

Policy wrapper:

```rust
#[derive(Debug, Clone)]
pub enum MergePolicy {
    Exact,
    AllowNullableWidening,
    AllowAddNullableFields,
    AllowLosslessNumericWidening,
    AllowMetadataOnlyDrift,
    RejectAllImplicitDrift,
}
```

Agent rule:

```text
Never call Schema::try_merge as the entire policy.
Call it as one mechanism inside a stricter schema-drift decision engine.
```

---

## S3.9 Hive partition-derived columns

### S3.9.1 Discovery

```text
s3://lake/streams/event_date=2026-05-24/site=baytown/part-000.parquet
s3://lake/streams/event_date=2026-05-24/site=rotterdam/part-001.parquet
```

```sql
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
LOCATION 's3://lake/streams/';

SELECT count(*)
FROM streams
WHERE site = 'baytown';
```

DataFusion documents that Hive-compliant partition paths have their columns and values automatically detected and incorporated into schema/data; configuration also includes `datafusion.execution.listing_table_factory_infer_partitions`, which defaults to true and causes `ListingTableFactory` to infer Hive partition columns into the table schema. ([Apache DataFusion][1]) ([Apache DataFusion][6])

### S3.9.2 Partition drift risks

```text
layout drift:
  event_date=2026-05-24/site=baytown/
  dt=2026-05-25/site=baytown/

depth drift:
  event_date=.../site=.../
  event_date=.../

case drift:
  Site=baytown
  site=baytown

type drift:
  year=2026
  year=unknown

conflict with file column:
  path partition `site`
  Parquet file also contains `site`
  values may disagree

high-cardinality partition explosion:
  session_id=<uuid>/
```

### S3.9.3 Explicit `PARTITIONED BY`

```sql
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
PARTITIONED BY (event_date, site)
LOCATION 's3://lake/streams/';
```

Use explicit partition declaration when partition pruning and table schema semantics are important. DataFusion’s DDL page shows `PARTITIONED BY` for Hive-style partitioned data and notes that partition columns can be used for pruning. ([Apache DataFusion][1])

### S3.9.4 Partition policy

```text
Partition columns:
  must be top-level scalar fields
  should be low/moderate cardinality
  should be stable across table lifetime
  should not conflict with file payload columns
  should be part of schema fingerprint
  should have explicit parse/type policy
```

---

## S3.10 Explicit override policy

### S3.10.1 When to supply explicit schema

```text
Supply explicit schema when:
  source is CSV or JSON
  table is used by production jobs
  table is exposed to users/API
  field nullability matters
  financial/decimal precision matters
  timestamp unit/timezone matters
  source sampling is incomplete/untrusted
  multi-file directories are externally produced
  source has dirty/sentinel values
  downstream sink schema must be stable
```

SQL:

```sql
CREATE EXTERNAL TABLE raw_prices (
    node_id        VARCHAR NOT NULL,
    product_code   VARCHAR NOT NULL,
    price_usd_bbl  DOUBLE,
    effective_ts   TIMESTAMP
)
STORED AS CSV
LOCATION 's3://lake/raw/prices/'
OPTIONS ('has_header' 'true');
```

Rust read with explicit options is version/API-specific, so treat the following as a pattern and verify exact `CsvReadOptions` methods under the pinned DataFusion version:

```rust
let df = ctx
    .read_csv("s3://lake/raw/prices/", CsvReadOptions::new().has_header(true))
    .await?;
```

### S3.10.2 When to trust Parquet footer

```text
Trust Parquet footer when:
  single producer or governed writer
  append protocol controls schema evolution
  directory compatibility tests run
  partition layout is known
  read-after-write tests exist
  output schema manifest is versioned
```

### S3.10.3 When to disable or avoid inference

```text
Disable/avoid inference when:
  startup latency matters
  remote listing/metadata scan is expensive
  inference scans data unnecessarily
  CSV/JSON samples are non-representative
  schema is contract-owned by application
  provider schema is already known
```

DataFusion DDL docs state that external-table creation may read files to gather statistics by default and that `datafusion.execution.collect_statistics = false` can be set before table creation to avoid that work; this is a statistics-specific knob, not a universal “disable schema inference” switch. ([Apache DataFusion][1])

### S3.10.4 When to use all-`Utf8` staging

```text
Use all-Utf8 staging when:
  dirty CSV/JSON data should not abort scan
  row-level rejection audit is required
  numeric/date formats are inconsistent
  upstream sends sentinel strings
  type coercion must be transparent
  ingestion quality metrics matter
```

Staging DDL:

```sql
CREATE EXTERNAL TABLE assay_stage (
    crude_id_raw       VARCHAR,
    api_gravity_raw    VARCHAR,
    sulfur_raw         VARCHAR,
    nitrogen_raw       VARCHAR,
    pour_point_raw     VARCHAR,
    sample_date_raw    VARCHAR
)
STORED AS CSV
LOCATION 's3://lake/raw/assays/'
OPTIONS ('has_header' 'true');
```

Typed view:

```sql
CREATE VIEW assay_typed AS
SELECT
    crude_id_raw AS crude_id,
    arrow_try_cast(api_gravity_raw, 'Float64') AS api_gravity,
    arrow_try_cast(sulfur_raw, 'Float64') AS sulfur_wt_pct,
    arrow_try_cast(nitrogen_raw, 'Float64') AS nitrogen_wt_pct,
    arrow_try_cast(pour_point_raw, 'Float64') AS pour_point_c,
    arrow_try_cast(sample_date_raw, 'Date32') AS sample_date
FROM assay_stage;
```

Reject view:

```sql
CREATE VIEW assay_rejected AS
SELECT *
FROM assay_stage
WHERE (api_gravity_raw IS NOT NULL AND arrow_try_cast(api_gravity_raw, 'Float64') IS NULL)
   OR (sulfur_raw IS NOT NULL AND arrow_try_cast(sulfur_raw, 'Float64') IS NULL)
   OR (sample_date_raw IS NOT NULL AND arrow_try_cast(sample_date_raw, 'Date32') IS NULL);
```

---

## S3.11 Failure taxonomy

### S3.11.1 Inconsistent file schema

```json
{
  "error_class": "inconsistent_file_schema",
  "phase": "source_schema_discovery",
  "table": "streams",
  "file_a": "part-000.parquet",
  "file_b": "part-001.parquet",
  "field_path": "mass_flow_kg_h",
  "expected_type": "Float64",
  "actual_type": "Utf8",
  "decision": "reject",
  "suggested_fix": "rewrite offending file with Float64 or stage directory as raw text under a new table"
}
```

### S3.11.2 Incompatible partition schema

```json
{
  "error_class": "incompatible_partition_schema",
  "phase": "partition_discovery",
  "table": "streams",
  "path": "event_date=unknown/site=baytown/",
  "partition_field": "event_date",
  "expected_type": "Date32",
  "actual_value": "unknown",
  "decision": "reject",
  "suggested_fix": "move invalid partition under quarantine prefix or define event_date_raw as Utf8"
}
```

### S3.11.3 Unsupported logical type

```json
{
  "error_class": "unsupported_logical_type",
  "phase": "file_schema_mapping",
  "file": "assays.avro",
  "field_path": "sample_uuid",
  "source_logical_type": "uuid",
  "decision": "normalize",
  "suggested_fix": "map to Utf8 with semantic.type=uuid and validate in application layer"
}
```

### S3.11.4 Inferred wrong numeric type

```json
{
  "error_class": "inferred_wrong_numeric_type",
  "phase": "csv_inference",
  "table": "raw_prices",
  "field_path": "price",
  "inferred_type": "Int64",
  "contract_type": "Float64",
  "sample_rows": 1000,
  "decision": "use_explicit_schema",
  "suggested_fix": "declare price DOUBLE or ingest price_raw VARCHAR + arrow_try_cast"
}
```

### S3.11.5 Nullable / non-nullable mismatch

```json
{
  "error_class": "nullability_mismatch",
  "phase": "schema_merge",
  "field_path": "stream_id",
  "expected_nullable": false,
  "actual_nullable": true,
  "decision": "reject",
  "suggested_fix": "filter/reject null stream_id rows before publishing curated table"
}
```

---

## S3.12 Agent test suite

### S3.12.1 `DESCRIBE`

```sql
DESCRIBE streams;
DESC streams;
```

Use for table-level field names, data types, and nullability after registration.

Golden output strategy:

```text
snapshot:
  column_name
  data_type
  is_nullable
  ordinal_position if available through information_schema

avoid:
  relying only on pretty table formatting
```

### S3.12.2 `arrow_typeof`

```sql
SELECT
  arrow_typeof(stream_id) AS stream_id_type,
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM streams
LIMIT 1;
```

Use for expression-level type verification, especially after casts, nested extraction, JSON normalization, string mapping, decimals, timestamps, and dirty-data parsing.

### S3.12.3 `df.schema()`

```rust
use datafusion::prelude::*;

let df = ctx
    .read_parquet("s3://lake/refinery/streams/", ParquetReadOptions::default())
    .await?;

let schema = df.schema();

for field in schema.fields() {
    println!("{} {:?} nullable={}", field.name(), field.data_type(), field.is_nullable());
}
```

`DataFrame::schema()` returns the `DFSchema` describing the DataFrame output, including each column’s name, data type, and nullability; this makes it a planning-time schema audit hook. ([Docs.rs][7])

### S3.12.4 First-batch schema vs full dataset schema

First batch is insufficient:

```text
first batch:
  proves only first emitted RecordBatch schema

full dataset schema:
  must be derived from provider/table contract and all compatible files
```

Runtime batch test:

```rust
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;

fn assert_all_batches_same_schema(batches: &[RecordBatch]) {
    if let Some(first) = batches.first() {
        let expected: SchemaRef = first.schema();
        for (idx, batch) in batches.iter().enumerate() {
            assert_eq!(
                expected.fields(),
                batch.schema().fields(),
                "schema mismatch at batch {idx}"
            );
        }
    }
}
```

Arrow `RecordBatch::try_new` validates field count, column count, array data types, and equal column lengths; `new_unchecked` bypasses these checks and can lead to undefined behavior if schema/data do not match. ([Docs.rs][8])

### S3.12.5 Negative drift tests

Directory fixture:

```text
tests/fixtures/schema_drift/
  ok/
    part-000.parquet    # id Int64, amount Float64
    part-001.parquet    # id Int64, amount Float64
  type_conflict/
    part-000.parquet    # id Int64, amount Float64
    part-001.parquet    # id Int64, amount Utf8
  missing_required/
    part-000.parquet    # id Int64, amount Float64
    part-001.parquet    # amount Float64
  partition_conflict/
    year=2026/part-000.parquet
    year=unknown/part-001.parquet
```

Test names:

```text
rejects_type_conflict_across_parquet_files
rejects_missing_required_column
warns_extra_nullable_column_if_policy_allows
rejects_partition_column_type_drift
rejects_partition_file_column_conflict
does_not_treat_first_batch_schema_as_full_contract
csv_all_utf8_staging_accepts_dirty_rows
csv_typed_schema_rejects_bad_numeric
json_inference_does_not_create_stable_contract
```

---

## S3.13 Schema discovery audit artifact

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct DatasetSchemaAudit {
    pub dataset_uri: String,
    pub format: FileFormatKind,
    pub discovery_mode: DiscoveryMode,
    pub explicit_schema_provided: bool,
    pub discovered_schema_fingerprint: String,
    pub contract_schema_fingerprint: Option<String>,
    pub partition_columns: Vec<PartitionColumnAudit>,
    pub files_scanned_for_schema: Vec<FileSchemaAudit>,
    pub drift_findings: Vec<SchemaDriftFinding>,
    pub decision: SchemaAuditDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum FileFormatKind {
    Csv,
    Json,
    Parquet,
    Avro,
    ArrowIpc,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DiscoveryMode {
    InferredBySampling,
    EmbeddedFileMetadata,
    ExplicitOverride,
    ProviderSnapshot,
    ContractFirst,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PartitionColumnAudit {
    pub name: String,
    pub observed_values_sample: Vec<String>,
    pub inferred_type: String,
    pub conflicts_with_file_column: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FileSchemaAudit {
    pub path: String,
    pub schema_fingerprint: String,
    pub row_count_estimate: Option<u64>,
    pub schema_status: FileSchemaStatus,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum FileSchemaStatus {
    Compatible,
    CompatibleWithWarning,
    Incompatible,
    Skipped,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SchemaAuditDecision {
    Accept,
    AcceptWithWarnings,
    Reject,
    Quarantine,
    RequiresExplicitSchema,
    RequiresMigration,
}
```

---

## S3.14 Schema compatibility engine

### S3.14.1 Diff model

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDriftFinding {
    pub path: String,
    pub kind: SchemaDriftKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub severity: Severity,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum SchemaDriftKind {
    MissingField,
    ExtraField,
    TypeConflict,
    NullabilityWidening,
    NullabilityTightening,
    MetadataConflict,
    FieldOrderConflict,
    NestedFieldConflict,
    PartitionColumnConflict,
    UnsupportedLogicalType,
    InferenceUnstable,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Fatal,
}
```

### S3.14.2 Policy model

```rust
#[derive(Debug, Clone)]
pub struct SchemaDriftPolicy {
    pub allow_extra_nullable_fields: bool,
    pub allow_nullability_widening: bool,
    pub allow_lossless_numeric_widening: bool,
    pub allow_metadata_only_drift: bool,
    pub require_field_order_match: bool,
    pub require_partition_schema_match: bool,
    pub reject_unknown_logical_types: bool,
    pub require_explicit_schema_for_csv: bool,
    pub require_explicit_schema_for_json: bool,
}
```

Recommended defaults:

```rust
impl Default for SchemaDriftPolicy {
    fn default() -> Self {
        Self {
            allow_extra_nullable_fields: false,
            allow_nullability_widening: true,
            allow_lossless_numeric_widening: false,
            allow_metadata_only_drift: true,
            require_field_order_match: true,
            require_partition_schema_match: true,
            reject_unknown_logical_types: true,
            require_explicit_schema_for_csv: true,
            require_explicit_schema_for_json: true,
        }
    }
}
```

### S3.14.3 Compatibility decision

```text
Exact:
  same field count, order, names, types, nullability, contract metadata

Compatible:
  exact except allowed metadata drift or nullable widening

Migratable:
  requires explicit cast/add/default/backfill but not impossible

Rejected:
  missing required field
  unsupported type
  lossy narrowing
  partition conflict
  ambiguous field conflict
```

---

## S3.15 Multi-file preflight pattern

### S3.15.1 File listing rule

```text
Do not depend on wildcard expansion.
Register/query directory roots.
Explicitly enumerate files for audit if strict compatibility is required.
```

DataFusion CLI docs state wildcards are not supported and explain that wildcard behavior is not universal across backends such as S3 and GCS; DataFusion expects directory paths and automatically reads compatible files in that directory. ([Apache DataFusion][4])

### S3.15.2 Preflight phases

```text
1. list candidate files
2. filter by extension / format / hidden file policy
3. group by partition path
4. read lightweight schema metadata where possible
5. compute per-file schema fingerprint
6. compare against contract schema
7. compare partition schema
8. produce audit report
9. accept / warn / reject / quarantine
10. register DataFusion table only after accepted
```

### S3.15.3 Rust skeleton

```rust
use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::error::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct DatasetPreflightConfig {
    pub uri: String,
    pub format: FileFormatKind,
    pub expected_schema: Option<SchemaRef>,
    pub drift_policy: SchemaDriftPolicy,
    pub allow_empty_dataset: bool,
}

pub async fn preflight_dataset_schema(
    config: DatasetPreflightConfig,
) -> Result<DatasetSchemaAudit> {
    // Pseudocode:
    // 1. list files through object_store or application registry
    // 2. read schema metadata per file if format supports it
    // 3. infer or normalize schema
    // 4. compare to expected_schema
    // 5. return audit decision

    Err(DataFusionError::NotImplemented(
        "preflight schema scanner is platform-specific".to_string(),
    ))
}
```

Implementation note:

```text
For Parquet/Arrow/Avro:
  prefer metadata-only schema read.

For CSV/JSON:
  either require explicit schema or perform bounded inference sample with audit flag:
    inference_mode = sampling
    sample_rows = N
    stable_contract = false
```

---

## S3.16 Explicit schema registry pattern

```rust
use std::sync::Arc;
use std::collections::HashMap;
use datafusion::arrow::datatypes::SchemaRef;

#[derive(Debug, Clone)]
pub struct DatasetSchemaRegistry {
    contracts: HashMap<String, DatasetSchemaContract>,
}

#[derive(Debug, Clone)]
pub struct DatasetSchemaContract {
    pub dataset_uri_prefix: String,
    pub table_name: String,
    pub format: FileFormatKind,
    pub schema: SchemaRef,
    pub partition_columns: Vec<String>,
    pub fingerprint: String,
    pub drift_policy: SchemaDriftPolicy,
}

impl DatasetSchemaRegistry {
    pub fn get_by_uri(&self, uri: &str) -> Option<&DatasetSchemaContract> {
        self.contracts
            .values()
            .find(|c| uri.starts_with(&c.dataset_uri_prefix))
    }
}
```

Use contract before registration:

```rust
pub async fn register_dataset_with_contract(
    ctx: &SessionContext,
    registry: &DatasetSchemaRegistry,
    uri: &str,
    table_name: &str,
) -> datafusion::error::Result<()> {
    let contract = registry
        .get_by_uri(uri)
        .ok_or_else(|| datafusion::error::DataFusionError::Execution(
            format!("no schema contract for dataset uri: {uri}")
        ))?;

    // Run preflight/diff here before ctx.register_* or CREATE EXTERNAL TABLE.
    // Then register using explicit schema path or a provider/listing table configured with schema.

    Ok(())
}
```

---

## S3.17 `ListingTable` and provided schema posture

DataFusion’s custom table-provider guide describes `ListingTable` as the file-based data source behind built-in Parquet, CSV, and JSON support, with file pruning, projection pushdown, filter pushdown, and schema inference. This makes `ListingTable` the correct abstraction for advanced file datasets before implementing a fully custom `TableProvider`. ([Apache DataFusion][9])

Use `ListingTable` when:

```text
directory dataset
object-store prefix
multi-file source
custom listing options
explicit/provided schema
Hive partition columns
advanced file pruning
format-specific read options
schema inference should be controlled
```

Provider escalation rule:

```text
Use built-in read_* for simple files.
Use register_* for named simple sources.
Use CREATE EXTERNAL TABLE for SQL-admin managed file sources.
Use ListingTable for complex file datasets.
Use custom TableProvider only when the source is not file-like or needs custom semantics.
```

---

## S3.18 Inference + drift deployment modes

### Mode A — exploratory

```text
Goal:
  fast schema discovery

Allowed:
  CSV/JSON inference
  direct path SQL
  DESCRIBE snapshot

Forbidden:
  treating result as contract
```

### Mode B — verified schema-bearing source

```text
Goal:
  production Parquet/Arrow/Avro table

Allowed:
  footer/embedded schema inference
  Hive partition discovery

Required:
  directory compatibility preflight
  DESCRIBE / df.schema snapshot
  schema fingerprint
  drift policy
```

### Mode C — explicit contract

```text
Goal:
  production CSV/JSON/API source

Required:
  explicit schema
  normalization
  row-level parse/reject audit
  no sampling-derived contract
```

### Mode D — all-`Utf8` staging

```text
Goal:
  never lose dirty rows

Required:
  all raw fields as VARCHAR
  typed view with arrow_try_cast
  rejected view
  metrics by reject_reason
```

### Mode E — remote provider snapshot

```text
Goal:
  stable schema for dynamic metadata source

Required:
  snapshot schema at registration/query-start
  fingerprint
  cache TTL/invalidation
  schema diff on refresh
```

---

## S3.19 Agent implementation: schema audit CLI

```text
schema-audit inspect \
  --uri s3://lake/refinery/streams/ \
  --format parquet \
  --contract contracts/streams.schema.json \
  --infer-partitions true \
  --policy strict

schema-audit infer \
  --uri data/raw/assays.csv \
  --format csv \
  --sample-rows 1000 \
  --emit contracts/assays.inferred.schema.json \
  --unstable

schema-audit validate \
  --table prod.analytics.streams \
  --query "SELECT * FROM prod.analytics.streams LIMIT 0"
```

Output:

```json
{
  "dataset_uri": "s3://lake/refinery/streams/",
  "format": "parquet",
  "discovery_mode": "EmbeddedFileMetadata",
  "explicit_schema_provided": false,
  "discovered_schema_fingerprint": "sha256:...",
  "contract_schema_fingerprint": "sha256:...",
  "partition_columns": [
    {"name": "event_date", "inferred_type": "Date32", "conflicts_with_file_column": false}
  ],
  "files_scanned_for_schema": [
    {"path": "event_date=2026-05-24/part-000.parquet", "schema_status": "Compatible"}
  ],
  "drift_findings": [],
  "decision": "Accept"
}
```

---

## S3.20 Best-practice advisory

```text
File-format posture:
  Parquet:
    infer from footer
    validate directory compatibility
    fingerprint schema
    use partition audit

  Arrow IPC:
    infer embedded Arrow schema
    test metadata and extension compatibility

  Avro:
    infer embedded schema
    test logical type and union/nullability mapping

  CSV:
    explicit schema or all-Utf8 staging
    do not treat sample inference as contract

  JSON:
    explicit schema, staged payload, or normalized views
    test nested/missing/null behavior
```

```text
Directory posture:
  use directory roots, not wildcards
  reject incompatible files before table registration
  validate partition layout
  require compatible schemas under one table prefix
  snapshot file schema fingerprints
```

```text
Testing posture:
  DESCRIBE for table contract
  arrow_typeof for expression-level types
  df.schema() for planning-level schema
  RecordBatch validation for runtime data
  negative fixtures for every drift class
```

```text
Governance posture:
  source schema ≠ contract schema
  inference result ≠ stable API
  schema fingerprint all registered datasets
  persist audit artifact beside curated outputs
  schema drift requires explicit accept/reject/migrate decision
```

---

## S3.21 Anti-pattern inventory

```text
Schema inference anti-patterns:
  using CSV inference as production schema
  using JSON inference as public API schema
  assuming first batch proves full dataset schema
  relying on wildcard file paths
  registering object-store prefix before schema audit
  mixing producers in one Parquet directory without schema contract
  allowing partition columns to conflict with file columns
  silently accepting type conflict through broad Utf8 coercion
  tightening nullability based on a sample
  widening decimal scale without migration decision
  ignoring timestamp unit/timezone drift
  treating metadata-only drift as always harmless
  using Schema::try_merge without policy wrapper
  allowing DDL-created inferred tables in multi-tenant user SQL
  assuming Hive partition discovery is always desired
  forgetting `listing_table_factory_infer_partitions` is configurable
  omitting rejected-row view for dirty all-text staging
  failing to snapshot DESCRIBE / arrow_typeof outputs across upgrades
```

---

## S3.22 Agent checklist

```text
[ ] Identify format:
    CSV / JSON / Parquet / Avro / Arrow IPC / directory / Hive partitioned / remote provider.

[ ] Identify discovery mode:
    sampling / embedded metadata / explicit schema / provider snapshot / contract-first.

[ ] Decide trust level:
    exploratory / verified / governed / rejected.

[ ] For CSV:
    explicit schema or all-Utf8 staging.
    configure delimiter/header/null/date/timestamp options.
    add typed accepted view and rejected view.

[ ] For JSON:
    avoid inferred public contract.
    normalize hot fields.
    test missing/null/nested drift.

[ ] For Parquet:
    trust footer only after directory compatibility audit.
    test timestamp/decimal/nested/partition drift.
    fingerprint schema.

[ ] For Avro:
    test logical types, unions, defaults, nullability.

[ ] For Arrow IPC:
    test extension metadata, dictionary/string/timestamp compatibility.

[ ] For directories:
    avoid wildcards.
    validate compatible schemas across files.
    audit file-level fingerprints.

[ ] For Hive partitions:
    validate partition key set, depth, type, and file-column conflicts.
    decide infer_partitions setting deliberately.

[ ] Apply drift policy:
    missing required field => reject.
    type conflict => reject or stage.
    nullable widening => warn/accept if policy allows.
    extra field => accept only under evolution policy.
    partition conflict => reject.

[ ] Test:
    DESCRIBE.
    arrow_typeof.
    df.schema().
    RecordBatch schemas.
    negative drift fixtures.

[ ] Persist:
    schema fingerprint.
    audit artifact.
    accepted/rejected decision.
    migration recommendations.
```

## S3.23 Minimal canonical schema-audit harness

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::error::{DataFusionError, Result};
use datafusion::prelude::*;

#[derive(Debug, Clone)]
pub enum ExpectedSchemaMode {
    Exact(SchemaRef),
    AllowInferredButSnapshot,
    RequireExplicit,
}

pub async fn audit_dataframe_schema(
    df: DataFrame,
    expected: ExpectedSchemaMode,
) -> Result<Vec<RecordBatch>> {
    let df_schema = df.schema().clone();
    let arrow_schema = Arc::new(df_schema.as_arrow().clone());

    match expected {
        ExpectedSchemaMode::Exact(expected_schema) => {
            if expected_schema.fields() != arrow_schema.fields() {
                return Err(DataFusionError::Execution(format!(
                    "DataFrame schema mismatch: expected={:?}, actual={:?}",
                    expected_schema.fields(),
                    arrow_schema.fields()
                )));
            }
        }
        ExpectedSchemaMode::AllowInferredButSnapshot => {
            eprintln!("INFERRED_SCHEMA_SNAPSHOT={arrow_schema:#?}");
        }
        ExpectedSchemaMode::RequireExplicit => {
            return Err(DataFusionError::Execution(
                "explicit schema required; inferred schema is not allowed".to_string(),
            ));
        }
    }

    let batches = df.collect().await?;

    if let Some(first) = batches.first() {
        let runtime_schema = first.schema();

        for (idx, batch) in batches.iter().enumerate() {
            if batch.schema().fields() != runtime_schema.fields() {
                return Err(DataFusionError::Execution(format!(
                    "runtime RecordBatch schema drift at batch {idx}: expected={:?}, actual={:?}",
                    runtime_schema.fields(),
                    batch.schema().fields()
                )));
            }
        }
    }

    Ok(batches)
}

pub async fn example_csv_stage_policy(ctx: &SessionContext) -> Result<()> {
    let df = ctx
        .read_csv("data/raw/streams.csv", CsvReadOptions::new().has_header(true))
        .await?;

    // CSV inference is allowed only as snapshot/discovery here, not contract.
    let _ = audit_dataframe_schema(df, ExpectedSchemaMode::AllowInferredButSnapshot).await?;

    Ok(())
}

pub async fn example_parquet_contract_policy(
    ctx: &SessionContext,
    expected_schema: SchemaRef,
) -> Result<()> {
    let df = ctx
        .read_parquet("s3://lake/refinery/streams/", ParquetReadOptions::default())
        .await?;

    let _ = audit_dataframe_schema(df, ExpectedSchemaMode::Exact(expected_schema)).await?;

    Ok(())
}
```

The practical rule for agents: **infer for discovery, declare for contracts, snapshot for audit, diff before registration, and reject drift before execution or write.**

[1]: https://datafusion.apache.org/user-guide/sql/ddl.html "DDL — Apache DataFusion  documentation"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
[3]: https://datafusion.apache.org/user-guide/sql/format_options.html "Format Options — Apache DataFusion  documentation"
[4]: https://datafusion.apache.org/_sources/user-guide/cli/datasources.md.txt "datafusion.apache.org"
[5]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[6]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[7]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html "DataFrame in datafusion::dataframe - Rust"
[8]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html "RecordBatch in arrow::record_batch - Rust"
[9]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html "Custom Table Provider — Apache DataFusion  documentation"


# DataFusion Advanced — S4) Naming, identifier normalization, qualifiers, and output field names

## S4.0 Objective

Define a deterministic naming policy across:

```text
external source names
  → Arrow Schema field names
  → catalog.schema.table identifiers
  → SQL identifiers
  → DataFrame / Expr column references
  → DFSchema qualifiers
  → LogicalPlan output fields
  → ExecutionPlan / RecordBatch fields
  → sink/API/output field names
```

The attached document already establishes the key ingredients: Arrow `Schema` vs `DFSchema`, qualifier-aware planning, `DFSchema::try_from_qualified_schema`, `DFSchema::as_arrow()` losing qualifier information, and the need to preserve qualifiers across multi-table plans.  This section turns those facts into a naming standard for LLM agents.

---

## S4.1 Naming problem statement

Naming is a correctness surface, not only a style surface.

```text
Failure classes:
  wrong column resolved
  ambiguous column after join
  output schema drift
  generated SQL fails due to casing
  RecordBatch field names differ from API contract
  view / CTAS emits unstable expression-derived names
  qualifiers stripped before binding
  duplicate output fields break downstream serializers
  tenant/catalog/schema names collide
```

DataFusion SQL lowercases unquoted query identifiers while inferred schema names are not automatically lowercased, so capitalized physical fields require double quotes in SQL. The official example also shows that DataFrame `col("\"A\"")` parses a quoted identifier, while `ident("A")` passes an unqualified column name without parsing. ([Apache DataFusion][1])

---

## S4.2 Global naming policy

### S4.2.1 Canonical physical names

```text
canonical column name:
  lowercase snake_case
  ASCII preferred for platform objects
  no spaces
  no punctuation except underscore
  no leading digit
  no reserved SQL keyword unless quoted intentionally
  no case-only distinctions
  stable across Arrow / SQL / DataFrame / JSON / Parquet / API
```

Recommended:

```text
stream_id
case_id
unit_id
mass_flow_kg_h
sulfur_wt_pct
event_ts
created_at
source_file_path
```

Avoid in stable contracts:

```text
Stream ID
Mass Flow kg/h
mass-flow
mass.flow
MassFlow
SELECT
1st_column
Δpressure
source/file
```

### S4.2.2 External-to-canonical normalization

```text
source field              canonical field
"Stream ID"               stream_id
"Mass Flow kg/h"          mass_flow_kg_h
"API Gravity"             api_gravity
"Sulfur wt%"              sulfur_wt_pct
"Created At"              created_at
"Unit.Name"               unit_name
```

SQL normalization pattern:

```sql
WITH normalized AS (
  SELECT
    "Stream ID" AS stream_id,
    "Case ID" AS case_id,
    "Unit ID" AS unit_id,
    "Mass Flow kg/h" AS mass_flow_kg_h,
    "Sulfur wt%" AS sulfur_wt_pct
  FROM raw_streams
)
SELECT *
FROM normalized;
```

Rust normalization sketch:

```rust
pub fn canonical_name(raw: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;

    for c in raw.trim().chars() {
        let mapped = if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
        } else {
            '_'
        };

        if mapped == '_' {
            if !prev_underscore && !out.is_empty() {
                out.push('_');
                prev_underscore = true;
            }
        } else {
            out.push(mapped);
            prev_underscore = false;
        }
    }

    let out = out.trim_matches('_').to_string();

    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("c_{out}")
    } else {
        out
    }
}
```

Deployment rule:

```text
Normalize source names once at ingestion / view / provider boundary.
Do not repeatedly canonicalize names inside downstream query generation.
Store original source name in metadata if traceability matters.
```

---

## S4.3 SQL identifier casing and quoting

### S4.3.1 DataFusion SQL casing rule

Unquoted SQL identifiers are effectively lowercased, while inferred schemas retain their original field names. Thus, a CSV or Parquet field named `Name` must be referenced as `"Name"`, not `Name`. ([Apache DataFusion][1])

```sql
-- wrong if physical field is "Name"
SELECT Name
FROM people;

-- correct
SELECT "Name"
FROM people;

-- preferred stable output
SELECT "Name" AS name
FROM people;
```

### S4.3.2 Quoting policy

```text
Quote only when:
  source schema has uppercase / spaces / punctuation
  preserving external compatibility field spelling
  referencing legacy fields before normalization
  calling case-sensitive UDF/function names
  generating one-off migration SQL

Avoid quoting when:
  defining new stable platform schema
  writing long-lived views
  exposing API result fields
  creating canonical Parquet/Arrow datasets
```

### S4.3.3 Immediate aliasing rule

Bad:

```sql
SELECT
  "Mass Flow kg/h",
  "Sulfur wt%"
FROM raw_streams;
```

Good:

```sql
SELECT
  "Mass Flow kg/h" AS mass_flow_kg_h,
  "Sulfur wt%" AS sulfur_wt_pct
FROM raw_streams;
```

Hard rule:

```text
Any quoted source identifier in a stable projection must be aliased immediately.
```

---

## S4.4 Catalog, schema, table, and tenant naming

### S4.4.1 Naming hierarchy

```text
catalog.schema.table.column
```

Recommended pattern:

```text
tenant_acme.raw.streams_stage.mass_flow_raw
tenant_acme.curated.streams.mass_flow_kg_h
tenant_acme.semantic.stream_balances.total_mass_flow_kg_h
```

### S4.4.2 Catalog-level policy

```text
catalog:
  tenant boundary
  deployment boundary
  environment boundary
  data-product boundary

examples:
  prod
  dev
  tenant_acme
  tenant_global
  sandbox_paul
```

### S4.4.3 Schema-level policy

```text
schema:
  lifecycle layer
  domain namespace
  authorization scope

examples:
  raw
  staging
  curated
  semantic
  diagnostics
  temp
  information_schema
```

### S4.4.4 Table-level policy

```text
table:
  plural nouns for entity sets
  singular if table models a single named object type by convention
  suffixes for lifecycle state

examples:
  streams
  units
  assays
  crude_slates
  blend_recipes
  streams_stage
  streams_rejected
  streams_v1
```

### S4.4.5 Multi-tenant qualification

```sql
SELECT
  stream_id,
  mass_flow_kg_h
FROM tenant_acme.curated.streams;
```

Service-policy rule:

```text
Public / multi-tenant service:
  never rely on unqualified table names
  bind tenant to catalog/schema server-side
  reject user-provided catalog/schema override unless explicitly authorized
```

---

## S4.5 DFSchema qualifier policy

`DFSchema` wraps an Arrow schema and adds optional relation/table qualification. Qualified and unqualified fields can coexist, and unqualified fields must be unique in a way that still permits unambiguous lookup. `DFSchema::try_from_qualified_schema("t1", &arrow_schema)` creates a qualified schema, and unqualified access is allowed only when unambiguous. `DFSchema::as_arrow()` and `inner()` return the inner Arrow schema and do not include qualifier information. ([Docs.rs][2])

### S4.5.1 Qualifier examples

```text
t.c1
orders.customer_id
curated.orders.customer_id
prod.curated.orders.customer_id
o.customer_id          -- table alias qualifier
```

### S4.5.2 Qualified schema construction

```rust
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::{Column, DFSchema};

let arrow_schema = Schema::new(vec![
    Field::new("id", DataType::Int64, false),
    Field::new("customer_id", DataType::Int64, false),
]);

let orders = DFSchema::try_from_qualified_schema("orders", &arrow_schema)?;

assert!(orders.has_column(&Column::from_qualified_name("orders.id")));
assert!(orders.has_column(&Column::from_qualified_name("id"))); // valid only if unambiguous
```

### S4.5.3 Qualifier-loss boundary

```rust
let arrow_schema = orders.as_arrow();
// qualifier information is not present here
```

Hard invariant:

```text
DFSchema qualifier = logical planning metadata.
Arrow Schema field name = physical/output name.
Converting DFSchema → Arrow Schema strips qualifier semantics.
Therefore alias/rename before crossing output boundary.
```

---

## S4.6 `Column` parsing and case behavior

`Column::from_qualified_name("foo.BAR")` treats the string as a SQL identifier, so it parses to relation `foo` and lower-case column name `bar`; quoted `"foo.BAR"` parses as one column name `foo.BAR`. `Column::from_qualified_name_ignore_case` preserves column text case behind the `sql` feature. ([Docs.rs][3])

### S4.6.1 DataFusion `Column` constructors

```rust
use datafusion::common::Column;

let c1 = Column::new_unqualified("stream_id");
let c2 = Column::from_name("stream_id");
let c3 = Column::from_qualified_name("streams.stream_id");
let c4 = Column::new(Some("streams"), "stream_id");
```

### S4.6.2 Quoted / parsed semantics

```rust
use datafusion::common::Column;

// Parses SQL identifier; BAR becomes bar.
let c = Column::from_qualified_name("foo.BAR");

// Quoted flat name is one column identifier named foo.BAR.
let c_quoted = Column::from_qualified_name("\"foo.BAR\"");
```

Agent rule:

```text
Use Column::new(...) / Column::new_unqualified(...) when names are already canonical.
Use Column::from_qualified_name(...) when parsing SQL-like references is intended.
Do not feed arbitrary external names into from_qualified_name without quoting/canonicalization.
```

---

## S4.7 Table alias vs table name

### S4.7.1 SQL aliasing

```sql
SELECT
  o.order_id,
  c.customer_id
FROM curated.orders AS o
JOIN curated.customers AS c
  ON o.customer_id = c.customer_id;
```

Policy:

```text
Inside a query:
  prefer short aliases for readability
  qualify all columns after first join
  aliases shadow long table names for column references

In output:
  never emit alias-qualified names as final fields
  project explicit field aliases
```

### S4.7.2 Good join projection

```sql
SELECT
  o.order_id AS order_id,
  o.customer_id AS order_customer_id,
  c.customer_id AS customer_dim_id,
  c.name AS customer_name
FROM curated.orders AS o
JOIN curated.customers AS c
  ON o.customer_id = c.customer_id;
```

### S4.7.3 Bad join projection

```sql
SELECT *
FROM curated.orders AS o
JOIN curated.customers AS c
  ON o.customer_id = c.customer_id;
```

Failure modes:

```text
duplicate id
duplicate customer_id
ambiguous downstream col("id")
unstable output schema when source table evolves
qualifier lost after Arrow output
```

---

## S4.8 DataFrame / Expr naming

### S4.8.1 `col("x")`

```rust
use datafusion::prelude::*;

let e = col("mass_flow_kg_h");
```

Use when:

```text
column is canonical lowercase snake_case
unqualified resolution is unambiguous
single-table plan or post-projection plan
```

### S4.8.2 `col("t.x")`

```rust
let e = col("streams.mass_flow_kg_h");
```

Use when:

```text
multi-table plan
join input contains duplicate field names
provider/table relation qualifier is available
DataFrame expression references need disambiguation
```

### S4.8.3 `col("\"A\"")`

```rust
let e = col("\"A\"");
```

Use when:

```text
DataFrame col() should parse a quoted SQL identifier
source field physically named A
case preservation is required
```

The official example shows `col("\"A\"")` for DataFrame filtering against a capitalized CSV field because `col` parses the input string. ([Apache DataFusion][1])

### S4.8.4 `ident("A")`

```rust
let e = ident("A");
```

Use when:

```text
the input is an unqualified column name and should not be SQL-identifier parsed/lowercased
capitalized physical field should be referenced directly
```

DataFusion’s example uses `ident("A")` as the alternative to `col("\"A\"")` for capitalized DataFrame column references. ([Apache DataFusion][1])

### S4.8.5 Aliasing expressions

```rust
let e = (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d");
```

`Expr::schema_name()` determines the field name a projection expression produces; for aliases, it shows only the alias, and for expression-derived names it uses expression text such as `foo = Int32(42)`. This is useful for debugging but too unstable for API/sink contracts. ([Docs.rs][4])

---

## S4.9 Output field names

### S4.9.1 DataFusion output-name semantics

DataFusion has an explicit output-field-name semantic specification covering how field names in output `RecordBatch`es are generated for SQL and DataFrame queries. Use aliases to bypass expression-name subtleties and produce stable field names. ([Apache DataFusion][5])

### S4.9.2 Computed expressions

Bad:

```sql
SELECT mass_flow_kg_h * 24.0
FROM streams;
```

Good:

```sql
SELECT mass_flow_kg_h * 24.0 AS mass_flow_kg_d
FROM streams;
```

Rust:

```rust
let df = df.select(vec![
    (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d"),
])?;
```

### S4.9.3 Aggregate aliases

Bad:

```sql
SELECT unit_id, SUM(mass_flow_kg_h)
FROM streams
GROUP BY unit_id;
```

Good:

```sql
SELECT
  unit_id,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h
FROM streams
GROUP BY unit_id;
```

Rust:

```rust
use datafusion::functions_aggregate::expr_fn::sum;

let df = df.aggregate(
    vec![col("unit_id")],
    vec![sum(col("mass_flow_kg_h")).alias("total_mass_flow_kg_h")],
)?;
```

### S4.9.4 Window aliases

Bad:

```sql
SELECT ROW_NUMBER() OVER (PARTITION BY unit_id ORDER BY mass_flow_kg_h DESC)
FROM streams;
```

Good:

```sql
SELECT
  stream_id,
  unit_id,
  ROW_NUMBER() OVER (
    PARTITION BY unit_id
    ORDER BY mass_flow_kg_h DESC, stream_id ASC
  ) AS unit_flow_rank
FROM streams;
```

### S4.9.5 CASE aliases

```sql
SELECT
  stream_id,
  CASE
    WHEN mass_flow_kg_h >= 1000 THEN 'large'
    WHEN mass_flow_kg_h >= 100 THEN 'medium'
    ELSE 'small'
  END AS flow_size_class
FROM streams;
```

### S4.9.6 Function aliases

```sql
SELECT
  stream_id,
  lower(unit_id) AS unit_id_norm,
  date_trunc('hour', event_ts) AS event_hour
FROM streams;
```

---

## S4.10 Join duplicate-column handling

### S4.10.1 Duplicate-column failure class

```sql
SELECT *
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.customer_id;
```

Potential output fields:

```text
order_id
customer_id
...
customer_id
name
...
```

Problems:

```text
same unqualified field name appears twice
Arrow output has no planner qualifier semantics
JSON object serialization can overwrite keys or reject duplicates
downstream DataFrame col("customer_id") ambiguous
view schema unstable
```

### S4.10.2 Required join projection

```sql
SELECT
  o.order_id AS order_id,
  o.customer_id AS order_customer_id,
  c.customer_id AS customer_id,
  c.name AS customer_name
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.customer_id;
```

### S4.10.3 Naming convention for duplicate keys

```text
left table natural key:
  order_id

right dimension key:
  customer_id

foreign key on fact:
  order_customer_id
  source_customer_id
  fk_customer_id

dim table duplicate:
  customer_dim_id
```

### S4.10.4 DFSchema ambiguity check

`Column::normalize_with_schemas_and_ambiguity_check` qualifies an unqualified column by searching through schemas and returns an ambiguity error when more than one matching column exists at the same search level, with a special case for `USING` join columns. ([Docs.rs][3])

```rust
fn require_unambiguous(df_schema: &datafusion::common::DFSchema, name: &str) -> datafusion::error::Result<()> {
    let matches = df_schema.columns_with_unqualified_name(name);

    if matches.len() > 1 {
        return Err(datafusion::error::DataFusionError::Plan(format!(
            "ambiguous unqualified column `{name}`; candidates={matches:?}; require qualifier or alias"
        )));
    }

    Ok(())
}
```

---

## S4.11 `SELECT *` expansion rules

### S4.11.1 Stable-contract rule

```text
SELECT * is allowed for:
  ad hoc exploration
  local debug
  schema discovery
  temporary CLI work
  throwaway test queries

SELECT * is forbidden for:
  views
  CTAS
  API responses
  Parquet/CSV/JSON exports
  semantic tables
  public SQL examples
  golden tests
```

### S4.11.2 `SELECT * EXCEPT` / `EXCLUDE`

DataFusion supports `SELECT * EXCEPT(age, person)` and `SELECT * EXCLUDE(age, person)` as wildcard projections that exclude named columns. This is useful for exploratory wide-table work, but should still be avoided for stable contracts because newly added source columns can silently enter outputs. ([Apache DataFusion][6])

```sql
SELECT * EXCEPT(internal_debug_blob, raw_payload)
FROM events;
```

### S4.11.3 Safer explicit projection

```sql
SELECT
  event_id,
  event_type,
  user_id,
  event_ts
FROM events;
```

### S4.11.4 DataFusion `Expr::Wildcard`

`Expr::Wildcard` is deprecated because wildcard expressions should be resolved to concrete expressions when constructing the logical plan; this reinforces the agent rule that stable generated plans should expand/provide explicit fields instead of carrying wildcard semantics forward. ([Docs.rs][4])

---

## S4.12 `unnest` field naming

### S4.12.1 Array/list unnest

```sql
SELECT
  stream_id,
  unnest(tags) AS tag
FROM streams;
```

Policy:

```text
array/list unnest:
  row cardinality changes
  output element must be explicitly aliased
```

### S4.12.2 Struct unnest

```sql
SELECT
  stream_id,
  unnest(assay)
FROM streams;
```

DataFusion documents `unnest(struct)` as expanding struct fields into individual columns, and each field can be accessed via a qualified form like `"<table>.<struct>.<field>"`. ([Apache DataFusion][7])

Preferred stable output:

```sql
SELECT
  stream_id,
  assay['api_gravity'] AS assay_api_gravity,
  assay['sulfur_wt_pct'] AS assay_sulfur_wt_pct,
  assay['tanf_mg_koh_g'] AS assay_tanf_mg_koh_g
FROM streams;
```

Reason:

```text
unnest(struct) output names can be implementation-/context-sensitive.
explicit field extraction + aliasing gives stable sink/API schema.
```

### S4.12.3 Struct field prefix convention

```text
struct column: assay
child: api_gravity
output: assay_api_gravity

struct column: geo
child: country
output: geo_country
```

---

## S4.13 Name uniqueness policy

### S4.13.1 Output uniqueness rule

```text
Stable output schemas must have unique field names after qualifier stripping.
```

DataFusion has an internal helper `unique_field_aliases(fields)` that returns aliases to make field names unique; the docs describe it as returning `None` to keep a field or `Some(alias)` to rename for uniqueness. Use this idea as a fallback, but prefer semantic aliases over auto-generated suffixes. ([Docs.rs][8])

### S4.13.2 Deterministic uniqueness algorithm

```rust
use std::collections::HashMap;

pub fn unique_output_names<I>(names: I) -> Vec<String>
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();

    for raw in names {
        let base = canonical_name(raw.as_ref());
        let n = counts.entry(base.clone()).or_insert(0);

        if *n == 0 {
            out.push(base.clone());
        } else {
            out.push(format!("{base}_{}", *n + 1));
        }

        *n += 1;
    }

    out
}
```

Policy:

```text
Auto-suffixing is acceptable for internal debug.
For public schemas, require semantic alias supplied by query generator.
```

---

## S4.14 Alias propagation and optimizer safety

### S4.14.1 Alias as contract

```rust
let expr = (col("a") + col("b")).alias("sum_ab");
```

In projection output, `sum_ab` is the contract. If optimizer or generated-code rewrite strips alias, output schema changes.

### S4.14.2 Anti-pattern rewrite

```rust
// bad optimizer/generator transformation
(col("a") + col("b")).alias("sum_ab").unalias()
```

### S4.14.3 Safe rewrite rule

```text
When rewriting Expr:
  preserve top-level Alias unless the rewrite explicitly changes output schema
  preserve alias metadata if metadata is part of contract
  validate Expr::schema_name() before/after rewrite
```

`Expr` supports `alias`, `alias_qualified`, metadata-bearing aliases, `unalias`, and `unalias_nested`; these are useful tools, but agents must distinguish semantic output aliases from internal normalization aliases. ([Docs.rs][4])

DataFusion 54 behavior change: `Expr::unalias_nested()` no longer removes aliases that carry non-empty `FieldMetadata` — metadata-bearing aliases survive rewrites that previously stripped them. This protects metadata contracts, but it breaks any code that assumed `unalias_nested` produces an alias-free expression (expression-equality checks, generated output-name comparisons, normalization passes). To strip every alias unconditionally, unwrap `Expr::Alias` explicitly — and only do so where discarding the attached metadata is a deliberate decision:

```rust
fn strip_all_aliases(expr: Expr) -> Expr {
    match expr {
        // Deliberately discards alias.metadata — only for contexts where the
        // output field metadata contract does not apply.
        Expr::Alias(alias) => strip_all_aliases(*alias.expr),
        other => other,
    }
}
```

---

## S4.15 Naming policy object

```rust
#[derive(Debug, Clone)]
pub struct NamingPolicy {
    pub canonical_case: CanonicalCase,
    pub allow_quoted_identifiers: bool,
    pub require_alias_for_computed_expr: bool,
    pub require_alias_for_aggregate: bool,
    pub require_alias_for_window: bool,
    pub require_unique_output_names: bool,
    pub forbid_select_star_in_contracts: bool,
    pub preserve_source_name_metadata: bool,
    pub qualifier_policy: QualifierPolicy,
}

#[derive(Debug, Clone)]
pub enum CanonicalCase {
    LowerSnake,
    Preserve,
}

#[derive(Debug, Clone)]
pub enum QualifierPolicy {
    PreserveUntilOutput,
    RequireAfterJoin,
    StripAtSink,
    PersistInMetadataAtSink,
}
```

Recommended defaults:

```rust
impl Default for NamingPolicy {
    fn default() -> Self {
        Self {
            canonical_case: CanonicalCase::LowerSnake,
            allow_quoted_identifiers: true,
            require_alias_for_computed_expr: true,
            require_alias_for_aggregate: true,
            require_alias_for_window: true,
            require_unique_output_names: true,
            forbid_select_star_in_contracts: true,
            preserve_source_name_metadata: true,
            qualifier_policy: QualifierPolicy::PreserveUntilOutput,
        }
    }
}
```

---

## S4.16 SQL generator rules

### S4.16.1 Canonical SQL projection generator

```sql
SELECT
  s.stream_id AS stream_id,
  s.case_id AS case_id,
  s.unit_id AS unit_id,
  s.mass_flow_kg_h AS mass_flow_kg_h,
  s.mass_flow_kg_h * 24.0 AS mass_flow_kg_d,
  u.unit_name AS unit_name
FROM curated.streams AS s
JOIN curated.units AS u
  ON s.unit_id = u.unit_id;
```

### S4.16.2 Rules

```text
[1] Every source table gets an alias.
[2] Every column after first join is qualified.
[3] Every computed expression is aliased.
[4] Every aggregate is aliased.
[5] Every window expression is aliased.
[6] Every quoted source field is immediately aliased to canonical form.
[7] SELECT * is forbidden in stable output.
[8] Duplicate source names are resolved by semantic aliases.
[9] Final output names are lowercase snake_case.
[10] Final output names are unique after qualifier stripping.
```

### S4.16.3 Reject examples

```sql
-- reject: unqualified after join
SELECT id
FROM orders o
JOIN customers c ON o.customer_id = c.id;

-- reject: expression-derived output name
SELECT amount * 1.08
FROM orders;

-- reject: stable view with star
CREATE VIEW v AS SELECT * FROM orders;

-- reject: duplicate aliases
SELECT
  o.id AS id,
  c.id AS id
FROM orders o
JOIN customers c ON o.customer_id = c.id;
```

---

## S4.17 DataFrame generator rules

### S4.17.1 Single-table projection

```rust
let df = df.select(vec![
    col("stream_id"),
    col("case_id"),
    col("unit_id"),
    col("mass_flow_kg_h"),
    (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d"),
])?;
```

### S4.17.2 Joined projection

```rust
let joined = streams.join(
    units,
    datafusion::logical_expr::JoinType::Inner,
    &["unit_id"],
    &["unit_id"],
    None,
)?;

let projected = joined.select(vec![
    col("streams.stream_id").alias("stream_id"),
    col("streams.unit_id").alias("stream_unit_id"),
    col("units.unit_id").alias("unit_dim_id"),
    col("units.unit_name").alias("unit_name"),
])?;
```

### S4.17.3 Capitalized external field

```rust
let df = df.select(vec![
    ident("Mass Flow kg/h").alias("mass_flow_kg_h"),
])?;
```

or:

```rust
let df = df.select(vec![
    col("\"Mass Flow kg/h\"").alias("mass_flow_kg_h"),
])?;
```

Agent rule:

```text
Use ident(...) for known raw field names that must not be SQL-parsed.
Use col("\"...\"") for SQL-quoted identifier behavior.
Immediately alias either path to canonical lower_snake_case.
```

---

## S4.18 Naming validation passes

### S4.18.1 Output schema validation

```rust
use datafusion::common::DFSchema;
use datafusion::error::{DataFusionError, Result};

pub fn validate_output_names(df_schema: &DFSchema) -> Result<()> {
    let mut seen = std::collections::HashSet::new();

    for field in df_schema.fields() {
        let name = field.name();

        if !is_lower_snake_case(name) {
            return Err(DataFusionError::Plan(format!(
                "non-canonical output field name `{name}`; expected lower_snake_case"
            )));
        }

        if !seen.insert(name.clone()) {
            return Err(DataFusionError::Plan(format!(
                "duplicate output field name `{name}`"
            )));
        }
    }

    Ok(())
}

fn is_lower_snake_case(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut prev_underscore = false;

    for (i, c) in name.chars().enumerate() {
        if c == '_' {
            if i == 0 || prev_underscore {
                return false;
            }
            prev_underscore = true;
            continue;
        }

        if !(c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return false;
        }

        if i == 0 && c.is_ascii_digit() {
            return false;
        }

        prev_underscore = false;
    }

    !name.ends_with('_')
}
```

### S4.18.2 Ambiguity validation

```rust
pub fn require_unambiguous_references(
    df_schema: &DFSchema,
    referenced_unqualified_names: &[&str],
) -> Result<()> {
    for name in referenced_unqualified_names {
        let candidates = df_schema.columns_with_unqualified_name(name);

        if candidates.len() > 1 {
            return Err(DataFusionError::Plan(format!(
                "ambiguous unqualified reference `{name}`; candidates={candidates:?}"
            )));
        }
    }

    Ok(())
}
```

### S4.18.3 Source-name preservation

```rust
use std::collections::HashMap;
use datafusion::arrow::datatypes::{DataType, Field};

pub fn canonical_field_from_source(
    source_name: &str,
    data_type: DataType,
    nullable: bool,
) -> Field {
    let canonical = canonical_name(source_name);
    let mut metadata = HashMap::new();

    if canonical != source_name {
        metadata.insert("source.name".to_string(), source_name.to_string());
    }

    Field::new(canonical, data_type, nullable).with_metadata(metadata)
}
```

---

## S4.19 Naming diagnostics payload

```json
{
  "error_class": "naming_policy_violation",
  "phase": "logical_projection_validation",
  "node_path": "Projection[3]",
  "field_name": "SUM(mass_flow_kg_h)",
  "violation": "computed_expression_without_alias",
  "expected_policy": "all computed expressions must have explicit lower_snake_case aliases",
  "suggested_fix": "SUM(mass_flow_kg_h) AS total_mass_flow_kg_h"
}
```

Ambiguous column diagnostic:

```json
{
  "error_class": "ambiguous_column_reference",
  "phase": "expression_binding",
  "identifier": "id",
  "candidate_resolutions": [
    "orders.id",
    "customers.id"
  ],
  "suggested_fix": "Use o.id AS order_id or c.id AS customer_id"
}
```

Qualifier-loss diagnostic:

```json
{
  "error_class": "qualifier_loss_at_output_boundary",
  "phase": "sink_schema_validation",
  "field_name": "id",
  "context": "DFSchema qualifiers are not preserved in Arrow Schema output",
  "suggested_fix": "Alias qualified fields before sink: o.id AS order_id, c.id AS customer_id"
}
```

---

## S4.20 Deployment advisory

### S4.20.1 ETL / lakehouse

```text
Raw zone:
  preserve source names in metadata
  canonicalize exposed field names
  allow quoted identifiers only in staging views

Curated zone:
  all fields lowercase snake_case
  no quoted identifiers in stable schemas
  no SELECT *
  schema fingerprint includes field names/order

Semantic zone:
  business-meaningful aliases
  duplicate technical IDs resolved
  views versioned if names change
```

### S4.20.2 Query service

```text
Public SQL endpoint:
  reject non-canonical output names unless ad hoc mode
  reject SELECT * in API/export mode
  require unique output names
  require qualifiers after joins
  include output schema in response metadata
  disable tenant catalog/schema override unless authorized
```

### S4.20.3 Custom provider

```text
TableProvider::schema():
  canonical field names only
  preserve source names in field metadata
  avoid duplicate names
  never expose raw backend names with spaces/case unless compatibility provider

Projection mapping:
  projection indices map to canonical schema order
  backend field names map through metadata/adapter
```

### S4.20.4 Views / CTAS / sinks

```text
CREATE VIEW:
  explicit projection
  aliases for all computed fields
  no SELECT *

CTAS:
  explicit casts
  explicit aliases
  deterministic field order

write_parquet / write_json / API:
  unique lower_snake_case field names
  no qualifier assumptions
  schema manifest / fingerprint included
```

---

## S4.21 Anti-pattern inventory

```text
Naming anti-patterns:
  unqualified id after join
  SELECT * in stable output
  SELECT * EXCEPT in stable output
  expression-derived output names
  aggregate without alias
  window expression without alias
  CASE expression without alias
  function call without alias
  duplicate aliases in projection
  relying on case-preserving unquoted SQL
  quoted source identifier not immediately aliased
  treating Arrow Schema as if it stored table/catalog qualifiers
  stripping DFSchema qualifiers before resolving joins
  using col("A") when physical field is "A" and parser lowercases
  using from_qualified_name on raw external names with punctuation
  auto-suffixing public field names instead of semantic aliasing
  allowing user-provided catalog/schema in multi-tenant service
  writing JSON objects with duplicate keys
```

---

## S4.22 Agent checklist

```text
[ ] Canonical physical names are lowercase snake_case.
[ ] External capitalized/spaced fields are quoted only at boundary.
[ ] Quoted fields are immediately aliased to canonical names.
[ ] Every source table in generated SQL has an alias.
[ ] Every column after a join is qualified.
[ ] Every computed expression has an alias.
[ ] Every aggregate expression has an alias.
[ ] Every window expression has an alias.
[ ] Every CASE expression has an alias.
[ ] Every unnest scalar output has an alias.
[ ] Struct unnest outputs are explicitly extracted/aliased for stable contracts.
[ ] SELECT * is forbidden in views, CTAS, sinks, APIs, and golden tests.
[ ] SELECT * EXCEPT/EXCLUDE is exploratory only.
[ ] Output field names are unique after qualifier stripping.
[ ] DFSchema qualifiers are preserved until output boundary.
[ ] Arrow Schema is used only for physical/output field names.
[ ] Column::from_qualified_name is used only for SQL-like parsed names.
[ ] ident(...) is used for direct unparsed unqualified physical names.
[ ] Final sink/API schema is validated for canonical names and uniqueness.
```

## S4.23 Minimal naming-policy validator

```rust
use datafusion::common::DFSchema;
use datafusion::error::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct NamingPolicy {
    pub require_lower_snake_case: bool,
    pub require_unique_output_names: bool,
    pub forbid_expression_derived_names: bool,
}

impl Default for NamingPolicy {
    fn default() -> Self {
        Self {
            require_lower_snake_case: true,
            require_unique_output_names: true,
            forbid_expression_derived_names: true,
        }
    }
}

pub fn validate_df_output_names(schema: &DFSchema, policy: &NamingPolicy) -> Result<()> {
    let mut seen = std::collections::HashSet::new();

    for field in schema.fields() {
        let name = field.name();

        if policy.require_lower_snake_case && !is_lower_snake_case(name) {
            return Err(DataFusionError::Plan(format!(
                "field `{name}` violates lower_snake_case naming policy"
            )));
        }

        if policy.require_unique_output_names && !seen.insert(name.clone()) {
            return Err(DataFusionError::Plan(format!(
                "duplicate output field name `{name}`"
            )));
        }

        if policy.forbid_expression_derived_names && looks_expression_derived(name) {
            return Err(DataFusionError::Plan(format!(
                "field `{name}` appears expression-derived; add explicit alias"
            )));
        }
    }

    Ok(())
}

fn is_lower_snake_case(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let bytes = name.as_bytes();

    if bytes[0].is_ascii_digit() || bytes[0] == b'_' || bytes[bytes.len() - 1] == b'_' {
        return false;
    }

    let mut prev_underscore = false;

    for b in bytes {
        match b {
            b'a'..=b'z' | b'0'..=b'9' => prev_underscore = false,
            b'_' if !prev_underscore => prev_underscore = true,
            _ => return false,
        }
    }

    true
}

fn looks_expression_derived(name: &str) -> bool {
    name.contains('(')
        || name.contains(')')
        || name.contains('+')
        || name.contains('-')
        || name.contains('*')
        || name.contains('/')
        || name.contains('=')
        || name.contains(' ')
}
```

Core operating rule: **qualify while planning, canonicalize at source boundaries, alias at output boundaries, and never let implicit expression names become schema contracts.**

[1]: https://datafusion.apache.org/user-guide/example-usage.html?utm_source=chatgpt.com "Example Usage — Apache DataFusion documentation"
[2]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html?utm_source=chatgpt.com "DFSchema in datafusion::common - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/common/struct.Column.html?utm_source=chatgpt.com "Column in datafusion::common - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html?utm_source=chatgpt.com "Expr in datafusion::logical_expr - Rust"
[5]: https://datafusion.apache.org/contributor-guide/specification/output-field-name-semantic.html?utm_source=chatgpt.com "Output field name semantics - Apache DataFusion"
[6]: https://datafusion.apache.org/user-guide/sql/select.html?utm_source=chatgpt.com "SELECT syntax — Apache DataFusion documentation"
[7]: https://datafusion.apache.org/user-guide/sql/special_functions.html?utm_source=chatgpt.com "Special Functions — Apache DataFusion documentation"
[8]: https://docs.rs/datafusion/latest/datafusion/logical_expr/logical_plan/builder/fn.unique_field_aliases.html?utm_source=chatgpt.com "unique_field_aliases in datafusion - logical_plan::builder"


# DataFusion Advanced — S5) Type compatibility, coercion, and schema equality

## S5.0 Objective

Move from “these types exist” to **policy-governed compatibility decisions**:

```text id="j4us3y"
Arrow DataType / Field / Schema
  → physical exactness
  → DataFusion logical equivalence
  → semantic equivalence
  → coercible compatibility
  → user-facing contract compatibility
  → explicit cast / reject / migrate decision
```

The attached documentation already covers SQL-to-Arrow type mapping, `DFSchema` versus Arrow `Schema`, schema qualifiers, `RecordBatch` validation, and a short compatibility note; S5 turns those facts into an end-to-end decision framework for LLM programming agents. 

Core invariant:

```text id="ezbbqk"
Compatibility is not one Boolean.
Compatibility is a tuple:

  (level, direction, lossiness, nullability, metadata policy, qualifier policy, operator context, enforcement point)
```

---

## S5.1 Compatibility stack

```text id="yqrjco"
CompatibilityLevel
  ├─ PhysicalExact
  │   └─ required by RecordBatch arrays, physical execution output, Arrow IPC exact contracts
  │
  ├─ LogicalDataFusion
  │   └─ planner-level equality: e.g. dictionary strings vs values, Utf8 vs Utf8View
  │
  ├─ SemanticPlatform
  │   └─ domain equality: e.g. price_usd_bbl Decimal(20,4) vs Decimal(18,4) may or may not be acceptable
  │
  ├─ Coercible
  │   └─ explicit cast/rewrite can produce compatible output
  │
  └─ UserFacingContract
      └─ field names, order, nullability, docs, units, precision, timezone, metadata, API stability
```

Arrow `Schema` is ordered metadata describing fields and schema metadata; `Schema::new` creates ordered field sequences, `Schema::new_with_metadata` attaches schema-level metadata, and Arrow schema information is metadata rather than physical memory layout. ([Docs.rs][1])

`DFSchema` exposes a deliberate compatibility API surface: `has_equivalent_names_and_types` ignores nullability and metadata while requiring compatible qualified field names/types; `datatype_is_logically_equal` is weaker and treats `Dictionary<K,V>` as logically equal to `V`, dictionary key changes as logically equal when values match, and `Utf8` as logically equal to `Utf8View`; `datatype_is_semantically_equal` is another comparison mode with its own tolerance around metadata/nullability/decimal/timezone details. ([Docs.rs][2])

---

## S5.2 Compatibility levels

### S5.2.1 Physical exact equality

Use when data already exists as Arrow arrays or will be emitted by a physical operator.

```text id="xdou3y"
PhysicalExact requires:
  same field count
  same field order
  same field names, if field names are part of checked contract
  same Arrow DataType
  compatible nullability contract
  arrays in same order as fields
  every emitted RecordBatch matches schema
```

`RecordBatch::try_new` requires non-empty columns, field count equal to column count, each schema field type equal to the corresponding array type, and all columns having equal length; `new_unchecked` skips validation and can cause undefined behavior if schema and data do not match. ([Docs.rs][3])

Required for:

```text id="mjr41c"
RecordBatch::try_new
custom RecordBatchStream output
ExecutionPlan::schema() vs emitted batches
Arrow IPC exact output
UDF physical array downcasts
provider scan projection output
strict Parquet/Arrow roundtrip tests
```

Rust validator:

```rust id="y6grnv"
use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::error::{DataFusionError, Result};

pub fn validate_physical_exact_schema(expected: &SchemaRef, batch: &RecordBatch) -> Result<()> {
    let actual = batch.schema();

    if expected.fields().len() != actual.fields().len() {
        return Err(DataFusionError::Execution(format!(
            "field count mismatch: expected={}, actual={}",
            expected.fields().len(),
            actual.fields().len()
        )));
    }

    for (idx, (e, a)) in expected.fields().iter().zip(actual.fields().iter()).enumerate() {
        if e.name() != a.name() {
            return Err(DataFusionError::Execution(format!(
                "field name mismatch at index {idx}: expected={}, actual={}",
                e.name(),
                a.name()
            )));
        }

        if e.data_type() != a.data_type() {
            return Err(DataFusionError::Execution(format!(
                "field type mismatch at index {idx}: field={} expected={:?}, actual={:?}",
                e.name(),
                e.data_type(),
                a.data_type()
            )));
        }

        if !e.is_nullable() && a.is_nullable() {
            return Err(DataFusionError::Execution(format!(
                "nullability contract weakened at index {idx}: field={}",
                e.name()
            )));
        }
    }

    Ok(())
}
```

---

### S5.2.2 Logical equality

Use when validating planner equivalence, optimizer rewrites, DataFusion logical schema compatibility, or query-derived schema drift.

```text id="m4z6lb"
Logical equality tolerates:
  representation-equivalent strings
  dictionary encoding differences
  planner-level string view differences
  selected metadata/nullability differences depending helper
```

`DFSchema::datatype_is_logically_equal` is weaker than semantic equality in the docs and treats dictionary-encoded UTF8 as logically equivalent to plain UTF8, dictionary key-type changes as logically equivalent when values match, and `Utf8` as logically equal to `Utf8View`. ([Docs.rs][2])

Use for:

```text id="3ckpqp"
optimizer rewrite validation
logical plan comparison
DataFrame plan schema compatibility
generated SQL output type audit
dictionary/plain-string equivalence in planning
Utf8 vs Utf8View non-physical equivalence
```

Do not use for:

```text id="4ro0sp"
RecordBatch construction
UDF array downcast selection
physical operator output
exact API contract
storage schema fingerprint
```

---

### S5.2.3 Semantic equality

Use when the platform decides two schemas mean the same thing even if their physical representation is not exactly the same.

```text id="9swa47"
Semantic equality may consider:
  field identity
  semantic unit
  source lineage
  domain type
  decimal precision/scale policy
  timestamp unit/timezone policy
  metadata allowlist
  nullable contract
  field order policy
```

DataFusion’s `datatype_is_semantically_equal` intentionally differs from physical equality and has specific tolerance around metadata/nullability/decimal/timezone dimensions, so platform-level semantic equality must be explicit rather than inferred from raw `==`. ([Docs.rs][2])

Example:

```text id="tms4tz"
Field A:
  name = price_usd_bbl
  type = Decimal128(20, 4)
  metadata.semantic.unit = USD/bbl

Field B:
  name = price_usd_bbl
  type = Decimal128(18, 4)
  metadata.semantic.unit = USD/bbl

PhysicalExact: false
LogicalDataFusion: maybe false / helper-dependent
SemanticPlatform: true only if precision loss risk is acceptable by policy
UserFacingContract: likely false unless contract version changes
```

---

### S5.2.4 Coercible equality

Use when inputs are not compatible as-is but can be made compatible via explicit casts or projections.

```text id="1f3mc5"
Coercible equality requires:
  target schema selected
  direction selected
  cast expression generated
  lossiness classified
  failure behavior selected
  NULL behavior understood
  rejected-row audit if dirty data
```

SQL coercion examples:

```sql id="pg5g27"
SELECT
  CAST(id AS BIGINT) AS id,
  CAST(amount AS DOUBLE) AS amount
FROM raw_orders;
```

Arrow-exact coercion examples:

```sql id="aq31c4"
SELECT
  arrow_cast(raw_ts, 'Timestamp(ms)') AS event_ts_ms,
  arrow_cast(raw_amount, 'Decimal128(20, 4)') AS amount_dec
FROM raw_events;
```

Dirty-data coercion:

```sql id="in0k2w"
SELECT
  arrow_try_cast(raw_amount, 'Decimal128(20, 4)') AS amount_dec
FROM raw_events;
```

DataFusion documents that SQL types are mapped to Arrow types in DDL and `CAST`; `arrow_typeof` exposes the Arrow type of an expression, and `arrow_cast` casts to a specific Arrow type string. ([Apache DataFusion][4])

---

### S5.2.5 User-facing contract equality

Use for APIs, semantic views, materialized outputs, Parquet lake contracts, model inputs, and downstream consumers.

```text id="v2wdho"
UserFacingContract requires:
  same field names
  same field order, unless consumer is name-based
  same documented type
  same nullable contract
  same semantic units
  same timezone/precision/scale policy
  same nested field paths
  same partition-column semantics
  accepted metadata drift only
  versioning if any visible change occurs
```

Contract equality is stricter than DataFusion logical equality:

```text id="u1xjkr"
Utf8 vs Utf8View:
  logical equality may pass
  API contract may fail if downstream expects StringArray/Utf8

Decimal128(20,4) vs Decimal128(18,4):
  semantic helper may tolerate
  finance API contract should likely fail

Timestamp(ns, UTC) vs Timestamp(us, UTC):
  DataFusion may coerce
  API contract may fail due precision loss
```

---

## S5.3 Type coercion cases

### S5.3.1 Integer widening

```text id="zliijy"
Int8  → Int16 → Int32 → Int64
UInt8 → UInt16 → UInt32 → UInt64
```

Safe-ish widening:

```sql id="b4717d"
SELECT CAST(small_id AS BIGINT) AS id
FROM raw;
```

Policy matrix:

| From              |                      To |             Lossless? | Default                           |
| ----------------- | ----------------------: | --------------------: | --------------------------------- |
| `Int8`            | `Int16`/`Int32`/`Int64` |                   yes | allow if target contract explicit |
| `Int32`           |                 `Int64` |                   yes | allow                             |
| `UInt32`          |                 `Int64` | value-range dependent | require max-value proof           |
| `Int64`           |                 `Int32` |                    no | reject unless explicit lossy cast |
| signed ↔ unsigned |         range dependent |         require proof | reject by default                 |

DataFusion SQL maps `TINYINT`, `SMALLINT`, `INT`/`INTEGER`, and `BIGINT` to Arrow `Int8`, `Int16`, `Int32`, and `Int64`; unsigned variants map to `UInt8`, `UInt16`, `UInt32`, and `UInt64`. ([Apache DataFusion][4])

Agent output:

```json id="zlwlyf"
{
  "field": "id",
  "from": "Int32",
  "to": "Int64",
  "conversion": "integer_widening",
  "lossless": true,
  "cast_sql": "CAST(id AS BIGINT) AS id",
  "decision": "allow"
}
```

---

### S5.3.2 Float promotion

```text id="q71ci4"
Float32 → Float64 = allow
Float64 → Float32 = reject unless explicit precision-loss policy
Int* → Float64 = potentially lossy for large integers
Decimal → Float64 = lossy; reject for finance
Float → Decimal = parse/rounding policy required
```

SQL:

```sql id="4venb5"
SELECT CAST(metric_f32 AS DOUBLE) AS metric_f64
FROM metrics;
```

DataFusion maps SQL `FLOAT` and `REAL` to Arrow `Float32`, and `DOUBLE` to `Float64`. ([Apache DataFusion][4])

Policy:

```text id="we2gfc"
Scientific metrics:
  Float32 → Float64 allowed
  Int32 → Float64 allowed with caution

Money / economics:
  Float → Decimal requires explicit scale/rounding
  Decimal → Float rejected by default
```

---

### S5.3.3 Decimal precision and scale

```text id="06d9cg"
Decimal128(p,s), p <= 38
Decimal256(p,s), 39 <= p <= 76
p > 76 unsupported by DataFusion SQL type mapping
```

DataFusion maps `DECIMAL(precision, scale)` to `Decimal128` when precision is at most 38 and to `Decimal256` above 38; maximum supported precision is 76. ([Apache DataFusion][4])

Compatibility cases:

| Case                    | Example                                  | Risk                       | Default                          |
| ----------------------- | ---------------------------------------- | -------------------------- | -------------------------------- |
| same p/s                | `Decimal128(20,4)` vs `Decimal128(20,4)` | none                       | allow                            |
| precision widen         | `Decimal128(18,4)` → `Decimal128(20,4)`  | generally lossless         | allow if target explicit         |
| precision narrow        | `Decimal128(20,4)` → `Decimal128(18,4)`  | overflow risk              | reject                           |
| scale widen             | `Decimal128(20,2)` → `Decimal128(22,4)`  | representation changes     | allow only with explicit policy  |
| scale narrow            | `Decimal128(20,4)` → `Decimal128(20,2)`  | rounding/truncation        | reject unless rounding specified |
| Decimal128 → Decimal256 | wider container                          | allow if consumer supports |                                  |
| Decimal256 → Decimal128 | overflow risk                            | reject                     |                                  |

SQL:

```sql id="x8y2b0"
SELECT
  arrow_cast(price_raw, 'Decimal128(20, 4)') AS price_usd_bbl
FROM price_stage;
```

Required report field:

```json id="3d1mts"
{
  "conversion": "decimal_scale_change",
  "from": "Decimal128(20,4)",
  "to": "Decimal128(20,2)",
  "lossless": false,
  "requires_rounding_policy": true,
  "decision": "reject"
}
```

---

### S5.3.4 `Utf8` / `Utf8View` / `LargeUtf8`

DataFusion maps SQL string declarations such as `CHAR`, `VARCHAR`, `TEXT`, and `STRING` to `Utf8View` by default, configurable with `datafusion.sql_parser.map_string_types_to_utf8view=false`; string literals may still be `Utf8` under `arrow_typeof`. ([Apache DataFusion][4])

Compatibility:

```text id="edep6r"
Utf8 ↔ Utf8View:
  LogicalDataFusion: compatible
  PhysicalExact: not equal
  UDF downcast: must handle both or normalize
  API contract: decide

Utf8 ↔ LargeUtf8:
  logically string-like
  offset width differs
  physical exact false
  cast may be needed

Utf8View:
  preferred by DataFusion SQL declarations by default
  downstream Rust code expecting StringArray may break
```

Policy options:

```sql id="n694ok"
SET datafusion.sql_parser.map_string_types_to_utf8view = false;
```

or explicit output:

```sql id="bs04xe"
SELECT arrow_cast(name, 'Utf8') AS name
FROM people;
```

Agent rule:

```text id="tu8dxc"
Never assume string-compatible means physically same Arrow array type.
Custom UDFs should either:
  support Utf8, Utf8View, LargeUtf8, Dictionary strings
  or force/validate a specific string representation before invocation.
```

---

### S5.3.5 Dictionary strings

DataFusion `DFSchema::datatype_is_logically_equal` treats `Dictionary<K,V>` as logically equal to plain `V`, and dictionary key-type differences as logically equal when values match. ([Docs.rs][2])

Compatibility:

```text id="hhxza8"
Dictionary(Int32, Utf8) vs Utf8:
  LogicalDataFusion: equal
  PhysicalExact: false
  UDF physical downcast: false unless dictionary path supported
  API output: depends whether dictionary encoding is allowed
```

Policy:

```text id="q3ozud"
For query planning:
  allow dictionary/plain logical equivalence.

For sink/API:
  either preserve dictionary explicitly
  or cast/densify to plain Utf8 with documented cost.

For UDFs:
  inspect DataType and implement dictionary path or reject.
```

---

### S5.3.6 Timestamp units and time zones

DataFusion maps SQL `TIMESTAMP` to Arrow `Timestamp(Nanosecond, None)` in the SQL type mapping table; `arrow_cast` can cast to specific Arrow timestamp precision and timezone strings such as `Timestamp(s)`, `Timestamp(ms)`, `Timestamp(us)`, `Timestamp(ns)`, or timestamp with timezone. ([Apache DataFusion][4])

Compatibility:

| From               | To                                |                           Loss | Default                         |
| ------------------ | --------------------------------- | -----------------------------: | ------------------------------- |
| `Timestamp(ms)`    | `Timestamp(us)`                   |              no precision loss | allow                           |
| `Timestamp(us)`    | `Timestamp(ns)`                   |              no precision loss | allow                           |
| `Timestamp(ns)`    | `Timestamp(us)`                   |        possible precision loss | reject unless truncation policy |
| timezone-free      | timezone-aware                    | semantic reinterpretation risk | reject unless policy            |
| timezone-aware UTC | timezone-aware `America/New_York` |           conversion semantics | explicit conversion required    |

SQL:

```sql id="r91xj8"
SELECT
  arrow_cast(event_ts_raw, 'Timestamp(ns, "UTC")') AS event_ts_utc
FROM events_stage;
```

Report:

```json id="nzuf4e"
{
  "field": "event_ts",
  "from": "Timestamp(ns, None)",
  "to": "Timestamp(ms, \"UTC\")",
  "lossless": false,
  "semantic_risk": "timezone_and_precision_change",
  "decision": "reject"
}
```

---

### S5.3.7 List element coercion

```text id="q85otz"
List<Int32> → List<Int64>
  allowed only if element widening policy allowed

List<Utf8> ↔ List<Utf8View>
  logical equality may be acceptable
  physical exact false

List<Struct<...>>
  recurse element field compatibility

List item nullability:
  parent list nullable != child item nullable
```

Policy:

```text id="kyoxqc"
List compatibility = parent nullability policy + element type compatibility + element nullability policy.
Do not compare only top-level DataType string.
```

Example:

```rust id="p33iz8"
#[derive(Debug, Clone)]
pub struct NestedCompatibilityFinding {
    pub path: String,          // e.g. "assays[].sulfur_wt_pct"
    pub expected: String,
    pub actual: String,
    pub lossless: bool,
    pub decision: CompatibilityDecision,
}
```

---

### S5.3.8 Struct field-name mapping

DataFusion uses **name-based field mapping** for struct coercion across operations such as array construction, `UNION`, and joins; struct fields with different order can be matched by name rather than by position. ([Apache DataFusion][5])

SQL example:

```sql id="o19r5n"
SELECT {a: 1, b: 2} AS s
UNION ALL
SELECT {b: 3, a: 4} AS s;
```

DataFusion’s struct coercion docs show this kind of field reordering producing values matched by field name rather than position. ([Apache DataFusion][5])

Policy:

```text id="09qyqb"
Struct compatibility:
  match children by field name
  recursively compare child types/nullability
  allow field-order differences only for logical/semantic compatibility
  preserve canonical output order from target schema
  reject duplicate child names
```

Do not assume:

```text id="0xpbfq"
Struct field order difference is physically exact.
Name-based matching is a logical coercion rule, not permission to change storage schemas silently.
```

---

### S5.3.9 Comparison coercion vs unification coercion (DataFusion 54)

DataFusion 54 splits binary coercion into two explicit context families in `datafusion_expr_common::type_coercion::binary` (re-exported through `datafusion-expr`):

```text
comparison_coercion(lhs_type, rhs_type)
  = comparison contexts: binary comparison operators, IN lists,
    CASE/WHEN conditions, BETWEEN
  = numeric-preferring: a string operand compared against a numeric operand
    is coerced to the numeric type

type_union_coercion(lhs_type, rhs_type)
  = type-unification contexts: UNION, CASE THEN/ELSE branches, NVL2
  = string-preferring: when a numeric branch meets a string branch, both are
    unified to string (every number has a textual representation; not every
    string parses as a number)
```

The 53.x helper `comparison_coercion_numeric` is removed in 54; `comparison_coercion` itself is numeric-preferring by default, and `type_union_coercion` is new.

Behavioral consequences that change results, output schemas, and error surfaces:

```text
SELECT 5 > '100'
  53.x: could compare as strings → '5' > '100' is true
  54.x: '100' is cast to the numeric type → numeric comparison → false

Invalid numeric strings in comparison contexts:
  int_col > 'abc' → runtime cast error instead of a silent string comparison

Mixed-type IN lists:
  str_col IN ('a', 1) → may fail coercion during planning
  instead of stringifying every element

UNION with mixed string/numeric branches:
  SELECT 1 UNION SELECT 'a' → both sides unified to string (type_union_coercion)
```

Agent policy:

```text
Never rely on the implicit string↔numeric coercion direction across versions.
Cast comparison operands explicitly when a column is Utf8 but semantically numeric.
Keep IN-list literals homogeneous; cast the probe or list side explicitly otherwise.
Reason about UNION / CASE THEN-ELSE / NVL2 output types with type_union_coercion
  (string-preferring), and about predicates with comparison_coercion
  (numeric-preferring).
When upgrading, add regression tests for 5 > '100'-style predicates and for
  dirty string columns compared against numeric literals.
```

---

## S5.4 Operator-specific schema compatibility

### S5.4.1 `UNION`

```sql id="mwj0ot"
SELECT
  CAST(id AS BIGINT) AS id,
  CAST(amount AS DOUBLE) AS amount
FROM current_orders

UNION ALL

SELECT
  CAST(id AS BIGINT) AS id,
  CAST(amount AS DOUBLE) AS amount
FROM historical_orders;
```

Compatibility contract:

```text id="7trwdz"
UNION requires:
  compatible column count
  compatible field positions for SQL UNION
  coercible types per output position
  deterministic output names from first branch / planner behavior
  explicit casts for stable contracts
```

DataFusion SELECT docs warn that set operations require compatible query shapes, and the attached SQL syntax section notes that set-operation inputs must have compatible column counts and coercible types. In DataFusion 54, per-position branch unification is computed by `type_union_coercion`, which prefers strings when a numeric branch meets a string branch (`SELECT 1 UNION SELECT 'a'` yields a string column); see S5.3.9.

Agent rules:

```text id="mxb0wx"
Use UNION ALL unless duplicate elimination is required.
Explicitly project same columns in same order.
Explicitly cast all drift-prone columns.
For structs, rely on name-based field mapping only after test coverage.
Snapshot output schema.
```

Name-based union-like DataFrame policy:

```rust id="vbmro4"
// When source columns may be reordered:
let combined = left.union_by_name(right)?;

// When schemas are exact and ordered:
let combined = left.union(right)?;
```

---

### S5.4.2 `JOIN`

Join compatibility spans key expressions, output schema, and nullability introduced by outer joins.

```sql id="yd6z2d"
SELECT
  o.order_id AS order_id,
  o.customer_id AS order_customer_id,
  c.customer_id AS customer_id,
  c.customer_name AS customer_name
FROM orders AS o
JOIN customers AS c
  ON CAST(o.customer_id AS BIGINT) = c.customer_id;
```

Compatibility contract:

```text id="h3o0mk"
Join key compatibility:
  equality predicate operands must be comparable/coercible
  avoid implicit signed/unsigned drift
  avoid string/numeric key comparison without explicit cast

Join output compatibility:
  duplicate names must be aliased
  outer joins can widen nullability on preserved/non-preserved sides
  semi/anti joins project only one side
  struct join conditions may use name-based field mapping
```

Struct join conditions can match struct fields by name rather than position according to the DataFusion struct coercion documentation. ([Apache DataFusion][5])

---

### S5.4.3 `CASE`

```sql id="e2qhqw"
SELECT
  CASE
    WHEN amount IS NULL THEN CAST(NULL AS DOUBLE)
    WHEN amount < 0 THEN 0.0
    ELSE amount
  END AS amount_nonnegative
FROM orders;
```

Compatibility contract:

```text id="0fl0tl"
CASE branches must coerce to one output type.
NULL arms should be typed.
Mixing integers/floats produces promoted numeric type.
Mixing numeric/string should be rejected unless explicit stringification intended.
```

Bad:

```sql id="vhcf8n"
CASE
  WHEN failed THEN 'N/A'
  ELSE amount
END AS amount
```

Good:

```sql id="pms5bx"
CASE
  WHEN failed THEN NULL
  ELSE arrow_cast(amount_raw, 'Decimal128(20, 4)')
END AS amount
```

Agent rule:

```text id="knx37q"
CASE output must be type-audited with arrow_typeof in tests.
Typed NULLs avoid context-dependent inference.
```

DataFusion 54 coercion split for `CASE`: the `WHEN` comparison side uses numeric-preferring `comparison_coercion`, while `THEN`/`ELSE` branch unification uses string-preferring `type_union_coercion` — so a numeric branch mixed with a string branch unifies to string rather than erroring, but the same mix inside a `WHEN` comparison follows numeric semantics. See S5.3.9.

---

### S5.4.4 `COALESCE`

```sql id="974rpx"
SELECT
  COALESCE(discount_amount, 0.0) AS discount_amount
FROM orders;
```

Compatibility contract:

```text id="7scj4t"
COALESCE arguments must coerce to common type.
First non-null value determines runtime value, not necessarily target type.
Use typed literal/cast to control output.
```

Bad:

```sql id="br0u0f"
COALESCE(amount, '0')
```

Good:

```sql id="z985ka"
COALESCE(amount, CAST(0 AS DOUBLE)) AS amount
```

For decimals:

```sql id="1u1f4p"
COALESCE(amount_dec, arrow_cast('0', 'Decimal128(20, 4)')) AS amount_dec
```

---

### S5.4.5 Aggregate outputs

Aggregate outputs are function-specific.

```sql id="uvzxd9"
SELECT
  unit_id,
  COUNT(*) AS row_count,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h,
  AVG(sulfur_wt_pct) AS avg_sulfur_wt_pct
FROM streams
GROUP BY unit_id;
```

Compatibility contract:

```text id="hgd51y"
COUNT returns integral count type; snapshot exact Arrow type with arrow_typeof.
SUM output type may widen/promote depending input type.
AVG output type may differ from input.
MIN/MAX preserve comparable input type.
Aggregate nullability depends on aggregate and empty groups/filter semantics.
```

Agent rules:

```text id="nxm236"
Alias every aggregate.
Audit aggregate output type with arrow_typeof.
Do not assume SUM(Int32) output is Int32.
Do not assume AVG(Decimal) output precision/scale without tests.
```

---

### S5.4.6 CTAS / view outputs

```sql id="i0o5s7"
CREATE VIEW streams_v1 AS
SELECT
  stream_id AS stream_id,
  arrow_cast(mass_flow_raw, 'Float64') AS mass_flow_kg_h,
  arrow_cast(event_ts_raw, 'Timestamp(ns, "UTC")') AS event_ts
FROM streams_stage;
```

Compatibility contract:

```text id="w46l96"
View/CTAS schema = SELECT output schema.
Every output expression must be named.
Every contract-sensitive type must be cast.
No SELECT *.
Output schema is API/storage contract.
```

Required post-check:

```sql id="c0s4c0"
DESCRIBE streams_v1;

SELECT
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM streams_v1
LIMIT 1;
```

---

## S5.5 Validation API

### S5.5.1 `DFSchema` equality helpers

Relevant helpers:

```text id="sbgprt"
has_equivalent_names_and_types(other)
datatype_is_logically_equal(dt1, dt2)
datatype_is_semantically_equal(dt1, dt2)
strip_qualifiers()
replace_qualifier(...)
matches_arrow_schema(...)
```

`has_equivalent_names_and_types` returns `Ok` when two schemas have the same qualified named fields with compatible data types and ignores nullability and metadata; `datatype_is_logically_equal` and `datatype_is_semantically_equal` expose weaker/different type comparisons. ([Docs.rs][2])

Illustrative use:

```rust id="xw5roj"
use datafusion::common::DFSchema;

pub fn validate_logical_schema_equivalence(
    expected: &DFSchema,
    actual: &DFSchema,
) -> datafusion::error::Result<()> {
    expected.has_equivalent_names_and_types(actual)?;
    Ok(())
}
```

Policy warning:

```text id="af9q5l"
has_equivalent_names_and_types ignores nullability and metadata.
Do not use it alone for user-facing contract equality.
```

---

### S5.5.2 `ExprSchemable::get_type`

```rust id="zqxq1n"
use datafusion::logical_expr::ExprSchemable;
use datafusion::prelude::*;

fn infer_type(schema: &datafusion::common::DFSchema) -> datafusion::error::Result<()> {
    let expr = col("c1") + col("c2");
    let dtype = expr.get_type(schema)?;
    println!("{dtype:?}");
    Ok(())
}
```

`ExprSchemable::get_type` returns the Arrow `DataType` of an expression based on an `ExprSchema`; `DFSchema` implements that trait, and the docs show that adding `Int32` and `Float32` produces a `Float32` expression type. It errors if a column is missing or an expression is incorrectly typed, such as adding UTF8 and bool. ([Docs.rs][6])

Use for:

```text id="pjtb1r"
preflight generated expressions
validate CASE branch output
validate arithmetic promotion
validate join key casts
validate UDF call outputs where expression-level type is known
```

---

### S5.5.3 `ExprSchemable::nullable`

```rust id="51wjkh"
use datafusion::logical_expr::ExprSchemable;

let nullable = expr.nullable(df_schema)?;
```

`nullable` returns expression nullability based on the input schema and errors if referenced columns cannot be resolved. ([Docs.rs][6])

Use for:

```text id="gfzhh6"
detect output nullability widening
validate non-null API outputs
flag aggregate/window nullability
reject nullable expressions for required sink fields
```

---

### S5.5.4 `ExprSchemable::to_field`

```rust id="9dfntg"
use datafusion::logical_expr::ExprSchemable;

let (qualifier, field) = expr.to_field(df_schema)?;
println!(
    "qualifier={:?}, name={}, type={:?}, nullable={}",
    qualifier,
    field.name(),
    field.data_type(),
    field.is_nullable()
);
```

`to_field` converts an expression into an Arrow `Field` compatible with the expression, including appropriate metadata and nullability, and is described as the primary mechanism for determining field-level schemas for expressions. ([Docs.rs][6])

Use for:

```text id="fvhyk1"
projection schema construction
computed field validation
alias checking
sink preflight
view/CTAS output contract generation
```

---

### S5.5.5 `Schema::try_merge`

```rust id="as3p66"
use datafusion::arrow::datatypes::Schema;

let merged = Schema::try_merge(vec![
    schema_a.as_ref().clone(),
    schema_b.as_ref().clone(),
])?;
```

Arrow `Schema::try_merge` can merge compatible schemas; Arrow `Schema` is an ordered sequence of fields plus metadata, so platform code must still decide whether merge behavior is acceptable for the contract. ([Docs.rs][1])

Policy wrapper:

```text id="fqw9k4"
Schema::try_merge is not a governance policy.
It is a mechanical merge primitive.
Wrap it with:
  allowed extra fields?
  allowed nullable widening?
  allowed type widening?
  allowed metadata drift?
  allowed nested changes?
```

---

### S5.5.6 `RecordBatch::try_new`

```rust id="xg7zvk"
use datafusion::arrow::record_batch::RecordBatch;

let batch = RecordBatch::try_new(schema, columns)?;
```

Use for physical validation. `RecordBatch::try_new` requires matching schema field count, matching column count, exact data type match by index, and equal column lengths. ([Docs.rs][3])

Policy:

```text id="saat69"
RecordBatch validation is physical exact validation.
It does not replace logical/schema contract validation.
It catches emitted data mismatch, not bad planning intent.
```

---

## S5.6 Nullability compatibility

DataFusion’s table-constraint docs state that nullability is the only schema property DataFusion enforces; returning null values for columns marked non-nullable causes runtime errors during execution, but DataFusion does not check/enforce nullability when data is ingested. Primary/unique/foreign/check constraints are not validated by DataFusion core. ([Apache DataFusion][7])

Nullability matrix:

| Expected                             | Actual             |                         Runtime risk | Default decision                                 |
| ------------------------------------ | ------------------ | -----------------------------------: | ------------------------------------------------ |
| nullable                             | nullable           |                                 none | allow                                            |
| nullable                             | non-nullable       |                      stronger actual | allow, but do not tighten contract automatically |
| non-nullable                         | nullable           | possible runtime/null contract break | reject unless proof/filter                       |
| non-nullable                         | non-nullable       |                                 none | allow                                            |
| non-nullable field but runtime nulls | runtime error risk |                    reject/quarantine |                                                  |

SQL hardening:

```sql id="p8nsr9"
SELECT *
FROM streams
WHERE stream_id IS NOT NULL
  AND unit_id IS NOT NULL;
```

Contract cast with null rejection:

```sql id="0na7q1"
CREATE VIEW streams_contract AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM streams_stage
WHERE stream_id IS NOT NULL
  AND unit_id IS NOT NULL
  AND mass_flow_kg_h IS NOT NULL;
```

Agent rule:

```text id="s7tbsv"
Never tighten nullable=true to nullable=false based on declared intent alone.
Require full-data validation, filtering, or provider-level enforcement.
```

---

## S5.7 Compatibility decision model

```rust id="vujxhz"
#[derive(Debug, Clone, serde::Serialize)]
pub enum CompatibilityLevel {
    PhysicalExact,
    LogicalDataFusion,
    SemanticPlatform,
    Coercible,
    UserFacingContract,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CompatibilityDecision {
    Accept,
    AcceptWithWarning,
    CastRequired,
    Reject,
    RequiresMigration,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ConversionLossiness {
    Lossless,
    PotentiallyLossy,
    Lossy,
    Unknown,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TypeCompatibilityFinding {
    pub path: String,
    pub expected_type: String,
    pub actual_type: String,
    pub level: CompatibilityLevel,
    pub decision: CompatibilityDecision,
    pub lossiness: ConversionLossiness,
    pub proposed_cast_sql: Option<String>,
    pub reason: String,
}
```

Default policy:

```rust id="5958ar"
#[derive(Debug, Clone)]
pub struct TypeCompatibilityPolicy {
    pub allow_integer_widening: bool,
    pub allow_float_promotion: bool,
    pub allow_decimal_precision_widening: bool,
    pub allow_decimal_scale_change: bool,
    pub allow_utf8_view_equivalence: bool,
    pub allow_dictionary_string_logical_equivalence: bool,
    pub allow_timestamp_precision_widening: bool,
    pub allow_timestamp_precision_loss: bool,
    pub allow_timezone_change: bool,
    pub allow_struct_field_reordering_by_name: bool,
    pub require_user_contract_exact_names: bool,
    pub require_user_contract_exact_nullability: bool,
}

impl Default for TypeCompatibilityPolicy {
    fn default() -> Self {
        Self {
            allow_integer_widening: true,
            allow_float_promotion: true,
            allow_decimal_precision_widening: true,
            allow_decimal_scale_change: false,
            allow_utf8_view_equivalence: true,
            allow_dictionary_string_logical_equivalence: true,
            allow_timestamp_precision_widening: true,
            allow_timestamp_precision_loss: false,
            allow_timezone_change: false,
            allow_struct_field_reordering_by_name: true,
            require_user_contract_exact_names: true,
            require_user_contract_exact_nullability: true,
        }
    }
}
```

---

## S5.8 Proposed casts

### S5.8.1 Cast plan object

```rust id="uq3lb6"
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProposedCast {
    pub field_path: String,
    pub source_expr: String,
    pub target_alias: String,
    pub source_type: String,
    pub target_type: String,
    pub sql_cast: String,
    pub rust_arrow_type: String,
    pub lossiness: ConversionLossiness,
    pub failure_mode: CastFailureMode,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CastFailureMode {
    FailFast,
    NullOnFailure,
    RequiresRejectedRowsAudit,
}
```

### S5.8.2 Fail-fast casts

```sql id="o7uhyo"
SELECT
  CAST(id AS BIGINT) AS id,
  arrow_cast(event_ts, 'Timestamp(ns, "UTC")') AS event_ts,
  arrow_cast(price_raw, 'Decimal128(20, 4)') AS price_usd_bbl
FROM raw;
```

### S5.8.3 Dirty casts with audit

```sql id="0w2aub"
WITH parsed AS (
  SELECT
    raw_id,
    raw_price,
    arrow_try_cast(raw_id, 'Int64') AS id,
    arrow_try_cast(raw_price, 'Decimal128(20, 4)') AS price_usd_bbl
  FROM raw_prices
)
SELECT *
FROM parsed
WHERE id IS NOT NULL
  AND price_usd_bbl IS NOT NULL;
```

Rejected:

```sql id="kkj9v4"
WITH parsed AS (
  SELECT
    raw_id,
    raw_price,
    arrow_try_cast(raw_id, 'Int64') AS id,
    arrow_try_cast(raw_price, 'Decimal128(20, 4)') AS price_usd_bbl
  FROM raw_prices
)
SELECT *
FROM parsed
WHERE (raw_id IS NOT NULL AND id IS NULL)
   OR (raw_price IS NOT NULL AND price_usd_bbl IS NULL);
```

---

## S5.9 Compatibility report

```json id="p5br0z"
{
  "report_version": "schema-compatibility-v1",
  "context": {
    "operation": "UNION",
    "left_relation": "curated.current_streams",
    "right_relation": "curated.historical_streams",
    "target_contract": "semantic.streams_v2"
  },
  "summary": {
    "decision": "CastRequired",
    "lossy_conversions": 0,
    "rejected_fields": 0,
    "warnings": 2
  },
  "findings": [
    {
      "path": "mass_flow_kg_h",
      "expected_type": "Float64",
      "actual_type": "Float32",
      "level": "Coercible",
      "decision": "CastRequired",
      "lossiness": "Lossless",
      "proposed_cast_sql": "CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h",
      "reason": "Float32 to Float64 promotion allowed by policy"
    },
    {
      "path": "stream_name",
      "expected_type": "Utf8View",
      "actual_type": "Utf8",
      "level": "LogicalDataFusion",
      "decision": "AcceptWithWarning",
      "lossiness": "Lossless",
      "proposed_cast_sql": "arrow_cast(stream_name, 'Utf8View') AS stream_name",
      "reason": "Utf8 and Utf8View are logically equal but physically different"
    }
  ],
  "required_sql_projection": [
    "stream_id AS stream_id",
    "CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h",
    "arrow_cast(stream_name, 'Utf8View') AS stream_name"
  ]
}
```

Required report fields:

```text id="29d9hr"
operation context
expected schema fingerprint
actual schema fingerprint
comparison level
field path
expected type
actual type
nullability expected/actual
metadata status
lossiness
decision
proposed cast
rejected reason
test query
```

---

## S5.10 Compatibility engine skeleton

```rust id="3dh9bf"
use datafusion::arrow::datatypes::{DataType, Field, SchemaRef};
use datafusion::error::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct FieldCompatibilityContext {
    pub path: String,
    pub expected: Field,
    pub actual: Field,
    pub policy: TypeCompatibilityPolicy,
}

pub fn compare_field(ctx: FieldCompatibilityContext) -> TypeCompatibilityFinding {
    let expected_type = ctx.expected.data_type().clone();
    let actual_type = ctx.actual.data_type().clone();

    if expected_type == actual_type
        && ctx.expected.is_nullable() == ctx.actual.is_nullable()
    {
        return TypeCompatibilityFinding {
            path: ctx.path,
            expected_type: format!("{expected_type:?}"),
            actual_type: format!("{actual_type:?}"),
            level: CompatibilityLevel::PhysicalExact,
            decision: CompatibilityDecision::Accept,
            lossiness: ConversionLossiness::Lossless,
            proposed_cast_sql: None,
            reason: "exact type and nullability match".to_string(),
        };
    }

    let type_result = compare_data_type(&ctx.path, &expected_type, &actual_type, &ctx.policy);

    if !ctx.expected.is_nullable() && ctx.actual.is_nullable() {
        return TypeCompatibilityFinding {
            decision: CompatibilityDecision::Reject,
            lossiness: ConversionLossiness::Unknown,
            reason: "actual nullable field cannot satisfy non-null expected contract without proof/filter".to_string(),
            ..type_result
        };
    }

    type_result
}

pub fn compare_data_type(
    path: &str,
    expected: &DataType,
    actual: &DataType,
    policy: &TypeCompatibilityPolicy,
) -> TypeCompatibilityFinding {
    if expected == actual {
        return accept(path, expected, actual, CompatibilityLevel::PhysicalExact);
    }

    if datafusion::common::DFSchema::datatype_is_logically_equal(expected, actual) {
        return TypeCompatibilityFinding {
            path: path.to_string(),
            expected_type: format!("{expected:?}"),
            actual_type: format!("{actual:?}"),
            level: CompatibilityLevel::LogicalDataFusion,
            decision: CompatibilityDecision::AcceptWithWarning,
            lossiness: ConversionLossiness::Lossless,
            proposed_cast_sql: None,
            reason: "DataFusion logical type equality accepted by policy".to_string(),
        };
    }

    if let Some(cast) = propose_cast(path, expected, actual, policy) {
        return TypeCompatibilityFinding {
            path: path.to_string(),
            expected_type: format!("{expected:?}"),
            actual_type: format!("{actual:?}"),
            level: CompatibilityLevel::Coercible,
            decision: CompatibilityDecision::CastRequired,
            lossiness: cast.lossiness,
            proposed_cast_sql: Some(cast.sql_cast),
            reason: "explicit cast required to satisfy target schema".to_string(),
        };
    }

    TypeCompatibilityFinding {
        path: path.to_string(),
        expected_type: format!("{expected:?}"),
        actual_type: format!("{actual:?}"),
        level: CompatibilityLevel::UserFacingContract,
        decision: CompatibilityDecision::Reject,
        lossiness: ConversionLossiness::Unknown,
        proposed_cast_sql: None,
        reason: "no allowed compatibility or coercion path".to_string(),
    }
}

fn accept(path: &str, expected: &DataType, actual: &DataType, level: CompatibilityLevel) -> TypeCompatibilityFinding {
    TypeCompatibilityFinding {
        path: path.to_string(),
        expected_type: format!("{expected:?}"),
        actual_type: format!("{actual:?}"),
        level,
        decision: CompatibilityDecision::Accept,
        lossiness: ConversionLossiness::Lossless,
        proposed_cast_sql: None,
        reason: "compatible".to_string(),
    }
}

fn propose_cast(
    path: &str,
    expected: &DataType,
    actual: &DataType,
    policy: &TypeCompatibilityPolicy,
) -> Option<ProposedCast> {
    // Implementation-specific:
    // match integer widening, float promotion, decimal precision, timestamps, strings, nested recursion.
    None
}
```

Version note:

```text id="lqdbql"
Treat this skeleton as platform code.
Verify exact DataFusion public paths/methods against pinned docs.rs before compiling.
```

---

## S5.11 Operator preflight examples

### S5.11.1 UNION preflight

```rust id="3v89y4"
pub fn preflight_union_fields(
    left: &[Field],
    right: &[Field],
    policy: &TypeCompatibilityPolicy,
) -> Result<Vec<TypeCompatibilityFinding>> {
    if left.len() != right.len() {
        return Err(DataFusionError::Plan(format!(
            "UNION field count mismatch: left={}, right={}",
            left.len(),
            right.len()
        )));
    }

    Ok(left.iter()
        .zip(right.iter())
        .enumerate()
        .map(|(idx, (l, r))| {
            compare_field(FieldCompatibilityContext {
                path: format!("column[{idx}].{}", l.name()),
                expected: l.clone(),
                actual: r.clone(),
                policy: policy.clone(),
            })
        })
        .collect())
}
```

### S5.11.2 CASE preflight

```rust id="yhb815"
use datafusion::logical_expr::ExprSchemable;
use datafusion::prelude::*;

pub fn preflight_case_arms(
    schema: &datafusion::common::DFSchema,
    arms: &[Expr],
) -> datafusion::error::Result<Vec<(DataType, bool)>> {
    arms.iter()
        .map(|e| {
            let dt = e.get_type(schema)?;
            let nullable = e.nullable(schema)?;
            Ok((dt, nullable))
        })
        .collect()
}
```

### S5.11.3 Sink preflight

```rust id="jrl746"
pub fn preflight_sink_schema(
    expected: &SchemaRef,
    actual: &SchemaRef,
    policy: &TypeCompatibilityPolicy,
) -> Result<Vec<TypeCompatibilityFinding>> {
    if expected.fields().len() != actual.fields().len() {
        return Err(DataFusionError::Plan(format!(
            "sink field count mismatch: expected={}, actual={}",
            expected.fields().len(),
            actual.fields().len()
        )));
    }

    Ok(expected.fields()
        .iter()
        .zip(actual.fields().iter())
        .map(|(e, a)| {
            compare_field(FieldCompatibilityContext {
                path: e.name().clone(),
                expected: e.as_ref().clone(),
                actual: a.as_ref().clone(),
                policy: policy.clone(),
            })
        })
        .collect())
}
```

---

## S5.12 Deployment advisory

```text id="xrbzox"
Physical/runtime:
  use RecordBatch::try_new
  require physical exact equality
  validate custom provider outputs under strict mode
  never substitute logical equality for array downcast compatibility

Planning/optimizer:
  use DFSchema helpers
  preserve aliases and qualifiers
  validate Expr get_type/nullable after rewrites
  do not rewrite expressions to schema-incompatible forms

Data ingestion:
  explicit cast plans for schema drift
  all-Utf8 staging for dirty data
  rejected-row audit for arrow_try_cast
  reject lossy numeric/time/decimal casts by default

Views/CTAS:
  aliases + explicit casts
  no SELECT *
  DESCRIBE + arrow_typeof tests
  contract fingerprint stored

APIs/sinks:
  use user-facing contract equality
  reject duplicate fields
  reject implicit type drift
  output schema manifest required
```

---

## S5.13 Anti-pattern inventory

```text id="q8v9xb"
Type compatibility anti-patterns:
  using `==` on Schema display strings
  treating DFSchema logical equality as physical equality
  treating Utf8 and Utf8View as physically interchangeable in UDFs
  treating Dictionary(Utf8) and Utf8 as same array type
  assuming SUM/AVG output type equals input type
  assuming CASE branches with mixed string/numeric are safe
  using COALESCE(amount, '0')
  narrowing Decimal scale without rounding policy
  converting Decimal money to Double
  timestamp timezone changes without explicit semantics
  accepting nullable actual field for non-null contract
  relying on Schema::try_merge as governance policy
  using RecordBatch::new_unchecked in provider code
  using `arrow_try_cast` without rejected-row audit
  allowing UNION branches to coerce invisibly in production contracts
  ignoring struct field names and comparing only positions
  treating DataFusion constraints as fully enforced database constraints
```

---

## S5.14 Agent checklist

```text id="wqmqga"
[ ] Identify comparison context:
    physical / logical / semantic / coercible / user-facing contract.

[ ] Identify operation:
    source ingestion / provider output / projection / UNION / JOIN / CASE / COALESCE / aggregate / CTAS / view / sink.

[ ] Compare field names and order:
    exact for physical/user contract unless name-based policy is explicit.

[ ] Compare nullability:
    reject nullable actual for non-null expected unless proof/filter exists.

[ ] Compare types:
    exact first.
    DFSchema logical equality second.
    semantic platform policy third.
    explicit cast fourth.
    reject otherwise.

[ ] Classify conversion:
    lossless / potentially lossy / lossy / unknown.

[ ] Generate casts:
    CAST for SQL-standard type targets.
    arrow_cast for Arrow-exact targets.
    arrow_try_cast only with rejected-row audit.

[ ] Special-case strings:
    Utf8 / Utf8View / LargeUtf8 / Dictionary strings are logical, not physical, equivalents.

[ ] Special-case decimals:
    enforce p <= 76.
    reject precision narrowing and scale narrowing by default.

[ ] Special-case timestamps:
    explicit unit/timezone.
    reject precision loss/timezone reinterpretation by default.

[ ] Special-case nested:
    recurse into list elements and struct fields.
    use name-based struct policy where DataFusion supports it.

[ ] Validate expressions:
    ExprSchemable::get_type.
    ExprSchemable::nullable.
    ExprSchemable::to_field.

[ ] Validate runtime:
    RecordBatch::try_new.
    provider stream schema equality.
    sink read-after-write schema.

[ ] Emit report:
    findings.
    proposed casts.
    lossiness.
    rejected fields.
    migration requirements.
```

## S5.15 Minimal compatibility report generator shape

```rust id="3z6nbf"
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaCompatibilityReport {
    pub report_version: String,
    pub operation: String,
    pub expected_schema_fingerprint: String,
    pub actual_schema_fingerprint: String,
    pub level_requested: CompatibilityLevel,
    pub decision: CompatibilityDecision,
    pub findings: Vec<TypeCompatibilityFinding>,
    pub proposed_projection_sql: Vec<String>,
    pub rejected_fields: Vec<String>,
}

impl SchemaCompatibilityReport {
    pub fn has_lossy_conversions(&self) -> bool {
        self.findings.iter().any(|f| {
            matches!(
                f.lossiness,
                ConversionLossiness::Lossy | ConversionLossiness::PotentiallyLossy
            )
        })
    }

    pub fn is_rejected(&self) -> bool {
        matches!(
            self.decision,
            CompatibilityDecision::Reject | CompatibilityDecision::RequiresMigration
        )
    }
}
```

## S5.16 DataFusion 54 function output-schema shape changes

Two function-level output-schema changes in DataFusion 54 break schema snapshots and downcast code that asserted the 53.x shapes:

```text
arrays_zip
  53.x: List<Struct<c0: ..., c1: ...>>   — zero-based field names c0, c1, …
  54.x: List<Struct<"1": ..., "2": ...>> — one-based field names "1", "2", …

approx_percentile_cont / approx_median
  54.x: numeric (including integer) inputs are coerced to Float64 through the
        aggregate's coercible signature, so integer inputs now produce Float64
        output where 53.x could preserve the input's numeric type.
```

The `arrays_zip` struct-field names come from the one-based element position (`Field::new(format!("{}", i + 1), …)` in `datafusion-functions-nested`), so struct-field access and `unnest` output columns change name: use `zipped_element['1']` instead of `zipped_element['c0']`.

Agent rules:

```text
Re-snapshot every golden schema that includes arrays_zip output.
Rewrite field access from ['c0'], ['c1'] to ['1'], ['2'].
Update approx_percentile_cont / approx_median output-type assertions to Float64
  for integer inputs, and re-verify sink/API contracts typed against them.
Audit UDFs and providers that downcast these outputs physically.
```

---

Operating rule: **physical exactness protects execution, logical equality protects planning, semantic equality protects platform meaning, coercion plans protect migrations, and user-facing contract equality protects downstream consumers.**

[1]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html "DFSchema in datafusion::common - Rust"
[3]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html?utm_source=chatgpt.com "RecordBatch in arrow::record_batch - Rust"
[4]: https://datafusion.apache.org/user-guide/sql/data_types.html "Data Types — Apache DataFusion  documentation"
[5]: https://datafusion.apache.org/user-guide/sql/struct_coercion.html "Struct Type Coercion and Field Mapping — Apache DataFusion  documentation"
[6]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html "Expr in datafusion::logical_expr - Rust"
[7]: https://datafusion.apache.org/library-user-guide/table-constraints.html "Table Constraint Enforcement — Apache DataFusion  documentation"


# DataFusion Advanced — S6) Schema evolution and migration lifecycle

## S6.0 Objective

Define schema evolution as a **versioned, auditable migration lifecycle**, not an incidental side effect of `CREATE VIEW`, `CTAS`, `UNION`, file append, or provider refresh.

```text id="58suln"
schema vN
  → proposed change
  → compatibility classification
  → migration artifact
  → old-data/new-query tests
  → new-data/old-query tests
  → mixed-partition tests
  → view/CTAS/sink stability tests
  → approve / stage / reject
  → publish schema vN+1
```

The attached document already frames schemas as Arrow/DFSchema/provider/catalog artifacts and calls out schema inference, explicit schemas, CTAS, views, partitioned output, custom provider schemas, and compatibility checks, but it does not yet provide a standalone schema-evolution lifecycle discipline. 

DataFusion itself is an embeddable query engine and catalog/query framework rather than a full transactional database; its DDL docs describe catalog-object creation/modification, including `CREATE SCHEMA`, `CREATE EXTERNAL TABLE`, `CREATE TABLE`, and `CREATE VIEW`, while table-constraint docs state that most constraints are informational and only field nullability is enforced during execution, not at ingestion. ([Apache DataFusion][1])

---

## S6.1 Evolution mental model

```text id="v9a3up"
SchemaEvolutionEvent
  ├─ old schema contract
  ├─ new schema contract
  ├─ change set
  ├─ compatibility direction
  ├─ migration plan
  ├─ backfill plan
  ├─ query compatibility plan
  ├─ sink/storage compatibility plan
  ├─ tests
  ├─ rollout policy
  └─ rollback policy
```

Schema evolution must distinguish:

```text id="fp4uak"
file schema
  = schema inside actual files

partition columns
  = columns derived from directory paths

table schema
  = complete schema presented to queries

logical schema
  = DFSchema after qualifiers/operators

output schema
  = sink/API/RecordBatch/Parquet/JSON contract
```

DataFusion’s 51.0 upgrade guide explicitly introduced `TableSchema` to separate file schema, partition columns, and complete table schema; the 52.0 guide then changed file-source construction so file sources receive the schema, including partition columns, at construction time. That makes schema evolution especially sensitive for custom file sources and partitioned datasets. ([Apache DataFusion][2])

---

## S6.2 Schema version model

### S6.2.1 Contract object

```rust id="76oovb"
use std::collections::HashMap;
use std::sync::Arc;
use datafusion::arrow::datatypes::SchemaRef;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaVersion {
    pub contract_name: String,
    pub version: String,
    pub previous_version: Option<String>,
    pub schema_fingerprint: String,
    pub schema_semver: SchemaSemver,
    pub owner: SchemaOwner,
    pub lifecycle_state: SchemaLifecycleState,
    pub compatibility_policy: CompatibilityPolicy,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaContract {
    pub version: SchemaVersion,
    #[serde(skip)]
    pub schema: SchemaRef,
    pub fields: Vec<FieldContract>,
    pub partition_columns: Vec<String>,
    pub views_provided: Vec<String>,
}
```

### S6.2.2 Version semantics

```rust id="nh7nrl"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaSemver {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SchemaLifecycleState {
    Draft,
    Proposed,
    Validated,
    Published,
    Deprecated,
    Retired,
    Rejected,
}
```

Recommended version semantics:

```text id="ewpl79"
PATCH:
  metadata-only, documentation, owner, description, non-contract annotation

MINOR:
  backward-compatible additive change
  add nullable column
  add nullable nested field
  widen type with no consumer break
  add compatibility view

MAJOR:
  breaking change
  drop column
  rename column
  narrow type
  tighten nullability
  change partition layout
  change semantic unit
  change timestamp timezone/precision in visible contract
```

---

## S6.3 Evolution actions

### S6.3.1 Add nullable column

```text id="z5e8uf"
old:
  stream_id Utf8 not null
  mass_flow_kg_h Float64 not null

new:
  stream_id Utf8 not null
  mass_flow_kg_h Float64 not null
  sulfur_wt_pct Float64 nullable
```

Classification:

```text id="9yed3t"
Backward compatible: usually yes
Forward compatible: old readers ignore extra field only if projection is explicit
Read compatible: yes if missing old files are null-filled or view supplies null
Write compatible: depends target provider/sink
Query compatible: SELECT * may drift; explicit projections stable
Parquet compatible: mixed-file schema requires merge/provider support
```

Migration SQL:

```sql id="x2hy5m"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  mass_flow_kg_h,
  CAST(NULL AS DOUBLE) AS sulfur_wt_pct
FROM streams_v1;
```

Agent rules:

```text id="eosctx"
Allowed only if:
  column is nullable
  default is not required for old data
  consumers do not depend on SELECT *
  output contract version increments MINOR
  mixed-file read test passes
```

Arrow note: `Schema::try_merge` can add a new field and widen nullability when schemas are compatible, including recursively for struct fields; this is useful mechanically, but it is not a governance policy by itself. ([Docs.rs][3])

---

### S6.3.2 Add non-nullable column with default

```text id="5jm5e9"
old:
  stream_id Utf8 not null

new:
  stream_id Utf8 not null
  source_system Utf8 not null default 'smartref'
```

Classification:

```text id="yxp69k"
Backward compatible: only with default/backfill
Forward compatible: no for old writers unless default inserted
Read compatible: yes through view/backfill
Write compatible: no unless writer supplies field or provider default exists
Parquet compatible: old files lack field; query layer must synthesize default
```

Migration view:

```sql id="04p19r"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  COALESCE(source_system, 'smartref') AS source_system
FROM streams_physical;
```

Backfill CTAS:

```sql id="ugyhp2"
CREATE TABLE streams_v2_backfilled AS
SELECT
  stream_id,
  'smartref' AS source_system
FROM streams_v1;
```

Agent rules:

```text id="j9rr33"
Never add non-nullable field without one of:
  physical rewrite/backfill
  logical default expression
  provider-level generated/default column
  sink-level default enforcement

Do not mark Field(nullable=false) until runtime data satisfies it.
```

DataFusion only enforces nullability during execution when returned data violates a non-nullable `Field`; it does not validate/enforce nullability at ingestion time, so migration tooling must prove or enforce non-nullability. ([Apache DataFusion][4])

---

### S6.3.3 Drop column

```text id="7udk17"
old:
  stream_id
  mass_flow_kg_h
  debug_raw_payload

new:
  stream_id
  mass_flow_kg_h
```

Classification:

```text id="w0kh9i"
Backward compatible: no for readers requiring old field
Forward compatible: yes only if old queries avoid field
Query compatible: no if any view/query references field
Parquet compatible: physical files may still contain dropped column
User contract: MAJOR
```

Safe migration pattern:

```sql id="2eg5ru"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  mass_flow_kg_h
FROM streams_v1;
```

Deprecation pattern:

```sql id="qjfzqi"
CREATE OR REPLACE VIEW streams_v1_compat AS
SELECT
  stream_id,
  mass_flow_kg_h,
  CAST(NULL AS VARCHAR) AS debug_raw_payload
FROM streams_v2;
```

Agent rules:

```text id="z82v87"
Drop in logical view first.
Track query references.
Retain compatibility view for old consumers.
Physically rewrite files only after deprecation window.
Never drop from output contract silently.
```

---

### S6.3.4 Rename column

```text id="u1ccs6"
old:
  api Float64

new:
  api_gravity Float64
```

Classification:

```text id="enatc1"
Breaking by name.
Physical type may be unchanged.
Requires compatibility alias view.
Requires query rewrite.
Usually MAJOR for public contract unless old alias retained.
```

Compatibility view:

```sql id="83f522"
CREATE OR REPLACE VIEW assays_v2 AS
SELECT
  crude_id,
  api AS api_gravity,
  sulfur_wt_pct
FROM assays_v1;
```

Reverse compatibility view:

```sql id="grmss2"
CREATE OR REPLACE VIEW assays_v1_compat AS
SELECT
  crude_id,
  api_gravity AS api,
  sulfur_wt_pct
FROM assays_v2;
```

Agent rules:

```text id="nk1zn5"
Rename = drop + add from consumer perspective.
Do not infer rename solely from matching type/position.
Require rename mapping artifact:
  old_name → new_name
  semantic reason
  compatibility alias duration
```

---

### S6.3.5 Reorder columns

```text id="v53112"
old order:
  stream_id, unit_id, mass_flow_kg_h

new order:
  unit_id, stream_id, mass_flow_kg_h
```

Classification:

```text id="z8m0dg"
Name-based readers: potentially compatible
Position-based readers: breaking
RecordBatch physical arrays: order is semantic
Parquet/Arrow output: order may matter to downstream tools/tests
User contract: breaking unless explicitly name-based
```

Safe projection:

```sql id="wjlvp5"
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM streams_new_physical_order;
```

Agent rules:

```text id="x9wrur"
Never reorder physical/output columns without explicit mapping.
Schema fingerprint must include field order.
For public output, treat reorder as breaking unless contract says name-based only.
```

Arrow `Schema` is an ordered sequence of fields and `RecordBatch` arrays are position-aligned with schema fields; `RecordBatch::try_new` validates the field/array correspondence by index. ([Docs.rs][3])

---

### S6.3.6 Widen type

Examples:

```text id="li40z1"
Int32 → Int64
Float32 → Float64
Decimal128(18,4) → Decimal128(20,4)
Timestamp(ms) → Timestamp(ns)
Utf8 → LargeUtf8
```

Classification:

```text id="rxld72"
Backward compatible: often yes
Forward compatible: old readers may fail if physical type changes
Read compatible: yes with cast
Write compatible: writers must update target type
Physical exact: false
Logical/semantic: policy-dependent
```

Migration projection:

```sql id="v8a538"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  CAST(volume_bbl AS BIGINT) AS volume_bbl
FROM streams_v1;
```

Agent rules:

```text id="gg2gul"
Allow only if lossless.
Record explicit cast.
Test arrow_typeof.
Update sink schema.
For Decimal, widening precision is not same as changing scale.
```

---

### S6.3.7 Narrow type

Examples:

```text id="bwa747"
Int64 → Int32
Float64 → Float32
Decimal128(20,4) → Decimal128(18,2)
Timestamp(ns) → Timestamp(ms)
LargeUtf8 → Utf8
```

Classification:

```text id="r9uieh"
Backward compatible: no
Forward compatible: no unless all values provably fit
Read compatible: only with validation
Write compatible: risky
User contract: MAJOR
```

Validation query:

```sql id="il6d82"
SELECT
  COUNT(*) AS bad_rows
FROM streams
WHERE volume_bbl > 2147483647
   OR volume_bbl < -2147483648;
```

Migration projection:

```sql id="vbn8nv"
SELECT
  CAST(volume_bbl AS INTEGER) AS volume_bbl_i32
FROM streams
WHERE volume_bbl BETWEEN -2147483648 AND 2147483647;
```

Agent rules:

```text id="6800b1"
Narrowing is rejected by default.
Require value-range proof.
Require rejected-row query.
Require lossy flag if precision/scale/time truncation can occur.
```

---

### S6.3.8 Change nullability

#### Non-nullable → nullable

```text id="dmbi1z"
Backward compatible: usually yes for readers
Contract weakening: yes
Physical exact: false
```

Policy:

```text id="t2cpo2"
Allow with warning.
Do not propagate nullable=false assumptions into UDFs/operators.
Update tests for NULL input.
```

#### Nullable → non-nullable

```text id="uyec0m"
Backward compatible: no unless old data proven non-null
Write compatible: no unless writers enforce
Runtime risk: DataFusion errors if non-nullable field returns null
```

Proof query:

```sql id="0flgb3"
SELECT COUNT(*) AS null_count
FROM streams
WHERE stream_id IS NULL;
```

Hardening view:

```sql id="yt0v1v"
CREATE OR REPLACE VIEW streams_non_null AS
SELECT *
FROM streams
WHERE stream_id IS NOT NULL;
```

Agent rules:

```text id="puwa8g"
Never silently tighten nullability.
Require:
  full-data validation
  ingestion enforcement
  rejected-row handling
  version bump
  non-null test fixture
```

---

### S6.3.9 Change metadata

Metadata examples:

```text id="aev0a8"
semantic.unit: kg/h → t/d
semantic.type: mass_flow → volume_flow
source.name: "Mass Flow" → "Mass Rate"
governance.classification: internal → confidential
schema.contract.version
```

Classification:

```text id="h293v2"
Advisory metadata:
  PATCH or warning

Contract metadata:
  MINOR/MAJOR depending semantics

Unit change:
  semantic breaking unless values converted

Classification change:
  governance-impacting; may require access-control migration
```

Value conversion:

```sql id="xoeniy"
SELECT
  mass_flow_kg_h / 1000.0 * 24.0 AS mass_flow_t_d
FROM streams;
```

Agent rules:

```text id="lhv8u2"
Do not treat metadata drift as harmless by default.
Maintain metadata allowlist:
  advisory keys
  contract keys
  governance keys
  lineage keys
```

---

### S6.3.10 Evolve nested struct field

```text id="jus2jy"
old:
  assay: Struct<
    api_gravity: Float64?,
    sulfur_wt_pct: Float64?
  >

new:
  assay: Struct<
    api_gravity: Float64?,
    sulfur_wt_pct: Float64?,
    nitrogen_wt_pct: Float64?
  >
```

Classification:

```text id="5n7jc0"
Add nullable nested field: usually backward compatible with explicit access
Drop nested field: breaking
Rename nested field: breaking
Reorder nested fields: DataFusion may support name-based logical struct coercion, but physical/output contracts still need policy
```

Migration projection:

```sql id="j3x0le"
SELECT
  crude_id,
  named_struct(
    'api_gravity', assay['api_gravity'],
    'sulfur_wt_pct', assay['sulfur_wt_pct'],
    'nitrogen_wt_pct', CAST(NULL AS DOUBLE)
  ) AS assay
FROM assays_v1;
```

Agent rules:

```text id="1fvnw2"
Nested evolution requires path-level diff:
  assay.api_gravity
  assay.sulfur_wt_pct
  assay.nitrogen_wt_pct

Do not compare only top-level DataType::Struct.
```

---

### S6.3.11 Evolve list element type

```text id="ojqaoj"
old:
  tags: List<Utf8?>
new:
  tags: List<Utf8View?>

old:
  embedding: List<Float32>
new:
  embedding: List<Float64>
```

Classification:

```text id="0iz2lm"
Parent list nullability change ≠ element nullability change.
Element widening may be allowed.
Element narrowing rejected.
Changing homogeneous element type can break vector functions/UDFs.
```

Migration projection:

```sql id="40xoy9"
SELECT
  array_transform(embedding, x -> CAST(x AS DOUBLE)) AS embedding
FROM vectors;
```

DataFusion 54 ships SQL lambda syntax (`x -> expr`) and higher-order functions (`array_transform`, `array_filter`, `array_any_match`) as first-class features, so this projection works as written on the pinned version. The higher-order UDF and lambda deep dive (implementation traits, registry methods, planning caveats) lives in `datafusion_calculations_rust.md`.

Agent rules:

```text id="vzdz04"
List evolution diff includes:
  list nullable
  item nullable
  item data type
  fixed-size length if fixed-size list
  nested item struct fields
```

---

### S6.3.12 Evolve partition columns

Old:

```text id="w55hpd"
s3://lake/streams/event_date=2026-05-24/site=baytown/file.parquet
```

New:

```text id="jbjbeu"
s3://lake/streams/event_date=2026-05-24/site=baytown/unit_id=CDU/file.parquet
```

Classification:

```text id="vnnqrh"
Partition layout change: usually MAJOR
Query pruning behavior changes
Table schema changes
Directory compatibility risk
Mixed old/new layout risk
File schema vs table schema separation required
```

DataFusion supports Hive-style partition columns in file/directory sources, and recent upgrade guides emphasize explicit handling of partition columns as part of table schema rather than file schema alone. ([Apache DataFusion][2])

Migration options:

```text id="kyq8xo"
Option A — new table root:
  streams_by_unit_v2/
    event_date=.../site=.../unit_id=.../

Option B — compatibility view:
  old table + new table UNION ALL with explicit unit_id default/null

Option C — full rewrite:
  rewrite all old data into new partition layout

Option D — metadata/catalog indirection:
  register separate physical roots behind one logical provider
```

Compatibility union view:

```sql id="4tskiz"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  event_date,
  site,
  CAST(NULL AS VARCHAR) AS unit_id,
  stream_id,
  mass_flow_kg_h
FROM streams_v1_old_layout

UNION ALL

SELECT
  event_date,
  site,
  unit_id,
  stream_id,
  mass_flow_kg_h
FROM streams_v2_new_layout;
```

Agent rules:

```text id="lpfsx8"
Never mix partition layouts under one unmanaged directory root.
Prefer new table root for partition evolution.
Keep partition columns low/moderate cardinality.
Test pruning and DESCRIBE output after migration.
```

---

## S6.4 Compatibility policy dimensions

### S6.4.1 Backward compatibility

```text id="di2kyb"
New schema can be read by old readers/queries.
```

Usually safe:

```text id="fr5s17"
add nullable field if old query projects explicit columns
add metadata-only non-contract key
widen type if old query casts/accepts
```

Usually unsafe:

```text id="4q3v8n"
rename field
drop field
change field type physically
tighten nullability
change partition layout
change output order for position-based consumers
```

### S6.4.2 Forward compatibility

```text id="4xv9e0"
Old data can be read by new readers/queries.
```

Requires:

```text id="1dnge1"
defaults for new non-null columns
NULL fill for new nullable columns
compatibility views over old physical data
provider-level schema adaptation
```

### S6.4.3 Read compatibility

```text id="zgmlq3"
All existing physical data can be queried under the new logical schema.
```

Test:

```sql id="37b8z9"
SELECT COUNT(*)
FROM new_view_over_old_and_new_data;
```

### S6.4.4 Write compatibility

```text id="q7vdpn"
Existing writers can append/insert into the new target schema.
```

Risk:

```text id="1bk0vl"
new non-null column
new partition column
new decimal scale
new timestamp timezone
provider insert_into rejects missing columns
```

### S6.4.5 Query compatibility

```text id="xx31op"
Existing SQL/DataFrame plans still bind, plan, and return expected schema.
```

Test:

```text id="7muoyt"
old query against new schema
new query against old data
old view over new table
new view over mixed old/new data
```

### S6.4.6 Parquet/Arrow compatibility

```text id="ihmif6"
Physical files remain readable as a coherent table schema.
```

Risk:

```text id="muywug"
mixed file schemas
partition-column changes
timestamp unit drift
decimal precision/scale drift
nested struct/list evolution
field metadata mismatch
```

Arrow’s `Schema::try_merge` can mechanically merge compatible schemas and recursively merge struct fields, but migration policy must still decide whether the resulting merged schema is valid for the table contract. ([Docs.rs][3])

---

## S6.5 Migration artifact model

```rust id="wkwc3s"
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaMigration {
    pub migration_id: String,
    pub contract_name: String,
    pub from_version: String,
    pub to_version: String,
    pub action_set: Vec<SchemaEvolutionAction>,
    pub compatibility_report: CompatibilityReport,
    pub migration_plan: MigrationPlan,
    pub validation_plan: ValidationPlan,
    pub rollout_plan: RolloutPlan,
    pub rollback_plan: RollbackPlan,
    pub status: MigrationStatus,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MigrationStatus {
    Draft,
    Proposed,
    Validated,
    Approved,
    Applied,
    RolledBack,
    Rejected,
}
```

### S6.5.1 Evolution action enum

```rust id="vh1t31"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SchemaEvolutionAction {
    AddColumn {
        path: String,
        data_type: String,
        nullable: bool,
        default_expr: Option<String>,
    },
    DropColumn {
        path: String,
        compatibility_alias: Option<String>,
    },
    RenameColumn {
        from: String,
        to: String,
    },
    ReorderColumns {
        mapping: Vec<(String, usize)>,
    },
    WidenType {
        path: String,
        from_type: String,
        to_type: String,
        cast_expr: String,
    },
    NarrowType {
        path: String,
        from_type: String,
        to_type: String,
        validation_expr: String,
        cast_expr: String,
    },
    ChangeNullability {
        path: String,
        from_nullable: bool,
        to_nullable: bool,
        proof_query: Option<String>,
    },
    ChangeMetadata {
        path: String,
        key: String,
        from_value: Option<String>,
        to_value: Option<String>,
    },
    EvolveNestedField {
        path: String,
        nested_action: Box<SchemaEvolutionAction>,
    },
    EvolvePartitionColumns {
        old_partition_cols: Vec<String>,
        new_partition_cols: Vec<String>,
        layout_plan: PartitionMigrationPlan,
    },
}
```

### S6.5.2 Compatibility report

```rust id="mjobfs"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompatibilityReport {
    pub backward_compatible: bool,
    pub forward_compatible: bool,
    pub read_compatible: bool,
    pub write_compatible: bool,
    pub query_compatible: bool,
    pub parquet_arrow_compatible: bool,
    pub breaking_changes: Vec<BreakingChange>,
    pub warnings: Vec<CompatibilityWarning>,
    pub decision: MigrationDecision,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MigrationDecision {
    Accept,
    AcceptWithCompatibilityView,
    RequiresBackfill,
    RequiresNewTableRoot,
    Reject,
}
```

### S6.5.3 Migration plan

```rust id="3iw29w"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationPlan {
    pub ddl: Vec<String>,
    pub compatibility_views: Vec<String>,
    pub backfill_queries: Vec<String>,
    pub validation_queries: Vec<String>,
    pub data_rewrite_steps: Vec<DataRewriteStep>,
    pub provider_changes: Vec<String>,
    pub sink_changes: Vec<String>,
}
```

### S6.5.4 Backfill expression

```rust id="c0q6v1"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackfillExpression {
    pub target_field: String,
    pub expr_sql: String,
    pub nullable_after_backfill: bool,
    pub rejected_rows_query: Option<String>,
    pub proof_query: Option<String>,
}
```

Example:

```json id="ehop2g"
{
  "target_field": "source_system",
  "expr_sql": "'smartref'",
  "nullable_after_backfill": false,
  "proof_query": "SELECT COUNT(*) FROM streams_v2 WHERE source_system IS NULL"
}
```

### S6.5.5 Rejected migration reason

```rust id="n93m9z"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RejectedMigrationReason {
    pub code: String,
    pub path: String,
    pub message: String,
    pub suggested_safe_pattern: Option<String>,
}
```

Example:

```json id="2ljddi"
{
  "code": "nullable_to_non_nullable_without_proof",
  "path": "stream_id",
  "message": "Cannot tighten nullability without full-data validation and write-path enforcement",
  "suggested_safe_pattern": "Create filtered compatibility view, run null-count proof query, then publish non-nullable contract"
}
```

---

## S6.6 SQL migration patterns

### S6.6.1 Add nullable column via view

```sql id="zrkn1i"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  CAST(NULL AS DOUBLE) AS sulfur_wt_pct
FROM streams_v1;
```

### S6.6.2 Add non-nullable default via CTAS

```sql id="fj2npu"
CREATE TABLE streams_v2_backfilled AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  'smartref' AS source_system
FROM streams_v1;
```

### S6.6.3 Rename with compatibility alias

```sql id="1w56j3"
CREATE OR REPLACE VIEW assays_v2 AS
SELECT
  crude_id,
  api AS api_gravity,
  sulfur_wt_pct
FROM assays_v1;

CREATE OR REPLACE VIEW assays_v1_compat AS
SELECT
  crude_id,
  api_gravity AS api,
  sulfur_wt_pct
FROM assays_v2;
```

### S6.6.4 Widen type

```sql id="3it4ac"
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  CAST(volume_bbl AS BIGINT) AS volume_bbl
FROM streams_v1;
```

### S6.6.5 Narrow type with validation

```sql id="xye24y"
-- proof / rejection query
SELECT
  COUNT(*) AS out_of_range_rows
FROM streams_v1
WHERE volume_bbl < -2147483648
   OR volume_bbl > 2147483647;

-- migration only after proof is zero
CREATE OR REPLACE VIEW streams_v2 AS
SELECT
  stream_id,
  CAST(volume_bbl AS INTEGER) AS volume_bbl
FROM streams_v1;
```

### S6.6.6 Partition evolution via new root

```sql id="yqel9s"
CREATE EXTERNAL TABLE streams_v2
STORED AS PARQUET
PARTITIONED BY (event_date, site, unit_id)
LOCATION 's3://lake/streams_v2/';
```

DataFusion DDL supports `CREATE EXTERNAL TABLE ... PARTITIONED BY (...)` and `LOCATION` can point to a file or directory of partitioned files; this makes new-root partition evolution clearer than silently mixing old/new partition layouts under one prefix. ([Apache DataFusion][1])

---

## S6.7 Rust migration planning types

```rust id="wzj2ih"
#[derive(Debug, Clone)]
pub struct SchemaEvolutionPolicy {
    pub allow_add_nullable_column: bool,
    pub allow_add_non_nullable_with_default: bool,
    pub allow_drop_with_compat_view: bool,
    pub allow_rename_with_alias_view: bool,
    pub allow_reorder_output_columns: bool,
    pub allow_lossless_type_widening: bool,
    pub allow_lossy_type_narrowing: bool,
    pub allow_nullable_to_non_nullable_with_proof: bool,
    pub allow_metadata_only_patch: bool,
    pub allow_nested_add_nullable_field: bool,
    pub require_new_root_for_partition_change: bool,
}

impl Default for SchemaEvolutionPolicy {
    fn default() -> Self {
        Self {
            allow_add_nullable_column: true,
            allow_add_non_nullable_with_default: true,
            allow_drop_with_compat_view: true,
            allow_rename_with_alias_view: true,
            allow_reorder_output_columns: false,
            allow_lossless_type_widening: true,
            allow_lossy_type_narrowing: false,
            allow_nullable_to_non_nullable_with_proof: true,
            allow_metadata_only_patch: true,
            allow_nested_add_nullable_field: true,
            require_new_root_for_partition_change: true,
        }
    }
}
```

Diff planner skeleton:

```rust id="s67k6f"
pub fn plan_schema_migration(
    old: &SchemaContract,
    new: &SchemaContract,
    policy: &SchemaEvolutionPolicy,
) -> SchemaMigration {
    let actions = diff_schema_contracts(old, new);
    let compatibility_report = classify_compatibility(&actions, policy);
    let migration_plan = build_migration_plan(&actions, &compatibility_report);

    SchemaMigration {
        migration_id: format!("{}-{}-to-{}", old.version.contract_name, old.version.version, new.version.version),
        contract_name: old.version.contract_name.clone(),
        from_version: old.version.version.clone(),
        to_version: new.version.version.clone(),
        action_set: actions,
        compatibility_report,
        migration_plan,
        validation_plan: build_validation_plan(old, new),
        rollout_plan: RolloutPlan::default(),
        rollback_plan: RollbackPlan::default(),
        status: MigrationStatus::Draft,
    }
}
```

---

## S6.8 Testing matrix

### S6.8.1 Old data with new schema

```text id="ogkbvb"
Goal:
  new contract can read all historical data.
```

SQL:

```sql id="1u3i0v"
SELECT COUNT(*) AS n
FROM streams_v2_over_old_data;
```

Validation:

```sql id="p0fjfg"
SELECT
  arrow_typeof(stream_id) AS stream_id_type,
  arrow_typeof(source_system) AS source_system_type
FROM streams_v2_over_old_data
LIMIT 1;
```

### S6.8.2 New data with old query

```text id="ysucb8"
Goal:
  old explicit projections still work after additive schema change.
```

Old query:

```sql id="ll2dff"
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM streams_v2;
```

Reject if old query used:

```sql id="kwk4su"
SELECT *
FROM streams_v2;
```

### S6.8.3 Mixed-partition schema

```text id="7fmwur"
Goal:
  table root with old/new files or partitions has coherent table schema.
```

Tests:

```sql id="2r2qf6"
DESCRIBE streams_mixed;

SELECT
  event_date,
  site,
  COUNT(*) AS row_count
FROM streams_mixed
GROUP BY event_date, site
ORDER BY event_date, site;
```

Assertions:

```text id="vxg7cf"
all partition columns present
old partitions fill new nullable columns with NULL/default
no partition/file column conflict
no mixed physical type conflict
```

### S6.8.4 Old view over new table

```sql id="287d00"
CREATE OR REPLACE VIEW streams_v1_compat AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM streams_v2;
```

Test:

```sql id="5z0dex"
DESCRIBE streams_v1_compat;

SELECT COUNT(*)
FROM streams_v1_compat;
```

### S6.8.5 CTAS output stability

DataFrame objects are lazy and build logical plans until an execution method such as `collect` runs; CTAS/view migration tests must therefore inspect schema at planning and output boundaries, not only after data movement. ([Docs.rs][5])

CTAS test:

```sql id="gfcrcp"
CREATE TABLE streams_v2_ctas AS
SELECT
  stream_id,
  unit_id,
  CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h,
  CAST(NULL AS DOUBLE) AS sulfur_wt_pct
FROM streams_v1;

DESCRIBE streams_v2_ctas;
```

---

## S6.9 Migration validation artifact

```rust id="uukudy"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationPlan {
    pub old_data_new_schema_queries: Vec<String>,
    pub new_data_old_query_queries: Vec<String>,
    pub mixed_partition_queries: Vec<String>,
    pub view_compatibility_queries: Vec<String>,
    pub ctas_output_schema_queries: Vec<String>,
    pub negative_tests: Vec<NegativeMigrationTest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NegativeMigrationTest {
    pub name: String,
    pub setup_sql: Vec<String>,
    pub query_sql: String,
    pub expected_error_class: String,
}
```

Example validation plan:

```json id="c1y2d3"
{
  "old_data_new_schema_queries": [
    "SELECT COUNT(*) FROM streams_v2_over_old_data",
    "SELECT COUNT(*) FROM streams_v2_over_old_data WHERE source_system IS NULL"
  ],
  "new_data_old_query_queries": [
    "SELECT stream_id, unit_id, mass_flow_kg_h FROM streams_v2"
  ],
  "mixed_partition_queries": [
    "DESCRIBE streams_mixed",
    "SELECT event_date, site, COUNT(*) FROM streams_mixed GROUP BY event_date, site"
  ],
  "ctas_output_schema_queries": [
    "DESCRIBE streams_v2_ctas"
  ]
}
```

---

## S6.10 Read/write rollout patterns

### S6.10.1 Compatibility view first

```text id="pw1q77"
1. Publish new physical table or provider schema.
2. Publish old compatibility view.
3. Run old query suite against compatibility view.
4. Run new query suite against new view/table.
5. Shift readers.
6. Deprecate old view.
```

### S6.10.2 Dual-write

```text id="649jz0"
1. Existing writers write v1.
2. New writers write v2.
3. Compatibility view unions v1/v2.
4. Validate row-count parity.
5. Migrate old data.
6. Stop v1 writes.
```

### S6.10.3 New-root rewrite

```text id="8kxftv"
1. Read old table root.
2. Apply migration projection.
3. Write new table root.
4. Read-after-write DESCRIBE.
5. Compare row count and key counts.
6. Register new external table.
7. Switch catalog binding or view.
```

### S6.10.4 Catalog swap

```text id="totms7"
1. Register streams_v2 under new name.
2. Validate.
3. Replace view streams_current to point at v2.
4. Keep streams_v1_compat.
5. Retire old physical table only after retention.
```

---

## S6.11 DataFusion-specific caveats

```text id="urlci8"
DataFusion is not a transactional lakehouse table format by itself.
Schema evolution across files, object-store prefixes, or custom providers is application/provider/storage responsibility.
DDL registration does not imply durable metastore persistence unless the catalog implementation persists metadata.
DROP TABLE removes catalog registration; external files are separate storage objects.
CTAS default behavior may create in-memory tables unless custom catalog/provider semantics change it.
```

The attached DDL section notes that default `CREATE TABLE` is an in-memory table unless custom catalog/provider behavior changes semantics, and that `DROP TABLE` removes catalog registration rather than deleting external files. 

DataFusion’s DDL docs describe `CREATE EXTERNAL TABLE` as registering a local or remote location as a named table, while the catalog guide describes built-in in-memory catalogs and custom catalog/provider extension points; use a durable catalog or table format if persistent schema evolution is required. ([Apache DataFusion][1])

---

## S6.12 Migration diagnostics payloads

### S6.12.1 Breaking rename

```json id="zgc0ap"
{
  "error_class": "breaking_schema_rename",
  "migration_id": "assays-1-to-2",
  "field_path": "api",
  "proposed_new_path": "api_gravity",
  "backward_compatible": false,
  "decision": "requires_compatibility_view",
  "suggested_view_sql": "SELECT api_gravity AS api FROM assays_v2"
}
```

### S6.12.2 Unsafe nullability tightening

```json id="8kjrxb"
{
  "error_class": "unsafe_nullability_tightening",
  "field_path": "stream_id",
  "from_nullable": true,
  "to_nullable": false,
  "decision": "reject",
  "required_proof_query": "SELECT COUNT(*) FROM streams WHERE stream_id IS NULL",
  "required_write_enforcement": "provider or ingestion must reject null stream_id"
}
```

### S6.12.3 Partition evolution rejected

```json id="p7842q"
{
  "error_class": "partition_layout_change_requires_new_root",
  "old_partition_columns": ["event_date", "site"],
  "new_partition_columns": ["event_date", "site", "unit_id"],
  "decision": "requires_new_table_root",
  "suggested_root": "s3://lake/streams_v2/"
}
```

### S6.12.4 Lossy narrowing rejected

```json id="f0h3ki"
{
  "error_class": "lossy_type_narrowing",
  "field_path": "volume_bbl",
  "from_type": "Int64",
  "to_type": "Int32",
  "decision": "reject",
  "required_validation": "range proof + rejected-row query",
  "suggested_fix": "keep Int64 or create volume_bbl_i32 after validation"
}
```

---

## S6.13 CI / regression harness

```text id="9pxehd"
schema-migration-test
  ├─ compile schemas
  ├─ diff old/new contracts
  ├─ generate compatibility report
  ├─ generate SQL migration plan
  ├─ run old data/new schema tests
  ├─ run new data/old query tests
  ├─ run mixed partition tests
  ├─ run view compatibility tests
  ├─ run CTAS output schema tests
  ├─ run negative drift tests
  └─ emit migration approval artifact
```

Example command layout:

```bash id="c0ogx6"
cargo test -p query-core schema_migration_add_nullable_column
cargo test -p query-core schema_migration_reject_nullable_tightening
cargo test -p query-core schema_migration_partition_requires_new_root
```

Rust test helper:

```rust id="p7vvlh"
pub async fn assert_query_schema(
    ctx: &SessionContext,
    sql: &str,
    expected: &datafusion::arrow::datatypes::Schema,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let actual = df.schema().as_arrow();

    assert_eq!(
        expected.fields(),
        actual.fields(),
        "query schema mismatch for SQL:\n{sql}"
    );

    Ok(())
}
```

---

## S6.14 Migration storage artifact

```json id="5yymsb"
{
  "migration_id": "streams-1.2.0-to-2.0.0",
  "contract_name": "refinery.streams",
  "from_version": "1.2.0",
  "to_version": "2.0.0",
  "created_at": "2026-05-24T00:00:00Z",
  "actions": [
    {
      "kind": "RenameColumn",
      "from": "api",
      "to": "api_gravity"
    },
    {
      "kind": "AddColumn",
      "path": "source_system",
      "data_type": "Utf8",
      "nullable": false,
      "default_expr": "'smartref'"
    }
  ],
  "compatibility": {
    "backward_compatible": false,
    "forward_compatible": true,
    "read_compatible": true,
    "write_compatible": false,
    "query_compatible": false,
    "parquet_arrow_compatible": true,
    "decision": "AcceptWithCompatibilityView"
  },
  "views": {
    "new": "CREATE OR REPLACE VIEW streams_v2 AS ...",
    "compat": "CREATE OR REPLACE VIEW streams_v1_compat AS ..."
  },
  "validation": {
    "required_queries": [
      "DESCRIBE streams_v2",
      "SELECT COUNT(*) FROM streams_v2 WHERE source_system IS NULL"
    ]
  }
}
```

---

## S6.15 Best-practice deployment advisory

```text id="18j6s5"
Schema evolution:
  version every schema contract
  generate diff before migration
  classify compatibility by direction
  never rely on type/name similarity alone
  create compatibility views for consumers
  use new physical roots for partition layout changes
  snapshot DESCRIBE / df.schema / arrow_typeof outputs
  preserve old views during deprecation window
  require explicit approval for breaking changes
```

```text id="qk5d6y"
DataFusion-specific:
  DDL mutates catalog objects, not necessarily durable storage
  CTAS default table semantics depend on catalog/provider
  external-table registration does not delete/rewrite files
  custom providers must implement write/default/constraint behavior
  nullability can produce runtime errors but is not ingestion-enforced
```

```text id="tzz9yo"
File/lakehouse:
  do not mix old/new partition layouts under same root
  avoid SELECT * in views over evolving tables
  write schema manifest beside outputs
  run read-after-write schema validation
  keep file schema, partition schema, and table schema separate
```

---

## S6.16 Anti-pattern inventory

```text id="t4lg49"
Schema evolution anti-patterns:
  treating add non-null column as safe without default/backfill
  treating rename as metadata-only change
  dropping columns without compatibility view
  using SELECT * in stable views over evolving tables
  reordering physical output columns silently
  narrowing types without proof/rejected-row audit
  tightening nullability without full-data validation
  changing decimal scale without rounding policy
  changing timestamp timezone/precision without explicit semantics
  treating metadata unit changes as harmless
  comparing only top-level struct fields
  changing list element type without nested diff
  mixing partition layouts under one object-store prefix
  assuming DataFusion enforces primary/unique/check constraints
  assuming DDL persistence in default in-memory catalog
  assuming DROP TABLE deletes external files
  migrating schema without old-query/new-data and new-query/old-data tests
  publishing migration without compatibility report
```

---

## S6.17 Agent checklist

```text id="f9a706"
[ ] Load old SchemaContract and proposed new SchemaContract.
[ ] Compute field-level and nested path-level diff.
[ ] Classify each action:
    add nullable
    add non-nullable with default
    drop
    rename
    reorder
    widen
    narrow
    nullability change
    metadata change
    nested evolution
    partition evolution

[ ] Classify compatibility:
    backward
    forward
    read
    write
    query
    Parquet/Arrow

[ ] Reject unsafe changes by default:
    nullable → non-nullable without proof
    narrowing without proof
    drop without compatibility view
    rename without mapping
    partition layout change without new root

[ ] Generate migration artifacts:
    schema version
    compatibility report
    SQL migration plan
    backfill expressions
    rejected-row queries
    compatibility views
    rollback plan

[ ] Test:
    old data with new schema
    new data with old query
    mixed partitions
    old view over new table
    CTAS output schema
    sink read-after-write schema

[ ] Publish:
    new schema fingerprint
    migration artifact
    view/table registration changes
    deprecation notice for old contract
```

## S6.18 Minimal schema migration planner skeleton

```rust id="50us6b"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MigrationFinding {
    pub path: String,
    pub action: String,
    pub compatibility: CompatibilityReport,
    pub proposed_sql: Vec<String>,
    pub required_tests: Vec<String>,
    pub decision: MigrationDecision,
}

pub fn classify_add_column(
    path: &str,
    data_type: &str,
    nullable: bool,
    default_expr: Option<&str>,
) -> MigrationFinding {
    if nullable {
        MigrationFinding {
            path: path.to_string(),
            action: "add_nullable_column".to_string(),
            compatibility: CompatibilityReport {
                backward_compatible: true,
                forward_compatible: true,
                read_compatible: true,
                write_compatible: true,
                query_compatible: true,
                parquet_arrow_compatible: true,
                breaking_changes: vec![],
                warnings: vec![],
                decision: MigrationDecision::Accept,
            },
            proposed_sql: vec![format!("CAST(NULL AS {data_type}) AS {path}")],
            required_tests: vec![
                format!("DESCRIBE migrated_view"),
                format!("SELECT COUNT(*) FROM migrated_view"),
            ],
            decision: MigrationDecision::Accept,
        }
    } else if let Some(expr) = default_expr {
        MigrationFinding {
            path: path.to_string(),
            action: "add_non_nullable_column_with_default".to_string(),
            compatibility: CompatibilityReport {
                backward_compatible: true,
                forward_compatible: true,
                read_compatible: true,
                write_compatible: false,
                query_compatible: true,
                parquet_arrow_compatible: true,
                breaking_changes: vec![],
                warnings: vec![],
                decision: MigrationDecision::RequiresBackfill,
            },
            proposed_sql: vec![format!("{expr} AS {path}")],
            required_tests: vec![
                format!("SELECT COUNT(*) FROM migrated_view WHERE {path} IS NULL"),
            ],
            decision: MigrationDecision::RequiresBackfill,
        }
    } else {
        MigrationFinding {
            path: path.to_string(),
            action: "add_non_nullable_column_without_default".to_string(),
            compatibility: CompatibilityReport {
                backward_compatible: false,
                forward_compatible: false,
                read_compatible: false,
                write_compatible: false,
                query_compatible: false,
                parquet_arrow_compatible: false,
                breaking_changes: vec![],
                warnings: vec![],
                decision: MigrationDecision::Reject,
            },
            proposed_sql: vec![],
            required_tests: vec![],
            decision: MigrationDecision::Reject,
        }
    }
}
```

Core operating rule: **schema evolution is safe only when the change is explicitly classified, versioned, migrated, validated against old and new data/query directions, and published with compatibility artifacts.**

[1]: https://datafusion.apache.org/user-guide/sql/ddl.html "DDL — Apache DataFusion  documentation"
[2]: https://datafusion.apache.org/library-user-guide/upgrading/51.0.0.html "Upgrade Guides — Apache DataFusion  documentation"
[3]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[4]: https://datafusion.apache.org/library-user-guide/table-constraints.html "Table Constraint Enforcement — Apache DataFusion  documentation"
[5]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html "DataFrame in datafusion::dataframe - Rust"


# DataFusion Advanced — S7) Schema metadata, Arrow extension types, and semantic annotations

## S7.0 Objective

Clarify which metadata is:

```text id="d1nbjr"
stored
preserved
ignored
propagated
overwritten
stripped
written to sinks
read from sources
available to SQL
consumed by custom code
```

Metadata is not a substitute for `DataType`, `Field::nullable`, `DFSchema` qualifiers, constraints, or runtime validation. It is an **annotation channel**: powerful for semantic contracts, lineage, governance, display, units, extension types, and audit, but dangerous if agents assume that arbitrary metadata changes DataFusion optimizer/runtime behavior.

The attached document already covers `Field::with_metadata`, `Schema::new_with_metadata`, expression alias metadata, Parquet Arrow metadata, `metadata::key_name`, and the warning that schema metadata should not be assumed to affect DataFusion optimization unless a feature explicitly consumes it. 

---

## S7.1 Metadata mental model

```text id="9a7oee"
MetadataSurface
  ├─ Arrow Schema metadata
  │   └─ dataset / contract / lineage annotations
  │
  ├─ Arrow Field metadata
  │   ├─ semantic annotations
  │   ├─ Arrow extension type keys
  │   └─ source lineage / governance / display hints
  │
  ├─ DataFusion Expr alias metadata
  │   └─ metadata attached to projected expression fields via alias_with_metadata
  │
  ├─ SQL expression metadata functions
  │   ├─ with_metadata(expr, key, value, ...)
  │   ├─ arrow_metadata(expr[, key])
  │   └─ arrow_field(expr)
  │
  ├─ Parquet file key-value metadata
  │   ├─ ARROW:schema metadata
  │   ├─ created_by
  │   └─ metadata::custom_key
  │
  ├─ Arrow IPC metadata
  │   └─ Arrow schema / field metadata serialized with IPC schema
  │
  └─ Platform metadata registry
      ├─ enforced keys
      ├─ advisory keys
      ├─ governance keys
      └─ lineage keys
```

Arrow `Field` contains `name`, `data_type`, `nullable`, and `metadata`, and Arrow extension types are encoded in `Field` metadata; Arrow docs point to `Field::try_extension_type` to retrieve an extension type if present. ([Apache Arrow][1]) Arrow `Schema` contains ordered fields and schema-level key-value metadata, and `Schema::new_with_metadata` attaches schema-level metadata. `Schema::project` carries parent schema metadata into the projected schema. ([Docs.rs][2])

---

## S7.2 Metadata locations

### S7.2.1 Schema metadata

Use for dataset-level / table-level / contract-level annotations.

```rust id="oaf5qy"
use std::collections::HashMap;
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};

pub fn stream_schema_with_contract_metadata() -> SchemaRef {
    let mut metadata = HashMap::new();

    metadata.insert("schema.contract.name".to_string(), "refinery.streams".to_string());
    metadata.insert("schema.contract.version".to_string(), "1.2.0".to_string());
    metadata.insert("schema.contract.fingerprint".to_string(), "sha256:...".to_string());
    metadata.insert("lineage.producer".to_string(), "smartref-sim-engine".to_string());
    metadata.insert("lineage.created_by".to_string(), "datafusion-pipeline".to_string());

    Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("stream_id", DataType::Utf8, false),
            Field::new("mass_flow_kg_h", DataType::Float64, false),
        ],
        metadata,
    ))
}
```

Recommended schema-level keys:

```text id="y0ynrg"
schema.contract.name
schema.contract.version
schema.contract.fingerprint
schema.owner
schema.lifecycle_state
lineage.producer
lineage.source_system
lineage.source_uri
lineage.created_at
governance.domain
governance.retention_policy
```

Do not store:

```text id="tttj7p"
secrets
credentials
raw SQL with user literals
tenant-private values in public schemas
large JSON blobs
mutable runtime statistics
authorization decisions
```

---

### S7.2.2 Field metadata

Use for per-column semantics.

```rust id="z5z2qf"
use std::collections::HashMap;
use datafusion::arrow::datatypes::{DataType, Field};

pub fn semantic_field(
    name: &str,
    data_type: DataType,
    nullable: bool,
    semantic_type: &str,
    unit: Option<&str>,
) -> Field {
    let mut metadata = HashMap::new();

    metadata.insert("semantic.type".to_string(), semantic_type.to_string());

    if let Some(unit) = unit {
        metadata.insert("semantic.unit".to_string(), unit.to_string());
    }

    Field::new(name, data_type, nullable).with_metadata(metadata)
}

let mass_flow = semantic_field(
    "mass_flow_kg_h",
    DataType::Float64,
    false,
    "mass_flow",
    Some("kg/h"),
);
```

Recommended field-level keys:

```text id="jts073"
semantic.type
semantic.unit
semantic.domain
semantic.enum
semantic.role
semantic.default_display_format
source.name
source.path
source.system
quality.level
quality.validated_by
quality.validation_rule
governance.classification
governance.masking_policy
governance.pii
governance.export_control
display.format
display.precision
display.label
```

---

### S7.2.3 Parquet key-value metadata

DataFusion Parquet write options include `skip_arrow_metadata`, `created_by`, and arbitrary custom file key-value metadata using `metadata::key_name`; `skip_arrow_metadata=false` means Arrow schema metadata is written into Parquet file metadata, while `metadata::your_key_name` adds custom file metadata. ([Apache DataFusion][3])

```sql id="guu397"
COPY facts
TO 's3://bucket/facts_out/'
STORED AS PARQUET
OPTIONS (
  'writer_version' '1.0',
  'skip_arrow_metadata' 'false',
  'created_by' 'my-service datafusion 54.1.0',
  'metadata::pipeline' 'daily-facts-v2',
  'metadata::owner' 'analytics-platform',
  'metadata::schema_contract' 'refinery.streams@1.2.0'
);
```

Read-side caution: DataFusion’s Parquet read option `skip_metadata` skips optional embedded metadata that may be in the file schema and can help avoid schema conflicts when querying multiple Parquet files with compatible types but different metadata. ([Docs.rs][4])

Policy:

```text id="nkq5rl"
Parquet file key-value metadata:
  good for durable lineage / writer / schema contract / pipeline id
  bad for secrets / per-row state / mutable operational counters

ARROW:schema metadata:
  good for preserving Arrow field/schema metadata
  risk for multi-file metadata conflict
```

---

### S7.2.4 Arrow IPC metadata

Arrow IPC preserves Arrow schema/field metadata as part of the serialized Arrow schema. Use IPC when preserving Arrow-native schema metadata is a primary goal; test cross-language consumers and version compatibility. Arrow `Schema` stores metadata as a key-value map, and `Field` metadata encodes both arbitrary metadata and extension-type metadata. ([Apache Arrow][1])

Policy:

```text id="opf17r"
Arrow IPC:
  best fidelity for Arrow-native metadata
  good for internal columnar interchange
  test extension types and custom keys across languages
  do not assume every downstream consumer interprets extension metadata
```

---

### S7.2.5 DataFusion expression alias metadata

`Expr::alias_with_metadata` and `Expr::alias_qualified_with_metadata` attach metadata to the Arrow field when the expression is converted to a field via `Expr::to_field()`. `Expr::to_field()` determines expression field properties and has explicit metadata-handling rules: column references preserve original field metadata, literals use explicit metadata or empty metadata, aliases merge underlying metadata with alias-specific metadata preferring alias metadata, and binary/boolean expressions have empty metadata. ([Docs.rs][5])

```rust id="1gxx4x"
use std::collections::HashMap;
use datafusion::common::DFSchema;
use datafusion::logical_expr::FieldMetadata;
use datafusion::prelude::*;

fn tagged_expr() -> Expr {
    let metadata = FieldMetadata::from(HashMap::from([
        ("semantic.unit".to_string(), "kg/h".to_string()),
        ("semantic.type".to_string(), "mass_flow".to_string()),
    ]));

    col("mass_flow_kg_h")
        .alias_with_metadata("mass_flow_kg_h", Some(metadata))
}
```

Agent rule:

```text id="f5xjum"
Use alias_with_metadata for projected expression outputs whose metadata must survive to the output Field.
Do not expect raw arithmetic/function/cast expressions to preserve arbitrary metadata unless tested.
```

DataFusion 54 rewrite-safety note: `Expr::unalias_nested()` now retains aliases whose `FieldMetadata` is non-empty, so metadata attached via `alias_with_metadata` survives optimizer-style rewrites that previously discarded it. The flip side: expression equality and output-name assumptions that expected `unalias_nested` to remove *all* aliases no longer hold for metadata-carrying expressions — plan comparisons and name-normalization code must either tolerate surviving `Expr::Alias` nodes or unwrap them explicitly where dropping the metadata is intended (see S4.14).

---

### S7.2.6 SQL metadata functions

DataFusion SQL includes `arrow_field`, `arrow_metadata`, and `with_metadata`: `arrow_field(expr)` returns a struct containing name, data type, nullability, and metadata; `arrow_metadata(expr[, key])` returns all metadata or a specific key; `with_metadata(expr, key, value, ...)` annotates the output Arrow field while preserving existing metadata and overwriting colliding keys. ([Apache DataFusion][6])

```sql id="swavzr"
SELECT
  arrow_field(mass_flow_kg_h) AS mass_flow_field,
  arrow_metadata(mass_flow_kg_h) AS mass_flow_metadata,
  arrow_metadata(mass_flow_kg_h, 'semantic.unit') AS mass_flow_unit
FROM streams
LIMIT 1;
```

```sql id="rzascj"
SELECT
  with_metadata(
    mass_flow_kg_h,
    'semantic.unit', 'kg/h',
    'semantic.type', 'mass_flow'
  ) AS mass_flow_kg_h
FROM streams;
```

Validation:

```sql id="1n64lj"
SELECT
  arrow_metadata(
    with_metadata(mass_flow_kg_h, 'semantic.unit', 'kg/h'),
    'semantic.unit'
  ) AS unit
FROM streams
LIMIT 1;
```

---

## S7.3 Arrow extension type conventions

### S7.3.1 Logical vs physical type problem

DataFusion directly uses Arrow `DataType`s as its type system; DataFusion’s own custom-types blog notes that this is simple and interoperable, but it means there is no built-in distinction between logical and physical types. The same blog gives strings as an example: `Utf8`, `LargeUtf8`, `Dictionary(Utf8)`, and `Utf8View` can all represent UTF-8 strings, and Arrow extension types provide logical type information layered over physical storage. ([Apache DataFusion][7])

```text id="cc24h0"
Physical type:
  DataType::Utf8
  DataType::Float64
  DataType::FixedSizeList(Float32, 768)
  DataType::Struct(...)

Logical / semantic type:
  uuid
  currency_amount
  mass_flow
  assay_curve
  embedding_vector
  encrypted_string
  enum:product_code
```

### S7.3.2 Extension metadata convention

Arrow extension types are encoded in `Field` metadata. In Arrow-rs, `Field::try_extension_type` can retrieve an `ExtensionType` if the field metadata contains one. ([Apache Arrow][1])

Common Arrow extension metadata convention:

```text id="0qdl6v"
ARROW:extension:name
ARROW:extension:metadata
```

Platform-specific metadata convention:

```text id="zp4p8e"
semantic.type
semantic.unit
semantic.domain
semantic.logical_type
semantic.storage_type
```

Example field metadata:

```rust id="s6zjzz"
use std::collections::HashMap;
use datafusion::arrow::datatypes::{DataType, Field};

let mut metadata = HashMap::new();
metadata.insert("ARROW:extension:name".to_string(), "refinery.mass_flow".to_string());
metadata.insert(
    "ARROW:extension:metadata".to_string(),
    r#"{"unit":"kg/h","quantity_kind":"mass_flow"}"#.to_string(),
);
metadata.insert("semantic.type".to_string(), "mass_flow".to_string());
metadata.insert("semantic.unit".to_string(), "kg/h".to_string());

let field = Field::new("mass_flow_kg_h", DataType::Float64, false)
    .with_metadata(metadata);
```

### S7.3.3 Extension type design rules

```text id="qrjm0d"
Extension type must define:
  extension name
  storage Arrow DataType
  extension metadata JSON schema
  semantic meaning
  cast/coercion policy
  nullability policy
  equality policy
  display policy
  downstream compatibility story
  fallback behavior when consumer ignores extension metadata
```

Recommended naming:

```text id="e3whs9"
company.domain.logical_type
refinery.mass_flow
refinery.currency_amount
refinery.assay_curve
refinery.embedding_vector
refinery.product_code
```

Avoid:

```text id="hzld8g"
generic names:
  amount
  vector
  custom
  enum
  uuid

unstable names:
  temp.v1
  paul_test
```

### S7.3.4 Extension type examples

#### UUID as string

```rust id="ba13my"
let mut md = HashMap::new();
md.insert("ARROW:extension:name".to_string(), "refinery.uuid".to_string());
md.insert("semantic.type".to_string(), "uuid".to_string());

let field = Field::new("stream_uuid", DataType::Utf8, false)
    .with_metadata(md);
```

#### Currency amount

```rust id="kq83ht"
let mut md = HashMap::new();
md.insert("ARROW:extension:name".to_string(), "refinery.currency_amount".to_string());
md.insert(
    "ARROW:extension:metadata".to_string(),
    r#"{"currency":"USD","basis":"bbl","scale":4}"#.to_string(),
);
md.insert("semantic.type".to_string(), "currency_amount".to_string());
md.insert("semantic.unit".to_string(), "USD/bbl".to_string());

let field = Field::new("price_usd_bbl", DataType::Decimal128(20, 4), true)
    .with_metadata(md);
```

#### Embedding vector

```rust id="u4hiio"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field};

let item = Arc::new(Field::new("item", DataType::Float32, false));

let mut md = HashMap::new();
md.insert("ARROW:extension:name".to_string(), "refinery.embedding_vector".to_string());
md.insert(
    "ARROW:extension:metadata".to_string(),
    r#"{"dimension":768,"metric":"cosine"}"#.to_string(),
);

let field = Field::new("embedding", DataType::FixedSizeList(item, 768), false)
    .with_metadata(md);
```

---

## S7.4 Metadata governance taxonomy

### S7.4.1 Key classes

```text id="bui5g1"
Contract metadata:
  schema.contract.name
  schema.contract.version
  schema.contract.fingerprint
  field.id
  semantic.type
  semantic.unit

Governance metadata:
  governance.classification
  governance.pii
  governance.masking_policy
  governance.export_control
  governance.retention_policy

Lineage metadata:
  source.system
  source.name
  source.path
  source.version
  lineage.producer
  lineage.pipeline
  lineage.created_at

Quality metadata:
  quality.level
  quality.validated_by
  quality.rule_set
  quality.null_policy
  quality.rejected_row_policy

Display metadata:
  display.label
  display.format
  display.precision
  display.unit_label
  display.sort_order

Operational metadata:
  created_by
  pipeline
  batch_id
  run_id
```

### S7.4.2 Metadata policy object

```rust id="rpbu64"
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct MetadataPolicy {
    pub contract_keys: HashSet<String>,
    pub governance_keys: HashSet<String>,
    pub lineage_keys: HashSet<String>,
    pub advisory_keys: HashSet<String>,
    pub forbidden_keys: HashSet<String>,
    pub preserve_unknown_keys: bool,
    pub reject_unknown_contract_namespace: bool,
    pub redact_in_logs: HashSet<String>,
}

impl MetadataPolicy {
    pub fn default_refinery() -> Self {
        Self {
            contract_keys: [
                "schema.contract.name",
                "schema.contract.version",
                "schema.contract.fingerprint",
                "semantic.type",
                "semantic.unit",
                "field.id",
            ].into_iter().map(String::from).collect(),

            governance_keys: [
                "governance.classification",
                "governance.pii",
                "governance.masking_policy",
                "governance.export_control",
            ].into_iter().map(String::from).collect(),

            lineage_keys: [
                "source.system",
                "source.name",
                "source.path",
                "lineage.producer",
                "lineage.pipeline",
            ].into_iter().map(String::from).collect(),

            advisory_keys: [
                "display.label",
                "display.format",
                "display.precision",
                "quality.level",
            ].into_iter().map(String::from).collect(),

            forbidden_keys: [
                "secret",
                "password",
                "token",
                "aws.secret_access_key",
                "credential",
            ].into_iter().map(String::from).collect(),

            preserve_unknown_keys: false,
            reject_unknown_contract_namespace: true,
            redact_in_logs: [
                "source.uri.signed",
                "credential",
                "token",
            ].into_iter().map(String::from).collect(),
        }
    }
}
```

### S7.4.3 Validation

```rust id="3adzh3"
use datafusion::arrow::datatypes::{Field, Schema};

pub fn validate_metadata_map(
    metadata: &HashMap<String, String>,
    policy: &MetadataPolicy,
) -> Result<(), String> {
    for (k, v) in metadata {
        if k.trim().is_empty() {
            return Err("metadata key must not be empty".to_string());
        }

        if policy.forbidden_keys.contains(k) {
            return Err(format!("forbidden metadata key: {k}"));
        }

        if k.starts_with("schema.contract.") && policy.reject_unknown_contract_namespace {
            if !policy.contract_keys.contains(k) {
                return Err(format!("unknown contract metadata key: {k}"));
            }
        }

        if v.len() > 4096 {
            return Err(format!("metadata value too large for key: {k}"));
        }
    }

    Ok(())
}
```

---

## S7.5 Metadata propagation matrix

### S7.5.1 Operator-level expected behavior

| Operation                | Field metadata behavior                                                                             | Schema metadata behavior                            | Agent policy                                   |
| ------------------------ | --------------------------------------------------------------------------------------------------- | --------------------------------------------------- | ---------------------------------------------- |
| table scan               | source/provider field metadata should be exposed if provider schema contains it                     | provider schema metadata exposed if present         | trust only if provider contract says so        |
| projection `col(x)`      | DataFusion `to_field` docs say column references preserve original field metadata                   | schema metadata may or may not matter downstream    | test                                           |
| projection computed expr | binary/boolean expression field metadata is empty per `to_field` docs                               | not automatically meaningful                        | attach alias metadata if needed                |
| alias                    | alias metadata merges with underlying metadata and alias metadata wins on collision                 | not schema-level                                    | use `alias_with_metadata`                      |
| cast                     | metadata handling is expression-specific; docs say determined by input expression metadata handling | test                                                | assume metadata can change; validate           |
| join                     | left/right field metadata may pass through for columns, but duplicate names/qualifiers matter       | schema metadata not a join semantic                 | alias output and reattach critical metadata    |
| aggregate                | aggregate output is new field; metadata generally not inherited semantically                        | schema metadata not preserved as aggregate contract | attach explicit metadata                       |
| window                   | window output is new field                                                                          | same                                                | attach explicit metadata                       |
| `CASE`                   | new expression field; metadata not semantically inherited                                           | same                                                | attach explicit metadata                       |
| `UNION`                  | branch metadata conflict likely; output policy needed                                               | schema metadata conflict likely                     | choose target schema metadata                  |
| `unnest`                 | child field metadata may be relevant but must be tested                                             | same                                                | explicit field extraction + alias metadata     |
| CTAS                     | output schema determined by query                                                                   | depends sink/provider                               | snapshot output metadata                       |
| view                     | logical output fields determined by query                                                           | catalog/provider dependent                          | treat view metadata as contract only if tested |
| Parquet write            | Arrow metadata written unless skipped; custom file metadata possible                                | durable if written                                  | read-after-write test                          |
| Arrow IPC write          | Arrow schema metadata expected to serialize                                                         | high fidelity                                       | cross-language test                            |
| CSV/JSON write           | weak/no typed metadata fidelity                                                                     | metadata external manifest needed                   | write sidecar manifest                         |

DataFusion’s `Expr::to_field()` documentation is unusually important for metadata propagation: column references preserve original field metadata, aliases merge metadata preferring alias metadata, binary and boolean expressions have empty metadata, and `alias_with_metadata` attaches metadata when converted to a field. ([Docs.rs][5])

---

## S7.6 Metadata preservation test harness

### S7.6.1 SQL tests

```sql id="4d6dcv"
WITH t AS (
  SELECT with_metadata(1, 'semantic.unit', 'kg/h') AS mass_flow_kg_h
)
SELECT
  arrow_metadata(mass_flow_kg_h, 'semantic.unit') AS unit
FROM t;
```

```sql id="tzvnv9"
WITH t AS (
  SELECT with_metadata(1, 'semantic.unit', 'kg/h') AS mass_flow_kg_h
)
SELECT
  arrow_field(mass_flow_kg_h) AS field_info,
  arrow_metadata(mass_flow_kg_h) AS metadata
FROM t;
```

Use SQL `arrow_field` and `arrow_metadata` for quick propagation checks; DataFusion documents `arrow_field` as returning name, data type, nullability, and metadata, and `arrow_metadata` as returning all metadata or a specific key. ([Apache DataFusion][6])

### S7.6.2 Rust plan/output metadata test

```rust id="z39glt"
use std::collections::HashMap;
use datafusion::logical_expr::FieldMetadata;
use datafusion::prelude::*;

#[tokio::test]
async fn alias_metadata_survives_projection_field() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();

    let metadata = FieldMetadata::from(HashMap::from([
        ("semantic.unit".to_string(), "kg/h".to_string()),
        ("semantic.type".to_string(), "mass_flow".to_string()),
    ]));

    let df = ctx
        .sql("SELECT 1 AS x")
        .await?
        .select(vec![
            col("x").alias_with_metadata("mass_flow_kg_h", Some(metadata))
        ])?;

    let field = df.schema().field_with_unqualified_name("mass_flow_kg_h")?;

    assert_eq!(
        field.metadata().get("semantic.unit").map(String::as_str),
        Some("kg/h")
    );

    Ok(())
}
```

### S7.6.3 Parquet read-after-write metadata test

```sql id="4huj1x"
COPY (
  SELECT
    with_metadata(mass_flow_kg_h, 'semantic.unit', 'kg/h') AS mass_flow_kg_h
  FROM streams
)
TO 's3://lake/test/metadata_roundtrip/'
STORED AS PARQUET
OPTIONS (
  'skip_arrow_metadata' 'false',
  'metadata::schema_contract' 'refinery.streams@1.2.0'
);
```

Read back:

```sql id="mu1uu6"
CREATE EXTERNAL TABLE metadata_roundtrip
STORED AS PARQUET
LOCATION 's3://lake/test/metadata_roundtrip/';

SELECT
  arrow_metadata(mass_flow_kg_h, 'semantic.unit') AS unit
FROM metadata_roundtrip
LIMIT 1;
```

Caveat: `datafusion.execution.parquet.skip_metadata` can skip optional embedded metadata on read; this can avoid conflicts across files with compatible types but different metadata, but it also means metadata-dependent tests/contracts can fail by configuration. ([Docs.rs][4])

---

## S7.7 Semantic annotation catalog

### S7.7.1 Unit of measure

```rust id="qzfn4k"
let field = semantic_field(
    "mass_flow_kg_h",
    DataType::Float64,
    false,
    "mass_flow",
    Some("kg/h"),
);
```

Recommended keys:

```text id="u2gdww"
semantic.unit = kg/h
semantic.quantity_kind = mass_flow
display.unit_label = kg/h
display.precision = 2
```

Unit conversion must be data transformation, not metadata update:

```sql id="49nn21"
SELECT
  mass_flow_kg_h / 1000.0 * 24.0 AS mass_flow_t_d
FROM streams;
```

### S7.7.2 Semantic type

```text id="3vt3jp"
semantic.type = mass_flow
semantic.type = sulfur_content
semantic.type = price
semantic.type = product_code
semantic.type = timestamp_event
semantic.type = enum
semantic.type = uuid
semantic.type = embedding_vector
```

Policy:

```text id="yddvft"
semantic.type drives platform validation only if your code consumes it.
DataFusion core does not automatically enforce semantic.type.
```

### S7.7.3 Source lineage

```text id="q2lm4k"
source.system = smartref
source.name = "Mass Flow kg/h"
source.path = workbook:Streams!C15
lineage.pipeline = csv_extract_v2
lineage.producer = assay_ingestion
```

Policy:

```text id="l9x9bv"
source lineage is allowed in raw/staging/curated metadata.
strip or redact source path in public API if sensitive.
```

### S7.7.4 Quality flag

```text id="m64557"
quality.level = raw
quality.level = parsed
quality.level = validated
quality.validation_rule = non_null_positive_mass_flow
quality.rejected_row_policy = reject_invalid_numeric
```

Policy:

```text id="jbm9dj"
quality metadata is advisory unless:
  query generator enforces filters
  provider injects validation
  sink rejects invalid rows
```

### S7.7.5 Confidentiality class

```text id="fmkwk6"
governance.classification = public
governance.classification = internal
governance.classification = confidential
governance.classification = restricted
governance.pii = false
governance.masking_policy = hash_email
```

Policy:

```text id="l4tq8l"
Classification metadata must be consumed by access-control code to matter.
Do not expose confidential metadata itself to unauthorized users.
Filter information_schema / arrow_metadata results in multi-tenant systems.
```

### S7.7.6 Display format

```text id="l38c06"
display.label = Mass Flow
display.format = fixed_decimal
display.precision = 2
display.unit_label = kg/h
display.sort_order = 20
```

Policy:

```text id="wy598q"
Display metadata belongs at UI/API layer.
Do not use display format for computational rounding.
```

### S7.7.7 Domain enum

```text id="pmfm4c"
semantic.type = enum
semantic.enum.name = product_code
semantic.enum.values = gasoline,diesel,jet_fuel,feedstock
```

Better for large/stable enums:

```text id="az6jng"
use dimension table:
  product_codes(product_code, product_name, group, active_flag)
```

---

## S7.8 Metadata governance registry

```rust id="exg62s"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataKeySpec {
    pub key: String,
    pub class: MetadataClass,
    pub value_type: MetadataValueType,
    pub required: bool,
    pub allowed_values: Option<Vec<String>>,
    pub preserve_through_projection: bool,
    pub preserve_to_parquet: bool,
    pub expose_to_users: bool,
    pub redact_in_logs: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MetadataClass {
    Contract,
    Semantic,
    Governance,
    Lineage,
    Quality,
    Display,
    Operational,
    Extension,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum MetadataValueType {
    String,
    BoolString,
    IntegerString,
    DecimalString,
    JsonString,
    EnumString,
}
```

Example registry entry:

```json id="7p706t"
{
  "key": "semantic.unit",
  "class": "Semantic",
  "value_type": "String",
  "required": false,
  "preserve_through_projection": true,
  "preserve_to_parquet": true,
  "expose_to_users": true,
  "redact_in_logs": false
}
```

---

## S7.9 Metadata validator

```rust id="5wg3qh"
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct MetadataValidationError {
    pub key: String,
    pub reason: String,
}

pub fn validate_metadata(
    metadata: &HashMap<String, String>,
    specs: &HashMap<String, MetadataKeySpec>,
    forbidden_prefixes: &[&str],
) -> Result<(), Vec<MetadataValidationError>> {
    let mut errors = Vec::new();

    for (key, value) in metadata {
        if key.trim().is_empty() {
            errors.push(MetadataValidationError {
                key: key.clone(),
                reason: "empty metadata key".to_string(),
            });
        }

        if forbidden_prefixes.iter().any(|p| key.starts_with(p)) {
            errors.push(MetadataValidationError {
                key: key.clone(),
                reason: "forbidden metadata namespace".to_string(),
            });
        }

        if value.len() > 4096 {
            errors.push(MetadataValidationError {
                key: key.clone(),
                reason: "metadata value exceeds 4096 bytes".to_string(),
            });
        }

        if let Some(spec) = specs.get(key) {
            if let Some(allowed) = &spec.allowed_values {
                if !allowed.contains(value) {
                    errors.push(MetadataValidationError {
                        key: key.clone(),
                        reason: format!("invalid metadata value `{value}`"),
                    });
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

---

## S7.10 Metadata propagation report

```rust id="6j1ycv"
#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataPropagationReport {
    pub operation: String,
    pub input_fields: Vec<FieldMetadataSummary>,
    pub output_fields: Vec<FieldMetadataSummary>,
    pub lost_keys: Vec<MetadataLoss>,
    pub overwritten_keys: Vec<MetadataOverwrite>,
    pub preserved_keys: Vec<String>,
    pub decision: MetadataDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldMetadataSummary {
    pub field_path: String,
    pub metadata_keys: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataLoss {
    pub field_path: String,
    pub key: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetadataOverwrite {
    pub field_path: String,
    pub key: String,
    pub old_value: String,
    pub new_value: String,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum MetadataDecision {
    Accept,
    AcceptWithWarnings,
    Reject,
    RequiresExplicitReannotation,
}
```

Example finding:

```json id="cb06bp"
{
  "operation": "projection_arithmetic",
  "lost_keys": [
    {
      "field_path": "mass_flow_kg_d",
      "key": "semantic.unit",
      "reason": "computed binary expression metadata is empty unless alias metadata is attached"
    }
  ],
  "decision": "RequiresExplicitReannotation"
}
```

---

## S7.11 Operator/sink preservation matrix for agents

```text id="gfyhef"
Preservation expectation levels:
  PRESERVE_BY_DOC      documented behavior
  PRESERVE_BY_TEST     observed in pinned version; test required
  REANNOTATE           attach metadata explicitly
  STRIP                metadata should be stripped
  MANIFEST_ONLY        store in sidecar manifest instead of field metadata
```

| Boundary                              | Default agent expectation             | Action                                      |
| ------------------------------------- | ------------------------------------- | ------------------------------------------- |
| source Arrow schema → provider schema | PRESERVE_BY_TEST                      | validate provider.schema metadata           |
| provider schema → `col(x)` projection | PRESERVE_BY_DOC                       | still test critical keys                    |
| source field → arithmetic expression  | REANNOTATE                            | use alias metadata / `with_metadata`        |
| source field → alias                  | PRESERVE_BY_DOC + override            | alias metadata wins on collisions           |
| source field → cast                   | PRESERVE_BY_TEST                      | verify; reannotate if contract-critical     |
| join projection                       | PRESERVE_BY_TEST                      | alias + reannotate critical keys            |
| aggregate output                      | REANNOTATE                            | aggregate creates new semantic field        |
| window output                         | REANNOTATE                            | attach rank/window semantics                |
| `CASE` output                         | REANNOTATE                            | branch metadata not reliable contract       |
| `UNION`                               | REANNOTATE / target schema wins       | choose canonical branch metadata            |
| CTAS                                  | PRESERVE_BY_TEST                      | snapshot output schema/metadata             |
| Parquet write                         | PRESERVE_BY_CONFIG                    | `skip_arrow_metadata=false` + readback test |
| Parquet read                          | PRESERVE_BY_CONFIG                    | `skip_metadata=false` if needed             |
| Arrow IPC write/read                  | PRESERVE_BY_TEST                      | roundtrip test                              |
| CSV/JSON output                       | MANIFEST_ONLY                         | metadata sidecar required                   |
| API response                          | MANIFEST_ONLY or explicit schema JSON | do not rely on serialized row data          |

---

## S7.12 SQL examples: metadata-aware projections

### Attach unit metadata to computed output

```sql id="g5fvq2"
SELECT
  with_metadata(
    mass_flow_kg_h / 1000.0 * 24.0,
    'semantic.type', 'mass_flow',
    'semantic.unit', 't/d',
    'display.label', 'Mass Flow',
    'display.precision', '2'
  ) AS mass_flow_t_d
FROM streams;
```

### Inspect metadata

```sql id="s0wxak"
SELECT
  arrow_field(mass_flow_t_d) AS field_info,
  arrow_metadata(mass_flow_t_d, 'semantic.unit') AS unit
FROM (
  SELECT
    with_metadata(
      mass_flow_kg_h / 1000.0 * 24.0,
      'semantic.unit', 't/d'
    ) AS mass_flow_t_d
  FROM streams
)
LIMIT 1;
```

### Parquet output with file metadata

```sql id="pphw6u"
COPY (
  SELECT
    with_metadata(
      mass_flow_kg_h,
      'semantic.type', 'mass_flow',
      'semantic.unit', 'kg/h'
    ) AS mass_flow_kg_h
  FROM streams
)
TO 's3://lake/curated/streams/'
STORED AS PARQUET
OPTIONS (
  'skip_arrow_metadata' 'false',
  'created_by' 'refinery-platform',
  'metadata::schema_contract' 'refinery.streams@1.2.0',
  'metadata::pipeline' 'curated-streams-v2'
);
```

---

## S7.13 Rust examples: metadata-aware schema construction

### Field metadata

```rust id="nujla4"
use std::collections::HashMap;
use datafusion::arrow::datatypes::{DataType, Field};

pub fn field_with_metadata(
    name: &str,
    data_type: DataType,
    nullable: bool,
    pairs: &[(&str, &str)],
) -> Field {
    let metadata = pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect::<HashMap<_, _>>();

    Field::new(name, data_type, nullable).with_metadata(metadata)
}

let f = field_with_metadata(
    "sulfur_wt_pct",
    DataType::Float64,
    true,
    &[
        ("semantic.type", "sulfur_content"),
        ("semantic.unit", "wt%"),
        ("display.precision", "3"),
    ],
);
```

### Schema metadata

```rust id="spab4f"
use std::sync::Arc;
use datafusion::arrow::datatypes::Schema;

let schema = Arc::new(Schema::new_with_metadata(
    vec![f],
    HashMap::from([
        ("schema.contract.name".to_string(), "refinery.assay".to_string()),
        ("schema.contract.version".to_string(), "1.0.0".to_string()),
    ]),
));
```

### Alias metadata

```rust id="8vs33x"
use std::collections::HashMap;
use datafusion::logical_expr::FieldMetadata;
use datafusion::prelude::*;

let metadata = FieldMetadata::from(HashMap::from([
    ("semantic.type".to_string(), "mass_flow".to_string()),
    ("semantic.unit".to_string(), "t/d".to_string()),
]));

let expr = (col("mass_flow_kg_h") / lit(1000.0) * lit(24.0))
    .alias_with_metadata("mass_flow_t_d", Some(metadata));
```

---

## S7.14 Extension-type governance

### S7.14.1 Extension registry

Platform-level governance registry (the spec object below is application metadata; since DataFusion 54 the engine also ships a first-class runtime registry — `ExtensionTypeRegistry` in `datafusion_expr::registry` — covered in S7.20, which this governance spec should feed):

```rust id="ql9cio"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtensionTypeSpec {
    pub extension_name: String,
    pub physical_type: String,
    pub metadata_json_schema: String,
    pub semantic_type: String,
    pub allowed_casts: Vec<String>,
    pub preserve_to_parquet: bool,
    pub preserve_to_ipc: bool,
    pub fallback_behavior: ExtensionFallbackBehavior,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ExtensionFallbackBehavior {
    TreatAsPhysicalType,
    RejectIfUnrecognized,
    PreserveOpaque,
    StripMetadata,
}
```

### S7.14.2 Required extension tests

```text id="3yndxf"
[ ] Field metadata contains ARROW:extension:name
[ ] extension metadata parses as JSON
[ ] physical DataType matches registry
[ ] DataFusion SQL can query physical type
[ ] UDF/provider recognizes extension type if required
[ ] Parquet write/read preserves extension metadata if configured
[ ] Arrow IPC write/read preserves extension metadata
[ ] unknown consumer fallback is acceptable
```

### S7.14.3 Extension failure diagnostics

```json id="fga12e"
{
  "error_class": "unknown_arrow_extension_type",
  "field_path": "embedding",
  "extension_name": "refinery.embedding_vector",
  "physical_type": "FixedSizeList(Float32, 768)",
  "decision": "reject",
  "suggested_fix": "register extension type spec or strip extension metadata explicitly"
}
```

---

## S7.15 Security and governance

Metadata can leak sensitive information even when row values are protected.

Sensitive metadata examples:

```text id="4r847x"
source.path = s3://private-bucket/client-x/acquisition-target/
lineage.pipeline = sanctions_screening
governance.classification = restricted_mna
source.name = SSN
metadata::aws_access_key_id = ...
```

Policy:

```text id="s3yq7d"
Never store secrets in schema/field/Parquet metadata.
Redact metadata in logs.
Filter arrow_metadata / arrow_field exposure in public SQL endpoints.
Treat information_schema + metadata functions as governance surfaces.
Do not allow untrusted users to attach arbitrary metadata in CTAS/export pipelines.
```

DataFusion’s Parquet format options support arbitrary file metadata through `metadata::key_name`, so pipelines must prevent user-supplied secrets or unbounded values from entering persisted file metadata. ([Apache DataFusion][3])

---

## S7.16 Deployment advisory

```text id="rlpz4q"
Production schema metadata:
  define allowlisted keys
  version key registry
  reject forbidden namespaces
  classify keys by contract/governance/lineage/display/advisory
  add metadata fingerprinting
  add propagation tests
  add read-after-write tests for Parquet/IPC
```

```text id="u15drt"
DataFusion execution:
  arbitrary metadata is not optimizer semantics unless specific code consumes it
  expression metadata propagation varies by expression kind
  alias metadata is the safest way to attach computed-output metadata
  with_metadata is the SQL path for output field metadata
```

```text id="f6y2kg"
Parquet:
  keep skip_arrow_metadata=false when Arrow metadata matters
  set skip_metadata=false on read when metadata matters
  use metadata::key_name for file-level lineage
  do not rely on metadata in multi-file datasets without conflict policy
```

```text id="8l8ows"
Arrow IPC:
  prefer for high-fidelity Arrow-native metadata interchange
  test extension metadata across consumers
```

```text id="902e4a"
CSV/JSON:
  write sidecar schema/metadata manifest
  row files do not carry rich Arrow metadata reliably
```

---

## S7.17 Anti-pattern inventory

```text id="pbjhkh"
Metadata anti-patterns:
  assuming metadata changes DataFusion optimizer/runtime behavior
  using metadata instead of DataType for computation
  using metadata instead of nullability/constraints for enforcement
  storing credentials in schema or Parquet metadata
  storing large mutable JSON blobs in Field metadata
  assuming arithmetic/cast/aggregate output preserves input metadata
  relying on Parquet metadata preservation with skip_arrow_metadata=true
  relying on Parquet metadata reads with skip_metadata=true
  treating Arrow extension metadata as universally understood by consumers
  using extension type without fallback behavior
  changing semantic.unit without changing data values
  changing governance.classification without access-control update
  exposing arrow_metadata to untrusted users
  using metadata-only schema fingerprints for physical compatibility
  failing to test metadata preservation across CTAS/views/sinks
```

---

## S7.18 Agent checklist

```text id="kivg95"
[ ] Identify metadata location:
    schema / field / expression alias / SQL with_metadata / Parquet file / Arrow IPC / sidecar manifest.

[ ] Classify metadata:
    contract / semantic / governance / lineage / quality / display / operational / extension.

[ ] Validate metadata keys:
    non-empty
    allowlisted
    no secrets
    bounded value length
    namespace policy satisfied.

[ ] Decide preservation requirement:
    must preserve
    preserve if available
    advisory
    strip
    sidecar-only.

[ ] Attach metadata correctly:
    Field::with_metadata for source/provider schema.
    Schema::new_with_metadata for dataset/schema contract.
    Expr::alias_with_metadata for DataFrame computed fields.
    SQL with_metadata for SQL computed fields.
    Parquet metadata::key_name for file-level lineage.

[ ] Test propagation:
    projection
    alias
    cast
    join
    aggregate
    CTAS
    Parquet write/read
    Arrow IPC write/read.

[ ] For extension types:
    define extension name
    define storage type
    define extension metadata schema
    define fallback behavior
    test downstream compatibility.

[ ] For governance:
    redact logs
    restrict metadata introspection
    filter public schema/metadata APIs
    do not allow untrusted metadata injection.

[ ] For sinks:
    Parquet/Arrow: read-after-write metadata tests.
    CSV/JSON/API: sidecar manifest or explicit schema response.
```

## S7.19 Minimal metadata manifest shape

```json id="ebf6bt"
{
  "schema_contract": {
    "name": "refinery.streams",
    "version": "1.2.0",
    "fingerprint": "sha256:..."
  },
  "schema_metadata": {
    "schema.owner": "simulation-engine",
    "lineage.pipeline": "curated-streams-v2"
  },
  "fields": [
    {
      "name": "mass_flow_kg_h",
      "data_type": "Float64",
      "nullable": false,
      "metadata": {
        "semantic.type": "mass_flow",
        "semantic.unit": "kg/h",
        "display.precision": "2",
        "governance.classification": "internal"
      }
    }
  ],
  "preservation": {
    "parquet_skip_arrow_metadata": false,
    "parquet_skip_metadata_on_read": false,
    "sidecar_required_for_csv_json": true
  }
}
```

## S7.20 The DataFusion 54 extension-type registry

### S7.20.1 What changed

Through 53.x, Arrow extension types were purely a metadata convention inside DataFusion: fields carried `ARROW:extension:name` / `ARROW:extension:metadata` (S7.3), and every consumer hand-rolled recognition — the `ExtensionTypeSpec` governance registry in S7.14 is exactly that pattern. DataFusion 54 adds an engine-level registry that resolves extension-type metadata into typed extension-type instances:

```text
datafusion_expr::registry::ExtensionTypeRegistry        trait — lookup + registration
datafusion_expr::registry::ExtensionTypeRegistryRef     = Arc<dyn ExtensionTypeRegistry>
datafusion_expr::registry::ExtensionTypeRegistration    struct — name + factory
datafusion_expr::registry::ExtensionTypeRegistrationRef = Arc<ExtensionTypeRegistration>
datafusion_expr::registry::MemoryExtensionTypeRegistry  in-memory implementation

datafusion_common::types::DFExtensionType               trait — a resolved extension type
datafusion_common::types::DFExtensionTypeRef            = Arc<dyn DFExtensionType>
```

`ExtensionTypeRegistry` lives in `datafusion_expr::registry` alongside `FunctionRegistry` (same module, sibling trait), and every session exposes one: the `Session` trait (`datafusion::catalog::Session`) gained `extension_type_registry(&self) -> &ExtensionTypeRegistryRef`, and `SessionStateBuilder::with_extension_type_registry(registry)` installs a custom implementation.

### S7.20.2 Trait surface

```rust
// datafusion_expr::registry (DataFusion 54.1.0)
pub trait ExtensionTypeRegistry: Debug + Send + Sync {
    /// Lookup by extension type name; error if not registered.
    fn extension_type_registration(
        &self,
        name: &str,
    ) -> Result<ExtensionTypeRegistrationRef>;

    /// Provided method: reads field.extension_type_name() and
    /// field.extension_type_metadata() (the ARROW:extension:name /
    /// ARROW:extension:metadata keys) and calls the matching registration's
    /// factory. Ok(None) when the field carries no extension metadata; error
    /// when the extension name is not registered.
    fn create_extension_type_for_field(
        &self,
        field: &Field,
    ) -> Result<Option<DFExtensionTypeRef>>;

    fn extension_type_registrations(&self) -> Vec<ExtensionTypeRegistrationRef>;

    fn add_extension_type_registration(
        &self,
        extension_type: ExtensionTypeRegistrationRef,
    ) -> Result<Option<ExtensionTypeRegistrationRef>>;

    /// Provided method: add_extension_type_registration for each entry.
    fn extend(&self, extension_types: &[ExtensionTypeRegistrationRef]) -> Result<()>;

    fn remove_extension_type_registration(
        &self,
        name: &str,
    ) -> Result<Option<ExtensionTypeRegistrationRef>>;
}
```

A registration binds an extension-type name to a factory closure. The factory receives the field's storage `DataType` and the optional serialized metadata string, validates that the storage type is legal for the extension type, and returns the resolved instance:

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::DataType;
use datafusion::common::types::{DFExtensionType, DFExtensionTypeRef};
use datafusion::common::{plan_err, Result};
use datafusion::logical_expr::registry::ExtensionTypeRegistration;

/// Resolved platform extension type: mass flow stored as Float64.
#[derive(Debug)]
pub struct MassFlowExtensionType {
    unit: String,
}

impl DFExtensionType for MassFlowExtensionType {
    fn storage_type(&self) -> DataType {
        DataType::Float64
    }

    fn serialize_metadata(&self) -> Option<String> {
        Some(format!(r#"{{"unit":"{}"}}"#, self.unit))
    }

    // create_array_formatter(...) is optional: return a custom ArrayFormatter
    // to control how values of this type render in record batches.
}

pub fn mass_flow_registration() -> Arc<ExtensionTypeRegistration> {
    ExtensionTypeRegistration::new_arc(
        "refinery.mass_flow",
        |storage_type: &DataType, metadata: Option<&str>| -> Result<DFExtensionTypeRef> {
            if !matches!(storage_type, DataType::Float64) {
                return plan_err!(
                    "refinery.mass_flow requires Float64 storage, got {storage_type}"
                );
            }
            let unit = parse_unit_from_metadata(metadata)?;
            Ok(Arc::new(MassFlowExtensionType { unit }))
        },
    )
}
```

`DFExtensionType` is deliberately storage-resolved: unlike arrow-rs `ExtensionType` (which only models metadata and is not dyn-compatible), a `DFExtensionType` instance is locked to one exact storage `DataType` — DataFusion's JSON extension type, for example, fixes one of `Utf8`/`LargeUtf8`/`Utf8View` and reports it via `storage_type()`.

### S7.20.3 Session wiring and canonical types

```rust
use std::sync::Arc;

use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::logical_expr::registry::{ExtensionTypeRegistry, MemoryExtensionTypeRegistry};
use datafusion::prelude::*;

let registry = MemoryExtensionTypeRegistry::new_with_canonical_extension_types();
registry.extend(&[mass_flow_registration()])?;

let state = SessionStateBuilder::new()
    .with_default_features()
    .with_extension_type_registry(Arc::new(registry))
    .build();

let ctx = SessionContext::new_with_state(state);

// Inside providers / planner extensions / UDF planning code:
//   let ext = session.extension_type_registry()
//       .create_extension_type_for_field(&field)?;
```

Registry population rules:

```text
Default registry = MemoryExtensionTypeRegistry::new_empty()
  SessionStateDefaults::default_extension_types() is empty in 54.1.0 —
  even the canonical Arrow extension types are NOT pre-registered.

MemoryExtensionTypeRegistry::new_with_canonical_extension_types()
  opt-in constructor that pre-registers the Arrow canonical extension types.

MemoryExtensionTypeRegistry::new_with_types(iter)
  bulk construction from ExtensionTypeRegistrationRef values.
```

The Arrow side of the contract is `arrow-schema` 58.4.0's canonical extension types (`arrow_schema::extension`, one module per type in `src/extension/canonical/`): `Bool8`, `FixedShapeTensor`, `VariableShapeTensor`, `Json`, `Opaque`, `TimestampWithOffset`, `Uuid`, unified by the `CanonicalExtensionType` enum. DataFusion mirrors each with a storage-resolved wrapper in `datafusion_common::types` (`DFBool8`, `DFFixedShapeTensor`, `DFVariableShapeTensor`, `DFJson`, `DFOpaque`, `DFTimestampWithOffset`, `DFUuid`), and those wrappers are what `new_with_canonical_extension_types()` registers.

### S7.20.4 Scope discipline

The registry does not turn metadata into automatic optimizer or execution semantics. In 54.1.0 the customization surface is intentionally narrow — extension-type-aware value formatting (`DFExtensionType::create_array_formatter`) plus programmatic resolution for your own providers, planner extensions, and UDFs. Governance rules for this doc's semantic-annotation model:

```text
Use plain field metadata (semantic.*, governance.*, S7.2.2) when annotations
  only need to ride along with the schema.

Use a registered extension type when code must *recognize* the logical type:
  reject invalid storage types at resolution time,
  customize display,
  gate provider/UDF behavior on the logical type rather than the storage type.

Register extension types only where physical compatibility is insufficient —
  an extension type creates a consumer obligation (S7.14 fallback_behavior)
  for every engine and language that reads the data.

Keep the platform governance registry (S7.14 ExtensionTypeSpec) as the source
  of truth and derive ExtensionTypeRegistration factories from it, so the
  runtime registry and the governance catalog cannot drift.
```

---

## S7.21 Metadata-aware expressions and field-aware casts (DataFusion 54)

### S7.21.1 New metadata-aware functions

DataFusion 54 adds two typed-cast companions to the metadata functions already covered in S7.2.6 (`with_metadata`, `arrow_metadata`, `arrow_field`), all in `datafusion-functions` core:

```text
with_metadata(expr, key1, value1[, key2, value2, ...])
  attaches Arrow field metadata to the output field; existing metadata is
  preserved, colliding keys are overwritten (inverse of arrow_metadata)

cast_to_type(expr, reference)
  casts expr to the data type of the reference expression;
  the reference's value is ignored, only its type is used

try_cast_to_type(expr, reference)
  same, but yields NULL instead of erroring on failed casts
```

```sql
-- Align a stage column to the type of the curated column without
-- spelling the target type in generated SQL:
SELECT cast_to_type(mass_flow_raw, mass_flow_kg_h) AS mass_flow_kg_h
FROM stage
CROSS JOIN (SELECT mass_flow_kg_h FROM streams LIMIT 1);
```

For contract-grade projections prefer explicit `arrow_cast` with a spelled-out type; `cast_to_type`/`try_cast_to_type` are for generated/generic query code where the target type is only available as another expression.

### S7.21.2 Field-aware casts: logical `field`, physical `target_field`

DataFusion 54 rebuilt the cast expressions around `FieldRef` instead of bare `DataType`, so casts can carry name, nullability, and metadata — including extension-type metadata — through planning:

```text
logical  datafusion_expr::expr::Cast     { expr, field: FieldRef }   (field named `field`)
logical  datafusion_expr::expr::TryCast  { expr, field: FieldRef }
physical datafusion_physical_expr::expressions::CastExpr
         { expr, target_field: FieldRef, cast_options }              (field named `target_field`)

CastColumnExpr is REMOVED in 54 — its role is absorbed by CastExpr with an
explicit target field.
```

Constructors:

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::physical_expr::expressions::CastExpr;

// Type-only compatibility constructors wrap the DataType into a fresh
// nullable field — any metadata contract must come from elsewhere:
//   Cast::new(expr, data_type)         (logical)
//   CastExpr::new(expr, cast_type, cast_options)   (physical)

// Field-preserving constructors — preferred whenever a Field already exists:
//   Cast::new_from_field(expr, field)              (logical)
let target_field: FieldRef = Arc::new(
    Field::new("mass_flow_kg_h", DataType::Float64, false)
        .with_metadata(mass_flow_metadata()),
);
let cast = CastExpr::new_with_target_field(input_expr, Arc::clone(&target_field), None);
```

### S7.21.3 Rewrite discipline: preserve the target Field

Optimizer rules, projection rewrites, and schema-adaptation helpers must carry the target `FieldRef` through, not rebuild casts from `target_field.data_type()`:

```rust
// BAD: silently drops metadata, extension-type identity, and the
// name/nullability contract of the original target field.
let rebuilt = Field::new(
    old_field.name(),
    new_data_type,
    old_field.is_nullable(),
);

// GOOD: clone + modify preserves everything not explicitly changed.
let rebuilt = old_field.as_ref().clone().with_data_type(new_data_type);
```

Agent rules:

```text
Any helper that reconstructs Field::new(name, data_type, nullable) from parts
  is a metadata-stripping hazard under 54 — audit every such call site.
When rewriting a Cast/TryCast/CastExpr, reuse the existing FieldRef (clone +
  with_data_type / with_nullable) instead of synthesizing a new Field.
Schema snapshots for cast-heavy plans should assert field metadata, not just
  (name, type, nullability), so stripped metadata fails tests.
Code still referencing CastColumnExpr does not compile against 54; migrate to
  CastExpr::new_with_target_field.
```

---

Core operating rule: **metadata is a governed annotation channel; it becomes semantics only when your schema factory, provider, planner extension, UDF, sink, or governance layer explicitly consumes and tests it.**

[1]: https://arrow.apache.org/rust/arrow/datatypes/struct.Field.html "Field in arrow::datatypes - Rust"
[2]: https://docs.rs/arrow/latest/arrow/datatypes/struct.Schema.html "Schema in arrow::datatypes - Rust"
[3]: https://datafusion.apache.org/user-guide/sql/format_options.html "Format Options — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/common/config/struct.ParquetOptions.html "ParquetOptions in datafusion::common::config - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html "Expr in datafusion::logical_expr - Rust"
[6]: https://datafusion.apache.org/user-guide/sql/scalar_functions.html "Scalar Functions — Apache DataFusion  documentation"
[7]: https://datafusion.apache.org/blog/2025/09/21/custom-types-using-metadata/ "Implementing User Defined Types and Custom Metadata in DataFusion - Apache DataFusion Blog"


# DataFusion Advanced — S8) Constraints, functional dependencies, defaults, and table contracts

## S8.0 Objective

Make table schema a **contract object**, not a field list.

```text id="ngn3rh"
TableContract
  ├─ fields
  ├─ nullability
  ├─ constraints
  ├─ functional dependencies
  ├─ uniqueness / key semantics
  ├─ default values
  ├─ generated columns
  ├─ check constraints
  ├─ partition columns
  ├─ ordering / distribution claims
  ├─ statistics
  ├─ pushdown guarantees
  ├─ write semantics
  └─ enforcement boundary
```

The attached documentation already mentions provider schema stability, constraints, statistics, filter pushdown, and provider test requirements, but it treats them primarily as provider implementation details rather than a unified contract model.  DataFusion’s `TableProvider` surface in 54.1.0 includes required `schema`, `table_type`, and `scan` methods, plus provided methods such as `constraints`, `get_column_default`, `statistics`, `supports_filters_pushdown`, and `insert_into`, which makes the provider the natural boundary for contract metadata. ([Docs.rs][1]) Note that in 54.1.0 the trait carries an `Any` supertrait (`TableProvider: Any + Debug + Sync + Send`) and no longer declares an `as_any` method; concrete-provider access goes through `downcast_ref::<T>()` via trait upcasting.

---

## S8.1 Contract stack

```text id="r8kotx"
Arrow Field / Schema
  = physical names, types, nullability, metadata

DFSchema
  = logical names, qualifiers, expression binding, functional dependencies

Constraints
  = table-level primary-key / unique-key metadata

FunctionalDependencies
  = determinant → dependent relationships used for logical reasoning

Statistics
  = optional/inexact planning metadata

TableProvider
  = provider-owned schema + constraints + statistics + defaults + scan/write semantics

Application contract
  = what is actually enforced, audited, versioned, and exposed
```

DataFusion’s table-constraint documentation states that table providers can describe constraints using `TableConstraint` / `Constraints` APIs, but DataFusion does **not** currently enforce those constraints at runtime; only `Field` nullability is enforced during execution, and DataFusion does not check nullability during ingestion. ([Apache DataFusion][2])

---

## S8.2 Schema contract components

### S8.2.1 Fields

Field contract:

```text id="2dg412"
field.name
field.data_type
field.nullable
field.metadata
field.ordinal
field.semantic_type
field.unit
field.default
field.generated_expr
field.check_exprs
field.governance_class
```

Rust schema:

```rust id="xo1s6p"
use std::sync::Arc;
use std::collections::HashMap;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};

pub fn streams_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("stream_id", DataType::Utf8, false),
        Field::new("case_id", DataType::Utf8, false),
        Field::new("unit_id", DataType::Utf8, false),
        Field::new("mass_flow_kg_h", DataType::Float64, false),
        Field::new("sulfur_wt_pct", DataType::Float64, true),
    ]))
}
```

Policy:

```text id="ios6qd"
Field contract is physically enforced only at RecordBatch construction/execution boundary.
Semantic metadata is advisory unless your provider/application consumes it.
```

---

### S8.2.2 Nullability

Nullability is the strongest built-in schema property in DataFusion’s table contract layer. DataFusion documentation says returning null values for columns marked non-nullable results in runtime errors during execution, but DataFusion does not check or enforce nullability when data is ingested. ([Apache DataFusion][2])

```rust id="j0kw63"
Field::new("stream_id", DataType::Utf8, false); // non-null contract
Field::new("sulfur_wt_pct", DataType::Float64, true); // nullable
```

Contract rule:

```text id="vz4rgf"
nullable=false requires one of:
  upstream storage guarantee
  provider validation
  ingestion rejection
  query filter before publication
  write-path enforcement

nullable=false without enforcement = runtime-error risk
```

Validation SQL:

```sql id="vbtjcb"
SELECT COUNT(*) AS null_stream_id_rows
FROM streams
WHERE stream_id IS NULL;
```

Provider rule:

```text id="b3p4m3"
If provider.schema() marks a field non-nullable, provider execution must never emit nulls for that field.
```

---

### S8.2.3 Constraints

DataFusion `Constraint` in 54.1.0 has `PrimaryKey(Vec<usize>)` and `Unique(Vec<usize>)` variants (unchanged from 53.x), where indices refer to schema fields; primary-key columns are jointly unique and not nullable, while unique-key columns are jointly unique. ([Docs.rs][3]) `Constraints` encapsulates a list of functional constraints; `Constraints::project` projects constraints through a projection and returns `None` if constraint columns are not included in the projection. ([Docs.rs][4])

```rust id="qgay3f"
use datafusion::common::{Constraint, Constraints};

// Syntax-level sketch. In production prefer DDL/parser-created constraints
// or a safe factory that validates indices against schema.fields().len().
let constraints = Constraints::new_unverified(vec![
    Constraint::PrimaryKey(vec![0]),      // stream_id
    Constraint::Unique(vec![1, 2]),       // case_id, unit_id
]);
```

Important caveat: docs.rs says `Constraints::new_unverified` is for internal purposes, does not validate its argument, and users are responsible for supplying a valid vector; prefer `Constraints::default` or SQL/planner construction paths when possible. ([Docs.rs][4])

Contract policy:

```text id="smmqi1"
Constraint metadata:
  useful for validation, custom optimizer rules, documentation, functional dependencies

Constraint enforcement:
  not automatic in DataFusion core except nullability
  must be implemented by provider/application/write path
```

---

### S8.2.4 Functional dependencies

A `FunctionalDependence` encodes determinant columns and dependent columns: if two rows have the same determinant key, dependent columns have the same values; if the determinant is unique, its dependent set can be the entire schema and serve as primary-key-like metadata. The struct includes `source_indices`, `target_indices`, `nullable`, and dependency `mode`. ([Docs.rs][5])

```rust id="d1l69d"
use datafusion::common::{
    Dependency, FunctionalDependence, FunctionalDependencies,
};

// stream_id determines all fields: stream_id -> stream_id, case_id, unit_id, mass_flow_kg_h
let fd = FunctionalDependence::new(
    vec![0],              // determinant: stream_id
    vec![0, 1, 2, 3],      // dependent fields
    false,                // determinant nullable?
).with_mode(Dependency::Single);

let fds = FunctionalDependencies::new(vec![fd]);
```

`FunctionalDependencies` can be empty, constructed from a vector of `FunctionalDependence`, constructed from `Constraints` via `new_from_constraints`, extended, joined, projected, and validated; the docs list methods such as `project_functional_dependencies`, `join`, and `is_valid`. ([Docs.rs][6])

Attach to `DFSchema`:

```rust id="yw6yzl"
use datafusion::common::{DFSchema, FunctionalDependencies};

let df_schema = DFSchema::try_from(schema.as_ref().clone())?
    .with_functional_dependencies(fds)?;
```

Contract interpretation:

```text id="lmt0jp"
Primary key:
  unique + non-null determinant
  may imply determinant -> all columns

Unique key:
  unique determinant
  nullable semantics matter

Functional dependency:
  logical relationship that can survive/degrade through joins/projections/aggregations
```

---

### S8.2.5 Uniqueness and primary-key-like metadata

DataFusion’s `Constraint::PrimaryKey(Vec<usize>)` and `Constraint::Unique(Vec<usize>)` are index-based metadata, not automatically enforced database constraints. DataFusion documentation explicitly states that primary/unique constraints are not verified by DataFusion and that table providers requiring this behavior must implement their own checks. ([Apache DataFusion][2])

Contract object:

```rust id="w4prle"
#[derive(Debug, Clone)]
pub struct KeyContract {
    pub name: String,
    pub columns: Vec<String>,
    pub kind: KeyKind,
    pub enforced_by: EnforcementLayer,
    pub nullable_allowed: bool,
}

#[derive(Debug, Clone)]
pub enum KeyKind {
    PrimaryLike,
    Unique,
    ForeignLike,
    FunctionalDependency,
}

#[derive(Debug, Clone)]
pub enum EnforcementLayer {
    DataFusionRuntime,
    TableProvider,
    InsertInto,
    IngestionPipeline,
    ExternalStorage,
    ApplicationOnly,
    InformationalOnly,
}
```

Policy:

```text id="ga789s"
Do not advertise primary-key-like semantics unless:
  duplicate detection exists
  nullability is enforced
  write path rejects violations
  batch append path is checked
  existing data has been audited
```

Validation SQL:

```sql id="o3r96g"
SELECT
  stream_id,
  COUNT(*) AS n
FROM streams
GROUP BY stream_id
HAVING COUNT(*) > 1;
```

---

### S8.2.6 Default values

`TableProvider::get_column_default(&self, column: &str) -> Option<&Expr>` returns a default value expression for a column, if available. ([Docs.rs][1])

```rust id="hkl21n"
use std::collections::HashMap;
use datafusion::logical_expr::Expr;
use datafusion::prelude::*;

#[derive(Debug)]
pub struct ContractTable {
    schema: datafusion::arrow::datatypes::SchemaRef,
    defaults: HashMap<String, Expr>,
}

impl ContractTable {
    fn new(schema: datafusion::arrow::datatypes::SchemaRef) -> Self {
        Self {
            schema,
            defaults: HashMap::from([
                ("source_system".to_string(), lit("smartref")),
            ]),
        }
    }
}

#[async_trait::async_trait]
impl datafusion::datasource::TableProvider for ContractTable {
    fn schema(&self) -> datafusion::arrow::datatypes::SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> datafusion::datasource::TableType {
        datafusion::datasource::TableType::Base
    }

    fn get_column_default(&self, column: &str) -> Option<&Expr> {
        self.defaults.get(column)
    }

    async fn scan(
        &self,
        state: &dyn datafusion::catalog::Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> datafusion::error::Result<std::sync::Arc<dyn datafusion::physical_plan::ExecutionPlan>> {
        todo!()
    }
}
```

Default policy:

```text id="vh8hur"
Default value metadata does not guarantee write enforcement.
If a column is non-nullable and has a default:
  insert_into must synthesize default when missing
  ingestion must backfill old data
  query views must expose default for old physical files
```

---

### S8.2.7 Generated columns

DataFusion does not provide a generic core table-level generated-column enforcement system through `TableProvider`; generated columns should be represented as provider/application contract metadata, views, or write-path projection rules.

Generated column contract:

```rust id="vlbead"
#[derive(Debug, Clone)]
pub struct GeneratedColumnContract {
    pub column: String,
    pub expr_sql: String,
    pub data_type: String,
    pub nullable: bool,
    pub materialized: bool,
    pub enforced_on_write: bool,
}
```

SQL view pattern:

```sql id="b51u5l"
CREATE OR REPLACE VIEW streams_with_daily_flow AS
SELECT
  stream_id,
  mass_flow_kg_h,
  mass_flow_kg_h * 24.0 AS mass_flow_kg_d
FROM streams;
```

Write-path materialization:

```sql id="f1b4bb"
INSERT INTO streams_curated
SELECT
  stream_id,
  mass_flow_kg_h,
  mass_flow_kg_h * 24.0 AS mass_flow_kg_d
FROM streams_stage;
```

Agent rule:

```text id="1h22dz"
Generated column = expression contract.
It is not enforced unless:
  provider insert_into computes/checks it
  ingestion pipeline computes/checks it
  view computes it instead of storing it
```

---

### S8.2.8 Check constraints

DataFusion docs state that foreign-key and check constraints are parsed but not validated or used during query planning. ([Apache DataFusion][2]) Treat check constraints as application/provider validation contracts.

Check contract object:

```rust id="hgg3n8"
#[derive(Debug, Clone)]
pub struct CheckConstraintContract {
    pub name: String,
    pub expr_sql: String,
    pub severity: ConstraintSeverity,
    pub enforced_by: EnforcementLayer,
    pub rejection_code: String,
}

#[derive(Debug, Clone)]
pub enum ConstraintSeverity {
    Error,
    Warning,
    AuditOnly,
}
```

Examples:

```sql id="7cf6k8"
-- validation query
SELECT *
FROM streams
WHERE mass_flow_kg_h < 0.0;
```

```sql id="wgtne3"
-- accepted rows
SELECT *
FROM streams_stage
WHERE mass_flow_kg_h >= 0.0;
```

```sql id="iwe1fp"
-- rejected rows
SELECT
  *,
  'negative_mass_flow' AS reject_reason
FROM streams_stage
WHERE mass_flow_kg_h < 0.0;
```

---

### S8.2.9 Partition columns

Partition columns are table-schema columns derived from file paths or provider metadata, not necessarily file-payload columns. They must be part of the table contract.

Partition contract:

```rust id="5x2whu"
#[derive(Debug, Clone)]
pub struct PartitionColumnContract {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub source: PartitionSource,
    pub cardinality_class: CardinalityClass,
    pub pruning_expected: bool,
}

#[derive(Debug, Clone)]
pub enum PartitionSource {
    HivePath,
    ProviderVirtualColumn,
    PhysicalPayloadColumn,
}

#[derive(Debug, Clone)]
pub enum CardinalityClass {
    Low,
    Moderate,
    High,
    Unknown,
}
```

Policy:

```text id="qaz66h"
Partition columns:
  must be stable across table lifetime
  must not conflict with file payload columns
  should be low/moderate cardinality
  should be validated in DESCRIBE/schema audits
  should be included in schema fingerprint
```

---

### S8.2.10 Ordering

Ordering can be a source/table contract only if guaranteed by the provider or file metadata. DataFusion external-table DDL supports `WITH ORDER`, which declares that the source data is already ordered; the DDL docs warn that if the data is not actually sorted according to the declaration, results may be incorrect, and the clause does not sort the file. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/ddl.html))

Ordering contract:

```rust id="0d0wct"
#[derive(Debug, Clone)]
pub struct OrderingContract {
    pub sort_exprs_sql: Vec<String>,
    pub guaranteed: bool,
    pub enforced_by: EnforcementLayer,
    pub null_ordering: Vec<String>,
    pub stability: OrderingStability,
}

#[derive(Debug, Clone)]
pub enum OrderingStability {
    GloballySorted,
    PartitionSorted,
    FileLocalSorted,
    Unknown,
}
```

Agent rule:

```text id="8gbskp"
Never declare ordering unless:
  provider can prove it
  tests verify it
  file writer guarantees it
  query output uses ORDER BY when required
```

---

## S8.3 DataFusion support boundary

### S8.3.1 Enforced by DataFusion core

```text id="8xm533"
Enforced:
  Field nullability during execution output
  RecordBatch schema/array compatibility
  expression type validity during planning
  projection schema/order at execution boundary
```

DataFusion’s table-constraint docs identify nullability as the only table property enforced by DataFusion; returning nulls for non-nullable columns causes runtime errors, but ingestion is not checked. ([Apache DataFusion][2])

### S8.3.2 Optimizer / planning metadata

```text id="mip97a"
May inform planning/custom planning:
  statistics()
  supports_filters_pushdown()
  constraints()
  functional dependencies
  ordering/distribution properties
  table type
```

`TableProvider::statistics()` returns optional table statistics; docs.rs notes that it is not presently used in mainline DataFusion but can support downstream repositories with specialized optimizer rules such as join reordering. ([Docs.rs][1]) The `Statistics` struct fields are optional/inexact because sources may provide approximate estimates for performance reasons. ([Docs.rs][7])

### S8.3.3 Provider-specific

```text id="4fd6vo"
Provider-specific:
  constraint enforcement
  primary/unique validation
  foreign-key validation
  check-constraint validation
  default materialization
  generated column materialization
  tenant predicates
  row-level access control
  insert/update/delete/truncate semantics
  exactness of statistics
```

`TableProvider::insert_into` returns an `ExecutionPlan` to insert data into a table if supported, and the returned plan should return one `UInt64` column named `count`; DataFusion points to `DataSinkExec` as the common pattern for inserting streams of `RecordBatch`es as files to an object store. ([Docs.rs][1])

### S8.3.4 Application-enforced

```text id="252qyo"
Application-enforced:
  data contracts
  schema versioning
  constraint violation reporting
  rejected-row tables
  data quality gates
  tenant authorization
  lineage and audit
  semantic unit/domain checks
  default value semantics
  generated column consistency
```

---

## S8.4 Contract object model

```rust id="7k7o96"
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableContract {
    pub table_ref: String,
    pub version: String,
    pub fields: Vec<FieldContract>,
    pub constraints: Vec<ConstraintContract>,
    pub functional_dependencies: Vec<FunctionalDependencyContract>,
    pub defaults: Vec<DefaultContract>,
    pub generated_columns: Vec<GeneratedColumnContract>,
    pub checks: Vec<CheckConstraintContract>,
    pub partition_columns: Vec<PartitionColumnContract>,
    pub ordering: Vec<OrderingContract>,
    pub statistics_policy: StatisticsPolicy,
    pub enforcement_policy: EnforcementPolicy,
    pub metadata: HashMap<String, String>,
}
```

Field contract:

```rust id="nc9vyk"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldContract {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub ordinal: usize,
    pub semantic_type: Option<String>,
    pub unit: Option<String>,
    pub default_expr_sql: Option<String>,
    pub generated_expr_sql: Option<String>,
    pub metadata: HashMap<String, String>,
}
```

Constraint contract:

```rust id="2yeltm"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConstraintContract {
    pub name: String,
    pub kind: ConstraintKind,
    pub columns: Vec<String>,
    pub enforced_by: EnforcementLayer,
    pub trusted: bool,
    pub validation_query: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConstraintKind {
    PrimaryKeyLike,
    Unique,
    ForeignKeyLike {
        referenced_table: String,
        referenced_columns: Vec<String>,
    },
    Check {
        expr_sql: String,
    },
}
```

Functional dependency contract:

```rust id="2681xt"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionalDependencyContract {
    pub determinant_columns: Vec<String>,
    pub dependent_columns: Vec<String>,
    pub nullable_determinant: bool,
    pub unique_determinant: bool,
    pub source: FunctionalDependencySource,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FunctionalDependencySource {
    PrimaryKey,
    UniqueConstraint,
    DomainRule,
    ProviderMetadata,
    DerivedFromAggregation,
}
```

---

## S8.5 Translating contract → DataFusion constraints

### S8.5.1 Name-to-index mapping

DataFusion `Constraint` uses field indices, not column names, so table-contract code must map canonical names to `schema.fields()` indices. ([Docs.rs][3])

```rust id="w0hjn7"
use std::collections::HashMap;
use datafusion::arrow::datatypes::Schema;

pub fn field_index_map(schema: &Schema) -> HashMap<String, usize> {
    schema
        .fields()
        .iter()
        .enumerate()
        .map(|(idx, field)| (field.name().clone(), idx))
        .collect()
}
```

### S8.5.2 Contract to constraints

```rust id="25s2mz"
use datafusion::common::{Constraint, Constraints};
use datafusion::arrow::datatypes::Schema;

pub fn constraints_from_contract(
    schema: &Schema,
    contract: &TableContract,
) -> datafusion::error::Result<Constraints> {
    let index = field_index_map(schema);
    let mut out = Vec::new();

    for c in &contract.constraints {
        let indices = c.columns
            .iter()
            .map(|name| {
                index.get(name).copied().ok_or_else(|| {
                    datafusion::error::DataFusionError::Plan(format!(
                        "constraint `{}` references unknown column `{}`",
                        c.name, name
                    ))
                })
            })
            .collect::<datafusion::error::Result<Vec<_>>>()?;

        match c.kind {
            ConstraintKind::PrimaryKeyLike => {
                out.push(Constraint::PrimaryKey(indices));
            }
            ConstraintKind::Unique => {
                out.push(Constraint::Unique(indices));
            }
            _ => {
                // DataFusion 54.1.0 datafusion_common::Constraint only models
                // PrimaryKey and Unique; keep FK/check in app/provider contract.
            }
        }
    }

    // new_unverified is internal/unverified; wrap it with validation like above.
    Ok(Constraints::new_unverified(out))
}
```

Because `Constraints::new_unverified` is documented as internal/unverified and does not validate its argument, the contract layer must validate column existence, index bounds, and nullability before construction. ([Docs.rs][4])

---

## S8.6 Translating constraints → functional dependencies

DataFusion provides `FunctionalDependencies::new_from_constraints(constraints, n_field)`, which creates dependencies from constraints. ([Docs.rs][6]) Conceptually:

```text id="xwr0jd"
PrimaryKey([k])
  → k determines all fields
  → determinant nullable=false
  → mode=Single

Unique([k])
  → k determines all fields
  → determinant nullable=true
  → mode=Single
```

Rust:

```rust id="ba82c8"
use datafusion::common::{Constraints, FunctionalDependencies};

pub fn functional_dependencies_from_constraints(
    constraints: Option<&Constraints>,
    n_fields: usize,
) -> FunctionalDependencies {
    FunctionalDependencies::new_from_constraints(constraints, n_fields)
}
```

Attach to `DFSchema`:

```rust id="8km2pi"
use datafusion::common::{DFSchema, FunctionalDependencies};

let constraints = constraints_from_contract(schema.as_ref(), &table_contract)?;
let fds = FunctionalDependencies::new_from_constraints(Some(&constraints), schema.fields().len());

let df_schema = DFSchema::try_from(schema.as_ref().clone())?
    .with_functional_dependencies(fds)?;
```

Functional-dependency value case:

```text id="41xhkr"
can represent key-like dependencies after projection/join/aggregation
can support planner simplifications or custom optimizer rules
can distinguish primary key from downgraded determinant after joins
can support group-by minimization / aggregate dependency reasoning
```

The `datafusion_common` docs describe helper functions such as `aggregate_functional_dependencies`, `get_required_group_by_exprs_indices`, and `get_target_functional_dependencies`, indicating that functional dependencies participate in aggregate/grouping reasoning. ([Docs.rs][8])

---

## S8.7 Statistics as contract metadata

### S8.7.1 Statistics surface

`Statistics` contains `num_rows`, `total_byte_size`, and `column_statistics`; fields are optional and can be inexact, and column statistics must include one `ColumnStatistics` value per field in the schema. ([Docs.rs][7])

```rust id="3riflb"
use datafusion::common::Statistics;

fn statistics(&self) -> Option<Statistics> {
    Some(self.statistics.clone())
}
```

Provider rule:

```text id="8m3cbf"
Statistics must be:
  cheap
  cached
  conservative
  marked exact/estimated/unknown through Precision
  schema-aligned
```

Contract object:

```rust id="652od9"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatisticsPolicy {
    pub expose_row_count: bool,
    pub expose_byte_size: bool,
    pub expose_column_stats: bool,
    pub exactness_required: bool,
    pub stale_after_seconds: Option<u64>,
    pub sensitive_columns: Vec<String>,
}
```

Security rule:

```text id="7mmjxw"
Statistics can leak sensitive information:
  row counts
  null counts
  min/max values
  distinct estimates
  partition sizes

Do not expose statistics unfiltered in multi-tenant metadata APIs.
```

### S8.7.2 DataFusion 54 statistics API notes

Two statistics-surface changes in DataFusion 54 affect contract code below the provider:

```text
ExecutionPlan::partition_statistics(partition: Option<usize>)
  now returns Result<Arc<Statistics>> (was Result<Statistics>) —
  statistics are shared, not cloned per caller; custom operators must
  return Arc<Statistics> and callers must stop assuming owned values.

PruningStatistics
  relocated to datafusion_common::pruning (datafusion-common/src/pruning.rs),
  and row_counts(&self) no longer takes a column argument — row counts are
  per-container, not per-column.
```

`TableProvider::statistics() -> Option<Statistics>` is unchanged. The full statistics overhaul (statistics registry, NDV/cardinality propagation) is covered in `datafusion_planning_rust.md` §47.

---

## S8.8 Custom provider pattern

### S8.8.1 Contract-backed provider skeleton

```rust id="nmm0ho"
use std::collections::HashMap;
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::catalog::Session;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::common::{Constraints, Statistics};
use datafusion::error::Result;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct ContractBackedProvider {
    schema: SchemaRef,
    constraints: Option<Constraints>,
    statistics: Option<Statistics>,
    defaults: HashMap<String, Expr>,
    contract: TableContract,
}

#[async_trait::async_trait]
impl TableProvider for ContractBackedProvider {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    fn constraints(&self) -> Option<&Constraints> {
        self.constraints.as_ref()
    }

    fn statistics(&self) -> Option<Statistics> {
        self.statistics.clone()
    }

    fn get_column_default(&self, column: &str) -> Option<&Expr> {
        self.defaults.get(column)
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // Build an ExecutionPlan whose schema honors projection
        // and whose scan semantics align with supports_filters_pushdown.
        todo!()
    }
}
```

`TableProvider::constraints()` returns `None` for tables that do not support constraints, and `Some(&Constraints::empty())` for tables that support constraints but have none. ([Docs.rs][1])

---

### S8.8.2 Accurate filter pushdown declarations

`supports_filters_pushdown` must return one value per input filter; each result is `Exact`, `Inexact`, or `Unsupported`, and the default is `Unsupported` for all filters. DataFusion docs emphasize that `Exact`/`Inexact` indicate filters can be applied during scan, while `Unsupported` leaves filtering to DataFusion. ([Docs.rs][1])

```rust id="q7b792"
use datafusion::datasource::TableProviderFilterPushDown;
use datafusion::logical_expr::{Expr, Operator};

fn supports_filters_pushdown(
    &self,
    filters: &[&Expr],
) -> datafusion::error::Result<Vec<TableProviderFilterPushDown>> {
    Ok(filters
        .iter()
        .map(|expr| classify_filter(expr))
        .collect())
}

fn classify_filter(expr: &Expr) -> TableProviderFilterPushDown {
    match expr {
        Expr::BinaryExpr(binary)
            if binary.op == Operator::Eq && references_column(&binary.left, "tenant_id") =>
        {
            TableProviderFilterPushDown::Exact
        }
        Expr::Between(between)
            if between.expr.try_as_col().is_some_and(|c| c.name == "event_date") =>
        {
            TableProviderFilterPushDown::Inexact
        }
        _ => TableProviderFilterPushDown::Unsupported,
    }
}
```

Agent rule:

```text id="08kqth"
Do not advertise exact filter pushdown unless it is semantically exact under SQL null/type semantics.
```

---

### S8.8.3 Tenant predicate injection

Provider-level tenant injection pattern:

```text id="14qmq2"
User query:
  SELECT * FROM streams WHERE site = 'baytown'

Provider enforced predicate:
  tenant_id = current_tenant

Effective scan predicate:
  tenant_id = current_tenant AND site = 'baytown'
```

Provider contract:

```rust id="cqdl2g"
#[derive(Debug, Clone)]
pub struct TenantPolicy {
    pub tenant_column: String,
    pub tenant_id: String,
    pub enforced_by_provider: bool,
    pub expose_tenant_column: bool,
}
```

Rules:

```text id="jb56m7"
Tenant predicate must not be optional.
Do not expose all-tenant statistics to tenant-local users.
Do not claim tenant_id primary key if uniqueness only holds per tenant.
Composite uniqueness should include tenant_id.
```

Composite key:

```rust id="5e4x2x"
// tenant_id, stream_id
Constraint::PrimaryKey(vec![0, 1])
```

---

### S8.8.4 Write enforcement in `insert_into`

`TableProvider::insert_into` accepts an input `ExecutionPlan` and `InsertOp` and returns an `ExecutionPlan`; the returned plan should emit a single `UInt64` `count` column. ([Docs.rs][1])

Write enforcement phases:

```text id="5as371"
1. validate input schema against target schema
2. materialize defaults for missing columns
3. compute generated columns
4. validate non-null fields
5. validate primary/unique constraints
6. validate check constraints
7. validate tenant predicate
8. write data
9. return count
10. emit rejected rows / diagnostics if configured
```

Provider pattern:

```rust id="vbit5p"
async fn insert_into(
    &self,
    state: &dyn Session,
    input: Arc<dyn ExecutionPlan>,
    insert_op: datafusion::logical_expr::dml::InsertOp,
) -> Result<Arc<dyn ExecutionPlan>> {
    // Validate schema.
    validate_insert_schema(input.schema(), self.schema())?;

    // Wrap input in execution node:
    //   ApplyDefaultsExec
    //   GeneratedColumnsExec
    //   ConstraintValidationExec
    //   DataSinkExec
    //
    // Return plan that produces UInt64 count.
    todo!()
}
```

Agent rule:

```text id="kuexde"
If insert_into does not validate a contract, the contract is informational for writes.
```

---

## S8.9 Defaults and generated columns in ingestion

### S8.9.1 Default injection query

```sql id="7x4a3h"
SELECT
  stream_id,
  case_id,
  COALESCE(source_system, 'smartref') AS source_system,
  mass_flow_kg_h
FROM streams_stage;
```

### S8.9.2 Generated-column query

```sql id="4es0qz"
SELECT
  stream_id,
  mass_flow_kg_h,
  mass_flow_kg_h / 1000.0 * 24.0 AS mass_flow_t_d
FROM streams_stage;
```

### S8.9.3 Generated-column consistency check

```sql id="01x73p"
SELECT *
FROM streams_curated
WHERE ABS(mass_flow_t_d - (mass_flow_kg_h / 1000.0 * 24.0)) > 1e-9;
```

Contract policy:

```text id="3n9u7m"
Default:
  fill when missing/null according to policy

Generated:
  recompute or validate
  never trust caller-supplied generated column unless provider explicitly allows override
```

---

## S8.10 Foreign-key-like contracts

DataFusion core does not validate foreign-key constraints and docs say foreign keys/checks are parsed but not validated or used during query planning. ([Apache DataFusion][2]) Model FK-like constraints as application/provider contracts.

```rust id="867n3c"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ForeignKeyLikeContract {
    pub name: String,
    pub local_table: String,
    pub local_columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub validation_query: String,
    pub enforcement_layer: EnforcementLayer,
}
```

Validation query:

```sql id="xmke9b"
SELECT s.*
FROM streams AS s
LEFT ANTI JOIN units AS u
  ON s.unit_id = u.unit_id;
```

Deployment options:

```text id="pbfbju"
Audit-only:
  run validation query, emit metrics

Strict ingestion:
  reject rows with missing reference

Dimension-late-arrival:
  quarantine rows until dimension arrives

Provider-enforced:
  remote/storage layer checks before insert
```

---

## S8.11 Table contract validation queries

### Non-null

```sql id="pwim8m"
SELECT COUNT(*) AS bad_rows
FROM streams
WHERE stream_id IS NULL;
```

### Primary key uniqueness

```sql id="4tscn4"
SELECT
  stream_id,
  COUNT(*) AS n
FROM streams
GROUP BY stream_id
HAVING COUNT(*) > 1;
```

### Composite uniqueness

```sql id="8g5uor"
SELECT
  tenant_id,
  stream_id,
  COUNT(*) AS n
FROM streams
GROUP BY tenant_id, stream_id
HAVING COUNT(*) > 1;
```

### Check constraint

```sql id="88t13h"
SELECT *
FROM streams
WHERE mass_flow_kg_h < 0.0;
```

### Foreign-key-like

```sql id="n0vbu7"
SELECT s.*
FROM streams AS s
LEFT ANTI JOIN units AS u
  ON s.unit_id = u.unit_id;
```

### Generated-column consistency

```sql id="a9vn3v"
SELECT *
FROM streams
WHERE ABS(mass_flow_t_d - mass_flow_kg_h / 1000.0 * 24.0) > 1e-9;
```

### Partition column validity

```sql id="txd1ar"
SELECT *
FROM streams
WHERE event_date IS NULL
   OR site IS NULL;
```

---

## S8.12 Contract diagnostics

### S8.12.1 Constraint violation

```json id="c41imr"
{
  "error_class": "contract_constraint_violation",
  "phase": "insert_validation",
  "table": "prod.curated.streams",
  "constraint": "pk_stream_id",
  "kind": "PrimaryKeyLike",
  "columns": ["stream_id"],
  "violating_key": {"stream_id": "S-001"},
  "violation_count": 2,
  "enforced_by": "TableProvider.insert_into",
  "decision": "reject_batch"
}
```

### S8.12.2 Misdeclared constraint

```json id="6cjrx8"
{
  "error_class": "untrusted_constraint_metadata",
  "phase": "provider_registration",
  "table": "prod.curated.streams",
  "constraint": "unique_stream_id",
  "reason": "provider declared Unique but validation query found duplicates",
  "decision": "register_without_constraint",
  "suggested_fix": "fix data or remove constraint declaration"
}
```

### S8.12.3 Unsafe statistics

```json id="2robyw"
{
  "error_class": "unsafe_statistics_exposure",
  "phase": "metadata_api",
  "table": "tenant_acme.curated.streams",
  "statistic": "column_min_max",
  "column": "margin_usd_bbl",
  "reason": "column classified confidential; min/max may leak sensitive values",
  "decision": "redact"
}
```

### S8.12.4 Inexact pushdown misadvertised

```json id="7y3mlh"
{
  "error_class": "incorrect_pushdown_exactness",
  "phase": "provider_validation",
  "filter": "region = 'US'",
  "declared": "Exact",
  "observed": "Inexact",
  "impact": "incorrect query results possible if residual filter skipped",
  "decision": "reject_provider"
}
```

---

## S8.13 Testing matrix

```text id="2nn5ka"
Provider contract tests:
  schema() field names/types/nullability
  constraints() none/empty/populated semantics
  constraints indices valid against schema
  primary key validation query returns zero rows
  unique validation query returns zero rows
  foreign-key-like validation query returns zero rows or allowed late arrivals
  check validation query returns zero rows
  get_column_default returns expected Expr
  generated-column consistency query returns zero rows
  statistics() schema-aligned and conservative
  supports_filters_pushdown returns vector length == filters.len()
  Exact pushdown verified against residual filtering
  insert_into rejects invalid rows
  insert_into fills defaults
  insert_into returns count UInt64
```

SQL test harness:

```rust id="itms0m"
pub async fn assert_no_rows(
    ctx: &datafusion::prelude::SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let batches = ctx.sql(sql).await?.collect().await?;
    let rows: usize = batches.iter().map(|b| b.num_rows()).sum();

    assert_eq!(rows, 0, "contract validation query returned rows:\n{sql}");
    Ok(())
}
```

---

## S8.14 Best-practice deployment advisory

```text id="k41pam"
Contract modeling:
  keep Arrow Schema for physical field contract
  keep TableContract for constraints/defaults/generated/check/partition/order semantics
  derive DataFusion Constraints only for supported PrimaryKey/Unique metadata
  derive FunctionalDependencies from constraints where useful
  keep FK/check/default/generated enforcement in provider/application layer
```

```text id="bickor"
Provider implementation:
  constraints() returns None unless constraints are supported
  Some(Constraints::default()) means constraints supported but absent
  statistics() returns conservative cached values or None
  get_column_default returns Expr only when provider/write path honors it
  insert_into enforces target schema/defaults/constraints if write support is advertised
```

```text id="s7064f"
Security:
  tenant_id should be part of uniqueness for tenant-scoped tables
  tenant predicate injection must be provider-enforced, not advisory
  statistics may leak sensitive data; redact by policy
  constraint diagnostics should avoid leaking unauthorized row values
```

```text id="fnz9qb"
Query/planning:
  do not rely on DataFusion core to enforce primary/unique/FK/check constraints
  use functional dependencies for logical reasoning only after validation
  do not expose exact pushdown/order/stat claims unless guaranteed
```

---

## S8.15 Anti-pattern inventory

```text id="iq6nt2"
Contract anti-patterns:
  marking a column non-nullable without ingestion/provider validation
  declaring primary key without duplicate audit
  declaring unique constraint on nullable business key without null policy
  assuming DataFusion enforces primary/unique/check/FK constraints
  exposing constraints() with stale/wrong indices
  returning Some(Constraints::empty()) when constraints are not supported
  using Constraints::new_unverified without index validation
  advertising statistics as exact when approximate
  exposing sensitive min/max/null-count stats to tenants
  claiming Exact filter pushdown for approximate search
  applying limit before inexact residual filtering
  defining defaults but not applying them in insert_into
  defining generated columns but trusting caller-provided values
  changing partition columns without updating contract/version
  declaring WITH ORDER or natural ordering without proof
  implementing insert_into without contract validation
```

---

## S8.16 Agent checklist

```text id="f2l1rp"
[ ] Build TableContract, not just Schema.
[ ] Fields include names, types, nullability, ordinal, metadata.
[ ] Nullability is treated as enforced at runtime but not ingestion.
[ ] Primary/unique constraints are declared only after validation.
[ ] Constraint indices are derived from schema names and bounds-checked.
[ ] Functional dependencies are derived from trusted constraints or domain rules.
[ ] Defaults are represented as Expr and enforced in write path.
[ ] Generated columns are recomputed or validated, not blindly trusted.
[ ] Check constraints have validation queries and enforcement layer.
[ ] Foreign-key-like contracts have anti-join validation queries.
[ ] Partition columns are explicit contract members.
[ ] Ordering claims are made only when guaranteed.
[ ] Statistics are conservative, schema-aligned, and access-controlled.
[ ] supports_filters_pushdown exactness is tested.
[ ] insert_into enforces schema/defaults/constraints or write is disabled.
[ ] Violations are emitted as contract diagnostics.
```

## S8.17 Minimal contract-to-provider pattern

```rust id="zlcnx9"
#[derive(Debug, Clone)]
pub struct EnforcementPolicy {
    pub enforce_nullability_on_insert: bool,
    pub enforce_unique_on_insert: bool,
    pub enforce_checks_on_insert: bool,
    pub enforce_generated_columns: bool,
    pub inject_tenant_predicate: bool,
}

#[derive(Debug)]
pub struct GovernedTableProvider {
    pub schema: datafusion::arrow::datatypes::SchemaRef,
    pub constraints: Option<datafusion::common::Constraints>,
    pub statistics: Option<datafusion::common::Statistics>,
    pub defaults: std::collections::HashMap<String, datafusion::logical_expr::Expr>,
    pub contract: TableContract,
    pub enforcement: EnforcementPolicy,
}

impl GovernedTableProvider {
    pub fn validate_contract_metadata(&self) -> datafusion::error::Result<()> {
        if let Some(constraints) = &self.constraints {
            for constraint in constraints.iter() {
                match constraint {
                    datafusion::common::Constraint::PrimaryKey(indices)
                    | datafusion::common::Constraint::Unique(indices) => {
                        for idx in indices {
                            if *idx >= self.schema.fields().len() {
                                return Err(datafusion::error::DataFusionError::Plan(format!(
                                    "constraint index {idx} out of bounds for schema with {} fields",
                                    self.schema.fields().len()
                                )));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
```

Core operating rule: **DataFusion can carry table-contract metadata, but application/provider code must decide what is informational, what is optimizer metadata, and what is actually enforced.**

[1]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html "TableProvider in datafusion::datasource - Rust"
[2]: https://datafusion.apache.org/library-user-guide/table-constraints.html "Table Constraint Enforcement — Apache DataFusion  documentation"
[3]: https://docs.rs/datafusion/latest/datafusion/common/enum.Constraint.html "Constraint in datafusion::common - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/common/struct.Constraints.html "Constraints in datafusion::common - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/common/struct.FunctionalDependence.html "FunctionalDependence in datafusion::common - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/common/struct.FunctionalDependencies.html "FunctionalDependencies in datafusion::common - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/common/struct.Statistics.html "Statistics in datafusion::common - Rust"
[8]: https://docs.rs/datafusion-common "datafusion_common - Rust"


# DataFusion Advanced — S9) Catalog schema management, remote metastores, and `information_schema`

## S9.0 Objective

Turn DataFusion catalog/schema/table organization into an **operational metadata plane**:

```text
CatalogProviderList
  └─ CatalogProvider                  catalog / tenant / environment / metastore namespace
      └─ SchemaProvider               workspace / lifecycle layer / authorization domain
          └─ TableProvider            table object / schema contract / scan + write boundary
              ├─ schema()
              ├─ constraints()
              ├─ statistics()
              ├─ supports_filters_pushdown()
              ├─ scan(...)
              └─ insert_into(...)
```

The attached document already covers this area well: catalog hierarchy, `CatalogProvider`, `SchemaProvider`, `TableProvider`, `information_schema`, testing, security, remote metadata caching, and production deployment. The remaining value is turning that coverage into a migration-grade control plane: deterministic namespace policy, explicit persistence semantics, cached remote snapshots, audited DDL, tenant-aware metadata visibility, and CI-stable introspection. 

DataFusion’s catalog API is explicitly metadata-oriented: the `CatalogProvider` docs state that planning/execution requires metadata about which schemas/tables exist, their columns/types, and how to access data; the hierarchy is `CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider`. ([Docs.rs][1])

---

## S9.1 Catalog mental model

```text
SQL identifier resolution:
  unqualified table:
    table
      → default_catalog.default_schema.table

  schema-qualified table:
    schema.table
      → default_catalog.schema.table

  catalog-qualified table:
    catalog.schema.table
      → catalog.schema.table

DataFrame access:
  ctx.table("catalog.schema.table")
  ctx.table("schema.table")
  ctx.table("table")

Catalog API:
  catalog_names()
  catalog(name)
  schema_names()
  schema(name)
  table_names()
  table(name).await
  table_exist(name)
```

DataFusion’s config settings include `datafusion.catalog.default_catalog`, defaulting to `datafusion`, and `datafusion.catalog.default_schema`, defaulting to `public`; these determine how unspecified SQL table names resolve. The same config surface includes `datafusion.catalog.create_default_catalog_and_schema`, which defaults to true. ([Apache DataFusion][2])

Agent invariant:

```text
Generated production SQL should prefer fully-qualified table references in multi-catalog / multi-tenant systems.

Ad hoc SQL can use defaults.

Service SQL should bind tenant/schema server-side, not trust user-provided qualification.
```

---

## S9.2 Namespace policy

### S9.2.1 Catalog-per-tenant

```text
CatalogProviderList
  ├─ tenant_acme
  │   ├─ raw
  │   ├─ curated
  │   ├─ semantic
  │   └─ temp
  ├─ tenant_zenith
  │   ├─ raw
  │   ├─ curated
  │   ├─ semantic
  │   └─ temp
  └─ platform
      ├─ reference
      ├─ diagnostics
      └─ admin
```

Use when:

```text
tenant data visibility differs
object-store credentials differ
table sets differ
schema evolution cadence differs
statistics visibility differs
information_schema must be tenant-filtered
catalog-level authorization is natural
```

Rust construction sketch:

```rust
use std::sync::Arc;
use datafusion::prelude::*;
use datafusion::catalog::{MemoryCatalogProvider, MemorySchemaProvider, CatalogProvider};

pub fn build_tenant_context(tenant_catalog_name: &str) -> datafusion::error::Result<SessionContext> {
    let mut config = SessionConfig::new();
    config.set_str("datafusion.catalog.default_catalog", tenant_catalog_name)?;
    config.set_str("datafusion.catalog.default_schema", "semantic")?;
    config.set_bool("datafusion.catalog.information_schema", false)?;

    let ctx = SessionContext::new_with_config(config);

    let catalog = Arc::new(MemoryCatalogProvider::new());
    catalog.register_schema("raw", Arc::new(MemorySchemaProvider::new()))?;
    catalog.register_schema("curated", Arc::new(MemorySchemaProvider::new()))?;
    catalog.register_schema("semantic", Arc::new(MemorySchemaProvider::new()))?;
    catalog.register_schema("temp", Arc::new(MemorySchemaProvider::new()))?;

    ctx.register_catalog(tenant_catalog_name, catalog);

    Ok(ctx)
}
```

Deployment rule:

```text
Catalog-per-tenant is preferable when tenant isolation is a metadata/security boundary.

Do not emulate tenant isolation only through table-name prefixes:
  acme_streams
  zenith_streams

Prefer:
  tenant_acme.curated.streams
  tenant_zenith.curated.streams
```

The attached security section makes the same point: catalog/schema/provider layers control visibility, and `information_schema` should be exposed only where metadata visibility is allowed. 

---

### S9.2.2 Schema-per-workspace

```text
catalog = tenant_acme
schemas:
  workspace_planning
  workspace_operations
  workspace_sandbox_paul
  workspace_ertc_abstract
```

Use when:

```text
same tenant has multiple projects/workspaces
schemas share credentials and tenant policies
users need workspace-local temporary/semantic objects
catalog explosion should be avoided
```

Policy:

```text
SchemaProvider = workspace metadata boundary.
SchemaProvider::table_names() reveals workspace tables.
SchemaProvider::table(name).await reveals provider-level table object.
Schema ownership should be explicit.
```

The `SchemaProvider` trait represents a schema containing named tables, with `table_names()`, async `table(name)`, and `table_exist(name)` as required methods; `owner_name()` is a provided method reported as part of information-schema schemata metadata. ([Docs.rs][3])

---

### S9.2.3 Schema-per-lifecycle-layer

Recommended lifecycle schemas:

```text
raw
  source-shaped tables
  mostly source names/types
  minimal transformations
  high drift risk
  restricted write access

staging
  all-Utf8 or parse-normalized staging
  rejected-row tables
  validation views

curated
  canonical fields
  stable Arrow/DataFusion types
  partitioned lake tables
  governed table contracts

semantic
  user-facing views
  business objects
  aliases, computed fields, joins, masking

diagnostics
  schema audits
  profile tables
  rejected rows
  lineage manifests

temp
  session/job-local scratch objects
  short TTL
```

Example naming:

```text
tenant_acme.raw.streams_stage
tenant_acme.staging.streams_rejected
tenant_acme.curated.streams
tenant_acme.semantic.stream_balances
tenant_acme.diagnostics.schema_audit_runs
tenant_acme.temp.run_20260524_streams_preview
```

Agent rule:

```text
raw/staging can tolerate source messiness.
curated/semantic cannot.

Never expose raw schema drift through semantic views without explicit casts/aliases.
```

---

## S9.3 Catalog traits and responsibilities

### S9.3.1 `CatalogProviderList`

Conceptual trait responsibility:

```text
CatalogProviderList:
  register catalog name → CatalogProvider
  list catalog names
  retrieve catalog by name
```

Use when:

```text
you need global catalog lookup control
you need tenant-aware catalog resolution
you need remote catalog-list snapshots
you need to restrict catalog discovery
```

Most applications can register catalogs directly on `SessionContext` rather than implementing a custom `CatalogProviderList`. The attached section says to implement `CatalogProviderList` only when controlling catalog lookup globally; otherwise `SessionContext::register_catalog` is usually sufficient. 

---

### S9.3.2 `CatalogProvider`

Conceptual responsibility:

```text
CatalogProvider:
  schema_names() -> Vec<String>
  schema(name) -> Option<Arc<dyn SchemaProvider>>
  register_schema(...)
  deregister_schema(...)
```

Docs.rs describes `CatalogProvider` as a catalog comprising named schemas, and its provided methods include `register_schema(name, schema)` and `deregister_schema(name, cascade)`. It also cautions that DataFusion’s built-in `MemoryCatalogProvider` has no persistence, and that complex catalog management is system-specific rather than a general-purpose DataFusion feature. ([Docs.rs][1])

Contract:

```text
CatalogProvider owns:
  schema namespace
  schema visibility
  schema registration/deletion policy
  schema owner metadata
  persistence semantics
  snapshot/refresh semantics for remote catalogs
```

Rust sketch:

```rust
use std::sync::Arc;
use datafusion::catalog::{CatalogProvider, SchemaProvider};
use datafusion::common::Result;

#[derive(Debug)]
pub struct GovernedCatalogProvider {
    name: String,
    schemas: dashmap::DashMap<String, Arc<dyn SchemaProvider>>,
    read_only: bool,
}

impl GovernedCatalogProvider {
    pub fn new(name: impl Into<String>, read_only: bool) -> Self {
        Self {
            name: name.into(),
            schemas: dashmap::DashMap::new(),
            read_only,
        }
    }
}
```

Implementation note: use current docs.rs signatures for the pinned DataFusion version; the conceptual contract is stable, but crate/module paths can move across major releases.

---

### S9.3.3 `SchemaProvider`

Conceptual responsibility:

```text
SchemaProvider:
  table_names() -> Vec<String>
  table(name).await -> Option<Arc<dyn TableProvider>>
  table_exist(name) -> bool
  register_table(...)
  deregister_table(...)
  owner_name()
```

`SchemaProvider::table(name)` is async and returns `Option<Arc<dyn TableProvider>>`; docs.rs says this makes simple providers easier and supports cases where listing tables is easy but reading table details such as statistics requires non-trivial access. ([Docs.rs][3])

Contract:

```text
SchemaProvider owns:
  table namespace
  table registration/deletion policy
  table listing order
  table lookup semantics
  table visibility
  owner metadata
  async hydration boundary
```

Agent rule:

```text
table_names() should be cheap and deterministic.
table(name).await is the only acceptable async table-hydration boundary.
Do not perform network lookup inside every table_names() unless explicitly cached.
```

---

### S9.3.4 `TableProvider`

Conceptual responsibility:

```text
TableProvider:
  schema()
  table_type()
  constraints()
  statistics()
  supports_filters_pushdown()
  scan(...)
  insert_into(...)
```

Docs.rs describes `TableProvider` as a table source that provides Arrow `RecordBatch`es and important planning information: schema, filter-pushdown capability, and scan execution plan. ([Docs.rs][4])

Contract:

```text
TableProvider owns:
  table schema contract
  scan contract
  filter/projection/limit pushdown truthfulness
  statistics exposure
  write semantics
  tenant predicate enforcement
  row/column access policy
```

Governance hooks from the attached document include hiding unauthorized columns in `schema()`, injecting tenant filters in `scan()`, advertising filter pushdown accurately, avoiding sensitive-statistics leakage, and enforcing write permissions in `insert_into()`. 

---

## S9.4 Remote metastore pattern

### S9.4.1 Core constraint

DataFusion’s planning APIs are not generally async, so remote catalog information should be available from an in-memory snapshot rather than lazily fetched through repeated network calls during planning. The official `CatalogProvider` docs specifically warn that doing remote procedure calls for all catalog accesses during query planning would cause multiple network calls per plan and poor planning performance; they recommend providing an in-memory snapshot and batching remote access. ([Docs.rs][1])

Correct pattern:

```text
remote metastore
  → batch load / refresh
  → immutable metadata snapshot
  → local CatalogProvider / SchemaProvider lookup
  → async table(name) hydrates detail only when needed
  → query planned from cached snapshot
```

Incorrect pattern:

```text
schema_names() -> network call
table_names() -> network call
table_exist(name) -> network call
schema(name) -> network call
every query planning path -> many network calls
```

---

### S9.4.2 Metadata snapshot model

```rust
use std::collections::BTreeMap;
use std::sync::Arc;
use datafusion::arrow::datatypes::SchemaRef;

#[derive(Debug, Clone)]
pub struct MetastoreSnapshot {
    pub snapshot_id: String,
    pub loaded_at_epoch_ms: i64,
    pub expires_at_epoch_ms: Option<i64>,
    pub catalog_names: Vec<String>,
    pub catalogs: BTreeMap<String, CatalogSnapshot>,
}

#[derive(Debug, Clone)]
pub struct CatalogSnapshot {
    pub name: String,
    pub owner: Option<String>,
    pub schemas: BTreeMap<String, SchemaSnapshot>,
}

#[derive(Debug, Clone)]
pub struct SchemaSnapshot {
    pub name: String,
    pub owner: Option<String>,
    pub tables: BTreeMap<String, TableSnapshot>,
}

#[derive(Debug, Clone)]
pub struct TableSnapshot {
    pub name: String,
    pub table_type: String,
    pub schema: SchemaRef,
    pub schema_fingerprint: String,
    pub location: Option<String>,
    pub provider_kind: ProviderKind,
    pub statistics_summary: Option<StatisticsSummary>,
    pub constraints_summary: Option<ConstraintsSummary>,
    pub visibility: VisibilityPolicy,
}
```

Use `BTreeMap` or sorted vectors for deterministic listing. The attached test matrix explicitly requires `catalog_names`, `schema_names`, and `table_names` to be sorted or stable enough for snapshots. 

---

### S9.4.3 Refresh interval

```rust
#[derive(Debug, Clone)]
pub struct RefreshPolicy {
    pub mode: RefreshMode,
    pub min_interval_ms: u64,
    pub max_staleness_ms: u64,
    pub refresh_on_error: bool,
    pub allow_stale_on_refresh_failure: bool,
}

#[derive(Debug, Clone)]
pub enum RefreshMode {
    Manual,
    Periodic,
    OnDemandBeforePlanning,
    WatchBased,
}
```

Recommended defaults:

```text
CLI / local:
  Manual refresh acceptable

Batch ETL:
  refresh once at job start
  freeze snapshot for job duration

SQL service:
  periodic refresh + explicit invalidation
  freeze snapshot per query
  allow stale reads only within configured TTL

Migration/admin:
  force refresh before DDL/migration
  reject stale snapshot for destructive operations
```

---

### S9.4.4 Invalidation

Invalidation triggers:

```text
DDL mutation:
  CREATE SCHEMA
  DROP/REGISTER/DEREGISTER TABLE
  CREATE VIEW
  DROP VIEW

metastore event:
  table schema version changed
  table deleted
  location changed
  partition spec changed

object-store event:
  new table root committed
  manifest changed
  Delta/Iceberg-like metadata changed

security event:
  role/tenant grants changed
  table visibility changed
  column policy changed
```

Invalidation object:

```rust
#[derive(Debug, Clone)]
pub struct MetadataInvalidation {
    pub scope: InvalidationScope,
    pub reason: String,
    pub requested_by: String,
    pub epoch_ms: i64,
}

#[derive(Debug, Clone)]
pub enum InvalidationScope {
    All,
    Catalog(String),
    Schema { catalog: String, schema: String },
    Table { catalog: String, schema: String, table: String },
}
```

Agent rule:

```text
Refresh and invalidation must be explicit metadata operations.
Do not rely on eventual accidental cache expiry for DDL correctness.
```

---

### S9.4.5 Stale-read semantics

```rust
#[derive(Debug, Clone)]
pub enum StaleReadPolicy {
    RejectIfStale,
    AllowStaleForSelect,
    AllowStaleForExplainOnly,
    AllowStaleWithWarning,
}
```

Recommended:

```text
SELECT:
  allow stale snapshot only inside TTL if table format supports it

INSERT / DML:
  reject stale metadata unless provider/table format has conflict detection

DDL / migration:
  reject stale metadata

information_schema:
  show snapshot_id and loaded_at in diagnostics if possible
```

Diagnostic payload:

```json
{
  "warning_class": "stale_catalog_snapshot",
  "catalog": "tenant_acme",
  "snapshot_id": "meta-20260524T120000Z",
  "loaded_at": "2026-05-24T12:00:00Z",
  "age_ms": 45000,
  "max_staleness_ms": 60000,
  "policy": "AllowStaleForSelect"
}
```

---

## S9.5 Deterministic listings

### S9.5.1 Rules

```text
catalog_names()
  sorted ascending by canonical catalog name

schema_names()
  sorted ascending by canonical schema name

table_names()
  sorted ascending by canonical table name

information_schema outputs
  tests should ORDER BY table_catalog, table_schema, table_name, ordinal_position
```

The `SchemaProvider` docs only require `table_names()` to retrieve available table names; they do not guarantee ordering, so custom providers should enforce deterministic ordering for snapshot tests and agent reproducibility. ([Docs.rs][3])

Rust pattern:

```rust
pub fn sorted_keys<K: Ord + Clone, V>(map: &std::collections::BTreeMap<K, V>) -> Vec<K> {
    map.keys().cloned().collect()
}
```

DashMap pattern:

```rust
pub fn deterministic_table_names(tables: &dashmap::DashMap<String, Arc<dyn TableProvider>>) -> Vec<String> {
    let mut names = tables.iter().map(|entry| entry.key().clone()).collect::<Vec<_>>();
    names.sort();
    names
}
```

Agent rule:

```text
Never snapshot-test unordered catalog/schema/table listings.
Always sort provider outputs or ORDER BY information_schema queries.
```

---

## S9.6 `information_schema`

### S9.6.1 Enable/disable

`datafusion.catalog.information_schema` defaults to `false` and controls whether DataFusion provides `information_schema` virtual tables for displaying schema information. ([Apache DataFusion][2])

Rust config:

```rust
use datafusion::prelude::*;

let mut config = SessionConfig::new();
config.set_bool("datafusion.catalog.information_schema", true)?;

let ctx = SessionContext::new_with_config(config);
```

SQL config:

```sql
SET datafusion.catalog.information_schema = true;
```

Service rule:

```text
Enable only in trusted/admin contexts unless metadata visibility is explicitly governed.
```

---

### S9.6.2 Tables and columns

DataFusion’s information schema docs state that table/view metadata can be accessed with ISO SQL `information_schema` views or DataFusion-specific `SHOW TABLES` and `SHOW COLUMNS`. `information_schema.tables` lists table catalog/schema/name/type, and `information_schema.columns` exposes column-level metadata such as table catalog/schema/name, column name, data type, and nullability. ([Apache DataFusion][5])

Tables:

```sql
SHOW TABLES;

SELECT
  table_catalog,
  table_schema,
  table_name,
  table_type
FROM information_schema.tables
ORDER BY table_catalog, table_schema, table_name;
```

Columns:

```sql
SHOW COLUMNS FROM streams;

SELECT
  table_catalog,
  table_schema,
  table_name,
  column_name,
  data_type,
  is_nullable
FROM information_schema.columns
WHERE table_schema = 'curated'
  AND table_name = 'streams'
ORDER BY ordinal_position;
```

---

### S9.6.3 Views

```sql
SELECT
  table_catalog,
  table_schema,
  table_name,
  table_type
FROM information_schema.tables
WHERE table_type = 'VIEW'
ORDER BY table_catalog, table_schema, table_name;
```

DataFusion’s `SHOW TABLES` example includes `information_schema.tables`, `information_schema.views`, and `information_schema.columns` as views in the `information_schema` schema. ([Apache DataFusion][5])

---

### S9.6.4 Settings

DataFusion’s information schema docs state that `SHOW ALL` and `information_schema.df_settings` expose session configuration options; the example includes settings such as `datafusion.execution.batch_size`, `datafusion.execution.time_zone`, and optimizer settings. ([Apache DataFusion][5])

```sql
SHOW ALL;

SELECT
  name,
  setting
FROM information_schema.df_settings
ORDER BY name;
```

Security risk:

```text
df_settings can reveal runtime configuration.
Configuration can encode operational/security posture.
Expose only in trusted/admin contexts.
```

The attached document explicitly calls out `information_schema.df_settings` exposure as an anti-pattern for untrusted users. 

---

### S9.6.5 Functions / routines

DataFusion’s information schema docs state that `SHOW FUNCTIONS` can list available functions and that function/routine metadata is exposed through information-schema routines/parameters views. ([Apache DataFusion][5])

```sql
SHOW FUNCTIONS LIKE '%date%';
```

Security risk:

```text
function inventory can reveal:
  UDF names
  internal capabilities
  diagnostic functions
  remote/side-effecting functions
  governance bypass surfaces
```

Agent rule:

```text
Public SQL service should use function allowlists and restrict metadata visibility for routines.
```

---

## S9.7 Filtered information schema

### S9.7.1 Problem

Default `information_schema` exposes catalog metadata for the session context. In multi-tenant or governed contexts, metadata itself can be sensitive.

Leakage examples:

```text
table names:
  acquisition_targets
  layoffs_model
  sanctions_hits

column names:
  ssn
  acquisition_price
  margin_usd_bbl

settings:
  runtime memory limits
  object-store/caching behavior
  feature toggles

function names:
  internal_udf_score_counterparty
  remote_fetch_secret
```

### S9.7.2 Filtering strategies

```text
Strategy A — disable:
  datafusion.catalog.information_schema = false

Strategy B — per-tenant SessionContext:
  only tenant-visible catalogs/tables registered

Strategy C — custom catalog/schema providers:
  table_names/table(name) only expose authorized objects

Strategy D — metadata views:
  expose your own safe metadata tables/views instead of built-in information_schema

Strategy E — plan validation:
  reject queries referencing information_schema unless role permits
```

### S9.7.3 Safe metadata view

```sql
CREATE VIEW safe_metadata_tables AS
SELECT
  'curated' AS table_schema,
  'streams' AS table_name,
  'BASE TABLE' AS table_type;
```

Better: implement a governed `TableProvider` for safe metadata.

Rust object:

```rust
#[derive(Debug, Clone)]
pub struct MetadataVisibilityPolicy {
    pub allow_information_schema: bool,
    pub allow_df_settings: bool,
    pub allow_function_inventory: bool,
    pub visible_catalogs: Vec<String>,
    pub visible_schemas: Vec<String>,
    pub visible_tables: Vec<String>,
    pub visible_column_classes: Vec<String>,
}
```

---

## S9.8 DDL semantics

### S9.8.1 DDL surface

DataFusion supports DDL through the catalog API; the catalog guide notes that `CREATE TABLE` and similar DDL use the catalog API, while DML such as `INSERT INTO` is tied to `TableProvider` behavior. ([Apache DataFusion][6])

Relevant DDL categories:

```text
CREATE DATABASE / CATALOG-like namespace
CREATE SCHEMA
CREATE EXTERNAL TABLE
CREATE TABLE AS SELECT
CREATE VIEW
DROP TABLE
DROP VIEW
```

Agent interpretation:

```text
DDL mutates catalog metadata.
DDL does not automatically mean durable metastore mutation.
DDL does not automatically rewrite external files.
DDL does not automatically define transactional lifecycle semantics.
```

---

### S9.8.2 Read-only catalogs

Use cases:

```text
production semantic catalog
external metastore mirror
read-only lakehouse snapshot
tenant-restricted service
regulated reporting view
```

Implementation policy:

```text
register_schema:
  return explicit read-only error

deregister_schema:
  return explicit read-only error

SchemaProvider::register_table:
  return explicit read-only error

SchemaProvider::deregister_table:
  return explicit read-only error

insert_into:
  unsupported unless write-authorized table provider
```

Diagnostic:

```json
{
  "error_class": "read_only_catalog_mutation_denied",
  "operation": "CREATE TABLE",
  "catalog": "prod",
  "schema": "semantic",
  "reason": "catalog is governed read-only for user SQL"
}
```

---

### S9.8.3 Mutable catalogs

Use cases:

```text
developer sandbox
migration/admin tool
temporary ETL workspace
interactive notebook
test harness
```

Requirements:

```text
audit every mutation
define persistence semantics
define concurrent mutation behavior
define replace semantics
define lifecycle/retention policy
define DDL authorization
```

Mutation audit object:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogAuditEvent {
    pub event_id: String,
    pub epoch_ms: i64,
    pub actor: String,
    pub operation: CatalogOperation,
    pub object_ref: ObjectRef,
    pub old_fingerprint: Option<String>,
    pub new_fingerprint: Option<String>,
    pub sql_text_redacted: Option<String>,
    pub result: AuditResult,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CatalogOperation {
    RegisterCatalog,
    RegisterSchema,
    DeregisterSchema,
    RegisterTable,
    DeregisterTable,
    CreateView,
    DropView,
    ReplaceTable,
}
```

---

### S9.8.4 Concurrent DDL

Concurrency hazards:

```text
two CREATE TABLE same name
DROP while SELECT planning
schema refresh while query uses old snapshot
catalog swap while information_schema query runs
table provider replaced while write is executing
remote metastore refresh races with DDL
```

Recommended model:

```text
Snapshot isolation for planning:
  query captures catalog snapshot id
  all table refs resolved from same snapshot
  execution uses resolved TableProvider arcs

Atomic metadata mutation:
  catalog mutation creates new snapshot
  readers continue with old snapshot
  new readers see new snapshot

DDL serialization:
  schema/table mutation lock per catalog/schema
  audit event emitted after commit
```

Rust conceptual snapshot pointer:

```rust
use std::sync::Arc;
use arc_swap::ArcSwap;

#[derive(Debug)]
pub struct SnapshotCatalog {
    snapshot: ArcSwap<MetastoreSnapshot>,
}

impl SnapshotCatalog {
    pub fn load_snapshot(&self) -> Arc<MetastoreSnapshot> {
        self.snapshot.load_full()
    }

    pub fn swap_snapshot(&self, next: Arc<MetastoreSnapshot>) {
        self.snapshot.store(next);
    }
}
```

---

### S9.8.5 Object lifecycle

Object state model:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CatalogObjectState {
    Active,
    Deprecated,
    Hidden,
    Tombstoned,
    Retired,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogObjectLifecycle {
    pub created_at_epoch_ms: i64,
    pub created_by: String,
    pub deprecated_at_epoch_ms: Option<i64>,
    pub retired_at_epoch_ms: Option<i64>,
    pub state: CatalogObjectState,
    pub replacement_ref: Option<String>,
}
```

Policy:

```text
DROP TABLE:
  default: deregister catalog object
  external data deletion: separate storage lifecycle action
  tombstone recommended for audit/migration windows

CREATE OR REPLACE:
  new fingerprint
  old fingerprint captured
  compatibility report required
```

The attached DDL guidance stresses that `DROP TABLE` removes catalog registration, not external files, and that default `CREATE TABLE` semantics are in-memory unless custom catalog/provider behavior changes persistence. 

---

## S9.9 Remote metastore implementation pattern

### S9.9.1 Loader

```rust
#[async_trait::async_trait]
pub trait MetastoreClient: Send + Sync + std::fmt::Debug {
    async fn load_snapshot(&self) -> datafusion::error::Result<MetastoreSnapshot>;
    async fn load_table(&self, table_ref: &ObjectRef) -> datafusion::error::Result<TableSnapshot>;
}
```

### S9.9.2 Catalog provider from snapshot

```rust
use std::sync::Arc;
use datafusion::catalog::{CatalogProvider, SchemaProvider};

#[derive(Debug)]
pub struct SnapshotCatalogProvider {
    name: String,
    snapshot: Arc<CatalogSnapshot>,
    schemas: std::collections::BTreeMap<String, Arc<dyn SchemaProvider>>,
}

impl SnapshotCatalogProvider {
    pub fn new(snapshot: CatalogSnapshot) -> Self {
        let schemas = snapshot
            .schemas
            .iter()
            .map(|(name, schema_snapshot)| {
                let provider = Arc::new(SnapshotSchemaProvider::new(schema_snapshot.clone()))
                    as Arc<dyn SchemaProvider>;
                (name.clone(), provider)
            })
            .collect();

        Self {
            name: snapshot.name.clone(),
            snapshot: Arc::new(snapshot),
            schemas,
        }
    }
}
```

### S9.9.3 Schema provider from snapshot

```rust
#[derive(Debug)]
pub struct SnapshotSchemaProvider {
    snapshot: SchemaSnapshot,
    tables: std::collections::BTreeMap<String, Arc<dyn TableProvider>>,
}

impl SnapshotSchemaProvider {
    pub fn new(snapshot: SchemaSnapshot) -> Self {
        let tables = snapshot
            .tables
            .iter()
            .map(|(name, table_snapshot)| {
                let provider = Arc::new(SnapshotTableProvider::new(table_snapshot.clone()))
                    as Arc<dyn TableProvider>;
                (name.clone(), provider)
            })
            .collect();

        Self { snapshot, tables }
    }
}
```

Implementation rule:

```text
Construct providers from snapshot before registering catalog.
Do not do remote lookup inside schema_names/table_names.
Only table(name).await may hydrate additional detail, and even then prefer pre-hydrated snapshot.
```

This follows the official remote-catalog guidance: provide in-memory metadata snapshots and batch remote catalog access rather than performing repeated network I/O during planning. ([Docs.rs][1])

---

## S9.10 `information_schema` as test and API surface

### S9.10.1 Testing matrix

```sql
-- tables
SELECT
  table_catalog,
  table_schema,
  table_name,
  table_type
FROM information_schema.tables
ORDER BY table_catalog, table_schema, table_name;

-- columns
SELECT
  table_catalog,
  table_schema,
  table_name,
  column_name,
  data_type,
  is_nullable
FROM information_schema.columns
ORDER BY table_catalog, table_schema, table_name, ordinal_position;

-- settings
SELECT
  name,
  setting
FROM information_schema.df_settings
ORDER BY name;
```

DataFusion’s information-schema docs explicitly map `SHOW TABLES` to `information_schema.tables`, `SHOW COLUMNS` to `information_schema.columns`, and `SHOW ALL` to `information_schema.df_settings`. ([Apache DataFusion][5])

### S9.10.2 Test assertions

```text
[ ] information_schema enabled only in test/admin contexts
[ ] expected tables visible
[ ] unauthorized tables not visible
[ ] expected columns visible
[ ] hidden/masked columns absent or represented safely
[ ] settings visibility matches policy
[ ] function visibility matches policy
[ ] output ordered deterministically
```

### S9.10.3 Safe use in CI

```rust
pub async fn assert_information_schema_has_column(
    ctx: &SessionContext,
    table_schema: &str,
    table_name: &str,
    column_name: &str,
) -> datafusion::error::Result<()> {
    let sql = format!(
        r#"
        SELECT column_name
        FROM information_schema.columns
        WHERE table_schema = '{table_schema}'
          AND table_name = '{table_name}'
          AND column_name = '{column_name}'
        "#
    );

    let batches = ctx.sql(&sql).await?.collect().await?;
    let rows: usize = batches.iter().map(|b| b.num_rows()).sum();

    assert_eq!(rows, 1);
    Ok(())
}
```

In production, avoid string interpolation for user-provided values; this test helper is for controlled CI fixtures.

---

## S9.11 DDL authorization and SQL options

Read-only SQL endpoint:

```rust
use datafusion::prelude::*;

pub async fn run_read_only_sql(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<DataFrame> {
    let options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    ctx.sql_with_options(sql, options).await
}
```

`SQLOptions` can disallow DDL, DML, and statement/config commands; the attached document repeatedly recommends using it for read-only or untrusted SQL surfaces. 

DDL authorization matrix:

| Environment          |             DDL |             DML | `SET`/statements |       `information_schema` |
| -------------------- | --------------: | --------------: | ---------------: | -------------------------: |
| Local CLI            |         allowed |         allowed |          allowed |                    allowed |
| Batch ETL            |    trusted only |    trusted only |          limited |        diagnostics if safe |
| Public SQL API       |        disabled |        disabled |         disabled |       disabled or filtered |
| Admin migration      |         allowed |         allowed |          allowed |                    allowed |
| Tenant SQL workspace | product-defined | product-defined |       restricted |            tenant-filtered |
| Regulated reporting  |        disabled |        disabled |         disabled | curated safe metadata only |

---

## S9.12 Catalog security boundaries

### S9.12.1 Layered enforcement

```text
CatalogProvider:
  which schemas exist for this user/session

SchemaProvider:
  which tables exist

TableProvider::schema():
  which columns are visible

TableProvider::scan():
  which rows are visible
  required tenant filters
  projection allowlist
  filter pushdown semantics

information_schema:
  mirrors only what catalog/schema/provider expose
  or must be disabled/filtered
```

### S9.12.2 Column hiding

Provider schema pattern:

```rust
pub fn visible_schema_for_role(full: &Schema, role: &RolePolicy) -> SchemaRef {
    let fields = full
        .fields()
        .iter()
        .filter(|field| role.can_see_column(field.name()))
        .map(|field| field.as_ref().clone())
        .collect::<Vec<_>>();

    Arc::new(Schema::new_with_metadata(fields, full.metadata().clone()))
}
```

### S9.12.3 Row filtering

```text
provider scan effective_filters =
  user_filters AND tenant_id = current_tenant AND row_policy_predicate
```

### S9.12.4 Metadata-leakage policy

```text
Disable or filter:
  information_schema.tables
  information_schema.columns
  information_schema.df_settings
  SHOW FUNCTIONS
  routines/parameters metadata
  statistics exposure
  source locations
  table definitions
```

The config docs show `information_schema` is disabled by default, and the information-schema docs show it can expose tables, columns, settings, and functions; this makes it a deliberate security decision rather than a harmless convenience. ([Apache DataFusion][2]) ([Apache DataFusion][5])

---

## S9.13 Catalog persistence semantics

### S9.13.1 Required provider declaration

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogPersistenceSemantics {
    pub metadata_persistence: MetadataPersistence,
    pub ddl_persistence: DdlPersistence,
    pub external_data_lifecycle: ExternalDataLifecycle,
    pub supports_concurrent_ddl: bool,
    pub supports_transactions: bool,
    pub supports_tombstones: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum MetadataPersistence {
    InMemorySession,
    LocalFile,
    ObjectStoreManifest,
    RemoteMetastore,
    TransactionalDatabase,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DdlPersistence {
    None,
    SessionOnly,
    DurableAfterCommit,
    DelegatedToRemoteMetastore,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum ExternalDataLifecycle {
    CatalogOnly,
    CatalogAndDataManagedTogether,
    DataManagedExternally,
}
```

### S9.13.2 Anti-ambiguity statement

Every custom catalog should document:

```text
CREATE SCHEMA:
  durable? visible when? conflict behavior?

CREATE EXTERNAL TABLE:
  does it persist location metadata?
  does it validate path?
  does it own files?

DROP TABLE:
  deregister only?
  tombstone?
  delete files?
  cascade policy?

CREATE VIEW:
  stores SQL text?
  stores logical plan?
  versioned?
```

Docs.rs says `MemoryCatalogProvider` has no persistence and is used by default; this is a key difference from a DBMS catalog. ([Docs.rs][1])

---

## S9.14 Operational metadata events

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogEvent {
    pub event_id: String,
    pub timestamp_ms: i64,
    pub actor: String,
    pub session_id: Option<String>,
    pub operation: CatalogOperation,
    pub object_ref: ObjectRef,
    pub snapshot_before: Option<String>,
    pub snapshot_after: Option<String>,
    pub schema_fingerprint_before: Option<String>,
    pub schema_fingerprint_after: Option<String>,
    pub sql_text_redacted: Option<String>,
    pub status: CatalogEventStatus,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CatalogEventStatus {
    Started,
    Succeeded,
    Failed,
    RejectedByPolicy,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ObjectRef {
    pub catalog: String,
    pub schema: String,
    pub name: String,
    pub object_type: CatalogObjectType,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CatalogObjectType {
    Schema,
    Table,
    View,
    Function,
}
```

Agent rule:

```text
Every DDL/migration/catalog refresh must emit an audit event.
```

---

## S9.15 Object lifecycle and migration-grade catalog state

```text
Active:
  visible and queryable

Deprecated:
  visible with warning; replacement exists

Hidden:
  not listed in information_schema, but direct access may be allowed for internal plans

Tombstoned:
  no new queries; retained for audit and migration rollback

Retired:
  removed from active metadata; audit record retained
```

Lifecycle object:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogObjectLifecycle {
    pub state: CatalogObjectState,
    pub created_by: String,
    pub created_at_ms: i64,
    pub deprecated_at_ms: Option<i64>,
    pub replacement_ref: Option<ObjectRef>,
    pub tombstone_reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum CatalogObjectState {
    Active,
    Deprecated,
    Hidden,
    Tombstoned,
    Retired,
}
```

Migration policy:

```text
CREATE OR REPLACE:
  old object becomes Deprecated or Tombstoned
  new object gets new fingerprint
  compatibility report required

DROP:
  tombstone first in governed catalogs
  purge later by retention policy

Rename:
  create new object
  compatibility alias/view
  deprecate old object
```

---

## S9.16 Remote snapshot + DataFusion query planning flow

```text
Query received:
  SELECT * FROM tenant_acme.semantic.stream_balances

1. Validate SQL class:
   DDL/DML/statements disabled unless authorized

2. Resolve namespace:
   tenant catalog = tenant_acme
   schema = semantic
   table = stream_balances

3. Load planning snapshot:
   snapshot_id = meta-20260524T120000Z
   staleness check

4. Resolve table:
   SchemaProvider::table("stream_balances").await

5. Provider built from snapshot:
   schema fingerprint = sha256:...
   row policies attached
   statistics policy applied

6. Plan SQL:
   no network calls for listing/metadata

7. Execute:
   TableProvider::scan enforces tenant/data access

8. Audit:
   query refs, snapshot_id, schema fingerprint
```

This aligns with the official remote-catalog pattern: DataFusion walks the query to find table references, performs remote lookups in parallel, stores results in a cached snapshot, and plans using that snapshot. ([Docs.rs][1])

---

## S9.17 Test matrix

```text
Catalog tests:
  [ ] default catalog name
  [ ] default schema name
  [ ] fully-qualified table resolution
  [ ] schema-qualified table resolution
  [ ] unqualified table resolution
  [ ] unknown catalog error
  [ ] unknown schema error
  [ ] unknown table error
  [ ] catalog_names deterministic
  [ ] schema_names deterministic
  [ ] table_names deterministic
  [ ] table_exist consistent with table(name).await
  [ ] register_schema allowed/denied
  [ ] deregister_schema allowed/denied
  [ ] register_table allowed/denied
  [ ] deregister_table allowed/denied

Remote metastore tests:
  [ ] snapshot load
  [ ] refresh interval
  [ ] invalidation event
  [ ] stale read warning/rejection
  [ ] no network call in listing methods
  [ ] deterministic snapshot id
  [ ] concurrent refresh while query plans

Information schema tests:
  [ ] disabled by default in secure config
  [ ] enabled in admin/test config
  [ ] tables visible
  [ ] columns visible
  [ ] views visible
  [ ] df_settings hidden/visible per policy
  [ ] unauthorized tables absent
  [ ] function metadata hidden/visible per policy

DDL tests:
  [ ] read-only catalog rejects DDL
  [ ] mutable catalog audits DDL
  [ ] concurrent create conflict
  [ ] drop creates tombstone if governed
  [ ] create/replace records old/new fingerprints
```

The attached document’s catalog testing matrix already includes default catalog/schema, qualified resolution, registration/deregistration, information-schema visibility, `SHOW TABLES`/`SHOW COLUMNS`, DDL, custom-catalog authorization, and deterministic remote-cache behavior. 

---

## S9.18 Diagnostics payloads

### Unknown table

```json
{
  "error_class": "catalog_table_not_found",
  "phase": "name_resolution",
  "catalog": "tenant_acme",
  "schema": "semantic",
  "table": "streams",
  "snapshot_id": "meta-20260524T120000Z",
  "candidate_tables": ["stream_balances", "stream_flows"],
  "suggested_fix": "Use tenant_acme.curated.streams or refresh catalog snapshot"
}
```

### Stale catalog snapshot

```json
{
  "warning_class": "stale_catalog_snapshot",
  "phase": "planning",
  "catalog": "tenant_acme",
  "snapshot_id": "meta-20260524T120000Z",
  "snapshot_age_ms": 70000,
  "max_staleness_ms": 60000,
  "policy": "RejectIfStale",
  "decision": "reject_query"
}
```

### Information-schema access denied

```json
{
  "error_class": "metadata_visibility_denied",
  "phase": "plan_validation",
  "object": "information_schema.df_settings",
  "reason": "runtime settings metadata is not exposed to this role",
  "suggested_fix": "Use the governed metadata API or request admin role"
}
```

### DDL denied

```json
{
  "error_class": "catalog_mutation_denied",
  "phase": "sql_validation",
  "operation": "CREATE EXTERNAL TABLE",
  "catalog": "tenant_acme",
  "schema": "curated",
  "reason": "catalog is read-only for user SQL",
  "suggested_fix": "Run migration through trusted admin workflow"
}
```

---

## S9.19 Deployment advisory

```text
Namespace:
  catalog per tenant when visibility/credentials differ
  schema per lifecycle layer when data maturity differs
  schema per workspace when project isolation matters
  never rely on unqualified names in multi-tenant generated SQL
```

```text
Remote metastore:
  batch remote metadata into snapshots
  no network calls in catalog_names/schema_names/table_names
  freeze snapshot per query/job
  define staleness policy
  explicitly invalidate on DDL/metastore events
  deterministic sorted listings
```

```text
information_schema:
  disabled by default for secure services
  enabled for CLI/admin/diagnostics
  filtered or replaced with governed metadata views for tenants
  never expose df_settings/functions/routines casually
```

```text
DDL:
  use SQLOptions to disable DDL/DML/statements for read-only SQL
  read-only catalogs return explicit policy errors
  mutable catalogs audit every mutation
  define persistence and external-data lifecycle for every catalog provider
  prefer tombstone/deprecate over hard delete in governed catalogs
```

```text
Testing:
  ORDER BY information_schema outputs
  assert stable listing order
  test table_exist == table(name).await.is_some()
  test remote refresh determinism
  test unauthorized metadata invisibility
```

---

## S9.20 Anti-pattern inventory

```text
Catalog/schema anti-patterns:
  using MemoryCatalogProvider as durable metadata
  one shared mutable default catalog for all tenants
  tenant isolation only through table-name prefixes
  unqualified generated SQL in multi-catalog systems
  network calls inside schema_names/table_names hot paths
  nondeterministic table_names used in snapshot tests
  table_exist inconsistent with table(name).await
  enabling information_schema in public endpoints without policy
  exposing information_schema.df_settings to untrusted users
  exposing SHOW FUNCTIONS / routines in restricted environments
  assuming DROP TABLE deletes external data
  assuming CREATE DATABASE creates durable DBMS database
  registering arbitrary user-provided external locations
  mutable catalog with no audit log
  DDL allowed without persistence semantics
  catalog refresh without invalidation strategy
  stale remote metadata used for migrations
  table replacement without old/new schema fingerprint record
```

---

## S9.21 Agent checklist

```text
[ ] Understand hierarchy:
    CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider.

[ ] Set defaults explicitly:
    datafusion.catalog.default_catalog
    datafusion.catalog.default_schema
    datafusion.catalog.information_schema

[ ] Choose namespace policy:
    catalog per tenant
    schema per workspace
    schema per lifecycle layer
    raw/staging/curated/semantic/diagnostics/temp

[ ] Use qualified names in generated multi-tenant SQL:
    catalog.schema.table

[ ] Use MemoryCatalogProvider only for:
    tests
    CLI
    session-local metadata
    examples

[ ] Use custom CatalogProvider/SchemaProvider for:
    durable metadata
    remote metastore
    tenant governance
    filtered visibility
    migration lifecycle

[ ] Remote catalog:
    preload snapshot
    cache metadata
    define TTL/staleness
    explicit invalidation
    deterministic listing
    no network calls in hot planning loops

[ ] information_schema:
    enable only when safe
    test tables/columns/views/settings visibility
    filter or disable for tenants
    avoid leaking df_settings/function metadata

[ ] DDL:
    disable through SQLOptions for untrusted SQL
    define read-only vs mutable catalog behavior
    audit every mutation
    define concurrency and persistence semantics

[ ] Object lifecycle:
    active/deprecated/hidden/tombstoned/retired
    CREATE OR REPLACE records old/new fingerprints
    DROP is catalog lifecycle, not necessarily file deletion

[ ] Tests:
    sorted catalog_names/schema_names/table_names
    table_exist consistency
    information_schema parity with SHOW TABLES/COLUMNS
    remote refresh determinism
    unauthorized objects invisible
```

## S9.22 Minimal governed catalog control-plane skeleton

```rust
use std::collections::BTreeMap;
use std::sync::Arc;

use datafusion::catalog::{CatalogProvider, SchemaProvider, TableProvider};
use datafusion::common::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct ObjectRef {
    pub catalog: String,
    pub schema: String,
    pub table: String,
}

#[derive(Debug, Clone)]
pub enum CatalogMutationPolicy {
    ReadOnly,
    MutableWithAudit,
}

#[derive(Debug, Clone)]
pub struct GovernedCatalogConfig {
    pub name: String,
    pub mutation_policy: CatalogMutationPolicy,
    pub deterministic_listing: bool,
    pub information_schema_allowed: bool,
    pub max_snapshot_staleness_ms: Option<u64>,
}

#[derive(Debug)]
pub struct GovernedCatalogProvider {
    config: GovernedCatalogConfig,
    schemas: BTreeMap<String, Arc<dyn SchemaProvider>>,
    snapshot_id: String,
}

impl GovernedCatalogProvider {
    pub fn new(
        config: GovernedCatalogConfig,
        schemas: BTreeMap<String, Arc<dyn SchemaProvider>>,
        snapshot_id: impl Into<String>,
    ) -> Self {
        Self {
            config,
            schemas,
            snapshot_id: snapshot_id.into(),
        }
    }

    fn ensure_mutable(&self, operation: &str) -> Result<()> {
        match self.config.mutation_policy {
            CatalogMutationPolicy::ReadOnly => Err(DataFusionError::Plan(format!(
                "catalog `{}` is read-only; operation `{operation}` denied",
                self.config.name
            ))),
            CatalogMutationPolicy::MutableWithAudit => Ok(()),
        }
    }

    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }
}

/*
Implement CatalogProvider using pinned DataFusion trait signatures.

Core implementation rules:
  schema_names() -> sorted keys from BTreeMap
  schema(name) -> Arc clone from snapshot
  register_schema() -> ensure_mutable + audit + atomic snapshot update
  deregister_schema() -> ensure_mutable + audit + tombstone or remove
*/
```

Core operating rule: **Catalogs are not just lookup maps; in a governed DataFusion platform they are the metadata, security, migration, visibility, and lifecycle control plane.**

[1]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.CatalogProvider.html "CatalogProvider in datafusion::catalog - Rust"
[2]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[3]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.SchemaProvider.html "SchemaProvider in datafusion::catalog - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/catalog/trait.TableProvider.html "TableProvider in datafusion::catalog - Rust"
[5]: https://datafusion.apache.org/user-guide/sql/information_schema.html "Information Schema — Apache DataFusion  documentation"
[6]: https://datafusion.apache.org/library-user-guide/catalogs.html "Catalogs, Schemas, and Tables — Apache DataFusion  documentation"


# DataFusion Advanced — S10) Custom `TableProvider` schema adaptation and projection mapping

## S10.0 Objective

Make custom `TableProvider`s safe under:

```text
projection pushdown
filter pushdown
limit pushdown
dynamic backend schemas
remote API schemas
backend → Arrow type adaptation
hidden / generated / virtual columns
tenant predicates
schema drift
runtime stream validation
```

The attached documentation already establishes the baseline: `schema()` is authoritative, projection indices refer to `schema()` field order, `scan(...)` receives projection/filter/limit hints, and every emitted batch must match the stream/plan schema exactly.  DataFusion’s current `TableProvider` docs state that projection, when specified, is a set of indexes into `Self::schema()` and that only those fields should be returned, in the specified order. ([Docs.rs][1])

Core operating rule:

```text
Provider schema adaptation is a compiler boundary:

backend schema snapshot
  → canonical Arrow Schema
  → field mapping table
  → projection mapping
  → filter pushdown mapping
  → ExecutionPlan schema
  → RecordBatchStream schema
  → every RecordBatch schema
```

---

## S10.1 Provider schema contract

### S10.1.1 Required contract

```text
TableProvider::schema()
  = authoritative logical table output schema
  = field order used by DataFusion projection indices
  = source of names/types/nullability for SQL/DataFrame planning
  = stable during planning and execution of a query
  = cheap Arc clone, not remote metadata lookup
```

The custom provider guide frames `TableProvider`, `ExecutionPlan`, and `SendableRecordBatchStream` as separate layers: provider describes the table and creates a physical plan; plan describes execution; stream yields `RecordBatch`es. ([Apache DataFusion][2]) The attached custom-provider baseline also states `schema()` must be cheap/stable and that returned batches must match projected stream schema. 

```rust
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::Result;
use datafusion::datasource::TableType;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct RemoteTableProvider {
    /// Canonical DataFusion-facing schema.
    /// Field order is the projection-index contract.
    schema: SchemaRef,

    /// Mapping from DataFusion field index/name to backend field/path/type.
    mapping: Arc<ProviderFieldMap>,

    /// Frozen backend schema snapshot used to build this provider.
    snapshot: Arc<BackendSchemaSnapshot>,

    /// Remote client; not used by schema().
    client: Arc<dyn RemoteClient>,
}

#[async_trait]
impl TableProvider for RemoteTableProvider {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        let projection_plan = self.mapping.plan_projection(&self.schema, projection)?;
        let filter_plan = self.mapping.plan_filters(filters)?;
        let output_schema = projection_plan.output_schema.clone();

        Ok(Arc::new(RemoteExec::new(
            Arc::clone(&self.client),
            Arc::clone(&self.snapshot),
            projection_plan,
            filter_plan,
            limit,
            output_schema,
        )))
    }
}
```

Agent rules:

```text
schema() returns Arc<Schema>; no network calls.
schema() must not change while a query is planning/executing.
schema field order defines projection indices.
projection indices are not backend column indices.
scan() builds ExecutionPlan; do not fetch all rows in scan().
execute() constructs stream quickly; stream polling does remote I/O.
```

---

## S10.2 Backend schema snapshot

### S10.2.1 Snapshot object

```rust
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct BackendSchemaSnapshot {
    pub backend_name: String,
    pub object_name: String,
    pub snapshot_id: String,
    pub loaded_at_epoch_ms: i64,
    pub expires_at_epoch_ms: Option<i64>,

    /// Backend-native fields keyed deterministically.
    pub fields: BTreeMap<String, BackendFieldDescriptor>,

    /// Backend schema fingerprint before DataFusion normalization.
    pub backend_fingerprint: String,

    /// DataFusion-facing schema fingerprint after normalization.
    pub arrow_fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct BackendFieldDescriptor {
    pub backend_name: String,
    pub backend_path: BackendFieldPath,
    pub backend_type: BackendType,
    pub nullable: BackendNullability,
    pub source_ordinal: Option<usize>,
    pub hidden: bool,
    pub virtual_column: bool,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum BackendFieldPath {
    Flat(String),
    Nested(Vec<String>),
    JsonPath(String),
    ApiPath(String),
}

#[derive(Debug, Clone)]
pub enum BackendNullability {
    NonNull,
    Nullable,
    Unknown,
}
```

Use a snapshot because remote APIs/databases/metastores can change schema between planning and execution. The custom provider pattern in the attached documentation explicitly describes remote API providers with a cached `schema: SchemaRef`, client, table configuration, static metadata, scan-time projection/filter/limit conversion, and stream-time remote page fetches. 

### S10.2.2 Snapshot lifecycle

```text
load remote metadata
  → normalize backend names/types/nullability
  → build Arrow SchemaRef
  → build ProviderFieldMap
  → compute fingerprints
  → construct TableProvider
  → freeze for query-start planning
```

Refresh modes:

```text
startup snapshot:
  load once at service/job start

periodic snapshot:
  refresh at interval, atomic provider/catalog swap

manual invalidation:
  explicit refresh after metastore event / DDL / remote schema change

query-start snapshot:
  each query pins snapshot id before planning

strict migration mode:
  reject query if snapshot stale or version mismatch
```

---

## S10.3 Backend type → Arrow type adaptation

### S10.3.1 Mapping table

| Backend type family | Arrow target                                     | Policy                                                    |
| ------------------- | ------------------------------------------------ | --------------------------------------------------------- |
| signed integer      | `Int8` / `Int16` / `Int32` / `Int64`             | choose minimum only if contract-stable; otherwise `Int64` |
| unsigned integer    | `UInt*` or widened signed                        | avoid signed conversion unless range-proven               |
| float               | `Float32` / `Float64`                            | prefer `Float64` for analytics                            |
| decimal/money       | `Decimal128(p,s)` / `Decimal256(p,s)`            | never map money to float by default                       |
| string/text         | `Utf8` / `Utf8View` / `LargeUtf8`                | select one provider policy; document UDF implications     |
| enum                | `Utf8` + metadata or dictionary                  | dictionary optional; contract must say                    |
| boolean             | `Boolean`                                        | direct                                                    |
| date                | `Date32`                                         | direct if day precision                                   |
| time                | `Time64(Nanosecond)` or configured unit          | define unit policy                                        |
| timestamp           | `Timestamp(unit, timezone)`                      | define unit/timezone explicitly                           |
| JSON object         | `Utf8`, `Struct`, or `Map`                       | prefer typed struct for hot fields                        |
| array/list          | `List(FieldRef)` / `LargeList` / `FixedSizeList` | define item nullability separately                        |
| struct/object       | `Struct(Fields)`                                 | recursive field mapping                                   |
| binary/blob         | `Binary` / `LargeBinary`                         | direct if supported                                       |
| UUID                | `Utf8` + `semantic.type=uuid`                    | DataFusion SQL type table does not map native UUID        |
| geography/custom    | physical fallback + extension metadata           | custom semantics outside core                             |

Arrow `RecordBatch` requires arrays whose data types match the schema exactly, so this mapping is not advisory: if the provider maps backend `decimal` to `Decimal128(20,4)`, the emitted Arrow array must actually have that data type. ([Docs.rs][3])

### S10.3.2 Type adapter trait

```rust
use datafusion::arrow::datatypes::{DataType, Field};

pub trait BackendTypeAdapter: Send + Sync + std::fmt::Debug {
    fn to_arrow_data_type(&self, backend_type: &BackendType) -> datafusion::common::Result<DataType>;

    fn to_arrow_field(
        &self,
        backend: &BackendFieldDescriptor,
        canonical_name: &str,
    ) -> datafusion::common::Result<Field> {
        let dt = self.to_arrow_data_type(&backend.backend_type)?;
        let nullable = match backend.nullable {
            BackendNullability::NonNull => false,
            BackendNullability::Nullable | BackendNullability::Unknown => true,
        };

        let mut metadata = backend.metadata.clone();
        metadata.insert("source.name".to_string(), backend.backend_name.clone());
        metadata.insert("source.backend_path".to_string(), format!("{:?}", backend.backend_path));

        Ok(Field::new(canonical_name, dt, nullable).with_metadata(metadata))
    }
}
```

### S10.3.3 Normalization examples

```rust
#[derive(Debug, Clone)]
pub enum BackendType {
    Int32,
    Int64,
    Float,
    Double,
    Decimal { precision: u8, scale: i8 },
    String,
    Boolean,
    Date,
    Timestamp { unit: BackendTimeUnit, timezone: Option<String> },
    Json,
    Array(Box<BackendType>),
    Struct(Vec<BackendFieldDescriptor>),
    Unknown(String),
}

#[derive(Debug, Clone)]
pub enum BackendTimeUnit {
    Seconds,
    Millis,
    Micros,
    Nanos,
}
```

```rust
use datafusion::arrow::datatypes::{DataType, TimeUnit};

#[derive(Debug)]
pub struct DefaultBackendTypeAdapter;

impl BackendTypeAdapter for DefaultBackendTypeAdapter {
    fn to_arrow_data_type(&self, t: &BackendType) -> datafusion::common::Result<DataType> {
        Ok(match t {
            BackendType::Int32 => DataType::Int32,
            BackendType::Int64 => DataType::Int64,
            BackendType::Float => DataType::Float32,
            BackendType::Double => DataType::Float64,
            BackendType::String => DataType::Utf8,
            BackendType::Boolean => DataType::Boolean,
            BackendType::Date => DataType::Date32,
            BackendType::Decimal { precision, scale } if *precision <= 38 => {
                DataType::Decimal128(*precision, *scale)
            }
            BackendType::Decimal { precision, scale } if *precision <= 76 => {
                DataType::Decimal256(*precision, *scale)
            }
            BackendType::Timestamp { unit, timezone } => {
                let unit = match unit {
                    BackendTimeUnit::Seconds => TimeUnit::Second,
                    BackendTimeUnit::Millis => TimeUnit::Millisecond,
                    BackendTimeUnit::Micros => TimeUnit::Microsecond,
                    BackendTimeUnit::Nanos => TimeUnit::Nanosecond,
                };
                DataType::Timestamp(unit, timezone.clone().map(Into::into))
            }
            BackendType::Array(item) => {
                let item_dt = self.to_arrow_data_type(item)?;
                DataType::List(std::sync::Arc::new(
                    datafusion::arrow::datatypes::Field::new_list_field(item_dt, true)
                ))
            }
            BackendType::Struct(fields) => {
                let children = fields
                    .iter()
                    .map(|f| {
                        let name = canonical_name(&f.backend_name);
                        self.to_arrow_field(f, &name).map(Into::into)
                    })
                    .collect::<datafusion::common::Result<Vec<_>>>()?;
                DataType::Struct(children.into())
            }
            BackendType::Json => DataType::Utf8,
            BackendType::Unknown(name) => {
                return Err(datafusion::common::DataFusionError::Plan(format!(
                    "unsupported backend type `{name}`"
                )))
            }
            BackendType::Decimal { precision, .. } => {
                return Err(datafusion::common::DataFusionError::Plan(format!(
                    "decimal precision {precision} exceeds Arrow/DataFusion max policy"
                )))
            }
        })
    }
}
```

---

## S10.4 Provider field map

### S10.4.1 Mapping object

```rust
use std::collections::BTreeMap;
use datafusion::arrow::datatypes::{Field, SchemaRef};

#[derive(Debug, Clone)]
pub struct ProviderFieldMap {
    /// DataFusion projection index → canonical field mapping.
    pub by_index: Vec<ProviderFieldMapping>,

    /// Canonical output name → projection index.
    pub by_name: BTreeMap<String, usize>,

    /// Backend path → projection index.
    pub by_backend_path: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub struct ProviderFieldMapping {
    pub df_index: usize,
    pub df_name: String,
    pub df_field: Field,

    pub backend_name: Option<String>,
    pub backend_path: Option<BackendFieldPath>,
    pub backend_type: Option<BackendType>,

    pub hidden: bool,
    pub virtual_column: bool,
    pub generated_expr: Option<String>,
    pub tenant_policy_column: bool,
}
```

Provider field-map invariants:

```text
by_index.len() == schema.fields().len()
by_index[i].df_index == i
by_index[i].df_field == schema.fields()[i]
by_name has no duplicates
hidden columns are either absent from schema() or explicitly policy-controlled
virtual columns have generation rules
backend_path exists for physical backend fields
```

### S10.4.2 Build mapping from snapshot

```rust
pub fn build_provider_schema_and_mapping(
    snapshot: &BackendSchemaSnapshot,
    adapter: &dyn BackendTypeAdapter,
) -> datafusion::common::Result<(SchemaRef, ProviderFieldMap)> {
    let mut fields = Vec::new();
    let mut mappings = Vec::new();

    for backend in snapshot.fields.values() {
        if backend.hidden {
            continue;
        }

        let canonical = canonical_name(&backend.backend_name);
        let field = adapter.to_arrow_field(backend, &canonical)?;
        let df_index = fields.len();

        fields.push(field.clone());

        mappings.push(ProviderFieldMapping {
            df_index,
            df_name: canonical.clone(),
            df_field: field,
            backend_name: Some(backend.backend_name.clone()),
            backend_path: Some(backend.backend_path.clone()),
            backend_type: Some(backend.backend_type.clone()),
            hidden: false,
            virtual_column: backend.virtual_column,
            generated_expr: None,
            tenant_policy_column: false,
        });
    }

    let schema = std::sync::Arc::new(datafusion::arrow::datatypes::Schema::new(fields));
    let map = ProviderFieldMap::new(schema.clone(), mappings)?;

    Ok((schema, map))
}
```

---

## S10.5 Projection mapping

### S10.5.1 Projection semantics

Projection from DataFusion is:

```text
projection: Option<&Vec<usize>>

None:
  return all visible schema fields, in schema order

Some(indices):
  return only schema fields at those indices, in the exact order supplied
```

DataFusion’s `TableProvider` docs say the projection is a set of indexes of fields in `Self::schema()` and that, if specified, only that subset should be returned in the specified order. ([Docs.rs][1])

### S10.5.2 Projection plan

```rust
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::common::{DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct ProjectionPlan {
    pub requested_indices: Vec<usize>,
    pub output_schema: SchemaRef,
    pub output_fields: Vec<ProviderFieldMapping>,

    /// Backend physical fields to request.
    pub backend_select: Vec<BackendSelectItem>,

    /// Fields synthesized after backend response.
    pub virtual_outputs: Vec<VirtualOutput>,
}

#[derive(Debug, Clone)]
pub struct BackendSelectItem {
    pub output_position: usize,
    pub backend_path: BackendFieldPath,
    pub backend_name: String,
    pub expected_arrow_type: datafusion::arrow::datatypes::DataType,
}

#[derive(Debug, Clone)]
pub struct VirtualOutput {
    pub output_position: usize,
    pub df_name: String,
    pub generation: VirtualGeneration,
}

#[derive(Debug, Clone)]
pub enum VirtualGeneration {
    ConstantNull,
    TenantId,
    SourcePath,
    IngestionTimestamp,
    ExpressionSql(String),
}
```

### S10.5.3 Projection mapper

```rust
impl ProviderFieldMap {
    pub fn new(schema: SchemaRef, mappings: Vec<ProviderFieldMapping>) -> Result<Self> {
        if schema.fields().len() != mappings.len() {
            return Err(DataFusionError::Plan(format!(
                "field map length {} does not match schema field count {}",
                mappings.len(),
                schema.fields().len()
            )));
        }

        let mut by_name = BTreeMap::new();
        let mut by_backend_path = BTreeMap::new();

        for (idx, mapping) in mappings.iter().enumerate() {
            if idx != mapping.df_index {
                return Err(DataFusionError::Plan(format!(
                    "field map index mismatch: vec index={idx}, mapping index={}",
                    mapping.df_index
                )));
            }

            if by_name.insert(mapping.df_name.clone(), idx).is_some() {
                return Err(DataFusionError::Plan(format!(
                    "duplicate provider output field name `{}`",
                    mapping.df_name
                )));
            }

            if let Some(path) = &mapping.backend_path {
                by_backend_path.insert(format!("{path:?}"), idx);
            }
        }

        Ok(Self {
            by_index: mappings,
            by_name,
            by_backend_path,
        })
    }

    pub fn plan_projection(
        &self,
        full_schema: &SchemaRef,
        projection: Option<&Vec<usize>>,
    ) -> Result<ProjectionPlan> {
        let requested_indices = match projection {
            Some(indices) => indices.clone(),
            None => (0..self.by_index.len()).collect(),
        };

        for idx in &requested_indices {
            if *idx >= self.by_index.len() {
                return Err(DataFusionError::Plan(format!(
                    "projection index {idx} out of bounds for provider schema with {} fields",
                    self.by_index.len()
                )));
            }
        }

        let output_schema = std::sync::Arc::new(full_schema.project(&requested_indices)?);

        let mut output_fields = Vec::new();
        let mut backend_select = Vec::new();
        let mut virtual_outputs = Vec::new();

        for (out_pos, df_idx) in requested_indices.iter().copied().enumerate() {
            let mapping = self.by_index[df_idx].clone();

            if let Some(path) = &mapping.backend_path {
                backend_select.push(BackendSelectItem {
                    output_position: out_pos,
                    backend_path: path.clone(),
                    backend_name: mapping.backend_name.clone().unwrap_or_else(|| mapping.df_name.clone()),
                    expected_arrow_type: mapping.df_field.data_type().clone(),
                });
            } else if mapping.virtual_column {
                virtual_outputs.push(VirtualOutput {
                    output_position: out_pos,
                    df_name: mapping.df_name.clone(),
                    generation: VirtualGeneration::ConstantNull,
                });
            } else {
                return Err(DataFusionError::Plan(format!(
                    "field `{}` has no backend path and is not virtual",
                    mapping.df_name
                )));
            }

            output_fields.push(mapping);
        }

        Ok(ProjectionPlan {
            requested_indices,
            output_schema,
            output_fields,
            backend_select,
            virtual_outputs,
        })
    }
}
```

### S10.5.4 Projection anti-corruption rule

```text
Backend physical order must never leak into RecordBatch output order.

RecordBatch output order =
  DataFusion projection order =
  ProjectionPlan.output_schema field order.
```

Bad:

```text
projection = [2, 0]
backend returns [field0, field2]
provider emits backend order [field0, field2]
```

Good:

```text
projection = [2, 0]
provider emits [schema[2], schema[0]]
```

---

## S10.6 Backend request mapping

### S10.6.1 REST sparse-fieldsets

```text
DataFusion projection:
  [stream_id, mass_flow_kg_h]

Backend request:
  GET /streams?fields=id,massFlow
```

Mapping:

```rust
pub fn to_rest_field_list(plan: &ProjectionPlan) -> Vec<String> {
    plan.backend_select
        .iter()
        .map(|item| item.backend_name.clone())
        .collect()
}
```

### S10.6.2 GraphQL projection

```text
DataFusion:
  stream_id, assay.api_gravity, assay.sulfur_wt_pct

GraphQL:
  query {
    streams {
      id
      assay { apiGravity sulfurWtPct }
    }
  }
```

Nested mapping requires backend path tree:

```rust
#[derive(Debug, Clone)]
pub struct BackendProjectionTree {
    pub fields: BTreeMap<String, BackendProjectionTree>,
}
```

### S10.6.3 SQL federation projection

```sql
SELECT
  backend_id AS stream_id,
  mass_flow AS mass_flow_kg_h
FROM remote_streams
```

Provider rule:

```text
Push only needed backend columns plus filter columns required for residual evaluation.
Do not request hidden columns unless provider needs them for tenant/filter enforcement.
```

---

## S10.7 Hidden columns

### S10.7.1 Hidden-column categories

```text
tenant_id:
  needed for provider-enforced tenant filter
  usually hidden from user schema

source_file_path:
  diagnostic/provenance virtual column
  may be exposed in diagnostics schema

row_version:
  needed for optimistic concurrency
  hidden from ordinary users

deleted_flag:
  provider applies deleted_flag=false
  hidden unless audit mode

backend_cursor:
  execution-only
  never exposed
```

### S10.7.2 Hidden-column policy

```rust
#[derive(Debug, Clone)]
pub enum ColumnVisibility {
    Public,
    HiddenRequiredForPolicy,
    HiddenDiagnostic,
    InternalOnly,
}

#[derive(Debug, Clone)]
pub struct ColumnVisibilityPolicy {
    pub visibility: ColumnVisibility,
    pub expose_in_schema: bool,
    pub allow_projection: bool,
    pub allow_filter: bool,
}
```

Rules:

```text
If hidden column is not in schema():
  DataFusion users cannot project/filter it.
  Provider may still request it from backend internally.

If hidden column is in schema():
  It is visible to planner and information_schema unless catalog/provider filters it.

Do not expose tenant_id unless product explicitly permits.
Composite uniqueness must include tenant_id if uniqueness is tenant-scoped.
```

---

## S10.8 Physical vs logical projection

### S10.8.1 Logical projection

```text
DataFusion requested:
  output fields [stream_id, mass_flow_kg_h]
```

### S10.8.2 Physical backend projection

```text
Backend must fetch:
  id
  massFlow
  tenantId         -- hidden policy predicate
  deletedFlag      -- soft-delete predicate
```

### S10.8.3 Residual projection

```text
Backend response:
  id, massFlow, tenantId, deletedFlag

Provider output RecordBatch:
  stream_id, mass_flow_kg_h
```

Provider rule:

```text
Physical request may include hidden/filter columns.
RecordBatch output must include only DataFusion projection fields.
```

---

## S10.9 Nested projection

### S10.9.1 Provider schema

```text
assay: Struct<
  api_gravity: Float64,
  sulfur_wt_pct: Float64,
  nitrogen_wt_pct: Float64
>
```

DataFusion projection index sees `assay` as a top-level field. Nested field pruning may be represented by expressions such as `assay['api_gravity']` in a projection above the provider unless a custom provider/planner recognizes nested projection requirements.

### S10.9.2 Backend nested selection

```text
Query:
  SELECT assay['api_gravity'] AS api_gravity FROM streams

Naive provider:
  fetch assay object

Advanced provider:
  detect get_field(assay, 'api_gravity')
  fetch assay.apiGravity only
```

### S10.9.3 Nested projection contract

```rust
#[derive(Debug, Clone)]
pub struct NestedProjectionRequirement {
    pub top_level_field: String,
    pub nested_paths: Vec<Vec<String>>,
    pub required_for_output: bool,
    pub required_for_filter: bool,
}
```

Agent rule:

```text
TableProvider::scan projection is top-level field-index based.
Nested pruning requires additional expression analysis or custom planner logic.
Do not claim nested projection pushdown unless implemented/tested.
```

---

## S10.10 Filter pushdown interaction

### S10.10.1 Exact / inexact / unsupported

`supports_filters_pushdown` returns `Exact`, `Inexact`, or `Unsupported` for each filter. `Exact` means the table provider fully evaluates the filter so DataFusion can omit a residual filter; `Inexact` means the provider reduces data but DataFusion must still evaluate the filter; `Unsupported` means no provider-side pushdown. ([Docs.rs][1])

```rust
use datafusion::datasource::TableProviderFilterPushDown;
use datafusion::logical_expr::Expr;

fn supports_filters_pushdown(
    &self,
    filters: &[&Expr],
) -> datafusion::common::Result<Vec<TableProviderFilterPushDown>> {
    Ok(filters.iter().map(|f| self.classify_filter(f)).collect())
}
```

### S10.10.2 Filter planning result

```rust
#[derive(Debug, Clone)]
pub struct FilterPushdownPlan {
    pub exact_backend_filters: Vec<BackendFilter>,
    pub inexact_backend_filters: Vec<BackendFilter>,
    pub unsupported_filters: Vec<Expr>,

    /// Fields needed only to evaluate residual filters.
    pub residual_filter_fields: Vec<usize>,
}
```

### S10.10.3 Pushdown safety rules

```text
Exact pushdown:
  backend semantics must match DataFusion SQL semantics
  NULL behavior must match
  type coercion must match
  timezone/string collation/case sensitivity must match
  provider output may omit rows only if definitely filtered out

Inexact pushdown:
  backend can prefilter candidates
  DataFusion residual filter must remain
  provider must not apply LIMIT before residual unless safe

Unsupported:
  do not translate
```

The attached anti-pattern list explicitly calls out claiming `Exact` pushdown for approximate predicates and applying `LIMIT` before inexact filters as unsafe. 

---

## S10.11 Limit pushdown interaction

Safe limit pushdown:

```text
No filters:
  backend LIMIT N can be safe if ordering/cardinality semantics acceptable

Exact filters:
  backend WHERE exact + LIMIT N can be safe

Inexact filters:
  backend approximate prefilter + LIMIT N is unsafe if residual may discard rows

Ordering required:
  remote ORDER BY must be guaranteed stable before LIMIT pushdown
```

Provider plan:

```rust
#[derive(Debug, Clone)]
pub struct LimitPushdownPlan {
    pub backend_limit: Option<usize>,
    pub local_limit_required: bool,
    pub reason: String,
}
```

Rules:

```text
If any inexact filter remains:
  backend_limit may be larger candidate cap, not final SQL LIMIT.

If unsupported residual filter remains:
  keep local LimitExec above residual filter unless proven safe.

If ordering not guaranteed:
  do not push Top-K semantics as backend LIMIT.
```

---

## S10.12 Schema adaptation errors

### S10.12.1 Missing backend column

```json
{
  "error_class": "backend_missing_column",
  "phase": "schema_snapshot_normalization",
  "table": "remote.streams",
  "df_field": "mass_flow_kg_h",
  "backend_path": "massFlow",
  "snapshot_id": "remote-20260524T120000Z",
  "decision": "reject_provider_registration",
  "suggested_fix": "refresh metadata or map mass_flow_kg_h to new backend field massRate"
}
```

### S10.12.2 Extra backend column

```json
{
  "warning_class": "backend_extra_column",
  "phase": "schema_snapshot_normalization",
  "backend_field": "debugTrace",
  "decision": "ignore",
  "suggested_fix": "add explicit contract field if this should be exposed"
}
```

### S10.12.3 Nullable mismatch

```json
{
  "error_class": "provider_nullability_mismatch",
  "phase": "runtime_batch_validation",
  "field": "stream_id",
  "schema_nullable": false,
  "observed_null_count": 3,
  "decision": "reject_batch",
  "suggested_fix": "mark field nullable or enforce non-null upstream"
}
```

### S10.12.4 String normalization mismatch

```json
{
  "error_class": "string_representation_mismatch",
  "phase": "backend_type_adaptation",
  "field": "stream_name",
  "expected_arrow_type": "Utf8",
  "actual_arrow_type": "Dictionary(Int32, Utf8)",
  "decision": "cast_or_support_dictionary",
  "suggested_fix": "decode dictionary to Utf8 or implement dictionary string path"
}
```

### S10.12.5 Timestamp unit mismatch

```json
{
  "error_class": "timestamp_unit_mismatch",
  "phase": "backend_type_adaptation",
  "field": "event_ts",
  "backend_type": "timestamp_millis",
  "provider_contract_type": "Timestamp(Nanosecond, UTC)",
  "conversion": "widen_ms_to_ns",
  "lossless": true,
  "decision": "allow_with_cast"
}
```

---

## S10.13 Dynamic schema lifecycle

### S10.13.1 Versioned provider

```rust
#[derive(Debug)]
pub struct VersionedProvider {
    provider_id: String,
    current: arc_swap::ArcSwap<RemoteTableProvider>,
}

impl VersionedProvider {
    pub fn current_provider(&self) -> Arc<RemoteTableProvider> {
        self.current.load_full()
    }

    pub fn swap_provider(&self, next: Arc<RemoteTableProvider>) {
        self.current.store(next);
    }
}
```

### S10.13.2 Query-start freeze

```text
Query start:
  provider Arc cloned from catalog snapshot
  provider.schema() returns frozen SchemaRef
  scan() uses same snapshot/mapping
  ExecutionPlan stores output schema from same snapshot
  stream converts backend rows according to same mapping
```

Never:

```text
schema() returns v1
scan() builds request from v2
stream decodes rows as v3
```

### S10.13.3 Refresh/invalidation flow

```text
remote schema event
  → load new BackendSchemaSnapshot
  → build new Arrow schema + field map
  → compare with old provider contract
  → produce schema drift report
  → if compatible: swap provider/catalog snapshot
  → if incompatible: keep old provider, mark stale/rejected, require migration
```

### S10.13.4 Stale snapshot policy

```rust
#[derive(Debug, Clone)]
pub enum ProviderStalenessPolicy {
    AllowUntilTtl,
    RejectIfStale,
    WarnAndProceed,
    FreezeForJob,
}
```

Agent rules:

```text
Dynamic provider must expose snapshot_id in diagnostics.
Schema refresh must never mutate existing ExecutionPlan.
Incompatible schema refresh must not silently change table output schema.
```

---

## S10.14 ExecutionPlan schema

### S10.14.1 ExecutionPlan stores projected schema

```rust
#[derive(Debug)]
pub struct RemoteExec {
    client: Arc<dyn RemoteClient>,
    snapshot: Arc<BackendSchemaSnapshot>,
    projection: ProjectionPlan,
    filters: FilterPushdownPlan,
    limit: Option<usize>,
    output_schema: SchemaRef,
    properties: Arc<datafusion::physical_plan::PlanProperties>,
}

impl RemoteExec {
    pub fn new(
        client: Arc<dyn RemoteClient>,
        snapshot: Arc<BackendSchemaSnapshot>,
        projection: ProjectionPlan,
        filters: FilterPushdownPlan,
        limit: Option<usize>,
        output_schema: SchemaRef,
    ) -> Self {
        let properties = build_plan_properties(output_schema.clone());
        Self {
            client,
            snapshot,
            projection,
            filters,
            limit,
            output_schema,
            properties,
        }
    }
}
```

### S10.14.2 Output schema consistency

```text
RemoteExec::schema/properties schema
  == RecordBatchStreamAdapter schema
  == every emitted RecordBatch schema
```

`RecordBatchStream` implementations must guarantee every `RecordBatch` returned has the same schema as returned by the stream’s `schema()` method. ([Docs.rs][4])

---

## S10.15 Runtime validation

### S10.15.1 Stream schema equals projected schema

```rust
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::{DataFusionError, Result};

pub fn validate_batch_schema(expected: &SchemaRef, batch: &RecordBatch) -> Result<()> {
    let actual = batch.schema();

    if expected.fields() != actual.fields() {
        return Err(DataFusionError::Execution(format!(
            "RecordBatch schema mismatch: expected={:?}, actual={:?}",
            expected.fields(),
            actual.fields()
        )));
    }

    Ok(())
}
```

### S10.15.2 Validate every batch or sample

```rust
#[derive(Debug, Clone)]
pub enum RuntimeSchemaValidationMode {
    Disabled,
    FirstBatchPerPartition,
    EveryBatch,
}

pub fn should_validate_batch(mode: &RuntimeSchemaValidationMode, batch_index: usize) -> bool {
    match mode {
        RuntimeSchemaValidationMode::Disabled => false,
        RuntimeSchemaValidationMode::FirstBatchPerPartition => batch_index == 0,
        RuntimeSchemaValidationMode::EveryBatch => true,
    }
}
```

Recommended:

```text
dev/test:
  EveryBatch

staging:
  FirstBatchPerPartition or EveryBatch

production:
  FirstBatchPerPartition by default
  EveryBatch for new providers / schema migrations / strict mode
```

### S10.15.3 Empty stream behavior

```text
Empty result must still expose schema.
No first batch is not schema absence.
SendableRecordBatchStream schema must equal ExecutionPlan schema.
collect() returning [] is valid for zero-row table/query.
```

Validation:

```rust
pub fn validate_empty_stream_schema(expected: &SchemaRef, stream_schema: &SchemaRef) -> Result<()> {
    if expected.fields() != stream_schema.fields() {
        return Err(DataFusionError::Execution(
            "empty stream schema does not match expected projected schema".to_string(),
        ));
    }
    Ok(())
}
```

---

## S10.16 Remote row → Arrow batch adapter

### S10.16.1 Row decoder contract

```rust
#[derive(Debug)]
pub struct RemoteRowDecoder {
    projection: ProjectionPlan,
    output_schema: SchemaRef,
    validation_mode: RuntimeSchemaValidationMode,
}

impl RemoteRowDecoder {
    pub fn decode_page(&self, page: RemotePage) -> datafusion::common::Result<RecordBatch> {
        let arrays = self
            .projection
            .output_fields
            .iter()
            .map(|field| self.decode_field(&page, field))
            .collect::<datafusion::common::Result<Vec<_>>>()?;

        let batch = RecordBatch::try_new(self.output_schema.clone(), arrays)?;

        validate_batch_schema(&self.output_schema, &batch)?;

        Ok(batch)
    }

    fn decode_field(
        &self,
        page: &RemotePage,
        field: &ProviderFieldMapping,
    ) -> datafusion::common::Result<datafusion::arrow::array::ArrayRef> {
        // Decode backend values into the exact Arrow type in field.df_field.data_type().
        todo!()
    }
}
```

### S10.16.2 Decoder rules

```text
Decode into output field order.
Decode into exact Arrow data type.
Honor nullable=false by rejecting/cleaning nulls before batch creation.
Normalize timestamps to contract unit/timezone.
Normalize strings to contract representation.
Decode dictionaries only if output type is dictionary.
Generate virtual columns after backend decode.
Do not silently coerce lossy values.
```

---

## S10.17 Provider diagnostics and `EXPLAIN`

### S10.17.1 Provider plan display

Provider `ExecutionPlan` should implement display text that exposes:

```text
provider_name
snapshot_id
projected fields
backend selected fields
exact filters
inexact filters
unsupported filters
limit pushdown decision
partitioning/shards
ordering guarantee
schema fingerprint
```

Example display:

```text
RemoteExec:
  table=streams
  snapshot=remote-20260524T120000Z
  output=[stream_id,mass_flow_kg_h]
  backend_select=[id,massFlow,tenantId]
  exact_filters=[tenantId = 'acme']
  inexact_filters=[search_text ~ 'pump']
  residual_filters=[mass_flow_kg_h > 0]
  backend_limit=None
  partitions=8
```

The attached provider checklist recommends using `EXPLAIN` to verify pushdown, partitioning, and the presence/absence of residual `FilterExec`, `SortExec`, and `RepartitionExec`. 

### S10.17.2 Metrics

Track:

```text
remote_requests
remote_rows_received
remote_rows_output
remote_bytes_received
projection_fields_requested
backend_fields_requested
exact_filters_pushed
inexact_filters_pushed
residual_filter_rows_removed
schema_validation_failures
decode_errors
rate_limit_retries
```

---

## S10.18 Negative tests

### S10.18.1 Projection order test

```text
Provider schema:
  [a, b, c]

Projection:
  [2, 0]

Expected batch schema:
  [c, a]
```

Test:

```rust
#[tokio::test]
async fn provider_respects_projection_order() -> datafusion::common::Result<()> {
    let ctx = SessionContext::new();
    ctx.register_table("t", Arc::new(make_test_provider()))?;

    let df = ctx.sql("SELECT c, a FROM t").await?;
    let batches = df.collect().await?;

    let schema = batches[0].schema();
    assert_eq!(schema.field(0).name(), "c");
    assert_eq!(schema.field(1).name(), "a");

    Ok(())
}
```

### S10.18.2 Backend type drift test

```text
snapshot v1:
  massFlow Double

snapshot v2:
  massFlow String

Expected:
  provider refresh rejected or requires migration
```

### S10.18.3 Nullability mismatch test

```text
schema:
  stream_id Utf8 nullable=false

backend row:
  stream_id = null

Expected:
  batch rejected before/at RecordBatch::try_new
```

### S10.18.4 Inexact filter + limit test

```text
query:
  WHERE fuzzy_match(description, 'pump') AND mass_flow_kg_h > 100
  LIMIT 10

backend:
  supports fuzzy candidate search only inexactly

Expected:
  backend candidate cap != final SQL LIMIT
  residual filter applied by DataFusion/provider
```

### S10.18.5 Empty stream schema test

```text
query returns zero rows
stream schema must equal projected schema
```

---

## S10.19 Test matrix

```text
Schema contract:
  [ ] schema() cheap Arc clone
  [ ] schema() stable across repeated calls
  [ ] field order deterministic
  [ ] hidden columns policy tested
  [ ] backend snapshot fingerprint stored
  [ ] backend type mapping tested
  [ ] nullability mapping tested

Projection:
  [ ] None returns full schema
  [ ] Some([i]) returns projected schema
  [ ] reordered projection respected
  [ ] duplicate projection indices handled according to policy
  [ ] out-of-bounds projection rejected
  [ ] backend selected fields minimal
  [ ] hidden policy fields not emitted

Filters:
  [ ] supports_filters_pushdown length equals filters length
  [ ] exact filters exactly match DataFusion semantics
  [ ] inexact filters retain residual evaluation
  [ ] unsupported filters not pushed
  [ ] hidden tenant predicate injected
  [ ] limit not pushed unsafely before residual filters

Dynamic schema:
  [ ] schema snapshot created
  [ ] refresh compatible change accepted
  [ ] incompatible change rejected
  [ ] old ExecutionPlan uses old snapshot
  [ ] diagnostics include snapshot id

Runtime:
  [ ] stream schema equals ExecutionPlan schema
  [ ] every batch validates under strict mode
  [ ] empty stream exposes schema
  [ ] decode errors are surfaced
  [ ] cancellation stops remote paging
```

---

## S10.20 Deployment advisory

```text
Provider construction:
  load backend metadata once
  normalize names/types/nullability
  build Arrow SchemaRef
  build ProviderFieldMap
  validate mapping invariants
  register provider only after validation
```

```text
Remote API execution:
  no one-call-per-row pattern
  page remote results
  bound page sizes
  honor cancellation
  retry rate-limit errors deliberately
  convert page to Arrow arrays, then emit RecordBatch
```

```text
Projection:
  DataFusion projection indices are canonical schema indices
  backend field order is irrelevant to output order
  include hidden physical fields only for policy/residual evaluation
  never emit hidden fields unless requested and allowed
```

```text
Schema drift:
  freeze schema at query start
  refresh provider by snapshot swap
  reject incompatible backend drift
  require migration for renamed/missing/type-changed backend fields
```

```text
Validation:
  validate every batch in tests
  validate first batch in production by default
  expose strict mode for new providers and migrations
  negative-test projection order, nullability, type drift, empty streams
```

---

## S10.21 Anti-pattern inventory

```text
Custom TableProvider anti-patterns:
  schema() calls remote API every planning pass
  schema() changes after provider registration without versioning
  projection indices treated as backend indices
  projection order ignored
  hidden tenant columns emitted to users
  backend order emitted instead of projected schema order
  full backend response fetched despite projection
  nested projection claimed but not implemented
  Exact filter pushdown claimed for approximate backend search
  LIMIT pushed before residual inexact filters
  timestamp units decoded inconsistently
  dictionary/string mismatch hidden until UDF failure
  nullable=false field emits nulls
  stream schema differs from first/any batch schema
  empty result stream has no reliable schema
  one HTTP request per row
  scan() performs heavy I/O instead of creating plan
  execute() buffers all rows before returning stream
  schema drift silently changes output fields
  provider errors converted to empty result sets
```

The attached anti-pattern inventory for custom providers contains many of the same high-risk cases: returning batches with wrong schema, claiming exact pushdown for approximate remote predicates, applying limit before inexact filters, ignoring projection, stale constraints/statistics, buffering all remote rows, and hiding provider errors as empty results. 

---

## S10.22 Agent checklist

```text
[ ] Prefer existing providers first:
    MemTable / StreamingTable / ViewTable / ListingTable before custom TableProvider.

[ ] Build schema snapshot:
    backend schema
    normalized Arrow schema
    schema fingerprint
    snapshot id

[ ] schema():
    cheap Arc clone
    stable field order
    no remote call
    no dynamic mutation during query

[ ] Build field mapping:
    projection index → canonical field
    canonical field → backend path
    backend type → Arrow type
    hidden/virtual/generated columns marked

[ ] scan():
    validate projection indices
    compute projected output schema
    map backend selected fields
    classify filters Exact/Inexact/Unsupported
    decide limit pushdown safely
    create ExecutionPlan only

[ ] ExecutionPlan:
    stores projected schema
    stores snapshot id
    exposes explain text
    exposes metrics
    creates stream quickly

[ ] Stream:
    performs remote I/O during polling
    batches rows into Arrow arrays
    emits RecordBatch::try_new(output_schema, arrays)
    validates schema under strict/test mode
    handles zero rows with schema intact
    supports cancellation

[ ] Dynamic schema:
    refresh by building new provider
    compare fingerprints
    reject incompatible drift
    never mutate existing plan schema

[ ] Tests:
    projection order
    type mapping
    nullability mismatch
    hidden columns
    exact/inexact pushdown
    empty stream
    schema drift
    EXPLAIN output
```

## S10.23 Minimal provider architecture summary

```text
RemoteTableProvider
  schema: Arc<Schema>
  mapping: Arc<ProviderFieldMap>
  snapshot: Arc<BackendSchemaSnapshot>
  client: Arc<RemoteClient>

schema()
  Arc::clone(schema)

scan(projection, filters, limit)
  projection_plan = mapping.plan_projection(schema, projection)
  filter_plan = mapping.plan_filters(filters)
  limit_plan = plan_limit(limit, filter_plan)
  output_schema = projection_plan.output_schema
  return RemoteExec(...)

RemoteExec::execute(partition, task_ctx)
  create RemoteRecordBatchStream(output_schema, snapshot, projection_plan, filter_plan, limit_plan)

RemoteRecordBatchStream::poll_next()
  fetch backend page
  decode requested backend fields
  synthesize virtual fields
  build arrays in projection order
  RecordBatch::try_new(output_schema, arrays)
  yield batch
```

## S10.24 Typed per-file extensions on `PartitionedFile` (DataFusion 54)

For file-based providers, DataFusion 54 replaces the untyped `PartitionedFile.extensions: Option<Arc<dyn Any + Send + Sync>>` slot with a typed map: `pub extensions: FileExtensions`, where `FileExtensions = datafusion_common::extensions::Extensions`. Attach values with `partitioned_file.with_extension(value)` and read them back type-safely with `partitioned_file.extension::<T>()`; multiple independent extensions coexist without the downcast-one-blob pattern (the legacy `with_extensions(Arc<dyn Any + Send + Sync>)` remains for compatibility). This is the right channel for per-file schema-adaptation state — e.g. a frozen `BackendSchemaSnapshot` or field-mapping table per file — instead of side tables keyed by path. Full coverage of the `Extensions` map API (`insert`/`insert_arc`/`get`/`get_arc`/`contains`/`merge`) is in `datafusion_rust.md` §14.

---

Core operating rule: **the provider schema is the compiler contract; every mapping, pushdown, decode, and batch emission must preserve that contract exactly unless an explicit schema-versioned migration occurs.**

[1]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html?utm_source=chatgpt.com "TableProvider in datafusion::datasource - Rust"
[2]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html?utm_source=chatgpt.com "Custom Table Provider — Apache DataFusion documentation"
[3]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html?utm_source=chatgpt.com "RecordBatch in arrow::record_batch - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/trait.RecordBatchStream.html?utm_source=chatgpt.com "RecordBatchStream in datafusion::execution - Rust"


# DataFusion Advanced — S11) Logical-plan schema propagation and operator output contracts

## S11.0 Objective

Define schema propagation as a **logical-plan invariant**:

```text id="wp95sl"
input LogicalPlan schema
  → operator-specific schema rule
  → output DFSchema
  → optimizer rewrite
  → optimized LogicalPlan schema
  → physical plan schema
  → RecordBatch runtime schema
  → sink/output schema
```

A `LogicalPlan` is a tree of relational operators such as projection and filter; each node transforms an input relation into an output relation, potentially with a different schema. Logical plans can be produced by SQL planning, the DataFrame API, or direct programmatic construction. ([Docs.rs][1]) The attached documentation already establishes the core plan pipeline and the need to validate schemas, alias computed columns, qualify joined names, and avoid manual logical-plan schema mismatches. 

Core invariant:

```text id="fyx4xv"
Every logical operator has a deterministic output schema rule.
Every optimizer rewrite must preserve semantic output schema unless it intentionally changes operator semantics.
Every stable output contract must use explicit aliases, casts, and duplicate-name resolution.
```

---

## S11.1 Schema objects by planning layer

```text id="gcqyp3"
Arrow Schema
  physical ordered fields + metadata

DFSchema
  logical fields + optional relation qualifiers + resolution helpers

LogicalPlan::schema()
  output DFSchemaRef of a logical node

DataFrame::schema()
  output DFSchema of the DataFrame’s current logical plan

ExecutionPlan::schema()
  physical output SchemaRef / properties communication

RecordBatch::schema()
  concrete runtime output schema
```

`DFSchema` wraps Arrow schema information and adds relation/table qualification; some fields may be qualified, some unqualified, and qualified fields carry relation information used for multi-table resolution. ([Docs.rs][2]) `DataFrame` represents a logical set of rows with the same named columns, and its workflow is plan-building before execution. ([Docs.rs][3])

---

## S11.2 Logical schema propagation model

```text id="ioyr79"
For every logical node N:

  input_schemas = N.inputs().map(schema)
  local_exprs   = N.expressions()
  output_schema = schema_rule(N.kind, input_schemas, local_exprs, node_options)

Required:
  output_schema field count deterministic
  output_schema field order deterministic
  output_schema field names deterministic
  output_schema qualifiers defined by operator rule
  output_schema nullability/type derived from ExprSchemable / operator semantics
  output_schema metadata policy explicit
```

DataFusion’s logical-plan guide notes that `LogicalPlanBuilder` is normally preferred over directly constructing `LogicalPlan` enum variants, and the direct construction example uses `display_indent_schema()` to show each logical node’s schema in the plan output. ([Apache DataFusion][4])

---

## S11.3 Operator schema contract table

| Logical operator | Output schema rule                                                                          | Qualifier behavior                                                         | Name risk                | Agent default                                 |
| ---------------- | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------ | --------------------------------------------- |
| Table scan       | table/provider schema, optionally projected                                                 | table/relation qualifier added                                             | source names/case        | normalize source names; qualify after joins   |
| Projection       | one output field per projection expression                                                  | column refs may retain/alter qualifier; aliases usually define output name | expression-derived names | alias every computed expression               |
| Filter           | same as input schema                                                                        | unchanged                                                                  | none                     | schema-preserving                             |
| Aggregate        | group fields + aggregate expression fields                                                  | usually new output scope                                                   | aggregate display names  | alias every aggregate                         |
| Window           | input fields + window expression fields, or projected expressions if SELECT controls output | window expr usually new field                                              | function-derived names   | alias every window expression                 |
| Sort             | same as input schema                                                                        | unchanged                                                                  | none                     | schema-preserving                             |
| Limit / fetch    | same as input schema                                                                        | unchanged                                                                  | none                     | schema-preserving                             |
| Join             | combined left/right schema unless projected                                                 | left/right qualifiers retained before output                               | duplicate names          | explicit projection/aliases after join        |
| Union            | compatible branch schemas; output names usually from first branch/query context             | qualifiers generally not durable output contract                           | position/type/name drift | explicit same-order projection/casts          |
| Distinct         | same as input schema                                                                        | unchanged                                                                  | none                     | schema-preserving, row-set changing           |
| Repartition      | same as input schema                                                                        | unchanged                                                                  | none                     | physical distribution only; schema-preserving |
| Unnest           | list/map expands rows; struct expands columns                                               | depends expression/operator form                                           | unstable/generated names | explicit aliases / struct field extraction    |
| Extension node   | implementer-defined                                                                         | implementer-defined                                                        | silent drift in rewrites | schema method must be authoritative           |

DataFusion’s `UserDefinedLogicalNodeCore` requires `schema()` to return the output schema, `expressions()` to return node-local expressions, and `with_exprs_and_inputs()` to reconstruct a rewritten node during optimization using expressions/inputs in the same order as `expressions()`/`inputs()`. ([Docs.rs][5])

---

## S11.4 Table scan schema

### Rule

```text id="az7abh"
TableScan output schema =
  TableSource / TableProvider schema
  qualified by table reference / alias
  optionally projected by scan projection
```

DataFusion’s manual logical-plan example creates a `LogicalTableSource` from a `SchemaRef`, builds a `TableScan`, and shows the output schema as `[id:Int32;N, name:Utf8;N]` in `display_indent_schema()`. ([Apache DataFusion][4])

### LogicalPlanBuilder scan syntax

```rust id="f1vazn"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::logical_expr::{LogicalPlanBuilder, LogicalTableSource};

let schema = SchemaRef::new(Schema::new(vec![
    Field::new("stream_id", DataType::Utf8, false),
    Field::new("mass_flow_kg_h", DataType::Float64, false),
]));

let source = Arc::new(LogicalTableSource::new(schema));

let plan = LogicalPlanBuilder::scan("streams", source, None)?
    .build()?;
```

### Scan invariants

```text id="q6ujr4"
scan field order = provider/source schema order
projection indices = source schema indices
unqualified column resolution valid only if no ambiguity
qualified scan fields must retain relation identity until output boundary
```

### Agent rules

```text id="x4yq1u"
Normalize source names before stable contracts.
Use lowercase snake_case field names in provider schema.
Do not assume Arrow Schema stores catalog/table qualifier semantics.
After scan joins, prefer qualified column references.
```

---

## S11.5 Projection schema

### Rule

```text id="o2w2ul"
Projection output schema =
  Vec<Expr>.map(expr.to_field(input_df_schema))
```

`Expr::qualified_name()` returns the qualifier and schema name of an expression when it forms an output field; `Expr::schema_name()` returns the field name used in a plan output, and `Expr::to_field()` determines field metadata, type, and nullability. ([Docs.rs][6])

### SQL

```sql id="9r7l4j"
SELECT
  stream_id AS stream_id,
  mass_flow_kg_h * 24.0 AS mass_flow_kg_d,
  CAST(sulfur_wt_pct AS DOUBLE) AS sulfur_wt_pct
FROM streams;
```

### DataFrame

```rust id="g1h4tt"
use datafusion::prelude::*;

let df = df.select(vec![
    col("stream_id").alias("stream_id"),
    (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d"),
    col("sulfur_wt_pct").cast_to(
        &datafusion::arrow::datatypes::DataType::Float64,
        df.schema(),
    )?.alias("sulfur_wt_pct"),
])?;
```

### Output-name rules

```text id="68a3oq"
Column reference:
  output name usually source field name

Alias:
  output name = alias

Computed expression without alias:
  output name = expression display-derived name

Stable contract:
  computed expression must be aliased
```

DataFusion has a formal output-field-name specification for SQL and DataFrame queries because output record-batch field names are part of observable query behavior. ([Apache DataFusion][7])

### Metadata / qualifier caution

```text id="dos8v5"
Projection may preserve metadata for direct column refs.
Computed expressions generally create new field semantics.
Alias metadata is the safest way to attach output metadata.
Qualifiers may be altered/lost by aliasing and output conversion.
```

---

## S11.6 Filter schema

### Rule

```text id="bkgw5g"
Filter output schema = input schema
Filter predicate type = Boolean or coercible to Boolean
Filter row semantics = keep TRUE; discard FALSE/NULL
```

SQL:

```sql id="l0pv8x"
SELECT *
FROM streams
WHERE mass_flow_kg_h > 0.0;
```

DataFrame:

```rust id="sob0r7"
let df = df.filter(col("mass_flow_kg_h").gt(lit(0.0)))?;
```

### Invariants

```text id="vah7rf"
filter does not add/drop/reorder fields
filter does not rename fields
filter should preserve qualifiers
filter may be pushed down, but schema remains child schema
```

### Agent rules

```text id="zyhros"
Filter is schema-preserving.
Use filter for row predicates only.
Do not encode schema changes in filter nodes.
Schema snapshot before/after filter should be equal.
```

---

## S11.7 Aggregate schema

### Rule

```text id="z5e9ot"
Aggregate output schema =
  group_expr fields
  + aggregate_expr fields
```

Aggregate output names are high risk because function display names can be verbose or version-sensitive; alias every aggregate.

SQL:

```sql id="j6niuc"
SELECT
  unit_id AS unit_id,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h,
  AVG(sulfur_wt_pct) AS avg_sulfur_wt_pct,
  COUNT(*) AS row_count
FROM streams
GROUP BY unit_id;
```

DataFrame:

```rust id="l45zbt"
use datafusion::functions_aggregate::expr_fn::{avg, count, sum};
use datafusion::prelude::*;

let df = df.aggregate(
    vec![col("unit_id").alias("unit_id")],
    vec![
        sum(col("mass_flow_kg_h")).alias("total_mass_flow_kg_h"),
        avg(col("sulfur_wt_pct")).alias("avg_sulfur_wt_pct"),
        count(lit(1)).alias("row_count"),
    ],
)?;
```

### Aggregate invariants

```text id="rcwyvz"
input non-group fields disappear unless aggregated
aggregate output type may differ from input type
aggregate output nullability depends on function + empty/filter semantics
aggregate expressions create new fields
grouping expression aliases stabilize group output names
```

### Agent rules

```text id="gbq5b7"
Alias all aggregate expressions.
Alias non-trivial grouping expressions.
Do not assume SUM/AVG output type equals input type.
Audit aggregate output with df.schema() and arrow_typeof tests.
```

---

## S11.8 Window schema

### Rule

```text id="e0btgn"
Window expression output schema =
  input fields retained if selected
  + window expression field(s)
```

SQL:

```sql id="j4p2iy"
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  ROW_NUMBER() OVER (
    PARTITION BY unit_id
    ORDER BY mass_flow_kg_h DESC, stream_id ASC
  ) AS unit_flow_rank
FROM streams;
```

### Window invariants

```text id="68s6nx"
window output is row-preserving
window expression creates new field
window function output type/function-specific
window alias required for stable schema
QUALIFY can filter rows but not change schema
```

### Agent rules

```text id="uocn9l"
Alias every window expression.
Use deterministic ORDER BY in ranking windows.
Validate window output schema separately from final ORDER BY.
```

---

## S11.9 Sort schema

### Rule

```text id="cmxqw5"
Sort output schema = input schema
Sort changes row order, not columns
```

SQL:

```sql id="jf6y5w"
SELECT stream_id, mass_flow_kg_h
FROM streams
ORDER BY mass_flow_kg_h DESC, stream_id ASC;
```

DataFrame:

```rust id="79tx0v"
let df = df.sort(vec![
    col("mass_flow_kg_h").sort(false, false),
    col("stream_id").sort(true, true),
])?;
```

### Agent rules

```text id="zum5jh"
Sort is schema-preserving.
Use sort before limit for deterministic top-N.
Snapshot schema and rows separately; order changes row sequence only.
```

---

## S11.10 Limit / fetch schema

### Rule

```text id="563hub"
Limit output schema = input schema
Limit changes row count only
```

SQL:

```sql id="d2izig"
SELECT stream_id, mass_flow_kg_h
FROM streams
ORDER BY stream_id
LIMIT 100;
```

DataFrame:

```rust id="s6csao"
let df = df.limit(0, Some(100))?;
```

### Agent rules

```text id="1raljg"
Limit is schema-preserving.
Limit without order is not deterministic top-N.
Limit pushdown must not change schema or residual-filter correctness.
```

---

## S11.11 Join schema

### Rule

```text id="qqg3hk"
Join output schema =
  depending join type:
    inner/left/right/full/cross: left fields + right fields
    semi/anti: one side only
    natural/using: merged/coalesced join columns depending planner semantics
  unless followed by explicit projection
```

DataFusion’s SELECT syntax supports inner, left/right/full outer, natural, cross, semi, anti, lateral, and left lateral joins. ([Apache DataFusion][8])

### Safe SQL

```sql id="w6s9po"
SELECT
  s.stream_id AS stream_id,
  s.unit_id AS stream_unit_id,
  u.unit_id AS unit_dim_id,
  u.unit_name AS unit_name
FROM streams AS s
JOIN units AS u
  ON s.unit_id = u.unit_id;
```

### Unsafe SQL

```sql id="cl1ygp"
SELECT *
FROM streams AS s
JOIN units AS u
  ON s.unit_id = u.unit_id;
```

### Join invariants

```text id="627eoj"
join can introduce duplicate unqualified field names
outer joins can widen nullability
semi/anti joins project only preserved side
natural/using joins have special output-column semantics
qualifiers are critical before final projection
```

### Agent rules

```text id="l5kd3h"
After joins, never depend on unqualified duplicate names.
Immediately project to stable output schema.
Use semantic aliases for duplicate IDs/keys.
Do not use SELECT * in stable join outputs.
```

---

## S11.12 Union schema

### Rule

```text id="eiwmhg"
Union output schema =
  compatible branch schemas
  field count must align
  field order matters for positional SQL UNION
  output field names generally derived from first branch/query context
```

SQL:

```sql id="g7ju5v"
SELECT
  stream_id AS stream_id,
  CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h
FROM streams_current

UNION ALL

SELECT
  stream_id AS stream_id,
  CAST(mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h
FROM streams_history;
```

DataFrame:

```rust id="f3gqce"
// Exact position/schema union
let df = current.union(history)?;

// Name-aligned schema-evolution union
let df = current.union_by_name(history)?;
```

### Union invariants

```text id="x5ljc9"
schema compatibility is required
implicit coercion may change output types
output names should be stabilized by explicit projections
union distinct changes rows, not schema
name-based union is not the same as positional SQL union
```

### Agent rules

```text id="k1d9av"
Project both branches explicitly.
Cast both branches to target schema.
Alias every expression in both branches.
Prefer UNION ALL unless dedupe is required.
Snapshot post-union schema.
```

---

## S11.13 Distinct schema

### Rule

```text id="722c1p"
Distinct output schema = input schema
Distinct changes row multiplicity only
```

SQL:

```sql id="4vd7uk"
SELECT DISTINCT
  unit_id,
  product_code
FROM streams;
```

### Agent rules

```text id="6do4sh"
Distinct is schema-preserving.
Distinct may require hashing/sorting.
Do not use distinct to fix duplicate field names; use aliases/projection.
```

---

## S11.14 Repartition schema

### Rule

```text id="8rtxfo"
Repartition output schema = input schema
Repartition changes physical distribution, not logical schema
```

DataFusion’s physical-plan docs describe `RepartitionExec` as mapping input partitions to output partitions based on a partitioning scheme, optionally preserving input row order. ([Docs.rs][9])

### Agent rules

```text id="0nphuo"
Repartition is not a logical output schema change.
Do not encode field additions/removals in repartition-like extension nodes.
Snapshot physical distribution separately from schema.
```

---

## S11.15 Unnest schema

### Rule

```text id="6k0ik6"
Unnest list/map:
  row cardinality changes
  output includes unnested element/value fields

Unnest struct:
  column shape changes
  output fields are struct children or generated names
```

DataFusion’s logical-expression item list describes `Unnest` as representing unnesting operation on a list column, including recursion depth and output column name after unnesting. ([Docs.rs][10])

### Safe scalar unnest

```sql id="b22sgc"
SELECT
  stream_id,
  unnest(tags) AS tag
FROM streams;
```

### Safer struct extraction instead of broad unnest

```sql id="21c6iz"
SELECT
  stream_id,
  assay['api_gravity'] AS assay_api_gravity,
  assay['sulfur_wt_pct'] AS assay_sulfur_wt_pct
FROM streams;
```

### Agent rules

```text id="tx9p8d"
Always alias scalar unnest outputs.
For struct unnest, prefer explicit field extraction + aliases in stable contracts.
Unnest can change row count; schema validation must be paired with cardinality tests.
```

---

## S11.16 Extension logical node schema

### Rule

```text id="qz5uj2"
Extension node output schema = implementer-defined DFSchemaRef
```

Custom logical nodes use `UserDefinedLogicalNodeCore`; required methods include `name`, `inputs`, `schema`, `expressions`, `fmt_for_explain`, and `with_exprs_and_inputs`, and the documentation states `schema()` must return the output schema of the logical node while `with_exprs_and_inputs()` is used during optimization rewrites. ([Docs.rs][5])

### Skeleton

```rust id="4vqy3k"
use datafusion::common::{DFSchemaRef, Result};
use datafusion::logical_expr::{Expr, LogicalPlan, UserDefinedLogicalNodeCore};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SampleLogicalNode {
    input: LogicalPlan,
    schema: DFSchemaRef,
    fraction_expr: Expr,
}

impl SampleLogicalNode {
    pub fn try_new(input: LogicalPlan, fraction_expr: Expr) -> Result<Self> {
        let schema = input.schema().clone(); // sample preserves columns
        Ok(Self { input, schema, fraction_expr })
    }
}

impl UserDefinedLogicalNodeCore for SampleLogicalNode {
    fn name(&self) -> &str {
        "Sample"
    }

    fn inputs(&self) -> Vec<&LogicalPlan> {
        vec![&self.input]
    }

    fn schema(&self) -> &DFSchemaRef {
        &self.schema
    }

    fn expressions(&self) -> Vec<Expr> {
        vec![self.fraction_expr.clone()]
    }

    fn fmt_for_explain(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sample: fraction={}", self.fraction_expr)
    }

    fn with_exprs_and_inputs(
        &self,
        exprs: Vec<Expr>,
        inputs: Vec<LogicalPlan>,
    ) -> Result<Self> {
        Self::try_new(inputs[0].clone(), exprs[0].clone())
    }
}
```

### Extension invariants

```text id="fai7iu"
schema() must be authoritative
expressions() must include node-local expressions only
inputs() order must match with_exprs_and_inputs reconstruction
with_exprs_and_inputs() must not silently change schema
projection/predicate/limit pushdown hooks must preserve schema semantics
```

The attached extension-node section makes the same invariants explicit and warns extension nodes not to expose child expressions or silently change output schema during optimizer reconstruction. 

---

## S11.17 Output field naming

### S11.17.1 Aliases as schema contracts

```sql id="1253hm"
SELECT
  mass_flow_kg_h * 24.0 AS mass_flow_kg_d
FROM streams;
```

```rust id="udaoex"
(col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d")
```

DataFusion’s optimizer guide emphasizes that DataFusion identifies columns by string names, so optimizer rewrites must preserve expression names, typically by adding aliases when rewriting expressions. ([Apache DataFusion][11])

### S11.17.2 Expression display-derived names

Bad:

```sql id="g3krdv"
SELECT mass_flow_kg_h * 24.0
FROM streams;
```

Risk:

```text id="bd19sc"
output name can be expression-derived
optimizer rewrite can require alias preservation
formatting can vary across versions
downstream schema snapshots can drift
```

DataFusion’s output-field-name specification exists because output field names are generated from user queries and apply to both SQL and DataFrame planning. ([Apache DataFusion][7])

### S11.17.3 Aggregate names

Bad:

```sql id="k8teyj"
SELECT SUM(mass_flow_kg_h)
FROM streams;
```

Good:

```sql id="w7vi58"
SELECT SUM(mass_flow_kg_h) AS total_mass_flow_kg_h
FROM streams;
```

### S11.17.4 Duplicate join names

Bad:

```sql id="d2q2mk"
SELECT *
FROM orders o
JOIN customers c ON o.customer_id = c.customer_id;
```

Good:

```sql id="x2wkng"
SELECT
  o.id AS order_id,
  o.customer_id AS order_customer_id,
  c.customer_id AS customer_id
FROM orders o
JOIN customers c ON o.customer_id = c.customer_id;
```

### S11.17.5 Unnest names

Bad:

```sql id="g86q6o"
SELECT unnest(tags)
FROM streams;
```

Good:

```sql id="krrvc4"
SELECT unnest(tags) AS tag
FROM streams;
```

---

## S11.18 Qualifier propagation

### S11.18.1 Scan qualifiers

```text id="fgbudl"
TableScan streams:
  streams.stream_id
  streams.mass_flow_kg_h
```

Use qualifiers for multi-table planning; `DFSchema` explicitly models qualified fields as fields with relation names. ([Docs.rs][2])

### S11.18.2 Projection qualifier retention/loss

```text id="llsovx"
SELECT s.stream_id
  may retain relation context during planning

SELECT s.stream_id AS stream_id
  output name = stream_id
  alias may remove/change qualifier for output field

Arrow output:
  qualifiers do not survive as DataFusion planner qualifiers
```

### S11.18.3 Join qualifier retention

```text id="m0habs"
Before final projection:
  s.stream_id
  u.unit_id
  s.unit_id
  u.unit_name

After stable projection:
  stream_id
  stream_unit_id
  unit_dim_id
  unit_name
```

### S11.18.4 Alias qualifier changes

`Expr::qualified_name()` returns the qualifier and name used when the expression forms an output field; aliasing can therefore intentionally change output identity. ([Docs.rs][6])

### S11.18.5 Final output stripping

```text id="v65r3y"
Final output schemas:
  should not rely on relation qualifiers
  should use unique canonical field names
  should be valid Arrow/JSON/Parquet/API fields
```

Agent rule:

```text id="ne2ujd"
Preserve qualifiers until all ambiguity is resolved.
Strip/rename qualifiers at final projection only by explicit aliases.
```

---

## S11.19 Validation hooks

### S11.19.1 `df.schema()`

```rust id="lxlvpx"
let df_schema = df.schema();
println!("{df_schema:#?}");
```

Use for current DataFrame output schema before execution. `DataFrame` represents a logical set of rows with named columns, and the DataFrame workflow builds and transforms plans until an action executes them. ([Docs.rs][3])

### S11.19.2 `logical_plan().schema()`

```rust id="v4e7gu"
let plan = df.logical_plan();
let schema = plan.schema();
println!("{}", plan.display_indent_schema());
```

The logical-plan docs show `display_indent_schema()` printing a logical plan with each node’s schema, making it a useful schema-propagation diagnostic. ([Apache DataFusion][4])

### S11.19.3 Optimized plan schema

```rust id="vz33bw"
let optimized = df.clone().into_optimized_plan()?;
let optimized_schema = optimized.schema();
```

Use only in tests/diagnostics; the attached DataFrame guidance warns against extracting optimized/unoptimized plans in production because it loses the `SessionState` context. 

### S11.19.4 Physical plan schema

```rust id="c7fshc"
let physical = df.create_physical_plan().await?;
let physical_schema = physical.schema();
```

`ExecutionPlan` represents physical plan nodes, and `schema()` / `properties()` communicate output properties to the optimizer; `execute` produces asynchronous `RecordBatch` streams. ([Docs.rs][12])

### S11.19.5 Runtime batch schema

```rust id="krf6a8"
let batches = df.collect().await?;

if let Some(first) = batches.first() {
    let expected = first.schema();
    for (idx, batch) in batches.iter().enumerate() {
        assert_eq!(
            expected.fields(),
            batch.schema().fields(),
            "batch {idx} schema drift"
        );
    }
}
```

---

## S11.20 Schema-propagation test harness

```rust id="k2yf8k"
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::DFSchema;
use datafusion::prelude::*;

pub fn assert_field(
    schema: &DFSchema,
    name: &str,
    data_type: &DataType,
    nullable: bool,
) -> datafusion::error::Result<()> {
    let field = schema.field_with_unqualified_name(name)?;

    assert_eq!(field.data_type(), data_type, "type mismatch for {name}");
    assert_eq!(field.is_nullable(), nullable, "nullability mismatch for {name}");

    Ok(())
}

#[tokio::test]
async fn projection_schema_is_stable() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();

    let df = ctx
        .sql("SELECT 1.0 AS mass_flow_kg_h")
        .await?
        .select(vec![
            (col("mass_flow_kg_h") * lit(24.0)).alias("mass_flow_kg_d")
        ])?;

    assert_field(df.schema(), "mass_flow_kg_d", &DataType::Float64, false)?;

    let optimized = df.clone().into_optimized_plan()?;
    assert!(optimized.schema().field_with_unqualified_name("mass_flow_kg_d").is_ok());

    let physical = df.create_physical_plan().await?;
    assert_eq!(physical.schema().field(0).name(), "mass_flow_kg_d");

    Ok(())
}
```

---

## S11.21 Operator schema diagnostics

### Projection alias missing

```json id="imcp1s"
{
  "error_class": "unstable_output_field_name",
  "phase": "logical_projection_validation",
  "operator": "Projection",
  "expr": "mass_flow_kg_h * Float64(24)",
  "reason": "computed expression lacks explicit alias",
  "suggested_fix": "mass_flow_kg_h * 24.0 AS mass_flow_kg_d"
}
```

### Join duplicate names

```json id="ma9j35"
{
  "error_class": "duplicate_join_output_field",
  "phase": "logical_join_validation",
  "operator": "Join",
  "field_name": "id",
  "left_candidate": "orders.id",
  "right_candidate": "customers.id",
  "suggested_fix": "project o.id AS order_id, c.id AS customer_id"
}
```

### Optimizer schema drift

```json id="hwxihh"
{
  "error_class": "optimizer_schema_drift",
  "phase": "optimized_logical_plan_validation",
  "before_schema_fingerprint": "sha256:...",
  "after_schema_fingerprint": "sha256:...",
  "operator": "Projection",
  "reason": "rewrite changed output field name",
  "suggested_fix": "wrap rewritten expression in Alias preserving original output name"
}
```

---

## S11.22 Agent schema contract validator

```rust id="9bkem7"
use datafusion::common::{DFSchema, DataFusionError, Result};

#[derive(Debug, Clone)]
pub struct LogicalSchemaPolicy {
    pub require_unique_names: bool,
    pub reject_expression_derived_names: bool,
    pub require_lower_snake_case: bool,
}

impl Default for LogicalSchemaPolicy {
    fn default() -> Self {
        Self {
            require_unique_names: true,
            reject_expression_derived_names: true,
            require_lower_snake_case: true,
        }
    }
}

pub fn validate_logical_output_schema(
    schema: &DFSchema,
    policy: &LogicalSchemaPolicy,
) -> Result<()> {
    let mut seen = std::collections::HashSet::new();

    for field in schema.fields() {
        let name = field.name();

        if policy.require_unique_names && !seen.insert(name.clone()) {
            return Err(DataFusionError::Plan(format!(
                "duplicate output field name `{name}`"
            )));
        }

        if policy.require_lower_snake_case && !is_lower_snake_case(name) {
            return Err(DataFusionError::Plan(format!(
                "non-canonical output field name `{name}`"
            )));
        }

        if policy.reject_expression_derived_names && looks_expression_derived(name) {
            return Err(DataFusionError::Plan(format!(
                "expression-derived field name `{name}`; add explicit alias"
            )));
        }
    }

    Ok(())
}

fn looks_expression_derived(name: &str) -> bool {
    name.contains('(')
        || name.contains(')')
        || name.contains('+')
        || name.contains('*')
        || name.contains('/')
        || name.contains('=')
        || name.contains(' ')
}

fn is_lower_snake_case(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('_')
        && !name.ends_with('_')
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !name.chars().next().unwrap().is_ascii_digit()
}
```

---

## S11.23 Optimizer rewrite schema safety

### Rule

```text id="y5gldw"
Optimizer rewrite may change expression internals.
Optimizer rewrite must not change output field identity unless operator semantics change.
```

DataFusion’s optimizer guide explicitly states that because DataFusion identifies columns by string name, expression names must not change when the optimizer rewrites expressions; adding an alias is the usual preservation mechanism. ([Apache DataFusion][11])

### Unsafe rewrite

```text id="7la9rh"
original:
  Projection: 1 + 2 AS x

rewrite:
  Projection: 3

result:
  output name may change from x / 1 + 2 to 3
```

### Safe rewrite

```text id="i1sttg"
original:
  1 + 2 AS x

rewrite:
  3 AS x
```

### Agent rules

```text id="3b6wlr"
For optimizer/analyzer/codegen passes:
  preserve top-level Alias
  preserve alias metadata if contract-critical
  compare pre/post DFSchema
  reject rewrite if output schema fingerprint changes unexpectedly
```

---

## S11.24 Extension-node schema safety

```text id="acibix"
Extension logical node must declare:
  input schemas
  output schema
  expression list
  expression/input reconstruction order
  pushdown behavior
  schema-preservation claim
```

Required tests:

```text id="pfihrb"
extension_node.schema() correct
with_exprs_and_inputs() preserves schema
display_indent_schema includes expected fields
optimizer rewrite does not drift schema
projection pushdown maps output columns to child columns correctly
predicate pushdown does not change output schema
limit pushdown safe for cardinality/order semantics
```

`UserDefinedLogicalNodeCore` also provides hooks such as `necessary_children_exprs` for projection-pushdown integration and `supports_limit_pushdown`; these must be implemented only when semantics are safe. ([Docs.rs][5])

---

## S11.25 Deployment advisory

```text id="uvea6u"
Generated plans:
  use explicit projection lists
  alias every computed expression
  qualify columns after joins
  reject duplicate output names
  validate df.schema() before execution

Views/CTAS:
  no SELECT *
  explicit aliases/casts
  snapshot DESCRIBE + df.schema()
  verify schema after optimizer rewrite

Custom optimizer rules:
  preserve expression names
  use aliases after simplification
  test pre/post schema fingerprints
  preserve field metadata where contract-critical

Custom logical nodes:
  schema() is authoritative
  with_exprs_and_inputs() must preserve semantic output schema
  projection/predicate/limit hooks require proof

Services:
  expose schema/fingerprint alongside result
  validate logical schema before streaming
  validate physical/batch schema in strict mode
```

---

## S11.26 Anti-pattern inventory

```text id="t59vm8"
Logical schema anti-patterns:
  relying on expression display names
  aggregate without alias
  window function without alias
  SELECT * in stable contracts
  SELECT * after join
  unqualified id after join
  duplicate aliases
  optimizer rewrite drops aliases
  extension node changes schema in with_exprs_and_inputs
  treating filter/sort/limit as schema-changing
  assuming union branches align by name in SQL UNION
  using DISTINCT to fix duplicate field names
  unnest without alias
  struct unnest in stable output without explicit field aliases
  stripping qualifiers before join/projection disambiguation
  validating only runtime rows, not logical schema
  snapshotting rows without schema snapshot
  comparing physical plan strings as stable schema contracts
```

---

## S11.27 Agent checklist

```text id="i6vw9p"
[ ] For every logical operator, identify output schema rule.
[ ] For every projection expression, ensure alias if computed.
[ ] For every aggregate expression, ensure alias.
[ ] For every window expression, ensure alias.
[ ] For every join, resolve duplicate names with explicit projection.
[ ] For every union, align field count/order/types explicitly.
[ ] For every unnest, alias output fields.
[ ] Preserve qualifiers until ambiguity is resolved.
[ ] Strip/rename qualifiers only in final projection.
[ ] Validate df.schema() before execution.
[ ] Validate logical_plan().schema() / display_indent_schema() in tests.
[ ] Validate optimized plan schema after optimizer rewrites.
[ ] Validate physical plan schema before custom execution.
[ ] Validate RecordBatch schemas in runtime tests.
[ ] Snapshot schema alongside row output.
[ ] Reject optimizer/codegen rewrites that change output schema unexpectedly.
```

## S11.28 Minimal schema-propagation audit flow

```rust id="m3ga23"
use datafusion::prelude::*;

pub async fn audit_schema_propagation(
    df: DataFrame,
) -> datafusion::error::Result<()> {
    let policy = LogicalSchemaPolicy::default();

    // 1. Current DataFrame logical output schema.
    validate_logical_output_schema(df.schema(), &policy)?;

    // 2. Unoptimized logical plan schema.
    let logical = df.logical_plan();
    validate_logical_output_schema(logical.schema(), &policy)?;

    // 3. Optimized logical plan schema. Testing/diagnostics only.
    let optimized = df.clone().into_optimized_plan()?;
    validate_logical_output_schema(optimized.schema(), &policy)?;

    // 4. Physical output schema.
    let physical = df.create_physical_plan().await?;
    let physical_schema = physical.schema();

    // 5. Runtime batches.
    let batches = df.collect().await?;
    for batch in &batches {
        assert_eq!(batch.schema().fields(), physical_schema.fields());
    }

    Ok(())
}
```

Core operating rule: **logical schema propagation is the contract layer between query meaning and runtime execution; aliases, qualifiers, and operator-specific schema rules must be treated as first-class correctness constraints, not display details.**

[1]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/enum.LogicalPlan.html?utm_source=chatgpt.com "LogicalPlan in datafusion_expr::logical_plan - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html?utm_source=chatgpt.com "DFSchema in datafusion::common - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"
[4]: https://datafusion.apache.org/_sources/library-user-guide/building-logical-plans.md.txt?utm_source=chatgpt.com "building-logical-plans.md.txt - Apache DataFusion"
[5]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/trait.UserDefinedLogicalNodeCore.html?utm_source=chatgpt.com "UserDefinedLogicalNodeCore in datafusion_expr"
[6]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html?utm_source=chatgpt.com "Expr in datafusion::logical_expr - Rust"
[7]: https://datafusion.apache.org/contributor-guide/specification/output-field-name-semantic.html?utm_source=chatgpt.com "Output field name semantics - Apache DataFusion"
[8]: https://datafusion.apache.org/user-guide/sql/select.html?utm_source=chatgpt.com "SELECT syntax — Apache DataFusion documentation"
[9]: https://docs.rs/datafusion/latest/datafusion/physical_plan/index.html?utm_source=chatgpt.com "datafusion::physical_plan - Rust"
[10]: https://docs.rs/datafusion/latest/datafusion/logical_expr/index.html?utm_source=chatgpt.com "datafusion::logical_expr - Rust"
[11]: https://datafusion.apache.org/library-user-guide/query-optimizer.html?utm_source=chatgpt.com "Query Optimizer — Apache DataFusion documentation"
[12]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"


# DataFusion Advanced — S12) Nested, partition, and virtual-column schemas

## S12.0 Objective

Consolidate **non-flat schema behavior** into one contract model:

```text id="mcokh2"
NonFlatSchema
  ├─ nested fields
  │   ├─ Struct
  │   ├─ List / LargeList / FixedSizeList
  │   ├─ Map
  │   └─ nested nullability / evolution
  │
  ├─ partition columns
  │   ├─ Hive path-derived columns
  │   ├─ write-time PARTITIONED BY
  │   ├─ table schema vs file schema
  │   └─ partition evolution / conflicts
  │
  └─ virtual columns
      ├─ path-derived columns
      ├─ tenant columns
      ├─ provenance columns
      ├─ ingestion timestamp columns
      └─ generated/provider columns
```

The attached documentation already has strong nested-type coverage: arrays/lists, structs, maps, field access, `unnest`, nested Parquet/Arrow I/O, struct coercion, and nested schema design. The gap is to treat **partition columns and virtual columns as schema elements with lifecycle, compatibility, visibility, and enforcement rules**, not as incidental file/provider details.

DataFusion’s file datasource layer now has an explicit `TableSchema` helper for partitioned data: it distinguishes **file schema**, the schema of actual data files, from **partition columns**, whose values are encoded in directory structure but not stored in the files. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/datasource/table_schema/struct.TableSchema.html))

---

## S12.1 Non-flat schema taxonomy

```text id="4f0owx"
Flat physical columns:
  stream_id: Utf8
  mass_flow_kg_h: Float64

Nested columns:
  assay: Struct<api_gravity: Float64, sulfur_wt_pct: Float64>
  tags: List<Utf8>
  attributes: Map<Utf8, Utf8>
  embedding: FixedSizeList<Float32, 768>

Partition columns:
  event_date: Date32        -- path-derived
  site: Utf8                -- path-derived
  case_id: Utf8             -- sometimes file payload, sometimes partition

Virtual columns:
  tenant_id: Utf8           -- provider-enforced, hidden or visible
  source_file_path: Utf8    -- path-derived provenance
  ingestion_ts: Timestamp   -- provider/generated
  row_number: UInt64        -- generated / synthetic
```

Operating distinction:

```text id="efiz15"
file schema:
  fields physically stored in file payload

table schema:
  file schema + partition columns + visible virtual columns

provider schema:
  table schema as exposed to DataFusion planner

output schema:
  final API/sink fields after aliases/projection
```

---

## S12.2 Nested nullability model

Nested nullability is multi-level. Agents must not collapse it into a single nullable bit.

```text id="wajzeg"
Struct nullability:
  struct_col is NULL
  struct_col is non-null but field is NULL
  struct_col is non-null and field missing/impossible by schema

List nullability:
  list_col is NULL
  list_col is empty []
  list_col has NULL element
  list_col has non-null elements

Map nullability:
  map_col is NULL
  map_col is empty {}
  map key missing
  map key exists with NULL value
  map value type nullable / non-nullable
```

Arrow `StructArray` can represent top-level null struct values and child arrays separately, while `RecordBatch` itself cannot represent a whole-row null; `StructArray` is nested and can have a null bitmap for the struct slots. ([docs.rs](https://docs.rs/arrow/latest/arrow/array/struct.StructArray.html))

---

## S12.3 Null struct vs null field

### S12.3.1 Schema

```rust id="qrgctl"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field};

let assay = Field::new_struct(
    "assay",
    vec![
        Arc::new(Field::new("api_gravity", DataType::Float64, true)),
        Arc::new(Field::new("sulfur_wt_pct", DataType::Float64, true)),
    ],
    true, // assay struct itself nullable
);
```

Meaning:

```text id="q7hnrm"
assay nullable=true:
  entire assay object may be NULL

api_gravity nullable=true:
  assay may exist but api_gravity may be NULL

sulfur_wt_pct nullable=true:
  assay may exist but sulfur_wt_pct may be NULL
```

SQL access:

```sql id="yliyzk"
SELECT
  assay IS NULL AS assay_is_null,
  assay['api_gravity'] IS NULL AS api_gravity_is_null
FROM assays;
```

Caution:

```text id="760lf2"
assay['api_gravity'] IS NULL may be true because:
  assay is NULL
  api_gravity is NULL
  field extraction returns NULL due nested value state
```

Agent rule:

```text id="gbdxu3"
If business semantics distinguish “missing assay” from “assay exists with missing API gravity”,
preserve an explicit quality/state field:
  assay_present
  assay_api_gravity_quality
  assay_source_status
```

---

## S12.4 Null list vs empty list vs null element

### S12.4.1 Schema

```rust id="a71vyz"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field};

let tags = Field::new_list(
    "tags",
    Arc::new(Field::new_list_field(DataType::Utf8, true)), // item nullable
    true, // list nullable
);
```

Meaning:

```text id="k148w1"
tags = NULL:
  unknown / absent list

tags = []:
  known empty list

tags = ['a', NULL, 'b']:
  known list with unknown/missing item
```

### S12.4.2 SQL tests

```sql id="zsx77q"
SELECT
  tags IS NULL AS tags_is_null,
  array_empty(tags) AS tags_is_empty,
  array_length(tags) AS tags_length
FROM streams;
```

DataFusion’s nested functions include array/list shape and membership functions such as `array_length`, `array_empty`, and many `array_*` / `list_*` functions. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/scalar_functions.html))

### S12.4.3 Agent rules

```text id="q5z5s7"
Never treat NULL list and empty list as equivalent without explicit business policy.
Never treat NULL element and missing element as equivalent.
When unnesting, test how NULL/empty lists affect row output under pinned version.
```

---

## S12.5 Missing map key vs null map value

### S12.5.1 Schema

```text id="h8ohks"
attributes: Map<Utf8, Utf8?>
```

States:

```text id="02i44q"
attributes is NULL
attributes is {}
key 'source' missing
key 'source' exists with NULL value
key 'source' exists with 'web'
```

SQL:

```sql id="f93iph"
SELECT
  element_at(attributes, 'source') AS source_value,
  map_keys(attributes) AS attr_keys,
  map_values(attributes) AS attr_values
FROM events;
```

DataFusion’s map functions include `element_at`, `map`, `make_map`, `map_entries`, `map_extract`, `map_keys`, and `map_values`; `element_at` is documented as an alias of `map_extract`. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/scalar_functions.html))

Agent rule:

```text id="tngsxr"
If missing key vs NULL value matters:
  add explicit has_key-like derivation where available
  or normalize map to struct with presence flags
  or unnest map entries and test key presence explicitly
```

---

## S12.6 Nested field access and introspection

### S12.6.1 Struct field access

```sql id="n0ga0y"
SELECT
  assay['api_gravity'] AS assay_api_gravity,
  assay['sulfur_wt_pct'] AS assay_sulfur_wt_pct
FROM assays;
```

DataFusion documents `get_field` and bracket syntax for extracting fields from structs and maps; nested access such as `my_struct['a']['b']` is optimized into `get_field(my_struct, 'a', 'b')`. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/scalar_functions.html))

### S12.6.2 Type audit

```sql id="w53zv1"
SELECT
  arrow_typeof(assay) AS assay_type,
  arrow_typeof(assay['api_gravity']) AS api_gravity_type,
  arrow_typeof(tags) AS tags_type,
  arrow_typeof(tags[1]) AS first_tag_type,
  arrow_typeof(attributes) AS attributes_type,
  arrow_typeof(element_at(attributes, 'source')) AS source_value_type
FROM streams
LIMIT 1;
```

### S12.6.3 Field metadata audit

```sql id="b8w5r9"
SELECT
  arrow_field(assay['api_gravity']) AS api_field,
  arrow_metadata(assay['api_gravity']) AS api_metadata
FROM assays
LIMIT 1;
```

`arrow_typeof`, `arrow_field`, and `arrow_metadata` are DataFusion SQL functions for expression-level type/field/metadata introspection. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/scalar_functions.html))

---

## S12.7 `unnest` schema behavior

### S12.7.1 Array/list unnest

```sql id="w3ea21"
SELECT
  stream_id,
  unnest(tags) AS tag
FROM streams;
```

DataFusion documents `unnest` as expanding an array or map into rows, and examples show `unnest(make_array(...))` producing one output row per element. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/special_functions.html))

Schema effect:

```text id="9tnngy"
input:
  stream_id: Utf8
  tags: List<Utf8>

output:
  stream_id: Utf8
  tag: Utf8

row count:
  increases/decreases based on list cardinality/null/empty semantics
```

### S12.7.2 Struct unnest

```sql id="16zgzb"
SELECT
  unnest(assay)
FROM assays;
```

DataFusion documents `unnest(struct)` separately from list/map `unnest`; struct unnest flattens fields into columns rather than expanding rows. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/special_functions.html))

Stable alternative:

```sql id="z3ruo8"
SELECT
  crude_id,
  assay['api_gravity'] AS assay_api_gravity,
  assay['sulfur_wt_pct'] AS assay_sulfur_wt_pct
FROM assays;
```

Agent rules:

```text id="6v6mdo"
Array/list/map unnest changes row cardinality.
Struct unnest changes column shape.
Always alias scalar unnest output.
Prefer explicit nested field extraction for stable schemas.
```

---

## S12.8 Nested evolution

### S12.8.1 Add struct field

Old:

```text id="y1b90w"
assay: Struct<
  api_gravity: Float64?,
  sulfur_wt_pct: Float64?
>
```

New:

```text id="na9l1s"
assay: Struct<
  api_gravity: Float64?,
  sulfur_wt_pct: Float64?,
  nitrogen_wt_pct: Float64?
>
```

Compatibility:

```text id="uaef07"
Backward compatible:
  yes if readers access explicit existing fields

Forward compatible:
  yes if old data can synthesize nitrogen_wt_pct = NULL

Physical exact:
  no

Nested contract:
  MINOR if nullable field and no consumer breaks
```

Migration SQL:

```sql id="y1xr04"
SELECT
  crude_id,
  named_struct(
    'api_gravity', assay['api_gravity'],
    'sulfur_wt_pct', assay['sulfur_wt_pct'],
    'nitrogen_wt_pct', CAST(NULL AS DOUBLE)
  ) AS assay
FROM assays_v1;
```

---

### S12.8.2 Reorder struct field

DataFusion uses name-based field mapping when coercing struct types across operations and explains that this differs from positional mapping; the docs cover struct coercion in arrays, unions, joins, and null handling. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/struct_coercion.html))

Old:

```text id="ehec2k"
Struct<a:Int64, b:Utf8>
```

New:

```text id="kt6w7z"
Struct<b:Utf8, a:Int64>
```

Policy:

```text id="tv6i92"
Logical compatibility:
  may be allowed through name-based struct coercion

Physical/output compatibility:
  not exact if field order changes

User-facing contract:
  treat as breaking unless contract says struct fields are name-based only
```

Agent rule:

```text id="wmrs3o"
Name-based struct coercion is a logical convenience, not permission to silently change physical output contracts.
```

---

### S12.8.3 Rename struct field

Old:

```text id="rpeumc"
assay['api']
```

New:

```text id="jv22bz"
assay['api_gravity']
```

Classification:

```text id="y4p0xh"
Breaking by nested path.
Requires compatibility projection.
```

Compatibility view:

```sql id="exrl4k"
SELECT
  crude_id,
  named_struct(
    'api', assay['api_gravity'],
    'sulfur_wt_pct', assay['sulfur_wt_pct']
  ) AS assay
FROM assays_v2;
```

Agent rule:

```text id="u9o12x"
Nested rename = drop old path + add new path.
Require explicit path mapping:
  assay.api → assay.api_gravity
```

---

### S12.8.4 List element type widening

Old:

```text id="unwy73"
embedding: List<Float32>
```

New:

```text id="qjqtc5"
embedding: List<Float64>
```

Compatibility:

```text id="yqq6cs"
logical:
  possible if element widening is allowed

physical:
  not exact

UDF/vector functions:
  may require exact Float32 or Float64

storage/API:
  breaking unless contract allows type widening
```

Migration:

```text id="egb6m8"
DataFusion 54 supports SQL lambda syntax and higher-order functions natively:
  array_transform(embedding, x -> CAST(x AS DOUBLE))

Alternatives when a provider-side contract is preferred:
  provider-side conversion
  custom UDF
  rewrite data externally
```

Lambda syntax (`x -> expr`) and the higher-order functions `array_transform`, `array_filter`, and `array_any_match` are first-class in DataFusion 54; the deep dive is in `datafusion_calculations_rust.md`.

Agent rule:

```text id="s1lxlx"
List compatibility is recursive:
  list nullability
  element nullability
  element type
  element metadata
  fixed-size length if applicable
```

---

### S12.8.5 Map key/value type change

Old:

```text id="8vofpx"
attributes: Map<Utf8, Utf8>
```

New:

```text id="nssjs2"
attributes: Map<Utf8, Struct<value: Utf8, quality: Utf8>>
```

Compatibility:

```text id="5am0jw"
Key type change:
  usually breaking
  affects lookup semantics

Value type change:
  breaking unless normalized by view

Map→Struct:
  only safe if key set is fixed and explicit
```

Migration:

```sql id="00tyb3"
SELECT
  event_id,
  named_struct(
    'source', element_at(attributes, 'source'),
    'device', element_at(attributes, 'device')
  ) AS attributes_struct
FROM events;
```

Agent rule:

```text id="7bfi2f"
Use Map for dynamic keys.
Use Struct for known keys.
Do not evolve dynamic maps into typed structs silently.
```

---

## S12.9 Partition schema

### S12.9.1 File schema vs table schema

DataFusion’s `TableSchema` docs define a partitioned table schema as two parts: file schema, which represents fields stored in data files, and partition columns, whose values are encoded in directory structure but not stored in the files themselves. The combined `table_schema()` includes both. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/datasource/table_schema/struct.TableSchema.html))

```text id="n3gpej"
file schema:
  stream_id: Utf8
  mass_flow_kg_h: Float64

partition columns:
  event_date: Date32
  site: Utf8

table schema:
  stream_id: Utf8
  mass_flow_kg_h: Float64
  event_date: Date32
  site: Utf8
```

Agent invariant:

```text id="hg8cf0"
Partition columns are schema elements even if not physically stored in Parquet/CSV/JSON files.
```

---

### S12.9.2 Hive partition discovery

Directory:

```text id="mhv1bu"
s3://lake/streams/event_date=2026-05-24/site=baytown/part-000.parquet
s3://lake/streams/event_date=2026-05-24/site=rotterdam/part-001.parquet
```

SQL:

```sql id="juzp2a"
CREATE EXTERNAL TABLE streams
STORED AS PARQUET
LOCATION 's3://lake/streams/';
```

DataFusion DDL docs state that Hive-compliant partition paths have their columns and values automatically detected and incorporated into schema/data; the config key `datafusion.execution.listing_table_factory_infer_partitions` defaults to true for `ListingTableFactory`, causing Hive partition columns to be inferred into the table schema. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/ddl.html)) ([datafusion.apache.org](https://datafusion.apache.org/user-guide/configs.html))

Validation:

```sql id="nax0oe"
DESCRIBE streams;

SELECT
  event_date,
  site,
  COUNT(*) AS n
FROM streams
GROUP BY event_date, site
ORDER BY event_date, site;
```

---

### S12.9.3 Partition-column type inference

Partition paths are strings on disk/object store; table schema must decide the logical type.

```text id="yu8m4t"
path value:
  event_date=2026-05-24

possible type:
  Utf8
  Date32

path value:
  year=2026

possible type:
  Utf8
  Int32
```

Policy:

```text id="4kwx0c"
Do not assume inferred partition type matches contract.
Supply explicit partition schema when available.
Audit DESCRIBE and arrow_typeof(partition_col).
Reject inconsistent partition values:
  event_date=unknown
  year=twenty_twenty_six
```

Test:

```sql id="n9862a"
SELECT
  arrow_typeof(event_date) AS event_date_type,
  arrow_typeof(site) AS site_type
FROM streams
LIMIT 1;
```

---

### S12.9.4 Partition/file column conflict

Bad dataset:

```text id="idjikz"
s3://lake/streams/site=baytown/part.parquet

file payload columns:
  stream_id
  site
  mass_flow_kg_h
```

Conflict modes:

```text id="g99bxs"
same name, same value:
  redundant but maybe acceptable

same name, different value:
  data correctness failure

same name, different type:
  schema conflict

same name, partition should override:
  explicit policy required
```

Agent rule:

```text id="dqz4g0"
Do not allow partition/file column name collisions without explicit resolution policy.
Preferred:
  remove partition columns from file payload
  or rename one side
  or enforce equality validation
```

Validation query:

```sql id="meqd0p"
-- If both physical and partition versions are exposed under different names:
SELECT *
FROM streams
WHERE site_from_path <> site_from_file;
```

---

### S12.9.5 Write-time `PARTITIONED BY`

```sql id="i6nz8o"
COPY streams
TO 's3://lake/streams_out/'
STORED AS PARQUET
PARTITIONED BY (event_date, site);
```

DataFusion’s DML docs state that `PARTITIONED BY` writes output files into Hive-style directories and that columns used in `PARTITIONED BY` are removed from the output format by default; to keep them in the file payload, use `execution.keep_partition_by_columns true`. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/dml.html))

Keep partition columns:

```sql id="f1q99f"
COPY streams
TO 's3://lake/streams_out/'
STORED AS PARQUET
PARTITIONED BY (event_date, site)
OPTIONS (
  'execution.keep_partition_by_columns' 'true'
);
```

Agent rules:

```text id="w2gx0f"
Partitioned writes create path-derived schema elements.
Default write removes partition columns from file payload.
Read table schema should re-add partition columns from paths.
Keep partition columns in payload only when downstream tools require them.
```

---

### S12.9.6 Partition evolution

Old:

```text id="z9ur41"
streams/event_date=2026-05-24/site=baytown/file.parquet
```

New:

```text id="6z41fn"
streams_v2/event_date=2026-05-24/site=baytown/unit_id=CDU/file.parquet
```

Compatibility:

```text id="81hfk5"
partition layout change = table schema change
partition pruning behavior changes
mixed old/new layout under one root is high risk
new root strongly preferred
```

Migration view:

```sql id="dx33bf"
CREATE OR REPLACE VIEW streams_all AS
SELECT
  event_date,
  site,
  CAST(NULL AS VARCHAR) AS unit_id,
  stream_id,
  mass_flow_kg_h
FROM streams_v1

UNION ALL

SELECT
  event_date,
  site,
  unit_id,
  stream_id,
  mass_flow_kg_h
FROM streams_v2;
```

Agent rule:

```text id="udrz97"
Partition evolution should create a new table/root unless a custom provider explicitly handles mixed layouts.
```

---

## S12.10 Virtual columns

### S12.10.1 Definition

```text id="ah9jbq"
Virtual column:
  appears in table/provider schema
  is not necessarily stored in backend/file payload
  is synthesized from path, context, backend metadata, or generation logic
```

Categories:

```text id="0i1ro4"
path-derived:
  source_file_path
  source_partition
  event_date from Hive path

tenant/security:
  tenant_id
  workspace_id

provenance:
  source_system
  source_object
  ingestion_batch_id
  row_source_uri

time:
  ingestion_ts
  extraction_ts

generated:
  mass_flow_t_d = mass_flow_kg_h / 1000 * 24
  normalized_product_code

diagnostic:
  _row_number
  _backend_shard
  _schema_snapshot_id
```

### S12.10.2 Virtual column contract

```rust id="be4e90"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VirtualColumnContract {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub visibility: VirtualColumnVisibility,
    pub generation: VirtualColumnGeneration,
    pub deterministic: bool,
    pub stable_across_reads: bool,
    pub pushdown_eligible: bool,
    pub materialized_in_sink: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VirtualColumnVisibility {
    Public,
    Diagnostic,
    HiddenPolicy,
    InternalOnly,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum VirtualColumnGeneration {
    HivePathSegment { key: String },
    SourceFilePath,
    TenantContext,
    IngestionTimestamp,
    Constant { value_sql: String },
    ExpressionSql { expr_sql: String },
    BackendMetadata { key: String },
}
```

### S12.10.3 Provider implementation rule

```text id="n6ook5"
If virtual column is visible in schema():
  projection index may request it
  provider must synthesize Arrow array in correct output position
  filter on virtual column must be supported, rejected, or evaluated residual-side
```

Projection synthesis:

```rust id="kffuad"
fn synthesize_virtual_column(
    contract: &VirtualColumnContract,
    rows: usize,
) -> datafusion::common::Result<datafusion::arrow::array::ArrayRef> {
    match &contract.generation {
        VirtualColumnGeneration::Constant { value_sql } => {
            todo!("parse/compile constant and repeat rows")
        }
        VirtualColumnGeneration::SourceFilePath => {
            todo!("repeat source path per row or per file fragment")
        }
        VirtualColumnGeneration::TenantContext => {
            todo!("repeat tenant id")
        }
        _ => todo!(),
    }
}
```

---

### S12.10.4 Tenant columns

Policy:

```text id="6us5t2"
tenant_id visible:
  user can group/filter by tenant if authorized
  may leak multi-tenant structure

tenant_id hidden:
  provider injects tenant predicate
  not visible in information_schema
  not projectable by user

tenant_id internal:
  never appears in DataFusion schema
  only backend request/policy uses it
```

Composite key rule:

```text id="u6vrm5"
If uniqueness is tenant-scoped:
  primary-like key = (tenant_id, natural_key)
not:
  natural_key alone
```

---

### S12.10.5 Row provenance columns

Examples:

```text id="3gopdf"
source_file_path: Utf8
source_row_number: UInt64
ingestion_batch_id: Utf8
schema_snapshot_id: Utf8
```

Use cases:

```text id="zl8glv"
debugging bad rows
audit lineage
rejected-row tables
roundtrip provenance
data quality diagnostics
```

Policy:

```text id="shwztt"
Expose provenance columns in diagnostics schema.
Hide in semantic/public schema unless product requires.
Do not include signed URLs or secrets in source_file_path.
```

---

### S12.10.6 Ingestion timestamp

```rust id="3h3m64"
let ingestion_ts = VirtualColumnContract {
    name: "ingestion_ts".to_string(),
    data_type: "Timestamp(Nanosecond, UTC)".to_string(),
    nullable: false,
    visibility: VirtualColumnVisibility::Public,
    generation: VirtualColumnGeneration::IngestionTimestamp,
    deterministic: false,
    stable_across_reads: false,
    pushdown_eligible: false,
    materialized_in_sink: true,
};
```

Policy:

```text id="32owu0"
ingestion_ts generated at write/ingest time is stable if materialized.
ingestion_ts generated at read time is non-deterministic and should not be used as persisted fact.
```

---

## S12.11 TableSchema, partition columns, and provider adapters

### S12.11.1 DataFusion `TableSchema` concept

`TableSchema` represents the overall schema for partitioned data sources and distinguishes file schema from partition columns. It provides accessors for file schema, table schema, partition fields, and projections over file-only or combined table schema. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/datasource/table_schema/struct.TableSchema.html))

Conceptual use:

```text id="oy2rl2"
file_schema:
  stream_id
  mass_flow_kg_h

partition_fields:
  event_date
  site

table_schema:
  stream_id
  mass_flow_kg_h
  event_date
  site
```

### S12.11.2 Provider schema construction

```rust id="vpx4pn"
#[derive(Debug, Clone)]
pub struct PartitionedProviderSchema {
    pub file_schema: datafusion::arrow::datatypes::SchemaRef,
    pub partition_fields: Vec<datafusion::arrow::datatypes::Field>,
    pub virtual_fields: Vec<datafusion::arrow::datatypes::Field>,
    pub table_schema: datafusion::arrow::datatypes::SchemaRef,
}
```

Agent rule:

```text id="48urv2"
Do not model partition columns as ordinary file fields.
Do not model virtual columns as if backend stores them unless materialized.
```

---

## S12.12 Nested/partition/virtual compatibility report

```rust id="y80jov"
#[derive(Debug, Clone, serde::Serialize)]
pub struct NonFlatSchemaCompatibilityReport {
    pub table_ref: String,
    pub old_fingerprint: String,
    pub new_fingerprint: String,
    pub nested_findings: Vec<NestedFinding>,
    pub partition_findings: Vec<PartitionFinding>,
    pub virtual_column_findings: Vec<VirtualColumnFinding>,
    pub decision: CompatibilityDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NestedFinding {
    pub path: String,
    pub kind: NestedFindingKind,
    pub old_type: Option<String>,
    pub new_type: Option<String>,
    pub decision: CompatibilityDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum NestedFindingKind {
    NullStructVsNullFieldPolicyChange,
    AddStructField,
    DropStructField,
    RenameStructField,
    ReorderStructField,
    ListElementTypeChange,
    ListElementNullabilityChange,
    MapKeyTypeChange,
    MapValueTypeChange,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PartitionFinding {
    pub partition_column: String,
    pub kind: PartitionFindingKind,
    pub decision: CompatibilityDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum PartitionFindingKind {
    AddedPartitionColumn,
    DroppedPartitionColumn,
    ReorderedPartitionColumn,
    PartitionTypeChange,
    PartitionFileColumnConflict,
    MixedPartitionLayout,
}
```

---

## S12.13 Agent tests

### S12.13.1 `arrow_typeof(payload['x'])`

```sql id="ewuw5f"
SELECT
  arrow_typeof(payload['event_type']) AS event_type_type,
  arrow_typeof(payload['user']['id']) AS user_id_type,
  arrow_typeof(tags) AS tags_type,
  arrow_typeof(tags[1]) AS tag_type
FROM events
LIMIT 1;
```

### S12.13.2 `DESCRIBE`

```sql id="tljf0t"
DESCRIBE streams;
```

Expected table schema includes visible partition columns and visible virtual columns.

### S12.13.3 Partition pruning explain

```sql id="tm9m9g"
EXPLAIN
SELECT
  stream_id,
  mass_flow_kg_h
FROM streams
WHERE event_date = DATE '2026-05-24'
  AND site = 'baytown';
```

DataFusion supports partition columns in table schema for Hive-style partitioned data, and partition filters can enable pruning; verify pruning using `EXPLAIN` and scan metrics under pinned version/config. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/datasource/table_schema/struct.TableSchema.html))

### S12.13.4 Mixed partition schemas

Fixture:

```text id="xg3sji"
streams/
  event_date=2026-05-24/site=baytown/part-000.parquet
  event_date=2026-05-24/site=rotterdam/part-001.parquet
  event_date=unknown/site=baytown/part-002.parquet
```

Test:

```sql id="ls4dpl"
SELECT
  event_date,
  site,
  COUNT(*) AS n
FROM streams
GROUP BY event_date, site
ORDER BY event_date, site;
```

Expected:

```text id="f1a8ih"
if event_date is Date32:
  event_date=unknown should reject or quarantine

if event_date is Utf8:
  accepted but semantic date contract not satisfied
```

### S12.13.5 Nested Parquet round-trip

Write:

```sql id="v4vs5z"
COPY (
  SELECT
    crude_id,
    named_struct(
      'api_gravity', api_gravity,
      'sulfur_wt_pct', sulfur_wt_pct
    ) AS assay,
    make_array('raw', 'validated') AS tags
  FROM assays
)
TO 's3://lake/test/nested_roundtrip/'
STORED AS PARQUET;
```

Read:

```sql id="92tocw"
CREATE EXTERNAL TABLE nested_roundtrip
STORED AS PARQUET
LOCATION 's3://lake/test/nested_roundtrip/';

SELECT
  arrow_typeof(assay) AS assay_type,
  arrow_typeof(assay['api_gravity']) AS api_type,
  arrow_typeof(tags) AS tags_type
FROM nested_roundtrip
LIMIT 1;
```

DataFusion’s feature page lists read/write support for nested `ARRAY`/`LIST` and `STRUCT` types, including field access syntax and nested Parquet support. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/features.html))

---

## S12.14 Runtime and provider validation

### S12.14.1 Nested batch validation

```rust id="3t7eja"
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::{DataFusionError, Result};

pub fn validate_nested_batch(expected: &SchemaRef, batch: &RecordBatch) -> Result<()> {
    if expected.fields() != batch.schema().fields() {
        return Err(DataFusionError::Execution(format!(
            "nested batch schema mismatch: expected={:?}, actual={:?}",
            expected.fields(),
            batch.schema().fields()
        )));
    }

    Ok(())
}
```

`RecordBatch::try_new` validates field count, column type equality, and equal column lengths; nested arrays must still match their declared nested `DataType`. ([docs.rs](https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html))

### S12.14.2 Partition/virtual provider validation

```text id="v7ctcw"
For every provider batch:
  file payload arrays match file schema portion
  partition columns synthesized with correct row count
  virtual columns synthesized with correct row count
  final arrays match table schema / projection schema
```

Projection rule:

```text id="e6jmn6"
projection can request:
  physical file field
  partition field
  virtual field
  nested top-level field

provider must emit requested fields in projection order.
```

---

## S12.15 Deployment advisory

```text id="dd6zbx"
Nested schemas:
  use Struct for known keys
  use Map for dynamic keys
  use List for repeated homogeneous values
  use List<Struct> for repeated records
  use FixedSizeList for fixed-dimensional vectors
  test null struct vs null field semantics
  test null list vs empty list vs null element
```

```text id="0avli6"
Partition schemas:
  treat partition columns as table schema fields
  separate file schema from table schema
  avoid partition/file column name conflicts
  prefer explicit partition schema where possible
  validate partition type inference
  create new table root for partition evolution
  test keep/drop partition columns in write payload
```

```text id="xlu1zh"
Virtual columns:
  document generation rule
  mark visibility
  define determinism/stability
  synthesize arrays in provider output
  keep hidden policy columns out of information_schema
  materialize virtual columns before sink if downstream requires them
```

```text id="ikhkva"
Sinks:
  Parquet: supports nested data and partitioned output
  Arrow IPC: high-fidelity nested schema/metadata
  CSV: flatten or reject nested/virtual schema as needed
  JSON: explicit output schema/manifest recommended
```

---

## S12.16 Anti-pattern inventory

```text id="zdkvqq"
Non-flat schema anti-patterns:
  treating NULL list and empty list as equivalent
  treating NULL struct and NULL child field as equivalent
  ignoring missing map key vs NULL map value
  comparing only top-level Struct type, not nested field paths
  relying on struct field position despite name-based logical coercion
  renaming nested fields without compatibility mapping
  changing list element type without recursive compatibility check
  using Map for fixed schema hot fields
  partitioning by high-cardinality UUID/session IDs
  mixing partition layouts under one directory root
  allowing partition/file column name conflict
  assuming partition columns are physically stored in files
  forgetting PARTITIONED BY drops partition columns from output payload by default
  exposing tenant_id virtual column accidentally
  generating ingestion_ts at read time and treating it as persisted fact
  exposing signed source_file_path values
  claiming nested projection pushdown without implementation
  unnesting arrays before filtering
  using struct unnest in stable API without aliases
```

---

## S12.17 Agent checklist

```text id="0llsa8"
[ ] Classify field:
    flat / struct / list / map / partition / virtual.

[ ] For nested:
    record parent nullability and child nullability separately.
    test null struct vs null field.
    test null list vs empty list vs null element.
    test missing map key vs null map value if relevant.

[ ] For nested evolution:
    diff by field path, not top-level type only.
    add nullable struct fields only with compatibility policy.
    treat rename/drop as breaking.
    treat list element type changes recursively.
    treat map key changes as breaking.

[ ] For partition schema:
    distinguish file schema from table schema.
    validate Hive partition discovery.
    audit partition type inference.
    detect partition/file column conflicts.
    use new root for partition evolution unless custom provider handles mixed layouts.

[ ] For write-time partitioning:
    remember PARTITIONED BY removes partition columns from file payload by default.
    set execution.keep_partition_by_columns only if needed.
    validate read-back table schema.

[ ] For virtual columns:
    define generation rule.
    define visibility.
    define determinism.
    synthesize arrays with correct row count.
    include/exclude from information_schema according to policy.

[ ] Tests:
    DESCRIBE.
    arrow_typeof nested paths.
    EXPLAIN partition pruning.
    mixed partition fixture.
    nested Parquet round-trip.
    empty/null edge cases.
```

## S12.18 Minimal non-flat schema contract shape

```rust id="e06nqu"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NonFlatSchemaContract {
    pub table_ref: String,
    pub file_fields: Vec<FieldContract>,
    pub nested_fields: Vec<NestedFieldContract>,
    pub partition_columns: Vec<PartitionColumnContract>,
    pub virtual_columns: Vec<VirtualColumnContract>,
    pub compatibility_policy: NonFlatCompatibilityPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NestedFieldContract {
    pub path: Vec<String>,
    pub data_type: String,
    pub nullable: bool,
    pub item_nullable: Option<bool>,
    pub semantic_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PartitionColumnContract {
    pub name: String,
    pub data_type: String,
    pub source: String,
    pub nullable: bool,
    pub cardinality_policy: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VirtualColumnContract {
    pub name: String,
    pub data_type: String,
    pub generation: String,
    pub visibility: String,
    pub deterministic: bool,
    pub materialized_in_sink: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NonFlatCompatibilityPolicy {
    pub allow_add_nullable_struct_field: bool,
    pub allow_struct_field_reorder_by_name: bool,
    pub allow_list_element_widening: bool,
    pub reject_partition_layout_mixing: bool,
    pub require_new_root_for_partition_evolution: bool,
    pub hide_internal_virtual_columns: bool,
}
```

Core operating rule: **nested fields, partition columns, and virtual columns all participate in the table schema contract; they differ only in where values come from and how compatibility/enforcement must be implemented.**


# DataFusion Advanced — S13) View, CTAS, and derived-table schema stability

## S13.0 Objective

Make derived catalog objects **schema-stable, versioned, auditable, and migration-safe**:

```text
base table / external table / provider table
  → SELECT projection
  → derived logical schema
  → CREATE VIEW / CTAS / compatibility view / materialized export
  → catalog object
  → downstream query/API/sink contract
```

DataFusion DDL supports `CREATE VIEW`, `DROP VIEW`, `CREATE TABLE AS SELECT`, `CREATE TABLE AS VALUES`, `CREATE EXTERNAL TABLE`, `DESCRIBE`, and related catalog mutations; DataFusion’s catalog guide also states that DDL uses the catalog API, while DML behavior is tied to `TableProvider` support. ([Apache DataFusion][1])

Core invariant:

```text
A view or CTAS schema is not “the source table schema.”
It is the SELECT output schema at creation/planning time.
Therefore:
  aliases define names
  casts define types
  expressions define nullability
  qualifiers are not a durable output contract
  SELECT * is schema drift by design
```

---

## S13.1 Derived object taxonomy

```text
DerivedCatalogObject
  ├─ View
  │   ├─ logical / virtual table
  │   ├─ schema derived from SELECT output
  │   ├─ depends on underlying table/view/function names
  │   └─ recalculated/planned when queried, depending provider/catalog implementation
  │
  ├─ CTAS table
  │   ├─ CREATE TABLE ... AS SELECT / VALUES
  │   ├─ schema derived from query output
  │   ├─ default DataFusion behavior may be in-memory
  │   └─ custom catalog/provider may make durable
  │
  ├─ Compatibility view
  │   ├─ stable old contract over new table
  │   ├─ aliases renamed fields back
  │   ├─ casts widened/narrowed under policy
  │   └─ fills defaults / NULLs for evolved fields
  │
  ├─ Semantic view
  │   ├─ domain-facing schema
  │   ├─ normalized field names
  │   ├─ joins / computed columns / masking
  │   └─ API-like contract
  │
  └─ Materialized derived dataset
      ├─ CTAS-like table
      ├─ COPY query TO Parquet/CSV/JSON/Arrow
      ├─ DataFrame write_*
      └─ sink-owned output schema
```

DataFusion’s custom-provider guide identifies `ViewTable` as a table provider that wraps a logical plan and represents a SQL view, making it useful when a provider is best expressed as a transformation of other tables. ([Apache DataFusion][2])

---

## S13.2 View schema derivation

### S13.2.1 Rule

```text
CREATE VIEW v AS <query>

view_schema =
  query.output_schema =
    SELECT item 1 field
    SELECT item 2 field
    ...
```

View schema is controlled by the query’s projection list, not by the base table directly. `LogicalPlan` nodes transform input relations into output relations with potentially different schemas, and DataFrames represent logical sets of rows with named columns; this is the correct mental model for views and CTAS. ([Docs.rs][3])

### S13.2.2 Stable view pattern

```sql
CREATE OR REPLACE VIEW semantic.streams_v1 AS
SELECT
  s.stream_id AS stream_id,
  s.case_id AS case_id,
  s.unit_id AS unit_id,
  CAST(s.mass_flow_kg_h AS DOUBLE) AS mass_flow_kg_h,
  CAST(s.sulfur_wt_pct AS DOUBLE) AS sulfur_wt_pct,
  date_trunc('hour', s.event_ts) AS event_hour
FROM curated.streams AS s;
```

### S13.2.3 Unstable view pattern

```sql
CREATE VIEW semantic.streams_bad AS
SELECT *
FROM curated.streams;
```

Failure modes:

```text
source adds column      → view schema changes
source drops column     → view query may fail
source reorders fields  → output order may drift
source expression name  → derived display name can drift
source duplicate names  → ambiguity / duplicate output fields
source type changes     → downstream view type changes
```

DataFusion’s output-field-name specification exists because output `RecordBatch` field names are generated from SQL/DataFrame queries and are observable behavior; stable views should not rely on implicit expression naming. ([Apache DataFusion][4])

---

## S13.3 View output names

### S13.3.1 Alias every select item

Rule:

```text
Every stable view SELECT item should have an explicit alias.
This includes direct column references if the source name is not already canonical.
```

Good:

```sql
CREATE VIEW semantic.assays_v1 AS
SELECT
  crude_id AS crude_id,
  "API Gravity" AS api_gravity,
  "Sulfur wt%" AS sulfur_wt_pct,
  CAST("TAN mg KOH/g" AS DOUBLE) AS tan_mg_koh_g
FROM raw.assays_stage;
```

Bad:

```sql
CREATE VIEW semantic.assays_bad AS
SELECT
  "API Gravity",
  "Sulfur wt%",
  "TAN mg KOH/g"
FROM raw.assays_stage;
```

DataFusion SQL lowercases unquoted query column names while inferred schemas preserve original casing; capitalized physical fields require double quotes, so stable views should immediately alias quoted source identifiers to canonical names. ([Apache DataFusion][5])

### S13.3.2 Computed field names

Bad:

```sql
CREATE VIEW semantic.streams_bad AS
SELECT
  mass_flow_kg_h / 1000.0 * 24.0
FROM curated.streams;
```

Good:

```sql
CREATE VIEW semantic.streams_v1 AS
SELECT
  mass_flow_kg_h / 1000.0 * 24.0 AS mass_flow_t_d
FROM curated.streams;
```

### S13.3.3 Aggregate field names

Bad:

```sql
CREATE VIEW semantic.unit_balance_bad AS
SELECT
  unit_id,
  SUM(mass_flow_kg_h)
FROM curated.streams
GROUP BY unit_id;
```

Good:

```sql
CREATE VIEW semantic.unit_balance_v1 AS
SELECT
  unit_id AS unit_id,
  SUM(mass_flow_kg_h) AS total_mass_flow_kg_h
FROM curated.streams
GROUP BY unit_id;
```

---

## S13.4 View output types

### S13.4.1 Explicit casts

```sql
CREATE OR REPLACE VIEW semantic.prices_v1 AS
SELECT
  node_id AS node_id,
  product_code AS product_code,
  arrow_cast(price_raw, 'Decimal128(20, 4)') AS price_usd_bbl,
  arrow_cast(effective_ts_raw, 'Timestamp(ns, "UTC")') AS effective_ts
FROM staging.prices_stage;
```

Use explicit casts for:

```text
financial values
timestamp unit/timezone
string representation
integer width
decimal precision/scale
dirty CSV/JSON staged text
union branch alignment
compatibility views
```

DataFusion’s SQL type mapping page documents SQL-to-Arrow type mapping and Arrow-specific `arrow_typeof` / `arrow_cast`, including decimal precision behavior and timestamp precision strings; use those to lock view output types under test. ([Apache DataFusion][5])

### S13.4.2 Type audit

```sql
SELECT
  arrow_typeof(price_usd_bbl) AS price_type,
  arrow_typeof(effective_ts) AS effective_ts_type
FROM semantic.prices_v1
LIMIT 1;
```

### S13.4.3 View type-stability rule

```text
Do not let base-table inference decide semantic-view output type.
Every contract-sensitive output should be CAST / arrow_cast / typed expression.
```

---

## S13.5 View nullability

### S13.5.1 Nullability sources

```text
Direct column:
  inherits source nullability

CAST(column AS type):
  generally inherits expression nullability

COALESCE(nullable_col, non_null_literal):
  can produce non-null logical output if inference proves it

CASE:
  nullable if any arm can be NULL or condition permits no ELSE

Aggregate:
  function-specific; SUM/AVG/MIN/MAX can be nullable, COUNT normally non-null

Outer join:
  non-preserved side fields can become nullable
```

DataFusion table-constraint docs state that nullability is the only schema property DataFusion enforces during execution; returning nulls for fields marked non-nullable causes runtime errors, but ingestion itself is not checked. Use this as a runtime contract, not an ingestion guarantee. ([datafusion.apache.org](https://datafusion.apache.org/library-user-guide/table-constraints.html))

### S13.5.2 Explicit hardening

```sql
CREATE OR REPLACE VIEW semantic.streams_non_null_v1 AS
SELECT
  stream_id AS stream_id,
  unit_id AS unit_id,
  mass_flow_kg_h AS mass_flow_kg_h
FROM curated.streams
WHERE stream_id IS NOT NULL
  AND unit_id IS NOT NULL
  AND mass_flow_kg_h IS NOT NULL;
```

### S13.5.3 Default fill

```sql
CREATE OR REPLACE VIEW semantic.streams_v2 AS
SELECT
  stream_id AS stream_id,
  COALESCE(source_system, 'smartref') AS source_system,
  mass_flow_kg_h AS mass_flow_kg_h
FROM curated.streams;
```

Agent rule:

```text
Never rely on desired nullability.
Prove it with:
  df.schema()
  DESCRIBE
  null-count validation query
  runtime test
```

---

## S13.6 View qualifiers

### S13.6.1 Qualifier lifetime

```text
Inside view definition:
  qualifiers disambiguate sources:
    s.stream_id
    u.unit_id

View output:
  qualifiers should not be the consumer contract.
  output fields should be unique canonical names:
    stream_id
    stream_unit_id
    unit_id
    unit_name
```

### S13.6.2 Join-derived stable view

```sql
CREATE OR REPLACE VIEW semantic.streams_with_units_v1 AS
SELECT
  s.stream_id AS stream_id,
  s.unit_id AS stream_unit_id,
  u.unit_id AS unit_dim_id,
  u.unit_name AS unit_name,
  s.mass_flow_kg_h AS mass_flow_kg_h
FROM curated.streams AS s
JOIN curated.units AS u
  ON s.unit_id = u.unit_id;
```

Do not expose duplicate unqualified fields:

```sql
CREATE VIEW semantic.bad_join_view AS
SELECT *
FROM curated.streams AS s
JOIN curated.units AS u
  ON s.unit_id = u.unit_id;
```

`DFSchema` supports qualified fields for logical planning, but conversion to Arrow/output schema does not make qualifiers a durable catalog/API output contract; final view outputs need explicit aliases. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html))

---

## S13.7 Dependency tracking

### S13.7.1 View dependency model

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewContract {
    pub view_ref: String,
    pub version: String,
    pub sql: String,
    pub output_schema_fingerprint: String,
    pub dependencies: Vec<ViewDependency>,
    pub compatibility_policy: ViewCompatibilityPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ViewDependency {
    pub object_ref: String,
    pub required_columns: Vec<String>,
    pub required_schema_fingerprint: Option<String>,
    pub dependency_kind: DependencyKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DependencyKind {
    Table,
    View,
    Function,
    Udf,
    Catalog,
}
```

### S13.7.2 Extracted dependency example

```json
{
  "view_ref": "semantic.streams_with_units_v1",
  "dependencies": [
    {
      "object_ref": "curated.streams",
      "required_columns": ["stream_id", "unit_id", "mass_flow_kg_h"],
      "dependency_kind": "Table"
    },
    {
      "object_ref": "curated.units",
      "required_columns": ["unit_id", "unit_name"],
      "dependency_kind": "Table"
    }
  ]
}
```

### S13.7.3 Agent dependency rules

```text
Track table dependencies.
Track required column dependencies.
Track function/UDF dependencies.
Track expected output schema fingerprint.
On source schema drift:
  re-plan view
  compare output schema
  run compatibility report
  decide accept/reject/migrate
```

DataFusion’s `LogicalPlan` trees can be inspected programmatically, and logical plans are schema-aware DAGs of relational operators and embedded expressions. ([Docs.rs][6])

---

## S13.8 Underlying table drift

### S13.8.1 Source column renamed

Old source:

```text
curated.assays.api
```

New source:

```text
curated.assays.api_gravity
```

Old view fails:

```sql
CREATE VIEW semantic.assays_v1 AS
SELECT
  api AS api_gravity
FROM curated.assays;
```

Compatibility fix:

```sql
CREATE OR REPLACE VIEW semantic.assays_v1_compat AS
SELECT
  api_gravity AS api_gravity
FROM curated.assays_v2;
```

Migration artifact:

```json
{
  "drift": "source_column_renamed",
  "old_column": "api",
  "new_column": "api_gravity",
  "affected_views": ["semantic.assays_v1"],
  "required_action": "rewrite view SQL or create source compatibility view"
}
```

### S13.8.2 Source type changed

Old:

```text
mass_flow_kg_h Float64
```

New:

```text
mass_flow_kg_h Utf8
```

Compatibility fix:

```sql
CREATE OR REPLACE VIEW semantic.streams_v1 AS
SELECT
  stream_id,
  arrow_cast(mass_flow_kg_h, 'Float64') AS mass_flow_kg_h
FROM curated.streams_v2;
```

Dirty-source fix:

```sql
CREATE OR REPLACE VIEW semantic.streams_accepted AS
SELECT
  stream_id,
  arrow_try_cast(mass_flow_kg_h, 'Float64') AS mass_flow_kg_h
FROM curated.streams_v2
WHERE arrow_try_cast(mass_flow_kg_h, 'Float64') IS NOT NULL;
```

### S13.8.3 Source nullability changed

Nullable source became nullable when view contract expects non-null:

```sql
SELECT COUNT(*) AS bad_rows
FROM curated.streams
WHERE stream_id IS NULL;
```

Hardening view:

```sql
CREATE OR REPLACE VIEW semantic.streams_v1 AS
SELECT
  stream_id,
  mass_flow_kg_h
FROM curated.streams
WHERE stream_id IS NOT NULL;
```

### S13.8.4 Source column dropped

Compatibility view:

```sql
CREATE OR REPLACE VIEW semantic.streams_v1_compat AS
SELECT
  stream_id,
  mass_flow_kg_h,
  CAST(NULL AS DOUBLE) AS sulfur_wt_pct
FROM curated.streams_v2;
```

Agent rule:

```text
Source dropped field can be:
  synthesized as NULL
  synthesized as default
  derived from other fields
  rejected as breaking
```

---

## S13.9 Stable view contracts

### S13.9.1 Stable view checklist

```text
[ ] fully qualified source tables
[ ] table aliases for all sources
[ ] explicit projection, no SELECT *
[ ] alias every select item
[ ] cast every contract-sensitive field
[ ] no duplicate output names
[ ] deterministic field order
[ ] no expression-derived output names
[ ] explicit join duplicate resolution
[ ] nullability hardening if required
[ ] schema fingerprint recorded
[ ] dependency list recorded
[ ] DESCRIBE snapshot stored
[ ] arrow_typeof test for critical fields
```

### S13.9.2 Versioned views

```sql
CREATE OR REPLACE VIEW semantic.streams_v1 AS
SELECT
  stream_id AS stream_id,
  unit_id AS unit_id,
  mass_flow_kg_h AS mass_flow_kg_h
FROM curated.streams;

CREATE OR REPLACE VIEW semantic.streams_v2 AS
SELECT
  stream_id AS stream_id,
  unit_id AS unit_id,
  mass_flow_kg_h AS mass_flow_kg_h,
  sulfur_wt_pct AS sulfur_wt_pct
FROM curated.streams;
```

Stable alias:

```sql
CREATE OR REPLACE VIEW semantic.streams_current AS
SELECT *
FROM semantic.streams_v2;
```

Better stable alias with explicit projection:

```sql
CREATE OR REPLACE VIEW semantic.streams_current AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  sulfur_wt_pct
FROM semantic.streams_v2;
```

### S13.9.3 Compatibility views

```sql
CREATE OR REPLACE VIEW semantic.streams_v1_compat AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM semantic.streams_v2;
```

Rename compatibility:

```sql
CREATE OR REPLACE VIEW semantic.assays_v1_compat AS
SELECT
  crude_id,
  api_gravity AS api,
  sulfur_wt_pct
FROM semantic.assays_v2;
```

Add default compatibility:

```sql
CREATE OR REPLACE VIEW semantic.streams_v2 AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  COALESCE(source_system, 'smartref') AS source_system
FROM semantic.streams_v1;
```

---

## S13.10 CTAS schema derivation

### S13.10.1 Rule

```text
CREATE TABLE new_table AS SELECT ...
  → output table schema = SELECT output schema
```

DataFusion DDL docs describe `CREATE TABLE ... AS SELECT` and `CREATE TABLE ... AS VALUES` as creating an in-memory table from a query or values list in the default behavior. ([Apache DataFusion][1])

### S13.10.2 Stable CTAS

```sql
CREATE TABLE curated.streams_v2 AS
SELECT
  stream_id AS stream_id,
  unit_id AS unit_id,
  CAST(mass_flow_raw AS DOUBLE) AS mass_flow_kg_h,
  arrow_cast(event_ts_raw, 'Timestamp(ns, "UTC")') AS event_ts
FROM staging.streams_stage
WHERE stream_id IS NOT NULL
  AND unit_id IS NOT NULL;
```

### S13.10.3 Unstable CTAS

```sql
CREATE TABLE curated.streams_bad AS
SELECT *
FROM staging.streams_stage;
```

CTAS failure modes:

```text
input source drift changes output
expression-derived names become table columns
unintended raw/staging fields become curated schema
dirty strings remain strings
computed columns lack semantic names
default provider may be in-memory only
```

---

## S13.11 CTAS default provider vs durable provider

### S13.11.1 Default behavior

DataFusion’s DDL docs state that `CREATE TABLE` creates an in-memory table from a query or values list; durable behavior depends on custom catalog/provider implementation. ([Apache DataFusion][1])

Operational implication:

```text
CTAS in default DataFusion:
  useful for session-local derived tables
  useful for tests and temp work
  not a durable lakehouse table unless custom catalog/provider implements persistence
```

### S13.11.2 Durable custom behavior

A durable custom catalog/provider may implement CTAS as:

```text
CTAS logical plan
  → execute query
  → write Parquet/Delta/custom storage
  → register table metadata
  → publish schema fingerprint
  → return count/status
```

Contract object:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CtasContract {
    pub table_ref: String,
    pub sql: String,
    pub output_schema_fingerprint: String,
    pub storage_location: Option<String>,
    pub durable: bool,
    pub provider_kind: String,
    pub created_at_ms: i64,
}
```

Agent rule:

```text
Never assume CTAS persists across sessions unless catalog/provider persistence semantics explicitly say so.
```

---

## S13.12 CTAS vs `COPY query TO`

Use CTAS when:

```text
need catalog table object
session-local or provider-managed table
subsequent SQL references table name
custom catalog controls lifecycle
```

Use `COPY query TO` / DataFrame `write_*` when:

```text
need explicit file/object-store output
need Parquet/CSV/JSON/Arrow export
need partitioned files
need writer options
need schema manifest beside data
```

DataFusion DML docs define `COPY { table_name | query } TO 'file_name' ...` for exporting a table or query to files, with `STORED AS` and `PARTITIONED BY` options. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/dml.html))

Example:

```sql
COPY (
  SELECT
    stream_id,
    unit_id,
    mass_flow_kg_h
  FROM semantic.streams_v1
)
TO 's3://lake/curated/streams_v1/'
STORED AS PARQUET
PARTITIONED BY (unit_id);
```

---

## S13.13 Derived table from DataFrame

```rust
use datafusion::dataframe::DataFrameWriteOptions;
use datafusion::prelude::*;

pub async fn write_stable_derived_table(
    ctx: &SessionContext,
) -> datafusion::error::Result<()> {
    let df = ctx
        .table("staging.streams_stage")
        .await?
        .select(vec![
            col("stream_id").alias("stream_id"),
            col("unit_id").alias("unit_id"),
            col("mass_flow_raw")
                .cast_to(&datafusion::arrow::datatypes::DataType::Float64, ctx.table("staging.streams_stage").await?.schema())?
                .alias("mass_flow_kg_h"),
        ])?;

    validate_derived_schema(df.schema())?;

    df.write_parquet(
        "s3://lake/curated/streams_v1/",
        DataFrameWriteOptions::new(),
        None,
    )
    .await?;

    Ok(())
}
```

DataFrame methods create logical plans until execution actions such as `collect`, `show`, `execute_stream`, or `write_*`; schema validation should happen before execution and then be read-after-write validated. ([Docs.rs][7])

---

## S13.14 Schema validation after view/CTAS creation

### S13.14.1 `DESCRIBE`

```sql
DESCRIBE semantic.streams_v1;
```

DataFusion DDL docs list `DESCRIBE` as a DDL/introspection command, and the SQL reference includes `DESCRIBE` alongside `CREATE VIEW`, `CREATE TABLE`, and `CREATE EXTERNAL TABLE`. ([Apache DataFusion][1])

### S13.14.2 `information_schema.columns`

```sql
SELECT
  table_catalog,
  table_schema,
  table_name,
  column_name,
  data_type,
  is_nullable
FROM information_schema.columns
WHERE table_schema = 'semantic'
  AND table_name = 'streams_v1'
ORDER BY ordinal_position;
```

### S13.14.3 `arrow_typeof`

```sql
SELECT
  arrow_typeof(stream_id) AS stream_id_type,
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM semantic.streams_v1
LIMIT 1;
```

### S13.14.4 Rust `df.schema()`

```rust
pub async fn validate_view_query_schema(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let schema = df.schema();

    validate_derived_schema(schema)?;

    Ok(())
}
```

---

## S13.15 Derived-schema fingerprinting

```rust
use datafusion::arrow::datatypes::Schema;

pub fn derived_schema_fingerprint(schema: &Schema) -> String {
    let mut canonical = String::new();

    for (idx, field) in schema.fields().iter().enumerate() {
        canonical.push_str(&format!(
            "{idx}|{}|{:?}|nullable={}\n",
            field.name(),
            field.data_type(),
            field.is_nullable()
        ));
    }

    // Replace with SHA-256 in production.
    format!("debug-len-{}", canonical.len())
}
```

View contract:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DerivedObjectSchemaSnapshot {
    pub object_ref: String,
    pub object_kind: DerivedObjectKind,
    pub version: String,
    pub sql_text: String,
    pub schema_fingerprint: String,
    pub fields: Vec<DerivedFieldSnapshot>,
    pub dependencies: Vec<ViewDependency>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum DerivedObjectKind {
    View,
    CtasTable,
    CompatibilityView,
    SemanticView,
    ExportQuery,
}
```

---

## S13.16 Drift handling matrix

| Drift                         | View impact                                   | CTAS impact                         | Safe response                           |
| ----------------------------- | --------------------------------------------- | ----------------------------------- | --------------------------------------- |
| source adds column            | `SELECT *` view changes; explicit view stable | CTAS re-run with `*` changes        | forbid `*`, explicit projection         |
| source drops column           | view fails if referenced                      | CTAS fails if referenced            | compatibility view or default/null fill |
| source renames column         | view fails                                    | CTAS fails                          | rename mapping + compatibility alias    |
| source type widens            | output type may drift                         | re-run table type drifts            | explicit cast to contract               |
| source type narrows           | possible failure/loss                         | persisted narrower type             | reject or validation+cast plan          |
| source nullability widens     | view nullable output may widen                | output nullability drift            | filter/coalesce/prove non-null          |
| source partition changes      | view over table may change schema             | CTAS may materialize changed fields | new root + compatibility view           |
| UDF return type changes       | view schema changes                           | CTAS schema changes                 | pin/test UDF contract                   |
| function availability changes | view planning fails                           | CTAS planning fails                 | dependency/version check                |

---

## S13.17 Drift diagnostics

### Source column renamed

```json
{
  "error_class": "derived_object_dependency_column_missing",
  "object_ref": "semantic.assays_v1",
  "dependency": "curated.assays",
  "missing_column": "api",
  "candidate_replacement": "api_gravity",
  "suggested_fix": "Rewrite view SQL: api_gravity AS api_gravity, or create curated.assays_v1_compat"
}
```

### Source type changed

```json
{
  "error_class": "derived_object_output_type_drift",
  "object_ref": "semantic.streams_v1",
  "field": "mass_flow_kg_h",
  "expected_type": "Float64",
  "actual_type": "Utf8",
  "suggested_fix": "Add arrow_cast(mass_flow_kg_h, 'Float64') AS mass_flow_kg_h"
}
```

### Nullability changed

```json
{
  "error_class": "derived_object_nullability_drift",
  "object_ref": "semantic.streams_v1",
  "field": "stream_id",
  "expected_nullable": false,
  "actual_nullable": true,
  "required_validation": "SELECT COUNT(*) FROM semantic.streams_v1 WHERE stream_id IS NULL"
}
```

### `SELECT *` drift

```json
{
  "error_class": "select_star_in_stable_view",
  "object_ref": "semantic.streams_current",
  "reason": "SELECT * makes output schema dependent on underlying table drift",
  "suggested_fix": "Replace with explicit projection and aliases"
}
```

---

## S13.18 Stable view linter

```rust
#[derive(Debug, Clone)]
pub struct DerivedSchemaPolicy {
    pub forbid_select_star: bool,
    pub require_aliases: bool,
    pub require_explicit_casts_for_sensitive_types: bool,
    pub require_unique_output_names: bool,
    pub require_lower_snake_case: bool,
}

impl Default for DerivedSchemaPolicy {
    fn default() -> Self {
        Self {
            forbid_select_star: true,
            require_aliases: true,
            require_explicit_casts_for_sensitive_types: true,
            require_unique_output_names: true,
            require_lower_snake_case: true,
        }
    }
}
```

Schema validator:

```rust
use datafusion::common::{DFSchema, DataFusionError, Result};

pub fn validate_derived_schema(schema: &DFSchema) -> Result<()> {
    let mut seen = std::collections::HashSet::new();

    for field in schema.fields() {
        let name = field.name();

        if !seen.insert(name.clone()) {
            return Err(DataFusionError::Plan(format!(
                "duplicate derived output field name `{name}`"
            )));
        }

        if looks_expression_derived(name) {
            return Err(DataFusionError::Plan(format!(
                "derived output field `{name}` appears expression-derived; add explicit alias"
            )));
        }

        if !is_lower_snake_case(name) {
            return Err(DataFusionError::Plan(format!(
                "derived output field `{name}` must be lower_snake_case"
            )));
        }
    }

    Ok(())
}

fn looks_expression_derived(name: &str) -> bool {
    name.contains('(')
        || name.contains(')')
        || name.contains('+')
        || name.contains('-')
        || name.contains('*')
        || name.contains('/')
        || name.contains('=')
        || name.contains(' ')
}

fn is_lower_snake_case(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('_')
        && !name.ends_with('_')
        && !name.chars().next().unwrap().is_ascii_digit()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
```

---

## S13.19 View dependency audit workflow

```text
1. Parse/register view SQL in isolated SessionContext.
2. Resolve dependencies from logical plan.
3. Validate output schema.
4. Snapshot:
   - DESCRIBE
   - df.schema()
   - arrow_typeof critical expressions
   - dependency list
   - output schema fingerprint
5. Persist ViewContract.
6. On base table change:
   - re-plan view
   - compare output schema
   - compare dependency set
   - emit drift report
   - accept / compatibility view / reject
```

Rust sketch:

```rust
pub async fn build_view_contract(
    ctx: &SessionContext,
    view_ref: &str,
    view_sql: &str,
) -> datafusion::error::Result<ViewContract> {
    let df = ctx.sql(view_sql).await?;
    validate_derived_schema(df.schema())?;

    let arrow_schema = df.schema().as_arrow().clone();
    let fingerprint = derived_schema_fingerprint(&arrow_schema);

    Ok(ViewContract {
        view_ref: view_ref.to_string(),
        version: "draft".to_string(),
        sql: view_sql.to_string(),
        output_schema_fingerprint: fingerprint,
        dependencies: vec![], // fill by logical-plan traversal in platform code
        compatibility_policy: ViewCompatibilityPolicy::default(),
    })
}
```

---

## S13.20 View versioning patterns

### S13.20.1 Major/minor naming

```text
semantic.streams_v1
semantic.streams_v2
semantic.streams_current
semantic.streams_v1_compat
```

### S13.20.2 Non-breaking addition

```sql
CREATE OR REPLACE VIEW semantic.streams_v2 AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  CAST(NULL AS DOUBLE) AS sulfur_wt_pct
FROM semantic.streams_v1;
```

### S13.20.3 Breaking rename with compatibility

```sql
CREATE OR REPLACE VIEW semantic.assays_v2 AS
SELECT
  crude_id,
  api AS api_gravity,
  sulfur_wt_pct
FROM semantic.assays_v1;

CREATE OR REPLACE VIEW semantic.assays_v1_compat AS
SELECT
  crude_id,
  api_gravity AS api,
  sulfur_wt_pct
FROM semantic.assays_v2;
```

### S13.20.4 Current pointer view

```sql
CREATE OR REPLACE VIEW semantic.streams_current AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h,
  sulfur_wt_pct
FROM semantic.streams_v2;
```

Avoid:

```sql
CREATE OR REPLACE VIEW semantic.streams_current AS
SELECT *
FROM semantic.streams_v2;
```

---

## S13.21 CTAS versioning patterns

### S13.21.1 Session-local CTAS

```sql
CREATE TABLE tmp_streams_debug AS
SELECT
  stream_id,
  unit_id,
  mass_flow_kg_h
FROM semantic.streams_v1
LIMIT 1000;
```

Use for:

```text
debugging
tests
temporary analysis
session-local derived data
```

### S13.21.2 Durable export alternative

```sql
COPY (
  SELECT
    stream_id,
    unit_id,
    mass_flow_kg_h
  FROM semantic.streams_v1
)
TO 's3://lake/curated/streams_v1/'
STORED AS PARQUET;
```

Use when:

```text
durable files required
partitioning required
writer options required
schema manifest required
object-store lifecycle controlled outside default catalog
```

### S13.21.3 Custom catalog CTAS

If custom catalog/provider implements durable CTAS:

```text
CTAS should:
  validate output schema
  write data atomically or stage+commit
  register table metadata
  persist schema fingerprint
  audit source query + dependencies
  expose DESCRIBE-compatible schema
```

---

## S13.22 Testing matrix

```text
View tests:
  [ ] CREATE VIEW succeeds
  [ ] DESCRIBE view matches expected schema
  [ ] df.schema() for view query matches expected schema
  [ ] arrow_typeof critical fields matches expected types
  [ ] no SELECT *
  [ ] all computed fields aliased
  [ ] duplicate output names rejected
  [ ] old view over new table works or fails intentionally
  [ ] source rename produces expected diagnostic
  [ ] source type drift produces expected diagnostic

CTAS tests:
  [ ] CTAS output schema matches SELECT output schema
  [ ] CTAS table provider persistence semantics documented
  [ ] CTAS with expression-derived names rejected
  [ ] CTAS with SELECT * rejected for curated/semantic layers
  [ ] CTAS read-after-create DESCRIBE snapshot stored
  [ ] durable custom CTAS read-after-write schema matches expected

Compatibility view tests:
  [ ] v1_compat over v2 preserves v1 schema
  [ ] renamed fields mapped
  [ ] dropped fields filled with NULL/default
  [ ] narrowed/widened types explicitly cast
  [ ] nullability proof query included
```

---

## S13.23 Deployment advisory

```text
Views:
  use as stable semantic/API schemas
  version explicitly
  never use SELECT * in stable views
  aliases are required
  casts are required for contract-sensitive fields
  compatibility views are required for breaking source changes
```

```text
CTAS:
  use for materialization only with persistence semantics understood
  default DataFusion CREATE TABLE is not a durable database table abstraction
  custom provider/catalog must define durable behavior
  output schema must be validated before/after creation
```

```text
Derived objects:
  treat as API surfaces
  snapshot schema fingerprints
  track dependencies
  re-plan after source schema drift
  test old-query/new-table and new-query/old-table compatibility
```

```text
Security:
  semantic views can hide columns and normalize access
  views are not a substitute for provider-enforced row-level security
  information_schema can expose view/table metadata; filter for tenants
```

---

## S13.24 Anti-pattern inventory

```text
Derived schema anti-patterns:
  CREATE VIEW AS SELECT *
  CREATE TABLE AS SELECT *
  unaliased computed expressions
  unaliased aggregates
  unaliased window functions
  duplicate output names after joins
  relying on inferred source field casing
  relying on implicit type coercion in stable views
  treating view schema as independent of dependencies
  creating compatibility view without schema snapshot
  assuming CTAS is durable in default catalog
  using CTAS when COPY/write_parquet is needed
  failing to re-plan views after source drift
  failing to test arrow_typeof for critical fields
  changing current view to point at new version without compatibility report
  using SELECT * in current pointer view
```

---

## S13.25 Agent checklist

```text
[ ] Every stable view select item has explicit alias.
[ ] Every computed/aggregate/window/CASE/function expression has explicit alias.
[ ] Every contract-sensitive type has explicit CAST or arrow_cast.
[ ] No SELECT * in stable views, CTAS, compatibility views, current views, exports.
[ ] Duplicate join columns are semantically renamed.
[ ] Source quoted identifiers are immediately aliased to canonical names.
[ ] View schema is snapshot after creation:
    DESCRIBE
    df.schema()
    arrow_typeof critical fields
    schema fingerprint
[ ] View dependencies are tracked:
    base tables
    base columns
    UDFs/functions
    catalogs/schemas
[ ] Source drift is handled:
    rename mapping
    cast plan
    nullability proof
    default/NULL fill
    compatibility view
[ ] CTAS persistence semantics are explicit:
    in-memory default vs durable custom provider.
[ ] Compatibility views preserve old schema exactly.
[ ] Current pointer view uses explicit projection.
[ ] Derived object schema is treated as API surface.
```

## S13.26 Minimal stable-view template

```sql
CREATE OR REPLACE VIEW semantic.<object>_v<major> AS
SELECT
  -- canonical direct fields
  src.id AS object_id,
  src.name AS object_name,

  -- explicit type contracts
  arrow_cast(src.amount_raw, 'Decimal128(20, 4)') AS amount_usd,

  -- explicit timestamp unit/timezone
  arrow_cast(src.event_ts_raw, 'Timestamp(ns, "UTC")') AS event_ts,

  -- explicit computed names
  src.mass_flow_kg_h / 1000.0 * 24.0 AS mass_flow_t_d,

  -- explicit nested extraction
  src.assay['api_gravity'] AS assay_api_gravity

FROM curated.<source_table> AS src

WHERE src.id IS NOT NULL;
```

Core operating rule: **a view or CTAS object is a schema contract derived from a query; make the derivation explicit, version it, snapshot it, and never let source drift or implicit output naming define public schema behavior.**

[1]: https://datafusion.apache.org/user-guide/sql/ddl.html?utm_source=chatgpt.com "DDL — Apache DataFusion documentation"
[2]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html?utm_source=chatgpt.com "Custom Table Provider — Apache DataFusion documentation"
[3]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/enum.LogicalPlan.html?utm_source=chatgpt.com "LogicalPlan in datafusion_expr::logical_plan - Rust"
[4]: https://datafusion.apache.org/contributor-guide/specification/output-field-name-semantic.html?utm_source=chatgpt.com "Output field name semantics - Apache DataFusion"
[5]: https://datafusion.apache.org/user-guide/sql/select.html?utm_source=chatgpt.com "SELECT syntax — Apache DataFusion documentation"
[6]: https://docs.rs/datafusion/latest/datafusion/?utm_source=chatgpt.com "datafusion - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"


# DataFusion Advanced — S14) Schema testing, diagnostics, and error cookbook

## S14.0 Objective

Unify schema validation into a deterministic, agent-runnable test and diagnostics layer:

```text
schema source
  → schema construction test
  → registration/catalog test
  → logical plan schema test
  → optimized plan schema test
  → physical plan schema test
  → runtime RecordBatch schema test
  → sink/read-after-write schema test
  → golden schema snapshot
  → diagnostic payload + remediation plan
```

The attached documentation already covers many ingredients—Arrow/`RecordBatch`, `DFSchema`, SQL type mapping, `DESCRIBE`, `information_schema`, custom providers, SQL logic tests, optimizer tests, and error handling—but those are distributed across general testing, diagnostics, provider, catalog, and SQL sections rather than expressed as one schema-specific cookbook. 

DataFusion’s test guidance emphasizes `sqllogictest` as a broad, fast regression harness for SQL behavior, while Rust-level tests are still required for low-level API invariants such as provider schema, `RecordBatch` correctness, and optimizer rewrites. ([Apache DataFusion][1])

---

## S14.1 Schema test taxonomy

```text
SchemaTest
  ├─ Arrow schema equality
  ├─ DFSchema equality / qualifier / logical type equality
  ├─ expression type/nullability inference
  ├─ logical-plan schema
  ├─ optimized logical-plan schema
  ├─ physical-plan schema
  ├─ RecordBatch schema
  ├─ RecordBatchStream schema
  ├─ DESCRIBE / SHOW COLUMNS
  ├─ information_schema.columns
  ├─ arrow_typeof / arrow_field / arrow_metadata
  ├─ sqllogictest schema/result assertions
  ├─ provider schema/projection tests
  ├─ source inference / drift tests
  ├─ sink read-after-write tests
  └─ negative error-class tests
```

Use `DFSchema` for logical planning checks because it wraps Arrow schema information with relation/table qualifiers and supports multi-table resolution. Use Arrow `Schema` / `RecordBatch` checks for physical/runtime validation. ([Docs.rs][2])

---

## S14.2 Test surface decision matrix

| Test                        | Scope               | Best API                                          | Use when                                        |
| --------------------------- | ------------------- | ------------------------------------------------- | ----------------------------------------------- |
| Arrow schema equality       | physical exactness  | `Schema`, `Field`                                 | `RecordBatch`, provider output, sink output     |
| `DFSchema` equality         | logical schema      | `df.schema()`, `LogicalPlan::schema()`            | SQL/DataFrame planning, qualifier checks        |
| Expression type/nullability | field derivation    | `ExprSchemable::get_type`, `nullable`, `to_field` | generated expressions, casts, CASE, UDF calls   |
| Logical vs physical schema  | plan lowering       | `df.logical_plan()`, `create_physical_plan()`     | custom operators, optimizer rewrites            |
| Runtime batch schema        | execution output    | `RecordBatch::try_new`, `batch.schema()`          | custom providers, UDFs, streams                 |
| Stream schema               | streaming execution | `RecordBatchStream::schema()`                     | custom `ExecutionPlan` / streaming APIs         |
| SQL schema audit            | user-facing SQL     | `DESCRIBE`, `SHOW COLUMNS`                        | DDL, views, CTAS, registered tables             |
| Catalog schema audit        | metadata plane      | `information_schema.columns`                      | catalog, schemas, views, column visibility      |
| Arrow type audit            | expression-level    | `arrow_typeof(expr)`                              | casts, inferred SQL types, nested fields        |
| SQL behavior regression     | broad SQL           | `.slt` files                                      | query compatibility, errors, optimizer behavior |
| Provider drift              | source integration  | custom Rust tests                                 | projection, filter pushdown, dynamic schemas    |
| Sink round-trip             | output durability   | write + read + compare                            | Parquet/Arrow/JSON/CSV exports                  |

The DataFrame API exposes schema and plan inspection methods, while `RecordBatch` requires its schema to match the column arrays’ data types and equal lengths. ([Docs.rs][3])

---

## S14.3 Arrow schema equality

### S14.3.1 Physical exact equality

Use when schema must match by field order, field names, data types, nullability, and selected metadata.

```rust
use datafusion::arrow::datatypes::{Schema, SchemaRef};
use datafusion::error::{DataFusionError, Result};

pub fn assert_arrow_schema_exact(
    expected: &Schema,
    actual: &Schema,
    include_metadata: bool,
) -> Result<()> {
    if expected.fields().len() != actual.fields().len() {
        return Err(DataFusionError::Execution(format!(
            "schema field count mismatch: expected={}, actual={}",
            expected.fields().len(),
            actual.fields().len()
        )));
    }

    for (idx, (e, a)) in expected.fields().iter().zip(actual.fields().iter()).enumerate() {
        if e.name() != a.name() {
            return Err(DataFusionError::Execution(format!(
                "schema field name mismatch at index {idx}: expected={}, actual={}",
                e.name(),
                a.name()
            )));
        }

        if e.data_type() != a.data_type() {
            return Err(DataFusionError::Execution(format!(
                "schema field type mismatch at index {idx} field `{}`: expected={:?}, actual={:?}",
                e.name(),
                e.data_type(),
                a.data_type()
            )));
        }

        if e.is_nullable() != a.is_nullable() {
            return Err(DataFusionError::Execution(format!(
                "schema field nullability mismatch at index {idx} field `{}`: expected={}, actual={}",
                e.name(),
                e.is_nullable(),
                a.is_nullable()
            )));
        }

        if include_metadata && e.metadata() != a.metadata() {
            return Err(DataFusionError::Execution(format!(
                "schema field metadata mismatch at index {idx} field `{}`",
                e.name()
            )));
        }
    }

    if include_metadata && expected.metadata() != actual.metadata() {
        return Err(DataFusionError::Execution(
            "schema-level metadata mismatch".to_string(),
        ));
    }

    Ok(())
}
```

### S14.3.2 Field-order test

```rust
#[test]
fn schema_field_order_is_contract() {
    use datafusion::arrow::datatypes::{DataType, Field, Schema};

    let expected = Schema::new(vec![
        Field::new("stream_id", DataType::Utf8, false),
        Field::new("mass_flow_kg_h", DataType::Float64, false),
    ]);

    let actual_wrong_order = Schema::new(vec![
        Field::new("mass_flow_kg_h", DataType::Float64, false),
        Field::new("stream_id", DataType::Utf8, false),
    ]);

    assert!(assert_arrow_schema_exact(&expected, &actual_wrong_order, false).is_err());
}
```

Agent rule:

```text
Arrow Schema equality for physical contracts is ordered.
Do not compare only field sets unless the consumer contract is explicitly name-based.
```

---

## S14.4 `DFSchema` equality

### S14.4.1 Logical compatibility

Use `DFSchema` when testing logical planning, qualifiers, and DataFusion’s type-equivalence helpers. `DFSchema` includes qualifiers and exposes helpers such as `has_equivalent_names_and_types`, `datatype_is_logically_equal`, and `datatype_is_semantically_equal`. ([Docs.rs][2])

```rust
use datafusion::common::{DFSchema, Result};

pub fn assert_df_schema_logically_equivalent(
    expected: &DFSchema,
    actual: &DFSchema,
) -> Result<()> {
    expected.has_equivalent_names_and_types(actual)?;
    Ok(())
}
```

### S14.4.2 Qualifier test

```rust
use datafusion::arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::{Column, DFSchema};

#[test]
fn df_schema_detects_ambiguous_unqualified_name() -> datafusion::error::Result<()> {
    let left = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
    ]);
    let right = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
    ]);

    let l = DFSchema::try_from_qualified_schema("orders", &left)?;
    let r = DFSchema::try_from_qualified_schema("customers", &right)?;
    let joined = l.join(&r)?;

    let ids = joined.columns_with_unqualified_name("id");
    assert_eq!(ids.len(), 2);

    assert!(joined.has_column(&Column::from_qualified_name("orders.id")));
    assert!(joined.has_column(&Column::from_qualified_name("customers.id")));

    Ok(())
}
```

Agent rule:

```text
Use DFSchema for planning-resolution tests.
Use Arrow Schema for runtime batch tests.
Never use Arrow Schema to test table-qualifier behavior.
```

---

## S14.5 Logical vs physical schema

### S14.5.1 Logical plan schema

```rust
use datafusion::prelude::*;

pub async fn assert_logical_schema(
    ctx: &SessionContext,
    sql: &str,
    expected_field_names: &[&str],
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let schema = df.logical_plan().schema();

    let actual = schema
        .fields()
        .iter()
        .map(|f| f.name().as_str())
        .collect::<Vec<_>>();

    assert_eq!(actual, expected_field_names);
    Ok(())
}
```

### S14.5.2 Optimized logical schema

```rust
pub async fn assert_optimized_schema_stable(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let before = df.schema().as_arrow().clone();

    let optimized = df.clone().into_optimized_plan()?;
    let after = optimized.schema().as_arrow().clone();

    assert_arrow_schema_exact(&before, &after, false)?;
    Ok(())
}
```

Use optimized-plan extraction in diagnostics and tests, not as a normal runtime execution path, because DataFrame plan extraction can lose the surrounding `SessionState` context. The uploaded document already treats these methods as testing-oriented rather than ordinary execution APIs. 

### S14.5.3 Physical plan schema

```rust
pub async fn assert_physical_schema_matches_dataframe(
    df: DataFrame,
) -> datafusion::error::Result<()> {
    let logical = df.schema().as_arrow().clone();
    let physical = df.create_physical_plan().await?;
    let physical_schema = physical.schema();

    assert_arrow_schema_exact(&logical, physical_schema.as_ref(), false)?;
    Ok(())
}
```

Agent rule:

```text
Logical schema and physical schema should agree for stable output.
If they differ, the plan lowered incorrectly or the comparison policy is wrong.
```

---

## S14.6 `RecordBatch` schema tests

### S14.6.1 Constructor test

```rust
use std::sync::Arc;
use datafusion::arrow::{
    array::{ArrayRef, Int64Array, StringArray},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};

#[test]
fn record_batch_rejects_schema_type_mismatch() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Int64, false),
    ]));

    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from(vec!["not-int"])),
    ];

    assert!(RecordBatch::try_new(schema, columns).is_err());
}
```

A `RecordBatch` is a two-dimensional columnar batch with a schema that must match the arrays’ data types; `RecordBatch::try_new` is therefore a core physical schema validation hook. ([Docs.rs][4])

### S14.6.2 All batches same schema

```rust
use datafusion::arrow::record_batch::RecordBatch;

pub fn assert_all_batches_same_schema(batches: &[RecordBatch]) {
    if let Some(first) = batches.first() {
        let expected = first.schema();

        for (idx, batch) in batches.iter().enumerate() {
            assert_eq!(
                expected.fields(),
                batch.schema().fields(),
                "RecordBatch schema mismatch at batch {idx}"
            );
        }
    }
}
```

### S14.6.3 Stream schema test

A `RecordBatchStream` implementation must guarantee that all returned `RecordBatch` values have the same schema as the stream’s `schema()` method. ([Docs.rs][5])

```rust
pub fn assert_stream_schema_matches_batch(
    stream_schema: &datafusion::arrow::datatypes::SchemaRef,
    batch: &RecordBatch,
) {
    assert_eq!(stream_schema.fields(), batch.schema().fields());
}
```

Agent rule:

```text
Runtime schema validation must test both stream schema and every emitted batch schema.
An empty stream still has a schema contract.
```

---

## S14.7 SQL-facing schema tests

### S14.7.1 `DESCRIBE`

```sql
DESCRIBE semantic.streams_v1;
DESC semantic.streams_v1;
```

DataFusion SQL reference includes `DESCRIBE`, and `information_schema` docs also state that schema details can be accessed through `SHOW COLUMNS` or `information_schema.columns`. ([Apache DataFusion][6])

Golden snapshot shape:

```text
column_name | data_type | is_nullable
stream_id   | Utf8      | NO
mass_flow   | Float64   | NO
```

### S14.7.2 `SHOW COLUMNS`

```sql
SHOW COLUMNS FROM semantic.streams_v1;
```

### S14.7.3 `information_schema.columns`

```sql
SELECT
  table_catalog,
  table_schema,
  table_name,
  column_name,
  data_type,
  is_nullable
FROM information_schema.columns
WHERE table_schema = 'semantic'
  AND table_name = 'streams_v1'
ORDER BY ordinal_position;
```

Use `ORDER BY` in all metadata assertions for deterministic outputs. `information_schema.columns` is the standard DataFusion metadata surface for table columns. ([Apache DataFusion][7])

### S14.7.4 `arrow_typeof`

```sql
SELECT
  arrow_typeof(stream_id) AS stream_id_type,
  arrow_typeof(mass_flow_kg_h) AS mass_flow_type,
  arrow_typeof(event_ts) AS event_ts_type
FROM semantic.streams_v1
LIMIT 1;
```

DataFusion uses Arrow’s type system for query execution, and SQL types map to Arrow types in `CREATE EXTERNAL TABLE` and `CAST`; `arrow_typeof` is the expression-level audit function for the actual Arrow type. ([Apache DataFusion][8])

---

## S14.8 SQLLogicTest schema assertions

DataFusion’s docs call out `datafusion_sqllogictest` as the SQL logic test runner, and the testing guide recommends sqllogictest as a fast, broad regression suite for SQL behavior. ([Docs.rs][9])

### S14.8.1 Deterministic schema query

```text
statement ok
CREATE TABLE streams AS VALUES
  ('S1', 100.0),
  ('S2', 200.0);

query TT
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_name = 'streams'
ORDER BY ordinal_position;
----
column1 Utf8
column2 Float64
```

### S14.8.2 `arrow_typeof` assertion

```text
query T
SELECT arrow_typeof(CAST('123.45' AS DECIMAL(20, 4)));
----
Decimal128(20, 4)
```

### S14.8.3 Error assertion

```text
statement error
SELECT missing_col FROM streams;
```

### S14.8.4 Rowsort for unordered outputs

```text
query TT rowsort
SELECT region, COUNT(*) FROM sales GROUP BY region;
----
eu 3
us 5
```

Agent rules:

```text
Use SQLLogicTest for:
  SQL behavior
  schema query outputs
  errors
  DDL/view/CTAS shape
  broad regression coverage

Do not use SQLLogicTest for:
  low-level Rust provider invariants
  RecordBatch constructor behavior
  custom stream schema invariants
  Arrow metadata object equality
```

---

## S14.9 Error taxonomy

```text
SchemaErrorTaxonomy
  ├─ ambiguous_column
  ├─ missing_column
  ├─ duplicate_field
  ├─ incompatible_type
  ├─ invalid_nullability
  ├─ invalid_record_batch
  ├─ unsupported_sql_type
  ├─ schema_inference_failure
  ├─ file_schema_drift
  ├─ provider_output_mismatch
  ├─ logical_physical_schema_mismatch
  ├─ sink_schema_mismatch
  ├─ optimizer_schema_drift
  └─ metadata_schema_mismatch
```

DataFusion exposes a central `DataFusionError` / `Result` error model, and the source includes schema-oriented error helpers such as schema data-fusion errors. ([Docs.rs][10])

---

## S14.10 Error cookbook

### S14.10.1 Ambiguous column

Symptom:

```sql
SELECT id
FROM orders o
JOIN customers c
  ON o.customer_id = c.id;
```

Diagnostic:

```json
{
  "error_class": "ambiguous_column",
  "phase": "logical_binding",
  "identifier": "id",
  "candidate_resolutions": ["orders.id", "customers.id"],
  "suggested_fix": "Qualify the column or project aliases: o.id AS order_id, c.id AS customer_id"
}
```

Remediation:

```sql
SELECT
  o.id AS order_id,
  c.id AS customer_id
FROM orders o
JOIN customers c
  ON o.customer_id = c.id;
```

Rust guard:

```rust
pub fn assert_unambiguous_column(
    schema: &datafusion::common::DFSchema,
    name: &str,
) -> datafusion::error::Result<()> {
    let matches = schema.columns_with_unqualified_name(name);

    if matches.len() > 1 {
        return Err(datafusion::error::DataFusionError::Plan(format!(
            "ambiguous column `{name}`; candidates={matches:?}"
        )));
    }

    Ok(())
}
```

---

### S14.10.2 Missing column

Symptom:

```sql
SELECT mass_flow_kg_h
FROM streams;
```

but source field is `"Mass Flow kg/h"` or `mass_flow_raw`.

Diagnostic:

```json
{
  "error_class": "missing_column",
  "phase": "logical_binding",
  "identifier": "mass_flow_kg_h",
  "candidate_fields": ["Mass Flow kg/h", "mass_flow_raw"],
  "suggested_fix": "Use quoted source identifier and alias: \"Mass Flow kg/h\" AS mass_flow_kg_h"
}
```

Remediation:

```sql
SELECT
  "Mass Flow kg/h" AS mass_flow_kg_h
FROM raw_streams;
```

Agent rule:

```text
Resolve missing column by checking:
  casing
  quoted identifiers
  table qualifier
  source normalization mapping
  view dependency drift
```

---

### S14.10.3 Duplicate field

Symptom:

```sql
SELECT
  o.id AS id,
  c.id AS id
FROM orders o
JOIN customers c ON o.customer_id = c.id;
```

Diagnostic:

```json
{
  "error_class": "duplicate_field",
  "phase": "output_schema_validation",
  "field_name": "id",
  "suggested_fix": "Use unique aliases: o.id AS order_id, c.id AS customer_id"
}
```

Remediation:

```sql
SELECT
  o.id AS order_id,
  c.id AS customer_id
FROM orders o
JOIN customers c ON o.customer_id = c.id;
```

Rust check:

```rust
pub fn assert_unique_output_names(schema: &datafusion::common::DFSchema) -> datafusion::error::Result<()> {
    let mut seen = std::collections::HashSet::new();

    for field in schema.fields() {
        if !seen.insert(field.name().clone()) {
            return Err(datafusion::error::DataFusionError::Plan(format!(
                "duplicate output field `{}`",
                field.name()
            )));
        }
    }

    Ok(())
}
```

---

### S14.10.4 Incompatible type

Symptom:

```sql
SELECT amount + is_valid
FROM t;
```

Diagnostic:

```json
{
  "error_class": "incompatible_type",
  "phase": "expression_type_inference",
  "expression": "amount + is_valid",
  "left_type": "Float64",
  "right_type": "Boolean",
  "suggested_fix": "Cast or rewrite expression; arithmetic requires numeric operands"
}
```

Remediation:

```sql
SELECT
  amount + CAST(flag_numeric AS DOUBLE) AS adjusted_amount
FROM t;
```

Rust inference:

```rust
use datafusion::logical_expr::ExprSchemable;

pub fn assert_expr_type(
    expr: &datafusion::logical_expr::Expr,
    schema: &datafusion::common::DFSchema,
    expected: &datafusion::arrow::datatypes::DataType,
) -> datafusion::error::Result<()> {
    let actual = expr.get_type(schema)?;

    if &actual != expected {
        return Err(datafusion::error::DataFusionError::Plan(format!(
            "expression type mismatch: expected={expected:?}, actual={actual:?}"
        )));
    }

    Ok(())
}
```

---

### S14.10.5 Invalid nullability

Symptom:

```text
schema says stream_id nullable=false
runtime batch contains NULL stream_id
```

Diagnostic:

```json
{
  "error_class": "invalid_nullability",
  "phase": "runtime_batch_validation",
  "field": "stream_id",
  "expected_nullable": false,
  "observed_nulls": true,
  "suggested_fix": "Filter/reject NULL stream_id rows or change field nullable=true"
}
```

Validation query:

```sql
SELECT COUNT(*) AS null_stream_id_rows
FROM streams
WHERE stream_id IS NULL;
```

Remediation:

```sql
CREATE VIEW streams_non_null AS
SELECT *
FROM streams
WHERE stream_id IS NOT NULL;
```

DataFusion table constraints docs state that nullability is enforced during execution output, but not validated at ingestion time, so applications/providers must validate ingested data. ([Docs.rs][10])

---

### S14.10.6 Invalid `RecordBatch`

Symptom:

```rust
let batch = RecordBatch::try_new(schema, columns)?;
```

fails.

Diagnostic:

```json
{
  "error_class": "invalid_record_batch",
  "phase": "record_batch_construction",
  "reason": "array datatype/order/length does not match schema",
  "suggested_fix": "Build arrays in schema field order and ensure exact Arrow data types"
}
```

Remediation checklist:

```text
[ ] field count == column count
[ ] array[i].data_type == schema.field(i).data_type
[ ] array[i].len == all other arrays.len
[ ] no nulls in non-nullable fields
[ ] arrays are in schema order
```

---

### S14.10.7 Unsupported SQL type

Symptom:

```sql
CREATE EXTERNAL TABLE t (
  id UUID,
  created DATETIME,
  payload VARBINARY
)
STORED AS CSV
LOCATION 't.csv';
```

Diagnostic:

```json
{
  "error_class": "unsupported_sql_type",
  "phase": "ddl_type_mapping",
  "types": ["UUID", "DATETIME", "VARBINARY"],
  "suggested_fix": "Use VARCHAR for UUID, TIMESTAMP for DATETIME, BYTEA for binary"
}
```

DataFusion’s SQL type mapping is based on Arrow types; unsupported SQL declaration types include types such as `UUID`, `BINARY`, `VARBINARY`, `ENUM`, `DATETIME`, and others. ([Apache DataFusion][8])

Remediation:

```sql
CREATE EXTERNAL TABLE t (
  id VARCHAR,
  created TIMESTAMP,
  payload BYTEA
)
STORED AS CSV
LOCATION 't.csv';
```

---

### S14.10.8 Schema inference failure

Symptom:

```text
CSV/JSON source inferred wrong type or incompatible rows later in file.
```

Diagnostic:

```json
{
  "error_class": "schema_inference_failure",
  "phase": "source_registration",
  "format": "CSV",
  "field": "price",
  "inferred_type": "Int64",
  "bad_value": "12.34",
  "suggested_fix": "Supply explicit schema or ingest as VARCHAR staging + arrow_try_cast"
}
```

Remediation A — explicit schema:

```sql
CREATE EXTERNAL TABLE raw_prices (
  node_id VARCHAR,
  price_usd_bbl DOUBLE
)
STORED AS CSV
LOCATION 'prices.csv'
OPTIONS ('has_header' 'true');
```

Remediation B — all-text staging:

```sql
CREATE EXTERNAL TABLE raw_prices_stage (
  node_id VARCHAR,
  price_raw VARCHAR
)
STORED AS CSV
LOCATION 'prices.csv'
OPTIONS ('has_header' 'true');

CREATE VIEW prices_accepted AS
SELECT
  node_id,
  arrow_try_cast(price_raw, 'Float64') AS price_usd_bbl
FROM raw_prices_stage
WHERE arrow_try_cast(price_raw, 'Float64') IS NOT NULL;
```

---

### S14.10.9 File schema drift

Symptom:

```text
part-000.parquet: amount Float64
part-001.parquet: amount Utf8
```

Diagnostic:

```json
{
  "error_class": "file_schema_drift",
  "phase": "dataset_preflight",
  "dataset": "s3://lake/streams/",
  "field": "amount",
  "expected_type": "Float64",
  "actual_type": "Utf8",
  "file": "part-001.parquet",
  "suggested_fix": "Quarantine incompatible file or rewrite to target schema"
}
```

Remediation:

```text
Option A: quarantine incompatible file
Option B: rewrite offending file with target schema
Option C: register separate table and compatibility view
Option D: ingest as staged text and cast
```

Test fixture:

```text
fixtures/file_schema_drift/
  good/part-000.parquet
  bad_type/part-001.parquet
  missing_required/part-002.parquet
```

---

### S14.10.10 Provider output mismatch

Symptom:

```text
TableProvider::schema() says [stream_id Utf8, mass_flow Float64]
ExecutionPlan stream emits [mass_flow Float64, stream_id Utf8]
```

Diagnostic:

```json
{
  "error_class": "provider_output_mismatch",
  "phase": "runtime_stream_validation",
  "provider": "RemoteStreamsProvider",
  "expected_schema": ["stream_id:Utf8", "mass_flow_kg_h:Float64"],
  "actual_schema": ["mass_flow_kg_h:Float64", "stream_id:Utf8"],
  "suggested_fix": "Emit RecordBatch arrays in projection/output schema order"
}
```

Remediation:

```rust
pub fn validate_provider_batch(
    expected: &datafusion::arrow::datatypes::SchemaRef,
    batch: &datafusion::arrow::record_batch::RecordBatch,
) -> datafusion::error::Result<()> {
    if expected.fields() != batch.schema().fields() {
        return Err(datafusion::error::DataFusionError::Execution(format!(
            "provider emitted wrong schema: expected={:?}, actual={:?}",
            expected.fields(),
            batch.schema().fields()
        )));
    }

    Ok(())
}
```

---

## S14.11 Remediation map

| Error class              | Primary remediation                  | Secondary remediation          |
| ------------------------ | ------------------------------------ | ------------------------------ |
| ambiguous column         | qualify column                       | project aliases after join     |
| missing column           | quote / qualify / use canonical name | update view dependency mapping |
| duplicate field          | alias output                         | reject generated projection    |
| incompatible type        | cast / arrow_cast                    | staging + arrow_try_cast       |
| invalid nullability      | filter / reject nulls                | widen nullability              |
| invalid RecordBatch      | fix array order/type/length          | use `RecordBatch::try_new`     |
| unsupported SQL type     | normalize SQL type                   | Arrow extension metadata       |
| schema inference failure | explicit schema                      | all-`Utf8` staging             |
| file schema drift        | reject/quarantine file               | rewrite or compatibility view  |
| provider output mismatch | fix projection mapping               | runtime strict validation      |
| optimized schema drift   | preserve alias                       | reject optimizer rewrite       |
| sink schema mismatch     | explicit sink projection             | read-after-write audit         |

---

## S14.12 Agent artifacts

### S14.12.1 Schema diff report

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiffReport {
    pub report_version: String,
    pub context: SchemaDiffContext,
    pub expected_fingerprint: String,
    pub actual_fingerprint: String,
    pub findings: Vec<SchemaDiffFinding>,
    pub decision: SchemaDecision,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiffContext {
    pub object_ref: String,
    pub phase: String,
    pub operation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiffFinding {
    pub path: String,
    pub kind: SchemaDiffKind,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub severity: Severity,
    pub suggested_fix: Option<String>,
}
```

### S14.12.2 Compatibility matrix

```json
{
  "object_ref": "semantic.streams_v2",
  "physical_exact": false,
  "logical_equivalent": true,
  "semantic_equivalent": true,
  "coercible": true,
  "user_contract_equal": false,
  "decision": "requires_migration",
  "reason": "output field added and schema version changed"
}
```

### S14.12.3 Suggested cast plan

```json
{
  "casts": [
    {
      "field": "price_usd_bbl",
      "source_expr": "price_raw",
      "source_type": "Utf8",
      "target_type": "Decimal128(20, 4)",
      "cast_sql": "arrow_try_cast(price_raw, 'Decimal128(20, 4)') AS price_usd_bbl",
      "lossiness": "UnknownUntilParsed",
      "requires_rejected_rows_audit": true
    }
  ]
}
```

### S14.12.4 Generated failing fixture

```json
{
  "fixture_name": "nullable_stream_id_violation",
  "schema": [
    {"name": "stream_id", "type": "Utf8", "nullable": false},
    {"name": "mass_flow_kg_h", "type": "Float64", "nullable": false}
  ],
  "rows": [
    {"stream_id": null, "mass_flow_kg_h": 100.0}
  ],
  "expected_error_class": "invalid_nullability"
}
```

### S14.12.5 Golden schema snapshot

```json
{
  "object_ref": "semantic.streams_v1",
  "datafusion_version": "54.1.0",
  "schema_fingerprint": "sha256:...",
  "fields": [
    {"ordinal": 0, "name": "stream_id", "data_type": "Utf8", "nullable": false},
    {"ordinal": 1, "name": "mass_flow_kg_h", "data_type": "Float64", "nullable": false}
  ]
}
```

---

## S14.13 Golden schema snapshot harness

```rust
use datafusion::common::DFSchema;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoldenField {
    pub ordinal: usize,
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GoldenSchema {
    pub object_ref: String,
    pub datafusion_version: String,
    pub schema_fingerprint: String,
    pub fields: Vec<GoldenField>,
}

pub fn golden_from_df_schema(object_ref: &str, schema: &DFSchema) -> GoldenSchema {
    let fields = schema
        .fields()
        .iter()
        .enumerate()
        .map(|(idx, f)| GoldenField {
            ordinal: idx,
            name: f.name().clone(),
            data_type: format!("{:?}", f.data_type()),
            nullable: f.is_nullable(),
        })
        .collect::<Vec<_>>();

    GoldenSchema {
        object_ref: object_ref.to_string(),
        datafusion_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_fingerprint: debug_schema_fingerprint(schema),
        fields,
    }
}

fn debug_schema_fingerprint(schema: &DFSchema) -> String {
    let mut s = String::new();

    for (idx, f) in schema.fields().iter().enumerate() {
        s.push_str(&format!(
            "{idx}|{}|{:?}|nullable={}\n",
            f.name(),
            f.data_type(),
            f.is_nullable()
        ));
    }

    // Use SHA-256 in production.
    format!("debug-len-{}", s.len())
}
```

---

## S14.14 Diagnostic payload schema

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaDiagnostic {
    pub error_class: String,
    pub phase: DiagnosticPhase,
    pub object_ref: Option<String>,
    pub node_path: Option<String>,
    pub expression_path: Option<String>,
    pub field_path: Option<String>,
    pub expected_type: Option<String>,
    pub actual_type: Option<String>,
    pub expected_nullable: Option<bool>,
    pub actual_nullable: Option<bool>,
    pub candidates: Vec<String>,
    pub suggested_fix: Option<String>,
    pub remediation: Vec<RemediationStep>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum DiagnosticPhase {
    SourceInference,
    CatalogRegistration,
    SqlParsing,
    LogicalBinding,
    ExpressionTypeInference,
    LogicalPlanning,
    LogicalOptimization,
    PhysicalPlanning,
    RuntimeExecution,
    RecordBatchConstruction,
    SinkWrite,
    ReadAfterWrite,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RemediationStep {
    pub kind: String,
    pub detail: String,
    pub sql: Option<String>,
    pub rust_hint: Option<String>,
}
```

---

## S14.15 Test organization

```text
tests/
  fixtures/
    csv/
      valid_streams.csv
      dirty_streams.csv
    parquet/
      streams_v1/
      streams_v2/
      schema_drift/
    json/
    arrow/
  golden/
    schemas/
      semantic.streams_v1.schema.json
    plans/
      stream_balance.optimized.txt
    outputs/
      stream_balance.rows.txt
  sqllogictest/
    schema_describe.slt
    schema_errors.slt
    views_ctas.slt
    nested_schema.slt
  rust/
    schema_arrow.rs
    schema_dfschema.rs
    schema_provider.rs
    schema_stream.rs
    schema_sink_roundtrip.rs
```

The uploaded plan already recommends a test layout with SQL tests, fixtures, golden plans/outputs, and Rust tests for providers/UDFs/optimizer rules; S14 specializes that structure for schema validation. 

---

## S14.16 CI gates

```text
Schema CI gates:
  cargo test --workspace schema
  sqllogictest schema suite
  provider projection/filter schema tests
  schema golden snapshot diff
  DESCRIBE snapshot diff
  arrow_typeof snapshot diff
  read-after-write Parquet schema diff
  negative drift fixtures
  DataFusion upgrade schema regression
```

Example commands:

```bash
cargo test --workspace schema

cargo test -p query-core schema_provider_projection_order

cargo test --test schema_roundtrip

cargo test --profile=ci --test sqllogictests
```

DataFusion’s contributor testing docs show `cargo test --profile=ci --test sqllogictests` for the sqllogictest suite and recommend broader crate tests before submitting changes. ([Apache DataFusion][1])

---

## S14.17 Deployment advisory

```text
Runtime services:
  validate generated SQL schema before execution
  return schema fingerprint with streamed results
  validate first batch per stream in production
  validate every batch in staging/strict mode
  reject duplicate output names
  reject expression-derived output field names for APIs
```

```text
Batch ETL:
  explicit schema for CSV/JSON
  arrow_typeof tests for parsed fields
  read-after-write Parquet schema audit
  rejected-row fixtures for dirty casts
  golden schema snapshot for curated/semantic outputs
```

```text
Custom providers:
  test schema()
  test scan(None)
  test scan(Some(projection))
  test projection reorder
  test empty stream schema
  test every batch schema
  test provider output mismatch diagnostics
```

```text
DataFusion upgrades:
  run schema golden diff
  run DESCRIBE / information_schema diff
  run arrow_typeof diff
  review output field naming changes
  review optimizer alias preservation
  review Arrow type mapping changes
```

---

## S14.18 Anti-pattern inventory

```text
Schema testing anti-patterns:
  testing only rows, not schema
  relying on pretty output without schema assertions
  snapshotting unordered query results
  using SELECT * in schema golden tests
  comparing schema Debug strings only
  ignoring nullability
  ignoring field order
  treating DFSchema equality as physical equality
  validating only first batch
  assuming empty stream has no schema
  using sqllogictest for provider internals
  using Rust unit tests for broad SQL compatibility only
  omitting negative drift fixtures
  omitting arrow_typeof tests for casts
  accepting expression-derived output names
  failing to test optimizer rewrite schema preservation
  failing to test read-after-write schema
  hiding provider errors as empty results
```

---

## S14.19 Agent checklist

```text
[ ] Determine phase:
    source inference / catalog registration / planning / optimization / physical / runtime / sink.

[ ] Choose schema test:
    Arrow Schema
    DFSchema
    ExprSchemable
    LogicalPlan schema
    PhysicalPlan schema
    RecordBatch schema
    DESCRIBE
    information_schema.columns
    arrow_typeof
    sqllogictest

[ ] Assert:
    field count
    field order
    field names
    data types
    nullability
    metadata if contract-critical
    qualifiers if logical-plan test
    all batches same schema
    stream schema equals batch schema

[ ] For SQL:
    ORDER BY outputs unless rowsort.
    Alias computed fields.
    Use arrow_typeof for type audits.
    Use DESCRIBE/information_schema for catalog schema audits.

[ ] For provider:
    schema() stable
    projection order
    hidden columns
    dynamic schema snapshot
    empty stream schema
    provider output mismatch diagnostics

[ ] For errors:
    emit structured diagnostic
    include phase
    include expected/actual
    include candidate resolutions
    include suggested SQL/Rust fix

[ ] Persist artifacts:
    schema diff report
    compatibility matrix
    suggested cast plan
    failing fixture
    golden schema snapshot
```

## S14.20 Minimal schema validation pipeline

```rust
use datafusion::prelude::*;
use datafusion::arrow::record_batch::RecordBatch;

pub async fn validate_schema_pipeline(
    ctx: &SessionContext,
    sql: &str,
    expected_field_names: &[&str],
) -> datafusion::error::Result<Vec<RecordBatch>> {
    let df = ctx.sql(sql).await?;

    // Logical schema.
    let df_schema = df.schema();
    assert_unique_output_names(df_schema)?;

    let actual_names = df_schema
        .fields()
        .iter()
        .map(|f| f.name().as_str())
        .collect::<Vec<_>>();

    assert_eq!(actual_names, expected_field_names);

    // Optimized logical schema.
    let optimized = df.clone().into_optimized_plan()?;
    assert_eq!(
        optimized
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect::<Vec<_>>(),
        expected_field_names
    );

    // Physical schema.
    let physical = df.clone().create_physical_plan().await?;
    assert_eq!(
        physical
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().as_str())
            .collect::<Vec<_>>(),
        expected_field_names
    );

    // Runtime schema.
    let batches = df.collect().await?;
    assert_all_batches_same_schema(&batches);

    Ok(batches)
}
```

Core operating rule: **every schema-sensitive feature needs both positive schema assertions and negative error fixtures; a row-result test without schema validation is incomplete for DataFusion agents.**

[1]: https://datafusion.apache.org/contributor-guide/testing.html?utm_source=chatgpt.com "Testing — Apache DataFusion documentation"
[2]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html?utm_source=chatgpt.com "DFSchema in datafusion::common - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"
[4]: https://docs.rs/arrow/latest/arrow/record_batch/struct.RecordBatch.html?utm_source=chatgpt.com "RecordBatch in arrow::record_batch - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/execution/trait.RecordBatchStream.html?utm_source=chatgpt.com "RecordBatchStream in datafusion::execution - Rust"
[6]: https://datafusion.apache.org/user-guide/sql/index.html?utm_source=chatgpt.com "SQL Reference — Apache DataFusion documentation"
[7]: https://datafusion.apache.org/user-guide/sql/information_schema.html?utm_source=chatgpt.com "Information Schema — Apache DataFusion documentation"
[8]: https://datafusion.apache.org/user-guide/sql/data_types.html?utm_source=chatgpt.com "Data Types — Apache DataFusion documentation"
[9]: https://docs.rs/datafusion/latest/datafusion/?utm_source=chatgpt.com "datafusion - Rust"
[10]: https://docs.rs/crate/datafusion-common/latest/source/src/error.rs?utm_source=chatgpt.com "datafusion-common 54.1.0"


# DataFusion Advanced — S15) Schema security, governance, and tenant isolation

## S15.0 Objective

Treat schema visibility as a **security boundary**, not a harmless metadata convenience.

```text id="qlb2ur"
Security-sensitive schema surfaces:
  ├─ CatalogProvider / SchemaProvider / TableProvider namespace visibility
  ├─ TableProvider::schema() column visibility
  ├─ TableProvider::scan() row visibility and tenant predicates
  ├─ information_schema / SHOW TABLES / SHOW COLUMNS / SHOW FUNCTIONS / SHOW ALL
  ├─ table statistics, constraints, defaults, partition columns
  ├─ field names, table names, metadata keys, comments, paths
  ├─ object-store registration and direct URL table access
  └─ audit logs and denied-access diagnostics
```

`TableProvider` is the main table abstraction exposed to planning: it supplies table schema, filter-pushdown support, and scan execution. Its `schema()` method returns the columns/types visible to the planner; therefore a governed provider’s `schema()` is a security boundary, not only a typing convenience. ([Docs.rs][1])

Core rule:

```text id="34xaqd"
Users must only be able to see:
  catalogs they are authorized to know exist
  schemas they are authorized to know exist
  tables/views they are authorized to know exist
  columns they are authorized to know exist
  metadata/statistics they are authorized to know exist
  rows permitted by tenant/row policies
```

---

## S15.1 Threat model

```text id="r239u1"
Threat classes:
  metadata enumeration
  tenant breakout
  column discovery
  sensitive table-name discovery
  row-count leakage
  min/max/statistics leakage
  partition-value leakage
  file-path / object-store URI leakage
  direct filesystem/object-store access
  hidden-column projection
  filter-pushdown bypass
  UDF/function-surface discovery
  setting/configuration discovery
  audit-log overexposure
```

DataFusion’s `information_schema` exposes metadata about tables, columns, settings, and functions through `information_schema` views and `SHOW` commands; the `information_schema.columns` surface includes table catalog/schema/name, column name, data type, and nullability, while `information_schema.df_settings` exposes session configuration settings. ([Apache DataFusion][2])

Security posture:

```text id="e9rfl8"
Ad hoc local CLI:
  broad visibility acceptable

single-tenant trusted batch:
  broad visibility acceptable but audit still useful

multi-tenant service:
  least-privilege metadata visibility required

regulated / client-confidential service:
  deny-by-default schema visibility required

LLM-agent-operable platform:
  explicit schema visibility contracts required
```

---

## S15.2 Schema visibility model

### S15.2.1 Visibility object

```rust id="0mtqeq"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaVisibilityPolicy {
    pub principal: PrincipalRef,
    pub tenant_id: String,
    pub catalog_allowlist: Vec<String>,
    pub schema_allowlist: Vec<String>,
    pub table_allowlist: Vec<ObjectRef>,
    pub column_policies: Vec<ColumnVisibilityRule>,
    pub metadata_policy: MetadataVisibilityPolicy,
    pub statistics_policy: StatisticsVisibilityPolicy,
    pub information_schema_policy: InformationSchemaPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrincipalRef {
    pub user_id: String,
    pub roles: Vec<String>,
    pub service_account: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectRef {
    pub catalog: String,
    pub schema: String,
    pub table: String,
}
```

---

### S15.2.2 Column visibility rule

```rust id="7sdke2"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnVisibilityRule {
    pub object_ref: ObjectRef,
    pub column: String,
    pub action: ColumnVisibilityAction,
    pub reason: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ColumnVisibilityAction {
    Allow,
    Hide,
    Mask { mask_expr_sql: String },
    Rename { exposed_name: String },
    RedactMetadata,
    DenyProjection,
    DenyFilter,
}
```

Policy distinction:

```text id="z7nsai"
Hide:
  column does not appear in TableProvider::schema()
  column does not appear in information_schema.columns
  user cannot reference it

Mask:
  column appears, but value expression is transformed
  e.g. email_hash, NULL, bucketed value, redacted string

DenyProjection:
  user may filter/authorize using column indirectly but cannot select it

DenyFilter:
  user may view sanitized values but cannot filter to infer sensitive subgroups

Rename:
  raw source name hidden behind stable semantic field name
```

---

## S15.3 Column allowlists

### S15.3.1 Provider-level allowlist

```rust id="lm9o6a"
use std::collections::HashSet;
use std::sync::Arc;

use datafusion::arrow::datatypes::{Field, Schema, SchemaRef};

pub fn project_visible_schema(
    full_schema: &Schema,
    allowed_columns: &HashSet<String>,
) -> SchemaRef {
    let fields = full_schema
        .fields()
        .iter()
        .filter(|f| allowed_columns.contains(f.name().as_str()))
        .map(|f| f.as_ref().clone())
        .collect::<Vec<Field>>();

    Arc::new(Schema::new_with_metadata(fields, full_schema.metadata().clone()))
}
```

Provider rule:

```text id="vse032"
If a column is hidden:
  remove it from schema()
  remove it from projection mapping
  reject user references during planning
  omit it from information_schema by not registering it in the visible provider
```

Implementation pattern:

```text id="spqij2"
Full backend provider:
  internal_full_schema
  internal field map
  tenant/security filters

Visible provider wrapper:
  visible_schema
  visible projection mapping
  hidden backend fields only used internally
```

---

### S15.3.2 Allowlist wrapper provider

```rust id="x6icoy"
use std::sync::Arc;

use async_trait::async_trait;
use datafusion::arrow::datatypes::SchemaRef;
use datafusion::catalog::{Session, TableProvider};
use datafusion::common::{DataFusionError, Result};
use datafusion::datasource::TableType;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct ColumnAllowlistProvider {
    inner: Arc<dyn TableProvider>,
    visible_schema: SchemaRef,
    visible_to_inner_projection: Vec<usize>,
    policy_name: String,
}

#[async_trait]
impl TableProvider for ColumnAllowlistProvider {
    fn schema(&self) -> SchemaRef {
        self.visible_schema.clone()
    }

    fn table_type(&self) -> TableType {
        self.inner.table_type()
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // Map visible projection indices to inner provider indices.
        let inner_projection = match projection {
            Some(p) => {
                let mapped = p.iter()
                    .map(|i| {
                        self.visible_to_inner_projection.get(*i).copied().ok_or_else(|| {
                            DataFusionError::Plan(format!(
                                "projection index {i} out of bounds for visible schema under policy `{}`",
                                self.policy_name
                            ))
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                Some(mapped)
            }
            None => Some(self.visible_to_inner_projection.clone()),
        };

        // Additional filter validation should reject hidden-column references.
        self.inner.scan(state, inner_projection.as_ref(), filters, limit).await
    }
}
```

Caveat:

```text id="c2q7yq"
Filter expressions passed to scan are already planned against visible schema.
If hidden columns must be used for row security, inject them inside the provider, not through user-visible expressions.
```

---

## S15.4 Hidden columns

### S15.4.1 Hidden-column categories

```text id="i5b7eo"
tenant_id:
  row isolation key

workspace_id:
  workspace isolation key

source_file_path:
  provenance / diagnostics

row_version:
  optimistic concurrency

deleted_flag:
  soft delete policy

security_classification:
  authorization policy

backend_cursor:
  internal pagination / API state
```

### S15.4.2 Hidden-column policy

```rust id="g2h7ul"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HiddenColumnPolicy {
    pub column: String,
    pub present_in_backend: bool,
    pub present_in_provider_schema: bool,
    pub usable_for_policy: bool,
    pub user_can_project: bool,
    pub user_can_filter: bool,
    pub expose_in_information_schema: bool,
    pub expose_in_audit: bool,
}
```

Recommended defaults:

```text id="ks2w16"
tenant_id:
  present_in_backend=true
  present_in_provider_schema=false for tenant-scoped context
  usable_for_policy=true
  user_can_project=false
  user_can_filter=false
  expose_in_information_schema=false
  expose_in_audit=true

source_file_path:
  visible only in diagnostics schema
  redact signed URLs / credentials
```

Agent rule:

```text id="z6apvf"
Hidden policy columns should generally be absent from user-facing schema().
If a hidden column is exposed for filtering, it is no longer hidden metadata.
```

---

## S15.5 Masked columns

### S15.5.1 SQL masking view

```sql id="dkizk3"
CREATE OR REPLACE VIEW semantic.customers_masked AS
SELECT
  customer_id AS customer_id,
  sha256(email) AS email_hash,
  CAST(NULL AS VARCHAR) AS email,
  region AS region
FROM curated.customers;
```

### S15.5.2 Role-specific projection

```sql id="x8g2ok"
-- analyst view
CREATE OR REPLACE VIEW semantic.orders_analyst AS
SELECT
  order_id,
  customer_id,
  product_code,
  CASE
    WHEN margin_usd_bbl IS NULL THEN NULL
    ELSE 'REDACTED'
  END AS margin_redacted
FROM curated.orders;
```

### S15.5.3 Provider masking pattern

```rust id="5orovo"
#[derive(Debug, Clone)]
pub struct MaskRule {
    pub column: String,
    pub exposed_name: String,
    pub mask_expr_sql: String,
    pub applies_to_roles: Vec<String>,
}
```

Provider strategy:

```text id="lbfkta"
Option A — view masking:
  easiest
  SQL-visible
  not sufficient for row-level security if base table is also accessible

Option B — provider masking:
  schema() exposes masked column only
  scan() computes masked values
  base column never exposed

Option C — catalog isolation:
  only masked providers/views registered in tenant/user context
```

Agent rule:

```text id="jbleup"
Masking must be enforced at the lowest layer a user can access.
A masked view is ineffective if the raw table remains visible in the same session.
```

---

## S15.6 Tenant-specific schemas

### S15.6.1 Catalog-per-tenant

```text id="iy1dc8"
tenant_acme.curated.streams
tenant_acme.semantic.stream_balances

tenant_zenith.curated.streams
tenant_zenith.semantic.stream_balances
```

Use when:

```text id="405rlo"
tenant object visibility differs
tenant credentials differ
tenant object-store prefixes differ
tenant statistics must be isolated
tenant information_schema should show only tenant objects
```

DataFusion uses catalog/schema/table hierarchy for table organization, and default catalog/schema settings affect name resolution when SQL omits qualification. The config table shows defaults `datafusion` and `public`, plus `datafusion.catalog.information_schema=false` by default. ([Apache DataFusion][3])

### S15.6.2 Schema-per-workspace

```text id="0z00ht"
tenant_acme.workspace_planning.streams
tenant_acme.workspace_ops.streams
tenant_acme.workspace_sandbox.streams
```

Use when:

```text id="zmkc0q"
same tenant
different workspaces/projects
same credential domain
different object lifecycle/ownership
```

### S15.6.3 Lifecycle schemas

```text id="qs42p8"
tenant_acme.raw
tenant_acme.staging
tenant_acme.curated
tenant_acme.semantic
tenant_acme.diagnostics
tenant_acme.temp
```

Security model:

```text id="fmykyg"
raw:
  restricted, drift-prone, source names may be sensitive

staging:
  restricted, rejected rows, dirty values

curated:
  governed canonical schema

semantic:
  exposed business contract

diagnostics:
  limited to operators/admins

temp:
  session/workspace scoped
```

---

## S15.7 Filtered `information_schema`

### S15.7.1 Why it matters

`information_schema` is not automatically safe because it exposes available tables/views, columns, settings, and functions. DataFusion also exposes `SHOW TABLES`, `SHOW COLUMNS`, `SHOW ALL`, and `SHOW FUNCTIONS` equivalents for many of these metadata surfaces. ([Apache DataFusion][2])

Leak examples:

```text id="4t6zbr"
table names:
  acquisition_targets
  sanctions_hits
  layoffs_model
  confidential_crude_slates

column names:
  social_security_number
  margin_usd_bbl
  acquisition_price
  employee_termination_reason

settings:
  object store behavior
  optimizer/runtime settings
  batch size and memory posture

functions:
  internal scoring UDFs
  remote-fetch UDFs
  masking/bypass UDFs
```

### S15.7.2 Strategies

```text id="uq2w8w"
Strategy 1 — disable information_schema:
  datafusion.catalog.information_schema = false

Strategy 2 — per-tenant SessionContext:
  only register tenant-visible catalogs/schemas/tables

Strategy 3 — filtered CatalogProvider/SchemaProvider:
  catalog/schema/table listings are already policy-filtered

Strategy 4 — safe metadata provider:
  expose curated metadata tables instead of built-in information_schema

Strategy 5 — plan validator:
  reject references to information_schema unless role permits
```

### S15.7.3 Config

```rust id="4r1ov3"
use datafusion::prelude::*;

pub fn secure_session_config() -> datafusion::error::Result<SessionConfig> {
    let mut config = SessionConfig::new();
    config.set_bool("datafusion.catalog.information_schema", false)?;
    config.set_str("datafusion.catalog.default_schema", "semantic")?;
    Ok(config)
}
```

DataFusion’s configuration list states that `datafusion.catalog.information_schema` defaults to `false` and controls access to `information_schema` virtual tables. ([Apache DataFusion][3])

---

## S15.8 Catalog isolation

### S15.8.1 Catalog per tenant

```rust id="ydt7oh"
use std::sync::Arc;
use datafusion::catalog::{MemoryCatalogProvider, MemorySchemaProvider};
use datafusion::prelude::*;

pub fn tenant_context(tenant: &str) -> datafusion::error::Result<SessionContext> {
    let mut config = secure_session_config()?;
    config.set_str("datafusion.catalog.default_catalog", tenant)?;

    let ctx = SessionContext::new_with_config(config);

    let catalog = Arc::new(MemoryCatalogProvider::new());
    catalog.register_schema("semantic", Arc::new(MemorySchemaProvider::new()))?;
    catalog.register_schema("curated", Arc::new(MemorySchemaProvider::new()))?;
    catalog.register_schema("diagnostics", Arc::new(MemorySchemaProvider::new()))?;

    ctx.register_catalog(tenant, catalog);
    Ok(ctx)
}
```

### S15.8.2 Object-store registry per credential domain

```rust id="8lggx8"
use std::sync::Arc;
use datafusion::prelude::*;
use object_store::ObjectStore;
use url::Url;

pub fn register_tenant_object_store(
    ctx: &SessionContext,
    tenant_url_prefix: &str,
    store: Arc<dyn ObjectStore>,
) -> datafusion::error::Result<()> {
    let url = Url::parse(tenant_url_prefix)
        .map_err(|e| datafusion::error::DataFusionError::External(Box::new(e)))?;

    ctx.register_object_store(&url, store);
    Ok(())
}
```

`SessionContext::register_object_store` registers an `ObjectStore` implementation for a specific URL prefix, so credential/tenant boundaries should align with URL prefixes and catalog visibility. ([Docs.rs][4])

### S15.8.3 No direct URL table access

```rust id="f0j4tx"
// Do NOT do this in a multi-tenant or untrusted SQL service:
let ctx = SessionContext::new().enable_url_table();
```

`SessionContext::enable_url_table` is explicitly documented as security-sensitive because it permits direct SQL access to arbitrary local files such as `SELECT * FROM 'my_file.parquet'`. ([Docs.rs][4])

Policy:

```text id="8a2hvs"
Untrusted / multi-tenant service:
  never enable_url_table
  never accept arbitrary LOCATION strings
  register vetted tables only
  register object stores only for authorized prefixes
  validate CREATE EXTERNAL TABLE through admin workflow only
```

---

## S15.9 SQL authorization surface

### S15.9.1 Disable DDL/DML/statements for read-only users

```rust id="h2ztpw"
use datafusion::prelude::*;

pub async fn read_only_sql(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<DataFrame> {
    let options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    ctx.sql_with_options(sql, options).await
}
```

`SessionContext::sql_with_options` validates SQL against `SQLOptions`; the docs show `with_allow_ddl(false)` preventing `CREATE TABLE`, and the Python API documents `with_allow_dml` and `with_allow_statements` for DML and statement/config control. ([Docs.rs][4])

### S15.9.2 Statement risk matrix

| SQL class               | Risk                                  | Default service policy      |
| ----------------------- | ------------------------------------- | --------------------------- |
| `SELECT`                | reads authorized data                 | allow after plan validation |
| `CREATE TABLE`          | mutable catalog / schema exposure     | admin only                  |
| `CREATE EXTERNAL TABLE` | path/object-store exposure            | admin only                  |
| `CREATE VIEW`           | persistent metadata object            | admin/workspace owner       |
| `INSERT` / `COPY`       | writes data / exfil path              | admin/job role              |
| `SET`                   | config mutation / information leakage | deny unless explicit        |
| `SHOW ALL`              | runtime setting leakage               | admin only                  |
| `SHOW FUNCTIONS`        | function-surface leakage              | filtered/admin              |
| `information_schema`    | metadata enumeration                  | disabled/filtered           |

---

## S15.10 Metadata leakage

### S15.10.1 Leakage surfaces

```text id="k0uvza"
Column names:
  indicate sensitive concepts even without data

Table names:
  reveal business activities, clients, projects, incidents

Statistics:
  row count, null count, min/max, distinct count, byte size

Partition values:
  sites, dates, tenants, geographies, customers, case IDs

Schema metadata:
  source paths, lineage, owner, classification, units, comments

File paths:
  S3 bucket names, client names, project names, signed URLs

Function names:
  internal scoring, sanctions, fraud, acquisition, confidential models

Settings:
  engine config, memory/runtime posture, object-store behavior
```

`TableProvider` exposes optional `statistics()` and docs.rs notes statistics can support implementation-specific optimizer behavior; this means statistics should be treated as metadata that can influence planning and can leak values if exposed without policy. ([Docs.rs][1])

### S15.10.2 Metadata redaction policy

```rust id="etmmg6"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetadataLeakagePolicy {
    pub expose_table_names: bool,
    pub expose_column_names: bool,
    pub expose_statistics: bool,
    pub expose_partition_values: bool,
    pub expose_schema_metadata: bool,
    pub expose_source_locations: bool,
    pub expose_function_inventory: bool,
    pub redact_metadata_keys: Vec<String>,
    pub deny_metadata_key_prefixes: Vec<String>,
}
```

Recommended deny prefixes:

```text id="lchxae"
secret
credential
token
password
aws.
gcp.
azure.
source.signed_url
source.private_path
lineage.private_note
```

### S15.10.3 Metadata redaction function

```rust id="4el3jp"
use std::collections::HashMap;

pub fn redact_metadata(
    metadata: &HashMap<String, String>,
    policy: &MetadataLeakagePolicy,
) -> HashMap<String, String> {
    metadata
        .iter()
        .filter_map(|(k, v)| {
            if policy.redact_metadata_keys.contains(k)
                || policy.deny_metadata_key_prefixes.iter().any(|p| k.starts_with(p))
            {
                Some((k.clone(), "[REDACTED]".to_string()))
            } else {
                Some((k.clone(), v.clone()))
            }
        })
        .collect()
}
```

---

## S15.11 Statistics security

### S15.11.1 Sensitive statistics

```text id="7zldj2"
row_count:
  reveals customer/activity volume

null_count:
  reveals completeness / operational gaps

min/max:
  reveals price/margin/volume extremes

distinct_count:
  reveals number of customers/sites/counterparties

partition sizes:
  reveals activity per date/site/tenant

byte size:
  can reveal workload size or sensitive event spikes
```

### S15.11.2 Statistics visibility policy

```rust id="1j54m2"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatisticsVisibilityPolicy {
    pub expose_row_count: bool,
    pub expose_total_byte_size: bool,
    pub expose_column_statistics: bool,
    pub expose_partition_statistics: bool,
    pub sensitive_columns: Vec<String>,
    pub min_group_size_for_stats: Option<u64>,
    pub rounding_bucket: Option<u64>,
}
```

### S15.11.3 Provider `statistics()` posture

```text id="2bb5tt"
Public / tenant-facing provider:
  return None or redacted coarse statistics

Admin / optimizer-internal provider:
  return cached statistics with visibility guard

Multi-tenant provider:
  never expose all-tenant stats through tenant-scoped context

Sensitive column:
  no min/max/null/distinct stats unless explicitly authorized
```

---

## S15.12 Partition-value leakage

### S15.12.1 Partition leakage examples

```text id="qh2ja8"
s3://lake/tenant=acme/customer=secret_corp/event_date=2026-05-24/
s3://lake/site=refinery_x/incident_date=2026-04-01/
s3://lake/acquisition_target=company_y/
```

Risk:

```text id="iqsdtk"
Partition values appear as table columns.
Partition directories may appear in file paths.
Pruning diagnostics may reveal partition values.
information_schema may expose partition columns.
Object-store listing may reveal partition layout.
```

Policy:

```text id="gtvuhy"
Avoid sensitive partition keys.
Use surrogate partition IDs if needed.
Never expose source_file_path with sensitive path segments.
Filter diagnostics and EXPLAIN output in public APIs.
```

---

## S15.13 Audit schema

### S15.13.1 Audit event object

```rust id="0xn7d5"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryAuditEvent {
    pub event_id: String,
    pub timestamp_ms: i64,

    pub user_id: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub session_id: String,

    pub query_hash: String,
    pub sql_redacted: Option<String>,

    pub referenced_objects: Vec<ObjectAccess>,
    pub projected_columns: Vec<ColumnRef>,
    pub filter_summary: Vec<FilterAuditSummary>,

    pub denied_accesses: Vec<DeniedSchemaAccess>,
    pub output_row_count: Option<u64>,
    pub output_schema_fingerprint: Option<String>,

    pub decision: AccessDecision,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectAccess {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub access_kind: AccessKind,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AccessKind {
    TableListed,
    TableScanned,
    ViewResolved,
    MetadataRead,
    StatisticsRead,
    FunctionUsed,
}
```

### S15.13.2 Column and filter audit

```rust id="tdlwqy"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnRef {
    pub catalog: String,
    pub schema: String,
    pub table: String,
    pub column: String,
    pub column_action: ColumnAuditAction,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ColumnAuditAction {
    Projected,
    Filtered,
    Joined,
    Aggregated,
    Masked,
    Hidden,
    Denied,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilterAuditSummary {
    pub object_ref: ObjectRef,
    pub filter_class: String,
    pub references_sensitive_column: bool,
    pub pushed_down: bool,
    pub exact_pushdown: Option<bool>,
}
```

### S15.13.3 Denied access

```rust id="bacfom"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeniedSchemaAccess {
    pub object_ref: Option<ObjectRef>,
    pub column: Option<String>,
    pub metadata_surface: Option<String>,
    pub reason_code: String,
    pub user_message: String,
}
```

### S15.13.4 Audit table DDL

```sql id="3yxh8o"
CREATE EXTERNAL TABLE audit.query_events (
  event_id VARCHAR NOT NULL,
  timestamp_ms BIGINT NOT NULL,
  user_id VARCHAR NOT NULL,
  tenant_id VARCHAR NOT NULL,
  session_id VARCHAR NOT NULL,
  query_hash VARCHAR NOT NULL,
  decision VARCHAR NOT NULL,
  output_row_count BIGINT,
  output_schema_fingerprint VARCHAR
)
STORED AS PARQUET
LOCATION 's3://governance/audit/query_events/';
```

Audit policy:

```text id="mac9pr"
Do not log raw SQL with literals unless redacted.
Do not log secret metadata values.
Do log denied metadata/schema accesses.
Do log output row count if allowed by governance.
Do log schema fingerprint for reproducibility.
```

---

## S15.14 Row-level tenant enforcement

### S15.14.1 Provider-level enforcement

```text id="ex3ybe"
Effective scan predicate:
  user_filter
  AND tenant_id = current_tenant
  AND row_policy_predicate
```

`TableProvider::scan` receives filters and projection/limit, and filter pushdown requires truthful `supports_filters_pushdown`; if the provider claims exact pushdown, only rows passing the expressions should be returned. ([Docs.rs][1])

Provider wrapper sketch:

```rust id="mgxddb"
#[derive(Debug, Clone)]
pub struct TenantPolicy {
    pub tenant_id: String,
    pub tenant_column_backend_name: String,
    pub tenant_column_visible: bool,
}

#[derive(Debug)]
pub struct TenantFilteredProvider {
    inner: Arc<dyn TableProvider>,
    policy: TenantPolicy,
    visible_schema: SchemaRef,
}

impl TenantFilteredProvider {
    fn inject_tenant_filter(&self, user_filters: &[Expr]) -> datafusion::error::Result<Vec<Expr>> {
        let tenant_expr = col(&self.policy.tenant_column_backend_name)
            .eq(lit(self.policy.tenant_id.clone()));

        let mut filters = vec![tenant_expr];
        filters.extend_from_slice(user_filters);
        Ok(filters)
    }
}
```

Rule:

```text id="tegh26"
Tenant predicate injection must happen below any user-controllable catalog/view layer.
If users can access the underlying raw table/provider, view-level tenant filtering is insufficient.
```

---

## S15.15 Filter-pushdown security

### S15.15.1 Exact vs inexact matters

`supports_filters_pushdown` must return one value per filter, and values distinguish `Exact`, `Inexact`, and `Unsupported`; the default is `Unsupported`. If result length mismatches input filter length, DataFusion throws an error. ([Docs.rs][1])

Security implications:

```text id="a1yvni"
Exact:
  DataFusion may trust provider-side filter semantics

Inexact:
  residual filtering must remain
  do not push LIMIT before residual filter

Unsupported:
  DataFusion evaluates filter outside provider
```

DataFusion’s `TableProvider` docs also warn that `LIMIT` cannot be pushed below inexact filters because doing so can return too few rows after residual filtering. ([Docs.rs][1])

Agent rule:

```text id="q6r2gg"
Misdeclared exact pushdown can become an access-control bug if tenant/security filters are incorrectly applied.
```

---

## S15.16 Diagnostics and denied-access messages

### S15.16.1 Safe denied-column error

```json id="jfsgee"
{
  "error_class": "schema_access_denied",
  "phase": "logical_binding",
  "user_id": "user_123",
  "tenant_id": "tenant_acme",
  "object_ref": "tenant_acme.curated.orders",
  "column": "margin_usd_bbl",
  "reason_code": "column_not_visible",
  "user_message": "Column is not available in this schema contract.",
  "audit": true
}
```

Avoid:

```text id="c8jmsl"
“Column margin_usd_bbl exists but you are not allowed to see it.”
```

because that confirms sensitive column existence.

### S15.16.2 Safe table denial

```json id="turzrj"
{
  "error_class": "object_not_found_or_not_authorized",
  "phase": "catalog_resolution",
  "object_ref": "tenant_acme.semantic.acquisition_targets",
  "user_message": "Object not found or not authorized.",
  "audit_reason": "table_exists_but_denied"
}
```

User-facing message should avoid existence confirmation. Audit record can include true reason in restricted governance logs.

---

## S15.17 Secure session builder

```rust id="t8quzy"
use datafusion::prelude::*;

pub struct SecureSessionRequest {
    pub tenant_id: String,
    pub user_id: String,
    pub roles: Vec<String>,
    pub allow_information_schema: bool,
    pub allow_ddl: bool,
    pub allow_dml: bool,
    pub allow_statements: bool,
}

pub fn build_secure_session(req: &SecureSessionRequest) -> datafusion::error::Result<SessionContext> {
    let mut config = SessionConfig::new();

    config.set_str("datafusion.catalog.default_catalog", &req.tenant_id)?;
    config.set_str("datafusion.catalog.default_schema", "semantic")?;
    config.set_bool("datafusion.catalog.information_schema", req.allow_information_schema)?;

    let ctx = SessionContext::new_with_config(config);

    // Register only tenant-authorized catalogs/schemas/tables.
    // Register only object stores for this tenant's allowed URL prefixes.
    // Do not call enable_url_table().

    Ok(ctx)
}

pub fn sql_options_for_request(req: &SecureSessionRequest) -> SQLOptions {
    SQLOptions::new()
        .with_allow_ddl(req.allow_ddl)
        .with_allow_dml(req.allow_dml)
        .with_allow_statements(req.allow_statements)
}
```

---

## S15.18 Governance test matrix

```text id="u8bf3c"
Schema visibility:
  [ ] unauthorized columns absent from provider.schema()
  [ ] unauthorized columns absent from information_schema.columns
  [ ] unauthorized columns cannot be projected
  [ ] unauthorized columns cannot be filtered if policy denies filter
  [ ] masked columns return masked values
  [ ] raw table unavailable when masked view is exposed

Catalog isolation:
  [ ] tenant A cannot see tenant B catalog
  [ ] tenant A cannot resolve tenant B table by fully-qualified name
  [ ] tenant A object store cannot access tenant B prefix
  [ ] default catalog/schema set to tenant scope
  [ ] enable_url_table not used

Information schema:
  [ ] disabled by default
  [ ] SHOW TABLES denied/filtered
  [ ] SHOW COLUMNS denied/filtered
  [ ] SHOW ALL denied/filtered
  [ ] SHOW FUNCTIONS denied/filtered
  [ ] df_settings absent unless admin

Statistics:
  [ ] row counts redacted for sensitive tables
  [ ] min/max stats redacted for sensitive columns
  [ ] all-tenant stats absent from tenant context
  [ ] partition values not exposed through diagnostics

Audit:
  [ ] projected columns logged
  [ ] filters summarized
  [ ] denied access logged
  [ ] output row count captured or redacted by policy
  [ ] SQL text redacted
  [ ] source URIs redacted
```

---

## S15.19 Security diagnostics payloads

### Information-schema denied

```json id="ud4m90"
{
  "error_class": "metadata_surface_denied",
  "phase": "plan_validation",
  "surface": "information_schema.columns",
  "tenant_id": "tenant_acme",
  "reason_code": "information_schema_disabled",
  "user_message": "Metadata introspection is not enabled for this session.",
  "audit": true
}
```

### Direct URL access denied

```json id="ycb85v"
{
  "error_class": "direct_url_table_access_denied",
  "phase": "sql_validation",
  "query_pattern": "SELECT FROM quoted file path",
  "reason_code": "enable_url_table_disabled",
  "user_message": "Direct file path queries are not enabled.",
  "audit": true
}
```

### Statistics redacted

```json id="o0vf00"
{
  "warning_class": "statistics_redacted",
  "phase": "metadata_response",
  "object_ref": "tenant_acme.curated.orders",
  "statistic": "column_min_max",
  "column": "margin_usd_bbl",
  "reason_code": "sensitive_column_statistics",
  "decision": "redact"
}
```

### Tenant breakout attempt

```json id="g9uub4"
{
  "error_class": "tenant_catalog_access_denied",
  "phase": "catalog_resolution",
  "requested_catalog": "tenant_zenith",
  "session_tenant": "tenant_acme",
  "user_message": "Object not found or not authorized.",
  "audit_reason": "cross_tenant_catalog_reference"
}
```

---

## S15.20 Deployment advisory

```text id="z3ftoh"
Session construction:
  create per-tenant/per-principal SessionContext
  register only authorized catalogs/schemas/tables
  set default catalog/schema to tenant scope
  keep information_schema disabled unless authorized
  never enable direct URL table access for untrusted users
```

```text id="shyd2b"
Catalog/provider:
  CatalogProvider filters schemas by user/tenant
  SchemaProvider filters tables by user/tenant
  TableProvider::schema() filters columns
  TableProvider::scan() enforces row/tenant policy
  TableProvider::statistics() returns redacted or no statistics
```

```text id="qm6w5z"
SQL control:
  use SQLOptions for DDL/DML/statement authorization
  deny CREATE EXTERNAL TABLE unless admin
  deny arbitrary LOCATION paths
  deny SHOW ALL / SHOW FUNCTIONS / information_schema unless policy allows
```

```text id="0me2k8"
Metadata:
  classify metadata keys
  redact secrets/source paths/signed URLs
  suppress sensitive row counts/min/max/partition values
  avoid sensitive table/column names in tenant-visible schemas
```

```text id="eis5fj"
Audit:
  log object refs, projected columns, filters, output row count, schema fingerprint
  log denied schema/metadata access
  redact SQL literals and source URIs
  separate user-facing denial messages from internal audit reasons
```

---

## S15.21 Anti-pattern inventory

```text id="h0vu2x"
Schema security anti-patterns:
  treating schema() as non-sensitive metadata
  registering raw and masked tables in same user context
  exposing hidden columns through information_schema
  hiding values but exposing sensitive column names
  tenant isolation only through WHERE clauses in user-editable views
  enabling enable_url_table in a service
  accepting user-provided external table LOCATION
  registering one object store with broad credentials for all tenants
  exposing information_schema.df_settings to users
  exposing SHOW FUNCTIONS in restricted environments
  returning exact row counts for sensitive tables
  exposing min/max statistics for confidential economics fields
  logging raw SQL with secrets/literals
  returning “exists but unauthorized” user messages
  declaring exact filter pushdown without semantics match
  pushing LIMIT below inexact security predicates
  exposing partition values that encode clients/sites/incidents
  storing secrets in schema metadata or Parquet key-value metadata
```

---

## S15.22 Agent checklist

```text id="b3r9d3"
[ ] Treat provider schema() as a security boundary.
[ ] Register only authorized catalogs/schemas/tables.
[ ] Use catalog per tenant when credentials or visibility differ.
[ ] Use schema per workspace/lifecycle layer when appropriate.
[ ] Disable information_schema unless authorized.
[ ] Filter or replace information_schema for tenant-facing metadata.
[ ] Never enable_url_table for untrusted SQL services.
[ ] Register object stores per credential/tenant URL prefix.
[ ] Disable DDL/DML/statements using SQLOptions for read-only users.
[ ] Deny arbitrary CREATE EXTERNAL TABLE LOCATION.
[ ] Hide unauthorized columns by removing them from schema().
[ ] Mask columns only if raw source is inaccessible.
[ ] Inject tenant predicates in provider scan, not only views.
[ ] Treat statistics as sensitive.
[ ] Treat partition values as potentially sensitive.
[ ] Redact metadata keys and source paths.
[ ] Audit projected columns, filters, output row counts, denied metadata access.
[ ] Use non-confirming denial messages.
[ ] Test cross-tenant catalog/table/column access.
```

## S15.23 Minimal governance control-plane skeleton

```rust id="5is7rz"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GovernanceContext {
    pub user_id: String,
    pub tenant_id: String,
    pub roles: Vec<String>,
    pub request_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GovernancePolicy {
    pub schema_visibility: SchemaVisibilityPolicy,
    pub sql_permissions: SqlPermissionPolicy,
    pub object_store_policy: ObjectStorePolicy,
    pub audit_policy: AuditPolicy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqlPermissionPolicy {
    pub allow_select: bool,
    pub allow_ddl: bool,
    pub allow_dml: bool,
    pub allow_statements: bool,
    pub allow_information_schema: bool,
    pub allow_show_functions: bool,
    pub allow_show_all: bool,
    pub allow_direct_url_tables: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectStorePolicy {
    pub allowed_url_prefixes: Vec<String>,
    pub deny_user_supplied_locations: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditPolicy {
    pub log_projected_columns: bool,
    pub log_filters: bool,
    pub log_output_row_count: bool,
    pub redact_sql_literals: bool,
    pub redact_source_locations: bool,
}
```

Core operating rule: **in a governed DataFusion platform, schema is data about data; table names, column names, metadata, statistics, partitions, and settings can be sensitive and must be filtered, redacted, audited, and tested like any other protected surface.**

[1]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html "TableProvider in datafusion::datasource - Rust"
[2]: https://datafusion.apache.org/user-guide/sql/information_schema.html "Information Schema — Apache DataFusion  documentation"
[3]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
