# CodeFabric: detailed implementation of remaining outcomes 4–8

Created 2026-09-09 against `126cf71f`; progress reconciled 2026-09-09 against the canonical `master` tree through production commit `4cc74d7c` (`Retain native Rust diagnostic locations and suggestion edits`). Implementation is paused at the user-requested stopping point.

This document expands outcomes 4–8 of the [production implementation plan](codefabric_pragmatic_production_implementation_plan.md). It is the detailed execution portion of that same backlog, not a competing plan or a new workflow. [STATUS](../../STATUS.md) remains the handoff for demonstrated behavior. Implementation is paused at the user-requested checkpoint. The status notes distinguish demonstrated committed behavior, uncommitted work and remaining acceptance; writing or updating this plan is not implementation evidence.

The objective is a useful, continuously updated Python/Rust code property graph, queried through the Rust daemon and thin FastMCP adapter, with Arrow, DataFusion and Delta doing the data work. Every selected fact family, all eight query forms, composition, truthful unfinished scope and sustained operation remain required. The first useful release is an intermediate delivery boundary, not a reduction of the full target.

## 1. Scope, authority and current baseline

### 1.1 Product authority

Use the revised selected domain specifications, with the [consolidated pragmatic review](../reviews/codefabric_pragmatic_product_delivery_consolidated_review_2026-09-08.md) and parent plan's delivery/resource decisions. Historical packet numbers are navigation only.

| Tag | Selected specification and relevant sections |
|---|---|
| ONT | [Fact ontology](../authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.3.md): §§5–58, source through language-specific semantics; §§61–67, metadata/identity/unknowns; AC-G-12–18 and AC-G-70–77, identity, schemas, projections, summaries and precision |
| GEN | [Fact generation](../authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.3.md): §5, authority; §§14–51, language providers and analyses; §§52–79, common analyses and relationship families; §§80–88, reconciliation, coverage and publication |
| FAB | [Data fabric](../authoritative_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v2.3.md): selected Arrow/schema, typed DataFusion, exact Delta, resource and maintenance contracts |
| QRY | [Semantic queries](../authoritative_design/code_property_graph_semantic_query_specification_v2.3.md): §4, The eight request forms; §5, Composition DAG and execution semantics; §§6–10, scope, unknowns, ordering, limits and response; §13, Preparation, input requirements, atomic start, and live references |
| LIFE | [Continuous lifecycle](../authoritative_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v2.3.md): authoritative source capture, present-state repository observations, update ownership, invalidation, activation, retention and recovery |
| SRV | [FastMCP serving](../authoritative_design/present_state_cpg_fastmcp_serving_specification_v2.3.md): §§4–6, handshake and accepted query lifecycle; §§7–13, public tools, progress, response, resources, authorization and lifespan |

### 1.2 What already works and what must change

This is the current implementation baseline, reconciled from code, commits and recorded checks on 2026-09-09. The original plan started at `126cf71f`; that creation baseline no longer describes the running implementation. No outcome from 4 through 8 is complete. Existing checks establish only their recorded scope; see §9.1 and STATUS for validation limits.

| Surface | Implemented now | Remaining implementation |
|---|---|---|
| Startup/publication/reopen | Linux outcomes 1–3; fresh and live source/syntax activation before semantic convergence; exact source-only and terminal semantic reopen | Preserve these paths across complete contexts, all query forms and maintenance |
| Resources | Shared workspace/pool/store owners, joined cancellation, Linux containment, broad workstation budgets, one charged immutable toolchain capture per semantic pass | Finite retained providers/caches/history/results, sustained pressure/recovery and representative cost measurements |
| Rust contexts | Captured path dependencies; package/virtual workspace inheritance; targets, custom/disabled build scripts, library linkage, configured platforms/rustflags and explicit feature/default-feature/profile selections; closed ordinary failure retains positive observations and typed diagnostics | Registry/git and generated/proc-macro/build input closure; complete actual unit/host-target/unified-feature contexts; external tool changes, raw argv, retained build cache and parallel scheduling |
| Python contexts | Captured version/platform/ordered roots and supported configuration; one chunked checker inventory; source/stub coexistence; exact checker-selected call definitions; source files outside import roots remain queryable | External roots/distributions/stubs, additional effective settings, complete type/member/import/reference propositions and retained incremental checker state |
| Source/syntax | Python Tree-sitter/Ruff and six Rust syntax relations; recoverable errors; raw source paths; UTF-8/BOM/ASCII/Latin-1 Python and rustc BOM/CRLF original-byte mappings; UTF-8/UTF-16 public columns | Full lexical/CST/trivia census, additional codecs/coordinate contexts, complete path/rename behavior, retained parser/query state and broader incomplete-edit cases |
| Canonical facts | Source, selected entities/declarations, Python lexical references and Python/Rust call occurrences; native DataFusion normalization and exact Delta type/list restoration | Full semantic families, authority/conflicts, external/generated identities, executable instances, types/propositions and complete edit-time correspondence |
| Public forms | Four limited installed-client paths: declaration selection/facts, one-step calls and Python lexical references, exact declaration/function definition/function body source with surrounding lines and independent disclosure checks | Remaining meanings/subjects/directives, broader traversal and full composition; last four forms remain undelivered |
| Processing/wire | Requested files/targets and family scopes, exact outgoing Rust caller scope, typed optional Cargo selections, retained 130-partition remainder paging, source/semantic-current barriers and observed result truncation | All-family/owner/dependency/frontier scope, efficient authorized status, target/family freshness and historical selectors |
| Analyses | Selected real raw compiler/checker inputs and substantial existing typed Python/MIR/common algorithms; native ordinary diagnostic messages/children/spans/suggestions/edits | Complete production wiring and corrected algorithms, canonical/public consumers, full family precision and replacement |
| Updates/corpus | Owned native watch/coalescing/census loop, whole-context replacement, source/semantic stages, obsolete completion rejection, live/clean comparisons for selected source/config/stub/root/path/Cargo cases | Git/external/poll/root-recovery topology; retained providers, selective persistence and fair scheduling; full independent all-family edit corpus |
| Persistence/maintenance | Exact reopen, immutable input reuse, native checkpoints and retention-aware dry runs | Unchanged version reuse, native optimize/destructive vacuum, coordinated finite retention and actual reclamation |
| Measurement | RSS/cgroup/headroom signals and attributable small native scenario timings | Correlated phase telemetry, representative workloads/distributions, sustained edit/retention/recovery cost and measured optimization |

Current bounds supersede the original 64-module/8 MiB startup limitation: Pyrefly accepts up to 16,384 modules, 32 MiB per source file, 512 MiB source bytes and 32 MiB descriptors per context run, sent in chunks of up to 64 modules. Mixed source capture allows 1 GiB. Frames remain bounded at 4 MiB; context opening remains unary/bounded. These are configured ceilings, not measured optimal capacities or proof that arbitrary external contexts work. The compiler source manifest retains its separate bounded contract. Scaling and retention remain required; increasing an input bound does not implement scheduling.

### 1.3 Execution policy

Implement and integrate in the canonical working tree. Each slice ends with a working consumer, relevant checks and a concise STATUS update; commit coherent changes promptly. Do not accumulate independently designed interfaces in temporary worktrees. The sequence below does not mandate subagents or a fixed commit count.

Runtime actions retain compact input/output references, selected context/snapshot, outcome, coverage and useful diagnostics. Git records code changes. Do not recreate proof-language execution, per-allocation artifacts, source-artifact certification, repeated audit/status cycles or licensing investigations. Types, short algorithm arguments, independent examples and targeted failure tests provide proportionate assurance.

### 1.4 Progress checkpoint and how to execute the remaining scope

**Checkpoint requested by the user on 2026-09-09.** Finish the current typed Rust diagnostic-detail
slice, record its final checks, update this plan and STATUS, then stop. This supersedes automatic
continuation of the earlier full-scope implementation request. The full target remains required
when work is explicitly resumed; no outcome from 4 through 8 is complete.

The current implementation includes the limited public forms and live source/semantic lifecycle
listed in §1.2. Do not redo those portions. Commit and scenario references remain in STATUS and
§9.1. In particular, source-context function/body/surrounding-line behavior, original-byte decoding,
raw source paths, Python root/stub/configuration changes, Cargo platform/feature/profile selections,
source-first fresh startup and retained failed-compilation diagnostics are implemented slices.
Their acceptance does not establish all meanings, contexts, fact families or sustained operation.

The diagnostic-detail continuation preserves ordinary native child notes, exact or explicitly
unmapped locations, suggestion alternatives and multipart edits in application-owned Arrow and
Delta. Failed compilation still leaves requested target families unknown. Separate future-breakage
report semantics and canonical/public diagnostic consumers remain open, as do the complete 7D
source/type/instance/lowering target and external/generated source closure in 4A.

| Slice | Current status | Next unmet boundary |
|---|---|---|
| 4A | Partial; captured selections and failed-run diagnostics implemented | External/generated/effective unit inputs, cache/scheduler, complete diagnostic consumers and tool-change invalidation |
| 4B | Partial, committed | Complete effective external Python contexts and semantic output; retain chunking and definition anchors |
| 4C | Partial, committed | Full source/syntax/coordinate/path behavior and parser reuse/live edits |
| 4D | Partial, committed | Remaining canonical families, authority/unknowns and external/generated/edit-time identity |
| 4E | Four limited forms demonstrated | Extend calls and SourceContext, broaden first-four meanings and composition |
| 5A | Partial; call and lexical-reference scopes demonstrated | Query dependency/owner scope, all families and efficient authorized live status |
| 5B | Partial live source/semantic barriers and typed status | Target/family-specific convergence and historical query selection |
| 6A | Partial native watch/census/rescan loop | Git inclusion, external roots, polling profile and root/config recovery |
| 6B | Partial live replacement and generation fences | Negative/configuration dependencies, complete identity rules and broader races |
| 6C | Partial shared orchestration and source/semantic stages | Retained Tree-sitter/Pyrefly/Cargo state, selective persistence and scheduling |
| 6D | Limited mixed clean/live comparison and deterministic publication pause | Broader language/context/edit corpus and all-family comparisons |
| 7A | Open; selected lexical/call prerequisites | Complete production Python semantics, canonical/query consumers and invalidation |
| 7B | Open; existing algorithms not full production behavior | Correct Python CFG/evaluation/dataflow and actual accepted input wiring |
| 7C | Open | Python memory/effects/resources/exceptions/capture/async/concurrency on 7B |
| 7D | Partial raw foundation and ordinary native diagnostic details | Full typed Rust family census, canonical types/instances/lowering, generated/hygiene mappings and diagnostic consumers |
| 7E | Open | MIR analyses, exact private borrow facts and advanced state on real bodies |
| 7F | Open | Demand-rooted graph algorithms, structural facts and interprocedural summaries |
| 7G | Open; typed request infrastructure exists | Last four forms, complete first four, multi-block/repeated-form DAGs and full directives |
| 7H | Partial foundation; full slice open | All-form modern delivery, cursors/permissions, replay/reconnect/expiry and Rust-owned retention |
| 8A | Partial foundation | Selective persistence, unchanged version reuse and bounded edit-time growth |
| 8B | Open; checkpoint/dry-run only | Fix native commit-properties seams and enable owned compaction/destructive vacuum |
| 8C | Open | Coordinated finite retention/eviction/reclamation across all real owners |
| 8D | Partial Linux baseline | New update/provider/query/maintenance failure and recovery behavior |
| 8E | Open; signals/tooling only | Correlated runtime metrics and representative performance/retention measurements |
| 8F | Open; a few enabling fixes | Correct native pushdown/properties and measured workload-driven optimization |

The numbered requirements and acceptance paragraphs below remain the complete target. They are not a request to redo already implemented portions. Each slice's current-status note identifies what can be reused and what still needs implementation or behavioral validation. A partial family is not complete merely because its raw schema, fixture or capability label exists. No progress percentage is assigned because the remaining work is not uniform in size.

## 2. Library selection and implementation rules

### 2.1 Installed baseline

Use manifests, locks and selected source origins when implementing. These are the observed baseline, not proposed upgrades.

| Boundary | Selected libraries |
|---|---|
| Stable fabric | Arrow/Parquet 59.2.0; DataFusion 55.0.0; object_store 0.13.2; delta-rs `43a0cf10a313e5077c48637ad786a05359136bbb` |
| Stable syntax | Tree-sitter 0.26.12; Python grammar 0.25.0; Rust grammar 0.24.2; Ruff analysis crates 0.0.7 |
| Python semantics | Pinned Pyrefly 1.2.0 source graph, including the local configured-context correction and checker-selected call-definition seam |
| Rust semantics | `rustc_public` and narrow private adapters on nightly-2026-08-18; stable daemon remains Rust 1.98.0 |
| Source observation/graph | gix 0.86.0; notify-debouncer-full 0.7.0; petgraph 0.8.3, currently `std` without its optional Rayon feature |
| Rust transport/runtime | tonic/tonic-prost 0.14.6; prost 0.14.4; Tokio 1.53.1 in root/sidecar, 1.49.0 in the separately built extractor; tokio-stream 0.1.18 and tokio-util 0.7.19 in the stable domain |
| Python presentation | Python 3.14.7; FastMCP 4.0.0; MCP 2.1.1; grpcio 1.83.0; protobuf 7.36.0; Pydantic 2.13.4 |
| JSON optimization candidate | orjson 3.12.0 is covered by the reference but is not a direct adapter runtime dependency. Add it only if measured presentation serialization justifies it |

Keep one Arrow/DataFusion/object-store type universe. No new Cargo root, native Python extension, PyArrow processing layer, Flight server, SQL endpoint or FastAPI deployment is implied by reference availability. Compatible dependency additions require ordinary API/feature/lock checks; a narrow correctness patch is acceptable when the selected upstream API cannot meet a real requirement.

### 2.2 Reference-to-implementation map

All eight requested skills were used as navigators. The repository name for `codefacts-lib-ref` is `code-facts-lib-ref`. Read the relevant chapter and exact installed implementation when coding; do not follow old procedural prescriptions embedded in a reference over the current pragmatic target.

