Yes. The strongest version of this pattern is to treat DataFusion not merely as a SQL engine over some Arrow tables, but as the **compiled relational runtime for an explicit ontology and object hierarchy**.

That fits both the DataFusion/Arrow design principles and the current CodeFabric ontology especially well: the ontology defines three canonical proposition forms—entity existence, relation facts, and property facts—and requires context, ownership, certainty, resolution, provenance, and completeness to remain explicit.  DataFusion's hierarchy then gives us a natural way to make those meanings structural: `CatalogProviderList → CatalogProvider → SchemaProvider → TableProvider`, with each `TableProvider` owning its schema and scan contract. 

I would structure a native Rust implementation roughly as follows.

## 1. The conceptual hierarchy

For one leased CodeFabric `ServingSnapshot`, I would expose one immutable DataFusion catalog:

```text
SessionContext
│
└── catalog: cpg
    │
    ├── schema: ontology
    │   ├── entity_kind
    │   ├── relation_kind
    │   ├── property_kind
    │   ├── certainty_kind
    │   ├── resolution_kind
    │   └── precision_profile
    │
    ├── schema: facts
    │   ├── entity
    │   ├── relation
    │   └── property_fact
    │
    ├── schema: source
    │   ├── file
    │   ├── syntax_node
    │   ├── token
    │   └── source_span
    │
    ├── schema: semantic
    │   ├── declaration
    │   ├── reference
    │   ├── callable
    │   ├── type
    │   └── member
    │
    ├── schema: rust
    │   ├── mir_block
    │   ├── mir_statement
    │   ├── place
    │   ├── instance
    │   └── borrow
    │
    └── schema: derived
        ├── dominance
        ├── reaching_definition
        ├── call_scc
        └── callable_summary
```

The important point is that this is **not arbitrary SQL organization**. Each level has semantics:

```text
CatalogProvider
    = one internally coherent semantic universe / ServingSnapshot

SchemaProvider
    = one ontology domain / namespace

TableProvider
    = one canonical typed relation

Arrow Field
    = one ontology-defined attribute/property contract

Arrow RecordBatch
    = one immutable physical realization of facts

DataFusion Expr / LogicalPlan
    = compiled relational reasoning over those facts
```

This closely follows the principle that hierarchy should encode shared guarantees and legal variation rather than merely organize names. 

---

# 2. Pin the runtime

For DataFusion 55, the coordinated Arrow version is 59.2.0:

```toml
[package]
name = "codefabric-data-model"
edition = "2024"

[dependencies]
datafusion = "=55.0.0"
arrow = "=59.2.0"
async-trait = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

In a real CodeFabric workspace I would generally use `datafusion::arrow::*` at DataFusion boundaries to prevent accidental duplicate Arrow type universes.

---

# 3. Start with Rust ontology objects, not schemas

The Arrow schemas should be **compiled from ontology objects**, rather than hand-written independently.

For example:

```rust
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    File = 1,
    Module = 2,
    Function = 10,
    Method = 11,
    Class = 20,
    Type = 30,
    CallSite = 40,
    MirBlock = 100,
    MirStatement = 101,
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    Contains = 1,
    Defines = 2,
    RefersTo = 3,

    CallsExact = 100,
    MayCall = 101,

    Reads = 200,
    Writes = 201,

    Dominates = 300,
    Reaches = 301,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Certainty {
    Exact = 1,
    SoundPossible = 2,
    Approximate = 3,
    Unknown = 255,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Resolved = 1,
    PartiallyResolved = 2,
    Unresolved = 3,
}

#[derive(Debug, Clone)]
pub struct ColumnContract {
    pub name: &'static str,
    pub data_type: arrow::datatypes::DataType,
    pub nullable: bool,

