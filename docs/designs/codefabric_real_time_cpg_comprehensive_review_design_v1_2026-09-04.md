---
artifact: design-dossier
design_id: codefabric-real-time-cpg-comprehensive-review
version: v1
date: 2026-09-04
status: draft
baseline_commit: df1c50c684a5e005c4b76ada9cd19d8030d9d7dd
working_tree_digest: 8bed1748d451922a9d6a37af22727c5c42688647ba69dbfa65dcb5c49cad4d2d
primary_scope:
  - src
  - rustc-extractor
  - pyrefly-sidecar
  - codefabric-cpg-mcp
  - docs/authoritative_design
doctrine_path: docs/library_ref/full_data_fabric_design_principles_v2.md
---

# Real-time Python/Rust CPG: comprehensive design review and recommended target

## 1. Executive decision

**Keep the Rust, Arrow, DataFusion, and Delta architecture. Complete and simplify its production execution path before adding another engine.** The current implementation contains valuable exact-version storage, typed provider boundaries, relational compilation, activation, authorization, and delivery infrastructure. It does not yet provide the complete, continuously updating, two-language CPG described by v2.3.

The largest improvement is not another DataFusion extension. It is connecting source change, effective compiler configuration, truthful semantic analysis, selective publication, and all eight query forms into one continuously running product. The next largest improvement is making that path incremental at the smallest sound ownership boundary while preserving native optimizer visibility.

The recommended target would still be preferred without this codebase: a dependency-driven relational CPG with provider-native Arrow observations, application-owned semantic transformations, immutable serving epochs, and bounded native query execution. Petgraph supplies selected algorithms; it is not a second graph database. Delta supplies table transactions; the application still owns coherent multi-table visibility. DataFusion supplies streaming execution, not automatic CPG invalidation or general incremental view maintenance.

### 1.1 Authority and review boundary

This is a **review and draft design recommendation**, not a replacement authoritative suite, an implementation plan, or an authorization to modify production behavior. The synchronized `codefabric-relational-data-fabric` **2.3.0** masters remain normative. Recommendations that extend their realization details require normal design acceptance and versioned integration. No production code, accepted design, active plan, or execution state is changed by this review.

The review uses `design-development` plus all six requested reference skills: DataFusion/Arrow, code-facts providers, Delta, gix/notify, petgraph, and Rust gRPC. Their main influence is the **native-first operation ladder**, exact-pin verification, strict separation of library capability from application semantics, and independent challenge of the target. Historical statements in reference skills about an unimplemented repository, earlier suites, FastMCP versions, or dependency pins are not treated as current facts.

The baseline is the live dirty worktree, not HEAD alone. The digest above identifies:

```bash
git diff --binary HEAD -- src pyrefly-sidecar rustc-extractor codefabric-cpg-mcp Cargo.toml Cargo.lock justfile .github | sha256sum
```

It does not fingerprint ignored build output, unrelated untracked files, or historical documentation. Source conclusions below are current-tree observations, not independently executed behavioral certifications. Independent provider/lifecycle, Delta, and query/serving reviewers inspected disjoint evidence; a fresh reviewer challenged the proposed target. The lead reconciled their findings rather than concatenating reports.

The same-session [v7 implementation status assessment](../reviews/implementation_status_codefabric_execution_proved_relational_data_fabric_implementation_plan_v7_2026-09-02_2026-09-04_v2.md) records focused passing checks, a failing `just ci-fast` at root Clippy, stale performance evidence, and an uncompleted current-tree certification. Those results are useful baseline evidence, not proof of the broader design proposed here. This design review did not rerun the full four-domain test matrix or measure new performance results.

### 1.2 What is already worth preserving

| Foundation | Evidence and design value |
|---|---|
| Isolated Rust/provider/Python build domains | Live manifests preserve the stable daemon, dated-nightly extractor, pinned Pyrefly sidecar, and presentation-only adapter. This contains unstable compiler/library APIs without introducing a Python data plane. |
| Owned, typed provider output and explicit gaps | `production_provider_recipe.rs`, `provider_native_syntax.rs`, provider admission, and relation-scoped IPC distinguish native observations from unsupported/missing results. |
| Programmatic session authority | `fabric/programmatic_schema.rs` installs typed transformations and derives schema/dependency/provenance observations. This is a stronger foundation than a hand-maintained schema/catalog census. |
| Exact Delta reads and governed writes | `fabric/delta_exact.rs:486`, `fabric/delta_write.rs:459`, `:754`, and `:1036` bind native providers/write plans to exact versions and sessions, application transaction identity, and zero library retries. |
| Manifest-last activation | The command/activation modules represent the necessary application transaction above per-table Delta commits. Retain fencing, readback, closed admission, immutable epoch pins, and forward recovery. |
| Reduced child authority | `fabric/child_session.rs:1214` rebuilds authorized provider graphs and installs an allowlisted object-store registry. Catalog names alone are correctly not assumed to authorize pre-bound views. |
| Conservative query-local cache treatment | `fabric/child_session.rs:1740` bypasses shared logical caching when request-local inputs or program-result bindings are present. This is safe, even though prepared-template reuse remains an optimization opportunity. |
| Bounded runtime/resource foundations | `fabric/epoch_runtime.rs:183` uses `TrackConsumersPool<FairSpillPool>` and spill limits. The result/resource and gRPC layers have explicit ownership and useful negative tests. These are foundations to exercise on the complete product route. |

### 1.3 Prioritized findings

“Blocking” means blocking the requested product claim, not necessarily a startup crash. “Semantic” means a wrong or unsupported meaning/claim. “Scale” identifies an amplification mechanism or missing operational capability; it does not assert a measured slowdown.

#### F01 — Blocking: continuous two-language execution is not installed

`fabric/production_workspace_startup.rs:489` captures only included Python files. It constructs `ExactPythonSyntaxRunner`, runs full per-file extraction, and supplies unconditional `RequiredInputAbsent` gaps to Pyrefly and rustc at `:702–710`. The production command composition at `:1044` installs unavailable source-wave and relation-publication effects. `PublishSourceWave` vocabulary and generic ports therefore do not establish a functioning update path.

Bounded structural searches under `src` found no live watcher construction/dirty-registry actor and no construction consumer of `GitCandidatePlanner`. Source-wave port implementations found in the inspected scope are test probes. This is structural evidence, not a whole-program call-graph proof.

**Decision:** build one workspace update owner around the existing command actor, with both language lanes and generation-fenced publication. The release oracle must edit files while the **same daemon process remains running**. The current causal test at `tests/integration/daemon.rs:1979`, through the helper at `:884`, compares separate startups; it cannot establish continuous updating. See LIFE §4 “Event ingestion, dirty registry, and authoritative reconciliation,” §5 “Source images, classification, and invalidation,” and §6 “Update pipeline and analysis lanes.”

#### F02 — Semantic: AST visitation order is labeled complete Python control flow

`programmatic_derived_analysis.rs:3094–3136` ranks Ruff `evaluation_ordinal` by `file_id` and joins adjacent rows. At `:3175` ownership is the file; at `:3185` completeness is literal `complete`. The node projection at `:2945–2955` makes the same assignments. This cannot represent separate callable CFGs, alternative branches, back edges, exceptions, cleanup, early return, or suspension. Reaching-definition/liveness/value-flow recipes then carry `Exact` precision over this substrate (`:1704–1728`).

This is not merely dormant example code: complete Ruff AST coverage and dependency-complete producer registration make the CFG transformation structurally reachable from fresh startup. Missing Pyrefly is not its dependency. However, the installed public recipe currently exposes only a narrow entity query; this review **does not claim an observed public false CFG answer**. The confirmed defect is internal semantic/proof completeness.

**Decision:** replace the sequential approximation with owner-local control/evaluation semantics, or restrict it to an explicitly proved subset and emit a remainder. Native relational expression does not make an incorrect algorithm correct. Preserve useful existing analysis in `ruff_adapter/cfg.rs:258` and `python_derived_analysis.rs:756` only after integration as application-owned analysis. GEN §24 “Python CFG generation,” §25 “Python value and dataflow generation,” ONT §15 “Control-flow ontology,” and §18 “Definition/use and dataflow ontology” are the acceptance authority.

#### F03 — Semantic: Pyrefly configuration identity is not causally effective

`pyrefly-sidecar/src/server.rs:538–606` validates the immutable context manifest but constructs `SemanticContext::new(state_root, handle)` without passing that context. `pyrefly_link.rs:271–296` builds `ConfigFile::default()` with fallback search and interpreter-query suppression. The accepted manifest's Python version, platform, import roots, stubs, and module map consequently do not configure that checker construction.

Startup also synthesizes context/environment hashes at `production_workspace_startup.rs:471–480` rather than using the existing immutable-input context discovery at `python_context.rs:385`.

**Decision:** construct the checker from the closed typed effective context, and derive its identity from the installed settings. A changed stub/search path must change actual resolved facts, not only a digest. Production does not yet invoke this sidecar, so this is a provider integration defect rather than a demonstrated public failure. GEN AC-G-14 “Analysis-context discovery, identity, and selection” and P27 are load-bearing here.

#### F04 — Blocking: provider breadth is narrower than the available library surfaces

The sidecar retains a real Query and calls `change_files`, but its coverage at `pyrefly_link.rs:769` explicitly leaves declared/expected types, import resolution, definitions/xrefs, navigation fallback, and actual affected-module coverage incomplete. These are partly missing integrations, not all unavoidable Python dynamism. Native Query, selected TSP/module-resolver operations, and selected Glean/internal exports have complementary roles.

Rust extraction has typed MIR and private stable-key foundations (`rustc-extractor/src/rustc_link.rs:1707`) but names remainders for loans/regions, full monomorphization/vtable closure, and structured compiler diagnostics (`:1789`). Its sandboxed lifecycle exists but is not installed in F01's route.

**Decision:** implement the selected profile family by family; classify missing implementation separately from unavailable input and irreducible semantic uncertainty. Do not equate a provider process handshake or a successful compile with complete CPG extraction. GEN §§14–51 and its acceptance clauses define the required breadth.

#### F05 — Semantic/lifecycle: source capture has unclosed outcomes

