---
artifact: design-dossier
design_id: codefabric-model-driven-artifact-and-assurance-control-plane
version: v1
date: 2026-08-22
status: accepted
baseline_commit: 6b42f33b7de72044b40939f7d86b5dee8888d06c
working_tree_digest: 0823e4d5764fb5d4e2c9db724f352ddb8b7b3e09ebebb4d804741d32a5c9e407
primary_scope:
  - docs/upfront_design/
  - contracts/
  - src/contracts/
  - src/generated/
  - tooling/contracts/
  - tooling/proto/
  - tooling/ci/
  - codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/
  - codefabric-cpg-mcp/src/codefabric_cpg_mcp/daemon/generated/
  - scripts/
  - rules/
  - rule-tests/
  - justfile
  - Cargo.toml
  - codefabric-cpg-mcp/pyproject.toml
doctrine_path: docs/library_ref/semantic_design_principles_holistic.md
---

# CodeFabric model-driven artifact and assurance control-plane design

## 1. Executive decision

### 1.1 Decision

Replace the authored artifact inventory, path lists, digest fields, trace indexes, proof-coverage
manifest, and packet-specific mutation campaigns with one compiled repository model and one
transactional reconciler.

The target is a **family-rule semantic compiler** over self-describing native authorities. It
discovers governed sources only inside closed roots, parses them into closed typed models, derives
the full artifact/derivation/assurance graph, plans an immutable desired output tree, validates that
tree independently in staging, and then either reports drift or applies only derived outputs.
`suite-manifest.json`, `fixture-oracles.json`, `traceability.jsonl`, bundle manifests, toolchain
identity, generated source indexes, proof coverage, and packaged artifact indexes become compiled
views rather than parallel authoring surfaces.

Routine development no longer runs packet-specific mutation campaigns. Model conformance,
metamorphic impact checks, independent known-answer tests, consumer validation, clean
reproducibility, and transactional fault tests replace them as the fidelity system. The generic
`mutants-file` tool may remain available as a manually chosen Tier-C diagnostic, but mutation
testing is absent from edit, packet, milestone, Tier-A, and release profiles.

The current typed `ContractCatalog`, strict ingress, canonicalization pipeline, Contract IR,
descriptor-first Protobuf flow, Pydantic generation, Arrow schema construction, and independent
fixtures are valuable substrate and will be reshaped rather than discarded. The 3,178-line
catalog and its central artifact-ID/cardinality dispatch are an intermediate architecture, not the
target.

### 1.2 Observable outcomes

- Adding an artifact to an existing family requires adding or editing its native semantic source;
  it does not require editing a central artifact list, output list, consumer list, package-data
  list, digest value, proof manifest, or Rust dispatch table.
- A semantic edit regenerates exactly its transitive derived-output closure. A source-format-only
  edit refreshes exact-byte provenance without rebuilding semantic consumers.
- One explicit `model-sync` operation, or an explicitly started `model-watch` session, updates all
  affected derived Rust, Python, Protobuf, schema, registry, bundle, traceability, provenance, and
  packaging views.
- All read-only gates execute `model-check`; CI never relies on a mutating command to become green.
- A failure before apply leaves normative authorities and the previously valid derived tree
  unchanged. An interruption during apply is crash-consistent: every supported consumer recovers
  under the repository lock before it can observe or use the tree again.
- Cold, warm-cache, cache-disabled, and two-root builds produce the same desired path set and exact
  bytes.
- Requirement, implementation, oracle, and generated-output traceability are reverse indexes over
  the compiled model, never separately authored records.
- CBEF identity recipes, registry code allocations, TableSpecs, Pydantic models, Protobuf bindings,
  and consumer fingerprints are generated from their governing models; callers cannot construct a
  field layout or numeric allocation that those models do not admit.

### 1.3 Non-goals

- Do not create another Cargo package, workspace, external build system, graph database, or daemon.
- Do not replace Cargo, Just, Nextest, Pytest, Maturin/uv, Pydantic, Protobuf, Arrow, or the adopted
  canonicalization libraries.
- Do not automatically accept a breaking schema/Proto baseline, edit normative prose, rewrite an
  owner-reviewed KAT, mint a registry allocation, or approve a compatibility change.
- Do not generate expected KAT answers from the renderer being tested.
- Do not make wall-clock timing, cache hit rate, coverage percentage, or mutation score a
  correctness claim.
- Do not change production CPG, storage, query, lifecycle, or serving semantics except where the
  compiled model exposes an existing undocumented divergence.

### 1.4 Current-state evidence

The design was developed against a dirty tree containing the in-progress WP32 work. The
frontmatter digest identifies the pre-dossier working tree by hashing the binary `HEAD` diff plus
sorted untracked path/content identities. Those changes are preserved and are not claimed by this
dossier.

| Evidence | Current fact | Design consequence |
|---|---|---|
| `just --list` | 96 recipes; four packet-specific mutation recipes already exist and the active Waves 4–7 plan requires ten more | Packet identity is leaking into permanent repository tooling. |
| `wc -lc contracts/manifests/suite-manifest.json` | 3,178 lines and 100,842 bytes | The typed model is useful, but the authored bootstrap is itself the main duplication surface. |
| `jq` over `suite-manifest.json` | 66 artifact records (51 native authorities and 15 derivation-backed projections), 7 derivations, 60 output paths, 10 resource profiles | Most repeated member/output metadata is derivable from seven family operations. |
| `ast-grep outline src/contracts --items exports` | Closed catalog, compiler, models, schema compiler, index, and registry-model seams already exist | Migration can preserve the typed core and replace its authority/loading/dispatch boundaries. |
| `rg` over compiler/tooling/scripts | 145 artifact-ID or tool-version string occurrences in the selected surfaces | Per-member IDs, output paths, versions, and family sizes remain copied into code and shell. |
| `tooling/ci/proof-coverage.json` plus `just --dump --dump-format json` | The proof manifest repeats Just's command DAG and proof membership by hand | Just supplies the executable DAG; only irreducible proof semantics need declaration. |
| `src/contracts/artifacts.rs` generation flow | Generation rewrites authority digests, requirements, bundles, and toolchain identity before rendering outputs; writes are atomic only per file | Routine generation is not one-way and cannot guarantee repository-level failure atomicity. |
| `scripts/contracts_repro_check.sh` | It reads the authored catalog with `jq`, copies listed inputs, and executes a shared `target/debug/codefabric-contracts` path | Reproduction repeats the manifest and concurrent feature builds can replace the shared binary. |
| pre-edit `just ci-fast` | Core Rust, 175 Nextest tests, doctests, extractor, sidecar, adapter 82 tests, and structural rules passed; `artifacts-check` then failed because the active plan has a stale Data Fabric declared-input digest | The baseline failure is pre-existing planning metadata drift, not evidence against this docs-only design. |

