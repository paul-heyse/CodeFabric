---
artifact: design-principles-remediation-proposal
date: 2026-08-24
version: v1
status: complete
principles_path: docs/library_ref/full_data_fabric_design_principles.md
principles_digest: c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1
conformance_review_path: docs/reviews/design_principles_conformance_2026-08-23_v1.md
conformance_review_digest: 9d3ec5bcd8569a8acc8900162f8859546dea4778951f932b751ec99a6c832fe5
alignment_manual_path: docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md
baseline_commit: f2dfcfe25dbfe46f0ca779a2fc4273787e18a445
---

# Design-principles remediation proposal — unifying the data fabric on DataFusion 55 + Arrow 59

This document proposes the **full resolution** of the 124 findings in the pass-3
conformance register and the achievement of full alignment with the 25 design
principles, by unifying the data fabric around best-in-class utilization of
DataFusion 55.0.0 and Arrow 59.2.0.

**This is a target-design proposal, not an implementation plan.** It names
mechanisms, authorities, contracts, and evidence obligations; it deliberately does
not name modules, files, packet boundaries, or sequencing below the wave level.
The `impl-plan` workflow converts it into dependency-closed packets; `plan-audit`
challenges it before execution.

## Citation tags used in this document

| Tag | Document |
|---|---|
| `PRIN Pn` | `docs/library_ref/full_data_fabric_design_principles.md`, principle *n* |
| `CONF DP-nnn` | `docs/reviews/design_principles_conformance_2026-08-23_v1.md`, finding *nnn* |
| `ALIGN` | `docs/library_ref/datafusion55_arrow59_design_principle_alignment_manual_2026-08-24.md`; pattern IDs (`MOD-`, `ARR-`, `SCH-`, `CAT-`, `EXP-`, `LOG-`, `PHY-`, `SRC-`, `RUN-`, `INT-`, `OBS-`, `GOV-`, `EXT-`, `TST-`) refer to its Part II catalogue; `ALIGN A.1`/`A.2` to its version-specific appendix |
| `DFREF §n` | `docs/library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md`, chapter *n* (chapters `0`–`40`, schema series `S1`–`S15`, planning series `41`–`56`, calculation series `C1`–`C13`) |
| `SUITE` / `ONT` / `GEN` / `FAB` / `QRY` / `LIFE` / `SRV` / `RM` | the design corpus, per `docs/spec_index/README.md` |

---

## 1. Purpose, inputs, and baseline

### 1.1 What changed since the register

The conformance register measured the tree at `d89cc90` under Arrow `=58.4.0` /
DataFusion `=54.1.0` / delta-rs `9f922319`, and recorded the Arrow 59 /
DataFusion 55 references as "research, cited by nothing." That is no longer the
state of the tree. At this proposal's baseline (`f2dfcfe`), the root manifest pins
**Arrow/Parquet `=59.2.0`, DataFusion `=55.0.0`, `object_store` `=0.13.2`, and
delta-rs revision `43a0cf10`** — the exact environment the alignment manual is
written against. The data-fabric upgrade the register treated as future work is
the platform this proposal builds on; every mechanism below is selected for the
pinned 55/59 environment (`ALIGN A.1`, `A.2`, `A.3`; `DFREF §34`, `§40A`).

None of the register's findings is invalidated by the upgrade: the findings are
architectural (authority, truthfulness, provenance, proof), not version defects.
The upgrade changes *what the best resolution is*, not *whether the findings
stand*.

### 1.2 What "full resolution" means here

- Every `CONF` finding DP-001–DP-124 receives a disposition in §4: a named
  remediation move, a spec-feedback routing, or an explicit accepted-state
  record. No finding is dropped silently.
- The four divergence-ledger closures in the register's §6 (tenancy, masking,
  advisory display metadata, user-visible `Expr` transparency) **remain closed**.
  Full alignment does not mean implementing principle clauses the design corpus
  explicitly refuses (§9).
- The five unowned principles in the register's §7 are resolved by *giving them
  mechanisms here* and *routing their normative homes to the owning
  specifications* per `RM §28` (§6).

### 1.3 Design posture

The alignment manual's constitution governs every move: model the truth once,
compile it through Arrow and DataFusion, keep the optimizer able to see it,
execute through truthful contracts, and preserve identity, state, evidence, and
lineage (`ALIGN` Part VIII). Three postures follow from repository doctrine and
bound what "best-in-class utilization" may mean here:

1. **The extension ladder is honored** (`PRIN P14`, `ALIGN` §2.3). Nothing below
   proposes a custom `ExecutionPlan`, custom planner, or UDF. Every query-plane
   requirement in this system is expressible with built-in `Expr` nodes,
   `LogicalPlanBuilder`, and the existing `TableProvider` seam — the highest
   viable levels. `EXT-09`/`EXT-10` are explicitly *not* selected.
2. **Agents never see SQL or plan syntax** (`SRV §6` inv. 5). DataFusion's
   logical plane becomes the daemon's *internal* compiled representation; the
   semantic-envelope contract (`QRY`) remains the only agent-facing surface.
3. **Python remains presentation only** (`RM §1` inv. 8). Every mechanism that
   computes, validates, or re-derives moves behind the daemon contract.

---

## 2. Resolution thesis — one fabric, one compiler, one authority per concept

The register's three dominant patterns — divided authority (P3), untruthful
capability claims (P20), and proof that names contracts without executing them
(P25) — share one root cause: **the system compiles its contracts into artifacts
but does not execute through them.** Registries are generated and then re-typed
as string literals; schemas are generated and then re-encoded by hand; the query
path bypasses planning with `format!`-built SQL; golden answers are descriptors
rather than outputs. The remediation is therefore not 124 point fixes but a
single architectural commitment, applied ten ways:

> **Every semantic authority already in `contracts/` compiles into exactly one
> executable representation per plane — Arrow objects for data, DataFusion
> logical objects for queries, generated registry types for vocabularies — and
> no consumer is permitted a second encoding.** Where an authority is missing
> (fingerprint domains, request identity, evolution policy, golden answers), it
> is created as a contract first and compiled second.

Mapped onto the fabric stack of `PRIN §8` and `ALIGN §2.1`:

```text
contracts/ (schema IR · registries · identity · bundles · policy)   ← authorities
        ↓ one compiler per authority (model drivers)                 R1, R10
generated Rust/Python/proto/Arrow forms, digest-linked               R1
        ↓
QuerySpec → Expr/LogicalPlan (no SQL text, no literals)              R2, R3
        ↓
TableProviders with truthful pushdown/statistics/constraints         R6
        ↓
Arrow RecordBatch / IPC as the only fact transport                   R5
        ↓
Delta commits + snapshot manifests + plan artifacts, all pinned      R4
        ↓ observed through
phase-tagged errors · persisted QueryPlanArtifact · explain surface  R4, R7
        ↓ proved by
contract-named oracles · executable Gate B · convergence comparator  R9
        ↓ presented by
a thin adapter over one proto boundary family                        R8
```

---

## 3. Program of record — ten remediation moves

Each move states its intent, mechanism (with `ALIGN` pattern IDs and `DFREF`
chapters), the DataFusion 55 / Arrow 59 capabilities it leverages, the findings
it closes, and the evidence that proves it. Findings marked ◐ are closed jointly
with another move.

---

### R1 — One identity, fingerprint, and registry authority

**Intent.** End the P3 pattern at its source: every enum vocabulary, digest
domain, and identity recipe has exactly one authority in `contracts/`, one
generated projection per language, and an executable drift oracle. (`PRIN P3`,
`P18`; `ALIGN` P3, P18; patterns `MOD-04`, `MOD-06`, `SCH-10`, `OBS-09`.)

**Mechanism.**

- **Fingerprint-domain registry.** The 27+ ad-hoc `b"codefabric…"` digest
  domains (DP-120) become records in a new `contracts/identity` registry:
  domain string, separator convention, field set, field order, normalization.
  `crate::identity` compiles it into the *only* digest constructors; a
  governance rule forbids `blake3::Hasher` construction outside
  `src/identity.rs` and the generated recipes (extending the existing
  boundary-rule harness). This subsumes the duplicated `capability-scope`
  fingerprints (DP-086, DP-119), the twice-implemented `fact_evidence` identity
  (DP-031), and the nine independent `digest_bytes` definitions (DP-044, digest
  half).
