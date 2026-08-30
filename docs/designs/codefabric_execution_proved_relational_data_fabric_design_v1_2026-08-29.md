---
artifact: design-dossier
design_id: codefabric-execution-proved-relational-data-fabric
version: v1
date: 2026-08-29
status: accepted
baseline_commit: 0ff3dc62adddb4ba1c6fae8469b7f776f768624a
reconciled_head: 4e94c22ad28e6b5a01b9eb3f55db74fc0d81d7fb
working_tree_digest: 47d40164b6e106486c8a5cca05eec33c409aee954c0f4962897c90ce3de62714
primary_scope:
  - docs/authoritative_design/
  - docs/library_ref/full_data_fabric_design_principles_v2.md
  - contracts/
  - src/
  - rustc-extractor/
  - pyrefly-sidecar/
  - codefabric-cpg-mcp/
  - rules/
  - scripts/
  - tooling/ci/
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
supersedes:
  - docs/designs/codefabric_ontology_compiled_data_fabric_datafusion_arrow_unified_design_v5_2026-08-28.md
predecessor_plan:
  - docs/plans/codefabric_ontology_compiled_data_fabric_implementation_plan_v3_2026-08-28.md
---

# CodeFabric execution-proved relational data fabric — target design v1

## 1. Executive decision

CodeFabric will make a hard architectural pivot to one epoch-scoped, relationally
self-describing data fabric. The semantic model, provider observations, canonical
facts, derivations, query vocabulary, policies, provenance, capabilities, and proof
results will all be Arrow relations registered in one immutable DataFusion session.
Execution will read those relations directly. A declaration that execution does not
read will not be retained as authority.

This is a clean-sheet selection. The current implementation and the superseded v5
design are evidence about requirements and failed approaches, not constraints on the
target. In particular, this design reverses the decisions to commit a generated
ontology-program bundle, keep YAML registries as authoring authorities, generate
parallel Rust/Python catalogs, maintain static censuses and fingerprints of live
surfaces, preserve low-fidelity provider DTOs, and let SQLite participate in semantic
activation authority.

The selected architecture is:

1. a minimal, intrinsically static Rust bootstrap metamodel and closed compiler
   algebra;
2. an immutable chain of accepted model migrations that produces typed Arrow model
   relations by execution;
3. a `FabricEpoch` containing one sealed DataFusion `SessionState`, one `RuntimeEnv`,
   and one catalog graph for an exact model epoch, source generation, provider set,
   table-version set, policy set, and proof result;
4. exact-version provider adapters that project current Tree-sitter, Ruff, Pyrefly,
   and `rustc_public` outputs into provider-native Arrow relations without a
   lowest-common-denominator compatibility layer;
5. generic model compilers that lower normalization, authority, derivation,
   semantic-query, policy, and proof relations into native DataFusion expressions and
   logical plans;
6. Delta tables and immutable Arrow segments as durable data-plane state, with an
   append-only Delta activation log recording exact selected epochs;
7. one fenced daemon writer per workspace, with SQLite used only for reconstructible
   temporal coordination such as queues, leases, retries, and local command progress;
8. an atomically swapped `Arc<FabricEpoch>` as the only serving handle;
9. one Rust daemon per workspace, exact-version extractor/sidecar processes where
   toolchain isolation is real, and a presentation-only FastMCP STDIO adapter; and
10. governance whose clauses are relational queries and whose results are produced by
    running independent oracles and causal mutations.

The phrase “one fabric” means one semantic authority, one catalog namespace, one
Arrow type universe, one mutation model, and one query/proof engine. It does not mean
collapsing all typed domains into one entity-attribute-value table, or pretending that
an analytical engine is a queue, lease manager, or process supervisor.

### 1.1 What remains functionally invariant

The high-level product contract remains intact:

- the ontology remains a fact substrate rather than an evaluative judgment system
  (`ONT §1`, `ONT §67`, and `ONT §86`);
- raw and normalized observations coexist, provider conflicts retain evidence, and
  absence never masquerades as proof (`GEN §5`, `GEN §12`, and `QRY §30`);
- the Python and Rust provider stack remains Tree-sitter/Ruff/Pyrefly and
  Tree-sitter/`rustc_public`, with petgraph only for objective graph algorithms
  (`GEN §4`, `GEN §7`, and `GEN §52`);
- agents retain semantic-first, compositional query forms, query-block dependency
  graphs, deterministic responses, provenance, bounded execution, explicit unknowns,
  and one-snapshot consistency (`QRY §4`, `QRY §21`, `QRY §22`, `QRY §31`, and
  `QRY §34`);
- the system remains a central Rust daemon with one FastMCP STDIO presentation process
  per agent over a private local RPC boundary (`LIFE §122` and `SRV §4`–`§6`);
- source capture, incremental invalidation, owner-scoped replacement, cancellation,
  crash recovery, fairness, and freshness barriers remain first-class lifecycle
  behavior (`LIFE §93`, `LIFE §99`, `LIFE §114`, `LIFE §124`, and `LIFE §157`–`§159`);
  and
- every query pins one immutable present-state epoch and cannot mix generations.

The realization clauses that require static registries, generated manifests, bundle
fingerprints, hand-maintained traceability, or bespoke plan catalogs do not remain
invariant. They must be replaced in the authoritative suite after this design is
accepted. Historical specifications, designs, plans, decisions, and released wire
contracts remain immutable history; they do not remain runtime authority.

### 1.2 Observed current-state evidence

At baseline commit `0ff3dc62adddb4ba1c6fae8469b7f776f768624a`, with the dirty-tree
identity recorded in frontmatter:

- the repository contains a roughly 24-thousand-line Rust model compiler, a separate
  model/governance tooling layer, and approximately six megabytes of committed
  generated model and adapter material;
- runtime `include!` paths consume generated registries, table specifications, result
  schemas, fact encoders, provider-kind catalogs, and the Arrow ontology bundle;
- the schema IR still represents substantial provider output as opaque binary JSON:
  Ruff semantic families use multiple `*_json` fields, Pyrefly is reduced to a module
  row containing type/callee/diagnostic payloads, and rustc MIR is reduced largely to
  names, counts, debug strings, and kind lists;
- `CoreFactEngine` validates but does not relationally expose most Pyrefly type-table
  semantics or rustc MIR semantics, so information already produced by the pinned
  libraries is cold evidence rather than queryable fabric data;
- current ontology relations 11–30 already demonstrate the useful nucleus of the new
  design: ontology/schema/rule relations can describe themselves and can be discovered
  through catalog rows, but the surrounding generated projection pipeline prevents
  that relational nucleus from being the only authority; and
- the independent implementation review records unresolved causality, activation,
  reconstruction, provider-scan, and production-path proof gaps in the superseded v5
  realization.

The baseline `just ci-fast` run was red before this dossier was written: `root-fmt`
reported formatting differences in the pre-existing edits to
`src/ontology_program.rs` and `src/schema_registry.rs`. This dossier does not modify or
format those files and does not treat that baseline failure as evidence about the new
design.

During the design review, repository `HEAD` advanced from the recorded baseline to
`4e94c22ad28e6b5a01b9eb3f55db74fc0d81d7fb`. The intervening commit was inspected: it
contains three localized remediation/lint edits in `src/daemon.rs`,
`src/fabric/snapshot_catalog.rs`, and `src/governed_session.rs`. It does not change the
inventory classifications, exact library surfaces, or target selection in this dossier;
the reconciled head is recorded separately rather than rewriting the original evidence
identity.

### 1.3 Outcome and non-goals

The outcome is a target design that an implementation planner can realize without
preserving dual authority or compatibility scaffolding. It is not an implementation
plan, a commitment to retain existing source-file boundaries, or a request to finish
the active v3 packets. The next plan must supersede that plan and schedule the hard
cutover and decommission as one dependency-closed program.

The redesign does not add arbitrary SQL to the public API, move domain execution into
Python, merge the nightly extractor into the stable root, turn provider-native IDs into
canonical IDs, add git history or runtime coverage to the ontology, or collapse typed
facts into an opaque universal record.

## 2. Constraints and target invariants

### 2.1 Precedence and design law

`docs/library_ref/full_data_fabric_design_principles_v2.md` is the sole design-principles
authority for this dossier. Its staticness test and P26–P36 materially supersede the
v1-era assumption that a checked-in registry, census, manifest, or fingerprint is
governance merely because generation and drift checks surround it.

The governing rules for this design are:

- model semantics must precede behavior and the model must be what execution reads
  (v2 P1–P2);
- each concept has one authority, and staleness is answered by re-derivation (v2 P3);
- Arrow is the canonical data boundary and DataFusion is the common relational
  execution substrate (v2 P7–P8);
- provenance is emitted by execution and closure is computed by a resolver (v2
  P9–P10);
- snapshots and state transitions are explicit, schemas are executable, and policy is
  enforced at the authoritative boundary (v2 P11–P13);
- the highest semantic extension is preferred and optimizer/validator visibility is
  retained (v2 P14–P15);
- derived artifacts are reconstructed, fingerprints identify but do not validate, and
  reproducibility is proved by re-execution (v2 P17–P19);
- capabilities begin unknown and become advertised only after an executable prover
  succeeds (v2 P20);
- only things that cannot change are declared, every declaration is causally
  load-bearing, and change is computed (v2 P26–P28);
- validation is relational, expectations are independent, and forget-to-synchronize
  paths are eliminated (v2 P29–P31);
- construction makes invalid state unrepresentable, the core is functional, all
  mutation uses one idempotent command path, dependencies point inward, and governance
  executes (v2 P32–P36).

### 2.2 Staticness allowlist

Static material is allowed only when stability is the semantics of the material. The
target repository has this allowlist:

| Retained declaration | Staticness class and reason | How execution uses it | Correctness proof |
|---|---|---|---|
| Pinned Cargo dependencies, lockfiles, toolchains, and provider revisions | Class 1: chosen build inputs | Compilation directly selects the exact API | resolved-graph and provider compile probes |
| Released `.proto` and public JSON request/response contracts | Class 1: external stability is their purpose | RPC and FastMCP boundaries compile/load these exact contracts | regenerate, compare, and cross-language interoperability |
| Minimal bootstrap metamodel and closed logical operator algebra | Class 1: changing them changes the compiler and requires a migration | session construction and generic plan compilers read them | exhaustive Rust types plus bootstrap/self-description equivalence |
| Released `FabricCompilerRelease` identity | Class 1: reducer/compiler/function semantics must be replayed with the implementation that defined them | every model/fabric epoch pins the exact release | exact-source/toolchain rebuild and cross-release migration proof |
| Accepted model migration events | Class 1: immutable human/product decisions with predecessors | replay produces the model epoch relations execution reads | isolated dual replay, relational invariants, and causal mutation |
| Independent semantic expectations and accepted release decisions | Class 1: their independence and historical acceptance are the value | proof sessions load them as separate relations | provenance separation, human review, and mutant sensitivity |
| Historical designs, plans, reviews, and frozen principle versions | Class 1 history, not runtime authority | no production execution dependency | structural rule forbidding production imports/reads |
| Stable canonical identity domains and released public symbols | Class 1 only after release | generic identity/wire encoders consume them | collision/domain tests and released-contract conformance |

The allowlist is intentionally small. A new static declaration must state which row it
fits, why re-derivation is impossible or inappropriate, and the executable oracle that
makes it causally load-bearing.

### 2.3 Derived and materialized state

The following are always derived from live model, catalog, runtime, repository, or
execution state:

| Derived concept | Target derivation |
|---|---|
| current tables, columns, types, and schemas | DataFusion `information_schema` plus live Arrow schemas |
| registered functions and operators | DataFusion/runtime registration introspection emitted by the same installer that registers them |
| capability status | executable proof over current model demands, provider observations, runtime registrations, and coverage |
| dependency and traceability closure | relational joins/recursive graph operation over model and source dependency facts |
| validation and governance status | invariant queries over the exact epoch; no persisted green flag is trusted |
| current artifact/package contents | filesystem/package query at the point of use |
| change sets and affected closure | repository/source facts joined to model dependency edges |
| plan identity and plan coverage | freshly compiled plan plus dependency/provenance rows |
| current epoch | the unique head of the validated append-only activation chain |
| current provider/API surface | compiled adapter registrations and exact-version probe relations |
| current performance/materialization policy | measured workload facts joined to semantic constraints |