One additional current defect makes the need concrete. The in-flight WP32 implementation builds
`IdentityDomain::Entity` and `RelationFact` values with field layouts that differ from the exact
recipes in `contracts/identity/cbef-v1.yaml`; generic CBEF framing accepts them because it does not
validate the selected domain recipe. It also introduces occurrence-family codes and
`provider_node_flags` allocations without a governed registry allocation. WP32 is not conforming
until those constructions use generated recipe-aware builders and governed generated codes, or a
reviewed contract version explicitly changes the recipes.

### 1.5 Required design-corpus corrections

This target changes documented design and therefore requires explicit integration before an
implementation plan is executed:

1. **SUITE AC-G-02** — source identities become detached for every authority. An authority does
   not embed or get rewritten with `canonical_digest` or `source_digest`; the compiler records
   both in the generated provenance index. A format may retain external sentinels for readable
   documentation, but no computed identity is an authored input.
2. **SUITE AC-G-04** — requirement declarations are co-located with their normative source;
   implementation and oracle declarations point to requirement IDs; `requirements.jsonl` and
   `traceability.jsonl` are generated reverse indexes. Digest, selector expansion, and
   `verified_by` joins are never authored twice.
3. **SUITE AC-G-05** — `suite-manifest.json` ceases to be the compiler bootstrap and becomes a
   compatibility/provenance projection of the compiled repository model. Closed family roots,
   source self-identification, family rules, an owner-accepted versioned release census, and
   explicit tombstones define discovery completeness. Routine synchronization cannot modify the
   release census.
4. **SUITE AC-G-07** — author only bundle policy and any accountable signature acceptance.
   Membership, member identities, bundle payload, and bundle digest are compiler outputs.
5. **RM evidence doctrine and the active Waves 4–7 plan** — remove every mandatory
   `mutants-wp*` obligation and replace it with the model, metamorphic, KAT, consumer, and
   failure-injection oracles in §6. No packet adds a permanent packet-named recipe.
6. **CBEF/ontology allocation ownership** — require generated recipe-aware identity builders and
   generated enum/flag accessors. Undeclared positional fields or raw numeric allocations fail
   structurally and at compile/test time.

## 2. Constraints and target invariants

### 2.1 Architectural constraints

- The repository remains one root Cargo package and one library crate. The model compiler is a
  narrow handwritten-only binary/feature surface inside that package. It does not link generated
  production registries, bindings, schemas, or the production library surface it regenerates.
- Rust remains the semantic core; Python remains an adapter and library-native projection seam.
- Mutating commands never become prerequisites of read-only gates. Starting `model-sync` or
  `model-watch` is the developer's scoped consent to update compiler-owned derived paths only.
- Existing exact dependency and protocol pins remain authoritative until separately reviewed.
- `docs/upfront_design/` remains normative. Generated indexes cannot outrank or silently rewrite
  their source authorities.
- Generated outputs needed for Rust compilation, Python editing/packaging, or review remain
  materialized and committed unless a later packaging design proves a safer build-tree-only path.
- Independent KATs, negative fixtures, and accountable acceptance records remain authored evidence;
  the reconciler is forbidden from writing them.

These constraints apply doctrine P1/P2 (hide mechanics and separate concerns), P5/P7 (one-way,
acyclic dependencies), P10 (declarative knowledge single-sourcing), P11/P12 (parse into legal
states), P14 (staged compilation), P17 (functional core/imperative shell), P25
(reproducibility/incrementality), P27 (provenance), P30 (testability), and P31 (additive extension
plus executable governance).

### 2.2 Target invariants

- **I-01 — one semantic model:** one immutable `RepositoryModel` owns the current artifact,
  derivation, output, requirement, oracle, compatibility, and provenance graph in memory.
- **I-02 — distributed authority, compiled index:** native sources and irreducible local policy are
  authored; aggregate inventories and reverse indexes are generated.
- **I-03 — exact claiming:** every present filesystem path under a governed authority or evidence
  root—tracked, staged, or untracked—is claimed exactly once by one family rule, explicitly
  ignored by typed policy, or rejected. Git ignore state never authorizes omission.
- **I-04 — released absence is explicit:** deletion of a previously released artifact requires a
  compatible tombstone/deprecation or reviewed major transition against the accepted release
  census; disappearance from directory discovery or a generated index never proves intended
  removal.
- **I-05 — typed edges:** source, semantic, generator, output, consumer, requirement, and oracle
  edges are closed enum variants with validated endpoint kinds.
- **I-06 — acyclic phases:** identity, render, validation, packaging, and assurance edges form one
  DAG. Cycles report stable node IDs and edge kinds.
- **I-07 — detached derived identities:** source files never contain compiler-maintained digests.
  Generated provenance cannot feed back into source semantic identity.
- **I-08 — desired-tree ownership:** every derived path has exactly one producer and provenance;
  no generator may write outside its planned output set.
- **I-09 — full plan before write:** all renderers and independent validators succeed in staging,
  and input identities are rechecked, before the first committed derived path changes.
- **I-10 — incremental equals full:** affected-closure execution, warm-cache execution, and a
  cache-disabled full rebuild produce the same desired path set and bytes.
- **I-11 — independent evidence:** normative KAT answers and external/library-native validation
  are outside the renderer's write surface.
- **I-12 — proof selection is sound:** an unknown or opaque dependency widens the selected proof
  profile; it never causes an oracle to be silently skipped.
- **I-13 — no packet infrastructure:** permanent recipes, manifests, and proof IDs describe
  capabilities and contracts, never implementation-plan packet numbers.
- **I-14 — bounded ingress and diagnostics:** discovery, parsing, graph construction, subprocess
  output, and diagnostics use named limits and safe repository-relative paths.
- **I-15 — acceptance is separate:** compatibility baselines, signatures, KAT changes, registry
  allocations, and breaking contract changes require explicit owner acceptance, never routine
  synchronization.
- **I-16 — model-enforced encodings:** CBEF domain builders and registry/flag allocations are
  generated from their governing models; generic encoders cannot accept a recipe-incompatible
  field set.
- **I-17 — acyclic bootstrap:** the compiler executable and its handwritten model definitions are
  upstream of all generated outputs. Staged production-consumer builds use an isolated tree and
  cannot become a prerequisite for building the compiler itself.

## 3. Target architecture

### 3.1 Authority classes

Every governed item has one of four roles:

| Role | Meaning | Reconciler write policy |
|---|---|---|
| `Authority` | Normative prose, Contract IR, Proto, EBNF, typed registries, deployment/security policy | read-only |
| `EvidenceAuthority` | Owner-reviewed KAT, negative case, semantic acceptance fixture | read-only |
| `Acceptance` | Release census, tombstone, compatibility-baseline acceptance, signature, allocation approval | explicit accept command only |
| `Derived` | Catalog/index, canonical registry, generated binding, schema view, bundle, trace graph, toolchain identity, proof report | routine sync may replace |

