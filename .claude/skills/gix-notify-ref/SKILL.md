---
name: gix-notify-ref
description: "Reference navigator for the CPG lifecycle input boundary — the two version-pinned deep-dives behind *what changed on disk* and *what the repository actually contains*. SKILL.md maps them at `docs/library_ref/`: `notify_debouncer_full_rust_reference.md` (debounce timing, event coalescing, rename stitching, rescan/loss recovery, backends, backpressure, watcher lifecycle; §0-§40) and `gix_rust_advanced_reference.md` (repository/worktree discovery, git-dir/work-dir/common-dir, byte-safe Git paths, index, pathspecs, attributes/ignores/dirwalk, status, tree/blob diff, linked worktrees, locks, interruption; §0-§50 + §1A + Appendices A-F); REFERENCE.md (same folder) holds per-doc section indexes, the authority ladder and role split, the permitted/forbidden gix surface map, decision trees, and operating rules. Use when Rust touches `use gix::`/`gix::discover`/`gix::open`/`Repository::`/`ThreadSafeRepository`/`use notify::`/`notify_debouncer_full`/`new_debouncer`/`new_debouncer_opt`/`DebouncedEvent`/`DebounceEventResult`/`DebounceEventHandler`/`FileIdCache`/`FileIdMap`/`RecommendedCache`/`RecommendedWatcher`/`PollWatcher`/`EventKind`/`RenameMode`/`need_rescan`, when building a watcher or git-state adapter, or when editing `Cargo.toml` pins for those crates. Parsing the files a change points at → sibling `code-facts-lib-ref`; storing or serving the resulting facts → `deltalake-rust-ref` / `datafusion-pyarrow-rust-ref`."
allowed-tools: Read, Grep, Glob, Bash
---

# gix + notify Lifecycle Reference Navigator

Routes the two deep-dive references behind CodeFabric's **lifecycle input boundary** — the layer that decides *what changed* and *whether it matters*, before anything is parsed, stored, or served. This SKILL.md is the **core map**: version anchors, the two-document topic table, reading strategy, where-to-look routing, and the key invariants. The companion **`REFERENCE.md`** (same folder) carries the per-document section indexes, the **authority ladder** and **role split** (quoted from the lifecycle spec), the **permitted/forbidden gix surface map** — the single most useful thing here, because roughly half the gix document describes capabilities this project normatively forbids — the eight decision trees, the full 18 operating rules, and the CodeFabric project context. Reach for REFERENCE.md once you know which document you need; cross-references back here are written `SKILL §...`.

The lifecycle spec states the relationship in one sentence (§1), and it is the thesis of this skill:

> notify discovers urgency; gix explains Git state and reduces candidate work; current filesystem bytes establish truth; CodeFabric controls semantic invalidation and atomic publication.

**Out of scope** (covered elsewhere): parsing or extracting facts from the files a change points at → sibling **`code-facts-lib-ref`** (tree-sitter · Ruff · Pyrefly · rustc/MIR). Storing or serving those facts → **`deltalake-rust-ref`** and **`datafusion-pyarrow-rust-ref`**. Derived graph analyses → `docs/library_ref/petgraph.md`. Scheduling, thread budgeting, and backpressure mechanics (lifecycle Part VIII) → `docs/library_ref/rust_parallel_concurrency_stack_reference_2026-08-19.md`, which no skill currently routes.

---

## Version anchors