Arrow IPC, Parquet/Delta files, provider batches, result artifacts, cached logical plans,
and rendered review material are Class 2 materializations. Each carries source/model
epoch identities, schema identity, provenance, and a named re-derivation oracle. None is
an authoring authority. DataFusion logical or physical plan serialization is diagnostic
or cache material only and is never required to reconstruct semantics.

The following Class 3 declarations are forbidden in the target: current registries,
censuses, suite/package manifests, hand-maintained traceability ledgers, stored
validation flags, mutable-tree fingerprints presented as correctness, capability lists
without proof, generated Rust/Python copies of the model, static function catalogs,
static provider-kind inventories, and current-state status files read as truth.

### 2.4 Target invariants

#### I-20 — One model authority

For a model epoch, one replayed set of typed model relations is authoritative. Runtime
schemas, views, normalizers, query bindings, policies, derivations, invariants, and
capability requirements are constructed from it. No YAML, generated Rust, JSON census,
or embedded Arrow bundle supplies a parallel answer.

#### I-21 — One serving epoch

Every accepted query holds one `Arc<FabricEpoch>` from admission through terminal result.
That epoch owns one sealed `SessionState`, one runtime, one catalog graph, exact Delta
versions, immutable overlay segments if present, model identity, provider identities,
policy identity, and proof status. No query consults a global “latest” during execution.

#### I-22 — One Arrow type universe

All stable-root, extractor, and sidecar data payloads use Arrow 59.2.0 schemas and IPC.
Canonical identifiers use fixed binary physical types with semantic extension metadata;
nested structures use Arrow list/struct/map types rather than JSON payloads. JSON exists
only at public JSON, human-authored fixture, or vendor boundary seams.

#### I-23 — Raw fidelity before normalization

Every provider observation used or retained is available in a typed provider-native
relation with exact provider version, provider-local identity, coordinates, and run
provenance. Normalized facts are a separate relational projection. A new provider API
shape is handled by an explicit adapter/schema migration, not silently dropped into an
opaque field.

#### I-24 — Model-compiled execution

One generic compiler per semantic concern consumes model rows and emits native
DataFusion `Expr`/`LogicalPlan` structures. Domain decisions do not live in match arms,
filename conventions, UI code, generated constants, or hand-built graph assembly.
Changing an active semantic model decision must change an independently observed plan,
result, authorization, unknown, or violation. Rejection alone proves causality only for
structural type/key/reference rows whose mutation makes the model ill-formed.

#### I-25 — One mutation path

All changes to durable fabric truth enter as an idempotent `FabricCommand`, including
source waves, provider publication, model migration, activation, rollback, compaction,
and retention. The command path owns authorization, validation, provenance,
reconciliation, and publication. Tests do not have a second production-state write path.

#### I-26 — One fenced writer; activation is an event and current is a query

An activation is an immutable row committed to the Delta control relation with its
predecessor, writer generation, operation ID, exact epoch pins, compiler release, and
proof receipt. One OS-released workspace lease plus the daemon's single command actor
permits exactly one writer; concurrent multi-daemon mutation of one workspace is not a
supported deployment mode. The current epoch is derived as the unique valid
activation-chain head. SQLite and in-memory state may cache that answer but cannot name
a different epoch.

#### I-27 — Proof is part of the epoch

An epoch cannot be activated until model invariants, schema contracts, provider
coverage, provenance closure, policy checks, independent semantic expectations,
resource-envelope checks, and activation-chain checks execute against that exact epoch.
A pass requires both zero violations and proved input coverage; missing capability is
`unknown`, never an empty pass.

#### I-28 — Public composition remains semantic and bounded

The public request remains the structured semantic query envelope. The daemon converts
request blocks into temporary Arrow relations, resolves them against the model, compiles
them to DataFusion plans, and executes the dependency DAG in one epoch. FastMCP never
parses domain semantics or executes Arrow/DataFusion logic.

#### I-29 — Optimizer and validator visibility

Relational normalization, authority selection, overlay consolidation, projections,
filters, joins, aggregations, and derivations use native DataFusion nodes. A custom
`TableProvider`, table function, analyzer rule, or physical operator is permitted only
when the semantic operation cannot be expressed correctly at a higher level. Every
provider truthfully forwards `ScanArgs`, including statistics requests, and truthfully
reports pushdown, constraints, partitioning, ordering, and statistics.

#### I-30 — Exact current APIs, explicit future migrations

Provider adapters compile directly against Ruff 0.0.7, Pyrefly 1.2.0 at revision
`1933169ad8ee9e4d4114112eb56ef0811fb0a094`, Tree-sitter 0.26.12 with the pinned
language grammars, and `rustc_public` on nightly-2026-08-18. The architecture does not
weaken present functionality to hide hypothetical future changes. An upgrade changes
the adapter, provider-native schema, model migration, fixtures, and proof together.

#### I-31 — Exact compiler release is part of epoch meaning

Every model and fabric epoch pins one `FabricCompilerRelease`. Reducer, metamodel ABI,
primitive implementation, function package, effective policy/configuration, provider
schema, library, toolchain, and wire-contract identities are replay inputs. Current code
never reinterprets an old migration implicitly; cross-release use is an explicit
migration producing and proving a new epoch.

#### I-32 — Authorization is a catalog-construction boundary

The full epoch catalog is internal. Every agent query receives an epoch-pinned child
session whose catalog contains only authorized public providers/views for its
`AccessScopeId`. No request-context filtering is delegated to DataFusion catalog methods
that do not carry request context, and no raw provider/session/plan handle crosses the
query port.

### 2.5 Authoritative-suite mechanism changes

The suite remains the authority for product behavior until it is versioned, but these
mechanisms must be revised before implementation completion:

| Existing mechanism | Behavioral intent retained | v2-aligned replacement |
|---|---|---|
| `SUITE AC-G-05`–`AC-G-08` generated artifact/registry/bundle authorities | executable contracts and release compatibility | minimal released wire sources plus replayed model relations and catalog queries |
| `ONT AC-G-70`–`AC-G-77` machine registries | precise ontology, properties, unknowns, projections, summaries | typed model migrations and current model views |
| `GEN §85`, `AC-G-36`, and static provider capability catalogs | honest capability/unknown reporting | demand-to-runtime proof joins per epoch |
| `GEN AC-G-38` model-pack format | explicit provider/model extensions | model migrations or foreign signed model inputs compiled through the same command path |
| `FAB §8`, `§11`, `§79A`, and `§110` registries and plan artifact bundle | typed schema, single derivation authority, reproducibility | self-describing catalog, model-compiled plans, emitted dependency/provenance, re-execution |
| `FAB §12`, `LIFE §102`, and JSON serving manifests | exact snapshot pinning and atomic visibility | relational `fabric_epoch` plus append-only activation events and sealed session handle |
| `QRY` phrase/error/query-form registries | stable semantic language and deterministic errors | released wire grammar plus model rows read by resolver/compiler |
| static transition, traceability, detector, and filter censuses | exhaustive governance and lifecycle legality | relational dependency/state/invariant models with executable oracles |

The implementation plan must include a versioned authoritative-suite rewrite before the
new runtime is declared conformant. It must not edit historical design or plan artifacts
to pretend the earlier mechanism was never adopted.

## 3. Target architecture — contracts, ownership, flows, and library/platform decisions

### 3.1 Architectural shape and dependency direction

```text
FastMCP STDIO adapter (presentation only)
                 |
                 v
released gRPC control contract + Arrow IPC data/result streams
                 |
                 v
workspace daemon / command and query shell
        |                         |
        | commands                | epoch-pinned queries
        v                         v
  FabricMutationPort        Arc<FabricEpoch>
        |                  /        |         \
        v                 /         |          \
  pure command reducer    model   DataFusion    proof/system
        |                 tables   catalog       relations
        v                    \       |          /
  Arrow candidate batches    \      |         /
        |                     generic plan compilers
        v                            |
Delta fact tables + immutable        v
Arrow segments + activation log   Arrow streams

provider edges:
  source image -> Tree-sitter/Ruff adapter ---------> provider-native Arrow
  source image -> Pyrefly sidecar (exact API) ------> Arrow IPC
  source image -> rustc extractor (nightly API) ----> Arrow IPC

dependency rule:
  transport/storage/provider adapters -> ports -> relational semantic core
```

The semantic core contains immutable values and pure transformations over Arrow batches,
model rows, commands, and the closed query/derivation algebra. The concrete
`DataFusion55Compiler` is an inward-facing adapter that maps that algebra directly to the
current DataFusion API; it is not a defensive compatibility facade. Delta, SQLite, gix,
notify, Tree-sitter, Ruff, Pyrefly, rustc, petgraph, tonic, and FastMCP are likewise edge
implementations. Exact provider/compiler adapters are intentionally version-coupled at
their outer edge; that coupling never enters canonical identity or public query
contracts.

The existing four build domains remain because each has a real toolchain or language
boundary: stable Rust daemon/data plane, dated-nightly rustc extractor, pinned Pyrefly
sidecar, and Python FastMCP adapter. No new Cargo root is justified.

### 3.2 D-20 — Replayed relational model authority

The authoritative model is not a checked-in current registry and not a generated Arrow
bundle. It is the deterministic result of replaying an immutable chain of accepted
`ModelMigration` events through a fixed metamodel.

The bootstrap metamodel is the smallest closed set required to describe and execute the
rest of the system:

| Metamodel family | Purpose |
|---|---|
| `model_epoch`, `model_migration`, `model_decision` | ordered accepted history, predecessor, owner decision, applicability |
| `semantic_type`, `relation`, `field`, `key`, `foreign_key` | logical schema and constraints independent of physical storage |
| `authority_rule`, `normalization_rule`, `unknown_rule` | provider precedence, raw-to-canonical mapping, conflict/absence behavior |
| `derivation`, `derivation_input`, `derivation_output` | calculation dependency graph and materialization posture |
| `program`, `program_step`, `step_input`, `step_output`, `primitive_binding` | compositions over compiled intrinsic primitives and their typed bindings |
| `query_form`, `phrase`, `phrase_binding`, `result_role` | semantic request resolution and result composition |
| `policy`, `invariant`, `oracle`, `capability_requirement` | enforceable governance, proof plans, and demanded capabilities |
| `projection`, `representation`, `public_symbol` | stable public views and released names |
| `state_machine`, `state`, `transition` | generic command/lifecycle legality |
| `materialization_policy`, `physical_binding` | epoch-specific strategy derived from measured facts, never semantic meaning |

A migration uses a typed Rust builder over this metamodel. It may add, supersede, or
retire model rows, but it cannot mutate prior events. The builder validates types and
references at construction. Replaying the chain yields Arrow `RecordBatch` values;
DataFusion invariant queries validate them; canonical ordering supplies a model identity
digest. The digest identifies the replay result but is never accepted as evidence that
the result is correct.

Intrinsic primitive semantics do not appear a second time as authored model rows. The
closed Rust algebra and compiled function implementations are their authority. The exact
installer registers them and emits the derived `system.intrinsic_primitive` relation.
Model rows own only compositions, typed bindings, semantic parameters, policy, and
expected outputs over those primitives. A closure query joins model bindings to the
derived runtime relation; it does not make a handwritten function registry authoritative.

Every replay is parameterized by a released `FabricCompilerRelease`. That immutable input
pins the source/build identity, metamodel ABI, command-reducer version, logical-algebra
version, exact DataFusion/Arrow/Delta versions, intrinsic function packages, provider
schema versions, policy/config schema and effective configuration identity, toolchains,
and released wire-contract set. The same migration bytes under a different reducer or
function package are not the same replay.