Each authority retains a small native header: stable ID, kind, public version, compatible suite
major, status, and optional typed local override. Owner, format profile, compatibility family,
resource profile, consumers, bundle membership, and ordinary output naming are derived from the
claiming family rule. Computed digests are absent.

The compiler has a fixed, small registry of `FamilyRule` implementations. A rule owns a closed
root and extension set, native typed parser, default policy, output-planning convention, and
independent validator set. This registry is the unavoidable format boundary; it never lists
individual artifacts or per-member outputs. Adding a member to a family is data-only. Adding a
new representation requires one rule plus its registration and tests.

Discovery uses one byte-safe inventory algorithm. It resolves repository and linked-worktree
topology through the read-only gix adapter, then walks the current filesystem beneath fixed roots
with explicit extension allowlists, no symlink traversal, bounded entry/byte counts, and
safe-relative-path checks. `PlatformPath` retains native `OsString`; gix-derived `GitRepoPath`
retains raw bytes; display strings are never identity. Matching tracked, staged, and untracked
files all enter the candidate model from their current stable filesystem bytes. A matching ignored
or non-UTF-8 path is explicitly claimed/ignored by typed policy or rejected—it is never silently
omitted. Index/status/HEAD information classifies candidates and detects conflicts but does not
replace the filesystem bytes or their BLAKE3 identities. If gix fails, the same bounded full-root
walk remains correct; only Git-aware diagnostics and acceleration are lost. Git diffs and watcher
events can seed a changed-path set but never define the authoritative inventory.

Released completeness is independent of routine generation. A compact, versioned
`Acceptance` record contains only released stable IDs, suite major/status, and accepted tombstone
references. `model-sync` cannot write it. `model-accept release-census` may promote a generated
candidate only after explicit owner review at a release boundary. A released ID missing from the
candidate model blocks unless that accepted census names a compatible tombstone or reviewed major
transition. Generated artifact indexes remain useful output/provenance views but are not release
oracles.

### 3.2 Repository model

The functional core compiles authorities into closed values conceptually equivalent to:

```text
RepositoryModel
  source_artifacts: ArtifactId -> SourceArtifact
  evidence:         EvidenceId -> EvidenceAuthority
  acceptances:      AcceptanceId -> AcceptanceRecord
  derivations:      DerivationId -> Derivation
  outputs:          OutputId -> PlannedOutput
  requirements:     RequirementId -> Requirement
  oracles:          OracleId -> Oracle
  profiles:         ProfileId -> AssuranceProfile
  graph:            typed directed dependency graph
```

IDs and repository paths are validated newtypes, not interchangeable strings. Node types encode
whether their identity derives from source bytes, compiled semantics, a toolchain, or a preceding
output. Edges distinguish `semantic-input`, `source-input`, `generates`, `packages`, `consumes`,
`implements`, `verifies`, `bundles`, and `invalidates`. Illegal endpoint combinations are rejected
while building the model.

The dependency topology is an ephemeral `petgraph::DiGraph<ModelNode, DependencyKind>` with an
external `BTreeMap<StableId, NodeIndex>`. `petgraph::algo::toposort` supplies cycle detection and
dependency order; reverse-graph traversal supplies affected dependants. Graph-local indices never
escape the compiler or become stable identities.

### 3.3 Family-native model leverage

The compiler does not translate every family into generic JSON and then reimplement its native
semantics:

- JSON/YAML/JSONL authorities use strict bounded Serde models. RFC 8785 bytes and BLAKE3 framing
  stay with the adopted canonicalization stack.
- Protobuf source sets compile once through `grpcio-tools` to one `FileDescriptorSet` semantic IR;
  descriptor APIs plan packages, bindings, census, compatibility checks, and Rust `compile_fds`
  outputs.
- Adapter Contract IR compiles through strict frozen Pydantic models and module-scoped
  `TypeAdapter`s. Pydantic's validation and serialization JSON Schema modes and model-field
  introspection produce public schemas and FastMCP fingerprints.
- Schema Contract IR produces Arrow `Schema`/`Field`, TableSpecs, SQLite DDL, and public JSON
  Schema views from the same typed records. It also owns named row/cross-field constraints and
  encoder projection metadata, allowing generated row builders and generic semantic-type checks to
  replace table-code/column-name matches in ingestion code. DataFusion is a consumer validator,
  not a second schema author.
- CBEF and categorical registries produce typed constructors, enums, bitflags, lookup tables, and
  validators. Runtime callers pass named semantic operands; generated code fixes field order,
  tags, widths, and allocations.
- Just's JSON dump supplies the executable recipe DAG; Cargo metadata supplies resolved Rust
  package/tool identity; Pytest/Nextest collection supplies live test inventories. The repository
  does not transcribe those facts into another manifest.

### 3.4 Derivation drivers and output planning

Each family driver has three logical operations:

1. `describe` returns its stable driver/rule/schema/toolchain identity and input-selection policy;
2. `plan` derives output identities, paths, consumers, and validators from the typed model;
3. `render` writes only to a supplied staging root and returns observed output identities.

Drivers never resolve global state at module import, write repository paths directly, or infer an
output by matching a literal artifact ID. Fixed singleton paths may exist inside the owning driver,
but generic compiler code contains no family paths or cardinalities. Multi-member outputs derive
from semantic identifiers such as Proto package names, registry artifact slugs, public Pydantic
views, and TableSpec records. Drivers also generate Rust/Python aggregator modules so adding a
package or registry does not require editing `src/rpc.rs`, `__init__.py`, or another hand-maintained
include list.

`PlannedOutput` carries a tagged, typed projection specification in addition to its path and kind.
That specification owns such values as the Pydantic root models and validation/serialization mode,
schema title/public identity, Proto package role, registry primary-ID field, and TableSpec
projection. Path-indexed sibling maps such as `PUBLIC_SCHEMA_ARTIFACTS` are prohibited.

Python and external-tool drivers receive a canonical resolved invocation from the Rust
orchestrator and return a canonical plan/result document. They do not shell back into Cargo to
rediscover the same model. Every subprocess runs with a cleaned environment, exact tool identity,
bounded output, and an isolated staging directory.

The model compiler is itself an explicit bootstrap action. Its selected Cargo binary target is
compiled from handwritten compiler/model/driver code plus `Cargo.lock` and toolchain inputs, with a
dedicated feature surface that neither links the production library nor includes generated
registries, schemas, provider bindings, or RPC bindings. The graph therefore has the strict order
`compiler sources/toolchain -> compiler executable -> generated outputs -> production consumers`.
To validate new Rust or Python outputs before apply, the orchestrator creates an isolated
validation tree by overlaying the complete `DesiredTree` on the current authorities and runs
consumer builds there with an isolated Cargo target directory and Python environment. No staged
consumer is needed to build the compiler that produced it.

