---
artifact: design-dossier
design_id: codefabric-real-time-cpg-planning-contracts
version: v1
date: 2026-09-04
status: accepted
baseline_commit: df1c50c684a5e005c4b76ada9cd19d8030d9d7dd
working_tree_digest: 8bed1748d451922a9d6a37af22727c5c42688647ba69dbfa65dcb5c49cad4d2d
primary_scope:
  - src
  - rustc-extractor
  - pyrefly-sidecar
  - codefabric-cpg-mcp
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
source_review_path: docs/designs/codefabric_real_time_cpg_comprehensive_review_design_v1_2026-09-04.md
---

# Real-time CPG planning contracts

## 1. Executive decision

This additive dossier resolves the planning blockers in the comprehensive review's §6.4. It incorporates that review's complete target, fourteen findings, LD-01–LD-07, capability selections, alternatives, inventory dispositions and proof strategy by reference. The user's request to plan realization accepts that architectural direction; this document records the concrete planning decisions needed to make it executable. It does not claim that these contracts are implemented, that the user approved new external operations, or that the review was already an accepted specification.

The synchronized v2.3 suite remains normative. The v7 compiled-release/provider/fabric/runtime boundary design remains a constraint, not an obsolete implementation to undo. Neither source artifact is rewritten. This addendum supplies realization contracts under them; a discovered contradiction reopens design instead of silently overriding a master. No new process boundary, conceptual Cargo root, arbitrary SQL interface or second persistent graph engine is selected. LD-RT08 below selects a narrowly scoped native Delta maintenance amendment as required future work; it does not change a dependency now or authorize publishing an upstream patch.

The current source diff still matches the review baseline. The planning risk class is documentation only; implementation will span all four build domains and requires the full relevant assurance matrix. The baseline `just ci-fast` again fails at `root-clippy`, after formatting and type checking, so this dossier confers no runtime certification.

## 2. Constraints and target invariants

The review's §2 invariants and v7 design I-60–I-71 are retained. The following decisions are binding on the new plan:

- **D-RT01 — Dependency evidence is closed or invalidation widens.** An owner can survive a change only through complete positive and negative support evidence. Advances P9/P10 provenance closure, P27 causal declarations and P28 computed change.
- **D-RT02 — Availability is not admission evidence.** Each selected capability state is a complete, proved epoch; optional unavailable semantics cannot excuse missing source, withdrawal, dependency, schema, policy or activation evidence. Advances P20 executable capability and P25 named oracles; maintains P11 atomic immutable state.
- **D-RT03 — Control semantics precede precision.** Application analyses consume explicit owner-local control/evaluation semantics. Their precision describes a named abstraction and executable transfer rules, not provider availability. Advances P1/P2/P30; maintains P14's highest viable native rung.
- **D-RT04 — Identity restoration is a checked field mapping.** Missing metadata and contradictory identity have different outcomes. A shape-preserving relabel is not semantic proof. Advances P12/P15/P21; the existing safe projection exclusions are a bounded transition, not the terminal design.
- **D-RT05 — Resource limits are aggregate and causal.** Workspace/process owners account all live work and retained state; per-epoch pools cannot multiply the budget. Advances P23/P31/P34; no process-RSS claim follows solely from DataFusion reservations.
- **D-RT06 — Empty replacement has an unconditional native-relational path.** Explicit owner replacement keys/tombstones and anti-join/union semantics are authoritative even when replacement has zero rows. Native predicate overwrite is an optional proved physical optimization, never a correctness dependency.
- **D-RT07 — Native maintenance never weakens command ownership.** Exact versions, approved resource closure, zero uncontrolled retry, operation reconciliation and forward recovery govern every maintenance operation. A verified native API limitation remains a typed unavailable operation, never a bypass.

## 3. Target architecture and design-bearing contracts

### 3.1 D-RT01: positive, negative and unknown dependencies

Provider and analysis output carries support relations with these semantic dimensions:

```text
consumer owner / relation family / context / producer release
  -> dependency kind
  -> searched or consumed scope and typed lookup key
  -> selected namespace, ordered roots, policy and effective-context identity
  -> exact input revision or closed search-universe identity
  -> resolved candidates, proven absence, or incomplete-search remainder
```

Dependency kinds distinguish source bytes, context/build configuration, provider/analysis release, positive facts, imports/exports, name/member/implementation lookup, callable summaries, and graph projection support. A lookup's failed outcome retains the searched namespace and selection inputs; no positive edge is required for it to be invalidated. These are application-owned typed observations, not a generic string-predicate language or an authored current-impact registry.

Creation/deletion/rename, export/member/impl-set changes and changed stubs/search roots invalidate matching search dependencies, including formerly absent candidates. The change relation joins support and owner relations and closes transitively. If an API cannot supply sufficient support, invalidate the full relevant namespace/context or reverse-dependency closure; if that closure itself is unproved, widen to the selected context. A changed byte or missing proof never licenses a narrow cache hit.

Regeneration replaces a complete owner/family/context partition, including its previous unknowns, diagnostics, coverage, dependencies and summaries. Empty output still emits a replacement key. Cross-owner references to withdrawn entities are recomputed or become explicit unknowns. SCC split/merge and edge deletion cause affected-component recomputation; monotonicity inside one fixed-point computation is not monotonicity across epochs.

### 3.2 D-RT02: admitted capability states and generation ownership

The immutable compiled release supplies separate typed obligations for source-current, syntax-current and semantic-current availability. They do not introduce a second suite selector or mutable profile registry.

Required for **every** candidate: closed authorized source inventory; one terminal capture disposition per inventoried source or unresolved work that prevents claiming its closure; valid source/context/generation fences; explicit replacement/withdrawal; closed support/provenance; valid schemas and owner/endpoints; exact durable inputs; lawful authorization, proof and activation. A deferred capture remains dirty and cannot disappear from the inventory.

A syntax-current candidate may contain proved pending/unavailable semantic coverage because LIFE §6.1 explicitly permits it. It must withdraw invalidated semantics and prove every retained owner's support. LIFE §7 still rejects an unknown **required admission input**. Semantic-current additionally requires all requested selected-profile families to have their proper producer result, admitted language uncertainty or concrete input/trust/API limitation. Unimplemented required functionality is not a terminal conformance waiver.

One update owner controls inventory/watcher state and provider jobs; one command actor controls durable writes and activation. Expensive computation occurs outside the actor's critical section. Before publication the actor revalidates exact predecessor, writer generation, source/context/dependency closure and authority. Late work is discarded or lawfully rebound only after dependency equality is proved; changing a label cannot reauthorize stale facts.

Retained Pyrefly state receives both the **complete selected module inventory** and a distinct changed-work set. The existing `analyze_modules` removes retained modules omitted from its supplied module list; the new boundary must not confuse a dirty subset with a complete inventory. Context mutation is serialized or snapshot-isolated; cancellation joins work before the state is reused. Aging/coalescing and stable-watermark work provide progress after a bounded quiet window and observable fairness under continued unrelated edits.

Deleting the last module is a valid transition to a proved empty inventory, not a provider failure. The existing empty-list rejection must not bypass deletion. Withdraw all old facts/support, join or retire checker state, and prove delete-last/recreate behavior.

### 3.3 D-RT03: owner-local control and analysis precision

Python control owners distinguish module, class execution, callable/lambda and comprehension/generator scopes. Creation of a nested callable does not execute its body. Control/evaluation relations preserve ordered operand evaluation, short-circuit branches, loop entry/back/exit, normal and exceptional successors, handler selection, return/break/continue/raise, pending abrupt completion through `finally`, context-manager entry/exit, pattern alternatives, suspension/resumption and unresolved successor remainders.