The startup capture loop (`production_workspace_startup.rs:513`) retains `Published` outcomes but does not materialize `Excluded(SourceCapabilityGap)` or keep `Deferred` work pending. `source_image.rs:407` defines those outcomes. The convenience `capture` path at `:683` uses a constant change-token callback; the available caller-owned fence is not connected to a running update generation.

**Decision:** each inventoried source must reach a typed capture disposition. A deferred read cannot silently close the inventory's coverage. Preserve events arriving during reconciliation, revalidate the capture/predecessor fence before activation, and emit exclusions as capability/proof rows. This does not mean existing descriptor-relative reads are useless; it means whole-wave source completeness is not proved by collecting only successful reads.

#### F06 — Blocking: eight declared forms do not equal eight executable forms

`production_query_recipe.rs:478–501` constructs only `compiled_find_entities_program` from `provider.ruff.binding`. Its implementation at `:678–786` resolves three phrases to `entity_kind = function`, and has empty return mappings, request inputs, and consumer slots. The eight-form requirement table at `:511–554` does not install eight executors.

**Decision:** compile the entire typed request contract through one relational compiler, with explicit form/selection/return/dependency support. Do not add eight unrelated engines. Capability discovery must distinguish supported forms and operands from declarations. QRY §4 “The eight request forms” and §5 “Composition DAG and execution semantics” require substantially more than this entity vertical.

#### F07 — Blocking: public response meaning and partial-DAG behavior are incomplete

`fabric/programmatic_query_backend.rs:1056–1070` returns whole-request failure when any block is not compiled. QRY §5 instead preserves independent branches and marks dependent blocks `NOT_EXECUTED_DEPENDENCY`. The backend's `canonical-response` payload at `:1216–1220` contains format, request identity, and snapshot rather than the complete QRY §10 response.

`codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/wire_models.py:164–180` exposes terminal execution/package/page counts without the independent execution, availability, completeness, freshness, and limit dimensions. The input accepts `delivery`, but `server.py:932–945` projects resource descriptors without using that choice.

**Decision:** produce one daemon-owned canonical semantic response, then project bounded JSON inline or externalize it according to SRV §9 “One logical response and delivery policy.” Arrow pages remain a valuable bulk representation, not a substitute for an agent-readable CPG answer. Keep content/coverage/ordering identical between delivery forms and independently authorize source disclosure.

#### F08 — Scale/architecture: metadata preservation disables native projection optimization

`fabric/programmatic_schema.rs:1450–1471` removes the logical `optimize_projections` and physical `ProjectionPushdown` rules from the entire candidate session. The code documents metadata loss/schema-check failures as the reason. `IdentityPreservingViewTable` and `SchemaIdentityExec` at `:919–1270` implement a generic value-preserving boundary; nested view planning disables its inner physical optimizer pass so the outer pass can handle the complete tree.

These facts must not be overstated: **not all optimizers are disabled**, and suppressing a duplicate nested pass is different from globally omitting projection rules. The global omission nevertheless forfeits important native column-pruning opportunities on wide facts and joins. No speedup or regression magnitude was measured here.