### 3.5 Staged compiler and reconciler

```text
current stable native filesystem bytes
  -> bounded discovery and exact claiming
  -> strict family-native parse
  -> normalized RepositoryModel
  -> typed graph validation and topological plan
  -> semantic/source identities
  -> immutable DesiredTree rendered in staging
  -> independent staged-tree validators and consumer probes
  -> source-generation fence recheck
  -> check/report OR journaled apply of Derived paths
```

`DesiredTree` is a sorted mapping from safe repository-relative path to bytes, role, producer,
lineage, output kind, and content digest. Comparing it with the current tracked derived tree yields
typed `Add`, `Replace`, `DeleteStale`, and `Unchanged` actions. A stale generated file is therefore
deleted programmatically; an unplanned file in a generated root is an error.

Apply is crash-consistent, not falsely described as a filesystem-wide atomic rename. Every
supported model/generation/gate/consumer recipe first resolves the worktree and acquires a shared
repository reader lock; `model-sync` and recovery acquire the exclusive writer lock. The
reconciler renders and validates everything first, rejects pre-existing edits on affected derived
paths unless explicitly adopted, then writes a checksummed, fsynced recovery journal and backups
under repository-private administrative storage resolved from the worktree Git directory—not
under disposable `target/`. It fsyncs the journal before replacing paths atomically one at a time,
records progress durably, verifies the final tree, and removes the journal only after the new tree
is complete. At startup, any supported consumer must recover to the complete old or complete new
tree before releasing the lock. Direct tools that bypass the command contract are not promised a
multi-file atomic view. Authorities, evidence, and acceptance paths are never members of the
routine write set.

### 3.6 Content-addressed incremental execution

An action key contains:

```text
driver ID and rule version
model schema and output schema versions
ordered semantic-input digests
ordered exact-byte input digests where declared
ordered upstream output content digests
exact lock-resolved toolchain/executable identity and Cargo feature set
assurance/generation profile
environment contract
```

The cache stores rendered bytes and metadata under `target/model-cache/`; it never stores an
authoritative pass verdict. An action may be skipped only when its action key matches, every
declared output exists, every current output content digest matches, and the exact output census
matches the compiled model. A corrupt, incomplete, or over-complete entry is a miss. Correctness
is always provable with caching disabled.

Executables built with different feature sets or toolchains use distinct target/artifact paths or
are serialized. This is required by an observed current race: a reproduction script executing the
shared `target/debug/codefabric-contracts` path can see that binary replaced by a concurrent Cargo
build with another feature set, producing an incomplete output set. The action scheduler must
never identify an executable by path alone. It acquires deterministic resource locks for
overlapping output sets and for any intentionally shared Cargo target directory or executable.

Change input comes from explicit changed paths (`git diff --name-status -z`, CI-provided paths, or
watch events), but current source digests remain authoritative. Watch notifications are hints that
trigger reconciliation, never proof of current bytes. Reverse graph traversal computes the
affected derivation and oracle closure. Opaque commands or missing dependency declarations widen
to a conservative profile.

### 3.7 Model-derived assurance graph

An `Oracle` records stable ID, evidence class, requirements, read-set selectors, command/provider,
cost class, platform constraints, and whether it is independent of a renderer. Oracle declarations
are co-located with what gives them meaning:

- KAT/negative metadata lives in the fixture or a typed adjacent record for formats that cannot
  self-describe;
- ast-grep rule IDs and rule-test cases are discovered from `rules/` and `rule-tests/`;
- Rust/Python test IDs come from live Nextest/Pytest collection plus a stable co-located
  requirement marker;
- family rules supply mandatory library-native and generated-tree validators;
- Just supplies recipe dependencies through `just --dump --dump-format json`.

The compiler joins these into the assurance graph and emits proof coverage as a report. There is
no authored `proof-coverage.json`. A requirement without an implementation claim or executable
oracle, an oracle selecting no live test, an unclaimed rule, and an uncovered released model node
all fail.

Profiles are intent-based:

| Profile | Purpose |
|---|---|
| `edit` | parse changed authorities, build the full model, reconcile the affected desired subtree, run affected fast oracles |
| `changed` | `edit` plus affected consumer and cross-language tests |
| `tier-a` | complete routine repository assurance, replacing the current `ci-fast` graph |
| `release` | full released census, clean/cache-disabled reproduction, packaging, compatibility, and acceptance checks |

The active implementation plan may name a profile and a stable oracle ID, but it may not create a
packet-specific permanent recipe or copy tool flags into execution state.

### 3.8 Mutation-testing disposition

Delete all `mutants-wp*` recipes and remove them from all packet/milestone/final matrices. Do not
replace them with a generated mutation manifest. The generic `mutants-file` recipe remains a
human-invoked investigative tool outside every assurance profile, consistent with repository
Tier-C doctrine. A future scheduled audit may target only the generic model validator, affected
closure, transaction fence, or handwritten boundary adapter after a concrete escaped-defect or
test-strength question justifies it.

The replacement evidence is stronger for this system's actual risks:

- generated small-model/metamorphic cases prove graph and impact invariants;
- independent KATs fix semantic answers;
- library-native validators exercise emitted contracts;
- full-versus-incremental differential checks prove selection soundness;
- fault injection proves no accepted partial update or stale-cache acceptance, and deterministic
  recovery before supported consumers proceed;
- consumer compilation, wheel installation, and cross-language checks prove usable outputs.

### 3.9 Command and developer experience

Just remains the public command facade with a small stable surface:

| Intent | Behavior |
|---|---|
| `model-explain <id-or-path>` | read-only source, lineage, outputs, consumers, oracles, and invalidation explanation |
| `model-plan [changed paths]` | read-only structured action/proof plan |
| `model-check [profile]` | read-only model, desired-tree, and selected assurance verification |
| `model-sync` | explicit locked, journaled, crash-consistent update of all affected `Derived` paths |
| `model-watch` | explicitly started watchexec loop that repeatedly performs scoped `model-sync` |
| `model-accept <kind>` | separately guarded compatibility/KAT/signature/allocation acceptance |

Existing `contracts-gen`, `adapter-contracts-gen`, and `proto-gen` become temporary aliases to the
single reconciler and are then removed. Existing verification recipe names may remain as thin
profile/selector aliases where callers depend on them; their bodies do not restate source/output
paths or family cardinalities.

### 3.10 Failure, provenance, security, performance, and extension contracts

- Diagnostics use stable failure class, node ID, edge/path, phase, bounded message, and suggested
  owner action. Text is explanatory; the class is the automation contract.
- Every derived output records driver/rule identity, semantic and exact input identities as
  applicable, model version, and content digest in the generated artifact index. No circular
  digest participates in its own input.
