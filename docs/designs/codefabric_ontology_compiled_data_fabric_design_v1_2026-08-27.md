---
artifact: design-dossier
design_id: codefabric-ontology-compiled-data-fabric
version: v1
date: 2026-08-27
status: superseded
superseded_by: codefabric_ontology_compiled_data_fabric_design_v2_2026-08-27.md
supersession_reason: >
  v1 wrongly treated the upfront-design suite's past decisions as unmodifiable hard
  constraints. v2 re-derives the target on merit, reopening prior decisions where an
  improved approach maintains or enhances the targeted functionality, and carries the
  corresponding spec amendments in its transition.
baseline_commit: eebb958
working_tree_digest: 2fbc6c7061cb556c
primary_scope:
  - src/fabric.rs
  - src/fabric/
  - src/schema_registry.rs
  - src/semantic_query.rs
  - src/bin/codefabric_model/
  - src/generated/
  - contracts/schema/
  - contracts/registry/
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
governing_principles: docs/library_ref/full_data_fabric_design_principles.md
---

# CodeFabric ontology-compiled data fabric — target design v1

Sources of authority for this dossier: the data-fabric design constitution
(`full_data_fabric_design_principles.md`, cited `P1`–`P25` — per project direction these
take precedence over the holistic doctrine where both speak), the DataFusion 55 / Arrow 59
alignment manual (`align`, pattern IDs cited by family), the representative usage review
(`docs/reviews/representative_datafusion_arrow_relational_usage.md`, cited `REP §N`), and
the governing suite (`FAB`, `ONT`, `QRY`, `LIFE`, `SUITE`, `GEN` citations). The baseline
tree is intentionally dirty with pre-existing wave 8–12 work; the digest above identifies it.

## 1. Executive decision

**Deepen the existing ontology-compiled fabric in place; do not restructure it.** The
requested end-state — "ontology as semantic authority, Rust objects making it executable,
Arrow as its canonical typed realization, the DataFusion provider hierarchy exposing it as
an object hierarchy, LogicalPlan as the relational IR" (`REP §18`) — is already the
implemented architecture in first-generation form, and it is *mandated* by the governing
suite (`FAB §11` schema Contract IR + `SchemaContractCompilation`; `FAB §§12.6/91` frozen
per-snapshot catalogs; `QRY AC-G-46` typed PlanSpec, no SQL IR). What is missing is not the
architecture but its completion at eight specific seams, ranked by leverage:

| # | Gap (evidence) | Decision |
|---|---|---|
| G1 | The six ontology registries (`ENTITY_KIND`, `ENTITY_FAMILY`, `RELATION_KIND`, `RELATION_FAMILY`, `PROPERTY_KIND`, `FACT_KIND`) exist only as generated Rust consts (`src/generated/registries.rs:8059+`); `ontology:*`-typed columns get no in-catalog name resolution, and the registries' rich semantics (cardinality, symmetry, transitivity, allowed families) are unreachable from any query | **D-01** — materialize them as generated dimension tables and make the ontology executable in the catalog |
| G2 | `codefabric.id16` is enforced by five scattered application checks; the serving session has no engine-level extension-type registration; the FixedSizeBinary(16) contract is reconstituted per scan by a `ProjectionExec` cast (`src/fabric.rs:874-916`) | **D-02** — register the logical ID type with DataFusion 55's extension-type registry and centralize the contract seam |
| G3 | Query *result* Arrow schemas are hand-written `Field::new` lists (`src/semantic_query.rs:1550,1802,1932-2020`) that drop the extension type and all semantic metadata at exactly the agent-facing boundary | **D-03** — compile result schemas from the query-form contract |
| G4 | `cpg_control` is a second, untyped Arrow surface — `workspace_id` is plain `Binary` with no metadata (`src/schema_registry.rs:796-816`) while the same concept in `cpg_base` is FSB(16)+extension | **D-04** — one logical type system across the whole catalog |
| G5 | Statistics collapse to `unknown` for any table with a live overlay (`src/fabric/overlay.rs:780`); primary keys never reach DataFusion `Constraints`; no column min/max anywhere | **D-05** — truthful statistics deepening |
| G6 | Two generated column lists for the same 39 tables are reconciled at runtime (`src/schema_registry.rs:561-583`); 29 hand-written row DTOs sit beside generated encoders (`src/fact_ingest.rs:65-578`) | **D-06** — one generated column authority; generate the row shapes |
| G7 | Certainty codes appear as magic integers inside the typed compiler (`src/semantic_query.rs:1339-1352`) although `EVIDENCE_CERTAINTY` is a live registry domain | **D-07** — registry-complete compilation, zero literal codes |
| G8 | Arrow nesting is unused (`StringMap`: zero columns) but undecided | **D-08** — deliberate flatness, recorded trigger for revisiting |

A literal transcription of the representative document's layout (its own namespace set,
nested provenance structs, per-domain extension names, FixedSizeBinary storage) was
developed as the clean-sheet alternative and **rejected** (§4): it violates hard suite
constraints that are themselves well-motivated, and the Delta kernel at the pinned revision
cannot store fixed-size binary at all. Three of its ideas are adopted into the selected
design: queryable ontology registries (D-01), engine-registered logical ID types (D-02),
and per-domain ID identity — carried as contractual field metadata rather than as distinct
extension names (D-02).

## 2. Constraints and target invariants

### 2.1 Hard constraints (violating any reopens the design)

Numbered constraints below are established by the spec-evidence review of this session;
each is cited to its normative home.

1. Every Arrow/Delta schema is emitted by `SchemaContractCompilation` from the closed
   schema Contract IR; hand-authored schema authorities and hand-edited generated files are
   prohibited (`FAB §11`, `SUITE AC-G-05`). This is also `P3` (one authority) and `SCH-01`.
