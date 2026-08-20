# gix + notify Lifecycle — Detailed Reference

This document is the deep-dive companion to `SKILL.md` in the same directory. SKILL.md carries the core map (version anchors, the two-document topic table, reading strategy, where-to-look routing, and the seven key invariants); this file carries the full **per-document section indexes**, the **authority ladder** and **role split** quoted from the lifecycle spec, the **permitted/forbidden gix surface map**, the **decision trees**, the **full 18 operating rules**, and the **CodeFabric project context**.

Cross-references back into the core map are written `SKILL §...`. Read SKILL.md first; reach here once you know which document you need, when a routing choice gets hard, or — most often — when you need to know whether a gix capability you just found is one this project is allowed to use.

These two docs are the most grep-hostile in `docs/library_ref/`: notify changes its own heading prefix mid-document, its table of contents disagrees with its chapter titles, gix has a lettered chapter that sorts before §0, and gix shadows its own subsection numbering in 28 places. Operating Rules 13-17 cover each. Prefer `lib-outline`, which is markdown-aware, and use the line numbers below with `Read(offset, limit)`.

## Table of Contents

- **§1 — Per-document section indexes**
  - §1.1 `notify_debouncer_full_rust_reference.md` (§0-§40), with sub-indexes for §29, §33, §40
  - §1.2 `gix_rust_advanced_reference.md` (§0-§50 + §1A + Appendices A-F)
- **§2 — Authority, policy surface, and handoff** (authority ladder · role split · permitted/forbidden gix surface · pipeline seam)
- **§3 — Decision trees** (which library · gix policy · event handling · backend · rename · loss recovery · status cost · path type)
- **§4 — Operating rules** (full set of 18)
- **§5 — Project context: CodeFabric**

---

## §1 — Per-document section indexes

Line numbers are **start lines**, verified against the source files, for use as `Read(offset, ...)`. Deep-dive rows are **bolded**; front-matter, appendix, and tail rows are plain.

### §1.1 `notify_debouncer_full_rust_reference.md` — section index (§0-§40)

10,307 lines, 28% blank. Front matter `1-229`; deep-dives start at line `230`. **The prefix changes mid-document**: §0-§8 are `` # `notify-debouncer-full` Advanced — N) `` (backticked) and §9-§40 are `# notify-debouncer-full Advanced — N) ` (bare) — Rule 13. Nearly every chapter ends with an anti-pattern section then a checklist. No appendices; the doc closes with two unnumbered tail chapters. **Do not build an index from the doc's own map at line 50** — its titles differ from the real chapter titles for 24 of 41 chapters (Rule 14); the titles below are the chapter headings.