- Discovery rejects path traversal, symlink escape, duplicate/case-colliding paths, unknown
  extensions, unclaimed governed files, duplicate IDs, output/authority overlap, multi-owner
  output, and graph cycles.
- External drivers are trusted, exact-pinned build tools with no declared network capability.
  They receive a clean environment with credentials/proxy variables removed, a staging root, and
  only their resolved input/output plan. The orchestrator rejects outputs outside that plan and
  source/authority fences detect repository writes. Platform process sandboxing may add
  socket/filesystem denial where available, but portable correctness and security do not depend on
  an unavailable cross-platform sandbox guarantee.
- Work cost is structurally proportional to the affected graph plus profile-mandated oracles.
  Performance reports may record counts and timings as operational evidence, but no timing value is
  a correctness gate until a stable workload and SLO are separately accepted.
- Adding an existing-family member changes data only. Adding a new family adds one `FamilyRule`,
  its native model, independent KAT, and registration. Adding a new renderer adds one driver; the
  generic graph, reconciler, cache, transaction, and proof machinery do not change.

### 3.11 Library and platform decisions

#### LD-01 — Serde remains the closed repository-model boundary

- **Decision:** Preserve derived Rust models with `deny_unknown_fields`, path-aware decode errors,
  `BTreeMap`/`BTreeSet` normalization, and strict family-native parsers. Remove generic `Value`
  dispatch except at explicitly dynamic projection seams.
- **Version basis:** resolved `serde` 1.0.229, `serde_json` 1.0.151 with `arbitrary_precision`,
  `serde_path_to_error` 0.1.20, and `serde_yaml_ng` 0.10.0 already declared.
- **Displaces:** manual field presence checks, duplicated header maps, untyped manifest joins, and
  artifact-ID-specific decode branches.
- **Risk:** Serde attributes are wire-schema policy; a model change can alter identity.
- **Validation:** closed-model negative cases, path-aware diagnostics, versioned projection KATs,
  and full/incremental canonical-byte equivalence.

#### LD-02 — Petgraph supplies dependency algorithms, not persistence

- **Decision:** Add exact optional `petgraph = "=0.8.3"` to `contracts-tooling` with only the needed
  `std` surface. Use an ephemeral `DiGraph` plus external stable-ID map, `toposort` for cycle/order,
  and reverse traversal for impact closure. Do not serialize graph-local indices.
- **Version basis:** 0.8.3 is already resolved in `Cargo.lock` through DataFusion and is the
  repository's documented line; trunk's multi-crate rewrite is not used.
- **Displaces:** custom topological sort, reverse-adjacency traversal, cycle handling, and future
  bespoke graph algorithms.
- **Risk:** a direct narrow-feature dependency must not pull DataFusion or default graph families;
  `NodeIndex` is not a domain identity.
- **Validation:** stable graph feature check, cycle/diamond/fan-out fixtures, insertion-order
  invariance, and affected-closure differential against full recomputation.

#### LD-03 — Keep the adopted canonicalization pipeline intact

- **Decision:** Preserve the four stages: strict source decode, CodeFabric validation/normalization,
  `serde_json_canonicalizer`/Python `rfc8785` bytes, then unkeyed BLAKE3 with `b3:` framing. The
  reconciler consumes canonical bytes; it does not implement sorting, escaping, or number
  formatting.
- **Version basis:** `serde_json_canonicalizer` 0.3.2, Python `rfc8785` 0.1.4, Python `blake3`
  1.0.9, and resolved Rust `blake3` 1.8.7. Action identity uses the lock resolution, never the
  `Cargo.toml` range spelling.
- **Displaces:** digest rewrite helpers, ad hoc serialization, and cross-language serializer
  alternatives.
- **Risk:** detaching identities changes AC-G-02 representation but not canonical byte semantics;
  unsafe numeric or duplicate-key evidence must still be rejected before materialization.
- **Validation:** independent expected-byte/digest KATs, Rust/Python parity, source-only mutation
  cases, and no post-canonicalization byte transformation.

#### LD-04 — Pydantic owns Python contract execution and schema views

- **Decision:** Preserve strict frozen Pydantic 2.13.4 models, module-scoped `TypeAdapter`s,
  validation/serialization schema modes, and model introspection. Resolve inputs once from the
  repository model and run the Python driver against staging.
- **Version basis:** Pydantic 2.13.4 and FastMCP 3.4.7 exact runtime pins.
- **Displaces:** handwritten adapter schema authorities, dynamic hot-path model construction,
  manual public field inventories, and duplicated fingerprints.
- **Risk:** generated tests can agree with a wrong IR projection.
- **Validation:** independent public examples, JSON Schema metaschema/consumer validation,
  FastMCP runtime fingerprint comparison, installed-wheel import, and exact field mutations.

#### LD-05 — One Protobuf descriptor remains the compiled wire IR

- **Decision:** Preserve one exact `grpcio-tools` compiler invocation producing the
  `FileDescriptorSet` and Python outputs; drive Rust `compile_fds`, census, compatibility, and
  package/output planning from descriptor APIs.
- **Version basis:** grpcio/grpcio-tools 1.83.0, protobuf 7.36.0, libprotoc 35.1, prost 0.14.4,
  tonic/tonic-prost-build 0.14.6.
- **Displaces:** per-file output lists, package-name mirrors, a second Proto compiler, and manual
  descriptor field/option census.
- **Risk:** concurrent builds of one shared binary path can cross feature identities; unknown
  descriptor options must remain preserved.
- **Validation:** isolated action executable identity, descriptor-pool checks, normalized unknown
  option wire, compatibility baseline, cross-language round trips, and two-root reproduction.

#### LD-06 — Arrow and JSON Schema validate their own projections

- **Decision:** Continue to build Arrow `Schema`/`Field` values and TableSpecs directly from schema
  Contract IR; use pinned `jsonschema` Draft 2020-12 validation as an independent consumer of
  emitted schemas. DataFusion may validate table consumption but does not participate in the
  repository-model control plane.
- **Version basis:** Arrow 58.4.0 and `jsonschema` 4.26.0.
- **Displaces:** handwritten DDL/TableSpec/public-schema siblings and custom metaschema logic.
- **Risk:** one IR renderer can omit the same semantic element from several outputs.
- **Validation:** independent row/schema KATs, Arrow/DDL/public-schema crosswalk completeness, and
  real consumer construction.

#### LD-07 — Just and existing collectors remain the operational shell

- **Decision:** Derive recipe topology from Just's JSON dump and live test inventories; use
  watchexec only to trigger an explicitly started model loop. Watch/Git events are hints and are
  confirmed by source digests.
- **Version basis:** repository-installed Just, watchexec, Cargo/Nextest, Pytest, ast-grep, and
  ripgrep command contracts.