| Skill and reference | Capabilities selected for this implementation |
|---|---|
| [datafusion-pyarrow-rust-ref](../../.claude/skills/datafusion-pyarrow-rust-ref/SKILL.md); [DataFusion 55 reference](../library_ref/datafusion_rust_55_arrow59_comprehensive_advanced_reference_2026-08-23.md), chapters 43, 47, 50–54, 56; [Arrow reference](../library_ref/arrow_rust_59_datafusion55_advanced_reference_2026-08-23.md), chapters 3–7, 10, 12 | `Expr`, `DataFrame`, `LogicalPlanBuilder`, native joins/aggregates/windows and bounded recursion; `TableProvider` pushdown; `SendableRecordBatchStream`; shared `RuntimeEnv`; Arrow builders/kernels/IPC; truthful schema/statistics/property propagation |
| [code-facts-lib-ref](../../.claude/skills/code-facts-lib-ref/SKILL.md); [Tree-sitter](../library_ref/tree_sitter_rust_python.md), incremental parsing/query chapters; [Ruff](../library_ref/ruff_python_crates_advanced_reference_2026-08-18.md), chapters 2–8, 11, 14–15; [Pyrefly](../library_ref/pyrefly_rust_cpg_advanced_reference_1.2.0_2026-08-19.md), chapters 6, 9–16, 23–30; [MIR](../library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md), typed MIR, identity, continuous updates and private enrichment | Reused parsers/query packs, recoverable Ruff parsing, bulk deduplicated Pyrefly type tables/callees/members, retained checker state, typed rustc bodies/types/instances, exact private identity/borrow seams |
| [deltalake-rust-ref](../../.claude/skills/deltalake-rust-ref/SKILL.md); [exact-pin Delta reference](../library_ref/deltalake_rust_1.0.0_43a0cf10_datafusion55_arrow59_advanced_reference_2026-08-23.md), chapters 5–9, 12–14 | Exact-version readers, native DataFusion provider/writes, owner replacement, caller-owned session, commit reconciliation, compaction/checkpoint/vacuum and conditional CDF |
| [gix-notify-ref](../../.claude/skills/gix-notify-ref/SKILL.md); [gix](../library_ref/gix_rust_advanced_reference.md), chapters 3–7, 16–18, 20–21, 24–25; [notify](../library_ref/notify_debouncer_full_rust_reference.md), chapters 8, 11, 27, 29–30, 40 | Read-only repository/inclusion hints, worker-local handles, debounced events, overflow/rescan recovery, dirty sets, generation fences, watch-first startup and polling fallback |
| [rust-grpc-daemon-ref](../../.claude/skills/rust-grpc-daemon-ref/SKILL.md); [tonic daemon reference](../library_ref/rust_grpc_daemon_advanced_reference_tonic_0.14.6.md), chapters 18–19, 25–29, 37 and UDS/auth sections | Long-lived channels, streamed bounded batches, deadline propagation, cancellation trees, joined tasks/processes, UDS identity and accepted-query reconnect |
| [petgraph-ref](../../.claude/skills/petgraph-ref/SKILL.md); [petgraph](../library_ref/petgraph.md), traversal, SCC/path and chapter 20 dominance sections | Demand-rooted graph projections; BFS/DFS, SCC/condensation, topological worklists, rooted dominance; canonical mapping outside transient graph indices |
| [grpcio-orjson-protobuf-ref](../../.claude/skills/grpcio-orjson-protobuf-ref/SKILL.md); [grpcio](../library_ref/grpcio_python_advanced_reference_1.83.0.md), chapters 14, 17–19, 27; [protobuf](../library_ref/protobuf_python_advanced_reference_7.36.0.md), chapters 7, 11, 14, 16–18, 26; [orjson](../library_ref/orjson_python_advanced_reference_3.12.0.md), chapters 13, 16–19, 22 | Lifespan-owned `grpc.aio` channel; explicit deadlines/cancellation; generated presence/oneof-safe DTOs; compatible field evolution; optional bytes-oriented JSON serialization |
| [fastmcp-pydantic-ref](../../.claude/skills/fastmcp-pydantic-ref/SKILL.md); [FastMCP 4](../library_ref/fastmcp_python_advanced_reference_4.0.0.md), chapters 38–44 and tool/resource/lifespan sections; [Pydantic](../library_ref/pydantic_python_advanced_reference_2.13.4.md), chapter 21 and strict/union/serialization sections | Modern negotiated extensions, replay-aware input rounds, daemon-owned handles/resources, strict typed projections, reused `TypeAdapter`, one canonical logical response |

### 2.3 Default mechanism choices

1. **Relational work stays relational.** Express selection, normalization, reconciliation, owner replacement, set operations, grouping and fixed-depth joins using typed DataFusion plans. Bind identifiers through application schemas, not generated SQL strings. Apply authorization before scans, statistics, negative reasoning or source disclosure.
2. **Arrow remains columnar.** Build owned typed arrays and bounded `RecordBatch` streams. Reuse `Arc` buffers and native filter/take/concatenation kernels where appropriate; avoid per-row JSON maps and whole-result `collect()` on unbounded paths. A zero-copy slice may retain its entire parent allocation: copy a small long-lived result when measurement shows retained-parent waste.
3. **Graph extensions are bounded and specific.** First use native joins/aggregates or bounded `RecursiveQuery`. Use existing graph integration for algorithms that require graph state. A custom `ExecutionPlan` is warranted only where it contributes real streaming/partitioning/cancellation behavior; a scalar UDF is not a hidden whole-graph executor.
4. **Reuse plans, not exhausted executions.** Cache typed plan templates by schema/algorithm/policy/context compatibility; bind exact relation versions and create fresh execution state for each request. Never reuse a consumed stream, a cancelled physical execution or a catalog authorized for another principal.
5. **Native storage owns its format.** Use delta-rs transaction, checkpoint, optimization and vacuum code. The application owns selected versions, writer/maintenance exclusion, reader leases and recovery decisions. Do not implement a second Delta log or custom file-list deletion planner.
6. **Use concurrency where work exists.** Parallelize independent files/contexts, independent query branches and partitions within the shared budget. Keep blocking compiler/graph work off control-runtime threads. Avoid multiplying 16-worker checker pools by an unrestricted number of concurrent contexts.

## 3. Delivery order and interfaces

The first mixed-language vertical is Rust source/context preparation → real compiler result → canonical relations → four public forms. Build query-relevant coverage alongside it. Then connect the live input loop. Expand full language analyses against that working product, and finish sustained operation using measured workloads.

| Slice group | Prerequisites | Observable delivery |
|---|---|---|
| 4A–4D | Existing startup/provider owners | Correct source/context identity and actual normalized Python/Rust facts |
| 5A–5B, then 4E | 4D; develop 5A while integrating providers | First four public forms with accurate requested/completed/remainder scope |
| 6A–6D | 4E and 5B | One daemon observes edits, reports pending work and converges to clean reconstruction |
| 7A–7F | Working canonical inputs and update replacement | All remaining Python/Rust/common fact families, delivered family by family |
| 7G–7H | 4E/5B; extend as 7A–7F add facts | All eight forms, full composition, modern public delivery |
| 8A–8F | Start persistence/instrumentation during 6; finish against 7 | Finite retained state, real maintenance, recovery and measured responsiveness |
| 9 | All groups | Integrated full-product demonstration and honest final handoff |

Do not wait for every family before exposing useful answers. Conversely, do not mark a family complete because an enum, schema or test seed exists. Each family needs its actual producer → canonical relation → query consumer → coverage → update replacement path.

Shared runtime interfaces should stay small:

- A captured input set identifies workspace, source generation, immutable bytes and effective analysis contexts.
- A provider job selects requested families/scopes and resource/deadline/trust policy; accepted output is owned Arrow plus terminal scoped coverage.
- An analysis consumes immutable accepted relations and produces facts, diagnostics and scoped completeness with its algorithm/precision identity.
- A snapshot binds exact selected relation versions, source/context generation and coverage. The activation coordinator is its single publication owner.
- A query pins one authorized snapshot and a dependency scope. Its response describes results and the unfinished portion of that same requested universe.

## 4. Outcome 4: complete the first mixed-language semantic vertical

### 4A. Production Rust contexts, dependencies and scheduling

**Current status — partial, committed.** Startup now performs contained metadata/compiler work for captured packages/path dependencies and multiple target contexts, with reusable immutable sysroot/dependency blobs and retained target/family failure scope (`eba6f19a`–`b6a7d777`). This implements the startup portions of items 1–2 and 5–7, plus captured dependencies and multi-target selection in items 3–4. Captured custom/default/disabled build scripts now enter context identity, and the selected host C compiler runs from an owned dependency view without widening containment. The installed cfg-change/direct-call/source comparison against an independent clean daemon passes (124.86 s); `cargo-build-live` selects it. Standard-library build-script calls retain their unresolved scope under the broad target partition; explicit admitted outgoing caller bodies now have their own coverage. Captured `build.target` strings/arrays expand requested targets into distinct platforms, resolve/deduplicate `host-tuple`, and retain typed unavailable-platform explanations. The installed missing/mixed/config-precedence/flag-change scenario passes against clean reconstruction and exact reopen (198.82 s); `cargo-platforms-live` selects it. Four focused target/processing cases, 109 adapter tests, 212 tooling tests, default/featureless root checks and full governance pass. Compiler/sysroot/extractor bytes are now captured once per semantic publication pass with one retained budget owner; their actual content digest enters context identity. Four focused context/capture cases and the installed platform scenario pass (205.79 s). This single sample does not establish an end-to-end speedup over the preceding 198.82 s sample. Typed captured library/example linkage and exact Cargo metadata admission now support custom and combined library crate types. The extractor retains the complete flag selection; 17 strict extractor tests and seven affected context/metadata tests pass. Installed live/clean/exact-reopen declaration/call/source queries pass for `cdylib`, `dylib` and combined `rlib`/`cdylib`/`staticlib` (175.53 s); `cargo-linkage-live` selects the case. All 214 tooling tests and default/featureless root checks pass; Clippy retains 954 library/36 integration warnings with no new code/file findings. Captured package/workspace metadata now selects explicit/default/disabled features, profiles and platforms with typed failure scope. The installed inheritance/override/duplicate/invalid-selection/call/source scenario passes against clean reconstruction and exact reopen (192.07 s); `cargo-selections-live` selects it. Optional typed remainder fields retain empty feature lists and disabled defaults. Kernel list storage and logical restoration pass regular/large/view/fixed-size round trips. Sixteen final schema/context/paging checks, 109 adapter tests, 216 tooling tests, default/featureless root checks and full governance pass. Clippy retains 952 library/36 integration warnings with no new findings. Remaining: registry/git, generated/build-script/proc-macro input closure, complete effective configuration/host-target separation, retained build caches, parallel scheduling, complete diagnostic/source mapping, byte-safe paths and broader update-time cases. The full acceptance below has not passed.

**Diagnostic continuation.** The pinned native emitter now supplies bounded typed primary messages,
severity, compiler/lint code and ordinal, including closed owners when compilation fails before MIR.
The real contained successful-warning and failed-no-MIR cases pass (8.21/8.12 s); installed failed
and valid targets pass together with persisted diagnostics and explicit completed/failed context
selection (76.48 s). Retained calls in the failed parent keep their unavailable remainder.
A fully closed ordinary compiler failure can retain positive observations under the same exact
source/plan binding and kernel/process/output requirements as success. Every requested target
family remains unknown and the target remains unavailable. Canonical declaration, call and body
projections select their own native prerequisites; diagnostics alone do not enable MIR transformations.
Incomplete containment, cancellation
and protocol failure still reject publication. Primary capture bounds leave explicit gaps.
The typed-detail slice implements children/suggestion alternatives/edits and exact captured
message locations with explicit unmapped states. Its actual compiler checks pass (7.80/7.71 s),
as do installed live/clean/repair/exact-reopen details (350.01 s) and all-failed startup (58.77 s). Canonical/public consumers remain open;
live/clean repair and exact failed-epoch reopening pass (374.04 s), all-failed fresh startup
retains exact source/syntax (48.36 s), and staged cancellation/restart passes (188.20 s).
All 19 strict extractor tests, 18 Rust service checks, four canonical checks, launcher proof checks,
default/featureless root checks, full governance and 216 tooling tests pass. Clippy retains its
952/36-warning baseline. This does not close 4A or 7D.

**Surfaces:** `src/analysis_context/rust_context.rs`, `src/rust_compilation_trust.rs`, `src/rustc_source_files.rs`, `src/rustc_service.rs`, `src/fabric/production_workspace_startup.rs`, startup `inputs.rs`, `rustc-extractor/src/`, existing provider contracts and containment owners.

1. Move selected compilation preparation from the real fixture into the production startup/update path. Discover each selected package/target/context from captured Cargo manifests, lockfile, target/features/profile, compiler identity, relevant `.cargo` configuration and explicit environment inputs. Separate host build dependencies/proc macros from target compilation.
2. Run Cargo metadata through the contained dependency-preparation boundary. The production contained metadata path is now implemented; any fixture-only host invocation remains limited to its test-authored trusted project. Production must not inspect an untrusted workspace by inheriting host Cargo configuration, credential helpers, network or arbitrary tool overrides.
3. Assemble the exact resolved dependency/source/sysroot view before compilation. Reuse immutable content-addressed dependency bundles across compatible jobs; do not copy the full nightly sysroot on every edit. Account source dependencies, build scripts, proc macros, generated `OUT_DIR` inputs and declared configuration in context identity. Default compilation remains locked/offline/contained. Missing materialization becomes an actionable dependency scope, not a silently different resolution.
4. Support workspace members, multiple crate types, target triples and feature selections as distinct contexts. Key Cargo incremental/build caches by compiler, target, effective build configuration and dependency/source selection; keep build output private and outside watched source inventory. Revalidate authoritative inputs even on a cache hit.
5. Supply the existing captured source-file manifest and source digests to the extractor. Extend the implemented raw-path workspace-file seam to authorized external/generated source maps and complete reversible argument handling. Preserve application file identity and content revision separately. Do not convert a rustc index or crate-root hash into a source owner.
6. Replace the Rust gap in production `ProductionProviderRuns` with the accepted contained compilation. Publish emitted raw relations even when a separately optional family is missing, with exact scope. Produce structured diagnostics; classify parse/type/build failure independently of whether some MIR bodies were emitted. Invalidate old compiler facts before treating a failed new compilation as current.
7. Reuse the supervisor/service task and descendant ownership already implemented. Bound admitted contexts and active Cargo jobs through the workspace scheduler. Trust status must distinguish contained compilation from any explicitly selected trusted-local profile; no fallback from failed containment.

**Library utilization:** Cargo/rustc incremental caches reduce repeated compilation without defining source authority; typed `rustc_public` supplies bodies/types/instances, the exact private seam supplies stable keys. Tonic carries application-owned jobs and bounded Arrow IPC. Native Rust output never passes through Python for normalization.

**Acceptance:** a real daemon indexes the mixed fixture and a multi-package/multi-file Rust workspace; changing features/target or a build input changes context-selected output. Tests cover one dependency and generated/proc-macro input, missing dependency, compiler failure, changed captured bytes, cancellation and obsolete results. The integrated 4E check queries the expected direct call and actual owner source range through the public service. The boundary test alone is insufficient to close outcome 4.