| § | Line | Title | Key subsections |
|---|------|-------|-----------------|
| — | 1 | Front matter | `## Version / source anchors` (9) · `## Feature inventory` (24) |
| — | 50 | Proposed comprehensive documentation map | `## 0)`-`## 40)` — **titles diverge from chapters for §9-§28, §32, §35, §38, §39** |
| — | 218 | Suggested reading / implementation order | 7 phases |
| **0** | 230 | Scope, versioning, and mental model | 0.1 what the crate is / is not · 0.3 canonical pipeline · 0.4 vocabulary · **0.5 events are hints about state transitions** · **0.6 LLM-agent invariants** · 0.7 anti-patterns |
| **1** | 427 | Installation, crate selection, and project layout | 1.1 production version policy · 1.2 when a direct `notify` dep is justified · 1.3 feature flags · 1.4 MSRV · 1.5 workspace layout for a code-intelligence service · **1.6 application-owned event facade** · 1.8 checklist |
| **2** | 631 | First executable Rust watcher | 2.1 callback · 2.2 channel · 2.3 custom handler · 2.4 multiple roots · **2.5 deterministic shutdown skeleton** · 2.7 anti-patterns |
| **3** | 835 | Core type system and vocabulary | 3.1 `Debouncer<T, C>` · 3.2 `DebouncedEvent` · 3.3 `DebounceEventResult` · 3.4 `DebounceEventHandler` · 3.5 `notify::Event` · 3.6 `EventKind` · **3.7 type decision table** · 3.8 checklist |
| **4** | 1016 | Debounce timing semantics | **4.0 the most important timing correction** · 4.1 timing pipeline · 4.3 continuous writes · **4.4 `tick_rate` validation (tick ≤ timeout)** · 4.5 timeout by workload · 4.6 timeout vs downstream batching · 4.8 anti-patterns |
| **5** | 1215 | Event queueing, coalescing, and ordering | 5.0 internal queue model · 5.1 eligibility extraction · 5.2-5.6 coalescing rules (duplicate kind · duplicate create · modify-after-create · create-then-remove · directory removal) · 5.7 cross-path ordering · **5.8 queue semantics decision table** · **5.9 dedupe again at domain level** |
| **6** | 1388 | Rename normalization | 6.1 correlation strategy · **6.2 `RenameMode` meanings** · 6.3 chained renames · 6.4 target override · 6.5 move-in/move-out · 6.6 extraction helper · 6.7 rename-aware index update · 6.8 anti-patterns |
| **7** | 1557 | File-ID caches | 7.1 platform default · **7.2 `FileIdCache` contract** · 7.3 `FileIdMap` · 7.4 `NoCache` · 7.6 custom cache use case · 7.7 rescan behavior · 7.8 cost model · 7.9 anti-patterns |
| **8** | 1729 | Watch roots and lifecycle | 8.1 recursive vs non-recursive · 8.2 duplicate roots · 8.3 unwatch · **8.4 parent deletion caveat** · 8.5 deprecated access · 8.6 owner-service pattern · 8.7 roots for a Rust monorepo |
| **9** | 1875 | Handler delivery and output integration | 9.1 `DebounceEventResult` · 9.2 closure handler · 9.4 `mpsc` sender as handler · **9.5 bounded vs unbounded handoff** · **9.6 handler-latency budget** · 9.7 panic policy · 9.8 anti-patterns |
| **10** | 2154 | Event taxonomy and semantic interpretation | 10.1 top-level `EventKind` · **10.2 match broad semantics before fine** · 10.3 rename representation · 10.5 `DebouncedEvent::time` · 10.6 event → intent mapping · **10.7 why event kind is not your source of truth** |
| **11** | 2403 | Rescan semantics and loss recovery | **11.0 core invariant** · 11.2 detecting a rescan event · **11.3 reconciliation algorithm for code intelligence** · **11.4 generation fence** · 11.5 digest-based reconciliation · 11.6 scope · 11.7 state machine · 11.10 checklist |
| **12** | 2624 | Error model and diagnostics | 12.0 error channels · 12.2 watch-registration errors · 12.4 error context · **12.5 Linux `ENOSPC` is usually watch exhaustion** · 12.6 severity policy · 12.7 diagnostics snapshot |
| **13** | 2833 | Custom construction with `new_debouncer_opt` | 13.0 when to leave `new_debouncer` · 13.2 custom poll watcher · **13.3 `notify::Config` options that matter** (poll interval · compare contents · follow symlinks) · **13.4 poll interval is not debounce timeout** · 13.6 cache selection · 13.8 config profiles |
| **14** | 3098 | Backend selection and platform model | 14.0 recommended aliases · **14.1 backend selection is a deployment property** · **14.2 backend capabilities matrix** · 14.4 fallback strategy · 14.5 macOS backend feature choice · 14.6 acceptance test |
| **15** | 3265 | Linux / inotify deployment deep dive | 15.1 recursive-watch resource pressure · 15.2 watch-root minimization · **15.3 queue overflow and event loss** · 15.4 directory removal · 15.5 parent deletion · 15.6 WSL / mounted filesystems |
| **16** | 3407 | macOS / FSEvents deployment deep dive | 16.1 startup/cache cost · 16.2 ownership/security caveat · 16.3 FSEvents vs kqueue · **16.4 atomic-save behavior** |
| **17** | 3505 | Windows / ReadDirectoryChangesW deployment deep dive | 17.1 path representation · **17.2 file identity and rename correlation** · 17.3 network shares · 17.4 antivirus/indexer contention |
| **18** | 3601 | PollWatcher and non-native environments | **18.1 when polling is the right choice** · 18.2 latency model · 18.4 `compare_contents` · **18.5 polling and debouncing are complementary** · 18.6 interval tuning · 18.7 adaptive polling |
| **19** | 3786 | Filtering, ignore rules, and scope reduction | **19.1 root-level vs event-level filtering** · 19.2 path-filter function · 19.3 gitignore-compatible filtering · 19.4 directory-event pruning · **19.5 rename across filter boundaries** · 19.6 extension changes on rename · 19.8 filter ordering |
| **20** | 4066 | Tokio and async integration | **20.0 core rule** · 20.1 unbounded bridge · 20.2 bounded + `try_send` · **20.3 dirty-set bridge: preferred for code indexing** · 20.4 `spawn_blocking` and parsing · 20.5 cancellation and stale work |
| **21** | 4287 | Backpressure, burst control, and work coalescing | **21.0 three independent queues** · 21.2 dirty set vs FIFO · 21.3 batch draining · **21.4 burst-to-rescan threshold** · **21.5 backlog age beats queue length** · 21.6 fairness across workspaces |
| **22** | 4456 | Editor save patterns and logical-change interpretation | 22.1 in-place save · **22.2 atomic replace save** · 22.3 formatter-on-save · 22.4 language-server/build-tool writes · 22.5 swap and backup files · 22.6 acceptance tests |
| **23** | 4608 | Path identity, normalization, symlinks, and races | **23.0 path is not file identity** · 23.1 preserve `PathBuf` · 23.2 absolute vs relative keys · **23.3 `canonicalize()` caveat** · 23.5 symlinks · 23.6 TOCTOU · 23.7 root escape validation |
| **24** | 4803 | **Code-intelligence / property-graph architecture** | 24.0 end-to-end model · 24.1 state model · 24.2 change pipeline · **24.3 semantic vs content invalidation** · 24.4 two-stage invalidation · **24.5 atomic graph commit** · 24.6 config files · 24.7 `need_rescan()` in a graph system · 24.8 canonical `FileDelta` · **24.9 high-confidence architecture rule** |
| **25** | 5112 | Hot reload, build triggers, and process restart | 25.1 simple command trigger · 25.2 restart-after-success · 25.3 full debouncer vs watchexec |
| **26** | 5199 | Large repositories and performance engineering | 26.0 cost stack · 26.1 startup benchmark · 26.2 high-churn directory exclusion · 26.3 timeout tuning benchmark · 26.4 hash strategy · 26.6 graph commit batching |
| **27** | 5382 | Multiple workspaces, watcher ownership, isolation | 27.0 one-debouncer-many-roots vs one-per-workspace · **27.1 choose isolation domain intentionally** · 27.2 root overlap · 27.3 routing · 27.4 per-workspace health · 27.5 dynamic lifecycle |
| **28** | 5534 | Metrics, tracing, and observability | 28.2 core counters · 28.3 event-kind counters · 28.4 latency histograms · 28.5 backlog gauges · **28.7 sampling paths safely** · 28.8 health status |
| **29** | 5730 | Testing and correctness (618 lines) | see §1.1a below |
| **30** | 6348 | Shutdown and resource lifecycle | 30.1 `stop(self)` · **30.2 `stop_nonblocking(self)`** · 30.3 `Drop` · 30.4 ordering with channels · 30.5 drain vs cancel · **30.6 shutdown with reconciliation in progress** · 30.8 tick-rate implication |
| **31** | 6579 | Security and resource governance | 31.1 root authorization · **31.2 symlink escape** · 31.3 watch exhaustion as resource attack · 31.4 poll amplification · 31.6 path disclosure · 31.9 TOCTOU · 31.10 privilege separation |
| **32** | 6838 | Stable application wrapper and API design | **32.1 recommended public abstraction** · 32.2 wrapper responsibilities · 32.4 stable service interface · 32.6 event classifier module · **32.7 do not over-normalize in wrapper** · 32.8 version isolation · 32.9 test fake |
| **33** | 7098 | Troubleshooting cookbook (453 lines) | see §1.1b below |
| **34** | 7551 | Upgrades, prereleases, and compatibility | **34.1 why pin this crate** · 34.2 stable 0.7.0 anchor · **34.3 0.8 prerelease differences** · 34.4 create→remove migration · 34.5 `notify` 8 → 9 coupling · 34.9 serialization is not a wire contract · 34.12 upgrade test matrix |
| **35** | 7898 | Comparison with adjacent crates and abstractions | **35.0 selection map** · 35.1 bare `notify` · 35.2 `notify-debouncer-mini` · 35.3 `watchexec` · 35.4 direct `inotify` · **35.7 comparison table** · **35.8 decision tree for code intelligence** · 35.9-35.10 migrations |
| **36** | 8198 | Production deployment patterns | **36.0 pattern map** · 36.3 IDE / code-intelligence service · 36.6 multi-tenant remote indexing · 36.7 container with bind mount · 36.8 network-mounted workspace · 36.11 startup readiness · **36.12 failure mode matrix** |
| **37** | 8520 | Best-practice rules | **`37.0` holds 30 `### Rule N —` H3 entries** (8524-8792), not `##` subsections · 37.1 compact architecture doctrine |
| **38** | 8809 | Cross-cutting anti-pattern inventory | 13 buckets: timing · event semantics · consistency · async scheduling · filtering · path/identity · backend/platform · cache/rename · lifecycle · testing · security · upgrade · **38.12 code-intelligence mistakes** |
| **39** | 8979 | Production implementation and review checklist | 20 buckets: dependency · construction · polling · roots · path/security · handler · event interpretation · filtering · **39.8 current-state resolution** · async · **39.10 reconciliation** · **39.11 code-intelligence graph updates** · platforms · tests · observability · security · shutdown · upgrade |
| **40** | 9276 | API quick reference and canonical recipes (959 lines) | see §1.1c below |
| — | 10235 | Source anchors | `[S1]`-`[S21]` link definitions |
| — | 10283 | Closing implementation stance | closing doctrine |