- **One registry module per language, generated, imported.** The
  `registries.rs` / `model_registries.rs` twin emission (DP-040) collapses to a
  single generated Rust module; the governed Python registry module becomes the
  imported one by fixing the `role_for` path-matching defect
  (`repository_model.rs:825-829`) that currently governs the orphan and ignores
  `model_registries.py` and `identity.py` (DP-001, DP-003). The hand-written
  `NewlineKind` and `FreshnessState` re-declarations are deleted in favor of the
  generated enums (DP-039, DP-118), and the crosswalks they required disappear.
- **Registry→proto emission.** Wire enums in `contracts/rpc/*.proto` whose
  domain exists in `enum-registry.yaml` are generated from the registry by the
  proto driver, ending the `QueryExecutionState` double authority (DP-002). Wire
  codes are read from registry records, never reconstructed by array-position
  arithmetic (DP-085).
- **Normalization in generated recipes.** The CBEF recipe generator emits the
  `normalization: ASCII_LOWER` contract field it currently drops, so the
  generated path can honor `cbef-v1.yaml` before
  `model-no-positional-cbef-construction` pushes production onto it (DP-005).
  Public IDs mint only through `identity::encode_public_id` (DP-098 ◐).
- **Cross-root agreement by generation, not duplication.** The
  daemon/extractor `digest_frames` pair (DP-083) is emitted by the model
  compiler into both Cargo roots from one template, with a byte-equality oracle
  — model-derived duplication with drift detection, respecting the dated-nightly
  toolchain boundary (no shared crate; `AGENTS.md §1`).
- **Authority reads at startup.** Advertised digests are read from the installed
  artifact they describe — the query-language bundle digest comes from
  `contracts/bundles/query-language-bundle.json` at build or bootstrap, never
  from a copied literal (DP-076). Registry vocabularies referenced by the golden
  corpus and query plane are imported from generated constants
  (DP-095 ◐, DP-121 ◐, DP-122: the six-place query-form vocabulary becomes a
  registry domain with serde renames derived from it).

**55/59 leverage.** None required — this move is the application-overlay half of
`ALIGN §0.3` ("durable semantic-model registry" is explicitly not supplied by
the libraries). Its Arrow-facing consequence is that every generated schema and
registry projection carries the digest link that R6 validates.

**Closes.** DP-001, DP-002, DP-003, DP-005, DP-031, DP-039, DP-040, DP-044
(digest half), DP-076, DP-083, DP-085, DP-086, DP-095 ◐, DP-118, DP-119,
DP-120, DP-121 ◐, DP-122; enables DP-017/DP-117 closure in R7.

**Evidence.** Registry-conformance KATs decoded by both languages from shared
fixtures; a digest-domain census oracle (`registered domains == domains in use`,
the DP-120 detector inverted into a gate); byte-equality oracle across the two
roots; `rg -c 'Cancellation|blake3::Hasher'`-class structural rules promoted
into the tested ast-grep harness. (`TST-01`, `TST-14`; `DFREF §32`.)

---

### R2 — The semantic query plane becomes a compiled `LogicalPlan` pipeline

**Intent.** Replace the `format!`-built SQL string and the compile-time-constant
response states with the compiler chain the principles document opens with:
typed request → validation → binding → `LogicalPlan` → execution → observed
result. This is the centerpiece unification: DataFusion's logical plane becomes
the daemon's only internal query representation. (`PRIN P1`, `P2`, `P6`;
`ALIGN` P1, P2, P6; patterns `MOD-02`, `MOD-03`, `MOD-05`, `MOD-07`, `LOG-01`–
`LOG-07`, `EXP-01`, `EXP-02`.)

**Mechanism.**

- **A Rust `QuerySpec` compiler.** The semantic envelope (`QRY` request forms)
  parses into a typed `QuerySpec`; a single binder compiles it through
  `LogicalPlanBuilder` against the serving session's `DFSchema`s — projections
  as typed column lists, predicates as `Expr` trees, limits as logical fetch
  nodes, ordering as `SortExpr` (`DFREF §43`, `§11`, `§19`). SQL text ceases to
  exist on this path (`semantic_query.rs:322` retired), which resolves the P6
  register row's "plan layer bypassed" verdict (DP-098's DTO family becomes the
  compiled form's *output* projection, not a parallel representation).
- **Advertised filters become real.** `QueryInput`/`QueryPredicate` and
  `response_projection` — currently public DTOs whose non-empty forms are
  unconditionally rejected (DP-123) — compile to `Expr` predicates and
  projection lists through the same binder. What the schema advertises, the
  binder implements; what the binder cannot implement is removed from the
  schema (`PRIN P20`: conservative claims).
- **Result states are computed, not declared.** The six `&'static str` state
  fields (DP-110) become the generated registry enums (R1), assigned from
  execution facts: `limit_state` from fetch-vs-produced counts (DP-077);
  `freshness_state` from the live freshness reading on *both* paths (DP-109);
  `execution_state`/`completeness_state` from the runtime outcome (DP-080).
  `EffectiveLimitsProfile.profile_digest` hashes the limit values through the R1
  fingerprint registry (DP-111). `FreshnessState::Unavailable` gains its
  production writer via the continuous engine, or is withdrawn from the
  advertised set until it has one (DP-112) — unknown is preferable to falsely
  known.
- **Activation.** The query service becomes reachable from `daemon::serve`:
  `StaticConfig` declares the query socket, and the coordinator constructs
  `ContinuousWorkspaceEngine` and `ProductionQueryService` on the production
  path (DP-075). Until W17's full RPC scope lands, activation may be minimal
  (bind, health, one KAT query end-to-end), but the island must have inbound
  edges from `serve` for any register resolution living in it to count as
  proved. `CORE_SOURCE_V1` coverage computed by the corpus checker is returned
  through the status surface rather than dead-ending in a test (DP-105 ◐).
- **Policy validation as a plan pass.** The semantic-envelope rejection rules
  (evaluative-request refusal, table/function allowlists) run as a logical-plan
  validation pass between binding and execution (`LOG-07`, `GOV-03`;
  `DFREF §46`), giving the refusal doctrine a structural enforcement point
  instead of string checks.

**55/59 leverage.** `SessionState`-scoped planning snapshots (`RUN-03`;
`DFREF §3`) give each query a pinned catalog/config identity for R4's artifact;
parameterized plans (`DFREF §5`, `§43`) keep per-request literals out of plan
fingerprints (R3).

**Closes.** DP-075, DP-077, DP-080, DP-095, DP-098, DP-105 ◐, DP-109, DP-110,
DP-111, DP-112 ◐, DP-123; converts the P6 register row from
`conflict (regressed)` to enforced.

**Evidence.** Cross-entry equivalence tests (spec-compiled plan vs. the
retired SQL rendering during transition — `TST-06`); optimized/unoptimized
result equivalence; adversarial state-truth tests that force stale, limited,
partial, and failed outcomes and assert the reported enums (`TST-03`-style
falsification, per `ALIGN` P20 evidence).

---

### R3 — Deterministic result identity and modeled reproducibility

**Intent.** Make `result_checksum` a truthful function of the query and
snapshot, and make reproducibility a modeled status rather than an implied
promise. (`PRIN P19`; `ALIGN` P19; patterns `MOD-06`, `OBS-10`, `RUN-10`.)

**Mechanism.**

- **Imposed total order.** The R2 binder appends a canonical `SortExpr` over
  each result form's declared identity columns (from the generated table specs)
  whenever the request does not fix an order. DataFusion's multi-partition
  merge does not preserve arrival order (`DFREF §21`), so the checksum
  contract is *defined over the canonically ordered stream* — the DP-012
  defect (order-sensitive sequential hasher over unordered partitions) becomes
  structurally impossible rather than configurationally avoided. Where a
  canonical sort is disproportionate, the fallback is an order-insensitive
  commutative accumulator; either way the contract states which function of
  what inputs the checksum is.
