# CodeFabric 1.3 specification index

A navigation and traceability layer over the **seven** design artifacts in
`docs/upfront_design/` — the suite governance and release manifest plus the six domain
specifications — together with the implementation roadmap and the pinned library references in
`docs/library_ref/`.

**This directory is not normative.** It restates no contract and settles no design question. It
records where contracts live, how they connect, and — in the [gap register](#7-gap-register) —
which cited authorities do not exist. Where it reports a conflict between two specs, it reports
it; it does not adjudicate it. The manifest and the six specifications remain the only
authorities; `SUITE AC-G-01` is the precedence rule when two of them appear to disagree.

## 1. What is here

| File | Answers |
|---|---|
| **README.md** (this file) | how to cite, what the suite looks like structurally, what is missing |
| [`fact-domain-map.md`](./fact-domain-map.md) | one fact domain traced across all six domain specs, plus the table and serving registries |
| [`library-routing.md`](./library-routing.md) | which library reference chapter covers a given spec section |
| [`wave-traceability.md`](./wave-traceability.md) | which spec sections and contracts each roadmap wave implements |
| [`contract-census.md`](./contract-census.md) | all 84 `AC-G` contracts, the transposed consumer view, and the enumerated registries |
| [`invariants-and-doctrine.md`](./invariants-and-doctrine.md) | the invariants every wave must preserve, traced to their normative homes |

Start with `fact-domain-map.md` if you are writing fact-generation, storage, or query code;
with `wave-traceability.md` if you are scoping a wave plan; with `library-routing.md` if you are
about to call an unfamiliar library API.

## 2. Citation convention

Seven design-artifact tags, plus `RM` for the roadmap. The form follows the `repo-spec §N` /
`tooling-ref §N` precedent already used in `AGENTS.md`.

| Tag | File (all under `docs/upfront_design/`) |
|---|---|
| `SUITE` | `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md` |
| `ONT` | `code_property_graph_present_state_fact_ontology_specification_v1.3.md` |
| `GEN` | `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` |
| `FAB` | `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` |
| `LIFE` | `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` |
| `QRY` | `code_property_graph_semantic_query_specification_v1.3.md` |
| `SRV` | `present_state_cpg_fastmcp_serving_specification_v1.3.md` |
| `RM` | `codefabric_1.3_implementation_roadmap_v1.0.md` |

Citations are written **`TAG §N`, always carrying the section title alongside** — `FAB §26
type_detail`, `LIFE §59 Branch switch / checkout acceleration`. Never bare line numbers: a spec
revision moves every line but usually keeps the title, so the title is what lets you re-find a
section that has drifted. `RM W6` denotes roadmap Wave 6.

`AC-G-NN` and the bare `G-NN` form used inside prose and cross-layer tables are **the same
anchor**. This index normalizes to `AC-G-NN`.

## 3. Suite census

| Tag | Lines | `## N.` sections | `AC-G` contracts | Parts | Appendices |
|---|---:|---:|---:|---:|---:|
| SUITE | 1,901 | 1 (§0 only) | 15 | 5 | — |
| ONT | 4,653 | 88 (§0–§87) | 14 | 10 | 2 |
| GEN | 4,823 | 113 (§0–§104) | 12 | 11 | 3 |
| FAB | 4,408 | 122 (§0–§120) | 8 | 19 | 4 |
| LIFE | 6,238 | 160 (§0–§159) | 10 | 16 | 6 |
| QRY | 6,649 | 122 (§0–§121) | 14 | 10 | 4 |
| SRV | 4,183 | 84 (§0–§83) | 11 | 13 | 7 |
| **Six specs** | **30,954** | **689** | **69** | **79** | **26** |
| **Suite total** | **32,855** | **690** | **84** | **84** | **26** |
| RM | 1,527 | 31 (§0–§30) | — | 6 | — |

Section numbers are continuous across Part boundaries, and a few carry letter suffixes —
`FAB §79A`, `GEN §67A`, `§71A`, `§71B`, `§77A`–`§77E`. Sort accordingly.

### 3.1 Part structure

`spec-outline` does not emit `# Part` or `# Appendix` headings (verified — it maps `## N.` to
items and `### N.N` to members), so this table is the only place the Part structure is written
down outside the specs themselves.

**Four Part numerals are skipped.** `QRY` has no Part IV; `LIFE` has no Part IV; `FAB` has no
Part V and no Part XIII. Never expand a Part range as a contiguous numeral sequence — use this
table.

<details>
<summary><b>SUITE</b> — the governance manifest; §0 only, then Parts</summary>

`SUITE` is structured unlike the domain specs: one numbered section, then Parts holding
`AC-G` contracts and reference material directly.

| Part | Contents |
|---|---|
| §0 Purpose and release authority | §0.1 released domain artifacts · §0.2 the ten global invariants |
| I — Governance and machine contracts | `AC-G-01`–`AC-G-08` |
| II — Permanent gap ownership and propagation | the authoritative `G-01`–`G-84` owner table |
| III — Verification, performance, upgrades, and security acceptance | `AC-G-78`–`AC-G-84` |
| IV — Required generated artifacts | the ~46-path `contracts/` tree |
| V — Implementation-readiness gates | Gates A–G |

Closes with `## Release completion criterion`.
</details>

<details>
<summary><b>ONT</b> — front matter §0–§4</summary>

| Part | Sections |
|---|---|
| I — Language-Neutral Core Ontology | §5–§32 |
| II — Python Ontology Profile | §33–§43 |
| III — Rust Ontology Profile | §44–§58 |
| IV — Derived Fact Families | §59–§60 |
| V — Fact Metadata and Conformance | §61–§67 |
| VI — Canonical Layer Model | §68 |
| VII — Recommended Canonical Relationship Inventory | §69–§81 |
| VIII — Conformance Requirements | §82–§85 |
| IX — Non-Goals | §86 |
| X — Final Specification Principle | §87 |

Appendices: A — Compact Ontology Checklist · B — Explicitly Excluded Analytical Outputs
</details>

<details>
<summary><b>GEN</b> — front matter §0–§3</summary>

| Part | Sections |
|---|---|
| I — Provider Architecture | §4–§7 |
| II — Canonical Extraction Contracts | §8–§13 |
| III — Python Fact Generation | §14–§33 |
| IV — Rust Fact Generation | §34–§51 |
| V — Derived Analyses and petgraph | §52–§66 |
| VI — Complete Fact-Generation Matrix | §67–§79 |
| VII — Reconciliation and Unknown Semantics | §80–§85 |
| VIII — Handoff to Reconciliation, Derivation, and Lifecycle | §86–§88 |
| IX — Rust Workspace Architecture | §89–§92 |
| X — Validation and Conformance | §93–§97 |
| XI — Implementation Sequence | §98–§104 |

Appendices: A — Provider Capability Legend · B — Required Model-Pack Categories ·
C — Explicit Non-Outputs
</details>

<details>
<summary><b>FAB</b> — front matter §0–§2 · <b>no Part V, no Part XIII</b></summary>

| Part | Sections |
|---|---|
| I — Architectural Model | §3–§6 |
| II — Canonical Types, Identity, and Schema Contracts | §7–§11 |
| III — Multi-Table Publication and Snapshot Consistency | §12–§13 |
| IV — Universal Graph Tables | §14–§25 |
| VI — Types, Members, Calls, and Control Flow | §26–§36 |
| VII — Values, Dataflow, Memory, and State | §37–§43 |
| VIII — Effects, Exceptions, Resources, Async, and Generated Semantics | §44–§49 |
| IX — Python and Rust Extension Tables | §50–§58 |
| X — Unknowns, Derived Components, Metrics, and Summaries | §59–§62 |
| XI — Arrow Ingestion and Batch Construction | §63–§66 |
| XII — Delta Table Creation and Write Operations | §67–§75 |
| XIV — Calculations and Derived-Fact Execution | §76–§90 (incl. §79A) |
| XV — Serving Catalog and Query Surface | §91–§94 |
| XVI — Physical Layout and Performance | §95–§101 |
| XVII — Constraints, Integrity, and Schema Evolution | §102–§103 |
| XVIII — Operational Workflows | §104–§109 |
| XIX — Query, Validation, and Observability Artifacts | §110–§112 |
| XX — Rust Workspace Architecture | §113–§114 |
| XXI — Implementation Sequence | §115–§120 |

Appendices: A — Table Dependency Order · B — Default Table Properties · C — Mandatory
Invariants · D — Explicit Non-Outputs
</details>

<details>
<summary><b>LIFE</b> — front matter §0–§3 · <b>no Part IV</b></summary>

| Part | Sections |
|---|---|
| I — Lifecycle and Scenario Inventory | §4–§12 |
| II — Update Categories and Invalidation | §13–§17 |
| III — State Model | §18–§36 |
| V — Git-Aware Repository and Worktree State | §37–§92 |
| VI — Update Pipeline | §93–§99 |
| VII — Atomicity and Serving Snapshot Design | §100–§108 |
| VIII — Scheduling, Parallelism, and Backpressure | §109–§116 |
| IX — Failure Taxonomy and Recovery | §117–§121 |
| X — Agent and MCP Delivery Contract | §122–§129 |
| XI — Durable Operational State | §130–§131 |
| XII — Performance Objectives and Tuning | §132–§136 |
| XIII — Validation and Testing | §137–§147 |
| XIV — Observability and Operations | §148–§150 |
| XV — Shutdown and Recovery | §151–§154 |
| XVI — Rust Workspace Architecture | §155–§156 |
| XVII — Mandatory Invariants | §157–§159 |

Appendices: A — Update-Class Decision Guide · B — Recommended Starting Configuration ·
C — Query Result Example for Non-Compiling Rust · D — Clean-Rebuild Equivalence Procedure ·
E — Recommended Read-Only gix Dependency Profile · F — Core Git-State DTOs
</details>

<details>
<summary><b>QRY</b> — front matter §0–§10 · <b>no Part IV</b></summary>

| Part | Sections |
|---|---|
| I — Query Request Forms | §11–§20 |
| II — Composition and Execution Semantics | §21–§35 |
| III — Canonical Response | §36–§70 |
| V — Python Semantic Query Vocabulary | §71–§80 |
| VI — Rust Semantic Query Vocabulary | §81–§94 |
| VII — Composite Query Examples | §95–§102 |
| VIII — Schema Artifacts | §103–§105 |
| IX — Conformance | §106–§115 |
| X — Agent Authoring Guidance | §116–§120 |
| XI — Final Specification Principle | §121 |

Appendices: A — Canonical Request-Form Names · B — Required Response Distinctions ·
C — Recommended Default Policies · D — Explicitly Rejected Output Classes
</details>

<details>
<summary><b>SRV</b> — front matter §0–§3</summary>

| Part | Sections |
|---|---|
| I — Governing Architecture | §4–§7 |
| II — Rust Daemon Boundary | §8–§17 |
| III — FastMCP and Pydantic Server Contract | §18–§32 |
| IV — Lifespan, Settings, Dependency Injection, and Middleware | §33–§36 |
| V — Query Semantics, Validation, and Error Mapping | §37–§42 |
| VI — Delivery Adaptation | §43–§46 |
| VII — Agent Guidance and Recipes | §47–§53 |
| VIII — Python Implementation Specification | §54–§60 |
| IX — Security and Isolation | §61–§62 |
| X — Observability and Operations | §63–§67 |
| XI — Testing and Verification | §68–§72 |
| XII — Deployment and Lifecycle | §73–§78 |
| XIII — Implementation Phases | §79–§83 |

Appendices: A — Recommended Environment Variables · B — Version and Timeout Matrix ·
C — Generated Adapter Schema Policy · D — Pydantic Feature Decision Matrix · E — Anti-pattern
Inventory · F — Production Readiness Checklist · G — Final Design Rules
</details>

<details>
<summary><b>RM</b> (roadmap) — front matter §0–§4</summary>

| Part | Waves | Sections |
|---|---|---|
| I — Foundation Waves | W0–W3 | §5–§8 |
| II — Core Fact and Continuous-Correctness Waves | W4–W7 | §9–§12 |
| III — Language Semantic Profile Waves | W8–W12 | §13–§17 |
| IV — Advanced Analysis Waves | W13–W14 | §18–§19 |
| V — Query and Output Waves | W15–W18 | §20–§23 |
| VI — Production Acceptance Wave | W19 | §24–§30 |
</details>

## 4. Two synchronized blocks

Every spec opens and closes with the same structure. Knowing this turns a §0.N number into a
suite-wide address.

### 4.1 The `## 0.` governing-contract preamble

`§0.3` through `§0.18` are **byte-identical across all six specs** (verified by diff). Only
`§0.1` varies, carrying the per-document `artifact_id`. Cite `§0.N` once and it holds
everywhere.

| | | | |
|---|---|---|---|
| 0.1 Artifact identity and version | 0.2 Permanent ownership and precedence | 0.3 Canonical component topology and terminology | 0.4 Compatibility and fail-fast negotiation |
| 0.5 Requirement traceability and generated machine contracts | 0.6 Default deployment profile | 0.7 Canonical source-instance and root identity | 0.8 Canonical current-state object and leases |
| 0.9 Freshness policies and barrier semantics | 0.10 Analysis contexts, canonical types, dependencies, and FFI | 0.11 Byte-safe paths, file identity, and source content | 0.12 Canonical IDs and first-class facts |
| 0.13 Orthogonal state dimensions and completeness | 0.14 Reconciliation, derivation, and materialization ownership | 0.15 Query, RPC, and serving boundaries | 0.16 Authorization, source disclosure, and local security |
| 0.17 Conformance, upgrades, and supersession | 0.18 Release-integration status | | |

`§0.18` is deliberately an `##`, not a `###`. `§0.2`'s seven-row ownership table is the
suite's concern→owner map; it names owners in prose, not by filename or section — which is one
reason this index exists.

### 4.2 The architecture-completion contract tail

Every spec ends with an h1 `# CodeFabric 1.3 architecture-completion contracts` part
containing its owned `## AC-G-NN — Title` sections, then:

- `## Cross-layer integration obligations` — a `| Gap | Contract | Permanent owner |
  Integration obligation |` table listing contracts owned **elsewhere** that bind this document.
  Row counts: FAB 32, QRY 27, GEN 23, LIFE 23, SRV 18, ONT 7.
- `## Release conformance obligations`.

`SUITE` does not carry these two tables — it is the origin, not a consumer. Its **Part II** is
the authoritative owner side of the same relation: one row per `G-01`–`G-84` naming the single
permanent full-text owner.

Part II gives owners; the six inbound tables give consumers. Neither direction alone is a
traceability view. The join is in
[`contract-census.md`](./contract-census.md#2-transposed-consumer-view).

## 5. How to navigate

Reach for the navigators before opening a spec:

```bash
just spec-outline                             # map every spec by section
just spec-outline <spec>.md --match '^93\.' --view expanded
just lib-outline  <ref>.md   --match '^Appendix M' --view expanded
```

Three things they will not do for you:

- **`spec-outline` does not emit `# Part` or `# Appendix` headings.** All 116 of them are in
  §3.1 above — 90 Parts and 26 Appendices across the eight artifacts. On `LIFE` that is 22 headings invisible to the navigator.
- **`spec-outline` does surface `## AC-G-NN` sections** — they appear as items alongside the
  numbered sections. Do not grep for them separately.
- **Word-boundary your greps.** `rg -i arrow` over `ONT`/`QRY` is roughly 80% false positives
  (`NARROWS_TO`, "narrowing"); `rg -i chrono` matches "asynchronous" and "synchronous", and
  `chrono` has zero real mentions in the suite. Use `rg -w` or `\b…\b`.

## 6. What the specs do *not* give you

Worth knowing before you go looking:

- **Almost no section-level cross-referencing exists.** `rg '§'` finds **two** section-style
  cross-references in 30,600 lines of spec (`FAB §8`'s citation of `ONT §§62.1–62.10` being the
  substantive one). Specs point at each other by filename or by shared vocabulary token. Every
  section-to-section link in this directory was derived, not copied.
- **No concrete digest or fingerprint value appears in prose headers.** Every
  `canonical_digest` is `external`; the generated artifact index owns computed semantic
  and exact-source identities.

## 7. Gap register

Cited authorities that do not exist, schemes declared but never populated, and conflicts between
specs. Consult this before concluding that you have failed to find something.

### 7.1 Absent referents

The suite governance and release manifest — cited by every spec's `§0` and previously missing —
**is present**. `SUITE` Part II resolves all 84 `G-NN` owners, and `SUITE` Part V defines
Readiness Gates A–G. The generated
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json` is present and is
the single packaged value authority cited by every spec's `§0.1` and `SUITE AC-G-02`; its
released profile must carry each governed artifact's semantic `canonical_digest` and
exact-byte `source_digest`. Rust embeds these bytes directly and Python loads the same package
resource through `importlib.resources`.

The absent `codefabric_architecture_completion_and_missing_design_specifications_v1.0.md`
is historical source material only; `SUITE §0` states that no implementation needs it
to interpret the 1.3 release.

No concrete digest or fingerprint value appears in the prose corpus. Every
`canonical_digest` is `external`; `SUITE AC-G-02` assigns computed values to the
generated artifact index to avoid self-reference.

### 7.2 Specified as generated artifacts, absent as prose

These are **not defects**. `SUITE AC-G-05` and `SUITE` Part IV make the `contracts/` tree a
first-class source of truth, and several registries are specified there rather than enumerated
in prose. Do not go looking for the values in the specifications.

| Thing | Specified at | Generated artifact | Prose status |
|---|---|---|---|
| `CF-<owner>-<four digits>` requirement IDs | `SUITE AC-G-04`, which fixes the owner set `ARCH ONT GEN FAB LIFE QUERY SERVE SEC TEST` and the machine record shape | `contracts/manifests/requirements.jsonl`, `traceability.jsonl` | **Zero instances** in any document. The scheme is real; the IDs are a build output. `AC-G-NN` is the only ID scheme you can cite today. |
| Flag registry | `SUITE AC-G-06`, which fixes the 64-bit band layout — bits 0–31 language-neutral, 32–47 language-profile, 48–55 generated/lowered, 56–62 reserved, bit 63 always zero | `contracts/registry/flag-registry.yaml` | **No individual bit is named.** `flags64` columns appear across ~14 `FAB` tables with `cpg_flags_or`/`cpg_flags_has` as UDFs. |
| Property kind names | `ONT AC-G-71` (20-field property record schema) | `contracts/registry/ontology-property-registry.yaml` | **Not one property name** is enumerated anywhere. |
| Provider registry | `SUITE` Part IV; consumed by `ONT AC-G-70`, `FAB §68` | `contracts/registry/provider-registry.yaml` | No defining prose section. `GEN` Part I describes provider architecture but is not a registry. |
| Entity-kind numeric codes | `ONT AC-G-70` defines `kind_code` as positive append-only; `SUITE AC-G-06` fixes the code discipline (code 0 reserved, append-only, increments of ten, no gap insertion after release) | `contracts/registry/ontology-entity-registry.yaml` | Schema only. `ONT §62` is the sole place in the corpus where stable integer codes are actually written down (~104 values). |

The full ~46-path `contracts/` tree is enumerated at `SUITE` Part IV. `RM W1` builds it, and
`SUITE` Gate A is the check that it is complete.

### 7.3 Conflicts between specs

Recorded, not adjudicated. `SUITE AC-G-01`'s ownership map is the precedence rule for each.

| Conflict | Detail | Precedence |
|---|---|---|
| **Error names** | `QRY §47 Canonical error record and registry` and `SRV AC-G-65 Stable error registry and layer mappings` list overlapping errors under different names — `AMBIGUOUS_SEMANTIC_PHRASE` vs `SEMANTIC_PHRASE_AMBIGUOUS`, `WORKSPACE_UNAUTHORIZED` vs `WORKSPACE_NOT_AUTHORIZED`, `NEGATIVE_CLAIM_INDETERMINATE` vs `NEGATIVE_PROOF_INDETERMINATE`. | `SUITE` Part II assigns `G-65` to the serving spec, and `SUITE AC-G-01` gives it "RPC framing … adapter contracts". `SRV AC-G-65` wins; `QRY §47`'s names need reconciling against it. |
| **gRPC method count** | `SRV §9 Protobuf service and accepted-handle protocol` sketches a 7-method service; `SRV AC-G-58 Complete Protobuf service and query state machine` specifies 9. | Same document. `§0.2` ("A less-specific statement elsewhere in this document SHALL be read through the 1.3 contract sections") makes `AC-G-58` authoritative. |
| **Conformance profile silence** | `LIFE`, `QRY` and `SRV` never name the five conformance profiles (`CORE_SOURCE_V1` … `SERVING_V1`), though all three gate behavior on capability status. | `ONT AC-G-72` owns them (`G-72`). A gap in cross-referencing, not a contradiction. |
| **`RM §29` traceability** | Incomplete for 11 of 20 waves and cites four non-existent Parts. | The roadmap is explicitly subordinate: `RM §0` states the 1.3 specifications and `SUITE` prevail. Corrections in [`wave-traceability.md`](./wave-traceability.md#5-corrections-to-rm-29). |

### 7.4 Library coverage gaps

Libraries the specs depend on that have **no deep reference** in `docs/library_ref/`:

| Library | Where the specs need it | Status |
|---|---|---|
| **Embedded SQLite** (WAL operational store) | `FAB §13`, `AC-G-23`, `AC-G-26`; `LIFE §130`, `§131`, `AC-G-27`, `AC-G-41`, `AC-G-62`; `SRV AC-G-60`, `AC-G-63` | No doc, **and no crate is ever named** — not `rusqlite`, not `sqlx`. `LIFE AC-G-27` only says what it must *not* be ("No RocksDB, redb, or independent append journal"). |

The canonicalization/BLAKE3 references are routed by `canonicalization-lib-ref`; Python
gRPC, Protobuf, and orjson references are routed by `grpcio-orjson-protobuf-ref`.
Rust Prost/Tonic descriptor generation is covered by the pinned source/API evidence
recorded by the Wave-1 plan until a dedicated Rust reference is added.

References that exist but that **no skill routes**:

| Reference | Covers | Needed by |
|---|---|---|
| `rust_parallel_concurrency_stack_reference_2026-08-19.md` (7,135 lines) | Tokio §§6–15 · Rayon §§16–24 · Crossbeam §§25–31 · DashMap §§32–36 · tokio-rayon §§37–38 · integration §§39–51 | `LIFE §70`, `§71`, `§109`–`§113`, `§114`, `§151`–`§153`; `GEN AC-G-32` |
| `rust_development_environment_tooling_agent_reference_2026-08-19.md` (6,463 lines) | cargo tooling, `rustc-dev`, Miri, Maturin (§47), nextest, insta; §60 compiler-internals and CPG workflow | `GEN §7.4`, `AC-G-31`; `ONT AC-G-17`; `RM W0` |

`gix-notify-ref` already flags the concurrency reference as unrouted. See
[`library-routing.md`](./library-routing.md) for the section-level map.

### 7.5 Skill-routing corrections

Found while building this index and **fixed on 2026-08-20**, recorded so the next reader knows
what changed and why.

| Was | Now |
|---|---|
| `CLAUDE.md` and the `code-facts-lib-ref` / `gix-notify-ref` skills cited the specs by their `_v1.2.md` filenames | All cite `_v1.3`. `CLAUDE.md` lists the governance manifest as the seventh artifact and points here. `gix-notify-ref`'s Part IV note carried stale line numbers (1435/1994) and said §26–§36; it now cites Part III = §18–§36, watcher sections §27–§36, Part V from §37 |
| `code-facts-lib-ref` claimed `GEN §2` "names exactly these four as its source basis" | `GEN §2` names **five** library references — those four plus `petgraph.md`, routed by `petgraph-ref` — alongside the ontology companion |
| `datafusion-pyarrow-rust-ref`, `deltalake-rust-ref` and `typer-rich-ref` carried **`smartref`** project context — another project's crates, paths and command vocabulary | All three re-grounded on CodeFabric. The two Rust ones now carry a **spec-section → chapter map** and the boundary rules that constrain them; version pins are sourced from `FAB §2.1` rather than a lockfile, with a note that they move |
| `deltalake-rust-ref` cited eight nonexistent `docs/library_ref/` documents; `datafusion-pyarrow-rust-ref` cited one (`datafusion_54vs53.md`) | Every remaining mention states plainly that the file is absent. `deltalake_rust_1.0.0_9f922319_advanced_reference_2026-08-20.md` is declared the only Delta reference here |
| `datafusion-pyarrow-rust-ref` and `deltalake-rust-ref` pointed at a `datafusion-pyarrow-ref` sibling skill that does not exist | Replaced with the fact that `docs/library_ref/datafusion.md` and `pyarrow.md` exist but are unrouted — and that `SRV §18` and `SRV §6` invariant 3 give the Python side no Arrow/DataFusion demand |
| `datafusion-pyarrow-rust-ref` advertised chapter ranges its targets do not have | Counted from the files: `datafusion_planning_rust.md` ends at **§56** (not §60), `datafusion_calculations_rust.md` at **C13** (not C26). See [`library-routing.md §1`](./library-routing.md#1-reference-shorthands) |
| `attrs-cattrs-ref` and `typer-rich-ref` silently routed reference documents that do not exist | Both now self-declare. None of these libraries is a direct CodeFabric dependency after the Wave 0 packaging cutover; transitive adapter-lock presence is not adoption. The skills remain available only for a future direct seam. |

Two things remain genuinely absent rather than merely misattributed: `docs/library_ref/attrs.md` +
`cattrs.md` (neither is a current direct dependency), and the SQLite gap in
[§7.4](#74-library-coverage-gaps).

## 8. Verification

Every citation in this directory was checked mechanically during authoring against a ground-truth
section table built from `spec-outline --json=compact`, a fence-aware extraction of the 116
`# Part`/`# Appendix` h1 headings the outliner does not emit, and `## AC-G-` anchors. The checks
asserted both that each section exists and that the title recorded here still matches its
heading.

```text
tagged spec citations (TAG §N / TAG AC-G-NN)   326 checked   0 unresolved
contextual § citations in library-routing      227 checked   0 unresolved
title assertions                                35 checked   0 mismatched
library-reference chapter citations             238 checked   0 unresolved
closure: all 84 AC-G contracts owned exactly once            pass
closure: all 62 FAB table sections mapped                    pass
closure: every wave W0-W19 has a row                         pass
closure: every spec-named library routed or in gap register  pass
internal cross-links and anchors                              0 broken
typos docs/spec_index/                                       clean
```

**Verified against the seven `v1.3` design artifacts and roadmap `v1.0` as of 2026-08-20.**

Drift is not hypothetical. `FAB §2`'s Arrow and DataFusion pins moved from `58.3.0`/`54.0.0` to
`58.4.0`/`54.1.0` while this index was being written, and the delta-rs pin moved from
`35cfed45…` to `9f922319…` — bringing a Rust floor of `1.94.1`, Cargo resolver `3`, and eleven
new `FAB` subsections — the day after. The
[pin ledger](./library-routing.md#8-version-pin-ledger) records the current values.

There is **no committed checker** — the scaffolding above was discarded. When a spec is revised,
section numbers move and this index will drift silently. Re-verification is manual; that is the
accepted tradeoff. Titles are carried alongside every citation precisely so a drifted section can
be re-found. If drift becomes a recurring problem, a `scripts/spec-index-check` plus a `just`
recipe, in the style of `tooling/ast-grep/outline/specs.test.sh`, is the cheap fix.
