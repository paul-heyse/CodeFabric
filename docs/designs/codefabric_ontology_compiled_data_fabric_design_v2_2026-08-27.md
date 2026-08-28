---
artifact: design-dossier
design_id: codefabric-ontology-compiled-data-fabric
version: v2
date: 2026-08-27
status: accepted
baseline_commit: eebb958
working_tree_digest: 2fbc6c7061cb556c
supersedes: codefabric_ontology_compiled_data_fabric_design_v1_2026-08-27.md
primary_scope:
  - src/fabric.rs
  - src/fabric/
  - src/schema_registry.rs
  - src/semantic_query.rs
  - src/fact_ingest.rs
  - src/bin/codefabric_model/
  - src/generated/
  - contracts/schema/
  - contracts/registry/
  - docs/upfront_design/
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
governing_principles: docs/library_ref/full_data_fabric_design_principles.md
---

# CodeFabric ontology-compiled data fabric — target design v2

**Posture correction from v1.** v1 treated the upfront-design suite's decisions as hard
constraints and shrank the design to fit them. That was wrong: the suite records this
project's *own past decisions* and is revised in place. In this dossier only three things
are immovable — **(a)** targeted functionality, which must be maintained or enhanced;
**(b)** verified library facts at the pinned versions; **(c)** the design constitution
(`full_data_fabric_design_principles.md`, `P1`–`P25`). Every other prior decision is
re-examined on merit: reaffirmed where it wins, revised where a better approach exists —
and where the target departs from current spec text, **the corresponding spec amendment is
part of this design's transition**, not a reason to weaken the design.

Citation conventions: `REP §N` = `docs/reviews/representative_datafusion_arrow_relational_usage.md`;
`P1`–`P25` = the constitution; `align` = the DataFusion 55/Arrow 59 alignment manual
(pattern IDs by family); `FAB`/`ONT`/`QRY`/`LIFE`/`SUITE`/`GEN` = the suite (as *evidence
of prior decisions*, not as authority over this design). Baseline tree is intentionally
dirty with in-flight wave 8–12 work; the digest above identifies it.

## 1. Executive decision

**Target: the fabric becomes a fully ontology-compiled, extension-typed, self-describing
relational universe** — the `REP §18` end-state realized without reservation:

1. **The ontology lives in the catalog.** A dedicated `cpg_ontology` namespace serves the
   six ontology registries, the enum domains, and the provider raw-kind catalogs as
   queryable dimension tables, so `cpg.cpg_ontology.relation_kind` is a typed semantic
   address and `REP §13`-style conformance queries are standing relational gates.
2. **IDs are true logical types, per domain.** Each ID domain gets its own Arrow extension
   type (`codefabric.entity_id`, `codefabric.fact_id`, `codefabric.workspace_id`, …) over
   one physical representation — `FixedSizeBinary(16)` in the Arrow universe, `Binary` at
   the Delta storage seam — all generated from a new ID-domain registry and all registered
   in DataFusion 55's extension-type registry (`REP §5`, `§16`).
3. **One logical type system, everywhere.** Base tables, control projections, serving
   views, and query results all lower from the same generated vocabulary; the same concept
   is never typed two ways anywhere in the catalog, including the agent-facing result
   boundary.
4. **Structure follows the `REP §17` criterion, deliberately.** Independent
   identity/cardinality/join participation → relational (evidence stays a table; property
   values stay flat tagged columns); structurally-owned cohesive payloads → nested where
   the substrate proves it out (source-byte spans as a presence-coherent `Struct`,
   probe-gated against the pinned Delta kernel and production pruning path).
5. **Planning facts are truthful and non-degenerate**; the generation pipeline has one
   column authority and zero hand-written schemas or magic code literals anywhere between
   registry YAML and delivered `RecordBatch`.

The current implementation is preserved where it is genuinely the best design — the frozen
per-snapshot catalog, the typed `Expr`/`LogicalPlan` compiler, the universal fact tables,
Delta-native providers with overlay-as-plan-rewrite. It is **revised** where prior
decisions fall short of the target: the namespace layout, the single anonymous `id16`
extension, flat-only structure rules, the untyped control plane, hand-written result
schemas, and the split generation pipeline. Five spec amendments carry those revisions
(§5.4).

## 2. Constraints and target invariants

### 2.1 Genuine constraints

**Functionality contracts (maintain or enhance — never regress):**

- F-1. The eight semantic request forms and their response semantics; agents never see
  storage schema, operators, or database syntax (`QRY §4.2`, `§23.7`). Requests compile
  through typed `PlanSpec → BoundPlanSpec → LogicalPlan / GraphOperatorPlan`; SQL strings
  are not an IR.
- F-2. Atomic present state: every query pins exactly one immutable snapshot; publication
  activates by one pointer swap; base + one consolidated overlay; scope predicates sit
  below all user-controllable predicates (`LIFE §§100.3/100.6/104`, `FAB §91`).
- F-3. Absence is never proof of absence — explicit unknowns and capability gaps survive
  every representation choice (doctrine; `ONT` proposition envelope).
- F-4. Determinism: partition/batch-independent result identity (order-independent result
  checksums), reproducible clean rebuild, content-addressed snapshot identity.
- F-5. Provider isolation and application-owned canonical identity: 16-byte BLAKE3-128
  IDs from recipe-built CBEF preimages; provider-native handles never persisted as
  identity (`GEN §13`, `ONT §64`).
- F-6. Provenance closure from any delivered result back through plan, snapshot,
  publication, bundles, and producing operations (`P9`/`P10`; existing artifact
  accumulator and commit-metadata machinery).

**Library facts at the pins (verified this session against the pinned references):**

- L-1. DataFusion `=55.0.0`, Arrow/Parquet `=59.2.0`, `object_store =0.13.2`, deltalake
  git `43a0cf10`, Rust 1.95 — the dependency baseline this design builds on (movement is
  a separate decision this design does not require).
- L-2. The Delta kernel at `43a0cf10` has `BINARY` only — no fixed-size binary type — and
  **signed integer primitives only, no unsigned types** (which is why every current span
  column is `Int64`, and why the generated logical-type vocabulary has no unsigned
  member). FSB therefore cannot be the storage type at this pin, and no nested or flat
  column may declare unsigned children; FSB is the canonical Arrow contract type,
  reattached at the provider seam (the implementation already does this).
- L-3. DataFusion 55's extension-type registry (`ExtensionTypeRegistry`,
  `MemoryExtensionTypeRegistry`, `DFExtensionType`,
  `SessionStateBuilder::with_extension_type_registry`) supplies: field-aware cast
  preservation, extension-aware value formatting, and programmatic resolution for
  providers/UDFs. It does **not** change join/equality semantics; enforcement remains
  application-owned.
- L-4. Field metadata survives planning only on pass-through column paths; computed
  projections/aggregates drop it (documented drop-map) — re-annotation at shaping
  boundaries is an application obligation.