A program point identifies an owner, source/evaluation occurrence and transfer position. CFG, evaluation-order, value-flow and memory-access facts are separate relations. The selected analysis defines finite abstract locations/state, transfer, join, initialization, convergence and bounds; exactness is relative to that named abstraction and closed inputs. Distinguish exact compiler-private loans from application borrow approximations, and generic Rust bodies from executable instances. Preserve normal/unwind/cleanup/resume MIR edges.

Native expressions, joins, windows and bounded recursive plans are preferred where they preserve these semantics. Irreducible structured-control construction is a small versioned application analysis over owned observations, not a fake raw provider family. Alias/effect/resource/async/capture and interprocedural summaries depend on the correct control substrate. A limit, unsupported construct or missing dependency widens uncertainty rather than producing a complete set.

Independently authored expected examples cover every selected family, including GEN's lettered relationship sections. Both clean and incremental executions are compared to those expectations; comparing two runs of the same algorithm is insufficient. Existing useful kernels may be reused only after their input mapping, ownership and production causality are proved.

### 3.4 D-RT04: optimizer-visible identity

Logical field identity is derived from the installed typed input/transform expression, including occurrence/alias position where multiple equal-shaped fields exist. An explicit field mapping binds logical fields to physical output positions. Pass-through identity may be restored when a native projection drops annotation; a conflicting present identity, ambiguous mapping, reordered equal-shaped mismatch, wrong type/nullability or invalid logical-ID encoding is rejected. Computed expressions receive their actual output contract, not a copied input identity.

Validate ingress, analyzed/optimized logical plans, physical boundaries, stream/batch output and sinks—not only final output. Preserve requested projection/filter/limit/statistics mapping, filter-only columns, partitioning, ordering, equivalence, metrics and child replacement semantics. The narrow generic boundary may repair missing annotations; it may not inject CPG policy or invent statistics.

At the pinned DataFusion version, use the existing generic view/schema boundary and supported plan/expression APIs. Restore native projection rules only after real nested Delta/view/alias/join/aggregate cases and their identity-swap mutants pass. If this cannot be achieved without domain-specific optimizer forks, reopen the library decision. The plan does not promise a new undocumented upstream API.

### 3.5 D-RT05: budgets, ownership and exhaustion

The process has a root budget; each workspace owns a subordinate envelope; updates, provider jobs, queries, graphs and results receive reserved sub-budgets. Current, candidate and leased epochs share that envelope. Bound bytes, rows, individual values/pages, concurrent jobs, queued work, retained generations, overlay count/bytes/replacement keys, scan amplification, spill and disk headroom. Share immutable allocation charges by ownership identity; charge retention once for its actual lifetime, not zero times for Arc clones or repeatedly as new allocation.

Admission reserves before work; RAII/owned terminal paths release exactly once after joins. Provider-process limits, IPC decoding, non-DataFusion graph/AST heaps and adapter JSON/base64 expansion remain separately accounted. Hard process/containment limits and measured allocator overhead supplement—not silently equal—DataFusion pool accounting. Missing accounting for a large allocation path blocks the resource claim.

A validated release-owned runtime policy supplies positive finite limits and scheduling weights, intersected with supervisor-authorized grant ceilings. Concrete workstation values are preregistered and measured before performance certification; they may be tuned without changing this ownership contract. No epoch constructor independently creates another full workspace budget. Existing valid leases remain honored; capacity exhaustion defers/rejects new work or requests lawful consolidation, never evicts active pins or publishes partial results as complete. Control/status/cancel/release retains reserved capacity.

### 3.6 D-RT06/D-RT07: selective durability and constrained maintenance

Durability follows FAB §9.4. Restart/provenance/invalidation/proof-bearing relations remain Delta histories where required; immutable Arrow segments are durable only after validated bytes and exact pins. Stable relation roots and unchanged version reuse replace epoch-root genesis/full-replacement policy. Effective state is the native owner anti-join plus replacement union defined by FAB §10.

