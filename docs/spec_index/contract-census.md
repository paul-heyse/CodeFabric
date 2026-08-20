# Contract and registry census

Mechanically derived inventories. Nothing here is normative — every row points at the section
that is.

See [`README.md §2`](./README.md#2-citation-convention) for the tag convention.

## 1. The 84 architecture-completion contracts

`AC-G-NN` and the bare `G-NN` form used in prose and cross-layer tables are **the same anchor**.
The contract is defined once, as a `## AC-G-NN — Title` section with `### Decision` and
`### Contract` subsections, in the document that permanently owns it.

**Owner** is from `SUITE` Part II, the authoritative gap-ownership table.
**Consumers** are derived by transposing the six `## Cross-layer integration obligations`
tables — those list, per document, the contracts owned *elsewhere* that bind it. `SUITE` carries
no such table; it is the origin, not a consumer.
**Wave** is from `RM §29`, with its ranges expanded. Two waves means staged delivery: the
contract's machine artifact is generated in the earlier wave and its behavior implemented in
the later one.

| Contract | Title | Owner | Consumers | Wave |
|---|---|---|---|---|
| `AC-G-01` | Master architecture, terminology, ownership, and precedence | `SUITE` | — | W1 |
| `AC-G-02` | Normative version for every artifact | `SUITE` | — | W1 |
| `AC-G-03` | Compatibility matrix and fail-fast negotiation | `SUITE` | — | W1 |
| `AC-G-04` | Requirement IDs and end-to-end traceability | `SUITE` | — | W1 |
| `AC-G-05` | Required machine artifacts and repository layout | `SUITE` | — | W1 |
| `AC-G-06` | Canonical enum and flag registry | `SUITE` | — | W1 |
| `AC-G-07` | Bundle manifests and fingerprints | `SUITE` | — | W1 |
| `AC-G-08` | Default deployment profile manifest | `SUITE` | — | W1 |
| `AC-G-09` | Generalized source-instance identity | `LIFE` | `ONT` `FAB` `QRY` `SRV` | W2 |
| `AC-G-10` | Daemon workspace registry and administrative lifecycle | `LIFE` | `GEN` | W2 |
| `AC-G-11` | Root authorization, symlink boundaries, and secure path opening | `LIFE` | `GEN` | W2 |
| `AC-G-12` | File identity across replacement, rename, and move | `ONT` | `GEN` `FAB` `LIFE` | W2 |
| `AC-G-13` | Canonical ID preimage serialization | `ONT` | `GEN` `FAB` `QRY` | W2 |
| `AC-G-14` | Analysis-context discovery, identity, and selection | `GEN` | `ONT` `FAB` `QRY` | W9 |
| `AC-G-15` | Canonical type algebra | `ONT` | `GEN` `FAB` `QRY` | W12 |
| `AC-G-16` | External dependency identity and body policy | `ONT` | `GEN` `FAB` `QRY` | W12 |
| `AC-G-17` | Cross-language and FFI linking profile | `ONT` | `GEN` `FAB` `QRY` | W12 |
| `AC-G-18` | Path canonicalization, display, URI, and ordering | `ONT` | `GEN` `FAB` `LIFE` `QRY` | W2 |
| `AC-G-19` | Complete `ServingSnapshot` manifest schema | `FAB` | `LIFE` `QRY` `SRV` | W3 |
| `AC-G-20` | Hot-overlay physical schemas and mutation representation | `FAB` | `LIFE` | W3 |
| `AC-G-21` | Overlay semantics for owner-scoped, cross-owner, and global tables | `FAB` | `LIFE` | W3 |
| `AC-G-22` | Deterministic overlay consolidation, merge, and durable rebase | `FAB` | `LIFE` | W3 |
| `AC-G-23` | Snapshot leases, overlay lifetime, result retention, and Delta vacuum | `FAB` | `LIFE` | W3 |
| `AC-G-24` | Formal freshness state machine and query barrier | `LIFE` | `GEN` `QRY` `SRV` | W6 |
| `AC-G-25` | Machine-testable lifecycle transition tables | `LIFE` | `GEN` | W6 |
| `AC-G-26` | Durable and active current-pointer transaction protocols | `FAB` | `LIFE` | W3 |
| `AC-G-27` | Operational-state persistence | `LIFE` | `FAB` | W2 |
| `AC-G-28` | Startup readiness, durable usability, and recovery generations | `LIFE` | — | W2 |
| `AC-G-29` | Logical multi-file edit batches and publication barriers | `LIFE` | `GEN` `FAB` | W6 |
| `AC-G-30` | Pyrefly sidecar wire protocol | `GEN` | `LIFE` | W9 |
| `AC-G-31` | rustc extractor protocol | `GEN` | `LIFE` | W10 |
| `AC-G-32` | Common asynchronous provider execution interface | `GEN` | `LIFE` | W4,W10 |
| `AC-G-33` | Immutable source snapshot transport | `GEN` | `LIFE` | W4,W10 |
| `AC-G-34` | Build and project-configuration discovery | `GEN` | `LIFE` | W9,W10 |
| `AC-G-35` | Provider sandbox and trust model | `GEN` | `LIFE` | W9,W10 |
| `AC-G-36` | Provider capability granularity and aggregation | `GEN` | `ONT` `LIFE` `QRY` | W4,W9 |
| `AC-G-37` | Canonical reconciliation algorithm | `FAB` | `GEN` | W12 |
| `AC-G-38` | Declarative model-pack format, matching, and trust | `GEN` | `ONT` | W14 |
| `AC-G-39` | Derived-analysis precision profiles | `GEN` | `ONT` `FAB` `QRY` | W13 |
| `AC-G-40` | Generated, expanded, stub, shim, and lowered source capture | `GEN` | `ONT` | W11 |
| `AC-G-41` | Operational dependency graph schema and update algorithm | `LIFE` | `GEN` `FAB` | W6 |
| `AC-G-42` | Derivation materialization matrix | `FAB` | `GEN` `QRY` | W12 |
| `AC-G-43` | Unsupported, oversized, binary, generated, and vendored files | `GEN` | `FAB` `LIFE` | W4 |
| `AC-G-44` | Controlled semantic language grammar and phrase registry | `QRY` | `SRV` | W1,W15 |
| `AC-G-45` | Deterministic semantic resolver architecture | `QRY` | `SRV` | W15 |
| `AC-G-46` | Typed internal `PlanSpec` | `QRY` | `SRV` | W1,W15 |
| `AC-G-47` | Result-reference role type system and selector grammar | `QRY` | `FAB` `SRV` | W16 |
| `AC-G-48` | Completeness and negative-proof algebra | `QRY` | `ONT` `GEN` `FAB` `SRV` | W12,W16 |
| `AC-G-49` | Entity matching, qualified-name parsing, grouping, and ranking | `QRY` | `FAB` `SRV` | W15 |
| `AC-G-50` | Semantic source-boundary compiler | `QRY` | `FAB` `SRV` | W15 |
| `AC-G-51` | Multi-context query semantics | `QRY` | `FAB` `LIFE` `SRV` | W12,W16 |
| `AC-G-52` | Query cost model, defaults, and hard limits | `QRY` | `FAB` `SRV` | W15 |
| `AC-G-53` | Canonical JSON and checksum contract | `QRY` | `FAB` `SRV` | W1,W16 |
| `AC-G-54` | Canonical human-readable fact statements | `QRY` | `FAB` `SRV` | W16 |
| `AC-G-55` | Source-context wire encoding | `QRY` | `FAB` `SRV` | W16 |
| `AC-G-56` | Streaming, chunk interning, terminal completeness, and resumability | `QRY` | `FAB` `SRV` | W16 |
| `AC-G-57` | Query plan cache contract | `QRY` | `FAB` `LIFE` `SRV` | W16 |
| `AC-G-58` | Complete Protobuf service and query state machine | `SRV` | `LIFE` | W1,W17 |
| `AC-G-59` | Cancellation, acknowledgement, reconnect, and orphan handling | `SRV` | `LIFE` | W17 |
| `AC-G-60` | Capability credential issuance, binding, rotation, and revocation | `SRV` | `LIFE` | W17 |
| `AC-G-61` | Local IPC platform and security profile | `SRV` | `LIFE` | W17 |
| `AC-G-62` | Daemon service, configuration, discovery, singleton, and upgrade behavior | `LIFE` | `SRV` | W2,W17 |
| `AC-G-63` | Immutable result artifact store | `SRV` | `QRY` | W17 |
| `AC-G-64` | Delivery precedence, host limits, and automatic externalization | `SRV` | `QRY` | W17 |
| `AC-G-65` | Stable error registry and layer mappings | `SRV` | `QRY` | W1,W17 |
| `AC-G-66` | Public status contract and redaction levels | `SRV` | `QRY` | W17 |
| `AC-G-67` | MCP resource read, range, expiry, and release semantics | `SRV` | `QRY` | W17 |
| `AC-G-68` | Multi-agent fairness, reservations, and starvation guarantees | `SRV` | `LIFE` `QRY` | W17 |
| `AC-G-69` | Fine-grained source disclosure and fact ACL policy | `SRV` | `QRY` | W17 |
| `AC-G-70` | Machine ontology registry | `ONT` | `GEN` `FAB` `QRY` | W1 |
| `AC-G-71` | Property schema, value types, cardinality, null, and storage mapping | `ONT` | `GEN` `FAB` `QRY` | W1,W12 |
| `AC-G-72` | Mandatory conformance profiles | `ONT` | `GEN` `FAB` `QRY` | W1 |
| `AC-G-73` | Unknown entities, unknown remainder, and explicit negative facts | `ONT` | `GEN` `FAB` `QRY` | W12 |
| `AC-G-74` | Graph projection registry | `ONT` | `GEN` `FAB` `QRY` | W13 |
| `AC-G-75` | Interprocedural summary semantics registry | `ONT` | `GEN` `FAB` `QRY` | W14 |
| `AC-G-76` | Static concurrency and happens-before semantics | `ONT` | `GEN` `FAB` `QRY` | W14 |
| `AC-G-77` | Effect and resource model semantics | `ONT` | `GEN` `FAB` `QRY` | W14 |
| `AC-G-78` | End-to-end golden corpus | `SUITE` | — | W19 |
| `AC-G-79` | Canonical clean-rebuild comparator | `SUITE` | — | W6,W19 |
| `AC-G-80` | Cross-document and machine-contract conformance harness | `SUITE` | — | W19 |
| `AC-G-81` | Deterministic fault-injection harness | `SUITE` | — | W19 |
| `AC-G-82` | Performance acceptance profiles and degradation behavior | `SUITE` | — | W19 |
| `AC-G-83` | Upgrade, migration, reindex, rollback, and acceptance suite | `SUITE` | — | W19 |
| `AC-G-84` | Security and adversarial-input test corpus | `SUITE` | — | W19 |

### 1.1 What the census establishes

- **All 84 contracts have exactly one owner** and appear exactly once as a `## AC-G-NN`
  section. 15 are owned by `SUITE` (`AC-G-01`–`AC-G-08`, `AC-G-78`–`AC-G-84`), 69 by the six
  domain specs — ONT 14, QRY 14, GEN 12, SRV 11, LIFE 10, FAB 8.
- **All 84 are cited by `RM §29`** once its ranges are expanded. Reading `RM §29` literally
  suggests otherwise: it writes `AC-G-58`–`AC-G-69` and `AC-G-19`–`AC-G-23`, so a
  token-level scan finds only the endpoints. Fifteen contracts appear in two waves.
- **Cross-layer binding is heavily concentrated.** The most-consumed contracts are the identity
  and ontology-registry ones — `AC-G-09`, `AC-G-12`, `AC-G-13`, `AC-G-15`, `AC-G-18`,
  `AC-G-70`–`AC-G-77` — each binding three or four downstream documents. Change one of those and
  the blast radius is the whole suite.
- **`SUITE`-owned contracts have no listed consumers** because no document declares them as
  inbound obligations. That is a structural artifact of the crosswalk, not evidence that they
  bind nothing: `AC-G-01`–`AC-G-08` govern every document.

## 2. Transposed consumer view

The same relation grouped by owner — useful when you are editing a spec and need to know who
depends on what you own.

| Owner | Contracts owned | Contracts it consumes from elsewhere |
|---|---:|---:|
| `SUITE` | 15 (`AC-G-01`–`08`, `78`–`84`) | — (origin) |
| `ONT` | 14 | 7 |
| `GEN` | 12 | 23 |
| `FAB` | 8 | 32 |
| `LIFE` | 10 | 23 |
| `QRY` | 14 | 27 |
| `SRV` | 11 | 18 |

`FAB` consumes the most (32) and owns among the fewest (8) — it is where other layers' contracts
land as schema. `ONT` consumes the least (7) and is consumed the most; it sits at the root.

## 3. Readiness gates

Defined at `SUITE` Part V. `RM §26` maps them to closure waves.

| Gate | Contract | Closes at | What it proves |
|---|---|---|---|
| A — Contract generation | `SUITE` Part V | `RM W1` | all registries, schemas, protocols, identity vectors, manifests and traceability files exist and pass `codefabric-contracts verify` without released-profile warnings |
| B — Vertical golden slice | `SUITE` Part V, backed by `AC-G-78` | `RM W5` | one Python owner, one Rust MIR owner, one unknown fact, one property fact, one relation fact, one derived projection, one hot-overlay update, one durable publication, one semantic query, one streamed result, one artifact result — end to end |
| C — Continuous-update equivalence | `SUITE` Part V, backed by `AC-G-79` | `RM W14` | every golden edit scenario converges incrementally and compares equal to a clean rebuild |
| D — Failure and recovery | `SUITE` Part V, backed by `AC-G-81` | `RM W19` | all blocker/high fault scenarios, including process death at every persistence boundary |
| E — Security and authorization | `SUITE` Part V, backed by `AC-G-84` | `RM W19` | local IPC, credentials, sandbox, path confinement, source ACLs, artifacts, malformed input, cross-agent isolation |
| F — Performance | `SUITE` Part V, backed by `AC-G-82` | `RM W19` | selected hardware/workload profile meets all hard SLOs with a reproducible report |
| G — Upgrade and rollback | `SUITE` Part V, backed by `AC-G-83` | `RM W19` | one additive and one breaking synthetic upgrade complete, compare correctly, and roll back within the preserved window |

`SUITE` Part V closes with a rule worth quoting: *"An LLM programming agent SHALL NOT be asked
to invent a missing gate contract during implementation. A failed or absent gate produces a
specification or implementation issue owned by the corresponding permanent document."*

## 4. Coded enum registries — `ONT §62`

`ONT §62 Canonical evidence, resolution, directness, and completeness registries` is the **only
place in the suite where stable integer codes are actually assigned** — roughly 104 values
across ten sub-registries. `SUITE AC-G-06` supplies the discipline they follow; `FAB §8
Canonical enum and state registries` is the canonical citing site and carries the suite's one
substantive section-level cross-reference ("exactly those in ontology §§62.1–62.10").

| Sub-registry | ONT § | Values |
|---|---|---|
| Evidence certainty | §62.1 | 7, codes 10–70 (`SOURCE_EXACT`=10 … `UNRESOLVED`=70) |
| Resolution class | §62.2 | 9, codes 10–90 (`EXACT`=10 …) |
| Directness | §62.3 | 4 — `DIRECT` `TRANSITIVE` `SUMMARY` `NOT_APPLICABLE` |
| Completeness | §62.4 | 5 |
| Owner-capability state | §62.5 | 12, codes 10–120 |
| Provider-run state | §62.6 | 12, codes 10–120 |
| Query execution / availability / completeness / freshness / limit / dependency | §62.7 | six enums, 25 values total |
| `DurablePublicationState` / `ServingActivationState` | §62.8 | 7 + 6 |
| `SourceTrust` / `EventStreamHealth` / `GitAcceleration` | §62.9 | 5 + 4 + 8 |
| Registry governance | §62.10 | append-only rules |

Code discipline from `SUITE AC-G-06`: code `0` is reserved for invalid/uninitialized and never
persisted; codes are positive, append-only, and never reassigned; registries increment by ten
for readability but **gap insertion is prohibited after release** — new values append past the
highest released code; names are `UPPER_SNAKE`, public slugs `lower-kebab`.

Flag words are 64-bit with a fixed band layout — bits 0–31 language-neutral semantic, 32–47
language-profile, 48–55 generated/lowered representation, 56–62 reserved, bit 63 always zero.
Mutually exclusive flags must become an enum domain rather than consume separate bits.

## 5. The registries `RM W1` must instantiate

`RM §6` W1 work package 3 names thirteen registries. `SUITE` Part IV gives each its generated
artifact path. This table records where the *semantics* are specified and whether the *values*
are written down in prose anywhere.

| Registry | Specified at | Generated artifact | Values enumerated in prose? |
|---|---|---|---|
| Entity | `ONT AC-G-70` | `contracts/registry/ontology-entity-registry.yaml` | Names yes (~208 `UPPER_SNAKE` tokens across `ONT` §§5–58), **codes no** |
| Relation | `ONT AC-G-70` + `ONT` Part VII | `contracts/registry/ontology-relation-registry.yaml` | **Yes — ~154 names** in 13 grouped fences, `ONT` §69–§81 |
| Property | `ONT AC-G-71` | `contracts/registry/ontology-property-registry.yaml` | **No** — full 20-field record schema, zero property names |
| Unknown | `ONT AC-G-73` + `ONT §32` | `contracts/registry/unknown-registry.yaml` | **Yes** — 8 unknown classes + 4 negative-fact families |
| Projection | `ONT AC-G-74` (informal counterpart `ONT §60`) | `contracts/registry/projection-registry.yaml` | **Yes — 13 mandatory** (see §6 below) |
| Summary | `ONT AC-G-75` | `contracts/registry/summary-registry.yaml` | Schema + one named profile, `CALLABLE_SUMMARY_BALANCED_V1` |
| Capability | `GEN AC-G-36` | `contracts/registry/capability-registry.yaml` | **Yes** — 8 scope kinds + 22 capability families, explicitly "representative", not closed |
| Error | `SRV AC-G-65` | `contracts/registry/error-registry.yaml` | **Yes** — 9 numeric domains 1000–9999, 12-field record, ~26 named errors. Conflicts with `QRY §47`; see [`README.md §7.3`](./README.md#73-conflicts-between-specs) |
| Provider | `SUITE` Part IV | `contracts/registry/provider-registry.yaml` | **No defining prose section** |
| Derivation | `FAB §79A` + `FAB AC-G-42` | `contracts/registry/derivation-registry.yaml` | **Yes** — 6-row authority matrix, 11-row materialization matrix, 10 custom operators |
| Phrase | `QRY AC-G-44` | `contracts/registry/phrase-registry.yaml` | Grammar and record schema yes; the phrase catalogue in `QRY` §§50–94 is explicitly **non-normative** — `QRY §49` calls it "recommended semantic meanings, not mandatory exact wording" |
| Enum | `ONT §62` + `SUITE AC-G-06` | `contracts/registry/enum-registry.yaml` | **Yes — ~104 coded values.** The only registry with real integers |
| Flag | `SUITE AC-G-06` | `contracts/registry/flag-registry.yaml` | Band layout yes; **no individual bit is named** |

## 6. Profiles

Two orthogonal axes routinely confused with each other.

**Conformance profiles** — `ONT AC-G-72 Mandatory conformance profiles`. Status vocabulary is
`COMPLETE | PARTIAL | UNAVAILABLE | NOT_APPLICABLE`.

| Profile | Completed at | Meaning |
|---|---|---|
| `CORE_SOURCE_V1` | `RM W6` | current source/syntax facts continuously maintained |
| `PYTHON_SEMANTIC_V1` | `RM W9` | required Python semantic facts complete for selected contexts |
| `RUST_SEMANTIC_V1` | `RM W11` | required Rust compiler/MIR/ownership facts complete |
| `ADVANCED_FLOW_V1` | `RM W14` | flow, alias, effects, resources, recursion, summaries complete |
| `SERVING_V1` | `RM W18` | agents can consume the complete service through FastMCP |

`LIFE`, `QRY` and `SRV` never name these five, though all three gate behavior on capability
status.

**Precision profiles** — `GEN AC-G-39 Derived-analysis precision profiles`: `FAST_V1`,
**`BALANCED_V1`** (default), `PRECISE_V1`. These select analysis precision, not conformance.
`FAB AC-G-42`'s materialization matrix uses `BALANCED_V1` as a column value, and `RM W13`
implements points-to and alias under it.

**Graph projections** — `ONT AC-G-74`, 13 mandatory: `SYNTAX_TREE_V1` `SYMBOL_BINDING_V1`
`TYPE_GRAPH_V1` `CALL_EXACT_V1` `CALL_SOUND_V1` `CFG_NORMAL_V1` `CFG_FULL_V1` `DATAFLOW_V1`
`ALIAS_V1` `OWNERSHIP_V1` `EFFECT_V1` `DEPENDENCY_V1` `CONCURRENCY_V1`.

## 7. Query and serving surface

**The eight request forms** — defined at `QRY §4.2`, restated verbatim as a conformance fence at
`QRY §107 Query-form conformance`, and named again in `QRY` Appendix A. They are plain-language
lowercase phrases, not identifiers:

| # | Form | QRY § |
|---:|---|---|
| 1 | `find code entities` | §13 |
| 2 | `retrieve facts about code` | §14 |
| 3 | `follow code relationships` | §15 |
| 4 | `find connecting fact paths` | §16 |
| 5 | `match a code fact pattern` | §17 |
| 6 | `combine result sets` | §18 |
| 7 | `summarize objective facts` | §19 |
| 8 | `retrieve source and syntax context` | §20 |

`SRV §30 Why the server does not expose eight query-form tools` explains the deliberate
collapse into one MCP tool.

**The four MCP tools** — `SRV §22 query_code_graph` · `SRV §23 validate_code_graph_query` ·
`SRV §24 get_code_graph_status` · `SRV §25 get_code_graph_reference`. Resources are at
`SRV §27` — 12 `cpg://` URIs (`snapshot/current`, `capabilities/current`, `schema/request/1`,
`schema/response/1`, four `schema/mcp/*-tool-output/1`, `spec/query/1`, `guide/agent`,
`recipes/index`, `recipes/<recipe-name>`) plus nine `cpg-result://<result-id>[/…]` forms.
Prompts are at `SRV §28`.

**The gRPC service** — `SRV §9` sketches 7 methods; `SRV AC-G-58` specifies 9. The AC-G contract
governs.

## 8. Cross-spec vocabulary

Of roughly 450 distinct `UPPER_SNAKE` identifiers in the suite, the overwhelming majority appear
in exactly one document. Only sixteen appear in four or more, and all sixteen come from the
byte-identical `§0` preamble — which means they are shared *by construction*, not by
cross-referencing.

| Group | Tokens | Defined at |
|---|---|---|
| Negotiation errors | `INCOMPATIBLE_MAJOR` `UNSUPPORTED_MINOR` `BUNDLE_DIGEST_MISMATCH` `REQUIRED_FEATURE_UNSUPPORTED` `SCHEMA_DIGEST_MISMATCH` `TOOLCHAIN_MISMATCH` `MODEL_PACK_INCOMPATIBLE` | `§0.4` in every spec; contract at `SUITE AC-G-03` |
| Freshness policies | five, plus `POTENTIALLY_STALE` | `§0.9`; contract at `LIFE AC-G-24` |
| ID encoding | `BLAKE3_128` | `§0.12`; contract at `ONT AC-G-13` |

Everything else is single-document vocabulary. Do not expect a suite-wide glossary to exist —
use `rg -w` against the owning spec, or the relation inventory at `ONT` Part VII.
