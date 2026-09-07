---
artifact: design-dossier
design_id: codefabric-real-time-cpg-planning-contracts
version: v2
date: 2026-09-07
status: accepted
baseline_commit: e24627a29ee44bc3f288eeb07197b66e6dec7c7e
primary_scope:
  - src
  - rustc-extractor
  - pyrefly-sidecar
  - codefabric-cpg-mcp
  - third_party/native
  - tooling/native-dependencies
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
predecessor_design_path: docs/designs/codefabric_real_time_cpg_planning_contracts_design_v1_2026-09-04.md
source_review_path: docs/reviews/implementation_status_codefabric_real_time_cpg_implementation_plan_v2_2026-09-04_2026-09-07_v2.md
approval_basis: The user explicitly approved the proposed native-resource amendment and authorized plan revision and resumed execution on 2026-09-07.
---

# Real-time CPG planning contracts: approved native resource amendment

## 1. Executive decision

Adopt LD-RT09 below and resume the complete real-time CPG implementation. This successor incorporates every contract, invariant, selected capability, alternative disposition and proof obligation of the accepted planning-contracts design v1 and its incorporated comprehensive-review design v1. D-RT01–D-RT07 and LD-RT08 remain binding. It resolves the native-resource design reopening reported in the implementation-status review v2; the user's explicit approval supplies acceptance authority, not implementation evidence.

The amendment is required inside WP79, before downstream acceptance can close. All WP77–WP106, M18–M25 and DB24–DB30 obligations remain. WP94 still owns the separate native maintenance correction; WP95 still requires actual safe reclamation. A resource patch does not satisfy either by implication.

The existing tree includes substantial concurrent and prior implementation work. The preservation snapshot for this revision is `/tmp/codefabric-native-amendment-20260907-m15dd4tj`; the successor plan records the complete activation snapshot through the repository's digest procedure. No packet becomes complete merely through this authority transition.

## 2. Constraints and target invariants

CPG meanings, the eight bounded query forms, present-state authority, provider isolation, native Delta transaction/log/planning authority, and the four first-party build domains remain unchanged. Arrow/Parquet 59.2.0, DataFusion 55.0.0 and the existing public type universe are retained. The currently selected Delta source remains the exact FAB §2.1 pin until a concrete amended artifact passes the versioned source-authority transition.

- Reserve finite resource capacity before native decoding, replay, collection growth or worker admission. An after-decode size observation alone cannot prove admission.
- Share the charge for original retained allocation ownership across clones and slices until the last owner releases it. Preserve lazy versus materialized native state; a replayed, projected or serialized copy is not evidence about retained backing.
- Preserve typed resource exhaustion through optional CRC and checkpoint-hint paths and every application error adapter. An exhausted optional optimization may not silently retry through an unbounded fallback.
- Bound actual prefetch, batch, channel, physical partition fanout, runtime workers, blocking workers and nesting. Validate ambient native configuration before any corresponding one-time initialization. A DataFusion target-partition setting alone is insufficient.
- Join every native worker and drain cleanup before releasing execution reservations or reconciling uncertain physical writes. Native retained state may leave an operation only with its continuing ownership receipt; thread-bound engine/runtime state may not escape a joined lane.
- Current, candidate, leased and query state consume one aggregate process/workspace envelope. Reserved control capacity stays responsive under data pressure. Allocation overhead and runtime containment require separate accounting and measurement; this does not equate reservations with RSS.

These constraints apply P3/P12/P13/P14/P18/P19/P23/P25/P26/P31/P32/P34/P35/P36 to the concrete native boundary. They do not create an authored CPG fact registry, a second semantic engine, or an application Delta planner.

## 3. Target architecture and library decision

### LD-RT09 — Native allocation admission and retained-state ownership

**Decision:** build a narrow resource correction on the pinned native sources, then adopt its reviewed immutable artifact. **Version basis:** Delta `43a0cf10a313e5077c48637ad786a05359136bbb`, buoyant_kernel 0.25.1, buoyant_kernel_engine 0.25.0 and Arrow/Parquet 59.2.0, with DataFusion 55.0.0 unchanged. These are upstream origins, not a fabricated amended revision. **Displaces:** unproved native allocation assumptions, copied-state ownership proxies, unbounded native defaults and permanent resource-closure denial. Retains native decoding, replay, planning and transactions. **Risk:** hidden allocation paths, decoded expansion, error swallowing, retained clone lifetimes and dependency reproducibility. **Validation:** `just real-time-cpg-native-resource-check`, `just native-dependency-artifacts-check`, WP79/M18, native compatibility, feature/source graph, and the full relevant final gate matrix.

The native API has four required capabilities; concrete names and source decomposition belong to implementation:

