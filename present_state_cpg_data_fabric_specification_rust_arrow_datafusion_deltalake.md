# Present-State CPG Data Fabric Specification

**Status:** Draft normative implementation specification  
**Companion specification:** `present_state_cpg_fact_generation_specification_python_rust.md`  
**Primary implementation language:** Rust  
**Core data-plane technologies:** Apache Arrow Rust, Apache DataFusion Rust, and `deltalake` / delta-rs  
**Logical scope:** Present-state Python and Rust code-property-graph facts and mechanically derived facts  
**Excluded semantic scope:** Git/history analytics, runtime observation, test-impact assessment, refactor assessment, risk scoring, recommendations, and other evaluative conclusions

---

## 1. Purpose

This document specifies the complete data fabric that stores, transforms, derives, publishes, and serves the present-state CPG facts defined by the companion fact-generation specification.

The fabric SHALL provide:

- one canonical Arrow representation for every fact batch;
- one transactional Delta Lake persistence contract for every durable table;
- one DataFusion catalog and planning surface over the published current state;
- deterministic owner-scoped replacement of facts;
- cross-table publication consistency;
- typed relational schemas for all fact families;
- a universal graph projection for generic traversal;
- vectorized and streaming calculations for reconciliation and derived facts;
- high-performance storage layout, pruning, compaction, and query patterns;
- explicit schema, integrity, provenance, completeness, and unknown-state contracts.

The governing architecture is:

```text
Fact providers and derived analyzers
        ↓
Arrow RecordBatch streams
        ↓
Schema validation and normalization
        ↓
DataFusion reconciliation / derivation plans
        ↓
Delta Lake owner-scoped table updates
        ↓
Publication manifest pins exact Delta versions
        ↓
DataFusion current-state catalog
        ↓
LLM-agent fact queries
```

The data fabric SHALL stop at factual storage and factual calculation. It SHALL NOT encode conclusions such as `SAFE_TO_REFACTOR`, `TEST_IMPACTED`, `HIGH_RISK`, or `SHOULD_CHANGE`.

---

## 2. Source basis and version anchors

This specification is grounded in the attached references and uses their terminology and version posture.

| Technology | Version anchor used by this specification | Primary role |
|---|---:|---|
| Arrow Rust | `58.3.0` family | Canonical in-memory schemas, arrays, buffers, builders, `RecordBatch`, vectorized kernels, Parquet interchange |
| DataFusion Rust | `54.0.0` | Catalog, SQL/DataFrame/Expr planning, streaming execution, joins, aggregations, custom functions, custom logical/physical operators |
| `deltalake` / delta-rs | `1.0.0` at git rev `35cfed4545f41c2f483706d29670f7cc2fe7e217` | Transactional Delta tables, table schemas, DataFusion providers, writes, DML, constraints, optimize, vacuum |
| Parquet Rust | `58.3.0` | Physical data-file format beneath Delta Lake |
| `object_store` | `0.13.2` | Local and object-store I/O used by DataFusion and delta-rs |
| Rust toolchain | `1.91.1` for the pinned delta-rs baseline | Workspace compatibility floor |

The delta-rs `1.0.0` target is a pinned pre-release revision rather than a tagged stable release. All code generated from this specification SHALL be compile-tested against that exact revision before adoption.

### 2.1 Canonical workspace baseline

```toml
[workspace]
resolver = "2"

[workspace.package]
edition = "2024"
rust-version = "1.91.1"

[workspace.dependencies]
datafusion = "=54.0.0"

arrow = "=58.3.0"
arrow-array = "=58.3.0"
arrow-buffer = "=58.3.0"
arrow-schema = "=58.3.0"
arrow-cast = "=58.3.0"
arrow-select = "=58.3.0"
arrow-ord = "=58.3.0"
arrow-string = "=58.3.0"
arrow-row = "=58.3.0"

parquet = { version = "=58.3.0", features = ["arrow", "async", "object_store"] }
object_store = "=0.13.2"

deltalake = {
  git = "https://github.com/delta-io/delta-rs.git",
  rev = "35cfed4545f41c2f483706d29670f7cc2fe7e217",
  default-features = false,
  features = ["rustls", "datafusion", "s3"]
}

tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
futures = "0.3"
url = "2"
tracing = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
blake3 = "1"
```

Utility crates such as `blake3`, `serde`, `tokio`, and `futures` MAY be used inside the Rust implementation. The storage, batch, query, and relational-computation engines SHALL remain Arrow, DataFusion, and Delta Lake.

### 2.2 Version-alignment invariant

```text
one Arrow major/minor family
one Parquet family matching Arrow
one DataFusion family
one object_store family
one pinned delta-rs revision
```

CI SHALL reject duplicate Arrow, Parquet, DataFusion, or `object_store` versions that cross public type boundaries.

---

# Part I — Architectural Model

## 3. Technology responsibility model

### 3.1 Arrow responsibility

Arrow SHALL be the canonical in-memory and inter-component fact representation.

Arrow owns:

- canonical `DataType`, `Field`, and `Schema` contracts;
- typed array builders;
- null bitmaps;
- immutable `RecordBatch` publication units;
- zero-copy slicing and projection where possible;
- vectorized scalar kernels;
- batch streams between extractors, reconciler, DataFusion, and Delta writers;
- Parquet schema hints and round-trip fixtures.

Arrow SHALL NOT be treated as a graph database or durable table catalog.

### 3.2 DataFusion responsibility

DataFusion SHALL be the relational planning and execution engine.

DataFusion owns:

- the current-state query catalog;
- `DataFrame`, `Expr`, and `LogicalPlanBuilder` pipelines;
- schema binding and type coercion;
- projection, predicate, and limit pushdown;
- joins, aggregations, windows, sorting, and union;
- custom UDFs, UDAFs, UDTFs, logical nodes, and `ExecutionPlan`s;
- graph and dataflow calculations implemented as custom Rust operators;
- validation queries and publication integrity checks;
- streaming query results as `SendableRecordBatchStream`.

DataFusion SHALL NOT be treated as the durable source of truth.

### 3.3 Delta Lake responsibility

Delta Lake SHALL be the durable, transactional table-state authority.

Delta Lake owns:

- durable table schemas;
- atomic per-table commits;
- optimistic concurrency control;
- active Parquet file selection;
- append, delete, update, merge, and bounded overwrite operations;
- table constraints and metadata;
- exact table versions used by a publication;
- compaction, Z-order maintenance, and vacuum;
- object-store portability.

### 3.4 Canonical invariant

```text
Delta is the durable table-state authority.
DataFusion is the query and calculation engine.
Arrow is the batch and memory contract.
Parquet is the physical fact-file representation.
```

---

## 4. Present-state semantics and operational versioning

The semantic product exposes one present-state graph. Delta transaction history exists as a storage mechanism but SHALL NOT be exposed as a code-history ontology.

The fabric distinguishes:

```text
semantic history       excluded
transaction versions   required for atomic storage and recovery
```

Old Delta versions MAY exist temporarily for:

- multi-table publication consistency;
- failed-publication recovery;
- optimistic retry resolution;
- maintenance safety.

They SHALL NOT be presented to agents as prior code states unless a separate future history product is explicitly introduced.

---

## 5. Hybrid relational graph design

The canonical physical model SHALL be a **hybrid relational graph**:

1. a universal `entity` Delta table stores every graph node;
2. a universal `relation` Delta table stores every graph edge;
3. strongly typed extension tables store payloads that do not fit the common graph envelope;
4. control tables publish exact versions of all tables;
5. serving views combine generic topology with typed detail.

This design is mandatory because it provides:

- generic graph traversal without unioning dozens of tables;
- typed schemas without an EAV property store;
- predicate pushdown by entity/relation family;
- fast source/target joins;
- extension-table scans only when payload detail is requested;
- deterministic materialization into Arrow streams.

### 5.1 Explicitly prohibited canonical designs

The following SHALL NOT be canonical persistence models:

```text
one JSON blob per entity
one Map<String,String> property bag per fact
one EAV table for all properties
one serialized petgraph object
one opaque provider-native payload as the only representation
one table per individual relation kind
```

Cold provider evidence MAY use a compact map or binary payload, but canonical queryable fields SHALL remain typed columns.

---

## 6. Deployment topology

### 6.1 Default topology: one fabric namespace per repository/workspace

For maximum locality and minimal partition overhead, the default deployment SHALL allocate one Delta table namespace per analyzed repository or workspace.

```text
/cpg/<repository-id>/control/...
/cpg/<repository-id>/facts/...
/cpg/<repository-id>/derived/...
```

In this topology, hot fact tables do not need to repeat `repository_id` on every row because the table root already identifies the repository.

### 6.2 Shared-corpus topology

A shared multi-repository fabric MAY add:

```text
repository_id  BINARY(16)
repository_bucket SMALLINT
```

as leading columns and partitions. This is an optional deployment profile, not the default schema shown below.

### 6.3 Catalog namespaces

The DataFusion catalog SHALL expose:

```text
cpg_control   publication, owners, capability status, diagnostics
cpg_base      canonical entities, relations, and typed base-fact extensions
cpg_python    Python-specific extensions
cpg_rust      Rust/MIR-specific extensions
cpg_derived   graph/dataflow derived facts and summaries
cpg_serving   stable views and table functions for agent consumption
```

---

# Part II — Canonical Types, Identity, and Schema Contracts

## 7. Canonical physical types

The schema registry SHALL define the following reusable logical types.

| Logical type | Arrow type | Delta type | Invariant |
|---|---|---|---|
| `id16` | `Binary` | `BINARY` | exactly 16 bytes |
| `hash32` | `Binary` | `BINARY` | exactly 32 bytes |
| `code16` | `Int16` | `SHORT` | registered enum code |
| `code32` | `Int32` | `INTEGER` | registered enum code |
| `flags64` | `Int64` | `LONG` | bitset; no unsigned Delta dependency |
| `count64` | `Int64` | `LONG` | non-negative |
| `byte_offset` | `Int64` | `LONG` | non-negative byte position |
| `ordinal32` | `Int32` | `INTEGER` | ordered child/argument/edge position |
| `bucket16` | `Int16` | `SHORT` | `0..255` by default |
| `text` | `Utf8` | `STRING` | persisted strings are UTF-8 |
| `bytes` | `Binary` | `BINARY` | exact source/payload bytes |
| `timestamp_utc` | `Timestamp(Microsecond, UTC)` | `TIMESTAMP` | operational timestamps only |
| `id_list` | `List<Binary>` | `ARRAY<BINARY>` | sorted/deduplicated where specified |
| `string_map` | `Map<Utf8,Utf8>` | `MAP<STRING,STRING>` | cold metadata only |

### 7.1 ID encoding

All durable graph IDs SHALL be application-owned 128-bit values encoded as 16-byte binary.

