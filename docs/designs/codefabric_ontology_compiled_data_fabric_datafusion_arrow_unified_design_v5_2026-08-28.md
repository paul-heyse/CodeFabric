---
artifact: design-dossier
design_id: codefabric-ontology-compiled-data-fabric
version: v5
date: 2026-08-28
status: accepted
baseline_commit: 71a888fed8aae660f97a8bc420f04a039f5aacae
working_tree_digest: e14f4dd603e042d5b0eaccf0889e915c6962fc0a3bafe9facda77c66797d67de
supersedes: docs/designs/codefabric_ontology_compiled_data_fabric_datafusion_arrow_unified_design_v4_2026-08-28.md
review_source: docs/reviews/implementation_review_codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27_2026-08-28_v1.md
comparative_source: docs/designs/codefabric_ontology_fabric_datafusion_governance_remediation_design_v1_2026-08-28.md
primary_scope:
  - src/compiled_ontology.rs
  - src/domain_conformance.rs
  - src/ontology_activation.rs
  - src/ontology_plane.rs
  - src/ontology_rules.rs
  - src/schema_registry.rs
  - src/semantic_query.rs
  - src/snapshot.rs
  - src/snapshot_runtime.rs
  - src/daemon.rs
  - src/fabric/
  - src/bin/codefabric_model/
  - src/generated/
  - contracts/schema/
  - contracts/registry/
  - contracts/generated/
  - tooling/ci/
  - scripts/
  - rules/
  - rule-tests/
  - docs/upfront_design/
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
governing_principles: docs/library_ref/full_data_fabric_design_principles.md
---

# CodeFabric ontology-compiled data fabric — target design v5

This dossier reopens and supersedes v3 because the independent implementation review found
that its central ideas were not strong enough to prevent a plausible but non-causal
implementation. Generated rule metadata did not drive execution, recursive self-description
was reducible to a table census, domain analysis was bypassable, and Stage 2b depended on a
test-only, process-local route. The problem is therefore not just missing wiring. The target
must make the required causal connections architectural properties.

This v5 preserves v4's Arrow-native program, governed DataFusion execution, and unified
activation kernel after independently comparing them with the parallel governance-remediation
candidate in `comparative_source`. It tightens seven contracts where that comparison exposed
useful precision or a v4 ambiguity: non-cyclic bundle identity, deterministic IPC packaging,
the concrete ID-domain value lattice, fail-closed phrase binding, analyzer-bypass containment,
once-executed gate artifacts, and versioned gate-result checksums. It does not import the
parallel candidate's split Rust runtime authority, second activation route, narrow result pin,
wildcard analyzer, or permanent zero-UDF constraint.

Citation conventions used here:

- `IR-NNN` refers to the implementation review in `review_source`.
- `DF-P1` through `DF-P25` refer to `full_data_fabric_design_principles.md`.
- `SD-P1` through `SD-P31` refer to the numbered principles in
  `semantic_design_principles_holistic.md`.
- `df`, `df-plan`, `df-schema`, and `df-calc` refer to the pinned DataFusion 55 reference.
- `arrow` refers to the pinned Arrow 59 reference.
- `delta` and `delta-align` refer to the pinned delta-rs reference and alignment manual.
- Pattern IDs such as `MOD-02`, `LOG-04`, `TXN-03`, and `TST-12` are from the corresponding
  DataFusion/Arrow or Delta alignment manual.

No performance claim or performance baseline is part of this design. Resource boundedness,
cancellation, and spill correctness are required; comparative speed is not.

## 1. Executive decision

**Selected target: one Arrow-native ontology program, compiled into one governed DataFusion
planning and execution pipeline, surrounded by one small application-owned durability and
authorization kernel.**

DataFusion becomes the relational compiler and executor for ontology closure, publication
validation, phrase calculations, result shaping, and query execution. Arrow becomes the one
runtime representation for schemas, semantic fields, compiled program relations, inputs,
violations, closure results, and delivered batches. Delta remains the authority for each
table's immutable committed versions. SQLite remains the authority for the cross-table owner
decision and active serving pointer.

This is deliberately **DataFusion-first, not DataFusion-sovereign**. DataFusion can analyze,
optimize, and execute a relational program; it cannot authenticate an owner, decide that a
candidate is acceptable, atomically coordinate twenty Delta tables with an operational
pointer, preserve leases, or reconcile an unknown commit outcome. Making those claims would
repeat the review's central error by confusing an execution mechanism with a governance
authority.

The clean target is:

```text
governed registries + Schema Contract IR
                 |
                 v
       OntologyProgramCompilation
                 |
                 v
 content-addressed Arrow OntologyProgramBundle
   | schemas and ontology relations
   | normalized typed program relations
   | calculation/function contracts
   | result and compatibility contracts
   | activation-policy references
                 |
                 v
      DataFusion ProgramCompiler
   bind -> analyze -> authorize -> optimize
                 |
                 v
       bounded Arrow stream execution
                 |
        +--------+---------+
        |                  |
 violation/closure   governed query result
 receipts                   |
        |                    v
        |              lease-pinned schema
        v              and checksum version
 durable ActivationKernel
 authorize -> CAS -> commit -> recover
```

The principal reversals from v3, retained by v5, are explicit:

| v3 decision | v5 decision |
|---|---|
| generated Rust `RuntimeCompiledOntology` is the runtime semantic object | a content-addressed Arrow `OntologyProgramBundle` is the runtime semantic artifact; Rust views are derived adapters only |
| coarse `CompiledRuleOperationKind` metadata plus operation-specific lowering | normalized, closed, typed relational-program tables plus one exhaustive DataFusion lowerer |
| one analyzer rule attempts to infer domains from selected expressions | every runtime plan is sealed through one compiler and an exhaustive DataFusion analyzer fails closed over every `Expr` and `LogicalPlan` variant |
| recursive discovery promises no fixed table-name knowledge | one honest, versioned bootstrap relation is fixed; every other relation and program is discovered and validated from it |
| generic activation and Stage-2b activation are separate callable routes | one `ActivateCandidate` command and one durable state machine classify all manifest changes; no ontology-capable bypass exists |
| a caller constructs dossiers and carries activation state | authoritative constructors create opaque, candidate-bound receipts; durable state is reconstructed after restart |
| result V1/V2 exists beside a global V2 runtime selection | every lease pins its result-contract set and checksum algorithm |
| Stage-0 probes preselect architecture branches, including a performance baseline | conservative pinned choices are fixed now; future observations cannot create owner decisions and performance baselining is absent |

V5 strengthens those decisions without changing their ownership model:

| v4 ambiguity | v5 resolution |
|---|---|
| the bootstrap relation, its own digest, and the bundle identities could form a cycle | bootstrap rows cover every non-bootstrap relation; the candidate manifest pins the bootstrap digest; the semantic program digest and package digest are computed afterward in that order |
| byte-identical IPC was required without a packaging contract | logical bundle identity and versioned deterministic IPC packaging are specified separately |
| `DomainEffect` named policy but not the value-state algebra | `DomainState = Domain(id) | Neutral | Bottom | Opaque` is defined for every pinned expression and plan variant while exhaustive matching remains mandatory |
| phrase bindings were compiled but unmatched behavior was implicit | an unknown or unbound phrase is a typed error; omission or an empty predicate can never represent unsupported semantics |
| analyzer installation could be mistaken for universal ingress enforcement | the sealed adapter denies statement/DDL/DML and raw-optimizer paths that DataFusion dispatches outside the normal analyzer lifecycle |
| gate metrics and receipt identity were not sharply separated | one execution yields a semantic checksum first and diagnostic metrics afterward; observed metrics never alter candidate acceptance identity |
| receipt checksum semantics were named but not fully specified | a versioned, resource-bounded `GateResultChecksum` reuses the released canonical Arrow row-multiset discipline |

This target resolves the review findings by construction:

| Finding | Design resolution |
|---|---|
| IR-001 | D-13 executes a compiled semantic-closure program and emits a candidate-bound closure receipt; table count is not a success criterion |
| IR-002 | D-15 creates one production admin command and removes generic ontology-capable activation |
| IR-003 | D-14/D-15 make receipts opaque, canonical, durable, candidate-bound, and restart-recoverable |
| IR-004 | D-10/D-11 make program records causally generate every validation plan and execution receipt |
| IR-005 | D-12 seals all plan ingress and exhaustively classifies the pinned DataFusion expression and plan enums |
| IR-006 | D-16 pins result authority and checksum selection in every immutable lease |
| IR-007 | D-17 separates observation from accountable decision and removes self-authorizing branch tests |
| IR-008 | D-10/D-11 compile phrase bindings and calculations through the same program/function catalog |
| IR-009 | the legacy matrix deletes the retired recipe and census registration |
| IR-010 | the transition supersedes plan v2, requires new proving commits, and forbids certification from the current dirty candidate |

This decision advances SD-P10 (declarative single-sourcing), SD-P14 (staged compilation),
SD-P17 (functional core/imperative shell), SD-P18 (generic runtime), SD-P19 (durable versus
temporal truth), SD-P20 (unified mutation), SD-P24 (idempotency), SD-P27 (provenance), and
SD-P31 (executable governance). It advances DF-P1–P3, DF-P6–P17, and DF-P20–P25.

## 2. Constraints and target invariants

### 2.1 Hard functional constraints

The following v3 outcomes remain non-negotiable:

1. The eight semantic request forms remain the only agent-facing query language. Storage
   schema, SQL, DataFrames, and DataFusion plans are not public request forms.
2. Every query pins one immutable snapshot and exact Delta version per table. A query never
   mixes publications or ontology bundles.
3. Explicit unknowns and capability gaps remain facts. Empty output never silently proves
   provider absence.
4. Result identity is partition- and batch-order independent and uses application-owned,
   versioned canonicalization. DataFusion plan text or optimizer shape is never identity.
5. Canonical CodeFabric identity remains application-owned. Provider-native keys and library
   object identities never become durable semantic identity.
6. Every delivered result resolves to its request, semantic program, result contract, leased
   snapshot, publication, ontology bundle, exact source tables, execution profile, and output
   identity.