The current model is a view over migration events, not a census copied into source.
Materialized model tables may be stored in Delta for startup performance, but startup
replays or independently replays and compares them before use. A mismatch discards the
materialization and fails the candidate; it does not rewrite the migration history.

Product-default migrations are source-controlled Class 1 release inputs. Workspace model
extensions enter as signed/authorized model commands and are durable migration-event
rows. Both use the same reducer. Reconstruction uses the exact pinned compiler release;
if that release cannot be rebuilt from its source, lockfiles, and toolchains, the epoch is
unavailable rather than silently replayed by current code. A release upgrade explicitly
replays/revalidates under the old release, migrates to the new ABI, reruns independent
semantic expectations, and activates a new epoch.

The bootstrap metamodel self-describes after replay. A closure query compares the
hard-coded bootstrap fields used to start the session with the corresponding rows in
the completed model. Any difference fails model construction. This bounds the necessary
static core and prevents it from becoming a second expanding registry.

### 3.3 D-21 — Immutable `FabricEpoch`

Conceptually:

```rust
struct FabricEpoch {
    identity: FabricEpochId,
    compiler_release: FabricCompilerReleaseId,
    model_epoch: ModelEpochId,
    source_generation: SourceGeneration,
    provider_runs: Arc<ProviderRunSet>,
    table_versions: Arc<TableVersionSet>,
    overlay_segments: Arc<OverlaySegmentSet>,
    internal_session_state: Arc<SessionState>,
    access_sessions: Arc<AccessSessionFactory>,
    runtime_env: Arc<RuntimeEnv>,
    proof: Arc<EpochProof>,
}
```

This signature carries ownership, not an implementation prescription. The epoch builder
alone receives mutable registration handles. It installs catalogs, tables, views,
functions, analyzer rules, runtime limits, and object stores; runs closure and proof;
then seals the object behind an `Arc`. Production callers never receive a raw mutable
`SessionContext` or registration API.

The daemon owns `ArcSwap<FabricEpoch>` per workspace. Query admission loads the pointer
once and constructs a query lease. Activation swaps a fully proved epoch in one atomic
operation. Result artifacts, progress, cancellation, and status all carry the same epoch
identity. Epoch retirement waits for query/result leases and Delta/segment retention.

Construction uses DataFusion 55's in-memory catalog implementations and
`SessionStateBuilder`: a fresh catalog list, catalog provider, schema providers, runtime,
analyzer/optimizer rules, and query planner are assembled for the epoch. DataFusion does
not itself make a `SessionContext` immutable—its context retains mutable session state—so
immutability is a CodeFabric ownership rule enforced by keeping every registration
handle inside the builder and exposing only the sealed query facade.

The full internal session is never used for an agent query. `AccessSessionFactory`
constructs or reuses an epoch-pinned child `SessionState` keyed by an immutable
`AccessScopeId`. A child contains only authorized projected providers and views. It may
share immutable arrays/providers and a `RuntimeEnv` only inside the same memory, spill,
credential, and object-store trust domain. Per-request source restrictions that cannot be
represented by a cached policy class receive a request-scoped child session.

### 3.4 D-22 — Catalog is the runtime architecture

Each epoch has one DataFusion catalog named `codefabric`. Its schemas are roles, not
duplicated authorities:

| Schema | Contents | Backing/visibility |
|---|---|---|
| `model` | replayed metamodel and active model rows | immutable Arrow/Delta; daemon and proof |
| `source` | workspace, source image, coordinate, inclusion, context, and generation facts | exact Delta/Arrow versions; authorized daemon queries |
| `raw_tree_sitter` | CST nodes, fields, tokens, errors, queries, coordinates, grammar-native kinds | provider-native Arrow/Delta |
| `raw_ruff` | Ruff tokens, AST, scopes, bindings, imports, references, CFG/dataflow observations, diagnostics | provider-native Arrow/Delta |
| `raw_pyrefly` | contexts, modules, definitions, types, members, imports, call targets, diagnostics, unresolved observations | Arrow IPC to exact Delta/Arrow schema |
| `raw_rustc` | compilation, items, HIR-facing identities where exposed, MIR bodies/blocks/locals/places/operands/terminators/instances/dataflow and diagnostics | Arrow IPC to exact Delta/Arrow schema |
| `fact` | canonical typed entities, occurrences, relations, properties, unknowns, and universal provenance keys | model-compiled views/materializations |
| `derived` | graph, dataflow, alias, dominance, reachability, summary, and metric facts | native plans or a bounded logical extension/table function selected by input shape; materialized only by policy |
| `proof` | invariants, expectations, coverage, violations, provenance edges, proof runs, mutants, receipts | separately sourced expectation relations plus computed results |
| `system` | epoch, runtime, catalog, provider, capability, query, resource, spill, cache, and lifecycle observations | virtual/read-only providers over live state and temporal stores |
| `public` | ACL-filtered stable semantic response views | only surface visible to public query compiler |
| `_storage` | exact base tables and immutable overlay segments | private implementation schema, excluded from public information schema |

DataFusion's `CatalogProviderList -> CatalogProvider -> SchemaProvider -> TableProvider`
hierarchy is the namespace and discovery mechanism. Native `information_schema` supplies
catalogs, schemata, tables, columns, parameters, routines, and settings where supported.
Product-specific metadata that DataFusion does not expose is implemented as read-only
`system` providers. DataFusion 55 catalog/schema/provider lookup APIs do not carry a
request authorization context, so the full internal catalog is never request-filtered in
place. Built-in `information_schema` is internal-only. An authorized child session
contains only its permitted `public` views and either enables information schema over
that reduced catalog or exposes explicit ACL-filtered `public.metadata_*` relations.
Hidden tables and columns are absent from lookup as well as listing.

Durable relations use exact-snapshot delta-rs providers; immutable hot batches use
`MemTable`; optimizer-visible derived, overlay-consolidation, invariant, and public
relations use `ViewTable` after their plans have been type/analyzer validated. The
`ViewTable` constructor is not treated as a validator. A custom provider is reserved for
remote, policy-sensitive, streaming, or otherwise irreducible sources.

There is no generated table list or schema registry. `information_schema.columns` joined
to `model.field` proves catalog closure. The same pattern proves function, constraint,
projection, provider, and oracle closure. A missing row is a query result, not a forgotten
sync check.

Internal closure queries run only in the full session. Public metadata comes only from an
authorized child catalog; neither a raw provider handle, internal `DataFrame`, serialized
plan, nor table identifier crosses the query port.

### 3.5 D-23 — Exact provider-native Arrow boundary

Provider isolation is retained but redefined precisely: borrowed vendor values, compiler
contexts, database handles, and unstable internal IDs do not escape the adapter or
provider process. The adapter does not hide the current API behind a long-lived generic
DTO. It copies every API family required by the accepted ontology/provider boundary into
versioned, typed Arrow relations designed against the exact pinned API. Any intentional
omission is an independently reviewed model decision with an explicit remainder or
unsupported/unknown result; “we did not model it” can never be inferred from no rows.

The provider path is:

```text
exact source image + exact analysis context
  -> exact-version library API
  -> provider-native Arrow relations (raw, loss-minimized, versioned)
  -> model-compiled normalization and authority plans
  -> canonical fact relations + explicit unknown/conflict relations
```

Provider-native schemas keep raw kinds and provider-local keys, but those keys never
become canonical identity. Canonical IDs are computed only after coordinate,
source-instance, semantic-identity, and authority inputs are present. Raw and normalized
relations remain joinable through observation IDs and run provenance.

For Tree-sitter and Ruff in the stable root, adapters build Arrow batches directly while
their exact library objects are live. Pyrefly and rustc retain their process/toolchain
boundaries; Protobuf carries job/control/backpressure/cancellation metadata and Arrow IPC
carries data. No provider sends row-per-message Protobuf or JSON blobs for semantic
payloads.

Every provider run emits its actual observed schema, API revision, source/context pins,
requested families, completed families, diagnostics, and coverage. Capability starts
unknown. A proof plan joins demanded model families to completed provider relations and
only then emits supported/partial/unknown.

For each pinned provider release, an independently owned `ProviderBoundaryContract`
enumerates upstream API family/symbol, Arrow relation and fields, provider-local and
canonical identity roles, coordinates, authority role, retention policy, intentional
omission/remainder representation, unavailable behavior, and executable oracle. The
compiled adapter installer emits the derived `system.provider_surface` rows from its
actual handlers. Closure is the relational difference between the independent boundary
contract and those runtime rows plus fixture coverage—not a handwritten provider
registry.

Any custom provider implements both DataFusion scan entry points through one internal
`plan_scan(ScanArgs)` path. The legacy `scan` call constructs structured arguments with
no statistics requests; `scan_with_args` forwards projection, filters, limit, and
statistics requests without down-conversion. This prevents the compatibility default
from silently discarding the DataFusion 55 statistics channel.

### 3.6 D-24 — Generic DataFusion plan compilers

The application owns a small family of exact DataFusion 55 compiler adapters, each
consuming semantic-core relations and producing standard DataFusion plans:

| Compiler | Model inputs | Execution output |
|---|---|---|
| catalog assembler | relation/field/type/key/physical binding | Arrow schemas, providers, views, validated constraints |
| normalization compiler | provider relation, normalization, authority, unknown rules | raw-to-canonical projection, reconciliation, conflict/unknown plans |
| derivation compiler | derivation graph, primitive bindings, precision/materialization policy | relational derived plans and selected graph extension nodes/providers |
| semantic query compiler | request-block relations, query forms, phrases, bindings, projections | typed block DAG and parameterized logical plans |
| policy compiler | policies, catalog visibility, plan-node constraints, resource envelopes | analyzer validation and authorized catalog surface |
| proof compiler | invariants, oracles, expectations, coverage requirements | violation, coverage, causality, and receipt plans |

The compilers interpret model-owned compositions over the Rust-owned intrinsic algebra;
they are not per-domain dispatch tables and the model does not restate primitive
semantics. Model rows name relation/field IDs, derived runtime primitive IDs, typed
bindings, and parameters; they do not contain SQL fragments or Rust expression-display
strings. The compiler resolves those IDs against the live `DFSchema` and derived
intrinsic relation, constructs `Expr` and `LogicalPlanBuilder` nodes, validates the
output schema, optimizes, and executes as a stream.

Native projections, filters, joins, semi/anti joins, unions, windows, aggregates, sorts,
limits, and built-in functions are always preferred. Reusable calculation semantics use
the narrowest DataFusion function family that fits. A planning-time table function is
used only when scalar parameters can select an epoch-pinned provider and no upstream
relational child is required. An algorithm that consumes a relational subplan uses a
typed logical extension with that child and an `ExtensionPlanner`/physical implementation.

The extension ladder is explicit: built-in expression/kernel; scalar UDF; aggregate UDF;
window UDF; planning-time table function returning a provider; higher-order UDF only for
true collection/lambda semantics; and finally a custom logical node with extension
planner/physical plan. External I/O is a provider boundary rather than an asynchronous
scalar UDF in the reproducible core. A custom physical plan must expose and rewrite its
expressions/children and participate honestly in child statistics requests and
statistics propagation.

Analyzer rules are generic enforcement, not a second semantic model. The policy analyzer
reads the epoch's compiled policy values, walks every logical-plan and expression node,
including nested and subquery forms, and rejects unauthorized tables, functions,
unbounded traversal, hidden metadata, invalid result schemas, and missing resource
budgets. Optimizer rules may improve execution but cannot supply correctness that the
unoptimized plan lacks.

Plans are rebuilt from model and request relations. Plan serialization, `EXPLAIN`, and
physical metrics are evidence and caches; they are not durable semantics.

### 3.7 D-25 — Relational graph execution without relational pretense

CodeFabric remains a hybrid relational graph system (`FAB §5`). Canonical graph nodes
and edges are typed relations. Relationally expressible traversals, reductions,
aggregations, closures of bounded depth, and set composition stay visible as native
DataFusion plans.

