---
artifact: implementation-plan
plan_id: codefabric-waves-0-3-foundation
version: v2
date: 2026-08-20
status: draft
design_path: docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md
design_version: v1.0
baseline_commit: e14e175071df5c98dfcdeba81dd8bcca3fe91fb0
state_path: docs/plans/state/codefabric-waves-0-3-foundation_state.json
cutover: true
---

# CodeFabric Waves 0–3 Foundation — Implementation Plan v2

This plan converts Waves 0–3 of the CodeFabric 1.3 implementation roadmap into
dependency-closed work packets. It covers:

- **Wave 0** — Program, toolchain, and build foundation (four build domains).
- **Wave 1** — Machine contracts, registries, and code generation (Gate A).
- **Wave 2** — Daemon kernel, workspace registry, path security, source images.
- **Wave 3** — Canonical data fabric, publication, overlay, snapshot kernel.

The repository is pre-implementation: everything in `src/` and
`python/codefabric/` is a toolchain-proving seed with an explicit mandate to be
replaced. This plan is therefore a green-field build plus a deliberate
decommission of the seed's packaging surface.

---

## 1. Outcome and non-goals

### 1.1 Outcome

At completion (Milestone M04):

1. A clean checkout builds four isolated compatibility domains — stable Rust
   daemon/data-plane, date-pinned-nightly rustc extractor shell, Pyrefly
   sidecar shell, and the Python FastMCP adapter shell — with per-domain
   version identity, per-domain lockfiles where required, and CI that rejects
   duplicate Arrow/DataFusion/object_store/Parquet families (roadmap §5).
2. The complete `contracts/` machine-contract tree exists and is the sole
   source for generated Rust and Python compatibility types; regeneration from
   unchanged sources is byte-identical; identity/path/type/enum/canonical-JSON
   known-answer vectors pass in both Rust and Python; the four Protobuf
   packages compile and round-trip in both languages; Readiness Gate A —
   `codefabric-contracts verify` clean under the released profile — passes
   (roadmap §6; manifest Part V; milestone M02).
3. The daemon registers, authorizes, inventories, and captures byte-exact
   source images for Git-worktree and non-Git workspaces under the AC-G-11
   secure-open discipline, persists operational state in SQLite WAL, and
   recovers registration/inventory state across restart without claiming an
   active fact snapshot (roadmap §7).
4. Synthetic owner-scoped canonical facts can be inserted, replaced, removed,
   overlaid, durably published, rebased, leased, and queried through a
   `ServingSnapshot`-pinned overlay-aware DataFusion catalog; a leased query is
   unaffected by a later active-snapshot swap; crash/restart at publication and
   pointer boundaries recovers to one coherent current state; overlay merge
   equals the durable effective state under canonical comparison (roadmap §8).

### 1.2 Non-goals (explicitly deferred by the roadmap)

- Real providers (Tree-sitter, Ruff, Pyrefly, rustc execution) — Wave 4+.
- Reconciliation authority tables and derivations — Wave 4 (minimal, roadmap
  W4.7) then Waves 5/12+ for canonical/full reconciliation.
- Watcher-driven updates and freshness barriers — Wave 6.
- Git status/index/HEAD acceleration beyond discovery correctness — Wave 7.
- The public semantic query language, RPC service behavior, FastMCP tools —
  Waves 15–18. Waves 0–1 generate their *contracts* only.
- Windows support (excluded by `local-workstation-v1`, spec §0.6).
- Object-store (S3) backends: spec §2.1 now keeps `s3` **out** of the default
  delta-rs feature set, exposing it behind a CodeFabric `s3-storage` feature.
  All Wave 3 storage runs on the local filesystem, so the baseline build
  compiles no AWS dependency. WP01 declares the feature table; nothing in
  Waves 0–3 enables it.

### 1.3 Wave-boundary discipline