7. The Python/FastMCP adapter remains presentation-only. Arrow, DataFusion, Delta, activation,
   and validation remain in the stable Rust daemon.

### 2.2 Pinned platform facts and ownership boundary

The target uses the current baseline without an upgrade: DataFusion `55.0.0`, Arrow/Parquet
`59.2.0`, `object_store` `0.13.2`, and delta-rs revision
`43a0cf10a313e5077c48637ad786a05359136bbb`.

The exact source and pinned references establish these load-bearing facts:

- DataFusion 55 exposes `AnalyzerRule::analyze`,
  `SessionStateBuilder::with_analyzer_rule`,
  `SessionStateBuilder::with_extension_type_registry`, `Expr`/`LogicalPlan` tree traversal,
  native expression and relational builders, a function registry, and scalar/aggregate/window/
  higher-order/table function families (`df` §§11, 19, 24; `df-plan` §§41, 46, 48;
  `df-calc` C1/C3/C9).
- At this pin the public `Expr` and `LogicalPlan` enums can be matched exhaustively. The
  semantic analyzer must not contain a catch-all arm, including one that maps future variants
  to `Opaque`; an added variant at a future pin therefore becomes a compile-time upgrade
  obligation. Known pinned variants whose semantics are unsupported are matched explicitly and
  fail closed.
- `SessionState::optimize` executes analyzer rules before logical optimization, but analyzer
  registration is not universal ingress enforcement. `SessionContext::execute_logical_plan`
  dispatches DDL and statement forms before that path, and `Statement::Prepare` invokes the raw
  optimizer. The sealed adapter must therefore reject DDL, DML, statements, `COPY`, `ANALYZE`,
  raw optimizer access, and direct query-planner access independently of `AnalyzerRule`.
- `ScalarUDFImpl::return_field_from_args` receives argument `FieldRef`s, so a genuinely custom
  calculation can validate and return contractual metadata. This does not make UDFs the
  default: built-in expressions remain preferred (`EXP-01`/`EXP-02`).
- Arrow extension metadata carries logical ID-domain identity and validates storage/name/
  version when the generated `ExtensionType` is actually invoked. Metadata alone does not
  enforce comparison policy (`arrow` §26; `df-schema` S7).
- Delta owns one table transaction and exact immutable versions. It does not provide a
  transaction across multiple Delta tables plus SQLite. CodeFabric continues to own retries,
  idempotency keys, predecessor checks, and unknown-outcome reconciliation (`delta` §§3, 5,
  6; `TXN-01`–`TXN-08`).
- DataFusion native plan/proto artifacts are engine-version coupled. They may be diagnostic or
  cache artifacts, never the durable semantic program (`df-plan` §§55–56).
- DataFusion physical metrics are available from the executed `ExecutionPlan`; complete harvest
  requires recursively following every `children()` edge after stream exhaustion. A scalar
  subquery execution node exposes both its primary input and its subquery plans. `EXPLAIN
  ANALYZE` is a separate execution and is therefore not an admissible gate-observation path.
- Arrow `RowConverter` bytes already participate in CodeFabric's released, versioned result
  checksum contract. They remain durable within that contract and its KATs; an engine or Arrow
  upgrade does not authorize refreshing an accepted checksum. An incompatible encoding requires
  a new checksum version and retained replay support. Engine/configuration identity is recorded
  separately as provenance rather than used to namespace semantic output identity.
- Deterministic Arrow IPC packaging is an application overlay. Arrow supplies IPC mechanics but
  does not select CodeFabric's relation order, row order, batch boundaries, dictionary
  normalization, schema-metadata ordering, or writer profile.

### 2.3 Non-goals

- No dependency upgrade.
- No raw SQL, DataFrame, or arbitrary logical-plan API for agents.
- No attempt to make DataFusion DML or a Delta commit advance the serving pointer.
- No new graph-traversal semantics; existing `GraphOperatorPlan` remains outside this
  relational-ontology remediation.
- No performance claim, benchmark baseline, view-type experiment, join-elimination claim, or
  physical-plan customization.
- No physical-schema redesign beyond what is needed to validate exact Arrow extension metadata.
  Source spans remain on the conservative flat representation in this release.
- No durable serialization of DataFusion plans as ontology authority.
- No `EXPLAIN ANALYZE` or `LogicalPlan::Analyze` on query, validation, closure, or proof paths.
- No engine-version namespace that changes a semantic output checksum without an explicit new
  checksum contract version.

### 2.4 Target invariants

The new invariants refine and supersede v3 TI-7–TI-9 while maintaining v3 TI-1–TI-6 where
not explicitly revised.

- **TI-10 — Arrow-native compiled authority.** One content-addressed
  `OntologyProgramBundle` is the complete runtime semantic artifact. Generated Rust values,
  ontology Delta tables, function registrations, schemas, and documentation are projections
  that carry its digest; none reparses owner authorities or redeclares semantic rows. Bundle
  identity has a non-cyclic bootstrap calculation, and its deterministic IPC package conforms
  to an explicit versioned packaging profile rather than defining identity by raw file bytes.
- **TI-11 — DataFusion causal execution.** Every mechanically relational validation, closure,
  phrase predicate, result projection, and calculation is bound from one typed program record
  and lowered exactly once to native DataFusion `Expr`/`LogicalPlan` or to a declared function
  contract. Removing or changing the program record changes execution and its receipt. The
  initial native-operation profile is proven expressible entirely with built-ins; an unknown or
  unbound phrase is a typed error, never an empty code set or omitted predicate.
- **TI-12 — Fail-closed semantic planning.** No executable DataFusion session escapes the
  adapter. Every authorized plan passes one generated policy and domain analyzer whose value
  lattice is `Domain(id) | Neutral | Bottom | Opaque`. Every pinned expression and plan variant
  is matched explicitly; unsupported known forms fail closed, and a new library variant fails
  compilation. Statement/DDL/DML dispatch and raw optimizer/planner paths are denied at the
  adapter boundary rather than assumed to converge through the analyzer.
- **TI-13 — Semantic self-description.** A single stable bootstrap relation discovers every
  non-bootstrap ontology relation and program. Its own exact schema/content identity is pinned
  externally by the candidate manifest. A closure receipt proves authority, schema, content,
  code/edge/type/table/column/result/identity/phrase/rule/publication/snapshot/plan linkage;
  counts and names alone can never satisfy it.
- **TI-14 — Candidate-bound proof.** Every receipt binds the canonical candidate manifest,
  exact table identities and versions, ontology program, result authority, engine profile,
  validation programs, inputs, outputs, and executable identity. Syntactically valid digests,
  plan text, filenames, and WP labels are not proof. Each gate executes once, commits its output
  through a versioned `GateResultChecksum`, and only then emits a `GateExecutionArtifact` whose
  plan displays and observed metrics are diagnostic and cannot alter acceptance identity.
- **TI-15 — One activation command.** All snapshot candidates use one classification and
  activation state machine. Ontology/result-authority changes require the active trusted
  policy's closure, proof, and owner-decision requirements. No generic bypass exists.
- **TI-16 — Durable idempotence and recovery.** Acceptance plus pointer CAS is one SQLite
  transaction. After any crash, durable records reconstruct active ontology state, result
  authority, and the process-local pointer. An identical retry returns the committed outcome;
  conflicting bytes under the same request ID fail permanently.
- **TI-17 — Lease-scoped compatibility.** Every lease pins a result-contract set, query-form
  binding, schema generation, checksum algorithm, ontology bundle, function catalog, and policy
  digest. Old and new leases remain semantically independent across cutover and restart.
- **TI-18 — Bounded shared execution.** Candidate validation and serving use explicitly built
  `SessionState`/`RuntimeEnv` instances with governed memory, spill, cancellation, and catalog
  scope. A validation candidate uses one session, not one default context per rule. The governed
  resource profile is receipt input; per-run metric values, timestamps, and spill paths are not.
- **TI-19 — Accountable decisions.** Capability observation, architecture decision, candidate
  validation, owner acceptance, and activation are distinct artifacts and operations. A test
  or probe cannot manufacture a reviewed decision.

## 3. Target architecture

### 3.1 Responsibilities and dependency direction

```text
Authored authorities
  YAML/JSON registries + Schema Contract IR
             |
             v
OntologyProgramCompilation                 pure, deterministic, application-owned
             |
             v
OntologyProgramBundle                      Arrow schemas + immutable RecordBatches
             |
      +------+------------------+
      |                         |
      v                         v
Delta publication adapter       DataFusion ProgramCompiler
exact per-table versions        bind native Expr/LogicalPlan
      |                         |
      +-----------+-------------+
                  v
        FrozenCandidateSession
        exact catalog + bounded RuntimeEnv
                  |
                  v
    violations / closure / result Arrow streams
                  |
                  v
    application-owned canonical receipts
                  |
                  v
         durable ActivationKernel
                  |
                  v
          immutable ServingEpoch
```

Dependency direction is inward toward application semantic contracts:

- authored authorities know no Arrow, DataFusion, Delta, SQLite, or daemon type;
- the compiler knows Arrow schemas and the engine-neutral program vocabulary, but not
  `SessionContext`, `DeltaTable`, or operational transactions;
- the DataFusion adapter consumes the bundle and produces execution artifacts, but cannot
  mutate activation state;
- the Delta adapter consumes schemas and write plans, but cannot choose the active publication;
- the activation kernel consumes opaque receipts and durable manifests, but never interprets
  row-level ontology semantics;
- query and admin transports call narrow command/query ports and do not construct receipts or
  plans.

### 3.2 D-10 — Arrow-native `OntologyProgramBundle`

**Decision.** Replace the runtime semantic authority embodied by generated Rust arrays and
coarse operation enums with one content-addressed Arrow bundle emitted by the model compiler.
Owner YAML/JSON and Schema Contract IR remain authored authorities; the Arrow bundle is their
single compiled runtime artifact.

The bundle contains immutable, schema-versioned relation families:

1. the existing ontology vocabulary and contract relations;
2. one stable `ontology_manifest` bootstrap relation;
3. closed typed relational-program relations;
4. calculation/function contracts;
5. phrase, rule, result, compatibility, and activation-policy bindings;
6. bundle provenance and invalidation dependencies.