Petgraph 0.8.3 is retained for SCCs, dominance support, unbounded traversal, and other
algorithms where its tested implementation is stronger than a custom relational fixed
point. DataFusion first performs every relationally visible selection/projection. For an
algorithm with a relational input, a typed `LogicalPlan::Extension` owns that child,
materializes only its output batches, uses canonical external IDs at the boundary, keeps
`NodeIndex` and graph storage private, checks cancellation and memory reservations, and
returns Arrow batches with provenance. Pushdown is visible only up to the extension
boundary; the design does not claim optimizer visibility inside petgraph.

A parameterized table function is reserved for algorithms whose inputs are fully named by
planning-time arguments and an epoch-pinned provider. Its input access is intentionally
opaque and is evaluated as such. Each compiled graph use emits a derived
`system.extension_selection` row naming the chosen rung, input shape, rejected higher
rungs, implementation release, and compile probe. It is not an authored extension
registry.

Derived facts state algorithm/version, input epoch, projection, precision, parameters,
and completeness. Materialization is chosen by measured reuse and cost; it never changes
semantic identity. Clean recomputation is the correctness oracle for every incremental
graph result.

DataFusion recursive CTEs may implement bounded compositional reachability when they
remain clearer and cheaper than graph materialization. They are never accepted as an
unbounded safety mechanism: the compiler supplies explicit depth/cycle/resource bounds,
and petgraph remains the preferred kernel for irreducible algorithms.

### 3.8 D-26 — Single command, publication, and activation path

All durable change enters as a typed command:

```text
command + predecessor epoch + explicit inputs + policy
  -> authorize and deduplicate operation ID
  -> pure reduction to intended relation changes
  -> provider/model/derivation execution
  -> validate Arrow batches and schemas
  -> write exact Delta versions and immutable Arrow segments
  -> construct candidate FabricEpoch
  -> run proof against candidate
  -> atomically append epoch + activation event and read it back
  -> swap Arc<FabricEpoch>
  -> acknowledge
```

Owner replacement, deletion, source-wave update, model migration, compaction, rollback,
and maintenance are command variants, not separate write routes. Each has explicit
predecessor, operation ID, input identities, affected owners/relations, output versions,
and terminal reconciliation. Repeating an acknowledged command returns the prior result;
repeating an unknown outcome inspects durable Delta state before deciding whether to
continue.

Delta does not provide a cross-table transaction, so atomic visibility is obtained by
construction: all component versions are committed and validated first; the final
control-table commit writes the immutable `fabric_epoch` and `activation_event` rows that
pin them. Queries can see only epochs named by a valid activation-chain head. Orphaned
component versions are unreachable candidates and are reclaimed after retention proof.

Activation is deliberately a single-writer protocol, matching the mandatory one-daemon-
per-workspace topology. An OS-released workspace lease and monotonically increasing
writer generation fence the daemon; one actor serializes every `FabricCommand`; SQLite
`BEGIN IMMEDIATE` transactions own only the temporal command/lease/cache state. A second
daemon or stale writer generation fails before any domain write. Concurrent multi-host
writers for one workspace are outside the supported deployment contract rather than an
unproved Delta row-CAS feature.

The final Delta commit contains an `OperationSelectionRecord` and a
`TransactionContract`. The selection records command ID, writer generation, predecessor,
selected epoch, compiler release, component pins, proof receipt, and terminal operation
state. The contract records the atomicity scope, idempotency/reconciliation key, required
single-writer fence, backend identity, recovery query, and acknowledgment rule. Delta's
normal atomic log creation still protects a table commit, but the architecture does not
claim it supplies distributed writer election or row compare-and-swap.

Query admission closes before the activation record is committed and read back. Existing
leases continue on the old epoch; no new query is admitted until the new `Arc<FabricEpoch>`
has swapped and the temporal cache agrees. A failure after durable selection but before
swap terminates the daemon; restart rederives and publishes the selected epoch before
opening admission. A failure before durable selection leaves the predecessor current.

`current_fabric_epoch` is a query over the valid activation-event chain, not a mutable
row. The daemon may cache it in SQLite and `ArcSwap`; startup and recovery rederive it
from Delta and compare. A fork, missing predecessor, invalid proof receipt, or multiple
heads is a hard failure, never “pick the newest timestamp.”

### 3.9 D-27 — Physical state and materialization

Delta owns durable model, source, raw, canonical, derived, proof, and activation
relations. Every epoch opens exact Delta versions and reuses their providers for its
lifetime. A query never refreshes a table handle to latest.

Delta providers are built from exact snapshots/versions with the query session and
registered object stores; canonical tables are never opened as raw Parquet listings.
Writes from logical plans receive the epoch's `SessionState` and require it rather than
silently constructing a fallback session. This preserves the same functions, runtime,
object stores, and policy context across read, derivation, validation, and write.

For interactive freshness, an epoch may also pin immutable Arrow overlay segments. A
segment is staged durably through `object_store`, carries its Arrow schema and source/run
provenance, and is accepted only after byte/schema validation. `_storage` registers base
and segment providers separately; model-compiled native union/anti-join/window plans
produce the canonical owner-replacement view, so overlay semantics remain visible to the
optimizer. An in-memory batch may cache a durable segment, but unpersisted memory is not
serving authority.

Consolidation writes new Delta versions from the same canonical view, builds a candidate
epoch without the segments, and proves logical equality before activation. No query sees
an intermediate rebase. A materialization without a clean re-execution oracle is
forbidden.

SQLite remains because temporal control is a different problem. It owns local queue
state, in-flight command stages, retry schedules, cancellation acknowledgements, daemon
leases, short-lived query/result leases, and bounded operational logs. All of it is
reconstructible or safely expirable. Read-only `system` providers expose relevant
operational rows as Arrow without making SQLite a semantic source. SQLite contains no
ontology/schema/derivation authority, epoch manifest, or current-pointer override.

### 3.10 D-28 — Proof-native governance and provenance

Every model contract clause has an `oracle` relation row naming its typed input coverage,
violation relation, and expected terminal semantics. The proof compiler executes the
oracle against the candidate epoch. A proof result contains:

- model and fabric epoch IDs;
- exact `FabricCompilerRelease` and intrinsic implementation identities;
- exact source, provider, table, function, and policy inputs;
- query/plan identity and execution metrics;
- input coverage and unavailable inputs;
- violation rows or independent comparison mismatches;
- oracle owner/provenance, independence class, and targeted fault/mutant identity;
- provenance-closure status;
- causal-mutant outcomes; and
- terminal `pass`, `fail`, or `unknown`.

Provenance is emitted by the same plan or bounded algorithm that emits a fact. Canonical
facts include direct observation/run/derivation keys; multi-input derivations emit
lineage edges. A provenance resolver walks those edges to source images, accepted model
decisions, provider runs, exact table versions, and independent expectations. “Closed”
is the resolver result for the current row set, never a maintained flag.

Governance of the repository uses the same architecture. Source/import/module/tooling
facts are loaded into Arrow relations, and dependency, forbidden-edge, generated-output,
public-contract, and legacy-zero-state rules are DataFusion violation queries. Ripgrep
and ast-grep may harvest candidates or supply an independent negative-search oracle; a
text scan is not the authority for a relational invariant.

Every active model decision must have a consumer edge emitted by compilation. Structural
integrity rows such as type/key/reference declarations may prove causality by causing
construction to reject an illegal mutation. Semantic composition, authority, policy,
normalization, derivation, query, and oracle rows must instead make an independently
owned expectation, targeted fault, compiled plan, or decoded public result discriminate
the mutation. A presence check or self-authored rejection is insufficient. A decision
whose semantic mutation is independently inert is deleted or reclassified as advisory.

### 3.11 D-29 — Semantic query and FastMCP boundary

The eight semantic request forms remain one compositional request language. Rust parses
the released envelope into typed request-block, dependency-edge, binding, selection,
limit, and return relations. Resolver plans join those relations to `model.query_form`,
`model.phrase`, `model.phrase_binding`, `model.projection`, and live capability rows.
Ambiguity and unsupported semantics become explicit data or typed errors. The resulting
bound plan is compiled to DataFusion; independent branches may execute concurrently
within the same epoch and fan-in remains deterministic.

The public surface does not accept SQL, table names, function names, or physical plan
fragments. It exposes semantic values and controlled composition. All row/column/source
ACLs are resolved before plan compilation. The query executes only in the epoch's
`AccessScopeId` child session, whose catalog contains authorized projections rather than
request-filtering a global provider. Public information schema is disabled unless it is
the information schema of that reduced catalog; explicit `public.metadata_*` views are
the default reference surface.

FastMCP retains one STDIO process per agent, strict public Pydantic boundary models,
lifespan-managed daemon client, middleware, cancellation/deadline propagation, progress,
inline/resource delivery, and one logical response. It loads only released public wire
contracts. It neither imports model registries nor duplicates the semantic type graph.
The daemon remains the only owner of workspaces, snapshots, provider state, DataFusion,
Arrow processing, result authority, and semantic errors.

### 3.12 State, resource, failure, and security ownership

| Concern | Owner and lifetime | Failure/recovery rule |
|---|---|---|
| model/fact/proof versions | Delta, immutable/versioned | never mutate; rebuild candidate and append activation |
| current serving handle | workspace daemon `ArcSwap`, process lifetime | rederive unique Delta activation head at startup |
| source event coalescing | watcher/coordinator, wave lifetime | overflow marks coverage unknown and forces authoritative rescan |
| provider process state | provider adapter, job lifetime | crash yields typed gap; borrowed/vendor state is discarded |
| query state | epoch lease plus DataFusion task, request lifetime | cancellation reaches actual stream/tasks; no detached computation |
| memory and spill | epoch `RuntimeEnv`, query reservations | bounded pool, private quota-limited spill, terminal resource error |
| queue/retry/lease state | SQLite/Tokio actor, temporal | reconcile from Delta/source, expire safely, never change domain truth |
| result artifacts | immutable Arrow/response objects plus leases | epoch-pinned, range-checked, retained until all leases expire |
| credentials and ACL | daemon security boundary, session/request | catalog and information-schema visibility fail closed |

The daemon retains the singleton-workspace lease, descriptor-relative source reads,
provider sandboxing, bounded ingress, backpressure, same-user UDS authorization, and
redacted status surfaces. Source bytes remain authoritative over watcher and git hints.
gix and notify remain accelerators/adapters; loss or ambiguity triggers a source rescan,
not a false negative.

### 3.13 Performance posture

The hot path is `request relations -> model-resolved native logical plan -> optimized
physical plan -> Arrow stream`. A session and exact-version Delta providers are built
once per epoch, not per query. Projection/filter/limit pushdown, Parquet pruning,
partitioning, ordering, constraints, functional dependencies, and statistics are
reported only when proved for that exact source. Every wrapper implements and forwards
DataFusion 55 `scan_with_args`, including `StatisticsRequest`; using the compatibility
default that drops structured statistics requests is forbidden.

The runtime uses DataFusion's bounded memory pool, spill manager, batch size, target
partitions, and task metrics. Custom graph execution reserves memory and accounts for
its Arrow and petgraph allocations. Planning and execution emit stage metrics into
`system.query_run`/`system.query_stage`. Materialization and clustering decisions are
derived from measured workload relations, semantic dependency reuse, and maintenance
cost; they are not hard-coded folklore.

### 3.14 Library decisions

### LD-17 — Arrow 59.2.0 as the sole semantic data boundary

**Decision:** adopt
**Version basis:** Arrow/Parquet 59.2.0 in the root, rustc extractor, and Pyrefly sidecar; exact root graph from `Cargo.toml`/`Cargo.lock`
**Displaces:** JSON/binary provider payload fields, generated row encoders, parallel DTO graphs, and row-per-message payloads
**Risk:** extension metadata can be lost through expressions or storage mappings; stable boundaries reattach from model rows and validate schema/metadata explicitly
**Validation:** `just relational-arrow-boundary-check` must prove IPC, Parquet/Delta, expression, nested-type, nullability, and extension-metadata round trips

### LD-18 — DataFusion 55.0.0 catalog hierarchy as runtime model

