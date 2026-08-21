---
artifact: implementation-plan
plan_id: codefabric-waves-0-3-foundation
version: v5
date: 2026-08-20
status: approved
design_path: docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md
design_version: v1.0
design_correction_path: docs/designs/codefabric_local_storage_dependency_isolation_design_v1_2026-08-20.md
design_correction_version: v1
predecessor_plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md
remediation_plan_path: docs/plans/codefabric_model_based_foundation_remediation_implementation_plan_v1_2026-08-20.md
status_review_path: docs/reviews/implementation_status_codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20_2026-08-20_v1.md
implementation_review_path: docs/reviews/implementation_review_codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20_2026-08-20_v2.md
baseline_commit: ca7ce57c12bfcf2a3208875230bc9f5ecc6ddb2e
state_path: docs/plans/state/codefabric-waves-0-3-foundation_v5_state.json
cutover: true
---

# CodeFabric Waves 0–3 Foundation — Implementation Plan v5

This plan converts Waves 0–3 of the CodeFabric 1.3 implementation roadmap into
dependency-closed work packets. It covers:

- **Wave 0** — Program, toolchain, and build foundation (four build domains).
- **Wave 1** — Machine contracts, registries, and code generation (Gate A).
- **Wave 2** — Daemon kernel, workspace registry, path security, source images.
- **Wave 3** — Canonical data fabric, publication, overlay, snapshot kernel.

This successor preserves every stable v4 packet, milestone, and decommission ID while
integrating the completed model-based foundation remediation and the two accepted
post-remediation reviews. It is the sole executable continuation contract for Waves 0–3;
v4 and the remediation plan remain immutable provenance.

The current implementation contains reusable Wave-0 and contract-compiler substrate but
does not contain the Wave-1 production contracts or the Wave-2/3 product runtime. Before
WP07 resumes, WP00 reconciles the execution-state/proving-commit contract and WP06a
introduces a first-class typed compilation/derivation-unit graph so generated outputs are
attributed to their complete source set rather than to an arbitrary single artifact.
Ubuntu clean-checkout evidence is explicitly user-deferred assurance, not a packet blocker.
IR-010's tracked auxiliary build outputs have been removed from the current tree and
reachable branch history; WP00 makes that zero state a permanent repository gate.

---

## 1. Outcome and non-goals

### 1.1 Outcome

At M04, CodeFabric has:

1. Four reproducible and isolated build domains: the stable Rust root, dated-nightly
   rustc extractor, stable Pyrefly sidecar, and locked Python FastMCP adapter.
2. One closed typed contract catalog and Contract IR. Every governed source has distinct
   semantic and exact-source identity, and every generated output has provenance through a
   typed compilation unit and its complete resolved input set.
3. Released Wave-1 contracts: identity/path/type rules, populated registries, generated
   Arrow/Delta/SQLite/JSON/Pydantic contracts, four production Protobuf packages compiled
   through one FileDescriptorSet, populated bundles, deployment profile, and zero-orphan
   traceability. Readiness Gate A is green with zero draft warnings.
4. A secure Wave-2 daemon control plane for lifecycle, workspace registration, operational
   persistence, path authorization, immutable source images, Git topology, and bootstrap.
5. A Wave-3 canonical fact-state substrate for generated TableSpecs, validated Arrow
   batches, idempotent Delta mutation, durable publication, overlays, immutable snapshots,
   provider catalogs, and snapshot-pinned read-only queries.

### 1.2 Current-state continuation boundary

The baseline is the current implementation HEAD named in frontmatter. Its Wave-0 and
model-based compiler substrate are reusable but not automatically trusted as completed
packets because the two historical schema-1 states have null proving commits. V5 preserves
all v4 packet IDs and dependencies, adds WP00 for state/provenance reconciliation and
WP06a for compilation units, and treats current compatibility probes as evidence rather
than product completion.

The following current facts govern continuation:

- WP01–WP05 outcomes exist and are locally green but need schema-2 proof reconciliation.
- WP06 is behaviorally replaced by the stronger remediation implementation.
- WP07, WP08, and WP08b product content is not started.
- WP09, WP10, and WP11 have substrate only; their production contracts remain incomplete.
- WP12–WP26 product outcomes are not started.
- just contracts-verify-released currently reports 49 draft warnings; M02 remains open.
- Ubuntu clean-checkout evidence is user-deferred and is not a blocker.
- IR-010 is resolved at the current baseline; WP00/DB06 prevent reintroduction.
- License selection/evaluation remains outside active scope by prior user direction.

### 1.3 Non-goals

- Providers, reconciliation, watchers, public semantic-query runtime, and public FastMCP
  tools assigned to later roadmap waves.
- A universal schema replacing JSON Schema, YAML registries, EBNF, or Protobuf.
- A second implementation of daemon semantics in Python.
- A new Cargo package or workspace for conceptual organization.
- Another Protobuf compiler interpretation or an independent adapter schema authority.
- orjson adoption without a separate measured and bounded design decision.
- Generated fixtures approving or overwriting their own normative expected values.
- Clean-build timing, stored benchmark results in execution state, or performance claims
  without a stable workload and controlled comparison.

---

## 2. Source design and declared inputs

The roadmap remains the primary design path. SUITE AC-G-02, AC-G-05, and AC-G-07;
RM §§5–8, 27, and 28; SRV §§18, 19, 60, and 70; the remediation plan; and the two
accepted reviews provide the v5 corrections.

| Path | SHA-256 |
|---|---|
| docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md | 598b4971574c245cfd4f3f560ad52e2838eef884d21ea4c77c9233c70ad3d3db |
| docs/plans/codefabric_model_based_foundation_remediation_implementation_plan_v1_2026-08-20.md | 94ef42472e513676f4818908414bd4b4c7d38dac506ccbf6150dda3e968f0adf |
| docs/reviews/implementation_status_codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20_2026-08-20_v1.md | 52e11573eb89ebc947318014f56d443074f449b8e7348f3620e089994425d483 |
| docs/reviews/implementation_review_codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20_2026-08-20_v2.md | c9ec7f3642d77a5495783d82f8c5fe8612deb769e9d3c5b73a63ae99c1662e68 |
| docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json | e57909b1227fda8b339271e8c2d53d8a6099e3c7ca6057c4a1daf9e0bdfe1906 |
| docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json | 3c0c8c1d0fca9877903cb585f4e92e590d94c74ab381d008d3ea10a2f1c72e53 |
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | 087408e86b6dfde7ebb66624cafa58793f26860288861addbf9a0124f7647a78 |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | a90cea42b5f13092000e06326513f40f8365a2e1b44eec091aca47a413ce51f8 |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | a8ba008a94c72c55b48b91fd8967eafe6f46419f0fe5f65d1bcb3a874d79adab |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | ba3b33acaab3c4df9d6a47ec0b748ff57ac227bcf3ca0a8a0061d493539597ac |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 3ce756c018196ca677ab2ad4284b008e5da1b097b79c9e8f7006693ec53b4b4e |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | bf622f58354c7a11d5d9427663a9da6bd5d5e77fe1f3ca4e3fe249eb8f0ff151 |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | 6512e715c118fd7c72dbdc2d4ccce6828fafb6526f7e6db540f5ab587eb33847 |
| docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | 26fe78128357aebbca8d25b60bd5e533fd6dc9fbdb26d65a9d34387215b130e0 |
| docs/designs/codefabric_local_storage_dependency_isolation_design_v1_2026-08-20.md | 9251cfbc4fcd23db6858c0bf0ead1b05dc675833e90e62816f8cd3298b1e7745 |
| docs/designs/codefabric_build_cache_and_feature_isolation_design_v1_2026-08-20.md | 460e2f36a8a61a9972c976adbd20e1a86b82a26cd7bd38840cb1565948475e8f |
| docs/rust_core_python_interface_repository_specification_2026-08-20.md | 42678a93d6c323d3c527255c2f266b2520bd13dca485c1f1d4af7991a9243848 |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |

### 2.1 Precedence

1. Normative owner in the current design suite.
2. Accepted design correction recorded by the two reviews and this plan.
3. V5 packet contract.
4. Current repository implementation.
5. V4 and remediation illustrative mechanics.

Where v4 contradicts the accepted model-based design, v5 governs. Current repository and
pinned-library behavior remain higher authority than illustrative implementation detail.

---

## 3. Baseline and implementation disposition

Baseline HEAD is ca7ce57c12bfcf2a3208875230bc9f5ecc6ddb2e. The tree is clean, and the
two accepted reviews and this plan are committed at that baseline. just ci-fast
passed at this baseline: 37 root tests plus doctests, 3 extractor tests, 2 sidecar tests,
68 adapter tests, and the current governance/contract/proto proof surface.

Current status is judgment, not packet completion:

| Scope | V5 disposition |
|---|---|
| WP01–WP05 | Outcome present; reconcile proving commits in WP00, then re-run packet checks. |
| WP06 | Original mechanics superseded; corrected typed compiler substrate present. |
| WP07–WP08b | Product work not started. |
| WP09 | Contract-IR-to-Pydantic substrate present; production schemas/TableSpecs/DDL remain. |
| WP10 | Wave-0 one-FDS substrate present; four production protocols remain. |
| WP11 | Closed bundle model present; eight bundles remain empty/draft. |
| WP12–WP26 | Product work not started; compatibility probes are not completion. |
| M01 | Locally reconcilable after WP00; Ubuntu evidence deferred. |
| M02 | Open with 49 draft warnings. |
| M03–M04 | Not started. |

No v5 execution state is created by this planning task. The executor initializes the
schema-2 state at the path in frontmatter.

---

## 4. Global target invariants

- I-01: workspace_id identifies exactly one authorized analyzed source instance and is
  derived from a persisted registration nonce, never from a root path.
- I-02: one immutable leased ServingSnapshot is the only query pin; later activations
  cannot affect an existing lease.
- I-03: current stable filesystem bytes are present-state authority; CodeFabric BLAKE3
  content identity is canonical, not Git OIDs or watcher events.
- I-04: provider observations are not canonical until reconciled; providers and encoders
  never write canonical tables directly.
- I-05: every fact row is scoped by workspace and analysis context; exact facts never
  merge across either boundary.
- I-06: absence is never proof of absence; unknowns, capability gaps, and explicit
  negatives are registry-defined.
- I-07: Rust owns semantics, planning, storage, snapshots, and canonical bytes; Python is
  a thin adapter.
- I-08: every compatibility-sensitive artifact is versioned and fingerprinted; unchanged
  source regeneration is deterministic.
- I-09: incremental and overlay results converge to the corresponding clean durable state.
- I-10: compiler, Pyrefly, gix, delta-rs, and FastMCP internals do not cross
  application-owned boundaries.
- I-11: contracts machine sources and Contract IR are the sole generation authority; no
  downstream registry, identity, enum, status, or schema authority is re-declared.
- I-12: one Arrow/Parquet/DataFusion/object_store family crosses public boundaries.
- I-13: registry codes are append-only and orthogonal state dimensions remain distinct.
- I-14: storage mutation is idempotent and generation-fenced.
- I-15: the ontology contains facts and mechanically derived facts only, never evaluative
  classifications.
- I-16: local-workstation-v1 constructs local filesystem storage only; latent compiled
  cloud features grant no authority.
- I-17: every governed source has distinct canonical_digest and source_digest; bundles
  additionally use the AC-G-07 bundle_digest projection.
- I-18: closed typed models reject unknown fields; dynamic values are confined to named
  projection seams; ingress is staged and bounded; YAML anchors and aliases are rejected.
- I-19: language-neutral generated data exists once as canonical packaged resources;
  generated source is reserved for static exhaustiveness or required bindings.
- I-20: every generated output is owned by one typed compilation unit whose resolved
  inputs, roles, consumers, budget, and tool identity are explicit.
- I-21: one grpcio-tools invocation emits one FileDescriptorSet and Python bindings; Rust
  consumes that descriptor IR through compile_fds.
- I-22: Pydantic validation and serialization schemas are separate generated views, and
  FastMCP fingerprints the frozen client-visible surface.
- I-23: models, adapters, descriptor pools, channels, and stubs are lifecycle-owned and
  never rebuilt per request.
- I-24: normative KATs are independent and cannot be approved or overwritten by their
  production generator.
- I-25: complete packets have a non-null ancestor proving commit and execution state stores
  judgment only.
- I-26: every generated artifact, pass, mutation, snapshot, and failure has inspectable
  provenance and stable diagnostics.
- I-27: Ubuntu clean-checkout is user-deferred and is not a packet blocker; tracked Cargo
  build output is absent from the current tree and reachable branch history and remains
  prohibited by an executable zero-state gate.
- I-28: aggregate command optimization preserves proof coverage; performance claims require
  a controlled representative benchmark and are not stored as execution-state facts.

Doctrine disposition: Principles 10, 11, 12, 14, 17, 25, 27, 29, 30, and 31 advance;
Principles 5–8 and 19–24 are maintained; graph cycles and duplicate authority are risks
mitigated through executable validation.

### 4.1 Cross-packet obligations

Every packet from WP06 onward updates affected requirement/traceability records, contract
digests, fault points, security fixtures, comparison rules, and metrics in the same packet.
A contract source edit always runs generation/check/reproduction before packet completion.
Owner-reviewed normative content must be accepted before generated code consumes it.
Packet completion requires every acceptance check at its proving commit and at HEAD.

---

## 5. Governing decisions and library basis

### 5.1 Decisions

- D-01: Preserve four build domains as the root stable package, standalone dated-nightly
  extractor, standalone stable Pyrefly sidecar, and locked Python adapter.
- D-02: Preserve one stable root package and no Cargo workspace; separate roots exist only
  for toolchain/dependency isolation and separately built executables.
- D-03: Enforce compiler/Pyrefly/gix/delta/FastMCP boundaries with private modules,
  application DTOs, compiler/type checks, and executable governance.
- D-04: Commit deterministic generated outputs through declared derivation units. Package
  language-neutral semantic data once; generated source exists only for static
  exhaustiveness or required language bindings.
- D-05: Keep the seed PyO3/Maturin/root-Python surface decommissioned; the adapter is the
  only Python project.
- D-06: Persist the registry and coordinator lifecycle machines separately and derive
  public startup status from their orthogonal state.
- D-07: Use SyntheticCanonicalIngest as a bounded Wave-3 body behind the final
  reconciliation signature; later replacement changes the body, not all consumers.
- D-08: SQLite owns high-churn operational truth; Delta owns publication-pinned fabric
  truth; names and writes never have dual authority.
- D-09: Retain isolated deep integration with the dated rustc toolchain and exact Pyrefly
  source identity under managed update procedures.
- D-10: Keep one integrated Waves 0–3 plan but execute dependency-closed wave segments and
  store only judgment in schema-2 state.
- D-11: Preserve the local-only default storage graph and exact dependency family.
- D-12: Use one staged typed Contract IR, not byte searches or shared generic values.
- D-13: Keep canonical, source, and bundle identity projections distinct.
- D-14: Use closed derivation units as the sole generated-output ownership model.
- D-15: Package one canonical artifact index and one copy of each canonical machine-data
  resource; eliminate sibling mirrors and re-declared authorities.
- D-16: Compile Protobuf once to one FDS; Python generation and Rust compile_fds consume
  that single compiled descriptor path.
- D-17: Generate strict Pydantic models, both schema modes, cached adapters, and FastMCP
  fingerprints from Contract IR.
- D-18: Reject YAML anchors, aliases, tags, and merges before materialization.
- D-19: Keep normative fixtures independently reviewed and immutable to generators.
- D-20: Complete packets require ancestor proving commits; artifacts-check validates
  structure and plan-status derives freshness/trust.
- D-21: Proof coverage is programmatic; benchmark only a stable claim and never store
  derived measurements in execution state.
- D-22: Ubuntu is user-deferred and does not block local packet execution; IR-010 is
  resolved and guarded against reintroduction.

### 5.2 Library decisions

- LD-01: DataFusion 54.1.0 is the query/catalog engine; every production session uses a
  bounded memory pool, spill policy, read-only planner policy, and frozen private catalog.
- LD-02: Arrow/Parquet 58.4.0 own batch/file contracts; generated TableSpecs own exact
  schemas, metadata, policies, partitioning, and builder capacities.
- LD-03: object_store 0.13.2 owns storage I/O; local-workstation-v1 authorizes only local
  filesystem construction.
- LD-04: delta-rs rev 9f922319 owns durable tables through exact-version providers,
  application transactions, OCC, constraints, and local-only default features.
- LD-05: Tokio and futures own async orchestration; blocking work uses bounded classes.
- LD-06: gix 0.86.0 is confined to GitStateAdapter and returns application DTOs.
- LD-07: serde, serde_json 1.0.151 arbitrary_precision, blake3, base64, url, and tracing own
  generic representation, hashing/encoding, identifiers, and observability. Rust JCS uses
  serde_json_canonicalizer 0.3.2.
- LD-08: exact lockfile-pinned rusqlite with bundled and backup owns SQLite WAL.
- LD-09: ArcSwap owns active snapshot publication; async-trait is admitted only at the
  application port boundary.
- LD-10: grpcio/grpcio-tools 1.83.0, protobuf 7.36.0, prost-build 0.14.4, and
  tonic/tonic-prost-build 0.14.6 implement one grpcio-tools FDS plus Rust compile_fds.
- LD-11: FastMCP 3.4.7, Pydantic 2.13.4, and pydantic-settings 2.15.0 own adapter
  publication, strict models, settings, schema modes, TypeAdapters, and fingerprints;
  orjson is absent.
- LD-12: dated nightly-2026-08-18 plus rustc_public/rustc-dev is the isolated extractor
  toolchain.
- LD-13: Pyrefly 1.2.0 with exact source identity is the isolated semantic sidecar.
- LD-14: later provider pins are recorded in bundles but not adopted before use.
- LD-15: uv is the sole Python environment/lock manager; Maturin remains removed.
- LD-16: rustix 1.1.4 fs APIs own safe descriptor-relative authoritative-byte reads.
- LD-17: Python canonical bytes use rfc8785 0.1.4, strict CPython 3.14 json hooks, and
  blake3 1.0.9.
- LD-18: serde_yaml_ng 0.10.0 decodes only after the bounded YAML-subset scanner rejects
  anchors, aliases, tags, and merges.
- LD-19: jsonschema 4.26.0 validates Draft 2020-12 schemas in hermetic tooling.
- LD-20: sccache and shared target roots reduce compile cost but never supply correctness
  evidence.

No library decision authorizes a new crate or a second semantic implementation.

---

## 6. Review integration and legacy disposition

- IR-010: resolved before final v5 planning by rewriting the affected unpublished history,
  adding nested Cargo-root ignore coverage, and proving zero tracked/reachable target
  objects. WP00 and DB06 preserve the zero state.
- IR-011: closed by v5 replacing stale mechanics and absent recipe names.
- IR-012: resolved by WP00 and the schema-2 state contract.
- IR-013: resolved by WP06a, DB05, and the WP10 migration.
- Prior IR-001–IR-009: their landed remediation rules remain standing acceptance
  constraints for WP06–WP11.

Legacy dispositions:

- L-01 seed PyO3/Maturin/root-Python surface remains absent; DB01 prevents return.
- L-02 manual catalog census, generic header/status scans, secondary artifact indexes,
  and generated language-neutral mirrors remain absent; DB02 prevents return.
- L-03 protoc-bin-vendored, a second Rust protoc interpretation, open Python pins, and
  orjson remain absent; DB03 prevents return.
- L-04 independent adapter schemas and request-hot-loop model construction remain absent;
  DB04 prevents return.
- L-05 ArtifactDescriptor.generated_outputs, suite-self umbrella Proto ownership, and
  hard-coded Wave-0 generator filenames are removed by WP06a/WP10; DB05 proves zero state.
- L-06 the synthetic reconciliation implementation is a bounded Wave-3 transition and
  retains the production signature for later replacement.

---

## 7. Work packets — Wave 0 (Program, toolchain, and build foundation)

### WP00 — Execution-state and provenance reconciliation

#### Outcome

The v5 schema-2 execution state exists; the v4 and remediation schema-1 states are
migrated without derived facts; the accepted remediation disposition, current baseline,
Ubuntu deferral, resolved IR-010 zero state, and every trusted completed packet have an
explicit ancestor proving commit. No product packet is considered complete from recorded
prose. A permanent gate rejects nested Cargo build outputs in both the index and reachable
HEAD history.

#### Dependencies

None. This is the first v5 execution packet.

#### Target Invariants

I-25 and I-27. Maintains doctrine Principles 25, 27, 29, and 31.

#### Design and Library References

Implementation-status review, implementation review IR-012, artifact-schemas §3 and §8.

#### Change Surface

##### Preflight Query

~~~bash
jq '{schema_version,status,current_packet,packets,milestones,decommission_batches}' docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json
git cat-file -e ca7ce57c12bfcf2a3208875230bc9f5ecc6ddb2e
git merge-base --is-ancestor ca7ce57c12bfcf2a3208875230bc9f5ecc6ddb2e HEAD
rg -n 'proving_commit|current_head|check_results|changed_files|evidence' docs/plans/state
test -z "$(git ls-files | rg '(^|/)target/' || true)"
test -z "$(git rev-list --objects HEAD | rg ' (pyrefly-sidecar|rustc-extractor)/target/' || true)"
~~~

##### Known Touch (verified this session)