* **gix 0.86.0** — exact-pinned `gix = { version = "=0.86.0", default-features = false }`, Rust edition 2024, MSRV 1.85. The lifecycle spec allows exactly eleven features: `sha1`, `index`, `status`, `attributes`, `excludes`, `dirwalk`, `blob-diff`, `interrupt`, `parallel`, `auto-chain-error`, `tracing` (§39, App. E) — the broad default bundle (30 of 55 flags) is to be avoided because it enables mutation and credential surfaces. **Security floor:** GHSA-pmm9-4h7q-24c8 (published 2026-08-02) means `gix <= 0.85.0` must not be deployed; the spec enforces the floor *"although CodeFabric does not perform checkout"* (§2.2). Fixes land in `gix >= 0.86.0`, `gix-features >= 0.49.0`, `gix-worktree >= 0.55.0`, `gix-worktree-state >= 0.33.0`.
* **notify-debouncer-full 0.7.0** — over `notify 8.2.0`, with `notify-types 2.0.0`, `file-id 0.2.3`, `walkdir 2.4.0`; MSRV 1.85. **0.8.0 release candidates exist and are deliberately not targeted** — do not lift examples from `main` or the RCs (`notify §34`). This version is pinned only in CLAUDE.md's prose and the reference's own header; no spec writes a `notify-debouncer-full = "…"` line.
* **Tuning baseline** (lifecycle App. B): debounce timeout **75 ms**, tick rate **20 ms**, downstream gather window **20 ms**, watch ingress capacity **4,096** events. The permitted band is 50-100 ms timeout and 10-25 ms tick, and **tick must never exceed timeout** or construction fails (`notify §4.4`).
* **Stack contract** (lifecycle §36) — the chain both libraries sit in: `notify-debouncer-full → dirty path / selected Git metadata hints → gix read-only state adapter (topology · Git-native inclusion · status/index candidate reduction · HEAD-tree bulk diff · operation state) → authoritative current filesystem reads → CodeFabric BLAKE3 source digests → ordinary owner/fact-family invalidation and publication`. The spec's closing line on that chain is the one to remember: *"gix improves discovery, topology, inclusion fidelity, and work avoidance. It never supersedes current-byte verification."*

---

## The two reference documents

Both live at `docs/library_ref/`. Each opens with a version anchor and its own **"Proposed comprehensive documentation map"**, then deep-dives. **The heading prefixes are hostile to naive grepping in three separate ways** — notify switches prefix mid-document, gix has a lettered out-of-order chapter, and gix shadows its own subsection numbering. Operating Rules 13-15 in REFERENCE.md spell each out; prefer `lib-outline`.

| Doc | Path (`docs/library_ref/`) | Lines | Deep-dive prefix | Scope (deep-dive range) |
|-----|------|------:|------------------|-------|
| **notify** | `notify_debouncer_full_rust_reference.md` | 10,307 | `` # `notify-debouncer-full` Advanced — N) `` for **§0-§8**, then `# notify-debouncer-full Advanced — N) ` for **§9-§40** | **§0-§40** — the debouncer as a live invalidation signal: install/version policy, type stack (`Debouncer<T,C>`, `DebouncedEvent`, `DebounceEventResult`), **debounce timing (§4)**, **queue coalescing and ordering (§5)**, **rename normalization (§6)**, **file-ID caches (§7)**, watch roots, handler delivery, event taxonomy, **rescan and loss recovery (§11)**, errors, `new_debouncer_opt`, backend/platform model (§14-§18, one chapter each for inotify/FSEvents/ReadDirectoryChangesW/PollWatcher), filtering, Tokio integration, backpressure, **editor save patterns (§22)**, path identity, **code-intelligence architecture (§24)**, large repos, multi-workspace, observability, testing, shutdown, security, wrapper design, troubleshooting, upgrades, crate comparison, deployment, and the closing rules/anti-patterns/checklist/recipes quartet (§37-§40). |
| **gix** | `gix_rust_advanced_reference.md` | 11,186 | `# gix Advanced — N) ` (lowercase `gix`; `N` may be `1A`) | **§0-§50 + §1A + App. A-F** — the full gix surface, of which CodeFabric may use only part: mental model, install/features, **§1A release-pinned feature-flag catalog**, first apps, **handle model and thread safety (§3)**, **discovery/open/bare/trust (§4)**, config, **layout: git-dir/work-dir/common-dir (§6)**, object IDs, ODB, blobs, trees, commits, tags, refs/HEAD, rev-parse, revision walking, **index (§16)**, **pathspecs (§17)**, **attributes/ignores/excludes/dirwalk (§18)**, filters, **status (§20)**, **diff and rename tracking (§21)**, merge, blame, submodules, **linked worktrees (§25)**, checkout, archives, remotes, credentials, transport, fetch, clone, push, commit creation, packfiles, **locks/tempfiles (§36)**, **interruption (§37)**, errors, **concurrency (§39)**, perf, testing, cross-platform, security, deployment, plumbing catalog, git2 migration, **capability matrix (§47)**, API stability, CLI, playbook. |