    // Ontological meaning
    pub semantic_type: &'static str,
    pub ontology_property: Option<&'static str>,
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub struct TableContract {
    pub schema_name: &'static str,
    pub table_name: &'static str,

    /// e.g. "RELATION_FACT"
    pub ontology_class: &'static str,

    pub contract_version: &'static str,
    pub columns: Vec<ColumnContract>,
}
```

So the dependency direction is:

```text
CodeFabric ontology
        ↓
Rust enum / model registry
        ↓
TableContract
        ↓
Arrow Schema
        ↓
TableProvider
        ↓
DataFusion
```

Never:

```text
Arrow Schema
    ↓ reverse-engineer ontology meaning
```

That distinction matters a lot.

---

# 4. Use Arrow metadata as executable semantic annotation

Now compile those objects to Arrow.

I would put durable semantic metadata directly on `Field`s and `Schema`s:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use datafusion::arrow::datatypes::{
    DataType, Field, Fields, Schema, SchemaRef,
};

fn ontology_field(
    name: &str,
    data_type: DataType,
    nullable: bool,
    semantic_type: &str,
    ontology_property: &str,
) -> Field {
    Field::new(name, data_type, nullable).with_metadata(HashMap::from([
        (
            "cf.semantic.type".to_string(),
            semantic_type.to_string(),
        ),
        (
            "cf.ontology.property".to_string(),
            ontology_property.to_string(),
        ),
        (
            "cf.schema.version".to_string(),
            "1.3".to_string(),
        ),
    ]))
}
```

That metadata should not itself become the ontology authority. It is a **compiled annotation pointing back to the authority**.

---

# 5. Make CodeFabric IDs true logical types

Internally, CodeFabric IDs are 16-byte values. That strongly suggests:

```rust
DataType::FixedSizeBinary(16)
```

rather than storing:

```text
"entity:function:a81b..."
```

as UTF-8 on every row.

For example:

```rust
fn entity_id_field(name: &str) -> Field {
    Field::new(
        name,
        DataType::FixedSizeBinary(16),
        false,
    )
    .with_metadata(HashMap::from([
        (
            "ARROW:extension:name".to_string(),
            "codefabric.entity_id".to_string(),
        ),
        (
            "ARROW:extension:metadata".to_string(),
            r#"{"id_preimage_version":1}"#.to_string(),
        ),
        (
            "cf.semantic.type".to_string(),
            "entity_identity".to_string(),
        ),
    ]))
}
```

This is particularly interesting with DataFusion 55 because DataFusion now has the engine-level `ExtensionTypeRegistry`: Arrow extension metadata can resolve into typed `DFExtensionType` objects instead of being merely an opaque convention. 

Conceptually, I would register:

```text
codefabric.entity_id
codefabric.fact_id
codefabric.workspace_id
codefabric.context_id
codefabric.owner_id
codefabric.type_id
```

all with efficient physical storage but different logical identities.

That gives you three levels simultaneously:

```text
Rust type          EntityId
Arrow storage      FixedSizeBinary(16)
logical extension  codefabric.entity_id
```

This is exactly the sort of model-first typing DataFusion 55 now lets you carry much deeper into planning.

---

# 6. Use complex Arrow types where the relationship is genuinely contained

Relational normalization should remain the default for graph relationships.

But things that are structurally **part of one fact** are good candidates for nested Arrow types.

For example, source evidence:

```rust
fn source_span_type() -> DataType {
    DataType::Struct(Fields::from(vec![
        Arc::new(entity_id_field("file_id")),

        Arc::new(ontology_field(
            "start_byte",
            DataType::UInt32,
            false,
            "source_byte_offset",
            "source.span.start_byte",
        )),

        Arc::new(ontology_field(
            "end_byte",
            DataType::UInt32,
            false,
            "source_byte_offset",
            "source.span.end_byte",
        )),

        Arc::new(Field::new(
            "start_line",
            DataType::UInt32,
            true,
        )),

        Arc::new(Field::new(
            "start_column",
            DataType::UInt32,
            true,
        )),
    ]))
}
```

Likewise provenance:

```rust
fn provenance_type() -> DataType {
    DataType::Struct(Fields::from(vec![
        Arc::new(Field::new(
            "producer",
            DataType::Utf8View,
            false,
        )),
        Arc::new(Field::new(
            "producer_version",
            DataType::Utf8View,
            false,
        )),
        Arc::new(Field::new(
            "certainty_code",
            DataType::UInt8,
            false,
        )),
        Arc::new(Field::new(
            "resolution_code",
            DataType::UInt8,
            false,
        )),
        Arc::new(Field::new(
            "directness_code",
            DataType::UInt8,
            false,
        )),
        Arc::new(Field::new(
            "source_span",
            source_span_type(),
            true,
        )),
    ]))
}
```

So one relation row can be:

```text
subject ──CALLS_EXACT──> object

plus:
    ordinal
    role
    precision
    provenance {
        producer
        version
        certainty
        resolution
        source_span {...}
    }
```

without turning source evidence into a dozen additional joins.

---

# 7. But keep actual graph semantics relational

The universal relation table should remain flat where it matters:

```rust
fn relation_schema() -> SchemaRef {
    Arc::new(
        Schema::new_with_metadata(
            vec![
                entity_id_field("fact_id"),
                entity_id_field("workspace_id"),
                entity_id_field("analysis_context_id"),
                entity_id_field("owner_id"),

                entity_id_field("subject_id"),

                ontology_field(
                    "relation_kind_code",
                    DataType::UInt16,
                    false,
                    "relation_kind",
                    "relation.kind",
                ),

                entity_id_field("object_id"),

                Field::new(
                    "role_code",
                    DataType::UInt16,
                    true,
                ),

                Field::new(
                    "ordinal",
                    DataType::UInt32,
                    true,
                ),

                Field::new(
                    "certainty_code",
                    DataType::UInt8,
                    false,
                ),

                Field::new(
                    "resolution_code",
                    DataType::UInt8,
                    false,
                ),

                Field::new(
                    "directness_code",
                    DataType::UInt8,
                    false,
                ),

                Field::new(
                    "evidence",
                    provenance_type(),
                    true,
                ),
            ],
            HashMap::from([
                (
                    "cf.ontology.fact_form".to_string(),
                    "RELATION_FACT".to_string(),
                ),
                (
                    "cf.schema.contract".to_string(),
                    "codefabric.relation_fact".to_string(),
                ),
                (
                    "cf.schema.version".to_string(),
                    "1.3".to_string(),
                ),
            ]),
        )
    )
}
```

That is a very powerful relational graph representation because:

```text
subject_id
relation_kind_code
object_id
```

are ordinary typed columns.

DataFusion can therefore perform:

```text
filter
hash join
sort-merge join
aggregate
semi join
anti join
window
grouping
distinct
union
recursive/custom traversal operators
```

without converting the CPG into some secondary graph-object format.

---

# 8. Property values: typed, not JSON blobs

The same principle applies to `PROPERTY_FACT`.

I would not do:

```text
value_json: Utf8
```

as the canonical model.

Instead:

```rust
fn property_value_type() -> DataType {
    DataType::Struct(Fields::from(vec![
        Arc::new(Field::new("bool_value", DataType::Boolean, true)),
        Arc::new(Field::new("i64_value", DataType::Int64, true)),
        Arc::new(Field::new("u64_value", DataType::UInt64, true)),
        Arc::new(Field::new("f64_value", DataType::Float64, true)),
        Arc::new(Field::new("text_value", DataType::Utf8View, true)),
        Arc::new(Field::new(
            "entity_value",
            DataType::FixedSizeBinary(16),
            true,
        )),
    ]))
}
```

and then:

```text
property_kind_code
value_kind_code
value: Struct<...>
```

For hot properties, the ontology already permits denormalized projections as long as they are not confused with the canonical provenance-bearing fact. 

Thus you might additionally project:

```text
property_fact.value_i64
property_fact.value_text
```

for common filters while retaining:

```text
property_fact.value
```

as the canonical typed representation.

---

# 9. A real `TableProvider` object

The next step is important: the table itself should be an object carrying the semantic contract, not merely an Arrow batch registered under a string.

For an illustrative in-memory provider:

```rust
use async_trait::async_trait;

use datafusion::catalog::Session;
use datafusion::common::Result;
use datafusion::datasource::{
    memory::MemTable,
    TableProvider,
    TableProviderFilterPushDown,
    TableType,
};
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct OntologyTableProvider {
    contract: Arc<TableContract>,
    inner: MemTable,
}

impl OntologyTableProvider {
    pub fn new(
        contract: Arc<TableContract>,
        schema: SchemaRef,
        partitions: Vec<Vec<datafusion::arrow::record_batch::RecordBatch>>,
    ) -> Result<Self> {
        Ok(Self {
            contract,
            inner: MemTable::try_new(schema, partitions)?,
        })
    }

    pub fn contract(&self) -> &TableContract {
        &self.contract
    }
}

#[async_trait]
impl TableProvider for OntologyTableProvider {
    fn schema(&self) -> SchemaRef {
        self.inner.schema()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>> {
        // Conservative for the illustrative provider.
        //
        // A production Delta/overlay provider would classify supported
        // ontology predicates as Exact / Inexact / Unsupported.
        Ok(filters
            .iter()
            .map(|_| TableProviderFilterPushDown::Unsupported)
            .collect())
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        self.inner
            .scan(state, projection, filters, limit)
            .await
    }
}
```

This is a small example, but the architecture is significant.

The production implementation becomes:

```text
OntologyTableProvider
    owns:
        TableContract
        Arrow Schema
        ontology identity
        exact snapshot identity
        completeness contract
        access policy
        pushdown behavior
        statistics contract

    delegates physical reading to:
        DeltaSnapshotProvider
        + HotOverlayProvider
```

DataFusion explicitly treats a `TableProvider` as the object responsible for table schema, planning information, pushdown truthfulness, statistics and scan execution.  DataFusion 55 additionally provides the more structured scan/statistics machinery around `ScanArgs` and statistics requests for advanced providers. 

---

# 10. The schema namespace should also be an object

For a leased snapshot, I would make it immutable:

```rust
use std::collections::BTreeMap;

use datafusion::catalog::SchemaProvider;

#[derive(Debug)]
pub struct OntologySchemaProvider {
    tables: BTreeMap<String, Arc<dyn TableProvider>>,
}

impl OntologySchemaProvider {
    pub fn new(
        tables: BTreeMap<String, Arc<dyn TableProvider>>,
    ) -> Self {
        Self { tables }
    }
}

#[async_trait]
impl SchemaProvider for OntologySchemaProvider {
    fn table_names(&self) -> Vec<String> {
        self.tables.keys().cloned().collect()
    }

    async fn table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<dyn TableProvider>>> {
        Ok(self.tables.get(name).cloned())
    }

    fn table_exist(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }
}
```

Notice the use of `BTreeMap`.

That is intentional:

```text
deterministic registration
deterministic introspection
deterministic schema listings
deterministic testing/fingerprinting
```

rather than relying on incidental hash-map order.

---

# 11. And the snapshot catalog itself should be an object

```rust
use datafusion::catalog::{CatalogProvider, SchemaProvider};

#[derive(Debug)]
pub struct ServingSnapshotCatalog {
    snapshot_id: [u8; 16],
    schemas: BTreeMap<String, Arc<dyn SchemaProvider>>,
}

impl CatalogProvider for ServingSnapshotCatalog {
    fn schema_names(&self) -> Vec<String> {
        self.schemas.keys().cloned().collect()
    }

    fn schema(
        &self,
        name: &str,
    ) -> Option<Arc<dyn SchemaProvider>> {
        self.schemas.get(name).cloned()
    }
}
```

I would deliberately **not** expose mutation through this object after construction.

Its semantics are:

```text
ServingSnapshotCatalog
    = immutable relational realization
      of one leased CodeFabric ServingSnapshot
```

You then construct a new one when a different snapshot is leased.

That is much stronger than updating tables inside one live mutable catalog while a query is executing.

---

# 12. Register the entire semantic universe once

```rust
use datafusion::prelude::*;

fn build_catalog(
    snapshot_id: [u8; 16],
    ontology: Arc<dyn SchemaProvider>,
    facts: Arc<dyn SchemaProvider>,
    source: Arc<dyn SchemaProvider>,
    semantic: Arc<dyn SchemaProvider>,
    rust: Arc<dyn SchemaProvider>,
    derived: Arc<dyn SchemaProvider>,
) -> Arc<dyn CatalogProvider> {
    Arc::new(ServingSnapshotCatalog {
        snapshot_id,
        schemas: BTreeMap::from([
            ("ontology".into(), ontology),
            ("facts".into(), facts),
            ("source".into(), source),
            ("semantic".into(), semantic),
            ("rust".into(), rust),
            ("derived".into(), derived),
        ]),
    })
}

let ctx = SessionContext::new();

ctx.register_catalog(
    "cpg",
    build_catalog(
        snapshot_id,
        ontology_schema,
        facts_schema,
        source_schema,
        semantic_schema,
        rust_schema,
        derived_schema,
    ),
);
```

Now names such as:

```text
cpg.facts.entity
cpg.facts.relation
cpg.ontology.relation_kind
cpg.rust.mir_block
cpg.derived.dominance
```

are not merely paths.

They are **typed semantic addresses** in the ontology.

---

# 13. Put the ontology itself in the fabric

An important part of the approach is that the ontology registry should itself be queryable.

For example:

```text
cpg.ontology.relation_kind

code | canonical_name | semantic_class
-----+----------------+-----------------
1    | CONTAINS       | structural
2    | DEFINES        | semantic
3    | REFERS_TO      | semantic
100  | CALLS_EXACT    | call
101  | MAY_CALL       | call
200  | READS          | memory
201  | WRITES         | memory
300  | DOMINATES      | control_flow
```

Consequently, the data describes itself.

You can ask DataFusion:

```sql
SELECT canonical_name
FROM cpg.ontology.relation_kind
ORDER BY code;
```

or validate:

```sql
SELECT DISTINCT r.relation_kind_code
FROM cpg.facts.relation r
LEFT ANTI JOIN cpg.ontology.relation_kind k
    ON r.relation_kind_code = k.code;
```

and a nonempty result means:

> We have facts referring to relation kinds not defined by the ontology.

That is an excellent example of making the ontology **executable rather than descriptive**.

---

# 14. Object-defined relational query

Suppose we want:

> Return exact calls from Rust functions to other functions, together with callee cyclomatic complexity.

The first stage can be built entirely from typed DataFusion expressions:

```rust
use datafusion::prelude::*;

let exact_calls = ctx
    .table("cpg.facts.relation")
    .await?
    .filter(
        col("relation_kind_code")
            .eq(lit(RelationKind::CallsExact as u16))
    )?
    .select(vec![
        col("subject_id"),
        col("object_id"),
        col("certainty_code"),
        col("evidence"),
    ])?;
```

At this point the logical plan means:

```text
Scan RELATION_FACT
    ↓
retain CALLS_EXACT
    ↓
project caller/callee/provenance
```

No graph DSL is required.

For illustration, the full relational join is easiest to read in SQL:

```sql
WITH exact_calls AS (
    SELECT
        fact_id,
        subject_id AS caller_id,
        object_id  AS callee_id,
        certainty_code,
        evidence
    FROM cpg.facts.relation
    WHERE relation_kind_code = 100
),

complexity AS (
    SELECT
        subject_id AS entity_id,
        value_i64 AS cyclomatic_complexity
    FROM cpg.facts.property_fact
    WHERE property_kind_code = 410
)

SELECT
    caller.qualified_name AS caller,
    callee.qualified_name AS callee,
    calls.certainty_code,
    complexity.cyclomatic_complexity
FROM exact_calls calls

JOIN cpg.facts.entity caller
    ON calls.caller_id = caller.entity_id

JOIN cpg.facts.entity callee
    ON calls.callee_id = callee.entity_id

LEFT JOIN complexity
    ON complexity.entity_id = callee.entity_id

WHERE caller.language_code = 2
ORDER BY
    caller.qualified_name,
    callee.qualified_name;
```

Physically DataFusion is free to choose:

```text
projection pushdown
predicate pushdown
hash joins
partitioning
batch scheduling
parallel execution
streaming
```

while the semantic query remains relational.

That separation—semantic `LogicalPlan` versus physical `ExecutionPlan`—is precisely the model-first compiler architecture the design principles target. 

---

# 15. This also makes graph traversal relational

Consider two-hop calls:

```sql
SELECT DISTINCT
    r1.subject_id AS source,
    r2.object_id  AS target
FROM cpg.facts.relation r1
JOIN cpg.facts.relation r2
    ON r1.object_id = r2.subject_id
WHERE
    r1.relation_kind_code IN (100, 101)
AND r2.relation_kind_code IN (100, 101);
```

That is literally graph composition expressed as a relational join:

```text
A --R--> B
B --R--> C

join on:
    r1.object_id = r2.subject_id

produces:
    A --R²--> C
```

Likewise:

```text
incoming edges   = filter object_id
outgoing edges   = filter subject_id
edge-type slice  = filter relation_kind
node slice       = semi-join entity set
2-hop path       = self join
N-hop traversal  = iterative/custom relational operator
degree           = GROUP BY
SCC              = custom graph derivation → relational result table
dominance        = graph algorithm → relation facts
```

So graph computation can remain **inside the Arrow/DataFusion universe even when petgraph or a custom Rust algorithm performs the actual specialized calculation**.

---

# 16. DataFusion 55 extension types make the design even stronger

DataFusion's extension-type registry means CodeFabric can go beyond:

```text
Field {
    DataType::FixedSizeBinary(16)
    metadata = "trust me, this is an EntityId"
}
```

to a session that actually knows about `codefabric.entity_id`. DataFusion 55 retains the extension-type registry introduced in 54, including session-level registration and field-aware casts. 

Conceptually:

```rust
let registry =
    MemoryExtensionTypeRegistry::new_with_canonical_extension_types();

registry.extend(&[
    entity_id_registration(),
    fact_id_registration(),
    type_id_registration(),
])?;

let state = SessionStateBuilder::new()
    .with_default_features()
    .with_extension_type_registry(Arc::new(registry))
    .build();

let ctx = SessionContext::new_with_state(state);
```

This is particularly useful for CodeFabric because its object model has many **semantically different ID domains sharing one efficient physical storage representation**.

---

# 17. Where I would use nested versus relational structures

A useful rule for CodeFabric would be:

| Information | Representation |
|---|---|
| entity → entity relation | separate relational row |
| entity property with independent provenance | `property_fact` row |
| source span belonging to one fact | nested `Struct` |
| provenance attached to one fact | nested `Struct` |
| several evidence spans for one fact | `List<Struct<...>>` |
| canonical ID | extension-typed `FixedSizeBinary(16)` |
| ontology enum | compact integer code + ontology registry |
| human display label | `Utf8View` |
| repeated categorical text | potentially `Dictionary` |
| multiple semantic contexts | separate rows/context partition, not list |
| call graph | relation table |
| dominance graph | relation/derived table |
| complex type algebra | normalized type tables + nested components where appropriate |

The key criterion is:

> **If something has independent identity, independent provenance, independent cardinality, or participates independently in joins, make it relational. If it is structurally owned by one fact and normally consumed with it, consider nested Arrow representation.**

---

# 18. The strongest end-state

The resulting system would look like this:

```text
CodeFabric Ontology Registry
│
├─ EntityKind
├─ RelationKind
├─ PropertyKind
├─ Type algebra
├─ Certainty / resolution
└─ Precision profiles
        │
        ▼ compile
Rust semantic contract objects
        │
        ├─ TableContract
        ├─ ColumnContract
        ├─ Logical type contract
        └─ Relation contract
        │
        ▼ compile
Arrow 59
        │
        ├─ Schema / Field metadata
        ├─ extension types
        ├─ Struct / List / Dictionary
        ├─ FixedSizeBinary IDs
        └─ RecordBatch
        │
        ▼ exposed through
DataFusion 55 provider hierarchy
        │
        ├─ ServingSnapshotCatalog
        │
        ├─ OntologySchemaProvider
        │
        └─ OntologyTableProvider
        │
        ▼
DFSchema / Expr / LogicalPlan
        │
        ├─ projection
        ├─ filtering
        ├─ joins
        ├─ semi/anti joins
        ├─ aggregations
        ├─ windows
        ├─ sorting
        ├─ union
        └─ custom graph operators where justified
        │
        ▼
optimized ExecutionPlan
        │
        ▼
SendableRecordBatchStream
        │
        ▼
Arrow result
```

That is substantially more powerful than thinking of DataFusion as merely:

```text
"we have some CPG tables and can run SQL against them"
```

It instead becomes:

> **The ontology is the semantic authority; Rust objects make the ontology executable; Arrow is its canonical typed data realization; the DataFusion provider hierarchy exposes that realization as an object hierarchy; DataFusion's logical plan becomes the relational intermediate representation for code-intelligence operations; and Arrow streams remain the common data representation all the way through execution.**

That is, in my view, the clearest path to the **maximally object-defined, typed, relational, ontology-defined DataFusion implementation** you are describing. It also exploits exactly the distinction your design principles emphasize between semantic authority, compiled representation, execution strategy, and runtime data. 

One particularly important consequence is that **I would not create a separate “graph object model” for normal CPG querying**. The `entity`, `relation`, and `property_fact` tables *are* the canonical graph representation. Petgraph/custom graph operators should be temporary computational projections used to derive facts such as SCCs or dominance, which then return to the same relational fabric. That is also consistent with the current ontology's requirement that derived graph facts remain ordinary canonical facts rather than belonging to a competing graph authority. 