The two schema-1 state files, the new v5 state path, artifact-schema validation tooling,
`.gitignore`, a tracked-target zero-state checker, and the justfile gate registry.

#### Required Changes

Create the v5 schema-2 state when execution starts. Migrate both historical states by
retaining decisions, deviations, failed approaches, blockers, and packet judgment while
dropping digests, changed-file inventories, check output, current-head caches, and other
derivable facts. Record current proving commits only after re-running each packet's named
checks. Record the remediation plan as user-accepted/executed despite its historical draft
frontmatter. Record Ubuntu as user-deferred and IR-010 as resolved baseline evidence. Add
`tracked-target-zero-state-check`: it asserts nested Cargo-root target paths are ignored,
the Git index contains none, and reachable HEAD history contains none.

#### Legacy Disposition and Decommission

Schema-1 state remains immutable provenance after migration. No compatibility reader may
make a null proving commit equivalent to completion.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp00_behavioral_acceptance` in the packet's focused test target.

state_schema_v2_round_trips_judgment_only and packet_trust_requires_ancestor_commit pass.

##### Structural

Executable oracle: `wp00_structural_acceptance` in the packet's focused test target.

New just artifacts-check validates plan/review/state schemas, paths, IDs, and declared
inputs. New just plan-status derives freshness, commit existence/ancestry, and named-check
trust without storing those facts. New just tracked-target-zero-state-check validates
nested-root ignore coverage, the current index, and reachable HEAD history.

##### Negative / Zero-State

Executable oracle: `wp00_negative_zero_state` in the packet's focused test target.

artifacts-check rejects schema-2 state containing null proving commits or derived-fact
fields; plan-status reports a missing/non-ancestor proving commit as untrusted. A temporary
Git-repository fixture proves tracked-target-zero-state-check rejects both a currently
tracked nested target path and one present only in reachable history.

##### Operational

Executable oracle: `wp00_operational_acceptance` in the packet's focused test target.

just artifacts-check, just plan-status, just tracked-target-zero-state-check, and just
ci-fast exit 0 at the proving commit and HEAD.

#### Edit-Local Gates

jq parse checks, changed-file Typos, and git diff --check.

#### Packet-Local Gates

just artifacts-check; just plan-status; just tracked-target-zero-state-check; just ci-fast.

#### Integration Milestone

M00.

#### Replan Triggers

A state judgment cannot be migrated without inventing history; a proposed proving commit
is not in current ancestry; or schema-2 validation needs facts forbidden by the schema.

#### Rollback or Recovery

Revert only the new schema-2 artifacts; retain schema-1 files unchanged as history. Never
revert the completed IR-010 cleanup or nested-root ignore coverage.

#### Design-Bearing Contracts and Exemplars

The schema-2 state shape in artifact-schemas §3 is normative; do not extend it with
derived evidence fields.

### WP01 — Stable-domain re-baseline and seed decommission

#### Outcome

The root package `codefabric` is the stable daemon/data-plane
domain: edition 2024, `rust-version = "1.94.1"`, no PyO3/Maturin surface, no
Python packaging at the root, lints preserved (`unsafe_code = "deny"`, clippy
all+pedantic), sccache wrapper preserved. Its locked manifest resolves the
**actual stable production graph**: the exact Arrow/Parquet/DataFusion/
object_store/delta-rs/gix pins and features, Tokio/futures/serde/hash utilities,
exact `rusqlite` (`bundled`,`backup`), and exact `rustix` (`fs`). A narrow
compatibility module/test compiles load-bearing schema/session/provider,
Delta-transaction, gix SHA-1/SHA-256, SQLite backup, and secure-open APIs so
dependency hygiene sees real use. The seed code and packaging surface are
gone. The exact graph honestly records the kernel-forced latent
`object_store` cloud features while default resolution excludes
`deltalake-aws` and the AWS SDK. Repository docs describe the four-domain
topology and corrected local-storage boundary.

#### Dependencies

WP00.

#### Target Invariants

I-10, I-12 (preparation), I-16, L-01–L-04. Doctrine
P6, P8, P27, P31.

#### Design and Library References

Roadmap §5 WP1; Data Fabric §2.1–2.2;
repo-spec §0.3, §9–11, §77; D-01/D-02/D-05; LD-01–LD-10 and LD-16 adopted as
the stable compatibility baseline (domain-specific LD-10 generators complete
in WP05).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`Cargo.toml` (drop `[lib] cdylib`, `python`
  feature, `pyo3`; set edition 2024 + `rust-version = "1.94.1"`; adopt the
  stable graph and feature table), `Cargo.lock`, `src/lib.rs`, new
  `src/compatibility.rs`, `src/python.rs` (delete), `tests/integration.rs` +
  `tests/integration/`, `pyproject.toml`, `python/`, `python_tests/`,
  `uv.lock`, `.python-version`, `.envrc`, `scripts/bootstrap.sh`,
  `scripts/wheel_test.sh` (delete), `CLAUDE.md`, `AGENTS.md`, `README.md`,
  **`justfile` and `.github/workflows/ci.yml`** (minimal de-Python reshape in
  this packet — every recipe/step referencing the removed surfaces is
  removed or stubbed so the tree is never red between WP01 and WP05; WP05
  performs the four-domain build-out), and **`sgconfig.yml` + `rules/`
  bootstrap** (empty-but-running `ast-grep scan` governance harness, so
  WP02–WP04 can land their boundary rules).
- *Likely touch:* `_typos.toml` (path excludes), `bacon.toml` (drop
  `check-python-feature` job), `deny.toml` (exact allow-git entry for only the
  pinned delta-rs rev; WP05 adds duplicate-family policy), `.gitignore`
  (dist/ no longer produced), `.gitattributes`
  (generated-tree `linguist-generated` marks land with WP05 — D-04), and an
  editor multi-root seed (rust-analyzer `linkedProjects`; extended by
  WP02/WP03 as their roots land — audit Q1).
##### Preflight Query

  `rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'pyo3|maturin|_native|python/codefabric'`
  to enumerate every residual reference (includes `.claude/` skills text —
  update or annotate as documentation-only).

#### Required Changes

1. Rewrite root `Cargo.toml` per D-02: single rlib crate, edition 2024,
   `rust-version = "1.94.1"` (verified — `cargo msrv verify` joins the packet
   gates and the §14 matrix, honoring repo-spec §27's "never advertise an
   unverified MSRV"; the floor is set by the pinned delta-rs revision, not by
   any language feature CodeFabric uses, so it is a build-tooling obligation
   that `cargo msrv verify` must confirm the installed stable satisfies).
   Resolve the real stable dependency graph and commit its lockfile. Declare
   exactly `default = ["local-workstation"]`, `local-workstation = []`, and
   `s3-storage = ["deltalake/s3"]`; default builds must not resolve
   `deltalake-aws` or the AWS SDK, while the graph report must show the latent
   `object_store` features forced by `buoyant_kernel` instead of claiming they
   are absent. Exact-version probe-selected crates are recorded in LD-08/
   LD-10/LD-16 and state before merge.
2. Delete L-01/L-02/L-03 surfaces; keep the one `tests/integration.rs` target
   and add a compatibility case that exercises public APIs at each
   load-bearing library seam. Production packets replace the compatibility
   module incrementally; it is not a second engine.
3. Update `.envrc`/`bootstrap.sh`: no root `uv sync`; report the four domains
   (adapter sync arrives with WP04; extractor/sidecar checks with WP02/03).
4. Minimal `justfile`/CI reshape (see verified current touch points) — the packet gate below is
   the reshaped recipe set, and it must be green at packet close.
5. Update `CLAUDE.md`/`AGENTS.md`/`README.md` to the new topology and v1.3
   spec filenames.
6. Add `tooling/security/advisory-exceptions.json` plus an independent checker
   that requires exact equality with `[advisories].ignore`, verifies each
   selected package/version against `Cargo.lock`, and requires owner/review
   metadata. The initial exact set is RUSTSEC-2024-0436 (`paste 1.0.15`),
   RUSTSEC-2026-0173 (`proc-macro-error2 2.0.1`), and
   RUSTSEC-2026-0194/0195 (`quick-xml 0.39.4`), all owned by WP19 review.
   License evaluation is outside the active gate by explicit user direction.

#### Legacy Disposition and Decommission

Executes L-01–L-04 (see §6).

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp01_behavioral_acceptance` in the packet's focused test target.

`cargo check --all-targets` and `cargo clippy --all-targets
  -- -D warnings` pass for default and `--no-default-features`; `cargo nextest
  run` passes the compatibility tests. A provider/session/schema, application-
  transaction, gix algorithm, live-WAL backup, and descriptor-relative open
  smoke compiles against the exact selected APIs.
##### Structural

Executable oracle: `wp01_structural_acceptance` in the packet's focused test target.

an actual-graph metadata validator proves one approved
  Arrow/Parquet/DataFusion/object_store/kernel family, the exact delta/gix/
  SQLite/rustix features, the kernel-forced `object_store` cloud features,
  default absence of `deltalake-aws`/AWS SDK, explicit S3 activation, resolver
  3, and default rlib crate type. Advisory registry/deny equality and locked
  package/version selection are checked. `cargo tree -e features` evidence is
  retained at M01.
##### Negative / Zero-State

Executable oracle: `wp01_negative_zero_state` in the packet's focused test target.

preflight `rg` sweep returns zero live-code hits
  for `pyo3|maturin|_native|python/codefabric` (declared scope: whole repo
  minus `.git`, `docs/`, `.claude/` annotations); `cargo tree -i pyo3` errors
  (not in graph).
##### Operational

Executable oracle: `wp01_operational_acceptance` in the packet's focused test target.

scripted assertion: `./scripts/bootstrap.sh` output and
  `just --list` contain zero matches for
  `maturin|wheel|python-develop|test-python` (grep-based test committed with
  the packet).

#### Edit-Local Gates

`cargo check`; `cargo fmt --check`; Typos on changed docs.

#### Packet-Local Gates

The reshaped root-domain gate: fmt, default and
featureless check/clippy, nextest, doctest, typos, machete/shear, advisory
registry check, deny advisories/bans/sources, `cargo audit`,
`cargo msrv verify`, the actual-graph metadata validator, and `ast-grep scan`.
If a hygiene scanner cannot recognize a compatibility use, any exemption must
name the crate, rationale, owner, and expiry packet (no later than WP19); no
blanket ignore is accepted.
#### Integration Milestone

M01.
#### Replan Triggers

A hidden consumer of the seed surfaces (e.g., a script
importing `codefabric` Python package) that cannot be deleted — none known;
if found, plan revision.
#### Rollback or Recovery

Single revert commit; baseline is green.

### WP02 — Nightly rustc-extractor build domain shell

#### Outcome

`rustc-extractor/` is a standalone Cargo root pinned to
`nightly-2026-08-18` (components `rustc-dev`, `rust-src`, `llvm-tools` —
audit L-1), building executable `codefabric-rustc-extractor` that
prints its exact toolchain identity (rustc version + commit hash — the
AC-G-14 Rust context-manifest identity fields, Fact Gen — audit C-3)
to STDERR via a `--identity` invocation, writes nothing non-protocol to
STDOUT, and terminates cleanly. Per D-09 the domain **links `rustc_public`
via `rustc-dev` in its default build** — the deep-integration baseline is
proven at Wave 0, not deferred — and the exact rustc commit hash is recorded
into the identity output and the WP11 toolchain-bundle record. The root
`rust-toolchain.toml` comment and AGENTS.md/CLAUDE.md toolchain sections are
updated to the ratified posture: nightly is the extractor domain's
production toolchain (no longer analysis-only); the root stays stable.

#### Dependencies

WP01 (repo shape).
#### Target Invariants

I-08, I-10. Doctrine P6, P8, P29.
#### Design and Library References