```rust
#[repr(transparent)]
pub struct Id128(pub [u8; 16]);
```

ID derivation SHALL be deterministic and domain-separated.

```text
entity_id   = BLAKE3_128("entity" || semantic-key)
relation_id = BLAKE3_128("relation" || relation-kind || source || target || role || ordinal)
owner_id    = BLAKE3_128("owner" || owner-key)
type_id     = BLAKE3_128("type" || canonical-type-algebra)
```

The full 256-bit BLAKE3 digest MAY be retained as `hash32` for collision diagnostics and schema fingerprints.

### 7.2 Bucket derivation

Hot tables SHALL include `owner_bucket` and, for relation-heavy tables, `source_bucket` and `target_bucket`.

```text
bucket = first_byte(id16) as signed-safe SMALLINT
```

Buckets provide bounded Delta partitions and shall not be interpreted as semantic facts.

### 7.3 Signed hash columns

Where Z-ordering or file statistics over binary IDs are insufficient, tables MAY include a hidden `*_hash64` `Int64` column derived from the first eight digest bytes.

These columns are operational accelerators and SHALL be marked with schema metadata:

```text
com.codefabric.cpg.hidden_operational = true
```

---

## 8. Enum registries

Repeated categorical values SHALL use integer codes rather than persisted strings.

Required enum domains include:

```text
language
entity_family
entity_kind
relation_family
relation_kind
owner_kind
scope_kind
binding_kind
reference_kind
type_kind
type_role
call_dispatch_kind
cfg_node_kind
cfg_edge_kind
value_kind
operation_kind
dataflow_event_kind
memory_location_kind
memory_access_kind
alias_kind
state_kind
effect_kind
exception_relation_kind
resource_event_kind
async_relation_kind
certainty
resolution
producer
derivation
capability
capability_status
unknown_reason
```

The application SHALL register these as immutable Arrow `RecordBatch` dimensions in DataFusion. An optional Delta `enum_catalog` table MAY mirror them for external introspection.

Enum codes SHALL be append-only. Existing numeric meanings SHALL never be reassigned.

---

## 9. Common fact metadata

Canonical entities and relations SHALL inline the metadata needed by most queries.

Common fields:

```text
owner_id
owner_bucket
language
certainty
resolution
producer_code
derivation_code
file_id
start_byte
end_byte
flags
fact_hash64
```

Multiple provider observations of the same canonical fact SHALL be stored in `fact_evidence`, not duplicated as canonical rows.

---

## 10. Schema metadata conventions

Every Arrow schema SHALL carry:

```text
com.codefabric.cpg.table_name
com.codefabric.cpg.table_family
com.codefabric.cpg.table_grain
com.codefabric.cpg.schema_version
com.codefabric.cpg.ontology_version
com.codefabric.cpg.primary_key
com.codefabric.cpg.partition_columns
com.codefabric.cpg.owner_replacement_policy
com.codefabric.cpg.compatibility_mode
```

Important fields SHALL carry metadata such as:

```text
com.codefabric.cpg.semantic_type = id16 | hash32 | byte_offset | enum:<domain>
com.codefabric.cpg.primary_key_part = true
com.codefabric.cpg.foreign_key = <table>.<field>
com.codefabric.cpg.hidden_operational = true
com.codefabric.cpg.id_width = 16
```

Metadata is advisory unless consumed by explicit validation code. It SHALL NOT replace nullability, table constraints, or application integrity checks.

---

## 11. Schema registry

All schemas SHALL be defined once in a Rust schema registry.

```rust
pub struct TableSpec {
    pub table_code: i16,
    pub name: &'static str,
    pub schema_version: &'static str,
    pub arrow_schema: arrow_schema::SchemaRef,
    pub primary_key: &'static [&'static str],
    pub partition_columns: &'static [&'static str],
    pub zorder_columns: &'static [&'static str],
    pub owner_policy: OwnerReplacementPolicy,
    pub dependencies: &'static [i16],
    pub required_for_publication: bool,
}
```

The registry SHALL generate or validate:

- Arrow `SchemaRef`;
- Delta `StructType`;
- DataFusion `DFSchema` compatibility;
- primary-key metadata;
- Delta creation properties;
- builder capacity hints;
- schema fingerprints;
- table dependency order.

### 11.1 Schema round-trip gate

Every schema SHALL pass:

```text
Arrow Schema
  → Delta StructType
  → create empty Delta table
  → open Delta table
  → DataFusion TableProvider schema
  → Arrow Schema
  → exact contract comparison
```

---

# Part III — Multi-Table Publication and Snapshot Consistency

## 12. Publication model

Delta transactions are atomic per table, not across tables. The fabric SHALL therefore implement **manifest-pinned multi-table MVCC**.

### 12.1 Publication rule

A publication is a mapping:

```text
publication_id
  → exact Delta version for every required table
```

The current-state catalog SHALL open each table at the version pinned by the active publication.

### 12.2 Consequence

Intermediate Delta versions created while refreshing several tables are never visible through `cpg_serving` until the publication pointer is advanced.

This permits:

- delete-then-append owner replacement within one table;
- multi-table derived recomputation;
- retries after partial failure;
- cross-table schema and integrity validation before visibility.

### 12.3 No semantic-history guarantee

The publication mechanism is operational. Previous publications MAY be vacuumed once they are no longer required for active or recovery pointers.

---

## 13. Control-plane table schemas

### 13.1 `repository`

**Grain:** one row for the repository represented by the namespace.  
**Primary key:** `repository_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `repository_id` | `id16` | no | Stable repository identity |
| `canonical_name` | `Utf8` | no | Display/catalog name |
| `root_uri` | `Utf8` | no | Canonical source root URI |
| `source_snapshot_digest` | `hash32` | no | Digest of current source inventory |
| `language_mask` | `Int16` | no | Languages represented |
| `ontology_version` | `Utf8` | no | CPG ontology version |
| `schema_bundle_version` | `Utf8` | no | Fabric schema bundle |
| `created_at` | `timestamp_utc` | no | Operational creation time |

**Partitioning:** none.

### 13.2 `publication`

**Grain:** one staged or completed publication.  
**Primary key:** `publication_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `publication_id` | `id16` | no | Publication identity |
| `repository_id` | `id16` | no | Repository |
| `state_code` | `code16` | no | staging, validated, complete, failed |
| `source_snapshot_digest` | `hash32` | no | Source state represented |
| `base_fact_digest` | `hash32` | no | Canonical base-fact digest |
| `derived_fact_digest` | `hash32` | yes | Derived-fact digest |
| `ontology_version` | `Utf8` | no | Ontology version |
| `schema_bundle_version` | `Utf8` | no | Schema registry version |
| `derivation_bundle_version` | `Utf8` | no | Derived-analysis implementation version |
| `started_at` | `timestamp_utc` | no | Operational timestamp |
| `completed_at` | `timestamp_utc` | yes | Operational timestamp |
| `required_table_count` | `Int32` | no | Expected table-version rows |
| `published_table_count` | `Int32` | no | Completed table-version rows |
| `diagnostic_count` | `Int64` | no | Publication diagnostics |

**Partitioning:** none.  
**CDF:** disabled by default.

### 13.3 `publication_table`

**Grain:** one physical Delta table version in one publication.  
**Primary key:** `(publication_id, table_code)`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `publication_id` | `id16` | no | Publication |
| `table_code` | `code16` | no | Registry table code |
| `table_uri` | `Utf8` | no | Delta table root |
| `delta_version` | `Int64` | no | Exact version to open |
| `schema_fingerprint` | `hash32` | no | Exact Arrow/Delta contract fingerprint |
| `row_count` | `Int64` | no | Validated row count |
| `owner_count` | `Int64` | no | Distinct owner count where applicable |
| `table_checksum` | `hash32` | no | Deterministic row-set checksum |
| `required` | `Boolean` | no | Required for complete publication |
| `validated` | `Boolean` | no | Passed table checks |

**Partitioning:** none.

### 13.4 `current_publication`

**Grain:** one current pointer per repository.  
**Primary key:** `repository_id`.

| Column | Type | Null |
|---|---|---:|
| `repository_id` | `id16` | no |
| `publication_id` | `id16` | no |
| `updated_at` | `timestamp_utc` | no |
| `pointer_generation` | `Int64` | no |

This table SHALL be updated last.

### 13.5 `owner`

**Grain:** one deterministic fact owner.  
**Primary key:** `owner_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `owner_id` | `id16` | no | Owner identity |
| `parent_owner_id` | `id16` | yes | Hierarchical owner |
| `owner_bucket` | `bucket16` | no | Physical partition bucket |
| `owner_kind_code` | `code16` | no | file, module, callable, type, MIR body, crate |
| `language` | `code16` | no | Common/Python/Rust |
| `file_id` | `id16` | yes | Source file |
| `semantic_entity_id` | `id16` | yes | Owning declaration |
| `start_byte` | `Int64` | yes | Source start |
| `end_byte` | `Int64` | yes | Source end |
| `source_fingerprint` | `hash32` | yes | Source-sensitive owner fingerprint |
| `semantic_fingerprint` | `hash32` | yes | Normalized semantic fingerprint |
| `capability_mask` | `Int64` | no | Capability summary bits |
| `status_code` | `code16` | no | complete, partial, failed, stale-internal |

**Partitioning:** `owner_bucket`.

### 13.6 `capability_status`

**Grain:** one owner/capability status.  
**Primary key:** `(owner_id, capability_code)`.

| Column | Type | Null |
|---|---|---:|
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `capability_code` | `code16` | no |
| `status_code` | `code16` | no |
| `producer_code` | `code16` | no |
| `reason_code` | `code16` | yes |
| `diagnostic_id` | `id16` | yes |

**Partitioning:** `owner_bucket`.

### 13.7 `diagnostic`

**Grain:** one provider, reconciliation, validation, or derivation diagnostic.

| Column | Type | Null |
|---|---|---:|
| `diagnostic_id` | `id16` | no |
| `owner_id` | `id16` | yes |
| `owner_bucket` | `bucket16` | no |
| `producer_code` | `code16` | no |
| `severity_code` | `code16` | no |
| `diagnostic_code` | `code32` | no |
| `message` | `Utf8` | no |
| `file_id` | `id16` | yes |
| `start_byte` | `Int64` | yes |
| `end_byte` | `Int64` | yes |
| `details` | `Map<Utf8,Utf8>` | yes |

**Partitioning:** `owner_bucket`.

---

# Part IV — Universal Graph Tables

## 14. `entity`

**Grain:** one canonical CPG node of any family.  
**Primary key:** `entity_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `entity_id` | `id16` | no | Globally unique graph node |
| `owner_id` | `id16` | no | Replacement owner |
| `owner_bucket` | `bucket16` | no | Physical partition |
| `language` | `code16` | no | Common/Python/Rust |
| `entity_family_code` | `code16` | no | source, syntax, semantic, type, call, CFG, value, memory, generated, unknown, derived |
| `entity_kind_code` | `code32` | no | Canonical ontology kind |
| `raw_kind_code` | `code32` | yes | Provider-native normalized registry code |
| `file_id` | `id16` | yes | Source file |
| `start_byte` | `Int64` | yes | Source start |
| `end_byte` | `Int64` | yes | Source end |
| `name` | `Utf8` | yes | Unqualified name |
| `qualified_name` | `Utf8` | yes | Semantic qualified name |
| `parent_entity_id` | `id16` | yes | Immediate semantic/structural parent when meaningful |
| `type_id` | `id16` | yes | Primary computed type shortcut |
| `flags` | `Int64` | no | Canonical flags bitset |
| `certainty_code` | `code16` | no | Fact certainty |
| `resolution_code` | `code16` | no | Resolution state |
| `producer_code` | `code16` | no | Canonical authority producer |
| `derivation_code` | `code16` | yes | Derived-analysis identifier |
| `fact_hash64` | `Int64` | no | Equality/clustering accelerator |

