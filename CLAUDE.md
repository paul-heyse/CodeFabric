# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repository currently is

CodeFabric is **pre-implementation**. The repository and tooling architecture is fully in
place and verified end to end; the system itself is not built. What exists in `src/` and
`python/codefabric/` is a deliberately minimal seed whose only job was to prove the
toolchain — a `version()` accessor and a `normalize_workspace_id()` that can fail. None of
it is a design. Replace it.

Two documents govern, at different layers:

- **`docs/rust_core_python_interface_repository_specification_2026-08-20.md`** governs
  package/repository/tooling architecture, the assurance tiers, and the agent operating
  rules. It is authoritative for everything in this file. Its section 60 change-risk table
  decides which tools a given change actually warrants — consult it before reaching for an
  expensive one.
- **`docs/upfront_design/`** (a governance manifest, six domain specs, and the implementation
  roadmap) defines what the system *does* and in what order it gets built. These are **in flux**
  and are deliberately not an input to repository/tooling decisions. `docs/spec_index/` indexes
  them — see *The design corpus* below.

### Shape

One Cargo package, one library crate. No workspace, no `crates/`. Rust is the
implementation core; Python is the interface layer:

```
python/codefabric/     public Python package -- the supported contract
  -> codefabric._native   private PyO3 extension (cdylib from the same crate)
    -> src/                 Rust core, Python-agnostic
```

Two deliberate compile surfaces: the featureless core must build with no Python runtime
present, and `--features python` adds the PyO3 adapter. A Python-only dependency must
never leak into the featureless core.

How either side is divided into files internally is **intentionally unspecified**. Do not
infer a preferred decomposition from the current seed. But do not create a new crate for
organizational reasons either — that needs a package/build justification from spec
section 0.3, and "another conceptual area" is not one.

## Commands

**Read `just --list` first.** The justfile is the operational API (spec sections 14, 59,
92); prefer its recipes over reconstructing tool flags.

```bash
just ci-fast      # the routine gate -- run before editing, record pre-existing failures
just test         # Rust tests + doctests + Python interface tests
just check        # both compile surfaces
just wheel-test   # build a wheel, install it in a clean env, prove the import origin
just doctor       # environment report (scripts/bootstrap.sh)
```

Three traps the tools will not catch for you:

- **`cargo nextest` does not run doctests.** Never report "all Rust tests passed" from
  nextest alone. `just test` covers both; `just test-rust` does not.
- **`maturin develop` is not packaging evidence.** Only `just wheel-test` — a clean-env
  install of the built artifact — validates the wheel. A stale editable install produces
  convincing false passes, which is why the script asserts the import origin.
- **`--all-features` is not a feature matrix.** It validates only the maximal union. Use
  `just features-each`.

Recipes in the `[mutating]` group (`fmt-write`, `typos-write`, `snapshots-accept`,
`deps-fix`) change source or manifests, are never dependencies of a gate, and require
deliberate invocation plus diff inspection.

### Toolchain

Stable, pinned by `rust-toolchain.toml`. **Nightly is not required for normal
development** — it backs only `just miri` and `just udeps`. `rustc-dev` is deliberately
not declared (spec section 76); adopting it would be a separate architectural decision
requiring a date-pinned nightly and a semantic golden corpus.

`sccache` is a **hard prerequisite**: `.cargo/config.toml` commits
`rustc-wrapper = "sccache"`, so cargo fails outright without it. Watch the hit rate with
`just cache-stats` rather than assuming it helps.

### One coupling worth knowing

The build backend is Maturin, so **`uv sync` compiles the Rust extension** — including the
`uv sync` inside `.envrc`. A broken Rust build therefore degrades the Python environment,
not just the Rust one. `.envrc` treats sync failure as non-fatal, so the symptom is a
stale `.venv` rather than an unenterable directory.


## The design corpus (`docs/upfront_design/`)

Eight files: the governance manifest, six domain specifications, and the implementation
roadmap. Read the six domain specs as a stack — each layer consumes the one above and the
ontology is the root vocabulary; the governance manifest sits across all of them. Cite
suite-wide as `TAG §N` with the section title alongside (the convention
`docs/spec_index/README.md` §2 fixes), never by line number — revisions move lines but
usually keep titles.