Explicit replacement keys make zero-row replacement correct without relying on empty `replaceWhere` behavior. A pinned `WriteBuilder` predicate path is admitted only after empty/null/schema/constraint and unaffected-row probes; otherwise the already-selected native effective-state path applies. Controlled consolidation uses the current zero-retry writer and exact session, proves row/provenance/unknown/public-result equality, then activates. Its delete/insert CDF behavior is explicit and cannot masquerade as native optimize's `data_change=false`.

CDF consumers stream bounded exact ranges, record the applied interval with downstream durable effects, and reconcile checkpoint loss. Retention protects snapshot Add files **and** required CDF intervals/logs/schema eras, pending commands, releases, expectations, immutable segments and leases. Finite capacity and bounded consumer lag are mandatory even while an unsafe native deletion operation is unavailable. Missing history triggers an explicit exact-snapshot rebuild contract or typed failure, not empty success.

The pinned OptimizeBuilder's hidden retry/property behavior and public vacuum's inability to consume its reviewed private plan remain known constraints. Vacuum also reconstructs default properties for its START commit, losing the supplied application transaction/retry contract before deletion. An ObjectStore deletion fence cannot fix that earlier violation. Keep these operations denied until the complete native contract is proved.

Denial and storage-pressure rejection satisfy interim safety, not full maintenance completion. The plan retains mandatory native-capability amendment and reclamation work. Full-target completion requires repeated bounded operation with eligible unleased storage actually reclaimed while protected versions, CDF intervals and provenance remain reconstructible. No test may count a refused update as successful fresh-update convergence.

### LD-RT08 — Native Delta maintenance contract amendment

**Decision:** build a narrow correction at the native library boundary, then adopt only its certified immutable artifact through a versioned pin-authority amendment. **Version basis:** the exact `43a0cf10` source and retained DataFusion/Arrow universe; no hypothetical replacement revision is asserted. **Displaces:** permanent inability to reclaim eligible obsolete Delta data; does not displace delta-rs planning, transaction-log authority or CodeFabric activation. **Risk:** a new dependency identity, all maintenance subcommits, schema/CDF retention and supply-chain reproducibility. **Validation:** the successor plan's proposed `real-time-cpg-native-maintenance-check`, existing `data-fabric-upgrade-check`, `stable-graph-check`, `features-each`, `policy` and full relevant compatibility matrix.

The correction must preserve caller commit properties, operation/application transaction identities and zero retry for **every** maintenance commit, including START/END. Native code continues to calculate candidates; expose an opaque native prepared-operation boundary or an equally strong native approved-set enforcement contract bound to exact root/version/cutoff/operation. The application supplies a proved lease/CDF/provenance protection closure and approves the native candidate set. Execution rejects a changed predecessor or any unapproved deletion. Lease admission and the maintenance protection generation must be coordinated so a concurrently acquired lease cannot newly protect an object after its deletion has been approved. Retry/unknown-outcome handling remains with the command actor, including interrupted partial deletion of only approved eligible objects.

Prefer a minimal certified upstream correction that satisfies this contract when available; otherwise implement and prove the narrow correction on the pinned source. This is a required implementation deliverable, not a claim that today's public API already works. Do not implement an application Delta log/file-action planner. Native optimize remains unselected unless its entire commit contract is certified; controlled zero-retry consolidation already meets that need.

Before switching the pin, create the required versioned dependency/authority amendment, identify the actual reproducible source artifact and review the dependency diff. Preserve the existing Arrow/DataFusion type universe and four first-party build domains. No external publication, registry upload, remote push or new deployment is authorized by this document; obtain any needed distribution authority separately. If a reproducible approved dependency artifact cannot be supplied, this packet and full-target completion remain blocked, not waived.

## 4. Alternatives and clean-sheet challenge

The review's alternatives remain: full rebuild as the clean oracle, dependency-maintained relational fabric as the product, and a second persistent graph/differential engine deferred pending evidence. This addendum chooses the smallest sound affected-owner recomputation before general incremental view maintenance. Typed failed-lookup dependencies and explicit empty replacement avoid correctness depending on a specialized differential engine.