**Decision:** isolate exact identity restoration at the narrowest generic boundary and seek a source-proved metadata-preserving native path. Keep the safe workaround until nested views, projected/filter-only columns, and real Delta schemas pass. Do not simply flip the rules back on. Native projection pruning is an engine capability, not a custom CPG optimization to reimplement. See DataFusion reference §40A and schema chapters S7/S10/S11; upstream documents [projection optimization](https://docs.rs/datafusion/latest/datafusion/optimizer/optimize_projections/index.html).

#### F09 — Scale: the persistence path does not select changed owners or durability classes

`fabric/programmatic_relation_delta.rs:499–585` persists every selected sealed relation using `ReplaceAll`; the selector at `:824` excludes observations, not transient or unchanged relations. `ControlledDeltaWriteMode` at `delta_write.rs:194` supports append/full replacement, not owner predicate replacement. Fresh startup uses epoch-specific roots and `Genesis` (`production_workspace_startup.rs:714–761`). The library has useful `Advance` support for stable histories (`programmatic_relation_delta.rs:644`), but inspected explicit callers are tests.

**Decision:** use stable relation histories, reuse unchanged pins, distinguish transient/durable outputs, and replace only soundly affected owner scopes. Native `WriteBuilder::with_replace_where` is present at the exact pin; empty replacement and schema/null behavior still need an executable preflight. Predicate overwrite can rewrite shared physical files, so owner selectivity alone does not guarantee low write amplification. FAB §9 “Durable Delta relations” and §10 “Effective state and immutable overlays” already support the preferred direction.

#### F10 — Scale/recovery: CDF is eager and not an installed incremental consumer

`fabric/delta_exact.rs:785` collects the complete CDF range into batches. `delta_cdf_replay.rs:42` has no explicit range/row/byte envelope; its schema inspection at `:423` opens intervening versions. Inspected `ExactDeltaCdfDownstream` implementations are test doubles. The coordinator does correctly advance its SQLite checkpoint after downstream success.

**Decision:** stream bounded exact version windows, and persist downstream application of the source range with the semantic commit so checkpoint loss/retry is reconcilable. CDF transports changes; it does not infer owner invalidation, aggregate retractions, recursive closure, or semantic completeness. Start with affected-owner recomputation, not a general differential compiler.

#### F11 — Operational design: retention and maintenance have genuine unresolved constraints

Snapshot table creation enables CDF for every such table, keeps roughly a century of logs/deleted files, disables expired-log cleanup, and requests statistics broadly (`programmatic_relation_delta.rs:45`, `:704`). These preservation defaults are not a finite long-running capacity policy.

`DeltaRetainedResource` can name individual versions but not required CDF intervals (`delta_exact.rs:1507`, `:1775`). Protecting active Add files at a snapshot does not prove retention of interior log and CDF artifacts. Current destructive maintenance is denied, so this is **not a demonstrated data-loss incident**.

There is also a verified library constraint: pinned `OptimizeBuilder` reconstructs commit properties, drops supplied application transactions, and sets retries from `DEFAULT_RETRIES + commits_made` (`operations/optimize.rs:957–978`). Public vacuum cannot consume the exact privately constructed reviewed candidate plan. FAB §9.1's prohibition of uncontrolled retrying maintenance is justified. [Pinned optimize source](https://github.com/delta-io/delta-rs/blob/43a0cf10a313e5077c48637ad786a05359136bbb/crates/core/src/operations/optimize.rs).

**Decision:** model finite retention including CDF intervals; keep destructive operations fail-closed until proven. Use controlled consolidation only with equality and explicit CDF behavior. Pursue a narrow native API improvement or deliberate certified upgrade for maintenance, not a second application-written Delta transaction engine.

#### F12 — Assurance: current proof does not close the requested product claim

Typed schemas, invoked tests, producer closure, identical digests, and negative fixtures are useful evidence, but do not establish real two-language edits, complete CFG semantics, effective Pyrefly configuration, or complete public responses. F02 illustrates why input availability plus a producer registration is not a semantic oracle. The same-session terminal-gate failures further prevent release certification.

**Decision:** make installed, independently expected semantic behavior the acceptance boundary. Keep boundary checks; extend their workload to the real path. Compare incremental and clean state, but also compare both to expectations neither path generates. Do not execute the entire release gate for each source edit: runtime candidate proof and release certification have different purposes.

#### F13 — Scale: execution settings are a safety baseline, not an optimized profile

Fresh startup selects `FabricEpochRuntimeConfig::default()` (`production_workspace_startup.rs:537`, `:993`). That profile chooses a 256 MiB pool, 2 GiB spill limit, 8,192-row batches, target partitions 1, and disables forced Parquet view types (`epoch_runtime.rs:163–212`). These are explicit, bounded settings; they are not evidence of optimal throughput or latency. Target partitions 1 does not mean every library/process runs on one thread.

Transformation proof runs a fresh physical plan twice and retains output batches (`programmatic_schema.rs:2693–2801`). This has real determinism value, but repeated expansion of shared upstream views and retaining each complete output can amplify update work. It needs workload evidence and deduplication of **execution dependencies**, not removal of proof.

**Decision:** measure profile-specific CPU, memory, retained-epoch, scan, write, and proof costs. Budget raw provider buffers, IPC, graph allocations, and pinned old epochs as well as DataFusion reservations. Do not confuse an operator memory pool with a whole-process RSS guarantee.

#### F14 — Scale/resource: graph prototypes bound output more strongly than work

`fabric/graph_program.rs:868–935` seeds recursive reachability from all edge sources, retains `(source, target, depth)`, then aggregates/sorts before its final output limit. The shortest-witness helper at `:339–380` clones and enumerates simple-path prefixes instead of computing a distance/predecessor structure. Diamond-shaped inputs can exhaust the bounded frontier despite a modest shortest-path answer. An edge-only projection also cannot represent isolated endpoints.

The stream loop at `:753` checks cancellation only after `next().await` returns. `common_derived_analysis.rs:679` has no cancellation argument for its synchronous graph work. These helpers are not demonstrated current production query routes, so the finding is a **pre-integration design risk**, not a claim that current agent queries execute them.

**Decision:** demand-root native recursion, account intermediate work, and use distance/predecessor computation plus bounded canonical witness reconstruction for shortest policies. Keep explicit bounded enumeration for simple paths. Add cancellation around pending async work and within owned CPU work; pre-cancelled tests do not prove active cancellation. Preserve the real query coordinator's existing task ownership and joined cancellation (`query_coordinator.rs:1072`, `query_service.rs:2273`).

## 2. Constraints and target invariants

### 2.1 What “complete” and “real time” must mean

The useful target is not perfect static prediction of arbitrary dynamic programs. It is:

- Every included source construct is represented, or has an explicit capture/parser/coverage remainder.
- Every required fact family in the selected Python/Rust profile has a producer or an explicit capability gap.
- Every call/member/dispatch site has resolved candidates, conservative candidates, or an explicit unresolved remainder, with its context and precision.
- Unknown, unavailable, unsupported, partial, stale, and empty-known-complete are distinct.
- Every served fact is traceable to exact source, context, provider, algorithm, and epoch inputs.
- A source edit eventually yields a current, coherent epoch while the daemon stays alive; latency and backlog are measured separately for syntax and semantic availability.

Coverage is a query over **language/profile × owner/scope × fact family × context × source generation × precision**. A single workspace-wide green flag is inadequate. Required profile membership is an immutable release decision; current support is derived from actual production installation and executed evidence. Never turn “not implemented yet” into a permanent explanation of language uncertainty.

“Current” is relative to a captured source generation and verified dependency closure, not a promise that disk cannot change after a query starts. Queries pin one epoch. An explicitly requested exact semantic freshness policy may wait within a deadline; it must not quietly serve invalidated last-known-good facts.

### 2.2 Settled boundaries

| Boundary | Required behavior |
|---|---|
| Semantic owner | The Rust daemon and one immutable compiled release own meanings, identity recipes, query programs, policies, and algorithm selections. |
| Source truth | Descriptor-relative authorized current bytes; notify is an urgency signal, gix a candidate accelerator. Git history is not graph ontology. |
| Python ownership | Ruff owns typed source structure and lexical semantics; Pyrefly owns selected checker-backed semantics. Neither output is silently relabeled as the other. |
| Rust ownership | `rustc_public` supplies typed compiler observations; the narrow exact private seam supplies named enrichments. Application analyses remain distinguishable. |
| Data plane | Owned Arrow schemas/arrays/batches/streams in Rust; relation-scoped Arrow IPC across provider processes; no Python Arrow/DataFusion/Delta processing layer. |
| Durable authority | Native Delta exact versions plus immutable pinned Arrow segments and manifest-last activation. SQLite is temporal/reconstructible state only. |
| Mutation | One `FabricCommand` path owns authorization, idempotency, fencing, durable effects, reconciliation, and activation. Update workers may compute concurrently but cannot independently publish. |
| Query authority | Fresh reduced catalogs, recursive bound-provider/function/extension validation, independently enforced source disclosure, and no public arbitrary SQL. |
| Identity | ONT §64 “Required identity and public encoding rules”: application-owned IDs, path-based file identity, source/context-bound occurrences, private stable Rust keys when available. Rename changes file identity; cache continuity is not semantic continuity. |
| Trust | Untrusted Rust build scripts/proc macros execute only in the accepted containment profile; unavailable containment produces a typed gap, never host fallback. Python source analysis must not import/execute arbitrary project code. |
| Topology | Preserve the four existing build domains and singleton workspace supervisor; no new Cargo package merely for conceptual organization. |

### 2.3 Design constraints versus open measurements

Hard requirements are bounded queues/resources, no mixed epoch, no stale-current semantics, no silent data loss, deadline/cancellation convergence, and forward-only recovery. Absolute latency targets need a declared machine, repository corpus, cache condition, and mutation class.

Initial **proposed measurement targets**, not current results or contractual promises:

| Workload on a declared local reference machine | Candidate objective |
|---|---|
| Small save in a warm workspace | Syntax-current activation p95 below 500 ms, including debounce; separately report capture/provider/proof/commit time. |
| Small Python semantic change with stable context | Measure a 1–2 s p95 goal; report reverse-import closure size and sidecar work. |
| Rust semantic change | Separate compiler latency from fabric overhead; aim to bound post-provider integration/activation to hundreds of milliseconds for a small affected closure. Do not promise subsecond compilation. |
| Bounded entity/fact lookup | Evaluate warm p95 below 100 ms at the daemon, separately from MCP transport and cold startup. |
| Large burst or branch change | Eventual clean-equivalent convergence with bounded memory and observable backlog; no finite worst-case latency promise independent of repository size. |
| Cancellation under load | Measure time to terminal state, joined CPU/provider work, and resource release; control requests retain reserved service capacity. |

Choose actual limits from evidence before accepting a release. Correctness must hold when those performance goals are missed.

## 3. Target architecture

### 3.1 One causal pipeline, two freshness lanes

```text
notify events / explicit rescan / admitted context change
                  |
       bounded workspace update owner
                  |
 authorized immutable source images + effective typed contexts
                  |
  relational changed-owner/dependency closure
          |                         |
 fast Tree-sitter/Ruff       Pyrefly / contained rustc
          |                         |
 raw Arrow + coverage       raw Arrow + coverage/remainders
          |                         |
 native normalization / authority / typed application analyses
          |                         |
 syntax-current candidate   semantic-current candidate
          +------------+------------+
                       |
 exact Delta versions / durable Arrow segments + executed proof
                       |
 FabricCommand: fenced manifest-last activation of one complete ActiveWorkspace
                       |
 authorized typed semantic DAG -> DataFusion stream -> sealed response
                       |
       Tonic UDS -> FastMCP JSON / immutable result resource
```

The two lanes publish **separate complete epoch packages**, not partially committed epochs. “Syntax-current” describes semantic availability, not transactional incompleteness. The fast epoch withdraws all invalidated semantic rows and carries unknown/pending coverage; unchanged owners survive only with proved dependency validity. A late semantic result must be compatible with the source/context predecessor selected at publication or be discarded/recomputed.

Resolve LIFE §6.1 versus §7's required-input rule explicitly: an admitted, fully evidenced semantic **gap** may be valid data for a syntax-current capability state; missing source, ownership, withdrawal, provenance, schema, fencing, or proof evidence needed to establish that state still blocks activation. This distinction belongs in the immutable release's typed admission obligations and is evaluated over the candidate, not improvised by the update worker. A changed callee cannot leave “unaffected” callers pointing to invalidated semantic entities or preserve an obsolete negative answer.

The update owner owns watcher lifetime, source coordination, provider jobs, and cancellation. The command actor owns durable mutation. Keeping these responsibilities distinct permits parallel computation without introducing parallel writers.

### 3.2 Source and provider execution

**Watch/capture.** Use one bounded dirty-scope structure per workspace, with newest generation/reason, instead of one expensive job per notification. Register source/config roots, including external selected context inputs under explicit authority. Callback work is minimal. Overflow, root replacement, ambiguous rename, or `need_rescan()` triggers a generation-fenced authorized inventory; saves during the scan stay dirty afterward. The debouncer's internal cache refresh is not application reconciliation. Use gix status/tree/index/ignore information only to reduce candidates; bypass it for unsupported/untrusted repositories without changing correctness. See notify reference §§11, 20–24 and gix §§18, 20, 25, 37, 39.

**Fast syntax.** Retain bounded Tree-sitter state per exact file revision and grammar. Apply `Tree::edit` before reparsing against the new bytes; combine structural changed ranges with textual changes and enclosing-owner boundaries. Reuse compiled queries and one parser per concurrent parse. Ruff reparses changed files as a unit; cache AST/tokens/trivia/line information by content, source type, selected Python version, and provider release. Do not invent an incremental Ruff AST API. Emit application-owned relations; no borrowed tree/node/semantic-model objects escape their adapter. Tree-sitter reference §§10–12 and Ruff reference §§4, 8, 15–18 support this split.

**Python semantic contexts.** Discover and freeze Python version/platform, module map, search roots, stubs, dependency bytes, and policy. Construct the actual Pyrefly configuration from those values. Keep Query state long-lived per compatible context; use `change_files` for create/modify/delete, and query bulk inferred types/calls/members. Add selected TSP/module-resolver operations for declared/expected types/import meaning and selected Glean/internal exports for bulk declarations/xrefs. Do not copy the entire upstream schema. Normalize its identity/coordinates inside the sidecar. LSP is a narrow navigation fallback, not a per-node bulk extraction engine. Pyrefly reference §§14–17 and §23 establish these complementary surfaces; all selected internal APIs remain exact-pin contracts.

When the provider cannot report actual affected modules, conservatively refresh the proved reverse dependency closure. Requested files are not evidence of actual rechecks. Changing effective context requires actual state reconstruction/reconfiguration, not merely a new label around old Query state.

Serialize mutation of each retained checker context or prove its internal snapshot isolation. Cancelling a job must join its native work before later jobs can rely on the state. Coalescing, admission aging, and stable-watermark work prevent irrelevant saves from repeatedly cancelling all useful semantic progress. Any reuse across a newer source generation requires proof that the complete relevant content/context dependencies remain unchanged and a lawful newly bound job; it must not bypass the generation fence. Semantic convergence is required after a bounded quiet window, with sustained-churn backlog and starvation behavior measured separately.

**Rust semantic contexts.** Schedule compilation units by package/target/features/cfg/target/toolchain/dependency/build-input identity. Let Cargo/rustc perform compiler incrementality inside the accepted sandbox/cache authority. Capture typed MIR, spans/expansion provenance, definitions/types/instances, normal and unwind successors, places/projections, moves/copies/borrows/drops, and explicit private enrichments. Distinguish generic bodies from executable instances. Preserve sound unknowns for dynamic dispatch, missing dependency bodies, unsupported private APIs, or failed compilation. Rust MIR reference §§4–6, 19–24, 26–32, 37, 41 and its incremental/fixture appendices are the relevant allocation, not compiler debug-string parsing.

No provider runs arbitrary repository commands from a query expression or UDF. Safe compilation is an explicit lifecycle activity with source pins, containment, a deadline, bounded output, and process-group cleanup.

### 3.3 Application analyses: semantically correct before physically clever

The first required analysis is a real **owner-local control/evaluation model**. Python must preserve branch alternatives, evaluation order within expressions, short circuit, loops, abrupt completion, exception/finally cleanup, comprehensions, context managers, generators, and async suspension. Nested callable bodies do not become fall-through successors of the enclosing file.

Model these semantics as typed application transformations over owned provider observations. Prefer native projections/joins/unions/windows/recursive plans where they faithfully express the rule. For an irreducible AST-to-control construction, retain a small versioned application analysis kernel at the appropriate extension boundary. Do not force it into an incorrect row-number query merely to claim native execution. Its output is `derived`, not `raw.ruff`.

Subsequent analyses consume that control model: gen/kill and flow relations, reaching definitions, liveness, memory-location abstraction, alias/points-to, effects, resource states, and summaries. Their precision must name the abstraction and scope. Bounds and nonconvergence produce remainders; they do not quietly produce `complete`.

For interprocedural analyses, derive the affected dependency graph from actual input/output relations. Compute SCCs, evaluate summaries within affected SCCs to convergence, and propagate only changed semantic summaries. Deletions and edge removal must invalidate previously supported paths/summaries, not just add new facts. A changed provider/analysis release can invalidate an entire family even when source bytes are unchanged.

Use ordinary `Graph` for bounded multigraph algorithm inputs when parallel typed edges matter. Use `Reversed`/filtered adapters to avoid copying topology. Consider `Csr` only for a deduplicated simple projection whose canonical edge relation retains multiplicity separately. Petgraph `NodeIndex` stays execution-local; `StableGraph` slot reuse is not durable identity. Native `tarjan_scc`, `condensation`, `toposort`, and `dominators::simple_fast` are candidates for their precise semantics, with preflight bounds and cancellation strategy. Native algorithms that cannot be interrupted internally require a bounded input/cost envelope or a specifically justified cooperative implementation. Petgraph reference §§2, 8, 11, 13, 16, 20.6 supplies these constraints.

For deep graphs prefer the iterative `kosaraju_scc` option when the recursive Tarjan stack is an unacceptable risk. Dominance/postdominance need explicit entry, unreachable-node, and multi-exit policy; reversing edges alone does not define a correct multi-exit postdominator problem. Keep canonical fact-ID tie-breaking for equal-length paths when replacing path enumeration. `k_shortest_path` returns distance information, not the required ordered witness set.

### 3.4 Incrementality without a second computation engine

Use explicit relation dependencies and ownership to compute invalidation:

```text
changed source/context/program inputs
  JOIN observed dependencies and owner/support relations
  -> affected owners and relation families
  -> regenerate their current facts
  -> replacement rows + explicit owner tombstones + coverage changes
```

The first implementation should use **scoped clean recomputation** for each affected owner/family. This is easier to prove for removals, unknowns, exceptions, recursive graph changes, and aggregates than arbitrary incremental maintenance of every operator. DataFusion performs the joins and transformations; CodeFabric defines when and what to recompute.

Preserve explicit support/provenance dependencies, including negative lookups such as an unresolved import that may become resolvable after file creation. Structural expression scans discover relation/field dependencies, but cannot invent hidden provider configuration or semantic support dependencies. Typed kernels/providers must expose those dependencies; missing dependency evidence broadens invalidation conservatively.

Do not materialize transitive closure for every graph pair by default. Query bounded reachability on demand, and persist only measured reusable summaries that have clear invalidation and durability reasons. For recursive native execution, a final LIMIT alone does not bound recursive work: bound the working domain/depth, terminate at a fixed point, and enforce memory/time constraints.

CDF is optional transport between exact selected versions, not the update authority. Within one command the changed relation/owner set is already known. Add native CDF when a retained downstream consumer genuinely needs catch-up, avoiding redundant log replay in the hot local path.

### 3.5 DataFusion and Arrow capability selection

The objective is **maximum useful native capability**, not enabling every feature or adding an extension for every family. The following choices tie engine features to CPG outcomes and their falsifying evidence. Version-sensitive behavior is grounded in DataFusion reference §40A; broader main-site documentation is supplementary, not authority to silently upgrade from 55.0.0.

| Outcome | Preferred capability and target use | Boundary / proof obligation |
|---|---|---|
| Authority, normalization, unknown selection | Typed `Expr` builders; native joins/semi-joins/anti-joins, CASE, unions, windows | Keep policy/precedence operands visible. Independent conflict/unknown fixtures, not hard-coded empty-success relations. MOD-02/03, EXP-01/02, LOG-01. |
| Owner replacement and overlays | Native base anti-join replaced-owner/tombstone keys plus overlay `UNION ALL` | Include context/family/owner dimensions; empty replacement must withdraw old rows. No bespoke concatenate/take overlay engine. |
| Entity lookup, pattern binding, set composition | Native filter/projection/join/intersect/except/aggregate plans compiled from typed requests | Null, identity, multiplicity, branch provenance, and scoped negation are semantic contracts. |
| Bounded reachability and paths | Native bounded `RecursiveQuery`, joins, distinct/frontier relations, deterministic witness ordering | Bound expansion before output; shortest/all-shortest/simple-bounded paths differ. Petgraph only where the native rung cannot preserve semantics/resource needs. |
| Objective summaries | Built-in aggregation/windows first; UDAF only for genuine new mergeable state | For a custom `GroupsAccumulator`, prove partial/final merge, null handling, memory, and 55's `convert_to_state`. Nested keys alone do not justify a UDAF. |
| Domain scalar operations | Small vectorized UDFs only for missing semantic kernels such as canonical identity | Exact coercion, nullability, volatility, strictness, and truthful optimizer hooks. No row-wise external provider calls. HOF only for actual lambda/list semantics; not a generic rule interpreter. |
| Column and row avoidance | Restore projection optimization safely; preserve predicate/limit pushdown and residual filters through native Delta scans | Test filter-only columns, reordered projections, inexact filters with limits, and actual bytes decoded. F08 requires a narrow fix before global re-enable. |
| Storage pruning | Exact delta-rs provider, full serving statistics, Parquet min/max, row-group/page/bloom/row-filter mechanisms | Use features exposed by the pinned provider; measure real pruning. Do not claim all mechanisms are active merely because generic Parquet supports them. SRC-03/04/08. |
| Top-K and selective search | Native sort/limit, dynamic-filter propagation, sort pushdown, native early stopping | Layout-dependent. Keep residual sort when ordering is inexact. Prove identical canonical order and fewer rows/bytes processed. DF §40A.19/23. |
| Join planning | Honest row/column statistics, proved uniqueness/FDs, native join selection/reordering and `EnsureRequirements` | Never install false constraints to force a plan. Semi/anti joins preserve existential meaning without multiplying rows unnecessarily. |
| Distribution | Measured `target_partitions`, native repartition/coalescing and file work stealing | Share a workspace CPU budget with providers. Range partitioning only with proved compatible ordered boundaries, not as a universal replacement for hashing. DF §40A.6/7/25. |
| Schemas and metadata | Plan-derived Arrow/DF schemas plus executable `SchemaContract`; native casts/physical expression adaptation for storage | Separate annotation from enforcement; preserve logical `FixedSizeBinary(16)` IDs across Delta BINARY. Generic schema seam must preserve projection/statistics mapping. |
| Data representation | `RecordBatch`/`ArrayRef`, typed builders/kernels, shared immutable buffers; dictionary/view encodings where valid | Keep raw/provider strings and repeated tags compact where measured. No forced public schema change for a physical optimization. Tiny retained slices may pin large buffers. Arrow §§4–8. |
| Streaming/resource limits | `SendableRecordBatchStream`, runtime memory reservations/spill, bounded IPC/result channels | Stream rather than eager collect except for explicitly bounded small results. Account non-DataFusion allocations separately. Dropping an engine stream aborts its execution; it does not join unrelated spawned tasks. |
| Plan reuse | Epoch-scoped compiled/optimized logical cache keyed by full semantic/authority dependencies | Fresh physical plans/results per execution. For request-local relations, current cache bypass remains until a parameter/template design proves request/provider isolation and effective keys. |
| Observability and reproducibility | Structured logical/physical tree observations, native metrics, exact dependency environment, optional pinned plan serialization | EXPLAIN strings and physical plan hashes are diagnostics, not semantic identity. Prefer no physical-plan cache; use native proto/Substrait only for an actual supported exchange need. |
| Custom relational algorithms | Typed logical extension with relational children and a bounded physical implementation, only after higher rungs fail | Forward `PhysicalPlanningContext`; expose expressions; recompute properties on child replacement; statistics requests, reset, cancellation, reservations and metrics must be real. PHY-01–11. |
| Public interoperability | Existing Tonic control + Arrow IPC provider/bulk boundary + canonical JSON FastMCP projection | Flight, ADBC, a SQL endpoint, GPU execution, distributed Ballista, or a second graph engine are not required to deliver this product. Reopen only for a measured external need. |

The native engine's streaming contract is documented by [DataFusion 55 `execute_stream`](https://docs.rs/datafusion/55.0.0/datafusion/physical_plan/fn.execute_stream.html); this is the basis for bounded pull-driven integration, not a claim that every join/sort/graph algorithm can emit before consuming its input.

The initial `StatisticsRequest` posture remains v2.3's conservative one: forward supplied requests without inventing a query-aware producer/consumer system. Ordinary exact/inexact/unknown statistics and native pruning come first. A future custom statistics feature needs all of producer, transport/mapping, optimizer consumer, precision, cache key, and plan/result oracle together.

### 3.6 Delta, durable overlays, and finite retention

Keep table roots stable per durable relation history and workspace. Epoch manifests select exact root/version vectors; they do not imply a new physical table for every epoch. Reuse predecessor versions for unchanged relations. Append immutable evidence/events to histories, and persist proof-bearing state required for restart and provenance. Cheap deterministic intermediate plans stay transient unless their producer's durability decision says otherwise.

Choose the mutation from its semantics:

| Change | Preferred realization |
|---|---|
| No changed facts/coverage/dependencies | Reuse exact existing relation version; do not create a ceremonial commit. |
| Complete affected-owner regeneration | Controlled native predicate overwrite (`replaceWhere`) when its contract and physical cost are proved. |
| Empty owner replacement/deletion | Explicit owner tombstone/effective anti-join; certify the exact native empty predicate-overwrite path before relying on it. |
| New evidence/provider run/command event | Governed append with source/operation identity and deduplication policy. |
| Real key upsert | Native merge only with proved key/multiple-match semantics and application retry/session controls; not as an automatic replacement for owner overwrite. |
| Latency-sensitive small update | Durably stage immutable Arrow segments and owner replacement keys; activate their native effective-state view. |
| Overlay consolidation | Controlled new base/version plus equivalence proof, followed by activation; old queries retain old base/segment pins. |
| Full rebuild/schema migration | Explicit full replacement, with predictable cost and a selected successor epoch. |

Native `with_input_plan` + exact `SessionState` + `RequireSessionState` retain engine planning, spilling, and schema behavior inside writes. `with_replace_where` accepts typed expressions at the exact pin. See [pinned WriteBuilder source](https://github.com/delta-io/delta-rs/blob/43a0cf10a313e5077c48637ad786a05359136bbb/crates/core/src/operations/write/mod.rs), Delta reference §§5.8–5.10 and §7.7. Arbitrary row predicates still may rewrite whole shared files; choose file size, clustering, indexed statistics columns, and compaction cadence from the workload. Avoid high-cardinality per-owner directory partitioning by default.

Keep zero-retry command ownership and the single-selector exact read rule. Never combine a supplied snapshot with a contradictory version selector and assume the latter wins. A checkpoint is a physical replay accelerator; the same table version must mean the same facts before and after a checkpoint appears.

For CDF consumers, persist exact applied source interval identity with downstream durable effects. Limit each range/window and stream it under cancellation/backpressure. Checkpoint progress cannot advance before successful downstream publication; a crash afterward but before SQLite update must reconcile to the same result. Missing history or incompatible schema triggers an explicit exact-snapshot rebuild, not “nothing changed.”

Retention must cover selected and leased epochs, result/read leases, unresolved writes, immutable segments, required provider/application releases and expectations, and **CDF intervals with all necessary logs/data/schema eras**. Derive this closure; do not maintain a manual list of safe files. Bound consumer lag or define a rebuild policy. Enable CDF only where a named consumer contract requires it.

Native maintenance remains blocked where it cannot honor the command contract. A controlled full overwrite can preserve final rows while producing delete/insert CDF churn; it is not automatically equivalent to native optimize's `data_change=false`. Prove the intended consumer behavior separately. A narrow upstream/native fix is preferable to implementing a parallel Delta log or file-action planner.

### 3.7 Serving, query composition, and operation ownership

One request is decoded into typed blocks, bindings, inputs, selection/return rules, dependency edges, limits, and scope. Compile to native logical plans in one authorized epoch. Evaluate independent branches concurrently only within a single shared resource envelope. Dependent blocks receive typed results or `NOT_EXECUTED_DEPENDENCY`; the failure of one branch does not erase unrelated successes.

The eight forms need a single canonical entity/fact/path/source model with typed references, not eight unrelated payload shapes. Negation/difference requiring absence proof is admitted only for complete owner/family/context universes. Path results retain ordered edge/fact identity and witnesses; objective summaries retain input coverage and precision. Source retrieval uses exact immutable bytes with independent disclosure authorization and lossless text or byte/base64 output.

Seal one canonical response with QRY's independent status dimensions, deduplicated dictionaries, query results, provenance, errors, snapshot, and limits. Compute delivery after sealing: automatic bounded JSON inline, explicit resource, or policy-governed oversized inline fallback. SRV's existing thresholds are the starting contract, not new arbitrary numbers from this review. A caller asking for inline content should not receive only opaque Arrow descriptors with no explanation.

Tonic/Prost/UDS remain transport. Reuse channels; bound messages, IPC pages, queues and per-request budgets. Preserve lightweight status/cancel/read/release capacity separately from heavy query admission. A connection concurrency limit is not the scheduler. Generated protobuf types terminate at adapters. A `Bytes` payload reduces some copying but does not make cross-process IPC end-to-end zero-copy.

Use a daemon-rooted operation cancellation tree and owned joins for query, source wave, provider, graph, spill, and result materialization tasks. Propagate the remaining deadline with cleanup reserve; cancellation intent is not proof of task termination. Blocking graph/compiler work must be cooperatively interruptible or bounded and supervised. On shutdown, stop admission, cancel/drain, join, reconcile durable outcomes, then release endpoints. Rust gRPC reference §§18–20, 25, 27–28, 37–39 supplies these contracts.

### 3.8 Library decisions

The following IDs are referenced by transition and proof decisions; they are proposals, not installation records.

#### LD-01 — DataFusion: retain-current, improve native visibility

**Decision:** retain-current. **Version basis:** 55.0.0, same Arrow/Parquet universe. **Displaces:** procedural relational transformations, broad optimizer suppression after a safe narrow replacement, redundant materialization of cheap views. Retain the typed compiler and necessary authority checks. **Risk:** metadata-preserving optimization and recursive resource behavior are exact-version-sensitive. **Validation:** `just data-fabric-upgrade-check`, `just datafusion-scan-contract-check`, `just datafusion-plan-schema-cache-check`, expanded with F08's real Delta/nested-view projection oracle and optimized/unoptimized result equality. No unverified upgrade is selected.

#### LD-02 — Arrow: retain-current as the Rust relation boundary

**Decision:** retain-current. **Version basis:** 59.2.0. **Displaces:** repeated row/JSON conversions, unbounded CDF/result collection, and unnecessary buffer copies; keeps explicit small canonical JSON presentation at the daemon boundary. **Risk:** encoding/schema compatibility, retained slices, malformed IPC and memory outside engine reservations. **Validation:** `just exact-provider-batch-check`, `just relation-ipc-provider-operations-check`, `just scheduled-streamed-semantic-query-check` with large/cancelled/slow-consumer cases.

#### LD-03 — Delta: wrap only application transaction semantics

**Decision:** wrap. **Version basis:** 1.0.0 at `43a0cf10a313e5077c48637ad786a05359136bbb`. **Displaces:** whole-fabric replacement where a proved native predicate write suffices; preserves manifest-last multi-table activation, fencing, idempotency, exact pins and retention closure. **Risk:** empty replacements, native hidden retries, CDF churn, and unavailable approved-plan vacuum. **Validation:** `just delta-publication-contract-check`, `just delta-exact-version-reconstruction-check`, `just vacuum-dry-run-check` with the strengthened tests in §6. Destructive native maintenance remains denied until its API contract is proven; an upstream fix/upgrade is a separate decision.

#### LD-04 — Provider libraries: retain-current, complete the selected hybrid

**Decision:** retain-current. **Version basis:** Tree-sitter 0.26.12, Python grammar 0.25.0, Rust grammar 0.24.2; Ruff 0.0.7; Pyrefly 1.2.0 at `1933169ad8ee9e4d4114112eb56ef0811fb0a094`; extractor nightly-2026-08-18. **Displaces:** ad hoc semantic context hashes, permanent external-lane gaps, semantic debug-string parsing, and duplicated unbound analyses. Preserve build/process isolation including Pyrefly's separate Ruff train. **Risk:** private APIs, unobserved recheck scope, compiler trust, and false precision. **Validation:** `just inprocess-provider-lifecycle-check`, `just pyrefly-incremental-lifecycle-check`, `just rustc-provider-lifecycle-check`, `just provider-trust-coverage-remainder-check`, strengthened with effective-context and installed four-provider semantics.

#### LD-05 — notify/gix: adopt the already-pinned lifecycle capabilities

**Decision:** adopt in the production path. **Version basis:** notify-debouncer-full 0.7.0 and gix 0.86.0 with the live narrow read feature profile. **Displaces:** restart-only refresh and unnecessary full repository work for every event. **Risk:** loss/overflow, rename ambiguity, non-UTF8 paths, optional Git acceleration, untrusted config, and capture races. **Validation:** `just git-parity-check`, `just source-capture-race-check`, and an expanded `just lifecycle-production-vertical-check` that mutates the same running daemon's workspace.

#### LD-06 — petgraph: wrap bounded irreducible algorithms

**Decision:** wrap. **Version basis:** 0.8.3, current `std` feature profile. **Displaces:** hand-written SCC/dominator/topological primitives where the native algorithm fits; does not displace Arrow/Delta as graph authority. **Risk:** uninterruptible CPU, quadratic/exponential workloads, multigraph loss in simplified projections, and graph-local index leakage. **Validation:** `just graph-query-resource-operations-check`, `just analysis-producer-semantic-check`, `just analysis-fixed-point-resource-check` with explicit branch/loop/exception and cancellation adversaries. Each selected algorithm records why native relational execution is insufficient.

#### LD-07 — Rust gRPC and FastMCP: retain-current, complete semantic presentation

**Decision:** retain-current. **Version basis:** Tonic/tonic-prost 0.14.6, Prost 0.14.4, repository-owned Tokio and tokio-stream pins, FastMCP 4.0.0 locked adapter. The reference's differing tokio-stream/protoc examples do not override live locks/tooling. **Displaces:** resource-only pseudo-responses and whole-request failure for independent block gaps; preserves UDS authentication and daemon-owned handles. **Risk:** hidden queues, cancellation without join, semantic authority leaking into Python, and insufficient source reauthorization. **Validation:** `just semantic-request-program-check`, `just fastmcp4-stdio-vertical-check`, `just grpc-flow-control-contract-check`, `just fastmcp4-resource-authority-check`, including canonical inline/resource equality and mixed-success DAGs.

### 3.9 Resource ownership, failure, and proof cost

An epoch-local pool is not the aggregate capacity boundary. A workspace coordinator, under process-wide limits, must charge current/candidate/retained epochs, queries, provider processes, graph heaps, caches, spill, source staging, and result/read buffers. Shared immutable buffers have a clear accounting owner; cloning an Arc does not create free capacity or require pretending the bytes were allocated twice. Bound retained epoch count/bytes and refuse or delay new work under pressure while honoring existing leases.

Limit rows **and bytes and individual value/page sizes**. An enormous nested value can exhaust memory with one row. Bound overlay segment count, total bytes, replacement-key size, retained generations, and base-plus-overlay scan amplification; trigger consolidation or backpressure before crossing those bounds. FAB §9.4's durability classification still decides which proof-bearing outputs need Delta histories. Faster activation is not permission to demote them to process memory or unclassified segments.

For resource reads, avoid repeatedly loading and hashing a whole page for each tiny chunk (`streamed_result_registry.rs:654`). A bounded, leased validated-page buffer may amortize that work while preserving per-read authorization and immutable identity; a new general result cache is unnecessary. Account Python `bytearray`, copy, and base64/JSON expansion at `daemon/client.py:1757` separately. The existing page bound makes this a bounded efficiency issue, not an unbounded-memory incident.

| Failure | Required state transition |
|---|---|
| Capture unstable/deferred | Keep affected scope dirty; no complete-source claim. |
| Provider missing, compile fails, trust unavailable | Current source/syntax may activate only with proved withdrawal and explicit permitted gaps. |
| Stale result / changed effective context | Reject publication; cancel/join or reconcile provider state; issue a new correctly bound job. |
| Analysis nonconvergence/resource bound | Emit scoped remainder when the selected profile permits it; otherwise reject candidate. Never promote partial output as complete. |
| Component commit outcome unknown | Stop blind retries; inspect durable operation markers and exact selected control state. |
| Crash before activation selection | Candidate components are unreachable; predecessor remains selected. |
| Crash after selection before installation | Close/terminate serving; reconstruct selected exact state before admission. |
| Retention cannot prove closure | Deny deletion, expose capacity pressure, and pursue a controlled consumer rebuild/lease policy. |
| Query branch failure | Preserve independent results; skip dependents; seal truthful per-block and envelope states. |
| Cancellation/deadline | Stop new work, propagate intent, join owned tasks, reconcile irreversible effects, release reservations/leases, emit one terminal outcome. |

Runtime candidate proof should execute schema/identity checks, ownership/referential invariants, withdrawals, input/provenance closure, permitted coverage, and exact publication readback. It may reuse immutable proof inputs only through complete dependency identity and explicit invalidation. Release certification separately runs independent semantic corpus, causal mutants, clean differentials, containment, recovery, and resource/performance campaigns. Do not weaken P19's re-execution obligation into digest comparison, or interpret it as an instruction to rebuild the entire workspace twice on every keystroke.

For deterministic set comparison, use a bounded relational difference or supported external sort/streaming comparison with explicit schema and identity; do not retain every expanded upstream view solely for proof convenience. Reusing an execution result within one command does not create cross-query result authority. Any proof scheduling change must demonstrate that its falsifying tests still fail.

## 4. Alternatives and clean-sheet challenge

### 4.1 Material alternatives

| Dimension | A: full-rebuild snapshot fabric | B: dependency-maintained relational fabric — recommended | C: separate persistent graph/differential runtime with DataFusion serving |
|---|---|---|---|
| Core idea | Re-extract/recompute and replace complete snapshots for each accepted wave. | Exact provider changes, affected-owner/SCC recomputation, selective versions/segments, one native relational authority. | Maintain graph/derived state in a second specialized engine, project its results into the fabric. |
| Invariant fit | Simple clean-state reasoning, but expensive under edits and still requires honest gaps/activation. | Direct fit with v2.3 ownership, unknowns, native plans, exact epochs, and forward recovery. | Possible in principle, but coherence, provenance, retractions and recovery require a new cross-engine contract. |
| Library leverage | Native DataFusion/Delta for batch work; little provider or state incrementality. | Uses native provider incrementality, relational optimization, Delta exact histories and controlled writes; petgraph is bounded. | May offer finer incremental graph operations; existing DF/Delta proof machinery no longer covers the full semantics. |
| Failure model | Large redo after failure; large candidates but relatively few incremental cases. | Explicit operation markers, scoped withdrawals, exact versions, deterministic rebuild fallback. | Must coordinate two stores/runtimes and reconcile partial cross-engine application. |
| Performance | Cost tied to repository size and number of materialized relations; unsuitable as the only interactive strategy. | Cost normally tied to affected closure; worst-case broad changes remain explicit and bounded. | Potential gain for very frequent small graph deltas, offset by bridges/materialization and duplicate indexing. |
| Proof | Useful independent cold oracle; not sufficient as the primary product. | Independent fixtures plus incremental-versus-clean and fault matrices. | Needs the same oracles plus cross-engine equality, transaction, multiplicity, unknown, and deletion proofs. |
| Transition | Least immediate redesign, but preserves F09/F13 amplification. | Reshapes current startup/persistence/analysis while retaining sound boundaries. | Broad migration and additional runtime/package/security operations. |
| Lock-in/reversibility | Low algorithmic lock-in, high ongoing recomputation cost. | Logical semantics independent of physical layout; replace algorithms behind precise typed boundaries. | Additional engine-specific state/upgrade semantics; reversal requires a complete reconstructible projection. |
| Disposition | Keep as a correctness/recovery oracle, not default update architecture. | Select. Design it clean-sheet, then reuse current pieces only where they fit. | Defer until B misses a declared workload despite measured native optimization. |

Alternative B is not chosen because it resembles existing filenames. Without this implementation, one would still need typed source/context identity, provider isolation, sound invalidation including missing lookups, owner-local analysis, exact immutable state, a single selection event, bounded query execution, and canonical presentation. The existing implementation happens to supply useful portions of those necessities.

A new differential engine would not eliminate the hardest CPG problems: Python dynamic unknowns, Rust compilation contexts, deletion/retraction, negative dependencies, sound alias abstraction, or canonical provenance. Adopt it only if a representative bottleneck remains and its complete maintenance/operational cost is lower. No such measurement was obtained in this review.

### 4.2 Independent challenge and disposition

| Challenge | Resolution in this draft |
|---|---|
| Positive dependency edges miss newly resolvable names/imports | §3.4 requires scoped failed-lookup/namespace/context dependencies or conservative invalidation. Exact schema/producer contracts remain a design blocker before planning closure. |
| “Partial epochs” could excuse incomplete proof | §3.1 distinguishes a complete atomic epoch with explicit capability gaps from missing evidence required to admit it. |
| Monotone fixed points do not imply monotone updates | §3.3/§3.4 require recomputing affected components for removals and SCC changes. A finer retraction engine is deferred. |
| Overlay optimization creates a second storage product | §3.6 uses one native effective-state expression; §3.9 bounds segments/scan amplification and preserves durability classification/retention. |
| Equal physical shape is not equal semantic identity | A generic schema adapter must reject contradictory identity metadata and validate explicit field mappings, including intermediate consumers. Final-output relabeling alone is insufficient. F08 remains a preflight blocker for optimizer restoration. |
| Long-lived providers may leak mutable state or starve under edits | §3.2 requires context ownership, joined cancellation, dependency-correct reuse, watermarks/coalescing, and sustained-churn experiments. |
| One pool per epoch can exceed process capacity | §3.9 establishes aggregate workspace/process accounting and retention pressure. |
| Generalized incremental infrastructure can delay useful product delivery | §5 sequences narrow installed semantic verticals first, with selective owner recomputation before general differential maintenance. |

### 4.3 Staticness and doctrine assessment

| Artifact/contract | Classification and required treatment |
|---|---|
| Released meanings, public allocations, algorithm/precision choices, independent expected facts | Class 1: immutable decisions/releases. Version deliberately; execution must consume them. |
| Exact source images, provider runs, table commits, activation events, completed proof observations | Class 1: completed facts with immutable identity and retention obligations. |
| Current catalog/schema/dependency inventory, selected head, capability, closure, plan membership | Class 2: derive from installed/executed authority. Never hand-maintain a parallel current registry. |
| Dirty scopes, queue progress, leases, retry state, caches | Mutable temporal state, explicitly owned and reconstructible where required; never semantic selection authority. |
| “Implemented,” “complete,” “safe to delete,” “current” without executed evidence | Class 3 if treated as independent declarations: reject or derive. F02/F11/F12 show the risk. |

The architecture aligns well with P1–P16 when typed programs and authority are actually connected, and with P22/P32/P34/P35 through owned boundaries and isolated build domains. Its current weakest areas are P20/P25/P27/P30 (capability and causal semantic proof), P19/P28 (sound incrementality), and P23/P31/P36 (aggregate ownership and executable closure rather than synchronization by convention).

This is a qualitative assessment, not a percentage score. Record individual obligations as **enforced**, **by-convention**, or **unenforced** only from their actual detector. A passing structural check has a narrower evidence envelope than a semantic negative fixture; this review does not certify all 36 principles.

## 5. Transition, cutover, and legacy disposition

### 5.1 Dependency-ordered delivery

This is a design sequence, not a new active packet plan. Preserve v7's historical IDs and evidence; integrate accepted findings through a successor artifact instead of silently editing its acceptance claims.

1. **Truthful capability and semantic anchors.** Close F02's false CFG/precision claims, materialize capture outcomes, distinguish required admission evidence from supported gaps, and preregister independent branch/loop/import/context fixtures. A gap is an honest interim state, not completion of the profile.
2. **Useful installed two-language vertical.** Connect effective context discovery, Tree-sitter for both languages, Ruff, retained Pyrefly, and contained rustc to the actual daemon. Complete entity/fact/source responses for real fixtures through canonical JSON and resources. Fail closed on unsupported/trust cases.
3. **Continuous update owner.** Install watcher/reconciliation, bounded source-wave scheduling, capture/provider generation fences, negative dependencies, withdrawal, and fast/semantic activation. Prove same-process edit/delete/rename and stale-result rejection before claiming real time.
4. **Correct local and common analyses.** Replace sequential Python CFG; integrate true control/dataflow, Rust analyses, and affected SCC summaries. Close required families and precision/remainders against independent semantic examples. Finer graph algorithms must meet resource/cancellation contracts before production wiring.
5. **Selective durable updates and complete query composition.** Reuse stable Delta histories, classify durability, implement affected-owner replacement/overlays, and complete all eight forms/operands/returns with independent branch survival. These can proceed in parallel only where their contracts and file ownership are genuinely disjoint.
6. **Native optimization and finite operation.** Resolve the schema metadata seam, tune projection/pruning/partition/encoding/proof work, stream CDF, establish aggregate resource limits, and complete lease/CDF-safe maintenance. Measure after real semantic workloads exist.
7. **Target-only closure.** Remove superseded shortcuts, run full source-to-FastMCP semantic/recovery/resource certification at one stable revision, and record remaining explicitly unsupported capability boundaries. Do not report success based on packet count or a predecessor proving commit.

Do not hold the command actor through expensive provider computation. A bounded worker can stage a result; the actor revalidates predecessor, ownership, fence, and authority at the durable boundary. A single writer is compatible with parallel parsing/query work and controlled independent staging.

### 5.2 Generated-inventory legacy disposition

The inventory was generated over the following bounded material Rust surfaces using:

```bash
ast-grep outline src/source_image.rs src/git_state.rs src/python_context.rs src/provider_native_syntax.rs src/production_provider_recipe.rs src/tree_sitter_adapter.rs src/ruff_adapter.rs src/ruff_adapter/cfg.rs src/pyrefly_service.rs src/rustc_service.rs src/rust_compilation_trust.rs src/programmatic_derived_analysis.rs src/python_derived_analysis.rs src/rust_mir_derived_analysis.rs src/common_derived_analysis.rs src/fabric/source_wave_command_effect.rs src/fabric/production_workspace_startup.rs src/fabric/programmatic_relation_delta.rs src/fabric/programmatic_observation_delta.rs src/fabric/delta_exact.rs src/fabric/delta_write.rs src/fabric/delta_cdf_replay.rs src/fabric/activation.rs src/fabric/delta_guarded_maintenance.rs src/fabric/graph_program.rs src/fabric/programmatic_schema.rs src/fabric/epoch_runtime.rs src/fabric/datafusion_cache.rs src/fabric/child_session.rs src/relational_program.rs src/production_query_recipe.rs src/fabric/programmatic_ingress_port.rs src/fabric/programmatic_query_backend.rs src/fabric/streamed_result_registry.rs src/query_service.rs src/cancellation.rs src/supervisor.rs pyrefly-sidecar/src/server.rs pyrefly-sidecar/src/pyrefly_link.rs rustc-extractor/src/rustc_link.rs --items exports --json=compact --lang rust

rg --files contracts/rpc codefabric-cpg-mcp/src/codefabric_cpg_mcp .github/workflows tooling/proto | rg '(\.proto$|server.py$|wire_models.py$|daemon/client.py$|ci.yml$|README.md$)'
```

The first command enumerates exported surfaces in 40 explicitly named Rust files; it is not a repository-wide proof of liveness or dead code. The second enumerates the relevant wire/presentation/tooling files. Every inventoried file receives a disposition below. “Reshape” preserves sound semantics, not all existing functions; a later implementation plan must derive its exact symbol/caller write set afresh. Nothing here authorizes deletion now.

| Surface (paths under `src/` unless prefixed otherwise) | Disposition | Reason / exit invariant |
|---|---|---|
| `source_image.rs`, `git_state.rs`, `python_context.rs` | reshape | Preserve safe capture/candidate/context primitives; connect terminal capture outcomes, effective configuration, and generation ownership. F01/F03/F05, LD-05. |
| `tree_sitter_adapter.rs`, `ruff_adapter.rs`, `provider_native_syntax.rs` | reshape | Preserve native extraction and bounded state; reuse it across waves and expose complete native relations/remainders. No borrowed vendor state escapes. LD-02/04. |
| `production_provider_recipe.rs` | reshape | One exhaustive release-owned recipe; replace unconditional missing lanes with actual accepted provider results, preserving genuine gap behavior. |
| `pyrefly_service.rs`, `rustc_service.rs` | reshape | Connect long-lived/provider lifecycle to installed jobs, context isolation and joined cancellation; retain typed IPC and trust failure behavior. |
| `rust_compilation_trust.rs` | preserve | Compilation may execute build scripts/proc macros; keep explicit containment, immutable inputs, private outputs, no credential/network fallback. Validate on host. |
| `pyrefly-sidecar/src/server.rs`, `pyrefly-sidecar/src/pyrefly_link.rs` | reshape | Effective typed settings must configure the checker; complete selected Query/TSP/Glean hybrid without exporting upstream identity. |
| `rustc-extractor/src/rustc_link.rs` | reshape | Preserve typed MIR/private stable-key seam; close required selected enrichments and diagnostics with explicit precision gaps. |
| `ruff_adapter/cfg.rs`, `python_derived_analysis.rs`, `rust_mir_derived_analysis.rs`, `common_derived_analysis.rs` | reshape | Reuse correct analysis semantics as bounded application kernels/transformations; remove duplicate authority and ungoverned CPU/heap work. |
| `programmatic_derived_analysis.rs` | reshape | Keep native builders that express correct semantics; replace file-sequential complete CFG and unjustified precision, expose dependency/withdrawal contracts. |
| `fabric/source_wave_command_effect.rs`, `fabric/production_workspace_startup.rs` | reshape | Connect real source-wave ports; bootstrap becomes lawful genesis of the same evolving pipeline, not a separate rebuild-only product. |
| `fabric/programmatic_relation_delta.rs`, `fabric/programmatic_observation_delta.rs`, `fabric/delta_write.rs` | reshape | Stable histories, explicit durability, unchanged-pin reuse, governed native predicate writes and append. Remove unconditional full-materialization policy after equivalent replacement. |
| `fabric/delta_exact.rs` | reshape | Preserve exact provider selection and full serving stats; add bounded streaming and range-aware retention without raw-Parquet authority. |
| `fabric/delta_cdf_replay.rs` | reshape | Bounded real downstream application and durable range reconciliation; no checkpoint-only semantic progress. |
| `fabric/activation.rs` | preserve | Necessary multi-table selection/readback/fence semantics; no replacement by a mutable latest pointer. |
| `fabric/delta_guarded_maintenance.rs` | reshape | Preserve deny-by-default safety; range/lease closure and native approved-operation feasibility must precede destructive enablement. |
| `fabric/programmatic_schema.rs` | reshape | Preserve plan-derived schemas/observations; narrow metadata compatibility boundary. Temporary global projection exclusions exit only after F08's full native-path oracle succeeds. |
| `fabric/epoch_runtime.rs`, `fabric/datafusion_cache.rs` | reshape | Keep bounded native runtime/logical caching; add measured profiles and aggregate accounting; retain exact authority keys. |
| `fabric/child_session.rs` | preserve | Keep reduced catalogs/bound-authority checks and query-local cache isolation; optimize construction/reuse only with isolation proof. |
| `relational_program.rs`, `production_query_recipe.rs`, `fabric/programmatic_ingress_port.rs`, `fabric/programmatic_query_backend.rs` | reshape | One compiler must execute the complete typed eight-form contract and per-branch results, not just validate declarations. |
| `fabric/graph_program.rs` | reshape | Demand-root native graph plans, distance/predecessor witnesses, true bounds/cancellation; no second persistent graph authority. |
| `fabric/streamed_result_registry.rs` | reshape | Keep immutable packages and reauthorization; amortize bounded page reads without leaking authority or extending leases. |
| `query_service.rs`, `cancellation.rs`, `supervisor.rs` | preserve | Keep singleton, authenticated service, separate control admission and owned shutdown; extend proof to real installed workload. |
| Adapter `server.py`, `contracts/wire_models.py`, `daemon/client.py` | reshape | Complete canonical response/delivery projection; account chunk/encoding memory; Python remains presentation/transport only. |
| `contracts/rpc/{cpg_query_service,provider_control,pyrefly_sidecar,rustc_extractor}.proto` | reshape | Only deliberate additive/versioned contract changes required for full meanings; preserve released allocations and generated single-source discipline. |
| `tooling/proto/README.md`, `.github/workflows/ci.yml` | preserve | Preserve generator identity and multi-domain assurance; add real workload coverage through existing gate ownership, not manual green ledgers. |

Obsolete sequential-CFG “complete” behavior, Python-only production selection, unconditional provider gaps, full-replace-everything policy, narrow phrase-only form implementation, and resource-only pseudo-response are **replacement targets**, not compatibility commitments. Delete each obsolete path in the same cutover that proves its replacement. Do not keep user-selectable old/new semantic authority, hidden fallback constructors, or dual durable writes.

### 5.3 Cutover and recovery

The current deployment profile is target-only FreshActivation. Extend that authority; do not revive bootstrap/model replay or any historical suite. A fresh workspace uses lawful genesis through the same actor. An existing v2.3 epoch advances only through exact predecessor validation and a proved successor; immutable earlier queries/results retain their pins.

If a real externally deployed predecessor requiring data migration is discovered, stop and design its handoff explicitly. Do not infer a migration or permission to delete data from this review. Runtime rollback-to-predecessor, dual serving authorities, and silent downgrade are outside the accepted target. Repair remains forward-only.

The temporary schema optimizer exclusion has a bounded purpose: preserve current exact schema contracts while the narrow metadata seam is proved. Its latest safe removal point is the native-optimization milestone before claiming optimized fabric performance. Do not delete the workaround merely to satisfy a textual zero-state scan.

## 6. Proof strategy and readiness

### 6.1 Acceptance matrix

These are **required expansions of existing named commands**, verified present in `just --list`. Their names are not assertions that current tests already cover these cases. A test must select at least one real case and demonstrate rejection of its falsifying mutant.

| Obligation | Existing command owner(s) | Required positive evidence | Falsifying case |
|---|---|---|---|
| Same-process continuous Python/Rust updates — F01/F05 | `just lifecycle-production-vertical-check`; `just real_source_to_fastmcp_causal_vertical` | Same PID, real edit/add/delete/rename, new selected epochs and decoded changed records; clean-equivalent final state | Disable watcher/commit port; lose event during reconciliation; return delayed old provider output. |
| Source/Git correctness — F05, LD-05 | `just source-capture-race-check`; `just git-parity-check` | Generic and accelerated inventories converge, including non-UTF8 paths, atomic saves, ignored/untracked files, linked worktrees | Constant fence, unstable read, excluded source omitted from coverage, gix candidate omission. |
| Effective provider context — F03/F04 | `just pyrefly-incremental-lifecycle-check`; `just exact-provider-batch-check` | Hold source constant; change selected stub/import root/platform/version; resolved facts follow effective settings | Manifest hash changes but checker still uses default settings. |
| Safe real Rust semantics — F04, LD-04 | `just rustc-provider-lifecycle-check`; `just semantic-sandbox-host-matrix-check` | Actual contained extraction, selected MIR/profile facts, exact inputs; failure retains syntax and semantic gap | Host fallback, proc-macro escape, mixed dependency input, failed compile retaining stale-current facts. |
| Correct Python CFG/dataflow — F02 | `just analysis-producer-semantic-check`; `just analysis-causal-fault-check` | Independently expected owner boundaries, branches, loop backs, return, finally, short circuit, with/await and exact small dataflow sets | Replace real successor with AST adjacency; erase exception/back edge; merge nested callable owners. |
| Sound incrementality and unknowns | `just inprocess-provider-lifecycle-check`; `just provider-trust-coverage-remainder-check`; `just query-unknown-negative-proof-check` | Clean/incremental equality for positive and negative dependencies, unresolved-to-resolved imports, SCC splits and unknown-call changes | Add newly visible candidate outside previous positive edges; keep old summaries after deletion. |
| Eight forms and operand causality — F06/F07 | `just semantic-request-program-check`; `just semantic-release-vertical-check` | Installed decoded facts for every form and supported operand/return; independent success survives failed sibling; dependent is skipped | Remove a producer/form; mutate direction/distance/name/return; fail every block on one gap. |
| Canonical agent delivery — F07, LD-07 | `just fastmcp4-stdio-vertical-check`; `just fastmcp4-resource-authority-check` | Inline/automatic/resource canonical object equality, full status dimensions, source policy and limits | Ignore delivery, omit unknowns, accept a dangling dictionary ID, disclose source through a fact-only grant. |
| Projection/schema/native visibility — F08, LD-01 | `just datafusion-plan-schema-cache-check`; `just datafusion-scan-contract-check`; `just provider-statistics-contract-check` | Real nested Delta views, alias/join/aggregate/projection rewrites preserve values, field identity, filters, statistics and limits | Equal-shaped swapped identity, filter-only column removed, inexact predicate with prematurely pushed limit. |
| Selective durable updates — F09, LD-03 | `just delta-publication-contract-check`; `just delta-exact-version-reconstruction-check` | Affected owner/family versions advance; unrelated pins identical; empty replacement removes old rows; actual bytes/files/commits measured | Unchanged relation rewrite, forgotten tombstone, mixed root/version, session fallback. |
| Activation and crash safety | `just activation-fault-matrix-check`; `just fabric-activation-recovery-check`; `just fabric-epoch-pinning-check` | Crash at each component/selection/swap boundary; reopen only selected exact vector; old leases remain stable | Admit after durable selection but before coherent installation; select by timestamp/latest or stale writer. |
| CDF progress/retention — F10/F11 | `just delta-exact-reconstruction-v3-check`; `just vacuum-dry-run-check` | Large bounded CDF range, interrupted downstream commit, checkpoint loss, schema-era change, protected lag interval | Advance checkpoint early; remove interior log/CDF file; treat expired history as empty success. |
| Graph resources and paths — F14, LD-06 | `just graph-query-resource-operations-check`; `just analysis-fixed-point-resource-check` | Root-scoped chains/diamonds/cycles/parallel edges/isolated vertices; correct shortest/all-shortest/bounded-simple witnesses and intermediate resource bounds | All-pairs seed for one-root request; path-prefix explosion; cancel only before starting. |
| Aggregate ownership/cancellation — F13/F14 | `just cancellation-tree-check`; `just grpc-flow-control-contract-check`; `just grpc-slow-consumer-check` | Cancel real active provider/graph/DF work, joined tasks, reclaimed reservations; control responsive under full admission and many leased epochs | Detached CPU task; pending stream never polls token; per-epoch pool multiplication exceeds global bound. |
| No obsolete authority — §5 | `just provider-type-boundary-check`; `just compiled-release-legacy-zero-state-check`; `just remaining-legacy-zero-state-check` | Target-positive behavior plus structural/caller absence of superseded semantic/writer routes | Reintroduce old constructor/registry/empty-success bypass; selector finds zero cases but passes. |
| Release evidence — F12 | `just ci-fast`; `just ci-pr`; relevant current certification after accepted plan integration | Exact retained domains/features, independent semantics, resources, recovery and decommission all pass at one stable candidate | Reuse stale measurements/proving commits after implementation drift; skip a required domain/oracle. |

The existing v7 certification is plan-specific. A successor implementation needs a derived terminal gate that includes the accepted new obligations, not a claim that old packet completion automatically certifies them.

### 6.2 Independent semantic corpus

Author expected canonical facts separately from production builders. Include at least:

- Python scopes/rebinding/import aliases/re-exports, `.py`/`.pyi`, descriptors/properties/MRO, decorators, generics, comprehensions, pattern binding, closures, indirect calls and explicit dynamic unknowns.
- Python owner-local branches/loops/abrupt completion/exception cleanup/context managers/generator and async transitions; small expected reaching-definition/liveness/effect/resource results.
- Rust generic definitions versus instances, direct/trait/dynamic/function-pointer calls, normal/unwind MIR edges, projections/moves/borrows/reborrows/drops, macros/generated spans, async lowering, unsafe/FFI boundaries and missing external bodies.
- Broken source, failed compilation, withheld provider output, wrong context, incomplete IPC, changed dependency/stub/config, and unsupported requested precision.
- Graph diamonds, cycles, unreachable and isolated nodes, multiple exits, parallel facts, equal-length witnesses, deletions, SCC split/merge, and a formerly unknown lookup acquiring candidates.

Two equivalent executions can agree on the same wrong algorithm. Therefore incremental-versus-clean equality is necessary but not sufficient. Independent fixtures and seeded causal mutations must distinguish the actual expected graph from a plausible schema-correct result.

### 6.3 Performance experiments

Measure the entire installed source-to-agent path and each stage separately. Use a declared small/medium/large corpus, cold and warm conditions, Python/Rust/mixed workspaces, high-fanout imports/calls, sparse/dense graphs, and a fixed edit while total repository size grows. Track:

- Event age and reconcile backlog, captured bytes, providers invoked and actual work/rechecks where observable.
- Affected owners/relations versus total owners/relations; no-op and unrelated-pin reuse.
- Logical/physical planning time, scan projection, rows/bytes decoded, row groups/files skipped, recursive iterations/frontier size, intermediate cardinality and spills.
- Provider/daemon RSS, native reservation peaks, graph/IPC/result allocations, CPU saturation, retained epoch/source/cache bytes, and disk headroom.
- Commits, files/bytes rewritten, overlay depth/scan amplification, CDF events, checkpoint/reopen latency, and consolidation cost.
- First meaningful result, activation and semantic freshness latency, inline/resource delivery cost, slow-consumer queues, and cancellation-to-joined-cleanup time.

Run controlled comparisons of full replacement versus owner replacement versus bounded segments; safe native projection rules versus the current workaround; target partitions/batch sizes; plain versus supported dictionary/view encodings; pruning/statistics/layout combinations; and repeated upstream proof evaluation versus dependency-aware execution reuse. Change one factor at a time or use a declared experimental design. Publish raw samples and reproducible workload identities. Do not copy generic lakehouse file-size advice into local interactive defaults without measuring.

`just semantic-profile-bench` is a starting diagnostic; `just compiled-release-resource-performance-check` is tied to existing preregistered evidence and rejects implementation drift. New capture is an explicit mutating workflow, not something this review performs or silently approves.

### 6.4 Open design blockers and preflight assumptions

These are actionable boundaries for the next design integration, not reasons to abandon the architecture.

| Item | Classification | Required closure |
|---|---|---|
| Dependency completeness for missing lookups and withdrawals | Design blocker | Specify typed support/lookup/namespace/context dependencies and conservative widening rules for each provider/analysis family. Prove newly resolvable and deletion cases. |
| Syntax-current versus semantic-required admission | Design blocker | Define immutable capability-state obligations distinguishing a proved gap from missing evidence required to admit the epoch; reconcile LIFE §§6–7 explicitly. |
| Correct Python CFG and analysis precision | Design blocker | Specify owner/control/evaluation semantics and the independent semantic corpus; a sequential complete projection is not an admissible target. |
| Generic schema identity under optimization | Design blocker for optimized-path acceptance | A field-map/identity contract must distinguish missing metadata from conflicting metadata and survive all relevant phases. Select the narrow exact-pin implementation only after the native-path probe. |
| Aggregate resource and retained-epoch policy | Design blocker | Choose owners, global/workspace envelopes, charge/release semantics, overlay bounds, admission aging, and exhaustion behavior across processes and epochs. |
| Empty predicate overwrite and native maintenance | Preflight assumption A01 | Exact source confirms the builder surfaces; source inspection alone has not executed empty-input replacement. Prove it or use explicit tombstone/effective-view semantics. Maintenance remains disabled until retry/transaction/approved-set guarantees hold. |
| Long-lived provider speedup and incremental closure precision | Measurement assumption A02 | Show semantic equality, context isolation, cancellation/join, and sustained-edit progress before claiming latency benefit. |
| Proposed latency/throughput and layout choices | Measurement assumption A03 | Select the reference machine/corpus and remeasure the complete production topology; §2.3 values are proposals only. |

The most useful next action is a focused, versioned integration of these blockers into the v2.3-derived target and a dependency-closed successor plan, beginning with truthful semantics and the real two-language update vertical. It is **not** a blind dependency upgrade, a graph-database pivot, or another broad registry/gate layer.

### 6.5 Evidence envelope and review verification

The review inspected the live exact manifests, all six requested skill entrypoints, the full v2 doctrine, routed reference sections, the relevant v2.3 masters, targeted definitions/callers/construction sites, and the bounded export inventory above. Library-sensitive assertions used exact local source or version-pinned documentation; online main-site material was not used to select a new pin. Load-bearing Delta maintenance behavior was checked against the pinned checkout, not guessed from a high-level API name.

Reproducible discovery includes `just --list`, `just spec-outline <master> --match <section>`, the inventory commands in §5.2, and scoped `rg`/`ast-grep run` for watcher construction, provider calls, source-wave port implementations, `Advance` construction, query-form recipes, and graph-helper callers. Searches exclude unrelated generated/vendor/build trees and cannot prove dynamic or out-of-scope callers absent. No broad source count or zero text match is promoted to behavioral certification.

Final document verification confirmed the required frontmatter, six core sections, 14 finding IDs, seven library-decision IDs, the linked status report, and all 43 inline named recipes against `just --summary`. Cited v2.3 section titles were checked with `just spec-outline`. Scoped `typos` and new-file whitespace checks passed. `just artifacts-check` passed its 21 checker tests and active v7 plan/state validation; that command does **not** validate this new dossier or certify its proposed behavior. The scoped implementation diff still matches the frontmatter digest. The report's findings remain qualified to this baseline; recompute that identity before acting on them.

**Readiness:** recommended architectural direction, draft review artifact; the blockers above remain before acceptance for implementation planning.

not ready — design blockers remain