- **Displaces:** `proof-coverage.json`, shell path/cardinality assertions, packet recipes, and
  duplicated command DAGs.
- **Risk:** collector output formats are external contracts and opaque commands can hide reads.
- **Validation:** version identity, parser fixtures, non-empty selector checks, and conservative
  full-profile fallback for unknown dependencies.

#### LD-08 — Proptest replaces handwritten mutation/permutation grids

- **Decision:** Add dev-only
  `proptest = { version = "=1.11.0", default-features = false, features = ["std"] }` and use
  validity-by-construction strategies for repository models, edit sequences, action plans, cache
  state, and transaction faults. Disable repository-local failure persistence; print the seed and
  minimized case, then promote valuable failures manually into the classified fixture corpus.
- **Version basis:** proptest 1.11.0 has an MSRV below CodeFabric's Rust 1.95 floor and supplies
  strategy composition, recursive data, shrinking, configurable runners, and reproducible seeds.
- **Displaces:** large custom permutation tables and recurring mutation campaigns intended to
  discover missing branches in model, closure, cache, or transaction logic.
- **Risk:** rejection-heavy strategies waste cases, unbounded recursion creates slow tests, and
  automatic regression-file writes would create another hidden fixture authority.
- **Validation:** bounded case/depth/size configuration, fixed smoke seeds plus randomized CI
  seeds, failure persistence outside the repository, and differential comparison with a fresh full
  rebuild.

#### LD-09 — Do not adopt a general action engine or sibling schema generator

- **Decision:** Do not add Bazel, Ninja, Salsa, a plugin runtime, `schemars`, or a Rust `jsonschema`
  generator/validator. The current seven derivation families do not justify another build
  authority; Pydantic/descriptor/Arrow models already own their native schemas, and the pinned
  Python `jsonschema` remains a deliberately independent consumer.
- **Version basis:** current repository scale and dependency graph.
- **Displaces:** none; this avoids a second orchestration or schema authority.
- **Risk:** a homegrown scheduler may become a bottleneck as the graph grows.
- **Validation:** replan on the thresholds in §6.8 and keep the action protocol serializable so an
  external engine can be adopted without changing semantic models.

#### LD-10 — gix classifies repository state; filesystem bytes define the model

- **Decision:** Reuse exact-pinned read-only gix for repository/linked-worktree topology, byte-safe
  Git paths, index/status classification, and candidate acceleration. The bounded closed-root
  filesystem walk and BLAKE3 of stable current reads remain the authoritative inventory and byte
  identity. gix failure falls back to the full walk; Git ignore state never authorizes omission.
- **Version basis:** `gix = "=0.86.0"`, `default-features = false`, with the repository's governed
  read-only feature set, including `revision` for immutable baseline-blob validation of typed
  transition patches. No Git mutation, network, credentials, hooks, checkout, filters, or index
  writes enter the compiler.
- **Displaces:** ad hoc `git diff`/UTF-8 path parsing as the source universe while retaining diffs
  and watch events as invalidation hints.
- **Risk:** worktree/index bytes can disagree, linked-worktree paths are subtle, Git paths need not
  be UTF-8, and a status candidate is not present-byte truth.
- **Validation:** tracked/staged/untracked/ignored/conflicted/deleted/non-UTF-8/linked-worktree
  fixtures, current-byte digest fences, read-only capability checks, and gix-enabled versus
  gix-disabled full-model equivalence.

## 4. Alternatives and clean-sheet challenge

### 4.1 Alternative A — retain the explicit authored graph, add a reconciler

Keep `suite-manifest.json` authoritative, remove derived digests, and add `DesiredTree`, staging,
and impact execution. This is the shortest transition and preserves explicit review.

It is rejected as the target because it preserves the largest maintenance defect: every artifact
and output still repeats path, owner, consumers, lineage, resource profile, and cardinality. It is
acceptable only as a temporary parity oracle during migration.

### 4.2 Alternative B — family-rule compiler plus transactional reconciler

Derive membership, outputs, and assurance from native sources and small family rules, while using
the explicit compiled graph and desired tree for inspection and execution. This is the selected
target because it keeps auditability without keeping a hand-authored build DSL.

The chief risk is hidden discovery. The selected design counters it with exact path claiming,
owner-accepted release-census reconciliation, explicit tombstones, generated discovery reports,
and missing/extra/duplicate/unclaimed negative tests.

### 4.3 Alternative C — external incremental action engine

Compile the semantic graph and delegate scheduling/caching to Bazel, Ninja, Salsa, or a similar
engine. This offers mature invalidation and parallelism but creates another build abstraction next
to Cargo, Just, uv, and the language generators. Seven current derivations do not justify that
operational and maintenance cost. The target action protocol remains serializable so this can be
revisited if scale proves the need.

### 4.4 Alternative D — Rust constants/macros as the master manifest

Move artifact and derivation records into Rust source and generate all files from those constants.
This is rejected: it merely relocates hardcoding, makes non-Rust inspection harder, and still
duplicates meaning already present in native authorities.

### 4.5 Clean-sheet challenge

The selected design survives the clean-sheet test only under these conditions:

- The compiled graph is a report and execution IR, never another authored authority.
- A family member is discoverable from its native source and family convention alone.
- A required released member cannot disappear silently.
- A generator cannot write its own expected KAT or mutate an authority to make verification pass.
- The assurance planner proves its selected closure equivalent to full execution on model-generated
  changes; unknown dependencies widen rather than narrow.
- Routine sync is one-way. Compatibility acceptance is a different state transition and command.
- Adding a new family changes one registration boundary, not matches scattered through compiler,
  scripts, tests, and packaging.

If any of those conditions fails during implementation, revert to an explicit minimal graph for
that family rather than introducing hidden convention or a second authority.

## 5. Transition, cutover, and legacy disposition

### 5.1 Transition sequence

1. **Accept design corrections.** Update SUITE AC-G-02/04/05/07, CBEF/registry ownership, RM
   evidence doctrine, and the active plan before resuming packet certification.
2. **Introduce the compiled model read-only.** Reuse current typed parsers; add family claiming,
   the typed graph, desired-tree planning, and `model-explain/plan/check`. Treat the current catalog
   only as a temporary parity oracle.
3. **Break the bootstrap cycle.** Land the handwritten-only model-compiler binary surface, model
   its exact toolchain/build inputs, and prove it builds without any generated production output.
   Validate candidate generated consumers in an isolated overlaid tree.
4. **Close the current fidelity defect.** Generate CBEF recipe builders and registry/flag
   allocations, migrate WP32 to them, and reject recipe-incompatible identity construction.
5. **Make derived identities external.** Remove compiler-maintained digests from authorities;
   compile the artifact index, requirements/trace indexes, bundles, and toolchain identity into
   staging. Introduce the owner-accepted release census and seed it from the reviewed current
   release before the generated artifact index loses oracle status.