**Decision:** adopt
**Version basis:** DataFusion 55.0.0 with Arrow 59.2.0; catalog, schema, table, and `information_schema` APIs in the exact dated reference
**Displaces:** generated table registries, schema catalogs, package manifests, function censuses, and manual discovery lists
**Risk:** metadata visibility can leak unauthorized names; agent queries therefore use a reduced child catalog rather than filtering the full catalog in place
**Validation:** `just relational-catalog-closure-check` must join live information schema to the model and prove authorized closure and redaction

### LD-19 — Programmatic DataFusion logical planning

**Decision:** adopt
**Version basis:** DataFusion 55.0.0 `DataFrame`, `Expr`, `DFSchema`, and `LogicalPlanBuilder` surfaces
**Displaces:** SQL-string builders, operation-specific Rust plan assembly, generated plan catalogs, and durable serialized plans
**Risk:** a generic compiler could become an opaque interpreter; its algebra is closed, typed, exhaustive, emits decision dependencies, and is mutation-tested
**Validation:** `just model-plan-causality-check` and `just semantic-plan-conformance-check`

### LD-20 — Honest DataFusion providers and metadata

**Decision:** wrap
**Version basis:** DataFusion 55.0.0 `TableProvider`, `ScanArgs`, statistics requests, constraints, functional dependencies, partitioning, ordering, and pushdown contracts
**Displaces:** `Id16ContractProvider`-style wrappers that inherit lossy compatibility defaults and hand-maintained statistics posture constants
**Risk:** incorrect Exact pushdown or stale metadata changes results; unsupported/inexact is the default until an epoch proof establishes stronger claims
**Validation:** `just table-provider-contract-check` with residual-filter, projection, limit, statistics-request, constraint, and `EXPLAIN` oracles

### LD-21 — DataFusion functions and logical extensions at the highest semantic level

**Decision:** adopt
**Version basis:** DataFusion 55.0.0 scalar, aggregate, window, table-function, logical-extension, planner-extension, and analyzer/optimizer surfaces
**Displaces:** operation-kind dispatch, hand-built graph plan routing, and custom physical operators for relational work
**Risk:** any extension can hide semantics or bypass policy; the compiler selects the narrowest valid surface by input shape, and the same installer that registers intrinsic implementations emits the runtime rows used for closure
**Validation:** `just function-runtime-closure-check`, `just graph-extension-conformance-check`, and plan-node policy mutation tests

### LD-22 — Delta at revision 43a0cf10 as durable relation and activation-event log

**Decision:** adopt
**Version basis:** `deltalake` 1.0.0 source at revision `43a0cf10a313e5077c48637ad786a05359136bbb`, DataFusion 55.0.0, object_store 0.13.2
**Displaces:** mutable serving manifests, semantic SQLite epoch data, latest-table lookup during queries, and ad hoc multi-table visibility
**Risk:** Delta has no cross-table transaction or writer election; a fenced single daemon stages exact versions and appends an operation selection only after proof
**Validation:** `just fabric-transaction-contract-check` and `just fabric-activation-recovery-check` must cover every commit/acknowledgment fault and reject a second/stale writer before domain writes

### LD-23 — SQLite 0.40.2 plus OS lease for single-writer temporal coordination

**Decision:** retain-current
**Version basis:** rusqlite 0.40.2 with bundled SQLite and backup
**Displaces:** the current operational store's ontology candidate, epoch package, and semantic current-pointer authority
**Risk:** temporal fencing is local and intentionally does not support concurrent multi-host writers for one workspace; startup re-derivation and structural rules forbid semantic model/epoch tables and pointer override
**Validation:** `just single-writer-fence-check`, `just temporal-store-boundary-check`, and delete/rebuild equivalence from Delta and source inputs

### LD-24 — petgraph 0.8.3 behind bounded plan extensions/providers

**Decision:** wrap
**Version basis:** petgraph 0.8.3 with `std`, using typed graph algorithms and visit traits
**Displaces:** custom graph algorithm implementations and canonical graph DTOs that expose implementation indices
**Risk:** the petgraph boundary is opaque to DataFusion; relational selection happens in an extension child before materialization, while parameterized providers admit their opacity explicitly
**Validation:** `just graph-extension-conformance-check`, extension compile probes, differential reference fixtures, and forced resource/cancellation cases

### LD-25 — Exact current code-fact APIs without defensive semantic abstraction

**Decision:** adopt
**Version basis:** Ruff crates 0.0.7; Pyrefly 1.2.0 at `1933169a...`; Tree-sitter 0.26.12 with Python 0.25.0 and Rust 0.24.2 grammars; `rustc_public` on nightly-2026-08-18
**Displaces:** lossy lowest-common-denominator DTOs, provider kind JSON censuses, cold JSON/Binary payloads, and hypothetical compatibility shims
**Risk:** upstream upgrades require deliberate migrations; exact adapter compile probes and schema/fixture diffs make that work visible
**Validation:** `just exact-provider-api-check` plus provider-native Arrow fixture conformance for every modeled family

### LD-26 — gRPC control plus Arrow IPC data and FastMCP presentation

**Decision:** retain-current
**Version basis:** tonic/tonic-prost 0.14.6, prost 0.14.4, Arrow IPC 59.2.0, and the adapter's pinned FastMCP/Pydantic environment
**Displaces:** Protobuf fact DTO expansion, Python data-plane logic, generated model registries in the adapter, and any native Python extension
**Risk:** schema/wire skew and oversized messages; handshake negotiates released control features while Arrow schema validation gates every stream
**Validation:** `just provider-protocol-check`, `just adapter-test`, and cross-language Arrow stream corruption/backpressure/cancellation tests

## 4. Alternatives and clean-sheet challenge

### 4.1 Alternative A — Repair the v5 generated ontology program

This alternative would finish the active v3 plan: keep YAML/JSON authorities, complete
the generated Arrow bundle, strengthen the generic ontology compiler, close SQLite/Delta
activation gaps, and add more drift/causality gates.

It has the lowest near-term rewrite cost and preserves substantial current code. It is
rejected because its principal failure is architectural rather than incomplete. The
model reaches execution only after passing through parallel registries, generators,
bindings, fingerprints, package manifests, Arrow artifacts, and runtime loaders. A
forgotten or inert declaration remains possible at every projection. Adding stronger
checks increases the number of synchronization points instead of removing them. The
current independent-review findings about non-causal rule records, split activation
authority, incomplete reconstruction, and bypassing oracles are symptoms of that shape.

### 4.2 Alternative B — Replayed relational authority and immutable epoch catalogs

This is the selected design. Accepted decisions replay into typed model relations;
runtime construction, execution, validation, introspection, and governance all consume
those relations. Native DataFusion views and plans keep work visible, exact Delta
versions provide durable facts, the activation chain provides atomic visibility, and
one epoch object pins all query state.

It has the highest deliberate migration cost, but it removes the model compiler as an
artifact generator, eliminates parallel authorities, exposes current provider semantics,
reduces custom execution code, and makes future provider/library upgrades explicit
schema/model migrations. Its main risks—bootstrap circularity, catalog immutability as
application discipline, local single-writer fencing, and model-interpreter opacity—have
bounded design responses and named proofs.

### 4.3 Alternative C — Rust types and code as the only model

This alternative would replace YAML/generated artifacts with handwritten Rust enums,
structs, builders, and direct DataFusion plan functions. It would gain compile-time
exhaustiveness and delete much tooling.

It is rejected as the semantic authority because the active model would again be hidden
inside code branches. DataFusion could execute the result but could not query the model,
validate its closure, expose why a plan exists, derive capabilities, or prove that a
human decision is causally consumed. Rust remains the right place for the minimal
metamodel, closed compiler algebra, and exact vendor bindings—not for the mutable current
ontology and governance census.

### 4.4 Alternative D — One universal EAV/property-graph relation

This alternative would maximize apparent relational uniformity by storing all nodes,
edges, properties, model declarations, observations, and proof rows in a few generic
tables.

It is rejected because “maximally relational” does not mean “minimally typed.” EAV would
move field types, cardinality, requiredness, nested structure, keys, and legal joins out
of Arrow schemas and into runtime predicates. It would weaken Parquet pruning,
DataFusion planning, schema evolution, and comprehensibility while increasing invalid
states. A relational catalog of many strongly typed relations is the correct common
representation.

### 4.5 Alternative E — Custom providers/operators as the semantic facade

This alternative would keep most current Rust logic but expose each semantic domain as a
custom `TableProvider` or `ExecutionPlan`.

It is rejected as the default because it hides filters, joins, overlay consolidation,
authority resolution, and calculation structure from the optimizer and validator. It
also requires CodeFabric to reimplement projection, filter, statistics, constraints,
partitioning, cancellation, and metrics contracts per domain. Custom providers remain
valid only at irreducible source/security boundaries. A custom physical node remains
valid only behind a typed logical extension when an operation cannot be expressed as a
native relational plan or a planning-time scalar-argument table function.

### 4.6 Alternative F — SQLite or a mutable Delta row as model/current authority

SQLite provides excellent local transactions, and a singleton Delta row could be
overwritten on each activation. Both are rejected as the semantic model/current-state
authority. SQLite would create a second non-Arrow semantic plane and cannot by itself
govern object-store-visible epochs. A mutable “current” row freezes a moving concept and
hides accepted activation history.

The selected split stores immutable activation events in Delta, derives the unique head,
caches it in process/SQLite, and keeps all SQLite data reconstructible. This uses each
engine for the state kind it actually supports.

### 4.7 Clean-sheet answer

If the current repository did not exist, the preferred design would still be Alternative
B: a Rust service that treats its product model as typed relational data, creates an
immutable DataFusion catalog per present-state epoch, projects exact provider APIs to
Arrow at their boundary, persists exact versions in Delta, and exposes a semantic query
language through a thin MCP adapter.

The existing process isolation, public topology, canonical identity rules, and
present-state semantics survive because their reasons survive. The model generation
pipeline, static registries, low-fidelity DTOs, custom overlay machinery, and most
artifact governance do not survive because their reasons disappear under v2.

## 5. Transition, cutover, and legacy disposition

### 5.1 Transition law

The transition may run old and new candidates side by side for comparison, but production
truth may never be dual-written or resolved from both. The new fabric is built under a
separate internal namespace and reads frozen copies of source/provider inputs. Only one
runtime owns mutation and serving authority at a time.

Compatibility is retained only where an external/released boundary requires it:

- released public request/response/status/source JSON schemas;
- released Protobuf service and control messages;
- stable public IDs, error codes, result identities, and accepted artifact tombstones;
- result-resource lifecycle visible to MCP clients; and
- persisted Delta data required for rollback or accepted retention.

Internal YAML paths, generated Rust modules, bundle layouts, package fingerprints,
current DTO fields, internal SQL schemas, module names, and active-plan packet IDs are not
compatibility contracts.

### 5.2 Foundation and cutover sequence

#### Stage 0 — Authority reset

1. Accept this dossier and version the authoritative suite so the v2 principles and this
   realization no longer conflict with static-artifact mandates.
2. Stop execution of the superseded v3 implementation plan at a coherent boundary and
   activate a new plan rather than appending remedial packets.
3. Record external wire, public ID, accepted artifact, and rollback commitments. Every
   other current surface is presumed replaceable.
4. Freeze legacy registry/model changes except corrections required to preserve current
   operation during the migration window.

#### Stage 1 — Bootstrap and model migration

1. Implement the minimal metamodel and closed logical algebra without importing current
   generators.
2. Write a one-time importer that reads current registries/schema IR/released allocations
   as migration evidence and produces reviewed typed `ModelMigration` events.
3. Require row-level bijection or an explicit disposition: migrated, combined, split,
   superseded, tombstoned, or rejected as false static.
4. Replay into Arrow, self-describe the bootstrap, run relational invariants, and build a
   model-only DataFusion epoch.
5. Independently review the semantic rows; do not accept a mechanically imported current
   model merely because it is byte-complete.

