# Model-First, Contract-Driven, Provenance-Native Data Fabric

## 1. Purpose

The target architecture should optimize not merely for code that works, but for systems whose **meaning, authority, state, execution, and history are explicit in the design**.

The desired system should have five defining characteristics:

1. **Model-first:** important semantics exist as explicit typed models rather than being implicit in procedural control flow.
2. **Contract-driven:** every important boundary states what is invariant, what may vary, who owns the contract, and how compliance is validated.
3. **Authority-centered:** each concept has one canonical source of truth; alternative representations are projections, caches, compiled forms, or views of that authority.
4. **Provenance-native:** lineage, versions, configuration, transformations, and execution identity are produced automatically as part of normal operations.
5. **Fabric-oriented:** storage, tables, schemas, calculations, plans, execution, and interoperability share common representations and lifecycle rules rather than behaving as disconnected subsystems.

The architecture should consequently resemble a **semantic compiler and execution platform** more than a collection of service methods.

DataFusion already illustrates this shape through a compiler-like sequence from SQL/DataFrame/`LogicalPlanBuilder` into `LogicalPlan`, logical optimization, physical planning, `ExecutionPlan`, and finally an Arrow `RecordBatch` stream.

---

# 2. Principle 1 — Model semantics before implementing behavior

## Philosophy

When a concept is important enough to affect multiple operations, it should normally exist first as an explicit **semantic model**.

Do not allow the meaning of the system to reside primarily in:

* sequences of function calls;
* nested `if` statements;
* scattered configuration lookups;
* strings constructed at runtime;
* conventions understood only by callers;
* duplicated special-case logic.

Instead, represent that meaning with typed structures that can be inspected, validated, compared, serialized, versioned, and transformed.

The calculation documentation already proposes exactly this pattern: a `CalculationSpec` captures inputs, outputs, expression structure, null policy, type policy, units, assumptions, and diagnostics, after which it can compile to `Expr`, SQL, UDFs, reference implementations, or test harnesses.

## Agent directive

**Prefer:**

```text
semantic intent
    ↓
typed semantic model
    ↓
validation
    ↓
binding / resolution
    ↓
compiled representation
    ↓
execution
```

over:

```text
request
    ↓
procedural code with embedded decisions
    ↓
execution
```

A programming agent should therefore ask before implementing substantial behavior:

> **What is the model that represents this concept independently of the code that executes it?**

Examples include:

```text
CalculationSpec
ExprSpec
PlanSpec
SchemaContract
TableSpec
SourceSpec
PartitionSpec
WriteSpec
OptimizationSpec
ValidationSpec
PolicySpec
ProvenanceSpec
```

These names are illustrative; the important principle is the explicit intermediate representation.

---

# 3. Principle 2 — Make models executable, not merely descriptive

A weak modeling approach creates configuration DTOs that are immediately unpacked into procedural code.

A stronger approach treats the model as a **declarative program**.

The same model should, where appropriate, support multiple derived operations:

```text
Model
 ├─ validate
 ├─ bind to schemas/catalog
 ├─ compile to Expr
 ├─ compile to LogicalPlan
 ├─ render as SQL
 ├─ derive required columns
 ├─ derive provenance dependencies
 ├─ derive documentation
 ├─ derive test fixtures
 ├─ fingerprint
 └─ execute
```

This is analogous to DataFusion's model of `LogicalPlan`: the logical plan describes **what computation means**, while a later stage determines **how that computation runs**. DataFusion intentionally makes logical plans independent of physical execution details, while providers, planners, optimizers, and `ExecutionPlan`s lower them to concrete execution.

## Agent rule

If a model exists, **do not re-encode its semantics separately in every consumer**.

Instead:

```text
one semantic representation
        ↓
multiple controlled interpreters / compilers
```

The objective is to reduce the number of places in which business or domain semantics can independently drift.

---

# 4. Principle 3 — One authoritative owner for every concept

Every important semantic concept should have exactly one clearly identified **authority**.

This does **not** mean there can only be one representation.

There may be:

* cached forms;
* projected forms;
* physical forms;
* indexes;
* serialized forms;
* API views;
* compiled forms;
* materialized outputs.

But those forms must say what authority they derive from.

For example:

```text
SchemaContract           = semantic authority
Arrow Schema             = canonical runtime representation
DFSchema                  = planning-qualified representation
ExecutionPlan schema      = physical execution representation
RecordBatch schema        = runtime realization
Parquet schema            = persisted representation
API schema document       = exposed projection
```

The schema documentation explicitly describes schema as a compiled contract traversing the entire chain from source/Arrow schema through provider, `DFSchema`, logical plan, execution plan, `RecordBatch`, sink, introspection, and tests.

## Authority rule

For every substantive design object, the agent should be able to answer:

```text
Who owns the truth?
Who may mutate it?
Who may derive from it?
How is a derived representation tied back to it?
How is stale derivation detected?
```

If two components can independently decide what the same concept means, the design should normally be rejected.

---

# 5. Principle 4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation

The `CatalogProvider → SchemaProvider → TableProvider` family is especially important as a design exemplar.

The hierarchy is not merely organizational. It establishes **levels of semantic responsibility**:

```text
CatalogProviderList
    ↓ owns catalog namespace

CatalogProvider
    ↓ owns schema namespace

SchemaProvider
    ↓ owns table namespace

TableProvider
    ↓ owns table contract and scan/write behavior
```

The attached schema reference makes these responsibilities particularly explicit: `CatalogProvider` owns schema namespace/visibility/persistence policy; `SchemaProvider` owns table lookup, visibility, and registration semantics; `TableProvider` owns schema, scan behavior, pushdown truthfulness, statistics, write semantics, and access policy.

This is the pattern to emulate elsewhere.

## Design objective

A hierarchy should answer two questions unambiguously:

### What is universal?

For example, every `TableProvider` has:

```text
a schema
a table type
a scan contract
a defined relationship to filters/projections/limits
planning metadata
a defined write posture
```

### What may differ?

Instances may vary in:

```text
where data resides
how scans are executed
what filters can be pushed down
whether writes are supported
what statistics are available
what authorization rules apply
whether data comes from Delta, Parquet, memory, an API, or another backend
```

The consumer interacts with the **shared contract**, not backend-specific branches.

That produces substitutability without pretending implementations are identical.

---

# 6. Principle 5 — Encode variability behind contracts, not throughout consumers

Once a hierarchy exists, consumers should not continually ask:

```rust
if source_is_delta { ... }
else if source_is_parquet { ... }
else if source_is_api { ... }
```

That is a failure to exploit the abstraction.

Prefer:

```text
consumer
   ↓
canonical contract
   ↓
backend-specific implementation
```

Backend-specific knowledge should be localized to the adapter/provider that owns that variability.

A useful design test is:

> **If we introduce another valid implementation of this concept, how many existing modules must change?**

The desired answer is often:

> **None outside registration/configuration and the new implementation itself.**

This is what makes the system a fabric rather than a collection of integrations.

---

# 7. Principle 6 — Separate semantic meaning from execution strategy

This is one of the strongest DataFusion design principles and should be generalized aggressively.

```text
LogicalPlan = what should happen
ExecutionPlan = how it should happen
```

A filter remains semantically a filter regardless of:

* partitioning;
* object store;
* vectorization strategy;
* batch size;
* join implementation;
* spilling;
* parallelism;
* streaming;
* physical file layout.

The planning documentation accordingly treats planning as a lifecycle with distinct parse, bind, analyze, logical optimize, physical-plan, physical-optimize, and execute phases, each producing different artifacts and having different failure classes.

## Generalized rule

For any substantial subsystem, distinguish:

```text
intent
semantic representation
validated representation
physical strategy
runtime execution
observed result
```

Do not contaminate the semantic model with implementation choices unless those choices actually alter semantics.

For example, a model should usually say:

```text
join A to B on key K
```

rather than:

```text
perform an 8-way hash-partitioned HashJoinExec with 16 partitions
```