**Partitioning:** `(entity_family_code, owner_bucket)`.  
**Z-order candidates:** `entity_id_hash64`, `parent_entity_id_hash64`, `file_id_hash64`.  
**Bloom-filter candidates:** `entity_id`, `parent_entity_id` after benchmarking.

### 14.1 Entity-family values

At minimum:

```text
SOURCE
SYNTAX
SEMANTIC
SCOPE_BINDING_REFERENCE
TYPE
CALLABLE_CALLSITE
CFG
VALUE_OPERATION
MEMORY
EXCEPTION_RESOURCE_ASYNC
GENERATED_LOWERED
PYTHON_EXTENSION
RUST_EXTENSION
UNKNOWN
DERIVED_COMPONENT
```

---

## 15. `relation`

**Grain:** one canonical directed CPG edge.  
**Primary key:** `relation_id`.

| Column | Type | Null | Meaning |
|---|---|---:|---|
| `relation_id` | `id16` | no | Edge identity |
| `owner_id` | `id16` | no | Replacement owner |
| `owner_bucket` | `bucket16` | no | Physical partition |
| `language` | `code16` | no | Common/Python/Rust |
| `relation_family_code` | `code16` | no | structural, type, call, CFG, dataflow, memory, etc. |
| `relation_kind_code` | `code32` | no | Canonical relation kind |
| `source_id` | `id16` | no | Source entity |
| `target_id` | `id16` | no | Target entity |
| `source_bucket` | `bucket16` | no | Source pruning bucket |
| `target_bucket` | `bucket16` | no | Target pruning bucket |
| `ordinal` | `Int32` | yes | Ordered relation position |
| `role_code` | `code16` | yes | AST field, operand role, parameter role, etc. |
| `distance` | `Int32` | yes | Derived graph distance when applicable |
| `direct` | `Boolean` | no | Direct versus transitive/summary relation |
| `file_id` | `id16` | yes | Source evidence file |
| `start_byte` | `Int64` | yes | Source evidence start |
| `end_byte` | `Int64` | yes | Source evidence end |
| `certainty_code` | `code16` | no | Certainty |
| `resolution_code` | `code16` | no | Resolution |
| `producer_code` | `code16` | no | Canonical producer |
| `derivation_code` | `code16` | yes | Derivation algorithm |
| `flags` | `Int64` | no | Edge flags |
| `fact_hash64` | `Int64` | no | Equality/clustering accelerator |

**Partitioning:** `(relation_family_code, owner_bucket)`.  
**Z-order candidates:** `source_hash64`, `target_hash64`, `relation_kind_code`.  
**Bloom-filter candidates:** `source_id`, `target_id` after benchmarking.

### 15.1 Relation-family values

```text
STRUCTURAL
SYMBOL_BINDING
MODULE_DEPENDENCY
TYPE
MEMBER
INVOCATION
CONTROL_FLOW
DATAFLOW
MEMORY_ALIAS
OWNERSHIP_LIFETIME
EFFECT
EXCEPTION
RESOURCE
ASYNC_CONCURRENCY
GENERATED_LOWERED
DERIVED_GRAPH
```

---

## 16. `fact_evidence`

**Grain:** one provider observation supporting or conflicting with a canonical entity/relation.

| Column | Type | Null |
|---|---|---:|
| `evidence_id` | `id16` | no |
| `fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `provider_code` | `code16` | no |
| `provider_version` | `Utf8` | no |
| `evidence_kind_code` | `code16` | no |
| `raw_kind_code` | `code32` | yes |
| `file_id` | `id16` | yes |
| `start_byte` | `Int64` | yes |
| `end_byte` | `Int64` | yes |
| `certainty_code` | `code16` | no |
| `payload` | `Map<Utf8,Utf8>` | yes |

**Partitioning:** `owner_bucket`.

This table is intentionally cold. Canonical serving queries SHALL not require it unless provenance is requested.

---

# Part V — Source, Syntax, and Semantic Extension Tables

## 17. `source_file`

**Grain:** one present-state source file.  
**Primary key:** `file_id`.

| Column | Type | Null |
|---|---|---:|
| `file_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `normalized_path` | `Utf8` | no |
| `language` | `code16` | no |
| `source_digest` | `hash32` | no |
| `byte_len` | `Int64` | no |
| `line_count` | `Int32` | no |
| `encoding_name` | `Utf8` | no |
| `newline_kind_code` | `code16` | no |
| `source_bytes` | `Binary` | no |
| `decoded_text` | `Utf8` | yes |
| `line_start_offsets` | `List<Int64>` | no |
| `module_entity_id` | `id16` | yes |
| `is_stub` | `Boolean` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

`source_bytes` is authoritative. `decoded_text` exists for fast DataFusion string slicing and search when decoding succeeded.

## 18. `source_token`

**Grain:** one lexical token.

| Column | Type | Null |
|---|---|---:|
| `token_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `file_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `token_kind_code` | `code32` | no |
| `start_byte` | `Int64` | no |
| `end_byte` | `Int64` | no |
| `normalized_value` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Token text SHALL normally be recovered from `source_file` to avoid duplication.

## 19. `source_annotation`

**Grain:** one comment, documentation item, directive, pragma, parse error, or missing-syntax record.

| Column | Type | Null |
|---|---|---:|
| `annotation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `file_id` | `id16` | no |
| `annotation_kind_code` | `code32` | no |
| `start_byte` | `Int64` | no |
| `end_byte` | `Int64` | no |
| `target_entity_id` | `id16` | yes |
| `text` | `Utf8` | yes |
| `diagnostic_code` | `code32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 20. `syntax_detail`

**Grain:** one syntax entity extension keyed by `entity_id`.

| Column | Type | Null |
|---|---|---:|
| `entity_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `raw_kind_code` | `code32` | no |
| `normalized_kind_code` | `code32` | no |
| `parent_syntax_id` | `id16` | yes |
| `field_role_code` | `code16` | yes |
| `ordinal` | `Int32` | yes |
| `named` | `Boolean` | no |
| `extra` | `Boolean` | no |
| `error` | `Boolean` | no |
| `missing` | `Boolean` | no |
| `explicitly_parenthesized` | `Boolean` | no |
| `provider_node_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

`AST_CHILD` relations SHALL be generated into `relation` from these parent/role/ordinal columns.

## 21. `semantic_detail`

**Grain:** one semantic declaration/symbol/member entity extension.

| Column | Type | Null |
|---|---|---:|
| `entity_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `semantic_kind_code` | `code32` | no |
| `visibility_code` | `code16` | yes |
| `mutability_code` | `code16` | yes |
| `declaration_syntax_id` | `id16` | yes |
| `name_span_start` | `Int64` | yes |
| `name_span_end` | `Int64` | yes |
| `signature_hash` | `hash32` | yes |
| `external` | `Boolean` | no |
| `generated` | `Boolean` | no |
| `synthesized` | `Boolean` | no |
| `modifiers` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 22. `scope_detail`

**Grain:** one scope entity extension.

| Column | Type | Null |
|---|---|---:|
| `scope_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_kind_code` | `code16` | no |
| `parent_scope_id` | `id16` | yes |
| `semantic_entity_id` | `id16` | yes |
| `file_id` | `id16` | yes |
| `start_byte` | `Int64` | yes |
| `end_byte` | `Int64` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 23. `binding_detail`

**Grain:** one binding entity extension.

| Column | Type | Null |
|---|---|---:|
| `binding_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_id` | `id16` | no |
| `bound_entity_id` | `id16` | yes |
| `binding_kind_code` | `code16` | no |
| `name` | `Utf8` | no |
| `definition_event_id` | `id16` | yes |
| `target_scope_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 24. `reference_detail`

**Grain:** one reference entity extension.

| Column | Type | Null |
|---|---|---:|
| `reference_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `scope_id` | `id16` | no |
| `reference_kind_code` | `code16` | no |
| `name` | `Utf8` | no |
| `resolved_entity_id` | `id16` | yes |
| `candidate_count` | `Int32` | no |
| `unknown_reason_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 25. `module_import_detail`

**Grain:** one import/use occurrence.

| Column | Type | Null |
|---|---|---:|
| `import_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `source_module_id` | `id16` | no |
| `target_module_id` | `id16` | yes |
| `imported_entity_id` | `id16` | yes |
| `local_binding_id` | `id16` | yes |
| `import_kind_code` | `code16` | no |
| `relative_level` | `Int16` | yes |
| `source_name` | `Utf8` | no |
| `alias_name` | `Utf8` | yes |
| `star_import` | `Boolean` | no |
| `unknown_reason_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

---

# Part VI — Types, Members, Calls, and Control Flow

## 26. `type_detail`

**Grain:** one canonical semantic type entity.

| Column | Type | Null |
|---|---|---:|
| `type_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `type_kind_code` | `code32` | no |
| `canonical_key` | `Utf8` | no |
| `display_name` | `Utf8` | yes |
| `primitive_code` | `code16` | yes |
| `nominal_entity_id` | `id16` | yes |
| `callable_entity_id` | `id16` | yes |
| `raw_shape_hash` | `hash32` | yes |
| `nullable_semantics_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `(type_kind_code, owner_bucket)`.

Type algebra components and relationships such as union members, generic arguments, parameters, bounds, coercions, and narrowing SHALL be rows in `relation` using the `TYPE` family.

## 27. `type_fact_detail`