2. Catalog namespaces are exactly `cpg_control`, `cpg_base`, `cpg_python`, `cpg_rust`,
   `cpg_derived`, `cpg_serving` (`FAB §6.3`); the catalog *name* is design freedom (the
   implementation uses `codefabric`, `src/fabric/serving.rs:48-53`).
3. One immutable frozen catalog per `ServingSnapshot`, built off-path, frozen, activated by
   one atomic pointer swap; providers die with their snapshot (`FAB §12.6`, `§91`,
   `LIFE §§100.3/106/157.1`; `P11`).
4. Universal `entity`/`relation`/`property_fact`/`fact_evidence` tables; per-relation-kind
   tables, EAV, JSON blobs prohibited as canonical persistence (`FAB §5/§5.1`); property
   values are the tagged typed column set (`FAB §16.1`).
5. Flat `FAB §9` provenance columns are mandatory on canonical rows; nesting only for
   bounded cohesive payloads (`FAB §65.4`).
6. Agents never emit SQL; requests compile `PlanSpec → BoundPlanSpec → LogicalPlan /
   GraphOperatorPlan`; custom logical/physical nodes, UDFs, and planner hooks on the query
   path require an accepted `ExtensionDecisionRecord` (`QRY AC-G-46`).
7. Enum and ontology codes come only from the generated registry; no independent
   assignment (`SUITE AC-G-06`, `ONT §62.10`); canonical kinds and provider-raw kinds are
   separate namespaces (`ONT AC-G-70`).
8. Metadata is advisory unless consumed by explicit validation code; every metadata field
   is classified into the six-way taxonomy with a named consumer; DataFusion extension
   registration may be used only for behavior the pinned implementation actually supplies
   (`FAB AC-G-20`, `P21`).
9. Clean-rebuild equality requires the exact schema fingerprint including field order,
   nullability, extension metadata, and governed schema metadata (`SUITE AC-G-79`).
10. Schema evolution is `EXACT_PIN`: any Contract-IR change is a governed release event —
    versioned IR revision, migration probes, manifest-pinned candidate migration, recorded
    owner acceptance (`contracts/generated/model/schema/schema-evolution-policy.json`,
    `FAB §103`, `SUITE AC-G-83`).
11. Version pins are immovable inside this design: DataFusion `=55.0.0`, Arrow/Parquet
    `=59.2.0`, `object_store =0.13.2`, deltalake git `43a0cf10` (`FAB §2.1`).
12. The Delta kernel type system at the pinned revision has `BINARY` only — no fixed-size
    binary (deltalake reference `§4.19`). FixedSizeBinary(16) therefore cannot be the
    storage type; it is the canonical Arrow *contract* type reattached at the provider
    boundary. (Observed fact, library reference; the current implementation already does
    this, `src/fabric.rs:1203-1218`.)

### 2.2 Target invariants (what must become true)

- **TI-1 (executable ontology).** For every column whose `semantic_type` is
  `ontology:<registry>`, the owning registry is a queryable dimension table in the frozen
  catalog, the serving view joins it to a `<field>_name` column, and a generated
  referential check proves zero fact rows reference codes absent from the registry —
  the `REP §13` validation query as a named gate, not an illustration. (`P2`, `P12`, `P25`)
- **TI-2 (one logical type system).** Every column in every schema the catalog serves —
  `cpg_base`, `cpg_control`, `cpg_serving`, and query *results* — is lowered from the same
  generated logical-type vocabulary with the same physical type, extension type, and
  metadata rules. The same concept is never typed two ways inside one catalog. (`P3`, `P7`,
  `SCH-01/02`)
- **TI-3 (engine-known ID identity).** The serving `SessionState` carries an extension-type
  registry in which `codefabric.id16` is registered; the ID's logical identity is
  engine-resolvable wherever DataFusion 55 actually consults the registry (field-aware
  casts, value formatting, provider/UDF resolution), and application-side enforcement
  remains the authority (`AC-G-20`). Each ID column additionally carries a contractual
  `id_domain` metadata key naming its ID domain. (`P8`, `P22`, `SCH-06`, `INT-09`)
- **TI-4 (contract-complete result boundary).** The Arrow schema of every query-form
  response is generated from the query-form contract, carries extension types and semantic
  metadata, and computed projections re-annotate metadata explicitly (the DataFusion 55
  metadata drop-map is compensated by construction, not by convention). (`P12`, `MOD-08`,
  `EXP-11`)
- **TI-5 (truthful, non-degenerate planning facts).** Statistics and constraints exposed to
  DataFusion are exactly as strong as what is known: exact base row counts survive overlay
  composition as `Inexact` rather than collapsing to unknown; primary keys are surfaced as
  `Constraints`; every pushdown claim remains per-filter truthful. Unknown stays preferable
  to falsely known. (`P15`, `P20`, `CAT-05/07`)
- **TI-6 (single generation seam).** One generated column list per table; row shapes for
  ingest are generated beside their encoders; the runtime reconciliation of two generated
  lists is deleted. (`P3`, anti-pattern "multiple authorities")
- **TI-7 (registry-complete compiler).** The semantic compiler contains zero literal
  ontology/enum code values; every code reaches the plan through generated constants or
  dimension lookups. (`P1`, `MOD-02`)
- **TI-8 (no silent architecture).** All of the above land without moving the pinned
  dependency baseline, without new Cargo roots, and without unrecorded extension-path
  escalation (`QRY AC-G-46` gate preserved).

### 2.3 Explicitly out of scope

Python adapter changes beyond regenerated wire artifacts (presentation-only invariant);
Delta write-protocol or publication-choreography redesign; the graph-projection runtime
(roadmap `W13`) and query-language expansion (`W15/16`) — this design prepares their
substrate but does not implement them; absolute performance SLOs (Gate F, `W19`).