- **Plan and environment fingerprints.** Query identity is
  `(plan_fingerprint, snapshot_manifest_digest, config_fingerprint)` — the plan
  fingerprint computed from the compiled `LogicalPlan` under an
  engine-version-namespaced canonicalization (`MOD-06`; `DFREF §56`;
  `ALIGN` P18's caution that native serializations are version-coupled). This
  replaces `query_id = f(sql, snapshot_id)` and separates *what was asked* from
  *who asked* (request identity is R4's).
- **Reproducibility status.** `QueryPlanArtifact` gains the `Reproducibility`
  record of `PRIN §20` — `deterministic`, `inputs_pinned`,
  `volatile_functions`, `environment_recorded` — derived from the plan (no
  volatile functions exist today; the field exists so the claim is checked, not
  assumed) and from `RUN-10` environment capture.

**Closes.** DP-012; provides the mechanism half of the register's P19 row (the
normative home routes to `LIFE` per §6).

**Evidence.** A determinism harness executing the same spec over the same
frozen snapshot across varied `target_partitions` and asserting checksum
equality (the DP-012 detector inverted); replay tests under pinned environment
(`TST-06`, `TST-14`).

---

### R4 — Provenance closure: the artifact plane, execution identity, and retained lineage

**Intent.** Every durable result resolves the chain of `PRIN §11` — commit →
execution → plans → spec versions → input versions → schema fingerprints →
source snapshots — through stored references, and the richest diagnostic
artifact in the system stops being dropped. (`PRIN P9`, `P10`, `P17`, `P24`;
`ALIGN` P9, P10, P17, P24; patterns `OBS-01`–`OBS-12`, `RUN-05`.)

**Mechanism.**

- **Persist the artifact bundle.** `QueryPlanArtifact` — already carrying
  logical/optimized/physical plan text, versions, snapshot and publication IDs,
  source table versions, output schema, metrics — is written to
  `result_artifact_lease` (its designed home with zero INSERT sites, DP-036),
  keyed by execution identity, with the retention policy `ALIGN` P17 requires.
  `EXPLAIN`/`EXPLAIN ANALYZE` output (including the machine-readable format,
  `DFREF §30`, `§55`) joins the bundle.
- **Complete the artifact's pin.** The artifact records all seven manifest
  bundle IDs, overlay generation/digest, and overlay-supplied table versions —
  not 1 of 7 and base tables only (DP-056) — and the control-schema capture is
  stamped with a generation fingerprint recorded alongside
  `source_table_versions` (DP-035), turning the one unversioned cache in the
  fabric plane into a `RUN-08`-conformant entry.
- **Request and execution identity in Rust.** `semantic_request_id` and
  `mcp_call_id` enter the daemon (proto → handler → artifact → trace), and an
  `execution_id` is allocated *before* planning (`ALIGN` §11 flow), propagated
  through `TaskContext` (`RUN-05`) and tracing spans (`OBS-06`). Two agents
  issuing identical SQL become distinguishable at the fabric layer (DP-053).
- **Join keys that join.** `provider_run.owner_id` is encoded as the same
  16-byte Id16 every fact table length-checks, restoring the one
  provider-run→fact join (DP-023). The wave tables gain their writer — the
  continuous engine records `update_wave`/`update_wave_item` rows and validates
  `wave_id` as Id16 (DP-022). `table_mutation_operation` gains typed scope
  columns (`workspace_id`, `analysis_context_id`, `source_generation`,
  `wave_id`) and a `workspace_scope`, ending join-by-substring; the three
  no-preimage digest keys either gain stored preimages or are re-documented as
  integrity (not provenance) fields (DP-054).
- **Evidence and diagnostics reach their tables.** Derived relations get
  evidence rows through the same accumulator the other projections use
  (DP-026); `IngestDiagnostic`/`ConflictRecord` are written to the `diagnostic`
  table (DP-050); provider attribution carries the producer the call site
  already knows, plus `derivation_code`, instead of six constants (DP-024 ◐
  with R5). `capability_status` population is completed: `reason_code` and
  `diagnostic_id` are emitted so *unknown* is expressible, and the query
  service advertises real statuses instead of `Vec::new()` (DP-013 residual).
- **The snapshot's source link is persisted.** `source_blob_digests` joins
  `ServingSnapshotManifestBody`, making the snapshot→bytes link
  content-addressed rather than a NULLable lease field (DP-052). For fact-row
  identity, the `file_id` location-identity design (DP-051) is retained *as a
  declared contract* — location identity plus pinned Delta version — and the
  ambiguity is routed to `GEN §13`'s owner per §6; the persisted manifest link
  makes the pin resolvable, which is what closure requires (`PRIN §11`:
  "stable references and fingerprints are sufficient").
- **Retention respects closure.** `SnapshotRetentionSet` unions provenance
  reachability — retained publications protect their `provider_run`,
  `table_mutation_operation`, `source_inventory`, and source-blob explainers;
  terminal cleanup checks publication reachability before deleting (DP-027).
- **An explain surface.** `explain_version(table_code, delta_version)` reads
  Delta `history()` commit metadata and the artifact store, giving operators
  the "why does this row exist" traversal (DP-055; `OBS-12`; deltalake
  `history()` per the delta-rs reference).

**55/59 leverage.** `EXPLAIN ANALYZE` operator metrics and the PG-JSON explain
format (`DFREF §30`) make the bundle machine-diffable; `file_row_index()`
(`ALIGN A.1`) is available for file-relative provenance if source-image ingest
later flows through file scans — recorded as a candidate, not selected now.

**Closes.** DP-013 (residual), DP-015 ◐, DP-022, DP-023, DP-024 ◐, DP-026,
DP-027, DP-035, DP-036, DP-050, DP-051 (contract + routing), DP-052, DP-053,
DP-054, DP-055, DP-056.

**Evidence.** A closure-traversal oracle: from a committed Delta version,
resolve commit → execution → artifact → spec/schema/source identities, failing
on any missing link (`OBS-12`); failure-path tests asserting a partial bundle
exists through the failing phase (`ALIGN` P17 evidence); retention tests
proving a retained publication's explainers survive GC.

---

### R5 — Arrow IPC as the sole provider fact protocol; one ingest pipeline

**Intent.** One transport, one decoder, one validation pipeline for facts —
Arrow end-to-end from provider adapter to Delta publication, with the provider
hierarchy made real. (`PRIN P4`, `P5`, `P7`, `P8`, `P22`; `ALIGN` P5, P7, P8;
patterns `ARR-03`, `ARR-08`, `ARR-10`, `INT-01`, `INT-08`, `CAT-10`.)

**Mechanism.**

- **One channel.** Of the two parallel provider channels — `ArrowIpcChunk`
  with no decoder anywhere, and `ObservationMessage` with one test sender
  (DP-028) — the Arrow IPC channel becomes the only one. A real
  `arrow::ipc::reader::StreamReader` decode path validates chunks into
  `ValidatedFactBatch`es; the `CanonicalFact` enum family and its
  four-of-ten-tables ceiling (DP-029) are retired. Coverage is schema-driven:
  any table in the generated specs is representable because representation *is*
  a `RecordBatch` against that spec's Arrow schema. Arrow 59's IPC improvements
  — configurable stream compression and sans-I/O encoding (`ALIGN A.2`) — are
  adopted as the protocol profile, with the codec recorded in the protocol
  contract.
- **One ingest pipeline.** The projection and observation paths (DP-030) merge
  above `ValidatedFactBatch::validate`: one cross-table referential validator,
  one row-budget mechanism, one provider-precedence table (the explicit
  `BTreeMap` sort), one conflict-disposition and evidence encoder, and
  schema-fingerprint fencing on every path. Arrow 59's fallible
  `FixedSizeBinaryArray::TryFrom` construction (`ALIGN A.2`) turns Id16 shape
  defects into contract-validation failures at the boundary.
- **The provider hierarchy becomes real.** `TreeSitterAdapter` and
  `RuffAdapter` implement `ProviderAdapter` (DP-009); the ingest entry point
  takes a registry-driven collection of adapter outputs instead of a signature
  fixed to `(tree, ruff)` (DP-010), so adding Pyrefly or the rustc extractor
  changes registration and one implementation — the `PRIN §6` design test.
  The two hand-maintained field-role tables (DP-033) become one generated
  crosswalk registry (provider raw role → `SyntaxFieldRole`), with the current
  semantic coercions either declared as records or removed.
- **Provider facts survive the boundary.** `evaluation_ordinal`,
  `source_ordinal`, line/column, `depth`, and provider-parsed names are carried
  as batch columns instead of being dropped and re-derived downstream
  (DP-032); attribution columns carry the true producer and derivation rule
  (DP-024 ◐ with R4). `RuffTokenClass`→`TokenKind` narrowing becomes a declared
  registry mapping or is removed in favor of the raw-plus-normalized pair the
  doctrine requires ("raw and normalized coexist").
- **One cancellation type.** The five unrelated cancellation encodings
  (DP-011) collapse to one `Cancellation` handle threaded from the RPC
  boundary through provider execution to stream polling — the structural
  precondition for `SRV §6` inv. 10's end-to-end cancellation, which R8
  completes at the adapter.
- **One extractor seam.** The `--extract-json` bypass protocol (DP-084) is
  deleted; the gRPC + Arrow IPC path becomes the tested path, and the
  extractor's determinism oracle runs against it. `ProviderJobSpec` stops being
  the domain type: a domain DTO owns the seam and the prost message is confined
  to the rpc adapter (DP-047), ending the lossy `ScopeBegin`→`Progress` string
  collapse.
- **Runtime state hygiene.** `AdmissionController` maps gain eviction tied to
  workspace lifecycle, and `SourceImageStore`'s ownership doc-comment is made
  true (coordinator-owned) or corrected (DP-049).

**55/59 leverage.** IPC compression + sans-I/O encoding (`ALIGN A.2`);
`TryFrom` fallible construction; Arrow kernels for any batch-level
normalization the merged pipeline performs (`ARR-06`), replacing row loops.

**Closes.** DP-009, DP-010, DP-011, DP-024 ◐, DP-028, DP-029, DP-030, DP-032,
DP-033, DP-047, DP-049, DP-084.

**Evidence.** Provider-contract suite run identically against every adapter
(`TST-02`; the `PRIN §6` substitution test as an executable oracle); IPC
round-trip fixtures across the daemon/extractor boundary (`TST-09`);
cancellation propagation tests from RPC to stream drop (`TST-10`); a
differential test proving the merged pipeline reproduces both former paths'
accepted corpora.

---

### R6 — Truthful `TableProvider`s and executable schema contracts

**Intent.** The serving plane's capability claims become exact, inexact, or
absent — never decorative — and the schema IR's declared semantics (foreign
keys, semantic types, normalization, evolution) become enforced or explicitly
reclassified. (`PRIN P12`, `P20`, `P21`; `ALIGN` P12, P20, P21; patterns
`SCH-01`–`SCH-12`, `CAT-05`–`CAT-07`, `GOV-06`, `GOV-07`.)

