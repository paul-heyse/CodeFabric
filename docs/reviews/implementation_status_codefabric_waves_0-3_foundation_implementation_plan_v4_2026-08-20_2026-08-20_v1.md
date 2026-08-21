---
artifact: implementation-status
date: 2026-08-20
version: v1
status: complete
plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md
state_path: docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json
---

# Implementation status audit: CodeFabric Waves 0–3 foundation plan v4

## Executive status

Plan v4 must not resume literally, but most of its product scope remains relevant. The
model-based remediation implemented at `cdad126` repaired the Wave-1 compiler substrate; it
did not implement the CBEF, registry, production schema/protocol, daemon, workspace, or data-
fabric outcomes that v4 assigns to WP07–WP26.

The current status is:

- the implementation tree was clean at `cdad126`, two commits ahead of `origin/master`,
  before this audit added its two untracked report artifacts;
- the current local `just ci-fast` passes all four domains and governance;
- v4 WP01–WP05 outcomes are present, and v4 WP06 has been replaced by a stronger corrected
  implementation through the remediation plan;
- v4 WP07 has no production CBEF/path/type-algebra implementation and must remain stopped
  until the execution artifacts are reconciled;
- v4 WP09, WP10, and WP11 have reusable remediation substrate, but their product contracts
  remain incomplete;
- v4 WP12–WP26 remain product work, notwithstanding their Wave-0 compatibility probes;
- Readiness Gate A remains open: `just contracts-verify-released` reports 49 unresolved draft
  artifacts;
- Ubuntu clean-checkout evidence is user-deferred assurance as of this review and is not a
  current blocker;
- the immediate blocker is repository history: 7,156 tracked files under auxiliary Cargo
  `target/` directories total about 2.53 GB in both unpublished commits.

The correct next planning artifact is a v5 successor that preserves v4 packet IDs and
dependencies while integrating the remediation overlays, one additional catalog-model
correction, and the infrastructure cleanup below.

## Provenance

| Item | Reviewed value |
|---|---|
| v4 plan | `docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md` |
| v4 state | `docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json` |
| remediation plan | `docs/plans/codefabric_model_based_foundation_remediation_implementation_plan_v1_2026-08-20.md` |
| remediation state | `docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json` |
| baseline | `a689f1ddf712c0f8fe5cf93d9a50a559f84e4b91` |
| current HEAD | `cdad126b8af7c25f09e9b25b9cb51e1fdf9b822a` |
| candidate pre-remediation implementation commit | `da18263804de09a2b24de7745b95388fb4df7050` |
| candidate remediation implementation commit | `cdad126b8af7c25f09e9b25b9cb51e1fdf9b822a` |

Both candidate commits exist and are ancestors of HEAD. Neither is recorded as a proving
commit in the corresponding state file. The branch history should be repaired before those
references are finalized because removing the tracked build outputs from unpublished history
will change the commit IDs.

### Input freshness and drift

Direct SHA-256 recomputation shows that all eight v4 suite inputs changed after v4 planning;
the local-storage correction dossier is unchanged. This is expected design drift from the
accepted remediation, but it invalidates literal continuation under v4's declared input table.

The remediation plan's review, v4-plan, and cache-design inputs remain byte-identical. Its v4
state, SUITE, Serving, and roadmap inputs changed during execution as intended. The remediation
plan still says `status: draft`, although the user accepted and executed it; this is historical
artifact drift that the successor state/plan must reconcile rather than silently reinterpret.

The non-target change surface from the v4 baseline to HEAD is 275 files. The pre-remediation
commit affects 212 such files and the remediation commit affects 173. This broad drift is
consistent with the two accepted implementation efforts, but packet trust must come from
recorded proving commits and current checks, not from the size of the diff.

## Derived status snapshot

“Stale by proof” below means the outcome is present and currently green, but the schema-1
state records `proving_commit: null`; it does not mean the product capability must be
reimplemented.