The importer is `encapsulate-temporarily`: it is never linked into the daemon, its input
tree is frozen, and it is deleted immediately after the accepted migration and rollback
window. It cannot become the ongoing authoring path.

#### Stage 2 — Exact provider relations

1. Independently author the exact `ProviderBoundaryContract` for every pinned API family,
   including intentional omissions/remainders, then define provider-native Arrow schemas
   and compiled handler rows from the current library APIs.
2. Replace Pyrefly's module-level JSON payload with typed context/module/definition/type/
   member/import/call/diagnostic/remainder relations.
3. Replace rustc's MIR summary payload with typed item/body/block/local/place/operand/
   rvalue/statement/terminator/CFG/call/instance/span/ownership/dataflow relations exposed
   by the pinned `rustc_public` surface.
4. Replace Ruff's opaque semantic fields and bespoke normalized DTO chain with direct
   provider-native relations; retain raw AST/token/trivia/semantic distinctions.
5. Emit Tree-sitter grammar-native nodes/fields/errors and query captures without losing
   raw kinds.
6. Version the Arrow IPC provider protocol and prove flow control, cancellation, schema,
   source/context, and completeness semantics end to end.

No provider family is advertised merely because its relation exists. Exact fixtures and
runtime proof decide supported/partial/unknown.

#### Stage 3 — Catalog, compilers, and proof

1. Assemble immutable catalog/session candidates from the model epoch and exact provider
   batches.
2. Compile normalization, authority, unknown, and derivation plans; populate canonical
   and derived views.
3. Compile semantic request relations into native DataFusion plans and verify all eight
   request forms and composition roles.
4. Compile every invariant and capability prover; load independent expectation relations
   into the proof schema.
5. Expose filtered dynamic status/reference/capability content from the catalog through
   the daemon and FastMCP adapter.

#### Stage 4 — Durable epoch publication

1. Write the new relation layout to separate Delta tables or versions. Do not reinterpret
   legacy tables in place.
2. Implement the single command reducer, durable overlay segment staging, exact-version
   epoch construction, proof, fenced single-writer activation selection, readback, query
   admission barrier, and `ArcSwap` publication.
3. Rebuild an epoch from migrations, Delta, Arrow segments, provider/source pins, and
   the exact `FabricCompilerRelease` plus independent expectations without any generated
   model artifact.
4. Exercise crash points before/after every durable write, activation commit, readback,
   swap, acknowledgment, compaction, and lease transition.

#### Stage 5 — Isolated comparison and atomic serving cutover

1. Run old and new engines read-only against identical frozen inputs.
2. Compare logical facts and public query results using independently authored expected
   semantics, not old-engine output as the oracle. Document intentional deltas.
3. Run clean rebuild, incremental update, provider degradation, concurrency, resource,
   security, and FastMCP delivery verticals.
4. Execute the durable cutover state machine:

   ```text
   LEGACY_AUTHORITATIVE
     -> LEGACY_QUIESCED
     -> NEW_BINARY_FENCED_READ_ONLY
     -> NEW_EPOCH_SELECTED
     -> NEW_SERVING_NO_MUTATION
     -> NEW_MUTATING
     -> LEGACY_RETIRED
   ```

   The legacy daemon drains and releases its writer lease before the new binary starts.
   The new binary acquires a higher writer generation, proves and selects the epoch,
   swaps it, and serves read-only before mutation is enabled. Every transition is durable,
   idempotent, predecessor-checked, and crash-reconcilable.
5. Do not retain a runtime fallback to the old model. Old-binary rollback is allowed only
   from `LEGACY_QUIESCED`, `NEW_BINARY_FENCED_READ_ONLY`, or
   `NEW_SERVING_NO_MUTATION`, before any new-architecture mutation. Enabling
   `NEW_MUTATING` irreversibly fences the legacy writer; subsequent recovery is forward
   through the new command path.

#### Stage 6 — Total decommission

1. Remove every legacy consumer before deleting its authority.
2. Record required released-ID tombstones/retirements, then remove generated copies,
   compiler/tooling paths, registry inputs, features, recipes, CI jobs, rules, and tests.
3. Reconcile Cargo dependencies and feature graphs only after reachability, build scripts,
   generated code, tests, and platform `cfg` use are proved absent.
4. Run the coverage-qualified legacy zero-state suite and all four build-domain gates.
5. Declare completion only when old authority cannot be selected, imported, generated,
   installed, activated, queried, or packaged.

### 5.3 Rollback and recovery

Before the first new-architecture mutation, rollback stops the new daemon, proves its
writer generation released, writes the cutover rollback transition, reactivates the last
compatible old deployment and its exact old versions, and leaves new tables isolated.
The old binary never interprets or owns the new activation log. The external cutover
controller owns only this fenced deployment transition and cannot write fabric facts.

After `NEW_MUTATING`, old-binary rollback is forbidden because it would restore dual
authority and lose new-format changes. Recovery activates an older compatible
`FabricEpoch` or builds a corrective forward epoch through the new command path.

If a model/wire migration is not
backward compatible, recovery rebuilds a new epoch from the prior accepted migration head
and source inputs using the pinned old compiler release, then explicitly migrates and
revalidates under the current release. Vacuum cannot remove any Delta version, compiler
release input, or Arrow segment referenced by an active query/result lease, rollback
window, accepted expectation, or activation event within retention.

Unknown commit outcomes always reconcile by operation ID, Delta transaction marker,
control-table history, and readback. The system never retries by guessing.

### 5.4 Generated inventory and coverage envelope

The disposition matrix below was generated from these read-only inventory commands at
the recorded baseline:

```sh
ast-grep outline src --items exports --view names
ast-grep outline rustc-extractor/src --items exports --view names
ast-grep outline pyrefly-sidecar/src --items exports --view names
ast-grep outline codefabric-cpg-mcp/src --items exports --view names
ast-grep outline tooling/ci --items exports --view names
ast-grep outline tests --items exports --view names

rg --files src rustc-extractor/src pyrefly-sidecar/src \
  codefabric-cpg-mcp/src tooling/ci scripts tests | sort
rg --files contracts docs/authoritative_design docs/spec_index docs/plans \
  docs/designs docs/reviews .github rules rule-tests tooling/proto | sort

ast-grep run --lang rust --pattern 'include!($A)' \
  src rustc-extractor/src pyrefly-sidecar/src
ast-grep run --lang rust --pattern 'include_bytes!($A)' \
  src rustc-extractor/src pyrefly-sidecar/src tooling
ast-grep run --lang rust --pattern 'include_str!($A)' \
  src rustc-extractor/src pyrefly-sidecar/src tooling
```

The code envelope contained 202 handwritten Rust/Python/shell files; the
contract/design/config envelope contained 274 files; the generated/materialized envelope
contained 58 files totaling 6,050,706 bytes across `src/generated`,
`contracts/generated`, `contracts/manifests`, and adapter contract outputs. The
`include!` search covered 104 Rust files with none skipped; the `include_bytes!` and
`include_str!` search covered 109 Rust/tooling files with none skipped. Those counts are
observations from the inventory run, not target censuses to maintain.

#### 5.4.1 Disposition coverage is derived

The L-20–L-55 decisions are immutable human design judgments. Their membership is not a
hand-maintained path list. The implementation plan must compile each row's path and, where
needed, symbol selector into `legacy_disposition_selector` input rows and join them to a
fresh `legacy_inventory` relation containing path, language, exported symbol, artifact
role, package membership, generator/consumer edges, and archive classification.

Three relational queries are mandatory:

```text
uncovered_surface = inventory LEFT ANTI JOIN selector_match
overlapping_surface = selector_match GROUP BY surface_id HAVING count(*) != 1
unresolved_mixed_file = files with multiple dispositions but no symbol-level selector
```

`just legacy-disposition-coverage-check` fails on any row from those relations, any
unparsed/skipped candidate, or any selector that matches nothing. Broad file selectors
cover every exported symbol only when the whole file has one disposition. A mixed file
must be split in the target or use non-overlapping symbol selectors. The selector input is
generated from this accepted design/its plan; inventory and membership are always
recomputed from the tree.

### 5.5 Legacy disposition matrix

No material surface is deferred. `Encapsulate-temporarily` is used only for bounded
migration inputs with an explicit exit.