6. **Adapt each generator.** Convert registry, schema, adapter, Proto, and provider-inventory
   generators to `describe/plan/render`; derive aggregators and package views.
7. **Land crash-consistent sync.** Compare/apply the complete desired tree with source fences,
   repository reader/writer locking, durable recovery journal, stale-output deletion, and
   action-key isolation.
8. **Compile assurance.** Discover oracles and recipe/test topology, replace the proof manifest and
   shell mirrors, add `edit/changed/tier-a/release` profiles, and prove incremental/full parity.
9. **Decommission mutation campaigns and old authority.** Remove `mutants-wp*`, central per-member
   catalog/output lists, artifact-ID dispatch, and the old generator chain. Retain compatibility
   output filenames only where consumers require them.
10. **Cut over deliberately.** A clean cache-disabled release check and two-root reproduction must be
   green on the same tree before the active plan uses the new profiles.

Completed packet evidence remains historical evidence. The remediation does not retroactively
rename those packets, but it reruns every affected schema, registry, identity, provider, adapter,
Proto, and standing integration oracle before new work relies on the reshaped substrate. The
in-progress WP32 packet receives no proving status until its identity and allocation deviations are
removed.

The shadow period is bounded: both compilers may read the same native authorities, but only the old
one writes until parity; then only the new reconciler writes. The old manifest never becomes a
second semantic owner.

### 5.2 Legacy disposition matrix

| Surface | Disposition | Target ownership / exit condition |
|---|---|---|
| `contracts/manifests/suite-manifest.json` | reshape | Generated compatibility/provenance projection; no bootstrap reads or authored member/output lists remain. |
| owner-accepted released-artifact census | add as acceptance | Compact versioned released-ID/status/tombstone record; changed only by explicit reviewed release acceptance, never by `model-sync`. |
| computed `canonical_digest` fields in machine authorities | delete | Detached generated artifact index owns computed identities; zero compiler rewrite paths. |
| `contracts/manifests/fixture-oracles.json` | reshape | Generated from fixture-local typed evidence declarations; independent KAT bytes remain read-only. |
| `contracts/manifests/requirements.jsonl` | reshape | Generated from co-located normative requirement declarations and implementation/oracle joins. |
| `contracts/manifests/traceability.jsonl` | preserve as derived | Exact reverse index emitted from the repository model; never authored. |
| `contracts/bundles/*.json` | reshape | Bundle policy/signature acceptance separate; manifests, members, identities, and provenance generated. |
| `contracts/toolchain/toolchain-identity.json` | reshape | Generated from Cargo/uv/tool/driver identities; no copied version or lock digest strings in generic code. |
| `contracts/generated/**` | preserve as derived | Desired-tree exact census, stale deletion, and provenance. |
| `src/generated/**` | preserve as derived | Family drivers also generate aggregator modules; no handwritten per-member includes. |
| adapter generated contracts and Proto modules | preserve as derived | Planned from Pydantic views and descriptor packages; wheel/install validation remains independent. |
| runtime `include!`/`include_bytes!`, `OnceLock`, and Python cache accessors | preserve | Efficient immutable consumption remains; only their member lists and backing bytes become compiler-owned. |
| `src/contracts/catalog.rs` | reshape | Source-family compiler plus typed model/graph; remove authored catalog bootstrap and custom graph algorithms. |
| `src/contracts/compiler.rs` | reshape | Preserve bounded native compilation/canonicalization; replace artifact-ID matches with registered family parsers/models. |
| `src/contracts/artifacts.rs` | replace | Generic desired-tree reconciler and driver boundary replace `sync_*`, central render dispatch, and direct authority writes. |
| `src/contracts/models.rs`, `registry_models.rs`, `schema_models.rs` | preserve/reshape | Remain closed semantic models; add reusable family/header/requirement/evidence types and generated recipe builders. |
| `src/contracts/schema_artifacts.rs` | reshape | Schema family driver planning/rendering against staging. |
| `src/identity.rs` manual domain/tag constructions | replace selectively | Preserve generic CBEF codec; generate recipe-aware builders, domains, tags, widths, and public-ID accessors from CBEF authority. |
| `src/fact_ingest.rs` table/column/semantic-type matches | replace | Generate typed row builders and generic semantic validators from TableSpec and Arrow field metadata. |
| in-flight `src/source_syntax.rs` raw occurrence/flag allocations | replace or govern | Use existing governed semantic fields or add explicit registry allocations before persistence. |
| `tooling/contracts/generate_adapter_models.py` | encapsulate then reshape | Pydantic driver receives one resolved invocation; no Cargo callback or path manifest. |
| `PUBLIC_SCHEMA_ARTIFACTS` and other path-indexed projection maps | delete | Tagged output specifications in Contract IR/model own projection membership and mode. |
| `tooling/proto/generate.py` and Rust Proto generator | encapsulate then reshape | Descriptor family driver derives source/package/output plan; shared executable race eliminated. |
| `tooling/contracts/derivation.py` | delete | Rust orchestrator passes canonical invocations directly; Python no longer shells back into Cargo. |
| `tooling/ci/proof-coverage.json` | delete | Assurance graph and Just/test collectors derive the report. |
| `tooling/ci/proof_coverage.py` | reshape | Preserve useful Just JSON and live-test collection adapters behind the assurance compiler. |
| `scripts/contracts_repro_check.sh`, `compilation_units_check.sh`, adapter/fixture governance mirrors | replace | Model-native oracles own path, cardinality, reproduction, and zero-state rules; shell remains only where an external tool requires an adapter. |
| `rules/*.yml` and `rule-tests/*.yml` | preserve | Independent structural policy; model discovers IDs, tests, and coverage. |
| `justfile` contract generation/proof recipes | reshape | Thin intent aliases to model profiles; no source/output lists or family counts. |
| `justfile` `mutants-wp*` recipes and plan requirements | delete | No recurring packet mutation infrastructure. |
| `justfile` `mutants-file` | preserve outside profiles | Optional human diagnostic only. |
| independent KATs and negative fixtures | preserve | Read-only evidence authorities with local typed metadata and accountable change path. |
| active plan and schema-2 execution state | reshape | Successor plan uses model profiles/oracle IDs; state retains human judgments only. |
| `target/` | preserve as ignored derived storage | Disposable model staging/cache may live below a scoped subdirectory; never authoritative or committed. |
| repository-private model administration area | add | Worktree-aware reader/writer lock plus fsynced transaction journals/backups live outside `target/`; recovery precedes every supported consumer. |

### 5.3 Cutover zero state

Cutover is incomplete while any of these remain:

- an authored full `artifacts`, `derivations`, or `outputs` array for a convention-derived family;
- generic compiler matches on individual artifact IDs or validates literal family cardinalities;
- a routine generator writes a normative authority, KAT, allocation acceptance, or signature;
- a source authority contains a compiler-maintained digest;
- a shell/Python script repeats model-owned source paths, output paths, family counts, tool pins, or
  recipe dependency edges;