**Configuration decision implemented by the feature/profile continuation (2026-09-09).** Store explicit Rust
analysis selections in captured `package.metadata.codefabric.rust_contexts`, with fallback to
`workspace.metadata.codefabric.rust_contexts` in the owning captured Cargo workspace. Each entry
selects `features`, `default_features`, `profile` and optional `platforms`. An absent list preserves
the existing default-feature/dev/configured-platform selection. A package list replaces inherited
selections. Effective duplicates are coalesced; invalid entries retain a preparation failure.
Workspace inheritance must agree with contained Cargo metadata. Its manifest participates in
context identity. The selected configuration is semantic input and grants no additional filesystem,
network, environment or execution authority.

Cargo's pinned `manifest.html` and `workspaces.html`, both “The metadata table,” provide this
extension point and describe package-to-workspace fallback as a tool-owned convention. A registry
profile is a viable alternative for operator overrides, but would add a separate persistence and
update path for these repository-owned build choices. Captured metadata uses the existing immutable
input and live-update path. This decision assumes manifests remain inside the authorized captured
universe. Affected consumers are target discovery, effective context preparation, contained Cargo,
requested processing scope and public remainder delivery. The installed scenario recorded above covers default/disabled
features, a custom profile, workspace inheritance and package override, duplicate selections,
missing features/profiles, independent valid contexts, live changes and clean/exact reopen. Typed
remainder fields identify the feature/profile selection when compilation cannot establish a
context ID; malformed entries remain unknown. The decision itself is not acceptance evidence.

### 4B. Complete effective Python contexts and bulk semantic extraction

**Current status — partial, committed.** Configured version/platform, captured contained inputs, raw semantic output and one-checker chunked inventories work. Item 3 is implemented within the current bounds (`1301df5a`); do not recreate the old 64-module ceiling. Item 7 now includes the narrow definition-index seam (`92bb153d`) used to normalize actual cross-module and bound-method targets. Captured configuration with fully applied version/platform/search settings now reaches the checker; unapplied settings remain scoped unavailable. The installed live/clean case passes version/platform changes, missing import creation/deletion, unsupported configuration and exact restoration (165.86 s). External roots/stubs, additional configuration, complete type/member/import/reference propositions and retained update state remain. Local source/stub pairs and ordered roots now coexist in one checker inventory, owned by input/file identity. Installed namespace-package stub deletion/recreation and same-name declaration/call identities match independent clean builds (72.87 s); `python-stubs-live` selects that case. Configured import-root reversal now also passes independent clean/live comparison (73.85 s): all captured source files retain query bindings, including files outside those roots, without adding an import search path. `python-roots-live` selects that case. These scenarios demonstrate selected semantics, not complete context acceptance.

**Surfaces:** `src/python_context.rs`, `src/analysis_context.rs`, startup `inputs.rs`/`pyrefly.rs`, `src/pyrefly_service.rs`, `pyrefly-sidecar/src/`, `third_party/pyrefly/lib/query.rs` and the pinned configuration/module-resolution seams.

1. Resolve project configuration, Python version/platform, ordered import search paths, packages/namespace packages, `.pyi` precedence, dependency distributions and selected stub/typeshed bundles. Capture/digest the semantic inputs actually consumed. Context inventory is internal analysis input, not a public environment-inventory graph domain.
2. Materialize authorized external roots into the sidecar's immutable input view. Preserve ecosystem package/version/source identity and explicit missing-body policy. Do not execute project imports to discover semantics.
3. Preserve the implemented chunked input descriptors and one complete inventory/check under an effective checker context; extend scheduling and bounds when actual workloads require it. Increase legitimate per-file capacity with measured bounded reads; report a concrete file/context remainder when a configured bound is reached. Do not start independent checkers per chunk and lose cross-module resolution.
4. Use `Query::get_type_table_in_file` once per needed file/checker revision, intern its deduplicated structural type table into application-owned type identities, and validate every local type-table index. Use `get_callees_with_location`, `get_attributes` and solver-backed subtype requests for their supported meanings. Preserve raw type representation and unknown/error variants.
5. Use the pinned module resolver/TSP handler seam for import targets and declared/computed/expected propositions absent from the bulk Query API. Export declarations/references through the existing pinned semantic/index seam where needed; do not infer project-aware references from token spelling. Keep these adapters inside the sidecar and return the same owned Arrow contract, rather than introducing multiple independent semantic daemons.
6. A `TypeQueryStmtWalker` may filter extraction for an explicitly narrow request; its callback does not recursively traverse children. Full-profile indexing must retain every required occurrence. Use the timing-capable type-table methods to distinguish checker work from transformation cost.
7. Preserve the existing configured-context patch and handshake profile. Only add a further pinned-source seam when a required semantic output is otherwise inaccessible; test its actual answer and isolate upstream types.

**Acceptance:** real imports/re-exports, namespace packages, version/platform branches, external `.pyi` precedence, generic/member calls and unresolved dynamic targets produce expected semantic output, with public answers exercised in 4E. Context changes invalidate previous answers. A missing stub/dependency yields scoped unknowns without discarding unrelated valid facts.

### 4C. Complete owned source and syntax inputs for both languages

**Current status — partial, committed.** The real Rust Tree-sitter lane and malformed-source retention in item 1 exist (`11a61909`); Python parse failures retain syntax/diagnostics and qualify semantic coverage. Ruff callable/call-site/callable-syntax output is now published (`734821db`). Raw Python source paths, escaped file-URI transport and root initializer inputs now survive installed source/call queries and clean/live deletion/recreation (81.85 s after the final diagnostic-owner refinement). Exact checker diagnostic paths prevent lossy display collisions; all 34 sidecar tests and strict sidecar checking/lint pass. `python-paths-live` selects the installed case; default/featureless root checks pass, with the same 955-warning library Clippy baseline. Shared decoding and original-byte projections now cover UTF-8/BOM/Latin-1 Python through Ruff and Pyrefly and BOM/CRLF Rust through the pinned compiler normalization map. The mixed installed source/call scenario passes (203.40 s), including encoding changes/restoration and independent clean daemons; `decoded-source-live` selects it. Thirty-six sidecar, 14 extractor and 45 affected root source tests pass, including governed 10,000-file capture. Both strict executable checks, default/featureless root checks, all 204 tooling tests and full governance pass. Root library Clippy retains its 955-warning baseline with no changed-line library/integration findings. Explicit zero-based UTF-8/UTF-16 source-context columns now pass mixed native/clean acceptance with astral characters (202.48 s) and public split-character truncation (204.34 s). BOM and partial-character byte positions remain unmappable as text positions. Raw compiler manifests now retain non-UTF-8 paths and legacy UTF-8 reads. The pinned local-file seam supplies a binary path independently of remapped display names; owner verification and call-source joins consume it. `rust-paths-live` passes mixed declarations/calls/source through edits, independent clean comparison and exact reopen (138.00 s); a remapped compiler IPC round trip passes with all 16 extractor tests. Remaining work covers the full lexical/CST census, parser/query reuse, further coordinate contexts/codecs, reversible compiler paths and live incomplete-edit behavior. Fresh startup now activates source/syntax with pending semantics, using the existing update owner for convergence; the mixed initial/pending/deadline/obsolete/restart scenario passes (186.06 s).

**Surfaces:** `src/source_image/`, `src/provider_native_syntax.rs`, `src/production_provider_recipe.rs`, provider relation schemas and normalization consumers.

1. Retain the implemented Rust Tree-sitter grammar lane and complete the remaining Python/Rust source, lexical and CST coverage. Emit raw kind, normalized kind, parent/field/child order, named/anonymous distinctions, errors/missing nodes, comments/tokens and enclosing owner as required.
2. Reuse parser instances and compile query packs once per grammar/release. Keep trees/nodes within the adapter. Use `TreeCursor` for structural traversal and query captures for bounded feature extraction; configure query limits/cancellation and turn match exhaustion into coverage remainder.
3. Parse Python with recoverable Ruff `parse_unchecked`, retaining the complete parsed result, errors and unsupported-syntax diagnostics. Build `LineIndex`, token/trivia/index structures once per source revision. Construct semantic scopes/bindings through actual traversal; constructing `SemanticModel` alone does not analyze a file.
4. Establish byte offsets as the join coordinate. Convert through the pinned source bytes to UTF-8/UTF-16/line-column presentation; cover Unicode, CRLF, empty/end-of-file spans and missing final newline. Keep undecodable/binary/oversized/excluded inputs explicit. Source context uses lossless text or an authorized byte/base64 variant.
5. Preserve raw/reversible path bytes, comparison keys, display names and URIs separately. Extend the current compiler UTF-8 manifest limitation with an explicit byte-safe wire representation where supported; never substitute lossy path strings for identity. Test case-folding collisions and ambiguous rename continuity.

**Acceptance:** real `.py`, `.pyi` and `.rs` inputs produce correctly located facts, including incomplete edits. Provider disagreements/unmappable ranges remain diagnostics/ambiguity. Rust syntax continues to update when compilation fails.

### 4D. Canonical two-language normalization and authority

**Current status — partial, committed.** Native DataFusion constructs captured source, Python/Rust entities/declarations, function selectors, Python lexical references and both languages' call occurrences. Exact Python checker anchors and Rust stable keys resolve selected targets; unknown and unmapped calls remain explicit. Schema nullability refinement and exact ID/storage restoration work. Imports/exports, semantic references, structural types/propositions, members/signatures, complete dispatch/instances, module/lambda entities, external/generated endpoints and edit-time identity/authority remain. The validated call selector is a query projection, not completion of these families.

**Surfaces:** `src/production_provider_recipe.rs`, `src/provider_admission.rs`, `src/programmatic_derived_analysis.rs`, `src/schema_contract.rs`, programmatic relation builders and startup publication.

1. Route raw source/syntax, Pyrefly and rustc outputs into typed canonical entity, occurrence, declaration, binding, reference, import/export, type, member, callable, call-site and dispatch relations. Preserve raw relations alongside normalized facts.
2. Join ranges on `(file_id, content_digest, start_byte, end_byte)` and semantic role. Use exact matches first; a permitted containment/overlap rule records method and ambiguity. Never match unrelated declarations just because their ranges coincide.
3. Apply GEN §5 authority separately per fact proposition: syntax occurrence, binding, declared type, inferred type, call candidate, resolved target and compiler instance are different facts. Retain conflicting evidence; emit a candidate set/unknown when resolution is not justified.
4. Build canonical IDs using the released application recipes. Keep occurrence/entity/call-site/type/executable-instance distinctions and context dependence. Include external endpoints with exact ecosystem identity when available; unavailable bodies remain explicit.
   Complete ONT's closed type discriminants, including literal/never/any, unions/intersections where supported, callable/overload, ParamSpec/variadic tuple, generic parameters/applications, pointer/reference, array/slice, function item/pointer, trait object/projection/opaque and native escape/error variants. Normalize structure rather than making a provider's display string or local hash the application identity.
5. Use `LogicalPlanBuilder`, typed expressions, equijoins, anti/semi joins where valid, projection and aggregate operators. Feed immutable request/provider relations directly into the installed catalog. Avoid a generic serialized transformation interpreter or provider-specific row-map processing framework.
6. Preserve `SchemaContract` logical/physical mapping: fixed-width IDs, nullability, metadata, qualified fields and storage casts. Validate actual stream/batch and sink boundaries; Delta `BINARY` round-trips must restore the logical ID type. Do not validate by running every transformation twice.
7. Replace coarse all-or-nothing family availability with actual coverage in 5A. A valid empty completed partition and a missing partition must produce different coverage even when both have zero fact rows.

**Acceptance:** independently expected declarations/imports/references/types/calls from both languages survive Arrow → Delta → exact reopen → query. Tests check actual IDs/relationships/positions and uncertainty, not row-count positivity or matching hashes.

### 4E. First four production query forms and mixed-language demonstration

**Current status — partial.** Installed clients exercise canonical function and selected additional declaration-kind FindEntities and exact-ID declaration RetrieveFacts, including repeated subjects, scoped failure, empty results and truncation. Installed FollowRelationships queries execute one-step Python/Rust calls and exact Python reopen. The continuation adds explicit Python lexical-reference traversal, preserving write/read/call/type/import kinds, reusable occurrence endpoints and unresolved/unsupported family scope; project-aware semantic references and full traversal remain open. SourceContext serves exact canonical declaration spans with live independent disclosure checks, lossless byte limits and coordinates. Function definitions/bodies now use exact syntax-owner joins and an independent incomplete-owner processing family; the mixed live/clean behavioral case passes with independent expected source, nested functions, CRLF/Unicode truncation, edits and restoration (206.25 s). `function-source-live` selects it. Explicit surrounding-line windows now preserve captured CRLF, Unicode, file edges and separate anchor/requested/delivered ranges. Installed clients and exact reopen pass (33.70 s), including a non-retryable source hard-limit failure versus explicit prefix truncation; `source-lines-live` selects the case. Broader syntax context and source subject selection remain open. All broader meanings, source/representation scopes, semantic reference resolution, stop/filter/distance behavior and composition remain required. The first-four acceptance below is open.

**Prerequisites:** 4D and 5A–5B. **Surfaces:** `src/production_query_recipe.rs`, `src/relational_semantic_query.rs`, `src/query_service.rs`, `src/semantic_query_contract.rs`, existing child catalog and adapter.

Implement compositional typed plans for:

| Form | Concrete realization | Required demonstration |
|---|---|---|
| FindEntities | Authorized canonical entity/occurrence selection with representation/context filters; deterministic resolution of names and kinds | Find typed Python and Rust functions, first-class calls and explicitly requested syntax; ambiguous names return interpretations |
| FollowRelationships | Indexed/native joins for one/fixed steps; direction/stop conditions and bounded recursive expansion where needed | References, imports and resolved/candidate calls in both directions, without collapsing distinct call sites |
| RetrieveFacts | Family-selected joins, properties, provenance and point/context filters; expanded family scope for broad requests | Parameter/return types, members, call resolution and unknown reasons from actual semantic providers |
| RetrieveSourceContext | Exact snapshot source descriptors and independent disclosure authorization; byte/line bounds and syntax joins | Correct source span after current disk changes, Unicode positions and exact omitted bytes on truncation |

FindEntities now uses canonical declaration semantics, including Python class/parameter/binding/import/type-alias/type-parameter and Rust constant/static queries. New results carry reusable public entity IDs; installed clients use them for declaration fact retrieval. Guard choices have readable labels without changing their submitted opaque identities. Retain that path and extend its remaining kinds/scopes. Reuse the existing form/request infrastructure but remove assumptions that a form can exist only when every producer is globally complete. Unsupported semantic meanings must yield a typed gap, never a syntax/name fallback.

Connect `tests/fixtures/pragmatic_cpg/expectations.json` to `tests/integration/daemon.rs` and the modern client driver. Use the existing registered-supervisor fixture and installed provider binaries. Check at least one real Pyrefly and one real rustc semantic result through the public adapter, plus partial and empty cases. This closes the static mixed-language vertical, while the first useful release still awaits outcome 6.

## 5. Outcome 5: query-relevant coverage, freshness and unfinished scope