| ID | Current surface | Disposition | Target/justification and exit |
|---|---|---|---|
| L-20 | `src/bin/codefabric_model/**` and the `model-compiler` feature/bin | **delete** | Replace artifact generation with model replay and generic runtime compilers. Extract only foreign Protobuf build generation and the one-time importer; remove the feature, binary, dependencies, and tests after cutover. |
| L-21 | `tooling/model/**`, `scripts/model_*`, DesiredTree sync/plan/repro/family/release tooling | **delete** | Relational replay, catalog closure, causal proof, and package-time foreign generation replace it. The migration importer exits at Stage 1 acceptance. |
| L-22 | `contracts/registry/*.yaml`, `contracts/schema/schema-contract-ir.json`, schema fragments, current policy/comparison/fault/config registries | **encapsulate-temporarily** | Read once as migration evidence with row-level disposition. Never link into the daemon. Exit invariant: accepted migration bijection and rollback retention complete, then delete all inputs/importer consumption. |
| L-23 | `contracts/generated/model/**`, `contracts/generated/provider-raw-kinds/**`, current suite/requirements/traceability/fixture/package manifests | **delete** | Replaced by replayed model relations, information schema, current package queries, and proof relations. Preserve only separately accepted historical decisions in archival locations. |
| L-24 | generated Rust model surfaces: `fact_row_encoders`, `id_domains`, `model*`, `ontology_program_bundle`, `provider_raw_kinds`, `registries`, `result_schemas`, `table_specs` | **delete** | Runtime builds schemas/plans/encoders from the epoch model; canonical low-level identity/wire primitives move to the minimal bootstrap. Zero `include!`/imports remain. |
| L-25 | `src/generated/codefabric.*.rs`, generated Python Protobuf stubs, descriptor set | **reshape** | `.proto` remains released authority. Generate stubs/descriptors at build or package time. Commit a cache only if a foreign build environment demonstrably cannot derive it, with regenerate/compare proof. |
| L-26 | adapter fingerprints, package-data manifest, generated registry/schema aggregates, model artifact index, generated query-form tables | **delete** | Adapter loads released wire contracts and derives live reference/capability/status through the daemon catalog. Preserve only narrow canonical JSON, identity, and boundary model helpers that remain causally used. |
| L-27 | `src/ontology_program.rs`, `ontology_plane.rs`, `ontology_contract.rs`, `ontology_candidate.rs`, `ontology_gate.rs` and installed/resealed bundle path | **replace** | Model migrations, candidate `FabricEpoch`, proof relations, and activation event replace package/bundle authority. Existing accepted epoch artifacts are migration inputs/history, not future authoring surfaces. |
| L-28 | `src/ontology_relational_program.rs`, `ontology_rules.rs`, `ontology_executor.rs`, `domain_conformance.rs` | **reshape** | Retain the relational nucleus and learned semantics, but replace hard-coded/generated program catalogs and DataFusion variant censuses with the generic compilers and exhaustive exact-version implementation. |
| L-29 | `src/schema_registry.rs`, `src/registries.rs`, `src/provider_raw_kinds.rs`, `src/contracts/{index,registry_models}.rs` | **replace** | Replayed model tables plus live catalog/runtime queries are authoritative. Retain only truly static wire/bootstrap types in inward modules. |
| L-30 | `src/fact_ingest.rs`, `core_facts.rs`, `source_syntax.rs`, `python_semantic.rs` | **replace** | Direct typed raw relations and model-compiled normalization/authority/unknown plans replace procedural projection and cold-payload handling. |
| L-31 | `src/ruff_adapter/**`, `src/tree_sitter_adapter.rs` | **reshape** | Compile directly against exact current APIs and emit provider-native Arrow. Delete defensive mirrors, static kind catalogs, and opaque semantic JSON fields; retain short-lived vendor bindings and source-coordinate capture. |
| L-32 | `pyrefly-sidecar/**`, `src/pyrefly_service.rs` | **reshape** | Preserve process/revision isolation, source/context validation, backpressure, cancellation, and Arrow IPC. Replace one-row opaque payload with full typed current-API relations and derived capability proof. |
| L-33 | `rustc-extractor/**`, `src/rustc_service.rs` | **reshape** | Preserve dated-nightly process isolation and control protocol. Replace `OwnedMirItem` summaries with typed current `rustc_public` relations and stable semantic identity inputs. |
| L-34 | `src/provider_runtime*`, `provider_types.rs`, `provider_sandbox.rs` | **reshape** | Preserve job/sandbox/transport lifecycle; derive registrations/capabilities from installed exact adapters and proof, not hard-coded inventories. |
| L-35 | `src/fabric.rs`, `fabric/{mutation,publication,snapshot_catalog,serving}.rs` | **reshape** | Preserve exact Delta-version providers, one mutation spine, bounded sessions, and Arrow streams. Rebase around `FabricEpoch`, relational model, activation event, and honest DataFusion 55 provider contracts. |
| L-36 | `src/fabric/overlay.rs` and bespoke concatenate/take/row-conversion consolidation | **replace** | Native optimizer-visible anti-join/union/window views over base and immutable segment providers. Keep only a small epoch-pinning provider if an irreducible security/storage seam remains. |
| L-37 | `src/snapshot.rs`, `snapshot_runtime.rs`, serving-snapshot JSON schema/manifests | **replace** | Relational `fabric_epoch`, exact pins, proof receipt, activation chain, and query lease replace mutable/duplicated manifest authority. Public snapshot metadata remains a released response projection. |
| L-38 | `src/operational_store.rs` ontology candidate/package/current-pointer tables | **replace** | SQLite retains temporal queue/retry/lease/command progress only. Delta activation relations are semantic history; current is rederived. Operational schema is owned by one handwritten SQLite migration path, not generated current SQL. |
| L-39 | `src/semantic_query.rs`, `query_service.rs`, `governed_session.rs` | **replace** | Preserve public forms and bounded behavior; represent requests as relations and compile via model. Delete fixed form crosswalks, capability booleans, package-bound planning, and bypass execution paths. |
| L-40 | daemon/coordinator/lifecycle/workspace/source image/inventory/gix/notify/security/cancellation modules | **reshape** | Preserve central ownership, safe source truth, invalidation, cancellation, fairness, and security. Emit source/lifecycle/operational facts to Arrow/system providers and route every durable change through `FabricCommand`. |
| L-41 | petgraph projection DTOs/registries and persisted `NodeIndex`-like identities | **delete** | Retain petgraph only as a transient bounded kernel behind a typed logical extension for relational inputs, or an explicitly opaque planning-time provider for wholly scalar-named inputs, with canonical external IDs and Arrow output. |
| L-42 | `codefabric-cpg-mcp` server/client/settings and public boundary helpers | **reshape** | Preserve FastMCP topology and strict public models. Delete packaged model registries/fingerprints and derive live references from the daemon; keep no Arrow/DataFusion/domain state in Python. |
| L-43 | `contracts/rpc/*.proto`, public request/response/status/source schemas, externally stable grammar/admin protocol | **preserve** | These are Class 1 released boundaries. Revalidate and version through compatibility policy; do not infer internal static authority from them. |
| L-44 | released allocations, canonicalization KATs, independent goldens, accepted release decisions | **preserve** | Preserve immutable evidence and public allocations. Load applicable decisions as independent model/expectation inputs without making production code their author. |
| L-55 | generated/current identity-recipe arrays and their runtime registries | **replace** | Minimal released domain encoders remain intrinsic; active identity composition is a model-owned binding compiled by the exact release. Delete generated parallel arrays after equivalence proof. |
| L-45 | `gate_b_*`, `golden_corpus.rs`, `functional_golden*` execution machinery | **replace** | Replace producer-generated expectations and artifact-count acceptance with independently authored typed rows, causal mutants, decoded public-surface comparison, and explicit limitations. Released candidate/decision dossiers are preserved by L-52. |
| L-46 | v1 principle registry/detector/baseline, 124 detector mappings, alignment script, transformation traceability | **delete** | v2 relational invariant/oracle model and causal governance replace them. Do not port textual/path-count detectors as a new registry. Preserve the frozen v1/v2 prose documents as history/doctrine. |
| L-47 | model/artifact/census/error/property/gate CI scripts, recipes, JSON censuses, and ast-grep rules tied to generated authority | **replace** | Replace with replay, catalog closure, causal mutation, relational invariant, exact provider, transaction/fence, and legacy-zero-state gates. Retain structural rules only for true process/build/wire boundaries or as independent residue checks. |
| L-48 | `justfile`, `.github/workflows/**`, feature matrix, stable-graph rules | **reshape** | Remove retired model jobs and dependencies; add intent-level relational fabric/proof recipes. Preserve four-domain and exact-version isolation gates. |
| L-49 | tests and fixtures | **reshape** | Preserve behavioral/provider/protocol/KAT evidence. Replace digest/count/static-text tests with typed-row, invariant, causality, replay, migration, concurrency, resource, and delivery tests. Expectations must be independently authored. |
| L-50 | `docs/authoritative_design/**` current realization clauses | **replace** | Preserve high-level behavior; publish a v2-aligned suite with relational authority and mark the prior suite historical. Do not leave both as coequal current authority. |
| L-51 | `docs/spec_index/**` | **replace** | Generate on demand from the live design/model or keep only as disposable navigation cache. It remains nonnormative and cannot be a runtime/build input. |
| L-52 | prior principles, designs, plans, state, reviews, and `contracts/acceptance/released-artifact-census-v1.json` | **preserve** | Immutable history and accepted allocation/tombstone evidence remain. The current census records 66 released IDs; each requires an explicit compatibility/tombstone transaction before physical deletion. Remove active runtime/tooling reads; state/status is rederived from events and proofs. |
| L-53 | Cargo/uv locks, toolchains, Protobuf toolchain, four build roots | **preserve** | Preserve exact inputs and justified isolation. Dependency/feature removal is covered by L-20/L-48 and occurs only after retired-code reachability and all feature graphs prove zero use. |
| L-54 | pre-existing dirty changes in `src/ontology_program.rs`, `src/schema_registry.rs` and six retained epoch artifacts they introduce | **encapsulate-temporarily** | Do not overwrite. Treat accepted artifacts as immutable migration/replay evidence, import their semantic decisions, and remove live consumption only after migration proof and rollback retention. |

### 5.6 Total-purge completion condition

Legacy removal is complete only when all of these are true at the same proving commit:

- no legacy authority file, generated copy, bundle installer, current registry, model
  compiler, feature, recipe, CI job, rule, test helper, package datum, or runtime import
  remains outside explicitly archived history;
- no production path can load, generate, compare, activate, serve, or fall back to the
  old model;
- no Python package includes old fingerprints/registries/schema aggregates;
- no Protobuf data message carries legacy opaque semantic payloads;
- no Cargo dependency survives solely for retired machinery;
- no “temporary” compatibility alias, environment flag, test-only write path, or dual
  activation route remains;
- combined `ast-grep` structural search and `rg --hidden -g '!.git/**'
  -g '!docs/library_ref/**'` textual search cover all code/config/package surfaces,
  record skipped/unparsed files, and return zero candidates;
- default, featureless, each-feature, extractor, sidecar, adapter, package, protocol, and
  authoritative-suite builds/checks pass; and
- the new fabric reconstructs and answers independent semantic expectations with the
  archived legacy tree unavailable.

Archived historical documents are not legacy functionality. They remain only when no
build, runtime, test oracle, generator, or active status pointer consumes them.

## 6. Proof strategy — oracles and checks that will prove the target

### 6.1 Proof layers

Correctness is not established by a single digest, generated-output comparison, or
end-to-end execution. The target requires these orthogonal layers:

1. construction proof for typed metamodel, commands, schemas, and query algebra;
2. relational invariant proof over exact candidate model/data;
3. independent semantic expectations over decoded public and internal Arrow rows;
4. causal intervention proving declarations and policies affect production execution;
5. clean-rebuild and incremental equivalence;
6. storage/concurrency/recovery proof at real backends and failure points;
7. provider/API conformance against the exact current libraries;
8. DataFusion plan/provider contract proof;
9. protocol and public compatibility proof;
10. performance/resource/security proof; and
11. coverage-qualified legacy zero-state proof.

### 6.2 Invariant-to-oracle matrix

| Invariant | Named executable proof | What must distinguish failure from absence |
|---|---|---|
| I-20 one model authority | `just model-replay-check`, `just model-bootstrap-closure-check`, `just model-causality-check` | missing migration/input is `unknown` or hard failure; inert row mutation fails |
| I-21 one serving epoch | `just fabric-epoch-pinning-check` | concurrent activation cannot mix table/model/function/policy generations |
| I-22 one Arrow universe | `just relational-arrow-boundary-check` | type/nullability/metadata/nested mismatch fails before consumption |
| I-23 raw fidelity | `just exact-provider-api-check`, `just provider-native-arrow-conformance-check` | every requested family is rows, explicit remainder, diagnostic, or unknown |
| I-24 model-compiled execution | `just model-plan-causality-check`, `just semantic-plan-conformance-check` | mutation changes actual production plan/result, not a test-only compiler |
| I-25 one mutation path | `just fabric-single-mutation-path-check` | test/admin/import/maintenance bypass candidates are structural failures |
| I-26 fenced writer/activation event/current query | `just single-writer-fence-check`, `just fabric-transaction-contract-check`, `just fabric-control-recovery-check` | second/stale writers fail before domain writes; forks, stale predecessors, and unknown acknowledgements fail closed |
| I-27 proof in epoch | `just fabric-epoch-proof-closure-check` | zero violations without complete covered inputs yields `unknown`, not pass |
| I-28 semantic composition | `just semantic-query-conformance-check`, `just adapter-test` | independent/fan-in/fan-out/result-reference cases share one epoch |
| I-29 visibility/honest providers | `just table-provider-contract-check`, `just plan-visibility-check` | residual filters, statistics requests, constraints, and nested plans are exercised |
| I-30 exact APIs | `just exact-provider-api-check` plus extractor/sidecar gates | compile/API/schema drift is explicit migration work, never silently accepted |
| I-31 exact compiler release | `just compiler-release-reconstruction-check`, `just cross-release-epoch-migration-check` | old/current reducers cannot silently produce an epoch with the same identity |
| I-32 catalog-construction authorization | `just access-catalog-isolation-check`, `just public-query-port-check` | forbidden catalog/schema/table/column names and raw provider/session/`DataFrame`/serialized-plan handles are unreachable, not merely row-filtered |

### 6.3 Model and governance proof

`model-replay-check` runs the migration chain twice in isolated directories/processes
with the exact pinned `FabricCompilerRelease`, canonicalizes logical row ordering, and
compares complete Arrow content and schemas. It then discards materialized model tables,
reconstructs again, and compares. A cross-release test first reconstructs with the old
release, then executes the explicit migration/revalidation into the new release. Digests
may locate differences but row/schema comparison establishes equivalence.

`model-bootstrap-closure-check` joins the live `information_schema`, bootstrap
self-description, and derived intrinsic runtime relation to prove that the minimal
hard-coded metamodel is neither missing nor silently expanding. It also proves every
active model reference, key, dependency, primitive binding, policy, invariant, oracle,
and public projection resolves without a handwritten runtime registry.

`model-causality-check` executes the real epoch builder and production compiler. For
structural type/key/reference rows, an illegal mutation may prove construction is
load-bearing by rejection. For semantic rows, automatically generated mutations provide
coverage candidates only; an independently owned expectation or targeted fault must
observe a plan/public-result/authorization/unknown change. The production model,
compiler, provider, and proof compiler may not author that expected discrimination.

Repository governance loads module/import/artifact/contract/build facts into Arrow and
runs violation queries. Enforcement-oracle rows are production model inputs and prove
mechanical policy only. Independent expectation/fault relations are separately owned,
loaded through a separate port, and identify the exact disconnected-enforcer mutant they
must catch. The check on the checks is part of the release gate.