#### §1.1a `notify §29` — testing sub-index (5730-6347)

`29.0` philosophy 5732 · `29.1` test layers 5752 · `29.2` pure event classifier 5790 · `29.3` temp-dir integration harness 5820 · **`29.4` avoid fixed sleeps as assertions 5881** · `29.5` create 5909 · `29.6` modify 5931 · `29.7` rename normalization 5952 · `29.8` chained rename 5980 · `29.9` rename-over-target 6003 · `29.10` directory deletion 6026 · **`29.11` atomic-save 6048** · `29.12` formatter-on-save 6070 · `29.13` filter-boundary rename 6091 · `29.14` initial state 6106 · **`29.15` rescan path without kernel overflow 6122** · `29.16` app queue overflow 6139 · `29.17` root deletion/recreation 6158 · `29.18` PollWatcher profile 6182 · `29.19` cross-platform CI matrix 6201 · `29.20` timing assertions 6230 · `29.21` deterministic event sorting 6249 · `29.22` eventual-assertion helper 6257 · **`29.23` correctness oracle 6281** · `29.24` anti-patterns 6308 · `29.25` checklist 6325

#### §1.1b `notify §33` — troubleshooting cookbook, symptom-indexed (7098-7550)

`33.0` diagnostic order 7100 · no events 7120 · long delay 7153 · too many events 7180 · modify missing after create 7206 · duplicate create missing 7220 · rename not `Both` 7228 · rename target strange 7253 · children remain after dir delete 7271 · **`ENOSPC` on Linux watch 7286** · NFS/shared mount 7307 · WSL `/mnt/c` 7322 · container on macOS host 7330 · macOS missing files 7338 · PollWatcher CPU/IO 7353 · PollWatcher misses content 7369 · handler stops after consumer shutdown 7377 · shutdown blocks 7393 · callback touches state after shutdown 7410 · **index reverts to old source 7418** · **branch checkout leaves graph inconsistent 7432** · ignored files consume resources 7448 · slow file-ID cache startup 7456 · paths outside repository 7464 · `tick_rate > timeout` error 7472 · timeout does not wait for quiet 7486 · capture bundle 7494 · anti-patterns 7521 · checklist 7535

#### §1.1c `notify §40` — API reference and recipes A-T (9276-10234)

`40.0` core dependency 9278 · `40.1` imports 9301 · `40.2` type signatures 9334 · `40.3` `new_debouncer` 9361 · `40.4` `new_debouncer_opt` 9391 · `40.5` `Debouncer` methods 9419. **Recipes map A→`40.6` … T→`40.25`:** A smallest recursive watcher 9447 · B channel handoff 9507 · C detect normalized rename 9568 · D classify into invalidations 9600 · **E rename inclusion truth table 9651** · F current-state resolver 9689 · **G dirty-path set 9715** · **H Tokio bridge with overflow recovery 9751** · **I generation fence 9787** · J rescan flag handling 9825 · K custom `PollWatcher` 9849 · L symlink policy 9894 · M service owner + deterministic shutdown 9907 · **N initial reconciliation + startup ordering 9944** · O final-state reconciliation 9975 · P directory removal cascade 10003 · Q backend health snapshot 10020 · R platform-independent semantic test 10040 · S timeout/tick benchmark matrix 10068 · **T complete architecture for a code-property graph 10104** · `40.26` final agent invariants 10181

### §1.2 `gix_rust_advanced_reference.md` — section index (§0-§50 + §1A + Appendices A-F)

11,186 lines, **52.87% blank** (double-spaced, including table rows) — a `Read` window holds roughly half the content it appears to, so budget about twice the `limit` you would use on notify. Front matter `1-230`; deep-dives use `# gix Advanced — N) ` (**lowercase `gix`**, em dash). **§1A sorts before §0** (line 231 vs 357), is the only lettered chapter, and is absent from the doc's own map — Rule 15.

**Every numbered chapter carries the identical eleven-subsection skeleton**, so this index needs no per-chapter subsection column; memorise the skeleton once and jump straight to the `N.M` you want:

```
N.0  Scope and mental model          N.6   Testing matrix
N.1  Key public surface              N.7   Deployment advisory
N.2  Canonical [selection] pattern   N.8   Anti-pattern inventory
N.3  Value cases                     N.9   Agent checklist
N.4  Planning and ownership rules    N.10  Primary sources
N.5  Failure modes and edge cases
```

Only `N.2` varies ("Canonical pattern" in §0-4, 7, 8, 13, 14, 20, 26, 30, 32, 33, 39, 44; "Canonical selection pattern" elsewhere). **Caution:** ten chapters embed a `### N.M` ladder inside `N.0` that reuses these numbers with different titles — 28 collision pairs. Anchor on exactly `^## ` (Rule 16).

**Policy column** — CodeFabric may use only part of this document. `✅` in policy · `◐` capability-level split, read the note · `🕰` history, excluded ontology (lifecycle §81) · `⛔` forbidden behavior (§38, §82-§84). Full reasoning in §2 table 3.

