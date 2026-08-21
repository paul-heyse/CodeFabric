# CodeFabric Present-State CPG Suite Governance and Release Manifest

**Artifact ID:** `codefabric-present-state-cpg-suite-manifest`
**Artifact kind:** Normative suite manifest
**Status:** Released normative specification
**Version:** 1.3
**Compatible suite major:** 1
**Canonical digest:** External; recorded in `codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/artifact-index.json`
**Release date:** 2026-08-20
**Supersedes:** CodeFabric synchronized specification suite 1.2 plus the standalone architecture-completion override
**Audit integration (2026-08-20):** Plan-audit F-001; clarified executable phrase mappings and owner approval of initial machine-contract allocations.
**Implementation clarification (2026-08-20):** WP06; fixed the registry-YAML to canonical-JSON projection policy so YAML-only constructs cannot acquire implicit machine semantics.

---

## 0. Purpose and release authority

This manifest is the cross-cutting authority for the synchronized CodeFabric present-state CPG **1.3** design release. It governs artifact ownership, terminology, compatibility, machine-contract generation, the default deployment profile, conformance, performance, security acceptance, and upgrade/rollback behavior.

The six domain documents contain the permanent detailed contracts for their assigned gaps. The former `codefabric_architecture_completion_and_missing_design_specifications_v1.0.md` is historical source material only; no implementation needs it to interpret the 1.3 release.

### 0.1 Released domain artifacts