**Mechanism.**

- **Statistics and pushdown truth.** The overlay provider's `statistics() ->
  None` plus asserted-because-set pruning/repartition flags (DP-019) are
  replaced by `Statistics` with honest `Precision::{Exact, Inexact, Absent}`
  per column (row counts are cheaply exact for materialized overlay batches;
  Delta-backed tables report what the snapshot's Add-file stats support), and
  by per-predicate `TableProviderFilterPushDown` declarations under
  `scan_with_args`/`ScanArgs` (`ALIGN A.1`; `DFREF §18`, `§51`, `§47`).
  `ServingRuntimeEvidence` then records *observed* pruning and repartitioning
  from `EXPLAIN ANALYZE` metrics instead of reading configuration back to
  itself.
- **Constraints verified, not just installed.** `validate_open_table` checks
  `delta.constraints.*` against the generated spec on every serving open, and
  constraint installation moves after table authentication (DP-020). The
  CHECK-constraint claim becomes a verified property of the opened table.
- **Metadata reclassified per the five-way taxonomy.** Every schema-IR
  annotation is classified enforced / planner-consumed / contractual /
  lineage / advisory with a named consumer (`GOV-07`; `DFREF S7`). Concretely:
  `foreign_key` is promoted to enforced — the cross-table referential validator
  in the merged R5 pipeline reads it (14 annotations, today read by nothing;
  DP-021) — and the SQLite generator either emits `REFERENCES` clauses or the
  `foreign_keys` pragma is removed with the decision recorded. `semantic_type`
  strings are validated against the enum registry with a digest link between
  the schema IR and the registry (the ~14 unresolvable of 28 become build
  failures; DP-043). `fact_evidence.fact_form_code` gains its registry binding
  and one sourcing rule (DP-025). `TableSpec::dependencies` and
  `zorder_columns` either gain consumers (publication ordering; Delta table
  properties written and verified) or are removed as metadata theater.
- **IR-owned values emitted, not re-typed.** `ontology_version` and
  `compatibility_mode` are emitted by the schema driver into the runtime
  constants that currently hard-code them (DP-034); row encoders are generated
  from the same IR that generates the specs they are hand-checked against
  (DP-038), eliminating the ~161 hand-written column tuples as drift surface.
- **Evolution policy declared.** The de-facto "no evolution" pin —
  `enableTypeWidening=false`, digest-equality hard reject, no acceptance suite
  (DP-037) — becomes a stated contract: a schema-evolution policy artifact
  declaring the compatibility classes accepted (currently: exact-pin), the
  migration route when the pin must move, and the generated compatibility
  acceptance suite `schema-validation.json` falsely gestures at.
  `Schema::try_merge`/`contains` are the mechanisms; the policy is the
  authority (`SCH-04`; `DFREF S5`, `S6`). `FAB` App. C inv. 11's "explicit and
  versioned" requirement is met by making the pin explicit and versioned.
- **Domain identity as extension types.** Id16 identity columns adopt a custom
  Arrow `ExtensionType` (`codefabric.id16` over `FixedSizeBinary(16)`), using
  Arrow 59's extension-type API (`ALIGN A.2`; `DFREF S7`) — the logical type
  survives IPC/Parquet boundaries and unknown consumers degrade to the valid
  storage type. Classified contractual, consumer: the ingest validators and
  the adapter's ID rendering.
- **Instance validation.** The eight public JSON schemas gain instance
  validation — golden envelopes validated against `planspec.schema.json` and
  siblings in CI, replacing key-set comparison (DP-063 ◐ with R9).

**Closes.** DP-019, DP-020, DP-021, DP-025, DP-034, DP-037, DP-038, DP-043,
DP-063 ◐; with R1's normalization fix, DP-005.

**Evidence.** Adversarial pushdown-truth tests (claimed-exact predicates
falsified with boundary rows — `TST-03`); constraint-drop detection tests
(mutate a table's constraints out-of-band, assert serving open rejects);
schema-compatibility classification matrix tests (`TST-01`); a metadata
dictionary oracle asserting every non-advisory key names a consumer that
exists (`GOV-07` evidence).

---

### R7 — Lifecycle phases, one error vocabulary, and guard truth

**Intent.** Failures identify their phase; public error identity is registry
membership, not a `Display` substring; state-machine guards are evaluated, not
merely legal. (`PRIN P16`, `P20`; `ALIGN` P16; `LifecycleArtifactMap` with
phase-scoped failure codes; `DFREF §33`, `§41`.)

**Mechanism.**

- **Phase on every error.** A generated `Phase` enum (from the lifecycle /
  state-machine registries) becomes a structural field of the fabric error
  types — the `ALIGN` §17 failure-code scheme (`schema_binding`,
  `logical_planning`, `policy_validation`, `physical_planning`, `execution`,
  `write_validation`, `commit`, …). The 439-variant, zero-phase census (DP-016,
  DP-096) is retired not by editing 439 variants but by making phase a
  property of the error *envelope* every subsystem returns; `String`-typed
  error plumbing in the extractor wrapper gains a real error type.
- **Registry-closed public identity.** Every `CODE:`-prefixed public error is
  a member of `PUBLIC_ERROR_IDS`, enforced by the DP-117 detector promoted to
  a gate; the shadow vocabularies (`LIFECYCLE_*`, `CONTINUOUS_*`,
  `DERIVATION_*`) are registered or renamed to registered codes, and the
  registered codes naming exact conditions (`QUERY_HARD_LIMIT_EXCEEDED`,
  `SEMANTIC_PHRASE_*`, `INVALID_REQUEST_SCHEMA`) are raised where those
  conditions occur (DP-017, DP-117). The error registry's `grpc_status`,
  `severity`, `retryability`, and `mcp_mapping` columns are generated into the
  Rust projection and consumed at the RPC boundary (DP-018), collapsing the
  ten independent code vocabularies to one generated one plus declared
  domain-local diagnostics.
- **Traces that survive failure.** Stage traces are recorded incrementally —
  each stage appends before the fallible call, so a failure names its phase
  (DP-015; same shape in `snapshot_catalog` and `snapshot_runtime`).
- **Observability that observed.** Daemon shutdown logs each step *after* it
  completes and reports only completed steps in `DaemonExit` (DP-014).
- **Guard truth.** `generated_transition` verifies legality; the continuous
  engine additionally *evaluates* guards before asserting them — a wave
  containing files whose language requires semantic capability cannot declare
  `semantic-work-not-applicable`; the state registry's opposite branch is taken
  and the wave parks in the explicit not-yet-terminal state (DP-079). This is
  the wave-level enforcement of `RM §1` inv. 6 (absence is never proof).

**Closes.** DP-014, DP-015, DP-016, DP-017, DP-018, DP-079, DP-096, DP-117.

**Evidence.** Phase-injection tests failing each lifecycle phase and asserting
the reported phase (`MOD-05` evidence; `ALIGN` P16); the error-registry
closure oracle in `governance`; a guard-falsification test constructing a
Rust-bearing wave and asserting it cannot reach `required-capabilities-terminal`
without the semantic lane.

---

### R8 — One boundary family and a strictly-presentational adapter

**Intent.** The daemon's public surface is one contract family with generated
projections both languages consume; the Python adapter presents daemon state
and never re-derives it. (`PRIN P22`, `P23`; `SRV §6` inv. 3/6/8/9/10/11,
`RM §1` inv. 8; `ALIGN` P22; patterns `INT-10`, `GOV-06`.)

**Mechanism.**

- **Proto as the wire authority.** The three disjoint families (DP-048)
  reduce: the adapter-model IR derives from the proto contract (or is retired
  where the proto covers it); the admin socket's newline-JSON protocol gains a
  schema artifact under `contracts/`; peer-UID policy has one implementation
  (the interceptor). `contracts/rpc/feature-registry.yaml` is generated into
  typed masks in both languages, `negotiate_feature_bits` gains its production
  caller in the handshake, and `required` semantics are enforced (DP-066).
- **Token and lease honesty.** `opaque_bytes` uses the keyed construction the
  same file already demonstrates (`new_keyed` from the urandom-seeded lease
  secret), cancel tokens are minted distinct from resume tokens per the proto's
  own separation, and `stream_query` authorizes the workspace like its
  siblings (DP-078, DP-087). `lease_expires_at_unix_ms` is compared to now in
  `read_result`; `permission_claims` are consumed by `authorize_workspace` or
  removed from the claim struct (DP-081). Resource existence is
  daemon-authoritative: the adapter's `_leased_artifacts` dict becomes a cache
  over a daemon lease-status call, never the decider (DP-090).
- **The adapter stops interpreting.** `server.py`'s hardcoded
  `COMPLETE`/ternary-collapsed states (DP-088) are replaced by pass-through of
  the daemon's registry values — the generated Python registries (R1) give the
  adapter the full vocabulary, so no narrowing is needed. The client's
  freshness-policy string mapping and double transport of
  `semantic_request_id` (DP-089) collapse to sending the canonical request
  bytes with typed fields generated from the same contract.
  `canonical_error_records_json` is presented as structured
  code/path/diagnostic data, preserving the daemon's error identity (DP-091).
  The unregistered `cpg://reference/...` URI branch and the unbounded
  `while True` re-issue loop are fixed with a registered template and a
  bounded retry contract (DP-092).