| § | Line | Title | Policy |
|---|------|-------|--------|
| — | 1 | Front matter | `## Version / source anchors` (9) · `### Source-of-truth hierarchy` (21) · `## Feature inventory` (77) |
| — | 93 | Proposed comprehensive documentation map | `## 0)`-`## 50)`; titles identical to chapters, so content greps double-hit. **§1A is not listed** |
| — | 205 | Suggested expansion / reading order | 8 phases |
| **1A** | 231 | Release-pinned feature flag catalog | ✅ — **read first.** One 126-line table of all 55 features; the direct check against lifecycle §39's eleven-feature allowlist. No `##` subsections |
| **0** | 357 | Scope, versioning, and the gix mental model | ✅ (`### 0.4 LLM invariant` at 426) |
| **1** | 611 | Installation, crate selection, Cargo features, project layout | ✅ — `### 1.3 workspace feature unification warning` (665) matters for the allowlist |
| **2** | 861 | First executable applications: discover, inspect HEAD, resolve revisions | ✅ — `### 2.2 unborn HEAD is normal` (897) |
| **3** | 1088 | Repository handle model, attached vs detached objects, thread safety | ✅ — **the §69/§87 chapter**; `### 3.1 why `Repository` is not `Sync`` (1105) |
| **4** | 1304 | Repository discovery, open, init, bare repositories, and trust | ◐ — discovery/open/bare/trust ✅ (§41, §42, §76); `init` is repository creation ⛔ |
| **5** | 1496 | Configuration, environment, command context, trust-sensitive values | ◐ — trust-sensitive config ✅ (§76); command context spawns processes ⛔ (§38) |
| **6** | 1686 | Repository layout: git-dir, work-dir, common-dir, namespaces, state | ✅ — the §40 topology chapter; `common-dir` is how linked worktrees are identified |
| **7** | 1882 | Object IDs, hash algorithms, SHA-1/SHA-256 feature policy | ✅ — `sha1` is the required hash feature |
| **8** | 2069 | Object database: lookup, headers, writing, caching, in-memory writes | ◐ — reads ✅ **only** as the §63/§64 Level-2 blob-OID cache accelerator, never identity (§86); writing ⛔ |
| **9** | 2263 | Blob objects and binary-safe content handling | ✅ — byte-safety underpins `GitRepoPath` (§43) |
| **10** | 2453 | Tree objects, traversal, entry lookup, and tree editing | ◐ — traversal/lookup ✅ (§91 HEAD-tree acceleration); tree **editing** ⛔ (§82) |
| **11** | 2647 | Commit objects, actors, messages, signatures, parents, creation | 🕰 ⛔ — commit facts are excluded ontology; creation is mutation |
| **12** | 2847 | Tag objects and annotated/lightweight tag handling | 🕰 |
| **13** | 3037 | References, HEAD, symbolic refs, reflogs, edits, and transactions | ◐ — reading HEAD/symbolic refs ✅ (§58 tracked baseline); reflogs 🕰; ref edits/transactions ⛔ |
| **14** | 3239 | Revision specifications, rev-parse, describe, object peeling | ◐ — peeling HEAD to a tree ✅; `describe` and general revspec history 🕰 |
| **15** | 3429 | Revision walking, commit graphs, merge bases, graph caches | 🕰 — §65: commit-graph caches *"are not justified by the present-state lifecycle"* |
| **16** | 3621 | The Git index: load, create from tree, stages, write, integrity | ◐ — load/stages/integrity ✅ (§56, §57, conflict stages); **write** ⛔ (§38) |
| **17** | 3816 | Pathspecs, glob/path normalization, case behavior, and search | ✅ — §44 path normalization, §46 inclusion policy |
| **18** | 4006 | Attributes, ignores, excludes, directory walking, worktree stacks | ✅ — the `attributes` + `excludes` + `dirwalk` features; §47, §48, §49 |
| **19** | 4198 | Clean/smudge filter pipelines and external process boundaries | ⛔ — §84 "No external filters by default"; analyze current bytes instead |
| **20** | 4384 | Repository status: index↔worktree, tree↔index, untracked, submodules | ✅ — but §85: **not on every event**; §54 status is an accelerator, not proof |
| **21** | 4576 | Blob and tree diff, line diff, rewrite/rename tracking, diff caches | ✅ — `blob-diff`; §60 rename policy: *"rename similarity is evidence, not canonical semantic identity"* |
| **22** | 4766 | Merge primitives: blob, tree, commit merge, options, conflicts | ⛔ — §82 no Git mutation orchestration |
| **23** | 4958 | Blame and mailmap | 🕰 — §81 names blame explicitly |
| **24** | 5146 | Submodules: discovery, configuration, activation, open, status | ✅ — §66 submodule policy, §67 nested repositories |
| **25** | 5338 | Main and linked worktrees | ✅ — §68 linked-worktree scheduling |
| **26** | 5530 | Checkout/worktree mutation, low-level semantics, Windows safety | ⛔ — §83 "No checkout in the indexing daemon"; `### 26.1` (5547) is the GHSA advisory context, worth reading for the version floor even though the API is forbidden |
| **27** | 5750 | Worktree byte streams and archives | ⛔ — materializes trees; not needed by an analyzer |
| **28** | 5936 | Remotes, remote names, URLs, URL rewriting, and refspecs | ⛔ — §38 network |
| **29** | 6137 | Credentials, prompts, command spawning, execution permissions | ⛔ — §38 credential + command; §76 trust policy disables helpers |
| **30** | 6328 | Transport and protocol: file, git, SSH, HTTP(S), sync/async | ⛔ — §38 network |
| **31** | 6525 | Fetch: ref mapping, negotiation, shallow, ref updates, packs | ⛔ — §38 |
| **32** | 6724 | Clone builders, fetch-then-checkout, cleanup, persistence | ⛔ — §38 |
| **33** | 6953 | Push: current API surface and the workflow-completeness gap | ⛔ — §38; the chapter's own thesis (incomplete porcelain) is what §82 cites |
| **34** | 7150 | Creating commits and mutating objects/trees/refs/index safely | ⛔ — §82, §38 |
| **35** | 7348 | Packfiles, multi-pack indexes, ODB internals, alternates | ◐ — read-side only, same rule as §8 |
| **36** | 7538 | Locks, tempfiles, filesystem snapshots, atomicity boundaries | ✅ — §74 gix lock/tempfile integration, §75 singleton daemon lease |
| **37** | 7727 | Progress, tracing, interruption, signals, and cancellation | ✅ — the `interrupt` + `tracing` features; §72 cancellation |
| **38** | 7917 | Error model, error chains, diagnostics, application context | ✅ — the `auto-chain-error` feature |
| **39** | 8103 | Concurrency, `parallel`, ThreadSafeRepository, blocking/async | ✅ — §69 handle model, §71 shared thread budget, §70/§109.6 blocking placement |
| **40** | 8294 | Performance tuning: object cache, pack caches, commit graph, parallelism | ◐ — object/pack caches ✅ (§65); commit-graph cache 🕰 (§65) |
| **41** | 8486 | Testing and correctness: fixtures, Git parity, adversarial repositories | ✅ — §145 conformance suite, §146 Git CLI parity oracle |
| **42** | 8673 | Cross-platform behavior: Linux/macOS/Windows, symlinks, exec bits | ✅ — §78 symlink and Windows safety |
| **43** | 8863 | Security and governance for untrusted repositories and remotes | ✅ — §76 repository trust policy |
| **44** | 9063 | Production deployment patterns in Rust | ✅ — `### 44.1 read-mostly code-intelligence service` (9080) and `### 44.2 repository pool` (9098) are the relevant ones; `### 44.3 mutation bot` (9102) is not |
| **45** | 9312 | Complete `gix-*` plumbing-crate architecture catalog | ✅ reference |
| **46** | 9512 | Migration from `git2` / libgit2 and API-equivalence strategy | ✅ reference |
| **47** | 9702 | **Capability matrix: implemented plumbing vs incomplete porcelain** | ✅ — read before assuming any workflow exists; `### 47.1` matrix (9719), `### 47.2` agent decision algorithm (9748) |
| **48** | 9922 | API stability, upgrades, MSRV, feature drift, release migration | ✅ — pairs with §147's twelve-step upgrade gate |
| **49** | 10109 | CLI `gix` / `ein` as debugging tools, not stable scripting contracts | ✅ — matches §146: the CLI is a test oracle, never a hot-path dependency |
| **50** | 10286 | Final best practices, anti-patterns, and LLM-agent execution playbook | ✅ — `### 50.1 Non-negotiable invariants` (10303) |
| A | 10488 | Appendix A — High-level `gix` facade inventory | `A.1` primary facade types (10496) · `A.2` public modules (10532) · `A.3` plumbing re-exports (10590) |
| B | 10668 | Appendix B — `gix-*` plumbing-crate catalog and selection guide | `### B.1` release-family versions used by gix 0.86.0 (10786) |
| C | 10798 | Appendix C — `Repository` method-family map for agent discovery | `C.1` identity/layout 10802 · `C.2` HEAD/refs 10822 · `C.3` objects 10844 · `C.4` revisions 10867 · **`C.5` index/worktree 10884** · **`C.6` status/diff 10903** · **`C.7` paths/attributes 10920** · `C.8` merge/blame/mailmap 10936 · `C.9` remotes 10952 · `C.10` config/runtime 10971 · `C.11` submodules 10989 · `C.12` worktree stream/archive 11001 · `C.13` commit/mutation 11012 |
| D | 11034 | Appendix D — Cargo and deployment recipes | **`D.1` read-only local code-intelligence engine 11038** — the closest upstream analogue to lifecycle App. E · `D.2` clone/fetch worker 11062 · `D.3` lean reusable library 11080 · `D.4` high-performance batch analyzer 11099 |
| E | 11126 | Appendix E — Security threat model | one 7-row threat/control table |
| F | 11160 | Appendix F — Final LLM coding-agent prompt fragment | copy-pasteable 8-rule agent prompt |
| — | 11186 | End of reference | |

