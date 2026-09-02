# CodeFabric 2.3 specification index

This directory is a derived navigation layer over the current
codefabric-relational-data-fabric suite. It is never normative. Cite the
authoritative section it points to, not this index.

## 1. Current-suite discovery

Current masters are discovered from YAML frontmatter and predecessor edges under
docs/authoritative_design. A current master is in the unique synchronized terminal suite and has:

- suite_id codefabric-relational-data-fabric;
- suite_version 2.3.0;
- one unique artifact_tag from SUITE, ONT, GEN, FAB, QRY, LIFE, SRV, or RM;
- authority_status current;
- one predecessor_path naming its immutable v2.2 predecessor, whose chain continues through v2.1, v2.0, and v1.3.

The authoritative-design-conformance-check proves that exactly one terminal
master owns each role, the eight terminals share one suite version, every complete predecessor
chain exists, any authored successor link agrees when present, navigation selects the terminal
paths, and no generated suite manifest is semantic authority. An ancestor's issuance-time
`authority_status: current` is tolerated because the successor edge, not a rewritten status,
determines terminality.

## 2. Citation convention

Use TAG §N plus the section title. The current tag mapping is:

| Tag | Current master |
|---|---|
| SUITE | codefabric_present_state_cpg_suite_governance_and_release_manifest_v2.3.md |
| ONT | code_property_graph_present_state_fact_ontology_specification_v2.3.md |
| GEN | present_state_cpg_fact_generation_specification_python_rust_v2.3.md |
| FAB | present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md |
| QRY | code_property_graph_semantic_query_specification_v2.3.md |
| LIFE | codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md |
| SRV | present_state_cpg_fastmcp_serving_specification_v2.3.md |
| RM | codefabric_2.3_implementation_roadmap_v1.0.md |

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
| invariants-and-doctrine.md | where cross-cutting v2.3 invariants are owned and proved |

## 5. Historical boundary

The v2.2, v2.1, v2.0, and v1.3 masters remain adjacent as immutable design/release history. They are
never runtime, build, package, or acceptance authority for the v2.3 target. Human and agent
navigation, new design work, and implementation decisions select only the v2.3 terminal masters.

## 6. Failure interpretation

No search result is converted into semantic absence. A missing current section,
contract, provider family, or proof obligation is either a navigation error or
an explicit program/capability gap. Generated predecessor files are not fallback
authority.