- **Host profile digest specified.** The handshake's host capability digest
  gets a derivation rule in `contracts/` and daemon-side validation — or is
  removed; an unvalidated, unspecified compatibility digest is capability
  theater (DP-093).
- **Settings and rules.** `Settings` becomes one instance per process with an
  oracle (DP-072); `no-framework-internal-contract-imports` covers `mcp.*` and
  the `mcp` dependency is declared (DP-070); Python governance rules cover
  `server.py`, `settings.py`, `__main__.py`, `channel.py` (with R9's rule
  work, DP-071 ◐).
- **The contract suite drives production.** The adapter test suite's probe
  server is replaced by the production `mcp` object end-to-end against real
  stubs, and the tool-manifest fingerprint gains an accepted baseline
  (DP-064; DP-099's monkeypatched client tests are superseded by stub-backed
  protocol tests).

**Closes.** DP-048, DP-064, DP-065 (schemas consumed at the boundary they
describe), DP-066, DP-067 (single decode contract + differential test),
DP-070, DP-072, DP-078, DP-081, DP-087, DP-088, DP-089, DP-090, DP-091,
DP-092, DP-093.

**Evidence.** Cross-language wire KATs from shared fixtures on every RPC
(`TST-09`-shaped, replacing the tautological encode→decode test); forgery
tests proving unkeyed-token derivation no longer works; adapter
state-fidelity tests asserting the MCP surface reports exactly the daemon's
enums for forced non-success outcomes (`SRV §6` inv. 6/8/9 as oracles);
expiry-enforcement tests.

---

### R9 — Contract-derived proof: Gate B, golden answers, convergence, and parity

**Intent.** The proving layer catches what the register caught: oracles prove
their acceptance sentences, golden answers are outputs rather than
descriptors, and the three end-to-end claims (Gate B, rebuild equivalence,
gix parity) execute. (`PRIN P25`; `ALIGN` P25; patterns `TST-01`–`TST-14`.)

**Mechanism.**

- **Golden answers, executed.** The corpus's `expected/` plane is populated
  with real released outputs — IDs, rows, response bytes, checksums — and the
  Gate B check runs the eleven `SUITE` items end-to-end through the activated
  vertical (R2), comparing produced artifacts to released answers (DP-101).
  The 16 edit scenarios are executed by a scenario runner that deserializes
  `scenario.json`, applies the named edits through the watcher/wave path,
  and asserts the terminal state (DP-102); the missing scenario classes
  (overflow, multi-file logical save, context change, capability withdrawal)
  are added. Corpus manifest digest fields are verified, `corpus_status`
  gates acceptance, and owner acceptance references a digest computed from
  reviewed answers rather than itself (DP-113, DP-114, DP-116, DP-082).
  Registry-derived expectations import generated constants and use equality,
  not `is_subset` (DP-121 ◐ with R1).
- **Convergence proved.** The AC-G-79 comparator performs a true clean
  rebuild — re-walk inventory, re-capture bytes, reconcile from zero — and
  compares *effective state* (durable base − tombstones + overlay rows) via
  `CanonicalState::from_serving_session`, which gains its production caller
  (DP-100, DP-115). `comparison-ignore-registry.yaml` is read by the
  comparator. DataFusion is the natural comparator engine: both sides
  materialize as `RecordBatch` streams over the serving session and are
  diffed with set-difference queries — the fabric proving the fabric.
- **Parity proved.** `git-parity-check` constructs the authoritative
  `InventoryWalker` fallback and compares accelerated vs. authoritative
  candidates; gix-disabled / cache-disabled / full-rebuild configurations run
  the WP48 comparator corpus (DP-103).