## 3. Target architecture

### 3.1 The compilation chain (unchanged spine, completed reach)

```text
contracts/registry/ontology-*-registry.yaml   contracts/schema/schema-contract-ir.json
        │  (ONT AC-G-70/71 authorities)               │  (FAB §11 authority)
        └──────────────┬───────────────────────────────┘
                       ▼  src/bin/codefabric_model (SchemaContractCompilation)
        one generated column/table authority            [D-06]
        + ontology dimension table specs                [D-01]
        + operational logical types                     [D-04]
        + query-form result schemas                     [D-03]
        + registry code constants for the compiler      [D-07]
                       ▼  src/schema_registry.rs (single lowering)
        Arrow Schema / Field  (FSB16 + codefabric.id16 + cf metadata)
                       ▼  src/fabric/snapshot_catalog.rs (per-snapshot, frozen)
        Delta pinned-version providers → overlay wrap → scope wrap → stats wrap
                       ▼  src/fabric/serving.rs (immutable catalog `codefabric`)
        cpg_base │ cpg_control │ cpg_serving │ (empty: cpg_python cpg_rust cpg_derived)
                       ▼  src/semantic_query.rs (typed Expr / LogicalPlanBuilder only)
        BoundPlanSpec → LogicalPlan / GraphOperatorPlan → ExecutionPlan → RecordBatch
```

Dependency direction is one-way throughout: ontology → contract IR → generated Rust →
Arrow → providers → plans → batches. Nothing downstream re-declares an upstream shape
(`REP §3` "never reverse-engineer ontology meaning from Arrow schemas"; `P3`).

### 3.2 D-01 — Ontology dimension plane

Six new generated tables in `cpg_base`, family `bundle`,
`MaterializationRole::BundleDimension` + `OverlayMutationPolicy::BaseImmutable` (the slot
`FAB §8` / `AC-G-21` pre-shapes), table codes 12–17:

| Table | Registry authority | Beyond code/name |
|---|---|---|
| `ontology_entity_kind` (12) | `ontology-entity-registry.yaml` | family code, language applicability, query visibility |
| `ontology_entity_family` (13) | same | — |
| `ontology_relation_kind` (14) | `ontology-relation-registry.yaml` | family code, allowed subject/object family codes (`IdList`-style Int32 list), cardinality, `symmetric`, `transitive`, `self_edge_policy`, owner-selection rule code, query visibility |
| `ontology_relation_family` (15) | same | — |
| `ontology_property_kind` (16) | `ontology-property-registry.yaml` | value-kind code, cardinality, storage mapping code |
| `ontology_fact_kind` (17) | `ontology-fact-registry.yaml` | fact form code |

Every table carries `version` and `canonical_digest` columns mirroring the registry bundle
digest, exactly as `enum_catalog` (code 11) does today. Column sets are declared in the
schema Contract IR like every other table (constraint 1); the model compiler gains a small
driver step that renders registry YAML rows into the generated bundle-batch builders beside
the existing `enum_catalog` population (`src/fabric.rs:1136-1172` pattern).

Serving decoration: `serving_view_plan` (`src/fabric/serving.rs:1240-1281`) currently
decorates only `enum:*` semantic types. It is extended to decorate `ontology:*` types by
joining the owning dimension table (the mapping is already generated:
`GENERATED_SEMANTIC_TYPE_BINDINGS`, `src/generated/table_specs.rs:530-580`). Result:
`relations.relation_kind_code` gains `relation_kind_name` through the same left-join
mechanism `language_name` uses today. Because view shapes are governed by generated
`serving_projections` records (constraint: runtime owns no table/view tuples, `FAB §11`),
the added name columns enter through a Contract-IR revision, not runtime logic.

Executable ontology (TI-1): the publication referential-integrity check
(`src/fabric/publication.rs`, `just publication-referential-integrity-check`) gains
generated FK contracts from fact code columns to dimension tables, so `REP §13`'s
anti-join validation is a standing gate. The relation registry's semantic columns
(`symmetric`, `transitive`, allowed families) make ontology-conformance queries expressible
in the fabric itself — e.g. a derived-lane check that no `relation` row violates its kind's
allowed-family constraint — without new Rust: this is `P2` (models executable) delivered
relationally.

The six dimension tables are also mirrored to the `enum_catalog`-style Delta table only if
the existing `enum_catalog` Delta mirror decision applies to them (`FAB §8` `MAY`); the
in-memory dimension batches inside the frozen catalog are the serving authority either way,
and they are constructed from the same generated builders (no second authority; `P3`).

**Principles:** Advances `P2`, `P12`, `P25`; Maintains `P3` (registry YAML stays the
authority; tables are compiled projections carrying its digest).

### 3.3 D-02 — Engine-registered logical ID types

Three moves, none of which changes stored bytes:

1. **Session registration.** The serving session construction
   (`src/fabric/serving.rs:496`, currently `SessionContext::new_with_config_rt`) is
   reshaped to build through `SessionStateBuilder` and install a
   `MemoryExtensionTypeRegistry` containing a `DFExtensionType` registration for
   `codefabric.id16` (storage-resolved to `FixedSizeBinary(16)`). Verified DataFusion 55
   surface: `datafusion_expr::registry::ExtensionTypeRegistry`,
   `MemoryExtensionTypeRegistry`, `SessionStateBuilder::with_extension_type_registry`,
   `Session::extension_type_registry` (df ref §4, S7.20). What this buys at the pin — and
   the *only* behaviors claimed, per `AC-G-20` — is: field-aware cast preservation
   (`Cast { field: FieldRef }` carries extension metadata through planning, S7.21),
   extension-aware value formatting, and programmatic resolution for providers and any
   future registered functions. Join/equality semantics remain storage-typed; application
   validation remains the enforcement authority. The registration's named consumers are
   recorded in the metadata classification dictionary (classification:
   `planner_consumed`).
