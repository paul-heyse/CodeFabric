# Current design navigation

Updated 2026-09-08. This index is derived navigation, never semantic authority. The [suite governance](../authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md) explicitly selects the current working documents. Historical predecessor metadata does not select a current suite. Git revisions update current documents without synchronized successor issuance.

| Role | Current document |
|---|---|
| FAB | [present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md](../authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md) |
| ONT | [code_property_graph_present_state_fact_ontology_specification_v2.3.md](../authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.3.md) |
| QRY | [code_property_graph_semantic_query_specification_v2.3.md](../authoritative_design/code_property_graph_semantic_query_specification_v2.3.md) |
| GEN | [present_state_cpg_fact_generation_specification_python_rust_v2.3.md](../authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md) |
| LIFE | [codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md](../authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md) |
| SUITE | [codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md](../authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md) |
| RM | [codefabric_2.3_implementation_roadmap_v1.0.md](../authoritative_design/codefabric_2.3_implementation_roadmap_v1.0.md) |
| SRV | [present_state_cpg_fastmcp_serving_specification_v2.3.md](../authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md) |

Read [STATUS](../../STATUS.md) and the [production plan](../plans/codefabric_pragmatic_production_implementation_plan.md) for implementation progress and next work. Prior plans/reviews/state remain historical and must not be resumed automatically.

Use `just spec-outline` for the selected eight documents, or an explicit file/directory for historical navigation. `just lib-outline <path>` handles library references. Spec outlines emit numbered second-level headings; inspect Part/Appendix headings with a targeted text search. Cite a domain tag, section and title, and confirm the current heading. Line numbers in old indexes may have moved.

Other maps: [fact domains](fact-domain-map.md), [libraries](library-routing.md), [delivery outcomes](wave-traceability.md), [contracts](contract-census.md), [invariants](invariants-and-doctrine.md). `just docs-check` checks navigation/local links and tracked-output hygiene; it is not product certification.
