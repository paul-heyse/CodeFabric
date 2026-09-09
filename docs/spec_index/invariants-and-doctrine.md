# Product invariants and design guidance

The [selected suite](README.md) owns fact-only meaning, source authority, application-owned identity, provider isolation, raw/normalized distinctions, conflict/unknown semantics, coherent snapshots, scoped completeness, authorization and publication ownership.

[Current P1–P36 guidance](../library_ref/full_data_fabric_design_principles_v2.md) preserves these invariants while allowing ordinary Rust definitions and proportional tests. Arrow/DataFusion/Delta remain the fabric. Shared budgets, bounded work and state, owned task lifetimes, provider containment, measured headroom and recovery replace universal allocation pre-admission.

Runtime actions leave compact records. Git records code edits. Generalized independent proof execution, full intermediate histories, plan activation, source bundles and proving-commit ledgers are not required. [AGENTS](../../AGENTS.md) owns the short delivery workflow.
