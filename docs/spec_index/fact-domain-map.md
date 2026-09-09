# Fact-domain navigation

Derived from the selected suite; consult the detailed [ontology](../authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.3.md) and [generation](../authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md) specifications for every family. The full target remains intact; rows group navigation rather than limit coverage.

| Domain | Target and provider/analysis boundary |
|---|---|
| Source, lexical, syntax, ranges | Byte-authoritative source images, Tree-sitter and Ruff; raw and normalized kinds coexist |
| Entities, bindings, modules, references | Application-owned identities; Ruff/Pyrefly and rustc contexts, explicit conflicts and ambiguity |
| Types, members, calls | Semantic observations from Pyrefly/rustc; distinguish occurrences from resolved targets and possible sets |
| CFG, exceptions, program points | Python application CFG and Rust MIR; model actual evaluation/control order |
| Dataflow, memory, ownership, lifetimes | Application analyses with precision; exact compiler observations retain distinct provenance |
| Effects, resources, async/concurrency, closures | Direct and derived families, explicit unknown propagation and captured context |
| Graph analyses and summaries | Bounded SCC/reachability/dominance/control-dependence/loops and interprocedural summaries |
| Coverage and processing | Installed support, actual requested/completed/remainder, freshness and release-test confidence remain distinct |

[FAB](../authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md) owns Arrow/DataFusion/Delta representation and exact snapshots; [LIFE](../authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md) owns updates/publication; [QRY](../authoritative_design/code_property_graph_semantic_query_specification_v2.3.md) and [SRV](../authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md) own query meaning and presentation. No proof-process ledger is a fact source.
