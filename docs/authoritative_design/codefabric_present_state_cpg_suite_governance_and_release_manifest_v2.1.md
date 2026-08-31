---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-suite
suite_id: codefabric-relational-data-fabric
suite_version: 2.1.0
artifact_tag: SUITE
artifact_version: 2.1.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.0.md
---

# CodeFabric relational data-fabric suite governance and release authority

## 0. Purpose, identity, and precedence

This document is the current suite root with stable artifact identity
`codefabric-relational-data-fabric-suite`. It governs the versioned v2.1
authoritative design suite. The v2.0 suite remains immutable historical evidence
and a bounded legacy-runtime description; it is not a second current target.
The governing doctrine is
`docs/library_ref/full_data_fabric_design_principles_v2.md`; every suite contract
is interpreted under that document's execution, proof, and staticness rules.

The current suite contains the seven domain roles SUITE, ONT, GEN, FAB, QRY,
LIFE, and SRV plus the subordinate RM roadmap. Membership is discovered from
the per-document authority frontmatter for suite
`codefabric-relational-data-fabric` version `2.1.0`; no generated manifest,
digest census, or copied registry is semantic authority.

Precedence is:

1. this suite root for cross-domain ownership and release rules;
2. the six domain specifications for their owned behavior and contracts;
3. the roadmap for sequencing only;
4. the accepted target design and active implementation plan for transition
   detail;
5. derived indexes and generated projections for navigation only.

An apparent conflict is a failing governance relation. No consumer chooses the
more convenient document.

## 1. Product boundary retained by v2.1

CodeFabric remains a present-state code-intelligence fabric for Python and Rust.
It owns:

- byte-authoritative workspace source images and repository-aware change input;
- syntax, semantic, type, call, control-flow, dataflow, ownership, effect,
  resource, and mechanically derived graph facts;
- explicit requested/completed coverage, provider remainders, diagnostics,
  provenance, conflict, and unknown state;
- atomic immutable snapshots and exact durable table versions;
- compositional bounded semantic queries over one pinned snapshot;
- one Rust daemon per workspace and presentation-only FastMCP adapters;
- deterministic rebuild, incremental equivalence, failure, security, resource,
  compatibility, and cutover proof.

The fabric emits observations and mechanically derived facts. It does not emit
judgments such as safe-to-refactor, high-risk, should-change, or test-impacted.
Git history, runtime coverage, runtime observation, and environment inventory
remain outside the semantic fact substrate.

## 2. Programmatic session authority

Governance is data executed by the fabric, not a family of hand-maintained
registries. The session assembler consumes exact provider batches, explicit typed
inputs whose values cannot be derived, and typed `ProgrammaticTransformation`
values. It installs them in a candidate DataFusion session and derives its own
catalog observations to fixed point. The resulting typed relations include:

| Relation | Governing meaning |
|---|---|
| `explicit_input` | reviewed identity, compatibility, policy, algorithm, and wire commitments |
| `programmatic_transformation` | typed inputs, builder identity, dependencies, and output assertion |
| `artifact_role` | suite role, owner, version, status, and predecessor |
| `contract` | stable requirement identity, owner, consumers, and disposition |
| `relation_type`, `field`, `constraint` | logical schema and invariant model |
| `provider_family`, `analysis_family` | exact producer authority and capability |
| `normalization_rule`, `authority_rule`, `unknown_rule` | reconciliation behavior |
| `query_form`, `function`, `calculation` | closed compositional query language |
| `policy_rule`, `proof_obligation` | executable admission and proof behavior |
| `state_machine`, `state`, `transition` | durable command and cutover legality |
| `legacy_disposition_selector` | preserve, migrate, reshape, replace, or remove |

Typed Rust constructors and DataFusion's expression/plan algebra define the
executable primitive vocabulary. Relations parameterize those primitives. A row
that does not alter compiled schemas, plans, policies, proof, or lifecycle behavior
is inert and is rejected by causal conformance. Bootstrap metamodels, model-migration
replay, model-derived schema registries, and model digests are prohibited authorities.

Historical AC-G-01 through AC-G-84 identities are preserved in immutable
history and imported into explicit compatibility/disposition inputs with an accountable
preserve, supersede, migrate, or tombstone disposition. Their old generated
registries are not reconstructed as current authority.

## 3. Global invariants

Every domain specification and implementation packet preserves these
invariants:

1. one programmatic session authority and one current suite;
2. source bytes, provider observations, normalized facts, and derived facts
   remain distinct relations;
3. raw and normalized provider kinds coexist;
4. syntax occurrences never masquerade as semantic entities;
5. canonical identity is application-owned and provider-local identity is
   provenance only;
6. absence is never inferred from missing output;
7. conflicting evidence is retained and resolved only by typed transformation authority;
8. every query pins one immutable FabricEpoch;
9. schema, policy, functions, proof, and capability are epoch-scoped;
10. public requests cannot name SQL, tables, functions, plans, or internal
    catalogs;
11. durable change enters through one idempotent FabricCommand;
12. one fenced writer owns publication and activation;
13. proof is part of epoch construction and distinguishes pass, fail, and
    unknown;
14. no generated file, digest, test name, plan text, or metric authorizes
    semantic acceptance;
15. legacy and target mutation never run as an unbounded dual-write system;
16. released wire identities and accepted history are preserved explicitly;
17. every legacy disposition has positive target proof and coverage-qualified
    negative proof;
18. Python remains presentation/control only; Arrow and DataFusion processing
    remain Rust-owned.

## 4. Version and compatibility authority

### 4.1 Version tuple

Every released epoch binds a complete compatibility tuple:

- suite, explicit-input, program, application-release, schema-contract, provider-adapter, analysis,
  query-language, policy, proof, result-contract, wire, and storage versions;
- exact source images and semantic-environment identities;
- exact provider/toolchain identities;
- exact Delta table URI/version/schema tuples and immutable Arrow segments;
- exact function and extension identities.

Compatibility is evaluated from typed compatibility transformations. Unknown compatibility fails
closed and is returned as an explicit gap.

### 4.2 Released boundaries

The following are Class 1 commitments and cannot be deleted from an absent
current consumer:

- released Protobuf request/response/status/source messages and gRPC methods;
- stable public IDs, result fields, error families, pagination and ordering;
- accepted tombstones, release decisions, audit artifacts, and historical
  accepted historical transition records;
- persisted data required for the explicitly retained rollback window.

Every other predecessor mechanism is replaceable once its target behavior and
external-consumer disposition are proved.

### 4.3 Exact dependency basis

The data plane uses the exact dependency identities selected by FAB. Library
APIs are targeted directly at those current versions. Future library changes
are governed migrations, not justification for a defensive semantic facade.

## 5. Execution and proof governance

### 5.1 Proof relations

A FabricEpoch is eligible for activation only when its proof catalog contains
covered, independently discriminating results for:

- relational schema and integrity constraints;
- identity, authority, normalization, conflict, and unknown semantics;
- provider requested/completed/remainder closure;
- derived-analysis producer closure;
- query semantics and public result compatibility;
- catalog authorization and bound dependency closure;
- resource, cancellation, spill, and output limits;
- durable command, publication, fencing, recovery, and activation behavior;
- security and hostile-input behavior;
- clean rebuild and incremental equivalence;
- legacy cutover and zero state.

Each proof result names the exact program, input, implementation, and oracle
identity. Producer-authored expectations cannot authorize the producer.

### 5.2 Independent expectations

Expected semantics, hostile inputs, public protocol expectations, provider
contracts, activation expectations, and comparator behavior are accepted
before implementation consumers. Digests prove integrity only. Decoded
expectations, causal interventions, and independent review prove meaning.

### 5.3 Governance execution

Relational invariants and policy are compiled to optimizer-visible DataFusion
programs wherever semantics permit. Structural and textual tools remain
independent residue checks for build, process, packaging, and legacy surfaces.
They do not substitute for relational semantic proof.

## 6. Provider and analysis authority

Each provider family declares:

- exact API and toolchain identity;
- request and semantic-environment identity;
- requested and completed owners/modules/files;
- emitted relations and coordinate basis;
- unsupported remainder and diagnostics;
- raw provenance and precision;
- cancellation, resource, trust, and partial-failure behavior.

Provider-native facts contain only observations actually supplied by that
provider. Application-built Python CFG/flow, Rust MIR-derived ownership/flow,
common graph, effect/resource, and interprocedural summaries have separate
versioned analysis authority. Every accepted family has exactly one producer or
one explicit unsupported remainder.

## 7. FabricEpoch and durable state

A FabricEpoch owns:

- exact provider batches, explicit inputs, typed transformations, and release vector;
- session-derived logical and physical SchemaContracts;
- immutable source/provider/analysis relations;
- exact Delta and Arrow storage pins;
- sealed internal catalog/session state;
- a reduced authorized child-session factory;
- one governed resource runtime;
- proof, policy, capability, and activation relations.

Delta is durable relation and activation-event history. SQLite stores
reconstructible command/idempotency/lease temporal state only. Neither SQLite
nor a mutable current row is semantic authority. The current head is derived
from a valid activation chain.

## 8. Query and serving governance

The eight public semantic request forms remain bounded and compositional.
Requests become typed request relations and compile inside an authorized child
catalog. Native DataFusion relational plans, including native recursive query
where applicable, are preferred before UDF, table-function, logical-extension,
or physical-extension rungs.

One daemon owns mutable state, provider orchestration, catalogs, query
execution, results, and capabilities. One FastMCP STDIO process per agent
performs validation, presentation, resource delivery, and transport only. It
does not reconstruct semantics, hold Arrow tables, or own an independent CPG.

## 9. Transition and legacy governance

The deployment state machine is forward-only:

`LEGACY_AUTHORITATIVE -> COMPARISON_READY -> NEW_READ_ONLY ->
NEW_EPOCH_SELECTED -> LEGACY_REVOKED -> NEW_MUTATING -> LEGACY_RETIRED`.

Old and new candidates may run on frozen equivalent inputs for comparison.
Only one production mutation authority exists at a time. NEW_MUTATING requires
durable proof that the exact frozen legacy executable cannot reacquire serving
or writer authority after process or host restart.

Legacy inventory is the union of:

- Git tracked and untracked paths;
- hidden filesystem paths with explicit secret/build-output exclusions;
- language parsing, export and re-export results;
- Cargo package, feature, and build-target facts;
- installed and freshly built Python wheel/sdist manifests.

Every candidate matches exactly one legacy disposition or an explicit retained
class. Skipped, unreadable, unparsed, overlapping, and unmatched candidates are
unknown failures.

## 10. Domain ownership

| Tag | Owned boundary |
|---|---|
| SUITE | precedence, versioning, compatibility, proof, release, and legacy governance |
| ONT | fact vocabulary, identity, relation semantics, and non-judgment boundary |
| GEN | provider extraction, reconciliation inputs, derived analysis, and capability |
| FAB | Arrow/DataFusion/Delta catalogs, schemas, epochs, persistence, and execution |
| QRY | compositional requests, compilation, evidence, results, and semantic errors |
| LIFE | source change, commands, update waves, publication, activation, leases, and recovery |
| SRV | daemon RPC, FastMCP presentation, authorization, streaming, and operations |
| RM | implementation order only; subordinate to every domain owner |

Cross-domain behavior is legal only when one row owns it and every consumer
depends on that row. Duplicate ownership is a conformance failure.

## 11. Release readiness

The suite advances through six readiness milestones:

1. programmatic session foundation and independent expectations;
2. exact provider and analysis fabric;
3. authorized semantic delivery;
4. durable epoch reconstruction and recovery;
5. independent release evidence and fenced cutover;
6. total legacy purge and clean reconstruction.

Release requires all packet, milestone, decommission, and final-gate proofs at
one trusted HEAD. A local pass, execution capture, digest, or state label alone
does not authorize release.

## 12. Required executable conformance

The current suite is proved by named checks including:

- `authoritative-design-conformance-check`;
- `v2-authority-cutover-check`;
- `programmatic-schema-authority-check` and `programmatic-transformation-causality-check`;
- exact-provider and provider-authority checks;
- relational schema, catalog-isolation, and query conformance checks;
- command, publication, activation, recovery, and writer-fence checks;
- independent semantic, causal, hostile-input, and public compatibility checks;
- legacy inventory, disposition coverage, authority freeze, and final zero-state
  checks.

The checks are executable evidence. This prose never certifies its own
implementation.

## 13. Completion criterion

The v2.1 suite is realized only when one reconstructible FabricEpoch supplies the
complete present-state product behavior; every required proof is green; the
new runtime is the only serving and mutation authority; and predecessor
registries, generators, bundles, mutable pointers, opaque payloads, duplicate
query paths, package residue, and selectable fallbacks are absent outside
immutable history.