**Grain:** one subject/type attribution relation extension.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `type_id` | `id16` | no |
| `type_role_code` | `code16` | no |
| `program_point_id` | `id16` | yes |
| `origin_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 28. `member_relation_detail`

**Grain:** one member/inheritance/implementation/override relation extension.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `declaring_type_id` | `id16` | yes |
| `member_entity_id` | `id16` | yes |
| `contract_member_id` | `id16` | yes |
| `receiver_type_id` | `id16` | yes |
| `resolution_kind_code` | `code16` | yes |
| `mro_position` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 29. `callable_detail`

**Grain:** one callable semantic entity extension.

| Column | Type | Null |
|---|---|---:|
| `callable_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `signature_id` | `id16` | yes |
| `return_type_id` | `id16` | yes |
| `parameter_count` | `Int32` | no |
| `generic_parameter_count` | `Int32` | no |
| `calling_convention_code` | `code16` | yes |
| `abi_name` | `Utf8` | yes |
| `callable_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 30. `parameter_detail`

**Grain:** one callable parameter.

| Column | Type | Null |
|---|---|---:|
| `parameter_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `name` | `Utf8` | yes |
| `parameter_kind_code` | `code16` | no |
| `type_id` | `id16` | yes |
| `default_syntax_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 31. `call_site_detail`

**Grain:** one call-site entity extension.

| Column | Type | Null |
|---|---|---:|
| `call_site_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `caller_id` | `id16` | no |
| `syntax_id` | `id16` | yes |
| `callee_syntax_id` | `id16` | yes |
| `receiver_value_id` | `id16` | yes |
| `result_value_id` | `id16` | yes |
| `dispatch_kind_code` | `code16` | no |
| `declared_target_id` | `id16` | yes |
| `resolved_target_count` | `Int32` | no |
| `call_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 32. `call_argument_detail`

**Grain:** one argument occurrence at one call site.

| Column | Type | Null |
|---|---|---:|
| `argument_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `call_site_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `keyword_name` | `Utf8` | yes |
| `argument_syntax_id` | `id16` | yes |
| `argument_value_id` | `id16` | yes |
| `parameter_id` | `id16` | yes |
| `binding_status_code` | `code16` | no |
| `spread_kind_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

## 33. `call_target_detail`

**Grain:** one target candidate for one call site.  
**Primary key:** `(call_site_id, target_id, target_instance_id, target_kind_code)`.

| Column | Type | Null |
|---|---|---:|
| `call_site_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `target_id` | `id16` | no |
| `target_instance_id` | `id16` | yes |
| `target_kind_code` | `code16` | no |
| `exact` | `Boolean` | no |
| `certainty_code` | `code16` | no |
| `evidence_relation_id` | `id16` | no |

**Partitioning:** `owner_bucket`.

## 34. `cfg_graph`

**Grain:** one control-flow graph.

| Column | Type | Null |
|---|---|---:|
| `cfg_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | yes |
| `cfg_kind_code` | `code16` | no |
| `entry_node_id` | `id16` | no |
| `exit_node_id` | `id16` | no |
| `exceptional_exit_node_id` | `id16` | yes |
| `node_count` | `Int32` | no |
| `edge_count` | `Int32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 35. `cfg_node_detail`

**Grain:** one CFG node entity extension.

| Column | Type | Null |
|---|---|---:|
| `cfg_node_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_id` | `id16` | no |
| `node_kind_code` | `code16` | no |
| `syntax_id` | `id16` | yes |
| `mir_statement_id` | `id16` | yes |
| `ordinal` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 36. `cfg_edge_detail`

**Grain:** one CFG relation extension keyed by the corresponding `relation_id`.

| Column | Type | Null |
|---|---|---:|
| `relation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_id` | `id16` | no |
| `condition_id` | `id16` | yes |
| `case_value_text` | `Utf8` | yes |
| `case_value_hash` | `Int64` | yes |
| `exception_type_id` | `id16` | yes |
| `edge_flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

---

# Part VII — Values, Dataflow, Memory, and State

## 37. `value_detail`

**Grain:** one value entity extension.

| Column | Type | Null |
|---|---|---:|
| `value_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `value_kind_code` | `code16` | no |
| `type_id` | `id16` | yes |
| `producer_operation_id` | `id16` | yes |
| `constant_value_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 38. `operation_detail`

**Grain:** one normalized computation operation.

| Column | Type | Null |
|---|---|---:|
| `operation_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `operation_kind_code` | `code32` | no |
| `result_value_id` | `id16` | yes |
| `type_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `raw_kind_code` | `code32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Operands SHALL be `relation` rows of kind `OPERAND` with `ordinal` and `role_code`.

## 39. `dataflow_event_detail`

**Grain:** one definition, use, read, write, move, copy, borrow, or related event.

| Column | Type | Null |
|---|---|---:|
| `event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `event_kind_code` | `code16` | no |
| `binding_id` | `id16` | yes |
| `value_id` | `id16` | yes |
| `location_id` | `id16` | yes |
| `syntax_id` | `id16` | yes |
| `ordinal` | `Int32` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Reaching-definitions, def-use, data-dependency, and value-flow outputs SHALL be `relation` rows in the `DATAFLOW` family.

## 40. `memory_location_detail`

**Grain:** one canonical abstract memory/access-path location.

| Column | Type | Null |
|---|---|---:|
| `location_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `location_kind_code` | `code16` | no |
| `base_entity_id` | `id16` | yes |
| `base_local_id` | `id16` | yes |
| `type_id` | `id16` | yes |
| `parent_location_id` | `id16` | yes |
| `projection_depth` | `Int16` | no |
| `canonical_path_hash` | `hash32` | no |
| `display_path` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 41. `access_path_component`

**Grain:** one ordered projection component of one memory location.

| Column | Type | Null |
|---|---|---:|
| `component_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `location_id` | `id16` | no |
| `ordinal` | `Int16` | no |
| `projection_kind_code` | `code16` | no |
| `field_entity_id` | `id16` | yes |
| `index_value_id` | `id16` | yes |
| `variant_entity_id` | `id16` | yes |
| `constant_index` | `Int64` | yes |
| `subslice_from` | `Int64` | yes |
| `subslice_to` | `Int64` | yes |

**Partitioning:** `owner_bucket`.

## 42. `memory_access_detail`

**Grain:** one access event over one location.

| Column | Type | Null |
|---|---|---:|
| `access_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `location_id` | `id16` | no |
| `value_id` | `id16` | yes |
| `access_kind_code` | `code16` | no |
| `program_point_id` | `id16` | yes |
| `certainty_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Alias and points-to facts SHALL be `relation` rows in the `MEMORY_ALIAS` family. `alias_relation_detail` MAY store program-point and analysis-domain payloads when needed.

## 43. `program_state_detail`

**Grain:** one objective state fact for one subject at one program point.

| Column | Type | Null |
|---|---|---:|
| `state_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `program_point_id` | `id16` | no |
| `state_kind_code` | `code16` | no |
| `state_value_code` | `code16` | no |
| `related_id` | `id16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

---

# Part VIII — Effects, Exceptions, Resources, Async, and Generated Semantics

## 44. `effect_detail`

**Grain:** one direct or transitive callable effect.

| Column | Type | Null |
|---|---|---:|
| `effect_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `callable_id` | `id16` | no |
| `effect_kind_code` | `code16` | no |
| `direct` | `Boolean` | no |
| `target_id` | `id16` | yes |
| `source_call_site_id` | `id16` | yes |
| `certainty_code` | `code16` | no |
| `unknown` | `Boolean` | no |
| `model_pack_code` | `code16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `(effect_kind_code, owner_bucket)`.

The corresponding semantic relation SHALL also exist in `relation` when the effect has a target entity/location.

## 45. `exception_detail`

**Grain:** one raise/panic/assert/handler/unwind semantic event or relation payload.

| Column | Type | Null |
|---|---|---:|
| `exception_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `site_id` | `id16` | no |
| `cfg_node_id` | `id16` | yes |
| `exception_kind_code` | `code16` | no |
| `exception_type_id` | `id16` | yes |
| `handler_id` | `id16` | yes |
| `relation_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 46. `resource_event_detail`

**Grain:** one resource lifecycle event.

| Column | Type | Null |
|---|---|---:|
| `resource_event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `resource_kind_code` | `code16` | no |
| `resource_id` | `id16` | yes |
| `location_id` | `id16` | yes |
| `event_kind_code` | `code16` | no |
| `transfer_target_id` | `id16` | yes |
| `model_pack_code` | `code16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 47. `async_event_detail`

**Grain:** one async/concurrency relation payload.

| Column | Type | Null |
|---|---|---:|
| `async_event_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `cfg_node_id` | `id16` | yes |
| `concurrency_kind_code` | `code16` | no |
| `subject_id` | `id16` | no |
| `target_id` | `id16` | yes |
| `relation_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |
| `model_pack_code` | `code16` | yes |

**Partitioning:** `owner_bucket`.

## 48. `capture_detail`

**Grain:** one closure-capture fact.

| Column | Type | Null |
|---|---|---:|
| `capture_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `closure_id` | `id16` | no |
| `captured_entity_id` | `id16` | yes |
| `captured_location_id` | `id16` | yes |
| `source_scope_id` | `id16` | yes |
| `capture_kind_code` | `code16` | no |
| `ordinal` | `Int32` | yes |

**Partitioning:** `owner_bucket`.

## 49. `generated_detail`

**Grain:** one generated/lowered entity or expansion record.

| Column | Type | Null |
|---|---|---:|
| `generated_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `generated_kind_code` | `code16` | no |
| `source_entity_id` | `id16` | yes |
| `source_syntax_id` | `id16` | yes |
| `expansion_id` | `id16` | yes |
| `generation_depth` | `Int16` | yes |
| `provenance_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Generated/lowered relationships SHALL also be canonical rows in `relation`.

---

# Part IX — Python and Rust Extension Tables

## 50. `python_dynamic_detail`

**Grain:** one Python dynamic-semantics observation.

| Column | Type | Null |
|---|---|---:|
| `dynamic_fact_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `dynamic_kind_code` | `code16` | no |
| `target_name` | `Utf8` | yes |
| `target_value_id` | `id16` | yes |
| `unknown_entity_id` | `id16` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

This table covers objective facts such as use of `eval`, `exec`, dynamic imports, `getattr`, `setattr`, `__dict__`, star imports, monkey-patch writes, and dynamic attribute writes.

## 51. `rust_mir_body`

**Grain:** one MIR body.

| Column | Type | Null |
|---|---|---:|
| `body_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `definition_entity_id` | `id16` | no |
| `mir_phase_code` | `code16` | no |
| `return_local_id` | `id16` | no |
| `argument_count` | `Int32` | no |
| `local_count` | `Int32` | no |
| `basic_block_count` | `Int32` | no |
| `source_span_start` | `Int64` | yes |
| `source_span_end` | `Int64` | yes |
| `mir_fingerprint` | `hash32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 52. `rust_mir_local`

**Grain:** one MIR local.