1. Borrow original retained materialized batches from eager snapshot/log state without replay or copying. Visit retained non-Arrow schema, configuration, CRC, path and replay allocations or attach native shared receipts at their creation. Distinguish unique allocation, shared allocation and transient borrowed observation, including Arc clone/drop behavior.
2. Provide fallible admission at native allocation boundaries. Cover JSON token/string/container decoding, CRC and schema parsing, checkpoint/footer collections, compressed page and dictionary expansion, decoded variable-length values and nested offsets, cumulative replay maps and accumulated batches. Reject overflow, excessive nesting/counts and oversized individual values before the corresponding allocation. Reuse existing sound native limits; where a library lacks a sufficient seam, the approved amendment includes the smallest necessary Arrow/Parquet correction. No heuristic compressed-byte multiplier or infallible pool callback substitutes for this proof.
3. Couple admitted retained allocations to shared lifetime receipts, and transient allocations to the owning operation. The application supplies a generic budget/admission interface; the library supplies native cost/ownership observations. Error conversion preserves capacity class and bounds owned diagnostic strings. No CPG policy enters the decoder.
4. Accept a finite explicit execution policy in all native engine construction paths, including Delta's log-store engine and DataFusion file-format/statistics handlers. Enforce physical worker/channel limits where tasks are actually created. Optional CRC and last-checkpoint readers must propagate typed resource exhaustion instead of converting it to absence.

### 3.1 Reproducible source and pin authority

Use a committed content-addressed third-party source artifact and exact-version Cargo patches. Its manifest records exact upstream Git revisions or registry archive checksums, ordered reviewed patches, licenses, package identities and complete resulting file-tree identity. A verifier reconstructs upstream plus patches and rejects modified, missing or extra artifact files, unsafe symlinks, unexpected external path dependencies and wrong resolved package sources. Verification runs before Cargo and in an independent gate because Cargo.lock does not checksum path dependencies. Staging paths and Cargo cache edits cannot become dependency authority.

The artifact contains third-party dependency source, not a fifth first-party application build domain. First-party formatting and tests select their exact packages; native amendment checks select explicit artifact packages. If a public Arrow package is amended, all affected independent roots receive the same exact patch identity and compatibility proof.

No unknown future artifact hash is selected now. Before switching dependency resolution, generate and review the concrete artifact, issue a synchronized suite successor whose FAB source authority names that identity, and issue the corresponding successor implementation plan with exact declared inputs. Preserve all stable work IDs and migrate judgments through validated activation. The user has approved this bounded native-resource amendment; producing its concrete artifact and authority transition requires no repeat design confirmation. External publication, upload or remote push remains outside this local implementation authorization.

## 4. Alternatives and clean-sheet challenge

Retain the predecessor's clean-sheet and native-library alternatives. A complete bounded-input proof could satisfy allocation admission without changing a particular decoder, but no complete proof has been established for the existing Delta/kernel/Parquet paths. Input byte caps alone miss compressed and dictionary expansion; handler callbacks receive data after allocations; Snapshot serialization creates another buffer; projected file batches do not expose all original retained state. These are rejected as standalone closure proofs, not as mathematical claims that a bound is impossible.

Prefer existing native limit APIs wherever source and falsifiers prove sufficiency. A thread/process memory limit alone does not provide retained shared ownership, and another semantic or storage process is not selected. A global infallible Arrow pool cannot turn an already allocated batch into reserve-before-decode evidence. Resource denial remains valid interim behavior but cannot count as successful target conformance or fresh-update convergence.

## 5. Transition, cutover and legacy disposition

Preserve the tested hierarchical budgets, charged provider/source/result ownership, native task tracer, joined execution lane, owned local store and workspace-operation primitives. Complete their production integration across control open/provision/append, candidate generation and sealing, selected reconstruction/proof, query materialization/planning/execution, CDF and maintenance. Register physical roots and account uncertain native writes until joined reconciliation. A physical census has no Delta semantic authority.

WP77 and WP78 retain historical proofs as stale until their repaired consumers have coherent proving lineage and current checks. WP79 resumes under this approved contract. WP80–WP106 return to their unchanged dependency-ordered execution; their earlier invalidation was an acceptance dependency, not deleted scope. M18–M25 and DB24–DB30 retain every member and exit. Remove superseded ownership bypasses as each production route cuts over, with DB30 and WP105 final absence and target-positive proof.

Keep prior plans, designs, status reports and state files as history. Create fresh successor state through activation and carry forward deviations, failed approaches, unresolved implementation obligations and historical proving commits. Clear only the resolved request for design approval. Do not turn prior working-tree checks into terminal certification.

## 6. Proof strategy

Use independently constructed falsifiers for compressed oversized checkpoints, huge/nested schema and CRC inputs, cumulative small-action replay, reject-before-decode behavior, resource-error propagation through CRC and checkpoint fallback, original eager-buffer identity across clone/drop, cancelled hidden native workers, physical fanout and environment-limit rejection, concurrent candidate/query/retained pressure and reserved control responsiveness. Exercise production constructors as well as native unit seams. Assert that rejection precedes the guarded allocation/work counter, rather than inspecting only final size.

Prove artifact tampering, missing/extra source and wrong package resolution fail before Cargo; replay ordered patches from exact upstream inputs; inspect the native diff and locked dependency graph. Run `just data-fabric-upgrade-check`, `just stable-graph-check`, affected feature/build-domain checks and policy gates at the concrete pin transition. WP79 requires its complete packet gates and new native-resource gate before M18. WP94/WP95 retain their distinct maintenance and reclamation falsifiers. WP102–WP106 still require installed two-language behavior, combined failure recovery, clean target-only quality, preregistered measurements and independent exact-candidate certification.

accepted