| Domain | Artifact |
|---|---|
| Ontology | [`code_property_graph_present_state_fact_ontology_specification_v1.3.md`](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| Fact generation | [`present_state_cpg_fact_generation_specification_python_rust_v1.3.md`](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| Data fabric | [`present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md`](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| Continuous lifecycle | [`codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md`](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| Semantic query | [`code_property_graph_semantic_query_specification_v1.3.md`](./code_property_graph_semantic_query_specification_v1.3.md) |
| FastMCP serving | [`present_state_cpg_fastmcp_serving_specification_v1.3.md`](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |

### 0.2 Global invariants

1. `workspace_id` identifies exactly one authorized source instance.
2. One immutable leased `ServingSnapshot` is the only query pin.
3. Current source bytes, not watcher events or Git objects, are present-state authority.
4. Context-sensitive facts never cross analysis-context boundaries.
5. Provider observations are never canonical graph state until reconciled.
6. Unknown remainder is explicit; absence is not inferred from missing data.
7. Public source disclosure is authorization-scoped independently from fact access.
8. A partial stream is not a successful logical response until its terminal completeness record is emitted.
9. Operational state is not semantic program history.
10. Every compatibility-sensitive artifact is versioned and fingerprinted.

---

# Part I — Governance and machine contracts

## AC-G-01 — Master architecture, terminology, ownership, and precedence
### Decision

The architecture is governed by a seven-artifact hierarchy: the six synchronized domain specifications plus this suite governance and release manifest. Every transformation has one owning layer, and every machine artifact has one generating source.

### Contract

The permanent ownership map is:

| Concern | Permanent owner |
|---|---|
| Fact meanings, kind hierarchy, evidence semantics, IDs, unknowns, projections, summaries | Ontology specification |
| Source-image contract, provider adapters, provider protocols, generation completeness | Fact-generation specification |
| Arrow/Delta schemas, reconciliation, derivation materialization, snapshots, table providers | Data-fabric specification |
| Registration, watching, invalidation, state transitions, operational database, freshness barriers | Lifecycle specification |
| Controlled semantic language, `PlanSpec`, completeness proofs, canonical response | Semantic-query specification |
| RPC framing, credentials, MCP delivery, artifacts, adapter contracts | FastMCP serving specification |
| Cross-cutting compatibility, defaults, machine-artifact governance, conformance and upgrade choreography | Suite Governance and Release Manifest 1.3 |

The canonical component topology is:

```text
workspace registry and authorization
        ↓
WorkspaceCoordinator actor
        ├─ source inventory and immutable source-image store
        ├─ watcher/Git interpretation and update-wave scheduler
        ├─ provider job manager
        ├─ reconciliation and derivation engine
        ├─ durable publication manager
        └─ active ServingSnapshot pointer
                ↓
        overlay-aware DataFusion catalog
                ↓
semantic resolver → typed PlanSpec → execution → canonical response/artifact
                ↓
per-agent FastMCP STDIO adapter
```

Canonical terminology SHALL use these meanings:

| Term | Meaning |
|---|---|
| workspace | One registered and authorized source instance; one Git worktree or one non-Git root |
| repository | Optional common Git repository parent shared by one or more workspaces |
| context | One deterministic Python or Rust semantic/build configuration |
| context set | Ordered immutable set of contexts pinned by a snapshot |
| owner | Smallest deterministic current-state replacement unit for a fact family |
| provider observation | Provider-owned evidence prior to canonical reconciliation |
| canonical fact | Reconciled first-class relation, property, or entity-existence proposition |
| durable publication | Immutable Delta version map for a coherent durable base |
| hot overlay | Immutable in-memory effective-state delta over one durable publication |
| ServingSnapshot | Durable base plus consolidated overlay and all interpretation metadata |
| capability | Named fact-production ability for a declared scope/context/profile |
| completeness | Whether the relevant fact universe is closed for a declared proof scope |

A downstream layer SHALL consume the owned artifact or API. It SHALL NOT recreate the same registry, parser, status mapping, or identity rule.

### Conformance

The typed suite catalog SHALL contain the ownership table, the generated artifact index
SHALL expose its compiled view, and CI SHALL fail if two machine artifacts declare the
same concern as authoritative.

## AC-G-02 — Normative version for every artifact
### Decision

Every normative prose document and every machine bundle uses a two-component public
version `major.minor`. Every governed source has two deliberately distinct identities:

- `canonical_digest` identifies its compiled semantic contract under a named,
  versioned projection profile;
- `source_digest` identifies its exact checked-in bytes.

Generated artifacts additionally carry their source identities and generator revision.
Patch-only editorial changes do not change semantic versions and do not necessarily
change `canonical_digest`, but they always change `source_digest`.

### Contract

Every artifact SHALL have this metadata in its own header or in the generated artifact
index:

```yaml
artifact_id: stable ASCII identifier
artifact_kind: normative-document | manifest | json-schema | json-lines | registry | yaml-contract | ebnf-grammar | protobuf-schema | bundle-manifest
version: "<major>.<minor>"
compatible_suite_major: 1
status: draft | released | deprecated
digest_projection: prose-utf8-v1 | json-jcs-v1 | yaml-ac-g-53-v1 | jsonl-jcs-v1 | proto-descriptor-v1 | ebnf-source-v1 | bundle-ac-g-07-v1
canonical_digest: "b3:<64 lowercase hex>"
source_digest: "b3:<64 lowercase hex>"
generator_revision: optional source-control digest
```

For machine artifacts, `canonical_digest` is BLAKE3-256 over the artifact's canonical
semantic projection with only that artifact's root/header `canonical_digest` and
`source_digest` identity fields omitted when present. Nested member, evidence, requirement, and
referenced-artifact digests remain semantic data. `prose-utf8-v1` is the explicit
exception: prose contains external sentinels rather than computed identity fields, so
the complete document bytes are hashed. `source_digest` is BLAKE3-256 over the complete
checked-in byte sequence and therefore SHALL live outside those bytes in the generated
artifact index or another detached provenance envelope. Machine sources may embed
`canonical_digest` because their named semantic projection structurally omits that
field; prose headers use the `external` sentinel because prose hashes complete bytes.
Generated outputs SHALL embed the applicable source artifact IDs, source
digests, canonical digests, projection IDs, and generator revision when doing so does
not create a self-reference. This preserves both semantic comparison and exact-byte
verification.

Every source descriptor selects exactly one closed projection profile:

| Projection ID | Canonical semantic projection |
|---|---|
| `prose-utf8-v1` | Complete checked-in UTF-8 document bytes with no BOM. Prose has no separate executable semantic parser in suite 1.x, so its canonical and source digest values are equal; they remain separately named metadata fields. |
| `json-jcs-v1` | Strict typed JSON root with only its own identity fields omitted, serialized as RFC 8785 bytes. |
| `yaml-ac-g-53-v1` | The pinned AC-G-05 YAML semantic projection with only its own root identity fields omitted, serialized as RFC 8785 bytes. |
| `jsonl-jcs-v1` | Strict typed metadata and records with only the metadata record's own identity fields omitted; each record is RFC 8785 JSON followed by one LF byte, preserving declared record order and all evidence digests. |
| `proto-descriptor-v1` | A normalized typed descriptor model derived from the suite's single compiled `FileDescriptorSet`; source comments and source locations are excluded. |
| `ebnf-source-v1` | The metadata header is parsed and validated, then omitted from the projection; canonical bytes are the exact grammar payload with line endings normalized to LF. No other whitespace or text rewriting occurs. |
| `bundle-ac-g-07-v1` | A bundle artifact profile with two projections: artifact canonical bytes omit only root `canonical_digest`/`source_digest`; bundle identity bytes additionally omit `bundle_digest` and `signature`. Both use the same typed, artifact-sorted RFC 8785 model. |

The projection ID is part of the artifact contract. Changing a profile or its emitted
bytes is a versioned contract change, never an incidental generator refactor. RFC 8785
encoding and BLAKE3 are owned by the suite-pinned libraries; repository code owns strict
ingress, typed projection, field omission, record framing, and resource bounds.

Version change rules:

- **major**: any change that can alter existing identity, fact meaning, required field meaning, protocol interpretation, negative-proof semantics, or previously valid request behavior;
- **minor**: additive kinds, fields, aliases, capabilities, profiles, protocol methods, or optional behavior that preserves all prior meanings;
- **digest only**: a change retained by one or both selected digest projections that
  does not change the public contract version, such as formatting, examples, comments,
  spelling, non-normative explanation, or generated ordering.

The last rule always changes `source_digest`; it changes `canonical_digest` exactly when
the selected profile retains the edited bytes. Thus prose editorial changes affect both
digests, while JSON/YAML source formatting outside the semantic projection affects only
`source_digest`. A semantic change changes both artifact identities. `bundle_digest` is
a third identity defined only by AC-G-07 and SHALL NOT be substituted for either
artifact identity.

Worked cases are normative:

| Source kind/change | Canonical projection consequence | Required identity change |
|---|---|---|
| Normative prose wording, formatting, or examples change | `prose-utf8-v1` retains exact document bytes | both artifact digests; no semantic-version change is required for an editorial-only change |
| JSON object members are reordered or insignificant whitespace changes | `json-jcs-v1` emits the same RFC 8785 bytes | `source_digest` only |
| A JSON field value changes | the strict typed model and RFC 8785 bytes change | both artifact digests |
| YAML indentation, comments, mapping order, or an equivalent accepted scalar spelling changes | `yaml-ac-g-53-v1` resolves to the same semantic JSON value | `source_digest` only |
| A YAML scalar or sequence order with declared semantic order changes | the semantic JSON projection changes | both artifact digests |
| JSONL record whitespace changes without changing a typed record | that record's JCS bytes and LF framing remain identical | `source_digest` only |
| JSONL record order changes | ordered framed record bytes change | both artifact digests |
| Protobuf comments or source locations change | normalized `proto-descriptor-v1` remains identical | `source_digest` only |
| A Protobuf field number, type, label, package, service, or method changes | normalized descriptor semantics change | both artifact digests |
| EBNF CRLF becomes LF | `ebnf-source-v1` normalizes to the same grammar payload | `source_digest` only |
| EBNF grammar-payload whitespace, comments, or production text changes | exact LF-normalized payload changes; payload presentation is contract data in `ebnf-source-v1` | both artifact digests |
| A bundle signature changes | artifact canonical bytes include it; the AC-G-07 bundle projection omits it | `canonical_digest` and `source_digest` change; `bundle_digest` does not |
| A required bundle artifact digest changes | both typed projections retain member digests | `canonical_digest`, `source_digest`, and `bundle_digest` change |

The following minimal profile vectors are normative. `<LF>` denotes byte `0a`; displayed
JSON projection bytes have no trailing LF unless one is shown. The `b3:` values are
BLAKE3-256 over exactly the displayed decoded bytes.

| Profile | Exact source bytes | Exact canonical projection bytes | Expected identities |
|---|---|---|---|
| `prose-utf8-v1` | `# Example<LF>` | `# Example<LF>` | `canonical_digest = source_digest = b3:a0f5e8f16750f4638b7d8ed55e400ea266f7d938bd19c42a8c626fb9025d97ec` |
| `json-jcs-v1` A | `{"b":2, "a":1}<LF>` | `{"a":1,"b":2}` | `source_digest = b3:b0ab3e1f1fdd9ae807e31fe856be07c7c7a704c35fc2a48d88525b4d8245900b`; `canonical_digest = b3:8e80439b77ac62d4194499edd46684c479da3aa1ac80dd5511468efae049166e` |
| `json-jcs-v1` B | `{<LF>  "a": 1,<LF>  "b": 2<LF>}<LF>` | `{"a":1,"b":2}` | `source_digest = b3:80a64d48801b5d77caef68c66e807fd2b34126bef0cdfcb6ff99a924fd2ccac0`; same canonical digest as A |
| `yaml-ac-g-53-v1` A | `a: 1<LF>b: 2<LF>` | `{"a":1,"b":2}` | `source_digest = b3:82d1b8f4ff3274aaa304cfa9c704fad602f51eb0a1928cf9659b5b31130686ab`; canonical digest equals the JSON A/B value |
| `yaml-ac-g-53-v1` B | `# editorial<LF>b: 2<LF>a: 1<LF>` | `{"a":1,"b":2}` | `source_digest = b3:dec00b1309f573aa2ce543d0223eea79d7614f8dcc4c6e44ba12e9197da8aee9`; same canonical digest as YAML A |
| `jsonl-jcs-v1` | `{"record_kind":"metadata","canonical_digest":"b3:1111111111111111111111111111111111111111111111111111111111111111","status":"draft"}<LF>{"z":2,"a":1}<LF>` | `{"record_kind":"metadata","status":"draft"}<LF>{"a":1,"z":2}<LF>` | `source_digest = b3:903036f189b73b62ed3326caf917f2fa9d8aa53b8f603449258f0ee6897880a3`; `canonical_digest = b3:0dd1ac3334522bae92947a15c7949f85b4c0874d6d05f0f631355b582bb99765` |
| `proto-descriptor-v1` | `// canonical_digest: b3:1111111111111111111111111111111111111111111111111111111111111111<LF>syntax = "proto3";<LF>package codefabric.example.v1;<LF>` | `{"files":[{"name":"example.proto","package":"codefabric.example.v1","syntax":"proto3"}]}` | `source_digest = b3:9742a6b59a39e895fae46965179e4c3325408684df6643cbdd35f1d56bbfd5a6`; `canonical_digest = b3:47e3dbed48619fab3f445c0c297847682a8df0471535b0648b5c2e91aacb2abc` |
| `ebnf-source-v1` | `(* artifact_id: codefabric.query.example *)<LF>(* canonical_digest: b3:1111111111111111111111111111111111111111111111111111111111111111 *)<LF>document = "";<LF>` | `document = "";<LF>` | `source_digest = b3:0ffbb2b15bd9a2b5aa0121a94256069c5f43ed58deb242e710b11403763f8574`; `canonical_digest = b3:b8f869e4a1e4509bf2a85fada5cdc3d72fe7108f173ef5eeaa4d5622079e1a78` |
| `bundle-ac-g-07-v1` | `{"artifacts":[{"artifact_id":"a","canonical_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required":true}],"bundle_digest":"b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bundle_kind":"schema","bundle_version":"1.0","canonical_digest":"b3:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","signature":"sig"}` | Artifact: `{"artifacts":[{"artifact_id":"a","canonical_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required":true}],"bundle_digest":"b3:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","bundle_kind":"schema","bundle_version":"1.0","signature":"sig"}`; bundle: `{"artifacts":[{"artifact_id":"a","canonical_digest":"b3:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","required":true}],"bundle_kind":"schema","bundle_version":"1.0"}` | `source_digest = b3:eabf79fdfa14d0c8692c7bb7b5e20425946e2c15dc9e4d5375403a894024c5ab`; `canonical_digest = b3:0697267a6199027a9c53bb702b9a156c499a54be714ca74312ace151e09e8583`; `bundle_digest = b3:1155e940f9114f59d0b003b1f6aedd325ff1d19defb896486f7378768d6776e5` |

For catalog-order normalization, input descriptor arrays with IDs `["b", "a"]` and
`["a", "b"]` both compile to
`[{"artifact_id":"a"},{"artifact_id":"b"}]` with digest
`b3:41c2c452f3762017987d6f8702c7bcd7dabfb3515e381cf4f4d534f74eaa5c08`.

The synchronized prose suite and this manifest are version `1.3` and compatible with suite major `1`. Independent registries and deployment profiles introduced by the architecture-completion pass begin at version `1.0` where stated and are pinned by exact digest in this 1.3 suite manifest.

Versions SHALL be compared as integer pairs, never floating-point numbers. Thus `1.10` is newer than `1.9`.

## AC-G-03 — Compatibility matrix and fail-fast negotiation
### Decision

Compatibility is negotiated independently by artifact family. No global “suite versions approximately match” rule is sufficient.

### Contract

| Artifact family | Compatibility rule |
|---|---|
| Ontology registry | Same major required. Consumer minor may be lower only if no returned kind exceeds its advertised supported-code set. |
| Schema bundle | Exact digest required for direct table read/write. Query service may serve older public response minors through an explicit compatibility encoder. |
| ID-preimage/type-algebra version | Exact version required inside one snapshot. Any change requires reindex. |
| Provider protocol | Same major; negotiated minor and feature bits. Unknown required feature fails. |
| Provider bundle | Snapshot pins exact bundle digest. A daemon may host multiple installed bundles but never mixes them within one context generation. |
| Derivation bundle | Exact digest required for materialized derived facts. Query-time operators may negotiate additive minor features. |
| Query language | Same major. Server accepts a declared supported minor interval and resolves with that exact phrase registry. |
| Canonical response schema | Same major; additive optional fields are permitted only when the consumer advertises the minor. |
| RPC | Same Protobuf package major; additive fields/methods are minor-compatible; required oneof variants require feature negotiation. |
| FastMCP public adapter schema | Adapter and host use the exact generated schema shipped by that adapter process. |
| Model packs | Pack schema major must match; pack semantic compatibility and target package version ranges must match the context. |
| Rust compiler extractor | Exact pinned nightly/toolchain and extractor adapter digest per Rust context. |

Handshake failure SHALL return one of:

```text
INCOMPATIBLE_MAJOR
UNSUPPORTED_MINOR
BUNDLE_DIGEST_MISMATCH
REQUIRED_FEATURE_UNSUPPORTED
SCHEMA_DIGEST_MISMATCH
TOOLCHAIN_MISMATCH
MODEL_PACK_INCOMPATIBLE
```

A mismatch SHALL fail before query acceptance or provider output activation. Compatibility SHALL never be inferred from filenames alone.

## AC-G-04 — Requirement IDs and end-to-end traceability
### Decision

All normative requirements receive stable IDs and a generated cross-layer trace graph.

### Contract

Requirement IDs use:

```text
CF-<owner>-<four digits>
```

where `<owner>` is one of `ARCH`, `ONT`, `GEN`, `FAB`, `LIFE`, `QUERY`, `SERVE`, `SEC`, or `TEST`. IDs are never reused.

A machine record SHALL contain:

```yaml
requirement_id: CF-QUERY-0042
source_artifact: code_property_graph_semantic_query_specification
source_section: "48.3"
normative_text: "Exact normalized normative statement"
normative_text_digest: "b3:..."
implements:
  - rust module or generated artifact identifier
traces_to:
  ontology_kinds: []
  capability_codes: []
  table_fields: []
  query_phrase_ids: []
  response_fields: []
  error_codes: []
trace_selectors:
  - all-query-phrase-ids
  - all-response-fields
verified_by:
  - test IDs
status: active | deprecated | superseded
```

`trace_selectors` is an optional closed set of catalog-backed expansions. The initial
selectors cover all ontology kinds, capability codes, table fields, query phrase IDs,
response fields, and error codes. The contract generator resolves these selectors from
the same typed registries and Contract IR used by their consumers, writes the expanded
`traces_to` edges, and derives `traceability.jsonl` from the requirement records. A
second hand-maintained trace inventory is prohibited. Reordering or extending a selected
registry therefore updates the trace graph programmatically. The generator likewise
resolves `source_artifact` against the typed catalog and refreshes the detached accepted
source digest and normalized normative-text digest; digest strings are not authored twice.

The trace graph SHALL support these mandatory paths:

```text
ontology kind
  → generation capability/provider observation
  → canonical schema/table mapping
  → projection/derivation mapping
  → semantic phrase and PlanSpec role
  → response field
  → conformance tests
```

CI SHALL fail for orphaned mandatory ontology kinds, schema columns without owning requirements, query phrases with no executable mapping, and requirements with no test or explicit `verification_deferred` record.

For this rule, an **executable phrase mapping** is a versioned declarative
mapping from a phrase-registry entry and its typed slots to one or more
`PlanSpec` node templates. The mapping must validate against the released
`PlanSpec` schema, name all required slot bindings, and carry positive and
negative fixtures. It does not require the Wave-15 runtime compiler to exist,
but a placeholder, prose-only note, `deferred-mapping` marker, or future owner
reference is not an executable mapping and SHALL fail the released profile.

## AC-G-05 — Required machine artifacts and repository layout
### Decision

The machine artifacts are first-class sources of truth and SHALL live in a dedicated
`contracts/` tree generated or validated from the normative specifications.
`contracts/manifests/suite-manifest.json` is the sole compiler bootstrap and typed
catalog authority for artifact ownership and derivation edges. It describes native
sources; it does not replace JSON Schema, Protobuf, YAML-registry, JSONL, or EBNF
semantics.

### Contract

The required layout is:

```text
contracts/
  manifests/
    suite-manifest.json
    fixture-oracles.json
    deployment-profile.schema.json
    requirements.jsonl
    traceability.jsonl
  registry/
    enum-registry.yaml
    flag-registry.yaml
    ontology-entity-registry.yaml
    ontology-relation-registry.yaml
    ontology-property-registry.yaml
    ontology-fact-registry.yaml
    unknown-registry.yaml
    projection-registry.yaml
    summary-registry.yaml
    capability-registry.yaml
    error-registry.yaml
    provider-registry.yaml
    derivation-registry.yaml
    state-machine-registry.yaml
    phrase-registry.yaml
    model-pack.schema.json
  identity/
    cbef-v1.yaml
    type-algebra-v1.yaml
    path-canonicalization-v1.yaml
  schema/
    schema-contract-ir.json
    analysis-context.schema.json
    serving-snapshot.schema.json
    public-snapshot-metadata.schema.json
    source-context.schema.json
    cpg-semantic-query-request.schema.json
    cpg-semantic-query-response.schema.json
    public-status.schema.json
    arrow-delta/table-specs.json
    operational-store.sql
  query/
    english-controlled-v1.ebnf
    planspec.schema.json
  rpc/
    cpg_query_service.proto
    provider_control.proto
    pyrefly_sidecar.proto
    rustc_extractor.proto
    feature-registry.yaml
  adapter/
    adapter-model-ir.json
    fastmcp-input.schema.json
    fastmcp-output.schema.json
    fastmcp-public-meta.schema.json
  bundles/
    ontology-bundle.json
    schema-bundle.json
    provider-bundle.json
    derivation-bundle.json
    query-language-bundle.json
    tool-contract-bundle.json
    toolchain-bundle.json
    model-pack-bundle.json
  deployment/
    local-workstation-v1.yaml
  faults/
    fault-point-registry.yaml
  comparison/
    comparison-ignore-registry.yaml
  security/
    security-corpus-manifest.yaml
  toolchain/
    toolchain-identity.json
```

Generated Rust and Python types SHALL be emitted under `generated/` and SHALL contain a
header naming the semantic identity of the primary source artifact. Exact source identity
remains detached in the generated artifact index, so an editorial-only source edit does
not churn generated source. Hand-edited generated files are prohibited.

The suite manifest SHALL use closed typed catalog schema version 2. The catalog has two
peer collections: `artifacts` and `derivations`. Artifact descriptors own governed source
authority and declare stable ID, native source path and kind, compatibility family,
projection profile, consumer domains, provenance obligations, resource budget profile,
zero or more closed `bundle_membership` values, and a typed
`semantic_projection_source`. That source is either `Native` or one exact
`DerivationOutput(OutputRef)`, where `OutputRef` contains a derivation ID and output path.
Artifact descriptors SHALL NOT own generated outputs or build-order dependencies.
When a generated public file is itself governed (for example a released JSON Schema),
its descriptor catalogs that file's identity and selects the exact owning
`DerivationOutput` as its semantic-projection source. The descriptor does not thereby
become an independent source or output owner. The output path MAY equal that descriptor's
authority path only for this self-owned generated-authority case; unrelated
authority/output path collisions remain invalid.

A closed `DerivationUnitDescriptor` owns each generation or compilation operation. It
contains `derivation_id`, a closed `derivation_kind`, typed sorted inputs, typed sorted
outputs, and a unit resource-budget profile. Producer/tool dispatch is derived from the
kind and is never an authored command or producer field. The initial kinds are:

```text
ArtifactIndex
CanonicalRegistrySet
ProtobufDescriptorAndPython
ProtobufRustFromDescriptor
AdapterModelCompilation
SchemaContractCompilation
```

A derivation input is exactly one of:

```text
Artifact(artifact_id, SourceBytes | CompiledSemantic)
Output(OutputRef)
AllCompiledArtifacts
```

`AllCompiledArtifacts` is an intrinsic accepted only by `ArtifactIndex`; it resolves to
the complete sorted compiled-artifact set and does not make the catalog a query language.
Arbitrary predicates, globs, platform conditionals, phase numbers, scheduling commands,
cache directives, environment mutation, and shell fragments are forbidden.

Each derivation output declares a globally unique safe repository-relative `path`, a
closed `output_kind`, a non-empty sorted set of `primary_artifact_ids`, a sorted consumer
set, and an optional output-specific resource budget. Output kinds may repeat; consumers
select plural outputs within a derivation or an exact output reference, never a global
singleton by kind. A kind defines valid input views, output kinds, and cardinality. Adding
a kind is an additive catalog/compiler contract change.

The compiler SHALL validate a graph over native-source, semantic-artifact, derivation,
and output nodes. It SHALL reject unknown fields or kinds; duplicate/unknown IDs; missing
artifact/output references; invalid input views; invalid kind/cardinality combinations;
empty or unknown primary sets; authority/output path conflicts; path escape; dependency
cycles; and output paths claimed by more than one derivation. Every released generated
surface SHALL be reachable from exactly one unit. Catalog record order has no semantic
effect: derivations sort by ID, inputs by typed stable reference, outputs by path, primary
artifact IDs by ID, and consumer/provenance sets by stable code before RFC 8785 object
canonicalization. Duplicate sort keys fail before projection.

`SchemaContractCompilation` consumes the closed schema Contract IR and owns the complete
Arrow/Delta `TableSpec` manifest, generated Rust registry, operational SQLite DDL, and
the eight public schema artifacts. It rejects duplicate table codes/names, unknown key or
dependency columns, illegal mutation/materialization combinations, opaque JSON/EAV table
columns, `Utf8View`, incomplete public-schema censuses, and any generated public schema
whose descriptor does not point back to that exact output. Adapter public schemas remain
owned by `AdapterModelCompilation` and are generated from its strict Pydantic models.

The suite manifest's first logical artifact descriptor SHALL describe the manifest
itself, including authority path, projection, owner, compatibility family, and budget.
It owns no generated output. The `ArtifactIndex` derivation separately consumes the
`AllCompiledArtifacts` intrinsic. This is ordinary typed self-description, not a
computed census or embedded digest.

Computed observations do not become a second self-referential catalog. A generated
artifact index SHALL contain peer artifact and derivation collections. Artifact records
own projection ID, `canonical_digest`, `source_digest`, authority, and provenance.
Derivation records contain resolved inputs and outputs, derived generator revision/tool
identity, and deterministic transitive lineage by artifact/output reference; they do not
create another source-digest authority. Consumers join the peer records to recover every
input identity. The index does not contain its own exact-byte digest. Output checksums are
reproduction evidence, never catalog authority. Rust and Python consumers SHALL read the
same canonical index resource; language source generation is reserved for statically
useful types and behavior.

The semantic request/response JSON Schemas SHALL be complete, closed at public boundaries, and capable of validating every normative fixture. The `.proto` files SHALL be compiled in both Rust and Python CI. Registry YAML is the human-reviewable source; canonical JSON derived from it is the fingerprinted machine form.

Registry canonicalization SHALL parse exactly one YAML 1.1 document with the
suite-pinned parser, reject duplicate mapping keys, reject tagged values and
merge keys unless a future registry schema explicitly assigns them semantics,
and project the resolved semantic model into the AC-G-53 JSON domain before
fingerprinting. Anchors and aliases are rejected by a bounded pre-parse YAML-subset
scanner because the suite-pinned parser cannot enforce an alias-expansion bound before
materialization. A future policy that accepts aliases requires a parser with a proven
pre-expansion limit and a versioned design correction. String-keyed mappings become JSON
objects; any logical non-string-keyed mapping becomes the AC-G-53 sorted
key/value-record array. Numeric values remain subject to the AC-G-53 finite and
interoperable-range rules. Generic serialization of a dynamic YAML value is not
a permitted projection because it does not define these boundary decisions.

All compiler ingress is staged and bounded: raw-byte and token/alias checks precede
full allocation where applicable; typed closed records precede cross-record validation;
emitters consume the typed Contract IR rather than rediscovering metadata lexically.
Every catalog descriptor names explicit byte, depth, collection, token/alias, graph,
and diagnostic budgets appropriate to its source kind. JSON Schema inputs SHALL pass
Draft 2020-12 metaschema validation with the suite-pinned validator before release.
The AC-G-08 YAML deployment instance SHALL additionally validate against
`deployment-profile.schema.json` with the exact suite-pinned safe YAML loader and the
same bounded source bytes; schema validity alone is not instance evidence.

The four `.proto` authorities SHALL be compiled by one exact `grpcio-tools` invocation
into Python bindings and one committed `FileDescriptorSet` with imports included and
source information excluded. Rust generation SHALL decode that same descriptor set and
use the pinned Prost/Tonic descriptor APIs; it SHALL NOT invoke a second `protoc`.
The Protobuf derivations SHALL receive resolved typed invocations rather than rescan the
catalog. `ProtobufDescriptorAndPython` consumes every governed root as `SourceBytes`, maps
every `FileDescriptorProto.name` back to that exact root set, and permits only declared
well-known transitive imports. `ProtobufRustFromDescriptor` consumes the resulting
descriptor-set `OutputRef` and emits Rust bindings through `compile_fds`.
`descriptor-census.json` is a generated review projection and never semantic authority.
Registry derivations emit the already-compiled canonical JSON bytes. Adapter generation
consumes the compiled adapter-IR identity and retains distinct Pydantic validation schema,
serialization schema, model-source, and fingerprint outputs; models and `TypeAdapter`s
remain compiled once rather than constructed on a request path.
Normative known-answer vectors remain independently reviewed oracles. Generator-derived,
property, and differential corpora provide broad coverage but SHALL NOT approve or
overwrite their own expected bytes or digests.

All eight built-in bundle manifests are compiler-owned views of the catalog's closed
`bundle_membership` relation. Generation sorts members by artifact ID and copies each
member's compiled canonical identity and catalog version; release verification rejects
missing, extra, duplicate, optional, or stale members. Hand-maintaining bundle arrays or
assigning membership in a second source is prohibited.

## AC-G-06 — Canonical enum and flag registry
### Decision

All categorical codes and bit positions are generated from one append-only registry. Individual Rust modules, Arrow schemas, Protobuf enums, Pydantic enums, and JSON Schemas SHALL not assign codes independently.

### Contract

Each enum record contains:

```yaml
domain: resolution_class
code_width: 16
version: "1.0"
values:
  - code: 10
    name: EXACT
    public_slug: exact
    meaning: One exact endpoint or value is established.
    aliases: []
    introduced: "1.0"
    deprecated: false
    replacement: null
    emitted: true
```

Rules:

- code `0` is reserved for invalid/uninitialized memory and is never persisted or emitted;
- codes are positive signed integers and append-only within each domain;
- existing codes, names, or meanings are never reassigned;
- aliases are accepted only at parsing boundaries and are never emitted;
- deprecated values remain decodable indefinitely;
- code widths are fixed per domain;
- registries use increments of ten for human readability, but insertion into unused gaps is prohibited after release; new values append after the highest released code;
- names use uppercase ASCII snake case; public slugs use lowercase kebab case.

Flag registries contain a 64-bit word and use:

```text
bits 0–31   language-neutral semantic flags
bits 32–47  language-profile flags
bits 48–55  generated/lowered representation flags
bits 56–62  reserved signed-extension flags
bit 63      reserved and SHALL remain zero
```

A flag meaning is immutable. Mutually exclusive flags SHALL identify an enum domain instead of consuming separate bits.

## AC-G-07 — Bundle manifests and fingerprints
### Decision

A bundle is an immutable ordered manifest over compatibility-sensitive artifacts. Every snapshot pins exact bundle IDs and digests.

### Contract

A bundle manifest contains:

```yaml
bundle_kind: ontology | schema | provider | derivation | query-language | tool-contract | toolchain | model-pack
bundle_version: "1.0"
bundle_major: 1
artifacts:
  - artifact_id: ...
    version: "1.0"
    canonical_digest: "b3:..."
    required: true
    feature_bits: []
compatibility:
  minimum_consumer_minor: 0
  maximum_consumer_minor: 0
created_by:
  generator_id: codefabric-contracts
  generator_version: "1.0"
bundle_digest: "b3:..."
signature: optional
```

Artifacts are sorted by `artifact_id`. The bundle digest is BLAKE3-256 over RFC-8785
canonical JSON with the bundle's root `canonical_digest`, `source_digest`,
`bundle_digest`, and `signature` fields omitted. Member and referenced-artifact digests
are retained. The bundle artifact's separate `canonical_digest` omits only its root
`canonical_digest` and `source_digest`, so it includes the computed `bundle_digest` and
any signature.

Built-in bundles are trusted by exact digest shipped with the binary. External model-pack bundles require an Ed25519 signature by a configured trust root. No other bundle accepts executable extensions.

`ServingSnapshot` stores both the human version and exact digest for every bundle. Query responses expose versions and abbreviated digests; diagnostics expose full digests only to authorized clients.

The `toolchain` bundle SHALL contain the closed `toolchain-identity` artifact and record
every exact build/runtime boundary identity, because the durable data plane and its
generated adapters are reproducible only against specific compilers and dependency
graphs even where fact meaning is unaffected:

```text
rust_version
datafusion_version
arrow_version
parquet_version
object_store_version
delta_rs_git_rev
deltalake_declared_version
cargo_lock_digest
toml_version
adapter: python, fastmcp, pydantic, pydantic-settings, grpcio, protobuf, jsonschema,
  PyYAML, uv-lock digest
protobuf: grpcio-tools, libprotoc, prost, tonic, toolchain-identity digest
rustc_extractor: toolchain, release, commit hash, identity digest
pyrefly: version, commit, locked source, identity digest
recorded_provider_pins: tree-sitter, tree-sitter-python, Ruff components, petgraph
```

`delta_rs_git_rev` is required because the pinned `deltalake` dependency is an untagged pre-release revision; a declared crate version alone does not identify it. Changing any of these values changes the toolchain bundle digest and therefore the canonical build/deployment bundle digest.

A toolchain-bundle change of this kind does **not** by itself require new ontology IDs, query phrase IDs, fact ID preimages, or schema bundle IDs. Those change only when an accompanying ontology, grammar, identity, or schema change is made.

## AC-G-08 — Default deployment profile manifest
### Decision

The mandatory 1.x profile is a local, single-user, no-network service on Linux or macOS. Windows is not a conforming 1.x runtime profile. Remote multi-user service is outside scope.

### Contract

The default profile is named `local-workstation-v1`:

```yaml
profile_id: local-workstation-v1
supported_platforms: [linux-x86_64, linux-aarch64, macos-aarch64, macos-x86_64]
windows_support: unsupported
network_listeners: disabled
workspace_registration: explicit-only
operational_store: sqlite-wal
fact_store: delta-local-filesystem
object_store: local-filesystem
hot_overlay_journal: disabled
source_blob_persistence: runtime-lease-only
result_artifact_ttl_seconds: 3600
source_result_artifact_ttl_seconds: 1800
coordinator_command_capacity: 64
maximum_concurrent_source_reads: 4
maximum_concurrent_gix_jobs: 2
source_image_limits:
  ordinary_maximum_bytes: 16777216
  explicit_maximum_bytes: 67108864
  stable_read_retry_count: 3
  orphan_grace_seconds: 300
  garbage_collection_batch_size: 256
inventory_limits:
  maximum_file_count: 1000000
  maximum_directory_count: 100000
  maximum_directory_depth: 128
  maximum_total_bytes_considered: 17179869184
  maximum_duration_ms: 300000
  maximum_entries_per_directory: 100000
default_query_freshness: REQUIRE_CURRENT_FOR_TARGETS
provider_sandbox: required-for-untrusted
follow_directory_symlinks: false
follow_internal_file_symlinks: false
index_external_dependency_bodies: false
semantic_query_language: english-controlled-v1
canonical_json: rfc8785-plus-codefabric-v1
```

Filesystem roots are selected according to:

| Platform | State root | Runtime root | Config root |
|---|---|---|---|
| Linux | `$XDG_STATE_HOME/codefabric` or `~/.local/state/codefabric` | `$XDG_RUNTIME_DIR/codefabric` | `$XDG_CONFIG_HOME/codefabric` or `~/.config/codefabric` |
| macOS | `~/Library/Application Support/CodeFabric` | a private short-path directory under `$TMPDIR` | `~/Library/Application Support/CodeFabric/config` |

All roots SHALL be user-owned and mode `0700`; private files SHALL be `0600`. The daemon SHALL refuse group/world-writable state, runtime, or configuration roots.

The profile manifest MAY override numeric quotas and enabled contexts, but a runtime SHALL expose its effective manifest digest through handshake and status.


---

# Part II — Permanent gap ownership and propagation

| Gap | Contract | Permanent owner |
|---|---|---|
| `G-01` | Master architecture, terminology, ownership, and precedence | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-02` | Normative version for every artifact | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-03` | Compatibility matrix and fail-fast negotiation | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-04` | Requirement IDs and end-to-end traceability | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-05` | Required machine artifacts and repository layout | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-06` | Canonical enum and flag registry | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-07` | Bundle manifests and fingerprints | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-08` | Default deployment profile manifest | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-09` | Generalized source-instance identity | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-10` | Daemon workspace registry and administrative lifecycle | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-11` | Root authorization, symlink boundaries, and secure path opening | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-12` | File identity across replacement, rename, and move | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-13` | Canonical ID preimage serialization | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-14` | Analysis-context discovery, identity, and selection | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-15` | Canonical type algebra | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-16` | External dependency identity and body policy | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-17` | Cross-language and FFI linking profile | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-18` | Path canonicalization, display, URI, and ordering | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-19` | Complete `ServingSnapshot` manifest schema | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-20` | Hot-overlay physical schemas and mutation representation | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-21` | Overlay semantics for owner-scoped, cross-owner, and global tables | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-22` | Deterministic overlay consolidation, merge, and durable rebase | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-23` | Snapshot leases, overlay lifetime, result retention, and Delta vacuum | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-24` | Formal freshness state machine and query barrier | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-25` | Machine-testable lifecycle transition tables | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-26` | Durable and active current-pointer transaction protocols | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-27` | Operational-state persistence | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-28` | Startup readiness, durable usability, and recovery generations | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-29` | Logical multi-file edit batches and publication barriers | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-30` | Pyrefly sidecar wire protocol | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-31` | rustc extractor protocol | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-32` | Common asynchronous provider execution interface | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-33` | Immutable source snapshot transport | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-34` | Build and project-configuration discovery | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-35` | Provider sandbox and trust model | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-36` | Provider capability granularity and aggregation | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-37` | Canonical reconciliation algorithm | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-38` | Declarative model-pack format, matching, and trust | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-39` | Derived-analysis precision profiles | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-40` | Generated, expanded, stub, shim, and lowered source capture | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-41` | Operational dependency graph schema and update algorithm | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-42` | Derivation materialization matrix | [Data-fabric specification 1.3](./present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md) |
| `G-43` | Unsupported, oversized, binary, generated, and vendored files | [Fact-generation specification 1.3](./present_state_cpg_fact_generation_specification_python_rust_v1.3.md) |
| `G-44` | Controlled semantic language grammar and phrase registry | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-45` | Deterministic semantic resolver architecture | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-46` | Typed internal `PlanSpec` | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-47` | Result-reference role type system and selector grammar | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-48` | Completeness and negative-proof algebra | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-49` | Entity matching, qualified-name parsing, grouping, and ranking | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-50` | Semantic source-boundary compiler | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-51` | Multi-context query semantics | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-52` | Query cost model, defaults, and hard limits | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-53` | Canonical JSON and checksum contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-54` | Canonical human-readable fact statements | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-55` | Source-context wire encoding | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-56` | Streaming, chunk interning, terminal completeness, and resumability | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-57` | Query plan cache contract | [Semantic-query specification 1.3](./code_property_graph_semantic_query_specification_v1.3.md) |
| `G-58` | Complete Protobuf service and query state machine | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-59` | Cancellation, acknowledgement, reconnect, and orphan handling | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-60` | Capability credential issuance, binding, rotation, and revocation | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-61` | Local IPC platform and security profile | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-62` | Daemon service, configuration, discovery, singleton, and upgrade behavior | [Lifecycle specification 1.3](./codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md) |
| `G-63` | Immutable result artifact store | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-64` | Delivery precedence, host limits, and automatic externalization | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-65` | Stable error registry and layer mappings | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-66` | Public status contract and redaction levels | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-67` | MCP resource read, range, expiry, and release semantics | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-68` | Multi-agent fairness, reservations, and starvation guarantees | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-69` | Fine-grained source disclosure and fact ACL policy | [FastMCP serving specification 1.3](./present_state_cpg_fastmcp_serving_specification_v1.3.md) |
| `G-70` | Machine ontology registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-71` | Property schema, value types, cardinality, null, and storage mapping | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-72` | Mandatory conformance profiles | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-73` | Unknown entities, unknown remainder, and explicit negative facts | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-74` | Graph projection registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-75` | Interprocedural summary semantics registry | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-76` | Static concurrency and happens-before semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-77` | Effect and resource model semantics | [Ontology specification 1.3](./code_property_graph_present_state_fact_ontology_specification_v1.3.md) |
| `G-78` | End-to-end golden corpus | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-79` | Canonical clean-rebuild comparator | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-80` | Cross-document and machine-contract conformance harness | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-81` | Deterministic fault-injection harness | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-82` | Performance acceptance profiles and degradation behavior | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-83` | Upgrade, migration, reindex, rollback, and acceptance suite | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |
| `G-84` | Security and adversarial-input test corpus | [Suite governance and release manifest 1.3](./codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md) |

The domain documents also contain explicit cross-layer integration tables for every secondary propagation target from the architecture-completion matrix. A gap has one full-text owner but may bind multiple consuming layers.

---

# Part III — Verification, performance, upgrades, and security acceptance

## AC-G-78 — End-to-end golden corpus
### Decision

CodeFabric SHALL maintain one small, versioned, intentionally adversarial reference workspace whose expected outputs cover every contract plane from source bytes through MCP delivery. The corpus is a normative executable specification, not merely a collection of examples.

### Contract

The canonical fixture root is:

```text
tests/golden/codefabric-golden-v1/
```

The root SHALL contain exactly these logical fixture groups:

```text
workspace/
  pyproject.toml
  python/                         Python package and tests
  Cargo.toml
  rust/                           Rust workspace and tests
  ffi/                            Rust/Python boundary fixture
  generated/                      generated-source fixtures
  malformed/                      deliberately invalid source
  .codefabric/
    workspace-registration.yaml
    contexts/
    model-packs/

scenarios/
  000_clean_bootstrap/
  010_python_local_edit/
  020_python_import_surface_change/
  030_python_parse_failure_and_recovery/
  040_rust_body_edit/
  050_rust_public_signature_change/
  060_rust_compile_failure_and_recovery/
  070_rename_and_case_change/
  080_multi_file_logical_save/
  090_context_change/
  100_generated_source_change/
  110_watcher_loss_reconciliation/
  120_hot_overlay_flush/
  130_daemon_restart/
  140_capability_withdrawal/
  150_source_acl_redaction/

expected/
  source_inventory/
  identities/
  provider_observations/
  canonical_tables/
  publications/
  serving_snapshots/
  queries/
  rpc/
  mcp/
  diagnostics/
  rebuild_comparison/
```

#### 78.1 Required Python coverage

The Python fixture SHALL include, at minimum:

- packages, namespace packages, relative and absolute imports;
- imported aliases, re-exports, `__all__`, star-import uncertainty, and unresolved modules;
- functions, methods, nested functions, lambdas, closures, captures, and comprehensions;
- positional-only, keyword-only, variadic, defaulted, and decorated callables;
- classes, inheritance, protocols, descriptors, properties, class methods, static methods, and overrides;
- declared, inferred, expected, narrowed, union, callable, generic, and unknown types;
- direct, bound-method, callable-object, dynamically resolved, possible, and unresolved calls;
- branches, loops, exceptions, `try`/`except`/`finally`, context managers, generators, and async functions;
- task creation, awaiting, lock/channel-like library models, and unknown concurrency barriers;
- mutation, rebinding, attribute writes, subscripts, aliases, and conservative unknown memory;
- comments, docstrings, directives, multiline strings, f-strings, continuations, and invalid syntax;
- a file that is syntactically recoverable by Tree-sitter while Ruff/Pyrefly semantics are unavailable;
- source paths containing spaces and Unicode display names.

#### 78.2 Required Rust coverage

The Rust fixture SHALL include, at minimum:

- a workspace with library, binary, test, example, and build-script targets;
- modules, `use` aliases, re-exports, visibility, traits, impls, associated items, and overrides;
- generics, trait bounds, associated types, const generics, and monomorphized instances;
- direct calls, method calls, closures, function pointers, trait dispatch, `dyn Trait`, and unresolved external calls;
- MIR branches, loops, normal and unwind edges, assertions, cleanup blocks, and unreachable blocks;
- moves, copies, shared and mutable borrows, reborrows, dereferences, drops, and drop glue;
- async functions/futures, spawn-like model-pack calls, channels, locks, atomics, and unknown happens-before barriers;
- macros, macro-expanded source correspondence, generated files, and compiler-generated shims;
- `unsafe`, raw pointers, inline assembly or an intentionally opaque stand-in, FFI declarations, and external effects;
- feature-gated and target-gated items represented by at least two indexed contexts;
- one compile failure that preserves current syntax while withdrawing affected compiler-semantic capability.

#### 78.3 Required cross-language coverage

The `ffi/` fixture SHALL include a PyO3- or C-ABI-shaped boundary with:

- one Rust-exported callable linked to a Python-visible external declaration;
- one Python call that resolves to the external boundary but not to an executable Rust body unless the linking profile permits it;
- one ambiguous or intentionally unlinked external symbol;
- explicit ABI, symbol, and evidence records;
- no inferred cross-language call edge without a registered linking rule.

#### 78.4 Exact fixture inputs

Every fixture file SHALL be stored as exact bytes. The corpus manifest SHALL record:

```yaml
corpus_id: codefabric-golden-v1
corpus_version: "1.0"
source_archive_digest: "b3:..."
workspace_registration_digest: "b3:..."
context_manifest_digests:
  - "b3:..."
provider_bundle_digests:
  - "b3:..."
model_pack_bundle_digest: "b3:..."
ontology_bundle_digest: "b3:..."
schema_bundle_digest: "b3:..."
derivation_bundle_digest: "b3:..."
query_bundle_digest: "b3:..."
```

Line-ending style, executable mode, symlink target bytes, and non-UTF-8 path fixtures SHALL be preserved by a tar-based fixture archive on platforms that support them. A platform that cannot materialize a required fixture SHALL mark the corresponding test `PLATFORM_NOT_APPLICABLE`; it SHALL NOT silently substitute a different path.

#### 78.5 Expected identity artifacts

The corpus SHALL pin exact expected values for:

- `workspace_id`, optional `repository_id`, and optional `worktree_id`;
- file IDs, context IDs, context-set IDs, owners, entities, properties, relations, unknowns, types, projections, summaries, publications, overlays, and snapshots;
- every CBEF-v1 identity preimage before hashing;
- full 256-bit collision-diagnostic digests and public 128-bit IDs;
- public textual encodings and binary round trips.

A change to a pinned ID is a breaking corpus change unless the test explicitly targets an ID-version migration.

#### 78.6 Expected provider and canonical outputs

For each clean-build fixture and incremental scenario, the corpus SHALL provide:

```text
provider_observations/<provider>/<context>/<owner>.arrow
canonical_tables/<table>.arrow
canonical_tables/<table>.canonical.jsonl
capability_records.arrow
provider_diagnostics.arrow
reconciliation_diagnostics.arrow
```

Provider-observation goldens MAY be refreshed when a provider upgrade intentionally changes evidence, but canonical-fact changes require an ontology/generation change record. Canonical Arrow files SHALL use the exact schema-bundle digest pinned by the corpus.

#### 78.7 Expected publication and snapshot artifacts

Each scenario SHALL pin:

- durable table-version map;
- publication manifest and manifest digest;
- consolidated overlay manifest and per-table overlay digests;
- `ServingSnapshotManifest` and snapshot ID;
- source generation, context generations, capability index, source-trust state, and freshness state;
- active-pointer generation before and after activation;
- lease and retention expectations where the scenario exercises them.

Operational timestamps and process-local IDs are represented by named placeholders in diagnostic fixtures and are excluded from identity expectations.

#### 78.8 Expected semantic-query and serving artifacts

The query fixture set SHALL exercise every request form, result-reference role, context-selection mode, freshness policy, completeness outcome, stream mode, artifact mode, error class, and source-ACL outcome.

For every request, expected artifacts SHALL include:

```text
request.canonical.json
request.digest.txt
resolved_request.json
planspec.canonical.json
planspec.digest.txt
response.canonical.json
response.digest.txt
stream_events.canonical.jsonl          when streamed
artifact_manifest.canonical.json       when externalized
mcp_tool_result.canonical.json
```

Canonical responses SHALL pin deterministic ordering, explicit unknowns, coverage, proof status, limit status, and public snapshot metadata. A result that depends on a potentially stale snapshot SHALL be a separate fixture and SHALL never share the expected response of a strict-current request.

#### 78.9 Scenario mechanics

Each scenario SHALL be declared as operations over the prior scenario:

```yaml
scenario_id: 040_rust_body_edit
base_scenario: 000_clean_bootstrap
operations:
  - replace_bytes:
      path_bytes_b64: ...
      expected_old_digest: b3:...
      new_bytes_file: patches/rust_body_after.rs
  - emit_watcher_hint:
      kind: modify
      path_bytes_b64: ...
  - wait_for:
      condition: semantic_current_for_targets
      owners: [...]
expected_terminal_state: READY
```

The harness SHALL apply operations, not copy a separately edited repository, so the exact transition is reproducible. Scenarios involving races SHALL use deterministic barriers and fault hooks rather than timing sleeps.

#### 78.10 Corpus versioning

The corpus uses semantic versioning independent of the suite:

- major: expected semantics or identity changes;
- minor: additive fixture/scenario/query coverage;
- digest only: comments or test-harness metadata with no expected-output change.

A released corpus version is immutable. Updates create a new directory or versioned archive and preserve the prior version for rollback tests.

### Conformance

The full local conformance command SHALL run the corpus from bootstrap through every scenario, compare every required artifact, and finish with the clean-rebuild equivalence test defined in `G-79`.

---

## AC-G-79 — Canonical clean-rebuild comparator
### Decision

Incremental correctness is defined as exact semantic equivalence between the current effective state and a clean rebuild produced from the same immutable source inventory, analysis-context set, provider/model/derivation bundles, and conformance profile. Equivalence is computed over canonicalized effective tables and snapshot metadata, not over Delta file layout or process-local operational state.

### Contract

#### 79.1 Comparator inputs

The comparator receives two `ComparisonInput` manifests:

```rust
struct ComparisonInput {
    workspace_id: WorkspaceId,
    serving_snapshot_id: ServingSnapshotId,
    source_inventory_digest: Digest32,
    context_set_id: ContextSetId,
    context_manifest_digests: Vec<Digest32>,
    ontology_bundle_digest: Digest32,
    schema_bundle_digest: Digest32,
    provider_bundle_digests: Vec<Digest32>,
    model_pack_bundle_digest: Digest32,
    derivation_bundle_digest: Digest32,
    conformance_profile_ids: Vec<ProfileId>,
}
```

The comparator SHALL reject the comparison as `COMPARISON_DOMAIN_MISMATCH` before reading fact tables when any comparison-domain field differs. `serving_snapshot_id` is a locator, not an equality field. Both inputs must independently verify as source-current for the same inventory; otherwise the comparison fails `SNAPSHOT_FRESHNESS_MISMATCH`.

#### 79.2 Effective-state materialization

For each snapshot, the comparator SHALL materialize:

```text
effective table = durable base table version
                minus overlay tombstones/replacements
                plus overlay rows
```

using the same overlay-composition semantics as query serving. It SHALL NOT compare only the durable base when an overlay is present.

The comparator SHALL include:

- canonical entity, relation, and property facts;
- typed extension tables;
- evidence records required by the selected conformance profile;
- capability/completeness records;
- unknown and explicit-negative facts;
- projection and summary materializations declared `required_for_comparison`;
- canonical schema and enum bundle IDs;
- source inventory identity and snapshot interpretation metadata after the operational-normalization rules below.

#### 79.3 Excluded operational fields

Only fields explicitly listed in the machine-readable `contracts/comparison/comparison-ignore-registry.yaml` may be ignored. The initial registry SHALL contain:

```text
operational wall-clock timestamps
monotonic clock values
process IDs and thread IDs
provider run IDs and operation IDs
RPC and MCP request IDs
temporary staging paths
Delta physical file names and file ordering
object-store ETags when not semantic content IDs
lease IDs and lease expiry times
metrics samples and trace/span IDs
retry counters that do not alter terminal status
publication IDs, snapshot IDs, and active/durable pointer generations used only as activation locators
source/update/overlay generation counters and admitted/reconciled event sequence numbers
provider/capability last-updated generations when their normalized state and content are otherwise equal
```

The registry SHALL NOT ignore:

```text
fact IDs
owner IDs
source spans
certainty or resolution
producer or derivation bundle identity
capability/completeness state
unknown remainder
source digests
analysis contexts
public status values
effective publication/table and overlay content digests
```

Adding an ignored field requires a reviewed requirement and a comparator-registry minor version increment.

#### 79.4 Canonical row representation

Every table SHALL define a canonical comparison projection and primary sort key in the schema registry. The comparison projection removes only registered operational columns and normalizes operational foreign keys such as provider-run IDs into their semantic evidence content. Rows that become exactly equal after this projection are coalesced; unequal rows are never coalesced merely because an ignored key was removed. Projected rows are ordered lexicographically by the canonical byte encoding of the comparison key; ties are ordered by the canonical encoding of the full projected row.

Canonical scalar encoding SHALL follow:

- integers: fixed-width big-endian two's-complement;
- booleans: one byte `00` or `01`;
- binary: unsigned length prefix followed by exact bytes;
- UTF-8: unsigned byte-length prefix followed by normalized stored UTF-8 bytes, with no additional Unicode normalization;
- lists: element count followed by canonical element encodings in stored semantic order;
- sets represented as lists: sorted and deduplicated before encoding according to their registry rule;
- maps: sorted by canonical key bytes;
- structs: fields in schema order with explicit null markers;
- floating point: IEEE bit pattern, with all NaN values normalized to the registry-declared canonical quiet-NaN pattern only where that field permits NaN;
- timestamps: signed microseconds from the Unix epoch, UTC;
- dictionary arrays: decoded to logical values before comparison;
- chunking: ignored; all chunks are compared as one logical column.

The per-row checksum is:

```text
row_digest = BLAKE3("cpg-comparison-row-v1" || table_code || canonical_row_bytes)
```

The table checksum is:

```text
table_digest = BLAKE3("cpg-comparison-table-v1" || table_code || ordered(row_digest...))
```

The effective-state checksum is:

```text
state_digest = BLAKE3(
  "cpg-comparison-state-v1" ||
  ordered(table_code || table_digest) ||
  source_inventory_digest || context_set_id || bundle_digests
)
```

#### 79.5 Equality result

Comparator outcomes are:

```text
EQUIVALENT
COMPARISON_DOMAIN_MISMATCH
SCHEMA_MISMATCH
TABLE_SET_MISMATCH
ROW_MISMATCH
CAPABILITY_MISMATCH
SNAPSHOT_METADATA_MISMATCH
ID_COLLISION_DETECTED
COMPARATOR_ERROR
```

There is no numeric tolerance for fact equality. Approximate analysis is encoded through certainty, precision profile, and candidate sets, all of which must compare exactly.

#### 79.6 Difference artifact

On failure, the comparator SHALL produce an immutable diagnostic artifact containing:

- comparison manifests and bundle digests;
- table-level row counts and checksums;
- missing/extra table names;
- first 100 missing, extra, or differing rows per table;
- full machine-readable row-difference stream;
- CBEF preimages for differing IDs when available;
- capability/completeness differences;
- suspected collision groups when two preimages map to one public ID;
- a deterministic command to reproduce the comparison.

The artifact SHALL obey the same source-disclosure ACL as query artifacts.

#### 79.7 Incremental-equivalence gate

Every golden scenario, fault-recovery scenario, provider upgrade, schema migration, and lifecycle optimization SHALL finish by:

1. freezing the current source inventory;
2. preserving the incrementally produced snapshot;
3. building a clean snapshot in an isolated namespace from the same inputs;
4. running the comparator;
5. releasing the isolated snapshot only after the comparison artifact is durable.

A failed comparison blocks release. The implementation SHALL NOT waive a mismatch because query examples happen to return equal results.

### Conformance

The golden corpus SHALL include one intentionally corrupted overlay and one intentionally changed ignored operational field. The comparator must reject the first and accept the second.

---

## AC-G-80 — Cross-document and machine-contract conformance harness
### Decision

CodeFabric SHALL provide one repository-owned conformance executable that regenerates and validates every normative machine artifact, compiles all normative code fragments designated executable, verifies cross-document traceability, and runs layer-to-layer round trips. Human review of six prose documents is not a substitute for this harness.

### Contract

The canonical entrypoint is:

```bash
codefabric-contracts verify --profile full
```

and the repository SHALL expose the equivalent stable command:

```bash
just contracts-check
```

#### 80.1 Required harness phases

The harness SHALL run these phases in order:

```text
1. source-document inventory and header validation
2. requirement-ID and traceability validation
3. registry generation and validation
4. JSON Schema generation and fixture validation
5. Protobuf generation, lint, and compatibility validation
6. Rust DTO/schema generation and compile tests
7. Python/Pydantic contract generation and compile/import tests
8. Arrow ↔ Delta ↔ DataFusion schema round trips
9. identity/type/path canonicalization vectors
10. ontology ↔ storage ↔ query mapping validation
11. error/status/RPC/MCP mapping validation
12. golden corpus and clean-rebuild comparator
13. generated-file cleanliness and manifest signing/digesting
```

A later phase SHALL NOT run when a prerequisite phase fails in a manner that could make its output misleading. The final report still lists all skipped phases and the blocking failure.

#### 80.2 Prose-document validation

Every normative Markdown document SHALL pass:

- required header-field validation;
- unique explicit anchors for numbered normative sections; repeated structural subheadings such as `Decision` and `Contract` are scoped by their parent section and are permitted;
- balanced code fences;
- unique requirement IDs;
- valid references to existing artifacts, registry keys, enum values, error codes, profiles, and permanent owning sections;
- no undefined normative term marked by the terminology linter;
- no duplicate normative ownership declaration;
- no unresolved placeholder markers or delegated design decisions in released sections;
- crosswalk completeness from every `G-01` through `G-84` decision to at least one permanent owner target.

Examples MAY be non-executable only when marked:

```text
codefabric-example: illustrative
```

All other Rust, Python, Proto, JSON, YAML, SQL, and shell blocks designated `codefabric-example: compile` or `validate` SHALL be extracted and tested.

#### 80.3 Registry generation

The harness SHALL generate from canonical registry sources:

- ontology codes and phrase aliases;
- enum/status/error/reason registries;
- property/cardinality schemas;
- projection and summary registries;
- conformance profiles;
- table schemas and comparison keys;
- ID domain tags and canonicalization vectors;
- query grammar, phrase registry, and `PlanSpec` schemas;
- RPC feature-bit and compatibility registries;
- public status and MCP output enums.

Generated Rust and Python code SHALL include the source registry digest. Hand-edited generated files fail the generated-file cleanliness gate.

#### 80.4 JSON Schema and canonical-JSON tests

The harness SHALL:

- validate all positive and negative request/response fixtures against the exact JSON Schemas;
- reject duplicate JSON object keys before schema validation;
- generate both Pydantic validation and serialization schemas and compare the public serialization schema to the packaged MCP schema;
- run RFC 8785 canonicalization vectors plus CodeFabric-specific byte/base64/ID vectors;
- prove canonical JSON round trips without changing the request or response digest.

#### 80.5 Protobuf tests

The harness SHALL use the pinned Protobuf toolchain and run:

```text
format check
lint
code generation for Rust and Python
wire round-trip fixtures
unknown-additive-field compatibility fixtures
oneof invariant tests
sequence/terminal-order tests
buf-style breaking-change comparison against the last released RPC major/minor baseline
```

Generated Rust and Python clients SHALL compile against the pinned dependency locks.

#### 80.6 Rust compilation matrix

The minimum Rust matrix is:

| Target | Toolchain | Required checks |
|---|---|---|
| daemon/data fabric | pinned stable workspace toolchain | fmt, clippy, check, nextest, docs snippets |
| rustc extractor | exact dated nightly + `rustc-dev` | check, unit tests, golden extraction |
| provider protocol fixtures | stable and extractor nightly as applicable | generated DTO compile, wire round trip |
| minimal feature build | pinned stable | no-default/minimal supported feature set |
| full local profile | pinned stable | all production features |

Duplicate Arrow, DataFusion, Parquet, object-store, Protobuf, or application-contract crate versions crossing public boundaries SHALL fail.

#### 80.7 Python compilation matrix

The adapter matrix SHALL include:

- exact supported Python minor versions declared by the adapter profile;
- import/compile of generated Pydantic and gRPC modules;
- `mypy` or Pyrefly static checking according to repository policy;
- Pydantic schema snapshots;
- in-memory FastMCP tests;
- STDIO subprocess protocol tests proving STDOUT contains MCP framing only;
- public-output redaction and subclass-field non-leakage tests.

#### 80.8 Arrow/Delta/DataFusion round trip

Every `TableSpec` SHALL pass:

```text
registry schema
  → Arrow RecordBatch
  → Delta table creation/write
  → reopen exact Delta version
  → DataFusion TableProvider
  → projected/filtered RecordBatch
  → canonical schema and row comparison
```

Overlay schemas SHALL additionally pass base-plus-overlay composition, owner replacement, tombstone, and schema-evolution fixtures.

#### 80.9 Traceability graph

The harness SHALL build a machine-readable graph with nodes for:

```text
requirements
document sections
registry entries
schemas and fields
protocol messages/methods
Rust/Python implementation modules
tests and golden fixtures
```

Every released `SHALL` requirement SHALL map to at least one verification node. Every generated public field, enum, RPC method, and ontology kind SHALL map back to an owning requirement.

#### 80.10 Reports and exit status

The harness emits:

```text
contract-report.json
contract-report.md
traceability.graph.json
artifact-manifest.json
```

Any failed required phase returns nonzero. Warnings are permitted only for explicitly experimental profiles and SHALL be counted and versioned in the report.

### Conformance

CI SHALL run a fast profile on every change and the full profile before merging changes that touch a normative document, registry, schema, protocol, provider adapter, identity code, lifecycle state machine, query compiler, or public adapter contract.

---

## AC-G-81 — Deterministic fault-injection harness
### Decision

Every boundary that can lose, delay, duplicate, reorder, partially persist, or expose work SHALL have a named deterministic fault point. Recovery behavior is tested by enabling one or a bounded combination of fault points; production builds compile the hooks to inert branch points with no externally controllable activation.

### Contract

#### 81.1 Fault-point registry

Fault points are declared in `contracts/faults/fault-point-registry.yaml`:

```yaml
- code: SOURCE_STABLE_READ_AFTER_STAT
  owner: lifecycle
  allowed_actions: [return_error, mutate_fixture, block_on_barrier, terminate_process]
  production_exposable: false
  expected_invariants:
    - no_stale_activation
    - old_snapshot_remains_valid
```

Codes are append-only. Every point specifies owner, legal actions, safety invariants, and scenarios using it.

#### 81.2 Activation model

Test activation SHALL be deterministic and scoped to one test run:

```rust
enum FaultTrigger {
    OnNthHit(u64),
    BeforeBarrier(BarrierId),
    AfterBarrier(BarrierId),
    ForMatchingWorkspace(WorkspaceId),
    ForMatchingOwner(OwnerId),
}

enum FaultAction {
    ReturnRegisteredError(ErrorCode),
    DelayUntil(BarrierId),
    DropMessage,
    DuplicateMessage,
    ReorderWithNext,
    CorruptTestPayload(CorruptionKind),
    CloseChannel,
    TerminateProcess(ExitMode),
}
```

Wall-clock sleeps SHALL NOT be the primary synchronization method. The harness uses explicit barriers, accepted-handle events, transaction hooks, and process supervisors.

#### 81.3 Required source/watcher faults

The suite SHALL cover:

- watcher event loss, duplication, reorder, overflow/rescan, and backend restart;
- path rename hints that do not match final filesystem state;
- file deletion between inventory and open;
- file mutation between first and second stable-read checks;
- permission change, symlink substitution, and path becoming a directory/device/FIFO;
- a multi-file batch partially arriving before its boundary marker;
- source changing while a query waits at a freshness barrier.

Expected behavior: authoritative reconciliation converges from current source; no raw watcher sequence mutates graph state directly.

#### 81.4 Required Git faults

The suite SHALL cover:

- HEAD/index/worktree state changing during inventory;
- interrupted merge/rebase/cherry-pick state;
- gix open/status/tree-diff failure;
- stale candidate set omitting a changed file;
- linked-worktree addition/removal;
- corrupt or locked Git metadata;
- inclusion-policy fingerprint changing during a wave.

Expected behavior: gix acceleration may degrade, but source-current claims require stable filesystem verification.

#### 81.5 Required provider faults

For Pyrefly and rustc protocols, test:

- process crash before `RunAccepted`;
- crash after acceptance but before any owner output;
- crash during owner output;
- duplicate owner sequence;
- missing owner terminal record;
- final manifest checksum mismatch;
- output for a stale source/context/Git generation;
- deadline expiry and cooperative cancellation;
- forced termination after cancellation grace;
- protocol-version mismatch;
- stdout/stderr flood beyond configured limits;
- malformed Arrow or DTO payload;
- sandbox denial of a required but unauthorized operation.

Expected behavior: partial output is never activated; affected capabilities become explicit; stale output is discarded.

#### 81.6 Required reconciliation/derivation faults

Test:

- conflicting authoritative observations;
- missing provider evidence dependency;
- identity collision diagnostic;
- derivation worker crash mid-owner;
- fixed-point iteration cap reached;
- unknown remainder introduced or removed;
- projection/model-pack digest change during a wave.

Expected behavior: canonical owner batches are atomic; uncertainty is not silently resolved; derived capabilities reflect failure precisely.

#### 81.7 Required durable-state faults

The suite SHALL inject:

- SQLite transaction failure, WAL fsync failure, and corruption detection;
- disk full before and after staging metadata write;
- Delta optimistic conflict on each table;
- object-store write failure, partial upload, checksum mismatch, and stale listing;
- process death after one or more table commits but before publication manifest completion;
- process death after publication completion but before durable pointer CAS;
- process death after durable pointer CAS but before active snapshot swap;
- crash during overlay flush or compaction;
- lease-table failure during vacuum eligibility calculation.

Expected behavior: no incomplete publication becomes current; orphan work is recoverable or abandoned; the prior current snapshot remains valid.

#### 81.8 Required query/RPC/artifact faults

Test:

- cancellation before acceptance, after acceptance, during resolution, planning, execution, response assembly, artifact write, and streaming;
- transport disconnect and reconnect with valid/invalid resume cursor;
- duplicated or skipped stream sequence numbers;
- terminal event followed by an illegal extra event;
- adapter process death with live query and with live artifact lease;
- artifact partial write, expiry during read, release/read race, and checksum mismatch;
- credential revocation during an accepted query;
- source ACL change between query acceptance and source-context materialization.

Expected behavior: terminal state is unambiguous; resources are reclaimed; revoked permissions prevent new disclosure and are rechecked at source materialization.

#### 81.9 Process-death matrix

The harness SHALL be able to issue `SIGKILL` or platform-equivalent termination to daemon, provider sidecar, rustc extractor, and FastMCP adapter at every named persistent boundary. Restart validation SHALL inspect only durable state, not hidden harness memory.

#### 81.10 Required invariant oracle

Every fault scenario evaluates at least:

```text
no stale fact presented as current
no cross-context fact merge
no partial owner or publication activation
prior valid snapshot remains queryable when policy permits
strict-current query never succeeds on stale data
unknown/capability gaps remain explicit
credentials and source ACLs do not leak
restart converges to a clean rebuild
all accepted work reaches one terminal state
no leaked lease, process, temp file, or artifact beyond policy
```

### Conformance

The release suite SHALL run all single-fault scenarios and a curated pairwise matrix of faults across different layers. Randomized chaos testing MAY supplement but SHALL NOT replace deterministic scenarios.

---

## AC-G-82 — Performance acceptance profiles and degradation behavior
### Decision

Performance acceptance is defined by reproducible hardware/workload profiles, measured percentiles, and correctness-preserving degradation rules. SLO failure does not authorize lower precision, stale-current claims, silent truncation, or unbounded resource use.

### Contract

#### 82.1 Benchmark profiles

The initial local profiles are:

| Profile | CPU | Memory | Storage | Intended corpus |
|---|---:|---:|---|---|
| `DEV_SMALL_V1` | 8 physical-equivalent cores / 16 hardware threads | 32 GiB | local NVMe, sustained random-read >= 250k IOPS | <= 100k source LOC, <= 5k source files, <= 2 Rust contexts, <= 2 Python contexts |
| `WORKSTATION_V1` | 16 physical-equivalent cores / 32 hardware threads | 128 GiB | local NVMe, sustained random-read >= 500k IOPS | <= 1M source LOC, <= 50k source files, <= 4 Rust contexts, <= 4 Python contexts |

A run SHALL record exact CPU model, core topology, RAM, storage, OS/kernel, filesystem, power mode, toolchain, bundle digests, and thermal-throttling indicators. Results from materially weaker or virtualized hardware are informative but not acceptance evidence unless a separate profile is registered.

#### 82.2 Reference workloads

Acceptance uses versioned workloads:

```text
W1  golden corpus
W2  synthetic 100k-LOC mixed Python/Rust repository
W3  synthetic 1M-LOC mixed Python/Rust repository
W4  query-heavy prebuilt graph with registered fact cardinalities
W5  update storm with repeated saves and bounded multi-file batches
W6  large artifact streaming workload
```

Workload generators SHALL be deterministic and publish source and expected cardinality digests.

#### 82.3 Measurement method

For each SLO:

- use release binaries with production feature flags;
- pin process CPU affinity where supported;
- disable unrelated scheduled jobs and record system load;
- run at least five warm-up iterations where meaningful;
- collect at least 30 measured iterations for sub-minute operations and 10 for longer operations;
- report p50, p95, p99 where sample count permits, maximum, mean, standard deviation, CPU time, peak RSS, bytes read/written, and queue depth;
- distinguish cold filesystem cache, warm filesystem cache, cold provider cache, and warm provider cache;
- verify output digests during the measured run;
- exclude setup only when the SLO definition explicitly excludes it.

A performance run is invalid when correctness comparison fails, throttling is observed without accounting, or more than 5% of samples are discarded.

#### 82.4 Initial latency SLOs

| Operation | `DEV_SMALL_V1` p95 | `WORKSTATION_V1` p95 | Measurement boundary |
|---|---:|---:|---|
| unchanged warm daemon start to `READY` | 2.0 s | 5.0 s | process start → readiness published |
| isolated source-byte and syntax snapshot activation | 200 ms | 300 ms | stable read complete → active snapshot swap |
| isolated Python semantic refresh | 1.5 s | 3.0 s | update accepted → semantic-current barrier released |
| isolated Rust body edit within one already-built crate | 8.0 s | 15.0 s | update accepted → Rust semantic-current barrier released |
| metadata/status query | 50 ms | 75 ms | daemon accepted → terminal response bytes available |
| simple entity/fact query, <= 10k returned facts | 150 ms | 250 ms | daemon accepted → terminal response bytes available |
| depth-5 bounded relationship traversal, <= 100k explored facts | 1.5 s | 3.0 s | daemon accepted → terminal response bytes available |
| accepted-query cancellation acknowledgement | 250 ms | 250 ms | cancel received → cancellation terminal event |
| cooperative provider cancellation | 2.0 s | 2.0 s | cancel sent → provider terminal acknowledgement |
| forced provider termination after grace | 10.0 s | 10.0 s | grace expiry → process confirmed dead |
| first 1 MiB artifact read | 100 ms | 150 ms | read accepted → first chunk available |

Rust semantic-refresh SLO applies only when rustc incremental artifacts are valid and no build-context invalidation requires a clean build. Clean-build latency is recorded and bounded by workload-specific budgets rather than this interactive SLO.

#### 82.5 Throughput and resource SLOs

| Metric | `DEV_SMALL_V1` | `WORKSTATION_V1` |
|---|---:|---:|
| sustained source-change ingestion without event loss | 200 path hints/s for 60 s | 1,000 path hints/s for 60 s |
| canonical response stream throughput after first byte | >= 75 MiB/s | >= 150 MiB/s |
| warm steady-state daemon peak RSS on reference corpus | <= 8 GiB | <= 24 GiB |
| durable publication write amplification, excluding Delta maintenance | <= 8x changed canonical bytes | <= 8x changed canonical bytes |
| artifact store quota accounting error | 0 bytes unaccounted after GC cycle | 0 bytes unaccounted after GC cycle |
| update-query reserved headroom | >= 20% executor capacity remains available to updates | same |

Provider child processes are reported separately and included in whole-system peak memory.

#### 82.6 Queue and overload limits

Every bounded queue SHALL publish capacity, current depth, oldest age, rejection count, and superseded-work count. At 80% capacity the system enters `PRESSURE_HIGH`; at 100% it SHALL apply the owner-specific policy:

- watcher hints: collapse to dirty-set plus reconciliation requirement;
- supersedable provider work: cancel/discard older generations;
- non-supersedable publication work: backpressure producers;
- interactive queries: preserve reserved slots, then reject with `RESOURCE_EXHAUSTED_RETRYABLE` rather than queue without bound;
- artifacts: externalize or reject according to delivery limits.

#### 82.7 Correctness-preserving degradation

Under saturation, CodeFabric MAY:

- coalesce changes;
- reduce parallelism;
- defer non-required durable maintenance;
- externalize results;
- reject new expensive queries;
- return best-available snapshots only when explicitly requested;
- withdraw a semantic capability when its provider cannot complete.

It SHALL NOT:

- silently change precision profile;
- drop unknown candidates;
- skip current-source verification;
- present a stale capability as current;
- truncate without an explicit limit status;
- disable source ACL checks;
- abandon accepted work without a terminal event.

#### 82.8 Regression thresholds

A change fails the performance gate when, on the same profile and workload:

- any hard SLO is exceeded;
- p95 latency regresses by more than 15% and at least 20 ms for interactive operations;
- throughput falls by more than 15%;
- peak RSS rises by more than 15% or 512 MiB, whichever is larger;
- write amplification rises by more than 20%;
- correctness or clean-rebuild equivalence fails.

An accepted intentional regression requires a new profile/version or an explicit reviewed SLO amendment, not a test waiver.

### Conformance

The release pipeline SHALL publish a signed/digested performance report and retain at least the last 20 comparable runs for trend analysis.

---

## AC-G-83 — Upgrade, migration, reindex, rollback, and acceptance suite
### Decision

Upgrades are classified by the state they invalidate. Breaking changes are deployed side by side with an off-path rebuild and atomic activation. Downgrades never write to newer operational or data schemas. A hot overlay is reused only when every identity, schema, and derivation contract it depends on remains exactly compatible.

### Contract

#### 83.1 Upgrade classes

| Class | Examples | Required action |
|---|---|---|
| `ADAPTER_ONLY` | FastMCP descriptions, additive public fields, adapter bug fix | restart adapters after handshake compatibility check |
| `RPC_ADDITIVE` | additive optional Proto fields/methods | rolling daemon/adapter upgrade within same major |
| `QUERY_ADDITIVE` | new phrase aliases or optional PlanSpec operators | deploy query bundle; no reindex if existing meanings unchanged |
| `PROVIDER_PATCH` | provider bug fix with identical normalized output contract | differential golden test; regenerate affected owners only if output digest changes |
| `DERIVATION_CHANGE` | algorithm/precision/model-pack change | recompute affected derived families; base facts reusable |
| `SCHEMA_ADDITIVE` | nullable/optional table field or new table | migrate/create off path; old readers only if compatibility encoder exists |
| `ONTOLOGY_ADDITIVE` | new kinds/properties with stable prior codes | generate new facts for affected providers/owners; prior facts remain valid |
| `CONTEXT_CHANGE` | Python resolver, Cargo features, target triple, build script output | invalidate affected context partition and dependents |
| `IDENTITY_BREAKING` | CBEF, file-ID, type-algebra, workspace-identity rule | full reindex into a new identity namespace |
| `SEMANTIC_BREAKING` | changed meaning/cardinality/certainty/completeness/unknown rules | full or registry-declared scoped reindex; new suite major when prior facts change meaning |
| `RPC_MAJOR` | incompatible wire state machine | side-by-side endpoint/package; coordinated adapter cutover |
| `OPERATIONAL_DB_BREAKING` | incompatible SQLite state schema | stop-the-world migration with backup or side-by-side operational DB |

#### 83.2 Mandatory reindex triggers

A full workspace/context reindex is mandatory when any of these change incompatibly:

```text
workspace or file identity rules
CBEF version or domain separation
type algebra canonicalization
path canonicalization used in IDs
ontology kind meaning or code reassignment
property cardinality/null semantics
canonical reconciliation authority
provider normalization contract
schema field meaning or primary key
analysis-context fingerprint algorithm
unknown or completeness semantics
source-span coordinate semantics
```

A derived-only rebuild is sufficient when the base canonical facts remain valid and only projection, summary, model-pack, or derivation-bundle semantics change.

#### 83.3 Pre-upgrade acceptance

Before deployment, the candidate SHALL pass:

- full cross-document conformance harness;
- golden corpus under old and new binaries where wire/state compatibility is claimed;
- provider differential report;
- schema/registry/Proto breaking-change checks;
- migration dry run on a copied operational DB and copied publication namespace;
- clean-rebuild comparator;
- fault tests covering crash at each migration boundary;
- security and performance acceptance profiles.

The upgrade manifest records:

```yaml
from_versions: {...}
to_versions: {...}
upgrade_class: ...
required_reindex_scope: ...
overlay_policy: flush | discard_and_rebuild | compatible_replay
rollback_supported_until: ...
old_namespace_retention: ...
```

#### 83.4 Deployment choreography

The choreography has a **prestage phase** and a short **cutover phase**.

For upgrades requiring reindex or expensive migration:

1. install the candidate binary/bundles under a separate runtime/state root with no public query endpoint and no authority over the old namespace;
2. the old daemon captures an immutable handoff source/context/bundle manifest and begins a bounded upgrade-delta journal of subsequent accepted source generations;
3. the candidate performs its migration/reindex into a separate operational database and publication namespace while the old daemon continues normal service;
4. the candidate runs schema, golden, security, and clean-rebuild verification against the handoff generation;
5. if shadow verification fails, discard the candidate namespace without disturbing service.

Cutover then proceeds:

1. reject new workspace-registration mutations;
2. drain or cancel accepted queries according to shutdown policy and stop accepting new queries;
3. stop new update waves at a coherent wave boundary while retaining watcher admission;
4. capture stable current source, close the upgrade-delta journal, and reconcile every generation/path changed since the handoff into the candidate namespace;
5. flush the old hot overlay only when exact compatibility permits; otherwise preserve the old snapshot and let the candidate reconstruct from current source;
6. checkpoint and back up the old SQLite operational database and preserve the old durable publication/table versions;
7. run final current-generation verification and clean-rebuild comparison in the candidate;
8. atomically switch private daemon discovery and the candidate's active snapshot/pointer;
9. start candidate watchers/admission at the closed journal sequence, replay any events admitted during the final swap, and publish readiness only after the barrier is satisfied;
10. restart/reconnect adapters and complete handshake;
11. retain old binary, DB backup, publication namespace, and bundle set through the rollback window.

An additive upgrade that requires no reindex may skip the shadow rebuild and use the cutover phase directly. The candidate shadow process is never a concurrent writer to the old operational DB, old Delta namespace, or active pointer. For additive RPC upgrades, old and new adapters MAY overlap only when the daemon advertises both supported minor intervals.

#### 83.5 Hot-overlay policy

Overlay reuse requires exact equality of:

```text
schema bundle digest
ID/CBEF version
type-algebra version
ontology bundle digest
provider-normalization bundle digests
derivation bundle digest for overlay-resident derived tables
analysis-context manifests
base-publication identity
```

If any differ, the overlay SHALL be discarded and reconstructed from authoritative source after the prior snapshot is preserved. An overlay SHALL never be migrated by best-effort field mapping.

#### 83.6 Provider and toolchain upgrades

A Pyrefly, Ruff, Tree-sitter grammar, rustc nightly, MIR adapter, or model-pack upgrade SHALL produce:

- provider-observation differential by owner/fact family;
- canonical-fact differential after reconciliation;
- capability/completeness differential;
- query-response differential over the golden corpus;
- performance and memory differential;
- list of intended semantic changes mapped to release requirements.

The Rust extractor pins the exact nightly date and compiler build identity. An unpinned “latest nightly” is not an upgrade target.

A **storage-substrate upgrade** — a change to the pinned delta-rs revision, the DataFusion family, the Arrow/Parquet family, `object_store`, or the Rust floor — is a distinct class. It changes no fact meaning, so the fact-differential obligations above are expected to come back empty, and that expectation is itself the test. Such an upgrade SHALL additionally satisfy the delta-rs upgrade gate in Data Fabric §112.6, which covers snapshot and cache behavior, lazy/eager equivalence, statistics policy, optimize correctness, protocol-feature compatibility, and activation/query performance. It SHALL also restamp the toolchain bundle identity required by `G-07`.

Because the pinned `deltalake` dependency is an untagged pre-release revision, the upgrade target is an exact commit SHA. A branch name, a declared crate version, or “current `main`” is not an upgrade target.

#### 83.7 Rollback contract

Rollback SHALL:

- stop the candidate before opening old state for write;
- restore the old operational DB backup or reopen the preserved old DB, never run a down-migration in place unless specifically verified;
- restore daemon discovery to the old endpoint/binary;
- reactivate the old preserved snapshot/pointer using its own bundle set;
- discard candidate-only overlay and staging data after safety checks;
- reconnect compatible adapters or restart old adapters;
- run readiness, golden smoke queries, and source-inventory reconciliation.

The old binary SHALL NOT write to a newer schema, ontology, operational DB, or overlay. Rollback uses preserved old namespaces.

The default rollback window is 7 days for local releases and SHALL be extended when Delta vacuum retention or artifact leases require it.

#### 83.8 Post-upgrade acceptance

After cutover, the system SHALL prove:

```text
readiness and compatibility handshake succeed
source inventory matches authoritative filesystem
strict-current query succeeds for required profiles
old accepted IDs behave according to declared compatibility
no stale or orphan provider run can activate
clean rebuild equals migrated/incremental state
resource/lease/credential registries are consistent
performance smoke SLOs pass
rollback assets are intact until the window expires
```

### Conformance

The acceptance suite SHALL exercise at least one upgrade and rollback for every upgrade class, including an intentionally incompatible identity change that forces a new namespace and an intentionally additive RPC change that permits rolling adapter compatibility.

---

## AC-G-84 — Security and adversarial-input test corpus
### Decision

Security acceptance is driven by a versioned corpus spanning filesystem paths, source bytes, build execution, semantic inputs, RPC framing, credentials, artifacts, and disclosure policy. Every fixture specifies the expected denial, degradation, or bounded-success result and proves absence of cross-workspace, cross-agent, or unauthorized-source leakage.

### Contract

The canonical root is:

```text
tests/security/codefabric-security-v1/
```

Every case SHALL include:

```yaml
case_id: stable identifier
threat_class: ...
required_platforms: [...]
inputs: ...
operation: ...
expected_status_or_error: ...
expected_public_fields: ...
forbidden_observations: ...
resource_bounds: ...
cleanup_assertions: ...
```

#### 84.1 Path and filesystem corpus

The corpus SHALL include:

- `..` traversal, repeated separators, absolute paths, drive/UNC-like paths, NUL attempts, and overlong components;
- raw non-UTF-8 path bytes and invalid display decoding;
- Unicode normalization lookalikes and case-fold collisions;
- path names equal under the configured comparison key but different in raw bytes;
- symlinks inside the root, symlinks escaping the root, symlink loops, and terminal-component substitution races;
- mount-point/bind-mount escape where the test platform permits it;
- hard links to files outside the registered root where detectable;
- sockets, FIFOs, block/character devices, proc/sys pseudo files, and sparse files;
- permission-denied files and directories;
- rename-over-target and case-only rename races;
- nested Git repositories and linked-worktree metadata pointing outside authorized roots.

Expected behavior: root confinement uses descriptor- or handle-relative verification; unsupported file types are excluded with stable diagnostics; display paths never become authority.

#### 84.2 Source-byte and parser corpus

The corpus SHALL include:

- binary files misnamed with source extensions;
- invalid UTF-8 Python source, valid encoding-cookie source, conflicting BOM/cookie declarations, and undecodable source context;
- huge lines, huge tokens, deeply nested syntax, pathological comments/strings, and parser/query match explosion inputs;
- files at soft and hard size limits, sparse files reporting huge logical size, and files growing during read;
- adversarial macro/token expansion and generated-source mapping;
- malformed but recoverable Python/Rust source;
- source containing secrets used to verify ACL and diagnostic redaction.

Expected behavior: bounded parsing/extraction, explicit capability status, no unbounded allocation, and bytes-preserving source representation.

#### 84.3 Build-script, proc-macro, and compiler sandbox corpus

Malicious fixtures SHALL attempt to:

- open SSH keys, cloud credentials, environment secrets, and unrelated workspace files;
- write outside the allowed scratch/output roots;
- open network sockets or contact loopback services;
- spawn child-process trees, fork repeatedly, or escape process groups;
- consume CPU, memory, file descriptors, output bytes, and wall-clock time beyond limits;
- create symlinks/reparse points to redirect generated output;
- invoke external filters, hooks, credential helpers, or shell commands not allowed by profile;
- persist background processes after the provider job terminates.

Expected behavior: sandbox denial or bounded termination, stable diagnostic, process-tree cleanup, and no secret content in logs or provider output.

#### 84.4 Semantic request and JSON corpus

The corpus SHALL include:

- duplicate object keys;
- invalid UTF-8, invalid escapes, invalid base64, overlong IDs, and noncanonical ID text;
- excessive nesting, object members, strings, arrays, numeric magnitude, and query-block count;
- cyclic, forward-invalid, cross-context-invalid, and unauthorized result references;
- phrase ambiguity attacks and aliases that resemble ontology IDs;
- source-boundary expressions attempting path traversal or ACL widening;
- service-limit bypass through nested summaries, paths, or repeated aliases;
- NaN/infinity and negative-zero where JSON/value schemas prohibit them;
- canonicalization collisions or alternate number/string spellings.

Expected behavior: predeclared validation/error codes, no parser recursion crash, deterministic canonicalization, and no partial query acceptance.

#### 84.5 RPC and stream corpus

The corpus SHALL include:

- oversized frames and chunks;
- truncated/malformed Protobuf, illegal enum values, unset/contradictory oneofs, and unknown required feature bits;
- duplicate, skipped, reordered, and post-terminal sequence numbers;
- invalid compression metadata, decompression bombs, checksum mismatch, and claimed-size mismatch;
- resume cursor from another query/agent/workspace or beyond acknowledged sequence;
- cancellation/release for unknown, terminal, or foreign handles;
- high-rate handshake/query/cancel attempts and connection churn;
- peer-credential mismatch and unauthorized loopback/UDS access.

Expected behavior: bounded decoding, connection-scoped or request-scoped rejection as registered, no daemon crash, and no handle existence leak across principals.

#### 84.6 Credential corpus

Test:

- expired, not-yet-valid, revoked, malformed, wrong-workspace, wrong-agent, wrong-launcher, and wrong-audience tokens;
- copied token from another UID/process where peer binding applies;
- replay after nonce/session consumption for one-time launcher credentials;
- token rotation while adapters are connected;
- insecure token-file permissions and symlink substitution;
- token content accidentally included in logs, traces, crash reports, errors, status, artifacts, or child-process environment.

Expected behavior: fail closed, stable public authentication error, audit event without secret material, and immediate enforcement for new operations after revocation.

#### 84.7 Artifact corpus

Test:

- URI/path traversal, guessed artifact IDs, cross-agent/workspace reads, and unauthorized subresource derivation;
- invalid ranges, integer overflow, overlapping concurrent reads, and excessive concurrent range requests;
- compressed/decompression bomb artifacts;
- partial file, checksum mismatch, metadata/content mismatch, and object replacement;
- expiry during read, release/read race, daemon restart, and GC race;
- artifact containing restricted source facts while the reader has fact-only but not source permission.

Expected behavior: opaque capability-checked artifact identity, bounded range semantics, checksum verification, and source ACL enforcement at read time.

#### 84.8 Source-disclosure and side-channel corpus

The suite SHALL prove that a denied path or fact does not leak through:

```text
source-context bytes or decoded text
entity display names derived from restricted text
statement summaries
error messages and validation candidates
query plan/explain output
provider diagnostics
status inventories
artifact metadata and lengths where length itself is restricted
cache keys or result-reference errors
timing differences beyond documented unavoidable class-level variance
```

Redaction SHALL preserve response structural validity and explicitly mark `REDACTED_BY_POLICY`; it SHALL NOT replace denied text with misleading empty source.

#### 84.9 Fuzzing and property tests

Continuous fuzz targets SHALL include:

- path canonicalizer and root-confinement resolver;
- CBEF encoder/decoder and public ID parser;
- type-algebra parser/canonicalizer;
- JSON canonicalizer and semantic request parser;
- query grammar/resolver and result-reference compiler;
- Protobuf event decoder and stream state machine;
- Arrow batch/schema validator;
- overlay merge/composition;
- artifact URI/range parser;
- public redaction mapper.

Fuzzing runs with resource limits and a pinned seed corpus. Every fixed crash or security bug adds a minimized regression fixture.

#### 84.10 Security acceptance outcomes

Each case terminates as one of:

```text
ACCEPTED_BOUNDED
REJECTED_AUTHENTICATION
REJECTED_AUTHORIZATION
REJECTED_VALIDATION
REJECTED_RESOURCE_LIMIT
EXCLUDED_UNSUPPORTED_INPUT
SANDBOX_DENIED
PROVIDER_TERMINATED_BOUNDED
DEGRADED_WITH_EXPLICIT_CAPABILITY_GAP
```

Unexpected process exit, hang, unbounded growth, secret leakage, cross-principal disclosure, stale-current success, or partial publication is always a failure.

### Conformance

The security corpus SHALL run in CI for platform-independent cases, in platform-specific jobs for Linux/macOS filesystem and sandbox cases, and in a scheduled extended fuzz/chaos profile. Security-case manifests and minimized inputs are immutable once released.

---

---

# Part IV — Required generated artifacts

The design is not implementation-closed until these artifacts exist and pass the conformance harness:

```text
contracts/manifests/suite-manifest.json
contracts/manifests/fixture-oracles.json
contracts/manifests/deployment-profile.schema.json
contracts/manifests/requirements.jsonl
contracts/manifests/traceability.jsonl
contracts/registry/enum-registry.yaml
contracts/registry/flag-registry.yaml
contracts/registry/ontology-entity-registry.yaml
contracts/registry/ontology-relation-registry.yaml
contracts/registry/ontology-property-registry.yaml
contracts/registry/ontology-fact-registry.yaml
contracts/registry/unknown-registry.yaml
contracts/registry/projection-registry.yaml
contracts/registry/summary-registry.yaml
contracts/registry/capability-registry.yaml
contracts/registry/error-registry.yaml
contracts/registry/provider-registry.yaml
contracts/registry/derivation-registry.yaml
contracts/registry/state-machine-registry.yaml
contracts/registry/phrase-registry.yaml
contracts/registry/model-pack.schema.json
contracts/identity/cbef-v1.yaml
contracts/identity/type-algebra-v1.yaml
contracts/identity/path-canonicalization-v1.yaml
contracts/schema/analysis-context.schema.json
contracts/schema/serving-snapshot.schema.json
contracts/schema/public-snapshot-metadata.schema.json
contracts/schema/source-context.schema.json
contracts/schema/cpg-semantic-query-request.schema.json
contracts/schema/cpg-semantic-query-response.schema.json
contracts/schema/public-status.schema.json
contracts/schema/arrow-delta/*.yaml
contracts/query/english-controlled-v1.ebnf
contracts/query/planspec.schema.json
contracts/rpc/cpg_query_service.proto
contracts/rpc/provider_control.proto
contracts/rpc/pyrefly_sidecar.proto
contracts/rpc/rustc_extractor.proto
contracts/rpc/feature-registry.yaml
contracts/adapter/adapter-model-ir.json
contracts/bundles/*.json
contracts/deployment/local-workstation-v1.yaml
contracts/faults/fault-point-registry.yaml
contracts/comparison/comparison-ignore-registry.yaml
contracts/security/security-corpus-manifest.yaml
```

Generated code is a build output of these sources; it is not an independent authority.
The adapter model compiler emits its statically typed Pydantic source plus canonical
validation/serialization schema and fingerprint manifests under
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/`; these are catalog-owned package
outputs of `contracts/adapter/adapter-model-ir.json`, never sibling schema authorities.

---

# Part V — Implementation-readiness gates

Broad production implementation SHALL proceed through these gates:

### Gate A — Contract generation

All registries, schemas, protocol definitions, identity vectors, manifests, and traceability files exist and pass `codefabric-contracts verify` without released-profile warnings.

Initial numeric allocations, field tags, protocol package/message names, and
registry record schemas are design-contract decisions. They MAY be instantiated
in the machine-contract sources where the permanent owner delegates authority
to that source, but they SHALL be reviewed and accepted by the owning artifact
authority before generated application code consumes them. Deterministic
allocation rules fixed by a permanent owner are not implementation-time
invention.

### Gate B — Vertical golden slice

One Python owner, one Rust MIR owner, one unknown fact, one property fact, one relation fact, one derived projection, one hot-overlay update, one durable publication, one semantic query, one streamed result, and one artifact result pass end to end.

### Gate C — Continuous-update equivalence

Every golden edit scenario converges incrementally and compares equal to a clean rebuild.

### Gate D — Failure and recovery

All blocker/high fault scenarios prove the invariants in `G-81`, including process death at every persistence boundary.

### Gate E — Security and authorization

The local IPC, credential, sandbox, path-confinement, source-ACL, artifact, malformed-input, and cross-agent tests pass.

### Gate F — Performance

The selected hardware/workload profile meets all hard SLOs and publishes a reproducible report.

### Gate G — Upgrade and rollback

At least one additive and one breaking synthetic upgrade complete, compare correctly, and roll back within the preserved rollback window.

An LLM programming agent SHALL NOT be asked to invent a missing gate contract
during implementation or act as the sole approver of an initial machine-contract
allocation. A failed, absent, or unapproved gate contract produces a design or
implementation issue owned by the corresponding permanent document; production
implementation remains stopped until that owner accepts the contract source.

---

## Release completion criterion

The 1.3 **design** release is complete when all six prose documents, this manifest, the propagation crosswalk, and the artifact manifest pass structural validation and every `G-01` through `G-84` has one permanent full-text owner plus all required secondary integrations. Implementation conformance additionally requires the generated machine artifacts and gates specified above.