**Reading strategy.** Start with `lib-outline <file>`, then `Read(offset, limit)` from REFERENCE.md §1. **The two docs read very differently.** gix is *uniform*: chapters run 177-254 lines and every numbered chapter carries the **same eleven subsections** — `N.0` scope/mental model · `N.1` key public surface · `N.2` canonical (selection) pattern · `N.3` value cases · `N.4` planning and ownership rules · `N.5` failure modes · `N.6` testing matrix · `N.7` deployment advisory · `N.8` anti-pattern inventory · `N.9` agent checklist · `N.10` primary sources — so jump straight to the `N.M` you want. notify is *bespoke and lumpy*: 2-29 subsections per chapter, §25 is 87 lines and §40 is 959. Note also that **gix is 52.87% blank lines** (double-spaced, including table rows) against notify's 28%, so an equivalent `Read` window holds about half the content. Pure decision chapters worth reading before designing: gix **§47** (capability matrix) and **§50** (playbook); notify **§35.8** (decision tree for code intelligence), **§37** (30 numbered rules), **§38** (anti-patterns), **§39** (checklist), **§40** (recipes A-T). notify **§24** was written for exactly this use case.

---

## Where do I look?

| Question | Doc |
|---|---|
| How long until a burst of writes settles; why my timeout does not mean "quiet period" | **notify** §4 (`4.4` tick ≤ timeout is enforced) |
| Which events survive coalescing, and in what order | **notify** §5, §10 |
| An editor saved a file and I got create+rename+remove instead of one write | **notify** §22 · §6 (rename normalization) · §7 (file-ID caches) |
| Events stopped arriving / overflowed / the backend died | **notify** §11 (rescan), §12, §15-§18 (per-platform), §33 (symptom cookbook) |
| Which watcher backend, and what to do on a network mount or in a container | **notify** §14-§18 · §36.7-§36.8 |
| Where is this repository, what is its worktree, is it bare or linked | **gix** §4, §6, §25 |
| Is this path ignored / excluded / tracked / untracked / conflicted | **gix** §17 (pathspecs), §18 (attributes, excludes, dirwalk), §20 (status), §16 (index) |
| What changed between HEAD and the worktree, or between two trees | **gix** §20 (status), §21 (diff + rename tracking), §10 (trees) |
| A Git path is not valid UTF-8 | **gix** §9 (binary-safe content), §17 · lifecycle §43 + App. F (`GitRepoPath`) |
| Can I use `blame` / `log` / `fetch` / `checkout` here? | **Almost certainly no** — see REFERENCE.md §2 table 3 (permitted/forbidden surface) before reading the chapter |
| Threading: can I share one `Repository` across tasks | **gix** §3, §39 · lifecycle §69, §87 (no shared `Arc<Repository>`) |

For deeper routing — the authority ladder, the §27 role split, the **permitted/forbidden gix surface map**, and the eight decision trees — see **`REFERENCE.md`**.

---

## Key invariants

The seven that prevent the most errors; the full set of **18 operating rules** is in `REFERENCE.md`.