| Tag | File (in `docs/upfront_design/`) | Layer |
|---|---|---|
| `SUITE` | `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md` | **Who owns what.** Cross-cutting authority: artifact ownership and precedence, the compatibility matrix, requirement IDs, the `contracts/` machine-artifact tree, the ten global invariants, and Readiness Gates A–G. Owns `AC-G-01`–`AC-G-08` and `AC-G-78`–`AC-G-84`; its Part II is the authoritative owner table for all 84 contracts. |
| `ONT` | `code_property_graph_present_state_fact_ontology_specification_v1.3.md` | **What facts exist.** Language-neutral core ontology + Python and Rust profiles (~50 fact domains: syntax, semantics, types, CFG, dataflow, alias, effects, MIR, ownership, unknowns). |
| `GEN` | `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` | **How facts are produced.** Provider stack (Tree-sitter, Ruff crates, Pyrefly, `rustc_public`/MIR, petgraph), normalization, reconciliation, derived analyses. |
| `FAB` | `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` | **How facts are stored and served.** Arrow schemas, Delta Lake tables, DataFusion catalog (`cpg_control`/`cpg_base`/`cpg_python`/`cpg_rust`/`cpg_derived`/`cpg_serving`). `FAB §2.1` is the canonical Cargo dependency baseline. |
| `QRY` | `code_property_graph_semantic_query_specification_v1.3.md` | **How agents ask.** Semantic-first JSON request/response envelope with eight request forms; the agent never sees tables, edge labels, or traversal syntax. |
| `LIFE` | `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` | **How it stays current.** Watcher → dirty registry → update waves → invalidation → two-speed lanes → hot snapshot → durable publication. Also defines the runtime topology and recommended crate split (§155). |
| `SRV` | `present_state_cpg_fastmcp_serving_specification_v1.3.md` | **How agents connect.** FastMCP 3.4.7 STDIO server, one process per agent, Pydantic contracts and `pydantic-settings` configuration over the semantic query contract. |
| `RM` | `codefabric_1.3_implementation_roadmap_v1.0.md` | **In what order.** Twenty waves, W0 (repository foundation) through W19 (production acceptance). Explicitly subordinate: where it disagrees with the specs or manifest, they prevail (`RM §0`); its §29 traceability table is corrected by `docs/spec_index/wave-traceability.md`. |

End-to-end data path (fact generation §6 + lifecycle §93 + fabric §1):

```
source change → dirty registry → update wave → source images → invalidation plan
  → fast syntax lane (Tree-sitter)          → immutable syntax-current snapshot
  → semantic lane (Ruff+Pyrefly / rustc+MIR) → normalization → reconciliation
  → derived lane (petgraph, fixed-point)     → interprocedural summaries
  → validated immutable hot snapshot         → async Delta publication → DataFusion serving
```

Runtime topology: one central **Rust daemon per workspace** owning source state, snapshots, provider orchestration, query execution, and capability status; one **FastMCP STDIO process per agent** as presentation only — it must never hold independent mutable CPG state (lifecycle §122).

### The index layer (`docs/spec_index/`)

A derived navigation and traceability layer over the suite — **never normative**: cite the
section it points at, not the index. Where two specs appear to disagree, `SUITE AC-G-01` is
the precedence rule.

| File | Answers |
|---|---|
| `README.md` | how to cite (`TAG §N`), the suite census, the 110 `# Part`/`# Appendix` headings `spec-outline` cannot see, and the gap register: absent referents, registries that exist only as generated `contracts/` artifacts, known inter-spec conflicts |
| `fact-domain-map.md` | one fact domain traced across all six domain specs, plus the table and serving registries |
| `library-routing.md` | which `docs/library_ref/` chapter covers a given spec section; the version-pin ledger |
| `wave-traceability.md` | which spec sections and contracts each roadmap wave implements (corrects `RM §29`) |
| `contract-census.md` | all 84 `AC-G` contracts with owner, consumers, and wave |
| `invariants-and-doctrine.md` | the invariants every wave must preserve, traced to their normative homes |

Consult the gap register before concluding a search failed: several cited authorities
(requirement IDs, the flag registry, property names) are build outputs under `contracts/`,
not prose that exists anywhere yet.

## Cross-cutting doctrine (violating these contradicts every spec at once)