- a generated Rust/Python package requires a manual per-member include/import edit;
- a `mutants-wp*` recipe or mandatory mutation gate exists;
- a proof-coverage manifest repeats the live Just DAG;
- CBEF callers can pass an arbitrary positional field list without recipe validation;
- an occurrence/flag/code integer used by production lacks a governed generated allocation;
- concurrent feature/toolchain actions can publish or execute the same binary path.

## 6. Proof strategy

### 6.1 Model and discovery oracles

- Parse every family source into a closed type with path-aware diagnostics.
- Inventory governed roots with tracked, staged, untracked, ignored, deleted, conflicted,
  non-UTF-8, symlink, and linked-worktree cases. Reconcile gix DTOs with current filesystem bytes;
  every present path is claimed exactly once or rejected, and gix-disabled full inventory produces
  the same model and desired bytes.
- Compare the candidate release view with the immutable owner-accepted census plus explicit
  tombstones and additive declarations. Prove that routine sync and generated-index deletion cannot
  erase released history.
- Build the typed graph in multiple source insertion orders; normalized nodes, edges, topological
  order, desired paths, and bytes must match.
- Inject duplicate producer, missing endpoint, illegal edge type, cycle, and unsafe output path;
  each must fail with a stable bounded class.

### 6.2 Desired-tree and transaction oracles

- Build the compiler from a clean tree with every generated production output absent; prove the
  compiler DAG is acyclic and the staged overlay consumer build accepts newly generated outputs.
- Two isolated cache-disabled renders produce the same complete path set and bytes.
- `model-sync` followed by `model-plan` produces zero actions; a second sync is byte-idempotent.
- Deleting an obsolete generated output yields exactly one `DeleteStale`; adding an unplanned file
  to a generated root fails.
- Inject renderer/validator failure before apply, source mutation at the generation fence,
  destination symlink substitution, write failure during apply, process interruption, and
  `cargo clean` after interruption. Pre-apply failure leaves the old tree unchanged; mid-apply
  failure is recovered deterministically to the complete old or complete new tree from the durable
  journal before a supported reader proceeds.
- Run concurrent plan/check consumers against a blocked sync and prove shared/exclusive lock
  ordering; no supported consumer observes a mixed derived tree.
- Existing user edits on a planned derived path block with an explainable conflict; authorities
  are never in the routine write set.

### 6.3 Incremental and cache soundness oracles

- Generate representative acyclic model graphs (chain, diamond, fan-out, independent components)
  and compare affected closure with full recomputation after every node/edge change class.
- Use bounded Proptest strategies to generate valid typed models and edit sequences, shrink any
  discrepancy, and report a reproducible seed without writing a repository regression file.
- For each real family, mutate one source-format-only byte, one semantic field, one rule version,
  one tool identity, one output schema version, and one deleted/added member. Assert the exact
  affected outputs and oracles.
- Warm-cache, cold-cache, cache-disabled, corrupt-cache, truncated-cache, and wrong-feature
  executable cases must yield identical correct bytes or an explicit miss/failure.
- No cached pass verdict can bypass current source digest, model validation, staged validators, or
  the source-generation fence.

### 6.4 Family fidelity oracles

- Canonicalization: independent exact-byte/digest KATs and Rust/Python differential cases.
- Registry/CBEF: every domain recipe generates one typed builder; wrong/missing/extra/reordered
  operands fail; generated codes and flags round-trip to the authority and exhaust its allocation.
- Protobuf: one FDS, descriptor-pool/census/unknown-option checks, compatibility negative cases,
  Rust/Python round trips, and package/output discovery.
- Pydantic/FastMCP: strict validation, serialization and validation schema modes, public field
  fingerprint, handler equivalence, and installed-wheel package data.
- Arrow/schema: TableSpec/Arrow/DDL/public-schema crosswalk exactness and real Arrow/DataFusion and
  JSON Schema consumer construction.
- Traceability: every released requirement has source, implementation, and executable oracle;
  every mandatory released semantic node has a requirement path; all reverse indexes reproduce.

### 6.5 Assurance-selection oracles

- Every selected recipe resolves; every test selector collects at least one current test.
- For a generated change corpus, `changed` profile failures are a superset of failures observed by
  full Tier A for affected semantics. Any unmodeled read forces conservative widening.
- Removing or renaming a test, rule, fixture, recipe, source, or output produces an explicit model
  error rather than a smaller proof report.
- The generated proof report is reproducible but is not itself an authority or pass verdict.

### 6.6 Independent negative and legacy zero-state oracles

Use all three evidence forms where applicable:

- ast-grep structural rules reject direct authority writes outside the reconciler, manual
  generated include lists, generic CBEF positional construction, and production raw code/flag
  literals;
- ripgrep text scans reject `mutants-wp`, source digest rewrite helpers, retired manifest lookups,
  per-artifact compiler matches, and copied tool-version tables;
- compilation/tests prove generated APIs are the only constructible production path and that
  removed names are not referenced through macros or features.

### 6.7 Operational evidence

The reconciler emits structured counts for discovered/claimed nodes, affected nodes, rendered and
cached outputs, validators/oracles selected, bytes staged, transaction state, and conservative
fallback reason. These are diagnostics and optimization inputs. They are recomputed, not stored in
implementation state as verdicts.

### 6.8 Replan triggers and named assumptions

Replan when any of the following becomes true:

- adding an existing-family artifact still requires a generic compiler, Just, script, packaging,
  or proof-manifest edit;
- discovery cannot prove released completeness independently;
- the compiler requires any generated production output in order to build;
- a driver cannot plan all outputs before rendering or must write an authority;
- full-versus-incremental equivalence cannot be expressed with a complete action key;
- a new family needs executable plugins or untrusted code rather than a compiled registration;
- the model grows enough that in-process scheduling is a demonstrated bottleneck, at which point
  an external action engine may be evaluated;
- committed generated outputs cease to be necessary for build, packaging, or review, at which
  point build-tree-only generation deserves a separate packaging design.

Named assumptions for acceptance:

- **A-01:** generated Rust/Python/schema outputs remain committed during this migration.
- **A-02:** every existing authority family can expose stable identity/version/status in its native
  source or a typed adjacent declaration without a central member list.
- **A-03:** maintainers accept one compact, versioned release census as irreducible accountable
  release evidence; it contains stable released IDs/status/tombstones only and is outside routine
  generation.
- **A-04:** mutation testing is retained only as an optional manual diagnostic, not an assurance
  profile or implementation-plan obligation.
- **A-05:** the present seven-family scale does not justify an external action engine.

Design readiness decision: accepted-with-named-assumptions.