| Column | Type | Null |
|---|---|---:|
| `local_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `ordinal` | `Int32` | no |
| `local_kind_code` | `code16` | no |
| `debug_name` | `Utf8` | yes |
| `type_id` | `id16` | no |
| `mutability_code` | `code16` | no |
| `source_start` | `Int64` | yes |
| `source_end` | `Int64` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 53. `rust_instance`

**Grain:** one concrete executable Rust instance.

| Column | Type | Null |
|---|---|---:|
| `instance_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `definition_entity_id` | `id16` | no |
| `instance_kind_code` | `code16` | no |
| `body_id` | `id16` | yes |
| `abi_name` | `Utf8` | yes |
| `mangled_name` | `Utf8` | yes |
| `generic_argument_count` | `Int32` | no |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

Generic arguments are `relation` rows from instance to type/lifetime/const argument entities with ordinals.

## 54. `rust_loan`

**Grain:** one compiler-exposed or conservatively derived loan.

| Column | Type | Null |
|---|---|---:|
| `loan_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `place_id` | `id16` | no |
| `loan_kind_code` | `code16` | no |
| `created_at_node_id` | `id16` | no |
| `region_id` | `id16` | yes |
| `borrowed_type_id` | `id16` | yes |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 55. `rust_region`

**Grain:** one Rust region/lifetime entity extension.

| Column | Type | Null |
|---|---|---:|
| `region_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | yes |
| `region_kind_code` | `code16` | no |
| `display_name` | `Utf8` | yes |
| `flags` | `Int64` | no |

**Partitioning:** `owner_bucket`.

## 56. `rust_move_path`

**Grain:** one move-path node.

| Column | Type | Null |
|---|---|---:|
| `move_path_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `body_id` | `id16` | no |
| `place_id` | `id16` | no |
| `parent_move_path_id` | `id16` | yes |
| `ordinal` | `Int32` | no |

**Partitioning:** `owner_bucket`.

## 57. `rust_vtable_entry`

**Grain:** one vtable entry candidate.

| Column | Type | Null |
|---|---|---:|
| `vtable_entry_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `vtable_id` | `id16` | no |
| `dyn_type_id` | `id16` | no |
| `concrete_type_id` | `id16` | yes |
| `ordinal` | `Int32` | no |
| `target_instance_id` | `id16` | yes |
| `entry_kind_code` | `code16` | no |
| `certainty_code` | `code16` | no |

**Partitioning:** `owner_bucket`.

## 58. `rust_macro_expansion`

**Grain:** one Rust macro expansion.

| Column | Type | Null |
|---|---|---:|
| `expansion_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `invocation_syntax_id` | `id16` | no |
| `macro_definition_id` | `id16` | yes |
| `expansion_depth` | `Int16` | no |
| `callsite_file_id` | `id16` | yes |
| `callsite_start` | `Int64` | yes |
| `callsite_end` | `Int64` | yes |
| `defsite_file_id` | `id16` | yes |
| `defsite_start` | `Int64` | yes |
| `defsite_end` | `Int64` | yes |
| `hygiene_context_hash` | `Int64` | yes |

**Partitioning:** `owner_bucket`.

---

# Part X — Unknowns, Derived Components, Metrics, and Summaries

## 59. `unknown_detail`

**Grain:** one explicit unknown entity.

| Column | Type | Null |
|---|---|---:|
| `unknown_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `unknown_kind_code` | `code16` | no |
| `subject_id` | `id16` | yes |
| `expected_relation_kind_code` | `code32` | yes |
| `reason_code` | `code16` | no |
| `provider_code` | `code16` | yes |
| `diagnostic_id` | `id16` | yes |
| `detail` | `Utf8` | yes |

**Partitioning:** `owner_bucket`.

Unknown nodes SHALL also exist in `entity`; edges to them SHALL exist in `relation`.

## 60. `derived_component`

**Grain:** one SCC, connected component, loop, recursive set, or other graph component.

| Column | Type | Null |
|---|---|---:|
| `component_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `projection_code` | `code16` | no |
| `component_kind_code` | `code16` | no |
| `size` | `Int64` | no |
| `header_or_root_id` | `id16` | yes |
| `recursive` | `Boolean` | no |
| `nesting_depth` | `Int32` | yes |
| `derivation_code` | `code16` | no |

**Partitioning:** `(projection_code, owner_bucket)`.

Membership SHALL be `relation` rows in the `DERIVED_GRAPH` family.

## 61. `metric`

**Grain:** one scalar objective metric for one subject.

| Column | Type | Null |
|---|---|---:|
| `metric_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `subject_id` | `id16` | no |
| `metric_code` | `code16` | no |
| `int_value` | `Int64` | yes |
| `float_value` | `Float64` | yes |
| `derivation_code` | `code16` | no |
| `flags` | `Int64` | no |

**Partitioning:** `(metric_code, owner_bucket)`.

Only objective measurements are permitted.

## 62. `callable_summary`

**Grain:** one scalar summary row per callable or Rust instance.

| Column | Type | Null |
|---|---|---:|
| `callable_id` | `id16` | no |
| `owner_id` | `id16` | no |
| `owner_bucket` | `bucket16` | no |
| `instance_id` | `id16` | yes |
| `direct_callee_count` | `Int64` | no |
| `may_callee_count` | `Int64` | no |
| `direct_read_count` | `Int64` | no |
| `transitive_read_count` | `Int64` | no |
| `direct_write_count` | `Int64` | no |
| `transitive_write_count` | `Int64` | no |
| `return_type_count` | `Int64` | no |
| `summary_flags` | `Int64` | no |
| `unknown_effect` | `Boolean` | no |
| `derivation_code` | `code16` | no |
| `summary_fingerprint` | `hash32` | no |

**Partitioning:** `owner_bucket`.

Actual read/write/call/effect member sets SHALL remain typed relations/effect rows rather than opaque lists. Optional cached `id_list` columns MAY be added only as a serving accelerator.

---

# Part XI — Arrow Ingestion and Batch Construction

## 63. Provider-to-Arrow contract

Every provider and derived analysis SHALL emit table-specific `RecordBatch` streams.

```rust
pub trait FactTableEncoder<T> {
    fn table_code(&self) -> i16;
    fn schema(&self) -> arrow_schema::SchemaRef;
    fn encode<I>(&self, rows: I) -> Result<Vec<arrow_array::RecordBatch>, EncodeError>
    where
        I: IntoIterator<Item = T>;
}
```

Provider-native types SHALL be normalized before reaching the table encoders.

## 64. Batch-size policy

Starting values:

```text
small/wide extension tables      16,384 rows per RecordBatch
normal fact tables               65,536 rows per RecordBatch
narrow relation/event tables    131,072 rows per RecordBatch
source_file                       bounded by file count, not bytes
```

Batch size SHALL be benchmarked against:

- Arrow builder allocation;
- DataFusion batch overhead;
- Parquet row-group formation;
- owner-local replacement size;
- memory pool limits.

## 65. Builder policy

Hot-path encoding SHALL use typed Arrow builders and preallocation.

```text
PrimitiveBuilder::with_capacity
BinaryBuilder::with_capacity
StringBuilder::with_capacity
ListBuilder with child capacity
StructArray construction from typed child arrays
```

Serde row conversion SHALL NOT be the primary high-volume path.

### 65.1 Null policy

Null SHALL mean semantically unavailable or inapplicable, not merely “not populated by this producer.”

Missing provider evidence SHALL usually be represented through:

- certainty/resolution codes;
- capability status;
- explicit unknown entities;
- fact evidence.

### 65.2 String policy

Persisted schemas SHALL use `Utf8`, not `Utf8View`, because Delta/Parquet table contracts must remain stable. DataFusion MAY use view types internally during query execution.

### 65.3 Dictionary policy

Repeated strings MAY be dictionary-encoded in transient Arrow batches, but durable Delta schemas SHALL remain semantic `STRING` columns. Parquet writer dictionary encoding SHALL be preferred over making dictionary types part of the table contract.

### 65.4 Nested-type policy

`Struct`, `List`, and `Map` SHALL be used for bounded, cohesive payloads such as:

- line offsets;
- cold diagnostics metadata;
- optional cached summary sets.

Core graph adjacency, type components, arguments, access-path components, and provider evidence SHALL remain row-oriented relations for pushdown and joins.

---

## 66. Batch validation

Every batch SHALL pass before entering DataFusion or Delta:

```text
schema exact match
column count match
row count equal across arrays
non-null key enforcement
id length enforcement
bucket derivation check
source span bounds
start <= end
registered enum codes
owner_id present
no duplicate primary key within batch
```

Validation SHALL use Arrow kernels where possible and custom vectorized validators otherwise.

---

# Part XII — Delta Table Creation and Write Operations

## 67. Delta table creation

Each table SHALL be created from the schema registry using validated `StructType::try_new(...)` schemas.

Creation SHALL set:

- table comment/description;
- schema version metadata;
- ontology version metadata;
- partition columns;
- target-file-size property where supported;
- table constraints;
- log/checkpoint retention policy;
- CDF disabled by default.

### 67.1 Column mapping

Default:

```text
delta.columnMapping.mode = none
```

Column mapping SHALL not be enabled unless all reader, writer, DML, CDF, optimize, and schema-evolution paths are compatibility-certified.

### 67.2 Type widening

Delta type widening SHALL be disabled by default. Schema migrations SHALL be explicit and tested across Arrow, DataFusion, and Delta.

---

## 68. Table mutation classes

Each table SHALL be assigned one mutation class.

| Class | Tables | Default operation |
|---|---|---|
| Static dimension | enum catalogs, provider registry | append-only / replace at bootstrap |
| Current singleton | repository, current_publication | merge/upsert one row |
| Owner-replaced fact | entity, relation, almost all extensions | delete owner rows then append replacement |
| Publication append | publication, publication_table | append then pointer update |
| Derived owner-replaced | metrics, summaries, owner-local derived facts | delete owner rows then append |
| Global derived replacement | global call SCC / closure if materialized | bounded table overwrite before publication |

Full-table overwrite SHALL be limited to initial bootstrap, controlled schema migration, or small global derived tables.

---

## 69. Owner replacement protocol

The default owner replacement for one physical table is:

```text
1. Open table at latest writable state.
2. Delete rows whose owner_id is in the replacement owner set.
3. Append validated replacement RecordBatch stream.
4. Reload table state.
5. Validate owner row counts/checksum.
6. Record final Delta version in publication_table.
```

The delete and append are separate Delta commits. This is safe because the publication pointer continues to reference the old table version until the complete new publication is validated.

### 69.1 MERGE optimization

`DeltaTable::merge` MAY replace delete+append when:

- the table has a stable primary key;
- source cardinality is large enough to justify merge planning;
- delete-not-matched semantics are verified for the pinned delta-rs API;
- execution plans and DML metrics are regression-tested.

Delete+append remains the normative baseline because its visibility is controlled by the publication manifest.

### 69.2 Removed owners

An owner removed from the current source SHALL be represented by deleting all rows for that `owner_id` from every owner-scoped table and omitting it from the replacement batches.