The latter belongs downstream.

This separation permits optimization without semantic rewriting.

---

# 8. Principle 7 — Build a shared canonical data fabric

“Data fabric” here should not mean a vendor product or merely a data catalog.

It should mean:

> **The system possesses a small number of canonical representations through which data, schemas, queries, storage, and metadata compose across otherwise independent capabilities.**

The Arrow/DataFusion ecosystem exhibits this clearly. The Arrow reference describes a layered stack in which Arrow provides the memory model, `object_store` provides storage semantics, Parquet/IPC/Flight provide persistence or transport, and DataFusion consumes and emits `RecordBatch` streams at the query layer.

The target internal fabric should therefore look conceptually like:

```text
┌───────────────────────────────────────────────┐
│ Domain / semantic models                     │
│ PlanSpec · CalculationSpec · SchemaContract  │
└──────────────────────┬────────────────────────┘
                       ↓
┌───────────────────────────────────────────────┐
│ Catalog / authority plane                     │
│ Catalog → Schema → Table → Function           │
└──────────────────────┬────────────────────────┘
                       ↓
┌───────────────────────────────────────────────┐
│ Logical computation plane                     │
│ Expr · DFSchema · LogicalPlan                 │
└──────────────────────┬────────────────────────┘
                       ↓
┌───────────────────────────────────────────────┐
│ Physical execution plane                      │
│ ExecutionPlan · RecordBatchStream             │
└──────────────────────┬────────────────────────┘
                       ↓
┌───────────────────────────────────────────────┐
│ Common data plane                             │
│ Arrow Schema · Array · RecordBatch            │
└──────────────────────┬────────────────────────┘
                       ↓
┌───────────────────────────────────────────────┐
│ Persistence / transaction plane               │
│ Parquet · object_store · Delta snapshot/log   │
└───────────────────────────────────────────────┘

          ↕ provenance throughout ↕
```

The crucial attribute is that each boundary uses **canonical semantic objects**, not bespoke translations unique to each pair of components.

---

# 9. Principle 8 — Treat the common representation as infrastructure

Arrow is particularly instructive because it makes the data representation itself a compositional primitive.

The Arrow reference characterizes raw Arrow as the interoperability/memory substrate, while DataFusion provides the relational planner and execution engine over that substrate. It explicitly recommends `RecordBatch`, streams, IPC, Parquet, and protocol boundaries rather than unnecessary materialization into another object universe.

General design rule:

> **Prefer a single canonical representation flowing through components over repeated conversion into component-specific internal DTOs.**

For tabular computation, this means preserving Arrow-native representations wherever practicable.

More generally, a system should deliberately choose canonical representations for:

```text
data
schema
expressions
plans
identifiers
versions
provenance
diagnostics
policy decisions
```

This significantly reduces adapter code and semantic mismatch.

---

# 10. Principle 9 — Make provenance intrinsic to every meaningful transformation

Provenance should not be something reconstructed from logs after a failure.

It should be an **automatic output of normal computation**.

A derived artifact should be able to answer:

```text
What produced me?
From what inputs?
At what versions?
Against what schema contracts?
Using what calculations?
Using what plan?
Using what configuration?
Using what software version?
Under what execution/request identity?
When?
Into what committed state?
```

The schema documentation already identifies contract, governance, lineage, quality, and operational metadata as distinct metadata classes and proposes keys such as `source.version`, `lineage.producer`, `lineage.pipeline`, `batch_id`, and `run_id`.

Delta makes the same idea durable at the table-transition boundary. The reference explicitly recommends standardized commit metadata such as:

```text
application_id
application_version
pipeline_name
job_id
source_table_versions
input_snapshot_pin
schema_contract_version
request_id
trace_id
git_sha
build_id
```

and identifies commit metadata as **audit/provenance data**.

## Agent rule

Provenance must be designed **before** the operation is implemented.

Do not accept:

> “We can add tracing later.”

Require instead:

> “What provenance record does this operation produce by construction?”

---