| Packet | Derived status | Remaining scope and instruction validity |
|---|---|---|
| WP01 | stale by proof | Stable root and seed cutover are present; `just seed-zero-state-check` passes. Repair target history, then record the corrected proving commit. |
| WP02 | stale by proof | Nightly extractor shell is present and green. Remove its tracked historical build outputs and retain the separate target-root policy. |
| WP03 | stale by proof | Pyrefly sidecar shell is present and green. Remove its tracked historical build outputs; the current stable target-sharing design remains valid. |
| WP04 | stale by proof | FastMCP adapter shell is present and green. Replace v4's open gRPC/Protobuf/orjson language with the remediation's exact pins and no-orjson rule. |
| WP05 | stale by proof; acceptance mechanics superseded | Four-domain commands and protocol probes are present. The one-`grpcio-tools` `FileDescriptorSet` plus Rust `compile_fds` path replaces v4's older generator-identity mechanics. Generated-output provenance still needs the compilation-unit correction below. |
| WP06 | stale by proof; corrected implementation substantially present | Typed catalog, bounded ingress, dual identity, canonical artifact index, independent fixture classes, and reproduction checks are present. V4's manual census, lexical verification, generated-index mirrors, and directory-walk assumptions are superseded. The catalog's single-artifact output ownership is not sufficient for many-source compilers. |
| WP07 | blocked; product work not started | Implement CBEF, public IDs, path canonicalization, type algebra, and owner-reviewed normative KATs from the typed catalog. Use the remediation's dual-identity and fixture-oracle policies. |
| WP08 | not started | Populate the ontology/categorical registries and all state machines. Current registry files are typed, digest-valid draft scaffolds with empty records. |
| WP08b | not started | Populate the phrase registry and executable mappings, complete the controlled-language grammar, and finalize the model-pack schema. Current artifacts are draft scaffolds. |
| WP09 | in progress, substrate only | Contract-IR-to-Pydantic models, validation/serialization schemas, fingerprints, and schema metaschema validation exist. Arrow/Delta `TableSpec`s, SQLite DDL, snapshot/state models, and production JSON Schema content remain. Remove the old sibling adapter-schema renderer from the packet. |
| WP10 | in progress, substrate only | The Wave-0 probe proves one descriptor IR and cross-language generation. The four production protocol authorities remain 11-line draft placeholders and require real messages, services, feature negotiation, bindings, census, compatibility, and round trips through that same FDS path. Before population, the catalog must model their many-source compilation unit and output provenance explicitly. |
| WP11 | in progress, substrate only | Closed typed AC-G-07 bundle models and both digest projections exist. All eight bundle `artifacts` arrays remain empty; deployment content, complete CF-ID requirements/traceability, toolchain population, zero-orphan proof, and released-profile closure remain. |
| WP12 | not started | Implement daemon lifecycle, configuration, singleton lease, discovery, status/stop/drain, and joined shutdown. Existing RPC/compatibility probes are prerequisites only. |
| WP13 | not started | Implement the generated SQLite WAL operational store, migrations, backup/recovery, pragmas, and sole-writer discipline. The current rusqlite code is an API probe only. |
| WP14 | not started | Implement workspace registry/admin IPC, state machines, identities, revisions, nested-root exclusions, and audit rows. |
| WP15 | not started | Implement root authorization, descriptor-relative secure open, path collision handling, and adversarial Linux/macOS proofs. The current rustix call is a compatibility probe only. |
| WP16 | not started | Implement source-image capture, blob/line-index storage, inventory, generations, lease lifecycle, and bounded GC. |
| WP17 | not started | Implement `GitStateAdapter`, topology/state DTOs, inventory classification, interruption, and all five gix probes. `src/git_state.rs` currently exposes only hash-algorithm compatibility. |
| WP18 | not started | Implement the per-workspace coordinator, bootstrap fences, pre-ready health, persistence recovery, and single-mutator discipline. |
| WP19 | not started | Implement runtime `TableSpec` loading, Delta namespaces/tables/constraints, exact schema round trips, local provider enforcement, and advisory refresh. Current DataFusion/Delta code is compatibility probing. |
| WP20 | not started | Implement Arrow encoders, the eleven-check batch validator, bounded observation ingress, and synthetic reconciliation-signature stub. |
| WP21 | not started | Implement durable mutation classes, owner replacement/removal, application transactions, and retry reconciliation. |
| WP22 | not started | Implement durable publication, validation, conditioned current-pointer advance, crash recovery, and conflict proof. |
| WP26 | not started | Implement the access-profile factory, exact-version providers, immutable private catalog, and freeze-before-activation boundary. |
| WP24 | not started | Implement snapshot manifest/activation, leases, retention, source-blob coupling, crash recovery, and the first READY transition. |
| WP23 | not started | Implement overlay batches/tombstones, policy enforcement, consolidation, memory reservation, and durable rebase. |
| WP25 | not started | Implement snapshot-owned serving views, read-only DataFusion policy, plan allowlist, and the pinned-query proof. |