### 5A. One processing authority with a query dependency scope

**Current status — partial.** Committed requested file/target and run/family relations drive canonical function/declaration query summaries independently of fact rows. Language/context filtering, reason categories and bounded public remainder continuation work. The daemon resumes an accepted query ID after restart, reads its retained exact Delta processing selection and preserves the original snapshot/counts through subsequent workspace repair; the 130-partition installed case passes with invalid block/range/released-resource checks (53.38 s). Validated call-family scope adds semantic gaps and conservative context-wide potential callers; real implicit-property/decorator and public call cases pass. Lexical-reference scope now includes requested Ruff reference partitions, unresolved/candidate target gaps and explicitly unsupported Rust references. The committed caller-scope slice adds explicit outgoing Rust caller partitions, requiring admitted MIR and exact source ownership; incoming and unmatched subjects retain conservative target scope. The installed live/clean comparison with typed caller IDs and exact reopen passes (152.97 s) for direct and known-empty callers versus unresolved build-script calls. Failed-target public queries pass (67.73 s), and retained processing pages pass (56.71 s). Six focused recipe/selection/continuation cases, all 109 adapter and 210 tooling tests, default/featureless root checks and full governance pass. Affected Clippy retains its 955/36-warning baseline with no new code/file diagnostics. Remaining: all-family/owner/dependency/frontier scope, authorization-scoped efficient scans, live transitions, precision/next actions and a terminal-versus-runnable distinction. Do not turn semantic unknowns into endlessly pending jobs.

**Surfaces:** provider admission/input observations, processing relations, `src/fabric/production_workspace_startup/input_observations.rs`, query planning/status and lifecycle coordinator.

1. Represent requested work as scopes over workspace, language, context, source generation, file/owner and family. Reuse existing coverage relations; extend them only where actual consumers need a dimension. Do not derive requested scope from emitted rows or installed provider support.
2. Track pending, running, completed, failed, cancelled, unsupported and intentionally excluded work. Keep partial execution, unresolved semantics, not-applicable work and output truncation distinguishable. Terminal job state and complete semantic knowledge are orthogonal: a successfully completed dynamic analysis may still contain unknown targets.
3. Include reason categories, affected scope, algorithm/provider precision, input generation and useful retry/next action. A provider's terminal message establishes only its requested partition, after schema/context/owner checks. Missing terminal output does not mean completion.
4. Derive each query's dependency scope from the selected relations/semantics. For incoming references/callers, include unprocessed potential referring scopes, not just the target owner. For imports, include unresolved/negative dependency search scopes. For traversal, propagate unknown frontier scope. If exact dependencies are unavailable, use a conservative context-wide set and say why.
5. Summarize counts without double-counting overlapping owner/file/context scopes; paginate detailed remainder. Keep status scans indexed/aggregated so a query does not serialize millions of coverage units. Authorization filters scope before counts or reasons can disclose hidden files.
6. Keep installed support, snapshot processing coverage and development/test confidence separate. Only the first two belong to normal runtime status. Reuse one coordinator-owned state for queries and status projections.

**Acceptance:** known-empty, missing output, parse/type/compile failure, timeout, cancellation, denied scope, unsupported context, dynamic ambiguity and limited execution all yield distinct truthful responses. A references query warns about a pending potential importer even if the target file is complete.

### 5B. Snapshot freshness and wire projection

**Current status — partial, with a live whole-workspace barrier.** Typed Protobuf/Pydantic processing,
retained manifest/reopen and presence-safe truncation are implemented. Strict policies now request a
secure source census and await an exact successor; racing selection retries within a deadline.
Best-available snapshots retain actual freshness. Snapshot freshness/context and separate live workspace
watermarks/watch health/rescan/runnable state cross the typed wire. Retained old source pages pass
across live publication. Target/family-specific barriers, historical query selectors
and all-family terminal convergence remain open. Public typed remainder pages now retain their
original query selection across updates and exact restart; old results without selection metadata
remain readable without acquiring a new continuation. Source-current now selects a durably staged
source/syntax epoch while semantic work remains pending; the other strict policies conservatively
await the full provider pass. Status distinguishes source freshness and selected semantic pending.

**Surfaces:** `src/query_service.rs`, snapshot selection, `src/semantic_query_contract.rs`, released Protobuf files under `contracts/`, generated Rust/Python clients, adapter DTOs/status tools.

1. Implement all QRY §2.3 policies against real processing state: `best_available_snapshot`, `await_latest`, `require_current_for_targets`, `require_source_current`, `require_semantic_current`. Apply the freshness barrier before semantic resolution, then pin one immutable epoch for all request blocks, source reads and resources.
2. Every response identifies snapshot, source/context revision, resolved requested universe, completed and remaining scope, precision and freshness. Strict current policies return a current selection or typed deadline/availability failure. Only explicit best-available/historical semantics may serve older material and must label it.
3. When newer source arrives, do not mutate a pinned response's generation or coverage. Report newer workspace processing separately with its own observation/generation. Withdraw invalidated facts from current snapshot views; never mark an old compiler result current to fill a gap.
4. Distinguish complete absence, no match in the observed subset, unavailable family, partial result and truncation. Negative facts/clauses require complete authority for the exact authorized scope and sufficient semantic precision; unknown targets cannot support a universal negative.
5. Add only missing released fields. Preserve field numbers, reserve retired fields/names, use explicit `optional`/`oneof` presence and typed messages, and regenerate through `just proto-gen`. Do not stuff coverage into untyped `Struct` or opaque JSON merely to avoid schema evolution.
6. Implement the corpus convergence adapter now: wait until the requested generation/context/families have no runnable pending work and have declared terminal coverage, then separately assert the scenario's expected completeness. A failed compilation can be quiescent without semantic completeness. Use a monotonic deadline and observable state transition, not a fixed sleep.

**Acceptance:** actual service responses preserve scope through pagination, historical selection, compile failure/repair and deadline expiration. Cross-language wire tests exercise absent versus explicitly set values and terminal/remainder alternatives.

## 6. Outcome 6: continuous updates and quiet convergence

### 6A. Watcher, repository observations and authoritative reconciliation

**Current status — partial.** The daemon installs a native notify watcher before census, owns its
blocking lifetime, coalesces callbacks through a bounded queue, retains a rescan watermark and runs
periodic secure reconciliation. Public status exposes watch health and source observations. Installed
Python replacement/addition/deletion/atomic save and exact reopen pass. Selected external roots,
Git inclusion, excluded native watch topology, explicit polling and root/config recovery remain open.

**Surfaces:** `src/source_image/`, source/context preparation, supervisor/daemon ownership, `src/fabric/source_wave_command_effect.rs`; add a focused watcher/coordinator module within the existing stable package as needed.

1. Own one `notify-debouncer-full` watcher per workspace input topology. Install watches before initial census, buffer changes during capture, then reconcile changes observed across the capture fence. Include source/config roots and selected external input roots; exclude generated caches, Delta tables and build outputs by explicit policy.
2. Use `new_debouncer`/`new_debouncer_opt`, recursive root registration, normalized rename events and the debouncer's file-ID cache as hints. In its callback only classify/enqueue lightweight events; never parse, read large files, run Git status or block on a full queue.
3. Bridge with bounded Tokio `mpsc` and `try_send`. On queue overflow, watcher error or `need_rescan()`, set a retained reconciliation-required flag and wake the coordinator. Clearing a queue must not clear the obligation to rescan. Coalesce dirty paths and promote directory/root changes to a bounded subtree/root census.
4. Handle atomic save, rename-over-target, paired/unpaired rename, deletion/recreation, root disappearance, case collisions, ignore-boundary transitions and watcher reinstallation. A rename is continuity evidence, not permission to invent canonical identity continuity.
5. Use gix read-only discovery/index/status/ignore/attributes/directory walking to accelerate present-state inventory and inclusion. Use worker-local `Repository` handles or a `ThreadSafeRepository` converted locally; do not share `Arc<Repository>` as if it were `Sync`. Account linked-worktree git/common dirs, unborn repositories, conflict stages, nested repositories and selected submodule boundaries.
6. Reread bytes through the authoritative descriptor-relative capture path. Stat, Git index/OID, watcher events and rename similarity cannot establish current content by themselves. Disable external filters/helpers and Git writes/network in this observation path.
7. Expose watcher health and rescan state; use an explicit `PollWatcher` profile for unsuitable filesystems. Add periodic lightweight reconciliation so a lost event cannot leave the graph permanently stale.

**Acceptance:** a real running daemon observes edits without restart; forced queue loss/rescan, atomic save, root recreation and ignore/config changes reach correct source state. Tests wait on state, not an assumed debounce delay.

### 6B. Conservative invalidation and owner replacement

**Current status — partial.** Monotonic source generations, whole-context replacement and immediate
stale observation are wired into live updates. Providers and activation reuse exact source/context
pins; new events cancel or invalidate older candidate work. Python edit/delete/atomic-save scenarios
pass. Mixed Python/Rust call-target edits and compiler failure/repair now match independent clean
state for the four implemented query forms. Deterministically paused completed providers are rejected
after a newer edit; exact pending-stage restart resumes the same generation. Broader configuration,
negative-dependency and context acceptance remain open.

**Surfaces:** source/context relations, provider/analysis scheduling, owner-replacement normalization and activation inputs.

1. Assign monotonic source generations and immutable context revisions. Capture the complete changed input set, including deleted owners and configuration/dependency changes. Publish processing invalidation immediately enough that current-required queries cannot serve known-obsolete semantics.
2. Start with owner-local syntax replacement and context-wide semantic invalidation when dependency closure is uncertain. Include reverse imports/calls/summary dependents where known; include negative dependencies such as a previously missing module becoming available.
3. Track additions, removals and replacements explicitly. Delete all facts owned by a replaced/deleted body, affected endpoint edges and dependent summaries; repair external/cross-owner references through recomputation. No range translation may reuse program points across different body content digests.
4. Fence queued work, provider output, derived output and activation on source/context identity. Reject an obsolete completion before it can replace newer facts, even if its compiler process succeeded. Coalesce repeated pending work by effective context, preserving the latest requested generation.
5. Retain immutable facts for existing leased snapshots. Reuse unchanged owners only when their source/context/dependency inputs are still valid for the new snapshot; do not merely relabel their generation. Retain the observation generation and validity selection distinctly.
6. Bound dependency indexes and keep an explicit full-context fallback. Fine-grained invalidation is an optimization to be introduced after clean/incremental correctness, not a prerequisite for the live product.

**Acceptance:** deletion removes old declarations and edges; rename respects identity rules; changed imports/features/stubs invalidate affected answers; a delayed older provider result cannot overwrite a repaired newer generation.

### 6C. Two-speed publication and retained provider state

**Current status — partial orchestration reuse.** Startup capture/providers/normalization/publication
now serve serialized updates, and old source pages remain readable during successor publication.
Epoch retirement is separate from workspace admission shutdown. Live updates now publish source/syntax
with pending checker/named compiler target scope, then a semantic successor at the same generation.
A durable stage marker supports exact pending-stage reopen. Source-current queries use a distinct
barrier; terminal semantic failures remain incomplete without endless pending work. The deterministic mixed Python/Rust pause/deadline/obsolete-completion/restart case passes
(168.05 s), including named pending Cargo targets. Capture bytes/leases outlive the short operational
writer, allowing censuses during semantic work. Fresh startup now uses the same source-first publication and semantic-resumption path. The final installed initial-source/pending-Rust/deadline/obsolete/restart case passes (186.06 s); durable readiness passes (8.85 s). Installed semantic serving/restart, 70-module Python extraction, Python calls, failed Rust targets, guard delivery, Cargo selections with clean/reopen, and retained processing-page regressions also pass. Semantic assertions select the exact semantic successor; source-only assertions retain pending scope. Default/featureless checks, full governance, all 216 tooling tests and changed-file formatting pass; Clippy keeps its 952/36-warning code/file baseline. Retained checker/parser/Cargo state,
unchanged-version reuse, selective persistence and update scheduling remain open.

**Surfaces:** source-wave commands, candidate/activation builders, `src/fabric/production_workspace_startup.rs` reused as shared preparation/composition helpers, provider services and runtime scheduler.

1. Refactor startup-only orchestration into reusable capture → schedule → normalize → publish operations, retaining existing ownership and reopen behavior. Avoid a separate implementation of semantic rules for updates.
2. Publish a coherent source/syntax snapshot with semantic pending/remainder after invalidation, then a successor with completed semantic/derived partitions. Select one exact relation-version vector and coverage set per epoch. Readers must never independently select each table's latest version.
3. Reuse unchanged exact versions; stage only changed owner partitions/relations. A candidate that loses its generation/writer fence is abandoned and cleaned up through ordinary ownership. A write with uncertain acknowledgement is reconciled before retry or deletion.
4. For Tree-sitter, apply `InputEdit` to the retained old tree and parse the new authoritative bytes with that tree; use `changed_ranges` to narrow syntax work. Preserve text/digest and enclosing-owner changes even when tree structure is unchanged. Fall back to full parsing if an edit cannot be safely reconstructed. Ruff reparses the changed Python file; it is not an incremental parser.
5. Keep one Pyrefly checker state per effective context where resource limits allow. Use `Query::change_files`/categorized events and the pinned state invalidation machinery, then requery affected files. If the affected set is incomplete, recheck the context. Serialize mutation/extraction against checker revision; never answer from a half-updated checker.
6. Reuse Cargo incremental target state per compatible Rust context and immutable dependency bundle. Supervise long-lived sidecar services, and bound context caches by cost/last use. Dropping a context cancels and joins its work; eviction cannot alter pinned facts.
7. Prioritize source/status/control and interactive target work while allowing background semantic convergence. Bound burst coalescing with a maximum wait so continuous edits do not starve every publication. Respect shared CPU/memory headroom across providers, DataFusion and graph work.

**Acceptance:** syntax can become current while compiler facts remain explicitly pending; semantic convergence produces the expected successor; an old query continues to read its exact source/facts during multiple updates.

### 6D. Real incremental-versus-clean corpus

**Current status — partial live acceptance.** An installed-client persistent Python daemon case uses
actual strict freshness waits for edits, additions, deletion, atomic save and reopen; the retained source
page case spans a live successor. `mixed-clean-live` keeps one daemon running through Python/Rust
call-target edits, Rust compilation failure and repair while independent clean daemons use separate
state and provider caches. Four-form comparisons preserve canonical identity/relationships, facts,
positions, precision, order, coverage and source bytes; operational generations/provider runs and
the explicitly snapshot-bound source-context handle differ. Independent names/call pairs and the
repaired-to-original comparison pass (259.56 s, 2026-09-09). The broader edit corpus, all-family
coverage and Rust/external configuration/dependency cases remain open. Python version/platform changes, negative import creation/deletion, unsupported configuration and exact restoration now pass against independent clean state through public declarations/calls (165.86 s); `python-context-live` selects that case. Namespace-package source/stub precedence, deletion/recreation and exact target-owner identities also match independent clean state (72.87 s). Configured root-order reversal and restoration also match clean public declaration/call results while preserving source files outside the import roots (73.85 s). A separate source-stage case deterministically holds a completed provider candidate,
rejects it after a newer edit, and resumes a pending generation after exact restart; mixed Python/Rust
target coverage passes (168.05 s). The final two-stage clean comparison passes (294.29 s), as do the
full Python edit sequence (114.60 s) and retained source-page checks (41.57 s), within 43 affected tests.

