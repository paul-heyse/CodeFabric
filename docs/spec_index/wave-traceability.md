# Wave traceability

`RM §29 Primary specification traceability by wave` maps each of the 20 implementation waves to
its normative owners. It answers at **Part** granularity, expresses contract coverage as
**ranges**, cites **four Parts that do not exist**, and omits at least one whole document for
11 of the 20 waves.

This file refines it: exact section citations per work package, `AC-G` ranges expanded, and the
omissions recorded as additions rather than silently merged.

`RM §0` is explicit that the roadmap is subordinate — "If this roadmap conflicts with a 1.3
normative specification, the 1.3 specification and suite governance manifest prevail." Every
correction below is read that way: the roadmap's *wave boundaries* stand; its *citations* are
incomplete.

See [`README.md §2`](./README.md#2-citation-convention) for tags and
[`README.md §3.1`](./README.md#31-part-structure) for Part→section ranges.

## 1. Waves at a glance

| Stage | W | Name | RM § | Entry | Completion signal |
|---|---:|---|---|---|---|
| Foundation | 0 | Program, toolchain, and build foundation | §5 | — | every process domain builds reproducibly from a clean checkout |
| Foundation | 1 | Machine contracts, registries, and code generation | §6 | W0 | **Gate A** |
| Foundation | 2 | Daemon kernel, workspace registry, path security, source images | §7 | W1 | authorized Git/non-Git workspaces register and inventory safely |
| Foundation | 3 | Canonical data fabric, publication, overlay, snapshot kernel | §8 | W1 W2 | synthetic canonical facts survive overlay, publication, lease, recovery |
| Core facts | 4 | Source and syntax fact generation | §9 | W2 W3 | Python/Rust source and syntax facts canonical and queryable internally |
| Core facts | 5 | End-to-end vertical golden slice | §10 | W1–W4 | **Gate B** |
| Core facts | 6 | Continuous update, freshness, core equivalence | §11 | W5 | `CORE_SOURCE_V1` complete; incremental compares equal to rebuild |
| Core facts | 7 | Git-aware lifecycle acceleration and topology | §12 | W6 | Git transitions converge with generic fallback preserved |
| Semantic | 8 | Python local semantic substrate | §13 | W6 W4 | `PYTHON_SEMANTIC_V1` `PARTIAL` |
| Semantic | 9 | Pyrefly project semantics and Python profile closure | §14 | W8 W1 | `PYTHON_SEMANTIC_V1` `COMPLETE` |
| Semantic | 10 | Rust compiler/MIR semantic core | §15 | W6 W1 | `RUST_SEMANTIC_V1` `PARTIAL` |
| Semantic | 11 | Rust ownership, lowering, profile closure | §16 | W10 | `RUST_SEMANTIC_V1` `COMPLETE` |
| Semantic | 12 | Full reconciliation, completeness, contexts, unknown remainder | §17 | W7 W9 W11 | cross-provider canonical state and negative-proof semantics complete |
| Advanced | 13 | Intraprocedural flow and graph analyses | §18 | W12 | `ADVANCED_FLOW_V1` `PARTIAL` |
| Advanced | 14 | Effects, resources, concurrency, interprocedural summaries | §19 | W13 W12 | `ADVANCED_FLOW_V1` complete; **Gate C** |
| Query | 15 | Controlled language, resolver, core `PlanSpec` compiler | §20 | W12 (W14 for advanced phrases) W1 | core semantic queries lower to executable plans |
| Query | 16 | Full composable query and canonical response | §21 | W15 W14 | all eight query forms and canonical response pass conformance |
| Query | 17 | Daemon RPC, artifacts, credentials, multi-agent serving | §22 | W16 W6 | complete accepted-handle query service over local IPC |
| Query | 18 | FastMCP agent-facing outputs | §23 | W17 W1 | `SERVING_V1` complete in real MCP hosts |
| Acceptance | 19 | Failure, security, performance, upgrade, release acceptance | §24 | W0–W18 | **Gates D–G** |

Dependency graph at `RM §4.1`; permitted parallel prework at `RM §4.2`. W7, the Python lane
(W8→W9) and the Rust lane (W10→W11) may run concurrently after W6; **W12 is the integration
barrier** requiring all three.

## 2. Contracts per wave, ranges expanded

`RM §29` writes `AC-G-58`–`AC-G-69`, so a literal token scan finds two contracts where twelve
are meant. Expanded:

| W | Count | Contracts |
|---:|---:|---|
| W0 | 0 | — |
| W1 | 16 | `AC-G-01` `AC-G-02` `AC-G-03` `AC-G-04` `AC-G-05` `AC-G-06` `AC-G-07` `AC-G-08` `AC-G-44` `AC-G-46` `AC-G-53` `AC-G-58` `AC-G-65` `AC-G-70` `AC-G-71` `AC-G-72` |
| W2 | 9 | `AC-G-09` `AC-G-10` `AC-G-11` `AC-G-12` `AC-G-13` `AC-G-18` `AC-G-27` `AC-G-28` `AC-G-62` |
| W3 | 6 | `AC-G-19` `AC-G-20` `AC-G-21` `AC-G-22` `AC-G-23` `AC-G-26` |
| W4 | 4 | `AC-G-32` `AC-G-33` `AC-G-36` `AC-G-43` |
| W5 | 0 | — |
| W6 | 5 | `AC-G-24` `AC-G-25` `AC-G-29` `AC-G-41` `AC-G-79` |
| W7 | 0 | — |
| W8 | 0 | — |
| W9 | 5 | `AC-G-14` `AC-G-30` `AC-G-34` `AC-G-35` `AC-G-36` |
| W10 | 5 | `AC-G-31` `AC-G-32` `AC-G-33` `AC-G-34` `AC-G-35` |
| W11 | 1 | `AC-G-40` |
| W12 | 9 | `AC-G-15` `AC-G-16` `AC-G-17` `AC-G-37` `AC-G-42` `AC-G-48` `AC-G-51` `AC-G-71` `AC-G-73` |
| W13 | 2 | `AC-G-39` `AC-G-74` |
| W14 | 4 | `AC-G-38` `AC-G-75` `AC-G-76` `AC-G-77` |
| W15 | 6 | `AC-G-44` `AC-G-45` `AC-G-46` `AC-G-49` `AC-G-50` `AC-G-52` |
| W16 | 8 | `AC-G-47` `AC-G-48` `AC-G-51` `AC-G-53` `AC-G-54` `AC-G-55` `AC-G-56` `AC-G-57` |
| W17 | 12 | `AC-G-58` `AC-G-59` `AC-G-60` `AC-G-61` `AC-G-62` `AC-G-63` `AC-G-64` `AC-G-65` `AC-G-66` `AC-G-67` `AC-G-68` `AC-G-69` |
| W18 | 0 | — |
| W19 | 7 | `AC-G-78` `AC-G-79` `AC-G-80` `AC-G-81` `AC-G-82` `AC-G-83` `AC-G-84` |

**All 84 contracts are accounted for.** Fifteen appear in two waves — that is staged delivery,
not ambiguity: the contract's machine artifact is generated in the earlier wave and its runtime
behavior implemented in the later one.

| Contract | Waves | Split |
|---|---|---|
| `AC-G-32` `AC-G-33` | W4, W10 | provider execution interface and source-snapshot transport, first for Tree-sitter/Ruff, then for the rustc extractor |
| `AC-G-34` `AC-G-35` | W9, W10 | build/config discovery and provider sandbox, once per language lane |
| `AC-G-36` | W4, W9 | capability granularity, extended as semantic capabilities arrive |
| `AC-G-44` `AC-G-46` | W1, W15 | grammar and `PlanSpec` **schemas** generated in W1; parser and compiler built in W15 |
| `AC-G-48` `AC-G-51` | W12, W16 | completeness algebra and multi-context semantics defined with the facts, surfaced in the response |
| `AC-G-53` | W1, W16 | canonical JSON schema in W1, checksum contract exercised in W16 |
| `AC-G-58` `AC-G-65` | W1, W17 | Protobuf package and error registry generated in W1, service implemented in W17 |
| `AC-G-62` | W2, W17 | daemon singleton/lifecycle in W2, its RPC surface in W17 |
| `AC-G-71` | W1, W12 | property schema generated in W1, cardinality and storage integrity enforced in W12 |
| `AC-G-79` | W6, W19 | clean-rebuild comparator built for core-source equivalence in W6, run across the full corpus in W19 |

W0, W5, W7, W8 and W18 own no contract outright — W0 and W5 are build and integration waves,
W7/W8/W18 implement behavior whose contracts were closed earlier.

## 3. Waves that carry no `AC-G` but real spec scope

| W | Primary sections |
|---:|---|
| W0 | `FAB §2` source basis and version anchors · `FAB §2.1` workspace dependency baseline · `GEN §7` provider isolation requirements · `GEN §89` recommended crates · `SRV §18` framework and package posture · `SRV §77`/`§78` upgrade policy · `LIFE §155` recommended crates · `LIFE` Appendix E read-only gix dependency profile |
| W5 | narrow slices of `GEN` Part III/IV, `FAB` Parts IV/XI/XII, `QRY` Parts I–II, `SRV` Part II — plus `SUITE AC-G-78` golden corpus, which Gate B consumes |
| W7 | `LIFE` Part V (§37–§92) in full; `LIFE §88`–`§92` are that Part's own five implementation phases |
| W8 | `GEN §14`–`§19`, `§22`–`§25`; `ONT §8` `§33` `§34` scopes and bindings, `§12`–`§15` callables/calls/CFG |
| W18 | `SRV` Parts III–XIII (§18–§83); `QRY` Part X agent authoring guidance |

## 4. Spec-internal phases vs roadmap waves

Four specs carry their **own** implementation sequences, written before the cross-cutting wave
order existed. They are not the same axis and they do not agree.

| Spec | Phase sections | Relation to waves |
|---|---|---|
| `LIFE` §88–§92 | Phase 1 Repository correctness · 2 Git-native inventory · 3 Status/index acceleration · 4 Bulk HEAD-tree acceleration · 5 Shared caches and advanced topology | **1:1 with W7's work packages.** The only clean correspondence in the suite. |
| `FAB` §115–§120 | Phase 1 Schema and publication foundation · 2 Source and semantic base tables · 3 Types, calls, CFG · 4 Dataflow, memory, effects, language extensions · 5 Derived calculations · 6 Performance and production hardening | Cuts across W3, W4, W8–W11, W13, W19. Phase 1 ≈ W3; the rest interleave with the language lanes. |
| `GEN` §98–§104 | Phase 1 Source and syntax · 2 Semantic identity and types · 3 CFG and access events · 4 Dataflow and ownership · 5 Derived graph facts · 6 Effects and summaries · 7 Full conformance | Phase 1 ≈ W4; phases 2–4 split across the Python lane (W8–W9) and Rust lane (W10–W11); 5 ≈ W13; 6 ≈ W14; 7 ≈ W19. **Phases 2–4 are language-agnostic where the waves are language-partitioned** — the sharpest divergence. |
| `SRV` §79–§83 | Phase 1 Contract-minimal adapter · 2 Progress, cancellation, observability · 3 Large result resources · 4 Agent guidance · 5 Production hardening | All five land inside W18, whose eight work packages are a finer decomposition. |

**The roadmap's waves govern sequencing.** The spec phases remain useful as *within-document*
ordering — which table to create before which, which provider fact to extract before which — but
a wave plan should be scoped from `RM §§5–24` and this file, not from a spec's own phase list.

## 5. Corrections to `RM §29`

Verbatim `RM §29` citation, then what the wave's work packages actually require. Additions are
sections the row omits entirely; corrections are cited sections that do not resolve or that
belong to a different wave.

| W | `RM §29` says | Correction or addition |
|---:|---|---|
| 0 | *Suite process/toolchain topology; Data Fabric §2; Fact Generation provider isolation; FastMCP §18 and §77–78* | **Resolves.** `FAB §2`, `SRV §18`, `SRV §77`, `SRV §78` all exist with matching subjects. **Add `LIFE`** — WP1 pins gix and Tokio, specified at `LIFE §2` source basis, `LIFE §155` recommended crates, and `LIFE` Appendix E. **Add `SUITE AC-G-05`** — Part IV's `contracts/` layout is what WP6 establishes output locations for. |
| 1 | *Suite Manifest `AC-G-01`–`AC-G-08`, Part IV, Gate A; Ontology `AC-G-70`–`AC-G-72`; Query `AC-G-44`, `AC-G-46`, `AC-G-53`; Serving `AC-G-58`, `AC-G-65`* | **Resolves in full now that `SUITE` is present.** `SUITE` Part IV is the typed-catalog-owned `contracts/` tree; Gate A is `SUITE` Part V. Wave 1 owns the descriptor/model/schema compiler substrate, not the production daemon client or real FastMCP handlers. |
| 2 | *Lifecycle `AC-G-09`–`AC-G-11`, `AC-G-27`, `AC-G-28`, `AC-G-62`; Ontology `AC-G-12`, `AC-G-13`, `AC-G-18`* | **Add `GEN §8` Immutable source-image contract** and **`GEN §9` Canonical source coordinates** — WP6 is source-image capture, whose contract lives in `GEN`, not `LIFE`. **Add `SRV` Part II (§8–§17)** for WP1's daemon lifecycle kernel. |
| 3 | *Data Fabric `AC-G-19`–`AC-G-23`, `AC-G-26`; Data Fabric Parts II–IV, XI–XIII, XV* | **`FAB` has no Part XIII** — "XI–XIII" resolves to Parts XI and XII only (§63–§75). **Add `LIFE` Part VII (§100–§108)**: WP5 hot overlay → `LIFE §101` `§103` `§104`, WP6 publication and pointer → `LIFE §106` `§107`, WP7 leases → `LIFE §102`, recovery → `LIFE §108`. Part XV (§91–§94) correctly matches WP8. The later ontology-fabric cutover extends W3's standing substrate with `FAB §6.3`'s twenty `cpg_ontology` bundle dimensions; Waves 9–12 consume that seam and do not recreate it. |
| 4 | *Fact Generation Parts II–III/IV source-syntax sections; `AC-G-32`, `AC-G-33`, `AC-G-36`, `AC-G-43`; Ontology `CORE_SOURCE_V1` source/syntax subset* | **Add `FAB` Part XI (§63–§66)** — WP7's table encoders are the provider-observation-to-Arrow contract, batch policy, builder policy and validation. **Add `FAB §17`–`§20`** for the source and syntax tables themselves. |
| 5 | *Suite Gate B; minimal slices of Fact Generation, Data Fabric, Query, and Serving contracts* | **Resolves.** Gate B is `SUITE` Part V; the golden corpus it consumes is `SUITE AC-G-78`. Deliberately imprecise — W5 is a thin vertical slice, and `RM §2.3` says so. |
| 6 | *Lifecycle Parts I–IV, VI–VIII; `AC-G-24`, `AC-G-25`, `AC-G-29`, `AC-G-41`; Suite `AC-G-79`; Ontology `CORE_SOURCE_V1`* | **`LIFE` has no Part IV** — "Parts I–IV" resolves to Parts I–III (§4–§36). Reading it as I–III is correct: Part V is Git state, which W7 owns. **Add `FAB §71`** Durable publication and active-snapshot algorithm for WP6. **Add `LIFE §137` Correctness oracle and `LIFE §138`** for WP8's comparator — both sit in Part XIII, outside the cited range. |
| 7 | *Lifecycle Part V and §§88–92; gix correctness/acceleration contracts* | **Resolves.** `LIFE §§88–92` are inside Part V (§37–§92), so the citation is redundant rather than wrong — and useful, because those five sections are the phase decomposition W7's work packages follow 1:1. |
| 8 | *Fact Generation Python §§14–19, 22–25; Ontology Python scopes/bindings/calls/CFG* | Titles match. **Add `GEN §33` Python explicit-unknown generation** — WP7 is "Unknown and capability handling". **Add `LIFE §7` Python-specific lifecycle scenarios** for the body-local invalidation exit evidence. Note `GEN §22`–`§23` are also cited under W9. |
| 9 | *Fact Generation `AC-G-14`, `AC-G-30`, `AC-G-34`–`AC-G-36`; Python §§20–23; Ontology `PYTHON_SEMANTIC_V1`* | **Add `LIFE §95` Python semantic lane** — WP7 is sidecar lifecycle and dependency-driven invalidation. **Add `FAB AC-G-37`** for the Ruff/Pyrefly reconciliation half of WP7. |
| 10 | *Fact Generation `AC-G-31`–`AC-G-35`; Rust §§34–42; lifecycle Rust semantic lane* | **Boundary off by one in both directions.** `GEN §40` Rust place, memory, and access-event generation is W11 WP1; `GEN §42` Rust trait and dynamic-dispatch generation is W11 WP4 — both cited here. Conversely `GEN §51` Rust explicit-unknown generation is W10 WP7's compile-failure semantics and is cited under W11. |
| 11 | *Fact Generation Rust §§43–51 and `AC-G-40`; Ontology `RUST_SEMANTIC_V1`, Rust profiles* | See W10. **`GEN §§47–50` are claimed by both W11 and W14** — shared ownership, not a conflict: W11 extracts the direct Rust facts, W14 builds the effect and resource model over them. **Add `LIFE §96` Rust semantic lane** for WP7's owner fingerprints. |
| 12 | *Data Fabric `AC-G-37`, `AC-G-42`; Ontology `AC-G-15`–`AC-G-17`, `AC-G-71`, `AC-G-73`; Query `AC-G-48`, `AC-G-51`* | **The largest omission: `GEN` is absent entirely.** WP1 (reconciliation engine) and WP4 (unknown remainder) are specified at **`GEN` Part VII (§80–§85)** — range, declaration, type and call-target reconciliation, unknown materialization, capability reporting — and **`GEN` Part VIII (§86–§88)**. |
| 13 | *Ontology `AC-G-74`; Fact Generation derived §§52–65 and `AC-G-39`; Data Fabric calculations §§79A–90* | **Resolves, including `FAB §79A`** — a real section, `Derivation registry and single-authority matrix`. `GEN §§52–65` correctly stops short of `§66` interprocedural summaries, which W14 owns. |
| 14 | *Ontology `AC-G-75`–`AC-G-77`; Fact Generation `AC-G-38`, §§27–30, 47–50, 66; Suite Gate C* | **Resolves.** **Add `FAB §84` SCC and recursion calculation and `FAB §89` Interprocedural summary fixed point** for WP6/WP7. **Add `LIFE §98` Registered interprocedural derived lane** for WP8. |
| 15 | *Query `AC-G-44`–`AC-G-46`, `AC-G-49`, `AC-G-50`, `AC-G-52`; Query Parts I–II core execution* | **Simultaneously too broad and too narrow.** Too broad: Part I (§13–§20) is all eight request forms, which is W16 WP1. Too narrow: W15 needs `QRY §5`–`§10` (interface overview, request envelope, shared scope, structured freshness, semantic inputs) and `QRY` Part VIII (§103–§105, schema artifacts incl. the `PlanSpec` schema), neither inside Parts I–II. **Add `FAB` Part XV (§91–§94)** for WP7's lowering boundary: built-in DataFusion plans for relational nodes and typed `GraphOperatorPlan` nodes for graph semantics. |
| 16 | *Query `AC-G-47`, `AC-G-48`, `AC-G-51`, `AC-G-53`–`AC-G-57`; all request/response conformance* | **Add `SRV §13` Progress model and `§14` Cancellation and deadlines** plus **`LIFE` Part X (§122–§129)** for WP7 streaming and resumability. |
| 17 | *Serving `AC-G-58`–`AC-G-69`; Lifecycle `AC-G-62`; Data Fabric snapshot/artifact lease integration* | **Resolves.** The Data Fabric reference is `FAB AC-G-23` snapshot leases and retention. Wave 17 also owns the production lifespan-owned `grpc.aio` `DaemonClient`; Wave 1 only generates its descriptor and client substrate. |
| 18 | *FastMCP Serving Parts III–XIII and `SERVING_V1`* | **Add `QRY` Part X (§116–§120) Agent Authoring Guidance** and `QRY` Appendix A — WP3's four tools and WP7's agent guidance restate them for the MCP surface. Wave 18 owns the real four typed handlers and published manifest equivalence; Wave 1 only generates model/schema/fingerprint substrate. |
| 19 | *Suite Manifest `AC-G-78`–`AC-G-84`, Gates D–G; all domain release-conformance obligations* | **Resolves in full now that `SUITE` is present.** The "domain release-conformance obligations" are the `## Release conformance obligations` section each of the six specs closes with. |

### 5.1 Summary

- **Four Part citations do not resolve as written**: W3's "Parts XI–XIII" (`FAB` has no Part
  XIII), W6's "Parts I–IV" (`LIFE` has no Part IV). Both are safely readable as the truncated
  range.
- **Eleven rows omit a document** whose sections the wave's own work packages require. W12 is
  the most consequential — it omits `GEN` Part VII, which is the reconciliation algorithm the
  wave is named after.
- **Two rows have off-by-one section boundaries** between W10 and W11.
- **All 84 `AC-G` contracts are covered** once ranges are expanded, and every gate resolves.

## 6. Ontology-compiled remediation sequence

The roadmap's *Ontology-compiled data-fabric transition sequence* is a dependency correction
inside the existing program, not Wave 20 and not a new normative owner. It releases the eight
masters before candidate sealing, then orders Arrow program generation, native DataFusion
compilation, sealed analysis/closure, durable activation and lease compatibility, atomic cutover,
and decommission. The owning plan uses WP18–WP27 and M05–M08; those packet IDs are execution
history, while the master-document anchors remain the durable design authority.