- L-5. Nested-struct predicates push to Parquet leaf columns since DF 54 for Parquet
  sources; behavior *through the Delta provider* and FSB-inside-Struct behavior are
  undocumented — probes required (§7).
- L-6. `ScalarValue::FixedSizeBinary` literals, FSB hash-join/group-by acceptance,
  SQL-level recursive CTEs, and `MemTable` sort-order declaration are undocumented at the
  pins — probes, with fallbacks named (§7).

**Constitution:** P1–P25 govern throughout; each decision below cites the principles it
advances or maintains.

### 2.2 Prior decisions re-examined

Every materially prior decision is dispositioned here — **reaffirm** (kept on merit) or
**revise** (superseded by this design, spec amendment attached). This table is the
clean-sheet challenge applied to the suite itself.

| Prior decision (evidence) | Verdict | Reasoning |
|---|---|---|
| Schemas compiled from one closed Contract IR; no hand-authored schema authority (`FAB §11`, `SUITE AC-G-05`) | **Reaffirm** | This *is* the constitution (`P1`/`P3`); the design deepens it |
| Universal `entity`/`relation`/`property_fact`/`fact_evidence`; no per-kind tables, no EAV/JSON (`FAB §5/§5.1`) | **Reaffirm** | Matches `REP §7/§15`: flat typed relation columns are what make graph semantics relational |
| Property values as flat tagged typed columns (`FAB §16.1`) vs `REP §8`'s struct union | **Reaffirm (deliberate REP deviation)** | `REP §17`'s own criterion decides: value variants are filtered independently (`value_int64 > 5` prunes on Parquet); a struct wrapper adds no semantics and costs pushdown. REP §8 itself re-projects flat columns for hot filters — the flat form is that projection, canonical |
| Frozen immutable per-snapshot catalog; providers die with the snapshot (`FAB §12.6/§91`, `LIFE §106`) | **Reaffirm** | `P11`; also exactly `REP §11` |
| Durable control-family tables (`workspace`, `publication`, `owner`, `capability_status`, …) live in `cpg_base` beside fact tables | **Reaffirm (now explicit)** | They are publication-pinned Delta state sharing the fact tables' exact lifecycle (manifest-pinned versions, snapshot leases, atomic activation). `cpg_control` differs by *lifecycle* (operational SQLite projections), `cpg_ontology` by *lifecycle* (bundle-pinned vocabulary) — the namespace planes separate lifecycles, not the word "control" |
| Persisted strings are `Utf8`; `Utf8View` compiler-rejected in schemas; dictionary encoding transient-only (`FAB §65.2/65.3`) | **Reaffirm storage, disposition execution → LD-07** | Delta STRING fixes storage. Execution-side view types are currently disabled at the Delta session binding; whether enabling them under the pinned provider is safe and profitable is a probe, not a guess (PR-7) |
| Delta as durable substrate; DataFusion plans; Arrow batches (`FAB §3.4`) | **Reaffirm** | MVCC/time-travel/atomic multi-table publication would otherwise be rebuilt by hand |
| Typed-plan-only query path; extension nodes/UDFs gated (`QRY AC-G-46`) vs `FAB §§78–82/93` recommended UDTFs | **Reaffirm QRY side, resolve the conflict** | UDTFs would hide traversal semantics from the plan validator and optimizer (`P15`); `GraphOperatorPlan` keeps them inspectable. FAB's UDTF recommendation is struck by amendment |
| Six catalog namespaces, registries crammed into `cpg_base`, ontology registries not served at all (`FAB §6.3/§8`) | **Revise → D-01** | The ontology deserves a first-class namespace; two control surfaces in one catalog is incoherent |
| One anonymous `codefabric.id16` extension type (`FAB AC-G-20`; `src/schema_registry.rs:11-69`) | **Revise → D-02** | ID domains are semantically distinct logical types; one anonymous width-16 type erases exactly the identity `REP §5` says to keep |
| `id16` = Arrow `Binary` in the spec table (`FAB §7`) — already contradicted by the implementation (FSB16 contract, gate-proven) | **Revise (regularize) → D-02** | The spec table lags governed reality; amendment makes FSB(16) + extension types normative with the Binary Delta storage mapping |
| Flat-only fact rows; nesting reserved for cold payloads (`FAB §9/§65.4`) | **Revise, narrowly → D-03** | Byte spans are structurally owned, presence-coherent payloads — the `REP §17` nested criterion; hot filter codes and independently-joining `file_id` stay flat |
| Untyped operational Arrow surface (27 control projections, `Binary`/no metadata) | **Revise → D-05** | Same concept must not be typed two ways in one catalog (`P3`/`P7`) |
| Hand-written result schemas at the agent boundary | **Revise → D-04** | The last boundary must be contract-compiled like every other (`P12`) |
| Statistics collapse to unknown under overlay; PKs never surfaced | **Revise → D-06** | Truthful-but-degenerate is still degenerate; Inexact composition is both truthful and useful (`P20`/`P15`) |
| Dual generated column lists + 29 hand-written row DTOs | **Revise → D-07** | Generation seam should be single (`P3`) |
| Magic certainty literals in the typed compiler | **Revise → D-08** | `P1` |
| `EXACT_PIN` schema-evolution ceremony sized for a deployed system | **Reaffirm mechanism, right-size sequencing** | Pre-production with no deployed namespace (operational handoff): fingerprint-moving changes are cheap *now* and grow more expensive with every wave that adds tables — sequencing exploits this (§5) |

### 2.3 Target invariants

- **TI-1 (ontology in the catalog).** Every governed vocabulary — six ontology
  registries, all enum domains, provider raw-kind catalogs — is a queryable dimension
  table under `cpg_ontology`, generated from the registry authorities with their bundle
  digests as columns; every `ontology:*`/`enum:*`/`opaque:provider-raw-kind` code column
  resolves to a name (and its registry row) inside the catalog; registry-conformance
  anti-joins are standing named gates. (`P2`, `P12`, `P25`; `REP §§1/13`)
- **TI-2 (per-domain logical ID types).** Every ID column carries the extension type of
  its domain (`codefabric.<domain>_id`), generated from an ID-domain registry in the
  Contract IR; all domains are registered in the serving session's extension-type
  registry; physically all are FSB(16) in Arrow and `Binary` in Delta through one seam.
  Cross-domain ID comparison is structurally visible (distinct logical types) even though
  the engine compares storage bytes. (`P8`, `P22`; `REP §§5/16`; `SCH-06`, `INT-09`)
- **TI-3 (one logical type system).** All four served surfaces — `cpg_base` tables,
  `cpg_control` projections, `cpg_serving` views, and query-form results — lower from one
  generated logical-type vocabulary through one lowering path with identical physical
  types, extension types, and metadata rules. (`P3`, `P7`; `SCH-01/02`)