---

## §2 — Authority, policy surface, and handoff

Unlike the four docs routed by `code-facts-lib-ref`, these two rarely contend to answer the same question — they hand off. So this section is not a competing-authority matrix; it is the ladder they sit in, the roles the spec assigns them, and the policy boundary that makes half of one document unusable here.

### Table 1 — The authority ladder (lifecycle §2.1)

The spec's own ordering, verbatim in structure. Everything above a row outranks everything below it.

| Rank | Authority | Owns |
|---:|---|---|
| 1 | **current stable filesystem bytes** | authoritative present-state source content |
| 2 | **CodeFabric BLAKE3 content digest** | canonical current source-content identity |
| 3 | **notify-debouncer-full** | low-latency invalidation signal and rename assistance |
| 4 | **gix** | Git-aware repository/worktree topology, path/inclusion semantics, HEAD/index/operation-state interpretation, candidate-delta accelerator |
| 5 | Tree-sitter / Ruff / Pyrefly / rustc / MIR | syntax and semantic fact providers → **`code-facts-lib-ref`** |
| 6 | CodeFabric owner/fact-family reconciliation | present-state CPG mutation authority |

Two consequences the spec states outright: *"A Git blob object ID SHALL NOT replace the CodeFabric current-byte digest"*, and *"A gix status or tree-diff result SHALL generate candidate paths; it SHALL NOT directly mutate CPG facts or prove the current filesystem byte state."*

### Table 2 — Role split (lifecycle §27)

| | **SHALL be treated as** | **SHALL NOT be treated as** |
|---|---|---|
| **notify** | live invalidation signal · event normalization · rename assistance | durable journal · authoritative file state · filesystem transaction stream · Git status engine · graph mutation engine |
| **gix** | repository/worktree topology authority · Git-relative path and inclusion semantics · HEAD/index/operation-state interpreter · status/tree-diff candidate accelerator | current source-byte authority · repository mutation engine · checkout/switch/reset orchestrator · network/fetch service · **semantic fact provider** |

And the handler contract, also §27 — it *SHALL* perform only event classification, event sequencing, cheap path normalization, and bounded enqueue or reconcile escalation; it *SHALL NOT* parse, compile, run status, traverse trees, hash large files, or mutate the CPG.

### Table 3 — Permitted vs forbidden gix surface

This is the map the gix document itself cannot give you. Chapter classes as in §1.2.

| Class | gix chapters | Governing rule |
|---|---|---|
| **✅ In policy** | §0-§3 · §6 · §7 · §9 · §17 · §18 · §20 · §21 · §24 · §25 · §36-§39 · §41-§50 · App. A-F · **§1A** | Covered by the eleven-feature allowlist (§39, App. E): `sha1`, `index`, `status`, `attributes`, `excludes`, `dirwalk`, `blob-diff`, `interrupt`, `parallel`, `auto-chain-error`, `tracing` |
| **◐ Capability-split** | §4 (not `init`) · §5 (not command context) · §8 and §35 (reads only, as §63/§64 cache) · §10 (traversal, not editing) · §13 (HEAD yes, reflogs no, edits no) · §14 (peel yes, `describe` no) · §16 (load/stages yes, write no) · §40 (object/pack caches yes, commit-graph no) | The split is *capability*-level, not chapter-level — read the §1.2 note before using the chapter |
| **🕰 History — excluded ontology** | §11 · §12 · §15 · §23 · parts of §13, §14, §40 | §81 "No history ontology": no commit nodes, blame facts, historical code states, churn, lineage, or revision walks. *"HEAD/tree/OIDs are operational update baselines only."* Ontology §2.2 excludes the same list as a fact domain |
| **⛔ Forbidden behavior** | §19 · §22 · §26 · §27 · §28-§34 | §38's thirteen prohibited operations (write object/ref/index, checkout, reset, switch, fetch, clone, push, merge publication, clean/smudge execution, credential-helper execution, hook execution), plus §82 no mutation orchestration, §83 no checkout, §84 no external filters |

Two framings worth internalising. First, App. E: *"the application policy forbids command, credential, network, checkout, ref-write, and index-write behavior **even if a transitive crate exposes the underlying capability**."* The feature allowlist is the enforcement mechanism; discipline alone is not. Second, §38 permits the crate to contain what the product may not expose: *"gix mutation APIs may exist in the dependency but SHALL not be exposed through CodeFabric lifecycle interfaces."*

### Table 4 — Pipeline handoff seam (lifecycle §93, §36)

| Stage | Answered by | Must not be asked for |
|---|---|---|
| A path changed, and roughly when | **notify** (§5, §9, §10) | whether the file still exists, or what it now contains |
| Was anything lost | **notify** `need_rescan()` (§11) → lifecycle §19.2 `RESCAN_REQUIRED` | a guarantee that nothing was lost when the flag is clear |
| Which repository/worktree owns this path | **gix** §4, §6, §25 | the file's bytes |
| Is this path included — tracked, ignored, excluded, conflicted | **gix** §16, §17, §18, §20 | authorization (§46: ignore rules are **not** authorization) |
| What is the candidate change set after a bulk transition | **gix** §20 status, §21 tree diff, §10 trees | proof that those candidates are current (§54) |
| What are this file's current bytes | **the filesystem**, digested with BLAKE3 | either library |
| What does this file mean | **`code-facts-lib-ref`** providers | either library (§27: gix is not a semantic fact provider) |
| What gets published | CodeFabric reconciliation (§44, App. M) | either library |