2. **One contract seam.** `Id16ContractProvider` (`src/fabric.rs:920-1010`) remains the
   single place where Binary storage is re-presented as FSB(16)+extension; the five
   scattered `has_valid_extension_type` checks elsewhere become calls into one shared
   validation helper owned by `schema_registry`, so the enforcement surface is one function
   with one test suite rather than five idioms.
3. **ID-domain identity.** A new generated contractual field-metadata key
   `com.codefabric.cpg.id_domain` (values: `entity | fact | workspace | context | owner |
   type | file | publication | ...` — the generated FK/semantic bindings already know the
   domain for every ID column) delivers `REP §5`'s "many ID domains over one physical
   representation" without minting per-domain extension *names*. Distinct extension names
   (`codefabric.entity_id`, …) were considered and rejected: extension identity
   participates in the `AC-G-79` schema fingerprint and the `AC-G-20` contract, so renaming
   is an incompatible schema change purchasing no engine behavior at the pin. Metadata is
   additive and classified `contractual` with named consumers (result-boundary validation,
   D-03; diagnostic `arrow_field()` introspection in SQL).

**Storage posture (LD-03).** Delta storage stays `BINARY` — the kernel at `43a0cf10` has no
fixed-size type (constraint 12), so the current whitelisted `(FixedSizeBinary(16), Binary)`
compatibility and per-scan reattachment is the *correct* architecture, not debt. Two cost
reducers are probed, not assumed: `DeltaScanConfig::with_schema` (compatible-schema scan;
may absorb the cast into the scan) and pushing the reattachment `ProjectionExec` below
other projections. If both fail, the existing cast stands; it is visible in `EXPLAIN` and
already proven by `wp03` serving-equivalence oracles.

**Principles:** Advances `P8`, `P22`; Maintains `P20` (no claimed behavior beyond the
pin's documented surface); Risk — mitigated for `P21` (registration without consumers would
be metadata theater; consumers are named and tested).

### 3.4 D-03 — Generated result-schema plane

The model compiler's query-form driver (`contracts/query/query-form-contract.json` →
`src/generated/model_query_forms.rs`) is extended to emit, per query form and result role,
the response Arrow schema: field list, logical types (lowered by the same
`schema_registry` rules — FSB16+extension for IDs, `id_domain`, `semantic_type`), and the
deterministic field order the wire contract already fixes. `semantic_query.rs`'s three
hand-written `Field::new` sites are replaced by lookups into the generated schemas; the
`ORDER BY`/projection code re-annotates computed columns with `alias_with_metadata`
(DataFusion 55 drop-map: computed projections and aggregates carry empty metadata unless
re-annotated — df ref S7.5/S7.11; this is a REANNOTATE obligation discharged by
construction at the one place results are shaped).

Consequences owned explicitly:

- `ResultChecksumV1` covers `canonical_schema`; adding metadata/extension types to result
  schemas changes checksum inputs. The checksum is version-tagged
  (`RESULT_CHECKSUM_VERSION`); this change mints `ResultChecksumV2` computed over the
  richer schema, keeps V1 verifiable for the released KATs
  (`arrow58/59_codefabric_batch_checksum_kat`), and records the transition in the upgrade
  compatibility tests — never silently re-baselining a KAT (evidence-policy §7).
- The Python adapter consumes JSON/protobuf, not Arrow (verified: zero `pyarrow` under
  `codefabric-cpg-mcp/`), so the wire surface is unchanged; only the daemon-side Arrow
  boundary gains typing.

**Principles:** Advances `P12` (schema as executable contract at the last boundary),
`MOD-08`, `EXP-11`; Maintains `P6` (result shape is semantic, not physical).

### 3.5 D-04 — Typed control plane

The 27 operational projections (`GeneratedOperationalColumn` → Arrow at
`src/schema_registry.rs:796-816`) currently lower to `Int64/Float64/Utf8/Binary` only. The
Contract IR gains logical types for operational columns (at minimum: every `*_id` that is
an id16 becomes `LogicalType::Id16`; timestamps become `TimestampUtc`), and
`build_operational` lowers through the same `physical_type`/`field` path as `cpg_base`
(same extension type, same metadata keys). The SQLite capture layer
(`src/fabric/serving.rs:1283-1400`) converts blob→FSB(16) at capture time, where the
16-byte invariant is already enforced by the operational store's writers. `cpg_control`
stays MemTable-backed and snapshot-captured; only its typing unifies (TI-2). Operational
SQLite DDL is untouched — SQLite has no fixed-width blob type; the logical type governs
only the Arrow surface.

**Principles:** Advances `P7` (one canonical representation across the catalog);
eliminates the same-concept-two-types defect (`P3` anti-pattern).

### 3.6 D-05 — Truthful statistics deepening

- **Overlay-aware statistics.** `OverlayEffectiveProvider` currently returns
  `Statistics::new_unknown` whenever an overlay exists. Replace with composition: base
  provider statistics (exact pinned-version row counts from the publication manifest,
  already computed in `authenticated_statistics`) merged with exact overlay batch counts,
  reported as `Precision::Inexact` (replacement semantics make exact counts unknowable
  without executing the anti-join — Inexact is the truthful maximum, `P20`).
- **Constraints.** Generated primary keys are surfaced as DataFusion
  `Constraints` on the wrapped providers (today only Delta CHECK constraints pass
  through). `TableProvider::constraints` semantics verified at the pin (df ref §18).
- **`ScanArgs` posture.** Providers answer `statistics_requests` only with cheap,
  already-known values (manifest row counts, null counts for non-null columns) and
  explicitly ignore the rest — the existing `STATISTICS_REQUEST_POSTURE` discipline in
  `snapshot_catalog.rs` extends to the overlay wrapper instead of being bypassed by it.