---

## 70. Idempotency and retry

Every write operation SHALL carry:

```text
publication_id
operation_id
table_code
owner-set fingerprint
input checksum
```

in Delta commit metadata where supported.

Retry logic SHALL:

1. reload latest table state;
2. inspect whether the operation was already committed;
3. validate expected rows/checksum;
4. retry only when the prior outcome is known not to have committed.

Blind append retry is prohibited.

---

## 71. Multi-table publication algorithm

```text
A. Create publication row in STAGING state.
B. Determine changed owners and affected table families.
C. Encode and validate Arrow batches.
D. Update base fact tables.
E. Run DataFusion reconciliation.
F. Update canonical entity/relation tables and extension tables.
G. Compute owner-local derived facts.
H. Compute affected interprocedural/global derived facts.
I. Run cross-table integrity queries.
J. Append publication_table rows with exact Delta versions.
K. Mark publication COMPLETE.
L. Atomically update current_publication pointer.
M. Rebuild or refresh the serving catalog.
```

A failed publication SHALL never update `current_publication`.

---

# Part XIII — DataFusion Reconciliation and Normalization

## 72. Internal planning policy

Internal pipelines SHALL prefer:

```text
DataFrame + Expr
LogicalPlanBuilder
custom logical nodes
```

over generated SQL strings.

SQL MAY be used for diagnostics, testing, and stable serving views.

All computed expressions SHALL have deterministic aliases.

---

## 73. Reconciliation plan families

### 73.1 Source-range reconciliation

DataFusion joins provider observations using:

```text
file_id
range overlap or exact range
normalized kind
parent role
owner
```

The highest-authority observation becomes canonical; all others become `fact_evidence`.

### 73.2 Declaration reconciliation

```text
syntax declaration candidate
  JOIN local semantic binding
  LEFT JOIN project/compiler semantic declaration
  GROUP BY canonical semantic key
  → semantic entity
```

### 73.3 Type reconciliation

```text
declared type
computed type
expected type
flow-narrowed type
```

remain separate type-fact relations. Canonical type nodes are deduplicated by `type_id`.

### 73.4 Call-target reconciliation

Exact targets, may-targets, declaration targets, and unknown targets SHALL remain distinct rows. No aggregate shall collapse them into one unqualified `CALLS` relationship.

### 73.5 Unknown materialization

A DataFusion anti-join SHALL identify required semantic relationships with no resolved target and generate explicit unknown entities and relations according to the companion specification.

---

## 74. Canonical deduplication

Deduplication SHALL use deterministic primary keys and `row_number()` over authority order where multiple canonical candidates remain.

Authority ordering SHALL be encoded as integer rank and SHALL be stable across releases unless the ontology version changes.

Canonical dedupe plans SHALL sort only when required; hash aggregation and partition-local dedupe are preferred.

---

## 75. Integrity validation queries

Before publication, DataFusion SHALL verify at minimum:

```text
primary-key uniqueness for every table
entity IDs are 16 bytes
relation source and target exist in entity
owner IDs exist in owner
file spans lie within source_file.byte_len
start_byte <= end_byte
syntax parent is a syntax entity
call target points to callable/instance/unknown target
CFG edge endpoints belong to same cfg_id
CFG entry/exit nodes exist
dataflow events refer to existing CFG/value/location entities
access-path ordinals are contiguous per location
type relations point to type entities where required
unknown relations point to matching unknown kinds
summary derivation version matches publication derivation bundle
publication row counts match table scans
```

Foreign-key-like checks are application-enforced; Delta does not substitute for these joins.

---

# Part XIV — Calculations and Derived-Fact Execution

## 76. Calculation-placement policy

Use the highest-level DataFusion surface that preserves optimizer visibility.

```text
built-in Expr / aggregate
  before custom UDF

UDF / UDAF / UDTF
  before custom logical/physical operator

custom physical operator
  only for graph/fixed-point algorithms not naturally relational
```

---

## 77. Arrow kernel catalog

The fabric SHALL implement vectorized Arrow kernels for:

```text
validate_id16(binary) -> boolean
id_bucket(binary) -> int16
id_hash64(binary) -> int64
id_to_hex(binary) -> utf8
span_length(start, end) -> int64
validate_span(start, end, file_len) -> boolean
flags_has(flags, mask) -> boolean
flags_or(array<int64>) -> int64
canonical_path_hash(base, projections...) -> binary32
fact_row_hash(selected columns...) -> int64
fact_checksum_update(batch) -> binary state
sorted_unique_id_list(list<binary>) -> list<binary>
```

Kernels SHALL operate on arrays and preserve null semantics explicitly.

---

## 78. DataFusion scalar UDFs

Recommended registered UDFs:

| Function | Signature | Purpose |
|---|---|---|
| `cpg_id_bucket` | `BINARY -> SMALLINT` | Add mandatory bucket filter |
| `cpg_id_hash64` | `BINARY -> BIGINT` | Z-order/statistics accelerator |
| `cpg_id_hex` | `BINARY -> STRING` | Human-readable output |
| `cpg_span_len` | `(BIGINT,BIGINT) -> BIGINT` | Span metric |
| `cpg_flags_has` | `(BIGINT,BIGINT) -> BOOLEAN` | Flag filtering |
| `cpg_source_slice` | `(BINARY,BIGINT,BIGINT) -> STRING/BINARY` | Exact source snippet |
| `cpg_relation_family_name` | `SMALLINT -> STRING` | Serving display |
| `cpg_entity_kind_name` | `INTEGER -> STRING` | Serving display |

These UDFs SHALL be immutable and deterministic.

---

## 79. DataFusion aggregate UDFs

Recommended UDAFs:

### 79.1 `cpg_id_set_union`

```text
input: BINARY or LIST<BINARY>
state: sorted/deduplicated LIST<BINARY>
output: LIST<BINARY>
```

Used for bounded summaries and fixed-point propagation.

### 79.2 `cpg_fact_checksum`

```text
input: deterministic row hash
state: order-independent multiset checksum + row count
output: BINARY(32)
```

Used for table and owner validation.

### 79.3 `cpg_flags_or`

Bitwise union of effect and summary flags.

UDAF states SHALL be mergeable, deterministic, serializable in Arrow, and memory-accounted.

---

## 80. Relationally expressible derived facts

The following SHALL use ordinary DataFusion plans:

```text
direct caller/callee projections
direct in/out degree
unique callee/caller counts
entity/relation family counts
owner fact counts
branch/return/read/write counts
cyclomatic complexity from CFG nodes/edges
unknown counts
exact-vs-may target counts
summary scalar flags and counts
source span lengths
member/type/call lookup views
```

Example cyclomatic calculation:

```text
M = E - N + 2P
```

where `E` is CFG edge count, `N` is CFG node count, and `P` is connected-component count for the selected CFG policy.

---

## 81. Custom logical operators

The fabric SHALL define application-owned logical nodes for nontrivial graph computations.

```text
CpgGraphTraverse
CpgStrongComponents
CpgDominators
CpgPostDominators
CpgControlDependence
CpgNaturalLoops
CpgReachingDefinitions
CpgLiveness
CpgPointsTo
CpgSummaryFixpoint
```

Each logical node SHALL expose:

- input plans;
- input expressions;
- deterministic output schema;
- graph scope keys;
- relation-kind filters;
- certainty policy;
- maximum depth/iteration policy;
- display/EXPLAIN representation.

---

## 82. Custom physical graph representation

Inside graph execution operators, global `id16` values SHALL be mapped to dense local `u32` indexes per graph scope.

Recommended in-memory CSR representation:

```text
node_ids:          BinaryArray / FixedSizeBinary-compatible temporary buffer
row_offsets:       UInt64Buffer, length N + 1
neighbors:         UInt32Buffer, length E
edge_kind:         Int32Array, length E
edge_fact_ids:     BinaryArray, length E
```

This representation SHALL be built directly from sorted Arrow edge batches.

Petgraph is not required inside the data fabric. The operator may implement algorithms directly over Arrow-owned CSR buffers while still conforming to DataFusion `ExecutionPlan` contracts.

---

## 83. Reachability and graph traversal

### 83.1 Query-time traversal

`CpgGraphTraverseExec` SHALL support:

```text
seed IDs
relation-family/kind mask
direction: outgoing | incoming | both
maximum depth
maximum output rows
optional path predecessor output
certainty policy: exact-only | include-may
```

Output schema:

| Column | Type |
|---|---|
| `seed_id` | `id16` |
| `node_id` | `id16` |
| `depth` | `Int32` |
| `predecessor_id` | `id16` nullable |
| `via_relation_id` | `id16` nullable |
| `path_certainty_code` | `code16` |

### 83.2 Materialized reachability

Transitive closure SHALL only be materialized when bounded and repeatedly useful, such as:

- owner-local CFG reachability;
- call SCC condensation DAG reachability;
- small module dependency graphs.

Unbounded whole-graph closure is prohibited as a default table because of quadratic amplification.

---

## 84. SCC and recursion calculation

`CpgStrongComponentsExec` SHALL implement Tarjan or Kosaraju over CSR partitions.

Inputs:

```text
graph_scope_id
source_id
target_id
selected relation kinds
```

Outputs:

- `derived_component` rows;
- `relation` membership rows;
- recursive flags;
- component size metrics;
- condensed DAG edges when requested.

Global call-graph SCC computation SHALL use exact edges and may-edge variants as separate projection codes.

---

## 85. Dominator and post-dominator calculation

CFGs SHALL be grouped by `cfg_id` and computed owner-locally.

Outputs:

```text
IMMEDIATE_DOMINATOR
DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_POST_DOMINATOR
POST_DOMINATES
```

Post-dominators SHALL use a synthetic exit when the CFG has multiple exits. Normal and unwind policies SHALL be selectable and encoded in `projection_code`.

Only immediate dominator edges are mandatory to materialize. Full dominance closure MAY be query-time or materialized for small CFGs.

---

## 86. Control dependence and loop calculation

Control dependence SHALL be derived from post-dominator frontiers.

Natural loops SHALL be derived from back edges whose targets dominate their sources. Irreducible loops SHALL use SCC fallback and explicit loop-kind codes.

Outputs:

```text
BACK_EDGE
LOOP_MEMBER
LOOP_HEADER
CONTROL_DEPENDENT_ON
loop nesting-depth metrics
```

---

## 87. Reaching definitions and liveness

`CpgReachingDefinitionsExec` and `CpgLivenessExec` SHALL use dense bitsets over owner-local definitions/variables.

### 87.1 Reaching definitions

```text
IN[b]  = union OUT[p] for predecessors p
OUT[b] = GEN[b] union (IN[b] - KILL[b])
```

Outputs:

```text
REACHES
DEF_USE
DATA_DEP
```

Alias-aware kill rules SHALL be selected by analysis precision profile.