- **Oracle substance.** Alias oracles (`fn X() { Y(); }`) are rejected by a
  governance detector, and the five aliases are replaced with tests meeting
  their packets' acceptance sentences (DP-104). The oracle-catalog validator
  additionally requires per-oracle acceptance-criterion references, and the
  wave/gate recipes are wired into `ci-fast`/`ci-pr`/CI so green means ran
  (DP-061, DP-108). The CI step naming a nonexistent test either gets the
  test — a Rust decode of the shared wire fixture — or is removed (DP-057,
  with `--no-tests=fail` on every selector).
- **Contracts named by tests.** New and touched oracles carry `AC-G-NN`
  references (test-name or attribute-level), burning down DP-058/DP-099;
  the source-text oracle census (DP-062) is replaced by structural ast-grep
  rules where a policy already has one, and the remainder are rewritten
  against decoded artifacts (the DP-062 anchors list is the worklist).
- **Fixtures consumed.** The five negative fixtures gain their consumers
  (the released verifier the changelog already claims), the zero-digest
  released manifest is re-released with a real digest (DP-059, DP-060), and
  the security-corpus / fault-point / comparison-ignore registries are
  executed by the suites that cite them, with the fault-registry census
  reconciled to the code's fault points (DP-068).
- **Rule-set restoration.** The `authoritative-source-read-boundary` ignores
  widened this cycle are re-narrowed (the new modules route reads through
  `secure_path` instead of being exempted), `provider-observation-boundary-only`
  is re-scoped to real paths, snapshot tests are enabled (`__snapshots__`
  created, `--skip-snapshot-tests` dropped), and Python rules are added
  (DP-071).
- **Register hygiene as standing policy.** Whole-repo detectors exclude
  `docs/reviews/**` (DP-124); the process findings — one commit closing 22
  packets (DP-106), contradictory stale status artifacts (DP-107) — are
  addressed by superseding the stale status report and by the impl-plan for
  this proposal requiring per-packet proving commits, which the existing
  `plan-status` ancestry checks then enforce.

**55/59 leverage.** DataFusion as the comparator engine for rebuild
equivalence; `EXPLAIN ANALYZE` metrics as Gate B evidence; sqllogictest-style
KATs for the serving plane (`DFREF §32`) as the durable form of the
serving-query golden tests.

**Closes.** DP-057, DP-058, DP-059, DP-060, DP-061, DP-062, DP-063 ◐,
DP-068, DP-071, DP-082, DP-099, DP-100, DP-101, DP-102, DP-103, DP-104,
DP-105 ◐, DP-106 (process), DP-107, DP-108, DP-113, DP-114, DP-115, DP-116,
DP-124.

**Evidence.** This move *is* evidence; its own oracle is the register's
detector suite re-run green at the proving commit, plus mutation-testing spot
checks (`just mutants-file`) on the comparator and scenario runner to prove
the new oracles can fail.

---

### R10 — Model-compiler and governance-plane authority repairs

**Intent.** The model compiler's own plane obeys P3/P2: derived artifacts are
produced by something, checks can fail, vocabularies have one registration,
and the governance layer's self-description is true. (`PRIN P2`, `P3`, `P21`;
`ALIGN` P1–P3 application overlays.)

**Mechanism.**

- **The shadow plan dies.** `desired_tree` derives desired bytes from the
  model, not from current outputs, so `ModelPlan::check` can fail; the action
  graph, keys, and `explain` output report the real transaction's plan
  (DP-004), and the two action-key schemes merge into the one the render
  cache uses (DP-046).
- **Census and stale-derivation truth.** `model_outputs()` is computed after
  governance views, manifests, and validation are inserted (DP-006); Derived
  claims drive stale-deletion so unproduced Derived files are regenerated or
  deleted — including the 17 unproduced registry projections and the
  provably-stale arrow-delta copy (DP-007).
- **Traceability with normative content.** The requirements/traceability
  outputs stop being byte-identical templates: the `AC-G` corpus enters the
  model as parsed normative records, `verified_by` names real oracles (R9's
  contract-named tests), and the two files carry their distinct meanings
  (DP-008). The declared consumer graph names real consumers or the field is
  removed (DP-069); adapter-generated artifacts get `authority_path` coverage.
- **One JCS.** The drivers call the repository's own strict
  `codefabric-jcs-v1` profile instead of raw `serde_json_canonicalizer` plus
  ad-hoc duplicate-key rejection (DP-044).
- **Typed facts stay typed.** The operational store consumes the generated
  typed table specs instead of re-parsing generated SQL text, and the
  hand-written migration DDL is emitted from the same IR (DP-041). The dead
  31 KB contract-model vocabulary is either adopted by the drivers that
  shadow it privately or deleted (DP-045). `derivation.rs` treats the registry
  as authority — expected values come from the contract fixture, not a
  hardcoded second copy (DP-097) — and the derivation registry is populated so
  bundle membership and derivation facts are records, not path-string
  matching (DP-042).
- **Single registration, compared.** The review-artifact vocabulary is
  registered in the documented table and the validator, with a comparison
  oracle asserting they match (DP-074 — the table row and this document's own
  registration land together; the oracle prevents recurrence). The untracked
  `skills/` duplicate is removed or made a symlink, with the register's
  detector as a seed-zero-state check (DP-094).
- **Probes reclassified.** `compatibility.rs` is documented as a
  library-probe tier distinct from CodeFabric contracts, with its gate role
  stated (DP-073 — observation, accepted with documentation).

**Closes.** DP-004, DP-006, DP-007, DP-008, DP-041, DP-042, DP-044, DP-045,
DP-046, DP-069, DP-073, DP-074, DP-094, DP-097.

**Evidence.** A mutation test proving `model-plan` reports a non-empty change
set when an output is perturbed (the DP-004 inversion); a census oracle
(`suite manifest count == committed tree count == validation count`); the
DP-074 comparison oracle; the DP-094 symlink detector in `seed-zero-state`.

---

## 4. Finding-by-finding disposition

All 124 findings. **Move** names the primary owner (◐ = shared, second move in
parentheses). *spec* = additionally routed to the owning specification per §6.
*process* = resolved by process rule rather than code.