Rejected shortcuts include treating all gaps as admissible, rebinding stale sidecar state with a new digest, declaring file-sequential CFG complete, relabeling equal-shaped conflicting fields, granting each epoch its own aggregate budget, and bypassing native maintenance constraints. Each fails a concrete contract above, independently of the current file layout.

Independent provider/lifecycle, predecessor-scope, storage/API and fresh contract reviewers challenged closure. Their material findings are resolved here: inventory-versus-dirty and zero-inventory Pyrefly transitions, full GEN family breadth, historical ID collisions, inactive-plan validation, explicit identity mapping, and mandatory reclamation rather than denial-only completion. These influence contracts and packet boundaries; they are not runtime test results.

## 5. Transition, cutover and legacy disposition

The generated 40-Rust-file plus wire/presentation/tooling inventory and every disposition in review §5.2 are incorporated unchanged. The plan allocates each surface to a current-tree preflight and coherent cutover; no new source decomposition is required. Existing v7 architecture/feature/transport/task-tree obligations and physically deleted predecessor authority remain binding.

Truthful capability, shared contracts and independent expectations precede provider/analysis cutover. Actual two-language initial execution precedes continuous-update certification. Correct local analyses precede dependent common summaries. Selective durable updates, complete query composition and optimizer work converge before terminal measurements. Old shortcuts are removed with their proved replacements, not preserved as user-selectable fallback routes.

The original review, existing plans/states and authoritative masters stay immutable during this planning task. A new inactive plan names its future schema-2 state and predecessor; approval, activation, state creation and execution are distinct later actions. FreshActivation and forward-only recovery remain the only deployment transition. Discovery of an actual deployed predecessor requiring migration reopens that contract.

## 6. Proof strategy and planning acceptance

Every D-RT decision is assigned four named integrity/behavior/negative/operations oracles in the successor plan, including actual production consumers. Existing Just recipes are extended under their owning packets; newly named checks are explicitly proposed until implemented. Selectors reject zero matches and seeded child failures propagate. Neither a proposed recipe nor a passed document validator proves the product.

Named assumptions retained from the review:

- **A01 — optional native predicate fast path.** The storage packet owns executable predicate probes. Empty replacement correctness uses D-RT06 regardless. A failed fast-path probe cannot weaken rows, retention or command semantics. Required maintenance is LD-RT08, not an optional A01 waiver.
- **A02 — retained-provider performance.** Provider/update packets prove effective context, inventory separation, joined cancellation, clean equality and progress under churn before claiming a speedup. Conservative broader recomputation is valid when actual affected scope is unavailable.
- **A03 — numerical operating targets.** The performance packet preregisters corpus, machine, limits, timing method and thresholds, then measures actual installed behavior. Review latency numbers remain proposed objectives, not results. Failure requires optimization or an explicit accepted target revision, not restamped evidence.
- **A04 — optimized identity feasibility.** The optimizer packet owns the proposed `real-time-cpg-optimizer-identity-check`: supported native view/expression/physical-plan seams must satisfy the nested Delta/view/alias/join/aggregate field-map oracle with stock projection rules enabled. Source confirms the extension seams exist; failure of the behavioral probe blocks optimized-path acceptance and reopens that library decision. This is mandatory, unlike A01's optional fast path.

The five architectural blockers in review §6.4 now have selected contracts D-RT01–D-RT05 rather than unspecified design tasks. LD-RT08 selects the native-layer correction and acceptance contract instead of assuming an incompatible current API will suffice. Physical fast-path, optimizer feasibility and numerical performance remain named assumptions with explicit consequences. No assumption permits unimplemented required language/profile or reclamation functionality to count as complete.

Accepted as the planning target under the user's realization request, with the named assumptions above. Plan approval, dependency-artifact approval, activation and execution remain separate from this design decision.

accepted-with-named-assumptions