# 11. Principle 10 — Seek provenance closure

A particularly strong target is **provenance closure**:

> Starting from any durable result, an operator should be able to recursively resolve the material facts required to explain how it came into existence.

For example:

```text
output Delta version 184
    ↓
commit metadata
    ↓
execution / request ID
    ↓
physical + logical planning bundle
    ↓
PlanSpec version
    ↓
CalculationSpec versions
    ↓
input table versions
    ↓
input schema fingerprints
    ↓
source objects / snapshots
```

Not every byte of this chain has to be embedded in every artifact. Stable references and fingerprints are sufficient.

What matters is that the chain is **deliberately resolvable**.

Delta's `history()` provides provenance about table writes and supports audit trails, writer identification, rollback investigation, schema-change detection, commit-metadata observability, and run lineage.

---

# 12. Principle 11 — Prefer immutable snapshots and explicit state transitions

One of the cleanest ways to preserve reasonability and provenance is to avoid uncontrolled mutation.

Target:

```text
state N
   + explicit operation
   + explicit inputs
   + explicit policy
       ↓
state N+1
```

rather than:

```text
shared mutable object
   ← arbitrary callers modify fields
```

Arrow reinforces this through immutable columnar data and batch-oriented transformations. Delta reinforces it through table versions and transaction-log-mediated state transitions.

This does **not** mean all runtime structures must literally be immutable.

It means that **semantically significant change** should have:

* a before state;
* an operation;
* an after state;
* a version or identity;
* validation;
* provenance.

A mutable cache is therefore acceptable.

A silently mutable authoritative table definition is much more problematic.

---

# 13. Principle 12 — Schemas are executable contracts, not documentation

Schema should be treated as one of the strongest authorities in the architecture.

A schema contract encompasses more than column names:

```text
field identity
name
type
nullability
ordering
nested structure
semantic annotations
units where applicable
constraints
compatibility policy
schema version
fingerprint
```

The attached schema work explicitly describes schema as a compiled contract from source through provider, logical planning, physical planning, `RecordBatch`, sink, and diagnostics.

## Agent rules

Do not:

```text
infer contracts from example data
use arbitrary map keys as schema
silently widen/narrow types
silently change nullability
silently reorder columns
use expression display strings as durable field names
```

Do:

```text
validate schema at boundaries
make compatibility explicit
fingerprint stable contracts
alias derived fields deterministically
separate source schema from canonical schema
record schema version in provenance
```

Types are part of system behavior.

They are not merely compiler inconvenience.

---

# 14. Principle 13 — Put governance at the authoritative boundary

Security, tenancy, visibility, and policy should be enforced where the relevant semantic authority lives.

Examples:

```text
CatalogProvider
    → namespace visibility

SchemaProvider
    → table visibility

TableProvider::schema()
    → visible columns

TableProvider::scan()
    → tenant predicates / access policy

function registry
    → callable calculation policy

logical-plan validator
    → query policy

write/transaction boundary
    → mutation policy
```

The provider documentation explicitly treats catalog/provider surfaces as governance boundaries and warns against claiming capabilities such as filter pushdown that are not actually enforced.

This is superior to duplicating security decisions throughout arbitrary callers because it makes enforcement **structural**.

---

# 15. Principle 14 — Prefer the highest-level extension that preserves the semantics

Extension points should form a hierarchy.

Do not immediately drop to the most powerful abstraction.

The attached DataFusion material states the preferred progression explicitly:

```text
UDF
  > TableProvider
  > SQL planner hook
  > LogicalPlanBuilder
  > LogicalPlan::Extension
  > ExecutionPlan
  > custom QueryPlanner
```

with the instruction to use the highest-level extension that preserves the required semantics.

The calculation reference expresses the same philosophy: built-in SQL/`Expr` before scalar UDF, UDF before custom physical execution, and specialized aggregate/window/table abstractions rather than forcing all behavior into scalar functions.

## General principle

Prefer:

> **the most declarative representation that fully expresses the requirement**

because higher-level representations generally preserve more:

```text
semantic visibility
optimization opportunity
validation
portability
explainability
security inspection
testability
```

Drop lower only when the semantics actually require it.

---

# 16. Principle 15 — Preserve optimizer visibility

A custom abstraction is not automatically a better abstraction.

If introducing one hides useful semantic structure, it may make the system worse.

For example:

```text
amount > 1000 AND status = 'paid'
```

is more visible to an optimizer than:

```text
is_high_value_paid_order(amount, status)
```

when the UDF is opaque.

The calculation guide therefore recommends transparent `Expr` composition for such cases and reserves UDFs for true domain kernels or behavior that cannot be cleanly represented by built-ins.

## Agent test

Before introducing an abstraction, ask:

> **What semantic information becomes invisible once I wrap this?**

Good encapsulation hides implementation detail.

Bad encapsulation hides information other system components need for reasoning.

---

# 17. Principle 16 — Treat lifecycle phases as first-class architecture

Operations should move through explicit phases.

A good generalized lifecycle is:

```text
declare
    ↓
resolve
    ↓
validate
    ↓
normalize
    ↓
compile
    ↓
optimize
    ↓
authorize
    ↓
execute
    ↓
verify
    ↓
commit
    ↓
observe
```

Not every subsystem needs all phases, but important operations should make their phase boundaries visible.

Benefits include:

* better error taxonomy;
* easier agent debugging;
* deterministic hooks;
* easier testing;
* explicit policy gates;
* clean provenance;
* ability to inspect intermediate artifacts.

A failure should therefore identify its phase:

```text
schema_binding
type_validation
logical_planning
policy_validation
physical_planning
execution
write_validation
commit
```

rather than merely returning:

```text
operation failed
```

---

# 18. Principle 17 — Make intermediate artifacts inspectable and reproducible

The plan should not disappear between “request accepted” and “result produced.”

The planning reference proposes a reproducible bundle containing the input query/`PlanSpec`, configuration, library versions, feature flags, catalog snapshot, schemas, statistics, object-store metadata, logical plans, physical plans, explain output, runtime metrics, output schema, and row-count summaries.

Apply this philosophy broadly.

For important transformations, preserve or make reconstructible:

```text
input semantic spec
resolved dependencies
validated model
compiled representation
configuration snapshot
software versions
input versions
output contract
diagnostics
metrics
result/commit identity
```

This gives both human engineers and programming agents the ability to reason about what actually occurred.

---

# 19. Principle 18 — Fingerprint anything whose identity matters

Human-readable names are rarely enough for strong reproducibility.

Stable or deterministic fingerprints should be considered for:

```text
schema contracts
calculation specs
plan specs
logical plans
function registries
catalog snapshots
configuration sets
source snapshots
dependency environments
policy sets
```

The planning documentation already proposes hashes for SQL/AST/logical plans/configuration along with catalog/schema and function-registry versions when designing plan caching and invalidation.

A useful conceptual identity is therefore:

```text
ArtifactIdentity {
    semantic_id,
    semantic_version,
    fingerprint,
    environment_fingerprint,
}
```

This allows the system to answer the critical question:

> “Is this actually the same thing?”

rather than relying on labels.

---

# 20. Principle 19 — Make reproducibility a normal operating mode

Reproducibility should not require forensic work.

Given a meaningful past result, the architecture should strive to recover:

```text
input versions
schema versions
calculation versions
query/model spec
configuration
software/library versions
execution environment
output version
```

Exact reproducibility will not always be achievable—external services and volatile operations are obvious exceptions—but **reproducibility status itself should be modeled**.

For example:

```text
Reproducibility {
    deterministic: true,
    inputs_pinned: true,
    external_dependencies_pinned: true,
    volatile_functions: false,
    environment_recorded: true,
}
```

That is much more useful than an undocumented assumption that a calculation “should probably reproduce.”

---

# 21. Principle 20 — Be conservative about claimed capabilities

Metadata that influences execution must be truthful.

This is especially important for properties such as:

```text
filter pushdown
projection pushdown
ordering
partitioning
uniqueness
constraints
statistics
nullability
determinism
function volatility
idempotency
```

A false optimization hint can be worse than no hint.

The correct principle is:

> **Unknown is preferable to falsely known.**

If an implementation cannot guarantee a capability, advertise it as unavailable or uncertain.

Never invent optimizer-relevant facts merely to improve performance.

---

# 22. Principle 21 — Separate enforced semantics from advisory metadata

Metadata is valuable, but it must have a defined semantic class.

The schema reference explicitly warns that metadata is not a substitute for `DataType`, nullability, qualifiers, constraints, or runtime validation; it is an annotation channel for lineage, governance, units, semantic types, display, and audit.

Distinguish:

```text
Enforced
  types
  constraints
  validation rules
  access policies

Planner-consumed
  statistics
  ordering
  partitioning
  pushdown support

Contractual metadata
  semantic type
  units
  schema version
  field identity

Governance metadata
  classification
  retention
  masking

Lineage metadata
  producer
  source version
  run identity

Advisory metadata
  display name
  precision hint
  description
```

An agent must never assume that writing a metadata key causes the runtime to enforce it.

---

# 23. Principle 22 — Use protocols and canonical boundaries for interoperability

Interoperability should happen at deliberate protocol boundaries rather than through accidental object conversion.

The Arrow stack embodies this with:

```text
RecordBatch / RecordBatchReader
Arrow IPC
Parquet
C Data Interface
C Stream Interface
PyCapsule
Flight
Substrait
```

The Arrow reference specifically recommends Arrow IPC/Parquet for file interchange, PyCapsule/C Stream for Python in-process interoperability, and `RecordBatchReader` for Rust streaming rather than unnecessary row or pandas materialization.

General rule:

> **Integrate through stable semantic protocols whenever possible; write pairwise adapters only when no common boundary exists.**

---

# 24. Principle 23 — Keep state ownership local and explicit

A common failure mode in large systems is unclear state ownership.

Each stateful concern should declare:

```text
scope
owner
lifetime
mutability
refresh policy
concurrency policy
invalidation policy
authority relationship
```

Useful scopes include:

```text
process
runtime
session
tenant
query
transaction
batch
partition
request
```

Caches in particular must never silently become authorities.

A cache should conceptually be:

```text
CacheEntry {
    derived_from,
    source_version,
    fingerprint,
    created_at,
    invalidation_policy,
    value,
}
```

not merely:

```text
HashMap<Key, Value>
```

This turns caching into a controlled optimization rather than a second source of truth.

---

# 25. Principle 24 — Make observability semantic, not merely operational

Traditional observability asks:

```text
How long did this function run?
Did it error?
How much memory did it use?
```

The stronger system also asks:

```text
Which table versions were read?
Which schema was bound?
Which calculation versions were invoked?
Which logical plan was chosen?
Which physical strategy resulted?
Which predicates were pushed down?
Which configuration affected planning?
Which commit did the write produce?
```

`EXPLAIN`, logical/physical plans, schema snapshots, metrics, Delta history, and commit metadata collectively demonstrate this richer notion.

Observability should therefore cover both:

```text
runtime observability
+
semantic observability
```

---

# 26. Principle 25 — Make testing derive from contracts and invariants

Every contract should suggest its own tests.

If a `TableProvider` promises:

```text
schema
projection
filter pushdown
statistics
write semantics
```

then tests should prove those claims.

If a calculation declares:

```text
type policy
null policy
units
determinism
```

tests should verify them.

If a plan model declares:

```text
logical semantics
```

tests should compare:

```text
unoptimized result
optimized result
serialized/deserialized result
physical execution result
```

The preferred test architecture therefore follows the models.

Do not write tests only around whichever functions happen to exist.

---

# 27. The overall architectural pattern

The combined philosophy can be summarized as:

```text
                 SEMANTIC AUTHORITY
                       │
                       ▼
               explicit typed models
                       │
        ┌──────────────┼───────────────┐
        │              │               │
        ▼              ▼               ▼
     schemas       calculations       plans
        │              │               │
        └──────────────┼───────────────┘
                       ▼
              contract validation
                       ▼
               catalog resolution
                       ▼
               logical compilation
                       ▼
                  optimization
                       ▼
               physical execution
                       ▼
              canonical Arrow data
                       ▼
          transactional persistence
                       ▼
               versioned state
                       │
                       ▼
            provenance + diagnostics
```

The design goal is not maximum abstraction.

It is **maximum semantic coherence with minimum duplication of authority**.

---

# 28. Mandatory design questions for an LLM programming agent

Before implementing or materially revising a subsystem, the agent should answer the following.

| Question                                                 | Required design outcome                              |
| -------------------------------------------------------- | ---------------------------------------------------- |
| **What semantic concept is being represented?**          | Explicit model or explanation why one is unnecessary |
| **What is the authoritative representation?**            | One clearly named authority                          |
| **What derived representations exist?**                  | Explicit derivation relationships                    |
| **What is invariant?**                                   | Machine-testable contracts                           |
| **What may implementations vary?**                       | Explicit extension points                            |
| **What hierarchy does this concept belong to?**          | Parent/child responsibilities                        |
| **What lifecycle phases does it pass through?**          | Phase boundaries and artifacts                       |
| **What is logical vs physical?**                         | Semantic and implementation concerns separated       |
| **What common fabric type should cross boundaries?**     | Canonical representation rather than bespoke DTO     |
| **How is provenance captured?**                          | Automatic lineage/version/operation identity         |
| **How is identity/versioning represented?**              | IDs, versions, fingerprints                          |
| **Where is policy enforced?**                            | Authority-bound enforcement point                    |
| **How are capabilities advertised?**                     | Explicit and conservative contract                   |
| **How is drift detected?**                               | Validation/fingerprint/version comparison            |
| **How is the operation explained later?**                | Inspectable artifacts and diagnostics                |
| **Can an existing higher-level abstraction express it?** | Reuse before lower-level extension                   |
| **What proves the contract?**                            | Derived unit/integration/property/golden tests       |

If several of these questions have no clear answer, implementation should normally stop at the design stage.

---

# 29. Anti-patterns agents should actively reject

### Hidden semantic logic

```text
Business/domain meaning exists primarily in procedural branches.
```

**Replace with:** explicit semantic models compiled into behavior.

### Multiple authorities

```text
Schema defined in Rust struct + SQL migration + JSON config + writer code independently.
```

**Replace with:** one contract with generated/derived representations.

### Backend leakage

```text
Every consumer branches on Parquet vs Delta vs API vs memory.
```

**Replace with:** provider abstraction exposing a common contract.

### Opaque abstraction

```text
Wrap transparent Expr logic in a UDF merely for code organization.
```

**Replace with:** reusable expression builders; retain optimizer visibility.

### Premature physicalization

```text
Model hard-codes partitioning, join algorithm, concurrency, or storage mechanism.
```

**Replace with:** logical requirement first, physical planning later.

### Provenance afterthought

```text
Only application logs explain where data came from.
```

**Replace with:** provenance IDs/versions/fingerprints emitted with the operation.

### Mutable authority

```text
Shared authoritative objects may be silently changed by arbitrary callers.
```

**Replace with:** controlled state transitions and versioned snapshots.

### Metadata theater

```text
A metadata tag claims an invariant the runtime does not enforce.
```

**Replace with:** distinguish enforced, planner-consumed, contractual, and advisory metadata.

### Pairwise integration explosion

```text
A↔B, A↔C, B↔C each get custom data structures.
```

**Replace with:** a canonical fabric/protocol boundary.

### Capability overclaiming

```text
Provider claims exact pushdown/statistics/order it cannot guarantee.
```

**Replace with:** conservative capability reporting.

---

# 30. Compact agent design constitution

The following can be placed directly into an LLM programming agent's design instructions:

> **Architecture objective:** Design the system as a model-first, contract-driven, provenance-native data fabric. Important semantics should exist as explicit typed models that are validated and then compiled/interpreted into execution, rather than being embedded primarily in procedural control flow.
>
> **Authority:** Establish exactly one authoritative owner for each semantic concept. Other forms must be explicit derived representations, snapshots, caches, projections, serialized forms, or compiled forms tied to the authority by stable identity, version, or fingerprint.
>
> **Contracts:** Define the invariants shared by all implementations separately from the dimensions implementations are allowed to vary. Encode these shared guarantees through typed interfaces/traits and validate them at boundaries.
>
> **Hierarchy:** Prefer coherent conceptual hierarchies in which each level has one clear responsibility, analogous to catalog → schema → table/provider. Consumers should depend on the common contract rather than branch on implementation type.
>
> **Model vs execution:** Separate logical meaning from physical strategy. Domain/query semantics belong in semantic models, expressions, and logical plans; execution algorithms, partitioning, parallelism, storage mechanics, and resource choices belong in physical planning/execution layers.
>
> **Common fabric:** Reuse canonical representations across subsystem boundaries. For tabular data, prefer Arrow schemas, arrays, `RecordBatch`es, and streams rather than subsystem-specific row/DTO formats. Prefer standard interoperability protocols over pairwise conversion code.
>
> **Extensibility:** Use the highest-level existing abstraction that fully preserves the required semantics. Prefer built-ins and transparent expressions before UDFs; UDFs/providers before custom logical operators; logical operators before custom physical operators/planners. Do not recreate functionality already supplied by the underlying libraries.
>
> **Schema:** Treat schema, typing, nullability, constraints, field identity, and compatibility as executable contracts. Validate schemas during planning and again at runtime boundaries. Do not allow silent schema drift.
>
> **Provenance:** Make provenance an intrinsic output of data operations. Capture source identities and versions, schema-contract versions/fingerprints, calculation/model versions, plan identity, configuration/environment identity, request/run/trace IDs, and resulting transaction/version where applicable.
>
> **State:** Prefer immutable/versioned semantic snapshots and explicit state transitions. Mutable caches and runtime state must have declared ownership, scope, lifetime, refresh, and invalidation semantics and must never silently become alternative authorities.
>
> **Planning:** Treat complex operations as lifecycle pipelines with explicit phases such as declaration, resolution, validation, logical compilation, optimization, physical planning, execution, verification, and commit. Preserve inspectable intermediate artifacts where they materially aid reproducibility or diagnostics.
>
> **Truthfulness:** Never advertise optimizer/runtime capabilities—pushdown, ordering, statistics, constraints, determinism, idempotency, nullability, etc.—unless the implementation guarantees them. Unknown is safer than incorrect metadata.
>
> **Governance:** Apply access control, policy, tenancy, and write authority at the layer that owns the relevant semantic contract rather than scattering policy branches throughout consumers.
>
> **Reproducibility:** Give important semantic artifacts stable IDs, versions, and deterministic fingerprints. Record enough environment and dependency information to reproduce or diagnose past results.
>
> **Observability:** Make semantic observability first-class alongside runtime metrics. The system should expose what model/schema/plan/source versions were used, what execution was selected, and what state transition resulted.
>
> **Testing:** Derive tests from contracts and invariants. For every claimed property, create evidence that it holds across relevant planning, optimization, execution, serialization, persistence, and version-change boundaries.
>
> **Design review requirement:** Before coding, explicitly document the semantic model, authoritative owner, hierarchy, invariants, legal variation, lifecycle phases, common boundary types, provenance model, policy enforcement point, extension level, validation strategy, and test evidence. If these cannot be stated clearly, continue designing before implementing.

---

# 31. Short form

If one sentence needs to define the whole philosophy:

> **Represent meaning explicitly, assign each meaning a single authority, compose the system through typed hierarchical contracts and common canonical data representations, separate semantics from execution, and make every state transformation inherently versioned, inspectable, reproducible, and provenance-complete.**

An even shorter design maxim is:

> **Model the truth once; derive behavior from it; preserve its lineage everywhere.**