### 6.4 Provider, Arrow, and normalization proof

For each exact provider version, fixtures exercise every modeled API family, including
valid output, partial output, syntax/compile/type failure, dynamic ambiguity, cancellation,
oversize, unsupported platform/configuration, and corrupt protocol input. The oracle
compares typed provider-native rows to expectations authored independently from the
adapter.

Schema proof crosses Rust Arrow, IPC stream/file, Delta/Parquet, DataFusion expressions,
and PyArrow where applicable. It tests IDs, decimals, timestamps/timezones, dictionaries,
nested lists/structs/maps, nullability, ordering, metadata, extension registration,
projection, filtering, joins, and round-trip failure. Extension metadata is annotation
until an enforcement consumer and fault prove otherwise.

Normalization proof joins raw observations to canonical results and explicit
unknown/conflict rows. Authority mutants swap precedence, delete evidence, introduce
conflicts, or remove coverage. Independent expectations prove the resulting semantics.
Raw rows remain available to show why a normalized result exists.

### 6.5 DataFusion and graph proof

`table-provider-contract-check` exercises every non-native provider with DataFusion 55
`ScanArgs`: reordered/empty projection, supported/unsupported/inexact filters, residual
filters, zero and nonzero limits, statistics requests, constraints, multiple partitions,
cancellation, and schema mismatch. `EXPLAIN` structure and result equivalence prove that
native views expose overlay/derivation plans without snapshotting fragile plan strings.

Constraint and functional-dependency metadata is registered only after an independent
query proves uniqueness/nullability. A deliberate duplicate must make proof fail and
must prevent the metadata from being advertised. Foreign keys and checks remain
relational invariants because DataFusion optimizer metadata does not enforce them.

Graph extensions/providers are compared with small hand-authored graphs and an independent
reference implementation, plus property tests over permutation, duplicate edges,
cycles, disconnected components, empty graphs, bounded depth, and external-ID
round-trip. Resource and cancellation faults must terminate actual graph work. No test
may compare persisted `NodeIndex` values.

### 6.6 Publication, activation, and recovery proof

`single-writer-fence-check` launches a live daemon, then attempts same-host duplicate and
stale-generation writers through every mutation/admin/import/test entry point. Each must
fail before a Delta/segment domain write. The target does not claim concurrent multi-host
mutation support. `fabric-transaction-contract-check` verifies the emitted operation
selection and transaction contract for every command variant.

Faults cover lease loss, daemon death, timeout after successful commit, timeout before
commit, stale predecessor, forked activation history, duplicate operation ID, corrupt
manifest, missing component/compiler version, readback mismatch, query-admission barrier,
and crash between selection/readback/swap/cache/acknowledgment. No retry may guess or
silently rebase.

`fabric-epoch-pinning-check` holds old query leases while new source, model, provider,
function, policy, and table versions activate. Every row, status record, progress event,
result artifact, and checksum must remain attributable to one epoch. No code path may
open latest Delta state during an accepted query.

Crash injection covers every command stage. Restart deletes SQLite and in-memory caches
in selected cases, reconstructs control state from Delta/source, and reaches the same
unique epoch or a typed fail-closed state. Clean rebuild and incremental rebuild compare
complete logical facts and public results, with explicit ordering and float/null
semantics; digest equality alone is insufficient.

Overlay consolidation proof executes the visible base anti-join/union view, writes a
new Delta base, constructs a segment-free epoch, and compares logical rows, provenance,
unknowns, and public queries before activation. Vacuum dry-run proves no referenced
version/segment/result is eligible.

### 6.7 Independent semantic and public-surface proof

Acceptance expectations are authored as separate, reviewable relation fixtures with
explicit provenance and limitations. The production model/compiler never produces them.
They cover Python and Rust identity, types, imports/exports, calls/dispatch, CFG, MIR,
dataflow, ownership, unknowns, provider degradation, derived graph facts, source context,
composition, deterministic ordering, and negative-proof coverage.

The same logical expectations are decoded from:

- internal canonical Arrow relations;
- daemon gRPC/Arrow result streams;
- persisted result resources; and
- FastMCP STDIO responses.

A public vertical is accepted only if all four reflect the same epoch and semantics.
Wire compatibility tests exercise old supported clients against the new daemon/adapter
and reject genuinely incompatible versions at handshake.

### 6.8 Resource, security, and operations proof

Resource tests exercise admission, memory pool exhaustion, spilling, disk quotas, output
limits, deadline, cancellation, slow consumers, provider backpressure, graph
materialization, and concurrent-agent fairness on the real execution path. Killing a
query must stop DataFusion/provider/graph work and release reservations, spill files,
segments, and leases.

Security tests prove same-user UDS authentication, root/path authorization, symlink and
TOCTOU resistance, source ACLs, row/column/catalog filtering, information-schema
redaction, provider sandbox degradation, result-resource authorization, and rejection of
arbitrary SQL/physical identifiers. An unauthorized catalog name must be as invisible as
its rows. Negative compile- and runtime-path tests must also prove that the public query
port cannot receive or return an internal `TableProvider`, `SessionState`, `DataFrame`,
logical/physical plan, serialized plan, or unvalidated table identifier; authorization is
not considered proved if any such handle can bypass construction of the reduced child
catalog.

Performance profiles measure update-to-query freshness, epoch build/activation, common
semantic queries, compositional fan-out/fan-in, large graph operations, memory/spill,
Delta publication, overlay rebase, startup replay, and multiple-agent contention.
Performance policy changes only in response to these measurements and repeats semantic
equivalence proof.

### 6.9 Legacy zero-state proof

The final `just relational-fabric-legacy-zero-state-check` combines:

- `ast-grep` over all Rust/Python code and rule fixtures for imports, types, constructors,
  loaders, write paths, and compatibility aliases;
- `rg --hidden -g '!.git/**' -g '!docs/library_ref/**'` over code, manifests,
  configuration, packaging, scripts, CI, tests, and docs, with historical archive paths
  explicitly separated;
- skipped-file and parse-error inspection;
- Cargo metadata, per-feature builds, unused-dependency reconciliation, and package
  content inspection;
- Python wheel/sdist inspection and import/protocol tests;
- Protobuf regeneration and descriptor interoperability;
- catalog queries proving only target model/runtime authorities are registered; and
- clean reconstruction with legacy inputs physically absent from the test checkout.

The proof fails on a selectable fallback even if it is “disabled by default.” It also
fails if a legacy file remains solely to satisfy a presence/count/digest test.

## 7. Decision risks and reopen triggers

### 7.1 Principal design risks and controls

| Risk | Design control | Reopen trigger |
|---|---|---|
| bootstrap metamodel expands into another registry | fixed minimal algebra, self-description, closure and causality | a domain addition requires hard-coded bootstrap rows rather than a migration |
| generic compiler becomes an opaque VM | native DataFusion nodes, closed typed algebra, emitted decision dependencies, plan visibility | common relational semantics require custom bytecode or opaque physical execution |
| migration meaning changes across releases | every epoch pins `FabricCompilerRelease`; old-release replay precedes explicit migration/revalidation | reconstruction uses current code implicitly or an old release cannot be rebuilt |
| DataFusion catalog mutates after publication | builder-only mutable handles and sealed `Arc<FabricEpoch>` discipline | production code obtains a raw registration handle or mutable active context |
| global catalog leaks request metadata | internal-only full session plus authorized child catalog per `AccessScopeId` | a request can resolve a hidden provider/table/column through any planning API |
| Arrow metadata is mistaken for enforcement | semantic relation plus boundary validator and fault | correctness depends on metadata no consumer proves it reads |
| provider alignment loses current API facts | exact-version schemas, remainder rows, family coverage | a provider semantic is retained only in JSON/debug text or discarded |
| independent oracle is contaminated | separate provenance/ownership and mutant sensitivity | expected rows are generated by model/provider/compiler under test |
| overlay creates dual authority | durable immutable segments plus one canonical view and rebase equivalence | a query can select base or overlay semantics independently |
| model evolution breaks old epoch replay | immutable migrations and exact version/toolchain inputs | an accepted retained epoch cannot reconstruct or return typed incompatibility |
| local single-writer scope is violated | OS lease, writer generation, one actor, second/stale-writer negative proof | deployment requires concurrent multi-host mutation of one workspace |
| cutover revives legacy authority | durable cutover states and forward-only recovery after first new mutation | old binary can resume after `NEW_MUTATING` or both writer generations exist |
| public compatibility blocks cleanup | narrow released-contract inventory and tombstone transaction | an internal generated path is claimed compatible without an external consumer |
| total rewrite extends dual-run indefinitely | atomic serving cutover and same-plan legacy deletion | production writes reach both architectures or fallback remains after acceptance |

The design must be reopened if the v2 principles change, a required provider API cannot
expose the necessary facts, DataFusion cannot represent a load-bearing semantic operation
without an opaque general-purpose interpreter, the product requires concurrent
multi-host mutation of one workspace, or independent expectations reveal that the
selected ontology/query behavior itself is unsound.

### 7.2 Exact-version evidence and dossier validation

The library decisions were checked against the current manifests/locks, the two dated
DataFusion/Arrow and delta-rs references, and locally resolved exact source. The relevant
observed surfaces include:

- DataFusion 55 `SessionStateBuilder`, in-memory catalog providers, `ViewTable`,
  programmatic `Expr`/`LogicalPlanBuilder`, analyzer/optimizer hooks, UDF/UDAF/UDWF/table
  functions, `TableProvider::scan_with_args`, `ScanArgs.statistics_requests`, constraints,
  functional dependencies, memory pools, spill, task metrics, and recursive-query limits;
- Arrow 59 `DataType`, `Field`, `Schema`, `RecordBatch`, nested/dictionary/fixed-binary
  types, extension metadata, IPC, and Parquet mappings;
- delta-rs revision `43a0cf10...` exact-version table providers, DataFusion-plan writes
  with session state, atomic log creation through `PutMode::Create`, zero-retry conflict
  behavior, and object-store conditional-create requirements. These are table-commit
  capabilities, not the distributed writer-election or cross-table CAS mechanism this
  design deliberately does not claim; and
- current Ruff, Pyrefly, Tree-sitter, rustc, and petgraph APIs described in the pinned
  code-fact references and compiled manifests.

The read-only design work ran the repository/spec/library outlines, generated code and
non-code inventories, include-macro scans, targeted current-code searches, exact pin
inspection, `git diff --check`, the spelling checker on this dossier, and
`just artifacts-check`. `just artifacts-check` passed all 12 artifact-contract tests.

A fresh-context independent design challenge initially returned `NOT READY` on nine
load-bearing issues: activation concurrency, compiler-release identity, primitive/model
authority, request-scoped catalog security, cutover authority, graph extension shape,
producer-authored proof, legacy-disposition coverage, and provider-boundary completeness.
After the corresponding design corrections, the independent re-review returned
`ACCEPTED` with no remaining load-bearing blocker.

`just stable-graph-check` was attempted and did not reach graph validation because this
session's `cargo` executable rejected the script's `+nightly` invocation. That is
consistent with the session bootstrap's pre-existing dated-nightly extractor failure; it
is not evidence that the resolved graph is correct or incorrect. The implementation-plan
preflight must restore the extractor toolchain invocation and rerun the exact graph gate.

The pre-edit `just ci-fast` baseline remained red at `root-fmt` because of the pre-existing
unformatted edits in `src/ontology_program.rs` and `src/schema_registry.rs`. Those edits
were not modified by this design task.

## 8. Acceptance

This dossier selects the clean-sheet target and is ready for a new, versioned
implementation plan. The plan must supersede the current v3 plan, update the
authoritative suite, preserve the pre-existing dirty work, and make legacy deletion a
completion condition rather than a post-project cleanup. Concurrent multi-host writers
for one workspace are explicitly outside this accepted target; adding them requires a
new design with a proved distributed fencing protocol.

accepted