---

## §3 — Decision trees

### Which library answers this question?

```
Something on disk changed and I need to know about it soon?
  -> notify §5, §9, §10                                    (lifecycle §27)
Did I miss anything?
  -> notify §11 need_rescan(); escalate to reconcile        (lifecycle §19.2, §36)
Where is the repository / worktree / common-dir?
  -> gix §4, §6, §25                                       (lifecycle §40-§42, §68)
Does this path count -- tracked, ignored, excluded, conflicted?
  -> gix §16, §17, §18, §20                                (lifecycle §46, §47)
What is the candidate delta after a branch switch or bulk change?
  -> gix §20 status, §21 tree diff                         (lifecycle §55, §59, §91)
What are the file's current bytes?
  -> NEITHER. Read the filesystem, digest with BLAKE3.     (lifecycle §2.1)
What does the file mean -- syntax, types, symbols?
  -> NEITHER. sibling skill code-facts-lib-ref             (lifecycle §27)
How should this be scheduled / bounded / cancelled?
  -> partly here (notify §20, §21; gix §37, §39), but the
     scheduling stack is rust_parallel_concurrency_stack_* (lifecycle Part VIII)
```

### Is this gix capability in policy?

```
Does it write anything -- object, ref, index, worktree?
  -> FORBIDDEN                                             (§38, §82, §83)
Does it touch the network, credentials, or spawn a command?
  -> FORBIDDEN                                             (§38; gix §28-§33)
Does it execute repository-defined filters or hooks?
  -> FORBIDDEN. Analyze current bytes instead.             (§84; gix §19)
Does it produce a commit/blame/churn/lineage/revwalk fact?
  -> EXCLUDED ONTOLOGY -- not a CPG fact                   (§81; gix §11, §15, §23)
  -> HEAD/tree/OID as an *operational baseline* is fine    (§58, §91)
Is the enabling Cargo feature in the eleven-feature allowlist?
  -> yes -> in policy                                      (§39, App. E)
  -> no  -> stop; adding a feature is a policy change, not a build tweak
Still unsure whether the workflow even exists upstream?
  -> gix §47 capability matrix + §47.2 decision algorithm
```

### A file event arrived — what does the handler do?

```
In the watcher callback, do exactly four things:            (§27)
  1. classify the event
  2. sequence it (seq: u64)
  3. cheap path normalization
  4. bounded enqueue, or set the reconcile flag
Enqueue:
  try_send into a bounded channel                           (§30)
    ok    -> coordinator picks it up
    full  -> set reconcile_required; increment metric; DO NOT block
Never in the handler: parse, compile, run gix status,
traverse trees, hash large files, mutate the CPG.           (§27)
Never leak notify::EventKind downstream -- convert to
the WatchChange facade at the boundary.                     (§29)
Git metadata paths are watched separately and normalized
into Git-state categories; always reread gix state after.   (§27, §53)
```

### Which watcher backend and configuration?

```
Local disk, Linux?
  -> inotify (RecommendedWatcher). Watch roots, not files;
     watch descriptors are the scarce resource.             (notify §15)
  -> ENOSPC on watch registration usually means exhaustion  (notify §12.5, §33.9)
Local disk, macOS?
  -> FSEvents. Expect startup/cache cost and atomic-save
     patterns; choose FSEvents vs kqueue by feature flag.   (notify §16)
Local disk, Windows?
  -> ReadDirectoryChangesW. Watch for AV/indexer contention
     and path-representation differences.                   (notify §17)
Network mount, NFS/SMB, container bind mount, WSL /mnt/c?
  -> native backends miss events -> PollWatcher             (notify §18, §33.10-§33.12)
  -> polling and debouncing are complementary, not
     alternatives; poll interval != debounce timeout        (notify §18.5, §13.4)
Need to set Config (poll interval, compare_contents, symlinks)?
  -> new_debouncer_opt                                      (notify §13)
Timing: timeout 50-100 ms, tick 10-25 ms, tick <= timeout
(construction fails otherwise); CodeFabric baseline 75/20.  (notify §4.4; lifecycle App. B)
```

### A rename or an editor save — how do I read it?

```
Got Remove+Create for one logical save?
  -> almost certainly an atomic-replace save                (notify §22.2)
  -> do NOT apply raw delete/create to graph facts          (lifecycle §6.3)
Got RenameMode::Both?
  -> a stitched pair. Still only evidence.                  (notify §6.2)
Got only From or only To?
  -> move-out / move-in across a watch root or filter
     boundary -- treat the other side as unknown            (notify §6.5, §19.5)
Want better stitching?
  -> FileIdCache: RecommendedCache default, FileIdMap
     explicit, NoCache to opt out                           (notify §7)
Deciding identity, evidence order (strongest first):        (lifecycle §35)
  1. current worktree path identity
  2. stable filesystem file ID
  3. notify tracker / file-ID evidence
  4. bounded gix rename candidate (bulk transitions only)
  5. identical current BLAKE3 digest
  6. source structural/semantic identity
None of these is a canonical CPG ID.                        (§35, §86)
"A matched rename is an optimization, not proof."           (§35)
```

### Events were lost, or the queue overflowed

```
need_rescan() true, or backend reported overflow?
  -> EventStreamHealth = RESCAN_REQUIRED                    (lifecycle §19.2)
  -> SourceTrustState  = UNVERIFIED until reconciled
  -> run authoritative inventory, not a guess               (notify §11.3)
Application queue full (bounded try_send failed)?
  -> same escalation: set reconcile_required                (lifecycle §30)
During the rescan, do events keep arriving?
  -> yes; they are NOT discarded. They become
     dirty_after_reconcile behind a generation fence.       (lifecycle §36; notify §11.4)
Strict-current queries while trust is UNVERIFIED?
  -> must not claim source completeness                     (lifecycle §36)
Watcher permanently degraded?
  -> a generic authoritative inventory can still restore
     CURRENT; degraded != permanently untrustworthy         (lifecycle §19.2)
```

### Do I actually need `status`?

```
One isolated file save?
  -> NO. Hot path is: event -> stable read -> digest ->
     local invalidation.                                    (§85)
Branch switch, rebase, bulk checkout by an external tool?
  -> YES: HEAD-tree diff + bounded status                   (§59, §91; gix §20, §21)
Reconciliation after rescan / overflow?
  -> YES, bounded                                           (§55)
Startup?
  -> Git-native inventory, then status acceleration         (§89, §90)
Whatever the answer, status output is a CANDIDATE SET.
It "SHALL NOT be the final proof that CPG source bytes
are current" -- the digest is.                              (§54)
Rename detection on every small save?
  -> NO. "SHOULD NOT run for every small save."             (§60)
```