The relational program is normalized without becoming EAV or an opaque JSON AST. Each legal
variation has its own typed relation and Arrow schema:

```text
program_contract
scan_node        filter_node       project_node
join_node        aggregate_node    set_node
column_expr      literal_expr      binary_expr
call_expr        case_expr         cast_expr
plan_edge        expression_edge
calculation_contract
rule_binding     phrase_binding    result_binding
```

Stable IDs join those relations. Node kinds, operand ordinals, semantic types, ID domains,
null policy, result role, deterministic aliases, diagnostic IDs, and calculation IDs are
ordinary typed columns. There is no property bag, arbitrary expression string, serialized SQL,
or runtime Rust match arm containing domain data.

The compiler emits a deterministic Arrow IPC bundle under `contracts/generated/` for runtime
and reproducibility use. Delta ontology tables and any generated Rust façade are derived from
those same batches. Generated Rust may contain schema-safe adapters and the one bootstrap
address; it must not duplicate program rows, rule operands, phrase semantics, or function
contracts.

Bundle identity is deliberately non-cyclic:

1. Canonical schema fingerprints and order-independent row-multiset content digests are
   computed for every **non-bootstrap** relation.
2. A domain-separated `bundle_content_set_digest` is computed over those relations ordered by
   stable relation ID.
3. `ontology_manifest` is constructed from that closed set. Its rows list each non-bootstrap
   relation's address, role, schema/content identity, required posture, and the shared content-
   set digest. They do not contain either final digest or a row describing themselves.
4. The finalized bootstrap schema and rows receive their own schema/content digest.
5. The logical `ontology_program_digest` is computed over the semantic-digest algorithm,
   bootstrap-contract version, bootstrap schema/content digest, and content-set digest.
6. The deterministic IPC file is emitted and a `bundle_package_digest` is computed over the
   program digest, packaging-profile version, and raw IPC digest. The candidate manifest pins
   both digests and the bootstrap table's exact Delta identity externally.

Logical identity is independent of file packaging. The versioned IPC packaging profile fixes:

- relation order by stable relation ID;
- row order by declared stable primary key, falling back only where declared to the released
  canonical row encoding;
- deterministic batch boundaries after sorting;
- dictionary normalization by logical value and declared canonical ordering;
- canonical schema/field metadata ordering and the exact IPC writer options;
- packaging profile, Arrow version, and feature identity.

Identical inputs must produce both the same logical program digest and byte-identical IPC under
the same packaging profile. The raw IPC and package digests prove artifact reproducibility but
are not semantic authority; a packaging-only revision changes package identity without changing
the logical program digest. This
advances DF-P1–P3, DF-P7–P12, MOD-01/MOD-04/MOD-06, ARR-01–ARR-03, and SCH-01/SCH-10;
it maintains SD-P7 (acyclic dependency structure), SD-P13 (stable identity), SD-P15
(canonicalization before optimization), and SD-P25 (reproducibility).

### 3.3 D-11 — One DataFusion program compiler and calculation catalog

**Decision.** One `OntologyProgramCompiler` is the only consumer allowed to turn program
relations into executable semantics. Its phases are explicit:

```text
decode bundle -> validate typed graph -> bind catalog/functions
 -> lower native Expr/LogicalPlan -> semantic analysis/policy
 -> DataFusion optimize -> physical plan -> bounded Arrow stream
```

The compiler uses the highest visible DataFusion surface that preserves semantics:

- built-in `Expr`, boolean/null/cast/conditional functions, joins, left anti-joins,
  aggregates, and set operations first (`EXP-01`, `EXP-02`, `LOG-01`–`LOG-06`);
- reusable expression builders only as construction helpers, never as hidden semantic tables;
- `ScalarUDFImpl` only for a genuinely custom vector calculation not expressible by built-ins;
- `AggregateUDFImpl`, window UDF, higher-order UDF, or table function only when the calculation
  contract's cardinality/state model requires that family;
- no custom logical or physical node for operations already expressible in native relational
  algebra; no custom physical operator in this design.

Every non-built-in calculation has a `calculation_contract` row containing stable ID, function
family, input/output semantic fields, coercion policy, null policy, volatility, strictness,
determinism, resource class, implementation identity, and diagnostic contract. Runtime
`FunctionRegistry` contents are generated from that relation and must be bijective with it.
`return_field_from_args` reattaches contractual field metadata for custom functions. Phrases
bind to program or calculation IDs; phrases themselves are not function names.

The first release carries a closed native capability profile over the currently governed
semantics. Its typed records cover scan/filter/project, inner and left-anti join, grouping and
count predicates, set operations, comparison/null/boolean/conditional expressions, casts, and
an `assert_empty` violation contract. The profile must carry every operand previously discovered
or embedded in Rust: governed-code columns, span groups, primary keys, value-kind/value-column
bindings, semantic-authority-to-dimension bindings, cardinality grouping sides, self-edge
policy, edge predicates, one explicit owner-resolution join path per owner rule, and the
diagnostic projection. Its completeness is a release census over the current rule, foreign-key,
and phrase authorities; it is not a replacement for the general normalized program graph.

The initial profile must lower entirely to optimizer-visible DataFusion built-ins. The expected
custom UDF count is therefore zero for this release. That is a proven property of the profile,
not a permanent architecture constraint: a later unexpressible calculation requires an
accepted `ExtensionDecisionRecord`, a typed calculation contract, selection of the highest
viable DataFusion function family, and the truthful metadata/resource hooks named above.

A phrase binding is total and fail-closed. Operator, operand domain, null policy, output role,
and diagnostic contract all participate in lowering. Three-valued behavior is explicit through
native null tests, `CASE`, `IS TRUE`, and `coalesce` as specified by the binding. An unknown
phrase, a missing binding, or an unsupported operation returns a typed `SemanticQueryError`;
none may silently produce an empty code set, omit a predicate, or return an empty result that
appears to mean "none".

All validation programs return one common generated Arrow relation:

```text
RuleViolation {
  candidate_id,
  program_id,
  rule_id,
  diagnostic_code,
  subject_table_id,
  subject_identity,
  related_identity?,
  detail_code,
  evidence_digest
}
```

Foreign-key, governed-code, and membership violations lower to left anti-joins. Uniqueness and
cardinality lower to grouping plus count predicates. Relation-family/owner/self-edge rules
lower to joins and filters. The property one-of and span all-or-none rules lower to native null
and arithmetic expressions. Rule plans are unioned into a stream; the execution ledger proves
that every required `rule_binding` ran exactly once. A changed binding necessarily changes the
logical plan digest namespace, output, or diagnostic and therefore the candidate receipt.

One candidate session registers all exact-version providers and executes every validation
program under one bounded runtime. The current fixed function call sequence and per-rule
default sessions are deleted. This advances SD-P10/SD-P18, DF-P2/DF-P14/DF-P15, MOD-02/
MOD-03, EXP-01–EXP-11, LOG-01–LOG-07, and TST-05/TST-06.

### 3.4 D-12 — Sealed plan ingress and exhaustive semantic analyzer

**Decision.** The sealed execution adapter and DataFusion analyzer together form the
unavoidable runtime enforcement boundary for the bundle's generated domain policy, while the
bundle remains the single semantic authority. `SessionContext`, `SessionState`, the raw
`Optimizer`, and the `QueryPlanner` are private to the adapter; callers can execute only an
opaque `GovernedPlan` returned by `OntologyProgramCompiler` or the semantic query compiler.

Semantic analysis separates the state of a value from the policy governing an operation:

```text
DomainState  := Domain(id) | Neutral | Bottom | Opaque
DomainEffect := None | Preserve | ConsumeSameDomain | Produce | ExplicitErase
```

`Bottom` represents a typed or untyped null and joins with the other branch state where the
operator's null semantics permit it. `Neutral` is a non-ID value. Equal `Domain(id)` values
join to that domain; different domains are rejected at comparison/alignment points. Any join
with `Opaque` remains `Opaque`, and an opaque value cannot participate in a comparison, join
key, membership predicate, set alignment, or delivered domain-bearing output.

The installed `FabricSemanticAnalyzer` performs an exhaustive recursive analysis after
resolution:

1. `TreeNode` walks every nested plan and expression, including all subquery plans and lambda
   bodies. `ExprSchemable::to_field` plus full `FieldRef` metadata recover the resolved semantic
   type and ID domain wherever DataFusion represents them.
2. Sources are explicit: columns, outer references, scalar variables/placeholders, literals,
   and scalar-subquery outputs yield `Domain`, `Neutral`, `Bottom`, or `Opaque` from their
   resolved field and generated policy.
3. Alias and declared domain-preserving operations preserve state. Cast/try-cast, negative,
   unnest, grouping, wildcard, lambda, and higher-order forms each have an explicit pinned-
   variant rule; a metadata-erasing or unsupported known form becomes `Opaque` or is rejected
   according to its generated `DomainEffect`.
4. Boolean/null predicates validate their operands and produce `Neutral`. Comparisons,
   `BETWEEN`, `IN` list/subquery, set comparisons, and join keys require compatible domain
   states. `EXISTS` and scalar/correlated subqueries recurse into their plans.
5. `CASE` joins branch result states after independently validating predicates. Generated
   contracts define whether `coalesce`/`nullif`/equivalent calls preserve a domain. Other scalar,
   aggregate, window, or higher-order calls consuming a domain must resolve through the
   calculation catalog; unknown calls fail closed. `count` is neutral, while only explicitly
   declared min/max/first-value-class contracts preserve a domain.
6. A match over every `LogicalPlan` variant checks projections, filters, joins, grouping,
   windows, union/set alignment, values, distinct, unnest/list elements, recursive/subquery
   forms, and extension nodes. Statement, DDL, DML, `COPY`, `ANALYZE`, and other mutation or
   control forms are rejected.
7. Every output field is checked through the generated Arrow extension factory, including exact
   extension name, storage width, metadata JSON, domain, recipe/version, and list-child field.

There is no wildcard arm, including a catch-all that returns `Opaque`. Because the pinned
`Expr` and `LogicalPlan` enums are exhaustively matchable, a future DataFusion variant becomes a
compile failure that forces an explicit upgrade decision. Known unsupported variants are named
explicitly and fail closed. `AnalyzerRule` may run more than once; a valid plan is unchanged and
double analysis is an oracle.

