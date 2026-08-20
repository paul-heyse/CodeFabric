## Overall verdict

The attached documentation already has **strong coverage of DataFusion planning as individual surfaces**: SQL/DataFrame entry, `LogicalPlan`, `ExecutionPlan`, optimizer phases, physical execution, custom SQL planning, custom logical/physical operators, streaming execution, explainability, testing, and plan serialization. The main gap is that planning is **distributed across many chapters rather than treated as a unified lifecycle with phase boundaries, artifacts, metadata contracts, and agent-safe implementation patterns**.

The current doc already defines the canonical pipeline as:

`SQL/DataFrame/LogicalPlanBuilder → LogicalPlan → logical optimization → physical planning → physical optimization → ExecutionPlan → SendableRecordBatchStream → RecordBatch` 

It also gives a more detailed phase chain from `sqlparser::AST` through `SqlToRel`, analyzer rules, optimizer rules, physical planner, physical optimizer, `ExecutionPlan::execute`, and `SendableRecordBatchStream` . So the gap is **not “planning is missing.”** The gap is that planning needs a dedicated, agent-facing documentation family that explains how these pieces are composed into a governed, inspectable, reproducible planning system.

---

# Existing planning coverage map

## Well covered

The doc already has explicit planning primitives and vocabulary: `SessionContext`, `SessionState`, `TaskContext`, `DataFrame`, `Expr`, `LogicalPlan`, `ExecutionPlan`, `SendableRecordBatchStream`, `RecordBatch`, and `TableProvider` are defined as the minimum vocabulary for planning and execution work .

It also has full deep-dive sections for the main planning layers:

* **Section 19 — Logical plans**: logical operators, `LogicalPlanBuilder`, extension nodes, plan traversal, placeholders, logical-to-physical planning, policy validation.
* **Section 20 — Physical plans and execution operators**: `ExecutionPlan`, `PlanProperties`, partitioning, ordering, children, `execute`, streams, metrics, physical expressions, resource reservations.
* **Section 21 — Streaming execution model**: streaming by `RecordBatch`, batch size, backpressure, partitioned streams, bounded/unbounded sources, pipeline breakers.
* **Section 22 — Query optimizer**: analyzer, logical optimizer, physical optimizer, pushdown, join optimization, statistics, custom rules.
* **Section 23 — Join algorithms and tuning**.
* **Section 25 — Extending SQL syntax**.
* **Section 26 — Custom logical and physical operators**.
* **Section 30 — Metrics, profiling, and explainability**.
* **Section 32 — Testing and correctness**.
* **Section 36 — Plan serialization and interoperability**.

The logical plan chapter already frames `LogicalPlan` as the semantic relational program that flows into analyzer/optimizer, physical planner, and `ExecutionPlan` . The physical plan chapter already frames physical planning as `Optimized LogicalPlan → PhysicalPlanner → ExecutionPlan`, with concrete operator choices, `PhysicalExpr`, `PlanProperties`, and stream execution . The optimizer chapter already distinguishes analyzer, logical optimizer, physical planner, and physical optimizer phases .

## Main gap pattern

The doc is **chapter-complete but lifecycle-fragmented**. An LLM programming agent can find the pieces, but there is not yet a single planning-centered sequence that answers:

```text
Given SQL / DataFrame / custom PlanSpec:
  1. How is the plan created?
  2. What is unresolved vs bound?
  3. What schemas and metadata are needed?
  4. What validation gates happen before optimization?
  5. What optimizer artifacts should be captured?
  6. How is the physical plan chosen?
  7. How do ordering, partitioning, boundedness, and memory shape execution?
  8. How do we test, cache, serialize, diff, and explain the plan?
```

---

# Proposed new planning deep-dive topics

I would add these as a new planning-focused block after the existing core planning sections, probably after Section 23 or after Section 36 depending on whether you want conceptual order or implementation order.

---

## 41) End-to-end planning lifecycle and phase boundary map

**Purpose:** unify scattered planning material into one canonical “compiler pipeline” chapter.

Deep dive:

* Planning entrypoints:

  * `SessionContext::sql`
  * `DataFrame` transformations
  * `LogicalPlanBuilder`
  * `execute_logical_plan`
  * custom query language → `LogicalPlan`
  * `SqlToRel`
  * `QueryPlanner`
* Phase boundaries:

  * parse
  * bind / resolve
  * analyze
  * logical optimize
  * physical plan
  * physical optimize
  * execute
* Artifact at each phase:

  * SQL text
  * sqlparser AST
  * unresolved expression fragments
  * `Expr`
  * `DFSchema`
  * initial `LogicalPlan`
  * analyzed `LogicalPlan`
  * optimized `LogicalPlan`
  * initial `ExecutionPlan`
  * optimized `ExecutionPlan`
  * `SendableRecordBatchStream`
* State object at each phase:

  * `SessionContext`
  * `SessionState`
  * `ExecutionProps`
  * `ConfigOptions`
  * `TaskContext`
  * `RuntimeEnv`
* Error boundary:

  * parse error
  * name-resolution error
  * type-coercion error
  * analysis error
  * logical optimizer error
  * physical planning error
  * physical optimizer error
  * execution error

Why high value: this becomes the **single mental model chapter** for LLM agents. The current doc has the pipeline, but not yet the full artifact/state/error-contract matrix.

---

## 42) SQL planner and binder internals: `sqlparser` AST → `SqlToRel` → `LogicalPlan`

**Purpose:** make SQL planning understandable beyond “call `ctx.sql`.”

Deep dive:

* `sqlparser` AST statement classes.
* `SqlToRel` as SQL AST → `LogicalPlan` / `Expr` conversion.
* `ContextProvider` as metadata boundary.
* `PlannerContext` / relation planning context.
* table resolution:

  * catalog
  * schema
  * table
  * aliases
  * CTEs
  * views
  * subqueries
* column resolution:

  * unqualified columns
  * qualified columns
  * ambiguous columns
  * duplicate names
  * join output names
* expression binding:

  * literals
  * function lookup
  * aggregate validation
  * window validation
  * type coercion
  * casts
  * placeholders
* query-shape binding:

  * `SELECT *`
  * `* EXCEPT`
  * CTE expansion
  * subquery aliasing
  * correlated subqueries
  * lateral joins
  * `GROUP BY`
  * `HAVING`
  * `QUALIFY`
* planner extension hooks:

  * `ExprPlanner`
  * `TypePlanner`
  * `RelationPlanner`
  * parser wrappers when planner hooks are insufficient.

Existing anchor: Section 25 already introduces SQL extension planning and says `SqlToRel` converts SQL AST to `LogicalPlan` / `Expr`, with extension planners operating inside `SqlToRel` . The missing piece is a **non-extension binder deep dive**.

---

## 43) Programmatic logical planning with `DataFrame`, `Expr`, and `LogicalPlanBuilder`

**Purpose:** document the preferred non-SQL planning path for generated queries.

Deep dive:

* When to choose:

  * SQL
  * DataFrame API
  * direct `LogicalPlanBuilder`
  * fully custom `LogicalPlan`
* Plan factory patterns:

  * source factory
  * projection factory
  * filter factory
  * aggregation factory
  * join factory
  * window factory
  * sink/write factory
* Avoiding SQL string generation.
* Validating dynamic column names.
* Using `DFSchema` to preflight generated expressions.
* Building reusable plan templates.
* Parameterized plans:

  * placeholders
  * `with_param_values`
  * parameter type validation
* Output schema stability:

  * aliases
  * deterministic expression names
  * duplicate-name prevention.
* Pattern: app request → typed query spec → `Expr`/`LogicalPlanBuilder` → optimized plan → stream.

Why high value: this supports your semantic/PlanSpec direction directly: generated plans should often target `Expr`/`LogicalPlanBuilder`, not SQL text.

---

## 44) Plan schema, column identity, aliases, and qualifier governance

**Purpose:** make `DFSchema` and output identity a planning topic, not only a data-model topic.

Deep dive:

* `DFSchema` vs Arrow `Schema`.
* Relation qualifiers.
* Column identity:

  * catalog/schema/table
  * alias
  * projection alias
  * expression-derived name
* Join output identity:

  * duplicate field names
  * left/right qualifiers
  * ambiguous unqualified references
* Output schema contracts:

  * public API output
  * internal plan output
  * sink/write output
  * view output
* Naming hazards:

  * aggregate names
  * function names
  * cast names
  * expression display strings
* Alias preservation during rewrites.
* Schema diffing:

  * field name
  * type
  * nullability
  * metadata
  * qualifier
* Recommended agent policy:

  * always alias computed expressions
  * preserve qualifiers until final projection
  * normalize public output names late
  * never use expression display strings as API contracts.

Why high value: schema/name instability is one of the highest-friction failure modes for generated plans and golden tests.

---

## 45) Expression lifecycle: unresolved SQL expression → bound `Expr` → physical expression

**Purpose:** connect expression syntax, binding, type coercion, optimizer rewrites, and physical expression execution.

Deep dive:

* Expression source forms:

  * SQL AST expression
  * DataFrame `Expr`
  * `LogicalPlanBuilder` expression
  * UDF expression
  * optimizer-generated expression
* Binding:

  * column resolution
  * function resolution
  * aggregate/window context
  * placeholder resolution
* Type inference:

  * `ExprSchemable::get_type`
  * `nullable`
  * `to_field`
* Coercion:

  * numeric coercion
  * string type behavior
  * decimal coercion
  * timestamp coercion
  * struct/list coercion
  * null coercion
* Rewrite lifecycle:

  * constant folding
  * simplification
  * cast insertion
  * alias stripping/preservation
  * volatility constraints
* Physical lowering:

  * `Expr` → `PhysicalExpr`
  * `ColumnarValue`
  * scalar vs array
  * null handling
  * physical function implementations
* Testing:

  * expression type tests
  * nullability tests
  * physical output tests
  * error tests.

Why high value: the current expression chapter is excellent for syntax, but planning needs a **binding/coercion/physical-lowering** view.

---

## 46) Logical plan validation and policy linting before optimization/execution

**Purpose:** turn plan inspection into a first-class security/governance stage.

Deep dive:

* Why validate logical plans:

  * table allowlists
  * column allowlists
  * function allowlists
  * DDL/DML denial
  * unbounded scan denial
  * direct-path scan denial
  * cross join denial
  * result cap enforcement
  * partition filter requirement
  * expensive function denial
* `SQLOptions` vs logical-plan validation.
* Traversing `LogicalPlan`.
* Traversing expressions inside nodes.
* Capturing table references.
* Capturing column references.
* Capturing UDF/function references.
* Detecting:

  * `TableScan`
  * `CrossJoin`
  * `Dml`
  * `Ddl`
  * `Copy`
  * `Extension`
  * `Limit` absence
  * `Sort` without `Limit`
  * `Unnest`
* Plan lint result schema:

  * severity
  * code
  * node path
  * expression path
  * remediation
* Deployment patterns:

  * read-only SQL service
  * tenant-scoped catalog
  * internal batch job
  * admin mode.

Existing anchor: Section 19 already includes a logical-plan policy validation example for denying scans outside an allowlist and collecting column references, but this deserves a standalone policy/linting chapter .

---

## 47) Planner metadata: statistics, constraints, functional dependencies, partitioning, and ordering

**Purpose:** document the metadata that makes planning intelligent.

Deep dive:

* Metadata sources:

  * `TableProvider::statistics`
  * Parquet metadata
  * catalog/metastore statistics
  * custom provider estimates
  * Hive partitions
  * known ordering
  * known partitioning
  * constraints
  * functional dependencies
* Statistics:

  * row count
  * byte size
  * column min/max
  * null count
  * distinct count
  * exact vs estimated
  * stale vs fresh
* Constraints:

  * uniqueness
  * non-null
  * range/domain constraints
  * foreign-key-like app metadata
* Functional dependencies:

  * grouping simplification
  * distinct/aggregate reasoning
  * projection simplification
* Ordering metadata:

  * source sortedness
  * `WITH ORDER`
  * Parquet/file sort assumptions
  * false ordering risk
* Partitioning metadata:

  * physical partitions
  * file partitions
  * Hive partitions
  * repartitioning requirements
* Metadata quality policy:

  * unknown is safer than wrong
  * exact vs approximate
  * invalidation
  * privacy/security implications.

Existing anchor: Section 22 already says statistics support pruning, cost decisions, and custom optimizer rules, with sources including `TableProvider::statistics`, Parquet metadata, catalog row counts, custom estimates, and runtime metrics . The gap is a comprehensive planner-metadata chapter.

---

## 48) Analyzer and logical optimizer rule cookbook

**Purpose:** go beyond “there are optimizer rules” into exact rule-author patterns.

Deep dive:

* Analyzer vs logical optimizer:

  * semantic validity vs semantic-preserving optimization.
* Rule traits.
* Rule execution order.
* Fixed-point behavior.
* Rule idempotence.
* Rule preconditions.
* Rule output invariants:

  * schema unchanged or intentionally changed
  * expression names preserved
  * aliases preserved
  * qualifiers preserved
  * statistics updated or invalidated
* Common logical rewrites:

  * predicate pushdown
  * projection pushdown
  * limit pushdown
  * expression simplification
  * subquery decorrelation
  * join reordering
  * aggregate simplification
* Custom rule skeletons:

  * expression rewrite
  * plan-node rewrite
  * provider-aware rewrite
  * extension-node rewrite
* Rule testing:

  * input plan snapshot
  * optimized plan snapshot
  * result equivalence
  * negative cases
  * alias/name stability
  * rule-order hazards.

Why high value: the current optimizer section is broad; a cookbook would make it implementation-ready for agents modifying optimizer behavior.

---

## 49) Physical planning and logical-to-physical lowering map

**Purpose:** document how each logical node becomes physical execution nodes.

Deep dive:

* `SessionState::create_physical_plan`.
* `PhysicalPlanner`.
* `QueryPlanner`.
* Logical-to-physical mapping table:

  * `TableScan` → `DataSourceExec` / custom scan exec
  * `Projection` → `ProjectionExec`
  * `Filter` → `FilterExec`
  * `Aggregate` → aggregate exec variants
  * `Sort` → `SortExec`
  * `Limit` → limit/global/local limit exec
  * `Join` → hash/sort-merge/nested loop/symmetric hash
  * `Union` → union exec
  * `Window` → window exec
  * `Repartition` → repartition exec
  * `Extension` → extension planner/custom exec
* Physical expression planning.
* Distribution requirements.
* Ordering requirements.
* Config-sensitive choices.
* Provider-sensitive choices.
* Error cases:

  * no physical implementation
  * unsupported extension node
  * missing sort requirement
  * incompatible partitioning
  * unsupported unbounded operator.

Existing anchor: Section 20 says logical plans cannot be executed directly and must be compiled into `ExecutionPlan`s, with `SessionState::create_physical_plan` as the explicit compile path . The gap is a node-by-node lowering matrix.

---

## 50) Physical plan properties: partitioning, ordering, equivalence, boundedness, and emission

**Purpose:** make `PlanProperties` and `ExecutionPlanProperties` a standalone planning contract.

Deep dive:

* `PlanProperties`:

  * `EquivalenceProperties`
  * `Partitioning`
  * `EmissionType`
  * `Boundedness`
* Output partitioning:

  * unknown
  * round-robin
  * hash partitioning
  * single partition
* Ordering:

  * per-partition ordering
  * global ordering
  * sort preservation
  * false-ordering correctness bugs
* Equivalence properties:

  * column equivalences
  * sort equivalences
  * expression equivalences
  * optimizer use cases
* Boundedness:

  * finite scans
  * unbounded sources
  * streaming queries
* Emission type:

  * incremental
  * final / blocking
  * pipeline breaker behavior
* Required child properties:

  * required distribution
  * required ordering
  * repartition insertion
  * sort insertion
* Testing:

  * property accuracy tests
  * incorrect metadata negative tests
  * explain verification.

Existing anchor: Section 20 already introduces `PlanProperties`, including equivalence properties, partitioning, emission type, boundedness, and agent rules warning that false ordering/partitioning metadata can produce incorrect query results . This deserves a deeper dedicated chapter because property mistakes are correctness bugs, not just performance bugs.

---

## 51) Scan planning and source pushdown: `TableProvider`, file scans, and custom sources

**Purpose:** connect source registration to planner behavior.

Deep dive:

* `TableProvider::scan`.
* Projection pushdown.
* Filter pushdown:

  * exact pushdown
  * inexact pushdown
  * unsupported filters
  * residual filter handling
* Limit pushdown.
* Sort/order pushdown.
* Statistics exposure.
* Constraints exposure.
* Partitioning exposure.
* File scan planning:

  * listing
  * partition discovery
  * file groups
  * object-store registry
  * metadata cache
  * statistics cache
  * Parquet row group/page/bloom pruning
* Custom remote source planning:

  * API filters
  * index filters
  * pagination
  * async stream construction
  * backpressure
* Correctness rules:

  * never drop residual filters
  * preserve SQL null semantics
  * conservative pushdown reporting
  * unknown stats over false stats.

Why high value: source planning is where most real-world performance comes from, especially for Parquet/object stores/custom APIs.

---

## 52) Join planning decision model

**Purpose:** make join planning explainable and tunable.

Deep dive:

* Logical join anatomy:

  * join type
  * join keys
  * join filter
  * null semantics
  * output schema
* Join algorithm choice:

  * hash join
  * sort-merge join
  * nested loop
  * symmetric hash
  * piecewise merge
  * cross join
* Cost inputs:

  * row counts
  * byte sizes
  * key cardinality
  * null counts
  * ordering
  * partitioning
  * memory budget
* Repartitioning:

  * join key repartition
  * coalescing
  * broadcast-like patterns if available/custom.
* Build/probe side selection.
* Dynamic join filters.
* Spill behavior.
* Semi/anti join rewrites.
* Lateral/correlated join planning.
* Explain diagnostics:

  * how to read join physical plan
  * join metrics
  * spill metrics
  * build/probe timing.
* Query-shape rewrites:

  * `EXISTS` → semi join
  * `NOT EXISTS` → anti join
  * filter before join
  * cast keys before join
  * avoid functions around join keys.

Existing anchor: Section 22 already sketches join optimization as statistics + predicates + config → join order/algorithm/repartition/sort requirements . Section 23 is likely already strong; the incremental gap is an explicit **decision model / trace model**.

---

## 53) Streaming topology, boundedness, and pipeline-breaker planning

**Purpose:** recast streaming execution as a planning concern.

Deep dive:

* Physical plan as streaming topology.
* Partition streams.
* Backpressure.
* Batch size.
* Bounded vs unbounded source propagation.
* Pipeline-friendly operators:

  * projection
  * filter
  * simple map
  * streaming scan
* Pipeline breakers:

  * full sort
  * global aggregate
  * some joins
  * some windows
  * collect
* Partial pipeline breakers:

  * local/global aggregate split
  * TopK
  * partitioned sort
* Streaming-safe SQL patterns.
* Unbounded planning restrictions.
* Planner errors for unsupported unbounded queries.
* Service endpoint implications:

  * `execute_stream`
  * cancellation
  * chunk serialization
  * resource cleanup.

Why high value: the current streaming chapter is useful; this topic would explain how streaming constraints should affect plan generation and validation.

---

## 54) Runtime execution planning: partitions, task scheduling, memory reservations, and spill

**Purpose:** bridge physical planning and runtime execution.

Deep dive:

* `target_partitions`.
* `batch_size`.
* partition-to-stream mapping.
* `TaskContext`.
* `RuntimeEnv`.
* object-store registry.
* memory pool.
* disk manager.
* spill planning:

  * sort
  * hash join
  * group by
* memory reservations in custom operators.
* cancellation model.
* blocking vs async boundaries.
* CPU runtime vs network I/O runtime.
* service concurrency budgeting:

  * per-query memory
  * per-tenant memory
  * per-request timeout
  * max concurrent partitions
  * temp dir quotas.

Existing anchor: Section 20 already has resource reservation patterns and warns not to bypass memory pools for unbounded buffers . The gap is a runtime-planning chapter that connects these mechanics to query admission and service concurrency.

---

## 55) Planning artifact package: reproducible plan debug bundle

**Purpose:** define what to persist for debugging, regression, and LLM-agent handoff.

Deep dive:

* Artifact bundle contents:

  * input SQL / PlanSpec / query spec
  * parser dialect
  * `ConfigOptions`
  * DataFusion version
  * Arrow version
  * feature flags
  * catalog snapshot
  * table schemas
  * table statistics
  * object-store path metadata
  * unoptimized logical plan
  * analyzed logical plan if exposed
  * optimized logical plan
  * physical plan before physical optimizer if exposed
  * optimized physical plan
  * `EXPLAIN`
  * `EXPLAIN VERBOSE`
  * `EXPLAIN ANALYZE`
  * metrics
  * output schema
  * row-count summary
* Artifact formats:

  * markdown
  * JSON
  * protobuf
  * compact text snapshots
* Golden test policy:

  * logical snapshots
  * physical snapshots
  * result snapshots
  * metrics thresholds
  * version pinning.
* Agent reveal protocol:

  * summary first
  * plan tree next
  * node details on demand
  * metrics last.

Why high value: the doc has testing and explainability sections, but not a canonical **planning artifact bundle** for regression and agent workflows.

---

## 56) Plan serialization, caching, fingerprints, and invalidation

**Purpose:** extend Section 36 from interoperability into operational plan reuse.

Deep dive:

* What can be serialized:

  * `Expr`
  * `LogicalPlan`
  * `ExecutionPlan`
  * `PhysicalExpr`
* Native proto vs Substrait.
* Why serialized plans are not automatically durable contracts.
* Plan cache key design:

  * SQL text hash
  * normalized AST hash
  * logical plan hash
  * config hash
  * catalog/schema version
  * function registry version
  * table statistics version
  * DataFusion version
  * feature flag set
* Parameterized plans:

  * placeholders
  * parameter type signatures
  * parameter binding
* Invalidation triggers:

  * schema change
  * function change
  * config change
  * optimizer version change
  * stats refresh
  * object-store listing change
* Security:

  * plan cache poisoning
  * tenant leakage
  * stale authorization
  * serialized extension nodes.
* Storage guidance:

  * durable cache stores SQL + version + config + catalog snapshot
  * serialized plan only with explicit versioning/invalidation.

Existing anchor: Section 36 already says `datafusion_proto` has broad DataFusion-specific fidelity, Substrait is interoperability-oriented with incomplete coverage, and durable storage/long-lived caches should store SQL plus version/config/catalog snapshot rather than blindly storing serialized plans . The gap is a dedicated **cache key / invalidation / fingerprinting** chapter.

---

## 57) Planning diagnostics and error taxonomy

**Purpose:** make failures actionable for users and agents.

Deep dive:

* Error classes:

  * parse error
  * unsupported SQL syntax
  * binder/name-resolution error
  * ambiguous column
  * missing table
  * missing function
  * type coercion failure
  * aggregate/window validation error
  * unsupported logical plan
  * unsupported physical lowering
  * optimizer rule error
  * object-store planning error
  * statistics/metadata error
  * execution-time error
* Diagnostic payload:

  * phase
  * node path
  * expression path
  * input schema
  * offending identifier
  * expected type
  * actual type
  * candidate resolutions
  * suggested fix
* Agent remediation:

  * quote identifier
  * qualify column
  * cast expression
  * alias expression
  * register table
  * register UDF
  * disable unsupported syntax
  * add limit
  * rewrite cross join
  * materialize source.
* User-facing vs developer-facing errors.
* Error snapshots in tests.

Why high value: agents need to fix plans. Planning diagnostics should be treated as a structured interface, not just strings.

---

## 58) Custom planning extensions: choosing between SQL extension, logical extension, provider pushdown, and physical operator

**Purpose:** unify Sections 18, 25, and 26 into a decision framework.

Deep dive:

* Decision tree:

  * can it be expressed as normal `Expr`?
  * can it be a UDF?
  * can it be a `TableProvider`?
  * can it be provider pushdown?
  * can it be a logical rewrite?
  * does it need new SQL syntax?
  * does it need a logical extension node?
  * does it need custom physical execution?
* Extension surfaces:

  * `ExprPlanner`
  * `TypePlanner`
  * `RelationPlanner`
  * `LogicalPlan::Extension`
  * `UserDefinedLogicalNodeCore`
  * `ExtensionPlanner`
  * custom `QueryPlanner`
  * custom `ExecutionPlan`
  * custom `PhysicalExpr`
* Invariants:

  * schema
  * expressions
  * children
  * display
  * metrics
  * pushdown semantics
  * physical properties
  * memory reservations.
* End-to-end extension lifecycle:

  * parse
  * bind
  * logical node
  * optimizer rules
  * physical lowering
  * execution
  * explain
  * serialization
  * tests.

Existing anchor: Section 26 already has the custom extension flow from custom syntax/query language/DataFrame extension/optimizer rule → `LogicalPlan::Extension` → planner hook → custom `ExecutionPlan` → stream output . The gap is a **decision tree and lifecycle governance chapter**.

---

## 59) LLM-agent planning contract: PlanSpec / ExprSpec → DataFusion plans

**Purpose:** adapt the planning docs to agent-generated semantic query plans.

Deep dive:

* Inputs:

  * user intent
  * validated semantic objects
  * table registry
  * column registry
  * function registry
  * permission policy
* `ExprSpec`:

  * column refs
  * literals
  * casts
  * function calls
  * aggregate calls
  * window calls
  * aliases
  * sort keys
  * null semantics
* `PlanSpec`:

  * source
  * projection
  * filter
  * join
  * aggregate
  * window
  * sort
  * limit
  * write sink
* Compile pipeline:

  * raw spec validation
  * name binding
  * schema inference
  * policy lint
  * `Expr` construction
  * `LogicalPlanBuilder`
  * optimizer
  * physical planning
  * artifact export.
* Deterministic generation:

  * stable aliasing
  * canonical expression order
  * canonical join order where user-specified
  * stable plan fingerprints
  * deterministic tests.
* Agent feedback loop:

  * compile errors
  * suggested repairs
  * schema mismatch reports
  * plan lint warnings
  * explain summaries.

Why high value: this is the chapter most aligned with a semantic compiled platform. It bridges DataFusion primitives to agent-operable planning, without requiring the agent to emit raw SQL.

---

## 60) End-to-end governed planning reference architecture

**Purpose:** capstone that ties all planning pieces together.

Deep dive:

* Engine construction:

  * `SessionConfig`
  * `RuntimeEnv`
  * catalog registration
  * object-store registration
  * UDF registration
  * optimizer rule registration
* Request pipeline:

  * SQL or PlanSpec input
  * parse/bind
  * logical plan creation
  * logical plan validation
  * optimizer
  * physical planning
  * physical validation
  * explain artifact export
  * execution stream
  * metrics capture
* Security:

  * SQL options
  * table allowlist
  * function allowlist
  * path restrictions
  * memory/temp limits
  * timeouts
* Observability:

  * plan bundle
  * metrics
  * traces
  * explain snapshots
* Testing:

  * plan golden
  * result golden
  * optimizer rule tests
  * physical plan property tests
  * serialization tests
* Deployment profiles:

  * CLI
  * batch ETL
  * service endpoint
  * multi-tenant query service
  * semantic modeling workbench.

Why high value: this would become the implementation blueprint for a production DataFusion planning layer.

---

# Priority expansion order

1. **41) End-to-end planning lifecycle and phase boundary map**
   This should be first because it consolidates the existing scattered material.

2. **42) SQL planner and binder internals**
   Highest missing technical depth for SQL-created plans.

3. **43) Programmatic logical planning with DataFrame / Expr / LogicalPlanBuilder**
   Highest value for agent-generated plans and your PlanSpec direction.

4. **44) Plan schema, column identity, aliases, and qualifier governance**
   Prevents the most common plan-generation and golden-test instability.

5. **46) Logical plan validation and policy linting**
   Necessary for safe service deployment and LLM-agent guardrails.

6. **49–50) Physical planning and plan properties**
   Needed for custom execution, diagnostics, and performance reasoning.

7. **55–56) Planning artifact package + plan serialization/cache/fingerprints**
   Needed for reproducibility, regression, and agent-operable planning.

8. **59–60) LLM-agent planning contract + governed reference architecture**
   Capstone layer that turns DataFusion docs into a semantic planning platform.



# DataFusion Advanced — 41) End-to-end planning lifecycle and phase boundary map

## 41.0 Purpose

Unify DataFusion planning into one compiler-style lifecycle:

```text
input query surface
  → parse / build
  → bind / resolve
  → analyze
  → logical optimize
  → physical plan
  → physical optimize
  → execute
  → Arrow RecordBatch stream
```

The attached documentation already defines the canonical pipeline as SQL/DataFrame/`LogicalPlanBuilder` into `LogicalPlan`, then logical optimization, physical planning, physical optimization, `ExecutionPlan`, `SendableRecordBatchStream`, and Arrow `RecordBatch` output . This chapter turns that pipeline into an implementation contract for LLM programming agents.

---

## 41.1 Compiler pipeline: canonical map

```text
SQL path
  SQL text
    → parser dialect / SQL parser
    → sqlparser::ast::Statement
    → SqlToRel / SessionState::statement_to_plan
    → unresolved/bound SQL expressions
    → DataFusion Expr
    → DFSchema-resolved LogicalPlan
    → Analyzer
    → analyzed LogicalPlan
    → logical Optimizer
    → optimized LogicalPlan
    → QueryPlanner / PhysicalPlanner
    → initial ExecutionPlan
    → PhysicalOptimizerRule chain
    → optimized ExecutionPlan
    → ExecutionPlan::execute(partition, TaskContext)
    → SendableRecordBatchStream
    → RecordBatch stream

DataFrame path
  SessionContext source method
    → DataFrame
    → DataFrame transformations
    → LogicalPlan
    → same analyzer / optimizer / physical planner / executor path

LogicalPlanBuilder path
  table source / schema / provider source
    → LogicalPlanBuilder::scan / empty / values / etc.
    → builder transformations
    → LogicalPlan
    → same downstream path

Custom query language path
  custom AST / semantic IR / PlanSpec
    → validation
    → Expr construction
    → LogicalPlanBuilder or LogicalPlan enum
    → optional LogicalPlan::Extension
    → same downstream path
```

`SessionContext` is the main user-facing query/session interface; it can create DataFrames from sources, register sources as SQL tables, register custom sources, and execute SQL queries ([Docs.rs][1]). DataFrames are lazy: transformations build a query definition, and execution starts only at action methods such as `collect` ([Apache DataFusion][2]). Logical plans are the implementation-independent query representation; DataFusion’s logical-plan guide states they abstract away physical details and precede optimized physical execution plans ([Apache DataFusion][3]).

---

## 41.2 Phase boundary table

| Phase             | Input artifact                          | Main API / subsystem                                                           | Output artifact                                 | State object                                            | Primary failure class                                     |
| ----------------- | --------------------------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------- |
| parse             | SQL text / expression text              | `SessionState::sql_to_statement`, `SessionContext::parse_sql_expr`, SQL parser | SQL AST / SQL expression AST                    | `SessionState`, parser dialect, `ConfigOptions`         | parse error / unsupported syntax                          |
| bind / resolve    | SQL AST, catalog, table refs, functions | `SqlToRel`, `SessionState::statement_to_plan`, `create_logical_plan`           | initial `LogicalPlan`, bound `Expr`, `DFSchema` | `SessionState`, catalog list, function registry         | missing table, ambiguous column, missing function         |
| analyze           | initial `LogicalPlan`                   | analyzer rules                                                                 | analyzed `LogicalPlan`                          | `SessionState`, `ExecutionProps`, `ConfigOptions`       | type coercion / semantic validation error                 |
| logical optimize  | analyzed `LogicalPlan`                  | `SessionState::optimize`, `OptimizerRule` chain                                | optimized `LogicalPlan`                         | optimizer config, table stats, constraints              | optimizer rule error / invalid rewrite                    |
| physical plan     | optimized `LogicalPlan`                 | `QueryPlanner`, `PhysicalPlanner`                                              | `ExecutionPlan`                                 | `SessionState`, `RuntimeEnv`, catalog/providers         | unsupported logical node / physical planning error        |
| physical optimize | initial `ExecutionPlan`                 | `PhysicalOptimizerRule` chain                                                  | optimized `ExecutionPlan`                       | physical optimizer config, plan properties              | invalid physical property / physical rewrite error        |
| execute           | optimized `ExecutionPlan`               | `ExecutionPlan::execute`                                                       | `SendableRecordBatchStream`                     | `TaskContext`, `RuntimeEnv`, memory pool, object stores | execution error / object-store error / memory spill error |
| consume           | stream                                  | `collect`, `execute_stream`, writer, custom sink                               | `Vec<RecordBatch>` / streamed batches / files   | caller policy + runtime                                 | buffer OOM / serialization / sink error                   |

`SessionState` exposes methods for SQL parsing/planning, expression creation, logical optimization, physical planning, physical expression creation, function/table/file-format registry access, config access, runtime access, and `TaskContext` creation ([Docs.rs][4]). Its `create_physical_plan` convenience method creates an `ExecutionPlan` from a `LogicalPlan` and first calls logical optimization on the supplied plan, so phase-perfect capture sometimes requires using lower-level planner hooks or instrumentation rather than only the convenience API ([Docs.rs][4]).

---

## 41.3 Planning entrypoints

### 41.3.1 `SessionContext::sql`

Primary user-facing SQL entrypoint.

```rust
use datafusion::prelude::*;

let ctx = SessionContext::new();

ctx.register_parquet("facts", "s3://bucket/facts/", ParquetReadOptions::default())
    .await?;

let df = ctx
    .sql(
        "SELECT customer_id, SUM(amount) AS total_amount
         FROM facts
         WHERE event_date >= DATE '2026-01-01'
         GROUP BY customer_id"
    )
    .await?;
```

Phase behavior:

```text
ctx.sql(sql)
  → parse SQL
  → plan SQL into LogicalPlan
  → return DataFrame
  → no row execution yet
```

`SessionContext::sql` creates a `DataFrame` from SQL text and can also implement DDL/DML with default implementations; `sql_with_options` validates statement classes before creating the DataFrame ([Docs.rs][1]).

Agent rules:

```text
Use ctx.sql for trusted SQL.
Use ctx.sql_with_options for user SQL.
Do not assume ctx.sql executes rows.
Register all table/function/catalog dependencies before planning.
```

---

### 41.3.2 `SessionContext::sql_with_options`

Policy-gated SQL entrypoint.

```rust
use datafusion::prelude::*;

let options = SQLOptions::new()
    .with_allow_ddl(false)
    .with_allow_dml(false)
    .with_allow_statements(false);

let df = ctx
    .sql_with_options(
        "SELECT customer_id, SUM(amount) AS total_amount FROM facts GROUP BY customer_id",
        options,
    )
    .await?;
```

Use for:

```text
public SQL endpoint
read-only analytics API
generated SQL from trusted compiler with DDL/DML disabled
multi-tenant SQL surface
query preview / validation mode
```

`sql_with_options` validates that queries are allowed by `SQLOptions`; the docs show `with_allow_ddl(false)` rejecting `CREATE TABLE` during planning ([Docs.rs][1]).

---

### 41.3.3 DataFrame transformations

Programmatic plan construction without SQL string assembly.

```rust
use datafusion::prelude::*;
use datafusion::functions_aggregate::expr_fn::sum;

let df = ctx
    .read_parquet("s3://bucket/facts/", ParquetReadOptions::default())
    .await?
    .filter(col("event_date").gt_eq(lit("2026-01-01")))?
    .aggregate(
        vec![col("customer_id")],
        vec![sum(col("amount")).alias("total_amount")],
    )?
    .sort(vec![col("total_amount").sort(false, false)])
    .limit(0, Some(100))?;
```

Phase behavior:

```text
read_parquet
  → scan logical source wrapped in DataFrame

filter / aggregate / sort / limit
  → new DataFrame with transformed LogicalPlan

collect / execute_stream / write_*
  → downstream optimization + execution
```

DataFrames are usually created from `SessionContext` methods and modified by transformations such as `filter`, `select`, `aggregate`, and `limit`; each transformation creates a new plan, and execution occurs when an action such as `collect` is invoked ([Apache DataFusion][2]).

Agent rules:

```text
Prefer DataFrame + Expr for generated plans.
Avoid SQL string concatenation for programmatic predicates.
Alias computed expressions.
Use execute_stream for service output.
Use collect only for bounded results.
```

---

### 41.3.4 `LogicalPlanBuilder`

Lower-level programmatic planning.

```rust
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::DataFusionError;
use datafusion::logical_expr::{LogicalPlanBuilder, LogicalTableSource};
use datafusion::prelude::*;

fn build_plan() -> Result<datafusion::logical_expr::LogicalPlan, DataFusionError> {
    let schema = Schema::new(vec![
        Field::new("id", DataType::Int32, true),
        Field::new("name", DataType::Utf8, true),
    ]);

    let table_source = LogicalTableSource::new(SchemaRef::new(schema));

    let plan = LogicalPlanBuilder::scan("person", Arc::new(table_source), None)?
        .filter(col("id").gt(lit(500)))?
        .project(vec![col("id"), col("name")])?
        .limit(0, Some(100))?
        .build()?;

    Ok(plan)
}
```

DataFusion’s logical-plan guide states that logical plans can be created directly, but using `LogicalPlanBuilder` is easier; the DataFrame API is a higher-level API that delegates to `LogicalPlanBuilder` ([Apache DataFusion][3]). The same guide states that building logical-plan structure performs no query execution and that logical plans must later be compiled into `ExecutionPlan`s ([Apache DataFusion][5]).

Use when:

```text
building a custom query compiler backend
compiling PlanSpec / ExprSpec IR
creating optimizer tests
creating extension-node wrappers
avoiding SQL parser/binder entirely
requiring deterministic programmatic plan construction
```

---

### 41.3.5 `SessionContext::execute_logical_plan`

Runs a prebuilt `LogicalPlan` through DataFrame execution APIs.

```rust
use datafusion::prelude::*;
use datafusion::logical_expr::LogicalPlan;

async fn run_plan(
    ctx: &SessionContext,
    plan: LogicalPlan,
) -> datafusion::error::Result<Vec<datafusion::arrow::record_batch::RecordBatch>> {
    let df = ctx.execute_logical_plan(plan).await?;
    df.collect().await
}
```

`execute_logical_plan` returns a `DataFrame` for a supplied `LogicalPlan`, but it is not feature-limited; the docs point to `sql_with_options` and `SQLOptions::verify_plan` when SQL-derived plans must be restricted ([Docs.rs][1]).

Agent rules:

```text
Use execute_logical_plan for trusted programmatic plans.
Do not use execute_logical_plan as a security gate.
Run logical-plan policy validation before execute_logical_plan.
Apply service-level limit / table allowlist / function allowlist externally.
```

---

### 41.3.6 `SessionState::create_logical_plan`

Lower-level SQL → `LogicalPlan` without immediately wrapping as `DataFrame`.

```rust
use datafusion::prelude::*;
use datafusion::logical_expr::LogicalPlan;

async fn sql_to_logical(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<LogicalPlan> {
    let state = ctx.state();
    state.create_logical_plan(sql).await
}
```

`SessionState::create_logical_plan` creates a `LogicalPlan` from SQL and can plan SQL DataFusion supports, including DML/DDL-like statements; the docs recommend `SessionContext::sql` / `sql_with_options` as higher-level interfaces for DDL handling and allowed-statement verification ([Docs.rs][4]).

Use when:

```text
capturing initial logical plan artifact
running logical-plan lint before execution
building plan-cache fingerprints
exporting plan debug bundles
testing planner behavior separately from execution
```

---

### 41.3.7 `SqlToRel`

Explicit SQL AST → DataFusion logical representation.

```text
sqlparser::ast::Statement
  → SqlToRel::statement_to_plan / sql_statement_to_plan
  → LogicalPlan
```

`SqlToRel` can convert SQL expressions to DataFusion `Expr`, build schemas from SQL column definitions, and generate logical plans from SQL/DataFusion SQL statements ([Docs.rs][6]).

Use when:

```text
building custom SQL frontends
instrumenting SQL AST → plan conversion
using custom ContextProvider
injecting custom parser options
testing SQL planner behavior without SessionContext convenience wrappers
```

Agent rules:

```text
Use SessionContext/SessionState first.
Use SqlToRel when implementing planner internals or custom SQL contexts.
Keep ContextProvider complete: tables, functions, variables, config, dialect.
Treat SqlToRel errors as binder/planner errors, not execution errors.
```

---

### 41.3.8 `QueryPlanner` / `PhysicalPlanner`

Logical → physical lowering extension surfaces.

```rust
use std::fmt::Debug;
use std::sync::Arc;
use datafusion::common::Result;
use datafusion::execution::context::{QueryPlanner, SessionState};
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct MyQueryPlanner;

#[async_trait::async_trait]
impl QueryPlanner for MyQueryPlanner {
    async fn create_physical_plan(
        &self,
        logical_plan: &LogicalPlan,
        session_state: &SessionState,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // Match custom LogicalPlan::Extension nodes, otherwise delegate to default planner.
        todo!("delegate or lower custom logical nodes")
    }
}
```

`QueryPlanner` is the DataFusion trait for adding extensions to logical and physical plans; its required method receives a `LogicalPlan` and `SessionState` and returns an `ExecutionPlan` suitable for execution ([Docs.rs][7]). `PhysicalPlanner` converts `LogicalPlan` to `ExecutionPlan` and also creates physical expressions from logical `Expr`s and `DFSchema`s ([Docs.rs][8]).

---

## 41.4 Artifact lifecycle

```text
SQL text
  user-facing / generated text
  stable as user intent artifact
  unsafe as executable artifact unless policy-validated

sqlparser AST
  parsed syntax tree
  dialect-sensitive
  no catalog binding yet

unresolved expression fragments
  SQL expression AST
  identifier strings
  literals
  parser-level function names

Expr
  DataFusion logical expression
  typed or type-inferable under DFSchema
  used in filters/projections/aggregates/windows/sorts/joins

DFSchema
  logical schema with qualifiers
  column-resolution and expression-inference substrate

initial LogicalPlan
  binder output / DataFrame-builder output
  semantic relational tree
  may still require analyzer rewrites

analyzed LogicalPlan
  semantically valid logical plan
  type-coercion / normalization applied

optimized LogicalPlan
  equivalent logical tree after optimizer rules
  predicate/projection/limit pushdown
  join/order/aggregate rewrites

initial ExecutionPlan
  concrete physical operator DAG
  physical expressions
  partitioning/order requirements
  algorithm choices

optimized ExecutionPlan
  physical rewrite output
  inserted/removed sorts/repartitions
  optimized join/sort/dynamic-filter behavior

SendableRecordBatchStream
  async stream of Arrow RecordBatch values
  partition-local execution result

RecordBatch
  runtime data unit
  immutable Arrow column arrays + schema
```

`LogicalPlan` is a tree of relational operators such as projection and filter; it transforms input relations into output relations and can be created from SQL, the DataFrame API, or programmatically via custom query languages ([Docs.rs][9]). `ExecutionPlan` nodes are the physical plan: calling `execute` produces an async `SendableRecordBatchStream` of `RecordBatch` values, and methods such as `schema`, `properties`, input distribution, and input ordering communicate physical properties to the optimizer ([Docs.rs][10]).

---

## 41.5 State-object lifecycle

| State object     | Planning role                                                                         | Execution role                                       | Mutability / lifetime                      | Agent contract                                      |
| ---------------- | ------------------------------------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------ | --------------------------------------------------- |
| `SessionContext` | main facade; source registration; SQL/DataFrame entrypoint; high-level plan execution | creates `TaskContext`; owns/links runtime resources  | app / tenant / request-group               | default application object                          |
| `SessionState`   | per-query/custom planning state; catalog, functions, config, optimizers, planner      | creates physical plan and `TaskContext`              | snapshot-ish; use builder for custom state | compiler context                                    |
| `SessionConfig`  | user-facing config wrapper                                                            | affects execution policy                             | context/session-level                      | set batch size, partitions, parser, optimizer flags |
| `ConfigOptions`  | raw config tree                                                                       | referenced by optimizer/executor                     | version-sensitive, non-exhaustive          | include in plan artifact hash                       |
| `ExecutionProps` | query-time properties, `now()` stability, alias generation, variable providers        | expression evaluation metadata                       | per-query                                  | do not hand-roll in app code                        |
| `RuntimeEnv`     | object-store registry, memory/disk/cache resources                                    | physical execution resource domain                   | shared or isolated                         | production service must configure explicitly        |
| `TaskContext`    | none or minimal planning role                                                         | execution context passed to `ExecutionPlan::execute` | per execution/task                         | use for manual physical execution                   |

The attached documentation already models `SessionContext → SessionState → TaskContext` as a three-layer state stack, where `SessionContext` owns user-facing session facilities, `SessionState` carries per-query planning/execution state, and `TaskContext` carries execution-only runtime state such as memory/disk/cache/object-store resources . `SessionState` itself is documented as containing configuration, functions, and runtime environment needed to plan and execute queries ([Docs.rs][4]).

---

## 41.6 Phase-by-phase API map

### Parse / SQL AST

```rust
let state = ctx.state();

// Higher-level: SQL text directly to LogicalPlan.
let logical_plan = state.create_logical_plan(sql).await?;

// Lower-level parser utility exists on SessionState:
// state.sql_to_statement(sql, dialect)?
```

`SessionState` exposes `sql_to_statement`, `sql_to_expr`, and `sql_to_expr_with_alias` for parsing SQL strings into DataFusion/sqlparser AST forms; it also exposes `resolve_table_references` for resolving table references in SQL statements, excluding CTE references ([Docs.rs][4]).

---

### Bind / resolve

```rust
let state = ctx.state();

let logical_plan = state.create_logical_plan(
    "SELECT a, SUM(b) AS total_b FROM t GROUP BY a"
).await?;
```

Binding responsibilities:

```text
table names        → CatalogProviderList / CatalogProvider / SchemaProvider / TableProvider
column names       → DFSchema + qualifiers
function names     → scalar / aggregate / window / table function registries
SQL types          → Arrow DataType / DFSchema fields
aliases            → output field names
CTEs / subqueries  → scoped logical subplans
placeholders       → later parameter binding, if used
```

`SessionContext` exposes registration APIs for tables, catalogs, file sources, functions, variables, and relation planners, so successful SQL planning depends on the session’s registered metadata and planner extensions ([Docs.rs][1]).

---

### Analyze

```text
initial LogicalPlan
  → analyzer rules
  → analyzed LogicalPlan
```

Analyzer responsibilities:

```text
type coercion
semantic validation
function argument normalization
expression normalization
subquery/aggregate/window semantic checks
planning-time rewrites required for validity
```

The optimizer guide describes DataFusion’s optimizer module as containing analyzer, logical optimizer, and physical optimizer rule families that rewrite plans/expressions while preserving results or enforcing semantic validity ([Apache DataFusion][11]). The attached documentation separates Analyzer, Logical optimizer, Physical planner, and Physical optimizer into distinct phases, with analyzer output described as a valid analyzed `LogicalPlan` .

---

### Logical optimize

```rust
let state = ctx.state();

let initial = state.create_logical_plan(sql).await?;
let optimized = state.optimize(&initial)?;
```

Logical optimizer responsibilities:

```text
projection pushdown
predicate pushdown
limit pushdown
expression simplification
constant folding
join reordering / join simplification
aggregate/window rewrites
subquery decorrelation
extension-node rewrites where supported
```

`SessionState::optimize` applies optimizer rules to a logical plan ([Docs.rs][4]). The optimizer guide states that optimizer rules and physical optimizer rules may rewrite plans and expressions to execute more quickly while computing the same result ([Apache DataFusion][11]).

---

### Physical plan

```rust
let state = ctx.state();

let logical = state.create_logical_plan(sql).await?;
let physical = state.create_physical_plan(&logical).await?;
```

Physical planner responsibilities:

```text
LogicalPlan node → ExecutionPlan node
Expr → PhysicalExpr
logical schema → physical schema
logical join → hash / sort-merge / nested-loop / other join exec
logical aggregate → physical aggregate strategy
logical sort/limit → sort/top-k/limit exec
source scan → provider/file/object-store execution plan
extension node → extension planner / custom QueryPlanner
```

Logical plans cannot be directly executed; they must be compiled into `ExecutionPlan`s, which contain more implementation details and algorithm choices ([Apache DataFusion][5]). The `PhysicalPlanner` trait creates physical plans and physical expressions from logical plans/expressions ([Docs.rs][8]).

---

### Physical optimize

```text
initial ExecutionPlan
  → PhysicalOptimizerRule chain
  → optimized ExecutionPlan
```

Physical optimizer responsibilities:

```text
insert/remove SortExec
insert/remove RepartitionExec
coalesce partitions
exploit ordering
push down sort
select join strategy
insert dynamic filters
preserve/repair plan properties
validate required input order/distribution
```

Physical optimizer rules are part of DataFusion’s optimizer subsystem and operate on physical plans while preserving result semantics ([Apache DataFusion][11]). `ExecutionPlan` methods communicate output properties and input requirements to the optimizer, including output ordering, partitioning, required input distribution, and required input ordering ([Docs.rs][10]).

---

### Execute

```rust
use futures::StreamExt;
use datafusion::prelude::*;

let state = ctx.state();
let logical = state.create_logical_plan("SELECT * FROM t").await?;
let physical = state.create_physical_plan(&logical).await?;
let task_ctx = state.task_ctx();

let mut stream = physical.execute(0, task_ctx)?;

while let Some(batch) = stream.next().await {
    let batch = batch?;
    println!("rows={}", batch.num_rows());
}
```

Execution responsibilities:

```text
poll partition stream
read input batches
evaluate PhysicalExpr
apply operators
reserve/release memory
spill when needed
read object-store/file data
emit RecordBatch values
surface execution metrics/errors
```

`ExecutionPlan::execute` returns a stream; the docs note the method is not itself async, but returns an async stream that should incrementally compute output `RecordBatch` by `RecordBatch`, with most plans doing no work before the first batch is requested ([Docs.rs][10]).

---

## 41.7 Artifact capture: planning bundle contract

Minimum plan artifact bundle for agent diagnostics:

```rust
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::common::Result;
use datafusion::execution::context::SessionContext;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::ExecutionPlan;

pub struct PlanningBundle {
    pub sql: Option<String>,

    pub initial_logical_plan: LogicalPlan,
    pub optimized_logical_plan: LogicalPlan,

    // High-level convenience physical plan. Depending on DataFusion version/API
    // path, this may already include physical optimization.
    pub physical_plan: Arc<dyn ExecutionPlan>,

    pub output_schema: SchemaRef,
}

pub async fn build_sql_planning_bundle(
    ctx: &SessionContext,
    sql: &str,
) -> Result<PlanningBundle> {
    let state = ctx.state();

    let initial_logical_plan = state.create_logical_plan(sql).await?;
    let optimized_logical_plan = state.optimize(&initial_logical_plan)?;
    let physical_plan = state.create_physical_plan(&initial_logical_plan).await?;
    let output_schema = physical_plan.schema();

    Ok(PlanningBundle {
        sql: Some(sql.to_string()),
        initial_logical_plan,
        optimized_logical_plan,
        physical_plan,
        output_schema,
    })
}
```

Artifact policy:

```text
Always capture:
  input SQL / PlanSpec
  DataFusion crate version
  relevant feature flags
  SessionConfig / ConfigOptions
  table schemas
  function registry version/hash
  catalog snapshot/hash
  initial LogicalPlan
  optimized LogicalPlan
  physical plan display
  output schema
  execution mode: collect / stream / write

Capture when debugging performance:
  EXPLAIN
  EXPLAIN VERBOSE
  EXPLAIN ANALYZE
  operator metrics
  partition counts
  spill metrics
  object-store listing metadata
  table statistics

Capture when debugging correctness:
  input schema
  DFSchema with qualifiers
  expression type/nullability inference
  plan lints
  aliases/output field names
  result sample
  deterministic ORDER BY state
```

The attached serialization section already recommends storing SQL plus version/config/catalog snapshot for durable storage or long-lived caches rather than blindly storing serialized plans .

---

## 41.8 Phase boundary error taxonomy

| Error boundary                    | Typical trigger                                        | Phase             | Diagnostic fields                   | Agent remediation                                     |
| --------------------------------- | ------------------------------------------------------ | ----------------- | ----------------------------------- | ----------------------------------------------------- |
| parse error                       | invalid SQL grammar, unsupported dialect syntax        | parse             | SQL span, token, dialect            | fix syntax, set dialect, use DataFrame API            |
| SQL feature error                 | unsupported statement/function/operator                | parse/bind        | statement kind, function/operator   | rewrite query, register extension, use UDF            |
| name-resolution error             | table/column/function missing                          | bind              | identifier, candidate refs, schema  | register table/function, qualify column, fix alias    |
| ambiguous column error            | unqualified column after join                          | bind              | column name, candidate qualifiers   | qualify column, rename duplicate columns              |
| type-coercion error               | incompatible operands / casts                          | analyze           | expr, expected type, actual type    | cast explicitly, change literal type, repair schema   |
| aggregate/window validation error | invalid `GROUP BY`, invalid window context             | analyze           | clause, expr, group/window spec     | move filter to `HAVING`/`QUALIFY`, add group key      |
| logical optimizer error           | custom rule failure, invalid rewrite                   | logical optimize  | rule name, before/after plan        | disable/fix rule, preserve schema/aliases             |
| physical planning error           | unsupported logical node, custom extension not lowered | physical plan     | logical node, extension name        | implement planner hook, rewrite to built-in operators |
| physical optimizer error          | bad plan properties, invalid child replacement         | physical optimize | rule name, plan node, properties    | recompute properties, fix required distribution/order |
| execution error                   | IO, memory, UDF, object store, spill, panic-safe error | execute           | operator, partition, metric context | inspect metrics, memory limit, UDF null/type behavior |
| consumption error                 | `collect` OOM, serialization, sink write               | consume           | output size, sink path, schema      | stream, limit, partition write, use sink backpressure |

Agent default:

```text
Never collapse all failures into “query failed.”
Classify failure by phase:
  parse | bind | analyze | logical_optimize | physical_plan | physical_optimize | execute | consume
```

---

## 41.9 Planning entrypoint decision table

| Input source               | Preferred entrypoint                                           | Why                                       | Avoid when                                      |
| -------------------------- | -------------------------------------------------------------- | ----------------------------------------- | ----------------------------------------------- |
| human SQL                  | `ctx.sql_with_options`                                         | dialect/user-facing surface + policy gate | caller is fully trusted admin and needs DDL/DML |
| trusted internal SQL       | `ctx.sql`                                                      | simplest SQL path                         | public/multi-tenant endpoint                    |
| UI filters / typed request | DataFrame + `Expr`                                             | no SQL string concatenation               | SQL compatibility is product requirement        |
| semantic compiler IR       | `LogicalPlanBuilder`                                           | deterministic, typed, parser-free         | SQL dialect itself is product output            |
| custom SQL syntax          | `ExprPlanner` / `RelationPlanner` / `TypePlanner` + `SqlToRel` | product dialect extension                 | normal SQL/UDF can express semantics            |
| new relational semantics   | `LogicalPlan::Extension`                                       | optimizer-visible custom node             | built-ins can express operation                 |
| new runtime algorithm      | custom `ExecutionPlan`                                         | physical specialization                   | logical rewrite to built-ins is enough          |
| custom full lowering       | custom `QueryPlanner`                                          | intercept logical → physical              | default physical planner works                  |
| prebuilt plan              | `execute_logical_plan`                                         | trusted logical artifact                  | untrusted SQL-derived plan without lint         |

The attached extension chapter already frames the extension order as UDF, `TableProvider`, SQL planner hook, `LogicalPlanBuilder`, `LogicalPlan::Extension`, `ExecutionPlan`, then custom `QueryPlanner`, with the default preference being the highest-level extension that preserves semantics .

---

## 41.10 End-to-end SQL planning harness

```rust
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::common::Result;
use datafusion::execution::context::{SessionConfig, SessionContext};
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::prelude::*;

pub fn configured_context() -> Result<SessionContext> {
    let config = SessionConfig::new()
        .with_batch_size(8192)
        .with_target_partitions(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        );

    let runtime = RuntimeEnvBuilder::new()
        .with_memory_limit(8 * 1024 * 1024 * 1024, 0.80)
        .with_temp_file_path("/tmp/datafusion-spill")
        .build_arc()?;

    let state = SessionStateBuilder::new()
        .with_config(config)
        .with_runtime_env(runtime)
        .with_default_features()
        .build();

    Ok(SessionContext::from(state))
}

#[tokio::main]
async fn main() -> Result<()> {
    let ctx = configured_context()?;

    ctx.register_csv("example", "example.csv", CsvReadOptions::new())
        .await?;

    let sql = "
      SELECT
        a,
        MIN(b) AS min_b
      FROM example
      WHERE a <= b
      GROUP BY a
      ORDER BY a
      LIMIT 100
    ";

    let state = ctx.state();

    // Phase: SQL text → initial LogicalPlan
    let initial_logical = state.create_logical_plan(sql).await?;
    println!("INITIAL LOGICAL:\n{}", initial_logical.display_indent_schema());

    // Phase: initial/analyzed logical → optimized LogicalPlan
    let optimized_logical = state.optimize(&initial_logical)?;
    println!("OPTIMIZED LOGICAL:\n{}", optimized_logical.display_indent_schema());

    // Phase: LogicalPlan → ExecutionPlan
    // Note: create_physical_plan applies state.optimize internally.
    let physical = state.create_physical_plan(&initial_logical).await?;
    println!(
        "PHYSICAL:\n{}",
        datafusion::physical_plan::displayable(physical.as_ref()).indent(true)
    );

    // Phase: execute through DataFrame action
    let batches = ctx.sql(sql).await?.collect().await?;
    println!("{}", pretty_format_batches(&batches)?);

    Ok(())
}
```

Deployment note:

```text
For diagnostics:
  use state.create_logical_plan + state.optimize + state.create_physical_plan.

For application execution:
  use ctx.sql_with_options / DataFrame API / execute_stream.

For phase-perfect internals:
  instrument custom planners/optimizer rules;
  public convenience APIs may combine logical optimization and physical planning.
```

---

## 41.11 DataFrame planning harness

```rust
use datafusion::common::Result;
use datafusion::prelude::*;
use datafusion::functions_aggregate::expr_fn::sum;

pub async fn dataframe_planning_harness(ctx: &SessionContext) -> Result<()> {
    let df = ctx
        .read_parquet("s3://bucket/facts/", ParquetReadOptions::default())
        .await?
        .filter(col("amount").gt(lit(0.0)))?
        .aggregate(
            vec![col("customer_id")],
            vec![sum(col("amount")).alias("total_amount")],
        )?
        .sort(vec![col("total_amount").sort(false, false)])
        .limit(0, Some(100))?;

    println!("UNOPTIMIZED LOGICAL:\n{:#?}", df.logical_plan());

    // Testing/debug path; consumes DataFrame.
    let optimized = df.clone().into_optimized_plan()?;
    println!("OPTIMIZED LOGICAL:\n{}", optimized.display_indent_schema());

    let physical = df.create_physical_plan().await?;
    println!(
        "PHYSICAL:\n{}",
        datafusion::physical_plan::displayable(physical.as_ref()).indent(true)
    );

    Ok(())
}
```

Agent rules:

```text
df.logical_plan() = inspect current unoptimized logical tree.
df.into_optimized_plan() = useful for tests/tools; consumes DataFrame.
df.create_physical_plan() = create physical plan for diagnostics/manual execution.
df.collect() = execute + buffer.
df.execute_stream() = execute + stream.
```

---

## 41.12 Custom query-language planning harness

```rust
use datafusion::common::Result;
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
use datafusion::prelude::*;

// Example compiler target for a semantic PlanSpec-like object.
pub struct QuerySpec {
    pub table: String,
    pub projected_columns: Vec<String>,
    pub limit: Option<usize>,
}

pub async fn compile_query_spec(
    ctx: &SessionContext,
    spec: QuerySpec,
) -> Result<LogicalPlan> {
    let table_provider = ctx.table_provider(spec.table.as_str()).await?;

    let table_source = datafusion::datasource::provider_as_source(table_provider);

    let projection = spec
        .projected_columns
        .iter()
        .map(|name| col(name.as_str()))
        .collect::<Vec<_>>();

    let mut builder = LogicalPlanBuilder::scan(spec.table, table_source, None)?
        .project(projection)?;

    if let Some(limit) = spec.limit {
        builder = builder.limit(0, Some(limit))?;
    }

    builder.build()
}
```

Custom query-language path contract from the attached documentation:

```text
custom query AST
  → validate semantic model
  → build LogicalPlanBuilder chain
  → insert LogicalPlan::Extension only for unsupported relational operator
  → optimize
  → physical plan
  → execute stream
```

The attached operator-extension chapter states that custom query languages typically build `LogicalPlan` directly rather than using the SQL planner, while DataFrame planning works similarly but via `LogicalPlanBuilder` .

---

## 41.13 Plan validation insertion points

```text
SQL text
  → SQL policy:
      max length
      allowed dialect
      no forbidden tokens if cheap prefilter needed

SQL AST
  → AST policy:
      statement kind
      table reference pre-scan
      forbidden DDL/DML/session statements

initial LogicalPlan
  → semantic policy:
      allowed tables
      allowed columns
      allowed functions
      no CrossJoin unless approved
      partition filter required
      limit required for preview
      no direct path scan
      no unbounded scan unless streaming API

optimized LogicalPlan
  → optimizer sanity:
      output schema preserved
      table-source filters pushed as expected
      no forbidden extension nodes introduced

ExecutionPlan
  → physical policy:
      partition count
      memory-sensitive operators
      sorts / repartitions
      unsupported custom exec
      expected object-store source

stream
  → runtime policy:
      cancellation
      timeout
      memory limit
      row/byte cap
      sink backpressure
```

Agent deployment default:

```text
Untrusted SQL:
  SQLOptions first
  logical-plan lint second
  execution admission third
  streaming output fourth
```

---

## 41.14 Plan fingerprint inputs

Plan cache / reproducibility fingerprint should not hash only SQL text.

```text
PlanFingerprintV1:
  engine:
    datafusion_crate_version
    arrow_crate_version
    feature_flags
  input:
    sql_text_hash | planspec_hash
    parser_dialect
    normalized_identifier_policy
  session:
    ConfigOptions hash
    target_partitions
    batch_size
    timezone
    optimizer flags
    sql parser options
  catalog:
    catalog/schema/table refs
    table schema hashes
    table provider type ids / versions
    table statistics version/hash
    object-store URL roots
  functions:
    scalar/aggregate/window/table function registry hash
    UDF versions
    volatility signatures
  planning:
    initial_logical_plan_hash
    optimized_logical_plan_hash
    physical_plan_display_hash
  authorization:
    tenant id / policy version
    allowed table/column/function set hash
```

Invalidation triggers:

```text
DataFusion version change
Arrow version change
ConfigOptions change
catalog/table schema change
statistics refresh
function registry change
UDF implementation change
optimizer rule set/order change
object-store listing change
authorization policy change
extension node codec change
```

---

## 41.15 Phase-specific best practices

### Parse

```text
Pin dialect.
Keep parser config in artifact bundle.
Prefer DataFrame/Expr for generated predicates.
Do not accept arbitrary SQL statements in read-only services.
```

### Bind / resolve

```text
Register tables before planning.
Register UDFs before planning.
Use lowercase snake_case physical names where possible.
Qualify columns after joins.
Use aliases for computed outputs.
```

### Analyze

```text
Treat type coercion errors as compile errors.
Preflight dynamic expressions against DFSchema.
Prefer explicit casts for generated plans.
Test nullability inference for UDFs and computed columns.
```

### Logical optimize

```text
Preserve expression names and aliases in custom rules.
Do not rewrite volatile expressions unsafely.
Use optimizer snapshots under pinned DataFusion versions.
Validate output schema after custom rules.
```

### Physical plan

```text
Do not infer runtime algorithms from LogicalPlan.
Inspect ExecutionPlan for join/sort/repartition behavior.
Lower custom logical nodes explicitly.
Prefer built-in physical nodes when possible.
```

### Physical optimize

```text
Do not overclaim ordering or partitioning.
Recompute PlanProperties after child replacement.
Use exact/inexact/unsupported pushdown contracts conservatively.
Snapshot physical plans only under exact version pins.
```

### Execute

```text
Use execute_stream for services and large exports.
Use collect only for bounded results.
Set memory and temp-dir limits for production.
Use TaskContext from SessionState/SessionContext unless testing custom execs.
```

---

## 41.16 Anti-pattern inventory

* Treating `ctx.sql(...).await?` as row execution instead of DataFrame creation.
* Building generated SQL strings when DataFrame/`Expr` builders would be safer.
* Calling `execute_logical_plan` on untrusted plans without `SQLOptions::verify_plan` or logical linting.
* Assuming `state.create_physical_plan` is a pure physical-planning-only phase; it applies logical optimization first.
* Snapshotting physical plan strings across DataFusion upgrades without exact version pins.
* Losing aliases during optimizer/custom rule rewrites.
* Stripping qualifiers before join-derived column resolution.
* Registering UDFs after plan creation and expecting old plans to bind them.
* Exposing SQL `SET` / DDL / DML in a public endpoint unintentionally.
* Overclaiming custom `ExecutionPlan` ordering or partitioning.
* Returning incorrect `PlanProperties` from custom physical nodes.
* Using `collect` for arbitrary user queries.
* Creating one global `SessionContext` across tenants with different catalogs/credentials.
* Treating serialized plans as durable cross-version artifacts without version/config/catalog invalidation.
* Reporting all errors as execution errors instead of parse/bind/analyze/optimize/physical-plan/runtime failures.

---

## 41.17 Agent checklist

```text
[ ] Identify entrypoint:
    ctx.sql | ctx.sql_with_options | DataFrame | LogicalPlanBuilder | SqlToRel | execute_logical_plan | QueryPlanner

[ ] Identify current phase:
    parse | bind | analyze | logical_optimize | physical_plan | physical_optimize | execute | consume

[ ] Capture artifacts:
    SQL/PlanSpec
    parser dialect
    initial LogicalPlan
    optimized LogicalPlan
    physical plan display
    output schema
    ConfigOptions
    catalog/table schema snapshot
    function registry version/hash

[ ] Validate before execution:
    statement class
    table allowlist
    column allowlist
    function allowlist
    direct path access
    DDL/DML/session statements
    limit/result cap
    unbounded scan policy

[ ] Use correct state:
    SessionContext for app facade
    SessionState for per-query planning/customization
    RuntimeEnv for memory/disk/object-store domain
    TaskContext for physical execution

[ ] Use correct action:
    logical_plan / optimize / create_physical_plan for diagnostics
    execute_stream for service results
    collect for bounded tests
    write_* for sink/export

[ ] Classify errors by phase:
    parse error
    name-resolution error
    type-coercion error
    analysis error
    logical optimizer error
    physical planning error
    physical optimizer error
    execution error

[ ] Pin versions:
    datafusion
    datafusion-* subcrates
    arrow/parquet/object_store
    SQL dialect behavior
    optimizer snapshots
```

---

[1]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
[2]: https://datafusion.apache.org/user-guide/dataframe.html "DataFrame API — Apache DataFusion  documentation"
[3]: https://datafusion.apache.org/library-user-guide/building-logical-plans.html "Building Logical Plans — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/session_state/struct.SessionState.html "SessionState in datafusion::execution::session_state - Rust"
[5]: https://datafusion.apache.org/_sources/library-user-guide/building-logical-plans.md.txt "datafusion.apache.org"
[6]: https://docs.rs/datafusion/latest/datafusion/sql/planner/struct.SqlToRel.html "SqlToRel in datafusion::sql::planner - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/execution/context/trait.QueryPlanner.html "QueryPlanner in datafusion::execution::context - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_planner/trait.PhysicalPlanner.html "PhysicalPlanner in datafusion::physical_planner - Rust"
[9]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/enum.LogicalPlan.html "LogicalPlan in datafusion_expr::logical_plan - Rust"
[10]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html "ExecutionPlan in datafusion::physical_plan - Rust"
[11]: https://datafusion.apache.org/library-user-guide/query-optimizer.html "Query Optimizer — Apache DataFusion  documentation"


## 41.18 DataFusion 54 lifecycle change: uncorrelated scalar subqueries reach physical planning

Before DataFusion 54 the logical optimizer's `ScalarSubqueryToJoin` rule rewrote every uncorrelated `Expr::ScalarSubquery` into a left join, so the optimized `LogicalPlan` contained no scalar-subquery expressions and the physical planner never saw one. DataFusion 54 moves that phase boundary: uncorrelated scalar subqueries stay in the final logical plan and are lowered by the physical planner into `ScalarSubqueryExec` (plan node, `datafusion_physical_plan::scalar_subquery`) plus `ScalarSubqueryExpr` (physical expression, `datafusion_physical_expr::scalar_subquery`). Each subquery executes exactly once per query, and its scalar result is shared with every expression that references it.

Phase-boundary implications:

```text id="sqlc54"
optimized LogicalPlan:
  may contain uncorrelated Expr::ScalarSubquery nodes

physical plan:
  may contain ScalarSubqueryExec nodes and ScalarSubqueryExpr expressions

runtime:
  subquery returning more than one row is an execution error
  (the 53 join rewrite silently multiplied output rows instead)
  zero rows yields a typed NULL scalar

tooling:
  plan validators, visitors, serializers, and operator allowlists written
  against the "no subqueries after optimization" invariant must be updated
```

Gated by `datafusion.optimizer.enable_physical_uncorrelated_scalar_subquery = true` (default on). Disabling it restores the pre-54 join rewrite as a temporary escape hatch for distributed execution frameworks that cannot yet transport the new node — at the cost of the old limitations (silently incorrect multi-row results; no scalar subqueries in `ORDER BY`, `JOIN ON`, or aggregate arguments). Full physical treatment: 49.30. Serialization implications: 56.32.

---

# DataFusion Advanced — 42) SQL planner and binder internals: `sqlparser` AST → `SqlToRel` → `LogicalPlan`

## 42.0 Purpose

Make SQL planning understandable as a compiler sub-pipeline, not a black-box `ctx.sql(...)` call:

```text id="am74op"
SQL text
  → DFParser / sqlparser
  → sqlparser::ast::Statement
  → SqlToRel
      ├─ ContextProvider metadata lookup
      ├─ table/source resolution
      ├─ column / expression binding
      ├─ SQL type mapping
      ├─ function lookup
      ├─ CTE / subquery / view context
      └─ extension planners
  → LogicalPlan + Expr + DFSchema
  → analyzer / type coercion / optimizer / physical planner
```

The attached doc already frames `datafusion_sql` as the layer containing `DFParser`, `SqlToRel`, `ContextProvider`, planner-extension integration, SQL name/type/function resolution, and SQL unparsing; it also distinguishes the SQL path from direct `LogicalPlanBuilder` planning. 

`SqlToRel` is DataFusion’s SQL query planner and binder. Its docs state that it converts a SQL AST into a `LogicalPlan`, performs name and type resolution using `ContextProvider`, and mechanically translates AST nodes into logical plan/expression nodes; broad type coercion and optimization are subsequent passes, not `SqlToRel`’s job. ([Docs.rs][1])

---

## 42.1 Compiler-stage map

```text id="rfgodp"
Stage 0: SQL text
  input:  "SELECT a, SUM(b) FROM t GROUP BY a"
  owner:  caller / SessionContext / DFParser
  output: sqlparser AST

Stage 1: SQL AST
  input:  sqlparser::ast::Statement
  owner:  sqlparser / DataFusion SQL parser wrapper
  output: Statement::Query / DDL / DML / utility statement variants

Stage 2: binder/planner metadata lookup
  input:  AST names: table refs, column refs, function refs, type names
  owner:  SqlToRel + ContextProvider
  output: TableSource refs, function metadata, DFSchema-qualified fields

Stage 3: expression binding
  input:  sqlparser::ast::Expr
  owner:  SqlToRel::sql_to_expr
  output: datafusion_expr::Expr

Stage 4: relation binding
  input:  FROM items, JOINs, CTEs, subqueries, table functions
  owner:  SqlToRel relation planner
  output: relational LogicalPlan subtree

Stage 5: query-shape binding
  input:  SELECT / WHERE / GROUP BY / HAVING / QUALIFY / ORDER BY / LIMIT
  owner:  SqlToRel
  output: initial LogicalPlan

Stage 6: post-binder passes
  input:  initial LogicalPlan
  owner:  analyzer + optimizer
  output: analyzed / optimized LogicalPlan
```

DataFusion’s official SELECT reference documents the clause skeleton from `WITH` through `SELECT`, `FROM`, joins, `WHERE`, `GROUP BY`, `HAVING`, `QUALIFY`, `UNION`, `ORDER BY`, `LIMIT`, `EXCEPT`/`EXCLUDE`, and pipe operators; `SqlToRel` is the layer that gives that syntax a DataFusion `LogicalPlan` meaning. ([Apache DataFusion][2])

---

## 42.2 `sqlparser` AST statement classes

`sqlparser` produces a dialect-sensitive AST, not a DataFusion plan. Its `Statement` enum is intentionally broad and includes vendor-specific forms such as Snowflake `COPY INTO`; DataFusion must still decide which parsed statement variants it supports and how to lower them. ([Docs.rs][3])

Practical statement buckets for DataFusion agents:

```text id="lkzwer"
Query / DQL:
  Statement::Query
    → SELECT / WITH / UNION / ORDER BY / LIMIT / VALUES-like query bodies
    → usually lowered to LogicalPlan relational operators

DDL:
  CREATE TABLE
  CREATE EXTERNAL TABLE
  CREATE VIEW
  CREATE SCHEMA / DATABASE
  DROP TABLE / VIEW
  DESCRIBE / DESC
    → usually lowered to Ddl-like LogicalPlan variants or catalog operations

DML / write:
  INSERT
  COPY
    → lowered to Dml / Copy / write-oriented logical nodes

Utility / session:
  SET
  SHOW
  EXPLAIN
  PREPARE / EXECUTE where supported
    → may produce Explain/Analyze/statement logical nodes or mutate/query session config

Dialect-specific / parser-superset:
  Snowflake COPY INTO
  PostgreSQL-specific operators
  BigQuery pipe syntax
  dialect-specific identifiers/operators/types
    → only usable if parsed and either supported by DataFusion or handled by extension planners
```

Agent invariant:

```text id="8zv98u"
sqlparser parse success ≠ DataFusion planning success.
DataFusion planning success ≠ physical execution success.
```

---

## 42.3 Core API: SQL text → AST → `SqlToRel`

### High-level path: `SessionContext::sql`

```rust id="scsp4e"
use datafusion::prelude::*;

let ctx = SessionContext::new();

ctx.register_csv("t", "t.csv", CsvReadOptions::new()).await?;

let df = ctx
    .sql("SELECT a, SUM(b) AS total_b FROM t GROUP BY a")
    .await?;
```

`SessionContext::sql` is the convenient API: it parses/plans SQL into a lazy `DataFrame`; execution starts only at `collect`, `show`, `execute_stream`, or a writer action. `SessionState` contains the lower-level state needed for planning/execution, including configuration, functions, and runtime environment. ([Docs.rs][4])

### Lower-level path: `SessionState::create_logical_plan`

```rust id="vaby0y"
use datafusion::prelude::*;
use datafusion::logical_expr::LogicalPlan;

async fn sql_to_plan(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<LogicalPlan> {
    let state = ctx.state();
    state.create_logical_plan(sql).await
}
```

Use this path when the agent needs a planning artifact before execution:

```text id="cykwve"
SQL linting
table/function allowlist checks
plan fingerprinting
golden logical-plan tests
explain/debug bundles
custom optimizer input
```

### Explicit `sqlparser` + `SqlToRel`

```rust id="uwesum"
use datafusion_sql::{
    planner::{ContextProvider, SqlToRel},
    sqlparser::{dialect::GenericDialect, parser::Parser},
};

fn plan_with_context_provider<S: ContextProvider>(
    sql: &str,
    context_provider: &S,
) -> datafusion_common::Result<datafusion_expr::LogicalPlan> {
    let dialect = GenericDialect {};
    let ast = Parser::parse_sql(&dialect, sql)
        .map_err(|e| datafusion_common::DataFusionError::SQL(Box::new(e)))?;

    let statement = ast
        .into_iter()
        .next()
        .ok_or_else(|| datafusion_common::DataFusionError::Plan("empty SQL".to_string()))?;

    let planner = SqlToRel::new(context_provider);
    planner.sql_statement_to_plan(statement)
}
```

The `datafusion-sql` example source demonstrates this explicit shape: parse SQL with `sqlparser`, create a custom `ContextProvider`, construct `SqlToRel`, and call `sql_statement_to_plan` to produce a `LogicalPlan`. ([Docs.rs][5])

---

## 42.4 `SqlToRel`: responsibilities and non-responsibilities

### Responsibilities

```text id="5bmded"
SqlToRel:
  SQL AST Statement       → LogicalPlan
  SQL AST Expr            → Expr
  SQL column definitions  → Arrow Schema / DFSchema inputs
  table names             → TableSource through ContextProvider
  function names          → ScalarUDF / AggregateUDF / WindowUDF metadata
  SQL aliases             → projection fields / relation aliases
  CTE/subquery contexts   → scoped relation bindings
  SQL type names          → Arrow/DataFusion field types
  extension planner hooks → custom Expr / type / relation lowering
```

`SqlToRel` exposes `sql_to_expr`, `statement_to_plan`, `sql_statement_to_plan`, `sql_statement_to_plan_with_context`, `build_schema`, and constraint conversion methods; it is generic over a `ContextProvider`. ([Docs.rs][1])

### Non-responsibilities

```text id="ajw426"
SqlToRel does not own:
  broad type coercion pass
  logical optimization
  physical planning
  physical operator choice
  execution scheduling
  memory/spill/runtime behavior
  final result materialization
```

This distinction matters for agent debugging. If `SqlToRel` successfully emits a `LogicalPlan`, a later analyzer pass may still reject incompatible types, and a later physical planner may still reject an unsupported logical/extension node. `SqlToRel` docs explicitly say type coercion and optimization are done by subsequent passes. ([Docs.rs][1])

---

## 42.5 `ContextProvider`: metadata boundary

`ContextProvider` is the SQL planner’s metadata interface. Its docs describe it as providing table and function metadata to the SQL query planner without requiring a direct dependency on DataFusion catalog structures such as `TableProvider`. ([Docs.rs][6])

Conceptual trait contract:

```text id="vh6waq"
ContextProvider:
  get_table_source(TableReference) -> Arc<dyn TableSource>
  get_function_meta(name)          -> ScalarUDF metadata
  get_aggregate_meta(name)         -> AggregateUDF metadata
  get_window_meta(name)            -> WindowUDF metadata
  get_table_function_source(...)   -> table-function source if supported
  get_variable_type(...)           -> variable / system variable typing
  options()                        -> ConfigOptions / ParserOptions substrate
  udf_names / udaf_names / udwf_names
```

Minimal mental model:

```text id="jrzs5i"
SqlToRel is intentionally catalog-agnostic.

SessionContext / SessionState:
  catalog + function registries + config

ContextProvider:
  exposes enough of that metadata for SQL binding

SqlToRel:
  consumes ContextProvider, emits LogicalPlan
```

Custom provider skeleton, shape-only:

```rust id="e80osh"
use std::{collections::HashMap, sync::Arc};

use datafusion_common::{Result, TableReference};
use datafusion_expr::{AggregateUDF, ScalarUDF, TableSource, WindowUDF};
use datafusion_sql::planner::ContextProvider;

#[derive(Debug)]
pub struct MyContextProvider {
    tables: HashMap<String, Arc<dyn TableSource>>,
    scalar_udfs: HashMap<String, Arc<ScalarUDF>>,
    aggregate_udfs: HashMap<String, Arc<AggregateUDF>>,
    window_udfs: HashMap<String, Arc<WindowUDF>>,
    options: datafusion_common::config::ConfigOptions,
}

// Implement exact trait methods against the pinned DataFusion version.
// Signatures can drift; use docs.rs for the exact target crate version.
impl ContextProvider for MyContextProvider {
    fn get_table_source(
        &self,
        name: TableReference,
    ) -> Result<Arc<dyn TableSource>> {
        let key = name.to_string();
        self.tables
            .get(&key)
            .cloned()
            .ok_or_else(|| datafusion_common::DataFusionError::Plan(
                format!("table not found: {key}")
            ))
    }

    // implement function lookup / variable typing / options methods as required
}
```

Agent rules:

```text id="b7dg8n"
ContextProvider is the binder’s universe.
If a table/function/type is absent from ContextProvider, SQL planning cannot resolve it.
For custom semantic catalogs, implement ContextProvider or install catalog/providers in SessionContext.
For service isolation, ContextProvider/catalog content must be tenant-scoped.
```

---

## 42.6 `PlannerContext`: scoped planner state

`PlannerContext` is the planner’s scoped state bag for CTEs, views, subqueries, `PREPARE` statements, parameter data types, and outer query schema. DataFusion source docs state that CTEs are truly cloned when `PlannerContext` is cloned so subqueries can inherit CTEs from outer query context while maintaining correct scoping. ([Docs.rs][7])

Mental model:

```text id="40mq7u"
PlannerContext:
  ctes:
    WITH x AS (...)
    scoped named query fragments

  views:
    catalog-backed virtual relation definitions

  subquery state:
    nested SELECT planning
    outer-query schema for correlation

  prepare / placeholders:
    parameter data types
    placeholder resolution

  outer query schema:
    correlated subquery / lateral context
```

Use cases:

```text id="l1t2fh"
Use explicit PlannerContext when:
  manually invoking SqlToRel.sql_statement_to_plan_with_context
  planning expression fragments with nested scope
  testing CTE/subquery binding
  implementing RelationPlanner that calls context.plan(...)
  preserving parameter type metadata across planning calls
```

Anti-pattern:

```text id="9j9qvy"
Do not reuse one mutable PlannerContext across unrelated user queries.
PlannerContext is query-planning scope, not application-global state.
```

---

## 42.7 Table resolution

### Resolution hierarchy

```text id="t6oxn2"
SQL table reference:
  t
  schema.t
  catalog.schema.t
  "CaseSensitiveTable"
  subquery_alias
  cte_name
  view_name
  table_function(args)

Binder resolution order, conceptually:
  1. local relation aliases
  2. CTE names in PlannerContext
  3. subquery aliases / derived tables
  4. view names via catalog/context
  5. catalog.schema.table via ContextProvider
  6. table functions / RelationPlanner hooks where applicable
```

Actual lookup details are implementation/version-specific, but the metadata boundary is stable: table lookup goes through `ContextProvider::get_table_source`, and `PlannerContext` carries CTE/subquery/view/prepare state. ([Docs.rs][6])

### Catalog / schema / table

```sql id="p4mdss"
SELECT *
FROM prod.analytics.events AS e
WHERE e.event_date = DATE '2026-05-22';
```

Binding result:

```text id="0ekpbj"
prod.analytics.events
  → TableReference
  → ContextProvider::get_table_source
  → TableSource
  → DFSchema fields qualified by table/ref/alias context
```

### Aliases

```sql id="r61h5d"
SELECT e.event_id
FROM prod.analytics.events AS e;
```

Alias binding:

```text id="8bgus0"
base relation name:
  prod.analytics.events

visible relation qualifier in query:
  e

column reference:
  e.event_id

unqualified:
  event_id only valid if unambiguous in current DFSchema
```

### CTEs

```sql id="ndjp9m"
WITH active_events AS (
  SELECT event_id, user_id
  FROM events
  WHERE is_active
)
SELECT user_id
FROM active_events;
```

CTE binding:

```text id="yu1tl2"
WITH active_events AS (...)
  → plan CTE query
  → store in PlannerContext.ctes
  → FROM active_events resolves to scoped logical subplan
```

### Views

```sql id="j23xgh"
SELECT *
FROM analytics.active_users;
```

View binding:

```text id="p1me3i"
catalog view
  → table/view provider or SQL definition
  → logical subplan expansion
  → outer query binding proceeds against view output schema
```

### Subqueries

```sql id="etvca0"
SELECT *
FROM (
  SELECT user_id, COUNT(*) AS n
  FROM events
  GROUP BY user_id
) AS counts
WHERE counts.n > 10;
```

Subquery binding:

```text id="v4ozmo"
subquery SELECT
  → independent nested LogicalPlan
  → alias required/recommended for stable qualification
  → outer query sees counts.user_id, counts.n
```

Agent table-resolution rules:

```text id="7vfju3"
Always alias derived tables.
Always alias CTE output columns when computed.
Prefer catalog.schema.table in generated SQL for stable environments.
Prefer lowercase snake_case physical names.
Quote only when preserving case or special identifiers.
Do not rely on direct path / URL table resolution in public services.
```

---

## 42.8 Column resolution

### Unqualified columns

```sql id="tku8nl"
SELECT id
FROM users;
```

Valid only when `id` resolves to exactly one field in the current `DFSchema`.

### Qualified columns

```sql id="ymblzu"
SELECT u.id, o.id AS order_id
FROM users AS u
JOIN orders AS o ON u.id = o.user_id;
```

Preferred after joins, subqueries, views, table aliases, and CTEs.

### Ambiguous columns

```sql id="1cd11p"
SELECT id
FROM users AS u
JOIN orders AS o ON u.id = o.user_id;
```

Likely ambiguous if both inputs contain `id`.

The attached doc’s schema section already gives the correct agent guidance: if two relations contain `id`, unqualified `col("id")` is ambiguous; agents should validate against `DFSchema::columns_with_unqualified_name` or use relation-qualified columns. 

### Duplicate output names

```sql id="st4k6f"
SELECT *
FROM users AS u
JOIN orders AS o ON u.id = o.user_id;
```

Hazards:

```text id="v6hnpe"
duplicate field names
unstable expression-derived names
projection ambiguity
DataFrame post-processing ambiguity
downstream serialization collision
golden-test instability
```

Safer projection:

```sql id="beykaw"
SELECT
  u.id AS user_id,
  u.email AS user_email,
  o.id AS order_id,
  o.amount AS order_amount
FROM users AS u
JOIN orders AS o ON u.id = o.user_id;
```

Agent column-resolution rules:

```text id="kq4yrg"
Before join:
  unqualified columns acceptable if table schema is known and unique.

After join:
  qualify all column references.
  alias duplicate output names.
  never expose SELECT * as stable public output.

For generated plans:
  preflight every unqualified name against DFSchema.
  if count(matches) != 1, reject or qualify.
```

---

## 42.9 Expression binding

### SQL expression → `Expr`

`SqlToRel::sql_to_expr` generates a DataFusion `Expr` from a SQL expression under a `DFSchema` and mutable `PlannerContext`. ([Docs.rs][1])

```rust id="sc6zg8"
use datafusion::common::DFSchema;
use datafusion::sql::planner::{ContextProvider, PlannerContext, SqlToRel};

fn bind_expr<S: ContextProvider>(
    planner: &SqlToRel<'_, S>,
    sql_expr: datafusion::sql::sqlparser::ast::Expr,
    schema: &DFSchema,
) -> datafusion::error::Result<datafusion::logical_expr::Expr> {
    let mut planner_context = PlannerContext::new();
    planner.sql_to_expr(sql_expr, schema, &mut planner_context)
}
```

### Literal binding

```sql id="sn1ss2"
SELECT
  1 AS i64_default,
  1.0 AS numeric_or_float_contextual,
  'abc' AS utf8_literal,
  NULL AS untyped_null,
  DATE '2026-05-22' AS date_literal;
```

Binding result, conceptually:

```text id="k7lmvd"
SQL literal
  → sqlparser ast literal
  → ScalarValue-like logical literal / typed expression
  → later analyzer may coerce to required context type
```

### Function lookup

```sql id="euudre"
SELECT
  lower(email) AS email_norm,
  my_udf(score) AS custom_score
FROM users;
```

Binding steps:

```text id="fvr4po"
function name
  → normalize / quote-sensitive lookup
  → ContextProvider scalar function registry
  → ScalarUDF metadata
  → Expr::ScalarFunction
```

### Aggregate validation

```sql id="16sm2q"
SELECT
  region,
  SUM(amount) AS total_amount
FROM sales
GROUP BY region;
```

Binding duties:

```text id="fg9wul"
identify aggregate function calls
separate grouping expressions from aggregate expressions
validate SELECT expressions against aggregate context
bind HAVING against aggregate output/input context
construct LogicalPlan::Aggregate + post-aggregate Filter if needed
```

### Window validation

```sql id="6k450c"
SELECT
  region,
  user_id,
  ROW_NUMBER() OVER (
    PARTITION BY region
    ORDER BY amount DESC
  ) AS rk
FROM sales
QUALIFY rk <= 3;
```

Binding duties:

```text id="mck0m2"
identify window functions
bind PARTITION BY expressions
bind ORDER BY expressions inside OVER
bind window frame
bind QUALIFY as post-window filter
validate aggregate/window context separation
```

DataFusion’s SELECT docs explicitly include `GROUP BY`, `HAVING`, and `QUALIFY`, and show `QUALIFY` as a window-function filter clause. ([Apache DataFusion][2])

### Casts and SQL types

```sql id="bsvhse"
SELECT
  CAST(raw_amount AS DECIMAL(20, 2)) AS amount,
  CAST(raw_ts AS TIMESTAMP) AS event_ts
FROM raw_events;
```

Binding duties:

```text id="sw1e1s"
sqlparser::ast::DataType
  → DataFusion SQL type planner / TypePlanner
  → Arrow DataType / FieldRef
  → Expr::Cast / TryCast
```

### Placeholders / parameters

```sql id="kzaxyo"
PREPARE q AS
SELECT *
FROM events
WHERE event_date = $1;
```

Planner-state duties:

```text id="h0r0mf"
placeholder occurrence
  → parameter index/name
  → parameter data type in PlannerContext
  → later parameter binding
  → typed Expr
```

`PlannerContext` carries parameter data types for `PREPARE` statements and outer query schema for correlated planning contexts. ([Docs.rs][7])

---

## 42.10 Type coercion boundary

Important correction for agents:

```text id="68z012"
SqlToRel:
  name/type lookup
  SQL type mapping
  casts as explicit Expr nodes
  expression structure construction

Analyzer:
  broad implicit type coercion
  semantic rewrites needed for validity

Optimizer:
  performance-preserving rewrites
```

`SqlToRel` docs say it performs name/type resolution and AST-to-`LogicalPlan` translation, but does not perform type coercion or optimization; those are later passes. ([Docs.rs][1])

Agent diagnostic rule:

```text id="vtdx1a"
If SQL parses and SqlToRel emits LogicalPlan,
  but execution/planning later fails on types,
  classify as analyzer/type-coercion failure,
  not parser failure.
```

Example:

```sql id="tmwvsj"
SELECT amount + 'abc' AS bad_expr
FROM t;
```

Likely lifecycle:

```text id="v4jvjs"
parse: ok
bind: may produce expression structure
analyze/type coercion: rejects incompatible operands
```

---

## 42.11 Query-shape binding

### `SELECT *`

```sql id="jbs7or"
SELECT *
FROM events;
```

Binding behavior:

```text id="7zv676"
input DFSchema
  → projection expansion
  → one output expression per visible field
  → duplicate names preserved unless later projection aliases them
```

Agent rule:

```text id="iq6z7x"
Use SELECT * for exploration only.
For generated/stable outputs, expand explicitly and alias duplicates.
```

### `* EXCEPT` / `* EXCLUDE`

```sql id="0hif5a"
SELECT * EXCEPT(raw_payload, debug_blob)
FROM events;

SELECT * EXCLUDE(raw_payload, debug_blob)
FROM events;
```

DataFusion documents both `SELECT * EXCEPT(...)` and `SELECT * EXCLUDE(...)` as ways to exclude named columns from star projection. ([Apache DataFusion][2])

Binding behavior:

```text id="0qucz3"
input fields
  → expand star
  → remove excluded field names
  → preserve remaining field order
```

### CTE expansion

```sql id="hvyh65"
WITH base AS (
  SELECT user_id, amount
  FROM events
  WHERE amount > 0
)
SELECT user_id, SUM(amount) AS total_amount
FROM base
GROUP BY user_id;
```

Binding behavior:

```text id="x13y2a"
WITH base
  → plan subquery
  → store scoped name in PlannerContext
  → resolve FROM base to CTE subplan
```

### Subquery aliasing

```sql id="w8ho8l"
SELECT q.user_id
FROM (
  SELECT user_id, COUNT(*) AS n
  FROM events
  GROUP BY user_id
) AS q
WHERE q.n > 10;
```

Agent rule:

```text id="zuy738"
Always alias subqueries.
Always alias computed columns inside subqueries.
Expose only intended output names to outer scopes.
```

### Correlated subqueries

```sql id="c4we64"
SELECT u.user_id
FROM users AS u
WHERE EXISTS (
  SELECT 1
  FROM orders AS o
  WHERE o.user_id = u.user_id
);
```

Binding behavior:

```text id="upbnfg"
inner subquery
  → has local schema o.*
  → can reference outer schema u.* through planner context
  → outer references must remain distinguishable
  → later decorrelation/optimizer may rewrite to semi join
```

### Lateral joins

```sql id="m5wxt4"
SELECT d.name AS dept, e.name AS emp
FROM departments AS d,
LATERAL (
  SELECT employees.name
  FROM employees
  WHERE employees.dept_id = d.id
) AS e;
```

DataFusion requires the `LATERAL` keyword for lateral `FROM`-clause correlation and does not implicitly detect correlation in `FROM` subqueries; limitations include no outer references in the lateral subquery `SELECT` list and no `HAVING` in lateral subqueries. ([Apache DataFusion][2])

### `GROUP BY`

```sql id="y29btj"
SELECT
  region,
  SUM(amount) AS total_amount
FROM sales
GROUP BY region;
```

Binding behavior:

```text id="p74qfm"
FROM plan
  → optional WHERE filter
  → Aggregate group_expr=[region], aggr_expr=[SUM(amount)]
  → projection of grouping keys + aggregate aliases
```

### `HAVING`

```sql id="sret5j"
SELECT
  region,
  SUM(amount) AS total_amount
FROM sales
GROUP BY region
HAVING SUM(amount) > 1000;
```

Binding behavior:

```text id="joc7gf"
Aggregate plan
  → post-aggregate Filter
  → HAVING expression resolved in aggregate context
```

### `QUALIFY`

```sql id="s8kwi0"
SELECT
  region,
  user_id,
  ROW_NUMBER() OVER (
    PARTITION BY region
    ORDER BY amount DESC
  ) AS rk
FROM sales
QUALIFY rk <= 3;
```

Binding behavior:

```text id="w4tp4k"
input plan
  → window expression plan
  → post-window Filter
  → final projection
```

---

## 42.12 SQL AST → logical operators

| SQL syntax               | AST bucket        | Binder/planner output                          |
| ------------------------ | ----------------- | ---------------------------------------------- |
| `SELECT a, b`            | query projection  | `LogicalPlan::Projection`                      |
| `FROM t`                 | table factor      | `TableScan` / provider scan source             |
| `FROM (SELECT ...) AS q` | derived table     | subquery `LogicalPlan` with alias              |
| `WITH x AS (...)`        | CTE               | scoped plan in `PlannerContext`                |
| `WHERE pred`             | selection         | `LogicalPlan::Filter`                          |
| `JOIN ... ON ...`        | join relation     | `LogicalPlan::Join`                            |
| `GROUP BY` + aggregates  | aggregate query   | `LogicalPlan::Aggregate`                       |
| `HAVING`                 | aggregate filter  | `Filter` above aggregate                       |
| window functions         | window expr       | `Window` logical node / window expressions     |
| `QUALIFY`                | window filter     | `Filter` above window                          |
| `ORDER BY`               | order             | `LogicalPlan::Sort`                            |
| `LIMIT`                  | fetch/offset      | `LogicalPlan::Limit`                           |
| `UNION ALL`              | set op            | `LogicalPlan::Union`                           |
| `SELECT DISTINCT`        | distinct          | `LogicalPlan::Distinct`                        |
| `VALUES`                 | inline relation   | `LogicalPlan::Values`                          |
| `EXPLAIN`                | utility           | `LogicalPlan::Explain`                         |
| DDL                      | catalog statement | DDL logical node / catalog effect on execution |
| DML / `COPY`             | write statement   | DML / Copy logical node                        |

Logical plans represent high-level operations and transformations, abstracting away physical execution details; DataFusion’s logical-plan docs state they are an intermediate step before optimized physical execution plans and can be created from SQL, DataFrame APIs, or manual construction. ([Apache DataFusion][8])

---

## 42.13 Planner extension hooks

DataFusion’s SQL extension system intercepts specific SQL AST fragments during `SqlToRel` and lets callers customize conversion into DataFusion logical expressions/plans. The official extension guide lists three planner traits: `ExprPlanner`, `TypePlanner`, and `RelationPlanner`; `ExprPlanner` and `RelationPlanner` can be registered multiple times and are invoked in reverse registration order, while only one `TypePlanner` can be active. ([Apache DataFusion][9])

### `ExprPlanner`

Use for syntax already parsed as a SQL expression/operator.

```rust id="7a0160"
use datafusion::common::Result;
use datafusion::common::DFSchema;
use datafusion::logical_expr::{
    Expr,
    Operator,
    BinaryExpr,
};
use datafusion::logical_expr::planner::{
    ExprPlanner,
    PlannerResult,
    RawBinaryExpr,
};
use datafusion::sql::sqlparser::ast::BinaryOperator;

#[derive(Debug)]
struct JsonArrowPlanner;

impl ExprPlanner for JsonArrowPlanner {
    fn plan_binary_op(
        &self,
        expr: RawBinaryExpr,
        _schema: &DFSchema,
    ) -> Result<PlannerResult<RawBinaryExpr>> {
        match expr.op {
            BinaryOperator::Arrow => {
                // Example rewrite only. Real JSON/struct access should use get_field/UDF semantics.
                Ok(PlannerResult::Planned(Expr::BinaryExpr(BinaryExpr {
                    left: Box::new(expr.left),
                    op: Operator::StringConcat,
                    right: Box::new(expr.right),
                })))
            }
            _ => Ok(PlannerResult::Original(expr)),
        }
    }
}
```

`ExprPlanner` customizes planning of SQL AST expressions to `Expr`s. Its methods include binary operators, field access, literals, aggregate/window planning, compound identifiers, and special SQL expression forms such as `POSITION`, `EXTRACT`, `SUBSTRING`, and struct literals. ([Docs.rs][10])

Registration:

```rust id="7gssjo"
use std::sync::Arc;
use datafusion::prelude::*;

let mut ctx = SessionContext::new();
ctx.register_expr_planner(Arc::new(JsonArrowPlanner))?;
```

Agent rules:

```text id="cwnbiu"
Use ExprPlanner when sqlparser already parses the syntax.
Return Planned(...) only when extension handled the node.
Return Original(...) for unhandled cases.
Do not use ExprPlanner to fix syntax that fails parsing.
Prefer rewriting to existing Expr/UDFs over custom logical nodes.
```

### `TypePlanner`

Use for custom SQL type names / dialect type mapping.

```rust id="qs3nyh"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, FieldRef, TimeUnit};
use datafusion::common::Result;
use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::logical_expr::planner::TypePlanner;
use datafusion::prelude::*;
use datafusion::sql::sqlparser::ast;

#[derive(Debug)]
struct MyTypePlanner;

impl TypePlanner for MyTypePlanner {
    fn plan_type_field(&self, sql_type: &ast::DataType) -> Result<Option<FieldRef>> {
        match sql_type {
            ast::DataType::Datetime(precision) => {
                let unit = match precision {
                    Some(0) => TimeUnit::Second,
                    Some(3) => TimeUnit::Millisecond,
                    Some(6) => TimeUnit::Microsecond,
                    None | Some(9) => TimeUnit::Nanosecond,
                    _ => return Ok(None),
                };

                Ok(Some(
                    DataType::Timestamp(unit, None).into_nullable_field_ref()
                ))
            }
            _ => Ok(None),
        }
    }
}

let state = SessionStateBuilder::new()
    .with_default_features()
    .with_type_planner(Arc::new(MyTypePlanner))
    .build();

let ctx = SessionContext::new_with_state(state);
```

The attached extension section already identifies `TypePlanner` as the correct hook when external dialect/catalog SQL types need custom Arrow/DataFusion mappings, and shows `DATETIME`/`UUID`-style examples. 

Agent rules:

```text id="ssctle"
TypePlanner changes SQL type interpretation.
It does not magically convert runtime arrays.
TableProvider output arrays must match planned Arrow types.
Only one TypePlanner is active; centralize dialect type mapping.
```

### `RelationPlanner`

Use for custom `FROM`-clause table factors.

```rust id="rc3hhm"
use datafusion::common::Result;
use datafusion::logical_expr::planner::{
    RelationPlanner,
    RelationPlannerContext,
    RelationPlanning,
};
use datafusion::logical_expr::LogicalPlan;
use datafusion::sql::sqlparser::ast::TableFactor;

#[derive(Debug)]
struct MyRelationPlanner;

impl RelationPlanner for MyRelationPlanner {
    fn plan_relation(
        &self,
        relation: TableFactor,
        context: &mut dyn RelationPlannerContext,
    ) -> Result<RelationPlanning> {
        match relation {
            // Inspect TableFactor variants from sqlparser.
            // Rewrite to LogicalPlan when recognized.
            _ => Ok(RelationPlanning::Original(relation)),
        }
    }
}
```

`RelationPlanner` customizes planning SQL table factors to `LogicalPlan`s; returning `RelationPlanning::Planned` short-circuits further planning with the provided plan, while `Original` delegates to the next registered planner or DataFusion’s default logic. ([Docs.rs][11])

Registration:

```rust id="h01ru2"
use std::sync::Arc;
use datafusion::prelude::*;

let mut ctx = SessionContext::new();
ctx.register_relation_planner(Arc::new(MyRelationPlanner))?;
```

Agent rules:

```text id="1gwq1o"
Use RelationPlanner for FROM-clause syntax:
  TABLESAMPLE
  PIVOT / UNPIVOT
  semantic_model('name')
  parameterized_source(...)
  custom table factor wrappers

Use context.plan(...) for nested relation planning.
Use context.sql_to_expr(...) for embedded expressions.
Preserve aliases and output schema.
Return Original for unhandled TableFactor variants.
```

### Parser wrappers

Planner hooks can only handle syntax that parses into `sqlparser` AST. DataFusion’s extension blog recommends parser wrappers when custom syntax must be intercepted in the token stream before standard planning; if parsing succeeds, implement `ExprPlanner`, `TypePlanner`, or `RelationPlanner` to assign semantics, and prefer rewrites to existing operators before adding custom physical operators. ([Apache DataFusion][12])

Parser-wrapper decision:

```text id="2eymvb"
Raw SQL fails to parse:
  → configure dialect if available
  → if still fails, wrapper/custom parser required

Raw SQL parses but DataFusion cannot plan expression:
  → ExprPlanner

Raw SQL parses but type maps incorrectly:
  → TypePlanner

Raw SQL parses but FROM relation needs custom meaning:
  → RelationPlanner

Logical plan needs runtime behavior not expressible by built-ins:
  → LogicalPlan::Extension + physical planner support
```

---

## 42.14 Registration lifecycle

```rust id="kdw2a7"
use std::sync::Arc;
use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::prelude::*;

fn build_sql_engine() -> datafusion::error::Result<SessionContext> {
    let state = SessionStateBuilder::new()
        .with_default_features()
        .with_type_planner(Arc::new(MyTypePlanner))
        .build();

    let mut ctx = SessionContext::new_with_state(state);

    ctx.register_expr_planner(Arc::new(JsonArrowPlanner))?;
    ctx.register_relation_planner(Arc::new(MyRelationPlanner))?;

    // Register tables/functions before SQL planning.
    // ctx.register_udf(...)
    // ctx.register_table(...)

    Ok(ctx)
}
```

Registration rules:

```text id="czzwpx"
Register before planning.
Changing registrations does not rewrite already-created DataFrames.
Centralize registrations in engine construction.
Keep registration order intentional:
  ExprPlanner / RelationPlanner: reverse registration order
  TypePlanner: only one active planner
```

The official extension guide lists `ctx.register_expr_planner()` for `ExprPlanner`, `ctx.register_relation_planner()` for `RelationPlanner`, and `SessionStateBuilder::with_type_planner()` for `TypePlanner`. ([Apache DataFusion][9])

---

## 42.15 Binder diagnostics matrix

| Failure                    | Typical SQL                            | Phase        | Root cause                                  | Agent repair                                    |
| -------------------------- | -------------------------------------- | ------------ | ------------------------------------------- | ----------------------------------------------- |
| parser syntax error        | `SELECT a ->> FROM t`                  | parse        | invalid token sequence / dialect mismatch   | set dialect, rewrite syntax, parser wrapper     |
| unsupported statement      | dialect-specific `COPY INTO`           | parse/bind   | sqlparser variant not planned by DataFusion | rewrite to DataFusion DML or custom planner     |
| table not found            | `FROM missing`                         | bind         | `ContextProvider::get_table_source` fails   | register table/source, fix catalog/schema/table |
| CTE shadowing confusion    | CTE name equals table name             | bind         | scope resolution ambiguity                  | rename CTE/alias, inspect PlannerContext        |
| ambiguous column           | `SELECT id FROM a JOIN b`              | bind         | multiple `id` fields in `DFSchema`          | qualify `a.id`/`b.id`, alias output             |
| duplicate output names     | `SELECT * FROM a JOIN b`               | bind/output  | repeated field names                        | explicit projection and aliases                 |
| function not found         | `SELECT foo(x)`                        | bind         | missing scalar/UDAF/window function         | register UDF or rewrite function                |
| aggregate misuse           | `SELECT a, b, SUM(c) GROUP BY a`       | analyze/bind | `b` neither grouped nor aggregated          | add group key or aggregate                      |
| window misuse              | `QUALIFY` without valid window context | bind/analyze | invalid window expression scope             | add window alias/expression                     |
| type mismatch              | `amount + 'abc'`                       | analyze      | failed type coercion                        | cast explicitly, repair literal/schema          |
| extension not reached      | custom syntax fails before planner     | parse        | sqlparser cannot parse token sequence       | parser wrapper or supported dialect             |
| extension wrongly consumes | planner returns `Planned` too broadly  | bind         | custom planner masks default behavior       | return `Original` for unhandled nodes           |

---

## 42.16 Binder artifact package

For SQL planning debugging, persist:

```text id="2iw7lq"
SqlPlanningBundleV1:
  raw_sql
  parser_dialect
  parser_options
  sqlparser_statement_debug
  statement_class
  normalized_sql_if_available
  registered_catalogs
  registered_schemas
  registered_table_refs
  table_schema_hashes
  function_registry_hash
  active_type_planner_id
  expr_planner_ids_in_order
  relation_planner_ids_in_order
  initial_logical_plan_display
  output_dfschema
  resolved_table_refs
  resolved_column_refs
  unresolved_identifiers_if_error
  bind_errors
  analyzer_errors
```

Diagnostic-first API pattern:

```rust id="c8vw80"
pub struct SqlPlanDiagnostic {
    pub sql: String,
    pub parse_ok: bool,
    pub statement_debug: Option<String>,
    pub logical_plan: Option<String>,
    pub error_phase: Option<&'static str>,
    pub error_message: Option<String>,
}
```

Agent use cases:

```text id="ak4cp8"
LLM repair loop:
  parse error       → fix syntax/dialect
  bind error        → fix table/column/function
  analyzer error    → fix type/group/window semantics
  physical error    → implement extension planner / rewrite logical shape
```

---

## 42.17 Deployment patterns

### Read-only SQL service

```rust id="7ikc5k"
use datafusion::prelude::*;

pub async fn plan_read_only_sql(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<datafusion::logical_expr::LogicalPlan> {
    let options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    let df = ctx.sql_with_options(sql, options).await?;
    let plan = df.into_unoptimized_plan();

    // run custom logical plan lint here:
    // - table allowlist
    // - column allowlist
    // - function allowlist
    // - limit policy
    // - no direct path scan
    // - no cross join unless approved

    Ok(plan)
}
```

Service policy:

```text id="qbckoc"
SQLOptions gates statement class.
Logical-plan lint gates semantic access.
Execution admission gates resources.
Streaming output prevents collect OOM.
```

### Custom semantic catalog

```text id="wis0gn"
Semantic model registry
  → exposes virtual TableSource / TableProvider
  → ContextProvider resolves semantic tables
  → SqlToRel binds normal SQL
  → logical-plan lint enforces semantic object policy
```

### Dialect compatibility layer

```text id="s2gbse"
external dialect SQL
  → configure sqlparser dialect
  → TypePlanner maps dialect types
  → ExprPlanner maps dialect operators/functions
  → RelationPlanner maps dialect FROM constructs
  → DataFusion LogicalPlan
```

---

## 42.18 Best-practice advisory

```text id="k25fl9"
Normal app:
  use SessionContext::sql or sql_with_options.

Planning tooling:
  use SessionState::create_logical_plan.

Custom SQL engine:
  use SqlToRel + ContextProvider when you need full metadata control.

Generated queries:
  prefer DataFrame/Expr/LogicalPlanBuilder unless SQL text is the product interface.

Custom operator syntax:
  use ExprPlanner only if sqlparser parses the operator.

Custom SQL type:
  use TypePlanner; verify runtime Arrow arrays match mapped type.

Custom FROM relation:
  use RelationPlanner; preserve aliases and output schema.

Parser-unsupported syntax:
  use parser wrapper or upstream sqlparser support.

Security:
  SQLOptions is necessary but insufficient; add logical-plan lint.

Versioning:
  pin datafusion, datafusion-sql, datafusion-expr, sqlparser-facing APIs.
```

---

## 42.19 Anti-pattern inventory

* Treating `ctx.sql(...)` as execution instead of planning.
* Debugging all SQL failures as parser failures.
* Expecting `ExprPlanner` to see syntax that `sqlparser` rejects.
* Returning `Err` instead of `Original` for unhandled extension-planner nodes.
* Registering planners after `ctx.sql` already produced a `DataFrame`.
* Using one global `TypePlanner` without documenting all custom type mappings.
* Emitting `SELECT *` after joins in stable APIs.
* Allowing unqualified columns after multi-table joins.
* Relying on expression-derived aggregate names.
* Assuming `SqlToRel` performs all implicit type coercion.
* Assuming sqlparser AST support implies DataFusion planner support.
* Exposing dialect-specific custom SQL without a versioned dialect manifest.
* Using custom logical/physical nodes when a UDF or normal logical rewrite is enough.
* Failing to test parser, planner, logical plan, physical plan, and result output separately.
* Sharing planner/catalog/function state across tenants with different permissions.

---

## 42.20 Agent checklist

```text id="lkavjz"
[ ] Classify SQL input:
    query | DDL | DML | utility | dialect-specific | custom syntax

[ ] Parse:
    configured dialect correct?
    parser accepts syntax?
    parser wrapper needed?

[ ] ContextProvider / session metadata:
    table registered?
    catalog/schema/table ref correct?
    UDF/UDAF/window UDF registered?
    custom type planner installed?
    custom expr/relation planners installed?

[ ] Table binding:
    aliases preserved?
    CTEs scoped correctly?
    views expanded/resolved?
    subqueries aliased?
    lateral references legal?

[ ] Column binding:
    unqualified columns unique?
    joined duplicate names handled?
    output aliases stable?
    quoted identifiers required?

[ ] Expression binding:
    literals typed intentionally?
    function names resolve?
    aggregate expressions legal?
    window expressions legal?
    casts explicit where needed?
    placeholders typed?

[ ] Query-shape binding:
    SELECT * acceptable?
    * EXCEPT / EXCLUDE intentional?
    GROUP BY/HAVING semantics correct?
    QUALIFY used only for window filters?
    ORDER BY/LIMIT deterministic?

[ ] Extension hooks:
    ExprPlanner for parsed custom expressions/operators.
    TypePlanner for SQL type mapping.
    RelationPlanner for FROM-clause constructs.
    Parser wrapper for syntax sqlparser cannot parse.

[ ] Post-binder:
    run analyzer / optimizer.
    capture initial LogicalPlan.
    validate logical plan for policy.
    snapshot output DFSchema.
    classify errors by parse/bind/analyze/optimize/physical/execute phase.
```

[1]: https://docs.rs/datafusion/latest/datafusion/sql/planner/struct.SqlToRel.html "SqlToRel in datafusion::sql::planner - Rust"
[2]: https://datafusion.apache.org/user-guide/sql/select.html "SELECT syntax — Apache DataFusion  documentation"
[3]: https://docs.rs/sqlparser/latest/sqlparser/ast/enum.Statement.html "Statement in sqlparser::ast - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/session_state/struct.SessionState.html "SessionState in datafusion::execution::session_state - Rust"
[5]: https://docs.rs/crate/datafusion-sql/latest/source/examples/sql.rs "datafusion-sql 54.1.0 - Docs.rs"
[6]: https://docs.rs/datafusion/latest/datafusion/sql/planner/trait.ContextProvider.html "ContextProvider in datafusion::sql::planner - Rust"
[7]: https://docs.rs/crate/datafusion-sql/latest/source/src/planner.rs?utm_source=chatgpt.com "datafusion-sql 54.1.0"
[8]: https://datafusion.apache.org/_sources/library-user-guide/building-logical-plans.md.txt "datafusion.apache.org"
[9]: https://datafusion.apache.org/library-user-guide/extending-sql.html "Extending SQL Syntax — Apache DataFusion  documentation"
[10]: https://docs.rs/datafusion/latest/datafusion/logical_expr/planner/trait.ExprPlanner.html "ExprPlanner in datafusion::logical_expr::planner - Rust"
[11]: https://docs.rs/datafusion-expr/latest/datafusion_expr/planner/trait.RelationPlanner.html "RelationPlanner in datafusion_expr::planner - Rust"
[12]: https://datafusion.apache.org/blog/2026/01/12/extending-sql/?utm_source=chatgpt.com "Extending SQL in DataFusion: from ->> to TABLESAMPLE"


## 42.21 DataFusion 54 parser surface: sqlparser 0.62, `SparkSqlDialect`, subquery sort elimination

DataFusion 54 bumps the vendored SQL parser to `sqlparser` 0.62. Two binder-relevant additions:

### 42.21.1 `SparkSqlDialect`

`sqlparser::dialect::Dialect` is a trait; each dialect is a unit struct implementing it — there is no `Dialect::Spark` enum variant. New in 0.62 is `sqlparser::dialect::SparkSqlDialect` (`sqlparser/src/dialect/spark.rs`), and `dialect_from_str` accepts `"spark"` / `"sparksql"`, so it is selectable through the standard config key:

```sql id="spkd54"
SET datafusion.sql_parser.dialect = 'spark';
```

```rust id="spkd54r"
use datafusion_sql::sqlparser::dialect::SparkSqlDialect;

let dialect = SparkSqlDialect {};
// pass to Parser::parse_sql(&dialect, sql) in explicit SqlToRel pipelines (42.3)
```

### 42.21.2 Subquery sort elimination

New key `datafusion.sql_parser.enable_subquery_sort_elimination = true` (default on): during SQL planning DataFusion may remove `ORDER BY` clauses from subqueries or CTEs when their ordering cannot affect the result — that is, when no `LIMIT` or other order-sensitive operator depends on them. Disable it to preserve explicit subquery ordering in the planned query. Because this happens at planning time, logical-plan snapshots taken under 53 may differ under 54 for queries with ordered subqueries.

---

# DataFusion Advanced — 43) Programmatic logical planning with `DataFrame`, `Expr`, and `LogicalPlanBuilder`

## 43.0 Purpose

Document the preferred **non-SQL planning path** for generated queries:

```text id="f8s48u"
typed application request / semantic PlanSpec
  → validate table / column / function / policy
  → construct Expr values
  → compose DataFrame transformations or LogicalPlanBuilder chain
  → produce LogicalPlan
  → inspect / lint / optimize
  → create physical plan
  → execute_stream
  → Arrow RecordBatch stream
```

The attached planning map already identifies `DataFrame`, `Expr`, and `LogicalPlanBuilder` as the typed Rust route into DataFusion’s logical-planning layer, with SQL, DataFrame, custom query languages, and direct builders all converging on `LogicalPlan` before analyzer, optimizer, physical planner, and execution.  DataFusion’s `DataFrame` docs describe DataFrames as lazy wrappers around `LogicalPlan` plus `SessionState`; most methods build plans, and `collect` triggers execution through the same planning/execution process used for SQL. ([Docs.rs][1])

---

## 43.1 Core mental model

```text id="cielmr"
DataFrame API:
  high-level ergonomic LogicalPlan builder
  owns SessionState snapshot
  convenient for application pipelines
  can execute directly via collect / execute_stream / write_*

Expr API:
  logical scalar / aggregate / window expression language
  embedded inside DataFrame and LogicalPlanBuilder nodes
  schema-aware via DFSchema / ExprSchemable

LogicalPlanBuilder:
  lower-level relational builder
  builds LogicalPlan directly
  ideal for custom compilers, reusable templates, optimizer tests

LogicalPlan enum:
  direct logical node construction
  rare; use when builder cannot express exact node shape

LogicalPlan::Extension:
  custom relational semantics
  requires physical planner / execution support
```

`LogicalPlanBuilder` is explicitly documented as a builder for logical plans, with examples constructing a plan similar to `SELECT last_name FROM employees WHERE salary < 1000` using table scan, filter, project, and build steps. ([Docs.rs][2]) The DataFusion logical-plan docs state that creating `LogicalPlan` enum nodes directly is possible, but `LogicalPlanBuilder` is usually easier. ([Apache DataFusion][3])

---

## 43.2 Selection matrix: SQL vs DataFrame vs `LogicalPlanBuilder` vs direct `LogicalPlan`

| Planning surface          | Use when                                                                                           | Avoid when                                                            | Output                                              |
| ------------------------- | -------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------- |
| SQL                       | human-authored query text, product SQL interface, analyst-facing CLI/API                           | generated filters/projections from structured app params              | `DataFrame` / `LogicalPlan`                         |
| `DataFrame` API           | generated application plans, typed filters, service-enforced policies, normal Rust query pipelines | exact low-level logical node shape required                           | `DataFrame` wrapping `LogicalPlan` + `SessionState` |
| `Expr` API                | constructing predicates, projections, aggregates, joins, window functions                          | raw SQL expressions are product contract                              | `Expr` embedded in plan nodes                       |
| `LogicalPlanBuilder`      | custom compilers, PlanSpec/ExprSpec, optimizer tests, deterministic plan factories                 | need immediate execution convenience of `DataFrame`                   | `LogicalPlan`                                       |
| direct `LogicalPlan` enum | exact node construction, low-level tests, extension integration                                    | builder covers the operation                                          | `LogicalPlan`                                       |
| `LogicalPlan::Extension`  | truly new relational semantics                                                                     | built-in relational nodes / UDF / TableProvider can express operation | custom logical node requiring physical planning     |

DataFusion’s `LogicalPlan` docs describe it as a relational-operator tree that transforms input relations to output relations and can be created by the SQL planner, DataFrame API, or programmatically, including custom query languages. ([Docs.rs][4])

Agent decision rule:

```text id="ksusq3"
Human query language required?
  → SQL.

Structured request / LLM-generated query spec?
  → DataFrame + Expr or LogicalPlanBuilder.

Need reusable compiler backend?
  → LogicalPlanBuilder.

Need new relational operator?
  → LogicalPlan::Extension + physical planner support.

Need new scalar computation only?
  → UDF, not LogicalPlan::Extension.
```

---

## 43.3 Imports and crate posture

```rust id="smnqay"
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::arrow::record_batch::RecordBatch;

use datafusion::common::{
    Column,
    DFSchema,
    DFSchemaRef,
    DataFusionError,
    Result,
    ScalarValue,
};

use datafusion::logical_expr::{
    col,
    lit,
    Expr,
    ExprSchemable,
    JoinType,
    LogicalPlan,
    LogicalPlanBuilder,
    LogicalTableSource,
    SortExpr,
};

use datafusion::prelude::*;

use datafusion::functions_aggregate::expr_fn::{avg, count, max, min, sum};
```

Use `datafusion::arrow` re-exports in DataFusion applications to avoid Arrow version mismatches. The uploaded documentation explicitly treats this as a workspace-level agent invariant and warns against mixing arbitrary Arrow crate versions with DataFusion-facing Arrow types. 

---

## 43.4 Value case: why non-SQL planning is the agent-preferred path

| Requirement               | Programmatic planning advantage                                  |
| ------------------------- | ---------------------------------------------------------------- |
| avoid SQL injection       | no string interpolation for user literals                        |
| typed request compilation | map validated enums/fields directly to `Expr`                    |
| schema preflight          | validate `Expr` against `DFSchema` before execution              |
| deterministic output      | enforce aliases, sort keys, limits, projections                  |
| policy linting            | inspect `LogicalPlan` before execution                           |
| reusable templates        | compile `PlanSpec` / `ExprSpec` into fixed builder patterns      |
| service guardrails        | append filters/limits/masks after optional user SQL              |
| custom language backend   | bypass SQL parser and emit relational algebra directly           |
| testability               | unit-test expression factories and plan factories independently  |
| optimizer leverage        | built-in logical nodes still benefit from pushdown/rewrite rules |

DataFusion’s own `DataFrame` workflow is source creation through `SessionContext`, plan construction by methods such as `filter`, `select`, `aggregate`, and `limit`, and execution by `collect`; the docs emphasize that DataFrame methods are lazy plan builders. ([Docs.rs][1])

---

## 43.5 Programmatic planning layers

```text id="6zpryp"
Layer 0 — request DTO:
  user_id, filters, dimensions, measures, sort, limit

Layer 1 — semantic validation:
  table exists
  column exists
  column type compatible
  function allowed
  tenant policy satisfied

Layer 2 — Expr construction:
  col("amount").gt(lit(0.0))
  sum(col("amount")).alias("total_amount")
  col("event_ts").sort(false, false)

Layer 3 — relation construction:
  DataFrame.filter/select/aggregate/join/window/sort/limit
  or LogicalPlanBuilder.filter/project/aggregate/join/window/sort/limit

Layer 4 — artifact extraction:
  df.schema()
  df.logical_plan()
  df.into_optimized_plan()
  state.create_physical_plan(...)

Layer 5 — execution:
  execute_stream
  collect only for bounded tests
  write_* for sinks
```

`DataFrame::schema` returns the `DFSchema` describing output names, types, and nullability; `DataFrame::logical_plan` returns the unoptimized `LogicalPlan`; `into_unoptimized_plan` and `into_optimized_plan` are documented as testing-oriented because they lose the `SessionState` snapshot associated with the `DataFrame`. ([Docs.rs][1])

---

## 43.6 Source factory patterns

### 43.6.1 DataFrame source factory

```rust id="9dmng9"
use datafusion::prelude::*;

pub enum SourceSpec {
    RegisteredTable { name: String },
    Csv { path: String, has_header: bool },
    Parquet { path: String },
    InMemoryBatch { batch: RecordBatch },
}

pub async fn dataframe_source(
    ctx: &SessionContext,
    spec: SourceSpec,
) -> Result<DataFrame> {
    match spec {
        SourceSpec::RegisteredTable { name } => {
            ctx.table(name.as_str()).await
        }

        SourceSpec::Csv { path, has_header } => {
            ctx.read_csv(
                path.as_str(),
                CsvReadOptions::new().has_header(has_header),
            )
            .await
        }

        SourceSpec::Parquet { path } => {
            ctx.read_parquet(path.as_str(), ParquetReadOptions::default())
                .await
        }

        SourceSpec::InMemoryBatch { batch } => {
            ctx.read_batch(batch)
        }
    }
}
```

Use this for ordinary application pipelines. `DataFrame` creation methods live on `SessionContext`, and the docs list `read_csv`, `read_parquet`, `read_batch`, and table-based workflows as core DataFrame creation patterns. ([Docs.rs][1])

### 43.6.2 `LogicalPlanBuilder` source factory

```rust id="jo409i"
use std::sync::Arc;

use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder, LogicalTableSource};
use datafusion::prelude::*;

pub fn logical_source_from_schema(
    table_name: impl Into<String>,
    schema: SchemaRef,
) -> Result<LogicalPlanBuilder> {
    let source = Arc::new(LogicalTableSource::new(schema));
    LogicalPlanBuilder::scan(table_name.into(), source, None)
}

pub async fn logical_source_from_registered_table(
    ctx: &SessionContext,
    table_name: &str,
) -> Result<LogicalPlanBuilder> {
    let provider = ctx.table_provider(table_name).await?;
    let source = datafusion::datasource::provider_as_source(provider);
    LogicalPlanBuilder::scan(table_name, source, None)
}
```

Use `LogicalTableSource` only for logical construction/testing when execution is not required; use a real `TableProvider`-backed source for executable plans. The uploaded logical-plan section warns against confusing `LogicalTableSource` with real execution and recommends `TableProvider`-backed scans for executable plans. 

---

## 43.7 Projection factory

### 43.7.1 Stable projection spec

```rust id="0mcb1v"
#[derive(Debug, Clone)]
pub enum ProjectionItem {
    Column {
        name: String,
        alias: Option<String>,
    },
    Derived {
        expr: Expr,
        alias: String,
    },
}

pub fn projection_exprs(items: &[ProjectionItem]) -> Vec<Expr> {
    items
        .iter()
        .map(|item| match item {
            ProjectionItem::Column { name, alias } => {
                let e = col(name.as_str());
                match alias {
                    Some(alias) => e.alias(alias),
                    None => e,
                }
            }
            ProjectionItem::Derived { expr, alias } => {
                expr.clone().alias(alias)
            }
        })
        .collect()
}
```

### 43.7.2 DataFrame projection

```rust id="9dsaw8"
pub fn apply_projection_df(
    df: DataFrame,
    items: &[ProjectionItem],
) -> Result<DataFrame> {
    df.select(projection_exprs(items))
}
```

### 43.7.3 LogicalPlanBuilder projection

```rust id="dq6stq"
pub fn apply_projection_builder(
    builder: LogicalPlanBuilder,
    items: &[ProjectionItem],
) -> Result<LogicalPlanBuilder> {
    builder.project(projection_exprs(items))
}
```

`DataFrame::select` projects arbitrary expressions and gives one output column per expression. The docs show that unaliased computed expressions produce expression-derived names such as `?table?.b * ?table?.c`, which is a strong reason to alias generated expressions. ([Docs.rs][1])

Agent projection policy:

```text id="a0w6ik"
Public output:
  explicit projection only
  no SELECT *
  alias every computed expression
  alias duplicate source fields
  lowercase snake_case output names

Internal intermediate:
  qualifiers may remain
  aliases still recommended for computed fields
```

---

## 43.8 Filter factory

### 43.8.1 Typed filter spec

```rust id="31ev4x"
#[derive(Debug, Clone)]
pub enum ScalarLiteral {
    I64(i64),
    F64(f64),
    Utf8(String),
    Bool(bool),
    Null,
}

impl ScalarLiteral {
    pub fn to_expr(&self) -> Expr {
        match self {
            ScalarLiteral::I64(v) => lit(*v),
            ScalarLiteral::F64(v) => lit(*v),
            ScalarLiteral::Utf8(v) => lit(v.as_str()),
            ScalarLiteral::Bool(v) => lit(*v),
            ScalarLiteral::Null => lit(ScalarValue::Null),
        }
    }
}

#[derive(Debug, Clone)]
pub enum FilterOp {
    Eq,
    NotEq,
    Gt,
    GtEq,
    Lt,
    LtEq,
    In,
    IsNull,
    IsNotNull,
}

#[derive(Debug, Clone)]
pub struct FilterSpec {
    pub column: String,
    pub op: FilterOp,
    pub value: Option<ScalarLiteral>,
    pub values: Vec<ScalarLiteral>,
}
```

### 43.8.2 Filter expression compiler

```rust id="c9npmj"
pub fn compile_filter(spec: &FilterSpec) -> Result<Expr> {
    let c = col(spec.column.as_str());

    let expr = match spec.op {
        FilterOp::Eq => c.eq(required_value(spec)?),
        FilterOp::NotEq => c.not_eq(required_value(spec)?),
        FilterOp::Gt => c.gt(required_value(spec)?),
        FilterOp::GtEq => c.gt_eq(required_value(spec)?),
        FilterOp::Lt => c.lt(required_value(spec)?),
        FilterOp::LtEq => c.lt_eq(required_value(spec)?),
        FilterOp::In => {
            let values = spec.values
                .iter()
                .map(ScalarLiteral::to_expr)
                .collect::<Vec<_>>();
            c.in_list(values, false)
        }
        FilterOp::IsNull => c.is_null(),
        FilterOp::IsNotNull => c.is_not_null(),
    };

    Ok(expr)
}

fn required_value(spec: &FilterSpec) -> Result<Expr> {
    spec.value
        .as_ref()
        .map(ScalarLiteral::to_expr)
        .ok_or_else(|| DataFusionError::Plan(
            format!("missing scalar value for filter on {}", spec.column)
        ))
}
```

### 43.8.3 Conjunction builder

```rust id="kxb7hb"
pub fn compile_filters(filters: &[FilterSpec]) -> Result<Option<Expr>> {
    let mut compiled = filters
        .iter()
        .map(compile_filter)
        .collect::<Result<Vec<_>>>()?
        .into_iter();

    let Some(first) = compiled.next() else {
        return Ok(None);
    };

    Ok(Some(compiled.fold(first, |acc, e| acc.and(e))))
}
```

### 43.8.4 Apply filter

```rust id="klfg8x"
pub fn apply_filter_df(df: DataFrame, filters: &[FilterSpec]) -> Result<DataFrame> {
    match compile_filters(filters)? {
        Some(predicate) => df.filter(predicate),
        None => Ok(df),
    }
}

pub fn apply_filter_builder(
    builder: LogicalPlanBuilder,
    filters: &[FilterSpec],
) -> Result<LogicalPlanBuilder> {
    match compile_filters(filters)? {
        Some(predicate) => builder.filter(predicate),
        None => Ok(builder),
    }
}
```

`LogicalPlanBuilder::filter` applies a filter expression, and `DataFrame::filter` similarly returns a new planned DataFrame; neither reads rows at construction time. ([Docs.rs][2])

Agent filter policy:

```text id="s8c5qr"
Never concatenate SQL filter strings from user input.
Compile typed filter specs to Expr.
Validate column existence before Expr construction.
Validate literal type compatibility before execution.
Use is_null / is_not_null for NULL; never eq(NULL).
```

---

## 43.9 Aggregation factory

### 43.9.1 Aggregate spec

```rust id="6xwvzs"
#[derive(Debug, Clone)]
pub enum AggregateFn {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

#[derive(Debug, Clone)]
pub struct MeasureSpec {
    pub func: AggregateFn,
    pub column: Option<String>,  // None for count(*)
    pub alias: String,
}

#[derive(Debug, Clone)]
pub struct AggregateSpec {
    pub group_by: Vec<String>,
    pub measures: Vec<MeasureSpec>,
}
```

### 43.9.2 Aggregate expression compiler

```rust id="tn3c3s"
pub fn compile_measure(measure: &MeasureSpec) -> Result<Expr> {
    let arg = match &measure.column {
        Some(c) => col(c.as_str()),
        None => lit(1_i64),
    };

    let expr = match measure.func {
        AggregateFn::Count => count(arg),
        AggregateFn::Sum => sum(arg),
        AggregateFn::Avg => avg(arg),
        AggregateFn::Min => min(arg),
        AggregateFn::Max => max(arg),
    };

    Ok(expr.alias(measure.alias.as_str()))
}

pub fn compile_aggregate(spec: &AggregateSpec) -> Result<(Vec<Expr>, Vec<Expr>)> {
    let groups = spec
        .group_by
        .iter()
        .map(|c| col(c.as_str()))
        .collect::<Vec<_>>();

    let measures = spec
        .measures
        .iter()
        .map(compile_measure)
        .collect::<Result<Vec<_>>>()?;

    Ok((groups, measures))
}
```

### 43.9.3 Apply aggregate

```rust id="fx9l01"
pub fn apply_aggregate_df(df: DataFrame, spec: &AggregateSpec) -> Result<DataFrame> {
    let (groups, measures) = compile_aggregate(spec)?;
    df.aggregate(groups, measures)
}

pub fn apply_aggregate_builder(
    builder: LogicalPlanBuilder,
    spec: &AggregateSpec,
) -> Result<LogicalPlanBuilder> {
    let (groups, measures) = compile_aggregate(spec)?;
    builder.aggregate(groups, measures)
}
```

`LogicalPlanBuilder::aggregate` groups on `group_expr` and computes aggregate expressions for each distinct group, while `DataFrame::aggregate` provides the same logical operation at the DataFrame layer. ([Docs.rs][2])

Agent aggregation policy:

```text id="bcrseb"
Alias every aggregate.
Validate group_by columns before building.
Validate measure columns and numeric compatibility.
Use count(lit(1)) for count(*)-like generated aggregates.
Run post-aggregate filter as a second filter over aggregate output aliases.
```

---

## 43.10 Join factory

### 43.10.1 Join spec

```rust id="7rns6g"
#[derive(Debug, Clone)]
pub enum JoinKind {
    Inner,
    Left,
    Right,
    Full,
    Semi,
    Anti,
}

impl JoinKind {
    pub fn to_join_type(&self) -> JoinType {
        match self {
            JoinKind::Inner => JoinType::Inner,
            JoinKind::Left => JoinType::Left,
            JoinKind::Right => JoinType::Right,
            JoinKind::Full => JoinType::Full,
            JoinKind::Semi => JoinType::LeftSemi,
            JoinKind::Anti => JoinType::LeftAnti,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JoinKey {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone)]
pub struct JoinSpec {
    pub kind: JoinKind,
    pub keys: Vec<JoinKey>,
}
```

### 43.10.2 DataFrame join factory

```rust id="rug0no"
pub fn apply_join_df(
    left: DataFrame,
    right: DataFrame,
    spec: &JoinSpec,
) -> Result<DataFrame> {
    let left_cols = spec.keys.iter().map(|k| k.left.as_str()).collect::<Vec<_>>();
    let right_cols = spec.keys.iter().map(|k| k.right.as_str()).collect::<Vec<_>>();

    left.join(
        right,
        spec.kind.to_join_type(),
        &left_cols,
        &right_cols,
        None,
    )
}
```

### 43.10.3 LogicalPlanBuilder join factory

```rust id="h3l63d"
pub fn apply_join_builder(
    left: LogicalPlanBuilder,
    right: LogicalPlan,
    spec: &JoinSpec,
) -> Result<LogicalPlanBuilder> {
    let left_cols = spec
        .keys
        .iter()
        .map(|k| Column::new_unqualified(k.left.as_str()))
        .collect::<Vec<_>>();

    let right_cols = spec
        .keys
        .iter()
        .map(|k| Column::new_unqualified(k.right.as_str()))
        .collect::<Vec<_>>();

    left.join(
        right,
        spec.kind.to_join_type(),
        (left_cols, right_cols),
        None,
    )
}
```

`LogicalPlanBuilder::join` applies a join using explicitly specified columns and an optional filter; `join_on` accepts expressions and DataFusion automatically identifies equality predicates, with no performance difference for equality conditions according to the docs. ([Docs.rs][2])

Agent join policy:

```text id="8uqffe"
Validate both input schemas before join.
Rename duplicate columns before or immediately after join.
Prefer join(...) for simple equijoins.
Prefer join_on(...) for generated expression predicates.
Avoid cross_join unless explicitly requested.
Qualify columns after join-derived plan construction.
```

---

## 43.11 Window factory

### 43.11.1 Window plan application

```rust id="b5mh48"
pub struct WindowSpec {
    pub exprs: Vec<Expr>,
}

pub fn apply_window_df(df: DataFrame, spec: WindowSpec) -> Result<DataFrame> {
    df.window(spec.exprs)
}

pub fn apply_window_builder(
    builder: LogicalPlanBuilder,
    spec: WindowSpec,
) -> Result<LogicalPlanBuilder> {
    builder.window(spec.exprs)
}
```

`DataFrame::window` adds the result of one or more window-function expressions to the existing columns, and `LogicalPlanBuilder::window` similarly extends the schema with window functions. ([Docs.rs][1])

### 43.11.2 Agent window guidance

```text id="ofhacl"
Window expressions should be prebuilt through DataFusion window-function APIs or parsed from trusted SQL expression text.
Always alias window outputs.
Always specify deterministic ORDER BY for ranking/window-offset semantics.
Apply QUALIFY-like filters after window creation using filter over the alias.
Avoid window factories in generic request paths until window expression DSL is tightly validated.
```

Example shape:

```rust id="lv6kun"
// Pseudocode: exact window helper constructors vary by DataFusion version/function module.
// Build/validate window Exprs under pinned docs.rs version, then pass Vec<Expr> into df.window(...).
let df = df
    .window(vec![
        row_number_expr
            .alias("region_rank")
    ])?
    .filter(col("region_rank").lt_eq(lit(10_i64)))?;
```

---

## 43.12 Sort and limit factory

```rust id="2tcfia"
#[derive(Debug, Clone)]
pub struct SortSpec {
    pub column: String,
    pub asc: bool,
    pub nulls_first: bool,
}

pub fn compile_sort(specs: &[SortSpec]) -> Vec<SortExpr> {
    specs
        .iter()
        .map(|s| col(s.column.as_str()).sort(s.asc, s.nulls_first))
        .collect()
}

pub fn apply_sort_limit_df(
    df: DataFrame,
    sort: &[SortSpec],
    limit: Option<usize>,
) -> Result<DataFrame> {
    let df = if sort.is_empty() {
        df
    } else {
        df.sort(compile_sort(sort))?
    };

    match limit {
        Some(n) => df.limit(0, Some(n)),
        None => Ok(df),
    }
}

pub fn apply_sort_limit_builder(
    builder: LogicalPlanBuilder,
    sort: &[SortSpec],
    limit: Option<usize>,
) -> Result<LogicalPlanBuilder> {
    let builder = if sort.is_empty() {
        builder
    } else {
        builder.sort(compile_sort(sort))?
    };

    match limit {
        Some(n) => builder.limit(0, Some(n)),
        None => Ok(builder),
    }
}
```

`LogicalPlanBuilder` exposes `sort`, `sort_by`, `sort_with_limit`, `limit`, and `limit_by_expr`; `limit` accepts `skip` and optional `fetch`. ([Docs.rs][2])

Agent policy:

```text id="9ueufb"
For top-N semantics:
  sort first
  limit second
  add deterministic tie-breaker columns

For service result caps:
  add limit even if caller omitted one
  use sort only if deterministic ordering is required

For performance:
  prefer sort_with_limit when constructing builder-level top-k plans and appropriate for the pinned API
```

---

## 43.13 Sink/write factory

### 43.13.1 DataFrame sink spec

```rust id="04x91j"
use datafusion::dataframe::DataFrameWriteOptions;

pub enum SinkSpec {
    Stream,
    CollectBounded,
    Csv { path: String },
    Json { path: String },
    Parquet { path: String },
    Table { name: String },
}

pub async fn execute_sink(
    df: DataFrame,
    sink: SinkSpec,
) -> Result<Option<Vec<RecordBatch>>> {
    match sink {
        SinkSpec::Stream => {
            let mut stream = df.execute_stream().await?;
            // caller should consume stream; this placeholder consumes zero batches
            // in real service code, return stream from a lower-level function instead
            Ok(None)
        }

        SinkSpec::CollectBounded => {
            Ok(Some(df.collect().await?))
        }

        SinkSpec::Csv { path } => {
            let batches = df.write_csv(
                path.as_str(),
                DataFrameWriteOptions::new(),
                None,
            ).await?;
            Ok(Some(batches))
        }

        SinkSpec::Json { path } => {
            let batches = df.write_json(
                path.as_str(),
                DataFrameWriteOptions::new(),
                None,
            ).await?;
            Ok(Some(batches))
        }

        SinkSpec::Parquet { path } => {
            let batches = df.write_parquet(
                path.as_str(),
                DataFrameWriteOptions::new(),
                None,
            ).await?;
            Ok(Some(batches))
        }

        SinkSpec::Table { name } => {
            let batches = df.write_table(
                name.as_str(),
                DataFrameWriteOptions::new(),
            ).await?;
            Ok(Some(batches))
        }
    }
}
```

`DataFrame::write_parquet` executes a DataFrame and writes results to Parquet files; `DataFrame` also exposes `write_csv`, `write_json`, and `write_table` methods. ([Docs.rs][1]) `DataFrame::collect` buffers all output `RecordBatch`es into memory, while `execute_stream` returns a stream without buffering all batches. ([Docs.rs][1])

Service sink policy:

```text id="iavh9z"
HTTP / gRPC / Flight:
  execute_stream

small tests:
  collect

exports:
  write_parquet / write_csv / write_json

registered mutable sink:
  write_table, only when TableProvider::insert_into semantics are known
```

---

## 43.14 Avoiding SQL string generation

### Bad pattern

```rust id="af2oml"
let sql = format!(
    "SELECT {} FROM {} WHERE {} = '{}'",
    user_selected_column,
    user_selected_table,
    user_filter_column,
    user_filter_value,
);

let df = ctx.sql(&sql).await?;
```

Problems:

```text id="f5p8kt"
identifier injection
literal injection
dialect-specific quoting
ambiguous columns
type coercion surprises
unbounded output
harder linting
unstable output aliases
```

### Good pattern

```rust id="k32qa7"
pub struct QueryRequest {
    pub source: SourceSpec,
    pub projection: Vec<ProjectionItem>,
    pub filters: Vec<FilterSpec>,
    pub sort: Vec<SortSpec>,
    pub limit: Option<usize>,
}

pub async fn compile_request_df(
    ctx: &SessionContext,
    request: QueryRequest,
) -> Result<DataFrame> {
    let df = dataframe_source(ctx, request.source).await?;
    validate_request_against_schema(df.schema(), &request)?;

    let df = apply_filter_df(df, &request.filters)?;
    let df = apply_projection_df(df, &request.projection)?;
    let df = apply_sort_limit_df(df, &request.sort, request.limit)?;

    Ok(df)
}
```

Agent invariant:

```text id="w4e05q"
Generated plan = typed data structure → Expr → DataFrame/LogicalPlanBuilder.
SQL string = only for human-authored SQL or explicitly product-facing dialect output.
```

---

## 43.15 Dynamic column validation

### 43.15.1 Basic schema validation

```rust id="gr5vny"
pub fn has_unqualified_column(schema: &DFSchema, name: &str) -> bool {
    schema.columns_with_unqualified_name(name).len() == 1
}

pub fn require_unqualified_column(schema: &DFSchema, name: &str) -> Result<()> {
    let matches = schema.columns_with_unqualified_name(name);

    match matches.len() {
        1 => Ok(()),
        0 => Err(DataFusionError::Plan(format!("unknown column: {name}"))),
        n => Err(DataFusionError::Plan(format!(
            "ambiguous column: {name}; {n} matches; qualify the column"
        ))),
    }
}
```

`DFSchema` exposes methods for qualified and unqualified column lookup, including iterating qualifiers/fields and finding columns by unqualified name. ([Docs.rs][5])

### 43.15.2 Request-wide validation

```rust id="jgs0qx"
pub fn validate_request_against_schema(
    schema: &DFSchema,
    request: &QueryRequest,
) -> Result<()> {
    for p in &request.projection {
        if let ProjectionItem::Column { name, .. } = p {
            require_unqualified_column(schema, name)?;
        }
    }

    for f in &request.filters {
        require_unqualified_column(schema, &f.column)?;
    }

    for s in &request.sort {
        require_unqualified_column(schema, &s.column)?;
    }

    Ok(())
}
```

Validation policy:

```text id="ap4q5w"
Before constructing col(name):
  ensure name appears exactly once
  reject unknown names
  reject ambiguous names
  enforce authorization for name
  enforce type compatibility for operation
```

---

## 43.16 `DFSchema` expression preflight

### 43.16.1 Type/nullability inference

```rust id="ii6opt"
use datafusion::logical_expr::ExprSchemable;

pub fn preflight_expr(schema: &DFSchema, expr: &Expr) -> Result<(DataType, bool)> {
    let data_type = expr.get_type(schema)?;
    let nullable = expr.nullable(schema)?;
    Ok((data_type, nullable))
}
```

`DataFrame::schema` returns a `DFSchema` with output column names, types, and nullability, and `ExprSchemable` can infer type/nullability of expressions against such a schema. ([Docs.rs][1])

### 43.16.2 Filter type validation

```rust id="9024al"
pub fn validate_filter_expr(schema: &DFSchema, predicate: &Expr) -> Result<()> {
    let (data_type, _nullable) = preflight_expr(schema, predicate)?;

    if data_type != DataType::Boolean {
        return Err(DataFusionError::Plan(format!(
            "filter expression must be Boolean, got {data_type:?}: {predicate:?}"
        )));
    }

    Ok(())
}
```

### 43.16.3 Projection schema preview

```rust id="rrr6ui"
pub fn preview_projection_schema(
    input: &DFSchema,
    projection: &[Expr],
) -> Result<Vec<(String, DataType, bool)>> {
    projection
        .iter()
        .map(|expr| {
            let (_qualifier, field) = expr.to_field(input)?;
            Ok((
                field.name().clone(),
                field.data_type().clone(),
                field.is_nullable(),
            ))
        })
        .collect()
}
```

Agent preflight policy:

```text id="9l56xb"
Compile Expr.
Infer type/nullability.
Reject invalid filters before plan construction.
Preview output schema before execution.
Reject duplicate output names.
Alias computed outputs.
```

---

## 43.17 Duplicate-name prevention

```rust id="62y5hx"
use std::collections::HashSet;

pub fn reject_duplicate_output_names(
    schema_preview: &[(String, DataType, bool)],
) -> Result<()> {
    let mut seen = HashSet::new();

    for (name, _dt, _nullable) in schema_preview {
        if !seen.insert(name.clone()) {
            return Err(DataFusionError::Plan(format!(
                "duplicate output column name: {name}"
            )));
        }
    }

    Ok(())
}
```

Recommended output contract:

```text id="m4o1rk"
Public schema:
  no duplicate names
  no expression-derived names
  no relation-qualified names unless product contract requires them
  deterministic aliases for all computed columns
  stable order
```

DataFrame docs show expression-derived names in output when computed projections are not aliased, reinforcing that aliases are required for stable generated schemas. ([Docs.rs][1])

---

## 43.18 Reusable plan templates

### 43.18.1 Template as pure Rust function

```rust id="0miwa1"
#[derive(Debug, Clone)]
pub struct SalesRollupSpec {
    pub source: SourceSpec,
    pub dimensions: Vec<String>,
    pub amount_col: String,
    pub filters: Vec<FilterSpec>,
    pub sort_desc_by_total: bool,
    pub limit: Option<usize>,
}

pub async fn sales_rollup_df(
    ctx: &SessionContext,
    spec: SalesRollupSpec,
) -> Result<DataFrame> {
    let df = dataframe_source(ctx, spec.source).await?;

    for dim in &spec.dimensions {
        require_unqualified_column(df.schema(), dim)?;
    }
    require_unqualified_column(df.schema(), &spec.amount_col)?;

    let df = apply_filter_df(df, &spec.filters)?;

    let group_exprs = spec
        .dimensions
        .iter()
        .map(|d| col(d.as_str()))
        .collect::<Vec<_>>();

    let aggr_exprs = vec![
        sum(col(spec.amount_col.as_str())).alias("total_amount"),
        count(lit(1_i64)).alias("row_count"),
    ];

    let df = df.aggregate(group_exprs, aggr_exprs)?;

    let df = if spec.sort_desc_by_total {
        df.sort(vec![col("total_amount").sort(false, false)])?
    } else {
        df
    };

    match spec.limit {
        Some(n) => df.limit(0, Some(n)),
        None => Ok(df),
    }
}
```

### 43.18.2 Template properties

```text id="vu4ivk"
Plan template:
  stable operator sequence
  validated dynamic fields
  stable output aliases
  stable output order
  bounded optional limit
  deterministic sort when requested
  pure compile step before execution
```

### 43.18.3 Template fingerprint

```rust id="a2npue"
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanTemplateFingerprint {
    pub template_id: String,
    pub template_version: String,
    pub datafusion_version: String,
    pub source_schema_hash: String,
    pub request_hash: String,
    pub output_schema_hash: String,
}
```

Cache policy:

```text id="17jbca"
Cache template metadata, not blindly physical plans.
Invalidate on:
  DataFusion version
  config/options
  schema hash
  function registry
  optimizer rule set
  table statistics version
```

---

## 43.19 Parameterized plans

### 43.19.1 SQL placeholders + `DataFrame::with_param_values`

```rust id="47xqqs"
use datafusion::common::ScalarValue;
use datafusion::prelude::*;

let results = ctx
    .sql("SELECT a FROM example WHERE b = $1")
    .await?
    .with_param_values(vec![
        ScalarValue::from(2_i64), // index 0 → $1
    ])?
    .collect()
    .await?;
```

`DataFrame::with_param_values` replaces all parameters in a logical plan with supplied values before execution; docs show both positional `$1` and named `$my_param` examples. ([Docs.rs][1])

Named parameter shape:

```rust id="s6vbgf"
let results = ctx
    .sql("SELECT a FROM example WHERE b = $my_param")
    .await?
    .with_param_values(vec![
        ("my_param", ScalarValue::from(2_i64)),
    ])?
    .collect()
    .await?;
```

### 43.19.2 Builder-level placeholders

`LogicalPlanBuilder::values` and `values_with_schema` docs note that if values include params/binders such as `$1`, `$2`, or `$3`, parameter data types should be provided; this matters when logical values relations contain binders rather than concrete typed literals. ([Docs.rs][2])

### 43.19.3 Preferred generated-plan parameter strategy

For non-SQL generated plans, prefer **typed template inputs** over SQL-style placeholders:

```rust id="lt6avm"
#[derive(Debug, Clone)]
pub struct UserQueryParams {
    pub min_amount: f64,
    pub event_type: String,
    pub limit: usize,
}

pub fn compile_param_filters(params: &UserQueryParams) -> Expr {
    col("amount")
        .gt_eq(lit(params.min_amount))
        .and(col("event_type").eq(lit(params.event_type.as_str())))
}
```

Parameter strategy matrix:

| Pattern                         | Use when                           | Validation                             |
| ------------------------------- | ---------------------------------- | -------------------------------------- |
| direct typed literals in `Expr` | generated DataFrame/Builder plans  | Rust type + schema preflight           |
| SQL `$1` + `with_param_values`  | SQL text is product/user interface | parameter count/name/type validation   |
| named parameters                | query template authored as SQL     | name whitelist + value type validation |
| builder placeholders            | values/prepare-like logical plans  | provide parameter data types           |
| custom PlanSpec parameters      | semantic compiler                  | validate before `Expr` construction    |

Agent rules:

```text id="ns5lqh"
For generated plans:
  prefer typed Rust params → lit(value).

For SQL templates:
  use placeholders + with_param_values.
  never interpolate literals into SQL strings.

For reusable PlanSpec:
  store typed parameter schema.
  validate supplied values before compilation.
```

---

## 43.20 Parameter type validation

```rust id="xja9hp"
#[derive(Debug, Clone)]
pub enum ParamType {
    Int64,
    Float64,
    Utf8,
    Boolean,
}

#[derive(Debug, Clone)]
pub struct ParamDef {
    pub name: String,
    pub data_type: ParamType,
    pub nullable: bool,
}

pub fn validate_scalar_param(def: &ParamDef, value: &ScalarValue) -> Result<()> {
    let ok = matches!(
        (&def.data_type, value),
        (ParamType::Int64, ScalarValue::Int64(_))
            | (ParamType::Float64, ScalarValue::Float64(_))
            | (ParamType::Utf8, ScalarValue::Utf8(_))
            | (ParamType::Boolean, ScalarValue::Boolean(_))
    );

    if !ok {
        return Err(DataFusionError::Plan(format!(
            "parameter {} has wrong type: expected {:?}, got {:?}",
            def.name, def.data_type, value
        )));
    }

    if !def.nullable && value.is_null() {
        return Err(DataFusionError::Plan(format!(
            "parameter {} is non-nullable but got NULL",
            def.name
        )));
    }

    Ok(())
}
```

Deployment rule:

```text id="b7g5tf"
Parameter validation belongs before plan execution.
For SQL placeholders, validate supplied ScalarValue vector/map.
For direct Expr literals, validate request DTO before calling lit(...).
For cache keys, include parameter type signature but not sensitive parameter values.
```

---

## 43.21 App request → typed query spec → `Expr` / `LogicalPlanBuilder` → optimized plan → stream

### 43.21.1 Request DTO

```rust id="vkqkdh"
#[derive(Debug, Clone)]
pub struct QuerySpec {
    pub source: SourceSpec,
    pub projection: Vec<ProjectionItem>,
    pub filters: Vec<FilterSpec>,
    pub aggregate: Option<AggregateSpec>,
    pub sort: Vec<SortSpec>,
    pub limit: Option<usize>,
}
```

### 43.21.2 Compile to DataFrame

```rust id="nc7mkt"
pub async fn compile_to_dataframe(
    ctx: &SessionContext,
    spec: QuerySpec,
) -> Result<DataFrame> {
    let df = dataframe_source(ctx, spec.source).await?;

    validate_request_against_schema(df.schema(), &QueryRequest {
        source: SourceSpec::RegisteredTable { name: "_already_bound".to_string() },
        projection: spec.projection.clone(),
        filters: spec.filters.clone(),
        sort: spec.sort.clone(),
        limit: spec.limit,
    })?;

    let df = apply_filter_df(df, &spec.filters)?;

    let df = match &spec.aggregate {
        Some(agg) => apply_aggregate_df(df, agg)?,
        None => df,
    };

    let df = if spec.projection.is_empty() {
        df
    } else {
        apply_projection_df(df, &spec.projection)?
    };

    let df = apply_sort_limit_df(df, &spec.sort, spec.limit)?;

    Ok(df)
}
```

### 43.21.3 Compile to optimized logical artifact

```rust id="irbfaw"
pub async fn compile_to_optimized_logical(
    ctx: &SessionContext,
    spec: QuerySpec,
) -> Result<LogicalPlan> {
    let df = compile_to_dataframe(ctx, spec).await?;

    // Testing/diagnostic path; loses DataFrame's state snapshot.
    // Use into_parts() when you need both SessionState and LogicalPlan.
    df.into_optimized_plan()
}
```

`DataFrame::into_optimized_plan` returns the optimized logical plan but is documented as testing-oriented for the same reason as `into_unoptimized_plan`: it loses the `SessionState` snapshot attached to the DataFrame. ([Docs.rs][1])

### 43.21.4 Compile and stream

```rust id="0bk9wm"
use futures::StreamExt;

pub async fn stream_query_spec(
    ctx: &SessionContext,
    spec: QuerySpec,
) -> Result<()> {
    let df = compile_to_dataframe(ctx, spec).await?;

    let mut stream = df.execute_stream().await?;

    while let Some(batch) = stream.next().await {
        let batch = batch?;
        // serialize batch to Arrow IPC / JSON / CSV / Flight / HTTP chunk
        println!("rows={}", batch.num_rows());
    }

    Ok(())
}
```

`DataFrame::execute_stream` executes a DataFrame and returns a stream over a single partition; dropping the stream aborts execution and frees allocated resources. ([Docs.rs][1])

---

## 43.22 Direct `LogicalPlanBuilder` compiler pattern

```rust id="bt6xyd"
pub struct BuilderQuerySpec {
    pub table_name: String,
    pub projection: Vec<ProjectionItem>,
    pub filters: Vec<FilterSpec>,
    pub aggregate: Option<AggregateSpec>,
    pub sort: Vec<SortSpec>,
    pub limit: Option<usize>,
}

pub async fn compile_builder_plan(
    ctx: &SessionContext,
    spec: BuilderQuerySpec,
) -> Result<LogicalPlan> {
    let provider = ctx.table_provider(spec.table_name.as_str()).await?;
    let source = datafusion::datasource::provider_as_source(provider);

    let mut builder = LogicalPlanBuilder::scan(
        spec.table_name.as_str(),
        source,
        None,
    )?;

    if let Some(predicate) = compile_filters(&spec.filters)? {
        builder = builder.filter(predicate)?;
    }

    if let Some(agg) = &spec.aggregate {
        builder = apply_aggregate_builder(builder, agg)?;
    }

    if !spec.projection.is_empty() {
        builder = apply_projection_builder(builder, &spec.projection)?;
    }

    if !spec.sort.is_empty() {
        builder = builder.sort(compile_sort(&spec.sort))?;
    }

    if let Some(limit) = spec.limit {
        builder = builder.limit(0, Some(limit))?;
    }

    builder.build()
}
```

`LogicalPlanBuilder` exposes `schema()` to inspect the output schema of the plan built so far and `plan()` to inspect the current logical plan; this makes it suitable for stepwise compilers that validate after each factory stage. ([Docs.rs][2])

---

## 43.23 Output schema stability

### 43.23.1 Output schema contract

```rust id="5b7fko"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFieldContract {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}

pub fn output_contract_from_df(df: &DataFrame) -> Vec<OutputFieldContract> {
    df.schema()
        .iter()
        .map(|(_qualifier, field)| OutputFieldContract {
            name: field.name().clone(),
            data_type: field.data_type().clone(),
            nullable: field.is_nullable(),
        })
        .collect()
}
```

### 43.23.2 Alias enforcement

```rust id="8adfim"
pub fn derived(expr: Expr, alias: impl Into<String>) -> ProjectionItem {
    ProjectionItem::Derived {
        expr,
        alias: alias.into(),
    }
}

let projection = vec![
    ProjectionItem::Column {
        name: "customer_id".to_string(),
        alias: None,
    },
    derived(
        col("amount") * lit(1.08_f64),
        "amount_with_tax",
    ),
];
```

### 43.23.3 Duplicate prevention after compile

```rust id="b6ujks"
pub fn validate_output_schema(df: &DataFrame) -> Result<()> {
    let preview = output_contract_from_df(df)
        .into_iter()
        .map(|f| (f.name, f.data_type, f.nullable))
        .collect::<Vec<_>>();

    reject_duplicate_output_names(&preview)
}
```

Output stability policy:

```text id="qc4hfd"
Always:
  alias computed fields
  alias aggregate fields
  alias window fields
  reject duplicate output names
  snapshot output schema in tests
  use stable lowercase snake_case

Never:
  expose ?table?.b * ?table?.c as public field name
  rely on join duplicate-field behavior
  rely on physical plan order without sort
```

---

## 43.24 Plan template validation pipeline

```text id="3v7eib"
1. Parse request DTO.
2. Validate source availability.
3. Create source DataFrame or builder.
4. Validate dynamic columns against source DFSchema.
5. Compile filters to Expr.
6. Preflight filter expr returns Boolean.
7. Compile projection / aggregate / window exprs.
8. Preview output schema.
9. Reject duplicate/unstable output names.
10. Apply sort/limit policy.
11. Produce DataFrame or LogicalPlan.
12. Run logical-plan policy lint.
13. Optimize.
14. Execute stream.
```

Canonical compiler function:

```rust id="ic41tu"
pub async fn compile_validate_streamable(
    ctx: &SessionContext,
    spec: QuerySpec,
) -> Result<DataFrame> {
    let df = compile_to_dataframe(ctx, spec).await?;

    validate_output_schema(&df)?;

    // Optional:
    // validate_logical_plan_policy(df.logical_plan())?;

    Ok(df)
}
```

---

## 43.25 Hybrid pattern: user SQL + programmatic guardrails

```rust id="epzpdq"
pub async fn guarded_user_sql(
    ctx: &SessionContext,
    user_sql: &str,
    tenant_id: &str,
    max_rows: usize,
) -> Result<DataFrame> {
    let options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    let df = ctx.sql_with_options(user_sql, options).await?;

    // Apply trusted service guardrails programmatically.
    let df = df
        .filter(col("tenant_id").eq(lit(tenant_id)))?
        .limit(0, Some(max_rows))?;

    // Optional: validate logical plan policy after appending guardrails.
    // validate_logical_plan_policy(df.logical_plan())?;

    Ok(df)
}
```

This pattern keeps SQL as a product/user surface but uses `Expr`/DataFrame transformations for non-bypassable application constraints. The DataFrame API supports composing additional transformations after SQL creates the initial DataFrame because SQL returns a lazy DataFrame, not rows. ([Docs.rs][1])

---

## 43.26 Deployment architecture

```text id="1uqp37"
query-core/
  catalog.rs
    register_tables(ctx)
    register_object_stores(runtime)
    table allowlists

  schema.rs
    DFSchema validation
    output contract snapshots
    type compatibility functions

  expr_factories.rs
    compile_filter
    compile_projection
    compile_measure
    compile_sort
    UDF call helpers

  plan_factories.rs
    dataframe_source
    compile_to_dataframe
    compile_builder_plan
    compile_to_logical

  policy.rs
    logical plan lint
    table/column/function authorization
    cross join denial
    direct path denial
    unbounded scan policy

  execute.rs
    execute_stream
    collect_bounded
    write_parquet/csv/json/table
    timeout/cancellation wrappers
```

Service profile:

```text id="yvb2sw"
Shared engine:
  configured SessionContext
  registered catalogs/tables/functions
  RuntimeEnv with memory/temp/object-store config

Per request:
  validate QuerySpec
  compile DataFrame
  validate schema/policy
  execute_stream
  serialize batches
  emit metrics/artifacts
```

---

## 43.27 Testing strategy

### Unit tests

```text id="vrpe6b"
expr_factories:
  compile_filter Eq/Gt/In/IsNull
  reject missing value
  reject type mismatch

schema validation:
  unknown column
  ambiguous column
  duplicate output name
  invalid filter type

plan factories:
  expected logical plan shape
  stable output schema
  stable aliases
  sort-before-limit
```

### Golden logical-plan test

```rust id="2v1740"
#[tokio::test]
async fn generated_rollup_plan_is_stable() -> Result<()> {
    let ctx = test_context().await?;

    let spec = SalesRollupSpec {
        source: SourceSpec::RegisteredTable { name: "sales".to_string() },
        dimensions: vec!["region".to_string()],
        amount_col: "amount".to_string(),
        filters: vec![],
        sort_desc_by_total: true,
        limit: Some(10),
    };

    let df = sales_rollup_df(&ctx, spec).await?;

    let plan = df.logical_plan().display_indent_schema().to_string();

    insta::assert_snapshot!(plan);

    Ok(())
}
```

### Output schema test

```rust id="21zzdk"
#[tokio::test]
async fn generated_rollup_schema_is_stable() -> Result<()> {
    let ctx = test_context().await?;
    let df = sales_rollup_df(&ctx, test_spec()).await?;

    let fields = output_contract_from_df(&df);

    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["region", "total_amount", "row_count"]
    );

    Ok(())
}
```

Testing policy:

```text id="f6chah"
Snapshot logical plan only under pinned DataFusion version.
Prefer schema-contract tests over physical-plan snapshots.
Use result snapshots only with deterministic ORDER BY.
Use collect only for bounded fixture tests.
```

---

## 43.28 Performance and optimizer considerations

```text id="i8bxud"
Push filters before projection when possible.
Project only needed columns before joins/aggregates.
Aggregate after filters.
Join after reducing input where semantically valid.
Sort late.
Limit after sort for top-N.
Use sort_with_limit / top-k-friendly patterns when building directly.
Avoid collect in services.
Prefer execute_stream for large results.
Expose TableProvider statistics/pushdown for custom sources.
```

Important distinction:

```text id="m4rfab"
Logical construction order expresses semantics.
Optimizer may reorder/push down safely.
Do not rely on optimizer to fix every generated inefficiency.
Generate sane plans first.
```

---

## 43.29 Security and governance

```text id="wr6s4f"
Programmatic planning security wins:
  no SQL literal injection
  no arbitrary statement class
  no DDL/DML unless explicitly generated
  central table allowlist
  central column allowlist
  central function allowlist
  deterministic tenant filter insertion
  result limit enforcement
  direct path access avoided
```

Guardrails:

```rust id="wwi2e4"
pub struct QueryPolicy {
    pub allowed_tables: Vec<String>,
    pub allowed_columns: Vec<String>,
    pub max_limit: usize,
    pub require_tenant_filter: bool,
}

pub fn enforce_limit(requested: Option<usize>, policy: &QueryPolicy) -> usize {
    requested
        .unwrap_or(policy.max_limit)
        .min(policy.max_limit)
}
```

Deployment policy:

```text id="f39ozd"
Public APIs:
  expose typed QuerySpec, not raw SQL, whenever possible.

Internal analyst tools:
  raw SQL acceptable with SQLOptions + plan lint.

Tenant-facing services:
  compile tenant filter into Expr.
  do not trust caller-supplied tenant_id filters.
  validate LogicalPlan before execution.

Exports:
  use write_* with output path allowlists.
  never allow arbitrary path construction from user input.
```

---

## 43.30 Anti-pattern inventory

* Generating SQL strings from user-selected column names and literal values.
* Using `select_exprs` with untrusted strings.
* Calling `col(dynamic_name)` without validating `DFSchema`.
* Using unqualified dynamic columns after joins.
* Failing to alias computed, aggregate, or window expressions.
* Exposing expression-derived names as API schema.
* Allowing duplicate output names.
* Calling `collect` in an HTTP service.
* Using `into_unoptimized_plan` / `into_optimized_plan` in production execution paths and losing `SessionState`.
* Treating `LogicalTableSource` as executable data source.
* Building direct `LogicalPlan` enum nodes when `LogicalPlanBuilder` suffices.
* Using `LogicalPlan::Extension` for scalar computations that should be UDFs.
* Forgetting to register UDFs before building expressions that call them.
* Snapshotting exact physical plans for generated plans without version pins.
* Assuming `with_param_values` validates business parameter semantics.
* Caching plans without schema/config/function/policy invalidation.

---

## 43.31 Agent checklist

```text id="kw8v92"
[ ] Choose planning surface:
    SQL | DataFrame | LogicalPlanBuilder | direct LogicalPlan | Extension

[ ] Prefer typed generation:
    request DTO → validation → Expr → DataFrame/LogicalPlanBuilder

[ ] Source factory:
    registered table | CSV | Parquet | RecordBatch | TableProvider-backed scan

[ ] Dynamic names:
    validate against DFSchema before col(name)
    reject unknown and ambiguous names
    enforce authorization

[ ] Expression preflight:
    infer type/nullability
    filter must be Boolean
    projection output names stable
    duplicate output names rejected

[ ] Projection:
    explicit field list
    alias computed expressions
    no SELECT * equivalent in public API

[ ] Filter:
    compile typed ops to Expr
    use is_null/is_not_null
    no SQL string interpolation

[ ] Aggregation:
    validate group and measure columns
    alias all measures
    post-aggregate filters after aggregate

[ ] Join:
    validate left/right schemas
    rename duplicates
    prefer join for equijoins, join_on for expressions
    avoid cross joins

[ ] Window:
    alias outputs
    deterministic ORDER BY inside window spec
    filter window outputs after window stage

[ ] Parameters:
    SQL templates → with_param_values
    generated plans → typed Rust params + lit(...)
    validate parameter type/nullability

[ ] Output:
    snapshot DFSchema
    stable lowercase names
    deterministic sort before top-N limit

[ ] Execution:
    service → execute_stream
    tests → bounded collect
    exports → write_parquet/csv/json/table

[ ] Versioning:
    pin DataFusion
    pin Arrow/DataFusion re-export use
    snapshot logical plans only under pinned versions
```

[1]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html "DataFrame in datafusion::dataframe - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.LogicalPlanBuilder.html "LogicalPlanBuilder in datafusion::logical_expr - Rust"
[3]: https://datafusion.apache.org/_sources/library-user-guide/building-logical-plans.md.txt?utm_source=chatgpt.com "building-logical-plans.md.txt - Apache DataFusion"
[4]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/enum.LogicalPlan.html?utm_source=chatgpt.com "LogicalPlan in datafusion_expr::logical_plan - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html?utm_source=chatgpt.com "DFSchema in datafusion::common - Rust"


# DataFusion Advanced — 44) Plan schema, column identity, aliases, and qualifier governance

## 44.0 Purpose

Make schema identity a **planning contract**, not merely a data-model detail:

```text id="jjpkkb"
Arrow Schema
  → physical data contract
  → fields, types, nullability, metadata
  → RecordBatch / TableProvider / ExecutionPlan output

DFSchema
  → logical planning contract
  → Arrow Schema + optional relation qualifiers + functional dependencies
  → column resolution / ambiguity detection / Expr inference / join planning

Output schema contract
  → external API / sink / view / test artifact
  → stable field names, types, nullability, metadata policy
  → explicit aliases, no duplicate public names, deterministic order
```

The attached documentation already flags this distinction: Arrow `Schema` is for physical data, while DataFusion `DFSchema` adds planning qualifiers and column-resolution semantics; it also explicitly warns agents to use `DFSchema` for planning, preserve qualifiers across multi-table plans, and avoid stringified schema output as compatibility logic.  Current DataFusion docs likewise describe `DFSchema` as wrapping an Arrow schema while adding relation/table qualification; a schema may hold fields across multiple tables, with some fields qualified and others unqualified. ([Docs.rs][1])

---

## 44.1 Value case: why schema identity governance matters

| Failure mode                                   | Root cause                                              | Correct governance                                        |
| ---------------------------------------------- | ------------------------------------------------------- | --------------------------------------------------------- |
| generated plan fails after join                | unqualified `id` resolves ambiguously                   | preserve qualifiers; require `orders.id` / `customers.id` |
| public API schema changes unexpectedly         | computed expression name derived from display string    | alias every computed expression                           |
| optimizer rewrite breaks downstream projection | expression name not preserved                           | rewrite with name/alias preservation invariant            |
| sink write fails                               | logical schema differs from physical output schema      | validate Arrow `Schema` contract at sink boundary         |
| golden tests churn                             | physical/expression display names drift across versions | snapshot explicit schema contracts, not display strings   |
| view output unstable                           | view query uses `SELECT *` over evolving source         | explicit projection + aliases in view definition          |
| UDF output column unreadable                   | function call name becomes field name                   | require `.alias(...)` on UDF calls                        |
| duplicate names after join                     | `SELECT *` over repeated field names                    | final projection with explicit output aliases             |
| metadata lost                                  | `DFSchema → Arrow Schema` conversion drops qualifiers   | preserve `DFSchema` until final output boundary           |

The DataFusion optimizer guide explicitly warns that expression names must be preserved during optimizer rewrites; otherwise expected columns may no longer be found. ([Apache DataFusion][2])

---

## 44.2 `DFSchema` vs Arrow `Schema`

### Arrow `Schema`

```text id="8g8c16"
Arrow Schema:
  fields: Vec<Field>
    field.name
    field.data_type
    field.nullable
    field.metadata
  schema.metadata
  ordered physical column contract
```

Use Arrow `Schema` for:

```text id="d6t2px"
RecordBatch construction
TableProvider::schema()
ExecutionPlan::schema()
file format IO
Arrow IPC / Parquet / CSV / JSON physical contracts
sink/write compatibility
physical test fixtures
```

Arrow `Field` describes one column in a schema and contains name, data type, nullable flag, and custom metadata; an Arrow `Schema` is an ordered collection of such fields. ([Docs.rs][3])

### `DFSchema`

```text id="fb9f6e"
DFSchema:
  inner Arrow Schema
  optional relation qualifier per field
  functional dependencies
  column lookup APIs
  qualified/unqualified resolution rules
  logical-planning schema contract
```

Use `DFSchema` for:

```text id="zbo7g1"
SQL binding
DataFrame schema inspection
LogicalPlan output schema
Expr type/nullability inference
join column ambiguity detection
optimizer rule validation
qualified column lookup
logical plan schema diffs
```

The attached documentation’s `DFSchema` chapter shows the same split: Arrow `Schema` defines physical data fields, whereas `DFSchema` carries optional qualifiers, functional dependencies, and logical column-resolution APIs used for logical planning, expression inference, join schemas, SQL binding, and ambiguity detection. 

---

## 44.3 Core imports

```rust id="dcs7sf"
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, FieldRef, Schema, SchemaRef};

use datafusion::common::{
    Column,
    DFSchema,
    DFSchemaRef,
    DataFusionError,
    Result,
    TableReference,
};

use datafusion::logical_expr::{
    col,
    lit,
    Expr,
    ExprSchemable,
    LogicalPlan,
    LogicalPlanBuilder,
};

use datafusion::prelude::*;
```

Agent import rule:

```text id="zt76tc"
Use datafusion::arrow re-exports for DataFusion-facing Arrow types.
Use DFSchema for logical identity.
Use Arrow Schema for physical output contracts.
```

---

## 44.4 Relation qualifiers

### Qualifier shapes

```text id="scblfh"
unqualified:
  id

table-qualified:
  orders.id

schema.table-qualified:
  analytics.orders.id

catalog.schema.table-qualified:
  prod.analytics.orders.id

alias-qualified:
  o.id
```

In `DFSchema`, each field may have an optional `TableReference` qualifier. The attached doc lists relevant APIs such as `qualified_field`, `field_with_name`, `qualified_field_with_name`, `fields_with_qualified`, `qualified_fields_with_unqualified_name`, `columns_with_unqualified_name`, and `columns`, and explains these APIs support exact lookup, ambiguous-name detection, and join/schema construction. 

### Create unqualified `DFSchema`

```rust id="6cocjm"
pub fn unqualified_dfschema() -> Result<DFSchema> {
    let arrow_schema = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("amount", DataType::Float64, true),
    ]);

    DFSchema::try_from(arrow_schema)
}
```

### Create qualified `DFSchema`

```rust id="vcfvud"
pub fn qualified_dfschema() -> Result<DFSchema> {
    let arrow_schema = Schema::new(vec![
        Field::new("id", DataType::Int64, false),
        Field::new("amount", DataType::Float64, true),
    ]);

    DFSchema::try_from_qualified_schema("orders", &arrow_schema)
}
```

### Inspect qualifiers

```rust id="a977if"
pub fn inspect_qualifiers(schema: &DFSchema) {
    for (qualifier, field) in schema.iter() {
        match qualifier {
            Some(q) => {
                println!("{q}.{}: {:?}, nullable={}",
                    field.name(),
                    field.data_type(),
                    field.is_nullable(),
                );
            }
            None => {
                println!("{}: {:?}, nullable={}",
                    field.name(),
                    field.data_type(),
                    field.is_nullable(),
                );
            }
        }
    }
}
```

### Lookup qualified column

```rust id="10d92w"
pub fn lookup_qualified(schema: &DFSchema, name: &str) -> Result<()> {
    let col = Column::from_qualified_name(name);
    let idx = schema.index_of_column(&col)?;
    let (qualifier, field) = schema.qualified_field(idx);

    println!("matched qualifier={qualifier:?}, field={}", field.name());
    Ok(())
}
```

---

## 44.5 Column identity model

Column identity has multiple layers. Agents must not collapse them into a single string.

```text id="xqhvg5"
Physical field identity:
  Arrow Field:
    name
    data_type
    nullable
    metadata

Logical field identity:
  DFSchema field:
    optional qualifier
    field.name
    field.data_type
    field.nullable
    field.metadata
    functional dependency context

Reference identity:
  Column:
    relation qualifier, optional
    field name

Output identity:
  projection alias or expression display name
  final Arrow field name
  public contract name
```

### Identity sources

| Source                  | Example                                   | Identity consequence                            |
| ----------------------- | ----------------------------------------- | ----------------------------------------------- |
| catalog/schema/table    | `prod.analytics.orders.id`                | full relation path before aliasing              |
| table alias             | `orders AS o`                             | visible qualifier becomes `o`                   |
| CTE alias               | `WITH q AS (...)`                         | output fields qualified by CTE/subquery context |
| subquery alias          | `(SELECT ...) AS q`                       | outer scope sees `q.field`                      |
| projection alias        | `amount * 1.08 AS gross_amount`           | final field name becomes `gross_amount`         |
| aggregate alias         | `SUM(amount) AS total_amount`             | final field name stable                         |
| expression-derived name | `SUM(amount)` or `amount * Float64(1.08)` | unstable / not API-safe                         |
| cast expression         | `CAST(x AS DOUBLE)`                       | display-derived name unless aliased             |
| function call           | `lower(email)`                            | display-derived name unless aliased             |

DataFusion’s `Expr` type represents logical computations such as columns, literals, casts, scalar functions, aggregate functions, and window functions. ([Docs.rs][4]) DataFrame expression-string projection returns one output column for each expression, so unaliased expression names directly become output schema names and should be treated as unstable unless explicitly governed. ([Docs.rs][5])

---

## 44.6 Join output identity

### Hazard: unqualified duplicate names

```sql id="0ez6u1"
SELECT *
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.id;
```

Potential output identity:

```text id="9sl0xl"
o.id
o.customer_id
o.amount
c.id
c.name
```

Public output may contain duplicate unqualified names:

```text id="4kqt2v"
id
customer_id
amount
id
name
```

### Safe SQL projection

```sql id="s0rffb"
SELECT
  o.id AS order_id,
  o.customer_id AS customer_id,
  o.amount AS order_amount,
  c.id AS customer_id_joined,
  c.name AS customer_name
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.id;
```

### Safe DataFrame projection after join

```rust id="bqw3so"
pub fn normalize_join_output(df: DataFrame) -> Result<DataFrame> {
    df.select(vec![
        col("orders.id").alias("order_id"),
        col("orders.customer_id").alias("customer_id"),
        col("orders.amount").alias("order_amount"),
        col("customers.id").alias("customer_id_joined"),
        col("customers.name").alias("customer_name"),
    ])
}
```

### Ambiguity-safe lookup

```rust id="w2ingg"
pub enum ColumnResolution<'a> {
    Unique(Column),
    Missing(&'a str),
    Ambiguous { name: &'a str, matches: Vec<Column> },
}

pub fn resolve_unqualified<'a>(
    schema: &DFSchema,
    name: &'a str,
) -> ColumnResolution<'a> {
    let matches = schema.columns_with_unqualified_name(name);

    match matches.len() {
        0 => ColumnResolution::Missing(name),
        1 => ColumnResolution::Unique(matches[0].clone()),
        _ => ColumnResolution::Ambiguous {
            name,
            matches: matches.into_iter().cloned().collect(),
        },
    }
}
```

Agent join policy:

```text id="5gnpvd"
Before join:
  unqualified references acceptable only when unique.

During join:
  preserve relation qualifiers.

After join:
  qualified references only.
  explicit final projection required.
  duplicate public names rejected.
  SELECT * forbidden in public API outputs.
```

The attached doc explicitly states that query-generation agents should generate qualified columns after joins; if two relations contain `id`, unqualified `col("id")` is ambiguous, and agents should validate via `DFSchema::columns_with_unqualified_name` or use relation-qualified columns. 

---

## 44.7 Output schema contract classes

| Contract class       | Consumer                        | Required stability            | Recommended schema policy                                                             |
| -------------------- | ------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------- |
| public API output    | HTTP/gRPC/Flight/SDK users      | strongest                     | explicit projection, stable aliases, no duplicate names, documented types/nullability |
| internal plan output | optimizer / next plan stage     | medium                        | preserve qualifiers and expression identity; aliases where helpful                    |
| sink/write output    | Parquet/CSV/JSON/table provider | strong physical compatibility | Arrow `Schema` exactness, writer options, metadata policy                             |
| view output          | SQL users / downstream queries  | strong                        | explicit view projection, no `SELECT *`, stable aliases                               |
| test output          | golden snapshots / CI           | strong but version-pinned     | snapshot schema contract; logical plan snapshots version-pinned                       |
| diagnostic output    | EXPLAIN / logs                  | weak                          | may include expression/display strings; not API contract                              |

### Public API output contract

```rust id="k8ph6j"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicFieldContract {
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicSchemaContract {
    pub fields: Vec<PublicFieldContract>,
    pub version: String,
}
```

### Internal logical output contract

```rust id="e1fcat"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalFieldIdentity {
    pub qualifier: Option<String>,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
}
```

### Sink/write output contract

```rust id="aczomu"
#[derive(Debug, Clone)]
pub struct SinkSchemaContract {
    pub arrow_schema: SchemaRef,
    pub allow_extra_fields: bool,
    pub allow_missing_nullable_fields: bool,
    pub metadata_policy: MetadataPolicy,
}

#[derive(Debug, Clone)]
pub enum MetadataPolicy {
    Ignore,
    PreserveExact,
    PreserveSelectedKeys(BTreeSet<String>),
}
```

---

## 44.8 Naming hazards

### Aggregate names

Bad:

```rust id="o0xddf"
let df = df.aggregate(
    vec![col("region")],
    vec![sum(col("amount"))],
)?;
```

Likely output name becomes expression-derived, for example:

```text id="odphm8"
SUM(?table?.amount)
sum(amount)
SUM(amount)
```

Safe:

```rust id="wt6x2z"
let df = df.aggregate(
    vec![col("region")],
    vec![sum(col("amount")).alias("total_amount")],
)?;
```

### Function names

Bad:

```rust id="mbzrq1"
let df = df.select(vec![
    lower(col("email")),
])?;
```

Safe:

```rust id="sqxviq"
let df = df.select(vec![
    lower(col("email")).alias("email_normalized"),
])?;
```

### Cast names

Bad:

```rust id="a1mdsk"
let df = df.select_exprs(&[
    "CAST(amount AS DOUBLE)",
])?;
```

Safe:

```rust id="8fhbjw"
let df = df.select_exprs(&[
    "CAST(amount AS DOUBLE) AS amount_f64",
])?;
```

### Expression display strings

Never treat these as API names:

```text id="mixlek"
?table?.amount * Float64(1.08)
CAST(?table?.x AS Float64)
lower(?table?.email)
SUM(?table?.amount)
```

The attached doc’s best-practice inventory already includes “Preserve expression names in optimizer rewrites” and “Add aliases for computed columns,” and warns against snapshotting schemas by string display alone. 

---

## 44.9 Alias policy

### Required aliases

```text id="2xxyvg"
Required:
  arithmetic expressions
  casts
  scalar function calls
  UDF calls
  aggregate expressions
  window expressions
  CASE expressions
  subquery computed outputs
  join duplicate columns
  public API fields after quote/case normalization
```

### Optional aliases

```text id="8ym7q5"
Optional:
  direct unique source columns in internal plans
  already-stable public field names
  final projection passthrough columns
```

### Alias helper

```rust id="6ycciv"
pub fn must_alias(expr: Expr, alias: impl AsRef<str>) -> Result<Expr> {
    let alias = alias.as_ref();

    validate_public_field_name(alias)?;

    Ok(expr.alias(alias))
}

pub fn validate_public_field_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(DataFusionError::Plan("empty output alias".to_string()));
    }

    let valid = name
        .chars()
        .enumerate()
        .all(|(i, c)| {
            if i == 0 {
                c.is_ascii_lowercase() || c == '_'
            } else {
                c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'
            }
        });

    if !valid {
        return Err(DataFusionError::Plan(format!(
            "invalid public output alias: {name}; expected lowercase snake_case"
        )));
    }

    Ok(())
}
```

### Projection builder with alias enforcement

```rust id="hn181f"
#[derive(Debug, Clone)]
pub enum ProjectionSpec {
    Passthrough {
        input: String,
        alias: Option<String>,
    },
    Computed {
        expr: Expr,
        alias: String,
    },
}

pub fn compile_projection(specs: &[ProjectionSpec]) -> Result<Vec<Expr>> {
    specs
        .iter()
        .map(|spec| match spec {
            ProjectionSpec::Passthrough { input, alias } => {
                let expr = col(input.as_str());
                match alias {
                    Some(a) => must_alias(expr, a),
                    None => Ok(expr),
                }
            }
            ProjectionSpec::Computed { expr, alias } => {
                must_alias(expr.clone(), alias)
            }
        })
        .collect()
}
```

---

## 44.10 Normalize public names late

### Internal plan

```text id="ph89u8"
internal:
  orders.id
  orders.customer_id
  customers.id
  customers.name
  amount * 1.08 AS gross_amount
```

### Final public projection

```text id="bglqsh"
public:
  order_id
  customer_id
  customer_name
  gross_amount
```

### Pattern

```rust id="fp6z6d"
pub fn final_public_projection(df: DataFrame) -> Result<DataFrame> {
    df.select(vec![
        col("orders.id").alias("order_id"),
        col("orders.customer_id").alias("customer_id"),
        col("customers.name").alias("customer_name"),
        col("gross_amount"),
    ])
}
```

Agent rule:

```text id="tonw99"
Preserve qualifiers as long as plan composition requires disambiguation.
Strip/normalize only at final public projection boundary.
```

---

## 44.11 `DFSchema` to Arrow `Schema` boundary

### Conversion loses qualifiers

```rust id="b3zqb4"
pub fn logical_to_physical_schema(schema: &DFSchema) -> SchemaRef {
    schema.inner()
}
```

Boundary meaning:

```text id="7ppwq2"
DFSchema:
  Some("orders").id
  Some("customers").id

Arrow Schema:
  id
  id
```

The attached docs explicitly state that `DFSchema::as_arrow` and `inner` return the inner Arrow schema but do not include qualifier information; qualifier metadata is not preserved when converting back to Arrow schema. 

Agent boundary rule:

```text id="yxn391"
Before converting DFSchema to Arrow Schema:
  verify public field names are stable
  reject duplicates
  ensure qualifiers are no longer needed
  document metadata preservation policy
```

---

## 44.12 Output schema extraction

### From DataFrame

```rust id="t91kzy"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldIdentity {
    pub qualifier: Option<String>,
    pub name: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub metadata: BTreeMap<String, String>,
}

pub fn field_identities(schema: &DFSchema) -> Vec<FieldIdentity> {
    schema
        .iter()
        .map(|(qualifier, field)| FieldIdentity {
            qualifier: qualifier.map(|q| q.to_string()),
            name: field.name().clone(),
            data_type: field.data_type().clone(),
            nullable: field.is_nullable(),
            metadata: field
                .metadata()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        })
        .collect()
}

pub fn dataframe_field_identities(df: &DataFrame) -> Vec<FieldIdentity> {
    field_identities(df.schema())
}
```

### From LogicalPlan

```rust id="o7fke5"
pub fn logical_plan_field_identities(plan: &LogicalPlan) -> Vec<FieldIdentity> {
    field_identities(plan.schema())
}
```

### From ExecutionPlan

```rust id="o59fzv"
use datafusion::physical_plan::ExecutionPlan;

pub fn physical_field_identities(plan: &dyn ExecutionPlan) -> Vec<PublicFieldContract> {
    plan.schema()
        .fields()
        .iter()
        .map(|field| PublicFieldContract {
            name: field.name().clone(),
            data_type: field.data_type().clone(),
            nullable: field.is_nullable(),
            metadata: field
                .metadata()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        })
        .collect()
}
```

---

## 44.13 Duplicate-name detection

```rust id="ulmt2t"
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateName {
    pub name: String,
    pub positions: Vec<usize>,
}

pub fn find_duplicate_field_names(fields: &[FieldIdentity]) -> Vec<DuplicateName> {
    let mut positions: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    for (idx, field) in fields.iter().enumerate() {
        positions.entry(field.name.clone()).or_default().push(idx);
    }

    positions
        .into_iter()
        .filter_map(|(name, positions)| {
            if positions.len() > 1 {
                Some(DuplicateName { name, positions })
            } else {
                None
            }
        })
        .collect()
}

pub fn reject_duplicate_public_names(fields: &[FieldIdentity]) -> Result<()> {
    let duplicates = find_duplicate_field_names(fields);

    if duplicates.is_empty() {
        return Ok(());
    }

    Err(DataFusionError::Plan(format!(
        "duplicate output field names: {duplicates:?}"
    )))
}
```

Use this:

```text id="zed2pb"
after join
after projection
before public API response
before write_table / file sink
before view creation
before golden snapshot
```

---

## 44.14 Schema diffing

### Diff dimensions

```text id="1gb5yk"
field-level:
  position
  qualifier
  name
  data_type
  nullable
  metadata

schema-level:
  field count
  field order
  duplicate names
  functional dependencies
  schema metadata
```

### Diff model

```rust id="4ncc2b"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaDiffKind {
    FieldAdded,
    FieldRemoved,
    FieldRenamed,
    FieldReordered,
    TypeChanged,
    NullabilityChanged,
    MetadataChanged,
    QualifierChanged,
    DuplicateNameIntroduced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDiff {
    pub kind: SchemaDiffKind,
    pub path: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
```

### Diff function

```rust id="2poh4d"
pub fn diff_field_identities(
    before: &[FieldIdentity],
    after: &[FieldIdentity],
) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();

    let max_len = before.len().max(after.len());

    for idx in 0..max_len {
        match (before.get(idx), after.get(idx)) {
            (Some(b), Some(a)) => {
                let path = format!("field[{idx}]");

                if b.name != a.name {
                    diffs.push(SchemaDiff {
                        kind: SchemaDiffKind::FieldRenamed,
                        path: path.clone(),
                        before: Some(b.name.clone()),
                        after: Some(a.name.clone()),
                    });
                }

                if b.qualifier != a.qualifier {
                    diffs.push(SchemaDiff {
                        kind: SchemaDiffKind::QualifierChanged,
                        path: format!("{path}.qualifier"),
                        before: b.qualifier.clone(),
                        after: a.qualifier.clone(),
                    });
                }

                if b.data_type != a.data_type {
                    diffs.push(SchemaDiff {
                        kind: SchemaDiffKind::TypeChanged,
                        path: format!("{path}.data_type"),
                        before: Some(format!("{:?}", b.data_type)),
                        after: Some(format!("{:?}", a.data_type)),
                    });
                }

                if b.nullable != a.nullable {
                    diffs.push(SchemaDiff {
                        kind: SchemaDiffKind::NullabilityChanged,
                        path: format!("{path}.nullable"),
                        before: Some(b.nullable.to_string()),
                        after: Some(a.nullable.to_string()),
                    });
                }

                if b.metadata != a.metadata {
                    diffs.push(SchemaDiff {
                        kind: SchemaDiffKind::MetadataChanged,
                        path: format!("{path}.metadata"),
                        before: Some(format!("{:?}", b.metadata)),
                        after: Some(format!("{:?}", a.metadata)),
                    });
                }
            }

            (Some(b), None) => {
                diffs.push(SchemaDiff {
                    kind: SchemaDiffKind::FieldRemoved,
                    path: format!("field[{idx}]"),
                    before: Some(format!("{:?}", b)),
                    after: None,
                });
            }

            (None, Some(a)) => {
                diffs.push(SchemaDiff {
                    kind: SchemaDiffKind::FieldAdded,
                    path: format!("field[{idx}]"),
                    before: None,
                    after: Some(format!("{:?}", a)),
                });
            }

            (None, None) => {}
        }
    }

    for dup in find_duplicate_field_names(after) {
        diffs.push(SchemaDiff {
            kind: SchemaDiffKind::DuplicateNameIntroduced,
            path: dup.name.clone(),
            before: None,
            after: Some(format!("{:?}", dup.positions)),
        });
    }

    diffs
}
```

### Compatibility policy

```rust id="w0c42j"
#[derive(Debug, Clone)]
pub struct SchemaCompatibilityPolicy {
    pub allow_field_addition: bool,
    pub allow_field_removal: bool,
    pub allow_reorder: bool,
    pub allow_nullability_widening: bool,
    pub allow_metadata_changes: bool,
    pub require_qualifier_preservation: bool,
}

pub fn validate_schema_diff(
    diffs: &[SchemaDiff],
    policy: &SchemaCompatibilityPolicy,
) -> Result<()> {
    for diff in diffs {
        let allowed = match diff.kind {
            SchemaDiffKind::FieldAdded => policy.allow_field_addition,
            SchemaDiffKind::FieldRemoved => policy.allow_field_removal,
            SchemaDiffKind::FieldReordered => policy.allow_reorder,
            SchemaDiffKind::NullabilityChanged => policy.allow_nullability_widening,
            SchemaDiffKind::MetadataChanged => policy.allow_metadata_changes,
            SchemaDiffKind::QualifierChanged => !policy.require_qualifier_preservation,
            SchemaDiffKind::FieldRenamed
            | SchemaDiffKind::TypeChanged
            | SchemaDiffKind::DuplicateNameIntroduced => false,
        };

        if !allowed {
            return Err(DataFusionError::Plan(format!(
                "schema compatibility violation: {diff:?}"
            )));
        }
    }

    Ok(())
}
```

---

## 44.15 Alias preservation during rewrites

### Rule

```text id="sfehhf"
Optimizer rule / expression rewrite must preserve externally visible names unless the rule explicitly changes projection semantics.
```

### Bad rewrite

```rust id="01zq8r"
// Input expression:
//   amount * 1.08 AS gross_amount
//
// Bad rewrite result:
//   amount * 1.08
//
// Consequence:
//   output name falls back to expression display string
```

### Rewrite invariant

```text id="z5hgox"
For every projection expression:
  before display_name == after display_name
  or explicit schema-changing rule recorded and tested.
```

### Wrapper utility shape

```rust id="sr1tzg"
pub fn preserve_alias_if_present(before: &Expr, rewritten: Expr) -> Expr {
    match before {
        Expr::Alias(alias) => {
            // Exact Alias internals vary by DataFusion version.
            // Under pinned version, reconstruct alias around rewritten child.
            // Pseudocode:
            // rewritten.alias(alias.name())
            rewritten
        }
        _ => rewritten,
    }
}
```

DataFusion’s optimizer guide specifically warns optimizer authors that expression names must be preserved; otherwise expected columns may not be found. ([Apache DataFusion][2])

### Test invariant

```rust id="nnu6es"
pub fn assert_schema_preserved(before: &DFSchema, after: &DFSchema) -> Result<()> {
    let before_fields = field_identities(before);
    let after_fields = field_identities(after);
    let diffs = diff_field_identities(&before_fields, &after_fields);

    if !diffs.is_empty() {
        return Err(DataFusionError::Plan(format!(
            "schema changed unexpectedly: {diffs:?}"
        )));
    }

    Ok(())
}
```

---

## 44.16 Expression name discipline

### Expression naming categories

| Expression                | Name source            |     API-safe? | Required action              |
| ------------------------- | ---------------------- | ------------: | ---------------------------- |
| `col("amount")`           | field name             | yes if unique | optional alias               |
| `col("orders.amount")`    | field name + qualifier |  internal yes | final public alias if needed |
| `col("a") + col("b")`     | display string         |            no | alias                        |
| `sum(col("amount"))`      | aggregate display      |            no | alias                        |
| `lower(col("email"))`     | function display       |            no | alias                        |
| `cast(col("x"))`          | cast display           |            no | alias                        |
| `case(...).when(...)`     | case display           |            no | alias                        |
| `row_number() OVER (...)` | window display         |            no | alias                        |

### Enforce computed-expression aliases

```rust id="y7dw4o"
pub fn is_direct_column(expr: &Expr) -> bool {
    matches!(expr, Expr::Column(_))
}

pub fn validate_projection_aliases(exprs: &[Expr]) -> Result<()> {
    for expr in exprs {
        if is_direct_column(expr) {
            continue;
        }

        if !matches!(expr, Expr::Alias(_)) {
            return Err(DataFusionError::Plan(format!(
                "computed projection expression must be aliased: {expr:?}"
            )));
        }
    }

    Ok(())
}
```

Exact `Expr::Alias` shape is version-specific; match against the pinned crate API. The `Expr` docs define `Alias`, `Cast`, `ScalarFunction`, `AggregateFunction`, `WindowFunction`, and many other expression variants, and examples show `Expr::alias` as the standard aliasing method. ([Docs.rs][4])

---

## 44.17 View output governance

### Bad view

```sql id="9v4xvx"
CREATE VIEW active_orders AS
SELECT *
FROM orders
WHERE status = 'active';
```

Hazards:

```text id="ubtv32"
source schema addition leaks into view
source schema reorder changes output order
duplicate names if view later joins
computed names if source view changes
public contract undocumented
```

### Governed view

```sql id="h805zt"
CREATE VIEW active_orders AS
SELECT
  id AS order_id,
  customer_id,
  amount AS order_amount,
  created_at AS order_created_at
FROM orders
WHERE status = 'active';
```

### View policy

```text id="lofzqx"
Views are public schemas.
Views must use explicit projection.
Views must alias computed expressions.
Views should normalize field names.
Views should avoid SELECT * unless purely internal and versioned.
View output schema must be tested with DESCRIBE / DataFrame schema.
```

---

## 44.18 Sink/write schema governance

### Sink boundary requirements

```text id="uawagy"
Before writing:
  output field count known
  output names stable
  output types compatible with target format/provider
  output nullability compatible with target table/sink
  metadata policy selected
  duplicate names rejected
  partition columns handled explicitly
```

### Validate DataFrame against expected Arrow schema

```rust id="mt2d90"
pub fn validate_against_expected_arrow_schema(
    df: &DataFrame,
    expected: &Schema,
) -> Result<()> {
    let actual = df.schema().as_arrow();

    if actual.fields().len() != expected.fields().len() {
        return Err(DataFusionError::Plan(format!(
            "field count mismatch: actual={}, expected={}",
            actual.fields().len(),
            expected.fields().len(),
        )));
    }

    for (idx, (a, e)) in actual.fields().iter().zip(expected.fields()).enumerate() {
        if a.name() != e.name() {
            return Err(DataFusionError::Plan(format!(
                "field[{idx}] name mismatch: actual={}, expected={}",
                a.name(),
                e.name(),
            )));
        }

        if a.data_type() != e.data_type() {
            return Err(DataFusionError::Plan(format!(
                "field[{idx}] type mismatch: actual={:?}, expected={:?}",
                a.data_type(),
                e.data_type(),
            )));
        }

        if a.is_nullable() != e.is_nullable() {
            return Err(DataFusionError::Plan(format!(
                "field[{idx}] nullability mismatch: actual={}, expected={}",
                a.is_nullable(),
                e.is_nullable(),
            )));
        }
    }

    Ok(())
}
```

### Write path

```rust id="382a58"
use datafusion::dataframe::DataFrameWriteOptions;

pub async fn write_governed_parquet(
    df: DataFrame,
    expected: &Schema,
    output_path: &str,
) -> Result<Vec<RecordBatch>> {
    validate_against_expected_arrow_schema(&df, expected)?;
    validate_output_schema(&df)?;

    df.write_parquet(
        output_path,
        DataFrameWriteOptions::new(),
        None,
    )
    .await
}

pub fn validate_output_schema(df: &DataFrame) -> Result<()> {
    let fields = dataframe_field_identities(df);
    reject_duplicate_public_names(&fields)
}
```

---

## 44.19 Public API schema manifest

```rust id="fzv9m2"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiSchemaManifest {
    pub schema_id: String,
    pub schema_version: String,
    pub fields: Vec<ApiFieldManifest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiFieldManifest {
    pub name: String,
    pub arrow_type: String,
    pub nullable: bool,
    pub semantic_type: Option<String>,
    pub description: Option<String>,
}
```

Generate from `DFSchema`:

```rust id="hqnzvj"
pub fn manifest_from_df(
    schema_id: impl Into<String>,
    schema_version: impl Into<String>,
    df: &DataFrame,
) -> ApiSchemaManifest {
    ApiSchemaManifest {
        schema_id: schema_id.into(),
        schema_version: schema_version.into(),
        fields: dataframe_field_identities(df)
            .into_iter()
            .map(|f| ApiFieldManifest {
                name: f.name,
                arrow_type: format!("{:?}", f.data_type),
                nullable: f.nullable,
                semantic_type: f.metadata.get("semantic_type").cloned(),
                description: f.metadata.get("description").cloned(),
            })
            .collect(),
    }
}
```

API policy:

```text id="ogx7cq"
Public API schema:
  versioned
  explicit
  duplicate-free
  deterministic order
  stable aliases
  nullable contract documented
  metadata policy documented
```

---

## 44.20 Case sensitivity and identifier normalization

DataFusion SQL docs warn that query column names are made lower-case, while inferred schemas are not; capitalized physical fields require double quotes. ([Apache DataFusion][6])

### Hazard

```sql id="vvb57s"
-- Physical CSV header: "CustomerID"
SELECT CustomerID FROM customers;
```

May fail or resolve incorrectly because unquoted SQL names are normalized.

### Safe

```sql id="1c9hv5"
SELECT
  "CustomerID" AS customer_id
FROM customers;
```

### Programmatic naming policy

```text id="fggfad"
Physical source names:
  preserve as source contract.

Internal logical plan:
  use qualifiers to disambiguate.

Public output:
  normalize to lowercase snake_case aliases.

Generated SQL:
  quote source identifiers when case-sensitive.
  alias normalized output names immediately.
```

---

## 44.21 Schema governance in plan factories

### Source validation

```rust id="ubk6da"
pub fn require_unique_column(schema: &DFSchema, name: &str) -> Result<Column> {
    match resolve_unqualified(schema, name) {
        ColumnResolution::Unique(c) => Ok(c),
        ColumnResolution::Missing(_) => Err(DataFusionError::Plan(format!(
            "unknown column: {name}"
        ))),
        ColumnResolution::Ambiguous { matches, .. } => Err(DataFusionError::Plan(format!(
            "ambiguous column {name}; candidates: {matches:?}"
        ))),
    }
}
```

### Projection validation

```rust id="76rhlj"
pub fn compile_governed_projection(
    input_schema: &DFSchema,
    specs: &[ProjectionSpec],
) -> Result<Vec<Expr>> {
    let exprs = compile_projection(specs)?;

    validate_projection_aliases(&exprs)?;

    for expr in &exprs {
        let (_qualifier, field) = expr.to_field(input_schema)?;
        validate_public_field_name(field.name())?;
    }

    let preview = exprs
        .iter()
        .map(|expr| {
            let (_q, field) = expr.to_field(input_schema)?;
            Ok(FieldIdentity {
                qualifier: None,
                name: field.name().clone(),
                data_type: field.data_type().clone(),
                nullable: field.is_nullable(),
                metadata: field
                    .metadata()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    reject_duplicate_public_names(&preview)?;

    Ok(exprs)
}
```

### Join validation

```rust id="0hvn7e"
pub fn validate_join_inputs(
    left: &DFSchema,
    right: &DFSchema,
    left_keys: &[String],
    right_keys: &[String],
) -> Result<()> {
    if left_keys.len() != right_keys.len() {
        return Err(DataFusionError::Plan(format!(
            "join key count mismatch: left={}, right={}",
            left_keys.len(),
            right_keys.len(),
        )));
    }

    for key in left_keys {
        require_unique_column(left, key)?;
    }

    for key in right_keys {
        require_unique_column(right, key)?;
    }

    Ok(())
}
```

---

## 44.22 Optimizer rule schema invariants

For custom analyzer/optimizer rules:

```text id="yur75j"
If rule claims semantic-preserving rewrite:
  output field count must match
  output field names must match
  output types must match or remain logically equivalent
  output nullability must match unless rule explicitly tightens/widens with proof
  qualifiers must match unless moving to final output context
  aliases must be preserved
  metadata must follow rule policy
```

### Rule test skeleton

```rust id="4idw21"
pub fn assert_rewrite_preserves_schema(
    before: &LogicalPlan,
    after: &LogicalPlan,
) -> Result<()> {
    let before_schema = before.schema();
    let after_schema = after.schema();

    let before_fields = field_identities(before_schema);
    let after_fields = field_identities(after_schema);

    let diffs = diff_field_identities(&before_fields, &after_fields);

    if !diffs.is_empty() {
        return Err(DataFusionError::Plan(format!(
            "optimizer rewrite changed schema unexpectedly: {diffs:?}"
        )));
    }

    Ok(())
}
```

### Rewrite logging payload

```rust id="5m3f39"
#[derive(Debug, Clone)]
pub struct RewriteAudit {
    pub rule_name: String,
    pub before_schema: Vec<FieldIdentity>,
    pub after_schema: Vec<FieldIdentity>,
    pub schema_diffs: Vec<SchemaDiff>,
    pub allowed_schema_change: bool,
}
```

---

## 44.23 Schema snapshots for golden tests

### Snapshot schema, not only data

```rust id="2g5b2h"
#[tokio::test]
async fn generated_schema_is_stable() -> Result<()> {
    let ctx = test_context().await?;
    let df = build_generated_query(&ctx).await?;

    let identities = dataframe_field_identities(&df);

    insta::assert_debug_snapshot!(identities);

    Ok(())
}
```

### Snapshot logical output schema

```rust id="epx4r1"
#[tokio::test]
async fn optimized_plan_schema_is_stable() -> Result<()> {
    let ctx = test_context().await?;
    let df = build_generated_query(&ctx).await?;

    let optimized = df.clone().into_optimized_plan()?;
    let identities = logical_plan_field_identities(&optimized);

    insta::assert_debug_snapshot!(identities);

    Ok(())
}
```

Test policy:

```text id="w7lxr2"
Always test:
  output field order
  output field names
  output field types
  output nullability
  duplicate-name rejection
  join ambiguity rejection
  alias preservation in computed outputs

Version-pin:
  logical plan string snapshots
  physical plan string snapshots
  expression display snapshots
```

---

## 44.24 End-to-end governed projection example

```rust id="i8apwd"
pub async fn governed_customer_orders(ctx: &SessionContext) -> Result<DataFrame> {
    let orders = ctx.table("orders").await?;
    let customers = ctx.table("customers").await?;

    validate_join_inputs(
        orders.schema(),
        customers.schema(),
        &["customer_id".to_string()],
        &["id".to_string()],
    )?;

    let joined = orders.join(
        customers,
        JoinType::Inner,
        &["customer_id"],
        &["id"],
        None,
    )?;

    // Preserve qualifiers through join; normalize only in final projection.
    let projection = vec![
        ProjectionSpec::Passthrough {
            input: "orders.id".to_string(),
            alias: Some("order_id".to_string()),
        },
        ProjectionSpec::Passthrough {
            input: "orders.customer_id".to_string(),
            alias: Some("customer_id".to_string()),
        },
        ProjectionSpec::Passthrough {
            input: "customers.name".to_string(),
            alias: Some("customer_name".to_string()),
        },
        ProjectionSpec::Computed {
            expr: col("orders.amount") * lit(1.08_f64),
            alias: "gross_amount".to_string(),
        },
    ];

    let exprs = compile_governed_projection(joined.schema(), &projection)?;

    let output = joined.select(exprs)?;

    validate_output_schema(&output)?;

    Ok(output)
}
```

---

## 44.25 Deployment advisory

```text id="0f8a8v"
SQL service:
  reject SELECT * for public saved queries/views.
  normalize quoted/case-sensitive source names into aliases.
  lint unqualified columns after joins.
  snapshot view/public API schemas.

Programmatic planning:
  validate dynamic column names with DFSchema before col(...).
  preserve qualifiers until final projection.
  alias computed expressions at construction time.
  reject duplicate output names before execution.

Custom optimizer:
  test schema-preservation invariants.
  preserve aliases and expression names.
  do not strip qualifiers unless rule owns final projection semantics.

Custom TableProvider:
  return stable Arrow Schema.
  match RecordBatch output exactly to schema.
  document metadata policy.
  expose logical names that are stable under projection/pushdown.

Sinks:
  validate Arrow Schema before write.
  handle partition columns explicitly.
  record output schema manifest with files/table versions.
```

---

## 44.26 Anti-pattern inventory

* Treating Arrow `Schema` as if it stores catalog/schema/table qualifiers.
* Converting `DFSchema` to Arrow `Schema` before resolving join ambiguity.
* Using `SELECT *` in public view/API definitions.
* Using unqualified `id` after a join.
* Assuming expression display strings are stable.
* Letting aggregate output names default to `SUM(...)`.
* Letting function output names default to `lower(...)`.
* Letting cast output names default to `CAST(...)`.
* Stripping aliases during optimizer rewrites.
* Snapshotting pretty table output without schema snapshots.
* Allowing duplicate public field names.
* Inferring output schema from first batch instead of plan schema.
* Treating metadata changes as irrelevant in systems that use extension types.
* Normalizing field names too early and losing disambiguating qualifiers.
* Writing to sinks without checking nullability/type compatibility.
* Depending on physical `EXPLAIN` text to define column identity.

---

## 44.27 Agent checklist

```text id="3vzq9a"
[ ] Distinguish Arrow Schema vs DFSchema.
[ ] Use DFSchema for logical planning, column resolution, expression inference.
[ ] Use Arrow Schema for RecordBatch/TableProvider/ExecutionPlan/sink contracts.
[ ] Preserve qualifiers through joins and intermediate plans.
[ ] Resolve unqualified columns only if exactly one match exists.
[ ] Qualify all generated references after joins.
[ ] Alias every computed expression.
[ ] Alias every aggregate expression.
[ ] Alias every window expression.
[ ] Alias every cast/function/UDF expression exposed publicly.
[ ] Reject duplicate output names before public API/sink/view boundary.
[ ] Normalize public output names late.
[ ] Never use expression display strings as API contracts.
[ ] Snapshot output schema contracts in tests.
[ ] Diff schema by name, type, nullability, metadata, qualifier, and order.
[ ] Preserve aliases and expression names in optimizer rewrites.
[ ] Treat qualifier loss during DFSchema → Arrow Schema as a boundary event.
[ ] Version public schemas and view outputs.
```

[1]: https://docs.rs/datafusion/latest/datafusion/common/struct.DFSchema.html?utm_source=chatgpt.com "DFSchema in datafusion::common - Rust"
[2]: https://datafusion.apache.org/library-user-guide/query-optimizer.html?utm_source=chatgpt.com "Query Optimizer — Apache DataFusion documentation"
[3]: https://docs.rs/datafusion/latest/datafusion/common/arrow/datatypes/struct.Field.html?utm_source=chatgpt.com "Field in datafusion::common::arrow::datatypes - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html?utm_source=chatgpt.com "Expr in datafusion::logical_expr - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"
[6]: https://datafusion.apache.org/user-guide/sql/select.html?utm_source=chatgpt.com "SELECT syntax — Apache DataFusion documentation"


# DataFusion Advanced — 45) Expression lifecycle: unresolved SQL expression → bound `Expr` → physical expression

## 45.0 Purpose

Connect expression syntax, SQL binding, schema-aware type inference, analyzer coercion, optimizer rewrites, and physical Arrow evaluation:

```text id="flj6np"
SQL text expression / DataFrame Expr / LogicalPlanBuilder Expr / optimizer-created Expr
  → unresolved syntax or programmatic Expr
  → binding / resolution against DFSchema + function registries
  → logical Expr
  → analyzer coercion + semantic validation
  → optimized Expr inside optimized LogicalPlan
  → physical planning
  → PhysicalExpr
  → evaluate(RecordBatch)
  → ColumnarValue::Array(ArrayRef) | ColumnarValue::Scalar(ScalarValue)
```

The attached expression section already frames `Expr` as the shared language between DataFrame APIs, SQL planning, optimizer rewrites, physical planning, and UDF call sites, including the lifecycle `SQL AST expression → Expr → PhysicalExpr → ColumnarValue`.  DataFusion’s `Expr` docs also identify `ExprSchemable::get_type` for schema access and the `TreeNode` traversal/rewrite APIs for visiting and transforming expressions. ([Docs.rs][1])

---

## 45.1 Expression lifecycle map

```text id="qmw6vb"
Source form
  ├─ SQL AST expression
  │   └─ sqlparser::ast::Expr
  ├─ DataFrame API expression
  │   └─ col("a").gt(lit(5))
  ├─ LogicalPlanBuilder expression
  │   └─ builder.filter(expr)
  ├─ UDF expression
  │   └─ ScalarFunction(Expr...)
  └─ optimizer-generated expression
      └─ rewritten / simplified / folded Expr

Binding / semantic planning
  ├─ column resolution
  ├─ function lookup
  ├─ aggregate/window context validation
  ├─ placeholder resolution
  ├─ Expr creation
  └─ DFSchema-mediated type/nullability inference

Analyzer / optimizer
  ├─ type coercion
  ├─ cast insertion
  ├─ constant folding
  ├─ simplification
  ├─ predicate/projection pushdown
  ├─ alias/name preservation
  └─ volatility-safe rewrites

Physical lowering
  ├─ Expr → PhysicalExpr
  ├─ logical DataType/nullability → physical expression properties
  ├─ Column binding by input index
  ├─ ScalarFunction → physical function implementation
  ├─ Cast/TryCast → Arrow kernel / physical cast expr
  └─ evaluate(RecordBatch) → ColumnarValue
```

DataFusion’s crate-level architecture docs describe SQL parsing into a `sqlparser` AST, conversion by `SqlToRel` into `LogicalPlan` and `Expr`, including name/type binding, then analysis/optimization and physical planning. ([Docs.rs][2])

---

## 45.2 Source forms

### 45.2.1 SQL AST expression

```sql id="gk4gao"
amount > 100 AND status = 'paid'
```

SQL path:

```text id="ku2es4"
SQL expression text
  → sqlparser::ast::Expr
  → SqlToRel::sql_to_expr
  → DataFusion Expr
  → analyzer/optimizer/physical planner
```

Use when:

```text id="q07myi"
user-authored SQL
SQL-compatible product interface
view definitions
CTE/subquery SQL
planner-extension tests
```

Agent rule:

```text id="jyjv20"
SQL parse success is not enough.
Expression still must bind against DFSchema and function registry.
```

---

### 45.2.2 DataFrame `Expr`

```rust id="cutj57"
use datafusion::prelude::*;

let predicate = col("amount")
    .gt(lit(100.0_f64))
    .and(col("status").eq(lit("paid")));

let df = df.filter(predicate)?;
```

Use when:

```text id="a8mpuy"
generated filters
service guardrails
tenant filters
typed request DTOs
post-SQL trusted constraints
```

DataFrame methods such as `filter`, `select`, and `aggregate` accept logical expressions; the attached expression chapter already highlights fluent expression construction like `col("a").gt(lit(6)).and(col("b").lt(lit(7)))`. 

---

### 45.2.3 `LogicalPlanBuilder` expression

```rust id="l435ly"
use datafusion::logical_expr::LogicalPlanBuilder;
use datafusion::prelude::*;

let plan = LogicalPlanBuilder::scan("orders", table_source, None)?
    .filter(col("amount").gt(lit(0.0)))?
    .project(vec![
        col("customer_id"),
        (col("amount") * lit(1.08_f64)).alias("gross_amount"),
    ])?
    .build()?;
```

Use when:

```text id="n8dd6w"
custom query compiler
PlanSpec / ExprSpec backend
optimizer rule tests
schema-stable generated plans
SQL-free planning surface
```

---

### 45.2.4 UDF expression

```rust id="ibbcg5"
// Registration happens before planning.
ctx.register_udf(my_udf);

// SQL use:
let df = ctx.sql("SELECT my_udf(a, b) AS y FROM t").await?;

// DataFrame use depends on the UDF helper object under the pinned API.
// Common pattern: expose a helper function that returns Expr.
```

UDF lifecycle:

```text id="3tmz8j"
UDF registration
  → function lookup during binding
  → Expr::ScalarFunction / AggregateFunction / WindowFunction
  → analyzer validates signature and return type
  → physical planner creates callable physical implementation
  → runtime receives ColumnarValue inputs
```

DataFusion function APIs distinguish logical function metadata from physical evaluation; `ColumnarValue` is the result of evaluating an expression and can hold either an array or scalar. ([Docs.rs][3])

---

### 45.2.5 Optimizer-generated expression

```text id="wvew5o"
Original:
  CAST(a AS Int64) + CAST(1 AS Int64)

Analyzer / optimizer may produce:
  CAST(a AS Int64) + Int64(1)

Constant folding may produce:
  Int64(3) for 1 + 2

Predicate pushdown may clone:
  filter predicate moved closer to TableScan

Projection pushdown may extract:
  referenced columns from expression tree
```

DataFusion’s optimizer contains analyzer, logical optimizer, and physical optimizer rules; its guide describes rules rewriting plans and expressions while preserving results. ([Apache DataFusion][4])

Agent rule:

```text id="udodza"
Optimizer-generated Expr values must remain schema-valid, alias-safe, and volatility-safe.
```

---

## 45.3 Binding lifecycle

### 45.3.1 Column resolution

```rust id="jd1i17"
use datafusion::common::{Column, DFSchema};
use datafusion::prelude::*;

pub fn resolve_column(schema: &DFSchema, name: &str) -> datafusion::error::Result<Column> {
    let matches = schema.columns_with_unqualified_name(name);

    match matches.len() {
        0 => Err(datafusion::error::DataFusionError::Plan(format!(
            "unknown column: {name}"
        ))),
        1 => Ok(matches[0].clone()),
        n => Err(datafusion::error::DataFusionError::Plan(format!(
            "ambiguous column: {name}; {n} candidates"
        ))),
    }
}
```

Binding rules:

```text id="w9l4se"
unqualified column:
  valid only if exactly one field with that name is visible

qualified column:
  resolved against relation qualifier + field name

post-join generated expression:
  prefer qualified column references

subquery/CTE:
  resolve against output DFSchema of nested relation

outer/correlated query:
  may require outer-reference binding context
```

The attached data-model section emphasizes that `Expr` type/nullability inference requires `DFSchema`, not merely Arrow `Schema`, because column names may be qualified. 

---

### 45.3.2 Function resolution

```text id="5pfyhq"
function_name(args...)
  → normalize / quote-sensitive SQL function name
  → lookup scalar / aggregate / window function registry
  → validate function class in context
  → validate signature / argument types
  → produce ScalarFunction / AggregateFunction / WindowFunction Expr
```

Context-specific function class:

| SQL context                 | Valid expression classes                                             |
| --------------------------- | -------------------------------------------------------------------- |
| `WHERE`                     | scalar / boolean expressions; no aggregates except subquery contexts |
| `SELECT` without `GROUP BY` | scalar expressions and windows depending query shape                 |
| aggregate query `SELECT`    | grouping expressions + aggregate expressions + valid scalar wrappers |
| `HAVING`                    | aggregate-context boolean expression                                 |
| `QUALIFY`                   | window-context boolean expression                                    |
| `ORDER BY`                  | output expressions, grouping/aggregate/window context dependent      |

Agent rule:

```text id="gf7fz0"
Register functions before planning.
Use the correct function family: scalar vs aggregate vs window.
Alias UDF/aggregate/window outputs.
Document volatility and null behavior.
```

---

### 45.3.3 Aggregate/window context binding

Aggregate expression:

```rust id="e2a4bn"
use datafusion::functions_aggregate::expr_fn::sum;
use datafusion::prelude::*;

let agg = sum(col("amount")).alias("total_amount");
```

Window expression shape:

```text id="qttc0t"
window_function(args)
  OVER (
    PARTITION BY exprs
    ORDER BY sort_exprs
    frame
  )
  AS alias
```

Binding constraints:

```text id="f7d2zx"
Aggregate:
  non-aggregate selected columns must be grouped or otherwise semantically valid
  aggregate inputs bind against pre-aggregate schema
  aggregate outputs bind against post-aggregate schema

Window:
  partition/order expressions bind against input schema
  window output adds a new expression field
  QUALIFY binds after window expression creation
```

Agent rule:

```text id="vzwy91"
Aggregate/window expressions are not just scalar function calls.
Their validity depends on query shape and plan node placement.
```

---

### 45.3.4 Placeholder resolution

SQL template:

```sql id="ppljs1"
SELECT *
FROM orders
WHERE customer_id = $1
  AND amount >= $2
```

Binding/execution shape:

```rust id="bz5atq"
use datafusion::common::ScalarValue;
use datafusion::prelude::*;

let df = ctx
    .sql("SELECT * FROM orders WHERE customer_id = $1 AND amount >= $2")
    .await?
    .with_param_values(vec![
        ScalarValue::Int64(Some(123)),
        ScalarValue::Float64(Some(100.0)),
    ])?;
```

Placeholder lifecycle:

```text id="nqfnle"
SQL placeholder
  → Expr::Placeholder / parameter marker
  → parameter type inferred or supplied by context
  → with_param_values replaces values before execution
  → analyzer validates final typed expression
```

Agent rule:

```text id="gd5msp"
For generated plans, prefer typed Rust params → lit(value).
For SQL templates, use placeholders + with_param_values.
Never interpolate user literals into SQL strings.
```

---

## 45.4 Type/nullability inference with `ExprSchemable`

### 45.4.1 `get_type`

```rust id="9ppjvz"
use datafusion::common::DFSchema;
use datafusion::logical_expr::ExprSchemable;
use datafusion::prelude::*;

pub fn infer_type(schema: &DFSchema, expr: &Expr) -> datafusion::error::Result<DataType> {
    expr.get_type(schema)
}
```

`ExprSchemable::get_type` is the documented way to access the `DataType` of an `Expr` against a schema; DataFusion’s `Expr` docs point to this trait for schema access. ([Docs.rs][1])

### 45.4.2 `nullable`

```rust id="lm2s2t"
pub fn infer_nullability(schema: &DFSchema, expr: &Expr) -> datafusion::error::Result<bool> {
    expr.nullable(schema)
}
```

### 45.4.3 `to_field`

```rust id="5xb5hr"
pub fn infer_field(
    schema: &DFSchema,
    expr: &Expr,
) -> datafusion::error::Result<(Option<datafusion::common::TableReference>, Field)> {
    expr.to_field(schema)
}
```

### 45.4.4 Preflight helper

```rust id="dabaxo"
#[derive(Debug, Clone)]
pub struct ExprTypeReport {
    pub expr_debug: String,
    pub data_type: DataType,
    pub nullable: bool,
    pub field_name: String,
}

pub fn preflight_expr(
    schema: &DFSchema,
    expr: &Expr,
) -> datafusion::error::Result<ExprTypeReport> {
    let data_type = expr.get_type(schema)?;
    let nullable = expr.nullable(schema)?;
    let (_qualifier, field) = expr.to_field(schema)?;

    Ok(ExprTypeReport {
        expr_debug: format!("{expr:?}"),
        data_type,
        nullable,
        field_name: field.name().clone(),
    })
}
```

Agent policy:

```text id="ggpcmh"
Before inserting generated Expr into plan:
  infer DataType
  infer nullability
  infer output Field
  reject invalid type for context
  require alias for computed output
```

---

## 45.5 Coercion lifecycle

```text id="cg3o8v"
Raw Expr:
  col("i32") + lit(1_i64)

Analyzer type coercion:
  determine common type
  insert casts as needed
  validate function signatures
  validate aggregate/window inputs

Optimized Expr:
  CAST(col("i32") AS Int64) + Int64(1)
```

DataFusion’s `TypeCoercion` analyzer rule is documented as determining schema and performing expression rewrites for type coercion. ([Docs.rs][5]) DataFusion also describes the optimizer as including expression coercion and simplification among its core optimizer capabilities. ([Apache DataFusion][6])

---

### 45.5.1 Numeric coercion

Examples:

```rust id="g6j48v"
let e1 = col("i32_col") + lit(1_i64);
let e2 = col("float_col") + lit(1_i64);
let e3 = col("decimal_col") + lit(1_i64);
```

Coercion concerns:

```text id="mio7sx"
integer width:
  Int8/Int16/Int32/Int64 common supertype

signed/unsigned:
  possible widening or error depending range/type rules

integer + float:
  often coerces toward floating type

decimal + integer:
  decimal precision/scale rules matter

numeric literal:
  Rust literal suffix controls initial Expr type in DataFrame API
```

Agent rules:

```text id="iuhp56"
Use typed literals:
  lit(1_i64), lit(1_i32), lit(1.0_f64)

For money:
  prefer DECIMAL-compatible expressions.

For generated code:
  cast explicitly when API output type matters.
```

---

### 45.5.2 String type behavior

Coercion concerns:

```text id="met50u"
Utf8 vs Utf8View
LargeUtf8
Dictionary(Int*, Utf8)
string literals
declared SQL string columns
function signatures expecting string-like values
```

DataFusion SQL type mapping currently maps SQL string declarations to `Utf8View` by default, while scalar literal behavior may differ; the attached SQL-type section calls out this distinction and recommends using `arrow_typeof` to verify inferred expression types. 

Agent rules:

```text id="7vrz21"
Do not assume every string expression is StringArray-compatible.
For UDFs:
  support string-like Arrow types you expect from your configured engine.
For stable output:
  cast/arrow_cast or configure string mapping explicitly.
```

---

### 45.5.3 Decimal coercion

```rust id="6dcgsf"
// SQL path often clearer for decimal type declaration:
SELECT CAST(amount AS DECIMAL(20, 2)) AS amount_dec
FROM t;
```

Coercion concerns:

```text id="xtvth5"
precision expansion
scale alignment
overflow behavior
Decimal128 vs Decimal256
integer/decimal common type
decimal/float crossing loses exactness
```

Agent policy:

```text id="06hlok"
Financial expressions:
  cast to DECIMAL early.
  avoid accidental DOUBLE coercion.
  assert output type with ExprSchemable or arrow_typeof.
```

---

### 45.5.4 Timestamp coercion

Concerns:

```text id="6x8yxw"
Timestamp unit:
  s / ms / us / ns

Timezone:
  None vs Some(tz)

Date vs Timestamp:
  date-to-timestamp casts may depend on timezone/session semantics

String parsing:
  to_timestamp / cast / arrow_cast can differ in accepted formats
```

Agent policy:

```text id="qec4wh"
For external contracts:
  choose timestamp unit explicitly.
  choose timezone explicitly.
  test with fixed session timezone.
  avoid implicit string timestamp coercion in production ingestion.
```

---

### 45.5.5 Struct/list coercion

Concerns:

```text id="5wg2fb"
List element common type
Struct field name mapping
Struct field order vs field name
Null-only elements
Nested nullability
```

The attached nested-data section notes DataFusion uses name-based field mapping when coercing struct types across operations, such as arrays and unions with struct values. It warns agents not to rely on field position and to cast or construct typed nulls when a field can be null-only in one branch. 

Agent policy:

```text id="5xl6h5"
For generated struct/list expressions:
  use explicit field names.
  cast null-only fields.
  test array/list element type inference.
  snapshot nested output schema.
```

---

### 45.5.6 Null coercion

```rust id="l6c8cl"
use datafusion::common::ScalarValue;
use datafusion::prelude::*;

let untyped_null = lit(ScalarValue::Null);
let typed_i64_null = lit(ScalarValue::Int64(None));
let typed_utf8_null = lit(ScalarValue::Utf8(None));
```

Null coercion concerns:

```text id="r8w5m9"
ScalarValue::Null:
  untyped null placeholder
  must be coerced from context

ScalarValue::Int64(None):
  typed NULL
  easier for generated code

CASE arms:
  NULL arm should coerce to common output type

UNION branches:
  NULL-only branch requires explicit cast/type context
```

The attached data-model chapter states that untyped NULL may be coerced by optimizer/context, while typed NULL is explicit. 

Agent policy:

```text id="eeyg93"
Generated expressions should use typed NULL when output type matters.
Avoid untyped NULL in reusable templates unless context guarantees inference.
```

---

## 45.6 Rewrite lifecycle

### 45.6.1 Constant folding

```text id="7yclz4"
Original:
  1 + 2

Folded:
  3

Original:
  DATE '2026-01-01' + INTERVAL '1 day'

Folded if rule supports and deterministic:
  DATE/TIMESTAMP literal
```

Safety rule:

```text id="43w8kd"
Fold only deterministic expressions.
Do not fold volatile functions such as random().
Time-dependent functions require query execution start time semantics.
```

`Expr::is_volatile` can identify expressions that may return different results across evaluations; the attached expression chapter specifically warns not to duplicate volatile expressions in rewrites. 

---

### 45.6.2 Simplification

```text id="37q7ni"
a AND true       → a
a OR false       → a
a = a            → true only if null semantics permit
NOT(NOT(a))      → a
CAST(CAST(x))    → possibly simplified if safe
```

Agent rule:

```text id="gk789g"
SQL three-valued logic matters.
Null-sensitive rewrites must be proven, not assumed.
```

---

### 45.6.3 Cast insertion

```text id="au61dk"
Before analyzer:
  col("i32") + lit(1_i64)

After coercion:
  CAST(col("i32") AS Int64) + lit(1_i64)
```

Cast insertion belongs to analyzer/type-coercion phases, not ordinary SQL AST parsing. DataFusion’s `TypeCoercion` analyzer rule performs expression rewrites to implement coercion. ([Docs.rs][5])

---

### 45.6.4 Alias stripping/preservation

```rust id="f8ufn5"
let original = (col("amount") * lit(1.08_f64)).alias("gross_amount");

// Optimizer may need to inspect semantic expression without alias.
// But final projection name must remain gross_amount.
```

Rewrite policy:

```text id="cwydva"
Internal comparison:
  unalias / unalias_nested may be acceptable.

Output projection:
  preserve aliases.

Optimizer rule:
  if expression name is externally visible, preserve it.

Generated plans:
  alias at construction, not after optimizer.
```

DataFusion’s optimizer guide warns that expression names must be preserved in rewrites; otherwise expected columns may not be found. ([Apache DataFusion][4])

---

### 45.6.5 Volatility constraints

```rust id="f1nhng"
if expr.is_volatile() {
    // Do not duplicate, reorder, fold, or push down unless rule explicitly handles semantics.
}
```

Volatile examples:

```text id="rdpwig"
random()
uuid()
now() / current_timestamp depending execution props and rule semantics
non-deterministic UDFs
external-state UDFs
```

Agent rewrite rules:

```text id="nlbssy"
Do not duplicate volatile expressions.
Do not push volatile predicates through joins/projections casually.
Do not constant-fold volatile calls.
Declare UDF volatility accurately.
```

---

## 45.7 Physical lowering

### 45.7.1 `Expr` → `PhysicalExpr`

```rust id="7uzqhd"
use std::sync::Arc;

use datafusion::arrow::{
    array::{ArrayRef, Int32Array},
    datatypes::{DataType, Field, Schema},
    record_batch::RecordBatch,
};
use datafusion::common::DFSchema;
use datafusion::execution::context::ExecutionProps;
use datafusion::physical_expr::create_physical_expr;
use datafusion::prelude::*;

let expr = col("a").gt(lit(1_i32));

let arrow_schema = Schema::new(vec![
    Field::new("a", DataType::Int32, true),
]);

let df_schema = DFSchema::try_from(arrow_schema)?;
let props = ExecutionProps::new();

let physical_expr = create_physical_expr(&expr, &df_schema, &props)?;

let batch = RecordBatch::try_from_iter(vec![
    ("a", Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef),
])?;

let value = physical_expr.evaluate(&batch)?;
```

`create_physical_expr` creates a physical expression from a logical `Expr`; docs state that `PhysicalExpr` is the physical counterpart to logical `Expr`, can be evaluated directly on a `RecordBatch`, and knows its type, nullability, and evaluation logic. ([Docs.rs][7]) The attached physical-plan section shows the same path and notes that evaluation returns a `ColumnarValue`. 

### 45.7.2 High-level physical expression creation

```rust id="60vmwc"
let physical_expr = ctx
    .create_physical_expr(
        col("a").gt(lit(1_i32)),
        df.schema(),
    )
    .await?;
```

Prefer the high-level `SessionContext::create_physical_expr` when available for app/tooling code because it uses DataFusion’s planning context; use low-level `create_physical_expr` for tests and tightly controlled physical-expression construction. The physical expression docs point users to both high-level `SessionContext::create_physical_expr` and low-level `create_physical_expr`. ([Docs.rs][8])

---

## 45.8 `PhysicalExpr` evaluation contract

Physical expression:

```text id="5xe8zp"
PhysicalExpr:
  type-aware
  nullable-aware
  evaluates against RecordBatch
  returns ColumnarValue
  may be state-free or function-backed
  is created by physical planner from type-coerced logical Expr
```

Common physical expression classes:

```text id="3gq6xs"
Column:
  by index into RecordBatch

Literal:
  ScalarValue

Binary expression:
  Arrow array/scalar kernel application

Cast:
  Arrow cast kernel / DataFusion cast semantics

Scalar function:
  physical scalar function implementation

Not / IsNull / IsTrue:
  boolean/null kernels

Case:
  conditional physical expression

Aggregate/window:
  not simple row-wise PhysicalExpr in same sense; lowered into aggregate/window physical operator machinery
```

`PhysicalExpr` examples include `Column` representing a column at an index in a `RecordBatch`; DataFusion docs describe `PhysicalExpr` as the physical counterpart to `Expr` used in logical planning. ([Docs.rs][8])

---

## 45.9 `ColumnarValue`

```text id="213wiu"
ColumnarValue::Array(ArrayRef):
  row-count-length Arrow array

ColumnarValue::Scalar(ScalarValue):
  one value logically repeated for every row
  avoids materializing constant arrays
```

`ColumnarValue::Array` represents a column of data stored as Arrow `ArrayRef`; a slice of `ColumnarValue`s logically represents a table where all array values have the same row count. ([Docs.rs][9]) The attached data-model chapter emphasizes that `ColumnarValue::Scalar` is an important performance optimization and should not be expanded to arrays unnecessarily in hot paths. 

### Scalar fast path

```rust id="nsvxxj"
use datafusion::logical_expr::ColumnarValue;

pub fn row_count_for_output(values: &[ColumnarValue], input_rows: usize) -> usize {
    values
        .iter()
        .find_map(|v| match v {
            ColumnarValue::Array(arr) => Some(arr.len()),
            ColumnarValue::Scalar(_) => None,
        })
        .unwrap_or(input_rows)
}
```

### Scalar-to-array only when required

```rust id="a2be2g"
pub fn force_array(
    value: ColumnarValue,
    rows: usize,
) -> datafusion::error::Result<ArrayRef> {
    value.into_array(rows)
}
```

Agent rule:

```text id="tl6c6p"
Custom functions/operators:
  accept both scalar and array inputs.
  preserve scalar outputs where possible.
  expand scalar to array only at Arrow-array-only kernel boundary.
```

---

## 45.10 Scalar vs array evaluation cases

| Expression          | Input rows | Likely `ColumnarValue` result                | Reason                          |
| ------------------- | ---------: | -------------------------------------------- | ------------------------------- |
| `lit(1)`            |      1,000 | `Scalar(Int64(1))`                           | constant value                  |
| `col("a")`          |      1,000 | `Array(a)`                                   | column reference                |
| `col("a") + lit(1)` |      1,000 | `Array`                                      | array + scalar produces array   |
| `lit(1) + lit(2)`   |      1,000 | `Scalar(Int64(3))` or folded scalar          | constant expression             |
| `is_null(col("a"))` |      1,000 | `Array(Boolean)`                             | per-row null test               |
| `lower(lit("ABC"))` |      1,000 | `Scalar("abc")` if function preserves scalar | scalar function can stay scalar |

Testing implication:

```text id="g3ywvv"
UDF tests must cover:
  scalar + scalar
  array + scalar
  scalar + array
  array + array
  null scalar
  array with nulls
  empty array
```

---

## 45.11 Null handling across lifecycle

### Logical nulls

```rust id="l6br17"
let untyped_null = lit(ScalarValue::Null);
let typed_null = lit(ScalarValue::Float64(None));
```

### Physical nulls

```text id="0dsle0"
ArrayRef:
  has validity bitmap / null count

ScalarValue:
  Option<T> None variant for typed null

ColumnarValue:
  Array with nulls or typed null scalar
```

### Predicate semantics

```rust id="vi1q6x"
let p1 = col("x").is_null();
let p2 = col("x").is_not_null();
let p3 = col("flag").is_true();
let p4 = col("flag").is_not_true();
```

Agent null rules:

```text id="gcwr7w"
Use IS NULL / is_null, not equality to NULL.
Filter keeps TRUE; FALSE and NULL are filtered out.
Typed NULL is safer in generated plans.
UDFs must define null propagation explicitly.
Do not call array.value(i) without checking is_null(i) unless null-free contract is proven.
```

The attached expression chapter lists `is_null`, `is_not_null`, and boolean tri-state checks and warns against equality comparisons to `NULL`. 

---

## 45.12 Physical function implementation expectations

For scalar UDFs and custom physical functions:

```text id="9roq7c"
Input:
  &[ColumnarValue]
  or Arrow arrays depending API layer

Must handle:
  scalar inputs
  array inputs
  mixed scalar/array inputs
  null scalar
  arrays with nulls
  empty arrays
  type mismatch
  length mismatch
  UTF8/Utf8View/dictionary string variants as applicable

Output:
  ColumnarValue::Scalar when result is scalar
  ColumnarValue::Array when per-row result
  DataType matches declared return type
  nullability matches return contract
```

Recommended implementation shape:

```rust id="6kyli5"
pub enum InputValue<'a> {
    Scalar(&'a ScalarValue),
    Array(&'a ArrayRef),
}

pub fn validate_same_array_lengths(values: &[ColumnarValue]) -> datafusion::error::Result<Option<usize>> {
    let mut len = None;

    for value in values {
        if let ColumnarValue::Array(array) = value {
            match len {
                None => len = Some(array.len()),
                Some(existing) if existing == array.len() => {}
                Some(existing) => {
                    return Err(DataFusionError::Execution(format!(
                        "array length mismatch: {existing} vs {}",
                        array.len()
                    )));
                }
            }
        }
    }

    Ok(len)
}
```

Agent rule:

```text id="zoa6lx"
Declared logical return type, physical returned array type, and output Field must agree.
```

---

## 45.13 Expression placement and pushdown

Expression placement changes where an expression is evaluated:

```text id="u8aypr"
Filter predicate:
  may push toward TableScan if source can evaluate/prune

Projection expression:
  may be pushed through projection if it only references available columns

Computed column:
  may need to remain after source scan

Volatile expression:
  must not be duplicated or reordered unsafely

UDF:
  pushdown only if source/provider understands it or it remains residual
```

DataFusion’s expression-placement API describes where expressions should be placed in the query plan for optimal execution, including pushdown decisions through projections. ([Docs.rs][10])

Agent pushdown rules:

```text id="4bwnn3"
Push simple deterministic predicates early.
Keep expensive computed expressions after column pruning where possible.
Never drop residual filters after source pushdown.
Do not push source-unknown UDFs into custom providers unless semantics are exact.
```

---

## 45.14 Expression lifecycle in a full query

```sql id="v138cs"
SELECT
  customer_id,
  SUM(amount * 1.08) AS gross_amount
FROM orders
WHERE amount > 0
  AND status = 'paid'
GROUP BY customer_id
HAVING SUM(amount * 1.08) > 1000
ORDER BY gross_amount DESC
LIMIT 100;
```

Lifecycle:

```text id="uqnjzy"
SQL parser:
  amount > 0
  status = 'paid'
  amount * 1.08
  SUM(amount * 1.08)
  gross_amount DESC

SqlToRel binding:
  amount/status/customer_id → DFSchema columns
  'paid' → literal
  0 / 1.08 → typed/inferred literals
  SUM → aggregate function metadata
  gross_amount → projection alias reference

Analyzer:
  numeric coercion for amount * 1.08
  aggregate context validation
  HAVING context validation
  ORDER BY alias validation

Optimizer:
  push amount/status filter toward scan
  maybe simplify/cast/fold literals
  preserve gross_amount alias

Physical planner:
  amount/status/customer_id columns → physical columns by index
  binary expressions → physical arithmetic/comparison expr
  SUM → aggregate physical expression/operator state
  ORDER BY gross_amount → physical sort expression

Execution:
  scan batches
  evaluate filter PhysicalExpr
  evaluate aggregate input expression
  update aggregate state
  evaluate HAVING
  sort/limit
  emit RecordBatch
```

---

## 45.15 Expression diagnostics bundle

```rust id="lptg4d"
#[derive(Debug, Clone)]
pub struct ExpressionDiagnostic {
    pub phase: ExpressionPhase,
    pub expr_debug: String,
    pub field_name: Option<String>,
    pub data_type: Option<DataType>,
    pub nullable: Option<bool>,
    pub column_refs: Vec<String>,
    pub volatile: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ExpressionPhase {
    Source,
    BoundLogical,
    Analyzed,
    Optimized,
    PhysicalLowered,
    Evaluated,
}
```

### Build diagnostic for logical `Expr`

```rust id="r59qzy"
pub fn diagnose_expr(
    schema: &DFSchema,
    expr: &Expr,
) -> ExpressionDiagnostic {
    let type_report = preflight_expr(schema, expr);

    ExpressionDiagnostic {
        phase: ExpressionPhase::BoundLogical,
        expr_debug: format!("{expr:?}"),
        field_name: type_report.as_ref().ok().map(|r| r.field_name.clone()),
        data_type: type_report.as_ref().ok().map(|r| r.data_type.clone()),
        nullable: type_report.as_ref().ok().map(|r| r.nullable),
        column_refs: expr
            .column_refs()
            .into_iter()
            .map(|c| c.to_string())
            .collect(),
        volatile: expr.is_volatile(),
        error: type_report.err().map(|e| e.to_string()),
    }
}
```

The attached expression chapter identifies `column_refs`, `column_refs_counts`, and volatility helpers as optimizer/planner analysis tools. 

---

## 45.16 Testing strategy

### 45.16.1 Type tests

```rust id="18ml0q"
#[test]
fn generated_amount_expr_has_expected_type() -> datafusion::error::Result<()> {
    let schema = DFSchema::try_from(Schema::new(vec![
        Field::new("amount", DataType::Float64, true),
    ]))?;

    let expr = col("amount") * lit(1.08_f64);

    assert_eq!(expr.get_type(&schema)?, DataType::Float64);
    assert!(expr.nullable(&schema)?);

    Ok(())
}
```

### 45.16.2 Nullability tests

```rust id="vwd8f3"
#[test]
fn non_null_column_plus_literal_is_non_null() -> datafusion::error::Result<()> {
    let schema = DFSchema::try_from(Schema::new(vec![
        Field::new("x", DataType::Int64, false),
    ]))?;

    let expr = col("x") + lit(1_i64);

    assert_eq!(expr.get_type(&schema)?, DataType::Int64);
    assert!(!expr.nullable(&schema)?);

    Ok(())
}
```

### 45.16.3 Physical output test

```rust id="rb9agg"
#[test]
fn physical_expr_evaluates_predicate() -> datafusion::error::Result<()> {
    use std::sync::Arc;
    use datafusion::arrow::{
        array::{ArrayRef, BooleanArray, Int32Array},
        record_batch::RecordBatch,
    };
    use datafusion::execution::context::ExecutionProps;
    use datafusion::physical_expr::create_physical_expr;

    let arrow_schema = Schema::new(vec![
        Field::new("a", DataType::Int32, true),
    ]);
    let df_schema = DFSchema::try_from(arrow_schema)?;

    let expr = col("a").gt(lit(1_i32));
    let physical = create_physical_expr(&expr, &df_schema, &ExecutionProps::new())?;

    let batch = RecordBatch::try_from_iter(vec![
        ("a", Arc::new(Int32Array::from(vec![1, 2, 3])) as ArrayRef),
    ])?;

    let result = physical.evaluate(&batch)?.into_array(batch.num_rows())?;
    let bools = result
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| DataFusionError::Execution("expected BooleanArray".to_string()))?;

    assert_eq!(bools.value(0), false);
    assert_eq!(bools.value(1), true);
    assert_eq!(bools.value(2), true);

    Ok(())
}
```

### 45.16.4 Error tests

```rust id="0nia29"
#[test]
fn unknown_column_fails_type_inference() -> datafusion::error::Result<()> {
    let schema = DFSchema::try_from(Schema::new(vec![
        Field::new("a", DataType::Int32, true),
    ]))?;

    let expr = col("missing").gt(lit(1_i32));

    assert!(expr.get_type(&schema).is_err());

    Ok(())
}
```

---

## 45.17 UDF test matrix

```text id="303akl"
Scalar UDF tests:
  [ ] scalar input → scalar output
  [ ] array input → array output
  [ ] scalar + array mixed input
  [ ] empty array
  [ ] all-null array
  [ ] typed null scalar
  [ ] wrong input type
  [ ] wrong input count
  [ ] Utf8 / Utf8View / Dictionary string inputs if relevant
  [ ] output DataType exactly matches declared return type
  [ ] nullability behavior documented
  [ ] volatility declared and rewrite-safe
```

Agent UDF policy:

```text id="m6zonf"
Expose Rust helper that returns Expr.
Register UDF before SQL/DataFrame planning.
Document signature, return type, null policy, volatility.
Test physical evaluation on RecordBatch.
```

---

## 45.18 Deployment advisory

```text id="sxfoth"
Generated plans:
  compile typed request → Expr.
  preflight Expr with DFSchema.
  reject invalid type/nullability before execution.
  alias every computed output.
  avoid untyped NULL unless context is controlled.

SQL services:
  classify expression errors by parse/bind/analyze/execute phase.
  apply guardrails as Expr after SQL planning where possible.
  use placeholders, not string interpolation.

Custom optimizer rules:
  preserve aliases.
  preserve semantic equivalence under null semantics.
  do not duplicate volatile expressions.
  re-run type/nullability validation after rewrite.

Custom UDFs:
  handle ColumnarValue::Scalar and Array.
  preserve scalar fast paths.
  test nulls and empty arrays.
  declare volatility accurately.

Physical operators:
  evaluate PhysicalExpr against RecordBatch.
  expect ColumnarValue, not always ArrayRef.
  avoid scalar expansion unless necessary.
```

---

## 45.19 Anti-pattern inventory

* Using Rust `&&` / `||` instead of `.and()` / `.or()`.
* Using Rust comparison operators instead of `.gt()`, `.eq()`, etc.
* Calling `col(dynamic_name)` without `DFSchema` validation.
* Comparing to `NULL` with equality.
* Leaving aggregate/function/cast/window expressions unaliased.
* Treating `Expr` display strings as stable API names.
* Assuming `SqlToRel` performs all type coercion.
* Assuming type inference can happen with Arrow `Schema` alone.
* Using untyped `ScalarValue::Null` in generated public-output expressions.
* Optimizer rewrites that strip aliases.
* Optimizer rewrites that duplicate volatile expressions.
* UDFs that assume all inputs are arrays.
* UDFs that eagerly expand scalar values to arrays.
* Physical-expression tests that only cover non-null arrays.
* Custom physical operators that ignore `ColumnarValue::Scalar`.
* Treating physical expression lowering as independent of analyzer-inserted casts.

---

## 45.20 Agent checklist

```text id="p3zz2j"
[ ] Identify expression source:
    SQL AST | DataFrame Expr | LogicalPlanBuilder Expr | UDF Expr | optimizer-generated Expr

[ ] Bind:
    resolve columns against DFSchema
    resolve functions against registry
    validate aggregate/window context
    resolve placeholders/parameters

[ ] Preflight:
    ExprSchemable::get_type
    ExprSchemable::nullable
    ExprSchemable::to_field
    column_refs / authorization
    is_volatile

[ ] Coercion:
    numeric literals typed intentionally
    strings: Utf8/Utf8View/dictionary policy
    decimals explicit for money
    timestamps explicit for unit/timezone
    structs/lists field and element types explicit
    typed NULL when context is not guaranteed

[ ] Rewrite:
    constant fold deterministic expressions only
    simplify under SQL null semantics
    preserve aliases/output names
    do not duplicate volatile expressions
    revalidate type/nullability after rewrite

[ ] Physical lowering:
    create PhysicalExpr through SessionContext or create_physical_expr
    evaluate against RecordBatch
    expect ColumnarValue::Array or Scalar
    preserve scalar fast path
    handle null arrays and null scalars

[ ] Test:
    type inference tests
    nullability tests
    physical output tests
    error tests
    UDF scalar/array/null/empty tests

[ ] Deploy:
    stream results
    avoid collect for arbitrary outputs
    register UDFs before planning
    pin DataFusion version for expression snapshots
```

[1]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.Expr.html?utm_source=chatgpt.com "Expr in datafusion::logical_expr - Rust"
[2]: https://docs.rs/datafusion/latest/src/datafusion/lib.rs.html?utm_source=chatgpt.com "datafusion/ lib.rs"
[3]: https://docs.rs/datafusion/latest/datafusion/logical_expr/index.html?utm_source=chatgpt.com "datafusion::logical_expr - Rust"
[4]: https://datafusion.apache.org/library-user-guide/query-optimizer.html?utm_source=chatgpt.com "Query Optimizer — Apache DataFusion documentation"
[5]: https://docs.rs/datafusion/latest/datafusion/optimizer/analyzer/type_coercion/struct.TypeCoercion.html?utm_source=chatgpt.com "TypeCoercion in datafusion::optimizer::analyzer"
[6]: https://datafusion.apache.org/user-guide/introduction.html?utm_source=chatgpt.com "Introduction — Apache DataFusion documentation"
[7]: https://docs.rs/datafusion/latest/datafusion/physical_expr/fn.create_physical_expr.html?utm_source=chatgpt.com "create_physical_expr in datafusion::physical_expr - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.PhysicalExpr.html?utm_source=chatgpt.com "PhysicalExpr in datafusion::physical_plan - Rust"
[9]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.ColumnarValue.html?utm_source=chatgpt.com "ColumnarValue in datafusion::logical_expr - Rust"
[10]: https://docs.rs/datafusion-expr/latest/datafusion_expr/?utm_source=chatgpt.com "datafusion_expr - Rust"


## 45.21 Lambda expressions in the DF 54 expression lifecycle

DataFusion 54 introduces three `Expr` variants for higher-order functions and lambdas (`datafusion-expr/src/expr.rs`):

```text id="lmb54v"
Expr::HigherOrderFunction(HigherOrderFunction)
  invocation of a HigherOrderUDF with arguments, some of which are lambdas
  e.g. array_transform(arr, v -> v + 1)

Expr::Lambda(Lambda)
  a lambda: parameter names + body expression

Expr::LambdaVariable(LambdaVariable)
  a named reference to a lambda parameter inside a lambda body
```

SQL `x -> expr` syntax binds through the higher-order function's signature; built-in consumers include `array_transform`, `array_filter`, and `array_any_match` (functions-nested). Registration goes through new `FunctionRegistry` methods — `register_higher_order_function`, `higher_order_function(name)`, `higher_order_function_names`, `deregister_higher_order_function` — where the impl trait is `HigherOrderUDFImpl` and `HigherOrderUDF` is the wrapper struct (there is no `RegisterFunction::HigherOrder`).

Lifecycle rules:

```text id="lmb54r"
Binding:
  a LambdaVariable resolves against the enclosing Lambda's parameter list,
  innermost scope first — NOT against the plan's DFSchema.

Column collection:
  a lambda variable is NOT a table column.
  column collectors (column_refs-style visitors, required-column
  computation for pushdown, authorization lint from 46.9) must not add
  lambda-locals to the outer plan's required-column set.

Traversal:
  expression visitors/rewriters must handle the three new variants;
  exhaustive matches over Expr written for 53 will not compile or,
  if written with catch-alls, will silently skip lambda subtrees.
```

Deep treatment (implementing `HigherOrderUDFImpl`, evaluation contract, testing): the UDF/UDAF deep-dive `datafusion_calculations_rust.md`.

---

## 45.22 Field-aware casts (DF 54)

DataFusion 54 makes casts carry a full Arrow `Field` instead of a bare `DataType`, so nullability and metadata (including extension-type metadata) survive the cast:

```rust id="fac54"
// Logical (datafusion-expr/src/expr.rs) — field is named `field`
pub struct Cast {
    pub expr: Box<Expr>,
    pub field: FieldRef,
}
// Cast::new(expr, data_type)          — wraps data_type in a nullable field
// Cast::new_from_field(expr, field)   — preserves explicit field identity

// Physical (datafusion-physical-expr/src/expressions/cast.rs) — `target_field`
// CastExpr { target_field: FieldRef, .. }
// CastExpr::new(expr, data_type)                     — derives a field
// CastExpr::new_with_target_field(expr, ..., field)  — explicit field control
```

`CastColumnExpr` was removed in 54; its role is absorbed by the field-carrying `CastExpr`. Note the naming asymmetry: the logical struct's field is `field`, the physical struct's is `target_field`.

Rewrite rule: expression rewrites that reconstruct casts must preserve the target `Field` — rebuild with `Cast::new_from_field` / `new_with_target_field`, never from `data_type()` alone, which drops metadata and nullability intent. Primary coverage: `datafusion_schemas_rust.md`.

---

# DataFusion Advanced — 46) Logical plan validation and policy linting before optimization/execution

## 46.0 Purpose

Make logical-plan inspection a first-class **security, governance, correctness, and resource-admission stage**:

```text id="8hryii"
SQL / DataFrame / LogicalPlanBuilder / custom PlanSpec
  → initial LogicalPlan
  → logical plan policy lint
      ├─ table allowlist
      ├─ column allowlist
      ├─ function allowlist
      ├─ DDL/DML/COPY denial
      ├─ direct-path denial
      ├─ unbounded scan denial
      ├─ cross join denial
      ├─ partition-filter requirement
      ├─ result cap enforcement
      ├─ expensive function denial
      └─ extension-node governance
  → only then optimize / physical plan / execute_stream
```

DataFusion `LogicalPlan` is the tree of relational operators produced by SQL planning, DataFrame APIs, or programmatic planning; docs.rs explicitly states that plans form a dataflow tree and can be inspected/re-written via the `TreeNode` API. ([Docs.rs][1]) The attached security chapter already treats logical-plan validation as a deployment hardening requirement alongside SQL class gating, table/column/function authorization, row policy, query timeouts, result caps, and resource controls. 

---

## 46.1 Why validate logical plans?

`SQLOptions` answers **“which statement classes are allowed?”** Logical-plan lint answers **“what does this planned query actually touch and require?”**

| Governance need              |  `SQLOptions`? |      Logical-plan lint? | Example                                            |
| ---------------------------- | -------------: | ----------------------: | -------------------------------------------------- |
| deny DDL                     |            yes | yes as defense-in-depth | `CREATE TABLE`, `CREATE VIEW`                      |
| deny DML                     |            yes | yes as defense-in-depth | `INSERT`, `COPY`                                   |
| deny SQL `SET`/statements    |            yes | yes as defense-in-depth | `SET datafusion.execution.target_partitions = ...` |
| table allowlist              |             no |                     yes | only `tenant_events`, `dim_customers`              |
| column allowlist/masking     |             no |                     yes | deny `ssn`, `raw_payload`                          |
| function allowlist           |             no |                     yes | deny `random`, `uuid`, `parquet_metadata`          |
| direct file path denial      | partial/config |                     yes | `SELECT * FROM 'file.parquet'`                     |
| cross join denial            |             no |                     yes | accidental Cartesian product                       |
| partition filter requirement |             no |                     yes | require `event_date` predicate                     |
| unbounded scan denial        |             no |                     yes | scan huge table without filter/limit               |
| result cap enforcement       |             no |           yes + rewrite | require `LIMIT <= max_rows`                        |
| extension-node governance    |             no |                     yes | custom logical operator approval                   |
| expensive function denial    |             no |                     yes | `levenshtein` on unbounded table                   |
| `UNNEST` cardinality control |             no |                     yes | array explosion                                    |

`SessionContext::sql` can plan DDL and DML by default, while `sql_with_options` validates a SQL query against `SQLOptions`; `execute_logical_plan` is explicitly not feature-limited, so plans supplied directly need separate validation. ([Docs.rs][2])

---

## 46.2 `SQLOptions` vs logical-plan validation

### `SQLOptions`: coarse SQL statement-class gate

```rust id="x27tpv"
use datafusion::prelude::*;

let options = SQLOptions::new()
    .with_allow_ddl(false)
    .with_allow_dml(false)
    .with_allow_statements(false);

let df = ctx.sql_with_options(user_sql, options).await?;
```

Use `SQLOptions` for:

```text id="f8ji13"
DDL denial
DML denial
SQL statement/config denial
first-line SQL endpoint restriction
```

DataFusion’s docs show `sql_with_options` rejecting `CREATE TABLE` when DDL is disabled and describe it as validating allowed queries before returning a `DataFrame`. ([Docs.rs][2])

### Logical-plan lint: semantic access/resource policy

```rust id="52ur4x"
let df = ctx.sql_with_options(user_sql, read_only_options).await?;
let plan = df.logical_plan();

let report = validate_logical_plan(plan, &policy)?;

if report.has_errors() {
    return Err(report.into_datafusion_error());
}
```

Use logical lint for:

```text id="is3lw8"
which tables are scanned
which columns are referenced
which functions are called
whether plan has unbounded scans
whether cross joins exist
whether UNNEST exists
whether public query has no LIMIT
whether table partition filters are present
whether custom extension nodes are approved
```

Policy layering:

```text id="w0hvdc"
SQL text input:
  SQLOptions first
  LogicalPlan lint second
  resource admission third
  execute_stream fourth

DataFrame / LogicalPlanBuilder input:
  no SQLOptions surface
  LogicalPlan lint first
  resource admission second
  execute_stream third

execute_logical_plan input:
  always lint before execution if plan is user-derived
```

---

## 46.3 LogicalPlan variants to govern

Current docs.rs lists `LogicalPlan` variants including `Projection`, `Filter`, `Window`, `Aggregate`, `Sort`, `Join`, `Repartition`, `Union`, `TableScan`, `EmptyRelation`, `Subquery`, `SubqueryAlias`, `Limit`, `Statement`, `Values`, `Explain`, `Analyze`, `Extension`, `Distinct`, `Dml`, `Ddl`, `Copy`, `DescribeTable`, `Unnest`, and `RecursiveQuery`. ([Docs.rs][1])

Policy-sensitive variants:

| Variant               | Why lint                                                                               |
| --------------------- | -------------------------------------------------------------------------------------- |
| `TableScan`           | table allowlist, direct-path detection, partition-filter requirement, scan cardinality |
| `Projection`          | column allowlist, masking, output schema policy                                        |
| `Filter`              | row policy, partition filter, tenant filter verification                               |
| `Join`                | cross join, natural/cross-equivalent semantics, join cardinality                       |
| `Aggregate`           | expensive aggregate, group cardinality, output policy                                  |
| `Window`              | expensive windows, ranking without bounds                                              |
| `Sort`                | global sort without limit, expensive order by                                          |
| `Limit`               | result cap detection/enforcement                                                       |
| `Dml`                 | data modification denial                                                               |
| `Ddl`                 | catalog mutation denial                                                                |
| `Copy`                | file/object-store export denial                                                        |
| `Extension`           | custom semantics require explicit allowlist                                            |
| `Unnest`              | cardinality explosion                                                                  |
| `Analyze` / `Explain` | diagnostic exposure policy                                                             |
| `Statement`           | session/config/transaction policy                                                      |
| `RecursiveQuery`      | recursion/resource policy                                                              |

DataFusion’s logical-expression index also describes `TableScan` as producing rows from a table provider, `Projection` as evaluating expression lists, `Filter` as row filtering, `Limit` as producing the first `n` rows, `Sort` as ordering, and `Unnest` as expanding nested list columns. ([Docs.rs][3])

---

## 46.4 Policy model

```rust id="o5psq4"
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone)]
pub struct PlanPolicy {
    pub mode: PolicyMode,

    pub allowed_tables: BTreeSet<String>,
    pub denied_tables: BTreeSet<String>,

    pub allowed_columns_by_table: BTreeMap<String, BTreeSet<String>>,
    pub denied_columns_by_table: BTreeMap<String, BTreeSet<String>>,

    pub allowed_functions: BTreeSet<String>,
    pub denied_functions: BTreeSet<String>,

    pub allow_ddl: bool,
    pub allow_dml: bool,
    pub allow_copy: bool,
    pub allow_explain: bool,
    pub allow_analyze: bool,
    pub allow_extension_nodes: bool,
    pub allow_unnest: bool,
    pub allow_cross_join: bool,
    pub allow_direct_path_scans: bool,

    pub require_limit: bool,
    pub max_fetch: Option<usize>,
    pub forbid_sort_without_limit: bool,

    pub partition_filter_requirements: BTreeMap<String, BTreeSet<String>>,
    pub max_joins: Option<usize>,
    pub max_unions: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyMode {
    ReadOnlySqlService,
    TenantScopedService,
    InternalBatch,
    Admin,
}
```

Recommended defaults:

```rust id="hi57oi"
impl PlanPolicy {
    pub fn read_only_sql_service() -> Self {
        Self {
            mode: PolicyMode::ReadOnlySqlService,
            allowed_tables: BTreeSet::new(),
            denied_tables: BTreeSet::new(),
            allowed_columns_by_table: BTreeMap::new(),
            denied_columns_by_table: BTreeMap::new(),
            allowed_functions: BTreeSet::new(),
            denied_functions: BTreeSet::new(),
            allow_ddl: false,
            allow_dml: false,
            allow_copy: false,
            allow_explain: false,
            allow_analyze: false,
            allow_extension_nodes: false,
            allow_unnest: false,
            allow_cross_join: false,
            allow_direct_path_scans: false,
            require_limit: true,
            max_fetch: Some(10_000),
            forbid_sort_without_limit: true,
            partition_filter_requirements: BTreeMap::new(),
            max_joins: Some(8),
            max_unions: Some(8),
        }
    }

    pub fn internal_batch() -> Self {
        Self {
            mode: PolicyMode::InternalBatch,
            require_limit: false,
            forbid_sort_without_limit: false,
            allow_explain: true,
            allow_analyze: true,
            allow_extension_nodes: true,
            allow_unnest: true,
            allow_cross_join: true,
            ..Self::read_only_sql_service()
        }
    }

    pub fn admin() -> Self {
        Self {
            mode: PolicyMode::Admin,
            allow_ddl: true,
            allow_dml: true,
            allow_copy: true,
            allow_explain: true,
            allow_analyze: true,
            allow_extension_nodes: true,
            allow_unnest: true,
            allow_cross_join: true,
            allow_direct_path_scans: true,
            require_limit: false,
            forbid_sort_without_limit: false,
            max_fetch: None,
            ..Self::read_only_sql_service()
        }
    }
}
```

---

## 46.5 Lint result schema

```rust id="2tsmim"
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LintSeverity {
    Info,
    Warning,
    Error,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanLint {
    pub severity: LintSeverity,
    pub code: &'static str,
    pub node_path: Vec<String>,
    pub expression_path: Option<Vec<String>>,
    pub message: String,
    pub remediation: String,
    pub evidence: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct PlanLintReport {
    pub lints: Vec<PlanLint>,
    pub summary: PlanLintSummary,
}

#[derive(Debug, Clone, Default)]
pub struct PlanLintSummary {
    pub table_refs: BTreeSet<String>,
    pub column_refs: BTreeSet<String>,
    pub function_refs: BTreeSet<String>,
    pub has_limit: bool,
    pub max_fetch: Option<usize>,
    pub has_sort: bool,
    pub has_cross_join: bool,
    pub has_unnest: bool,
    pub has_ddl: bool,
    pub has_dml: bool,
    pub has_copy: bool,
    pub has_extension: bool,
}
```

Helper:

```rust id="i4nlqz"
impl PlanLintReport {
    pub fn push(
        &mut self,
        severity: LintSeverity,
        code: &'static str,
        node_path: Vec<String>,
        expression_path: Option<Vec<String>>,
        message: impl Into<String>,
        remediation: impl Into<String>,
    ) {
        self.lints.push(PlanLint {
            severity,
            code,
            node_path,
            expression_path,
            message: message.into(),
            remediation: remediation.into(),
            evidence: BTreeMap::new(),
        });
    }

    pub fn has_errors(&self) -> bool {
        self.lints
            .iter()
            .any(|l| matches!(l.severity, LintSeverity::Error | LintSeverity::Fatal))
    }

    pub fn into_datafusion_error(self) -> datafusion::error::DataFusionError {
        datafusion::error::DataFusionError::Plan(format!("{self:#?}"))
    }
}
```

Lint-code namespace:

```text id="xbq5r0"
PLAN_TABLE_DENIED
PLAN_COLUMN_DENIED
PLAN_FUNCTION_DENIED
PLAN_DDL_DENIED
PLAN_DML_DENIED
PLAN_COPY_DENIED
PLAN_EXTENSION_DENIED
PLAN_UNNEST_DENIED
PLAN_CROSS_JOIN_DENIED
PLAN_DIRECT_PATH_DENIED
PLAN_LIMIT_REQUIRED
PLAN_LIMIT_EXCEEDS_MAX
PLAN_SORT_WITHOUT_LIMIT
PLAN_PARTITION_FILTER_REQUIRED
PLAN_EXPENSIVE_FUNCTION_DENIED
PLAN_UNBOUNDED_SCAN_DENIED
```

---

## 46.6 Traversing `LogicalPlan`

### 46.6.1 TreeNode API

The `TreeNode` trait is implemented for logical plans and expressions; docs.rs says it provides inspecting APIs such as `apply`, `visit`, and `exists`, transforming APIs such as `transform`, `transform_up`, `transform_down`, and `rewrite`, and is implemented for `ExecutionPlan`, `LogicalPlan`, `Expr`, `PhysicalExpr`, and context wrappers. ([Docs.rs][4])

### 46.6.2 Collect all expressions in a plan

```rust id="te13xm"
use std::collections::BTreeSet;

use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};
use datafusion::logical_expr::{Expr, LogicalPlan};

pub fn collect_plan_expressions(plan: &LogicalPlan) -> datafusion::error::Result<Vec<Expr>> {
    let mut expressions = Vec::new();

    plan.apply(|node| {
        node.apply_expressions(|expr| {
            expressions.push(expr.clone());
            Ok(TreeNodeRecursion::Continue)
        })?;

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(expressions)
}
```

Docs.rs includes the same traversal pattern: `plan.apply` visits plan nodes, and `node.apply_expressions` recursively visits expressions embedded in each node. ([Docs.rs][1])

### 46.6.3 Collect plan-node classes

```rust id="vf7go6"
#[derive(Debug, Clone, Default)]
pub struct PlanNodeCounts {
    pub table_scans: usize,
    pub filters: usize,
    pub projections: usize,
    pub joins: usize,
    pub sorts: usize,
    pub limits: usize,
    pub dml: usize,
    pub ddl: usize,
    pub copy: usize,
    pub extensions: usize,
    pub unnest: usize,
}

pub fn count_plan_nodes(plan: &LogicalPlan) -> datafusion::error::Result<PlanNodeCounts> {
    let mut counts = PlanNodeCounts::default();

    plan.apply(|node| {
        match node {
            LogicalPlan::TableScan(_) => counts.table_scans += 1,
            LogicalPlan::Filter(_) => counts.filters += 1,
            LogicalPlan::Projection(_) => counts.projections += 1,
            LogicalPlan::Join(_) => counts.joins += 1,
            LogicalPlan::Sort(_) => counts.sorts += 1,
            LogicalPlan::Limit(_) => counts.limits += 1,
            LogicalPlan::Dml(_) => counts.dml += 1,
            LogicalPlan::Ddl(_) => counts.ddl += 1,
            LogicalPlan::Copy(_) => counts.copy += 1,
            LogicalPlan::Extension(_) => counts.extensions += 1,
            LogicalPlan::Unnest(_) => counts.unnest += 1,
            _ => {}
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(counts)
}
```

---

## 46.7 Traversing expressions inside nodes

### 46.7.1 Column references

```rust id="wisf0w"
use datafusion::logical_expr::Expr;

pub fn collect_expr_columns(expr: &Expr) -> BTreeSet<String> {
    expr.column_refs()
        .into_iter()
        .map(|c| c.to_string())
        .collect()
}

pub fn collect_plan_columns(plan: &LogicalPlan) -> datafusion::error::Result<BTreeSet<String>> {
    let mut cols = BTreeSet::new();

    for expr in collect_plan_expressions(plan)? {
        cols.extend(collect_expr_columns(&expr));
    }

    Ok(cols)
}
```

The uploaded doc’s expression section identifies `column_refs` and `column_refs_counts` as expression-analysis tools for optimizer/planner decisions. 

### 46.7.2 Function references

Exact `Expr` variants and function metadata structures can shift by DataFusion version. Under 53.x, use pattern matching against `Expr` variants from the pinned API and record scalar/aggregate/window function names.

```rust id="nyscxg"
pub fn collect_expr_functions(expr: &Expr, out: &mut BTreeSet<String>) {
    // Pseudocode shape; match exact variant fields against pinned docs.rs.
    match expr {
        Expr::ScalarFunction(fun) => {
            out.insert(fun.name().to_string());
        }

        Expr::AggregateFunction(fun) => {
            out.insert(fun.name().to_string());
        }

        Expr::WindowFunction(fun) => {
            out.insert(fun.name().to_string());
        }

        _ => {}
    }

    // Use TreeNode traversal for recursive descent in production code.
}
```

Robust traversal should use `TreeNode` over expressions rather than hand-recursing every variant.

```rust id="ye7o4f"
use datafusion::common::tree_node::{TreeNode, TreeNodeRecursion};

pub fn collect_plan_functions(plan: &LogicalPlan) -> datafusion::error::Result<BTreeSet<String>> {
    let mut funcs = BTreeSet::new();

    plan.apply(|node| {
        node.apply_expressions(|expr| {
            match expr {
                Expr::ScalarFunction(fun) => {
                    funcs.insert(fun.name().to_string());
                }
                Expr::AggregateFunction(fun) => {
                    funcs.insert(fun.name().to_string());
                }
                Expr::WindowFunction(fun) => {
                    funcs.insert(fun.name().to_string());
                }
                _ => {}
            }

            Ok(TreeNodeRecursion::Continue)
        })?;

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(funcs)
}
```

DataFusion `Expr` is the logical expression tree abstraction; official docs describe it as the core expression abstraction for computations, including expression trees and UDF expressions. ([Apache DataFusion][5])

---

## 46.8 Capturing table references

### 46.8.1 TableScan extraction

```rust id="zxugou"
pub fn collect_table_refs(plan: &LogicalPlan) -> datafusion::error::Result<BTreeSet<String>> {
    let mut tables = BTreeSet::new();

    plan.apply(|node| {
        if let LogicalPlan::TableScan(scan) = node {
            // Exact field name is version-sensitive. In many versions this is `table_name`.
            // Use pinned docs.rs/source to select exact accessor/field.
            tables.insert(scan.table_name.to_string());
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(tables)
}
```

`LogicalPlan::TableScan` is documented as producing rows from a `TableSource`, implementing SQL `FROM` tables or views. ([Docs.rs][1])

### 46.8.2 Direct path / URL scan detection

Direct quoted file-path SQL is enabled via `SessionContext::enable_url_table`; docs.rs marks it security-sensitive because it permits SQL such as `SELECT * FROM 'my_file.parquet'` to access arbitrary local files. ([Docs.rs][2])

Direct path policy:

```text id="ri31xj"
Preferred public-service posture:
  do not call enable_url_table()
  register tables explicitly
  lint TableScan table refs anyway
  deny table names or providers that encode direct file URLs
```

Heuristic table-ref check:

```rust id="aqbp0z"
pub fn looks_like_direct_path(table_ref: &str) -> bool {
    table_ref.starts_with("file:")
        || table_ref.starts_with("s3:")
        || table_ref.starts_with("gs:")
        || table_ref.starts_with("http:")
        || table_ref.starts_with("https:")
        || table_ref.contains(".parquet")
        || table_ref.contains(".csv")
        || table_ref.contains("/")
}
```

Agent rule:

```text id="lwbavh"
Best direct-path defense is configuration: do not enable URL tables.
Lint direct-path-like TableScan refs as defense-in-depth.
```

---

## 46.9 Capturing column references

Column references can appear in:

```text id="023psj"
Projection expressions
Filter predicates
Join keys / join filters
Aggregate group expressions
Aggregate input expressions
Window partition/order/argument expressions
Sort expressions
Limit-by expressions if used
Unnest expressions
Extension node expressions
```

Extraction:

```rust id="hcb70p"
pub fn collect_column_refs_by_node(
    plan: &LogicalPlan,
) -> datafusion::error::Result<BTreeMap<String, BTreeSet<String>>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut node_idx = 0usize;

    plan.apply(|node| {
        let path = format!("{}#{node_idx}", node.display());
        node_idx += 1;

        node.apply_expressions(|expr| {
            let cols = expr.column_refs()
                .into_iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>();

            out.entry(path.clone())
                .or_default()
                .extend(cols);

            Ok(TreeNodeRecursion::Continue)
        })?;

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(out)
}
```

Column policy:

```text id="m9m7wc"
Allowed columns:
  apply to resolved Column refs, not only output schema.

Denied columns:
  fail if referenced anywhere, including filters, joins, sorts, aggregates.

Masked columns:
  cannot be referenced in projection; may or may not be allowed in filters depending policy.

Tenant column:
  must be constrained by trusted predicate, not user-supplied predicate alone.
```

---

## 46.10 Capturing UDF/function references

Function lint categories:

```text id="q0s256"
Allowed:
  lower
  upper
  coalesce
  date_trunc
  sum
  count

Denied volatile:
  random
  uuid

Denied expensive:
  levenshtein
  regexp-heavy functions
  vector distance across unbounded rows

Denied diagnostic:
  version
  metadata/cache introspection
  parquet_metadata if CLI-specific/registered

Denied side-effecting:
  custom UDFs that touch network/filesystem/global state
```

Function allowlist check:

```rust id="ogibqs"
pub fn lint_functions(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    let funcs = collect_plan_functions(plan)?;

    for func in funcs {
        report.summary.function_refs.insert(func.clone());

        if policy.denied_functions.contains(&func)
            || (!policy.allowed_functions.is_empty()
                && !policy.allowed_functions.contains(&func))
        {
            report.push(
                LintSeverity::Error,
                "PLAN_FUNCTION_DENIED",
                vec![],
                None,
                format!("function `{func}` is not allowed by policy"),
                "remove the function call or register it in the policy allowlist",
            );
        }
    }

    Ok(())
}
```

Agent rule:

```text id="dbckwl"
Function policy is not covered by SQLOptions.
Use plan/expression traversal or restricted function registries.
```

---

## 46.11 Detecting DDL, DML, and COPY

```rust id="7m6t0e"
pub fn lint_statement_classes(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    plan.apply(|node| {
        match node {
            LogicalPlan::Ddl(_) => {
                report.summary.has_ddl = true;
                if !policy.allow_ddl {
                    report.push(
                        LintSeverity::Fatal,
                        "PLAN_DDL_DENIED",
                        vec![node.display().to_string()],
                        None,
                        "DDL logical plan is not allowed",
                        "use a read-only SELECT query or admin endpoint",
                    );
                }
            }

            LogicalPlan::Dml(_) => {
                report.summary.has_dml = true;
                if !policy.allow_dml {
                    report.push(
                        LintSeverity::Fatal,
                        "PLAN_DML_DENIED",
                        vec![node.display().to_string()],
                        None,
                        "DML logical plan is not allowed",
                        "use a read-only SELECT query or write-authorized endpoint",
                    );
                }
            }

            LogicalPlan::Copy(_) => {
                report.summary.has_copy = true;
                if !policy.allow_copy {
                    report.push(
                        LintSeverity::Fatal,
                        "PLAN_COPY_DENIED",
                        vec![node.display().to_string()],
                        None,
                        "COPY logical plan is not allowed",
                        "use controlled export API with path allowlist",
                    );
                }
            }

            _ => {}
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(())
}
```

Defense-in-depth:

```text id="uplga2"
Use SQLOptions to reject DDL/DML/COPY early for SQL.
Use LogicalPlan lint to reject DDL/DML/COPY from prebuilt or custom plans.
```

---

## 46.12 Detecting `TableScan`

```rust id="tenq9e"
pub fn lint_table_scans(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    plan.apply(|node| {
        if let LogicalPlan::TableScan(scan) = node {
            let table = scan.table_name.to_string();

            report.summary.table_refs.insert(table.clone());

            if policy.denied_tables.contains(&table)
                || (!policy.allowed_tables.is_empty()
                    && !policy.allowed_tables.contains(&table))
            {
                report.push(
                    LintSeverity::Error,
                    "PLAN_TABLE_DENIED",
                    vec![node.display().to_string()],
                    None,
                    format!("table `{table}` is not allowed"),
                    "query only tables included in the table allowlist",
                );
            }

            if !policy.allow_direct_path_scans && looks_like_direct_path(&table) {
                report.push(
                    LintSeverity::Fatal,
                    "PLAN_DIRECT_PATH_DENIED",
                    vec![node.display().to_string()],
                    None,
                    format!("direct path or URL-like table reference denied: {table}"),
                    "register the source as an approved table instead of querying paths directly",
                );
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(())
}
```

---

## 46.13 Detecting cross joins

DataFusion represents cross join semantics through join planning rather than a `LogicalPlan::CrossJoin` variant: the 54.1.0 `LogicalPlan` enum (`datafusion-expr/src/logical_plan/plan.rs`) has a single `Join` variant and no `CrossJoin` variant, while the SQL docs still support `CROSS JOIN`. ([Docs.rs][1])

```rust id="kv9b4k"
pub fn lint_joins(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    let mut join_count = 0usize;

    plan.apply(|node| {
        if let LogicalPlan::Join(join) = node {
            join_count += 1;

            // Exact fields for Join differ by version.
            // Under pinned API, inspect:
            // - join.join_type
            // - join.on
            // - join.filter
            // - join.null_equals_null
            //
            // Pseudocode:
            let no_equi_keys = false; // join.on.is_empty()
            let no_filter = false;    // join.filter.is_none()
            let is_cross_like = no_equi_keys && no_filter;

            if is_cross_like {
                report.summary.has_cross_join = true;

                if !policy.allow_cross_join {
                    report.push(
                        LintSeverity::Error,
                        "PLAN_CROSS_JOIN_DENIED",
                        vec![node.display().to_string()],
                        None,
                        "cross join or join without predicate is not allowed",
                        "add an explicit join predicate or use an admin/batch policy",
                    );
                }
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    if let Some(max) = policy.max_joins {
        if join_count > max {
            report.push(
                LintSeverity::Error,
                "PLAN_TOO_MANY_JOINS",
                vec![],
                None,
                format!("query has {join_count} joins, max allowed is {max}"),
                "reduce join count or use an internal batch/admin policy",
            );
        }
    }

    Ok(())
}
```

Agent policy:

```text id="xh4t33"
Do not rely only on SQL text `CROSS JOIN`.
Detect predicate-less joins in LogicalPlan::Join.
Use exact Join fields from pinned DataFusion version.
```

---

## 46.14 Detecting `Extension`

```rust id="xfr1ks"
pub fn lint_extensions(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    plan.apply(|node| {
        if let LogicalPlan::Extension(ext) = node {
            report.summary.has_extension = true;

            if !policy.allow_extension_nodes {
                report.push(
                    LintSeverity::Error,
                    "PLAN_EXTENSION_DENIED",
                    vec![node.display().to_string()],
                    None,
                    format!("logical extension node is not allowed: {}", ext.node.name()),
                    "use only built-in logical operators or run under an extension-approved policy",
                );
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(())
}
```

The attached custom-operator section emphasizes that extension logical nodes must expose stable names, inputs, schemas, expressions, explain formatting, and correct reconstruction hooks, and that they require physical planning support. 

Extension governance:

```text id="ecr5zw"
Default public policy:
  deny all LogicalPlan::Extension.

Internal policy:
  allow only named/versioned extension nodes.

Admin policy:
  allow extension nodes but still validate node-specific resource semantics.
```

---

## 46.15 Detecting `Unnest`

```rust id="7z50pv"
pub fn lint_unnest(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    plan.apply(|node| {
        if let LogicalPlan::Unnest(_) = node {
            report.summary.has_unnest = true;

            if !policy.allow_unnest {
                report.push(
                    LintSeverity::Error,
                    "PLAN_UNNEST_DENIED",
                    vec![node.display().to_string()],
                    None,
                    "UNNEST is not allowed because it can multiply row cardinality",
                    "remove UNNEST or run under a policy with cardinality controls",
                );
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(())
}
```

`Unnest` is documented as a logical operator for columns containing nested list types. ([Docs.rs][3])

UNNEST policy variants:

```text id="kz5u80"
deny:
  public SQL

allow with cap:
  require LIMIT
  require pre-filter
  require array_length predicate if detectable

allow:
  internal batch/admin
```

---

## 46.16 Detecting missing `Limit` and excessive fetch

DataFusion `LogicalPlan` exposes helpers such as `max_rows`, `fetch`, and `skip`; docs.rs says `max_rows` returns `None` if a plan can return any number of rows, and `fetch` is carried by `Sort`, `TableScan`, and `Limit` where applicable. ([Docs.rs][1])

```rust id="amneyj"
pub fn lint_limit_policy(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    let max_rows = plan.max_rows();

    report.summary.max_fetch = max_rows;
    report.summary.has_limit = max_rows.is_some();

    if policy.require_limit && max_rows.is_none() {
        report.push(
            LintSeverity::Error,
            "PLAN_LIMIT_REQUIRED",
            vec![plan.display().to_string()],
            None,
            "query has no statically known row cap",
            "add LIMIT or use a service endpoint that enforces a maximum fetch",
        );
    }

    if let (Some(max_allowed), Some(actual)) = (policy.max_fetch, max_rows) {
        if actual > max_allowed {
            report.push(
                LintSeverity::Error,
                "PLAN_LIMIT_EXCEEDS_MAX",
                vec![plan.display().to_string()],
                None,
                format!("query may return {actual} rows, max allowed is {max_allowed}"),
                format!("reduce LIMIT to {max_allowed} or less"),
            );
        }
    }

    Ok(())
}
```

Enforcement option:

```rust id="u3ny7e"
pub fn enforce_result_limit_df(df: DataFrame, max_rows: usize) -> datafusion::error::Result<DataFrame> {
    df.limit(0, Some(max_rows))
}
```

Policy distinction:

```text id="r7jyze"
Lint:
  reject or warn.

Rewrite:
  append trusted LIMIT in service code.

Preferred public endpoint:
  append trusted LIMIT even if user supplied one.
```

---

## 46.17 Detecting `Sort` without `Limit`

```rust id="68j1uo"
pub fn lint_sort_without_limit(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    if !policy.forbid_sort_without_limit {
        return Ok(());
    }

    let has_global_cap = plan.max_rows().is_some();

    plan.apply(|node| {
        if let LogicalPlan::Sort(_) = node {
            report.summary.has_sort = true;

            if !has_global_cap {
                report.push(
                    LintSeverity::Warning,
                    "PLAN_SORT_WITHOUT_LIMIT",
                    vec![node.display().to_string()],
                    None,
                    "query contains SORT without a known result cap",
                    "add LIMIT, remove ORDER BY, or run under batch/admin policy",
                );
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(())
}
```

Sort policy:

```text id="hyf3hs"
Public service:
  sort without limit = reject or warn.
  top-N = sort + limit allowed.

Internal batch:
  sort without limit allowed if resource policy permits.
```

---

## 46.18 Partition-filter requirement

Policy shape:

```rust id="sw2632"
pub struct PartitionRequirement {
    pub table: String,
    pub required_columns: BTreeSet<String>,
}
```

Simplified detection:

```rust id="qwo0cn"
pub fn collect_filter_columns(plan: &LogicalPlan) -> datafusion::error::Result<BTreeSet<String>> {
    let mut cols = BTreeSet::new();

    plan.apply(|node| {
        if let LogicalPlan::Filter(filter) = node {
            for c in filter.predicate.column_refs() {
                cols.insert(c.to_string());
            }
        }

        Ok(TreeNodeRecursion::Continue)
    })?;

    Ok(cols)
}

pub fn lint_partition_filters(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    if policy.partition_filter_requirements.is_empty() {
        return Ok(());
    }

    let table_refs = collect_table_refs(plan)?;
    let filter_cols = collect_filter_columns(plan)?;

    for table in table_refs {
        if let Some(required) = policy.partition_filter_requirements.get(&table) {
            for col in required {
                let qualified = format!("{table}.{col}");

                let present = filter_cols.contains(col) || filter_cols.contains(&qualified);

                if !present {
                    report.push(
                        LintSeverity::Error,
                        "PLAN_PARTITION_FILTER_REQUIRED",
                        vec![],
                        None,
                        format!("table `{table}` requires filter on partition column `{col}`"),
                        format!("add a predicate on `{col}` before querying `{table}`"),
                    );
                }
            }
        }
    }

    Ok(())
}
```

Caveat:

```text id="hvqh1z"
This simple detector only checks column presence in Filter predicates.
Production-grade partition-filter validation should understand:
  equality/range predicate shape
  OR semantics
  pushed-down filters inside TableScan
  aliases/projections
  partition columns derived from table metadata
```

---

## 46.19 Expensive function denial

```rust id="xhdm1g"
pub fn lint_expensive_functions(
    plan: &LogicalPlan,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    let expensive = BTreeSet::from([
        "levenshtein".to_string(),
        "regexp_match".to_string(),
        "regexp_replace".to_string(),
        "cosine_distance".to_string(),
        "array_distance".to_string(),
    ]);

    let funcs = collect_plan_functions(plan)?;

    for f in funcs {
        if expensive.contains(&f) {
            report.push(
                LintSeverity::Warning,
                "PLAN_EXPENSIVE_FUNCTION",
                vec![],
                None,
                format!("query uses potentially expensive function `{f}`"),
                "require selective filters, result cap, or batch/admin policy",
            );
        }
    }

    Ok(())
}
```

Stronger policy:

```text id="3cb3cp"
deny expensive function if:
  no LIMIT
  no selective partition filter
  plan has cross join
  input table estimated rows exceed threshold
  function appears in join predicate
```

---

## 46.20 Full validator skeleton

```rust id="te5jdx"
pub fn validate_logical_plan(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
) -> datafusion::error::Result<PlanLintReport> {
    let mut report = PlanLintReport::default();

    lint_statement_classes(plan, policy, &mut report)?;
    lint_table_scans(plan, policy, &mut report)?;
    lint_columns(plan, policy, &mut report)?;
    lint_functions(plan, policy, &mut report)?;
    lint_joins(plan, policy, &mut report)?;
    lint_extensions(plan, policy, &mut report)?;
    lint_unnest(plan, policy, &mut report)?;
    lint_limit_policy(plan, policy, &mut report)?;
    lint_sort_without_limit(plan, policy, &mut report)?;
    lint_partition_filters(plan, policy, &mut report)?;
    lint_expensive_functions(plan, &mut report)?;

    Ok(report)
}
```

Column lint:

```rust id="0smbmm"
pub fn lint_columns(
    plan: &LogicalPlan,
    policy: &PlanPolicy,
    report: &mut PlanLintReport,
) -> datafusion::error::Result<()> {
    let cols = collect_plan_columns(plan)?;

    for col in cols {
        report.summary.column_refs.insert(col.clone());

        // Basic form: match fully-qualified or unqualified strings.
        // Production policy should resolve table/qualifier accurately from DFSchema.
        for (table, denied) in &policy.denied_columns_by_table {
            for denied_col in denied {
                if col == *denied_col || col == format!("{table}.{denied_col}") {
                    report.push(
                        LintSeverity::Error,
                        "PLAN_COLUMN_DENIED",
                        vec![],
                        None,
                        format!("column `{col}` is denied by policy"),
                        "remove the column reference or use an authorized view/projection",
                    );
                }
            }
        }
    }

    Ok(())
}
```

---

## 46.21 Integration: read-only SQL service

```rust id="2z0hje"
use datafusion::prelude::*;
use futures::StreamExt;

pub async fn run_read_only_query(
    ctx: &SessionContext,
    user_sql: &str,
    policy: &PlanPolicy,
) -> datafusion::error::Result<()> {
    let sql_options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    let df = ctx.sql_with_options(user_sql, sql_options).await?;

    let report = validate_logical_plan(df.logical_plan(), policy)?;

    if report.has_errors() {
        return Err(report.into_datafusion_error());
    }

    let max_rows = policy.max_fetch.unwrap_or(10_000);
    let df = df.limit(0, Some(max_rows))?;

    let mut stream = df.execute_stream().await?;

    while let Some(batch) = stream.next().await {
        let batch = batch?;
        // serialize batch to HTTP/gRPC/Flight chunk
        println!("rows={}", batch.num_rows());
    }

    Ok(())
}
```

Service rules:

```text id="b3gvtp"
Never call ctx.sql(user_sql) directly.
Never use collect for arbitrary user query output.
Use sql_with_options.
Lint LogicalPlan.
Append trusted limit.
Stream output.
```

---

## 46.22 Integration: tenant-scoped catalog

```rust id="qm40py"
pub struct TenantPolicy {
    pub tenant_id: String,
    pub plan_policy: PlanPolicy,
    pub max_rows: usize,
}

pub async fn run_tenant_query(
    ctx: &SessionContext,
    user_sql: &str,
    tenant: &TenantPolicy,
) -> datafusion::error::Result<()> {
    let options = SQLOptions::new()
        .with_allow_ddl(false)
        .with_allow_dml(false)
        .with_allow_statements(false);

    let df = ctx.sql_with_options(user_sql, options).await?;

    let report = validate_logical_plan(df.logical_plan(), &tenant.plan_policy)?;
    if report.has_errors() {
        return Err(report.into_datafusion_error());
    }

    // Trusted tenant guardrail. Use a guaranteed column/view contract.
    let df = df
        .filter(col("tenant_id").eq(lit(tenant.tenant_id.as_str())))?
        .limit(0, Some(tenant.max_rows))?;

    let mut stream = df.execute_stream().await?;
    while let Some(batch) = stream.next().await {
        let _batch = batch?;
    }

    Ok(())
}
```

Tenant-service rule:

```text id="oez7m7"
Do not rely on user-supplied WHERE tenant_id = ...
Apply tenant filter programmatically or use tenant-scoped catalogs/views.
Lint both original and guarded plans if necessary.
```

---

## 46.23 Integration: internal batch job

```rust id="se0xn9"
pub async fn run_internal_batch(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;

    let policy = PlanPolicy::internal_batch();
    let report = validate_logical_plan(df.logical_plan(), &policy)?;

    for lint in &report.lints {
        tracing::warn!(?lint, "batch query plan lint");
    }

    // Internal batch may intentionally collect only if bounded.
    // Prefer streaming or write_* for large outputs.
    let mut stream = df.execute_stream().await?;

    while let Some(batch) = stream.next().await {
        let _batch = batch?;
    }

    Ok(())
}
```

Batch policy:

```text id="q25ryv"
Warnings acceptable:
  sort without limit
  unnest
  expensive functions

Errors still useful:
  forbidden table
  forbidden output path
  unknown extension node
  missing partition predicate on very large tables
```

---

## 46.24 Integration: admin mode

```rust id="df0ad5"
pub async fn run_admin_sql(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;

    let policy = PlanPolicy::admin();
    let report = validate_logical_plan(df.logical_plan(), &policy)?;

    // Admin still gets audit.
    tracing::info!(?report, "admin logical plan lint report");

    df.show().await
}
```

Admin-mode guidance:

```text id="qd95ht"
Admin mode can allow DDL/DML/COPY.
Admin mode should still audit:
  affected tables
  functions
  output paths
  extension nodes
  estimated resource profile
```

---

## 46.25 Resource-admission handoff

Logical lint should produce structured facts for resource admission:

```rust id="dblzjt"
#[derive(Debug, Clone)]
pub struct AdmissionFacts {
    pub table_count: usize,
    pub join_count: usize,
    pub has_sort: bool,
    pub has_limit: bool,
    pub has_unnest: bool,
    pub has_expensive_function: bool,
    pub max_rows: Option<usize>,
}

pub fn admission_facts(report: &PlanLintReport) -> AdmissionFacts {
    AdmissionFacts {
        table_count: report.summary.table_refs.len(),
        join_count: 0, // populate in join lint
        has_sort: report.summary.has_sort,
        has_limit: report.summary.has_limit,
        has_unnest: report.summary.has_unnest,
        has_expensive_function: report.lints.iter().any(|l| {
            l.code == "PLAN_EXPENSIVE_FUNCTION"
                || l.code == "PLAN_EXPENSIVE_FUNCTION_DENIED"
        }),
        max_rows: report.summary.max_fetch,
    }
}
```

Admission decisions:

```text id="cdbv5l"
small interactive:
  no sort without limit
  max rows <= 10k
  no unnest
  no cross join

large interactive:
  selective partition filter required
  memory budget required
  timeout larger
  stream only

batch:
  queue / semaphore
  write to sink
  admin observability
```

---

## 46.26 Testing matrix

```text id="s5fhfn"
SQL class gating:
  [ ] CREATE TABLE denied
  [ ] CREATE VIEW denied
  [ ] INSERT denied
  [ ] COPY denied
  [ ] SET denied
  [ ] SELECT allowed

File/object access:
  [ ] direct quoted local path denied
  [ ] direct s3/gs/http path denied
  [ ] CREATE EXTERNAL TABLE arbitrary LOCATION denied
  [ ] registered table allowed
  [ ] disallowed table denied

Catalog isolation:
  [ ] tenant A cannot reference tenant B table
  [ ] information_schema disabled or filtered
  [ ] table list matches policy

Column policy:
  [ ] allowed column works
  [ ] denied projection column fails
  [ ] denied filter column fails if policy requires
  [ ] denied join column fails
  [ ] masked columns not projected

Function policy:
  [ ] allowed function works
  [ ] denied scalar function fails
  [ ] denied aggregate function fails
  [ ] denied window function fails
  [ ] volatile/expensive function policy tested

Plan shape:
  [ ] cross join denied
  [ ] join count cap enforced
  [ ] missing LIMIT denied
  [ ] LIMIT above max denied
  [ ] SORT without LIMIT warned/denied
  [ ] UNNEST denied
  [ ] Extension denied
  [ ] RecursiveQuery policy tested

Partition/resource:
  [ ] required partition filter present
  [ ] required partition filter missing
  [ ] unbounded scan policy
  [ ] resource-admission facts computed
```

The attached security testing matrix already includes SQL class gating, file/object access, catalog isolation, function policy, resources, and plan validation checks for cross join, unbounded order-by, unnest/cardinality, and missing-limit policy. 

---

## 46.27 Golden lint report test

```rust id="v0b1pq"
#[tokio::test]
async fn denies_unbounded_sort_without_limit() -> datafusion::error::Result<()> {
    let ctx = test_context().await?;

    let df = ctx
        .sql("SELECT * FROM events ORDER BY event_ts DESC")
        .await?;

    let mut policy = PlanPolicy::read_only_sql_service();
    policy.require_limit = true;
    policy.forbid_sort_without_limit = true;

    let report = validate_logical_plan(df.logical_plan(), &policy)?;

    assert!(report.has_errors());
    assert!(
        report.lints.iter().any(|l| l.code == "PLAN_LIMIT_REQUIRED")
    );

    Ok(())
}
```

---

## 46.28 Best-practice deployment advisory

```text id="cv4iur"
Public read-only SQL:
  sql_with_options
  DDL/DML/statements disabled
  no enable_url_table
  logical plan lint
  trusted max LIMIT
  execute_stream
  sanitized errors

Tenant SQL:
  tenant-scoped catalog or trusted tenant filter
  table/column/function allowlists
  logical plan lint before execution
  audit query_id + SQL hash + lint summary

Internal batch:
  lint as warning/error depending policy
  allow large scans only through batch queue
  write to sink, avoid collect
  capture explain/metrics

Admin:
  allow DDL/DML/COPY intentionally
  audit all affected tables/paths
  require admin-authenticated context
  still lint extension nodes and direct paths
```

---

## 46.29 Anti-pattern inventory

* Public SQL endpoint calling `ctx.sql(user_sql)` directly.
* Relying on `SQLOptions` alone for table/column/function authorization.
* Calling `execute_logical_plan` on user-derived plans without linting.
* Enabling `enable_url_table()` in multi-tenant/public services.
* Allowing arbitrary `CREATE EXTERNAL TABLE ... LOCATION`.
* Denying `CROSS JOIN` by string search rather than logical plan inspection.
* Enforcing row security only by asking users to include `WHERE tenant_id = ...`.
* Using `collect()` for arbitrary user queries.
* Allowing `SORT` without `LIMIT` in low-latency endpoints.
* Ignoring `UNNEST` cardinality expansion.
* Allowing extension nodes without explicit node allowlist.
* Treating function registry as safe by default.
* Returning raw internal lint/debug errors to public users.
* Failing open when lint traversal hits unknown node/extension.
* Snapshotting lint by display strings only; use structured lint codes.

---

## 46.30 Agent checklist

```text id="jr0dai"
[ ] Use SQLOptions for SQL statement-class gating.
[ ] Use logical-plan lint for semantic/resource policy.
[ ] Never rely on SQLOptions alone for authorization.
[ ] Traverse LogicalPlan with TreeNode APIs.
[ ] Traverse node-local expressions with apply_expressions.
[ ] Capture table refs from TableScan.
[ ] Capture column refs from Expr::column_refs().
[ ] Capture scalar/aggregate/window function refs.
[ ] Detect Ddl, Dml, Copy.
[ ] Detect TableScan allowlist/denylist/direct path.
[ ] Detect Join cross-like conditions under pinned API.
[ ] Detect Extension nodes.
[ ] Detect Unnest.
[ ] Detect missing Limit / excessive max rows.
[ ] Detect Sort without Limit.
[ ] Detect required partition filters.
[ ] Emit structured PlanLint records:
      severity, code, node_path, expression_path, remediation.
[ ] Enforce trusted result cap after lint.
[ ] Stream output; avoid collect for user queries.
[ ] Maintain separate policies:
      read-only SQL, tenant-scoped, internal batch, admin.
[ ] Test every policy with positive and negative plans.
```

[1]: https://docs.rs/datafusion-expr/latest/datafusion_expr/logical_plan/enum.LogicalPlan.html "LogicalPlan in datafusion_expr::logical_plan - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html "SessionContext in datafusion::execution::context - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/logical_expr/index.html "datafusion::logical_expr - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/common/tree_node/trait.TreeNode.html "TreeNode in datafusion::common::tree_node - Rust"
[5]: https://datafusion.apache.org/library-user-guide/working-with-exprs.html "Working with Exprs — Apache DataFusion  documentation"


# DataFusion Advanced — 47) Planner metadata: statistics, constraints, functional dependencies, partitioning, and ordering

## 47.0 Purpose

Planner metadata is the difference between a query engine that merely **can execute** a plan and one that can **choose, prune, preserve order, avoid repartitioning, simplify relational semantics, and explain cost drivers**.

```text id="zv3w5y"
metadata sources
  ├─ TableProvider::schema / statistics / constraints / pushdown capability
  ├─ Parquet footer / row-group / page / bloom-filter metadata
  ├─ catalog / metastore / lakehouse stats
  ├─ Hive partition directories
  ├─ known ordering declarations
  ├─ known physical partitioning
  ├─ functional dependencies
  └─ custom provider estimates

planner consumers
  ├─ SQL binder / DFSchema
  ├─ analyzer
  ├─ logical optimizer
  ├─ physical planner
  ├─ physical optimizer
  ├─ scan pruning
  ├─ join planning
  ├─ sort/repartition elimination
  └─ EXPLAIN / metrics / diagnostics
```

DataFusion’s `TableProvider` trait is the key provider-side metadata boundary: it exposes schema, scan planning, filter-pushdown capability, optional constraints, and optional statistics; the scan API receives projection, filters, and limit so the source can participate in pushdown. ([Docs.rs][1]) The attached documentation already treats `ContextProvider` as SQL-planning metadata and `TableProvider` as execution-capable source metadata, which is the correct split for agents designing custom sources. 

---

## 47.1 Metadata taxonomy

| Metadata class          | Primary Rust/API location                                        | Planner value                                                               | Correctness risk if wrong                     |
| ----------------------- | ---------------------------------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------- |
| schema                  | `TableProvider::schema`, Arrow `Schema`, `DFSchema`              | column binding, type inference, projection planning                         | hard correctness failure                      |
| statistics              | `TableProvider::statistics`, file metadata, catalog stats        | pruning, join order, cost estimates, limit/cardinality estimation           | wrong cost or wrong pruning if misused        |
| constraints             | `TableProvider::constraints`, `Constraints`                      | uniqueness, nullability, relational reasoning                               | invalid simplification if false               |
| functional dependencies | `DFSchema`, `FunctionalDependencies`                             | grouping reduction, distinct/aggregate reasoning, projection simplification | invalid grouping/distinct rewrites if false   |
| ordering                | `WITH ORDER`, physical `PlanProperties`, `EquivalenceProperties` | remove sorts, sort-preserving plans, merge joins                            | wrong query results if false                  |
| partitioning            | Hive dirs, physical `Partitioning`, file groups                  | pruning, repartition insertion/removal, parallelism                         | wrong joins/aggregates if false               |
| pushdown capability     | `supports_filters_pushdown`, scan args                           | source-side filtering, indexing, late materialization                       | wrong results if inexact/exact contract false |
| freshness/version       | catalog/metastore version, file metadata version                 | cache invalidation                                                          | stale plans, stale pruning, stale auth        |

DataFusion `Statistics` fields are optional/precision-qualified because sources may provide approximate estimates and transformations are not always predictable; the docs list `num_rows`, `total_byte_size`, and `column_statistics`, each using `Precision`, and provide `new_unknown`, `to_inexact`, `project`, `with_fetch`, and merge helpers. ([Docs.rs][2])

---

## 47.2 Metadata source map

```text id="pbrxnx"
Source: TableProvider
  ├─ schema()
  ├─ constraints()
  ├─ statistics()
  ├─ supports_filters_pushdown()
  └─ scan(projection, filters, limit)

Source: Parquet
  ├─ schema
  ├─ file metadata
  ├─ row-group count / row count / byte sizes
  ├─ column chunk metadata
  ├─ min/max/null statistics
  ├─ page index
  ├─ bloom filters
  └─ key-value metadata / optional embedded indexes

Source: catalog/metastore
  ├─ table row count
  ├─ table byte size
  ├─ per-column NDV/null/min/max
  ├─ partition inventory
  ├─ table constraints
  ├─ table sort/clustering declaration
  └─ freshness version / timestamp

Source: directory layout
  ├─ file partition groups
  ├─ Hive partitions: col=value
  ├─ file sizes
  ├─ object-store listings
  └─ cache metadata

Source: physical plan
  ├─ PlanProperties
  ├─ output_partitioning
  ├─ output_ordering
  ├─ equivalence_properties
  ├─ boundedness
  └─ pipeline behavior
```

`CREATE EXTERNAL TABLE` can gather file statistics at registration time, and the official DDL docs warn that this can be expensive but substantially accelerate later queries; `datafusion.execution.collect_statistics = false` disables that registration-time collection. ([Apache DataFusion][3]) Hive-compliant partition paths are automatically detected and incorporated into table schema/data when registering the top-level directory. ([Apache DataFusion][3])

---

## 47.3 `TableProvider` metadata contract

### Required and provided methods

```rust id="jp0k11"
pub trait TableProvider: Any + Debug + Sync + Send {
    fn schema(&self) -> Arc<Schema>;

    fn table_type(&self) -> TableType;

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>>;

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>> { ... }

    fn statistics(&self) -> Option<Statistics> { ... }

    fn constraints(&self) -> Option<&Constraints> { ... }
}
```

`TableProvider::scan` receives optional projection, filters, and limit; projection pushdown lets sources read only needed columns, filter pushdown lets sources evaluate predicates during scan, and limit pushdown can help some sources avoid extra work. Exact/inexact/unsupported filter-pushdown return values are positional and must match the filters input length. ([Docs.rs][1]) `TableProvider::constraints` returns `None` when constraints are unsupported, and `Some(&Constraints::empty())` when constraints are supported but absent. ([Docs.rs][1])

DataFusion 54 removed the `fn as_any(&self) -> &dyn Any` boilerplate from `TableProvider` (and the other extension traits — `ExecutionPlan`, `PhysicalExpr`, `SchemaProvider`, `CatalogProvider`, UDF impl traits): the trait now has `Any` as a supertrait (`datafusion-catalog/src/table.rs`). Downcasting uses trait upcasting through helpers on the trait object — `provider.is::<T>()` and `provider.downcast_ref::<T>()` (from `impl dyn TableProvider`), which also work on `Arc<dyn TableProvider>` via auto-deref. Do not implement or call `as_any` on these traits under 54; only ordinary Arrow array downcasts (`array.as_any().downcast_ref::<Int64Array>()`) keep the `as_any` idiom.

### Provider metadata pattern

```rust id="do5yhf"
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::common::{ColumnStatistics, Precision, ScalarValue, Statistics};
use datafusion::datasource::TableProvider;
use datafusion::error::Result;
use datafusion::logical_expr::{Expr, TableType};
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct IndexedProvider {
    schema: SchemaRef,
    stats: Statistics,
    // constraints: Constraints, // attach when your pinned API construction is finalized
}

impl IndexedProvider {
    pub fn new() -> Self {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("event_date", DataType::Date32, false),
            Field::new("amount", DataType::Float64, true),
        ]));

        let stats = Statistics::default()
            .with_num_rows(Precision::Inexact(10_000_000))
            .with_total_byte_size(Precision::Inexact(800_000_000))
            .add_column_statistics(
                ColumnStatistics::new_unknown()
                    .with_null_count(Precision::Exact(0))
                    .with_min_value(Precision::Exact(ScalarValue::Int64(Some(1))))
                    .with_max_value(Precision::Exact(ScalarValue::Int64(Some(10_000_000)))),
            );

        Self { schema, stats }
    }
}

#[async_trait::async_trait]
impl TableProvider for IndexedProvider {
    // DF 54: no `as_any` — `Any` is a supertrait of TableProvider.

    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    fn statistics(&self) -> Option<Statistics> {
        Some(self.stats.clone())
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<datafusion::datasource::TableProviderFilterPushDown>> {
        use datafusion::datasource::TableProviderFilterPushDown;

        Ok(filters
            .iter()
            .map(|expr| {
                // Use exact only if provider guarantees full SQL-equivalent filtering.
                if references_indexed_column(expr, "id") {
                    TableProviderFilterPushDown::Exact
                } else {
                    TableProviderFilterPushDown::Unsupported
                }
            })
            .collect())
    }

    async fn scan(
        &self,
        state: &dyn datafusion::catalog::Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        // Build an ExecutionPlan that honors projection, exact pushed filters, and limit contract.
        todo!("return provider-specific scan ExecutionPlan")
    }
}

fn references_indexed_column(expr: &Expr, name: &str) -> bool {
    expr.column_refs().iter().any(|c| c.name == name)
}
```

Provider rules:

```text id="yotzoz"
Return unknown stats instead of false stats.
Return Inexact when estimate is approximate.
Return Exact only when provably exact.
Return filter pushdown Exact only when provider removes all non-matching rows.
Return Inexact only when residual DataFusion filtering remains necessary.
Never drop residual filters for Inexact pushdown.
```

---

## 47.4 Statistics model

### 47.4.1 Top-level statistics

```rust id="d92coi"
pub struct Statistics {
    pub num_rows: Precision<usize>,
    pub total_byte_size: Precision<usize>,
    pub column_statistics: Vec<ColumnStatistics>,
}
```

Semantics:

```text id="wyk5xp"
num_rows:
  estimated/scanned output rows for the relation or plan node

total_byte_size:
  bytes of output Arrow data, not necessarily bytes read from storage

column_statistics:
  one ColumnStatistics per schema field
  min/max/null_count/distinct-like fields depending DataFusion version
```

The docs explicitly state that `total_byte_size` is output-data size rather than bytes scanned/processed; a Parquet scan might read 1GB and produce 2GB Arrow data, and the 2GB output size is what `total_byte_size` tracks. ([Docs.rs][2])

### 47.4.2 Precision lattice

```text id="y4v4ho"
Precision::Exact(v)
  value known exactly

Precision::Inexact(v)
  estimate / approximate / conservative or non-conservative depending source

Precision::Absent
  unknown / unavailable
```

Examples from DataFusion docs show merging exact and inexact statistics; if exactness is lost, `Statistics::to_inexact` relaxes exactness, and merge behavior may convert missing column stats to absent. ([Docs.rs][2])

### 47.4.3 Construct unknown stats

```rust id="e00sfz"
use datafusion::common::Statistics;

let stats = Statistics::new_unknown(schema.as_ref());
```

`Statistics::new_unknown(schema)` assigns unknown statistics to each column in a schema. ([Docs.rs][2])

### 47.4.4 Project stats after projection

```rust id="8gjvbg"
let projected = stats.project(Some(&vec![2_usize, 0_usize]))?;
```

`Statistics::project` projects statistics to a list of column indices; the docs show projecting stats for `{"a","b","c"}` to `[2,1]` to obtain stats for `{"c","b"}`. ([Docs.rs][2])

### 47.4.5 Limit/fetch-adjusted stats

```rust id="kpqyff"
let after_limit = stats.with_fetch(Some(100), 0, n_partitions)?;
```

`Statistics::with_fetch(fetch, skip, n_partitions)` calculates statistics after applying `fetch` and `skip`, interpreting the input statistics as per-partition statistics and using `n_partitions` for global multi-partition computation. ([Docs.rs][2])

### 47.4.6 Physical-plan statistics contract (DF 54)

On the physical side, statistics come from `ExecutionPlan::partition_statistics`, whose DataFusion 54 signature returns a shared handle:

```rust id="pstat54"
fn partition_statistics(&self, partition: Option<usize>) -> Result<Arc<Statistics>> {
    // default: Ok(Arc::new(Statistics::new_unknown(&self.schema())))
}
```

`partition` = `Some(i)` asks for that partition's statistics; `None` asks for whole-plan statistics. The `Arc` wrapper (new in 54; the method returned a bare `Statistics` in 53) lets operators pass statistics through unchanged without cloning. When an operator must adjust child statistics before propagating them, use `Arc::unwrap_or_clone` to obtain a mutable value cheaply when the reference is unique:

```rust id="pstatmut"
let mut stats = Arc::unwrap_or_clone(child.partition_statistics(None)?);
stats.num_rows = stats.num_rows.to_inexact();
Ok(Arc::new(stats))
```

Note the asymmetry: `TableProvider::statistics` still returns `Option<Statistics>` (logical-side provider hint), while physical plan nodes return `Result<Arc<Statistics>>`. There is no separate `ExecutionPlan::statistics()` method in 54 — `partition_statistics` is the single statistics entry point. See 47.24 for the full DataFusion 54 statistics overhaul.

---

## 47.5 Column statistics

Core fields to model in agents, regardless of exact Rust struct drift:

```text id="66m1oq"
ColumnStatistics:
  min_value
  max_value
  null_count
  distinct_count / ndv where available
  byte_size where available
  precision for each value
```

Use cases:

| Field                     | Planner value                                                     |
| ------------------------- | ----------------------------------------------------------------- |
| `min_value` / `max_value` | range pruning, row-group/page pruning, predicate selectivity      |
| `null_count`              | `IS NULL` / `IS NOT NULL` pruning, cardinality estimates          |
| `distinct_count` / NDV    | join selectivity, group cardinality, distinct/aggregate estimates |
| byte width/size           | memory estimates, join build-side estimate, spill risk            |
| exactness                 | trust level for rewrite vs heuristic                              |

Construction pattern using currently documented methods:

```rust id="o2klck"
use datafusion::common::{ColumnStatistics, Precision, ScalarValue};

let col_stats = ColumnStatistics::new_unknown()
    .with_null_count(Precision::Exact(0))
    .with_min_value(Precision::Exact(ScalarValue::Int64(Some(1))))
    .with_max_value(Precision::Exact(ScalarValue::Int64(Some(1_000_000))));
```

The current `Statistics` example in docs.rs shows `ColumnStatistics::new_unknown().with_null_count(...).with_min_value(...).with_max_value(...)` inside `Statistics::add_column_statistics`. ([Docs.rs][2])

DataFusion 54 makes `distinct_count` (NDV) substantially more load-bearing: it is extracted from Parquet metadata, feeds equality-predicate selectivity and semi/anti-join cardinality estimates, and is capped at the row count. See 47.24.

---

## 47.6 Statistics quality policy

```text id="n42t6i"
Exact:
  safe for strict decisions only if source semantics are trustworthy.

Inexact:
  usable for cost estimates and heuristic choices.
  not sufficient for correctness-critical pruning unless algorithm is conservative.

Absent:
  no estimate.
  planner should remain correct and choose conservative defaults.

Stale:
  worse than absent when used as truth.
  must carry freshness/version metadata outside Statistics if needed.
```

Policy table:

| Metadata state    | Planning use                                              | Rewrite/pruning use               |
| ----------------- | --------------------------------------------------------- | --------------------------------- |
| exact fresh       | cost + pruning + some semantic simplification if relevant | allowed when semantics prove safe |
| exact stale       | dangerous                                                 | treat as inexact or absent        |
| inexact fresh     | cost/admission                                            | heuristic only                    |
| absent            | fallback planning                                         | no pruning/semantic decision      |
| private/sensitive | internal optimizer only                                   | do not expose in public explain   |

Agent rule:

```text id="y38v4y"
Unknown is safer than wrong.
Wrong ordering/partitioning/constraint metadata can corrupt results.
Wrong statistics usually corrupts performance, but can corrupt correctness if used for pruning incorrectly.
```

---

## 47.7 Parquet metadata and pruning

Parquet metadata sources:

```text id="rvt7cw"
file footer:
  schema
  row-group metadata
  column chunk metadata
  row counts
  compressed/uncompressed sizes
  min/max/null statistics
  encodings/compression
  optional key-value metadata

page index:
  page-level min/max/null metadata

bloom filters:
  value membership pruning for equality / IN-like predicates

object-store listing:
  file inventory
  size
  modified time
```

DataFusion’s Parquet pruning article describes a multi-step pruning path: column projection, row-group statistics, page statistics, and row-level filtering/late materialization. ([Apache DataFusion][4]) The official EXPLAIN guide exposes Parquet pruning metrics such as `row_groups_pruned_statistics`, `limit_pruned_row_groups`, `pushdown_rows_matched`, and `pushdown_rows_pruned`. ([Apache DataFusion][5])

### Parquet scan metadata flow

```text id="8m7yug"
SQL:
  SELECT a FROM t WHERE b > 100

Projection:
  read b for filter and a for output

Row-group pruning:
  if row_group.b.max <= 100:
    skip whole row group

Page pruning:
  if page.b.max <= 100:
    skip page

Bloom pruning:
  if predicate b = 123 and bloom says absent:
    skip row group/page depending implementation

Row-level pushdown:
  evaluate predicate during decode
  avoid materializing later columns for filtered-out rows where supported
```

### Configuration knobs

```sql id="kxu1bb"
SET datafusion.execution.parquet.pruning = true;
SET datafusion.execution.parquet.enable_page_index = true;
SET datafusion.execution.parquet.bloom_filter_on_read = true;
```

Agent policy:

```text id="sgmokz"
Use Parquet stats for pruning, not as arbitrary truth.
Treat truncated/minmax-inexact stats conservatively.
Cluster/sort data by common filters to maximize row-group/page pruning.
Use bloom filters for high-cardinality equality predicates, not low-cardinality booleans.
```

---

## 47.8 Catalog / metastore statistics

Catalog-sourced metadata examples:

```text id="jvvjv4"
table_stats:
  row_count
  byte_size
  last_analyzed_at
  stats_version
  source_snapshot_id

column_stats:
  min
  max
  null_count
  ndv
  avg_width
  top_k / histogram if custom optimizer uses them

partition_stats:
  partition_value
  row_count
  file_count
  byte_size
  min/max per column
```

External catalog adapter shape:

```rust id="josyhw"
#[derive(Debug, Clone)]
pub struct CatalogTableStats {
    pub table_ref: String,
    pub row_count: Option<StatsValue<u64>>,
    pub total_byte_size: Option<StatsValue<u64>>,
    pub columns: Vec<CatalogColumnStats>,
    pub freshness: MetadataFreshness,
}

#[derive(Debug, Clone)]
pub struct StatsValue<T> {
    pub value: T,
    pub exact: bool,
}

#[derive(Debug, Clone)]
pub struct CatalogColumnStats {
    pub name: String,
    pub null_count: Option<StatsValue<u64>>,
    pub min: Option<ScalarValue>,
    pub max: Option<ScalarValue>,
    pub ndv: Option<StatsValue<u64>>,
}

#[derive(Debug, Clone)]
pub struct MetadataFreshness {
    pub stats_version: String,
    pub collected_at_epoch_ms: i64,
    pub source_snapshot_id: Option<String>,
}
```

Adapter to DataFusion:

```rust id="rya78c"
pub fn precision_usize(value: Option<StatsValue<u64>>) -> Precision<usize> {
    match value {
        Some(v) if v.exact => Precision::Exact(v.value as usize),
        Some(v) => Precision::Inexact(v.value as usize),
        None => Precision::Absent,
    }
}
```

Agent rules:

```text id="tkep1o"
Catalog stats must carry freshness outside core Statistics.
Stats collected from old snapshots should be invalidated.
Column stats must align with current schema order.
If schema changed, map or discard stats.
```

---

## 47.9 Constraints

Constraint classes:

```text id="9hozpp"
uniqueness:
  column or column-set uniquely identifies rows

non-null:
  field cannot contain nulls

range/domain:
  column value belongs to bounded range or enum

foreign-key-like:
  column values reference another table’s key

check constraints:
  arbitrary predicate true for all rows
```

DataFusion’s `TableProvider::constraints` exposes optional table constraints, and `datafusion_common` documents `Constraints` as a list of functional constraints and `Constraint` as a table constraint object. ([Docs.rs][1]) Functional dependence docs state that determinant keys uniquely determine dependent columns, and if the determinant key is unique, it can serve as a primary key; primary-key-like dependencies may downgrade through operations such as joins. ([Docs.rs][6])

### Constraint use cases

| Constraint       | Planning value                                                          |
| ---------------- | ----------------------------------------------------------------------- |
| unique key       | distinct elimination, join cardinality estimates, primary-key reasoning |
| non-null         | simplify `IS NOT NULL`, validate join semantics, aggregate/nullability  |
| range/domain     | predicate contradiction detection, partition-like pruning               |
| foreign-key-like | join cardinality, join elimination in custom optimizers                 |
| check constraint | impossible predicate detection if expression reasoning supports it      |

### Provider constraint policy

```text id="6eqgld"
Return None:
  provider does not support/know constraints.

Return Some(empty):
  provider supports constraints but table has none.

Return Some(non-empty):
  constraints are trusted metadata.
```

Agent rule:

```text id="cpjtnr"
False constraints are worse than absent constraints.
Only expose constraints that are enforced or derived from an immutable trusted source.
```

---

## 47.10 Functional dependencies

Functional dependency:

```text id="ebnr0x"
determinant columns:
  {customer_id}

dependent columns:
  {customer_name, customer_segment}

Meaning:
  rows with same customer_id must have same customer_name/customer_segment
```

Functional dependency planner use cases:

```text id="xee0cj"
GROUP BY simplification:
  if customer_id → customer_name,
  GROUP BY customer_id, customer_name
  may be equivalent to GROUP BY customer_id for some planning purposes

DISTINCT reasoning:
  if key is unique, DISTINCT over key is redundant

Aggregate output dependencies:
  group keys determine aggregate outputs

Projection simplification:
  preserve enough determinant keys to maintain dependent semantics
```

`datafusion_common` exposes functional-dependency helpers including `aggregate_functional_dependencies`, `get_required_group_by_exprs_indices`, and `get_target_functional_dependencies`; docs describe `aggregate_functional_dependencies` as calculating functional dependencies for aggregate output when there is a `GROUP BY`, and `get_required_group_by_exprs_indices` as returning a minimal subset of group-by expressions functionally equivalent to the original set. ([Docs.rs][6])

### Functional-dependency manifest

```rust id="1ske55"
#[derive(Debug, Clone)]
pub struct FunctionalDependencySpec {
    pub determinant_indices: Vec<usize>,
    pub dependent_indices: Vec<usize>,
    pub unique: bool,
    pub source: MetadataSource,
    pub freshness: MetadataFreshness,
}

#[derive(Debug, Clone)]
pub enum MetadataSource {
    TableConstraint,
    Catalog,
    DerivedFromAggregate,
    DerivedFromProjection,
    DerivedFromJoin,
    ManualTrusted,
}
```

Agent rules:

```text id="7u1lat"
Use functional dependencies for optimizer reasoning, not public semantics alone.
Recompute or drop dependencies after projection/join/aggregate.
Unique determinant keys can become non-unique after joins/unions.
Wrong functional dependency can make GROUP BY/DISTINCT rewrites incorrect.
```

---

## 47.11 Ordering metadata

Ordering sources:

```text id="tusvph"
explicit:
  CREATE EXTERNAL TABLE ... WITH ORDER (...)

physical plan:
  ExecutionPlanProperties::output_ordering
  EquivalenceProperties

operator-derived:
  SortExec
  SortPreservingMerge
  Projection preserving input order
  Filter preserving input order

file/layout assumptions:
  sorted CSV/Parquet files
  clustered data
  table sort key from metastore

custom provider:
  provider scan declares sorted output through ExecutionPlan properties
```

`CREATE EXTERNAL TABLE` supports `WITH ORDER`, and the official DDL docs warn that this only declares the order in which data should be read; if the external file is not actually sorted by that order, results may be incorrect. ([Apache DataFusion][3]) The physical property API exposes `output_ordering`, and its docs state that `None` means no assumptions should be made, while `Some(keys)` means output within each partition is sorted by those keys. ([Docs.rs][7])

### SQL declaration

```sql id="z8jj5e"
CREATE EXTERNAL TABLE ticks (
    symbol VARCHAR NOT NULL,
    ts TIMESTAMP NOT NULL,
    price DOUBLE NOT NULL
)
STORED AS PARQUET
WITH ORDER (symbol ASC, ts ASC)
LOCATION 's3://bucket/ticks/';
```

### Ordering correctness risk

```text id="r9rh3b"
False ordering metadata can:
  remove a required SortExec
  choose merge-style operators incorrectly
  produce incorrect window/ranking/order-sensitive results
  corrupt top-k / limit-with-order behavior
```

### Ordering policy

```text id="9zw3z5"
Only declare source ordering if:
  source creation process enforces it
  validation job verifies it
  layout immutability prevents later unsorted append
  metadata invalidates on append/compaction
```

---

## 47.12 Physical plan properties

`PlanProperties` caches expensive-to-compute physical plan metadata: equivalence properties, partitioning, emission type, boundedness, evaluation type, and scheduling type. ([Docs.rs][8]) `ExecutionPlanProperties` exposes `output_partitioning`, `output_ordering`, `boundedness`, `pipeline_behavior`, and `equivalence_properties`. ([Docs.rs][7])

### Inspect physical properties

```rust id="wkblt0"
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};

pub fn inspect_physical_properties(plan: &dyn ExecutionPlan) {
    println!("partitioning={:?}", plan.output_partitioning());
    println!("ordering={:?}", plan.output_ordering());
    println!("boundedness={:?}", plan.boundedness());
    println!("pipeline_behavior={:?}", plan.pipeline_behavior());
    println!("equivalence={:?}", plan.equivalence_properties());
}
```

### Custom `ExecutionPlan` policy

```text id="mlp1li"
Custom ExecutionPlan must report:
  schema exactly
  output partitioning accurately
  output ordering accurately
  equivalence properties conservatively
  boundedness accurately
  pipeline behavior accurately
```

Agent rule:

```text id="x73yra"
If unsure, report weaker properties:
  UnknownPartitioning over false hash partitioning.
  None ordering over false ordering.
  Inexact/Absent stats over false exact stats.
```

---

## 47.13 Partitioning metadata

Partitioning classes:

```text id="xnwqid"
logical/table partitioning:
  Hive partition directories
  partition columns in table metadata
  partition pruning predicates

file partitioning:
  file groups
  object-store listing
  one or more scan partitions per file/group

physical partitioning:
  ExecutionPlan output Partitioning:
    RoundRobinBatch(n)
    Hash(exprs, n)
    UnknownPartitioning(n)

operator requirements:
  required_input_distribution
  required_input_ordering
  repartition insertion
```

DataFusion’s `Partitioning` enum includes `RoundRobinBatch(usize)`, `Hash(Vec<Arc<dyn PhysicalExpr>>, usize)`, and `UnknownPartitioning(usize)`; an `ExecutionPlan` with `n` partitions produces `n` independent async `RecordBatch` streams by calling `execute(partition_index)` for each output partition. ([Docs.rs][9])

### Hive partition example

```text id="s6cq1b"
/lake/events/event_date=2026-05-22/region=us/file-0.parquet
/lake/events/event_date=2026-05-22/region=eu/file-1.parquet
```

```sql id="38n3k9"
CREATE EXTERNAL TABLE events
STORED AS PARQUET
PARTITIONED BY (event_date, region)
LOCATION '/lake/events';

SELECT count(*)
FROM events
WHERE event_date = DATE '2026-05-22'
  AND region = 'us';
```

DataFusion docs state that Hive-compliant partition columns/values are automatically detected from paths and can be used in filters; `PARTITIONED BY` can also be specified for Hive-style partition pruning. ([Apache DataFusion][3])

### Partitioning correctness risk

```text id="18us7o"
False hash partitioning can:
  skip required repartition before join/aggregate
  produce incorrect grouped results
  produce incomplete joins

False single partition can:
  force unnecessary bottlenecks if overly conservative

UnknownPartitioning:
  may add repartition/sort work
  preserves correctness
```

---

## 47.14 Repartitioning requirements

Physical optimizers use partitioning metadata and required distributions to decide whether to insert repartitions.

```text id="ra83eo"
HashJoinExec on key k:
  requires both sides partitioned compatibly by k
  if not, planner inserts RepartitionExec

AggregateExec group by k:
  local aggregate can run per partition
  final aggregate may require partitioning by k

SortMergeJoin:
  requires compatible ordering and partitioning

Window PARTITION BY k:
  may require repartition by k and sort/order by frame key
```

`ExecutionPlan` docs state that `schema` and `properties` communicate output metadata to the optimizer, while methods such as `required_input_distribution` and `required_input_ordering` express input requirements. ([Docs.rs][10])

Agent rule:

```text id="7l0n6n"
Custom physical operators:
  expose required input distribution/order if needed.
  expose output partitioning/order conservatively.
  do not bypass repartition if correctness requires co-location.
```

---

## 47.15 Constraints + statistics + partitioning interaction

Example: primary-key indexed source.

```text id="prcqxf"
Table users:
  constraints:
    unique(id)
    id non-null

statistics:
  num_rows = Exact(10_000_000)
  id.min = Exact(1)
  id.max = Exact(10_000_000)
  id.null_count = Exact(0)
  id.ndv = Exact(10_000_000)

partitioning:
  hash partitioned by id across 64 partitions

ordering:
  not ordered
```

Planner implications:

```text id="66uqpl"
WHERE id = 123:
  exact index pushdown possible
  expected cardinality <= 1
  limit pushdown safe

JOIN users.id = orders.user_id:
  users unique side supports cardinality reasoning
  hash repartition may be avoidable if physical partitioning aligned

GROUP BY id, name:
  if id → name, name may be functionally dependent
```

Agent implementation pattern:

```rust id="rbv83d"
#[derive(Debug, Clone)]
pub struct PlannerMetadataSnapshot {
    pub table_ref: String,
    pub schema_hash: String,
    pub statistics: Option<Statistics>,
    pub constraints_summary: Vec<String>,
    pub functional_dependencies_summary: Vec<String>,
    pub logical_partition_columns: Vec<String>,
    pub physical_partitioning_summary: Option<String>,
    pub ordering_summary: Vec<String>,
    pub freshness: Option<MetadataFreshness>,
}
```

---

## 47.16 Metadata invalidation

Invalidation triggers:

```text id="a6g0z1"
data changes:
  append
  overwrite
  delete
  compaction
  vacuum
  partition add/drop

schema changes:
  column add/drop/rename
  type change
  nullability change
  metadata change

layout changes:
  sort order lost
  repartitioned files
  partition column change
  clustering change

source metadata changes:
  Parquet footer changed
  object-store listing changed
  catalog stats refreshed
  metastore snapshot changed

engine changes:
  DataFusion version
  optimizer rules
  planner behavior
  feature flags
  config options
```

Cache key pattern:

```rust id="hgureo"
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlannerMetadataCacheKey {
    pub table_ref: String,
    pub schema_hash: String,
    pub stats_version: Option<String>,
    pub source_snapshot_id: Option<String>,
    pub object_listing_etag: Option<String>,
    pub datafusion_version: String,
    pub config_hash: String,
}
```

Agent policy:

```text id="qm3sai"
Attach freshness metadata externally; core Statistics does not encode snapshot staleness.
If freshness unknown, treat stats/order/partition metadata as inexact or absent.
Invalidate plan caches when metadata snapshot changes.
```

---

## 47.17 Privacy and security implications

Planner metadata can leak sensitive facts:

```text id="r9th7t"
row count:
  existence/scale of tenant/customer data

min/max:
  salary range, date range, account balance bounds

null count:
  data completeness or sensitive missingness

distinct count:
  population size, number of users/devices/customers

partition inventory:
  active regions, dates, business activity

ordering/clustering:
  operational layout / optimization hints

file sizes:
  data volume by tenant/partition
```

Public-explain policy:

```text id="zk57gg"
Public users:
  hide exact stats
  round counts/sizes
  omit min/max for sensitive columns
  hide partition/file inventory
  hide object-store paths
  expose only high-level plan shape

Internal users:
  expose stats with role-based access
  audit access to detailed metadata

Agents:
  do not paste detailed metadata into user-visible responses unless authorized
```

---

## 47.18 Metadata quality policy

```rust id="bjenhc"
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataTrust {
    Unknown,
    Approximate,
    ExactStale,
    ExactFresh,
    VerifiedFresh,
}

#[derive(Debug, Clone)]
pub struct TrustedMetadata<T> {
    pub value: T,
    pub trust: MetadataTrust,
    pub source: MetadataSource,
    pub freshness: Option<MetadataFreshness>,
}
```

Decision policy:

| Trust           | Allowed use                                           |
| --------------- | ----------------------------------------------------- |
| `Unknown`       | no optimization assumption                            |
| `Approximate`   | cost/admission heuristic only                         |
| `ExactStale`    | maybe display, not planning truth                     |
| `ExactFresh`    | cost + conservative optimization                      |
| `VerifiedFresh` | stronger optimizer rewrites/pruning if semantics safe |

Agent rules:

```text id="v0zkzs"
For costs:
  approximate acceptable.

For correctness:
  verified metadata required.

For ordering/partitioning:
  false claims are correctness bugs.

For statistics:
  false claims usually cost bugs; false pruning claims are correctness bugs.

For constraints:
  false claims are correctness bugs.
```

---

## 47.19 Metadata audit report

```rust id="ljmy0v"
#[derive(Debug, Clone)]
pub struct MetadataAuditReport {
    pub table_ref: String,
    pub schema_fields: usize,
    pub has_statistics: bool,
    pub statistics_quality: MetadataTrust,
    pub has_constraints: bool,
    pub functional_dependency_count: usize,
    pub partition_columns: Vec<String>,
    pub ordering_claims: Vec<String>,
    pub physical_partitioning: Option<String>,
    pub freshness: Option<MetadataFreshness>,
    pub warnings: Vec<String>,
}

pub fn audit_provider_metadata(provider: &dyn TableProvider) -> MetadataAuditReport {
    let schema = provider.schema();

    let stats = provider.statistics();
    let constraints = provider.constraints();

    MetadataAuditReport {
        table_ref: "<unknown>".to_string(),
        schema_fields: schema.fields().len(),
        has_statistics: stats.is_some(),
        statistics_quality: if stats.is_some() {
            MetadataTrust::Approximate
        } else {
            MetadataTrust::Unknown
        },
        has_constraints: constraints.is_some(),
        functional_dependency_count: 0,
        partition_columns: vec![],
        ordering_claims: vec![],
        physical_partitioning: None,
        freshness: None,
        warnings: vec![],
    }
}
```

Deployment use:

```text id="uv3h1d"
Run audit at:
  engine startup
  catalog refresh
  table registration
  stats refresh
  provider implementation tests
  upgrade checks
```

---

## 47.20 Testing matrix

```text id="z7v06h"
Statistics:
  [ ] unknown stats returned when unavailable
  [ ] exact row count preserved when known
  [ ] inexact row count marked Inexact
  [ ] stale stats invalidated
  [ ] projection maps column_statistics correctly
  [ ] limit/fetch stats adjusted
  [ ] merge handles absent/inexact correctly

Parquet:
  [ ] row-group pruning observed in EXPLAIN ANALYZE metrics
  [ ] page-index pruning enabled/disabled behavior tested
  [ ] bloom-filter pruning tested on equality predicate
  [ ] truncated/inexact stats treated conservatively
  [ ] no false negative pruning

Constraints:
  [ ] unique key exposed only when enforced
  [ ] non-null constraints match Arrow field nullability/data
  [ ] dropped constraints after non-preserving join/union
  [ ] constraint absence handled safely

Functional dependencies:
  [ ] aggregate output dependencies calculated
  [ ] group-by minimization tests
  [ ] projection preserves/removes dependencies correctly
  [ ] join downgrades primary-key dependencies when needed

Ordering:
  [ ] WITH ORDER only on verified sorted files
  [ ] false order negative test catches incorrect results
  [ ] Projection/Filter preserve input ordering
  [ ] SortExec reports ordering
  [ ] custom Exec output_ordering conservative

Partitioning:
  [ ] Hive partition columns detected
  [ ] partition pruning works on partition predicates
  [ ] Hash partitioning claims verified
  [ ] UnknownPartitioning used when unsure
  [ ] repartition inserted when required

Security:
  [ ] public EXPLAIN redacts sensitive stats
  [ ] metadata access role-gated
  [ ] tenant stats not cross-visible
```

---

## 47.21 Deployment advisory

```text id="kemr0x"
Custom TableProvider:
  implement schema exactly.
  implement supports_filters_pushdown accurately.
  expose statistics only with correct Precision.
  expose constraints only if enforced/trusted.
  return Unknown/Inexact instead of false Exact.

Parquet/lake tables:
  collect stats when query pruning benefit exceeds registration cost.
  disable collect_statistics for latency-sensitive registration.
  validate Hive partition layout.
  validate sort claims before WITH ORDER.
  audit pruning metrics with EXPLAIN ANALYZE.

Catalog/metastore:
  version all stats snapshots.
  invalidate on schema/data/layout change.
  map stats by schema field identity, not only position when schemas evolve.
  redact sensitive stats for public users.

Custom ExecutionPlan:
  report PlanProperties conservatively.
  output_ordering None when uncertain.
  output_partitioning UnknownPartitioning(n) when uncertain.
  required_input_distribution/order must reflect correctness needs.

LLM agents:
  prefer weaker metadata over false metadata.
  include metadata trust/freshness in debug bundles.
  never generate optimizer rewrites that depend on unverified constraints/order.
```

---

## 47.22 Anti-pattern inventory

* Returning `Precision::Exact` for approximate catalog estimates.
* Treating Parquet min/max as exact when source metadata may be truncated/stale.
* Using stale table stats for plan-cache decisions.
* Claiming sorted source output with `WITH ORDER` without verification.
* Claiming hash partitioning from file path layout alone.
* Returning `Exact` filter pushdown when provider performs inexact filtering.
* Dropping residual filters after inexact pushdown.
* Exposing sensitive min/max/NDV stats in public explain output.
* Treating `TableProvider::statistics` as freshness-aware; freshness must be external.
* Using constraints from documentation but not enforcing them in data.
* Preserving primary-key functional dependency after a join that duplicates rows.
* Assuming Hive partition columns are present inside Parquet files.
* Using high-cardinality Hive partitions without file-count controls.
* Custom `ExecutionPlan` returning false `output_ordering`.
* Physical optimizer relying on false `PlanProperties`.

---

## 47.23 Agent checklist

```text id="mz0s6k"
[ ] Identify metadata source:
    TableProvider | Parquet | catalog | Hive partitions | custom provider | physical plan

[ ] Statistics:
    num_rows
    total_byte_size
    per-column stats
    Precision: Exact | Inexact | Absent
    freshness/version externalized

[ ] Constraints:
    unique
    non-null
    range/domain
    foreign-key-like
    enforcement/trust source documented

[ ] Functional dependencies:
    determinant columns
    dependent columns
    unique determinant?
    preserved/downgraded after projection/join/aggregate?

[ ] Ordering:
    source sortedness verified?
    WITH ORDER safe?
    output_ordering conservative?
    false ordering negative test exists?

[ ] Partitioning:
    Hive partition columns
    physical Partitioning
    required repartitioning
    UnknownPartitioning when unsure

[ ] Pushdown:
    projection pushdown correct
    filter pushdown Exact/Inexact/Unsupported accurate
    limit pushdown safe only without inexact filter hazards

[ ] Invalidation:
    schema changes
    data changes
    layout changes
    stats refresh
    DataFusion/config/optimizer changes

[ ] Security:
    metadata redaction
    role-gated EXPLAIN detail
    tenant isolation
    sensitive min/max/NDV suppression

[ ] Deployment:
    audit metadata at table registration
    snapshot metadata trust in plan artifacts
    prefer unknown/inexact over false exact
```

[1]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html "TableProvider in datafusion::datasource - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/common/struct.Statistics.html "Statistics in datafusion::common - Rust"
[3]: https://datafusion.apache.org/user-guide/sql/ddl.html "DDL — Apache DataFusion  documentation"
[4]: https://datafusion.apache.org/blog/2025/03/20/parquet-pruning/?utm_source=chatgpt.com "Parquet Pruning in DataFusion: Read Only What Matters"
[5]: https://datafusion.apache.org/user-guide/explain-usage.html?utm_source=chatgpt.com "Reading Explain Plans — Apache DataFusion documentation"
[6]: https://docs.rs/datafusion-common "datafusion_common - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlanProperties.html "ExecutionPlanProperties in datafusion::physical_plan - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_plan/execution_plan/struct.PlanProperties.html "PlanProperties in datafusion::physical_plan::execution_plan - Rust"
[9]: https://docs.rs/datafusion/latest/datafusion/physical_plan/enum.Partitioning.html "Partitioning in datafusion::physical_plan - Rust"
[10]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"


## 47.24 DataFusion 54 statistics overhaul

DataFusion 54 reworked the statistics subsystem along five axes. This subsection is the planner-metadata summary; 47.4.6 covers the core signature change.

### 47.24.1 `Arc<Statistics>` partition statistics

`ExecutionPlan::partition_statistics(Option<usize>)` now returns `Result<Arc<Statistics>>` (was a bare `Statistics` in 53; `datafusion-physical-plan/src/execution_plan.rs`). Pass-through operators forward the child's `Arc` without cloning; operators that adjust statistics use `Arc::unwrap_or_clone` to get a mutable value (cheap when the reference is unique) and re-wrap:

```rust id="stov54"
fn partition_statistics(&self, partition: Option<usize>) -> Result<Arc<Statistics>> {
    let mut stats = Arc::unwrap_or_clone(self.input.partition_statistics(partition)?);
    stats.num_rows = estimate_after_filter(stats.num_rows);
    Ok(Arc::new(stats))
}
```

### 47.24.2 `PruningStatistics` relocation and `row_counts` signature

The `PruningStatistics` trait moved to `datafusion_common::pruning` (`datafusion-common/src/pruning.rs`) — update imports from the old physical-optimizer path. Its `row_counts` method lost the column argument:

```rust id="stov54b"
// 53: fn row_counts(&self, column: &Column) -> Option<ArrayRef>;
// 54:
fn row_counts(&self) -> Option<ArrayRef>;
```

Row counts are container-level (a row group / file has one row count regardless of column), so the per-column parameter was misleading; implementations that dispatched on it must collapse to the container-level answer. `min_values`/`max_values`/`null_counts` keep their `&Column` parameter, and the returned array must still contain `num_containers()` rows.

### 47.24.3 NDV-driven cardinality estimation

Distinct counts became load-bearing in 54:

```text id="stov54c"
NDV extraction:
  distinct counts are read from Parquet metadata into ColumnStatistics

equality selectivity:
  col = literal estimates selectivity as 1/NDV instead of a fixed guess

semi/anti-join cardinality:
  LeftSemi/LeftAnti (and right variants) get NDV-based output estimates
  instead of unknown

sanity cap:
  NDV is capped at the row count wherever both are known
```

Consequence for providers: a source that reports honest `distinct_count` now materially changes join ordering, build-side selection, and filter-selectivity decisions. A source that zeroes it out gets pre-54 guessing.

### 47.24.4 Pluggable statistics registry

`datafusion.optimizer.use_statistics_registry = false` (new key, default off). When enabled, the physical optimizer consults a pluggable `StatisticsRegistry` for statistics propagation across operators instead of relying solely on each operator's built-in `partition_statistics`, enabling externally supplied or cached cardinality models. Leave it off unless you install a registry; the built-in path remains the default contract.

### 47.24.5 Statistics-driven Top-K

For `ORDER BY ... LIMIT k`, DataFusion 54 uses file- and row-group-level min/max statistics to order scan input so likely-qualifying data is read first, terminate the scan early once the Top-K heap cannot change, and in favorable layouts eliminate the sort entirely. Related keys: `optimizer.enable_topk_repartition = true` (push TopK below hash repartition when the partition key is a sort-key prefix) and `optimizer.enable_window_topn = false` (rewrite `Filter(rn<=K) → Window(ROW_NUMBER) → Sort` into a per-partition Top-K operator; off by default because high-cardinality partition keys can regress). The four `*_dynamic_filter_pushdown` keys already existed in 53 — 54 improves their behavior, they are not new knobs. Scan-side view: 51.23.

### 47.24.6 Propagation guidance for custom operators

```text id="stov54d"
Do not reset statistics to unknown out of caution.
Propagate num_rows, total_byte_size, min/max, null counts, distinct counts.
Mark each value Exact only when provably exact; otherwise Inexact.
Downgrade exactness on transformations (filters, joins) — don't discard values.
Zeroed/absent NDV disables 54's selectivity and join-cardinality machinery.
```

---

# DataFusion Advanced — 48) Analyzer and logical optimizer rule cookbook

## 48.0 Purpose

Move from “DataFusion has optimizer rules” to **implementation-grade rule authoring**:

```text id="nj69na"
SQL/DataFrame/custom LogicalPlan
  → AnalyzerRule chain
      semantic validity
      type coercion
      function rewrites
      validity-enabling rewrites
  → OptimizerRule chain
      semantic-preserving optimization
      fixed-point logical rewrites
      predicate/projection/limit/join/aggregate/expression rewrites
  → optimized LogicalPlan
  → physical planning
```

DataFusion separates analyzer rules from optimizer rules: `AnalyzerRule`s transform `LogicalPlan`s to make them valid before optimization, while `OptimizerRule`s must preserve semantics while producing a more efficient plan. The docs give type coercion and subquery-reference resolution as analyzer-rule examples, and describe optimizer rules as rewrites that compute the same results more efficiently. ([Docs.rs][1])

---

## 48.1 Analyzer vs logical optimizer

| Dimension             | Analyzer                                              | Logical optimizer                                              |
| --------------------- | ----------------------------------------------------- | -------------------------------------------------------------- |
| Trait                 | `AnalyzerRule`                                        | `OptimizerRule`                                                |
| Purpose               | make plan valid / semantically normalized             | make valid plan faster / simpler                               |
| May change semantics? | may resolve/complete semantics                        | must preserve query result semantics                           |
| Typical actions       | type coercion, function rewrites, semantic resolution | pushdown, simplification, join rewrite, subquery decorrelation |
| Input                 | initial/bound `LogicalPlan`                           | analyzed `LogicalPlan`                                         |
| Output                | valid analyzed `LogicalPlan`                          | optimized equivalent `LogicalPlan`                             |
| Failure meaning       | query semantically invalid or analyzer bug            | rewrite precondition failed or optimizer bug                   |
| Testing focus         | validity, type correctness, error cases               | result equivalence, schema/name stability, idempotence         |

`Analyzer` is documented as applying `FunctionRewrite`s and `AnalyzerRule`s to transform a `LogicalPlan` in preparation for execution; the docs specifically mention type coercion as an analyzer responsibility. ([Docs.rs][2]) DataFusion’s query optimizer guide states that the optimizer contains `OptimizerRule`s and `PhysicalOptimizerRule`s that may rewrite a plan and/or expressions so execution is faster while producing the same result. ([Apache DataFusion][3])

Agent rule:

```text id="29f17s"
If rewrite is required to make the plan valid:
  AnalyzerRule.

If rewrite preserves an already valid plan:
  OptimizerRule.

If rewrite changes physical algorithm only:
  PhysicalOptimizerRule, not this chapter.

If rewrite changes query meaning:
  not an OptimizerRule.
```

---

## 48.2 Rule trait surface

### 48.2.1 `AnalyzerRule`

```rust id="uvqo7w"
use std::fmt::Debug;

use datafusion::common::{config::ConfigOptions, Result};
use datafusion::logical_expr::LogicalPlan;
use datafusion::optimizer::analyzer::AnalyzerRule;

#[derive(Debug)]
pub struct MyAnalyzerRule;

impl AnalyzerRule for MyAnalyzerRule {
    fn analyze(
        &self,
        plan: LogicalPlan,
        config: &ConfigOptions,
    ) -> Result<LogicalPlan> {
        // Rewrite plan to make it valid / normalized.
        Ok(plan)
    }

    fn name(&self) -> &str {
        "my_analyzer_rule"
    }
}
```

`AnalyzerRule::analyze` receives an owned `LogicalPlan` and `ConfigOptions` and returns a rewritten `LogicalPlan`; `name()` returns a human-readable rule name. ([Docs.rs][1])

---

### 48.2.2 `OptimizerRule`

```rust id="664kxi"
use std::fmt::Debug;

use datafusion::common::{Result, tree_node::Transformed};
use datafusion::logical_expr::LogicalPlan;
use datafusion::optimizer::{ApplyOrder, OptimizerConfig, OptimizerRule};

#[derive(Debug)]
pub struct MyOptimizerRule;

impl OptimizerRule for MyOptimizerRule {
    fn name(&self) -> &str {
        "my_optimizer_rule"
    }

    fn apply_order(&self) -> Option<ApplyOrder> {
        Some(ApplyOrder::BottomUp)
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        // Return Transformed::yes(new_plan) if rewritten.
        // Return Transformed::no(plan) if unchanged.
        Ok(Transformed::no(plan))
    }
}
```

`OptimizerRule` requires `name()` and provides `apply_order()` plus `rewrite(...)`; the docs state that `rewrite` should return `Transformed::yes` if it rewrote the plan and `Transformed::no` if unchanged. The optimizer calls `rewrite` multiple times until a fixed point is reached, so unchanged output must be reported as `Transformed::no`. ([Docs.rs][4])

---

### 48.2.3 `ApplyOrder`

```rust id="rveqfg"
fn apply_order(&self) -> Option<ApplyOrder> {
    Some(ApplyOrder::TopDown)
}
```

```rust id="pfkdlo"
fn apply_order(&self) -> Option<ApplyOrder> {
    Some(ApplyOrder::BottomUp)
}
```

`ApplyOrder::TopDown` applies a rule to a node before its inputs; `BottomUp` applies after inputs; returning `None` means the rule must handle recursion itself. ([Docs.rs][5])

Decision policy:

```text id="l7l69y"
TopDown:
  rewrite parent shape before children.
  useful for pushing predicates/projections/limits into inputs.

BottomUp:
  rewrite children first, then parent.
  useful for simplification, elimination, schema-aware cleanup.

None:
  rule owns recursion.
  useful when rewrite needs multi-node/global context.
```

---

## 48.3 Rule registration

### `SessionStateBuilder`

```rust id="mz72l4"
use std::sync::Arc;

use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::prelude::*;

let state = SessionStateBuilder::new()
    .with_default_features()
    .with_analyzer_rule(Arc::new(MyAnalyzerRule))
    .with_optimizer_rule(Arc::new(MyOptimizerRule))
    .build();

let ctx = SessionContext::new_with_state(state);
```

`SessionStateBuilder` exposes `with_analyzer_rule`, `with_analyzer_rules`, `with_optimizer_rule`, `with_optimizer_rules`, `with_physical_optimizer_rule`, and `with_physical_optimizer_rules`; the single-rule methods add rules to the end of the respective rule list. ([Docs.rs][6])

Deployment policy:

```text id="q5r4nl"
Call with_default_features() unless intentionally replacing defaults.
Append custom rules after defaults when rules depend on normalized built-in output.
Replace entire rule vector only for controlled engines/tests.
Pin DataFusion version when depending on rule order.
```

---

## 48.4 Running optimizer manually

```rust id="0kysb8"
use std::sync::Arc;

use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
use datafusion::optimizer::{Optimizer, OptimizerContext, OptimizerRule};

pub fn optimize_with_rules(
    plan: LogicalPlan,
    rules: Vec<Arc<dyn OptimizerRule + Send + Sync>>,
) -> datafusion::common::Result<LogicalPlan> {
    let optimizer = Optimizer::with_rules(rules);
    let config = OptimizerContext::new();

    optimizer.optimize(plan, &config, |_, _| {}) // observer closure shape may vary by version
}
```

The official optimizer guide demonstrates creating an optimizer with a default or custom rule set and applying it to a logical plan; it also notes that initial logical plans can come from `LogicalPlanBuilder`, the SQL planner, or the DataFrame API. ([Apache DataFusion][3])

Agent rule:

```text id="yeaorf"
Use SessionState::optimize for normal app/session behavior.
Use Optimizer directly for unit tests, rule isolation, and custom compiler pipelines.
```

---

## 48.5 Rule execution order and fixed-point behavior

Optimizer runtime model:

```text id="u51ktz"
for rule in optimizer_rules:
  apply rule recursively according to ApplyOrder
  repeat rule application until fixed point or optimizer iteration limit
  proceed to next rule
```

Implementation consequences:

```text id="lb8fj2"
Rule must be idempotent.
Rule must report no-change accurately.
Rule must not alternate between two equivalent forms.
Rule must not introduce unstable aliases or generated names each pass.
Rule must preserve schema unless intentionally changing semantics in allowed phase.
```

The `OptimizerRule::rewrite` docs explicitly state the optimizer will call `rewrite` several times until a fixed point is reached, making it important to return `Transformed::no` when output is unchanged. ([Docs.rs][4])

### Bad non-idempotent rewrite

```text id="m47kr4"
Pass 1:
  Projection(a) → Projection(a AS a_1)

Pass 2:
  Projection(a AS a_1) → Projection(a AS a_2)

Never reaches fixed point.
```

### Good idempotent rewrite

```text id="11qzjv"
Pass 1:
  Filter(true) → input

Pass 2:
  no Filter(true) exists → Transformed::no
```

---

## 48.6 Rule preconditions

Every rule should document and enforce preconditions.

```rust id="o0wl6b"
#[derive(Debug, Clone)]
pub struct RulePreconditions {
    pub requires_analyzed_plan: bool,
    pub requires_no_subqueries: bool,
    pub requires_no_volatile_exprs: bool,
    pub requires_known_schema: bool,
    pub requires_extension_node: Option<&'static str>,
    pub can_change_schema: bool,
}
```

Precondition examples:

| Rule kind                 | Required preconditions                                                               |
| ------------------------- | ------------------------------------------------------------------------------------ |
| expression simplification | analyzed types, SQL null semantics, volatility check                                 |
| predicate pushdown        | deterministic predicate, available input columns, residual filter correctness        |
| projection pushdown       | expression dependency analysis, alias/name preservation                              |
| limit pushdown            | order semantics preserved, no semantic interaction with aggregation/join incorrectly |
| join rewrite              | join predicate extraction sound, null semantics preserved                            |
| aggregate simplification  | functional dependency/constraint trust, grouping semantics preserved                 |
| extension-node rewrite    | known extension node name/version/schema contract                                    |

Agent rule:

```text id="w072d4"
If precondition cannot be proven, do not rewrite.
```

---

## 48.7 Rule output invariants

```text id="oe6xa2"
Semantic-preserving OptimizerRule invariants:
  same row set / same SQL semantics
  same output schema unless rule explicitly documented as schema-preserving-by-equivalence
  field names preserved
  expression names preserved
  aliases preserved
  qualifiers preserved unless final projection semantics justify removal
  null semantics preserved
  volatile expression evaluation count/order not changed unsafely
  statistics updated, preserved, or invalidated conservatively
```

### Schema invariant checker

```rust id="76808j"
use datafusion::common::DFSchema;

pub fn assert_same_schema(
    before: &LogicalPlan,
    after: &LogicalPlan,
    rule_name: &str,
) -> datafusion::common::Result<()> {
    let before_schema = before.schema();
    let after_schema = after.schema();

    if before_schema != after_schema {
        return Err(datafusion::common::DataFusionError::Internal(format!(
            "optimizer rule `{rule_name}` changed schema unexpectedly\nbefore={before_schema:?}\nafter={after_schema:?}"
        )));
    }

    Ok(())
}
```

### Alias/name preservation rule

```text id="fv2f16"
Do not rewrite:
  Projection(amount * 1.08 AS gross_amount)
to:
  Projection(amount * 1.08)

Do rewrite:
  Projection(amount * 1.08 AS gross_amount)
to:
  Projection((amount + amount * 0.08) AS gross_amount)
if mathematically and type/null semantics are valid.
```

The optimizer docs warn that rule authors must preserve expression names, otherwise expected columns may not be found. ([Apache DataFusion][3])

---

## 48.8 Function-specific rewrite hazard

Bad:

```rust id="zby8nd"
if func.name() == "sum" {
    // rewrite as if this is DataFusion's built-in SUM
}
```

Why unsafe:

```text id="p0aw5v"
Users can override or install compatibility packages.
A function named "sum" may not have built-in DataFusion SUM semantics.
A Spark-compatible function package may intentionally differ.
```

`OptimizerRule` docs explicitly warn rule authors to avoid function-name-specific transformations and prefer methods on `ScalarUDFImpl` and `AggregateUDFImpl`; checking `func.name() == "sum"` can be incorrect if the registered `sum` function has different semantics. ([Docs.rs][4])

Agent rule:

```text id="e5zdqk"
Never use function name alone as semantic proof.
Use function trait metadata / volatility / simplification hooks / semantic APIs from the function implementation.
```

---

## 48.9 Common logical rewrites

### 48.9.1 Predicate pushdown

```text id="aprw0m"
Before:
  Projection(a)
    Filter(b > 10)
      TableScan(t)

After:
  Projection(a)
    TableScan(t, filters=[b > 10])
or:
  Projection(a)
    Filter(residual)
      TableScan(t, pushed_filters=[supported])
```

Value:

```text id="59f8rd"
reduce rows early
enable Parquet row-group/page pruning
use provider indexes
reduce join/aggregate input cardinality
```

Correctness conditions:

```text id="3ddoek"
predicate references only columns available below target node
predicate deterministic or volatility-safe
outer join null-extension semantics preserved
provider pushdown exact/inexact contract honored
residual filter kept when pushdown inexact
```

---

### 48.9.2 Projection pushdown

```text id="mr5oy4"
Before:
  Projection(a + b AS x)
    TableScan(t: a,b,c,d)

After:
  Projection(a + b AS x)
    TableScan(t: projection=[a,b])
```

Value:

```text id="nw6zqh"
read fewer columns
reduce IO
reduce memory
reduce CPU for decoding/expressions
```

Correctness conditions:

```text id="lt4yix"
all referenced columns preserved
filter/join/sort/aggregate dependencies included
aliases preserved above scan
projection index mapping correct
```

---

### 48.9.3 Limit pushdown

```text id="sc92jy"
Before:
  Limit(fetch=100)
    TableScan(t)

After:
  Limit(fetch=100)
    TableScan(t, fetch=100)
```

Value:

```text id="gocgnp"
short-circuit scans
reduce source work
speed preview queries
```

Correctness conditions:

```text id="hik0lz"
safe only through operators that preserve sufficient row-order/cardinality semantics
unsafe through arbitrary sort/aggregate/join unless rule has proof
offset/skip semantics preserved
provider fetch semantics understood
```

---

### 48.9.4 Expression simplification

```text id="t7ifoe"
Before:
  Filter(a AND true)

After:
  Filter(a)
```

Examples:

```text id="ab7m1m"
constant folding:
  1 + 2 → 3

boolean simplification:
  a AND true → a
  a OR false → a

null-aware simplification:
  must respect SQL three-valued logic

cast simplification:
  CAST(CAST(x AS T) AS T) → CAST(x AS T) if safe
```

Value:

```text id="bof2uu"
fewer kernels
better pushdown predicates
cleaner plans
more provider-recognizable expressions
```

---

### 48.9.5 Subquery decorrelation

```sql id="rryo12"
SELECT *
FROM users u
WHERE EXISTS (
  SELECT 1
  FROM orders o
  WHERE o.user_id = u.id
);
```

Potential rewrite:

```text id="rwps0q"
Filter EXISTS correlated subquery
  → SemiJoin(users, orders, u.id = o.user_id)
```

Value:

```text id="0e2py3"
turn nested execution into joins
enable join optimizer
enable scan pushdowns
avoid repeated subquery execution
```

Correctness conditions:

```text id="u1o6ap"
correlation refs correctly identified
null semantics preserved
duplicate semantics preserved
aggregation/subquery cardinality handled
lateral semantics respected
```

DataFusion includes optimizer implementors such as `DecorrelateLateralJoin`, `DecorrelatePredicateSubquery`, and `ScalarSubqueryToJoin`, illustrating subquery-rewrite categories in the logical optimizer. ([Docs.rs][4])

---

### 48.9.6 Join reordering / join rewrite

```text id="g5bowf"
Before:
  Filter(a.x = b.y AND b.z = 100)
    CrossJoin(a, b)

After:
  Filter(b.z = 100)
    InnerJoin(a.x = b.y)
      a
      b
```

DataFusion documents `EliminateCrossJoin` as a logical optimizer rule that rewrites cross joins to inner joins when join predicates are available in filters; the docs show a `Filter` over `Cross Join` becoming a `Filter` over `InnerJoin`. ([Docs.rs][4])

Value:

```text id="lpubef"
avoid Cartesian products
enable hash/sort-merge join planning
move filters into join predicates
improve cardinality dramatically
```

Correctness conditions:

```text id="qbkxg1"
predicate extraction must preserve OR/AND structure
join type semantics preserved
null comparison semantics preserved
no volatile predicate duplication
```

---

### 48.9.7 Aggregate simplification

Examples:

```text id="ukviwn"
GROUP BY constant elimination:
  GROUP BY region, 1 → GROUP BY region

DISTINCT rewrite:
  SELECT DISTINCT a,b → Aggregate(group=[a,b])

single distinct:
  COUNT(DISTINCT a), SUM(b) rewrite strategies where supported

functional dependency reduction:
  GROUP BY key, dependent_col → GROUP BY key if key → dependent_col and output semantics preserved
```

DataFusion’s listed optimizer implementors include `EliminateGroupByConstant`, `ReplaceDistinctWithAggregate`, `SingleDistinctToGroupBy`, and aggregate-related optimizer rules. ([Docs.rs][4])

Correctness conditions:

```text id="izf47u"
functional dependencies trusted
NULL grouping semantics preserved
aggregate FILTER/DISTINCT/order modifiers preserved
aliases preserved
output schema unchanged unless explicitly expected
```

---

## 48.10 Custom rule skeleton: expression rewrite

Goal: rewrite expressions inside all plan nodes while preserving plan shape.

Example: simplify a domain predicate `amount >= 0 AND amount IS NOT NULL` into `amount >= 0` when `amount` is non-null by schema/constraint.

```rust id="4rit39"
use datafusion::common::{Result, tree_node::{Transformed, TreeNode}};
use datafusion::logical_expr::{Expr, LogicalPlan};
use datafusion::optimizer::{ApplyOrder, OptimizerConfig, OptimizerRule};

#[derive(Debug)]
pub struct MyExpressionRewriteRule;

impl OptimizerRule for MyExpressionRewriteRule {
    fn name(&self) -> &str {
        "my_expression_rewrite"
    }

    fn apply_order(&self) -> Option<ApplyOrder> {
        Some(ApplyOrder::BottomUp)
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        // Pseudocode: use DataFusion TreeNode rewrite APIs for expressions
        // under the pinned version. Exact helper names for mapping plan
        // expressions may vary by version.
        let rewritten = rewrite_expressions_in_plan(plan, |expr| {
            rewrite_expr(expr, config)
        })?;

        Ok(rewritten)
    }
}

fn rewrite_expr(
    expr: Expr,
    config: &dyn OptimizerConfig,
) -> Result<Transformed<Expr>> {
    if expr.is_volatile() {
        return Ok(Transformed::no(expr));
    }

    // Match exact Expr variants under pinned docs.rs.
    Ok(Transformed::no(expr))
}

fn rewrite_expressions_in_plan<F>(
    plan: LogicalPlan,
    mut f: F,
) -> Result<Transformed<LogicalPlan>>
where
    F: FnMut(Expr) -> Result<Transformed<Expr>>,
{
    // Use plan.map_expressions / TreeNodeRewriter / transform_up APIs
    // available under pinned DataFusion version.
    Ok(Transformed::no(plan))
}
```

Rule checklist:

```text id="633ur5"
[ ] Uses TreeNode traversal, not display-string parsing.
[ ] Checks volatility.
[ ] Preserves alias wrappers.
[ ] Preserves expression output name.
[ ] Revalidates expression type/nullability if schema-dependent.
[ ] Returns Transformed::no when unchanged.
```

---

## 48.11 Custom rule skeleton: plan-node rewrite

Goal: remove redundant `Projection` over `Projection` when safe.

```rust id="cpn6n7"
#[derive(Debug)]
pub struct CollapseProjectionRule;

impl OptimizerRule for CollapseProjectionRule {
    fn name(&self) -> &str {
        "collapse_projection"
    }

    fn apply_order(&self) -> Option<ApplyOrder> {
        Some(ApplyOrder::BottomUp)
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        match plan {
            LogicalPlan::Projection(proj) => {
                // Pseudocode:
                // if proj.input is Projection(inner) and outer expressions are direct aliases/columns
                // compose projection expressions safely.
                //
                // Required:
                // - preserve output schema
                // - preserve aliases
                // - preserve qualifiers or intentionally remove at projection boundary
                // - handle duplicate names
                Ok(Transformed::no(LogicalPlan::Projection(proj)))
            }
            other => Ok(Transformed::no(other)),
        }
    }
}
```

Plan-node rewrite checklist:

```text id="6w2qox"
[ ] Pattern-match exact LogicalPlan variant.
[ ] Prove rewrite preserves schema.
[ ] Preserve aliases/output names.
[ ] Preserve sort/order-sensitive semantics.
[ ] Avoid changing row cardinality unless exactly equivalent.
[ ] Return unchanged for unhandled nodes.
```

---

## 48.12 Custom rule skeleton: provider-aware rewrite

Goal: rewrite plans based on provider metadata, pushdown capability, constraints, or table statistics.

```rust id="545k5i"
#[derive(Debug)]
pub struct ProviderAwarePredicateRule;

impl OptimizerRule for ProviderAwarePredicateRule {
    fn name(&self) -> &str {
        "provider_aware_predicate_rule"
    }

    fn apply_order(&self) -> Option<ApplyOrder> {
        Some(ApplyOrder::TopDown)
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        match plan {
            LogicalPlan::Filter(filter) => {
                // Pseudocode:
                // inspect filter.input
                // if input is TableScan for provider with known index metadata,
                // decide whether predicate should be pushed / rewritten / annotated.
                //
                // Never remove residual filter unless provider reports exact pushdown.
                Ok(Transformed::no(LogicalPlan::Filter(filter)))
            }
            other => Ok(Transformed::no(other)),
        }
    }
}
```

Provider-aware rules need metadata discipline:

```text id="zdimwl"
Use only trusted provider metadata.
Treat statistics as estimates unless Exact+fresh.
Respect TableProviderFilterPushDown exact/inexact semantics.
Do not assume provider function semantics from table name.
Do not expose provider-internal security-sensitive metadata.
```

---

## 48.13 Custom rule skeleton: extension-node rewrite

Goal: rewrite or normalize a custom `LogicalPlan::Extension` node.

```rust id="6tayx3"
#[derive(Debug)]
pub struct MyExtensionRewriteRule;

impl OptimizerRule for MyExtensionRewriteRule {
    fn name(&self) -> &str {
        "my_extension_rewrite"
    }

    fn apply_order(&self) -> Option<ApplyOrder> {
        Some(ApplyOrder::BottomUp)
    }

    fn rewrite(
        &self,
        plan: LogicalPlan,
        _config: &dyn OptimizerConfig,
    ) -> Result<Transformed<LogicalPlan>> {
        match plan {
            LogicalPlan::Extension(extension) => {
                let node_name = extension.node.name();

                if node_name != "my_extension_node" {
                    return Ok(Transformed::no(LogicalPlan::Extension(extension)));
                }

                // Pseudocode:
                // downcast extension.node.as_any()
                // inspect version/schema/children/expressions
                // rewrite to canonical extension node or built-in plan
                // preserve node schema and semantics

                Ok(Transformed::no(LogicalPlan::Extension(extension)))
            }

            other => Ok(Transformed::no(other)),
        }
    }
}
```

Extension rewrite checklist:

```text id="6q0kz8"
[ ] Match by stable node name and version.
[ ] Downcast safely.
[ ] Preserve extension node schema.
[ ] Preserve child order and semantics.
[ ] Preserve expression aliases.
[ ] Recompute node hash/fingerprint if needed.
[ ] Test serialization if extension nodes are serialized.
[ ] Ensure physical planner still supports rewritten node.
```

---

## 48.14 Analyzer rule cookbook

### 48.14.1 Analyzer for semantic normalization

Example use cases:

```text id="io05xs"
custom type coercion policy
custom placeholder validation
domain-specific function rewrite
semantic normalization before optimizer
domain expression expansion
```

Skeleton:

```rust id="9xmvb5"
#[derive(Debug)]
pub struct DomainAnalyzerRule;

impl AnalyzerRule for DomainAnalyzerRule {
    fn name(&self) -> &str {
        "domain_analyzer_rule"
    }

    fn analyze(
        &self,
        plan: LogicalPlan,
        config: &datafusion::common::config::ConfigOptions,
    ) -> Result<LogicalPlan> {
        // Validate/normalize plan before logical optimization.
        // Analyzer may make validity-enabling changes.
        Ok(plan)
    }
}
```

Analyzer rule policy:

```text id="l5ix22"
Use analyzer rule when:
  plan is not executable/valid until rewrite occurs.
  type coercion or semantic resolution is required.
  rewrite is part of semantic interpretation, not optimization.

Do not use analyzer rule for:
  optional performance rewrites
  provider pushdown
  physical properties
```

---

## 48.15 OptimizerConfig and execution props

`OptimizerConfig` exposes query-start time, alias generator, and other optimizer-time configuration; docs state that `query_execution_start_time` is used for `now()`, and if absent, time-dependent functions like `now()` are not simplified during optimization. ([Docs.rs][7])

Rule implications:

```text id="esxkcc"
Constant folding now():
  only if query_execution_start_time is Some.

Generated aliases:
  use config alias generator when rule needs unique aliases.

Config options:
  consult config flags instead of hard-coded behavior.

Session time zone:
  timestamp/time/date simplification may depend on config.
```

Agent policy:

```text id="9a596y"
Rule behavior must be deterministic for same plan + same OptimizerConfig.
Include relevant config in golden test fixtures.
```

---

## 48.16 Rule-order hazards

Examples:

```text id="coknv9"
Projection pushdown before expression simplification:
  may push complex expression dependencies; simplification first might reduce dependency set.

Predicate pushdown before function rewrite:
  provider may not recognize function form; function rewrite first may expose pushdown-compatible predicate.

Join reorder before filter pushdown:
  filters may reduce cardinality estimates and change join order desirability.

Alias stripping before projection optimization:
  can break output schema expectations.

Subquery decorrelation before predicate simplification:
  can expose join predicates; order affects success.

Limit pushdown before sort rewrite:
  may be unsafe unless order semantics are clear.
```

Rule-order test strategy:

```text id="u3u0s5"
Test each rule alone.
Test rule before/after adjacent built-ins.
Test full optimizer rule chain.
Snapshot optimized logical plan.
Compare results with optimizer disabled / rule disabled.
```

---

## 48.17 Testing rule idempotence

```rust id="nzoijj"
pub fn assert_rule_idempotent(
    rule: &dyn OptimizerRule,
    plan: LogicalPlan,
    config: &dyn OptimizerConfig,
) -> Result<()> {
    let first = rule.rewrite(plan, config)?.data;
    let second = rule.rewrite(first.clone(), config)?;

    if second.transformed {
        return Err(datafusion::common::DataFusionError::Internal(format!(
            "rule `{}` is not idempotent; second application changed plan",
            rule.name()
        )));
    }

    Ok(())
}
```

If `Transformed` field names differ in the pinned version, adapt to its exact API. The semantic invariant remains mandatory because the optimizer repeatedly calls `rewrite` until fixed point. ([Docs.rs][4])

---

## 48.18 Testing schema stability

```rust id="wkq1fr"
pub fn assert_schema_stable(
    before: &LogicalPlan,
    after: &LogicalPlan,
    rule_name: &str,
) -> Result<()> {
    if before.schema() != after.schema() {
        return Err(datafusion::common::DataFusionError::Internal(format!(
            "rule `{rule_name}` changed schema\nbefore={:?}\nafter={:?}",
            before.schema(),
            after.schema(),
        )));
    }

    Ok(())
}
```

Test dimensions:

```text id="nf18j6"
field count
field order
field name
qualifier
data type
nullability
metadata
functional dependencies if rule relies on them
```

---

## 48.19 Testing result equivalence

```rust id="c7i6xk"
#[tokio::test]
async fn custom_rule_preserves_results() -> Result<()> {
    let ctx_without = context_without_rule().await?;
    let ctx_with = context_with_rule().await?;

    let sql = "
      SELECT customer_id, SUM(amount) AS total_amount
      FROM orders
      WHERE amount > 0
      GROUP BY customer_id
      ORDER BY customer_id
    ";

    let a = ctx_without.sql(sql).await?.collect().await?;
    let b = ctx_with.sql(sql).await?.collect().await?;

    assert_batches_eq_sorted(&a, &b)?;

    Ok(())
}
```

Result-equivalence policy:

```text id="0n5c2v"
Use deterministic ORDER BY when comparing rows.
Use sorted batch comparison when order not semantically relevant.
Use approximate comparison for floats.
Compare schema separately.
Test nulls, empty inputs, single-row, multi-partition.
```

---

## 48.20 Negative-case tests

```text id="3gi8s7"
Expression rewrite:
  volatile expression not rewritten
  alias preserved
  type mismatch not introduced
  NULL semantics preserved

Plan-node rewrite:
  unsupported node returns unchanged
  schema-changing variant rejected
  extension version mismatch returns unchanged

Provider-aware rewrite:
  unknown stats does not rewrite
  inexact pushdown keeps residual filter
  denied provider metadata does not leak

Join rewrite:
  cross join without predicate unchanged/denied elsewhere
  OR predicate semantics preserved
  outer join null semantics preserved
```

Example:

```rust id="jpfgrl"
#[test]
fn rule_does_not_rewrite_volatile_expr() -> Result<()> {
    let expr = /* random() or volatile UDF expression */;
    assert!(expr.is_volatile());

    // Apply rule and assert no transformation.
    Ok(())
}
```

---

## 48.21 Built-in rewrite category map

DataFusion’s listed logical optimizer implementors include common rewrite families such as common subexpression elimination, lateral/predicate subquery decorrelation, cross-join elimination, duplicated-expression elimination, filter elimination, group-by-constant elimination, join/limit/outer-join elimination, equijoin predicate extraction, projection optimization, union optimization, filter pushdown, limit pushdown, distinct-to-aggregate replacement, set-comparison rewrite, scalar-subquery-to-join, expression simplification, and single-distinct-to-group-by. ([Docs.rs][4])

| Category                  | Representative built-in-style rewrite                     | Custom-rule design lesson                         |
| ------------------------- | --------------------------------------------------------- | ------------------------------------------------- |
| expression simplification | `SimplifyExpressions`                                     | use expression tree APIs; respect null/volatility |
| predicate pushdown        | `PushDownFilter`                                          | preserve residuals and input availability         |
| projection optimization   | `OptimizeProjections`, `PushDownLeafProjections`          | track dependencies/aliases                        |
| join rewrite              | `EliminateCrossJoin`, `ExtractEquijoinPredicate`          | preserve join/null semantics                      |
| subquery decorrelation    | `DecorrelatePredicateSubquery`, `ScalarSubqueryToJoin`    | maintain duplicate/null/correlation semantics     |
| aggregate rewrite         | `ReplaceDistinctWithAggregate`, `SingleDistinctToGroupBy` | preserve aggregate modifiers/output names         |
| limit rewrite             | `PushDownLimit`, `EliminateLimit`                         | preserve order/cardinality semantics              |
| empty relation            | `PropagateEmptyRelation`                                  | schema must remain valid even with no rows        |

---

## 48.22 Deployment patterns

### 48.22.1 Safe extension in app engine

```rust id="p7t646"
pub fn build_engine_with_rules() -> Result<SessionContext> {
    let state = SessionStateBuilder::new()
        .with_default_features()
        .with_analyzer_rule(Arc::new(DomainAnalyzerRule))
        .with_optimizer_rule(Arc::new(MyOptimizerRule))
        .build();

    Ok(SessionContext::new_with_state(state))
}
```

### 48.22.2 Rule-gated environments

```text id="kdp2uv"
Development:
  enable rule
  verbose plan snapshots
  result-equivalence tests

Staging:
  enable rule behind config flag
  compare optimized plan with baseline
  record metrics deltas

Production:
  enable rule for allowed tenants/workloads
  emit rule-fire counters
  capture before/after plan hashes
  rollback flag available
```

### 48.22.3 Rule observability

```rust id="ybwgyh"
#[derive(Debug, Clone)]
pub struct RuleAuditEvent {
    pub rule_name: String,
    pub transformed: bool,
    pub before_plan_hash: String,
    pub after_plan_hash: String,
    pub before_schema_hash: String,
    pub after_schema_hash: String,
    pub warnings: Vec<String>,
}
```

---

## 48.23 Best-practice advisory

```text id="yklhao"
AnalyzerRule:
  use for validity-enabling rewrites.
  keep small and deterministic.
  fail with actionable errors.
  do not hide invalid user semantics by over-rewriting.

OptimizerRule:
  preserve semantics.
  preserve schema unless explicitly documented.
  preserve aliases/expression names.
  use ApplyOrder when rule is local.
  return Transformed::no when unchanged.
  make rule idempotent.
  avoid function-name-specific semantics.
  respect volatility.
  validate null semantics.
  write result-equivalence tests.

Provider-aware rule:
  trust exact metadata only for correctness.
  treat unknown/inexact conservatively.
  never drop residual filters unless pushdown is exact.

Extension-node rule:
  version custom node semantics.
  downcast safely.
  preserve extension schema.
  verify physical planner compatibility.
```

---

## 48.24 Anti-pattern inventory

* Using `OptimizerRule` to change query semantics.
* Using `AnalyzerRule` for optional performance tuning.
* Returning `Transformed::yes` when plan is unchanged.
* Alternating rewrites that never reach fixed point.
* Generating a fresh alias every optimizer pass.
* Stripping aliases from public projection expressions.
* Comparing function names such as `"sum"` as semantic proof.
* Rewriting volatile expressions by duplication/pushdown/folding.
* Ignoring SQL three-valued null logic.
* Assuming exact stats/constraints when metadata is inexact.
* Removing residual filters after inexact provider pushdown.
* Rewriting joins without outer/null semantics tests.
* Snapshotting only plan strings and not result equivalence.
* Testing rule alone but not in full rule chain.
* Failing open on unknown extension nodes.
* Version-upgrading DataFusion without refreshing rule snapshots.

---

## 48.25 Agent checklist

```text id="56lt6i"
[ ] Classify rule:
    AnalyzerRule | OptimizerRule | PhysicalOptimizerRule | SQL planner hook

[ ] AnalyzerRule if:
    validity-enabling
    type coercion / semantic normalization
    function rewrite before analysis

[ ] OptimizerRule if:
    valid input plan
    same output semantics
    improved cost / simpler logical shape

[ ] Implement:
    name()
    apply_order() if local recursion
    rewrite() with Transformed::yes/no
    analyze() for AnalyzerRule

[ ] Prove preconditions:
    schema known
    expression types known
    no unsafe volatile expressions
    metadata exact/fresh if needed
    extension node version matched

[ ] Preserve invariants:
    schema
    field names
    aliases
    qualifiers
    null semantics
    function semantics
    statistics invalidated or updated

[ ] Common rewrite patterns:
    predicate pushdown
    projection pushdown
    limit pushdown
    expression simplification
    subquery decorrelation
    join rewrite/reorder
    aggregate simplification

[ ] Test:
    input plan snapshot
    optimized plan snapshot
    idempotence
    result equivalence
    negative cases
    alias/name stability
    rule-order hazards
    full optimizer-chain integration

[ ] Deploy:
    register through SessionStateBuilder
    default features retained unless intentionally replaced
    rule behind config/feature flag when risky
    emit rule audit metrics
    pin DataFusion version for exact API/rule-order behavior
```

[1]: https://docs.rs/datafusion/latest/datafusion/optimizer/analyzer/trait.AnalyzerRule.html "AnalyzerRule in datafusion::optimizer::analyzer - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/optimizer/struct.Analyzer.html "Analyzer in datafusion::optimizer - Rust"
[3]: https://datafusion.apache.org/library-user-guide/query-optimizer.html "Query Optimizer — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/optimizer/trait.OptimizerRule.html "OptimizerRule in datafusion::optimizer - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/optimizer/enum.ApplyOrder.html "ApplyOrder in datafusion::optimizer - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/execution/session_state/struct.SessionStateBuilder.html "SessionStateBuilder in datafusion::execution::session_state - Rust"
[7]: https://docs.rs/crate/datafusion-optimizer/latest/target-redirect/datafusion_optimizer/optimizer/trait.OptimizerConfig.html?utm_source=chatgpt.com "OptimizerConfig in datafusion_optimizer::optimizer - Rust"


## 48.26 TreeNode rewriting note (DF 54): `Default` bound on `Box`/`Arc` containers

The `TreeNodeContainer` implementations for `Box<C>` and `Arc<C>` gained a `C: Default` bound in DataFusion 54 (`datafusion-common/src/tree_node.rs`):

```rust id="tnc54"
impl<'a, T: 'a, C: TreeNodeContainer<'a, T> + Default> TreeNodeContainer<'a, T> for Box<C> { ... }
impl<'a, T: 'a, C: TreeNodeContainer<'a, T> + Clone + Default> TreeNodeContainer<'a, T> for Arc<C> { ... }
```

`map_elements` now rewrites in place: it `mem::take`s the inner value out of the existing heap allocation (leaving `C::default()` in the slot so an unwinding drop still finds a valid value), maps it, and writes the result back — reusing the allocation instead of re-boxing. The `Default` value is a throwaway placeholder that is never observed by rule code, but it must exist and be cheap (no allocation).

Effect on rule authors: any type you store in `Box`/`Arc` positions of custom tree nodes that flow through `map_elements`/`transform` must implement `Default`. Read-only traversal (`apply`/visitors) is unaffected.

---

# DataFusion Advanced — 49) Physical planning and logical-to-physical lowering map

## 49.0 Purpose

Document the compiler boundary where a semantic `LogicalPlan` becomes a runnable `ExecutionPlan`:

```text id="ir0iot"
optimized LogicalPlan
  → SessionState::create_physical_plan
  → QueryPlanner
  → PhysicalPlanner
  → logical-node-specific lowering
  → ExecutionPlan DAG
  → PhysicalOptimizerRule chain
  → optimized ExecutionPlan
  → ExecutionPlan::execute(partition, TaskContext)
  → SendableRecordBatchStream
  → Arrow RecordBatch
```

The attached document already frames physical planning as the point where `LogicalPlan` becomes `ExecutionPlan`, and distinguishes logical plans as semantic/operator trees from physical plans as concrete executable operator DAGs.  DataFusion’s `ExecutionPlan` docs define physical plans as executable nodes whose `execute` method produces an async stream of `RecordBatch` values for one output partition. ([Docs.rs][1])

---

## 49.1 Physical planning mental model

```text id="sxkeff"
LogicalPlan:
  what computation means
  relation algebra
  schema-aware
  optimizer target
  not executable

ExecutionPlan:
  how computation runs
  concrete algorithms
  physical expressions
  partitioning/order properties
  memory/spill/resource behavior
  executable through execute(partition, TaskContext)
```

Logical plans cannot be directly executed; DataFusion’s logical-plan guide states they must be compiled into `ExecutionPlan`s, which contain more concrete details such as algorithms and physical optimizations. ([Apache DataFusion][2]) `PhysicalPlanner` is the trait-level API for converting a `LogicalPlan` into an executable `ExecutionPlan`. ([Docs.rs][3])

---

## 49.2 `SessionState::create_physical_plan`

Primary explicit compile API:

```rust id="k4d9vz"
use datafusion::prelude::*;
use datafusion::physical_plan::ExecutionPlan;
use std::sync::Arc;

pub async fn logical_to_physical(
    ctx: &SessionContext,
    logical: &datafusion::logical_expr::LogicalPlan,
) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
    let state = ctx.state();
    state.create_physical_plan(logical).await
}
```

Convenience pipeline:

```text id="ue6a4j"
ctx.sql(...)
  → DataFrame
  → df.collect / show / execute_stream
      internally:
        logical optimize
        physical plan
        physical optimize
        execute

explicit:
  state.create_logical_plan(sql)
  state.optimize(plan)
  state.create_physical_plan(plan)
  physical.execute(partition, task_ctx)
```

Deployment rule:

```text id="cp4bpn"
Use DataFrame actions for normal execution.
Use SessionState::create_physical_plan for diagnostics, tests, custom execution, or physical-plan inspection.
Use custom QueryPlanner / ExtensionPlanner when default lowering cannot handle custom nodes.
```

---

## 49.3 `PhysicalPlanner`

Trait-level role:

```rust id="yghgui"
use std::sync::Arc;
use datafusion::common::Result;
use datafusion::execution::session_state::SessionState;
use datafusion::logical_expr::{Expr, LogicalPlan};
use datafusion::physical_plan::{ExecutionPlan, PhysicalExpr};

#[async_trait::async_trait]
pub trait PhysicalPlanner {
    async fn create_physical_plan(
        &self,
        logical_plan: &LogicalPlan,
        session_state: &SessionState,
    ) -> Result<Arc<dyn ExecutionPlan>>;

    fn create_physical_expr(
        &self,
        expr: &Expr,
        input_dfschema: &datafusion::common::DFSchema,
        execution_props: &datafusion::execution::context::ExecutionProps,
    ) -> Result<Arc<dyn PhysicalExpr>>;
}
```

`PhysicalPlanner` converts logical plans to physical plans and logical expressions to physical expressions; it also exposes specialized physical expression APIs for sorting, aggregations, windows, and groups under the pinned DataFusion API. ([Docs.rs][3])

Agent rules:

```text id="fe1x9t"
PhysicalPlanner owns lowering.
It does not execute rows.
It must choose concrete exec nodes and physical expressions.
It must respect schema, ordering, distribution, boundedness, config, and provider contracts.
```

---

## 49.4 `QueryPlanner`

`QueryPlanner` is the session-level planner hook that DataFusion calls to create a physical plan. Use it when the default planner must be replaced or extended globally.

```rust id="q7m4ux"
use std::sync::Arc;
use datafusion::common::Result;
use datafusion::execution::context::QueryPlanner;
use datafusion::execution::session_state::SessionState;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct MyQueryPlanner {
    fallback: Arc<dyn QueryPlanner + Send + Sync>,
}

#[async_trait::async_trait]
impl QueryPlanner for MyQueryPlanner {
    async fn create_physical_plan(
        &self,
        logical_plan: &LogicalPlan,
        session_state: &SessionState,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        match logical_plan {
            LogicalPlan::Extension(ext) if ext.node.name() == "MyNode" => {
                // custom lowering
                todo!("return custom ExecutionPlan")
            }
            _ => self
                .fallback
                .create_physical_plan(logical_plan, session_state)
                .await,
        }
    }
}
```

Use `QueryPlanner` for:

```text id="ma5wtu"
global physical-planning control
custom logical extension lowering
planner replacement for a domain engine
inserting custom physical operators
enforcing physical-plan policy before execution
```

Avoid when:

```text id="fwc2yd"
a normal UDF is enough
a TableProvider scan implementation is enough
an optimizer rule is enough
a physical optimizer rule is enough
```

---

## 49.5 Logical-to-physical mapping table

| Logical node   | Typical physical lowering                                                                           | Key physical concerns                                                           |
| -------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| `TableScan`    | `DataSourceExec`, `MemoryExec`-like source, custom scan exec                                        | projection, filters, limit, file groups, object store, statistics, partitioning |
| `Projection`   | `ProjectionExec`                                                                                    | physical expressions, alias/output schema, order preservation                   |
| `Filter`       | `FilterExec`                                                                                        | boolean physical predicate, null semantics, order preservation                  |
| `Aggregate`    | aggregate exec variants                                                                             | partial/final aggregation, grouping, spill, repartition requirements            |
| `Sort`         | `SortExec`, top-k/sort-preserving variants                                                          | ordering, memory/spill, global vs local order                                   |
| `Limit`        | `GlobalLimitExec`, local/global limit stages                                                        | offset/fetch, partition interaction, top-k                                      |
| `Join`         | `HashJoinExec`, `SortMergeJoinExec`, `NestedLoopJoinExec`, `SymmetricHashJoinExec`, cross join exec | join type, join keys, build/probe, sort/order requirements, memory/spill        |
| `Union`        | `UnionExec`                                                                                         | schema equality, partition concatenation, order preservation per input          |
| `Window`       | `WindowAggExec` / bounded-window variants                                                           | partition/order requirements, frame semantics, memory                           |
| `Repartition`  | `RepartitionExec`                                                                                   | hash/round-robin/unknown partitioning, target partitions, order preservation    |
| `Extension`    | custom `ExecutionPlan` through extension/query planner                                              | node-specific lowering, schema/properties, custom stream                        |
| `Dml` / `Copy` | `DataSinkExec` / writer exec path                                                                   | sink schema, commit semantics, row count output                                 |
| `Unnest`       | unnest physical exec                                                                                | cardinality expansion, nested-list handling                                     |

`DataSourceExec` is DataFusion’s common file/data-source scan execution plan and applies projections while caching plan properties; `ProjectionExec` and projection modules define projection execution; `FilterExec` evaluates a boolean predicate against input batches; `UnionExec` concatenates same-schema input partitions without mixing/copying data across partitions. ([Docs.rs][4])

---

## 49.6 `ExecutionPlan` contract

Current conceptual shape:

```rust id="e7gjsb"
use std::sync::Arc;
use datafusion::common::Result;
use datafusion::execution::TaskContext;
use datafusion::physical_plan::{
    ExecutionPlan,
    SendableRecordBatchStream,
    execution_plan::PlanProperties,
};

pub trait ExecutionPlan: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &str;

    fn properties(&self) -> &Arc<PlanProperties>;

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>>;

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>>;

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream>;
}
```

Implementation invariants:

```text id="3pkxmr"
execute(partition, context):
  returns stream for exactly one output partition
  should not eagerly compute full result
  must emit RecordBatch values matching schema()
  must respect memory reservations / spill policy
  must surface metrics/errors
  must honor TaskContext runtime resources

children():
  leaf scan → empty
  unary op → one child
  binary join → two children
  n-ary union → multiple children

with_new_children():
  required for physical optimizer tree rewrites
  must preserve operator semantics
  must validate child count/order/schema

properties():
  must accurately describe output partitioning/order/boundedness/emission
```

DataFusion’s `ExecutionPlan` docs state that `execute` returns a stream that incrementally computes one partition, while `schema` and `properties` communicate metadata to the optimizer; child and input-requirement methods support plan-tree rewrites and automatic repartitioning. ([Docs.rs][1])

---

## 49.7 Physical expression planning

Logical expressions embedded inside logical plans must be lowered to executable `PhysicalExpr`s.

```text id="oatgnp"
Expr:
  col("amount") * lit(1.08)

PhysicalExpr:
  Column(index=...)
  Literal(ScalarValue::Float64(1.08))
  BinaryExpr(left, op=Multiply, right)
```

Example:

```rust id="8q1x13"
use datafusion::execution::context::ExecutionProps;
use datafusion::physical_expr::create_physical_expr;
use datafusion::prelude::*;

pub fn lower_expr_for_schema(
    expr: &Expr,
    schema: &datafusion::common::DFSchema,
) -> datafusion::error::Result<std::sync::Arc<dyn datafusion::physical_plan::PhysicalExpr>> {
    let props = ExecutionProps::new();
    create_physical_expr(expr, schema, &props)
}
```

Physical expression responsibilities:

```text id="87clbc"
resolve logical column refs to physical column indices
lower literals to ScalarValue
lower casts to physical cast expressions
lower scalar functions to physical implementations
lower sort expressions to physical sort expressions
produce type/nullability metadata
evaluate against RecordBatch
return ColumnarValue
```

`PhysicalExpr`s are the physical counterpart of logical `Expr`s; they know their type and nullability and can be evaluated directly on a `RecordBatch`. ([Docs.rs][5])

---

## 49.8 `TableScan` → `DataSourceExec` / custom scan exec

Logical:

```text id="0gbpbu"
TableScan {
  table_name,
  source,
  projection,
  filters,
  fetch,
}
```

Physical:

```text id="8cz4t3"
DataSourceExec
  ├─ source: files / object store / memory / provider-specific source
  ├─ projection
  ├─ file groups / partitions
  ├─ pushed predicates / residual predicate handling
  ├─ statistics / plan properties
  └─ execute(partition) → stream of RecordBatch
```

For file scans, `FileScanConfig` is used to create `DataSourceExec`; it represents scanning data from groups of files with a particular file format. ([Docs.rs][6])

Provider-sensitive choices:

```text id="169d74"
TableProvider::scan receives:
  projection
  filters
  limit

Provider may:
  return built-in DataSourceExec
  return custom ExecutionPlan
  perform exact filter pushdown
  perform inexact filter pushdown + require residual filter
  expose natural partitions
  expose ordering/partitioning if guaranteed
```

Agent rules:

```text id="bi4sim"
TableScan lowering is provider-owned.
Do not assume all scans become file DataSourceExec.
Custom TableProvider may return any valid ExecutionPlan.
Projection/filter/limit pushdown correctness is provider contract.
```

---

## 49.9 `Projection` → `ProjectionExec`

Logical:

```rust id="o2q1e6"
df.select(vec![
    col("customer_id"),
    (col("amount") * lit(1.08_f64)).alias("gross_amount"),
])?;
```

Physical:

```text id="ro1ku2"
ProjectionExec
  input ExecutionPlan
  physical expressions:
    Column(customer_id_idx)
    BinaryExpr(Column(amount_idx), *, Literal(1.08))
  output schema:
    customer_id
    gross_amount
```

Planning duties:

```text id="04acdz"
lower each Expr to PhysicalExpr
preserve aliases/output field names
compute output Arrow Schema
preserve input order if projection is row-preserving
preserve partitioning if projection does not change row count
update equivalence/order properties where expressions preserve/rename keys
```

Projection defines which columns or expressions are returned by a query; the DataFusion physical-plan docs describe projection as returning expressions such as `a`, `b`, and `a+b` from an input table. ([Docs.rs][7])

---

## 49.10 `Filter` → `FilterExec`

Logical:

```rust id="8s3lcx"
df.filter(
    col("amount")
        .gt(lit(0.0))
        .and(col("status").eq(lit("paid")))
)?;
```

Physical:

```text id="88hxrc"
FilterExec
  input ExecutionPlan
  predicate PhysicalExpr -> BooleanArray / Boolean scalar
  emits rows where predicate is true
```

Planning duties:

```text id="v70hl2"
lower predicate Expr to PhysicalExpr
validate output type Boolean
preserve input schema
preserve partitioning
preserve ordering if row-relative order is retained
respect SQL null semantics: false/null rows filtered out
```

`FilterExec` evaluates a boolean predicate against all input batches and includes only rows passing the predicate in output batches. ([Docs.rs][8])

---

## 49.11 `Aggregate` → aggregate execution variants

Logical:

```rust id="w3cp19"
df.aggregate(
    vec![col("customer_id")],
    vec![sum(col("amount")).alias("total_amount")],
)?;
```

Physical lowering choices:

```text id="hy9ovy"
single-stage aggregate:
  AggregateExec over input
  often for small/single-partition cases

two-stage aggregate:
  partial aggregate per partition
  repartition by group keys
  final aggregate
  common for parallel grouped aggregations

global aggregate:
  may collapse to single final partition

spill-enabled aggregate:
  uses memory reservations and disk spilling when supported/configured
```

Planning duties:

```text id="p2p1ou"
lower group expressions
lower aggregate input expressions
create aggregate physical expressions/accumulators
choose partial/final mode
insert repartition if grouping distribution required
handle DISTINCT / FILTER / ORDER BY modifiers
compute output schema/nullability
configure spill/memory behavior
```

Config-sensitive choices:

```text id="o3tab5"
target_partitions
repartition_aggregations
batch_size
memory limits
spill settings
distinct aggregate optimizer rewrites
```

Agent rules:

```text id="1ofz3g"
Do not assume one Aggregate logical node maps to one physical node.
Expect partial/final aggregate stages.
Expect repartition around grouped aggregates.
Inspect physical plan for actual aggregate topology.
```

---

## 49.12 `Sort` → `SortExec` / sort-preserving variants

Logical:

```rust id="kgaax2"
df.sort(vec![
    col("event_ts").sort(false, false),
])?;
```

Physical:

```text id="212baz"
SortExec
  input ExecutionPlan
  physical sort expressions
  local/global sort semantics depending surrounding operators
  may spill to disk
```

`SortExec` supports sorting datasets larger than memory by spilling to disk. ([Docs.rs][9])

Planning duties:

```text id="3bve4c"
lower SortExpr to physical sort expr
check child output_ordering
skip sort if ordering already satisfied
insert merge/sort-preserving operators if global order required
respect null ordering
configure spill/memory
```

Ordering risk:

```text id="r2najw"
False output_ordering can remove required SortExec.
Missing output_ordering can insert unnecessary SortExec.
Order is local per partition unless a merge/global operator proves otherwise.
```

---

## 49.13 `Limit` → local/global limit execution

Logical:

```rust id="p6hgcs"
df.limit(0, Some(100))?;
```

Physical patterns:

```text id="icr62k"
LocalLimitExec:
  limit per partition
  reduces rows before global coordination

GlobalLimitExec:
  applies final offset/fetch across combined partitions

Sort + Limit:
  may become top-k/sort-with-fetch plan
```

`GlobalLimitExec` is a physical limit operator whose docs follow the standard `ExecutionPlan` child semantics; DataFusion also has limit streams and limit physical-plan components under the limit module. ([Docs.rs][10])

Planning duties:

```text id="5y9oo4"
preserve skip/fetch semantics
insert local limit opportunistically where safe
ensure global final limit for multi-partition correctness
avoid pushing limit through operators where row choice semantics change
combine with sort/top-k when ORDER BY + LIMIT
```

Agent rules:

```text id="ghdokf"
LIMIT without ORDER BY caps rows but not deterministic row identity.
ORDER BY + LIMIT requires preserving order semantics.
Do not assume logical Limit maps to only one physical node.
```

---

## 49.14 `Join` → join exec variants

Logical join inputs:

```text id="ixy4kh"
join_type
left/right input plans
equi keys
optional non-equi filter
null equality semantics
output schema
```

Physical candidates:

| Physical join           | Use case                                   | Key requirements                                  |
| ----------------------- | ------------------------------------------ | ------------------------------------------------- |
| `HashJoinExec`          | equijoins, in-memory/build-side hash       | hashable keys, build/probe memory, repartitioning |
| `SortMergeJoinExec`     | equijoins over sorted inputs / large joins | compatible ordering on join keys                  |
| `NestedLoopJoinExec`    | non-equi joins / small inputs / fallback   | no equi-key requirement, potentially expensive    |
| `SymmetricHashJoinExec` | streaming/range-like/symmetric hash cases  | stream semantics and supported join shape         |
| cross join exec         | Cartesian product                          | explicit cross join / no predicate                |

DataFusion’s join module documents `HashJoinExec` as evaluating equijoin predicates in parallel using a hash table plus optional post-join filter, and `SortMergeJoinExec` as executing equijoins using sort-merge join and supporting cases where one or both inputs do not fit in memory. ([Docs.rs][11])

Planning duties:

```text id="vl42lq"
extract equijoin keys
lower join filter to PhysicalExpr
choose algorithm
choose build/probe side when relevant
insert repartition for hash join distribution
insert sort or exploit ordering for sort-merge join
handle join type output nullability
preserve semi/anti output schema semantics
configure memory/spill where supported
```

Config/provider-sensitive choices:

```text id="1yyifg"
optimizer join flags
target_partitions
available statistics
known ordering
known partitioning
memory limit
provider-provided sorted/partitioned output
join key data types
unbounded vs bounded inputs
```

Agent rules:

```text id="941esa"
Logical Join does not specify algorithm.
Inspect physical plan for HashJoinExec / SortMergeJoinExec / NestedLoopJoinExec / SymmetricHashJoinExec.
Join algorithm may change across config/version/statistics.
```

---

## 49.15 `Union` → `UnionExec`

Logical:

```rust id="9lgs9p"
let df = df1.union(df2)?;
```

Physical:

```text id="ayqidm"
UnionExec
  children: multiple ExecutionPlans with same schema
  output partitions: concatenation of child partitions
  no row mixing/copying across partitions
```

`UnionExec` is DataFusion’s `UNION ALL` physical plan; it combines same-schema inputs by concatenating partitions without mixing or copying data within/across partitions, and if input partitions are sorted, output partitions are also sorted. ([Docs.rs][12])

Planning duties:

```text id="wabl0o"
validate same physical schema
preserve partitioning/order properties where valid
avoid unnecessary materialization
handle distinct union separately through aggregate/distinct physical plans
```

---

## 49.16 `Window` → window execution

Logical:

```text id="yx6xqn"
Window:
  input
  window expressions:
    function(args) OVER (
      PARTITION BY exprs
      ORDER BY sort exprs
      frame
    )
```

Physical:

```text id="zyw1mx"
WindowAggExec / bounded-window variants
  partition/order physical expressions
  window frame evaluator
  input ordering/distribution requirements
  output schema = input columns + window result columns
```

Planning duties:

```text id="1nmz0q"
lower partition expressions
lower order expressions
lower window function implementation
insert repartition by PARTITION BY if required
insert sort by PARTITION BY + ORDER BY if required
handle frame memory
preserve alias/output names
```

Agent rules:

```text id="mmk66r"
Window physical plans are order/distribution-sensitive.
Ranking functions require deterministic ordering for stable results.
Window planning may insert repartition and sort even if logical plan shows only Window.
```

---

## 49.17 `Repartition` → `RepartitionExec`

Logical repartition or optimizer-inserted physical exchange:

```text id="kf6haz"
RepartitionExec
  input partitions N
  output partitions M
  scheme:
    round-robin
    hash(exprs)
    unknown
  optional order preservation behavior
```

`RepartitionExec` redistributes `RecordBatch`es from N input partitions to M output partitions based on a partitioning scheme; DataFusion uses exchange-style operators for parallelism. ([Docs.rs][13]) `Partitioning` includes `RoundRobinBatch`, `Hash`, and `UnknownPartitioning`, and output partitions are executed independently. ([Docs.rs][13])

Planning duties:

```text id="ib3zwh"
choose target partition count
hash repartition on join/group keys
round-robin repartition for load balancing
avoid repartition if child already satisfies distribution
preserve or invalidate ordering correctly
```

Config-sensitive choices:

```text id="a1c78g"
datafusion.execution.target_partitions
optimizer repartition flags
join/aggregate requirements
input partition count
known child partitioning
```

Since DataFusion 54, `RepartitionExec` coalesces small batches before handing them to the per-output distributor channels (a `LimitedBatchCoalescer` per output), so hash-repartitioned streams emit fewer, fuller batches; preserve-order mode skips the shared coalescer.

---

## 49.18 `Extension` → custom exec

Logical extension:

```rust id="7kgq6m"
LogicalPlan::Extension(extension_node)
```

Required physical lowering:

```text id="mth0kp"
logical extension node
  → extension planner / custom QueryPlanner
  → custom ExecutionPlan
  → custom RecordBatchStream
```

Extension lowering skeleton:

```rust id="vk4rd7"
#[derive(Debug)]
pub struct MyExec {
    input: Arc<dyn ExecutionPlan>,
    props: Arc<datafusion::physical_plan::execution_plan::PlanProperties>,
}

impl ExecutionPlan for MyExec {
    fn name(&self) -> &str {
        "MyExec"
    }

    fn properties(&self) -> &Arc<datafusion::physical_plan::execution_plan::PlanProperties> {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![&self.input]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> datafusion::common::Result<Arc<dyn ExecutionPlan>> {
        if children.len() != 1 {
            return Err(datafusion::common::DataFusionError::Internal(
                "MyExec expects one child".to_string(),
            ));
        }

        Ok(Arc::new(Self {
            input: children.into_iter().next().unwrap(),
            props: self.props.clone(), // recompute if child properties affect output
        }))
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<datafusion::execution::TaskContext>,
    ) -> datafusion::common::Result<datafusion::physical_plan::SendableRecordBatchStream> {
        let input_stream = self.input.execute(partition, context)?;
        todo!("wrap input_stream with custom RecordBatchStream")
    }
}
```

Agent rules:

```text id="4j8qdk"
LogicalPlan::Extension is only representational until physical lowering exists.
Custom ExecutionPlan must report accurate schema/properties.
Custom stream must emit batches matching schema.
Custom operator must implement memory/metrics/error behavior deliberately.
```

---

## 49.19 Physical properties: partitioning, ordering, boundedness, emission

```text id="dy4zav"
PlanProperties
  ├─ EquivalenceProperties
  ├─ output Partitioning
  ├─ EmissionType
  └─ Boundedness
```

`PlanProperties` caches physical-plan metadata used by query optimization; `ExecutionPlanProperties` exposes `output_partitioning`, `output_ordering`, `boundedness`, `pipeline_behavior`, and `equivalence_properties`. ([Docs.rs][14])

Property policy:

```text id="yapzf4"
Accurate partitioning:
  enables/removes repartition.

Accurate ordering:
  enables/removes sort and merge operators.

Accurate boundedness:
  prevents unsupported unbounded query execution.

Accurate emission type:
  informs pipeline/blocking behavior.

False stronger property:
  correctness bug.

Unknown/weaker property:
  possible performance cost, usually safe.
```

---

## 49.20 Distribution requirements

Physical operators can require child distributions.

Examples:

```text id="pb7vtn"
HashJoinExec:
  both sides distributed by join keys, or planner inserts repartition.

Final aggregate:
  input distributed by group keys or collapsed to final stage.

Window PARTITION BY:
  input distributed by partition keys.

Global limit/sort:
  may require merge/single-partition/global coordination.
```

Implementation surfaces:

```text id="2oe4r8"
ExecutionPlan::required_input_distribution()
ExecutionPlan::required_input_ordering()
ExecutionPlan::maintains_input_order()
Physical optimizer rules insert RepartitionExec / SortExec as needed.
```

DataFusion uses physical metadata to automatically repartition correctly, and docs warn that incorrect order-preservation claims can produce incorrect results. ([Docs.rs][1])

Agent rules:

```text id="sx169u"
Custom operators must declare required input distribution/order.
If child does not satisfy requirement, planner/optimizer must insert exchange/sort.
Never claim distribution/order without proof.
```

---

## 49.21 Ordering requirements

Ordering concepts:

```text id="blwpxk"
logical ORDER BY:
  user-visible output ordering requirement

physical output_ordering:
  local per-partition ordering guarantee

required_input_ordering:
  ordering child must provide for correctness/performance

maintains_input_order:
  whether operator preserves order from child to output
```

Common cases:

| Operator            | Ordering behavior                                             |
| ------------------- | ------------------------------------------------------------- |
| `ProjectionExec`    | often preserves row order                                     |
| `FilterExec`        | preserves relative row order                                  |
| `SortExec`          | creates ordering                                              |
| `RepartitionExec`   | may break ordering unless order-preserving mode applies       |
| `HashJoinExec`      | generally not order preserving                                |
| `SortMergeJoinExec` | depends on sorted inputs and join semantics                   |
| `UnionExec`         | preserves sorted input partitions under documented conditions |
| `AggregateExec`     | generally not user-order preserving                           |

DataFusion docs state `output_ordering()` is `Some(keys)` only when output within each partition is sorted; `None` means no ordering assumptions are allowed. ([Docs.rs][15])

---

## 49.22 Config-sensitive physical choices

Config categories:

```text id="m9uh6e"
parallelism:
  datafusion.execution.target_partitions

batching:
  datafusion.execution.batch_size

optimizer:
  repartition joins / aggregations / windows
  prefer hash join / sort merge flags where available
  enable top-k / limit pushdown where available

memory/spill:
  runtime memory limit
  spill directory
  spill compression
  max temp directory size

file formats:
  Parquet pruning
  page index
  bloom filters
  pushdown filters
  statistics collection

SQL/session:
  time zone
  string type mapping
```

Agent rules:

```text id="7169q8"
Physical plan is config-dependent.
Same LogicalPlan can lower differently under different ConfigOptions.
Plan snapshots must include DataFusion version + ConfigOptions.
```

---

## 49.23 Provider-sensitive physical choices

Provider affects physical plan through:

```text id="tdyr7c"
TableProvider::scan:
  provider returns physical scan plan

statistics:
  affects join/aggregate/sort choices

constraints:
  affects simplification and planning

supports_filters_pushdown:
  affects residual FilterExec vs provider-side filtering

known ordering:
  can eliminate SortExec

known partitioning:
  can eliminate RepartitionExec

file/object-store layout:
  affects DataSourceExec partitions

custom APIs:
  can expose indexed scans, paged scans, sharded scans, sorted scans
```

Agent provider policy:

```text id="v8hyse"
Provider should expose the strongest true metadata.
Provider must not overclaim pushdown/order/partitioning.
Provider-returned ExecutionPlan must obey its own properties.
```

---

## 49.24 Physical planning diagnostics

### Explicit physical plan creation

```rust id="t895nf"
use datafusion::physical_plan::displayable;

pub async fn print_physical_plan(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let physical = df.create_physical_plan().await?;

    println!("{}", displayable(physical.as_ref()).indent(true));

    Ok(())
}
```

### SQL EXPLAIN

```sql id="uiw2m5"
EXPLAIN VERBOSE
SELECT customer_id, SUM(amount) AS total_amount
FROM orders
WHERE amount > 0
GROUP BY customer_id
ORDER BY total_amount DESC
LIMIT 10;
```

`EXPLAIN` shows logical and physical execution plans for a statement, and `EXPLAIN ANALYZE` can include runtime metrics. ([Apache DataFusion][16])

Diagnostic bundle:

```text id="27hwmc"
logical plan
optimized logical plan
physical plan display
physical output schema
output partitioning
output ordering
required distributions
required orderings
operator metrics
config options
table/provider metadata
```

---

## 49.25 Error cases

### No physical implementation

```text id="sk1q4i"
Symptom:
  physical planner cannot lower logical node.

Typical causes:
  unsupported LogicalPlan variant
  feature-gated physical operator missing
  custom logical node without physical planner

Fix:
  rewrite logical plan to supported operators
  enable feature
  implement QueryPlanner / ExtensionPlanner / custom ExecutionPlan
```

### Unsupported extension node

```text id="y26uso"
Symptom:
  LogicalPlan::Extension survives to physical planning with no handler.

Fix:
  add custom QueryPlanner or extension planner
  validate extension node version/name
  return custom ExecutionPlan
```

### Missing sort requirement

```text id="9duvsw"
Symptom:
  operator requiring sorted input receives unsorted child.

Typical causes:
  wrong required_input_ordering implementation
  false output_ordering on child
  physical optimizer omitted SortExec

Fix:
  fix PlanProperties / required_input_ordering
  inspect EXPLAIN VERBOSE
  add tests for order-sensitive results
```

### Incompatible partitioning

```text id="oolh6q"
Symptom:
  join/aggregate/window receives incompatible distribution.

Typical causes:
  false output_partitioning
  missing RepartitionExec
  wrong hash expression equivalence
  custom exec overclaims partitioning

Fix:
  report UnknownPartitioning when unsure
  declare required_input_distribution
  let physical optimizer insert RepartitionExec
```

### Unsupported unbounded operator

```text id="4nw182"
Symptom:
  unbounded query cannot be physically planned/executed.

Typical causes:
  global sort
  blocking aggregate
  unsupported join/window over unbounded input

Fix:
  rewrite to streaming-safe query
  use bounded source
  materialize intermediate
  reject query at logical lint/admission stage
```

---

## 49.26 Physical lowering test matrix

```text id="4ei257"
Logical-to-physical:
  [ ] TableScan lowers to expected scan exec
  [ ] Projection lowers to projection exec with correct schema
  [ ] Filter lowers to FilterExec with Boolean predicate
  [ ] Aggregate produces expected partial/final topology
  [ ] Sort produces SortExec only when needed
  [ ] Limit produces correct local/global behavior
  [ ] Join algorithm matches expected config/statistics case
  [ ] Union preserves schema/partition semantics
  [ ] Window inserts required sort/repartition
  [ ] Repartition target partitions respected
  [ ] Extension lowers to custom exec

Properties:
  [ ] output_partitioning correct
  [ ] output_ordering correct
  [ ] boundedness correct
  [ ] maintains_input_order correct
  [ ] required_input_distribution correct
  [ ] required_input_ordering correct

Error:
  [ ] unsupported extension node rejected
  [ ] missing implementation error is actionable
  [ ] unbounded unsupported query rejected
  [ ] false property negative tests fail before production
```

---

## 49.27 Deployment advisory

```text id="018ugj"
Production services:
  inspect physical plans for expensive classes before rollout.
  stream outputs; avoid collect for user queries.
  configure RuntimeEnv memory/temp/spill.
  pin DataFusion version for physical-plan baselines.
  avoid relying on exact physical plan strings except in version-pinned tests.

Custom sources:
  implement TableProvider::scan carefully.
  return accurate ExecutionPlan properties.
  exploit projection/filter/limit pushdown where exact.
  use UnknownPartitioning/None ordering when unsure.

Custom operators:
  implement ExecutionPlan invariants.
  expose metrics.
  reserve memory through DataFusion memory APIs.
  preserve schema and properties.
  test with multi-partition execution.

Optimizer/physical-planner work:
  reason in terms of required distributions and orderings.
  inspect EXPLAIN VERBOSE.
  add property tests and result-equivalence tests.
```

---

## 49.28 Anti-pattern inventory

* Assuming `LogicalPlan` is executable.
* Inferring join algorithm from logical `Join`.
* Assuming one logical aggregate maps to one physical aggregate.
* Assuming `Limit` maps to a single physical node.
* Overclaiming custom exec `output_ordering`.
* Overclaiming custom exec `output_partitioning`.
* Forgetting `with_new_children` in custom `ExecutionPlan`.
* Eagerly computing all data in `execute`.
* Returning batches that do not match `schema`.
* Ignoring `TaskContext` memory/runtime resources.
* Implementing custom extension logical node without physical lowering.
* Snapshotting physical plan strings across version changes without pinning.
* Treating `target_partitions` as irrelevant to physical plan shape.
* Using `collect` as a production execution path for arbitrary queries.
* Dropping residual filters after inexact provider pushdown.

---

## 49.29 Agent checklist

```text id="4gxau8"
[ ] Compile explicitly:
    SessionState::create_physical_plan(logical_plan).await

[ ] Understand planner hooks:
    PhysicalPlanner for logical-to-physical conversion
    QueryPlanner for session-level planner replacement/customization

[ ] Map logical nodes:
    TableScan → DataSourceExec/custom scan
    Projection → ProjectionExec
    Filter → FilterExec
    Aggregate → aggregate exec stages
    Sort → SortExec/top-k/sort-preserving variants
    Limit → local/global limit
    Join → hash/sort-merge/nested-loop/symmetric/cross
    Union → UnionExec
    Window → window exec
    Repartition → RepartitionExec
    Extension → custom ExecutionPlan

[ ] Lower expressions:
    Expr → PhysicalExpr
    validate types/nullability
    preserve aliases/output schema

[ ] Validate physical properties:
    output_partitioning
    output_ordering
    boundedness
    emission/pipeline behavior
    required input distribution
    required input ordering

[ ] Account for dependencies:
    ConfigOptions
    target_partitions
    memory/spill config
    provider statistics
    provider ordering/partitioning
    pushdown contracts

[ ] Handle errors:
    no physical implementation
    unsupported extension node
    missing sort
    incompatible partitioning
    unsupported unbounded operator

[ ] Test:
    physical plan snapshot under pinned version
    result equivalence
    multi-partition execution
    property correctness
    negative false-ordering/partitioning cases
```

[1]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"
[2]: https://datafusion.apache.org/_sources/library-user-guide/building-logical-plans.md.txt?utm_source=chatgpt.com "building-logical-plans.md.txt - Apache DataFusion"
[3]: https://docs.rs/datafusion/latest/datafusion/physical_planner/trait.PhysicalPlanner.html?utm_source=chatgpt.com "PhysicalPlanner in datafusion::physical_planner - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/datasource/memory/struct.DataSourceExec.html?utm_source=chatgpt.com "DataSourceExec in datafusion::datasource::memory - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.PhysicalExpr.html?utm_source=chatgpt.com "PhysicalExpr in datafusion::physical_plan - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/datasource/physical_plan/struct.FileScanConfig.html?utm_source=chatgpt.com "FileScanConfig in datafusion::datasource::physical_plan"
[7]: https://docs.rs/datafusion/latest/datafusion/physical_plan/index.html?utm_source=chatgpt.com "datafusion::physical_plan - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_plan/filter/struct.FilterExec.html?utm_source=chatgpt.com "FilterExec in datafusion::physical_plan::filter - Rust"
[9]: https://docs.rs/datafusion/latest/datafusion/physical_plan/sorts/sort/struct.SortExec.html?utm_source=chatgpt.com "SortExec in datafusion::physical_plan::sorts::sort - Rust"
[10]: https://docs.rs/datafusion/latest/datafusion/physical_plan/limit/struct.GlobalLimitExec.html?utm_source=chatgpt.com "GlobalLimitExec in datafusion::physical_plan::limit - Rust"
[11]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/index.html?utm_source=chatgpt.com "datafusion::physical_plan::joins - Rust"
[12]: https://docs.rs/datafusion/latest/datafusion/physical_plan/union/struct.UnionExec.html?utm_source=chatgpt.com "UnionExec in datafusion::physical_plan::union - Rust"
[13]: https://docs.rs/datafusion/latest/datafusion/physical_plan/repartition/struct.RepartitionExec.html?utm_source=chatgpt.com "RepartitionExec in datafusion::physical_plan::repartition"
[14]: https://docs.rs/datafusion/latest/datafusion/physical_plan/execution_plan/struct.PlanProperties.html?utm_source=chatgpt.com "PlanProperties in datafusion::physical_plan::execution_plan"
[15]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlanProperties.html?utm_source=chatgpt.com "ExecutionPlanProperties in datafusion::physical_plan - Rust"
[16]: https://datafusion.apache.org/user-guide/sql/explain.html?utm_source=chatgpt.com "EXPLAIN — Apache DataFusion documentation"


## 49.30 Physical uncorrelated scalar subqueries: `ScalarSubqueryExec` + `ScalarSubqueryExpr` (DF 54)

### 49.30.1 What changed

Through DataFusion 53, uncorrelated scalar subqueries never reached physical planning — `ScalarSubqueryToJoin` rewrote them into left joins during logical optimization. DataFusion 54 keeps uncorrelated `Expr::ScalarSubquery` in the final logical plan and gives it a dedicated physical form:

```text id="pss54a"
ScalarSubqueryExec        datafusion_physical_plan::scalar_subquery
  plan node wrapping a main input plan + N subquery plans

ScalarSubqueryExpr        datafusion_physical_expr::scalar_subquery
  physical expression that reads a subquery's scalar result by index
```

Gated by `datafusion.optimizer.enable_physical_uncorrelated_scalar_subquery = true` (default on).

### 49.30.2 Execution model

```text id="pss54b"
ScalarSubqueryExec children:
  child 0    = main input plan (batches pass through unchanged)
  children 1..N = subquery plans

execution:
  when the first output partition is requested, all subquery plans are
  executed eagerly, each exactly once
  each result is stored in a shared ScalarSubqueryResults container
  (indexed by SubqueryIndex; the link struct is ScalarSubqueryLink { plan, index })
  ScalarSubqueryExpr instances embedded in the main input's expressions
  hold the same container and read their value by index

cardinality contract (enforced at runtime):
  0 rows  → typed NULL scalar (SQL semantics)
  1 row   → the scalar value
  >1 rows → execution error "Scalar subquery returned more than one row"
  ≠1 col  → execution error
```

From a results perspective `ScalarSubqueryExec` is a pass-through node: it yields exactly its main input's batches and exists only to populate subquery results as a side effect first.

### 49.30.3 Semantics and coverage gains over the join rewrite

```text id="pss54c"
correctness:
  the 53 join rewrite silently multiplied output rows when a scalar
  subquery returned multiple rows; 54 raises an execution error instead

new positions:
  scalar subqueries are now supported in ORDER BY, JOIN ON,
  and aggregate-function arguments — positions the join rewrite
  could not express

evaluation count:
  each uncorrelated subquery runs exactly once per query,
  shared across all referencing expressions and partitions
```

### 49.30.4 Escape hatch and tooling duties

Setting `enable_physical_uncorrelated_scalar_subquery = false` restores the `ScalarSubqueryToJoin` rewrite. The config docs describe it as a temporary escape hatch for distributed execution frameworks (planned for removal), and disabling re-inherits the old limitations above — do not disable it for correctness reasons.

```text id="pss54d"
Tooling checklist for the new plan shape:
[ ] physical-plan visitors and operator classifiers handle ScalarSubqueryExec
[ ] operator allowlists / plan lint policies (46.x) admit or explicitly
    deny the node
[ ] physical-expression walkers handle ScalarSubqueryExpr
[ ] plan serializers/codecs transport both (see 56.32)
[ ] EXPLAIN goldens re-baselined: subquery plans appear as extra children
    of ScalarSubqueryExec instead of a join subtree
```

---

## 49.31 Aggregate lowering: `LoweredAggregateBuilder` / `LoweredAggregate` (DF 54)

DataFusion 54 replaces the deprecated `create_aggregate_expr_*_maybe_filter`-style helpers with a builder at `datafusion_physical_expr::aggregate` (not re-exported at the crate root):

```rust id="lab54"
use datafusion_physical_expr::aggregate::{LoweredAggregate, LoweredAggregateBuilder};

let lowered: LoweredAggregate = LoweredAggregateBuilder::new(
    &agg_expr,               // logical aggregate Expr
    &logical_input_schema,   // DFSchema — resolves logical columns
    &physical_input_schema,  // Arrow Schema — input to the physical aggregate
    &execution_props,
)
.with_name("my_output_name")          // optional; defaults to alias/derived name
.with_human_display("count(*)")       // optional display-text override
.build()?;

// LoweredAggregate {
//     aggregate: Arc<AggregateFunctionExpr>,   // fun(), field(), human_display(),
//                                              // order_bys(), create_accumulator(), ...
//     filter: Option<Arc<dyn PhysicalExpr>>,   // FILTER (WHERE ...)
//     order_bys: Vec<PhysicalSortExpr>,        // aggregate ORDER BY
// }
```

The builder owns the logical-to-physical work the old helpers scattered: alias unwrapping, output-name choice, human-display preservation, and lowering of arguments, the optional filter, and aggregate `ORDER BY`. Two display notes: `AggregateFunctionExpr::human_display()` returns `Option<&str>` in 54, and physical `EXPLAIN` now shows lowered aggregates (e.g. `count(1) as count(*)`) — display text is not a stable identifier for plan matching. The plain `AggregateExprBuilder` still exists for building an `AggregateFunctionExpr` directly from physical expressions. Deep treatment: `datafusion_calculations_rust.md`.

---

# DataFusion Advanced — 50) Physical plan properties: partitioning, ordering, equivalence, boundedness, and emission

## 50.0 Purpose

Make `PlanProperties` and `ExecutionPlanProperties` a standalone **physical-planning correctness contract**:

```text id="n7g9xp"
ExecutionPlan
  ├─ schema()
  ├─ properties(): Arc<PlanProperties>
  │   ├─ EquivalenceProperties
  │   ├─ Partitioning
  │   ├─ EmissionType
  │   ├─ Boundedness
  │   ├─ EvaluationType
  │   └─ SchedulingType
  ├─ required_input_distribution()
  ├─ required_input_ordering()
  ├─ maintains_input_order()
  ├─ children()
  ├─ with_new_children()
  └─ execute(partition, TaskContext) -> SendableRecordBatchStream
```

The attached document already treats `PlanProperties` as the physical operator’s compact contract to DataFusion’s physical optimizer and downstream operators, and warns that false ordering/partitioning metadata can produce incorrect query results.  Current DataFusion docs describe `PlanProperties` as cached, expensive-to-compute physical-plan properties used in query optimization; its fields include `eq_properties`, `partitioning`, `emission_type`, `boundedness`, `evaluation_type`, and `scheduling_type`. ([Docs.rs][1])

---

## 50.1 Core value case

Physical properties let DataFusion answer:

```text id="xgpxvj"
Can this child already satisfy required distribution?
  → avoid RepartitionExec

Can this child already satisfy required ordering?
  → avoid SortExec

Are these expressions equivalent?
  → simplify sort requirements / join requirements / projection orderings

Can this plan run on unbounded input?
  → accept streaming query or reject unsupported topology

Can this operator emit incrementally?
  → stream results early or treat as pipeline breaker

Can downstream execution parallelize across N partitions?
  → schedule execute(0..N) concurrently
```

`ExecutionPlanProperties` exposes easy APIs derived from `ExecutionPlan::properties`, including `output_partitioning`, `output_ordering`, `boundedness`, `pipeline_behavior`, and `equivalence_properties`; `output_partitioning` says how output is split into partitions, and output ordering only applies when guaranteed. ([Docs.rs][2])

---

## 50.2 `PlanProperties`

### 50.2.1 Shape

```rust id="t9h9gm"
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::{
    execution_plan::{Boundedness, EmissionType},
    Partitioning,
    PlanProperties,
};

pub fn bounded_incremental_unknown_partitioning(
    schema: SchemaRef,
    partitions: usize,
) -> Arc<PlanProperties> {
    Arc::new(PlanProperties::new(
        EquivalenceProperties::new(schema),
        Partitioning::UnknownPartitioning(partitions),
        EmissionType::Incremental,
        Boundedness::Bounded,
    ))
}
```

Conceptual fields:

```text id="u4g9ou"
eq_properties:
  equivalences, orderings, constants

partitioning:
  output partition layout and partition count

emission_type:
  incremental vs final/blocking/both-style output behavior

boundedness:
  finite vs unbounded stream semantics

evaluation_type:
  physical evaluation mode / plan-specific execution category

scheduling_type:
  scheduling behavior used by runtime/optimizer where applicable
```

The attached document uses this exact construction pattern for custom sources and highlights the need to distinguish finite sources from unbounded streams and pipelined operators from pipeline breakers. 

---

## 50.3 `ExecutionPlanProperties`

### 50.3.1 Inspection helper

```rust id="damf2l"
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};

pub fn inspect_properties(plan: &dyn ExecutionPlan) {
    println!("partitioning={:?}", plan.output_partitioning());
    println!("ordering={:?}", plan.output_ordering());
    println!("boundedness={:?}", plan.boundedness());
    println!("pipeline_behavior={:?}", plan.pipeline_behavior());
    println!("equivalence_properties={:?}", plan.equivalence_properties());
}
```

`ExecutionPlanProperties` is an extension trait for `ExecutionPlan` objects; its required methods include `output_partitioning`, `output_ordering`, `boundedness`, `pipeline_behavior`, and `equivalence_properties`. ([Docs.rs][2])

### 50.3.2 Agent interpretation

```text id="itfat5"
output_partitioning():
  how many output streams exist and how rows are distributed across them

output_ordering():
  local per-partition ordering guarantee, not necessarily global order

boundedness():
  whether output stream is finite

pipeline_behavior():
  how/when operator emits rows

equivalence_properties():
  expression/order/constant equivalences useful for optimizer rules
```

---

## 50.4 Output partitioning

### 50.4.1 Partitioning enum

```rust id="jd923m"
use datafusion::physical_plan::Partitioning;

let p1 = Partitioning::UnknownPartitioning(8);
let p2 = Partitioning::RoundRobinBatch(8);
// let p3 = Partitioning::Hash(vec![physical_expr], 8);
```

DataFusion’s `Partitioning` enum includes `RoundRobinBatch(usize)`, `Hash(Vec<Arc<dyn PhysicalExpr>>, usize)`, and `UnknownPartitioning(usize)`; an `ExecutionPlan` with partitioning count 3 yields three independent output streams by calling `execute(0)`, `execute(1)`, and `execute(2)`. ([Docs.rs][3])

### 50.4.2 Partitioning meanings

| Partitioning             | Meaning                                                             | Correct use                                                              | Danger if false                                                               |
| ------------------------ | ------------------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| `UnknownPartitioning(n)` | `n` output partitions, distribution semantics unknown               | custom scans, arbitrary file groups, conservative default                | may insert extra repartitions but preserves correctness                       |
| `RoundRobinBatch(n)`     | rows/batches distributed round-robin across `n` partitions          | explicit round-robin exchange output                                     | false claim can break algorithms relying on balanced distribution assumptions |
| `Hash(exprs, n)`         | rows with same hash keys go to same partition                       | post-hash-repartition, sharded source keyed by same physical expressions | false claim can produce incorrect joins/aggregates                            |
| single partition         | usually `UnknownPartitioning(1)` or operator-specific single output | global aggregate, global limit, single-file source                       | underreports parallelism if source can partition                              |

### 50.4.3 Custom source partitioning policy

```rust id="wxbbk9"
pub enum NaturalPartitioning {
    Files(usize),
    Shards(usize),
    TenantRegions(usize),
    KeyRanges(usize),
    Single,
}

pub fn partitioning_from_layout(layout: NaturalPartitioning) -> Partitioning {
    match layout {
        NaturalPartitioning::Files(n)
        | NaturalPartitioning::Shards(n)
        | NaturalPartitioning::TenantRegions(n)
        | NaturalPartitioning::KeyRanges(n) => Partitioning::UnknownPartitioning(n.max(1)),
        NaturalPartitioning::Single => Partitioning::UnknownPartitioning(1),
    }
}
```

The custom table provider guide states that output partitioning tells DataFusion how many partitions a custom source has, which determines parallelism; if a source is naturally partitioned by files or shards, expose that partitioning. ([Apache DataFusion][4])

### 50.4.4 Hash partitioning correctness contract

```text id="dzy2ue"
Hash(exprs, n) asserts:
  every row is assigned to partition hash(exprs(row)) % n
  equal expr values must land in same partition
  exprs are physical expressions over output schema
  partition count is n
```

Use hash partitioning only when:

```text id="0c1106"
source enforces hash distribution by exact same keys
RepartitionExec produced the distribution
custom operator deliberately rehashed rows
partition function semantics match DataFusion expectations
```

Do not claim hash partitioning when:

```text id="u9eant"
data is merely grouped by files
data is range partitioned
data is sorted but not hash distributed
file path contains key value but rows may violate it
source distribution is approximate or stale
```

---

## 50.5 Ordering

### 50.5.1 Local vs global ordering

```text id="5m9e77"
Local per-partition ordering:
  partition 0 sorted by ts
  partition 1 sorted by ts
  partition 2 sorted by ts

Global ordering:
  all rows across all partitions sorted by ts
  requires stronger property, often merge/single partition/global sort
```

`ExecutionPlanProperties::output_ordering` returns ordering within each partition when guaranteed; `None` means no ordering assumptions should be made. ([Docs.rs][2])

### 50.5.2 Ordering examples

| Plan/operator                       | Likely ordering behavior                                                           |
| ----------------------------------- | ---------------------------------------------------------------------------------- |
| `DataSourceExec` over sorted source | may report ordering if guaranteed by source/config                                 |
| `FilterExec`                        | preserves relative input row order                                                 |
| `ProjectionExec`                    | may preserve order if sort expressions survive projection/equivalence              |
| `SortExec`                          | establishes local or global physical ordering depending topology                   |
| `RepartitionExec`                   | often breaks ordering unless order-preserving mode/semantics apply                 |
| `HashJoinExec`                      | generally destroys useful ordering                                                 |
| `SortMergeJoinExec`                 | consumes sorted inputs and may preserve relevant ordering depending implementation |
| `UnionExec`                         | may preserve partition-local input order under documented conditions               |
| `AggregateExec`                     | generally not ordered unless specialized                                           |

### 50.5.3 Sort preservation

```text id="kjcijl"
Order-preserving operators:
  Filter: removes rows, preserves relative order.
  Projection: preserves row order, but may rename/remove sort expressions.
  Limit: preserves prefix order.
  CoalesceBatches: preserves order.

Order-breaking operators:
  HashJoin
  Repartition
  HashAggregate
  Union across unordered children
  arbitrary custom operator
```

### 50.5.4 False ordering correctness bug

```text id="amw89p"
Source claims:
  output_ordering = [ts ASC]

Reality:
  one file is unsorted

Query:
  SELECT * FROM t ORDER BY ts LIMIT 10

Optimizer may:
  remove SortExec
  trust source order
  return wrong top-10 rows
```

The custom table provider guide explicitly says correct `PlanProperties` output ordering lets the optimizer avoid inserting `SortExec`; getting it right is a significant performance win, but false ordering is unsafe. ([Apache DataFusion][4]) The attached document likewise warns that incorrect ordering metadata can yield wrong results by removing needed sorts. 

### 50.5.5 Ordering policy

```text id="7l9dql"
Only report ordering if:
  source physically enforces it
  ingestion/compaction validates it
  all partitions independently satisfy it
  appended data cannot violate it
  projection/equivalence mapping preserves sort expressions
```

When unsure:

```rust id="cldokm"
// EquivalenceProperties::new(schema) with no ordering claims.
let eq = EquivalenceProperties::new(schema.clone());
```

---

## 50.6 Equivalence properties

### 50.6.1 Meaning

`EquivalenceProperties` stores optimizer-relevant facts about a plan node output, including sort expressions/orderings, equivalent expressions known to have the same value, and constant expressions known to contain a single constant value. ([Docs.rs][5])

```text id="55ffhs"
EquivalenceProperties:
  sort/order equivalences:
    ORDER BY a may satisfy ORDER BY b if a == b

  expression equivalences:
    join predicate a = b means a and b are equivalent downstream

  constants:
    WHERE region = 'US' means region is constant within output

  projection renames:
    SELECT a AS x can make x equivalent to a
```

### 50.6.2 Use cases

| Equivalence fact            | Optimizer value                                  |
| --------------------------- | ------------------------------------------------ |
| `a = b` after join/filter   | sort requirement on `a` may satisfy sort on `b`  |
| `region = 'US'` constant    | sort key containing `region` can be simplified   |
| `projection x = a`          | downstream ordering/partitioning can be remapped |
| same expression aliases     | avoid unnecessary sort/repartition               |
| constants in partition keys | reduce distribution/order requirements           |

`ExecutionPlanProperties::equivalence_properties` docs say equivalence properties tell DataFusion what columns are known equal during optimization; returning no known equivalences is always correct but may cause unnecessary re-sorting. ([Docs.rs][2])

### 50.6.3 Equivalence construction baseline

```rust id="m3h3xq"
use datafusion::physical_expr::EquivalenceProperties;

let eq = EquivalenceProperties::new(schema.clone());
```

Conservative baseline:

```text id="bv4qlf"
EquivalenceProperties::new(schema):
  no known orderings
  no known constants
  no known equivalent expressions
  always safe
  may be less optimal
```

### 50.6.4 Agent rules

```text id="k3leqr"
Add equivalences only when mathematically/semantically guaranteed.
Preserve equivalences through Projection only when expressions map exactly.
Preserve constants through Filter only when predicate enforces equality.
Drop equivalences after operators that may invalidate row/value relationships.
False equivalence can remove required sorts/repartitions and corrupt results.
```

---

## 50.7 Boundedness

### 50.7.1 Meaning

```text id="0j50g8"
Bounded:
  finite stream
  will eventually end
  files / finite RecordBatch sets / finite SQL source query

Unbounded:
  may never end
  Kafka-like topic / tailing log / WebSocket / streaming table
```

The attached streaming section says `boundedness()` describes whether a stream is bounded and that unbounded sources require DataFusion to plan only streaming-compatible query shapes.  DataFusion’s DDL docs for `CREATE UNBOUNDED EXTERNAL TABLE` state that DataFusion tries to execute queries over unbounded sources in streaming fashion and fails plan generation if the query cannot execute in streaming mode. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/ddl.html))

### 50.7.2 Custom source boundedness

```rust id="my7w80"
use datafusion::physical_plan::execution_plan::Boundedness;

pub enum SourceKind {
    FiniteFiles,
    FiniteApiExport,
    KafkaTopic,
    TailingLog,
}

pub fn boundedness_for_source(kind: SourceKind) -> Boundedness {
    match kind {
        SourceKind::FiniteFiles | SourceKind::FiniteApiExport => Boundedness::Bounded,
        SourceKind::KafkaTopic | SourceKind::TailingLog => Boundedness::Unbounded,
    }
}
```

### 50.7.3 Boundedness propagation

| Operator              | Bounded input → output  | Unbounded input → output                    | Notes                       |
| --------------------- | ----------------------- | ------------------------------------------- | --------------------------- |
| scan finite file      | bounded                 | n/a                                         | finite dataset              |
| unbounded source      | n/a                     | unbounded                                   | stream source               |
| projection            | same as input           | unbounded                                   | row-preserving              |
| filter                | same as input           | unbounded                                   | row-dropping only           |
| global sort           | bounded                 | usually invalid/blocking forever            | pipeline breaker            |
| global aggregate      | bounded                 | invalid unless streaming/windowed semantics | may wait forever            |
| time/window aggregate | bounded                 | possibly stream-compatible                  | depends implementation      |
| limit                 | bounded after limit     | may become bounded if limit can finish      | requires enough rows        |
| join                  | depends inputs/operator | often complex                               | streaming join restrictions |

Agent rule:

```text id="sveoj2"
Do not mark unbounded source as bounded.
Do not hide unboundedness in custom operators.
Reject unsupported blocking operators over unbounded input before execution.
```

---

## 50.8 Emission type

### 50.8.1 Meaning

`EmissionType` describes how an operator emits records: incrementally as records arrive, only final results at the end, or both. DataFusion’s docs illustrate `DataSourceExec` and `FilterExec` as incremental, while `SortExec` must wait for all input before emitting sorted output; left joins can emit matches incrementally and non-matches finally. ([Docs.rs][6])

Conceptual categories:

```text id="8ycx80"
Incremental:
  can emit output before consuming all input

Final/blocking:
  must consume all input before first output

Both / mixed:
  can emit some output incrementally and some final output after input ends
```

### 50.8.2 Operator examples

| Operator             | Emission behavior                                              |
| -------------------- | -------------------------------------------------------------- |
| file scan            | incremental                                                    |
| projection           | incremental                                                    |
| filter               | incremental                                                    |
| repartition          | often incremental but may buffer                               |
| hash aggregate final | often final/blocking for a group/global result                 |
| full sort            | final/blocking                                                 |
| top-k                | may buffer bounded state, emits final top-k                    |
| left join            | can emit matches incrementally and unmatched left rows finally |
| window               | depends frame/order requirements                               |

### 50.8.3 Pipeline-breaker classification

```text id="03kj6z"
Pipeline-friendly:
  DataSourceExec
  FilterExec
  ProjectionExec
  CoalesceBatches
  streaming map/UDF operators

Pipeline breaker:
  full SortExec
  global aggregate
  final distinct
  some window functions
  some joins
  collect()

Mixed:
  outer joins
  operators with incremental matched output and final unmatched output
```

Agent rule:

```text id="7gltbn"
EmissionType is not just performance metadata.
For unbounded streams, final-only operators may never emit.
```

---

## 50.9 Required child properties

### 50.9.1 Required distribution

```text id="qzbibt"
Parent operator says:
  child 0 must be hash-distributed by key k
  child 1 must be single-partition
  child 0 and child 1 must be co-partitioned

Physical optimizer:
  checks child output_partitioning
  inserts RepartitionExec if not satisfied
```

Examples:

| Parent                  | Required distribution                                  |
| ----------------------- | ------------------------------------------------------ |
| partitioned hash join   | both children hash-distributed by join keys            |
| final grouped aggregate | input distributed by grouping keys or single partition |
| global limit/sort       | may require single/global coordination                 |
| window `PARTITION BY`   | input distributed by partition keys                    |
| custom operator         | operator-specific                                      |

### 50.9.2 Required ordering

```text id="x05wdf"
Parent operator says:
  child must be sorted by [a ASC, b DESC]

Physical optimizer:
  checks child output_ordering/equivalence
  inserts SortExec if not satisfied
```

Examples:

| Parent                      | Required ordering                     |
| --------------------------- | ------------------------------------- |
| sort-merge join             | children ordered by join keys         |
| window order frame          | child ordered by partition/order keys |
| ordered aggregate           | input ordered by required expressions |
| custom streaming operator   | operator-specific                     |
| final global ordered output | sort/merge stage required             |

`PhysicalSortExpr::satisfy` checks whether a physical sort expression satisfies a sort requirement, including expression comparison and compatible sort options. ([Docs.rs][7])

### 50.9.3 Custom operator requirement sketch

```rust id="qj9qwx"
use datafusion::physical_plan::{Distribution, ExecutionPlan};

impl ExecutionPlan for MyExec {
    fn required_input_distribution(&self) -> Vec<Distribution> {
        // Pseudocode: exact Distribution variants depend on pinned version.
        // Return one requirement per child.
        vec![
            // Distribution::HashPartitioned(vec![...])
        ]
    }

    fn required_input_ordering(
        &self,
    ) -> Vec<Option<datafusion::physical_expr::LexRequirement>> {
        // Pseudocode: exact types depend on pinned version.
        vec![None]
    }
}
```

Agent rule:

```text id="yhgeh2"
Parent declares requirements.
Child reports properties.
Physical optimizer inserts repartition/sort when requirements are unmet.
False child properties can skip required repartition/sort and corrupt results.
```

---

## 50.10 Repartition insertion

```text id="x5rs69"
Parent requires:
  Hash distribution on [customer_id]

Child reports:
  UnknownPartitioning(8)

Optimizer inserts:
  RepartitionExec(Hash([customer_id], target_partitions))
```

Repartition decision inputs:

```text id="lgwblv"
parent required_input_distribution
child output_partitioning
equivalence_properties
target_partitions
operator config flags
known partition count
```

False metadata examples:

```text id="orazca"
Child falsely reports Hash([customer_id], 8):
  optimizer may skip RepartitionExec.
  grouped aggregate/join may produce wrong result.

Child conservatively reports UnknownPartitioning(8):
  optimizer may insert RepartitionExec.
  result correct, extra cost.
```

---

## 50.11 Sort insertion

```text id="9lxlsv"
Parent requires:
  input ordered by [event_ts ASC]

Child reports:
  output_ordering = None

Optimizer inserts:
  SortExec(event_ts ASC)
```

Sort decision inputs:

```text id="cqhwyy"
parent required_input_ordering
child output_ordering
EquivalenceProperties
constant/equivalent columns
sort options: asc/desc/nulls
global/local order requirement
```

False metadata examples:

```text id="kz938i"
Child falsely reports [event_ts ASC]:
  optimizer may skip SortExec.
  ORDER BY / window / sort-merge join can be wrong.

Child reports None despite sorted source:
  optimizer inserts extra SortExec.
  result correct, slower.
```

---

## 50.12 Property propagation by operator

| Operator        | Partitioning                             | Ordering                                        | Boundedness                                         | Emission                                               |
| --------------- | ---------------------------------------- | ----------------------------------------------- | --------------------------------------------------- | ------------------------------------------------------ |
| scan            | source-dependent                         | source-dependent                                | source-dependent                                    | usually incremental                                    |
| projection      | usually preserves partitioning           | row order preserved; sort expressions may remap | same as input                                       | incremental                                            |
| filter          | preserves partitioning                   | preserves relative order                        | same as input                                       | incremental                                            |
| limit           | may collapse/coordinate                  | preserves existing order                        | bounded if fetch finite                             | can be incremental/final depending global coordination |
| sort            | may preserve partitions or merge         | creates ordering                                | same as input if bounded; problematic for unbounded | blocking/final                                         |
| repartition     | changes partitioning                     | usually breaks ordering unless order-preserving | same as input                                       | incremental-ish with buffering                         |
| hash join       | join output partitioning depends mode    | usually breaks ordering                         | bounded if inputs bounded                           | incremental/final depends join type                    |
| sort-merge join | partitioning/order depends inputs/output | may preserve useful ordering                    | bounded if inputs bounded                           | incremental if inputs sorted/available                 |
| aggregate       | partitioning depends local/final stage   | usually not ordered                             | bounded unless streaming/windowed                   | often blocking/final                                   |
| window          | partitioning/order requirements dominate | output often follows required order             | input-dependent                                     | frame-dependent                                        |
| union           | combines partitions                      | may preserve child-local ordering               | bounded if all bounded                              | incremental                                            |
| unnest          | preserves input partitioning             | preserves input row order with expansion        | same as input                                       | incremental                                            |

Agent rule:

```text id="nz5tgw"
For custom operators, write a property propagation function.
Do not clone child PlanProperties blindly unless operator truly preserves all properties.
```

---

## 50.13 Custom `ExecutionPlan` property construction

```rust id="xi3e3c"
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::{
    execution_plan::{Boundedness, EmissionType, PlanProperties},
    Partitioning,
};

pub fn scan_properties(
    schema: SchemaRef,
    partitions: usize,
    boundedness: Boundedness,
    ordering_is_verified: bool,
) -> Arc<PlanProperties> {
    let mut eq = EquivalenceProperties::new(schema.clone());

    if ordering_is_verified {
        // Pseudocode:
        // Add ordering to eq according to pinned EquivalenceProperties API.
        // eq.add_ordering(...)
    }

    Arc::new(PlanProperties::new(
        eq,
        Partitioning::UnknownPartitioning(partitions),
        EmissionType::Incremental,
        boundedness,
    ))
}
```

Custom operator property checklist:

```text id="yzmilg"
[ ] schema matches emitted RecordBatch schema
[ ] partition count equals valid execute(partition) range
[ ] partitioning kind is conservative and true
[ ] ordering claims are verified
[ ] equivalences are true after operator transformation
[ ] boundedness reflects finite/infinite input/output
[ ] emission type reflects actual buffering/emission behavior
[ ] with_new_children recomputes properties if child affects them
```

---

## 50.14 `with_new_children` and property recomputation

Bad:

```rust id="vdip1u"
fn with_new_children(
    self: Arc<Self>,
    children: Vec<Arc<dyn ExecutionPlan>>,
) -> Result<Arc<dyn ExecutionPlan>> {
    Ok(Arc::new(Self {
        input: children[0].clone(),
        props: self.props.clone(), // unsafe if props depend on child
    }))
}
```

Good:

```rust id="wxei0l"
fn with_new_children(
    self: Arc<Self>,
    children: Vec<Arc<dyn ExecutionPlan>>,
) -> Result<Arc<dyn ExecutionPlan>> {
    if children.len() != 1 {
        return Err(datafusion::common::DataFusionError::Internal(
            "MyExec expects exactly one child".to_string()
        ));
    }

    let input = children[0].clone();

    let props = recompute_my_exec_properties(
        input.schema(),
        input.output_partitioning().clone(),
        input.boundedness(),
        input.pipeline_behavior(),
    );

    Ok(Arc::new(Self { input, props }))
}
```

Agent rule:

```text id="4ry36i"
If properties depend on children, recompute after child replacement.
Physical optimizers rely on with_new_children during rewrites.
```

---

## 50.15 Boundedness + emission and streaming query validity

Example invalid topology:

```text id="o6uuor"
Unbounded DataSourceExec
  → SortExec
  → output

Problem:
  SortExec needs all input before emitting final sorted order.
  Unbounded input never ends.
```

Example valid topology:

```text id="rv4egt"
Unbounded DataSourceExec
  → FilterExec
  → ProjectionExec
  → output

Reason:
  all operators are incremental and preserve unbounded streaming behavior.
```

DDL-level source declaration:

```sql id="te73g9"
CREATE UNBOUNDED EXTERNAL TABLE stream_events
STORED AS PARQUET
LOCATION '/path/or/source';
```

DataFusion documentation describes `CREATE UNBOUNDED EXTERNAL TABLE` as marking a data source unbounded and causing plan generation to fail if the query cannot execute in streaming mode. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/ddl.html))

Policy:

```text id="u25hp7"
Unbounded + Incremental:
  acceptable for streaming endpoints.

Unbounded + Final/blocking:
  reject unless operator has bounded state/window semantics.

Bounded + Final/blocking:
  acceptable if memory/spill/resource policy permits.
```

---

## 50.16 Property-aware plan lint

```rust id="cmnrsp"
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};

#[derive(Debug, Clone)]
pub struct PhysicalPropertyLint {
    pub severity: &'static str,
    pub code: &'static str,
    pub operator: String,
    pub message: String,
    pub remediation: String,
}

pub fn lint_physical_properties(
    plan: &dyn ExecutionPlan,
    out: &mut Vec<PhysicalPropertyLint>,
) {
    let bounded = plan.boundedness();
    let emission = plan.pipeline_behavior();

    if format!("{bounded:?}").contains("Unbounded")
        && format!("{emission:?}").contains("Final")
    {
        out.push(PhysicalPropertyLint {
            severity: "error",
            code: "UNBOUNDED_FINAL_OPERATOR",
            operator: plan.name().to_string(),
            message: "unbounded input/output with final-only emission may never emit".to_string(),
            remediation: "rewrite query to streaming-safe topology or use bounded source".to_string(),
        });
    }

    for child in plan.children() {
        lint_physical_properties(child.as_ref(), out);
    }
}
```

Use only as a scaffold; match exact `Boundedness`/`EmissionType` variants under pinned DataFusion APIs.

---

## 50.17 Explain verification

### SQL

```sql id="ctyc2o"
EXPLAIN VERBOSE
SELECT *
FROM events
WHERE event_date = DATE '2026-05-22'
ORDER BY event_ts
LIMIT 100;
```

### Rust

```rust id="o312ei"
use datafusion::physical_plan::{displayable, ExecutionPlanProperties};

pub async fn debug_properties(
    df: DataFrame,
) -> datafusion::error::Result<()> {
    let physical = df.create_physical_plan().await?;

    println!("{}", displayable(physical.as_ref()).indent(true));
    println!("partitioning={:?}", physical.output_partitioning());
    println!("ordering={:?}", physical.output_ordering());
    println!("boundedness={:?}", physical.boundedness());
    println!("pipeline={:?}", physical.pipeline_behavior());

    Ok(())
}
```

Verification targets:

```text id="mbf58d"
If source claims ordering:
  physical plan should not add SortExec for satisfied ORDER BY.

If source does not claim ordering:
  SortExec should appear for ORDER BY.

If source claims hash partitioning by join key:
  RepartitionExec may be absent before join.

If source reports UnknownPartitioning:
  RepartitionExec likely appears where required.

If source is unbounded:
  global SortExec should be rejected or absent in streaming-safe query.
```

---

## 50.18 Property accuracy tests

### 50.18.1 Partition count test

```rust id="ce98d8"
#[tokio::test]
async fn custom_exec_partition_count_matches_execute_range() -> Result<()> {
    let exec = build_custom_exec_with_partitions(4);

    assert_eq!(exec.output_partitioning().partition_count(), 4);

    let task_ctx = test_task_ctx();

    for i in 0..4 {
        let _stream = exec.execute(i, task_ctx.clone())?;
    }

    assert!(exec.execute(4, task_ctx).is_err());

    Ok(())
}
```

### 50.18.2 Ordering positive test

```rust id="bsfucr"
#[tokio::test]
async fn sorted_source_avoids_extra_sort_for_matching_order_by() -> Result<()> {
    let ctx = context_with_verified_sorted_source().await?;

    let df = ctx
        .sql("SELECT * FROM sorted_events ORDER BY event_ts")
        .await?;

    let physical = df.create_physical_plan().await?;
    let rendered = format!("{}", datafusion::physical_plan::displayable(physical.as_ref()).indent(true));

    // Under pinned version, assert no redundant SortExec if ordering is correctly reported.
    assert!(!rendered.contains("SortExec"));

    Ok(())
}
```

### 50.18.3 Ordering negative test

```rust id="c95eoe"
#[tokio::test]
async fn false_ordering_claim_would_change_topk_result() -> Result<()> {
    // Build deliberately unsorted source that falsely reports ordering.
    // Run ORDER BY key LIMIT n.
    // Compare against same source with no ordering claim.
    // This test should fail before such metadata reaches production.
    Ok(())
}
```

### 50.18.4 Boundedness test

```rust id="lckggt"
#[test]
fn unbounded_source_reports_unbounded_incremental() {
    let exec = build_unbounded_source_exec();

    assert!(format!("{:?}", exec.boundedness()).contains("Unbounded"));
    assert!(format!("{:?}", exec.pipeline_behavior()).contains("Incremental"));
}
```

---

## 50.19 Incorrect metadata negative tests

```text id="1c0tey"
False ordering:
  source reports ORDER BY ts but data unsorted
  query ORDER BY ts LIMIT 10 returns wrong rows if sort removed

False hash partitioning:
  source reports Hash(id, n) but same id appears in multiple partitions
  grouped aggregate or hash join returns incorrect/duplicated results

False boundedness:
  unbounded source reports bounded
  final aggregate/sort accepted and hangs or never emits

False incremental emission:
  operator buffers all input but reports incremental
  streaming admission accepts bad query; first batch delayed until end

False equivalence:
  reports a = b when not true
  sort/repartition removed incorrectly
```

Agent rule:

```text id="amsyv5"
Every custom property claim needs a negative test showing why false metadata would be caught.
```

---

## 50.20 Deployment advisory

```text id="nht0g9"
Custom TableProvider / ExecutionPlan:
  start conservative:
    UnknownPartitioning(n)
    no output_ordering
    EquivalenceProperties::new(schema)
    correct boundedness
    correct emission type

Then strengthen only with tests:
  Hash partitioning after verified hash distribution
  ordering after verified sorted source
  constants/equivalences after proof
  incremental emission after stream-polling behavior test

Production services:
  lint unbounded + blocking plans
  inspect physical plans for expensive SortExec/RepartitionExec
  include PlanProperties summary in diagnostics
  version-pin physical-plan snapshots

Upgrade workflow:
  re-run property accuracy tests
  re-run EXPLAIN VERBOSE expectations
  re-run false metadata negative tests
```

The custom table provider docs emphasize that setting output partitioning and output ordering correctly is key because output partitioning determines parallelism and output ordering lets the optimizer avoid unnecessary sorts. ([Apache DataFusion][4])

---

## 50.21 Anti-pattern inventory

* Reporting `Hash(keys, n)` because files are “mostly grouped” by key.
* Reporting output ordering because filenames are date-sorted while rows inside files are not.
* Treating local per-partition order as global order.
* Copying child `PlanProperties` after changing row distribution.
* Forgetting to recompute properties in `with_new_children`.
* Marking unbounded stream as `Bounded`.
* Marking full sort/global aggregate as `Incremental`.
* Adding equivalence properties from predicates that are not guaranteed after outer joins.
* Preserving equivalence properties through projections that remove/alter expressions.
* Claiming single partition for naturally sharded/file-partitioned source.
* Reporting many partitions for a source that serializes through one locked connection.
* Ignoring `required_input_distribution` for custom joins/aggregates.
* Ignoring `required_input_ordering` for order-sensitive custom operators.
* Using physical plan display strings as the only property test.
* Skipping negative tests for false ordering/partitioning.

---

## 50.22 Agent checklist

```text id="em4gns"
[ ] Build PlanProperties deliberately:
    EquivalenceProperties
    Partitioning
    EmissionType
    Boundedness

[ ] Output partitioning:
    UnknownPartitioning(n) when distribution semantics unknown
    RoundRobinBatch(n) only when round-robin distribution is true
    Hash(exprs,n) only when hash distribution is true
    single partition only when execution really has one output partition

[ ] Ordering:
    report output_ordering only if guaranteed per partition
    distinguish local vs global ordering
    preserve ordering through Filter/Projection only when valid
    never use false ordering to avoid SortExec

[ ] Equivalence:
    add column/expression equivalences only when proven
    preserve through projection/filter only when semantically valid
    drop after operators that invalidate relationships
    remember no-known-equivalence is always safe

[ ] Boundedness:
    finite files / finite batches => Bounded
    streams / queues / tailing logs => Unbounded
    reject unsupported blocking topology over unbounded input

[ ] Emission:
    scan/filter/projection usually Incremental
    full sort/global aggregate usually final/blocking
    outer joins/mixed operators may emit both incrementally and finally
    do not overclaim incremental behavior

[ ] Required children:
    declare required distribution when co-location is required
    declare required ordering when ordered input is required
    let optimizer insert RepartitionExec/SortExec when child properties do not satisfy requirements

[ ] Custom ExecutionPlan:
    properties() accurate
    children() accurate
    with_new_children() recomputes properties
    execute(partition) valid for every reported partition
    emitted batches match schema

[ ] Test:
    property accuracy tests
    false ordering negative test
    false partitioning negative test
    boundedness/emission tests
    EXPLAIN VERBOSE verification
    physical-plan snapshots under pinned DataFusion version
```

[1]: https://docs.rs/datafusion/latest/datafusion/physical_plan/execution_plan/struct.PlanProperties.html?utm_source=chatgpt.com "PlanProperties in datafusion::physical_plan::execution_plan"
[2]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlanProperties.html?utm_source=chatgpt.com "ExecutionPlanProperties in datafusion::physical_plan - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/physical_plan/enum.Partitioning.html?utm_source=chatgpt.com "Partitioning in datafusion::physical_plan - Rust"
[4]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html?utm_source=chatgpt.com "Custom Table Provider — Apache DataFusion documentation"
[5]: https://docs.rs/datafusion-physical-expr/latest/datafusion_physical_expr/equivalence/struct.EquivalenceProperties.html?utm_source=chatgpt.com "EquivalenceProperties in datafusion_physical_expr::equivalence"
[6]: https://docs.rs/datafusion/latest/datafusion/physical_plan/execution_plan/enum.EmissionType.html?search=&utm_source=chatgpt.com "\"\" Search - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/physical_expr/struct.PhysicalSortExpr.html?utm_source=chatgpt.com "PhysicalSortExpr in datafusion::physical_expr - Rust"


# DataFusion Advanced — 51) Scan planning and source pushdown: `TableProvider`, file scans, and custom sources

## 51.0 Purpose

Connect **source registration** to **planner behavior**:

```text id="m98fx4"
registered table / file path / custom provider
  → logical TableScan
  → TableProvider::supports_filters_pushdown
  → TableProvider::scan(projection, filters, limit)
  → ExecutionPlan
      ├─ DataSourceExec / custom scan exec
      ├─ projection pushdown
      ├─ filter pushdown
      ├─ limit pushdown
      ├─ file listing / partition discovery
      ├─ statistics / constraints / properties
      └─ RecordBatch stream
```

`TableProvider::scan` creates an `ExecutionPlan` for scanning a table with optional projection, filters, and limit; the returned plan is responsible for scanning source partitions in a streaming, parallelized fashion. DataFusion supplies projection to scan only query-needed columns, supplies boolean filters according to `supports_filters_pushdown`, and pushes `LIMIT` as far down as possible where safe. ([Docs.rs][1])

---

## 51.1 Source-planning layers

```text id="iqetw8"
Catalog / registration layer:
  register_csv / register_parquet / CREATE EXTERNAL TABLE / register_table
  produces named logical table

Logical planning:
  TableScan {
    table_name,
    source,
    projection,
    filters,
    fetch,
    constraints/stats metadata
  }

Provider planning:
  TableProvider::supports_filters_pushdown(filters)
  TableProvider::scan(state, projection, filters, limit)

Physical planning:
  DataSourceExec / custom ExecutionPlan
  partitioning/order/boundedness properties
  object-store/files/API/shard execution streams

Execution:
  execute(partition, TaskContext)
  SendableRecordBatchStream
  Arrow RecordBatch
```

The custom table provider guide describes the provider stack as `TableProvider` → `ExecutionPlan` → `RecordBatchStream`: the provider describes the source and creates a physical plan, the physical plan computes output, and the stream yields `RecordBatch` values. ([Apache DataFusion][2])

---

## 51.2 `TableProvider::scan`

### 51.2.1 Conceptual signature

```rust id="l2r8lx"
use std::sync::Arc;

use datafusion::catalog::Session;
use datafusion::common::Result;
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::ExecutionPlan;

#[async_trait::async_trait]
pub trait TableProvider {
    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>>;
}
```

Planner semantics:

```text id="n684o1"
projection:
  optional list of schema field indexes to return, in requested order

filters:
  boolean Expr predicates DataFusion wants evaluated during scan
  all filters are ANDed together
  only provided when supports_filters_pushdown opts in

limit:
  minimum useful source-side cap
  provider may return more rows
  not pushed when inexact filters make source-side cap semantically unsafe
```

The docs explicitly warn that columns used only by fully pushed filters may not be present in the projection; for `SELECT t.a FROM t WHERE t.b > 5`, projection pushdown may request only `a`, while predicate evaluation still needs access to `b` inside the scan. ([Docs.rs][1])

### 51.2.2 Minimal custom provider skeleton

```rust id="8xk729"
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::catalog::Session;
use datafusion::common::{Result, Statistics};
use datafusion::datasource::{TableProvider, TableType};
use datafusion::logical_expr::{Expr, TableProviderFilterPushDown};
use datafusion::physical_plan::ExecutionPlan;

#[derive(Debug)]
pub struct MyProvider {
    schema: SchemaRef,
    stats: Option<Statistics>,
}

#[async_trait::async_trait]
impl TableProvider for MyProvider {
    // DF 54: no `as_any` — `Any` is a supertrait of TableProvider.
    // Downcast with provider.downcast_ref::<MyProvider>() via trait upcasting.

    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    fn statistics(&self) -> Option<Statistics> {
        self.stats.clone()
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>> {
        Ok(filters
            .iter()
            .map(|expr| {
                if can_apply_exactly_at_source(expr) {
                    TableProviderFilterPushDown::Exact
                } else if can_use_for_pruning(expr) {
                    TableProviderFilterPushDown::Inexact
                } else {
                    TableProviderFilterPushDown::Unsupported
                }
            })
            .collect())
    }

    async fn scan(
        &self,
        state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        let plan = MyScanExec::try_new(
            self.schema.clone(),
            projection.cloned(),
            filters.to_vec(),
            limit,
        )?;

        Ok(Arc::new(plan))
    }
}

fn can_apply_exactly_at_source(_expr: &Expr) -> bool {
    false
}

fn can_use_for_pruning(_expr: &Expr) -> bool {
    false
}
```

Pushdown rule:

```text id="5ppgt1"
supports_filters_pushdown determines whether filters are passed to scan.
scan must implement the exact/inexact/unsupported contract it advertised.
```

---

## 51.3 Projection pushdown

### 51.3.1 Planner contract

```text id="tgyfu4"
Query:
  SELECT a, c FROM t WHERE b > 5

Projection needed for output:
  a, c

Columns needed for filter:
  b

Provider receives:
  projection may include only a,c
  filters may reference b
```

Projection is an index vector into `TableProvider::schema`; if specified, only those fields should be returned, in that order. DataFusion uses projection pushdown so sources such as Parquet can avoid reading unused columns. ([Docs.rs][1])

### 51.3.2 Projection application helper

```rust id="797wgs"
use datafusion::arrow::datatypes::{Schema, SchemaRef};

pub fn project_schema(
    schema: &SchemaRef,
    projection: Option<&Vec<usize>>,
) -> datafusion::common::Result<SchemaRef> {
    match projection {
        Some(indices) => Ok(Arc::new(schema.project(indices)?)),
        None => Ok(schema.clone()),
    }
}
```

### 51.3.3 Provider implementation rule

```text id="p5mn7n"
Output batches from scan must match projected schema exactly:
  field count
  field order
  data types
  nullability
  names
```

### 51.3.4 Projection pushdown anti-pattern

```rust id="me1hcf"
// Wrong: provider only loads projected columns and cannot evaluate pushed filter
// that references non-projected column b.
if projection == Some(vec![a_idx, c_idx]) {
    load_columns(vec!["a", "c"]);
    evaluate_filter("b > 5"); // fails or silently wrong
}
```

Correct strategy:

```text id="ft84ep"
Read/evaluate internal columns needed for filter.
Return only projected columns.
Filter-only columns may be internal scan dependencies, not output columns.
```

---

## 51.4 Filter pushdown

### 51.4.1 Pushdown classifications

| Classification | Provider guarantee                               | DataFusion residual filter? | Example                                            |
| -------------- | ------------------------------------------------ | --------------------------: | -------------------------------------------------- |
| `Exact`        | no output row can violate predicate              |                          no | partition column equality fully enforced by source |
| `Inexact`      | source reduces data but may emit false positives |                         yes | skip files by min/max but still scan rows          |
| `Unsupported`  | source ignores predicate                         |                         yes | arbitrary UDF predicate                            |

DataFusion’s custom provider guide defines these three responses: `Exact` means the provider fully evaluates the predicate and DataFusion will not add `FilterExec`; `Inexact` means source-side reduction may still include non-matching rows and DataFusion adds `FilterExec`; `Unsupported` leaves filtering to DataFusion. ([Apache DataFusion][2])

### 51.4.2 Exact pushdown

```rust id="t4qsuk"
fn can_apply_exactly_at_source(expr: &Expr) -> bool {
    // Exact only when source can produce precisely rows where expr is true
    // under SQL/DataFusion null semantics.
    is_partition_equality_on_region(expr)
}
```

Exact examples:

```text id="m5qqci"
Hive partition predicate:
  region = 'us-east-1'
  source reads only region=us-east-1 directories
  partition value is authoritative for every row

Indexed API predicate:
  id = 123
  API endpoint returns exactly matching id rows

Database-backed provider:
  SQL predicate delegated to database with equivalent semantics
```

Exact correctness requirements:

```text id="7jz799"
Predicate truth table identical to DataFusion.
NULL semantics identical.
Casts/comparisons identical or safely normalized.
No false positives.
No false negatives.
```

### 51.4.3 Inexact pushdown

```rust id="fwp7h2"
fn can_use_for_pruning(expr: &Expr) -> bool {
    // Source can skip files/partitions/row groups but not guarantee every emitted row satisfies expr.
    is_range_predicate_on_stats_column(expr)
}
```

Inexact examples:

```text id="wjdvug"
Parquet row-group min/max pruning.
Page-index pruning.
Bloom filter pruning.
File-level metadata pruning.
External index selecting candidate files/row groups.
Remote search endpoint returning candidates with possible false positives.
```

Correctness rule:

```text id="7smyix"
Inexact pushdown must keep residual FilterExec above scan.
```

### 51.4.4 Unsupported filters

Unsupported examples:

```text id="3kyv83"
volatile functions
custom UDF unknown to provider
regex if source lacks equivalent regex semantics
complex OR tree provider cannot reason about
nested expression with different null semantics
non-deterministic functions
```

Provider response:

```rust id="cruods"
TableProviderFilterPushDown::Unsupported
```

DataFusion default behavior assumes unsupported filters and inserts `FilterExec` above the scan; you opt into filter pushdown by implementing `supports_filters_pushdown`. ([Apache DataFusion][2])

---

## 51.5 Residual filter handling

### 51.5.1 Residual matrix

```text id="vay8xq"
Filter list:
  f1 Exact
  f2 Inexact
  f3 Unsupported

Provider:
  may use f1, f2 internally
  must ignore or cannot apply f3

DataFusion:
  no residual for f1
  residual FilterExec for f2
  residual FilterExec for f3
```

### 51.5.2 Never drop residuals

Wrong:

```rust id="7sxuux"
fn supports_filters_pushdown(...) -> Inexact
scan(...) returns candidate rows
// But planner/source drops residual filter → wrong results.
```

Correct:

```text id="7g7znn"
Inexact:
  scan reduces data.
  residual filter removes false positives.

Unsupported:
  scan ignores filter.
  residual filter applies full predicate.

Exact:
  source fully enforces predicate.
  no residual needed.
```

### 51.5.3 Limit interaction with inexact filters

DataFusion’s `TableProvider::scan` docs state that if inexact filters are pushed down, limit cannot be pushed down. Reason: applying limit before residual filtering can return too few final rows. ([Docs.rs][1])

Example:

```text id="6nq6jj"
Query:
  SELECT * FROM t WHERE x > 100 LIMIT 10

Inexact source pruning:
  source returns 10 candidate rows
  residual filter removes 8
  final output only 2 rows

Correct:
  do not push LIMIT below inexact filter.
```

Agent rule:

```text id="e106z0"
Limit pushdown is unsafe below inexact predicates unless provider can guarantee enough post-filter rows or DataFusion keeps semantics by another mechanism.
```

---

## 51.6 Limit pushdown

### 51.6.1 Contract

```text id="m753yx"
limit passed to scan:
  provider should produce at least this many rows if available
  provider may return more
  DataFusion still owns final semantics
```

DataFusion pushes limits as far down as possible because some sources can use the information to improve performance, but the docs explicitly note that inexact pushed filters prevent limit pushdown. ([Docs.rs][1])

### 51.6.2 Safe cases

```text id="38d039"
No filters:
  scan can stop after N rows.

Exact pushed filters:
  scan can stop after N matching rows.

Indexed query:
  API supports page size N after exact predicate.

Top-k source:
  source natively supports ORDER BY + LIMIT with equivalent ordering semantics.
```

### 51.6.3 Unsafe cases

```text id="sbnnhm"
Inexact filters.
Unsupported filters.
Global ORDER BY not enforced by source.
Joins/aggregates above scan where local limit changes result.
Provider pagination with unstable ordering.
```

### 51.6.4 Remote source limit pattern

```rust id="7nag8c"
pub struct RemoteScanRequest {
    pub projection: Vec<String>,
    pub filters: Vec<RemoteFilter>,
    pub page_size: usize,
    pub max_rows_hint: Option<usize>,
}

pub fn build_remote_request(
    projection: Option<&Vec<usize>>,
    filters: &[Expr],
    limit: Option<usize>,
) -> RemoteScanRequest {
    RemoteScanRequest {
        projection: compile_projection_names(projection),
        filters: compile_exact_remote_filters(filters),
        page_size: limit.unwrap_or(10_000).min(10_000),
        max_rows_hint: limit,
    }
}
```

---

## 51.7 Sort/order pushdown

### 51.7.1 Ordering metadata vs sort pushdown

```text id="xw2vz0"
Known ordering:
  source already emits sorted data.
  report output_ordering.
  optimizer may skip SortExec.

Sort pushdown:
  query requests ORDER BY.
  provider/API can produce requested ordering.
  provider returns ExecutionPlan with matching output_ordering.
```

`FileScanConfig` has an expression adapter for adapting pushed filters and projections from logical schema to physical file schema, and it can expose partition-derived output partitioning when file groups are organized by partition column values. ([Docs.rs][3])

### 51.7.2 Provider sort pushdown examples

```text id="ngucwp"
SQL database source:
  ORDER BY pushed to remote SQL engine.

Search/index source:
  sort by indexed timestamp desc.

Time-series API:
  source natively returns rows ordered by timestamp.

Sorted file source:
  source files verified sorted; report ordering, not active sort.
```

### 51.7.3 Correctness hazards

```text id="q7hsj3"
False ordering:
  optimizer skips SortExec.
  ORDER BY / LIMIT / windows / sort-merge joins can be wrong.

Local-only order:
  each partition sorted but global output not sorted.

Unstable remote order:
  pagination without deterministic tie-breaker yields duplicates/missing rows.

Projection rename:
  source order key must map through projection aliases/equivalences.
```

Agent policy:

```text id="9xqnii"
Report order only if verified.
Use deterministic tie-breakers for remote sort pagination.
Distinguish local per-partition ordering from global ordering.
Prefer no ordering claim over false ordering.
```

---

## 51.8 Statistics exposure

### 51.8.1 Provider stats

```rust id="32k3xm"
use datafusion::common::{Statistics, Precision, ColumnStatistics, ScalarValue};

fn statistics(&self) -> Option<Statistics> {
    Some(
        Statistics::default()
            .with_num_rows(Precision::Inexact(1_000_000))
            .with_total_byte_size(Precision::Inexact(128_000_000))
            .add_column_statistics(
                ColumnStatistics::new_unknown()
                    .with_null_count(Precision::Inexact(0))
                    .with_min_value(Precision::Inexact(ScalarValue::Int64(Some(1))))
                    .with_max_value(Precision::Inexact(ScalarValue::Int64(Some(1_000_000)))),
            )
    )
}
```

### 51.8.2 Stats policy

```text id="hclrd0"
Exact:
  source can prove value exactly for current snapshot.

Inexact:
  estimate, stale-ish but useful, sampled, metadata-derived with uncertainty.

Absent/unknown:
  source cannot safely provide value.
```

File scan statistics become inexact when filters are pushed down because DataFusion cannot guarantee how many rows will be removed by pruning/filtering. ([Docs.rs][3])

The scan `ExecutionPlan` you return reports statistics through `partition_statistics(Option<usize>) -> Result<Arc<Statistics>>` (DataFusion 54 wraps the result in `Arc`; see 47.4.6/47.24). Propagate `num_rows`, `total_byte_size`, min/max, null counts, and distinct counts with honest `Precision` instead of resetting them to unknown — 54's NDV-aware selectivity and join-cardinality estimates only work when sources forward them.

Agent rule:

```text id="i5dtuo"
Unknown stats are safer than false exact stats.
```

---

## 51.9 Constraints exposure

### 51.9.1 Constraint categories

```text id="8cwpwb"
unique key:
  id uniquely identifies row

non-null:
  id/event_date cannot be null

domain/range:
  event_type in finite set
  amount >= 0

foreign-key-like:
  customer_id references customers.id

functional dependency:
  customer_id → customer_name
```

### 51.9.2 Provider policy

```rust id="nlxc3m"
fn constraints(&self) -> Option<&datafusion::common::Constraints> {
    // Return None if unsupported/unknown.
    // Return Some(empty) if known no constraints.
    // Return Some(non_empty) only when trusted/enforced.
    None
}
```

Agent rules:

```text id="z01n8a"
Expose constraints only if enforced by source or immutable trusted metadata.
False constraints can make optimizer rewrites wrong.
Do not infer uniqueness from sample data.
Do not infer non-null from current batch unless source schema/enforcement proves it.
```

---

## 51.10 Partitioning exposure

### 51.10.1 Logical partition columns

```text id="2hron4"
/events/event_date=2026-05-22/region=us/file.parquet
```

Hive partition columns become table columns and can be used for pruning.

### 51.10.2 File group partitioning

`FileScanConfig::partitioned_by_file_group` says file groups are organized by partition column values; when true, output partitioning returns hash partitioning on partition columns, allowing the optimizer to skip hash repartitioning for aggregates and joins on partition columns. ([Docs.rs][3]) Setting it also pins each file to its file group: byte-range repartitioning is refused, and DataFusion 54's sibling work stealing between file-stream partitions is disabled (`create_sibling_state` returns `None` — see 54.25).

### 51.10.3 Custom provider partitioning

```rust id="13h5g8"
use datafusion::physical_plan::Partitioning;

pub fn source_partitioning(shards: usize) -> Partitioning {
    // Conservative: source has N independent output streams, distribution unknown.
    Partitioning::UnknownPartitioning(shards.max(1))
}
```

Rules:

```text id="130hkg"
Expose UnknownPartitioning(n) for N-way parallel source with unknown distribution.
Expose Hash(keys,n) only if exact hash partitioning by keys is guaranteed.
Expose ordering separately; partitioning and ordering are different contracts.
```

---

## 51.11 File scan planning

### 51.11.1 File scan pipeline

```text id="vpnkkz"
ListingTable / external table
  → object-store URL resolution
  → file listing
  → partition discovery
  → schema inference / provided schema
  → file groups
  → FileScanConfig
  → DataSourceExec
  → ParquetSource / CsvSource / JsonSource / AvroSource
  → execute(partition)
```

`ListingTable` is the built-in provider for reading one or more files as a single table through an `ObjectStore`, including local files and object stores such as S3. ([Docs.rs][4])

### 51.11.2 Listing

```text id="7dgcli"
Input:
  s3://bucket/events/

Steps:
  object store registry resolves s3://bucket
  list files under prefix
  filter by extension / format
  collect file size / modified / e_tag / version if available
  group files into scan partitions
```

### 51.11.3 Partition discovery

```text id="gukntg"
Path:
  /events/event_date=2026-05-22/region=us/file.parquet

Discovered partition columns:
  event_date = '2026-05-22'
  region = 'us'

Uses:
  partition pruning
  schema extension
  partition column filters
  possible file group partitioning
```

### 51.11.4 File groups

```text id="ab7sfa"
file_groups:
  Vec<Vec<PartitionedFile>>

Meaning:
  outer Vec = output partitions
  inner Vec = files scanned by that partition

Execution:
  execute(partition_i) scans file_groups[i]
```

### 51.11.5 Object-store registry

```text id="prgbfn"
URI:
  s3://bucket/path

RuntimeEnv / ObjectStoreRegistry:
  scheme + authority resolution
  credential/config resolution
  object_store instance

ExecutionPlan:
  uses object_store for list/get/range reads
```

Agent policy:

```text id="bplxc8"
Register object stores centrally.
Do not inline long-lived credentials in SQL.
Use separate registries/contexts for tenant-specific credentials.
```

---

## 51.12 File metadata and cache planning

### 51.12.1 Metadata cache

The CLI-specific `metadata_cache()` table function shows information about the default File Metadata Cache used by `ListingTable`; the cache speeds up reading file metadata when scanning directories with many files. It includes path, modified time, file size, ETag, version, metadata size, hit count, and extra info such as page-index inclusion. ([Apache DataFusion][5])

Diagnostic SQL:

```sql id="vkgkvi"
SELECT *
FROM metadata_cache();

SELECT
  SUM(metadata_size_bytes) AS cached_metadata_bytes
FROM metadata_cache();
```

### 51.12.2 Statistics cache

`statistics_cache()` shows File Statistics Cache entries used by `ListingTable`; statistics are collected only when `datafusion.execution.collect_statistics` is enabled. ([Apache DataFusion][5])

```sql id="8exqao"
SELECT *
FROM statistics_cache();

SELECT
  path,
  num_rows,
  num_columns,
  table_size_bytes
FROM statistics_cache();
```

### 51.12.3 List-files cache

`list_files_cache()` shows the `ListFilesCache` used by `ListingTable`; DataFusion caches file-listing results scoped to tables so subsequent queries can avoid re-listing. ([Apache DataFusion][5])

```sql id="yr10ca"
SELECT *
FROM list_files_cache();
```

Cache policy:

```text id="d4y6kz"
Metadata cache:
  reduce repeated footer reads.

Statistics cache:
  reduce repeated stats collection.

List-files cache:
  reduce object-store listing latency.

Invalidation:
  object modified time
  ETag/version
  table registration refresh
  source snapshot change
  explicit cache policy/TTL
```

---

## 51.13 Parquet pruning and pushdown

### 51.13.1 Pruning layers

```text id="s3ag8g"
Column projection:
  read only referenced columns

Row-group pruning:
  use row-group min/max/null stats and bloom filters

Page pruning:
  use Parquet page index to skip pages

Row filtering / late materialization:
  evaluate predicate during decode and avoid decoding later columns for rejected rows where possible

External access plan:
  restrict row groups/pages before normal pruning
```

`ParquetSource` docs state that `DataSourceExec` uses a physical predicate to skip reading unnecessary Parquet data via row-group pruning with min/max metadata and bloom filters, page pruning using the Parquet PageIndex, and row filtering/late materialization when `pushdown_filters` is enabled. ([Docs.rs][6])

### 51.13.2 Config knobs

```sql id="bm3yrd"
SET datafusion.execution.parquet.bloom_filter_on_read = true;
SET datafusion.execution.parquet.pushdown_filters = true;
SET datafusion.execution.parquet.enable_page_index = true;
SET datafusion.execution.parquet.max_predicate_cache_size = 104857600;
```

DataFusion configuration docs show `bloom_filter_on_read` defaults to true, and `max_predicate_cache_size` controls memory used to cache predicate results when `pushdown_filters` is enabled. ([Apache DataFusion][7])

### 51.13.3 External indexes

`ParquetSource` can accept a `ParquetAccessPlan` via `PartitionedFile` extensions to restrict row groups/selections before DataFusion further reduces the plan using Parquet metadata and settings. ([Docs.rs][6]) In DataFusion 54 the `extensions` field is a typed map — `PartitionedFile.extensions: FileExtensions`, where `FileExtensions = datafusion_common::extensions::Extensions` — keyed by `TypeId`: attach with `with_extension(value)` and read back with `extension::<T>()`, so an access plan and other sidecar metadata can coexist on the same file without conflict (the pre-54 single-slot `with_extensions(Arc<dyn Any + Send + Sync>)` is retained for compatibility). See 51.23.

Use cases:

```text id="eqd5su"
external inverted index
external zone map
remote metadata store
Delta/Iceberg-like file/row-group stats sidecar
domain-specific skip index
```

---

## 51.14 Custom remote source planning

### 51.14.1 Remote source planner model

```text id="98rqzo"
Logical TableScan
  → filters classified:
      exact remote filters
      inexact index filters
      unsupported residual filters
  → projection classified:
      remote fields needed
      local-only computed deps
  → pagination plan:
      page size
      max rows hint
      continuation token
      deterministic order key
  → async stream:
      fetch page
      decode to RecordBatch
      apply source-exact filters remotely
      emit batches
      respect backpressure
```

### 51.14.2 Remote filter compiler

```rust id="j49l2f"
#[derive(Debug, Clone)]
pub enum RemoteFilter {
    Eq { field: String, value: ScalarValue },
    Range { field: String, lower: Option<ScalarValue>, upper: Option<ScalarValue> },
    In { field: String, values: Vec<ScalarValue> },
}

#[derive(Debug, Clone)]
pub enum FilterPlan {
    ExactRemote(RemoteFilter),
    InexactIndex(RemoteFilter),
    Residual(Expr),
}

pub fn classify_remote_filter(expr: &Expr) -> FilterPlan {
    if let Some(f) = compile_exact_api_filter(expr) {
        FilterPlan::ExactRemote(f)
    } else if let Some(f) = compile_index_candidate_filter(expr) {
        FilterPlan::InexactIndex(f)
    } else {
        FilterPlan::Residual(expr.clone())
    }
}
```

### 51.14.3 Pagination contract

```rust id="qo62xu"
#[derive(Debug, Clone)]
pub struct RemotePageRequest {
    pub projection: Vec<String>,
    pub exact_filters: Vec<RemoteFilter>,
    pub inexact_filters: Vec<RemoteFilter>,
    pub order_by: Vec<RemoteSortKey>,
    pub page_size: usize,
    pub continuation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RemotePage {
    pub batch: RecordBatch,
    pub continuation: Option<String>,
    pub may_have_more: bool,
}
```

Pagination rules:

```text id="f61nzp"
Use deterministic ordering when paginating.
Include stable tie-breaker key.
Do not push LIMIT below residual filters.
Do not assume remote page_size equals final result limit.
Apply residual DataFusion FilterExec when remote filtering is inexact/unsupported.
```

### 51.14.4 Async stream construction

```rust id="xyjf8m"
use std::pin::Pin;
use std::task::{Context, Poll};

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::error::DataFusionError;
use datafusion::physical_plan::{RecordBatchStream, SendableRecordBatchStream};

pub struct RemoteRecordBatchStream {
    schema: SchemaRef,
    state: RemoteStreamState,
}

impl RecordBatchStream for RemoteRecordBatchStream {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
}

impl futures::Stream for RemoteRecordBatchStream {
    type Item = Result<RecordBatch, DataFusionError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // Poll remote request future only when downstream asks.
        // Preserve backpressure: do not prefetch unboundedly.
        todo!("fetch/decode next page and emit RecordBatch")
    }
}
```

Backpressure policy:

```text id="7uz5vp"
Do not spawn unbounded background fetches.
Only fetch when stream is polled or bounded prefetch window allows.
Respect cancellation: dropping stream should stop remote work.
Bound in-flight pages.
Expose metrics: requests, bytes, rows, latency, retries.
```

---

## 51.15 Custom scan `ExecutionPlan`

### 51.15.1 Physical plan skeleton

```rust id="mlsd9e"
use std::sync::Arc;

use datafusion::arrow::datatypes::SchemaRef;
use datafusion::common::Result;
use datafusion::execution::TaskContext;
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::{
    ExecutionPlan,
    SendableRecordBatchStream,
    execution_plan::{Boundedness, EmissionType, PlanProperties},
    Partitioning,
};

#[derive(Debug)]
pub struct MyScanExec {
    schema: SchemaRef,
    projected_schema: SchemaRef,
    partitions: usize,
    props: Arc<PlanProperties>,
    projection: Option<Vec<usize>>,
    filters: Vec<Expr>,
    limit: Option<usize>,
}

impl MyScanExec {
    pub fn try_new(
        schema: SchemaRef,
        projection: Option<Vec<usize>>,
        filters: Vec<Expr>,
        limit: Option<usize>,
    ) -> Result<Self> {
        let projected_schema = project_schema(&schema, projection.as_ref())?;

        let partitions = 1;

        let props = Arc::new(PlanProperties::new(
            EquivalenceProperties::new(projected_schema.clone()),
            Partitioning::UnknownPartitioning(partitions),
            EmissionType::Incremental,
            Boundedness::Bounded,
        ));

        Ok(Self {
            schema,
            projected_schema,
            partitions,
            props,
            projection,
            filters,
            limit,
        })
    }
}

impl ExecutionPlan for MyScanExec {
    fn name(&self) -> &str {
        "MyScanExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.props
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        if !children.is_empty() {
            return Err(datafusion::common::DataFusionError::Internal(
                "MyScanExec expects no children".to_string(),
            ));
        }

        Ok(self)
    }

    fn execute(
        &self,
        partition: usize,
        context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        if partition >= self.partitions {
            return Err(datafusion::common::DataFusionError::Execution(format!(
                "partition {partition} out of range, partitions={}",
                self.partitions,
            )));
        }

        todo!("return SendableRecordBatchStream")
    }
}
```

Scan exec rules:

```text id="v30yli"
Leaf exec has no children.
execute(partition) valid only for reported partitions.
Stream schema equals projected_schema.
Properties describe actual partitioning/order/boundedness/emission.
```

---

## 51.16 Source pushdown correctness rules

### 51.16.1 Never drop residual filters

```text id="lplf44"
Exact:
  residual may be removed.

Inexact:
  residual required.

Unsupported:
  residual required.
```

### 51.16.2 Preserve SQL null semantics

Predicate:

```sql id="ot8t29"
WHERE amount > 0
```

SQL/DataFusion semantics:

```text id="gihd1j"
amount = 10   → true
amount = -1   → false
amount = NULL → unknown/null → filtered out
```

Remote API hazard:

```text id="t4xuna"
API "amount > 0" may ignore nulls differently.
API may treat missing field as 0.
API may treat string "10" as number.
```

Rule:

```text id="qew3r7"
If semantics differ, pushdown is Inexact or Unsupported, not Exact.
```

### 51.16.3 Conservative pushdown reporting

```text id="e0zdxm"
When unsure:
  Unsupported

When can prune candidates but not exact rows:
  Inexact

Only when identical result:
  Exact
```

### 51.16.4 Unknown stats over false stats

```text id="2hykz6"
Bad:
  Statistics::Exact(10_000_000) from stale catalog

Good:
  Precision::Inexact(10_000_000)
  or Precision::Absent
```

### 51.16.5 Projection and filter dependency separation

```text id="gcucdb"
Projection:
  returned output columns

Internal dependency:
  columns needed to evaluate pushed filters/order/indexes

Provider must load enough internal data to apply pushed filters,
but output only projected columns.
```

---

## 51.17 Deployment patterns

### 51.17.1 Parquet lake source

```text id="fcxpx1"
Use:
  register_parquet / CREATE EXTERNAL TABLE
  directory roots, not wildcards
  Hive partitions for coarse pruning
  collect_statistics when amortized query benefit > registration cost
  metadata/statistics/list-files cache for repeated scans
  page index and bloom filters for high-selectivity workloads
```

### 51.17.2 Custom API source

```text id="x09ynw"
Implement:
  supports_filters_pushdown
  exact API filters
  inexact index filters
  residual handling
  projection mapping
  deterministic pagination
  async backpressure stream
  metrics and retry policy
  UnknownPartitioning unless distribution guaranteed
```

### 51.17.3 Indexed database source

```text id="7gefnb"
Push down:
  equality/range predicates with equivalent semantics
  projection
  limit when no residual filter
  order by if database returns deterministic ordering

Do not:
  claim Exact for functions/casts with different DB semantics
  trust DB collation/string comparison if different from DataFusion unless normalized
```

### 51.17.4 Tenant-partitioned source

```text id="9uxq9l"
Push down:
  tenant_id exact filter from trusted service policy
  partition columns
  date range
  region/shard

Governance:
  inject tenant filter before source planning
  do not rely on user-supplied tenant predicate
  source credentials scoped per tenant
```

---

## 51.18 Testing matrix

```text id="lpkul1"
Projection:
  [ ] projection order matches output schema
  [ ] projected subset returns only requested columns
  [ ] filter-only columns are internally available but not output
  [ ] projection + nested fields if supported

Filter pushdown:
  [ ] Exact removes residual filter and returns correct rows
  [ ] Inexact keeps residual filter and returns correct rows
  [ ] Unsupported keeps full FilterExec
  [ ] NULL predicate semantics match DataFusion
  [ ] volatile predicates not pushed

Limit:
  [ ] limit pushed with no filters
  [ ] limit pushed with exact filters
  [ ] limit not pushed with inexact filters
  [ ] remote pagination returns enough post-filter rows

Ordering:
  [ ] source ordering claim removes SortExec only when verified
  [ ] false-ordering negative test catches wrong top-k
  [ ] remote order pagination deterministic

Statistics:
  [ ] unknown stats when unavailable
  [ ] inexact stats when estimated
  [ ] stats invalidated after data change
  [ ] filtered scan statistics marked inexact

File scans:
  [ ] file listing cached/reused
  [ ] Hive partitions discovered
  [ ] partition pruning works
  [ ] Parquet row-group pruning metrics observed
  [ ] page pruning enabled/disabled
  [ ] bloom pruning tested
  [ ] metadata cache inspected

Remote source:
  [ ] cancellation stops remote requests
  [ ] bounded prefetch
  [ ] retries do not duplicate rows
  [ ] stream schema stable
  [ ] backpressure respected
```

---

## 51.19 Observability

### 51.19.1 Scan metrics to expose

```text id="qsnfc5"
source:
  files_listed
  files_pruned
  files_scanned
  partitions_scanned
  rows_requested
  rows_returned
  bytes_requested
  bytes_decoded
  remote_requests
  remote_retries
  remote_latency_ms

pushdown:
  projection_columns
  exact_filters
  inexact_filters
  unsupported_filters
  residual_filter_present
  limit_pushed
  order_pushed

Parquet:
  row_groups_pruned
  pages_pruned
  bloom_filter_checks
  predicate_rows_matched
  predicate_rows_pruned
```

### 51.19.2 Debug SQL

```sql id="5fvf7b"
EXPLAIN ANALYZE
SELECT *
FROM events
WHERE event_date = DATE '2026-05-22'
  AND region = 'us';
```

### 51.19.3 Cache inspection

```sql id="qtbnkb"
SELECT * FROM metadata_cache();
SELECT * FROM statistics_cache();
SELECT * FROM list_files_cache();
```

Cache introspection functions are CLI-specific helpers that expose ListingTable file metadata, statistics, and list-files cache state; do not assume these table functions are registered in every embedded engine. ([Apache DataFusion][5])

---

## 51.20 Best-practice advisory

```text id="4snicd"
TableProvider:
  implement projection pushdown first.
  implement exact filter pushdown only for equivalent source semantics.
  implement inexact pushdown for file/partition/stat pruning.
  expose unknown/inexact stats rather than false exact stats.
  expose constraints only if enforced/trusted.
  expose partitioning/order only if guaranteed.

File scans:
  use directory roots.
  use Hive partitions for common filters.
  enable Parquet pruning/page/bloom features based on workload.
  cache listings/metadata/stats when object-store latency matters.
  validate schema compatibility across files.

Remote APIs:
  compile filters conservatively.
  preserve residual filtering.
  paginate deterministically.
  respect backpressure and cancellation.
  cap in-flight requests.
  map remote errors to DataFusionError with context.

Security:
  restrict direct path scans.
  isolate object-store credentials.
  redact paths/metadata in public diagnostics.
  avoid exposing cache contents to unauthorized users.
```

---

## 51.21 Anti-pattern inventory

* Returning `Exact` when source only prunes files/row groups.
* Dropping residual filters after `Inexact` pushdown.
* Pushing `LIMIT` below inexact filters.
* Applying remote filters with different NULL/collation/cast semantics and reporting `Exact`.
* Ignoring columns used only in pushed filters because they are absent from projection.
* Reporting stale catalog row counts as exact.
* Claiming source ordering because filenames are sorted.
* Claiming hash partitioning because directory paths contain key values.
* Fetching all remote pages before first stream poll.
* Ignoring stream cancellation and continuing remote fetches.
* Inlining object-store credentials in SQL/table options.
* Exposing metadata/listing caches to public tenants.
* Assuming Parquet predicate errors should fail scan acceleration; ParquetSource docs note unusable predicates may be ignored for acceleration rather than erroring. ([Docs.rs][6])
* Treating ListingTable cache state as durable truth without invalidation policy.

---

## 51.22 Agent checklist

```text id="puh7fu"
[ ] Source registration:
    register_table | register_parquet | CREATE EXTERNAL TABLE | ListingTable | custom provider

[ ] TableProvider::scan:
    projection handled
    filters handled according to pushdown classification
    limit handled safely
    returned ExecutionPlan streams partitions

[ ] Projection pushdown:
    output schema matches projection order
    filter-only columns internally available
    no extra public columns returned

[ ] Filter pushdown:
    Exact only for fully equivalent filtering
    Inexact for pruning/candidate reduction
    Unsupported when unsure
    residual filters preserved for Inexact/Unsupported
    SQL null semantics preserved

[ ] Limit pushdown:
    allowed with no residual/inexact filters
    not pushed below inexact filters
    remote pagination deterministic

[ ] Sort/order:
    source ordering verified before reporting
    sort pushdown only with deterministic source order
    local vs global ordering distinguished

[ ] Statistics/constraints:
    Statistics precision correct
    stale stats invalidated
    constraints only if trusted
    unknown over false

[ ] Partitioning:
    Hive partitions discovered
    file groups correctly mapped
    UnknownPartitioning when distribution unknown
    Hash partitioning only if guaranteed

[ ] File scans:
    object-store registry configured
    listing cache considered
    metadata cache considered
    statistics cache considered
    Parquet row-group/page/bloom pruning tested

[ ] Remote source:
    API filters compiled conservatively
    index filters marked Inexact unless exact
    async stream respects backpressure
    cancellation stops work
    retries idempotent

[ ] Observability:
    EXPLAIN ANALYZE checked
    scan metrics exposed
    cache diagnostics available to authorized users only
```

[1]: https://docs.rs/datafusion/latest/datafusion/datasource/trait.TableProvider.html "TableProvider in datafusion::datasource - Rust"
[2]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html "Custom Table Provider — Apache DataFusion  documentation"
[3]: https://docs.rs/datafusion/latest/datafusion/datasource/physical_plan/struct.FileScanConfig.html "FileScanConfig in datafusion::datasource::physical_plan - Rust"
[4]: https://docs.rs/deltalake/latest/deltalake/datafusion/datasource/listing/struct.ListingTable.html?utm_source=chatgpt.com "ListingTable in deltalake::datafusion::datasource::listing"
[5]: https://datafusion.apache.org/user-guide/cli/functions.html "CLI Specific Functions — Apache DataFusion  documentation"
[6]: https://docs.rs/crate/datafusion/latest/target-redirect/x86_64-unknown-linux-gnu/datafusion/datasource/physical_plan/parquet/source/struct.ParquetSource.html "ParquetSource in datafusion::datasource::physical_plan::parquet::source - Rust"
[7]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"


## 51.23 DataFusion 54 scan-planning additions

### 51.23.1 Typed `PartitionedFile.extensions` (`FileExtensions`)

`PartitionedFile.extensions` is now a typed, `TypeId`-keyed map instead of a single opaque `Arc<dyn Any>` slot (`datafusion-datasource/src/mod.rs`):

```rust id="fext54"
// pub type FileExtensions = datafusion_common::extensions::Extensions;
// pub extensions: FileExtensions,   // on PartitionedFile

let file = PartitionedFile::new(path, size)
    .with_extension(my_access_plan)      // one value per type T
    .with_extension(my_index_hint);      // different T — coexists

let plan: Option<&ParquetAccessPlan> = file.extension::<ParquetAccessPlan>();
```

The underlying `Extensions` map (`datafusion-common/src/extensions.rs`) exposes `insert`/`insert_arc`/`insert_dyn`, `get`/`get_arc`, `contains::<T>()`, and `merge` — so an external index can attach a `ParquetAccessPlan` while other machinery attaches unrelated sidecar metadata to the same file without conflict. The legacy single-slot `with_extensions(Arc<dyn Any + Send + Sync>)` is retained for compatibility. Values must be `Any + Send + Sync`.

### 51.23.2 Nested Parquet leaf projection and struct-member filter pushdown

DataFusion 54's Parquet scan handles nested columns at leaf granularity: a projection that addresses struct members reads only the referenced Parquet leaf columns instead of materializing the whole struct, and filters on nested/struct members can be pushed all the way to the Parquet decoder (row filtering / late materialization). Pushdown eligibility for nested-column predicates is gated by an explicit registry of supported physical expressions (`datafusion-datasource-parquet/src/supported_predicates.rs`) — unsupported shapes fall back to post-scan `FilterExec` residuals as usual, so the 51.4/51.5 residual contract is unchanged. Primary coverage: `datafusion_rust.md` (engine deep-dive, Parquet sections).

### 51.23.3 Statistics-driven Top-K scans

For `ORDER BY ... LIMIT k` over file sources, 54 uses file- and row-group-level min/max statistics to order scan input toward likely Top-K candidates and terminate early once no remaining file can beat the current heap boundary (`datafusion-datasource-parquet/src/opener/early_stop.rs`); with favorable layouts the sort itself can be eliminated. This raises the planning value of accurate file-level statistics (51.8) and of `split_file_groups_by_statistics`-style layout — note that key already existed in 53. Config: `optimizer.enable_topk_repartition = true`, `optimizer.enable_window_topn = false`; the `*_dynamic_filter_pushdown` keys predate 54 (behavior improved, not new knobs). Planner-metadata view: 47.24.5.

---

# DataFusion Advanced — 52) Join planning decision model

## 52.0 Purpose

Make join planning explainable, tunable, and agent-operable:

```text id="2wbr1h"
SQL / DataFrame / LogicalPlanBuilder join
  → logical Join
      ├─ join_type
      ├─ equijoin keys
      ├─ non-equi join filter
      ├─ null equality semantics
      └─ output schema / projection
  → analyzer / logical optimizer
      ├─ predicate pushdown
      ├─ cross-join elimination
      ├─ subquery decorrelation
      ├─ semi/anti rewrite
      ├─ join ordering
      └─ key/cast simplification
  → physical planner
      ├─ HashJoinExec
      ├─ SortMergeJoinExec
      ├─ NestedLoopJoinExec
      ├─ PiecewiseMergeJoinExec
      ├─ SymmetricHashJoinExec
      └─ CrossJoinExec
  → physical optimizer
      ├─ repartition insertion/removal
      ├─ sort insertion/removal
      ├─ dynamic filter insertion
      ├─ build/probe side tuning
      └─ spill/memory behavior
```

DataFusion’s join module exposes physical join operators including `HashJoinExec`, `SortMergeJoinExec`, `NestedLoopJoinExec`, `PiecewiseMergeJoinExec`, `SymmetricHashJoinExec`, and `CrossJoinExec`; the attached documentation already identifies join tuning as a core DataFusion planning section covering join types, algorithms, repartitioning, join filters, dynamic filters, build/probe concerns, memory, and spilling.  ([Docs.rs][1])

---

## 52.1 Logical join anatomy

```rust id="7eddpc"
use datafusion::logical_expr::JoinType;
use datafusion::prelude::*;

let joined = left.join(
    right,
    JoinType::Inner,
    &["customer_id"],
    &["id"],
    None,
)?;
```

Logical join components:

```text id="0heagk"
join_type:
  Inner | Left | Right | Full | LeftSemi | RightSemi | LeftAnti | RightAnti | Cross-like forms

left input:
  LogicalPlan

right input:
  LogicalPlan

join keys:
  Vec<(left_expr, right_expr)>
  normally equality predicates

join filter:
  residual/non-equality predicate evaluated after/on candidate matches

null semantics:
  NULL != NULL by normal SQL equality unless null-equality mode says otherwise in specific physical contexts

output schema:
  depends on join type
  inner/outer: left + right fields
  semi/anti: usually preserving side only
  outer joins: nullability widened on nullable side
```

DataFusion SQL supports inner, left/right/full outer, natural, cross, semi, anti, lateral, and left lateral join syntax; physical hash joins are optimized for equijoin predicates and evaluate non-equality predicates as optional post-join filters. ([Docs.rs][1])

---

## 52.2 Join-type semantics matrix

| Join type   | Output row rule                            | Output schema rule             | Common use                                 |
| ----------- | ------------------------------------------ | ------------------------------ | ------------------------------------------ |
| `Inner`     | matching pairs only                        | left + right                   | fact/dim enrichment                        |
| `Left`      | all left rows, matched right or NULL right | left + nullable right          | preserve fact side                         |
| `Right`     | all right rows, matched left or NULL left  | nullable left + right          | less common; can swap to left              |
| `Full`      | all rows from both sides                   | nullable left + nullable right | reconciliation                             |
| `LeftSemi`  | left rows with at least one match          | left only                      | `EXISTS`                                   |
| `LeftAnti`  | left rows with no match                    | left only                      | `NOT EXISTS`                               |
| `RightSemi` | right rows with at least one match         | right only                     | right-side existence                       |
| `RightAnti` | right rows with no match                   | right only                     | right-side non-existence                   |
| `Cross`     | Cartesian product                          | left + right                   | explicit combination / dimension expansion |
| `Lateral`   | right subquery may reference left row      | left + right / join-dependent  | correlated table expressions               |

Agent rules:

```text id="mu8b0z"
Use semi/anti joins for existence/non-existence.
Avoid JOIN + DISTINCT when semi/anti semantics are intended.
Avoid NATURAL JOIN in generated SQL.
Avoid CROSS JOIN unless explicitly requested.
Always project/alias duplicate names after joins.
```

---

## 52.3 Equijoin keys vs join filters

SQL:

```sql id="t4dlwm"
SELECT *
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.id
 AND o.amount > c.minimum_amount
 AND c.region = 'us';
```

Planner split:

```text id="32esyv"
equijoin keys:
  o.customer_id = c.id

join filter:
  o.amount > c.minimum_amount

single-table predicate candidates:
  c.region = 'us'
    → push to customers before join if optimizer can separate it
```

Physical implication:

```text id="znvlnj"
Equijoin keys:
  enable HashJoinExec / SortMergeJoinExec

Non-equi join filters:
  can remain post-join filter
  may force NestedLoopJoinExec or PiecewiseMergeJoinExec for range-only joins

Single-table filters:
  should be pushed below join before physical planning
```

`HashJoinExec` represents equijoin predicates in its `on` list and evaluates non-equality predicates that cannot be pushed to inputs as post-join filter expressions. ([Docs.rs][2])

Agent rules:

```text id="wdxmuh"
Keep equijoin keys syntactically visible.
Do not hide keys inside functions/casts unless required.
Move single-table filters to WHERE or separate pre-join filters.
Alias join outputs explicitly.
```

---

## 52.4 Null semantics

Join null concerns:

```text id="onftit"
SQL equality:
  NULL = NULL -> NULL/unknown, not true

Inner equijoin:
  rows with NULL keys normally do not match

Outer join:
  unmatched side is null-extended

Semi join:
  left row retained if match exists

Anti join:
  null behavior depends on EXISTS / NOT EXISTS vs NOT IN semantics

Null-aware physical flags:
  physical join internals may expose null_equality / null_aware fields for specific join cases
```

`HashJoinExec` exposes fields such as `null_equality` and `null_aware`, reflecting that physical join null semantics are explicit and must match the logical query semantics. ([Docs.rs][2])

Agent rules:

```text id="8f4khj"
Do not rewrite NOT IN as anti join without NULL-semantics proof.
Do not coalesce join keys unless business semantics require it.
Do not assume NULL keys match.
Use IS NOT NULL filters explicitly if null-key elimination is intended.
```

---

## 52.5 Output schema and projection

Join output schema hazards:

```text id="3tymy1"
duplicate field names:
  a.id, b.id -> public "id", "id"

nullability widening:
  LEFT JOIN right fields become nullable

semi/anti output:
  only preserving side fields

projection:
  physical join may include projection over joined schema
```

Safe generated SQL:

```sql id="82x5bs"
SELECT
  o.id AS order_id,
  o.customer_id,
  c.id AS customer_id_joined,
  c.name AS customer_name,
  o.amount
FROM orders AS o
JOIN customers AS c
  ON o.customer_id = c.id;
```

Safe DataFrame shape:

```rust id="4j1ijv"
let joined = orders.join(
    customers,
    JoinType::Inner,
    &["customer_id"],
    &["id"],
    None,
)?;

let output = joined.select(vec![
    col("orders.id").alias("order_id"),
    col("orders.customer_id").alias("customer_id"),
    col("customers.id").alias("customer_id_joined"),
    col("customers.name").alias("customer_name"),
    col("orders.amount"),
])?;
```

Agent rule:

```text id="nvbomo"
Join planning does not end at algorithm choice.
Output identity must be normalized after join.
```

---

## 52.6 Join algorithm choice overview

| Algorithm                | Good for                                                  | Avoid when                                                    | Key metadata                                    |
| ------------------------ | --------------------------------------------------------- | ------------------------------------------------------------- | ----------------------------------------------- |
| `HashJoinExec`           | equijoins, small/medium build side, star schemas          | no equijoin keys, build side too large without spill          | row counts, build size, key cardinality, memory |
| `SortMergeJoinExec`      | equijoins where inputs are sorted or too large for memory | unsorted inputs with expensive sort cost, non-equi-only joins | ordering, sort cost, memory                     |
| `NestedLoopJoinExec`     | non-equi joins, fallback, small build side                | large × large joins                                           | row counts, filter selectivity, memory          |
| `PiecewiseMergeJoinExec` | single range comparison joins                             | complex predicates, unsorted buffered side                    | range predicate, ordering                       |
| `SymmetricHashJoinExec`  | stream-stream equijoin with bounded range/window filters  | unbounded joins without pruning/window                        | stream ordering, range conditions, state bounds |
| `CrossJoinExec`          | intentional Cartesian product                             | almost all generated/ad hoc queries                           | row counts, memory, explicit user intent        |

DataFusion’s join module documents these operator purposes: `HashJoinExec` evaluates equijoins in parallel with hash tables; `SortMergeJoinExec` executes equijoins using sort-merge and can handle arbitrarily large inputs when one or both inputs do not fit memory; `NestedLoopJoinExec` is intended for joins without equijoin keys; `PiecewiseMergeJoinExec` targets a single range filter; `SymmetricHashJoinExec` is designed for streaming joins with range conditions; and `CrossJoinExec` returns the Cartesian product. ([Docs.rs][1])

---

## 52.7 Hash join

### 52.7.1 Shape

```text id="m0w5lr"
HashJoinExec
  left  = build side
  right = probe side
  on    = equijoin keys
  filter = optional post-join filter
  join_type
  partition_mode
  projection
  null_equality
```

Execution model:

```text id="x1oqtw"
1. Build phase:
   read entire build side
   construct hash table on join keys
   concatenate build batches / maintain row indexes

2. Probe phase:
   stream probe side
   compute join-key hash
   lookup matching build rows
   apply optional filter
   emit joined rows
```

`HashJoinExec` docs state that build side is the first child, probe side is the second child, and placing the smaller input on the build side is “VERY important” to minimize hash-table construction work. ([Docs.rs][2])

### 52.7.2 Best use cases

```text id="2a313a"
fact.dim_id = dim.id
orders.customer_id = customers.id
events.user_id = users.id
integer/string equijoin keys
small filtered dimension table
star schema with large fact table as probe side
```

### 52.7.3 Perfect hash / ArrayMap optimization

`HashJoinExec` may use an `ArrayMap` / perfect-hash style optimization for single integer join keys when key range/density and build-side size constraints make it appropriate; the docs list constraints including one integer key, small/dense key range, build-side rows under `u32::MAX`, and compatible null-equality conditions. ([Docs.rs][2]) Config knobs include `datafusion.execution.perfect_hash_join_small_build_threshold` and `datafusion.execution.perfect_hash_join_min_key_density`, where density is calculated as rows divided by key-range size. ([Apache DataFusion][3])

### 52.7.4 Build/probe side rule

```text id="x071tv"
Build side:
  smaller
  filtered first
  projected narrow
  fits memory/spill budget
  ideally unique/low-dup dimension side

Probe side:
  larger stream
  fact side
  high row count
```

Star-schema physical shape:

```text id="2b10cc"
small_dim_1 build
  JOIN (
    small_dim_2 build
      JOIN (
        small_dim_3 build
          JOIN large_fact probe
      )
  )
```

Hash join docs describe right-deep join trees as optimal for classic star schema queries, keeping small dimension tables as build sides and the large fact table as the probe-side stream. ([Docs.rs][2])

---

## 52.8 Sort-merge join

### 52.8.1 Shape

```text id="m065eu"
SortMergeJoinExec
  left sorted by join keys
  right sorted by join keys
  equijoin predicates
  optional post-join filter
  join_type
```

### 52.8.2 Best use cases

```text id="v0crfo"
both inputs already sorted by join key
inputs too large for in-memory hash build
range/ordered source layout
merge-friendly lakehouse clustering
memory-limited execution
```

`SortMergeJoinExec` executes equijoin predicates using the sort-merge algorithm and can be used where one or both inputs do not fit available memory. ([Docs.rs][1]) DataFusion 54 reworked its filtered-join path: per-row match tracking for semi/anti/mark joins and batched deferred filtering for left/full outer joins replace the previous row-at-a-time correction logic, with large reported speedups (20–50x) when the streamed side is near-unique — see 52.32.

### 52.8.3 Tradeoff

```text id="mudx60"
Pros:
  memory-friendly for large inputs
  can exploit existing ordering
  avoids full build-side hash table

Cons:
  requires sorted inputs
  may need expensive SortExec
  sensitive to ordering metadata correctness
```

Agent rule:

```text id="0pd3gv"
Sort-merge join is attractive only if ordering already exists or sorting is cheaper than hash-build memory pressure.
```

---

## 52.9 Nested loop join

### 52.9.1 Shape

```text id="o8w7vf"
NestedLoopJoinExec
  left = buffered build side
  right = streamed probe side
  join filter evaluates pair combinations
```

`NestedLoopJoinExec` is a build-probe operator for joins without equijoin keys; it buffers the left input in memory and probes with right batches. Since DataFusion 54 it is memory-limited: when the memory budget is exceeded during left-side buffering (`ResourcesExhausted`) and disk spilling is available (a `DiskManager` configured on the `RuntimeEnv`), it falls back to a multi-pass strategy — buffer as many left rows as fit (one chunk), spill the right side to disk on the first pass, and re-read the spilled right side for each subsequent left chunk. All join types are supported in the fallback; RIGHT/FULL/RIGHT SEMI/RIGHT ANTI/RIGHT MARK joins track unmatched right rows with a global bitmap and replay the right side once at the end. Without a disk manager/memory budget the pre-54 behavior remains: the query fails if the left side cannot fit. ([Docs.rs][4]; `datafusion-physical-plan/src/joins/nested_loop_join.rs`)

### 52.9.2 Use cases

```text id="6sr0tu"
non-equi predicate:
  a.ts < b.ts

small left side:
  small configuration table against fact stream

fallback:
  predicate cannot become hash/sort/range-specialized join
```

### 52.9.3 Avoid

```text id="7yh7yz"
large left × large right
missing join predicate due to generated SQL bug
unselective non-equi predicates
unbounded left input
```

Agent rule:

```text id="gpweid"
Nested loop is a correctness fallback, not a default performance target.
If it appears unexpectedly, inspect ON predicates and equijoin-key extraction.
```

---

## 52.10 Piecewise merge join

### 52.10.1 Shape

```text id="07oa3b"
PiecewiseMergeJoinExec
  buffered side
  streamed side
  one range comparison:
    < | <= | > | >=
  sorted buffered input requirement
  streamed batches may be sorted during processing
```

`PiecewiseMergeJoinExec` is chosen for a single comparison filter such as `col0 < colb`; it requires sorted input on the buffered side and can perform much better than nested-loop joins for these range workloads. ([Docs.rs][5])

### 52.10.2 SQL examples

```sql id="bmydzi"
SELECT *
FROM a
JOIN b
  ON a.ts < b.ts;
```

```sql id="ew3j4g"
SELECT *
FROM a
JOIN b
  ON a.price >= b.threshold;
```

### 52.10.3 Planning rules

```text id="y5fd6a"
Single comparison filter only.
Buffered side must be globally sorted in required direction.
Planner/optimizer may insert SortExec on buffered side.
Streamed side may be sorted per batch during processing.
```

Agent rule:

```text id="xhyow7"
For range-only joins, keep the single comparison predicate simple and visible.
Do not wrap range keys in functions if PiecewiseMergeJoin is desired.
```

---

## 52.11 Symmetric hash join

### 52.11.1 Shape

```text id="l7eqv2"
SymmetricHashJoinExec
  left stream hash table
  right stream hash table
  equijoin keys
  range/window filter
  pruning of stale/unjoinable rows
```

`SymmetricHashJoinExec` is designed for streaming contexts: both streams are hashed on join keys, both maintain hash tables, and range conditions over sorted columns allow stale state to be pruned, enabling unbounded streaming joins without unbounded memory growth. ([Docs.rs][6])

### 52.11.2 Streaming SQL shape

```sql id="xp0xeo"
SELECT *
FROM left_stream AS l
JOIN right_stream AS r
  ON l.user_id = r.user_id
 AND l.event_ts > r.event_ts - INTERVAL '5 minutes'
 AND l.event_ts < r.event_ts + INTERVAL '5 minutes';
```

### 52.11.3 Required constraints

```text id="4bzpfj"
Equijoin key:
  l.user_id = r.user_id

Bounded state predicate:
  range/window condition on ordered/sortable fields

State pruning:
  conditions allow dropping rows outside joinable range

Ordering metadata:
  left/right sort expressions may be needed
```

Agent rule:

```text id="thzmna"
Stream-stream join without bounded pruning is a memory leak waiting to happen.
Require time/range constraints or reject at logical-plan admission.
```

The attached join section similarly warns to use symmetric hash joins for stream-stream joins only when pruning/window constraints exist and to inspect EXPLAIN for `SymmetricHashJoinExec`. 

---

## 52.12 Cross join

### 52.12.1 Shape

```sql id="j9o52q"
SELECT *
FROM a
CROSS JOIN b;
```

```text id="ujlw22"
CrossJoinExec:
  output rows = rows(a) × rows(b)
  buffers left input into memory
  streams right batches
  combines every right row with buffered left rows
```

`CrossJoinExec` is used when there are no predicates between two tables and returns the Cartesian product; it buffers the left side in memory and streams right-side batches. ([Docs.rs][7])

### 52.12.2 Agent policy

```text id="5s2nzx"
Generated SQL:
  deny cross join unless user explicitly asks for Cartesian product.

Logical lint:
  detect Join without keys/filter.
  detect SQL CrossJoin if present.

Performance:
  pre-filter/project both sides.
  cap exploratory queries.
  never use LIMIT as correctness protection.
```

---

## 52.13 Cost inputs

Join planning uses metadata quality. Unknown metadata is safe but less optimal; false metadata can produce poor algorithm choices and, if used as physical property truth, wrong results.

```text id="2o9i6u"
row counts:
  build/probe side selection
  join ordering
  cardinality estimates

byte sizes:
  memory budget
  spill risk
  build-table footprint

key cardinality / NDV:
  hash table size
  duplicate explosion
  join selectivity
  group cardinality after join

null counts:
  key-match selectivity
  null-aware semantics
  filter opportunities

ordering:
  sort-merge feasibility
  sort elimination
  piecewise merge feasibility

partitioning:
  repartition avoidance
  co-partitioned join feasibility
  target parallelism

memory budget:
  hash build feasibility
  nested-loop buffering risk
  spill behavior

source pushdown:
  pre-join filter selectivity
  dynamic filter usefulness
```

DataFusion configuration includes `target_partitions`, which controls query execution partitioning/concurrency, and join-related perfect-hash thresholds; file/table statistics collection is also controlled by `datafusion.execution.collect_statistics`. ([Apache DataFusion][3])

---

## 52.14 Cost/admission model for agents

```rust id="kjky1o"
#[derive(Debug, Clone)]
pub struct JoinCostInputs {
    pub left_rows: Option<usize>,
    pub right_rows: Option<usize>,
    pub left_bytes: Option<usize>,
    pub right_bytes: Option<usize>,
    pub left_key_ndv: Option<usize>,
    pub right_key_ndv: Option<usize>,
    pub left_key_nulls: Option<usize>,
    pub right_key_nulls: Option<usize>,
    pub left_ordered_on_keys: bool,
    pub right_ordered_on_keys: bool,
    pub left_partitioned_on_keys: bool,
    pub right_partitioned_on_keys: bool,
    pub memory_budget_bytes: Option<usize>,
}

#[derive(Debug, Clone)]
pub enum JoinRisk {
    Low,
    Medium,
    High,
    Reject,
}

pub fn estimate_join_risk(inputs: &JoinCostInputs, join_type: JoinType) -> JoinRisk {
    let left = inputs.left_rows.unwrap_or(usize::MAX / 4);
    let right = inputs.right_rows.unwrap_or(usize::MAX / 4);

    if left == usize::MAX / 4 || right == usize::MAX / 4 {
        return JoinRisk::Medium;
    }

    if left.saturating_mul(right) > 10_000_000_000 && !inputs.left_partitioned_on_keys {
        return JoinRisk::High;
    }

    if matches!(join_type, JoinType::Inner | JoinType::Left | JoinType::Right | JoinType::Full) {
        JoinRisk::Medium
    } else {
        JoinRisk::Low
    }
}
```

Agent rule:

```text id="vd1a1f"
Use cost model for admission and diagnostics.
Do not override DataFusion physical planner blindly unless you own the full workload model.
```

---

## 52.15 Repartitioning

### 52.15.1 Join-key repartition

```text id="eu3hmt"
Hash join requirement:
  rows with same join key must meet in same partition

If left/right not co-partitioned:
  RepartitionExec(Hash(keys, target_partitions)) inserted on one/both sides
```

Physical properties drive repartition insertion: child `output_partitioning` says what distribution exists, and parent `required_input_distribution` says what distribution is required. `ExecutionPlan` docs describe required input distributions/orderings as how physical plans communicate requirements to the optimizer. ([Docs.rs][8])

### 52.15.2 Coalescing

```text id="y4febe"
Too many tiny partitions/batches:
  coalesce batches/partitions to reduce overhead

Too few partitions:
  repartition to improve parallelism

Skewed keys:
  repartition alone may not solve skew
```

### 52.15.3 Broadcast-like patterns

DataFusion core join docs focus on hash/sort/nested/cross/symmetric/piecewise operators, not a universal “broadcast join” user surface. Agents building custom providers or physical rules can implement broadcast-like behavior by making a small side available to all partitions, but must treat it as a custom physical strategy with explicit memory and correctness tests.

```text id="e9w9fu"
Custom broadcast-like strategy:
  collect/build small side once
  share immutable hash table across probe partitions
  enforce small-side memory cap
  preserve join semantics
  expose metrics
```

Agent rule:

```text id="d26qyl"
Do not assume broadcast exists as a general DataFusion SQL knob.
Use build/probe side selection and partition modes available in pinned APIs.
Custom broadcast requires custom physical planning.
```

---

## 52.16 Build/probe side selection

### 52.16.1 Hash join build side

```text id="t73qhw"
HashJoinExec:
  left child = build side
  right child = probe side
```

Docs state that the smaller input should be on the build side because the build side is read into the hash table before probing. ([Docs.rs][2])

### 52.16.2 Selection rule

```text id="5jsj9z"
Prefer build side:
  fewer rows after filters
  fewer bytes after projection
  lower key duplication
  fits memory
  dimension/reference table
  unique/primary-key side
```

### 52.16.3 Build-side anti-pattern

```text id="bd0d65"
Bad:
  build = 10B row fact table
  probe = 10K row dim table

Good:
  build = 10K row dim table
  probe = 10B row fact table
```

### 52.16.4 Programmatic join normalization

```rust id="66f2sz"
#[derive(Debug, Clone)]
pub struct JoinSideStats {
    pub rows: Option<usize>,
    pub bytes: Option<usize>,
}

pub fn should_swap_for_hash_join(left: &JoinSideStats, right: &JoinSideStats) -> bool {
    match (left.bytes, right.bytes) {
        (Some(l), Some(r)) => l > r,
        _ => match (left.rows, right.rows) {
            (Some(l), Some(r)) => l > r,
            _ => false,
        },
    }
}
```

Agent warning:

```text id="qaqxfn"
Swapping inputs changes left/right outer join semantics unless join type is rewritten correctly.
Do not swap blindly.
```

---

## 52.17 Dynamic join filters

### 52.17.1 Concept

```text id="b92pm1"
Runtime build-side information
  → dynamic filter
  → probe/scan side pruning
  → fewer files/row groups/pages/rows decoded
```

DataFusion’s dynamic-filter blog describes runtime filters as a mechanism for operators such as TopK and joins to pass information to scan operators so scans can skip unnecessary rows, files, or parts of files; the blog reports order-of-magnitude improvements for some query patterns and describes join-derived dynamic filters as part of the design. ([Apache DataFusion][9])

### 52.17.2 Config

```sql id="jdvi3a"
SET datafusion.optimizer.enable_dynamic_filter_pushdown = true;
SET datafusion.optimizer.enable_join_dynamic_filter_pushdown = true;
```

Config docs list dynamic filter pushdown controls, including general dynamic filter pushdown and join-specific dynamic filter pushdown in current DataFusion-derived docs/search results. ([Docs.rs][10])

### 52.17.3 Good pattern

```sql id="ztk6gw"
SELECT *
FROM fact_events AS f
JOIN (
  SELECT id
  FROM dim_users
  WHERE status = 'active'
) AS d
ON f.user_id = d.id;
```

Runtime effect:

```text id="s7pjgp"
filtered dimension keys
  → dynamic user_id filter
  → fact scan prunes files/row groups/pages if metadata/index supports user_id pruning
```

### 52.17.4 Dynamic-filter usefulness

```text id="lisgwl"
Useful when:
  build side is selective
  probe side is large
  probe scan source supports pruning/indexing
  join key appears in Parquet stats/partitions/bloom filters
  custom source can accept dynamic filters

Less useful when:
  CSV/JSON full scan with no metadata
  build side nearly all keys
  join key has no source-level pruning metadata
```

The attached join section gives the same rule: dynamic join filters help only if the scan source can prune, and are strongest with Parquet statistics, partition columns, or indexed custom sources. 

---

## 52.18 Spill behavior and memory

### 52.18.1 Hash join memory

```text id="zclzfq"
Hash join memory consumers:
  build side batches
  join key hash table
  row indexes
  output buffers
  optional filter evaluation buffers
```

### 52.18.2 Cross/nested-loop memory

```text id="qelwie"
CrossJoinExec:
  buffers left input

NestedLoopJoinExec:
  eagerly buffers all left-side input until memory limit, currently errors if it cannot fit
```

Cross join and nested-loop docs both emphasize left/build-side buffering. ([Docs.rs][7])

### 52.18.3 Sort-merge memory

```text id="rdpxzz"
SortMergeJoinExec:
  avoids full build-side hash table
  requires sorted inputs
  sort operators may spill
  can be preferable when inputs exceed memory
```

`SortMergeJoinExec` docs explicitly say it can be used to join arbitrarily large inputs where one or both inputs do not fit available memory. ([Docs.rs][1])

### 52.18.4 Runtime configuration

```rust id="7gc0zk"
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::session_state::SessionStateBuilder;
use datafusion::prelude::*;

let runtime = RuntimeEnvBuilder::new()
    .with_memory_limit(16 * 1024 * 1024 * 1024, 0.80)
    .with_temp_file_path("/mnt/datafusion-spill")
    .build_arc()?;

let state = SessionStateBuilder::new()
    .with_runtime_env(runtime)
    .with_default_features()
    .build();

let ctx = SessionContext::new_with_state(state);
```

Agent rules:

```text id="z9gqky"
For large joins:
  configure memory limit
  configure spill/temp directory
  monitor spill metrics
  prefer sort-merge or repartitioned hash strategies when appropriate
  filter/project before join
```

---

## 52.19 Lateral and correlated join planning

### 52.19.1 Correlated EXISTS

```sql id="92gayk"
SELECT *
FROM users AS u
WHERE EXISTS (
  SELECT 1
  FROM orders AS o
  WHERE o.user_id = u.id
);
```

Preferred logical rewrite:

```text id="q39441"
EXISTS correlated subquery
  → LeftSemi join
```

### 52.19.2 Correlated NOT EXISTS

```sql id="tvlw4q"
SELECT *
FROM users AS u
WHERE NOT EXISTS (
  SELECT 1
  FROM orders AS o
  WHERE o.user_id = u.id
);
```

Preferred logical rewrite:

```text id="0dtdio"
NOT EXISTS correlated subquery
  → LeftAnti join
```

### 52.19.3 Lateral join

```sql id="b0asrm"
SELECT d.name, e.name
FROM departments AS d,
LATERAL (
  SELECT employees.name
  FROM employees
  WHERE employees.dept_id = d.id
) AS e;
```

DataFusion SQL docs require the `LATERAL` keyword for right-side subqueries that reference columns from the left side; they do not implicitly detect correlated `FROM` subqueries, and documented lateral limitations include no outer references in the lateral subquery `SELECT` list or `HAVING`. ([Docs.rs][1])

DataFusion 54 added basic support for `CROSS JOIN LATERAL`, `INNER JOIN LATERAL`, and `LEFT JOIN LATERAL`. The logical optimizer's `DecorrelateLateralJoin` rule (`datafusion-optimizer/src/decorrelate_lateral_join.rs`) rewrites the lateral join into ordinary relational join execution; the plan never executes a per-left-row re-evaluation loop. Support is still basic — laterals that resist decorrelation (for example the documented outer-reference positions above) are rejected at planning time, so treat LATERAL as sugar for decorrelatable shapes, not as a general correlated-execution engine.

Agent rules:

```text id="9sqeka"
Use EXISTS → semi join.
Use NOT EXISTS → anti join.
Avoid NOT IN rewrite unless NULL semantics are proven.
Use LATERAL explicitly for FROM-subquery correlation.
Prefer decorrelated join forms in generated SQL when possible.
```

---

## 52.20 Query-shape rewrites

### 52.20.1 `EXISTS` → semi join

```sql id="nkv4ta"
-- Less direct
SELECT *
FROM users u
WHERE EXISTS (
  SELECT 1 FROM orders o WHERE o.user_id = u.id
);

-- Direct generated form
SELECT u.*
FROM users u
LEFT SEMI JOIN orders o
  ON o.user_id = u.id;
```

### 52.20.2 `NOT EXISTS` → anti join

```sql id="n0ec56"
SELECT u.*
FROM users u
LEFT ANTI JOIN orders o
  ON o.user_id = u.id;
```

### 52.20.3 Filter before join

Bad:

```sql id="7db11a"
SELECT *
FROM fact f
JOIN dim d ON f.dim_id = d.id
WHERE d.status = 'active'
  AND f.event_date = DATE '2026-05-22';
```

Better generated shape:

```sql id="y54oso"
WITH f AS (
  SELECT *
  FROM fact
  WHERE event_date = DATE '2026-05-22'
),
d AS (
  SELECT *
  FROM dim
  WHERE status = 'active'
)
SELECT *
FROM f
JOIN d ON f.dim_id = d.id;
```

Optimizers often push filters automatically, but generated SQL should still expose pushdown-friendly structure.

### 52.20.4 Cast keys before join

Bad:

```sql id="xzlw4q"
ON CAST(f.dim_id AS BIGINT) = d.id
```

Better:

```sql id="5fuue4"
WITH f_norm AS (
  SELECT CAST(dim_id AS BIGINT) AS dim_id_i64, *
  FROM fact
)
SELECT *
FROM f_norm f
JOIN dim d
  ON f.dim_id_i64 = d.id;
```

Agent rule:

```text id="3n8ow7"
Normalize/cast join keys before join.
Keep ON equality simple.
```

### 52.20.5 Avoid functions around join keys

Bad:

```sql id="49jd68"
ON lower(a.email) = lower(b.email)
```

Better:

```sql id="fiekqf"
WITH a_norm AS (
  SELECT lower(email) AS email_norm, *
  FROM a
),
b_norm AS (
  SELECT lower(email) AS email_norm, *
  FROM b
)
SELECT *
FROM a_norm a
JOIN b_norm b
  ON a.email_norm = b.email_norm;
```

Why:

```text id="i3gza4"
clear key columns
better projection planning
possible source/materialized index use
simpler physical join keys
easier statistics/NDV association
```

---

## 52.21 Explain diagnostics

### 52.21.1 SQL

```sql id="f8e4ux"
EXPLAIN VERBOSE
SELECT *
FROM fact f
JOIN dim d
  ON f.dim_id = d.id
WHERE d.status = 'active';
```

```sql id="v38dk5"
EXPLAIN ANALYZE
SELECT *
FROM fact f
JOIN dim d
  ON f.dim_id = d.id
WHERE d.status = 'active';
```

`EXPLAIN` shows logical and physical plans; `EXPLAIN VERBOSE` provides more detail, and `EXPLAIN ANALYZE` runs the statement and includes runtime metrics. ([Apache DataFusion][11])

### 52.21.2 Rust

```rust id="nlge43"
use datafusion::physical_plan::displayable;
use datafusion::prelude::*;

pub async fn inspect_join_plan(ctx: &SessionContext, sql: &str) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;

    println!("logical:\n{}", df.logical_plan().display_indent_schema());

    let physical = df.create_physical_plan().await?;
    println!("physical:\n{}", displayable(physical.as_ref()).indent(true));

    Ok(())
}
```

### 52.21.3 What to look for

```text id="f96bjy"
Physical operator:
  HashJoinExec
  SortMergeJoinExec
  NestedLoopJoinExec
  PiecewiseMergeJoinExec
  SymmetricHashJoinExec
  CrossJoinExec

Join keys:
  on=[...]
  equijoin predicates visible

Join filter:
  filter=...
  non-equi predicates not pushed to keys

Repartition:
  RepartitionExec around join inputs

Sort:
  SortExec before SortMergeJoinExec / PiecewiseMergeJoinExec

Build/probe:
  left/right input order for HashJoinExec

Metrics:
  output_rows
  elapsed_compute
  output_batches
  memory/spill/operator-specific metrics where exposed
```

DataFusion metrics docs state that operators expose runtime metrics for understanding time spent and data flow, with baseline metrics such as `elapsed_compute`, `output_rows`, `output_bytes`, and `output_batches`; `EXPLAIN ANALYZE` is the main user-facing path to inspect them. ([Apache DataFusion][12])

---

## 52.22 Join metrics model

Join-specific metrics can vary by DataFusion version/operator. Agent-side diagnostic schema should be flexible:

```rust id="55k2fg"
#[derive(Debug, Clone, Default)]
pub struct JoinMetricsSummary {
    pub operator: String,
    pub join_type: String,
    pub output_rows: Option<usize>,
    pub output_batches: Option<usize>,
    pub elapsed_compute_ms: Option<f64>,

    pub build_input_rows: Option<usize>,
    pub probe_input_rows: Option<usize>,
    pub build_time_ms: Option<f64>,
    pub probe_time_ms: Option<f64>,

    pub spill_count: Option<usize>,
    pub spill_bytes: Option<usize>,
    pub memory_peak_bytes: Option<usize>,

    pub repartition_count: Option<usize>,
    pub dynamic_filter_rows_pruned: Option<usize>,
}
```

Metric interpretation:

```text id="zdu8e2"
High build time:
  build side too large, slow hash, skew, string keys, insufficient projection

High probe time:
  large probe side, weak filtering, poor dynamic filtering, skew

High output_rows:
  many-to-many explosion, duplicate keys, join predicate too broad

Spill metrics:
  build side/memory pressure, sort/merge pressure, temp disk bottleneck

Repartition time:
  incompatible partitioning, high network/memory shuffle cost

Tiny output with huge input:
  consider pushdown, dynamic filters, pre-filtering, bloom/partition layout
```

---

## 52.23 Physical-plan classification helper

```rust id="16x8fp"
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinExecKind {
    Hash,
    SortMerge,
    NestedLoop,
    PiecewiseMerge,
    SymmetricHash,
    Cross,
    Unknown(String),
}

pub fn classify_join_exec_name(name: &str) -> JoinExecKind {
    match name {
        "HashJoinExec" | "HashJoin" => JoinExecKind::Hash,
        "SortMergeJoinExec" | "SortMergeJoin" => JoinExecKind::SortMerge,
        "NestedLoopJoinExec" | "NestedLoopJoin" => JoinExecKind::NestedLoop,
        "PiecewiseMergeJoinExec" | "PiecewiseMergeJoin" => JoinExecKind::PiecewiseMerge,
        "SymmetricHashJoinExec" | "SymmetricHashJoin" => JoinExecKind::SymmetricHash,
        "CrossJoinExec" | "CrossJoin" => JoinExecKind::Cross,
        other => JoinExecKind::Unknown(other.to_string()),
    }
}
```

Use for diagnostics only; do not couple production correctness to display names without version-pinned tests.

---

## 52.24 Join planning lint rules

```text id="ef573d"
PLAN_CROSS_JOIN_DENIED:
  CrossJoinExec or predicate-less join in public service

PLAN_NESTED_LOOP_LARGE:
  NestedLoopJoinExec with large/unknown inputs

PLAN_JOIN_MISSING_EQUIKEY:
  join has no equality keys but appears intended as relational join

PLAN_JOIN_FUNCTION_KEY:
  ON lower(a.x) = lower(b.x) or CAST wrappers; recommend precompute key

PLAN_JOIN_BUILD_SIDE_LARGE:
  HashJoinExec build side larger than probe side

PLAN_JOIN_SORT_REQUIRED:
  SortMergeJoinExec/PiecewiseMergeJoinExec requires expensive inserted SortExec

PLAN_JOIN_DYNAMIC_FILTER_UNHELPFUL:
  dynamic filters enabled but source cannot prune

PLAN_STREAM_JOIN_UNBOUNDED:
  stream-stream join without range/window pruning
```

Example policy:

```rust id="5q2e3t"
pub struct JoinPolicy {
    pub allow_cross_join: bool,
    pub allow_nested_loop_large_unknown: bool,
    pub require_equi_keys_for_public_queries: bool,
    pub max_estimated_join_output_rows: Option<usize>,
    pub require_stream_join_range_bound: bool,
}
```

---

## 52.25 Join tuning workflow

```text id="6cjrp5"
Step 1: Inspect logical plan
  Are filters below joins?
  Are equijoin keys visible?
  Are join types intended?
  Are semi/anti joins used for existence?

Step 2: Inspect physical plan
  Which join exec?
  Are SortExec/RepartitionExec inserted?
  Which side is build/probe?
  Is CrossJoin/NestedLoop unexpected?

Step 3: Inspect metadata
  row counts
  byte sizes
  key NDV
  null counts
  ordering
  partitioning
  constraints

Step 4: Rewrite query shape
  pre-filter
  project narrow columns
  cast/precompute keys
  use semi/anti
  remove functions around keys

Step 5: Tune source/layout/config
  partition by common keys/date
  sort/cluster if sort-merge/range joins common
  collect stats
  enable Parquet pruning/bloom
  tune target_partitions
  memory/spill directory

Step 6: Benchmark with EXPLAIN ANALYZE
  compare wall time
  join elapsed_compute
  input/output rows
  bytes
  repartition/sort/spill metrics
  correctness
```

The attached performance section already indexes joins to stats, build side, repartitioning, hash-vs-sort-merge, and dynamic join filters as primary/secondary tuning knobs. 

---

## 52.26 Generated SQL best practices

```text id="79g5pt"
Always:
  explicit JOIN ... ON
  simple equality keys where possible
  qualify columns
  alias output duplicates
  filter before join
  project before join
  use semi/anti for existence
  cast/precompute join keys before join

Avoid:
  NATURAL JOIN
  CROSS JOIN unless explicit
  functions around join keys
  implicit comma joins
  unbounded stream joins
  many-to-many joins without cardinality expectation
  NOT IN rewrites without NULL proof
```

Example generated join template:

```sql id="09y4iu"
WITH
  f AS (
    SELECT
      dim_id,
      event_date,
      amount
    FROM fact_events
    WHERE event_date BETWEEN DATE '2026-05-01' AND DATE '2026-05-31'
  ),
  d AS (
    SELECT
      id,
      category
    FROM dim
    WHERE status = 'active'
  )
SELECT
  f.event_date,
  d.category,
  SUM(f.amount) AS total_amount
FROM f
JOIN d
  ON f.dim_id = d.id
GROUP BY
  f.event_date,
  d.category
ORDER BY
  f.event_date,
  d.category;
```

---

## 52.27 DataFrame join best practices

```rust id="4lnniv"
use datafusion::functions_aggregate::expr_fn::sum;
use datafusion::logical_expr::JoinType;
use datafusion::prelude::*;

pub async fn fact_dim_rollup(ctx: &SessionContext) -> datafusion::error::Result<DataFrame> {
    let fact = ctx.table("fact_events").await?
        .filter(col("event_date").gt_eq(lit("2026-05-01")))?
        .filter(col("event_date").lt_eq(lit("2026-05-31")))?
        .select(vec![
            col("dim_id"),
            col("event_date"),
            col("amount"),
        ])?;

    let dim = ctx.table("dim").await?
        .filter(col("status").eq(lit("active")))?
        .select(vec![
            col("id"),
            col("category"),
        ])?;

    let joined = fact.join(
        dim,
        JoinType::Inner,
        &["dim_id"],
        &["id"],
        None,
    )?;

    joined
        .aggregate(
            vec![col("event_date"), col("category")],
            vec![sum(col("amount")).alias("total_amount")],
        )?
        .sort(vec![
            col("event_date").sort(true, true),
            col("category").sort(true, true),
        ])
}
```

Agent rule:

```text id="zsrjzj"
DataFrame API makes pre-filter/pre-project obvious and avoids generated SQL string risks.
```

---

## 52.28 Deployment advisory

```text id="15pi40"
Public SQL service:
  deny cross joins by logical lint.
  require LIMIT for exploratory joins.
  require partition/date filters for large fact joins.
  disallow nested-loop joins over unknown-large inputs.
  stream outputs; never collect arbitrary joins.

Batch analytics:
  allow large joins through batch queue.
  set memory/spill directories.
  inspect EXPLAIN ANALYZE.
  collect stats and tune table layout.

Lakehouse/Parquet:
  partition by coarse filters, not high-cardinality join keys by default.
  cluster/sort by common join/order keys if workload justifies.
  enable pruning/page index/bloom where useful.
  dynamic filters help only if scan layout can exploit them.

Custom sources:
  expose row counts/byte sizes/key stats when trustworthy.
  expose partitioning/order only if guaranteed.
  accept dynamic filters if source index can prune.
  expose metrics for remote requests and join-side pruning.

Streaming:
  require bounded range/window conditions.
  prefer symmetric hash join only with state-pruning predicates.
  monitor buffer growth.
```

---

## 52.29 Testing matrix

```text id="zzlnsy"
Logical:
  [ ] join type correct
  [ ] equijoin keys extracted
  [ ] residual join filter preserved
  [ ] single-table filters pushed below join
  [ ] semi/anti rewrite semantics
  [ ] NOT IN null semantics negative test
  [ ] lateral/correlated subquery decorrelation

Physical:
  [ ] expected HashJoinExec for equijoin
  [ ] expected SortMergeJoinExec when sorted/large
  [ ] expected NestedLoopJoinExec for non-equi fallback
  [ ] expected PiecewiseMergeJoinExec for single range predicate
  [ ] expected SymmetricHashJoinExec for bounded stream join
  [ ] CrossJoinExec denied/allowed intentionally
  [ ] RepartitionExec inserted when inputs not co-partitioned
  [ ] SortExec inserted when sort-merge/range requires order

Cost:
  [ ] small side is build side for hash join
  [ ] row count/byte stats change join choice as expected
  [ ] null-heavy keys behave correctly
  [ ] skewed keys tested

Runtime:
  [ ] EXPLAIN ANALYZE join metrics captured
  [ ] spill scenario tested
  [ ] memory-limit behavior tested
  [ ] dynamic filters improve only on prunable source
  [ ] output correctness under multi-partition execution
```

---

## 52.30 Anti-pattern inventory

* Generated `CROSS JOIN` from missing `ON`.
* `JOIN + DISTINCT` used where `SEMI JOIN` is intended.
* Rewriting `NOT IN` to anti join without NULL analysis.
* Functions around join keys preventing key extraction/index use.
* Casting join keys inside `ON` instead of pre-normalizing.
* Large fact table placed as hash-build side.
* No projection before join, causing wide hash tables.
* No filter before join, causing avoidable cardinality.
* Trusting stale row counts for build/probe decisions.
* False source ordering causing invalid sort-merge planning.
* Stream-stream join without bounded range predicate.
* Nested-loop join over unknown-large inputs in public endpoint.
* Dynamic filters enabled without source pruning support and then assumed helpful.
* Physical plan snapshots used across DataFusion upgrades without pinning.
* Interpreting `LIMIT` after join as protection against Cartesian explosion.

---

## 52.31 Agent checklist

```text id="g3mew2"
[ ] Identify logical join:
    join_type
    left/right input
    equijoin keys
    join filter
    null semantics
    output schema

[ ] Normalize query shape:
    filters before join
    projection before join
    simple equality keys
    precomputed/cast keys
    semi/anti for EXISTS/NOT EXISTS
    no NATURAL/CROSS unless explicit

[ ] Choose/inspect algorithm:
    HashJoinExec for equijoin
    SortMergeJoinExec for sorted/large equijoin
    NestedLoopJoinExec for non-equi fallback/small build
    PiecewiseMergeJoinExec for single range comparison
    SymmetricHashJoinExec for bounded stream-stream joins
    CrossJoinExec only for intended Cartesian product

[ ] Use cost inputs:
    row counts
    byte sizes
    key NDV
    null counts
    ordering
    partitioning
    memory budget
    source pruning ability

[ ] Repartition:
    check join-key co-partitioning
    expect RepartitionExec if not satisfied
    avoid false partitioning claims

[ ] Build/probe:
    smaller/narrower side as hash build
    preserve outer join semantics when swapping
    inspect physical left/right inputs

[ ] Dynamic filters:
    enable/configure deliberately
    source must support pruning
    benchmark with real file layout

[ ] Spill/memory:
    configure RuntimeEnv memory/temp dir
    monitor spill bytes and peak memory
    prefer sort-merge or pre-filtering when hash build is too large

[ ] Diagnostics:
    EXPLAIN VERBOSE
    EXPLAIN ANALYZE
    join physical operator
    repartition/sort nodes
    output rows
    elapsed compute
    spill metrics where exposed

[ ] Test:
    result equivalence
    null-key behavior
    semi/anti semantics
    cross-join denial
    large/skewed inputs
    multi-partition correctness
```

[1]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/index.html "datafusion::physical_plan::joins - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.HashJoinExec.html "HashJoinExec in datafusion::physical_plan::joins - Rust"
[3]: https://datafusion.apache.org/user-guide/configs.html "Configuration Settings — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.NestedLoopJoinExec.html "NestedLoopJoinExec in datafusion::physical_plan::joins - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.PiecewiseMergeJoinExec.html "PiecewiseMergeJoinExec in datafusion::physical_plan::joins - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.SymmetricHashJoinExec.html "SymmetricHashJoinExec in datafusion::physical_plan::joins - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.CrossJoinExec.html "CrossJoinExec in datafusion::physical_plan::joins - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"
[9]: https://datafusion.apache.org/blog/2025/09/10/dynamic-filters/ "Dynamic Filters: Passing Information Between Operators During Execution for 25x Faster Queries - Apache DataFusion Blog"
[10]: https://docs.rs/deltalake/latest/deltalake/datafusion/common/config/struct.OptimizerOptions.html?utm_source=chatgpt.com "OptimizerOptions in deltalake::datafusion::common::config"
[11]: https://datafusion.apache.org/user-guide/sql/explain.html "EXPLAIN — Apache DataFusion  documentation"
[12]: https://datafusion.apache.org/user-guide/metrics.html "Metrics — Apache DataFusion  documentation"


## 52.32 DataFusion 54 join execution changes

### 52.32.1 Memory-limited nested loop join

`NestedLoopJoinExec` no longer simply fails when the left (build) side exceeds memory: with a `DiskManager` configured and a memory budget in place, it falls back to a chunked multi-pass strategy that spills the right side and replays it per left chunk, for all join types (details in 52.9.1). Planning consequence: NLJ admission policies that hard-denied "build side larger than memory" can relax to a cost warning when spilling is available — at multi-pass I/O cost.

### 52.32.2 Sort-merge join: per-row match tracking and deferred filtering

`SortMergeJoinExec` with a post-join filter was rewritten (`datafusion-physical-plan/src/joins/sort_merge_join/`):

```text id="smj54"
semi / anti / mark joins:
  per-row match tracking replaces row-at-a-time filter correction —
  a streamed row is settled as soon as its match state is decided

left / full outer joins with filters:
  deferred filtering batches the filter evaluation and corrects the
  match bitmap per batch instead of per row

reported effect:
  20–50x on filtered SMJ workloads where the streamed side is
  near-unique on the join keys
```

This narrows the historical "avoid filtered SMJ" guidance: filtered semi/anti/mark and outer SMJ shapes are now competitive when inputs are already sorted.

### 52.32.3 `RepartitionExec` batch coalescing

`RepartitionExec` coalesces small batches (per-output `LimitedBatchCoalescer`) before pushing them into the distributor channels, so hash-repartitioned join inputs arrive as fewer, fuller batches; preserve-order mode skips the shared coalescer. This mostly removes the "hash repartition fragments batches before the join" pathology from 52.15 tuning work.

### 52.32.4 Statistics-driven join input swapping: `optimizer.join_reordering`

New key `datafusion.optimizer.join_reordering = true` (default on): the physical optimizer may swap join inputs based on statistics (choosing the smaller side as build). Set to `false` to disable statistics-driven input swapping and keep the query's join order — useful when provider statistics are untrustworthy or when goldens must pin exact build/probe sides (52.16). Its effectiveness depends directly on the NDV/cardinality propagation described in 47.24.

### 52.32.5 LATERAL join decorrelation

CROSS/INNER/LEFT `JOIN LATERAL` are supported and decorrelated into ordinary relational joins by the `DecorrelateLateralJoin` logical rule — see 52.19.3 for syntax, limitations, and the basic-support caveat.

---

# DataFusion Advanced — 53) Streaming topology, boundedness, and pipeline-breaker planning

## 53.0 Purpose

Recast streaming execution as a **physical-plan topology concern**, not merely an output API choice:

```text id="9l7voi"
LogicalPlan
  → optimized LogicalPlan
  → ExecutionPlan DAG
      ├─ partitioned physical operators
      ├─ SendableRecordBatchStream per output partition
      ├─ RecordBatch-at-a-time polling
      ├─ boundedness propagation
      ├─ emission behavior
      ├─ pipeline-friendly operators
      └─ pipeline breakers / large-state operators
  → execute_stream / execute_stream_partitioned / collect / write_*
```

DataFusion’s core docs describe it as a streaming query engine: `ExecutionPlan`s incrementally read inputs and compute output one `RecordBatch` at a time by polling `SendableRecordBatchStream`s; output and intermediate batches are approximately `batch_size` rows to amortize per-batch overhead. ([Docs.rs][1]) The attached documentation already treats boundedness and pipeline behavior as physical-plan properties exposed through `ExecutionPlanProperties`, and warns that incorrectly marking unbounded or pipeline-breaking behavior can break unbounded execution planning. 

---

## 53.1 Physical plan as streaming topology

```text id="8i8ohj"
ExecutionPlan tree:
  root
    ├─ child ExecutionPlan
    │    ├─ child stream partition 0
    │    ├─ child stream partition 1
    │    └─ ...
    └─ child ExecutionPlan

Runtime:
  execute(root, partition_i, TaskContext)
    → SendableRecordBatchStream
      → poll_next()
        → poll child streams
        → receive RecordBatch
        → transform / buffer / combine
        → emit RecordBatch
```

`ExecutionPlan::execute` returns an async stream that incrementally computes a single output partition. It should generally produce work when the returned stream is polled, not eagerly materialize the full result before returning. ([Docs.rs][2])

Topology terms:

```text id="2g1yfw"
node:
  one ExecutionPlan operator

edge:
  stream of RecordBatch values from child to parent

partition:
  one independently executable output stream

batch:
  bounded chunk of columnar Arrow arrays

pipeline:
  chain of operators that can emit as input arrives

pipeline breaker:
  operator that must consume large/all input before output

backpressure:
  downstream poll rate controls upstream work
```

---

## 53.2 Partition streams

### 53.2.1 Execution partitions

```text id="fe9z1y"
ExecutionPlan.output_partitioning = N partitions

Runtime calls:
  execute(0, task_ctx)
  execute(1, task_ctx)
  ...
  execute(N-1, task_ctx)

Each call returns:
  SendableRecordBatchStream
```

A physical plan with multiple output partitions exposes multiple independent streams; this is how DataFusion maps parallelism to the async runtime. DataFusion’s docs state that execution plans process one `RecordBatch` at a time and multiple batches can be processed concurrently on different CPU cores for plans with multiple partitions. ([Docs.rs][3])

### 53.2.2 Partitioned stream execution

```rust id="uo2bsi"
use datafusion::prelude::*;
use futures::StreamExt;

pub async fn consume_partitioned(df: DataFrame) -> datafusion::error::Result<()> {
    let streams = df.execute_stream_partitioned().await?;

    for (partition, mut stream) in streams.into_iter().enumerate() {
        while let Some(batch) = stream.next().await {
            let batch = batch?;
            println!("partition={partition}, rows={}", batch.num_rows());
        }
    }

    Ok(())
}
```

Use partitioned execution when:

```text id="jz02n8"
preserving partition identity matters
writing partitioned output
custom scheduling across workers
debugging skew
tracking per-partition metrics
parallel downstream consumers can preserve partition semantics
```

Use single merged stream when:

```text id="7lxb5p"
HTTP streaming one response
Arrow Flight stream
CLI display
simple service API
consumer does not care about partition identity
```

---

## 53.3 `SendableRecordBatchStream`

Type shape:

```rust id="uf7btr"
pub type SendableRecordBatchStream =
    std::pin::Pin<
        Box<
            dyn datafusion::physical_plan::RecordBatchStream<
                Item = Result<
                    datafusion::arrow::record_batch::RecordBatch,
                    datafusion::error::DataFusionError
                >
            > + Send
        >
    >;
```

Docs.rs defines `SendableRecordBatchStream` as a pinned boxed stream of `RecordBatch` results that can be sent across threads and is used to retrieve results from execution plan nodes. ([Docs.rs][4])

Custom stream obligations:

```text id="8p32gb"
schema() available before first batch
every emitted batch matches schema
finite stream eventually returns None
unbounded stream may never return None
errors emitted as Err(DataFusionError)
stream respects cancellation/drop
stream avoids unbounded buffering
stream does not block Tokio worker threads
```

---

## 53.4 Backpressure

### 53.4.1 Pull-based execution

```text id="shpeko"
consumer.poll_next()
  → root stream polls child streams
    → child stream polls its child streams
      → source stream reads next file/page/remote page
  → one RecordBatch moves upward
```

Backpressure emerges because upstream work should occur primarily when downstream consumers poll the stream. A DataFusion cancellation article describes the runtime loop as: compile to `ExecutionPlan`, call `ExecutionPlan::execute`, receive `SendableRecordBatchStream`, then repeatedly call `Stream::poll_next` to get results. ([Apache DataFusion][5])

### 53.4.2 Backpressure-safe custom source

```rust id="1hsza8"
use std::{
    pin::Pin,
    task::{Context, Poll},
};

use datafusion::arrow::{datatypes::SchemaRef, record_batch::RecordBatch};
use datafusion::error::{DataFusionError, Result};
use datafusion::physical_plan::RecordBatchStream;

pub struct PagedApiStream {
    schema: SchemaRef,
    state: PagedApiState,
}

impl RecordBatchStream for PagedApiStream {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
}

impl futures::Stream for PagedApiStream {
    type Item = Result<RecordBatch>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        // Correct shape:
        // - if no in-flight request and demand exists, start one request
        // - poll in-flight request
        // - decode one/bounded number of batches
        // - emit at most one batch per poll_next
        // - do not prefetch unbounded pages
        // - return Pending while I/O is pending
        todo!("poll remote page fetch and emit RecordBatch")
    }
}

pub struct PagedApiState {
    pub continuation: Option<String>,
    pub done: bool,
    pub in_flight_request: Option<tokio::task::JoinHandle<Result<RecordBatch>>>,
}
```

Backpressure rules:

```text id="ld8cwv"
Do:
  fetch on demand
  use bounded prefetch
  use bounded channels for blocking bridges
  stop producer when consumer drops stream
  batch rows before emitting

Do not:
  spawn unbounded background fetches
  read all remote pages before first output
  block Tokio worker threads
  do one remote call per output row
  ignore cancellation
```

---

## 53.5 Batch size

DataFusion uses `batch_size`-sized `RecordBatch` chunks to amortize per-batch overhead while keeping memory bounded for streaming operators. ([Docs.rs][1])

### 53.5.1 Configuration

```rust id="ft9kv3"
use datafusion::prelude::*;

let config = SessionConfig::new()
    .with_batch_size(8192)
    .with_target_partitions(8);

let ctx = SessionContext::new_with_config(config);
```

SQL:

```sql id="xbg4z5"
SET datafusion.execution.batch_size = '8192';
SET datafusion.execution.target_partitions = '8';
```

### 53.5.2 Batch-size tradeoff

| Smaller batches                  | Larger batches                    |
| -------------------------------- | --------------------------------- |
| lower per-batch latency          | lower per-row overhead            |
| more scheduler/stream overhead   | better vectorization amortization |
| finer cancellation granularity   | larger temporary arrays           |
| more network chunks              | fewer network chunks              |
| useful for interactive streaming | useful for throughput scans       |

Agent policy:

```text id="tpyo6i"
Do not assume exact batch size.
Operators may emit smaller/larger batches.
Always code consumers against arbitrary RecordBatch sizes, including zero-row or final short batches.
```

---

## 53.6 Bounded vs unbounded source propagation

### 53.6.1 Source classification

```text id="wfuny7"
Bounded:
  finite files
  finite object-store dataset
  finite database query
  in-memory RecordBatch collection
  finite REST export
  finite generated data

Unbounded:
  Kafka topic
  message queue
  WebSocket feed
  tailing log
  streaming table
  long-running RPC stream
```

`CREATE UNBOUNDED EXTERNAL TABLE` marks a data source as unbounded; DataFusion attempts to execute queries over that source in streaming fashion and fails plan generation if the query cannot execute in streaming mode. The docs explicitly state that supported unbounded-source queries are a subset of bounded-source queries. ([Docs.rs][1])

### 53.6.2 Custom source properties

```rust id="dcv92b"
use std::sync::Arc;

use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::{
    execution_plan::{Boundedness, EmissionType, PlanProperties},
    Partitioning,
};

pub fn unbounded_stream_properties(
    schema: datafusion::arrow::datatypes::SchemaRef,
    partitions: usize,
) -> Arc<PlanProperties> {
    Arc::new(PlanProperties::new(
        EquivalenceProperties::new(schema),
        Partitioning::UnknownPartitioning(partitions),
        EmissionType::Incremental,
        Boundedness::Unbounded,
    ))
}
```

### 53.6.3 Propagation intuition

| Operator            | Bounded input              | Unbounded input                                    |
| ------------------- | -------------------------- | -------------------------------------------------- |
| projection          | bounded                    | unbounded                                          |
| filter              | bounded                    | unbounded                                          |
| simple map/UDF      | bounded                    | unbounded, if per-row and deterministic enough     |
| limit               | bounded                    | can become bounded if finite `fetch` is reachable  |
| full sort           | bounded but blocking       | usually unsupported / never emits final order      |
| global aggregate    | bounded but blocking       | usually unsupported / final result never completes |
| distinct            | bounded but stateful       | unbounded state unless windowed/bounded            |
| hash join           | bounded if both bounded    | stateful; requires bounded state strategy          |
| symmetric hash join | bounded/unbounded possible | viable only with pruning/range constraints         |
| window              | bounded possible           | streaming-safe only for bounded/windowed frames    |

Agent rule:

```text id="2mzwx8"
Unboundedness must not be hidden.
A pipeline can be streaming and still have state.
A query can return a stream and still contain blocking operators.
```

---

## 53.7 Pipeline-friendly operators

### 53.7.1 Projection

```text id="rw5db5"
RecordBatch in
  → evaluate expressions columnar
  → RecordBatch out
```

Streaming properties:

```text id="4d5geo"
row-preserving
incremental
preserves boundedness
usually preserves relative order
low state
```

### 53.7.2 Filter

```text id="rl9rjw"
RecordBatch in
  → evaluate BooleanArray predicate
  → filter rows
  → RecordBatch out
```

Streaming properties:

```text id="7ou5fv"
row-dropping
incremental
preserves relative order
low state
predicate null/false rows removed
```

### 53.7.3 Simple map / scalar UDF

```text id="7fp8sx"
RecordBatch in
  → evaluate scalar expressions/UDFs
  → RecordBatch out
```

Streaming-safe if:

```text id="n00ox7"
per-row / per-batch
no global state
bounded memory per batch
no blocking remote calls per row
null/scalar/array handling correct
```

### 53.7.4 Streaming scan

```text id="mp23m5"
source partition
  → read next file/page/API page/message batch
  → convert to RecordBatch
  → emit
```

Streaming-safe if:

```text id="kf1j86"
schema known up front
bounded batch/page size
source polling is async or bridged safely
cancellation stops source
state/memory bounded
```

---

## 53.8 Pipeline breakers

Pipeline breakers are operators that need **all input**, **large state**, or **end-of-stream** before they can produce complete output.

### 53.8.1 Full sort

```sql id="m6639x"
SELECT *
FROM events
ORDER BY event_ts;
```

Behavior:

```text id="5yc5gw"
must see all rows to emit globally sorted result
large memory/spill
on unbounded input, final global order may never complete
```

### 53.8.2 Global aggregate

```sql id="7v24y0"
SELECT count(*)
FROM unbounded_events;
```

Behavior:

```text id="1wkj01"
final count requires end-of-stream
unbounded stream never ends
no final result
```

### 53.8.3 Some joins

```text id="77qwu8"
hash join:
  build side must be fully read before probing in classic build/probe shape

nested loop:
  buffers build side

cross join:
  buffers one side and multiplies rows

stream-stream join:
  requires bounded state or pruning predicate
```

### 53.8.4 Some windows

```sql id="zqemz2"
SELECT
  row_number() OVER (ORDER BY event_ts) AS rn
FROM unbounded_events;
```

Behavior:

```text id="dywq4g"
global ordering requirement
potentially unbounded state
unsupported unless query/source/window semantics bound state
```

### 53.8.5 `collect`

```rust id="t21dvh"
let batches = df.collect().await?;
```

`collect` executes and buffers all output `RecordBatch`es into memory; `execute_stream` avoids buffering and is recommended when buffering is undesirable. ([Docs.rs][6])

Agent rule:

```text id="5jhvj5"
collect is a pipeline breaker at the client boundary.
It converts a stream into Vec<RecordBatch>.
Use only for bounded, small/moderate outputs.
```

---

## 53.9 Partial pipeline breakers

### 53.9.1 Local/global aggregate split

```text id="s3jtml"
Input partitions
  → local partial aggregate per partition
  → repartition by group key
  → final aggregate
```

Streaming nuance:

```text id="hcxclq"
local stage can emit partial state only after seeing partition input or chunks depending implementation
final stage may block per group/global result
bounded source: fine
unbounded source: needs windowing/state bounds
```

### 53.9.2 TopK

```sql id="3w33e1"
SELECT event_id, score
FROM events
ORDER BY score DESC
LIMIT 100;
```

Behavior:

```text id="u3fba6"
does not need full sorted output
keeps bounded heap/top-N state
can reduce memory relative to full sort
still may wait until enough/all input depending exact global semantics
```

### 53.9.3 Partitioned sort

```text id="3hxi05"
Sort each partition locally:
  can emit sorted partition after partition input consumed

Global sort:
  requires merge/global coordination
```

### 53.9.4 Sort-preserving merge

```text id="8z4hzh"
sorted partition streams
  → merge by sort key
  → globally ordered stream
```

State:

```text id="x9p94y"
needs one/few rows buffered per partition
stream-friendly for bounded sorted inputs
requires truthful ordering metadata
```

Agent rule:

```text id="25v5vj"
Partial pipeline breaker != free.
It bounds or localizes state, but still has blocking points.
Inspect physical plan, not only logical SQL shape.
```

---

## 53.10 Streaming-safe SQL patterns

### 53.10.1 Projection + filter + limit

```sql id="dg3rde"
SELECT
  event_id,
  event_ts,
  event_type
FROM events
WHERE region = 'us'
LIMIT 1000;
```

Topology:

```text id="t34s6o"
DataSourceExec
  → FilterExec
  → ProjectionExec
  → LimitExec
```

### 53.10.2 Bounded top-K

```sql id="nes7qf"
SELECT
  event_id,
  score
FROM events
WHERE event_date = DATE '2026-05-22'
ORDER BY score DESC, event_id ASC
LIMIT 100;
```

Streaming status:

```text id="7m1sev"
bounded source:
  top-k-friendly, acceptable

unbounded source:
  only safe if query semantics define bounded source/window or result can complete after LIMIT under physical plan
```

### 53.10.3 Time-bucketed aggregation over bounded source

```sql id="5xls0f"
SELECT
  date_bin(INTERVAL '5 minutes', event_ts, TIMESTAMP '1970-01-01') AS bucket,
  count(*) AS n
FROM events
WHERE event_date = DATE '2026-05-22'
GROUP BY bucket
ORDER BY bucket;
```

### 53.10.4 Stream-oriented time-bucket pattern

```sql id="p21z0f"
SELECT
  date_bin(INTERVAL '1 minute', event_ts, TIMESTAMP '1970-01-01') AS bucket,
  region,
  count(*) AS n
FROM stream_events
GROUP BY bucket, region;
```

Caution:

```text id="21e5ya"
This is syntactically stream-shaped, but actual support depends on DataFusion operator support for the source and aggregation/window semantics.
Unbounded final results may still need bounded windows/watermarks outside core SQL shape.
```

### 53.10.5 Stream-static join

```sql id="iwfgls"
SELECT
  s.event_id,
  d.category
FROM stream_events AS s
JOIN dim_static AS d
  ON s.dim_id = d.id;
```

Better than:

```sql id="fuvdih"
SELECT *
FROM stream_a AS a
JOIN stream_b AS b
  ON a.user_id = b.user_id;
```

Stream-stream join needs state bounds, such as time/range predicates.

---

## 53.11 Risky or invalid unbounded SQL patterns

### Full global sort

```sql id="lon1xs"
SELECT *
FROM stream_events
ORDER BY event_ts;
```

Risk:

```text id="34h7fq"
needs all input before final total order
unbounded input never ends
```

### Global count

```sql id="ss7wmt"
SELECT count(*)
FROM stream_events;
```

Risk:

```text id="6lkyqm"
final count requires stream termination
```

### Distinct

```sql id="pn5eq5"
SELECT DISTINCT user_id
FROM stream_events;
```

Risk:

```text id="j4970m"
unbounded distinct state unless windowed/bounded
```

### Cross join / unbounded nested loop

```sql id="kvopsh"
SELECT *
FROM stream_a
CROSS JOIN stream_b;
```

Risk:

```text id="6hdxxv"
unbounded cardinality and memory
```

### Unbounded window without bounded frame

```sql id="ceh7r0"
SELECT
  row_number() OVER (ORDER BY event_ts) AS rn
FROM stream_events;
```

Risk:

```text id="rlbjif"
global ordering/state problem
```

Agent lint rules:

```text id="872iv9"
Reject:
  unbounded + full sort
  unbounded + global aggregate
  unbounded + distinct
  unbounded + cross join
  unbounded + stream-stream join without bound
  unbounded + final-only physical operator
```

---

## 53.12 Unbounded planning restrictions

### 53.12.1 Source declaration

```sql id="x93jbo"
CREATE UNBOUNDED EXTERNAL TABLE stream_events (
  event_id VARCHAR,
  event_ts TIMESTAMP,
  region VARCHAR
)
STORED AS PARQUET
LOCATION '/path/to/source';
```

The DDL docs define this as an unbounded data source marker; if a query cannot be executed in streaming mode, DataFusion fails plan generation. ([Docs.rs][1])

### 53.12.2 Logical lint before physical planning

```rust id="qz38nx"
#[derive(Debug, Clone)]
pub struct StreamingPolicy {
    pub allow_unbounded_sources: bool,
    pub deny_global_sort: bool,
    pub deny_global_aggregate: bool,
    pub deny_distinct: bool,
    pub deny_cross_join: bool,
    pub require_stream_join_bounds: bool,
}

pub fn lint_streaming_query(
    plan: &datafusion::logical_expr::LogicalPlan,
    policy: &StreamingPolicy,
) -> datafusion::error::Result<()> {
    // Pseudocode:
    // - detect unbounded table/source metadata
    // - detect Sort without LIMIT/window bound
    // - detect Aggregate with global final output
    // - detect Distinct
    // - detect CrossJoin
    // - detect unbounded stream-stream join without time/range bound
    Ok(())
}
```

### 53.12.3 Physical property lint

```rust id="a8i7pe"
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};

pub fn lint_unbounded_physical_topology(
    plan: &dyn ExecutionPlan,
    diagnostics: &mut Vec<String>,
) {
    let boundedness = format!("{:?}", plan.boundedness());
    let pipeline = format!("{:?}", plan.pipeline_behavior());

    if boundedness.contains("Unbounded")
        && pipeline.contains("Final")
    {
        diagnostics.push(format!(
            "operator {} is unbounded but has final/blocking pipeline behavior",
            plan.name()
        ));
    }

    for child in plan.children() {
        lint_unbounded_physical_topology(child.as_ref(), diagnostics);
    }
}
```

---

## 53.13 Planner errors for unsupported unbounded queries

Expected failure classes:

```text id="80ig0n"
Logical planning failure:
  query shape not valid over unbounded source

Physical planning failure:
  no streaming physical implementation exists

Physical optimization failure:
  required ordering/distribution cannot be satisfied streaming-safely

Execution admission failure:
  service policy denies unbounded + blocking topology

Runtime failure:
  custom source/operator misreports boundedness and hangs or OOMs
```

Error reporting shape:

```rust id="yh6mqp"
#[derive(Debug, Clone)]
pub struct StreamingPlanError {
    pub phase: &'static str,
    pub code: &'static str,
    pub operator: Option<String>,
    pub message: String,
    pub remediation: String,
}

pub fn unbounded_global_sort_error() -> StreamingPlanError {
    StreamingPlanError {
        phase: "physical_plan",
        code: "UNBOUNDED_GLOBAL_SORT",
        operator: Some("SortExec".to_string()),
        message: "global sort over unbounded input cannot produce final ordered output".to_string(),
        remediation: "add bounded filter/window/limit semantics or run query over bounded source".to_string(),
    }
}
```

Agent rule:

```text id="6lovq9"
Treat unbounded-query planning failures as expected capability feedback, not engine bugs.
```

---

## 53.14 Service endpoint implications

### 53.14.1 Use `execute_stream`

```rust id="lux6wz"
use datafusion::prelude::*;
use futures::StreamExt;

pub async fn stream_query(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;

    let mut stream = df.execute_stream().await?;

    while let Some(batch) = stream.next().await {
        let batch = batch?;
        // serialize batch to response chunk
        println!("rows={}", batch.num_rows());
    }

    Ok(())
}
```

`execute_stream` begins execution and returns a `SendableRecordBatchStream`; DataFusion’s docs note that dropping this stream aborts execution and frees allocated resources. ([Docs.rs][6])

### 53.14.2 Avoid `collect`

```rust id="x15oaq"
let batches = df.collect().await?; // buffers all output
```

Use `collect` for:

```text id="6w4ijp"
small fixture tests
small admin queries
control-plane metadata
bounded results with explicit cap
```

Do not use `collect` for:

```text id="ei2x39"
arbitrary user SQL
large exports
unbounded streams
HTTP endpoints
long-running analytics
```

### 53.14.3 Cancellation

```text id="6n3rox"
HTTP client disconnect
  → drop stream
  → DataFusion aborts execution
  → resources freed

Timeout
  → drop stream / cancel task
  → upstream streams stop polling
  → custom sources must stop producers
```

Custom source cancellation rules:

```text id="a8zvzy"
If stream dropped:
  abort in-flight request
  close channel
  stop background task
  release memory reservation
  close file/network handles
```

### 53.14.4 Chunk serialization

```text id="en7m85"
Arrow IPC:
  write schema once
  write each RecordBatch as IPC message

JSON:
  serialize each RecordBatch to chunk
  preserve row cap / byte cap

CSV:
  write header once
  write batch rows incrementally

Parquet:
  streaming to writer/sink path, not HTTP row response usually
```

Chunking policy:

```text id="8c7tj7"
Do not convert entire stream to Vec.
Do not serialize entire result to String before sending.
Maintain response backpressure.
Track rows/bytes emitted.
Stop at service cap.
```

---

## 53.15 Runtime resource configuration

```rust id="1zgjl1"
use datafusion::execution::{
    runtime_env::RuntimeEnvBuilder,
    session_state::SessionStateBuilder,
};
use datafusion::prelude::*;

pub fn service_context() -> datafusion::error::Result<SessionContext> {
    let config = SessionConfig::new()
        .with_batch_size(8192)
        .with_target_partitions(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        );

    let runtime = RuntimeEnvBuilder::new()
        .with_memory_limit(8 * 1024 * 1024 * 1024, 0.80)
        .with_temp_file_path("/mnt/datafusion-spill")
        .build_arc()?;

    let state = SessionStateBuilder::new()
        .with_config(config)
        .with_runtime_env(runtime)
        .with_default_features()
        .build();

    Ok(SessionContext::from(state))
}
```

Streaming reduces output buffering but does not eliminate operator state. Full sorts, joins, group-by aggregates, windows, and distinct can still consume substantial memory or spill; the attached documentation explicitly warns that streaming output does not mean no memory use and recommends memory limits/spill paths in services. 

---

## 53.16 Custom `ExecutionPlan` streaming skeleton

```rust id="i1mq78"
use std::sync::Arc;

use datafusion::arrow::{datatypes::SchemaRef, record_batch::RecordBatch};
use datafusion::common::{internal_err, Result};
use datafusion::execution::TaskContext;
use datafusion::physical_expr::EquivalenceProperties;
use datafusion::physical_plan::{
    execution_plan::{Boundedness, EmissionType, PlanProperties},
    stream::RecordBatchStreamAdapter,
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning,
    SendableRecordBatchStream,
};
use futures::stream;

#[derive(Debug)]
pub struct MyStreamingExec {
    schema: SchemaRef,
    properties: Arc<PlanProperties>,
    partitions: usize,
}

impl MyStreamingExec {
    pub fn try_new(
        schema: SchemaRef,
        partitions: usize,
        boundedness: Boundedness,
    ) -> Result<Self> {
        let properties = Arc::new(PlanProperties::new(
            EquivalenceProperties::new(schema.clone()),
            Partitioning::UnknownPartitioning(partitions),
            EmissionType::Incremental,
            boundedness,
        ));

        Ok(Self {
            schema,
            properties,
            partitions,
        })
    }
}

impl DisplayAs for MyStreamingExec {
    fn fmt_as(
        &self,
        _t: DisplayFormatType,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        write!(f, "MyStreamingExec: partitions={}", self.partitions)
    }
}

impl ExecutionPlan for MyStreamingExec {
    fn name(&self) -> &str {
        "MyStreamingExec"
    }

    fn properties(&self) -> &Arc<PlanProperties> {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        if !children.is_empty() {
            return internal_err!("MyStreamingExec expects no children");
        }
        Ok(self)
    }

    fn execute(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        if partition >= self.partitions {
            return internal_err!(
                "partition {partition} out of range; partitions={}",
                self.partitions
            );
        }

        let schema = self.schema.clone();

        // Placeholder finite stream. Real source should poll file/API/queue incrementally.
        let s = stream::try_unfold(0_usize, move |state| {
            let schema = schema.clone();
            async move {
                if state >= 10 {
                    Ok(None)
                } else {
                    let batch = make_batch(schema, partition, state)?;
                    Ok(Some((batch, state + 1)))
                }
            }
        });

        Ok(Box::pin(RecordBatchStreamAdapter::new(
            self.schema.clone(),
            s,
        )))
    }
}

fn make_batch(
    _schema: SchemaRef,
    _partition: usize,
    _state: usize,
) -> Result<RecordBatch> {
    todo!("produce one RecordBatch")
}
```

Custom operator rules:

```text id="47esmy"
execute() should return quickly.
Data production happens during stream polling.
Properties must accurately report boundedness/emission.
Batches must match schema.
Partition id must be validated.
Stream drop must cancel producer work.
```

---

## 53.17 Custom source bridge for blocking APIs

When a source is blocking-only:

```rust id="mk4vws"
pub struct BlockingBridgeConfig {
    pub channel_capacity: usize,
    pub batch_rows: usize,
    pub max_inflight_requests: usize,
}
```

Pattern:

```text id="oxp6wb"
blocking producer thread/task
  → bounded channel<RecordBatch>
  → RecordBatchStream polls receiver
  → receiver drop signals producer stop
```

Rules:

```text id="k47y6v"
Use bounded channel.
Never use unbounded channel for infinite/large source.
Producer observes cancellation.
Producer batches rows.
Producer maps errors to DataFusionError.
Blocking work does not run on core Tokio worker threads.
```

---

## 53.18 Streaming diagnostics

### 53.18.1 Physical plan inspection

```rust id="q76qa6"
use datafusion::physical_plan::{displayable, ExecutionPlanProperties};

pub async fn inspect_streaming_topology(df: DataFrame) -> datafusion::error::Result<()> {
    let physical = df.create_physical_plan().await?;

    println!("{}", displayable(physical.as_ref()).indent(true));
    println!("partitioning={:?}", physical.output_partitioning());
    println!("ordering={:?}", physical.output_ordering());
    println!("boundedness={:?}", physical.boundedness());
    println!("pipeline={:?}", physical.pipeline_behavior());

    Ok(())
}
```

Inspect:

```text id="q0iruy"
SortExec present?
Aggregate partial/final shape?
HashJoinExec / SortMergeJoinExec / SymmetricHashJoinExec?
RepartitionExec inserted?
CoalesceBatchesExec inserted?
DataSourceExec partition count?
FilterExec above scan or pushed down?
Boundedness/emission of custom nodes?
```

### 53.18.2 EXPLAIN ANALYZE

```sql id="8n2w0w"
EXPLAIN ANALYZE
SELECT event_id, event_ts
FROM events
WHERE region = 'us'
LIMIT 1000;
```

Use for:

```text id="4rwuhx"
operator output row counts
operator elapsed compute
sort/aggregate/join bottlenecks
scan pruning
repartition effects
skew
unexpected pipeline breaker
```

---

## 53.19 Testing matrix

```text id="nwldx8"
Stream basics:
  [ ] stream.schema() known before first poll
  [ ] all batches match schema
  [ ] empty finite stream returns None
  [ ] finite stream terminates
  [ ] unbounded stream can be dropped/cancelled
  [ ] error item propagates
  [ ] stream can produce multiple batches
  [ ] final short batch handled

Backpressure:
  [ ] source does not fetch all data before first poll
  [ ] bounded prefetch honored
  [ ] network writer backpressure slows polling
  [ ] producer stops on receiver drop
  [ ] blocking source bridge uses bounded channel

Topology:
  [ ] projection/filter plan emits incrementally
  [ ] full sort identified as pipeline breaker
  [ ] global aggregate identified as pipeline breaker
  [ ] top-k uses bounded state compared to full sort where supported
  [ ] partial/final aggregate shape inspected
  [ ] unbounded + blocking rejected

Partitions:
  [ ] execute(partition) valid for all reported partitions
  [ ] partition out-of-range errors
  [ ] partitioned streams produce correct data
  [ ] skewed partitions diagnosed

Service:
  [ ] client disconnect drops stream
  [ ] timeout cancels stream
  [ ] row/byte cap stops stream
  [ ] serialization chunk boundaries valid
  [ ] resource cleanup verified
```

---

## 53.20 Deployment patterns

### 53.20.1 HTTP/gRPC/Arrow Flight service

```text id="l3ukcn"
Plan:
  ctx.sql_with_options / DataFrame plan
  logical lint
  physical topology lint
  execute_stream
  serialize RecordBatch chunks
  drop stream on disconnect
  enforce row/byte/time limits
```

### 53.20.2 Batch ETL

```text id="7q6p3v"
Plan:
  execute_stream to sink
  avoid collect except small metadata/control outputs
  use partitioned streams for partitioned writes
  configure memory/spill paths
  tolerate pipeline breakers under batch resource policy
```

### 53.20.3 Remote source service

```text id="8hiphe"
Plan:
  TableProvider::scan returns custom ExecutionPlan
  execute(partition) returns demand-driven RecordBatchStream
  async page fetch
  bounded page size
  bounded prefetch
  cancellation-safe producer
  projection/filter/limit pushdown
```

### 53.20.4 Unbounded stream processor

```text id="aickdb"
Plan:
  source Boundedness::Unbounded
  operators Incremental or state-bounded
  reject global sort/final aggregate/distinct
  enforce time/window/range constraints for joins/windows
  checkpointing/watermarks outside core query if needed
```

---

## 53.21 Best-practice advisory

```text id="lkgja8"
Always:
  prefer execute_stream for service outputs
  configure memory limits and spill directories
  inspect physical topology for blockers
  classify boundedness and emission accurately
  write custom streams as demand-driven
  preserve cancellation behavior
  test stream drop/timeout paths

For generated SQL:
  add LIMIT for exploratory queries
  avoid full ORDER BY over unbounded sources
  use top-k only when bounded semantics exist
  replace global aggregates on unbounded streams with windowed/bucketed forms where supported
  avoid DISTINCT on unbounded streams
  require bounded state for stream-stream joins

For custom ExecutionPlan:
  do not do heavy work in execute()
  do not read all data before returning stream
  use RecordBatchStreamAdapter or custom stream
  validate partition id
  expose accurate PlanProperties
```

---

## 53.22 Anti-pattern inventory

* Using `collect()` inside HTTP handler.
* Calling remote API for all pages before returning first batch.
* Unbounded channel between blocking source and stream.
* Ignoring stream drop/cancellation.
* Marking unbounded source as bounded.
* Marking blocking operator as incremental.
* Treating `execute_stream` as proof the query is memory-light.
* Running global sort over unbounded stream.
* Running global count over unbounded stream and expecting output.
* Using unbounded `DISTINCT`.
* Stream-stream join without state bound.
* Assuming `LIMIT` after a cross join prevents intermediate explosion.
* Treating local per-partition order as global order.
* Snapshotting only logical plan and missing physical pipeline breakers.
* Custom stream emitting batches with schema drift.

---

## 53.23 Agent checklist

```text id="jvu00b"
[ ] Treat ExecutionPlan as stream topology:
    nodes
    edges
    partitions
    RecordBatch streams

[ ] Use correct execution API:
    execute_stream for service output
    execute_stream_partitioned for partition-aware output
    collect only for bounded small results

[ ] Check boundedness:
    finite files / finite query = bounded
    Kafka/WebSocket/tailing log = unbounded
    custom source PlanProperties accurate

[ ] Check emission:
    projection/filter/map/scan = incremental
    full sort/global aggregate/distinct = final/blocking
    joins/windows depend on state/order/frame

[ ] Identify pipeline breakers:
    SortExec
    final AggregateExec
    HashJoin build side
    NestedLoop/CrossJoin
    window over unbounded partition
    collect()

[ ] Use streaming-safe SQL:
    projection + filter
    LIMIT for preview
    bounded top-k
    time-bucketed/windowed aggregates where supported
    stream-static joins

[ ] Reject risky unbounded shapes:
    global ORDER BY
    global COUNT/SUM without window
    DISTINCT
    cross join
    unbounded stream-stream join without range/window bound

[ ] Service integration:
    stream schema first
    serialize each RecordBatch chunk
    propagate backpressure
    drop stream on disconnect
    timeout and cancel
    track rows/bytes emitted
    clean up resources

[ ] Custom stream:
    schema known
    bounded batch/page size
    no blocking Tokio workers
    bounded channels only
    cancellation-aware producer
    all batches match schema

[ ] Test:
    finite termination
    unbounded cancellation
    partition correctness
    backpressure
    pipeline-breaker detection
    EXPLAIN/metrics verification
```

[1]: https://docs.rs/datafusion/latest/datafusion/?utm_source=chatgpt.com "datafusion - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"
[3]: https://docs.rs/datafusion/latest/src/datafusion/lib.rs.html?utm_source=chatgpt.com "datafusion/ lib.rs"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/type.SendableRecordBatchStream.html?utm_source=chatgpt.com "SendableRecordBatchStream in datafusion::execution - Rust"
[5]: https://datafusion.apache.org/blog/2025/06/30/cancellation/?utm_source=chatgpt.com "Using Rust async for Query Execution and Cancelling Long ..."
[6]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"


# DataFusion Advanced — 54) Runtime execution planning: partitions, task scheduling, memory reservations, and spill

## 54.0 Purpose

Bridge physical planning and runtime execution:

```text id="llydzt"
optimized ExecutionPlan
  → PlanProperties / required distributions / required orderings
  → target_partitions / batch_size / RuntimeEnv
  → execute(partition, TaskContext)
  → SendableRecordBatchStream
  → memory pool reservations
  → spill files through DiskManager
  → object-store reads through ObjectStoreRegistry
  → cancellation / timeout / service concurrency budget
```

`ExecutionPlan::execute` returns an async `SendableRecordBatchStream` that incrementally computes **one output partition** of a physical plan. DataFusion attempts to exploit available parallelism by repartitioning inputs toward `target_partitions`, and some sources can implement repartitioning directly rather than requiring an extra `RepartitionExec`. ([Docs.rs][1])

---

## 54.1 Runtime stack

```text id="a4o11z"
SessionContext
  ├─ user-facing query facade
  ├─ table/function/catalog registries
  └─ creates SessionState

SessionState
  ├─ ConfigOptions / SessionConfig
  ├─ optimizer / planner / function registries
  ├─ RuntimeEnv
  └─ creates TaskContext

RuntimeEnv
  ├─ MemoryPool
  ├─ DiskManager
  ├─ CacheManager
  └─ ObjectStoreRegistry

TaskContext
  ├─ execution-only query/task context
  ├─ session_id / task_id
  ├─ config
  ├─ function registries
  └─ RuntimeEnv reference

ExecutionPlan
  └─ execute(partition, TaskContext)
      → SendableRecordBatchStream
```

`RuntimeEnv` manages runtime resources such as memory, disk, cache, and storage; DataFusion’s docs list `MemoryPool`, `DiskManager`, `CacheManager`, and `ObjectStoreRegistry` as the core resource managers. ([Docs.rs][2])

---

## 54.2 `target_partitions`

### 54.2.1 Meaning

```text id="e82mrr"
target_partitions:
  desired parallelism target
  influences repartitioning
  influences number of independent streams
  affects join/aggregate/sort/source parallelism
```

Configuration:

```rust id="urxj7x"
use datafusion::prelude::*;

let config = SessionConfig::new()
    .with_target_partitions(8)
    .with_batch_size(8192);

let ctx = SessionContext::new_with_config(config);
```

SQL:

```sql id="u3mzog"
SET datafusion.execution.target_partitions = '8';
SET datafusion.execution.batch_size = '8192';
```

Environment:

```bash id="lnuzof"
export DATAFUSION_EXECUTION_TARGET_PARTITIONS=8
export DATAFUSION_EXECUTION_BATCH_SIZE=8192
```

DataFusion config docs recommend reducing `target_partitions` for very small data to avoid repartitioning overhead; for data under roughly 1MB, the docs recommend `target_partitions = 1`. ([Apache DataFusion][3])

### 54.2.2 Tuning policy

| Workload                     | Suggested posture                                                      |
| ---------------------------- | ---------------------------------------------------------------------- |
| tiny lookup / metadata query | `target_partitions = 1`                                                |
| small interactive query      | low partition count, avoid exchange overhead                           |
| large Parquet scan           | near CPU count or source natural partition count                       |
| object-store scan            | balance CPU parallelism against remote request pressure                |
| hash aggregate/join          | enough partitions to use CPU, not so many that overhead/skew dominates |
| service multi-tenancy        | per-query target lowered to preserve fleet concurrency                 |
| batch ETL                    | higher target if memory/network budgets permit                         |

Agent rule:

```text id="wyhid3"
target_partitions is not “more is always faster.”
It trades parallelism against repartition overhead, memory pressure, remote request concurrency, and scheduling overhead.
```

---

## 54.3 `batch_size`

### 54.3.1 Meaning

```text id="9lk07i"
batch_size:
  approximate target rows per RecordBatch
  affects vectorized-kernel amortization
  affects latency to first/next batch
  affects peak transient array memory
  affects network chunk size for streaming responses
```

Configuration:

```rust id="dqoyuo"
let config = SessionConfig::new()
    .with_batch_size(4096);
```

`SessionConfig::with_batch_size` is shorthand for setting `datafusion.execution.batch_size`, and `SessionConfig` maps dotted configuration keys such as `datafusion.execution.batch_size` to the corresponding `ConfigOptions` fields. ([Docs.rs][4])

### 54.3.2 Batch-size tradeoff

| Smaller batches                  | Larger batches                 |
| -------------------------------- | ------------------------------ |
| lower response latency           | better CPU amortization        |
| better cancellation granularity  | fewer batches / lower overhead |
| smaller transient arrays         | larger transient arrays        |
| more stream polls                | fewer stream polls             |
| more serialization chunks        | larger chunks                  |
| better for interactive streaming | better for throughput scans    |

Agent rules:

```text id="9pa590"
Never assume exact batch_size.
Operators may emit smaller batches.
Consumers must handle arbitrary batch lengths.
Custom sources should respect batch_size as a target, not a hard invariant.
```

---

## 54.4 Partition-to-stream mapping

### 54.4.1 Physical contract

```text id="jh111g"
ExecutionPlan.output_partitioning().partition_count() = N

Runtime:
  for partition in 0..N:
    stream = plan.execute(partition, task_ctx)
```

Example:

```rust id="9mu4z7"
use datafusion::physical_plan::{ExecutionPlan, ExecutionPlanProperties};
use futures::StreamExt;
use std::sync::Arc;

pub async fn run_all_partitions(
    plan: Arc<dyn ExecutionPlan>,
    task_ctx: Arc<datafusion::execution::TaskContext>,
) -> datafusion::error::Result<()> {
    let partitions = plan.output_partitioning().partition_count();

    for partition in 0..partitions {
        let mut stream = plan.execute(partition, task_ctx.clone())?;

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            println!("partition={partition}, rows={}", batch.num_rows());
        }
    }

    Ok(())
}
```

### 54.4.2 Custom operator partition validation

```rust id="sqiiqq"
fn validate_partition(partition: usize, partitions: usize) -> datafusion::common::Result<()> {
    if partition >= partitions {
        return Err(datafusion::common::DataFusionError::Execution(format!(
            "partition {partition} out of range; partitions={partitions}"
        )));
    }

    Ok(())
}
```

Agent rule:

```text id="xumksl"
If an ExecutionPlan reports N partitions, execute(0..N-1) must be valid and execute(N) should error.
```

---

## 54.5 `TaskContext`

`TaskContext` is execution-only state passed to physical operators. It carries config/function/runtime access needed during physical execution.

Conceptual contents:

```text id="wis6xz"
TaskContext:
  session_id
  task_id
  SessionConfig / ConfigOptions
  scalar function registry
  aggregate function registry
  window function registry
  RuntimeEnv
    ├─ MemoryPool
    ├─ DiskManager
    ├─ ObjectStoreRegistry
    └─ CacheManager
```

Manual execution:

```rust id="skys43"
use datafusion::prelude::*;
use futures::StreamExt;

pub async fn manual_physical_execute(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = ctx.sql(sql).await?;
    let physical = df.create_physical_plan().await?;
    let task_ctx = ctx.task_ctx();

    let mut stream = physical.execute(0, task_ctx)?;

    while let Some(batch) = stream.next().await {
        let batch = batch?;
        println!("rows={}", batch.num_rows());
    }

    Ok(())
}
```

Agent rules:

```text id="7zfjth"
Use ctx.task_ctx() / state.task_ctx() for manual execution.
Do not construct TaskContext directly unless testing custom ExecutionPlan internals.
Custom physical operators should obtain memory/object-store/temp resources through TaskContext/RuntimeEnv, not global state.
```

---

## 54.6 `RuntimeEnv`

### 54.6.1 Construction

```rust id="wlm3k7"
use datafusion::execution::{
    runtime_env::RuntimeEnvBuilder,
    session_state::SessionStateBuilder,
};
use datafusion::prelude::*;

pub fn configured_context() -> datafusion::error::Result<SessionContext> {
    let config = SessionConfig::new()
        .with_batch_size(8192)
        .with_target_partitions(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
        );

    let runtime = RuntimeEnvBuilder::new()
        .with_memory_limit(8 * 1024 * 1024 * 1024, 0.80)
        .with_temp_file_path("/mnt/datafusion-spill")
        .with_max_temp_directory_size(64 * 1024 * 1024 * 1024)
        .build_arc()?;

    let state = SessionStateBuilder::new()
        .with_config(config)
        .with_runtime_env(runtime)
        .with_default_features()
        .build();

    Ok(SessionContext::from(state))
}
```

`RuntimeEnvBuilder::with_memory_limit(max_memory, memory_fraction)` sets memory to `max_memory * memory_fraction`, using a `GreedyMemoryPool` wrapped in `TrackConsumersPool` by default; docs warn that DataFusion does not yet respect this limit in all cases. ([Docs.rs][5]) Runtime configuration keys also include `datafusion.runtime.memory_limit`, `datafusion.runtime.temp_directory`, `datafusion.runtime.max_temp_directory_size`, and metadata-cache limits. ([Apache DataFusion][3])

### 54.6.2 Deployment profiles

```text id="h9whcn"
local CLI:
  RuntimeEnv default acceptable
  memory/temp explicit for large local work

batch ETL:
  memory limit
  spill directory on fast local disk
  large temp dir quota
  high target_partitions if IO/CPU allow

public service:
  memory limit
  max temp directory size
  tenant/object-store isolation
  bounded target_partitions
  request timeout/cancellation
  result row/byte cap

multi-tenant service:
  separate RuntimeEnv per tenant or resource class if isolation required
  shared RuntimeEnv only when memory/temp/object-store policy is shared
```

---

## 54.7 Object-store registry

Object-store registry responsibilities:

```text id="k9o6yc"
URI scheme/authority:
  s3://bucket/path
  gs://bucket/path
  file:///path
  https://host/path

RuntimeEnv/ObjectStoreRegistry:
  map URI prefix to ObjectStore instance
  credential/config resolution
  object get/list/range APIs
```

DataFusion execution docs describe `ObjectStoreRegistry` as mapping URL schemes to object stores and enabling extension to systems such as S3 or HDFS. ([Docs.rs][6])

Registration pattern:

```rust id="bpfm7e"
use std::sync::Arc;
use datafusion::object_store::local::LocalFileSystem;
use datafusion::prelude::*;
use url::Url;

let ctx = SessionContext::new();

ctx.register_object_store(
    &Url::parse("file:///")?,
    Arc::new(LocalFileSystem::new()),
);
```

Agent rules:

```text id="vpqqop"
Register object stores centrally.
Do not place long-lived credentials in SQL text.
Use separate RuntimeEnv/ObjectStoreRegistry when tenants have different credentials.
Remote object-store concurrency is part of service budget.
```

---

## 54.8 Memory pool

### 54.8.1 Model

```text id="k4b62b"
MemoryPool:
  global or runtime-level accounting

MemoryConsumer:
  named operator/data-structure consumer

MemoryReservation:
  one reservation owned by a consumer
  freed on drop
```

DataFusion memory docs describe `MemoryReservation` as tracking an individual byte reservation in a `MemoryPool`, and each `MemoryConsumer`/operator can own multiple reservations for different internal data structures. ([Docs.rs][7]) The broader memory-pool docs also note that DataFusion limits memory only for operators that require large row-proportional memory, and does not track every internal allocation or the normal `RecordBatch` flow between operators. ([Docs.rs][7])

### 54.8.2 Memory pool implementations / policies

```text id="gzkvks"
UnboundedMemoryPool:
  no effective limit
  local/dev only or externally bounded process

GreedyMemoryPool:
  consumers reserve until global limit reached

FairSpillPool:
  intended to divide memory more evenly among partitions/consumers
  useful under memory pressure/spilling workloads

TrackConsumersPool:
  wrapper for tracking top consumers
  diagnostic value
```

### 54.8.3 Custom operator reservation pattern

```rust id="98t1ec"
use datafusion::execution::memory_pool::MemoryConsumer;
use datafusion::error::Result;

pub struct MyStatefulOperatorState {
    reservation: datafusion::execution::memory_pool::MemoryReservation,
    buffered_bytes: usize,
}

impl MyStatefulOperatorState {
    pub fn try_new(
        consumer_name: &str,
        task_ctx: &datafusion::execution::TaskContext,
    ) -> Self {
        let consumer = MemoryConsumer::new(consumer_name);
        let reservation = consumer.register(task_ctx.memory_pool());

        Self {
            reservation,
            buffered_bytes: 0,
        }
    }

    pub fn reserve_more(&mut self, additional: usize) -> Result<()> {
        self.reservation.try_grow(additional)?;
        self.buffered_bytes += additional;
        Ok(())
    }

    pub fn shrink(&mut self, amount: usize) {
        let amount = amount.min(self.buffered_bytes);
        self.reservation.shrink(amount);
        self.buffered_bytes -= amount;
    }
}
```

Exact reservation method names can vary by version; use pinned docs.rs for the final API, but preserve the invariant: **reserve before buffering; shrink/free when releasing; reservation drops free memory automatically**.

### 54.8.4 Operator memory policy

```text id="6mr73w"
Reserve memory for:
  hash tables
  sort buffers
  aggregate state
  join build batches
  custom operator state
  large intermediate buffers

Do not reserve for:
  every emitted RecordBatch
  tiny fixed-size fields
  untracked allocations you cannot free predictably
```

---

## 54.9 Disk manager and spill

`DiskManager` manages temporary files generated during query execution, such as spill files when processing data larger than memory. ([Docs.rs][8]) Runtime configuration exposes a temporary directory and maximum temporary directory size. ([Apache DataFusion][3])

### 54.9.1 Spill directory configuration

```rust id="g4rjlk"
let runtime = RuntimeEnvBuilder::new()
    .with_memory_limit(16 * 1024 * 1024 * 1024, 0.80)
    .with_temp_file_path("/mnt/fast-local-ssd/datafusion-spill")
    .with_max_temp_directory_size(256 * 1024 * 1024 * 1024)
    .build_arc()?;
```

SQL/config:

```sql id="lb70mb"
SET datafusion.runtime.memory_limit = '16G';
SET datafusion.runtime.temp_directory = '/mnt/fast-local-ssd/datafusion-spill';
SET datafusion.runtime.max_temp_directory_size = '256G';
```

### 54.9.2 Spill planning matrix

| Operator class | Why spills                                          | Runtime concern                                |
| -------------- | --------------------------------------------------- | ---------------------------------------------- |
| sort           | full input/order state exceeds memory               | spill run files, merge cost, temp bandwidth    |
| hash join      | build side hash table / buffered rows exceed memory | spill partitions, probe complexity, disk IO    |
| group by       | aggregate hash map/group state exceeds memory       | spill groups/partitions, merge partials        |
| distinct       | set state exceeds memory                            | similar to aggregate                           |
| window         | partition/frame state exceeds memory                | frame buffering/spill support depends operator |

DataFusion configuration docs state that memory-consuming queries under tight memory limits can spill intermediate results to disk, and that `FairSpillPool` divides memory among partitions. ([Apache DataFusion][9])

### 54.9.3 Spill policy

```text id="q2zk17"
Production:
  temp dir on fast local disk
  quota via max_temp_directory_size
  monitor free space
  isolate tenant/resource-class spill paths if needed
  clean up on query cancellation/failure
  treat spill as performance fallback, not normal fast path

Avoid:
  network filesystem spill if low latency needed
  shared OS /tmp without quota
  unlimited temp dir in multi-tenant service
```

---

## 54.10 Sort spill planning

```text id="7wilfx"
SortExec:
  read input batches
  accumulate/sort in memory
  spill sorted runs when memory constrained
  merge runs
  emit sorted RecordBatch stream
```

Runtime risk factors:

```text id="lxqptz"
large input rows
wide rows
ORDER BY many columns
string sort keys
no LIMIT / no top-k optimization
low memory limit
slow spill disk
```

Mitigation:

```text id="fzh9sn"
Push filters before sort.
Project narrow columns before sort.
Use ORDER BY + LIMIT for top-k when semantically acceptable.
Exploit source ordering when true.
Increase memory or use faster spill disk.
Avoid sort without limit in public endpoints.
```

---

## 54.11 Hash join spill/memory planning

```text id="6x2e9k"
HashJoinExec:
  build side batches + hash table
  probe side streaming
  memory pressure driven by build side width/cardinality/duplicates
```

Mitigation:

```text id="e9lxvz"
Filter build side early.
Project build side narrow.
Put smaller side as build side where semantics allow.
Collect/update table stats.
Use sort-merge join when inputs large/sorted.
Configure memory/spill.
Avoid string/function join keys when normalized keys possible.
```

Join docs emphasize that `HashJoinExec` reads the entire left input into a hash table, so the smaller input should be placed on the left/build side. ([docs.rs](https://docs.rs/datafusion/latest/datafusion/physical_plan/joins/struct.HashJoinExec.html?utm_source=chatgpt.com))

---

## 54.12 Group-by spill/memory planning

```text id="elr2lm"
GroupByHashExec / aggregate state:
  key hash map
  accumulator state per group
  partial/final aggregate state
  possible spill when group cardinality/state exceeds memory
```

Risk factors:

```text id="5zm7ea"
high-cardinality group keys
wide grouping keys
many aggregate functions
distinct aggregates
large accumulator states
skewed keys
too many partitions or too few partitions depending workload
```

Mitigation:

```text id="vynz3k"
Filter before aggregate.
Project narrow columns.
Use approximate aggregates when product-approved.
Partition by group keys.
Tune target_partitions.
Ensure memory/spill paths configured.
```

---

## 54.13 Cancellation model

DataFusion relies on Rust async polling and stream cancellation: query execution is driven by polling the returned stream, and dropping/canceling the stream/task stops further work if operators and sources are cancellation-safe. The DataFusion cancellation article explains query execution as compiling to `ExecutionPlan`, calling `execute`, receiving `SendableRecordBatchStream`, and repeatedly polling `Stream::poll_next`; it discusses cancellation problems and improvements around long-running tasks. ([Apache DataFusion][10])

### 54.13.1 Service timeout wrapper

```rust id="jnicfn"
use std::time::Duration;
use futures::StreamExt;
use tokio::time::timeout;

pub async fn consume_with_timeout(
    mut stream: datafusion::physical_plan::SendableRecordBatchStream,
    max_duration: Duration,
) -> datafusion::error::Result<()> {
    let fut = async move {
        while let Some(batch) = stream.next().await {
            let batch = batch?;
            println!("rows={}", batch.num_rows());
        }
        Ok::<_, datafusion::error::DataFusionError>(())
    };

    match timeout(max_duration, fut).await {
        Ok(result) => result,
        Err(_) => Err(datafusion::error::DataFusionError::Execution(
            "query timed out".to_string(),
        )),
    }
}
```

### 54.13.2 Cancellation-safe custom source

```text id="n2nlkb"
When stream dropped:
  stop remote fetch
  stop producer task
  close bounded channel
  release memory reservation
  delete temp files if owned
  decrement tenant concurrency counters
```

Anti-pattern:

```text id="nm95sk"
spawn background remote fetch loop
ignore stream drop
fill unbounded channel
continue query after client disconnected
```

---

## 54.14 Blocking vs async boundaries

### 54.14.1 CPU execution on Tokio

DataFusion uses Tokio as a work-stealing thread pool for CPU-intensive query work as well as async execution; docs/source comments describe DataFusion running each plan with multiple CPU cores via a Tokio runtime and using the runtime as a thread pool for query execution. ([GitHub][11]) Distinct from this generic Tokio task-level work stealing, DataFusion 54 adds file-level work stealing between sibling file-scan partitions — see 54.25.

### 54.14.2 Boundary rules

```text id="j0sjsw"
Async-safe:
  object-store async reads
  network requests through async clients
  stream polling
  bounded async channels

Blocking:
  synchronous filesystem API
  blocking database client
  CPU loop that never yields
  decompression/parsing function that monopolizes worker
```

### 54.14.3 Blocking bridge pattern

```rust id="g4q2xo"
use tokio::sync::mpsc;

pub struct BlockingBridge {
    pub channel_capacity: usize,
}

pub fn spawn_blocking_reader(
    capacity: usize,
) -> mpsc::Receiver<datafusion::error::Result<datafusion::arrow::record_batch::RecordBatch>> {
    let (tx, rx) = mpsc::channel(capacity);

    std::thread::spawn(move || {
        // Blocking API loop.
        // Send through bounded channel.
        // Stop when tx.blocking_send(...) fails.
    });

    rx
}
```

Rules:

```text id="62fthl"
Use bounded channels.
Stop producer when receiver dropped.
Do not block Tokio core worker threads with sync IO.
Avoid unbounded prefetch.
Use dedicated blocking pool/thread if source is sync-only.
```

---

## 54.15 CPU runtime vs network I/O runtime

Runtime split problem:

```text id="69oefx"
One Tokio runtime handles:
  CPU-heavy DataFusion operators
  network object-store reads
  API server request handling
  remote-source RPCs

Risk:
  CPU-heavy query tasks can delay network/API responsiveness.
```

The DataFusion docs/source comments discuss thread scheduling, CPU/IO thread pools, and Tokio runtimes, noting that DataFusion runs physical plans using Tokio as a thread pool and that runtime choices matter for CPU and IO behavior. ([GitHub][11])

Deployment patterns:

```text id="umq6kg"
Simple CLI:
  one runtime

Batch worker:
  one DataFusion runtime acceptable
  maybe separate upload/download workers

HTTP service:
  request/server runtime separate from query runtime
  bounded query task semaphore
  object-store/client request limits
  cancellation propagation

Remote federation:
  DataFusion CPU runtime
  remote API client concurrency limits
  bounded page prefetch
```

Agent rule:

```text id="k1yhmo"
Do not let arbitrary query parallelism share unlimited capacity with latency-sensitive server runtime.
```

---

## 54.16 Service concurrency budgeting

### 54.16.1 Budget dimensions

```text id="omd535"
per-query:
  memory limit
  target_partitions
  timeout
  result row cap
  result byte cap
  remote request concurrency
  temp spill quota

per-tenant:
  concurrent queries
  memory share
  temp dir quota
  remote request quota
  object-store credentials
  CPU share / priority

global service:
  total running queries
  total memory
  total spill disk
  total remote requests
  total CPU workers
  queue length
```

### 54.16.2 Budget struct

```rust id="oar3eh"
#[derive(Debug, Clone)]
pub struct QueryBudget {
    pub max_memory_bytes: usize,
    pub max_temp_bytes: usize,
    pub timeout_ms: u64,
    pub max_rows: usize,
    pub max_output_bytes: usize,
    pub target_partitions: usize,
    pub max_remote_inflight_requests: usize,
}

#[derive(Debug, Clone)]
pub struct TenantBudget {
    pub tenant_id: String,
    pub max_concurrent_queries: usize,
    pub max_memory_bytes: usize,
    pub max_temp_bytes: usize,
    pub max_remote_inflight_requests: usize,
}
```

### 54.16.3 Semaphore pattern

```rust id="3g3y8r"
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub struct QueryAdmission {
    global_queries: Arc<Semaphore>,
}

impl QueryAdmission {
    pub fn new(max_concurrent_queries: usize) -> Self {
        Self {
            global_queries: Arc::new(Semaphore::new(max_concurrent_queries)),
        }
    }

    pub async fn acquire(&self) -> Result<OwnedSemaphorePermit, datafusion::error::DataFusionError> {
        self.global_queries
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| datafusion::error::DataFusionError::Execution(
                "query admission semaphore closed".to_string(),
            ))
    }
}
```

Usage:

```rust id="3jlzfz"
pub async fn run_admitted_query(
    admission: QueryAdmission,
    ctx: SessionContext,
    sql: String,
    budget: QueryBudget,
) -> datafusion::error::Result<()> {
    let _permit = admission.acquire().await?;

    let df = ctx.sql(&sql).await?
        .limit(0, Some(budget.max_rows))?;

    let stream = df.execute_stream().await?;

    consume_with_timeout(
        stream,
        std::time::Duration::from_millis(budget.timeout_ms),
    ).await
}
```

---

## 54.17 Per-query memory and runtime isolation

### 54.17.1 Shared runtime

```text id="ksfk1s"
One RuntimeEnv:
  all queries share same MemoryPool
  all queries share temp dir quota
  all queries share object-store registry/cache

Pros:
  simple
  global memory cap
  shared metadata cache

Cons:
  weak tenant isolation
  noisy neighbor possible
```

### 54.17.2 Per-resource-class runtime

```rust id="e5wqaa"
pub fn runtime_for_budget(budget: &QueryBudget) -> datafusion::error::Result<Arc<datafusion::execution::runtime_env::RuntimeEnv>> {
    RuntimeEnvBuilder::new()
        .with_memory_limit(budget.max_memory_bytes, 1.0)
        .with_temp_file_path(format!("/mnt/spill/tenant-or-class/{}", budget.target_partitions))
        .with_max_temp_directory_size(budget.max_temp_bytes)
        .build_arc()
}
```

### 54.17.3 Policy

```text id="i2b2ps"
Shared RuntimeEnv:
  internal batch / single tenant / trusted workload

Separate RuntimeEnv:
  tenants need credential/resource isolation
  service tiers differ
  temp spill quotas differ
  memory limits differ materially
```

---

## 54.18 Max concurrent partitions

Potential pressure:

```text id="wn4m70"
concurrent queries * target_partitions
  = number of active partition streams/operators/tasks

Example:
  20 queries * target_partitions=32
  = 640 potential partition streams
```

Budget formula:

```rust id="ysqak4"
pub fn max_queries_for_partition_budget(
    global_partition_budget: usize,
    per_query_target_partitions: usize,
) -> usize {
    if per_query_target_partitions == 0 {
        return 0;
    }
    global_partition_budget / per_query_target_partitions
}
```

Policy:

```text id="nq9wge"
Interactive service:
  lower target_partitions per query
  cap concurrent queries
  cap remote requests separately

Batch:
  high target_partitions
  low concurrent query count
```

---

## 54.19 Temp dir quotas

### 54.19.1 Runtime config

```sql id="71riwz"
SET datafusion.runtime.temp_directory = '/mnt/datafusion-spill';
SET datafusion.runtime.max_temp_directory_size = '100G';
```

DataFusion configuration docs list `datafusion.runtime.temp_directory` and `datafusion.runtime.max_temp_directory_size`; `max_temp_directory_size` defaults to `100G` in the current docs. ([Apache DataFusion][3])

### 54.19.2 Quota policy

```text id="u4rcbc"
Per service:
  global spill mount quota
  monitor free bytes
  alarm before full

Per tenant:
  separate spill subdir
  max temp dir size
  cleanup on cancellation/failure

Per query:
  row/byte/memory admission
  avoid known spill-heavy topology unless budget allows
```

Agent rule:

```text id="89ogq2"
Disk spill is not free memory.
Spill consumes local disk, IO bandwidth, and cleanup reliability.
```

---

## 54.20 Runtime diagnostics

### 54.20.1 Config snapshot

```rust id="36s91f"
pub fn log_runtime_config(ctx: &SessionContext) {
    let state = ctx.state();
    let options = state.config_options();

    tracing::info!(
        batch_size = options.execution.batch_size,
        target_partitions = options.execution.target_partitions,
        "datafusion execution config"
    );
}
```

### 54.20.2 Physical plan inspection

```rust id="p3h4n9"
use datafusion::physical_plan::{displayable, ExecutionPlanProperties};

pub async fn inspect_runtime_plan(df: DataFrame) -> datafusion::error::Result<()> {
    let plan = df.create_physical_plan().await?;

    println!("{}", displayable(plan.as_ref()).indent(true));
    println!("partitioning={:?}", plan.output_partitioning());
    println!("boundedness={:?}", plan.boundedness());
    println!("pipeline={:?}", plan.pipeline_behavior());

    Ok(())
}
```

### 54.20.3 Metrics to track

```text id="j39jl8"
query:
  elapsed wall time
  output rows / bytes / batches
  first batch latency
  final batch latency
  cancellation count
  timeout count

operators:
  elapsed_compute
  output_rows
  spill_count / spill_bytes where exposed
  memory peak where exposed
  repartition rows
  sort elapsed
  join build/probe timing where exposed

runtime:
  memory reserved
  top memory consumers
  temp dir bytes used
  object-store requests
  object-store bytes
  metadata/listing cache size/hit rate

service:
  admitted queries
  queued queries
  rejected queries
  active partition streams
  per-tenant resource use
```

---

## 54.21 Runtime planning checklist by operator class

### Sort

```text id="5mjwdg"
[ ] ORDER BY necessary?
[ ] LIMIT/top-k possible?
[ ] input already ordered?
[ ] projected row width minimized?
[ ] memory budget sufficient?
[ ] spill dir configured?
```

### Hash join

```text id="z6x3ci"
[ ] build side small/narrow?
[ ] filters pushed before join?
[ ] join keys normalized?
[ ] target_partitions appropriate?
[ ] memory/spill budget sufficient?
[ ] dynamic filters useful?
```

### Group by

```text id="nvb0be"
[ ] group cardinality estimated?
[ ] partial/final aggregate topology expected?
[ ] target_partitions appropriate?
[ ] high-cardinality keys?
[ ] distinct aggregates?
[ ] spill configured?
```

### Remote scan

```text id="71z009"
[ ] projection pushed?
[ ] exact/inexact filters classified?
[ ] page size bounded?
[ ] max in-flight requests bounded?
[ ] cancellation stops remote work?
[ ] object-store/API credentials scoped?
```

---

## 54.22 Deployment reference profiles

### 54.22.1 Low-latency read-only API

```text id="7ne2vs"
target_partitions:
  low to moderate

batch_size:
  moderate, e.g. 2048-8192

execution:
  execute_stream

limits:
  max_rows
  max_output_bytes
  timeout

runtime:
  memory-limited
  temp dir quota
  result streaming
  no collect
```

### 54.22.2 Heavy batch worker

```text id="whcwy5"
target_partitions:
  CPU count or higher if IO-bound and source supports

batch_size:
  throughput-oriented

runtime:
  large memory budget
  fast spill disk
  broad temp quota

execution:
  write_* or streamed sink

admission:
  few concurrent queries
```

### 54.22.3 Multi-tenant query service

```text id="3oofiv"
per tenant:
  table/catalog scope
  object-store credentials
  concurrency cap
  memory/temp quota

per request:
  timeout
  result cap
  target_partitions cap
  stream cancellation
  lint expensive plan shapes
```

---

## 54.23 Anti-pattern inventory

* Assuming `execute_stream` means no memory pressure.
* Setting `target_partitions` to CPU count for every tiny query.
* Running many concurrent queries each with high `target_partitions`.
* Using `collect()` in service endpoints.
* Building custom operators that buffer without `MemoryReservation`.
* Using unbounded channels between blocking producers and streams.
* Ignoring stream drop/cancellation in custom sources.
* Spilling to shared `/tmp` with no quota.
* Sharing one object-store registry across tenants with different credentials.
* Treating `RuntimeEnvBuilder::with_memory_limit` as a perfect hard sandbox.
* Failing to configure temp directory for sort/join/group-by-heavy workloads.
* Running blocking sync I/O on core async runtime threads.
* Letting query CPU tasks starve server/network runtime.
* Forgetting to recompute budget after multiplying queries by partitions.
* Not tracking top memory/spill consumers in production.

---

## 54.24 Agent checklist

```text id="4frh9d"
[ ] Configure:
    SessionConfig.batch_size
    SessionConfig.target_partitions
    RuntimeEnv.memory_limit
    RuntimeEnv.temp_directory
    RuntimeEnv.max_temp_directory_size
    object-store registry

[ ] Understand partition execution:
    output_partitioning partition_count
    execute(partition, TaskContext)
    one SendableRecordBatchStream per partition

[ ] Use TaskContext:
    runtime resources
    memory_pool
    object stores
    config
    functions

[ ] Memory:
    reserve for large state
    use MemoryConsumer / MemoryReservation
    shrink/drop reservations
    remember not all allocations are tracked

[ ] Spill:
    sort spill
    hash join spill/memory
    group-by spill
    fast local temp dir
    quota and cleanup

[ ] Cancellation:
    drop stream on timeout/disconnect
    custom streams stop producers
    bounded channels
    abort remote requests

[ ] Async/blocking:
    async object-store/network preferred
    blocking sources isolated
    CPU runtime not overloaded with request I/O

[ ] Service budgets:
    per-query memory
    per-query timeout
    per-query result rows/bytes
    per-query target_partitions
    per-tenant concurrency
    per-tenant memory/temp/remote quota
    global partition-stream budget

[ ] Diagnostics:
    EXPLAIN ANALYZE
    physical plan properties
    memory consumers
    spill bytes
    first-batch latency
    cancellation metrics
```

[1]: https://docs.rs/datafusion/latest/datafusion/physical_plan/trait.ExecutionPlan.html?utm_source=chatgpt.com "ExecutionPlan in datafusion::physical_plan - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/runtime_env/struct.RuntimeEnv.html?utm_source=chatgpt.com "RuntimeEnv in datafusion::execution::runtime_env - Rust"
[3]: https://datafusion.apache.org/user-guide/configs.html?utm_source=chatgpt.com "Configuration Settings — Apache DataFusion documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionConfig.html?utm_source=chatgpt.com "SessionConfig in datafusion::execution::context - Rust"
[5]: https://docs.rs/deltalake/latest/deltalake/datafusion/execution/runtime_env/struct.RuntimeEnvBuilder.html?utm_source=chatgpt.com "RuntimeEnvBuilder in deltalake::datafusion - runtime_env"
[6]: https://docs.rs/datafusion-execution?utm_source=chatgpt.com "datafusion_execution - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/execution/memory_pool/trait.MemoryPool.html?utm_source=chatgpt.com "MemoryPool in datafusion::execution::memory_pool - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/execution/index.html?utm_source=chatgpt.com "datafusion::execution - Rust"
[9]: https://datafusion.apache.org/_sources/user-guide/configs.md.txt?utm_source=chatgpt.com "Show Source - Apache DataFusion"
[10]: https://datafusion.apache.org/blog/2025/06/30/cancellation/?utm_source=chatgpt.com "Using Rust async for Query Execution and Cancelling Long ..."
[11]: https://github.com/apache/arrow-datafusion/blob/main/datafusion/core/src/lib.rs?utm_source=chatgpt.com "datafusion/datafusion/core/src/lib.rs at main"


## 54.25 File-stream work stealing (DF 54)

### 54.25.1 Mechanism

Distinct from Tokio's task-level work stealing (54.14.1), DataFusion 54 adds file-level work stealing between sibling file-scan partitions: instead of each partition owning a fixed list of files, sibling `FileStream`s of one `DataSourceExec` can share a single per-execution queue of unopened files (`SharedWorkSource`, `datafusion-datasource/src/file_stream/work_source.rs`) and pop the next file when idle. A partition that finishes its files early steals remaining files from slower siblings, smoothing skew from uneven file sizes.

### 54.25.2 Trait plumbing

The public seam is on the `DataSource` trait (`datafusion-datasource/src/source.rs`); the queue type itself is crate-internal:

```rust id="wsteal54"
// DataSource additions (default-bodied — existing impls keep compiling):
fn create_sibling_state(&self) -> Option<Arc<dyn Any + Send + Sync>> { None }

fn open_with_args(&self, args: OpenArgs) -> Result<SendableRecordBatchStream> {
    self.open(args.partition, args.context)   // default delegates
}

pub struct OpenArgs {
    pub partition: usize,
    pub context: Arc<TaskContext>,
    pub sibling_state: Option<Arc<dyn Any + Send + Sync>>,  // shared across siblings
}
```

The executor calls `create_sibling_state()` once per execution and passes the same `sibling_state` to every partition's `open_with_args`. `FileScanConfig` implements this by handing out a shared work queue; custom `DataSource` implementations that want cross-partition coordination implement the same pair.

### 54.25.3 Gating — no config key

There is **no** SQL/config key for this in 54.1.0 (`execution.enable_file_stream_work_stealing` does not exist). Gating is structural, on `FileScanConfig`:

```text id="wsteal54b"
sharing DISABLED (create_sibling_state returns None) when:
  preserve_order = true               file order must be preserved
  partitioned_by_file_group = true    file groups define output partitioning
                                      (Hash on partition columns — see 51.10.2)

otherwise:
  siblings share one queue of unopened files
```

### 54.25.4 Distributed / isolated-partition warning

Work stealing assumes all sibling partitions run in one process sharing one queue. Distributed schedulers that execute each partition as an isolated task (serialize the plan, run `execute(i)` on different nodes) get no shared state — each isolated partition would see the full file list as its local queue, duplicating reads. Such frameworks must set `partitioned_by_file_group = true` (or otherwise pin files to partitions) so partition→file assignment stays static. Even in-process, note that with sharing enabled the mapping from partition index to file is no longer deterministic — per-partition golden tests must not assume which partition produced which file's rows.

---

# DataFusion Advanced — 55) Planning artifact package: reproducible plan debug bundle

## 55.0 Purpose

Define a **portable, reproducible, agent-consumable planning artifact bundle**:

```text id="uqjeom"
query input
  → planning environment snapshot
  → catalog/schema/statistics snapshot
  → logical plan artifacts
  → physical plan artifacts
  → explain artifacts
  → execution metrics
  → output schema/result summary
  → regression/golden-test package
  → LLM-agent handoff bundle
```

The attached documentation already identifies `EXPLAIN`, `EXPLAIN VERBOSE`, `EXPLAIN ANALYZE`, physical metrics, custom operator explainability, and regression baseline policy as core DataFusion diagnostics, and it explicitly recommends pinning DataFusion versions for exact plan-string snapshots.  This chapter turns those practices into a concrete artifact schema and workflow.

---

## 55.1 Artifact bundle mental model

```text id="q9uwe8"
PlanningArtifactBundleV1
  ├─ identity
  │   ├─ query_id
  │   ├─ artifact_id
  │   ├─ created_at
  │   ├─ environment
  │   └─ redaction policy
  ├─ input
  │   ├─ SQL / PlanSpec / QuerySpec
  │   ├─ parser dialect
  │   └─ parameter values / parameter schema
  ├─ engine
  │   ├─ DataFusion version
  │   ├─ Arrow version
  │   ├─ object_store version
  │   ├─ feature flags
  │   └─ Cargo lock hash
  ├─ session
  │   ├─ SessionConfig / ConfigOptions
  │   ├─ RuntimeEnv summary
  │   ├─ optimizer/analyzer/physical optimizer rule set
  │   └─ function registry snapshot
  ├─ catalog
  │   ├─ catalog/schema/table refs
  │   ├─ table schemas
  │   ├─ statistics
  │   ├─ constraints
  │   ├─ object-store path metadata
  │   └─ freshness/version hashes
  ├─ plans
  │   ├─ unoptimized logical plan
  │   ├─ analyzed logical plan if exposed
  │   ├─ optimized logical plan
  │   ├─ initial physical plan if exposed
  │   ├─ optimized physical plan
  │   └─ physical properties summary
  ├─ explain
  │   ├─ EXPLAIN
  │   ├─ EXPLAIN VERBOSE
  │   └─ EXPLAIN ANALYZE
  ├─ execution
  │   ├─ metrics
  │   ├─ row-count summary
  │   ├─ output schema
  │   ├─ result sample / snapshot pointer
  │   └─ errors / warnings
  └─ regression
      ├─ normalized snapshots
      ├─ expected output schema
      ├─ metric thresholds
      └─ comparison policy
```

Value case:

```text id="iq7b96"
Debug:
  reproduce what the planner saw.

Regression:
  detect plan-shape, schema, metric, and result drift.

LLM handoff:
  let an agent inspect summary first, then reveal details incrementally.

Upgrade:
  compare DataFusion vN vs vN+1 under pinned config/catalog/input.

Production:
  audit high-cost queries without exposing secrets or raw data.
```

---

## 55.2 Artifact identity

```rust id="tc0rsj"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactIdentity {
    pub artifact_schema_version: String, // "PlanningArtifactBundleV1"
    pub artifact_id: String,
    pub query_id: String,
    pub created_at_utc: String,
    pub created_by: ArtifactProducer,
    pub redaction_profile: RedactionProfile,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArtifactProducer {
    pub service_name: String,
    pub service_version: String,
    pub host_fingerprint: Option<String>,
    pub git_commit: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RedactionProfile {
    PublicSafe,
    InternalNoSecrets,
    InternalFull,
    TestFixture,
}
```

Identity policy:

```text id="yisyck"
artifact_id:
  unique stable artifact object id

query_id:
  ties SQL/API request/logs/traces/metrics

created_at:
  UTC ISO-8601

producer:
  service version, git commit, host/runtime class

redaction_profile:
  controls SQL literal masking, path masking, stats masking, parameter masking
```

---

## 55.3 Input artifact

### 55.3.1 SQL input

```rust id="p8h09c"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqlInputArtifact {
    pub raw_sql: String,
    pub normalized_sql: Option<String>,
    pub sql_hash_sha256: String,
    pub parser_dialect: String,
    pub parser_options: BTreeMap<String, String>,
    pub statement_class: Option<String>,
    pub parameters: Vec<ParameterArtifact>,
}
```

### 55.3.2 PlanSpec / QuerySpec input

```rust id="73gk2l"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanSpecInputArtifact {
    pub planspec_json: serde_json::Value,
    pub planspec_schema_version: String,
    pub planspec_hash_sha256: String,
    pub parameter_schema: Vec<ParameterDefinition>,
    pub parameter_values_redacted: Vec<ParameterArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterArtifact {
    pub name: Option<String>,
    pub position: Option<usize>,
    pub data_type: String,
    pub value_redacted: String,
    pub value_hash_sha256: Option<String>,
}
```

### 55.3.3 Input policy

```text id="9m6b48"
Raw SQL:
  store only in internal-safe artifacts.
  redact secrets, credentials, access tokens, object-store options.

Normalized SQL:
  preferred for snapshots.
  strip comments if comments contain user/secrets.

Parameters:
  store type and nullability.
  hash values when needed.
  redact literal values by policy.

PlanSpec:
  store canonical JSON with stable key order.
  hash before compilation.
  include schema version.
```

---

## 55.4 Engine and build snapshot

```rust id="13x7ua"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineSnapshot {
    pub datafusion_version: String,
    pub arrow_version: String,
    pub parquet_version: Option<String>,
    pub object_store_version: Option<String>,
    pub rustc_version: Option<String>,
    pub target_triple: Option<String>,
    pub cargo_lock_hash_sha256: Option<String>,
    pub feature_flags: BTreeSet<String>,
    pub datafusion_subcrate_versions: BTreeMap<String, String>,
}
```

Capture sources:

```text id="2xnrd8"
Cargo.toml / Cargo.lock:
  exact datafusion/datafusion-* versions
  arrow/parquet/object_store versions
  enabled features

Build:
  rustc version
  target triple
  release/debug profile
  target-cpu / LTO / PGO flags if relevant

Runtime:
  process binary version
  git commit
```

Version policy:

```text id="zdny0k"
Exact physical plan snapshots require:
  DataFusion version pin
  Arrow version pin
  feature flag pin
  config pin
  catalog/schema/statistics pin

Cross-version comparison:
  treat exact plan-string drift as expected.
  compare semantic schema/result and key operator classes separately.
```

---

## 55.5 Session/config snapshot

```rust id="e0rh4a"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionSnapshot {
    pub config_options_json: serde_json::Value,
    pub session_config_summary: BTreeMap<String, String>,
    pub optimizer_rules: Vec<String>,
    pub analyzer_rules: Vec<String>,
    pub physical_optimizer_rules: Vec<String>,
    pub query_planner: String,
    pub sql_options: Option<SqlOptionsArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SqlOptionsArtifact {
    pub allow_ddl: bool,
    pub allow_dml: bool,
    pub allow_statements: bool,
}
```

Suggested config keys to always surface in summary:

```text id="dymvac"
datafusion.execution.batch_size
datafusion.execution.target_partitions
datafusion.execution.time_zone
datafusion.execution.collect_statistics
datafusion.optimizer.*
datafusion.sql_parser.*
datafusion.execution.parquet.*
datafusion.runtime.memory_limit
datafusion.runtime.temp_directory
datafusion.runtime.max_temp_directory_size
```

Capture helper shape:

```rust id="b5l3aa"
pub fn capture_session_snapshot(
    ctx: &datafusion::prelude::SessionContext,
) -> SessionSnapshot {
    let state = ctx.state();
    let options = state.config_options();

    SessionSnapshot {
        // Prefer typed serialization if exposed in pinned API; otherwise store selected keys.
        config_options_json: serde_json::json!({
            "execution": {
                "batch_size": options.execution.batch_size,
                "target_partitions": options.execution.target_partitions,
            }
        }),
        session_config_summary: BTreeMap::new(),
        optimizer_rules: state.optimizer().rules().iter().map(|r| r.name().to_string()).collect(),
        analyzer_rules: vec![],
        physical_optimizer_rules: vec![],
        query_planner: "default_or_custom_query_planner".to_string(),
        sql_options: None,
    }
}
```

Exact method names for rules may vary by DataFusion version; the artifact contract is stable even when API hooks differ.

---

## 55.6 Runtime snapshot

```rust id="n761c1"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuntimeSnapshot {
    pub memory_limit_bytes: Option<usize>,
    pub memory_pool_kind: String,
    pub temp_directory_redacted: Option<String>,
    pub max_temp_directory_size_bytes: Option<usize>,
    pub object_store_roots: Vec<ObjectStoreRootArtifact>,
    pub cache_summary: CacheSummaryArtifact,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObjectStoreRootArtifact {
    pub scheme: String,
    pub authority_redacted: String,
    pub root_redacted: String,
    pub credential_source_redacted: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheSummaryArtifact {
    pub metadata_cache_enabled: Option<bool>,
    pub statistics_cache_enabled: Option<bool>,
    pub list_files_cache_enabled: Option<bool>,
    pub cache_limits: BTreeMap<String, String>,
}
```

Runtime snapshot purpose:

```text id="8whpkl"
Plan drift:
  target_partitions and memory config can alter physical topology.

Performance drift:
  memory/temp/cache settings alter spill and scan behavior.

Security:
  object-store roots must be redacted.

Repro:
  local file paths, object-store prefixes, temp dirs, cache state affect file scans.
```

---

## 55.7 Catalog snapshot

```rust id="x48acl"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogSnapshot {
    pub default_catalog: Option<String>,
    pub default_schema: Option<String>,
    pub tables: Vec<TableArtifact>,
    pub functions: FunctionRegistryArtifact,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableArtifact {
    pub table_ref: String,
    pub provider_type: String,
    pub table_type: String,
    pub schema: SchemaArtifact,
    pub statistics: Option<StatisticsArtifact>,
    pub constraints: Option<ConstraintsArtifact>,
    pub source_metadata: SourceMetadataArtifact,
    pub freshness: Option<MetadataFreshnessArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SchemaArtifact {
    pub fields: Vec<FieldArtifact>,
    pub schema_metadata_redacted: BTreeMap<String, String>,
    pub schema_hash_sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FieldArtifact {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub metadata_redacted: BTreeMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionRegistryArtifact {
    pub scalar_functions: Vec<FunctionArtifact>,
    pub aggregate_functions: Vec<FunctionArtifact>,
    pub window_functions: Vec<FunctionArtifact>,
    pub table_functions: Vec<FunctionArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionArtifact {
    pub name: String,
    pub kind: String,
    pub volatility: Option<String>,
    pub signature_redacted: Option<String>,
    pub implementation_version: Option<String>,
}
```

Catalog policy:

```text id="tq9aro"
Always capture:
  referenced table schemas
  referenced table provider type
  referenced function names
  output schema

Capture when debugging optimizer/performance:
  statistics
  constraints
  object-store/file metadata
  partition metadata

Avoid:
  full catalog dump for multi-tenant artifacts unless authorized
  raw credentials in source metadata
  sensitive min/max/NDV stats in public artifacts
```

---

## 55.8 Table statistics artifact

```rust id="s6ha8w"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StatisticsArtifact {
    pub num_rows: PrecisionArtifact<usize>,
    pub total_byte_size: PrecisionArtifact<usize>,
    pub columns: Vec<ColumnStatisticsArtifact>,
    pub statistics_hash_sha256: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PrecisionArtifact<T> {
    Exact(T),
    Inexact(T),
    Absent,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColumnStatisticsArtifact {
    pub column_name: String,
    pub min_value_redacted: Option<String>,
    pub max_value_redacted: Option<String>,
    pub null_count: PrecisionArtifact<usize>,
    pub distinct_count: Option<PrecisionArtifact<usize>>,
}
```

Statistics policy:

```text id="0768tr"
Exact stats:
  include exactness marker.

Inexact stats:
  include estimate marker.

Absent stats:
  record absence explicitly.

Sensitive stats:
  redact values but preserve precision class.
```

---

## 55.9 Object-store path metadata

```rust id="t5o6so"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourceMetadataArtifact {
    pub source_kind: String, // parquet, csv, json, avro, memory, custom
    pub root_uri_redacted: Option<String>,
    pub path_count: Option<usize>,
    pub file_groups: Option<Vec<FileGroupArtifact>>,
    pub hive_partition_columns: Vec<String>,
    pub listing_cache_key: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileGroupArtifact {
    pub partition_index: usize,
    pub files: Vec<FileArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileArtifact {
    pub path_redacted: String,
    pub size_bytes: Option<u64>,
    pub modified_redacted: Option<String>,
    pub etag_redacted: Option<String>,
    pub version_redacted: Option<String>,
    pub partition_values_redacted: BTreeMap<String, String>,
}
```

Redaction:

```text id="wriiau"
s3://secret-bucket/customer-a/events/date=2026-05-22/file.parquet
  → s3://<bucket_hash>/<prefix_hash>/events/date=<redacted>/file.parquet

Preserve:
  file count
  extension
  grouping shape
  partition column names
  approximate sizes if allowed

Redact:
  bucket/account names
  tenant/user identifiers
  credential-bearing query params
  sensitive partition values
```

---

## 55.10 Plan artifacts

### 55.10.1 Logical plan artifacts

```rust id="1h2f2e"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LogicalPlanArtifacts {
    pub unoptimized_plan_indent: String,
    pub unoptimized_plan_debug: Option<String>,
    pub unoptimized_schema: SchemaArtifact,
    pub unoptimized_plan_hash_sha256: String,

    pub analyzed_plan_indent: Option<String>,
    pub analyzed_plan_hash_sha256: Option<String>,

    pub optimized_plan_indent: String,
    pub optimized_schema: SchemaArtifact,
    pub optimized_plan_hash_sha256: String,
}
```

Capture options:

```rust id="2zgwea"
pub async fn capture_logical_plans(
    ctx: &SessionContext,
    sql: &str,
) -> datafusion::error::Result<LogicalPlanArtifacts> {
    let state = ctx.state();

    let initial = state.create_logical_plan(sql).await?;
    let optimized = state.optimize(&initial)?;

    Ok(LogicalPlanArtifacts {
        unoptimized_plan_indent: initial.display_indent_schema().to_string(),
        unoptimized_plan_debug: Some(format!("{initial:#?}")),
        unoptimized_schema: schema_artifact_from_dfschema(initial.schema()),
        unoptimized_plan_hash_sha256: sha256_string(&initial.display_indent_schema().to_string()),

        analyzed_plan_indent: None,
        analyzed_plan_hash_sha256: None,

        optimized_plan_indent: optimized.display_indent_schema().to_string(),
        optimized_schema: schema_artifact_from_dfschema(optimized.schema()),
        optimized_plan_hash_sha256: sha256_string(&optimized.display_indent_schema().to_string()),
    })
}
```

Note:

```text id="jsylgz"
DataFusion public convenience APIs may combine analyzer/optimizer/physical planning phases.
If analyzed plan is not separately exposed in pinned API, store None.
If instrumenting DataFusion internals/custom optimizer, capture analyzer output explicitly.
```

### 55.10.2 Physical plan artifacts

```rust id="d3hc2m"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPlanArtifacts {
    pub initial_physical_plan_indent: Option<String>,
    pub initial_physical_plan_hash_sha256: Option<String>,

    pub optimized_physical_plan_indent: String,
    pub optimized_physical_plan_hash_sha256: String,
    pub output_schema: SchemaArtifact,
    pub root_properties: PhysicalPropertiesArtifact,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PhysicalPropertiesArtifact {
    pub output_partitioning: String,
    pub output_ordering: Option<String>,
    pub boundedness: String,
    pub pipeline_behavior: String,
    pub equivalence_summary: Option<String>,
}
```

Capture:

```rust id="9jafc0"
use datafusion::physical_plan::{displayable, ExecutionPlanProperties};

pub async fn capture_physical_plan(
    df: DataFrame,
) -> datafusion::error::Result<PhysicalPlanArtifacts> {
    let physical = df.create_physical_plan().await?;
    let rendered = displayable(physical.as_ref()).indent(true).to_string();

    Ok(PhysicalPlanArtifacts {
        initial_physical_plan_indent: None,
        initial_physical_plan_hash_sha256: None,
        optimized_physical_plan_indent: rendered.clone(),
        optimized_physical_plan_hash_sha256: sha256_string(&rendered),
        output_schema: schema_artifact_from_arrow(physical.schema().as_ref()),
        root_properties: PhysicalPropertiesArtifact {
            output_partitioning: format!("{:?}", physical.output_partitioning()),
            output_ordering: physical.output_ordering().map(|o| format!("{o:?}")),
            boundedness: format!("{:?}", physical.boundedness()),
            pipeline_behavior: format!("{:?}", physical.pipeline_behavior()),
            equivalence_summary: Some(format!("{:?}", physical.equivalence_properties())),
        },
    })
}
```

Exact physical plan before physical optimizer may not be publicly exposed by default. Capture it via custom planner instrumentation, physical optimizer observer hooks, or patched/test-only APIs when needed.

---

## 55.11 `EXPLAIN` artifacts

```rust id="0ahgw6"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExplainArtifacts {
    pub explain: Option<String>,
    pub explain_verbose: Option<String>,
    pub explain_analyze: Option<String>,
    pub explain_format: String, // indent/tree/json-like if supported
}
```

SQL capture:

```rust id="m8ih9e"
use datafusion::arrow::util::pretty::pretty_format_batches;
use datafusion::prelude::*;

pub async fn explain_text(
    ctx: &SessionContext,
    sql: &str,
    verbose: bool,
    analyze: bool,
) -> datafusion::error::Result<String> {
    let explain_sql = match (verbose, analyze) {
        (false, false) => format!("EXPLAIN {sql}"),
        (true, false) => format!("EXPLAIN VERBOSE {sql}"),
        (_, true) => format!("EXPLAIN ANALYZE {sql}"),
    };

    let batches = ctx.sql(&explain_sql).await?.collect().await?;
    Ok(pretty_format_batches(&batches)?.to_string())
}
```

Policy:

```text id="ghsvcy"
EXPLAIN:
  plan shape only; safe for plan debugging without running query.

EXPLAIN VERBOSE:
  optimizer/physical planner debugging; may include more internals.

EXPLAIN ANALYZE:
  executes query; use only in bounded/test/internal contexts or with strict limits.
```

---

## 55.12 Metrics artifacts

```rust id="reg042"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsArtifacts {
    pub query_wall_time_ms: Option<u128>,
    pub first_batch_latency_ms: Option<u128>,
    pub output_rows: Option<usize>,
    pub output_batches: Option<usize>,
    pub output_bytes_estimate: Option<usize>,
    pub operators: Vec<OperatorMetricsArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OperatorMetricsArtifact {
    pub node_path: Vec<String>,
    pub operator_name: String,
    pub metrics: BTreeMap<String, String>,
}
```

Execution wrapper:

```rust id="uhbwx8"
use std::time::Instant;
use futures::StreamExt;

pub async fn collect_metrics_summary(
    df: DataFrame,
) -> datafusion::error::Result<MetricsArtifacts> {
    let started = Instant::now();

    let physical = df.create_physical_plan().await?;
    let mut stream = physical.execute(0, df.task_ctx())?;

    let mut rows = 0usize;
    let mut batches = 0usize;
    let mut first_batch_latency_ms = None;

    while let Some(batch) = stream.next().await {
        let batch = batch?;
        if first_batch_latency_ms.is_none() {
            first_batch_latency_ms = Some(started.elapsed().as_millis());
        }
        rows += batch.num_rows();
        batches += 1;
    }

    Ok(MetricsArtifacts {
        query_wall_time_ms: Some(started.elapsed().as_millis()),
        first_batch_latency_ms,
        output_rows: Some(rows),
        output_batches: Some(batches),
        output_bytes_estimate: None,
        operators: vec![], // fill by traversing ExecutionPlan::metrics under pinned API
    })
}
```

Metrics policy:

```text id="wbf7w5"
Always useful:
  output_rows
  output_batches
  elapsed_compute
  first_batch_latency
  wall time

Performance baselines:
  compare thresholds, not exact nanoseconds.

Custom operators:
  must expose metrics before production.

Security:
  avoid high-cardinality tenant/user values as metric labels.
```

---

## 55.13 Output schema and row-count summary

```rust id="iusm42"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputArtifact {
    pub output_schema: SchemaArtifact,
    pub row_count_summary: RowCountSummary,
    pub sample_batches_uri: Option<String>,
    pub result_snapshot_hash_sha256: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RowCountSummary {
    pub exact_rows: Option<usize>,
    pub lower_bound_rows: Option<usize>,
    pub batches_observed: Option<usize>,
    pub truncated_by_sample_limit: bool,
    pub result_limit: Option<usize>,
}
```

Policy:

```text id="gs2mh2"
Exact row count:
  only if full bounded result consumed or operator metrics trusted.

Lower bound:
  if sampling/truncation stopped early.

Sample:
  store tiny sample only.
  never store sensitive rows in public artifacts.
  hash full result in regression tests when possible.
```

---

## 55.14 Complete Rust bundle schema

```rust id="jm41s7"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningArtifactBundleV1 {
    pub identity: ArtifactIdentity,
    pub input: QueryInputArtifact,
    pub engine: EngineSnapshot,
    pub session: SessionSnapshot,
    pub runtime: RuntimeSnapshot,
    pub catalog: CatalogSnapshot,
    pub logical: LogicalPlanArtifacts,
    pub physical: PhysicalPlanArtifacts,
    pub explain: ExplainArtifacts,
    pub metrics: Option<MetricsArtifacts>,
    pub output: Option<OutputArtifact>,
    pub regression: RegressionPolicyArtifact,
    pub warnings: Vec<String>,
    pub errors: Vec<PlanningErrorArtifact>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum QueryInputArtifact {
    Sql(SqlInputArtifact),
    PlanSpec(PlanSpecInputArtifact),
    DataFrameFactory {
        factory_name: String,
        request_json: serde_json::Value,
        request_hash_sha256: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningErrorArtifact {
    pub phase: String,
    pub error_class: String,
    pub message_redacted: String,
    pub node_path: Option<Vec<String>>,
    pub expression_path: Option<Vec<String>>,
    pub remediation: Option<String>,
}
```

---

## 55.15 Capture pipeline

```text id="gngspe"
1. Assign query_id/artifact_id.
2. Redact/canonicalize input.
3. Capture engine/build snapshot.
4. Capture SessionConfig/ConfigOptions.
5. Capture catalog refs/table schemas/function registry.
6. Build unoptimized LogicalPlan.
7. Capture logical schema and plan string.
8. Run analyzer/optimizer or SessionState::optimize.
9. Capture optimized LogicalPlan.
10. Build physical plan.
11. Capture physical plan/properties.
12. Optionally run EXPLAIN / EXPLAIN VERBOSE.
13. Optionally run EXPLAIN ANALYZE under guardrails.
14. Capture metrics/output schema/row summary.
15. Normalize snapshots.
16. Serialize JSON + markdown summary + compact text snapshots.
```

Orchestrator skeleton:

```rust id="uvh11f"
pub async fn build_planning_artifact_for_sql(
    ctx: &SessionContext,
    sql: &str,
    artifact_policy: ArtifactPolicy,
) -> datafusion::error::Result<PlanningArtifactBundleV1> {
    let identity = make_identity(&artifact_policy);
    let input = QueryInputArtifact::Sql(capture_sql_input(sql, &artifact_policy)?);
    let engine = capture_engine_snapshot()?;
    let session = capture_session_snapshot(ctx);
    let runtime = capture_runtime_snapshot(ctx, &artifact_policy)?;
    let catalog = capture_catalog_snapshot(ctx, sql, &artifact_policy).await?;

    let logical = capture_logical_plans(ctx, sql).await?;

    let df = ctx.sql(sql).await?;
    let physical = capture_physical_plan(df.clone()).await?;

    let explain = if artifact_policy.capture_explain {
        ExplainArtifacts {
            explain: Some(explain_text(ctx, sql, false, false).await?),
            explain_verbose: if artifact_policy.capture_explain_verbose {
                Some(explain_text(ctx, sql, true, false).await?)
            } else {
                None
            },
            explain_analyze: if artifact_policy.capture_explain_analyze {
                Some(explain_text(ctx, sql, false, true).await?)
            } else {
                None
            },
            explain_format: "pretty_table".to_string(),
        }
    } else {
        ExplainArtifacts {
            explain: None,
            explain_verbose: None,
            explain_analyze: None,
            explain_format: "none".to_string(),
        }
    };

    Ok(PlanningArtifactBundleV1 {
        identity,
        input,
        engine,
        session,
        runtime,
        catalog,
        logical,
        physical,
        explain,
        metrics: None,
        output: None,
        regression: RegressionPolicyArtifact::default(),
        warnings: vec![],
        errors: vec![],
    })
}
```

---

## 55.16 Artifact formats

### 55.16.1 JSON

Use JSON as canonical machine-readable artifact.

```json id="2kyb1j"
{
  "identity": {
    "artifact_schema_version": "PlanningArtifactBundleV1",
    "artifact_id": "art_01J...",
    "query_id": "qry_01J...",
    "created_at_utc": "2026-05-24T00:00:00Z",
    "redaction_profile": "InternalNoSecrets"
  },
  "engine": {
    "datafusion_version": "54.1.0",
    "arrow_version": "58.4.0",
    "feature_flags": ["parquet", "sql"]
  },
  "logical": {
    "unoptimized_plan_hash_sha256": "...",
    "optimized_plan_hash_sha256": "..."
  },
  "physical": {
    "optimized_physical_plan_hash_sha256": "...",
    "root_properties": {
      "output_partitioning": "UnknownPartitioning(8)",
      "boundedness": "Bounded"
    }
  }
}
```

JSON policy:

```text id="6l9t58"
stable field order through serializer if snapshotting
canonical serialization for hashes
no raw secrets
large text artifacts may be externalized by URI/hash
```

### 55.16.2 Markdown

Use Markdown for human/agent reading.

````markdown id="4wwmxi"
# Query artifact: qry_...

## Summary
- status: success
- datafusion: 54.1.0
- output rows: 10,000
- warning count: 2

## Input
```sql
SELECT ...
````

## Optimized logical plan

```text
...
```

## Physical plan

```text
...
```

## Metrics

| operator | output_rows | elapsed_compute |
| -------- | ----------: | --------------: |

````

Markdown policy:

```text id="n0jv6e"
summary first
link/anchor sections
collapse long plan trees if UI supports it
store code-fenced plans
redact paths/secrets
````

### 55.16.3 Protobuf

Use protobuf for strongly typed binary interchange.

```text id="0a3a82"
PlanningArtifactBundleV1.proto:
  QueryInput
  EngineSnapshot
  SessionSnapshot
  CatalogSnapshot
  LogicalPlanArtifacts
  PhysicalPlanArtifacts
  ExplainArtifacts
  MetricsArtifacts
  OutputArtifact
```

Protobuf policy:

```text id="ndnx2l"
Use for:
  service-to-service artifacts
  durable binary records
  schema evolution

Avoid for:
  quick human debugging
  freeform plan text alone
```

DataFusion’s companion crates include `datafusion_proto` for plan serialization/deserialization and `datafusion_substrait` for Substrait interoperability; direct subcrate dependencies should be used only when the crate boundary is part of the interface and pinned to matching DataFusion versions. 

### 55.16.4 Compact text snapshots

Use for CI golden diffs:

```text id="if8w5d"
query_id=q_example
datafusion=54.1.0
config_hash=...
schema_hash=...

LOGICAL_OPTIMIZED:
Projection: ...
  Aggregate: ...
    Filter: ...
      DataSourceExec: ...

PHYSICAL:
SortExec: ...
  AggregateExec: ...
    RepartitionExec: ...
      DataSourceExec: ...
```

Snapshot policy:

```text id="62j3lh"
normalize dynamic paths
normalize query IDs
normalize temp dirs
normalize timings
version-pin physical snapshots
prefer semantic assertions for cross-version tests
```

---

## 55.17 Golden test policy

### 55.17.1 Logical snapshots

Use for:

```text id="fhhzlh"
optimizer regression
query compiler regression
PlanSpec compiler regression
logical rule changes
alias/schema stability
```

Policy:

```text id="y8jphm"
Snapshot:
  optimized logical plan
  output DFSchema
  referenced tables/functions
  lint report

Pin:
  DataFusion version
  config
  catalog schemas
  optimizer rule set
```

### 55.17.2 Physical snapshots

Use for:

```text id="d7eue5"
operator choice regression
repartition/sort placement
custom ExecutionPlan presence
provider pushdown regression
```

Policy:

```text id="p3d2h7"
Version-pin strictly.
Treat drift as review-required, not automatically failure across upgrades.
Normalize dynamic file paths and temp dirs.
Record ConfigOptions and physical optimizer rule set.
```

### 55.17.3 Result snapshots

Use for correctness:

```text id="03yuuh"
small deterministic fixture
ORDER BY included
stable aliases
stable schemas
null formatting specified
float tolerances defined
```

Pattern:

```rust id="a1hys9"
#[tokio::test]
async fn result_snapshot_is_stable() -> datafusion::error::Result<()> {
    let ctx = fixture_context().await?;

    let batches = ctx
        .sql("
          SELECT region, SUM(amount) AS total_amount
          FROM sales
          GROUP BY region
          ORDER BY region
        ")
        .await?
        .collect()
        .await?;

    let rendered = datafusion::arrow::util::pretty::pretty_format_batches(&batches)?.to_string();
    insta::assert_snapshot!(rendered);

    Ok(())
}
```

### 55.17.4 Metrics thresholds

```rust id="1lvf1w"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsThresholdPolicy {
    pub max_output_rows: Option<usize>,
    pub max_output_batches: Option<usize>,
    pub max_wall_time_ms: Option<u128>,
    pub max_first_batch_latency_ms: Option<u128>,
    pub max_repartition_count: Option<usize>,
    pub max_spill_bytes: Option<usize>,
    pub required_operator_names: BTreeSet<String>,
    pub forbidden_operator_names: BTreeSet<String>,
}
```

Policy:

```text id="05i6xi"
Do not compare exact timings.
Use thresholds/tolerances.
Compare operator presence/absence.
Compare row counts exactly.
Compare bytes approximately.
Run performance baselines in release builds only.
```

### 55.17.5 Version pinning

```text id="ls3n78"
Exact snapshot requires pinning:
  DataFusion version
  Arrow version
  feature flags
  ConfigOptions
  SQL dialect
  table schemas
  statistics
  function registry
  optimizer rule set
  physical optimizer rule set
```

---

## 55.18 Regression policy artifact

```rust id="k0e0bj"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RegressionPolicyArtifact {
    pub logical_snapshot_policy: SnapshotPolicy,
    pub physical_snapshot_policy: SnapshotPolicy,
    pub result_snapshot_policy: SnapshotPolicy,
    pub metrics_thresholds: Option<MetricsThresholdPolicy>,
    pub normalize_rules: Vec<NormalizeRule>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SnapshotPolicy {
    pub enabled: bool,
    pub strict_version_pin: bool,
    pub compare_exact_text: bool,
    pub compare_schema: bool,
    pub compare_operator_classes: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NormalizeRule {
    pub name: String,
    pub pattern: String,
    pub replacement: String,
}
```

Example normalization rules:

```text id="ky8qfj"
file paths:
  s3://bucket/customer-x/... → s3://<bucket>/<prefix>/...

temp dirs:
  /tmp/.tmpabc123 → <temp_dir>

query ids:
  qry_01J... → <query_id>

timings:
  elapsed_compute=12.34ms → elapsed_compute=<duration>

memory addresses:
  0x7ff... → <ptr>
```

---

## 55.19 Agent reveal protocol

LLM-agent handoff should be progressive. Do not dump all plans and metrics first.

### 55.19.1 Level 0 — summary first

```text id="bx2kpw"
Show:
  query_id
  status
  input kind
  main tables
  output schema hash
  root physical operator
  row count
  warnings/errors
  suspected bottleneck
```

Example:

```text id="6g8l09"
Summary:
  query_id: qry_abc
  status: success
  tables: events, users
  output: 8 fields, schema_hash=...
  optimized plan: Filter pushed to DataSourceExec
  physical plan: HashJoinExec + RepartitionExec
  rows: 1,248,003
  warnings: SortExec without LIMIT
```

### 55.19.2 Level 1 — plan tree next

```text id="a6uphu"
Reveal:
  optimized logical plan
  physical plan tree
  operator list
  table scan details
  joins/sorts/repartitions
```

### 55.19.3 Level 2 — node details on demand

```text id="uy99ro"
Reveal for selected node:
  node path
  input schema
  output schema
  expressions
  predicates
  partitioning
  ordering
  metrics
  pushed filters
  statistics
  source metadata
```

### 55.19.4 Level 3 — metrics last

```text id="snvw1q"
Reveal:
  operator metrics
  elapsed compute
  output rows/batches/bytes
  spill bytes
  scan pruning
  first batch latency
  wall-clock
```

### 55.19.5 Protocol schema

```rust id="mmb91b"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RevealLevel {
    Summary,
    PlanTree,
    NodeDetails { node_path: Vec<String> },
    Metrics,
    FullBundle,
}
```

Agent policy:

```text id="plhhlx"
Default:
  summary + plan tree.

Only reveal:
  node details when asked.
  metrics when performance question exists.
  raw SQL/table stats only if authorized.
  EXPLAIN ANALYZE only if query was safely executed.
```

---

## 55.20 Markdown artifact template

````markdown id="cxf3wz"
# DataFusion Planning Artifact

## Summary
- artifact_id:
- query_id:
- status:
- datafusion_version:
- arrow_version:
- config_hash:
- catalog_hash:
- output_schema_hash:
- main warning:

## Input
- kind:
- parser_dialect:
- sql_hash:
- parameters:

## Environment
- feature_flags:
- target_partitions:
- batch_size:
- memory_limit:
- temp_dir_redacted:

## Catalog Snapshot
| table | schema_hash | rows | bytes | stats_precision |
|---|---|---:|---:|---|

## Output Schema
| ordinal | name | type | nullable |
|---:|---|---|---|

## Optimized Logical Plan
```text
...
````

## Physical Plan

```text
...
```

## EXPLAIN

```text
...
```

## EXPLAIN VERBOSE

```text
...
```

## EXPLAIN ANALYZE

```text
...
```

## Metrics Summary

| node | operator | rows | batches | elapsed |
| ---- | -------- | ---: | ------: | ------: |

## Warnings

* ...

## Errors

* ...

## Reproduction Checklist

* DataFusion version:
* Cargo.lock hash:
* ConfigOptions hash:
* catalog snapshot:
* input fixture:

````

---

## 55.21 Artifact storage layout

```text id="xnyw6g"
artifacts/
  qry_20260524_abc123/
    bundle.json
    summary.md
    input.sql
    plans/
      logical_unoptimized.txt
      logical_optimized.txt
      physical_optimized.txt
      explain.txt
      explain_verbose.txt
      explain_analyze.txt
    schemas/
      output_schema.json
      table_schemas.json
    metrics/
      metrics.json
      operator_metrics.csv
    snapshots/
      result.pretty.txt
      result.arrow
      result.parquet
    redaction/
      redaction_report.json
````

Storage policy:

```text id="d1jsgu"
bundle.json:
  canonical machine-readable source of truth

summary.md:
  human/LLM first-read artifact

plans/*.txt:
  stable-ish text snapshots, version-pinned

schemas/*.json:
  semantic schema snapshots

metrics/*.json/csv:
  threshold comparisons

result.*:
  test-only; never store sensitive production rows unless authorized
```

---

## 55.22 Security and redaction

### 55.22.1 Redaction targets

```text id="y1ylmg"
SQL literals:
  emails, account ids, tokens, addresses

Object-store paths:
  bucket names, tenant prefixes, signed URLs

Credentials:
  AWS/GCP keys, headers, query params

Statistics:
  sensitive min/max, NDV, row counts by tenant

Schemas:
  sensitive column names if public artifact

Metrics:
  high-cardinality labels with user/tenant identifiers

Errors:
  source paths, provider errors with secrets
```

### 55.22.2 Redaction artifact

```rust id="waf1bi"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RedactionReport {
    pub profile: RedactionProfile,
    pub rules_applied: Vec<String>,
    pub fields_redacted: Vec<String>,
    pub raw_sql_stored: bool,
    pub object_paths_redacted: bool,
    pub statistics_redacted: bool,
}
```

Policy:

```text id="kvuqhg"
PublicSafe:
  no raw SQL literals
  no paths
  no exact stats
  no samples

InternalNoSecrets:
  SQL text allowed after secret redaction
  paths hashed
  exact stats allowed only if not sensitive

InternalFull:
  full bundle, access-controlled

TestFixture:
  fixtures synthetic/non-sensitive
```

---

## 55.23 Error artifact

```rust id="vd75yi"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PlanningPhase {
    Parse,
    Bind,
    Analyze,
    LogicalOptimize,
    PhysicalPlan,
    PhysicalOptimize,
    Execute,
    Consume,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ErrorArtifact {
    pub phase: PlanningPhase,
    pub datafusion_error_class: String,
    pub message_redacted: String,
    pub sql_span: Option<String>,
    pub node_path: Option<Vec<String>>,
    pub expression_path: Option<Vec<String>>,
    pub table_ref: Option<String>,
    pub column_ref: Option<String>,
    pub remediation: Option<String>,
}
```

Error classification policy:

```text id="dgc3yn"
Parse error:
  syntax/dialect

Bind error:
  missing table/column/function, ambiguous identifier

Analyze error:
  type coercion, aggregate/window semantics

Logical optimize error:
  optimizer rule failure

Physical plan error:
  no physical implementation, extension unsupported

Execute error:
  object store, memory, UDF, runtime

Consume error:
  collect OOM, serializer, sink failure
```

---

## 55.24 Repro command artifact

```rust id="sm1ju4"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReproCommandArtifact {
    pub cargo_command: String,
    pub env: BTreeMap<String, String>,
    pub data_fixture_uri: Option<String>,
    pub sql_file: Option<String>,
    pub expected_artifact_hash: Option<String>,
}
```

Example:

```bash id="l5l3v7"
DATAFUSION_EXECUTION_TARGET_PARTITIONS=8 \
DATAFUSION_EXECUTION_BATCH_SIZE=8192 \
cargo test --release --test query_regression -- exact_query_qry_abc123
```

Repro policy:

```text id="twz4pf"
Include:
  exact command
  env vars
  fixture location/hash
  Cargo.lock hash
  expected snapshot hashes

Exclude:
  raw credentials
  production object paths unless authorized
```

---

## 55.25 Golden comparison modes

```rust id="gqzj9e"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ComparisonMode {
    ExactText,
    NormalizedText,
    OperatorClassSet,
    SchemaOnly,
    ResultRowsExact,
    ResultRowsSorted,
    MetricsThreshold,
    InformationalOnly,
}
```

Recommended mapping:

| Artifact                 | Normal mode                         | Upgrade mode               |
| ------------------------ | ----------------------------------- | -------------------------- |
| unoptimized logical plan | normalized text                     | informational + schema     |
| optimized logical plan   | normalized text                     | review diff                |
| physical plan            | exact text only when version-pinned | operator-class set         |
| output schema            | exact semantic                      | exact semantic             |
| result rows              | exact/sorted fixture                | exact/sorted fixture       |
| metrics                  | thresholds                          | thresholds + manual review |
| EXPLAIN ANALYZE          | thresholds                          | thresholds                 |

---

## 55.26 Best-practice deployment advisory

```text id="vthmfc"
Always capture:
  query_id
  input hash
  DataFusion version
  ConfigOptions hash
  table schema hashes
  optimized logical plan
  physical plan
  output schema
  warning/error class

For performance regression:
  EXPLAIN ANALYZE
  operator metrics
  table statistics
  object-store/file metadata
  runtime config

For correctness regression:
  output schema
  deterministic result snapshot
  row count
  logical plan
  type/nullability snapshot

For LLM-agent handoff:
  summary.md
  bundle.json
  plan tree
  schema artifacts
  targeted node details

For production:
  redact aggressively
  do not execute EXPLAIN ANALYZE unless safe
  do not persist raw result samples unless authorized
```

---

## 55.27 Anti-pattern inventory

* Saving only raw SQL and not config/catalog/schema.
* Snapshotting physical plan text across DataFusion versions without pinning.
* Treating EXPLAIN ANALYZE as plan inspection only; it executes the query.
* Logging object-store credentials in SQL/options.
* Storing production row samples in debug artifacts without redaction.
* Comparing exact runtime timings.
* Ignoring feature flags and Cargo.lock hash.
* Omitting table statistics and then debugging join/scan plan drift blindly.
* Omitting output schema and relying on result pretty table.
* Capturing physical plan after changing config from original request.
* Failing to distinguish unoptimized vs optimized logical plan.
* Treating Substrait/protobuf serialization as a complete stable cache format.
* Dumping full artifact to LLM context before summary/plan tree.
* Hiding error phase, making parse/bind/analyze/physical failures indistinguishable.

---

## 55.28 Agent checklist

```text id="qk17og"
[ ] Capture input:
    SQL / PlanSpec / QuerySpec
    parser dialect
    parameters
    normalized/hash form

[ ] Capture engine:
    DataFusion version
    Arrow version
    feature flags
    Cargo.lock hash
    build profile

[ ] Capture session:
    ConfigOptions
    target_partitions
    batch_size
    optimizer/analyzer rules
    function registry

[ ] Capture runtime:
    RuntimeEnv summary
    memory limit
    temp directory redacted
    object-store roots redacted
    cache summary

[ ] Capture catalog:
    referenced tables
    table schemas
    table stats
    constraints
    source metadata
    freshness hashes

[ ] Capture plans:
    unoptimized logical
    analyzed logical if available
    optimized logical
    initial physical if available
    optimized physical
    physical properties

[ ] Capture explain:
    EXPLAIN
    EXPLAIN VERBOSE
    EXPLAIN ANALYZE only under guardrails

[ ] Capture execution:
    metrics
    row counts
    output schema
    sample/result snapshot only when safe

[ ] Store formats:
    JSON canonical bundle
    Markdown summary
    compact text snapshots
    protobuf only for typed interchange

[ ] Golden policy:
    logical snapshots
    physical snapshots version-pinned
    result snapshots deterministic
    metrics thresholds
    schema exact checks

[ ] Reveal protocol:
    summary first
    plan tree next
    node details on demand
    metrics last

[ ] Redaction:
    SQL literals
    object-store paths
    credentials
    sensitive stats
    result samples
    error messages
```


# DataFusion Advanced — 56) Plan serialization, caching, fingerprints, and invalidation

## 56.0 Purpose

Extend plan serialization from “interoperability artifact” into an operational system for:

```text
parse avoidance
logical-planning cache
optimized-plan reuse
physical-plan debug snapshots
cross-process plan transport
distributed execution handoff
regression snapshots
LLM-agent reproducibility
safe invalidation
tenant-safe cache partitioning
```

DataFusion’s native `datafusion-proto` crate serializes/deserializes `LogicalPlan`s, including `Expr`, and `ExecutionPlan`s, including `PhysicalExpr`, to bytes via Protocol Buffers/prost. `datafusion-substrait` serializes/deserializes DataFusion `LogicalPlan` and `ExecutionPlan` values to/from generated `substrait::proto` types for Substrait interoperability. ([Docs.rs][1])

---

## 56.1 Serialization mental model

```text
Input artifact:
  SQL text | PlanSpec | DataFrame factory request

Planner-local artifacts:
  sqlparser AST
  Expr
  LogicalPlan
  optimized LogicalPlan
  ExecutionPlan
  PhysicalExpr

Serializable artifacts:
  native DataFusion proto:
    LogicalPlan + Expr
    ExecutionPlan + PhysicalExpr

  Substrait:
    interoperable relational plan representation
    strongest for cross-engine/cross-language intent exchange

Operational cache artifacts:
  input hash
  normalized AST hash
  logical plan hash
  optimized logical plan hash
  physical plan hash
  catalog/config/function/statistics fingerprints
```

`SessionState` contains planning and execution state such as configuration, functions, and runtime environment; therefore any reusable plan artifact is implicitly dependent on more than the query string alone. ([Docs.rs][2])

---

## 56.2 What can be serialized?

| Object                    |                                    Native proto |                                                           Substrait | Operational caveat                                                    |
| ------------------------- | ----------------------------------------------: | ------------------------------------------------------------------: | --------------------------------------------------------------------- |
| `Expr`                    | yes, as part of logical plan / expression proto |             yes where supported by Substrait function/type coverage | function registry and extension function semantics still external     |
| `LogicalPlan`             |                                             yes |                                                                 yes | catalog, schema, UDFs, config, extension codecs must match            |
| `ExecutionPlan`           |                                             yes | yes in crate support, but physical fidelity is more engine-specific | physical plans are most version/runtime-sensitive                     |
| `PhysicalExpr`            |     yes, as part of physical plan serialization |                                            partial/engine-dependent | physical function implementations and Arrow/DataFusion version matter |
| `DFSchema` / Arrow schema |             embedded/represented in plan protos |                 represented through Substrait types where supported | metadata/qualifiers/extension types need testing                      |
| custom logical nodes      |                     only with codecs/extensions |                     only if mapped to Substrait extension semantics | explicit versioned codec required                                     |
| custom physical nodes     |                     only with codecs/extensions |                     generally not portable unless extension-defined | strong invalidation required                                          |

The DataFusion Java proto docs describe generated protobuf classes for plan and expression nodes, scalar values, schemas, column references, and file formats, and note that builds use pinned upstream DataFusion `.proto` files from a matching tag; this illustrates the version-coupled nature of the native proto format. ([Apache DataFusion][3])

---

## 56.3 Native proto vs Substrait

### 56.3.1 Native DataFusion proto

Use for:

```text
DataFusion-to-DataFusion transport
same-version or tightly pinned-version cache
distributed DataFusion execution handoff
regression artifact storage
debug bundle binary payloads
logical/physical plan round-trip tests
```

Properties:

```text
high DataFusion fidelity
supports DataFusion-specific logical/physical constructs
implemented by converting plans to protobuf via prost
best when producer and consumer use compatible DataFusion/proto versions
not a stable semantic contract by itself
```

Native proto is the right choice when the consumer is another DataFusion process that you control and version-pin. `datafusion-proto` explicitly targets DataFusion plan serialization rather than cross-engine semantic portability. ([Docs.rs][1])

### 56.3.2 Substrait

Use for:

```text
cross-engine plan interchange
cross-language plan interchange
semantic interoperability experiments
portable relational-algebra exchange
Python/Rust/other-client handoff
```

Properties:

```text
interoperability-oriented
less DataFusion-specific
requires function/type extension mapping
not guaranteed to preserve every DataFusion-specific optimization
better for semantic exchange than engine-private physical cache
```

Substrait describes compute operations on structured data and is designed for interoperability across languages and systems; DataFusion provides producer/consumer support for converting DataFusion plans to and from Substrait plans. ([Substrait][4])

### 56.3.3 Decision table

| Goal                                         | Preferred format                                      |
| -------------------------------------------- | ----------------------------------------------------- |
| same DataFusion service logical-plan cache   | native proto or structured JSON metadata + plan text  |
| same DataFusion distributed physical handoff | native proto with strict version pin                  |
| cross-engine query intent                    | Substrait                                             |
| long-lived durable query definition          | SQL / PlanSpec + versioned environment snapshot       |
| golden test artifact                         | normalized text + schema JSON + optional native proto |
| LLM-agent handoff                            | markdown summary + JSON manifest + compact plan text  |
| security-sensitive cache                     | opaque key + redacted metadata + no raw user literals |

---

## 56.4 Why serialized plans are not durable contracts

A serialized plan is not self-sufficient unless all external semantics are pinned.

```text
Serialized LogicalPlan depends on:
  DataFusion version
  Arrow version
  datafusion-proto version
  SQL parser behavior if source SQL is reused
  ConfigOptions
  optimizer/analyzer rule set
  catalog/schema/table providers
  function registry
  UDF implementation versions
  table statistics
  constraints
  extension-node codecs
  object-store/table source semantics
  authorization policy

Serialized ExecutionPlan additionally depends on:
  physical operator implementations
  physical optimizer rule set
  RuntimeEnv configuration
  object-store registry
  memory/spill behavior
  custom ExecutionPlan codecs
  physical expression implementations
  target_partitions / batch_size
```

`ConfigOptions` can be constructed from environment variables using DataFusion’s dotted-key convention transformed to uppercase underscore variables, so an apparently identical SQL string can plan differently under a different environment/config snapshot. ([Docs.rs][5])

Durable storage rule:

```text
Durable query definition:
  SQL / PlanSpec
  parameter schema
  DataFusion version
  ConfigOptions hash
  catalog/schema snapshot
  function registry hash
  optimizer rule-set hash
  table statistics snapshot
  authorization policy version

Serialized plan:
  optional acceleration/debug artifact
  never the only source of truth
```

---

## 56.5 Cache layers

```text
Layer 0 — input normalization cache:
  raw SQL / PlanSpec → normalized SQL / canonical PlanSpec

Layer 1 — parse cache:
  normalized SQL + dialect → sqlparser AST

Layer 2 — bind/logical cache:
  AST/PlanSpec + catalog/function/schema fingerprints → unoptimized LogicalPlan

Layer 3 — optimized logical cache:
  LogicalPlan + ConfigOptions + optimizer rules + stats/constraints → optimized LogicalPlan

Layer 4 — physical plan cache:
  optimized LogicalPlan + physical config + provider/runtime metadata → ExecutionPlan

Layer 5 — scan metadata cache:
  object-store listing / file metadata / Parquet footer / statistics

Layer 6 — result cache:
  query + catalog/data snapshot + parameters + auth policy → RecordBatch/file result
```

Caching gets more fragile as it moves downward:

```text
SQL text cache:
  easiest to invalidate

logical plan cache:
  catalog/function/config-sensitive

optimized logical plan cache:
  optimizer/statistics-sensitive

physical plan cache:
  highly version/config/runtime/provider-sensitive

result cache:
  data snapshot/auth-tenant-sensitive
```

---

## 56.6 Plan cache key design

### 56.6.1 Top-level key

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanCacheKeyV1 {
    pub key_schema_version: String,

    pub engine: EngineFingerprint,
    pub input: InputFingerprint,
    pub parameters: ParameterFingerprint,
    pub config: ConfigFingerprint,
    pub catalog: CatalogFingerprint,
    pub functions: FunctionRegistryFingerprint,
    pub optimizer: OptimizerFingerprint,
    pub data: DataFreshnessFingerprint,
    pub security: SecurityFingerprint,
    pub target: CacheTarget,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CacheTarget {
    ParsedAst,
    UnoptimizedLogicalPlan,
    OptimizedLogicalPlan,
    PhysicalPlan,
    ExplainText,
    Result,
}
```

### 56.6.2 Engine fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EngineFingerprint {
    pub datafusion_version: String,
    pub arrow_version: String,
    pub datafusion_proto_version: Option<String>,
    pub substrait_version: Option<String>,
    pub object_store_version: Option<String>,
    pub feature_flags_hash: String,
    pub cargo_lock_hash: Option<String>,
    pub build_profile: Option<String>,
}
```

### 56.6.3 Input fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputFingerprint {
    pub raw_sql_hash: Option<String>,
    pub normalized_sql_hash: Option<String>,
    pub normalized_ast_hash: Option<String>,
    pub planspec_hash: Option<String>,
    pub planspec_schema_version: Option<String>,
    pub parser_dialect: Option<String>,
}
```

DataFusion config docs identify `datafusion.sql_parser.dialect` as a parser dialect setting with supported values including Generic, MySQL, PostgreSQL, Hive, SQLite, Snowflake, Redshift, MsSQL, ClickHouse, BigQuery, Ansi, DuckDB, and Databricks; parser dialect belongs in the cache key. ([Apache DataFusion][6])

### 56.6.4 Config fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigFingerprint {
    pub config_options_hash: String,
    pub batch_size: usize,
    pub target_partitions: usize,
    pub timezone: Option<String>,
    pub sql_parser_dialect: Option<String>,
    pub optimizer_options_hash: String,
    pub parquet_options_hash: String,
    pub runtime_options_hash: String,
}
```

### 56.6.5 Catalog/schema fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CatalogFingerprint {
    pub default_catalog: Option<String>,
    pub default_schema: Option<String>,
    pub catalog_version: Option<String>,
    pub referenced_tables: Vec<TableFingerprint>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TableFingerprint {
    pub table_ref: String,
    pub provider_type: String,
    pub schema_hash: String,
    pub constraints_hash: Option<String>,
    pub statistics_hash: Option<String>,
    pub table_version: Option<String>,
    pub source_snapshot_id: Option<String>,
}
```

### 56.6.6 Function registry fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionRegistryFingerprint {
    pub scalar_functions_hash: String,
    pub aggregate_functions_hash: String,
    pub window_functions_hash: String,
    pub table_functions_hash: String,
    pub udf_versions: Vec<UdfFingerprint>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UdfFingerprint {
    pub name: String,
    pub kind: String,
    pub signature_hash: String,
    pub implementation_version: String,
    pub volatility: Option<String>,
}
```

### 56.6.7 Optimizer fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OptimizerFingerprint {
    pub analyzer_rules_hash: String,
    pub logical_optimizer_rules_hash: String,
    pub physical_optimizer_rules_hash: String,
    pub query_planner_id: String,
    pub physical_planner_id: String,
}
```

### 56.6.8 Data freshness fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataFreshnessFingerprint {
    pub table_statistics_version_hash: String,
    pub object_store_listing_hash: Option<String>,
    pub file_metadata_hash: Option<String>,
    pub parquet_footer_hash: Option<String>,
    pub source_snapshot_ids: Vec<String>,
    pub delta_or_lakehouse_snapshot_id: Option<String>,
}
```

### 56.6.9 Security fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityFingerprint {
    pub tenant_id_hash: Option<String>,
    pub role_set_hash: String,
    pub authorization_policy_version: String,
    pub table_allowlist_hash: Option<String>,
    pub column_allowlist_hash: Option<String>,
    pub function_allowlist_hash: Option<String>,
    pub row_policy_hash: Option<String>,
}
```

Security rule:

```text
Authorization policy is part of the cache key.
A plan valid for tenant A must not be reused for tenant B unless the policy snapshot is provably identical and source credentials are isolated.
```

---

## 56.7 Fingerprint canonicalization

### 56.7.1 Canonical JSON hashing

```rust
use sha2::{Digest, Sha256};

pub fn sha256_json<T: serde::Serialize>(value: &T) -> anyhow::Result<String> {
    // Use canonical serialization in production:
    // - deterministic map ordering
    // - no pretty whitespace
    // - stable enum tags
    let bytes = serde_json::to_vec(value)?;
    let hash = Sha256::digest(&bytes);
    Ok(format!("{hash:x}"))
}
```

For deterministic hashes:

```text
Use BTreeMap/BTreeSet, not HashMap/HashSet.
Sort arrays where order is semantically irrelevant.
Preserve arrays where order is semantic.
Normalize paths/literals according to redaction policy.
Version the key schema.
Include DataFusion version and feature flags.
```

### 56.7.2 Plan text hashing

```rust
pub fn hash_plan_text(plan_text: &str) -> String {
    let normalized = plan_text
        .lines()
        .map(normalize_plan_line)
        .collect::<Vec<_>>()
        .join("\n");

    let hash = sha2::Sha256::digest(normalized.as_bytes());
    format!("{hash:x}")
}

fn normalize_plan_line(line: &str) -> String {
    line
        .replace("/tmp/", "<tmp>/")
        // add regex-based redaction for paths/query ids/timings in production
}
```

Use plan-text hash for:

```text
debug comparison
golden snapshots
human-visible regression review
```

Do not use plan-text hash as the only cache key for executable reuse.

---

## 56.8 Parameterized plans

### 56.8.1 SQL placeholders

```rust
use datafusion::common::ScalarValue;
use datafusion::prelude::*;

let df = ctx
    .sql("SELECT * FROM orders WHERE customer_id = $1 AND amount >= $2")
    .await?
    .with_param_values(vec![
        ScalarValue::Int64(Some(123)),
        ScalarValue::Float64(Some(100.0)),
    ])?;
```

`DataFrame::with_param_values` replaces parameters in a logical plan with supplied values; the DataFrame docs expose this method on the lazy plan wrapper. ([Docs.rs][7])

### 56.8.2 Named parameters

```rust
let df = ctx
    .sql("SELECT * FROM orders WHERE customer_id = $customer_id")
    .await?
    .with_param_values(vec![
        ("customer_id", ScalarValue::Int64(Some(123))),
    ])?;
```

Named-parameter support has been discussed around the same `with_param_values` API; use the exact pinned DataFusion version’s accepted parameter input shape in production. ([GitHub][8])

### 56.8.3 Parameter signature fingerprint

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterFingerprint {
    pub parameter_style: ParameterStyle,
    pub type_signature_hash: String,
    pub value_hash_policy: ValueHashPolicy,
    pub bound_value_hash: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ParameterStyle {
    None,
    Positional,
    Named,
    PlanSpecTyped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ValueHashPolicy {
    DoNotHashValues,
    HashValuesForResultCache,
    HashRedactedValues,
}
```

### 56.8.4 Cache layers for parameterized plans

| Cache target             |                                                       Include parameter values? |                  Include parameter types? |
| ------------------------ | ------------------------------------------------------------------------------: | ----------------------------------------: |
| parsed AST               |                                                                              no |                                        no |
| unbound logical template |                                                                              no | yes if placeholders typed during planning |
| optimized logical plan   |                         often no, unless optimization depends on literal values |                                       yes |
| physical plan            | maybe, if literal-specific pruning/top-k/constant folding changes physical plan |                                       yes |
| result cache             |                                                                             yes |                                       yes |

Parameter rule:

```text
Plan cache:
  parameter type signature usually belongs in key.

Result cache:
  parameter value hash belongs in key.

Security:
  raw parameter values should rarely be stored.
```

---

## 56.9 Cache value schema

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanCacheEntryV1 {
    pub key: PlanCacheKeyV1,
    pub created_at_utc: String,
    pub expires_at_utc: Option<String>,
    pub producer: CacheProducer,
    pub value: CachedPlanValue,
    pub validation: CacheValidation,
    pub audit: CacheAudit,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CachedPlanValue {
    NormalizedSql {
        normalized_sql: String,
    },
    UnoptimizedLogicalPlanProto {
        bytes_b64: String,
        display_indent_schema: String,
    },
    OptimizedLogicalPlanProto {
        bytes_b64: String,
        display_indent_schema: String,
    },
    PhysicalPlanProto {
        bytes_b64: String,
        display_indent: String,
    },
    SubstraitPlan {
        bytes_b64: String,
        substrait_version: String,
    },
    ExplainText {
        text: String,
    },
}
```

Validation block:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheValidation {
    pub validated_against_current_catalog: bool,
    pub schema_hashes_checked: bool,
    pub function_registry_checked: bool,
    pub authorization_checked: bool,
    pub data_snapshot_checked: bool,
}
```

---

## 56.10 Native proto encode/decode sketch

Exact module APIs should be checked against the pinned `datafusion-proto` version, but the operational pattern is:

```rust
// Pseudocode: adapt to pinned datafusion-proto API.
use datafusion::logical_expr::LogicalPlan;

pub fn encode_logical_plan(plan: &LogicalPlan) -> datafusion::common::Result<Vec<u8>> {
    // datafusion_proto::logical_plan::to_proto::...
    // prost::Message::encode_to_vec(...)
    todo!("encode LogicalPlan using datafusion-proto")
}

pub fn decode_logical_plan(bytes: &[u8]) -> datafusion::common::Result<LogicalPlan> {
    // prost::Message::decode(...)
    // datafusion_proto::logical_plan::from_proto::...
    todo!("decode LogicalPlan using datafusion-proto")
}
```

`datafusion_proto::logical_plan::to_proto` is documented as code that converts Arrow schemas and DataFusion logical plans to protocol-buffer format so logical plans can be serialized and transmitted between processes. ([Docs.rs][9])

Operational wrapper:

```rust
pub struct EncodedPlanArtifact {
    pub format: PlanSerializationFormat,
    pub bytes_b64: String,
    pub datafusion_version: String,
    pub proto_version: String,
    pub codec_registry_hash: String,
    pub plan_text_hash: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PlanSerializationFormat {
    DataFusionProtoLogical,
    DataFusionProtoPhysical,
    Substrait,
}
```

---

## 56.11 Substrait encode/decode sketch

```rust
// Pseudocode: adapt to pinned datafusion-substrait API.
pub fn logical_to_substrait_bytes(
    plan: &datafusion::logical_expr::LogicalPlan,
) -> datafusion::common::Result<Vec<u8>> {
    // datafusion_substrait producer path
    todo!("produce Substrait protobuf bytes")
}

pub fn substrait_bytes_to_logical(
    bytes: &[u8],
) -> datafusion::common::Result<datafusion::logical_expr::LogicalPlan> {
    // datafusion_substrait consumer path
    todo!("consume Substrait protobuf bytes")
}
```

Use Substrait when:

```text
consumer is not guaranteed to be the same DataFusion version
plan is a semantic interchange artifact
cross-language/cross-engine integration matters
engine-specific physical details are less important
```

Avoid Substrait for:

```text
exact DataFusion physical plan cache
custom physical operator transport without extension mapping
long-lived cache that assumes all functions/types/operators round-trip identically
```

---

## 56.12 Invalidation triggers

### 56.12.1 Schema change

Invalidate when:

```text
column added/dropped/renamed
type changed
nullability changed
metadata changed if used
field order changed when order-sensitive
qualifier/view output changed
```

Action:

```text
invalidate:
  logical plan cache
  optimized logical cache
  physical plan cache
  result cache

maybe keep:
  parsed AST cache
  normalized SQL cache
```

### 56.12.2 Function change

Invalidate when:

```text
UDF registered/unregistered
UDF signature changed
return type changed
volatility changed
implementation changed
aggregate/window state semantics changed
function package upgraded
```

Action:

```text
invalidate:
  bound logical plans using function
  optimized logical plans
  physical plans
  result cache
```

### 56.12.3 Config change

Invalidate when:

```text
SQL dialect changes
batch_size changes
target_partitions changes
optimizer flags change
parquet pruning/filter options change
runtime memory/spill options change
timezone changes
string type mapping changes
```

Action:

```text
parser dialect:
  parse cache and downstream invalid

optimizer config:
  optimized logical and physical invalid

execution config:
  physical and result/runtime metrics invalid

timezone:
  plans involving temporal literals/functions invalid
```

### 56.12.4 Optimizer version/rule change

Invalidate when:

```text
DataFusion version changes
analyzer rule order changes
optimizer rule set changes
physical optimizer rule set changes
custom rule implementation changes
query planner changes
physical planner changes
```

Action:

```text
logical optimized plans invalid
physical plans invalid
golden snapshots require review
```

### 56.12.5 Statistics refresh

Invalidate depending on cache target:

```text
parse cache:
  no invalidation

unoptimized logical plan:
  usually no invalidation

optimized logical plan:
  invalidate if optimizer uses stats/costs

physical plan:
  invalidate if join/scan/sort choices depend on stats

result cache:
  invalidate only if data snapshot changed, not merely stats refresh
```

### 56.12.6 Object-store listing change

Invalidate when:

```text
files added/removed
file size/etag/version changed
partition directories changed
Parquet footer metadata changed
Delta/Iceberg snapshot changed
object-store listing cache expired/refreshed
```

Action:

```text
scan metadata cache invalid
physical scan plan maybe invalid
result cache invalid if data changed
statistics cache maybe invalid
```

---

## 56.13 Invalidation matrix

| Change                  | Parse cache |             Bound logical | Optimized logical |                      Physical plan |                 Result |
| ----------------------- | ----------: | ------------------------: | ----------------: | ---------------------------------: | ---------------------: |
| SQL whitespace/comment  |       maybe |     no if normalized same |                no |                                 no |                     no |
| parser dialect          |  invalidate |                invalidate |        invalidate |                         invalidate |             invalidate |
| table schema            |        keep |                invalidate |        invalidate |                         invalidate |             invalidate |
| UDF implementation      |        keep |                     maybe |        invalidate |                         invalidate |             invalidate |
| optimizer flags         |        keep |                      keep |        invalidate |                         invalidate |                  maybe |
| target partitions       |        keep |                      keep |             maybe |                         invalidate |   no if semantics same |
| batch size              |        keep |                      keep |              keep | invalidate metrics, maybe physical |   no if semantics same |
| stats refresh           |        keep |                      keep |  maybe invalidate |                   maybe invalidate | no unless data changed |
| object listing changed  |        keep |                      keep |             maybe |               invalidate scan plan |             invalidate |
| auth policy change      |       maybe |                invalidate |        invalidate |                         invalidate |    invalidate/security |
| DataFusion version      |       maybe |                invalidate |        invalidate |                         invalidate |      review/invalidate |
| extension codec version |        keep | invalidate affected nodes |        invalidate |                         invalidate |             invalidate |

---

## 56.14 Cache validation before reuse

```rust
pub fn validate_cache_entry(
    entry: &PlanCacheEntryV1,
    current: &PlanCacheKeyV1,
) -> CacheDecision {
    if entry.key.key_schema_version != current.key_schema_version {
        return CacheDecision::Miss("cache key schema changed".into());
    }

    if entry.key.engine != current.engine {
        return CacheDecision::Miss("engine fingerprint changed".into());
    }

    if entry.key.config != current.config {
        return CacheDecision::Miss("config fingerprint changed".into());
    }

    if entry.key.catalog != current.catalog {
        return CacheDecision::Miss("catalog fingerprint changed".into());
    }

    if entry.key.functions != current.functions {
        return CacheDecision::Miss("function registry changed".into());
    }

    if entry.key.security != current.security {
        return CacheDecision::Miss("security fingerprint changed".into());
    }

    CacheDecision::Hit
}

#[derive(Debug, Clone)]
pub enum CacheDecision {
    Hit,
    Miss(String),
    Stale(String),
    Denied(String),
}
```

Rule:

```text
Never deserialize and execute a cached physical plan before validating the cache key against current environment/security/catalog state.
```

---

## 56.15 Plan cache poisoning

Threat:

```text
attacker causes service to cache a plan under a key later reused by another query/tenant/user
```

Vectors:

```text
weak key:
  raw SQL hash only

missing auth:
  tenant/user/role absent from key

missing schema:
  table name same but schema changed

missing function version:
  UDF behavior changed

extension node:
  malicious serialized extension payload decoded by trusted engine

object-store paths:
  direct path from one tenant reused in another context
```

Mitigation:

```text
include tenant/auth policy fingerprint
include catalog/schema/function/config/version fingerprints
verify decoded plan tables/functions against current policy
disable serialized extension nodes from untrusted cache
namespace cache by tenant/security domain
sign cache entries if crossing trust boundary
```

---

## 56.16 Tenant leakage

Bad cache key:

```text
hash(sql)
```

Good cache key:

```text
hash(
  normalized_sql,
  tenant_policy_version,
  tenant_role_set,
  table_allowlist,
  row_policy,
  catalog/schema hashes,
  function registry,
  data snapshot
)
```

Tenant-safe policy:

```text
Plan cache:
  tenant namespace required unless all tenants share identical catalog/auth/data snapshot.

Result cache:
  tenant namespace mandatory.

Serialized physical plan:
  tenant namespace + object-store credential scope mandatory.
```

---

## 56.17 Stale authorization

Problem:

```text
User had access to table A yesterday.
Plan was cached.
User lost access today.
Cached plan still scans table A if auth not rechecked.
```

Mitigation:

```text
authorization policy version in key
role-set hash in key
table/column/function allowlist hash in key
validate plan references before execution
short TTL for user-specific plan caches
invalidate on ACL updates
```

Validation pass:

```rust
pub fn validate_cached_plan_authorization(
    plan: &datafusion::logical_expr::LogicalPlan,
    policy: &SecurityFingerprint,
) -> datafusion::common::Result<()> {
    // Traverse TableScan, Expr column refs, function refs, Extension nodes.
    // Compare against current policy, not only cached policy.
    Ok(())
}
```

---

## 56.18 Serialized extension nodes

Risk:

```text
LogicalPlan::Extension or custom ExecutionPlan serialization can encode engine-private behavior.
Decoding custom nodes requires registered codecs and trusted implementation identity.
```

Required metadata:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExtensionCodecFingerprint {
    pub extension_name: String,
    pub extension_version: String,
    pub codec_version: String,
    pub implementation_hash: Option<String>,
    pub allow_deserialize_from_cache: bool,
}
```

Policy:

```text
Untrusted cache:
  reject extension nodes.

Trusted internal cache:
  require extension codec fingerprint match.

Cross-version cache:
  reject unless extension declares compatibility.

Substrait:
  require extension function/type URI mapping and version.
```

---

## 56.19 Storage guidance

### 56.19.1 Durable query definition store

Store:

```text
SQL / PlanSpec
parameter schema
normalized input hash
DataFusion version range
required feature flags
config profile id
catalog snapshot id
function registry version
authorization policy version
expected output schema
test fixtures / result baselines
```

Do not rely on:

```text
serialized physical plan alone
serialized optimized logical plan alone
plan text alone
```

### 56.19.2 Short-lived operational cache

May store:

```text
parsed AST
unoptimized logical proto
optimized logical proto
physical proto
scan metadata
statistics
```

Requirements:

```text
strict key
TTL
validation-before-use
tenant namespace
codec version
redaction/signing if persisted
```

### 56.19.3 Result cache

Store:

```text
result file / Arrow IPC / Parquet artifact
output schema
row count
query key
data snapshot id
tenant/auth key
parameter value hash
expiration
```

Invalidate on:

```text
data snapshot change
authorization change
function semantics change
query/config semantics change
```

---

## 56.20 Cache implementation blueprint

```rust
#[async_trait::async_trait]
pub trait PlanCache {
    async fn get(&self, key: &PlanCacheKeyV1) -> anyhow::Result<Option<PlanCacheEntryV1>>;
    async fn put(&self, entry: PlanCacheEntryV1) -> anyhow::Result<()>;
    async fn invalidate_by_table(&self, table_ref: &str) -> anyhow::Result<()>;
    async fn invalidate_by_function(&self, function_name: &str) -> anyhow::Result<()>;
    async fn invalidate_by_policy_version(&self, policy_version: &str) -> anyhow::Result<()>;
}
```

Read path:

```rust
pub async fn get_or_plan(
    cache: &dyn PlanCache,
    ctx: &SessionContext,
    sql: &str,
    target: CacheTarget,
) -> datafusion::common::Result<datafusion::logical_expr::LogicalPlan> {
    let current_key = build_cache_key(ctx, sql, target).await?;

    if let Some(entry) = cache.get(&current_key).await.map_err(to_df_err)? {
        match validate_cache_entry(&entry, &current_key) {
            CacheDecision::Hit => {
                return decode_cached_logical_plan(entry);
            }
            CacheDecision::Miss(reason) | CacheDecision::Stale(reason) | CacheDecision::Denied(reason) => {
                tracing::debug!(%reason, "plan cache miss");
            }
        }
    }

    let plan = ctx.state().create_logical_plan(sql).await?;
    let entry = encode_plan_cache_entry(current_key, &plan)?;
    cache.put(entry).await.map_err(to_df_err)?;

    Ok(plan)
}
```

Operational rule:

```text
Cache miss must fall back to planning.
Cache hit must validate before decode/execute.
Cache decode failure must be treated as miss plus metric, not silent corruption.
```

---

## 56.21 Fingerprint construction pipeline

```text
1. Normalize input:
   SQL dialect, SQL text, parameters, PlanSpec

2. Build input fingerprint:
   raw SQL hash
   normalized SQL/AST hash
   parameter type signature

3. Snapshot engine:
   DataFusion/Arrow/proto/Substrait versions
   feature flags

4. Snapshot config:
   ConfigOptions canonical JSON/hash
   optimizer options
   runtime-sensitive options

5. Resolve catalog references:
   tables
   schemas
   provider types
   table versions/snapshot ids

6. Snapshot functions:
   scalar/UDAF/window/table functions
   UDF implementation versions
   volatility/signature hashes

7. Snapshot data freshness:
   stats version
   object listing version
   source snapshot id

8. Snapshot security:
   tenant/role/policy/table-column-function allowlists

9. Assemble key:
   stable serialization
   SHA-256
```

---

## 56.22 Normalized AST hash

Use when SQL text differs but parses to same structure.

Examples:

```sql
SELECT * FROM t WHERE a = 1
```

```sql
select * from t where a=1
```

Potential same normalized AST.

Caveats:

```text
comments may carry optimizer hints if supported
quoted identifiers preserve case
dialect changes AST
literal values may or may not be parameterized
commutative expression normalization must be semantics-aware
```

Recommended normalized AST artifact:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NormalizedAstArtifact {
    pub dialect: String,
    pub statement_class: String,
    pub canonical_debug: String,
    pub canonical_hash: String,
    pub normalization_rules: Vec<String>,
}
```

---

## 56.23 Logical plan hash

Use for:

```text
compiler output regression
logical-cache identity
LLM-agent plan equivalence review
```

Do not use alone for:

```text
authorization
physical-plan reuse
result-cache reuse
```

Hash inputs:

```text
LogicalPlan display/debug/proto bytes
DFSchema field names/types/nullability/qualifiers
extension node names/versions
function names/signatures
```

Rule:

```text
Hash stable canonical representation when possible.
Display strings are useful but version-sensitive.
Proto bytes are compact but version/codec-sensitive.
```

---

## 56.24 Physical plan hash

Use for:

```text
physical regression under exact version pin
operator topology comparison
cache diagnostics
```

High invalidation sensitivity:

```text
DataFusion version
physical optimizer rules
target_partitions
batch_size
memory/spill config
source statistics/order/partition metadata
provider implementation
object-store file groups
```

Physical plan cache policy:

```text
Allowed:
  short-lived same-process/same-version cache
  controlled distributed execution handoff
  debug artifact

Dangerous:
  long-lived persistent executable cache
  cross-tenant cache
  cross-version cache
  cache involving custom extension nodes without codec version
```

---

## 56.25 Prepared-plan / parameter template policy

```text
Prepared logical template:
  SQL with placeholders
  parameter type signature
  table/function/schema fingerprint
  optimizer config

Bind step:
  validate parameter count/names/types
  bind values
  optionally re-optimize if literal values influence optimization

Execution:
  physical plan may depend on values if:
    partition pruning
    statistics pruning
    constant folding
    limit/top-k
    dynamic filters
```

Prepared cache matrix:

| Query                                        | Cache plan without values? |                                Re-plan after binding? |
| -------------------------------------------- | -------------------------: | ----------------------------------------------------: |
| `WHERE id = $1` on indexed/partitioned table |                      maybe |      often yes for pruning/value-specific source plan |
| `WHERE amount > $1` with Parquet stats       |                      maybe | yes if row-group pruning/source planning uses literal |
| `LIMIT $1`                                   |                 no/limited |                  yes or include value in physical key |
| `SELECT $1 + 1`                              |            type cache only |                        value affects constant folding |
| UDF with parameter                           |     depends UDF volatility |                                usually bind then plan |

---

## 56.26 Security checklist for cache deployment

```text
[ ] Namespace by tenant/security domain.
[ ] Include authorization policy version in key.
[ ] Include role-set hash in key.
[ ] Include table/column/function allowlist hashes.
[ ] Validate cached plan references before execution.
[ ] Reject extension nodes from untrusted serialized plans.
[ ] Sign cache entries crossing process/trust boundary.
[ ] Encrypt cache at rest if SQL/schema/stats sensitive.
[ ] Redact literals and object-store paths in diagnostics.
[ ] Invalidate on ACL/policy changes.
[ ] Log cache hits with query_id/key prefix, not raw SQL.
```

---

## 56.27 Observability

Metrics:

```text
plan_cache_lookup_total{target}
plan_cache_hit_total{target}
plan_cache_miss_total{target,reason}
plan_cache_stale_total{target,reason}
plan_cache_decode_error_total{format}
plan_cache_validation_error_total{reason}
plan_cache_entry_bytes{format}
plan_cache_plan_time_saved_ms
plan_cache_replan_time_ms
plan_cache_tenant_hit_ratio
```

Audit event:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanCacheAuditEvent {
    pub query_id: String,
    pub cache_target: CacheTarget,
    pub decision: String,
    pub reason: Option<String>,
    pub key_prefix: String,
    pub tenant_hash: Option<String>,
    pub datafusion_version: String,
    pub catalog_fingerprint_prefix: String,
    pub config_fingerprint_prefix: String,
}
```

---

## 56.28 Testing matrix

```text
Serialization:
  [ ] Expr round-trip
  [ ] LogicalPlan round-trip
  [ ] ExecutionPlan round-trip where supported
  [ ] PhysicalExpr round-trip where supported
  [ ] custom logical extension encode/decode
  [ ] custom physical extension rejected or round-tripped under codec

Substrait:
  [ ] logical plan producer/consumer
  [ ] function extension mapping
  [ ] type mapping
  [ ] unsupported operator error
  [ ] output schema preservation

Cache keys:
  [ ] SQL whitespace does not change normalized key
  [ ] dialect change invalidates
  [ ] schema change invalidates
  [ ] UDF version change invalidates
  [ ] ConfigOptions change invalidates
  [ ] optimizer-rule hash change invalidates
  [ ] stats refresh invalidates optimized/physical if policy says so
  [ ] object listing change invalidates scan/result
  [ ] tenant policy change invalidates

Security:
  [ ] tenant A cannot hit tenant B cache entry
  [ ] stale role loses access despite cached plan
  [ ] extension node rejected from untrusted cache
  [ ] poisoned cache entry signature failure rejected

Prepared plans:
  [ ] parameter type mismatch rejected
  [ ] positional parameter count mismatch rejected
  [ ] named parameter missing rejected
  [ ] result cache includes value hash
```

---

## 56.29 Deployment advisory

```text
Plan cache maturity levels:

Level 0:
  no executable cache
  store SQL + artifact bundle only

Level 1:
  normalized SQL / parse cache
  low risk

Level 2:
  unoptimized logical proto cache
  validate schema/function/auth before reuse

Level 3:
  optimized logical proto cache
  include config/optimizer/stats/constraints/version fingerprints

Level 4:
  physical proto cache
  same-version, same-runtime profile, short TTL, strict validation

Level 5:
  result cache
  requires data snapshot + parameter values + authorization fingerprint
```

Recommended production default:

```text
Start:
  artifact bundle + normalized SQL cache + scan metadata cache

Add:
  optimized logical cache for repeated machine-generated PlanSpec templates

Be cautious:
  physical plan cache only for controlled internal workloads

Avoid:
  long-lived cross-version physical cache
  cross-tenant result cache without strong isolation
```

---

## 56.30 Anti-pattern inventory

* Cache key = raw SQL hash only.
* Reusing cached plan after schema change.
* Reusing cached plan after authorization change.
* Reusing physical plan across DataFusion versions.
* Treating Substrait as full-fidelity DataFusion physical serialization.
* Treating native proto bytes as durable forever.
* Ignoring function registry/UDF version in cache key.
* Ignoring `ConfigOptions` in cache key.
* Ignoring table statistics when caching optimized/physical plans.
* Ignoring object-store listing/file metadata in scan/result caches.
* Caching serialized extension nodes from untrusted input.
* Storing raw SQL parameters with secrets in cache metadata.
* Using plan text hash as executable cache proof.
* Failing open on decode failure.
* Not distinguishing plan cache from result cache.
* Letting tenant A and tenant B share cache namespace.

---

## 56.31 Agent checklist

```text
[ ] Choose serialization:
    native proto for DataFusion fidelity
    Substrait for interoperability
    SQL/PlanSpec for durable definition

[ ] Identify cache target:
    parse
    unoptimized logical
    optimized logical
    physical
    explain
    result

[ ] Build key:
    SQL text hash
    normalized AST hash
    logical plan hash
    ConfigOptions hash
    catalog/schema version
    function registry version
    table statistics version
    DataFusion version
    Arrow version
    feature flags
    optimizer rule hash
    security/tenant policy hash

[ ] Parameterization:
    placeholder style
    parameter type signature
    bind values safely
    include value hash only for result/value-dependent cache

[ ] Invalidate on:
    schema change
    function change
    config change
    optimizer/rule change
    stats refresh
    object-store listing change
    data snapshot change
    authorization policy change
    extension codec change

[ ] Security:
    tenant namespace
    auth validation before execution
    reject untrusted extensions
    sign/encrypt cache if crossing trust boundary
    redact literals/paths

[ ] Store:
    durable SQL/PlanSpec + config/catalog/function/version snapshot
    serialized plan only as acceleration/debug artifact
    physical plan only with strict version/runtime validation

[ ] Test:
    round-trip
    cache hit/miss correctness
    invalidation correctness
    tenant isolation
    stale auth rejection
    extension-node rejection
```

[1]: https://docs.rs/datafusion-proto?utm_source=chatgpt.com "datafusion_proto - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/session_state/struct.SessionState.html?utm_source=chatgpt.com "SessionState in datafusion::execution::session_state - Rust"
[3]: https://datafusion.apache.org/java/user-guide/proto-plans.html?utm_source=chatgpt.com "Logical plans via datafusion-proto"
[4]: https://substrait.io/?utm_source=chatgpt.com "Home - Substrait: Cross-Language Serialization for Relational ..."
[5]: https://docs.rs/datafusion/latest/datafusion/common/config/struct.ConfigOptions.html?utm_source=chatgpt.com "ConfigOptions in datafusion::common::config - Rust"
[6]: https://datafusion.apache.org/user-guide/configs.html?utm_source=chatgpt.com "Configuration Settings — Apache DataFusion documentation"
[7]: https://docs.rs/datafusion/latest/datafusion/dataframe/struct.DataFrame.html?utm_source=chatgpt.com "DataFrame in datafusion"
[8]: https://github.com/apache/datafusion/issues/8245?utm_source=chatgpt.com "Support named query parameters · Issue #8245"
[9]: https://docs.rs/datafusion-proto/latest/datafusion_proto/logical_plan/to_proto/index.html?utm_source=chatgpt.com "datafusion_proto::logical_plan::to_proto - Rust"

## 56.32 DataFusion 54 decode contexts and transport coverage

### 56.32.1 Context-carrying decode APIs

DataFusion 54 threads an execution context through every proto decode path so UDFs and runtime state resolve during deserialization:

```rust id="dctx54"
// datafusion-proto bytes/mod.rs — Serializeable trait:
fn from_bytes_with_ctx(bytes: &[u8], ctx: &TaskContext) -> Result<Self>;   // required
fn from_bytes(bytes: &[u8]) -> Result<Self> {                              // default
    Self::from_bytes_with_ctx(bytes, &TaskContext::default())
    // errors if the serialized bytes reference user-defined functions
}

// Expr decode with functions available:
let expr = Expr::from_bytes_with_ctx(&bytes, ctx.task_ctx().as_ref())?;

// logical from_proto helpers now take the context:
pub fn parse_expr(proto: &LogicalExprNode, ctx: &TaskContext,
                  codec: &dyn LogicalExtensionCodec) -> Result<Expr, Error>;
// parse_exprs / parse_sorts follow the same pattern
```

Callers that used bare `Expr::from_bytes(&bytes)` under 53 must migrate to `from_bytes_with_ctx` whenever the payload can reference registered functions.

### 56.32.2 `PhysicalPlanDecodeContext`

Physical decode bundles its state into one struct (`datafusion-proto/src/physical_plan/mod.rs`):

```rust id="dctx54b"
pub struct PhysicalPlanDecodeContext<'a> {
    // task_ctx: &'a TaskContext          — accessor task_ctx()
    // codec: &'a dyn PhysicalExtensionCodec — accessor codec()
    // scalar_subquery_results: Option<ScalarSubqueryResults>
}
let decode_ctx = PhysicalPlanDecodeContext::new(task_ctx, &codec);
// physical_plan_from_bytes(bytes, ctx) / parse_physical_sort_expr(s)(..., &decode_ctx, ...)
```

`PhysicalProtoConverterExtension` hooks receive `&PhysicalPlanDecodeContext<'_>` for recursive plan/expression deserialization instead of separate registry+codec arguments — custom converters written for 53 need their signatures updated. The context also scopes the shared `ScalarSubqueryResults` container so a decoded `ScalarSubqueryExpr` reconnects to its decoded `ScalarSubqueryExec` (49.30).

### 56.32.3 Transport coverage duties

Plans serialized under 54 can carry constructs that 53-era codecs, allowlists, and cache validators have never seen:

```text id="dctx54c"
[ ] ScalarSubqueryExec nodes + ScalarSubqueryExpr expressions (49.30)
[ ] lambda expressions: Expr::Lambda / LambdaVariable / HigherOrderFunction (45.21)
[ ] field-aware casts: logical Cast { field } / physical CastExpr { target_field } (45.22)
[ ] cache keys: 53↔54 plan bytes are not interchangeable — version-partition
    the cache (56.6) and treat the decode-context requirement as part of the
    codec version
```

Primary proto/serialization coverage: `datafusion_rust.md` (engine deep-dive, plan-serialization section).