Roadmap §5 WP2; Fact Gen §2, §7.4,
AC-G-31 (rules 1, 6–7 shape the shell's I/O discipline); repo-spec §76;
LD-12.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

new `rustc-extractor/{Cargo.toml,Cargo.lock,rust-toolchain.toml,src/main.rs}`;
  domain-local `rustc-extractor/toolchain-identity.json`. Root bootstrap,
  docs, editor config, CI, and aggregate commands are shared integration
  surfaces owned by WP05; WP02 does not edit them.
##### Preflight Query

`rustc +nightly-2026-08-18 --version --verbose`
  for the exact commit hash; mechanics of the `rustc-dev` link at this date
  (`extern crate rustc_public` module shape, required `-Z`/wrapper flags) —
  the consumption mode itself is decided by D-09; only its mechanics are
  confirmed here.

#### Required Changes

Package with `publish = false`; `main.rs` shell:
parses `--identity`/`--serve <endpoint>` (serve mode is a stub that connects
nothing yet), prints identity to STDERR, exits 0. A smoke test asserts STDOUT
byte-emptiness. A **mandatory** link-smoke module (default build, no feature
gate) links `rustc_public` through `rustc-dev` and exercises one trivial
entry point, so every CI run proves the deep-integration toolchain is
whole; extraction logic itself remains deferred to Waves 5/10 per the
roadmap.

#### Legacy Disposition and Decommission

None.
#### Acceptance Checks
##### Behavioral

Executable oracle: `wp02_behavioral_acceptance` in the packet's focused test target.

`cd rustc-extractor && cargo check && cargo test` on the
  pinned nightly; `--identity` prints toolchain + commit hash to STDERR;
  STDOUT is empty in all invocations.
##### Structural

Executable oracle: `wp02_structural_acceptance` in the packet's focused test target.

`rust-toolchain.toml` pins the dated nightly + components;
  own `Cargo.lock` committed; the `rustc_public` link smoke compiles and
  runs in the default build.
##### Negative / Zero-State

Executable oracle: `wp02_negative_zero_state` in the packet's focused test target.

root `rust-toolchain.toml` unchanged (stable); root
  `Cargo.lock` has no extractor entries; extractor is not a workspace member.
##### Operational

Executable oracle: `wp02_operational_acceptance` in the packet's focused test target.

clean-checkout build documented in CI (WP05 wires it).
#### Edit-Local Gates

`cargo +nightly-2026-08-18 check` in the domain directory.

#### Packet-Local Gates

Domain check/clippy/test + STDOUT-discipline test.
#### Integration Milestone

M01.
#### Replan Triggers

`rustc_public`/`rustc-dev` unavailable or broken on
2026-08-18 for aarch64-apple-darwin → design issue to Fact Gen §2 (pin moves);
this is a **plan-revision** trigger, not an ad-hoc pin change.
#### Rollback or Recovery

Delete the directory; no other domain depends on it in Wave 0.

### WP03 — Pyrefly sidecar build domain shell

#### Outcome

`pyrefly-sidecar/` is a standalone Cargo root (stable toolchain,
own `Cargo.lock`, own `deny.toml` permitting exactly the pinned Pyrefly
source) whose executable `codefabric-pyrefly-sidecar` links Pyrefly 1.2.0,
prints identity (sidecar build + Pyrefly source digest per AC-G-30 handshake
fields) to STDERR, keeps STDOUT protocol-silent, and exposes no
Pyrefly-internal Rust type in any public item.

#### Dependencies

WP01.
#### Target Invariants

I-08, I-10. Doctrine P6, P8.
#### Design and Library References

Roadmap §5 WP3; Fact Gen §2, §7.3,
AC-G-30 (stdout/stderr rules), AC-G-14 (`pyrefly_bundle_digest`); LD-13;
code-facts reference: Pyrefly bundles Ruff component crates 0.0.6 — one minor
behind the 0.0.7 anchor — which is the standing justification for the
process/build isolation.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

new `pyrefly-sidecar/{Cargo.toml,Cargo.lock,deny.toml,src/main.rs}`;
  domain-local `pyrefly-sidecar/toolchain-identity.json`. Root bootstrap,
  docs, editor config, CI, and aggregate commands are owned by WP05.
##### Preflight Query

resolve Pyrefly 1.2.0's exact coordinate
  (crates.io `=1.2.0` vs git tag) and record it plus a BLAKE3 digest of the
  locked source (LD-13).

#### Required Changes

Shell binary with `--identity`; a private module links
`pyrefly` and computes/embeds the bundle digest at build time (build script
hashing the lockfile entry), and a **mandatory link-smoke** exercises the
`pyrefly::query` facade entry points (construct/inspect only — no analysis),
so the Wave 9 deep-integration surface is proven from Wave 0 per D-09.
Public API surface: none (bin-only). An `ast-grep` governance rule (D-03)
asserts no `pub` item references a `pyrefly::` type. Updates to the Pyrefly
rev follow the D-09 managed procedure.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp03_behavioral_acceptance` in the packet's focused test target.

domain `cargo check/test`; `--identity` on STDERR; STDOUT
  empty.
##### Structural

Executable oracle: `wp03_structural_acceptance` in the packet's focused test target.

independent `Cargo.lock`; root lockfile untouched.
##### Negative / Zero-State

Executable oracle: `wp03_negative_zero_state` in the packet's focused test target.

governance rule zero-hit for exposed Pyrefly types; root
  `cargo tree -i pyrefly` errors.
##### Operational

Executable oracle: `wp03_operational_acceptance` in the packet's focused test target.

domain deny policy passes (git source allowed only here if a
  git pin is required).
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

Domain fmt/check/clippy/test + STDOUT test + deny.
#### Integration Milestone

M01.
#### Replan Triggers

Pyrefly 1.2.0 not resolvable as a pinned source, or it
fails to build on stable 1.97.1 → plan revision (sidecar toolchain pin) +
design issue to Fact Gen §2.
#### Rollback or Recovery

Delete directory.

### WP04 — Python FastMCP adapter domain shell

#### Outcome

`codefabric-cpg-mcp/` exists exactly per Serving §54's layout
(Wave 0 subset): locked project with the exact pins
`fastmcp==3.4.7`, `pydantic==2.13.4`, `pydantic-settings==2.15.0`,
`grpcio==1.83.0`, and `protobuf==7.36.0`; `orjson` is absent;
`python -m codefabric_cpg_mcp`
starts a STDIO-safe FastMCP shell (settings module with the §55 immutable
`SettingsConfigDict`, `mcp.run()` entrypoint) and terminates cleanly with
**zero non-protocol STDOUT bytes**; a pytest asserts the launch discipline
using the locked command `uv run --frozen --project <abs> python -m
codefabric_cpg_mcp`; `python -m codefabric_cpg_mcp --identity` prints the
adapter, FastMCP, Pydantic, pydantic-settings, and Python versions to STDERR
and exits 0 (the domain's version-identity surface for the Wave 0 exit).

#### Dependencies

WP01.
#### Target Invariants

I-07, I-08, I-10. Doctrine P6, P29.
#### Design and Library References

Roadmap §5 WP4; Serving §18–20, §54, §55,
§60.2, §68.6, §79 (context only — its Phase 1 begins at the serving
waves; audit C-15), §0.6; LD-11.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

new `codefabric-cpg-mcp/` tree (`pyproject.toml`, `uv.lock`,
  `README.md`, `src/codefabric_cpg_mcp/{__init__,__main__,server,settings}.py`,
  `tests/` skeleton); dev group:
  `pytest`, `ruff`, `pyrefly` with configs scoped to this project. **No
  adapter-local `.proto`**: `contracts/rpc/` is the single generating source
  (AC-G-01/05); the Serving §54 tree's `proto/` entry is superseded by the
  manifest layout under SUITE AC-G-05, and the adapter consumes generated
  stubs only. Root `.envrc`, bootstrap, CI, and aggregate recipes are WP05's
  serialized integration surface.
##### Preflight Query

verify exact Python pins and the frozen adapter lock;
  no JSON acceleration dependency is admitted by compatibility probing.

#### Required Changes

Settings per §55 verbatim (env aliases, `OpaqueId`,
bounded numerics with the spec's defaults/ranges, locked source order without
dotenv); `server.py` constructs `FastMCP` with `instructions` and a lifespan
that only loads settings in Wave 0 (daemon handshake arrives Waves 15+);
`__main__.py` is `mcp.run()`. Tools/resources/prompts are **not** registered
yet — Wave-0 shell scope per roadmap §5 W0.4. (Serving §79's own Phase 1
already includes four public tools, so §79 phasing begins with the serving
waves, not here — audit C-15.) All logging goes to STDERR. Use FastMCP
3.4.7's in-memory `Client(mcp)` to initialize, ping, and list tools through the
real protocol pipeline, asserting an empty tool list. Retain the subprocess
test for STDOUT isolation. Add a Pyrefly coverage sentinel test that injects a
known type error into a configured source path and proves the recipe fails,
then proves the clean project passes. `attrs`/`cattrs` are not adopted.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp04_behavioral_acceptance` in the packet's focused test target.

STDIO test — spawn the locked command with required env vars,
  assert startup, clean shutdown, and zero stray STDOUT (Serving §68.6);
  `uv run --frozen` succeeds from clean checkout; in-memory FastMCP
  initialize/ping/list-tools passes through the protocol pipeline.
##### Structural

Executable oracle: `wp04_structural_acceptance` in the packet's focused test target.

`uv.lock` carries the three exact pins; no `pydantic-core`
  pin; `requires-python >=3.12`; interpreter 3.14.7 recorded.
##### Negative / Zero-State

Executable oracle: `wp04_negative_zero_state` in the packet's focused test target.

no Arrow/DataFusion/Maturin/PyO3 dependency in the adapter
  graph (`uv tree` scoped check); no dotenv source in settings.
##### Operational

Executable oracle: `wp04_operational_acceptance` in the packet's focused test target.

ruff + pyrefly pass on the new project; the Pyrefly inclusion
  sentinel's fail/pass pair proves source coverage.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

ruff format/check, pyrefly, pytest for this project.
#### Integration Milestone

M01.
#### Replan Triggers

fastmcp 3.4.7 or pydantic 2.13.4 incompatible with
Python 3.14.7 → the interpreter pin drops to the newest compatible 3.x
(recorded deviation; the spec floor is 3.12) — implementation adaptation, not
design change.
#### Rollback or Recovery

Delete directory; no shared root-file revert is needed.

### WP05 — Protobuf toolchain, repository command contract, and four-domain CI

#### Outcome

One exact `grpcio-tools==1.83.0` invocation emits Python bindings
and one committed `FileDescriptorSet`; Rust decodes that same descriptor IR
through Prost/Tonic `compile_fds`. The `justfile` is reshaped into per-domain recipe groups plus
cross-domain gates; `.github/workflows/ci.yml` builds all four domains from a
clean checkout, runs the duplicate-family policy, and executes the
STDOUT-discipline smoke tests. It integrates the domain-local identities from
WP02–WP04 into bootstrap/docs/editor configuration, and locks the exact
Python compiler plus Rust/Python generator identities. The selected
tonic incoming-stream integration propagates OS peer credentials into request
extensions and rejects missing/mismatched peers before dispatch. Deterministic test roots and fixture
conventions are established (`contracts/fixtures/` as the shared
cross-language corpus root; per-domain `tests/`).

#### Dependencies

WP01–WP04.
#### Target Invariants

I-08, I-10, I-12. Doctrine P25, P29, P31.
#### Design and Library References

Roadmap §5 WP5–6; Data Fabric §2.2 (CI
duplicate rejection); Serving §70 (fingerprint comparison posture), §77
(upgrade-gate posture); repo-spec §14, §49–52; LD-10; D-03/D-04.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`justfile`, `.github/workflows/ci.yml` (four-domain
  build-out over WP01's minimal reshape), `deny.toml` (`[bans]`
  `multiple-versions = "deny"` scoped via `skip`/`skip-tree` so the
  arrow/parquet/datafusion/object_store families are hard-denied duplicates),
  new `tooling/proto/` build wiring, `rules/` additions (the D-03 boundary
  rules incl. the adapter-side FastMCP/Pydantic-internals rule; harness
  bootstrapped in WP01), `contracts/fixtures/` skeleton, `.envrc`,
  `scripts/bootstrap.sh`, root toolchain comments/docs, and editor multi-root
  configuration. WP05 is the sole Wave-0 owner of those shared surfaces.
##### Preflight Query

re-run the UDS peer-identity/interoperability probe and
  verify exact grpcio-tools/protobuf plus Prost/Tonic pins. Ambient system `protoc`
  and a second Rust compiler invocation are not allowed correctness inputs.

#### Required Changes

1. `proto-gen` invokes grpcio-tools once to emit Python bindings and the committed
   FileDescriptorSet; Rust decodes that FDS and uses compile_fds. `proto-check` and
   `proto-repro-check` verify identity and two-root byte equality. Generation records
   exact grpcio-tools, protobuf, Prost, and Tonic identities.
2. Recipe groups: `root-*` (fmt/check/clippy/test/doctest), `extractor-*`,
   `sidecar-*`, `adapter-*` (ruff/pyrefly/pytest), `contracts-*`
   (regen/verify — lands fully in WP06), `governance` (`ast-grep scan`),
   aggregate `ci-fast` and `ci-pr` preserving the two justfile rules
   (mutating recipes never gate dependencies; smallest-tool-set discipline).
3. CI jobs per domain with pinned actions; `uv sync --frozen` for the
   adapter; nightly toolchain install for the extractor job; resolved-feature
   assertion for gix (`cargo tree -e features -i gix` — active from WP17, wired
   now); duplicate-family check active immediately. Extractor and sidecar
   jobs are **path/pin-triggered** (their directories, toolchain/pin files,
   shared `contracts/`) plus a scheduled nightly run and mandatory execution
   at every milestone gate once external CI assurance is reprioritized — not every-PR
   (repo-spec §49 Tier B/C placement; audit Q6). Root, adapter, contracts, and
   governance jobs remain configured, but Ubuntu execution is not a v5 blocker.
4. Duplicate-family enforcement validates the **actual WP01 graph** on every
   run: exactly one approved Arrow/Parquet/DataFusion/object_store family and
   one compatible buoyant-kernel line, with default absence of
   `deltalake-aws`/AWS SDK, honest reporting of kernel-forced `object_store`
   cloud features, and explicit S3 activation. Retain a
   **committed negative fixture**: a
   `tooling/ci/duplicate-family-fixture/` manifest carrying a second arrow
   version, against which `cargo deny check` must fail — run permanently as
   a `governance` step (expected-failure assertion). The deny config itself
   is additionally covered by a config-shape unit test.
5. Deterministic test/temp roots: all tests use per-test temp dirs (std
   tempdir or `target/tmp/<test>`); no test touches a shared mutable state
   root; daemon-state fixtures always point at packet-local temp roots.
6. Cache-authority rule (roadmap §5 W0.5): caches (sccache, uv cache) are
   never correctness authority — regeneration byte-identity gates compare
   digests of outputs, not build logs; CI records `sccache --show-stats` as
   telemetry only.
7. Generated-tree hygiene (D-04, audit Q2): `.gitattributes`
   `linguist-generated` for every generated source/binding directory declared by a
   compilation unit. The current `contracts/generated/registry/` tree is the single
   canonical machine-data resource set, not a sibling authority; WP06a migrates its output
   ownership to derivation units. Generated source/binding
   paths are excluded from `cargo fmt --check`, typos,
   `ast-grep scan`, and machete surfaces; generator output asserted
   rustfmt-stable (format-then-diff test).
8. Tonic/UDS compatibility harness: extract platform peer credentials from
   the accepted Unix stream, propagate the verified identity through request
   extensions, enforce same-user policy before handler dispatch, and set both
   encode and decode limits to 4 MiB on Rust and Python. Same UID succeeds;
   missing identity fails; a different UID fails where a platform fixture can
   create it; rejected-request handler instrumentation remains zero. Record a
   typed platform skip only for the different-UID setup, never for missing
   credential or pre-dispatch enforcement.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp05_behavioral_acceptance` in the packet's focused test target.

local four-domain gates are green; configured CI is structurally
  validated, while Ubuntu execution remains deferred; proto
  round-trip smoke test (a trivial placeholder message) passes in Rust and
  Python; peer-credential and size-limit behavior tests pass.
##### Structural

Executable oracle: `wp05_structural_acceptance` in the packet's focused test target.

`just --list` shows the domain groups; deny config carries
  the family bans.
##### Negative / Zero-State

Executable oracle: `wp05_negative_zero_state` in the packet's focused test target.

the committed duplicate-family fixture fails `cargo deny
  check` on every CI run (expected-failure test); a stub/byte drift in
  committed generated placeholders fails `proto-check`.
##### Operational

Executable oracle: `wp05_operational_acceptance` in the packet's focused test target.

`just doctor` covers all four domains; two-clean-root
  generation records matching output digests and generator identities.
#### Edit-Local Gates

Recipe-by-recipe smoke runs.

#### Packet-Local Gates

Full `just ci-fast` (new shape) plus workflow/configuration
validation that does not require an external runner.
#### Integration Milestone

M01 (closes Wave 0).
#### Replan Triggers

LD-10 probe failure (no tonic/prost pair supports the
required transport/peer-identity shape) → plan revision to a different Rust gRPC stack —
this is transport mechanics, not a design change (Serving §8/AC-G-61 name
gRPC+UDS, not a crate).
#### Rollback or Recovery

WP01–WP05 revert as a unit (WP05 rewrites shared files whose
pre-WP01 form references deleted surfaces; WP05 alone is not independently
revertible).

---

## 8. Work packets — Wave 1 (Machine contracts, registries, code generation)

### WP06 — Typed contract catalog, bounded compiler, identities, and artifact index

#### Outcome

The landed remediation substrate is re-proved as the Wave-1 compiler foundation: the
suite manifest is the only bootstrap; ContractCatalog and every artifact family use
closed typed records; format ingress is staged and budgeted; canonical/source/bundle
identities follow SUITE AC-G-02/07; RFC 8785 and BLAKE3 are library-owned; one canonical
artifact index is packaged by Rust and Python; independent KAT governance and
reproduction checks are executable.

#### Dependencies

WP05.

#### Target Invariants

I-08, I-11, I-17, I-18, I-19, I-24, I-25, and I-26.

#### Design and Library References

SUITE AC-G-02, AC-G-05, AC-G-07, AC-G-53; RM §6; remediation WP01–WP06;
LD-01–LD-04.

#### Change Surface

##### Preflight Query

~~~bash
ast-grep outline src/contracts tooling/contracts --items structure --view signatures
rg -n 'serde_json::Value|serde_yaml_ng::Value|canonical_digest|source_digest|bundle_digest|artifact-index' src tooling contracts codefabric-cpg-mcp tests -g '!**/target/**'
just contracts-verify
just contracts-repro-check
just fixture-check
~~~

##### Known Touch (verified this session)

src/contracts, contracts/manifests/suite-manifest.json, contracts/fixtures,
tooling/contracts, adapter artifact-index resource loading, justfile, and focused tests.

#### Required Changes

Preserve and revalidate the current closed ContractCatalog, resource profiles, staged raw
scan/typed decode/cross-record validation, strict JSON/JCS rules, YAML subset scanner,
EBNF parser, Proto descriptor projection, typed bundle models, dual identities, canonical
artifact index, independent fixture classes, and proof-coverage gates. Remove any residual
v4 assumption that a compiler walks generated directories, reads metadata lexically,
emits language-neutral source mirrors, or treats generated bytes as their own oracle.
WP06 does not populate the still-draft product records owned by WP07–WP11.

#### Legacy Disposition and Decommission

DB02 and DB03 retain the remediation zero states. WP06a separately replaces the remaining
single-artifact output graph.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp06_behavioral_acceptance` in the packet's focused test target.

normative_projection_vectors_have_exact_blake3_identities,
governed_sources_fit_their_named_resource_profiles,
bundle_projection_uses_the_closed_sorted_model_and_retains_member_identity, and the shared
Rust/Python JCS corpus pass.

##### Structural

Executable oracle: `wp06_structural_acceptance` in the packet's focused test target.

just contracts-verify, just schema-check, and just fixture-check pass over the catalog
census.

##### Negative / Zero-State

Executable oracle: `wp06_negative_zero_state` in the packet's focused test target.

Duplicate JSON keys, unsafe/non-finite numbers, YAML anchors/aliases/tags/merges, malformed
EBNF, unknown fields, budget overflow, digest mutation, and generator-writing-normative-KAT
fixtures fail with bounded path-aware diagnostics.

##### Operational

Executable oracle: `wp06_operational_acceptance` in the packet's focused test target.

just contracts-repro-check proves two isolated generations are byte-identical and catalog
record reordering changes only source identity.

#### Edit-Local Gates

just contracts-tooling-lint; targeted root tests; targeted tooling pytest.

#### Packet-Local Gates

just contracts-verify; just contracts-repro-check; just schema-check; just fixture-check;
just adapter-wheel-test; just proof-coverage-check; just ci-fast.

#### Integration Milestone

M02.

#### Replan Triggers

A governed family cannot be represented by a closed typed model; a named parser cannot
enforce required pre-allocation bounds; a profile cannot be implemented without custom
canonical-byte rendering; or a second artifact index becomes necessary.

#### Rollback or Recovery

Revert compiler/model/resource changes with their generated index. Normative KATs are
never regenerated or auto-accepted during rollback.

#### Design-Bearing Contracts and Exemplars

SUITE AC-G-02/05/07 projection and resource-profile tables are normative. WP06a owns only
the generated-output graph correction.

### WP06a — First-class compilation and derivation units

#### Outcome

SUITE AC-G-05 and RM §6 define catalog schema v2 with a closed typed derivation graph.
Artifact descriptors own native source authority; derivation units own every generated
output. The compiler, artifact index, registry compiler, Proto compiler, and adapter
compiler consume resolved typed invocations. Every output has one derivation owner and
inspectable transitive lineage. The catalog remains semantic governance and never becomes
a general-purpose build-system DSL.

#### Dependencies

WP06.

#### Target Invariants

I-08, I-11, I-17–I-21, I-24–I-26. Advances doctrine Principles 2, 7, 10–17,
25, 27, and 29–31.

#### Design and Library References

Implementation review IR-013; SUITE AC-G-02/05; RM §6; remediation decisions for typed
Contract IR, one artifact index, one descriptor IR, and generated Pydantic views. Serde,
grpcio-tools, Protobuf descriptors, Prost/Tonic compile_fds, and Pydantic remain the
pinned execution libraries.

#### Change Surface

##### Preflight Query

~~~bash
rg -n 'generated_outputs|depends_on|OutputsByPath|output_of_kind|SOURCE_RELATIVE|RUST_OUTPUT|wave0_probe_pb2|wave0-probe-descriptor|descriptor-census' src tooling contracts codefabric-cpg-mcp tests -g '!**/target/**'
ast-grep outline src/contracts tooling/proto tooling/contracts --items structure --view signatures
just contracts-verify
just proto-repro-check
just adapter-contracts-repro-check
~~~

##### Known Touch (verified this session)

SUITE AC-G-05, RM §6, contracts/manifests/suite-manifest.json,
src/contracts/catalog.rs, src/contracts/compiler.rs, tooling/proto/generate.py,
tooling/contracts/generate_adapter_models.py, registry generation, artifact-index
consumers, and their focused Rust/Python tests.

#### Required Changes

1. Correct SUITE AC-G-05 and RM §6 before production code consumes catalog schema v2.
2. ContractCatalog separates artifacts from derivations. ArtifactDescriptor removes
   generated_outputs and build-order depends_on and gains a typed semantic-projection
   source: Native or DerivationOutput(OutputRef).
3. Add closed deny-unknown-fields DerivationUnitDescriptor records. Each record contains
   derivation_id, closed DerivationKind, sorted typed inputs, sorted outputs, and a
   resource-budget profile. DerivationKind determines producer dispatch and valid output
   cardinality; no independently authored producer or shell command exists.
4. The initial closed DerivationKind set is ArtifactIndex, CanonicalRegistrySet,
   ProtobufDescriptorAndPython, ProtobufRustFromDescriptor, and
   AdapterModelCompilation. Adding a kind is an additive catalog/compiler contract change.
5. DerivationInput is one of Artifact with view SourceBytes or CompiledSemantic,
   Output(OutputRef), or the intrinsic AllCompiledArtifacts accepted only by
   ArtifactIndex. Arbitrary predicates, globs, platform conditionals, phase numbers,
   cache policy, and scheduling commands are forbidden.
6. DerivationOutput declares globally unique path, closed output kind, sorted primary
   artifact IDs, sorted consumers, and an optional output-specific budget. Output kinds
   may repeat; lookup is plural or scoped to derivation ID.
7. Validate the graph over source, semantic-artifact, derivation, and output nodes.
   Reject duplicate/unknown IDs, missing refs, invalid input views, invalid kind/cardinality
   combinations, empty/invalid primary sets, output/authority path conflicts, path escape,
   cycles, and an output path claimed by more than one derivation.
8. Sort derivations by ID, inputs by typed stable reference, outputs by path, primary IDs
   by artifact ID, and consumer sets by code before projection. Catalog reorder has no
   semantic effect.
9. CompiledCatalog exposes derivation(id), deterministic derivations(),
   outputs_for_derivation(id), outputs_of_kind(id, kind), output_by_path(path), package
   data by consumer, and resolved DerivationInvocation values. Remove global
   output_of_kind and artifact topological build ordering.
10. The generated index has peer artifact and derivation collections. Artifact records own
    canonical/source digests. Derivation records contain resolved input/output references,
    generator revision/tool identity, and derived lineage; they reference artifact IDs
    instead of creating a second digest authority. Consumers join the peer records to
    recover every input identity.
11. Generated source headers name primary artifact semantic identity only. Exact
    source_digest remains detached in the index, so editorial-only changes do not churn
    generated source. Output checksums are reproduction evidence, never catalog authority.
12. Registry derivations emit the already-compiled canonical JSON bytes. Provenance stays
    in the index rather than an injected wrapper that changes semantic output.
13. Proto staging is explicit:

~~~text
governed Proto SourceBytes
  -> ProtobufDescriptorAndPython
  -> FileDescriptorSet plus Python bindings
  -> typed descriptor semantic projections
  -> artifact index/final provenance
  -> ProtobufRustFromDescriptor using compile_fds
  -> Rust bindings
~~~

14. Map FileDescriptorProto.name to governed artifacts and verify the exact root-input
    census plus only allowed transitive well-known imports. descriptor-census.json is a
    generated review projection, not semantic authority.
15. Adapter generation resolves AdapterModelCompilation by derivation ID, consumes the
    compiled adapter-IR identity supplied by the invocation, and retains separate
    Pydantic validation/serialization outputs and fingerprints.
16. Python generators receive an ephemeral resolved invocation from the Rust compiler;
    they do not rescan the raw catalog, globally search output kinds, or recompute source
    identity.
17. Migrate artifact-index, registry, adapter, and Wave-0 Proto derivations. WP10 atomically
    replaces the Wave-0 probe unit and files with the production Proto units; test-only
    schema never remains in the production FDS.

#### Legacy Disposition and Decommission

DB05 proves zero artifact-level generated_outputs and build depends_on; authored producer
fields; global output_of_kind; generator scans over artifact outputs; hard-coded Wave-0
Proto constants; semantic dependence on descriptor census; suite-self Proto outputs;
custom aggregate source-set SHA-256; Python-side independent output discovery; and
Wave-0 probe source/bindings/tests after WP10.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp06a_behavioral_acceptance` in the packet's focused test target.

compilation_unit_derives_many_inputs_and_outputs,
proto_staging_uses_source_and_compiled_semantic_views,
registry_and_adapter_outputs_are_graph_complete,
catalog_reorder_preserves_derivation_bytes, and
multiple_outputs_of_one_kind_require_scoped_plural_lookup pass.

High-value metamorphic oracles pass:

- Proto comment-only edit changes source identity/index but not FDS, semantic identity, or
  Rust/Python bindings.
- Proto field-number/type edit changes descriptor identity and bindings and trips
  compatibility review.
- YAML formatting/comment edit changes source identity/index but not canonical registry.
- Adapter-IR formatting-only edit leaves Pydantic models/schemas/fingerprints unchanged.
- Removing one Proto root makes FDS census validation fail.

##### Structural

Executable oracle: `wp06a_structural_acceptance` in the packet's focused test target.

just compilation-units-check proves the suite self descriptor owns no outputs, every
released generated surface is reachable from exactly one unit, every output has one owner,
and every unit input/reference resolves.

##### Negative / Zero-State

Executable oracle: `wp06a_negative_zero_state` in the packet's focused test target.

Fixtures reject duplicate/unknown derivation IDs, missing input/output refs, invalid
input views, invalid output cardinality, invalid primary sets, authority/output path
conflict, cycles, duplicate claims, and unknown fields. DB05 repository searches are
zero outside plans/reviews and negative fixtures.

##### Operational

Executable oracle: `wp06a_operational_acceptance` in the packet's focused test target.

just contracts-repro-check, just proto-repro-check, and just
adapter-contracts-repro-check remain byte-identical across isolated roots. Installed-wheel
Python imports and Rust compile_fds compilation pass from the declared outputs.

#### Edit-Local Gates

just contracts-tooling-lint; targeted catalog/compiler nextest; targeted Proto/adapter
generator pytest; changed design-doc Typos.

#### Packet-Local Gates

just compilation-units-check; just contracts-verify; just contracts-repro-check; just
proto-repro-check; just adapter-contracts-check; just adapter-contracts-repro-check;
just governance; just ci-fast.

#### Integration Milestone

M02.

#### Replan Triggers

Reopen design if a generator requires arbitrary commands, globs, platform predicates,
scheduling policy, or input mutation; non-well-known Proto imports lack governed artifact
identity; Python cannot remain one compiler invocation; Rust cannot consume the same FDS;
descriptor names cannot map exactly to catalog inputs/outputs; a comment-only edit changes
descriptor semantics; consumers require output checksums inside the index; external
compatibility requires catalog schema v1/Wave-0 probe names; or typed artifact/output refs
are insufficient and a real build-system integration is required.

#### Rollback or Recovery

Revert catalog schema, compiler, generators, index, and normative correction together. Do
not retain v1 compatibility aliases, dual output ownership, or a partially migrated unit
graph. Regenerate only from the last accepted catalog.

#### Design-Bearing Contracts and Exemplars

~~~text
ContractCatalog
  artifacts: ArtifactDescriptor[]
  derivations: DerivationUnitDescriptor[]

ArtifactDescriptor
  native authority and projection metadata
  semantic_projection_source = Native | DerivationOutput(OutputRef)

DerivationUnitDescriptor
  derivation_id
  derivation_kind
  inputs[] = Artifact(SourceBytes | CompiledSemantic)
           | Output(OutputRef)
           | AllCompiledArtifacts for ArtifactIndex only
  outputs[] = path + output_kind + primary_artifact_ids + consumers
  resource_budget_profile
~~~

### WP07 — CBEF-v1 identity, path canonicalization, and known-answer vectors

#### Outcome

CBEF-v1 is implemented per AC-G-13: exact record header
(`CFID`, version `0x01`, big-endian domain/field framing), all 13 type codes,
ascending-field-tag emission and rejection of duplicate/nonascending tags,
truncated/non-minimal lengths, and trailing bytes (all now owned by AC-G-13),
per-field normalization rules, BLAKE3-256-truncate-16 derivation with full
32-byte digests retained in collision diagnostics and `ID_COLLISION` blocking;
the 16 required domain recipes (`WORKSPACE` … `UNKNOWN_REMAINDER`) have
owner-accepted field-tag/type-code/normalization schemas in
`contracts/identity/cbef-v1.yaml` generated by AC-G-13's deterministic
domain/field allocation rules; public
ID encode/decode with strict prefix/slug/32-hex validation and the sole
symbolic `context:source`; **`identity/type-algebra-v1.yaml`** authored here
(AC-G-15 constructor set + normalization rules + de Bruijn binders +
version pin; interning rules come from Fact Gen §20.2 — AC-G-15 contains
none, audit C-6) with a canonical type-algebra encoder and its own KAT vectors
— type IDs are CBEF-derived like all others. `WorkspacePath` per AC-G-18: canonical component encoding
(percent-escaping `/`, `%`, non-display bytes; reversible; no symlink
resolution), platform rules (Linux byte-exact; macOS volume probe + NFD/case
folding on case-insensitive volumes; WTF-8 platform code reserved), display
encoding with uppercase `%XX` and `display_is_lossy`, canonical URI
(`codefabric://workspace/<hex>/path/<base64url>`), ordering by
`(comparison_key_bytes, raw_relative_path_bytes)`. KAT vectors for every
domain recipe and path rule live in `contracts/fixtures/identity/` and pass in
Rust and Python.

#### Dependencies

WP06a.
#### Target Invariants

I-01 (preimage rules), I-08, I-11, I-13. Doctrine P13
(stable semantic identity — Advances), P12.
#### Design and Library References

Ontology AC-G-12/13/18, §64; Lifecycle
§43 (PlatformPath/GitRepoPath — §43's richer struct forms adopted over
Appendix F's conflicting minimal forms), AC-G-09 preimages; Data
Fabric §7.1–7.3.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/identity/{cbef-v1,path-canonicalization-v1,type-algebra-v1}.yaml` are current
draft authorities. No production Rust identity/path module, adapter public-ID view,
identity KAT directory, or CBEF/path fuzz target exists yet.
##### Preflight Query

macOS case-sensitivity probe API (`pathconf`/`getattrlist`
  approach) — verify on the dev volume; document Linux CI behavior.

#### Required Changes

As stated; use the owner-fixed AC-G-13 choices:
BLAKE3_128 ≡ BLAKE3-256[0..16], u32 big-endian container counts/lengths,
post-normalization payload length, canonical domain order, and 1-based field
tags from recipe declaration order. AC-G-18 owns platform codes. Record owner
acceptance of the generated initial contract before implementing the encoder.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp07_behavioral_acceptance` in the packet's focused test target.

KAT vectors green in both languages — identity domains,
  paths, **and type algebra** (roadmap §6 exit names identity, path, type,
  enum, flag, canonical-JSON vectors; enum/flag vectors land with WP08);
  property tests (round-trip public IDs; ordering total and stable;
  component encoding reversible; type interning idempotent).
##### Structural

Executable oracle: `wp07_structural_acceptance` in the packet's focused test target.

every domain recipe file validates against the recipe schema;
  field tags unique and ascending.
##### Negative / Zero-State

Executable oracle: `wp07_negative_zero_state` in the packet's focused test target.

decoder rejects wrong prefix/width/case, non-hex, unknown
  domain, out-of-order fields; collision injection test yields
  `ID_COLLISION` and blocks.
##### Operational

Executable oracle: `wp07_operational_acceptance` in the packet's focused test target.

independent owner-reviewed vectors are immutable to production
  generation; derived/property corpora reproduce byte-identically. Bounded CBEF/ID/path
  replay retains every crash as a regression.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just adapter-contracts-check; just fixture-check; just
contracts-repro-check.
#### Integration Milestone

M02.
#### Replan Triggers

A later suite artifact publishes canonical field-tag
assignments differing from ours → plan revision (regenerate vectors; possible
reindex note per §0.4 "exact match" rule) + design issue.
#### Rollback or Recovery

Additive.

### WP08 — Ontology and categorical registries + state machines

#### Outcome

`contracts/registry/` (AC-G-05 spelling) holds the machine
registries and the generator emits canonical JSON + Rust/Python lookup
artifacts for: entity kinds, relation kinds, property kinds (AC-G-71 full
record incl. value-type algebra, cardinality, `null_semantics: prohibited`,
storage mapping), fact kinds, unknown kinds (AC-G-73 mandatory 12) + reason
classes (9) + negative-fact families (4), graph projections (13 mandatory
IDs), summary profiles (`CALLABLE_SUMMARY_BALANCED_V1`), capability codes
(AC-G-36 record shape; families reserved), **derivation registry**
(`derivation-registry.yaml` — record shape per Data Fabric §79A ownership
fields; entries populate in Waves 5+ but the registry, schema, and
append-only validation exist now), error registry (AC-G-65 numeric domains
1000–9999, full record shape, all named codes incl.
`CURRENT_POINTER_CONFLICT`, `OVERLAY_GENERATION_CONFLICT`, `ID_COLLISION`,
`STATE_TRANSITION_VIOLATION`, `SOURCE_SNAPSHOT_MISMATCH`,
`SEMANTIC_PHRASE_AMBIGUOUS`, and `SEMANTIC_PHRASE_UNRECOGNIZED` — the
latter a Query AC-G-44 code admitted via AC-G-65's include-all rule (audit
C-16)…), enum/flag registries with the §62 code tables (62.1–62.6 verbatim
numeric tables; 62.7–62.9 receive the §62.10 owner-fixed declaration-order
allocations in increments of ten), provider registry with the AC-G-36
owner-fixed record, and AC-G-25 state-machine YAML for the Wave-2
machines (`WorkspaceLifecycle`, `SourceTrustState`, `EventStreamHealth`,
`GitAccelerationStatus`) **plus the Wave-3 machines
`DurablePublicationState` and `ServingActivationState`** (both in
AC-G-25's mandatory eleven-machine roster and consumed by WP22/WP24 —
audit blocker B-2), **the remaining five roster machines**
(`UpdateWaveState`, `ProviderRunState`, `OwnerCapabilityState`,
`QueryExecutionState`, `ArtifactState`) as contract-only YAML whose
runtimes arrive with their waves, **plus the AC-G-10 registry machine**
(same framework, beyond AC-G-25's mandatory roster — D-06), all with
`from/event/guard/to/actions/idempotency_key/error_on_illegal` rows and
model-checked reachability. The phrase registry, `english-controlled-v1`
grammar artifact, and `model-pack.schema.json` are split out to **WP08b**
(audit blocker B-3; supersedes the R-07 in-flight split contingency).
Enum/flag registries follow the manifest's AC-G-06 record shape and rules
verbatim: code 0 reserved-invalid and never emitted, positive signed
append-only codes in increments of ten with no gap insertion after release,
immutable names/meanings, aliases parse-only, fixed per-domain code widths,
UPPER_SNAKE names + kebab slugs; the 64-bit flag word layout (bits 0–31
language-neutral, 32–47 language-profile, 48–55 generated/lowered, 56–62
reserved, bit 63 zero). The duplicate-authority check is AC-G-01's: CI fails
if two machine artifacts declare the same concern as authoritative.
Registry invariants enforced by the verifier: per-domain code/slug
uniqueness, acyclic families, abstract kinds barred from canonical rows,
capability+storage mapping for every concrete kind, provider-native kinds
firewalled, append-only discipline.

**Completion is counted, not judged** (Gate A requires *all* registries):
all 9 §62 tables (62.1–62.6 verbatim codes; 62.7–62.9 deterministic
§62.10 codes); 12 unknown kinds + 9 reason classes + 4 negative families; 13
projections; 37 effect + 10 resource codes (both spec floors marked
"Initial"/"at least"); every error code named anywhere in the 1.3 suite;
and the eleven AC-G-25 machines plus the AC-G-10 machine, model-checked.
The phrase-section count moved to WP08b with the split (B-3).

#### Dependencies

WP06 (verifier), WP07 (slugs/ID conventions).
#### Target Invariants

I-06, I-11, I-13, I-15. Doctrine P10, P12, P29, P31.
#### Design and Library References

Ontology AC-G-70–73, §62, §67, §68 (L0–L14
layer axis; `family_code` carries the layer), §5–§58
heading families; Query AC-G-44; Serving AC-G-65; Lifecycle AC-G-25;
Fact Gen §85, AC-G-36.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/registry/*.yaml` and the current draft canonical registry projections under
`contracts/generated/registry/` are present. Production registry content, generated
Rust/Python static views, registry fixtures, and registry fuzz targets do not yet exist;
WP08 preserves one canonical resource set while WP06a replaces artifact-level output
ownership and prohibits sibling mirrors.
##### Preflight Query

none external (the phrase-harvest checklist moved to WP08b).

#### Required Changes

Replace empty draft registry records with the complete owner-approved
sets and compile every lookup/state-machine view through WP06a units. Consume the typed
catalog and shared artifact index; do not add per-language code lists or directory scans.
Unknown fields, aliases outside parsing boundaries, reassigned codes, missing mappings,
and unreachable/illegal transitions fail before emission.

#### Legacy Disposition and Decommission

DB02 forbids manual parallel registries and language-local code
allocation. Normative registry fixtures remain under independent fixture governance.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp08_behavioral_acceptance` in the packet's focused test target.

generated Rust and Python compile and expose code↔name
  lookups; **enum and flag KAT vectors pass in both languages** (fixed
  code→name→slug triples per §62 table, flag-word round-trips); distinct
  Rust/Python types per registry domain (certainty vs resolution never
  share an enum type).
##### Structural

Executable oracle: `wp08_structural_acceptance` in the packet's focused test target.

verifier enforces the eight AC-G-70 invariants + §62.10
  append-only rule; duplicate-authority check (same slug in two registries →
  error).
##### Negative / Zero-State

Executable oracle: `wp08_negative_zero_state` in the packet's focused test target.

evaluative deny-list test — registry sources containing
  `SAFE_TO_REFACTOR`-class kinds are rejected (I-15).
##### Operational

Executable oracle: `wp08_operational_acceptance` in the packet's focused test target.

byte-identical regeneration; state-machine artifacts pass
  reachability check.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

Root + adapter gates; `just contracts-verify`; bounded registry
decode fuzz replay and a focused mutation campaign over registry/state-machine
validation, with every survivor classified.
#### Integration Milestone

M02.
#### Replan Triggers

Registry record shapes prove insufficient for a Wave 2/3
consumer (e.g., missing overlay policy field despite the generated policy axes) →
plan revision limited to registry schema extension (append-only).
#### Rollback or Recovery

Additive.

### WP08b — Phrase registry, controlled-language grammar, and model-pack schema

*(Added by the v2 audit integration — blocker B-3. Realizes the split R-07
pre-designed, so the §50–§94 phrase harvest leaves the Wave-1 critical
path; runs parallel to WP09/WP10.)*

#### Outcome

`contracts/registry/phrase-registry.yaml` and the
`english-controlled-v1` grammar artifact
(`contracts/query/english-controlled-v1.ebnf`; AC-G-44 EBNF + registry
record schema + `SEMANTIC_PHRASE_UNRECOGNIZED`/`SEMANTIC_PHRASE_AMBIGUOUS`
error behavior), with a phrase-registry entry set covering **every** Query
§50–§94 catalog section — the verifier counts sections and fails on any
gap. The range is §50–§94, **not** §50–§102: §95–§102 are Part VII worked
examples that define no phrases (audit correction to v1's count rule).
Also `contracts/registry/model-pack.schema.json` (AC-G-38 format schema; no
packs ship in the 1.x baseline). Every phrase entry carries Query AC-G-44's
executable declarative `planspec_mapping`: node kind, typed slot bindings,
constant fields, and output role. A runtime natural-language compiler is not
required in Wave 1, but a placeholder or `deferred-mapping` record is invalid.

#### Dependencies

WP08 (registry framework, verifier invariants, slug/ID
conventions).
#### Target Invariants

I-11, I-13, I-15. Doctrine P10, P29.
#### Design and Library References

Query AC-G-44, §50–§94 (catalog range
verified; §95–§102 excluded); Serving AC-G-65 (error codes); manifest
AC-G-05 (artifact locations).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/registry/phrase-registry.yaml`,
`contracts/query/english-controlled-v1.ebnf`, and
`contracts/registry/model-pack.schema.json` are current draft authorities. Phrase fixtures
and a production EBNF/phrase fuzz target do not yet exist.
##### Preflight Query

enumerate the Query §50–§94 section list (spec-outline) as
  the harvest checklist the verifier counts against.

#### Required Changes

Populate phrase records and executable PlanSpec mappings from the
owner sections; replace the draft grammar/model-pack scaffold with closed typed content;
compile all views through declared units. The bounded EBNF parser, not string splitting,
proves productions, references, alternatives, delimiters, depth, and node budgets.

#### Legacy Disposition and Decommission

Prose-only mappings, deferred-mapping placeholders, generated
normative expected values, and runtime-specific phrase tables are prohibited by DB02 and
fixture governance.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp08b_behavioral_acceptance` in the packet's focused test target.

generated phrase lookups compile in Rust and Python; the
  EBNF artifact parses (grammar-lint step); the model-pack schema validates
  its committed negative fixture.
##### Structural

Executable oracle: `wp08b_structural_acceptance` in the packet's focused test target.

the verifier's section count equals the §50–§94 list
  exactly; every phrase entry names its owning section and carries a
  schema-valid executable mapping; zero placeholders exist.
##### Negative / Zero-State

Executable oracle: `wp08b_negative_zero_state` in the packet's focused test target.

an entry citing a §95–§102 example section fails the count
  rule; evaluative phrases, missing/ill-typed slots, unknown PlanSpec nodes,
  and `deferred-mapping` are rejected (I-15).
##### Operational

Executable oracle: `wp08b_operational_acceptance` in the packet's focused test target.

byte-identical regeneration.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

Root + adapter gates; `just contracts-verify`; bounded phrase/
grammar parser fuzz replay.
#### Integration Milestone

M02.
#### Replan Triggers

The §50–§94 harvest proves materially larger than one
packet → further split by language family (neutral/Python/Rust) — plan
revision, sequence unchanged.
#### Rollback or Recovery

Additive.

### WP09 — Schema generation: Arrow/Delta TableSpecs, snapshot/state schemas, JSON Schemas, adapter contracts

#### Outcome

The generator emits, from registry + identity sources:
(a) the `TableSpec` set for every Wave-3 table — control plane (`workspace`,
`common_repository`, `analysis_context`, `analysis_context_set`,
`publication`, `publication_table`, `current_publication`, `owner`,
`capability_status`, `diagnostic`, `enum_catalog` — with `workspace`
gaining `registration_revision` + `updated_at`, which §13.1 lacks but
AC-G-10/AC-G-19 require) and universal facts
(`entity`, `relation`, `property_fact`, `fact_evidence`) — with §7 physical
types (`id16`=Binary/16, `hash32`, codes, `Utf8` not `Utf8View`), §10 schema
metadata keys, primary keys, partition columns (§95: entity by
`entity_family_code, owner_bucket`; relation by `relation_family_code,
owner_bucket`; owner-bucket count 256), **three orthogonal policy axes per
table** (Data Fabric §11/AC-G-21: `durable_mutation:
DurableMutationClass`, `overlay_mutation: OverlayMutationPolicy`, and
`materialization_role: MaterializationRole`; facts use owner-replace overlay,
query-time-derived surfaces remain query-visible without pretending to be
operational projections, and current-singleton is a durable class rather than
an overlay policy), plus overlay tombstone schemas (AC-G-20 verbatim
owner/primary-key tombstone Arrow schemas);
(b) operational-store (SQLite) schema DDL for §130 tables and the
`serving_snapshot_manifest`/`active_snapshot` records (AC-G-19 field set wins;
mutable `SnapshotActivationRecord` separated);
(c) `ServingSnapshotManifest` schema (AC-G-19 complete field list, CBEF body
order, `manifest_digest`/`snapshot_id` derivations);
(d) the complete `contracts/schema/` JSON Schema set with the AC-G-05
hyphenated filenames: `analysis-context.schema.json`,
`serving-snapshot.schema.json`, `public-snapshot-metadata.schema.json`
(`PublicSnapshotMetadata` defined once, consumed by response/status/artifact
surfaces), `source-context.schema.json`, `public-status.schema.json`,
`cpg-semantic-query-request.schema.json`,
`cpg-semantic-query-response.schema.json` (envelope fields per Query §6, public-ID
patterns per §32; §103–104 merely name the artifacts — audit C-21), plus `query/planspec.schema.json` (AC-G-46
node/value types; unbound + bound forms; JSON Schema 2020-12);
(e) `contracts/adapter/` public schemas per AC-G-05:
`fastmcp-input.schema.json`, `fastmcp-output.schema.json`,
`fastmcp-public-meta.schema.json`, generated from the adapter contract
models;
(f) adapter public contracts generated from Contract IR: strict Pydantic models,
cached `TypeAdapter`s, distinct validation- and serialization-mode schemas,
`$schema` plus stable `$id`, stdlib-JSON presentation, and the frozen FastMCP
client-visible fingerprint; no sibling schema authority or `orjson` dependency
(Serving §19–20, §60.1, §70).

#### Dependencies

WP07, WP08.
#### Target Invariants

I-05, I-06, I-08, I-11, I-12, I-19, I-20, I-22,
I-23, I-24. Doctrine P10, P14, P29, P31.
#### Design and Library References

Data Fabric §7–§16, §95, AC-G-19/20/21;
Query §32–33, §36–48 (record shapes), §103–105, AC-G-46; Serving §19, §20,
§55, §60, §70; LD-01/02 (ref-doc: metadata survival caveats — schema metadata
is contract-tested, never relied on through plan operators).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/schema/**`, `contracts/adapter/adapter-model-ir.json`,
`contracts/query/planspec.schema.json`, and the landed adapter Contract-IR/Pydantic outputs
are current. Production TableSpecs, SQLite DDL, snapshot/state/public schema content, and
schema fixtures remain incomplete.
##### Preflight Query

Arrow schema snapshot tests harness; confirm `Binary` (not
  `LargeBinary`) across builders (LD-02 caveat).

#### Required Changes

Preserve the landed Contract-IR-to-Pydantic compiler and extend its
owner-approved input model rather than adding a sibling renderer. Generate production
TableSpecs, SQLite DDL, snapshot/state/public JSON Schemas, validation/serialization
schema views, cached TypeAdapters, and the frozen FastMCP fingerprint through WP06a
compilation units. Unknown fields fail; models/adapters build once at import/lifespan.

#### Legacy Disposition and Decommission

DB04 prohibits independent adapter schemas, handwritten public
model duplication, and request-hot-loop model/schema construction.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp09_behavioral_acceptance` in the packet's focused test target.

every released fixture validates against its schema; Arrow
  schema snapshot tests; `just adapter-contracts-check` is green; validation and
  serialization schemas carry the required dialect and stable `$id`.
##### Structural

Executable oracle: `wp09_structural_acceptance` in the packet's focused test target.

every `TableSpec` declares one value on each applicable axis;
  the generated validity matrix rejects illegal cross-products; durable-only
  consumers cannot read overlay/materialization fields and vice versa;
  `OPERATIONAL_PROJECTION` never backs a query-visible effective fact and
  query-time-derived rows use `QUERY_TIME_DERIVED`; every property fact
  value-type maps to exactly one typed column set; all AC-G-05 `schema/` and
  `adapter/` filenames present (verifier layout check).
##### Negative / Zero-State

Executable oracle: `wp09_negative_zero_state` in the packet's focused test target.

a committed drift fixture (schema changed, version unchanged)
  under `contracts/fixtures/negative/` fails the fingerprint check on every
  run; JSON-blob/EAV shapes rejected by generator tests (§5.1 prohibitions);
  `Utf8View` rejected per §65.2 (audit C-9).
##### Operational

Executable oracle: `wp09_operational_acceptance` in the packet's focused test target.

`just adapter-contracts-repro-check` and `just contracts-repro-check`
  are byte-identical across isolated roots.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

`just schema-check`; `just adapter-contracts-check`; `just
adapter-contracts-governance`; `just adapter-contracts-repro-check`; `just
contracts-verify`; root and adapter gates.
#### Integration Milestone

M02.
#### Replan Triggers

A required AC-G-19 field cannot be computed in Wave 3
(`effective_content_digest` cost) → interim contract records the
computation as full-scan-at-publication with a design issue filed; if that is
rejected, plan revision.
#### Rollback or Recovery

Additive.

### WP10 — Protocol generation: four Protobuf packages + negotiated features

#### Outcome

`contracts/rpc/` defines one production Proto compilation unit over the
four governed source authorities. One exact grpcio-tools invocation emits Python
bindings and the committed FDS; Rust emits every binding through compile_fds over that
same descriptor IR. Descriptor census and compatibility baselines cover the complete
source set. The packages compile and round-trip in Rust and Python: (1)
`codefabric.cpgd.v1` `CpgQueryService` in the AC-G-58
nine-RPC form (unary `StartQuery`, streaming `StreamQuery`/`AttachQuery`,
`QueryEvent` closed oneof with the five AC-G-58 variants, `FreshnessPolicy`
enum verbatim from Serving §9 (AC-G-58 names only a "structured freshness
policy" — audit C-16), message field sets per AC-G-58, 4 MiB/1 MiB caps, `identity|zstd`
compression fields, idempotency-key fields); (2) `codefabric.provider.v1`
provider-control package realizing AC-G-32 (job spec/accepted/events/cancel,
run-state enum from the ontology `ProviderRunState` registry (Fact Gen
§85 restates it; AC-G-32 names no registry — audit C-19), supersession
keys, credit-control
constants fixed by Fact Generation AC-G-36: four chunks and 16 MiB); (3) `codefabric.pyrefly.v1`
sidecar package realizing AC-G-30 (six operations, Hello/HelloAck fields,
credit flow, strictly ordered event stream, ObservationBatchChunk with Arrow
IPC payload references); (4) `codefabric.rustc.v1` extractor package realizing
AC-G-31 (env-var handshake constants, CompilationAccepted→…→CompilationEnd
events, owner records, rejection-rule error codes). File names and locations
are fixed by AC-G-05 (`contracts/rpc/{cpg_query_service,provider_control,
pyrefly_sidecar,rustc_extractor}.proto` + `feature-registry.yaml`);
package/service names and provider-event mappings for (2)–(4) are fixed by
Fact Generation §90 and AC-G-30/31. Message/field numbers are instantiated from
those owner schemas and require owner acceptance before code generation. The feature registry (an AC-G-05
artifact) backs the handshake feature bits negotiated under the AC-G-03
per-family compatibility matrix (AC-G-03 states the posture; it does not
name the artifact — audit C-4).

#### Dependencies

WP05 (toolchain), WP06a (compilation-unit graph), WP08
(enums/errors).
#### Target Invariants

I-08, I-10, I-11, I-20, I-21, I-24, I-26. Doctrine
P14, P16, P25, P27, P29.
#### Design and Library References

Serving §8–10, AC-G-58; Fact Gen
AC-G-30/31/32/33; roadmap §6 WP6; LD-10.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/rpc/*.proto` and `feature-registry.yaml` are current drafts; the committed Wave-0
FDS/census/compatibility/toolchain outputs and bindings prove the one-FDS substrate only.
The WP06a production units, complete four-domain bindings, round-trip suites, and bounded
production decode targets do not yet exist.
##### Preflight Query

run `just proto-repro-check`; inspect the compiled descriptor census and
  compatibility baseline; verify every production source is a resolved unit input and
  every binding is a declared unit output.

#### Required Changes

Replace the four 11-line draft authorities with owner-approved
messages/services/options/reservations; generate the complete FDS and bindings through
the unit; update descriptor census and reviewed compatibility baseline; preserve unknown
fields, presence, oneofs, enum/reservation rules, status/deadline/limit behavior, and
cross-language wire fixtures. Generated text equality is supplemental, never the semantic
oracle.

#### Legacy Disposition and Decommission

DB03 and DB05 prohibit a second compiler, ambient protoc,
per-domain source interpretation, suite-self ownership, and hard-coded Wave-0 filenames.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp10_behavioral_acceptance` in the packet's focused test target.

encode→decode round-trip fixtures pass in Rust and Python for
  representative messages of all four packages; a loopback UDS echo test for
  `CpgQueryService.Handshake` between a stub Rust server and the Python
  client (transport probe only — no daemon semantics).
##### Structural

Executable oracle: `wp10_structural_acceptance` in the packet's focused test target.

`QueryEvent` oneof is closed with exactly the five variants;
  sequence fields u64; freshness enum matches the canonical request enum.
##### Negative / Zero-State

Executable oracle: `wp10_negative_zero_state` in the packet's focused test target.

the superseded Serving §9 seven-RPC form does not appear
  (`ExecuteQuery` absent — grep gate); unknown required feature bits fail the
  handshake fixture.
##### Operational

Executable oracle: `wp10_operational_acceptance` in the packet's focused test target.

`just proto-check` and `just proto-repro-check` prove current committed
  outputs and two-root byte identity; descriptor/DescriptorPool assertions prove semantic
  equivalence independently of generated source text.
  Bounded protocol corpus replay is deterministic and retains crashes.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

`just compilation-units-check`; `just proto-check`; `just
proto-repro-check`; root/extractor/sidecar/adapter compile gates; round-trip suites;
bounded four-package protocol decode fuzz replay.
#### Integration Milestone

M02.
#### Replan Triggers

LD-10 probe failures; AC-G-30/31 prose→proto mapping
uncovers a contradiction (e.g., AC-G-31's fd-based channel vs gRPC framing —
the extractor package is length-delimited framing over fd/UDS, **not** gRPC;
if prost framing proves unsuitable, plan revision on the framing layer only).
#### Rollback or Recovery

Additive.

### WP11 — Bundles, deployment profile, CF-ID traceability

#### Outcome

The landed deny-unknown-fields AC-G-07 bundle model is populated for all
eight bundle manifests
(ontology, schema, provider, derivation, query-language, tool-contract,
toolchain — carrying LD-12/13/14 pins and domain identities — and model-pack)
with the exact AC-G-07 record shape and two projection rules: artifact canonical bytes
omit only the bundle's own `canonical_digest`/`source_digest`; bundle identity bytes
additionally omit `bundle_digest`/`signature`; artifacts sorted by
`artifact_id`; built-in bundles trusted by shipped digest; Ed25519 reserved
for external model packs); `contracts/manifests/suite-manifest.json` carrying
AC-G-02 metadata for every artifact and detached source identities in the canonical
artifact index; the effective `deployment/local-workstation-v1.yaml` with the
AC-G-08 field set verbatim (sqlite-wal operational store,
`delta-local-filesystem` fact store (audit C-8), network listeners
disabled, overlay journal disabled, symlink
policy, TTLs, default freshness, platform root table); CF-IDs per AC-G-04
(`CF-<ARCH|ONT|GEN|FAB|LIFE|QUERY|SERVE|SEC|TEST>-<4 digits>`, never reused)
recorded as `manifests/requirements.jsonl` machine records (source artifact +
section + normative-text digest + implements + traces_to + verified_by) and
`manifests/traceability.jsonl` supporting the mandatory trace paths; CI
zero-orphan rules per AC-G-04 — all **four** conditions (audit C-2):
orphaned mandatory ontology kinds; schema columns without owning
requirements; query phrases with no executable mapping (satisfied only by
WP08b's schema-valid declarative PlanSpec mappings; placeholders and
`deferred-mapping` fail released verification);
and requirements with no test or explicit `verification_deferred` record.

The toolchain bundle covers **every pinned boundary family**: LD-01–LD-07
data-plane pins, LD-06 gix, LD-08–LD-10 at their pinned versions, LD-11
adapter pins, LD-12 extractor toolchain (identity/digest records emitted by
WP02 in Wave 0), LD-13 Pyrefly source digest (emitted by WP03), LD-14
provider pins recorded-not-adopted. `manifests/deployment-profile.schema.json`
(the schema the profile instance validates against) is authored here.

#### Dependencies

WP06, WP07, WP08, WP08b, WP09, WP10.
#### Target Invariants

I-08, I-11, I-17, I-19, I-20, I-24, I-26. Doctrine
P10, P25, P27, P29, P31.
#### Design and Library References

Spec §0.5; roadmap §6 WP7–8; AC-G-14
digest fields (context bundle inputs).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/manifests/**`, eight closed-but-empty draft bundles,
`contracts/deployment/local-workstation-v1.yaml`, and current negative fixtures are
present. Populated members, release/deployment completion, CF-ID closure, and the final
traceability oracle remain incomplete.
##### Preflight Query

none external.

#### Required Changes

Replace every empty draft `artifacts` array with the complete sorted
compatibility-sensitive member set; populate compatibility and created-by records from
typed catalog/unit/toolchain data; finish deployment, CF-ID requirements, traceability,
consumer/provenance joins, toolchain identity, and zero-orphan validation. No bundle
member or digest is handwritten in a second authority.

#### Legacy Disposition and Decommission

DB02–DB05 remain green. Empty released bundles, generic bundle
values, suite-self output ownership, and detached hand-maintained trace tables are
prohibited.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp11_behavioral_acceptance` in the packet's focused test target.

`codefabric-contracts verify --profile full` green and
  `--profile released` warning-free: registry invariants, schema digests,
  KAT vectors, proto round-trips, bundle digests, trace zero-orphan.
##### Structural

Executable oracle: `wp11_structural_acceptance` in the packet's focused test target.

every bundle pinned by digest; profile instance validates
  against `deployment-profile.schema.json`.
##### Negative / Zero-State

Executable oracle: `wp11_negative_zero_state` in the packet's focused test target.

the committed broken-trace-edge fixture fails verify on every
  run.
##### Operational

Executable oracle: `wp11_operational_acceptance` in the packet's focused test target.

`just contracts-repro-check` remains byte-identical; Gate A is the
  released-profile verifier itself, not a hand-produced evidence bundle.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

`just compilation-units-check`; `just contracts-verify`; `just
contracts-verify-released`; `just contracts-repro-check`; `just
adapter-contracts-check`; `just proto-repro-check`; `just ci-fast`.
#### Integration Milestone

M02 (closes Wave 1).
#### Replan Triggers

Governance manifest appears with conflicting AC-G-01–08
content → plan revision (R-01).
#### Rollback or Recovery

Additive.

---

## 9. Work packets — Wave 2 (Daemon kernel, workspace registry, path security, source images)

### WP12 — Daemon lifecycle kernel: process, config, singleton lease, discovery file

#### Outcome

`codefabricd` exists as a bin target of the root package:
`codefabricd serve --config <path>` and `check-config`, plus the administrative
shell `codefabric daemon status|stop|drain`; TOML configuration in
the three AC-G-62 tiers (static / reloadable / workspace-admin-only); the §75
singleton lease sequence (lock → endpoint tempfile → fsync → atomic rename →
serve → retire on joined shutdown); the private `daemon.json` discovery file
with exactly the AC-G-62 field set and nothing secret; Tokio runtime with the
§113 posture (small I/O/orchestration worker pool, bounded blocking classes
scaffolded); §151 shutdown ordering skeleton (the steps that exist in Wave 2:
mark STOPPING, close ingress, await workers, close durable stores, retire
endpoint metadata, release lease); daemon liveness distinct from workspace
readiness (AC-G-28). Wave-2 `drain` rejects new administrative ingress,
observes that no update/query work exists yet, checkpoints SQLite, completes
the joined shutdown order, and exits within a tested deadline. Credentials,
service-manager installation, and populated-work overlay/query drain remain
staged AC-G-62 obligations named in §16.

#### Dependencies

WP11 (M02 certifies the generated config/status/error contracts).
#### Target Invariants

I-07, I-13 (status dimensions separate). Doctrine P8,
P22, P23.
#### Design and Library References

Lifecycle §75, §109.1, §113, §151,
AC-G-62; roadmap §7 WP1. LD-05, LD-09.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/rpc.rs`, the Wave-0 generated transport probe, adapter daemon-channel substrate,
`contracts/schema/public-status.schema.json`, and the deployment profile are current.
Daemon lifecycle modules, `codefabricd`, and daemon recipes do not yet exist.
##### Preflight Query

none material — state/runtime/config roots are fixed by
  AC-G-08's platform table (macOS: state root
  `~/Library/Application Support/CodeFabric`, config root
  `~/Library/Application Support/CodeFabric/config` — audit C-8 — and a
  private short-path directory under `$TMPDIR` for runtime; Linux: XDG
  roots). Verify the macOS `$TMPDIR` path stays under the UDS
  `sockaddr_un` length limit.

#### Required Changes

Implement the named daemon lifecycle, configuration tiers, lease/discovery protocol, administrative status/stop/drain path, joined shutdown, private permissions, and metrics from generated contracts. Consume catalog-generated configuration/status/error types; do not add an alternate configuration schema or network listener.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp12_behavioral_acceptance` in the packet's focused test target.

second daemon start against the same state root fails the
  lease; `check-config` validates and rejects tier violations; clean shutdown
  leaves no lease/tempfile residue; `daemon.json` appears/retires atomically;
  status reports liveness without readiness, stop joins shutdown, and no-work
  drain rejects new ingress, checkpoints, and meets the deadline.
##### Structural

Executable oracle: `wp12_structural_acceptance` in the packet's focused test target.

config fields map 1:1 to the generated profile schema.
##### Negative / Zero-State

Executable oracle: `wp12_negative_zero_state` in the packet's focused test target.

`daemon.json` contains no token/root-path/secret fields (test
  asserts the exact field set); no network listener sockets opened; the
  daemon refuses group/world-writable state, runtime, or config roots and
  creates them `0700` with private files `0600` (AC-G-08).
##### Operational

Executable oracle: `wp12_operational_acceptance` in the packet's focused test target.

startup/shutdown traced via `tracing` with the §151 step
  names.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check.
#### Integration Milestone

M03.
#### Replan Triggers

None specific.
#### Rollback or Recovery

Bin target is additive.

### WP13 — Operational-state store (SQLite WAL)

#### Outcome

One SQLite database per daemon state root, opened with the
AC-G-27 pragma set verbatim (`journal_mode=WAL`, `synchronous=FULL`,
`foreign_keys=ON`, `trusted_schema=OFF`, `secure_delete=FAST`,
`busy_timeout=5000`, `wal_autocheckpoint=1000`); numbered forward-only
transactional migrations preceded by an online backup, with
refuse-to-open-newer-schema; the Wave-2 table set: §130's named tables
(`worktree_state` keyed by `workspace_id`, `git_state_vector`,
`git_operation_run`) plus the AC-G-27 persisted domains §130 leaves
without schemas — workspace registration, credentials metadata,
generation counters — and nested-root exclusion records, all generated from WP09 DDL;
the coordinator-sole-writer discipline (writer connection owned by the
coordinator actor; separate read connections for status).

#### Dependencies

WP12; WP09 (schemas).
#### Target Invariants

I-08, I-13, I-14. Doctrine P19 (durable vs temporal
truth), P22, P24.
#### Design and Library References

Lifecycle §130–131, AC-G-27; roadmap §7
WP2. LD-08.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/compatibility.rs` contains the current `rusqlite` backup/WAL feasibility probe.
No operational store module, migration set, or generated production DDL exists yet.
##### Preflight Query

confirm the exact WP01-selected rusqlite with
  `bundled`,`backup`; WAL +
  `BEGIN IMMEDIATE` behavior with one writer + N readers under load;
  `wal_autocheckpoint` interaction with long-lived read snapshots.

#### Required Changes

Generate and consume SQLite DDL from WP09, implement exact pragmas, transactional forward migrations with backup, sole-writer ownership, read connections, retention, recovery, and registered fault points. No handwritten schema may diverge from the generated DDL.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp13_behavioral_acceptance` in the packet's focused test target.

migration up from empty; reopen after crash mid-transaction
  recovers; newer-schema refusal test; retention cleanup preserves the §131
  protected classes; `rusqlite::backup` copies a live WAL database with an
  active reader, restores into a fresh database, and migration failure leaves
  source and restored logical state coherent.
##### Structural

Executable oracle: `wp13_structural_acceptance` in the packet's focused test target.

pragma assertions read back at open; table shapes match
  generated DDL digests.
##### Negative / Zero-State

Executable oracle: `wp13_negative_zero_state` in the packet's focused test target.

a second writer connection attempt is rejected by the store
  API (structural discipline, not SQLite enforcement — asserted in code);
  high-volume payload classes (source bytes, Arrow rows) have no tables.
##### Operational

Executable oracle: `wp13_operational_acceptance` in the packet's focused test target.

backup file produced before each migration; store fault
  points (crash mid-transaction, crash mid-migration) registered per §4.1.
  All write transactions use `TransactionBehavior::Immediate`.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check.
#### Integration Milestone

M03.
#### Replan Triggers

WAL/latency probe shows the sole-writer model cannot
meet the §131 atomic wave+pointer transaction under contention → plan
revision of connection topology (still SQLite; AC-G-27 excludes
alternatives).
#### Rollback or Recovery

Code revert **plus** state-root disposal or restore from the
pre-migration backup — forward-only migrations mean a reverted binary
refuses a newer schema (applies to every packet downstream of WP13:
WP14–WP18, WP19–WP25 all treat the daemon state root as disposable-per-test
and restorable-from-backup in development).

### WP14 — Workspace registry, administrative lifecycle, and identity

#### Outcome

The AC-G-10 admin surface (`codefabric workspace
add|list|show|relink|configure|enable|disable|reconcile|remove
[--retain-data|--purge-data]`) implemented as a local admin CLI speaking a
private admin IPC to the daemon (same-OS-user only, distinct from the future
query RPC); the AC-G-10 registry state machine and the §18 lifecycle machine
generated from WP08 state-machine YAML with `STATE_TRANSITION_VIOLATION`
enforcement (D-06); AC-G-09 identity: 128-bit registration nonces,
`workspace_id`/`repository_id`/`worktree_id` CBEF preimages, worktree
administrative keys with duplicate-active rejection, `registration_revision`
monotonicity, the identity outcome table honored (move/relink preserves,
copy/re-register mints); authorization fingerprints computed over the
AC-G-11 root-authorization record via CBEF; nested-root
registration writes the mandatory parent subtree exclusion.

#### Dependencies

WP13 (persistence), WP07 (preimages).
#### Target Invariants

I-01, I-13. Doctrine P8, P12, P20 (all mutation via
the admin command path), P21.
#### Design and Library References

Lifecycle §18, AC-G-09/10; roadmap §7
WP3.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

The draft registry/state-machine authorities under `contracts/registry/` and identity
contracts under `contracts/identity/` are current. No workspace registry, admin module,
admin CLI, or WP13 production store table exists yet.
##### Preflight Query

admin IPC mechanics (UDS with peer-uid check) — reuse the
  LD-10 transport probe results.

#### Required Changes

Implement the generated registry/lifecycle machines, admin IPC/CLI, persisted identities/revisions, nested-root exclusions, audit rows, and all legal/illegal transitions. All IDs and statuses come from WP07/WP08 contracts.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp14_behavioral_acceptance` in the packet's focused test target.

full state-machine walk
  REGISTERING→DISABLED→OPENING→BOOTSTRAPPING→DISABLING→DISABLED→
  REMOVING→REMOVED with persistence across restart; relink with Git identity
  proof; `--purge-data` double-confirmation + active-lease refusal. A
  model-level transition fixture proves only `first valid snapshot activated`
  can move BOOTSTRAPPING→READY, but Wave-2 runtime never emits that event.
##### Structural

Executable oracle: `wp14_structural_acceptance` in the packet's focused test target.

IDs equal the CBEF KAT-derivations for fixed nonces; two
  linked worktrees of one repo yield distinct `workspace_id`s sharing
  `repository_id`; non-Git root has null Git identities (no synthetic
  repository).
##### Negative / Zero-State

Executable oracle: `wp14_negative_zero_state` in the packet's focused test target.

re-registering a removed workspace mints a new ID; duplicate
  active administrative keys rejected; illegal transitions raise
  `STATE_TRANSITION_VIOLATION`; the query surface exposes no admin verbs
  (structural: admin service bound to a separate socket).
##### Operational

Executable oracle: `wp14_operational_acceptance` in the packet's focused test target.

every admin mutation writes an audit row.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check; just mutants-file for the changed generated
transition module.
#### Integration Milestone

M03.
#### Replan Triggers

None specific.
#### Rollback or Recovery

Additive.

### WP15 — Root authorization, secure open, and path identity runtime

#### Outcome

The AC-G-11 discipline is implemented and fixture-proven:
root-authorization record (all eight fields), the 8 mandatory checks on every
workspace-relative path, component-wise safe `rustix` opening (Linux
`openat2` with `RESOLVE_BENEATH|NO_MAGICLINKS|NO_SYMLINKS|NO_XDEV` +
`openat`/`O_NOFOLLOW` fallback;
macOS directory-relative opens + `fstat` no-follow walk), directory symlinks
never followed, differing-device mount denial, root-identity revalidation
(authorization change → `VERIFYING` trust), `WorkspacePath`/`PlatformPath`/
`GitRepoPath` runtime types wired to WP07 canonicalization, comparison-key
collision → `BLOCKED_PATH_COLLISION`, AC-G-12 `file_id` derivation.

#### Dependencies

WP07 (path contracts), WP13 (records).
#### Target Invariants

I-01, I-03 (path leg). Doctrine P8, P11 (parse at the
boundary), P12.
#### Design and Library References

Lifecycle §43–§45, AC-G-11; Ontology
AC-G-12/18; roadmap §7 WP4–5; LD-16.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/compatibility.rs` contains the current descriptor-relative `rustix` open probe;
identity/path contracts and the security-corpus manifest are current. No production
secure-open/path module or adversarial runtime corpus exists yet.
##### Preflight Query

`openat2` availability probing (CI Linux kernel) with fallback
  path both tested; macOS volume case-sensitivity probe from WP07 reused.

#### Required Changes

Implement the safe descriptor-relative authorization boundary with generated path types, root revalidation, collision blocking, platform-specific no-follow behavior, and complete adversarial fixtures. All authoritative byte reads route through this port.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp15_behavioral_acceptance` in the packet's focused test target.

authorized files open and read byte-exact through the secure
  path using safe `rustix` descriptor-relative operations returning
  `OwnedFd`; case-only rename on case-insensitive volume preserves
  comparison-key identity. Linux uses `NO_XDEV`; fallback platforms compare
  device and descriptor identity before and after reads.
##### Structural

Executable oracle: `wp15_structural_acceptance` in the packet's focused test target.

every **authoritative source-byte** read routes through the
  secure-open module. AST positive/negative fixtures cover direct
  `std::fs::{read,read_to_string,File::open,OpenOptions}` and equivalent
  path-based opens, not one symbol. First-party unsafe remains denied. gix
  internal path reads are advisory only and their derived identity is
  revalidated before authority use.
##### Negative / Zero-State

Executable oracle: `wp15_negative_zero_state` in the packet's focused test target.

on local macOS, escaped symlink, mid-path
  symlink swap, `..` and absolute injections, NUL bytes, device/drive
  prefixes, nested-mount escape, root-identity swap, comparison-key collision
  → all rejected with the registered error codes; display strings never
  accepted as identity (type-level: display fields are non-constructible into
  path identity).
##### Operational

Executable oracle: `wp15_operational_acceptance` in the packet's focused test target.

rejections emit diagnostics with stable codes.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check. The latter includes the adversarial suite on
local macOS. Equivalent supported-Linux evidence remains deferred until the user
reprioritizes external Linux assurance.
#### Integration Milestone

M03.
#### Replan Triggers

macOS lacks a no-follow-equivalent for a required check
→ design issue to Lifecycle AC-G-11 with interim strictest-available
behavior.
#### Rollback or Recovery

Additive.

### WP16 — Source images, blob store, inventory, and generations

#### Outcome

Source-image capture per Lifecycle §33 (7-step capture fence)
+ AC-G-33's **nine-step** stable-read algorithm (Fact Gen owns AC-G-33;
steps 8–9 add the line-index artifact and a source-snapshot lease record,
persisted via the WP13 store — audit C-20) with metadata fencing and
retry/defer (retry count 3 is an Appendix-B starting value,
benchmark-adjustable), BLAKE3-256 digests, content-addressed immutable blob store (temp-write,
fsync, mode 0400, atomic rename; blob names are content hashes), size caps
(16 MiB ordinary / 64 MiB explicit), `u64` line-index artifact + digest,
encoding classification (UTF-8 requirement recorded for Rust; BOM/PEP-263 for
Python; undecodable → explicit unsupported-encoding capability entry),
`SourceImage`/`SourceSnapshot` DTOs per generated schemas; persisted
`source_generation` rules (increments per accepted coherent wave; restart
never resets; rebuilt state uses a new generation — AC-G-28); the bounded
generic inventory walker (all six bound dimensions configurable, values
recorded as deployment-profile defaults), §46 inclusion classification enum, Merkle
inventory digest (mandatory here despite §34.3 SHOULD — it feeds
`GitStateVector.worktree_inventory_digest`); rename/identity policy
§35/§45 evidence hierarchy (operational continuity only, never canonical
identity). This packet also owns the complete source-blob lease lifecycle:
typed holder kinds (provider run, source artifact, serving snapshot),
acquire/renew/release, restart orphaning with deployment-profile grace,
atomic delete eligibility, and idempotent bounded garbage collection.

#### Dependencies

WP13, WP15.
#### Target Invariants

I-03, I-14. Doctrine P11, P13, P24, P25.
#### Design and Library References

Lifecycle §33–§36, §45–§47.1, AC-G-28,
AC-G-33; Fact Gen §8–§9; roadmap §7 WP6–7.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

The deployment profile, security-corpus manifest, and fault-point registry are current
contract touch points. No source-image, inventory, blob-store, lease, or GC production
module/table exists yet. The packet will also update
  `contracts/deployment/local-workstation-v1.yaml` overrides section (walker
  bound defaults — a contract edit, so regeneration +
  `just contracts-verify` join this packet's gates per §4.1 item 7);
  `contracts/security/security-corpus-manifest.yaml` (register the
  capture-race harness); `contracts/faults/fault-point-registry.yaml`
  (lease/GC/restart points).
##### Preflight Query

concurrent-mutation harness design (a writer process
  rewriting files during capture) — must exist before acceptance, with a
  numeric criterion: ≥10,000 capture attempts against an active rewriter at
  three file sizes (1 KiB, 1 MiB, 15 MiB), zero falsely-stable images
  (digest mismatch between published image and any full quiescent re-read),
  RNG seed recorded for replay.

#### Required Changes

Implement bounded stable capture, immutable content-addressed blobs, line indexes, inventory/generation fences, holder leases, restart orphaning, and idempotent bounded GC. Register budgets, security corpus entries, and fault points through the catalog.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp16_behavioral_acceptance` in the packet's focused test target.

byte-exact capture round-trip; retry/defer on mutation; line
  index correct on LF/CRLF/mixed/empty/no-trailing-newline files; walker
  respects every bound + cancellation; a live holder prevents deletion, all
  holders released leads to eventual deletion, and restart recovers or safely
  orphans holders until grace expiry.
##### Structural

Executable oracle: `wp16_structural_acceptance` in the packet's focused test target.

blob paths are digests; blobs immutable (mode assertions);
  inventory rows carry all §34 fields.
##### Negative / Zero-State

Executable oracle: `wp16_negative_zero_state` in the packet's focused test target.

**concurrent mutation during capture never yields a falsely
  stable image** (fuzz-style harness, the Wave 2 exit's hardest clause);
  oversized files yield explicit capability entries, not silent skips; `.git`
  never inventoried as source; concurrent release/GC cannot delete a live
  blob and repeated cleanup is idempotent.
##### Operational

Executable oracle: `wp16_operational_acceptance` in the packet's focused test target.

capture/walk/lease/GC metrics (files, bytes, retries,
  duration, live/orphan holders, reclaimed blobs/bytes).
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check.
#### Integration Milestone

M03.
#### Replan Triggers

Stable-read fencing insufficient on APFS (mtime
granularity) → strengthen with content re-hash comparison; record deviation.
#### Rollback or Recovery

Additive.

### WP17 — Read-only gix discovery and Git topology

#### Outcome

A `git_state` module (boundary per D-03: the only module that
sees `gix` types) providing `GitStateAdapter` per Lifecycle §156 Wave-2
subset (`open_worktree`, `capture_state`, `inventory`), returning detached
DTOs (`GitRepositoryIdentity`, `GitWorktreeIdentity`, `GitStateVector`,
`GitInventoryResult`); exact-path open with the trust policy (hooks,
filters, credentials, network, repository mutation, checkout, and external
commands disabled; only CodeFabric and repository-local configuration
accepted; environment/global/system overrides rejected per revised §76);
repository kind/bare detection, work/git/common
dirs, worktree enumeration with administrative names, HEAD kind/target/tree,
operation state, object format; Git-native inventory classification feeding
WP16's inventory (tracked/untracked/ignored/conflicted; ignore rules are
inclusion policy, never authorization). Lifecycle §76 now owns the strict
policy values: CodeFabric and repository-local configuration only,
environment/global/system overrides rejected, attributes/excludes used only
for classification, and external commands disabled. A bounded blocking execution class
(Tokio coordinator → semaphore → blocking gix job → DTO) with interruption;
`GitAccelerationStatus` handling with generic-walker fallback (§80: gix
failure degrades acceleration, never correctness).

**Wave-boundary note (roadmap §7 deferred list).** This packet stops at
discovery, topology, state capture, and inventory classification. Explicitly
**out**: status/tree-diff candidate deltas (`status_candidates`,
`tree_diff_candidates` remain unimplemented trait stubs), rename-candidate
detection, warm-start pruning, blob-OID caches, and the cache hierarchy —
all Wave 7. `GitStateVector` capture and Git-native inventory classification
are Wave-2 obligations, not acceleration: cold start captures G0/G1
(Lifecycle §5.1 steps 6–7), the rescan fence needs the vector (§36), warm
start verifies HEAD/index/inclusion fingerprints (§5.2, AC-G-28), and §34.1
requires gix pathspec/exclude/attribute/dirwalk semantics for inventory.

#### Dependencies

WP15 (paths), WP16 (inventory integration), WP12 (runtime).
#### Target Invariants

I-03, I-10. Doctrine P6, P8, P22.
#### Design and Library References

Lifecycle §37–§44, §50, §69–§73, §76,
§78–§80, §109.6 (bounded blocking execution class — audit C-23), §156;
roadmap §7 WP7. LD-06 (all caveats).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/git_state.rs` is the current isolated gix compatibility boundary; its focused
integration test and the gix boundary governance rule are current. Production topology,
state capture, inventory, and interruption behavior do not yet exist.
##### Preflight Query

  1. **Linked-worktree exact-path open**: probe `gix::open` on a linked
     worktree work-dir; if per-worktree git-dir/HEAD/index resolution is
     wrong, route through owning-repo `worktrees()` enumeration (the
     reference's sanctioned path) — implementation adaptation.
  2. **Index fingerprint**: probe gix 0.86 rustdoc for a checksum/state
     identity; fallback: BLAKE3 over sorted `(path, oid, stage, mode)` entry
     tuples with cost measured; if cost is prohibitive on large indexes,
     stat-based fingerprint + design issue (R-04).
  3. **Write/lock freedom**: filesystem-snapshot probe — hash every file
     under `.git/` (paths + digests) before and after open + inventory +
     state capture; the trees must be identical and no `*.lock` may appear
     (portable to macOS; Linux CI may additionally strace). Wave 2 exit
     requires no locks left behind.
  4. **`revision` feature need:** verify `head_id`/`head_commit`/
     `head_tree_id` resolve HEAD without the `revision` feature at the
     pinned feature set; if not, add `revision` with a recorded deviation.
  5. **SHA-256 behavior (LD-06):** construct real SHA-1 and SHA-256 fixture
     repositories and exercise open, topology, object-format, and algorithm-
     tagged ID widths through `GitStateAdapter`. Feature presence is not
     sufficient evidence. Incomplete SHA-256 support fails closed and triggers
     a design-owned typed unsupported-format contract before WP17 can close.

#### Required Changes

Implement GitStateAdapter only at the gix boundary, run all five compatibility probes, return detached DTOs, apply the strict trust policy, and fall back to the generic walker without weakening authoritative-byte rules.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp17_behavioral_acceptance` in the packet's focused test target.

fixture repos (bare, main worktree, two linked worktrees,
  merge-in-progress, detached HEAD, unborn branch, submodule pointer,
  non-UTF-8 path, SHA-256 object format) yield correct identity/state DTOs;
  interruption cancels a long inventory.
##### Structural

Executable oracle: `wp17_structural_acceptance` in the packet's focused test target.

governance rule zero-hit outside the boundary;
  `GitStateVector` fields populated per §50 with fingerprints from WP16/WP07.
##### Negative / Zero-State

Executable oracle: `wp17_negative_zero_state` in the packet's focused test target.

repository byte-identical after all read operations (probe 3
  as a repeatable test); mutation API usage absent (`ast-grep` rule for
  `edit_reference|write_object|checkout` symbols); external command execution
  disabled (trust-policy assertion + no `command` invocation paths).
##### Operational

Executable oracle: `wp17_operational_acceptance` in the packet's focused test target.

gix job metrics (queue depth, duration, interruptions).
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check. Its Git fixtures remain a test-support module,
not a crate.
#### Integration Milestone

M03.
#### Replan Triggers

Probe 1 or 2 fails with no viable adaptation → plan
revision (WP17 scope) + design issue to Lifecycle §41/§50. `revision` feature
needed for HEAD resolution → add the feature with a recorded deviation.
SHA-256 fixture behavior incomplete → return the support classification and
typed error to the Lifecycle owner; do not claim parity from the feature flag.
#### Rollback or Recovery

Additive module behind the boundary.

### WP18 — WorkspaceCoordinator actor, bootstrap-without-watchers, pre-ready health

#### Outcome

One coordinator task per `workspace_id` (bounded command
channels, sole mutator of workspace state — §110), owning lifecycle/trust/
health/acceleration dimensions and generation counters per
`WorkspaceCoordinatorState` (Wave-2 form: `active_snapshot` is the explicit
AC-G-28 `NO_SNAPSHOT` startup state); cold-start bootstrap without watchers (§5.1 steps 1,
3–10 with watcher registration replaced by an explicit
event-stream-unavailable status; §154 readiness barrier with
`source_trust_state = CURRENT` after an authoritative inventory (§154's
"Git acceleration is CURRENT" clause maps to `GIT_READY` — the enum has
no `CURRENT` member). Source-control-plane health is exposed as an
orthogonal status while workspace lifecycle remains `BOOTSTRAPPING`; any
query attempt returns `WORKSPACE_BOOTSTRAPPING`. Warm restart per AC-G-28 (registration +
inventory restored; no fact-snapshot claim; `source_generation` never
resets); rescan generation fence (§36) in its no-watcher form (W0/W1
watermarks trivial, G0/G1 GitStateVector fencing real); admin diagnostics
(`workspace show`, health surface §150 subset without credential/config
leakage).

#### Dependencies

WP12–WP17.
#### Target Invariants

I-01, I-03, I-13. Doctrine P17, P19, P22, P23.
#### Design and Library References

Lifecycle §5.1–5.3, §26, §36, §110, §112
(Wave-2 limits: concurrent source reads, concurrent gix jobs), §154, §150,
AC-G-28; roadmap §7 WP1/WP8.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/rpc.rs`, the adapter channel substrate, public-status schema, deployment profile, and
fault-point registry are current. Coordinator, bootstrap/readiness modules, admin status
verbs, and WP13/WP14 production rows do not yet exist.
##### Preflight Query

none beyond WP17's resolved probes.

#### Required Changes

Implement one coordinator mutator per workspace, generated lifecycle/trust/health transitions, bootstrap fences, restart recovery, bounded queues/jobs, and the pre-ready health surface. READY remains impossible until WP24 activates a valid frozen snapshot.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp18_behavioral_acceptance` in the packet's focused test target.

register→enable→bootstrap→BOOTSTRAPPING with source-control-
  plane health for (a) non-Git root, (b) main worktree, (c) two linked
  worktrees as distinct workspaces; restart restores the pre-ready state and
  re-verifies rather than trusting; G0≠G1 during bootstrap triggers
  reconcile-before-ready.
##### Structural

Executable oracle: `wp18_structural_acceptance` in the packet's focused test target.

exactly one mutator task per workspace (deterministic
  assertion test on the command-channel discipline: all mutations flow
  through the single coordinator receiver; no loom dependency is added);
  startup states follow the AC-G-28 vocabulary.
##### Negative / Zero-State

Executable oracle: `wp18_negative_zero_state` in the packet's focused test target.

no strict-current claim while trust ≠ CURRENT; restart never
  fabricates an active snapshot or reports `READY`; the invariant
  `READY => active frozen snapshot exists` is model-checked; second
  coordinator for the same workspace cannot spawn.
##### Operational

Executable oracle: `wp18_operational_acceptance` in the packet's focused test target.

health endpoint fields (§150 subset) exposed via admin CLI.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave2-integration-check. WP18 assembles the Wave-2 exit evidence.
#### Integration Milestone

M03 (closes Wave 2).
#### Replan Triggers

Actor/channel model cannot satisfy §131's atomic
wave+pointer transaction shape when Wave 3 arrives → plan revision at WP24.
#### Rollback or Recovery

Additive.

---

## 10. Work packets — Wave 3 (Canonical data fabric, publication, overlay, snapshot kernel)

### WP19 — Schema-registry runtime, Delta namespace, control-plane tables

#### Outcome

The generated `TableSpec` set loads at daemon start with
schema-digest validation (`SCHEMA_DIGEST_MISMATCH` on drift); the §11.1
round-trip gate (Arrow Schema → Delta StructType → create → open → DataFusion
provider schema → Arrow → exact comparison) passes for every Wave-3 table;
per-workspace Delta namespace `/cpg/<workspace-id>/{control,facts,derived}/`
under the daemon storage root; table creation per §67 (comment, schema +
ontology version metadata, partitions, `columnMapping.mode = none` asserted,
CDF disabled, type widening off) followed by `ConstraintBuilder` commits for
the §102 row-local checks while tables are empty (LD-04); control-plane
tables created and wired to the registry/coordinator flows: `workspace`,
`common_repository`, `analysis_context` (+`analysis_context_set` with
`context:source` seeded), `publication`, `publication_table`,
`current_publication`, `owner`, `capability_status`, `diagnostic`,
`enum_catalog` dimension mirror (§8's optional MAY-mirror, adopted; not a
§13 control-plane table — audit C-10).

#### Dependencies

WP09 (generated schemas), WP12 (daemon), WP13 (store), WP14
(workspace rows), and WP18 (coordinator integration/M03).
#### Target Invariants

I-05, I-08, I-12, I-16. Doctrine P8, P16, P29, P31.
#### Design and Library References

Data Fabric §6, §8, §10–§13, §67, §102,
§104 (bootstrap steps 1–4); roadmap §8 WP1–2. LD-01/02/03/04.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`src/fabric.rs` and `src/compatibility.rs` contain only the current DataFusion/Delta
feasibility seams; draft schema contracts are current. WP19 replaces those
compatibility-only calls with the
  first production use of LD-01/02/03/04; confirm the WP01 `deny.toml`
  exact-rev delta-rs exception remains no broader than the locked source;
  toolchain bundle update per §4.1 item 6. The §2.1 `deltalake` dependency
  block is transcribed in valid TOML table form
  (`[dependencies.deltalake]`) — the spec prints a multi-line inline
  table, because the illustrative multi-line inline form is invalid TOML.
##### Preflight Query

compile probe for `CreateBuilder` configuration keys
  (retention/checkpoint property names — LD-04 caveat; `TableProperty`
  non-exhaustive) and `ConstraintBuilder` expression support for the §102
  checks at the pinned rev. Also, moved forward from WP25 (audit Q7): the
  §91 composition probe — programmatic `ViewTable`/logical-plan view
  construction and the anti-join effective-rows plan over a two-table
  fixture (LD-01; the reference documents only SQL `CREATE VIEW`). WP25's
  custom-`ExecutionPlan` replan trigger fires **here**, before the Wave-3
  chain commits, if both the programmatic view and the thin-provider
  fallback fail.
  Before those API probes, refresh all four WP01 advisory records against the
  current RustSec database and supported upstream releases. Remove any stale
  exception. Any retained exception requires a new explicit design/plan
  decision with current reachability evidence; WP19 cannot silently carry the
  original review boundary forward. Add provider-factory negatives proving
  `local-workstation-v1` rejects cloud schemes, credentials, endpoints, and
  storage-option maps before provider construction.

#### Required Changes

Load generated TableSpecs, validate exact schema digests, create constrained local Delta tables and control-plane tables, build exact-version providers, and enforce the local-only storage profile. Manual schema constants or provider rebinding after freeze are prohibited.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp19_behavioral_acceptance` in the packet's focused test target.

bootstrap creates the full namespace on a fresh workspace;
  reopen validates digests; round-trip gate green per table; enable →
  relink → re-upsert: `cpg_control.workspace.registration_revision` equals
  the operational registry's current revision (D-08).
##### Structural

Executable oracle: `wp19_structural_acceptance` in the packet's focused test target.

every table's Delta metadata carries the §10 keys; partition
  specs match §95; constraints present (verified via table metadata).
##### Negative / Zero-State

Executable oracle: `wp19_negative_zero_state` in the packet's focused test target.

opening a table whose schema digest differs fails closed;
  column-mapping ≠ none fails an invariant check at open; local profile input
  cannot construct or register a cloud provider.
##### Operational

Executable oracle: `wp19_operational_acceptance` in the packet's focused test target.

creation is idempotent (re-run bootstrap = no-op commits).
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just advisory-policy-check; just contracts-verify; just
wave3-integration-check.
#### Integration Milestone

M04.
#### Replan Triggers

A §102 constraint expression unsupported at the pinned
rev → constraint moves to Arrow-validation-only with recorded deviation
(§102 already permits this split).
No compatible upstream advisory resolution at the WP19 review boundary →
design reopening to disable future S3 support, adopt a new baseline, or accept
a newly bounded exception; never an automatic renewal.
#### Rollback or Recovery

Fabric modules additive; tempdir state disposable.

### WP20 — Universal fact core, observation boundary, encoders, batch validation

#### Outcome

`entity`, `relation`, `property_fact`, `fact_evidence` encoders
(typed Arrow builders with capacity hints; §64 starting batch sizes; §65
builder policy — no serde row path in the hot loop); the §66 eleven-check
batch validator (schema exact match, column/row counts, non-null keys, 16-byte
ID enforcement, bucket derivation from digest byte 0, span bounds +
`start<=end`, registered enum codes, owner present, in-batch PK uniqueness —
composed from sort/adjacent-compare kernels + custom vectorized checks per
LD-02/LD-01 grounding); the §63 observation boundary: manifest-precedes-
batches streams, per-stream schema fingerprints, workspace/context/generation
fences, bounded channels with backpressure, terminal
completed/partial/failed manifests, stale-generation rejection
(`SOURCE_SNAPSHOT_MISMATCH`/`STALE_RESULT` codes); the bounded
`SyntheticCanonicalIngest` (D-07) as the only canonicalization ingress —
implementing the §72/§73.1 reconciliation signature (N observation streams
+ provider-precedence input → canonical batches + `fact_evidence` rows +
conflict records; D-07, audit Q4) — consuming synthetic observation
fixtures from `contracts/fixtures/synthetic/` (authored in this packet,
including a conflicting-observation family: two observations of the same
fact range, exercising the evidence and conflict-record legs). Scope note: this is the **fabric-side** ingest
boundary only (Data Fabric §63 — owned by the Wave 3 spec); the
provider-side job runtime (AC-G-32 executors, §90 traits beyond generated
types) is Wave 4.

#### Dependencies

WP19.
#### Target Invariants

I-04, I-05, I-06 (null ≠ unknown enforced by
validator), I-14. Doctrine P11, P16, P17.
#### Design and Library References

Data Fabric §9, §14–§16, §63–§66;
Fact Gen §10–§12, §85–§86; roadmap §8 WP3 + Wave 4 entry dependency
("observation ingestion"). LD-02 caveats (no engine-level PK enforcement —
custom validators are the mechanism).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

Current touch points are the draft schema/registry authorities and compatibility-only
Arrow/DataFusion seams. No fact encoder, batch validator, observation ingress, synthetic
fixture family, or production fabric test suite exists yet.
##### Preflight Query

validator kernel composition benchmark at the §64 batch sizes
  (sort/adjacent-compare throughput on 65,536-row batches).

#### Required Changes

Generate Arrow encoders from TableSpecs; implement the bounded observation boundary, all eleven batch checks, application-owned validation diagnostics, and the production-shaped synthetic reconciliation adapter with evidence/conflict outputs.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp20_behavioral_acceptance` in the packet's focused test target.

synthetic fixture families ingest to exact expected rows;
  the conflicting-observation family materializes `fact_evidence` +
  conflict records through the ingress (Q4); §112.2 batch matrix green (empty batch, one row, all-nullable-null, max
  lengths, invalid ID length, duplicate PK, malformed spans).
##### Structural

Executable oracle: `wp20_structural_acceptance` in the packet's focused test target.

exactly one populated value representation per
  `value_kind_code` (validator test); every fact row carries the §9 metadata.
##### Negative / Zero-State

Executable oracle: `wp20_negative_zero_state` in the packet's focused test target.

a batch bypassing the ingest boundary cannot reach a writer
  (module privacy + governance rule); cross-context/cross-workspace rows
  rejected; stale generation rejected.
##### Operational

Executable oracle: `wp20_operational_acceptance` in the packet's focused test target.

ingest metrics (§111 subset: rows received/encoded,
  validation failures).
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check; just mutants-file for the changed batch
validation or canonical-ingress module.
#### Integration Milestone

M04.
#### Replan Triggers

Validator throughput far below §64 batch-size targets →
performance adaptation (kernel composition), not scope change.
#### Rollback or Recovery

Additive.

### WP21 — Mutation classes, owner replacement, idempotency

#### Outcome

Every table's §68 mutation class recorded in its `TableSpec` and
enforced by the writer layer; the §69 owner-replacement protocol (open →
predicate delete on `owner_id` set → append validated stream → reload →
validate counts/checksum → record Delta version) as two commits with
publication-pointer protection; delete+append as the normative baseline
(merge deferred per §69.1); §106 owner deletion across all owner-scoped
tables; §70 idempotency: `publication_id`/`operation_id`/`table_code`/
owner-set fingerprint/input checksum attached via `CommitProperties`
metadata **and** `with_application_transaction(Transaction::new(app_id,
version))`, using Data Fabric §70's owner-fixed application identity
`codefabric/<workspace_id>/<table_code>/<mutation_phase>` and a
coordinator-persisted monotonic `i64` version. The operation record binds
`operation_id`, table, phase, application identity/version, input checksum,
and expected predecessor. Retry reloads the snapshot, reads
`transaction_version`, reconciles commit metadata and operation state, and
then returns the prior result or advances; blind append retry is structurally
impossible. Per-table application transactions own Delta idempotency;
operation records remain necessary for multi-table orchestration/recovery.

#### Dependencies

WP20.
#### Target Invariants

I-14. Doctrine P24.
#### Design and Library References

Data Fabric §68–§70, §106; roadmap §8
WP4. LD-04.

#### Change Surface

##### Preflight Query

Exact-revision compile/behavior probes cover commit metadata, application
transactions, `Snapshot::transaction_version`, and delete-metrics `Option`
semantics (None = unknown, never zero).

##### Known Touch (verified this session)

`src/fabric.rs` and `src/compatibility.rs` contain the current application-transaction
compatibility seam; no production writer layer exists. WP09's generated `TableSpec` is the
durable-mutation authority. If enforcement finds an incorrect class, correct and regenerate
that owner rather than adding an inline override.

#### Required Changes

Implement each generated durable mutation class, owner replacement/removal, native application transactions, retries, operation records, and deterministic conflict recovery. No table writer may bypass the generated policy.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp21_behavioral_acceptance` in the packet's focused test target.

replace/remove/re-add owner cycles converge; §112.3 retry
  idempotency test (kill between delete and append; retry completes without
  duplication or loss). First commit, same-version duplicate, concurrent
  duplicate, process reload/restart, monotonic advance, and metadata
  persistence all pass against the pinned revision.
##### Structural

Executable oracle: `wp21_structural_acceptance` in the packet's focused test target.

every table has exactly one durable mutation class (from the
  registry); application transaction, commit metadata, and operation record
  carry the §70 key set;
  write-boundary fault points registered per §4.1.
##### Negative / Zero-State

Executable oracle: `wp21_negative_zero_state` in the packet's focused test target.

concurrent second writer to the same table yields a detected
  conflict, never silent duplication; blind-retry API path does not exist.
##### Operational

Executable oracle: `wp21_operational_acceptance` in the packet's focused test target.

owner-replacement latency metric.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check; just mutants-file for the changed
application-transaction or outcome-reconciliation module.
#### Integration Milestone

M04.
#### Replan Triggers

Exact-revision behavior contradicts the inspected
application-transaction API (duplicate/concurrent/reload semantics fail) →
plan revision and Data Fabric owner issue; do not silently promote the
external operation record to a replacement for Delta conflict/history state.
#### Rollback or Recovery

Additive.

### WP22 — Durable publication and the current-pointer protocol

#### Outcome

The §71.1 durable-publication algorithm end-to-end for synthetic
facts: STAGING row → pins (source generation, inventory digest, context set,
fingerprints, bundle versions) → owner replacements → per-table version/
checksum records in `publication_table` → §75 integrity validation (Wave-3
subset — six of §75's sixteen "at minimum" checks: PK uniqueness, 16-byte
IDs, relation endpoints exist, owners exist, span sanity, row counts; the
other ten attach to tables that first exist in Waves 4+ — audit C-13) → VALIDATING→VALIDATED→COMMITTING→COMPLETE → AC-G-26
durable CAS on `current_publication` realized as: exclusive coordinator
publication lease → read pointer at pinned Delta version → predecessor +
generation verification → one-row replace committed from the version-pinned
handle (the pointer table is single-row, so any concurrent commit is
expected to be a *conflicting* change under OCC — the reference documents
conflict detection for conflicting changes only, so this is a
probe-confirmed assumption rather than a documented guarantee) →
post-commit
reopen verifying exactly one row and expected generation → otherwise
`CURRENT_POINTER_CONFLICT` (LD-04; the AC-G-26 text itself names OCC
conflict *or* predecessor mismatch as the failure legs); §107 failed-
publication recovery (active pointer untouched; abandoned versions
unreferenced; same-ID retry where safe).

#### Dependencies

WP21.
#### Target Invariants

I-08, I-14. Doctrine P23, P24.
#### Design and Library References

Data Fabric §12, §13.5–§13.7, §71.1, §75,
§107, AC-G-26; roadmap §8 WP6. LD-04.

#### Change Surface

##### Preflight Query

Concurrency probe: two processes racing the
pointer commit → exactly one wins, loser gets a typed conflict; crash
injection between delete/append/pointer steps.

##### Known Touch (verified this session)

`src/fabric.rs` and `src/compatibility.rs` are the only current Delta transaction seams; no
production publication/current-pointer implementation exists. The executor records exact
new module and test paths after the concurrency preflight.

#### Required Changes

Implement staged multi-table publication, predecessor-pinned OCC, completion validation, conditioned current-pointer advance, idempotent retry, crash recovery, and registered fault points without claiming unsupported cross-table atomicity.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp22_behavioral_acceptance` in the packet's focused test target.

publish → pointer advance → reopen shows the new base;
  validation failure marks FAILED/ABANDONED with pointer untouched.
##### Structural

Executable oracle: `wp22_structural_acceptance` in the packet's focused test target.

`publication_table` rows pin exact versions + checksums;
  states follow `DurablePublicationState` exactly.
##### Negative / Zero-State

Executable oracle: `wp22_negative_zero_state` in the packet's focused test target.

intermediate table versions invisible through serving reads;
  racing pointer writers → one `CURRENT_POINTER_CONFLICT`; crash at each step
  (fault points injected) recovers to a coherent state on restart.
##### Operational

Executable oracle: `wp22_operational_acceptance` in the packet's focused test target.

publication latency + diagnostics recorded.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check; just mutants-file for the changed
predecessor/conflict/retry module.
#### Integration Milestone

M04.
#### Replan Triggers

OCC at the pinned rev does not surface concurrent
pointer-table commits as failures (probe contradicts the deltalake
reference §9.22's OCC model — audit C-14) → plan
revision: pointer moves behind a SQLite-guarded commit sequence +
design issue to Data Fabric AC-G-26 — the spec's own hedge acknowledges
mechanism variance.
#### Rollback or Recovery

Publications are additive; ABANDONED rows are inert.

### WP26 — Snapshot provider/catalog substrate and access-profile factory

*(Added by the v3 audit integration to close the snapshot-order blocker. It separates immutable
snapshot construction from the user-facing views that remain in WP25.)*

#### Outcome

An access-profile-aware Delta handle factory makes every table
open name exactly one of `QUERY_SERVING`, `PUBLICATION_METADATA`,
`APPEND_ONLY_WRITER`, `VACUUM_FILESYSTEM_CHECK`, or `OPTIMIZE_DML`; the
profile owns its `skip_stats` and materialization posture and no unclassified
handle can compile. Given a validated durable
publication and one consolidated overlay value, a candidate builder resolves
every exact Delta version, constructs the version-pinned providers, wraps
them in the supplied overlay (empty generation 0 is valid), registers a
private `CatalogProvider`/`SchemaProvider` object graph, runs schema/version/
checksum/access-profile integrity checks, and freezes the provider/catalog
set for inclusion in `ServingSnapshot`. It does not register user-facing
views. Repeated leases reuse pointer-identical provider objects; no later
reopen/rebind path exists.

#### Dependencies

WP22 (validated publication and exact versions), WP19
(schema/catalog composition probe), WP09 (three-axis `TableSpec`).
#### Target Invariants

I-02, I-08, I-12. Doctrine P6, P19, P24.
#### Design and Library References

Data Fabric §12.6, §91, §98.1–§98.2;
roadmap §8 snapshot-provider ordering; LD-01/LD-04.

#### Change Surface

##### Preflight Query

~~~bash
rg -n 'Snapshot|CatalogProvider|SchemaProvider|DeltaTableProvider|AccessProfile' src tests contracts -g '!**/target/**'
ast-grep outline src --items structure --view signatures
~~~

##### Known Touch (verified this session)

`src/compatibility.rs` currently proves only `MemTable` registration and
`src/fabric.rs` contains the Delta boundary. Snapshot-provider, access-profile, and private
catalog production modules do not yet exist. WP26 owns their construction and freezing;
WP25 later owns only views and sessions.

#### Required Changes

Implement the access-profile factory, exact-version provider resolution, overlay wrapping, schema/constraint validation, private catalog construction, and freeze-before-activation boundary. Provider construction is snapshot-owned and never lazy after activation.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp26_behavioral_acceptance` in the packet's focused test target.

deterministic construction trace proves `resolve versions →
  construct providers → wrap → validate → freeze`; exact-version providers
  retain the expected schemas/checksums and query statistics.
##### Structural

Executable oracle: `wp26_structural_acceptance` in the packet's focused test target.

handle construction requires an access-profile enum; provider
  instances stored in the candidate snapshot are pointer-identical across
  leases; no mutable global/current-pointer read occurs inside a provider.
##### Negative / Zero-State

Executable oracle: `wp26_negative_zero_state` in the packet's focused test target.

unresolved version, schema/checksum mismatch, missing access
  profile, `QUERY_SERVING` with `skip_stats=true`, or post-freeze rebind blocks
  candidate creation before activation.
##### Operational

Executable oracle: `wp26_operational_acceptance` in the packet's focused test target.

construction duration and provider-count metrics are emitted.

#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check.
#### Integration Milestone

M04.
#### Replan Triggers

Exact-version provider or private-catalog construction
cannot be completed before activation at the pinned family → stop and revise
the snapshot architecture; activation may not move earlier.
#### Rollback or Recovery

Additive; no snapshot is activated by this packet.

### WP23 — Hot overlay: representation, policies, consolidation, rebase

#### Outcome

AC-G-20 `OverlayTable` (exact-schema replacement batches sorted
by PK ordering, `Arc<RecordBatch>` zero-copy sharing, owner + primary-key
tombstone indexes with the verbatim tombstone Arrow schemas, generation
bounds, content digest, hard memory reservation that fails before
activation); AC-G-21 policies enforced from `TableSpec` (OWNER_REPLACE /
PRIMARY_KEY_UPSERT / FULL_TABLE_REPLACE / BASE_IMMUTABLE / NOT_APPLICABLE;
partial replacement of a FULL_TABLE_REPLACE table rejected — AC-G-21's
escape hatch for a formally proven smaller stable replacement partition is
a Waves-5+ derivation-profile concern, out of Wave-3 scope, audit C-11); AC-G-22
deterministic consolidation (the seven rules; highest accepted
source_generation wins; equal generation requires identical payload digest
else `OVERLAY_GENERATION_CONFLICT`; digests recomputed from logical content);
the three-snapshot durable-rebase protocol (capture O_flush → publish
P_(n+1) → CAS pointer → rebase O_delta with content-digest-guarded row
removal → validate effective digest unchanged → activate S_new; failed CAS or
digest mismatch aborts and restarts from the current base).

#### Dependencies

WP21, WP22 (hard edge: the rebase protocol consumes
WP22's publication + pointer CAS), **WP24** (hard edge: the
rebase's "activate S_new" step and the consolidated-overlay swap consume
WP24's AC-G-26 activation transaction and its WP26-built candidate factory;
WP23 executes **after** WP24 per §15).
#### Target Invariants

I-02, I-09, I-14. Doctrine P12, P17, P24.
#### Design and Library References

Data Fabric §12.2, AC-G-20/21/22;
Lifecycle §101–§107 (overlay rationale); roadmap §8 WP5–6.

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

`contracts/comparison/comparison-ignore-registry.yaml` and the fault-point registry are
current. No overlay representation, tombstone index, consolidation/rebase module, or
production fabric suite exists yet. The packet updates the comparison registry with the
overlay-versus-durable rules—excluding operational columns and file-layout fields there,
never inline—and registers rebase-boundary fault points in
`contracts/faults/fault-point-registry.yaml`.
##### Preflight Query

none beyond WP22's probes (rebase reuses its CAS).

#### Required Changes

Implement typed replacement/tombstone overlay batches, generated policy enforcement, memory reservation, deterministic consolidation, durable rebase, retry recovery, and canonical overlay-versus-durable equality.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp23_behavioral_acceptance` in the packet's focused test target.

property-based consolidation tests over generated mutation
  sequences: consolidate(merge(a,b)) ≡ consolidate(consolidate(a)+b);
  rebase preserves effective state bit-exactly (I-09 check via canonical
  comparison).
##### Structural

Executable oracle: `wp23_structural_acceptance` in the packet's focused test target.

overlay rows validate against the exact base schema digest;
  tombstones use the verbatim schemas; every overlay table's policy comes
  from the `overlay_mutation` axis of `TableSpec`; this packet cannot branch
  on durable mutation or materialization role.
##### Negative / Zero-State

Executable oracle: `wp23_negative_zero_state` in the packet's focused test target.

equal-generation conflicting payloads →
  `OVERLAY_GENERATION_CONFLICT` blocks activation; memory reservation breach
  fails before activation; no query ever observes a chain of mutable
  overlays (activation swaps one consolidated overlay).
##### Operational

Executable oracle: `wp23_operational_acceptance` in the packet's focused test target.

overlay memory accounting metric.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check.
#### Integration Milestone

M04.
#### Replan Triggers

None beyond WP22's.
#### Rollback or Recovery

The in-memory overlay is disposable, but rebase-produced
durable publications and pointer advances are not: rolling back WP23
requires the WP22 abandon/recovery path plus pointer restoration to the
pre-rebase base (and, in development, state-root disposal per WP13's
rollback rule).

### WP24 — ServingSnapshot manifest, activation transaction, leases, retention

#### Outcome

AC-G-19 manifest construction (complete field set; CBEF-encoded
body in generated schema order; `manifest_digest =
BLAKE3-256("codefabric-serving-snapshot-manifest-v1" || body)`;
`snapshot_id = BLAKE3-128(CBEF(SERVING_SNAPSHOT, manifest_digest))`;
construction fails closed on any missing required reference); the AC-G-26
candidate is supplied by WP26 with exact-version providers, empty-overlay
wrappers, access profiles, private catalog, integrity checks, and a frozen
provider set already complete; the AC-G-26 activation transaction: SQLite `BEGIN IMMEDIATE` (insert READY manifest →
verify pointer generation + predecessor → retire predecessor → mark ACTIVE →
replace `active_snapshot` row → commit) then the in-memory `ArcSwap` —
never swapped before the durable commit; restart reconstructs memory from
SQLite choosing only fully-validating manifests; `SnapshotActivationRecord`
kept separate from the immutable manifest); AC-G-23 leases (kinds,
`ACTIVE|RELEASING|RELEASED|EXPIRED|ORPHANED`, 15 s heartbeats for >30 s work,
5-minute expiry, artifact-TTL coupling stubbed to query/resource kinds in
Wave 3, ORPHANED + 24 h crash grace), coupled to WP16's source-blob holder
records so a serving-snapshot lease acquires/releases the referenced source
artifacts without conflating the two lease tables; vacuum guards: retention set = current
publication ∪ active snapshot ∪ non-expired leases ∪ recovery-eligible
publications ∪ 7-day minimum window; `vacuum` dry-run-first recipe (§101
workflow — but the retention *set* is AC-G-23's five-element union, which
supersedes §101's narrower four-item enumeration). Successful
first activation emits the sole BOOTSTRAPPING→READY lifecycle event.

#### Dependencies

WP26; WP22; WP13 (SQLite); WP16 (source-blob lease API).
Executes **before** WP23: this
packet activates a WP26-constructed snapshot over a base
publication with an **empty overlay** (`overlay_generation` 0, no overlay
tables — a valid AC-G-19 manifest); WP23 later populates the overlay block
and consumes this packet's activation transaction. The lease/heartbeat
half depends only on WP13 and may be developed in parallel from M03.
#### Target Invariants

I-02, I-08, I-14. Doctrine P19, P22, P24.
#### Design and Library References

Data Fabric AC-G-19/23/26 (activation
leg), §101; roadmap §8 WP6–7. LD-04 (vacuum APIs confirmed), LD-09
(`ArcSwap`).

#### Change Surface

The executor re-runs the packet-specific discovery below before editing; listed files are verified current touch points, not a frozen manifest.
##### Known Touch (verified this session)

The serving-snapshot schema and fault-point registry are current. Snapshot/lease/vacuum
modules and WP13 manifest/lease tables do not yet exist.
##### Preflight Query

validate the `effective_content_digest`/
  `primary_key_digest` computation cost over the synthetic corpus at
  publication scale; record the measurement with the design issue. Interim
  mechanism (compute at the publication boundary) is adopted only if the
  measured cost is acceptable at synthetic scale; otherwise escalate before
  building.

#### Required Changes

Implement the immutable ServingSnapshot manifest, candidate validation, atomic activation, leases/heartbeats, source-blob coupling, retention/vacuum protection, crash recovery, and the sole first transition to READY after provider freeze.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp24_behavioral_acceptance` in the packet's focused test target.

build→validate→activate cycle; lease/heartbeat/expiry state
  walk; crash between SQLite commit and memory swap → restart converges to
  the committed snapshot (fault point injected); crash before commit → prior
  snapshot remains active. First activation transitions the workspace from
  BOOTSTRAPPING to READY only after the frozen candidate is durable and active.
  Serving-lease acquire/release pins and releases the referenced source blobs;
  expiry/orphan grace never makes a live source artifact collectible.
##### Structural

Executable oracle: `wp24_structural_acceptance` in the packet's focused test target.

`snapshot_id` KAT vector (fixed manifest → expected ID);
  manifest excludes activation-record fields; the activation function accepts
  only WP26's frozen-candidate type and cannot construct/rebind providers.
##### Negative / Zero-State

Executable oracle: `wp24_negative_zero_state` in the packet's focused test target.

activation with a failed digest verification is impossible;
  vacuum dry-run never lists a pinned file (test over a pinned+leased
  fixture); memory swap before durable commit structurally prevented (single
  code path, asserted by test hook ordering); no `READY` and no active pointer
  are observable before provider/catalog freeze.
##### Operational

Executable oracle: `wp24_operational_acceptance` in the packet's focused test target.

lease table exposed read-only; orphan sweep idempotent.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check; just mutants-file for the changed
validation/freeze/durable-commit/swap module.
#### Integration Milestone

M04.
#### Replan Triggers

`effective_content_digest`/`primary_key_digest` cost
makes snapshot construction non-interactive even for synthetic scale →
interim: compute at publication time (already a full-scan boundary), record
in manifest; design issue filed.
#### Rollback or Recovery

Snapshots additive; pointer protocol governs visibility.

### WP25 — Serving views, read-only DataFusion sessions, pinned-query proof

#### Outcome

Serving views and query sessions consume the already-frozen,
`ServingSnapshot`-owned private catalog/provider set from WP26/WP24 and expose
`cpg_control`, `cpg_base`, `cpg_serving` (Wave-3 subset;
`cpg_python`/`cpg_rust`/`cpg_derived` namespaces registered empty). WP25 does
not reopen, reconstruct, or rebind Delta providers. It registers the §91
effective-rows composition (overlay UNION ALL base
ANTI JOIN replaced-keys ANTI JOIN tombstones) built as a programmatic logical
view (preflight: `ViewTable` construction; fallback: thin custom
`TableProvider` composing the same plan — LD-01 grounding); DataFusion
runtime per §98 (bounded memory pool — not the unbounded default — spill
directory, batch size 65 536, pruning on); read-only SQL surface with DDL/DML
and statements disabled (`SQLOptions::with_allow_ddl(false)` /
`with_allow_dml(false)` / `with_allow_statements(false)`), plus a logical-plan
allowlist rejecting unapproved providers/functions and direct-file scans;
serving views for the Wave-3 tables (`entities`,
`relations`, plus property/evidence projections) hiding operational columns
and joining enum dimensions for names — with the §92 full-view list recorded
as staged conformance; operational-store read-only projections under
`cpg_control` (§13.12 minimum set that exists in Waves 2–3; point-in-time
captures taken at snapshot-lease acquisition, documented as
operationally-current-not-snapshot-pinned — cross-store join semantics
recorded as an explicit cross-store consistency contract); **the pinned-
query proof**: a long-running query against a leased snapshot returns
identical results while an activation swaps the active pointer mid-flight.

#### Dependencies

WP23, WP24, WP26.
#### Target Invariants

I-02, I-05, I-07 (daemon-owned catalog), I-12.
Doctrine P6, P21, P26 (views as projections).
#### Design and Library References

Data Fabric §6.3, §91–§94, §98, §13.12;
roadmap §8 WP8. LD-01 grounding (view-over-providers is the recommended
pattern; anti-join semantics confirmed).

#### Change Surface

##### Preflight Query

The `ViewTable`/anti-join composition
probe was resolved in WP19's preflight (audit Q7) — this packet consumes
its outcome (programmatic view or thin-provider fallback). Remaining
preflight: `MemTable::try_new` partition shape (`Vec<Vec<RecordBatch>>`).

##### Known Touch (verified this session)

`src/compatibility.rs` contains the current `MemTable`/`SessionContext` feasibility seam;
no production serving-view or query-session implementation exists. WP26's future private
catalog is the only permitted provider source.

#### Required Changes

Implement snapshot-owned serving views, read-only bounded DataFusion sessions, immutable private catalogs, plan allowlisting, source-consistency projections, and the pinned-query proof with no hidden mutation or provider rebinding.

#### Legacy Disposition and Decommission

No packet-local compatibility authority is retained. Any predecessor probe or scaffold is removed once the production path and its named zero-state check are green; cross-packet exits remain governed by §12.

#### Acceptance Checks
##### Behavioral

Executable oracle: `wp25_behavioral_acceptance` in the packet's focused test target.

SQL over `cpg_serving.entities`/`relations` returns effective
  rows across all overlay policies; §112.4 subset (catalog opens exact pinned
  versions; projection/filter pushdown observed in plans; plan snapshots
  recorded); the pinned-query proof passes deterministically (barrier-
  synchronized test).
##### Structural

Executable oracle: `wp25_structural_acceptance` in the packet's focused test target.

every catalog is constructed from one leased snapshot object;
  every lease observes pointer-identical providers; no mutable global pointer
  read inside providers and no provider construction/rebind symbol appears in
  this module (governance rule). View eligibility follows only
  `materialization_role`; overlay composition follows only `overlay_mutation`.
##### Negative / Zero-State

Executable oracle: `wp25_negative_zero_state` in the packet's focused test target.

DDL, DML, `SET`/`SHOW`/`RESET`-class statements, direct-file
  references, and unauthorized providers/functions are rejected before
  execution; hidden operational
  columns absent from view schemas; a table with an unspecified overlay
  policy fails view registration (schema error per §91).
##### Operational

Executable oracle: `wp25_operational_acceptance` in the packet's focused test target.

memory-pool limit + spill configuration observable; plan
  artifacts (§110 subset) emitted for the conformance queries.
#### Edit-Local Gates

Run the smallest changed-module lint and targeted test slice.

#### Packet-Local Gates

just ci-fast; just wave3-integration-check. WP25 assembles the Wave-3 exit evidence.
#### Integration Milestone

M04 (closes Wave 3).
#### Replan Triggers

(Fires at WP19 preflight, where the probe now lives —
audit Q7.) Neither programmatic view nor thin provider composes the
anti-join plan with acceptable correctness → plan revision to a custom
`ExecutionPlan` (drags in the §18.20 pushdown test matrix — sized as +1
packet).
#### Rollback or Recovery

Catalog additive.

---

## 11. Integration milestones

### M00 — Current-state evidence contract

Packets: WP00. Decommission batch: DB06.

Required evidence: schema-2 state validation; accepted remediation and user deferrals
recorded; every packet marked complete has a non-null ancestor proving commit; no derived
facts are stored; tracked Cargo outputs remain absent from the index and reachable HEAD
history. Gates: just artifacts-check, just plan-status, and just
tracked-target-zero-state-check.

### M01 — Wave 0 exit: four isolated build domains

Packets: WP01–WP05.

Required evidence: all four local domains build/test; exact identities are emitted on
STDERR; STDOUT protocol silence holds; the stable graph and local/S3 activation boundary
match policy; one Proto descriptor compiler path is reproducible; DB01 and DB06 are green.
Gates:
just ci-fast, just stable-graph-check, just features-each, just proto-repro-check, and
just seed-zero-state-check, and just tracked-target-zero-state-check. Ubuntu
clean-checkout is deferred and not required.

### M02 — Wave 1 exit: Readiness Gate A

Packets: WP06, WP06a, WP07, WP08, WP08b, WP09, WP10, and WP11.

Required evidence: every required contract is released; just contracts-verify-released
has zero warnings; compilation units own every output; identity/path/type/registry/KAT
oracles pass; both schema modes and FastMCP fingerprints are current; one FDS produces all
four production protocols and both language bindings; all eight bundles are populated;
traceability has no orphan. DB02–DB05 are green. Gates are the contract/schema/Proto/
adapter/reproduction group in §14 plus bounded parser/protocol fuzz targets.

### M03 — Wave 2 exit: secure source-instance control plane

Packets: WP12–WP18.

Required evidence: Git and non-Git workspaces register correctly; secure-open adversarial
fixtures pass on the local macOS platform and supported Linux evidence when prioritized;
capture races never publish false stability; restart restores operational state without
inventing a snapshot; source-control health is separate from READY; blob lease/GC is safe.
Gate: just wave2-integration-check plus just ci-fast and just contracts-verify.

### M04 — Wave 3 exit: canonical fact-state substrate

Packets: WP19, WP20, WP21, WP22, WP26, WP24, WP23, and WP25. This is distinct from the
historical remediation milestone also named M04.

Required evidence: synthetic facts insert/replace/remove/publish/overlay/rebase/lease/query
end to end; leased reads survive active-snapshot swaps; provider sets are exact-version and
frozen before activation; publication/pointer crash recovery is coherent; overlay equality
and schema round trips pass. Gate: just wave3-integration-check, just ci-fast, just
contracts-verify, and the focused mutation/fuzz obligations from §14.

---

## 12. Cross-packet decommission batches

### DB01 — Seed and packaging-surface zero state

Prerequisites: WP01, WP04, WP05. just seed-zero-state-check proves PyO3, Maturin, root
Python packaging, the native extension, the old cargo feature, and seed APIs remain absent.

### DB02 — Manual contract authority zero state

Prerequisites: WP06, WP06a, WP11. just governance and just contracts-verify prove there is
one typed catalog, one artifact index, no lexical header/status authority, no secondary
language-neutral generated resource, and no directory-walk ownership discovery.

### DB03 — Dual Proto compiler and speculative JSON dependency zero state

Prerequisites: WP05, WP06a, WP10. just proto-check, just proto-repro-check, just
stable-graph-check, and adapter lock checks prove one grpcio-tools descriptor compiler,
Rust compile_fds consumption, exact grpcio/protobuf pins, and no protoc-bin-vendored or
orjson dependency.

### DB04 — Independent adapter schema authority zero state

Prerequisites: WP09, WP11. just adapter-contracts-governance and just
adapter-contracts-repro-check prove every adapter model/schema/fingerprint comes from
Contract IR and no request path constructs models, TypeAdapters, channels, or stubs.

### DB05 — Single-artifact output ownership zero state

Prerequisites: WP06a, WP10, WP11. just compilation-units-check plus structural and textual
governance prove artifact-level generated_outputs/build depends_on, authored producer,
global output_of_kind, suite-self Proto ownership, generator-side catalog/output scans,
SOURCE_RELATIVE/RUST_OUTPUT/wave0_probe constants, descriptor-census semantic authority,
custom aggregate source-set SHA-256, and all Wave-0 probe source/binding/test residue are
absent outside history and negative fixtures.

### DB06 — Tracked Cargo build-output zero state

Prerequisites: completed baseline cleanup and WP00. just
tracked-target-zero-state-check proves nested Cargo-root target paths are ignored, absent
from the Git index, and absent from reachable HEAD history. Its temporary-repository
negative fixtures prove the checker fails for both current-tree and historical-only target
objects. This batch preserves the already-completed IR-010 cleanup; it performs no history
rewrite during v5 execution.

---

## 13. Design assumptions, accepted deferrals, and replan ownership

- A-01: Derivation inputs are explicit typed artifact/output references, plus the one
  AllCompiledArtifacts intrinsic reserved for artifact-index generation. The catalog is
  not a shell/glob/conditional build DSL.
- A-02: SourceBytes and CompiledSemantic are distinct input views; resolved transitive
  lineage is deterministic and visible in the generated index.
- A-03: The suite manifest remains ordinary self-description; generated index bytes are
  not embedded into or hashed by their own source.
- A-04: WP09 production schemas remain derived from the owner-approved Contract IR; a
  required field unavailable in Wave 3 is returned to its normative owner rather than
  represented by a placeholder.
- A-05: The four production Protobuf authorities enter one descriptor/Python derivation
  and one Rust-from-FDS derivation; there is one compiler invocation and one FDS.
- A-06: The synthetic reconciliation body in WP20 is temporary but its input/output
  signature is production-shaped. A signature change is design reopening.
- A-07: local-workstation-v1 remains no-network and local-filesystem only.
- A-08: Ubuntu clean-checkout evidence is deferred. Local macOS evidence remains required;
  unsupported platform claims are not inferred from it.
- A-09: IR-010 is resolved at the current baseline. DB06 is a reintroduction guard, not
  authorization for another history rewrite.
- A-10: License selection and license-policy expansion are outside scope.
- A-11: DO-01, the production DaemonClient, stays assigned to Wave 17; DO-02, the real
  four-tool FastMCP handlers, stays assigned to Wave 18.
- A-12: Any current compatibility probe establishes API feasibility only and cannot close a
  product packet.

Design-owner changes alter SUITE/RM/SRV/FAB/LIFE/QRY/ONT/GEN before implementation
consumes the new rule. Library behavior mismatch is first an implementation adaptation
when invariants remain intact; it becomes a plan revision when dependencies, packet
boundaries, or proof change; it becomes design reopening when authority, public contracts,
identity, security, lifecycle, or storage semantics change.

---

## 14. Final gate matrix

The final matrix is recipe names only. WP00 proposes the first three absent recipes;
WP06a proposes compilation-units-check; Wave-2 and Wave-3 integration recipes are added by
the first packet that needs them.
Ubuntu clean-checkout is not in this matrix under the user's assurance deferral.

Routine and cross-domain gates:

- just artifacts-check
- just plan-status
- just tracked-target-zero-state-check
- just ci-fast
- just ci-pr
- just root-check
- just root-clippy
- just root-test
- just extractor-ci-fast
- just sidecar-ci-fast
- just adapter-ci-fast
- just adapter-stdio-test
- just stable-graph-check
- just features-each
- just deps-fast
- just policy
- just governance
- just proof-coverage-check
- just seed-zero-state-check
- just typos

Contract, schema, Proto, adapter, and release gates:

- just contracts-tooling-lint
- just contracts-verify
- just contracts-verify-released
- just contracts-repro-check
- just schema-check
- just fixture-check
- just compilation-units-check
- just proto-check
- just proto-repro-check
- just adapter-contracts-check
- just adapter-contracts-governance
- just adapter-contracts-repro-check
- just adapter-wheel-test

Wave integration gates:

- just wave2-integration-check
- just wave3-integration-check

Risk-triggered parser/protocol evidence, once the named targets exist:

- just fuzz
- just mutants-file

The fuzz and mutation recipes are packet/milestone obligations for their named risky
surfaces, not blanket per-commit gates. A performance recipe becomes a completion gate only
after a packet states a concrete performance claim, workload, environment, and threshold.
No clean-build timing or benchmark result is written to execution state.

---

## 15. Execution sequence

Declared dependency edges govern; this sequence preserves every v4 packet ID and the
WP26-before-WP24-before-WP23 correction.

~~~text
Reconciliation
  WP00 -> DB06 -> M00

Wave 0
  M00 -> WP01 -> {WP02 || WP03 || WP04} -> WP05 -> M01

Wave 1
  M01 -> WP06 -> WP06a -> WP07 -> WP08
       -> {WP08b || WP09 || WP10} -> WP11 -> M02

Wave 2
  M02 -> WP12 -> WP13
       -> {WP14 || (WP15 -> WP16 -> WP17)}
       -> WP18 -> M03

Wave 3
  M03 -> WP19 -> WP20 -> WP21 -> WP22
       -> WP26 -> WP24 -> WP23 -> WP25 -> M04
~~~

Additional direct edges:

- WP10 depends on WP05, WP06a, and WP08.
- WP11 depends on WP06, WP07, WP08, WP08b, WP09, and WP10.
- WP13 depends on WP12 and WP09.
- WP14 depends on WP13 and WP07.
- WP15 depends on WP07 and WP13.
- WP16 depends on WP13 and WP15.
- WP17 depends on WP12, WP15, and WP16.
- WP18 depends on WP12–WP17.
- WP19 depends on WP09, WP12, WP13, WP14, and WP18.
- WP26 depends on WP09, WP19, and WP22.
- WP24 depends on WP13, WP16, WP22, and WP26.
- WP23 depends on WP21, WP22, and WP24.
- WP25 depends on WP23, WP24, and WP26.

WP02, WP03, and WP04 may run in parallel because their build roots are disjoint; WP05 owns
shared integration. WP08b, WP09, and WP10 may run in parallel only after predeclaring
compilation units and either serializing catalog writes or proving disjoint unit records.
No later-wave prework may be reported as completion of an earlier packet.

---

## 16. Completion checklist

- [ ] WP00 — schema-2 state/proving-commit reconciliation
- [ ] DB06 — tracked Cargo build-output zero state remains green
- [ ] M00 — state contract green
- [ ] WP01–WP05 — Wave-0 outcomes re-proved on the current v5 baseline
- [ ] M01 — four local domains and standing zero states green
- [ ] WP06 — corrected typed contract compiler substrate re-proved
- [ ] WP06a — compilation units and generated-output provenance
- [ ] WP07 — CBEF, public IDs, path rules, type algebra, independent KATs
- [ ] WP08 — populated registries and generated state machines
- [ ] WP08b — phrase mappings, controlled grammar, model-pack schema
- [ ] WP09 — TableSpecs, SQLite DDL, snapshot/state/public JSON schemas
- [ ] WP10 — four production protocols through one FDS
- [ ] WP11 — populated bundles, deployment, CF-ID traceability
- [ ] M02 — released-profile zero warnings and DB02–DB05 green
- [ ] WP12–WP18 — secure Wave-2 control plane
- [ ] M03 — source-instance control-plane integration green
- [ ] WP19–WP22 — schema runtime, batches, mutation, publication
- [ ] WP26 — exact-version provider/catalog factory
- [ ] WP24 — immutable snapshot activation, leases, retention
- [ ] WP23 — overlay and rebase
- [ ] WP25 — serving views and pinned-query proof
- [ ] M04 — Wave-3 end-to-end and final gate matrix green
- [ ] DO-01 remains explicitly assigned to Wave 17
- [ ] DO-02 remains explicitly assigned to Wave 18
- [ ] Ubuntu remains recorded as user-deferred; IR-010 remains recorded as resolved and
  guarded by DB06

---

## 17. Plan risks and replan policy

### 17.1 Risks

- R-01: Compilation-unit input expansion may accidentally change a derivation's source
  set. Mitigation: explicit typed references, the single ArtifactIndex-only
  AllCompiledArtifacts intrinsic, deterministic resolution, resolved-input provenance,
  release review, and mutation tests.
- R-02: The exact delta-rs revision has per-table application transactions but no
  cross-table CAS. Mitigation: WP21 native transaction markers and WP22 predecessor-pinned
  OCC with post-commit reread.
- R-03: gix linked-worktree, index-fingerprint, write-freedom, revision-feature, or SHA-256
  behavior may differ from current probes. Mitigation: five WP17 executable probes and a
  typed unsupported-format outcome rather than guessed parity.
- R-04: Rust gRPC UDS peer identity is library-sensitive. Mitigation: preserve the landed
  pre-dispatch peer test, bilateral limits, deadlines, status mapping, and Python interop.
- R-05: Wave-1 normative content is broad. Mitigation: schema/validator first, owner
  acceptance, append-only records, catalog-driven generation, and parallel WP08b/WP09/WP10
  only after compilation-unit predeclaration.
- R-06: macOS and Linux secure-open behavior differs. Mitigation: local macOS adversarial
  proof is mandatory; Linux evidence remains a supported-platform obligation when the user
  re-prioritizes it, without blocking unrelated current packets.
- R-07: deep rustc/Pyrefly pins churn. Mitigation: isolated lock/toolchain domains, exact
  identities, golden protocol/semantic corpora, and fail-fast negotiation.
- R-08: the pinned storage kernel compiles latent cloud features. Mitigation: local-only
  profile authority, dependency graph gates, and no default cloud provider construction.
- R-09: schema-1 history may not justify every previously recorded completion. Mitigation:
  WP00 leaves such packets in progress/stale unless a current ancestor proving commit and
  checks exist.
- R-10: generated-output migration could create temporary dual authority. Mitigation:
  WP06a is one dependency-closed cutover and DB05 forbids aliases or dual ownership.
- R-11: nested Cargo build outputs could be reintroduced by a future sub-root or ignore
  regression. Mitigation: DB06 checks both the index and reachable HEAD history and proves
  failure against temporary-repository negative fixtures.
- R-12: performance optimization may erase proof. Mitigation: proof-coverage-check is
  authoritative; benchmark only stable claims and never replace independent intent gates.

### 17.2 Replan classification

Implementation adaptation stays within the accepted authority, identity, lifecycle,
security, storage, and library decisions and is recorded in schema-2 state.

A new plan version is required when packet boundaries, dependencies, milestones,
decommission exit proofs, gate recipes, or declared inputs materially change.

Design reopens when a normative owner, public contract, semantic identity, provenance
model, compatibility rule, trust boundary, storage authority, or lifecycle invariant must
change. The executor corrects the owning design before code consumes the change.

### 17.3 Standing packet triggers

Replan when any of the following occurs:

1. A pinned API is absent or behaviorally incompatible and no recorded adaptation preserves
   the invariant.
2. Current-tree preflight finds a materially larger or different consumer/authority surface.
3. The packet cannot end dependency-closed without pulling later-wave product behavior.
4. A required target invariant cannot be constructed with the planned mechanism.
5. A migration would require unbounded dual authority, silent compatibility aliases, or
   duplicate writes.
6. Security, correctness, recovery, or resource evidence invalidates the mechanism.
7. A normative design input changes after planning and affects the active packet.
8. A performance claim lacks a stable workload, controlled baseline, or correctness parity.

### 17.4 Rollback doctrine

Source-only packets revert atomically with their generated outputs. Persisted-state packets
restore from their named backup/checkpoint and never pretend forward-only migrations are
reversible. Publication/pointer/snapshot packets recover through idempotent replay and
predecessor validation. No rollback restores a decommissioned second authority.

---

End of v5. V4 and the remediation plan remain immutable provenance; v5 is the sole
continuation contract for Waves 0–3.