Analyzer installation is defense, not ingress proof. `SessionContext::execute_logical_plan`
dispatches DDL and statements before ordinary action-time optimization, and `PREPARE` can call
the raw optimizer. Direct SQL, arbitrary DataFrames, statements, DDL, DML, `COPY`, `ANALYZE`,
caller-created logical plans, raw optimizer calls, and direct planner calls are therefore
denied by the adapter's closed input type and structural governance, then denied again by plan
policy where they reach it (`LOG-07`, `GOV-03`, `TST-12`). Early semantic binding may report the
same error before lowering, but it consumes the identical generated `DomainOperationPolicy` and
is a diagnostic projection rather than a second authority.

This replaces the partial `expression_domain` recursion rather than expanding it case by case.
It advances SD-P11/SD-P12/SD-P16, DF-P12/DF-P13/DF-P15/DF-P20, SCH-05/SCH-06/SCH-09,
LOG-04/LOG-05/LOG-07, and GOV-03/GOV-04.

### 3.5 D-13 — Honest bootstrap and compiled semantic closure

**Decision.** Replace the impossible promise of recursion with no fixed knowledge by one
explicit, stable bootstrap contract:

```text
codefabric.cpg_ontology.ontology_manifest
```

Its address, Arrow schema, compatibility rules, and semantic-digest algorithm version are daemon
bootstrap knowledge. Its rows identify every **non-bootstrap** ontology/program relation by
stable relation ID, catalog address, semantic role, authority, required/optional status, schema
contract/digest, content digest, content-set digest, and exact candidate table identity. They do
not describe the bootstrap relation or carry either final digest. The candidate manifest
pins the bootstrap relation's own exact Delta version, schema/content digest, packaging profile,
logical program digest, and package digest. No other ontology table name or count is compiled
into discovery code.

The candidate manifest first opens exact Delta versions and freezes the catalog. The closure
compiler then reads `ontology_manifest`, resolves every declared provider from the frozen
catalog, validates its exact Arrow schema and content identity, and binds the bundle's closure
programs. Before execution it recomputes the non-bootstrap content-set digest, bootstrap
schema/content digest, logical program digest, IPC digest, and package digest through D-10's
ordered algorithm, rejecting any cycle, omission, or external-pin mismatch. DataFusion then
executes the programs over the catalog and produces normalized violations for:

- missing, duplicate, undeclared, or wrong-version relations;
- authority, bootstrap, content-set, program-digest, packaging-profile, IPC, or package-digest
  mismatch;
- unresolved governed codes or ontology edges;
- invalid semantic-type, ID-domain, identity-recipe, phrase, rule, and function references;
- incomplete table/column/result/query-form contracts;
- mismatched candidate publication/snapshot/result-authority linkage;
- missing or duplicate program execution receipts.

Success is an empty violation relation plus a `SemanticClosureReceipt` whose relation and
program census is derived from `ontology_manifest`, not from code. Adding a valid domain or
program row requires no resolver edit. Corrupting any relation family produces a stable
diagnostic. The receipt binds the exact candidate and is consumed by activation; it is not an
informational report.

This revises v3 TI-8 honestly: recursive semantics require a root. The root is minimal,
versioned, and testable instead of being hidden in generated constants. DataFusion supplies the
catalog hierarchy and relational execution (`CAT-01`–`CAT-04`); CodeFabric owns the root and
closure policy. This advances SD-P12/SD-P14/SD-P27, DF-P3/DF-P10/DF-P17, MOD-04, CAT-01–CAT-04,
OBS-05/OBS-08/OBS-12, and TST-11.

### 3.6 D-14 — Opaque, canonical, candidate-bound evidence

**Decision.** Replace public data bags and string-shaped proof maps with opaque application
types whose constructors perform the authoritative work:

```text
SealedCandidate
SemanticClosureReceipt
ValidationReceipt
GateExecutionArtifact
GateResultChecksum
OwnerDecision
ActivationPermit
CommittedActivation
```

Callers can read stable identities and diagnostics but cannot populate trust-bearing fields.
The candidate identity covers:

- workspace, publication, snapshot, predecessor snapshot, and expected pointer generation;
- canonical manifest digest;
- each table's code, canonical URI, Delta version, schema digest, PK digest, row count, and
  effective-content digest;
- ontology-program, authored-authority, schema, phrase, query-form, function-catalog, policy,
  result-contract, and checksum-algorithm digests;
- DataFusion/Arrow/Delta dependency identity and semantic session configuration;
- validation program set and resource-profile digest.

Every gate program follows one observation protocol:

1. Create one optimized logical plan and one physical plan under the frozen candidate session.
2. Execute once and fully drain the bounded Arrow stream while retaining the physical-plan
   graph.
3. Compute the version-selected `GateResultChecksum` from the output schema and complete row
   multiset.
4. After exhaustion, recursively visit every `ExecutionPlan::children()` edge and harvest each
   node's native `metrics()`. A scalar-subquery node's main input and all subquery plans are
   equally required children.
5. Render logical/physical displays and normalized metrics into a `GateExecutionArtifact`.
   Never execute `LogicalPlan::Analyze` or `EXPLAIN ANALYZE` to obtain them.

`GateResultChecksumV1` reuses the released CodeFabric result-checksum semantics under a distinct
integrity domain: JCS-canonical Arrow schema bytes, `RowConverter` logical row encodings sorted
without deduplication, duplicate multiplicity preserved, length-delimited hashing, batch/
partition/arrival-order independence, canonical-map validation, and an explicit maximum encoded
byte budget. The checksum algorithm version—not the DataFusion/Arrow version—selects semantic
identity. If an Arrow upgrade cannot reproduce V1 KATs, the system introduces V2 and retains V1
replay; it never refreshes accepted values. Engine, configuration, execution, and resource-
profile identities remain separate provenance fields on the outer receipt.

`ValidationReceipt` binds each program ID/version and digest to the candidate, input relation
digests, fixture or production-input identity, `GateResultChecksum` version/value, output
schema, violation count, execution status, executable/code identity, and reproducibility status.
`GateExecutionArtifact` binds the execution ID to bounded violation projections, output counts,
and normalized plan/metric diagnostics. Logical/physical displays and observed metrics are not
semantic or acceptance identity and may vary without changing the gate-result checksum
(`OBS-01`–`OBS-12`; `df-plan` §§55–56).

Development/release proof and runtime candidate validation are kept distinct. A release
`BuildProofReceipt` may bind a program-bundle release to independent executable oracles. A
workspace candidate receipt proves that the released programs ran over exact candidate data.
Runtime code contains no WP IDs, report filenames, plan paths, or hard-coded implementation
proof list.

Receipt digests are always recomputed server-side from typed content. A `b3:` prefix check is
never acceptance evidence. Delta commit metadata links per-table commits to candidate and
operation IDs, but retention-bounded Delta history is not the acceptance authority. This
advances SD-P12/SD-P24/SD-P25/SD-P27, DF-P9/DF-P10/DF-P18/DF-P19, MOD-06, OBS-01–OBS-12, and
Delta OBS-01–OBS-10. It maintains SD-P27 (derivation provenance) and SD-P28 (structured
observability) by separating reproducible semantic identity from execution diagnostics.

### 3.7 D-15 — One durable activation command and state machine

**Decision.** Ordinary fact candidates, ontology-bundle candidates, result-authority
candidates, and combined candidates all use one command and one state machine. Candidate class
is derived from the manifest difference against the active predecessor; callers cannot declare
a weaker class. The currently active trusted policy determines the required validation programs,
owner role, and compatibility checks so a candidate cannot waive its own gates.

The production ingress is a new closed `WorkspaceAdminCommand::ActivateCandidate` on the
existing same-user `0600` administrative UDS. The query UDS and FastMCP adapter have no
activation variant. Same-user peer authentication is necessary but not sufficient: the command
also resolves the actor against the durable workspace owner/authority registry and records the
policy version that authorized the decision.

The durable state machine is:

```text
DELTA VERSIONS WRITTEN
        |
        v
SEALED  exact immutable candidate manifest persisted
        |
        v  bounded DataFusion/Arrow validation
PROVED  closure and validation receipts persisted
   |          |                    |
   |          | predecessor moved  | owner rejects
   |          v                    v
   |        STALE               REJECTED
   |
   v authenticated owner accept command
ACTIVE  acceptance + predecessor retirement + pointer CAS in one SQLite commit
   |
   v successor wins
RETIRED
```

There is no separately durable `ACCEPTED` or `ACTIVATING` state. Acceptance without pointer
movement would be an illegal intermediate state. One short SQLite transaction:

1. resolves the request idempotency key;
2. reloads the sealed candidate, receipts, active policy, owner authority, pointer, and
   predecessor generation;
3. recomputes every trust-bearing digest;
4. verifies the exact predecessor and expected generation;
5. inserts the owner decision and complete activation record;
6. marks the predecessor retired and candidate active;
7. advances the active pointer and binds the active result authority;
8. appends an audit-event projection;
9. commits once.

Only an opaque `ActivationPermit` can call the low-level pointer transaction. The current
public generic `activate` and separate `activate_stage2b` routes disappear as authorities.
Ordinary fact publication receives a permit through the same classifier; ontology or result
changes receive one only after their stronger policy succeeds.

Crash/retry/concurrency semantics are fixed:

- Delta write ambiguity is reconciled by application transaction ID and exact reopened version
  before any retry; library retries remain zero where CodeFabric owns coordination.
- Validation is read-only. Cancellation or resource failure leaves no durable semantic change.
- Failure before SQLite commit rolls back completely and the same request is safe to retry.
- Lost response or crash after commit returns the original result when reconciled by request ID
  and candidate digest.
- The same idempotency key with different bytes is a permanent collision.
- Two candidates racing from one predecessor have one CAS winner; the loser becomes stale and
  must be reproved rather than silently rebased.
- A lease acquisition linearizes wholly before or after pointer commit.
- Process-local pointer/session installation is a cache convergence step. New lease acquisition
  remains unavailable until restart recovery reconstructs activation, ontology, result
  authority, and the `Arc` graph from durable records.
