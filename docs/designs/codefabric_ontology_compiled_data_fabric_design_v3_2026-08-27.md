---
artifact: design-dossier
design_id: codefabric-ontology-compiled-data-fabric
version: v3
date: 2026-08-27
status: accepted
baseline_commit: eb7a738fa55037b19706fd842737cecad65ffe16
working_tree_digest: 174abedf68765285989783ba89d8d7de09657bc60cc8390730d4f802c6a16395
supersedes: codefabric_ontology_compiled_data_fabric_design_v2_2026-08-27.md
audit_source: docs/reviews/plan_audit_codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27_v1.md
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

# CodeFabric ontology-compiled data fabric — target design v3

**Audit integration from v2.** This version preserves v2's intentional reversal of prior
design decisions while closing the independent plan audit's eleven findings. It adds one
typed `CompiledOntology` model and governed compilation pass, normalizes ontology
memberships, makes self-description complete and testable from the catalog, installs
universal ID-domain enforcement in DataFusion's analyzer path, separates logical
structure classification from physical lowering, and gives Stage 2b one atomic activation
owner. No finding is deferred; the parallel implementation plan records the exact
disposition and implementation-owned revalidation command for each finding.

**Posture correction retained from v1.** v1 treated the upfront-design suite's decisions as hard
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
of prior decisions*, not as authority over this design). The baseline is the WP08-complete
commit; the working-tree digest identifies the contemporaneous uncommitted documentation
set without treating those documents as implementation evidence.

## 1. Executive decision

**Target: the fabric becomes a fully ontology-compiled, extension-typed, self-describing
relational universe** — the `REP §18` end-state realized without reservation:

1. **The ontology lives in the catalog completely and relationally.** A dedicated
   `cpg_ontology` namespace serves every governed vocabulary and contract authority as
   normalized, bundle-pinned relations. A universal term index and typed ontology edges
   make N:M memberships queryable without nested-list scans; table, column, result,
   identity, phrase, and rule contracts make the fabric recursively self-describing.
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
5. **One compiled semantic object drives all generated operations.** A typed
   `CompiledOntology` is produced by one governed `SchemaContractCompilation` pass and is
   the sole model consumed by schema generation, row shapes, ontology relations,
   phrase/rule plans, result schemas, identity recipes, and validation. Planning facts are
   truthful and non-degenerate; no parallel semantic declaration survives between
   registry YAML/JSON and the delivered `RecordBatch`.

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
  `SessionStateBuilder::with_extension_type_registry`) provides programmatic resolution
  and formatting behavior through registered `DFExtensionType` factories. Arrow
  `ExtensionType` implementations and field-aware storage casts are separate application
  surfaces; registration does **not** make the optimizer domain-aware or enforce
  join/equality semantics. Enforcement remains application-owned through an installed
  analyzer rule.
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

- **TI-1 (complete ontology in the catalog).** Every governed vocabulary and semantic
  authority — ontology/enum/raw-kind codes, capability and diagnostic codes, table and
  column contracts, semantic-type bindings, result schemas and fields, identity recipes,
  phrase bindings, and compiled rule contracts — is a normalized queryable relation under
  `cpg_ontology`, generated from owner authorities with bundle digests. Every governed
  code resolves through `ontology_term`; every N:M membership resolves through
  `ontology_edge`; catalog-only anti-joins and relational rule plans are standing named
  gates. (`P2`, `P12`, `P25`; `REP §§1/13`)
- **TI-2 (per-domain logical ID types).** Every ID column carries the extension type of
  its domain (`codefabric.<domain>_id`), generated from an ID-domain registry in the
  Contract IR; all domains are registered in the serving session's extension-type
  registry; physically all are FSB(16) in Arrow and `Binary` in Delta through one seam.
  Cross-domain comparison is rejected by one idempotent DataFusion `AnalyzerRule` installed
  in every serving session and reached by every logical-plan ingress; early bind checks may
  delegate to the same generated rule model but are never a second authority. (`P8`,
  `P22`; `REP §§5/16`; `SCH-06`, `INT-09`)
- **TI-3 (one logical type system).** All four served surfaces — `cpg_base` tables,
  `cpg_control` projections, `cpg_serving` views, and query-form results — lower from one
  generated logical-type vocabulary through one lowering path with identical physical
  types, extension types, and metadata rules. (`P3`, `P7`; `SCH-01/02`)
- **TI-4 (deliberate structure).** The `REP §17` criterion is encoded logically in the
  Contract IR before physical probing: source spans are classified as structurally-owned
  cohesive payloads; evidence remains an independently identified relation; property
  values remain independently filterable flat tagged columns. PR-3 selects only the
  physical lowering for the already-classified source span — nested `Struct` when the
  pinned production path proves it, otherwise a flat representation with an explicit
  presence-coherence constraint. (`P6`, `P8`, `P14`)
