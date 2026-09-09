---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-suite
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: SUITE
artifact_version: 2.3.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.2.md
---

# CodeFabric product target and suite governance

> Target revised 2026-09-08 under the consolidated pragmatic delivery review. These are target contracts, not a claim of implemented behavior; see [current status](../../STATUS.md). Historical predecessors are unchanged.

## 0. Current selection and precedence

This table explicitly selects the current working domain documents. Versioned filenames retain their historical identity; ordinary Git revisions update this target without synchronized successor issuance or plan activation. Predecessor suites are historical. Navigation checks validate these links and unique roles, not implementation correctness.

| Role | Selected document |
|---|---|
| FAB | [present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md](present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md) |
| ONT | [code_property_graph_present_state_fact_ontology_specification_v2.3.md](code_property_graph_present_state_fact_ontology_specification_v2.3.md) |
| QRY | [code_property_graph_semantic_query_specification_v2.3.md](code_property_graph_semantic_query_specification_v2.3.md) |
| GEN | [present_state_cpg_fact_generation_specification_python_rust_v2.3.md](present_state_cpg_fact_generation_specification_python_rust_v2.3.md) |
| LIFE | [codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md](codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md) |
| SUITE | [codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md](codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md) |
| RM | [codefabric_2.3_implementation_roadmap_v1.0.md](codefabric_2.3_implementation_roadmap_v1.0.md) |
| SRV | [present_state_cpg_fastmcp_serving_specification_v2.3.md](present_state_cpg_fastmcp_serving_specification_v2.3.md) |

The user's accepted product decisions and consolidated review guide this revision. This document owns cross-domain contracts; domain specs own detailed semantics; the roadmap and editable production plan own sequencing; indexes only navigate. Resolve contradictions directly in the affected document. Git history records edits; no compiled plan, approval ledger, digest census, or proving chain selects development authority.

## 1. Product and first useful release

Build a full present-state code property graph for Python and Rust in a Rust daemon, with Arrow data, DataFusion query/relational execution, and native Delta persistence. Preserve every ONT/GEN fact family and all eight QRY forms as eventual scope. FastMCP presents daemon-owned results. The four isolated build domains and released wire identities remain.

The first useful release includes a real semantic contribution from both Python and Rust, source/entity lookup, references/imports/calls, available types, scoped status, real edit-driven updates, and persisted reopen. The initial forms are FindEntities, FollowRelationships, RetrieveFacts, and RetrieveSourceContext; FindPaths, MatchPattern, CombineResults, and SummarizeFacts follow explicitly in the production backlog. Syntax-only output cannot satisfy semantic delivery. Missing semantic work remains visible and does not suppress useful syntax.

Later scope includes complete CFG/dataflow/ownership/effects/resources/async/derived analyses, the remaining query forms, demanding recovery and security scenarios, and measured scale. This staging changes sequence, not the final CPG target.

## 2. Core semantic contracts

- Facts and mechanically derived facts only: no refactoring safety, risk, test-impact, or change verdicts. Git history, runtime coverage, and environment inventory are outside the semantic substrate.
- Source bytes are authoritative. Watchers and Git accelerate discovery but never replace authoritative reads.
- Raw provider kinds and normalized kinds coexist. Syntax occurrences and semantic entities remain distinct.
- Canonical identities are application-owned; compiler/tree/local indices remain provider provenance.
- Adapters own provider isolation and emit application-owned Arrow/DTO boundaries.
- Provider conflicts use explicit per-family authority, retain diagnostics, and yield candidates/unknown where unresolved.
- Every query pins one immutable snapshot and exact table versions. No silent mixture of generations or stale-current semantic facts.
- Missing output is scoped unknown, pending, partial, unsupported, failed, or stale history; it is never complete absence.

## 3. Execution and publication

Use ordinary typed Rust builders, DataFusion expressions/plans, and native Delta operations. Describe installed schemas and actual provider support where useful; do not create a generalized semantic release compiler, self-description fixed point, or independently re-executed proof engine as a prerequisite for useful results.

One owned publication coordinator checks source/context generation, schema and identity consistency, exact table versions, writer ownership, and coverage before atomically selecting a snapshot. In-flight obsolete results cannot publish. Reopen uses committed versions and publication records; routine reopen does not rerun all providers. Conservative invalidation and full rebuilds are acceptable early correct implementations.

## 4. Capability, completeness, and artifacts

Keep three dimensions separate: installed support, processing coverage for the requested workspace/snapshot/scope, and confidence established by release tests. A passing test does not certify arbitrary workspace facts. An unavailable provider does not invalidate unrelated completed work.

Runtime actions leave compact records useful for result explanation, diagnosis, replay, and recovery: operation/snapshot identifiers, source/context/provider versions, requested/completed/remainder scope, publication versions, terminal outcomes and diagnostics. Retain detailed intermediate data only when needed or explicitly requested. Do not create artifacts for source edits, shadow receipt systems, permanent full execution histories, or universal independent proof receipts.

## 5. Resource and storage contract

Share DataFusion memory/spill resources across current, candidate and leased work. Bound jobs, queues, batches, retained application state, result bytes, concurrency and deadlines. Own and join tasks through cancellation and shutdown; contain provider subprocesses. Measure RSS/disk headroom, apply backpressure, and recover from interrupted work. These controls do not promise pre-admission for every native allocation or immunity from OOM.

Delta owns transaction/log/schema/checkpoint/maintenance mechanics. Retention protects exact versions and active leases. Native maintenance and finite retention remain required; avoid application reimplementations of the Delta engine. Existing native patches remain installed until production consumers are replaced and useful fixes classified.

## 6. Serving and validation

Rust owns semantic state, authorization, bounded query execution, freshness and status. FastMCP owns presentation only. Preserve atomic Accepted/InputRequired/Rejected start, daemon-authored challenges and handles, reauthorization, bounded cancellation, and current modern protocol compatibility from SRV/QRY.

Use a small independently justified product corpus, relevant boundary tests, reusable clean/incremental comparison, and targeted lifecycle/failure cases. Run tests according to changed behavior and integration risk. Algorithms need clear invariants and meaningful examples; correctness does not require replaying each intermediate result during every update. A real failing product run stays failing. Product completion requires demonstrated behavior, not navigation or process checks.

## 7. Transition

Follow [the production plan](../plans/codefabric_pragmatic_production_implementation_plan.md) and [STATUS](../../STATUS.md). Replace consumers before deleting old runtime proof/resource machinery or native patches. Preserve released wire allocations and durable data deliberately; do not start unbounded dual writes or silently revive old runtime authority. Old plans and state files are history, never tasks to resume automatically.