- Rollback is a new governed forward activation of a retained predecessor, never an unrecorded
  pointer edit.

This is the irreducible application-owned kernel. DataFusion and Delta are fully used below it,
but neither is misrepresented as a cross-table governance engine. It advances SD-P8/SD-P19–
SD-P24, DF-P11/DF-P13/DF-P16/DF-P23, and Delta MOD-07, STA-03/STA-10, TXN-01–TXN-08,
GOV-04, and OBS-01–OBS-10.

### 3.8 D-16 — Lease-scoped result, function, and policy authority

**Decision.** Extend `ServingSnapshotManifestBody` with a content-addressed
`ResultAuthorityPin`:

```text
result_contract_set_id + digest
query_form_result_binding_digest
schema_generation + schema_version
checksum_algorithm_id + version
public_wire_contract_version
function_catalog_digest
semantic_policy_digest
```

Lease acquisition constructs an immutable `ServingEpoch` containing those pins, the ontology
program, exact providers, extension registry, function registry, analyzer policy, and bounded
session. Query compilation, result shaping, metadata reattachment, and checksum dispatch select
only from that epoch. Global `.v2` schema IDs and `RESULT_CHECKSUM_VERSION` are forbidden in
serving code.

Old and new implementations coexist only as generated compatibility authorities:

- leases acquired before cutover continue with V1/current result contracts;
- leases acquired after cutover use V2/target contracts;
- held old leases survive activation and restart;
- rollback reactivates the predecessor with its original authority pin;
- V1 retires only when no active, leased, orphan-grace, recovery-eligible, replay-eligible, or
  rollback-eligible snapshot references it and a later plan explicitly removes its KAT.

This preserves exact snapshot semantics and makes compatibility a data-driven lease property,
not a process-global branch. It advances SD-P13/SD-P24/SD-P29, DF-P3/DF-P11/DF-P12/DF-P18,
SCH-01/SCH-10, RUN-01–RUN-03, and Delta STA-03/STA-10/QRY-03–QRY-06.

### 3.9 D-17 — Observation is not decision

**Decision.** Delete the self-authorizing Stage-0 probe mechanism. The findings it was intended
to decide are removed from this design's critical path by choosing conservative pinned behavior:

- keep the existing exact-version Delta provider wrapper rather than claiming an unproved
  `DeltaScanConfig` replacement;
- keep storage-typed literal rewriting at the Delta seam;
- keep source spans flat with the compiled all-or-none constraint;
- keep view types disabled;
- assume no unused-left-join elimination;
- validate Arrow extension metadata through generated factories and the manifest rather than
  assuming Parquet metadata supplies authority;
- make no performance claim and require no performance baseline.

Future capability investigation uses two independent artifacts:

1. `CapabilityObservation` records exact pins, feature graph, semantic session configuration,
   fixture/input digest, environment identity, command, raw evidence, and observed result. It is
   ephemeral or a review artifact and cannot alter design or state.
2. `OwnerDecision` names the observation digest, selected branch, actor authority, rationale,
   timestamp, and applicability envelope. It is written only by the accountable plan/design
   decision transaction; a test cannot create it.

Drift in pins, configuration, fixture, or evidence invalidates the decision's applicability.
This advances SD-P8/SD-P23/SD-P27/SD-P31, DF-P13/DF-P19/DF-P20/DF-P24/DF-P25, GOV-06–GOV-10,
and TST-12/TST-14.

### 3.10 D-18 — One bounded execution environment per candidate or serving epoch

**Decision.** Candidate validation constructs one `RuntimeEnv` and `SessionState` from the
governed resource profile, registers the frozen candidate catalog once, registers the exact
extension and calculation catalogs once, and executes every compiled program through it.
Serving epochs use the same construction policy with separate epoch identity.

The governed resource-profile digest covers memory-pool kind/limit, spill policy, timeout and
cancellation semantics, batch sizing, catalog/function/analyzer configuration, and deterministic
execution settings. Per-run task identity is recorded as provenance. Concrete spill paths,
timestamps, scheduler timing, counters, durations, and other observed metric values are
diagnostics and do not enter the semantic checksum or candidate-acceptance digest. A rule or FK
check may not call `SessionContext::new()` or install its own default runtime. Resource
exhaustion returns a stable resource error and leaves candidate/activation state unchanged.
Spill cleanup and cancellation are operational state, not semantic authority.

This advances SD-P22/SD-P23/SD-P28, DF-P16/DF-P20/DF-P23/DF-P24, RUN-01–RUN-10, PHY-11, and
TST-07/TST-10.

### 3.11 Failure and security semantics

| Failure class | Examples | Durable effect | Retry class |
|---|---|---|---|
| semantic compilation | malformed registry, invalid program graph, unknown calculation, unbound phrase semantics | no candidate | never until inputs change |
| semantic analysis | domain mismatch, metadata erasure, unsupported expression/plan form, sealed-ingress bypass | no execution | never for same program |
| candidate integrity | manifest/table/schema/content mismatch, broken closure | candidate remains non-active | never for same bytes |
| capability | unsupported Delta feature, result version, or function implementation | activation blocked | after runtime/config change |
| resource | memory/time limit, cancellation, spill failure, checksum-encoding budget | no semantic mutation | bounded safe retry |
| authorization | wrong peer, unknown owner, absent confirmation | no mutation; security audit projection | after valid authority |
| decision integrity | stale, replayed, or tampered decision | no mutation | new decision required |
| Delta conflict/ambiguity | OCC conflict or lost commit response | candidate incomplete until reconciled | reconcile first |
| SQLite pre-commit | busy, I/O, injected failure | transaction rollback | same request |
| pointer CAS | predecessor/generation changed | candidate stale | rebuild and reprove |
| SQLite unknown outcome | response lost after commit | durable state may be active | reconcile by request ID |
| local convergence | crash after commit before session swap | durable active; daemon not ready | recover, do not reactivate |
| retention | referenced Delta version unavailable | rollback/replay blocked | operator incident |
| programmer invariant | impossible state or receipt decoder defect | fail closed and quarantine | code fix |

Public errors expose a stable phase, code, retry class (`never`, `safe`, or
`reconcile-first`), diagnostic ID, and whether a durable commit may have occurred. Proof and
owner-decision contents are never accepted from the query socket. Plan artifacts are redacted
according to existing security policy and never contain credentials.

### 3.12 Library decisions

The capability research was performed inline with this dossier; no separate library report is
created. The exact local references and resolved pinned source were sufficient, so no network
research or dependency movement is required.

### LD-09 — Arrow 59 as the compiled ontology-program substrate

**Decision:** adopt

**Version basis:** Arrow/Parquet `59.2.0`; `Schema`, `Field`, `RecordBatch`, IPC, extension
types, typed arrays/builders, and compute kernels (`arrow` §§3, 5–8, 10–12, 26).

**Displaces:** runtime semantic rows and rule operands duplicated in
`src/compiled_ontology.rs`, `src/generated/compiled_ontology.rs`, handwritten result/row
shapes, and operation-specific Rust constants. Small generated adapters remain derived from the
bundle.

**Risk:** treating metadata or IPC bytes as authored policy, introducing a bootstrap digest
cycle, or allowing generated projections to drift. Mitigation: authored authorities remain
explicit, D-10's content/bootstrap/program/package ordering is acyclic, semantic identity is
independent of packaging bytes, every projection carries the logical program digest, and the
versioned IPC profile makes same-profile rebuilds byte-identical.

**Validation:** `just ontology-program-compiler-check`,
`just ontology-program-packaging-check`, and `just model-repro-check`.

### LD-10 — Native DataFusion 55 relational planning for ontology programs

**Decision:** adopt

**Version basis:** DataFusion `55.0.0`; `Expr`, `LogicalPlanBuilder`, native joins/anti-joins,
aggregates, set operations, `SessionState::{optimize, create_physical_plan}`, and Arrow stream
execution (`df` §§11, 19; `df-plan` §§41, 43, 45, 49).

**Displaces:** the fixed validation call sequence in `src/ontology_rules.rs`, per-rule
`SessionContext` construction, phrase-specific relational branches, and row-by-row custom
relational calculations.

**Risk:** DataFusion executes the supplied program but does not understand CodeFabric rule IDs,
authority, diagnostics, or receipt identity, and an underspecified "closed" algebra could hide
special cases. Mitigation: one general normalized program graph, one typed lowerer, a common
violation schema, and a complete first-release native capability census own those application
semantics; built-ins remain visible to the optimizer.

**Validation:** `just ontology-program-causality-check` and
`just ontology-relational-closure-check`.

### LD-11 — DataFusion calculation and function families behind one generated catalog

**Decision:** wrap

**Version basis:** DataFusion `55.0.0` built-ins, `FunctionRegistry`, `ScalarUDFImpl`,
`AggregateUDFImpl`, window/higher-order/table function contracts, and
`return_field_from_args` (`df` §§12, 24; `df-calc` C1/C3/C5–C10/C13).

**Displaces:** handwritten phrase calculation dispatch and ad hoc custom kernels. It does not
wrap transparent built-in expressions merely for uniformity.

**Risk:** UDF overuse can hide semantics from the optimizer; function-name registration is not
phrase or governance authority. Mitigation: the initial profile proves zero custom UDFs, any
later UDF requires an accepted `ExtensionDecisionRecord`, and every admitted function has an
explicit family/semantics contract, stable ID, truthful volatility/null/resource hooks, and a
registry-to-runtime bijection.

**Validation:** `just ontology-calculation-catalog-check` and
`just semantic-query-conformance-check`.

### LD-12 — DataFusion analyzer and plan-policy hooks as runtime enforcement

**Decision:** adopt

**Version basis:** DataFusion `55.0.0` `AnalyzerRule`, `TreeNode`, `ExprSchemable`, exhaustive
`Expr`/`LogicalPlan` variants, `SessionStateBuilder::with_analyzer_rule`, and the extension-type
registry (`df-plan` §§46, 48; `df-schema` S7).

**Displaces:** partial domain inference in `src/domain_conformance.rs`, binder/path-local policy
switches, and unsealed direct plan execution.