**Surfaces:** `tooling/product/corpus.py`, `edits.json`, daemon integration fixture, modern driver and golden case selection.

Implement runtime adapters that start one persistent incremental daemon, capture semantic responses, and build the same edited source in an independent clean state root for comparison. The clean rebuild must not destroy or restart the incremental daemon. Use the same effective contexts and explicit source-identity rules.

Compare identities and their relationships, facts, positions, deterministic ordering, precision and coverage. Exclude only documented operational differences such as elapsed time, operation IDs and independently allocated snapshot IDs; do not normalize away semantic identity drift. For rename continuity allowed to differ by reconstruction, compare the released identity relation/continuity semantics explicitly.

Extend edits to Python/Rust declaration and call changes, negative imports, delete/recreate, rename, config/features/stubs, malformed syntax, compile failure/repair, rapid consecutive edits and cross-file changes. Add a deterministic delayed-completion seam at the provider/publication boundary for the otherwise unreliable race test.

**First useful release is complete when:** all four public forms answer real mixed-language semantics, queries explain unfinished scope, one running daemon converges after edits, and exact persisted state reopens. Full outcome 7/8 remains open.

## 7. Outcome 7: all fact families and all query behavior

### 7.1 Coverage map for the complete target

Each row includes its raw observations, canonical properties/relationships, derived facts where applicable, explicit unknowns, public retrieval and owner/context invalidation. The map covers ONT's semantic sections and GEN's generation/relationship sections; it does not replace those specifications with a smaller vocabulary.

| ONT section and title | Required implementation, including language specialization | Owning slices | Current implementation boundary |
|---|---|---|---|
| §5 Source and lexical ontology; §6 Syntax ontology | Files, bytes/ranges/lines, tokens/comments/trivia, syntax structure, raw/normalized kinds, malformed/generated input and coordinate mappings | 4C, 7A, 7D | Partial: real Python/Rust syntax; full lexical/coordinates and live replacement open |
| §7 Semantic identity ontology; §8 Scope, binding, and name-resolution ontology | Declarations, symbols, definitions/references, lexical owners, qualified identity, overload/candidate sets, local/global/nonlocal scopes and Rust namespaces | 4D, 7A, 7D | Partial: canonical declarations and Python lexical references; semantic scopes/references open |
| §9 Module, import, export, and dependency ontology | Packages/modules/crates, aliases/re-exports/globs, dependencies, external endpoints, positive and negative resolution inputs | 4A–4B, 7A, 7D | Partial captured Cargo inputs; canonical modules/imports/exports and external roots open |
| §10 Type ontology; §35 Python type ontology extensions; §47 Rust type ontology extensions | Canonical structural type algebra; declared/computed/expected/narrowed propositions; generics, unions, callable types, traits/projections and unknown/error forms | 4B/4D, 7A, 7D | Raw semantic inputs only; complete canonical type algebra/propositions open |
| §11 Member and object-model ontology; §36 Python object-model ontology | Fields/properties/descriptors, inheritance/MRO/protocols, visibility, overrides, Rust impl/trait items and associated members | 7A, 7D | Selected provider observations only; complete canonical/public object model open |
| §12 Callable contract ontology; §13 Call-site ontology; §14 Dispatch ontology | Signatures, defaults/argument binding, receiver, call occurrence, resolved/possible/unknown target, callable value and executable instance distinctions | 4D, 7A, 7D | Partial: actual canonical calls; full contracts/dispatch and public traversal open |
| §15 Control-flow ontology; §16 Derived control-flow facts | Normal/exception/cleanup/unwind/suspend edges, entry/exits, reachability, dominance/post-dominance, control dependence and loops | 7B, 7E, 7F | Existing algorithms/raw inputs; real complete CFG/derived delivery open |
| §17 Value and computation ontology; §18 Definition/use and dataflow ontology | Evaluation order, temporaries/constants/operators, definitions/uses, reaching definitions, liveness and value/data dependence | 7B, 7E | Existing algorithms/raw inputs; real complete value/dataflow delivery open |
| §19 Abstract memory and state-location ontology; §20 Alias and points-to ontology | Variables/fields/elements/allocations/unknown locations, addresses, reads/writes/moves/copies/borrows, may/must alias and points-to under declared precision | 7C, 7E | Open production abstraction/analysis/query path |
| §21 Program-point state ontology | Entry/exit/before/after states tied to exact owner/body and source revision, joins and unknown states | 7B–7C, 7E | Open production state/precision/query path |
| §22 Effect ontology | Direct effects separated from propagated summaries; unknown calls imply unknown effects | 7C, 7E, 7F | Open direct/propagated effect delivery |
| §23 Exceptional-flow ontology | Python raise/handlers/exception groups/finally; Rust unwind/cleanup; normal and exceptional successors kept distinct | 7B–7C, 7E | Open complete exceptional-flow delivery |
| §24 Resource-lifetime ontology | Acquire/release/transfer/escape, exceptional cleanup, Python context managers and Rust Drop; no leak/safety verdicts | 7C, 7E, 7F | Open semantic resource-lifetime delivery |
| §25 Async and concurrency ontology | Suspension/resumption, spawn/join, lock/channel and semantics-backed possible happens-before, with no runtime ordering claims | 7C, 7E, 7F | Open static async/concurrency delivery |
| §26 Closure and capture ontology | Free/cell variables, capture mode/environment, escapes and source/lowering correspondence | 7A/7C, 7D/7E | Open complete capture/environment/escape delivery |
| §27 Generated and lowered-code ontology; §28 Generic and specialization ontology | Expansion/desugaring/synthesized nodes, provenance, monomorphized instances, shims/glue and explicit unmappable/generated spans | 7A, 7D–7E | Partial unmapped-call reasons; generated/lowered/generic normalization open |
| §29 Objective graph-analysis facts; §30 Objective structural metrics | Typed graph projections, SCC, connectivity/reachability, justified closure/reduction, degree/count/nesting distributions; no quality scores | 7F–7G | Existing graph helpers; production projections/analyses/query delivery open |
| §31 Interprocedural summary ontology; §32 Explicit unknown ontology | Bounded SCC fixpoints, effects/resources/value summaries, convergence/precision/unknown propagation and dependency invalidation | 5A, 7F | Partial scope/unknown handling; interprocedural summaries and full precision open |
| §33 Python scope ontology; §34 Python binding ontology | Module/class/function/comprehension/type-parameter scopes, rebinding/shadowing, assignment/destructuring, global/nonlocal, deletion and definition ownership | 7A–7B | Partial Ruff binding/reference normalization; full scope/evaluation semantics open |
| §37 Python call ontology; §38 Python dynamic-semantics facts | Constructor/bound-method/overload/descriptor calls, dynamic attribute/import/exec/eval uncertainty, runtime mutation boundaries | 7A–7C | Partial checker-selected call targets; full dynamic/call semantics open |
| §39 Python decorator ontology; §40 Python pattern-matching ontology; §41 Python comprehension ontology | Decorator evaluation/application order, source/transformed correspondence, pattern bindings/guards and comprehension/generator scope/evaluation | 7A–7C | Raw syntax prerequisites; canonical/evaluation/derived delivery open |
| §42 Python context-manager ontology; §43 Python async and generator ontology | Enter/exit/suppression, async enter/exit, yield/yield-from/await, exceptional resumption and cleanup/capture | 7B–7C | Open complete context-manager/async/generator semantics |
| §44 Rust source-semantic entities; §45 Rust declaration properties; §46 Rust generic ontology | Items/visibility/attributes/cfg, modules, impl/trait members, parameters/bounds/associated items and substitutions | 7D | Partial stable item/declaration normalization; full Rust semantic properties open |
| §48 Rust MIR ontology; §49 Rust place and projection ontology; §50 Rust MIR state-transition ontology | Typed bodies/blocks/statements/terminators, locals/place projections, storage transitions, calls/assert/switch/drop/return/unwind, moves/copies and aggregate/rvalue semantics | 7D–7E | Raw typed compiler foundation; complete canonical/MIR-derived delivery open |
| §51 Rust ownership and borrow ontology | Public typed accesses plus separate exact private loan/region evidence and conservative derived ownership/initialization/liveness states | 7E | Open exact private borrow and complete derived ownership delivery |
| §52 Rust call and executable-instance ontology; §53 Rust trait and dynamic-dispatch ontology | Direct/virtual/indirect calls, specialization, function-item values versus calls, trait selection, vtable candidates and unknown targets | 7D–7F | Partial direct/indirect canonical calls; instances/traits/virtual dispatch open |
| §54 Rust macro ontology; §55 Rust drop and destruction ontology; §56 Rust async and coroutine-lowering ontology | Invocation/expansion/hygiene mappings, drop glue/flags/cleanup, coroutine states/suspension/resumption and source correspondence | 7D–7E | Partial explicit unmapped macro calls; full macro/drop/coroutine delivery open |
| §57 Rust unsafe and FFI ontology; §58 Rust constants, statics, and CTFE ontology | Unsafe/ABI/raw-pointer/static facts, evidence-qualified cross-language links, const evaluation outcomes/errors and inline assembly boundaries | 7D–7F | Raw compiler foundation; complete canonical CTFE/unsafe/FFI delivery open |
| §§61–67 metadata, evidence, ownership, identity, separation, unknowns and non-evaluative rules; AC-G-12–18/70–77 | Apply across every family, including external/body policy, path encoding, precision, schema mapping, negative facts, summaries and static concurrency | All slices | Identity/pinning/unknown foundations in use; all-family/external/update closure open |

GEN §§67–79 relationship generation is included in the corresponding rows, including the lettered source/object/exception/resource/async/capture/program-state additions. A family is not delivered if its raw producer exists but its canonical relationship or public projection is missing.

### 7A. Complete Python language semantics

**Current status — open beyond selected prerequisites.** Real lexical bindings/references, callable syntax and checker-selected call targets now feed canonical relations. The complete scope/import/type/member/decorator/pattern/comprehension/dynamic census and its public/update consumers remain; implicit properties/decorators and module/lambda caller entities are known normalization gaps.

**Surfaces:** native Ruff adapter, Pyrefly sidecar, canonical normalization and `src/programmatic_derived_analysis.rs`.

Use Ruff's typed AST, scopes/bindings/reference machinery, trivia/index and source utilities for their actual authority. Complete global/nonlocal, rebinding, deletion, class versus function lookup, comprehensions/generators, pattern binding/guards, imports/re-exports/star imports, PEP 695 parameters and callable defaults/decorators. Evaluation order is not the AST's generic source-order traversal.

Use Pyrefly's type table/member/callee/resolver seams for inferred types, MRO/descriptors/properties, protocols/generics, overloads and dispatch. Preserve declared/computed/expected/narrowed propositions independently. Model decorators as source call/application facts and evidence-backed transformed semantics; arbitrary decorators do not justify claiming the original callable signature survives unchanged.

Emit dynamic `getattr`, monkey-patching, dynamic imports, `exec`/`eval`, star expansion and callable objects with candidate/unknown precision. Querying Python source must not execute it. Give unresolved targets and unavailable external bodies explicit edges/remainders so downstream effects and negative queries stay conservative.

**Acceptance:** add source-derived cases for every language row in the coverage map, including class-scope traps, comprehensions, match guards, descriptors, overloads, decorated functions and missing exports. Exercise accepted canonical relations and public facts, not only provider DTOs.

### 7B. Correct Python CFG, evaluation order and core dataflow

**Current status — open.** Existing typed analysis code and `analysis_cases.json` are reusable. The real owner-scoped CFG/evaluation builder and production input wiring remain; ordinal/sequential approximations are not complete Python control semantics. No full source-to-public CFG/dataflow acceptance is claimed.

**Surfaces:** `src/python_derived_analysis.rs`, native syntax-to-analysis seeds, `src/programmatic_derived_analysis.rs`.

1. Replace ordinal/sequential flow as the default semantic construction with an explicit owner-scoped CFG builder. Reuse typed Ruff traversal but implement application-owned Python control semantics: conditionals, short-circuit booleans, conditional expressions, loops/else, break/continue, return/raise, try/except/except*/else/finally, with/async-with, match guards and suspension.
2. Distinguish definition-time evaluation from nested function/class body execution. Split normal, exceptional, cleanup and suspend/resume successors. Do not append fallthrough after terminating statements. Assign program points to actual evaluation events, including RHS-before-binding and chained/destructuring assignment.
3. Emit definitions, reads, kills and value computation before deriving flow. Reaching definitions is a forward may analysis with union joins; liveness is backward with successor union and use/def transfer. Preserve branch/loop fixpoints rather than connecting each use to the latest textual assignment.
4. Use native DataFusion joins/set operations for bulk extraction and relation construction. Use a bounded owner-local worklist when iterative transfer requires it; do not rebuild an entire global DataFusion session for each block iteration. State lattice, transfer, join and termination conditions in the implementation next to the algorithm.
5. Retain explicit unreachable points without inventing reachable flow. Partial parse/control structure or missing binding input qualifies dependent analyses. Support program-point before/after/entry/exit retrieval through RetrieveFacts.

**Acceptance:** activate the existing branch/loop/return/nested-callable expectations in `analysis_cases.json`. Add short-circuit, finally-overrides-return, exception group, loop-else, context-manager suppression and comprehension cases. Compare small graphs against an independent simple reference solver where it exposes errors the implementation could otherwise mirror.

### 7C. Python memory, effects, resources, exceptions and concurrency

**Current status — open.** Existing structures do not establish production memory/effect/resource/exception/capture/async/concurrency analysis. Implement the finite abstraction and real input/query/update path after 7B, with declared precision and unknown propagation.

**Prerequisite:** 7B's actual CFG/dataflow. **Surfaces:** Python analysis modules, canonical derived relations and common summary inputs.

Implement a finite abstraction with lexical locations and allocation-site objects; retain known field/member selectors, use an unknown selector for dynamic access, and distinguish known constant indices from summary elements. Start context-insensitive/path-insensitive where required precision permits, and label that choice. Maintain a separate conservative unknown location. Do not emit must-alias from a may-analysis or strong updates for non-singleton targets.

Derive reads/writes/escapes/points-to and before/after program state; model exception paths and resource acquisition/release/transfer through context managers and explicit semantic library models. Named models describe static behavior with version/applicability conditions; unmatched calls propagate unknown effect/resource facts. Models do not execute user imports.