- **Pushdown truth audit.** `OverlayEffectiveProvider::supports_filters_pushdown` claims
  `Exact` for all filters; this is truthful only because filters are re-applied inside the
  effective `LogicalPlan` it builds. That justification becomes a test (adversarial filter
  through the overlay path, compared against unfiltered + engine-filtered execution) so
  the claim is proven, not remembered (`CAT-05`, `TST-03`).
- Column min/max statistics from Delta file stats are **deferred**: verify what the pinned
  delta-rs exposes before promising; recorded as a plan-preflight probe, not a commitment.

**Principles:** Advances `P15`, `P20`, `P24`; Maintains `P23` (statistics remain derived,
snapshot-scoped, never authoritative).

### 3.7 D-06 — One generated column authority

The schema driver emits one merged generated column shape carrying the union of what
`MODEL_TABLES` and `GENERATED_TABLE_SPECS[*].columns` carry today (type, nullability,
semantic type, FK, `hidden_operational`, field id); `schema_registry::model_field`'s
by-name reconciliation of the two lists (`src/schema_registry.rs:561-583`) is deleted. The
29 hand-written `*Row` structs in `fact_ingest.rs` are displaced by generated row-shape
definitions emitted beside `fact_row_encoders.rs` (same include mechanism); hand-written
ingest logic keeps constructing them, but their field sets can no longer drift from the
schema. Generated-file bytes change; **schema bytes must not**: the `AC-G-79` fingerprint
comparator is the gate proving this stage is a pure re-plumbing.

**Principles:** Advances `P3`; reduces the drift surface `P1` warns about.

### 3.8 D-07 — Registry-complete typed compiler

The registry-CBEF driver already emits typed code constants; the semantic compiler's
condition builders consume generated constants for certainty/resolution/directness (and
any other domain it filters on) instead of `ScalarValue::Int16(Some(10))` literals. A
governance rule (`rules/`, ast-grep, with `rule-tests/` fixtures) bans bare integer
literals inside the compiler's predicate-construction modules thereafter — the invariant
is asserted once and promoted to a gate (evidence-policy §0).

**Principles:** Advances `P1`, `P2`; closes the "hidden semantic logic" anti-pattern.

### 3.9 D-08 — Deliberate flatness

Canonical rows stay flat: `FAB §9` mandates flat provenance columns and `FAB §65.4`
reserves nesting for bounded cohesive payloads; the representative document's nested
`provenance`/`source_span` structs (`REP §6`) are therefore adopted **only** as an optional
serving-view projection concept, and even that is deferred — no current consumer requests
it, the QRY response contract owns agent-facing shape, and struct-bearing result schemas
would churn the checksum surface for zero present value. Recorded trigger for revisiting:
a QRY revision that defines a struct-shaped response field, or a measured join-cost problem
on the `fact_evidence` path. The `REP §17` decision table is retained as the criterion
(independent identity/provenance/cardinality/joins → relational; structurally owned and
consumed together → nested).

**Principles:** Maintains `P6`/`P8`; avoids speculative structure (`P14` — highest
abstraction that preserves semantics, no lower).

### 3.10 Library decisions

### LD-01 — DataFusion 55 extension-type registry: adopt

**Decision:** adopt (wrap)
**Version basis:** DataFusion `=55.0.0` — `ExtensionTypeRegistry` /
`MemoryExtensionTypeRegistry` / `DFExtensionType` / `SessionStateBuilder::
with_extension_type_registry` verified in the pinned reference (§4, S7.20–S7.21).
**Displaces:** nothing removed; adds engine-aware formatting and field-aware cast
preservation over the existing application-enforced `Id16Extension`.
**Risk:** claiming behavior the pin does not supply (`AC-G-20`). Mitigated: claimed
consumers limited to the three verified behaviors; classification `planner_consumed` with
named consumers.
**Validation:** compile+execute probe registering the type and asserting formatter and
cast-path behavior; serving oracle asserting the registry is installed in the session
state.

### LD-02 — Arrow 59 `ExtensionType` (`codefabric.id16`): retain-current

**Decision:** retain-current
**Version basis:** arrow-schema `=59.2.0` extension module (verified; `arrow.` namespace
reserved, custom namespaced names supported).
**Displaces:** rejected alternative: per-domain extension names — incompatible fingerprint
change with no engine payoff at the pin; `id_domain` metadata carries the distinction.
**Risk:** unknown-consumer degradation. Mitigated: `INT-09` round-trip tests (known and
unknown consumer) already partially exist (`id16-extension-contract-check`); extended to
the result boundary.
**Validation:** existing `just id16-extension-contract-check` plus D-03 result-schema
tests.

### LD-03 — deltalake `43a0cf10` Binary ID storage: retain-current

**Decision:** retain-current
**Version basis:** deltalake git `43a0cf10` — kernel type catalog has `BINARY` only
(reference §4.19); `TableProviderBuilder` with supplied snapshot = no-I/O provider
construction (§6.5–6.6), as the frozen catalog requires.
**Displaces:** the clean-sheet FSB-storage idea; not achievable at the pin.
**Risk:** per-scan reattachment cost. Mitigated: probe `DeltaScanConfig::with_schema` as a
cast absorber; cost already covered by serving-equivalence oracles; revisit trigger = Delta
upgrade adding fixed-width types.
**Validation:** `FAB §11.1` round-trip gate (existing) continues to pass unchanged.

### LD-04 — FixedSizeBinary literals and join keys: assumption to validate