**Risk:** analyzer registration alone does not create domain semantics, function/type coercion
can erase metadata, and DataFusion dispatches some DDL/statement paths before ordinary analyzer
execution. Mitigation: the Arrow program supplies the concrete value lattice and operation
policy, all ingress and raw optimizer/planner access are sealed, matches are exhaustive and
wildcard-free, unsupported known forms fail closed, custom functions return full fields, and
output schemas are independently validated.

**Validation:** `just id-domain-plan-enforcement-check` and `just governance-scan`.

### LD-13 — Delta exact versions as table authority, not activation authority

**Decision:** retain-current

**Version basis:** delta-rs revision `43a0cf10a313e5077c48637ad786a05359136bbb` exact-version
loading/providers, per-table OCC, commit metadata, application transactions, and history
reconciliation (`delta` §§3, 5–7).

**Displaces:** nothing in the table storage seam. It explicitly rejects treating a Delta
transaction, history entry, or DataFusion write plan as multi-table acceptance.

**Risk:** automatic retries or retention-bounded history can obscure an unknown outcome.
Mitigation: CodeFabric uses operation IDs, zero library retries where it coordinates, exact
reopen/reconcile, and a durable cross-table candidate record.

**Validation:** `just ontology-candidate-delta-binding-check` and the existing
`just data-fabric-stack-compat`.

### LD-14 — SQLite activation kernel over immutable Delta versions

**Decision:** build

**Version basis:** the existing operational SQLite store, snapshot lease machinery, and exact
Delta versions; neither pinned library supplies the required cross-system transaction.

**Displaces:** caller-owned `OntologyActivationState`, freely constructible dossiers, the
test-only Stage-2b route, acceptance-only audit rows, and generic ontology-capable pointer
activation.

**Risk:** split-brain between durable state and process-local session, or misuse of same-user
authentication as sufficient authorization. Mitigation: one transaction, recovery before lease
availability, opaque permits, owner-registry resolution, request idempotency, and CAS.

**Validation:** `just ontology-activation-recovery-check`.

### LD-15 — DataFusion plan serialization as diagnostic/cache only

**Decision:** reject as durable semantic authority; adopt only as a version-coupled diagnostic
or cache artifact.

**Version basis:** DataFusion `55.0.0` native proto/Substrait and plan-artifact guidance
(`df` §36; `df-plan` §§55–56).

**Displaces:** any proposal to persist optimized plan text/hash as the ontology program or
candidate proof. The Arrow program and application canonical receipt remain durable.

**Risk:** engine upgrades change serialized plans, display, optimizer shape, and hashes.
Mitigation: cache keys include exact engine/config/program identity; cache miss recompiles from
the Arrow program; semantic receipts never depend on plan bytes.

**Validation:** `just ontology-plan-artifact-boundary-check`.

### LD-16 — Arrow 59 row encoding for versioned gate-result checksums

**Decision:** wrap

**Version basis:** Arrow `59.2.0` `RowConverter`/`SortField`, Arrow schema Serde support, and
CodeFabric's released `ResultChecksumV1`/`ResultChecksumV2` contract in
`src/fabric/result_checksum.rs` (`arrow` §§3, 7–8; MOD-06, OBS-09).

**Displaces:** ad hoc violation-row hashing, plan-hash proof, engine-native digest functions,
and any receipt algorithm sensitive to batch, partition, or arrival order.

**Risk:** treating an engine version as permission to refresh durable identity, or conflating
observed physical metrics with semantic output. Mitigation: a frozen integrity domain and KATs
make row encoding part of the released checksum contract; incompatible evolution creates a new
checksum version with old-version replay; engine/config identity and metrics remain separate
provenance/diagnostics.

**Validation:** `just ontology-gate-result-checksum-check` and
`just ontology-gate-execution-artifact-check`.

## 4. Alternatives and clean-sheet challenge

### 4.1 Alternative A — Repair v3 in place

Keep `RuntimeCompiledOntology`, add operands to `CompiledRuleContract`, expand the analyzer,
wire `activate_stage2b`, and persist `OntologyActivationState`.

This has the smallest code change, but it preserves the architecture that allowed the review
failures: semantic data remains split between Arrow relations and Rust values; each new rule
kind tends to add a match arm; self-description and runtime semantics can drift; activation
remains a special path beside ordinary publication. It improves correctness without reaching
the requested unified fabric. Rejected.

### 4.2 Alternative B — Arrow program plus governed DataFusion execution

This is the selected design. It maximizes native relational planning, expression visibility,
function-family selection, optimizer use, Arrow contracts, and exact Delta providers while
retaining only the application responsibilities the libraries cannot supply.

Its costs are a richer model-compiler output and one generic Arrow-program-to-DataFusion
compiler. Those costs replace several independent runtimes and make causal tests possible. The
engine-independent program keeps the design reversible if DataFusion is ever replaced.

### 4.3 Alternative C — Custom Arrow ontology interpreter

Compile the same bundle but execute rule and phrase operations through custom Arrow kernels,
handwritten joins, and a bespoke scheduler. This would make semantic effects directly
controllable, but it recreates null logic, joins, aggregation, streaming, memory/spill,
optimization, metrics, and conformance behavior already supplied by DataFusion. It creates a
second query engine and violates DF-P14/DF-P15. Rejected.

### 4.4 Alternative D — Make DataFusion or Delta own activation

Use DataFusion DML/provider mutation or a final Delta commit as the activation decision. This is
superficially the most library-centric option and architecturally the least truthful. DataFusion
has no durable cross-table owner decision; Delta transactions are per table; neither owns the
SQLite pointer, leases, owner registry, or retry/recovery policy. It cannot satisfy TI-15 or
TI-16. Rejected.

### 4.5 Clean-sheet answer

If the current implementation did not exist, Alternative B would still be preferred. The
design starts from one semantic artifact, chooses native Arrow/DataFusion mechanisms at the
highest viable level, and introduces custom code only at two unavoidable seams:

1. compiling the application-owned typed ontology program into DataFusion objects; and
2. coordinating accountable cross-table activation and recovery.

The selected design does not preserve existing module boundaries, public constructors, test
fixtures, or probe machinery out of incumbency. The exact-version provider, frozen catalog,
operational transaction, and leases survive because the clean sheet independently requires
them.

### 4.6 Parallel governance-remediation candidate

The candidate in `comparative_source` develops a repair-in-place variant around generated Rust
`CompiledOntology`/`RulePlanSpec`, a specialized ontology acceptance route, a narrow result-
version pin, a default-to-opaque analyzer, and a permanent zero-UDF posture. Those core choices
are rejected because they preserve split runtime authority, a second mutation route, weaker
lease identity, hidden DataFusion upgrade obligations, and an unnecessary extension limit.

V5 does adopt its useful precision where it strengthens rather than reverses the clean target:
the concrete `DomainState` lattice, complete operand/diagnostic census for the initial native
profile, fail-closed unmatched phrases, post-exhaustion metrics traversal, single-execution gate
artifacts, and the explicit rejection of `EXPLAIN ANALYZE`. The comparative challenge also
exposed and caused correction of v4's bootstrap-digest cycle, unspecified IPC packaging, and
metrics-versus-identity ambiguity.

## 5. Transition, cutover, and legacy disposition

### 5.1 Program governance decision

Implementation plan v2 is no longer the correct execution authority for this target. Its state
and review remain immutable history, but its packets must not be marked complete or repaired by
quietly implementing v5 under v2 wording. After this dossier, the planning workflow must create
a new versioned implementation plan that declares this design and explicitly supersedes the v2
execution path.

No planning-time hashes are refreshed during design or partial implementation. The new plan
records its inputs once; execution accepts only named input evolutions through its state
transactions and proving commits.

### 5.2 Transition stages

#### Stage 1 — Compiled program foundation, no runtime authority

- Define and version the Arrow schemas for `ontology_manifest`, program node/edge relations,
  calculation contracts, bindings, and receipts.
- Extend `OntologyProgramCompilation` to emit a deterministic Arrow IPC bundle plus the existing
  ontology tables from the same batches.
- Implement the non-cyclic non-bootstrap/content-set/bootstrap/program/package digest sequence
  and the versioned deterministic IPC packaging profile.
- Generate only schema-safe Rust adapters and stable bootstrap constants.
- Prove current schemas and semantic rows can be represented without loss.

The current runtime remains authoritative in this stage. The new bundle is a comparison
artifact, not a second active authority. Exit invariant: non-cyclic identity, same-profile byte-
reproducible IPC, complete authority/input coverage, and no downstream direct registry parsing.

#### Stage 2 — Generic DataFusion compiler and bounded candidate session

- Implement the typed program decoder/validator and native DataFusion lowerer.
- Compile the complete initial native capability profile, including owner-resolution operands,
  diagnostics, and fail-closed phrase/null semantics; prove its custom UDF count is zero.
- Register the generated calculation catalog and exact extension factories.
- Execute current rules and phrase operations in a non-authoritative differential harness.
- Produce the common violation schema, once-executed `GateExecutionArtifact`, versioned
  `GateResultChecksum`, and program execution ledger.
- Replace per-rule default contexts with one bounded candidate session.

The temporary dual execution is fixture-only and owned by this migration stage. Existing
behavior supplies observations, not expected semantics; independent fixture expectations decide
correctness. Exit invariant: every program record has one plan and one receipt, every plan
result matches independently authored semantics, every gate plan executes once, an unmatched
phrase fails closed, and changing a record changes behavior.

#### Stage 3 — Sealed ingress and exhaustive DataFusion analysis

- Build the exhaustive `Expr`/`LogicalPlan` semantic-value/effect analyzer over the concrete
  `DomainState` lattice without wildcard arms.
- Route every semantic request, candidate program, serving-view plan, and internal authorized
  plan through the opaque execution adapter.
- Make direct session/plan execution, DDL/DML/statement dispatch, raw optimizer access, and
  direct planner access private and add structural governance against every bypass.
- Validate extension metadata through generated factories at Delta reopen, plan output, custom
  function output, list children, and delivered results.