### Which path type am I holding?

```
Came from gix (repo-relative)?
  -> GitRepoPath { raw: Vec<u8>, display: String }          (§43, App. F)
  -> raw bytes are authoritative; Git paths need not be UTF-8
Came from notify / the filesystem?
  -> PlatformPath { native: OsString, display: String }
Crossing between them?
  -> convert explicitly; never round-trip through the
     display string. "Display strings are never identity."  (§43)
Using it as a map key?
  -> normalized platform path for the dirty registry,
     plus the byte-safe Git path where available            (§31)
Using it for an authorization decision?
  -> gitignore is NOT authorization. CodeFabric's explicit
     root-boundary policy always outranks it.               (§46, §157 inv. 25)
```

---

## §4 — Operating rules

Rules 1-12 are doctrine and library semantics. **Rules 13-18 are navigation meta-rules** — read them before your first grep.

1. **Neither library is the source-byte authority.** Current filesystem bytes are, identified by a BLAKE3 digest. gix is explicitly *not* the "current source-byte authority" and notify *not* "authoritative file state". A status result or a watcher event generates **candidates**; only a read plus digest proves content. (lifecycle §2.1, §27, §54)

2. **gix is a read-only present-state oracle, never a history provider.** §81 "No history ontology" excludes commit nodes, blame facts, historical code states, churn, lineage, and revision walks from the CPG; HEAD, trees, and OIDs are *operational update baselines only*. The ontology spec §2.2 excludes the same list as a fact domain. This puts gix §11, §12, §15, §23 and parts of §13/§14/§40 out of bounds. (lifecycle §81, §3.2; ontology §2.2)

3. **The prohibitions bind even when the crate exposes the capability.** §38 lists thirteen forbidden operations; App. E adds *"even if a transitive crate exposes the underlying capability"*; §38 also permits the dependency to contain what the product must not expose. Enforce with `default-features = false` plus the eleven-feature allowlist, not with care. (lifecycle §38, §39, App. E)

4. **gix must remain provably optional.** §80: any read-side gix failure falls back to bounded generic filesystem inventory — *"The system may be slower, but it remains correct."* §118: *"gix failure SHALL degrade acceleration before it degrades source correctness."* App. D requires a clean-rebuild equivalence run **with gix acceleration disabled** yielding the same semantic CPG. If a design cannot lose gix, it is wrong. (lifecycle §80, §118, App. D)

5. **A missing event is not proof that nothing changed.** Watcher health and source truth are orthogonal (§19). `need_rescan()` or a bounded-queue overflow sets `EventStreamHealth = RESCAN_REQUIRED` and source trust to `UNVERIFIED` until reconciliation completes. Events arriving during a rescan are not discarded — they become `dirty_after_reconcile` behind a generation fence. (lifecycle §19.2, §30, §36, §157 inv. 16; `notify §11`)

6. **The watcher handler does four things and nothing else.** Classify, sequence, cheaply normalize, bounded-enqueue or escalate. No parsing, compiling, `status`, tree traversal, large-file hashing, or CPG mutation. Raw `notify::EventKind` must not leak downstream; `WatchChange` (§29) is the mandated facade — the exact analogue of `code-facts-lib-ref`'s ban on long-lived `Node<'tree>`. (lifecycle §27, §29)

7. **Identity is content, not any provider's handle.** *"Filesystem file IDs, watcher tracker IDs, gix object IDs, and gix rename similarity SHALL NOT be canonical CPG IDs"* (§35). A blob OID may serve as a **Level-2 cache key** behind the BLAKE3 digest (§63, §64) but never as identity (§86). A matched rename is an optimization, not proof (§35).

8. **Git paths are bytes.** `GitRepoPath { raw: Vec<u8>, display: String }` and `PlatformPath { native: OsString, display: String }` are distinct types and *"display strings are never identity"* (§43, App. F). Non-UTF-8 Git paths must stay representable (§157 inv. 30).

9. **Gitignore is not authorization.** Ignore rules reduce work; CodeFabric's explicit security and root-boundary policy always has higher authority (§46, §157 inv. 25). Never let a `.gitignore` decide what a caller is allowed to see.

10. **One `Repository` handle per thread.** §69 forbids `Arc<gix::Repository>` as a globally shared concurrent handle (the spec typesets the type in a fenced block mid-sentence) — use thread-local handles, `ThreadSafeRepository`, an actor, or reopened repositories (§69, §87; `gix §3`, `§39`). Do not retain attached values across update waves; keep immutable OIDs and application DTOs instead (§73).

11. **gix work is blocking and must not run on latency-sensitive Tokio workers.** Repository, status, index, tree, and ODB operations are blocking filesystem/CPU work; route them through a bounded Git-work semaphore to a dedicated blocking worker or `spawn_blocking`, returning detached DTOs. gix's internal `parallel` and CodeFabric's outer parallelism share one process-wide thread budget. (lifecycle §70, §109.6, §71, §158 inv. 6)

12. **Debounce timing is a contract, not a preference.** Timeout 50-100 ms, tick 10-25 ms and **never greater than the timeout** (construction fails), gather window 10-25 ms downstream and independent of the filesystem debounce. Continuous writes yield *periodic* eligible batches, so rely on **generation supersession**, not on a single trailing-edge event. CodeFabric baseline: 75 / 20 / 20 ms, ingress 4,096. (lifecycle §28, App. B; `notify §4.4`, `§4.6`)

13. **notify changes its own heading prefix mid-document.** §0-§8 are `` # `notify-debouncer-full` Advanced — N) `` (backticked, 9 chapters); §9-§40 are `# notify-debouncer-full Advanced — N) ` (32). Grep either form alone and you silently lose the other block. Robust: `^# .\?notify-debouncer-full.\? Advanced — `.

14. **notify's own table of contents disagrees with its chapters.** Its map at line 50 uses titles that differ from the real chapter headings for **24 of 41 chapters** — all of §9-§28 plus §32, §35, §38, §39 (e.g. TOC "`notify::Event` and event taxonomy" vs chapter "Event taxonomy and semantic interpretation"). Index from the chapter headings, never the map. gix has the opposite problem: its TOC titles are byte-identical to its chapters, so a content grep returns two hits — disambiguate on heading level.

15. **gix §1A sorts before §0 and is invisible to the obvious regex.** `# gix Advanced — 1A) Release-pinned feature flag catalog` is at line 231; §0 is at 357. It is the only lettered chapter, has no `##` subsections, and is absent from the doc's map. Use `^# gix Advanced — ([0-9]+[A-Z]?)\)`. It is also the chapter you most want first, since it is the feature table the §39 allowlist is checked against.