**Decision:** adopt-if-proven; fallback retained
**Version basis:** `ScalarValue::FixedSizeBinary` is absent from the pinned reference;
hash-join/group-by acceptance of FSB(16) keys (including inside future struct fields) is
undocumented.
**Displaces:** if proven, the filter-literal `FixedSizeBinary→Binary` rewrite
(`src/fabric.rs:905-916`) can narrow; if not, it stands.
**Risk:** building D-03/D-07 predicate paths on an unverified literal type. Mitigated: the
current Binary-rewrite path remains the default until the probe passes.
**Validation:** plan-preflight compile+execute probe at the pin (typed point-lookup,
IN-list, and a two-table FSB join), recorded with the plan.

### LD-05 — Recursive CTEs for N-hop traversal: reject

**Decision:** reject
**Version basis:** DataFusion 55 has `RecursiveQuery` plan/operator machinery, but
SQL-level authoring is unverified at the pin, and `QRY AC-G-46` routes traversal through
inspectable `GraphOperatorPlan` nodes regardless.
**Displaces:** nothing; N-hop stays in the `GraphOperatorPlan` / derived-lane (petgraph)
path, whose results return as ordinary relation facts (`REP §19`'s closing rule).
**Risk:** none new.
**Validation:** n/a (rejection); the `query-legacy-zero-state-check` continues to prove no
SQL path exists.

### LD-06 — MemTable for dimension/control tables: retain-current

**Decision:** retain-current
**Version basis:** `MemTable::try_new` verified at the pin; declared sort order and
statistics undocumented — not load-bearing for dimension tables (tiny, joined by code).
**Displaces:** n/a. **Risk:** none material. **Validation:** existing catalog oracles.

### 3.11 Governance, state, and failure surfaces (deltas only)

- **Governance** stays at the owning boundaries (`P13`): scope predicates below user
  predicates (`FAB §91`, unchanged), plan allowlist validation before physical planning
  (unchanged, and it must now allowlist the six dimension tables), `information_schema`
  remains disabled on the agent path (default-off verified at the pin; agents get catalog
  knowledge only through QRY forms).
- **State ownership** is unchanged: dimension batches are snapshot-scoped, constructed from
  bundle-pinned registries, and die with the frozen catalog; no new caches, no new
  mutable state (`P23`).
- **Failure taxonomy** is unchanged; new failure points added by this design surface
  through existing channels: dimension-referential violations fail publication (like FK
  violations today), result-schema mismatches fail at the existing batch-validation
  boundary with the existing error classes.

## 4. Alternatives and clean-sheet challenge

### Alternative A — contract-deepening in place (selected)

Everything in §3. Change surface is concentrated in the model compiler, `schema_registry`,
the two catalog files, and the semantic compiler's boundary code; the architecture,
topology, and doctrine of the current fabric are preserved because they already satisfy
the constitution.

### Alternative B — literal restructure per the representative document (clean-sheet)

Rebuild the catalog as `cpg.{ontology,facts,source,semantic,rust,derived}` with nested
provenance structs on fact rows, per-domain extension names, FixedSizeBinary storage, and
a new `codefabric-data-model` unit.

Rejected on evidence, not taste:

- Namespace layout violates `FAB §6.3` (six namespaces, fixed, different); the suite's
  layout also encodes lifecycle semantics (`cpg_serving` staging, empty-immutable
  language namespaces) the representative sketch lacks.
- Nested provenance on canonical rows violates `FAB §9`/`§65.4`, and those rules exist for
  pushdown/join reasons the representative document itself endorses (`REP §7`, `§17`).
- FixedSizeBinary storage is impossible at the pinned Delta kernel (constraint 12).
- Per-domain extension names are an incompatible fingerprint change with no engine payoff
  at the pin (§3.3).
- It reopens the repository's most-proved, lowest-churn layer (~111 colocated oracles,
  ~20 name-coupled gates) for outcomes achievable additively.

**Clean-sheet answer** (Phase-7 question): if the current implementation did not exist,
the preferred design would still be the suite-mandated architecture — which is what the
current implementation is — with the D-01…D-07 completions built in from the start. The
current architecture is therefore preserved on merit, not incumbency. Three Alternative-B
ideas are adopted where spec-compatible: queryable ontology registries (D-01),
engine-registered extension types (D-02), ID-domain identity as metadata (D-02).

### Alternative C — SQL-introspection instead of dimension tables (considered, rejected)

Serving ontology knowledge via `information_schema` + `arrow_field()` SQL functions instead
of dimension tables: rejected — it exposes engine introspection rather than ontology
semantics, cannot carry the registries' relational metadata (cardinality, allowed
families), conflicts with keeping `information_schema` off the agent path, and makes
`REP §13`-style validation queries impossible as standing relational gates.

## 5. Transition, cutover, and legacy disposition

### 5.1 Position in the program

The active waves 8–12 plan (state: `executing`, `WP08` current) forbids fabric-baseline
movement and QRY-contract reopening in its own non-goals; landing this design mid-plan is
a plan-level replan, not a packet. **Recommendation: execute as its own implementation plan
at the W12/W13 seam** — after wave-12 reconciliation/storage-integrity packets close and
before `W13` (graph-projection runtime) and `W15/16` (query compiler expansion) consume the
fabric surface this design improves. Stage 0 below is safe to run earlier because it
changes no schema and no behavior. If program priorities force earlier landing, that is a
replan decision for the waves 8–12 plan owner, outside this dossier.

### 5.2 Stages

Each schema-affecting stage is a governed `EXACT_PIN` release event (constraint 10):
versioned Contract-IR revision → model regeneration → migration probes → owner acceptance.
No stage introduces dual authority; each is atomic at its boundary.