- **TI-5 (contract-complete result boundary).** Result-schema and result-field authorities
  live in the Schema Contract IR and compile into both query-form bindings and Arrow
  schemas; computed projections re-annotate metadata by construction (L-4); extension
  types survive to the delivered batch. (`P12`; `MOD-08`, `EXP-11`)
- **TI-6 (truthful, non-degenerate planning facts).** Base-exact row counts compose with
  overlay counts as `Inexact` (never unknown when a manifest count exists, never falsely
  exact); `ScanArgs::statistics_requests` remains explicitly declined until DataFusion has
  a demonstrated production consumer and cheap truthful response; validated PKs surface
  as DataFusion `Constraints` classified as planner-consumed/advisory and not
  DataFusion-enforced; every pushdown claim is per-filter truthful and adversarially
  tested. (`P15`, `P20`; `CAT-05/07`)
- **TI-7 (single generation seam).** One typed `CompiledOntology` is the complete output
  of one governed `SchemaContractCompilation` transformation pass. All schemas, row
  shapes, dimension relations, semantic operation/rule plans, result contracts, identity
  recipes, and generated code constants consume it; zero parallel declarations or
  literal semantic control flow survive. (`P1`, `P2`, `P3`)
- **TI-8 (self-describing fabric).** From a leased catalog and a delivered result artifact
  alone, an operator-side consumer can resolve any code, ontology edge, registry authority,
  semantic type, table/column/result contract, ID domain and identity recipe, phrase/rule
  binding, snapshot, publication, and plan identity — without fixed table-name knowledge,
  generated Rust/Python constants, or direct registry-file reads. This does not add an
  agent-facing query form; existing serving views and diagnostics remain the agent
  boundary. (`P9`, `P17`, `P24`; `REP §13` "the data describes itself")