### Milestones and decommission batches

| Item | Derived status | Evidence or remaining obligation |
|---|---|---|
| M01 | ready for local reconciliation after history cleanup | All local four-domain checks pass. Ubuntu clean-checkout is explicitly deferred by the user and must be recorded as such, not left as a live blocker. |
| M02 | not started / Gate A open | `just contracts-verify-released` fails with 49 unresolved draft warnings. WP07–WP11 product content remains. |
| M03 | not started | WP12–WP18 product outcomes remain. |
| M04 | not started | WP19–WP26 product outcomes remain. Do not confuse this with remediation milestone M04. |
| DB01 | behaviorally complete; stale by proof | `just seed-zero-state-check` passes at HEAD. |
| Remediation DB02 | behaviorally complete; stale by proof | Typed catalog/index and authority zero-state checks pass. |
| Remediation DB03 | behaviorally complete; stale by proof | Exact Proto dependency and one-compiler checks pass. |
| Remediation DB04 | behaviorally complete; stale by proof | Adapter Contract-IR generation and hot-path governance pass. |

## Remediation alignment and required design changes

### Normative design disposition

The accepted remediation is already present in the normative owners:

- SUITE AC-G-02 defines semantic and exact-source identities, total projection profiles,
  structural identity omission, and the independent bundle projection;
- SUITE AC-G-05 defines the closed typed catalog, bounded staged ingress, one canonical
  artifact-index resource, one Protobuf descriptor IR, and independent KAT discipline;
- SUITE AC-G-07 defines the closed sorted bundle model and its distinct digest projection;
- RM §6 makes those compiler/model rules the Wave-1 execution contract;
- SRV §§18, 19, 60, and 70 own exact Python dependency intent, Contract-IR-to-Pydantic
  generation, separate validation/serialization schemas, and FastMCP fingerprinting;
- the roadmap retains production `DaemonClient` and real public tool handlers in Waves 17
  and 18 rather than falsely treating their substrate as runtime completion.

One bounded normative compiler-model correction remains. SUITE AC-G-05 currently attaches
each `GeneratedOutput` to one `ArtifactDescriptor`. The implementation therefore assigns all
Proto descriptor, census, toolchain, Rust, and Python outputs to the suite-manifest self
descriptor, while the actual `.proto` inputs declare no outputs. The generator separately
discovers every Proto source and hard-codes the Wave-0 primary filenames. That model cannot
faithfully represent WP10's four-source Proto compilation, one FDS, and multiple language
outputs, and the generated index attributes those outputs to the wrong authority.