- **Stage 0 — evidence floor (no schema, no behavior).** Capture the perf baseline
  (`just data-fabric-upgrade-bench` differential anchor at the pre-change commit — the ops
  review is explicit that this evidence is unrecoverable later); promote a protective
  subset of colocated fabric oracles (serving equivalence, checksum KATs, catalog freeze,
  overlay composition) into `tests/integration/` so the oracles survive edits to the files
  they currently live in; ship the gate filter-expression diff policy (any packet renaming
  tests must diff the `nextest` filter expressions of affected recipes).
- **Stage 1 — pure re-plumbing (generated bytes change, schema bytes do not).** D-06
  single column authority; D-07 registry constants + governance rule; D-02 moves 1–2
  (session registry, one validation seam). Gate: `AC-G-79` fingerprint equality against
  the pre-stage snapshot, full `wave3-integration-check` + serving/checksum oracles.
- **Stage 2 — additive Contract-IR revision.** D-01 dimension tables (new codes 12–17) +
  serving-view `ontology:*` decoration + dimension FK contracts; D-02 move 3
  (`id_domain` metadata — additive to field metadata, which *does* change schema
  fingerprints: this stage is the fingerprint-moving release and is treated as such).
  Migration: none for stored data (new tables are bundle-loaded; existing table bytes
  untouched; field-metadata addition is a schema-bundle revision with `AC-G-83`
  choreography but no data rewrite).
- **Stage 3 — result boundary.** D-03 generated result schemas + `ResultChecksumV2` +
  KAT continuity tests. Wire-visible only to Arrow-side consumers (none external today).
- **Stage 4 — control plane.** D-04 operational logical types (in-memory Arrow surface
  only; no Delta migration; SQLite untouched).
- **Stage 5 — planning facts.** D-05 statistics/constraints deepening + pushdown-truth
  tests (no schema change).

Rollback per stage: stages 1/4/5 revert by commit (no durable-state coupling); stages 2/3
follow the schema-evolution policy's candidate-migration + owner-acceptance route, with
the prior schema bundle remaining activatable until acceptance is recorded. The Delta
rollback window noted in the operational handoff stays intact: no stage enables kernel
features (column mapping, type widening, deletion vectors remain off).

### 5.3 Legacy disposition matrix

Inventory generated by `ast-grep outline src/fabric src/fabric.rs src/schema_registry.rs
--items exports` and `ast-grep outline src/semantic_query.rs --items exports` (ast-grep
0.45.1, this session); dispositions cover every exported surface group:

| Surface | Disposition | Justification |
|---|---|---|
| `schema_registry.rs` — `Id16Extension`, `TableSpec`, policy enums, metadata dictionary, scope/projection specs | **preserve** | conformant authority lowering; D-02 adds one shared validation helper |
| `schema_registry.rs` — `model_field` dual-list reconciliation (`:561-583`) | **delete** | displaced by D-06 single generated shape |
| `schema_registry.rs` — `build_operational` untyped lowering (`:796-816`) | **reshape** | D-04 routes through the common logical-type lowering |
| `fabric.rs` — `exact_provider`, `Id16ContractProvider`, `validate_delta_schema`, `delta_schema_digest`, workspace bootstrap | **preserve** | the correct Delta seam under constraint 12; probes may narrow the cast |
| `fabric.rs` — `enum_catalog` population (`:1136-1172`) | **reshape** | generalized into generated dimension-batch builders serving codes 11–17 |
| `fabric/snapshot_catalog.rs` — `SnapshotProviderCatalog`, `Frozen*Provider`, handle factory, stats posture | **preserve** | mandated frozen-catalog shape; D-05 extends the stats wrapper |
| `fabric/overlay.rs` — `ConsolidatedOverlay`, `OverlayEffectiveProvider`, rebase machinery | **preserve / reshape (statistics only)** | composition rule is spec-mandated; only `Statistics::new_unknown` and the untested `Exact` claim change under D-05 |
| `fabric/serving.rs` — `ServingQuerySession`, `Immutable*Provider`, artifact accumulator, plan allowlist | **preserve / reshape (session build + view decoration)** | D-02 session registry at `:496`; D-01 `ontology:*` decoration at `:1240-1281`; allowlist gains dimension tables |
| `fabric/serving.rs` — control-table capture (`:1283-1400`) | **reshape** | D-04 typed capture |
| `fabric/publication.rs`, `fabric/mutation.rs` | **preserve** | out of scope; D-01 adds generated FK contracts consumed by the existing integrity check |
| `fabric/result_checksum.rs` — `ResultChecksumV1` | **preserve + extend** | V1 stays verifiable for released KATs; V2 added by D-03 |
| `semantic_query.rs` — parse/type/bind pipeline, `BoundPlanSpec`, `GraphOperatorPlan`, set algebra | **preserve** | conformant typed compiler |
| `semantic_query.rs` — hand-written result-schema sites (`:1550,1802,1932-2020`) | **replace** | D-03 generated schemas |
| `semantic_query.rs` — literal certainty codes (`:1339-1352`) | **replace** | D-07 |
| `fact_ingest.rs` — 29 hand-written `*Row` structs | **replace (shape), preserve (logic)** | D-06 generates the shapes; ingest logic unchanged |
| `src/bin/codefabric_model/` drivers | **preserve + extend** | gains dimension-table, result-schema, operational-type, and merged-column emission |
| `src/generated/*` (all) | **regenerate** | never hand-edited (AC-G-05) |
| `query_service.rs` transport/authorization/artifact store | **preserve** | no query construction inside; untouched |
| Six-namespace catalog topology, empty `cpg_python/cpg_rust/cpg_derived` | **preserve** | `FAB §6.3`/`§92`; the language waves fill them |

No surface is `encapsulate-temporarily`: every intermediate state in §5.2 is a complete,
gated architecture, not a compatibility layer awaiting removal. The only dual-form object
is `ResultChecksumV1`/`V2`, which is versioned coexistence by contract (KAT continuity),
not silent duality — exit condition: V1 retires when the arrow-58 comparison KATs retire
with the next data-fabric upgrade plan.

