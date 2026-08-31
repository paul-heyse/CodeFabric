---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-roadmap
suite_id: codefabric-relational-data-fabric
suite_version: 2.0.0
artifact_tag: RM
artifact_version: 1.0.0
authority_status: historical
successor_path: docs/authoritative_design/codefabric_2.1_implementation_roadmap_v1.0.md
predecessor_path: docs/authoritative_design/codefabric_1.3_implementation_roadmap_v1.0.md
---

# CodeFabric 2.0 relational data-fabric implementation roadmap

## 0. Authority and boundary

This roadmap has stable artifact identity
`codefabric-relational-data-fabric-roadmap`. It is subordinate to SUITE,
ONT, GEN, FAB, QRY, LIFE, and SRV. It orders implementation but cannot weaken,
reinterpret, or certify their contracts.

The approved versioned implementation plan is the execution authority for work
packet dependencies, acceptance checks, decommission batches, and proving
commits. This roadmap describes capability order and the reason for that order.

## 1. Sequencing invariants

The transition obeys these rules:

1. select one current v2 suite while labeling the running predecessor runtime
   explicitly legacy;
2. freeze independent expectations before shared implementation consumes them;
3. replay the minimal relational model before importing predecessor meaning;
4. establish relation-scoped Arrow IPC and logical/physical SchemaContracts
   before provider or persistence cutover;
5. emit exact provider-native relations before normalization and derived
   analyses;
6. complete every accepted analysis producer before semantic query closure;
7. construct and seal FabricEpoch catalogs before public serving;
8. route every durable mutation through FabricCommand before publication and
   activation;
9. prove reconstruction, recovery, and fenced cutover before legacy deletion;
10. delete old consumers before old authorities, then prove clean reconstruction
    with archived inputs unavailable.

No stage may introduce production dual writes or a silent fallback.

## 2. Capability stages

### 2.1 Stage 0 — Authority and evidence

WP01 establishes the sole current v2 target, historical predecessor routing,
released-compatibility classes, and executable legacy inventory/disposition
coverage.

WP22 freezes independently accepted provider, query, public, security,
activation, and comparator expectations before any implementation consumer
can adapt them.

Exit capability: the target and its semantic expectations are reviewable,
immutable, and causally discriminating; the runtime remains explicitly legacy.

### 2.2 Stage 1 — Replayed model foundation

WP02 implements the minimal metamodel, intrinsic algebra, compiler release, and
immutable migration replay.

WP03 performs the one-time predecessor import, produces independently reviewed
initial relations, and removes every live importer/static migration-input route
through DB01.

WP04 establishes typed Arrow relation schemas and one independently framed IPC
stream per relation under a bounded control envelope.

WP27 owns the model-derived logical/physical schema lifecycle across Arrow,
DataFusion, Delta, streams, batches, filters, projections, and statistics.

Exit capability: a clean checkout can replay the model and reproduce accepted
typed relations without generated registries or live migration inputs.

### 2.3 Stage 2 — Epoch catalog and compiled governance

WP05 constructs immutable epoch catalogs, honest provider contracts, and
sealed runtime foundations.

WP06 compiles schema, normalization, authority, derivation, semantic query,
policy, and proof relations into optimizer-visible DataFusion programs.

WP07 makes provenance, coverage, capability, proof, and governance executable
with pass/fail/unknown semantics and causal intervention.

Exit capability: one model produces schemas and plans whose meaning changes
when load-bearing model rows change.

### 2.4 Stage 3 — Exact providers and analysis closure

WP08 emits exact Tree-sitter and Ruff provider-native relations.

WP09 integrates the pinned Pyrefly Query, TSP/module-resolver, selected
Glean/internal, and LSP surfaces through relation-scoped Arrow IPC.

WP26 establishes the untrusted Rust compilation launcher and fail-closed trust
profiles before extractor integration.

WP10 emits exact rustc public MIR/instance relations and the narrow private
identity/source/borrow enrichment seam.

WP11 compiles normalization, authority, requested/completed coverage,
remainders, diagnostics, and unknowns across exact provider relations.

WP23 and WP24 implement Python owner-local flow and Rust MIR-derived flow
analyses. WP25 closes common graph, effect/resource, and interprocedural
producer authority.

Exit capability: every accepted fact and analysis family has exactly one
producer or an explicit unsupported remainder.

### 2.5 Stage 4 — Query, authorization, and serving

WP12 lowers graph and recursive operations at the highest correct DataFusion
rung and fully implements every selected custom physical contract.

WP13 compiles all eight bounded semantic request forms from request relations.

WP14 constructs reduced authorized child catalogs, recompiles views or proves
their complete bound dependency closure, and installs fresh allowlisted
function and object-store registries.

WP15 streams dynamic catalog results through the daemon and presentation-only
FastMCP adapter while preserving released public behavior.

Exit capability: an agent can issue every supported compositional request and
receive canonical bounded evidence from one pinned epoch without reaching an
internal table, plan, function, or session.

### 2.6 Stage 5 — Durable state and reconstruction

