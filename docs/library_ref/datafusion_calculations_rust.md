## Executive synthesis

The attached DataFusion Rust documentation already gives a strong foundation for user-defined calculations. It explicitly covers the UDF family map—scalar UDFs, async scalar UDFs, UDAFs, UDWFs, and UDTFs—plus `create_udf`, `ScalarUDFImpl`, signatures, volatility, return typing, Arrow vectorization, registration, SQL/DataFrame invocation, documentation metadata, optimizer hooks, error handling, testing, deployment advice, anti-patterns, and an agent checklist.  

The main gap is not “missing UDF basics.” The main gap is an **end-to-end user-defined calculation subsystem**: lifecycle, governance, cataloging, external-library integration, semantic typing, packaging, security policy, optimizer contracts, domain-calculation architecture, and production validation across SQL/DataFrame/physical execution.

The current documentation also gives strong adjacent coverage: expressions are identified as the shared language across DataFrame, SQL planning, optimizer rules, and UDF calls; registries are mapped across scalar, aggregate, and window functions; SQL extension hooks cover `ExprPlanner`, `TypePlanner`, and `RelationPlanner`; and security sections already mention function allowlists and high-risk function classes.   

---

# Coverage assessment

| Topic area                              |                                                                                                   Current attachment coverage | Gap severity |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------: | -----------: |
| UDF family taxonomy                     |                                                      Strong: scalar, async scalar, aggregate, window, table UDFs are mapped.  |          Low |
| Basic scalar UDF creation               |                                           Strong: `create_udf`, `ScalarUDFImpl`, invocation, registration, scalar fast path.  |          Low |
| Signatures / volatility                 |                                                          Strong: `Signature` constructors, named args, volatility semantics.  |       Medium |
| Return typing / nullability             |                                                                Good: `return_type`, `return_field_from_args`, docs metadata.  |       Medium |
| Vectorized Arrow mechanics              |                                                       Good: scalar fast path, arrays, `ColumnarValue`, row-count invariants.  |       Medium |
| UDAF / UDWF / UDTF                      |                                           Good examples and testing guidance, but could use more production design patterns.  |       Medium |
| Function registry / management          |                                                   Present, but mostly at API level; lacks full lifecycle/catalog governance.  |         High |
| Built-in vs custom calculations         |                                    Covered separately in expression and function sections; lacks unified decision framework.  |         High |
| External-library integration            | Partial: Python/DataFusion/PyArrow/Pandas interop is covered; SciPy/SymPy/PyO3/WASM/native FFI calculation strategy is not.   |         High |
| Complex conditional/nested calculations |      Covered through `Expr`, `CASE`, nested/list/struct functions; lacks calculation-design patterns for real domain models.  |       Medium |
| Calculation packaging                   |                                                  Project layout mentions `functions.rs`; no full plugin/module architecture.  |         High |
| Security / allowlists                   |                                     Strong initial coverage; lacks UDF-specific execution-risk taxonomy and policy compiler.  |       Medium |
| Testing                                 |                                           Strong checklist; needs deeper property/golden/fuzz/differential testing strategy.  |       Medium |
| Observability                           |          Mentioned via metrics/explain; lacks function-level metrics, per-UDF spans, cache hit/miss, external call telemetry. |         High |
| Distributed execution                   |   Ballista extension implications are covered, but UDF/UDAF portability across distributed execution needs deeper treatment.  |       Medium |

---

# Proposed deep-dive gap topics

## C1) User-defined calculation architecture and decision tree

**Purpose:** create one canonical map for deciding whether a calculation belongs in `Expr`, built-in SQL, scalar UDF, async UDF, UDAF, UDWF, UDTF, custom logical node, custom physical operator, or external preprocessing.

**Current attachment coverage:** Section 24 has a UDF family map and selection matrix; Section 11 covers `Expr`; Section 25 covers SQL planner extensions.   

**Gap to fill:** a unified “calculation placement” framework.

**Deep dive:**

* Calculation output cardinality: scalar-per-row, scalar-per-group, scalar-per-window-row, relation/table, side-effecting external lookup.
* Evaluation phase: planning-time, logical expression evaluation, physical batch evaluation, aggregate state update, window-frame evaluation, table scan planning.
* Recommended placement:

  * `Expr` for composable SQL/DataFrame expressions.
  * built-in functions for standard SQL semantics.
  * scalar UDF for row-wise deterministic custom transforms.
  * async scalar UDF for network/I/O lookups.
  * higher-order UDF for lambda-taking element-wise array calculations (DataFusion 54; see C1.19/C1.20).
  * UDAF for grouped custom state.
  * UDWF for frame-aware analytics.
  * UDTF for parameterized relation generation.
  * custom physical operator when execution behavior cannot be represented as ordinary expressions.
* Decision matrix by cost, statefulness, determinism, optimizer visibility, distributed portability, security, and packaging complexity.

**Why high value:** prevents LLM agents from overusing scalar UDFs for work better expressed as `Expr`, UDAF, table provider, or physical operator.

---

## C2) Calculation lifecycle and invariants

**Purpose:** make user-defined calculations first-class lifecycle artifacts, not ad hoc functions registered in setup code.

**Current attachment coverage:** Section 24 has implementation, testing, deployment, anti-patterns, and checklist coverage; Section 3 documents function registries and context lifetime.  

**Gap to fill:** a full lifecycle model.

**Deep dive:**

* Lifecycle phases:

  * define semantic intent;
  * choose calculation class;
  * define name, aliases, signature, parameter names;
  * define return type/nullability/metadata;
  * implement Arrow vectorization;
  * register into context/session/catalog;
  * expose SQL/DataFrame invocation helpers;
  * document and version;
  * authorize;
  * test;
  * profile;
  * deploy;
  * deprecate.
* Hard invariants:

  * UDF registered before planning.
  * signature agrees with coercion.
  * volatility matches true semantics.
  * output row count matches input row count for scalar array outputs.
  * UDAF state schema matches `state()` and `merge_batch()`.
  * UDWF respects partition/frame/order contracts.
  * UDTF planning must not perform expensive scans.
* Lifecycle artifacts:

  * `CalculationSpec`;
  * `FunctionManifest`;
  * `SignatureSpec`;
  * `ReturnFieldSpec`;
  * `NullPolicy`;
  * `VolatilityPolicy`;
  * `TestManifest`;
  * `SecurityPolicy`.

**Why high value:** gives agents a stable checklist for adding calculations safely and repeatably.

---

## C3) Function registry, cataloging, and discovery

**Purpose:** connect DataFusion function registration with a governed product-level function catalog.

**Current attachment coverage:** the docs map scalar, aggregate, and window registries to `SessionState` / `TaskContext`, and show `register_udf`, `register_udaf`, and `register_udwf`. 

**Gap to fill:** catalog semantics beyond API registration.

**Deep dive:**

* Distinguish:

  * DataFusion runtime registry;
  * product function catalog;
  * tenant function policy;
  * documentation registry;
  * version/deprecation registry.
* Catalog fields:

  * canonical name;
  * aliases;
  * family: scalar / async / aggregate / window / table;
  * signature;
  * named parameters;
  * volatility;
  * null behavior;
  * deterministic flag;
  * cost class;
  * external I/O flag;
  * security class;
  * enabled tenants;
  * doc section;
  * examples;
  * test fixture IDs.
* Discovery surfaces:

  * generated docs;
  * `SHOW FUNCTIONS`-style product metadata;
  * `information_schema` extension;
  * CLI diagnostic command;
  * agent-readable JSON/YAML manifest.
* Collision rules:

  * overriding built-ins;
  * alias precedence;
  * quoted vs unquoted function names;
  * case normalization.
* Versioning:

  * semantic version per function;
  * breaking signature changes;
  * compatibility aliases;
  * deprecation warnings.

**Why high value:** makes UDFs manageable in a long-lived platform rather than hidden inside Rust initialization code.

---

## C4) Function package and plugin architecture

**Purpose:** define how to organize many custom calculations across crates/modules/plugins.

**Current attachment coverage:** project layout mentions `functions.rs` for UDF/UDAF registration, and dependency guidance covers top-level `datafusion` versus subcrates. 

**Gap to fill:** modular packaging strategy.

**Deep dive:**

* Crate layout:

  * `calc-core`;
  * `calc-arrow-kernels`;
  * `calc-functions`;
  * `calc-aggregates`;
  * `calc-window`;
  * `calc-table-functions`;
  * `calc-manifest`;
  * `calc-tests`.
* Registration pattern:

  * `register_all(ctx)`;
  * `register_domain_package(ctx, PackageOptions)`;
  * `FunctionPackage` trait;
  * deterministic package ordering.
* Feature flags:

  * `math`;
  * `strings`;
  * `engineering`;
  * `external_io`;
  * `python_bridge`;
  * `experimental`.
* Plugin model:

  * static linking for trusted production;
  * dynamic loading only with explicit sandboxing;
  * no arbitrary user-supplied native code.
* Deployment manifests:

  * list enabled packages;
  * tenant/function allowlists;
  * cost/resource class;
  * external credentials needed.

**Why high value:** supports large function libraries without turning `main.rs` or `functions.rs` into an unmaintainable registration dump.

---

## C5) Signature design and overload resolution

**Purpose:** make signatures predictable, coercion-safe, and agent-generatable.

**Current attachment coverage:** Section 24 lists `Signature` constructors, named args, exact/uniform/numeric/string/variadic/any/user-defined signatures. 

**Gap to fill:** applied signature design patterns and anti-patterns.

**Deep dive:**

* Exact vs coercible vs comparable vs numeric vs string vs variadic.
* Named-argument ergonomics.
* Overload alternatives:

  * one polymorphic UDF;
  * multiple named UDFs;
  * `Signature::one_of`;
  * `Signature::user_defined` plus `coerce_types`.
* Signature examples:

  * `add_one(Int64) -> Int64`;
  * `clip(Float64, Float64, Float64) -> Float64`;
  * `safe_divide(Numeric, Numeric) -> Float64/Decimal`;
  * `vector_distance(List<Float64>, List<Float64>) -> Float64`;
  * `struct_get(Struct, Utf8) -> dynamic field`.
* Coercion hazards:

  * decimal scale loss;
  * `Utf8` vs `Utf8View`;
  * dictionary strings;
  * timestamp units/timezones;
  * null-only arguments;
  * nested list element coercion.
* Agent contract:

  * signature is public API;
  * parameter names are public API;
  * variadic signatures cannot use parameter names;
  * avoid `any` unless runtime type logic is robust.

**Why high value:** prevents function APIs that compile but behave unpredictably under SQL coercion.

---

## C6) Return type, nullability, and metadata inference

**Purpose:** separate simple type inference from production return-field inference.

**Current attachment coverage:** Section 24 describes `return_type` and advanced `return_field_from_args`; Section 7 covers type mapping and UDF type compatibility risks.  

**Gap to fill:** return-field design patterns.

**Deep dive:**

* `return_type(arg_types)` for pure type-from-type logic.
* `return_field_from_args(args)` for:

  * nullability propagation;
  * metadata propagation;
  * nested output fields;
  * value-dependent return shape;
  * field-name-sensitive return shape.
* Nullability modes:

  * strict null propagation;
  * null-skipping;
  * null-defaulting;
  * error-on-null;
  * all-null -> null.
* Metadata modes:

  * preserve input semantic type;
  * attach unit metadata;
  * attach source lineage;
  * remove unsafe metadata.
* Return shape patterns:

  * scalar primitive;
  * list;
  * struct;
  * map;
  * dictionary;
  * timestamp with timezone;
  * decimal with precision/scale.

**Why high value:** many advanced functions need correct output fields, not just output `DataType`.

---

## C7) Null, NaN, infinity, error, and invalid-input semantics

**Purpose:** make calculation behavior explicit across SQL, Arrow, and domain logic.

**Current attachment coverage:** Section 24 includes error-handling rules and null testing; Section 11 covers null checks and conditional expressions.  

**Gap to fill:** a formal semantic matrix for invalid data.

**Deep dive:**

* Behavior categories:

  * return null;
  * return NaN;
  * return infinity;
  * raise planning error;
  * raise execution error;
  * clamp;
  * default;
  * emit diagnostic side table.
* Function modes:

  * strict;
  * tolerant;
  * `try_` variant;
  * audited variant.
* Examples:

  * division by zero;
  * log negative;
  * sqrt negative;
  * invalid enum;
  * malformed timestamp;
  * decimal overflow;
  * vector length mismatch;
  * missing struct field;
  * map key absent.
* Agent rules:

  * never panic;
  * use `plan_err!` / `exec_err!` style errors;
  * document null policy;
  * test empty arrays and all-null arrays.

**Why high value:** calculation correctness is usually about edge semantics, not happy-path arithmetic.

---

## C8) Vectorized Arrow implementation patterns

**Purpose:** provide implementation templates for efficient batch-native UDFs.

**Current attachment coverage:** Section 24 emphasizes vectorized Arrow input/output, scalar fast paths, and avoiding scalar-to-array expansion where possible. 

**Gap to fill:** kernel-level implementation cookbook.

**Deep dive:**

* Input patterns:

  * unary;
  * binary;
  * ternary;
  * variadic;
  * scalar + array;
  * scalar-only;
  * list input;
  * struct input.
* Output builders:

  * primitive array builders;
  * string builders;
  * list builders;
  * struct builders;
  * boolean builders.
* Dispatch:

  * match on `DataType`;
  * use Arrow cast helpers;
  * support `Utf8`, `Utf8View`, `LargeUtf8`;
  * support dictionary inputs where reasonable.
* Performance rules:

  * prefer Arrow compute kernels;
  * avoid row loops for simple arithmetic;
  * pre-allocate builders;
  * avoid repeated downcasts;
  * specialize scalar paths;
  * avoid cloning buffers unnecessarily.
* Validation:

  * array output length;
  * null count expectations;
  * data type equality;
  * memory footprint.

**Why high value:** LLM agents need concrete implementation recipes, not just trait names.

---

## C9) Complex conditionality and expression composition

**Purpose:** show when to use `Expr`/`CASE`/built-ins instead of UDFs.

**Current attachment coverage:** Section 11 covers `col`, `lit`, boolean expressions, casts, aliases, aggregate expressions, scalar functions, `case/when`, and null handling. 

**Gap to fill:** compositional calculation patterns.

**Deep dive:**

* `CASE WHEN` vs scalar UDF:

  * use `CASE` for SQL-visible conditional logic;
  * use UDF when behavior must be reused, type-specialized, or too complex.
* Patterns:

  * conditional defaulting;
  * safe arithmetic;
  * bucketization;
  * piecewise functions;
  * conditional aggregates;
  * conditional window metrics;
  * nested-field conditional extraction;
  * rule-table joins instead of giant `CASE`.
* Generated-Expr builder:

  * structured rule spec -> `Expr`;
  * rule priority;
  * conflict detection;
  * stable aliases.
* Optimizer value:

  * expressions remain visible for pushdown/simplification;
  * opaque UDFs limit optimization unless hooks are implemented.

**Why high value:** prevents UDF overuse and keeps query plans optimizable.

---

## C10) Nested and structured return calculations

**Purpose:** handle functions that consume or produce `LIST`, `STRUCT`, `MAP`, and vector-like arrays.

**Current attachment coverage:** nested support is covered separately, including arrays/lists, structs, map-like fields, field access, and nested Parquet/Arrow support. 

**Gap to fill:** UDF-specific nested contracts.

**Deep dive:**

* UDFs returning:

  * `StructArray`;
  * `ListArray`;
  * `MapArray`;
  * fixed-size list embeddings;
  * nested diagnostics records.
* Functions over nested inputs:

  * vector distance;
  * array normalization;
  * struct validation;
  * map lookup with defaults;
  * nested schema normalization.
* Return field design:

  * child field names;
  * child nullability;
  * metadata propagation;
  * schema evolution.
* Test cases:

  * empty list vs null list;
  * missing map key;
  * null struct;
  * nested null child;
  * unequal vector lengths.

**Why high value:** user-defined calculations in engineering/data-platform settings often produce structured diagnostics, not just primitive scalars.

---

## C11) External-library integration strategy

**Purpose:** define how to integrate non-DataFusion calculation libraries safely.

**Current attachment coverage:** ecosystem integration covers DataFusion Python, PyArrow/Pandas interoperability, and Python UDF/UDAF support; it does not deeply cover SciPy/SymPy/PyO3/native FFI/WASM calculation integration.  

**Gap to fill:** a decision framework for external computation.

**Deep dive:**

* Integration routes:

  * pure Rust crate inside UDF;
  * Arrow compute kernel;
  * Python UDF in DataFusion Python;
  * PyO3 bridge inside Rust service;
  * subprocess/microservice call via async UDF;
  * WASM plugin;
  * precompute outside DataFusion;
  * table provider over external engine.
* Decision criteria:

  * latency;
  * throughput;
  * determinism;
  * memory copy cost;
  * Arrow compatibility;
  * GIL behavior;
  * packaging complexity;
  * sandboxing;
  * service isolation;
  * failure semantics.
* SciPy/SymPy-style cases:

  * numeric kernels;
  * root finding;
  * interpolation;
  * symbolic simplification;
  * expression compilation;
  * derivative/Jacobian generation.
* Rust alternative mapping:

  * native Rust math crates;
  * `nalgebra` / `ndarray`;
  * `faer`;
  * `argmin`;
  * symbolic crates where viable;
  * custom kernels when no equivalent exists.

**Why high value:** this is the major missing area for calculation-heavy engineering environments.

---

## C12) Async UDFs for external I/O and services

**Purpose:** make async UDFs production-safe.

**Current attachment coverage:** async scalar UDFs are introduced, including `AsyncScalarUDFImpl`, `ideal_batch_size`, and network/I/O use cases. 

**Gap to fill:** operational design for external calls.

**Deep dive:**

* Async execution model:

  * batch sizing;
  * concurrency;
  * task scheduling;
  * cancellation;
  * retry;
  * timeout;
  * circuit breaker.
* External call semantics:

  * deterministic cacheable lookup;
  * volatile live API lookup;
  * idempotent vs side-effecting calls;
  * authentication/secret handling.
* Caching:

  * per-query cache;
  * global TTL cache;
  * tenant-scoped cache;
  * negative cache;
  * cache key schema.
* Resource controls:

  * rate limit;
  * concurrency semaphore;
  * request budget;
  * fallback behavior.
* Tests:

  * timeout;
  * remote error;
  * cancellation;
  * partial failure;
  * scalar/null propagation;
  * batch-size sensitivity.

**Why high value:** async UDFs are powerful but can destabilize a query engine if treated like ordinary functions.

---

## C13) Aggregate UDF state design

**Purpose:** make UDAF state schemas correct, mergeable, and memory-accountable.

**Current attachment coverage:** Section 24 includes UDAF registration, state/merge/evaluate rules, state-type correspondence, and UDAF tests.  

**Gap to fill:** deeper state-design cookbook.

**Deep dive:**

* State design:

  * primitive state;
  * multi-field state;
  * list/struct state;
  * approximate sketches;
  * bounded vs unbounded state.
* Required invariants:

  * `state()` shape equals declared state datatypes;
  * `merge_batch()` accepts states from partitions;
  * `update_batch()` and `merge_batch()` are associative/compatible;
  * `evaluate()` is deterministic for same state.
* Distributed readiness:

  * state serializability;
  * merge associativity;
  * floating-point nondeterminism;
  * partial aggregation.
* Memory accounting:

  * `size()`;
  * dynamic state memory;
  * spill implications.
* Examples:

  * weighted average;
  * geometric mean;
  * top-k sketch;
  * approximate quantile;
  * online variance;
  * domain-specific quality metric.

**Why high value:** aggregate functions are where correctness bugs often appear under partitioning and parallel execution.

---

## C14) Window UDF frame semantics

**Purpose:** give agents correct patterns for frame-aware analytics.

**Current attachment coverage:** Section 24 covers `PartitionEvaluator`, `evaluate`, `evaluate_all`, frame tests, ordering, and partitions.  

**Gap to fill:** window-function design guide.

**Deep dive:**

* Window UDF types:

  * rank-like;
  * lag/lead-like;
  * moving statistics;
  * frame-aware custom metrics;
  * partition-global metrics.
* Frame semantics:

  * rows vs range vs groups;
  * bounded vs unbounded;
  * empty frame;
  * order ties;
  * null ordering.
* Performance:

  * row-wise `evaluate`;
  * vectorized `evaluate_all`;
  * rank-aware evaluation;
  * prefix-sum/state reuse patterns.
* Tests:

  * one partition;
  * multiple partitions;
  * tie ordering;
  * all-null frames;
  * empty frames;
  * non-deterministic order risks.

**Why high value:** window semantics are subtle, especially when agents generate SQL and tests.

---

## C15) Table UDFs and parameterized relations

**Purpose:** clarify UDTFs as source/table-producing calculations.

**Current attachment coverage:** Section 24 classifies table UDFs as `TableFunctionImpl -> Arc<dyn TableProvider>` and includes UDTF testing points. DataFusion 54 note: `TableFunctionImpl::call(&[Expr])` is deprecated and default-bodied; implement `call_with_args(args: TableFunctionArgs)` instead, which supplies the argument `Expr`s plus the calling `Session` (`args.session()`) for config/registry access during planning. Do not retain the session reference beyond the `call_with_args` invocation. `TableFunctionImpl` also lost its `fn as_any` — `Any` is now a supertrait.

**Gap to fill:** UDTF architecture and examples.

**Deep dive:**

* Use cases:

  * `range`;
  * parameterized scenario source;
  * generated calendar table;
  * simulation case table;
  * external API table;
  * synthetic data;
  * model-result table.
* Planning contract:

  * implement `call_with_args(TableFunctionArgs)`, not the deprecated `call`;
  * validate arguments;
  * require literals where necessary;
  * build schema without scanning;
  * return `TableProvider`.
* Runtime contract:

  * scan should honor projection/filter/limit where possible;
  * remote scanning deferred to execution;
  * expose statistics if possible.
* Security:

  * limit generated cardinality;
  * path/tenant restrictions;
  * no expensive work during planning.

**Why high value:** many domain calculations are table-valued, not scalar.

---

## C16) Optimizer interaction for custom calculations

**Purpose:** expose UDFs to optimization where safe.

**Current attachment coverage:** Section 24 lists `ScalarUDFImpl` optional hooks: `coerce_types`, `return_field_from_args`, `simplify`, `evaluate_bounds`, `propagate_constraints`, `output_ordering`, `preserves_lex_ordering`, documentation, and placement. 

**Gap to fill:** rules for when and how to implement hooks.

**Deep dive:**

* Hook-by-hook guide:

  * `simplify`;
  * `coerce_types`;
  * `evaluate_bounds`;
  * `propagate_constraints`;
  * `output_ordering`;
  * `preserves_lex_ordering`;
  * `placement`.
* Conservative implementation principles:

  * never overclaim monotonicity;
  * preserve aliases;
  * respect volatility;
  * preserve null semantics;
  * return unknown when unsure.
* Examples:

  * monotone transform;
  * clamp range propagation;
  * safe cast bounds;
  * deterministic constant folding;
  * predicate rewrite around UDF.
* Test strategy:

  * pre/post optimization plan comparison;
  * randomized equivalence testing;
  * constraint soundness tests.

**Why high value:** optimizer hooks can produce major performance wins but also wrong answers if overclaimed.

---

## C17) Function authorization, allowlists, and risk classes

**Purpose:** turn function availability into a security-governed policy.

**Current attachment coverage:** Section 39 includes function allowlists, high-risk function classes, and rules that function registration is not authorization. 

**Gap to fill:** policy specification tied to calculation metadata.

**Deep dive:**

* Risk classes:

  * pure deterministic;
  * volatile;
  * expensive CPU;
  * high-cardinality expanding;
  * external I/O;
  * secret-reading;
  * diagnostic metadata exposure;
  * filesystem/object-store path access.
* Policy schema:

  * allowed functions;
  * denied functions;
  * allowed aliases;
  * max cost class;
  * max expansion factor;
  * volatile allowed?;
  * external I/O allowed?;
  * tenant-specific visibility.
* Plan validation:

  * collect function names from expressions;
  * inspect UDF metadata;
  * reject policy violations;
  * sanitize error messages.
* Runtime enforcement:

  * timeouts;
  * memory limits;
  * row caps;
  * concurrency semaphores;
  * external-call budgets.

**Why high value:** public SQL/function surfaces are security boundaries.

---

## C18) Function observability and diagnostics

**Purpose:** make custom calculations measurable and debuggable.

**Current attachment coverage:** metrics/profiling are covered generally, and Section 24 has error/testing guidance; function-level observability is not deeply specified. 

**Gap to fill:** per-function telemetry model.

**Deep dive:**

* Metrics:

  * calls;
  * input batches;
  * input rows;
  * output rows;
  * null counts;
  * elapsed time;
  * allocation estimates;
  * external calls;
  * cache hits/misses;
  * retries/timeouts.
* Tracing:

  * query ID;
  * function name;
  * batch ID;
  * tenant;
  * argument types;
  * cost class.
* Diagnostics:

  * schema mismatch;
  * wrong row count;
  * downcast failure;
  * invalid argument;
  * external failure.
* `EXPLAIN ANALYZE` integration:

  * where function time appears;
  * limitations;
  * custom metrics exposure.

**Why high value:** UDF-heavy systems otherwise become opaque performance and correctness bottlenecks.

---

## C19) Testing strategy for custom calculations

**Purpose:** upgrade checklist testing into a full test harness.

**Current attachment coverage:** Section 24 includes a robust testing matrix for scalar UDFs, async scalar UDFs, UDAFs, UDWFs, and UDTFs. 

**Gap to fill:** test architecture and reusable fixtures.

**Deep dive:**

* Test levels:

  * direct Arrow array invocation;
  * direct `ColumnarValue` invocation;
  * SQL invocation;
  * DataFrame invocation;
  * optimized plan invocation;
  * physical execution;
  * write/read round-trip.
* Test data:

  * empty batch;
  * one-row batch;
  * all-null batch;
  * mixed-null batch;
  * scalar constants;
  * wrong type;
  * nested types;
  * large batch;
  * randomized data.
* Property tests:

  * associativity for UDAFs;
  * merge equivalence;
  * null invariants;
  * monotonicity if optimizer hook claims it;
  * deterministic result under partition variation.
* Differential tests:

  * compare Rust UDF to Python/SciPy reference;
  * compare SQL expression implementation to UDF implementation;
  * compare optimized vs unoptimized results.
* Golden tests:

  * SQL output;
  * schema;
  * `arrow_typeof`;
  * `EXPLAIN`;
  * error messages.

**Why high value:** custom calculations need correctness proof beyond compilation.

---

## C20) External reference and differential validation

**Purpose:** provide a disciplined route for validating Rust UDFs against trusted scientific or domain libraries.

**Current attachment coverage:** Python/PyArrow/Pandas interop is covered, but not reference-model testing against external math libraries. 

**Gap to fill:** external reference test harness.

**Deep dive:**

* Reference model sources:

  * Python/SciPy;
  * SymPy;
  * NumPy;
  * Pandas;
  * existing Excel formulas;
  * validated client data;
  * rigorous simulator outputs.
* Harness:

  * generate Arrow input batches;
  * export to Python/Pandas/PyArrow;
  * compute reference output;
  * import back to Arrow;
  * compare with tolerance/null policy.
* Comparison modes:

  * exact;
  * absolute tolerance;
  * relative tolerance;
  * ULP;
  * category match;
  * monotonic relation;
  * invariant relation.
* Artifact outputs:

  * failing row examples;
  * max error;
  * distribution of residuals;
  * schema mismatch;
  * null mismatch.

**Why high value:** lets Rust-native UDFs replace Python/SciPy/SymPy/Excel calculations safely.

---

## C21) Calculation DSL / semantic calculation specs

**Purpose:** define calculations declaratively, then compile to `Expr`, UDF calls, or physical execution.

**Current attachment coverage:** expressions and UDFs are covered, but there is no explicit semantic calculation-spec layer. 

**Gap to fill:** an agent-operable IR for calculations.

**Deep dive:**

* `CalculationSpec`:

  * name;
  * inputs;
  * output;
  * expression tree;
  * UDF call;
  * null policy;
  * type policy;
  * units;
  * assumptions;
  * diagnostics.
* Compilation targets:

  * DataFusion `Expr`;
  * SQL string;
  * scalar UDF;
  * UDAF;
  * Python reference function;
  * test harness.
* Validation:

  * schema binding;
  * type inference;
  * unit compatibility;
  * dependency graph;
  * cycle detection.
* Use cases:

  * engineering model calculations;
  * scenario KPIs;
  * derived columns;
  * validation checks;
  * objective-function components.

**Why high value:** aligns with agent-generated, auditable calculation systems.

---

## C22) Domain-specific calculation libraries

**Purpose:** move from generic UDF mechanics to domain packages.

**Current attachment coverage:** generic UDF mechanics are strong; domain-package structure is not developed.

**Gap to fill:** patterns for domain-specific calculation families.

**Deep dive:**

* Package examples:

  * `refinery_assay`;
  * `blend_quality`;
  * `unit_conversion`;
  * `thermo_properties`;
  * `economics`;
  * `constraint_penalties`;
  * `scenario_metrics`.
* Each package:

  * function manifest;
  * UDF implementations;
  * expression helpers;
  * test fixtures;
  * reference data;
  * docs.
* Domain semantics:

  * units;
  * basis;
  * temperature/pressure conditions;
  * density/API gravity conventions;
  * missing-data policy;
  * interpolation/extrapolation policy.
* Versioning:

  * calculation methodology version;
  * data-source version;
  * formula version.

**Why high value:** domain calculations need semantic provenance, not just function names.

---

## C23) Stateful, cached, and memoized calculations

**Purpose:** define what state is allowed and where it belongs.

**Current attachment coverage:** UDAF state and async UDF deployment concerns are covered; broader caching/memoization patterns are not.

**Gap to fill:** state placement policy.

**Deep dive:**

* State categories:

  * per-row local;
  * per-batch temporary;
  * per-query cache;
  * shared immutable reference data;
  * shared TTL cache;
  * aggregate state;
  * external service state.
* Safe state patterns:

  * `Arc` immutable lookup table;
  * tenant-scoped cache;
  * query-local memoization;
  * UDAF accumulator state.
* Unsafe state patterns:

  * mutable global state;
  * cross-tenant cache without tenant key;
  * hidden external side effects;
  * non-idempotent function calls.
* Caching policy:

  * key schema;
  * TTL;
  * invalidation;
  * memory limit;
  * metrics.

**Why high value:** many custom calculations need reference data or external lookups.

---

## C24) Calculation performance engineering

**Purpose:** create a UDF-specific performance guide.

**Current attachment coverage:** general performance tuning covers workload knobs, CPU build tuning, SIMD, Arrow kernels, and UDF scalar fast-path notes.  

**Gap to fill:** performance tuning specifically for calculations.

**Deep dive:**

* UDF profiling:

  * per-batch timing;
  * allocation count;
  * scalar vs array path frequency;
  * null density;
  * branch predictability;
  * string allocation.
* Optimization tactics:

  * Arrow compute kernels;
  * scalar fast path;
  * dictionary-aware path;
  * preallocated builders;
  * no blocking sync I/O;
  * async batch sizing;
  * native Rust math kernels.
* Benchmark cases:

  * small batches;
  * large batches;
  * null-heavy;
  * string-heavy;
  * nested arrays;
  * scalar constants;
  * many partitions.
* Build flags:

  * `--release`;
  * `target-cpu=native`;
  * LTO;
  * PGO.

**Why high value:** user-defined calculations can dominate query runtime.

---

## C25) Distributed execution and portability

**Purpose:** define whether custom calculations work in single-node DataFusion, Ballista, Python bindings, Java bindings, or Spark/Comet contexts.

**Current attachment coverage:** ecosystem integration covers Ballista, Python, Java, and Comet; Ballista extension implications note custom logical/physical operators need codecs and planner logic.  

**Gap to fill:** calculation portability matrix.

**Deep dive:**

* Portability dimensions:

  * scalar UDF;
  * UDAF state;
  * window UDF;
  * UDTF;
  * custom physical operator;
  * custom catalog;
  * custom SQL syntax.
* Single-node Rust:

  * full control.
* Python:

  * Python UDF/UDAF support, PyArrow/Pandas interop.
* Ballista:

  * serialization/codec requirements;
  * distributed state merge;
  * executor deployment.
* Java:

  * native boundary and Arrow memory lifecycle.
* Comet/Spark:

  * Spark-compatible function behavior.
* Deployment checklist:

  * extension availability on every executor;
  * version pinning;
  * state serialization;
  * deterministic merge;
  * fallback behavior.

**Why high value:** prevents building UDFs that only work in one execution mode.

---

## C26) Generated code discipline for LLM agents

**Purpose:** give LLM programming agents rules for adding calculations safely.

**Current attachment coverage:** many sections include “Agent rules” and checklists, especially Section 24. 

**Gap to fill:** a dedicated agent workflow.

**Deep dive:**

* Workflow:

  * classify calculation;
  * write manifest;
  * choose UDF family;
  * implement minimal function;
  * add Arrow tests;
  * add SQL tests;
  * add docs metadata;
  * add allowlist entry;
  * run benchmark;
  * update catalog.
* Required generated artifacts:

  * Rust implementation;
  * registration function;
  * SQL examples;
  * DataFrame helper;
  * test cases;
  * manifest entry;
  * docs entry.
* Prohibited shortcuts:

  * `unwrap`;
  * panics;
  * unvalidated downcast;
  * wrong volatility;
  * missing null tests;
  * hidden blocking I/O;
  * unbounded table function.

**Why high value:** turns UDF creation into a reliable agent-run coding task.

---

# Recommended expansion order

1. **C1 + C2:** calculation architecture and lifecycle.
2. **C3 + C4:** registry/catalog/plugin organization.
3. **C5 + C6 + C7:** signature, typing, null/error semantics.
4. **C8 + C9 + C10:** Arrow vectorization, expression composition, nested outputs.
5. **C11 + C12:** external-library and async integration.
6. **C13 + C14 + C15:** aggregate, window, and table-valued functions.
7. **C16 + C17 + C18:** optimizer, security, observability.
8. **C19 + C20:** testing and differential validation.
9. **C21 + C22 + C23:** semantic calculation specs, domain packages, state/caching.
10. **C24 + C25 + C26:** performance, distributed portability, and LLM-agent implementation discipline.


# DataFusion Advanced — C1) User-defined calculation architecture and decision tree

## C1.0 Objective

Create a **canonical calculation-placement framework** for LLM programming agents deciding whether a user-defined calculation belongs in:

```text
Expr / DataFrame expression
built-in SQL function
scalar UDF
async scalar UDF
higher-order UDF (lambda-taking; DataFusion 54, see C1.19/C1.20)
UDAF
UDWF
UDTF
custom SQL planner extension
custom logical node
custom physical operator
custom TableProvider
external preprocessing / preprocessing table
```

This chapter complements, rather than repeats, the attachment’s existing Section 24 UDF guide, Section 11 expression guide, Section 25 SQL-extension guide, and Section 26 custom-operator guide. The attachment already identifies DataFusion’s public capability surface as SQL, DataFrame API, expression API, logical/physical plans, optimizer, custom table providers, UDF/UDAF/UDWF/UDTF functions, custom SQL syntax, and custom logical/physical nodes. 

DataFusion’s own current docs describe it as an extensible Rust query engine using Arrow, with SQL/DataFrame APIs, full query planning, vectorized streaming execution, and customization points including data sources, query languages, functions, and operators. ([Docs.rs][1])

---

## C1.1 Core placement principle

```text
Choose the highest-level representation that preserves:
  correctness
  optimizer visibility
  type/nullability semantics
  security policy
  deployment portability
  execution efficiency
```

Default hierarchy:

```text
external preprocessing only when calculation does not belong inside query execution

built-in SQL / Expr
  before scalar UDF

scalar UDF
  before custom physical operator

UDAF / UDWF / UDTF
  before abusing scalar UDF for group/window/table behavior

custom logical / physical operator
  only when relational algebra + functions cannot express required execution semantics
```

**Primary anti-pattern:** implementing every domain calculation as a scalar UDF. Scalar UDFs are appropriate for row-wise custom transforms, but they become opaque relative to built-in `Expr` rewrites, can reduce optimizer visibility, and do not naturally model grouped state, window frames, relation generation, custom scans, or execution algorithms.

---

## C1.2 DataFusion pipeline placement map

```text
Source data / external system
  │
  ├─ external preprocessing / materialization
  │     └─ produces files, tables, Arrow batches, Parquet, Delta/lake objects
  │
  ├─ TableProvider / UDTF planning
  │     └─ relation/table generation or source abstraction
  │
  ├─ SQL parsing / SqlToRel
  │     ├─ built-in SQL syntax
  │     ├─ ExprPlanner
  │     ├─ TypePlanner
  │     └─ RelationPlanner
  │
  ├─ LogicalPlan + Expr
  │     ├─ built-in expressions
  │     ├─ scalar function Expr
  │     ├─ aggregate Expr
  │     ├─ window Expr
  │     └─ custom logical extension node
  │
  ├─ analyzer + logical optimizer
  │     ├─ type coercion
  │     ├─ expression simplification
  │     ├─ projection/filter/limit pushdown
  │     ├─ custom optimizer rules
  │     └─ UDF hooks where implemented
  │
  ├─ physical planner
  │     ├─ built-in ExecutionPlan nodes
  │     ├─ physical expressions
  │     └─ custom physical operator
  │
  ├─ physical execution
  │     ├─ scalar UDF invoke/invoke_with_args
  │     ├─ async UDF invoke_async_with_args
  │     ├─ UDAF accumulator update/merge/evaluate
  │     ├─ UDWF partition evaluator
  │     ├─ TableProvider scan
  │     └─ custom ExecutionPlan::execute
  │
  └─ SendableRecordBatchStream
        └─ Arrow RecordBatch output
```

DataFusion SQL extension planners intercept SQL AST fragments during `SqlToRel` and convert them to DataFusion `LogicalPlan` / `Expr`; the official extension points are `ExprPlanner`, `TypePlanner`, and `RelationPlanner`. ([Apache DataFusion][2])

---

## C1.3 Calculation cardinality taxonomy

| Calculation cardinality                | Semantic shape                                            | Preferred DataFusion surface                 | Avoid                                         |
| -------------------------------------- | --------------------------------------------------------- | -------------------------------------------- | --------------------------------------------- |
| scalar-per-row                         | `row -> value`                                            | `Expr`, built-in scalar function, scalar UDF | UDAF, custom physical node                    |
| scalar-per-row + external async lookup | `row -> future(value)`                                    | async scalar UDF                             | sync blocking scalar UDF                      |
| element-wise over array/list column    | `array<T> -> array<U> / bool / scalar` per row            | SQL lambda + built-in higher-order function (`array_transform`/`array_filter`/`array_any_match`); custom `HigherOrderUDFImpl` (DF 54) | `UNNEST` → filter/transform → re-aggregate pipelines; scalar UDF over serialized arrays |
| scalar-per-group                       | `group<Row> -> value`                                     | built-in aggregate, UDAF                     | scalar UDF with hidden global state           |
| scalar-per-window-row                  | `row + partition/frame -> value`                          | built-in window function, UDWF               | UDAF, scalar UDF                              |
| relation/table                         | `args -> table`                                           | UDTF, `TableProvider`, `RelationPlanner`     | scalar UDF returning encoded JSON/table       |
| custom source scan                     | `source + projection/filter/limit -> stream<RecordBatch>` | custom `TableProvider`                       | UDTF if source must be catalog-governed       |
| custom execution algorithm             | `logical semantics + nonstandard runtime`                 | custom logical + physical operator           | opaque scalar UDF                             |
| source-independent preprocessing       | `raw data -> curated data`                                | external preprocessing / ETL                 | row-wise UDF if expensive/reused              |
| side-effecting calculation             | `row -> external side effect`                             | usually outside DataFusion                   | UDF, unless read-only/idempotent async lookup |

DataFusion’s UDF guide classifies scalar, window, aggregate, table, and async scalar UDFs; scalar UDFs take a row and return one value, aggregate UDFs take a group and return one value, window UDFs access surrounding rows, table UDFs return a `TableProvider`, and async scalar UDFs support async network/I/O calls. ([Apache DataFusion][3])

---

## C1.4 Evaluation-phase taxonomy

| Phase                         | Calculation examples                                                                | Correct surface                                  | Planner/optimizer visibility                          |
| ----------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------ | ----------------------------------------------------- |
| pre-query materialization     | unit conversions over raw files, simulation outputs, reference table builds         | external preprocessing, ETL, materialized table  | high after materialization                            |
| table scan planning           | custom API source, parameterized generated relation, projection/filter-aware source | `TableProvider`, UDTF                            | high if provider exposes pushdown/statistics          |
| SQL planning                  | custom operator syntax, custom SQL type, custom relation syntax                     | `ExprPlanner`, `TypePlanner`, `RelationPlanner`  | high after conversion to `Expr`/`LogicalPlan`         |
| logical expression evaluation | arithmetic, `CASE`, casts, built-ins, scalar UDF calls                              | `Expr`, built-in function, scalar UDF            | highest for built-ins/Expr; lower for opaque UDF      |
| aggregate state update        | weighted average, custom variance, grouped domain metric                            | UDAF                                             | moderate/high if state is well-defined                |
| window frame evaluation       | rolling score, frame-aware anomaly, custom rank-like calculation                    | UDWF                                             | moderate; frame/order-sensitive                       |
| physical batch evaluation     | Arrow-array kernel, vectorized numeric transform                                    | scalar UDF, physical expression, custom operator | function-dependent                                    |
| physical operator execution   | nonstandard repartitioning, custom temporal index, specialized stream operator      | custom `ExecutionPlan`                           | high only if planner/optimizer integration is correct |
| post-query formatting         | report rendering, chart formatting, API JSON shaping                                | outside DataFusion, output layer                 | irrelevant to query optimizer                         |

Logical plans can be built directly with `LogicalPlanBuilder`, and DataFusion emphasizes that building logical plan structure does not execute the query; execution happens later after planning and optimization. ([Apache DataFusion][4])

---

## C1.5 Canonical decision tree

```text
START: define calculation contract.

1. Does calculation change row cardinality into a new relation/table?
   ├─ yes, args -> table:
   │     ├─ backed by source/storage/API and should support scan pushdown?
   │     │     → custom TableProvider
   │     ├─ parameterized table generator callable in SQL?
   │     │     → UDTF
   │     └─ custom FROM syntax required?
   │           → RelationPlanner + TableProvider / LogicalPlan
   └─ no:
         continue

2. Does calculation aggregate multiple rows into one value per group?
   ├─ yes:
   │     ├─ built-in aggregate exists?
   │     │     → built-in aggregate
   │     └─ custom state/merge/evaluate required?
   │           → UDAF
   └─ no:
         continue

3. Does calculation need access to window partition/order/frame?
   ├─ yes:
   │     ├─ built-in window function + Expr composition works?
   │     │     → built-in window function + Expr
   │     └─ custom frame-aware logic?
   │           → UDWF
   └─ no:
         continue

4. Is calculation expressible as ordinary column expressions?
   ├─ yes:
   │     ├─ all needed built-in functions/operators exist?
   │     │     → Expr / DataFrame / SQL expression
   │     └─ syntax sugar desired only?
   │           → Expr helper function, not UDF
   └─ no:
         continue

5. Is calculation row-wise, deterministic/volatile, vectorizable over Arrow arrays?
   ├─ yes:
   │     ├─ element-wise over array/list values (per-element transform/predicate)?
   │     │     ├─ expressible with built-in higher-order functions + SQL lambdas?
   │     │     │     → array_transform / array_filter / array_any_match with `x -> expr` (DF 54, see C1.19)
   │     │     └─ custom lambda-consuming semantics required?
   │     │           → HigherOrderUDFImpl (DF 54, see C1.20)
   │     ├─ standard SQL function exists?
   │     │     → built-in SQL function
   │     └─ custom domain transform required?
   │           → scalar UDF
   └─ no:
         continue

6. Does calculation perform network / database / external service I/O?
   ├─ yes:
   │     ├─ read-only/idempotent lookup, safe under query cancellation?
   │     │     → async scalar UDF with timeout/cache/rate-limit
   │     └─ side-effecting or transactional external mutation?
   │           → external workflow, not DataFusion UDF
   └─ no:
         continue

7. Does calculation require custom SQL syntax but maps to normal Expr/LogicalPlan?
   ├─ custom expression/operator → ExprPlanner
   ├─ custom SQL type → TypePlanner
   └─ custom FROM relation → RelationPlanner

8. Does calculation require execution behavior not representable by relational algebra/functions?
   ├─ yes:
   │     → custom logical node + custom physical ExecutionPlan
   └─ no:
         → revisit Expr/UDF/UDAF/UDWF/UDTF; avoid overextension.
```

---

# C1.6 Placement surface deep dives

## C1.6.1 `Expr` / DataFrame expression

### Use when

```text
calculation = composable expression over columns/literals/functions
output = scalar per input row
logic = arithmetic / boolean / cast / CASE / null handling / built-in function composition
```

### Value case

`Expr` is the most optimizer-visible calculation representation. Use it for generated plans because DataFusion can inspect expression structure for type coercion, projection pruning, filter pushdown, constant folding, expression simplification, and plan rewrites. The attachment already frames `Expr` as the shared language between DataFrame, SQL planning, optimizer rules, and UDF calls. 

### Syntax

```rust
use datafusion::prelude::*;

let margin_expr =
    (col("revenue") - col("cost")).alias("margin");

let margin_pct_expr =
    ((col("revenue") - col("cost")) / nullif(col("revenue"), lit(0.0)))
        .alias("margin_pct");

let bucket_expr =
    case(col("margin_pct").gt(lit(0.20)))
        .when(lit(true), lit("high_margin"))
        .otherwise(lit("normal_margin"))?
        .alias("margin_bucket");

let df = df.select(vec![
    col("unit_id"),
    margin_expr,
    margin_pct_expr,
    bucket_expr,
])?;
```

### Placement policy

| Calculation shape                    |                               Use `Expr`? | Reason                            |
| ------------------------------------ | ----------------------------------------: | --------------------------------- |
| arithmetic derived column            |                                       yes | transparent to optimizer          |
| `CASE`-based business rule           |                                       yes | visible, testable, SQL-equivalent |
| null/default handling                |                                       yes | built-in null semantics           |
| unit conversion with constant factor |                                       yes | no UDF needed                     |
| reusable expression family           | yes, wrap in Rust helper returning `Expr` | reusable without hiding logic     |
| complex iterative numerical method   |                                        no | use scalar UDF / preprocessing    |
| group state                          |                                        no | use UDAF                          |
| frame state                          |                                        no | use UDWF                          |
| relation generation                  |                                        no | use UDTF/TableProvider            |

### Rust helper pattern

```rust
use datafusion::prelude::*;

pub fn safe_divide(numerator: Expr, denominator: Expr, alias: &str) -> Expr {
    (numerator / nullif(denominator, lit(0.0))).alias(alias)
}

pub fn gross_margin_expr(revenue: Expr, cost: Expr) -> Expr {
    safe_divide(revenue.clone() - cost, revenue, "gross_margin")
}
```

### Agent rules

```text
Prefer Expr when:
  logic can be written with col/lit/operators/CASE/built-ins.
  calculation should remain optimizer-visible.
  generated Rust code controls the query.
  SQL portability is not the primary user contract.

Avoid scalar UDF when:
  UDF only wraps a simple expression.
  UDF hides predicates that could be pushed down.
  UDF hides casts/null logic that the optimizer can otherwise see.
```

---

## C1.6.2 Built-in SQL / built-in function

### Use when

```text
calculation = standard SQL / DataFusion built-in behavior
output = scalar / aggregate / window / nested-function output
```

### Value case

Built-ins provide the best combination of optimizer integration, type coercion maturity, documentation, SQL portability, and test coverage. Use built-ins before introducing custom code.

### Syntax

```sql
SELECT
  customer_id,
  coalesce(nickname, full_name, 'unknown') AS display_name,
  round(amount, 2) AS amount_rounded,
  date_trunc('day', event_ts) AS event_day,
  sum(amount) AS total_amount,
  row_number() OVER (
    PARTITION BY customer_id
    ORDER BY event_ts
  ) AS event_rank
FROM events
GROUP BY customer_id, nickname, full_name, amount, event_ts;
```

### Placement policy

| Need                  | Preferred built-in family                                 |
| --------------------- | --------------------------------------------------------- |
| defaulting            | `coalesce`, `nullif`, `ifnull`                            |
| string normalization  | `lower`, `upper`, `trim`, `regexp_*` where available      |
| timestamp bucketing   | `date_trunc`, `date_bin`                                  |
| vector/list ops       | `array_*`, `list_*`                                       |
| struct/map extraction | `get_field`, bracket access, map functions                |
| group metrics         | `sum`, `avg`, `count`, `min`, `max`, percentile functions |
| window ranking        | `row_number`, `rank`, `dense_rank`, `lag`, `lead`         |

### Agent rules

```text
Search built-ins before writing UDF.
Prefer built-in aggregate/window functions over UDAF/UDWF.
Prefer built-in nested functions over custom JSON/string parsing.
Prefer SQL built-ins for user-authored SQL surfaces.
```

---

## C1.6.3 Scalar UDF

### Use when

```text
calculation = row-wise custom transform
input cardinality = one logical row
output cardinality = one scalar value for same row
implementation = vectorizable over Arrow arrays
```

DataFusion scalar UDFs are row-wise functions that are vectorized: they receive Arrow arrays as input and produce an Arrow array with the same number of rows as output. ([Apache DataFusion][3])

### Correct examples

```text
normalize_api_gravity(api: Float64) -> Float64
calculate_blend_index(density, sulfur, aromatics) -> Float64
classify_stream_type(api, sulfur, tan) -> Utf8
parse_domain_code(code: Utf8) -> Struct
vector_norm(values: List<Float64>) -> Float64
```

### Incorrect examples

```text
sum_custom(x) over group                 → UDAF
rolling_custom_score(x) over window      → UDWF
generate_scenario_table(case_id)         → UDTF / TableProvider
fetch_remote_price(symbol)               → async scalar UDF if read-only; otherwise external
custom scan of database/API              → TableProvider
specialized join algorithm               → custom physical operator
```

### Simple registration skeleton

```rust
use std::sync::Arc;
use datafusion::prelude::*;
use datafusion::arrow::datatypes::DataType;
use datafusion::logical_expr::{create_udf, Volatility};

pub fn register_domain_functions(ctx: &SessionContext) {
    let normalize_api = create_udf(
        "normalize_api_gravity",
        vec![DataType::Float64],
        DataType::Float64,
        Volatility::Immutable,
        Arc::new(|args| {
            // skeleton: implement Arrow-array vectorized logic
            // return ColumnarValue / ArrayRef according to current DataFusion API
            todo!("vectorized Arrow implementation")
        }),
    );

    ctx.register_udf(normalize_api);
}
```

### Production scalar UDF decision checklist

```text
[ ] logic cannot be expressed cleanly as Expr/built-ins
[ ] one output row per input row
[ ] deterministic/volatility class is known
[ ] no hidden mutable global state
[ ] no blocking network/filesystem call
[ ] Arrow vectorized implementation exists
[ ] scalar fast path implemented or consciously omitted
[ ] null policy specified
[ ] type coercion specified
[ ] return type/nullability specified
[ ] direct Arrow-array tests exist
[ ] SQL invocation tests exist
```

### Deployment advisory

```text
Use scalar UDF for domain formulas with stable semantics.
Use ScalarUDFImpl for production-grade UDFs with explicit signature, docs, return-field logic, and optimizer hooks.
Do not use scalar UDF as a convenient hiding place for business logic that should remain visible as Expr.
Do not perform blocking I/O inside scalar UDF.
Do not mutate global state.
```

---

## C1.6.4 Async scalar UDF

### Use when

```text
calculation = row-wise lookup/enrichment
requires async external call
external operation = read-only, idempotent, bounded, cancellable
```

DataFusion async scalar UDFs were introduced for asynchronous operations such as network requests or database queries without blocking query execution. ([Apache DataFusion][5])

### Correct examples

```text
lookup_reference_price(symbol, date) -> Decimal
fetch_material_property(material_id, property_name) -> Float64
ask_llm(text, question) -> Utf8      # only in explicitly governed environments
geocode_address(address) -> Struct   # if rate-limited/cacheable
```

### Incorrect examples

```text
charge_credit_card(row)              → external workflow, not UDF
write_to_remote_system(row)          → external sink/workflow
mutate_reference_database(row)       → external transaction
unbounded web scrape(row)            → preprocessing / separate pipeline
```

### Skeleton

```rust
use async_trait::async_trait;
use datafusion::logical_expr::{AsyncScalarUDFImpl, ScalarFunctionArgs};
use datafusion::common::Result;
use datafusion::arrow::array::ArrayRef;
use datafusion::config::ConfigOptions;

#[derive(Debug)]
pub struct LookupPriceUdf {
    // signature, client, cache, policy handles
}

#[async_trait]
impl AsyncScalarUDFImpl for LookupPriceUdf {
    async fn invoke_async_with_args(
        &self,
        args: ScalarFunctionArgs,
        options: &ConfigOptions,
    ) -> Result<ArrayRef> {
        // 1. convert ColumnarValue inputs to batch representation only if needed
        // 2. deduplicate lookup keys
        // 3. consult per-query / shared TTL cache
        // 4. enforce concurrency and timeout
        // 5. build Arrow output array matching input row count
        todo!()
    }
}
```

### Required runtime policy

```text
timeout_ms
max_concurrency
max_requests_per_batch
max_unique_keys_per_query
cache_ttl
negative_cache_ttl
retry_policy
circuit_breaker
tenant credential scope
redacted logging
cancellation behavior
fallback behavior
```

### Agent rules

```text
Use async scalar UDF only for read-only/idempotent enrichment.
Batch and deduplicate external calls.
Never run blocking HTTP/database clients inside scalar UDF.
Never perform side effects.
Expose cost/risk class in function catalog.
Disable in public SQL unless explicitly allowed.
```

---

## C1.6.5 UDAF — aggregate user-defined function

### Use when

```text
calculation = many rows per group -> one scalar value
requires custom accumulation state
```

### Correct examples

```text
weighted_avg(value, weight) -> Float64
online_variance(x) -> Float64
custom_quality_index(component_score, weight, penalty) -> Float64
top_k_sketch(item, k) -> List
mass_balance_error(inflow, outflow) -> Float64
```

### Incorrect examples

```text
row-wise formula                    → Expr / scalar UDF
rolling metric by timestamp         → UDWF
relation-valued summary table       → aggregate query / UDTF / preprocessing
```

### Logical skeleton

```rust
use datafusion::prelude::*;

let df = df.aggregate(
    vec![col("unit_id")],
    vec![
        weighted_avg(col("value"), col("weight")).alias("weighted_value")
    ],
)?;
```

### Accumulator-state design contract

```text
update_batch(input arrays)     -> update partial state for one partition/batch
state()                        -> emit serializable state arrays/scalars
merge_batch(states)            -> merge partial states from partitions
evaluate()                     -> final scalar output
size()                         -> memory accounting estimate
```

### Decision policy

| Condition                                       |                                        Use UDAF? |
| ----------------------------------------------- | -----------------------------------------------: |
| calculation depends on multiple rows in a group |                                              yes |
| state is associative/mergeable                  |                                              yes |
| result must be identical under partitioning     |                   yes, if merge logic is correct |
| state requires unbounded collection of all rows |                      maybe; use with memory risk |
| calculation depends on row order within group   | maybe; ordered aggregate or window may be better |
| result per row, not per group                   |                                               no |

### Agent rules

```text
Use UDAF for grouped state.
State must be mergeable.
Floating-point results may vary with partition order; document tolerance.
Do not hide grouped state inside static/global variables.
Test update/state/merge/evaluate separately.
Test single partition vs multiple partitions.
```

---

## C1.6.6 UDWF — user-defined window function

### Use when

```text
calculation = one output per input row
requires partition/order/frame context
```

### Correct examples

```text
rolling_zscore(x) OVER (...)
frame_weighted_avg(x, w) OVER (...)
custom_rank_with_ties(score, category) OVER (...)
event_gap_score(ts, event_type) OVER (...)
frame_quality_flag(metric) OVER (...)
```

### Incorrect examples

```text
group-level one-row output           → UDAF
simple row-wise formula              → Expr / scalar UDF
precomputed time-series rollup table → preprocessing / TableProvider
```

### SQL usage

```sql
SELECT
  unit_id,
  ts,
  flow,
  rolling_quality_score(flow) OVER (
    PARTITION BY unit_id
    ORDER BY ts
    ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
  ) AS rolling_quality_score
FROM unit_flows;
```

### Decision policy

| Condition                                    |        Use UDWF? |
| -------------------------------------------- | ---------------: |
| needs `PARTITION BY` context                 |              yes |
| needs `ORDER BY` context                     |              yes |
| needs frame bounds                           |              yes |
| output cardinality equals input cardinality  |              yes |
| one output per group only                    |     no, use UDAF |
| can be expressed with built-in window + Expr | no, use built-in |

### Agent rules

```text
Use UDWF only for frame-aware row outputs.
Require deterministic ORDER BY in examples/tests.
Test empty frame, one-row frame, all-null frame, tie ordering.
Document ROWS vs RANGE assumptions.
Do not use UDWF as generic group aggregate.
```

---

## C1.6.7 UDTF — table user-defined function

### Use when

```text
calculation = parameters -> relation/table
result = queryable table provider
```

The DataFusion UDF guide defines table UDFs as taking parameters and returning a `TableProvider` used in a query plan. ([Apache DataFusion][3])

### Correct examples

```text
scenario_cases('case_set_001') -> table(case_id, parameter, value)
calendar_table(start_date, end_date, granularity) -> table(date, period)
range_with_metadata(start, stop, step) -> table(value, bucket)
simulation_results(case_id) -> table(unit_id, stream_id, metric, value)
reference_curve(curve_id) -> table(x, y)
```

### Incorrect examples

```text
row-wise transform                       → scalar UDF / Expr
custom scan over durable remote source    → custom TableProvider registered as table
aggregate state                           → UDAF
window frame logic                        → UDWF
```

### SQL usage

```sql
SELECT
  c.case_id,
  c.parameter_name,
  c.parameter_value
FROM scenario_cases('base_2026') AS c
WHERE c.parameter_name LIKE 'crude_%';
```

### Planning contract

```text
validate literal/parameter arguments
construct schema at planning time
return TableProvider
avoid expensive remote scans during planning
support projection/filter/limit pushdown where possible
bound generated cardinality
```

### Agent rules

```text
Use UDTF for parameterized relation generation.
Use TableProvider directly for named/governed durable sources.
Reject unbounded generated tables unless explicit limit/guard exists.
Do not encode table output as JSON in scalar UDF.
```

---

## C1.6.8 Custom SQL planner extension

### Use when

```text
calculation has custom SQL syntax
but can be lowered to standard Expr / LogicalPlan / TableProvider
```

DataFusion’s SQL extension docs say `ExprPlanner` handles custom expressions/operators, `TypePlanner` handles custom SQL data types, and `RelationPlanner` handles custom `FROM` clause elements. Multiple expression/relation planners are invoked in reverse registration order, while only one type planner can be active. ([Apache DataFusion][2])

### Correct examples

```text
custom operator:
  payload ->> 'field'
  vector <=> query_vector

custom type:
  GEOMETRY
  VARIANT
  ENGINEERING_UNIT

custom relation:
  TABLESAMPLE(...)
  SCENARIO 'base_case'
  SIMULATION_CASES(...)
```

### Decision table

| Need                                            | Correct hook                           |
| ----------------------------------------------- | -------------------------------------- |
| SQL parses but custom operator needs semantics  | `ExprPlanner`                          |
| SQL type name should map to Arrow/metadata type | `TypePlanner`                          |
| custom `FROM` relation or table factor          | `RelationPlanner`                      |
| parser cannot parse syntax at all               | dialect/parser wrapper before planner  |
| syntax maps to custom runtime behavior          | planner + custom logical/physical node |

### Agent rules

```text
Extend SQL only for product dialect requirements.
Prefer Rust Expr helpers for internal generated plans.
Planner extensions should lower to standard Expr/LogicalPlan when possible.
Custom syntax must be documented as product dialect.
Return Original/next-planner behavior for unhandled nodes.
Version-pin parser-facing code.
```

---

## C1.6.9 Custom logical node

### Use when

```text
calculation has relational semantics not representable by built-in LogicalPlan nodes
but should remain visible to optimizer/planner as a named semantic operator
```

### Correct examples

```text
semantic model solve node
temporal-index lookup node
domain-specific recursive expansion
scenario overlay operator
custom incremental-materialization operator
```

### Avoid when

```text
ordinary projection/filter/aggregate/join can represent it
function call can represent it
TableProvider can represent it
UDTF can represent it
only syntax sugar is needed
```

### Skeleton mental model

```text
LogicalPlan::Extension(UserDefinedLogicalNode)
  ├─ name
  ├─ inputs
  ├─ schema
  ├─ expressions
  ├─ fmt_for_explain
  └─ with_exprs_and_inputs
```

### Agent rules

```text
Custom logical node requires physical lowering.
Expose schema deterministically.
Expose input plans and expressions for optimizer traversal.
Preserve aliases/output names.
Add EXPLAIN snapshots.
Do not create custom logical node for simple expression macros.
```

---

## C1.6.10 Custom physical operator

### Use when

```text
calculation requires execution algorithm not representable as built-in operators/functions
```

DataFusion’s extending-operators docs describe extension by transforming `LogicalPlan` and `ExecutionPlan` through customized optimizer rules, including replacing recognized logical patterns with indexed/precomputed alternatives where possible. ([Apache DataFusion][6])

### Correct examples

```text
custom temporal index scan / aggregate
specialized streaming state machine
custom model-solve operator
hardware-accelerated kernel pipeline
custom repartitioned algorithm
external engine bridge with Arrow stream output
```

### Avoid when

```text
Expr + built-ins works
scalar UDF works
UDAF state works
TableProvider scan works
optimizer rule rewrite to standard operators works
```

### Execution contract

```text
ExecutionPlan
  ├─ schema/properties
  ├─ children
  ├─ with_new_children
  ├─ execute(partition, TaskContext)
  ├─ metrics
  └─ DisplayAs / EXPLAIN integration

execute(...)
  → SendableRecordBatchStream
  → RecordBatch output matching schema
```

### Agent rules

```text
Custom physical operator is last resort.
Must implement metrics.
Must respect partitioning/order/distribution contracts.
Must support cancellation/backpressure via stream behavior.
Must obey memory reservation/spill policy where applicable.
Must have physical-plan tests and execution tests.
```

---

## C1.6.11 Custom `TableProvider`

### Use when

```text
calculation/source = relation with scan-time behavior
input = table reference + projection/filter/limit
output = stream<RecordBatch>
```

DataFusion’s custom table provider guide describes logical planning as a tree of relational operators and notes that optimizer rewrites such as predicate pushdown, projection pruning, expression simplification, subquery decorrelation, and limit pushdown reduce work while preserving meaning. ([Apache DataFusion][7])

### Correct examples

```text
read model results from domain database
query remote simulation API
expose materialized scenario store
read custom binary file format
provide projected/filter-aware reference data
```

### TableProvider vs UDTF

| Need                                            | Preferred                           |
| ----------------------------------------------- | ----------------------------------- |
| stable named table in catalog                   | `TableProvider`                     |
| parameterized generated table callable in query | UDTF                                |
| durable external source                         | `TableProvider`                     |
| synthetic table from literals/options           | UDTF                                |
| custom `FROM` syntax producing source           | `RelationPlanner` + `TableProvider` |

### Agent rules

```text
Use TableProvider when source semantics matter.
Expose projection/filter/limit pushdown early.
Expose statistics where possible.
Do not implement source scans as scalar UDFs.
Do not do expensive source work during SQL planning.
```

---

## C1.6.12 External preprocessing / materialization

### Use when

```text
calculation is expensive, global, non-query-local, side-effecting, iterative, model-solving, or reused many times
```

### Correct examples

```text
run nonlinear process simulation
solve Pyomo/SCIP/IPOPT optimization
compute large reference property table
materialize assay curves
normalize raw event JSON into typed Parquet
precompute vector embeddings
generate scenario case matrices
```

### Avoid when

```text
calculation is cheap row-wise transform
calculation is simple aggregate/window
calculation benefits from query predicates
calculation must be ad hoc over query-selected rows
```

### Materialization targets

```text
Parquet / Delta table
Arrow IPC
PostgreSQL table
custom TableProvider-backed store
in-memory MemTable for tests
```

### Agent rules

```text
Preprocess when query-time execution would be too slow, unsafe, stateful, or non-deterministic.
Materialize reusable expensive calculations.
Keep DataFusion query path for filtering, joining, projecting, aggregating materialized outputs.
Record formula/model version, source data version, and parameterization.
```

---

# C1.7 Multi-axis decision matrix

| Surface                  | Cardinality               |              State | External I/O |    Optimizer visibility |      Security risk | Portability | Packaging complexity |
| ------------------------ | ------------------------- | -----------------: | -----------: | ----------------------: | -----------------: | ----------: | -------------------: |
| `Expr`                   | row -> scalar             |               none |           no |                 highest |                low |        high |                  low |
| built-in SQL function    | row/group/window          | function-dependent |           no |                    high |         low/medium |        high |                  low |
| scalar UDF               | row -> scalar             |         local only |           no |                  medium |             medium |      medium |               medium |
| async scalar UDF         | row -> scalar             |       cache/client |          yes |              low/medium |               high |  medium/low |                 high |
| UDAF                     | group -> scalar           |        accumulator |           no |                  medium |             medium |      medium |                 high |
| UDWF                     | row + frame -> scalar     |    partition/frame |           no |                  medium |             medium |      medium |                 high |
| UDTF                     | args -> table             |     provider state |        maybe |             medium/high |        medium/high |      medium |                 high |
| `ExprPlanner`            | syntax -> Expr            |               none |           no |     high after lowering |             medium |      medium |          medium/high |
| `TypePlanner`            | SQL type -> type/metadata |               none |           no |                    high |             medium |      medium |          medium/high |
| `RelationPlanner`        | syntax -> relation        |              maybe |        maybe |             medium/high |               high |      medium |                 high |
| custom logical node      | relation -> relation      |              maybe |        maybe |        depends on rules |               high |  low/medium |            very high |
| custom physical operator | stream -> stream          |            runtime |        maybe |   low unless integrated |               high |         low |            very high |
| custom `TableProvider`   | source -> relation        |       source state |        maybe |        high if pushdown |               high |      medium |                 high |
| external preprocessing   | data -> materialized data |           external |    yes/maybe | high after materialized | controlled outside |        high |          medium/high |

---

# C1.8 Cost/state/determinism placement matrix

## Cost class

| Cost class                      | Examples                                  | Recommended placement                                    |
| ------------------------------- | ----------------------------------------- | -------------------------------------------------------- |
| O(1) per row                    | arithmetic, simple cast, flag logic       | `Expr` / built-in                                        |
| O(k) small per row              | string parse, domain formula, vector norm | scalar UDF if no built-in                                |
| O(n) per group                  | weighted average, online variance         | UDAF                                                     |
| O(frame) per row                | rolling custom metric                     | UDWF                                                     |
| O(remote latency) per row/batch | reference lookup, LLM call                | async UDF with batching/cache, or preprocessing          |
| O(dataset) global               | model solve, large normalization          | preprocessing / custom operator only if query-integrated |
| unbounded/side-effecting        | external write, transaction               | outside DataFusion query path                            |

## Determinism class

| Determinism      | Examples                    | Placement                                   |
| ---------------- | --------------------------- | ------------------------------------------- |
| immutable        | `x * 1.8 + 32`              | `Expr`, scalar UDF `Immutable`              |
| stable per query | `now()`, reference snapshot | built-in/stable UDF with query snapshot     |
| volatile         | `random()`, live API value  | volatile UDF / async UDF; restrict in tests |
| side-effecting   | write/charge/send           | outside DataFusion                          |

## Statefulness class

| State                     | Correct surface                             |
| ------------------------- | ------------------------------------------- |
| no state                  | `Expr`, built-in, scalar UDF                |
| per-batch temporary       | scalar UDF internal local variables         |
| per-query cache           | async UDF policy layer                      |
| per-group state           | UDAF accumulator                            |
| per-partition/frame state | UDWF evaluator                              |
| source state              | `TableProvider`                             |
| global mutable state      | avoid; external service or controlled cache |

---

# C1.9 Optimizer-visibility rules

## Highest visibility

```text
Expr
built-in functions
standard LogicalPlan nodes
TableProvider with pushdown/statistics
```

## Medium visibility

```text
scalar UDF with correct signature, volatility, return type, simplify/bounds/constraints hooks
UDAF with declared state
UDWF with declared partition/window semantics
UDTF returning well-described TableProvider
```

## Lowest visibility

```text
opaque scalar UDF hiding predicates
async UDF
custom physical operator without logical semantics
external service calls
string-encoded nested data
```

DataFusion’s query optimizer contains analyzer, logical optimizer, and physical optimizer rules that can rewrite plans and expressions while preserving results. ([Apache DataFusion][8])

### Agent placement rule

```text
If optimizer should see inside the calculation:
  use Expr / built-ins / standard LogicalPlan.

If optimizer only needs function-level metadata:
  use UDF with correct Signature, Volatility, return typing, and optional hooks.

If optimizer must choose a custom algorithm:
  use logical extension + optimizer/planner integration.

If optimizer cannot safely reason about it:
  isolate, restrict, and benchmark.
```

---

# C1.10 Syntax-first placement examples

## Example A — simple derived KPI: use `Expr`

```rust
use datafusion::prelude::*;

pub fn yield_pct(feed: Expr, product: Expr) -> Expr {
    (product / nullif(feed, lit(0.0))).alias("yield_pct")
}

let df = df.select(vec![
    col("unit_id"),
    yield_pct(col("feed_bbl"), col("product_bbl")),
])?;
```

Placement: `Expr`.

Reason:

```text
row-wise
no external state
transparent arithmetic
null policy visible
optimizer can see denominator/nullif
```

---

## Example B — domain formula too complex for expression: scalar UDF

```rust
// SQL usage
SELECT
  stream_id,
  viscosity_index(temp_c, viscosity_cst) AS viscosity_index
FROM stream_properties;
```

Placement: scalar UDF.

Reason:

```text
one output per input row
domain-specific formula
vectorizable over Arrow arrays
not an aggregate/window/source
```

---

## Example C — weighted group metric: UDAF

```sql
SELECT
  crude_slate,
  weighted_avg(sulfur_wt_pct, volume_bbl) AS slate_sulfur_wt_pct
FROM crude_components
GROUP BY crude_slate;
```

Placement: UDAF.

Reason:

```text
many rows per group -> one output
requires state: sum(value * weight), sum(weight)
must merge partial states
not scalar-per-row
```

---

## Example D — rolling custom score: UDWF

```sql
SELECT
  unit_id,
  ts,
  custom_rolling_stability(feed_rate, pressure) OVER (
    PARTITION BY unit_id
    ORDER BY ts
    ROWS BETWEEN 12 PRECEDING AND CURRENT ROW
  ) AS rolling_stability
FROM unit_history;
```

Placement: UDWF.

Reason:

```text
one output per input row
requires ordered frame
not group-level aggregate
```

---

## Example E — scenario table generator: UDTF

```sql
SELECT
  s.case_id,
  s.parameter_name,
  s.parameter_value
FROM scenario_parameters('turnaround_2026') AS s;
```

Placement: UDTF.

Reason:

```text
function returns relation
parameterized relation generation
query can filter/project returned table
```

---

## Example F — remote reference lookup: async scalar UDF or preprocessing

```sql
SELECT
  symbol,
  lookup_market_price(symbol, trade_date) AS price
FROM trades;
```

Placement:

```text
async scalar UDF if:
  read-only
  idempotent
  cached
  bounded
  allowed by policy

external preprocessing if:
  high-volume
  expensive
  reused often
  requires strong audit/retry controls
```

---

## Example G — custom Parquet-backed model store: `TableProvider`

```rust
ctx.register_table(
    "simulation_results",
    Arc::new(SimulationResultsProvider::new(store_config)),
)?;
```

Placement: custom `TableProvider`.

Reason:

```text
stable table/source
needs projection/filter/limit pushdown
source-specific statistics possible
not a parameter-only generated table
```

---

## Example H — custom SQL operator: `ExprPlanner`

```sql
SELECT
  payload ->> 'event_type' AS event_type
FROM events;
```

Placement: `ExprPlanner` lowering to `get_field`/cast/string extraction.

Reason:

```text
syntax customization only
runtime can use normal Expr/function
no custom physical operator required
```

---

## Example I — temporal index rewrite: custom optimizer / operator

```sql
SELECT
  date_bin(INTERVAL '1 minute', ts, TIMESTAMP '1970-01-01') AS bucket,
  avg(metric) AS avg_metric
FROM telemetry
WHERE ts >= TIMESTAMP '2026-01-01'
GROUP BY bucket;
```

Placement: optimizer rule + possible custom table/index/physical operator.

Reason:

```text
standard SQL semantics
custom acceleration via precomputed temporal index
rewrite recognized pattern to indexed scan/table
do not expose as scalar UDF
```

---

# C1.11 Placement anti-patterns

| Anti-pattern                                                | Correct replacement                                             |
| ----------------------------------------------------------- | --------------------------------------------------------------- |
| scalar UDF wraps `a + b`                                    | `Expr` helper                                                   |
| scalar UDF implements `CASE WHEN`                           | `case(...).when(...).otherwise(...)`                            |
| scalar UDF computes group average with global mutable state | UDAF                                                            |
| scalar UDF scans database per row synchronously             | async UDF with batching/cache, or TableProvider/preprocessing   |
| scalar UDF returns JSON string representing table           | UDTF or TableProvider                                           |
| UDTF used for stable source table                           | registered `TableProvider`                                      |
| custom physical operator used for simple projection         | `Expr` / built-in projection                                    |
| custom SQL syntax used only inside Rust-generated plans     | Rust `Expr` helper                                              |
| async UDF performs side effects                             | external workflow                                               |
| UDAF stores all rows unboundedly                            | streaming/mergeable state, approximate sketch, or preprocessing |
| UDWF used without deterministic `ORDER BY`                  | require explicit window ordering                                |
| custom operator ignores aliases/schema                      | preserve schema and output names                                |

---

# C1.12 Calculation manifest for placement governance

Use a manifest before coding. This is the agent-facing contract that drives placement.

```yaml
calculation:
  name: weighted_sulfur
  description: Weighted average sulfur content by volume.
  semantic_class: aggregate_metric

  cardinality:
    input: group_rows
    output: scalar_per_group

  evaluation_phase: aggregate_state_update

  placement:
    selected: UDAF
    rejected:
      Expr: "Requires cross-row grouped state."
      scalar_udf: "Scalar UDF cannot aggregate group rows."
      udwf: "One output per group, not per input row."
      table_provider: "No custom source behavior."

  inputs:
    - name: sulfur_wt_pct
      type: Float64
      nullable: true
      unit: wt_pct
    - name: volume_bbl
      type: Float64
      nullable: true
      unit: bbl

  output:
    name: weighted_sulfur_wt_pct
    type: Float64
    nullable: true
    unit: wt_pct

  semantics:
    null_policy: skip_row_if_value_or_weight_null
    zero_weight_policy: return_null
    determinism: immutable
    volatility: immutable
    state:
      fields:
        - weighted_sum: Float64
        - weight_sum: Float64
      mergeable: true

  optimizer:
    visibility: aggregate_function
    simplification_hooks: false
    bounds_hooks: false

  security:
    external_io: false
    side_effects: false
    allowed_in_public_sql: true
    cost_class: medium

  tests:
    direct_accumulator: true
    sql_group_by: true
    partition_merge_equivalence: true
    null_cases: true
    zero_weight_case: true
```

---

# C1.13 Agent implementation workflow

```text
1. Write CalculationManifest.
2. Classify cardinality:
     row / group / window / table / source / external.
3. Classify phase:
     preprocessing / planning / logical Expr / physical batch / aggregate / window / scan / custom runtime.
4. Choose highest-level surface.
5. Write placement rejection notes for all lower/wrong surfaces.
6. Implement minimal version.
7. Register through explicit package function.
8. Add SQL + DataFrame invocation examples.
9. Add direct unit tests for implementation.
10. Add SQL integration tests.
11. Add EXPLAIN/schema tests if optimizer/planner behavior matters.
12. Add security allowlist/cost metadata.
13. Add benchmark if function is hot-path or external.
```

---

# C1.14 Placement validation test matrix

| Placement                | Required tests                                                               |
| ------------------------ | ---------------------------------------------------------------------------- |
| `Expr` helper            | schema inference, SQL-equivalent output, null behavior                       |
| built-in function use    | version-pinned SQL output, `arrow_typeof`, null behavior                     |
| scalar UDF               | direct Arrow arrays, scalar inputs, nulls, wrong type, SQL invocation        |
| async UDF                | timeout, cancellation, cache, retry, rate limit, nulls, external failure     |
| UDAF                     | update, state, merge, evaluate, partition equivalence, empty group           |
| UDWF                     | partition, order, frame, empty frame, ties, all-null frame                   |
| UDTF                     | argument validation, schema, bounded cardinality, projection/filter behavior |
| SQL planner extension    | parse, plan, fallback, equivalent lowered `Expr`, alias preservation         |
| custom logical node      | schema, expressions, inputs, optimizer traversal, physical lowering          |
| custom physical operator | stream output, partition behavior, cancellation, metrics, memory             |
| TableProvider            | schema, projection/filter/limit pushdown, statistics, scan execution         |
| preprocessing            | materialized schema, lineage, version, differential validation               |

---

# C1.15 Deployment policy by environment

| Environment                | Allowed calculation surfaces                                      | Restricted surfaces                                   |
| -------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------- |
| local developer CLI        | all, with version pinning                                         | side-effecting UDFs                                   |
| internal batch ETL         | `Expr`, built-ins, UDFs, UDAFs, preprocessing, TableProvider      | async UDF without retry/cache policy                  |
| public read-only SQL       | built-ins, approved `Expr`, approved scalar/UDAF/UDWF             | DDL, DML, async external UDFs, unbounded UDTFs        |
| multi-tenant analytics     | tenant-scoped functions and catalogs                              | shared global mutable state, cross-tenant caches      |
| production service API     | `Expr` guardrails, registered table providers, approved functions | direct path SQL, arbitrary UDF packages               |
| distributed execution      | portable functions with serializable state                        | custom physical operators without executor deployment |
| regulated/audited workflow | materialized/preprocessed calculations with lineage               | volatile/remote functions in core metric path         |

---

# C1.16 Security placement rules

```text
Pure deterministic row calculation:
  Expr / scalar UDF allowed if registered and tested.

External read lookup:
  async UDF only if idempotent, bounded, cached, authorized, cancellable.

External write / mutation:
  not a DataFusion function; use workflow/sink layer.

Filesystem/object-store access:
  TableProvider with scoped credentials; avoid arbitrary path UDTFs.

Unbounded cardinality generation:
  reject unless explicit max rows / bounds.

Volatile functions:
  disable in deterministic reports and snapshot tests.

Custom physical operators:
  require code review, metrics, resource controls, and version pinning.
```

---

# C1.17 Recommended default policy for LLM agents

```text
Default generated calculation:
  Expr helper.

If standard function exists:
  built-in function.

If row-wise and not expressible:
  scalar UDF.

If row-wise with network/database read:
  async scalar UDF only with policy/caching; otherwise preprocessing.

If group-level:
  UDAF.

If frame/window-level:
  UDWF.

If relation-valued:
  UDTF or TableProvider.

If source-backed:
  TableProvider.

If syntax-only:
  ExprPlanner / TypePlanner / RelationPlanner.

If runtime algorithm is genuinely new:
  custom logical + physical operator.

If expensive/global/side-effecting:
  external preprocessing.
```

---

# C1.18 Final agent checklist

```text
[ ] Define calculation cardinality.
[ ] Define evaluation phase.
[ ] Check built-ins first.
[ ] Check Expr composition second.
[ ] Reject scalar UDF unless row-wise one-output-per-row is true.
[ ] Use async scalar UDF only for read-only/idempotent async lookup.
[ ] Use UDAF for grouped state.
[ ] Use UDWF for ordered partition/window frame logic.
[ ] Use UDTF for parameterized table generation.
[ ] Use TableProvider for governed source/table behavior.
[ ] Use SQL planner extension only for custom syntax/type/relation lowering.
[ ] Use custom logical node only when standard LogicalPlan cannot express semantics.
[ ] Use custom physical operator only when standard execution cannot implement algorithm.
[ ] Use preprocessing for expensive, global, side-effecting, or reusable calculations.
[ ] Record placement decision in CalculationManifest.
[ ] Add SQL/DataFrame examples.
[ ] Add direct implementation tests.
[ ] Add schema/type/nullability tests.
[ ] Add EXPLAIN/optimizer tests when optimizer behavior matters.
[ ] Add security/cost/allowlist metadata.
[ ] Pin DataFusion version for all extension APIs.
```

[1]: https://docs.rs/datafusion/latest/datafusion/ "datafusion - Rust"
[2]: https://datafusion.apache.org/library-user-guide/extending-sql.html "Extending SQL Syntax — Apache DataFusion  documentation"
[3]: https://datafusion.apache.org/library-user-guide/functions/adding-udfs.html "Adding User Defined Functions: Scalar/Window/Aggregate/Table Functions — Apache DataFusion  documentation"
[4]: https://datafusion.apache.org/library-user-guide/building-logical-plans.html?utm_source=chatgpt.com "Building Logical Plans — Apache DataFusion documentation"
[5]: https://datafusion.apache.org/blog/2025/07/28/datafusion-49.0.0/ "Apache DataFusion 49.0.0 Released - Apache DataFusion Blog"
[6]: https://datafusion.apache.org/library-user-guide/extending-operators.html?utm_source=chatgpt.com "Extending Operators — Apache DataFusion documentation"
[7]: https://datafusion.apache.org/library-user-guide/custom-table-providers.html?utm_source=chatgpt.com "Custom Table Provider — Apache DataFusion documentation"
[8]: https://datafusion.apache.org/library-user-guide/query-optimizer.html?utm_source=chatgpt.com "Query Optimizer — Apache DataFusion documentation"


# C1.19 Higher-order UDFs and lambdas (DataFusion 54)

## C1.19.1 What DataFusion 54 adds

DataFusion 54 introduces first-class **lambda expressions** and **higher-order functions** (functions that take lambdas as arguments) across SQL, the `Expr` API, planning, and execution. This closes the largest historical placement gap in this chapter: per-element logic over `List`/`LargeList` columns no longer requires `UNNEST` pipelines, join-back re-aggregation, or scalar UDFs that reimplement list iteration.

SQL lambda syntax is `param -> expression` (multi-parameter syntax `(x, y) -> expression` also parses; how many parameters a lambda may declare is decided per function by its `lambda_parameters` contract — the shipped array built-ins declare a single element parameter), and lambda bodies may capture outer columns:

```sql
-- per-element transform: no cardinality change, no UNNEST
SELECT array_transform(readings, v -> v * conversion_factor) AS converted
FROM sensor_batches;

-- per-element predicate: keep elements above a per-row threshold (outer-column capture)
SELECT array_filter(readings, v -> v > quality_floor) AS accepted
FROM sensor_batches;

-- existence test over elements
SELECT array_any_match(readings, v -> v IS NULL OR v < 0.0) AS has_bad_reading
FROM sensor_batches;
```

## C1.19.2 Expression model

Three new `Expr` variants carry the logical representation (`datafusion-expr` `expr.rs:429–433`):

```text
Expr::HigherOrderFunction(HigherOrderFunction { func: Arc<HigherOrderUDF>, args: Vec<Expr> })
Expr::Lambda(Lambda { params: Vec<String>, body: Box<Expr> })
Expr::LambdaVariable(LambdaVariable { name, field: Option<FieldRef>, spans })
```

```text id="hof-expr-tree"
HigherOrderFunction(array_transform)
├── args[0]: Column(readings)          -- value argument
└── args[1]: Lambda
    ├── params: ["v"]
    └── body: BinaryExpr(*)
        ├── LambdaVariable("v")        -- lambda-local, NOT a table column
        └── Column(conversion_factor)  -- captured outer column
```

DataFrame/`Expr` composition uses the `lambda`/`lambda_var` helpers from `datafusion_expr::expr_fn` plus the generated expression functions in `datafusion_functions_nested::expr_fn`:

```rust id="hof-expr-compose"
use datafusion::prelude::*;
use datafusion_expr::expr_fn::{lambda, lambda_var};
use datafusion_functions_nested::expr_fn::array_transform;

let converted = array_transform(
    col("readings"),
    lambda(["v"], lambda_var("v") * col("conversion_factor")),
)
.alias("converted");
```

`lambda_var` creates an *unresolved* lambda variable (`field: None`). The SQL planner resolves lambda variables automatically; expression trees built programmatically must be resolved before type inference/execution via `Expr::resolve_lambda_variables` or `LogicalPlan::resolve_lambda_variables`.

## C1.19.3 Built-in higher-order functions

`datafusion-functions-nested` ships the built-ins (each is a `HigherOrderUDF` registered in the session's higher-order function map, see C3.29):

| Function | Shape | Semantics |
|---|---|---|
| `array_transform(array, x -> expr)` (alias `list_transform`) | `List<T> -> List<U>` | element-wise map; output element type = lambda body type |
| `array_filter(array, x -> pred)` | `List<T> -> List<T>` | keep elements where predicate is true |
| `array_any_match(array, x -> pred)` | `List<T> -> Boolean` | true if any element matches |

Compose them like ordinary expressions — the composition stays element-wise and row-aligned:

```sql
SELECT array_transform(
         array_filter(readings, v -> v IS NOT NULL AND v > 0.0),
         v -> ln(v)
       ) AS ln_valid_readings
FROM sensor_batches;
```

## C1.19.4 Replacing UNNEST pipelines

Pre-54 per-element logic typically exploded cardinality and re-aggregated:

```sql
-- BEFORE (53): cardinality explosion + join-back + fragile ordering
WITH exploded AS (
  SELECT b.batch_id, u.v
  FROM sensor_batches b, UNNEST(b.readings) AS u(v)
)
SELECT batch_id, array_agg(v * 2.0) AS converted
FROM exploded
WHERE v IS NOT NULL
GROUP BY batch_id;

-- AFTER (54): element-wise, no cardinality change, element order preserved
SELECT batch_id,
       array_transform(array_filter(readings, v -> v IS NOT NULL), v -> v * 2.0) AS converted
FROM sensor_batches;
```

The rewrite is not merely cosmetic: the `UNNEST` form multiplies row counts through the pipeline, forces a hash aggregation to reassemble arrays, loses guaranteed element ordering unless explicitly tracked, and hides the per-element expression from row-count estimates. The higher-order form is a single projection.

## C1.19.5 Placement decision: compose built-ins vs implement `HigherOrderUDFImpl`

```text id="hof-placement-tree"
Per-element calculation over List/LargeList columns?
├─ expressible as map/filter/exists with an ordinary Expr body?
│     → compose array_transform / array_filter / array_any_match with SQL lambdas
│       (highest optimizer visibility; zero custom code)
├─ needs a custom scalar kernel per element, but iteration is plain map?
│     → register the kernel as a scalar UDF, call it inside the lambda body:
│       array_transform(arr, v -> my_scalar_udf(v))
├─ custom lambda-consuming semantics required?
│   (custom iteration order, multiple lambdas, accumulator threading,
│    element+index parameters, early termination, cross-element state)
│     → implement HigherOrderUDFImpl (see C1.20)
└─ group-wise rather than element-wise state?
      → UDAF, not a higher-order function
```

Anti-patterns:

```text id="hof-anti-patterns"
Do NOT reintroduce UNNEST → transform → array_agg pipelines for element-wise logic.
Do NOT serialize arrays to JSON strings to process them in a scalar UDF.
Do NOT implement HigherOrderUDFImpl when a lambda body over built-ins suffices.
Do NOT model lambdas as string parameters parsed inside a scalar UDF.
```

---

# C1.20 Implementing `HigherOrderUDFImpl`

## C1.20.1 Trait shape

The implementation trait is **`HigherOrderUDFImpl`** (defined in datafusion-expr `src/higher_order_function.rs:458`); the registerable wrapper struct is **`HigherOrderUDF`** (same file, `:829`) — exactly mirroring the `ScalarUDFImpl`/`ScalarUDF` split. The `higher_order_function` module itself is private: import both (plus `HigherOrderSignature`, `HigherOrderTypeSignature`, `HigherOrderFunctionArgs`, `HigherOrderReturnFieldArgs`, `LambdaArgument`, `LambdaParametersProgress`, `ValueOrLambda`) from the crate root — `datafusion_expr::HigherOrderUDFImpl` / `datafusion::logical_expr::HigherOrderUDFImpl`. The trait is declared:

```rust
pub trait HigherOrderUDFImpl: Debug + DynEq + DynHash + Send + Sync + Any {
    // required
    fn name(&self) -> &str;
    fn signature(&self) -> &HigherOrderSignature;
    fn lambda_parameters(
        &self,
        step: usize,
        fields: &[ValueOrLambda<FieldRef, Option<FieldRef>>],
    ) -> Result<LambdaParametersProgress>;
    fn return_field_from_args(&self, args: HigherOrderReturnFieldArgs) -> Result<FieldRef>;
    fn invoke_with_args(&self, args: HigherOrderFunctionArgs) -> Result<ColumnarValue>;

    // optional (default-bodied)
    fn aliases(&self) -> &[String] { &[] }
    fn schema_name(&self, args: &[Expr]) -> Result<String> { /* name(args...) */ }
    fn coerce_value_types(&self, arg_types: &[DataType]) -> Result<Vec<DataType>> { /* not_impl */ }
    fn coerce_values_for_lambdas(
        &self,
        fields: &[ValueOrLambda<DataType, DataType>],
    ) -> Result<Option<Vec<DataType>>> { Ok(None) }
    fn clear_null_values(&self) -> bool { true }
    fn short_circuits(&self) -> bool { false }
    fn conditional_arguments<'a>(&self, args: &'a [Expr])
        -> Option<(Vec<&'a Expr>, Vec<&'a Expr>)> { /* derived from short_circuits */ }
    fn documentation(&self) -> Option<&Documentation> { None }
}
```

There is no `fn as_any` — `Any` is a supertrait (as with all DF-54 UDF impl traits). `DynEq`/`DynHash` have blanket implementations for any `Eq + Hash + Any` type, so `#[derive(Debug, PartialEq, Eq, Hash)]` on the impl struct satisfies them.

## C1.20.2 Signature discipline

`HigherOrderSignature` (constructed with `HigherOrderSignature::new(type_signature, volatility)` or the `exact`/`any`/`variadic_any`/`user_defined` helpers) describes the **positional mix of value arguments and lambdas**:

```rust id="hof-signature"
use datafusion_expr::{HigherOrderSignature, ValueOrLambda, Volatility};

// one value argument followed by one lambda: f(array, x -> expr)
let signature = HigherOrderSignature::exact(
    vec![ValueOrLambda::Value(()), ValueOrLambda::Lambda(())],
    Volatility::Immutable,
);
```

The signature contract decomposes into five obligations:

```text id="hof-signature-discipline"
1. Non-lambda argument types: describe/coerce via the signature plus
   coerce_value_types (called BEFORE lambda output types are known).
2. Lambda arity and position: encoded in HigherOrderTypeSignature::Exact
   (ValueOrLambda::Value(()) vs ValueOrLambda::Lambda(())).
3. Lambda input (parameter) types: returned by lambda_parameters as Fields;
   DataFusion type-checks the lambda body against these. Multi-step resolution
   (LambdaParametersProgress::Partial) supports lambdas whose parameters depend
   on other lambdas' output types; guard rail: signature.lambda_parameters_max_iterations
   (default 256).
4. Result constraints that depend on lambda OUTPUT types: enforce/coerce via
   coerce_values_for_lambdas (e.g. an initial accumulator value that must match
   the merge lambda's output type).
5. Output type derivation: return_field_from_args receives
   ValueOrLambda<FieldRef, FieldRef> per argument — for lambdas, the Field of the
   lambda body's RESULT — and must derive the output Field (type + nullability).
```

## C1.20.3 Complete implementation skeleton

Verified against the commented reference implementation `datafusion-functions-nested-54.1.0/src/array_transform.rs`:

```rust id="hof-impl-skeleton"
use std::sync::Arc;

use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::common::{plan_err, utils::take_function_args, Result};
use datafusion::logical_expr::{
    ColumnarValue, HigherOrderFunctionArgs, HigherOrderReturnFieldArgs,
    HigherOrderSignature, HigherOrderUDFImpl, LambdaParametersProgress,
    ValueOrLambda, Volatility,
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct ElementwiseCalibrate {
    signature: HigherOrderSignature,
}

impl ElementwiseCalibrate {
    pub fn new() -> Self {
        Self {
            // calibrate(readings_array, (v) -> expr)
            signature: HigherOrderSignature::exact(
                vec![ValueOrLambda::Value(()), ValueOrLambda::Lambda(())],
                Volatility::Immutable,
            ),
        }
    }
}

impl HigherOrderUDFImpl for ElementwiseCalibrate {
    fn name(&self) -> &str {
        "elementwise_calibrate"
    }

    fn signature(&self) -> &HigherOrderSignature {
        &self.signature
    }

    // Coerce the value arguments (lambda outputs not yet known here):
    // e.g. accept List/LargeList and views, normalize to a concrete list type.
    fn coerce_value_types(&self, arg_types: &[DataType]) -> Result<Vec<DataType>> {
        match &arg_types[0] {
            DataType::List(_) | DataType::LargeList(_) => Ok(vec![arg_types[0].clone()]),
            other => plan_err!("{} expects a list argument, got {other}", self.name()),
        }
    }

    // Declare the lambda's parameter Fields. For a single list argument the
    // lambda parameter is the list's element field: Complete at step 0.
    fn lambda_parameters(
        &self,
        _step: usize,
        fields: &[ValueOrLambda<FieldRef, Option<FieldRef>>],
    ) -> Result<LambdaParametersProgress> {
        let ValueOrLambda::Value(list_field) = &fields[0] else {
            return plan_err!("{} expects a value followed by a lambda", self.name());
        };
        let element_field = match list_field.data_type() {
            DataType::List(f) | DataType::LargeList(f) => Arc::clone(f),
            other => return plan_err!("expected list, got {other}"),
        };
        Ok(LambdaParametersProgress::Complete(vec![vec![element_field]]))
    }

    // Derive the output Field. For lambdas, args.arg_fields carries the Field
    // of the lambda body's result when evaluated with the declared parameters.
    fn return_field_from_args(
        &self,
        args: HigherOrderReturnFieldArgs,
    ) -> Result<FieldRef> {
        let [ValueOrLambda::Value(list), ValueOrLambda::Lambda(lambda)] =
            take_function_args(self.name(), args.arg_fields)?
        else {
            return plan_err!("{} expects a value followed by a lambda", self.name());
        };
        let element = Arc::new(Field::new(
            Field::LIST_FIELD_DEFAULT_NAME,
            lambda.data_type().clone(),
            lambda.is_nullable(),
        ));
        let return_type = match list.data_type() {
            DataType::List(_) => DataType::List(element),
            DataType::LargeList(_) => DataType::LargeList(element),
            other => return plan_err!("expected list, got {other}"),
        };
        Ok(Arc::new(Field::new("", return_type, list.is_nullable())))
    }

    fn invoke_with_args(&self, args: HigherOrderFunctionArgs) -> Result<ColumnarValue> {
        // args.args: Vec<ValueOrLambda<ColumnarValue, LambdaArgument>>
        // args.number_rows, args.return_field, args.config_options also available.
        let [list, lambda] = take_function_args(self.name(), &args.args)?;
        let (ValueOrLambda::Value(list), ValueOrLambda::Lambda(lambda)) = (list, lambda)
        else {
            return plan_err!("{} expects a value followed by a lambda", self.name());
        };

        let list_array = list.to_array(args.number_rows)?;
        // 1. flatten list values to one contiguous values array;
        // 2. lambda.evaluate(&[&values_closure], spread_captures) evaluates the
        //    lambda body once over ALL elements (vectorized, not per row) —
        //    spread_captures must expand captured outer columns from one value
        //    per outer row to one value per element (take with row indices);
        // 3. reassemble the result with the original offsets/null buffer.
        todo!("flatten -> lambda.evaluate -> reassemble; see array_transform.rs")
    }
}
```

Wrap and register:

```rust id="hof-register"
use datafusion::logical_expr::HigherOrderUDF;

let udf = HigherOrderUDF::new_from_impl(ElementwiseCalibrate::new());
ctx.register_higher_order_function(Arc::new(udf));
// SQL: SELECT elementwise_calibrate(readings, v -> v * 1.05 + 0.2) FROM t;
```

## C1.20.4 Lambda variables are NOT table columns

`Expr::Lambda` and `Expr::LambdaVariable` are explicitly **excluded** from column collection: `expr_to_columns` in `datafusion-expr` `utils.rs` matches `Expr::HigherOrderFunction(_) | Expr::Lambda(_) | Expr::LambdaVariable(_)` and adds nothing for the lambda-locals. Any custom expression visitor, required-column analyzer, projection-pushdown helper, or lineage extractor in your codebase must follow the same rule:

```text id="lambda-visitor-rules"
Lambda parameters (LambdaVariable matching a param of the enclosing Lambda)
  are lambda-locals: never add them to outer required-column sets.
Outer columns captured inside a lambda body ARE real dependencies:
  they must appear in required-column sets (execution snapshots them into the
  LambdaArgument's captures RecordBatch).
A visitor that naively treats LambdaVariable("v") as Column("v") will demand a
  nonexistent column "v" from the input schema and break projection pushdown.
```

## C1.20.5 Optimizer-facing hooks

```text id="hof-optimizer-hooks"
short_circuits() -> true      if some lambda arguments may not be evaluated
                              (guards against CSE hoisting side-effect-visible
                              subexpressions such as division).
conditional_arguments(args)   refine which arguments are eager vs lazy when
                              short_circuits() is true.
clear_null_values() -> true   (default) list arguments get non-empty null
                              sublists cleaned before invoke; return false only
                              if you handle null sublists yourself.
Volatility                    declared in HigherOrderSignature; a Volatile
                              function (or a lambda body containing volatile
                              functions) blocks constant folding as usual.
```

## C1.20.6 Correctness matrix

Test every higher-order function against this matrix (extends the C2/C7 matrices with the lambda-specific axes):

| Case | Expected behavior to pin |
|---|---|
| NULL array (row-level null) | output row is NULL (or documented alternative) |
| NULL elements inside array | lambda sees nullable parameter; nulls propagate per body semantics |
| empty array `[]` | empty output array / `false` for any-match; never an error |
| captured outer column NULL | capture propagates NULL into body for every element of that row |
| nested lambdas (lambda inside lambda body) | inner lambda resolves against inner parameters first, then outer, then table columns |
| `List` vs `LargeList` input | both accepted (or explicitly coerced/rejected in `coerce_value_types`) |
| view types (`ListView` input, `Utf8View` elements) | coerced per `coerce_value_types`; element kernels must not assume `Utf8` |
| zero-row batch | returns zero-row output; no panic |
| volatility | `random()` in lambda body re-evaluates per element; function's own volatility declared honestly |
| lambda arity mismatch in SQL | plan-time error, not runtime panic |

---

# DataFusion Advanced — C2) Calculation lifecycle and invariants

## C2.0 Objective

Convert user-defined calculations from **ad hoc registration code** into **governed lifecycle artifacts**:

```text id="d8g4ua"
semantic intent
  → calculation placement
  → function/calc manifest
  → signature + coercion contract
  → return field + nullability + metadata contract
  → Arrow-vectorized implementation
  → registry/catalog integration
  → SQL/DataFrame invocation helpers
  → docs + examples + version metadata
  → authorization policy
  → tests + benchmarks + observability
  → deployment gates
  → deprecation / migration / removal
```

The attachment already frames DataFusion’s public extension surface around SQL, DataFrame, expressions, logical/physical plans, optimizer, custom table providers, UDF/UDAF/UDWF/UDTF functions, custom SQL syntax, custom logical/physical nodes, configuration, profiling, and deployment patterns.  This chapter adds the missing lifecycle model: **calculation artifacts, invariants, validation gates, and deployment discipline**.

---

## C2.1 Lifecycle control plane

```text id="s7gpnq"
CalculationSpec
  ├─ semantic intent
  ├─ placement decision
  ├─ function family
  ├─ public name / aliases
  ├─ signature / coercion
  ├─ return field / nullability / metadata
  ├─ implementation binding
  ├─ registry binding
  ├─ SQL/DataFrame helper binding
  ├─ docs binding
  ├─ version/deprecation binding
  ├─ security policy
  ├─ test manifest
  ├─ benchmark profile
  └─ deployment status

Runtime registration
  ├─ SessionContext
  ├─ SessionState
  ├─ scalar function registry
  ├─ aggregate function registry
  ├─ window function registry
  ├─ table function registry / table provider
  └─ tenant/function allowlist

Execution
  ├─ planning-time resolution
  ├─ logical expression / aggregate / window / table-function node
  ├─ physical expression / accumulator / partition evaluator / table scan
  ├─ Arrow RecordBatch / ArrayRef / ScalarValue
  └─ metrics / diagnostics / audit events
```

DataFusion scalar UDFs are logical functions that produce one output row for each input row and carry the name, type signature, return type, and implementation DataFusion needs to plan and invoke the function. ([Docs.rs][1]) Advanced scalar UDF implementations expose required methods for `name`, `signature`, `return_type`, and `invoke_with_args`, plus optional aliases, coercion, documentation, ordering, constraint, and simplification hooks. ([Docs.rs][2])

---

## C2.2 Lifecycle state machine

```text id="syquab"
DRAFT
  → SPECIFIED
  → IMPLEMENTED
  → REGISTERED
  → DOCUMENTED
  → AUTHORIZED
  → TESTED
  → PROFILED
  → RELEASE_CANDIDATE
  → ACTIVE
  → DEPRECATED
  → DISABLED
  → REMOVED
```

| State               | Entry criteria                                                | Exit gate                                |
| ------------------- | ------------------------------------------------------------- | ---------------------------------------- |
| `DRAFT`             | semantic need identified                                      | `CalculationSpec` skeleton exists        |
| `SPECIFIED`         | placement, inputs, outputs, null policy, volatility specified | reviewer/agent validates placement       |
| `IMPLEMENTED`       | Rust implementation compiles                                  | unit tests pass                          |
| `REGISTERED`        | registration function added to package                        | SQL/DataFrame planning resolves function |
| `DOCUMENTED`        | docs metadata + examples + manifest generated                 | doc snapshot approved                    |
| `AUTHORIZED`        | security policy assigned                                      | policy tests pass                        |
| `TESTED`            | direct/SQL/DataFrame/null/error tests pass                    | CI green                                 |
| `PROFILED`          | benchmark or cost classification recorded                     | cost class approved                      |
| `RELEASE_CANDIDATE` | changelog + version + migration notes ready                   | deployment approval                      |
| `ACTIVE`            | available in allowed contexts                                 | runtime metrics monitored                |
| `DEPRECATED`        | replacement exists, warning/migration plan active             | usage below removal threshold            |
| `DISABLED`          | no longer callable in policy                                  | stored plans/views audited               |
| `REMOVED`           | implementation/registration removed                           | compatibility tombstone retained         |

Agent invariant:

```text id="eq54ui"
A calculation is not production-ready when it merely compiles.
A calculation is production-ready only when its manifest, registration, policy, tests, docs, and deployment gates agree.
```

---

## C2.3 Artifact graph

```text id="n9k3pw"
CalculationSpec
  ├─ PlacementSpec
  ├─ FunctionManifest
  │    ├─ SignatureSpec
  │    ├─ ReturnFieldSpec
  │    ├─ NullPolicy
  │    ├─ VolatilityPolicy
  │    ├─ CoercionPolicy
  │    ├─ DocumentationSpec
  │    ├─ VersionSpec
  │    └─ DeprecationSpec
  ├─ ImplementationSpec
  │    ├─ RustSymbol
  │    ├─ FeatureFlag
  │    ├─ PackagePath
  │    └─ RegistrationHook
  ├─ InvocationSpec
  │    ├─ SQL examples
  │    ├─ DataFrame helper
  │    └─ Expr helper
  ├─ TestManifest
  ├─ BenchmarkManifest
  ├─ SecurityPolicy
  ├─ ObservabilityPolicy
  └─ DeploymentPolicy
```

Minimum artifact set for nontrivial UDF/UDAF/UDWF/UDTF:

```text id="fcliqu"
CalculationSpec        required
FunctionManifest       required
SignatureSpec          required
ReturnFieldSpec        required
NullPolicy             required
VolatilityPolicy       required
TestManifest           required
SecurityPolicy         required
DocumentationSpec      required
VersionSpec            required
BenchmarkManifest      required if hot path / external I/O / high cost
DeprecationSpec        required once public
```

---

# C2.4 Lifecycle phase deep dive

## C2.4.1 Define semantic intent

### Purpose

Convert user wording into a stable semantic contract before selecting DataFusion implementation machinery.

### Required fields

```yaml id="rwf4u0"
semantic_intent:
  business_name: weighted_sulfur
  domain: refinery.blending.quality
  purpose: Compute volume-weighted sulfur content for a blend group.
  calculation_semantics: sum(sulfur_wt_pct * volume_bbl) / sum(volume_bbl)
  output_meaning: Blend sulfur content on volume-weighted basis.
  unit: wt_pct
  basis: volume_bbl
  invalid_input_policy:
    null_sulfur: skip_row
    null_volume: skip_row
    zero_total_volume: return_null
  reference_methodology:
    name: internal_blend_quality_v1
    version: 2026.05
```

### Agent rules

```text id="sk9dvg"
Define intent before code.
Do not infer null policy from implementation convenience.
Do not infer units from column names alone.
Do not decide scalar/UDAF/UDWF before cardinality is explicit.
Capture business replacement semantics before alias/deprecation decisions.
```

---

## C2.4.2 Choose calculation class

### Placement record

```yaml id="e32jru"
placement:
  selected: UDAF
  cardinality: scalar_per_group
  evaluation_phase: aggregate_state_update
  rejected:
    Expr: Requires cross-row grouped state.
    scalar_udf: Would hide grouped state and violate row-wise output invariant.
    udwf: Output is one row per group, not one row per input row.
    udtf: Does not generate relation.
    preprocessing: Must remain ad hoc by query grouping.
```

### Valid classes

```text id="lb29ir"
Expr
built_in_function
scalar_udf
async_scalar_udf
udaf
udwf
udtf
custom_sql_planner_extension
custom_logical_node
custom_physical_operator
table_provider
external_preprocessing
```

### Gate

```text id="onsf5e"
Every non-Expr user-defined calculation must record:
  why built-ins were insufficient
  why Expr composition was insufficient
  why selected class matches cardinality
  why lower-complexity alternatives were rejected
```

---

## C2.4.3 Define public name, aliases, signature, parameter names

### FunctionManifest

```yaml id="ihgt8z"
function:
  canonical_name: weighted_sulfur
  aliases:
    - blend_sulfur
    - wt_avg_sulfur
  family: aggregate_udf
  sql_case_policy: lowercase_snake_case
  stability: stable
  named_parameters:
    - sulfur_wt_pct
    - volume_bbl
```

### SignatureSpec

```yaml id="vfgwmp"
signature:
  mode: exact
  input_types:
    - Float64
    - Float64
  variadic: false
  named_arguments_supported: true
  coercion:
    enabled: true
    accepted_source_types:
      sulfur_wt_pct: [Float32, Float64, Decimal128]
      volume_bbl: [Float32, Float64, Decimal128]
    canonical_types:
      sulfur_wt_pct: Float64
      volume_bbl: Float64
```

### Rust shape: scalar UDF

```rust id="pcgwp0"
#[derive(Debug)]
pub struct NormalizeApiGravity {
    signature: Signature,
    aliases: Vec<String>,
}

impl NormalizeApiGravity {
    pub fn new() -> Self {
        Self {
            signature: Signature::uniform(
                1,
                vec![DataType::Float64],
                Volatility::Immutable,
            ),
            aliases: vec!["normalize_api".to_string()],
        }
    }
}
```

### Hard invariant

```text id="gnl5im"
signature() must describe exactly what the implementation can evaluate after DataFusion coercion.
coerce_types() must return exactly one output type per input argument.
aliases must not include canonical name.
aliases must be part of authorization policy.
```

`ScalarUDFImpl::signature` defines accepted argument types and volatility, while `coerce_types` can be used for user-defined type coercion only when the signature uses `TypeSignature::UserDefined`; DataFusion casts function arguments to the returned types. ([Docs.rs][2])

---

## C2.4.4 Define return type, nullability, and metadata

### ReturnFieldSpec

```yaml id="vwr5t7"
return_field:
  name: weighted_sulfur_wt_pct
  data_type: Float64
  nullable: true
  metadata:
    unit: wt_pct
    basis: volume_weighted
    semantic_type: quality_metric
  inference:
    mode: static
    depends_on_arg_types: false
    depends_on_arg_fields: false
```

### Static return type: simple scalar UDF

```rust id="itkr1o"
fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
    Ok(DataType::Float64)
}
```

### Dynamic return field: advanced UDF

```rust id="af6cdm"
fn return_field_from_args(
    &self,
    args: ReturnFieldArgs,
) -> Result<FieldRef> {
    // Use input field nullability/metadata to produce output Field.
    // Prefer explicit DataFusionError over unreachable!/panic.
    let mut metadata = HashMap::new();
    metadata.insert("unit".to_string(), "wt_pct".to_string());
    metadata.insert("semantic_type".to_string(), "quality_metric".to_string());

    Ok(Arc::new(
        Field::new("weighted_sulfur_wt_pct", DataType::Float64, true)
            .with_metadata(metadata)
    ))
}
```

### Hard invariant

```text id="r583jt"
return_type() / return_field_from_args() must match actual output arrays.
nullable=false implies implementation never emits nulls.
metadata must be deterministic for same input schema.
dynamic return field logic must not inspect data values unless API explicitly permits/plans for that behavior.
```

`ScalarUDFImpl::return_type` receives argument data types after signature-compatible coercion, and DataFusion will not call `return_type` if `return_field_from_args` is implemented; the docs recommend returning a `DataFusionError::Internal` instead of panicking if an unexpected path is reached. ([Docs.rs][2])

---

## C2.4.5 Define null policy

### NullPolicy

```yaml id="cyzbwr"
null_policy:
  mode: skip_null_pairs
  scalar_behavior:
    any_null_input: return_null
  aggregate_behavior:
    null_value: skip_row
    null_weight: skip_row
    all_rows_skipped: return_null
  window_behavior:
    null_frame: return_null
    empty_frame: return_null
  diagnostics:
    count_skipped_rows: false
```

### Standard modes

| Mode                      | Behavior                                    | Use                      |
| ------------------------- | ------------------------------------------- | ------------------------ |
| `strict_null_propagation` | any null input -> null output               | row-wise pure transforms |
| `skip_nulls`              | ignore null values                          | aggregates like average  |
| `skip_null_pairs`         | ignore row if any required pair member null | weighted metrics         |
| `null_as_default`         | null -> configured default                  | compatibility functions  |
| `error_on_null`           | null input -> execution error               | strict validation UDFs   |
| `preserve_null_structure` | nested nulls preserved                      | struct/list transforms   |
| `try_mode_null_on_error`  | invalid input -> null                       | `try_*`-style functions  |

### Agent rules

```text id="hp01tu"
Null policy is part of public semantics.
Do not bury null policy inside row loop.
Test null policy independently from happy path.
Separate null input from invalid non-null input.
Separate empty aggregate/window frame from all-null input.
```

---

## C2.4.6 Define volatility policy

### VolatilityPolicy

```yaml id="kw1422"
volatility_policy:
  datafusion_volatility: Immutable
  determinism: deterministic
  external_state: none
  time_dependent: false
  random: false
  query_snapshot_required: false
  cacheable: true
```

### Standard volatility mapping

| True behavior                                 | DataFusion volatility | Examples                                            |
| --------------------------------------------- | --------------------- | --------------------------------------------------- |
| same args always same output                  | `Immutable`           | arithmetic formula, deterministic parser            |
| same args same within query / stable snapshot | `Stable`              | reference table snapshot, query timestamp           |
| may change per call                           | `Volatile`            | random, live external API, nondeterministic service |

### Hard invariant

```text id="k9ggdh"
Volatility must match reality, not desired optimization behavior.
Never mark external live lookup Immutable unless lookup version/snapshot is part of arguments.
Never mark random/time/current-state functions Immutable.
```

DataFusion function signatures include volatility, and scalar UDF signatures expose both accepted argument types and volatility to planning. ([Docs.rs][2])

---

## C2.4.7 Implement Arrow vectorization

### ImplementationSpec

```yaml id="za1v5l"
implementation:
  rust_type: refinery_calc::functions::NormalizeApiGravity
  package: refinery_calc
  feature_flag: engineering_quality
  scalar_fast_path: true
  array_vectorized_path: true
  supports_dictionary: false
  supports_utf8view: false
  external_io: false
  unsafe_code: false
```

### Scalar UDF implementation skeleton

```rust id="cz9c1o"
use std::sync::Arc;

use datafusion::arrow::array::{Array, ArrayRef, Float64Array, Float64Builder};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{DataFusionError, Result, ScalarValue};
use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDFImpl, Signature, Volatility,
};

#[derive(Debug)]
pub struct NormalizeApiGravity {
    signature: Signature,
}

impl NormalizeApiGravity {
    pub fn new() -> Self {
        Self {
            signature: Signature::uniform(
                1,
                vec![DataType::Float64],
                Volatility::Immutable,
            ),
        }
    }
}

impl ScalarUDFImpl for NormalizeApiGravity {
    fn name(&self) -> &str {
        "normalize_api_gravity"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(DataType::Float64)
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let [arg] = args.args.as_slice() else {
            return Err(DataFusionError::Execution(
                "normalize_api_gravity expects exactly one argument".to_string(),
            ));
        };

        match arg {
            ColumnarValue::Scalar(ScalarValue::Float64(v)) => {
                Ok(ColumnarValue::Scalar(ScalarValue::Float64(
                    v.map(normalize_api_formula),
                )))
            }
            ColumnarValue::Array(array) => {
                let input = array
                    .as_any()
                    .downcast_ref::<Float64Array>()
                    .ok_or_else(|| DataFusionError::Execution(
                        format!("expected Float64Array, got {:?}", array.data_type())
                    ))?;

                let mut builder = Float64Builder::with_capacity(input.len());

                for i in 0..input.len() {
                    if input.is_null(i) {
                        builder.append_null();
                    } else {
                        builder.append_value(normalize_api_formula(input.value(i)));
                    }
                }

                Ok(ColumnarValue::Array(Arc::new(builder.finish()) as ArrayRef))
            }
            other => Err(DataFusionError::Execution(format!(
                "unsupported argument for normalize_api_gravity: {other:?}"
            ))),
        }
    }
}

fn normalize_api_formula(api: f64) -> f64 {
    api
}
```

### Hard invariants

```text id="uxgiq4"
Scalar array output length == input row count.
Scalar output type == declared return type.
No panic on wrong type, null, empty array, scalar input, or unexpected argument count.
ColumnarValue::Scalar path must be correct.
Array path must preserve row alignment.
Error path must return DataFusionError, not unwrap/expect/panic.
```

`ScalarUDFImpl::invoke_with_args` returns a `ColumnarValue`; the docs recommend handling constant arguments represented as `ColumnarValue::Scalar` for best performance rather than always expanding scalars to arrays. ([Docs.rs][2])

DataFusion 54 removed the `fn as_any(&self) -> &dyn Any { self }` boilerplate from `ScalarUDFImpl` / `AggregateUDFImpl` / `WindowUDFImpl` / `TableFunctionImpl`: the traits are now declared as `Debug + DynEq + DynHash + Send + Sync + Any` (`datafusion-expr` `udf.rs:524`, `udaf.rs:445`, `udwf.rs:315`), so `Any` is a supertrait and no method needs to be written. Downcasting a registered impl back to its concrete type uses trait upcasting via the provided helpers on the trait object: `udf.inner().downcast_ref::<NormalizeApiGravity>()` (and `.is::<T>()`), which work through `Arc<dyn ScalarUDFImpl>` auto-deref. Note this is unrelated to Arrow *array* downcasts inside kernel bodies — `array.as_any().downcast_ref::<Float64Array>()` is ordinary Arrow API and is unchanged.

---

## C2.4.8 Register into context/session/catalog

### RegistrationSpec

```yaml id="yikjw6"
registration:
  package: refinery_calc
  hook: register_refinery_calculations
  default_enabled: false
  feature_flag: engineering_quality
  registries:
    scalar:
      - normalize_api_gravity
    aggregate:
      - weighted_sulfur
    window:
      - rolling_stability_score
    table:
      - scenario_parameters
```

### Rust registration pattern

```rust id="lxri7p"
use datafusion::prelude::*;
use datafusion::logical_expr::{ScalarUDF, AggregateUDF, WindowUDF};

pub fn register_refinery_calculations(ctx: &SessionContext) -> datafusion::error::Result<()> {
    register_scalar_functions(ctx)?;
    register_aggregate_functions(ctx)?;
    register_window_functions(ctx)?;
    register_table_functions(ctx)?;
    Ok(())
}

fn register_scalar_functions(ctx: &SessionContext) -> datafusion::error::Result<()> {
    let normalize_api = ScalarUDF::from(NormalizeApiGravity::new());
    ctx.register_udf(normalize_api);
    Ok(())
}

fn register_aggregate_functions(ctx: &SessionContext) -> datafusion::error::Result<()> {
    ctx.register_udaf(weighted_sulfur_udaf());
    Ok(())
}

fn register_window_functions(ctx: &SessionContext) -> datafusion::error::Result<()> {
    ctx.register_udwf(rolling_stability_udwf());
    Ok(())
}

fn register_table_functions(ctx: &SessionContext) -> datafusion::error::Result<()> {
    // ctx.register_udtf(...) depending on the exact DataFusion API surface in pinned version.
    Ok(())
}
```

### Hard invariant

```text id="cmwxsg"
All UDF/UDAF/UDWF/UDTF registrations must occur before SQL/DataFrame planning.
Changing registry after planning does not rewrite already-created plans.
Tenant-specific function policy must be applied before exposing SQL planning.
```

The attachment’s session-state section already records this operational rule: register UDF/UDAF/UDWF before SQL planning and avoid assuming post-planning registry changes affect existing plans. 

---

## C2.4.9 Expose SQL/DataFrame invocation helpers

### InvocationSpec

```yaml id="ttmxwz"
invocation:
  sql:
    examples:
      - "SELECT normalize_api_gravity(api) AS api_norm FROM streams"
  dataframe:
    helper: normalize_api_gravity_expr
  expr:
    canonical_helper: refinery_calc::exprs::normalize_api_gravity
```

### Rust `Expr` helper

```rust id="cofrsw"
use datafusion::logical_expr::{Expr, ScalarUDF};
use datafusion::prelude::*;

pub fn normalize_api_gravity_expr(arg: Expr) -> Expr {
    ScalarUDF::from(NormalizeApiGravity::new())
        .call(vec![arg])
        .alias("api_norm")
}
```

A `ScalarUDF` exposes `call` to create an `Expr` invoking the UDF. ([Docs.rs][1]) The DataFusion Expr guide shows creating a UDF and then producing expressions such as `add_one_udf.call(vec![lit(5)])` and `add_one_udf.call(vec![col("my_column")])`. ([Apache DataFusion][3])

### Agent rules

```text id="tbxdt6"
Every public calculation should have:
  SQL example
  DataFrame/Expr helper
  schema/type example
  null example
  error example if validation can fail

Do not force agents to hand-write string SQL for known calculations.
```

---

## C2.4.10 Document and version

### DocumentationSpec

```yaml id="ihnnkf"
documentation:
  summary: Normalize API gravity value under domain convention v1.
  category: refinery.quality
  arguments:
    - name: api
      type: Float64
      description: API gravity.
      nullable: true
  returns:
    type: Float64
    nullable: true
    description: Normalized API gravity.
  examples:
    sql:
      - "SELECT normalize_api_gravity(api) FROM streams"
    dataframe:
      - "normalize_api_gravity_expr(col(\"api\"))"
  null_policy_ref: strict_null_propagation
  volatility_ref: immutable
```

### VersionSpec

```yaml id="l7lx14"
version:
  function_version: 1.2.0
  methodology_version: normalize_api_v1
  introduced_in: calc_pkg_0.4.0
  datafusion_min: 54.1.0
  datafusion_max_tested: 54.1.0
  arrow_min: 58.4.0
  breaking_change: false
```

### DeprecationSpec

```yaml id="9cu462"
deprecation:
  status: active
  replacement: null
  deprecated_since: null
  removal_not_before: null
  migration_sql: null
```

### Agent rules

```text id="5k5qyz"
Function name is API.
Parameter order is API.
Parameter names are API if named args are exposed.
Return type/nullability are API.
Null behavior is API.
Volatility is API.
Aliases are API.
Examples are tests.
```

`ScalarUDFImpl::documentation` returns documentation for the UDF, and DataFusion notes that documentation can be accessed programmatically and used to generate public-facing documentation. ([Docs.rs][2])

---

## C2.4.11 Authorize

### SecurityPolicy

```yaml id="m9rajo"
security:
  default_allowed: false
  allowed_contexts:
    - internal_batch
    - refinery_modeling
  denied_contexts:
    - public_sql
  risk_class: pure_deterministic
  external_io: false
  side_effects: false
  volatility_allowed_in_reports: true
  max_cost_class: low
  requires_tenant_scope: false
  aliases_in_policy:
    - normalize_api_gravity
    - normalize_api
```

### Risk classes

| Risk class              | Examples                          | Default public SQL policy |
| ----------------------- | --------------------------------- | ------------------------- |
| `pure_deterministic`    | arithmetic/domain formula         | allow if tested           |
| `expensive_cpu`         | vector distance over large arrays | allow with quota          |
| `volatile`              | random/current/live state         | restrict                  |
| `external_read`         | async lookup                      | deny by default           |
| `external_write`        | side effects                      | not UDF                   |
| `cardinality_expanding` | UDTF/unnest-like                  | restrict                  |
| `diagnostic_metadata`   | cache/metadata introspection      | admin only                |
| `unsafe_native`         | FFI/plugin                        | internal only             |

### Plan validation sketch

```rust id="954ivl"
pub fn authorize_function_call(
    function_name: &str,
    tenant_policy: &TenantFunctionPolicy,
) -> datafusion::error::Result<()> {
    if tenant_policy.is_function_allowed(function_name) {
        Ok(())
    } else {
        Err(datafusion::error::DataFusionError::Plan(format!(
            "function `{function_name}` is not allowed in this context"
        )))
    }
}
```

### Agent rules

```text id="hdou55"
Registration is not authorization.
Aliases must be authorized.
Volatile functions require policy.
External I/O functions require policy + resource limits.
Public SQL should use allowlists, not deny-by-default assumptions hidden in code.
```

The attachment’s security testing matrix already includes function-policy tests for allowed functions, denied functions, aliases, volatile/expensive function policy, resource limits, and plan validation. 

---

## C2.4.12 Test

### TestManifest

```yaml id="my33os"
tests:
  direct_arrow:
    empty_array: true
    one_row: true
    all_null: true
    mixed_null: true
    scalar_input: true
    wrong_type: true
  sql:
    invocation: true
    alias_invocation: true
    type_error: true
    null_behavior: true
  dataframe:
    expr_helper: true
    schema: true
  optimizer:
    explain_snapshot: false
    simplification_equivalence: false
  aggregate:
    update_state_merge_evaluate: false
  window:
    frame_order_partition: false
  policy:
    allowed_context: true
    denied_context: true
```

### Scalar UDF test skeleton

```rust id="gi6gww"
#[tokio::test]
async fn normalize_api_sql_happy_path() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();
    register_refinery_calculations(&ctx)?;

    let batches = ctx
        .sql("SELECT normalize_api_gravity(42.0) AS api_norm")
        .await?
        .collect()
        .await?;

    assert_batches_eq!(
        &[
            "+----------+",
            "| api_norm |",
            "+----------+",
            "| 42.0     |",
            "+----------+",
        ],
        &batches
    );

    Ok(())
}
```

### Direct implementation tests

```rust id="uurtiw"
#[test]
fn normalize_api_preserves_null_scalar() -> datafusion::error::Result<()> {
    let udf = NormalizeApiGravity::new();

    let args = ScalarFunctionArgs {
        args: vec![ColumnarValue::Scalar(ScalarValue::Float64(None))],
        // fill fields according to pinned DataFusion version
        ..test_scalar_function_args()
    };

    let out = udf.invoke_with_args(args)?;

    assert_eq!(
        out,
        ColumnarValue::Scalar(ScalarValue::Float64(None))
    );

    Ok(())
}
```

### Required test classes by family

| Family                  | Required tests                                                                   |
| ----------------------- | -------------------------------------------------------------------------------- |
| scalar UDF              | scalar input, array input, nulls, empty array, wrong type, SQL, DataFrame helper |
| async scalar UDF        | timeout, cancellation, dedup/cache, remote error, concurrency, nulls             |
| UDAF                    | update, state, merge, evaluate, partition equivalence, empty/all-null group      |
| UDWF                    | partition, order, frame bounds, empty frame, all-null frame, tie order           |
| UDTF                    | argument validation, schema, bounded cardinality, pushdown behavior              |
| custom logical/physical | schema, EXPLAIN, physical lowering, execution, metrics, cancellation             |

---

## C2.4.13 Profile

### BenchmarkManifest

```yaml id="ccdj86"
benchmark:
  required: true
  reason: hot_path_function
  datasets:
    - name: small_batch
      rows: 1024
    - name: default_batch
      rows: 8192
    - name: large_batch
      rows: 1048576
    - name: null_heavy
      rows: 8192
      null_fraction: 0.75
  metrics:
    - rows_per_second
    - ns_per_row
    - allocations_per_batch
    - output_null_count
    - scalar_fast_path_latency
  baselines:
    - expr_equivalent
    - python_reference
```

### Rust benchmark placement

```text id="7fzedr"
benches/
  normalize_api_bench.rs
  weighted_sulfur_bench.rs
  rolling_stability_bench.rs
```

### Agent rules

```text id="jirvzk"
Benchmark release builds only.
Benchmark scalar and array paths separately.
Benchmark null-heavy data.
Benchmark wrong-type planning path only as validation, not throughput.
Record batch size.
Record DataFusion + Arrow version.
```

---

## C2.4.14 Deploy

### DeploymentPolicy

```yaml id="og0fst"
deployment:
  package: refinery_calc
  feature_flag: engineering_quality
  default_enabled: false
  rollout:
    stage: canary
    tenants:
      - internal_refinery_modeling
  gates:
    tests_passed: true
    benchmark_recorded: true
    docs_generated: true
    security_policy_approved: true
    migration_notes_written: true
```

### Engine construction gate

```rust id="wjncll"
pub fn build_engine(policy: &TenantPolicy) -> datafusion::error::Result<SessionContext> {
    let ctx = configured_context(policy)?;

    register_core_sources(&ctx, policy)?;
    register_builtin_compatible_functions(&ctx)?;

    if policy.features.engineering_quality {
        register_refinery_calculations(&ctx)?;
    }

    validate_registered_functions_against_policy(&ctx, policy)?;

    Ok(ctx)
}
```

### Agent rules

```text id="r740em"
No manifest → no registration.
No policy → no public availability.
No tests → no active deployment.
No version pin → no extension API stability claim.
No migration note → no breaking change.
```

---

## C2.4.15 Deprecate

### Deprecation lifecycle

```text id="2wdl1x"
ACTIVE
  → DEPRECATED_VISIBLE
  → DEPRECATED_WARN
  → DISABLED_BY_DEFAULT
  → REMOVED_FROM_REGISTRY
  → TOMBSTONE_ONLY
```

### DeprecationSpec

```yaml id="tn7gd6"
deprecation:
  status: deprecated_warn
  deprecated_since: 1.5.0
  replacement: normalized_api_gravity_v2
  removal_not_before: 2.0.0
  reason: Corrected pressure basis metadata and null handling.
  migration:
    sql_rewrite:
      from: "normalize_api_gravity(api)"
      to: "normalized_api_gravity_v2(api, 'standard_basis')"
    dataframe_rewrite:
      from: "normalize_api_gravity_expr(col(\"api\"))"
      to: "normalized_api_gravity_v2_expr(col(\"api\"), lit(\"standard_basis\"))"
```

### Agent rules

```text id="zn7zs4"
Never silently change semantics of a stable function.
Add new function/version for semantic changes.
Keep alias only when behavior is identical.
Deprecation must include replacement and migration examples.
Keep tombstone metadata after removal for query/view migration diagnostics.
```

---

# C2.5 Hard invariants

## C2.5.1 Registration-before-planning invariant

```text id="wxi6ly"
Function registry must contain UDF/UDAF/UDWF/UDTF before SQL/DataFrame planning resolves function calls.
```

### Failure mode

```rust id="ag8mcc"
let ctx = SessionContext::new();

let df = ctx.sql("SELECT normalize_api_gravity(api) FROM streams").await?;
// Too late:
register_refinery_calculations(&ctx)?;
```

### Correct

```rust id="tmrgp0"
let ctx = SessionContext::new();

register_refinery_calculations(&ctx)?;

let df = ctx.sql("SELECT normalize_api_gravity(api) FROM streams").await?;
```

### Test

```rust id="yl4pmy"
#[tokio::test]
async fn udf_must_be_registered_before_planning() {
    let ctx = SessionContext::new();

    let err = ctx
        .sql("SELECT normalize_api_gravity(42.0)")
        .await
        .expect_err("unregistered function should fail during planning");

    assert!(err.to_string().contains("normalize_api_gravity"));
}
```

---

## C2.5.2 Signature/coercion agreement invariant

```text id="n5drmr"
signature + coerce_types + implementation downcasts must agree.
```

### Bad

```text id="rb8fw1"
signature accepts Float64
coerce_types returns Float64
implementation downcasts Int64Array
```

### Good

```text id="n6fu41"
signature accepts numeric
coerce_types canonicalizes to Float64
implementation downcasts Float64Array
return type Float64
```

### Validation

```text id="oelvdx"
[ ] accepted argument types plan successfully
[ ] coercible argument types cast successfully
[ ] unsupported argument types fail at planning
[ ] implementation downcast cannot fail after planning/coercion
[ ] error path still returns DataFusionError if mismatch occurs
```

---

## C2.5.3 Volatility truth invariant

```text id="tepkx6"
Volatility must model actual output stability.
```

### Bad

```text id="ultn29"
live_market_price(symbol) marked Immutable
random_sample() marked Stable
now_string() marked Immutable
```

### Good

```text id="fi9xfb"
unit_conversion(x)                         → Immutable
reference_price(symbol, snapshot_id)       → Immutable if snapshot_id fixes data
reference_price(symbol) with query snapshot → Stable
live_market_price(symbol)                  → Volatile / async restricted
```

---

## C2.5.4 Scalar output row-count invariant

```text id="svkmk9"
For scalar UDF array inputs:
  output_array.len() == logical input row count
```

### Validation helper

```rust id="z1ghj8"
pub fn validate_scalar_output_len(
    function_name: &str,
    expected_rows: usize,
    output: &ColumnarValue,
) -> datafusion::error::Result<()> {
    match output {
        ColumnarValue::Array(array) if array.len() == expected_rows => Ok(()),
        ColumnarValue::Scalar(_) => Ok(()),
        ColumnarValue::Array(array) => Err(datafusion::error::DataFusionError::Execution(
            format!(
                "{function_name} returned {} rows, expected {expected_rows}",
                array.len()
            ),
        )),
    }
}
```

---

## C2.5.5 UDAF state schema invariant

```text id="mvl6u1"
UDAF declared state_type must match Accumulator::state() output and Accumulator::merge_batch() input.
```

`create_udaf` creates a UDAF with a signature, state type, return type, accumulator factory, and explicitly states that the signature and state type must match the accumulator implementation. ([Docs.rs][4]) The `Accumulator` trait requires `update_batch`, `evaluate`, `size`, `state`, and `merge_batch`; DataFusion uses `state()` output as intermediate arrays and combines them through `merge_batch()` during multi-phase grouping. ([Docs.rs][5])

### Weighted average state

```text id="xmv7r2"
state_type:
  - Float64   # weighted_sum
  - Float64   # weight_sum

state():
  [ScalarValue::Float64(weighted_sum), ScalarValue::Float64(weight_sum)]

merge_batch(states):
  states[0] = weighted_sum array
  states[1] = weight_sum array
```

### Required tests

```text id="nbrzqy"
[ ] state vector length == state_type length
[ ] each state ScalarValue type == corresponding state_type
[ ] merge_batch accepts arrays constructed from state()
[ ] update → state → merge → evaluate equals single-pass evaluate
[ ] multi-partition aggregate equals single-partition aggregate within tolerance
```

---

## C2.5.6 UDAF evaluate non-consuming invariant

```text id="iaqowx"
Accumulator::evaluate() must not consume internal state.
```

The `Accumulator::evaluate` documentation states that it must not consume internal state because accumulators may be used in window aggregate functions where `evaluate` can be executed multiple times for the current frame. ([Docs.rs][5])

### Bad

```rust id="v3sdzo"
fn evaluate(&mut self) -> Result<ScalarValue> {
    let values = std::mem::take(&mut self.values);
    Ok(compute(values))
}
```

### Good

```rust id="86de0p"
fn evaluate(&mut self) -> Result<ScalarValue> {
    Ok(compute_from_ref(&self.values))
}
```

---

## C2.5.7 UDAF memory accounting invariant

```text id="ccmntx"
Accumulator::size() must estimate allocated state memory.
```

The accumulator `size()` method is used to calculate memory used during execution so DataFusion can stay within its memory limit; for internal containers, capacity rather than length should be used. ([Docs.rs][5])

### Agent rule

```text id="ou70ml"
If UDAF stores Vec/String/HashMap/sketch state:
  size() must include capacity-backed allocation.
```

---

## C2.5.8 GroupsAccumulator invariant

```text id="mxhwm1"
GroupsAccumulator state is indexed by contiguous group_index values.
evaluate/state output must emit rows in group_index order.
```

`GroupsAccumulator` stores state for all groups internally, receives `group_indices`, optional filter arrays, and total group count; `evaluate` and `state` must return values in group-index order, and `size()` should be O(n), not O(num_groups), because it is called once per batch. ([Docs.rs][6])

### Use when

```text id="h1gb0g"
high cardinality groups
aggregate is hot path
Accumulator implementation is correct and benchmarked
performance gain justifies complexity
```

---

## C2.5.9 UDWF partition/frame/order invariant

```text id="refn5h"
UDWF must respect:
  PARTITION BY
  ORDER BY
  window frame
  null ordering
  frame emptiness
  batch boundaries
```

A `WindowUDF` is a logical user-defined window function invoked with SQL `OVER`; it differs from a scalar UDF because it is stateful across batches and uses a `PartitionEvaluator`. ([Docs.rs][7]) The simple `create_udwf` path creates a UDWF with signature, return type, volatility, and a `PartitionEvaluator` factory, and the signature/state expectations must match the evaluator implementation. ([Docs.rs][8])

### Required tests

```text id="my2lql"
[ ] one partition
[ ] multiple partitions
[ ] ORDER BY ascending
[ ] ORDER BY descending if supported
[ ] ties
[ ] ROWS bounded frame
[ ] ROWS unbounded frame
[ ] empty frame
[ ] all-null frame
[ ] frame crossing RecordBatch boundary
```

---

## C2.5.10 UDTF planning-cost invariant

```text id="susatl"
UDTF planning must validate arguments and produce schema/provider.
UDTF planning must not perform expensive scans or unbounded external work.
```

### Required planning behavior

```text id="d4eokl"
validate argument count/type/literal-ness
reject unbounded cardinality
construct schema deterministically
construct TableProvider cheaply
defer actual data production to scan/execution
```

### Bad

```text id="xqfx5u"
scenario_results('case_1') planning:
  connects to remote service
  downloads all results
  infers schema from full data
```

### Good

```text id="au5o82"
scenario_results('case_1') planning:
  validates case_id
  loads cached metadata/schema
  creates provider
  scans lazily at execution with projection/filter/limit
```

---

# C2.6 Artifact schemas

## C2.6.1 `CalculationSpec`

```yaml id="nbd538"
calculation_spec:
  id: refinery.weighted_sulfur.v1
  lifecycle_state: active
  semantic_intent:
    business_name: weighted_sulfur
    domain: refinery.blending.quality
    purpose: Volume-weighted sulfur content.
    formula: sum(sulfur_wt_pct * volume_bbl) / sum(volume_bbl)
  placement:
    selected: UDAF
    cardinality: scalar_per_group
    evaluation_phase: aggregate_state_update
  function_manifest_ref: refinery.weighted_sulfur.function.v1
  security_policy_ref: policy.internal_deterministic.v1
  test_manifest_ref: tests.weighted_sulfur.v1
  benchmark_manifest_ref: benches.weighted_sulfur.v1
```

### Rust model

```rust id="t1w856"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CalculationSpec {
    pub id: String,
    pub lifecycle_state: LifecycleState,
    pub semantic_intent: SemanticIntent,
    pub placement: PlacementSpec,
    pub function: FunctionManifest,
    pub security: SecurityPolicy,
    pub tests: TestManifest,
    pub benchmark: Option<BenchmarkManifest>,
    pub version: VersionSpec,
    pub deprecation: Option<DeprecationSpec>,
}
```

---

## C2.6.2 `FunctionManifest`

```rust id="lk7t4u"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionManifest {
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub family: FunctionFamily,
    pub signature: SignatureSpec,
    pub return_field: ReturnFieldSpec,
    pub null_policy: NullPolicy,
    pub volatility: VolatilityPolicy,
    pub implementation: ImplementationSpec,
    pub invocation: InvocationSpec,
    pub documentation: DocumentationSpec,
}
```

```rust id="g0ab6p"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FunctionFamily {
    ExprHelper,
    ScalarUdf,
    AsyncScalarUdf,
    AggregateUdf,
    WindowUdf,
    TableUdf,
    TableProvider,
    CustomLogicalNode,
    CustomPhysicalOperator,
    ExternalPreprocessing,
}
```

---

## C2.6.3 `SignatureSpec`

```rust id="slo9a3"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignatureSpec {
    pub mode: SignatureMode,
    pub input_types: Vec<ArrowTypeSpec>,
    pub named_parameters: Vec<ParameterSpec>,
    pub variadic: bool,
    pub coercion: CoercionPolicy,
}
```

```rust id="idwyej"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParameterSpec {
    pub name: String,
    pub data_type: ArrowTypeSpec,
    pub nullable: bool,
    pub semantic_type: Option<String>,
    pub unit: Option<String>,
    pub description: String,
}
```

---

## C2.6.4 `ReturnFieldSpec`

```rust id="x75n9h"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReturnFieldSpec {
    pub name: String,
    pub data_type: ArrowTypeSpec,
    pub nullable: bool,
    pub metadata: BTreeMap<String, String>,
    pub inference: ReturnInferenceMode,
}
```

```rust id="s60r3k"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ReturnInferenceMode {
    Static,
    DependsOnArgTypes,
    DependsOnArgFields,
    DependsOnLiteralValues,
}
```

---

## C2.6.5 `NullPolicy`

```rust id="quul94"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NullPolicy {
    pub mode: NullMode,
    pub scalar_behavior: Option<ScalarNullBehavior>,
    pub aggregate_behavior: Option<AggregateNullBehavior>,
    pub window_behavior: Option<WindowNullBehavior>,
    pub invalid_input_behavior: InvalidInputBehavior,
}
```

```rust id="al24wr"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NullMode {
    StrictNullPropagation,
    SkipNulls,
    SkipNullPairs,
    NullAsDefault,
    ErrorOnNull,
    PreserveNullStructure,
    TryModeNullOnError,
}
```

---

## C2.6.6 `VolatilityPolicy`

```rust id="ax48bq"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VolatilityPolicy {
    pub datafusion_volatility: DataFusionVolatilitySpec,
    pub deterministic: bool,
    pub depends_on_time: bool,
    pub depends_on_randomness: bool,
    pub depends_on_external_state: bool,
    pub query_snapshot_required: bool,
    pub cacheable: bool,
}
```

---

## C2.6.7 `TestManifest`

```rust id="mf2cxs"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TestManifest {
    pub direct_arrow_tests: Vec<TestCaseSpec>,
    pub sql_tests: Vec<TestCaseSpec>,
    pub dataframe_tests: Vec<TestCaseSpec>,
    pub policy_tests: Vec<TestCaseSpec>,
    pub error_tests: Vec<TestCaseSpec>,
    pub property_tests: Vec<PropertyTestSpec>,
    pub golden_tests: Vec<GoldenTestSpec>,
}
```

---

## C2.6.8 `SecurityPolicy`

```rust id="qohfxg"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SecurityPolicy {
    pub default_allowed: bool,
    pub allowed_contexts: Vec<String>,
    pub denied_contexts: Vec<String>,
    pub risk_class: RiskClass,
    pub external_io: bool,
    pub side_effects: bool,
    pub max_cost_class: CostClass,
    pub tenant_scoped: bool,
    pub aliases_included: bool,
}
```

---

# C2.7 Repository layout

```text id="uvoaem"
crates/
  calc-core/
    src/
      lib.rs
      manifest.rs
      lifecycle.rs
      validation.rs
      security.rs
      docs.rs
      registry.rs
  calc-functions/
    src/
      lib.rs
      scalar/
        normalize_api.rs
        density_to_api.rs
      aggregate/
        weighted_sulfur.rs
      window/
        rolling_stability.rs
      table/
        scenario_parameters.rs
      register.rs
  calc-manifests/
    calculations/
      normalize_api_gravity.yaml
      weighted_sulfur.yaml
      rolling_stability_score.yaml
  calc-tests/
    fixtures/
    sql/
    golden/
    property/
  benches/
    normalize_api.rs
    weighted_sulfur.rs
```

Agent rule:

```text id="s0lila"
Never place lifecycle metadata only in prose docs.
Never place registration logic only in main.rs.
Never place function policy only in service code.
The manifest is the source of truth; Rust registration and docs are generated or validated against it.
```

---

# C2.8 Lifecycle gates

## C2.8.1 Specification gate

```text id="gwi7a0"
[ ] semantic intent present
[ ] placement selected
[ ] rejected alternatives recorded
[ ] canonical name valid
[ ] aliases valid and non-conflicting
[ ] signature specified
[ ] return field specified
[ ] null policy specified
[ ] volatility specified
[ ] security risk class assigned
```

## C2.8.2 Implementation gate

```text id="cu0hss"
[ ] compiles under pinned DataFusion version
[ ] uses datafusion::arrow re-exports
[ ] no unwrap/expect on user data
[ ] no panic on wrong type/null/empty
[ ] scalar fast path handled when relevant
[ ] output type equals return field
[ ] output length invariant tested
[ ] DataFusionError returned for invalid inputs
```

## C2.8.3 Registration gate

```text id="oc6ud8"
[ ] register_all package hook includes function
[ ] feature flag controls registration if optional
[ ] aliases registered
[ ] tenant policy includes canonical name and aliases
[ ] registration occurs before SQL planning
[ ] duplicate function-name behavior tested
```

## C2.8.4 Documentation gate

```text id="xccnjw"
[ ] generated docs include signature
[ ] generated docs include return field
[ ] generated docs include null policy
[ ] generated docs include volatility
[ ] generated docs include SQL example
[ ] generated docs include DataFrame helper example
[ ] deprecation status visible
```

## C2.8.5 Security gate

```text id="k34ujg"
[ ] function allowlist includes/denies expected contexts
[ ] aliases covered by same policy
[ ] external I/O risk reviewed
[ ] volatility allowed where used
[ ] cost class allowed where used
[ ] public SQL plan validator tested
```

## C2.8.6 Test gate

```text id="a1v53c"
[ ] direct implementation tests
[ ] SQL tests
[ ] DataFrame tests
[ ] null tests
[ ] wrong-type tests
[ ] empty input tests
[ ] schema tests
[ ] policy tests
[ ] error classification tests
[ ] benchmark if required
```

---

# C2.9 Deployment advisory

## C2.9.1 Internal batch

```text id="d77b8j"
allowed:
  Expr helpers
  scalar UDF
  UDAF
  UDWF
  UDTF
  external preprocessing

required:
  version pin
  manifest
  tests
  benchmark for hot path
```

## C2.9.2 Public SQL endpoint

```text id="pnz267"
allowed:
  explicitly allowlisted pure deterministic functions

restricted:
  volatile functions
  async external I/O functions
  UDTFs with unbounded cardinality
  diagnostic metadata functions
  functions with unknown cost

required:
  sql_with_options
  plan validation
  function allowlist
  result limits
  timeout
```

## C2.9.3 Multi-tenant service

```text id="mk7mtj"
required:
  tenant-scoped SessionContext or policy-bound SessionState
  tenant function allowlist
  tenant-specific external credentials
  cache key includes tenant
  no global mutable function state
```

## C2.9.4 Distributed / Ballista-like execution

```text id="ww6046"
required:
  implementation available on every executor
  accumulator state serializable/mergeable
  deterministic partition equivalence tests
  no process-local hidden state
  version identical across scheduler/executors
```

---

# C2.10 Anti-pattern inventory

* UDF registered in `main.rs` with no manifest.
* UDF name documented in prose but not in machine-readable catalog.
* Alias exists in implementation but not policy.
* Function policy uses canonical name but forgets aliases.
* Function marked `Immutable` while reading external mutable state.
* Scalar UDF used for aggregate/window/table behavior.
* `return_type` says `Float64`; implementation returns `Int64Array`.
* `nullable=false`; implementation emits nulls.
* UDAF `state_type` differs from `state()` output.
* `evaluate()` consumes UDAF state.
* UDAF `size()` ignores heap allocations.
* UDWF ignores `ORDER BY` or frame bounds.
* UDTF downloads full remote table during planning.
* Registration occurs after `ctx.sql(...)`.
* Tests only SQL happy path.
* No direct Arrow-array test.
* No scalar `ColumnarValue::Scalar` test.
* Wrong-type path panics instead of returning `DataFusionError`.
* Public SQL endpoint registers experimental UDF package globally.
* Deprecated function silently changes behavior instead of adding replacement function.

---

# C2.11 Final agent checklist

```text id="zkdodq"
[ ] Create CalculationSpec before implementation.
[ ] Record semantic intent, units, basis, null policy, invalid-input policy.
[ ] Choose calculation class and record rejected alternatives.
[ ] Define canonical name, aliases, family, parameter names.
[ ] Define SignatureSpec and CoercionPolicy.
[ ] Define ReturnFieldSpec with type, nullability, metadata.
[ ] Define VolatilityPolicy truthfully.
[ ] Implement Arrow-vectorized logic with scalar and array paths.
[ ] Return DataFusionError instead of panic/unwrap/expect.
[ ] Validate scalar output row count.
[ ] For UDAF, validate state_type/state/merge_batch/evaluate.
[ ] For UDWF, validate partition/order/frame behavior.
[ ] For UDTF, keep planning cheap and bounded.
[ ] Register before planning.
[ ] Expose SQL and DataFrame/Expr helper examples.
[ ] Generate docs from manifest.
[ ] Assign SecurityPolicy and include aliases.
[ ] Add direct Arrow tests, SQL tests, DataFrame tests, policy tests.
[ ] Add benchmarks for hot/expensive/external calculations.
[ ] Version function semantics.
[ ] Add deprecation/migration spec for public functions.
[ ] Gate deployment on manifest + tests + policy + docs + version pin.
```

[1]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.ScalarUDF.html "ScalarUDF in datafusion::logical_expr - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/trait.ScalarUDFImpl.html "ScalarUDFImpl in datafusion::logical_expr - Rust"
[3]: https://datafusion.apache.org/library-user-guide/working-with-exprs.html "Working with Exprs — Apache DataFusion  documentation"
[4]: https://docs.rs/datafusion/latest/datafusion/logical_expr/fn.create_udaf.html?utm_source=chatgpt.com "create_udaf in datafusion::logical_expr - Rust"
[5]: https://docs.rs/datafusion/latest/datafusion/logical_expr/trait.Accumulator.html?utm_source=chatgpt.com "Accumulator in datafusion::logical_expr - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/logical_expr/trait.GroupsAccumulator.html?utm_source=chatgpt.com "GroupsAccumulator in datafusion::logical_expr - Rust"
[7]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.WindowUDF.html "WindowUDF in datafusion::logical_expr - Rust"
[8]: https://docs.rs/datafusion/latest/datafusion/logical_expr/fn.create_udwf.html "create_udwf in datafusion::logical_expr - Rust"


# C2.12 Upstream built-in contract changes (DataFusion 54)

The `datafusion_min`/`datafusion_max_tested` fields in the VersionSpec exist precisely because engine upgrades change the observable contracts of *built-in* functions your calculations and consumers compose with. DataFusion 54 changes that affect calculation pipelines directly:

## C2.12.1 Approximate aggregates now coerce integers to Float64

`approx_percentile_cont`, `approx_percentile_cont_with_weight`, and `approx_median` coerce integer inputs to `Float64` in DataFusion 54. In 53 these signatures enumerated exact numeric variants and the return type followed the input type (an `Int64` column yielded an `Int64` percentile); in 54 the coercible signature targets `NativeType::Float64`, so integer inputs produce **`Float64` outputs**. This is a return-type contract change for consumers:

```text id="approx-coercion-rebaseline"
Re-baseline any schema snapshot, Arrow round-trip test, or downstream contract
that pinned Int* output for approx_percentile_cont / approx_percentile_cont_with_weight /
approx_median over integer columns: the output field is now Float64.
Exact-value assertions on integer percentiles must switch to float tolerance.
```

## C2.12.2 Comparison coercion is numeric-preferring

Mixed string/number comparisons coerce the string side numerically (invalid numeric strings raise execution errors), and `greatest`/`least` follow the same rule — see C5.3.5 for the full treatment. Lifecycle consequence: SQL examples and test fixtures written under 53's string-preferring comparisons can silently change results (`5 > '100'` is now `false`), so the C2.8 gates must re-run example/fixture suites on the engine bump, not merely recompile.

---

# DataFusion Advanced — C3) Function registry, cataloging, and discovery

## C3.0 Objective

Connect DataFusion’s **runtime function registry** to a governed **product-level function catalog**:

```text id="c3-root"
DataFusion runtime registry
  ├─ resolves function calls during planning/execution
  ├─ stores ScalarUDF / AggregateUDF / WindowUDF implementations
  ├─ lives in SessionState / TaskContext / FunctionRegistry surfaces
  └─ is necessary but insufficient for product governance

Product function catalog
  ├─ stores canonical metadata, policy, docs, versions, aliases, ownership
  ├─ drives registration into DataFusion
  ├─ drives SQL/DataFrame docs and discovery
  ├─ drives tenant/function authorization
  ├─ drives generated tests and compatibility checks
  └─ becomes source of truth for LLM agents
```

The attachment already maps DataFusion registries: scalar, aggregate, and window function registries are held by `SessionState` / `TaskContext`, and public entry points include `register_udf`, `register_udaf`, and `register_udwf`. It also states that `datafusion::execution` includes a `FunctionRegistry` trait and that SQL function names are lowercased unless quoted, with aliases capable of overwriting existing functions in the registry. 

DataFusion’s current `FunctionRegistry` trait exposes `register_udf`, `register_udaf`, `register_udwf`, and deregistration methods that return any previously registered implementation, while the docs note registration may error for read-only registries. ([Docs.rs][1]) DataFusion’s `SessionState` contains the state necessary to plan and execute queries, including configuration, functions, and runtime environment. ([Docs.rs][2])

---

## C3.1 Core principle

```text id="c3-principle"
Runtime registration is not catalog governance.
Catalog presence is not runtime availability.
Runtime availability is not authorization.
Authorization is not documentation.
Documentation is not version compatibility.
```

Required separation:

```text id="c3-separation"
FunctionCatalog      = source of truth for metadata, versions, policy, docs
RuntimeRegistry      = DataFusion planning/execution lookup
TenantFunctionPolicy = authorization view over catalog
DocumentationIndex   = generated human/agent docs
VersionRegistry      = semantic compatibility ledger
DiscoverySurface     = SQL/CLI/API/UI/JSON/YAML read model
```

Agent invariant:

```text id="c3-agent-invariant"
Never add a UDF by only calling ctx.register_udf(...).
Every public calculation needs:
  catalog entry
  manifest entry
  runtime registration hook
  tenant policy entry
  documentation entry
  test fixture entry
  version/deprecation entry
```

---

## C3.2 Registry/catolog strata

| Layer                        | Purpose                                    |         Mutability | Scope                                             | Failure mode if missing              |
| ---------------------------- | ------------------------------------------ | -----------------: | ------------------------------------------------- | ------------------------------------ |
| DataFusion runtime registry  | query planner/executor function resolution |    process/session | `SessionContext` / `SessionState` / `TaskContext` | SQL/DataFrame function call fails    |
| Product function catalog     | canonical metadata source                  | controlled release | product/application                               | agents cannot reason about functions |
| Tenant function policy       | allow/deny/cost/risk view                  |     runtime/config | tenant/request/workload                           | unauthorized or missing functions    |
| Documentation registry       | generated docs/examples                    |   release artifact | docs/API/CLI                                      | users/agents misuse functions        |
| Version/deprecation registry | compatibility ledger                       |   release artifact | product version                                   | breaking changes become silent       |
| Discovery read model         | queryable/searchable metadata              |  generated/runtime | SQL/CLI/API/UI                                    | functions invisible or inconsistent  |

The attachment’s security sections already require function allowlists and explicitly include tests for allowed functions, denied functions, UDF aliases, volatile/expensive functions, and function policy validation. 

---

## C3.3 DataFusion runtime registry

### Runtime registry responsibilities

```text id="runtime-responsibilities"
resolve function name during planning
provide signature and volatility
provide return type / return field inference
provide implementation handle
support scalar / aggregate / window function families
allow DataFrame Expr helpers to build function call Exprs
```

### Registry map

```text id="runtime-map"
SessionContext
  ├─ register_udf(ScalarUDF)
  ├─ register_udaf(AggregateUDF)
  ├─ register_udwf(WindowUDF)
  └─ creates SessionState snapshots

SessionState
  ├─ scalar_functions()
  ├─ aggregate_functions()
  ├─ window_functions()
  ├─ function registries for planning
  └─ task_ctx()

TaskContext
  ├─ scalar functions for execution
  ├─ aggregate functions for execution
  ├─ window functions for execution
  └─ runtime resources
```

### Basic registration

```rust id="runtime-register-basic"
use datafusion::prelude::*;
use datafusion::logical_expr::{ScalarUDF, AggregateUDF, WindowUDF};

pub fn register_runtime_functions(ctx: &SessionContext) -> datafusion::error::Result<()> {
    ctx.register_udf(ScalarUDF::from(NormalizeApiGravity::new()));
    ctx.register_udaf(weighted_sulfur_udaf());
    ctx.register_udwf(rolling_stability_udwf());
    Ok(())
}
```

### Runtime registry invariants

```text id="runtime-invariants"
[ ] registration occurs before query planning
[ ] canonical names are lowercase snake_case unless quoted-name behavior is intentional
[ ] aliases resolve to same semantic function/version
[ ] registered implementation matches catalog signature
[ ] registered implementation matches tenant policy
[ ] function is registered in every SessionContext where policy says enabled
[ ] distributed executors receive same function set and versions
```

DataFusion’s `SessionContext` is the main interface for executing queries and registering data/functions; current docs describe it as maintaining connection state between the user and a DataFusion engine instance. ([Docs.rs][3])

---

## C3.4 Product function catalog

### Product catalog responsibilities

```text id="catalog-responsibilities"
canonicalize function identity
declare family and placement
declare signature, parameter names, return field
declare volatility, null behavior, deterministic flag
declare risk/cost/security class
declare documentation and examples
declare test fixtures and benchmark requirements
declare version, deprecation, replacement
drive runtime registration and tenant policy
```

### Minimal catalog entry

```yaml id="function-catalog-yaml"
id: refinery.normalize_api_gravity.v1
canonical_name: normalize_api_gravity
aliases:
  - normalize_api
family: scalar
status: active
function_version: 1.0.0
package: refinery_calc
rust_symbol: refinery_calc::scalar::NormalizeApiGravity

signature:
  mode: exact
  args:
    - name: api_gravity
      data_type: Float64
      nullable: true
      semantic_type: api_gravity
      unit: deg_api
  variadic: false
  named_args: true

return_field:
  name: api_gravity_normalized
  data_type: Float64
  nullable: true
  metadata:
    unit: deg_api
    semantic_type: api_gravity

semantics:
  volatility: Immutable
  deterministic: true
  null_policy: strict_null_propagation
  external_io: false
  side_effects: false

governance:
  cost_class: low
  security_class: pure_deterministic
  enabled_tenants:
    - internal_refinery_modeling
  public_sql_allowed: false

docs:
  section: refinery.quality
  summary: Normalize API gravity according to internal convention.
  examples:
    sql:
      - "SELECT normalize_api_gravity(api) AS api_norm FROM streams"
    dataframe:
      - "normalize_api_gravity_expr(col(\"api\"))"
  test_fixture_ids:
    - normalize_api.basic
    - normalize_api.nulls
    - normalize_api.type_error

versioning:
  introduced_in: refinery_calc_0.1.0
  deprecated_since: null
  replacement: null
```

### Rust model

```rust id="function-catalog-rust"
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FunctionCatalogEntry {
    pub id: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub family: FunctionFamily,
    pub status: FunctionStatus,
    pub function_version: semver::Version,
    pub package: String,
    pub rust_symbol: String,

    pub signature: SignatureSpec,
    pub return_field: ReturnFieldSpec,
    pub semantics: FunctionSemantics,
    pub governance: FunctionGovernance,
    pub docs: FunctionDocs,
    pub versioning: FunctionVersioning,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FunctionFamily {
    Scalar,
    AsyncScalar,
    Aggregate,
    Window,
    Table,
    ExprHelper,
    CustomLogical,
    CustomPhysical,
    ExternalPreprocess,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FunctionStatus {
    Experimental,
    Active,
    Deprecated,
    Disabled,
    Removed,
}
```

---

## C3.5 Catalog field schema

### Identity fields

| Field            |         Type | Required | Notes                                                   |
| ---------------- | -----------: | -------: | ------------------------------------------------------- |
| `id`             |       string |      yes | globally unique, stable, usually namespace + version    |
| `canonical_name` |       string |      yes | SQL-visible canonical lowercase function name           |
| `aliases`        | list[string] |      yes | additional SQL-visible names; policy must include them  |
| `family`         |         enum |      yes | scalar / async scalar / aggregate / window / table      |
| `status`         |         enum |      yes | experimental / active / deprecated / disabled / removed |
| `package`        |       string |      yes | crate/package owner                                     |
| `rust_symbol`    |       string |      yes | implementation symbol or registration hook              |

### Type/signature fields

| Field                        | Required | Notes                                               |
| ---------------------------- | -------: | --------------------------------------------------- |
| `signature.mode`             |      yes | exact / uniform / variadic / one_of / user_defined  |
| `signature.args[].name`      |      yes | public API for named args and docs                  |
| `signature.args[].data_type` |      yes | Arrow/DataFusion type spec                          |
| `signature.args[].nullable`  |      yes | input nullability expectations                      |
| `signature.variadic`         |      yes | true only if implementation supports variable arity |
| `signature.named_args`       |      yes | docs/SQL generator behavior                         |
| `return_field.name`          |      yes | stable default output name                          |
| `return_field.data_type`     |      yes | expected Arrow/DataFusion type                      |
| `return_field.nullable`      |      yes | output nullability contract                         |
| `return_field.metadata`      |      yes | units, semantic type, lineage, domain metadata      |

### Semantic/governance fields

| Field                | Required | Notes                                                   |
| -------------------- | -------: | ------------------------------------------------------- |
| `volatility`         |      yes | DataFusion volatility; affects optimization eligibility |
| `deterministic`      |      yes | product-level stronger boolean                          |
| `null_policy`        |      yes | strict/skip/default/error/try-mode                      |
| `cost_class`         |      yes | low/medium/high/external/unbounded                      |
| `external_io`        |      yes | function can call external system                       |
| `side_effects`       |      yes | must be false for query-time functions                  |
| `security_class`     |      yes | pure, expensive, volatile, external-read, admin-only    |
| `enabled_tenants`    |      yes | tenant allowlist or policy key                          |
| `public_sql_allowed` |      yes | explicit exposure flag                                  |

### Discovery/documentation fields

| Field                     |    Required | Notes                            |
| ------------------------- | ----------: | -------------------------------- |
| `docs.section`            |         yes | generated docs placement         |
| `docs.summary`            |         yes | human/agent short description    |
| `docs.examples.sql`       |         yes | queryable examples               |
| `docs.examples.dataframe` |         yes | Rust/Expr usage                  |
| `test_fixture_ids`        |         yes | link to generated tests          |
| `benchmark_ids`           | conditional | hot/high-cost/external functions |
| `introduced_in`           |         yes | release traceability             |
| `deprecated_since`        | conditional | deprecation lifecycle            |
| `replacement`             | conditional | migration target                 |

---

## C3.6 Runtime registry vs product catalog

| Concern                       | Runtime registry              | Product function catalog |
| ----------------------------- | ----------------------------- | ------------------------ |
| resolves SQL calls            | yes                           | no                       |
| holds implementation object   | yes                           | maybe symbol reference   |
| records security class        | no                            | yes                      |
| records enabled tenants       | no                            | yes                      |
| records test fixture IDs      | no                            | yes                      |
| records docs section          | maybe via UDF docs hook       | yes                      |
| records deprecation lifecycle | no                            | yes                      |
| records compatibility aliases | maybe registered              | yes                      |
| drives generated docs         | no                            | yes                      |
| drives plan validation        | no                            | yes                      |
| survives process restart      | only if registration repeated | yes as artifact/storage  |
| versioned by product release  | indirectly                    | yes                      |

### Agent rule

```text id="runtime-vs-catalog-rule"
Runtime registry answers: “Can DataFusion resolve this function in this session?”
Product catalog answers: “What is this function, who may use it, how is it versioned, and how should agents call/test/document it?”
```

---

# C3.7 Tenant function policy

## C3.7.1 Policy model

```yaml id="tenant-policy-yaml"
tenant: internal_refinery_modeling
policy_version: 2026.05.24

function_policy:
  default: deny

  allow:
    exact:
      - normalize_api_gravity
      - normalize_api
      - weighted_sulfur
    families:
      - built_in_math
      - built_in_string
      - aggregate_udf

  deny:
    exact:
      - live_market_price
      - llm_extract
    risk_classes:
      - external_write
      - diagnostic_metadata

  constraints:
    max_cost_class: high
    allow_external_io: false
    allow_volatile: false
    allow_experimental: false
    allow_deprecated: warn
    require_limit_for_table_functions: true
```

## C3.7.2 Rust policy structures

```rust id="tenant-policy-rust"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TenantFunctionPolicy {
    pub tenant_id: String,
    pub default: DefaultDecision,
    pub allow_exact: BTreeSet<String>,
    pub deny_exact: BTreeSet<String>,
    pub allow_families: BTreeSet<FunctionFamily>,
    pub deny_risk_classes: BTreeSet<RiskClass>,
    pub max_cost_class: CostClass,
    pub allow_external_io: bool,
    pub allow_volatile: bool,
    pub allow_experimental: bool,
    pub deprecated_behavior: DeprecatedBehavior,
}
```

## C3.7.3 Policy normalization

```rust id="policy-normalize"
pub fn normalize_function_name(name: &str) -> String {
    // Product-level canonicalization should match unquoted SQL policy.
    // Keep quoted/case-sensitive function support separate and explicit.
    name.to_ascii_lowercase()
}

pub fn expand_aliases<'a>(
    catalog: &'a FunctionCatalog,
    canonical_name: &str,
) -> Vec<&'a str> {
    catalog
        .by_canonical(canonical_name)
        .map(|entry| {
            std::iter::once(entry.canonical_name.as_str())
                .chain(entry.aliases.iter().map(String::as_str))
                .collect()
        })
        .unwrap_or_default()
}
```

## C3.7.4 Authorization decision

```rust id="authorize-call"
pub fn authorize_catalog_entry(
    entry: &FunctionCatalogEntry,
    policy: &TenantFunctionPolicy,
) -> Result<(), FunctionAuthError> {
    let names = std::iter::once(entry.canonical_name.as_str())
        .chain(entry.aliases.iter().map(String::as_str));

    if names.clone().any(|n| policy.deny_exact.contains(n)) {
        return Err(FunctionAuthError::DeniedExact(entry.canonical_name.clone()));
    }

    if policy.deny_risk_classes.contains(&entry.governance.security_class) {
        return Err(FunctionAuthError::DeniedRiskClass(entry.governance.security_class.clone()));
    }

    if !policy.allow_external_io && entry.semantics.external_io {
        return Err(FunctionAuthError::ExternalIoNotAllowed(entry.canonical_name.clone()));
    }

    if !policy.allow_volatile && entry.semantics.volatility == VolatilitySpec::Volatile {
        return Err(FunctionAuthError::VolatileNotAllowed(entry.canonical_name.clone()));
    }

    if entry.governance.cost_class > policy.max_cost_class {
        return Err(FunctionAuthError::CostClassTooHigh {
            function: entry.canonical_name.clone(),
            cost: entry.governance.cost_class.clone(),
            max: policy.max_cost_class.clone(),
        });
    }

    if names.clone().any(|n| policy.allow_exact.contains(n))
        || policy.allow_families.contains(&entry.family)
    {
        return Ok(());
    }

    Err(FunctionAuthError::DefaultDeny(entry.canonical_name.clone()))
}
```

### Agent rules

```text id="policy-agent-rules"
Function aliases must be authorized identically to canonical names.
Risk-class deny overrides exact allow unless policy explicitly permits override.
Runtime registration should not imply tenant authorization.
Public SQL should validate planned function calls against catalog+policy before execution.
```

---

# C3.8 Documentation registry

## C3.8.1 Documentation record

```yaml id="docs-registry"
docs:
  section: refinery.quality
  title: normalize_api_gravity
  summary: Normalize API gravity according to refinery calculation convention.
  signature: normalize_api_gravity(api_gravity: Float64) -> Float64
  aliases:
    - normalize_api
  examples:
    sql:
      - |
        SELECT
          stream_id,
          normalize_api_gravity(api) AS api_norm
        FROM streams;
    dataframe:
      - |
        df.with_column(
            "api_norm",
            normalize_api_gravity_expr(col("api")),
        )?
  null_behavior: Null input returns null.
  volatility: Immutable
  security: pure_deterministic
  cost_class: low
  status: active
  introduced_in: refinery_calc_0.1.0
```

## C3.8.2 Generated docs layout

```text id="docs-layout"
docs/
  functions/
    index.md
    scalar/
      normalize_api_gravity.md
      density_to_api.md
    aggregate/
      weighted_sulfur.md
    window/
      rolling_stability_score.md
    table/
      scenario_parameters.md
  generated/
    functions.json
    functions.yaml
    function_policy_schema.json
```

## C3.8.3 Documentation generation command

```bash id="docs-command"
cargo run -p calc-catalog-cli -- generate-docs \
  --catalog calc-manifests/functions.yaml \
  --out docs/functions \
  --format markdown,json
```

### Agent rules

```text id="docs-agent-rules"
Generated docs are build artifacts, not manually edited source of truth.
Every public function must have at least one SQL example and one DataFrame/Expr example.
Examples must compile or execute in tests.
Docs must expose status: experimental/active/deprecated/disabled.
```

`ScalarUDFImpl` supports a `documentation` method for function docs metadata; use DataFusion-local documentation when useful, but keep product catalog docs as the broader governed source of truth. ([Docs.rs][4])

---

# C3.9 Version and deprecation registry

## C3.9.1 Version record

```yaml id="version-record"
versioning:
  canonical_name: normalize_api_gravity
  function_version: 1.2.0
  api_stability: stable
  introduced_in_package: refinery_calc_0.1.0
  introduced_in_product: smartref_engine_0.3.0
  datafusion_min: 54.1.0
  datafusion_max_tested: 54.1.0
  arrow_min: 58.4.0
  signature_hash: sha256:...
  semantics_hash: sha256:...
  docs_hash: sha256:...
```

## C3.9.2 Breaking-change matrix

| Change                                              |                       Breaking? | Required action               |
| --------------------------------------------------- | ------------------------------: | ----------------------------- |
| rename canonical function                           |                             yes | alias old name + deprecate    |
| remove alias                                        |                   yes if public | deprecate first               |
| reorder positional parameters                       |                             yes | create new function/version   |
| change parameter type accepted set                  |                           maybe | compatibility tests           |
| narrow accepted type                                |                             yes | major version or new function |
| widen accepted type                                 |                      usually no | add tests                     |
| change null behavior                                |                             yes | new function/version          |
| change output type                                  |                             yes | new function/version          |
| change output nullability from nullable to non-null |                           maybe | compatibility review          |
| change output metadata only                         |                           maybe | downstream audit              |
| change volatility stricter                          | usually no but can affect plans | plan regression               |
| change volatility looser                            |                        yes risk | plan regression + docs        |
| change deterministic algorithm with same semantics  |            no if outputs stable | benchmark/regression          |
| change numerical tolerance                          |                           maybe | reference comparison          |

## C3.9.3 Deprecation record

```yaml id="deprecation-record"
deprecation:
  canonical_name: normalize_api_gravity
  status: deprecated_warn
  deprecated_since: refinery_calc_1.4.0
  disabled_by_default_since: null
  removal_not_before: refinery_calc_2.0.0
  replacement: normalize_api_gravity_v2
  reason: v1 used legacy pressure basis.
  migration:
    sql:
      from: "normalize_api_gravity(api)"
      to: "normalize_api_gravity_v2(api, 'standard_pressure')"
    dataframe:
      from: "normalize_api_gravity_expr(col(\"api\"))"
      to: "normalize_api_gravity_v2_expr(col(\"api\"), lit(\"standard_pressure\"))"
```

## C3.9.4 Deprecation runtime behavior

```text id="deprecation-runtime"
experimental:
  allowed only in dev/internal tenants

active:
  normal registration and authorization

deprecated:
  registered if policy allows deprecated
  emits plan-validation warning / audit event
  docs show replacement

disabled:
  catalog entry retained
  not registered by default
  policy can temporarily enable for migration

removed:
  not registered
  tombstone retained for helpful diagnostics
```

---

# C3.10 Discovery surfaces

## C3.10.1 Agent-readable JSON/YAML manifest

### JSON output

```json id="functions-json"
{
  "functions": [
    {
      "canonical_name": "normalize_api_gravity",
      "aliases": ["normalize_api"],
      "family": "scalar",
      "signature": "normalize_api_gravity(api_gravity: Float64) -> Float64",
      "volatility": "Immutable",
      "deterministic": true,
      "null_policy": "strict_null_propagation",
      "cost_class": "low",
      "external_io": false,
      "security_class": "pure_deterministic",
      "status": "active",
      "docs_section": "refinery.quality",
      "examples": {
        "sql": [
          "SELECT normalize_api_gravity(api) AS api_norm FROM streams"
        ]
      },
      "test_fixture_ids": ["normalize_api.basic", "normalize_api.nulls"]
    }
  ]
}
```

### CLI command

```bash id="manifest-cli"
calc-catalog functions export \
  --tenant internal_refinery_modeling \
  --format json \
  --include builtins,custom \
  --out target/function_catalog.internal_refinery_modeling.json
```

### Agent rule

```text id="agent-manifest-rule"
LLM programming agents should read JSON/YAML manifest, not scrape Markdown docs.
Markdown docs are for humans; manifest is for code generation and plan validation.
```

---

## C3.10.2 Generated docs

```bash id="generated-docs-cli"
calc-catalog docs build \
  --catalog calc-manifests/functions.yaml \
  --output docs/functions
```

Generated docs should include:

```text id="generated-docs-fields"
canonical name
aliases
family
signature
argument table
return field
null behavior
volatility
determinism
cost/security class
SQL examples
DataFrame examples
version/deprecation
test fixture references
```

---

## C3.10.3 `SHOW FUNCTIONS`-style product metadata

DataFusion’s current `information_schema` docs describe table/column discovery and settings discovery via views such as `information_schema.columns` and `information_schema.df_settings`; they do not establish a complete product-level function catalog with security/deprecation/version metadata. ([Apache DataFusion][5]) Implement `SHOW FUNCTIONS`-style metadata as a product-specific surface.

### SQL view/table shape

```sql id="show-functions-sql"
SELECT
  function_catalog,
  function_schema,
  function_name,
  canonical_name,
  family,
  signature,
  return_type,
  volatility,
  deterministic,
  null_policy,
  cost_class,
  security_class,
  status,
  deprecated_since,
  replacement
FROM product_information_schema.functions
WHERE tenant_enabled = true
ORDER BY family, function_name;
```

### Possible backing options

```text id="show-functions-backing"
Option A: Product metadata table registered as TableProvider
Option B: UDTF: product_functions()
Option C: custom information_schema extension
Option D: CLI/API endpoint only
```

### UDTF-style query

```sql id="product-functions-udtf"
SELECT
  function_name,
  family,
  signature,
  status,
  replacement
FROM product_functions()
WHERE tenant_enabled = true
ORDER BY function_name;
```

### Agent rule

```text id="show-functions-agent"
Do not assume core DataFusion exposes governed SHOW FUNCTIONS metadata.
Implement product_functions() / product_information_schema.functions from FunctionCatalog.
```

---

## C3.10.4 CLI diagnostic command

```bash id="cli-diagnostics"
calc-catalog functions list --tenant internal_refinery_modeling

calc-catalog functions describe normalize_api_gravity \
  --tenant internal_refinery_modeling \
  --format markdown

calc-catalog functions check-policy \
  --tenant public_sql \
  --function live_market_price

calc-catalog functions diff \
  --old function_catalog_v1.yaml \
  --new function_catalog_v2.yaml
```

### CLI output shape

```text id="cli-output"
FUNCTION                 FAMILY     VERSION  STATUS      ALLOWED  COST  RISK
normalize_api_gravity    scalar     1.2.0    active      yes      low   pure
weighted_sulfur          aggregate  1.0.0    active      yes      med   pure
live_market_price        async      0.8.0    experimental no      ext   external_read
```

---

## C3.10.5 API discovery endpoint

```http id="api-discovery"
GET /v1/functions?tenant=internal_refinery_modeling
GET /v1/functions/normalize_api_gravity?tenant=internal_refinery_modeling
GET /v1/functions?family=aggregate&allowed=true
```

### JSON shape

```json id="api-function-response"
{
  "canonical_name": "weighted_sulfur",
  "family": "aggregate",
  "signature": "weighted_sulfur(sulfur_wt_pct: Float64, volume_bbl: Float64) -> Float64",
  "status": "active",
  "allowed": true,
  "version": "1.0.0",
  "docs_url": "/docs/functions/aggregate/weighted_sulfur",
  "examples": {
    "sql": [
      "SELECT crude_slate, weighted_sulfur(sulfur, volume) FROM components GROUP BY crude_slate"
    ]
  }
}
```

---

# C3.11 Collision rules

## C3.11.1 Collision taxonomy

| Collision type                | Example                                       | Required policy                              |
| ----------------------------- | --------------------------------------------- | -------------------------------------------- |
| canonical vs built-in         | custom `round`                                | deny by default                              |
| alias vs built-in             | alias `sum`                                   | deny                                         |
| canonical vs custom canonical | two `normalize_api_gravity`                   | deny                                         |
| alias vs custom canonical     | alias `weighted_avg` conflicts with canonical | deny unless deliberate migration             |
| alias vs alias                | two aliases `normalize_api`                   | deny                                         |
| case-only collision           | `MyFunc` vs `myfunc`                          | deny for unquoted SQL                        |
| quoted-name exception         | `"MyFunc"`                                    | allow only if explicit case-sensitive policy |
| tenant-specific override      | tenant A has custom `score`                   | isolate by context/catalog/policy            |
| package override              | plugin replaces built-in                      | require explicit override record             |

## C3.11.2 Name normalization

```rust id="name-normalization"
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FunctionNameKey {
    pub normalized: String,
    pub quoted_case_sensitive: bool,
}

pub fn unquoted_function_key(name: &str) -> FunctionNameKey {
    FunctionNameKey {
        normalized: name.to_ascii_lowercase(),
        quoted_case_sensitive: false,
    }
}
```

SQL function names are looked up lowercase unless quoted in DataFusion; the attachment records the same rule for function registration and notes that existing aliases can overwrite existing functions. 

## C3.11.3 Collision validator

```rust id="collision-validator"
pub fn validate_catalog_collisions(
    entries: &[FunctionCatalogEntry],
    reserved_builtin_names: &BTreeSet<String>,
) -> Result<(), CatalogValidationError> {
    let mut owner_by_name: BTreeMap<String, String> = BTreeMap::new();

    for entry in entries {
        let mut names = Vec::new();
        names.push(entry.canonical_name.as_str());
        names.extend(entry.aliases.iter().map(String::as_str));

        for name in names {
            let key = name.to_ascii_lowercase();

            if reserved_builtin_names.contains(&key) && !entry.governance.allow_builtin_override {
                return Err(CatalogValidationError::BuiltinCollision {
                    name: key,
                    owner: entry.id.clone(),
                });
            }

            if let Some(existing_owner) = owner_by_name.insert(key.clone(), entry.id.clone()) {
                return Err(CatalogValidationError::DuplicateFunctionName {
                    name: key,
                    first_owner: existing_owner,
                    second_owner: entry.id.clone(),
                });
            }
        }
    }

    Ok(())
}
```

## C3.11.4 Built-in override policy

```yaml id="override-policy"
collision_policy:
  builtins:
    default: deny
    allowed_overrides:
      - name: regexp_match
        package: postgres_compat
        reason: Product dialect requires PostgreSQL-compatible semantics.
        approved_by: query_platform_owner
        tests:
          - regexp_match.postgres_compat
  aliases:
    default: deny_if_conflict
  case_only:
    default: deny
```

### Agent rules

```text id="collision-agent-rules"
Do not override built-ins accidentally.
Do not add aliases without collision validation.
Do not rely on case-only distinction for unquoted SQL.
Every intentional override needs:
  reason
  owning package
  compatibility tests
  migration note
  policy approval
```

---

# C3.12 Registration generated from catalog

## C3.12.1 Generated registration manifest

```yaml id="registration-manifest"
registration_plan:
  context: internal_refinery_modeling
  functions:
    - canonical_name: normalize_api_gravity
      family: scalar
      rust_constructor: refinery_calc::scalar::NormalizeApiGravity::new
      enabled: true
    - canonical_name: weighted_sulfur
      family: aggregate
      rust_constructor: refinery_calc::aggregate::weighted_sulfur_udaf
      enabled: true
    - canonical_name: rolling_stability_score
      family: window
      rust_constructor: refinery_calc::window::rolling_stability_udwf
      enabled: false
      reason: policy_denied
```

## C3.12.2 Registration driver

```rust id="registration-driver"
pub fn register_from_catalog(
    ctx: &SessionContext,
    catalog: &FunctionCatalog,
    policy: &TenantFunctionPolicy,
) -> datafusion::error::Result<RegistrationReport> {
    let mut report = RegistrationReport::default();

    for entry in catalog.entries() {
        match authorize_catalog_entry(entry, policy) {
            Ok(()) => {
                register_entry(ctx, entry)?;
                report.registered.push(entry.canonical_name.clone());
            }
            Err(reason) => {
                report.skipped.push(SkippedFunction {
                    function: entry.canonical_name.clone(),
                    reason: reason.to_string(),
                });
            }
        }
    }

    Ok(report)
}

fn register_entry(
    ctx: &SessionContext,
    entry: &FunctionCatalogEntry,
) -> datafusion::error::Result<()> {
    match entry.family {
        FunctionFamily::Scalar => register_scalar_entry(ctx, entry),
        FunctionFamily::Aggregate => register_aggregate_entry(ctx, entry),
        FunctionFamily::Window => register_window_entry(ctx, entry),
        FunctionFamily::AsyncScalar => register_async_scalar_entry(ctx, entry),
        FunctionFamily::Table => register_table_entry(ctx, entry),
        _ => Ok(()),
    }
}
```

## C3.12.3 Explicit function package registration

```rust id="explicit-package-registration"
pub trait FunctionPackage {
    fn package_name(&self) -> &'static str;
    fn package_version(&self) -> &'static str;
    fn catalog_entries(&self) -> Vec<FunctionCatalogEntry>;
    fn register(
        &self,
        ctx: &SessionContext,
        policy: &TenantFunctionPolicy,
    ) -> datafusion::error::Result<RegistrationReport>;
}
```

### Agent rules

```text id="registration-agent-rules"
Registration must be deterministic.
Registration must be policy-aware.
Registration report must be auditable.
Skipped functions should include reason.
Do not partially register aliases without canonical function.
Do not register deprecated functions unless policy allows.
```

---

# C3.13 Product `information_schema` extension pattern

## C3.13.1 Metadata table provider

```rust id="functions-table-provider"
#[derive(Debug)]
pub struct FunctionsTableProvider {
    schema: datafusion::arrow::datatypes::SchemaRef,
    rows: Arc<Vec<FunctionCatalogRow>>,
}

impl FunctionsTableProvider {
    pub fn new(catalog: &FunctionCatalog, policy: &TenantFunctionPolicy) -> Self {
        Self {
            schema: functions_schema(),
            rows: Arc::new(catalog
                .entries()
                .map(|entry| FunctionCatalogRow::from_entry(entry, policy))
                .collect()),
        }
    }
}
```

## C3.13.2 Schema

```rust id="functions-schema"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};

pub fn functions_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("function_catalog", DataType::Utf8, false),
        Field::new("function_schema", DataType::Utf8, false),
        Field::new("function_name", DataType::Utf8, false),
        Field::new("canonical_name", DataType::Utf8, false),
        Field::new("aliases", DataType::Utf8, true),          // JSON or comma-delimited
        Field::new("family", DataType::Utf8, false),
        Field::new("signature", DataType::Utf8, false),
        Field::new("return_type", DataType::Utf8, false),
        Field::new("volatility", DataType::Utf8, false),
        Field::new("deterministic", DataType::Boolean, false),
        Field::new("null_policy", DataType::Utf8, false),
        Field::new("cost_class", DataType::Utf8, false),
        Field::new("security_class", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("tenant_enabled", DataType::Boolean, false),
        Field::new("deprecated_since", DataType::Utf8, true),
        Field::new("replacement", DataType::Utf8, true),
        Field::new("docs_section", DataType::Utf8, true),
    ]))
}
```

## C3.13.3 Registration

```rust id="register-functions-metadata"
pub fn register_product_information_schema(
    ctx: &SessionContext,
    catalog: &FunctionCatalog,
    policy: &TenantFunctionPolicy,
) -> datafusion::error::Result<()> {
    let provider = Arc::new(FunctionsTableProvider::new(catalog, policy));

    ctx.register_table(
        "product_information_schema_functions",
        provider,
    )?;

    Ok(())
}
```

## C3.13.4 Query examples

```sql id="product-functions-query"
SELECT
  function_name,
  family,
  signature,
  status,
  tenant_enabled
FROM product_information_schema_functions
WHERE tenant_enabled
ORDER BY function_name;
```

```sql id="deprecated-functions-query"
SELECT
  function_name,
  deprecated_since,
  replacement
FROM product_information_schema_functions
WHERE status = 'deprecated'
ORDER BY function_name;
```

### Deployment note

```text id="metadata-deployment-note"
If function metadata contains internal/security-sensitive functions:
  filter by tenant policy before registration
  avoid exposing denied functions by default
  separate admin metadata table from user metadata table
```

---

# C3.14 Discovery through CLI

## C3.14.1 Command surface

```bash id="cli-command-surface"
calc-fn list
calc-fn list --tenant public_sql
calc-fn describe weighted_sulfur
calc-fn policy-check weighted_sulfur --tenant public_sql
calc-fn docs weighted_sulfur --format markdown
calc-fn export --format json --tenant internal_refinery_modeling
calc-fn diff old.yaml new.yaml
calc-fn validate-collisions
calc-fn validate-registration --engine-config engine.yaml
```

## C3.14.2 `describe` output

```text id="cli-describe"
Function: weighted_sulfur
Family: aggregate
Version: 1.0.0
Status: active
Signature: weighted_sulfur(sulfur_wt_pct: Float64, volume_bbl: Float64) -> Float64
Volatility: Immutable
Deterministic: true
Null policy: skip_null_pairs
Cost class: medium
Security class: pure_deterministic
Allowed in tenant internal_refinery_modeling: yes
Allowed in tenant public_sql: no

SQL:
  SELECT crude_slate, weighted_sulfur(sulfur, volume)
  FROM crude_components
  GROUP BY crude_slate;
```

## C3.14.3 `diff` output

```text id="cli-diff"
BREAKING:
  weighted_sulfur:
    return_type changed Float64 -> Decimal128(10,4)

NON_BREAKING:
  normalize_api_gravity:
    alias added: normalize_api

DEPRECATION:
  legacy_density_index:
    deprecated_since 1.4.0
    replacement density_index_v2
```

---

# C3.15 Built-ins, packages, and custom functions

## C3.15.1 Built-in catalog ingestion

DataFusion’s built-in function packages are registered as part of default features / function packages. The `datafusion-functions` crate describes function packages implemented using DataFusion’s extension API and notes that users may control available functions to manage binary size or dialect-specific implementations. ([Docs.rs][6])

### Product strategy

```text id="builtin-strategy"
Option A: Treat built-ins as opaque but allowed by category.
Option B: Snapshot built-in function metadata into product catalog.
Option C: Register only selected function packages and catalog them explicitly.
Option D: Wrap/alias built-ins under product names for stable semantics.
```

## C3.15.2 Built-in metadata record

```yaml id="builtin-record"
id: datafusion.builtin.lower
canonical_name: lower
family: scalar
origin: datafusion_builtin
package: datafusion-functions
status: active
governance:
  security_class: pure_deterministic
  cost_class: low
  public_sql_allowed: true
docs:
  section: builtins.string
```

## C3.15.3 Built-in override policy

```text id="builtin-override-policy"
Default: custom functions cannot override built-ins.
Exception:
  dialect package intentionally replaces behavior.
  override is declared in catalog.
  compatibility tests compare old/new behavior.
  migration note exists.
```

---

# C3.16 Quoted vs unquoted function names

## C3.16.1 Name classes

```text id="name-classes"
unquoted SQL:
  SELECT MyFunc(x)
  → normalized according to SQL/DataFusion behavior, generally lowercase

quoted SQL:
  SELECT "MyFunc"(x)
  → case-sensitive / exact identifier semantics where supported
```

## C3.16.2 Product naming policy

```yaml id="naming-policy"
naming_policy:
  canonical_function_names: lowercase_snake_case
  aliases: lowercase_snake_case
  quoted_case_sensitive_functions: disallowed
  reserved_prefixes:
    - sys_
    - df_
    - information_schema_
  package_prefixes:
    refinery: refinery_
    economics: econ_
```

## C3.16.3 Agent rules

```text id="quoted-agent-rules"
Use lowercase snake_case for all generated functions.
Do not rely on quoted case-sensitive function names unless product dialect explicitly supports them.
Do not add aliases differing only by case.
Normalize policy names exactly as the SQL planner resolves unquoted names.
```

---

# C3.17 Version-compatible aliases

## C3.17.1 Alias categories

| Alias type          | Example                                       | Policy                         |
| ------------------- | --------------------------------------------- | ------------------------------ |
| ergonomic alias     | `normalize_api` -> `normalize_api_gravity`    | allowed if semantics identical |
| compatibility alias | `old_weighted_sulfur` -> `weighted_sulfur_v1` | deprecate with migration       |
| dialect alias       | `nvl` -> `coalesce`-like behavior             | test against dialect           |
| tenant alias        | tenant-specific name                          | avoid unless isolated          |
| case alias          | `MyFunc` -> `myfunc`                          | deny by default                |

## C3.17.2 Alias record

```yaml id="alias-record"
aliases:
  - name: normalize_api
    type: ergonomic
    target: normalize_api_gravity
    introduced_in: 1.0.0
    deprecated_since: null
    policy_inherits_target: true
```

## C3.17.3 Agent invariant

```text id="alias-invariant"
Alias must never change semantics.
If semantics differ, create a separate canonical function.
```

---

# C3.18 Catalog validation passes

## C3.18.1 Pass list

```text id="validation-passes"
Pass 1: schema validation
  YAML/JSON structure, required fields, enums

Pass 2: naming validation
  canonical name, aliases, reserved prefixes, case collisions

Pass 3: collision validation
  built-in collisions, custom collisions, alias collisions

Pass 4: signature validation
  valid Arrow/DataFusion type specs
  parameter count
  variadic/named-arg compatibility

Pass 5: semantic validation
  volatility/determinism coherence
  null policy/family compatibility
  cost/security coherence

Pass 6: policy validation
  enabled tenants exist
  aliases included
  public_sql_allowed consistent with risk class

Pass 7: implementation validation
  rust_symbol exists or registration hook maps entry
  feature flag available
  family matches implementation object

Pass 8: documentation validation
  docs section exists
  SQL examples parse
  DataFrame examples compile where possible

Pass 9: test validation
  fixture IDs exist
  required family-specific tests present

Pass 10: version validation
  semver valid
  breaking changes detected
  deprecation/replacement valid
```

## C3.18.2 Validator skeleton

```rust id="catalog-validator"
pub fn validate_function_catalog(
    catalog: &FunctionCatalog,
    builtins: &BuiltinFunctionIndex,
    tenants: &TenantIndex,
) -> Result<CatalogValidationReport, CatalogValidationError> {
    let mut report = CatalogValidationReport::default();

    validate_required_fields(catalog, &mut report)?;
    validate_names(catalog, &mut report)?;
    validate_collisions(catalog, builtins, &mut report)?;
    validate_signatures(catalog, &mut report)?;
    validate_semantics(catalog, &mut report)?;
    validate_policies(catalog, tenants, &mut report)?;
    validate_docs(catalog, &mut report)?;
    validate_tests(catalog, &mut report)?;
    validate_versions(catalog, &mut report)?;

    Ok(report)
}
```

---

# C3.19 Runtime registration validation

## C3.19.1 Expected vs actual registry

```rust id="registry-validation"
pub struct RuntimeRegistrySnapshot {
    pub scalar: BTreeSet<String>,
    pub aggregate: BTreeSet<String>,
    pub window: BTreeSet<String>,
    pub table: BTreeSet<String>,
}

pub fn validate_runtime_registry(
    expected: &RegistrationPlan,
    actual: &RuntimeRegistrySnapshot,
) -> Result<(), RegistryValidationError> {
    for name in &expected.scalar {
        if !actual.scalar.contains(name) {
            return Err(RegistryValidationError::MissingScalar(name.clone()));
        }
    }

    for name in &expected.aggregate {
        if !actual.aggregate.contains(name) {
            return Err(RegistryValidationError::MissingAggregate(name.clone()));
        }
    }

    for name in &expected.window {
        if !actual.window.contains(name) {
            return Err(RegistryValidationError::MissingWindow(name.clone()));
        }
    }

    Ok(())
}
```

## C3.19.2 Planning smoke tests

```rust id="planning-smoke-tests"
#[tokio::test]
async fn all_allowed_functions_plan() -> datafusion::error::Result<()> {
    let ctx = build_test_context_for_tenant("internal_refinery_modeling").await?;

    let catalog = load_function_catalog()?;
    let policy = load_tenant_policy("internal_refinery_modeling")?;

    for entry in catalog.entries_allowed_by(&policy) {
        if let Some(sql) = entry.docs.examples.sql.first() {
            ctx.sql(sql)
                .await
                .map_err(|e| datafusion::error::DataFusionError::Plan(format!(
                    "failed to plan example for {}: {e}",
                    entry.canonical_name
                )))?;
        }
    }

    Ok(())
}
```

---

# C3.20 Function call extraction for policy validation

## C3.20.1 Plan validation objective

```text id="call-extraction-objective"
Given LogicalPlan:
  collect scalar/aggregate/window/table function calls
  normalize names
  map to catalog entries
  validate tenant policy
  reject denied/unknown/high-risk functions before execution
```

## C3.20.2 Pseudocode traversal

```rust id="function-call-traversal"
pub fn validate_plan_functions(
    plan: &LogicalPlan,
    catalog: &FunctionCatalog,
    policy: &TenantFunctionPolicy,
) -> datafusion::error::Result<()> {
    let calls = collect_function_calls(plan)?;

    for call in calls {
        let name = normalize_function_name(&call.name);

        let entry = catalog
            .resolve_name(&name)
            .ok_or_else(|| datafusion::error::DataFusionError::Plan(format!(
                "function `{name}` is not registered in product catalog"
            )))?;

        authorize_catalog_entry(entry, policy)
            .map_err(|e| datafusion::error::DataFusionError::Plan(e.to_string()))?;
    }

    Ok(())
}
```

## C3.20.3 Unknown function policy

```text id="unknown-policy"
Unknown to runtime registry:
  planning fails naturally.

Known to runtime registry but unknown to product catalog:
  product validation should fail.

Known to product catalog but not runtime registered:
  deployment/registration bug.

Known and registered but tenant denied:
  authorization failure.

Known, registered, tenant allowed:
  execute.
```

---

# C3.21 Multi-tenant registry patterns

## C3.21.1 Per-tenant `SessionContext`

```rust id="per-tenant-context"
pub async fn build_tenant_engine(
    tenant: &TenantConfig,
    catalog: &FunctionCatalog,
) -> datafusion::error::Result<SessionContext> {
    let ctx = configured_context_for_tenant(tenant)?;

    register_sources_for_tenant(&ctx, tenant).await?;
    register_from_catalog(&ctx, catalog, &tenant.function_policy)?;

    Ok(ctx)
}
```

### Use when

```text id="per-tenant-use"
tenants have different function sets
tenants have different object-store credentials
tenants have different catalogs/schemas/tables
tenants require hard isolation
```

## C3.21.2 Shared context + plan validation

```rust id="shared-context-plan-validation"
pub async fn execute_tenant_sql(
    shared_ctx: &SessionContext,
    tenant: &TenantRuntime,
    sql: &str,
) -> datafusion::error::Result<()> {
    let df = shared_ctx
        .sql_with_options(sql, read_only_sql_options())
        .await?;

    validate_plan_functions(df.logical_plan(), &tenant.catalog, &tenant.function_policy)?;
    validate_plan_tables(df.logical_plan(), &tenant.table_policy)?;

    stream_with_limits(df, &tenant.resource_policy).await
}
```

### Use when

```text id="shared-context-use"
function registry is superset
tenant isolation enforced by logical plan validation
function implementations do not carry tenant-specific secrets/state
performance demands shared context
```

## C3.21.3 Hybrid pattern

```text id="hybrid-pattern"
shared core context:
  built-ins
  pure deterministic common UDFs

tenant context:
  tenant-specific catalogs
  tenant-specific object stores
  external-I/O UDFs
  experimental packages
```

### Agent rule

```text id="tenant-agent-rule"
If function implementation contains tenant-specific credentials/state, do not register it in a shared global context.
```

The attachment’s session-state deployment guidance recommends separate `SessionContext` or isolated catalogs/config per tenant in multi-tenant services, especially when object-store credentials, SQL permissions, memory limits, function sets, or table visibility differ. 

---

# C3.22 Package-level cataloging

## C3.22.1 Package manifest

```yaml id="package-manifest"
package:
  name: refinery_calc
  version: 0.4.0
  owner: process_modeling
  default_enabled: false
  functions:
    - refinery.normalize_api_gravity.v1
    - refinery.weighted_sulfur.v1
    - refinery.rolling_stability_score.v1
  feature_flags:
    - engineering_quality
    - refinery_aggregate_metrics
  compatibility:
    datafusion_min: 54.1.0
    datafusion_max_tested: 54.1.0
    arrow_min: 58.4.0
```

## C3.22.2 Package trait

```rust id="package-trait"
pub trait CatalogedFunctionPackage {
    fn package_name(&self) -> &'static str;
    fn package_version(&self) -> semver::Version;

    fn entries(&self) -> Vec<FunctionCatalogEntry>;

    fn validate(&self) -> Result<(), CatalogValidationError>;

    fn register(
        &self,
        ctx: &SessionContext,
        policy: &TenantFunctionPolicy,
    ) -> datafusion::error::Result<RegistrationReport>;
}
```

## C3.22.3 Package registration report

```rust id="registration-report"
#[derive(Debug, Default)]
pub struct RegistrationReport {
    pub package_name: String,
    pub registered: Vec<String>,
    pub skipped: Vec<SkippedFunction>,
    pub overridden: Vec<OverriddenFunction>,
    pub errors: Vec<RegistrationError>,
}
```

### Agent rules

```text id="package-agent-rules"
Package registration must be deterministic.
Package manifest version must match crate version.
Package functions must have matching catalog entries.
Package registration report must be logged at startup.
```

---

# C3.23 Function documentation and DataFusion documentation metadata

## C3.23.1 Product docs vs DataFusion UDF docs

```text id="docs-vs-udf-docs"
Product docs:
  broad governance metadata
  examples
  policy
  version/deprecation
  tenant exposure
  generated docs/API/CLI

DataFusion UDF docs:
  function-level metadata discoverable from function implementation
  useful for engine-local docs
  not sufficient for tenant policy/version governance
```

## C3.23.2 Implementing UDF documentation

```rust id="udf-docs-impl"
impl ScalarUDFImpl for NormalizeApiGravity {
    fn name(&self) -> &str {
        "normalize_api_gravity"
    }

    fn documentation(&self) -> Option<&Documentation> {
        Some(&self.documentation)
    }

    // signature, return_type, invoke_with_args...
}
```

### Agent rule

```text id="udf-doc-agent-rule"
If ScalarUDFImpl::documentation is implemented, validate it against FunctionCatalogEntry.docs.
Do not maintain divergent docs in implementation and manifest.
```

---

# C3.24 Operational observability for catalogs

## C3.24.1 Startup logs

```text id="startup-logs"
INFO function_catalog_loaded entries=142 version=2026.05.24
INFO function_catalog_validated errors=0 warnings=3
INFO function_registration_start tenant=internal_refinery_modeling
INFO function_registered name=normalize_api_gravity family=scalar version=1.2.0
INFO function_skipped name=live_market_price reason=policy_denied_external_io
INFO function_registration_complete registered=87 skipped=55
```

## C3.24.2 Runtime audit event

```json id="runtime-audit-event"
{
  "event": "function_policy_denied",
  "query_id": "q_123",
  "tenant": "public_sql",
  "function": "live_market_price",
  "family": "async_scalar",
  "risk_class": "external_read",
  "reason": "external_io_not_allowed"
}
```

## C3.24.3 Metrics

```text id="catalog-metrics"
function_catalog_entries_total{family,status}
function_registration_total{tenant,family,status}
function_policy_denials_total{tenant,function,reason}
function_deprecated_usage_total{tenant,function}
function_unknown_to_catalog_total{tenant,function}
function_runtime_missing_total{tenant,function}
```

---

# C3.25 Testing matrix

## C3.25.1 Catalog validation tests

```text id="catalog-validation-tests"
[ ] missing canonical_name fails
[ ] invalid function family fails
[ ] alias collision fails
[ ] built-in collision fails unless approved
[ ] case-only collision fails
[ ] invalid Arrow type fails
[ ] unknown tenant fails
[ ] external_io + public_sql_allowed fails
[ ] deprecated without replacement warning/fails by policy
[ ] test fixture IDs exist
```

## C3.25.2 Runtime registration tests

```text id="runtime-registration-tests"
[ ] each allowed scalar function resolves in SQL
[ ] each allowed aggregate function resolves in GROUP BY query
[ ] each allowed window function resolves in OVER query
[ ] aliases resolve or intentionally fail
[ ] denied functions are not registered or are blocked by plan validation
[ ] duplicate registration behavior is explicit
[ ] fresh test context has isolated registry
```

## C3.25.3 Discovery tests

```text id="discovery-tests"
[ ] generated JSON manifest parses
[ ] generated YAML manifest parses
[ ] generated Markdown contains all active functions
[ ] CLI list output includes allowed functions
[ ] product_functions metadata table filters denied functions
[ ] deprecated functions show replacement
[ ] examples in docs plan/execute
```

## C3.25.4 Version compatibility tests

```text id="version-tests"
[ ] signature hash unchanged for patch release
[ ] breaking changes detected
[ ] removed aliases detected
[ ] deprecated functions still register if policy allows
[ ] disabled functions produce helpful diagnostics
[ ] old SQL examples migrate through replacement rules
```

---

# C3.26 Deployment checklists

## C3.26.1 Build-time

```text id="build-checklist"
[ ] load function catalog manifests
[ ] validate schemas/enums
[ ] validate name collisions
[ ] validate built-in override policy
[ ] validate signature/return specs
[ ] generate docs
[ ] generate JSON/YAML discovery artifacts
[ ] run catalog diff against previous release
[ ] fail build on unapproved breaking changes
```

## C3.26.2 Startup-time

```text id="startup-checklist"
[ ] load tenant policies
[ ] load function catalog
[ ] validate catalog version compatibility
[ ] build SessionContext / SessionState
[ ] register built-in/default features intentionally
[ ] register package functions by policy
[ ] produce registration report
[ ] register metadata/discovery table if enabled
[ ] emit startup metrics/logs
```

## C3.26.3 Query-time

```text id="query-checklist"
[ ] parse/plan SQL with SQLOptions
[ ] validate logical plan functions against catalog+policy
[ ] reject unknown-to-catalog runtime functions
[ ] warn/audit deprecated functions
[ ] enforce cost/external/volatile policy
[ ] execute with timeout/resource limits
```

## C3.26.4 Release-time

```text id="release-checklist"
[ ] update function versions
[ ] update deprecation records
[ ] generate changelog
[ ] diff function catalog
[ ] update docs
[ ] update tenant policies
[ ] run fixture SQL corpus
[ ] run benchmark corpus for changed functions
```

---

# C3.27 Anti-pattern inventory

* Calling `ctx.register_udf(...)` without a catalog entry.
* Adding UDF alias without policy entry.
* Relying on runtime registry as documentation source.
* Relying on docs as authorization source.
* Exposing all registered functions to every tenant.
* Registering external-I/O UDFs in a public SQL context.
* Allowing custom function to override a built-in accidentally.
* Allowing aliases that collide with built-ins.
* Adding case-only aliases.
* Removing alias without deprecation.
* Changing signature without semver/version diff.
* Changing null behavior without new version.
* Marking function deprecated in docs but still active in machine manifest.
* Exposing metadata table that lists denied/internal/admin-only functions to public tenants.
* Registering tenant-specific credential-bearing functions in shared context.
* Using `SHOW FUNCTIONS`-style UX without product metadata backing.
* Letting LLM agents infer available functions by scraping examples.
* No startup registration report.
* No function catalog diff in CI.
* No test that SQL examples plan under actual registered function set.

---

# C3.28 Agent checklist

```text id="c3-final-checklist"
[ ] Distinguish runtime registry from product catalog.
[ ] Create FunctionCatalogEntry for every public calculation.
[ ] Include canonical name, aliases, family, status, package, rust symbol.
[ ] Include SignatureSpec with named parameters.
[ ] Include ReturnFieldSpec with nullability and metadata.
[ ] Include volatility, deterministic flag, null behavior.
[ ] Include cost class, external I/O flag, side-effect flag, security class.
[ ] Include enabled tenants or policy key.
[ ] Include docs section, SQL examples, DataFrame examples.
[ ] Include test fixture IDs and benchmark IDs where needed.
[ ] Include function version, introduced version, deprecation metadata.
[ ] Validate built-in, alias, and case collisions.
[ ] Deny built-in overrides unless explicitly approved.
[ ] Normalize unquoted function names to lowercase policy keys.
[ ] Authorize aliases exactly like canonical names.
[ ] Generate runtime registration plan from catalog+tenant policy.
[ ] Register functions before SQL/DataFrame planning.
[ ] Validate logical plan functions against catalog+policy.
[ ] Generate JSON/YAML manifest for LLM agents.
[ ] Generate Markdown docs for humans.
[ ] Provide CLI/API/product_information_schema discovery.
[ ] Diff catalog across releases and block unapproved breaking changes.
[ ] Emit startup registration report and runtime function-policy audit events.
```

[1]: https://docs.rs/datafusion/latest/datafusion/execution/trait.FunctionRegistry.html?utm_source=chatgpt.com "FunctionRegistry in datafusion::execution - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/execution/session_state/struct.SessionState.html?utm_source=chatgpt.com "SessionState in datafusion::execution::session_state - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/execution/context/struct.SessionContext.html?utm_source=chatgpt.com "SessionContext in datafusion::execution::context - Rust"
[4]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html?utm_source=chatgpt.com "ScalarUDFImpl in datafusion_expr - Rust"
[5]: https://datafusion.apache.org/user-guide/sql/information_schema.html?utm_source=chatgpt.com "Information Schema — Apache DataFusion documentation"
[6]: https://docs.rs/datafusion-functions?utm_source=chatgpt.com "datafusion_functions - Rust"


# C3.29 Higher-order function registry and discovery (DataFusion 54)

Higher-order UDFs (C1.19/C1.20) live in their **own registry namespace**, parallel to scalar/aggregate/window functions. Every registry, catalog, discovery, and policy surface in this chapter gains a fourth function family.

## C3.29.1 `FunctionRegistry` surface

`FunctionRegistry` (`datafusion-expr` `registry.rs`) adds four methods in DataFusion 54:

```rust id="hof-registry-surface"
fn higher_order_function_names(&self) -> HashSet<String>;
fn higher_order_function(&self, name: &str) -> Result<Arc<HigherOrderUDF>>;
fn register_higher_order_function(
    &mut self,
    function: Arc<HigherOrderUDF>,
) -> Result<Option<Arc<HigherOrderUDF>>>;   // returns previously registered impl
fn deregister_higher_order_function(
    &mut self,
    name: &str,
) -> Result<Option<Arc<HigherOrderUDF>>>;
```

Session-level registration mirrors `register_udf`:

```rust id="hof-session-register"
use datafusion::logical_expr::HigherOrderUDF;

let f = Arc::new(HigherOrderUDF::new_from_impl(ElementwiseCalibrate::new()));
ctx.register_higher_order_function(Arc::clone(&f));
// lookup is lowercase-normalized like other unquoted function names:
// SELECT ELEMENTWISE_CALIBRATE(...) resolves "elementwise_calibrate"
ctx.deregister_higher_order_function("elementwise_calibrate");
```

## C3.29.2 Planner and session discovery

- `ContextProvider::get_higher_order_meta(&self, name: &str) -> Option<Arc<HigherOrderUDF>>` and `ContextProvider::higher_order_function_names() -> Vec<String>` (`datafusion-expr` `planner.rs`) — custom SQL planners must implement these or lambda-taking calls fail to resolve.
- `Session::higher_order_functions(&self) -> &HashMap<String, Arc<HigherOrderUDF>>` (`datafusion-session` `session.rs`) — returns a **reference to the registry-owned map**. Custom `Session` implementations must store the map and return a reference to that storage; the signature cannot be satisfied by building a fresh map per call, so do not fake it with a leaked or cached-on-demand allocation.
- `TaskContext::new` (`datafusion-execution` `task.rs`) now takes the higher-order function map as a parameter, between the scalar and aggregate maps. Code constructing `TaskContext` directly (custom schedulers, distributed executors, test harnesses) must pass the session's map — or `HashMap::new()` when no higher-order functions are used:

```rust id="hof-task-context"
let task_ctx = TaskContext::new(
    task_id,
    session_id,
    session_config,
    scalar_functions,
    higher_order_functions, // NEW in 54; empty HashMap is fine
    aggregate_functions,
    window_functions,
    runtime,
);
```

## C3.29.3 Catalog and policy integration

Extend the C3 catalog artifacts with the new family:

```text id="hof-catalog-rules"
FunctionCatalogEntry.family gains: higher_order.
Registration plans (C3.12) emit register_higher_order_function calls for the family.
Collision validation (C3.11) must check higher_order_function_names() in addition
  to udfs()/udafs()/udwfs() — the namespaces are separate maps, but product policy
  should still forbid a scalar UDF and a higher-order UDF sharing one name.
Function-call extraction for policy validation (C3.20) must walk
  Expr::HigherOrderFunction (function name = .func.name()) AND recurse into
  Expr::Lambda bodies — scalar UDF calls inside lambda bodies are still calls
  and must pass the same allowlist checks.
Discovery surfaces (C3.10/C3.13/C3.14) list the built-ins from
  datafusion-functions-nested (array_transform/list_transform, array_filter,
  array_any_match) plus product higher-order functions.
```

---

# DataFusion Advanced — C4) Function package and plugin architecture

## C4.0 Objective

Define a **modular, governed, version-pinned package architecture** for large DataFusion calculation libraries:

```text id="c4-root"
workspace
  ├─ calc-core              # traits, manifests, policy, registration orchestration
  ├─ calc-arrow-kernels     # Arrow-native vector kernels, no DataFusion session dependency if possible
  ├─ calc-functions         # scalar + async scalar UDF wrappers
  ├─ calc-aggregates        # UDAF accumulators / GroupsAccumulator implementations
  ├─ calc-window            # UDWF / PartitionEvaluator implementations
  ├─ calc-table-functions   # UDTF + TableProvider adapters
  ├─ calc-manifest          # YAML/JSON catalog artifacts + generated Rust manifest constants
  ├─ calc-tests             # SQL/DataFrame/Arrow/property/golden test harnesses
  └─ app/query-engine       # SessionContext construction, tenant policy, package enablement
```

The attachment already recommends isolating DataFusion-specific code in a reusable library crate, with `functions.rs` holding UDF/UDAF registration, and keeping binary `main.rs` focused on runtime/config/transport glue. It also advises using the top-level `datafusion` crate for ordinary SQL/DataFrame applications, custom `TableProvider`s, UDF/UDAF/UDWF registration, catalog integration, and most logical-plan/expression construction, while depending directly on subcrates only when the crate boundary itself matters.  

DataFusion’s current top-level crate is the standard Rust entry point and advertises customization points for data sources, query languages, functions, operators, and more; the `datafusion-functions` crate explicitly describes function packages implemented through DataFusion’s extension API, including use cases such as controlling binary size and dialect-specific function availability. ([Docs.rs][1])

---

## C4.1 Core packaging principle

```text id="c4-principle"
Package functions by lifecycle domain, not by registration convenience.

Bad:
  query-core/src/functions.rs
    4,000 lines of mixed scalar, aggregate, window, table, policy, docs, tests

Good:
  packages expose FunctionPackage
  app selects packages by tenant/workload/features
  package registration is deterministic
  package metadata drives docs, tests, policy, and deployment
```

Primary invariant:

```text id="c4-invariant"
main.rs must never know individual function constructors.
SessionContext construction calls package registration orchestration.
Package orchestration reads manifest + tenant policy + feature flags.
```

---

## C4.2 Workspace layout

```text id="workspace-layout"
my-datafusion-engine/
  Cargo.toml
  Cargo.lock

  crates/
    calc-core/
      Cargo.toml
      src/
        lib.rs
        prelude.rs
        package.rs
        manifest.rs
        registry.rs
        policy.rs
        validation.rs
        errors.rs
        docs.rs
        discovery.rs

    calc-arrow-kernels/
      Cargo.toml
      src/
        lib.rs
        numeric/
          mod.rs
          safe_divide.rs
          weighted.rs
        strings/
          mod.rs
          normalize.rs
        nested/
          mod.rs
          vector.rs
        validation/
          mod.rs

    calc-functions/
      Cargo.toml
      src/
        lib.rs
        scalar/
          mod.rs
          normalize_api_gravity.rs
          density_to_api.rs
          safe_divide.rs
        async_scalar/
          mod.rs
          reference_lookup.rs
        register.rs

    calc-aggregates/
      Cargo.toml
      src/
        lib.rs
        weighted_avg.rs
        weighted_sulfur.rs
        online_variance.rs
        top_k.rs
        register.rs

    calc-window/
      Cargo.toml
      src/
        lib.rs
        rolling_zscore.rs
        rolling_stability.rs
        event_gap_score.rs
        register.rs

    calc-table-functions/
      Cargo.toml
      src/
        lib.rs
        scenario_parameters.rs
        calendar_table.rs
        reference_curve.rs
        providers/
          scenario_provider.rs
          reference_curve_provider.rs
        register.rs

    calc-manifest/
      Cargo.toml
      build.rs
      src/
        lib.rs
        generated.rs
      manifests/
        packages/
          engineering.yaml
          math.yaml
          external_io.yaml
        functions/
          normalize_api_gravity.yaml
          weighted_sulfur.yaml
          rolling_stability.yaml

    calc-tests/
      Cargo.toml
      src/
        lib.rs
        harness.rs
        fixtures.rs
      fixtures/
        arrow/
        csv/
        parquet/
      sql/
        scalar/
        aggregate/
        window/
        table/
      golden/
        schemas/
        explain/
        output/

    query-engine/
      Cargo.toml
      src/
        lib.rs
        engine.rs
        tenant.rs
        config.rs

    query-cli/
      Cargo.toml
      src/main.rs

    query-server/
      Cargo.toml
      src/main.rs
```

### Design rules

```text id="layout-rules"
calc-core:
  no domain formulas
  owns package trait, manifest structs, policy, validation, registration orchestration

calc-arrow-kernels:
  no SessionContext dependency
  preferably no DataFusion dependency except datafusion::arrow re-export compatibility layer
  pure Arrow kernels and reference-safe helpers

calc-functions:
  scalar/async ScalarUDFImpl wrappers over kernels

calc-aggregates:
  UDAF definitions, Accumulator/GroupsAccumulator implementations

calc-window:
  WindowUDF/PartitionEvaluator implementations

calc-table-functions:
  UDTFs and domain TableProvider adapters

calc-manifest:
  machine-readable catalog source
  generated Rust constants and validation

calc-tests:
  shared test harness and fixtures
  no production registration side effects

query-engine:
  builds SessionContext
  selects packages according to policy
  registers packages deterministically

query-cli/query-server:
  transport/runtime only
```

---

## C4.3 Workspace `Cargo.toml`

```toml id="workspace-cargo"
[workspace]
resolver = "2"
members = [
  "crates/calc-core",
  "crates/calc-arrow-kernels",
  "crates/calc-functions",
  "crates/calc-aggregates",
  "crates/calc-window",
  "crates/calc-table-functions",
  "crates/calc-manifest",
  "crates/calc-tests",
  "crates/query-engine",
  "crates/query-cli",
  "crates/query-server",
]

[workspace.dependencies]
datafusion = { version = "=54.1.0" }
tokio = { version = "1.48", features = ["rt-multi-thread", "macros"] }
futures = "0.3"
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
semver = { version = "1", features = ["serde"] }
thiserror = "2"
tracing = "0.1"
once_cell = "1"
```

### Version policy

```text id="version-policy"
Production:
  pin datafusion exactly
  pin any direct datafusion-* subcrate exactly
  prefer datafusion::arrow re-exports
  run cargo tree -d after upgrades
  keep Cargo.lock committed for binaries/services

Extension crates:
  use top-level datafusion first
  use datafusion-expr / datafusion-common / datafusion-physical-plan directly only when trait/API boundaries require it
```

The attachment explicitly warns that direct `datafusion-*` subcrates should be version-aligned, that top-level `datafusion` should be preferred for most applications, and that mixing subcrate versions is an avoidable API/ABI risk. 

---

# C4.4 Crate responsibilities

## C4.4.1 `calc-core`

### Purpose

Common package/control-plane crate. No formula logic.

```rust id="calc-core-lib"
pub mod package;
pub mod manifest;
pub mod policy;
pub mod registry;
pub mod validation;
pub mod discovery;
pub mod errors;

pub mod prelude {
    pub use crate::package::{
        FunctionPackage, PackageOptions, RegistrationReport, PackageId,
        PackageRegistry,
    };
    pub use crate::manifest::{
        FunctionCatalogEntry, PackageManifest, FunctionFamily,
        FunctionStatus,
    };
    pub use crate::policy::{
        TenantFunctionPolicy, CostClass, SecurityClass,
    };
}
```

### Dependencies

```toml id="calc-core-cargo"
[package]
name = "calc-core"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"

[dependencies]
datafusion = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
semver = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

### Agent rules

```text id="calc-core-rules"
calc-core owns:
  package trait
  package ordering
  manifest validation
  policy checking
  registration report shape
  discovery read models

calc-core must not own:
  domain formulas
  business-specific constants
  tenant secrets
  external clients
```

---

## C4.4.2 `calc-arrow-kernels`

### Purpose

Pure Arrow/batch kernels, reusable by scalar UDFs, UDAFs, tests, Python references, and preprocessing code.

```rust id="arrow-kernel-example"
use datafusion::arrow::array::{Array, Float64Array, Float64Builder};
use datafusion::common::{DataFusionError, Result};

pub fn safe_divide_f64_arrays(
    numerator: &Float64Array,
    denominator: &Float64Array,
) -> Result<Float64Array> {
    if numerator.len() != denominator.len() {
        return Err(DataFusionError::Execution(format!(
            "safe_divide_f64_arrays length mismatch: {} vs {}",
            numerator.len(),
            denominator.len(),
        )));
    }

    let mut out = Float64Builder::with_capacity(numerator.len());

    for i in 0..numerator.len() {
        if numerator.is_null(i) || denominator.is_null(i) || denominator.value(i) == 0.0 {
            out.append_null();
        } else {
            out.append_value(numerator.value(i) / denominator.value(i));
        }
    }

    Ok(out.finish())
}
```

### Dependencies

```toml id="calc-arrow-kernels-cargo"
[package]
name = "calc-arrow-kernels"
version = "0.1.0"
edition = "2024"
rust-version = "1.88"

[dependencies]
datafusion = { workspace = true } # for datafusion::arrow + DataFusionError; optional facade alternative
```

### Best-practice split

```text id="kernel-split"
Kernel crate:
  accepts concrete Arrow arrays
  returns Arrow arrays / ScalarValue-compatible values
  no SessionContext
  no SQL
  no tenant policy
  no registration

UDF wrapper crate:
  handles ColumnarValue
  handles signature/coercion/return type
  calls kernel
```

---

## C4.4.3 `calc-functions`

### Purpose

Scalar and async scalar UDF wrappers.

```rust id="calc-functions-lib"
pub mod scalar;
pub mod async_scalar;
pub mod register;

pub use register::ScalarFunctionPackage;
```

```rust id="scalar-mod"
pub mod normalize_api_gravity;
pub mod density_to_api;
pub mod safe_divide;

pub use normalize_api_gravity::NormalizeApiGravity;
pub use density_to_api::DensityToApi;
pub use safe_divide::SafeDivide;
```

### UDF wrapper pattern

```rust id="scalar-wrapper-pattern"
#[derive(Debug)]
pub struct SafeDivide {
    signature: Signature,
    documentation: Documentation,
}

impl SafeDivide {
    pub fn new() -> Self {
        Self {
            signature: Signature::exact(
                vec![DataType::Float64, DataType::Float64],
                Volatility::Immutable,
            ),
            documentation: safe_divide_docs(),
        }
    }
}

impl ScalarUDFImpl for SafeDivide {
    // DataFusion 54: no `as_any` — `Any` is a supertrait of `ScalarUDFImpl`.
    fn name(&self) -> &str { "safe_divide" }

    fn signature(&self) -> &Signature { &self.signature }

    fn return_type(&self, _arg_types: &[DataType]) -> datafusion::error::Result<DataType> {
        Ok(DataType::Float64)
    }

    fn invoke_with_args(
        &self,
        args: ScalarFunctionArgs,
    ) -> datafusion::error::Result<ColumnarValue> {
        // 1. validate arity
        // 2. scalar/scalar fast path
        // 3. scalar/array and array/scalar handling if needed
        // 4. array/array uses calc_arrow_kernels
        todo!()
    }

    fn documentation(&self) -> Option<&Documentation> {
        Some(&self.documentation)
    }
}
```

---

## C4.4.4 `calc-aggregates`

### Purpose

UDAF definitions and state machines.

```rust id="calc-aggregates-lib"
pub mod weighted_avg;
pub mod weighted_sulfur;
pub mod online_variance;
pub mod register;

pub use register::AggregateFunctionPackage;
```

### Package exports

```rust id="aggregate-exports"
pub fn aggregate_udfs() -> Vec<AggregateUDF> {
    vec![
        weighted_avg::weighted_avg_udaf(),
        weighted_sulfur::weighted_sulfur_udaf(),
        online_variance::online_variance_udaf(),
    ]
}
```

### Agent rule

```text id="aggregate-package-rule"
UDAF crate owns:
  Accumulator implementation
  optional GroupsAccumulator implementation
  state schema
  merge semantics
  aggregate SQL examples
  partition-equivalence tests
```

---

## C4.4.5 `calc-window`

### Purpose

Window UDFs and `PartitionEvaluator` implementations.

```rust id="calc-window-lib"
pub mod rolling_zscore;
pub mod rolling_stability;
pub mod event_gap_score;
pub mod register;

pub use register::WindowFunctionPackage;
```

### Agent rule

```text id="window-package-rule"
UDWF crate owns:
  window signature
  return type
  partition evaluator factory
  ROWS/RANGE/GROUPS assumptions
  deterministic ORDER BY examples
  frame-boundary tests
```

---

## C4.4.6 `calc-table-functions`

### Purpose

UDTFs plus relation/table providers for parameterized generated tables.

```rust id="calc-table-functions-lib"
pub mod scenario_parameters;
pub mod calendar_table;
pub mod reference_curve;
pub mod providers;
pub mod register;

pub use register::TableFunctionPackage;
```

### UDTF vs provider split

```text id="udtf-provider-split"
UDTF object:
  validates function arguments
  creates TableProvider cheaply
  exposes SQL-callable table function

Provider:
  owns schema
  scan planning
  projection/filter/limit pushdown
  actual data generation/read
```

---

## C4.4.7 `calc-manifest`

### Purpose

Machine-readable manifest source and generated Rust constants.

```text id="manifest-layout"
calc-manifest/
  manifests/
    packages/
      math.yaml
      strings.yaml
      engineering.yaml
      external_io.yaml
      experimental.yaml
    functions/
      safe_divide.yaml
      normalize_api_gravity.yaml
      weighted_sulfur.yaml
  build.rs
  src/generated.rs
```

### Build-script pattern

```rust id="manifest-build-rs"
fn main() {
    println!("cargo:rerun-if-changed=manifests/packages");
    println!("cargo:rerun-if-changed=manifests/functions");

    // 1. load YAML
    // 2. validate schema
    // 3. validate collisions
    // 4. emit generated.rs with static manifest data or include_str! references
}
```

### Agent rule

```text id="manifest-rule"
Manifest crate is the governance source of truth.
Registration code must be validated against manifest, not copied manually from docs.
```

---

## C4.4.8 `calc-tests`

### Purpose

Central test harness for package-level and end-to-end function validation.

```rust id="calc-tests-harness"
pub struct FunctionTestHarness {
    pub ctx: SessionContext,
    pub package_registry: PackageRegistry,
}

impl FunctionTestHarness {
    pub async fn new_with_packages(
        packages: &[Box<dyn FunctionPackage>],
        options: PackageOptions,
    ) -> datafusion::error::Result<Self> {
        let ctx = SessionContext::new();
        let mut registry = PackageRegistry::new();

        for pkg in packages {
            registry.add(pkg.box_clone());
        }

        registry.register_all(&ctx, &options)?;

        Ok(Self {
            ctx,
            package_registry: registry,
        })
    }
}
```

### Test corpus

```text id="test-corpus"
direct Arrow kernel tests
ScalarUDFImpl direct invocation tests
SQL planning tests
SQL execution golden tests
DataFrame helper tests
schema/nullability tests
policy allow/deny tests
package-order tests
feature-flag compile tests
manifest validation tests
benchmark smoke tests
```

---

# C4.5 `FunctionPackage` trait

## C4.5.1 Core trait

```rust id="function-package-trait"
use std::fmt::Debug;
use datafusion::prelude::SessionContext;

pub trait FunctionPackage: Debug + Send + Sync {
    fn id(&self) -> PackageId;
    fn name(&self) -> &'static str;
    fn version(&self) -> semver::Version;

    fn dependencies(&self) -> Vec<PackageId> {
        Vec::new()
    }

    fn feature_flags(&self) -> Vec<&'static str> {
        Vec::new()
    }

    fn manifest(&self) -> PackageManifest;

    fn validate(&self, options: &PackageOptions) -> Result<(), PackageError>;

    fn register(
        &self,
        ctx: &SessionContext,
        options: &PackageOptions,
    ) -> datafusion::error::Result<PackageRegistrationReport>;
}
```

## C4.5.2 Package ID

```rust id="package-id"
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct PackageId {
    pub namespace: String,
    pub name: String,
}

impl PackageId {
    pub fn new(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn key(&self) -> String {
        format!("{}.{}", self.namespace, self.name)
    }
}
```

## C4.5.3 Package options

```rust id="package-options"
#[derive(Debug, Clone)]
pub struct PackageOptions {
    pub tenant_id: String,
    pub enabled_features: std::collections::BTreeSet<String>,
    pub function_policy: TenantFunctionPolicy,
    pub register_experimental: bool,
    pub register_deprecated: bool,
    pub allow_external_io: bool,
    pub strict_collision_checks: bool,
    pub fail_on_skipped_required: bool,
}
```

## C4.5.4 Registration report

```rust id="package-registration-report"
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PackageRegistrationReport {
    pub package_id: String,
    pub package_version: String,
    pub registered: Vec<RegisteredFunction>,
    pub skipped: Vec<SkippedFunction>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RegisteredFunction {
    pub canonical_name: String,
    pub family: FunctionFamily,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SkippedFunction {
    pub canonical_name: String,
    pub family: FunctionFamily,
    pub reason: String,
}
```

### Agent rules

```text id="function-package-agent-rules"
Every package must declare:
  id
  version
  dependencies
  feature flags
  manifest
  deterministic register(ctx, options)

register(...) must:
  apply policy
  return report
  not panic
  not perform expensive runtime initialization unless explicit
```

---

# C4.6 Package registry and deterministic ordering

## C4.6.1 Registry

```rust id="package-registry"
#[derive(Debug, Default)]
pub struct PackageRegistry {
    packages: std::collections::BTreeMap<PackageId, Box<dyn FunctionPackage>>,
}

impl PackageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, package: Box<dyn FunctionPackage>) -> Result<(), PackageError> {
        let id = package.id();

        if self.packages.contains_key(&id) {
            return Err(PackageError::DuplicatePackage(id.key()));
        }

        self.packages.insert(id, package);
        Ok(())
    }

    pub fn register_all(
        &self,
        ctx: &SessionContext,
        options: &PackageOptions,
    ) -> datafusion::error::Result<Vec<PackageRegistrationReport>> {
        let ordered = self.resolve_order()
            .map_err(|e| datafusion::error::DataFusionError::External(Box::new(e)))?;

        let mut reports = Vec::new();

        for id in ordered {
            let package = self.packages.get(&id).expect("validated package id");
            package.validate(options)
                .map_err(|e| datafusion::error::DataFusionError::External(Box::new(e)))?;
            reports.push(package.register(ctx, options)?);
        }

        Ok(reports)
    }
}
```

## C4.6.2 Deterministic ordering

Ordering must be stable across machines and builds.

```text id="ordering-rules"
1. Construct dependency graph.
2. Detect missing dependencies.
3. Detect dependency cycles.
4. Topologically sort.
5. Within same dependency level, sort by PackageId key.
6. Apply explicit override priority only through manifest.
7. Emit registration order in startup report.
```

## C4.6.3 Topological sort pseudocode

```rust id="toposort-pseudocode"
impl PackageRegistry {
    fn resolve_order(&self) -> Result<Vec<PackageId>, PackageError> {
        // Use BTreeMap/BTreeSet for deterministic iteration.
        let mut result = Vec::new();
        let mut temporary = BTreeSet::new();
        let mut permanent = BTreeSet::new();

        for id in self.packages.keys() {
            self.visit(id, &mut temporary, &mut permanent, &mut result)?;
        }

        Ok(result)
    }

    fn visit(
        &self,
        id: &PackageId,
        temporary: &mut BTreeSet<PackageId>,
        permanent: &mut BTreeSet<PackageId>,
        result: &mut Vec<PackageId>,
    ) -> Result<(), PackageError> {
        if permanent.contains(id) {
            return Ok(());
        }

        if !temporary.insert(id.clone()) {
            return Err(PackageError::DependencyCycle(id.key()));
        }

        let package = self.packages
            .get(id)
            .ok_or_else(|| PackageError::MissingPackage(id.key()))?;

        let mut deps = package.dependencies();
        deps.sort();

        for dep in deps {
            if !self.packages.contains_key(&dep) {
                return Err(PackageError::MissingDependency {
                    package: id.key(),
                    dependency: dep.key(),
                });
            }
            self.visit(&dep, temporary, permanent, result)?;
        }

        temporary.remove(id);
        permanent.insert(id.clone());
        result.push(id.clone());

        Ok(())
    }
}
```

### Agent invariant

```text id="ordering-invariant"
Package registration order must never depend on HashMap iteration order.
Use BTreeMap/BTreeSet or explicit sorted Vec.
```

---

# C4.7 Registration patterns

## C4.7.1 `register_all(ctx)`

Use for test/dev internal engines where all compiled packages are safe.

```rust id="register-all"
pub fn register_all(ctx: &SessionContext) -> datafusion::error::Result<Vec<PackageRegistrationReport>> {
    let options = PackageOptions::internal_dev_all_enabled();

    let mut registry = PackageRegistry::new();
    registry.add(Box::new(MathPackage::default()))?;
    registry.add(Box::new(StringPackage::default()))?;
    registry.add(Box::new(EngineeringPackage::default()))?;
    registry.add(Box::new(AggregatesPackage::default()))?;
    registry.add(Box::new(WindowPackage::default()))?;

    registry.register_all(ctx, &options)
}
```

### Use when

```text id="register-all-use"
unit/integration tests
internal CLI
offline batch jobs
developer sandbox
```

### Avoid when

```text id="register-all-avoid"
public SQL
multi-tenant service
external-I/O package present
experimental package present
```

---

## C4.7.2 `register_domain_package(ctx, PackageOptions)`

Use for production.

```rust id="register-domain-package"
pub fn register_domain_package(
    ctx: &SessionContext,
    package: &dyn FunctionPackage,
    options: &PackageOptions,
) -> datafusion::error::Result<PackageRegistrationReport> {
    package
        .validate(options)
        .map_err(|e| datafusion::error::DataFusionError::External(Box::new(e)))?;

    package.register(ctx, options)
}
```

### Engine construction

```rust id="engine-construction"
pub async fn build_engine_for_tenant(
    tenant: &TenantConfig,
    package_registry: &PackageRegistry,
) -> datafusion::error::Result<SessionContext> {
    let ctx = configured_context(&tenant.runtime)?;

    register_sources(&ctx, tenant).await?;
    register_object_stores(&ctx, tenant)?;

    let options = PackageOptions {
        tenant_id: tenant.id.clone(),
        enabled_features: tenant.enabled_features.clone(),
        function_policy: tenant.function_policy.clone(),
        register_experimental: tenant.allow_experimental,
        register_deprecated: tenant.allow_deprecated,
        allow_external_io: tenant.allow_external_io,
        strict_collision_checks: true,
        fail_on_skipped_required: true,
    };

    let reports = package_registry.register_all(&ctx, &options)?;
    emit_registration_reports(&reports);

    Ok(ctx)
}
```

### Agent rules

```text id="registration-agent-rules"
register_all(ctx) is not production policy.
Production registration must use PackageOptions.
PackageOptions must include tenant policy and enabled features.
Registration report must be logged/audited.
```

---

## C4.7.3 Explicit family registration inside package

```rust id="family-registration"
impl FunctionPackage for EngineeringPackage {
    fn register(
        &self,
        ctx: &SessionContext,
        options: &PackageOptions,
    ) -> datafusion::error::Result<PackageRegistrationReport> {
        let mut report = PackageRegistrationReport::new(self);

        self.register_scalar(ctx, options, &mut report)?;
        self.register_aggregate(ctx, options, &mut report)?;
        self.register_window(ctx, options, &mut report)?;
        self.register_table(ctx, options, &mut report)?;

        Ok(report)
    }
}

impl EngineeringPackage {
    fn register_scalar(
        &self,
        ctx: &SessionContext,
        options: &PackageOptions,
        report: &mut PackageRegistrationReport,
    ) -> datafusion::error::Result<()> {
        self.register_one_scalar(
            ctx,
            options,
            ScalarUDF::from(NormalizeApiGravity::new()),
            report,
        )?;

        self.register_one_scalar(
            ctx,
            options,
            ScalarUDF::from(DensityToApi::new()),
            report,
        )?;

        Ok(())
    }
}
```

---

# C4.8 Feature flags

## C4.8.1 Feature taxonomy

```text id="feature-taxonomy"
math:
  pure deterministic numeric UDFs / Expr helpers

strings:
  text normalization / parsing / string-domain functions

engineering:
  domain formulas, units, refinery/simulation calculations

external_io:
  async scalar UDFs / external reference lookups

python_bridge:
  PyO3/subprocess/Python-backed compatibility functions

experimental:
  unstable functions, prototypes, performance experiments
```

## C4.8.2 Workspace feature flags

```toml id="workspace-feature-flags"
# crates/calc-functions/Cargo.toml

[features]
default = ["math", "strings"]
math = []
strings = []
engineering = ["dep:calc-arrow-kernels"]
external_io = ["dep:reqwest", "dep:tokio"]
python_bridge = ["dep:pyo3"]
experimental = []
all = [
  "math",
  "strings",
  "engineering",
  "external_io",
  "python_bridge",
  "experimental",
]

[dependencies]
datafusion = { workspace = true }
calc-core = { path = "../calc-core" }
calc-arrow-kernels = { path = "../calc-arrow-kernels", optional = true }

reqwest = { version = "0.12", optional = true, features = ["json"] }
pyo3 = { version = "0.23", optional = true, features = ["auto-initialize"] }
tokio = { workspace = true, optional = true }
```

## C4.8.3 Conditional module compilation

```rust id="feature-mods"
#[cfg(feature = "math")]
pub mod math;

#[cfg(feature = "strings")]
pub mod strings;

#[cfg(feature = "engineering")]
pub mod engineering;

#[cfg(feature = "external_io")]
pub mod external_io;

#[cfg(feature = "python_bridge")]
pub mod python_bridge;

#[cfg(feature = "experimental")]
pub mod experimental;
```

## C4.8.4 Conditional registration

```rust id="conditional-registration"
pub fn add_compiled_packages(registry: &mut PackageRegistry) -> Result<(), PackageError> {
    #[cfg(feature = "math")]
    registry.add(Box::new(math::MathPackage::default()))?;

    #[cfg(feature = "strings")]
    registry.add(Box::new(strings::StringPackage::default()))?;

    #[cfg(feature = "engineering")]
    registry.add(Box::new(engineering::EngineeringPackage::default()))?;

    #[cfg(feature = "external_io")]
    registry.add(Box::new(external_io::ExternalIoPackage::default()))?;

    #[cfg(feature = "python_bridge")]
    registry.add(Box::new(python_bridge::PythonBridgePackage::default()))?;

    #[cfg(feature = "experimental")]
    registry.add(Box::new(experimental::ExperimentalPackage::default()))?;

    Ok(())
}
```

## C4.8.5 Compile-time vs runtime flags

| Flag type                  | Example                             | Enforced by     | Purpose                                   |
| -------------------------- | ----------------------------------- | --------------- | ----------------------------------------- |
| compile-time feature       | `external_io`                       | Cargo           | binary size, dependencies, attack surface |
| runtime package enablement | `enabled_packages: ["engineering"]` | config          | tenant/workload deployment                |
| tenant function policy     | `allow: weighted_sulfur`            | plan validation | authorization                             |
| per-function status        | `experimental`                      | manifest/policy | lifecycle                                 |
| resource class             | `max_cost_class: medium`            | query admission | cost control                              |

### Agent rule

```text id="feature-agent-rule"
Cargo feature controls whether code exists.
PackageOptions controls whether package registers.
Tenant policy controls whether function may be planned/executed.
Do not conflate the three.
```

---

# C4.9 Package manifest

## C4.9.1 Package-level manifest

```yaml id="package-yaml"
package:
  id: refinery.engineering
  name: engineering
  namespace: refinery
  version: 0.4.0
  status: active
  owner: process_modeling

features:
  required:
    - engineering
  optional:
    - experimental
  incompatible:
    - public_minimal

dependencies:
  packages:
    - core.math
  crates:
    - calc-arrow-kernels

registration:
  order_hint: 300
  default_enabled: false
  requires_policy: true

security:
  max_default_cost_class: medium
  external_io: false
  side_effects: false

functions:
  scalar:
    - normalize_api_gravity
    - density_to_api
  aggregate:
    - weighted_sulfur
  window:
    - rolling_stability_score
  table:
    - scenario_parameters
```

## C4.9.2 Function entry linkage

```yaml id="function-linkage"
function:
  canonical_name: weighted_sulfur
  package_id: refinery.engineering
  family: aggregate
  rust_constructor: calc_aggregates::weighted_sulfur::weighted_sulfur_udaf
  required_feature: engineering
  required_policy_class: pure_deterministic
```

## C4.9.3 Deployment manifest

```yaml id="deployment-manifest"
deployment:
  environment: production
  engine_version: 2026.05.24
  datafusion_version: 54.1.0

packages:
  enabled:
    - core.math
    - core.strings
    - refinery.engineering
  disabled:
    - external.reference_lookup
    - experimental.prototype

tenants:
  internal_refinery_modeling:
    packages:
      - refinery.engineering
    function_allowlist:
      - normalize_api_gravity
      - density_to_api
      - weighted_sulfur
    max_cost_class: high
    allow_external_io: false

  public_sql:
    packages:
      - core.math
      - core.strings
    function_allowlist:
      - safe_divide
    max_cost_class: low
    allow_external_io: false

external_credentials:
  reference_lookup:
    enabled: false
    secret_ref: null
```

---

# C4.10 Static linking model

## C4.10.1 Preferred production model

```text id="static-model"
All trusted packages compiled into binary.
Cargo features determine compiled package set.
Runtime config determines enabled package set.
Tenant policy determines allowed function set.
```

### Why static linking

```text id="static-benefits"
reproducible builds
Cargo/audit visibility
Rust type checking across DataFusion APIs
no unstable Rust dynamic ABI boundary
simpler deployment
easier CI testing
easier security review
works with exact DataFusion version pin
```

### Static registry construction

```rust id="static-registry"
pub fn build_static_package_registry() -> Result<PackageRegistry, PackageError> {
    let mut registry = PackageRegistry::new();

    add_compiled_packages(&mut registry)?;

    registry.validate_package_graph()?;
    registry.validate_no_collisions()?;

    Ok(registry)
}
```

### Agent rules

```text id="static-agent-rules"
Use static linking for production.
Compile packages into service image.
Do not load arbitrary user native code.
Treat package enablement as config, not code loading.
```

---

# C4.11 Dynamic plugin model

## C4.11.1 Default stance

```text id="dynamic-default"
Dynamic native plugin loading is not the default production architecture.
Use only for trusted, reviewed, version-locked plugins with explicit sandboxing or process isolation.
```

### Why

```text id="dynamic-risks"
Rust ABI is not stable for arbitrary trait object loading.
DataFusion trait APIs are version-sensitive.
Arrow type identity must match exactly.
Native plugin can execute arbitrary process code.
Tenant isolation is difficult.
Crash/UB in plugin can crash engine.
Security review and signing are mandatory.
```

## C4.11.2 Acceptable plugin models

| Model                            |       Safety | Use                                           |
| -------------------------------- | -----------: | --------------------------------------------- |
| static Rust crates               |      highest | production trusted packages                   |
| dynamically loaded C ABI adapter |       medium | trusted internal plugin with strict ABI       |
| WASM sandbox                     |  medium/high | pure deterministic sandboxable functions      |
| subprocess/microservice          |  medium/high | external language/runtime isolation           |
| Python bridge in-process         |   low/medium | internal scientific compatibility, not public |
| arbitrary user native `.so`      | unacceptable | do not allow                                  |

## C4.11.3 C ABI plugin sketch

```rust id="c-abi-plugin"
#[repr(C)]
pub struct CalcPluginDescriptor {
    pub abi_version: u32,
    pub plugin_name: *const std::os::raw::c_char,
    pub plugin_version: *const std::os::raw::c_char,
    pub manifest_json: *const std::os::raw::c_char,
}

#[no_mangle]
pub extern "C" fn calc_plugin_descriptor() -> CalcPluginDescriptor {
    // Return static descriptor only.
    // Actual registration should happen through a narrow host-mediated API.
    todo!()
}
```

### Required gates

```text id="dynamic-gates"
plugin signed
plugin built against exact host ABI version
plugin manifest validated before loading
plugin loaded in restricted process if possible
plugin cannot access tenant secrets unless explicitly granted
plugin function set is policy-controlled
plugin has crash isolation or service-level rollback
```

## C4.11.4 WASM/plugin alternative

```text id="wasm-model"
WASM function plugin:
  host passes Arrow buffers or serialized scalar batches
  plugin returns Arrow-compatible output
  no direct filesystem/network unless WASI permissions granted
  deterministic fuel/time/memory limits possible
```

### Use when

```text id="wasm-use"
third-party pure functions
tenant-authored formulas
sandboxing required
lower performance acceptable
FFI safety more important than maximum speed
```

## C4.11.5 Subprocess/plugin alternative

```text id="subprocess-model"
DataFusion async UDF / TableProvider
  -> local sidecar process over Arrow IPC / Flight / gRPC
  -> external runtime isolation
  -> crash containment
  -> explicit timeout/retry/circuit breaker
```

### Use when

```text id="subprocess-use"
Python/SciPy/SymPy compatibility
proprietary model runtime
untrusted-ish plugin code
large dependencies unsuitable for main service
external resource isolation required
```

### Agent rules

```text id="dynamic-agent-rules"
Do not propose arbitrary user-supplied native plugins.
Prefer static Rust packages.
Use WASM or subprocess for sandboxing.
If dynamic native loading is unavoidable, require exact ABI/version/signature/manifest policy.
```

---

# C4.12 External I/O package architecture

## C4.12.1 Package split

```text id="external-io-split"
external-io package
  ├─ async scalar UDF wrappers
  ├─ client trait
  ├─ cache trait
  ├─ credentials policy
  ├─ timeout/retry/circuit breaker
  ├─ tenant-scoped construction
  └─ disabled by default
```

## C4.12.2 Client injection

```rust id="external-client-injection"
pub trait ReferenceDataClient: Send + Sync {
    fn name(&self) -> &'static str;

    fn lookup_price<'a>(
        &'a self,
        symbol: &'a str,
        date: chrono::NaiveDate,
    ) -> futures::future::BoxFuture<'a, datafusion::error::Result<Option<f64>>>;
}

#[derive(Clone)]
pub struct ExternalIoPackage<C> {
    client: std::sync::Arc<C>,
    cache: std::sync::Arc<dyn ReferenceCache>,
}

impl<C> ExternalIoPackage<C>
where
    C: ReferenceDataClient + 'static,
{
    pub fn new(client: std::sync::Arc<C>, cache: std::sync::Arc<dyn ReferenceCache>) -> Self {
        Self { client, cache }
    }
}
```

## C4.12.3 Tenant-aware construction

```rust id="external-tenant-construction"
pub fn build_external_io_package(
    tenant: &TenantConfig,
    secrets: &SecretResolver,
) -> Option<Box<dyn FunctionPackage>> {
    if !tenant.function_policy.allow_external_io {
        return None;
    }

    let credentials = secrets.resolve(&tenant.external_reference_secret_ref?)?;
    let client = Arc::new(HttpReferenceDataClient::new(credentials));
    let cache = Arc::new(TenantScopedReferenceCache::new(tenant.id.clone()));

    Some(Box::new(ExternalIoPackage::new(client, cache)))
}
```

### Agent rules

```text id="external-agent-rules"
external_io package:
  compile-time gated
  runtime disabled by default
  tenant-scoped
  policy-gated
  metrics-heavy
  timeout/circuit-breaker required
  never available in public SQL by default
```

---

# C4.13 Python bridge package architecture

## C4.13.1 Use cases

```text id="python-bridge-use"
reference compatibility with SciPy/SymPy/NumPy
differential validation
temporary migration path from Python formulas
low-throughput internal calculations
model code not yet ported to Rust
```

## C4.13.2 Risk profile

```text id="python-risks"
GIL contention
runtime initialization complexity
dependency packaging
copy/serialization cost
crash propagation if in-process
non-deterministic dependency environment
harder sandboxing
```

## C4.13.3 Recommended pattern

```text id="python-recommended"
Production high-throughput:
  port kernel to Rust/Arrow

Internal compatibility:
  subprocess sidecar using Arrow IPC/Flight

In-process PyO3:
  only for trusted internal tools
  feature-gated python_bridge
  explicit init/shutdown policy
  benchmark and memory profile
```

## C4.13.4 Package feature gate

```toml id="python-bridge-cargo"
[features]
python_bridge = ["dep:pyo3", "dep:arrow-pyarrow"]

[dependencies]
pyo3 = { version = "0.23", optional = true, features = ["auto-initialize"] }
arrow-pyarrow = { version = "0.3", optional = true }
```

### Agent rule

```text id="python-agent-rule"
Do not put Python bridge functions in the same package as pure Rust functions.
Keep python_bridge isolated, optional, policy-gated, and benchmarked.
```

---

# C4.14 Package dependency graph patterns

## C4.14.1 Typical graph

```text id="package-graph"
core.math
  └─ no deps

core.strings
  └─ no deps

refinery.engineering
  ├─ core.math
  └─ calc-arrow-kernels

refinery.aggregates
  ├─ core.math
  └─ refinery.engineering

refinery.windows
  ├─ refinery.aggregates
  └─ core.math

external.reference_lookup
  ├─ core.strings
  └─ external_io runtime client
```

## C4.14.2 Dependency rules

```text id="dependency-rules"
No cycles.
No package may depend on experimental unless it is experimental.
External I/O package cannot be dependency of pure package.
Python bridge package cannot be dependency of pure package.
Core package cannot depend on domain package.
Domain packages may depend on core packages.
Registration order follows dependency graph.
```

## C4.14.3 Validation

```rust id="dependency-validation"
pub fn validate_package_dependencies(registry: &PackageRegistry) -> Result<(), PackageError> {
    registry.validate_no_missing_dependencies()?;
    registry.validate_no_cycles()?;
    registry.validate_no_forbidden_dependency_edges(&[
        ForbiddenEdge::from_risk_to_pure(SecurityClass::ExternalRead),
        ForbiddenEdge::from_feature_to_core("experimental"),
        ForbiddenEdge::from_feature_to_core("python_bridge"),
    ])?;
    Ok(())
}
```

---

# C4.15 Package-level policy

## C4.15.1 Package policy record

```yaml id="package-policy-record"
package_policy:
  package_id: refinery.engineering
  default_enabled: false
  allowed_tenants:
    - internal_refinery_modeling
    - batch_etl
  denied_tenants:
    - public_sql
  max_cost_class: high
  external_io: false
  experimental: false
  functions:
    allow:
      - normalize_api_gravity
      - weighted_sulfur
    deny:
      - prototype_flash_correlation
```

## C4.15.2 Policy precedence

```text id="policy-precedence"
deny function exact
  > deny package
  > deny risk class
  > allow function exact
  > allow package
  > allow family
  > default deny
```

## C4.15.3 PackageOptions construction

```rust id="package-options-construction"
impl PackageOptions {
    pub fn from_tenant(tenant: &TenantConfig) -> Self {
        Self {
            tenant_id: tenant.id.clone(),
            enabled_features: tenant.enabled_features.clone(),
            function_policy: tenant.function_policy.clone(),
            register_experimental: tenant.flags.experimental_functions,
            register_deprecated: tenant.flags.deprecated_functions,
            allow_external_io: tenant.flags.external_io_functions,
            strict_collision_checks: true,
            fail_on_skipped_required: tenant.flags.fail_on_skipped_required,
        }
    }
}
```

---

# C4.16 Collision and override control

## C4.16.1 Collision domains

```text id="collision-domains"
function canonical names
function aliases
built-in DataFusion function names
package IDs
feature flag names
manifest IDs
SQL table function names
Expr helper Rust names
```

## C4.16.2 Package collision policy

```yaml id="package-collision-policy"
collision_policy:
  duplicate_package_id: deny
  duplicate_function_name: deny
  duplicate_alias: deny
  alias_overrides_builtin: deny
  canonical_overrides_builtin: deny_by_default
  allowed_overrides:
    - function: regexp_match
      package: dialect.postgres_compat
      reason: product dialect override
      tests:
        - regexp_match.postgres_compat
```

## C4.16.3 Deterministic override syntax

```rust id="override-syntax"
pub struct OverrideDeclaration {
    pub function_name: String,
    pub original_owner: FunctionOwner,
    pub new_owner: FunctionOwner,
    pub reason: String,
    pub approved: bool,
}
```

### Agent rules

```text id="collision-agent"
No package may override another package by registration order accident.
Every override must be declared.
Every alias must pass collision validation.
Feature flags must not silently change function semantics without manifest diff.
```

---

# C4.17 Discovery and docs generation per package

## C4.17.1 Package docs layout

```text id="package-docs-layout"
docs/functions/
  packages/
    core.math.md
    core.strings.md
    refinery.engineering.md
    external.reference_lookup.md
  scalar/
  aggregate/
  window/
  table/
  experimental/
```

## C4.17.2 CLI

```bash id="package-cli"
calc-package list

calc-package describe refinery.engineering

calc-package functions refinery.engineering \
  --tenant internal_refinery_modeling \
  --format table

calc-package export-manifest refinery.engineering \
  --format json \
  --out target/refinery.engineering.functions.json
```

## C4.17.3 Package report

```text id="package-report"
Package: refinery.engineering
Version: 0.4.0
Status: active
Feature: engineering
Dependencies:
  - core.math
Functions:
  scalar:
    - normalize_api_gravity
    - density_to_api
  aggregate:
    - weighted_sulfur
  window:
    - rolling_stability_score
Tenant availability:
  internal_refinery_modeling: enabled
  public_sql: disabled
```

---

# C4.18 Test architecture

## C4.18.1 Package-level tests

```rust id="package-test"
#[tokio::test]
async fn engineering_package_registers_allowed_functions() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();
    let package = EngineeringPackage::default();
    let options = PackageOptions::internal_dev_all_enabled();

    let report = package.register(&ctx, &options)?;

    assert!(report.registered.iter().any(|f| f.canonical_name == "normalize_api_gravity"));
    assert!(report.registered.iter().any(|f| f.canonical_name == "weighted_sulfur"));

    ctx.sql("SELECT normalize_api_gravity(42.0)")
        .await?;

    Ok(())
}
```

## C4.18.2 Feature compile tests

```bash id="feature-compile-tests"
cargo check -p calc-functions --no-default-features
cargo check -p calc-functions --no-default-features --features math
cargo check -p calc-functions --no-default-features --features engineering
cargo check -p calc-functions --no-default-features --features external_io
cargo check -p calc-functions --all-features
```

## C4.18.3 Policy tests

```rust id="package-policy-test"
#[tokio::test]
async fn external_io_package_denied_for_public_sql() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();
    let package = ExternalIoPackage::mock();
    let options = PackageOptions::for_public_sql();

    let report = package.register(&ctx, &options)?;

    assert!(report.registered.is_empty());
    assert!(report.skipped.iter().any(|s| s.reason.contains("external_io")));

    Ok(())
}
```

## C4.18.4 Ordering tests

```rust id="ordering-test"
#[test]
fn package_registration_order_is_deterministic() {
    let mut a = build_registry_with_random_insert_order(1);
    let mut b = build_registry_with_random_insert_order(2);

    let order_a = a.resolve_order().unwrap();
    let order_b = b.resolve_order().unwrap();

    assert_eq!(order_a, order_b);
}
```

## C4.18.5 Manifest/implementation consistency

```rust id="manifest-consistency-test"
#[test]
fn engineering_manifest_matches_registered_functions() {
    let package = EngineeringPackage::default();
    let manifest = package.manifest();

    let manifest_names = manifest.function_names();
    let implementation_names = package.implementation_function_names();

    assert_eq!(manifest_names, implementation_names);
}
```

---

# C4.19 Benchmarking package families

## C4.19.1 Benchmark layout

```text id="bench-layout"
benches/
  scalar_math.rs
  scalar_engineering.rs
  aggregate_weighted.rs
  window_rolling.rs
  external_io_mock.rs
```

## C4.19.2 Benchmark matrix

| Package         | Required benchmark dimensions                               |
| --------------- | ----------------------------------------------------------- |
| `math`          | batch size, scalar path, array path, null density           |
| `strings`       | string length, null density, dictionary/string-view support |
| `engineering`   | realistic fixture distributions, numeric tolerance          |
| `external_io`   | cache hit/miss, timeout, retry, concurrency                 |
| `python_bridge` | copy cost, GIL contention, process startup, batch size      |
| `experimental`  | baseline only; no production SLA                            |

## C4.19.3 Output record

```yaml id="benchmark-record"
benchmark:
  package_id: refinery.engineering
  function: weighted_sulfur
  version: 0.4.0
  datafusion_version: 54.1.0
  arrow_version: 58.4.0
  batch_size: 8192
  rows: 1000000
  null_fraction: 0.05
  rows_per_second: 18500000
  ns_per_row: 54
  memory_bytes_per_group: 32
```

---

# C4.20 Deployment models

## C4.20.1 Minimal embedded engine

```text id="minimal-engine"
compiled packages:
  core.math
  core.strings

runtime:
  single SessionContext
  internal SQL only
  no external_io
  no python_bridge
```

## C4.20.2 Internal modeling engine

```text id="modeling-engine"
compiled packages:
  core.math
  core.strings
  refinery.engineering
  refinery.aggregates
  refinery.windows
  refinery.table_functions

runtime:
  per project/session context
  all deterministic domain packages enabled
  external_io disabled by default
```

## C4.20.3 Public SQL service

```text id="public-service"
compiled packages:
  core.math
  core.strings
  safe deterministic subset

runtime:
  tenant-specific policy
  read-only SQLOptions
  package allowlist
  function allowlist
  no external_io
  no python_bridge
  no experimental
```

## C4.20.4 Research sandbox

```text id="research-sandbox"
compiled packages:
  all features
  experimental enabled
  python_bridge optional
  external_io mock/limited

runtime:
  isolated network/credentials
  low trust boundary
  no production credentials
  bounded resource limits
```

---

# C4.21 Startup registration report

```json id="startup-report"
{
  "event": "function_package_registration_complete",
  "tenant": "internal_refinery_modeling",
  "datafusion_version": "54.1.0",
  "packages": [
    {
      "package_id": "core.math",
      "version": "0.1.0",
      "registered": 12,
      "skipped": 0,
      "warnings": []
    },
    {
      "package_id": "refinery.engineering",
      "version": "0.4.0",
      "registered": 17,
      "skipped": 2,
      "warnings": [
        "function prototype_flash_correlation skipped: experimental disabled"
      ]
    }
  ]
}
```

### Metrics

```text id="startup-metrics"
calc_packages_compiled_total{package}
calc_packages_registered_total{tenant,package,status}
calc_functions_registered_total{tenant,package,family}
calc_functions_skipped_total{tenant,package,reason}
calc_package_registration_duration_ms{tenant,package}
```

---

# C4.22 Best-practice deployment advisory

```text id="deployment-advisory"
Production:
  static linking
  exact DataFusion version pin
  workspace dependency pinning
  deterministic package ordering
  feature-minimized binary where practical
  runtime package enablement by manifest
  tenant policy gate
  startup registration report
  catalog/package diff in CI
  no arbitrary user native plugins

Internal research:
  all-features build acceptable
  experimental package allowed
  dynamic/subprocess plugins allowed only in sandbox
  no production credentials

External I/O:
  separate package
  compile-time gated
  tenant-scoped client
  secret resolver injection
  timeout/retry/circuit breaker
  metrics mandatory

Python bridge:
  separate package
  not default
  benchmarked
  isolated when possible
```

---

# C4.23 Anti-pattern inventory

* One giant `functions.rs`.
* Function constructors called directly from `main.rs`.
* Package registration order depends on `HashMap`.
* Feature flag enables external I/O silently.
* Tenant policy omitted from registration.
* Experimental package included in production default.
* Python bridge compiled into public SQL service by default.
* Dynamic native plugin loading without signing/sandbox/version pin.
* Plugin exposes Rust trait object ABI across dynamic library boundary.
* Package manifest differs from registered implementation.
* Alias collision resolved by “last registration wins.”
* Built-in override by accident.
* UDF implementation imports a different Arrow crate version.
* External-I/O function stores tenant credentials in static global.
* Package dependency cycle hidden by registration order.
* Tests only call `register_all(ctx)` and never test restricted package options.
* No package-level startup report.
* No feature-flag compile matrix.
* No package diff in release CI.
* No benchmark for hot package.

---

# C4.24 Agent checklist

```text id="c4-final-checklist"
[ ] Create workspace crates: calc-core, calc-arrow-kernels, calc-functions, calc-aggregates, calc-window, calc-table-functions, calc-manifest, calc-tests.
[ ] Keep query-cli/query-server as runtime/transport glue only.
[ ] Pin datafusion exactly in production.
[ ] Use datafusion::arrow re-exports.
[ ] Avoid direct datafusion-* subcrates unless extension boundary requires them.
[ ] Define FunctionPackage trait.
[ ] Define PackageOptions with tenant policy, features, external_io, experimental/deprecated controls.
[ ] Define PackageRegistrationReport.
[ ] Build PackageRegistry with deterministic BTreeMap/BTreeSet ordering.
[ ] Topologically sort packages by declared dependencies.
[ ] Validate missing dependencies and cycles.
[ ] Implement register_all(ctx) only for dev/test/internal.
[ ] Implement register_domain_package(ctx, PackageOptions) for production.
[ ] Split features: math, strings, engineering, external_io, python_bridge, experimental.
[ ] Separate external_io and python_bridge packages from pure deterministic packages.
[ ] Use static linking for trusted production.
[ ] Avoid arbitrary user-supplied native plugins.
[ ] Use WASM or subprocess for sandboxed plugin needs.
[ ] Build deployment manifests listing enabled packages, tenant allowlists, cost/resource classes, credentials.
[ ] Validate package manifests against registered implementation.
[ ] Validate collisions against built-ins, aliases, package IDs.
[ ] Log startup registration report.
[ ] Expose package/function discovery via CLI/JSON/YAML/docs.
[ ] Add feature-flag compile tests.
[ ] Add package policy tests.
[ ] Add deterministic ordering tests.
[ ] Add benchmark matrix for hot/high-risk packages.
```

[1]: https://docs.rs/datafusion/latest/datafusion/?utm_source=chatgpt.com "datafusion - Rust"


# DataFusion Advanced — C5) Signature design and overload resolution

## C5.0 Objective

Make user-defined function signatures **predictable, coercion-safe, testable, versionable, and agent-generatable**:

```text id="c5-root"
Function semantic contract
  ├─ public name / aliases
  ├─ function family
  ├─ positional parameter order
  ├─ optional named parameter names
  ├─ accepted argument type sets
  ├─ coercion policy
  ├─ overload strategy
  ├─ return type / return field inference
  ├─ nullability and null-only literal behavior
  ├─ runtime downcast contract
  └─ compatibility/versioning contract
```

The attachment already identifies signatures and volatility as core UDF documentation topics, alongside scalar/async/aggregate/window/table UDFs, return type inference, Arrow vectorized inputs/outputs, registration, SQL/DataFrame invocation, and documentation metadata.  DataFusion’s current `Signature` stores a `TypeSignature`, `Volatility`, and optional `parameter_names`; parameter names enable named-argument notation and must match the argument count defined by the type signature. ([Docs.rs][1])

---

## C5.1 Signature mental model

```text id="c5-model"
SQL / DataFrame function call
  ├─ raw argument Exprs
  ├─ inferred argument DataTypes
  ├─ function lookup by name/alias
  ├─ Signature matching
  ├─ optional implicit casts inserted by type coercion
  ├─ return_type / return_field_from_args inference
  ├─ physical expression construction
  └─ invoke_with_args receives coerced ColumnarValue arguments
```

Core invariant:

```text id="c5-invariant"
Signature describes the types the implementation can actually evaluate after DataFusion coercion.
Implementation downcasts must match post-coercion types, not merely user-written SQL types.
```

DataFusion performs type coercion automatically when argument/operator types do not exactly match required types, inserting casts where possible; its coercion module states that inserted casts are lossless and never discard information. ([Docs.rs][2]) `ScalarUDFImpl::return_type` can assume another part of DataFusion has coerced actual argument types to match the function signature; if `return_field_from_args` is implemented, DataFusion will not call `return_type`. ([Docs.rs][3])

---

## C5.2 Signature API inventory

`Signature` constructors currently include exact, uniform, numeric, string, comparable, any, variadic, variadic-any, one-of, user-defined, coercible, nullary, and specialized array/list helpers. Use docs.rs for the pinned crate when writing deployable code. ([Docs.rs][1])

| Constructor                                      | Meaning                                          | Best use                                       |
| ------------------------------------------------ | ------------------------------------------------ | ---------------------------------------------- |
| `Signature::exact(types, volatility)`            | exact ordered argument types                     | safest public API                              |
| `Signature::uniform(n, valid_types, volatility)` | fixed arity; all args same type from allowed set | binary arithmetic over same type               |
| `Signature::numeric(n, volatility)`              | fixed arity numeric args                         | broad numeric formulas                         |
| `Signature::string(n, volatility)`               | fixed arity string args                          | text transforms                                |
| `Signature::comparable(n, volatility)`           | fixed arity coerced to comparable common type    | `least`, `greatest`, comparison-like functions |
| `Signature::any(n, volatility)`                  | fixed arity any type                             | type-inspecting functions                      |
| `Signature::variadic(common_types, volatility)`  | arbitrary arity, common allowed type set         | `concat_like`, `coalesce_like`                 |
| `Signature::variadic_any(volatility)`            | arbitrary arity any type                         | diagnostics/meta functions only                |
| `Signature::one_of(type_signatures, volatility)` | multiple accepted signatures                     | explicit overload set                          |
| `Signature::user_defined(volatility)`            | implementation controls coercion                 | complex polymorphism                           |
| `Signature::coercible(target_types, volatility)` | explicit per-arg coercion rules                  | controlled implicit coercion                   |
| `Signature::nullary(volatility)`                 | zero args                                        | `now_like()`, `version_like()`                 |
| `Signature::arrays(n, coercion, volatility)`     | fixed number of arrays                           | vector/list functions                          |
| `Signature::array_and_element(volatility)`       | array + element                                  | append/search-style functions                  |
| `Signature::element_and_array(volatility)`       | element + array                                  | prepend/search-style functions                 |

`Coercion` has exact and implicit variants: exact accepts only the desired type class, while implicit accepts the desired type plus configured implicit source classes. ([Docs.rs][4])

---

# C5.3 Exact vs coercible vs comparable vs numeric vs string vs variadic

## C5.3.1 Exact signatures

### Use when

```text id="exact-use"
public API must be stable
runtime implementation downcasts one concrete Arrow type
coercion would be surprising
decimal/timestamp/string-view semantics are sensitive
error messages should be simple
```

### Example

```rust id="exact-sig"
use datafusion::arrow::datatypes::DataType;
use datafusion::logical_expr::{Signature, Volatility};

let sig = Signature::exact(
    vec![DataType::Int64],
    Volatility::Immutable,
);
```

### SQL behavior

```sql id="exact-sql"
SELECT add_one(41);              -- ok if literal coerces to Int64
SELECT add_one(CAST(41 AS BIGINT));
SELECT add_one('41');            -- should fail unless coercion allows string
```

### Agent rules

```text id="exact-agent"
Prefer exact for first production version.
Use exact for domain APIs where implicit casts can hide data quality issues.
Use exact for decimal/timestamp functions unless widening/coercion is intentionally specified.
```

---

## C5.3.2 Uniform signatures

### Use when

```text id="uniform-use"
fixed arity
all arguments must share one type
implementation supports a finite type list
```

### Example

```rust id="uniform-sig"
let sig = Signature::uniform(
    3,
    vec![DataType::Float64, DataType::Int64],
    Volatility::Immutable,
);
```

### Function shape

```text id="uniform-shape"
clip(x, min, max)
  accepted:
    Float64, Float64, Float64
    Int64, Int64, Int64
  rejected/coerced:
    mixed numeric types depend on DataFusion coercion rules and signature matching
```

### Agent rules

```text id="uniform-agent"
Use uniform for same-type multi-arg kernels.
Do not use uniform if arguments have semantically different required types.
If mixed numeric inputs are common, prefer explicit `coercible` or `user_defined` canonicalization.
```

---

## C5.3.3 Numeric signatures

### Use when

```text id="numeric-use"
function is mathematically numeric
same implementation can canonicalize numeric inputs
exact return type policy is documented
```

### Example

```rust id="numeric-sig"
let sig = Signature::numeric(2, Volatility::Immutable);
```

### Hazard

```text id="numeric-hazard"
numeric accepts broad numeric classes.
Implementation must either:
  handle every post-coercion numeric type, or
  use coerce_types / user_defined signature to canonicalize.
```

### Agent rules

```text id="numeric-agent"
Use numeric only if runtime implementation supports broad numeric dispatch.
For initial production APIs, canonicalize to Float64 or Decimal via `coercible` / `user_defined`.
Do not advertise numeric if implementation only downcasts Float64Array.
```

---

## C5.3.4 String signatures

### Use when

```text id="string-use"
function accepts logical string values
implementation supports configured string representations
```

### Hazards

```text id="string-hazards"
Utf8 vs Utf8View
LargeUtf8
Dictionary(Int*, Utf8)
string literals vs declared string columns
SQL parser string type mapping config
```

The attachment documents DataFusion’s string type mapping concerns: SQL string declarations map to `Utf8View` by default unless `datafusion.sql_parser.map_string_types_to_utf8view` is changed, and UDFs built only around `StringArray` must either adapt to `Utf8View` or force compatible declarations. 

### Agent rules

```text id="string-agent"
Never assume string UDF input is always StringArray.
Declare supported string physical/logical types.
Test Utf8, Utf8View, LargeUtf8, dictionary-encoded strings if function claims broad string support.
If only Utf8 is supported, use exact/coercion policy and fail clearly otherwise.
```

---

## C5.3.5 Comparable signatures

### Use when

```text id="comparable-use"
function needs ordered/equality-comparable arguments
all args should coerce to a single comparable type
```

### Examples

```text id="comparable-examples"
least_like(a, b, c)
greatest_like(a, b, c)
range_contains(value, lower, upper)
```

### DataFusion 54 coercion change

Comparison coercion is numeric-preferring in DataFusion 54: when string and numeric arguments meet in a comparison context, the strings are coerced to the numeric type (not the numbers to strings), and strings that fail numeric parsing raise an execution error instead of comparing lexicographically. The built-in `greatest`/`least` follow this rule — they fold their argument types through `comparison_coercion` (`datafusion-functions` `core/greatest_least_utils.rs`), so `greatest('2', 10)` now evaluates numerically to `10` where 53 compared strings. The former `comparison_coercion_numeric` helper was removed in 54; `comparison_coercion` itself is now numeric-preferring, and the new `type_union_coercion` covers the string-preferring UNION/`CASE`/`NVL2` contexts. Comparable UDFs that mimic `greatest`/`least` should mirror this policy and test string/number mixes explicitly.

### Agent rules

```text id="comparable-agent"
Use comparable for comparison semantics, not arbitrary arithmetic.
Specify null/NaN behavior separately.
Test mixed int/float/decimal/string/timestamp cases if accepted.
DF 54: mixed string/number comparisons coerce numerically; invalid numeric strings error.
```

---

## C5.3.6 Variadic signatures

### Use when

```text id="variadic-use"
function arity is genuinely variable
all or many arguments share a compatible class
```

### Example

```rust id="variadic-sig"
let sig = Signature::variadic(
    vec![DataType::Utf8, DataType::Utf8View],
    Volatility::Immutable,
);
```

### Public API warning

```text id="variadic-warning"
Variadic signatures make parameter names ambiguous.
Use for `concat`, `coalesce`, `make_array`-like APIs.
Avoid for domain formulas where each parameter has a distinct semantic meaning.
```

### Agent contract

```text id="variadic-contract"
Variadic signatures cannot use stable per-position semantic names beyond generic docs.
If parameter names are public API, prefer exact fixed arity.
```

---

## C5.3.7 `any` and `variadic_any`

### Use only when

```text id="any-use"
function is explicitly type-inspecting
function returns metadata
function implements robust runtime dispatch for every possible input type
```

### Examples

```text id="any-examples"
arrow_typeof_like(expr)
debug_field_metadata(expr)
to_json_debug(expr)
```

### Agent rules

```text id="any-agent"
Avoid `any` for domain calculations.
Avoid `variadic_any` unless function is diagnostic/meta.
If using any, every runtime branch must return DataFusionError for unsupported type, never panic.
Test primitive, string, decimal, timestamp, list, struct, dictionary, null-only cases.
```

---

# C5.4 Named-argument ergonomics

## C5.4.1 Signature parameter names

`Signature` has `parameter_names: Option<Vec<String>>`; when provided, the parameter-name vector enables named-argument notation and its length must match the argument count defined by the type signature. ([Docs.rs][1])

### Fixed-arity named signature

```rust id="named-signature"
use datafusion::logical_expr::{Signature, TypeSignature, Volatility};

let mut sig = Signature::exact(
    vec![DataType::Float64, DataType::Float64, DataType::Float64],
    Volatility::Immutable,
);

sig.parameter_names = Some(vec![
    "value".to_string(),
    "min_value".to_string(),
    "max_value".to_string(),
]);
```

### Product manifest

```yaml id="named-manifest"
signature:
  mode: exact
  args:
    - name: value
      data_type: Float64
    - name: min_value
      data_type: Float64
    - name: max_value
      data_type: Float64
  named_args: true
```

### SQL usage shape

```sql id="named-sql"
SELECT clip(value => x, min_value => 0.0, max_value => 1.0) AS clipped
FROM t;
```

### Agent rules

```text id="named-agent"
Parameter names are public API.
Use named args for domain formulas with multiple same-typed parameters.
Never reorder parameters after release.
Do not use named parameters for variadic signatures.
Validate parameter_names length against fixed arity in tests.
Use long descriptive names for same-type arguments: min_value/max_value, not a/b.
```

---

# C5.5 Overload strategy

## C5.5.1 Overload decision table

| Need                                                  | Recommended pattern                                        |
| ----------------------------------------------------- | ---------------------------------------------------------- |
| one concrete API                                      | `Signature::exact`                                         |
| same arity, same type across args                     | `Signature::uniform`                                       |
| broad numeric support, same algorithm                 | `Signature::numeric` + robust dispatch or canonicalization |
| explicit finite overload list                         | `Signature::one_of`                                        |
| custom coercion/canonicalization                      | `Signature::user_defined` + `coerce_types`                 |
| per-arg coercion rules                                | `Signature::coercible`                                     |
| radically different semantics by type                 | multiple named UDFs                                        |
| domain clarity more important than SQL terseness      | multiple named UDFs                                        |
| return type depends strongly on input fields/metadata | `ScalarUDFImpl` + `return_field_from_args`                 |
| unknown/runtime-defined behavior                      | avoid unless diagnostic                                    |

---

## C5.5.2 One polymorphic UDF

### Use when

```text id="poly-use"
same semantic function across types
same null policy
same volatility
same broad documentation
return type rule is predictable
```

### Example

```text id="poly-example"
safe_abs(x)
  Int64 -> Int64
  Float64 -> Float64
  Decimal128(p,s) -> Decimal128(p,s)
```

### Implementation requirement

```text id="poly-requirement"
runtime dispatch must cover every post-coercion type
return_type must match dispatch
tests must cover each overload
```

### Anti-pattern

```text id="poly-antipattern"
one function name hides different semantics:
  parse_date(Utf8) -> Date32
  parse_date(Int64) -> Timestamp
  parse_date(Struct) -> domain object
```

---

## C5.5.3 Multiple named UDFs

### Use when

```text id="multi-named-use"
semantics differ by type
return types differ in user-visible way
coercion would be surprising
domain clarity matters
```

### Example

```text id="multi-named-example"
safe_divide_f64(numerator: Float64, denominator: Float64) -> Float64
safe_divide_decimal(numerator: Decimal128, denominator: Decimal128) -> Decimal128
safe_ratio_percent(numerator: Float64, denominator: Float64) -> Float64
```

### Agent rules

```text id="multi-named-agent"
Prefer explicit function names over surprising polymorphism.
Use aliases only for identical semantics.
Do not use overloads to hide different units/basis/precision policy.
```

---

## C5.5.4 `Signature::one_of`

### Use when

```text id="oneof-use"
finite overload set
same public function name
same semantic meaning
implementation can dispatch safely
```

### Example

```rust id="oneof-sig"
use datafusion::logical_expr::{Signature, TypeSignature, Volatility};

let sig = Signature::one_of(
    vec![
        TypeSignature::Exact(vec![DataType::Int64]),
        TypeSignature::Exact(vec![DataType::Float64]),
        TypeSignature::Exact(vec![
            DataType::Decimal128(20, 4),
        ]),
    ],
    Volatility::Immutable,
);
```

### Agent rules

```text id="oneof-agent"
Use one_of when overloads are finite and explicit.
Do not use one_of for open-ended numeric/string support.
Each overload must have a test fixture.
Each overload must have documented return type behavior.
```

---

## C5.5.5 `Signature::user_defined` + `coerce_types`

### Use when

```text id="user-defined-use"
built-in signature constructors cannot express desired coercion
function needs canonical internal types
argument coercion depends on all argument types
nested/list/struct rules need custom logic
decimal/timestamp/string rules need explicit policy
```

`Signature::user_defined` marks user-defined coercion rules, while `ScalarUDFImpl::coerce_types` provides the target types for argument coercion. ([Docs.rs][1])

### Skeleton

```rust id="user-defined-skeleton"
#[derive(Debug)]
pub struct SafeDivide {
    signature: Signature,
}

impl SafeDivide {
    pub fn new() -> Self {
        Self {
            signature: Signature::user_defined(Volatility::Immutable),
        }
    }
}

impl ScalarUDFImpl for SafeDivide {
    fn name(&self) -> &str {
        "safe_divide"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn coerce_types(&self, arg_types: &[DataType]) -> datafusion::error::Result<Vec<DataType>> {
        if arg_types.len() != 2 {
            return plan_err!("safe_divide expects 2 arguments, got {}", arg_types.len());
        }

        match (&arg_types[0], &arg_types[1]) {
            // simple canonicalization: all numeric to Float64
            (a, b) if is_numeric(a) && is_numeric(b) => {
                Ok(vec![DataType::Float64, DataType::Float64])
            }
            _ => plan_err!(
                "safe_divide expects numeric arguments, got {:?} and {:?}",
                arg_types[0],
                arg_types[1],
            ),
        }
    }

    fn return_type(&self, arg_types: &[DataType]) -> datafusion::error::Result<DataType> {
        // called after coerce_types; should see Float64, Float64
        if arg_types == [DataType::Float64, DataType::Float64] {
            Ok(DataType::Float64)
        } else {
            internal_err!("safe_divide return_type saw unexpected coerced types: {arg_types:?}")
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> datafusion::error::Result<ColumnarValue> {
        // implementation can now downcast Float64 arrays/scalars
        todo!()
    }
}

fn is_numeric(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
            | DataType::Decimal128(_, _)
            | DataType::Decimal256(_, _)
    )
}
```

### Agent rules

```text id="user-defined-agent"
Use user_defined only when simpler constructors are insufficient.
coerce_types must be total over every type combination that Signature permits.
return_type must assume post-coercion types.
invoke_with_args must downcast post-coercion types.
Use plan errors for unsupported user input; internal errors for impossible post-coercion states.
```

---

## C5.5.6 `Signature::coercible`

### Use when

```text id="coercible-use"
each parameter has a desired type
each parameter has explicit implicit source-type set
coercion policy is local per argument
```

### Example shape

```rust id="coercible-sig-shape"
let sig = Signature::coercible(
    vec![
        Coercion::Exact {
            desired_type: TypeSignatureClass::Native(DataType::Float64.into()),
        },
        Coercion::Implicit {
            desired_type: TypeSignatureClass::Native(DataType::Utf8.into()),
            implicit_coercion: /* configured implicit classes */,
        },
    ],
    Volatility::Immutable,
);
```

### Agent rules

```text id="coercible-agent"
Use coercible when desired type per argument is stable.
Prefer user_defined when desired type depends on multiple arguments together.
Keep coercion source sets narrow.
Document every implicit cast accepted by product API.
```

---

# C5.6 Applied signature examples

## C5.6.1 `add_one(Int64) -> Int64`

### Contract

```text id="add-one-contract"
add_one(x: Int64) -> Int64
null_policy: strict_null_propagation
volatility: Immutable
coercion: exact Int64 only unless product allows integer literal coercion
```

### Signature

```rust id="add-one-sig"
let sig = Signature::exact(
    vec![DataType::Int64],
    Volatility::Immutable,
);
```

### Manifest

```yaml id="add-one-manifest"
canonical_name: add_one
family: scalar
signature:
  mode: exact
  args:
    - name: x
      data_type: Int64
      nullable: true
return_field:
  data_type: Int64
  nullable: true
semantics:
  volatility: Immutable
  null_policy: strict_null_propagation
```

### Tests

```text id="add-one-tests"
SELECT add_one(CAST(41 AS BIGINT)) = 42
SELECT add_one(NULL::BIGINT) IS NULL
SELECT add_one('41') fails unless string coercion explicitly supported
SELECT arrow_typeof(add_one(CAST(41 AS BIGINT))) = Int64
```

---

## C5.6.2 `clip(Float64, Float64, Float64) -> Float64`

### Contract

```text id="clip-contract"
clip(value: Float64, min_value: Float64, max_value: Float64) -> Float64
named args recommended because all parameters are Float64
invalid range policy: if min_value > max_value then error or swap? choose explicitly
```

### Signature

```rust id="clip-sig"
let mut sig = Signature::exact(
    vec![DataType::Float64, DataType::Float64, DataType::Float64],
    Volatility::Immutable,
);

sig.parameter_names = Some(vec![
    "value".to_string(),
    "min_value".to_string(),
    "max_value".to_string(),
]);
```

### SQL

```sql id="clip-sql"
SELECT
  clip(value => metric, min_value => 0.0, max_value => 1.0) AS metric_clipped
FROM t;
```

### Agent rules

```text id="clip-agent"
Use exact Float64 if function is a numeric convenience kernel.
Use user_defined/coercible if Int/Float inputs should canonicalize to Float64.
Do not use variadic; parameters have distinct semantics.
Named args reduce same-type argument-order bugs.
```

---

## C5.6.3 `safe_divide(Numeric, Numeric) -> Float64`

### Contract

```text id="safe-divide-f64-contract"
safe_divide(numerator: Numeric, denominator: Numeric) -> Float64
coercion: accepted numeric inputs -> Float64
denominator zero -> NULL
any null input -> NULL
decimal precision is not preserved
```

### Signature strategy

```text id="safe-divide-strategy"
Use Signature::user_defined + coerce_types:
  numeric numeric -> Float64 Float64
return_type:
  Float64
```

### Manifest

```yaml id="safe-divide-f64-manifest"
canonical_name: safe_divide
signature:
  mode: user_defined
  args:
    - name: numerator
      type_class: Numeric
    - name: denominator
      type_class: Numeric
coercion:
  canonical_types:
    - Float64
    - Float64
return_field:
  data_type: Float64
  nullable: true
semantics:
  null_policy: strict_null_or_zero_denominator_to_null
  precision_policy: approximate_float
```

### Agent warning

```text id="safe-divide-f64-warning"
This function loses decimal exactness by design.
Do not use for financial/accounting exact division.
Expose as approximate or create safe_divide_decimal separately.
```

---

## C5.6.4 `safe_divide_decimal(Decimal, Decimal) -> Decimal`

### Contract

```text id="safe-divide-dec-contract"
safe_divide_decimal(numerator: Decimal128(p1,s1), denominator: Decimal128(p2,s2)) -> Decimal128(p_out,s_out)
coercion: exact decimal or numeric-to-decimal only if explicitly allowed
zero denominator -> NULL
precision/scale policy documented
```

### Recommended API

```text id="decimal-recommendation"
Prefer separate name over overloading safe_divide:
  safe_divide        -> approximate Float64
  safe_divide_decimal -> exact/controlled Decimal
```

### Agent rules

```text id="decimal-agent"
Do not silently coerce Decimal to Float64 in financial contexts.
Do not claim Decimal return type without explicit precision/scale rule.
Test overflow, scale widening, zero denominator, nulls.
```

---

## C5.6.5 `vector_distance(List<Float64>, List<Float64>) -> Float64`

### Contract

```text id="vector-distance-contract"
vector_distance(left: List<Float64>, right: List<Float64>) -> Float64
same-length vectors required
null vector -> NULL
null element policy explicit: error, skip, or null result
```

### Signature strategy

```rust id="vector-distance-sig"
let sig = Signature::exact(
    vec![
        DataType::List(Arc::new(Field::new_list_field(DataType::Float64, true))),
        DataType::List(Arc::new(Field::new_list_field(DataType::Float64, true))),
    ],
    Volatility::Immutable,
);
```

### Alternatives

```text id="vector-alternatives"
If FixedSizeList length is required:
  use exact FixedSizeList where dimension known
  or user_defined with runtime field/dimension checks

If Float32/Float64 both accepted:
  one_of finite overloads or user_defined canonicalize to Float64 list

If built-in vector functions suffice:
  use built-in list_distance / cosine_distance rather than UDF
```

DataFusion exposes many array/list functions in current docs, including vector-oriented functions such as list distance and related list operations in the Python API index; use built-ins before custom UDFs when semantics match. ([Apache DataFusion][5])

### Agent rules

```text id="vector-agent"
Exact nested signatures are verbose but safest.
Test empty list, null list, null element, mismatched length, large vector.
Do not accept List<Any> unless runtime dispatch is robust.
For embeddings with fixed dimension, prefer FixedSizeList if upstream schema can enforce it.
```

---

## C5.6.6 `struct_get(Struct, Utf8) -> dynamic field`

### Contract

```text id="struct-get-contract"
struct_get(obj: Struct, field_name: Utf8) -> value type of selected field
return type may depend on literal field_name and struct input schema
```

### Placement warning

```text id="struct-get-warning"
If field name is static:
  prefer DataFusion bracket/get_field expression.
If dynamic runtime field name:
  return type may not be statically knowable unless constrained.
```

### Signature strategies

| Strategy                                         | Use                                         |
| ------------------------------------------------ | ------------------------------------------- |
| built-in `get_field` / bracket syntax            | static field access                         |
| UDF returning `Utf8` JSON/text                   | diagnostic only; loses type                 |
| UDF returning union/struct wrapper               | advanced; complex downstream handling       |
| SQL planner extension lowering to `get_field`    | custom syntax                               |
| `return_field_from_args` with literal field name | possible if planning-time literal available |

### Agent rules

```text id="struct-agent"
Avoid generic struct_get UDF if it destroys type information.
Use native nested access for static fields.
Use custom planner lowering to preserve Expr visibility.
If dynamic field selection is unavoidable, document return type restrictions.
```

---

# C5.7 Coercion hazard catalog

## C5.7.1 Decimal scale and precision loss

### Hazard

```text id="decimal-hazard"
Decimal128(20,4) + Float64 canonicalized to Float64:
  exactness lost
  downstream financial semantics may break
```

### Policy

```text id="decimal-policy"
financial/domain-exact:
  keep Decimal
  define output precision/scale
  reject Float64 unless explicitly cast

approximate analytics:
  canonicalize to Float64
  name/docs must say approximate
```

### Tests

```sql id="decimal-tests"
SELECT arrow_typeof(safe_divide_decimal(CAST(1 AS DECIMAL(20,4)), CAST(3 AS DECIMAL(20,4))));
SELECT safe_divide(CAST(1 AS DECIMAL(20,4)), CAST(3 AS DECIMAL(20,4)));
```

The attachment’s SQL type mapping section notes that DataFusion decimal declarations map to Decimal128 up to precision 38 and Decimal256 above 38, with max precision 76; use those constraints in signature design. 

---

## C5.7.2 `Utf8` vs `Utf8View` vs `LargeUtf8`

### Hazard

```text id="utf8-hazard"
SQL declaration maps string columns to Utf8View by default.
String literal may infer Utf8.
Implementation downcasts only StringArray.
Function fails on Utf8ViewArray / LargeStringArray.
```

### Policies

```text id="utf8-policies"
Policy A: exact Utf8 only
  simplest implementation
  less compatible

Policy B: logical string class
  support Utf8, Utf8View, LargeUtf8, dictionary strings
  more robust

Policy C: coerce to Utf8
  predictable implementation
  may copy/cast
```

### Agent rules

```text id="utf8-agent"
If using Signature::string, test every supported string representation.
If implementation downcasts StringArray only, signature must not claim broad string support.
Document SQL string mapping configuration in deployment docs.
```

---

## C5.7.3 Dictionary strings

### Hazard

```text id="dictionary-hazard"
Dictionary(Int32, Utf8) is logically string-like but physically dictionary-encoded.
A string UDF that only downcasts Utf8Array fails or forces decode/cast.
```

### Policies

```text id="dictionary-policy"
support dictionary:
  implement dictionary-aware path or cast/coerce to Utf8

reject dictionary:
  exact signature excludes dictionary
  error message suggests CAST
```

### SQL workaround

```sql id="dictionary-workaround"
SELECT my_string_udf(arrow_cast(dict_col, 'Utf8')) AS out
FROM t;
```

---

## C5.7.4 Timestamp units and timezones

### Hazard

```text id="timestamp-hazard"
Timestamp(Nanosecond, None)
Timestamp(Microsecond, Some("UTC"))
Timestamp(Millisecond, Some("+08:00"))
all require distinct interpretation.
```

### Policies

```text id="timestamp-policy"
Use exact timestamp unit/timezone for time-sensitive functions.
Use user_defined coercion only when conversion semantics are explicitly correct.
Do not drop timezone metadata silently.
Use arrow_cast in examples to force target type.
```

### Test cases

```text id="timestamp-tests"
[ ] Timestamp(ns, None)
[ ] Timestamp(us, UTC)
[ ] Timestamp(ms, +08:00)
[ ] Date32 input rejected or explicitly promoted
[ ] timezone conversion documented
```

---

## C5.7.5 Null-only arguments

### Hazard

```text id="null-only-hazard"
SQL NULL literal has weak/unknown type until context coerces it.
Function with broad/ambiguous overloads may not infer expected type.
```

### Examples

```sql id="null-only-sql"
SELECT add_one(NULL);                       -- ambiguous unless context resolves
SELECT add_one(CAST(NULL AS BIGINT));       -- stable
SELECT clip(NULL, 0.0, 1.0);                -- first arg may need target type
```

### Agent rules

```text id="null-only-agent"
In tests and generated SQL, cast NULL literals.
UDF docs should show typed NULL examples.
For overloaded functions, verify NULL-only invocation produces helpful planning error or expected coercion.
```

---

## C5.7.6 Nested list element coercion

### Hazard

```text id="nested-list-hazard"
List<Int32> vs List<Float64>
List<Float64 nullable elements> vs List<Float64 non-null elements>
FixedSizeList<Float64, N> vs List<Float64>
```

### Policies

```text id="nested-list-policy"
Exact vector API:
  require List<Float64> or FixedSizeList<Float64,N>

Flexible vector API:
  user_defined coerce numeric element lists to Float64 list
  validate dimension at runtime

Diagnostic API:
  accept any list but return metadata/string, not numeric distance
```

### Agent rules

```text id="nested-list-agent"
Nested coercion must specify element type and nullability policy.
Do not treat all lists as equal.
Test list null, element null, empty list, mismatched element types, fixed-size list.
```

---

# C5.8 SignatureSpec manifest

## C5.8.1 Schema

```yaml id="signature-spec-yaml"
signature:
  mode: user_defined
  arity:
    min: 2
    max: 2
  args:
    - name: numerator
      accepted:
        type_class: Numeric
      canonical: Float64
      nullable: true
      semantic_type: numerator
    - name: denominator
      accepted:
        type_class: Numeric
      canonical: Float64
      nullable: true
      semantic_type: denominator
  named_args: true
  variadic: false
  coercion:
    policy: canonicalize_to_float64
    loss_policy: approximate_ok
  overloads: []
```

## C5.8.2 Rust model

```rust id="signature-spec-rust"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SignatureSpec {
    pub mode: SignatureMode,
    pub arity: AritySpec,
    pub args: Vec<ArgumentSpec>,
    pub named_args: bool,
    pub variadic: bool,
    pub coercion: CoercionSpec,
    pub overloads: Vec<OverloadSpec>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SignatureMode {
    Exact,
    Uniform,
    Numeric,
    String,
    Comparable,
    Any,
    Variadic,
    VariadicAny,
    OneOf,
    Coercible,
    UserDefined,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArgumentSpec {
    pub name: String,
    pub accepted: AcceptedTypeSpec,
    pub canonical: Option<ArrowTypeSpec>,
    pub nullable: bool,
    pub semantic_type: Option<String>,
    pub unit: Option<String>,
    pub description: String,
}
```

## C5.8.3 Validation rules

```text id="signature-validation"
[ ] fixed arity: args.len == arity.min == arity.max
[ ] named_args true implies non-variadic fixed arity
[ ] variadic true implies generic parameter docs, not per-position domain semantics
[ ] exact mode implies concrete accepted types
[ ] user_defined mode implies coerce_types implementation exists
[ ] any mode implies runtime dispatch matrix exists
[ ] overloads all have tests
[ ] return type inference covers every overload
[ ] canonical types match implementation downcasts
```

---

# C5.9 Return type interaction

## C5.9.1 Static return type

Use when output type is independent of input type beyond accepted signature.

```rust id="static-return-type"
fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
    Ok(DataType::Float64)
}
```

## C5.9.2 Input-dependent return type

Use when output preserves input type.

```rust id="input-dependent-return"
fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
    match arg_types {
        [DataType::Int64] => Ok(DataType::Int64),
        [DataType::Float64] => Ok(DataType::Float64),
        [DataType::Decimal128(p, s)] => Ok(DataType::Decimal128(*p, *s)),
        _ => internal_err!("unexpected coerced arg_types for safe_abs: {arg_types:?}"),
    }
}
```

## C5.9.3 Field-dependent return field

Use when output metadata/nullability depends on input fields.

```rust id="return-field-from-args"
fn return_field_from_args(&self, args: ReturnFieldArgs) -> Result<FieldRef> {
    let input_field = &args.arg_fields[0];

    let mut metadata = input_field.metadata().clone();
    metadata.insert("derived_by".to_string(), self.name().to_string());

    Ok(Arc::new(
        Field::new("normalized_value", input_field.data_type().clone(), input_field.is_nullable())
            .with_metadata(metadata)
    ))
}
```

### Agent invariant

```text id="return-type-invariant"
Every overload accepted by signature must have exactly one documented return field outcome.
```

---

# C5.10 Error taxonomy for signature/coercion failures

| Phase     | Error cause                     | Error class                        |
| --------- | ------------------------------- | ---------------------------------- |
| planning  | unknown function                | plan/schema function resolution    |
| planning  | wrong arity                     | plan error                         |
| planning  | no matching signature           | plan error                         |
| planning  | unsupported coercion            | plan error                         |
| planning  | ambiguous overload              | plan error                         |
| planning  | return type inference failure   | plan/internal depending cause      |
| execution | post-coercion downcast mismatch | internal or execution error        |
| execution | invalid value                   | execution error or null per policy |
| execution | output type mismatch            | internal/execution bug             |
| execution | output length mismatch          | execution bug                      |

### Error message pattern

```text id="signature-error-msg"
Function `safe_divide` expected:
  safe_divide(numerator: Numeric, denominator: Numeric) -> Float64

Received:
  safe_divide(Utf8, Float64)

Suggestion:
  Cast arguments explicitly or use `arrow_try_cast(raw_value, 'Float64')`.
```

### Agent rules

```text id="signature-error-agent"
Planning-time type/signature failure should not panic.
Downcast mismatch after coercion is usually implementation/internal bug.
Public errors should show expected signature and actual types.
Avoid matching entire error strings in tests; assert stable substrings.
```

---

# C5.11 Testing matrix

## C5.11.1 Signature resolution tests

```text id="signature-resolution-tests"
[ ] correct exact types plan
[ ] coercible source types plan
[ ] unsupported types fail during planning
[ ] wrong arity fails during planning
[ ] named-argument invocation plans
[ ] named-argument typo fails
[ ] aliases resolve to same signature
[ ] NULL literals require/receive expected type
[ ] every overload plans
[ ] ambiguous overload case fails clearly
```

## C5.11.2 Runtime type tests

```text id="runtime-type-tests"
[ ] ColumnarValue::Scalar path
[ ] ColumnarValue::Array path
[ ] mixed scalar/array path if function supports it
[ ] empty array
[ ] all-null array
[ ] mixed-null array
[ ] wrong post-coercion type cannot happen or errors cleanly
[ ] output Arrow DataType equals return type
[ ] output length equals input row count
```

## C5.11.3 Coercion audit SQL

```sql id="coercion-audit-sql"
SELECT
  arrow_typeof(safe_divide(CAST(1 AS BIGINT), CAST(2 AS BIGINT))) AS int_case,
  arrow_typeof(safe_divide(CAST(1.0 AS DOUBLE), CAST(2.0 AS DOUBLE))) AS float_case,
  arrow_typeof(safe_divide(CAST(1 AS DECIMAL(20,4)), CAST(2 AS DECIMAL(20,4)))) AS decimal_case;
```

## C5.11.4 Null-only SQL

```sql id="null-tests-sql"
SELECT safe_divide(CAST(NULL AS DOUBLE), 1.0) IS NULL AS null_num;
SELECT safe_divide(1.0, CAST(NULL AS DOUBLE)) IS NULL AS null_den;
SELECT safe_divide(CAST(NULL AS DOUBLE), CAST(NULL AS DOUBLE)) IS NULL AS both_null;
```

---

# C5.12 Deployment policy

## C5.12.1 Public SQL endpoint

```text id="public-sig-policy"
Prefer:
  exact signatures
  named args for same-type domain parameters
  no `any`
  no `variadic_any`
  no user_defined unless heavily tested
  no implicit string-to-numeric coercion unless product-approved

Require:
  signature docs
  arrow_typeof tests
  wrong-type tests
  helpful errors
```

## C5.12.2 Internal analytics

```text id="internal-sig-policy"
Allowed:
  numeric signatures
  one_of overloads
  user_defined coercion
  broad string support

Require:
  broader dispatch tests
  benchmark per overload
  manifest of accepted canonical types
```

## C5.12.3 Domain/engineering calculations

```text id="engineering-sig-policy"
Prefer:
  explicit names
  exact or user_defined canonicalization
  unit/basis metadata
  named parameters
  separate decimal/float APIs
  no ambiguous overloads

Avoid:
  any
  variadic
  stringly typed units
  silent decimal-to-float conversion
```

---

# C5.13 Anti-pattern inventory

* Signature says `numeric`; implementation downcasts only `Float64Array`.
* Signature says `string`; implementation downcasts only `StringArray`.
* Function accepts decimals but returns `Float64` without docs saying approximate.
* Function uses `any` to avoid writing signature logic.
* Function uses `variadic` for fixed semantic parameters.
* Parameter names omitted for same-type arguments like `clip(x, min, max)`.
* Parameter names changed after release.
* Alias has different signature/semantics from canonical function.
* `coerce_types` returns types not handled by `invoke_with_args`.
* `return_type` ignores overload-specific output types.
* NULL-only SQL examples are untyped.
* `Signature::one_of` overload lacks test coverage.
* `user_defined` coercion accepts strings to numeric silently.
* Timestamp function drops timezone.
* Vector function accepts `List<Any>` and panics on non-float element.
* Function relies on current DataFusion string mapping without pinning/config tests.
* Wrong-type tests assert entire error strings across versions.

---

# C5.14 Agent checklist

```text id="c5-final-checklist"
[ ] Treat signature as public API.
[ ] Treat parameter names as public API.
[ ] Choose exact signature unless broader coercion is required.
[ ] Use uniform only when all args must share same type.
[ ] Use numeric only if implementation handles/canonicalizes numeric types.
[ ] Use string only if implementation handles/canonicalizes DataFusion string variants.
[ ] Use comparable only for comparison/order semantics.
[ ] Use variadic only for genuinely variable arity.
[ ] Avoid any/variadic_any unless function is diagnostic/type-inspecting.
[ ] Use one_of for finite explicit overloads.
[ ] Use user_defined + coerce_types for cross-arg coercion or canonicalization.
[ ] Use coercible for explicit per-argument coercion rules.
[ ] Prefer multiple named UDFs when semantics differ by type.
[ ] Use named args for same-type domain parameters.
[ ] Do not use named args with variadic signatures.
[ ] Document accepted source types and canonical coerced types.
[ ] Ensure return_type/return_field covers every accepted overload.
[ ] Ensure invoke_with_args downcasts post-coercion types only.
[ ] Test exact, coercible, wrong-type, wrong-arity, named-arg, null-only, and every overload.
[ ] Test Utf8/Utf8View/dictionary strings if string support is claimed.
[ ] Test Decimal precision/scale if decimals are accepted.
[ ] Test timestamp unit/timezone if timestamps are accepted.
[ ] Test List/Struct element fields if nested types are accepted.
[ ] Use arrow_typeof SQL tests to lock output types.
[ ] Pin DataFusion version for signature/coercion behavior.
```

[1]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.Signature.html?utm_source=chatgpt.com "Signature in datafusion::logical_expr - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/type_coercion/index.html?utm_source=chatgpt.com "datafusion::logical_expr::type_coercion - Rust"
[3]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html?utm_source=chatgpt.com "ScalarUDFImpl in datafusion_expr - Rust"
[4]: https://docs.rs/datafusion-expr/latest/datafusion_expr/enum.Coercion.html?utm_source=chatgpt.com "Coercion in datafusion_expr - Rust"
[5]: https://datafusion.apache.org/python/autoapi/datafusion/functions/index.html?utm_source=chatgpt.com "datafusion.functions"


# DataFusion Advanced — C6) Return type, nullability, and metadata inference

## C6.0 Objective

Separate **simple type inference** from **production return-field inference**:

```text id="c6-root"
return_type(arg_types)
  └─ DataType-only inference
     ├─ input: &[DataType]
     ├─ output: DataType
     ├─ no input Field metadata
     ├─ no input nullability beyond type-level assumptions
     └─ good for simple type-from-type functions

return_field_from_args(args)
  └─ full Field inference
     ├─ input: ReturnFieldArgs
     │    ├─ arg_fields: &[Arc<Field>]
     │    └─ scalar_arguments: &[Option<&ScalarValue>]
     ├─ output: Arc<Field>
     ├─ controls output name/type/nullability/metadata
     ├─ can inspect literal scalar arguments
     └─ required for production metadata/nullability/nested/extension-type contracts
```

The attachment already identifies `return_type` as sufficient for deterministic type-from-type UDFs and `return_field_from_args` as the advanced path for output nullability, output metadata, value-dependent return types, field-name-aware typing, and nested/list/struct return fields.  Current DataFusion docs state that `return_type` receives already-coerced argument `DataType`s and that if `return_field_from_args` is implemented, DataFusion will not call `return_type`; docs recommend returning a `DataFusionError::Internal` rather than panicking if an impossible path is reached. ([Docs.rs][1])

---

## C6.1 Return contract mental model

```text id="return-contract"
Logical planning:
  Function call Expr
    ├─ resolved UDF implementation
    ├─ signature/coercion
    ├─ argument Fields
    ├─ optional scalar literal arguments
    ├─ return_type OR return_field_from_args
    └─ output Field stored in logical/physical expression

Physical execution:
  ScalarFunctionArgs
    ├─ args: Vec<ColumnarValue>
    ├─ arg_fields: Vec<Arc<Field>>
    ├─ number_rows
    ├─ return_field: Arc<Field>
    └─ config_options
      ↓
  invoke_with_args(...)
      ↓
  ColumnarValue::Array(ArrayRef) OR ColumnarValue::Scalar(ScalarValue)
```

`ScalarFunctionArgs` includes evaluated `args`, `arg_fields`, `number_rows`, `return_field`, and `config_options`; its `return_field` is the field returned by `return_type` or `return_field_from_args` when the physical expression was created. ([Docs.rs][2])

Agent invariant:

```text id="agent-invariant"
Planner-time return Field and runtime output Array/Scalar must agree:
  return_field.data_type == output DataType
  return_field.nullable == actual nullability contract
  return_field.metadata == documented semantic metadata
  output array length == number_rows for array outputs
```

---

## C6.2 `DataType` vs `Field`

## C6.2.1 `DataType`

```text id="datatype-contract"
DataType answers:
  What physical/logical Arrow type is this value?

Examples:
  Int64
  Float64
  Utf8View
  Decimal128(20, 4)
  Timestamp(Nanosecond, Some("UTC"))
  List(Field("item", Float64, true))
  Struct([Field("x", Float64, false), Field("y", Float64, false)])
```

Use `DataType` when:

```text id="datatype-use"
output type depends only on argument types
output nullability is default/obvious
metadata is irrelevant
no nested child metadata needs propagation
no field-name-sensitive behavior
```

## C6.2.2 `Field`

```text id="field-contract"
Field answers:
  What is the output column contract?

Field =
  name
  data_type
  nullable
  metadata
```

Arrow/DataFusion `Field` describes a schema column and contains name, data type, nullability, and metadata; Arrow extension types are encoded in field metadata, and constructors exist for primitive, list, struct, map, dictionary, fixed-size-list, union, and other nested field shapes. ([Docs.rs][3])

Use `Field` when:

```text id="field-use"
output nullability must be controlled
metadata must be preserved/attached/removed
return type depends on literal arguments
return type depends on input field metadata
return shape is nested/list/struct/map/dictionary
return name is part of API
extension type semantics matter
```

---

# C6.3 `return_type(arg_types)` deep dive

## C6.3.1 Use cases

```text id="return-type-use"
Use return_type when:
  output type is deterministic from argument DataTypes
  output nullability can follow DataFusion default behavior
  metadata is not part of function semantics
  no literal scalar argument changes output type
  no input field name/metadata changes output type
```

## C6.3.2 Basic type-preserving return

```rust id="type-preserving-return"
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{internal_err, plan_err, Result};
use datafusion::logical_expr::ScalarUDFImpl;

fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
    match arg_types {
        [DataType::Int64] => Ok(DataType::Int64),
        [DataType::Float64] => Ok(DataType::Float64),
        [DataType::Decimal128(p, s)] => Ok(DataType::Decimal128(*p, *s)),
        _ => plan_err!("{} unsupported argument types: {arg_types:?}", self.name()),
    }
}
```

## C6.3.3 Static return type

```rust id="static-return"
fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
    if arg_types.len() != 2 {
        return plan_err!("safe_divide expects 2 arguments, got {}", arg_types.len());
    }

    Ok(DataType::Float64)
}
```

## C6.3.4 Type-combining return

```rust id="type-combining-return"
fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
    match arg_types {
        [DataType::Float64, DataType::Float64] => Ok(DataType::Float64),
        [DataType::Float32, DataType::Float32] => Ok(DataType::Float32),
        [DataType::Int64, DataType::Int64] => Ok(DataType::Int64),
        _ => plan_err!("clip expects same-type numeric args, got {arg_types:?}"),
    }
}
```

## C6.3.5 `return_type` anti-patterns

```text id="return-type-antipatterns"
Bad:
  uses arg_types but output nullability should differ from default
  silently ignores metadata needed for extension types
  returns Float64 for Decimal inputs without precision policy
  returns Struct/List/Map without child field nullability discipline
  panics on unsupported type
  assumes uncoerced user-written SQL types
  duplicates logic that should live in return_field_from_args
```

---

# C6.4 `return_field_from_args(args)` deep dive

## C6.4.1 API facts

`ReturnFieldArgs` currently contains `arg_fields: &[Arc<Field>]` and `scalar_arguments: &[Option<&ScalarValue>]`; it provides metadata about how the function was called, including argument fields, scalar constants, and whether arguments can ever be null. For example, a call like `my_function(column_a, 5)` has scalar arguments `[None, Some(ScalarValue::Int32(Some(5)))]`. ([Docs.rs][4])

## C6.4.2 Use cases

```text id="return-field-use"
Use return_field_from_args when:
  output nullability must be explicitly controlled
  output metadata must be preserved/attached/removed
  output field name matters
  output nested child fields need names/nullability/metadata
  output type depends on literal scalar arguments
  output type depends on input field metadata
  output type depends on extension-type metadata
  function supports user-defined logical types over Arrow physical types
```

## C6.4.3 Skeleton

```rust id="return-field-skeleton"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::common::{internal_err, plan_err, Result};
use datafusion::logical_expr::{ReturnFieldArgs, ScalarUDFImpl};

impl ScalarUDFImpl for MyFunc {
    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        // If return_field_from_args is implemented, DataFusion will not call return_type.
        // Keep a defensive implementation for impossible paths.
        internal_err!(
            "{} uses return_field_from_args; return_type should not be called with {arg_types:?}",
            self.name()
        )
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
        if args.arg_fields.len() != 1 {
            return plan_err!("{} expects 1 argument", self.name());
        }

        let input = args.arg_fields[0].as_ref();

        let mut metadata = input.metadata().clone();
        metadata.insert("derived_by".to_string(), self.name().to_string());

        Ok(Arc::new(
            Field::new(
                "my_func_output",
                input.data_type().clone(),
                input.is_nullable(),
            )
            .with_metadata(metadata)
        ))
    }

    // signature + invoke_with_args...
}
```

## C6.4.4 Planner/runtime link

```rust id="runtime-return-field-use"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    let output_type = args.return_field.data_type();
    let output_nullable = args.return_field.is_nullable();
    let output_metadata = args.return_field.metadata();

    // Use planner-computed return contract to choose runtime builder,
    // validate output type, or propagate metadata-sensitive behavior.
    todo!()
}
```

Agent invariant:

```text id="return-field-invariant"
If return_field_from_args computes the contract, invoke_with_args should treat args.return_field as authoritative.
```

---

# C6.5 Return-field design patterns

## C6.5.1 Preserve input type + nullability + metadata

### Use case

```text id="preserve-use"
identity-like transform
semantic-preserving normalization
unit-preserving clamp
metadata-preserving cast wrapper
```

### Pattern

```rust id="preserve-pattern"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let input = args.arg_fields
        .first()
        .ok_or_else(|| DataFusionError::Plan("expected one argument".to_string()))?;

    Ok(Arc::new(
        Field::new(
            format!("{}_normalized", input.name()),
            input.data_type().clone(),
            input.is_nullable(),
        )
        .with_metadata(input.metadata().clone())
    ))
}
```

### Policy

```text id="preserve-policy"
Preserve metadata only if function does not change semantics.
If value transformation changes unit/meaning, attach new metadata instead.
```

---

## C6.5.2 Attach unit metadata

### Use case

```text id="unit-use"
domain formulas
unit conversions
engineering metrics
economic values
quality properties
```

### Pattern

```rust id="unit-pattern"
fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("semantic_type".to_string(), "yield".to_string());
    metadata.insert("unit".to_string(), "fraction".to_string());
    metadata.insert("basis".to_string(), "volume".to_string());
    metadata.insert("derived_by".to_string(), self.name().to_string());

    Ok(Arc::new(
        Field::new("yield_fraction", DataType::Float64, true)
            .with_metadata(metadata)
    ))
}
```

### Agent rule

```text id="unit-agent"
Unit-changing functions must not blindly preserve input unit metadata.
Attach output unit/basis explicitly.
```

---

## C6.5.3 Strip unsafe metadata

### Use case

```text id="strip-use"
hashing
redaction
tokenization
rounding that breaks exactness
stringification
coarse bucketing
privacy-preserving transforms
```

### Pattern

```rust id="strip-pattern"
fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("semantic_type".to_string(), "redacted_string".to_string());
    metadata.insert("privacy_transform".to_string(), self.name().to_string());

    Ok(Arc::new(
        Field::new("redacted", DataType::Utf8, true)
            .with_metadata(metadata)
    ))
}
```

### Agent rule

```text id="strip-agent"
Do not preserve input semantic metadata after irreversible transforms unless metadata is still true.
```

---

## C6.5.4 Literal-dependent return field

### Use case

```text id="literal-dependent-use"
make_fixed_size_vector(x, dimension_literal)
parse_with_format(value, format_literal)
cast_to_unit(value, unit_literal)
bucketize(value, bucket_count_literal)
```

### Pattern

```rust id="literal-dependent-pattern"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    if args.arg_fields.len() != 2 {
        return plan_err!("make_zero_vector expects 2 arguments");
    }

    let Some(Some(dim_scalar)) = args.scalar_arguments.get(1) else {
        return plan_err!("make_zero_vector dimension argument must be a scalar literal");
    };

    let dim = match dim_scalar {
        ScalarValue::Int32(Some(v)) if *v > 0 => *v,
        ScalarValue::Int64(Some(v)) if *v > 0 && *v <= i32::MAX as i64 => *v as i32,
        other => return plan_err!("dimension must be positive Int32/Int64 literal, got {other:?}"),
    };

    let item = Arc::new(Field::new("item", DataType::Float64, false));

    Ok(Arc::new(Field::new_fixed_size_list(
        "vector",
        item,
        dim,
        false,
    )))
}
```

### Agent rules

```text id="literal-agent"
Use scalar_arguments only for planner-known literal-dependent shape.
Reject non-scalar shape parameters during planning.
Do not inspect runtime array values for return shape.
```

---

## C6.5.5 Field-name-sensitive return field

### Use case

```text id="field-name-use"
auto-naming derived outputs
schema lineage
struct field propagation
metadata based on source column identity
```

### Pattern

```rust id="field-name-pattern"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let input = &args.arg_fields[0];

    let output_name = format!("{}_quality_flag", input.name());

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("source_field".to_string(), input.name().to_string());
    metadata.insert("semantic_type".to_string(), "quality_flag".to_string());

    Ok(Arc::new(
        Field::new(output_name, DataType::Boolean, true)
            .with_metadata(metadata)
    ))
}
```

### Agent rule

```text id="field-name-agent"
Field-name-sensitive return names are convenient but can destabilize public schemas.
For public APIs, prefer explicit SQL/DataFrame aliases.
```

---

# C6.6 Nullability modes

## C6.6.1 Nullability decision matrix

| Mode                           |     Output nullable? | Runtime behavior              | Use                                |
| ------------------------------ | -------------------: | ----------------------------- | ---------------------------------- |
| strict null propagation        | `any input nullable` | any null row -> null          | ordinary scalar transforms         |
| null-skipping                  |         usually true | ignore nulls                  | list/aggregate-like scalar kernels |
| null-defaulting                |              depends | null -> default               | compatibility/defaulting functions |
| error-on-null                  |        false or true | null input -> execution error | validation functions               |
| all-null -> null               |                 true | empty/all-null input -> null  | aggregate/list reductions          |
| non-null by construction       |                false | output always non-null        | predicates, constants, counts      |
| nullable due to invalid values |                 true | invalid non-null -> null      | `try_` functions                   |

---

## C6.6.2 Strict null propagation

```text id="strict-null"
if any required input is null:
  output null

output nullable:
  any required input field nullable OR function can emit null for invalid non-null values
```

```rust id="strict-null-field"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let nullable = args.arg_fields.iter().any(|f| f.is_nullable());

    Ok(Arc::new(Field::new("out", DataType::Float64, nullable)))
}
```

---

## C6.6.3 Null-skipping

```text id="null-skipping"
function examines a collection/list/group-like input
null elements skipped
empty or all-null result may be null
```

```rust id="null-skipping-field"
fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    Ok(Arc::new(Field::new(
        "mean_non_null",
        DataType::Float64,
        true, // all-null input -> null
    )))
}
```

---

## C6.6.4 Null-defaulting

```text id="null-defaulting"
null input becomes a configured default
output can be non-null if default is non-null and all branches are non-null
```

```rust id="null-default-field"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    // Example: fill_null(x, default_literal)
    // If default is guaranteed non-null and output type non-null by construction:
    let default_is_non_null = matches!(
        args.scalar_arguments.get(1),
        Some(Some(s)) if !s.is_null()
    );

    let nullable = !default_is_non_null && args.arg_fields[0].is_nullable();

    Ok(Arc::new(Field::new(
        "filled",
        args.arg_fields[0].data_type().clone(),
        nullable,
    )))
}
```

---

## C6.6.5 Error-on-null

```text id="error-on-null"
null input is invalid
planning may allow nullable input field
runtime errors if null appears
```

Return-field policy:

```rust id="error-null-field"
fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    // Output is non-null if function succeeds for every row.
    Ok(Arc::new(Field::new("validated_value", DataType::Float64, false)))
}
```

Runtime rule:

```text id="error-null-runtime"
If any input row is null:
  return DataFusionError::Execution with actionable message
```

Agent warning:

```text id="error-null-agent"
`nullable=false` means successful output contains no nulls.
It does not mean nullable input fields cannot appear unless planning rejects them.
```

---

## C6.6.6 All-null -> null

```text id="all-null"
aggregate/list reduction:
  no valid values -> null
```

Return-field policy:

```rust id="all-null-field"
Field::new("reduced_value", DataType::Float64, true)
```

---

# C6.7 Metadata modes

## C6.7.1 Preserve input semantic type

### Use when

```text id="preserve-semantic-use"
function changes representation but not meaning
function clamps/validates/normalizes within same unit
function returns same logical extension type
```

```rust id="preserve-semantic-metadata"
let mut metadata = input.metadata().clone();
metadata.insert("normalized_by".to_string(), self.name().to_string());
```

## C6.7.2 Attach unit metadata

```rust id="attach-unit-metadata"
let mut metadata = HashMap::new();
metadata.insert("unit".to_string(), "bbl_per_day".to_string());
metadata.insert("semantic_type".to_string(), "flow_rate".to_string());
metadata.insert("basis".to_string(), "calendar_day".to_string());
```

## C6.7.3 Attach source lineage

```rust id="source-lineage-metadata"
let mut metadata = HashMap::new();
metadata.insert("derived_by".to_string(), self.name().to_string());
metadata.insert("source_fields".to_string(), "feed_bbl,product_bbl".to_string());
metadata.insert("calculation_version".to_string(), "yield_v1".to_string());
```

## C6.7.4 Remove unsafe metadata

```rust id="remove-unsafe-metadata"
let mut metadata = HashMap::new();
metadata.insert("semantic_type".to_string(), "hashed_identifier".to_string());
metadata.insert("privacy_transform".to_string(), "sha256".to_string());
```

## C6.7.5 Extension-type metadata

UDF APIs receive full input `Field` information and return full output `Field` information (`return_field_from_args`); the same mechanism supports Arrow extension types and arbitrary field metadata. ([Apache DataFusion][5]) DataFusion 54 turns this into a first-class extension-type story: a session-level extension-type registry (`ExtensionTypeRegistry` trait + `ExtensionTypeRegistration` in `datafusion_expr::registry`) resolves the Arrow `ARROW:extension:name` / `ARROW:extension:metadata` field metadata into typed `DFExtensionType` values (`datafusion_common::types`), including the Arrow canonical extension types (uuid, json, bool8, fixed-shape tensor, opaque). On the function side, `datafusion-functions` core adds `with_metadata` (attach field metadata to an expression's output) and `cast_to_type` / `try_cast_to_type` (field-aware casts that carry the target extension type/metadata, not just the storage `DataType`). See C6.16 for the full DF-54 pattern and `datafusion_schemas_rust.md` S7 for the registry deep dive.

### Agent rule

```text id="extension-agent"
If logical type is encoded in Arrow Field metadata, use return_field_from_args.
DataType-only return_type is insufficient for extension-type-safe UDFs.
```

---

# C6.8 Return shape patterns

## C6.8.1 Scalar primitive

### Contract

```text id="scalar-primitive-contract"
output Field:
  name: stable output name
  data_type: primitive DataType
  nullable: policy-driven
  metadata: optional semantic metadata
```

### Example

```rust id="scalar-primitive"
Field::new("gross_margin", DataType::Float64, true)
```

### Use

```text id="scalar-use"
numeric formulas
boolean flags
parsed scalar values
single extracted values
```

---

## C6.8.2 List

### Contract

```text id="list-contract"
output Field:
  data_type: List(Field("item", item_type, item_nullable))
  nullable: list slot nullable
child Field:
  controls element type/nullability/metadata
```

### Example

```rust id="list-return"
let item = Arc::new(
    Field::new("item", DataType::Float64, false)
        .with_metadata(HashMap::from([
            ("unit".to_string(), "dimensionless".to_string()),
        ]))
);

let field = Field::new_list(
    "normalized_vector",
    item,
    true, // list itself may be null
);
```

### Agent rules

```text id="list-agent"
List nullability and element nullability are separate.
Empty list != null list.
Child metadata matters for vector/series semantics.
```

---

## C6.8.3 Fixed-size list

### Use

```text id="fixed-list-use"
embeddings
fixed-dimension vectors
coordinate tuples
state vectors with known length
```

### Example

```rust id="fixed-list-return"
let item = Arc::new(Field::new("item", DataType::Float64, false));

let field = Field::new_fixed_size_list(
    "embedding_384",
    item,
    384,
    false,
);
```

### Agent rules

```text id="fixed-list-agent"
Use literal dimension in return_field_from_args if dimension is user-provided.
Reject non-scalar dimension arguments.
Test dimension mismatch at runtime if inputs are dynamic.
```

---

## C6.8.4 Struct

### Use

```text id="struct-use"
multi-output diagnostics
parse results
quality-check record
domain object
named vector-like output
```

### Example

```rust id="struct-return"
let fields = vec![
    Arc::new(Field::new("value", DataType::Float64, true)),
    Arc::new(Field::new("status", DataType::Utf8, false)),
    Arc::new(Field::new("reason", DataType::Utf8, true)),
];

let field = Field::new_struct("quality_check", fields, false);
```

### Agent rules

```text id="struct-agent"
Use Struct instead of JSON string for typed multi-output.
Name every child field deterministically.
Child nullability must reflect runtime builders.
Struct nullability and child nullability are separate.
```

---

## C6.8.5 Map

### Use

```text id="map-use"
dynamic key-value output
sparse metadata generated per row
attributes where key set is not stable
```

### Example

```rust id="map-return"
let key = Arc::new(Field::new("key", DataType::Utf8, false));
let value = Arc::new(Field::new("value", DataType::Utf8, true));

let field = Field::new_map(
    "attributes",
    "entries",
    key,
    value,
    false, // sorted
    true,  // map nullable
);
```

### Agent rules

```text id="map-agent"
Prefer Struct when keys are known.
Use Map when keys are dynamic.
Map keys should be non-null.
Document duplicate-key behavior.
```

---

## C6.8.6 Dictionary

### Use

```text id="dictionary-use"
low-cardinality strings
encoded categories
memory-efficient categorical output
```

### Example

```rust id="dictionary-return"
let field = Field::new_dictionary(
    "category",
    DataType::Int32,
    DataType::Utf8,
    true,
);
```

### Agent rules

```text id="dictionary-agent"
Dictionary output must match actual DictionaryArray key/value types.
Document whether consumers may cast to Utf8.
Test SQL/Parquet/Arrow export compatibility.
```

---

## C6.8.7 Timestamp with timezone

### Use

```text id="timestamp-use"
timezone-aware timestamps
normalized event timestamps
external contract requires unit/timezone
```

### Example

```rust id="timestamp-return"
let field = Field::new(
    "event_ts_utc",
    DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
    true,
);
```

### Agent rules

```text id="timestamp-agent"
Do not drop timezone metadata silently.
Specify time unit explicitly.
Use arrow_typeof tests.
```

---

## C6.8.8 Decimal with precision/scale

### Use

```text id="decimal-use"
money
rates
quality values requiring exact precision
bounded scientific/economic values
```

### Example

```rust id="decimal-return"
let field = Field::new(
    "price_usd",
    DataType::Decimal128(20, 4),
    true,
);
```

### Agent rules

```text id="decimal-agent"
Define output precision/scale formula.
Reject overflow during planning if possible.
Avoid Decimal -> Float64 unless function name/docs say approximate.
Test Decimal128 and Decimal256 boundaries.
```

The attachment’s SQL type mapping section records `DECIMAL(p,s)` mapping to `Decimal128(p,s)` up to precision 38, `Decimal256(p,s)` above 38, and a maximum precision of 76. 

---

# C6.9 Production implementation patterns

## C6.9.1 Static primitive return with metadata

```rust id="static-primitive-metadata"
fn return_field_from_args(&self, _args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let metadata = HashMap::from([
        ("semantic_type".to_string(), "profit_margin".to_string()),
        ("unit".to_string(), "fraction".to_string()),
        ("derived_by".to_string(), self.name().to_string()),
    ]);

    Ok(Arc::new(
        Field::new("profit_margin", DataType::Float64, true)
            .with_metadata(metadata)
    ))
}
```

## C6.9.2 Nullability from input fields

```rust id="nullability-from-input"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    if args.arg_fields.len() != 2 {
        return plan_err!("safe_divide expects 2 arguments");
    }

    let nullable = args.arg_fields.iter().any(|f| f.is_nullable())
        || true; // denominator zero can produce null

    Ok(Arc::new(Field::new(
        "safe_divide",
        DataType::Float64,
        nullable,
    )))
}
```

## C6.9.3 Literal-controlled decimal return

```rust id="literal-decimal-return"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    // round_decimal(value, scale_literal)
    let Some(Some(scale_scalar)) = args.scalar_arguments.get(1) else {
        return plan_err!("round_decimal scale must be scalar literal");
    };

    let scale = match scale_scalar {
        ScalarValue::Int32(Some(v)) if *v >= 0 && *v <= 38 => *v as i8,
        ScalarValue::Int64(Some(v)) if *v >= 0 && *v <= 38 => *v as i8,
        other => return plan_err!("scale must be integer literal in [0, 38], got {other:?}"),
    };

    let input_type = args.arg_fields[0].data_type();

    let precision = match input_type {
        DataType::Decimal128(p, _) => *p,
        _ => return plan_err!("round_decimal expects Decimal128 input, got {input_type:?}"),
    };

    Ok(Arc::new(Field::new(
        "rounded_decimal",
        DataType::Decimal128(precision, scale),
        args.arg_fields[0].is_nullable(),
    )))
}
```

## C6.9.4 Metadata-preserving extension type

```rust id="extension-preserve"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let input = &args.arg_fields[0];

    let Some(ext_name) = input.metadata().get("ARROW:extension:name") else {
        return plan_err!("{} expects an Arrow extension-typed field", self.name());
    };

    if ext_name != "my_company.uuid" {
        return plan_err!("{} expected uuid extension type, got {ext_name}", self.name());
    }

    let mut metadata = input.metadata().clone();
    metadata.insert("validated_by".to_string(), self.name().to_string());

    Ok(Arc::new(
        Field::new("uuid_validated", input.data_type().clone(), input.is_nullable())
            .with_metadata(metadata)
    ))
}
```

---

# C6.10 Runtime validation against `return_field`

## C6.10.1 Output type guard

```rust id="output-type-guard"
fn validate_output_type(
    fn_name: &str,
    return_field: &Field,
    value: &ColumnarValue,
) -> Result<()> {
    match value {
        ColumnarValue::Scalar(s) => {
            if s.data_type() != *return_field.data_type() {
                return internal_err!(
                    "{fn_name} returned scalar type {:?}, expected {:?}",
                    s.data_type(),
                    return_field.data_type()
                );
            }
        }
        ColumnarValue::Array(a) => {
            if a.data_type() != return_field.data_type() {
                return internal_err!(
                    "{fn_name} returned array type {:?}, expected {:?}",
                    a.data_type(),
                    return_field.data_type()
                );
            }
        }
    }

    Ok(())
}
```

## C6.10.2 Output nullability guard

```rust id="output-nullability-guard"
fn validate_output_nullability(
    fn_name: &str,
    return_field: &Field,
    array: &dyn Array,
) -> Result<()> {
    if !return_field.is_nullable() && array.null_count() > 0 {
        return internal_err!(
            "{fn_name} returned {} nulls for non-nullable field `{}`",
            array.null_count(),
            return_field.name()
        );
    }

    Ok(())
}
```

## C6.10.3 Output length guard

```rust id="output-length-guard"
fn validate_output_len(
    fn_name: &str,
    number_rows: usize,
    value: &ColumnarValue,
) -> Result<()> {
    if let ColumnarValue::Array(array) = value {
        if array.len() != number_rows {
            return internal_err!(
                "{fn_name} returned {} rows, expected {number_rows}",
                array.len()
            );
        }
    }

    Ok(())
}
```

---

# C6.11 Aggregate and window return notes

## C6.11.1 UDAF return fields

UDAFs typically expose return type/nullability through aggregate-UDF-specific APIs and accumulator behavior, but the same design discipline applies:

```text id="udaf-return"
UDAF return contract:
  group input fields
  state fields
  merge state fields
  evaluate output ScalarValue
  output Field type/nullability/metadata
```

Required invariants:

```text id="udaf-return-invariants"
evaluate() ScalarValue type == declared return DataType
empty group behavior matches nullable flag
all-null group behavior matches nullable flag
state metadata is internal, not output metadata unless intentionally propagated
```

## C6.11.2 UDWF return fields

```text id="udwf-return"
UDWF return contract:
  partition/order/frame semantics
  output one value per input row
  output DataType fixed by signature/return logic
  output nullable if any frame can produce null
```

Required invariants:

```text id="udwf-return-invariants"
output length == partition length
empty frame behavior matches nullable flag
all-null frame behavior matches nullable flag
ORDER BY/frame assumptions documented
```

---

# C6.12 Testing matrix

## C6.12.1 Planning-time return tests

```text id="planning-return-tests"
[ ] return_type simple type cases
[ ] return_field_from_args simple field cases
[ ] nullable input -> nullable output
[ ] non-null input -> non-null output when strict and valid
[ ] metadata preserved where intended
[ ] metadata removed where intended
[ ] unit metadata attached where intended
[ ] scalar literal controls output shape
[ ] non-scalar shape argument fails at planning
[ ] nested child fields match expected schema
```

## C6.12.2 SQL `arrow_typeof` tests

```sql id="arrow-typeof-tests"
SELECT
  arrow_typeof(normalize_api_gravity(CAST(42.0 AS DOUBLE))) AS api_type,
  arrow_typeof(make_zero_vector(3)) AS vector_type,
  arrow_typeof(round_decimal(CAST(1.2345 AS DECIMAL(20, 4)), 2)) AS decimal_type;
```

## C6.12.3 Metadata tests

```rust id="metadata-test"
#[tokio::test]
async fn udf_preserves_unit_metadata() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();
    register_domain_functions(&ctx)?;

    let df = ctx.sql(
        "SELECT normalize_with_unit(value) AS normalized FROM t"
    ).await?;

    let schema = df.schema();
    let field = schema.field_with_unqualified_name("normalized")?;

    assert_eq!(
        field.metadata().get("unit").map(String::as_str),
        Some("bbl_per_day")
    );

    Ok(())
}
```

## C6.12.4 Runtime contract tests

```text id="runtime-contract-tests"
[ ] output array type == return_field.data_type
[ ] output scalar type == return_field.data_type
[ ] output null count == 0 when return_field.nullable=false
[ ] output length == ScalarFunctionArgs.number_rows
[ ] nested child array types match nested Field
[ ] dictionary key/value types match dictionary Field
[ ] timestamp unit/timezone match Field
[ ] decimal precision/scale match Field
```

---

# C6.13 Deployment policy

## C6.13.1 Public SQL

```text id="public-policy"
Require return_field_from_args when:
  metadata affects semantics
  output nullability differs from default
  output nested type is public API
  output decimal precision/scale matters
  output timestamp timezone/unit matters

Reject:
  UDFs that return DataType only but claim semantic metadata
  UDFs that emit nullable outputs with nullable=false
  UDFs that return JSON strings for typed structs
```

## C6.13.2 Internal modeling

```text id="internal-modeling-policy"
Allow:
  metadata-rich domain fields
  extension types
  nested diagnostic structs
  list/vector outputs
  decimal precision/scale policies

Require:
  schema snapshots
  arrow_typeof snapshots
  metadata snapshots
  downstream Parquet/Arrow round-trip tests
```

## C6.13.3 Data lake / Parquet output

```text id="lake-policy"
Before writing UDF outputs to Parquet:
  validate output schema
  validate metadata preservation expectations
  validate decimal/timestamp compatibility
  validate nested fields round-trip
  decide whether field metadata is required by downstream readers
```

---

# C6.14 Anti-pattern inventory

* Using `return_type` while claiming output metadata matters.
* Using `return_type` while output nullability must be non-default.
* Preserving input unit metadata after unit conversion.
* Preserving PII semantic metadata after hashing/redaction.
* Returning `Float64` for decimal inputs without approximate-name/docs.
* Returning `Timestamp(Nanosecond, None)` after timezone-aware parsing.
* Returning `Utf8` JSON string instead of typed `Struct`.
* Creating `List` return type without specifying item nullability.
* Creating `Struct` return type with unstable child field names.
* Using literal argument for output shape but not checking scalar literal at planning.
* Inspecting runtime array values to decide output schema.
* `nullable=false` while runtime can emit null for invalid input.
* Runtime output array type differs from planner return field.
* Runtime output metadata assumed but not present in `Field`.
* No tests for `return_field_from_args`.
* No `arrow_typeof` snapshot for complex return shapes.
* No schema round-trip test for nested/decimal/timestamp outputs.

---

# C6.15 Agent checklist

```text id="c6-final-checklist"
[ ] Decide if DataType-only return_type is sufficient.
[ ] Use return_field_from_args when nullability, metadata, nested fields, literals, field names, or extension types matter.
[ ] If return_field_from_args exists, make return_type return DataFusionError::Internal rather than panic.
[ ] Use arg_fields for input type/nullability/metadata.
[ ] Use scalar_arguments only for planner-known scalar/literal shape decisions.
[ ] Never inspect runtime array values for return schema.
[ ] Define output name, DataType, nullable flag, and metadata explicitly.
[ ] Separate list nullability from item nullability.
[ ] Separate struct nullability from child nullability.
[ ] Define map key/value fields explicitly.
[ ] Define dictionary key/value types explicitly.
[ ] Define timestamp unit/timezone explicitly.
[ ] Define decimal precision/scale explicitly.
[ ] Preserve metadata only when semantics remain true.
[ ] Attach unit/semantic/lineage metadata for domain outputs.
[ ] Remove unsafe metadata after irreversible transforms.
[ ] Validate runtime output type against args.return_field.
[ ] Validate runtime nulls against args.return_field.is_nullable().
[ ] Validate runtime output length against args.number_rows.
[ ] Add SQL arrow_typeof tests.
[ ] Add schema/metadata snapshot tests.
[ ] Add Parquet/Arrow round-trip tests for public nested/decimal/timestamp outputs.
[ ] Pin DataFusion version because return-field APIs are extension hotspots.
```

[1]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html "ScalarUDFImpl in datafusion_expr - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.ScalarFunctionArgs.html "ScalarFunctionArgs in datafusion::logical_expr - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/common/arrow/datatypes/struct.Field.html "Field in datafusion::common::arrow::datatypes - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.ReturnFieldArgs.html "ReturnFieldArgs in datafusion::logical_expr - Rust"
[5]: https://datafusion.apache.org/blog/2025/09/21/custom-types-using-metadata/ "Implementing User Defined Types and Custom Metadata in DataFusion - Apache DataFusion Blog"


# C6.16 Extension-type-aware return fields (DataFusion 54)

## C6.16.1 The DF-54 extension-type registry

DataFusion 54 upgrades the Field-metadata mechanism of this chapter into a governed extension-type system:

```text id="extension-type-pieces"
datafusion_expr::registry        ExtensionTypeRegistry trait + ExtensionTypeRegistration
                                 (register/resolve extension types by name;
                                  MemoryExtensionTypeRegistry::new_with_canonical_extension_types()
                                  preloads the Arrow canonical set)
datafusion_common::types         DFExtensionType / DFExtensionTypeRef
                                 (typed logical-type object resolved from field metadata)
arrow-schema 58.4 canonical set  bool8, json, uuid, opaque, fixed_shape_tensor,
                                 variable_shape_tensor, timestamp_with_offset
```

The registry resolves the standard Arrow field-metadata keys (`ARROW:extension:name`, `ARROW:extension:metadata`) into `DFExtensionType` values via `create_extension_type_for_field`, so an extension type declared on an input column survives as a *typed* object rather than an opaque string pair.

## C6.16.2 Interplay with `return_field_from_args`

`return_field_from_args` (C6.4) remains the UDF-side contract, and DF 54 makes its metadata obligations sharper:

```text id="extension-return-rules"
1. Inspect input extension types via ReturnFieldArgs.arg_fields[i].extension_type_name()
   / .extension_type_metadata() (Arrow Field accessors) — not by string-matching
   raw metadata maps.
2. If the function preserves the logical type (normalize/clamp/identity-shaped),
   copy BOTH extension keys onto the output Field (C6.7.1 pattern).
3. If the function changes the logical type, either emit the new extension type's
   keys or emit a plain Field — never leak the input's extension name onto an
   output whose storage/semantics no longer satisfy that extension.
4. If the function cannot operate on a given extension type, reject in planning
   (plan_err!) from return_field_from_args — not at runtime.
```

## C6.16.3 `with_metadata`, `cast_to_type`, `try_cast_to_type`

`datafusion-functions` core adds three functions that make field metadata first-class *inside expressions* (`datafusion_functions::core::expr_fn::{with_metadata, cast_to_type, try_cast_to_type}`; SQL names `with_metadata`, `cast_to_type`, `try_cast_to_type`):

```text id="metadata-fn-usage"
with_metadata(expression, key1, value1 [, key2, value2, ...])
    attaches Field metadata to the expression's output field — use to tag
    unit/semantic_type/extension keys without writing a UDF.
cast_to_type(expression, reference)
    field-aware cast: casts the first argument to the data type of the second
    (reference) argument — only the reference's TYPE is used, its value is
    ignored — so the target carries a full Field (extension name/metadata
    included), not just a storage DataType; errors on invalid input.
try_cast_to_type(expression, reference)
    same, but yields NULL instead of erroring on unconvertible values.
```

These compose with the C6 patterns: a pipeline can attach an extension type with `with_metadata`, and a metadata-aware UDF downstream sees it in `ReturnFieldArgs.arg_fields`. Field-aware casting is also what the 54 cast machinery uses internally — logical `Cast` now carries `field: FieldRef` (physical `CastExpr` carries `target_field`), so casts preserve extension metadata end to end.

## C6.16.4 Agent rules

```text id="extension-registry-agent"
Preserve Field metadata through UDF outputs deliberately: copy, replace, or strip —
  never accidentally drop extension keys by returning Field::new without metadata.
Register product extension types in the session's ExtensionTypeRegistry so casts
  and functions resolve them; do not parse ARROW:extension:* strings ad hoc.
Use with_metadata/cast_to_type in generated Expr pipelines before reaching for a
  metadata-only UDF.
Snapshot-test output Fields including metadata (C6.12) — extension-type regressions
  are invisible to DataType-only tests.
```

Primary reference for the registry itself: `datafusion_schemas_rust.md` S7 (extension-type registry deep dive). This section covers only the calculation-facing surface.

---

# DataFusion Advanced — C7) Null, NaN, infinity, error, and invalid-input semantics

## C7.0 Objective

Make calculation behavior explicit across **SQL semantics**, **Arrow array semantics**, **DataFusion planning/execution**, and **domain logic**:

```text id="c7-root"
Invalid / exceptional input
  ├─ NULL
  ├─ NaN
  ├─ +∞ / -∞
  ├─ malformed string
  ├─ missing nested field/key
  ├─ invalid enum/domain value
  ├─ decimal overflow
  ├─ division by zero
  ├─ vector length mismatch
  ├─ timestamp parse failure
  └─ physically impossible state

Behavior policy
  ├─ return NULL
  ├─ return NaN
  ├─ return ±∞
  ├─ clamp
  ├─ default
  ├─ raise planning error
  ├─ raise execution error
  ├─ emit side diagnostics
  └─ audited accepted/rejected record split
```

The attachment already covers UDF families, vectorized Arrow execution, registration, signatures, return typing, error handling, null testing, and expression-level null/conditional constructs; this section formalizes a semantic matrix for invalid data rather than treating invalid-input behavior as incidental implementation detail.   DataFusion scalar UDFs are vectorized: they receive Arrow arrays and return Arrow arrays with the same number of rows; `ScalarUDFImpl::invoke_with_args` returns `Result<ColumnarValue, DataFusionError>`, making invalid-input behavior a first-class execution contract rather than a panic path. ([Apache DataFusion][1])

---

## C7.1 Semantic layers

```text id="semantic-layers"
SQL layer
  ├─ NULL / three-valued logic
  ├─ casts / try-casts
  ├─ CASE / COALESCE / NULLIF
  ├─ built-in scalar behavior
  └─ planning vs execution error distinction

Logical Expr layer
  ├─ Expr::IsNull / IsNotNull
  ├─ Expr::Case
  ├─ Expr type/nullability inference
  ├─ UDF signature/coercion
  └─ optimizer visibility

Arrow layer
  ├─ array null bitmap
  ├─ floating NaN / ±∞ values
  ├─ typed arrays
  ├─ ScalarValue::<T>(None)
  ├─ typed vs untyped NULL
  └─ RecordBatch schema nullability

Domain layer
  ├─ invalid enum/code
  ├─ unit/basis mismatch
  ├─ out-of-range physical value
  ├─ malformed identifiers/timestamps
  ├─ vector dimension mismatch
  └─ audit/diagnostic requirements

Execution layer
  ├─ Result<T, DataFusionError>
  ├─ plan_err! for planning/user query failures
  ├─ exec_err! for execution-time value failures
  ├─ internal_err! for impossible invariant failures
  └─ never panic on user/data input
```

DataFusion expressions are the computation abstraction used by `select`, `filter`, and other DataFrame methods, and expression builders such as `col("a").gt(lit(6)).and(...)` are the normal way to encode SQL-visible calculation logic. ([Apache DataFusion][2]) DataFusion’s error module provides macros such as `plan_err!` for planning errors and `exec_err!` for expected execution errors; its source examples show `plan_err!` producing “Error during planning” messages. ([Docs.rs][3])

---

## C7.2 Behavior category matrix

| Behavior                   | Meaning                                            | Planning? |  Execution? | Use when                                                         | Avoid when                                          |
| -------------------------- | -------------------------------------------------- | --------: | ----------: | ---------------------------------------------------------------- | --------------------------------------------------- |
| return NULL                | invalid/unknown value represented as SQL null      |        no |         yes | missing/dirty value is data-quality issue, not query failure     | invalid input should stop pipeline                  |
| return NaN                 | IEEE floating invalid numeric result               |        no |         yes | floating scientific semantics; NaN is meaningful downstream      | SQL users expect null/error                         |
| return ±∞                  | IEEE overflow/asymptote result                     |        no |         yes | mathematical function intentionally exposes infinity             | storage/export consumers reject infinity            |
| raise planning error       | query/function call invalid before data evaluation |       yes |          no | wrong arity/type, invalid literal shape, impossible return field | row-specific data issue                             |
| raise execution error      | row/batch value invalid during execution           |        no |         yes | strict validation, unsafe domain value, vector mismatch          | dirty ingestion needs row-level reject              |
| clamp                      | replace out-of-range value with boundary           |        no |         yes | physical bounded metric, UI-friendly normalization               | data-quality audit requires original invalid value  |
| default                    | replace invalid/missing value with default         |     maybe |         yes | compatibility, display, optional field default                   | analytical metric where imputation must be explicit |
| emit diagnostic side table | separate accepted/rejected records                 |     maybe | yes/outside | ETL validation/audit                                             | pure SQL scalar UDF alone cannot emit side relation |
| structured result          | return `{value,status,reason}` struct              |        no |         yes | UDF must preserve row count and expose diagnostics inline        | consumers require primitive output                  |

Agent rule:

```text id="behavior-agent-rule"
Every public calculation must declare exactly one invalid-input policy per invalid-input class.
Do not let Rust/Arrow/math-library defaults accidentally define product semantics.
```

---

## C7.3 Function modes

## C7.3.1 Strict mode

```text id="strict-mode"
strict:
  invalid input -> DataFusionError::Execution
  wrong call shape/type -> DataFusionError::Plan
  null input -> error or null depending explicit NullPolicy
```

Use when:

```text id="strict-use"
invalid input means query result is unsafe
data is expected to be clean
failure should stop pipeline
validation is part of contract
```

Example:

```sql id="strict-sql"
SELECT strict_parse_api_code(raw_code) AS api_code
FROM streams;
```

Rust pattern:

```rust id="strict-rust"
if !is_valid_code(code) {
    return exec_err!(
        "strict_parse_api_code invalid code `{}` at row {}; expected pattern API-[0-9]+",
        code,
        row_idx
    );
}
```

---

## C7.3.2 Tolerant mode

```text id="tolerant-mode"
tolerant:
  invalid input -> NULL / default / clamp
  query continues
  invalid row may be counted only if diagnostics are implemented
```

Use when:

```text id="tolerant-use"
dirty source data is expected
pipeline should continue
invalid values can be represented as missing/defaulted
```

Example:

```sql id="tolerant-sql"
SELECT tolerant_parse_api_code(raw_code) AS api_code
FROM streams;
```

---

## C7.3.3 `try_` variant

```text id="try-mode"
try_ variant:
  invalid non-null input -> NULL
  null input -> NULL
  type/signature errors still fail planning
  no audit unless paired with rejection query
```

DataFusion already exposes `arrow_try_cast`, which casts to a specific Arrow type and returns `NULL` on cast failure; the attachment recommends pairing `arrow_try_cast` with rejected-record queries to avoid silent data loss. 

Example:

```sql id="try-sql"
WITH parsed AS (
  SELECT
    raw_ts,
    try_parse_operating_ts(raw_ts) AS operating_ts
  FROM raw_events
)
SELECT *
FROM parsed
WHERE operating_ts IS NOT NULL;
```

Rejected rows:

```sql id="try-reject-sql"
WITH parsed AS (
  SELECT
    raw_ts,
    try_parse_operating_ts(raw_ts) AS operating_ts
  FROM raw_events
)
SELECT *
FROM parsed
WHERE raw_ts IS NOT NULL
  AND operating_ts IS NULL;
```

Agent rule:

```text id="try-agent"
Every try_ function used for ETL should have an accepted-query and rejected-query template.
```

---

## C7.3.4 Audited variant

```text id="audited-mode"
audited:
  output = Struct(value, status, reason, code)
  preserves row count
  no side channel required
  caller can filter status
```

Example SQL:

```sql id="audited-sql"
SELECT
  event_id,
  parse_timestamp_audited(raw_ts) AS ts_parse
FROM raw_events;
```

Then:

```sql id="audited-extract-sql"
SELECT
  event_id,
  ts_parse['value'] AS event_ts,
  ts_parse['status'] AS parse_status,
  ts_parse['reason'] AS parse_reason
FROM (
  SELECT event_id, parse_timestamp_audited(raw_ts) AS ts_parse
  FROM raw_events
);
```

Return shape:

```text id="audited-return-shape"
Struct<
  value: Timestamp(Nanosecond, Some("UTC")) nullable,
  status: Utf8 non-null,
  reason: Utf8 nullable,
  code: Utf8 nullable
>
```

Agent rule:

```text id="audited-agent"
Use audited variants when scalar UDF must expose diagnostics without breaking row cardinality.
Use external ETL/table sink when diagnostics must be materialized as a separate reject table.
```

---

# C7.4 Planning error vs execution error

## C7.4.1 Planning error

Use for errors knowable before scanning data:

```text id="planning-error-use"
wrong arity
wrong argument type
unsupported literal parameter
invalid return shape literal
invalid enum of function mode argument
unsupported timestamp unit
unsupported vector dimension literal
missing required static field name
```

Rust pattern:

```rust id="plan-err-pattern"
fn coerce_types(&self, arg_types: &[DataType]) -> Result<Vec<DataType>> {
    if arg_types.len() != 2 {
        return plan_err!("{} expects 2 arguments, got {}", self.name(), arg_types.len());
    }

    if !is_numeric(&arg_types[0]) || !is_numeric(&arg_types[1]) {
        return plan_err!(
            "{} expects numeric arguments, got {:?}, {:?}",
            self.name(),
            arg_types[0],
            arg_types[1]
        );
    }

    Ok(vec![DataType::Float64, DataType::Float64])
}
```

Literal-return-shape example:

```rust id="planning-literal-error"
let Some(Some(dim)) = args.scalar_arguments.get(1) else {
    return plan_err!("make_fixed_vector dimension must be a scalar integer literal");
};
```

---

## C7.4.2 Execution error

Use for row/batch values discovered only during execution:

```text id="execution-error-use"
malformed row value in strict parser
vector length mismatch between two row values
decimal overflow during arithmetic
domain value outside permitted range
invalid enum code in data row
timestamp string malformed in strict mode
```

Rust pattern:

```rust id="exec-err-pattern"
if denominator == 0.0 {
    return exec_err!(
        "strict_divide denominator was zero at row {}; use safe_divide or try_divide for null-on-zero behavior",
        row_idx
    );
}
```

The DataFusion error docs distinguish expected/user-facing errors from internal invariant failures and recommend macros such as `exec_err!` for expected errors and assertion/internal-error macros for invariant failures. ([Docs.rs][4])

---

## C7.4.3 Internal error

Use when code reaches an impossible state after planning/coercion:

```rust id="internal-err-pattern"
let arr = input
    .as_any()
    .downcast_ref::<Float64Array>()
    .ok_or_else(|| internal_datafusion_err!(
        "{} expected Float64Array after coercion, got {:?}",
        self.name(),
        input.data_type()
    ))?;
```

Agent rule:

```text id="internal-agent"
If user data can cause it, it is not internal.
If only a bug in signature/coercion/implementation can cause it, use internal error.
```

---

# C7.5 Formal invalid-input semantics matrix

| Invalid input            | Strict                      | Tolerant         | `try_` | Audited                     | Notes                                                 |
| ------------------------ | --------------------------- | ---------------- | ------ | --------------------------- | ----------------------------------------------------- |
| null input               | error or null by NullPolicy | default/null     | null   | status `null_input`         | null is not malformed                                 |
| division by zero         | execution error             | null or infinity | null   | status `zero_denominator`   | choose SQL-like vs IEEE-like behavior                 |
| log negative             | execution error or NaN      | NaN/null         | null   | status `domain_error`       | math library default may be NaN                       |
| sqrt negative            | execution error or NaN      | NaN/null         | null   | status `domain_error`       | do not accidentally inherit Rust default              |
| invalid enum             | execution error             | default/unknown  | null   | status `invalid_enum`       | often should be audited                               |
| malformed timestamp      | execution error             | null/default     | null   | status `parse_error`        | parsing functions should have strict/try variants     |
| decimal overflow         | execution error             | null/clamp       | null   | status `overflow`           | avoid silent float fallback                           |
| vector length mismatch   | execution error             | null             | null   | status `dimension_mismatch` | row-level mismatch; planning catches fixed dimensions |
| missing struct field     | planning error if static    | null if dynamic  | null   | status `missing_field`      | prefer native field access for static fields          |
| map key absent           | null/default                | null/default     | null   | status `missing_key`        | distinguish absent vs present-null if needed          |
| all-null aggregate group | null or default             | null/default     | null   | status `all_null`           | count-like functions may return 0                     |
| empty array/list         | error/null/default          | null/default     | null   | status `empty_input`        | distinguish empty from null                           |

---

# C7.6 Detailed examples

## C7.6.1 Division by zero

### Preferred function split

```text id="division-functions"
strict_divide(n, d) -> error on d = 0
safe_divide(n, d)   -> NULL on d = 0
ieee_divide(n, d)   -> ±∞ or NaN according to IEEE floating semantics
audited_divide(n,d) -> Struct(value, status, reason)
```

### SQL behavior

```sql id="division-sql"
SELECT
  safe_divide(numerator, denominator) AS ratio
FROM metrics;
```

### Rust safe pattern

```rust id="safe-divide-rust"
fn safe_divide_scalar(n: Option<f64>, d: Option<f64>) -> Option<f64> {
    match (n, d) {
        (Some(_), Some(0.0)) => None,
        (Some(n), Some(d)) => Some(n / d),
        _ => None,
    }
}
```

### IEEE pattern

```rust id="ieee-divide-rust"
fn ieee_divide_scalar(n: Option<f64>, d: Option<f64>) -> Option<f64> {
    match (n, d) {
        (Some(n), Some(d)) => Some(n / d), // Rust f64: inf / NaN as applicable
        _ => None,
    }
}
```

### Agent rule

```text id="division-agent"
Never name a null-on-zero function `divide` without documenting zero behavior.
Prefer `safe_divide` / `strict_divide` / `ieee_divide` names.
```

---

## C7.6.2 `log` negative

### Semantic options

```text id="log-negative-options"
strict_log(x):
  x <= 0 -> execution error

safe_log(x):
  x <= 0 -> NULL

ieee_log(x):
  x < 0 -> NaN
  x = 0 -> -∞

audited_log(x):
  Struct(value, status, reason)
```

DataFusion’s expression documentation notes Rust-like mathematical behavior for some corner cases such as `log(-1)` returning `NaN`, `log(0)` returning `-inf`, and `sqrt(-1)` returning `NaN`; if a domain-specific UDF needs different semantics, it must encode them explicitly. 

### Rust safe pattern

```rust id="safe-log"
fn safe_ln(x: Option<f64>) -> Option<f64> {
    match x {
        Some(v) if v > 0.0 => Some(v.ln()),
        Some(_) => None,
        None => None,
    }
}
```

---

## C7.6.3 `sqrt` negative

```text id="sqrt-options"
strict_sqrt:
  x < 0 -> execution error

safe_sqrt:
  x < 0 -> NULL

ieee_sqrt:
  x < 0 -> NaN
```

Rust pattern:

```rust id="sqrt-rust"
fn safe_sqrt(x: Option<f64>) -> Option<f64> {
    match x {
        Some(v) if v >= 0.0 => Some(v.sqrt()),
        Some(_) => None,
        None => None,
    }
}
```

---

## C7.6.4 Invalid enum

### Example

```text id="enum-example"
stream_phase ∈ {liquid, vapor, mixed}
```

### Strict parser

```rust id="strict-enum"
fn parse_phase_strict(value: &str, row_idx: usize) -> Result<Phase> {
    match value {
        "liquid" => Ok(Phase::Liquid),
        "vapor" => Ok(Phase::Vapor),
        "mixed" => Ok(Phase::Mixed),
        other => exec_err!(
            "invalid stream_phase `{}` at row {}; expected liquid|vapor|mixed",
            other,
            row_idx
        ),
    }
}
```

### Tolerant parser

```rust id="tolerant-enum"
fn parse_phase_tolerant(value: Option<&str>) -> Option<&'static str> {
    match value {
        Some("liquid") => Some("liquid"),
        Some("vapor") => Some("vapor"),
        Some("mixed") => Some("mixed"),
        Some(_) | None => None,
    }
}
```

### Audited struct

```text id="enum-audit-struct"
Struct<
  value: Utf8 nullable,
  status: Utf8 non-null,     -- ok | invalid_enum | null_input
  reason: Utf8 nullable
>
```

Agent rule:

```text id="enum-agent"
For domain enums, prefer strict or audited over silent default unless `unknown` is a real domain value.
```

---

## C7.6.5 Malformed timestamp

### Function family

```text id="timestamp-functions"
parse_ts_strict(raw, format, timezone) -> Timestamp(...), error on malformed
try_parse_ts(raw, format, timezone)    -> Timestamp(...), NULL on malformed
parse_ts_audited(...)                  -> Struct(value,status,reason)
```

### Planning errors

```text id="timestamp-plan-errors"
format argument not scalar literal
unsupported timezone literal
unsupported timestamp unit
wrong arity
wrong input type
```

### Execution errors

```text id="timestamp-exec-errors"
row value does not match format
date impossible: 2026-02-30
ambiguous/nonexistent local time if timezone semantics require rejection
```

### SQL accepted/rejected pattern

```sql id="timestamp-audit-sql"
WITH parsed AS (
  SELECT
    raw_ts,
    try_parse_ts(raw_ts, '%Y-%m-%d %H:%M:%S', 'UTC') AS ts
  FROM raw_events
)
SELECT *
FROM parsed
WHERE raw_ts IS NOT NULL
  AND ts IS NULL;
```

---

## C7.6.6 Decimal overflow

### Policy matrix

| Mode        | Behavior                                         |
| ----------- | ------------------------------------------------ |
| strict      | execution error on overflow                      |
| try         | null on overflow                                 |
| clamp       | max/min decimal value                            |
| approximate | cast to Float64 with explicit function name/docs |
| audited     | struct with `overflow` status                    |

### Rust policy sketch

```rust id="decimal-overflow-policy"
pub enum DecimalOverflowPolicy {
    Error,
    Null,
    Clamp,
    ApproximateFloat,
}
```

### Agent rules

```text id="decimal-agent-rules"
Financial/exact functions should default to Error or Null, not ApproximateFloat.
Clamp only when domain explicitly supports saturation.
Decimal precision/scale must be documented and tested.
```

---

## C7.6.7 Vector length mismatch

### Planning-time dimension check

```text id="vector-planning"
FixedSizeList<Float64, 384> vs FixedSizeList<Float64, 384>:
  planning can validate shape

List<Float64> vs List<Float64>:
  runtime must validate each row length
```

### Runtime error

```rust id="vector-length-rust"
if left.len() != right.len() {
    return exec_err!(
        "vector_distance length mismatch at row {}: left={}, right={}",
        row_idx,
        left.len(),
        right.len()
    );
}
```

### Tolerant version

```rust id="vector-length-tolerant"
if left.len() != right.len() {
    output.append_null();
    continue;
}
```

### Agent rule

```text id="vector-agent"
Use FixedSizeList when dimension is schema-level invariant.
Use audited or try variant when variable List dimensions may be dirty.
```

---

## C7.6.8 Missing struct field

### Static field

```sql id="static-field"
SELECT payload['user']['id'] AS user_id
FROM events;
```

If field access is static and schema-aware, a missing field should normally be a planning/schema error.

### Dynamic field

```sql id="dynamic-field"
SELECT dynamic_get_field(payload, requested_field) AS value
FROM events;
```

Dynamic missing-field policy must be explicit:

```text id="missing-field-policy"
strict_dynamic_get_field:
  missing field -> execution error

try_dynamic_get_field:
  missing field -> NULL

audited_dynamic_get_field:
  Struct(value,status,reason)
```

Agent rule:

```text id="missing-struct-agent"
Prefer native static field access for known fields.
Use dynamic field UDF only when field names are data.
```

---

## C7.6.9 Map key absent

### Semantics to distinguish

```text id="map-semantics"
map is NULL
key is NULL
key absent
key present with NULL value
key present with non-null value
duplicate key if map construction allows/encounters it
```

### Policy

```text id="map-policy"
element_at/map_extract style:
  absent key -> NULL

strict_map_get:
  absent key -> execution error

map_get_default:
  absent key -> default

map_get_audited:
  Struct(value,status,reason)
```

### Agent rule

```text id="map-agent-rule"
If absent key and present-null must be distinguishable, return audited struct or explicit boolean flags.
Primitive nullable output cannot distinguish those cases.
```

---

# C7.7 SQL/Expr tools for invalid-input control

## C7.7.1 `CASE`

```sql id="case-sql"
SELECT
  CASE
    WHEN denominator IS NULL THEN NULL
    WHEN denominator = 0 THEN NULL
    ELSE numerator / denominator
  END AS safe_ratio
FROM t;
```

Use when logic is composable and optimizer-visible.

## C7.7.2 `NULLIF`

```sql id="nullif-sql"
SELECT
  numerator / NULLIF(denominator, 0) AS safe_ratio
FROM t;
```

Use for compact zero-to-null denominator conversion.

## C7.7.3 `COALESCE`

```sql id="coalesce-sql"
SELECT
  COALESCE(parsed_value, default_value, 0.0) AS value_with_default
FROM t;
```

Use when defaulting is intentional and documented.

## C7.7.4 `arrow_try_cast`

```sql id="arrow-try-cast-sql"
SELECT
  arrow_try_cast(raw_amount, 'Decimal128(20, 4)') AS amount
FROM raw_t;
```

Use for dirty ingestion; pair with rejected-row query. The attachment documents `arrow_try_cast` as returning `NULL` when cast fails and recommends rejected-record audits when data quality matters. 

## C7.7.5 DataFrame expression equivalent

```rust id="expr-equivalent"
let safe_ratio =
    (col("numerator") / nullif(col("denominator"), lit(0.0)))
        .alias("safe_ratio");

let df = df.select(vec![safe_ratio])?;
```

Agent rule:

```text id="expr-agent"
Prefer Expr/SQL constructs for simple invalid-input handling.
Use UDF only when invalid-input behavior cannot be expressed transparently.
```

---

# C7.8 Rust UDF implementation policies

## C7.8.1 Shared policy enum

```rust id="invalid-policy-enum"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidInputPolicy {
    Error,
    Null,
    NaN,
    Infinity,
    Clamp,
    Default,
    AuditedStruct,
}
```

## C7.8.2 Domain error enum

```rust id="domain-error-enum"
#[derive(Debug, thiserror::Error)]
pub enum CalcInputError {
    #[error("division by zero")]
    DivisionByZero,

    #[error("negative value outside domain: {0}")]
    NegativeDomain(f64),

    #[error("invalid enum value: {0}")]
    InvalidEnum(String),

    #[error("malformed timestamp: {0}")]
    MalformedTimestamp(String),

    #[error("decimal overflow")]
    DecimalOverflow,

    #[error("vector length mismatch: left={left}, right={right}")]
    VectorLengthMismatch { left: usize, right: usize },

    #[error("missing field: {0}")]
    MissingField(String),

    #[error("map key absent: {0}")]
    MapKeyAbsent(String),
}
```

## C7.8.3 Policy application helper

```rust id="policy-helper"
pub fn handle_invalid_f64(
    policy: InvalidInputPolicy,
    err: CalcInputError,
) -> datafusion::error::Result<Option<f64>> {
    match policy {
        InvalidInputPolicy::Error => exec_err!("{err}"),
        InvalidInputPolicy::Null => Ok(None),
        InvalidInputPolicy::NaN => Ok(Some(f64::NAN)),
        InvalidInputPolicy::Infinity => match err {
            CalcInputError::DivisionByZero => Ok(Some(f64::INFINITY)),
            _ => exec_err!("cannot convert `{err}` to infinity"),
        },
        InvalidInputPolicy::Clamp => exec_err!("clamp requires function-specific bounds"),
        InvalidInputPolicy::Default => exec_err!("default requires function-specific default value"),
        InvalidInputPolicy::AuditedStruct => {
            exec_err!("audited struct must be handled by struct-returning function")
        }
    }
}
```

## C7.8.4 Never panic pattern

Bad:

```rust id="bad-panic"
let arr = args[0].as_any().downcast_ref::<Float64Array>().unwrap();
let value = arr.value(i).ln(); // no domain policy
```

Good:

```rust id="good-error"
let arr = args[0]
    .as_any()
    .downcast_ref::<Float64Array>()
    .ok_or_else(|| internal_datafusion_err!(
        "safe_ln expected Float64Array after coercion, got {:?}",
        args[0].data_type()
    ))?;

if arr.is_null(i) {
    builder.append_null();
} else {
    let v = arr.value(i);
    if v <= 0.0 {
        builder.append_null(); // safe_ln policy
    } else {
        builder.append_value(v.ln());
    }
}
```

---

# C7.9 Audited struct return pattern

## C7.9.1 Return field

```rust id="audited-return-field"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field};

pub fn audited_f64_field(name: &str) -> Field {
    Field::new_struct(
        name,
        vec![
            Arc::new(Field::new("value", DataType::Float64, true)),
            Arc::new(Field::new("status", DataType::Utf8, false)),
            Arc::new(Field::new("reason", DataType::Utf8, true)),
            Arc::new(Field::new("code", DataType::Utf8, true)),
        ],
        false,
    )
}
```

## C7.9.2 Status vocabulary

```text id="status-vocab"
ok
null_input
division_by_zero
domain_error
parse_error
invalid_enum
overflow
dimension_mismatch
missing_field
missing_key
internal_error
```

## C7.9.3 SQL consumption

```sql id="audited-consumption"
WITH checked AS (
  SELECT
    id,
    audited_divide(numerator, denominator) AS div
  FROM t
)
SELECT
  id,
  div['value'] AS ratio,
  div['status'] AS ratio_status,
  div['reason'] AS ratio_reason
FROM checked
WHERE div['status'] <> 'ok';
```

Agent rules:

```text id="audited-struct-agent"
Use non-null `status`.
Use nullable `value`.
Use stable machine-readable `code`.
Use human-readable `reason`.
Do not encode diagnostics as unstructured string only.
```

---

# C7.10 Diagnostic side table pattern

Scalar UDFs cannot directly emit a second relation while preserving ordinary scalar call semantics. For ETL-grade diagnostics, use a planned two-output pipeline, UDTF/TableProvider, or explicit accepted/rejected queries.

## C7.10.1 Accepted/rejected SQL split

```sql id="accepted-rejected"
WITH parsed AS (
  SELECT
    *,
    try_parse_ts(raw_ts) AS parsed_ts
  FROM raw_events
)
SELECT *
FROM parsed
WHERE parsed_ts IS NOT NULL;
```

```sql id="rejected"
WITH parsed AS (
  SELECT
    *,
    try_parse_ts(raw_ts) AS parsed_ts
  FROM raw_events
)
SELECT *
FROM parsed
WHERE raw_ts IS NOT NULL
  AND parsed_ts IS NULL;
```

## C7.10.2 Audited struct split

```sql id="audited-split"
WITH checked AS (
  SELECT
    *,
    parse_ts_audited(raw_ts) AS ts_check
  FROM raw_events
)
SELECT *
FROM checked
WHERE ts_check['status'] = 'ok';
```

```sql id="audited-reject"
WITH checked AS (
  SELECT
    *,
    parse_ts_audited(raw_ts) AS ts_check
  FROM raw_events
)
SELECT *
FROM checked
WHERE ts_check['status'] <> 'ok';
```

## C7.10.3 UDTF/table-provider diagnostic pattern

```sql id="diagnostic-udtf"
SELECT *
FROM validate_events('s3://bucket/raw/events/')
WHERE status <> 'ok';
```

Use when validation itself is relation-producing and diagnostics are primary output.

---

# C7.11 NaN and infinity policy

## C7.11.1 Floating values are not SQL NULL

```text id="nan-not-null"
NULL:
  unknown/missing SQL value
  tracked by Arrow null bitmap
  IS NULL returns true

NaN:
  concrete Float32/Float64 payload
  not null
  arithmetic may propagate NaN
  equality/order semantics can be surprising

Infinity:
  concrete Float32/Float64 payload
  not null
  may not serialize cleanly to every downstream format
```

## C7.11.2 Policy table

| Domain                        | Recommended invalid numeric behavior                    |
| ----------------------------- | ------------------------------------------------------- |
| financial exactness           | error/null; never NaN/inf                               |
| engineering physical quantity | error/null/audited; NaN only if downstream expects IEEE |
| scientific exploratory        | NaN/inf acceptable if documented                        |
| public SQL analytics          | null/error usually safer                                |
| ML feature computation        | NaN/inf only if model pipeline handles them             |
| Parquet lake output           | test downstream reader behavior for NaN/inf             |
| dashboards                    | null/default/clamp preferred                            |

## C7.11.3 Cleanup functions

```sql id="nan-cleanup"
SELECT
  CASE
    WHEN isnan(metric) THEN NULL
    ELSE metric
  END AS metric_no_nan
FROM t;
```

DataFusion scalar functions include math predicates/functions such as `isnan` and NaN-handling helpers such as `nanvl` in the SQL scalar function catalog. ([Apache DataFusion][5])

---

# C7.12 Empty arrays, all-null arrays, and zero-row batches

## C7.12.1 Required distinctions

```text id="empty-distinctions"
zero-row RecordBatch:
  no rows to evaluate
  scalar UDF array output length must be 0

empty list value:
  row exists
  list length = 0

null list value:
  row exists
  list is null

all-null array:
  rows exist
  every slot is null

all-null aggregate group:
  group exists
  no non-null values

empty aggregate input:
  no rows
```

## C7.12.2 Scalar UDF test cases

```text id="empty-tests"
[ ] zero-row input array -> zero-row output array
[ ] all-null input -> output according to NullPolicy
[ ] mixed-null input -> row-aligned output
[ ] scalar NULL input -> scalar NULL/error according to policy
[ ] empty list -> distinct from null list
[ ] all-null list elements -> policy-specific
```

## C7.12.3 Rust output length rule

```rust id="empty-length-rule"
if let ColumnarValue::Array(out) = &result {
    assert_eq!(out.len(), args.number_rows);
}
```

---

# C7.13 Manifest schema

## C7.13.1 Invalid-input policy manifest

```yaml id="invalid-policy-yaml"
invalid_input_policy:
  mode: audited
  null_input:
    behavior: return_null
  nan_input:
    behavior: preserve
  infinity_input:
    behavior: error
  invalid_domain:
    behavior: return_null
    code: domain_error
  parse_error:
    behavior: return_null
    code: parse_error
  overflow:
    behavior: error
  vector_length_mismatch:
    behavior: error
  missing_struct_field:
    behavior: plan_error
  map_key_absent:
    behavior: return_null

diagnostics:
  audited_struct: true
  status_field: status
  reason_field: reason
  code_field: code
  rejected_query_template: true
```

## C7.13.2 Rust model

```rust id="invalid-policy-rust"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvalidInputPolicySpec {
    pub mode: FunctionMode,
    pub null_input: BehaviorSpec,
    pub nan_input: BehaviorSpec,
    pub infinity_input: BehaviorSpec,
    pub invalid_domain: BehaviorSpec,
    pub parse_error: BehaviorSpec,
    pub overflow: BehaviorSpec,
    pub vector_length_mismatch: BehaviorSpec,
    pub missing_struct_field: BehaviorSpec,
    pub map_key_absent: BehaviorSpec,
    pub diagnostics: DiagnosticSpec,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Behavior {
    ReturnNull,
    ReturnNaN,
    ReturnInfinity,
    PlanError,
    ExecutionError,
    Clamp,
    Default,
    AuditedStatus,
}
```

---

# C7.14 Testing matrix

## C7.14.1 Required test fixtures

```text id="required-fixtures"
null input
typed null literal
untyped null literal if SQL path matters
NaN input
+∞ input
-∞ input
zero denominator
negative log/sqrt input
invalid enum string
malformed timestamp string
decimal overflow case
vector length mismatch
missing struct field
map key absent
empty array
zero-row batch
all-null array
mixed-null array
```

## C7.14.2 SQL tests

```sql id="invalid-sql-tests"
SELECT safe_divide(1.0, 0.0) IS NULL AS div_zero_null;

SELECT isnan(ieee_log(-1.0)) AS log_negative_nan;

SELECT try_parse_ts('not-a-timestamp') IS NULL AS bad_ts_null;

SELECT strict_parse_enum('bad_value');
-- expected execution error containing invalid enum
```

## C7.14.3 Arrow/kernel tests

```rust id="invalid-arrow-tests"
#[test]
fn safe_divide_zero_denominator_returns_null() -> datafusion::error::Result<()> {
    let n = Float64Array::from(vec![Some(1.0), Some(2.0)]);
    let d = Float64Array::from(vec![Some(0.0), Some(2.0)]);

    let out = safe_divide_f64_arrays(&n, &d)?;

    assert!(out.is_null(0));
    assert_eq!(out.value(1), 1.0);

    Ok(())
}
```

## C7.14.4 Error tests

```rust id="error-tests"
#[tokio::test]
async fn strict_divide_zero_denominator_errors() {
    let ctx = test_context_with_functions();

    let err = ctx
        .sql("SELECT strict_divide(1.0, 0.0)")
        .await
        .unwrap()
        .collect()
        .await
        .expect_err("strict_divide should fail on zero denominator");

    let msg = err.to_string();
    assert!(msg.contains("strict_divide"));
    assert!(msg.contains("zero"));
}
```

## C7.14.5 Audited tests

```sql id="audited-tests"
WITH checked AS (
  SELECT audited_divide(1.0, 0.0) AS div
)
SELECT
  div['status'] AS status,
  div['code'] AS code
FROM checked;
```

Expected:

```text id="audited-expected"
status = 'error'
code = 'division_by_zero'
```

---

# C7.15 Deployment policy

## C7.15.1 Public SQL

```text id="public-deploy"
Preferred:
  NULL-on-invalid or audited variants
  strict variants only when user expects query failure
  clear function names: try_, strict_, safe_, audited_, ieee_

Required:
  documented null/NaN/inf policy
  denied side effects
  no panics
  wrong-type planning tests
  invalid-value execution tests
```

## C7.15.2 ETL pipelines

```text id="etl-deploy"
Preferred:
  try_ + rejected query
  audited struct
  side diagnostic table
  all invalid rows countable

Avoid:
  silent defaulting
  silent NaN/inf
  query-aborting strict functions unless source is expected clean
```

## C7.15.3 Engineering calculations

```text id="engineering-deploy"
Preferred:
  strict or audited for physically impossible values
  safe/null-returning for expected missing values
  explicit NaN/inf only for IEEE-scientific workflows

Required:
  units/basis in error messages where useful
  domain bounds documented
  out-of-range tests
```

## C7.15.4 ML feature pipelines

```text id="ml-deploy"
Policy must state:
  whether NaN is allowed
  whether infinity is allowed
  whether nulls are imputed later
  whether vector dimension mismatch is reject/null/error
  whether feature clipping is applied
```

---

# C7.16 Error message templates

## C7.16.1 Planning error

```text id="planning-template"
Function `{function}` cannot be planned.

Expected:
  {signature}

Received:
  {actual_types}

Reason:
  {reason}

Suggested fix:
  {suggestion}
```

## C7.16.2 Execution error

```text id="execution-template"
Function `{function}` failed during execution.

Invalid value class:
  {code}

Reason:
  {reason}

Policy:
  {policy}

Suggested fix:
  use `{alternative_function}` or filter/clean input before calling this function.
```

## C7.16.3 Diagnostic status record

```text id="diagnostic-template"
status:
  ok | null_input | domain_error | parse_error | overflow | dimension_mismatch | missing_field | missing_key

code:
  stable machine-readable enum

reason:
  human-readable text, not parsed by code
```

---

# C7.17 Anti-pattern inventory

* `unwrap()` on downcast or parse result.
* Panic on malformed user data.
* Returning NaN accidentally from Rust math when SQL contract says null.
* Returning infinity accidentally from division when SQL contract says error/null.
* Treating null and NaN as equivalent.
* Treating empty list and null list as equivalent.
* Hiding invalid enum as `"unknown"` when unknown is not a real domain value.
* `try_` function with no rejected-row template.
* Strict function used in dirty ingestion pipeline.
* Decimal overflow cast to Float64 silently.
* Timestamp parser drops timezone or defaults timezone silently.
* Vector distance panics on length mismatch.
* Dynamic struct field lookup panics on missing field.
* Map absent key indistinguishable from present-null when product requires distinction.
* Audited result has free-text status without stable code.
* Error messages omit function name and expected behavior.
* Tests cover happy path only.
* No zero-row/all-null/mixed-null tests.
* No NaN/inf tests for floating functions.
* No policy manifest for invalid-input behavior.

---

# C7.18 Agent checklist

```text id="c7-final-checklist"
[ ] Define invalid-input policy before implementation.
[ ] Choose mode: strict, tolerant, try_, audited.
[ ] Distinguish NULL, NaN, +∞, -∞, empty list, null list, absent key, present-null.
[ ] Decide planning error vs execution error for each invalid case.
[ ] Use plan_err! for invalid call shape/type/literal planning failures.
[ ] Use exec_err! for expected execution-time invalid values in strict mode.
[ ] Use internal errors only for impossible post-planning invariants.
[ ] Never panic, unwrap, or expect on user/data input.
[ ] Document null policy.
[ ] Document NaN/infinity policy.
[ ] Document decimal overflow policy.
[ ] Document timestamp timezone/unit policy.
[ ] Document vector dimension policy.
[ ] Provide safe_/strict_/try_/audited_ variants when semantics differ.
[ ] Prefer Expr/CASE/NULLIF/COALESCE for simple invalid-input behavior.
[ ] Use audited Struct output for row-preserving diagnostics.
[ ] Use accepted/rejected queries or UDTF/TableProvider for side diagnostics.
[ ] Test empty arrays.
[ ] Test zero-row batches.
[ ] Test all-null arrays.
[ ] Test mixed-null arrays.
[ ] Test scalar NULL.
[ ] Test NaN and ±∞.
[ ] Test invalid domain values.
[ ] Test malformed strings/timestamps.
[ ] Test decimal overflow.
[ ] Test vector length mismatch.
[ ] Test missing struct field and map key absence.
[ ] Snapshot stable diagnostic codes, not free-text reason strings.
```

[1]: https://datafusion.apache.org/library-user-guide/functions/adding-udfs.html?utm_source=chatgpt.com "Adding User Defined Functions: Scalar/Window/Aggregate ..."
[2]: https://datafusion.apache.org/user-guide/expressions.html?utm_source=chatgpt.com "Expression API — Apache DataFusion documentation"
[3]: https://docs.rs/crate/datafusion-common/latest/source/src/error.rs?utm_source=chatgpt.com "datafusion-common 54.1.0"
[4]: https://docs.rs/datafusion-python/latest/datafusion_python/datafusion_common/error/index.html?utm_source=chatgpt.com "datafusion_python::datafusion_common::error - Rust"
[5]: https://datafusion.apache.org/user-guide/sql/scalar_functions.html?utm_source=chatgpt.com "Scalar Functions — Apache DataFusion documentation"


# C7.19 Filter evaluation order is optimizer-controlled (DataFusion 54)

## C7.19.1 The contract change

DataFusion 54's optimizer reorders conjunctive filter predicates by estimated evaluation cost (cheap-before-expensive), and may split, push down, or reorder them across the plan. Consequence for every calculation in this chapter: **textual `AND` ordering in SQL is not an evaluation-order guarantee.** A guard predicate written to the left of an expression does not reliably run first:

```sql
-- UNSAFE in 54 (and never contractual): the optimizer may evaluate the
-- CAST/divide predicate before (or instead of) the guard, on rows the
-- guard would have excluded.
SELECT *
FROM readings
WHERE raw_value ~ '^[0-9]+$' AND CAST(raw_value AS DOUBLE) > threshold;

SELECT *
FROM flows
WHERE denominator <> 0.0 AND numerator / denominator > 3.5;
```

Predicates in a conjunction must therefore each be **total**: safe to evaluate on any row of the input, in any order, possibly on rows other predicates would reject.

## C7.19.2 CASE-protected evaluation

The contractual short-circuit constructs are `CASE WHEN` (C7.7) and the null-substitution forms — protection belongs *inside* one predicate, not across the `AND`:

```sql
-- SAFE: the guard and the risky expression are one CASE expression;
-- CASE lazily evaluates only the matching branch.
SELECT *
FROM readings
WHERE CASE
        WHEN raw_value ~ '^[0-9]+$' THEN CAST(raw_value AS DOUBLE) > threshold
        ELSE false
      END;

SELECT *
FROM flows
WHERE CASE WHEN denominator <> 0.0 THEN numerator / denominator > 3.5 ELSE false END;

-- equivalently for divide-by-zero: make the expression total with NULLIF
WHERE numerator / NULLIF(denominator, 0.0) > 3.5;
```

The same rule applies to generated `Expr` pipelines (C9): build `when(guard, risky).otherwise(lit(false))`, never `guard.and(risky)`, when `risky` can error on unguarded rows.

## C7.19.3 UDF implications

```text id="filter-order-udf-rules"
A UDF used in WHERE/HAVING/JOIN predicates must not rely on a sibling AND
  predicate for domain protection — it may see the full unfiltered batch.
Make predicate-position UDFs total: return false/NULL (per C7.8 policy) for
  out-of-domain input instead of erroring, or document that callers must wrap
  the call in CASE.
Volatile or short-circuit-sensitive UDFs: ScalarUDFImpl::short_circuits() exists
  for functions whose own arguments are lazily evaluated (CASE-like functions);
  it does NOT protect the UDF from being invoked on rows a textual AND guard
  "should" have removed.
Cost note: 54's reordering means an expensive UDF predicate is typically pushed
  behind cheap column comparisons automatically — declare honest volatility so
  the optimizer can order it correctly.
Testing: the C7.14 matrix gains a case — evaluate each predicate-position
  calculation against the raw unguarded input distribution, not only the
  post-guard subset.
```

---

# DataFusion Advanced — C8) Vectorized Arrow implementation patterns

## C8.0 Objective

Provide a **kernel-level implementation cookbook** for efficient, batch-native DataFusion scalar UDFs:

```text id="c8-root"
ScalarUDFImpl::invoke_with_args(ScalarFunctionArgs)
  ├─ inspect ColumnarValue inputs
  ├─ specialize Scalar/Scalar fast path
  ├─ specialize Scalar/Array and Array/Scalar paths
  ├─ avoid scalar-to-array expansion when cheap to specialize
  ├─ downcast Arrow arrays once
  ├─ use Arrow compute kernels where possible
  ├─ use preallocated Arrow builders for custom logic
  ├─ return ColumnarValue::Scalar or ColumnarValue::Array(ArrayRef)
  └─ validate output type, length, nullability, memory profile
```

The attachment already emphasizes that `ScalarUDFImpl::invoke_with_args` should handle `ColumnarValue::Scalar` for performance and that `ColumnarValue::values_to_arrays` is simpler but slower because it expands constants into arrays.  Current DataFusion docs say the same: implementations should handle constant `ColumnarValue::Scalar` arguments for best performance, while `ColumnarValue::values_to_arrays` is a convenience path that is likely simpler but slower. ([Docs.rs][1])

---

## C8.1 Runtime value model

```text id="runtime-model"
ScalarFunctionArgs
  ├─ args: Vec<ColumnarValue>
  │    ├─ ColumnarValue::Scalar(ScalarValue)
  │    └─ ColumnarValue::Array(ArrayRef)
  ├─ arg_fields: Vec<FieldRef>
  ├─ number_rows: usize
  ├─ return_field: FieldRef
  └─ config_options: Arc<ConfigOptions>

ColumnarValue
  ├─ ScalarValue = one logical constant value
  └─ ArrayRef = Arrow array with one value/null per row
```

`ScalarFunctionArgs` is the argument bundle passed to `ScalarUDFImpl::invoke_with_args`; current docs list evaluated `args: Vec<ColumnarValue>` and other invocation metadata. ([Docs.rs][2]) `ColumnarValue::to_array` repeats scalar constants into arrays, but the DataFusion docs explicitly note this is less efficient than handling scalars directly. ([Docs.rs][3])

### Import baseline

```rust id="imports"
use std::sync::Arc;

use datafusion::arrow::{
    array::{
        Array, ArrayRef,
        BooleanArray, BooleanBuilder,
        Float64Array, Float64Builder,
        Int64Array, Int64Builder,
        StringArray, StringBuilder,
        LargeStringArray, StringViewArray,
        ListArray, ListBuilder,
        StructArray, StructBuilder,
        ArrayBuilder,
    },
    compute,
    datatypes::{DataType, Field, Fields},
};

use datafusion::common::{
    exec_err, internal_err, plan_err,
    DataFusionError, Result, ScalarValue,
};

use datafusion::logical_expr::{
    ColumnarValue, ScalarFunctionArgs, ScalarUDFImpl,
    Signature, Volatility,
};
```

Use `datafusion::arrow` re-exports by default to avoid Arrow type identity mismatches; the attachment repeatedly records this as the safe import rule for DataFusion applications and extension crates. 

---

## C8.2 Implementation architecture

Recommended crate split:

```text id="kernel-split"
calc-arrow-kernels/
  ├─ pure Arrow array kernels
  ├─ concrete array input/output
  ├─ no SessionContext
  ├─ no SQL
  └─ no tenant policy

calc-functions/
  ├─ ScalarUDFImpl wrappers
  ├─ Signature / return type / null policy
  ├─ ColumnarValue dispatch
  ├─ scalar fast path
  └─ calls calc-arrow-kernels

query-engine/
  ├─ register package
  ├─ tenant policy
  └─ SessionContext construction
```

Value case:

```text id="architecture-value"
Pure kernel:
  testable without SQL/DataFusion planner
  reusable in preprocessing and UDFs
  benchmarkable at Arrow-array level

UDF wrapper:
  isolates DataFusion API churn
  owns ColumnarValue/scalar handling
  owns function signature and return contract
```

---

# C8.3 Input dispatch patterns

## C8.3.1 Generic dispatch skeleton

```rust id="generic-dispatch"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.len() != 2 {
        return plan_err!("{} expects 2 arguments, got {}", self.name(), args.args.len());
    }

    match (&args.args[0], &args.args[1]) {
        (ColumnarValue::Scalar(a), ColumnarValue::Scalar(b)) => {
            self.invoke_scalar_scalar(a, b)
        }
        (ColumnarValue::Scalar(a), ColumnarValue::Array(b)) => {
            self.invoke_scalar_array(a, b, args.number_rows)
        }
        (ColumnarValue::Array(a), ColumnarValue::Scalar(b)) => {
            self.invoke_array_scalar(a, b, args.number_rows)
        }
        (ColumnarValue::Array(a), ColumnarValue::Array(b)) => {
            self.invoke_array_array(a, b, args.number_rows)
        }
    }
}
```

### Agent rule

```text id="dispatch-agent-rule"
Start with explicit Scalar/Scalar, Scalar/Array, Array/Scalar, Array/Array paths for hot UDFs.
Use values_to_arrays only for low-volume functions, prototypes, or complex variadic logic where simplicity dominates.
```

---

## C8.3.2 Convenience dispatch with `values_to_arrays`

```rust id="values-to-arrays"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.len() != 1 {
        return plan_err!("{} expects 1 argument", self.name());
    }

    let arrays = ColumnarValue::values_to_arrays(&args.args)?;
    let input = as_f64_array(self.name(), &arrays[0])?;

    let output = normalize_f64_array(input)?;

    Ok(ColumnarValue::Array(Arc::new(output) as ArrayRef))
}
```

Use when:

```text id="values-to-arrays-use"
prototype
rarely called function
complex variadic function
performance not critical
function already dominated by heavy computation
```

Avoid when:

```text id="values-to-arrays-avoid"
hot path
many scalar constants
simple arithmetic
per-row logic cheap
batch sizes large
service latency sensitive
```

---

# C8.4 Downcast helpers

## C8.4.1 Primitive downcast

```rust id="downcast-f64"
fn as_f64_array<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a Float64Array> {
    array
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| {
            DataFusionError::Internal(format!(
                "{fn_name} expected Float64Array after coercion, got {:?}",
                array.data_type()
            ))
        })
}
```

## C8.4.2 Boolean downcast

```rust id="downcast-bool"
fn as_bool_array<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a BooleanArray> {
    array
        .as_any()
        .downcast_ref::<BooleanArray>()
        .ok_or_else(|| {
            DataFusionError::Internal(format!(
                "{fn_name} expected BooleanArray after coercion, got {:?}",
                array.data_type()
            ))
        })
}
```

## C8.4.3 List downcast

```rust id="downcast-list"
fn as_list_array<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a ListArray> {
    array
        .as_any()
        .downcast_ref::<ListArray>()
        .ok_or_else(|| {
            DataFusionError::Internal(format!(
                "{fn_name} expected ListArray after coercion, got {:?}",
                array.data_type()
            ))
        })
}
```

## C8.4.4 Struct downcast

```rust id="downcast-struct"
fn as_struct_array<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a StructArray> {
    array
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| {
            DataFusionError::Internal(format!(
                "{fn_name} expected StructArray after coercion, got {:?}",
                array.data_type()
            ))
        })
}
```

### Agent rule

```text id="downcast-agent-rule"
Downcast mismatch after signature/coercion is usually an implementation invariant failure.
Use internal error for impossible post-coercion mismatch.
Use plan error for unsupported user-declared types before execution.
```

---

# C8.5 Unary input patterns

## C8.5.1 Scalar-only fast path

```rust id="unary-scalar-fast"
fn normalize_api_scalar(value: &ScalarValue) -> Result<ColumnarValue> {
    match value {
        ScalarValue::Float64(Some(v)) => {
            Ok(ColumnarValue::Scalar(ScalarValue::Float64(Some(normalize_api(*v)))))
        }
        ScalarValue::Float64(None) => {
            Ok(ColumnarValue::Scalar(ScalarValue::Float64(None)))
        }
        other => internal_err!("normalize_api expected Float64 scalar, got {other:?}"),
    }
}

fn normalize_api(v: f64) -> f64 {
    v
}
```

## C8.5.2 Array path with primitive builder

```rust id="unary-array-builder"
fn normalize_api_array(input: &Float64Array) -> Result<Float64Array> {
    let mut out = Float64Builder::with_capacity(input.len());

    for i in 0..input.len() {
        if input.is_null(i) {
            out.append_null();
        } else {
            out.append_value(normalize_api(input.value(i)));
        }
    }

    Ok(out.finish())
}
```

## C8.5.3 UDF wrapper

```rust id="unary-wrapper"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.len() != 1 {
        return plan_err!("{} expects 1 argument", self.name());
    }

    match &args.args[0] {
        ColumnarValue::Scalar(s) => normalize_api_scalar(s),
        ColumnarValue::Array(a) => {
            let input = as_f64_array(self.name(), a)?;
            let out = normalize_api_array(input)?;
            Ok(ColumnarValue::Array(Arc::new(out) as ArrayRef))
        }
    }
}
```

### Performance note

```text id="unary-performance"
Unary arithmetic can often be implemented with Arrow compute kernels or unary kernel utilities.
If a built-in Arrow compute kernel exists, prefer it over hand-written row loops.
Use row loops only for domain-specific logic not covered by Arrow kernels.
```

Arrow is a complete native Rust implementation of the Arrow columnar format, and Arrow’s compute module provides reusable kernels; using those kernels avoids writing slow or inconsistent per-row code for operations already implemented in Arrow. ([Docs.rs][4])

---

# C8.6 Binary input patterns

## C8.6.1 Scalar/scalar

```rust id="binary-scalar-scalar"
fn safe_divide_scalar_scalar(n: &ScalarValue, d: &ScalarValue) -> Result<ColumnarValue> {
    match (n, d) {
        (ScalarValue::Float64(Some(n)), ScalarValue::Float64(Some(d))) if *d != 0.0 => {
            Ok(ColumnarValue::Scalar(ScalarValue::Float64(Some(n / d))))
        }
        (ScalarValue::Float64(Some(_)), ScalarValue::Float64(Some(0.0))) => {
            Ok(ColumnarValue::Scalar(ScalarValue::Float64(None)))
        }
        (ScalarValue::Float64(None), _) | (_, ScalarValue::Float64(None)) => {
            Ok(ColumnarValue::Scalar(ScalarValue::Float64(None)))
        }
        other => internal_err!("safe_divide expected Float64 scalars, got {other:?}"),
    }
}
```

## C8.6.2 Array/array

```rust id="binary-array-array"
fn safe_divide_array_array(n: &Float64Array, d: &Float64Array) -> Result<Float64Array> {
    if n.len() != d.len() {
        return internal_err!(
            "safe_divide array length mismatch: numerator={}, denominator={}",
            n.len(),
            d.len()
        );
    }

    let mut out = Float64Builder::with_capacity(n.len());

    for i in 0..n.len() {
        if n.is_null(i) || d.is_null(i) || d.value(i) == 0.0 {
            out.append_null();
        } else {
            out.append_value(n.value(i) / d.value(i));
        }
    }

    Ok(out.finish())
}
```

## C8.6.3 Scalar/array

```rust id="binary-scalar-array"
fn safe_divide_scalar_array(n: &ScalarValue, d: &Float64Array) -> Result<Float64Array> {
    let mut out = Float64Builder::with_capacity(d.len());

    let Some(n) = match n {
        ScalarValue::Float64(v) => *v,
        other => return internal_err!("safe_divide expected Float64 scalar, got {other:?}"),
    } else {
        out.append_nulls(d.len());
        return Ok(out.finish());
    };

    for i in 0..d.len() {
        if d.is_null(i) || d.value(i) == 0.0 {
            out.append_null();
        } else {
            out.append_value(n / d.value(i));
        }
    }

    Ok(out.finish())
}
```

## C8.6.4 Array/scalar

```rust id="binary-array-scalar"
fn safe_divide_array_scalar(n: &Float64Array, d: &ScalarValue) -> Result<Float64Array> {
    let mut out = Float64Builder::with_capacity(n.len());

    let Some(d) = match d {
        ScalarValue::Float64(v) => *v,
        other => return internal_err!("safe_divide expected Float64 scalar, got {other:?}"),
    } else {
        out.append_nulls(n.len());
        return Ok(out.finish());
    };

    if d == 0.0 {
        out.append_nulls(n.len());
        return Ok(out.finish());
    }

    for i in 0..n.len() {
        if n.is_null(i) {
            out.append_null();
        } else {
            out.append_value(n.value(i) / d);
        }
    }

    Ok(out.finish())
}
```

## C8.6.5 UDF wrapper

```rust id="binary-wrapper"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.len() != 2 {
        return plan_err!("{} expects 2 arguments", self.name());
    }

    match (&args.args[0], &args.args[1]) {
        (ColumnarValue::Scalar(n), ColumnarValue::Scalar(d)) => {
            safe_divide_scalar_scalar(n, d)
        }
        (ColumnarValue::Scalar(n), ColumnarValue::Array(d)) => {
            let d = as_f64_array(self.name(), d)?;
            Ok(ColumnarValue::Array(Arc::new(safe_divide_scalar_array(n, d)?) as ArrayRef))
        }
        (ColumnarValue::Array(n), ColumnarValue::Scalar(d)) => {
            let n = as_f64_array(self.name(), n)?;
            Ok(ColumnarValue::Array(Arc::new(safe_divide_array_scalar(n, d)?) as ArrayRef))
        }
        (ColumnarValue::Array(n), ColumnarValue::Array(d)) => {
            let n = as_f64_array(self.name(), n)?;
            let d = as_f64_array(self.name(), d)?;
            Ok(ColumnarValue::Array(Arc::new(safe_divide_array_array(n, d)?) as ArrayRef))
        }
    }
}
```

---

# C8.7 Ternary input patterns

## C8.7.1 Example: `clip(value, min, max)`

### Scalar helper

```rust id="clip-scalar-helper"
fn clip_value(value: f64, min_value: f64, max_value: f64) -> Result<f64> {
    if min_value > max_value {
        return exec_err!(
            "clip invalid bounds: min_value={} > max_value={}",
            min_value,
            max_value
        );
    }

    Ok(value.max(min_value).min(max_value))
}
```

### Simple array path using `values_to_arrays`

```rust id="ternary-values-to-arrays"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.len() != 3 {
        return plan_err!("clip expects 3 arguments");
    }

    // Simpler but can expand scalar min/max constants.
    let arrays = ColumnarValue::values_to_arrays(&args.args)?;

    let value = as_f64_array(self.name(), &arrays[0])?;
    let min_v = as_f64_array(self.name(), &arrays[1])?;
    let max_v = as_f64_array(self.name(), &arrays[2])?;

    let mut out = Float64Builder::with_capacity(value.len());

    for i in 0..value.len() {
        if value.is_null(i) || min_v.is_null(i) || max_v.is_null(i) {
            out.append_null();
        } else {
            out.append_value(clip_value(value.value(i), min_v.value(i), max_v.value(i))?);
        }
    }

    Ok(ColumnarValue::Array(Arc::new(out.finish()) as ArrayRef))
}
```

### Optimized constant-bounds path

```rust id="ternary-constant-bounds"
fn clip_array_const_bounds(
    value: &Float64Array,
    min_value: f64,
    max_value: f64,
) -> Result<Float64Array> {
    if min_value > max_value {
        return exec_err!("clip invalid bounds: min_value > max_value");
    }

    let mut out = Float64Builder::with_capacity(value.len());

    for i in 0..value.len() {
        if value.is_null(i) {
            out.append_null();
        } else {
            out.append_value(value.value(i).max(min_value).min(max_value));
        }
    }

    Ok(out.finish())
}
```

### Agent rule

```text id="ternary-agent"
For ternary functions with common scalar constants, specialize constant paths.
`clip(col, 0.0, 1.0)` should not allocate two repeated constant arrays in hot paths.
```

---

# C8.8 Variadic input patterns

## C8.8.1 Variadic strategy

```text id="variadic-strategy"
Variadic function options:
  1. validate arity range
  2. inspect all ColumnarValue variants
  3. choose scalar-only fast path if all args scalar
  4. choose array path with values_to_arrays if simplicity acceptable
  5. for hot paths, process each arg as scalar-or-array per row
```

## C8.8.2 Example: `coalesce_like`

```rust id="coalesce-like"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if args.args.is_empty() {
        return plan_err!("{} expects at least one argument", self.name());
    }

    // Scalar-only fast path.
    if args.args.iter().all(|v| matches!(v, ColumnarValue::Scalar(_))) {
        for value in &args.args {
            let ColumnarValue::Scalar(s) = value else { unreachable!() };
            if !s.is_null() {
                return Ok(ColumnarValue::Scalar(s.clone()));
            }
        }

        return Ok(ColumnarValue::Scalar(ScalarValue::Null));
    }

    // Simpler mixed path: expand scalars.
    let arrays = ColumnarValue::values_to_arrays(&args.args)?;
    let len = arrays[0].len();

    // Builder must match function return type; Utf8 shown as example.
    let mut out = StringBuilder::with_capacity(len, len * 8);

    for row in 0..len {
        let mut wrote = false;

        for arr in &arrays {
            let arr = as_string_array(self.name(), arr)?;
            if !arr.is_null(row) {
                out.append_value(arr.value(row));
                wrote = true;
                break;
            }
        }

        if !wrote {
            out.append_null();
        }
    }

    Ok(ColumnarValue::Array(Arc::new(out.finish()) as ArrayRef))
}
```

### Agent rules

```text id="variadic-agent"
Variadic functions often justify values_to_arrays for simplicity.
Hot variadic functions should avoid expanding scalar constants repeatedly.
Never use variadic for fixed semantic parameters.
```

---

# C8.9 Scalar-only functions

## C8.9.1 Use case

```text id="scalar-only-use"
zero-arg or scalar-constant function:
  version()
  build_info()
  config_value('key')
  parse_format_spec('literal')
```

## C8.9.2 `ScalarValue` return

```rust id="scalar-only-return"
fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
    if !args.args.is_empty() {
        return plan_err!("{} expects zero arguments", self.name());
    }

    Ok(ColumnarValue::Scalar(ScalarValue::Utf8(Some(
        env!("CARGO_PKG_VERSION").to_string()
    ))))
}
```

### Agent rule

```text id="scalar-only-agent"
Return ColumnarValue::Scalar for constant functions.
Do not create an array of repeated constants unless the API requires array output.
```

---

# C8.10 List input patterns

## C8.10.1 List-of-Float64 vector norm

### Return contract

```text id="list-contract"
vector_norm(values: List<Float64>) -> Float64
null list -> null
empty list -> 0.0 or null per policy
null element -> skip / null / error per policy
```

### Implementation

```rust id="vector-norm-list"
fn vector_norm_list_array(input: &ListArray) -> Result<Float64Array> {
    let mut out = Float64Builder::with_capacity(input.len());

    for row in 0..input.len() {
        if input.is_null(row) {
            out.append_null();
            continue;
        }

        let values = input.value(row);
        let values = values
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataFusionError::Internal(format!(
                "vector_norm expected List<Float64>, got child {:?}",
                values.data_type()
            )))?;

        let mut sum_sq = 0.0;

        for j in 0..values.len() {
            if values.is_null(j) {
                // Policy: null element makes whole vector null.
                out.append_null();
                continue;
            }

            let v = values.value(j);
            sum_sq += v * v;
        }

        out.append_value(sum_sq.sqrt());
    }

    Ok(out.finish())
}
```

### Better null-element loop

The above `continue` only continues the inner loop; use a helper to avoid row-control bugs:

```rust id="vector-norm-helper"
fn norm_one(values: &Float64Array) -> Option<f64> {
    let mut sum_sq = 0.0;

    for j in 0..values.len() {
        if values.is_null(j) {
            return None;
        }

        let v = values.value(j);
        sum_sq += v * v;
    }

    Some(sum_sq.sqrt())
}
```

```rust id="vector-norm-list-fixed"
fn vector_norm_list_array(input: &ListArray) -> Result<Float64Array> {
    let mut out = Float64Builder::with_capacity(input.len());

    for row in 0..input.len() {
        if input.is_null(row) {
            out.append_null();
            continue;
        }

        let values = input.value(row);
        let values = values
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataFusionError::Internal(format!(
                "vector_norm expected List<Float64>, got child {:?}",
                values.data_type()
            )))?;

        match norm_one(values) {
            Some(v) => out.append_value(v),
            None => out.append_null(),
        }
    }

    Ok(out.finish())
}
```

Arrow’s `GenericListBuilder` / list APIs use child values plus parent offsets; the docs state that values are appended to the child builder and `append` must be called to delimit each distinct list value. ([Docs.rs][5])

---

## C8.10.2 List output builder

### Example: split string to list of strings

```rust id="list-output-builder"
fn split_words_array(input: &StringArray) -> Result<ListArray> {
    let values_builder = StringBuilder::new();
    let mut builder = ListBuilder::new(values_builder);

    for row in 0..input.len() {
        if input.is_null(row) {
            builder.append(false);
            continue;
        }

        for part in input.value(row).split_whitespace() {
            builder.values().append_value(part);
        }

        builder.append(true);
    }

    Ok(builder.finish())
}
```

### Agent rules

```text id="list-agent-rules"
Call builder.values().append_* for child elements.
Call builder.append(true/false) exactly once per parent row.
Null list and empty list are different.
Preallocate child capacity if expected list lengths are known.
```

---

# C8.11 Struct input/output patterns

## C8.11.1 Struct input

```rust id="struct-input"
fn quality_score_from_struct(input: &StructArray) -> Result<Float64Array> {
    let api = input
        .column_by_name("api")
        .ok_or_else(|| DataFusionError::Execution(
            "quality_score expected struct field `api`".to_string()
        ))?;

    let sulfur = input
        .column_by_name("sulfur")
        .ok_or_else(|| DataFusionError::Execution(
            "quality_score expected struct field `sulfur`".to_string()
        ))?;

    let api = api
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataFusionError::Internal("api field must be Float64".to_string()))?;

    let sulfur = sulfur
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataFusionError::Internal("sulfur field must be Float64".to_string()))?;

    let mut out = Float64Builder::with_capacity(input.len());

    for row in 0..input.len() {
        if input.is_null(row) || api.is_null(row) || sulfur.is_null(row) {
            out.append_null();
        } else {
            out.append_value(score(api.value(row), sulfur.value(row)));
        }
    }

    Ok(out.finish())
}

fn score(api: f64, sulfur: f64) -> f64 {
    api - sulfur
}
```

## C8.11.2 Struct output

```rust id="struct-output"
fn audited_divide_struct(n: &Float64Array, d: &Float64Array) -> Result<StructArray> {
    if n.len() != d.len() {
        return internal_err!("audited_divide length mismatch");
    }

    let fields = Fields::from(vec![
        Arc::new(Field::new("value", DataType::Float64, true)),
        Arc::new(Field::new("status", DataType::Utf8, false)),
        Arc::new(Field::new("reason", DataType::Utf8, true)),
    ]);

    let builders: Vec<Box<dyn ArrayBuilder>> = vec![
        Box::new(Float64Builder::with_capacity(n.len())),
        Box::new(StringBuilder::new()),
        Box::new(StringBuilder::new()),
    ];

    let mut b = StructBuilder::new(fields, builders);

    for row in 0..n.len() {
        let value_builder = b
            .field_builder::<Float64Builder>(0)
            .expect("value builder");
        let status_builder = b
            .field_builder::<StringBuilder>(1)
            .expect("status builder");
        let reason_builder = b
            .field_builder::<StringBuilder>(2)
            .expect("reason builder");

        if n.is_null(row) || d.is_null(row) {
            value_builder.append_null();
            status_builder.append_value("null_input");
            reason_builder.append_value("input was null");
        } else if d.value(row) == 0.0 {
            value_builder.append_null();
            status_builder.append_value("division_by_zero");
            reason_builder.append_value("denominator was zero");
        } else {
            value_builder.append_value(n.value(row) / d.value(row));
            status_builder.append_value("ok");
            reason_builder.append_null();
        }

        b.append(true);
    }

    Ok(b.finish())
}
```

`StructBuilder` supports nested/complex layouts but Arrow’s docs note that constructing deeply nested arrays can be complex because builders are recursive and strongly typed. ([Docs.rs][6])

### Agent rules

```text id="struct-agent"
Use Struct output for typed diagnostics, not JSON strings.
Append every child field once per parent row before b.append(true/false).
Parent struct nullability and child nullability are separate.
Prefer column_by_name over field-position assumptions for struct input.
```

---

# C8.12 String dispatch patterns

## C8.12.1 Problem

```text id="string-problem"
DataFusion logical string input may physically arrive as:
  Utf8 / StringArray
  LargeUtf8 / LargeStringArray
  Utf8View / StringViewArray
  Dictionary(Int*, Utf8)
```

The attachment highlights `Utf8View` as an important DataFusion SQL type-mapping detail and warns that UDF ecosystems built around `StringArray` must either adapt or force compatible mapping.  Arrow exposes `StringViewArray` as a distinct Arrow string-view representation. ([Docs.rs][7])

## C8.12.2 Exact `Utf8` only

```rust id="string-utf8-only"
fn as_string_array<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a StringArray> {
    array
        .as_any()
        .downcast_ref::<StringArray>()
        .ok_or_else(|| DataFusionError::Internal(format!(
            "{fn_name} expected Utf8/StringArray after coercion, got {:?}",
            array.data_type()
        )))
}
```

Use when signature/coercion guarantees `DataType::Utf8`.

## C8.12.3 Multi-string dispatch

```rust id="string-dispatch"
fn lower_string_array(fn_name: &str, array: &ArrayRef) -> Result<ArrayRef> {
    match array.data_type() {
        DataType::Utf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_else(|| DataFusionError::Internal("Utf8 downcast failed".to_string()))?;

            let mut out = StringBuilder::with_capacity(arr.len(), arr.len() * 8);

            for i in 0..arr.len() {
                if arr.is_null(i) {
                    out.append_null();
                } else {
                    out.append_value(arr.value(i).to_ascii_lowercase());
                }
            }

            Ok(Arc::new(out.finish()) as ArrayRef)
        }
        DataType::LargeUtf8 => {
            let arr = array
                .as_any()
                .downcast_ref::<LargeStringArray>()
                .ok_or_else(|| DataFusionError::Internal("LargeUtf8 downcast failed".to_string()))?;

            // Could build LargeStringBuilder if preserving LargeUtf8 is required;
            // or cast/canonicalize to Utf8 if documented.
            let mut out = StringBuilder::with_capacity(arr.len(), arr.len() * 8);

            for i in 0..arr.len() {
                if arr.is_null(i) {
                    out.append_null();
                } else {
                    out.append_value(arr.value(i).to_ascii_lowercase());
                }
            }

            Ok(Arc::new(out.finish()) as ArrayRef)
        }
        DataType::Utf8View => {
            let arr = array
                .as_any()
                .downcast_ref::<StringViewArray>()
                .ok_or_else(|| DataFusionError::Internal("Utf8View downcast failed".to_string()))?;

            let mut out = StringBuilder::with_capacity(arr.len(), arr.len() * 8);

            for i in 0..arr.len() {
                if arr.is_null(i) {
                    out.append_null();
                } else {
                    out.append_value(arr.value(i).to_ascii_lowercase());
                }
            }

            Ok(Arc::new(out.finish()) as ArrayRef)
        }
        other => exec_err!("{fn_name} expected string input, got {other:?}"),
    }
}
```

## C8.12.4 Cast-to-Utf8 fallback

```rust id="string-cast-fallback"
fn canonicalize_to_utf8(array: &ArrayRef) -> Result<ArrayRef> {
    if matches!(array.data_type(), DataType::Utf8) {
        return Ok(array.clone());
    }

    compute::cast(array.as_ref(), &DataType::Utf8)
        .map_err(DataFusionError::from)
}
```

### Agent rules

```text id="string-agent-rules"
Choose one:
  exact Utf8 only
  multi-string dispatch
  cast/canonicalize to Utf8

Document:
  output type
  allocation/copy behavior
  dictionary handling
  Utf8View compatibility
```

---

# C8.13 Dictionary input support

## C8.13.1 Problem

Arrow dictionary arrays represent repeated values by integer keys plus a values array; docs describe `DictionaryArray` as mostly used for strings or limited primitive sets represented as integers. ([Docs.rs][8])

## C8.13.2 Simple fallback: cast dictionary to Utf8

```rust id="dictionary-cast"
fn string_like_to_utf8(array: &ArrayRef) -> Result<ArrayRef> {
    match array.data_type() {
        DataType::Utf8 => Ok(array.clone()),
        DataType::Dictionary(_, value_type) if value_type.as_ref() == &DataType::Utf8 => {
            compute::cast(array.as_ref(), &DataType::Utf8).map_err(DataFusionError::from)
        }
        other => exec_err!("expected Utf8 or Dictionary(_, Utf8), got {other:?}"),
    }
}
```

## C8.13.3 Performance policy

```text id="dictionary-performance-policy"
Low volume:
  cast dictionary to Utf8 for simplicity.

Hot path:
  implement dictionary-aware kernel:
    operate once on dictionary values
    remap keys or preserve dictionary
    avoid repeated string decode per row.

Output:
  preserve dictionary only if downstream expects dictionary.
  otherwise return Utf8 and document allocation.
```

### Agent rules

```text id="dictionary-agent-rules"
If signature claims string-like support, include dictionary tests.
If dictionary unsupported, fail planning or execution with clear cast suggestion.
Do not accidentally cast large dictionary arrays in hot paths without benchmark.
```

---

# C8.14 Output builder cookbook

## C8.14.1 Primitive builders

```rust id="primitive-builder"
let mut out = Float64Builder::with_capacity(input.len());

for i in 0..input.len() {
    if input.is_null(i) {
        out.append_null();
    } else {
        out.append_value(input.value(i) * 2.0);
    }
}

let array: ArrayRef = Arc::new(out.finish());
```

Primitive builders allow preallocating with capacity; Arrow docs describe primitive builders as generating arrays with the primitive type’s `DATA_TYPE` by default. ([Docs.rs][9])

## C8.14.2 Boolean builders

```rust id="boolean-builder"
let mut out = BooleanBuilder::with_capacity(input.len());

for i in 0..input.len() {
    if input.is_null(i) {
        out.append_null();
    } else {
        out.append_value(input.value(i) > 0.0);
    }
}

let array: ArrayRef = Arc::new(out.finish());
```

## C8.14.3 String builders

```rust id="string-builder"
let mut out = StringBuilder::with_capacity(input.len(), input.len() * 16);

for i in 0..input.len() {
    if input.is_null(i) {
        out.append_null();
    } else {
        out.append_value(format!("stream_{}", input.value(i)));
    }
}

let array: ArrayRef = Arc::new(out.finish());
```

`StringBuilder` exposes capacity-oriented constructors for preallocating item and byte-buffer capacity. ([Docs.rs][10])

DataFusion 54 note: the engine's internal string builders under `datafusion_functions::strings` (the specialized concat builders used by `concat`/`concat_ws`) became crate-private in 54 — do not import them from `datafusion_functions`. Build string outputs with the Arrow builders directly (`StringBuilder`, `LargeStringBuilder`, `StringViewBuilder`) as shown above.

## C8.14.4 List builders

```rust id="list-builder"
let values = Float64Builder::new();
let mut out = ListBuilder::new(values);

for row in 0..input.len() {
    if input.is_null(row) {
        out.append(false); // null list
    } else {
        out.values().append_value(input.value(row));
        out.values().append_value(input.value(row) * 2.0);
        out.append(true); // non-null list with 2 elements
    }
}

let array: ArrayRef = Arc::new(out.finish());
```

## C8.14.5 Struct builders

```rust id="struct-builder-cookbook"
let fields = Fields::from(vec![
    Arc::new(Field::new("is_valid", DataType::Boolean, false)),
    Arc::new(Field::new("score", DataType::Float64, true)),
    Arc::new(Field::new("reason", DataType::Utf8, true)),
]);

let builders: Vec<Box<dyn ArrayBuilder>> = vec![
    Box::new(BooleanBuilder::with_capacity(n)),
    Box::new(Float64Builder::with_capacity(n)),
    Box::new(StringBuilder::new()),
];

let mut out = StructBuilder::new(fields, builders);

for row in 0..n {
    out.field_builder::<BooleanBuilder>(0).unwrap().append_value(true);
    out.field_builder::<Float64Builder>(1).unwrap().append_value(1.23);
    out.field_builder::<StringBuilder>(2).unwrap().append_null();
    out.append(true);
}

let array: ArrayRef = Arc::new(out.finish());
```

### Builder validation rules

```text id="builder-validation"
Primitive/String/Boolean:
  append exactly one value/null per output row.

List:
  append zero or more child values.
  call parent append exactly once per output row.

Struct:
  append every child field exactly once per output row.
  call parent append exactly once per output row.

Map:
  append key/value pairs.
  enforce non-null keys and duplicate-key policy.
```

---

# C8.15 Arrow compute kernel usage

## C8.15.1 Prefer compute kernels

```rust id="compute-kernel-cast"
fn cast_to_f64(array: &ArrayRef) -> Result<ArrayRef> {
    compute::cast(array.as_ref(), &DataType::Float64)
        .map_err(DataFusionError::from)
}
```

## C8.15.2 Kernel-first decision table

| Operation                  | Prefer                                                      |
| -------------------------- | ----------------------------------------------------------- |
| cast                       | Arrow `compute::cast`                                       |
| arithmetic over primitives | Arrow arithmetic kernels if available                       |
| boolean combine/filter     | Arrow boolean/filter kernels                                |
| string simple transforms   | built-ins or Arrow/DataFusion function package if available |
| null fill/coalesce         | built-ins/Arrow kernels if available                        |
| sort/take/filter           | Arrow/DataFusion physical kernels                           |
| custom domain formula      | custom builder/loop                                         |
| nested diagnostic struct   | custom builders                                             |

### Agent rules

```text id="compute-agent"
Do not hand-write kernels for operations Arrow/DataFusion already implements.
Use compute kernels for correctness and vectorization.
Use row loops for domain-specific branching, custom null policy, or unsupported nested logic.
Benchmark both for hot paths.
```

---

# C8.16 Dispatch by `DataType`

## C8.16.1 Numeric dispatch

```rust id="numeric-dispatch"
fn abs_numeric(fn_name: &str, array: &ArrayRef) -> Result<ArrayRef> {
    match array.data_type() {
        DataType::Int64 => {
            let arr = array.as_any().downcast_ref::<Int64Array>()
                .ok_or_else(|| DataFusionError::Internal("Int64 downcast failed".to_string()))?;

            let mut out = Int64Builder::with_capacity(arr.len());

            for i in 0..arr.len() {
                if arr.is_null(i) {
                    out.append_null();
                } else {
                    out.append_value(arr.value(i).abs());
                }
            }

            Ok(Arc::new(out.finish()) as ArrayRef)
        }
        DataType::Float64 => {
            let arr = as_f64_array(fn_name, array)?;
            let mut out = Float64Builder::with_capacity(arr.len());

            for i in 0..arr.len() {
                if arr.is_null(i) {
                    out.append_null();
                } else {
                    out.append_value(arr.value(i).abs());
                }
            }

            Ok(Arc::new(out.finish()) as ArrayRef)
        }
        other => exec_err!("{fn_name} expected numeric input, got {other:?}"),
    }
}
```

## C8.16.2 Canonicalize-then-dispatch

```rust id="canonicalize-dispatch"
fn numeric_to_f64_array(array: &ArrayRef) -> Result<ArrayRef> {
    match array.data_type() {
        DataType::Float64 => Ok(array.clone()),
        DataType::Float32
        | DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => {
            compute::cast(array.as_ref(), &DataType::Float64).map_err(DataFusionError::from)
        }
        other => exec_err!("expected numeric input, got {other:?}"),
    }
}
```

### Tradeoff

```text id="canonicalize-tradeoff"
Direct dispatch:
  faster, more code, preserves exact types.

Cast/canonicalize:
  simpler, may allocate/copy, may lose exact decimal semantics.

Use manifest/signature to make this explicit.
```

---

# C8.17 Validation helpers

## C8.17.1 Output type

```rust id="validate-type"
fn validate_output_type(fn_name: &str, expected: &DataType, value: &ColumnarValue) -> Result<()> {
    let actual = match value {
        ColumnarValue::Scalar(s) => s.data_type(),
        ColumnarValue::Array(a) => a.data_type().clone(),
    };

    if &actual != expected {
        return internal_err!(
            "{fn_name} returned type {:?}, expected {:?}",
            actual,
            expected
        );
    }

    Ok(())
}
```

## C8.17.2 Output length

```rust id="validate-length"
fn validate_output_len(fn_name: &str, expected_rows: usize, value: &ColumnarValue) -> Result<()> {
    if let ColumnarValue::Array(a) = value {
        if a.len() != expected_rows {
            return internal_err!(
                "{fn_name} returned {} rows, expected {expected_rows}",
                a.len()
            );
        }
    }

    Ok(())
}
```

## C8.17.3 Nullability expectation

```rust id="validate-nullability"
fn validate_non_nullable_output(fn_name: &str, array: &dyn Array) -> Result<()> {
    if array.null_count() != 0 {
        return internal_err!(
            "{fn_name} produced {} nulls for non-nullable output",
            array.null_count()
        );
    }

    Ok(())
}
```

## C8.17.4 Memory footprint

```rust id="memory-footprint"
fn approximate_array_memory_bytes(array: &dyn Array) -> usize {
    array.get_array_memory_size()
}
```

### Agent rules

```text id="validation-agent"
Validate in debug/test builds at minimum.
For production hot path, validate output invariants in tests and optionally behind debug assertions.
Track memory for variable-length string/list/struct outputs.
```

---

# C8.18 Full example: production scalar UDF wrapper

```rust id="production-udf"
#[derive(Debug)]
pub struct SafeDivide {
    signature: Signature,
}

impl SafeDivide {
    pub fn new() -> Self {
        Self {
            signature: Signature::exact(
                vec![DataType::Float64, DataType::Float64],
                Volatility::Immutable,
            ),
        }
    }
}

impl ScalarUDFImpl for SafeDivide {
    fn name(&self) -> &str {
        "safe_divide"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        if arg_types == [DataType::Float64, DataType::Float64] {
            Ok(DataType::Float64)
        } else {
            internal_err!("safe_divide unexpected coerced arg_types: {arg_types:?}")
        }
    }

    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        if args.args.len() != 2 {
            return plan_err!("safe_divide expects 2 arguments");
        }

        let output = match (&args.args[0], &args.args[1]) {
            (ColumnarValue::Scalar(n), ColumnarValue::Scalar(d)) => {
                safe_divide_scalar_scalar(n, d)?
            }
            (ColumnarValue::Scalar(n), ColumnarValue::Array(d)) => {
                let d = as_f64_array(self.name(), d)?;
                ColumnarValue::Array(Arc::new(safe_divide_scalar_array(n, d)?) as ArrayRef)
            }
            (ColumnarValue::Array(n), ColumnarValue::Scalar(d)) => {
                let n = as_f64_array(self.name(), n)?;
                ColumnarValue::Array(Arc::new(safe_divide_array_scalar(n, d)?) as ArrayRef)
            }
            (ColumnarValue::Array(n), ColumnarValue::Array(d)) => {
                let n = as_f64_array(self.name(), n)?;
                let d = as_f64_array(self.name(), d)?;
                ColumnarValue::Array(Arc::new(safe_divide_array_array(n, d)?) as ArrayRef)
            }
        };

        validate_output_type(self.name(), &DataType::Float64, &output)?;
        validate_output_len(self.name(), args.number_rows, &output)?;

        Ok(output)
    }
}
```

---

# C8.19 Testing matrix

## C8.19.1 Input-shape tests

```text id="input-shape-tests"
[ ] scalar/scalar
[ ] scalar/array
[ ] array/scalar
[ ] array/array
[ ] zero-row array
[ ] all-null array
[ ] mixed-null array
[ ] wrong physical type after forced misuse
[ ] list input with null list
[ ] list input with empty list
[ ] list input with null element
[ ] struct input with null parent
[ ] struct input with null child
```

## C8.19.2 Output-builder tests

```text id="builder-tests"
[ ] primitive output length
[ ] boolean output length
[ ] string output length and nulls
[ ] list parent offsets and child length
[ ] list null vs empty distinction
[ ] struct child lengths equal parent length
[ ] struct parent null behavior
[ ] dictionary output key/value types
```

## C8.19.3 Dispatch tests

```text id="dispatch-tests"
[ ] DataType::Float64
[ ] DataType::Int64 if supported
[ ] DataType::Utf8
[ ] DataType::Utf8View if supported
[ ] DataType::LargeUtf8 if supported
[ ] DataType::Dictionary(_, Utf8) if supported
[ ] unsupported DataType returns clean error
```

## C8.19.4 Performance tests

```text id="performance-tests"
[ ] scalar fast path does not allocate array
[ ] scalar/array path faster than values_to_arrays for constants
[ ] array/array path benchmarked
[ ] Arrow compute kernel compared to custom loop
[ ] builder capacity avoids excessive reallocations
[ ] string/list outputs measured for memory footprint
```

---

# C8.20 Deployment guidance

## C8.20.1 Public SQL service

```text id="public-service"
Require:
  scalar fast paths for hot functions
  no panics
  output invariant tests
  function allowlist
  memory/timeout/query limits
  benchmarked string/list/struct outputs

Avoid:
  broad `any` dispatch
  unchecked dictionary/string-view casts
  unbounded list/string builders
```

## C8.20.2 Internal batch/ETL

```text id="internal-batch"
Allow:
  values_to_arrays for non-hot functions
  richer diagnostic struct outputs
  audited variants
  broader input dispatch

Require:
  rejected-row tests
  memory footprint measurement
  Arrow/Parquet round-trip if materialized
```

## C8.20.3 Engineering/modeling workbench

```text id="engineering-workbench"
Prefer:
  Arrow kernels for common math
  exact decimal/timestamp dispatch where needed
  typed struct/list outputs for diagnostics
  explicit unit metadata via return_field_from_args

Avoid:
  JSON string diagnostics
  Float64 canonicalization for exact financial/quality values unless explicitly approximate
```

---

# C8.21 Performance rulebook

```text id="perf-rulebook"
1. Prefer built-in DataFusion/Arrow compute kernels over custom loops.
2. Use Expr/built-ins instead of UDF when logic is composable.
3. Specialize ColumnarValue::Scalar.
4. Avoid ColumnarValue::values_to_arrays in hot simple functions.
5. Downcast once per input array, outside row loops.
6. Preallocate primitive/string/list builders.
7. Avoid format! in row loops when simple append is possible.
8. Avoid cloning ArrayRef buffers unless necessary.
9. Avoid cast/canonicalize in hot paths unless benchmarked.
10. Support dictionary/string-view only when tested.
11. Separate kernel implementation from UDF wrapper.
12. Measure null-heavy, scalar-heavy, and large-batch workloads.
```

---

# C8.22 Anti-pattern inventory

* Always calling `ColumnarValue::values_to_arrays` in hot scalar UDF.
* Expanding scalar constants into full arrays for simple arithmetic.
* Downcasting inside every row loop iteration.
* `unwrap()` on downcasts.
* Calling `.value(i)` without null check.
* Returning array with wrong length.
* Returning `Float64Array` when `return_type` says `Decimal128`.
* Ignoring `Utf8View` while using broad string signature.
* Casting dictionary strings to Utf8 in hot path without benchmark.
* Treating empty list and null list as equivalent.
* Forgetting `builder.append(true/false)` for every list/struct parent row.
* Child builder lengths not matching struct parent length.
* Using JSON strings for typed diagnostics.
* Not preallocating builders for large outputs.
* Using `format!` per row for simple string transforms.
* No zero-row/all-null tests.
* No scalar/scalar fast-path tests.
* No memory footprint tests for string/list/struct outputs.

---

# C8.23 Agent checklist

```text id="c8-final-checklist"
[ ] Use datafusion::arrow re-exports.
[ ] Keep Arrow kernel logic separate from ScalarUDFImpl wrapper.
[ ] Match on ColumnarValue::Scalar vs ColumnarValue::Array.
[ ] Implement scalar/scalar fast path.
[ ] Implement scalar/array and array/scalar fast paths for binary hot functions.
[ ] Use values_to_arrays only when simplicity is acceptable.
[ ] Downcast arrays once outside row loops.
[ ] Use Arrow compute kernels when available.
[ ] Preallocate primitive builders with input length.
[ ] Preallocate string builders with item and byte capacity estimate.
[ ] For ListBuilder, call values().append_* for children and append once per parent row.
[ ] For StructBuilder, append every child and append parent once per row.
[ ] Dispatch on DataType for polymorphic functions.
[ ] Decide exact Utf8 vs Utf8View/LargeUtf8/dictionary support.
[ ] Use compute::cast for canonicalization only when documented and benchmarked.
[ ] Validate output type equals return_field.data_type or return_type.
[ ] Validate output length equals ScalarFunctionArgs.number_rows.
[ ] Validate null count when output is non-nullable.
[ ] Estimate memory footprint for variable-length outputs.
[ ] Test scalar, array, mixed scalar/array, zero-row, all-null, mixed-null.
[ ] Test every supported physical DataType.
[ ] Benchmark hot functions with representative batch sizes.
```

[1]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html?utm_source=chatgpt.com "ScalarUDFImpl in datafusion_expr - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.ScalarFunctionArgs.html?utm_source=chatgpt.com "ScalarFunctionArgs in datafusion::logical_expr - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/logical_expr/enum.ColumnarValue.html?utm_source=chatgpt.com "ColumnarValue in datafusion::logical_expr - Rust"
[4]: https://docs.rs/arrow/latest?utm_source=chatgpt.com "arrow - Rust"
[5]: https://docs.rs/arrow/latest/arrow/array/builder/struct.GenericListBuilder.html?utm_source=chatgpt.com "GenericListBuilder in arrow::array::builder - Rust"
[6]: https://docs.rs/arrow/latest/arrow/array/struct.StructBuilder.html?utm_source=chatgpt.com "StructBuilder in arrow::array - Rust"
[7]: https://docs.rs/arrow/latest/arrow/array/type.StringViewArray.html?utm_source=chatgpt.com "StringViewArray in arrow::array - Rust"
[8]: https://docs.rs/arrow/latest/arrow/array/struct.DictionaryArray.html?utm_source=chatgpt.com "DictionaryArray in arrow::array - Rust"
[9]: https://docs.rs/arrow/latest/arrow/array/builder/type.Int32Builder.html?utm_source=chatgpt.com "Int32Builder in arrow::array::builder - Rust"
[10]: https://docs.rs/arrow/latest/arrow/array/type.StringBuilder.html?utm_source=chatgpt.com "StringBuilder in arrow::array - Rust"


# DataFusion Advanced — C9) Complex conditionality and expression composition

## C9.0 Objective

Make conditional user-defined calculations **transparent, optimizer-visible, testable, and composable** before escalating to scalar UDFs:

```text id="c9-root"
Structured calculation intent
  ├─ primitive expressions: col / lit / cast / alias
  ├─ boolean predicates: eq / gt / lt / and / or / not / is_null
  ├─ conditional expressions: CASE / coalesce / nullif
  ├─ built-in scalar functions
  ├─ conditional aggregates: aggregate FILTER / CASE-in-aggregate
  ├─ window metrics: built-in window + CASE / QUALIFY / post-window Expr
  ├─ nested field extraction: get_field / bracket syntax / map access
  ├─ rule-table joins
  └─ scalar UDF only when expression composition is insufficient
```

The attachment already positions `Expr` as DataFusion’s shared language across DataFrame methods, SQL planning, optimizer rules, and UDF calls; it also documents `case(...).when(...).otherwise(...)`, `coalesce`, `nullif`, null checks, scalar functions, aggregate expressions, aliases, and expression validation against `DFSchema`.   DataFusion’s current expression docs likewise describe conditional expressions including `coalesce`, `case(...).when(...).end()`, and `case(...).when(...).otherwise(...)`; `coalesce` returns the first non-null argument and returns null only if all arguments are null. ([Apache DataFusion][1])

---

## C9.1 Core principle: expression-first, UDF-second

```text id="expression-first"
Use Expr / SQL / built-ins when:
  calculation can be represented as an expression tree
  conditional branches are visible from columns/literals
  optimizer should see predicates/casts/null logic
  result is scalar-per-row or aggregate/window composition
  type/nullability inference should remain DataFusion-native

Use scalar UDF when:
  logic is reused widely and too complex for readable Expr
  behavior requires custom Arrow kernel/runtime dispatch
  type-specific behavior cannot be expressed in SQL/Expr
  domain algorithm is nontrivial but still scalar-per-row
  optimizer visibility loss is acceptable or UDF hooks are implemented
```

A scalar UDF is row-wise and vectorized over Arrow arrays; use it for custom transforms that cannot be represented cleanly as existing expressions, not as a default wrapper for every business rule. DataFusion’s UDF docs define scalar UDFs as row-to-value functions, and DataFusion’s `ScalarUDFImpl::simplify` hook exists specifically because opaque UDFs otherwise need explicit per-function simplification logic if they are to participate in expression optimization. ([Apache DataFusion][2])

---

## C9.2 Placement matrix: `CASE` / `Expr` / built-in / UDF

| Calculation shape                    | Prefer                                  | Why                                                              |
| ------------------------------------ | --------------------------------------- | ---------------------------------------------------------------- |
| simple null default                  | `coalesce`                              | optimizer-visible, standard SQL                                  |
| sentinel-to-null                     | `nullif`                                | concise, pushdown-friendly when embedded in predicate/projection |
| safe divide                          | `Expr` with `nullif` or `CASE`          | no UDF needed                                                    |
| bucket by thresholds                 | `CASE` / generated `Expr`               | branch logic remains visible                                     |
| piecewise arithmetic                 | `CASE` with arithmetic branches         | transparent branch formula                                       |
| conditional aggregate                | aggregate `FILTER` or `SUM(CASE...)`    | group logic stays in aggregate plan                              |
| conditional window metric            | built-in window + `CASE` / `QUALIFY`    | preserves window semantics                                       |
| nested field fallback                | bracket/get_field + `coalesce` / `CASE` | avoids JSON/string UDF                                           |
| many business rules                  | rule table join                         | data-governed rules, not code explosion                          |
| type-specialized numerical algorithm | scalar UDF                              | custom Arrow kernel needed                                       |
| external reference lookup            | async UDF or preprocessing              | cannot be pure `Expr`                                            |
| custom grouped state                 | UDAF                                    | not scalar expression                                            |
| custom frame-aware state             | UDWF                                    | not scalar expression                                            |
| custom table generation              | UDTF/TableProvider                      | relation-valued                                                  |

Agent invariant:

```text id="placement-invariant"
If a calculation can be expressed as `Expr`, do not create a scalar UDF merely to centralize code.
Centralize as a Rust function returning `Expr`.
```

---

# C9.3 Expression composition foundations

## C9.3.1 Imports

```rust id="expr-imports"
use datafusion::prelude::*;
use datafusion::logical_expr::{Expr, ExprSchemable};
use datafusion::functions_aggregate::expr_fn::{sum, count, avg, min, max};
use datafusion::functions::string::expr_fn::{lower, trim};
```

Use `Expr` helpers for generated Rust plans. Use SQL only when user-authored SQL or SQL dialect compatibility is the product contract. The attachment explicitly recommends `Expr` for trusted programmatic filters because it avoids SQL string assembly, while SQL is preferable for user-authored query text. 

---

## C9.3.2 Boolean expression discipline

```rust id="bool-expr"
let predicate = col("amount")
    .is_not_null()
    .and(col("amount").gt(lit(0.0)))
    .and(
        col("status").in_list(
            vec![lit("paid"), lit("settled")],
            false,
        )
    );
```

Do not use Rust `&&`, `||`, `<`, `>`, or `==` operators for expression construction; DataFusion’s expression docs and the attachment note that fluent methods such as `.gt(...)`, `.lt(...)`, `.and(...)`, and `.or(...)` are required for comparison/logical expressions.  ([Apache DataFusion][1])

---

## C9.3.3 Type/nullability validation

```rust id="expr-validation"
use datafusion::logical_expr::ExprSchemable;

pub fn validate_generated_expr(
    expr: &Expr,
    schema: &datafusion::common::DFSchema,
) -> datafusion::error::Result<()> {
    let dtype = expr.get_type(schema)?;
    let nullable = expr.nullable(schema)?;

    tracing::debug!(?dtype, nullable, "validated generated expression");
    Ok(())
}
```

The attachment states that `ExprSchemable::get_type` returns an Arrow `DataType`, `nullable` returns expression nullability, and both can fail when referenced columns do not exist or expression types are incorrect; treat these failures as compile-time-like validation failures for generated plans. 

---

# C9.4 `CASE WHEN` vs scalar UDF

## C9.4.1 Use `CASE` when logic is declarative

```text id="case-use"
Use CASE when:
  branches are predicate-driven
  branch formulas use existing Expr/built-ins
  branch count is modest
  SQL readability matters
  optimizer should inspect predicates
  output type can be coerced across branches
```

Rust:

```rust id="case-rust"
let amount_bucket = case(col("amount").gt_eq(lit(1_000_000.0)))
    .when(lit(true), lit("large"))
    .otherwise(lit("standard"))?
    .alias("amount_bucket");
```

SQL:

```sql id="case-sql"
SELECT
  CASE
    WHEN amount >= 1000000 THEN 'large'
    ELSE 'standard'
  END AS amount_bucket
FROM orders;
```

The attachment includes equivalent `case(...).when(...).otherwise(...)` DataFusion expression examples and states that CASE outputs should be aliased and arms must coerce to compatible types. 

---

## C9.4.2 Use scalar UDF when logic is algorithmic

```text id="udf-use"
Use scalar UDF when:
  branch logic is too complex/noisy for readable Expr
  branch logic requires custom array kernel
  behavior depends on physical Arrow type dispatch
  function must expose custom signature/coercion/return metadata
  algorithm is reused across many plans and not naturally decomposable
```

Example UDF candidate:

```text id="udf-candidate"
calculate_flash_phase(
  temperature,
  pressure,
  composition_vector,
  eos_model_id
) -> Struct<phase, vapor_fraction, status>
```

Do not implement this as a 400-line generated `CASE`; use a scalar UDF or external preprocessing depending on cost and statefulness.

---

## C9.4.3 Use Rust `Expr` helper instead of UDF for reusable logic

```rust id="expr-helper"
pub fn safe_ratio_expr(
    numerator: Expr,
    denominator: Expr,
    alias: &'static str,
) -> Expr {
    (numerator / nullif(denominator, lit(0.0))).alias(alias)
}

pub fn margin_pct_expr(revenue: Expr, cost: Expr) -> Expr {
    safe_ratio_expr(
        revenue.clone() - cost,
        revenue,
        "margin_pct",
    )
}
```

Value case:

```text id="expr-helper-value"
centralized logic
no runtime UDF registration
optimizer-visible arithmetic
no Arrow kernel maintenance
no signature/volatility surface
no function allowlist burden
```

---

# C9.5 Pattern: conditional defaulting

## C9.5.1 Null defaulting with `coalesce`

SQL:

```sql id="coalesce-sql"
SELECT
  coalesce(display_name, username, email, 'unknown') AS display_name_safe
FROM users;
```

Rust:

```rust id="coalesce-rust"
let display_name_safe = coalesce(vec![
    col("display_name"),
    col("username"),
    col("email"),
    lit("unknown"),
]).alias("display_name_safe");
```

Use when:

```text id="coalesce-use"
input nulls are acceptable
first non-null value wins
default is not a data-quality error
```

Avoid when:

```text id="coalesce-avoid"
missing value must be audited
default changes metric semantics
null source column identity matters
```

---

## C9.5.2 Sentinel defaulting with `nullif` + `coalesce`

```rust id="nullif-coalesce"
let normalized_status = coalesce(vec![
    nullif(col("raw_status"), lit("")),
    nullif(col("raw_status"), lit("UNKNOWN")),
    lit("unclassified"),
]).alias("status_norm");
```

Better:

```rust id="normalize-status-case"
let status_norm = case(col("raw_status").is_null())
    .when(lit(true), lit("unclassified"))
    .when(col("raw_status").eq(lit("")), lit("unclassified"))
    .when(col("raw_status").eq(lit("UNKNOWN")), lit("unclassified"))
    .otherwise(lower(trim(vec![col("raw_status")])))?
    .alias("status_norm");
```

Agent rule:

```text id="defaulting-agent"
Use `coalesce` for true null fallback.
Use `nullif` to convert sentinel values to null.
Use `CASE` when fallback reason differs by predicate or when sentinel logic is multi-branch.
```

---

## C9.5.3 Defaulting manifest pattern

```yaml id="defaulting-manifest"
calculation:
  name: status_norm
  placement: Expr
  pattern: conditional_defaulting
  inputs:
    - raw_status
  null_policy:
    null_input: "unclassified"
    empty_string: "unclassified"
    unknown_literal: "unclassified"
  expression:
    kind: case
    alias: status_norm
```

---

# C9.6 Pattern: safe arithmetic

## C9.6.1 Safe divide with `nullif`

```rust id="safe-divide-expr"
pub fn safe_divide_expr(n: Expr, d: Expr, alias: &'static str) -> Expr {
    (n / nullif(d, lit(0.0))).alias(alias)
}
```

SQL:

```sql id="safe-divide-sql"
SELECT
  numerator / NULLIF(denominator, 0.0) AS ratio
FROM metrics;
```

Use when:

```text id="safe-divide-use"
zero denominator should yield NULL
no custom error/audit behavior required
approximate floating math acceptable
```

---

## C9.6.2 Strict arithmetic with `CASE` guard

```rust id="strict-arithmetic-case"
let ratio_status = case(col("denominator").eq(lit(0.0)))
    .when(lit(true), lit("zero_denominator"))
    .otherwise(lit("ok"))?
    .alias("ratio_status");

let ratio = case(col("denominator").eq(lit(0.0)))
    .when(lit(true), lit(ScalarValue::Float64(None)))
    .otherwise(col("numerator") / col("denominator"))?
    .alias("ratio");
```

Use UDF instead when:

```text id="strict-udf-escalation"
strict behavior must raise execution error
audit struct output is required
division policy is shared across many packages and must be versioned
decimal precision/scale logic is complex
```

---

## C9.6.3 Multi-input safe arithmetic

```rust id="safe-margin"
pub fn margin_expr(revenue: Expr, cost: Expr) -> Expr {
    (revenue.clone() - cost).alias("margin")
}

pub fn margin_pct_expr(revenue: Expr, cost: Expr) -> Expr {
    safe_divide_expr(revenue.clone() - cost, revenue, "margin_pct")
}
```

Agent rules:

```text id="safe-arithmetic-agent"
Prefer nullif/CASE for simple arithmetic safety.
Prefer UDF for exact Decimal overflow policy, audited result structs, or strict error semantics.
Always alias computed arithmetic outputs.
Use typed NULLs when CASE branches include NULL.
```

---

# C9.7 Pattern: bucketization

## C9.7.1 Threshold bucketization

```rust id="threshold-bucket"
pub fn amount_bucket_expr(amount: Expr) -> Expr {
    case(amount.clone().lt(lit(0.0)))
        .when(lit(true), lit("negative"))
        .when(amount.clone().lt(lit(100.0)), lit("small"))
        .when(amount.clone().lt(lit(1000.0)), lit("medium"))
        .otherwise(lit("large"))
        .unwrap()
        .alias("amount_bucket")
}
```

Prefer explicit helper that returns `Result<Expr>` instead of unwrap:

```rust id="threshold-bucket-result"
pub fn amount_bucket_expr(amount: Expr) -> datafusion::error::Result<Expr> {
    Ok(
        case(amount.clone().lt(lit(0.0)))
            .when(lit(true), lit("negative"))
            .when(amount.clone().lt(lit(100.0)), lit("small"))
            .when(amount.clone().lt(lit(1000.0)), lit("medium"))
            .otherwise(lit("large"))?
            .alias("amount_bucket")
    )
}
```

## C9.7.2 SQL

```sql id="bucket-sql"
SELECT
  CASE
    WHEN amount < 0 THEN 'negative'
    WHEN amount < 100 THEN 'small'
    WHEN amount < 1000 THEN 'medium'
    ELSE 'large'
  END AS amount_bucket
FROM orders;
```

## C9.7.3 Generated bucket spec

```yaml id="bucket-spec"
bucket:
  alias: amount_bucket
  input: amount
  ordered_rules:
    - label: negative
      predicate: "amount < 0"
    - label: small
      predicate: "amount < 100"
    - label: medium
      predicate: "amount < 1000"
    - label: large
      default: true
  conflict_policy: first_match_wins
  output_type: Utf8
```

## C9.7.4 Agent rules

```text id="bucket-agent"
Use ordered CASE for small/moderate threshold sets.
Use rule table join for large or business-managed bucket definitions.
Always define boundary inclusivity.
Always test boundary values.
Always alias bucket output.
```

---

# C9.8 Pattern: piecewise functions

## C9.8.1 Piecewise linear formula

```rust id="piecewise-linear"
pub fn tariff_expr(kwh: Expr) -> datafusion::error::Result<Expr> {
    Ok(
        case(kwh.clone().lt_eq(lit(100.0)))
            .when(lit(true), kwh.clone() * lit(0.10))
            .when(kwh.clone().lt_eq(lit(500.0)), lit(10.0) + (kwh.clone() - lit(100.0)) * lit(0.15))
            .otherwise(lit(70.0) + (kwh.clone() - lit(500.0)) * lit(0.20))?
            .alias("tariff")
    )
}
```

SQL:

```sql id="piecewise-sql"
SELECT
  CASE
    WHEN kwh <= 100 THEN kwh * 0.10
    WHEN kwh <= 500 THEN 10.0 + (kwh - 100.0) * 0.15
    ELSE 70.0 + (kwh - 500.0) * 0.20
  END AS tariff
FROM usage;
```

## C9.8.2 Piecewise with domain guard

```rust id="piecewise-domain-guard"
pub fn viscosity_correction_expr(temp_c: Expr) -> datafusion::error::Result<Expr> {
    Ok(
        case(temp_c.clone().is_null())
            .when(lit(true), lit(ScalarValue::Float64(None)))
            .when(temp_c.clone().lt(lit(-50.0)), lit(ScalarValue::Float64(None)))
            .when(temp_c.clone().gt(lit(250.0)), lit(ScalarValue::Float64(None)))
            .when(temp_c.clone().lt(lit(0.0)), lit(1.25))
            .when(temp_c.clone().lt(lit(100.0)), lit(1.00))
            .otherwise(lit(0.85))?
            .alias("viscosity_correction")
    )
}
```

## C9.8.3 UDF escalation triggers

```text id="piecewise-udf-triggers"
Escalate to UDF when:
  branch count is huge and generated CASE becomes unmaintainable
  branch formulas require iterative/numerical methods
  branch selection depends on nested/array algorithms
  output is structured diagnostics
  performance requires custom Arrow kernel
```

---

# C9.9 Pattern: conditional aggregates

## C9.9.1 SQL aggregate `FILTER`

```sql id="aggregate-filter-sql"
SELECT
  customer_id,
  SUM(amount) FILTER (WHERE status = 'paid') AS paid_amount,
  SUM(amount) FILTER (WHERE status = 'refunded') AS refunded_amount,
  COUNT(*) FILTER (WHERE amount > 0) AS positive_rows
FROM orders
GROUP BY customer_id;
```

DataFusion aggregate documentation supports SQL `FILTER (WHERE ...)`; when no rows pass a filter, `COUNT` returns 0 while `SUM`, `AVG`, `MIN`, and `MAX` return null. This matters for generated nullability and defaulting policy. ([datafusion.apache.org](https://datafusion.apache.org/user-guide/sql/aggregate_functions.html?utm_source=chatgpt.com))

---

## C9.9.2 DataFrame aggregate with `CASE`

If builder-level aggregate `FILTER` ergonomics are inconvenient or version-sensitive, use `CASE` inside the aggregate:

```rust id="aggregate-case-rust"
let paid_amount = sum(
    case(col("status").eq(lit("paid")))
        .when(lit(true), col("amount"))
        .otherwise(lit(0.0))?
).alias("paid_amount");

let refunded_amount = sum(
    case(col("status").eq(lit("refunded")))
        .when(lit(true), col("amount"))
        .otherwise(lit(0.0))?
).alias("refunded_amount");

let df = df.aggregate(
    vec![col("customer_id")],
    vec![paid_amount, refunded_amount],
)?;
```

## C9.9.3 Count by condition

```rust id="count-condition"
let positive_count = sum(
    case(col("amount").gt(lit(0.0)))
        .when(lit(true), lit(1_i64))
        .otherwise(lit(0_i64))?
).alias("positive_count");
```

## C9.9.4 Agent rules

```text id="conditional-aggregate-agent"
Prefer aggregate FILTER in SQL when SQL surface is target.
Use CASE-in-aggregate for DataFrame-generated expressions when simpler.
Use UDAF only for custom aggregate state, not ordinary conditional sums/counts.
Document zero-vs-null behavior for no matching rows.
```

---

# C9.10 Pattern: conditional window metrics

## C9.10.1 Window + `CASE` after window

SQL:

```sql id="window-case-sql"
WITH ranked AS (
  SELECT
    customer_id,
    order_id,
    amount,
    ROW_NUMBER() OVER (
      PARTITION BY customer_id
      ORDER BY amount DESC, order_id ASC
    ) AS amount_rank
  FROM orders
)
SELECT
  *,
  CASE
    WHEN amount_rank = 1 THEN true
    ELSE false
  END AS is_top_order
FROM ranked;
```

## C9.10.2 `QUALIFY` for conditional row selection

```sql id="qualify-sql"
SELECT
  customer_id,
  order_id,
  amount,
  ROW_NUMBER() OVER (
    PARTITION BY customer_id
    ORDER BY amount DESC, order_id ASC
  ) AS amount_rank
FROM orders
QUALIFY amount_rank <= 3;
```

The attachment’s SQL syntax map distinguishes `WHERE`, `HAVING`, and `QUALIFY`: `WHERE` filters raw rows, `HAVING` filters grouped rows, and `QUALIFY` filters window-function results. 

## C9.10.3 Conditional rolling metric

```sql id="conditional-window-aggregate"
SELECT
  unit_id,
  ts,
  flow,
  SUM(CASE WHEN flow > 0 THEN flow ELSE 0 END) OVER (
    PARTITION BY unit_id
    ORDER BY ts
    ROWS BETWEEN 6 PRECEDING AND CURRENT ROW
  ) AS rolling_positive_flow
FROM unit_history;
```

## C9.10.4 Agent rules

```text id="window-agent"
Use built-in windows + CASE for conditional window metrics.
Use UDWF only when frame-aware custom state cannot be represented with built-in aggregate/window functions.
Always specify deterministic ORDER BY for ranking/rolling metrics.
Do not filter window output with WHERE; use QUALIFY or outer SELECT.
```

---

# C9.11 Pattern: nested-field conditional extraction

## C9.11.1 Static nested extraction

SQL:

```sql id="nested-static-sql"
SELECT
  payload['user']['id'] AS user_id,
  coalesce(payload['geo']['country'], 'unknown') AS country
FROM events;
```

Rust `select_exprs` route:

```rust id="nested-select-exprs"
let df = ctx.table("events").await?
    .select_exprs(&[
        "payload['user']['id'] AS user_id",
        "coalesce(payload['geo']['country'], 'unknown') AS country",
    ])?;
```

The attachment’s nested-data section documents bracket field access such as `col['field']` and `col[1]`, along with struct, array/list, map, and nested Parquet/Arrow support. 

---

## C9.11.2 Conditional extraction with fallback

```sql id="nested-fallback-sql"
SELECT
  CASE
    WHEN payload['user']['id'] IS NOT NULL THEN payload['user']['id']
    WHEN payload['anonymous_id'] IS NOT NULL THEN payload['anonymous_id']
    ELSE 'unknown'
  END AS actor_id
FROM events;
```

Rust expression-string route:

```rust id="nested-fallback-rust"
let df = df.select_exprs(&[
    "
    CASE
      WHEN payload['user']['id'] IS NOT NULL THEN payload['user']['id']
      WHEN payload['anonymous_id'] IS NOT NULL THEN payload['anonymous_id']
      ELSE 'unknown'
    END AS actor_id
    ",
])?;
```

## C9.11.3 When to use UDF

```text id="nested-udf-use"
Use UDF only when:
  extraction depends on dynamic runtime field names
  output type is dynamic/structured
  nested validation logic is complex
  field fallback requires external/domain registry
```

Agent rule:

```text id="nested-agent"
Static nested access should remain Expr/SQL.
Do not write scalar UDFs that parse JSON strings when data is already typed as Struct/Map/List.
```

---

# C9.12 Pattern: rule-table joins instead of giant `CASE`

## C9.12.1 When CASE becomes wrong abstraction

```text id="case-too-large"
CASE is a bad fit when:
  hundreds/thousands of rules
  rules maintained by business users
  rules have effective dates
  rules require priority/conflict resolution
  rules are tenant/version-specific
  rules require auditability
```

## C9.12.2 Rule table schema

```sql id="rule-table-schema"
CREATE EXTERNAL TABLE classification_rules (
  rule_id VARCHAR,
  priority BIGINT,
  effective_start DATE,
  effective_end DATE,
  min_amount DOUBLE,
  max_amount DOUBLE,
  status VARCHAR,
  output_class VARCHAR
)
STORED AS PARQUET
LOCATION 's3://rules/classification/';
```

## C9.12.3 Rule-table join

```sql id="rule-table-join"
WITH matched AS (
  SELECT
    o.order_id,
    r.rule_id,
    r.priority,
    r.output_class,
    ROW_NUMBER() OVER (
      PARTITION BY o.order_id
      ORDER BY r.priority ASC, r.rule_id ASC
    ) AS rule_rank
  FROM orders AS o
  JOIN classification_rules AS r
    ON o.amount >= r.min_amount
   AND o.amount < r.max_amount
   AND o.status = r.status
   AND o.order_date >= r.effective_start
   AND o.order_date < r.effective_end
)
SELECT
  order_id,
  output_class
FROM matched
WHERE rule_rank = 1;
```

## C9.12.4 Conflict detection query

```sql id="rule-conflict-detection"
SELECT
  o.order_id,
  COUNT(*) AS matching_rule_count
FROM orders AS o
JOIN classification_rules AS r
  ON o.amount >= r.min_amount
 AND o.amount < r.max_amount
 AND o.status = r.status
GROUP BY o.order_id
HAVING COUNT(*) > 1;
```

## C9.12.5 Agent rules

```text id="rule-table-agent"
Use rule table when rules are data.
Use CASE when rules are code.
Always include priority and deterministic tie-breaker.
Always include conflict detection query.
Always include unmatched-record query.
```

---

# C9.13 Generated-Expr builder architecture

## C9.13.1 RuleSpec

```rust id="rule-spec"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RuleSpec {
    pub id: String,
    pub priority: i64,
    pub predicate: PredicateSpec,
    pub output: OutputSpec,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum PredicateSpec {
    Eq { column: String, value: LiteralSpec },
    Lt { column: String, value: LiteralSpec },
    LtEq { column: String, value: LiteralSpec },
    Gt { column: String, value: LiteralSpec },
    GtEq { column: String, value: LiteralSpec },
    IsNull { column: String },
    IsNotNull { column: String },
    And(Vec<PredicateSpec>),
    Or(Vec<PredicateSpec>),
    Not(Box<PredicateSpec>),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputSpec {
    pub value: LiteralSpec,
}
```

## C9.13.2 LiteralSpec

```rust id="literal-spec"
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LiteralSpec {
    Utf8(String),
    Int64(i64),
    Float64(f64),
    Boolean(bool),
    NullFloat64,
    NullUtf8,
}
```

## C9.13.3 Predicate compiler

```rust id="predicate-compiler"
pub fn compile_predicate(spec: &PredicateSpec) -> datafusion::error::Result<Expr> {
    Ok(match spec {
        PredicateSpec::Eq { column, value } => {
            col(column).eq(compile_lit(value)?)
        }
        PredicateSpec::Lt { column, value } => {
            col(column).lt(compile_lit(value)?)
        }
        PredicateSpec::LtEq { column, value } => {
            col(column).lt_eq(compile_lit(value)?)
        }
        PredicateSpec::Gt { column, value } => {
            col(column).gt(compile_lit(value)?)
        }
        PredicateSpec::GtEq { column, value } => {
            col(column).gt_eq(compile_lit(value)?)
        }
        PredicateSpec::IsNull { column } => {
            col(column).is_null()
        }
        PredicateSpec::IsNotNull { column } => {
            col(column).is_not_null()
        }
        PredicateSpec::And(children) => {
            let mut iter = children.iter();
            let first = compile_predicate(
                iter.next().ok_or_else(|| DataFusionError::Plan("empty AND".to_string()))?
            )?;
            iter.try_fold(first, |acc, child| Ok(acc.and(compile_predicate(child)?)))?
        }
        PredicateSpec::Or(children) => {
            let mut iter = children.iter();
            let first = compile_predicate(
                iter.next().ok_or_else(|| DataFusionError::Plan("empty OR".to_string()))?
            )?;
            iter.try_fold(first, |acc, child| Ok(acc.or(compile_predicate(child)?)))?
        }
        PredicateSpec::Not(child) => {
            compile_predicate(child)?.not()
        }
    })
}
```

## C9.13.4 Literal compiler

```rust id="literal-compiler"
pub fn compile_lit(spec: &LiteralSpec) -> datafusion::error::Result<Expr> {
    Ok(match spec {
        LiteralSpec::Utf8(v) => lit(v.as_str()),
        LiteralSpec::Int64(v) => lit(*v),
        LiteralSpec::Float64(v) => lit(*v),
        LiteralSpec::Boolean(v) => lit(*v),
        LiteralSpec::NullFloat64 => lit(ScalarValue::Float64(None)),
        LiteralSpec::NullUtf8 => lit(ScalarValue::Utf8(None)),
    })
}
```

## C9.13.5 CASE compiler

```rust id="case-compiler"
pub fn compile_first_match_case(
    rules: &[RuleSpec],
    default: LiteralSpec,
    alias: &str,
) -> datafusion::error::Result<Expr> {
    let mut sorted = rules.to_vec();
    sorted.sort_by_key(|r| (r.priority, r.id.clone()));

    let mut builder = case(compile_predicate(&sorted[0].predicate)?)
        .when(lit(true), compile_lit(&sorted[0].output.value)?);

    for rule in sorted.iter().skip(1) {
        builder = builder.when(
            compile_predicate(&rule.predicate)?,
            compile_lit(&rule.output.value)?,
        );
    }

    Ok(builder
        .otherwise(compile_lit(&default)?)?
        .alias(alias))
}
```

### Note

The exact builder API can vary by DataFusion version; pin the crate and compile examples. The attachment’s examples use `case(expr).when(...).otherwise(...)?`, and docs.rs exposes `datafusion::prelude::case` returning a `CaseBuilder`.  ([Docs.rs][3])

---

# C9.14 Rule priority and conflict detection

## C9.14.1 Static conflict classes

```text id="conflict-classes"
duplicate priority + overlapping predicates
unreachable rule due to earlier broader rule
missing default
mixed output types that cannot coerce
ambiguous boundary: < vs <=
null predicate not specified
```

## C9.14.2 Generated conflict checks

```rust id="conflict-struct"
#[derive(Debug)]
pub struct RuleValidationReport {
    pub duplicate_priorities: Vec<(i64, Vec<String>)>,
    pub missing_default: bool,
    pub mixed_output_types: Vec<String>,
    pub potential_overlaps: Vec<(String, String)>,
}
```

## C9.14.3 Runtime overlap detection query

For rule-table approach:

```sql id="overlap-query"
SELECT
  input_id,
  COUNT(*) AS matching_rule_count
FROM evaluated_rule_matches
GROUP BY input_id
HAVING COUNT(*) > 1;
```

For generated CASE with first-match semantics:

```text id="case-overlap"
CASE permits overlapping predicates.
First matching branch wins.
Therefore overlap is not runtime error unless product policy declares conflict.
```

## C9.14.4 Agent rules

```text id="conflict-agent"
Always specify first-match vs conflict-error semantics.
Always sort by priority + stable id.
Always add default branch unless output nullable-by-design.
For threshold rules, test exact boundaries.
```

---

# C9.15 Stable aliases and schema contracts

## C9.15.1 Alias every derived expression

```rust id="stable-alias"
let expr = (col("amount") * lit(1.08)).alias("amount_with_tax");
```

## C9.15.2 Alias generated CASE

```rust id="stable-case-alias"
let expr = amount_bucket_expr(col("amount"))?
    .alias("amount_bucket");
```

## C9.15.3 Avoid expression-derived names

Bad:

```rust id="bad-derived-name"
df.select(vec![col("amount") * lit(1.08)])?;
```

Good:

```rust id="good-alias"
df.select(vec![(col("amount") * lit(1.08)).alias("amount_with_tax")])?;
```

The attachment repeatedly emphasizes aliases for computed columns and plan/test stability; expression-derived names are not a robust API contract. 

---

# C9.16 Optimizer value: why composition matters

## C9.16.1 Transparent expression

```rust id="transparent-expression"
let expr = col("amount")
    .gt(lit(0.0))
    .and(col("status").eq(lit("paid")));
```

Potential optimizer benefits:

```text id="transparent-benefits"
predicate can be pushed below projection/join where valid
column dependencies are visible
constant expressions can be simplified
casts can be reasoned about
filter/projection pruning can identify referenced columns
DataFusion can infer type/nullability
```

DataFusion’s optimizer docs describe analyzer, logical optimizer, and physical optimizer rules that rewrite plans/expressions while preserving results; custom optimizer rules must preserve expression names. 

## C9.16.2 Opaque scalar UDF

```sql id="opaque-udf"
SELECT is_paid_positive(amount, status) AS flag
FROM orders;
```

Potential losses:

```text id="udf-losses"
optimizer cannot see status = 'paid'
optimizer cannot see amount > 0
predicate pushdown may be blocked if UDF used in filter
column-level semantics hidden
simplification requires UDF-specific simplify hook
function authorization/cost/security needed
runtime Arrow kernel required
```

DataFusion’s `ScalarUDFImpl::simplify` hook can apply function-specific simplification rules during optimization, but the default implementation does nothing; therefore ordinary expression composition is inherently more visible than an opaque UDF unless hooks are implemented. ([Docs.rs][4])

## C9.16.3 Pushdown-sensitive example

Bad:

```sql id="bad-pushdown-udf"
SELECT *
FROM orders
WHERE is_high_value_paid_order(amount, status);
```

Better:

```sql id="better-pushdown-expr"
SELECT *
FROM orders
WHERE amount > 1000
  AND status = 'paid';
```

Best if reused in Rust:

```rust id="better-pushdown-helper"
pub fn high_value_paid_predicate(amount: Expr, status: Expr) -> Expr {
    amount.gt(lit(1000.0))
        .and(status.eq(lit("paid")))
}
```

---

# C9.17 Deployment patterns

## C9.17.1 Public SQL endpoint

```text id="public-sql-policy"
Prefer:
  built-ins
  CASE
  coalesce/nullif
  rule-table joins
  transparent predicates

Restrict:
  custom scalar UDFs
  volatile/external UDFs
  giant generated CASE without limits
  dynamic SQL expression strings from user input
```

## C9.17.2 Internal generated pipelines

```text id="internal-generated-policy"
Prefer:
  structured rule specs -> Expr
  DFSchema validation
  SQL snapshot tests
  expression helper functions
  rule-table joins for large rulesets

Allow:
  UDF for domain kernels
  UDAF/UDWF for true group/window state
```

## C9.17.3 Model/simulation workbench

```text id="workbench-policy"
Prefer Expr for:
  KPIs
  rule flags
  scenario filters
  quality buckets
  unit conversions with simple factors
  null/default logic

Prefer UDF for:
  thermodynamic formula kernel
  vector/list numerical kernel
  custom parser with audited output
  exact Decimal semantics not expressible in built-ins
```

---

# C9.18 Testing matrix

## C9.18.1 Expression helper tests

```text id="expr-helper-tests"
[ ] get_type against DFSchema
[ ] nullable against DFSchema
[ ] SQL-equivalent output
[ ] boundary values
[ ] null inputs
[ ] mixed branch output type coercion
[ ] stable alias
```

## C9.18.2 CASE tests

```text id="case-tests"
[ ] first branch match
[ ] middle branch match
[ ] default branch
[ ] null predicate input
[ ] boundary inclusivity
[ ] branch output type
[ ] all branches compatible
[ ] generated alias stable
```

## C9.18.3 RuleSpec tests

```text id="rulespec-tests"
[ ] priority sorting deterministic
[ ] duplicate priority detection
[ ] missing default detection
[ ] mixed output type detection
[ ] overlapping rule detection if possible
[ ] compiled Expr type-validates
[ ] compiled Expr SQL output matches fixture
```

## C9.18.4 Rule-table tests

```text id="rule-table-tests"
[ ] exactly one match for clean inputs
[ ] no-match query produces expected rejects
[ ] multi-match conflict query catches overlap
[ ] priority tie-break deterministic
[ ] effective-date filters correct
[ ] boundary conditions correct
```

## C9.18.5 Optimizer visibility tests

```text id="optimizer-tests"
[ ] expression plan references expected columns
[ ] UDF-free filter pushdown still present where expected
[ ] generated expression validates under optimized plan
[ ] EXPLAIN snapshot under pinned DataFusion version
```

---

# C9.19 Manifest patterns

## C9.19.1 Expr calculation manifest

```yaml id="expr-manifest"
calculation:
  name: safe_margin_pct
  placement: Expr
  output_alias: margin_pct
  inputs:
    - revenue
    - cost
  expression_kind: arithmetic_with_nullif
  formula: "(revenue - cost) / NULLIF(revenue, 0)"
  null_policy:
    zero_revenue: null
    null_input: null
  optimizer_visibility: high
  udf_required: false
```

## C9.19.2 CASE calculation manifest

```yaml id="case-manifest"
calculation:
  name: amount_bucket
  placement: ExprCase
  output_alias: amount_bucket
  first_match_wins: true
  rules:
    - id: negative
      priority: 10
      predicate: "amount < 0"
      output: "negative"
    - id: small
      priority: 20
      predicate: "amount < 100"
      output: "small"
    - id: medium
      priority: 30
      predicate: "amount < 1000"
      output: "medium"
  default:
    output: "large"
  conflict_policy: allow_first_match
```

## C9.19.3 Rule-table manifest

```yaml id="rule-table-manifest"
calculation:
  name: order_classification
  placement: RuleTableJoin
  rule_table: classification_rules
  input_table_alias: o
  rule_table_alias: r
  priority_column: priority
  tie_breaker:
    - rule_id
  match_predicates:
    - "o.amount >= r.min_amount"
    - "o.amount < r.max_amount"
    - "o.status = r.status"
  output_columns:
    - output_class
  conflict_detection_query_required: true
  no_match_policy: output_null
```

---

# C9.20 Anti-pattern inventory

* Scalar UDF wrapping `CASE`.
* Scalar UDF wrapping `coalesce`.
* Scalar UDF wrapping `numerator / NULLIF(denominator, 0)`.
* Scalar UDF hiding `status = 'paid' AND amount > 0` in a filter.
* Generated SQL string concatenation instead of `Expr` helpers.
* Giant `CASE` for business-managed rules that belong in a table.
* Rule table without priority/tie-breaker.
* Rule table without conflict/no-match queries.
* CASE branches with incompatible output types.
* CASE output without alias.
* Boundary rules without explicit `<` vs `<=`.
* Window metric filtered with `WHERE` instead of `QUALIFY`/outer select.
* Conditional aggregate implemented as scalar UDF.
* Nested typed fields converted to JSON strings and parsed by UDF.
* `Expr` generation without `DFSchema` validation.
* UDF added before checking whether built-ins already solve the problem.
* Opaque UDF used in WHERE, blocking predicate visibility.
* No EXPLAIN/pushdown regression tests for critical filters.

---

# C9.21 Agent checklist

```text id="c9-final-checklist"
[ ] Check if built-in SQL/Expr functions solve the calculation.
[ ] Use Expr helper for reusable generated logic.
[ ] Use CASE for SQL-visible conditional derivations.
[ ] Use coalesce for null defaulting.
[ ] Use nullif for sentinel-to-null and divide-by-zero guards.
[ ] Use CASE for bucketization and piecewise formulas.
[ ] Use aggregate FILTER or CASE-in-aggregate for conditional aggregates.
[ ] Use built-in windows + CASE/QUALIFY for conditional window metrics.
[ ] Use nested field access + coalesce/CASE for static nested extraction.
[ ] Use rule-table join instead of giant CASE when rules are data.
[ ] Add priority and deterministic tie-breaker to rule-table logic.
[ ] Add conflict-detection query for rule tables.
[ ] Add no-match query for rule tables.
[ ] Build generated expressions from structured specs, not string concatenation.
[ ] Validate generated Expr with get_type / nullable against DFSchema.
[ ] Alias every generated expression.
[ ] Test boundary values and null behavior.
[ ] Use UDF only when expression composition is insufficient.
[ ] If using UDF, record optimizer-visibility loss and consider simplify hooks.
[ ] Snapshot EXPLAIN for optimizer-sensitive expression rewrites under pinned DataFusion version.
```

[1]: https://datafusion.apache.org/user-guide/expressions.html?utm_source=chatgpt.com "Expression API — Apache DataFusion documentation"
[2]: https://datafusion.apache.org/library-user-guide/functions/adding-udfs.html?utm_source=chatgpt.com "Adding User Defined Functions: Scalar/Window/Aggregate ..."
[3]: https://docs.rs/datafusion/latest/datafusion/prelude/fn.case.html?utm_source=chatgpt.com "case in datafusion::prelude - Rust"
[4]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html?utm_source=chatgpt.com "ScalarUDFImpl in datafusion_expr - Rust"


# DataFusion Advanced — C10) Nested and structured return calculations

## C10.0 Objective

Handle DataFusion UDFs that **consume or produce nested Arrow values**:

```text id="c10-root"
Nested UDF contract
  ├─ input shape
  │    ├─ LIST / LargeList / FixedSizeList
  │    ├─ STRUCT
  │    ├─ MAP
  │    ├─ Dictionary / categorical
  │    └─ nested combinations: List<Struct<...>>, Struct<List<...>>, Map<Utf8, List<...>>
  ├─ output shape
  │    ├─ StructArray
  │    ├─ ListArray
  │    ├─ MapArray
  │    ├─ FixedSizeListArray embeddings
  │    └─ nested diagnostics records
  ├─ planner contract
  │    ├─ return_field_from_args
  │    ├─ child field names
  │    ├─ child nullability
  │    ├─ metadata propagation
  │    └─ schema evolution policy
  └─ runtime contract
       ├─ exact Arrow array type
       ├─ row-count preservation
       ├─ parent/child null handling
       ├─ builder invariants
       └─ Parquet/Arrow round-trip tests
```

The attachment already covers nested `ARRAY`/`LIST`, `STRUCT`, map-like fields, field access, struct construction, array/map/struct functions, nested Parquet/Arrow reading and writing, plus UDF-family placement.  This chapter adds the UDF-specific layer: **return-field design, Arrow builder contracts, runtime validation, schema evolution, diagnostics, and nested edge-case tests**.

Current DataFusion docs expose `get_field(expression, field_name[, ...])` for struct/map access, noting that common bracket syntax such as `my_struct_col['field_name']` lowers to `get_field`; nested access like `my_struct['a']['b']` is optimized to one `get_field(my_struct, 'a', 'b')` call. ([Apache DataFusion][1])

---

## C10.1 Core principle

```text id="c10-principle"
Use typed nested Arrow returns when output has structure.

Prefer:
  StructArray diagnostics
  ListArray repeated values
  FixedSizeListArray embeddings
  MapArray dynamic attributes

Avoid:
  JSON strings for typed values
  comma-delimited vectors
  opaque Utf8 diagnostics
  List<Any>-style schemas
  runtime-dependent output schemas
```

Value case:

```text id="c10-value"
Typed nested returns preserve:
  schema introspection
  Arrow/Parquet compatibility
  SQL field access
  downstream projection
  nullability contracts
  metadata/units/semantic tags
  LLM-agent type reasoning
```

---

## C10.2 Nested type contract layers

| Layer              | Contract                                                               | Example                                            |
| ------------------ | ---------------------------------------------------------------------- | -------------------------------------------------- |
| `DataType`         | nested physical/logical shape                                          | `Struct`, `List(FieldRef)`, `Map`, `FixedSizeList` |
| child `Field`      | child name, child type, child nullability, metadata                    | `Field::new("status", Utf8, false)`                |
| parent `Field`     | output column name, parent nullability, parent metadata                | `Field::new_struct("diagnostic", fields, false)`   |
| runtime array      | actual `StructArray` / `ListArray` / `MapArray` / `FixedSizeListArray` | builder output                                     |
| logical SQL access | bracket / `get_field` / `unnest` / map functions                       | `diagnostic['status']`                             |
| storage            | Arrow/Parquet nested schema                                            | round-trip tests                                   |

DataFusion’s `ScalarUDFImpl::return_field_from_args` is the correct production hook for full return-field inference; if implemented, DataFusion will not call `return_type`, and docs recommend returning a `DataFusionError::Internal` instead of panicking from `return_type` in this path. ([Docs.rs][2])

---

# C10.3 Return-field design

## C10.3.1 Field-first pattern

Nested UDFs should usually implement `return_field_from_args`, not only `return_type`.

```rust id="return-field-first"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};
use datafusion::common::{internal_err, plan_err, Result};
use datafusion::logical_expr::{ReturnFieldArgs, ScalarUDFImpl};

impl ScalarUDFImpl for MyNestedUdf {
    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        internal_err!(
            "{} uses return_field_from_args; return_type should not be called with {arg_types:?}",
            self.name()
        )
    }

    fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
        // compute full nested Field
        todo!()
    }
}
```

## C10.3.2 Required return-field decisions

```text id="return-field-decisions"
Parent:
  output field name
  parent DataType
  parent nullable
  parent metadata

Children:
  child field names
  child DataTypes
  child nullable flags
  child metadata
  child order
  child evolution policy

Semantics:
  null input behavior
  empty collection behavior
  invalid child behavior
  output metadata propagation
  schema version
```

## C10.3.3 Metadata policy

```yaml id="metadata-policy"
metadata_policy:
  preserve_input_metadata: false
  attach:
    semantic_type: vector_diagnostics
    calculation_version: vector_distance_diag_v1
    unit: dimensionless
  child_metadata:
    value:
      semantic_type: distance
      unit: dimensionless
    status:
      semantic_type: diagnostic_status
    reason:
      semantic_type: diagnostic_reason
```

Agent rule:

```text id="metadata-agent"
Nested child fields need metadata discipline too.
Do not attach parent metadata and forget child semantics.
```

---

# C10.4 UDFs returning `StructArray`

## C10.4.1 Use cases

```text id="struct-use"
audited calculations
parse results
validation records
multi-output domain metrics
diagnostic structs
status/value/reason triples
nested normalized objects
```

Example output:

```text id="struct-shape"
parse_stream_code(raw_code) -> Struct<
  value: Utf8 nullable,
  status: Utf8 non-null,
  reason: Utf8 nullable,
  code: Utf8 nullable
>
```

## C10.4.2 Return field

```rust id="struct-return-field"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};

pub fn parse_stream_code_field() -> FieldRef {
    let fields = vec![
        Arc::new(Field::new("value", DataType::Utf8, true)),
        Arc::new(Field::new("status", DataType::Utf8, false)),
        Arc::new(Field::new("reason", DataType::Utf8, true)),
        Arc::new(Field::new("code", DataType::Utf8, true)),
    ];

    Arc::new(Field::new_struct(
        "stream_code_parse",
        fields,
        false, // parent struct itself is always present
    ))
}
```

## C10.4.3 Runtime builder

```rust id="struct-builder-runtime"
use std::sync::Arc;
use datafusion::arrow::{
    array::{Array, ArrayBuilder, ArrayRef, StringArray, StringBuilder, StructArray, StructBuilder},
    datatypes::{DataType, Field, Fields},
};
use datafusion::common::{DataFusionError, Result};

pub fn parse_stream_code_array(input: &StringArray) -> Result<StructArray> {
    let fields = Fields::from(vec![
        Arc::new(Field::new("value", DataType::Utf8, true)),
        Arc::new(Field::new("status", DataType::Utf8, false)),
        Arc::new(Field::new("reason", DataType::Utf8, true)),
        Arc::new(Field::new("code", DataType::Utf8, true)),
    ]);

    let child_builders: Vec<Box<dyn ArrayBuilder>> = vec![
        Box::new(StringBuilder::with_capacity(input.len(), input.len() * 8)),
        Box::new(StringBuilder::with_capacity(input.len(), input.len() * 8)),
        Box::new(StringBuilder::with_capacity(input.len(), input.len() * 16)),
        Box::new(StringBuilder::with_capacity(input.len(), input.len() * 8)),
    ];

    let mut b = StructBuilder::new(fields, child_builders);

    for row in 0..input.len() {
        let raw = if input.is_null(row) { None } else { Some(input.value(row)) };

        let (value, status, reason, code) = match raw {
            None => (None, "null_input", Some("input was null"), Some("NULL_INPUT")),
            Some(s) if s.starts_with("STR-") => (Some(s), "ok", None, None),
            Some(s) => (
                None,
                "invalid",
                Some("expected code prefix STR-"),
                Some("INVALID_PREFIX"),
            ),
        };

        b.field_builder::<StringBuilder>(0)
            .ok_or_else(|| DataFusionError::Internal("missing value builder".to_string()))?
            .append_option(value);

        b.field_builder::<StringBuilder>(1)
            .ok_or_else(|| DataFusionError::Internal("missing status builder".to_string()))?
            .append_value(status);

        b.field_builder::<StringBuilder>(2)
            .ok_or_else(|| DataFusionError::Internal("missing reason builder".to_string()))?
            .append_option(reason);

        b.field_builder::<StringBuilder>(3)
            .ok_or_else(|| DataFusionError::Internal("missing code builder".to_string()))?
            .append_option(code);

        b.append(true);
    }

    Ok(b.finish())
}
```

Arrow’s `StructBuilder` exposes `field_builder` to obtain each child builder and then append child values; the builder must maintain child lengths consistently with parent struct slots. ([Docs.rs][3])

## C10.4.4 SQL consumption

```sql id="struct-consumption-sql"
WITH parsed AS (
  SELECT
    stream_id,
    parse_stream_code(raw_code) AS p
  FROM streams
)
SELECT
  stream_id,
  p['value'] AS stream_code,
  p['status'] AS parse_status,
  p['reason'] AS parse_reason
FROM parsed
WHERE p['status'] <> 'ok';
```

## C10.4.5 Agent rules

```text id="struct-agent-rules"
Use StructArray for row-preserving diagnostics.
Make parent struct non-null when diagnostics always exists.
Make `value` nullable and `status` non-null.
Use stable machine-readable `code`.
Do not return free-text-only diagnostic strings.
Append every child once per parent row.
Call parent append exactly once per row.
```

---

# C10.5 UDFs returning `ListArray`

## C10.5.1 Use cases

```text id="list-use"
tokenization: Utf8 -> List<Utf8>
top-k local candidates: row -> List<Struct<id,score>>
decomposition: domain object -> List<Float64>
normalization trace: input -> List<Float64>
variable-length vector features
```

## C10.5.2 Return field

```rust id="list-return-field"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};

pub fn token_list_field() -> FieldRef {
    let item = Arc::new(Field::new("item", DataType::Utf8, false));

    Arc::new(Field::new_list(
        "tokens",
        item,
        true, // list itself may be null
    ))
}
```

List parent nullability and element nullability are separate; Arrow list builders require appending child values and then calling `append` to delimit each parent list value. ([Docs.rs][4])

## C10.5.3 Runtime builder

```rust id="list-builder-runtime"
use datafusion::arrow::array::{Array, ListArray, ListBuilder, StringArray, StringBuilder};
use datafusion::common::Result;

pub fn tokenize_array(input: &StringArray) -> Result<ListArray> {
    let values = StringBuilder::new();
    let mut b = ListBuilder::new(values);

    for row in 0..input.len() {
        if input.is_null(row) {
            b.append(false); // null list
            continue;
        }

        let s = input.value(row);

        for token in s.split_whitespace() {
            b.values().append_value(token);
        }

        // Empty string => non-null empty list if no tokens appended.
        b.append(true);
    }

    Ok(b.finish())
}
```

## C10.5.4 Empty list vs null list

```text id="empty-vs-null"
NULL list:
  parent slot invalid
  meaning: no list value / unknown / invalid input

empty list:
  parent slot valid
  zero child elements
  meaning: known empty collection
```

SQL expectation:

```sql id="list-sql-expect"
SELECT
  token_count,
  tokens IS NULL AS tokens_is_null
FROM (
  SELECT
    array_length(tokenize_text(raw_text)) AS token_count,
    tokenize_text(raw_text) AS tokens
  FROM docs
);
```

## C10.5.5 Agent rules

```text id="list-agent-rules"
Use ListArray for variable-length outputs.
Define null list vs empty list semantics.
Define element nullability.
For tokenization, empty string usually -> empty list, NULL input -> NULL list.
For validation outputs, invalid input may be NULL list or audited struct; choose explicitly.
```

---

# C10.6 UDFs returning `MapArray`

## C10.6.1 Use cases

```text id="map-use"
dynamic attributes
sparse diagnostics
schema-less metadata bags
key-value extracted tags
unbounded set of non-hot fields
```

Use `StructArray` instead when keys are known and stable.

## C10.6.2 Arrow Map facts

Arrow `MapArray` is physically a list of key-value pairs stored as an `entries` struct with two child fields; keys should be non-null, while values may be null. ([Docs.rs][5]) Arrow’s `MapBuilder::new(None, key_builder, value_builder)` constructs maps, `keys()` and `values()` append entries, and `append(is_valid)` finishes each map slot. ([Docs.rs][6])

## C10.6.3 Return field

```rust id="map-return-field"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};

pub fn attributes_map_field() -> FieldRef {
    let key = Arc::new(Field::new("keys", DataType::Utf8, false));
    let value = Arc::new(Field::new("values", DataType::Utf8, true));

    Arc::new(Field::new_map(
        "attributes",
        "entries",
        key,
        value,
        false, // keys sorted?
        true,  // map itself nullable
    ))
}
```

## C10.6.4 Runtime builder

```rust id="map-builder-runtime"
use datafusion::arrow::array::{Array, MapArray, MapBuilder, StringArray, StringBuilder};
use datafusion::common::{DataFusionError, Result};

pub fn extract_attributes(input: &StringArray) -> Result<MapArray> {
    let key_builder = StringBuilder::new();
    let value_builder = StringBuilder::new();
    let mut b = MapBuilder::new(None, key_builder, value_builder);

    for row in 0..input.len() {
        if input.is_null(row) {
            b.append(false).map_err(DataFusionError::from)?;
            continue;
        }

        let raw = input.value(row);

        // Example: always emit two keys for valid input.
        b.keys().append_value("raw_length");
        b.values().append_value(raw.len().to_string());

        b.keys().append_value("has_dash");
        b.values().append_value(raw.contains('-').to_string());

        b.append(true).map_err(DataFusionError::from)?;
    }

    Ok(b.finish())
}
```

## C10.6.5 Map lookup with defaults

Prefer native SQL map functions when possible:

```sql id="map-lookup-sql"
SELECT
  coalesce(element_at(attrs, 'source'), 'unknown') AS source
FROM events;
```

Use UDF when:

```text id="map-udf-use"
default depends on complex key policy
absent-vs-present-null must be audited
keys need normalization
map values are nested and require validation
```

## C10.6.6 Agent rules

```text id="map-agent-rules"
Map keys must be non-null.
Document duplicate-key behavior.
Use Struct when key set is fixed.
Use Map for dynamic sparse attributes.
Primitive nullable map lookup cannot distinguish absent key from present-null unless extra diagnostics are returned.
```

---

# C10.7 Fixed-size list embeddings

## C10.7.1 Use cases

```text id="fixed-list-use"
embedding vectors
dense numeric feature arrays
RGB/XYZ coordinate vectors
fixed-length process-state vector
model input features
```

Arrow `FixedSizeListArray` represents a list where each value has the same fixed number of child elements, and each list may itself be null or contain null/non-null values. ([Docs.rs][7])

## C10.7.2 Return field

```rust id="fixed-list-return-field"
use std::sync::Arc;
use datafusion::arrow::datatypes::{DataType, Field, FieldRef};

pub fn embedding_field(dim: i32) -> FieldRef {
    let item = Arc::new(Field::new("item", DataType::Float32, false));

    Arc::new(Field::new_fixed_size_list(
        "embedding",
        item,
        dim,
        false, // embedding parent non-null if always produced
    ))
}
```

## C10.7.3 Runtime builder

```rust id="fixed-list-builder"
use datafusion::arrow::array::{Array, FixedSizeListArray, FixedSizeListBuilder, Float32Builder, StringArray};
use datafusion::common::{DataFusionError, Result};

pub fn mock_embedding_array(input: &StringArray, dim: i32) -> Result<FixedSizeListArray> {
    let values = Float32Builder::with_capacity(input.len() * dim as usize);
    let mut b = FixedSizeListBuilder::new(values, dim);

    for row in 0..input.len() {
        if input.is_null(row) {
            // Append dim null child values, then invalid parent.
            for _ in 0..dim {
                b.values().append_null();
            }
            b.append(false);
            continue;
        }

        let seed = input.value(row).len() as f32;

        for j in 0..dim {
            b.values().append_value(seed + j as f32);
        }

        b.append(true);
    }

    Ok(b.finish())
}
```

`FixedSizeListBuilder::values()` returns the child builder and requires calling `append` to delimit each distinct fixed-size list value. ([Docs.rs][8])

## C10.7.4 Dimension from literal argument

```rust id="fixed-list-return-from-literal"
fn return_field_from_args(&self, args: ReturnFieldArgs<'_>) -> Result<FieldRef> {
    let Some(Some(dim_scalar)) = args.scalar_arguments.get(1) else {
        return plan_err!("embedding dimension must be a scalar literal");
    };

    let dim = match dim_scalar {
        ScalarValue::Int32(Some(v)) if *v > 0 => *v,
        ScalarValue::Int64(Some(v)) if *v > 0 && *v <= i32::MAX as i64 => *v as i32,
        other => return plan_err!("invalid embedding dimension literal: {other:?}"),
    };

    Ok(embedding_field(dim))
}
```

## C10.7.5 Agent rules

```text id="fixed-list-agent-rules"
Use FixedSizeList for fixed-dimension embeddings.
Reject non-literal dimensions at planning if output schema depends on dimension.
Ensure child value count = rows * dimension.
Parent null requires correct child slots.
Test zero dimension only if explicitly supported.
```

---

# C10.8 Nested diagnostics records

## C10.8.1 Shape

```text id="nested-diagnostics-shape"
validate_vector(vec) -> Struct<
  value: FixedSizeList<Float32, 384> nullable,
  diagnostics: Struct<
    status: Utf8 non-null,
    code: Utf8 nullable,
    reason: Utf8 nullable,
    metrics: Map<Utf8, Utf8> nullable
  > non-null
>
```

## C10.8.2 Return field

```rust id="nested-diagnostics-field"
pub fn vector_validation_field(dim: i32) -> FieldRef {
    let value_item = Arc::new(Field::new("item", DataType::Float32, false));

    let value = Arc::new(Field::new_fixed_size_list(
        "value",
        value_item,
        dim,
        true,
    ));

    let metrics_key = Arc::new(Field::new("keys", DataType::Utf8, false));
    let metrics_val = Arc::new(Field::new("values", DataType::Utf8, true));

    let metrics = Arc::new(Field::new_map(
        "metrics",
        "entries",
        metrics_key,
        metrics_val,
        false,
        true,
    ));

    let diagnostics = Arc::new(Field::new_struct(
        "diagnostics",
        vec![
            Arc::new(Field::new("status", DataType::Utf8, false)),
            Arc::new(Field::new("code", DataType::Utf8, true)),
            Arc::new(Field::new("reason", DataType::Utf8, true)),
            metrics,
        ],
        false,
    ));

    Arc::new(Field::new_struct(
        "vector_validation",
        vec![value, diagnostics],
        false,
    ))
}
```

## C10.8.3 Agent rules

```text id="nested-diagnostics-rules"
Use nested diagnostics when:
  value and diagnostic metadata must stay row-aligned
  rejected rows must be queryable without side table
  downstream SQL should filter on status/code
  multiple diagnostic fields are needed

Avoid:
  nested diagnostics for hot primitive metrics unless needed
  unbounded maps inside every row without memory budget
```

---

# C10.9 Functions over nested inputs

## C10.9.1 Vector distance

### Contract

```text id="vector-distance-contract"
vector_distance(left: List<Float64>, right: List<Float64>) -> Float64
null list -> null
empty lists -> 0 only if both empty and metric permits
unequal lengths -> error/null/audited by policy
null element -> null/error/skip by policy
```

### Runtime

```rust id="vector-distance-runtime"
use datafusion::arrow::array::{Array, Float64Array, ListArray};
use datafusion::common::{exec_err, internal_err, Result};

fn list_value_as_f64(list: &ListArray, row: usize, fn_name: &str) -> Result<Option<&Float64Array>> {
    if list.is_null(row) {
        return Ok(None);
    }

    let value = list.value(row);
    let arr = value
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| datafusion::error::DataFusionError::Internal(format!(
            "{fn_name} expected List<Float64>, got child {:?}",
            value.data_type()
        )))?;

    Ok(Some(arr))
}

fn vector_distance_lists(left: &ListArray, right: &ListArray) -> Result<Float64Array> {
    if left.len() != right.len() {
        return internal_err!("vector_distance input arrays have different row counts");
    }

    let mut out = Float64Builder::with_capacity(left.len());

    for row in 0..left.len() {
        let Some(l) = list_value_as_f64(left, row, "vector_distance")? else {
            out.append_null();
            continue;
        };
        let Some(r) = list_value_as_f64(right, row, "vector_distance")? else {
            out.append_null();
            continue;
        };

        if l.len() != r.len() {
            return exec_err!(
                "vector_distance length mismatch at row {}: left={}, right={}",
                row,
                l.len(),
                r.len()
            );
        }

        let mut sum_sq = 0.0;

        for i in 0..l.len() {
            if l.is_null(i) || r.is_null(i) {
                out.append_null();
                continue;
            }

            let delta = l.value(i) - r.value(i);
            sum_sq += delta * delta;
        }

        out.append_value(sum_sq.sqrt());
    }

    Ok(out.finish())
}
```

### Safer helper for null-element flow

```rust id="vector-distance-helper"
fn distance_one(l: &Float64Array, r: &Float64Array, row: usize) -> Result<Option<f64>> {
    if l.len() != r.len() {
        return exec_err!(
            "vector_distance length mismatch at row {}: left={}, right={}",
            row,
            l.len(),
            r.len()
        );
    }

    let mut sum_sq = 0.0;

    for i in 0..l.len() {
        if l.is_null(i) || r.is_null(i) {
            return Ok(None);
        }
        let delta = l.value(i) - r.value(i);
        sum_sq += delta * delta;
    }

    Ok(Some(sum_sq.sqrt()))
}
```

### Agent rules

```text id="vector-distance-rules"
Prefer built-in vector/array functions when available and semantics match.
Use UDF when null policy, metric, or diagnostics differ.
Use FixedSizeList when dimension is fixed.
Use List when variable dimension is expected but validate each row.
```

---

## C10.9.2 Array normalization

### Contract

```text id="array-normalization-contract"
normalize_vector(values: List<Float64>) -> List<Float64>
null list -> null list
empty list -> empty list or null by policy
zero norm -> null/error/original by policy
null element -> null output list or null element by policy
```

### Runtime

```rust id="array-normalization-runtime"
pub fn normalize_vector_list(input: &ListArray) -> Result<ListArray> {
    let values = Float64Builder::new();
    let mut b = ListBuilder::new(values);

    for row in 0..input.len() {
        if input.is_null(row) {
            b.append(false);
            continue;
        }

        let value = input.value(row);
        let arr = value
            .as_any()
            .downcast_ref::<Float64Array>()
            .ok_or_else(|| DataFusionError::Internal(format!(
                "normalize_vector expected List<Float64>, got child {:?}",
                value.data_type()
            )))?;

        let mut norm_sq = 0.0;
        let mut has_null = false;

        for i in 0..arr.len() {
            if arr.is_null(i) {
                has_null = true;
                break;
            }
            norm_sq += arr.value(i) * arr.value(i);
        }

        if has_null {
            // Policy: null element => null parent list.
            b.append(false);
            continue;
        }

        let norm = norm_sq.sqrt();

        if norm == 0.0 {
            // Policy: zero vector => null parent list.
            b.append(false);
            continue;
        }

        for i in 0..arr.len() {
            b.values().append_value(arr.value(i) / norm);
        }

        b.append(true);
    }

    Ok(b.finish())
}
```

---

## C10.9.3 Struct validation

### Contract

```text id="struct-validation-contract"
validate_stream_props(props: Struct<api, sulfur, phase>) -> Struct<is_valid,status,reason>
null parent struct -> diagnostic status null_input
missing field -> planning or execution error depending static/dynamic contract
null child -> invalid or nullable depending policy
```

### Runtime

```rust id="struct-validation-runtime"
fn validate_stream_props(input: &StructArray) -> Result<StructArray> {
    let api = input
        .column_by_name("api")
        .ok_or_else(|| DataFusionError::Execution("missing struct field `api`".to_string()))?;

    let sulfur = input
        .column_by_name("sulfur")
        .ok_or_else(|| DataFusionError::Execution("missing struct field `sulfur`".to_string()))?;

    let api = api.as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataFusionError::Internal("api must be Float64".to_string()))?;

    let sulfur = sulfur.as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| DataFusionError::Internal("sulfur must be Float64".to_string()))?;

    // build StructArray diagnostic as in prior section
    todo!()
}
```

### Agent rules

```text id="struct-validation-rules"
Use column_by_name for semantic fields.
Do not depend on struct child position unless contract explicitly says positional.
Test null parent and null children separately.
For schema evolution, tolerate added fields but reject missing required fields.
```

DataFusion uses name-based field mapping when coercing struct types across operations; this matters for UDF contracts because field names, not just positions, are semantic. ([Apache DataFusion][9])

---

## C10.9.4 Map lookup with defaults

### Contract

```text id="map-lookup-contract"
map_get_default(map: Map<Utf8, Utf8>, key: Utf8, default: Utf8) -> Utf8
null map -> default or null by policy
null key -> error/null/default by policy
absent key -> default
present key with null value -> null or default by policy
```

### Prefer SQL built-ins when possible

```sql id="map-default-sql"
SELECT
  coalesce(element_at(attrs, 'source'), 'unknown') AS source
FROM events;
```

Use UDF when absent-key vs present-null distinction matters, or when key normalization/default policy is complex.

### Agent rules

```text id="map-lookup-rules"
Primitive map lookup cannot always distinguish absent key vs present-null.
Return audited struct if distinction matters:
  value
  status: ok | absent | present_null | null_map | null_key
```

---

## C10.9.5 Nested schema normalization

### Use case

```text id="schema-normalization-use"
normalize_payload(payload: Struct<...>) -> Struct<stable fields>
```

### Output field policy

```text id="schema-normalization-policy"
Input schema may evolve.
Output schema must remain stable.
Added input fields ignored unless mapped.
Missing required input fields -> planning/execution error.
Missing optional input fields -> null/default child values.
```

### Return field

```rust id="normalized-payload-field"
pub fn normalized_payload_field() -> FieldRef {
    Arc::new(Field::new_struct(
        "normalized_payload",
        vec![
            Arc::new(Field::new("user_id", DataType::Utf8, true)),
            Arc::new(Field::new("event_type", DataType::Utf8, true)),
            Arc::new(Field::new(
                "event_ts",
                DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
                true,
            )),
            Arc::new(Field::new("attributes", DataType::Utf8, true)),
        ],
        false,
    ))
}
```

### Agent rules

```text id="schema-normalization-rules"
Nested normalization UDF is a schema boundary.
Stabilize output child names and types.
Version output schema.
Add migration tests.
Do not mirror arbitrary input struct fields unless output schema is intentionally dynamic.
```

---

# C10.10 Schema evolution

## C10.10.1 Evolution classes

| Change                                    |                                    Compatibility | Notes                                                                                   |
| ----------------------------------------- | -----------------------------------------------: | --------------------------------------------------------------------------------------- |
| add nullable child field to output struct | usually backward-compatible for tolerant readers | may break strict schema snapshots                                                       |
| add non-null child field                  |                                         breaking | old rows cannot produce value                                                           |
| remove child field                        |                                         breaking | downstream SQL field access fails                                                       |
| rename child field                        |                                         breaking | field access is name-based                                                              |
| reorder struct children                   |     maybe compatible logically, risky physically | DataFusion struct coercion is name-based in operations, but external consumers may care |
| change child nullability true -> false    |                                   maybe breaking | downstream may rely on nullable                                                         |
| change child type                         |                                         breaking | use new function/version                                                                |
| change list item nullability              |                                   maybe breaking | semantics changed                                                                       |
| change fixed-size list dimension          |                                         breaking | embedding dimension changed                                                             |
| change map key/value type                 |                                         breaking | downstream lookups/casts fail                                                           |

## C10.10.2 Versioned struct output

```text id="versioned-output"
parse_stream_code_v1(raw) -> Struct<value,status,reason>
parse_stream_code_v2(raw) -> Struct<value,status,reason,code,confidence>
```

Agent rule:

```text id="schema-evolution-agent"
Do not silently mutate public nested return schema.
Add a v2 function or compatibility alias with deprecation.
```

---

# C10.11 SQL discovery and testing helpers

## C10.11.1 Type audit

```sql id="type-audit-sql"
SELECT
  arrow_typeof(parse_stream_code(raw_code)) AS parse_type,
  arrow_typeof(tokenize_text(raw_text)) AS tokens_type,
  arrow_typeof(mock_embedding(text, 384)) AS embedding_type
FROM t
LIMIT 1;
```

DataFusion’s `arrow_typeof` exposes the Arrow type of SQL expressions and is the right SQL-level assertion tool for nested UDF return types. ([Apache DataFusion][10])

## C10.11.2 Field access audit

```sql id="field-access-audit"
WITH p AS (
  SELECT parse_stream_code(raw_code) AS parsed
  FROM streams
)
SELECT
  parsed['value'] AS value,
  parsed['status'] AS status,
  parsed['reason'] AS reason
FROM p;
```

## C10.11.3 Unnest audit

```sql id="unnest-audit"
SELECT
  doc_id,
  unnest(tokenize_text(raw_text)) AS token
FROM docs;
```

DataFusion’s special functions include `unnest`, which expands arrays/maps into rows and `unnest(struct)`, which expands a struct into columns; use it for test inspection but remember it changes cardinality. ([Apache DataFusion][11])

---

# C10.12 Test matrix

## C10.12.1 List tests

```text id="list-tests"
[ ] null list input
[ ] empty list input
[ ] list with null element
[ ] all-null list elements
[ ] variable-length lists
[ ] nested List<Struct>
[ ] output parent nullability
[ ] output child nullability
[ ] SQL array_length
[ ] unnest output
[ ] Parquet round-trip if materialized
```

## C10.12.2 Struct tests

```text id="struct-tests"
[ ] null parent struct
[ ] null required child
[ ] null optional child
[ ] missing required field
[ ] added irrelevant field
[ ] child field order changed
[ ] child field name changed
[ ] SQL field access by bracket
[ ] unnest(struct)
[ ] schema snapshot
```

## C10.12.3 Map tests

```text id="map-tests"
[ ] null map
[ ] empty map
[ ] absent key
[ ] present key with null value
[ ] present key with non-null value
[ ] null key rejected
[ ] duplicate key policy
[ ] map_keys/map_values/map_entries if used
[ ] Parquet round-trip if materialized
```

## C10.12.4 Fixed-size list tests

```text id="fixed-size-tests"
[ ] dimension from literal
[ ] non-literal dimension rejected
[ ] dimension zero rejected unless supported
[ ] null parent embedding
[ ] null child element
[ ] child value count = rows * dimension
[ ] FixedSizeListArray type audit
[ ] vector distance with same dimension
[ ] vector distance mismatch fails/null/audits by policy
```

## C10.12.5 Diagnostics tests

```text id="diagnostics-tests"
[ ] status ok
[ ] status invalid
[ ] status null_input
[ ] status missing_field
[ ] value null with non-ok status
[ ] status non-null for every row
[ ] code stable and machine-readable
[ ] reason human-readable but not used as key
```

---

# C10.13 Deployment guidance

## C10.13.1 Public SQL

```text id="public-sql"
Allow:
  stable Struct/List/Map outputs with documented schema
  field-access examples
  arrow_typeof tests
  cardinality-safe nested functions

Restrict:
  functions producing unbounded lists/maps
  dynamic output schemas
  deeply nested diagnostics without result-size limits
  UDFs requiring arbitrary map keys from user input without policy
```

## C10.13.2 ETL / lakehouse

```text id="etl-lake"
Before materializing nested UDF outputs:
  snapshot schema
  test Arrow IPC round-trip
  test Parquet round-trip
  test downstream reader compatibility
  decide metadata retention
  decide schema evolution policy
```

## C10.13.3 Engineering/modeling

```text id="engineering"
Prefer:
  Struct outputs for multi-metric results
  FixedSizeList for state vectors / embeddings
  List<Float64> for variable-length series
  Map only for sparse dynamic attributes

Avoid:
  untyped JSON strings
  map for hot fixed fields
  dynamic schema based on row values
```

---

# C10.14 Anti-pattern inventory

* Returning JSON string for typed diagnostics.
* Returning `StructArray` without stable child names.
* Parent struct nullable when diagnostics should always exist.
* `status` nullable in diagnostic struct.
* `value` non-null when invalid rows exist.
* Using `List` when dimension is fixed and should be enforced.
* Using `FixedSizeList` while accepting runtime variable dimensions.
* Treating empty list and null list as equivalent.
* Treating null struct and struct with null children as equivalent.
* Map keys nullable.
* Map duplicate-key behavior undocumented.
* Using Map for fixed hot fields.
* Missing `return_field_from_args` for nested output.
* Runtime `StructArray` child fields differ from planner `Field`.
* Child builder lengths differ from parent length.
* Output schema changes without new function version.
* No `arrow_typeof` tests.
* No nested Parquet/Arrow round-trip tests.
* No tests for missing map key / missing struct field / unequal vector length.

---

# C10.15 Agent checklist

```text id="c10-final-checklist"
[ ] Choose nested output shape: Struct, List, Map, FixedSizeList, or nested combination.
[ ] Prefer Struct for known fields.
[ ] Prefer Map for dynamic sparse keys.
[ ] Prefer FixedSizeList for fixed-dimension vectors/embeddings.
[ ] Prefer List for variable-length repeated values.
[ ] Implement return_field_from_args for nested outputs.
[ ] Define parent field name, type, nullability, metadata.
[ ] Define every child field name, type, nullability, metadata.
[ ] Separate parent nullability from child nullability.
[ ] Separate null list from empty list.
[ ] Separate null struct from struct with null children.
[ ] Define map absent-key vs present-null behavior.
[ ] Define duplicate map-key behavior.
[ ] Use stable diagnostic status/code fields.
[ ] Use column_by_name for struct input fields.
[ ] Validate vector dimensions.
[ ] Reject non-literal output-shape arguments at planning.
[ ] Build arrays with Arrow builders; append parent slot once per output row.
[ ] Validate runtime output type matches return Field.
[ ] Validate child lengths and parent length.
[ ] Add SQL bracket-access tests.
[ ] Add arrow_typeof tests.
[ ] Add unnest tests where relevant.
[ ] Add Arrow/Parquet round-trip tests for materialized outputs.
[ ] Version nested return schemas.
```

[1]: https://datafusion.apache.org/user-guide/sql/scalar_functions.html?utm_source=chatgpt.com "Scalar Functions — Apache DataFusion documentation"
[2]: https://docs.rs/datafusion-expr/latest/datafusion_expr/trait.ScalarUDFImpl.html?utm_source=chatgpt.com "ScalarUDFImpl in datafusion_expr - Rust"
[3]: https://docs.rs/arrow/latest/arrow/array/struct.StructBuilder.html?utm_source=chatgpt.com "StructBuilder in arrow::array - Rust"
[4]: https://docs.rs/arrow/latest/arrow/array/builder/struct.GenericListBuilder.html?utm_source=chatgpt.com "GenericListBuilder in arrow::array::builder - Rust"
[5]: https://docs.rs/arrow/latest/arrow/array/struct.MapArray.html?utm_source=chatgpt.com "MapArray in arrow::array - Rust"
[6]: https://docs.rs/datafusion/latest/datafusion/common/arrow/array/builder/struct.MapBuilder.html "MapBuilder in datafusion::common::arrow::array::builder - Rust"
[7]: https://docs.rs/arrow/latest/arrow/array/struct.FixedSizeListArray.html?utm_source=chatgpt.com "FixedSizeListArray in arrow::array - Rust"
[8]: https://docs.rs/arrow/latest/arrow/array/builder/struct.FixedSizeListBuilder.html?utm_source=chatgpt.com "FixedSizeListBuilder in arrow::array::builder - Rust"
[9]: https://datafusion.apache.org/user-guide/sql/struct_coercion.html?utm_source=chatgpt.com "Struct Type Coercion and Field Mapping - Apache DataFusion"
[10]: https://datafusion.apache.org/user-guide/sql/data_types.html?utm_source=chatgpt.com "Data Types — Apache DataFusion documentation"
[11]: https://datafusion.apache.org/user-guide/sql/special_functions.html?utm_source=chatgpt.com "Special Functions — Apache DataFusion documentation"



# DataFusion Advanced — C11) External-library integration strategy

## C11.0 Objective

Define a **safe integration decision framework** for non-DataFusion calculation libraries:

```text id="c11-root"
External calculation need
  ├─ pure Rust crate inside scalar UDF / UDAF / UDWF
  ├─ Arrow compute kernel / Arrow-native custom kernel
  ├─ DataFusion Python UDF/UDAF
  ├─ PyO3 bridge inside Rust service
  ├─ subprocess / microservice via async UDF
  ├─ WASM plugin
  ├─ precompute outside DataFusion
  └─ TableProvider over external engine
```

The attachment already covers DataFusion Python as a Python binding to Rust DataFusion with SQL/DataFrame APIs, CSV/Parquet/in-memory support, multithreaded Rust execution, PyArrow/Pandas interoperability, and Python UDF/UDAF support.  It also distinguishes Python ergonomics from Rust-core extension depth, noting that Rust is preferred for custom `TableProvider`, `ExecutionPlan`, optimizer rules, resource governance, and high-throughput service internals. 

Current DataFusion docs classify UDF families as scalar, window, aggregate, table, and async scalar functions; async scalar UDFs are specifically useful for async work such as network or I/O calls.  DataFusion Python UDF docs state that scalar Python UDFs take one or more PyArrow arrays and return a single array; Python UDFs operate on batches, though their logical evaluation is row-wise. ([Apache DataFusion][1])

---

## C11.1 Core principle

```text id="c11-principle"
Prefer the integration route with the least runtime boundary crossing that still satisfies:
  correctness
  reproducibility
  deployment simplicity
  performance
  safety
  observability
  packaging
  tenant/resource isolation
```

Default route order:

```text id="route-order"
1. Expr / built-ins / Arrow compute kernel
2. Pure Rust crate inside UDF/UDAF/UDWF
3. Rust TableProvider / physical operator
4. Precompute outside DataFusion and query materialized results
5. DataFusion Python for notebook/ETL/Python-owned workflows
6. Subprocess/microservice for Python/SciPy/SymPy/external engines in production
7. WASM for sandboxed pure plugins
8. PyO3 in-process only for trusted/internal compatibility
9. Dynamic native plugins only with explicit ABI/version/signing/sandboxing
```

Primary anti-pattern:

```text id="anti-primary"
Do not embed arbitrary heavy external runtimes inside hot scalar UDF paths.
Do not call Python/SciPy/SymPy row-by-row from Rust DataFusion execution.
Do not let external-library convenience override Arrow batch semantics.
```

---

# C11.2 Integration route matrix

| Route                              |          Latency |         Throughput |         Copy cost |         Safety |   Packaging | Best use                               | Avoid                         |
| ---------------------------------- | ---------------: | -----------------: | ----------------: | -------------: | ----------: | -------------------------------------- | ----------------------------- |
| pure Rust crate inside UDF         |           lowest |               high |               low |           high |      medium | production kernels, deterministic math | missing Rust implementation   |
| Arrow compute kernel               |           lowest |            highest |               low |           high |      medium | vectorized columnar operations         | bespoke algorithm absent      |
| DataFusion Python UDF              |      medium/high |             medium |     PyArrow batch |         medium |      Python | notebooks, ETL, Python-first logic     | high-throughput Rust services |
| PyO3 in Rust service               |           medium |         medium/low |   Python boundary |     low/medium |        high | trusted internal compatibility         | public/multi-tenant/hot paths |
| subprocess/microservice async UDF  |             high |             medium | serialization/RPC | high isolation |        high | SciPy/SymPy/LLM/external service       | cheap per-row math            |
| WASM plugin                        |           medium |             medium |   host/guest copy |      sandboxed |        high | tenant/plugin formulas                 | heavy SciPy-like libraries    |
| external preprocessing             |    batch/offline | high at query time |   materialization |           high | medium/high | expensive/global/reused calculations   | ad hoc query-specific logic   |
| TableProvider over external engine | source-dependent |   source-dependent |   stream boundary |    medium/high |        high | external solver/model/result source    | scalar row transforms         |

Arrow’s C Data Interface exists to expose an ABI-stable interface and allow zero-copy sharing of Arrow data between independent runtimes/components in the same process; use Arrow-native interchange wherever crossing language/runtime boundaries is necessary. ([Apache Arrow][2])

---

# C11.3 Route selection decision tree

```text id="decision-tree"
START: external calculation requirement

1. Can DataFusion built-ins / Expr / Arrow compute do it?
   ├─ yes → use Expr / built-in / Arrow compute kernel
   └─ no → continue

2. Is there a production-grade Rust crate or simple Rust implementation?
   ├─ yes:
   │   ├─ scalar-per-row → scalar UDF
   │   ├─ group state → UDAF
   │   ├─ window frame state → UDWF
   │   ├─ table/source → TableProvider / UDTF
   │   └─ custom stream algorithm → ExecutionPlan
   └─ no → continue

3. Is calculation expensive/global/reused across queries?
   ├─ yes → precompute/materialize outside DataFusion
   └─ no → continue

4. Is Python/SciPy/SymPy only needed for development/reference validation?
   ├─ yes → DataFusion Python or offline differential harness
   └─ no → continue

5. Is Python/SciPy/SymPy required in production?
   ├─ high throughput / service / multi-tenant:
   │     → subprocess/microservice over Arrow IPC/Flight/gRPC
   ├─ trusted internal low-volume:
   │     → PyO3 bridge or Python sidecar
   └─ no → continue

6. Is third-party/user plugin code required?
   ├─ pure deterministic plugin:
   │     → WASM sandbox
   ├─ native trusted internal plugin:
   │     → static linking preferred; dynamic loading only with signing/sandbox
   └─ arbitrary native user code:
         → reject

7. Does external engine naturally expose a relation/table?
   ├─ yes → custom TableProvider
   └─ no → async scalar UDF or preprocessing, depending cost/isolation
```

---

# C11.4 Decision criteria deep dive

## C11.4.1 Latency

```text id="latency-classes"
sub-millisecond:
  Expr / Arrow compute / pure Rust UDF

millisecond:
  moderate Rust numerical crate
  WASM plugin with warm instance
  in-process PyO3 small batch

10ms+:
  subprocess/microservice
  network reference data
  Python/SciPy sidecar
  external solver call

seconds/minutes:
  optimization solve
  symbolic simplification
  simulation
  large interpolation table generation
```

Agent rule:

```text id="latency-agent"
Do not put seconds/minutes algorithms inside scalar UDF execution.
Use preprocessing, TableProvider, UDTF, or async workflow with explicit resource policy.
```

---

## C11.4.2 Throughput

```text id="throughput-rule"
High-throughput path:
  Arrow arrays in
  Arrow arrays out
  no Python object loops
  no per-row FFI
  no per-row RPC
  no per-row allocation-heavy conversion
```

DataFusion Python UDFs receive PyArrow arrays, but DataFusion’s own Python UDF performance guidance recommends using PyArrow compute functions rather than pure Python object loops, and suggests Rust UDFs when logic is not well represented by PyArrow compute. ([Apache DataFusion][3])

---

## C11.4.3 Determinism

| External route      | Determinism risk                          |
| ------------------- | ----------------------------------------- |
| pure Rust crate     | low if version-pinned                     |
| Arrow compute       | low                                       |
| Python UDF          | Python/package/env dependent              |
| PyO3                | Python/package/env + GIL/threading        |
| subprocess          | service version/config dependent          |
| WASM                | good if deterministic host imports        |
| external solver     | algorithm tolerance/random seed/threading |
| symbolic simplifier | version/canonicalization drift            |

Required manifest fields:

```yaml id="determinism-manifest"
determinism:
  deterministic: true
  external_state: false
  random_seed_required: false
  thread_count_sensitive: false
  tolerance:
    abs: 1.0e-10
    rel: 1.0e-8
  package_versions:
    rust_crate: faer=0.24
    python: scipy=...
```

---

## C11.4.4 Memory copy cost

```text id="copy-cost"
Best:
  Arrow-native arrays
  same-process Arrow C Data Interface
  zero-copy/lifetime-safe PyArrow boundary

Acceptable:
  Arrow IPC batch to subprocess
  Flight/gRPC batches
  columnar numeric Vec conversion per batch

Bad:
  per-row Python objects
  JSON serialization per row
  stringifying vectors
  copying entire table into Python for row-wise scalar UDF
```

Arrow defines a language-independent columnar memory format for flat and nested data organized for efficient analytic operations, and its memory format supports zero-copy reads without serialization overhead. ([Apache Arrow][4])

---

## C11.4.5 GIL behavior

PyO3 can be used to write native Python modules in Rust or embed Python in a Rust binary, which makes it technically viable for Rust↔Python integration. ([PyO3][5]) For DataFusion service internals, PyO3 introduces Python runtime/GIL/environment complexity; keep it isolated unless you have a strong compatibility reason.

Policy:

```text id="gil-policy"
If Python is required in a high-throughput server:
  prefer sidecar process
  batch Arrow data
  avoid per-row Python calls
  set timeouts
  isolate environment
  pin Python/package versions

If PyO3 in-process:
  internal/trusted only
  feature-gated
  benchmarked
  no public tenant code
  no unbounded Python imports in query path
```

---

## C11.4.6 Sandboxing and isolation

WASM is inherently sandboxed by design and must import functionality explicitly; Wasmtime’s security docs describe sandboxed execution as a main goal. ([Wasmtime][6]) Wasmtime also exposes fuel and resource-limiting mechanisms such as `Store::set_fuel` and `ResourceLimiter`-style controls. ([Wasmtime][7])

Isolation hierarchy:

```text id="isolation-hierarchy"
lowest isolation:
  pure Rust function in-process
  PyO3 in-process
  dynamic native plugin

medium:
  WASM sandbox
  restricted subprocess
  sidecar with limited credentials

highest:
  separate service/process/container
  no shared secrets
  explicit network/file permissions
  batch protocol
```

Agent rule:

```text id="sandbox-agent"
Untrusted or tenant-authored code must not run as arbitrary native code in the DataFusion process.
Use WASM or subprocess/service isolation.
```

---

# C11.5 Route deep dives

## C11.5.1 Pure Rust crate inside UDF

### Use when

```text id="pure-rust-use"
Rust implementation exists
calculation is deterministic
batch-native implementation possible
low latency / high throughput required
production service path
function is scalar/group/window/table but no external runtime needed
```

### Cargo

```toml id="pure-rust-cargo"
[dependencies]
datafusion = { workspace = true }
nalgebra = "0.34"
ndarray = "0.17"
faer = "0.24"
argmin = "0.11"
```

`nalgebra` is a general-purpose low-dimensional linear algebra library, especially oriented toward tools for computer graphics and physics; `ndarray` provides an N-dimensional container for general elements and numerics; `faer` is a Rust linear algebra library focused on high performance for medium/large matrices and decompositions; `argmin` is a Rust numerical optimization library with a consistent interface and backend-agnostic design that can use math backends such as `nalgebra` or `ndarray`. ([Docs.rs][8])

### Pattern

```rust id="pure-rust-pattern"
pub fn register_numeric_package(ctx: &SessionContext) -> datafusion::error::Result<()> {
    ctx.register_udf(ScalarUDF::from(VectorNormUdf::new()));
    ctx.register_udaf(weighted_avg_udaf());
    Ok(())
}
```

### Best practice

```text id="pure-rust-best"
Keep external Rust math crate behind a pure Arrow kernel wrapper:
  Arrow arrays -> Rust numeric structures only when needed
  compute per batch, not per row FFI
  return Arrow arrays
  benchmark conversion overhead
```

### Avoid

```text id="pure-rust-avoid"
using nalgebra/faer inside every row for tiny operations
allocating matrix objects per row
using optimization solvers inside scalar UDF for unbounded work
```

---

## C11.5.2 Arrow compute kernel

### Use when

```text id="arrow-compute-use"
operation exists as Arrow/DataFusion compute
simple cast/arithmetic/string/list/filter/take
maximum throughput needed
logic remains columnar
```

### Pattern

```rust id="arrow-compute-pattern"
use datafusion::arrow::{array::ArrayRef, compute};
use datafusion::common::{DataFusionError, Result};

pub fn cast_to_f64(input: &ArrayRef) -> Result<ArrayRef> {
    compute::cast(input.as_ref(), &DataType::Float64)
        .map_err(DataFusionError::from)
}
```

### Best practice

```text id="arrow-compute-best"
Prefer built-in DataFusion/Arrow kernels over UDF row loops.
Only create UDF wrapper if product needs custom function name/signature/policy.
```

---

## C11.5.3 Python UDF in DataFusion Python

### Use when

```text id="df-python-use"
Python is primary environment
notebook/script/ETL workflow
PyArrow/Pandas interop matters
Python UDF/UDAF sufficient
not a high-throughput Rust service extension
```

DataFusion Python is a Python binding to the Rust Arrow in-memory query engine; it supports SQL/DataFrame APIs over in-memory data, Parquet, or CSV, runs in a multithreaded environment, and allows UDFs/UDAFs. ([Apache DataFusion][9])

### Python UDF pattern

```python id="python-udf-pattern"
import pyarrow as pa
import pyarrow.compute as pc
from datafusion import SessionContext, udf, col

@udf(
    input_types=[pa.float64()],
    return_type=pa.float64(),
    volatility="immutable",
)
def normalize_api(api: pa.Array) -> pa.Array:
    # Prefer PyArrow compute/vectorized operations.
    return pc.multiply(api, 1.0)

ctx = SessionContext()
ctx.register_udf(normalize_api)

df = ctx.table("streams").select(
    col("stream_id"),
    normalize_api(col("api")).alias("api_norm"),
)
```

DataFusion Python’s UDF guide defines scalar UDFs as Python functions taking PyArrow arrays and returning one PyArrow array. ([Apache DataFusion][1])

### Agent rules

```text id="python-udf-agent"
Use DataFusion Python for Python-owned workflows.
Prefer PyArrow compute inside Python UDFs.
Avoid pure Python row loops.
Do not assume Python UDF design transfers directly to Rust service UDFs.
```

---

## C11.5.4 PyO3 bridge inside Rust service

### Use when

```text id="pyo3-use"
trusted internal application
must call existing Python logic from Rust
low/moderate volume
migration bridge while porting to Rust
in-process simplicity is more important than isolation
```

### Cargo

```toml id="pyo3-cargo"
[features]
python_bridge = ["dep:pyo3"]

[dependencies]
pyo3 = { version = "0.25", optional = true, features = ["auto-initialize"] }
```

PyO3 is Rust bindings to the Python interpreter and can be used to write native Python modules or run Python code/modules from Rust. ([Docs.rs][10])

### Bridge pattern

```rust id="pyo3-pattern"
#[cfg(feature = "python_bridge")]
pub struct PythonReferenceFunction {
    module: String,
    function: String,
}

#[cfg(feature = "python_bridge")]
impl PythonReferenceFunction {
    pub fn call_batch_f64(&self, values: &[f64]) -> datafusion::error::Result<Vec<f64>> {
        Python::with_gil(|py| {
            let module = py.import_bound(&self.module)
                .map_err(|e| DataFusionError::External(Box::new(e)))?;

            let func = module.getattr(&self.function)
                .map_err(|e| DataFusionError::External(Box::new(e)))?;

            let result = func.call1((values.to_vec(),))
                .map_err(|e| DataFusionError::External(Box::new(e)))?;

            result.extract::<Vec<f64>>()
                .map_err(|e| DataFusionError::External(Box::new(e)))
        })
    }
}
```

### Deployment policy

```text id="pyo3-policy"
PyO3 bridge:
  feature-gated
  disabled by default
  internal/trusted only
  benchmarked
  no public SQL
  no arbitrary imports from user SQL
  no per-row calls
  batch calls only
```

### Agent rules

```text id="pyo3-agent"
Treat PyO3 as a migration/compatibility bridge, not the default production kernel path.
Prefer subprocess sidecar when isolation, dependency complexity, or failure containment matters.
```

---

## C11.5.5 Subprocess / microservice via async UDF

### Use when

```text id="sidecar-use"
Python/SciPy/SymPy required in production
external engine has heavy dependencies
failure isolation required
runtime should not contain Python/GIL
calculation can be batched
latency budget allows RPC
```

### Architecture

```text id="sidecar-arch"
DataFusion async scalar UDF
  ├─ collects batch inputs / deduplicates keys
  ├─ serializes Arrow IPC / JSON / protobuf / Flight batch
  ├─ calls local sidecar or remote microservice
  ├─ timeout / retry / circuit breaker
  ├─ validates response row count/type/nullability
  └─ returns Arrow Array
```

### Async UDF skeleton

```rust id="async-udf-sidecar"
#[derive(Debug)]
pub struct SciPySidecarUdf {
    signature: Signature,
    client: Arc<dyn BatchSidecarClient>,
    timeout: Duration,
}

#[async_trait::async_trait]
impl AsyncScalarUDFImpl for SciPySidecarUdf {
    async fn invoke_async_with_args(
        &self,
        args: ScalarFunctionArgs,
        _options: &ConfigOptions,
    ) -> datafusion::error::Result<ArrayRef> {
        let request = build_sidecar_request(&args)?;
        let response = tokio::time::timeout(
            self.timeout,
            self.client.call(request),
        )
        .await
        .map_err(|_| DataFusionError::Execution("sidecar timeout".to_string()))??;

        validate_sidecar_response(&args.return_field, args.number_rows, &response)?;
        Ok(response.array)
    }
}
```

### Sidecar protocol options

| Protocol           | Use                           |
| ------------------ | ----------------------------- |
| Arrow IPC stream   | batch numeric/columnar data   |
| Arrow Flight       | high-throughput Arrow service |
| gRPC/protobuf      | stable service contract       |
| HTTP JSON          | low-volume diagnostics only   |
| Unix domain socket | local sidecar low overhead    |

### Agent rules

```text id="sidecar-agent"
Use async UDF for read-only, row-preserving external batch calls.
Never perform side-effecting writes from UDF.
Always enforce timeout, max batch size, max concurrency, retry budget, and response validation.
```

---

## C11.5.6 WASM plugin

### Use when

```text id="wasm-use"
third-party/user-authored pure calculations
sandboxing required
deterministic functions
no heavy SciPy-like dependency
lower performance acceptable
capability-controlled host imports
```

### Wasmtime integration sketch

```rust id="wasm-sketch"
pub struct WasmFunctionPlugin {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    fuel_per_batch: u64,
}

impl WasmFunctionPlugin {
    pub fn call_batch(&self, input: &[f64]) -> datafusion::error::Result<Vec<f64>> {
        let mut store = wasmtime::Store::new(&self.engine, ());
        store.set_fuel(self.fuel_per_batch)
            .map_err(|e| DataFusionError::External(Box::new(e)))?;

        // Instantiate module, copy batch into guest memory or use component ABI.
        // Call exported function.
        // Validate output length/type.
        todo!()
    }
}
```

### WASM policy

```text id="wasm-policy"
compile-time:
  wasmtime feature optional

runtime:
  signed plugin
  manifest validated
  fuel/time/memory limits
  no filesystem/network unless explicitly granted
  deterministic host imports
  tenant-specific allowlist
```

### Agent rules

```text id="wasm-agent"
Use WASM for sandboxed pure plugins, not for heavy Python scientific stacks.
Prefer Wasmtime fuel/resource limits and capability-based host imports.
Do not expose arbitrary host filesystem/network.
```

---

## C11.5.7 Precompute outside DataFusion

### Use when

```text id="precompute-use"
calculation is expensive
calculation is global across dataset
calculation is reused by many queries
calculation has side effects
calculation invokes optimizer/solver/simulator
calculation requires full dataset context
calculation needs Python/SciPy/SymPy but query-time latency matters
```

### Architecture

```text id="precompute-arch"
External batch job
  ├─ reads raw data
  ├─ invokes SciPy/SymPy/solver/simulator
  ├─ writes materialized Arrow/Parquet/Delta/Postgres table
  ├─ records formula/model/data version
  └─ DataFusion queries materialized result
```

### Materialization manifest

```yaml id="precompute-manifest"
materialized_calculation:
  name: assay_curve_coefficients
  engine: python_scipy
  version: 2026.05.24
  input_tables:
    - raw_assay_measurements
  output_table:
    location: s3://lake/curated/assay_coefficients/
    format: parquet
  reproducibility:
    python: "3.12"
    scipy: "..."
    sympy: "..."
    git_sha: "..."
  refresh_policy:
    cadence: daily
    trigger: source_data_changed
```

### Agent rules

```text id="precompute-agent"
If computation is heavy, global, iterative, or reused, precompute.
Then expose results through Parquet/TableProvider, not scalar UDF.
```

---

## C11.5.8 TableProvider over external engine

### Use when

```text id="provider-use"
external computation naturally returns a table/relation
external engine supports projection/filter/limit pushdown
external result set can be streamed as RecordBatch
query-time table abstraction is useful
```

### Examples

```text id="provider-examples"
solver_cases(case_set_id) table
simulation_results(case_id) table
interpolation_surface(surface_id) table
external_model_predictions(model_id, input_table) table
scenario_parameter_grid(scenario_id) table
```

### Provider skeleton

```rust id="provider-skeleton"
#[derive(Debug)]
pub struct ExternalEngineTableProvider {
    schema: SchemaRef,
    client: Arc<dyn ExternalEngineClient>,
}

#[async_trait::async_trait]
impl TableProvider for ExternalEngineTableProvider {
    // DataFusion 54: no `as_any` — `Any` is a supertrait of `TableProvider`.
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
    ) -> datafusion::error::Result<Arc<dyn ExecutionPlan>> {
        // 1. translate supported filters/projection/limit
        // 2. build custom ExecutionPlan that streams external batches
        // 3. preserve unsupported filters for DataFusion if needed
        todo!()
    }
}
```

The attachment’s custom-table-provider material describes `TableProvider` as the source abstraction that supplies schema, planning information, and an `ExecutionPlan`, and notes projection/filter/limit pushdown as a major optimization boundary. 

### Agent rules

```text id="provider-agent"
Use TableProvider when external computation/source is table-shaped.
Do not encode tables as scalar UDF JSON strings.
Expose projection/filter/limit pushdown early.
Avoid expensive external scans during planning.
```

---

# C11.6 SciPy/SymPy-style case taxonomy

## C11.6.1 Numeric kernels

Examples:

```text id="numeric-kernels"
special functions
interpolation evaluation
linear algebra transforms
signal/filter primitives
distance metrics
basic root function evaluation
```

Placement:

```text id="numeric-placement"
Arrow/DataFusion built-in if available
pure Rust crate inside UDF if stable/hot
Python UDF for notebook/ETL
sidecar/PyO3 only for compatibility
precompute if expensive/reused
```

Rust alternatives:

```text id="numeric-rust-alt"
nalgebra:
  small/medium linear algebra, vectors/matrices, geometry/physics orientation

ndarray:
  N-dimensional arrays and numeric containers

faer:
  high-performance medium/large matrix linear algebra and decompositions

argmin:
  optimization algorithms/framework, backend-agnostic over math backends
```

Citations: `nalgebra`, `ndarray`, `faer`, and `argmin` docs describe exactly these broad purposes. ([Docs.rs][8])

---

## C11.6.2 Root finding

Options:

```text id="root-finding-options"
cheap scalar root formula per row:
  Rust UDF if bounded and deterministic

iterative root solve per row:
  avoid unless bounded iteration and benchmarked
  consider preprocessing

root solve over group/dataset:
  UDAF only if state is mergeable
  otherwise external preprocessing / solver service

SciPy compatibility required:
  sidecar or PyO3 internal bridge
```

UDF policy:

```yaml id="root-finding-policy"
root_solve:
  max_iterations: 50
  tolerance_abs: 1.0e-10
  tolerance_rel: 1.0e-8
  failure_behavior: null_or_error
  deterministic: true
  benchmark_required: true
```

Agent rule:

```text id="root-agent"
Never allow unbounded iterative algorithms inside UDF.
Expose max iterations and tolerance as fixed config or scalar literals.
```

---

## C11.6.3 Interpolation

Options:

```text id="interpolation-options"
small static table:
  pure Rust UDF with embedded lookup arrays

large reference table:
  TableProvider / join / preprocessing

per-row interpolation over list inputs:
  scalar UDF over List<Float64> with dimension checks

SciPy-specific interpolation:
  precompute coefficients or sidecar service
```

Preferred architecture:

```text id="interp-architecture"
Fit interpolation model outside DataFusion.
Materialize coefficients/table.
Use Rust UDF only for cheap evaluation:
  evaluate_interpolant(x, coeff_struct) -> y
```

---

## C11.6.4 Symbolic simplification

SymPy-style simplification should usually not run during query execution.

Placement:

```text id="sympy-placement"
planning-time expression compiler:
  preprocess symbolic formulas into ExprSpec / Expr
  not per-row UDF

offline canonicalization:
  simplify formulas before deployment
  generate Rust/Expr/UDF code

runtime symbolic simplification:
  avoid in query path except internal tools
```

Agent rule:

```text id="sympy-agent"
SymPy belongs in build-time/authoring-time formula compilation more often than runtime UDF execution.
```

---

## C11.6.5 Expression compilation

Architecture:

```text id="expression-compile-arch"
Symbolic/domain formula
  ├─ parse into formula IR
  ├─ simplify/canonicalize outside DataFusion
  ├─ infer units/types/nullability
  ├─ compile to DataFusion Expr when possible
  ├─ compile to Rust Arrow kernel when needed
  └─ register generated UDF only if Expr is insufficient
```

Compile target matrix:

| Formula property          | Compile target         |
| ------------------------- | ---------------------- |
| arithmetic + conditionals | `Expr`                 |
| built-in funcs only       | `Expr` / SQL           |
| row-wise custom numeric   | scalar UDF             |
| group state               | UDAF                   |
| window state              | UDWF                   |
| table generation          | UDTF                   |
| heavy iterative           | external preprocessing |

---

## C11.6.6 Derivative / Jacobian / Hessian generation

Placement:

```text id="derivative-placement"
symbolic derivative generation:
  build-time / preprocessing

finite difference validation:
  test harness / offline diagnostics

per-row derivative evaluation:
  Rust UDF if formula fixed and cheap

Jacobian/Hessian table:
  materialized side table or TableProvider

optimization solver:
  external solver pipeline, not scalar UDF
```

Artifact pattern:

```yaml id="derivative-artifact"
derivative_artifact:
  formula_id: blend_quality_v3
  generated_by: sympy
  generated_at: 2026-05-24
  outputs:
    expr_spec: blend_quality_v3.expr.yaml
    rust_kernel: blend_quality_v3_kernel.rs
    jacobian_schema: jacobian_entries.parquet
  validation:
    finite_difference_samples: 1000
    tolerance_abs: 1.0e-7
```

---

# C11.7 Rust alternative mapping

## C11.7.1 Linear algebra

| Need                                          | Rust option | Placement                                                 |
| --------------------------------------------- | ----------- | --------------------------------------------------------- |
| small vectors/matrices                        | `nalgebra`  | scalar UDF / preprocessing                                |
| N-dimensional array manipulation              | `ndarray`   | preprocessing / UDF if batch conversion acceptable        |
| larger dense matrix operations/decompositions | `faer`      | preprocessing / TableProvider / careful UDF               |
| optimization algorithms                       | `argmin`    | preprocessing / solver service; bounded internal use only |

Citations for crate purposes: ([Docs.rs][8])

## C11.7.2 Suggested placement

```text id="rust-placement"
nalgebra:
  row-wise vector/geometry functions
  small fixed-dimensional calculations
  convert List/FixedSizeList to vector carefully

ndarray:
  batch-oriented numeric arrays
  preprocessing pipelines
  avoid per-row ndarray allocation in hot UDFs

faer:
  matrix factorization/decomposition
  expensive linear algebra
  likely preprocessing or table-valued results

argmin:
  optimization workflows
  external/precompute or TableProvider over solve results
  not scalar UDF unless tiny bounded solve
```

## C11.7.3 Conversion rule

```text id="conversion-rule"
Arrow List/FixedSizeList -> Rust numeric vector/matrix:
  zero-copy rarely trivial
  validate dimension
  avoid allocation per row when possible
  batch conversions preferred
  benchmark copy cost
```

---

# C11.8 Integration manifest

## C11.8.1 Manifest schema

```yaml id="integration-manifest"
external_integration:
  name: scipy_interpolation_bridge
  route: subprocess_async_udf
  owner: modeling_platform
  status: internal_only

  function_family: async_scalar_udf
  deterministic: true
  external_io: true
  side_effects: false

  runtime:
    language: python
    isolation: sidecar_process
    protocol: arrow_ipc
    timeout_ms: 5000
    max_concurrency: 8
    max_batch_rows: 8192
    retry_policy:
      max_retries: 1
      retry_on:
        - timeout
        - transient_5xx

  dependencies:
    python: "3.12"
    packages:
      scipy: "pinned"
      numpy: "pinned"
      pyarrow: "pinned"

  data_contract:
    input_arrow_schema: interpolation_request_schema
    output_arrow_schema: interpolation_response_schema
    row_count_preserved: true

  failure_semantics:
    timeout: execution_error
    malformed_response: internal_error
    invalid_input: null
    service_unavailable: execution_error

  security:
    public_sql_allowed: false
    tenant_scoped_credentials: true
    network_access: local_only
    secrets_required:
      - reference_service_token

  observability:
    metrics:
      - latency_ms
      - batch_rows
      - cache_hit
      - timeout_count
      - error_count
```

---

# C11.9 Deployment patterns

## C11.9.1 Production pure Rust package

```text id="pure-rust-deploy"
Cargo features:
  engineering
  no python_bridge
  no external_io unless needed

Runtime:
  register deterministic UDF/UDAF/UDWF packages
  run Arrow kernel tests
  run SQL integration tests
  benchmark hot paths
```

## C11.9.2 Python sidecar package

```text id="python-sidecar-deploy"
Service image:
  Rust DataFusion service
  Python sidecar image/version
  Arrow IPC/Flight/gRPC protocol

Config:
  sidecar endpoint
  timeout
  concurrency
  max batch rows
  tenant policy
  retry/circuit breaker

CI:
  contract tests
  differential tests against Python reference
  malformed response tests
  timeout tests
```

## C11.9.3 WASM plugin deployment

```text id="wasm-deploy"
Build:
  compile plugin to wasm/component
  sign plugin
  attach manifest
  validate imports/exports

Runtime:
  load only signed plugin
  configure fuel/time/memory limits
  deny filesystem/network by default
  register functions by policy
  validate output row counts/types
```

## C11.9.4 Precompute deployment

```text id="precompute-deploy"
Batch job:
  runs SciPy/SymPy/solver
  writes Parquet/Delta/Postgres
  emits lineage/version metadata
  produces validation report

DataFusion:
  registers output table
  queries materialized results
  no Python/SciPy in query path
```

---

# C11.10 Failure semantics

| Failure            | In-process Rust            | Python UDF       | PyO3                       | Sidecar                       | WASM               | Precompute          |
| ------------------ | -------------------------- | ---------------- | -------------------------- | ----------------------------- | ------------------ | ------------------- |
| invalid input      | null/error/audited         | null/error       | null/error                 | null/error                    | null/error         | rejected table      |
| panic/exception    | process risk unless caught | Python exception | Python exception/GIL state | contained by service boundary | trap               | job failure         |
| timeout            | custom                     | difficult        | difficult                  | explicit                      | fuel/epoch/time    | scheduler timeout   |
| dependency missing | compile/startup            | runtime/import   | startup/runtime            | sidecar startup               | plugin load        | job startup         |
| memory blowup      | process risk               | process risk     | process risk               | sidecar isolated              | resource limiter   | job/container limit |
| version mismatch   | compile/test               | env drift        | env drift                  | protocol/version check        | manifest ABI check | manifest version    |

Agent rule:

```text id="failure-agent"
External integration must declare failure semantics before implementation.
For production, prefer failure containment over convenience.
```

---

# C11.11 Observability

## C11.11.1 Metrics

```text id="integration-metrics"
external_calc_calls_total{function,route,status}
external_calc_rows_total{function,route}
external_calc_latency_ms{function,route}
external_calc_timeout_total{function}
external_calc_retry_total{function}
external_calc_batch_rows{function}
external_calc_response_bytes{function}
external_calc_cache_hits_total{function}
external_calc_cache_misses_total{function}
external_calc_invalid_rows_total{function,reason}
```

## C11.11.2 Trace fields

```json id="trace-fields"
{
  "function": "scipy_interpolate",
  "route": "sidecar_async_udf",
  "tenant": "internal_modeling",
  "query_id": "q_123",
  "batch_rows": 8192,
  "timeout_ms": 5000,
  "dependency_version": "scipy=...",
  "status": "ok"
}
```

---

# C11.12 Testing strategy

## C11.12.1 Route-specific tests

| Route                 | Required tests                                                                 |
| --------------------- | ------------------------------------------------------------------------------ |
| pure Rust UDF         | direct Arrow arrays, SQL, null/invalid, benchmark                              |
| Arrow compute         | output type/nullability, kernel equivalence                                    |
| DataFusion Python UDF | PyArrow array tests, SQL invocation, Pandas/PyArrow round-trip                 |
| PyO3                  | import failure, Python exception, batch size, GIL/concurrency, version pin     |
| sidecar               | timeout, retry, malformed response, row-count mismatch, service unavailable    |
| WASM                  | fuel exhaustion, memory limit, invalid export, bad manifest, output validation |
| preprocessing         | reproducibility, lineage, schema, row counts, materialization validation       |
| TableProvider         | projection/filter/limit pushdown, stream output, external failure              |

## C11.12.2 Differential validation

```text id="differential-validation"
Rust implementation vs Python reference:
  generate Arrow input batches
  compute Rust output
  compute Python/SciPy output
  compare exact/tolerance/null policy
  save failing rows
  snapshot dependency versions
```

## C11.12.3 Tolerance manifest

```yaml id="tolerance-manifest"
comparison:
  mode: numeric_tolerance
  abs_tol: 1.0e-10
  rel_tol: 1.0e-8
  nan_policy: equal_nan
  infinity_policy: exact
  null_policy: exact
```

---

# C11.13 Security policy

```text id="security-policy"
Public SQL:
  no PyO3
  no Python bridge
  no external_io unless explicitly productized
  no arbitrary WASM unless sandboxed and allowlisted
  no native dynamic plugins

Internal batch:
  pure Rust allowed
  sidecar allowed
  Python allowed if environment pinned
  preprocessing preferred for heavy work

Multi-tenant:
  tenant-scoped credentials
  no shared mutable external clients without tenant key
  function allowlist
  sidecar endpoint allowlist
  timeout/concurrency quotas
```

---

# C11.14 Anti-pattern inventory

* Per-row Python/SciPy calls from scalar UDF.
* PyO3 bridge enabled in public SQL service.
* External service call inside synchronous scalar UDF.
* Solver/root-finder with unbounded iterations inside UDF.
* Symbolic simplification during row-wise execution.
* Large model/simulation run inside `invoke_with_args`.
* JSON serialization of Arrow batches for high-throughput sidecar.
* Sidecar response not validating row count.
* Sidecar timeout missing.
* Python package versions unpinned.
* GIL-heavy path used as hot production kernel.
* WASM plugin with filesystem/network imports by default.
* Arbitrary user native `.so` plugin loading.
* Decimal exact calculation silently delegated to Float64 library.
* Precomputed external result has no lineage/version metadata.
* Table-shaped external result returned as scalar JSON.
* Differential tests absent when replacing SciPy/SymPy with Rust.
* No benchmark for external integration route.

---

# C11.15 Agent checklist

```text id="c11-final-checklist"
[ ] Check built-ins / Expr / Arrow compute first.
[ ] Prefer pure Rust crate inside UDF for deterministic hot-path production kernels.
[ ] Use Arrow compute kernels before custom row loops.
[ ] Use DataFusion Python for Python-owned notebooks/ETL, not Rust service internals by default.
[ ] Prefer PyArrow compute inside Python UDFs.
[ ] Use PyO3 only for trusted internal bridge use cases.
[ ] Prefer subprocess/sidecar for production SciPy/SymPy compatibility.
[ ] Use async UDF only for read-only/idempotent external batch calls.
[ ] Use WASM for sandboxed pure plugins.
[ ] Reject arbitrary user-supplied native code.
[ ] Precompute heavy/global/reused/iterative calculations outside DataFusion.
[ ] Use TableProvider for external engines that produce relations.
[ ] Declare latency, throughput, determinism, copy cost, packaging, isolation, failure semantics.
[ ] Pin every external dependency version.
[ ] Batch all runtime boundary crossings.
[ ] Avoid per-row FFI/RPC/Python object loops.
[ ] Validate output row count, schema, type, nullability.
[ ] Add timeout, retry, circuit breaker, max batch rows, max concurrency for sidecars.
[ ] Add fuel/time/memory limits for WASM.
[ ] Add differential tests against SciPy/SymPy references when porting to Rust.
[ ] Add observability metrics for every external route.
[ ] Store lineage/version metadata for precomputed outputs.
```

[1]: https://datafusion.apache.org/python/user-guide/common-operations/udf-and-udfa.html?utm_source=chatgpt.com "User-Defined Functions - Apache DataFusion"
[2]: https://arrow.apache.org/docs/format/CDataInterface.html?utm_source=chatgpt.com "The Arrow C data interface — Apache Arrow v24.0.0"
[3]: https://datafusion.apache.org/blog/2024/11/19/datafusion-python-udf-comparisons/?utm_source=chatgpt.com "Comparing approaches to User Defined Functions in Apache ..."
[4]: https://arrow.apache.org/?utm_source=chatgpt.com "Apache Arrow | Apache Arrow"
[5]: https://pyo3.rs/?utm_source=chatgpt.com "Introduction - PyO3 user guide"
[6]: https://docs.wasmtime.dev/security.html?utm_source=chatgpt.com "Security"
[7]: https://docs.wasmtime.dev/api/wasmtime/struct.Store.html?utm_source=chatgpt.com "Store in wasmtime - Rust"
[8]: https://docs.rs/nalgebra/latest/nalgebra/?utm_source=chatgpt.com "nalgebra - Rust"
[9]: https://datafusion.apache.org/python/?utm_source=chatgpt.com "DataFusion in Python"
[10]: https://docs.rs/pyo3?utm_source=chatgpt.com "pyo3 - Rust"


# DataFusion Advanced — C12) Async UDFs for external I/O and services

## C12.0 Objective

Make DataFusion async scalar UDFs safe for production external I/O:

```text id="c12-root"
AsyncScalarUDFImpl
  ├─ row-preserving scalar UDF contract
  ├─ async execution path
  ├─ external service/client integration
  ├─ batch sizing
  ├─ request deduplication
  ├─ timeout / retry / circuit breaker
  ├─ cache policy
  ├─ tenant credential scope
  ├─ resource budgets
  ├─ cancellation semantics
  ├─ observability
  └─ deterministic error/fallback behavior
```

The attachment already introduces async scalar UDFs, including `AsyncScalarUDFImpl`, `AsyncScalarUDF::new`, `into_scalar_udf`, `ideal_batch_size`, and a skeleton showing async registration through `ctx.register_udf(udf.into_scalar_udf())`; it also states deployment rules: use async UDFs only for real async work, batch external calls, avoid one request per row, implement timeout/retry/circuit-breaker policy, mark external-state functions volatile unless stronger semantics are guaranteed, and cache carefully. 

Current DataFusion docs define `AsyncScalarUDFImpl` as a scalar UDF trait invoked using async methods, with required `invoke_async_with_args(args: ScalarFunctionArgs) -> Future<Output = Result<ColumnarValue, DataFusionError>>` and optional `ideal_batch_size() -> Option<usize>`; docs note async scalar UDFs are less efficient than ordinary `ScalarUDFImpl`, but useful for registering remote functions. ([Docs.rs][1]) Current docs for `AsyncScalarUDF` show `new(inner: Arc<dyn AsyncScalarUDFImpl>)`, `ideal_batch_size`, `invoke_async_with_args`, and `into_scalar_udf`, where `into_scalar_udf` turns the async UDF into a `ScalarUDF` suitable for context registration. ([Docs.rs][2])

---

## C12.1 Core principle

```text id="c12-principle"
Async UDFs are for bounded, row-preserving, read-oriented, async enrichment.

They are not:
  general workflow engines
  side-effect sinks
  transaction processors
  unbounded web scrapers
  arbitrary external computation launchers
  per-row network-call convenience wrappers
```

Use async scalar UDF when:

```text id="async-use"
calculation is scalar-per-row
external operation is read-only or logically read-only
query output cardinality equals input cardinality
batch calls are possible
latency is acceptable
timeout/cancellation/failure semantics are defined
tenant credentials can be scoped
function can be disabled by policy
```

Avoid async scalar UDF when:

```text id="async-avoid"
external operation mutates state
operation charges money / sends email / writes ticket / changes database
operation is long-running solve/simulation
operation naturally returns a table
operation needs transaction/commit semantics
operation should be materialized once and reused
operation cannot be bounded by timeout/concurrency/budget
```

---

# C12.2 Async execution model

## C12.2.1 Logical contract

```text id="async-logical-contract"
Async scalar UDF:
  input: ColumnarValue args for a batch/sub-batch
  output: ColumnarValue
  output row count: equal to logical input row count
  output order: same row order as input
  side effects: none
  failure: DataFusionError or policy-defined null/default/audited output
```

DataFusion’s scalar UDF guide states scalar UDFs are row-wise functions but vectorized in execution: they receive one or more Arrow arrays and produce an Arrow array with the same row count; async scalar UDFs are listed as scalar functions for async operations such as network or I/O calls. ([Apache DataFusion][3])

## C12.2.2 Physical execution risk

```text id="async-risk"
Async UDF call site may be invoked:
  multiple times per query
  per partition
  per RecordBatch
  per ideal_batch_size chunk
  concurrently across query tasks
  with scalar constants or arrays
  under cancellation when result stream is dropped
```

Deployment implication:

```text id="async-deploy-implication"
External service must tolerate:
  duplicate requests
  concurrent requests
  partial query cancellation
  retries
  batch fragmentation
  out-of-order batch execution across partitions
```

## C12.2.3 `ideal_batch_size`

`ideal_batch_size()` controls how much data should be evaluated at once; if it returns `None`, the whole batch is evaluated at once. ([Docs.rs][1])

```rust id="ideal-batch-size"
#[async_trait::async_trait]
impl AsyncScalarUDFImpl for LookupPriceUdf {
    fn ideal_batch_size(&self) -> Option<usize> {
        Some(256)
    }

    async fn invoke_async_with_args(
        &self,
        args: ScalarFunctionArgs,
    ) -> datafusion::error::Result<ColumnarValue> {
        todo!()
    }
}
```

### Batch size selection

| External call shape                            | Suggested `ideal_batch_size` |
| ---------------------------------------------- | ---------------------------: |
| local memory cache lookup                      |        `None` or large batch |
| local sidecar over UDS                         |                     512–8192 |
| remote HTTP lookup                             |                       32–512 |
| expensive service call                         |                        8–128 |
| LLM/API call                                   |                         1–32 |
| service has strict payload limit               |        derive from max bytes |
| service charges per request but supports batch |             as large as safe |

Agent rule:

```text id="ideal-batch-agent"
ideal_batch_size is a service-integration control, not a query-result batch-size guarantee.
Tune from latency, payload size, rate limits, service concurrency, cache hit ratio, and memory footprint.
```

---

# C12.3 Async UDF implementation skeleton

## C12.3.1 Minimal structure

```rust id="minimal-async-udf"
use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use datafusion::arrow::{
    array::{ArrayRef, Float64Array, Float64Builder, StringArray},
    datatypes::DataType,
};
use datafusion::common::{internal_err, plan_err, DataFusionError, Result, ScalarValue};
use datafusion::logical_expr::{
    async_udf::{AsyncScalarUDF, AsyncScalarUDFImpl},
    ColumnarValue, ScalarFunctionArgs, ScalarUDF, ScalarUDFImpl,
    Signature, Volatility,
};

#[derive(Debug)]
pub struct LookupPriceUdf {
    signature: Signature,
    client: Arc<dyn PriceClient>,
    policy: Arc<ExternalCallPolicy>,
}

impl LookupPriceUdf {
    pub fn new(client: Arc<dyn PriceClient>, policy: Arc<ExternalCallPolicy>) -> Self {
        Self {
            signature: Signature::uniform(
                2,
                vec![DataType::Utf8],
                Volatility::Volatile,
            ),
            client,
            policy,
        }
    }

    pub fn into_registered_udf(self) -> ScalarUDF {
        AsyncScalarUDF::new(Arc::new(self)).into_scalar_udf()
    }
}

impl ScalarUDFImpl for LookupPriceUdf {
    fn name(&self) -> &str {
        "lookup_price"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        if arg_types.len() != 2 {
            return plan_err!("lookup_price expects 2 arguments");
        }

        Ok(DataType::Float64)
    }

    fn invoke_with_args(&self, _args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        internal_err!("lookup_price must be invoked through async UDF execution")
    }
}

#[async_trait]
impl AsyncScalarUDFImpl for LookupPriceUdf {
    fn ideal_batch_size(&self) -> Option<usize> {
        Some(self.policy.ideal_batch_rows)
    }

    async fn invoke_async_with_args(
        &self,
        args: ScalarFunctionArgs,
    ) -> Result<ColumnarValue> {
        self.invoke_lookup_batch(args).await
    }
}
```

## C12.3.2 Registration

```rust id="async-registration"
pub fn register_external_udfs(
    ctx: &SessionContext,
    client: Arc<dyn PriceClient>,
    policy: Arc<ExternalCallPolicy>,
) -> datafusion::error::Result<()> {
    let udf = LookupPriceUdf::new(client, policy).into_registered_udf();
    ctx.register_udf(udf);
    Ok(())
}
```

---

# C12.4 External call semantics

## C12.4.1 Semantics matrix

| Function type                                    | Volatility          |      Cacheable | Retryable | Side effects | Public SQL |
| ------------------------------------------------ | ------------------- | -------------: | --------: | -----------: | ---------: |
| deterministic reference lookup with snapshot arg | `Immutable`         |            yes |       yes |           no |      maybe |
| query-snapshot lookup                            | `Stable`            |      per query |       yes |           no |      maybe |
| live price / live status                         | `Volatile`          | short TTL only |     maybe |           no |   restrict |
| LLM enrichment                                   | `Volatile`          |          maybe |     maybe |           no |   restrict |
| remote validation                                | `Stable`/`Volatile` |          maybe |       yes |           no |   internal |
| external write / ticket / email / charge         | not UDF             |             no | dangerous |          yes |         no |

### Volatility rule

```text id="volatility-rule"
External mutable state usually means Volatile.
It is only Immutable if every state version that can affect output is an explicit input argument.
It is only Stable if a query-level snapshot/version is fixed for the duration of execution.
```

## C12.4.2 Deterministic cacheable lookup

```text id="cacheable-lookup"
lookup_curve_value(curve_id, curve_version, x) -> y

cache key:
  tenant_id
  function_name
  curve_id
  curve_version
  x_canonical
```

Good properties:

```text id="cacheable-properties"
explicit version
idempotent
retry-safe
cacheable
deterministic under same inputs
```

## C12.4.3 Volatile live API lookup

```text id="volatile-lookup"
lookup_live_market_price(symbol) -> price

cache key:
  tenant_id
  function_name
  symbol
  time_bucket? optional
```

Required policy:

```text id="volatile-policy"
Volatility::Volatile
short TTL or no cache
public SQL disabled by default
audit usage
timeouts mandatory
query reproducibility warning
```

## C12.4.4 Idempotent vs side-effecting

```text id="idempotence"
Idempotent read:
  safe for retry
  safe for duplicate execution
  safe under partition re-execution
  eligible for async UDF

Side effect:
  unsafe for retry
  unsafe for duplicate execution
  must not be async scalar UDF
```

Side-effect examples:

```text id="side-effect-examples"
send_email(row)
create_ticket(row)
charge_account(row)
write_remote_record(row)
update_model_state(row)
```

Agent rule:

```text id="side-effect-agent"
If function changes external state, it does not belong in DataFusion scalar UDF execution.
Use a workflow engine, sink, or explicit DML/write path with commit semantics.
```

---

# C12.5 Authentication and secret handling

## C12.5.1 Credential model

```text id="credential-model"
Credentials belong to:
  tenant runtime config
  external client construction
  secret resolver
  credential provider

Credentials do not belong to:
  SQL literals
  function arguments
  logged query text
  global static variables
  shared cross-tenant caches
```

The attachment’s session/runtime guidance states credential scope belongs with object-store/provider construction rather than being scattered through query strings, and tenant isolation should use separate contexts/runtimes when credentials differ. 

## C12.5.2 Client construction

```rust id="client-construction"
pub struct TenantExternalClients {
    pub price_client: Arc<dyn PriceClient>,
}

pub fn build_tenant_external_clients(
    tenant: &TenantConfig,
    secrets: &dyn SecretResolver,
) -> datafusion::error::Result<TenantExternalClients> {
    let token = secrets.resolve(&tenant.price_service_secret_ref)?;

    Ok(TenantExternalClients {
        price_client: Arc::new(HttpPriceClient::new(
            tenant.price_service_endpoint.clone(),
            token,
        )),
    })
}
```

## C12.5.3 Redaction

```text id="redaction"
Never log:
  bearer tokens
  API keys
  signed URLs
  raw auth headers
  secret references if sensitive

Log:
  tenant id
  endpoint logical name
  function name
  request id
  batch row count
  status code class
  latency
```

---

# C12.6 Batch extraction and row mapping

## C12.6.1 Preserve row order

```text id="row-order"
External batch call may reorder keys internally.
UDF output must preserve input row order.
Use row index mapping:
  row -> key
  key -> response
  row -> output value
```

## C12.6.2 Input extraction

```rust id="extract-keys"
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PriceLookupKey {
    pub tenant_id: String,
    pub symbol: String,
    pub date: String,
}

pub fn extract_price_keys(
    args: &ScalarFunctionArgs,
    tenant_id: &str,
) -> Result<Vec<Option<PriceLookupKey>>> {
    if args.args.len() != 2 {
        return plan_err!("lookup_price expects symbol and date");
    }

    let arrays = ColumnarValue::values_to_arrays(&args.args)?;

    let symbols = arrays[0].as_string::<i32>();
    let dates = arrays[1].as_string::<i32>();

    if symbols.len() != dates.len() {
        return internal_err!("lookup_price input length mismatch");
    }

    let mut keys = Vec::with_capacity(symbols.len());

    for row in 0..symbols.len() {
        if symbols.is_null(row) || dates.is_null(row) {
            keys.push(None);
            continue;
        }

        keys.push(Some(PriceLookupKey {
            tenant_id: tenant_id.to_string(),
            symbol: symbols.value(row).to_string(),
            date: dates.value(row).to_string(),
        }));
    }

    Ok(keys)
}
```

## C12.6.3 Deduplicate keys

```rust id="deduplicate-keys"
pub fn unique_keys(keys: &[Option<PriceLookupKey>]) -> Vec<PriceLookupKey> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();

    for key in keys.iter().flatten() {
        if seen.insert(key.clone()) {
            out.push(key.clone());
        }
    }

    out
}
```

## C12.6.4 Reconstruct row-aligned output

```rust id="reconstruct-output"
pub fn build_price_output(
    row_keys: &[Option<PriceLookupKey>],
    values: &std::collections::HashMap<PriceLookupKey, Option<f64>>,
) -> Result<Float64Array> {
    let mut b = Float64Builder::with_capacity(row_keys.len());

    for key in row_keys {
        match key {
            None => b.append_null(),
            Some(k) => match values.get(k) {
                Some(Some(v)) => b.append_value(*v),
                Some(None) => b.append_null(),
                None => {
                    return exec_err!("lookup_price missing response for key {:?}", k);
                }
            },
        }
    }

    Ok(b.finish())
}
```

---

# C12.7 Timeout, retry, circuit breaker

## C12.7.1 Policy struct

```rust id="external-call-policy"
#[derive(Debug, Clone)]
pub struct ExternalCallPolicy {
    pub ideal_batch_rows: usize,
    pub timeout: std::time::Duration,
    pub max_retries: usize,
    pub retry_backoff_base: std::time::Duration,
    pub max_concurrent_requests: usize,
    pub max_requests_per_query: usize,
    pub max_unique_keys_per_batch: usize,
    pub failure_behavior: FailureBehavior,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureBehavior {
    Error,
    Null,
    Default,
    AuditedStruct,
}
```

## C12.7.2 Timeout wrapper

```rust id="timeout-wrapper"
pub async fn call_with_timeout<T, F>(
    timeout: Duration,
    fut: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| DataFusionError::Execution("external UDF timeout".to_string()))?
}
```

## C12.7.3 Retry wrapper

```rust id="retry-wrapper"
pub async fn retry_external<T, Fut, MakeFut>(
    max_retries: usize,
    base_backoff: Duration,
    mut make_call: MakeFut,
) -> Result<T>
where
    MakeFut: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0usize;

    loop {
        match make_call().await {
            Ok(v) => return Ok(v),
            Err(e) if attempt < max_retries && is_retryable(&e) => {
                let sleep = base_backoff * (1 << attempt as u32);
                tokio::time::sleep(sleep).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_retryable(e: &DataFusionError) -> bool {
    let msg = e.to_string();
    msg.contains("timeout")
        || msg.contains("429")
        || msg.contains("503")
        || msg.contains("connection")
}
```

## C12.7.4 Circuit breaker state

```rust id="circuit-breaker"
#[derive(Debug)]
pub struct CircuitBreaker {
    failures: std::sync::atomic::AtomicU64,
    opened_until_ms: std::sync::atomic::AtomicU64,
    failure_threshold: u64,
    open_duration: Duration,
}

impl CircuitBreaker {
    pub fn check(&self, now_ms: u64) -> Result<()> {
        let opened_until = self.opened_until_ms.load(std::sync::atomic::Ordering::Relaxed);

        if now_ms < opened_until {
            return exec_err!("external service circuit breaker is open");
        }

        Ok(())
    }

    pub fn record_success(&self) {
        self.failures.store(0, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn record_failure(&self, now_ms: u64) {
        let failures = self.failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

        if failures >= self.failure_threshold {
            let until = now_ms + self.open_duration.as_millis() as u64;
            self.opened_until_ms.store(until, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
```

## C12.7.5 Agent rules

```text id="resilience-agent-rules"
Timeout is mandatory.
Retry only idempotent calls.
Circuit breaker is mandatory for shared external dependencies.
Retry budget must be small; DataFusion may already execute many batches/partitions.
Never infinite-retry inside query execution.
```

---

# C12.8 Concurrency and rate controls

## C12.8.1 Semaphore

```rust id="semaphore"
#[derive(Debug)]
pub struct SharedExternalLimiter {
    semaphore: tokio::sync::Semaphore,
}

impl SharedExternalLimiter {
    pub fn new(max_concurrent_requests: usize) -> Self {
        Self {
            semaphore: tokio::sync::Semaphore::new(max_concurrent_requests),
        }
    }

    pub async fn acquire(&self) -> Result<tokio::sync::OwnedSemaphorePermit> {
        self.semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| DataFusionError::Execution("external limiter closed".to_string()))
    }
}
```

## C12.8.2 Query budget

```rust id="query-budget"
#[derive(Debug)]
pub struct QueryExternalBudget {
    remaining_requests: std::sync::atomic::AtomicUsize,
    remaining_keys: std::sync::atomic::AtomicUsize,
}

impl QueryExternalBudget {
    pub fn reserve(&self, requests: usize, keys: usize) -> Result<()> {
        let prev_req = self.remaining_requests.fetch_sub(requests, Ordering::SeqCst);
        let prev_keys = self.remaining_keys.fetch_sub(keys, Ordering::SeqCst);

        if prev_req < requests || prev_keys < keys {
            return exec_err!(
                "external UDF budget exceeded: requests={}, keys={}",
                requests,
                keys
            );
        }

        Ok(())
    }
}
```

## C12.8.3 Rate limit dimensions

```text id="rate-dimensions"
per process
per tenant
per function
per external service
per query
per time window
per unique key
per request bytes
```

Agent rule:

```text id="rate-agent"
Concurrency semaphore protects service saturation.
Query budget protects runaway SQL plans.
Rate limiter protects external provider quota.
Use all three for public/multi-tenant deployments.
```

---

# C12.9 Caching architecture

## C12.9.1 Cache layers

| Cache                  | Scope              | Use                                     | Risk                        |
| ---------------------- | ------------------ | --------------------------------------- | --------------------------- |
| per-batch map          | one UDF call       | dedupe within batch                     | minimal                     |
| per-query cache        | query execution    | repeated keys across batches/partitions | query context plumbing      |
| global TTL cache       | process/service    | repeated lookups across queries         | staleness/tenant leakage    |
| tenant-scoped cache    | tenant             | shared tenant reuse                     | must include tenant/version |
| negative cache         | failures/not-found | reduce repeated misses                  | stale misses                |
| external service cache | sidecar/service    | centralize policy                       | operational complexity      |

## C12.9.2 Cache key schema

```rust id="cache-key"
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalCacheKey {
    pub tenant_id: String,
    pub function_name: String,
    pub function_version: String,
    pub external_dataset_version: Option<String>,
    pub args_fingerprint: String,
}
```

## C12.9.3 Cache value

```rust id="cache-value"
#[derive(Debug, Clone)]
pub enum CachedLookupValue {
    Hit(Option<f64>),          // Some(v) or known null/not-found
    NegativeNotFound,
    NegativeErrorClass(String),
}
```

## C12.9.4 Global TTL cache skeleton

```rust id="ttl-cache"
#[derive(Debug)]
pub struct TtlCache<K, V> {
    entries: dashmap::DashMap<K, CacheEntry<V>>,
}

#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    pub value: V,
    pub expires_at: std::time::Instant,
}

impl<K, V> TtlCache<K, V>
where
    K: Eq + std::hash::Hash,
    V: Clone,
{
    pub fn get(&self, key: &K) -> Option<V> {
        let entry = self.entries.get(key)?;
        if std::time::Instant::now() <= entry.expires_at {
            Some(entry.value.clone())
        } else {
            drop(entry);
            self.entries.remove(key);
            None
        }
    }

    pub fn insert(&self, key: K, value: V, ttl: Duration) {
        self.entries.insert(key, CacheEntry {
            value,
            expires_at: std::time::Instant::now() + ttl,
        });
    }
}
```

## C12.9.5 Cache correctness rules

```text id="cache-rules"
Cache key must include:
  tenant_id
  function name
  function semantic version
  external dataset/snapshot version if deterministic
  all normalized arguments
  auth scope if output depends on permission

Do not cache:
  side effects
  volatile live values beyond product-approved TTL
  errors that are likely transient unless negative TTL is short
```

## C12.9.6 Negative cache

```text id="negative-cache"
Use negative cache for:
  not found
  invalid symbol/id
  permission denied if tenant-scoped

Do not long-cache:
  timeout
  5xx
  connection reset
  rate limited responses unless retry-after honored
```

---

# C12.10 Full lookup flow

```text id="lookup-flow"
invoke_async_with_args
  ├─ validate arity/types
  ├─ extract row-aligned keys
  ├─ null inputs -> null outputs
  ├─ deduplicate non-null keys
  ├─ check per-batch map
  ├─ check per-query/global/tenant cache
  ├─ reserve request/key budget
  ├─ acquire semaphore
  ├─ circuit breaker check
  ├─ call external service with timeout
  ├─ retry idempotent retryable failures
  ├─ validate response completeness/types
  ├─ update cache
  ├─ rebuild row-aligned output array
  ├─ validate output length/type/nullability
  └─ emit metrics/traces
```

## C12.10.1 End-to-end skeleton

```rust id="end-to-end-lookup"
impl LookupPriceUdf {
    async fn invoke_lookup_batch(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let row_keys = extract_price_keys(&args, &self.policy.tenant_id)?;
        let unique = unique_keys(&row_keys);

        if unique.len() > self.policy.max_unique_keys_per_batch {
            return exec_err!(
                "lookup_price batch has {} unique keys; max is {}",
                unique.len(),
                self.policy.max_unique_keys_per_batch
            );
        }

        let mut resolved: HashMap<PriceLookupKey, Option<f64>> = HashMap::new();
        let mut misses = Vec::new();

        for key in &unique {
            if let Some(v) = self.policy.cache.get(key) {
                resolved.insert(key.clone(), v);
            } else {
                misses.push(key.clone());
            }
        }

        if !misses.is_empty() {
            self.policy.query_budget.reserve(1, misses.len())?;

            let _permit = self.policy.limiter.acquire().await?;

            self.policy.circuit_breaker.check(now_ms())?;

            let response = retry_external(
                self.policy.max_retries,
                self.policy.retry_backoff_base,
                || async {
                    call_with_timeout(
                        self.policy.timeout,
                        self.client.lookup_prices(misses.clone()),
                    ).await
                },
            )
            .await;

            match response {
                Ok(batch_result) => {
                    self.policy.circuit_breaker.record_success();

                    for (key, value) in batch_result {
                        self.policy.cache.insert(key.clone(), value, self.policy.cache_ttl);
                        resolved.insert(key, value);
                    }
                }
                Err(e) => {
                    self.policy.circuit_breaker.record_failure(now_ms());
                    return self.handle_external_error(e, row_keys.len());
                }
            }
        }

        let out = build_price_output(&row_keys, &resolved)?;
        Ok(ColumnarValue::Array(Arc::new(out) as ArrayRef))
    }
}
```

---

# C12.11 Fallback behavior

## C12.11.1 FailureBehavior

```rust id="fallback-enum"
#[derive(Debug, Clone)]
pub enum ExternalFailureBehavior {
    Error,
    Null,
    DefaultFloat64(f64),
    AuditedStruct,
}
```

## C12.11.2 Error fallback

```rust id="fallback-error"
fn handle_external_error(
    &self,
    e: DataFusionError,
    _rows: usize,
) -> Result<ColumnarValue> {
    match self.policy.failure_behavior {
        ExternalFailureBehavior::Error => Err(e),
        _ => self.handle_non_error_fallback(e, _rows),
    }
}
```

## C12.11.3 Null fallback

```rust id="fallback-null"
fn null_fallback_f64(rows: usize) -> ColumnarValue {
    let mut b = Float64Builder::with_capacity(rows);
    b.append_nulls(rows);
    ColumnarValue::Array(Arc::new(b.finish()) as ArrayRef)
}
```

## C12.11.4 Audited fallback

```text id="audited-fallback"
Return Struct<
  value: Float64 nullable,
  status: Utf8 non-null,       -- ok | external_error | timeout | rate_limited
  reason: Utf8 nullable,
  code: Utf8 nullable
>
```

Agent rule:

```text id="fallback-agent"
For analytics metrics, Error is usually safer than silent default.
For enrichment/optional attributes, Null or audited struct may be acceptable.
Default fallback must be explicitly product-approved.
```

---

# C12.12 Cancellation semantics

## C12.12.1 What cancellation means

```text id="cancellation"
DataFusion may stop polling output streams if:
  user cancels query
  result stream is dropped
  timeout at API layer
  upstream/downstream operator fails
```

The attachment’s execution sections state that dropping a DataFrame result stream aborts query execution and frees allocated resources. 

## C12.12.2 Async UDF design

```text id="cancellation-design"
External calls should be:
  cancellable by dropping future
  timeout-bounded
  idempotent
  safe if remote request completes after local cancellation
  free of externally visible side effects
```

## C12.12.3 Cancellation test pattern

```rust id="cancellation-test"
#[tokio::test]
async fn async_udf_cancels_without_leaking_permits() {
    let limiter = Arc::new(SharedExternalLimiter::new(1));
    let client = Arc::new(SlowMockClient::new(Duration::from_secs(60)));

    let udf = LookupPriceUdf::new(
        client,
        Arc::new(test_policy_with_limiter(limiter.clone())),
    );

    let fut = udf.invoke_async_with_args(test_args_with_rows(100));

    tokio::pin!(fut);

    tokio::select! {
        _ = &mut fut => panic!("should not finish"),
        _ = tokio::time::sleep(Duration::from_millis(10)) => {}
    }

    drop(fut);

    // Then acquire should not hang forever after cleanup.
    let _permit = tokio::time::timeout(Duration::from_secs(1), limiter.acquire())
        .await
        .expect("limiter acquire timed out")
        .expect("limiter failed");
}
```

Agent rule:

```text id="cancel-agent"
Never rely on async UDF destructor/drop to perform external rollback.
If rollback is required, the operation is not a scalar UDF.
```

---

# C12.13 Task scheduling and runtime posture

## C12.13.1 Runtime rule

```text id="runtime-rule"
Async UDF external calls must be non-blocking.
Do not use blocking HTTP/database clients inside async UDF.
Do not perform CPU-heavy work on async runtime.
Do not call tokio::task::block_in_place in query path.
```

The attachment’s runtime model notes DataFusion uses Tokio for plan execution, and warns that runtime/thread-pool choices matter under CPU-heavy query work and latency-sensitive network I/O. 

## C12.13.2 Blocking client anti-pattern

Bad:

```rust id="blocking-bad"
let response = reqwest::blocking::get(url)?; // bad inside async UDF
```

Good:

```rust id="async-good"
let response = self.http_client
    .get(url)
    .send()
    .await
    .map_err(|e| DataFusionError::External(Box::new(e)))?;
```

## C12.13.3 Dedicated sidecar/client runtime

```text id="dedicated-runtime"
Use separate runtime/process when:
  external client library blocks
  Python/GIL is involved
  high-latency external calls dominate
  service p99 must isolate query CPU from network I/O
```

---

# C12.14 Resource-control manifest

```yaml id="resource-manifest"
async_udf:
  name: lookup_price
  route: http_batch_lookup
  default_enabled: false

  batch:
    ideal_batch_rows: 256
    max_unique_keys_per_batch: 512
    max_payload_bytes: 1048576

  timeout:
    per_request_ms: 2000
    whole_query_ms: 30000

  retry:
    max_retries: 1
    backoff_ms: 100
    retry_on:
      - timeout
      - http_429
      - http_503

  circuit_breaker:
    failure_threshold: 10
    open_ms: 30000
    half_open_max_requests: 1

  concurrency:
    max_in_flight_per_process: 32
    max_in_flight_per_tenant: 4
    max_in_flight_per_query: 2

  budget:
    max_requests_per_query: 100
    max_unique_keys_per_query: 10000

  cache:
    scope: tenant
    ttl_ms: 300000
    negative_ttl_ms: 30000
    key_fields:
      - tenant_id
      - function_version
      - dataset_version
      - symbol
      - date

  failure_behavior:
    timeout: error
    remote_5xx: error
    not_found: null
    malformed_response: internal_error
```

---

# C12.15 SQL/API policy

## C12.15.1 Public SQL

```text id="public-sql-policy"
Default:
  async external UDFs disabled

Allow only when:
  read-only
  rate-limited
  tenant-scoped
  timeout-bounded
  row/batch budgeted
  audited
  cache semantics documented
```

## C12.15.2 Internal modeling

```text id="internal-policy"
Allow:
  reference data lookup
  metadata enrichment
  sidecar computation

Require:
  deterministic snapshot argument where reproducibility matters
  startup health check
  mock service tests
  explicit failure behavior
```

## C12.15.3 Batch ETL

```text id="etl-policy"
Prefer:
  preprocessing or TableProvider for large external enrichment
  async UDF for small lookup enrichment
  accepted/rejected materialization for missing/invalid remote responses
```

---

# C12.16 Testing matrix

## C12.16.1 Timeout

```rust id="timeout-test"
#[tokio::test]
async fn lookup_price_times_out() {
    let udf = test_udf_with_client(SlowMockClient::new(Duration::from_secs(60)))
        .with_timeout(Duration::from_millis(10));

    let err = udf
        .invoke_async_with_args(test_args_with_symbol("ABC"))
        .await
        .expect_err("expected timeout");

    assert!(err.to_string().contains("timeout"));
}
```

## C12.16.2 Remote error

```rust id="remote-error-test"
#[tokio::test]
async fn lookup_price_remote_503_errors_or_retries() {
    let client = MockPriceClient::sequence(vec![
        MockResponse::Http503,
        MockResponse::Ok(hashmap! {
            key("ABC", "2026-05-24") => Some(100.0),
        }),
    ]);

    let udf = test_udf_with_client(client).with_max_retries(1);

    let out = udf
        .invoke_async_with_args(test_args_with_symbol("ABC"))
        .await
        .expect("retry should succeed");

    assert_price_output(out, &[Some(100.0)]);
}
```

## C12.16.3 Cancellation

```text id="cancel-test-list"
[ ] future dropped before external response
[ ] semaphore permit released
[ ] no cache write after canceled response unless explicitly safe
[ ] client request cancellation propagated where supported
```

## C12.16.4 Partial failure

```text id="partial-failure-tests"
[ ] one key not found -> null / audited status
[ ] one key remote error in batch -> whole batch error OR per-row audited result
[ ] malformed response missing key -> execution/internal error
[ ] duplicate keys resolved once and expanded to all matching rows
```

## C12.16.5 Scalar/null propagation

```text id="null-propagation-tests"
[ ] scalar symbol + array date
[ ] array symbol + scalar date
[ ] scalar NULL -> scalar/array null
[ ] array null rows -> null output rows and no remote key
[ ] all-null batch -> no remote request
```

## C12.16.6 Batch-size sensitivity

```text id="batch-size-tests"
[ ] ideal_batch_size = 1
[ ] ideal_batch_size = 32
[ ] ideal_batch_size = None
[ ] output identical across batch sizes
[ ] request count changes as expected
[ ] cache hit ratio changes as expected
```

---

# C12.17 Observability

## C12.17.1 Metrics

```text id="metrics"
async_udf_invocations_total{function,tenant,status}
async_udf_batches_total{function,tenant}
async_udf_input_rows_total{function,tenant}
async_udf_unique_keys_total{function,tenant}
async_udf_external_requests_total{function,tenant,status}
async_udf_latency_ms{function,tenant,phase}
async_udf_timeout_total{function,tenant}
async_udf_retry_total{function,tenant,reason}
async_udf_circuit_open_total{function,tenant}
async_udf_cache_hit_total{function,tenant}
async_udf_cache_miss_total{function,tenant}
async_udf_budget_exceeded_total{function,tenant}
```

## C12.17.2 Trace fields

```json id="trace"
{
  "query_id": "q_123",
  "tenant_id": "tenant_a",
  "function": "lookup_price",
  "batch_rows": 256,
  "unique_keys": 87,
  "cache_hits": 62,
  "remote_keys": 25,
  "timeout_ms": 2000,
  "attempt": 1,
  "status": "ok"
}
```

## C12.17.3 Logging rules

```text id="logging-rules"
Log:
  function name
  tenant id
  query id
  batch rows
  unique key count
  latency
  error class
  retry count

Do not log:
  secrets
  tokens
  raw auth headers
  PII keys unless approved
  full request payloads by default
```

---

# C12.18 Deployment anti-patterns

* One HTTP request per input row.
* Blocking HTTP client inside async UDF.
* No timeout.
* Infinite retry.
* Retry for side-effecting remote operation.
* Async UDF used to write/mutate external system.
* Cache key missing tenant ID.
* Cache key missing external data snapshot/version.
* Global cache shared across tenants without tenant key.
* Volatile live API marked `Immutable`.
* Public SQL endpoint registers external async UDF by default.
* External credentials passed as SQL literals.
* Missing cancellation test.
* Missing batch-size sensitivity test.
* Missing output row-count validation.
* Missing malformed response test.
* Circuit breaker absent for shared external dependency.
* Rate limiter absent for paid/quota-limited API.
* Sidecar failure brings down query engine.
* Async UDF used for long-running simulation/solver instead of preprocessing/TableProvider.

---

# C12.19 Agent checklist

```text id="c12-final-checklist"
[ ] Use async UDF only for real async read/enrichment work.
[ ] Confirm operation is scalar-per-row and row-count preserving.
[ ] Confirm external call is read-only/idempotent.
[ ] Do not perform side effects.
[ ] Set correct volatility: Immutable only with explicit state/version input; otherwise Stable/Volatile.
[ ] Implement AsyncScalarUDFImpl and wrap with AsyncScalarUDF::new(...).into_scalar_udf().
[ ] Register before SQL/DataFrame planning.
[ ] Set ideal_batch_size from external service characteristics.
[ ] Batch external requests.
[ ] Deduplicate keys per batch.
[ ] Preserve input row order.
[ ] Skip remote calls for null input rows.
[ ] Add timeout.
[ ] Add retry only for idempotent retryable failures.
[ ] Add circuit breaker.
[ ] Add per-process/per-tenant/per-query concurrency limits.
[ ] Add query request/key budget.
[ ] Add tenant-scoped cache.
[ ] Include tenant/function/version/snapshot/args in cache key.
[ ] Add negative cache with short TTL for not-found cases.
[ ] Keep credentials out of SQL and logs.
[ ] Use async/non-blocking clients.
[ ] Validate response row count, type, nullability.
[ ] Define fallback behavior: error, null, default, audited struct.
[ ] Add timeout tests.
[ ] Add remote error tests.
[ ] Add cancellation tests.
[ ] Add partial failure tests.
[ ] Add scalar/null propagation tests.
[ ] Add batch-size sensitivity tests.
[ ] Emit metrics and trace fields.
[ ] Disable in public SQL unless explicitly policy-approved.
```

[1]: https://docs.rs/datafusion/latest/datafusion/logical_expr/async_udf/trait.AsyncScalarUDFImpl.html "AsyncScalarUDFImpl in datafusion::logical_expr::async_udf - Rust"
[2]: https://docs.rs/datafusion-expr/latest/datafusion_expr/async_udf/struct.AsyncScalarUDF.html "AsyncScalarUDF in datafusion_expr::async_udf - Rust"
[3]: https://datafusion.apache.org/library-user-guide/functions/adding-udfs.html "Adding User Defined Functions: Scalar/Window/Aggregate/Table Functions — Apache DataFusion  documentation"


# DataFusion Advanced — C13) Aggregate UDF state design

## C13.0 Objective

Make DataFusion aggregate UDFs **state-correct, mergeable, distributed-ready, memory-accountable, and testable**:

```text id="c13-root"
AggregateUDF / UDAF
  ├─ SQL aggregate call
  ├─ AggregateUDF metadata
  │    ├─ name / aliases
  │    ├─ signature
  │    ├─ return type / return field
  │    ├─ state fields / state types
  │    ├─ volatility / nullability
  │    └─ accumulator factory
  ├─ Accumulator
  │    ├─ update_batch(input arrays)
  │    ├─ state() -> Vec<ScalarValue>
  │    ├─ merge_batch(state arrays)
  │    ├─ evaluate() -> ScalarValue
  │    └─ size() -> bytes
  ├─ optional GroupsAccumulator
  │    ├─ vectorized state for all groups
  │    ├─ group_indices
  │    ├─ opt_filter
  │    └─ EmitTo output/state slicing
  └─ distributed/multi-phase grouping
       ├─ partial aggregate
       ├─ state arrays
       ├─ repartition by group keys
       ├─ final aggregate
       └─ merge/evaluate
```

The attachment already includes UDAF registration, an accumulator skeleton for geometric mean, and the key rule that the `state_type` vector must correspond to values emitted by `Accumulator::state()` and consumed by `merge_batch`.  Current DataFusion docs define `Accumulator` as a stateful object for one group, with required methods `update_batch`, `evaluate`, `size`, `state`, and `merge_batch`; DataFusion uses `state()` and `merge_batch()` for efficient multi-phase grouping across partial aggregate instances. ([Docs.rs][1])

---

## C13.1 Core mental model

```text id="accumulator-model"
For each GROUP BY group:
  Accumulator::new()
    ├─ update_batch(input arrays)    # raw rows -> local state
    ├─ state()                       # local state -> Arrow/Scalar state values
    ├─ merge_batch(state arrays)     # partial states -> merged state
    └─ evaluate()                    # merged state -> final aggregate ScalarValue
```

DataFusion’s `AggregateUDF` is the logical representation of a UDAF; it provides name, type information, and a factory function to create an `Accumulator`, and it can be used in ordinary SQL aggregates and as window functions with `OVER`. ([Docs.rs][2]) `create_udaf` creates a UDAF with a specific input signature, state type, and return type, and its docs explicitly say the signature and state type must match the `Accumulator` implementation. ([Docs.rs][3])

Agent invariant:

```text id="state-invariant"
UDAF correctness = algebraic state correctness.

If:
  update_batch(row partition A) -> state_A
  update_batch(row partition B) -> state_B
  merge_batch([state_A, state_B]) -> state_AB
then:
  evaluate(state_AB) must equal evaluate(update_batch(A ∪ B))
within documented numerical tolerance.
```

---

# C13.2 Accumulator lifecycle

## C13.2.1 Required methods

```rust id="accumulator-trait-shape"
pub trait Accumulator: Send + Sync + std::fmt::Debug {
    fn update_batch(&mut self, values: &[ArrayRef]) -> Result<()>;
    fn evaluate(&mut self) -> Result<ScalarValue>;
    fn size(&self) -> usize;
    fn state(&mut self) -> Result<Vec<ScalarValue>>;
    fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()>;

    // provided:
    fn retract_batch(&mut self, values: &[ArrayRef]) -> Result<()> { ... }
    fn supports_retract_batch(&self) -> bool { ... }
}
```

DataFusion docs state `update_batch` updates state from input arrays, `evaluate` returns the final aggregate value, `state` emits intermediate state for multi-phase grouping, `merge_batch` combines partial state from other accumulators, and `size` reports allocated memory used during execution. ([Docs.rs][1])

## C13.2.2 Method semantics

| Method         | Input                         | Output             | Must do                  | Must not do                        |
| -------------- | ----------------------------- | ------------------ | ------------------------ | ---------------------------------- |
| `update_batch` | raw aggregate argument arrays | `()`               | update local state       | emit final value                   |
| `state`        | local state                   | `Vec<ScalarValue>` | serialize partial state  | be called twice in normal flow     |
| `merge_batch`  | arrays of partial states      | `()`               | merge all partial states | assume raw input rows              |
| `evaluate`     | current state                 | `ScalarValue`      | compute final output     | consume internal state             |
| `size`         | current state                 | `usize`            | report allocated bytes   | use only `len` for dynamic buffers |

DataFusion docs are explicit that `evaluate()` must not consume internal state because the accumulator may be used in window aggregate contexts and evaluated multiple times; `state()` returns intermediate state, is used for multi-phase grouping, and should not be called twice because it may produce nondeterministic behavior if state is consumed. ([Docs.rs][1])

---

# C13.3 UDAF registration surfaces

## C13.3.1 Simple `create_udaf`

```rust id="create-udaf-shape"
use std::sync::Arc;
use datafusion::arrow::datatypes::DataType;
use datafusion::logical_expr::{create_udaf, Volatility};

let udaf = create_udaf(
    "weighted_avg",
    vec![DataType::Float64, DataType::Float64],       // input types
    Arc::new(DataType::Float64),                      // return type
    Volatility::Immutable,
    Arc::new(|_args| Ok(Box::new(WeightedAvg::new()))),
    Arc::new(vec![DataType::Float64, DataType::Float64]), // state types
);

ctx.register_udaf(udaf);
```

`create_udaf` takes name, input types, return type, volatility, an accumulator factory accepting `AccumulatorArgs`, and state types; DataFusion’s docs state the signature and state type must match the accumulator implementation. ([Docs.rs][3])

## C13.3.2 DataFrame usage

```rust id="udaf-call"
let weighted_avg = weighted_avg_udaf();

let df = df.aggregate(
    vec![col("region")],
    vec![weighted_avg.call(vec![col("value"), col("weight")]).alias("weighted_value")],
)?;
```

`AggregateUDF::call` creates an `Expr` that calls the aggregate function and allows using the UDAF without registry lookup, such as in DataFrame API construction. ([Docs.rs][2])

## C13.3.3 SQL usage

```sql id="udaf-sql"
SELECT
  region,
  weighted_avg(value, weight) AS weighted_value
FROM t
GROUP BY region;
```

---

# C13.4 State design taxonomy

## C13.4.1 Primitive state

```text id="primitive-state"
State = one scalar.

Examples:
  count_non_null:
    state: UInt64 count

  sum_f64:
    state: Float64 sum

  max_f64:
    state: Float64 current_max + optional seen flag
```

Use when:

```text id="primitive-use"
state is one primitive value
merge operation is simple
null/empty behavior is trivial or encoded with sentinel/seen flag
```

Risk:

```text id="primitive-risk"
A single primitive often cannot distinguish:
  no rows seen
  all rows null
  legitimate zero result
```

Add `count` / `seen` field when needed.

---

## C13.4.2 Multi-field state

```text id="multi-field-state"
State = fixed tuple of scalar fields.

Examples:
  average:
    sum: Float64
    count: UInt64

  weighted_avg:
    weighted_sum: Float64
    weight_sum: Float64

  online_variance:
    count: UInt64
    mean: Float64
    m2: Float64
```

Use when:

```text id="multi-field-use"
partial state is fixed-size
merge is associative
state fields are known at planning time
```

---

## C13.4.3 List state

```text id="list-state"
State = ScalarValue::List or ListArray.

Examples:
  exact median:
    all values in List<Float64>

  exact percentile:
    all values in List<Float64>

  top_k:
    List<Struct<value, score>>

  sketch bytes:
    List<UInt8> or Binary
```

DataFusion docs note `ScalarValue::List` can be used to pass multiple intermediate values when the number of intermediate values is not known at planning time, such as median. ([Docs.rs][1])

Use when:

```text id="list-state-use"
state cardinality varies by group
state is bounded by explicit k
state is a serialized sketch
exact method requires retaining values
```

Risk:

```text id="list-state-risk"
unbounded list state can explode memory
state serialization can dominate runtime
merge may require sorting/deduplication
```

---

## C13.4.4 Struct state

```text id="struct-state"
State = Struct of named subfields.

Examples:
  quality_state:
    valid_count
    invalid_count
    weighted_sum
    weight_sum
    worst_reason

  sketch_state:
    version
    seed
    registers
```

Use when:

```text id="struct-state-use"
state itself needs schema clarity
state fields are nested or versioned
state may evolve internally
diagnostic state is complex
```

Risk:

```text id="struct-state-risk"
more complicated Arrow builders
harder merge_batch downcasts
strict state schema tests required
```

---

## C13.4.5 Approximate sketch state

```text id="sketch-state"
State = compact mergeable sketch.

Examples:
  approximate quantile
  approximate distinct
  top-k approximate
  heavy hitters
  t-digest / KLL / HLL-like structures
```

Use when:

```text id="sketch-use"
exact state is unbounded
approximation is acceptable
merge algorithm is associative
error bounds can be documented
memory bound is fixed/configurable
```

Required manifest:

```yaml id="sketch-manifest"
aggregate:
  name: approx_quantile_custom
  state_kind: sketch
  algorithm: kll
  max_bytes_per_group: 4096
  error_bound:
    type: rank_error
    value: 0.01
  deterministic_merge: true
  seed: 0
```

---

## C13.4.6 Bounded vs unbounded state

| State type                        | Memory bound | Production posture         |
| --------------------------------- | -----------: | -------------------------- |
| count/sum/min/max                 |        fixed | safe                       |
| weighted average                  |        fixed | safe                       |
| online variance                   |        fixed | safe                       |
| top-k exact with fixed k          |      bounded | safe if k capped           |
| sketch with fixed max size        |      bounded | safe if error documented   |
| exact median retaining all values |    unbounded | restrict/precompute/sketch |
| exact list aggregation            |    unbounded | restrict                   |
| distinct set exact                |    unbounded | restrict or sketch         |
| string concat all values          |    unbounded | restrict                   |

Agent rule:

```text id="bounded-agent-rule"
Unbounded per-group state is a deployment risk.
Use bounded sketches, explicit k, or external preprocessing unless exact unbounded behavior is product-required and resource-controlled.
```

---

# C13.5 Required invariants

## C13.5.1 `state()` shape equals declared `state_type`

```text id="state-shape-invariant"
create_udaf state_type:
  [Float64, Float64]

Accumulator::state():
  [ScalarValue::Float64(...), ScalarValue::Float64(...)]
```

Test helper:

```rust id="state-shape-test-helper"
pub fn assert_state_matches_types(
    state: &[ScalarValue],
    state_types: &[DataType],
) {
    assert_eq!(state.len(), state_types.len());

    for (value, expected_type) in state.iter().zip(state_types) {
        assert_eq!(&value.data_type(), expected_type);
    }
}
```

The attachment already states the `state_type` vector must correspond to values emitted by `state()` and consumed by `merge_batch`. 

---

## C13.5.2 `merge_batch()` accepts state arrays, not raw input arrays

```text id="merge-input-invariant"
update_batch inputs:
  raw argument arrays:
    value_array
    weight_array

merge_batch inputs:
  arrays built from state():
    weighted_sum_state_array
    weight_sum_state_array
```

Bad:

```rust id="bad-merge"
fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
    // wrong: treats state arrays as raw value, weight input rows
    self.update_batch(states)
}
```

Good:

```rust id="good-merge"
fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
    let weighted_sums = as_f64_array("weighted_avg", &states[0])?;
    let weight_sums = as_f64_array("weighted_avg", &states[1])?;

    for i in 0..weighted_sums.len() {
        if !weighted_sums.is_null(i) && !weight_sums.is_null(i) {
            self.weighted_sum += weighted_sums.value(i);
            self.weight_sum += weight_sums.value(i);
        }
    }

    Ok(())
}
```

DataFusion docs explain that partial state is serialized as arrays and then combined by other accumulator instances using `merge_batch`, and that state can be a different type from the aggregate output, such as `COUNT` partial state being summed together. ([Docs.rs][1])

---

## C13.5.3 `update_batch` and `merge_batch` must be associative-compatible

```text id="associativity"
For arbitrary partitioning:
  update(A ∪ B) == merge(state(update(A)), state(update(B)))
```

Test:

```rust id="merge-equivalence-test"
#[test]
fn weighted_avg_merge_equivalence() -> Result<()> {
    let all = batches_from_values(&[
        (10.0, 2.0),
        (20.0, 1.0),
        (30.0, 3.0),
    ]);

    let left = batches_from_values(&[(10.0, 2.0)]);
    let right = batches_from_values(&[(20.0, 1.0), (30.0, 3.0)]);

    let single = evaluate_accumulator(WeightedAvg::new(), &all)?;
    let merged = evaluate_merged_accumulators(
        WeightedAvg::new(),
        WeightedAvg::new(),
        &left,
        &right,
    )?;

    assert_f64_close(single, merged, 1e-12);
    Ok(())
}
```

---

## C13.5.4 `evaluate()` deterministic for same state

```text id="evaluate-determinism"
Given identical internal state:
  evaluate() returns identical ScalarValue
  evaluate() does not consume or corrupt state
```

Test:

```rust id="evaluate-idempotence-test"
#[test]
fn evaluate_does_not_consume_state() -> Result<()> {
    let mut acc = WeightedAvg::new();
    acc.update_batch(&weighted_avg_input_arrays(&[(10.0, 2.0), (20.0, 1.0)]))?;

    let a = acc.evaluate()?;
    let b = acc.evaluate()?;

    assert_eq!(a, b);
    Ok(())
}
```

DataFusion’s `Accumulator::evaluate` docs explicitly say it must not consume internal state because it can be used in window aggregate contexts where `evaluate` may run multiple times. ([Docs.rs][1])

---

## C13.5.5 `state()` should not be treated as repeatable

```text id="state-repeatability"
DataFusion docs:
  state() returns intermediate state and may consume it.
  calling it twice can cause nondeterministic behavior.
```

Design rule:

```text id="state-rule"
Tests may call state() once per accumulator branch.
If repeated state inspection is needed, implement explicit debug_snapshot() outside Accumulator trait for tests.
```

Cite: DataFusion states `state()` should not be called twice because it can result in potentially nondeterministic behavior. ([Docs.rs][1])

---

# C13.6 Distributed readiness

## C13.6.1 Multi-phase grouping

```text id="multiphase"
Partial phase:
  input partition -> update_batch -> state()

Repartition:
  state rows repartitioned by group key

Final phase:
  merge_batch(partial state arrays) -> evaluate()
```

DataFusion docs illustrate multi-phase grouping: partial group-by accumulators run on input partitions, `state()` is called, resulting record batches are passed to final group-by via `merge_batch`, and repartitioning by hash group keys ensures each distinct group appears in exactly one final partition. ([Docs.rs][1])

## C13.6.2 State serializability

```text id="state-serializability"
UDAF state must be representable as:
  Vec<ScalarValue> from state()
  Arrow arrays for merge_batch()
  stable DataType list from state_type/state_fields
```

Rule:

```text id="serial-rule"
If state cannot be serialized as Arrow scalars/arrays, it is not a valid DataFusion aggregate state.
```

## C13.6.3 Floating-point nondeterminism

```text id="float-nondeterminism"
Floating aggregation order can vary:
  partitioning
  batch order
  merge order
  thread count
  hash group distribution
```

Policies:

```text id="float-policies"
Fast approximate:
  normal f64 sum/merge
  test with tolerance

More stable:
  Kahan/Neumaier compensated sum state
  pairwise merge
  deterministic sorting only if cost acceptable

Exact:
  Decimal / integer fixed-point where viable
```

Manifest:

```yaml id="float-manifest"
numerical_policy:
  type: floating_point
  deterministic_bitwise: false
  tolerance:
    abs: 1.0e-10
    rel: 1.0e-8
  merge_order_sensitive: true
```

Agent rule:

```text id="float-agent"
Do not promise bitwise-deterministic floating aggregate output unless implementation enforces deterministic order/precision.
```

---

## C13.6.4 Distributed executor deployment

```text id="distributed-deploy"
Every executor must have:
  same UDAF implementation
  same DataFusion version
  same Arrow type versions
  same state schema
  same function version
  same serialization logic
```

The attachment’s upgrade checklist explicitly includes `Accumulator / GroupsAccumulator` among DataFusion extension traits that must be reviewed after version upgrades. 

---

# C13.7 Memory accounting

## C13.7.1 `size()` contract

```text id="size-contract"
size() returns allocated bytes including Self.
For Vec/String/HashMap/sketch:
  use capacity, not length.
```

DataFusion docs say `Accumulator::size()` returns allocated size in bytes including `Self`, and that for internal containers such as `Vec`, capacity should be used rather than length because the value is used to calculate execution memory. ([Docs.rs][1])

## C13.7.2 Fixed-size state

```rust id="fixed-size"
fn size(&self) -> usize {
    std::mem::size_of_val(self)
}
```

Use for:

```text id="fixed-size-use"
sum
count
weighted average
online variance
geometric mean
bounded primitive state
```

## C13.7.3 Dynamic state

```rust id="dynamic-size"
#[derive(Debug)]
struct TopKState {
    k: usize,
    items: Vec<(f64, String)>,
}

impl TopKState {
    fn allocated_size(&self) -> usize {
        std::mem::size_of_val(self)
            + self.items.capacity() * std::mem::size_of::<(f64, String)>()
            + self.items.iter().map(|(_, s)| s.capacity()).sum::<usize>()
    }
}

impl Accumulator for TopKState {
    fn size(&self) -> usize {
        self.allocated_size()
    }

    // update_batch, state, merge_batch, evaluate...
}
```

## C13.7.4 Sketch state

```rust id="sketch-size"
fn size(&self) -> usize {
    std::mem::size_of_val(self)
        + self.sketch.capacity_bytes()
}
```

## C13.7.5 Spill implications

```text id="spill-implications"
DataFusion memory accounting can track aggregate state size.
But UDAF implementors must report allocated state accurately.
Underreported size:
  OOM risk
  spill/admission decisions wrong
  tenant resource controls bypassed

Overreported size:
  premature spilling/failure
  lower concurrency
```

Agent rule:

```text id="memory-agent"
Every dynamic-state UDAF must have a size() test.
Use capacity, not length.
Include nested allocation inside sketches/strings/maps.
```

---

# C13.8 GroupsAccumulator

## C13.8.1 Purpose

```text id="groups-purpose"
GroupsAccumulator:
  one aggregate implementation stores state for all groups internally
  receives group_indices for each input row
  can be much faster for high-cardinality GROUP BY
  optional and harder than Accumulator
```

DataFusion docs state every aggregate must first implement the simpler `Accumulator`; `GroupsAccumulator` is optional, more complex, and can be much faster for many groups. It stores state for all groups internally, indexed by contiguous `group_index` values, and implementations commonly use `Vec` storage. ([Docs.rs][4])

## C13.8.2 Trait shape

```rust id="groups-shape"
pub trait GroupsAccumulator: Send {
    fn update_batch(
        &mut self,
        values: &[ArrayRef],
        group_indices: &[usize],
        opt_filter: Option<&BooleanArray>,
        total_num_groups: usize,
    ) -> Result<()>;

    fn evaluate(&mut self, emit_to: EmitTo) -> Result<ArrayRef>;

    fn state(&mut self, emit_to: EmitTo) -> Result<Vec<ArrayRef>>;

    fn merge_batch(
        &mut self,
        values: &[ArrayRef],
        group_indices: &[usize],
        opt_filter: Option<&BooleanArray>,
        total_num_groups: usize,
    ) -> Result<()>;

    fn size(&self) -> usize;
}
```

Current docs show required `GroupsAccumulator` methods `update_batch`, `evaluate`, `state`, `merge_batch`, and `size`, with `update_batch` accepting input arrays, `group_indices`, optional filter, and `total_num_groups`. ([Docs.rs][4])

## C13.8.3 When to implement

```text id="groups-use"
Implement GroupsAccumulator when:
  GROUP BY cardinality is high
  Accumulator-per-group allocation dominates
  state is simple enough for Vec-backed grouped storage
  performance benchmark justifies complexity
```

Avoid when:

```text id="groups-avoid"
aggregate is rarely used
state is complex/unbounded
simple Accumulator is fast enough
correct group-index handling is risky
no benchmark harness exists
```

## C13.8.4 Group-index invariants

```text id="group-index-invariants"
group_indices.len == values[0].len
each group_index < total_num_groups
group_indices are contiguous logical ids
subsequent update_batch may increase total_num_groups
state vectors must resize to total_num_groups
opt_filter false -> skip row
evaluate/state emits rows in group-index order for requested EmitTo
```

DataFusion docs state `group_index` values are contiguous, `total_num_groups` is the number of groups, and later `update_batch` calls may have a larger `total_num_groups` as new groups are seen. ([Docs.rs][4])

---

# C13.9 Example: weighted average

## C13.9.1 State contract

```text id="weighted-state"
weighted_avg(value, weight) -> Float64

state:
  weighted_sum: Float64
  weight_sum: Float64

update:
  if value and weight non-null:
    weighted_sum += value * weight
    weight_sum += weight

merge:
  weighted_sum += partial_weighted_sum
  weight_sum += partial_weight_sum

evaluate:
  if weight_sum == 0: NULL
  else weighted_sum / weight_sum
```

## C13.9.2 Implementation

```rust id="weighted-avg-impl"
use std::sync::Arc;
use datafusion::arrow::{
    array::{Array, ArrayRef, Float64Array},
    datatypes::DataType,
};
use datafusion::common::{internal_err, Result, ScalarValue};
use datafusion::logical_expr::Accumulator;

#[derive(Debug, Default)]
pub struct WeightedAvg {
    weighted_sum: f64,
    weight_sum: f64,
}

impl WeightedAvg {
    pub fn new() -> Self {
        Self::default()
    }
}

fn as_f64<'a>(fn_name: &str, array: &'a ArrayRef) -> Result<&'a Float64Array> {
    array
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or_else(|| datafusion::error::DataFusionError::Internal(format!(
            "{fn_name} expected Float64Array, got {:?}",
            array.data_type()
        )))
}

impl Accumulator for WeightedAvg {
    fn update_batch(&mut self, values: &[ArrayRef]) -> Result<()> {
        if values.len() != 2 {
            return internal_err!("weighted_avg expected 2 input arrays");
        }

        let x = as_f64("weighted_avg", &values[0])?;
        let w = as_f64("weighted_avg", &values[1])?;

        if x.len() != w.len() {
            return internal_err!(
                "weighted_avg length mismatch: value={}, weight={}",
                x.len(),
                w.len()
            );
        }

        for i in 0..x.len() {
            if x.is_null(i) || w.is_null(i) {
                continue;
            }

            let weight = w.value(i);
            self.weighted_sum += x.value(i) * weight;
            self.weight_sum += weight;
        }

        Ok(())
    }

    fn state(&mut self) -> Result<Vec<ScalarValue>> {
        Ok(vec![
            ScalarValue::Float64(Some(self.weighted_sum)),
            ScalarValue::Float64(Some(self.weight_sum)),
        ])
    }

    fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
        if states.len() != 2 {
            return internal_err!("weighted_avg expected 2 state arrays");
        }

        let weighted_sums = as_f64("weighted_avg.state.weighted_sum", &states[0])?;
        let weight_sums = as_f64("weighted_avg.state.weight_sum", &states[1])?;

        if weighted_sums.len() != weight_sums.len() {
            return internal_err!("weighted_avg state length mismatch");
        }

        for i in 0..weighted_sums.len() {
            if weighted_sums.is_null(i) || weight_sums.is_null(i) {
                continue;
            }

            self.weighted_sum += weighted_sums.value(i);
            self.weight_sum += weight_sums.value(i);
        }

        Ok(())
    }

    fn evaluate(&mut self) -> Result<ScalarValue> {
        if self.weight_sum == 0.0 {
            Ok(ScalarValue::Float64(None))
        } else {
            Ok(ScalarValue::Float64(Some(self.weighted_sum / self.weight_sum)))
        }
    }

    fn size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}
```

## C13.9.3 Registration

```rust id="weighted-avg-registration"
use datafusion::logical_expr::{create_udaf, Volatility};

pub fn weighted_avg_udaf() -> AggregateUDF {
    create_udaf(
        "weighted_avg",
        vec![DataType::Float64, DataType::Float64],
        Arc::new(DataType::Float64),
        Volatility::Immutable,
        Arc::new(|_args| Ok(Box::new(WeightedAvg::new()))),
        Arc::new(vec![DataType::Float64, DataType::Float64]),
    )
}
```

## C13.9.4 Tests

```text id="weighted-tests"
[ ] empty group -> NULL
[ ] all-null values -> NULL
[ ] zero total weight -> NULL
[ ] partial merge equivalence
[ ] negative weight policy
[ ] floating tolerance
```

---

# C13.10 Example: geometric mean

The attachment’s geometric-mean example uses `product: f64` and `count: u64` state, `update_batch` multiplies values and increments count, `state()` emits `Float64(product)` and `UInt64(count)`, `merge_batch` multiplies partial products and sums counts, `evaluate()` returns `NULL` for zero count or `product.powf(1.0 / count)`, and `create_udaf` registers state types `[Float64, UInt64]`. 

## C13.10.1 State contract

```text id="geo-state"
geo_mean(x) -> Float64

state:
  log_sum: Float64
  count: UInt64

update:
  if x > 0:
    log_sum += ln(x)
    count += 1
  else:
    null/error depending policy

merge:
  log_sum += partial_log_sum
  count += partial_count

evaluate:
  if count == 0: NULL
  else exp(log_sum / count)
```

## C13.10.2 Recommended improvement over product-state

Use log-sum state instead of product state to reduce overflow/underflow risk:

```rust id="geo-mean-log"
#[derive(Debug, Default)]
pub struct GeoMeanLog {
    log_sum: f64,
    count: u64,
}

impl Accumulator for GeoMeanLog {
    fn update_batch(&mut self, values: &[ArrayRef]) -> Result<()> {
        let x = as_f64("geo_mean", &values[0])?;

        for i in 0..x.len() {
            if x.is_null(i) {
                continue;
            }

            let v = x.value(i);

            if v <= 0.0 {
                // Policy: skip non-positive or error. Choose explicitly.
                continue;
            }

            self.log_sum += v.ln();
            self.count += 1;
        }

        Ok(())
    }

    fn state(&mut self) -> Result<Vec<ScalarValue>> {
        Ok(vec![
            ScalarValue::Float64(Some(self.log_sum)),
            ScalarValue::UInt64(Some(self.count)),
        ])
    }

    fn merge_batch(&mut self, states: &[ArrayRef]) -> Result<()> {
        let log_sums = as_f64("geo_mean.state.log_sum", &states[0])?;
        let counts = states[1]
            .as_any()
            .downcast_ref::<datafusion::arrow::array::UInt64Array>()
            .ok_or_else(|| datafusion::error::DataFusionError::Internal(
                "geo_mean expected UInt64 count state".to_string()
            ))?;

        for i in 0..log_sums.len() {
            if log_sums.is_null(i) || counts.is_null(i) {
                continue;
            }
            self.log_sum += log_sums.value(i);
            self.count += counts.value(i);
        }

        Ok(())
    }

    fn evaluate(&mut self) -> Result<ScalarValue> {
        if self.count == 0 {
            Ok(ScalarValue::Float64(None))
        } else {
            Ok(ScalarValue::Float64(Some((self.log_sum / self.count as f64).exp())))
        }
    }

    fn size(&self) -> usize {
        std::mem::size_of_val(self)
    }
}
```

---

# C13.11 Example: online variance

## C13.11.1 State contract

```text id="variance-state"
online_variance(x) -> Float64

state:
  count: UInt64
  mean: Float64
  m2: Float64

update one value x:
  count += 1
  delta = x - mean
  mean += delta / count
  delta2 = x - mean
  m2 += delta * delta2

merge two states:
  n = n_a + n_b
  delta = mean_b - mean_a
  mean = mean_a + delta * n_b / n
  m2 = m2_a + m2_b + delta^2 * n_a * n_b / n

evaluate sample variance:
  if count < 2: NULL
  else m2 / (count - 1)
```

## C13.11.2 Implementation sketch

```rust id="variance-impl"
#[derive(Debug, Default, Clone)]
pub struct OnlineVariance {
    count: u64,
    mean: f64,
    m2: f64,
}

impl OnlineVariance {
    fn update_one(&mut self, x: f64) {
        self.count += 1;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;
    }

    fn merge_state(&mut self, other: OnlineVariance) {
        if other.count == 0 {
            return;
        }

        if self.count == 0 {
            *self = other;
            return;
        }

        let n_a = self.count as f64;
        let n_b = other.count as f64;
        let n = n_a + n_b;
        let delta = other.mean - self.mean;

        self.mean += delta * n_b / n;
        self.m2 += other.m2 + delta * delta * n_a * n_b / n;
        self.count += other.count;
    }
}
```

## C13.11.3 State types

```rust id="variance-state-types"
Arc::new(vec![
    DataType::UInt64,  // count
    DataType::Float64, // mean
    DataType::Float64, // m2
])
```

## C13.11.4 Agent rules

```text id="variance-agent"
Use mergeable online variance, not sum/sum_sq naive formula, for numerical stability.
Document sample vs population variance.
Test partition merge equivalence with tolerance.
```

---

# C13.12 Example: top-k exact bounded state

## C13.12.1 State contract

```text id="topk-state"
top_k(value, score, k_literal) -> List<Struct<value, score>>

state:
  bounded min-heap or sorted Vec of at most k entries

update:
  insert candidate
  retain best k

merge:
  merge candidate lists
  retain best k

evaluate:
  return sorted List<Struct<value, score>>
```

## C13.12.2 State representation options

| Representation               | State type                     | Pros                         | Cons             |
| ---------------------------- | ------------------------------ | ---------------------------- | ---------------- |
| `List<Struct<value, score>>` | Arrow list/struct              | typed, inspectable           | complex builders |
| serialized binary sketch     | `Binary`                       | compact, flexible            | opaque           |
| multiple parallel lists      | `List<Value>`, `List<Float64>` | easier than struct sometimes | alignment risk   |

## C13.12.3 Memory accounting

```rust id="topk-size"
fn size(&self) -> usize {
    std::mem::size_of_val(self)
        + self.items.capacity() * std::mem::size_of::<TopKItem>()
        + self.items.iter().map(|i| i.value.capacity()).sum::<usize>()
}
```

## C13.12.4 Agent rules

```text id="topk-agent"
k must be bounded.
k should be scalar literal or config-limited.
state size is O(k) per group.
tie-break ordering must be deterministic.
evaluate output order must be documented.
```

---

# C13.13 Example: approximate quantile

## C13.13.1 State contract

```text id="approx-quantile-state"
approx_quantile(x, q) -> Float64

state:
  bounded sketch bytes / registers / centroids

update:
  insert x into sketch

merge:
  merge sketches

evaluate:
  query sketch at q

properties:
  approximate
  bounded memory
  mergeable
  error-bound documented
```

## C13.13.2 State choices

```text id="approx-state-choices"
Binary serialized sketch:
  state_type = [Binary]
  easiest versioned serialization
  merge_batch deserializes and merges

List/Struct centroids:
  state_type = [List<Struct<mean, weight>>]
  inspectable but more complex

Multiple primitive lists:
  state_type = [List<Float64>, List<Float64>]
  means + weights
```

## C13.13.3 Manifest

```yaml id="approx-manifest"
aggregate:
  name: approx_quantile_custom
  return_type: Float64
  state:
    kind: sketch
    state_types:
      - Binary
    algorithm: tdigest
    max_centroids: 100
    version: 1
  accuracy:
    deterministic: false
    error_bound_doc: approximate rank error depends on data distribution
```

## C13.13.4 Agent rules

```text id="approx-agent"
Approximate UDAF must disclose algorithm, memory bound, merge behavior, error semantics.
Serialized sketch state must include version.
merge_batch must reject incompatible sketch versions.
```

---

# C13.14 Example: domain-specific quality metric

## C13.14.1 State contract

```text id="quality-state"
blend_quality_score(component_quality, component_volume, penalty_flag) -> Float64

state:
  weighted_quality_sum: Float64
  volume_sum: Float64
  penalty_count: UInt64
  invalid_count: UInt64

update:
  if quality/volume valid:
    weighted_quality_sum += quality * volume
    volume_sum += volume
  if penalty_flag:
    penalty_count += 1
  if invalid:
    invalid_count += 1

merge:
  sum every field

evaluate:
  if volume_sum == 0: NULL
  else weighted_quality_sum / volume_sum - penalty_factor * penalty_count
```

## C13.14.2 State type

```rust id="quality-state-types"
Arc::new(vec![
    DataType::Float64, // weighted_quality_sum
    DataType::Float64, // volume_sum
    DataType::UInt64,  // penalty_count
    DataType::UInt64,  // invalid_count
])
```

## C13.14.3 Agent rules

```text id="quality-agent"
Domain aggregates should include invalid/diagnostic counters in state when output depends on data-quality policy.
Expose separate audited aggregate if users need diagnostics:
  blend_quality_score(...)
  blend_quality_diagnostics(...)
```

---

# C13.15 Advanced API: `AggregateUDFImpl`

## C13.15.1 When to use

```text id="aggregate-impl-use"
Use create_udaf for:
  simple fixed-type aggregate
  simple state_type vector
  no custom state fields/nullability/docs/coercion

Use AggregateUDFImpl for:
  advanced return fields
  custom state_fields
  custom default values
  custom groups accumulator
  custom ordering/null handling support
  documentation
  aliases
  optimizer/statistics hooks
```

`AggregateUDF` docs state simple use cases should use `create_udaf`, while advanced use cases should use `AggregateUDFImpl`, which provides full API access. ([Docs.rs][2]) Current `AggregateUDF` methods include `create_groups_accumulator`, `groups_accumulator_supported`, `state_fields`, `return_field`, `return_type`, `default_value`, aliases, documentation, null-handling support, ordering support, and related planning/display hooks. ([Docs.rs][2])

## C13.15.2 Agent rule

```text id="aggregate-impl-agent"
Start with create_udaf.
Escalate to AggregateUDFImpl when state/return/nullability/documentation/GroupsAccumulator behavior must be controlled explicitly.
```

---

# C13.16 Null and filter behavior

## C13.16.1 Null-state rules

```text id="null-rules"
Aggregate must distinguish:
  empty group
  all-null group
  zero-valued valid group
  filtered-out group
  group with invalid rows skipped
```

## C13.16.2 `FILTER` support in grouping

SQL:

```sql id="filter-sql"
SELECT
  region,
  weighted_avg(value, weight) FILTER (WHERE weight > 0) AS weighted_positive
FROM t
GROUP BY region;
```

For `GroupsAccumulator`, `opt_filter` indicates only rows with `opt_filter[i] == true` should update aggregate state; current docs explicitly describe `opt_filter` this way. ([Docs.rs][4])

## C13.16.3 Agent rules

```text id="null-filter-agent"
Do not treat filtered-out rows as null rows.
Do not treat all-null groups as zero unless aggregate semantics say so.
Add explicit seen/count state when output NULL depends on no valid values.
```

---

# C13.17 Test cookbook

## C13.17.1 Direct accumulator tests

```rust id="direct-acc-test"
#[test]
fn weighted_avg_direct_update_evaluate() -> Result<()> {
    let mut acc = WeightedAvg::new();

    acc.update_batch(&weighted_avg_input_arrays(&[
        (Some(10.0), Some(2.0)),
        (Some(20.0), Some(1.0)),
        (None, Some(9.0)),
    ]))?;

    assert_eq!(
        acc.evaluate()?,
        ScalarValue::Float64(Some((10.0 * 2.0 + 20.0 * 1.0) / 3.0))
    );

    Ok(())
}
```

## C13.17.2 State-shape test

```rust id="state-shape-test"
#[test]
fn weighted_avg_state_shape() -> Result<()> {
    let mut acc = WeightedAvg::new();
    acc.update_batch(&weighted_avg_input_arrays(&[(Some(10.0), Some(2.0))]))?;

    let state = acc.state()?;
    let state_types = vec![DataType::Float64, DataType::Float64];

    assert_state_matches_types(&state, &state_types);
    Ok(())
}
```

## C13.17.3 Merge equivalence

```rust id="merge-eq-test"
#[test]
fn weighted_avg_merge_equivalence() -> Result<()> {
    let mut single = WeightedAvg::new();
    single.update_batch(&weighted_avg_input_arrays(&[
        (Some(10.0), Some(2.0)),
        (Some(20.0), Some(1.0)),
        (Some(30.0), Some(3.0)),
    ]))?;
    let single_out = single.evaluate()?;

    let mut left = WeightedAvg::new();
    left.update_batch(&weighted_avg_input_arrays(&[(Some(10.0), Some(2.0))]))?;
    let left_state = state_to_arrays(left.state()?);

    let mut right = WeightedAvg::new();
    right.update_batch(&weighted_avg_input_arrays(&[
        (Some(20.0), Some(1.0)),
        (Some(30.0), Some(3.0)),
    ]))?;
    let right_state = state_to_arrays(right.state()?);

    let mut merged = WeightedAvg::new();
    merged.merge_batch(&left_state)?;
    merged.merge_batch(&right_state)?;

    assert_scalar_f64_close(single_out, merged.evaluate()?, 1e-12);
    Ok(())
}
```

## C13.17.4 SQL integration

```rust id="sql-integration-test"
#[tokio::test]
async fn weighted_avg_sql_group_by() -> datafusion::error::Result<()> {
    let ctx = SessionContext::new();
    ctx.register_udaf(weighted_avg_udaf());

    register_test_batch(&ctx, "t")?;

    let batches = ctx
        .sql(
            "SELECT region, weighted_avg(value, weight) AS wavg
             FROM t
             GROUP BY region
             ORDER BY region"
        )
        .await?
        .collect()
        .await?;

    assert_batches_eq!(
        &[
            "+--------+------+",
            "| region | wavg |",
            "+--------+------+",
            "| east   | 12.5 |",
            "| west   |      |",
            "+--------+------+",
        ],
        &batches
    );

    Ok(())
}
```

## C13.17.5 Partition sensitivity

```text id="partition-tests"
Run same query with:
  target_partitions = 1
  target_partitions = 2
  target_partitions = 8

Assert:
  exact match for deterministic integer/decimal aggregates
  tolerance match for floating aggregates
```

## C13.17.6 Required matrix

```text id="required-matrix"
[ ] empty input
[ ] all-null group
[ ] filtered-out group
[ ] single row
[ ] multiple batches
[ ] multiple partitions
[ ] partial merge equivalence
[ ] evaluate twice
[ ] state shape
[ ] merge_batch state array shape
[ ] size() dynamic capacity
[ ] SQL GROUP BY
[ ] window OVER use if supported
[ ] null handling clause if supported
[ ] decimal/timestamp/nested state if relevant
```

---

# C13.18 Deployment policy

## C13.18.1 Public SQL

```text id="public-policy"
Allow:
  fixed-state deterministic UDAFs
  bounded-state UDAFs with clear limits
  approximate sketches with documented error

Restrict:
  exact median/list/distinct unbounded state
  top-k without k cap
  string/list aggregations without output size cap
  high-memory sketches without per-group bound
```

## C13.18.2 Internal analytics

```text id="internal-policy"
Allow:
  broader aggregates
  exact algorithms
  high-memory aggregates

Require:
  memory accounting
  target_partitions tests
  benchmark by group cardinality
  input-size/resource limits
```

## C13.18.3 Distributed execution

```text id="distributed-policy"
Require:
  merge associativity
  state serializability
  versioned state schema
  identical implementation on all executors
  tolerance policy for floats
  no process-local hidden state
```

---

# C13.19 Performance guidance

## C13.19.1 Choose state carefully

```text id="perf-state"
Fixed scalar state:
  fastest, smallest

Vec/list state:
  cost grows per group
  size must include capacity

Sketch:
  bounded, mergeable, approximate

GroupsAccumulator:
  fastest for many groups if implemented correctly
  more complex
```

## C13.19.2 Benchmark axes

```text id="benchmark-axes"
row count
group count
rows per group distribution
null fraction
filter selectivity
batch size
target_partitions
state size per group
merge phase cost
evaluate cost
```

## C13.19.3 Benchmark manifest

```yaml id="benchmark-manifest"
benchmark:
  aggregate: weighted_avg
  rows:
    - 10000
    - 1000000
    - 10000000
  groups:
    - 10
    - 10000
    - 1000000
  null_fraction:
    - 0.0
    - 0.5
  target_partitions:
    - 1
    - 8
  metrics:
    - rows_per_second
    - bytes_state_per_group
    - merge_time_ms
    - peak_memory_bytes
```

---

# C13.20 Anti-pattern inventory

* `state_type` length differs from `state()` vector length.
* `state_type` type differs from `ScalarValue::data_type()`.
* `merge_batch` treats state arrays as raw input arrays.
* `evaluate()` consumes internal state.
* `state()` called twice in tests and expected to be stable.
* `size()` ignores `Vec` / `String` / sketch heap allocation.
* Using `Vec::len()` instead of `Vec::capacity()` for memory accounting.
* Floating aggregate tested with exact equality across partitions.
* Unbounded exact median/list state exposed in public SQL.
* Top-k aggregate without `k` cap.
* Approximate sketch without algorithm/version/error documentation.
* Merge operation not associative.
* Null/all-null/empty group collapsed to zero accidentally.
* GroupsAccumulator state vectors not resized to `total_num_groups`.
* GroupsAccumulator ignores `opt_filter`.
* Group-index output order wrong.
* State schema changed without new function version.
* Hidden global mutable state.
* External service calls inside UDAF.
* No SQL GROUP BY integration test.
* No target_partitions regression test.

---

# C13.21 Agent checklist

```text id="c13-final-checklist"
[ ] Decide if UDAF is necessary; use built-in aggregate if possible.
[ ] Define aggregate state before implementation.
[ ] Prefer fixed primitive state when possible.
[ ] Use multi-field state for averages/variance/domain metrics.
[ ] Use bounded list/sketch state for top-k/quantile-like behavior.
[ ] Avoid unbounded state in public SQL.
[ ] Declare state_type/state_fields exactly.
[ ] Ensure state() vector length equals state_type length.
[ ] Ensure state() ScalarValue types equal state_type entries.
[ ] Ensure merge_batch consumes state arrays, not raw input arrays.
[ ] Ensure update_batch and merge_batch are associative-compatible.
[ ] Ensure evaluate() does not consume/corrupt state.
[ ] Ensure size() includes Self and allocated capacity of dynamic containers.
[ ] Distinguish empty group, all-null group, zero-valued group, and filtered-out group.
[ ] Document floating-point tolerance and merge-order sensitivity.
[ ] Use Decimal/integer state when exactness is required.
[ ] Version serialized sketch/list/struct state.
[ ] Implement GroupsAccumulator only after basic Accumulator and benchmarks.
[ ] For GroupsAccumulator, resize state vectors to total_num_groups.
[ ] For GroupsAccumulator, respect opt_filter.
[ ] For GroupsAccumulator, output in group-index order.
[ ] Add direct update/evaluate tests.
[ ] Add state-shape tests.
[ ] Add merge-equivalence tests.
[ ] Add evaluate-idempotence tests.
[ ] Add SQL GROUP BY tests.
[ ] Add target_partitions tests.
[ ] Add memory-size tests.
[ ] Add benchmark matrix by rows/groups/nulls/partitions.
```

[1]: https://docs.rs/datafusion/latest/datafusion/logical_expr/trait.Accumulator.html "Accumulator in datafusion::logical_expr - Rust"
[2]: https://docs.rs/datafusion/latest/datafusion/logical_expr/struct.AggregateUDF.html "AggregateUDF in datafusion::logical_expr - Rust"
[3]: https://docs.rs/datafusion/latest/datafusion/logical_expr/fn.create_udaf.html "create_udaf in datafusion::logical_expr - Rust"
[4]: https://docs.rs/datafusion/latest/datafusion/logical_expr/trait.GroupsAccumulator.html "GroupsAccumulator in datafusion::logical_expr - Rust"


# C13.22 Lowering logical aggregates: `LoweredAggregateBuilder` (DataFusion 54)

## C13.22.1 Deprecation

Code that lowers a logical aggregate `Expr` into physical planning pieces (custom physical planners, plan rewriters, engines that build `AggregateExec` inputs directly) used the free helpers in `datafusion::physical_planner`. DataFusion 54 deprecates both:

```text id="aggregate-lowering-deprecation"
create_aggregate_expr_with_name_and_maybe_filter  #[deprecated(note = "use LoweredAggregateBuilder")]
create_aggregate_expr_and_maybe_filter            #[deprecated(note = "use LoweredAggregateBuilder")]
```

The replacement lives at **`datafusion_physical_expr::aggregate`** (`aggregate.rs`) — the `aggregate` module is public but the builder is **not re-exported at the `datafusion_physical_expr` crate root**, so the full path is required:

```rust id="aggregate-lowering-imports"
use datafusion::physical_expr::aggregate::{
    AggregateExprBuilder, AggregateFunctionExpr, LoweredAggregate, LoweredAggregateBuilder,
};
```

## C13.22.2 Builder API

```rust id="lowered-aggregate-builder"
// expr: &Expr — the logical aggregate (Expr::AggregateFunction, possibly aliased)
let lowered: LoweredAggregate = LoweredAggregateBuilder::new(
    expr,
    logical_input_schema,   // &DFSchema — resolves logical columns
    physical_input_schema,  // &Schema   — input schema of the physical aggregate
    execution_props,        // &ExecutionProps
)
.with_name("total_flow")                    // optional: override output column name
.with_human_display("sum(flow_bbl_day)")    // optional: override EXPLAIN display text
.build()?;
```

The builder does the full logical-to-physical work the deprecated helpers did — unwrapping aggregate aliases, choosing the output name, preserving user-facing display text, lowering aggregate arguments, lowering the optional `FILTER (WHERE ...)`, and lowering aggregate `ORDER BY` — and returns a `LoweredAggregate` with three **public fields**:

```rust
pub struct LoweredAggregate {
    pub aggregate: Arc<AggregateFunctionExpr>,      // for AggregateExec
    pub filter: Option<Arc<dyn PhysicalExpr>>,      // lowered FILTER (WHERE ...)
    pub order_bys: Vec<PhysicalSortExpr>,           // lowered aggregate ORDER BY
}
```

Hard invariant:

```text id="lowering-invariant"
Never lower an aggregate and keep only .aggregate.
Dropping .filter silently un-filters FILTER (WHERE ...) aggregates.
Dropping .order_bys corrupts order-sensitive aggregates
  (array_agg(x ORDER BY ...), first_value/last_value, nth_value).
Wire all three into the aggregate execution node.
```

Introspection happens on the inner `AggregateFunctionExpr` (via `lowered.aggregate`), whose accessors include `fun() -> &AggregateUDF`, `expressions() -> Vec<Arc<dyn PhysicalExpr>>`, `name() -> &str`, `human_display() -> Option<&str>`, `is_distinct()`, `ignore_nulls()`, `field() -> FieldRef`, `state_fields() -> Result<Vec<FieldRef>>`, `order_bys() -> &[PhysicalSortExpr]`, `create_accumulator()`, `groups_accumulator_supported()`, `create_groups_accumulator()`, and `reverse_expr()`.

## C13.22.3 `human_display()` is now `Option<&str>`

`AggregateFunctionExpr::human_display()` returns `Option<&str>` in 54 (it returned a plain `&str` in 53). The display text is user-facing EXPLAIN material — physical `EXPLAIN` output now shows lowered aggregates like `count(1) as count(*)` — and is **not** a stable identifier:

```rust id="human-display-pattern"
let display = agg.human_display().unwrap_or(agg.name());
```

```text id="human-display-rules"
Match/plan-diff on name(), not on human_display().
Snapshot tests over EXPLAIN text must tolerate the lowered form
  (e.g. `count(1) as count(*)`).
```

## C13.22.4 `AggregateExprBuilder` still exists

`LoweredAggregateBuilder` replaces the *logical→physical lowering* helpers only. `AggregateExprBuilder` (same module) remains the API for constructing an `AggregateFunctionExpr` directly from already-physical expressions — `AggregateExprBuilder::new(udaf, physical_args).schema(schema).alias(name).with_ignore_nulls(...).distinct().build()` — and is what C13.6's distributed-readiness patterns and the accumulator tests in C13.17 continue to use. Choose by input representation:

```text id="which-aggregate-builder"
Have a logical Expr (with alias/FILTER/ORDER BY to preserve)?
  → LoweredAggregateBuilder (returns aggregate + filter + order_bys)
Have physical exprs + an AggregateUDF already?
  → AggregateExprBuilder (returns AggregateFunctionExpr)
```


