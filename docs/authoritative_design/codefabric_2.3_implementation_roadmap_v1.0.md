---
artifact: authoritative-design
artifact_id: codefabric-relational-data-fabric-roadmap
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: RM
artifact_version: 1.0.0
authority_status: current
predecessor_path: docs/authoritative_design/codefabric_2.2_implementation_roadmap_v1.0.md
---

# CodeFabric product delivery roadmap

> Target revised 2026-09-08 under the consolidated pragmatic delivery review. These are target contracts, not a claim of implemented behavior; see [current status](../../STATUS.md). Historical predecessors are unchanged.

## 0. Authority and sequence

The [suite governance](codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md) selects target semantics. [STATUS](../../STATUS.md) identifies current work. The editable [production plan](../plans/codefabric_pragmatic_production_implementation_plan.md) supplies concrete implementation tasks. This roadmap is sequencing guidance, not packet state or an activation ledger.

## 1. First useful product

1. Restore real daemon startup and persisted reopen; replace excessive proof/resource prerequisites at their callers before deleting machinery.
2. Deliver source/entity, references/imports/calls and available types with Python and Rust semantic providers through the real service.
3. Return precise installed support, snapshot processing coverage and unfinished scope; make syntax available while semantics converge.
4. Deliver conservative watcher-driven invalidation, reject obsolete completions, and compare final incremental answers to clean rebuilds.
5. Exercise the first four forms (FindEntities, FollowRelationships, RetrieveFacts, RetrieveSourceContext), cancellation, restart and bounded resource behavior on a compact mixed-language corpus.

## 2. Full product

Complete every ONT/GEN family, including Python/Rust CFG, dataflow, ownership, effects, resource lifetime, async/concurrency, graph analyses and summaries. Add FindPaths, MatchPattern, CombineResults and SummarizeFacts with all composition, precision, limit and unknown semantics. Extend corpus and tests alongside each capability.

## 3. Performance and simplification

Measure startup/reopen, query latency, edit-to-syntax, semantic convergence, backlog, RSS, disk and recovery on repeatable workloads. Keep conservative algorithms until measurements justify finer invalidation, overlays, caching or graph extensions. Remove redundant runtime code and native source patches after their consumers and useful fixes have explicit replacements. Preserve one Arrow/DataFusion type universe.

## 4. Completion

First-release success is not full-product completion. Record implemented behavior, precise gaps, relevant checks and next action in STATUS. Do not require synchronized design editions, source-freeze manifests, artifact replay or a separate audit cycle to integrate ordinary changes.