- **Fact substrate, not judgment.** The system emits facts and mechanically derived facts. It never encodes `SAFE_TO_REFACTOR`, `TEST_IMPACTED`, `HIGH_RISK`, `SHOULD_CHANGE`, complexity verdicts, or test-impact conclusions. The query service *rejects* evaluative requests; the fact-equivalent form is the allowed rewrite. Excluded domains: git history, runtime observation/coverage, environment inventory.
- **Absence is never proof of absence.** Missing provider output must materialize as an *explicit unknown* or *capability gap*, never as an empty result implying "none". Compile failure yields capability gaps, not stale-current compiler facts.
- **Raw and normalized coexist.** Every syntax/MIR fact keeps both the provider-native kind and the normalized kind; normalization must not block representing new grammar or compiler variants.
- **Syntax occurrence ≠ semantic entity.** Call syntax is not a callable; type syntax is not a type. Call sites are first-class entities, not just caller→callee edges.
- **Canonical identity is application-owned.** Raw `DefId`, MIR local/block indices, Tree-sitter node IDs, Ruff node indices, and Pyrefly internal keys are never canonical identity (generation §13). Rust prefers `StableCrateId + DefPathHash`.
- **Provider isolation.** Every provider sits behind an application-owned adapter emitting application-owned DTOs; no long-lived borrowed provider types (e.g. `Node<'tree>`) escape an adapter.
- **Authority, never silent overwrite.** Conflicting provider facts are resolved by the per-fact-family authority tables (generation §5); conflicting evidence is retained in provenance/diagnostics, and unresolvable conflict emits an unknown or multi-candidate fact.
- **Atomic present state.** Owner-scoped replacement + manifest-pinned multi-table MVCC: a publication maps to an exact Delta version per table, and intermediate versions are invisible through `cpg_serving` until the pointer advances. Every query pins exactly one immutable snapshot and never mixes source generations.

The lifecycle spec §§157–159 lists the mandatory consistency, performance, and failure invariants — check any incremental-update design against them.

## Library references (`docs/library_ref/`)

Deep, **version-pinned** references for the exact dependency baseline the specs assume (Arrow/Parquet 58.4.0, DataFusion 54.1.0, delta-rs 1.0.0 @ rev `9f922319…`, `object_store` 0.13.2, tree-sitter 0.26.12, Ruff 0.16.1, Pyrefly 1.2.0, petgraph 0.8.3, `notify-debouncer-full` 0.7.0, FastMCP 3.4.7, Pydantic 2.13.4, FastAPI 0.141.1, `rustc_public` 1.100.0-nightly). Consult the matching file before writing code against any of these — the specs pin exact versions and several APIs are pre-release or nightly-sensitive. The canonical Cargo workspace baseline lives in the data-fabric spec §2.1.

## Environment

`direnv` (2.37.1) is hooked into zsh; `.envrc` syncs the uv venv, puts `.venv/bin` and
`scripts/` on PATH, and documents the `CODEFABRIC_*` variables the serving spec expects.
It needs `direnv allow` once. Secrets — notably `CODEFABRIC_CPG_CAPABILITY_TOKEN` — belong in
`.envrc.local`, which `.envrc` sources and `.gitignore` excludes.

**Agents do not inherit any of that.** Each Bash call runs in a fresh non-interactive shell,
which direnv's prompt hook never touches and which inherits nothing from the previous call.
So invoke tools one of three ways rather than assuming an activated environment:

```bash
uv run <cmd>                     # Python — no activation needed
direnv exec . <cmd>              # full .envrc environment, non-interactively
. scripts/bootstrap.sh && <cmd>  # within a single compound command
```

`scripts/bootstrap.sh` is the shared mechanism: sourcing it applies the environment, running it
checks one. `./scripts/bootstrap.sh` reports tool and toolchain state against the spec anchors
and exits non-zero on a real problem; `--quiet` is silent when healthy. A `SessionStart` hook in
`.claude/settings.json` runs `--context` and injects the result, so each session opens knowing
the environment state without probing for it.

`rust-toolchain.toml` pins **stable** and is the only toolchain ordinary work needs. Nightly is
installed and stays reachable through `cargo +nightly` for `just miri` and `just udeps` only;
`./scripts/bootstrap.sh` reports its absence as a note rather than a failure.

If MIR extraction via `rustc_public` is eventually built, that is a deliberate architectural
change, not an edit to the pin: it requires an exact date-pinned nightly plus `rustc-dev`,
matching `rust-src` and `llvm-tools-preview`, a semantic golden corpus, and an explicit nightly
upgrade process (repo spec §76). `docs/library_ref/rust_mir_cpg_continuous_reference_2026-08-18.md`
carries the pin that subsystem would use. Until then, `cargo-show-asm` and stable compiler output
are the lower-cost tools for inspecting codegen.