Add closure/cell/environment capture, generator/async state, await/yield/yield-from, async iteration and cancellation cleanup. Record source-backed spawn/join/lock/channel operations and possible ordering only where language/library semantics support it. Keep unknown scheduling, missing bodies and dynamic resource protocols visible. Do not emit runtime order, leak verdicts or refactoring judgments.

**Acceptance:** alias through two references, dynamic attribute/index, closure mutation, exceptional release, exit suppression, async context cleanup, generator suspension and unknown external calls. Observe the distinction between direct effects and propagated summaries and between possible and definite facts.

### 7D. Complete typed Rust source, type, instance and lowering facts

**Current status — partial raw foundation; complete slice open.** Contained compiler publication, exact multi-file owners, stable declaration keys and selected direct/indirect/macro/dependency call normalization work. Full typed source/type/generic/trait/instance/MIR payload coverage, generated/hygiene/coroutine/CTFE/FFI mapping and canonical/public diagnostic consumers remain to be completed against actual compiled fixtures. The 4A continuation retains native primary diagnostic messages under ordinary compiler failure with exact receipt/source binding; failed-no-MIR and mixed-target persisted scenarios pass (8.12/76.48 s), with live/clean repair and exact failed-epoch reopening (374.04 s). The detail slice in `4cc74d7c` adds ordinary native child notes, labeled spans, suggestion alternatives and multipart edits. Its contained successful/failed cases pass original BOM/CRLF ranges and separate second-file identity/digest (7.80/7.71 s in the final selection). Twenty strict extractor tests pass, including remapped paths and unknown-versus-changed source handling. Full root checks/governance pass with the same 988 Clippy warnings; the expanded installed live/clean/repair/exact-reopen scenario passes (350.01 s), as does all-failed startup (58.77 s). All four final native cases pass. Older-provider-bundle upgrade migration was not tested. Separate future-breakage report semantics, broader generated/hygiene mapping and canonical/public diagnostic consumers remain open.

**Surfaces:** `rustc-extractor/src/`, `src/rustc_relation_schema.rs`, `src/rustc_service.rs`, source mapping and Rust normalization.

1. Enumerate supported selected-crate items and bodies through typed `rustc_public`; emit declarations/properties, types, generics/substitutions/bounds, traits/impls, associated items and executable instances. Keep compiler-local handles inside the extraction run and map stable compiler keys through application recipes.
2. Walk every required statement, terminator, operand, rvalue and place projection exhaustively. Preserve raw variants and payloads alongside normalized forms, including body phase, source scopes, promoted constants and compiler-only/false edges. A semantic CFG projection declares how such edges differ from executable successors. A new/unhandled nightly variant becomes explicit remainder with location, not a debug-string parser or silent skip.
3. Separate direct function/instance calls, virtual dispatch, function-pointer calls and executable uses that are not calls. Retain call-site arguments/receiver/return/unwind, shims, intrinsics, drop glue and available specialization correspondence.
4. Complete macro invocation/expansion/source-map/hygiene information through the narrow private adapter. Handle generated/non-file spans and dependencies without pretending they have a workspace source range. Distinguish macro syntax from expanded semantics.
5. Emit constants/statics/CTFE outcomes and errors, unsafe/ABI/raw-pointer/inline-assembly facts, attributes/cfg selection and async/coroutine lowering. Use exact evidence for cross-language ABI/export links; otherwise emit candidates retaining both contexts.
6. Complete structured diagnostics and per-body/per-family extraction coverage. A compilation that fails to expose a body cannot support negative body facts. Cache typed extraction only against exact compiler/source/context input identity.

**Acceptance:** actual compiled fixtures cover generics, trait/default/associated methods, function pointers, closures, macros, async, constants/statics and FFI declarations. Validate source/owner identity across files and generated spans, not merely raw MIR row counts.

### 7E. Rust MIR dataflow, ownership and advanced state

**Current status — open.** Real compiler relations and existing MIR analysis modules are inputs to this work, not evidence of complete derived behavior. Exact private loans/regions, finite transfer/join analyses, partial moves, drop/unwind/coroutine state and changed-body replacement all require production integration and the acceptance below.

**Surfaces:** `src/rust_mir_derived_analysis.rs`, private extractor enrichment and canonical derived integration.

Use typed MIR CFG/places/rvalues as inputs. MIR locals may be assigned repeatedly; MIR is not SSA. Implement explicit storage-live/dead, initialization/uninitialization, move/copy/assignment, projections, references/reborrows, aggregates, discriminants, calls, drops and normal/unwind cleanup. Track partial moves/field state and unknown aliasing conservatively.

Derive reaching definitions, liveness, value flow, points-to/memory accesses, ownership approximations and program-point state using explicit finite lattices and joins. A path-insensitive may fact is separate from definite initialization or exact compiler ownership evidence. Keep raw public observations and application analysis in distinct relations/provenance.

For exact borrow-check loan/region facts required by the selected profile, expose the narrow dated-nightly private seam at the compiler phase that retains those results. Do not label an inferred lifetime interval as an exact loan. Add reborrow/two-phase/region/end-point cases; missing private coverage is an explicit remaining implementation task, not permanent closure by a capability label.

Complete resource acquisition/transfer/drop/escape, unwind paths, closure capture modes, coroutine saved locals/suspension, effects and static concurrency models. Keep execution specialization and source owner correspondence intact. Integrate changed-body replacement with invalidation of all derived point/state rows.

**Acceptance:** real MIR fixtures cover branch joins, loops, partial moves, reassignment, nested projections, reborrows, drop/unwind, closure escape and async suspension. Check exact private facts separately from conservative derivations and propagate their precision into queries.

### 7F. Common graphs, structural facts and interprocedural summaries

**Current status — open.** Existing petgraph/common-analysis code and newly canonical calls can be reused. No full demand-rooted graph, dominance, structural or interprocedural summary delivery has been demonstrated from real inputs through public queries and updates. Preserve call-site witnesses and explicit unknown frontiers.

**Surfaces:** `src/common_derived_analysis.rs`, existing graph execution interfaces, `src/programmatic_derived_analysis.rs`, query projections.

1. Reuse existing `tarjan_scc`, `condensation`, `toposort` and `dominators::simple_fast` integration after binding it to real accepted canonical inputs. A graph projection names node/edge identities, parallel-edge policy, direction, filters, context, unknown frontier and bounds. Build only the requested owner/component/reachable region when possible.
2. Use compact `DiGraph`/appropriate adjacency storage for immutable projections and stable application-ID ↔ local-index maps. Use `StableGraph` only where its deletion behavior is useful; local stability does not make indices canonical. Preserve distinct call-site/edge witnesses even when a topology projection deduplicates parallel edges.
3. Use DFS/BFS with reusable visited state, SCC condensation for recursive components, and demand-rooted reachability. Avoid unconditional all-pairs closure or all-simple-path materialization. Cache bounded projections by exact relation version/context/direction/filter/policy; release them under retention pressure.
4. Dominance uses a declared entry and only reachable CFG nodes. `simple_fast` is the Cooper algorithm with documented quadratic worst-case time, not Lengauer–Tarjan. Post-dominance uses a reversed CFG and explicit synthetic exit policy for normal/exceptional exits; separately handle infinite loops/non-exiting regions instead of inventing post-dominators. Derive control dependence from that policy and identify natural versus irreducible loops.
5. State each interprocedural family's lattice/order, direct inputs, transfer, join, unknown propagation and invalidation closure. Seed direct callable effects/resources/value relations, solve recursive SCCs with a monotone worklist, and propagate only changed summaries to reverse dependents. Bound iterations and abstract-domain growth; nonconvergence is a precise unknown, not an empty summary.
6. Aggregate objective counts, degrees, nesting and distributions through DataFusion. Graph closure/reduction must state graph class and semantics; do not apply a DAG-only reduction to a cyclic graph or promote structural metrics into quality/risk labels.
7. Use existing parallel worker scheduling across independent owners/SCCs. Enabling petgraph's optional Rayon algorithms is conditional on an applicable measured algorithm; do not duplicate Tokio/checker pools merely because the feature exists.

**Acceptance:** diamond/multiple-exit/unreachable/nonterminating CFGs, self loops/mutual recursion, parallel call sites, ambiguous callees, external bodies and SCC summary convergence. Compare canonical facts across reordered provider batches and incremental reconstruction.

### 7G. Remaining forms, complete composition and query semantics

**Current status — open.** Eight-form parsing/typed ingress exists; four limited canonical forms are publicly demonstrated. FindPaths, MatchPattern, Compare, Summarize and full first-four behavior remain. Real prior-result resolution and repeated-form/multi-block DAG execution also remain; fixed per-form output relations must not collide or silently reuse another block's output.

**Surfaces:** `src/production_query_recipe.rs`, `src/relational_semantic_query.rs`, `src/query_service.rs`, query contracts and graph integration. Complete the first four forms from 4E and extend them to every relevant family in the coverage map as those families land.

| Form | Native-first plan and bounded algorithm | Correctness and partial-result requirements |
|---|---|---|
| FindPaths | Authorized edge projection; BFS for unweighted shortest paths; predecessor DAG for all-shortest witnesses; bounded simple-path search for explicit depth/count policy | Ordered node and fact IDs, parallel-edge evidence, cycles, source=target, direction, depth/path/work/deadline bounds and unknown frontier; reject unrestricted all-path enumeration |
| MatchPattern | Typed aliases and selective joins/semi joins; alternatives as tagged unions; anti joins only over complete scoped universes | Preserve binding/branch identity; negative clauses are indeterminate when coverage/semantic certainty cannot establish absence; budget intermediate cardinality and avoid implicit Cartesian explosion |
| CombineResults | Canonical-ID union/distinct, semi-join intersection, qualified anti-join difference and typed joins; context-wise output separation | Compatible workspace/epoch/context/role/representation/precision; incomplete right side cannot establish definite set difference; preserve each input's coverage and lineage |
| SummarizeFacts | Native aggregate/group/window functions for count/set/distribution and deterministic derived values; exact grouping and input filters | State input set/coverage/precision; partial counts are partial/lower bounds where mathematically valid, never complete totals; approximate algorithms require explicit requested approximation |

Use a typed compiler shared by the eight forms rather than eight unrelated executors. Implement all released scope/selection/return directives, program-point filters, stop conditions, defaults and resolved-interpretation projection. Strictly separate literal code names from controlled phrases; ambiguity produces a typed input requirement or candidates, not guessed semantic interpretation.

Build the request dependency DAG before execution; reject duplicate IDs, invalid roles and cycles. Pin one epoch. Run independent branches concurrently within the query budget, materialize/fan out one canonical prior result, and execute dependents only after valid typed inputs are available. An upstream failed branch yields `NOT_EXECUTED_DEPENDENCY` downstream while independent branches finish. Fan-in and final records use deterministic canonical ordering.

Implement row/byte/path/depth/iteration/work limits and cancellation through every operator. Limits qualify completeness; return-page truncation is distinct from incomplete input processing. An optimizer rewrite must preserve authorization, schema metadata, unknown/negative semantics and deterministic ordering. No hidden physical provider/UDF/SQL handles enter the public request.

**Acceptance:** each form has real public source-to-answer cases. Add mixed-form DAGs with fan-out/fan-in, a cycle, a failed independent branch, cross-context refusal/context-wise comparison, an incomplete negative clause, deterministic tied paths, limit exhaustion and cancellation. Test all eight forms after exact reopen and after updates.

### 7H. Complete daemon-authored modern presentation

**Current status — partial foundation; full slice open.** Installed FastMCP resource/guarded-input paths, typed processing, explicit presence and diagnostic handling have passing scoped checks. Complete new forms, source permissions, public paging/cursors, replay/reconnect/expiry and slow-reader/retention behavior remain. The 95 adapter tests do not establish all-form or live-update presentation.

**Surfaces:** Rust v2 service/query handles, `codefabric-cpg-mcp/src/codefabric_cpg_mcp/server.py`, client/DTO modules, released Protobuf and modern driver.

1. Reuse one lifespan-owned `grpc.aio` channel/stub per daemon attachment on the same event loop. Set explicit per-call timeouts from the remaining tool budget. Stream bounded data using one read style per RPC and preserve response backpressure; do not concatenate an arbitrarily large daemon result before returning it.
2. Propagate Rust downstream deadlines with `Request::set_timeout`; endpoint-local timeouts are not the same as `grpc-timeout`. Use parent/child `CancellationToken`s for accepted query, provider, graph and materialization work. Join registered tasks/processes after cancellation; `spawn_blocking` work needs cooperative checks because dropping its future does not stop it.
3. Keep pure preparation and atomic start in the daemon. FastMCP 4 `InputRequiredResult`, `ctx.input_responses` and sealed request state carry only typed guarded continuation. Each input round re-enters middleware and the tool: replay must not create duplicate accepted queries. Bind continuations to principal/workspace/request/authority and enforce expiry/change behavior.
4. Preserve the modern negotiated extension model per request. Advertise only capabilities the adapter/client implement; do not revive legacy FastMCP task/elicitation APIs. Existing query/status/cancel/resource tools remain the public catalog; eight semantic forms are not eight new tools.
5. Keep canonical result, delivery representation and compact summary consistent. The daemon authors scope, facts, coverage, error semantics and authorized immutable resources. Python may format/project them but may not recompute completeness or maintain a second mutable graph/status cache. `UserSession`/`Context` state is not CPG authority.
6. Use strict Pydantic models/discriminated unions and reuse module-level `TypeAdapter`s where appropriate. Preserve aliases and emitted JSON schema; avoid permissive coercion, unknown-field leakage, `model_construct` at untrusted boundaries or partial-validation modes that admit truncated requests. Serialize through declared field types and explicit public projections.
7. Preserve Protobuf `optional`/`oneof`/unknown-field compatibility. Deterministic protobuf serialization and orjson `OPT_SORT_KEYS` are not canonical semantic identity encodings. Keep released canonical JSON/ID construction at its current boundary. If measured JSON encoding needs orjson, compare against Pydantic's byte serializers, require JSON-safe integers/values, reject unsupported types and never use unvalidated `Fragment`/`default=str` as a bypass.
8. Bind pagination/cursors/resource reads to snapshot, principal/access scope, query parameters and deterministic sort position. Source permission is checked separately. Cancel/reconnect uses the accepted query handle and sequence; it does not start a new logical query. Resource TTL/lease release must reach Rust ownership. Keep STDIO protocol output clean and internal paths/tokens out of errors/log projections.

**Acceptance:** real modern client tests cover all forms, multi-round/replayed input, expired/changing authority, cancellation and reconnect, bounded paging, slow readers, resource retention, source denial and consistent errors. Unit DTO/schema checks supplement these behaviors; they do not replace them.