- **TI-9 (one governed semantic-operation model).** Every mechanically expressible
  validation, phrase predicate, ontology-conformance rule, and result-shaping operation is
  a closed typed variant in `CompiledOntology`; one registry-declared transformation pass
  lowers those variants to typed DataFusion `Expr`/`LogicalPlan` fragments. Algorithmic
  operations that cannot use the closed relational algebra remain explicitly typed,
  named, and conformance-linked — never opaque JSON/EAV or ad hoc match arms. (`P1`,
  `P2`, `P13`, `P15`)

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
├── cpg_ontology      vocabulary + contract plane — twenty bundle-pinned tables  [D-01]
│   ├── nine typed vocabulary dimensions (kind/family/enum/raw-kind/id-domain)
│   ├── ontology_term · ontology_edge · registry_authority
│   ├── semantic_type_binding · table_contract · column_contract
│   ├── result_schema · result_field · identity_recipe
│   └── phrase_binding · rule_contract
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
contracts/registry/*.yaml/json       contracts/schema/schema-contract-ir.json
  ontology-{entity,relation,           tables · columns · logical types ·
  property,fact} · enum · capability     structure classes · serving projections
  + NEW id-domain registry              + NEW ontology/self-description specs
  + provider raw-kind catalogs          + NEW result-schema and typed-rule specs
        └───────────────┬──────────────────────┘
                        ▼   src/bin/codefabric_model
              SchemaContractCompilation transformation pass             [D-09]
              validates + emits one typed CompiledOntology               [TI-7]
   one generated column/table authority (single list + row shapes)        [D-07]
   generated extension-type impls per ID domain                           [D-02]
   generated normalized ontology/self-description batch builders          [D-01]
   generated result schemas per query form                                [D-04]
   generated typed semantic-operation and rule plans                      [D-08/D-09]
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

**Decision.** A seventh namespace, `cpg_ontology`, holds every governed vocabulary and
contract authority as normalized bundle-pinned, base-immutable relations. Storage follows
the proven `enum_catalog` pattern exactly: **Delta-backed `BundleDimension` tables,
populated from generated builders, manifest-pinned, `required_for_publication`**. The
namespace is a serving-catalog organization, not a storage demotion: YAML/JSON and Schema
Contract IR remain owner authorities; all twenty tables are deterministic projections of
the same `CompiledOntology`, carry their source authority/digest, and never become a
second authoring surface.

The first nine tables are ergonomic typed vocabulary projections:

| Table | Authority | Load-bearing columns beyond `code`/`name`/`version`/`canonical_digest` |
|---|---|---|
| `entity_kind` | `ontology-entity-registry.yaml` | family code, language applicability, query visibility |
| `entity_family` | same | — |
| `relation_kind` | `ontology-relation-registry.yaml` | family code, cardinality, `symmetric`, `transitive`, `self_edge_policy`, owner-selection rule, query visibility; no list-valued memberships |
| `relation_family` | same | — |
| `property_kind` | `ontology-property-registry.yaml` | value-kind code, cardinality, storage mapping |
| `fact_kind` | `ontology-fact-registry.yaml` | fact form |
| `enum_domain` | `enum-registry.yaml` (today's 73 domains) | domain, code, name — relocated from `cpg_base.enum_catalog` |
| `provider_raw_kind` | `contracts/generated/provider-raw-kinds/*.json` | provider code, raw kind code, raw name, normalized-kind mapping where owner-authored (`GEN §12`) — canonical and raw namespaces remain separate registries per `ONT AC-G-70`, now both queryable |
| `id_domain` | new ID-domain registry (D-02) | domain slug, extension-type name, preimage recipe id |

Eleven normalized self-description relations close the metadata loop:

| Table | Grain and purpose |
|---|---|
| `ontology_term` | one stable term per governed integer/text code or contract concept; authority, semantic type, mutually exclusive typed code value, canonical name, version, digest |
| `ontology_edge` | one typed N:M semantic edge `(subject_term_id, predicate_term_id, object_term_id, ordinal)`; allowed-family, membership, owner, required/optional property, and projection-membership lists are rows here |
| `registry_authority` | one owner authority/version/digest and its canonical source identity |
| `semantic_type_binding` | one semantic type to resolver/domain/registry/table/column binding |
| `table_contract` | one table code/address, lifecycle class, contract version/digest, PK and publication requirement |
| `column_contract` | one ordered table field with logical type, semantic type, ID domain, nullability, FK, structure class, and governed metadata |
| `result_schema` | one query-form/result-role schema identity/version/digest |
| `result_field` | one ordered result field with the same logical/semantic authority as table columns |
| `identity_recipe` | one canonical identity recipe, preimage version, domain, and output logical type |
| `phrase_binding` | one phrase-registry binding to a typed semantic operation/rule contract |
| `rule_contract` | one closed typed relational/validation operation with inputs, outputs, determinism class, diagnostics, and owner authority |

`ontology_term` is the universal resolver for code authorities not deserving their own
typed dimension (for example capability and diagnostic codes). Typed vocabulary tables
remain ergonomic projections, not parallel authorities. `ontology_edge` is the canonical
relational representation for every N:M membership; nested code lists are forbidden in
the ontology plane.

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
DataFusion 55's elimination of unused left joins is unverified: PR-6 measures it with and
without experimental constraints over independently validated unique dimensions. The
reviewed decision sets decoration breadth; production constraints remain withheld until
uniqueness validation and D-06 classification are complete.
`enum_catalog` (table code 11) is renamed/relocated as `cpg_ontology.enum_domain` in the
same candidate revision. Table codes and all twenty table/column/result contracts are
themselves discoverable through `ontology_term`, `table_contract`, and `column_contract`.

**Executable ontology.** Registry and semantic conformance are compiled from
`rule_contract` through D-09's typed operation model into ordinary DataFusion plans:
anti-join every governed code against `ontology_term`/its typed projection, anti-join
every ID-domain and FK binding against contract relations, and validate relation-family,
cardinality, owner, and property one-of rules through `ontology_edge`. The oracle starts
with the leased catalog's `registry_authority` and `table_contract` relations, dynamically
discovers the rest of the plane, and contains no hard-coded table census or generated
constants. This is `REP §13` delivered recursively: the data describes itself and the
compiled ontology validates the data.

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
for description alone. The generated model contains `DomainTypedLiteral` and domain-aware
expression contracts. One idempotent application-owned DataFusion `AnalyzerRule`,
installed in every serving `SessionState`, traverses every logical plan after resolution
and rejects mismatched comparison predicates, equi/non-equi joins, `IN` lists, explicit
casts, set-operation column alignments, and extension/storage reinterpretations. It is
therefore reached by semantic query lowering, serving-view construction, direct provider
plans, and future authorized plan ingresses rather than only one binder path. A binder
may emit the same typed diagnostic earlier, but delegates to the identical generated rule
model and is not a second authority. Domain erasure or an unknown domain fails closed.

The serving `SessionState` is built through `SessionStateBuilder` with a
`MemoryExtensionTypeRegistry` registering one generated `DFExtensionType` factory per
domain and with the analyzer rule installed exactly once (L-3). Responsibilities are
explicit: Arrow `ExtensionType` impls validate field metadata; DataFusion registrations
resolve/format the logical type; field-aware casts reattach the Arrow contract at the
storage seam; the analyzer rule enforces domain semantics. The registrations and analyzer
rule both consume `CompiledOntology`, so extension metadata cannot create a parallel
policy registry.

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
FSB-literal probe (L-6) passes. The five scattered field-validation call-sites consolidate
into one generated helper; they validate schemas but do not replace plan-wide semantic
enforcement. `DeltaScanConfig::with_schema` is adopted only if PR-2 proves the complete
production provider path, including pruning, projection, filter, statistics, and emitted
batch schemas; otherwise the existing wrapper remains the default seam.

**Principles:** Advances `P8`, `P12`, `P18`, `P22`; Maintains `P20` (only verified engine
behavior claimed).

### 3.5 D-03 — Deliberate structure: the `REP §17` criterion, encoded

**Decision.** The Contract IR gains a logical structure classification per column group,
recording which side of the `REP §17` criterion it falls on independently of its physical
lowering. Applied now:

- **Source spans are logically `StructurallyOwnedCohesive` in every outcome.** The
  preferred physical lowering is `source_span: Struct{start_byte: Int64, end_byte:
  Int64}`, nullable as a unit with non-null children. Its children remain signed (L-2).
  PR-3 selects physical lowering only: if the full pinned Delta round-trip and production
  pruning path pass, lower to the struct; otherwise lower to the two existing flat
  `Int64` columns and attach a compiled presence-coherence constraint. A negative physical
  probe never reclassifies the concept as relational.
  `file_id` stays a flat extension-typed FK column in every outcome: it joins
  independently (`REP §17`) and file-scoped queries keep first-class pruning.
- **`fact_evidence` remains a table** — independent identity and N:1 cardinality per fact
  (`REP §17`; also `REP §7`'s own relation row keeps hot codes flat with evidence
  attached, not inlined).
- **Certainty/resolution/directness codes remain flat** — hot filter columns (`REP §7`).
- **Property values remain flat tagged columns** — independent filterability (§2.2).
- Line/column display coordinates stay derivable from `source_file` line indexes (no
  duplication into spans).

Validation: struct round-trip through Delta `STRUCT` and statistics/file-skipping pruning
on `source_span.start_byte` through the production Delta provider are probes (L-5, §7).
The decision transaction records only `NestedStruct` or `FlatConstraintLowering`; the
logical classification is stable contract data in either branch.

**Principles:** Advances `P6`, `P8` (structure carries meaning), `P14`; Maintains F-4
(checksum path already validates nested types recursively).

### 3.6 D-04 — Generated result schemas

Result-schema and result-field records are first-class Schema Contract IR authorities.
Every query-form/result-role binding references one `result_schema_id`; the
`SchemaContractCompilation` pass validates totality and emits response Arrow schemas
through the same lowering path as every table: extension-typed IDs, semantic metadata,
deterministic field order. `semantic_query.rs`'s three hand-written `Field::new` sites are
replaced by generated lookups; computed projections re-annotate via `alias_with_metadata`
(discharging L-4 by construction at the single shaping site). **The packed-`Binary` ID
sequences in path/pattern results (`ordered_entity_ids`, `ordered_fact_ids`,
`binding_entity_ids`, `witness_fact_ids`) are retyped as `List` of
extension-typed FSB(16) elements** — the current opaque byte-packing is entity identity
typed a second way at exactly the boundary TI-3 governs; the retyping is a response-shape
change owned by this stage (JSON/protobuf wire semantics unchanged — the packing was a
daemon-side Arrow artifact detail). `ResultChecksumV1` is version-superseded by
`ResultChecksumV2` over the richer canonical schema. Selection is result-schema-version
gated so old leases keep V1/current shapes while new Stage-3 leases use V2/target shapes;
the result-authority transaction advances ontology rows and runtime selection together. V1 remains
verifiable for the released predecessor-upgrade KATs until those retire with the next upgrade
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
- Generated primary keys surface as DataFusion `Constraints` on wrapped providers only
  after publication validation proves uniqueness. They are classified
  **planner-consumed/application-validated/not DataFusion-enforced**; they never stand in
  for validation or storage enforcement. PR-6 records whether the pinned optimizer
  consumes them for join elimination, and decoration breadth does not assume that result.
- `ScanArgs::statistics_requests` remains **declined** at this pin. The referenced API is
  not a general statistics request/response protocol and no production consumer justifies
  a custom chain. Cheap truthful statistics are exposed through the ordinary
  `TableProvider`/`ExecutionPlan` statistics surfaces and `StatisticsContext`; reopening
  requires a named consumer, an API probe, and an explicit decision transaction.
- The overlay provider's blanket `Exact` pushdown claim gets a standing adversarial test
  (filtered overlay-path execution vs engine-filtered reference) — the claim is proven,
  not remembered. (`CAT-05`, `TST-03`)
- Column min/max from Delta file statistics: probe first (§7), adopt if cheap and truthful.

### 3.9 D-07 — Single generation seam

One merged column model inside `CompiledOntology` (type, nullability, semantic type, FK,
domain, structure class, `hidden_operational`, field id) replaces the `MODEL_TABLES` /
`GENERATED_TABLE_SPECS` pair and runtime reconciliation between them. Generated row
shapes replace the 29 hand-written `*Row` structs (ingest logic unchanged, shapes
drift-proof). Where this stage changes no schema bytes, the fingerprint comparator proves
it (§6). No downstream generator reparses Contract IR or a registry authority.

### 3.10 D-08 — Registry-complete compiler

Generated code constants for every domain the semantic compiler filters on are projections
of `CompiledOntology`; magic integer literals are replaced; a `rules/` governance rule
(with `rule-tests/` fixtures) bans literal codes in predicate-construction modules
thereafter.

**Discovered defect this design fixes (found by the independent challenge):** the two
hand-written certainty sites *disagree today* — the relational path maps "certainty is
exact" to codes `{10, 20}` while the graph path maps the same phrase to
`{10, 20, 30, 50}` (`src/semantic_query.rs:1341-1342` vs `:1965`). This is precisely the
drift `P1` predicts for duplicated hidden semantics. The authoritative phrase→code-set
binding is decided once, in the phrase registry, by the ontology owner; unification is a
recorded behavior fix with its own behavioral oracle on both paths — not silent
re-plumbing.

**Scope extension (phrase bindings).** Constants alone leave phrase→predicate semantics
as hand-written control flow. D-08 moves each mechanically expressible binding into a
closed typed `SemanticOperationSpec` variant — column reference, operator, typed operand
or code set, null/unknown policy, and output role — inside `CompiledOntology`. D-09 lowers
these through the same governed pass as ontology-conformance rules. Genuinely algorithmic
bindings use an explicit typed algorithm variant with a phrase-registry ID, inputs,
outputs, determinism class, and diagnostic contract; opaque match arms are forbidden.

### 3.11 D-09 — One compiled ontology and one semantic-operation pass

**Decision.** `SchemaContractCompilation` is a registered transformation pass over the
owner-specific registry YAML/JSON plus Schema Contract IR. Its complete output is one
versioned typed `CompiledOntology` containing vocabulary terms/edges, table and result
contracts, semantic-type bindings, ID domains and identity recipes, phrase bindings, and
closed typed semantic-operation/rule variants. The pass registry declares its full input
set, output families, invalidation keys, determinism class, diagnostics, dependency
ordering, and dual-generation reproducibility oracle.

All downstream generators and runtime installers consume `CompiledOntology`; none parses
an authority independently. Relational operations compile to DataFusion `Expr` and
`LogicalPlan` fragments, with DataFusion kernels/functions used for casts, comparisons,
joins, anti-joins, set operations, aggregation, null logic, and constraint checks.
Application Rust owns orchestration, diagnostics, and the few typed algorithm variants
that DataFusion cannot express; it does not duplicate relational calculations row by row.
The model is closed and strongly typed — never an EAV rule table or arbitrary JSON AST.

**Why this consolidation is load-bearing.** Without D-09, schema generation, phrase
binding, result shaping, ontology validation, and identity documentation can each be
metadata-driven yet still drift as independent pipelines. One compiled object and pass
make those views projections of one semantic authority, let `cpg_ontology.rule_contract`
describe the executable operations, and give reproducibility/invalidation one proof
surface.

**Principles:** Advances `P1`–`P3`, `P13`, `P15`, `P17`, `P24`; maintains provider and
storage boundaries because the compiled object contains application-owned values only.

### 3.12 Library decisions

### LD-01 — DataFusion 55 extension-type registry: adopt

**Decision:** adopt
**Version basis:** DataFusion `=55.0.0` (`ExtensionTypeRegistry`,
`MemoryExtensionTypeRegistry`, `DFExtensionType`,
`SessionStateBuilder::with_extension_type_registry` — verified, pinned reference §4/S7.20–21).
**Displaces:** nothing; adds programmatic resolution and formatting behavior for
application-owned logical types. Arrow field validation and storage-seam casts remain
separate responsibilities; semantic enforcement is LD-08.
**Risk:** claiming unsupplied behavior or treating registration as policy. Mitigated by
separate generated Arrow `ExtensionType` impls, DataFusion `DFExtensionType` factories,
field-aware casts, and analyzer-rule tests.
**Validation:** compile+execute registration/formatting probe; serving oracle asserting
registry contents match `CompiledOntology` exactly; cast preservation proved at the
storage/result boundary, not attributed to the registry.

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

### LD-08 — DataFusion 55 analyzer rule for universal ID-domain enforcement: adopt

**Decision:** adopt one application-owned, idempotent `AnalyzerRule` installed through
the serving `SessionStateBuilder`; binder diagnostics delegate to its generated rule
model.
**Version basis:** DataFusion 55 invokes registered analyzer rules during logical-plan
optimization after resolution; rules may be reached more than once, so the rule must be
idempotent and preserve already-valid plans byte-for-byte at the logical contract level.
**Displaces:** binder-only/path-local domain checks and any direct extension-name policy
switches.
**Risk:** missed plan ingress, rule-order coupling, or domain erasure by casts. Mitigated
by constructing plans through every authorized ingress, checking all comparison/join/IN/
cast/set-operation shapes, and a generated unknown-domain fail-closed case.
**Validation:** plan-ingress census; same-domain pass corpus; cross-domain negative corpus;
double-analysis idempotence; binder/analyzer diagnostic equivalence.

**Ordering and functional dependencies** (dispositioned for completeness): `zorder`
remains layout-only; no declared sort orders or functional dependencies are exposed to
planning today, and none are added by this design — trigger for revisiting is
perf-evidence from the Gate-F workloads plus the PR-6 join-elimination outcome (dimension
PK `Constraints` are this design's first planning-visible uniqueness declaration).

### 3.13 Governance, state, failure (deltas)

- Plan-allowlist validation gains the `cpg_ontology` tables for daemon compilation,
  diagnostics, publication gates, and operator-side self-description. `information_schema`
  and direct ontology-table access stay off the agent path; agents retain the eight QRY
  forms and generated serving views. This design adds no query-language surface.
- Dimension batches are snapshot-scoped and die with the frozen catalog; no new caches or
  mutable state (`P23`).
- Ontology tables are written at workspace bootstrap and only when an input authority or
  compiled contract digest changes. Ordinary fact publications reuse the exact pinned
  ontology table versions; they do not rewrite identical dimensions. Candidate versions
  remain invisible until Stage 2b's single manifest activation.
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
  emits observations and reviewed state transactions bind Stage-2 detail decisions.
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
- **Stage 2b — one atomic ontology-plane activation (governed Contract-IR revision).**
  Build one candidate containing the complete twenty-table `cpg_ontology` plane,
  `enum_catalog` relocation [D-01], per-projection decoration, compiled relational rules,
  dimension FK/property/ontology validation, the PR-3-selected source-span physical
  lowering [D-03], first-class result-schema/result-field authorities describing the
  current response boundary, and the spec amendments (§5.4). Construction packets may publish
  non-active candidate Delta versions, but no packet advances the serving pointer or
  records owner acceptance independently. After catalog-only self-description,
  relational-closure, fault-injection, version-stability, and full compatibility gates
  pass over the complete candidate, one activation packet records owner acceptance and
  advances the manifest pointer exactly once. Stage 2a and Stage 2b are therefore two
  separately rollbackable releases; Stage 2b itself is never partially active. Delta
  reader/writer feature posture remains unchanged.
- **Stage 3 — governed result boundary, after Stage-2b activation.** D-04 revises the
  now-first-class result authorities to generated target schemas + typed ID-list columns,
  `ResultChecksumV2`, and KAT continuity. Old leases keep V1/current shapes while new
  leases use V2/target shapes by result-schema version. Determinism and conformance
  baselines move through confirm-gated `snapshots-accept` with reviewed diff; ontology
  result rows and runtime selection advance together in this bounded transaction.
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
| `fabric.rs` — `enum_catalog` population | **reshape** | generalized into generated builders for the twenty `cpg_ontology` tables |
| `fabric/snapshot_catalog.rs` — frozen catalog machinery, handle factory, stats posture | **preserve / extend** | reaffirmed on merit; gains the ontology schema and D-06 statistics |
| `fabric/overlay.rs` — overlay machinery | **preserve / reshape (statistics + tested pushdown claim)** | composition rule reaffirmed; D-06 |
| `fabric/serving.rs` — session, immutable providers, artifact accumulator, allowlist | **preserve / reshape** | session build (D-02), view decoration (D-01), allowlist additions |
| `fabric/serving.rs` — control capture | **reshape** | D-05 typed capture |
| `fabric/publication.rs`, `mutation.rs` | **preserve / extend** | gain dimension FK + ontology-conformance checks |
| `fabric/result_checksum.rs` | **preserve + version** | V2 added; V1 retirement condition named |
| `semantic_query.rs` — pipeline, `BoundPlanSpec`, `GraphOperatorPlan`, set algebra | **preserve / reshape** | typed-plan architecture retained; semantic operations and result bindings consume `CompiledOntology`; binder-only domain authority retires |
| `semantic_query.rs` — hand-written result schemas | **replace** | D-04 |
| `semantic_query.rs` — literal codes | **replace** | D-08 |
| `fact_ingest.rs` — 29 hand-written `*Row` structs | **replace (shape), preserve (logic)** | D-07 |
| `src/bin/codefabric_model/` drivers | **preserve + consolidate** | one registered `SchemaContractCompilation` pass emits `CompiledOntology`; specialized emitters consume it rather than reparsing authorities |
| `src/generated/*` | **regenerate** | never hand-edited |
| `query_service.rs` transport/authorization/artifacts | **preserve** | no query construction inside |
| `cpg_python`/`cpg_rust`/`cpg_derived` empty namespaces | **preserve** | language waves fill them — on the new shape |

No `encapsulate-temporarily` surfaces; every stage is a complete gated architecture.

### 5.4 Spec amendments carried by this design

The suite is revised in place with the Stage-2a/2b Contract-IR revisions (drafted with
them, accepted together — amendment 2 with 2a; amendments 1, 3, 4, 5 with 2b):

1. **`FAB §6.3`** — add `cpg_ontology` to the namespace set; relocate `enum_catalog`;
   define the complete normalized ontology/contract plane, bootstrap-or-bundle-change
   lifecycle, candidate invisibility, and single Stage-2b activation transaction.
2. **`FAB §7`** — `id16`/`hash32` rows become: canonical Arrow `FixedSizeBinary(16/32)`
   with per-domain extension types from the ID-domain registry; Delta storage mapping
   `BINARY` with the provider-seam reattachment contract (regularizes the already-shipped
   implementation and extends it per-domain).
3. **`FAB §8`** — dimension serving generalized from enum domains to all governed
   vocabularies and self-description contracts; N:M memberships normalize through
   `ontology_edge`; serving-view decoration covers `ontology:*` and raw-kind codes.
4. **`FAB §9`/`§65.4`** — structure classification per the `REP §17` criterion replaces
   the flat-only rule; SourceSpan's cohesive logical class is normative while the reviewed
   probe decision selects nested-struct or flat-constraint physical lowering;
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

- **TI-1:** `ontology-self-description-check` dynamically discovers all twenty relations
  from `registry_authority`/`table_contract`, resolves every code and contract family,
  and proves a new-domain fixture appears without oracle code changes;
  `ontology-relational-closure-check` compiles registry-declared anti-joins,
  relation-family/cardinality/owner rules, ID-domain/FK checks, and the property one-of
  rule through DataFusion. `ontology-dimension-check` remains the bundle-row/digest and
  decoration parity aggregate.
- **TI-2:** `id-domain-extension-check` (successor to `id16-extension-contract-check`) —
  per-domain extension preservation through provider schema, plan, result batch, and
  Parquet round-trip, scalar fields and list elements both; registry-contents oracle
  against the generated ID-domain registry; **universal analyzer-rule tests** — every
  authorized plan ingress reaches the rule; cross-domain comparison/join/IN/cast/set-op
  shapes are rejected with typed errors; same-domain equivalents and double-analysis pass;
  binder/analyzer diagnostics agree. Negative:
  zero-state for `codefabric.id16` including list children (ast-grep + `rg` + tier-1
  clean build after deletion).
- **TI-3:** `model-family-check schemas` extended to all four surfaces; golden schemas
  compared under the full fingerprint definition (field order, nullability, extension
  metadata, governed metadata).
- **TI-4:** `structure-classification-check` proves source spans retain
  `StructurallyOwnedCohesive` classification in both physical branches; nested branch
  proves Delta round-trip and child coherence, flat branch proves the compiled
  presence-coherence constraint; production pruning evidence selects only the lowering.
- **TI-5:** Schema Contract IR census proves every query-form/result-role has one
  `result_schema` and ordered `result_field` authority; golden per-form result schemas
  include metadata; REANNOTATE test — a computed
  projection's delivered field carries re-attached metadata; negative: ast-grep + `rg`
  zero-state for hand-written `Field::new` in result-shaping modules + tier-1 deletion
  proof; `ResultChecksumV2` KATs + V1 continuity assertions.
- **TI-6:** overlay-present statistics tests per mutation class (the §3.8 composition
  table, including `FullTableReplace → Exact(overlay)` and the never-unknown floor);
  structural proof that statistics-request handling remains declined; validated PK
  `Constraints` visibility plus explicit planner-consumed/application-validated/not-
  enforced classification; adversarial pushdown-truth comparison on the overlay path.
- **TI-7/TI-9:** `model-repro-check` proves one `SchemaContractCompilation` pass and
  byte-identical `CompiledOntology`; pass-registry census proves complete inputs/outputs/
  invalidation/diagnostics; downstream-consumer census forbids direct authority parsing;
  relational rule plans are compared with independently constructed fixture expectations.
  Stage-1 fingerprint-equality gate; dual-list reconciliation deleted with
  tier-1 proof; governance rules in `governance-scan` for literal codes *and* for match
  arms without phrase-registry IDs; **behavioral oracle for the certainty-set
  unification** — the same phrase produces the same registry-decided code set on the
  relational and graph paths (the §3.10 divergence, fixed and pinned).
- **TI-8:** the end-to-end self-description oracle starts only from a leased catalog and a
  delivered result artifact, discovers the ontology plane dynamically, and resolves the
  result's snapshot, publication, plan identity, every code/edge/authority, every semantic
  type and ID recipe, every table/column/result contract, and every phrase/rule binding.
  A seeded new-domain fixture proves recursive closure. This is Stage 2b's activation gate,
  not a milestone-level composition inferred from narrower checks.
- **Stage-2b atomicity:** fault injection before each candidate validation/acceptance/
  pointer step proves the prior active pointer remains; success advances exactly once;
  unchanged ontology inputs across fact publications reuse identical Delta versions.
- **Continuity:** `wave3-integration-check`, `query-determinism-check`,
  `semantic-query-conformance-check`, `query-legacy-zero-state-check`,
  `data-fabric-stack-compat` green at every stage boundary; perf differential against the
  Stage-0 anchor per stage; filter-expression diffs for the ~20 name-coupled recipes with
  any test move.
- **Packet oracles:** each work packet derives its four oracles
  (behavioral/structural/negative/operational) from the TI it advances (`P25`).

## 7. Probe suite (Stage 0; binds Stage 2 detail decisions)

Executable probes at the pinned versions produce **ephemeral observational evidence**
under `target/ontology-fabric-probes/<pin>/<environment-digest>/`; running them never
edits tracked fixtures, the design, or execution state. Each report records exact pins,
feature graph, production session configuration, workload/fixture digest, hardware/OS,
command, and raw plan/result evidence. An accountable reviewer then records the selected
named branch and evidence digest in the plan-state decision transaction. Downstream
packets consume that decision record and fail on pin/evidence/configuration drift; a test
does not "decide" architecture merely by observing behavior.

| Probe | Binds | Fallback if negative |
|---|---|---|
| PR-1 `ScalarValue::FixedSizeBinary` literal, IN-list, FSB hash-join/group-by | LD-04; D-02 predicate paths | storage-typed literal rewrite stands |
| PR-2 Full production delta-rs provider path with `DeltaScanConfig::with_schema`: projection, filter/pruning, statistics, physical plan, and emitted-batch FSB-over-Binary schemas | LD-03 provider-seam choice | current wrapper reattachment remains the default |
| PR-3a Span-struct type mapping: `Struct{Int64,Int64}` Delta `STRUCT` round-trip through the full production provider gate; FSB-adjacent struct casts | D-03 physical lowering only | flat constraint lowering; logical classification unchanged |
| PR-3b Span-struct pruning **under production session config** (Parquet filter pushdown disabled): file-skipping/statistics pruning on `source_span.start_byte` vs flat columns | D-03 physical lowering only | same fallback as PR-3a; logical classification unchanged |
| PR-4 Delta file-statistics exposure (column min/max) at the pin | D-06 scope | row-count/null composition only |
| PR-5 Parquet `ARROW:schema` metadata round-trip with per-domain extension names under current writer config | LD-02 validation detail | config adjusted (`skip_arrow_metadata=false` is already default) or metadata carried by manifest with round-trip test |
| PR-6 Unused-left-join elimination on decorated views at DF 55 (EXPLAIN over an undecorated projection; with and without already-validated dimension PK `Constraints`) | D-01 decoration breadth and constraint consumer classification | decoration declared narrowly; constraints remain truthful but not presumed consumed |
| PR-7 View-types-enabled execution through the pinned Delta provider (correctness first; then a recorded-environment, repeated statistical comparison against the WP01 anchor) | LD-07 | execution view types stay disabled; posture recorded as final for this pin |

## 8. Audit integration note

This v3 is the design-side integration companion to
`docs/reviews/plan_audit_codefabric_ontology_compiled_data_fabric_implementation_plan_v1_2026-08-27_v1.md`.
It resolves design-bearing findings F-001–F-005 and F-007–F-009 through TI-1–TI-9,
D-01–D-09, and LD-01/LD-08. Plan-graph, baseline, and production-provider proof findings
F-006/F-010/F-011 are integrated in implementation plan v2. The plan's Audit Integration
Log is the authoritative finding-by-finding disposition ledger.

## 9. Acceptance

**accepted — focused re-audit passed 2026-08-28.** The independent focused audit at
`docs/reviews/plan_audit_codefabric_ontology_compiled_data_fabric_implementation_plan_v2_2026-08-27_v1.md`
found no unresolved Blocker or Major integration defect. The owner's 2026-08-28
instruction to implement the complete plan accepts this dossier and authorizes
implementation plan v2 activation. Assumptions retained for execution (each labeled per
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
probe PR-3 failing *and* span-filtered query paths later becoming hot (reopens D-03's
physical lowering, never its logical classification).