WP16 routes every model, source, provider, publication, activation,
maintenance, and administrative change through one idempotent FabricCommand
actor.

WP17 persists exact Delta relations and optimizer-visible epoch overlays with
one exact version selector, controlled zero-retry reconciliation, and schema
contract validation.

WP18 constructs, proves, activates, pins, and recovers one immutable
FabricEpoch in the required order: close admission, drain, append/read back
activation, swap, reconcile cache, reopen, then acknowledge.

WP19 integrates repository lifecycle, resources, cancellation, spill cleanup,
incremental invalidation, and clean reconstruction.

Exit capability: restart and unknown outcomes reconstruct the same semantic
head without guessing or mutable semantic pointers.

### 2.7 Stage 6 — Independent release and cutover

WP20 re-executes the preaccepted semantic, causal, security, public,
provider, activation, and performance evidence over the release candidate.

WP21 executes the durable forward-only cutover. NEW_MUTATING is impossible
until the exact frozen predecessor executable is mechanically denied serving
and writer authority across restart and reboot.

DB02 through DB08 remove predecessor provider, serving, storage, mutation,
query, model, generator, governance, dependency, package, live-history, and
comparator/archive residue at their earliest safe dependency boundary.

Exit capability: only the relational fabric can serve or mutate, and a clean
build cannot import or select predecessor functionality.

## 3. Dependency spine and permitted parallelism

The load-bearing spine is:

`WP01 -> WP22 -> WP02 -> WP03 -> WP04 -> WP27 -> WP05 -> WP06 -> WP07`

Provider work branches after the schema boundary:

- `WP08 -> WP23`;
- `WP09 -> WP23`;
- `WP26 -> WP10 -> WP24`;
- `WP08 + WP09 + WP10 -> WP11`;
- `WP23 + WP24 + WP11 -> WP12 -> WP25 -> WP13`.

Serving and durability converge through:

`WP13 -> WP14 -> WP15` and
`WP07 -> WP16 -> WP17 -> WP18 -> WP19 -> WP20 -> WP21`.

Parallel work is permitted only where the implementation plan shows a
dependency-closed boundary and integration checks can expose interaction
defects. File disjointness alone is not dependency closure.

## 4. Milestone exits

### 4.1 M01 — Replayed model foundation

The current suite, independent expectations, replay core, initial relations,
Arrow boundary, and SchemaContract are proved. Live import inputs are removed.

### 4.2 M02 — Exact provider fabric

All exact provider relations, semantic environments, trust profiles,
normalization, authority, remainders, and analysis producers are complete and
honest.

### 4.3 M03 — Authorized semantic delivery

All query forms execute through sealed child catalogs and the Rust daemon;
FastMCP is presentation only.

### 4.4 M04 — Durable reconstruction

FabricCommand, Delta persistence, overlay equivalence, activation order,
recovery, epoch pinning, lifecycle, and clean rebuild are proved.

### 4.5 M05 — Release and fenced cutover

Independent release evidence passes and the exact predecessor binary is
revoked before target mutation authority advances.

### 4.6 M06 — Total purge

Every legacy disposition is closed, every decommission batch is proved, all
old selectable authorities are absent, and the final gate matrix passes at one
trusted HEAD.

## 5. Decommission order

Deletion follows consumer-to-authority order:

1. one-time importer and live migration inputs;
2. legacy provider payload and procedural projection consumers;
3. old serving, storage, mutation, query, and adapter consumers;
4. model compiler, generated registries, manifests, bundles, and static
   semantic products;
5. predecessor governance, rules, recipes, tests, and live routing;
6. unused dependencies, features, targets, and package edges;
7. live history/comparator reads;
8. retained non-live comparator/archive after the accepted retention window.

Historical design, plan, review, release, and tombstone evidence is not deleted
merely because runtime authority is removed.

## 6. Evidence discipline

Each work packet:

- discovers the current change surface before editing;
- records execution judgment in schema-v2 state;
- implements immediate consumers and attached decommission coherently;
- runs every named packet oracle;
- receives a proving commit before completion;
- is re-opened if a later milestone exposes incomplete behavior.

Checks prove behavior; state labels do not. A passing digest or captured output
does not establish independently justified semantics. Negative legacy claims
require compiler/type proof plus structural and hidden-aware textual coverage
with skipped-file accounting.

## 7. Rollback and recovery

Before target writes, rollback selects one authoritative predecessor route; it
never marks both suites current. After new-format mutation, recovery is a new
forward FabricCommand and activation event under the target model. The old
binary never interprets the new activation log.

An unknown command outcome is reconciled from durable idempotency, Delta, and
activation facts. No controller invents success or repairs a digest.

## 8. Roadmap completion criterion

The roadmap is complete when all WP01-WP27 packets, M01-M06 milestones,
DB01-DB08 batches, and final required recipes are proved at a trusted HEAD;
the current runtime reconstructs from relational model and data alone; and
legacy functionality is physically absent outside immutable history and the
explicitly expired evidence archive.