16. **gix shadows its own subsection numbering — 28 collision pairs.** Ten chapters embed a `### N.M` ladder inside `N.0` reusing the numbers of the `## N.M` skeleton with different titles (e.g. `### 47.1 Capability matrix` at 9719 vs `## 47.1 Key public surface` at 9762). Anchor on exactly `^## ` for the skeleton; `^#+ 47\.1` returns two structurally different hits.

17. **gix is 52.87% blank lines.** Double-spaced throughout, including markdown table rows, against notify's 28%. A `Read(offset, limit)` window holds about half the content it appears to, and `grep -A5` captures roughly two content lines. Budget roughly double the `limit`.

18. **The lifecycle spec has no Part IV.** Part III "State Model" runs §18-§36 and Part V "Git-Aware Repository and Worktree State" picks up at §37, so every watcher section (§27-§36) sits under a Part titled for state, not for watching. Cite lifecycle material by **section number, never by Part**, and by section rather than line: the filename carries a version suffix that moves between releases and its lines drift, while section numbers have held. Note also that `spec-outline` does not emit `# Part` headings at all — `docs/spec_index/README.md` §3.1 tabulates all 110 of them. Both library docs are h1-rooted, so use `lib-outline`, not `spec-outline`, on them.

---

## §5 — Project context: CodeFabric

**Pre-implementation.** Neither crate is a declared dependency yet — one Cargo package, one library crate, and a seed that only proves the toolchain. So unlike the sibling library-ref skills there is no crate→document map to give. These two docs are read *against the lifecycle spec*, which is their sole doctrinal home: no other spec assigns either library a role, and the five sibling specs mention gix only in a boilerplate process-topology line.

Note the asymmetry in how the spec treats them. `gix_rust_advanced_reference.md` is listed under **Companion specifications**; `notify_debouncer_full_rust_reference.md` appears only in the §2 source-basis table. gix carries an exact pin (`=0.86.0`), a security floor, a feature allowlist, and a twelve-step upgrade gate (§147); notify's version is pinned in no spec at all — only in CLAUDE.md's prose — and has no upgrade gate. Read that as uneven emphasis, not as permission to be casual with the watcher.

### Lifecycle-spec anchor map

| Spec section | Subject | Doc chapters |
|---|---|---|
| §1 | The governing one-line integration rule | — |
| §2.1 | Authority ladder | §2 table 1 above |
| §2.2 | gix pin, security floor, DTO isolation | `gix` §1, §1A, §48 |
| §3.1 / §3.2 | Included vs excluded lifecycle work | — |
| §19-§20 | `SourceTrustState`, `EventStreamHealth`, Git acceleration states | `notify` §11, §12 |
| §27 | Watcher and Git-state roles | both — the role split |
| §28 | Debounce policy | `notify` §4, §5 |
| §29 | `WatchChange` / `GitStateChange` facade | `notify` §32 (wrapper design), §1.6 |
| §30-§32 | Bounded ingress, dirty registry, bulk thresholds | `notify` §20, §21 |
| §35-§36 | Rename evidence hierarchy, rescan generation fence | `notify` §6, §7, §11 |
| §37-§39 | `codefabric-git-state`, read-only policy, feature profile | `gix` §1A, §1, §3 |
| §40-§42 | Source-instance topology, startup discovery, bare repos | `gix` §4, §6 |
| §43-§49 | Paths, normalization, identity, inclusion, walking, attributes, ignore fingerprint | `gix` §9, §17, §18 |
| §50-§59 | Git state vector, operation state, metadata watch set, status, index, HEAD | `gix` §13, §16, §20 |
| §60-§65 | Rename policy, mode/symlink changes, stabilization, blob-OID cache, gix caches | `gix` §21, §8, §40 |
| §66-§68 | Submodules, nested repositories, linked worktrees | `gix` §24, §25 |
| §69-§75 | Handle model, execution placement, parallelism, cancellation, staleness, locks, daemon lease | `gix` §3, §36, §37, §39 |
| §76-§80 | Trust policy, filters, symlink/Windows safety, resource governance, generic fallback | `gix` §42, §43, §19 (as prohibition) |
| §81-§87 | The seven anti-requirements | §2 table 3 above |
| §88-§92 | Phased adoption: repository correctness → Git-native inventory → status/index acceleration → bulk HEAD-tree → shared caches | `gix` §4/§6 → §18 → §16/§20 → §10/§21 → §40 |
| §93 | Pipeline overview | §2 table 4 above |
| §109.6 | Bounded blocking gix execution class | `gix` §39 |
| §145-§147 | Conformance suite, Git CLI parity oracle, gix upgrade gate | `gix` §41, §49, §48 |
| §155-§156 | `codefabric-watch` / `codefabric-git-state` crates, `GitStateAdapter` | `notify` §32 · `gix` §38 |
| §157-§159 | Consistency, performance, and failure invariants | both |
| App. B / D / E / F | Tuning profile · clean-rebuild equivalence · gix dependency profile · Git-state DTOs | `gix` App. D, `notify` §4 |

### Adapter boundary

The spec names two crates: **`codefabric-watch`** (notify-debouncer wrapper, source/Git-metadata event facade, watcher health) and **`codefabric-git-state`** (the gix dependency and read-only adapter), with `codefabric-git-worker` for bounded blocking execution and `codefabric-git-testkit` for adversarial repository fixtures (§155). The `GitStateAdapter` trait (§156) exposes exactly five methods — `open_worktree`, `capture_state`, `inventory`, `status_candidates`, `tree_diff_candidates` — and *"SHALL return detached CodeFabric DTOs. It SHALL not expose `gix::Repository`, attached objects, references, index handles, or thread-local provider state."* No public crate outside `codefabric-git-state` should expose gix types (§155). An optional compile-time `codefabric-git-read` / `codefabric-git-admin` split can enforce the read-only policy structurally (§38).

### Boundary discipline

The doctrine these two docs serve is the same one that governs the fact providers: **fact substrate, not judgment**, and **present state, not history**. Git is the most tempting place in the system to violate the second — the library makes `log`, `blame`, and revision walks trivially available, and they are exactly what the ontology excludes. Operational Git state (HEAD, index, worktree topology, acceleration status) enters as *lifecycle metadata*, never as CPG facts (§37.2: *"The CPG fact ontology remains history-free"*).

**Rule of thumb:** deciding *whether a file matters, or what changed* → this skill. Deciding *what a file means* → **`code-facts-lib-ref`**. Storing or serving the result → **`deltalake-rust-ref`** / **`datafusion-pyarrow-rust-ref`**. Scheduling and backpressure mechanics → `docs/library_ref/rust_parallel_concurrency_stack_reference_2026-08-19.md`, which no skill routes; `docs/spec_index/library-routing.md` maps its chapters to the lifecycle sections that need them (§70-§71, §109-§116, §151-§153).