### 5.4 Spec-drift items to record with the transition

1. `FAB §7` still types `id16` as Arrow `Binary`; the implementation's governed contract is
   FSB(16)+extension over Binary storage, gate-proven (`id16-extension-contract-check`).
   The Contract-IR revision in Stage 2 should carry the corresponding FAB text revision so
   the spec table matches the governed reality (specs here are revised in place).
2. The `QRY AC-G-46` extension prohibition vs `FAB §§78-82/93` recommended UDTFs/graph
   operators: this design takes the QRY side (LD-05) and leaves graph operators to the
   `W13+` designs, where any escalation must arrive as an `ExtensionDecisionRecord`.

## 6. Proof strategy

Positive target-state, negative legacy-state, and continuity evidence — all named,
executable, and staged with §5.2. Existing gates are reused wherever they already prove
the invariant; new checks become `just` recipes (never raw flags in CI).

**TI-1 (executable ontology).**
- New recipe `ontology-dimension-check`: (a) registry parity — generated Rust consts,
  dimension batch rows, and registry YAML digest agree; (b) referential zero-state — the
  `REP §13` anti-join returns zero rows for every fact code column, over a populated
  fixture publication; (c) serving decoration — `relations` view exposes
  `relation_kind_name` via the dimension join.
- Extended `publication-referential-integrity-check` covering dimension FK contracts.

**TI-2 / TI-4 (one type system; result boundary).**
- `model-family-check schemas` extended: every served schema (base, control, serving,
  result) lowers through the single generated authority; golden per-form result schemas
  asserted with metadata and extension types (not just name/type/nullability — the
  `AC-G-79` comparator definition of equality).
- Negative: zero-hit structural rule for `Field::new` construction inside
  `semantic_query.rs` result-shaping modules (ast-grep rule + `rg`, both, per
  evidence-policy zero-state rule), plus tier-1 proof — the hand-written schema functions
  are deleted and the build is clean.

**TI-3 (engine-known IDs).**
- LD-01 probe as a test: session state exposes the registry; formatter renders id16
  columns via the extension; a field-aware cast preserves extension identity through a
  plan.
- Existing `id16-extension-contract-check` remains the storage/fallback oracle.

**TI-5 (truthful planning facts).**
- New tests: overlay-present statistics report Inexact base+overlay composition (never
  unknown, never exact); PK constraints visible on wrapped providers; adversarial
  pushdown-truth test comparing overlay-path filtered execution against engine-filtered
  reference (the `Exact` claim's standing proof).
- Existing `provider-statistics-contract-check` extended rather than duplicated.

**TI-6 (single generation seam).**
- `AC-G-79` fingerprint equality across Stage 1 (schema bytes unmoved).
- Negative: the dual-list reconciliation function is deleted; `cargo check` clean (tier 1);
  `rg` zero-hit for the legacy generated symbol names over `src/`.

**TI-7 (registry-complete compiler).**
- New `rules/` governance rule (with `rule-tests/` fixtures, negative space included)
  banning integer literals in the compiler's predicate builders; `just governance-scan`
  carries it thereafter.

**TI-8 / continuity.**
- `wave3-integration-check`, `query-determinism-check`,
  `semantic-query-conformance-check`, `query-legacy-zero-state-check`,
  `data-fabric-stack-compat`, and the checksum KATs run green at every stage boundary;
  `ResultChecksumV2` KATs added in Stage 3 alongside V1 continuity assertions.
- Perf: `data-fabric-upgrade-bench` differential between the Stage-0 anchor and each
  stage's proving commit; regression bounds per the existing benchmark comparator
  contract.
- Gate hygiene: every packet that moves tests ships the filter-expression diff for the
  ~20 name-coupled recipes (transition risk #1 from the ops review).

**Oracle-derivation rule** (`P25`): each work packet in the eventual plan derives its four
oracles (behavioral / structural / negative / operational) from the TI it advances, per
the repository's packet-oracle convention.

## 7. Acceptance

**accepted-with-named-assumptions** — ready for implementation planning
(`impl-plan`), with these assumptions to validate in plan preflight, each an executable
probe at the pinned versions (LD-04 and §3.6 deferrals):

1. **A-1 (LD-04):** `ScalarValue::FixedSizeBinary` literal construction, IN-list, and
   FSB(16) hash-join/group-by behavior at DataFusion 55.0.0 — compile+execute probe.
   Consequence if false: the Binary literal-rewrite path stays; D-03/D-07 predicate code
   keeps the current rewrite seam; no architectural change.
2. **A-2 (LD-03 probe):** `DeltaScanConfig::with_schema` compatibility with an
   FSB(16)-presenting schema at `43a0cf10`. Consequence if false: per-scan cast stands
   (already proven acceptable).
3. **A-3 (D-05 deferral):** column-level min/max availability from delta-rs file
   statistics at the pin. Consequence if false: statistics deepening ships row-count/null
   composition only.
4. **A-4 (Stage-2 choreography):** field-metadata addition (`id_domain`) is executable
   under the `AC-G-83` migration route without data rewrite — confirmed by the migration
   probe the `EXACT_PIN` policy already requires. Consequence if constrained: `id_domain`
   ships in the same revision as the next otherwise-required schema change.

Evidence that would force reopening this design: a Delta-pin upgrade adding fixed-width
binary types (reopens LD-03 toward native FSB storage); a QRY contract revision defining
struct-shaped responses (reopens D-08); an accepted `ExtensionDecisionRecord` introducing
graph operators on the query path (reopens the LD-05 boundary); failure of A-4's migration
probe in a way that blocks additive metadata (reopens Stage 2 sequencing).