Exit invariant: the analyzer ingress matrix covers every authorized route, explicit negatives
cover `PREPARE`/DDL/DML/`COPY`/`ANALYZE`/raw-optimizer/direct-planner paths, the compiler proves
the DataFusion 55 variant census is exhaustive, and unsupported domain-bearing operations fail
closed.

#### Stage 4 — Durable candidate, receipt, activation, and compatibility kernel

- Extend the immutable snapshot manifest with candidate and result-authority pins.
- Add durable candidate, semantic receipt, gate-result checksum, gate-execution-artifact,
  owner-decision, activation, and idempotency records without treating diagnostic metrics as
  acceptance identity.
- Add the production `WorkspaceAdminCommand::ActivateCandidate` route and owner authorization.
- Restrict the pointer transaction behind `ActivationPermit`.
- Recover activation, ontology, result authority, and local session together before readiness.
- Make result/checksum selection lease-scoped while preserving generated V1/V2 authorities.

This stage can land before the ontology program cutover because it strengthens the existing
snapshot path. Exit invariant: real file-backed SQLite and temporary Delta fault/concurrency
tests prove exactly-once activation and simultaneous old/new leases.

#### Stage 5 — Atomic semantic cutover

- Publish one non-active candidate containing the Arrow program-backed ontology plane, the
  externally pinned bootstrap relation, the non-bootstrap content set, and distinct logical
  program/package identities under one packaging profile.
- Reopen every exact version and construct the frozen candidate session.
- Execute and drain each semantic closure, validation, compatibility, and resource gate exactly
  once; compute its versioned result checksum, then harvest diagnostic metrics from the retained
  physical-plan graph.
- Obtain the independent owner decision through the admin command.
- Perform the single activation transaction.
- Acquire old and new leases, restart, reconcile the activation request, and prove forward
  rollback to the predecessor.

No packet before this stage advances the active pointer. Failure leaves the predecessor active.
After success, all active semantic rule/phrase/closure execution uses the generic DataFusion
compiler.

#### Stage 6 — Decommission and certification

- Delete the superseded semantic Rust data, fixed dispatch, caller-owned activation state,
  generic bypass, global result versioning, self-authorizing probes, and retired command surface.
- Delete every direct `SessionContext`/`SessionState` execution route, raw optimizer/planner
  route, `Analyze`/`EXPLAIN ANALYZE` gate path, accepting analyzer wildcard, and silent
  phrase-binding fallback outside the sealed compiler/adapter.
- Promote repeated invariants to `rules/` with positive and negative fixtures.
- Run decommission zero-state proof, dependency-closed packet/milestone gates, and repository
  final gates at proving commits.
- Amend the upfront design suite to describe the Arrow program, stable bootstrap root, unified
  activation, and lease-pinned result authority.

### 5.3 Rollback and recovery during transition

- Before Stage 5 activation, rollback is code/config reversion; non-active Delta versions may be
  abandoned without affecting serving.
- Stage 5 activation retains the predecessor manifest and exact Delta versions. Rollback is a
  new forward activation using the same policy and receipts, preserving a complete audit trail.
- Old query leases continue on their immutable epochs throughout. No process-global toggle can
  change their result schema or checksum algorithm.
- The Stage-2 fixture-only differential path is removed at Stage 5 exit. It is never a production
  fallback and has no latest-safe-removal date beyond the cutover packet.
- Generated V1 result compatibility is the only intentional post-cutover coexistence, bounded by
  D-16's lease/retention retirement condition.

### 5.4 Legacy disposition matrix

The code inventory was generated in this design session with:

```bash
ast-grep outline src/compiled_ontology.rs src/domain_conformance.rs \
  src/ontology_activation.rs src/ontology_plane.rs src/ontology_rules.rs \
  src/schema_registry.rs src/semantic_query.rs src/snapshot_runtime.rs src/fabric \
  --items exports --view names
```

Non-code surfaces were generated with:

```bash
rg --files contracts/schema contracts/registry scripts tooling/ci rules rule-tests \
  docs/upfront_design | \
  rg '(schema-contract-ir|ontology|phrase|query-form|gate-filter|data_fabric|stage2b|id-domain|result)'
```

| Current surface | Disposition | Exit condition |
|---|---|---|
| `CompiledRuleOperationKind` and operand-free `CompiledRuleContract` | **delete** | typed Arrow program relations are causally executed and mutation-tested |
| `RuntimeCompiledOntology` semantic arrays and generated row constants | **replace** | Arrow bundle is the runtime artifact; any Rust façade contains adapters only |
| `compiled_ontology()` public semantic accessor | **replace** | callers obtain a digest-checked `OntologyProgramBundle` handle |
| `ontology_plane.rs` batch builders | **reshape** | builders publish the canonical bundle batches rather than reconstructing semantic rows |
| `ontology_manifest` bootstrap rows and bundle identity | **reshape** | rows cover non-bootstrap relations only; the candidate pins bootstrap identity; program then package identities recompute acyclically under the packaging profile |
| `ontology_rules.rs` fixed validator sequence and rule-specific semantic functions | **delete** | generic program compiler and rule-execution bijection are active |
| `DomainConformanceRule` partial recursion | **replace** | the concrete `DomainState` lattice, exhaustive wildcard-free analyzer, and sealed-ingress bypass negatives pass |
| `schema_registry.rs` shared lowering, table contracts, generated extensions | **preserve / reshape** | consumes bundle schemas and validates exact extension metadata through factories |
| `semantic_query.rs` typed request/bind pipeline | **preserve / reshape** | phrase/result operations bind program IDs; execution returns `GovernedPlan` |
| `semantic_query.rs` phrase match arms, literal semantic branches, and unmatched fallbacks | **delete** | registry-only mutation causality, typed unknown-phrase rejection, and structural zero-state pass |
| `OntologyCandidateDossier` public fields and synthetic proof map | **delete** | opaque canonical receipt constructors are the only path |
| `OntologyActivationState` process-local authority | **delete** | durable activation recovery supplies all state |
| `activate_stage2b` public/special route | **delete** | unified admin command classifies ontology change and obtains a permit |
| generic `ServingSnapshotRuntime::activate` ontology-capable API | **replace / make internal** | only an `ActivationPermit` can advance the pointer |
| snapshot candidate/frozen catalog/provider graph | **preserve / reshape** | sealed candidate embeds result/policy/program pins and exact provider receipts |
| snapshot lease, orphan grace, retention, and vacuum safety | **preserve** | clean-sheet lifecycle requirement |
| `ServingSnapshotManifestBody` | **reshape** | includes `ResultAuthorityPin` and candidate/program identity |
| `ServingSnapshotRuntime::recover` | **reshape** | restores activation, program, result authority, and pointer before readiness |
| `result_checksum_v1`, `result_checksum_v2`, version dispatcher | **preserve / generate** | selected only from lease; V1 later retires by D-16 |
| `fabric/result_checksum.rs` canonical row-multiset encoder | **preserve / promote** | `GateResultChecksum` uses the same released encoding contract under its own integrity domain and KAT set |
| physical-plan metric traversal (`physical_metrics` / `physical_metric_map`) | **preserve / reshape** | one shared post-exhaustion diagnostic collector traverses every child, including scalar-subquery plans, without re-execution |
| global `RESULT_CHECKSUM_VERSION` serving selection | **delete** | no production call site; lease matrix passes |
| `fabric/publication.rs` validation orchestration | **reshape** | one compiled validation program set and bounded candidate session |
| `fabric/mutation.rs` operation IDs, Delta OCC, retries=0, reconcile | **preserve / extend** | commit metadata binds sealed candidate IDs |
| exact-version Delta provider wrapper and frozen catalog | **preserve** | correct pinned library seam |
| `WorkspaceAdminCommand` and same-user admin UDS | **reshape** | gains candidate activation with owner-registry authorization |
| query UDS/FastMCP command family | **preserve** | negative oracle proves no activation variant |
| `scripts/ontology_fabric_probe_suite.py` and self-authorizing tests | **delete** | conservative design decisions land; observation/decision contract replaces them |
| `id16-extension-contract-check` and gate-filter census entry | **delete** | per-domain gate and `just --list` zero-state pass |
| hard-coded WP proof IDs and hash-shape checks in runtime activation | **delete** | release proof and runtime validation receipts are separate typed artifacts |
| `schema-contract-ir.json` and registries | **reshape / preserve authority** | add program/function/result/policy models; remain authored authorities |
| `src/generated/*` | **regenerate** | no file is hand-edited and reproducibility passes |
| v2 implementation plan/state | **preserve as superseded history** | new plan activation is an explicit governance transaction |
| upfront FAB/LIFE/QRY/SUITE text | **reshape** | v5 target and contracts replace the v3 Stage-2b wording |

No `defer` disposition remains for a review finding. Temporary encapsulation is limited to the
Stage-2 fixture comparison and generated V1 result compatibility, each with a named exit
invariant above.

## 6. Proof strategy

The implementation plan must add recipes for any named check below that does not yet exist. A
passing legacy packet selector is not a substitute.

### 6.1 Invariant-to-oracle matrix

| Invariant | Required executable oracle |
|---|---|
| TI-10 Arrow-native compiled authority | `just ontology-program-compiler-check`; `just ontology-program-packaging-check`; dual isolated `just model-repro-check` |
| TI-11 DataFusion causal execution | `just ontology-program-causality-check`; mutate every rule/phrase/calculation binding and operand and prove the expected plan/result/diagnostic changes; reject unknown phrases |
| TI-12 fail-closed semantic planning | `just id-domain-plan-enforcement-check`; concrete lattice truth table, compile-time DataFusion variant census, and statement/raw-planner bypass negatives |
| TI-13 semantic self-description | `just ontology-self-description-check`; acyclic bootstrap/program/package-digest proof, relation-family corruption matrix, and additive-domain fixture with no resolver code change |
| TI-14 candidate-bound proof | `just ontology-candidate-receipt-check`; `just ontology-gate-result-checksum-check`; `just ontology-gate-execution-artifact-check`; mutate every manifest/program/session/proof field and require rejection |
| TI-15 one activation command | `just ontology-activation-route-check`; query/generic bypass negatives and admin owner-positive path |
| TI-16 durable idempotence/recovery | `just ontology-activation-recovery-check`; pre/post-commit crash, restart, identical retry, request collision, concurrent CAS, and rollback |
| TI-17 lease-scoped compatibility | `just result-authority-lease-check`; concurrent old/new leases before/after activation and restart |
| TI-18 bounded shared execution | `just ontology-runtime-resource-check`; every gate executes once; deterministic memory/time/cancel/spill failures leave durable state unchanged; observed metrics remain diagnostic |
| TI-19 accountable decisions | `just ontology-decision-integrity-check`; observation cannot create decision and drift invalidates applicability |
| complete decommission | `just ontology-datafabric-legacy-zero-state-check` plus compiler/type checks |
| integrated target | `just ontology-datafabric-integration-check` using real temporary Delta tables, file-backed SQLite, admin UDS, restart, and query leases |