Roadmap §28: a detailed plan SHALL NOT introduce new high-level design
decisions; discovered ambiguities are returned to the owning 1.3 specification
as design issues. This plan's Ambiguity Register (§13) classifies every known
gap as: resolved-by-precedence, contract-authoring (Wave 1 mints machine
values the prose leaves open — authorized by AC-G-70's "machine registries are
the code-generation authority"), packet preflight probe, recorded deviation,
or design issue returned to the spec owner.

---

## 2. Source design and governing decisions

### 2.1 Design inputs and digests (staleness boundary)

| Artifact | SHA-256 |
|---|---|
| `codefabric_1.3_implementation_roadmap_v1.0.md` | `06f3a9d530db83d514b6ef2c148cce87482dccff277d3eee628c1994e20ea2c3` |
| `code_property_graph_present_state_fact_ontology_specification_v1.3.md` | `ab4d179614822be04995e6ccd3401bed44b0801f75e9ef977af01b86dab49fba` |
| `present_state_cpg_fact_generation_specification_python_rust_v1.3.md` | `9c29e33dfa5b50638fb38d1bca649fa227a32caf2c8435274890afccf81d4b4b` |
| `present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md` | `33ed119cd269ae7a6b246ba422f7b9d191ab79770c4b017d7c0b430f98384420` |
| `codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md` | `a5763a54b71993b9162f65af2a17be84ac2e25e4ce0f38b3c8046c216d7e8fe0` |
| `code_property_graph_semantic_query_specification_v1.3.md` | `b2ed90f0da26620c44a128cfa828b5ba611c0f89d5aa0009de79376937493a3b` |
| `present_state_cpg_fastmcp_serving_specification_v1.3.md` | `8ee8ec7dbc06b1a8ac1b03d882abe33fef5d3dbe70e35446e56dbfd399f25524` |
| `codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md` | `61057ee10905d4616dc6893f96ebe895464c0ca3374a772e148a24943de1a8f8` |

Also governing, at the tooling layer:
`docs/rust_core_python_interface_repository_specification_2026-08-20.md`
("repo-spec"), authoritative for package/repository/tooling architecture, and
the doctrine at `docs/library_ref/semantic_design_principles_holistic.md`.

The Data Fabric and Suite-manifest digests were restamped after the
storage-substrate revision described below; the other six are unchanged from
first planning.

(The digests above are the plan's staleness-boundary record and use SHA-256;
contract fingerprints inside `contracts/` use the `b3:` digest form per the
manifest's AC-G-02 (BLAKE3-256 binding per AC-G-07 — audit C-1), including the manifest-carried digests of the prose
documents themselves.)

**In-flight spec revisions captured at planning time.** The data-fabric spec
was revised in place twice during planning.

*First revision.* §2.1 moved to **Arrow/Parquet `=58.4.0`** and **DataFusion
`=54.1.0`**, and the DataFusion/Arrow/deltalake reference docs in
`docs/library_ref/` were refreshed to the same family. LD-01/LD-02 carry those
pins.

*Second revision — storage-substrate restamp.* §2 and §2.1 moved the delta-rs
pin from `35cfed45…` to **`9f9223197469897ef05ae4369eb4fd1390174e65`**, raising
the Rust floor to **`1.94.1`** (set by the pinned revision itself, after
upstream AWS crates lifted their MSRV), adopting Cargo **resolver 3**, and
dropping **`s3`** from the default delta-rs feature set in favour of a
CodeFabric `s3-storage` feature gate. Arrow and DataFusion were **not** moved:
the pinned revision declares `arrow = "58"` and `datafusion = "54.0.0"` as
caret requirements, both satisfied by the exact `=58.4.0`/`=54.1.0` pins. The
same revision added §2.2's transitive Delta-kernel rule and eleven new
subsections (§12.5–§12.9, §67.3, §98.1–§98.3, §100.1, §101.1, §103.4, §111.1,
§112.6) covering the snapshot/provider boundary, access profiles, nested-schema
optimize, action paths, activation metrics, and a delta-rs upgrade gate. LD-04
carries the new pin; the old `deltalake_rust.md` reference was replaced by
`deltalake_rust_1.0.0_9f922319_advanced_reference_2026-08-20.md` (chapter
numbering unchanged, so every `deltalake ref §N` citation in this plan still
resolves).

The digests above reflect the revised files. The library grounding for these
decisions was performed against the same-family reference docs; WP19's
preflight compile-verifies the exact pins.

**Suite governance manifest.** The manifest
(`codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md`)
was absent at planning start and was added to the repository during planning;
it is fully integrated here. It owns `AC-G-01`–`AC-G-08` (ownership map,
artifact versioning, compatibility matrix, CF-ID traceability, the exact
`contracts/` layout, enum/flag registry rules, bundle manifests, and the
`local-workstation-v1` deployment profile), the `G-01`–`G-84` ownership
crosswalk, the acceptance harnesses (`AC-G-78`–`AC-G-84`), and Part V's
readiness gates. **Gate A (verbatim):** "All registries, schemas, protocol
definitions, identity vectors, manifests, and traceability files exist and
pass `codefabric-contracts verify` without released-profile warnings." Part V
also rules: "An LLM programming agent SHALL NOT be asked to invent a missing
gate contract during implementation" — gaps go back to the owning document as
specification issues, which is exactly this plan's ambiguity-disposition
policy (§1.3).

### 2.2 Precedence rules used throughout this plan

1. Each spec's `AC-G-*` completion contracts override less-specific prose in
   the same document (each spec §0: "A less-specific statement elsewhere …
   SHALL NOT override them"). Concretely applied: AC-G-19 over Data Fabric
   §13.8 (snapshot manifest); AC-G-58 over Serving §9 (9-RPC service, unary
   `StartQuery`); AC-G-73 over Ontology §66 (unknown-kind names); AC-G-30 over
   Fact Gen §7.3 (six wire operations). (The Lifecycle §43-over-Appendix-F
   path-struct resolution, previously listed here, is **not** an AC-G
   precedence — the appendix conflict is intra-document and unannotated; it
   is a recorded editorial resolution, A-54, with an ISSUE filed.)
2. The §0.2 ownership table routes each concern to exactly one owning spec; a
   downstream layer consumes the upstream machine artifact and never recreates
   it.
3. The repo-spec governs package/tooling mechanics (crate boundaries §0.3,
   workspace graduation §77–79, assurance tiers). Where a 1.3 spec recommends
   an internal crate decomposition (Lifecycle §155, Fact Gen §89, Data Fabric
   §113), those are treated as *module/boundary* obligations inside the single
   stable-domain crate unless a §0.3 criterion independently justifies a
   package (D-02 below).

### 2.3 Governing decisions

- **D-01 — Four build domains as three Cargo roots plus one uv project.**
  The stable daemon/data-plane remains the root package `codefabric` (single
  crate, per repo-spec §0.3/§77). The nightly rustc extractor
  (`rustc-extractor/`, package `codefabric-rustc-extractor`) and the Pyrefly
  sidecar (`pyrefly-sidecar/`, package `codefabric-pyrefly-sidecar`) are
  **standalone Cargo roots outside any shared workspace**, each with its own
  `Cargo.lock` (and, for the extractor, its own `rust-toolchain.toml` pinning
  nightly-2026-08-18). Justification per repo-spec §0.3: materially distinct
  toolchain/platform requirements (#3), separately built artifacts (#5), and
  hard dependency isolation with an independent lockfile (#2 — mandated
  verbatim by roadmap Wave 0 WP3; WP2 says "separate executable/build
  domain", carried by #3/#5 — audit C-22). They cannot be workspace members:
  a workspace shares one lockfile, which alone violates the isolation
  mandate (per-directory `rust-toolchain.toml` files nest fine, so the
  lockfile argument is the load-bearing one — and sufficient). The Python adapter is `codefabric-cpg-mcp/` exactly
  per Serving §54. Doctrine: P6 ports-and-adapters (Advances), P8 trust
  boundaries (Advances).
- **D-02 — No `[workspace]` in Waves 0–3.** Data Fabric §2.1 presents its
  canonical baseline as a workspace skeleton; its normative content is the
  exact pin set, edition 2024, and `rust-version = "1.94.1"`. With exactly one
  stable-domain package, the pins live in `[dependencies]` of the root
  `Cargo.toml` with identical `=` versions. §2.1's `resolver = "3"` is a
  `[workspace]` key and therefore inert here — edition 2024 already selects
  resolver 3 for a standalone package, so the requirement is satisfied by
  construction and the explicit key is adopted with the workspace form. The workspace *form* is adopted
  only when a second stable-domain package clears repo-spec §0.3 (recorded
  deviation; see Ambiguity Register A-30).
- **D-03 — Boundary isolation is enforced by executable governance, not by
  crates.** The gix-isolation rule (Lifecycle §155: only the git-state
  boundary sees gix types), the delta-rs non-leak rule (Data Fabric §114), and
  the provider-DTO rules (Fact Gen §7), and the adapter-side rule that no
  FastMCP/Pydantic-internal type or module crosses into the public contract
  modules (roadmap §5 exit's fifth boundary) become executable governance:
  `ast-grep` rules in `rules/` run by `ast-grep scan` in CI (Rust and Python
  grammars), plus module-privacy boundaries in the root crate. The harness
  (`sgconfig.yml` + empty ruleset) bootstraps in WP01 so every later packet
  can land rules with its code. Doctrine P31 executable governance
  (Advances).
- **D-04 — Generated artifacts are committed.** `contracts/` holds the
  declared sources; generated outputs (canonical JSON registries, JSON
  Schemas, `.proto`, Rust/Python code) are committed under `contracts/generated/`
  and per-domain `src/generated/` locations; CI regenerates and fails on any
  byte difference (spec §0.5: "reproducible from one declared source and
  compared by canonical digest in CI"). This lets the three other domains
  consume contracts without cross-domain build dependencies.
  `contracts/generated/` is the **authority copy**; per-domain
  `src/generated/` trees are derived mirrors whose byte-identity with the
  authority copy is asserted by `contracts-regen-check` (accepted
  duplication — the single-committed-location alternative was deferred to
  avoid cross-domain `include!` path coupling; audit Q2). Hygiene: all
  generated trees are marked `linguist-generated` in `.gitattributes` and
  excluded from the fmt/typos/ast-grep/machete surfaces, and the generator
  must emit rustfmt-stable output so the fmt gate and the byte-identity
  gate never fight (WP05 item 7).
- **D-05 — The seed's PyO3/Maturin packaging surface is retired in Wave 0**
  (L-01/L-03). The 1.3 architecture has no PyO3 boundary: the adapter is a
  thin gRPC client over UDS (Serving §4, §8, §18 — "does not require Arrow,
  DataFusion, Delta Lake … or an HTTP framework"), and the daemon is a native
  Rust service. Keeping a dead cdylib/wheel pipeline would preserve a false
  compile surface and dual Python packaging authority. Doctrine: clean-sheet
  challenge applied; anti-principle "duplicate authority" avoided.
- **D-06 — Two persisted workspace state machines with a derived public
  projection.** The three overlapping machines (Lifecycle §18 runtime
  lifecycle, AC-G-10 registry machine, AC-G-28 public startup states) are
  realized as **two persisted machines plus one projection**: the AC-G-10
  registry machine persisted in the operational store; the §18 lifecycle as
  the coordinator's runtime state (persisted in
  `worktree_state.lifecycle_state_code` — §130.2's single lifecycle
  column); and AC-G-28 startup states as a derived projection whose inputs
  are the §130.2 four-column tuple (`lifecycle_state_code`,
  `source_trust_state_code`, capability status, snapshot usability) — e.g.
  `BEST_AVAILABLE_STALE` needs source trust, `READY_WITH_CAPABILITY_GAPS`
  needs capabilities, `VERIFYING_DURABLE_SNAPSHOT` needs snapshot state.
  All are generated from the AC-G-25 registry YAML (Ambiguity A-14; audit
  Q3).
- **D-07 — Wave 3 synthetic ingest is a bounded transition with the real
  reconciliation signature.** §72 makes the `ReconciliationEngine` the sole
  canonicalization authority, but real reconciliation arrives Wave 4
  (minimal, roadmap W4.7) through Wave 12 (full). Wave 3 implements the §63
  observation boundary (validated Arrow streams, manifests, generation
  fences) plus a `SyntheticCanonicalIngest` that is the *only* ingress and
  is explicitly marked as the pre-reconciliation stub. The ingress
  implements the §72/§73.1 signature from the start — N observation streams
  plus a provider-precedence input, returning canonical batches **plus**
  `fact_evidence` rows **plus** conflict records — with the synthetic
  implementation as the degenerate single-authority case, so the Wave 4/5
  replacement is a body swap, not a signature change at every WP21/WP22
  call site (audit Q4). Exit condition: replaced by the roadmap's minimal
  reconciliation packets (W4.7/W5.3). Owner: WP20. Doctrine: bounded
  transition recorded per doctrine policy.
- **D-08 — Operational vs control-plane authority split.** The SQLite
  operational store is the authority for registration, lifecycle, leases,
  snapshot manifests/pointers, and all high-churn state (AC-G-27); the Delta
  control-plane tables (`workspace`, `common_repository`, `analysis_context*`,
  `publication*`, `current_publication`, `owner`, `capability_status`,
  `diagnostic`) are the durable, publication-pinned registry the fabric owns
  (Data Fabric §13). The Delta `workspace` row is written from the
  operational registry and never edited through query SQL; it is upserted
  on **every registry revision bump** (enable, relink, configure — not only
  enable/publication) and carries `registration_revision` + `updated_at`
  (A-44), so a relinked workspace is never silently stale in Delta. `cpg_control` exposes both with disjoint names (`workspace` =
  Delta; `worktree_state` and friends = SQLite read-only projections per
  §13.12); a name may appear in only one backing store.
- **D-09 — Deep-integration toolchain posture (user decision, 2026-08-20).**
  The date-pinned nightly (`nightly-2026-08-18`, components `rustc-dev`,
  `rust-src`, `llvm-tools` — the canonical rustup name per the MIR
  reference; rustup accepts the legacy alias `llvm-tools-preview`, which
  the root stable toolchain file still uses — audit L-1) with
  `rustc_public` plus narrowly scoped
  `rustc_private` is **settled architecture** for the extractor domain — a
  committed baseline, not a provisional experiment or fallback-laden option.
  Three of repo-spec §76's four adoption conditions are satisfied at Wave 0
  — exact date pin and matching components (WP02) and the managed update
  procedure below; the fourth, the semantic golden corpus, is a registered
  obligation whose first fixtures land with the extractor's first real
  facts in Wave 5 (recorded deviation; audit Q6). The same
  posture applies to Pyrefly: an exact source-rev pin (git rev preferred so
  the sidecar can deliberately track Pyrefly internals) with **deep
  integration through the `pyrefly::query` facade** despite its
  "not intended for external use" label. The build-domain and process
  boundaries (D-01) are retained — AC-G-30/31 mandate them, and they are
  what makes aggressive pinning manageable: an update's blast radius is one
  toolchain/lockfile domain, and digest negotiation (`TOOLCHAIN_MISMATCH`,
  `BUNDLE_DIGEST_MISMATCH`) fails fast instead of drifting.
  **Managed update procedure** (both domains; deliberate per update, never
  automatic): (1) bump the pin on a branch (nightly date or Pyrefly rev);
  (2) rebuild the domain alone; (3) run the domain's conformance surface —
  extractor: `rustc_public` link smoke + semantic golden corpus (once it
  exists) + protocol round-trips; sidecar: Query-facade link smoke +
  AC-G-30 protocol conformance + type-table goldens (once they exist);
  (4) update the toolchain-bundle record and digests (AC-G-02 artifact
  digests, consumed by AC-G-14's analysis-context manifests — audit C-3;
  §4.1 item 6); (5) daemon↔extractor/sidecar version negotiation keeps a stale
  domain from feeding a newer daemon. Doctrine: P25 reproducibility
  (Advances — hermetic pins), P29 versioned contracts (Advances).

---

## 3. Current baseline and staleness boundary

- Baseline is a **commit-plus-working-tree identity**: commit
  `e14e175071df5c98dfcdeba81dd8bcca3fe91fb0` (branch `master`) plus these
  uncommitted changes, all treated as part of the baseline contract:
  - modified: the data-fabric spec (§2.1 pin bump to Arrow 58.4.0 /
    DataFusion 54.1.0), the six DataFusion/Arrow/deltalake reference docs and
    their navigator skills, `CLAUDE.md` (pin table), `pyproject.toml` +
    `uv.lock` (adds `attrs>=26.1.0`, `cattrs>=26.1.0` as root runtime deps);
  - untracked: `AGENTS.md`, the suite governance manifest,
    `docs/spec_index/`, `docs/plans/` (this plan).
  Execution preflight SHOULD start from a commit of this tree so packet
  diffs attribute cleanly.
- `just ci-fast` passes clean against exactly this working tree (fmt, both
  compile surfaces, clippy, ruff, pyrefly, nextest 9, doctests 2, pytest 12,
  typos, machete/shear). **There are no pre-existing failures**; any red gate
  during execution is plan-caused.
- Note on `attrs`/`cattrs`: added to the root pyproject at baseline. The
  serving spec's adapter dependency block (§18) does not include them; L-03
  retires the root pyproject. Whether they move into the adapter project (as
  internal, non-public-contract libraries per the doctrine's
  contract-substrate note under P29) is a WP04 decision to confirm with the
  user; they must not become a second public-contract mechanism beside
  Pydantic (I-07; Serving §6 invariants 13/15 — schema-closed public
  output; §19.5 concerns the daemon-owned full schemas, audit C-17).
- `.python-version` at baseline contains `3.14` (interpreter resolved:
  3.14.7). The adapter project pins its own interpreter (A-40).
- Environment (SessionStart report): uv 0.12.5, Python 3.14.7, stable Rust
  1.97.1 (satisfies the §2.1 floor 1.94.1), nightly 1.100.0 available,
  sccache/just/nextest/maturin/typos/rg/ast-grep/cargo-deny/audit/shear/
  machete present, direnv allowed.
- Seed inventory to be dispositioned: `src/lib.rs` (127 lines), `src/python.rs`
  (46), `python/codefabric/` (56 incl. stub), `tests/integration.rs` + 2 cases,
  `python_tests/` (2 files), Maturin backend in `pyproject.toml`, `python`
  cargo feature, wheel scripts/recipes.
- Stale-documentation notes: `CLAUDE.md`'s v1.2 spec filenames were corrected
  to `_v1.3` (and the governance manifest added as the seventh artifact) before
  execution; what remains for WP01/WP05 (L-04) is its PyO3 two-surface
  narrative, plus the seed manifests' repo-spec section citations.
- Known repository traps that apply to all packet preflights: `.claude/` is
  hidden from default `rg`; `docs/library_ref/` (~12 MB) must be excluded from
  unscoped searches; `ast-grep run` exits 1 on clean no-match.

---

## 4. Global target invariants

Each packet cites the invariants it advances or must not regress.

- **I-01** `workspace_id` identifies exactly one authorized analyzed source
  instance; it is derived from a persisted 128-bit registration nonce, never
  from the root path (AC-G-09).
- **I-02** One immutable leased `ServingSnapshot` is the only query pin; a
  leased snapshot is never affected by later activations (suite inv. 2;
  AC-G-19/23).
- **I-03** Current stable filesystem bytes are present-state authority; the
  CodeFabric BLAKE3-256 content digest is canonical content identity; blob
  OIDs and watcher events never substitute (suite inv. 3; Lifecycle §2.1,
  §157.13–15).
- **I-04** Provider observations are not canonical until reconciled; in this
  plan's scope the sole canonicalization ingress is the bounded
  `SyntheticCanonicalIngest` (D-07); providers/encoders never write canonical
  tables directly (suite inv. 4; Fact Gen §86; AC-G-32).
- **I-05** Every fact row carries `workspace_id` and `analysis_context_id`
  (`context:source` for context-independent facts); exact facts never merge
  across contexts or workspaces (suite inv. 5; Data Fabric §9, §15).
- **I-06** Absence is never proof of absence: unknown kinds, capability gaps,
  and explicit-negative families are registry-defined from Wave 1; a null cell
  never means "unknown value" (suite inv. 6; AC-G-71 Decision + rule 3,
  AC-G-73 — audit C-7).
- **I-07** The Rust daemon owns semantics, planning, snapshots, and canonical
  bytes; the Python adapter stays a thin client and never grows a second
  engine (suite inv. 7–8; Serving §5–6).
- **I-08** Every compatibility-sensitive artifact is versioned and
  fingerprinted; regeneration from unchanged sources is byte-identical; CI
  compares canonical digests (suite inv. 9; §0.5).
- **I-09** Incremental results converge to clean-rebuild results; Wave 3's
  instance: consolidated overlay state equals the corresponding durable
  effective state under canonical comparison (suite inv. 10; AC-G-22).
- **I-10** Domain isolation: no compiler-owned, Pyrefly-internal, gix, delta-rs
  internal, or FastMCP-internal type crosses an application-owned boundary;
  provider/adapter DTOs are application-owned (Wave 0 exit; Fact Gen §7;
  doctrine P6).
- **I-11** `contracts/` machine registries are the sole code-generation
  authority; no downstream layer re-declares a registry, identity rule, enum,
  or status mapping (spec §0.2, §0.5; AC-G-70; doctrine P10).
- **I-12** Exactly one Arrow/Parquet/DataFusion/object_store version family
  crosses public type boundaries; CI rejects duplicates (Data Fabric §2.2).
- **I-13** Registry codes are append-only; names are never reassigned;
  orthogonal state dimensions are never collapsed into one status
  (Ontology §62.10, §62 orthogonality rule; doctrine P12).
- **I-14** All storage mutation is idempotent and generation-fenced: writes
  carry publication/operation identity, retries inspect prior outcomes, stale
  generations are rejected (Data Fabric §70; AC-G-22/26/28; doctrine P24).
- **I-15** The system emits facts and mechanically derived facts only — no
  evaluative ontology anywhere, including registries (Ontology §67).

### 4.1 Cross-cutting per-packet obligations (roadmap §2.4, §27)

These apply to **every** packet from WP06 onward and are part of each
packet's acceptance even where not restated:

1. **Traceability.** Each packet adds/updates its `CF-*` records in
   `contracts/manifests/requirements.jsonl` (+ `traceability.jsonl`
   `implements`/`verified_by` edges) and leaves the AC-G-04 zero-orphan
   check green. M03 and M04 gates re-run it.
2. **Fault points.** Every packet that introduces a state transition, write
   boundary, pointer swap, or process boundary registers its deterministic
   fault points in `contracts/faults/fault-point-registry.yaml` in the same
   packet (owners in this plan: WP13, WP14, WP16, WP18, WP21, WP22, WP23,
   WP24). A verifier rule checks every injected fault point has a registry
   record and vice versa.
3. **Security corpus.** Adversarial fixtures are registered in
   `contracts/security/security-corpus-manifest.yaml` as they are created
   (WP15 registers the path/symlink corpus; WP16 the capture-race harness).
4. **Comparison rules.** Canonical-comparison ignore rules (e.g., overlay
   rebase equality in WP23, clean-rebuild checks later) are declared in
   `contracts/comparison/comparison-ignore-registry.yaml`, never inline.
5. **Metrics.** Each Wave 2/3 packet's Operational acceptance names its
   metrics; WP11's suite manifest carries the consolidated metric-name list
   so names stay stable (roadmap §27.5). Limits fail explicitly — no silent
   truncation.
6. **Bundle updates at adoption.** When a packet first adopts a pinned
   dependency family (WP17 gix; WP19 Arrow/DataFusion/delta-rs/object_store),
   it updates the toolchain bundle record and re-runs `just contracts-verify`
   in its packet gates (roadmap §27.6: fixtures and fingerprints from the
   boundary's first wave — WP02/WP03 emit their identity/digest records in
   Wave 0 for WP11 to bundle).
7. **Contract edits after M02.** Any packet that changes a `contracts/`
   source regenerates and re-verifies in-packet; Gate A artifacts are never
   left stale within a packet boundary.

---

## 5. Library decisions carried into execution

Evidence tiers: **spec-pin** (normative version in a 1.3 spec), **ref-doc**
(verified against the version-pinned reference in `docs/library_ref/`),
**probe** (requires a compile/behavior probe at the stated packet's
preflight). All `=` pins are exact.

| ID | Decision | Version / pin | Evidence and caveats |
|---|---|---|---|
| LD-01 | DataFusion is the query/catalog/execution engine | `datafusion = "=54.1.0"` | spec-pin (Fabric §2.1); ref-doc: custom Catalog/Schema/TableProvider confirmed; `Any` is a supertrait in 54 (no `as_any`); planning APIs are sync — keep metadata snapshots cheap. **Memory pool is unbounded by default**; WP25 must configure bounded pool + spill (Fabric §98). |
| LD-02 | Arrow/Parquet are the batch and file contracts | `arrow* = "=58.4.0"`, `parquet = "=58.4.0"` (features `arrow`,`async`,`object_store`) | spec-pin; ref-doc: typed builders + `with_capacity` confirmed (byte-capacity two-arg forms: compile-verify); `Binary` for 16-byte IDs confirmed (i32 offsets safe at batch scale); IPC stream format for chunk transport; Parquet embeds `ARROW:schema` by default. |
| LD-03 | object_store for storage I/O | `object_store = "=0.13.2"` | spec-pin; local filesystem needs no handler registration (deltalake ref §2.4.1). |
| LD-04 | delta-rs is the durable table authority | git rev `9f9223197469897ef05ae4369eb4fd1390174e65`, `default-features = false`, features `rustls`,`datafusion` (object-store backends behind a CodeFabric `s3-storage` feature gate) | spec-pin; ref-doc: feature names confirmed (§0.6–0.7); create/append/predicate-delete/load-at-version/TableProvider-at-version/vacuum-with-dry-run all confirmed. **Caveats → probes:** (a) no CAS primitive — AC-G-26 realized as read-version-pinned OCC + predecessor check + post-commit reread (WP22 probe + concurrency test); (b) `CommitProperties` metadata **setter unexampled** — WP21 preflight compile probe; no appId/version app-transaction exists at this rev; (c) CHECK constraints are post-create commits via `ConstraintBuilder` (bootstrap = create→constrain while empty); (d) `columnMapping.mode = none` is a hard invariant (DML rejects other modes at this rev); (e) 1.0.0 `write()` returns `DeltaTable`, not a metrics tuple; (f) `DeltaOps` is deprecated at this rev — use the `DeltaTable::{create,write,delete,update,merge,optimize,vacuum}` entry points — and the legacy `DeltaTableProvider` type is removed (kernel-backed `table_provider()` path only; audit L-3). **Do not git-pin the Delta kernel** — this rev consumes released `buoyant_kernel` / `buoyant_kernel_engine` `0.25.x`, selected transitively by the `deltalake` pin plus the committed lockfile (Fabric §2.2); the duplicate-family gate must inspect the resolved graph, since a mismatched kernel pair can introduce a second Arrow universe invisible in our own manifests. `rust-version` floor 1.94.1, edition 2024 (Fabric §2.1). New obligations from the same revision land in later waves: snapshot-scoped `DeltaBaseCatalog` (Fabric §12.6), access profiles and `skip_stats` policy (§98.1–§98.2), nested-schema optimize fixtures (§100.1, §112.3), opaque action paths (§101.1), and the §112.6 upgrade gate. |
| LD-05 | Tokio async runtime + futures | `tokio = "1"` (`rt-multi-thread`,`macros`,`sync`,`time`), `futures = "0.3"` | spec-pin (Fabric §2.1). Version-range pins are as-spec; lockfile freezes exact versions. |
| LD-06 | gix for read-only Git topology | `gix = "=0.86.0"`, `default-features = false`, features `sha1`,`index`,`status`,`attributes`,`excludes`,`dirwalk`,`blob-diff`,`interrupt`,`parallel`,`auto-chain-error`,`tracing` | spec-pin (Lifecycle §39, twice, byte-identical; security floor §157.31); ref-doc feature graph confirmed: `status`→dirwalk+index+blob-diff; `dirwalk`→attributes+excludes; **`attributes` transitively enables `command`** (external-process capability) so the no-external-execution posture is runtime-enforced via trust policy + governance, not feature exclusion; `interrupt` = signal-hook integration (daemon must own signal strategy); the pin omits `revision` (rev_parse/rev_walk absent — acceptable for Wave 2; A-24). **Probes (WP17):** linked-worktree exact-path open; index fingerprint source (no checksum API surfaced in the reference — R-04); open-path write/lock-freedom. CI asserts the resolved gix feature set (`cargo tree -e features -i gix`). |
| LD-07 | Hashing/serde utilities | `blake3 = "1"`, `serde = "1"` + derive, `serde_json = "1"`, `url = "2"`, `tracing = "0.1"` | spec-pin (Fabric §2.1). BLAKE3-128 is defined as BLAKE3-256 truncated to the first 16 bytes (AC-G-13; A-01 disposition). |
| LD-08 | SQLite operational store | `rusqlite` (bundled libsqlite3), version pinned at WP13 adoption | AC-G-27 mandates SQLite WAL with exact pragmas; no 1.3 spec pins the crate version. Probe: WAL + `BEGIN IMMEDIATE` + busy_timeout behavior under the coordinator-sole-writer discipline. |
| LD-09 | Spec-named runtime utilities | `arc-swap`, `async-trait` — pinned at adoption | AC-G-26 names `ArcSwap` for the in-memory pointer; §114/§156/AC-G-32 signatures use `#[async_trait]`. Neither is version-pinned by spec. |
| LD-10 | Rust Protobuf/gRPC toolchain | `prost`/`prost-build` + `tonic`/`tonic-build`, versions pinned at WP05 after a compatibility probe | Serving §8 recommends (SHOULD) and AC-G-61 mandates the private-UDS transport with OS peer-credential verification (audit C-17); no 1.3 spec pins the Rust crates and **no `docs/library_ref/` reference covers tonic/prost** (R-05). WP05 probe: UDS transport, proto3 `oneof`, 4 MiB message cap, Python interop against grpcio-generated stubs. Assumption A-41 (owner WP05, registered in §13): tonic supports UDS server/client on macOS/Linux at the chosen version. |
| LD-11 | Python adapter stack | `fastmcp==3.4.7`, `pydantic==2.13.4`, `pydantic-settings==2.15.0`; `grpcio`, `protobuf`, `orjson` pinned **in `uv.lock`** after the WP04 compatibility test; interpreter pinned 3.14.7 (`requires-python >=3.12` per Serving §18) | spec-pin (Serving §18 verbatim); `pydantic-core` never pinned independently; dotenv loading disabled; STDOUT protocol-only. |
| LD-12 | Nightly extractor toolchain — **committed baseline (D-09)** | `nightly-2026-08-18` + `rustc-dev`,`rust-src`,`llvm-tools` (canonical name; legacy alias `llvm-tools-preview` accepted); `rustc_public 1.100.0-nightly`; exact rustc commit hash recorded at WP02 | spec-pin of the dated nightly (Fact Gen §2 — which names no components; the component set is grounded in the MIR reference's toolchain block, with `rustc-dev` echoed by the governance compatibility table — audit C-18); adopted as settled architecture per D-09 (three of repo-spec §76's four conditions at Wave 0; the golden corpus lands Wave 5 — recorded deviation); the root toolchain stays stable. Consumption mode is `rustc-dev` components; the WP02 probe confirms mechanics only and records the exact commit hash (A-22 → decided). Updates follow the D-09 managed procedure. |
| LD-13 | Pyrefly sidecar dependency — **committed deep integration (D-09)** | Pyrefly `1.2.0`, exact source rev resolved at WP03 (git rev preferred over crates.io so internals can be tracked deliberately); digest-pinned bundle (`pyrefly_bundle_digest`) | spec-pin of the version; **no tag/rev given anywhere in the suite** (A-21). WP03 resolves and records the rev + BLAKE3 bundle digest, and proves the `pyrefly::query` facade links (the Wave 9 integration surface starts proven). The "not intended for external use" label is accepted deliberately; the sidecar boundary + D-09 update procedure are the management mechanism. Replan trigger if 1.2.0 is not resolvable as a pinned source. |
| LD-14 | Wave-4+ provider pins recorded, not yet adopted | tree-sitter `0.26.12`, tree-sitter-python `0.25.0`, Ruff `0.16.1` (component crates `0.0.7`), petgraph `0.8.3`, notify-debouncer-full `0.7.0` (notify `8.2.0`) | spec-pin for tree-sitter/tree-sitter-python/Ruff/petgraph (Fact Gen §2); the notify-debouncer-full/notify versions exist **only** in the notify reference doc — no spec pin (audit C-18). Recorded in the Wave 1 toolchain bundle (WP11) and enforced when first adopted; **not added to `Cargo.toml` before use**, so the machete/shear hygiene gates stay meaningful (A-31). |
| LD-15 | uv remains the Python environment manager; Maturin is removed | — | D-05/L-01. The adapter project is the only Python project; `uv run --frozen` is the locked launch mechanism (Serving §60.2). |

---

## 6. Legacy disposition matrix

| ID | Legacy surface | Disposition | Packet | Deletion-safe when |
|---|---|---|---|---|
| L-01 | PyO3 binding layer: `src/python.rs`, `python` cargo feature, `pyo3` dependency, `crate-type = ["cdylib"]`, `#[pymodule] _native` | **Remove** (D-05) | WP01 | Immediately — nothing outside the seed consumes it. Negative proof at M01. |
| L-02 | Seed library code: `version()`, `normalize_workspace_id()`, `tests/integration/*`, doctests | **Remove**; the root crate keeps a minimal empty lib + one integration target placeholder until WP06 lands real code | WP01 | Immediately. |
| L-03 | Root Python packaging: `pyproject.toml` (Maturin backend, `codefabric` console script, fastmcp/pydantic runtime deps), `python/codefabric/`, `python_tests/`, `uv.lock`, `.python-version`, `scripts/wheel_test.sh`, wheel/`python-develop`/`test-python` recipes | **Remove/replace**: the adapter project `codefabric-cpg-mcp/` (WP04) becomes the only Python project; root `.envrc`/`bootstrap.sh` updated to sync it | WP01+WP04 | After WP04 provides the replacement lint/type/test target. |
| L-04 | Stale docs: `CLAUDE.md` (v1.2 spec table, two-compile-surface narrative), `AGENTS.md` (repo shape, §2/§3/§7 tables), `README.md` bootstrap text, `_shared/code-intelligence.md` seed example | **Update** to the four-domain topology | WP01/WP05 | With their packet. |
| L-05 | Single-domain operational API: `justfile` recipes and `.github/workflows/ci.yml` assuming one crate + Maturin | **Reshape** into per-domain recipe groups + contracts gates; mutating-recipe and Tier discipline preserved | WP05 | With WP05. |

No silent compatibility aliases: after WP01, `rg`/`ast-grep`/`cargo tree` must
show zero references to `pyo3`, `maturin`, `_native`, or `python/codefabric`
outside `docs/plans/` and historical design documents (DB01).

---

## Audit Integration Log

- **Audit:** `docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v1_audit_report_2026-08-20.md`
  (six parallel verification agents: manifest+ontology, data-fabric,
  lifecycle, serving/fact-gen/query/roadmap citations, library references,
  and an independent design challenge).
- **Source plan:** v1 (2026-08-20) → **revised plan:** v2 (2026-08-20, this
  document). No design dossier exists — `design_path` is the roadmap
  specification, and roadmap §28 routes design-level findings back to the
  owning 1.3 spec as ISSUE items — so this is a **plan-only** revision.
- **Revision reason:** three executability blockers (Wave-3 packet cycle;
  two unauthored mandatory state machines; WP08 omnibus with a wrong
  phrase-harvest range), ten upstream spec defects, four library risks, and
  a citation-accuracy sweep.
- **Re-verification.** The data-fabric spec was revised in place (delta-rs
  `9f922319`, Rust floor 1.94.1, resolver 3) *after* the audit's fabric
  agent read it. Every fabric finding integrated below was re-verified
  against the revised file; all eight §2.1 digests were recomputed and
  match this plan's table. Findings the revision overtook are rejected, not
  silently dropped. The rustup component name was probed directly
  (`llvm-tools` is canonical; the legacy alias still resolves).

**Disposition counts:** applied-plan 44 · added-packet 1 · deferred 1 ·
rejected 3.

### Blockers

- `B-1` — `applied-plan`. WP23↔WP24 activation cycle (WP23's "activate
  S_new" and swap-based acceptance need WP24's AC-G-26 transaction; WP24's
  manifest needs WP23's overlay block).
  - Resolution: §15 Wave-3 order is now WP22 → WP24 → WP23; WP24 activates
    over an empty overlay (`overlay_generation` 0 — a valid AC-G-19
    manifest); WP23's and WP24's Dependencies rewritten; checklist
    reordered.
  - Re-verification: dependency text of both packets in v1; AC-G-19's
    overlay block and AC-G-26's activation transaction ownership.
  - Rationale: breaks the cycle without moving any deliverable between
    packets.
- `B-2` — `applied-plan`. `DurablePublicationState` and
  `ServingActivationState` transition tables were never authored though
  AC-G-25's mandatory roster lists both and WP22/WP24 consume them
  (`ServingActivationState` had zero occurrences in v1).
  - Resolution: added to WP08's mandate with §62.8 code authoring (A-45).
- `B-3` — `added-packet` (**WP08b**). The §50–§102 phrase harvest sat on
  the Wave-1 critical path with no Wave 0–3 consumer, and the range was
  wrong: the Query catalog is §50–§94; §95–§102 are Part VII worked
  examples defining no phrases, so v1's counting verifier would have failed
  on eight phraseless sections.
  - Resolution: WP08b created (phrase registry + EBNF + model-pack schema),
    parallel to WP09/WP10, range corrected to §50–§94; WP11 dependencies,
    §15, §16, R-07, and the traceability table updated.

### Spec defects filed upstream (new ISSUE rows)

- `S-1` — `applied-plan` → A-43. §101 retention omits the active snapshot
  and non-expired leases; AC-G-23's five-element set (which WP24 already
  implemented) wins. Re-verified present in the revised spec.
- `S-2` — `applied-plan` → A-11 extended (three mutation taxonomies;
  §91's `query-time derived`). Re-verified at §11/§91 of the revised spec.
- `S-3` — `applied-plan` → A-44; WP09 adds the columns, D-08 defines the
  upsert trigger, WP19 adds the relink test. Re-verified: §13.1 still has
  no revision column.
- `S-4` — `applied-plan` → A-46. Re-verified: the invalid multi-line
  inline TOML persists in the revised §2.1.
- `S-5` — `applied-plan` → A-47; WP18 maps §154's "CURRENT" to
  `GIT_READY`.
- `S-6` — `applied-plan` → A-48 (PREC: AC-G-13's SHALL).
- `S-7` — `applied-plan` → A-06 extended (majority spelling ×10;
  7-vs-12 kinds; SHOULD-vs-mandatory).
- `S-8` — `applied-plan` → A-45; WP08 wording corrected (only §62.1–§62.6
  carry verbatim codes).
- `S-9` — `applied-plan` → A-49 (§9's supersession by AC-G-58 is
  unannotated in the spec; §2.2 already applied the precedence).
- `S-10` — `rejected` (overtaken): the audit read the pre-revision spec;
  the current spec pins `9f922319`, the digests match, and the
  design-change recommendations document is integrated into both the spec
  and this plan. Residual `applied-plan` → A-50 for the missing
  `datafusion_54vs53.md` citation target.

### Library risks

- `L-1` — `applied-plan`. `llvm-tools-preview` → `llvm-tools` in
  D-09/LD-12/WP02. Re-verification softened the audit: the nightly rustup
  manifest lists `llvm-tools` (canonical) but the legacy alias still
  resolves (the root stable toolchain file uses it today), so this is a
  canonicality fix, not the hard `rustup component add` failure the audit
  predicted.
- `L-2` — `applied-plan`. WP22's "any concurrent commit conflicts"
  downgraded from stated fact to probe-confirmed assumption; A-32 already
  owned the probe and the replan trigger.
- `L-3` — `applied-plan`. LD-04 caveat (f): `DeltaOps` deprecated and
  `DeltaTableProvider` removed at the pinned rev.
- `L-4` — `rejected` (already covered): the `>=`-declared root
  `pyproject.toml` is retired by L-03; WP04's adapter project declares the
  exact `==` pins including `pydantic-settings`; grpcio/protobuf/orjson
  lock-pinning is explicitly a WP04 outcome, not a present-tense claim.

### Citation corrections (audit §4, items C-1…C-23)

All `applied-plan` except C-12; provenance fixes, not behavior changes.
C-1 §2.1 `b3:`/AC-G-07 · C-2 WP11 fourth zero-orphan condition (+A-51
staging) · C-3 AC-G-14 reframed as analysis-context-manifest fields
(D-09/WP02) · C-4 WP10 feature-registry vs AC-G-03 posture · C-5 WP07
out-of-order rejection relabeled contract-authoring (A-52) · C-6 WP07
interning → Fact Gen §20.2 · C-7 I-06 → AC-G-71 Decision + rule 3 ·
C-8 WP06 fourteen-files split, WP11 `delta-local-filesystem`, WP12 macOS
config root · C-9 WP09 `Utf8View` → §65.2 · C-10 WP19 `enum_catalog` =
§8 MAY-mirror · C-11 WP23 FULL_TABLE_REPLACE escape-hatch scope note ·
C-12 — `rejected`, no change needed (v1 never attributed
`SOURCE_SNAPSHOT_MISMATCH`/`STALE_RESULT` to the fabric spec) · C-13 WP22
six-of-sixteen §75 subset made explicit · C-14 WP22 §9.22 = deltalake
reference · C-15 WP04 §79 correction (its Phase 1 has four tools; Wave-0
shell scope comes from the roadmap) · C-16 WP10 `FreshnessPolicy` → §9;
WP08 + `SEMANTIC_PHRASE_AMBIGUOUS` · C-17 LD-10/WP05 §8-SHOULD vs
AC-G-61-mandate; §3 attrs note → §6 invariants 13/15 · C-18 LD-12/LD-14
component and notify grounding · C-19 WP10 run-state registry attribution
· C-20 WP16 nine-step AC-G-33 (line-index + lease steps; retry 3 =
Appendix-B starting value) · C-21 WP09 envelope §6 / IDs §32 · C-22 D-01
WP3-only verbatim mandate; lockfile-only workspace argument · C-23 WP13
§130-vs-AC-G-27 domains (A-53); §2.2 Appendix-F precedence demoted (A-54);
WP17 §76 defaults (A-55) + §109.6 reference; A-24 grounding note;
§1.2/D-07 Wave-4 minimal reconciliation.

### Design-challenge recommendations (audit §5, Q1–Q8)

- `Q-1` — `applied-plan`: D-01 over-claims fixed (C-22); rust-analyzer
  multi-root configuration added to WP01/WP02.
- `Q-2` — `applied-plan` (hygiene: D-04 authority-copy rule,
  `.gitattributes` `linguist-generated`, generated trees excluded from
  fmt/typos/ast-grep/machete, rustfmt-stable emission — WP05 item 7) and
  `deferred` (single-committed-location collapse: per-domain mirrors are
  kept to avoid cross-domain `include!` path coupling; revisit if
  regen-check churn proves costly — closure condition recorded in D-04).
- `Q-3` — `applied-plan`: D-06 retitled to two persisted machines; the
  AC-G-28 projection's four inputs named; A-14 updated. (The machine gap is
  B-2.)
- `Q-4` — `applied-plan`: D-07/WP20 ingress pinned to the §72/§73.1
  N-stream + evidence + conflict signature; conflicting-observation fixture
  family added.
- `Q-5` — `applied-plan`: S-3 edits plus WP25 capture-point semantics
  (A-56).
- `Q-6` — `applied-plan`: D-09 three-of-four §76 wording;
  extractor/sidecar CI moved to path/pin-triggered + scheduled + milestone
  cadence (WP05 item 3; §14 rows).
- `Q-7` — `applied-plan`: the `ViewTable`/anti-join probe moved from WP25
  to WP19 preflight; WP25 consumes its outcome. (The WP08 half is B-3.)
- `Q-8` — `applied-plan`: the B-1 reorder plus the §15 parallelism note
  for WP24's lease half.

---

## 7. Work packets — Wave 0 (Program, toolchain, and build foundation)

### WP01 — Stable-domain re-baseline and seed decommission

**Outcome.** The root package `codefabric` is the stable daemon/data-plane
domain: edition 2024, `rust-version = "1.94.1"`, no PyO3/Maturin surface, no
Python packaging at the root, lints preserved (`unsafe_code = "deny"`, clippy
all+pedantic), sccache wrapper preserved. The seed code and its packaging
surface are gone. Repository docs describe the four-domain topology.

**Dependencies.** None (first packet).

**Target invariants.** I-10, I-12 (preparation), L-01–L-04. Doctrine P6, P8.

**Design and library references.** Roadmap §5 WP1; Data Fabric §2.1–2.2;
repo-spec §0.3, §9–11, §77; D-01/D-02/D-05; LD-05/LD-07 partially adopted
here (only crates actually consumed by WP01 code — normally none yet).

**Change surface.**
- *Must touch — verified:* `Cargo.toml` (drop `[lib] cdylib`, `python`
  feature, `pyo3`; set edition 2024 + `rust-version = "1.94.1"`),
  `src/lib.rs`, `src/python.rs` (delete), `tests/integration.rs` +
  `tests/integration/`, `pyproject.toml`, `python/`, `python_tests/`,
  `uv.lock`, `.python-version`, `.envrc`, `scripts/bootstrap.sh`,
  `scripts/wheel_test.sh` (delete), `CLAUDE.md`, `AGENTS.md`, `README.md`,
  **`justfile` and `.github/workflows/ci.yml`** (minimal de-Python reshape in
  this packet — every recipe/step referencing the removed surfaces is
  removed or stubbed so the tree is never red between WP01 and WP05; WP05
  performs the four-domain build-out), and **`sgconfig.yml` + `rules/`
  bootstrap** (empty-but-running `ast-grep scan` governance harness, so
  WP02–WP04 can land their boundary rules).
- *Likely touch:* `_typos.toml` (path excludes), `bacon.toml` (drop
  `check-python-feature` job), `deny.toml` (see WP05 for the duplicate-family
  policy), `.gitignore` (dist/ no longer produced), `.gitattributes`
  (generated-tree `linguist-generated` marks land with WP05 — D-04), and an
  editor multi-root seed (rust-analyzer `linkedProjects`; extended by
  WP02/WP03 as their roots land — audit Q1).
- *Discover at packet preflight:*
  `rg -n --hidden -g '!.git/**' -g '!docs/library_ref/**' 'pyo3|maturin|_native|python/codefabric'`
  to enumerate every residual reference (includes `.claude/` skills text —
  update or annotate as documentation-only).

**Required changes.**
1. Rewrite root `Cargo.toml` per D-02: single rlib crate, edition 2024,
   `rust-version = "1.94.1"` (verified — `cargo msrv verify` joins the packet
   gates and the §14 matrix, honoring repo-spec §27's "never advertise an
   unverified MSRV"; the floor is set by the pinned delta-rs revision, not by
   any language feature CodeFabric uses, so it is a build-tooling obligation
   that `cargo msrv verify` must confirm the installed stable satisfies),
   empty `[dependencies]` (pins enter with the packets
   that consume them; the full §2.1 pin table is recorded in the WP11
   toolchain bundle and mirrored in a manifest comment).
2. Delete L-01/L-02/L-03 surfaces; keep `tests/integration.rs` shell with an
   empty module so the single-integration-target topology survives.
3. Update `.envrc`/`bootstrap.sh`: no root `uv sync`; report the four domains
   (adapter sync arrives with WP04; extractor/sidecar checks with WP02/03).
4. Minimal `justfile`/CI reshape (see Must touch) — the packet gate below is
   the reshaped recipe set, and it must be green at packet close.
5. Update `CLAUDE.md`/`AGENTS.md`/`README.md` to the new topology and v1.3
   spec filenames.

**Legacy disposition.** Executes L-01–L-04 (see §6).

**Acceptance evidence.**
- *Behavioral:* `cargo check --all-targets` and `cargo clippy --all-targets
  -- -D warnings` pass on the featureless root crate; `cargo nextest run`
  passes (empty-but-compiling test target).
- *Structural:* `cargo tree` shows zero dependencies; no `[features]` table;
  `crate-type` is default rlib.
- *Negative / zero-state:* preflight `rg` sweep returns zero live-code hits
  for `pyo3|maturin|_native|python/codefabric` (declared scope: whole repo
  minus `.git`, `docs/`, `.claude/` annotations); `cargo tree -i pyo3` errors
  (not in graph).
- *Operational:* scripted assertion: `./scripts/bootstrap.sh` output and
  `just --list` contain zero matches for
  `maturin|wheel|python-develop|test-python` (grep-based test committed with
  the packet).

**Edit-local gates.** `cargo check`; `cargo fmt --check`; typos on changed
docs.
**Packet-local gates.** The reshaped root-domain gate: fmt, check, clippy,
nextest, doctest (none yet), typos, machete/shear, deny, `cargo msrv
verify`, `ast-grep scan` (bootstrap ruleset).
**Integration milestone.** M01.
**Replan triggers.** A hidden consumer of the seed surfaces (e.g., a script
importing `codefabric` Python package) that cannot be deleted — none known;
if found, plan revision.
**Rollback.** Single revert commit; baseline is green.

### WP02 — Nightly rustc-extractor build domain shell

**Outcome.** `rustc-extractor/` is a standalone Cargo root pinned to
`nightly-2026-08-18` (components `rustc-dev`, `rust-src`, `llvm-tools` —
audit L-1), building executable `codefabric-rustc-extractor` that
prints its exact toolchain identity (rustc version + commit hash — the
AC-G-14 Rust context-manifest identity fields, Fact Gen — audit C-3)
to STDERR via a `--identity` invocation, writes nothing non-protocol to
STDOUT, and terminates cleanly. Per D-09 the domain **links `rustc_public`
via `rustc-dev` in its default build** — the deep-integration baseline is
proven at Wave 0, not deferred — and the exact rustc commit hash is recorded
into the identity output and the WP11 toolchain-bundle record. The root
`rust-toolchain.toml` comment and AGENTS.md/CLAUDE.md toolchain sections are
updated to the ratified posture: nightly is the extractor domain's
production toolchain (no longer analysis-only); the root stays stable.

**Dependencies.** WP01 (repo shape).
**Target invariants.** I-08, I-10. Doctrine P6, P8, P29.
**Design and library references.** Roadmap §5 WP2; Fact Gen §2, §7.4,
AC-G-31 (rules 1, 6–7 shape the shell's I/O discipline); repo-spec §76;
LD-12.

**Change surface.**
- *Must touch:* new `rustc-extractor/{Cargo.toml,Cargo.lock,rust-toolchain.toml,src/main.rs}`;
  root `.gitignore` (its `target/`); `scripts/bootstrap.sh` (report nightly
  domain); root `rust-toolchain.toml` comment + `AGENTS.md`/`CLAUDE.md`
  toolchain-posture text (D-09 ratification); editor multi-root config
  (rust-analyzer `linkedProjects` entry + `rust-analyzer.rustc.source =
  "discover"` for this domain — audit Q1).
- *Discover at preflight:* `rustc +nightly-2026-08-18 --version --verbose`
  for the exact commit hash; mechanics of the `rustc-dev` link at this date
  (`extern crate rustc_public` module shape, required `-Z`/wrapper flags) —
  the consumption mode itself is decided (D-09/A-22), only its mechanics are
  confirmed here.

**Required changes.** Package with `publish = false`; `main.rs` shell:
parses `--identity`/`--serve <endpoint>` (serve mode is a stub that connects
nothing yet), prints identity to STDERR, exits 0. A smoke test asserts STDOUT
byte-emptiness. A **mandatory** link-smoke module (default build, no feature
gate) links `rustc_public` through `rustc-dev` and exercises one trivial
entry point, so every CI run proves the deep-integration toolchain is
whole; extraction logic itself remains deferred to Waves 5/10 per the
roadmap.

**Legacy disposition.** None.
**Acceptance evidence.**
- *Behavioral:* `cd rustc-extractor && cargo check && cargo test` on the
  pinned nightly; `--identity` prints toolchain + commit hash to STDERR;
  STDOUT is empty in all invocations.
- *Structural:* `rust-toolchain.toml` pins the dated nightly + components;
  own `Cargo.lock` committed; the `rustc_public` link smoke compiles and
  runs in the default build.
- *Negative:* root `rust-toolchain.toml` unchanged (stable); root
  `Cargo.lock` has no extractor entries; extractor is not a workspace member.
- *Operational:* clean-checkout build documented in CI (WP05 wires it).
**Edit-local gates.** `cargo +nightly-2026-08-18 check` in the domain dir.
**Packet-local gates.** Domain check/clippy/test + STDOUT-discipline test.
**Integration milestone.** M01.
**Replan triggers.** `rustc_public`/`rustc-dev` unavailable or broken on
2026-08-18 for aarch64-apple-darwin → design issue to Fact Gen §2 (pin moves);
this is a **plan-revision** trigger, not an ad-hoc pin change.
**Rollback.** Delete the directory; no other domain depends on it in Wave 0.

### WP03 — Pyrefly sidecar build domain shell

**Outcome.** `pyrefly-sidecar/` is a standalone Cargo root (stable toolchain,
own `Cargo.lock`, own `deny.toml` permitting exactly the pinned Pyrefly
source) whose executable `codefabric-pyrefly-sidecar` links Pyrefly 1.2.0,
prints identity (sidecar build + Pyrefly source digest per AC-G-30 handshake
fields) to STDERR, keeps STDOUT protocol-silent, and exposes no
Pyrefly-internal Rust type in any public item.

**Dependencies.** WP01.
**Target invariants.** I-08, I-10. Doctrine P6, P8.
**Design and library references.** Roadmap §5 WP3; Fact Gen §2, §7.3,
AC-G-30 (stdout/stderr rules), AC-G-14 (`pyrefly_bundle_digest`); LD-13;
code-facts reference: Pyrefly bundles Ruff component crates 0.0.6 — one minor
behind the 0.0.7 anchor — which is the standing justification for the
process/build isolation.

**Change surface.**
- *Must touch:* new `pyrefly-sidecar/{Cargo.toml,Cargo.lock,deny.toml,src/main.rs}`;
  `scripts/bootstrap.sh`.
- *Discover at preflight:* resolve Pyrefly 1.2.0's exact coordinate
  (crates.io `=1.2.0` vs git tag) and record it plus a BLAKE3 digest of the
  locked source (LD-13, A-21).

**Required changes.** Shell binary with `--identity`; a private module links
`pyrefly` and computes/embeds the bundle digest at build time (build script
hashing the lockfile entry), and a **mandatory link-smoke** exercises the
`pyrefly::query` facade entry points (construct/inspect only — no analysis),
so the Wave 9 deep-integration surface is proven from Wave 0 per D-09.
Public API surface: none (bin-only). An `ast-grep` governance rule (D-03)
asserts no `pub` item references a `pyrefly::` type. Updates to the Pyrefly
rev follow the D-09 managed procedure.

**Acceptance evidence.**
- *Behavioral:* domain `cargo check/test`; `--identity` on STDERR; STDOUT
  empty.
- *Structural:* independent `Cargo.lock`; root lockfile untouched.
- *Negative:* governance rule zero-hit for exposed Pyrefly types; root
  `cargo tree -i pyrefly` errors.
- *Operational:* domain deny policy passes (git source allowed only here if a
  git pin is required).
**Edit/packet gates.** Domain fmt/check/clippy/test + STDOUT test + deny.
**Integration milestone.** M01.
**Replan triggers.** Pyrefly 1.2.0 not resolvable as a pinned source, or it
fails to build on stable 1.97.1 → plan revision (sidecar toolchain pin) +
design issue to Fact Gen §2.
**Rollback.** Delete directory.

### WP04 — Python FastMCP adapter domain shell

**Outcome.** `codefabric-cpg-mcp/` exists exactly per Serving §54's layout
(Wave 0 subset): locked project with the exact pins
`fastmcp==3.4.7`, `pydantic==2.13.4`, `pydantic-settings==2.15.0` plus
lockfile-pinned `grpcio`/`protobuf`/`orjson`; `python -m codefabric_cpg_mcp`
starts a STDIO-safe FastMCP shell (settings module with the §55 immutable
`SettingsConfigDict`, `mcp.run()` entrypoint) and terminates cleanly with
**zero non-protocol STDOUT bytes**; a pytest asserts the launch discipline
using the locked command `uv run --frozen --project <abs> python -m
codefabric_cpg_mcp`; `python -m codefabric_cpg_mcp --identity` prints the
adapter, FastMCP, Pydantic, pydantic-settings, and Python versions to STDERR
and exits 0 (the domain's version-identity surface for the Wave 0 exit).

**Dependencies.** WP01.
**Target invariants.** I-07, I-08, I-10. Doctrine P6, P29.
**Design and library references.** Roadmap §5 WP4; Serving §18–20, §54, §55,
§60.2, §68.6, §79 (context only — its Phase 1 begins at the serving
waves; audit C-15), §0.6; LD-11.

**Change surface.**
- *Must touch:* new `codefabric-cpg-mcp/` tree (`pyproject.toml`, `uv.lock`,
  `README.md`, `src/codefabric_cpg_mcp/{__init__,__main__,server,settings}.py`,
  `tests/` skeleton); root `.envrc` (sync this project); dev group:
  `pytest`, `ruff`, `pyrefly` with configs scoped to this project. **No
  adapter-local `.proto`**: `contracts/rpc/` is the single generating source
  (AC-G-01/05); the Serving §54 tree's `proto/` entry is superseded by the
  manifest layout (register item A-42), and the adapter consumes generated
  stubs only.
- *Decision to confirm with the user:* whether the baseline's root
  `attrs`/`cattrs` dependencies move into this project as internal libraries
  (see §3 baseline note); they are not part of the Serving §18 pin block and
  never carry public wire contracts.
- *Discover at preflight:* grpcio/protobuf/orjson versions compatible with
  Python 3.14.7 (the compatibility test Serving §18 mandates before
  lock-pinning).

**Required changes.** Settings per §55 verbatim (env aliases, `OpaqueId`,
bounded numerics with the spec's defaults/ranges, locked source order without
dotenv); `server.py` constructs `FastMCP` with `instructions` and a lifespan
that only loads settings in Wave 0 (daemon handshake arrives Waves 15+);
`__main__.py` is `mcp.run()`. Tools/resources/prompts are **not** registered
yet — Wave-0 shell scope per roadmap §5 W0.4. (Serving §79's own Phase 1
already includes four public tools, so §79 phasing begins with the serving
waves, not here — audit C-15.) All logging to STDERR.

**Acceptance evidence.**
- *Behavioral:* STDIO test — spawn the locked command with required env vars,
  assert startup, clean shutdown, and zero stray STDOUT (Serving §68.6);
  `uv run --frozen` succeeds from clean checkout.
- *Structural:* `uv.lock` carries the three exact pins; no `pydantic-core`
  pin; `requires-python >=3.12`; interpreter 3.14.7 recorded.
- *Negative:* no Arrow/DataFusion/Maturin/PyO3 dependency in the adapter
  graph (`uv tree` scoped check); no dotenv source in settings.
- *Operational:* ruff + pyrefly pass on the new project.
**Edit/packet gates.** ruff format/check, pyrefly, pytest for this project.
**Integration milestone.** M01.
**Replan triggers.** fastmcp 3.4.7 or pydantic 2.13.4 incompatible with
Python 3.14.7 → the interpreter pin drops to the newest compatible 3.x
(recorded deviation; the spec floor is 3.12) — implementation adaptation, not
design change.
**Rollback.** Delete directory; root `.envrc` revert.

### WP05 — Protobuf toolchain, repository command contract, and four-domain CI

**Outcome.** Protobuf generation is installed and verified for Rust
(prost/tonic-build) and Python (grpcio-tools) with canonical output locations
(D-04); the `justfile` is reshaped into per-domain recipe groups plus
cross-domain gates; `.github/workflows/ci.yml` builds all four domains from a
clean checkout, runs the duplicate-family policy, and executes the
STDOUT-discipline smoke tests. Deterministic test roots and fixture
conventions are established (`contracts/fixtures/` as the shared
cross-language corpus root; per-domain `tests/`).

**Dependencies.** WP01–WP04.
**Target invariants.** I-08, I-10, I-12. Doctrine P25, P29, P31.
**Design and library references.** Roadmap §5 WP5–6; Data Fabric §2.2 (CI
duplicate rejection); Serving §70 (fingerprint comparison posture), §77
(upgrade-gate posture); repo-spec §14, §49–52; LD-10; D-03/D-04.

**Change surface.**
- *Must touch:* `justfile`, `.github/workflows/ci.yml` (four-domain
  build-out over WP01's minimal reshape), `deny.toml` (`[bans]`
  `multiple-versions = "deny"` scoped via `skip`/`skip-tree` so the
  arrow/parquet/datafusion/object_store families are hard-denied duplicates),
  new `tooling/proto/` build wiring, `rules/` additions (the D-03 boundary
  rules incl. the adapter-side FastMCP/Pydantic-internals rule; harness
  bootstrapped in WP01), `contracts/fixtures/` skeleton.
- *Discover at preflight:* tonic/prost version pair passing the LD-10 UDS +
  interop probe; `protoc` availability vs vendored compilation.

**Required changes.**
1. A `proto-gen` recipe generating Rust and Python stubs from `proto/`
   sources into committed locations, plus a `proto-check` recipe that
   regenerates and diffs (byte-identical gate, I-08).
2. Recipe groups: `root-*` (fmt/check/clippy/test/doctest), `extractor-*`,
   `sidecar-*`, `adapter-*` (ruff/pyrefly/pytest), `contracts-*`
   (regen/verify — lands fully in WP06), `governance` (`ast-grep scan`),
   aggregate `ci-fast` and `ci-pr` preserving the two justfile rules
   (mutating recipes never gate dependencies; smallest-tool-set discipline).
3. CI jobs per domain with pinned actions; `uv sync --frozen` for the
   adapter; nightly toolchain install for the extractor job; resolved-feature
   assertion for gix (`cargo tree -e features -i gix` — active from WP17, wired
   now); duplicate-family check active immediately. Extractor and sidecar
   jobs are **path/pin-triggered** (their directories, toolchain/pin files,
   shared `contracts/`) plus a scheduled nightly run and mandatory execution
   at every milestone gate — not every-PR (repo-spec §49 Tier B/C placement;
   audit Q6). Root, adapter, contracts, and governance gates run every PR.
4. Duplicate-family enforcement as a **committed negative fixture**: a
   `tooling/ci/duplicate-family-fixture/` manifest carrying a second arrow
   version, against which `cargo deny check` must fail — run permanently as
   a `governance` step (expected-failure assertion). The deny config itself
   is additionally covered by a config-shape unit test. (Full-graph
   enforcement becomes live when WP19 adopts the real Arrow graph.)
5. Deterministic test/temp roots: all tests use per-test temp dirs (std
   tempdir or `target/tmp/<test>`); no test touches a shared mutable state
   root; daemon-state fixtures always point at packet-local temp roots.
6. Cache-authority rule (roadmap §5 W0.5): caches (sccache, uv cache) are
   never correctness authority — regeneration byte-identity gates compare
   digests of outputs, not build logs; CI records `sccache --show-stats` as
   telemetry only.
7. Generated-tree hygiene (D-04, audit Q2): `.gitattributes`
   `linguist-generated` for `contracts/generated/` and every per-domain
   generated dir; those paths excluded from `cargo fmt --check`, typos,
   `ast-grep scan`, and machete surfaces; generator output asserted
   rustfmt-stable (format-then-diff test).

**Acceptance evidence.**
- *Behavioral:* CI green on a clean checkout for all four domains; proto
  round-trip smoke test (a trivial placeholder message) passes in Rust and
  Python.
- *Structural:* `just --list` shows the domain groups; deny config carries
  the family bans.
- *Negative:* the committed duplicate-family fixture fails `cargo deny
  check` on every CI run (expected-failure test); a stub/byte drift in
  committed generated placeholders fails `proto-check`.
- *Operational:* `just doctor` covers all four domains.
**Edit/packet gates.** Recipe-by-recipe smoke runs.
**Packet-local gates.** Full `just ci-fast` (new shape) + CI dry run.
**Integration milestone.** M01 (closes Wave 0).
**Replan triggers.** LD-10 probe failure (no tonic/prost pair supports the
required transport shape) → plan revision to a different Rust gRPC stack —
this is transport mechanics, not a design change (Serving §8/AC-G-61 name
gRPC+UDS, not a crate).
**Rollback.** WP01–WP05 revert as a unit (WP05 rewrites shared files whose
pre-WP01 form references deleted surfaces; WP05 alone is not independently
revertible).

---

## 8. Work packets — Wave 1 (Machine contracts, registries, code generation)

### WP06 — `contracts/` tree, canonical JSON, and the `codefabric-contracts` verifier core

**Outcome.** The `contracts/` source tree exists with the **exact AC-G-05
layout** (`manifests/{suite-manifest.json,deployment-profile.schema.json,
requirements.jsonl,traceability.jsonl}`, `registry/` — the fourteen
named files (thirteen `*-registry.yaml` plus `model-pack.schema.json` —
audit C-8), `identity/{cbef-v1.yaml,type-algebra-v1.yaml,
path-canonicalization-v1.yaml}`, `schema/` incl. `arrow-delta/`,
`query/{english-controlled-v1.ebnf,planspec.schema.json}`, `rpc/` — the four
named `.proto` files + `feature-registry.yaml`, `adapter/`, `bundles/` — the
eight named bundle manifests, `deployment/local-workstation-v1.yaml`,
`faults/fault-point-registry.yaml`, `comparison/comparison-ignore-registry.yaml`,
`security/security-corpus-manifest.yaml`); registry YAML is the
human-reviewable source and derived canonical JSON is the fingerprinted
machine form; generated Rust/Python types are emitted under
`contracts/generated/` (and per-domain generated dirs) with headers naming the
source-artifact digest — hand edits prohibited. A `codefabric-contracts`
binary (bin target in the root package, repo-spec §3) implements canonical
JSON (`codefabric-jcs-v1`, all ten AC-G-53 rules), BLAKE3-256 checksumming
(`b3:<64 hex>`), artifact canonicalization/fingerprinting per AC-G-02
(prose-document digests carried in `suite-manifest.json`, never
self-referential), and `codefabric-contracts verify` (Gate A's named command)
exposed as `just contracts-verify`. Regeneration is byte-identical and CI-gated. The
canonical-JSON fixture corpus (Unicode, escaping, number boundaries, empty
values, insertion-order permutations) lives in `contracts/fixtures/jcs/` and
passes in **Rust and Python** (adapter-side encoder for fingerprints).

**Dependencies.** WP05.
**Target invariants.** I-08, I-11. Doctrine P10, P25, P29.
**Design and library references.** Roadmap §6 WP2; Query AC-G-53 (ten rules,
checksum form, cross-language corpus obligation); spec §0.5 (generated
artifact inventory); D-04.

**Change surface.**
- *Must touch:* new `contracts/` skeleton; root `src/` gains the contracts
  library modules + `src/bin/codefabric-contracts.rs`; adapter
  `contracts/json.py` canonical encoder; `contracts/fixtures/jcs/`.
- *Preflight:* none beyond LD adoption (serde_json value model vs JCS number
  rules — confirm `serde_json` preserves the required integer range checks;
  arbitrary-precision feature decision).

**Required changes.** Implement JCS-v1 exactly: RFC 8785 member ordering and
escaping, duplicate-key rejection, finite numbers only, interoperable-integer
range with `codefabric-int64`/`codefabric-uint64` string formats, unpadded
base64url `codefabric-bytes`, lowercase IDs/digests, no Unicode normalization
at serialization, sorted key/value-record encoding for non-string-keyed maps.
Checksum wrapper emits `b3:` form. Verifier walks `contracts/generated/`,
recomputes digests, and fails on drift.

**Acceptance evidence.**
- *Behavioral:* JCS fixture corpus green in Rust (`cargo nextest`) and Python
  (adapter pytest) from the same fixture files.
- *Structural:* `just contracts-verify` exists; a **committed** negative
  fixture set under `contracts/fixtures/negative/` (perturbed artifact,
  drifted digest) is asserted non-zero by the standard suite on every run.
  The verifier supports `--profile full` (CI default) and
  `--profile released` (Gate A assertion: zero warnings).
- *Negative:* rejection tests — duplicate keys, NaN/Infinity, out-of-range
  integers, padded base64.
- *Operational:* regeneration twice from clean checkout → byte-identical.
**Edit/packet gates.** Root-domain gates + adapter pytest subset.
**Integration milestone.** M02.
**Replan triggers.** JCS rule conflicts with a generated-schema need (e.g., a
field requiring full 64-bit JSON numbers) → design issue to Query AC-G-53.
**Rollback.** Contracts tree is additive.

### WP07 — CBEF-v1 identity, path canonicalization, and known-answer vectors

**Outcome.** CBEF-v1 is implemented per AC-G-13: exact record header
(`CFID`, version `0x01`, big-endian domain/field framing), all 13 type codes,
ascending-field-tag emission (AC-G-13 fixes emission order only; decoder
**rejection** of out-of-order records is contract-authored hardening,
A-52 — audit C-5),
per-field normalization rules, BLAKE3-256-truncate-16 derivation with full
32-byte digests retained in collision diagnostics and `ID_COLLISION` blocking;
the 16 required domain recipes (`WORKSPACE` … `UNKNOWN_REMAINDER`) have
authored field-tag/type-code/normalization schemas in
`contracts/identity/cbef-v1.yaml` (contract-authoring per A-02/A-03); public
ID encode/decode with strict prefix/slug/32-hex validation and the sole
symbolic `context:source`; **`identity/type-algebra-v1.yaml`** authored here
(AC-G-15 constructor set + normalization rules + de Bruijn binders +
version pin; interning rules come from Fact Gen §20.2 — AC-G-15 contains
none, audit C-6) with a canonical type-algebra encoder and its own KAT vectors
— type IDs are CBEF-derived like all others. `WorkspacePath` per AC-G-18: canonical component encoding
(percent-escaping `/`, `%`, non-display bytes; reversible; no symlink
resolution), platform rules (Linux byte-exact; macOS volume probe + NFD/case
folding on case-insensitive volumes; WTF-8 platform code reserved), display
encoding with uppercase `%XX` and `display_is_lossy`, canonical URI
(`codefabric://workspace/<hex>/path/<base64url>`), ordering by
`(comparison_key_bytes, raw_relative_path_bytes)`. KAT vectors for every
domain recipe and path rule live in `contracts/fixtures/identity/` and pass in
Rust and Python.

**Dependencies.** WP06.
**Target invariants.** I-01 (preimage rules), I-08, I-11, I-13. Doctrine P13
(stable semantic identity — Advances), P12.
**Design and library references.** Ontology AC-G-12/13/18, §64; Lifecycle
§43 (PlatformPath/GitRepoPath — §43's richer struct forms adopted over
Appendix F's conflicting minimal forms, A-54), AC-G-09 preimages; Data
Fabric §7.1–7.3.

**Change surface.**
- *Must touch:* root `src/` identity + path modules; `contracts/identity/`
  recipe sources; `contracts/fixtures/identity/`; adapter
  `contracts/types.py` public-ID validators + KAT runner.
- *Preflight:* macOS case-sensitivity probe API (`pathconf`/`getattrlist`
  approach) — verify on the dev volume; document Linux CI behavior.

**Required changes.** As stated; plus the A-01 disposition (BLAKE3_128 ≡
BLAKE3-256[0..16], stated in the contract file), A-04 (inner
length-prefix widths for list/set/map/union elements: authored as u32
big-endian, recorded in the CBEF contract), A-05 (payload_length measured
post-normalization).

**Acceptance evidence.**
- *Behavioral:* KAT vectors green in both languages — identity domains,
  paths, **and type algebra** (roadmap §6 exit names identity, path, type,
  enum, flag, canonical-JSON vectors; enum/flag vectors land with WP08);
  property tests (round-trip public IDs; ordering total and stable;
  component encoding reversible; type interning idempotent).
- *Structural:* every domain recipe file validates against the recipe schema;
  field tags unique and ascending.
- *Negative:* decoder rejects wrong prefix/width/case, non-hex, unknown
  domain, out-of-order fields; collision injection test yields
  `ID_COLLISION` and blocks.
- *Operational:* vectors regenerated byte-identically.
**Edit/packet gates.** Root gates + adapter pytest identity subset.
**Integration milestone.** M02.
**Replan triggers.** A later suite artifact publishes canonical field-tag
assignments differing from ours → plan revision (regenerate vectors; possible
reindex note per §0.4 "exact match" rule) + design issue.
**Rollback.** Additive.

### WP08 — Ontology and categorical registries + state machines

**Outcome.** `contracts/registry/` (AC-G-05 spelling) holds the machine
registries and the generator emits canonical JSON + Rust/Python lookup
artifacts for: entity kinds, relation kinds, property kinds (AC-G-71 full
record incl. value-type algebra, cardinality, `null_semantics: prohibited`,
storage mapping), fact kinds, unknown kinds (AC-G-73 mandatory 12) + reason
classes (9) + negative-fact families (4), graph projections (13 mandatory
IDs), summary profiles (`CALLABLE_SUMMARY_BALANCED_V1`), capability codes
(AC-G-36 record shape; families reserved), **derivation registry**
(`derivation-registry.yaml` — record shape per Data Fabric §79A ownership
fields; entries populate in Waves 5+ but the registry, schema, and
append-only validation exist now), error registry (AC-G-65 numeric domains
1000–9999, full record shape, all named codes incl.
`CURRENT_POINTER_CONFLICT`, `OVERLAY_GENERATION_CONFLICT`, `ID_COLLISION`,
`STATE_TRANSITION_VIOLATION`, `SOURCE_SNAPSHOT_MISMATCH`,
`SEMANTIC_PHRASE_AMBIGUOUS`, and `SEMANTIC_PHRASE_UNRECOGNIZED` — the
latter a Query AC-G-44 code admitted via AC-G-65's include-all rule (audit
C-16)…), enum/flag registries with the §62 code tables (62.1–62.6 verbatim
numeric tables; 62.7–62.9 are **uncoded** value lists in the spec, so their
numeric codes are contract-authored — A-45, audit S-8), provider registry
(record authored per A-19), and AC-G-25 state-machine YAML for the Wave-2
machines (`WorkspaceLifecycle`, `SourceTrustState`, `EventStreamHealth`,
`GitAccelerationStatus`) **plus the Wave-3 machines
`DurablePublicationState` and `ServingActivationState`** (both in
AC-G-25's mandatory eleven-machine roster and consumed by WP22/WP24 —
audit blocker B-2), **the remaining five roster machines**
(`UpdateWaveState`, `ProviderRunState`, `OwnerCapabilityState`,
`QueryExecutionState`, `ArtifactState`) as contract-only YAML whose
runtimes arrive with their waves, **plus the AC-G-10 registry machine**
(same framework, beyond AC-G-25's mandatory roster — D-06), all with
`from/event/guard/to/actions/idempotency_key/error_on_illegal` rows and
model-checked reachability. The phrase registry, `english-controlled-v1`
grammar artifact, and `model-pack.schema.json` are split out to **WP08b**
(audit blocker B-3; supersedes the R-07 in-flight split contingency).
Enum/flag registries follow the manifest's AC-G-06 record shape and rules
verbatim: code 0 reserved-invalid and never emitted, positive signed
append-only codes in increments of ten with no gap insertion after release,
immutable names/meanings, aliases parse-only, fixed per-domain code widths,
UPPER_SNAKE names + kebab slugs; the 64-bit flag word layout (bits 0–31
language-neutral, 32–47 language-profile, 48–55 generated/lowered, 56–62
reserved, bit 63 zero). The duplicate-authority check is AC-G-01's: CI fails
if two machine artifacts declare the same concern as authoritative.
Registry invariants enforced by the verifier: per-domain code/slug
uniqueness, acyclic families, abstract kinds barred from canonical rows,
capability+storage mapping for every concrete kind, provider-native kinds
firewalled, append-only discipline.

**Completion is counted, not judged** (Gate A requires *all* registries):
all 9 §62 tables (62.1–62.6 verbatim codes; 62.7–62.9 authored codes,
A-45); 12 unknown kinds + 9 reason classes + 4 negative families; 13
projections; 37 effect + 10 resource codes (both spec floors marked
"Initial"/"at least"); every error code named anywhere in the 1.3 suite;
and the eleven AC-G-25 machines plus the AC-G-10 machine, model-checked.
The phrase-section count moved to WP08b with the split (B-3).

**Dependencies.** WP06 (verifier), WP07 (slugs/ID conventions).
**Target invariants.** I-06, I-11, I-13, I-15. Doctrine P10, P12, P29, P31.
**Design and library references.** Ontology AC-G-70–73, §62, §67, §68 (L0–L14
layer axis; A-08 disposition: `family_code` carries the layer), §5–§58
heading families; Query AC-G-44; Serving AC-G-65; Lifecycle AC-G-25;
Fact Gen §85, AC-G-36.

**Change surface.**
- *Must touch:* `contracts/registry/**` sources; generator modules;
  generated Rust (`src/generated/registries.rs` or equivalent) and Python
  (adapter `contracts/` generated module); `contracts/fixtures/registries/`.
- *Preflight:* none external (the phrase-harvest checklist moved to WP08b).

**Acceptance evidence.**
- *Behavioral:* generated Rust and Python compile and expose code↔name
  lookups; **enum and flag KAT vectors pass in both languages** (fixed
  code→name→slug triples per §62 table, flag-word round-trips); distinct
  Rust/Python types per registry domain (A-07: certainty vs resolution never
  share an enum type).
- *Structural:* verifier enforces the eight AC-G-70 invariants + §62.10
  append-only rule; duplicate-authority check (same slug in two registries →
  error).
- *Negative:* evaluative deny-list test — registry sources containing
  `SAFE_TO_REFACTOR`-class kinds are rejected (I-15).
- *Operational:* byte-identical regeneration; state-machine artifacts pass
  reachability check.
**Gates.** Root + adapter gates; `just contracts-verify`.
**Integration milestone.** M02.
**Replan triggers.** Registry record shapes prove insufficient for a Wave 2/3
consumer (e.g., missing overlay policy field — already handled by A-11) →
plan revision limited to registry schema extension (append-only).
**Rollback.** Additive.

### WP08b — Phrase registry, controlled-language grammar, and model-pack schema

*(Added by the v2 audit integration — blocker B-3. Realizes the split R-07
pre-designed, so the §50–§94 phrase harvest leaves the Wave-1 critical
path; runs parallel to WP09/WP10.)*

**Outcome.** `contracts/registry/phrase-registry.yaml` and the
`english-controlled-v1` grammar artifact
(`contracts/query/english-controlled-v1.ebnf`; AC-G-44 EBNF + registry
record schema + `SEMANTIC_PHRASE_UNRECOGNIZED`/`SEMANTIC_PHRASE_AMBIGUOUS`
error behavior), with a phrase-registry entry set covering **every** Query
§50–§94 catalog section — the verifier counts sections and fails on any
gap. The range is §50–§94, **not** §50–§102: §95–§102 are Part VII worked
examples that define no phrases (audit correction to v1's count rule).
Also `contracts/registry/model-pack.schema.json` (AC-G-38 format schema; no
packs ship in the 1.x baseline). Phrase entries carry explicit
deferred-mapping records until the Wave-15 compiler provides executable
mappings, keeping AC-G-04's fourth zero-orphan condition green (A-51).

**Dependencies.** WP08 (registry framework, verifier invariants, slug/ID
conventions).
**Target invariants.** I-11, I-13, I-15. Doctrine P10, P29.
**Design and library references.** Query AC-G-44, §50–§94 (catalog range
verified; §95–§102 excluded); Serving AC-G-65 (error codes); manifest
AC-G-05 (artifact locations).

**Change surface.**
- *Must touch:* `contracts/registry/phrase-registry.yaml`,
  `contracts/query/english-controlled-v1.ebnf`,
  `contracts/registry/model-pack.schema.json`; generator + verifier
  section-count rule; `contracts/fixtures/registries/` phrase fixtures.
- *Preflight:* enumerate the Query §50–§94 section list (spec-outline) as
  the harvest checklist the verifier counts against.

**Acceptance evidence.**
- *Behavioral:* generated phrase lookups compile in Rust and Python; the
  EBNF artifact parses (grammar-lint step); the model-pack schema validates
  its committed negative fixture.
- *Structural:* the verifier's section count equals the §50–§94 list
  exactly; every phrase entry names its owning section and carries a
  deferred-mapping record (A-51).
- *Negative:* an entry citing a §95–§102 example section fails the count
  rule; evaluative phrases are rejected (I-15).
- *Operational:* byte-identical regeneration.
**Gates.** Root + adapter gates; `just contracts-verify`.
**Integration milestone.** M02.
**Replan triggers.** The §50–§94 harvest proves materially larger than one
packet → further split by language family (neutral/Python/Rust) — plan
revision, sequence unchanged.
**Rollback.** Additive.

### WP09 — Schema generation: Arrow/Delta TableSpecs, snapshot/state schemas, JSON Schemas, adapter contracts

**Outcome.** The generator emits, from registry + identity sources:
(a) the `TableSpec` set for every Wave-3 table — control plane (`workspace`,
`common_repository`, `analysis_context`, `analysis_context_set`,
`publication`, `publication_table`, `current_publication`, `owner`,
`capability_status`, `diagnostic`, `enum_catalog` — with `workspace`
gaining `registration_revision` + `updated_at`, which §13.1 lacks but
AC-G-10/AC-G-19 require, A-44) and universal facts
(`entity`, `relation`, `property_fact`, `fact_evidence`) — with §7 physical
types (`id16`=Binary/16, `hash32`, codes, `Utf8` not `Utf8View`), §10 schema
metadata keys, primary keys, partition columns (§95: entity by
`entity_family_code, owner_bucket`; relation by `relation_family_code,
owner_bucket`; owner-bucket count 256), **overlay mutation policy per table**
(A-11: `TableSpec` extended with `overlay_policy: OverlayMutationPolicy`,
declared for every table: facts `OWNER_REPLACE`; `current_publication`
current-singleton per AC-G-26; enum dimensions `BASE_IMMUTABLE`; operational
views `OPERATIONAL_ONLY`), plus overlay tombstone schemas (AC-G-20 verbatim
owner/primary-key tombstone Arrow schemas);
(b) operational-store (SQLite) schema DDL for §130 tables and the
`serving_snapshot_manifest`/`active_snapshot` records (AC-G-19 field set wins;
mutable `SnapshotActivationRecord` separated — A-12);
(c) `ServingSnapshotManifest` schema (AC-G-19 complete field list, CBEF body
order, `manifest_digest`/`snapshot_id` derivations);
(d) the complete `contracts/schema/` JSON Schema set with the AC-G-05
hyphenated filenames: `analysis-context.schema.json`,
`serving-snapshot.schema.json`, `public-snapshot-metadata.schema.json`
(`PublicSnapshotMetadata` defined once, consumed by response/status/artifact
surfaces), `source-context.schema.json`, `public-status.schema.json`,
`cpg-semantic-query-request.schema.json`,
`cpg-semantic-query-response.schema.json` (envelope fields per Query §6, public-ID
patterns per §32; §103–104 merely name the artifacts — audit C-21), plus `query/planspec.schema.json` (AC-G-46
node/value types; unbound + bound forms; JSON Schema 2020-12 per A-27);
(e) `contracts/adapter/` public schemas per AC-G-05:
`fastmcp-input.schema.json`, `fastmcp-output.schema.json`,
`fastmcp-public-meta.schema.json`, generated from the adapter contract
models;
(f) adapter public contracts: `StrictWireModel` family, `contracts/{types,
public,daemon,json,errors}.py`, `schema_export.py` with serialization-mode
schema generation, `$schema`+stable `$id`, orjson sorted-key export, and the
four tool-output schema snapshots; fingerprint self-check wired for CI
(Serving §19–20, §60.1, §70).

**Dependencies.** WP07, WP08.
**Target invariants.** I-05, I-06, I-08, I-11, I-12. Doctrine P10, P29.
**Design and library references.** Data Fabric §7–§16, §95, AC-G-19/20/21;
Query §32–33, §36–48 (record shapes), §103–105, AC-G-46; Serving §19, §20,
§55, §60, §70; LD-01/02 (ref-doc: metadata survival caveats — schema metadata
is contract-tested, never relied on through plan operators).

**Change surface.**
- *Must touch:* `contracts/schema/**` + `contracts/adapter/**` +
  `contracts/query/planspec.schema.json`; generator; generated Rust
  TableSpec module; adapter contracts modules + `resources/*.schema.json`;
  `contracts/fixtures/schemas/`.
- *Preflight:* Arrow schema snapshot tests harness; confirm `Binary` (not
  `LargeBinary`) across builders (LD-02 caveat).

**Acceptance evidence.**
- *Behavioral:* every released fixture validates against its schema; Arrow
  schema snapshot tests; adapter `schema_export --check` green; four tool
  schemas carry dialect + `$id`.
- *Structural:* every `TableSpec` declares exactly one overlay policy
  (unspecified policy = generator error, per §91); every property fact
  value-type maps to exactly one typed column set; all AC-G-05 `schema/` and
  `adapter/` filenames present (verifier layout check).
- *Negative:* a committed drift fixture (schema changed, version unchanged)
  under `contracts/fixtures/negative/` fails the fingerprint check on every
  run; JSON-blob/EAV shapes rejected by generator tests (§5.1 prohibitions);
  `Utf8View` rejected per §65.2 (audit C-9).
- *Operational:* byte-identical regeneration.
**Gates.** Root + adapter gates; contracts-verify.
**Integration milestone.** M02.
**Replan triggers.** A required AC-G-19 field cannot be computed in Wave 3
(`effective_content_digest` cost — A-13) → interim contract records the
computation as full-scan-at-publication with a design issue filed; if that is
rejected, plan revision.
**Rollback.** Additive.

### WP10 — Protocol generation: four Protobuf packages + negotiated features

**Outcome.** `contracts/rpc/` defines and the toolchain compiles + round-trips
in Rust and Python: (1) `codefabric.cpgd.v1` `CpgQueryService` in the AC-G-58
nine-RPC form (unary `StartQuery`, streaming `StreamQuery`/`AttachQuery`,
`QueryEvent` closed oneof with the five AC-G-58 variants, `FreshnessPolicy`
enum verbatim from Serving §9 (AC-G-58 names only a "structured freshness
policy" — audit C-16), message field sets per AC-G-58, 4 MiB/1 MiB caps, `identity|zstd`
compression fields, idempotency-key fields); (2) `codefabric.provider.v1`
provider-control package realizing AC-G-32 (job spec/accepted/events/cancel,
run-state enum from the ontology `ProviderRunState` registry (Fact Gen
§85 restates it; AC-G-32 names no registry — audit C-19), supersession
keys, credit-control
constants authored once and shared — A-18); (3) `codefabric.pyrefly.v1`
sidecar package realizing AC-G-30 (six operations, Hello/HelloAck fields,
credit flow, strictly ordered event stream, ObservationBatchChunk with Arrow
IPC payload references); (4) `codefabric.rustc.v1` extractor package realizing
AC-G-31 (env-var handshake constants, CompilationAccepted→…→CompilationEnd
events, owner records, rejection-rule error codes). File names and locations
are fixed by AC-G-05 (`contracts/rpc/{cpg_query_service,provider_control,
pyrefly_sidecar,rustc_extractor}.proto` + `feature-registry.yaml`);
package/RPC/message names for (2)–(4) are contract-authored (A-17) since the
suite defines those protocols in prose. The feature registry (an AC-G-05
artifact) backs the handshake feature bits negotiated under the AC-G-03
per-family compatibility matrix (AC-G-03 states the posture; it does not
name the artifact — audit C-4).

**Dependencies.** WP05 (toolchain), WP08 (enums/errors).
**Target invariants.** I-08, I-10, I-11. Doctrine P16, P29.
**Design and library references.** Serving §8–10, AC-G-58; Fact Gen
AC-G-30/31/32/33; roadmap §6 WP6; LD-10.

**Change surface.**
- *Must touch:* `contracts/rpc/*.proto` + `feature-registry.yaml`; generated Rust stubs (root
  `src/generated/proto/`), extractor + sidecar domains' generated stubs (each
  domain runs the same generation recipe against the shared `.proto` sources
  — D-04), adapter `daemon/generated/` pb2 modules; round-trip test suites.
- *Preflight:* prost/tonic handling of proto3 `oneof` + large-message caps;
  grpcio codegen import layout in the adapter package.

**Acceptance evidence.**
- *Behavioral:* encode→decode round-trip fixtures pass in Rust and Python for
  representative messages of all four packages; a loopback UDS echo test for
  `CpgQueryService.Handshake` between a stub Rust server and the Python
  client (transport probe only — no daemon semantics).
- *Structural:* `QueryEvent` oneof is closed with exactly the five variants;
  sequence fields u64; freshness enum matches the canonical request enum.
- *Negative:* the superseded Serving §9 seven-RPC form does not appear
  (`ExecuteQuery` absent — grep gate); unknown required feature bits fail the
  handshake fixture.
- *Operational:* `proto-check` byte-identical regeneration.
**Gates.** Root/extractor/sidecar/adapter compile gates + round-trip suites.
**Integration milestone.** M02.
**Replan triggers.** LD-10 probe failures; AC-G-30/31 prose→proto mapping
uncovers a contradiction (e.g., AC-G-31's fd-based channel vs gRPC framing —
the extractor package is length-delimited framing over fd/UDS, **not** gRPC;
if prost framing proves unsuitable, plan revision on the framing layer only).
**Rollback.** Additive.

### WP11 — Bundles, deployment profile, CF-ID traceability

**Outcome.** The generator emits the eight AC-G-07 bundle manifests
(ontology, schema, provider, derivation, query-language, tool-contract,
toolchain — carrying LD-12/13/14 pins and domain identities — and model-pack)
with the exact AC-G-07 record shape and digest rule (BLAKE3-256 over RFC-8785
canonical JSON omitting `bundle_digest`/`signature`; artifacts sorted by
`artifact_id`; built-in bundles trusted by shipped digest; Ed25519 reserved
for external model packs); `contracts/manifests/suite-manifest.json` carrying
AC-G-02 metadata for every artifact including the b3 digests of the seven
prose documents; the effective `deployment/local-workstation-v1.yaml` with the
AC-G-08 field set verbatim (sqlite-wal operational store,
`delta-local-filesystem` fact store (audit C-8), network listeners
disabled, overlay journal disabled, symlink
policy, TTLs, default freshness, platform root table); CF-IDs per AC-G-04
(`CF-<ARCH|ONT|GEN|FAB|LIFE|QUERY|SERVE|SEC|TEST>-<4 digits>`, never reused)
recorded as `manifests/requirements.jsonl` machine records (source artifact +
section + normative-text digest + implements + traces_to + verified_by) and
`manifests/traceability.jsonl` supporting the mandatory trace paths; CI
zero-orphan rules per AC-G-04 — all **four** conditions (audit C-2):
orphaned mandatory ontology kinds; schema columns without owning
requirements; query phrases with no executable mapping (satisfied via
WP08b's deferred-mapping records until the Wave-15 compiler lands, A-51);
and requirements with no test or explicit `verification_deferred` record.

The toolchain bundle covers **every pinned boundary family**: LD-01–LD-07
data-plane pins, LD-06 gix, LD-08–LD-10 at their pinned versions, LD-11
adapter pins, LD-12 extractor toolchain (identity/digest records emitted by
WP02 in Wave 0), LD-13 Pyrefly source digest (emitted by WP03), LD-14
provider pins recorded-not-adopted. `manifests/deployment-profile.schema.json`
(the schema the profile instance validates against) is authored here.

**Dependencies.** WP08, WP08b, WP09, WP10.
**Target invariants.** I-08, I-11. Doctrine P27, P29.
**Design and library references.** Spec §0.5; roadmap §6 WP7–8; AC-G-14
digest fields (context bundle inputs).

**Change surface.**
- *Must touch:* `contracts/manifests/**`, `contracts/bundles/**`,
  `contracts/deployment/local-workstation-v1.yaml`, generator trace module,
  CI traceability step; `contracts/fixtures/negative/` (broken-trace-edge
  fixture).
- *Preflight:* none external.

**Acceptance evidence.**
- *Behavioral:* `codefabric-contracts verify --profile full` green and
  `--profile released` warning-free: registry invariants, schema digests,
  KAT vectors, proto round-trips, bundle digests, trace zero-orphan.
- *Structural:* every bundle pinned by digest; profile instance validates
  against `deployment-profile.schema.json`.
- *Negative:* the committed broken-trace-edge fixture fails verify on every
  run.
- *Operational:* Gate A evidence bundle produced (see M02); consolidated
  metric-name list carried in the suite manifest (§4.1 item 5).
**Gates.** contracts-verify + full four-domain CI.
**Integration milestone.** M02 (closes Wave 1).
**Replan triggers.** Governance manifest appears with conflicting AC-G-01–08
content → plan revision (R-01).
**Rollback.** Additive.

---

## 9. Work packets — Wave 2 (Daemon kernel, workspace registry, path security, source images)

### WP12 — Daemon lifecycle kernel: process, config, singleton lease, discovery file

**Outcome.** `codefabricd` exists as a bin target of the root package:
`codefabricd serve --config <path>` and `check-config`; TOML configuration in
the three AC-G-62 tiers (static / reloadable / workspace-admin-only); the §75
singleton lease sequence (lock → endpoint tempfile → fsync → atomic rename →
serve → retire on joined shutdown); the private `daemon.json` discovery file
with exactly the AC-G-62 field set and nothing secret; Tokio runtime with the
§113 posture (small I/O/orchestration worker pool, bounded blocking classes
scaffolded); §151 shutdown ordering skeleton (the steps that exist in Wave 2:
mark STOPPING, close ingress, await workers, close durable stores, retire
endpoint metadata, release lease); daemon liveness distinct from workspace
readiness (AC-G-28).

**Dependencies.** M02 (generated config/status/error contracts).
**Target invariants.** I-07, I-13 (status dimensions separate). Doctrine P8,
P22, P23.
**Design and library references.** Lifecycle §75, §109.1, §113, §151,
AC-G-62; roadmap §7 WP1. LD-05, LD-09.

**Change surface.**
- *Must touch:* root `src/` daemon modules + `src/bin/codefabricd.rs`;
  config schema source in `contracts/` (deployment profile consumption);
  justfile `daemon-*` recipes.
- *Preflight:* none material — state/runtime/config roots are fixed by
  AC-G-08's platform table (macOS: state root
  `~/Library/Application Support/CodeFabric`, config root
  `~/Library/Application Support/CodeFabric/config` — audit C-8 — and a
  private short-path directory under `$TMPDIR` for runtime; Linux: XDG
  roots). Verify the macOS `$TMPDIR` path stays under the UDS
  `sockaddr_un` length limit.

**Acceptance evidence.**
- *Behavioral:* second daemon start against the same state root fails the
  lease; `check-config` validates and rejects tier violations; clean shutdown
  leaves no lease/tempfile residue; `daemon.json` appears/retires atomically.
- *Structural:* config fields map 1:1 to the generated profile schema.
- *Negative:* `daemon.json` contains no token/root-path/secret fields (test
  asserts the exact field set); no network listener sockets opened; the
  daemon refuses group/world-writable state, runtime, or config roots and
  creates them `0700` with private files `0600` (AC-G-08).
- *Operational:* startup/shutdown traced via `tracing` with the §151 step
  names.
**Edit/packet gates.** Root gates + daemon integration tests (nextest).
**Integration milestone.** M03.
**Replan triggers.** None specific.
**Rollback.** Bin target is additive.

### WP13 — Operational-state store (SQLite WAL)

**Outcome.** One SQLite database per daemon state root, opened with the
AC-G-27 pragma set verbatim (`journal_mode=WAL`, `synchronous=FULL`,
`foreign_keys=ON`, `trusted_schema=OFF`, `secure_delete=FAST`,
`busy_timeout=5000`, `wal_autocheckpoint=1000`); numbered forward-only
transactional migrations preceded by an online backup, with
refuse-to-open-newer-schema; the Wave-2 table set: §130's named tables
(`worktree_state` keyed by `workspace_id`, `git_state_vector`,
`git_operation_run`) plus the AC-G-27 persisted domains §130 leaves
without schemas — workspace registration, credentials metadata,
generation counters (A-53, audit C-23) — and nested-root exclusion
records (A-15), all generated from WP09 DDL;
the coordinator-sole-writer discipline (writer connection owned by the
coordinator actor; separate read connections for status).

**Dependencies.** WP12; WP09 (schemas).
**Target invariants.** I-08, I-13, I-14. Doctrine P19 (durable vs temporal
truth), P22, P24.
**Design and library references.** Lifecycle §130–131, AC-G-27; roadmap §7
WP2. LD-08 (adoption + probe).

**Change surface.**
- *Must touch:* root `src/` store module; migration files; generated DDL.
- *Preflight (LD-08 probe):* rusqlite version selection; WAL +
  `BEGIN IMMEDIATE` behavior with one writer + N readers under load;
  `wal_autocheckpoint` interaction with long-lived read snapshots.

**Acceptance evidence.**
- *Behavioral:* migration up from empty; reopen after crash mid-transaction
  recovers; newer-schema refusal test; retention cleanup preserves the §131
  protected classes.
- *Structural:* pragma assertions read back at open; table shapes match
  generated DDL digests.
- *Negative:* a second writer connection attempt is rejected by the store
  API (structural discipline, not SQLite enforcement — asserted in code);
  high-volume payload classes (source bytes, Arrow rows) have no tables.
- *Operational:* backup file produced before each migration; store fault
  points (crash mid-transaction, crash mid-migration) registered per §4.1.
**Gates.** Root gates + store nextest suite.
**Integration milestone.** M03.
**Replan triggers.** WAL/latency probe shows the sole-writer model cannot
meet the §131 atomic wave+pointer transaction under contention → plan
revision of connection topology (still SQLite; AC-G-27 excludes
alternatives).
**Rollback.** Code revert **plus** state-root disposal or restore from the
pre-migration backup — forward-only migrations mean a reverted binary
refuses a newer schema (applies to every packet downstream of WP13:
WP14–WP18, WP19–WP25 all treat the daemon state root as disposable-per-test
and restorable-from-backup in development).

### WP14 — Workspace registry, administrative lifecycle, and identity

**Outcome.** The AC-G-10 admin surface (`codefabric workspace
add|list|show|relink|configure|enable|disable|reconcile|remove
[--retain-data|--purge-data]`) implemented as a local admin CLI speaking a
private admin IPC to the daemon (same-OS-user only, distinct from the future
query RPC); the AC-G-10 registry state machine and the §18 lifecycle machine
generated from WP08 state-machine YAML with `STATE_TRANSITION_VIOLATION`
enforcement (D-06); AC-G-09 identity: 128-bit registration nonces,
`workspace_id`/`repository_id`/`worktree_id` CBEF preimages, worktree
administrative keys with duplicate-active rejection, `registration_revision`
monotonicity, the identity outcome table honored (move/relink preserves,
copy/re-register mints); authorization fingerprints computed over the
AC-G-11 root-authorization record via CBEF (A-16 disposition); nested-root
registration writes the mandatory parent subtree exclusion.

**Dependencies.** WP13 (persistence), WP07 (preimages).
**Target invariants.** I-01, I-13. Doctrine P8, P12, P20 (all mutation via
the admin command path), P21.
**Design and library references.** Lifecycle §18, AC-G-09/10; roadmap §7
WP3.

**Change surface.**
- *Must touch:* root `src/` registry + admin modules; `src/bin/codefabric.rs`
  (admin CLI); store tables from WP13.
- *Preflight:* admin IPC mechanics (UDS with peer-uid check) — reuse the
  LD-10 transport probe results.

**Acceptance evidence.**
- *Behavioral:* full state-machine walk
  REGISTERING→DISABLED→OPENING→BOOTSTRAPPING→READY→DISABLING→DISABLED→
  REMOVING→REMOVED with persistence across restart; relink with Git identity
  proof; `--purge-data` double-confirmation + active-lease refusal.
- *Structural:* IDs equal the CBEF KAT-derivations for fixed nonces; two
  linked worktrees of one repo yield distinct `workspace_id`s sharing
  `repository_id`; non-Git root has null Git identities (no synthetic
  repository).
- *Negative:* re-registering a removed workspace mints a new ID; duplicate
  active administrative keys rejected; illegal transitions raise
  `STATE_TRANSITION_VIOLATION`; the query surface exposes no admin verbs
  (structural: admin service bound to a separate socket).
- *Operational:* every admin mutation writes an audit row.
**Gates.** Root gates + registry nextest suite.
**Integration milestone.** M03.
**Replan triggers.** None specific.
**Rollback.** Additive.

### WP15 — Root authorization, secure open, and path identity runtime

**Outcome.** The AC-G-11 discipline is implemented and fixture-proven:
root-authorization record (all eight fields), the 8 mandatory checks on every
workspace-relative path, component-wise secure opening (Linux `openat2` with
`RESOLVE_BENEATH|NO_MAGICLINKS|NO_SYMLINKS` + `openat`/`O_NOFOLLOW` fallback;
macOS directory-relative opens + `fstat` no-follow walk), directory symlinks
never followed, differing-device mount denial, root-identity revalidation
(authorization change → `VERIFYING` trust), `WorkspacePath`/`PlatformPath`/
`GitRepoPath` runtime types wired to WP07 canonicalization, comparison-key
collision → `BLOCKED_PATH_COLLISION`, AC-G-12 `file_id` derivation.

**Dependencies.** WP07 (path contracts), WP13 (records).
**Target invariants.** I-01, I-03 (path leg). Doctrine P8, P11 (parse at the
boundary), P12.
**Design and library references.** Lifecycle §43–§45, AC-G-11; Ontology
AC-G-12/18; roadmap §7 WP4–5.

**Change surface.**
- *Must touch:* root `src/` path/secure-open modules; adversarial fixture
  corpus under `tests/integration/` + fixture trees generated at test time.
- *Preflight:* `openat2` availability probing (CI Linux kernel) with fallback
  path both tested; macOS volume case-sensitivity probe from WP07 reused.

**Acceptance evidence.**
- *Behavioral:* authorized files open and read byte-exact through the secure
  path; case-only rename on case-insensitive volume preserves comparison-key
  identity.
- *Structural:* every filesystem read in the daemon routes through the
  secure-open module (governance rule: no raw `std::fs::read` outside it —
  `ast-grep` rule, D-03).
- *Negative (adversarial corpus, Linux + macOS):* escaped symlink, mid-path
  symlink swap, `..` and absolute injections, NUL bytes, device/drive
  prefixes, nested-mount escape, root-identity swap, comparison-key collision
  → all rejected with the registered error codes; display strings never
  accepted as identity (type-level: display fields are non-constructible into
  path identity).
- *Operational:* rejections emit diagnostics with stable codes.
**Gates.** Root gates + adversarial suite on macOS (local) and Linux (CI).
**Integration milestone.** M03.
**Replan triggers.** macOS lacks a no-follow-equivalent for a required check
→ design issue to Lifecycle AC-G-11 with interim strictest-available
behavior.
**Rollback.** Additive.

### WP16 — Source images, blob store, inventory, and generations

**Outcome.** Source-image capture per Lifecycle §33 (7-step capture fence)
+ AC-G-33's **nine-step** stable-read algorithm (Fact Gen owns AC-G-33;
steps 8–9 add the line-index artifact and a source-snapshot lease record,
persisted via the WP13 store — audit C-20) with metadata fencing and
retry/defer (retry count 3 is an Appendix-B starting value,
benchmark-adjustable), BLAKE3-256 digests, content-addressed immutable blob store (temp-write,
fsync, mode 0400, atomic rename; blob names are content hashes), size caps
(16 MiB ordinary / 64 MiB explicit), `u64` line-index artifact + digest,
encoding classification (UTF-8 requirement recorded for Rust; BOM/PEP-263 for
Python; undecodable → explicit unsupported-encoding capability entry),
`SourceImage`/`SourceSnapshot` DTOs per generated schemas; persisted
`source_generation` rules (increments per accepted coherent wave; restart
never resets; rebuilt state uses a new generation — AC-G-28); the bounded
generic inventory walker (all six bound dimensions configurable, values
recorded as plan defaults A-20), §46 inclusion classification enum, Merkle
inventory digest (mandatory here despite §34.3 SHOULD — it feeds
`GitStateVector.worktree_inventory_digest`, A-23); rename/identity policy
§35/§45 evidence hierarchy (operational continuity only, never canonical
identity).

**Dependencies.** WP13, WP15.
**Target invariants.** I-03, I-14. Doctrine P11, P13, P24, P25.
**Design and library references.** Lifecycle §33–§36, §45–§47.1, AC-G-28,
AC-G-33; Fact Gen §8–§9; roadmap §7 WP6–7.

**Change surface.**
- *Must touch:* root `src/` source-image + inventory modules; blob-store root
  under the daemon state directory; store tables;
  `contracts/deployment/local-workstation-v1.yaml` overrides section (walker
  bound defaults per A-20 — a contract edit, so regeneration +
  `just contracts-verify` join this packet's gates per §4.1 item 7);
  `contracts/security/security-corpus-manifest.yaml` (register the
  capture-race harness).
- *Preflight:* concurrent-mutation harness design (a writer process
  rewriting files during capture) — must exist before acceptance, with a
  numeric criterion: ≥10,000 capture attempts against an active rewriter at
  three file sizes (1 KiB, 1 MiB, 15 MiB), zero falsely-stable images
  (digest mismatch between published image and any full quiescent re-read),
  RNG seed recorded for replay.

**Acceptance evidence.**
- *Behavioral:* byte-exact capture round-trip; retry/defer on mutation; line
  index correct on LF/CRLF/mixed/empty/no-trailing-newline files; walker
  respects every bound + cancellation.
- *Structural:* blob paths are digests; blobs immutable (mode assertions);
  inventory rows carry all §34 fields.
- *Negative:* **concurrent mutation during capture never yields a falsely
  stable image** (fuzz-style harness, the Wave 2 exit's hardest clause);
  oversized files yield explicit capability entries, not silent skips; `.git`
  never inventoried as source.
- *Operational:* capture/walk metrics (files, bytes, retries, duration).
**Gates.** Root gates + source suite incl. the concurrency harness.
**Integration milestone.** M03.
**Replan triggers.** Stable-read fencing insufficient on APFS (mtime
granularity) → strengthen with content re-hash comparison; record deviation.
**Rollback.** Additive.

### WP17 — Read-only gix discovery and Git topology

**Outcome.** A `git_state` module (boundary per D-03: the only module that
sees `gix` types) providing `GitStateAdapter` per Lifecycle §156 Wave-2
subset (`open_worktree`, `capture_state`, `inventory`), returning detached
DTOs (`GitRepositoryIdentity`, `GitWorktreeIdentity`, `GitStateVector`,
`GitInventoryResult`); exact-path open with the trust policy (hooks,
filters, credentials, network, repository mutation, and checkout disabled
per §76's recommended defaults; allowed-config-sources and env-override
acceptance are §76 policy dimensions with **no spec default** — strictest
values adopted as contract-authoring, A-55, audit C-23); repository kind/bare detection, work/git/common
dirs, worktree enumeration with administrative names, HEAD kind/target/tree,
operation state, object format; Git-native inventory classification feeding
WP16's inventory (tracked/untracked/ignored/conflicted; ignore rules are
inclusion policy, never authorization); bounded blocking execution class
(Tokio coordinator → semaphore → blocking gix job → DTO) with interruption;
`GitAccelerationStatus` handling with generic-walker fallback (§80: gix
failure degrades acceleration, never correctness).

**Wave-boundary note (roadmap §7 deferred list).** This packet stops at
discovery, topology, state capture, and inventory classification. Explicitly
**out**: status/tree-diff candidate deltas (`status_candidates`,
`tree_diff_candidates` remain unimplemented trait stubs), rename-candidate
detection, warm-start pruning, blob-OID caches, and the cache hierarchy —
all Wave 7. `GitStateVector` capture and Git-native inventory classification
are Wave-2 obligations, not acceleration: cold start captures G0/G1
(Lifecycle §5.1 steps 6–7), the rescan fence needs the vector (§36), warm
start verifies HEAD/index/inclusion fingerprints (§5.2, AC-G-28), and §34.1
requires gix pathspec/exclude/attribute/dirwalk semantics for inventory.

**Dependencies.** WP15 (paths), WP16 (inventory integration), WP12 (runtime).
**Target invariants.** I-03, I-10. Doctrine P6, P8, P22.
**Design and library references.** Lifecycle §37–§44, §50, §69–§73, §76,
§78–§80, §109.6 (bounded blocking execution class — audit C-23), §156;
roadmap §7 WP7. LD-06 (all caveats).

**Change surface.**
- *Must touch:* root `src/` git_state module; governance rule "no `gix::`
  outside git_state" active in `rules/`; CI resolved-feature assertion.
- *Discover at packet preflight (all three are LD-06 probes):*
  1. **Linked-worktree exact-path open**: probe `gix::open` on a linked
     worktree work-dir; if per-worktree git-dir/HEAD/index resolution is
     wrong, route through owning-repo `worktrees()` enumeration (the
     reference's sanctioned path) — implementation adaptation.
  2. **Index fingerprint**: probe gix 0.86 rustdoc for a checksum/state
     identity; fallback: BLAKE3 over sorted `(path, oid, stage, mode)` entry
     tuples with cost measured; if cost is prohibitive on large indexes,
     stat-based fingerprint + design issue (R-04).
  3. **Write/lock freedom**: filesystem-snapshot probe — hash every file
     under `.git/` (paths + digests) before and after open + inventory +
     state capture; the trees must be identical and no `*.lock` may appear
     (portable to macOS; Linux CI may additionally strace). Wave 2 exit
     requires no locks left behind.
  4. **`revision` feature need (A-24)**: verify `head_id`/`head_commit`/
     `head_tree_id` resolve HEAD without the `revision` feature at the
     pinned feature set; if not, add `revision` with a recorded deviation.

**Acceptance evidence.**
- *Behavioral:* fixture repos (bare, main worktree, two linked worktrees,
  merge-in-progress, detached HEAD, unborn branch, submodule pointer,
  non-UTF-8 path, SHA-256 object format) yield correct identity/state DTOs;
  interruption cancels a long inventory.
- *Structural:* governance rule zero-hit outside the boundary;
  `GitStateVector` fields populated per §50 with fingerprints from WP16/WP07.
- *Negative:* repository byte-identical after all read operations (probe 3
  as a repeatable test); mutation API usage absent (`ast-grep` rule for
  `edit_reference|write_object|checkout` symbols); external command execution
  disabled (trust-policy assertion + no `command` invocation paths).
- *Operational:* gix job metrics (queue depth, duration, interruptions).
**Gates.** Root gates + git fixture suite (uses `codefabric-git-testkit`-style
fixtures as a test-support module, not a crate).
**Integration milestone.** M03.
**Replan triggers.** Probe 1 or 2 fails with no viable adaptation → plan
revision (WP17 scope) + design issue to Lifecycle §41/§50. `revision` feature
needed for HEAD resolution → add feature with recorded deviation (A-24).
**Rollback.** Additive module behind the boundary.

### WP18 — WorkspaceCoordinator actor, bootstrap-without-watchers, readiness

**Outcome.** One coordinator task per `workspace_id` (bounded command
channels, sole mutator of workspace state — §110), owning lifecycle/trust/
health/acceleration dimensions and generation counters per
`WorkspaceCoordinatorState` (Wave-2 form: `active_snapshot` is the explicit
AC-G-28 `NO_SNAPSHOT` startup state — recorded deviation A-25 until Wave 3
supplies snapshots); cold-start bootstrap without watchers (§5.1 steps 1,
3–10 with watcher registration replaced by an explicit
event-stream-unavailable status; §154 readiness barrier with
`source_trust_state = CURRENT` after an authoritative inventory (§154's
"Git acceleration is CURRENT" clause maps to `GIT_READY` — the enum has
no `CURRENT` member, A-47); queries —
i.e., admin/status probes in Wave 2 — before readiness return
`WORKSPACE_BOOTSTRAPPING`); warm restart per AC-G-28 (registration +
inventory restored; no fact-snapshot claim; `source_generation` never
resets); rescan generation fence (§36) in its no-watcher form (W0/W1
watermarks trivial, G0/G1 GitStateVector fencing real); admin diagnostics
(`workspace show`, health surface §150 subset without credential/config
leakage).

**Dependencies.** WP12–WP17.
**Target invariants.** I-01, I-03, I-13. Doctrine P17, P19, P22, P23.
**Design and library references.** Lifecycle §5.1–5.3, §26, §36, §110, §112
(Wave-2 limits: concurrent source reads, concurrent gix jobs), §154, §150,
AC-G-28; roadmap §7 WP1/WP8.

**Change surface.**
- *Must touch:* root `src/` coordinator + bootstrap + readiness modules;
  admin CLI status verbs; store rows from WP13/WP14;
  `contracts/faults/fault-point-registry.yaml` (bootstrap/readiness fault
  points per §4.1).
- *Preflight:* none beyond WP17's resolved probes.

**Acceptance evidence.**
- *Behavioral:* register→enable→bootstrap→READY for (a) non-Git root, (b)
  main worktree, (c) two linked worktrees as distinct workspaces; restart
  restores REGISTERED/READY-eligible state and re-verifies rather than
  trusting; G0≠G1 during bootstrap triggers reconcile-before-ready.
- *Structural:* exactly one mutator task per workspace (deterministic
  assertion test on the command-channel discipline: all mutations flow
  through the single coordinator receiver; no loom dependency is added);
  startup states follow the AC-G-28 vocabulary.
- *Negative:* no strict-current claim while trust ≠ CURRENT; restart never
  fabricates an active snapshot; second coordinator for the same workspace
  cannot spawn.
- *Operational:* health endpoint fields (§150 subset) exposed via admin CLI.
**Gates.** Root gates + coordinator integration suite → **Wave 2 exit
evidence assembled here** (M03).
**Integration milestone.** M03 (closes Wave 2).
**Replan triggers.** Actor/channel model cannot satisfy §131's atomic
wave+pointer transaction shape when Wave 3 arrives → plan revision at WP24.
**Rollback.** Additive.

---

## 10. Work packets — Wave 3 (Canonical data fabric, publication, overlay, snapshot kernel)

### WP19 — Schema-registry runtime, Delta namespace, control-plane tables

**Outcome.** The generated `TableSpec` set loads at daemon start with
schema-digest validation (`SCHEMA_DIGEST_MISMATCH` on drift); the §11.1
round-trip gate (Arrow Schema → Delta StructType → create → open → DataFusion
provider schema → Arrow → exact comparison) passes for every Wave-3 table;
per-workspace Delta namespace `/cpg/<workspace-id>/{control,facts,derived}/`
under the daemon storage root; table creation per §67 (comment, schema +
ontology version metadata, partitions, `columnMapping.mode = none` asserted,
CDF disabled, type widening off) followed by `ConstraintBuilder` commits for
the §102 row-local checks while tables are empty (LD-04c); control-plane
tables created and wired to the registry/coordinator flows: `workspace`,
`common_repository`, `analysis_context` (+`analysis_context_set` with
`context:source` seeded), `publication`, `publication_table`,
`current_publication`, `owner`, `capability_status`, `diagnostic`,
`enum_catalog` dimension mirror (§8's optional MAY-mirror, adopted; not a
§13 control-plane table — audit C-10).

**Dependencies.** M02 contracts; WP12/WP13 (daemon + store); WP14 (workspace
rows).
**Target invariants.** I-05, I-08, I-12. Doctrine P16, P29.
**Design and library references.** Data Fabric §6, §8, §10–§13, §67, §102,
§104 (bootstrap steps 1–4); roadmap §8 WP1–2. LD-01/02/03/04.

**Change surface.**
- *Must touch:* root `src/` fabric modules (schema-registry runtime, delta
  namespace, table creation); adoption of the LD-01/02/03/04 dependency pins
  into `Cargo.toml`; **root `deny.toml`** — `[sources]` gains an explicit
  `allow-git` entry for exactly the pinned delta-rs rev (the baseline policy
  is `unknown-git = "deny"` with an empty allow list, so the delta-rs
  adoption fails `cargo deny check` without this recorded exception);
  toolchain bundle update per §4.1 item 6. The §2.1 `deltalake` dependency
  block is transcribed in valid TOML table form
  (`[dependencies.deltalake]`) — the spec prints a multi-line inline
  table, which is invalid TOML (A-46).
- *Preflight:* compile probe for `CreateBuilder` configuration keys
  (retention/checkpoint property names — LD-04 caveat; `TableProperty`
  non-exhaustive) and `ConstraintBuilder` expression support for the §102
  checks at the pinned rev. Also, moved forward from WP25 (audit Q7): the
  §91 composition probe — programmatic `ViewTable`/logical-plan view
  construction and the anti-join effective-rows plan over a two-table
  fixture (LD-01; the reference documents only SQL `CREATE VIEW`). WP25's
  custom-`ExecutionPlan` replan trigger fires **here**, before the Wave-3
  chain commits, if both the programmatic view and the thin-provider
  fallback fail.

**Acceptance evidence.**
- *Behavioral:* bootstrap creates the full namespace on a fresh workspace;
  reopen validates digests; round-trip gate green per table; enable →
  relink → re-upsert: `cpg_control.workspace.registration_revision` equals
  the operational registry's current revision (A-44/D-08).
- *Structural:* every table's Delta metadata carries the §10 keys; partition
  specs match §95; constraints present (verified via table metadata).
- *Negative:* opening a table whose schema digest differs fails closed;
  column-mapping ≠ none fails an invariant check at open.
- *Operational:* creation is idempotent (re-run bootstrap = no-op commits).
**Gates.** Root gates + fabric suite (tempdir-based Delta roots).
**Integration milestone.** M04.
**Replan triggers.** A §102 constraint expression unsupported at the pinned
rev → constraint moves to Arrow-validation-only with recorded deviation
(§102 already permits this split).
**Rollback.** Fabric modules additive; tempdir state disposable.

### WP20 — Universal fact core, observation boundary, encoders, batch validation

**Outcome.** `entity`, `relation`, `property_fact`, `fact_evidence` encoders
(typed Arrow builders with capacity hints; §64 starting batch sizes; §65
builder policy — no serde row path in the hot loop); the §66 eleven-check
batch validator (schema exact match, column/row counts, non-null keys, 16-byte
ID enforcement, bucket derivation from digest byte 0, span bounds +
`start<=end`, registered enum codes, owner present, in-batch PK uniqueness —
composed from sort/adjacent-compare kernels + custom vectorized checks per
LD-02/LD-01 grounding); the §63 observation boundary: manifest-precedes-
batches streams, per-stream schema fingerprints, workspace/context/generation
fences, bounded channels with backpressure, terminal
completed/partial/failed manifests, stale-generation rejection
(`SOURCE_SNAPSHOT_MISMATCH`/`STALE_RESULT` codes); the bounded
`SyntheticCanonicalIngest` (D-07) as the only canonicalization ingress —
implementing the §72/§73.1 reconciliation signature (N observation streams
+ provider-precedence input → canonical batches + `fact_evidence` rows +
conflict records; D-07, audit Q4) — consuming synthetic observation
fixtures from `contracts/fixtures/synthetic/` (authored in this packet,
including a conflicting-observation family: two observations of the same
fact range, exercising the evidence and conflict-record legs). Scope note: this is the **fabric-side** ingest
boundary only (Data Fabric §63 — owned by the Wave 3 spec); the
provider-side job runtime (AC-G-32 executors, §90 traits beyond generated
types) is Wave 4.

**Dependencies.** WP19.
**Target invariants.** I-04, I-05, I-06 (null ≠ unknown enforced by
validator), I-14. Doctrine P11, P16, P17.
**Design and library references.** Data Fabric §9, §14–§16, §63–§66;
Fact Gen §10–§12, §85–§86; roadmap §8 WP3 + Wave 4 entry dependency
("observation ingestion"). LD-02 caveats (no engine-level PK enforcement —
custom validators are the mechanism).

**Change surface.**
- *Must touch:* root `src/` fabric encoder/validator/ingest modules;
  `contracts/fixtures/synthetic/` (fixture corpus authored here against the
  WP09 observation schemas); fabric test suites.
- *Preflight:* validator kernel composition benchmark at the §64 batch sizes
  (sort/adjacent-compare throughput on 65,536-row batches).

**Acceptance evidence.**
- *Behavioral:* synthetic fixture families ingest to exact expected rows;
  the conflicting-observation family materializes `fact_evidence` +
  conflict records through the ingress (Q4); §112.2 batch matrix green (empty batch, one row, all-nullable-null, max
  lengths, invalid ID length, duplicate PK, malformed spans).
- *Structural:* exactly one populated value representation per
  `value_kind_code` (validator test); every fact row carries the §9 metadata.
- *Negative:* a batch bypassing the ingest boundary cannot reach a writer
  (module privacy + governance rule); cross-context/cross-workspace rows
  rejected; stale generation rejected.
- *Operational:* ingest metrics (§111 subset: rows received/encoded,
  validation failures).
**Gates.** Root gates + fabric suite.
**Integration milestone.** M04.
**Replan triggers.** Validator throughput far below §64 batch-size targets →
performance adaptation (kernel composition), not scope change.
**Rollback.** Additive.

### WP21 — Mutation classes, owner replacement, idempotency

**Outcome.** Every table's §68 mutation class recorded in its `TableSpec` and
enforced by the writer layer; the §69 owner-replacement protocol (open →
predicate delete on `owner_id` set → append validated stream → reload →
validate counts/checksum → record Delta version) as two commits with
publication-pointer protection; delete+append as the normative baseline
(merge deferred per §69.1); §106 owner deletion across all owner-scoped
tables; §70 idempotency: `publication_id`/`operation_id`/`table_code`/
owner-set fingerprint/input checksum attached via `CommitProperties`
(preflight probe LD-04b) or, on probe failure, via the documented fallback
(deterministic operation identity + history/state reconciliation before any
retry); retry logic reloads state and inspects prior outcome — blind append
retry structurally impossible (the writer API requires an outcome
inspection token).

**Dependencies.** WP20.
**Target invariants.** I-14. Doctrine P24.
**Design and library references.** Data Fabric §68–§70, §106; roadmap §8
WP4. LD-04 caveats (a)(b)(e).

**Change surface / preflight.** Must touch: root `src/` writer-layer
modules; fabric suites. The §68 mutation classes are **authored in the WP09
TableSpec registry** — this packet *enforces* them in the writer layer; if
enforcement reveals a missing/incorrect class, the fix is a contracts edit +
regeneration under §4.1 item 7, not an inline override. Preflight:
`CommitProperties` metadata-setter compile probe against the pinned rev
(LD-04b); delete-metrics `Option` semantics (None = unknown, never zero).

**Acceptance evidence.**
- *Behavioral:* replace/remove/re-add owner cycles converge; §112.3 retry
  idempotency test (kill between delete and append; retry completes without
  duplication or loss).
- *Structural:* every table has exactly one mutation class (from the
  registry); commit metadata (or fallback records) carry the §70 key set;
  write-boundary fault points registered per §4.1.
- *Negative:* concurrent second writer to the same table yields a detected
  conflict, never silent duplication; blind-retry API path does not exist.
- *Operational:* owner-replacement latency metric.
**Gates.** Root gates + fabric suite.
**Integration milestone.** M04.
**Replan triggers.** LD-04b probe finds no commit-metadata mechanism at all →
plan revision adopting the fallback as primary (more code; §5.16–5.17
posture) — flagged to the user since it affects §70 conformance wording.
**Rollback.** Additive.

### WP22 — Durable publication and the current-pointer protocol

**Outcome.** The §71.1 durable-publication algorithm end-to-end for synthetic
facts: STAGING row → pins (source generation, inventory digest, context set,
fingerprints, bundle versions) → owner replacements → per-table version/
checksum records in `publication_table` → §75 integrity validation (Wave-3
subset — six of §75's sixteen "at minimum" checks: PK uniqueness, 16-byte
IDs, relation endpoints exist, owners exist, span sanity, row counts; the
other ten attach to tables that first exist in Waves 4+ — audit C-13) → VALIDATING→VALIDATED→COMMITTING→COMPLETE → AC-G-26
durable CAS on `current_publication` realized as: exclusive coordinator
publication lease → read pointer at pinned Delta version → predecessor +
generation verification → one-row replace committed from the version-pinned
handle (the pointer table is single-row, so any concurrent commit is
expected to be a *conflicting* change under OCC — the reference documents
conflict detection for conflicting changes only, so this is a
probe-confirmed assumption, A-32/audit L-2, not a documented guarantee) →
post-commit
reopen verifying exactly one row and expected generation → otherwise
`CURRENT_POINTER_CONFLICT` (LD-04a; the AC-G-26 text itself names OCC
conflict *or* predecessor mismatch as the failure legs); §107 failed-
publication recovery (active pointer untouched; abandoned versions
unreferenced; same-ID retry where safe).

**Dependencies.** WP21.
**Target invariants.** I-08, I-14. Doctrine P23, P24.
**Design and library references.** Data Fabric §12, §13.5–§13.7, §71.1, §75,
§107, AC-G-26; roadmap §8 WP6. LD-04a.

**Change surface / preflight.** Concurrency probe: two processes racing the
pointer commit → exactly one wins, loser gets a typed conflict; crash
injection between delete/append/pointer steps.

**Acceptance evidence.**
- *Behavioral:* publish → pointer advance → reopen shows the new base;
  validation failure marks FAILED/ABANDONED with pointer untouched.
- *Structural:* `publication_table` rows pin exact versions + checksums;
  states follow `DurablePublicationState` exactly.
- *Negative:* intermediate table versions invisible through serving reads;
  racing pointer writers → one `CURRENT_POINTER_CONFLICT`; crash at each step
  (fault points injected) recovers to a coherent state on restart.
- *Operational:* publication latency + diagnostics recorded.
**Gates.** Root gates + fabric crash/concurrency suite.
**Integration milestone.** M04.
**Replan triggers.** OCC at the pinned rev does not surface concurrent
pointer-table commits as failures (probe contradicts the deltalake
reference §9.22's OCC model — audit C-14) → plan
revision: pointer moves behind a SQLite-guarded commit sequence +
design issue to Data Fabric AC-G-26 — the spec's own hedge acknowledges
mechanism variance.
**Rollback.** Publications are additive; ABANDONED rows are inert.

### WP23 — Hot overlay: representation, policies, consolidation, rebase

**Outcome.** AC-G-20 `OverlayTable` (exact-schema replacement batches sorted
by PK ordering, `Arc<RecordBatch>` zero-copy sharing, owner + primary-key
tombstone indexes with the verbatim tombstone Arrow schemas, generation
bounds, content digest, hard memory reservation that fails before
activation); AC-G-21 policies enforced from `TableSpec` (OWNER_REPLACE /
PRIMARY_KEY_UPSERT / FULL_TABLE_REPLACE / BASE_IMMUTABLE / OPERATIONAL_ONLY;
partial replacement of a FULL_TABLE_REPLACE table rejected — AC-G-21's
escape hatch for a formally proven smaller stable replacement partition is
a Waves-5+ derivation-profile concern, out of Wave-3 scope, audit C-11); AC-G-22
deterministic consolidation (the seven rules; highest accepted
source_generation wins; equal generation requires identical payload digest
else `OVERLAY_GENERATION_CONFLICT`; digests recomputed from logical content);
the three-snapshot durable-rebase protocol (capture O_flush → publish
P_(n+1) → CAS pointer → rebase O_delta with content-digest-guarded row
removal → validate effective digest unchanged → activate S_new; failed CAS or
digest mismatch aborts and restarts from the current base).

**Dependencies.** WP21, WP22 (hard edge: the rebase protocol consumes
WP22's publication + pointer CAS), **WP24** (hard edge, audit B-1: the
rebase's "activate S_new" step and the consolidated-overlay swap consume
WP24's AC-G-26 activation transaction; WP23 executes **after** WP24 per
§15 — the packet numbering predates the audit reorder).
**Target invariants.** I-02, I-09, I-14. Doctrine P12, P17, P24.
**Design and library references.** Data Fabric §12.2, AC-G-20/21/22;
Lifecycle §101–§107 (overlay rationale); roadmap §8 WP5–6.

**Change surface.**
- *Must touch:* root `src/` overlay modules (representation, tombstone
  indexes, consolidation, rebase); fabric suites;
  `contracts/comparison/comparison-ignore-registry.yaml` (the
  overlay-vs-durable canonical-comparison rules — operational columns and
  file-layout fields excluded there, never inline);
  `contracts/faults/fault-point-registry.yaml` (rebase-boundary fault
  points).
- *Preflight:* none beyond WP22's probes (rebase reuses its CAS).

**Acceptance evidence.**
- *Behavioral:* property-based consolidation tests over generated mutation
  sequences: consolidate(merge(a,b)) ≡ consolidate(consolidate(a)+b);
  rebase preserves effective state bit-exactly (I-09 check via canonical
  comparison).
- *Structural:* overlay rows validate against the exact base schema digest;
  tombstones use the verbatim schemas; every overlay table's policy comes
  from `TableSpec`.
- *Negative:* equal-generation conflicting payloads →
  `OVERLAY_GENERATION_CONFLICT` blocks activation; memory reservation breach
  fails before activation; no query ever observes a chain of mutable
  overlays (activation swaps one consolidated overlay).
- *Operational:* overlay memory accounting metric.
**Gates.** Root gates + overlay property suite.
**Integration milestone.** M04.
**Replan triggers.** None beyond WP22's.
**Rollback.** The in-memory overlay is disposable, but rebase-produced
durable publications and pointer advances are not: rolling back WP23
requires the WP22 abandon/recovery path plus pointer restoration to the
pre-rebase base (and, in development, state-root disposal per WP13's
rollback rule).

### WP24 — ServingSnapshot manifest, activation transaction, leases, retention

**Outcome.** AC-G-19 manifest construction (complete field set; CBEF-encoded
body in generated schema order; `manifest_digest =
BLAKE3-256("codefabric-serving-snapshot-manifest-v1" || body)`;
`snapshot_id = BLAKE3-128(CBEF(SERVING_SNAPSHOT, manifest_digest))`;
construction fails closed on any missing required reference); the AC-G-26
activation transaction: SQLite `BEGIN IMMEDIATE` (insert READY manifest →
verify pointer generation + predecessor → retire predecessor → mark ACTIVE →
replace `active_snapshot` row → commit) then the in-memory `ArcSwap` —
never swapped before the durable commit; restart reconstructs memory from
SQLite choosing only fully-validating manifests; `SnapshotActivationRecord`
kept separate from the immutable manifest (A-12); AC-G-23 leases (kinds,
`ACTIVE|RELEASING|RELEASED|EXPIRED|ORPHANED`, 15 s heartbeats for >30 s work,
5-minute expiry, artifact-TTL coupling stubbed to query/resource kinds in
Wave 3, ORPHANED + 24 h crash grace); vacuum guards: retention set = current
publication ∪ active snapshot ∪ non-expired leases ∪ recovery-eligible
publications ∪ 7-day minimum window; `vacuum` dry-run-first recipe (§101
workflow — but the retention *set* is AC-G-23's five-element union, which
supersedes §101's narrower four-item enumeration, A-43/audit S-1).

**Dependencies.** WP22; WP13 (SQLite). Executes **before** WP23 (audit
B-1): this packet constructs and activates snapshots over a base
publication with an **empty overlay** (`overlay_generation` 0, no overlay
tables — a valid AC-G-19 manifest); WP23 later populates the overlay block
and consumes this packet's activation transaction. The lease/heartbeat
half depends only on WP13 and may be developed in parallel from M03.
**Target invariants.** I-02, I-08, I-14. Doctrine P19, P22, P24.
**Design and library references.** Data Fabric AC-G-19/23/26 (activation
leg), §101; roadmap §8 WP6–7. LD-04 (vacuum APIs confirmed), LD-09
(`ArcSwap`).

**Change surface.**
- *Must touch:* root `src/` snapshot/lease/vacuum modules; SQLite manifest +
  lease tables (WP13 store); `contracts/faults/fault-point-registry.yaml`
  (activation/pointer/lease fault points).
- *Preflight (A-13):* measure `effective_content_digest`/
  `primary_key_digest` computation cost over the synthetic corpus at
  publication scale; record the measurement with the design issue. Interim
  mechanism (compute at the publication boundary) is adopted only if the
  measured cost is acceptable at synthetic scale; otherwise escalate before
  building.

**Acceptance evidence.**
- *Behavioral:* build→validate→activate cycle; lease/heartbeat/expiry state
  walk; crash between SQLite commit and memory swap → restart converges to
  the committed snapshot (fault point injected); crash before commit → prior
  snapshot remains active.
- *Structural:* `snapshot_id` KAT vector (fixed manifest → expected ID);
  manifest excludes activation-record fields.
- *Negative:* activation with a failed digest verification is impossible;
  vacuum dry-run never lists a pinned file (test over a pinned+leased
  fixture); memory swap before durable commit structurally prevented (single
  code path, asserted by test hook ordering).
- *Operational:* lease table exposed read-only; orphan sweep idempotent.
**Gates.** Root gates + snapshot/lease suite with crash injection.
**Integration milestone.** M04.
**Replan triggers.** `effective_content_digest`/`primary_key_digest` cost
(A-13) makes snapshot construction non-interactive even for synthetic scale →
interim: compute at publication time (already a full-scan boundary), record
in manifest; design issue filed.
**Rollback.** Snapshots additive; pointer protocol governs visibility.

### WP25 — Overlay-aware DataFusion catalog, serving views, pinned-query proof

**Outcome.** A `ServingSnapshot`-pinned catalog: `CatalogProvider`/
`SchemaProvider` set exposing `cpg_control`, `cpg_base`, `cpg_serving` (Wave-3
subset; `cpg_python`/`cpg_rust`/`cpg_derived` namespaces registered empty);
per-table providers binding exact durable Delta versions
(`DeltaTableBuilder::with_version` + `table_provider()`), overlay
`MemTable`s, and the §91 effective-rows composition (overlay UNION ALL base
ANTI JOIN replaced-keys ANTI JOIN tombstones) built as a programmatic logical
view (preflight: `ViewTable` construction; fallback: thin custom
`TableProvider` composing the same plan — LD-01 grounding); DataFusion
runtime per §98 (bounded memory pool — not the unbounded default — spill
directory, batch size 65 536, pruning on); read-only SQL surface with DDL/DML
disabled (`SQLOptions`); serving views for the Wave-3 tables (`entities`,
`relations`, plus property/evidence projections) hiding operational columns
and joining enum dimensions for names — with the §92 full-view list recorded
as staged conformance (A-26); operational-store read-only projections under
`cpg_control` (§13.12 minimum set that exists in Waves 2–3; point-in-time
captures taken at snapshot-lease acquisition, documented as
operationally-current-not-snapshot-pinned — cross-store join semantics
recorded and ISSUE filed, A-56/audit Q5); **the pinned-
query proof**: a long-running query against a leased snapshot returns
identical results while an activation swaps the active pointer mid-flight.

**Dependencies.** WP23, WP24.
**Target invariants.** I-02, I-05, I-07 (daemon-owned catalog), I-12.
Doctrine P6, P21, P26 (views as projections).
**Design and library references.** Data Fabric §6.3, §91–§94, §98, §13.12;
roadmap §8 WP8. LD-01 grounding (view-over-providers is the recommended
pattern; anti-join semantics confirmed).

**Change surface / preflight.** The `ViewTable`/anti-join composition
probe was resolved in WP19's preflight (audit Q7) — this packet consumes
its outcome (programmatic view or thin-provider fallback). Remaining
preflight: `MemTable::try_new` partition shape (`Vec<Vec<RecordBatch>>`).

**Acceptance evidence.**
- *Behavioral:* SQL over `cpg_serving.entities`/`relations` returns effective
  rows across all overlay policies; §112.4 subset (catalog opens exact pinned
  versions; projection/filter pushdown observed in plans; plan snapshots
  recorded); the pinned-query proof passes deterministically (barrier-
  synchronized test).
- *Structural:* every catalog is constructed from one leased snapshot object;
  no mutable global pointer read inside providers (governance rule).
- *Negative:* DDL/DML rejected on the query session; hidden operational
  columns absent from view schemas; a table with an unspecified overlay
  policy fails catalog construction (schema error per §91).
- *Operational:* memory-pool limit + spill configuration observable; plan
  artifacts (§110 subset) emitted for the conformance queries.
**Gates.** Root gates + catalog suite → **Wave 3 exit evidence assembled
here** (M04).
**Integration milestone.** M04 (closes Wave 3).
**Replan triggers.** (Fires at WP19 preflight, where the probe now lives —
audit Q7.) Neither programmatic view nor thin provider composes the
anti-join plan with acceptable correctness → plan revision to a custom
`ExecutionPlan` (drags in the §18.20 pushdown test matrix — sized as +1
packet).
**Rollback.** Catalog additive.

---

## 11. Integration milestones

### M01 — Wave 0 exit: four domains build reproducibly (after WP01–WP05)

Roadmap §5 exit evidence, verified as a whole:
- clean checkout builds all four domains without manual edits;
- each executable **that exists at M01** (extractor, sidecar, adapter) prints
  its exact version/toolchain identity via `--identity` on STDERR; the stable
  root domain ships no product executable until WP06/WP12 — its identity
  surface (`codefabric-contracts --identity`) is asserted at M02;
- CI **is configured to reject** duplicate incompatible
  Arrow/DataFusion/object_store versions: the deny-family bans are live, the
  committed duplicate-family fixture proves the failure mode on every run,
  and full-graph enforcement becomes substantive when WP19 adopts the real
  Arrow graph (A-31 records this staging);
- no compiler-, Pyrefly-, gix-, delta-rs-, or FastMCP-internal type crosses an
  application-owned boundary (all five governance rules exist and run;
  trivially satisfied at shell stage);
- skeleton processes start and terminate cleanly with no non-protocol STDOUT.

Milestone gates: full reshaped `just ci-fast` + CI on a fresh clone (ubuntu
runner; macOS covered locally until the macOS runner joins at M02) + DB01
negative proofs (below).

### M02 — Wave 1 exit: Readiness Gate A (after WP06–WP08, WP08b, WP09–WP11)

Gate A verbatim (manifest Part V): "All registries, schemas, protocol
definitions, identity vectors, manifests, and traceability files exist and
pass `codefabric-contracts verify` without released-profile warnings."

Plus the roadmap §6 exit evidence: generated Rust and Python code compiles;
every released fixture validates against its public schema; identity, path,
type, enum, flag, and canonical-JSON known-answer vectors pass in both Rust
and Python; Protobuf packages compile and round-trip in both languages; no
mandatory requirement/kind/field is orphaned in traceability; re-running
generation from unchanged sources produces byte-identical canonical
artifacts.

Milestone gates: `codefabric-contracts verify --profile full` green **and**
`--profile released` with zero warnings (via `just contracts-verify` /
`contracts-verify-released`); double-regeneration byte-identity; four-domain
CI green with the macOS runner active from this milestone (WP07's
case-folding path rules need macOS in CI, not only locally).

### M03 — Wave 2 exit: secure source-instance control plane (after WP12–WP18)

Roadmap §7 exit evidence:
- multiple linked worktrees of one repository register as distinct
  workspaces;
- a non-Git root registers without a synthetic repository identity;
- authorized files captured byte-for-byte; escaped symlinks and path-prefix
  attacks fail;
- concurrent file mutation during capture never yields a falsely stable
  source image;
- restart restores registration and inventory state without claiming an
  active fact snapshot;
- adversarial path/permission fixtures pass for Linux and macOS profiles.

Milestone gates: traceability zero-orphan re-run (§4.1 item 1);
coordinator integration suite; adversarial corpus on both
platforms (macOS locally + CI job, Linux in CI); store crash-recovery suite;
lifecycle state machines model-checked; Wave-2 fault points registered in
`contracts/faults/fault-point-registry.yaml`.

### M04 — Wave 3 exit: canonical fact-state substrate (after WP19–WP25)

Roadmap §8 exit evidence:
- synthetic owner facts insert/replace/remove/overlay/flush/rebase/lease/
  query end to end;
- a query against a leased snapshot is unaffected by a later active-snapshot
  swap;
- crash/restart at publication and pointer boundaries recovers to one
  coherent current state;
- overlay merge equals durable effective state under canonical comparison
  (I-09);
- schema round-trip and integrity queries pass for the foundational tables.

Notes: under `local-workstation-v1` the overlay journal is disabled
(AC-G-08), so crash recovery legitimately loses un-flushed overlay state and
rebuilds from durable base + current source — recovery tests assert coherence,
not overlay survival. All Wave-3 crash boundaries are registered fault points.

Milestone gates: traceability zero-orphan re-run (§4.1 item 1); fabric
suite incl. crash/concurrency harnesses; pinned-query
proof; §11.1 round-trip gate per table; `just contracts-verify` still green
(registry drift guard); full four-domain CI.

---

## 12. Cross-packet decommission batches

### DB01 — Seed and packaging-surface zero state (prerequisites: WP01, WP04, WP05)

Old authorities that must reach zero and stay zero:

| Surface | Proof |
|---|---|
| `pyo3` in any dependency graph | `cargo tree -i pyo3` errors in all three Cargo roots (tier 1) |
| Maturin backend / wheel pipeline | `rg -n 'maturin' --hidden -g '!.git/**' -g '!docs/**'` zero hits outside plan/history docs (tier 3); no `[build-system]` referencing maturin |
| `codefabric._native` / `_native.pyi` | `rg -n '_native'` zero hits in live code (tier 3); no Python package `codefabric` at the root (tier 1: adapter imports resolve `codefabric_cpg_mcp` only) |
| `python/codefabric/`, `python_tests/`, root `pyproject.toml`, root `uv.lock` | paths absent; `.envrc`/`bootstrap.sh` reference only the adapter project |
| `python` cargo feature / cdylib | root `Cargo.toml` has no `[features]` python entry, no cdylib crate-type (tier 2 structural read) |
| Stale doc claims (two-compile-surface, v1.2 spec table) | `CLAUDE.md`/`AGENTS.md`/`README.md` updated; `rg -n 'features python|_native' CLAUDE.md AGENTS.md README.md` zero |

Exit invariant: all proofs green at M01 and re-checked mechanically at M02
(the sweep runs as a CI governance step, not a one-off). Coverage envelope:
whole repository including hidden files, excluding `.git/`, `docs/plans/`,
`docs/upfront_design/`, `docs/library_ref/`; `.claude/skills/` references to
the removed seed example are updated or annotated as historical in WP05.

---

## 13. Ambiguity register (spec gaps with dispositions)

Dispositions: **PREC** resolved by AC-G precedence (§2.2); **AUTHOR**
contract-authoring decision recorded in `contracts/` (the machine registry is
the code-generation authority — AC-G-70); **PROBE** packet-preflight
probe/spike; **DEV** recorded deviation; **ISSUE** design issue returned to
the owning spec (manifest Part V mandates this over invention).

| ID | Item | Owner spec | Disposition → where |
|---|---|---|---|
| A-01 | `BLAKE3_128` vs "BLAKE3-256 truncate 16" | Ontology AC-G-13 | AUTHOR: defined as BLAKE3-256[0..16] in `identity/cbef-v1.yaml` (WP07) |
| A-02 | CBEF field-tag numbers per domain recipe unassigned | Ontology AC-G-13 | AUTHOR: WP07 assigns; KAT vectors freeze them; ISSUE filed so the suite can canonize |
| A-03 | `record_domain` 2-byte codes + `platform_code` byte values unassigned | Ontology AC-G-13/18 | AUTHOR: WP07 (WTF-8 platform code reserved-unimplemented) |
| A-04 | Nested container length-prefix widths (types 9–12) | Ontology AC-G-13 | AUTHOR: u32 big-endian, recorded in cbef-v1.yaml (WP07) |
| A-05 | Type-2 `payload_length` pre/post normalization | Ontology AC-G-13 | AUTHOR: post-normalization (WP07) |
| A-06 | `UNKNOWN_MEMORY` vs `UNKNOWN_MEMORY_LOCATION` — audit: `UNKNOWN_MEMORY` is the majority spelling (10× across two docs vs 1× in AC-G-73); §66 also says SHOULD (vs mandatory) and lists only 7 of the 12 kinds | Ontology §66 vs AC-G-73 | PREC: AC-G-73's 12 names + mandatory force win (WP08); ISSUE filed for §66 alignment (audit S-7) |
| A-07 | MODELLED/HEURISTIC/UNRESOLVED appear in two registries with same codes | Ontology §62 | DEV: separate generated types per domain; codes coincide legitimately (WP08) |
| A-08 | Layer axis L0–L14 vs `family_code` linkage | Ontology §68/AC-G-70 | AUTHOR: `family_code` encodes the layer family; recorded in registry schema (WP08) |
| A-09 | `ProviderEvent` enum vs wire event names diverge | Fact Gen §90 vs AC-G-30/31/32 | AUTHOR: one in-process enum + explicit wire→enum mapping table in `rpc/feature-registry.yaml` companion doc (WP10) |
| A-10 | §7.3 Pyrefly request groups vs AC-G-30 six operations | Fact Gen | PREC: AC-G-30 wire set; §7.3 groups become capability codes/module-request options (WP10) |
| A-11 | `TableSpec` lacks overlay policy; **three** overlapping taxonomies (§11 `OwnerReplacementPolicy` with undefined variants, §68's six prose mutation classes naming no enum, AC-G-21's five-value `OverlayMutationPolicy`), and §91 adds a fourth category `query-time derived` absent from AC-G-21 (audit S-2; re-verified in the revised spec) | Data Fabric §11 vs §68/§91/AC-G-21 | AUTHOR: TableSpec gains `overlay_policy: OverlayMutationPolicy`; `OwnerReplacementPolicy` (§11) retained for the durable §68 mutation class; `query-time derived` mapped to `OPERATIONAL_ONLY`/view-backed surfaces; relationship documented (WP09); ISSUE filed |
| A-12 | §13.8 manifest fields vs AC-G-19; activation state in/out of manifest | Data Fabric | PREC: AC-G-19 field set; mutable `SnapshotActivationRecord` separate (WP09/WP24) |
| A-13 | `effective_content_digest`/`primary_key_digest` computation cost undefined | Data Fabric AC-G-19 | PROBE+ISSUE: computed at publication boundary in Wave 3; design issue filed (WP24) |
| A-14 | Three overlapping workspace state machines | Lifecycle §18 / AC-G-10 / AC-G-28 | DEV: D-06 — **two persisted machines** (AC-G-10 registry + §18 lifecycle) with AC-G-28 derived as a projection over the §130.2 four-column tuple, generated from AC-G-25 YAML (WP08/WP14; audit Q3) |
| A-15 | Nested-root exclusion record has no schema | Lifecycle AC-G-10 | AUTHOR: operational-store table added in WP09 DDL / WP13 |
| A-16 | `authorization_fingerprint` (and inclusion/attributes/trust fingerprints) preimages unspecified | Lifecycle §26/§50/AC-G-11 | AUTHOR: CBEF records over the respective policy field sets, in cbef-v1.yaml (WP07/WP14) |
| A-17 | Provider-control/sidecar/extractor proto package+message names | Fact Gen AC-G-30/31/32 | AUTHOR: file names fixed by AC-G-05; packages `codefabric.provider.v1` / `.pyrefly.v1` / `.rustc.v1` authored in WP10; ISSUE filed |
| A-18 | Credit-control constants duplicated (4 chunks / 16 MiB) | Fact Gen AC-G-30/31 | AUTHOR: single shared constant in the provider-control contract (WP10) |
| A-19 | Provider-registry record format absent | Fact Gen AC-G-36 / roadmap | AUTHOR: `registry/provider-registry.yaml` record = id, placement (in-process/sidecar/compiler-group), protocol package, bundle-digest fields, capability codes (WP08) |
| A-20 | Walker/gix bound values named but not numbered | Lifecycle §47.1/§79 | AUTHOR: defaults recorded in `deployment/local-workstation-v1.yaml` overrides section (WP16); Appendix-B starting points used where given |
| A-21 | Pyrefly 1.2.0 source tag/rev not stated | Fact Gen §2 | PROBE: WP03 resolves + digests (git rev preferred per D-09); replan if unresolvable |
| A-22 | `rustc_public` consumption mode (rustc-dev vs crates.io) | Fact Gen §2 | **DECIDED (D-09)**: `rustc-dev` components are the baseline; WP02 confirms mechanics and records the exact commit hash |
| A-23 | Merkle inventory digest SHOULD vs required `worktree_inventory_digest` | Lifecycle §34.3 vs §50 | DEV: treated as required (WP16) |
| A-24 | gix pin omits `revision` feature | Lifecycle §39 | PROBE: WP17 (the gix reference annotates HEAD accessors with no feature gate, so the expectation is no-feature-needed; unborn-HEAD failure is the documented hazard, already fixtured); add feature only if the probe requires it (DEV recorded if so) |
| A-25 | `WorkspaceCoordinatorState.active_snapshot` non-optional before Wave 3 | Lifecycle §26 | DEV: AC-G-28 `NO_SNAPSHOT` startup state until WP24 (WP18) |
| A-26 | §92 requires 23 serving views; Wave 3 has 4 base tables | Data Fabric §92 | DEV+ISSUE: staged conformance recorded; views land with their tables in Waves 4+ (WP25) |
| A-27 | PlanSpec serialization dialect unstated | Query AC-G-46 | AUTHOR: JSON Schema 2020-12, both unbound and bound forms (WP09) |
| A-28 | `b3:` vs `blake3:` digest prefixes in response examples | Query AC-G-53 vs §36.1 | PREC: `b3:` (AC-G-53); response schema uses one prefix family (WP09); ISSUE noted |
| A-29 | Fixture-corpus location for cross-language JCS vectors | Query AC-G-53 | AUTHOR: `contracts/fixtures/` shared root (WP05/WP06) |
| A-30 | Data Fabric §2.1 workspace form vs repo-spec no-preallocated-workspace | Data Fabric / repo-spec | DEV: D-02 — pins verbatim, workspace form deferred until a second stable-domain package exists |
| A-31 | Wave 0 "pin supporting crates" vs unused-dependency hygiene gates | Roadmap §5 | DEV: pins recorded in toolchain bundle at WP11; cargo adoption at first use (LD-14) |
| A-32 | AC-G-26 conditioned commit vs delta-rs capability | Data Fabric AC-G-26 | PROBE: WP22 OCC+predecessor+reread mechanism; ISSUE only if probe contradicts §9.22 semantics |
| A-33 | `CommitProperties` metadata setter unverified; no app-txn at pinned rev | Data Fabric §70 | PROBE: WP21; fallback documented |
| A-34 | Delta CHECK constraints post-create only | Data Fabric §67/§102 | DEV: create→constrain-while-empty sequence (WP19) |
| A-35 | `worktree_state` naming vs workspace keying; `repository` vs `common_repository` table name | Lifecycle §130 / Data Fabric §13.2 vs §68/§95.1 | AUTHOR: registry fixes `worktree_state` (keyed by workspace_id, documented) and `common_repository` as canonical names (WP09) |
| A-36 | `fact_evidence` primary key undeclared | Data Fabric §16.2 | AUTHOR: PK `(workspace_id, analysis_context_id, evidence_id)` in TableSpec (WP09); ISSUE noted |
| A-37 | GitStateVector index fingerprint has no gix API | Lifecycle §50 | PROBE: WP17 (rustdoc probe → entry-tuple hash fallback → stat-based + ISSUE) |
| A-38 | Singleton lease scope wording ("user domain" vs "repository/worktree group") | Lifecycle §75 / AC-G-27 / AC-G-62 | AUTHOR: one lease + one operational DB per daemon **state root** (AC-G-62's group lock reading); recorded in config contract (WP12/WP13) |
| A-39 | Overlay "capability withdrawals" have no AC-G-20 representation | Data Fabric §12.2 vs AC-G-20 | DEV: capability changes ride `capability_status` owner-replacement rows in the overlay; ISSUE filed (WP23) |
| A-40 | Python interpreter exact pin absent (floor >=3.12 only) | Serving §18 | AUTHOR: 3.14.7 pinned in adapter `.python-version`/lock (WP04) |
| A-41 | tonic/prost UDS + oneof + 4 MiB cap + Python interop unverified (no reference doc) | Serving §8 (transport mechanics) | PROBE: WP05 compatibility probe before any contract code depends on it (LD-10) |
| A-42 | Serving §54 shows an adapter-local `proto/cpg_query_service.proto`; AC-G-05 fixes `contracts/rpc/` as the single source | Serving §54 vs manifest AC-G-05 | PREC: AC-G-05 wins (one generating source, AC-G-01); adapter consumes generated stubs only; ISSUE noted for §54 |
| A-43 | Data Fabric §101's vacuum-retention list omits the active snapshot and non-expired leases that AC-G-23 protects (§101's new checkpoint note implies lease pinning but the enumerated list is incomplete; re-verified in the revised spec) | Data Fabric §101 vs AC-G-23 | PREC: AC-G-23's five-element union implemented (WP24); ISSUE filed (audit S-1) |
| A-44 | §13.1 `workspace` has no `registration_revision`/`updated_at`, yet AC-G-10 relink/configure mutate identity fields and AC-G-19 pins `registration_revision` | Data Fabric §13.1 | AUTHOR+ISSUE: columns added in the WP09 TableSpec; D-08 upserts on every revision bump; WP19 relink test (audit S-3/Q5) |
| A-45 | §62.7–§62.9 are uncoded enum blocks — no numeric codes, unlike §62.1–§62.6 | Ontology §62 | AUTHOR: codes assigned in WP08 registries under §62.10 append-only rules (audit S-8) |
| A-46 | Fabric §2.1's `deltalake` dependency entry is a multi-line inline table — invalid TOML as printed (re-verified in the revised spec) | Data Fabric §2.1 | ISSUE filed; WP19 transcribes it as a `[dependencies.deltalake]` table (audit S-4) |
| A-47 | §154 requires Git acceleration "`CURRENT`" but `GitAccelerationStatus` has no such member | Lifecycle §154 vs §18 enums | DEV: mapped to `GIT_READY` (WP18); ISSUE filed (audit S-5) |
| A-48 | §64.7 SHOULD vs AC-G-13 SHALL on retaining the full 256-bit digest | Ontology §64.7 vs AC-G-13 | PREC: AC-G-13's SHALL (WP07 retains the 32-byte digest); ISSUE noted (audit S-6) |
| A-49 | Serving §9's seven-RPC form is superseded by AC-G-58 but never annotated as such in the spec | Serving §9 vs AC-G-58 | Already applied as PREC (§2.2); ISSUE filed for the missing annotation (audit S-9) |
| A-50 | The deltalake reference cites `datafusion_54vs53.md` four times; the file does not exist in `docs/library_ref/` | deltalake reference doc | ISSUE (library-ref hygiene); 54.0-vs-54.1 delta claims verify against `datafusion_rust.md` at WP19 preflight (audit S-10 residual) |
| A-51 | AC-G-04's fourth zero-orphan condition (query phrases with no executable mapping) cannot be literally satisfied before the Wave-15 compiler exists | Manifest AC-G-04 | DEV: phrase entries carry explicit deferred-mapping records until Wave 15 (WP08b/WP11; audit C-2) |
| A-52 | Decoder rejection of out-of-order CBEF field tags is not in AC-G-13 (which fixes emission order only) | Ontology AC-G-13 | AUTHOR: rejection specified in `identity/cbef-v1.yaml` as decode-side hardening (WP07); ISSUE noted (audit C-5) |
| A-53 | AC-G-27 persisted domains (workspace registration, credentials metadata, generation counters) have no §130 table schemas | Lifecycle AC-G-27 vs §130 | AUTHOR: WP09 DDL defines them; WP13 consumes (audit C-23) |
| A-54 | Lifecycle Appendix F defines conflicting, poorer `GitRepoPath`/`PlatformPath` forms than §43; no intra-document precedence is stated | Lifecycle §43 vs Appendix F | DEV: §43's richer forms adopted (WP07/WP15); ISSUE filed — this was mislabeled as AC-G precedence in v1 §2.2 (audit C-23) |
| A-55 | §76 names allowed-config-sources and env-override acceptance as sandbox policy dimensions but assigns them no default values | Lifecycle §76 | AUTHOR: strictest values (restricted sources, overrides refused) in the WP17 trust policy (audit C-23) |
| A-56 | `cpg_control` joins publication-pinned Delta tables with live SQLite projections; cross-store consistency semantics are unspecified | Data Fabric §13.12 | DEV: projections captured at snapshot-lease acquisition and documented operationally-current (WP25); ISSUE filed (audit Q5) |

---

## 14. Final gate matrix

Run at every milestone from the "First at" column onward; full matrix at
M04. Every cell is a `just` recipe (the recipe wraps the shown command with
correct project/working-directory handling). Baseline failures: none (clean
baseline, §3).

| Domain | Gate | Recipe → command | First at |
|---|---|---|---|
| Stable root | format | `root-fmt` → `cargo fmt --all -- --check` | M01 |
| Stable root | compile + lints | `root-check` / `root-clippy` → `cargo check --all-targets`; `cargo clippy --all-targets -- -D warnings` | M01 |
| Stable root | tests | `root-test` → `cargo nextest run`; `cargo test --doc` | M01 |
| Stable root | MSRV | `msrv` → `cargo msrv verify` (rust-version 1.94.1) | M01 |
| Stable root | dependency hygiene/policy | `deps-fast` / `policy` → machete, shear, `cargo deny check` (family bans + delta-rs git-source exception from WP19), `cargo audit` | M01 |
| Stable root | gix feature envelope | `gix-features` → `cargo tree -e features -i gix` asserted against the pinned set | M03 (wired at M01) |
| Extractor | compile + tests on pinned nightly | `extractor-check` / `extractor-test` (runs in `rustc-extractor/`) | M01 (path/pin-triggered + scheduled + milestone gates — audit Q6) |
| Sidecar | compile + tests + policy | `sidecar-check` / `sidecar-test` / `sidecar-policy` (runs in `pyrefly-sidecar/`) | M01 (path/pin-triggered + scheduled + milestone gates — audit Q6) |
| Adapter | lint + types + tests | `adapter-lint` / `adapter-type` / `adapter-test` → each `uv run --frozen --project codefabric-cpg-mcp {ruff format --check .; ruff check .; pyrefly check; pytest}` | M01 |
| Adapter | schema fingerprints | `adapter-schemas` → `uv run --frozen --project codefabric-cpg-mcp python -m codefabric_cpg_mcp.schema_export --check` | M02 |
| Adapter | STDIO discipline | `adapter-stdio-test` → locked-command spawn test (Serving §68.6) | M01 |
| Contracts | Gate A verify | `contracts-verify` → `cargo run --bin codefabric-contracts -- verify --profile full`; `contracts-verify-released` → `… --profile released` (zero warnings) | M02 |
| Contracts | regeneration byte-identity | `contracts-regen-check` → regenerate twice + `git diff --exit-code` on `contracts/generated/` and committed stubs | M02 |
| Cross-language | KAT + JCS + proto round-trip | Rust nextest suites + adapter pytest suites over `contracts/fixtures/` | M02 |
| Governance | boundary rules + negative fixtures | `governance` → `ast-grep scan` (gix/delta/provider/FastMCP-internal/secure-open rules; DB01 sweep) + expected-failure fixtures (duplicate-family, perturbed-artifact, broken-trace-edge, schema-drift) | M01 (rule set grows per packet) |
| Repo hygiene | spelling | `typos` | M01 |
| CI | four-domain clean-checkout build + smoke tests | `.github/workflows/ci.yml` — ubuntu from M01, macOS runner added at M02 | M01 |

Tier discipline preserved from the repo-spec: mutating recipes never gate;
Tier C tools (miri, mutants, fuzz) remain risk-triggered — none is mandated by
Waves 0–3 scope (no unsafe code planned; parser/untrusted-input fuzz surfaces
begin with real providers in Wave 4, except the CBEF/JCS decoders, which get
proptest-style negative suites in WP06/WP07 instead of cargo-fuzz — revisit at
Wave 4 when `fuzz/` is justified).

### 14.1 Performance evidence per wave (roadmap §28 item 11)

Measurements are recorded as baseline artifacts (committed under
`target/perf-baselines/` exports into the packet evidence record, or the
execution-state file); no SLO gates before Wave 19 — these establish the
baselines Gate F will later consume.

| Wave | Measurement | Where captured |
|---|---|---|
| 0 | clean-build wall time per domain; sccache hit rate (telemetry only) | WP05 CI run |
| 1 | full contracts regeneration wall time; KAT corpus runtime | WP11 |
| 2 | source-capture throughput (files/s, MB/s) and walker duration on a generated 10k-file fixture tree; secure-open overhead vs raw open | WP16/WP15 |
| 3 | owner-replacement latency, publication end-to-end latency, overlay consolidation time and peak memory, catalog point-query latency over the synthetic corpus | WP21/WP22/WP23/WP25 |

---

## 15. Execution sequence

The diagram below matches each packet's declared Dependencies line exactly;
where they ever diverge, the Dependencies line wins.

```text
Wave 0:   WP01 ──► { WP02 ∥ WP03 ∥ WP04 } ──► WP05 ──► M01
          (WP02/03/04 parallel — disjoint directories; WP05 integrates)

Wave 1:   M01 ──► WP06 ──► WP07 ──► WP08 ──► { WP08b ∥ WP09 ∥ WP10 } ──► WP11 ──► M02
          (strictly WP06→WP07→WP08: WP07 consumes the verifier, WP08
           consumes WP07's slug/ID conventions and both write the shared
           generated trees; WP08b, WP09, WP10 are parallel — disjoint
           outputs, and the §50–§94 phrase harvest stays off the WP09/WP10
           critical path (audit B-3); WP11 last)

Wave 2:   M02 ──► WP12 ──► WP13 ──► WP14 ──┐
                           WP13 ──► WP15 ──► WP16 ──► WP17 ──► WP18 ──► M03
          (WP14 parallel with WP15–WP17 after WP13; WP17 needs WP15+WP16
           +WP12; WP18 integrates everything)

Wave 3:   M03 ──► WP19 ──► WP20 ──► WP21 ──► WP22 ──► WP24 ──► WP23 ──► WP25 ──► M04
          (linear with WP24 before WP23 — audit B-1 reorder: WP24 activates
           snapshots over an empty overlay first; WP23's rebase consumes
           WP22's pointer CAS *and* WP24's activation transaction — both
           hard edges. WP24's lease/heartbeat half depends only on WP13 and
           may be developed in parallel from M03.)
```

Parallel-prework note (roadmap §4.2): nothing in this plan begins Wave 4+
work; the extractor/sidecar shells deliberately stop at protocol-silent
skeletons.

---

## 16. Completion checklist

- [ ] WP01 — stable-domain re-baseline; seed removed
- [ ] WP02 — nightly extractor domain shell
- [ ] WP03 — Pyrefly sidecar domain shell
- [ ] WP04 — FastMCP adapter domain shell
- [ ] WP05 — protobuf toolchain, command contract, four-domain CI
- [ ] **M01** — Wave 0 exit evidence
- [ ] DB01 — seed/packaging zero-state proofs green in CI
- [ ] WP06 — contracts tree + JCS + verifier core
- [ ] WP07 — CBEF identity + paths + KAT vectors (Rust+Python)
- [ ] WP08 — registries + state machines (incl. `DurablePublicationState`
      and `ServingActivationState`)
- [ ] WP08b — phrase registry (§50–§94) + grammar + model-pack schema
- [ ] WP09 — schema generation (TableSpecs, snapshot, JSON Schemas, adapter)
- [ ] WP10 — four Protobuf packages + feature registry
- [ ] WP11 — bundles, deployment profile, CF-ID traceability
- [ ] **M02** — Gate A: `codefabric-contracts verify` clean
- [ ] WP12 — daemon kernel + singleton lease + discovery file
- [ ] WP13 — SQLite WAL operational store
- [ ] WP14 — workspace registry + admin lifecycle + identity
- [ ] WP15 — root authorization + secure open + path identity
- [ ] WP16 — source images + blob store + inventory + generations
- [ ] WP17 — gix read-only discovery + topology (3 probes resolved)
- [ ] WP18 — coordinator + bootstrap + readiness
- [ ] **M03** — Wave 2 exit evidence incl. adversarial corpus (Linux+macOS)
- [ ] WP19 — schema-registry runtime + Delta namespace + control plane
- [ ] WP20 — fact core + observation boundary + validators
- [ ] WP21 — mutation classes + owner replacement + idempotency
- [ ] WP22 — durable publication + current-pointer protocol
- [ ] WP24 — ServingSnapshot + activation + leases + retention
      (empty-overlay activation first — B-1)
- [ ] WP23 — hot overlay + consolidation + rebase (after WP24 — B-1)
- [ ] WP25 — overlay-aware catalog + pinned-query proof
- [ ] **M04** — Wave 3 exit evidence; final gate matrix green
- [ ] Ambiguity register: every ISSUE item filed as a design issue against its
      owning spec (roadmap §28 / manifest Part V)

---

## 17. Plan risks and replan policy

### 17.1 Risks

- **R-01 (downgraded — resolved during planning).** The suite governance
  manifest was initially absent; it has been added and integrated (digest in
  §2.1). Residual risk: none beyond ordinary spec-revision drift; any future
  edit to the manifest re-opens §2.1's staleness boundary.
- **R-02 delta-rs capability gaps.** No CAS primitive; commit-metadata setter
  unverified; no app-transaction idempotency at the pinned rev. Mitigation:
  WP21/WP22 preflight probes with documented fallbacks; the AC-G-26 text
  itself names OCC-or-predecessor-mismatch as acceptable failure legs.
  Escalation: probe contradiction → plan revision + design issue.
- **R-03 Pyrefly pin resolution.** The integration decision is settled
  (D-09); the residual risk is only that no exact 1.2.0 source rev can be
  resolved or that it fails to build on stable. Mitigation: WP03 resolves,
  digest-pins, and link-proves the Query facade; replan if unresolvable.
- **R-04 gix unknowns.** Index fingerprint source, linked-worktree exact-path
  open, write/lock freedom. Mitigation: three WP17 preflight probes with
  ordered fallbacks; CI resolved-feature assertion.
- **R-05 Rust gRPC stack ungrounded.** tonic/prost have no pinned reference
  doc in this repo. Mitigation: WP05 compatibility probe (UDS, oneof, caps,
  Python interop) before any contract code depends on it; library evidence
  hierarchy followed (official docs + compile probes).
- **R-06 contract-authoring drift.** CBEF field tags, domain codes, proto
  package names, provider-registry record are minted here (AUTHOR items). If
  the suite later canonizes different values, §0.4's exact-match rule forces
  regeneration and possibly reindexing. Mitigation: every AUTHOR item is
  filed as a design issue so the suite can adopt or correct; all minted
  values live in exactly one contracts file each.
- **R-07 Wave 1 registry breadth (mitigated by pre-split).** The ontology
  registry spans ~50 fact domains; authoring is large but mechanical. The
  v2 audit integration pre-applied the split this risk foresaw: the
  §50–§94 phrase harvest is WP08b, parallel to WP09/WP10 and off the
  critical path (B-3). Mitigation: registry schema + verifier land first
  (WP08 acceptance is invariant enforcement, not domain completeness
  prose). Escalation: if breadth still stalls M02, split further by
  registry family — plan revision, sequence unchanged.
- **R-08 platform coverage.** Dev machine is macOS; AC-G-11 requires
  Linux `openat2` behavior too. Mitigation: ubuntu CI from M01, macOS runner
  from M02 (WP07 path rules), adversarial corpus on both from M03; before
  M02, macOS coverage is the dev machine and is recorded in packet evidence.
- **R-09 in-flight spec revisions.** The data-fabric spec was revised in
  place mid-planning (§2.1 note). Any further in-place revision of a §2.1
  input re-opens the staleness boundary: execution preflight re-digests all
  eight inputs and a digest change is a plan-revision trigger (§17.2).
- **R-10 pinned-toolchain update churn (accepted, D-09).** Nightly and
  Pyrefly pins will need periodic deliberate bumps, each of which can move
  `rustc_public`/Pyrefly-internal APIs. Mitigation: the D-09 managed update
  procedure (branch bump → domain-isolated rebuild → conformance surface →
  bundle digests → version negotiation); the golden corpora grow with Waves
  5/9/10 precisely to make these bumps mechanical. Escalation: an update
  whose API breakage exceeds adapter-level adaptation → design issue to
  Fact Gen §2 per repo-spec §76, not an ad-hoc in-place fix.

### 17.2 Replan policy

- **Implementation adaptation** (record in execution state, no plan change):
  choosing probe-selected mechanisms among the fallbacks this plan already
  enumerates (e.g., gix worktree enumeration route, index-fingerprint
  material, ViewTable vs thin provider, CommitProperties fallback); adding a
  spec-permitted feature flag with a recorded deviation (A-24).
- **Plan revision** (new plan version): packet boundaries or sequence change;
  a probe eliminates all enumerated fallbacks; Wave-1 breadth split (R-07);
  the governance manifest or any §2.1 input changes digest.
- **Design reopening** (return to the owning 1.3 spec, per manifest Part V):
  any change to public contracts, identity/type-algebra versions, gate
  definitions, or a contradiction between two normative owners that
  precedence rules cannot resolve. The executor never resolves such a
  conflict ad hoc in code.

### 17.3 Standing packet-level replan triggers (apply to every packet)

1. A pinned library API is absent or behaviorally incompatible at its exact
   pin and no enumerated fallback survives.
2. The current tree contradicts a Must-Touch assumption (staleness).
3. A packet cannot be left dependency-closed without pulling in deferred-wave
   scope.
4. A target invariant (§4) cannot be satisfied with the planned mechanism.
5. Security or correctness evidence invalidates an approach (e.g., secure-open
   bypass on a supported platform).

---

## 18. Traceability summary

| Roadmap wave work package | Plan packets |
|---|---|
| W0.1 stable domain | WP01, WP05 |
| W0.2 nightly extractor domain | WP02 |
| W0.3 Pyrefly sidecar domain | WP03 |
| W0.4 FastMCP adapter domain | WP04 |
| W0.5 command/CI contract | WP05 |
| W0.6 protobuf prerequisites | WP05 (generation verified both languages + canonical output locations; the real packages are W1.6/WP10) |
| W1.1 contracts tree | WP06 |
| W1.2 contract compiler/verifier | WP06, WP11 |
| W1.3 registries | WP08, WP08b |
| W1.4 identity/canonicalization | WP07 |
| W1.5 schema generation | WP09 |
| W1.6 protocol generation | WP10 |
| W1.7 bundles/deployment | WP11 |
| W1.8 traceability | WP11 |
| W2.1 daemon lifecycle kernel | WP12, WP18 |
| W2.2 operational-state persistence | WP13 |
| W2.3 workspace admin lifecycle | WP14 |
| W2.4 root authorization/secure open | WP15 |
| W2.5 file/path identity | WP07, WP15 |
| W2.6 source-image capture | WP16 |
| W2.7 initial inventory | WP16, WP17 |
| W2.8 admin/diagnostics | WP14, WP18 |
| W3.1 schema/table registry runtime | WP19 |
| W3.2 control-plane tables/views | WP19, WP25 |
| W3.3 universal fact core | WP20 |
| W3.4 Delta durable namespace | WP19, WP21 |
| W3.5 hot overlay | WP23 |
| W3.6 publication/pointer protocols | WP22, WP24 |
| W3.7 snapshot leases/retention | WP24 |
| W3.8 overlay-aware catalog | WP25 |

Normative requirement inventory (roadmap §28 item 2), by `G-*` contract and
depth of realization in this plan:

| Wave | Fully realized | Contract-only (runtime later) |
|---|---|---|
| 0 (M01) | build-domain topology per AC-G-05's `contracts/` + generated-artifact posture; §2.1/§2.2 pin + duplicate-family obligations | G-30/31/35 domain shells (protocol/runtime later) |
| 1 (M02) | G-01–G-08 (manifest Part I), G-13, G-15 (algebra contract + encoder), G-18, G-25 (machine YAML + checker), G-70, G-71, G-72 (registry defs), G-73 (registry defs) | G-06-derived enums for later-wave domains; G-30–G-34 (proto/DTO contracts), G-36 (registry record), G-38 (schema only), G-44/G-46/G-53 (grammar/PlanSpec/JCS artifacts; resolver/compiler are Waves 15+), G-58/G-65 (proto + error registry; service runtime Wave 17) |
| 2 (M03) | G-09, G-10, G-11, G-12 (file_id runtime), G-27, G-28, G-62; G-33's daemon-side source-image store | G-24/G-29/G-41 (watcher-era; schemas exist) |
| 3 (M04) | G-19, G-20, G-21, G-22, G-23, G-26 | G-37 (reconciliation — bounded synthetic stub only, D-07), G-42 (registry exists, matrix populated Waves 5+) |

Exact `CF-*` IDs are minted in WP11's `requirements.jsonl` (AC-G-04 owner
prefixes) and become the machine form of this table; the execution-state
record links packets to CF-IDs as they land.

*End of plan v2 — audit-integrated revision of v1; see the Audit Integration Log.*