Correct SUITE AC-G-05 and RM §6 to add a closed typed compilation/derivation-unit model with
at least: stable unit ID, producer, sorted input artifact IDs or a closed declared input-set
selector, outputs/roles, consumers, resource profile, and generator/tool identity. Generated
index records must bind each output to the unit and to all input semantic/source identities.
The compiler must reject missing inputs, duplicate outputs, cycles, and an output claimed by
more than one unit. Registry and adapter generation should use the same unit model so there is
one derivation abstraction, not a Proto-only exception. The suite self descriptor must cease
to act as an umbrella owner for Proto outputs.

Wave 2 and Wave 3 outcomes otherwise remain semantically aligned. Their successor-plan
wording should state that all contract data, identities, schemas, protocol types, and bundle
metadata are consumed from the typed catalog/Contract IR and shared packaged resources;
packets must not reintroduce manual lists, directory ownership discovery, or secondary schema
authorities.

### Required successor-plan changes

Create v5 rather than editing v4 history. Preserve stable packet IDs and dependency order,
but apply these execution overlays:

1. Add an infrastructure decommission prerequisite for tracked auxiliary `target/` files,
   permanent ignore coverage, and an executable tracked-target zero-state gate.
2. Add a prerequisite packet before WP07 that implements the corrected first-class
   compilation/derivation-unit graph, migrates registry/adapter/Proto outputs, and derives
   generator source sets and destinations without hard-coded Wave-0 filenames.
3. Replace v4 D-04/WP06 language-neutral source mirrors with one canonical packaged artifact
   index; retain generated source only where types or behavior gain static exhaustiveness.
4. Make WP07/WP08/WP08b consume catalog-selected bounded typed ingress, dual identities,
   YAML anchor/alias rejection, and independently governed KATs.
5. Remove WP09's independent adapter schema renderer. Treat the landed Contract-IR-to-
   Pydantic pipeline as prerequisite substrate; keep the actual `TableSpec`, DDL, snapshot,
   and public-schema outcomes in WP09.
6. Make WP10 populate all four real protocols through the single `grpcio-tools`
   `FileDescriptorSet` and Rust `compile_fds` path. Generated-text equality is supplemental,
   not the protocol semantic oracle.
7. Make WP11 populate the already-closed bundle model with real sorted artifact members and
   finish deployment/CF-ID/traceability content. Detached `source_digest` stays in the index.
8. Replace absent v4 recipe names (`adapter-schemas`, `contracts-regen-check`,
   `fuzz-contracts`) with current named recipes such as `adapter-contracts-check`,
   `adapter-contracts-repro-check`, `contracts-repro-check`, `fixture-check`,
   `proto-repro-check`, and the targeted `fuzz` recipe.
9. Remove v4's clean-build timing obligation and any proposal to store derived measurements
   in execution state. Preserve the remediation's proof-coverage comparison and make only
   benchmark-supported performance claims.
10. Record the user's Ubuntu-evidence deferral as an accepted assurance deviation; do not
   continue to model it as the current WP07 blocker.

## Reconciliation decisions

- Preserve v4 WP01–WP05 as historical implementation outcomes, subject to corrected commit
  proof after history cleanup.
- Treat v4 WP06 as implementation-invalidated in its original form but behaviorally replaced
  by the completed remediation substrate; do not repeat the old manual design.
- Treat remediation WP01–WP08 as locally implemented at the candidate remediation commit,
  subject to corrected post-rewrite proving commit IDs.
- Reopen only the generated-output graph slice of remediation WP02/WP05 as a bounded design
  correction; the typed ingress, digest, FDS, and adapter results remain reusable.
- Treat remediation WP09/M04 as locally closeable after the accepted Ubuntu deferral, state
  migration, history cleanup, and one final local gate pass.
- Keep v4 WP07–WP11 product completion separate from remediation substrate completion.
- Keep DO-01 and DO-02 open for Waves 17 and 18.

## Blockers and invalidated assumptions

### Tracked build outputs block a safe push