### 87.2 Liveness

```text
OUT[b] = union IN[s] for successors s
IN[b]  = USE[b] union (OUT[b] - DEF[b])
```

Outputs SHALL be program-point state rows or relations, not opaque bitsets.

Bitsets are internal execution state only.

---

## 88. Points-to and alias fixed point

`CpgPointsToExec` SHALL consume normalized constraints such as:

```text
address/reference creation
assignment/copy/move flow
field projection
load/store
argument-to-parameter flow
return flow
call target constraints
```

It SHALL iterate to fixed point per configured analysis domain.

Outputs:

```text
POINTS_TO
MAY_POINT_TO
MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS only when proven
```

Unknown memory SHALL be propagated explicitly rather than discarded.

---

## 89. Interprocedural summary fixed point

`CpgSummaryFixpointExec` SHALL:

1. build a selected call projection;
2. compute call SCCs;
3. condense to a DAG;
4. process SCCs in reverse topological order;
5. iterate recursive SCC members until summary stabilization;
6. union direct reads, writes, calls, effects, returns, and unknown flags;
7. emit transitive `effect_detail`, summary relations, and `callable_summary` rows.

Exact-only and exact-plus-may summaries SHALL be separate derivation profiles.

Unknown call targets SHALL set `unknown_effect = true` and prevent claims of a closed effect set.

---

## 90. Custom-operator execution requirements

Every custom `ExecutionPlan` SHALL:

- report a correct Arrow schema;
- expose correct `PlanProperties`;
- preserve partitioning/order claims conservatively;
- use DataFusion memory reservations;
- support cancellation;
- stream `RecordBatch` output;
- avoid unbounded output without explicit caps;
- expose metrics;
- spill or reject when memory limits are exceeded;
- include deterministic EXPLAIN formatting;
- have plan-property and execution golden tests.

---

# Part XV — Serving Catalog and Query Surface

## 91. Publication-pinned catalog provider

A custom `CatalogProvider` SHALL:

1. read `current_publication`;
2. load its `publication_table` rows;
3. open each Delta table at the exact pinned version;
4. build DataFusion `TableProvider`s;
5. register stable schema namespaces;
6. expose one immutable catalog snapshot to a query session.

Query sessions SHALL never mix table versions from different publications.

### 91.1 Provider wrapping

`PinnedDeltaTableProvider` MAY wrap delta-rs providers to add:

- exact row counts from publication metadata;
- primary/unique constraints from the schema registry;
- automatic bucket predicate injection for `entity_id`, `owner_id`, `source_id`, and `target_id` equality filters;
- hidden operational-column projection removal;
- stable table descriptions.

---

## 92. Stable serving views

The catalog SHALL expose at least:

```text
cpg_serving.entities
cpg_serving.relations
cpg_serving.files
cpg_serving.syntax
cpg_serving.symbols
cpg_serving.types
cpg_serving.members
cpg_serving.calls
cpg_serving.call_graph
cpg_serving.cfg_nodes
cpg_serving.cfg_edges
cpg_serving.def_use
cpg_serving.value_flow
cpg_serving.memory_accesses
cpg_serving.aliases
cpg_serving.effects
cpg_serving.exceptions
cpg_serving.resources
cpg_serving.async_relations
cpg_serving.generated
cpg_serving.unknowns
cpg_serving.metrics
cpg_serving.callable_summaries
```

Views SHALL:

- hide operational hash/bucket columns by default;
- preserve fact IDs;
- expose enum names alongside codes where useful;
- retain certainty and resolution;
- avoid collapsing exact and may relationships.

---

## 93. Table functions

Recommended UDTFs:

### 93.1 `cpg_neighbors`

```text
cpg_neighbors(node_id, relation_family, direction)
```

Returns direct relation rows and endpoint metadata.

### 93.2 `cpg_reachable`

```text
cpg_reachable(seed_id, relation_set, direction, max_depth, include_may)
```

Backed by `CpgGraphTraverseExec`.

### 93.3 `cpg_source_context`

```text
cpg_source_context(entity_id, before_lines, after_lines)
```

Returns source bytes/text and enclosing syntax/semantic owners.

### 93.4 `cpg_owner_facts`

```text
cpg_owner_facts(owner_id, fact_family_mask)
```

Returns all fact IDs owned by the selected owner without scanning unrelated buckets.

These functions provide factual retrieval only.

---

## 94. Query-planning policy

Internal agent-query compilation SHOULD build `Expr` and `LogicalPlan` directly rather than emit arbitrary SQL.

The query compiler SHALL:

- bind enum names to codes;
- inject ID buckets;
- qualify all columns;
- alias computed output fields;
- push owner/file/entity filters to base tables;
- choose typed extension tables only when requested fields require them;
- cap recursive/traversal output;
- preserve exact/may distinctions;
- include certainty in returned facts.

---

# Part XVI — Physical Layout and Performance

## 95. Partitioning policy

### 95.1 Small control and dimension tables

No partitioning:

```text
repository
publication
publication_table
current_publication
enum catalog
```

### 95.2 Owner-local fact tables

Partition by:

```text
owner_bucket
```

### 95.3 Universal `entity`

Partition by:

```text
entity_family_code, owner_bucket
```

### 95.4 Universal `relation`

Partition by:

```text
relation_family_code, owner_bucket
```

### 95.5 High-volume effect/derived tables

Partition by their low-cardinality semantic family plus owner bucket, for example:

```text
effect_kind_code, owner_bucket
projection_code, owner_bucket
metric_code, owner_bucket
```

High-cardinality IDs SHALL NOT be partition columns.

---

## 96. Z-order and clustering policy

Z-order is a maintenance optimization, not a semantic requirement.

Recommended candidates:

| Table | Z-order candidates |
|---|---|
| `entity` | `entity_id_hash64`, `parent_entity_id_hash64`, `file_id_hash64` |
| `relation` | `source_hash64`, `target_hash64`, `relation_kind_code` |
| `reference_detail` | resolved-entity hash, name hash if materialized |
| `call_target_detail` | call-site hash, target hash |
| `memory_access_detail` | location hash, cfg-node hash |
| `effect_detail` | callable hash, target hash |

Z-order SHALL only be scheduled after representative query benchmarks show file-skipping benefit.

---

## 97. Parquet writer policy

Starting writer targets:

```text
target Delta file size          128–256 MiB
Parquet row-group size           32–128 MiB
compression                      ZSTD unless interoperability dictates otherwise
dictionary encoding              enabled for low/medium-cardinality strings and codes
statistics                       enabled for IDs, buckets, codes, file IDs, offsets
Bloom filters                    benchmark for point-looked-up IDs
Arrow schema metadata            retained
```

Very small owner updates SHALL be micro-batched across owners before publication to avoid tiny files.

One owner SHALL NOT imply one Parquet file.

---

## 98. DataFusion runtime policy

The query/derivation runtime SHALL configure:

```text
target_partitions               based on CPU and workload
batch_size                      benchmarked; start 65,536
limited memory pool             mandatory for services
spill directory                 mandatory for large/global calculations
max spill size                  bounded
metadata/file/statistics cache  enabled
Parquet pruning                 enabled
repartition joins/aggregates    enabled where beneficial
```

Custom graph operators SHALL use the same `RuntimeEnv`, memory pool, disk manager, and object-store registry as normal DataFusion execution.

---

## 99. Update locality

Owner replacement and derived invalidation SHALL minimize rewritten files by:

- grouping changed owners by `owner_bucket`;
- sorting outgoing rows by owner and primary key;
- writing multi-owner batches;
- avoiding full-table overwrite;
- only recomputing derived owners reachable in the dependency graph;
- materializing global derived tables only when their cost is justified.

---

## 100. Compaction thresholds

An optimize job SHOULD be triggered when any partition exceeds configured thresholds such as:

```text
active file count
median file size below threshold
small-file ratio
query planning latency
post-DML rewrite fragmentation
```

Default maintenance:

- compact closed owner buckets or relation-family partitions;
- target 128–256 MiB files initially;
- cap concurrent optimize tasks;
- use the service DataFusion session state;
- require session fallback policy rather than silently using internal defaults.

---

## 101. Vacuum policy

Vacuum SHALL preserve:

- every version pinned by `current_publication`;
- any publication in staging/validation that may still complete;
- the configured recovery publication, if one exists;
- any explicit operational hold.

The core fabric SHALL not retain old versions for semantic history.

Vacuum workflow:

```text
1. enumerate pinned table versions
2. dry run
3. verify candidates do not serve pinned publications
4. execute retention-governed vacuum
5. reopen current publication
6. run table and cross-table smoke queries
```

---

# Part XVII — Constraints, Integrity, and Schema Evolution

## 102. Delta constraints

Delta constraints SHOULD enforce row-local invariants where supported.

Examples:

```text
start_byte >= 0
end_byte >= start_byte
owner_bucket BETWEEN 0 AND 255
source_bucket BETWEEN 0 AND 255
target_bucket BETWEEN 0 AND 255
counts >= 0
required IDs NOT NULL
```

ID byte-length checks SHOULD be enforced in Arrow validation and MAY also be expressed as Delta checks when the pinned expression support is compile-tested.

### 102.1 Uniqueness and foreign keys

Delta does not replace application-level uniqueness and foreign-key validation.

The schema registry SHALL declare:

- primary keys;
- unique constraints;
- foreign-key-like references;
- required relation endpoint families.

DataFusion validation plans SHALL enforce these before publication.

### 102.2 DataFusion constraints

Published `TableProvider`s MAY expose primary/unique constraints to DataFusion only after the publication has passed uniqueness validation.

Incorrect constraint metadata is prohibited because optimizer behavior may rely on it.

---

## 103. Schema compatibility policy

Default compatible changes:

```text
add nullable column
add advisory field metadata
add new enum code
add new optional extension table
```

Default incompatible changes:

```text
rename/drop persisted column
change primary key
change partition columns
narrow type
change nullability from nullable to required
reuse enum code
change ID encoding
change table grain
```

### 103.1 Required-field additions

A new non-nullable field requires:

1. new schema version;
2. deterministic backfill;
3. validation;
4. publication using the migrated table;
5. compatibility review.

### 103.2 Partition evolution

Partition changes SHALL create a new Delta table root, backfill through DataFusion, validate, and update the publication manifest. In-place routine partition changes are prohibited.

### 103.3 Schema merge

`SchemaMode::Merge` SHALL not be the default ingestion mode. Schema evolution is an explicit migration operation.

---

# Part XVIII — Operational Workflows

## 104. Bootstrap workflow

```text
1. initialize schema registry
2. create control tables
3. create all required Delta fact tables
4. register immutable enum dimensions
5. ingest complete source/fact snapshot as Arrow streams
6. reconcile and derive
7. validate all tables
8. publish first manifest
9. open current-state DataFusion catalog
10. run conformance queries
```