### 6.2 Semantic and causality proof

- Independently authored fixture expectations enumerate valid and invalid rows for every rule
  family. The program compiler and existing implementation cannot generate their own oracle.
- Every program/calculation variant has positive, negative, null, empty, duplicate, order,
  partition, and diagnostic cases as applicable.
- For each rule, mutate/remove its program binding, governed-code column, span group, PK/value-
  kind mapping, semantic-authority dimension, cardinality side, self-edge flag, edge predicate,
  owner-resolution join path, and diagnostic projection as applicable; prove exactly that
  rule's plan, behavior, and receipt change. This distinguishes causal authority from metadata
  presence.
- Compare optimized and unoptimized DataFusion results, output schemas, and semantic checksums
  over adversarial plans. Optimizer-node names are diagnostic only.
- A plan execution ledger is anti-joined against required program bindings; missing or duplicate
  execution fails the candidate.
- Phrase-registry-only changes must alter both relational and graph-bound semantic compilation
  without handwritten code edits. Unknown, missing, or unsupported operator/domain/null/output/
  diagnostic bindings must return the typed semantic-query error and can never become an empty
  code set, an omitted predicate, or a false assertion that nothing matched.
- The first-release calculation catalog proves that every admitted operation lowers to the
  closed native DataFusion profile with zero custom UDFs. A future extension is admissible only
  with an accepted `ExtensionDecisionRecord` and complete typed semantics/resource contracts.

### 6.3 Analyzer and Arrow contract proof

- The analyzer source uses exhaustive matches over the pinned DataFusion `Expr` and
  `LogicalPlan` enums. A structural rule rejects every wildcard arm, including one that returns
  `Opaque`, so a new engine variant is a compile-time upgrade decision.
- Unit/property cases exercise the complete `DomainState` join table (`Domain`, `Neutral`,
  `Bottom`, `Opaque`) and every `DomainEffect` (`None`, `Preserve`, `ConsumeSameDomain`,
  `Produce`, `ExplicitErase`) before plan-level fixtures are evaluated.
- The corpus covers direct comparison, `BETWEEN`, `IN` list/subquery, set comparison,
  scalar/correlated subquery, joins, set operations, `CASE`, aliases, casts/try-casts,
  scalar/aggregate/window/higher-order functions, grouping, unnest/list children, literals,
  placeholders, outer references, and unknown functions.
- Every authorized plan constructor is executed through the installed session analyzer and the
  sealed adapter. Explicit negatives cover SQL statements, `PREPARE`, DDL, DML, `COPY`,
  `ANALYZE`, direct `execute_logical_plan`, raw optimizer calls, and direct planner calls.
- Missing name, wrong name, absent/malformed metadata, wrong domain/version/recipe, wrong width,
  and invalid list child are rejected through exact Arrow extension factories.
- Output metadata is verified after projection, custom calculation, Delta reopen, IPC/Parquet
  round trip, and delivered result shaping.

### 6.4 Gate execution, checksum, and metric proof

- A counting source/extension node proves each gate physical plan is executed and drained once,
  even when semantic output, plans, metrics, and diagnostics are all requested.
- Post-exhaustion collection recursively visits every `ExecutionPlan::children()` edge across
  scans, joins, aggregates, repartitions, and scalar subqueries. No `AnalyzeExec`,
  `LogicalPlan::Analyze`, or `EXPLAIN ANALYZE` path is present.
- `GateResultChecksumV1` KATs cover batch/partition/arrival permutations, duplicate
  multiplicity, empty output, nested values, canonical maps, schema changes, encoded-byte
  exhaustion, and replay under every retained checksum version.
- Engine/configuration/execution/resource provenance changes may alter diagnostic artifacts but
  do not alter a semantic checksum for identical logical schema and row multiset. Changing a
  governed result does alter it.
- Resource-profile inputs are acceptance identity; run IDs, spill paths, timestamps, counters,
  durations, and physical metric values are explicitly excluded and tested as diagnostics.

### 6.5 Closure, activation, and recovery proof

- Closure tests corrupt each manifest and relation family separately: address, exact version,
  schema, content, authority, code, edge, semantic type, identity, phrase, calculation, rule,
  result, publication, snapshot, and plan links. Separate cases corrupt the non-bootstrap
  content-set digest, bootstrap schema/content digest, logical program digest, packaging profile,
  IPC digest, and package digest, and prove that none of those values recursively includes
  itself.
- Activation tests use real temporary Delta tables and exact-version providers, a file-backed
  SQLite database, barriers for races, and subprocess termination at commit seams. `MemTable`
  unit tests remain useful but cannot certify recovery.
- Faults cover every Delta write/reconcile seam, before SQLite transaction, every transaction
  mutation, immediately before commit, lost response after commit, and process death before
  local session convergence.
- Existing predecessor and active leases are present during every destructive fault test.
- Concurrent activation has exactly one winner. Lease acquisition observes an entirely old or
  entirely new epoch.
- Synthetic hashes, plan text, test names, filenames, and self-asserted reviewer strings are
  rejected as proof.

### 6.6 Structural governance and zero-state proof

Promote these recurring invariants to tested `rules/` entries:

- no direct DataFusion execution, raw optimizer access, or direct planner access outside the
  sealed adapter;
- no `LogicalPlan::Analyze`, `AnalyzeExec`, or `EXPLAIN ANALYZE` gate instrumentation;
- no wildcard arm in the semantic `Expr`/`LogicalPlan` analyzer;
- no semantic phrase match arm without a program/phrase binding ID;
- no unknown/unbound phrase fallback to an empty code set, omitted predicate, or false result;
- no bare semantic code literal in predicate/program construction;
- no operation-specific ontology validator dispatch outside the generic lowerer;
- no default `SessionContext` construction in candidate validation;
- no public trust-bearing receipt fields or process-local activation authority;
- no global result/checksum version selection in serving;
- no ontology activation command on query/FastMCP protocols.

Each rule requires positive and negative fixtures. Final legacy proof combines:

```bash
rg --files --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**'
ast-grep run ... --inspect summary
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' ...
cargo check --all-targets
just --list
just gate-filter-census
```

The plan must spell out the exact structural/text patterns and declared candidate envelope.
Zero hits from one tool are insufficient.

### 6.7 Final validation posture

The final plan gate includes the new named semantic/activation checks, including packaging,
gate-result checksum, and gate-execution-artifact checks, plus `just artifacts-check`,
`just plan-status`, `just model-repro-check`, `just governance-scan`,
`just data-fabric-stack-compat`, `just query-determinism-check`,
`just semantic-query-conformance-check`, `just query-legacy-zero-state-check`, and
`just ci-pr` at committed proving HEADs.

No performance recipe, comparative benchmark, or performance baseline is required by this
design. If a later artifact makes a performance claim, it must define its workload and baseline
then; this design does not pre-spend that work.

## 7. Current-state evidence and reproducibility

The implementation review remains the finding authority. The following commands regenerate the
current-state facts that materially shaped this design:

```bash
git rev-parse HEAD
{ git diff HEAD; git ls-files --others --exclude-standard -z | sort -z | \
  xargs -0 shasum -a 256; } | shasum -a 256

ast-grep outline src/compiled_ontology.rs src/domain_conformance.rs \
  src/ontology_activation.rs src/ontology_plane.rs src/ontology_rules.rs \
  src/schema_registry.rs src/semantic_query.rs src/snapshot_runtime.rs src/fabric \
  --items exports --view names

ast-grep run -l rust -p '$R.activate_stage2b($$$A)' src tests --inspect summary
ast-grep run -l rust -p 'OntologyCandidateDossier::build($$$A)' src tests \
  --inspect summary
rg -n --hidden -g '!.git/**' -g '!target/**' -g '!docs/library_ref/**' \
  'id16-extension-contract-check|RESULT_CHECKSUM_VERSION|plan-owner-v2-implementation-authorization'

rg -n 'pub trait AnalyzerRule|fn analyze' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-optimizer-55.0.0/src/analyzer/mod.rs
rg -n 'with_analyzer_rule|with_extension_type_registry' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-55.0.0/src/execution/session_state.rs
rg -n 'pub struct ReturnFieldArgs|arg_fields|return_field_from_args' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-expr-55.0.0/src/udf.rs
rg -n 'pub enum Expr|pub enum LogicalPlan' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-expr-55.0.0/src/expr.rs \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-expr-55.0.0/src/logical_plan/plan.rs
rg -n 'pub async fn execute_logical_plan|Statement::Prepare|optimizer\(\)\.optimize' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-55.0.0/src/execution/context/mod.rs
rg -n 'pub struct ScalarSubqueryExec|fn children\(&self\)' \
  /Users/paulheyse/.cargo/registry/src/index.crates.io-*/datafusion-physical-plan-55.0.0/src/scalar_subquery.rs
rg -n 'RowConverter|result_checksum_v1|result_checksum_v2|MAX_RESULT_CHECKSUM' \
  src/fabric/result_checksum.rs
```

The baseline tree was intentionally dirty. The frontmatter digest covers `git diff HEAD`
concatenated with sorted untracked-file hashes before this dossier was created. It records
identity, not completion evidence.

## 8. Acceptance

The pinned load-bearing APIs have been verified in the exact local references and resolved
source; no target invariant depends on an unselected probe branch. A new implementation plan
must supersede plan v2, preserve the old state as history, and implement the transition and
proof obligations in this dossier without restoring the rejected duplicate authorities.

accepted