1. **Neither library is the source-byte authority.** Current filesystem bytes are, verified by a BLAKE3 digest. §27 says gix is *not* the "current source-byte authority" and notify is *not* "authoritative file state"; §2.1 puts both below the bytes in the authority ladder. A `status` result or a watcher event **generates candidates**; it never proves current content. (lifecycle §2.1, §27, §54)
2. **gix is a read-only present-state oracle, never a history provider.** §81 is titled "No history ontology": no commit nodes, blame facts, historical states, churn, lineage, or revision walks enter the CPG. HEAD, trees, and OIDs are *operational update baselines only*. Roughly half the gix document — §11, §12, §14, §15, §23, and §26-§34 — is therefore out of policy. (lifecycle §81, §3.2; ontology §2.2)
3. **The prohibitions are policy even when the crate exposes the capability.** §38 forbids write-object, write-ref, write-index, checkout, reset, switch, fetch, clone, push, merge publication, clean/smudge execution, credential-helper execution, and hook execution. App. E repeats it: *"the application policy forbids command, credential, network, checkout, ref-write, and index-write behavior even if a transitive crate exposes the underlying capability."* Enforce it with the feature allowlist, not with discipline alone. (lifecycle §38, §39, App. E)
4. **gix must remain provably optional.** Every gix read-side failure falls back to bounded generic filesystem inventory — *"The system may be slower, but it remains correct"* (§80). §118: *"gix failure SHALL degrade acceleration before it degrades source correctness."* App. D requires a clean-rebuild equivalence run **with gix acceleration disabled** producing the same semantic CPG. (lifecycle §80, §118, App. D)
5. **A missing event is not proof that nothing changed.** Watcher health and source truth are orthogonal (§19). An overflow or `need_rescan()` sets `EventStreamHealth = RESCAN_REQUIRED` and source trust to `UNVERIFIED` until reconciliation completes — it does not silently continue. Events during a scan are not discarded; they become `dirty_after_reconcile`. (lifecycle §19.2, §36, §157 inv. 16; `notify §11`)
6. **The watcher handler does almost nothing.** §27 allows exactly four things — event classification, sequencing, cheap path normalization, and bounded enqueue or reconcile escalation — and forbids parsing, compiling, running status, traversing trees, hashing large files, or mutating the CPG. Raw `notify::EventKind` must not leak downstream; the `WatchChange` enum (§29) is the mandated facade. (lifecycle §27, §29, §30)
7. **Identity comes from content, not from any provider's handle.** Git blob OIDs, filesystem file IDs, watcher tracker IDs, and gix rename similarity are all *evidence*, never canonical CPG identity (§35, §86). A matched rename is an optimization, not proof. Git paths are bytes — `GitRepoPath { raw: Vec<u8>, display: String }` — and *"display strings are never identity"* (§43, App. F). And Git ignore rules are **not** authorization: CodeFabric's root-boundary policy always outranks them (§46).

---

## Project context: CodeFabric

**Pre-implementation.** Neither crate is a declared dependency yet; there is one Cargo package and a seed that only proves the toolchain. So these two docs are read *against the lifecycle spec*, which is their sole doctrinal home — no other spec assigns either library a role.

| Concern | Doc | Lifecycle spec |
|---|---|---|
| Watcher role, debounce, event facade | **notify** §4, §5, §9 | §27 roles · §28 debounce · §29 `WatchChange` · §30 bounded ingress |
| Rename, atomic save, loss recovery | **notify** §6, §7, §11, §22 | §6.3 editor save · §35 rename evidence hierarchy · §36 rescan fence |
| Repository/worktree topology, discovery | **gix** §4, §6, §25 | §37 subsystem · §40-§42 topology · §68 linked worktrees |
| Paths, inclusion, ignores, walking | **gix** §17, §18, §9 | §43-§49 paths, inclusion, walk, attributes, ignore fingerprint |
| Status, index, HEAD, tree diff | **gix** §16, §20, §21, §10 | §50-§59 state vector, status, index, HEAD · §85 not on every event |
| Handles, concurrency, cancellation | **gix** §3, §37, §39 | §69 handle model · §70/§109.6 blocking placement · §72 cancellation |
| Adapter boundary and DTOs | **gix** §38 (errors) · **notify** §32 (wrapper) | §155 crates · §156 `GitStateAdapter` · App. E/F |
| Version and upgrade discipline | **gix** §48 · **notify** §34 | §147 twelve-step gix upgrade gate (no notify equivalent) |

**Isolation is asymmetric, and deliberately so.** gix isolation is restated roughly eight times across §2.2, §37, §38, §69, §73, §155, §156 and Appendices E/F, with an optional compile-time `codefabric-git-read` / `codefabric-git-admin` split; the `GitStateAdapter` *"SHALL return detached CodeFabric DTOs"* and must not expose `gix::Repository`, attached objects, references, index handles, or thread-local provider state (§156). notify's isolation is stated once (§29). Treat the lighter wording as an accident of emphasis, not a licence.

**Cite the lifecycle spec by section number, not line number** — `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` carries a version suffix that moves between releases and its line numbers drift, while section numbers have held. Note it has **no Part IV**: Part III "State Model" runs §18-§36, so every watcher section (§27-§36) sits under it, and Part V "Git-Aware Repository and Worktree State" picks up at §37. Cite sections, never Parts. Full Part-to-section ranges for all seven design artifacts are in `docs/spec_index/README.md` §3.1.

**Rule of thumb:** deciding *whether a file matters, or what changed* → this skill. Deciding *what a file means* → `code-facts-lib-ref`. Fuller context: `REFERENCE.md` §5.