`git ls-files | rg '(^|/)target/'` reports 7,156 tracked files: 6,803 under
`pyrefly-sidecar/target` and 353 under `rustc-extractor/target`. The HEAD tree contains about
2.53 GB of those files, and the branch is still unpublished. A follow-up deletion commit
would not remove the large blobs from the pushed history. Repair both unpublished commits
before any push, add explicit nested-target ignore coverage, and add a permanent zero-state
check.

### Execution-state trust is not current

Both state files use schema version 1. V4 marks six packets complete with null proving
commits; remediation marks eight complete with null proving commits. Both also retain derived
check evidence, and both record `current_head` as `da18263` rather than the actual HEAD.
Under the current execution-state contract, no complete packet is trusted until the state is
migrated to schema 2 and records an ancestor proving commit whose named checks still pass.

### V4's acceptance commands are stale

V4 still names orjson, a language-neutral `contracts/generated/` authority/mirror model,
and the absent `adapter-schemas`, `contracts-regen-check`, and `fuzz-contracts` recipes. Those
instructions contradict the accepted remediation and cannot be used as completion oracles.

### Generated-output provenance is modeled as one-to-one

`ArtifactDescriptor.generated_outputs` and `OutputsByPath = Path -> (artifact_id, output)`
allow exactly one source owner. The suite self descriptor currently owns seven Proto outputs,
while `codefabric.tooling.wave0-probe` and all four production Proto descriptors own none.
`tooling/proto/generate.py` then gathers Proto inputs independently and hard-codes the Wave-0
primary/output filenames. This is insufficient for WP10's many-source compilation unit and
must be corrected before new generated surfaces multiply the false provenance.

## Recommended resume order

1. Stop before v4 WP07.
2. Repair the two unpublished commits so no Cargo `target/` output exists in their history;
   add explicit ignore coverage and a tracked-target zero-state gate.
3. Correct SUITE AC-G-05/RM §6 with first-class compilation units and implement/migrate that
   graph as a dependency of WP07 and WP10.
4. Create v5 with current design digests and the overlays above.
5. Migrate both execution states to schema 2 after history stabilization; record corrected
   proving commits and the user-directed Ubuntu assurance deferral.
6. Re-run `just ci-fast`, the reproduction checks, and state/zero-state validators; close
   remediation WP09/M04 and v4 M01 locally.
7. Execute WP07 → WP08 → WP08b/WP09/WP10 (predeclare or serialize catalog writes) → WP11 →
   M02.
8. Execute WP12 → WP13 → WP14/WP15 → WP16 → WP17 → WP18 → M03.
9. Execute WP19 → WP20 → WP21 → WP22 → WP26 → WP24 → WP23 → WP25 → M04.

## Exact next action

Do not push the current branch. First prepare a reviewed history repair that removes every
tracked `pyrefly-sidecar/target/**` and `rustc-extractor/target/**` object from the two
unpublished commits, adds permanent ignore/governance coverage, and proves:

```bash
test -z "$(git ls-files | rg '(^|/)target/' || true)"
test -z "$(git rev-list --objects origin/master..HEAD | rg ' (pyrefly-sidecar|rustc-extractor)/target/' || true)"
```

This review does not authorize or perform the history rewrite.

## State reconciliation summary

No plan or state file was edited. After the unpublished history is repaired, migrate both
schema-1 states to schema 2, discard derivable fields, retain judgment/deviation history,
record the new proving commits, change the stale `current_head`/next-action posture, and record
Ubuntu clean-checkout as user-deferred assurance rather than a blocker.

## Checks run for this audit

- `just ci-fast` — passed at `cdad126`.
- `just proto-repro-check` — passed; one compiler/descriptor path and two-root output match.
- `just adapter-contracts-check` — passed.
- `just contracts-repro-check` — passed; two isolated generations match and catalog reorder
  changes only source identity.
- `just contracts-verify-released` — expected failure with 49 unresolved draft warnings.
- `git diff --check` and per-report `git diff --no-index --check` — passed.