## 8. Outcome 8: sustained operation, native maintenance and measured performance

### 8A. Durable/recomputable split and selective persistence

**Current status — partial foundation.** Exact selected-version reopen, proof-only history removal and immutable input-blob reuse exist. Selective persistence by consumer, reuse of unchanged relation versions/owner partitions, reduced redundant observation writes and measured edit-time file/version growth remain. The current call selector does not justify a new durable-history policy by itself.

**Surfaces:** programmatic publication/observation Delta histories, `src/fabric/programmatic_delta_runtime.rs`, exact reopen, source image storage, result/cursor leases.

Classify each retained relation by its real consumer:

| Data | Default disposition |
|---|---|
| Active snapshot version vector, authoritative input/context references, compact operation/publication state and requested/completed/remainder coverage | Durable; required for coherent reopen and explanation |
| Provider-native/canonical facts selected by current or retained snapshots | Durable or exactly recoverable under the selected retention policy; raw/normalized coexistence remains queryable |
| Expensive derived facts with actual query/reopen consumers | Persist selectively and reuse unchanged exact versions |
| Intermediate joins, graph worklists/projections, recomputable catalogs/plan caches | Memory/cache by default; evictable; do not write a new Delta history merely because an internal phase exists |
| Public immutable results and source context | Retain only while leased or within configured user-visible TTL/history |
| Detailed provider/operation diagnostics | Bounded retention; compact failure/coverage references survive as needed |
| Obsolete proof/process histories | Stop new writes; remove readers or provide an explicit forward migration before reclaiming old data |

Implement unchanged-version reuse per relation/owner partition, native predicate replacement/merge where appropriate and source-blob deduplication. Under FAB §9.1, Writes and retry ownership, durable writes retain the command-owned operation/writer/application marker and `CommitProperties::with_max_retries(0)`, with the caller's exact session and required-session fallback policy. Reconcile uncertainty in the command actor before a new attempt. Batch publication writes by practical relation/context layout; avoid one Delta table/file per source owner and small-file multiplication. Preserve exact schema/ID restoration and owner deletion semantics.

Reopen the selected exact version vector; do not reconstruct an epoch by asking each table for latest. If a selected snapshot cannot be read, expose recovery/corruption with a deliberate repair/rebuild path rather than silently combining surviving versions. Migrate retained records only where a current reader requires it.

**Acceptance:** unchanged inputs avoid redundant relation rewrites, changed/deleted owners update correctly, raw and canonical queries reopen identically, and cache eviction does not change answers. Measure actual file/byte/version growth over repeated edits.

### 8B. Enable native compaction, checkpoints and vacuum

**Current status — open for compaction/reclamation.** The inspected executor still returns `OptimizeCommitIdentityAndRetryControl` for optimization and `AtomicVacuumApprovalBinding` for destructive vacuum. Native checkpoints and retention-aware dry runs work. The native commit-properties corrections, ownership replacement and actual reclamation/reader-race/recovery acceptance below have not been implemented.

**Surfaces:** `src/fabric/delta_guarded_maintenance.rs`, `programmatic_delta_maintenance_command.rs`, `programmatic_delta_maintenance_relation.rs`, `delta_commit_reconciliation.rs`, exact reader/store/writer owners and selected delta-rs source.

The current implementation still has two different blockers. Treat them differently:

**Compaction has a confirmed native commit-properties gap.** At the selected pin, `operations/optimize.rs` creates fresh `CommitProperties` for internal commits, copies `app_metadata`, and supplies its own retry count. It does not preserve the caller's entire transaction/retry policy. A builder method existing in rustdoc does not resolve that behavior.

1. Use native `OptimizeType::Compact` with explicit caller `SessionState`, `SessionFallbackPolicy::RequireSessionState`, target file size and bounded concurrent tasks. Reuse the owned object-store registry, memory pool and spill manager; no fallback internal session.
2. Initially use a single maintenance commit where the native option allows it; give it the operation/writer/application transaction marker and exact predecessor under the workspace writer/maintenance owner. Implement the narrow native fix that preserves caller transaction properties and zero retries through the actual optimize commit, including post-commit handling. The unmodified retrying builder is not admitted under FAB §9.1. Controlled native write primitives are the fallback if fixing the builder cannot meet that contract; neither route implements another Delta log or generic transaction framework.
3. If multi-commit optimization is needed, assign distinguishable per-commit markers and reconcile the committed prefix. Reusing one application transaction version across separate internal commits is not an acceptable idempotency design. Cancellation/lost acknowledgement must not trigger blind replay.
4. Retain native metrics and exact resulting table versions; publish a coherent successor relation vector. Optimization changes physical layout while preserving logical rows, owner identities and coverage. Test before/after logical queries and old leased readers.

**Vacuum's old approval-receipt requirement is unnecessary.** The current code demands an exact dry-run set bound to a later application approval revision. Replace that requirement with real exclusion and retention ownership:

1. Acquire a per-table destructive-maintenance lease through the existing writer owner. Coordinate it with snapshot publication, candidate/unresolved writers and admission of new historical/result leases. Existing readers remain active with their versions protected. Ordinary current queries may continue while the protected version set remains valid; do not serialize all workspace query execution.
2. Under that ownership, compute the actual protected versions/resources described in 8C and use native `with_keep_versions`, configured retention duration and `with_enforce_retention_duration(true)`. Run native `vacuum().with_dry_run(false)` while the protection set cannot be invalidated by admitting an older reader. A dry run is diagnostic/forecasting, not a user approval gate or deletion manifest authority.
3. Use native lite vacuum for tombstone cleanup and scheduled full vacuum for orphaned data files, with no in-flight/unresolved writer whose files could be removed. Preserve native path encoding and object-store deletion. Never hand-delete Delta data paths from an old dry-run list.
4. Inspect the pinned keep-version and native log/checkpoint cleanup behavior against actual retained readers. Native file protection alone does not guarantee that logs/checkpoints required to reconstruct those versions remain available. If the API cannot preserve a necessary read, use a narrowly scoped correction at the real cleanup seam before enabling that cleanup.
5. Record started/completed/partial maintenance outcome and useful native metrics. The pinned vacuum implementation also creates fresh properties for its start record: preserve command-owned markers and zero-retry behavior through both native start/end commits with a focused correction where necessary. A cancelled/failed deletion can have removed a safe subset; reconcile/retry as maintenance rather than treating it as an all-or-nothing semantic transaction. Full recovery must tolerate orphan files and incomplete maintenance bookkeeping.
6. Keep native checkpoint creation; coordinate log retention with protected snapshot reconstruction and any real consumer. Never implement a second checkpoint writer. Remove `AtomicVacuumApprovalBinding` and obsolete proof-receipt dependencies when their runtime consumers are replaced.

**Acceptance:** actual compaction commits with intended identity/retry behavior; lost-ack recovery identifies committed work; native vacuum really reclaims an obsolete file while current, historical and leased readers reopen; new-old-reader admission and concurrent writer races cannot invalidate the protected set. Test the actual pinned library seam rather than a mock approval receipt.

### 8C. Finite retention and reclamation across all owners

**Current status — open.** Broad disk budgets, physical headroom and existing leases/handles are foundations. Provider views/build/cache state and all snapshot/source/result/diagnostic owners still need coordinated finite retention and real maintenance/TTL cycles. No sustained finite-state or reclamation result is claimed.

**Surfaces:** `src/fabric/retention_command_effect.rs`, `src/fabric/delta_exact.rs`, snapshot/source/result leases, provider context caches, runtime disk accounting and maintenance scheduler.

1. Define configurable history age/count, result/cursor TTL, source/dependency cache policy, diagnostics age/size and table retention periods. Publish what historical requests remain supported. Finite retention can expire unleased history; it must not break an active authorized lease.
2. Protect current/candidate snapshot relation versions, explicitly retained history, active query/result/source leases, unresolved operations/writers and any actual CDF consumer interval. Protect the source blobs and context/dependency inputs needed by those snapshots, not only Parquet files.
3. Coordinate lease admission/renewal/expiry and reclamation in the same owner. Remove expired handles from public lookup before deleting their data; make repeated release idempotent. Reference counts/leases correspond to real readers and jobs, not allocation receipts.
4. Bound provider/checker contexts, graph/plan caches, queued jobs, result pages, source snapshots, spill, private build directories and diagnostic retention. Use actual disk free space plus configured budgets. Avoid evicting hot dependency/sysroot bundles repeatedly; account shared immutable storage once.
5. Start maintenance before hard disk pressure. If protected live state prevents reclamation, apply backpressure and report the limiting leases/work, while keeping control/cancel/status responsive. Do not corrupt active data to satisfy a numeric cap. Legitimate working sets may require raising broad configurable budgets.
6. CDF remains conditional. If a concrete consumer is added, use native bounded start/end-version loading with durable consumer checkpoint and retained interval, deduplication and gap recovery. Do not introduce CDF merely to connect daemon components that already share an exact epoch.

**Acceptance:** repeated edits and reads across at least one actual retention/TTL cycle reach finite retained state; ending leases permits measurable reclamation; old protected queries survive compaction/vacuum; expired requests receive clear errors; interrupted publication and CDF intervals, if enabled, remain protected.

### 8D. Failure, cancellation and deployment recovery

**Current status — partial Linux baseline.** Actual containment, joined provider/native cleanup, exact restart and scoped cancellation/lost-ack tests pass in earlier slices. Extend that behavior across the live update loop, retained providers, new queries, expiry/pressure and real maintenance. No equivalent unvalidated non-Linux profile is implied.

**Surfaces:** daemon/supervisor, provider services, runtime ownership, command reconciliation, gRPC/adapter lifespan and storage maintenance.

Exercise failures at the actual new ownership boundaries: source changes during capture, provider death/hang, metadata/build failure, stale completion, query/stream cancellation, slow/disconnected client, daemon restart during publication, activation acknowledgement loss, interrupted optimization/vacuum, expired leases, disk pressure and unavailable source roots.

Use one cancellation tree and registered task/process ownership. Bound queues and retain control-runtime headroom. Join subprocess groups/cgroups and spawned tasks; do not equate a dropped future with stopped native work. Distinguish computation cancellation from cleanup/reconciliation, which may outlive the user deadline but must finish under the owning service.

On restart, recover selected activation and uncertain operations before accepting conflicting mutations. Recreate watcher/provider state from durable authoritative inputs, mark work needing recomputation pending, and resume convergence. Reject incompatible protocol/provider identities with a useful recovery instruction.

Preserve platform-specific truth: the implemented RSS/containment profile is Linux. For each other supported deployment profile, implement the corresponding observer/containment/lifecycle behavior or expose a specific unsupported capability; never report an unavailable RSS sample as zero. Validate a non-Linux profile before advertising equivalent containment or resource behavior. This does not delay Linux product delivery for an unselected platform.

**Acceptance:** affected boundary cases plus a real mixed-language restart/cancel/recovery scenario demonstrate no stale-current facts, orphaned active providers, permanently held writer leases or unbounded abandoned results. Use focused fault injection, not a Cartesian product of every failure at every function.

### 8E. Runtime phase metrics and representative measurements

**Current status — open.** RSS/cgroup/headroom observations and benchmark/corpus tooling exist; small fixture timings are recorded. Correlated phase coverage, representative mixed/real-repository workloads, distributions and sustained update/retention/recovery measurements have not been completed. Existing timings are not latency objectives or a performance pass.

**Surfaces:** existing tracing/metrics, source watcher/coordinator, providers, DataFusion execution/publication, `tooling/product/benchmark.py`, golden driver and reusable scale fixtures.

Add correlated monotonic timestamps/counters for detection, debounce, authoritative capture, queue delay, parse completion, syntax activation, provider start/end, normalization, semantic activation, first result batch, final response, reopen and recovery. Carry operation/source generation/query correlation so phases can be joined without retaining every internal call. Report terminal reasons and missing observations explicitly.

Measure:

| Question | Required observations |
|---|---|
| Startup/reopen | Process-ready, capture/provider/publication phases, first useful answer and exact reopen latency; warm/cold state identified |
| Query | Queue time, planning, first batch, completion, scanned/returned rows/bytes, spill, limits/coverage and selected form |
| Edit latency | Detection/debounce/capture to syntax activation, semantic convergence and query-visible result; input rate and scope size |
| Sustained load | Pending/running/replaced jobs, oldest pending age, convergence under quiet, native/thread use, fairness/control responsiveness |
| Memory/disk | Managed/DataFusion reservations, actual current/peak-sampled RSS, provider cgroup totals, system headroom, spill, source/build/Delta/result retained bytes |
| Maintenance/recovery | Files/bytes before/after, elapsed time, protected leases, retry/reconciliation outcomes, recovery-to-queryable time |

Use small, medium and large mixed-language workloads with actual imports, Cargo dependencies, calls, types, control flow and edits. Include a representative real repository and deterministic scale variants; a workload that only hashes files is not a CPG benchmark. Record corpus/revision, compiler/context, machine, effective settings and sample count. Separate end-to-end wall time from daemon-only phases. Report distributions and sample sizes; do not present a tail percentile from a handful of runs as robust.

Start with the current broad workstation profile: 64 GiB managed workspace budget, 32 GiB shared DataFusion pool, 16 data workers/partitions and 112/96 GiB RSS pause/resume, plus system-memory/disk headroom. These are starting settings, not proof of optimality or universal caps. Root RSS excludes provider RSS; observe aggregate pressure without double-counting shared reservations as physical memory. Adjust provider/context concurrency together with memory measurements.

Set service objectives after measurement. The earlier 100 ms/500 ms ideas are hypotheses. Report supported workload sizes and what remains pending at each latency, rather than claiming universal real-time completion.

### 8F. Optimize measured bottlenecks using native capabilities

**Current status — open.** Native canonical joins, immutable input reuse, schema nullability refinement and one-pass result lookahead are useful enabling changes. Complete safe scan pushdown/statistics/properties and workload-driven tuning with before/after evidence. No representative performance improvement or optional overlay/CDF/Rayon/orjson adoption is claimed.

**Surfaces:** schema/provider adapters, child catalogs, query planning, relation publication, graph/provider caches and scheduling.