- **TI-4 (deliberate structure).** The `REP §17` criterion is encoded in the Contract IR
  itself: each column group is classified relational or nested with its criterion
  recorded; byte-span presence coherence is guaranteed either structurally
  (`Struct{start_byte: Int64, end_byte: Int64}`, if PR-3a/3b pass) or by a named
  validation gate (flat fallback); evidence remains a table; property values remain flat
  tagged columns. (`P6`, `P8`, `P14`)
- **TI-5 (contract-complete result boundary).** Result Arrow schemas are generated per
  query form; computed projections re-annotate metadata by construction (L-4);
  extension types survive to the delivered batch. (`P12`; `MOD-08`, `EXP-11`)
- **TI-6 (truthful, non-degenerate planning facts).** Base-exact row counts compose with
  overlay counts as `Inexact` (never unknown, never falsely exact); PKs surface as
  DataFusion `Constraints`; every pushdown claim is per-filter truthful and
  adversarially tested. (`P15`, `P20`; `CAT-05/07`)
- **TI-7 (single generation seam).** One generated column authority per table; generated
  row shapes; zero hand-written schema, row-shape, or code-literal declarations between
  registry YAML and delivered batch. (`P1`, `P3`)
- **TI-8 (self-describing fabric).** From a leased catalog alone, a consumer can resolve:
  any code to its registry row, any ID column to its domain, any table to its contract
  identity/version/digest, any result to its snapshot and plan — without reading Rust.
  (`P9`, `P17`, `P24`; `REP §13` "the data describes itself")

### 2.4 Out of scope

The graph-projection runtime and traversal operators (roadmap W13+ — this design defines
where they attach: `GraphOperatorPlan` lowering plus the derived lane, never UDTFs); query
language expansion (W15/16); daemon multi-agent serving (W17); absolute performance SLOs
(Gate F); dependency-baseline movement; Python adapter beyond regenerated wire artifacts.

## 3. Target architecture

### 3.1 The semantic universe as served

One catalog (`codefabric`) per leased `ServingSnapshot`, frozen at construction:

```text
codefabric (immutable per leased ServingSnapshot)
│
├── cpg_ontology      the vocabulary plane — bundle-pinned dimension tables      [D-01]
│   ├── entity_kind · entity_family · relation_kind · relation_family
│   ├── property_kind · fact_kind
│   ├── enum_domain            (today's enum_catalog, relocated)
│   ├── provider_raw_kind      (tree-sitter / ruff / rustc / pyrefly raw catalogs)
│   └── id_domain              (the ID-domain registry itself)
│
├── cpg_base          durable universe — Delta-backed at exact pinned versions
│   ├── control family: workspace · publication · owner · capability_status · …
│   ├── entity · relation · property_fact · fact_evidence
│   └── detail tables (source, syntax, types, bindings, callables, cfg, dataflow)
│
├── cpg_python │ cpg_rust │ cpg_derived     language/derived extension tables
│                                            (empty until their waves fill them)
├── cpg_control       operational projections (SQLite-captured MemTables) — typed [D-05]
└── cpg_serving       generated agent-facing views (hidden columns dropped,
                      every code column name-joined against cpg_ontology)         [D-01]
```

Every level keeps the `REP §1` semantics: catalog = one internally coherent semantic
universe pinned to one snapshot; schema = one semantic plane; table = one canonical typed
relation; field = one ontology-defined contract; batch = immutable realization; plan =
compiled relational reasoning.

### 3.2 The compilation chain

```text
contracts/registry/*.yaml            contracts/schema/schema-contract-ir.json
  ontology-{entity,relation,           tables · columns · logical types ·
  property,fact} · enum · capability     structure classes · serving projections
  + NEW id-domain registry              + NEW dimension-table specs
  + provider raw-kind catalogs          + NEW result-schema specs
        └───────────────┬──────────────────────┘
                        ▼   src/bin/codefabric_model  (SchemaContractCompilation)
   one generated column/table authority (single list + row shapes)        [D-07]
   generated extension-type impls per ID domain                           [D-02]
   generated dimension-batch builders (ontology/enum/raw-kind/id-domain)  [D-01]
   generated result schemas per query form                                [D-04]
   generated code constants for the semantic compiler                     [D-08]
                        ▼   src/schema_registry.rs  (single lowering, all surfaces)
   Arrow Schema/Field: FSB16 + codefabric.<domain>_id + cf.* metadata
                        ▼   src/fabric/snapshot_catalog.rs · serving.rs
   pinned Delta providers → overlay wrap → scope wrap → stats wrap → frozen catalog
   SessionState with MemoryExtensionTypeRegistry (all ID domains)         [D-02]
                        ▼   src/semantic_query.rs
   typed PlanSpec → BoundPlanSpec → LogicalPlan/GraphOperatorPlan → batches
```

Dependency direction is one-way; nothing downstream re-declares an upstream shape
(`REP §3`; `P3`). The chain compiles *everything* the catalog serves — this is what
"ontology-compiled" means end to end.

### 3.3 D-01 — The ontology plane: `cpg_ontology`

**Decision.** A seventh namespace, `cpg_ontology`, holds every governed vocabulary as a
bundle-pinned, base-immutable dimension table. Storage follows the proven `enum_catalog`
pattern exactly: **Delta-backed `BundleDimension` tables, populated from generated
builders at publication, manifest-pinned, `required_for_publication`** — the namespace is
a serving-catalog organization, not a storage demotion, so there is no optional-mirror
dual authority: one Delta table per vocabulary, its `canonical_digest` column tied to the
registry bundle, served under `cpg_ontology` from the same pinned-version provider
machinery as every other durable table. Nine tables:

| Table | Authority | Load-bearing columns beyond `code`/`name`/`version`/`canonical_digest` |
|---|---|---|
| `entity_kind` | `ontology-entity-registry.yaml` | family code, language applicability, query visibility |
| `entity_family` | same | — |
| `relation_kind` | `ontology-relation-registry.yaml` | family code, allowed subject/object family code lists, cardinality, `symmetric`, `transitive`, `self_edge_policy`, owner-selection rule, query visibility |
| `relation_family` | same | — |
| `property_kind` | `ontology-property-registry.yaml` | value-kind code, cardinality, storage mapping |
| `fact_kind` | `ontology-fact-registry.yaml` | fact form |
| `enum_domain` | `enum-registry.yaml` (today's 73 domains) | domain, code, name — relocated from `cpg_base.enum_catalog` |
| `provider_raw_kind` | `contracts/generated/provider-raw-kinds/*.json` | provider code, raw kind code, raw name, normalized-kind mapping where owner-authored (`GEN §12`) — canonical and raw namespaces remain separate registries per `ONT AC-G-70`, now both queryable |
| `id_domain` | new ID-domain registry (D-02) | domain slug, extension-type name, preimage recipe id |

**Why a namespace and not more `cpg_base` tables** (v1's shape): the ontology is a
different semantic plane with a different lifecycle (bundle-pinned vocabulary vs
publication-pinned facts), and `cpg.cpg_ontology.relation_kind` is the typed semantic
address `REP §12` asks for. It also ends the incoherence of registries living beside fact
tables while a second "control" namespace holds operational state.

**Serving decoration.** `serving_view_plan` decoration extends from `enum:*` to
`ontology:*` and `opaque:provider-raw-kind` semantic types via generated left-joins
against `cpg_ontology` (mapping already generated:
`GENERATED_SEMANTIC_TYPE_BINDINGS`). `relations.relation_kind_code` finally resolves
in-catalog. Decoration coverage is **declared per projection in the Contract IR's
`serving_projections` records** — a contract decision per view, not a blanket join over
every code column — because each decoration is one `LEFT JOIN` in the view plan and
DataFusion 55's elimination of unused left joins (which needs right-side uniqueness
knowledge) is unverified: probe PR-6 measures it, dimension primary keys are declared as
`Constraints`, and the probe's outcome sets the default decoration breadth.
`enum_catalog` (table code 11) is renamed/relocated as `cpg_ontology.enum_domain` in the
same revision.

**Executable ontology.** Registry-conformance checks become standing relational gates
(TI-1): anti-join of fact code columns against their dimension tables (zero rows), plus
semantic conformance derived from the registries themselves — e.g. no `relation` row whose
kind's allowed-family lists exclude its subject/object families. These run at publication
validation alongside the existing FK integrity checks. This is `REP §13` delivered:
the data describes itself, and the ontology validates the data.

**Principles:** Advances `P2` (executable model), `P12`, `P24`, `P25`; Maintains `P3`
(YAML registries stay the authority; tables are compiled projections carrying digests).

### 3.4 D-02 — Per-domain logical ID types

**Decision.** A new **ID-domain registry** in the Contract IR enumerates every ID domain
(`entity`, `fact`, `evidence`, `workspace`, `analysis_context`, `owner`, `type`, `file`,
`publication`, …) with its extension-type name and preimage-recipe binding. The model
compiler generates one Arrow `ExtensionType` implementation per domain —
`codefabric.entity_id`, `codefabric.fact_id`, … — each storage-resolved to
`FixedSizeBinary(16)`, each carrying versioned metadata (`{"domain":…,
"preimage_version":…}`). Every ID column's Contract-IR declaration names its domain; the
single lowering path attaches the domain's extension type. `hash32` similarly becomes
`FixedSizeBinary(32)` + `codefabric.hash32`. The anonymous `codefabric.id16` type is
**retired** (zero-state proven).

**The mechanical justification: domain-checked plans.** Per-domain types are not adopted
for description alone. `BoundPlanSpec` validation gains a **domain-conformance rule**:
every equi-join key pair, ID literal, and IN-list in a bound plan must agree on ID domain
per the generated ID-domain registry (resolved through the session's extension-type
registry); a mismatch — joining `entity_id` to `fact_id`, filtering an entity column with
a fact literal — is a typed plan error before physical planning. The engine still
compares storage bytes; the *application plan layer* now enforces what the logical types
mean (`P2` — the model is executable; `LOG-07` — policy validation before physical
planning). This rule is what one-type-plus-metadata could not deliver cleanly, and it is
the deciding argument for per-domain types.

The serving `SessionState` is built through `SessionStateBuilder` with a
`MemoryExtensionTypeRegistry` registering every domain (L-3). Claimed engine behaviors
are exactly the verified surface, each with its named production consumer (`P21`, no
metadata theater): *programmatic resolution* → the domain-conformance plan rule above;
*field-aware cast preservation* → the storage-seam reattachment cast, which must not
strip extension identity in plans that project it onward; *extension-aware formatting* →
query-plan artifact and diagnostics rendering of ID literals. Enforcement authority
remains the application's one shared validation seam (below).

**Why per-domain names beat v1's single type + `id_domain` metadata:** the extension name
*is* the logical type identity in the Arrow ecosystem. Entity IDs and fact IDs are
different logical types that happen to share storage — `REP §5`'s exact point. Distinct
names make every schema self-describing to any Arrow consumer, give the DF registry
distinct resolutions per domain, and — through the domain-conformance rule above — make
cross-domain confusion a *rejected plan*, not a review hope. v1 rejected this to preserve
fingerprint stability — a deployed-system concern this pre-production project does not
have. List-typed ID columns (`IdList`) declare their **element** domain in the ID-domain
registry too, so list children carry the element's domain extension type; the
`codefabric.id16` zero-state proof covers scalar fields and list children alike.

**Storage seam (unchanged, reaffirmed).** Delta stores `Binary` (L-2). One seam —
`Id16ContractProvider`, generalized to the domain registry — re-presents storage as the
extension-typed FSB schema per scan; filter literals rewrite to storage type until the
FSB-literal probe (L-6) passes. The five scattered enforcement call-sites consolidate into
one generated validation helper with one test suite. Probes may absorb the reattachment
cast into `DeltaScanConfig::with_schema` (§7); if not, the visible `EXPLAIN`-able cast
stands, already covered by serving-equivalence oracles.

**Principles:** Advances `P8`, `P12`, `P18`, `P22`; Maintains `P20` (only verified engine
behavior claimed).

### 3.5 D-03 — Deliberate structure: the `REP §17` criterion, encoded

**Decision.** The Contract IR gains a structure classification per column group, recording
which side of the `REP §17` criterion it falls on — the criterion becomes contract data
either way the physical decision lands. Applied now:

- **Span offsets: flat `Int64` columns remain the default; the struct form is
  probe-gated, not pre-committed.** The candidate — `source_span: Struct{start_byte:
  Int64, end_byte: Int64}`, nullable as a unit with non-null children — would make
  span-presence coherence *structural* rather than a validation rule over independently
  nullable columns. But its children must be signed (L-2: the Delta kernel has no
  unsigned types — the reason today's spans are `Int64`), production span filtering rides
  statistics-based pruning rather than filter pushdown (the serving session disables
  Parquet filter pushdown at the Delta binding), and the fallback achieves coherence as a
  validation rule at zero churn. So the struct is adopted **only** if probe PR-3 —
  split into a Delta `STRUCT` type-mapping/round-trip leg and a file-skipping/pruning
  leg run **under the production session configuration** — passes both legs; otherwise
  flat columns stand with the classification recorded as relational-by-constraint.
  `file_id` stays a flat extension-typed FK column in every outcome: it joins
  independently (`REP §17`) and file-scoped queries keep first-class pruning.
- **`fact_evidence` remains a table** — independent identity and N:1 cardinality per fact
  (`REP §17`; also `REP §7`'s own relation row keeps hot codes flat with evidence
  attached, not inlined).
- **Certainty/resolution/directness codes remain flat** — hot filter columns (`REP §7`).
- **Property values remain flat tagged columns** — independent filterability (§2.2).
- Line/column display coordinates stay derivable from `source_file` line indexes (no
  duplication into spans).

Validation: struct round-trip through Delta `STRUCT` and predicate pushdown on
`source_span.start_byte` through the Delta provider are probes (L-5, §7); fallback if
pushdown regresses on span-filtered paths is recorded (flat columns retained for the
affected tables, classification updated) — the *criterion* stays in the IR either way.

**Principles:** Advances `P6`, `P8` (structure carries meaning), `P14`; Maintains F-4
(checksum path already validates nested types recursively).

### 3.6 D-04 — Generated result schemas

The query-form driver emits per-form, per-result-role response Arrow schemas through the
same lowering path as every table: extension-typed IDs, semantic metadata, deterministic
field order. `semantic_query.rs`'s three hand-written `Field::new` sites are replaced by
generated lookups; computed projections re-annotate via `alias_with_metadata`
(discharging L-4 by construction at the single shaping site). **The packed-`Binary` ID
sequences in path/pattern results (`ordered_entity_ids`, `ordered_fact_ids`,
`binding_entity_ids`, `witness_fact_ids`) are retyped as `List` of
extension-typed FSB(16) elements** — the current opaque byte-packing is entity identity
typed a second way at exactly the boundary TI-3 governs; the retyping is a response-shape
change owned by this stage (JSON/protobuf wire semantics unchanged — the packing was a
daemon-side Arrow artifact detail). `ResultChecksumV1` is
version-superseded by `ResultChecksumV2` over the richer canonical schema; V1 remains
verifiable for the released arrow-58/59 KATs until those retire with the next upgrade
plan — versioned coexistence by contract, never silent re-baselining.

**Principles:** Advances `P12` at the last boundary; `MOD-08`, `EXP-11`.

### 3.7 D-05 — Typed control plane

Operational projections declare logical types in the Contract IR (`Id16`+domain for every
16-byte ID, `TimestampUtc` for instants) and lower through the same path as `cpg_base` —
same physical types, extension types, metadata. SQLite DDL is untouched; blob→FSB
conversion happens at capture, where writers already enforce the 16-byte invariant. One
catalog, one type system (TI-3).

### 3.8 D-06 — Truthful statistics and constraints

- Overlay-present statistics compose **per mutation class, row-count only** (column
  statistics are never composed): `OwnerReplace` → `Inexact(base + overlay)` as a
  truthful upper bound; `PrimaryKeyUpsert` → `Inexact(base + overlay)` upper bound;
  `FullTableReplace` → `Exact(overlay)` (the replacement *is* the table); base-void
  derived replacement → `Inexact(overlay)`. Never `unknown` (degenerate), never falsely
  `Exact`. (`P20`)
- Generated primary keys surface as DataFusion `Constraints` on wrapped providers —
  **classified advisory** until a probe shows the 55 optimizer consumes them (candidate
  consumer: join-cardinality estimation; the truthfulness of the declaration is
  independent of consumption, so declaring is safe under `P20`).
- `ScanArgs::statistics_requests` answered only with cheap already-known values; the
  existing statistics-posture discipline extends through the overlay wrapper.
- The overlay provider's blanket `Exact` pushdown claim gets a standing adversarial test
  (filtered overlay-path execution vs engine-filtered reference) — the claim is proven,
  not remembered. (`CAT-05`, `TST-03`)
- Column min/max from Delta file statistics: probe first (§7), adopt if cheap and truthful.

### 3.9 D-07 — Single generation seam

One merged generated column list (type, nullability, semantic type, FK, domain, structure
class, `hidden_operational`, field id) replaces the `MODEL_TABLES` /
`GENERATED_TABLE_SPECS` pair and the runtime reconciliation between them; generated row
shapes replace the 29 hand-written `*Row` structs (ingest logic unchanged, shapes
drift-proof). Where this stage changes no schema bytes, the fingerprint comparator proves
it (§6).

### 3.10 D-08 — Registry-complete compiler

Generated code constants for every domain the semantic compiler filters on; the magic
integer literals are replaced; a `rules/` governance rule (with `rule-tests/` fixtures)
bans literal codes in predicate-construction modules thereafter.

**Discovered defect this design fixes (found by the independent challenge):** the two
hand-written certainty sites *disagree today* — the relational path maps "certainty is
exact" to codes `{10, 20}` while the graph path maps the same phrase to
`{10, 20, 30, 50}` (`src/semantic_query.rs:1341-1342` vs `:1965`). This is precisely the
drift `P1` predicts for duplicated hidden semantics. The authoritative phrase→code-set
binding is decided once, in the phrase registry, by the ontology owner; unification is a
recorded behavior fix with its own behavioral oracle on both paths — not silent
re-plumbing.

**Scope extension (phrase bindings).** Constants alone leave the phrase→predicate binding
itself as hand-written control flow. D-08 therefore also moves the *binding* into
contract data where it is mechanically expressible — phrase → (column, operator,
code-set) rows generated from the phrase registry — and, for bindings that are genuinely
algorithmic, a conformance gate ties each remaining match arm to its phrase-registry ID
so no arm exists without a governed binding. TI-7's "zero hand-written declarations
between registry YAML and delivered batch" is met by this combination.

### 3.11 Library decisions

### LD-01 — DataFusion 55 extension-type registry: adopt

**Decision:** adopt
**Version basis:** DataFusion `=55.0.0` (`ExtensionTypeRegistry`,
`MemoryExtensionTypeRegistry`, `DFExtensionType`,
`SessionStateBuilder::with_extension_type_registry` — verified, pinned reference §4/S7.20–21).
**Displaces:** nothing; adds engine-aware formatting, field-aware cast preservation, and
per-domain programmatic resolution over application-enforced extension types.
**Risk:** claiming unsupplied behavior. Mitigated: claims limited to the three verified
behaviors; consumers named in the metadata classification.
**Validation:** compile+execute probe (registration, formatter, cast-path); serving
oracle asserting registry contents match the generated ID-domain registry.

### LD-02 — Arrow 59 extension types, per ID domain: adopt (revises current)

**Decision:** adopt — generated per-domain `ExtensionType` impls; retire `codefabric.id16`
**Version basis:** arrow-schema `=59.2.0` extension module (verified; custom namespaced
names supported; `arrow.` namespace reserved and avoided).
**Displaces:** the single anonymous `Id16Extension` (`src/schema_registry.rs:11-69`) and
v1's `id_domain`-metadata compromise.
**Risk:** unknown-consumer degradation; fingerprint movement. Mitigated: `INT-09`
known/unknown-consumer round-trip tests per domain; fingerprint moves once in the
consolidated shape release while pre-production (§5.2).
**Validation:** extended `id16-extension-contract-check` successor
(`id-domain-extension-check`): per-domain preservation through provider, plan, and result
boundaries, plus Parquet metadata round-trip.

### LD-03 — deltalake `43a0cf10` Binary storage seam: retain-current

**Decision:** retain-current (kernel has no fixed-size type — L-2); revisit trigger: a
Delta upgrade adding fixed-width types.
**Validation:** round-trip gate continues; probes: `DeltaScanConfig::with_schema` as cast
absorber; struct span round-trip and nested pushdown (D-03).

### LD-04 — FSB literals / joins / nested keys: adopt-if-proven

**Decision:** adopt-if-proven; fallback (storage-typed literal rewrite) retained.
**Version basis:** `ScalarValue::FixedSizeBinary` and FSB join/group-by acceptance
undocumented at the pin (L-6).
**Validation:** plan-preflight compile+execute probes (point lookup, IN-list, two-table
FSB join, group-by on FSB, span-struct predicate).

### LD-05 — Recursive CTEs for traversal: reject

**Decision:** reject. Traversal semantics stay in inspectable `GraphOperatorPlan` nodes
lowering to relational plans plus the petgraph derived lane whose outputs return as
ordinary relation facts (`REP §15/§19`); SQL-level recursion is unverified at the pin and
would bypass the plan validator. UDTF alternatives are rejected on `P15` grounds (§2.2).

### LD-06 — MemTable for operational control projections: retain-current

Snapshot-captured operational batches; sort-order declaration not load-bearing.
(Dimension tables are Delta-backed per D-01's `enum_catalog` pattern, not MemTables.)

### LD-07 — String execution posture (Utf8View / dictionary): retain-current, probe-gated

**Decision:** retain-current (storage `Utf8`/Delta STRING; execution view types disabled
at the Delta session binding; dictionary encoding transient/writer-level only) — with a
probe before accepting that posture as final.
**Version basis:** DataFusion 55 defaults favor `Utf8View` on Parquet reads
(`schema_force_view_types`) and the fabric currently disables view types for Delta
round-trip stability; planning treats `Utf8`/`Utf8View`/dictionary strings as logically
equivalent.
**Displaces:** nothing; this dispositions a surface v2 previously left silent.
**Risk:** leaving the era's main string-execution win unexamined. Mitigated: probe PR-7
(view-types-enabled execution through the pinned Delta provider: correctness under the
round-trip gate, then measured benefit on string-heavy serving views); adopt only on a
clean probe and measured win, as a session-config change — never as a schema change.
**Validation:** PR-7; the round-trip gate and serving-equivalence oracles under the
toggled configuration.

**Ordering and functional dependencies** (dispositioned for completeness): `zorder`
remains layout-only; no declared sort orders or functional dependencies are exposed to
planning today, and none are added by this design — trigger for revisiting is
perf-evidence from the Gate-F workloads plus the PR-6 join-elimination outcome (dimension
PK `Constraints` are this design's first planning-visible uniqueness declaration).

### 3.12 Governance, state, failure (deltas)

- Plan-allowlist validation gains the `cpg_ontology` tables; `information_schema` stays
  off the agent path (agents get vocabulary through QRY forms and serving views; the
  ontology plane serves the *daemon's* compiler and diagnostics, and operators).
- Dimension batches are snapshot-scoped and die with the frozen catalog; no new caches or
  mutable state (`P23`).
- New failure points ride existing channels: dimension-referential and
  ontology-conformance violations fail publication validation; result-schema mismatches
  fail at batch validation with existing error classes; a registry/bundle digest mismatch
  at catalog construction fails snapshot construction (fail-closed, `LIFE §159.6`
  semantics preserved).

## 4. Alternatives and clean-sheet challenge

**Alternative A — v1's deference (deepen strictly inside current spec text).** Rejected —
it delivers TI-1 and TI-5–TI-7 but forfeits per-domain ID identity, the ontology
namespace, and deliberate structure, purely to avoid revising revisable decisions. Kept
from it: the gap analysis, D-04–D-08 substance, transition discipline.

**Alternative B — full literal `REP` transcription** (its exact namespace set
`ontology/facts/source/semantic/rust/derived`, nested provenance structs on fact rows,
struct-union property values, FSB storage). Rejected on merit, not deference:

- Its namespace *set* reorganizes fact tables by ontology domain but erases the
  lifecycle distinctions the fabric genuinely has (bundle-pinned vocabulary vs
  publication-pinned facts vs operational projections vs generated views). The selected
  design takes its *principle* — namespaces are semantic planes — and keeps the lifecycle
  planes that earn their existence.
- Nested provenance on fact rows and struct-union property values lose independent
  filterability/pushdown — condemned by `REP §17`'s own criterion (§2.2, §3.5).
- FSB storage is impossible at the pinned Delta kernel (L-2) — a library fact, not a
  preference.

**Clean-sheet answer.** If neither the implementation nor the suite existed, this design —
ontology plane, per-domain logical ID types, criterion-classified structure, one
generation seam, frozen snapshot catalogs, typed plans — is what the constitution and
`REP` would produce from scratch. The suite is amended toward it (§5.4); nothing is
retained out of incumbency. Surfaces retained (frozen catalog, overlay rewrite, typed
compiler, universal tables) are retained because the clean sheet reproduces them.

## 5. Transition, cutover, and legacy disposition

### 5.1 Position and timing

Pre-production, no deployed namespace, no external consumers — shape changes are at their
cheapest **now**, and every completed wave adds tables that would have to be migrated
later (waves 9–11 populate `cpg_python`/`cpg_rust` through this same pipeline).
**Recommendation: land this design as a dedicated implementation plan inserted before the
remaining semantic-profile waves continue** — a deliberate replan of the active waves 8–12
program (its own non-goals fence the fabric baseline, so this is a program-owner
decision, made once, with this dossier as its basis). The later boundary — the W12/W13
seam — remains viable but strictly more expensive: every wave executed first is a wave
born on the superseded shape. Stage 0 is safe immediately in either case.

### 5.2 Stages

- **Stage 0 — evidence floor (no schema, no behavior).** Perf baseline captured
  (`data-fabric-upgrade-bench` anchor — unrecoverable later); protective promotion of the
  serving-equivalence, checksum-KAT, catalog-freeze, and overlay-composition oracles into
  `tests/integration/`; gate filter-expression diff policy in force; the §7 probe suite
  executed and recorded (its outcomes bind Stage 2 details).
- **Stage 1 — re-plumbing plus one recorded behavior fix.** D-07 single column authority
  + generated row shapes; D-08 registry constants, phrase bindings, governance rule;
  session build through `SessionStateBuilder` (registry still empty). Schema bytes are
  proven unchanged (fingerprint-equality gate) — but this stage is **not** wholly
  behavior-neutral: unifying the divergent certainty sets (§3.10) changes one query
  path's results by design. That fix ships with its own behavioral oracle on both paths
  and the phrase-registry decision recorded; nothing else in the stage may move behavior.
- **Stage 2a — ID-domain release (governed Contract-IR revision).** ID-domain registry +
  per-domain extension types (+`hash32` FSB) [D-02], the domain-conformance plan rule,
  `codefabric.id16` retirement with zero-state proof. Fingerprint moves; migration is a
  workspace-local republish (pre-production clean rebuild) under the existing
  candidate-migration + owner-acceptance route.
- **Stage 2b — ontology-plane release (governed Contract-IR revision).** `cpg_ontology`
  namespace + nine Delta-backed dimension tables + `enum_catalog` relocation [D-01];
  per-projection serving decoration; dimension FK + ontology-conformance + property
  one-of publication checks; span-structure decision per the recorded PR-3 outcome
  [D-03]; the spec amendments (§5.4). Two releases, not one: the shape moves are cheap
  pre-production (§2.2), and two halves the review surface, decouples the
  PR-3-contingent span decision from the ID cutover, and gives rollback bundle-level
  grain. Delta reader/writer feature posture unchanged throughout (no kernel features
  enabled; rollback window preserved).
- **Stage 3 — result boundary.** D-04 generated result schemas + typed ID-list result
  columns + `ResultChecksumV2` + KAT continuity tests. Determinism/conformance snapshot
  baselines legitimately change here; re-baselining goes through the confirm-gated
  `snapshots-accept` with the diff reviewed — stated here so "continuity gates green"
  is honest about which baselines move and how.
- **Stage 4 — control plane.** D-05 typed operational projections (in-memory surface;
  no Delta or SQLite migration).
- **Stage 5 — planning facts.** D-06 statistics/constraints + adversarial pushdown-truth
  tests (no schema change).

Rollback: stages 1/4/5 revert by commit; stages 2a/2b each keep the prior schema bundle
activatable until owner acceptance is recorded; stage 3 keeps V1 checksums verifiable
throughout. No stage introduces dual authority; the only versioned coexistence is
`ResultChecksumV1`/`V2` with a named retirement condition (§3.6).

**Interaction with the executing waves 8–12 program (mechanism, not sentiment).** If the
replan-now recommendation is taken: (1) the waves 8–12 state file receives an
interruption record through its schema-v2 deviation mechanism — the plan is paused, not
corrupted, and its completed-packet history stays authoritative; (2) the active-plan
pointer moves to this design's plan through the existing confirm-gated activation
transaction (which never overwrites state and leaves the prior pointer on failure);
(3) **Stage-2b exit criterion:** an `integrate-plan-audit` pass over the *remaining*
wave 9–12 packets, whose oracle text and fixtures were specified against the superseded
shape — the cost asymmetry cuts both ways, and un-revised pending packets would
otherwise re-introduce the old shape packet by packet.

### 5.3 Legacy disposition matrix

Inventory generated by `ast-grep outline src/fabric src/fabric.rs src/schema_registry.rs
--items exports` and `ast-grep outline src/semantic_query.rs --items exports` (ast-grep
0.45.1, this session):

| Surface | Disposition | Justification |
|---|---|---|
| `schema_registry.rs` — `TableSpec`, policy enums, metadata dictionary, scope/projection specs, lowering | **preserve / extend** | the single lowering path gains domains, structure classes, and all-surface reach |
| `schema_registry.rs` — `Id16Extension` | **replace** | superseded by generated per-domain extension types (D-02); zero-state proven at Stage 2 |
| `schema_registry.rs` — `model_field` dual-list reconciliation | **delete** | D-07 |
| `schema_registry.rs` — `build_operational` untyped lowering | **reshape** | D-05 routes through the common path |
| `fabric.rs` — `exact_provider`, `Id16ContractProvider`, Delta validation/digest | **preserve / reshape** | correct storage seam (L-2); generalizes to the domain registry; probes may narrow the cast |
| `fabric.rs` — `enum_catalog` population | **reshape** | generalized into generated dimension builders for the nine `cpg_ontology` tables |
| `fabric/snapshot_catalog.rs` — frozen catalog machinery, handle factory, stats posture | **preserve / extend** | reaffirmed on merit; gains the ontology schema and D-06 statistics |
| `fabric/overlay.rs` — overlay machinery | **preserve / reshape (statistics + tested pushdown claim)** | composition rule reaffirmed; D-06 |
| `fabric/serving.rs` — session, immutable providers, artifact accumulator, allowlist | **preserve / reshape** | session build (D-02), view decoration (D-01), allowlist additions |
| `fabric/serving.rs` — control capture | **reshape** | D-05 typed capture |
| `fabric/publication.rs`, `mutation.rs` | **preserve / extend** | gain dimension FK + ontology-conformance checks |
| `fabric/result_checksum.rs` | **preserve + version** | V2 added; V1 retirement condition named |
| `semantic_query.rs` — pipeline, `BoundPlanSpec`, `GraphOperatorPlan`, set algebra | **preserve** | reaffirmed typed-plan architecture |
| `semantic_query.rs` — hand-written result schemas | **replace** | D-04 |
| `semantic_query.rs` — literal codes | **replace** | D-08 |
| `fact_ingest.rs` — 29 hand-written `*Row` structs | **replace (shape), preserve (logic)** | D-07 |
| `src/bin/codefabric_model/` drivers | **preserve + extend** | gains id-domain, dimension, result-schema, structure-class, merged-column emission |
| `src/generated/*` | **regenerate** | never hand-edited |
| `query_service.rs` transport/authorization/artifacts | **preserve** | no query construction inside |
| `cpg_python`/`cpg_rust`/`cpg_derived` empty namespaces | **preserve** | language waves fill them — on the new shape |

No `encapsulate-temporarily` surfaces; every stage is a complete gated architecture.

### 5.4 Spec amendments carried by this design

The suite is revised in place with the Stage-2a/2b Contract-IR revisions (drafted with
them, accepted together — amendment 2 with 2a; amendments 1, 3, 4, 5 with 2b):

1. **`FAB §6.3`** — add `cpg_ontology` to the namespace set; relocate `enum_catalog`;
   define the dimension-table plane and its bundle-pinned lifecycle.
2. **`FAB §7`** — `id16`/`hash32` rows become: canonical Arrow `FixedSizeBinary(16/32)`
   with per-domain extension types from the ID-domain registry; Delta storage mapping
   `BINARY` with the provider-seam reattachment contract (regularizes the already-shipped
   implementation and extends it per-domain).
3. **`FAB §8`** — dimension serving generalized from enum domains to all governed
   vocabularies; serving-view decoration covers `ontology:*` and raw-kind codes.
4. **`FAB §9`/`§65.4`** — structure classification per the `REP §17` criterion replaces
   the flat-only rule; `source_span` struct normative (or the recorded probe fallback);
   evidence-as-table and flat tagged property values reaffirmed explicitly.
5. **`FAB §§78–82/93` + `QRY AC-G-46` reconciliation** — the UDTF recommendation is
   struck; `GraphOperatorPlan` + derived lane is the sole sanctioned traversal path;
   `AC-G-20`'s extension-metadata example updates from `codefabric.id16` to the ID-domain
   registry.

`SUITE AC-G-05/06/07` artifacts (bundles, digests, registries) regenerate mechanically
from the same revision; `AC-G-79`'s fingerprint definition is unchanged — the fingerprint
simply moves once, at Stage 2, as designed.

## 6. Proof strategy

Existing gates are reused where they already prove an invariant; every new check is a
named `just` recipe. Stage binding per §5.2.

- **TI-1:** new `ontology-dimension-check` — (a) parity: dimension rows ≡ registry YAML ≡
  generated constants, digests equal; (b) referential zero-state: anti-join of every fact
  code column against its dimension, zero rows on a populated fixture publication;
  (c) semantic conformance: allowed-family/cardinality checks derived from
  `relation_kind` rows, plus the `property_fact` one-of coherence check
  (`value_kind_code` ↔ exactly one populated value column); (d) decoration: each
  projection-declared code column resolves to a name. Publication-integrity gate
  extended with the dimension FKs.
- **TI-2:** `id-domain-extension-check` (successor to `id16-extension-contract-check`) —
  per-domain extension preservation through provider schema, plan, result batch, and
  Parquet round-trip, scalar fields and list elements both; registry-contents oracle
  against the generated ID-domain registry; **domain-conformance rule tests** — a
  cross-domain join, a wrong-domain literal, and a mixed-domain IN-list are each rejected
  at plan validation with typed errors, and same-domain equivalents pass; negative:
  zero-state for `codefabric.id16` including list children (ast-grep + `rg` + tier-1
  clean build after deletion).
- **TI-3:** `model-family-check schemas` extended to all four surfaces; golden schemas
  compared under the full fingerprint definition (field order, nullability, extension
  metadata, governed metadata).
- **TI-4:** span-struct round-trip through Delta; presence-coherence negative test
  (struct-null vs child-null); pushdown probe result recorded with the plan; checksum
  path nested-type validation (already recursive) exercised on the new shape.
- **TI-5:** golden per-form result schemas incl. metadata; REANNOTATE test — a computed
  projection's delivered field carries re-attached metadata; negative: ast-grep + `rg`
  zero-state for hand-written `Field::new` in result-shaping modules + tier-1 deletion
  proof; `ResultChecksumV2` KATs + V1 continuity assertions.
- **TI-6:** overlay-present statistics tests per mutation class (the §3.8 composition
  table, including `FullTableReplace → Exact(overlay)` and the never-unknown floor); PK
  `Constraints` visibility test; adversarial pushdown-truth comparison on the overlay
  path; extended `provider-statistics-contract-check`.
- **TI-7:** Stage-1 fingerprint-equality gate; dual-list reconciliation deleted with
  tier-1 proof; governance rules in `governance-scan` for literal codes *and* for match
  arms without phrase-registry IDs; **behavioral oracle for the certainty-set
  unification** — the same phrase produces the same registry-decided code set on the
  relational and graph paths (the §3.10 divergence, fixed and pinned).
- **TI-8:** an end-to-end "self-description" oracle: starting from a leased catalog and a
  delivered result artifact, a test resolves — via queries and artifact records only —
  the result's snapshot, publication, plan identity, every code name, every ID domain,
  and every table contract version. This is the design's definition of done.
- **Continuity:** `wave3-integration-check`, `query-determinism-check`,
  `semantic-query-conformance-check`, `query-legacy-zero-state-check`,
  `data-fabric-stack-compat` green at every stage boundary; perf differential against the
  Stage-0 anchor per stage; filter-expression diffs for the ~20 name-coupled recipes with
  any test move.
- **Packet oracles:** each work packet derives its four oracles
  (behavioral/structural/negative/operational) from the TI it advances (`P25`).

## 7. Probe suite (Stage 0; binds Stage 2 detail decisions)

Executable probes at the pinned versions, each with its bound decision and fallback:

| Probe | Binds | Fallback if negative |
|---|---|---|
| PR-1 `ScalarValue::FixedSizeBinary` literal, IN-list, FSB hash-join/group-by | LD-04; D-02 predicate paths | storage-typed literal rewrite stands |
| PR-2 `DeltaScanConfig::with_schema` presenting FSB over Binary storage | LD-03 cast absorption | per-scan reattachment cast stands |
| PR-3a Span-struct type mapping: `Struct{Int64,Int64}` Delta `STRUCT` round-trip through the full `FAB §11.1`-style gate; FSB-adjacent struct casts | D-03 struct candidacy | flat span columns; classification recorded relational-by-constraint |
| PR-3b Span-struct pruning **under production session config** (Parquet filter pushdown disabled): file-skipping/statistics pruning on `source_span.start_byte` vs flat columns | D-03 adoption | same fallback as PR-3a |
| PR-4 Delta file-statistics exposure (column min/max) at the pin | D-06 scope | row-count/null composition only |
| PR-5 Parquet `ARROW:schema` metadata round-trip with per-domain extension names under current writer config | LD-02 validation detail | config adjusted (`skip_arrow_metadata=false` is already default) or metadata carried by manifest with round-trip test |
| PR-6 Unused-left-join elimination on decorated views at DF 55 (EXPLAIN over an undecorated projection; with and without dimension PK `Constraints` declared) | D-01 decoration breadth default | decoration declared narrowly per projection; name columns only where the view contract needs them |
| PR-7 View-types-enabled execution through the pinned Delta provider (correctness under round-trip + serving-equivalence oracles, then measured string-workload benefit) | LD-07 | execution view types stay disabled; posture recorded as final for this pin |

## 8. Acceptance

**accepted-with-named-assumptions** — ready for implementation planning (`impl-plan`),
with the §7 probe suite as plan-preflight validation. Assumptions (each labeled per
evidence policy):

1. **A-1 (assumption):** PR-1..PR-7 outcomes as tabled; every fallback named above is
   architecture-preserving, so no probe outcome invalidates the design — only Stage-2
   and posture detail.
2. **A-2 (assumption):** the Stage-2a/2b workspace-local republish path is exercisable
   under the existing candidate-migration machinery with no deployed-namespace
   obligations — confirmed by the migration probe that machinery already requires.
3. **A-3 — RESOLVED (owner decision, 2026-08-27):** the program owner approved landing
   this design **before the remaining wave 9–12 scope executes**; the §5.2
   waves-interaction mechanism (interruption record, pointer transaction, remaining-packet
   audit) applies.
4. **A-4 — RESOLVED (owner decision, 2026-08-27):** the phrase-registry binding is
   approved as the authoritative mechanism; the concrete phrase→certainty-code set is
   recorded in the phrase registry during Stage 1 with owner acceptance, and the Stage-1
   behavioral oracle pins it on both query paths.

Evidence that would force reopening: a Delta-pin upgrade adding fixed-width binary types
(reopens LD-03 toward native FSB storage — a strict simplification of D-02's seam); a QRY
revision adding struct-shaped response fields (extends D-03/D-04); an accepted
`ExtensionDecisionRecord` for traversal operators (interacts with LD-05's boundary);
probe PR-3 failing *and* span-filtered query paths later becoming hot (reopens the D-03
fallback).