## 105. Incremental owner refresh

```text
1. receive owner-scoped FactBatch outputs
2. validate Arrow schemas and IDs
3. encode base extension tables
4. replace affected owners in base tables
5. reconcile canonical entity/relation rows
6. rebuild owner-local CFG/dataflow/memory facts
7. recompute owner-local derived facts
8. propagate affected call/summary computations
9. validate
10. publish new manifest
```

## 106. Owner deletion

```text
1. identify removed owner and dependent generated owners
2. delete owner rows from every owner-scoped table
3. remove cross-owner relations owned by affected callers/sources
4. recompute affected global/SCC/summary tables
5. validate no dangling current relations
6. publish
```

## 107. Failed publication recovery

```text
active pointer remains unchanged
abandoned Delta versions remain unreferenced
retry uses same publication/operation IDs where safe
or start a replacement publication
cleanup occurs after retention and pinned-version checks
```

## 108. Schema migration workflow

```text
1. register new TableSpec version
2. create new table root when required
3. read current publication through pinned providers
4. transform with DataFusion
5. write migrated Delta table
6. validate Arrow/Delta/DataFusion schemas
7. publish manifest referencing new table/version
8. retain old pinned version until recovery window expires
9. vacuum according to policy
```

## 109. Maintenance workflow

```text
compact fragmented closed partitions
benchmark Z-order candidates
vacuum unreferenced versions after dry run
refresh statistics/manifest counts
reopen current catalog
run integrity and representative query suite
```

---

# Part XIX — Query, Validation, and Observability Artifacts

## 110. Plan artifact bundle

Every important derivation and serving query SHOULD be able to emit:

```text
input PlanSpec or query identifier
DataFusion version
Arrow version
schema registry version
publication ID
source table versions
logical plan
optimized logical plan
physical plan
output schema
partition count
EXPLAIN text/graphviz
execution metrics
row count
result checksum
```

Plan artifacts are operational diagnostics, not CPG facts.

---

## 111. Metrics

The fabric SHALL emit operational metrics for:

```text
provider rows received
Arrow rows encoded
validation failures
DataFusion reconciliation rows
Delta commits by table
owner replacement latency
publication latency
rows/files per table
Parquet file sizes
small-file counts
query planning/execution time
spill bytes
custom graph operator iterations
custom graph operator peak memory
unknown fact counts
integrity query failures
```

These metrics SHALL not be inserted into the semantic CPG metric table unless they describe objective code structure rather than fabric operation.

---

## 112. Testing strategy

### 112.1 Schema tests

- exact Arrow schema snapshots;
- Arrow-to-Delta-to-DataFusion round trip;
- field metadata preservation;
- unsupported type rejection;
- partition contract tests.

### 112.2 Batch tests

- empty batch;
- one row;
- all nullable fields null;
- maximum-length names/paths;
- invalid ID length;
- duplicate primary keys;
- malformed source spans.

### 112.3 Delta tests

- owner replacement visibility through old/new manifests;
- retry idempotency;
- concurrent publication conflict;
- delete+append recovery;
- optimize and vacuum safety;
- local and object-store backends.

### 112.4 DataFusion tests

- catalog opens exact pinned versions;
- projection/filter/bucket pushdown;
- logical and physical plan snapshots;
- custom UDF/UDAF tests;
- custom graph operator golden results;
- memory/spill limits;
- cancellation.

### 112.5 Integrity tests

- dangling edge detection;
- owner completeness;
- CFG consistency;
- dataflow endpoint consistency;
- type endpoint consistency;
- unknown materialization;
- table row counts/checksums;
- cross-publication isolation.

---

# Part XX — Rust Workspace Architecture

## 113. Recommended crates

```text
cpg-schema
  Arrow/Delta schemas, enum registries, TableSpec, metadata keys

cpg-arrow
  typed builders, batch encoders, validators, Arrow kernels

cpg-delta
  table creation, owner replacement, DML, publication manifest, maintenance

cpg-catalog
  publication-pinned DataFusion CatalogProvider / SchemaProvider / TableProvider wrappers

cpg-plans
  reconciliation plans, serving PlanSpec compiler, integrity plans

cpg-functions
  DataFusion UDFs, UDAFs, and UDTFs

cpg-graph-exec
  custom logical nodes and ExecutionPlans for graph/dataflow algorithms

cpg-publisher
  dependency scheduling, multi-table publication, recovery

cpg-query
  stable serving views and agent-facing fact query compiler

cpg-conformance
  fixtures, golden schemas, SQLLogicTests, property tests, benchmarks
```

Provider/extractor crates from the companion generation specification remain upstream of this fabric.

---

## 114. Core Rust interfaces

```rust
pub trait TableEncoder {
    fn table_spec(&self) -> &'static TableSpec;
    fn encode(&self, batch: CanonicalFactBatch) -> Result<Vec<RecordBatch>, EncodeError>;
}

#[async_trait::async_trait]
pub trait OwnerTableWriter {
    async fn replace_owners(
        &self,
        table: &TableSpec,
        owners: &[Id128],
        batches: Vec<RecordBatch>,
        operation: OperationContext,
    ) -> Result<CommittedTableVersion, WriteError>;
}

pub trait ReconciliationPlanner {
    fn build_plan(&self, inputs: ReconcileInputs) -> Result<LogicalPlan, PlanError>;
}

pub trait DerivationPlanner {
    fn dependencies(&self) -> &'static [TableCode];
    fn build_plan(&self, publication: &PublicationView) -> Result<LogicalPlan, PlanError>;
}

#[async_trait::async_trait]
pub trait PublicationStore {
    async fn stage(&self, request: PublicationRequest) -> Result<PublicationId, PublishError>;
    async fn record_table(&self, version: CommittedTableVersion) -> Result<(), PublishError>;
    async fn validate(&self, publication: PublicationId) -> Result<ValidationReport, PublishError>;
    async fn activate(&self, publication: PublicationId) -> Result<(), PublishError>;
}
```

Provider-specific and delta-rs-internal types SHALL not leak through stable application interfaces.

---

# Part XXI — Implementation Sequence

## 115. Phase 1 — Schema and publication foundation

Deliver:

- version-pinned workspace;
- `TableSpec` registry;
- `repository`, publication, owner, capability, diagnostic tables;
- `entity`, `relation`, `fact_evidence`;
- publication-pinned DataFusion catalog;
- Arrow/Delta/DataFusion schema round-trip tests.

## 116. Phase 2 — Source and semantic base tables

Deliver:

- source, token, annotation, syntax, semantic, scope, binding, reference, import tables;
- typed Arrow encoders;
- owner replacement;
- canonical reconciliation plans.

## 117. Phase 3 — Types, calls, and CFG

Deliver:

- type, member, callable, parameter, call site, argument, target tables;
- CFG graph/node/edge tables;
- core serving views;
- point-query pushdown.

## 118. Phase 4 — Dataflow, memory, effects, and language extensions

Deliver:

- value, operation, dataflow event, memory/access-path, program-state tables;
- effect, exception, resource, async, capture, generated tables;
- Python dynamic and Rust MIR extension tables.

## 119. Phase 5 — Derived calculations

Deliver:

- direct relational metrics;
- reachability operator;
- SCC operator;
- dominator/post-dominator/control-dependence operators;
- loop derivation;
- reaching definitions/liveness;
- points-to/alias fixed point;
- interprocedural summary propagation.

## 120. Phase 6 — Performance and production hardening

Deliver:

- optimized partition specs;
- compaction and vacuum runbooks;
- Bloom/Z-order benchmarks;
- memory/spill policies;
- idempotent retries and recovery;
- object-store tests;
- full conformance and performance suite.

---

# Appendix A — Table Dependency Order

```text
repository
  ↓
owner, source_file
  ↓
source_token, source_annotation, syntax_detail
  ↓
semantic_detail, scope_detail, binding_detail, reference_detail, module_import_detail
  ↓
type_detail, type_fact_detail, member_relation_detail
  ↓
callable_detail, parameter_detail, call_site_detail, call_argument_detail, call_target_detail
  ↓
cfg_graph, cfg_node_detail, cfg_edge_detail
  ↓
value_detail, operation_detail, dataflow_event_detail
  ↓
memory_location_detail, access_path_component, memory_access_detail, program_state_detail
  ↓
effect_detail, exception_detail, resource_event_detail, async_event_detail, capture_detail
  ↓
Python/Rust/generated extension tables
  ↓
entity, relation canonical reconciliation
  ↓
derived_component, metric, callable_summary, derived relations
  ↓
publication_table
  ↓
publication COMPLETE
  ↓
current_publication
```

Implementations MAY write canonical `entity` and `relation` earlier, but publication dependencies SHALL reflect all extension and derived tables required by the active schema bundle.

---

# Appendix B — Default Table Properties

Starting defaults to benchmark:

```text
Delta CDF                         disabled
Delta column mapping              none
Delta type widening               disabled
Parquet compression               ZSTD
Target file size                  256 MiB for large fact tables
Target file size                  128 MiB for medium tables
Parquet row group                 64 MiB
Arrow schema metadata             enabled
Owner bucket count                256
DataFusion batch size             65,536
DataFusion target partitions      available parallelism, workload-adjusted
Memory pool                       limited
Spill directory                   configured and bounded
Optimize concurrency              4–8 tasks initially
Vacuum                            dry-run first; preserve manifest-pinned versions
```

---

# Appendix C — Mandatory Invariants

```text
1. Every published table version is pinned by one complete publication manifest.
2. Query sessions never mix table versions from different publications.
3. Every graph entity and relation uses a deterministic application-owned ID.
4. Every entity/relation belongs to one deterministic owner.
5. Every hot fact table has a typed Arrow/Delta schema; no EAV canonical store exists.
6. Every relation endpoint exists or points to an explicit unknown entity.
7. Exact, may, and unknown relationships remain distinguishable.
8. Direct and transitive effects remain distinguishable.
9. Source spans are byte-based and validated against current source bytes.
10. Delta schemas are table contracts; Arrow schemas are batch contracts; Parquet schemas are physical-file contracts.
11. Schema evolution is explicit and versioned.
12. Owner replacement is invisible until publication activation.
13. Custom DataFusion graph operators obey memory, spill, cancellation, streaming, and PlanProperties contracts.
14. Global transitive closure is not materialized without a bounded, demonstrated need.
15. Old Delta versions are operational state, not exposed semantic history.
16. No canonical table contains engineering recommendations or evaluative conclusions.
```

---

# Appendix D — Explicit Non-Outputs

The data fabric SHALL NOT create canonical tables or fields for:

```text
refactor safety
test impact
coverage
runtime profiling
historical change analysis
risk scores
bug likelihood
architecture quality
vulnerability exploitability
recommendations
remediation plans
change prioritization
```

Such products may be built later as downstream analyses over this factual fabric, but they are outside this specification.