1. Restore safe native `TableProvider::scan` projection/filter/limit pushdown. Map physical/logical fields and statistics through `SchemaContract`. Return `Exact`, `Inexact` or `Unsupported` truthfully per filter; retain residual filtering for inexact predicates. A fully pushed filter may need a column absent from the output projection.
2. Preserve qualified schemas, null/cardinality/key constraints, ordering/partitioning and equivalence properties only when they remain true. Wrong statistics/properties can change answers, not just speed. Test a selective filtered projection, reordered/fixed-width IDs and limit with an inexact predicate against the unoptimized logical answer.
3. Let native Parquet/Delta pruning, projection, row-group statistics, compression and file sizing reduce IO. Measure compaction before choosing Z-order or additional indexes. Choose table partitioning by real selectivity/file sizes, avoiding per-owner tiny partitions and skew.
4. Tune batch size and target partitions by workload: tiny lookups can avoid 16-way exchange; large scans/joins use available cores. Preserve one shared pool and bounded spill. Stream responses and intermediate batches; avoid repeated Arrow→rows→Arrow conversion or full graph loading for a rooted query.
5. Cache immutable provider/graph/plan results by the exact inputs that affect semantics, including context, schema/algorithm and access scope. Use byte/cost-aware eviction and release retained buffers. Prefer Pyrefly bulk tables and Cargo/Tree-sitter incremental state over application reimplementation of their engines.
6. Add precise dependency invalidation when measured conservative rebuild time dominates. Validate negative dependencies and clean/incremental equivalence before narrowing scope. Full-context fallback remains available when dependency coverage is uncertain.
7. Add an Arrow overlay only if measured Delta publication cost prevents a chosen latency objective. Bind overlay batches to the same snapshot/coverage contract, bound ownership and flush/recovery semantics, and preserve exact query pinning. Do not introduce a second independent current graph or mutable Python cache.
8. Add durable CDF, optional Rayon algorithms or orjson only against a concrete bottleneck/consumer and retain the simpler path if improvement is not material. Do not make optional mechanisms prerequisites for closing unrelated functionality.

**Acceptance:** before/after representative measurements show the intended improvement and unchanged semantic/coverage answers. Sustained operation stays finite through retention cycles and remains cancellable under pressure.

## 9. Validation, integration and completion

### 9.1 Reuse the existing command surface

These are attributable implementation checks from 2026-09-09. The latest rows include checks run while completing the diagnostic-detail slice; earlier rows remain historical evidence for unchanged inputs. Counts overlap and are not a combined full-suite result.

| Implemented slice | Recorded check | What it establishes / limit |
|---|---|---|
| Contained Cargo/path dependencies/multiple targets | Focused provider/context tests and real mixed/virtual-workspace daemon cases | Actual captured dependency calls, inherited settings and failed-target retention; no generated/registry/proc-macro closure |
| Canonical Rust calls through `45421cff` | Seven final selected canonical/provider/restart cases plus real call fixtures | Direct, indirect, repeated, dependency and unmapped macro cases; exact reopen |
| Public declaration facts through `64ce1acf` | 42 affected scope/query/resource/service cases; installed client; exact restart | Real Python/Rust declaration IDs/ranges, duplicate subjects, failed target, completed empty scope and explicit unsupported references; not all RetrieveFacts |
| Chunked inventory `1301df5a` | 16 selected root Pyrefly/daemon cases; sidecar/protocol/adapter checks | Real contained cross-chunk imports and 70-module publication; 29 sidecar and 95 adapter tests, lint/types and proto checks pass |
| Checker anchors `92bb153d` | 30 sidecar cases and strict sidecar check; 26 selected root cases | Actual imported alias/bound-method definition anchors and substituted file/digest/range rejection |
| Canonical Python calls `734821db` | Affected native/canonical/recipe cases; final regression rerun; real mixed/Python daemon and installed restart | Stale relation-count assertions fixed; repeated/dynamic/module/cross-module calls and exact range pass; reopen 15.6 s |
| Committed governance/schema/result fixes | Governance rule cases/scan; root library Clippy; scoped schema/IPC tests | Governance passes; root library Clippy completes with existing backlog, not a strict clean result |
| Historical call processing, before call-query wiring | `just root-test-incremental -E 'test(processing_scope) \| test(pragmatic_python_semantics_publish_real_call_targets)'` with both provider binaries | Four tests pass; real Python run 6.59 s; implicit-property/decorator scenario subsequently passes |
| Validated call-query continuation | Installed Python/reopen and mixed Rust/declaration/call cases; 16 focused tests and five final regression cases | Pass; type checks/governance pass; strict root Clippy retains its existing backlog |
| Source/disclosure and richer source contexts (`46e260a9`, `10e577d5`, `33b2c2a5`) | Installed exact-span/disclosure/revocation, function definition/body live/clean and surrounding-line/reopen scenarios | Pass: 21.04 s source/revocation, 206.25 s function/body, 33.70 s lines/hard limits; remaining subjects/directives stay open |
| Decoding/columns/raw paths (`55e50782`, `d07d81e4`, `4516a5a7`, `fcd62fd9`) | Real source/call/live/clean/reopen scenarios and strict provider checks | Pass: mixed decoding 203.40 s, text columns 202.48/204.34 s, Python paths 81.85 s, Rust paths 138.00 s; further codecs/argv/rename behavior open |
| Python configuration/roots/stubs (`1c913767`, `6e339a3e`, `38629d50`) | Installed independent clean/live comparison with configuration changes and source/stub/root replacement | Pass: 165.86/72.87/73.85 s respectively; external distributions/roots and full semantic output open |
| Rust custom build/caller/platform/toolchain/linkage/selections (`41438e2e` through `09988b9d`) | Actual contained targets, installed live/clean/reopen fixtures, typed scope/schema/adapter checks | Selected custom build, exact caller scope, platform flags, shared toolchain capture, linkage and feature/profile cases pass; feature/profile final scenario 192.07 s, rerun after source-first startup 212.76 s; complete generated/external/unit closure open |
| Live lifecycle and source-first fresh startup (`a6d8569a`, `99b77ec0`, `56d2a36d`) | Running mixed daemon, independent clean comparisons, current barriers, obsolete completion and source-only restart | Final staged scenario 186.06 s; after primary diagnostics 188.20 s; public source precedes semantic successor; retained providers/full corpus open |
| Retained processing continuation (`730a346d`) | Installed 130-partition paging through restart and a live successor, invalid ranges/blocks/released handles | Pass 53.38 s, later source-first regression 74.63 s; all-family/efficient status open |
| Primary Rust diagnostics (`f1e44d80`) | Two contained compiler cases, mixed target/context, all-failed startup, live/clean repair and exact failed-epoch reopen; Rust service/launcher checks | Pass: failed-no-MIR 8.12 s, successful warning 8.21 s, mixed contexts 76.48 s, all-failed 48.36 s, live/clean/reopen 374.04 s; 18 service and 19 extractor tests; tooling 216; no containment weakening |
| Typed Rust diagnostic details (`4cc74d7c`) | `just extractor-check`, `just extractor-test`, `just extractor-identity`; provider/schema cases; final four-case contained/installed native selection; `just root-check`, `just governance`, affected Clippy | Pass: 20 extractor and ten provider/schema tests; successful warning 7.80 s, failed-no-MIR 7.71 s, all-failed startup 58.77 s, live/clean/repair/exact reopen 350.01 s; 988 existing Clippy warnings, no new findings; no full-suite or cross-upgrade migration claim |

Earlier four-case golden runs passed startup, installed Python serving, exact reopen and cancellation.
Subsequent source/configuration/Cargo/diagnostic scenarios establish the limited live convergence
recorded here and in STATUS. They do not close the complete edit corpus. The latest affected Clippy
result retains 952 library and 36 integration warnings (988 total), with no new findings; strict
lint cleanliness remains open. Global formatting retains seven previously recorded untouched-file
failures, and repository-wide spelling retains escaped-source fixtures plus vendored/historical
text findings; changed formatting/new text and navigation are checked separately. The historical
`0cc7242` full-root result (1,038 passed, 13 failed, two skipped) is not a current verdict. No new
four-domain/full-root aggregate green, full-family/eight-form, destructive-maintenance,
sustained-retention or representative benchmark acceptance is claimed.

Select focused cases during implementation; run integrated checks when the assembled product claim warrants them. The existing command surface remains:

| Changed boundary | Existing command/fixture route |
|---|---|
| Stable Rust implementation | `just root-check-fast` during edits; focused `just root-test-incremental --lib -E '<selection>' --no-tests=fail` with required provider identities supplied for process cases; `just root-test-rust` for the aggregate; affected Clippy, then `just root-check`/`just root-clippy` at integration; `just root-doctest` separately |
| Real Rust provider | `just rust-provider-test`; `just extractor-check`, `just extractor-test`, `just extractor-identity`; extend existing single integration target |
| Pyrefly | `just sidecar-check`, `just sidecar-test`, real UDS/provider cases and semantic public fixture |
| Wire/presentation | Deliberate `just proto-gen` when contracts change, inspect generated diff; `just proto-check`, applicable interop tests, `just adapter-lint`, `just adapter-type`, `just adapter-test`, `just adapter-stdio-test` |
| Source-to-public product | Extend `just golden` selectors in the existing runner for mixed semantics, live edits, full analyses/forms and retention/recovery; a missing or empty selection fails |
| Corpus/benchmark tooling | Focused `just tooling-test` cases; actual runtime adapters invoked through the product runner; `just product-bench` for real workloads |
| Dependency/native feature selection | `just stable-graph-check`, affected domain compilation and actual consumer tests; no broad licensing/source-artifact project |
| Final assembled product | Relevant four-domain gates and `just ci-fast`, meaningful golden/differential/recovery/retention scenarios, exact reopen and representative benchmark report |
| Documentation-only changes | `just docs-check` with applicable arguments, relevant spelling/navigation and `git diff --check` |

Keep known failures visible. Repair baseline regressions as their code is touched and resolve relevant integrated failures before claiming completion. Do not delete a product assertion merely because older scaffolding made it fail. Do not rerun full CI after every small edit, or add a new permanent check for an infrequent mechanical transformation.

### 9.2 Completion criteria

The complete remaining scope is delivered only when:

1. All coverage-map families have an actual supported production path for the selected Python/Rust profiles, including exact versus approximate distinctions, external/generated inputs and semantically unavoidable unknowns. Unsupported staging is not counted as implementation.
2. All eight forms and their released directives/composition operate on those facts through the real daemon/modern adapter, with authorization, exact source selection, deterministic ordering, bounds, cancellation and partial/negative semantics.
3. A running daemon handles edits, configuration/dependency changes and failures, promptly exposes invalidation/remainder, and converges under quiet. Independent clean reconstruction agrees semantically.
4. Snapshot/source/result leases preserve old reads; exact state reopens; uncertain writes and interrupted provider/maintenance work recover; unleased state is reclaimed through actual native maintenance.
5. Representative measured workloads demonstrate useful response/convergence behavior and sustained finite storage/work, with machine/settings and limitations reported. No claim promises universal real-time analysis, exact answers to undecidable dynamic behavior or immunity from OOM.
6. Obsolete fallback/proof/approval-only code and tests are removed as working replacements land; public compatibility is preserved or deliberately migrated. STATUS and the parent backlog reflect delivered behavior without a new certification state machine.

Ordinary corpus assertions and short algorithm arguments should explain why the implementation is trustworthy. There is no requirement to persist every intermediate calculation or produce an artifact for every source edit.

### 9.3 Material risks and concrete responses

| Risk | Implementation response |
|---|---|
| Compiler input closure differs from captured source | Contained metadata/dependency preparation, exact manifest/context identity, generated-input capture, mutation rejection and failure scope in 4A |
| Pyrefly bulk Query cannot answer every semantic question | Use the pinned resolver/TSP/index handler seams within the existing sidecar; extend narrowly and test actual semantic answers in 4B/7A |
| Existing analyses look complete because types/seeds exist | Bind real source/provider inputs and activate independent semantic examples; fix algorithms before claiming the corresponding coverage row |
| Unknown/missing inputs become false negatives | Dependency-scoped coverage, conservative invalidation and three-way negative reasoning in 5A/5B/7G |
| Update throughput cannot keep up | Coalescing, persistent provider state, broad shared parallelism and measured dependency precision; expose backlog and never reuse stale-current facts |
| Native maintenance violates identity/retention | Focused commit-properties correction, single owned maintenance operation, protected versions/logs and real concurrent reader/recovery cases |
| Graph/path/interprocedural state grows combinatorially | Demand-rooted projections, finite abstract domains, SCC worklists and explicit bounds/remainder; improve algorithms or budgets based on workload rather than silently reducing scope |
| New features recreate large supporting frameworks | Implement within existing owners and typed interfaces, prefer native library mechanisms, remove obsolete consumers in the same slice and require a real consumer for new durable state |

## 10. Paused implementation handoff

Stopped after diagnostic-detail production commit `4cc74d7c`, its final verification and this
documentation reconciliation, as requested by the user on 2026-09-09. Do not begin another slice.
STATUS records the final production commit, exact native command, executed checks and known limits.
All task-launched checks and fixture daemons have finished; no continuation is scheduled. The following is a resumption backlog, not an instruction to continue now.

1. Extend **4A/7D** from the captured Cargo/diagnostic foundation. Close registry/git and generated
   build-script/proc-macro inputs through exact owned capture; do not relax source-manifest checks to
   admit mutable `OUT_DIR` bytes. Resolve actual per-unit host/target/unified-feature configuration,
   complete byte-safe argv/tool-change invalidation and retained compatible build state. Finish
   diagnostic canonical/public consumers and separately qualify future-breakage reports. Verify
   source-call completeness under optimized profiles before using MIR absence as source-level absence.
2. Complete **4B–4D/7A** external Python roots/distributions/stubs and semantic type/member/import/
   reference propositions, full source/lexical/CST coverage and canonical authority/conflict/identity
   consumers. Native bulk tables and the selected pinned seams remain the implementation paths.
3. Finish **4E/5A–5B** remaining first-four meanings/subjects/directives and composition, all-family/
   owner/dependency/frontier processing scope, efficient authorized status, target/family barriers
   and historical selection. Preserve the demonstrated declaration/call/reference/source queries,
   disclosure checks, exact paging and conservative failed-context remainder.
4. Extend **6A–6D** Git/external/poll/root/config observation, retained parser/checker/Cargo state,
   selective persistence and fair scheduling. Reuse the existing source-first owner and broaden
   independent clean/incremental comparisons; do not replace it with another current-graph authority.
5. Deliver **7B–7H** corrected Python/MIR/common analyses on real inputs, the full ontology family map,
   all eight forms, typed multi-block/repeated-form DAGs and complete modern presentation.
6. Finish **8A–8F** unchanged-version reuse, native maintenance, coordinated finite retention,
   recovery, correlated phase telemetry and representative workload measurements. Optimize against
   measured costs; overlays/CDF/Rayon/orjson remain conditional on a concrete consumer or bottleneck.

Useful implementation findings for resumption: startup already reuses each syntax runner within a
publication pass; cross-edit retention is the missing boundary. Pyrefly's in-process state cannot be
reused across newly mounted immutable input views without an owned update design. Native Delta
optimize/vacuum still have the recorded commit/retry and reader/writer ownership blockers. No new
retention, maintenance or representative performance result was produced in the diagnostic slice.

The completion criteria in §9.2 are unchanged and unsatisfied. Continue editing this same backlog
and STATUS when work resumes; no activation state, proving-commit chain, independent worktree or
new approval ritual is required.