| Finding | Sev | Move | Resolution in one line |
|---|---|---|---|
| DP-001 | blocker | R1 | governed `identity.py`, role match fixed; cross-language identity KAT |
| DP-002 | blocker | R1 | registry→proto enum emission + cross-check oracle |
| DP-003 | blocker | R1 | imported Python registry becomes the governed one; orphan deleted |
| DP-004 | major | R10 | desired tree derived from model; check can fail |
| DP-005 | major | R1 ◐(R6) | recipes emit `ASCII_LOWER`; generated path honors cbef-v1 |
| DP-006 | major | R10 | census computed after all outputs inserted |
| DP-007 | major | R10 | Derived claims drive stale deletion; stale copies regenerated/removed |
| DP-008 | major | R10 | AC-G corpus parsed into real requirements; files distinct |
| DP-009 | major | R5 | real adapters implement `ProviderAdapter` |
| DP-010 | major | R5 | registry-driven provider collection replaces fixed signature |
| DP-011 | major | R5 | one cancellation type end-to-end |
| DP-012 | blocker | R3 | canonical order (or commutative digest); checksum contract defined |
| DP-013 | blocker | R4 | resolved; residual reason codes + advertised statuses completed |
| DP-014 | major | R7 | shutdown logs after completion; exit reports observed steps |
| DP-015 | major | R7 | incremental stage traces survive failure |
| DP-016 | major | R7 | phase field on the error envelope |
| DP-017 | major | R7 | public codes registry-closed; gate oracle |
| DP-018 | major | R7 | error-registry projections generated and consumed |
| DP-019 | minor | R6 | honest `Statistics`/`Precision`; observed (not echoed) evidence |
| DP-020 | minor | R6 | constraints verified at open, installed after authentication |
| DP-021 | blocker | R6 | FK promoted to enforced cross-table validation; SQL decision recorded |
| DP-022 | blocker | R4 | continuous engine writes wave tables; `wave_id` Id16-validated |
| DP-023 | blocker | R4 | `owner_id` encoded Id16; join restored |
| DP-024 | major | R5 ◐(R4) | true producer + `derivation_code` carried from call sites |
| DP-025 | major | R6 | `fact_form_code` registry-bound; one sourcing rule |
| DP-026 | major | R4 | derived relations get evidence rows |
| DP-027 | major | R4 | retention closure includes provenance tables |
| DP-028 | major | R5 | Arrow IPC sole channel with real decoder |
| DP-029 | major | R5 | schema-driven coverage; enum family retired |
| DP-030 | major | R5 | one ingest pipeline above the shared validator |
| DP-031 | minor | R1 | evidence identity from the fingerprint registry |
| DP-032 | major | R5 | provider facts carried as batch columns |
| DP-033 | major | R5 | generated field-role crosswalk registry |
| DP-034 | major | R6 | IR emits `ontology_version` / `compatibility_mode` |
| DP-035 | major | R4 | control capture stamped + pinned in the artifact |
| DP-036 | major | R4 | `QueryPlanArtifact` persisted to its lease table |
| DP-037 | major | R6 | evolution policy declared, versioned, with acceptance suite |
| DP-038 | major | R6 | row encoders generated from the schema IR |
| DP-039 | major | R1 | hand-written `NewlineKind` deleted; generated enum used |
| DP-040 | major | R1 | one generated registry module |
| DP-041 | minor | R10 | operational store consumes typed specs, not re-parsed SQL |
| DP-042 | major | R10 ◐(R1) | derivation registry populated; membership declarative |
| DP-043 | minor | R6 | `semantic_type` validated against registry with digest link |
| DP-044 | minor | R10 ◐(R1) | drivers use `codefabric-jcs-v1`; one `digest_bytes` |
| DP-045 | minor | R10 | contract models adopted by their drivers or deleted |
| DP-046 | minor | R10 | one action-key scheme |
| DP-047 | minor | R5 | domain DTO at the seam; prost confined to rpc adapter |
| DP-048 | major | R8 | one boundary family; admin protocol gets a schema |
| DP-049 | minor | R5 | admission eviction; ownership doc made true |
| DP-050 | major | R4 | diagnostics written to the diagnostic table |
| DP-051 | major | R4 *spec* | location-identity + pin declared as contract; routed to `GEN §13` owner |
| DP-052 | major | R4 | `source_blob_digests` persisted in the manifest body |
| DP-053 | major | R4 | request/execution identity threaded through Rust |
| DP-054 | major | R4 | typed scope columns; preimages stored or fields re-documented |
| DP-055 | major | R4 | `explain_version` surface over history + artifacts |
| DP-056 | major | R4 | all bundle IDs + overlay identity pinned in the artifact |
| DP-057 | blocker | R9 | Rust shared-fixture decode test exists and runs; `--no-tests=fail` |
| DP-058 | blocker | R9 | oracles carry `AC-G-NN` references |
| DP-059 | major | R9 | real digest or unreleased status |
| DP-060 | major | R9 | negative fixtures consumed by the released verifier |
| DP-061 | major | R9 | wave/gate recipes wired into gates and CI |
| DP-062 | major | R9 | source-text oracles replaced by structural rules / decoded artifacts |
| DP-063 | major | R6 ◐(R9) | instance validation against the public schemas |
| DP-064 | major | R8 | contract suite drives the production server; fingerprint baseline |
| DP-065 | major | R8 | generated MCP schemas validate the boundary they describe |
| DP-066 | major | R8 | feature registry projected; negotiation wired with `required` semantics |
| DP-067 | major | R8 | one decode contract + differential test |
| DP-068 | major | R9 | security/fault/comparison registries executed; census reconciled |
| DP-069 | major | R10 | consumer graph real or field removed; adapter artifacts indexed |
| DP-070 | minor | R8 | rule covers `mcp.*`; dependency declared |
| DP-071 | major | R9 | ignores re-narrowed; snapshot tests on; Python rules added |
| DP-072 | minor | R8 | one `Settings` instance per process, with oracle |
| DP-073 | minor | R10 | probes documented as a distinct tier (accepted observation) |
| DP-074 | minor | R10 | both authorities registered + comparison oracle (this artifact complies) |
| DP-075 | blocker | R2 | query service reachable from `daemon::serve` |
| DP-076 | blocker | R1 | bundle digest read from the installed bundle |
| DP-077 | blocker | R2 | `limit_state` computed from fetch vs. produced |
| DP-078 | blocker | R8 | keyed tokens; distinct cancel token; stream authorization |
| DP-079 | blocker | R7 | guards evaluated before transition assertion |
| DP-080 | major | R2 | runtime states computed; live freshness on success path |
| DP-081 | major | R8 | lease expiry enforced; permission claims consumed |
| DP-082 | major | R9 | corpus executes against system output |
| DP-083 | major | R1 | generated into both roots + byte-equality oracle |
| DP-084 | major | R5 | `--extract-json` deleted; gRPC path tested |
| DP-085 | major | R1 | wire codes from registry records |
| DP-086 | major | R1 | one capability-scope fingerprint in the domain registry |
| DP-087 | major | R8 | cancel token honored per proto separation |
| DP-088 | blocker | R8 | adapter passes daemon states through unmodified |
| DP-089 | major | R8 | one request encoding; adapter stops interpreting |
| DP-090 | major | R8 | lease existence daemon-authoritative |
| DP-091 | major | R8 | structured error records preserved to MCP |
| DP-092 | minor | R8 | registered URI template; bounded retry |
| DP-093 | major | R8 | host profile digest specified + validated, or removed |
| DP-094 | major | R10 | `skills/` removed/symlinked; detector in seed-zero-state |
| DP-095 | major | R2 ◐(R1) | state literals replaced by generated enums |
| DP-096 | major | R7 | new modules on the phase-carrying envelope; wrapper gets an error type |
| DP-097 | minor | R10 | registry is the authority; expectations from the contract |
| DP-098 | major | R2 | Arrow-native results; typed projection; `encode_public_id` |
| DP-099 | major | R9 | contract-named tests; stub-backed client tests |
| DP-100 | blocker | R9 | true clean rebuild + effective-state comparison (AC-G-79 §79.2) |
| DP-101 | blocker | R9 | Gate B executes end-to-end against released answers |
| DP-102 | blocker | R9 | scenario runner executes all scenarios + missing classes |
| DP-103 | blocker | R9 | gix-disabled / cache-disabled / full-rebuild comparisons implemented |
| DP-104 | blocker | R9 | alias oracles rejected; real acceptance tests |
| DP-105 | major | R2 ◐(R4) | coverage returned through the status surface |
| DP-106 | major | R9 *process* | per-packet proving commits required by the follow-on plan |
| DP-107 | minor | R9 *process* | stale status artifact superseded |
| DP-108 | major | R9 | oracle-substance validation; recipes actually run |
| DP-109 | blocker | R2 | freshness reading consulted on success |
| DP-110 | blocker | R2 | states typed as registry enums |
| DP-111 | major | R2 | limits digest hashes the limits |
| DP-112 | major | R2 ◐(R7) | production writer for `Unavailable` or claim withdrawn |
| DP-113 | major | R9 | manifest digest fields verified; `corpus_status` gates |
| DP-114 | major | R9 | acceptance digest independent of the thing accepted |
| DP-115 | major | R9 | `from_serving_session` gains its production caller |
| DP-116 | major | R9 | golden digests independently derived |
| DP-117 | major | R7 | one registered error vocabulary; closure oracle |
| DP-118 | major | R1 | generated `FreshnessState` the only declaration |
| DP-119 | major | R1 | one guarded fingerprint implementation |
| DP-120 | major | R1 | fingerprint-domain registry; construction confined to `identity` |
| DP-121 | major | R9 ◐(R1) | corpus imports generated constants; equality not subset |
| DP-122 | minor | R1 | query-form vocabulary becomes a registry domain |
| DP-123 | minor | R2 | advertised filters compiled to `Expr`, or removed from schema |
| DP-124 | major | R9 *process* | detectors exclude `docs/reviews/**` as standing rule |