## Project skills (`.claude/skills/`)

Ten design/planning/review skills, installed from `revised_code_design_skills_visible/`.
Intended flow: `design-development` → `library-capability-research` → `impl-plan` → `plan-audit`
→ `integrate-plan-audit` → `impl-plan-exec` → `impl-status` → `implementation-review`, with
`lib-leverage` and `skill-eval` as standalone tools. Shared policy lives in `_shared/`.

They were authored for a different project and have been re-grounded on this repo: the doctrine
and O01–O18 criteria they cite now exist in `docs/library_ref/`, and their structural-research
guidance — which previously routed through an MCP server that does not exist here — has been
rewritten around `ast-grep` and `rg`.

**Note the source bundle is now stale.** `revised_code_design_skills_visible/` and its `.zip`
still contain the original text, and `install.sh --force` would overwrite the corrections.
`.claude/skills/` is the source of truth.

## Repository research

`_shared/code-intelligence.md` is the reference; it defines a three-tier instrument ladder and
is worth reading before making any claim about what the code contains or what has reached zero.

| Tier | Tool | Proves |
|---|---|---|
| 1 | `cargo check`, `uv run ruff` | Caller coverage by construction — change the symbol, rebuild clean |
| 2 | `ast-grep` 0.45.1 | Structure: call sites, impls, subclasses, signatures |
| 3 | `rg` 15.2.0 | Literal residue: strings, config, comments, cross-language |

The design artifacts in `docs/upfront_design/` are navigable by section: `spec-outline` maps all
of them to 28 KB (23x smaller than reading them), one line per section with a line number, and
`spec-outline <spec>.md --match '^93\.' --view expanded` zooms to one section's subsections. It
wraps `ast-grep outline` with the project extractor at `tooling/ast-grep/outline/specs.yml`;
`tooling/ast-grep/outline/specs.test.sh` pins its output shape against grammar drift. Section
numbers move when a spec is revised — confirm a citation with `--match` before trusting it.

One thing `spec-outline` does **not** emit is the `# Part` and `# Appendix` headings — all 110 of
them are invisible to it. They are tabulated in `docs/spec_index/README.md` §3.1; that
directory's per-file map is in *The design corpus* above. It is navigation only — never cite it
as authority, cite the section it points at.

The library references in `docs/library_ref/` have their own navigator, `lib-outline`, because they
are rooted a level higher: chapters are `#`, subsections are `##`, so `spec-outline`'s h2/h3 mapping
finds no chapters and flattens the rest — on the MIR reference that silently hides all 18
appendices, 42% of the file. `lib-outline` maps the whole directory to ~1,670 lines (from 11.9 MB),
and `lib-outline <ref>.md --match '^Appendix M' --view expanded` zooms to one chapter. Each script
refuses the other's tree rather than emitting a misleading outline. Extractor:
`tooling/ast-grep/outline/library-ref.yml`; shape pinned by `library-ref.test.sh`.

Prefer either over `rg` for headings in those files: both parse markdown, so a `# Cargo.toml` line
inside a fenced block cannot masquerade as a heading, and a document's own table of contents nests
under its map rather than colliding with real sections. `--match` filters items, not members — zoom
to the chapter, then `--view expanded`.

For code, reach for `ast-grep outline` before opening a file: `outline <dir> --items exports`
maps a subtree's public surface, and `--view expanded --match '^Name$'` inspects one type's members
without reading the body. It is syntax-only — no reference resolution, no types, no call graph.

Deep references are `docs/library_ref/ast-grep_0.45.1_advanced_reference.md` and
`docs/library_ref/ripgrep.md`. Three traps that silently shrink or mislead results here: `.claude/`
is hidden so default `rg` cannot see the skills (use `--hidden -g '!.git/**'`); `docs/library_ref/`
is ~9 MB of prose that swamps unscoped searches (exclude with `-g '!docs/library_ref/**'`); and
exit codes are not uniform — `ast-grep run` returns 1 for a clean no-match while `outline` returns 0
on an empty result. Widening the ignore stack (`rg -uu`, `ast-grep --no-ignore`) reaches `.envrc.local`
and other secrets, so scope it to a path rather than making it a default. Invoke `ast-grep`, never the
deprecated `sg` shim on PATH.
