# CodeFabric 2.1 specification index

This directory is a derived navigation layer over the current
codefabric-relational-data-fabric suite. It is never normative. Cite the
authoritative section it points to, not this index.

## 1. Current-suite discovery

Current masters are discovered from YAML frontmatter under
docs/authoritative_design. A current master has:

- suite_id codefabric-relational-data-fabric;
- suite_version 2.1.0;
- one unique artifact_tag from SUITE, ONT, GEN, FAB, QRY, LIFE, SRV, or RM;
- authority_status current;
- one predecessor_path naming its immutable v2.0 predecessor, whose own predecessor is v1.3.

The authoritative-design-conformance-check proves that exactly one current
master owns each role, every complete predecessor chain exists and is historical, navigation
selects the current paths, and no generated suite manifest is semantic
authority.

## 2. Citation convention

Use TAG §N plus the section title. The current tag mapping is:

| Tag | Current master |
|---|---|
| SUITE | codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.1.md |
| ONT | code_property_graph_present_state_fact_ontology_specification_v2.1.md |
| GEN | present_state_cpg_fact_generation_specification_python_rust_v2.1.md |
| FAB | present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.1.md |
| QRY | code_property_graph_semantic_query_specification_v2.1.md |
| LIFE | codefabric_continuous_cpg_update_lifecycle_management_specification_v2.1.md |
| SRV | present_state_cpg_fastmcp_serving_specification_v2.1.md |
| RM | codefabric_2.1_implementation_roadmap_v1.0.md |

Historical citations retain their explicit filename/version. Historical
AC-G-01 through AC-G-84 identities remain valid evidence but do not select
current generated registries.

## 3. Navigation commands

- just spec-outline lists all current and historical masters.
- just spec-outline PATH --match '^N\.' narrows to a section.
- just lib-outline PATH navigates the pinned library reference corpus.
- just plan-status reports the active plan, input freshness, and proving trust.

The authoritative masters carry their own section structure; this index does
not duplicate line counts, part counts, generated artifact inventories, or
contract censuses.

## 4. Files in this index

| File | Derived question |
|---|---|
| fact-domain-map.md | how a fact family flows from provider observation to query result |
| library-routing.md | which pinned reference owns an implementation API decision |
| wave-traceability.md | which capability stage and work-packet family realizes a boundary |
| contract-census.md | how relational contract discovery replaces the static AC-G census |
| invariants-and-doctrine.md | where cross-cutting v2.1 invariants are owned and proved |

## 5. Historical boundary

The v2.0 and v1.3 masters remain adjacent as immutable design/release history. They are never
runtime, build, package, or acceptance authority for the v2.1 target. Human and agent navigation,
new design work, and new implementation decisions select only the v2.1 masters.

## 6. Failure interpretation

No search result is converted into semantic absence. A missing current section,
contract, provider family, or proof obligation is either a navigation error or
an explicit program/capability gap. Generated predecessor files are not fallback
authority.