Coverage check: 124 findings, 124 dispositions; every blocker maps to a move
with named evidence.

---

## 5. Principle end-state matrix

Register status is the pass-3 §5 row; target is what the program of record
achieves. "Enforced" means a named executable oracle would catch regression
(`artifact-schemas.md §6` standard).

| # | Principle | Register (p3) | Target | Enforced by |
|---|---|---|---|---|
| P1 | Model semantics first | partial | enforced | R1/R2 registry+spec compilers; governance rules |
| P2 | Models executable | conflict (eased) | enforced | R1/R6/R10 generation with can-fail checks |
| P3 | One authority | conflict (dominant) | enforced | R1 registries, digest census, DP-074-style comparison oracles |
| P4 | Explicit hierarchies | conflict | conformant | R5 real `ProviderAdapter` impls |
| P5 | Variability behind contracts | conflict | enforced | R5 substitution test as oracle |
| P6 | Semantic vs. execution | conflict (regressed) | enforced | R2 `LogicalPlan` pipeline; no SQL text on the path |
| P7 | Shared canonical fabric | conformant | conformant | unchanged; R5 extends it to the provider wire |
| P8 | Common representation as infrastructure | conflict | enforced | R5 single Arrow channel; decoder KATs |
| P9 | Provenance intrinsic | conflict | enforced | R4 by-construction artifact + identity |
| P10 | Provenance closure | conflict | enforced | R4 closure-traversal oracle |
| P11 | Immutable snapshots | conformant-and-enforced | conformant-and-enforced | strengthened by R7 guard truth (DP-079, DP-027) |
| P12 | Schemas executable contracts | conflict | enforced | R6 declared evolution policy; verified constraints |
| P13 | Governance at the boundary | conformant (in scope) | conformant | R8 keyed tokens, enforced leases; divergences stay closed |
| P14 | Highest-level extension | unowned | conformant | §6 routing; this proposal's own extension discipline (§1.3) |
| P15 | Optimizer visibility | n/a — scheduled | conformant | R6 truthful pushdown/statistics when claimed; no UDFs |
| P16 | Lifecycle phases | conflict (worse) | enforced | R7 phase envelope + phase-injection tests |
| P17 | Artifacts inspectable | conflict | enforced | R4 persisted bundle; failure-path bundle tests |
| P18 | Fingerprint identity | partial | enforced | R1 domain registry; R3 plan fingerprints |
| P19 | Reproducibility normal | conflict | enforced | R3 determinism harness + modeled status |
| P20 | Conservative claims | conflict (worst) | enforced | R2/R6 computed states + adversarial truth tests |
| P21 | Enforced vs. advisory metadata | conflict | enforced | R6 metadata dictionary oracle |
| P22 | Protocols and canonical boundaries | conflict | enforced | R5/R8 single families + wire KATs |
| P23 | State ownership explicit | partial | conformant | R4/R5/R8 versioned caches, eviction, daemon authority |
| P24 | Semantic observability | conflict | enforced | R4 artifact + explain surface; R7 honest shutdown |
| P25 | Tests from contracts | conflict (worst) | enforced | R9 in full |

---

## 6. Unowned principles — mechanisms here, normative homes routed

Per `RM §28` and the register's §7, the five unowned principles are not
resolved by implementation alone. This proposal supplies the mechanism; the
normative sentence belongs in the owning specification:

| Principle | Mechanism in this proposal | Normative home to amend |
|---|---|---|
| P2 — one model, multiple derived operations | R1/R10: registries and schema IR each emit ≥2 derived operations with drift oracles | `SUITE` (AC-G-05) |
| P10 — provenance closure as a resolvable chain | R4: closure-traversal oracle from any durable result | `FAB` / `LIFE` |
| P14 — extension-level preference ladder | §1.3 posture; `ALIGN §2.3` adopted as the repository's `ExtensionDecisionRecord` requirement | `FAB` / `QRY` |
| P19 — modeled reproducibility status | R3: `Reproducibility` record in the artifact | `LIFE` (AC-G-79) |
| P21 — five-way metadata taxonomy | R6: metadata dictionary with named consumers | `FAB` (AC-G-20) |

DP-051's identity question (location vs. fact identity for `file_id`) routes
to `GEN §13` with R4's persisted-pin contract as the proposed answer.

---

## 7. Sequencing and replan triggers

Coarse dependency order for the follow-on `impl-plan` (not packetization):

```text
R1 identity/registry substrate        ── everything downstream imports it
   ├─ R6 schema-contract truth        ── needs generated encoders + digest links
   ├─ R5 one fact protocol            ── needs registries + crosswalk registry
   └─ R7 error/phase vocabulary       ── needs registry projections
R2 compiled query plane + activation  ── needs R1 enums; enables end-to-end proof
   └─ R3 determinism + reproducibility
R4 provenance closure                 ── needs R2 identity threading, R5 join fixes
R8 boundary family + thin adapter     ── needs R1 Python registries, R7 error projections
R9 contract-derived proof             ── needs the activated vertical (R2) and R4/R5 outputs;
                                         its detector-suite oracle gates completion
R10 model-plane repairs               ── independent; schedulable in parallel with R1
```

Replan triggers (standing, per plan-audit policy): a pin move off
55.0.0/59.2.0/43a0cf10; a `docs/upfront_design/` revision touching `QRY`
request forms, `FAB` App. C invariants, or `AC-G-79`; any R2 discovery that a
request form cannot be expressed in built-in logical nodes (which would
re-open the `LOG-08` extension decision this proposal declines).

## 8. Standing governance additions

New oracles this program leaves behind, so the register's next pass is
mechanical (all are inversions of register detectors):

1. Digest-domain census: domains in use ⊆ fingerprint registry (DP-120).
2. Public-error closure: `#[error]` code prefixes ⊆ `PUBLIC_ERROR_IDS` (DP-117).
3. Alias-oracle detector: no `#[test]` whose body is a single call to another
   test (DP-104).
4. Artifact-vocabulary comparison: `artifact-schemas.md §7` table ==
   `REVIEW_REQUIREMENTS` keys (DP-074).
5. Skills-tree shape: `skills/` absent or a symlink to `.claude/skills` (DP-094).
6. State-literal ban: no string literal from a registered state domain outside
   generated code (DP-095/DP-110 detector as an ast-grep rule).
7. Determinism harness across `target_partitions` (DP-012).
8. Review-register detector hygiene: `--glob '!docs/reviews/**'` (DP-124).
9. Gate selectors run with `--no-tests=fail` and are reachable from a gate
   (DP-057, DP-061).

## 9. Explicit non-goals and preserved divergences

- **The divergence ledger stands.** No tenancy predicates or multi-tenant
  policy model (`SRV §6` inv. 4); no classification/retention/masking
  metadata (`FAB` App. D); no advisory display-name channel (`AC-G-54`); no
  user-facing `Expr` transparency (`SRV §6` inv. 5). `ALIGN`'s GOV-02/GOV-05
  tenancy patterns are deliberately not selected.
- **No new extension levels.** No custom `ExecutionPlan`, `PhysicalExpr`,
  planner, UDF, or `LogicalPlan::Extension` (`EXT-04`–`EXT-10` unselected);
  the P15 register row moves from "n/a" to conformant precisely because
  nothing opaque is introduced.
- **No new Cargo roots, no `crates/`,** and no module decomposition mandates —
  the scope boundary of `AGENTS.md §1` is untouched; moves name behaviors and
  contracts, not file layouts.
- **No Substrait/Flight adoption.** The process boundaries remain
  UDS gRPC + Arrow IPC (`INT-01`, `INT-08`); `INT-05`/`INT-07` are recorded as
  reviewed-and-declined for a single-host daemon (`ALIGN` P22: protocol per
  boundary semantics, not capability availability).
- **Delta writes stay behind the existing boundary.** `MERGE INTO` /
  `TableProvider::merge_into` (`ALIGN A.1`) is noted for the overlay
  publication path but not selected; the current mutation boundary satisfies
  P11 and is the register's strongest row.
