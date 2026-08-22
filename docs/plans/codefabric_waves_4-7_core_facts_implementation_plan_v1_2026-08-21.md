---
artifact: implementation-plan
plan_id: codefabric-waves-4-7-core-facts
version: v1
date: 2026-08-21
status: draft
design_path: docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md
design_version: v1.0
predecessor_plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md
baseline_commit: 63c1e9625f6c852ab918467b03cf16ee69660ab7
state_path: docs/plans/state/codefabric-waves-4-7-core-facts_state.json
cutover: true
---

# CodeFabric Waves 4–7 Core Facts — Implementation Plan v1

This plan converts Waves 4–7 of the CodeFabric 1.3 implementation roadmap into
dependency-closed work packets. It covers:

- **Wave 4** — Source and syntax fact generation (Tree-sitter/Ruff providers,
  canonical source/syntax tables, capability and unknown handling).
- **Wave 5** — End-to-end vertical golden slice (**Readiness Gate B**: golden corpus,
  thin rustc extractor slice, minimal reconciliation, derivation, query, accepted-handle
  service, hot+durable update path).
- **Wave 6** — Continuous update, freshness, and core equivalence
  (`CORE_SOURCE_V1` COMPLETE; incremental core-source scenarios compare equal to rebuild).
- **Wave 7** — Git-aware lifecycle acceleration and topology (Git transitions converge
  with generic fallback preserved).

The Waves 0–3 foundation plan v5 is **complete** (all packets WP00–WP26 and milestones
M00–M04 complete with proving commits). This plan is its successor and continues the
global packet/milestone/decommission ID sequence — packets **WP27–WP53**, milestones
**M05–M08**, decommission batches from **DB07** — so that packet proof-oracle test names
(`wpNN_behavioral_acceptance` …) remain globally unique across plans (D-24).

Sizing note: `RM §2.1` sizes one wave at four to eight major work packages, and `RM §28`
permits an integrated program plan when execution remains wave-segmented. This plan is
integrated across Waves 4–7 by user direction; execution loads one wave segment at a
time, each wave keeps four to eight packets, each wave exits through its own milestone,
and no later-wave prework may be reported as completion of an earlier packet (D-23).

---

## 1. Outcome and non-goals

### 1.1 Outcome

At M08, CodeFabric has:

1. **Canonical source and syntax facts** for valid, partially invalid, and malformed
   Python/Rust files: Tree-sitter and Ruff providers behind the AC-G-32 job runtime,
   the four FAB source/syntax tables (`source_file`, `source_token`, `source_annotation`,
   `syntax_detail`) generated from the schema Contract IR, ordered `AST_CHILD` derived
   from `syntax_detail`, deterministic identity under unchanged bytes, and explicit
   capability/unknown outcomes for unsupported input (Wave 4, M05).
2. **A proven vertical architecture**: Readiness Gate B passes exactly as `SUITE` Part V
   defines — one Python owner, one Rust MIR owner, one unknown fact, one property fact,
   one relation fact, one derived projection, one hot-overlay update, one durable
   publication, one semantic query, one streamed result, one artifact result — over the
   released golden corpus `tests/golden/codefabric-golden-v1/` (Wave 5, M06).
3. **A continuously maintained present-state service** for core source facts: watcher
   facade, dirty registry and update waves, invalidation graph, fast syntax lane,
   the formal freshness/barrier machine, continuous overlay rebase and durable flush,
   startup/crash recovery, and the AC-G-79 clean-rebuild comparator proving incremental
   ≡ clean rebuild on the core edit corpus. `CORE_SOURCE_V1` is advertised `COMPLETE`
   with exact per-owner coverage (Wave 6, M07).
4. **Git-aware acceleration that never changes authority**: git-native inventory,
   status/index and HEAD-tree candidate acceleration fenced by `GitStateVector` and
   current bytes, linked-worktree/submodule topology, bounded caches keyed by blob OID
   only under the LIFE §63.1 safeguards, and a proven generic fallback — disabling gix
   yields identical semantic state (Wave 7, M08).

### 1.2 Current-state continuation boundary

The baseline is the clean tree at the frontmatter commit. The Wave 0–3 substrate this
plan consumes exists and is proven (session-verified by structural survey):

- `source_image.rs`: `SourceImage`/`SourceSnapshot` with authoritative bytes,
  `ProviderText` byte-offset mapping, `LineIndex`, encoding/newline metadata, blob
  store, leases, `capture_with_fence`, and the `advance_source_generation` CAS.
- `fact_ingest.rs`: fact-row DTOs, Arrow encoders, the eleven-check batch validator,
  `ObservationManifest`/`ObservationStream` bounded channels, and the bounded
  `SyntheticCanonicalIngest` behind the production reconciliation signature (v5 D-07).
- `fabric/*`: 17 generated TableSpecs, owner-replacement mutation with idempotency
  journal, durable publication with pointer CAS, consolidated overlays with rebase,
  `ServingSnapshot` manifests, activation, leases, retention, and pinned read-only
  DataFusion serving sessions (`ServingQuerySession`).
- `git_state.rs`: the read-only gix boundary (`GitStateAdapter`, `GixGitStateAdapter`,
  `GitBlockingExecutor`) guarded by rules `gix-boundary-only` and `gix-read-only`.
- Generated contracts: registries and state machines in `src/generated/registries.rs`,
  provider/extractor/sidecar/query protocols in `src/generated/codefabric.*.v1.rs`,
  and the `contracts/` tree with Gate A green.
- `rustc-extractor/` and `pyrefly-sidecar/` are Wave 0 argument-parsing stubs whose
  `--serve` is a documented no-op; their `.proto` contracts are fully generated.

The following are **absent** and are this plan's work: every parser dependency
(tree-sitter, tree-sitter-python, tree-sitter-rust, Ruff component crates), any watcher
dependency (`notify`, `notify-debouncer-full`), the four FAB §17–§20 source/syntax
tables, a Rust `AnalysisContext` runtime type, any provider executor/adapter, the golden
corpus root `tests/golden/`, the query/PlanSpec compiler, the accepted-handle service,
the artifact store, the freshness machine, the invalidation graph, the comparator, and
all Git acceleration beyond Wave 2 topology discovery.

Standing carry-over: Ubuntu clean-checkout evidence remains **user-deferred assurance**
(remediation plan WP09/M04) and is not a blocker or gate of this plan (I-27).

### 1.3 Non-goals

- Python and Rust semantic profiles beyond the Wave 5 slices (`PYTHON_SEMANTIC_V1`,
  `RUST_SEMANTIC_V1` are Waves 8–11); Pyrefly sidecar activation (W9).
- Full reconciliation breadth, contexts, and unknown-remainder closure (W12); derived
  flow/effects/summaries (W13–W14); formal Gate C closure (W14; `RM §11` keeps it open).
- The full eight-form query language, all response conformance (W15–W16); production
  RPC security, credentials, ACLs, multi-agent serving (W17); FastMCP tools (W18);
  Gates D–G (W19).
- Git history ontology, checkout, fetch, credentials, hooks, or any Git mutation
  (`LIFE §§81–87`, permanent).
- Notebook source units (D-37), the optional durable overlay journal (`LIFE §108.2`,
  D-40), orjson, cargo-vet, license selection, performance SLO acceptance.
- A new Cargo package or workspace root for any of this work (repo-spec §0.3; D-26).
- Hand-authored sibling schemas or registries beside generated contracts (I-11).

---

## 2. Source design and declared inputs

The roadmap `RM §§9–12` fixes wave scope; the six 1.3 specifications and the suite
manifest are normative owners (`RM §0`). The `docs/spec_index/` layer was used for
navigation only and is not an input; every load-bearing citation below was re-verified
against the current spec text this session, because the specs were revised in place
during Wave 0–3 execution (several digests differ from the v5 plan's table).

| Path | SHA-256 |
|---|---|
| docs/upfront_design/codefabric_1.3_implementation_roadmap_v1.0.md | 70ee70eb30639789e642024b277286fc4667ee5f6ed6ac988ff7e12d0c207ce0 |
| docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md | 81a63c4ffddd5684ce7462e4f92c932f969db0fed27b95d94427df078bf299be |
| docs/upfront_design/code_property_graph_present_state_fact_ontology_specification_v1.3.md | a8ba008a94c72c55b48b91fd8967eafe6f46419f0fe5f65d1bcb3a874d79adab |
| docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md | 5bd5993cbad8e83d0ca27cf34505d6cf079528af4eef1ac1c8d307b564ffe214 |
| docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md | 708fb7210f0408c55103e42cccb37171c82eecfe1f203486876947ef09976e9a |
| docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md | b5c85a00a4a885867b12e6ee2c8f2ebb679d3587dc366de831a89ebb38fa8866 |
| docs/upfront_design/code_property_graph_semantic_query_specification_v1.3.md | 6512e715c118fd7c72dbdc2d4ccce6828fafb6526f7e6db540f5ab587eb33847 |
| docs/upfront_design/present_state_cpg_fastmcp_serving_specification_v1.3.md | d1fa709d4ab9c9ede3d1bbfbb70d7e2494eaf2f7ebe112c7142a2393c2d21633 |
| docs/rust_core_python_interface_repository_specification_2026-08-20.md | 42678a93d6c323d3c527255c2f266b2520bd13dca485c1f1d4af7991a9243848 |
| docs/library_ref/semantic_design_principles_holistic.md | bb0f28e54f701aa932cddb59fe5d9464b304ed59443f0280377e8c4d9a9d1892 |
| docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md | 2d33c1373ec9296b78bfe1bcf677eef3a97745b4677e74f06de0bf9d03392e92 |
| docs/plans/state/codefabric-waves-0-3-foundation_v5_state.json | e4aa8affdaaaa4d9210de398112428b3ebe7e9c3e05416fa54b33f9d76f063f1 |
| docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json | 3c0c8c1d0fca9877903cb585f4e92e590d94c74ab381d008d3ea10a2f1c72e53 |

### 2.1 Precedence

1. Normative owner in the current 1.3 design suite (`SUITE AC-G-01` ownership map).
2. This plan's recorded decisions (§5) and specification-issue dispositions (§6.2).
3. This plan's packet contracts.
4. Current repository implementation and pinned-library behavior.
5. Roadmap prose where it paraphrases a spec (`RM §0` makes the specs prevail — e.g.
   the extractor manifest event names come from `GEN AC-G-31`, not `RM §10`).

Current repository and verified library behavior remain higher authority than any
illustrative mechanics in this plan (`evidence-policy.md §4`).

---

## 3. Baseline and implementation disposition

Baseline HEAD is 63c1e9625f6c852ab918467b03cf16ee69660ab7 with a clean tree. The cached
gate baseline is green but stale relative to HEAD (session context); the executor
re-runs `just ci-fast` at execution preflight and records pre-existing failures in
state before any edit (repo-spec §59.1; `validation-policy.md §3`).

| Scope | Disposition |
|---|---|
| Waves 0–3 packets, M00–M04 | Complete; consumed as stable interfaces, re-proved only by standing gates. |
| Remediation plan WP09 / M04 (Ubuntu) | User-deferred assurance; recorded, never a blocker (I-27). |
| `SyntheticCanonicalIngest` | Bounded v5 D-07/L-06 transition; replaced by the production engine at WP36; DB07 proves the cutover. |
| Extractor/sidecar `--serve` stubs | Extractor stub replaced at WP35; sidecar stub remains (W9 scope). |
| `EmptySnapshotOverlay` | Not legacy — it is the legitimate empty-overlay case; production non-empty overlay providers arrive at WP40/WP46 beside it. |
| Wave-5 minimal freshness plumbing | Bounded transition inside this plan; the formal AC-G-24 machine replaces it at WP45; DB08 proves the cutover. |

No execution state is created by this planning task. The executor initializes the
schema-2 state at the frontmatter path.

---

## 4. Global target invariants

I-01 through I-28 of the v5 plan remain binding verbatim; the ones this plan's packets
exercise most are restated with the additions I-29–I-34:

- I-01: `workspace_id` identifies exactly one authorized analyzed source instance.
- I-02: one immutable leased `ServingSnapshot` is the only query pin; later activations
  never affect an existing lease.
- I-03: current stable filesystem bytes are present-state authority; CodeFabric BLAKE3
  identity is canonical — never Git OIDs, watcher events, or prior provider output.
- I-04: provider observations are not canonical until reconciled; providers and
  encoders never write canonical tables, overlays, or capability state directly.
- I-05: every fact row is scoped by workspace and analysis context; exact facts never
  merge across either boundary (`GEN §13.5`).
- I-06: absence is never proof of absence; unknowns, capability gaps, and explicit
  negatives are registry-defined; compile failure yields capability gaps, not
  stale-current compiler facts.
- I-07: Rust owns semantics, planning, storage, snapshots, and canonical bytes; Python
  remains a thin adapter (untouched by this plan until W18).
- I-08: every compatibility-sensitive artifact is versioned and fingerprinted;
  unchanged-source regeneration is deterministic.
- I-09: incremental and overlay results converge to the corresponding clean durable
  state (`LIFE §157` invariant 32; mechanized by AC-G-79 at WP48).
- I-10: compiler, Pyrefly, gix, delta-rs, tree-sitter, and Ruff internals do not cross
  application-owned boundaries (`GEN §7`; governance rules).
- I-11: contracts machine sources and Contract IR are the sole generation authority.
- I-13: registry codes are append-only; orthogonal state dimensions stay distinct.
- I-14: storage mutation is idempotent and generation-fenced.
- I-15: the ontology contains facts and mechanically derived facts only — never
  evaluative classifications (`ONT §4.7`, `ONT §67`).
- I-24: normative fixtures and golden expected values are independently reviewed and
  cannot be approved or overwritten by their own generators (`SUITE AC-G-05`).
- I-25: complete packets have a non-null ancestor proving commit; state stores
  judgment only.
- I-29 (new): every syntax fact preserves both the provider-native raw kind and the
  normalized kind; no provider-native node kind may become unrepresentable
  (`ONT §4.2`, `ONT §6.2`, `GEN §12`, `GEN §96`).
- I-30 (new): syntax occurrence ≠ semantic entity; call syntax is not a callable; a
  provider-local ID is never canonical identity (`ONT §4.3–§4.4`, `GEN §96`,
  `GEN §13`).
- I-31 (new): freshness is determined by admitted event sequence, reconciliation
  sequence, and generations — never elapsed time (`LIFE AC-G-24`); invalidated
  semantic facts are hidden before current syntax is published (`LIFE §157` inv. 4).
- I-32 (new): Git data accelerates candidate discovery only; every Git-derived
  candidate set is fenced by its captured `GitStateVector` and verified against
  current bytes before influencing source state; gix failure degrades acceleration,
  never correctness (`LIFE §§54–55`, `§73`, `§80`, `§157` inv. 12–18).
- I-33 (new): a lost hot overlay may cause durable lag after restart but never a false
  current semantic claim (`LIFE §108.3`); a partial stream is not a successful logical
  response until its terminal completeness record is emitted (`SUITE §0.2` inv. 8).
- I-34 (new): the fast syntax lane may publish syntax-current snapshots before
  semantics, and such snapshots mark pending/unavailable capabilities explicitly and
  never present stale invalidated facts as current (`LIFE §94.3`, `§99.1`).

Doctrine disposition: Principles 6, 8, 11, 12, 13, 16, 17, 23, 24, 25, 27 advance
(provider ports/adapters, trust boundaries, boundary parsing, typed states, stable
identity, contracts, functional reconciliation core, failure taxonomy, idempotent
recovery, incremental reproducibility, provenance); Principles 5, 7, 10, 19–22, 29–31
are maintained; the anti-principles most at risk — durable identity from provider-local
order, untracked side-writes outside the mutation model — are guarded by I-30, I-04,
and the governance rules named per packet. Unrelated principles: N/A.

### 4.1 Cross-packet obligations (`RM §2.4`)

Every packet updates, in the same packet where applicable: requirement/traceability
records for touched contracts, golden fixtures and canonical outputs, fault points
(`contracts/faults/`), security cases, observability metrics, comparison coverage, and
upgrade/bundle records. A contract-source edit always runs generation, verification,
and reproduction checks before packet completion. Owner-reviewed normative content is
accepted before generated code consumes it (I-24). Packet completion requires every
named acceptance check green at the proving commit and at HEAD.

---

## 5. Governing decisions and library basis

D-01–D-22 and LD-01–LD-20 of the v5 plan remain in force. New decisions:

### 5.1 Decisions

- **D-23 — Integrated plan, wave-segmented execution.** One plan covers Waves 4–7 by
  user direction; execution loads one wave at a time; per-wave packet sizing follows
  `RM §2.1`; each wave exits through its milestone before the next wave's packets
  start (except declared golden-corpus prework edges in §15).
- **D-24 — Global ID continuation.** Packets WP27–WP53, milestones M05–M08, batches
  from DB07 continue the v5 sequence so packet proof-oracle test names stay unique
  repository-wide.
- **D-25 — `fact-generation` build capability.** The parser/provider stack enters the
  stable root behind one new additive feature `fact-generation` (composed into
  `daemon`, hence `local-workstation`), keeping narrow graphs parser-free. FAB §2.1's
  canonical feature map does not yet name it — SI-02.
- **D-26 — `codefabric-git-state` is a module boundary, not a crate.** `LIFE §37`
  says "new crate"; the governing repository specification (§0.3, AGENTS invariant 8)
  and FAB §2.1's single-stable-root feature model prevail. The subsystem remains the
  isolated `git_state` module behind `repository-state`, enforced by the existing
  `gix-boundary-only`/`gix-read-only` rules. Recorded deviation; SI-08 asks LIFE to
  align its wording.
- **D-27 — Wave-5 minimal MIR subset.** GEN defines full families, no minimal set.
  Wave 5 extracts, for one body: `GEN §38` node families (`MIR_BODY`, `MIR_LOCAL`,
  `MIR_BASIC_BLOCK`, `MIR_STATEMENT`, `MIR_TERMINATOR` + operands/places as needed),
  `GEN §39.1` terminator→edge mapping, and `GEN §41` direct-call emission
  (`CALLS_DECLARATION`/`CALLS_EXACT_TARGET` plus one `CALLS_UNKNOWN` for the golden
  unknown fact). Deferred: `GEN §39.2` statement-level CFG expansion (explicitly
  query-time-permitted), `GEN §41.3–§41.5` fn-pointer/closure/monomorphization
  breadth, `GEN §82` type reconciliation.
- **D-28 — Parse-error/missing-syntax dual surface.** One adapter fact derives all
  three representations: the canonical `PARSE_ERROR`/`MISSING_SYNTAX` entity, the
  `syntax_detail` `error`/`missing` flags, and the `source_annotation` diagnostic row
  (`FAB §19`–`§20` both carry them; neither spec states the rule). `MISSING_SYNTAX`
  originates from Tree-sitter only — never synthesized from Ruff absence (`GEN §15`).
- **D-29 — Enum-registry authority for `FreshnessState`.** The generated enum registry
  (`ONT §62.7` via `SUITE AC-G-06`) is authoritative. `LIFE §25`'s `AWAITING_CURRENT`
  divergence is SI-06; if the AC-G-24 machine requires it, WP45 appends it through the
  append-only registry discipline rather than forking vocabularies.
- **D-30 — Comparator staging.** Gate B does not list the comparator, but
  `SUITE AC-G-79 §79.7` binds golden scenarios. Wave 5 scenarios therefore end with a
  bounded state-digest equality check (canonical-table digests via the existing
  `effective_state_digest` mechanism); the complete AC-G-79 comparator — ignore
  registry semantics, canonical row encoding, difference artifacts, the corrupted
  -overlay and ignored-field conformance cases — lands at WP48 and re-runs the Wave 5
  scenarios. Recorded staging decision.
- **D-31 — Periodic generic audit defaults.** `LIFE §55.1` fixes no cadence or
  divergence action. Plan default: audit after every bulk reconcile and on a
  configurable idle interval; divergence forces an authoritative rescan and sets
  `GIT_DEGRADED` until reconverged. SI-11 returns the gap to LIFE.
- **D-32 — Stabilization tuple hardening.** `LIFE §62` omits the inclusion-policy and
  attributes fingerprints that `GitStateVector` (`LIFE §50`) carries; WP52 includes
  them in the tuple conservatively (a mid-switch `.gitignore` change destabilizes the
  bulk wave). Deviation-as-improvement; SI-09.
- **D-33 — Git DTO naming and path forms.** `GitCandidateDelta` (LIFE §156/App F) is
  canonical; `LIFE §70`'s `GitStateDelta` is read as editorial. `LIFE §43`'s richer
  path representation prevails over Appendix F's lossy restatement. Types LIFE names
  but never defines (`HeadKind`, `GitPathChangeClass`, `GitRenameEvidence`, …) are
  authored as application DTOs inside the `git_state` boundary. SI-10.
- **D-34 — gix pin form.** Exact `=0.86.0` (LIFE §2.2/§39/App B/App E) with §78's
  "≥ 0.86.0" recorded as the security floor inside upgrade gates; SI-08.
- **D-35 — Watcher pin.** `notify-debouncer-full = 0.7.0` / `notify = 8.2.0` from the
  version-anchored library reference — the only grounded anchor; no spec pins it
  (SI-12). Pinned exactly; 0.8.0 release candidates are explicitly not targeted.
- **D-36 — tree-sitter-rust version decision at WP27.** No spec pins the Rust grammar
  (SI-01). WP27 selects the newest grammar release compatible with tree-sitter core
  0.26.12 (ABI-15 range probe), records it in the toolchain bundle
  (`SUITE AC-G-07` `recorded_provider_pins`), and treats later movement as a provider
  upgrade with fixture gates.
- **D-37 — Notebooks out of scope.** `GEN §15` mentions notebook source units, but
  `AC-G-43` has no notebook mapping rule and `AC-G-36` no capability code; Waves 4–7
  classify notebooks `UNSUPPORTED` with explicit capability records. SI-14.
- **D-38 — No petgraph adoption yet.** Wave 5's registered derivation is the
  `SYNTAX_TREE_V1` projection ("syntax occurrences + ordered AST-child edges", the
  cheapest AC-G-42 row), computed from canonical rows without new dependencies;
  petgraph adoption belongs to W13 with its own LD.
- **D-39 — Raw-kind registry home.** `ONT AC-G-70` requires provider-native raw kinds
  in separate provider registries, but `SUITE` Part IV enumerates no such path. WP28
  adds `contracts/registry/provider-raw-kinds/<provider>.yaml` through the existing
  catalog/derivation-unit machinery (I-11, I-20). SI-03 asks SUITE to adopt the path.
- **D-40 — Local-profile crash durability.** Wave 6 implements the no-journal local
  policy (`LIFE §108.1` + `§108.3`): rebuild lost overlays from source, never a false
  current claim; the optional §108.2 journal is out of scope.
- **D-41 — Wave-5 freshness shim is bounded.** WP38/WP39 accept the structured
  freshness field and serve `REQUIRE_CURRENT_FOR_TARGETS` only in the
  quiescent-workspace regime (source current by construction after bootstrap); the
  formal barrier machine replaces this at WP45; DB08 proves no shim survives.

### 5.2 Library decisions

- **LD-21 — tree-sitter core + Python grammar: adopt.**
  Version basis: `tree-sitter =0.26.12`, `tree-sitter-python =0.25.0` (`GEN §2`).
  Displaces: nothing (new capability). Risk: grammar/ABI drift changes node kinds —
  mitigated by exact pins, raw-kind registries (D-39), and fixture corpora.
  Validation: compile probe + grammar `LANGUAGE` version assertion + WP30 CST
  fixtures. Consult `docs/library_ref/tree_sitter_rust_python.md` before coding.
- **LD-22 — tree-sitter-rust: adopt, version chosen at WP27 (D-36).**
  Validation: ABI-compatibility probe against core 0.26.12 + malformed-input corpus;
  recorded in `contracts/bundles/toolchain`.
- **LD-23 — Ruff component crates: adopt.** Version basis: `=0.0.7` component crates
  of Ruff 0.16.1 (`GEN §2`): `ruff_python_ast`, `ruff_python_parser`,
  `ruff_python_trivia`, `ruff_text_size`, plus siblings the reference names for
  line-index/comment ranges if required (executor confirms the exact crate set against
  `docs/library_ref/ruff_python_crates_advanced_reference_2026-08-18.md`).
  Risk: 0.0.x API instability — exact pins; upgrades are provider upgrades with
  fixture gates. Validation: parse/token/trivia probe + WP31 fixtures.
- **LD-24 — notify-debouncer-full: adopt (Wave 6).** Version basis:
  `notify-debouncer-full =0.7.0`, `notify =8.2.0` (D-35). Displaces: nothing.
  Risk: platform backend semantics differ — mitigated by `LIFE §139` assert-final-
  state tests, PollWatcher fallback, and the rescan fence. Validation: WP41 real-FS
  debounce/overflow integration tests. Consult `docs/library_ref/notify_debouncer_full_rust_reference.md`.
- **LD-25 — gix profile completion.** Retain `=0.86.0` (LD-06) and complete the
  `LIFE §39`/Appendix E feature profile (`sha1, index, status, attributes, excludes,
  dirwalk, blob-diff, interrupt, parallel, auto-chain-error, tracing`,
  `default-features = false`). Open obligation from `LIFE §39`: confirm the resolved
  transitive feature graph before finalizing the manifest — WP49 acceptance.
- **LD-26 — No new query/graph engines.** DataFusion/Arrow (LD-01/LD-02) remain the
  only query substrate for Waves 4–7; petgraph deferred (D-38); orjson stays absent.

No library decision authorizes a new crate root or a second semantic implementation.

---

## 6. Legacy disposition and specification issues

### 6.1 Legacy dispositions

- **L-07 — `SyntheticCanonicalIngest` body.** v5 L-06's bounded transition ends when
  WP36 lands the production `ReconciliationEngine` behind the same signature. DB07
  proves zero production references; the synthetic observation fixtures remain as
  *inputs* to the production engine's tests, never as an alternate canonicalization
  path.
- **L-08 — Extractor `--serve` protocol-silence stub.** Replaced by the real
  AC-G-31 slice at WP35; packet-local negative check proves the no-op path is gone.
  The sidecar's equivalent stub is out of scope (W9).
- **L-09 — Wave-5 freshness shim.** Bounded by D-41; DB08 proves zero state after
  WP45.
- **L-10 — Wave-3 `wave3-integration-check` scope.** Retained as-is; the new per-wave
  integration recipes extend, never replace, the standing gates.

No silent compatibility aliases, dual writes, or duplicate authorities are authorized
anywhere in this plan beyond these bounded transitions.

### 6.2 Specification issues to file (RM §28: ambiguity returns to the owning spec)

Filed as design issues against the owning documents at execution start; none blocks a
packet — each has a plan disposition above.

| ID | Owner | Issue | Plan disposition |
|---|---|---|---|
| SI-01 | GEN §2 / SUITE AC-G-07 | `tree-sitter-rust` never pinned, and AC-G-07's `recorded_provider_pins` enumeration omits it — the filing extends both | D-36 |
| SI-02 | FAB §2.1 | no fact-generation feature/parser deps in the canonical feature map; gix pin lives only in LIFE | D-25 |
| SI-03 | SUITE Part IV | no raw-kind registry path for ONT AC-G-70 | D-39 |
| SI-04 | FAB §18/§19 | `source_token`/`source_annotation` grain without declared PK | WP28 declares `token_id`/`annotation_id` PKs in the Contract IR |
| SI-05 | RM §10 | `OWNER_COMPLETE` is not a spec token (AC-G-31 names `OwnerEnd` etc.) | spec names used throughout |
| SI-06 | LIFE §25 vs ONT §62.7 | `FreshnessState` value sets diverge (`AWAITING_CURRENT`) | D-29 |
| SI-07 | LIFE §30 vs §18/AC-G-25 | `RECONCILE_REQUESTED` is not a lifecycle state | implemented as a coordinator flag, not a state |
| SI-08 | LIFE §2.2 vs §78; LIFE §37 | `=0.86.0` vs `>= 0.86.0`; "new crate" wording | D-34, D-26 |
| SI-09 | LIFE §62 | stabilization tuple omits inclusion/attributes fingerprints | D-32 |
| SI-10 | LIFE §70 / App F | `GitStateDelta` naming collision; App F lossy path DTOs | D-33 |
| SI-11 | LIFE §55.1 | periodic audit cadence/divergence action unspecified | D-31 |
| SI-12 | LIFE §2.2/§155 | no `notify-debouncer-full` pin anywhere | D-35 |
| SI-13 | QRY §47 vs SRV AC-G-65 | overlapping error names differ | SRV AC-G-65 wins (`SUITE AC-G-01`); WP38/WP39 use SRV names |
| SI-14 | GEN §15 / AC-G-43 / AC-G-36 | notebook sources unmapped | D-37 |

---

## 7. Work packets — Wave 4 (Source and syntax fact generation)

### WP27 — Fact-generation build capability and provider pins

#### Outcome

The stable root gains one additive `fact-generation` feature carrying
`tree-sitter =0.26.12`, `tree-sitter-python =0.25.0`, `tree-sitter-rust =<D-36 choice>`,
and the exact `=0.0.7` Ruff component crates (LD-21/22/23), composed into `daemon` →
`local-workstation`; every narrow feature graph (`canonical-json`,
`contracts-tooling`, `proto-tooling`, featureless) stays parser-free;
`scripts/stable_graph_check.sh` proves the new activation boundary and single-version
families; `deny.toml` duplicate-family policy reviewed for the new transitive trees;
the toolchain bundle records the provider pins (`SUITE AC-G-07`
`recorded_provider_pins`).

#### Dependencies

None (first packet; M04 substrate).

#### Target Invariants

I-08, I-10, I-16 (v5), D-25, D-36. Doctrine P6, P8 maintained.

#### Design and Library References

`GEN §2 Source basis and version anchors`; `FAB §2.1 Canonical workspace baseline`
(feature-composition discipline); `SUITE AC-G-07`; LD-21, LD-22, LD-23; SI-01, SI-02.
Library refs: `tree_sitter_rust_python.md`, `ruff_python_crates_advanced_reference_2026-08-18.md`.

#### Change Surface

##### Preflight Query

```bash
rg -n 'tree.sitter|ruff_' Cargo.toml Cargo.lock            # expect zero before
just stable-graph-check                                     # baseline verdict
cargo search tree-sitter-rust --limit 5                     # D-36 candidate set
rg -n 'recorded_provider_pins' contracts/bundles/ -g '*.yaml' -g '*.json'
```

##### Known Touch (verified this session)

`Cargo.toml` (`[features]`, dependency table), `Cargo.lock`,
`scripts/stable_graph_check.sh`, `deny.toml`, `contracts/bundles/toolchain/`.

#### Required Changes

Add exact-pinned optional dependencies behind `fact-generation`; wire the feature into
`daemon`; extend the resolved-graph validator (activation boundary: parser crates
absent from every non-`fact-generation` graph; exactly one version per new family);
record pins in the toolchain bundle through the contracts machinery (mutating recipe,
diff-reviewed); execute the D-36 grammar choice with an ABI probe
(`tree_sitter::LANGUAGE_VERSION` compatibility assertion) recorded as evidence.

#### Legacy Disposition and Decommission

None — additive. No parser code lands in this packet.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp27_behavioral_acceptance`.

compile-level probes — parse one trivial
Python and Rust source through each grammar, one Ruff parse/token probe, asserting
grammar ABI versions match the pinned core.

##### Structural

Executable oracle: `wp27_structural_acceptance`.

`just stable-graph-check` — extended assertions green; `just features-each` green;
`just features-no-default` green.

##### Negative / Zero-State

Executable oracle: `wp27_negative_zero_state`.

`cargo tree` probes prove parser crates absent from
`canonical-json`, `contracts-tooling`, `proto-tooling`, and featureless graphs
(asserted inside `stable_graph_check.sh`).

##### Operational

Executable oracle: `wp27_operational_acceptance`.

`just deps-fast`, `just policy`, `just duplicate-family-check` — exit 0.

#### Edit-Local Gates

`just root-check` on the touched feature graphs.

#### Packet-Local Gates

`just ci-fast`; `just stable-graph-check`; `just features-each`.

#### Integration Milestone

M05.

#### Replan Triggers

No tree-sitter-rust release is ABI-compatible with core 0.26.12 → plan revision
(grammar vendoring decision needs design input). Duplicate-family conflicts that
cannot be resolved with exact transitive skips → plan revision.

#### Rollback or Recovery

Revert the manifest/lock/validator edits; additive.

### WP28 — Source/syntax schema and registry contract extension

#### Outcome

The schema Contract IR and registries gain the Wave-4 contract surface, regenerated
through the Wave-1 machinery: four new TableSpecs — `source_file` (`FAB §17`),
`source_token` (`FAB §18`), `source_annotation` (`FAB §19`), `syntax_detail`
(`FAB §20`) — with `FAB §7` physical types, declared PKs (`token_id`/`annotation_id`
per SI-04), `owner_bucket` partitioning (`FAB §95.2`), mutation axes
`OWNER_REPLACED_FACT`/`OWNER_REPLACE`/`DURABLE_EFFECTIVE`/`PINNED_DATA`, and staged
`cpg_serving` views (`FAB §92`: absent views are never placeholder schemas); registry
appends (append-only, `SUITE AC-G-06`): lexical/syntax entity kinds and token
specializations (`ONT §5.1`, `§6.1` 24-kind set), annotation kinds, normalized
field-role codes (`GEN §16.2` names), and the new provider raw-kind registries
(D-39: `contracts/registry/provider-raw-kinds/tree-sitter-python.yaml`,
`tree-sitter-rust.yaml`, `ruff-python.yaml`); a Rust `AnalysisContext` runtime model
matching `contracts/schema/analysis-context.schema.json` with the reserved
`context:source` (`GEN §3.3`); regenerated Rust/Python outputs with zero drift.

#### Dependencies

WP27.

#### Target Invariants

I-08, I-11, I-13, I-29; `ONT AC-G-70` ("provider-native raw kinds remain in separate
provider registries and do not consume canonical ontology codes"). Doctrine P10, P12,
P29 advance.

#### Design and Library References

`FAB §7`, `§9`–`§11` (TableSpec axes + round-trip gate §11.1), `§17`–`§20`, `§92`,
`§95.2`; `ONT §5`–`§6`, `§62`–`§64`, `AC-G-70`; `GEN §16.2`; D-28, D-39; SI-03, SI-04.

#### Change Surface

##### Preflight Query

```bash
rg -n 'table_code' src/generated/table_specs.rs | tail -5   # current top code (901)
ast-grep run -l rust -p 'pub struct AnalysisContext' src/    # expect zero before
rg -n 'source_file|syntax_detail' contracts/schema/arrow-delta/ contracts/registry/
just contracts-verify                                        # baseline green
```

##### Known Touch (verified this session)

Contract IR sources under `contracts/` (schema Contract IR, registries), regenerated
`src/generated/table_specs.rs`, `src/generated/registries.rs`,
`contracts/generated/registry/*.json`, `contracts/schema/arrow-delta/table-specs.json`,
adapter generated outputs (`codefabric-cpg-mcp/.../contracts/`), new `src` module for
the `AnalysisContext` model.

#### Required Changes

Author the IR/registry source edits; run `just contracts-gen` and
`just adapter-contracts-gen` (confirm-gated; diff inspected); implement the
`AnalysisContext` runtime type (validated construction, `context_fingerprint`
verification, `IdentityDomain::AnalysisContext` encoding); extend
`fabric::bootstrap_workspace` table creation to the new specs; keep `cpg_python` /
`cpg_rust` / `cpg_derived` namespaces empty (`FAB §92`).

#### Legacy Disposition and Decommission

None removed; registry changes are append-only (I-13).

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp28_behavioral_acceptance`.

the four new TableSpecs pass
`validate_delta_compatibility` and the `FAB §11.1` Arrow→Delta→DataFusion→Arrow
round-trip on synthetic rows; `AnalysisContext` round-trips against its JSON Schema.

##### Structural

Executable oracle: `wp28_structural_acceptance`.

every new registry code is append-only above prior
maxima; raw-kind registries contain zero canonical ontology codes; new TableSpecs
declare PKs and all four mutation axes.

##### Negative / Zero-State

Executable oracle: `wp28_negative_zero_state`.

no hand-written sibling of any generated output
(`just contracts-repro-check`, `just adapter-contracts-repro-check` prove
reproduction); `cpg_serving` exposes no placeholder view for absent families.

##### Operational

Executable oracle: `wp28_operational_acceptance`.

`just contracts-verify`, `just contracts-verify-released`, `just schema-check`,
`just compilation-units-check`, `just fixture-check` — exit 0.

#### Edit-Local Gates

`just contracts-tooling-lint`; targeted registry-model tests.

#### Packet-Local Gates

`just ci-fast`; `just governance`.

#### Integration Milestone

M05.

#### Replan Triggers

The Contract IR cannot express a `FAB §17`–`§20` column shape → plan revision (IR
extension is Wave-1 machinery). Released-profile verification reports draft warnings
that cannot be closed inside the packet → plan revision.

#### Rollback or Recovery

Regenerate from the prior IR state; generated outputs carry derivation units (I-20).

### WP29 — Provider job runtime (GEN AC-G-32)

#### Outcome

The production asynchronous provider execution interface: `ProviderExecutor::submit`
returning `AcceptedProviderJob { run_id, accepted_generation, events }` before
permits; admission control with global/workspace/context permits; supersession keyed
by `(workspace, context, provider, scope, capability family)` marking queued work
`SUPERSEDED` and cancelling or stale-rejecting running work; cooperative cancellation
with the 2 s in-process acknowledgement deadline; placement per `GEN AC-G-32` —
parsing on bounded dedicated Rayon pools, I/O/protocol/scheduling on Tokio; the
`provider_run` operational table populated through its generated state machine
(`ProviderRunState`, `registries::generated_transition`); the wire→`ProviderEvent`
mapping consumed from `contracts/rpc/feature-registry.yaml`; observation streams
delivered into the existing `fact_ingest` bounded channels with workspace/context/
generation fences intact. Providers cannot mutate canonical tables, overlays, or
capability state (I-04) — enforced by module privacy plus a governance rule.

#### Dependencies

WP27, WP28.

#### Target Invariants

I-04, I-05, I-14; `LIFE §22` provider-run states; `GEN §86` generation output
boundary. Doctrine P6, P16, P22 advance.

#### Design and Library References

`GEN AC-G-32`, `§10`–`§11`, `§86`, `§90 Provider job interfaces`; `FAB §63`;
`LIFE §111`–`§114` (priorities, admission, thread budget, generation rule);
generated `codefabric.provider.v1` DTOs (session-verified present and unused).

#### Change Surface

##### Preflight Query

```bash
ast-grep run -l rust -p 'trait ProviderExecutor' src/        # expect zero before
rg -n 'ProviderJobSpec|AcceptedProviderJob' src/ --type rust  # generated-only before
rg -n 'provider_run' src/operational_store.rs src/generated/  # DDL + states exist
rg -n 'wire_event|application_event' contracts/rpc/feature-registry.yaml
```

##### Known Touch (verified this session)

New provider-runtime module(s) in `src/` (decomposition is executor's choice within
repo-spec §1 scope boundary), `src/fact_ingest.rs` (stream intake seam),
`src/operational_store.rs` consumers, `rules/` (+ one new rule with `rule-tests/`).

#### Required Changes

Implement executor/admission/supersession/cancellation; bind generated DTOs to an
application `ProviderEvent` per the feature-registry mapping (provider-specific event
enums prohibited at the boundary, `GEN AC-G-36`); populate `provider_run` rows with
input/output fingerprints and diagnostics; add governance rule
`provider-observation-boundary-only` (providers reach storage only through the
observation ingress) with negative fixtures.

#### Legacy Disposition and Decommission

None; the synthetic ingress remains the canonicalization body until WP36 (L-07).

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp29_behavioral_acceptance`.

submit→accept→stream→terminal for a fake in-process
provider; supersession by newer generation; cancellation acknowledged within
deadline; stale-generation output rejected with `STALE_RESULT`.

##### Structural

Executable oracle: `wp29_structural_acceptance`.

every `provider_run` transition flows through the
generated transition table; illegal transition raises `STATE_TRANSITION_VIOLATION`.

##### Negative / Zero-State

Executable oracle: `wp29_negative_zero_state`.

no path from a provider adapter to Delta writers or
overlay mutation (module privacy + `provider-observation-boundary-only` rule green
with tested fixtures).

##### Operational

Executable oracle: `wp29_operational_acceptance`.

admission/permit/supersession/cancellation counters
emitted; bounded-channel backpressure observable under a slow consumer.

#### Edit-Local Gates

Targeted nextest slice on the new module; `just governance-scan`.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on the supersession/admission decision logic.

#### Integration Milestone

M05.

#### Replan Triggers

The generated provider.v1 DTO set cannot carry a required job field → contract change
via Wave-1 machinery (plan revision if a protocol package must change shape).

#### Rollback or Recovery

Additive runtime; disable by not constructing the executor.

### WP30 — Tree-sitter Python and Rust adapters

#### Outcome

Two in-process Tree-sitter provider adapters emitting application-owned
`RawSyntaxFact` streams (`GEN §7.1` recommended shape: raw kind, normalized kind,
span, named/extra/error/missing flags, parent, grammar field name, ordinal) for the
complete CST including anonymous tokens and parser recovery (`GEN §15.1`); Rust kinds
per `GEN §35`; no `Node<'tree>` or other tree-sitter type escapes the adapter
(governance rule `tree-sitter-boundary-only`, mirroring the gix rules); incremental
parse capability (edit application + changed ranges) proven equivalent to a clean
full parse on the fixture corpus (`GEN §94`), with no watcher driver (deferred to
Wave 6 per `RM §9`); malformed input never panics — it produces error/missing facts
or an explicit capability outcome.

#### Dependencies

WP27, WP28 (normalized-kind and raw-kind registries). From WP29 only the accepted
job-contract surface (DTO/event shapes); the live executor integration lands in
WP32/WP33, so WP30 may run parallel to WP29.

#### Target Invariants

I-10, I-29, I-30; `GEN §96` ("raw provider kind remains recoverable",
"provider-local ID is never canonical identity"). Doctrine P6, P11 advance.

#### Design and Library References

`GEN §4.1`, `§5.1`–`§5.2` (authority), `§7.1`, `§15.1`, `§35`, `§94`; `ONT §6.2`
required node properties; LD-21/LD-22. Library ref: `tree_sitter_rust_python.md`
(mandatory pre-read; APIs are version-sensitive).

#### Change Surface

##### Preflight Query

```bash
rg -n 'tree_sitter' src/ --type rust        # expect only the new adapter modules
ast-grep outline src --items exports --match 'Provider'    # runtime surface from WP29
just fixture-check                          # fixture harness baseline
```

##### Known Touch (verified this session)

New adapter module(s) behind `fact-generation`; `rules/` + `rule-tests/` (one new
rule); fixture corpus additions under `contracts/fixtures/` (adapter-level fixtures;
the golden workspace corpus is WP34).

#### Required Changes

Grammar registration, parse-tree walk producing `RawSyntaxFact` rows with raw kinds
recorded verbatim against the D-39 raw-kind registries; normalized-kind mapping from
the generated registries; error/missing/extra flags; byte-range validation against
`SourceImage` bounds; incremental `InputEdit` application + changed-range surfacing;
Python/Rust valid, partially invalid, and malformed fixture families with expected
observation outputs.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp30_behavioral_acceptance`.

fixture families produce exact expected observation
rows (valid/invalid/malformed × Python/Rust); every raw provider kind representable
(unknown raw kind is recorded, never dropped — I-29).

##### Structural

Executable oracle: `wp30_structural_acceptance`.

incremental parse ≡ clean parse over the edit corpus
(`GEN §94`); spans within bounds with `start <= end`.

##### Negative / Zero-State

Executable oracle: `wp30_negative_zero_state`.

`tree-sitter-boundary-only` rule green (no tree-sitter
type outside the adapter, tested fixtures incl. near-misses); malformed inputs yield
no panic (fixture-driven).

##### Operational

Executable oracle: `wp30_operational_acceptance`.

parse duration/node-count/error-count metrics per run.

#### Edit-Local Gates

Targeted nextest on adapter modules.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on the kind-normalization mapping.

#### Integration Milestone

M05.

#### Replan Triggers

Grammar node inventory diverges from the registry assumptions (raw kinds
unmappable) → implementation adaptation via registry append; a grammar cannot
round-trip byte ranges for multi-byte encodings → plan revision.

#### Rollback or Recovery

Additive; feature-gated.

### WP31 — Ruff syntax/lexical adapter

#### Outcome

The in-process Ruff provider adapter (`GEN §7.2`) emitting the Wave-4 members of
`PythonFrontendBatch` — source, tokens, syntax, comments, directives, parse
diagnostics — as CPG-owned records: per-fact rules of `GEN §15` (tokens with raw
kind/ordinal/spelling policy, comments own-line/end-of-line, docstrings linked to
owners, `noqa`/`type: ignore`/formatter directives, explicit parentheses, multiline
string regions); the 22-kind typed syntax set (`GEN §16.1`) with `AST_CHILD`
normalized field names and ordinals (`GEN §16.2`) preserving source containment order
separately from evaluation order (`GEN §16.3`); `PARSE_ERROR` canonicalized across
Ruff and Tree-sitter on overlap (Ruff is typed-parser authority; Tree-sitter
preserves the recovery region); `MISSING_SYNTAX` from Tree-sitter only (D-28);
`CORRESPONDS_TO` between Ruff AST nodes and smallest compatible Tree-sitter named
nodes (`GEN §16.4`, no 1:1 assumption); all coordinates mapped to authoritative bytes
through `ProviderText.original_byte_offsets` (`GEN §9`); no Ruff type escapes
(governance rule `ruff-boundary-only`).

#### Dependencies

WP27, WP28, WP29, WP30 (correspondence + shared fixtures).

#### Target Invariants

I-10, I-29, I-30; `GEN §96`; `ONT §4.2`–`§4.4`. Doctrine P6, P11 advance.

#### Design and Library References

`GEN §7.2`, `§9`, `§15`–`§16.4`; `ONT §5`–`§6`; LD-23. Library ref:
`ruff_python_crates_advanced_reference_2026-08-18.md` (mandatory pre-read; 0.0.x).

#### Change Surface

##### Preflight Query

```bash
rg -n 'ruff_python' src/ --type rust         # expect only the new adapter
ast-grep run -l rust -p 'struct ProviderText { $$$ }' src/source_image.rs
rg -n 'field_role' contracts/generated/registry/   # WP28 codes present
```

##### Known Touch (verified this session)

New adapter module(s) behind `fact-generation`; `rules/` + `rule-tests/`; fixture
additions.

#### Required Changes

Parse via Ruff parser crates on `ProviderText`; token/trivia/index extraction;
typed-AST walk emitting the 22-kind set with normalized field names; directive
recognition; diagnostic conversion; correspondence pass consuming WP30 output;
fixtures with expected rows for each `GEN §15` fact rule.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp31_behavioral_acceptance`.

each `GEN §15` fact rule proven by a fixture (tokens,
comments, docstrings, directives, parentheses, continuation lines, parse errors);
`GEN §16.3` source-order/evaluation-order separation asserted on a decorator/default
-argument fixture.

##### Structural

Executable oracle: `wp31_structural_acceptance`.

every emitted coordinate resolves into authoritative
bytes and round-trips through the byte-offset mapping; `CORRESPONDS_TO` only links
range-and-kind-compatible pairs, unmatched nodes remain valid.

##### Negative / Zero-State

Executable oracle: `wp31_negative_zero_state`.

`ruff-boundary-only` green with tested fixtures;
`MISSING_SYNTAX` never originates from Ruff absence (negative fixture).

##### Operational

Executable oracle: `wp31_operational_acceptance`.

parse/token/diagnostic counters per run.

#### Edit-Local Gates

Targeted nextest on the adapter.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on the coordinate-mapping code.

#### Integration Milestone

M05.

#### Replan Triggers

A pinned Ruff crate lacks a required surface (e.g. comment-range or index API
mismatch with the reference) → implementation adaptation via the documented
alternative; if the typed AST cannot express a required syntax kind → registry
append + spec issue.

#### Rollback or Recovery

Additive; feature-gated.

### WP32 — Canonical source/syntax encoders and source-range reconciliation

#### Outcome

Production encoders for `source_file`, `source_token`, `source_annotation`,
`syntax_detail` plus the `entity`/`relation` rows for source/syntax facts, all
through the existing validated-batch path (`FAB §63`–`§66` — typed builders, batch
sizes, eleven checks); `AST_CHILD` relations generated from `syntax_detail`
parent/role/ordinal columns as `FAB §20` mandates (never independently authored);
canonical identity per `ONT AC-G-12`/`AC-G-13` (`file_id` CBEF recipe over
`comparison_key_bytes`; occurrence identity per `GEN §13.2` excluding display text)
and lexical relationships per `ONT §5.2`/`GEN §67A` (`CONTAINS_SPAN`, `TOKEN_OF`,
immediate `LEXICALLY_PRECEDES`, `DOCUMENTS`, `DIRECTIVE_APPLIES_TO`,
`EXPLICITLY_PARENTHESIZED`, `PARSE_ERROR_AT`, `MISSING_AT`); the `GEN §80` five-step
range-reconciliation ladder and `GEN §81` declaration merge keys applied to
Tree-sitter × Ruff evidence within `context:source`, with conflicts retained as
evidence + diagnostics and never silently overwritten (`GEN §5.3`); syntax-detectable
declarations and call-site syntax entities emitted without project semantics
(`RM §9` WP4); repeated full extraction of identical bytes produces identical
canonical IDs and effective rows.

#### Dependencies

WP28, WP30, WP31.

#### Target Invariants

I-03, I-05, I-14, I-29, I-30; `GEN §96` in full for the emitted families. Doctrine
P11, P13, P15, P17 advance.

#### Design and Library References

`FAB §14`–`§20`, `§63`–`§66`, `§69`–`§70`, `§95`; `GEN §80`–`§81`, `§67A`, `§13`;
`ONT §5`–`§6`, `§63`–`§64`, `AC-G-12`, `AC-G-13`; D-28.

#### Change Surface

##### Preflight Query

```bash
rg -n 'SyntheticCanonicalIngest' src/ tests/     # consumers before extension
ast-grep run -l rust -p 'fn encode_entities($$$A)' src/fact_ingest.rs
rg -n 'derive_identity|encode_public_id' src/identity.rs   # CBEF entry points
```

##### Known Touch (verified this session)

`src/fact_ingest.rs` (new encoders + table codes), `src/identity.rs` consumers, new
reconciliation module(s) behind `fact-generation`, `contracts/fixtures/` expected-row
families.

#### Required Changes

Implement the four table encoders with capacity-hinted builders; extend batch
validation coverage to the new specs; implement the source/syntax canonicalization
path behind the same reconciliation signature the synthetic body implements (v5 D-07:
replacement changes the body, not consumers — full engine arrives WP36); implement
the §80 ladder with the "never attach to an arbitrary overlapping node" hard rule;
determinism harness (extract twice, byte-compare canonical outputs).

#### Legacy Disposition and Decommission

Begins L-07: the synthetic body remains the only *cross-provider* path until WP36;
this packet must not create a second parallel canonicalization authority — the
source/syntax path plugs into the same signature.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp32_behavioral_acceptance`.

fixture families land as exact expected rows in all
four tables plus `entity`/`relation`; `AST_CHILD` rows derived from `syntax_detail`
match ordinals/field roles; §80 ladder fixtures hit each of the five steps plus the
synthetic-occurrence fallback.

##### Structural

Executable oracle: `wp32_structural_acceptance`.

repeated extraction of identical bytes → identical
canonical IDs and effective rows (byte-level comparison); every edge endpoint exists;
every span is a valid current-byte range (`GEN §96`).

##### Negative / Zero-State

Executable oracle: `wp32_negative_zero_state`.

conflicting Tree-sitter/Ruff evidence produces retained
evidence + conflict diagnostics, never silent overwrite; cross-context merge
rejected; no second canonicalization ingress exists
(`rg`/`ast-grep` zero-hit + build).

##### Operational

Executable oracle: `wp32_operational_acceptance`.

encoder/validator/reconciliation counters; batch-size
conformance to `FAB §64`.

#### Edit-Local Gates

Targeted nextest on encoders + ladder.

#### Packet-Local Gates

`just ci-fast`; `just wave3-integration-check` (fabric substrate unbroken);
`just mutants-file` on the §80 ladder and §81 merge-key logic.

#### Integration Milestone

M05.

#### Replan Triggers

The ladder cannot deterministically anchor a required fact family → design reopening
candidate (returns to `GEN §80` as a spec issue); batch-validation extension breaks
Wave-3 throughput expectations → performance adaptation.

#### Rollback or Recovery

Additive tables; owner replacement is idempotent (I-14).

### WP33 — Classification, capability, unknown handling, and Wave-4 integration

#### Outcome

`GEN AC-G-43` file admission: size thresholds (16 MiB ordinary / 64 MiB hard, 8 KiB
binary sample, 1 MiB line warning, 256 components / 4096 path bytes), binary
detection, language-mapping precedence (workspace override → registered extension →
shebang → none; conflicts produce a diagnostic and never run two parsers silently),
default exclusions (`.git`, build/dist/cache trees, `node_modules`, venvs, vendored
profiles), oversized/unsupported outcomes as inventory entities with digests and
explicit capability records — never silent omission; `GEN §85`/`AC-G-36` capability
records with the aggregation algebra (`COMPLETE` only when every applicable child is
complete; one `INDETERMINATE` child blocks parent `COMPLETE`; exclusions are explicit
scope exclusions) persisted to `capability_status` (`FAB §13.10`); parse-error/
missing-syntax dual-surface rule (D-28) proven end-to-end; source-context exact-range
extraction proof (requested vs delivered ranges over authoritative bytes with
checksums — storage side of `QRY AC-G-55`; wire encoder is WP38); notebooks
classified `UNSUPPORTED` (D-37); the `wave4-integration-check` recipe added: capture
→ jobs → adapters → encoders → publish → `ServingQuerySession` SQL over the new
tables under a pinned snapshot, covering valid, partially invalid, malformed,
oversized, binary, and excluded inputs.

#### Dependencies

WP29, WP32.

#### Target Invariants

I-06 (absence never proof of absence; explicit gaps), I-34 precursor; `GEN §96`
"unknown != absent". Doctrine P8, P23 advance.

#### Design and Library References

`GEN AC-G-43`, `AC-G-36`, `§84`–`§85`, `§97`; `ONT AC-G-73` (W4 subset: parse/missing
+ capability gaps); `FAB §13.10`, `§17`; `QRY AC-G-55` (storage half); D-28, D-37.

#### Change Surface

##### Preflight Query

```bash
rg -n 'SourceCapabilityGap|CaptureOutcome' src/source_image.rs   # existing seams
rg -n 'capability_status' src/generated/table_specs.rs src/fabric.rs
rg -n 'wave3-integration-check' justfile -A5                      # recipe pattern
```

##### Known Touch (verified this session)

Classification module(s), `src/source_image.rs` capture-policy integration,
`capability_status` writers, `justfile` (+`wave4-integration-check`),
`tests/integration/` (new feature-gated case module inside the single target).

#### Required Changes

Implement classification + admission ahead of provider dispatch; capability record
emission and aggregation; integration recipe and fixtures; exact-range extraction
API over `source_file` columns with byte checksums.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp33_behavioral_acceptance`.

each AC-G-43 class (oversized, binary, unsupported
encoding, vendored, generated-tree exclusion, notebook) yields the exact expected
inventory entity + capability record + diagnostic; aggregation algebra truth-table
fixtures.

##### Structural

Executable oracle: `wp33_structural_acceptance`.

source-context extraction round-trips exact bytes and
ranges incl. multi-byte boundaries and CRLF profiles.

##### Negative / Zero-State

Executable oracle: `wp33_negative_zero_state`.

no silent omission — every authorized inventory path is
accounted for as analyzed, excluded, or capability-gapped (closed-world assertion
over the integration fixture); conflicting language evidence never runs two parsers
(fixture).

##### Operational

Executable oracle: `wp33_operational_acceptance`.

`just wave4-integration-check` — exit 0 (becomes a standing gate).

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just wave4-integration-check`.

#### Integration Milestone

M05.

#### Replan Triggers

The closed-world accounting cannot be satisfied for a discovered file class → plan
revision (classification taxonomy extension via AC-G-43 spec issue).

#### Rollback or Recovery

Additive.

## 8. Work packets — Wave 5 (End-to-end vertical golden slice)

Gate B is planned to the `SUITE` Part V gate text, which is broader than `RM §10`'s
prose: it requires one **Python** owner as well as one Rust MIR owner, an **unknown
fact**, a **streamed result**, and an **artifact result**. No packet may invent a
missing gate contract; a gap stops and files against the owning document
(`SUITE` Part V closing rule).

### WP34 — Golden corpus v1 substrate (SUITE AC-G-78)

#### Outcome

`tests/golden/codefabric-golden-v1/` exists with the three logical groups
(`workspace/`, `scenarios/`, `expected/`) and the Wave-5 subset of coverage: one
Python owner package, one small Rust build unit with an MIR-bearing body, the
compile-failure fixture (`060`-class) that preserves syntax while withdrawing
compiler-semantic capability, and explicit unknown/property/relation/derived
-projection fixtures (`RM §10` WP1); the corpus manifest pins `corpus_id`,
`corpus_version`, and `b3:` digests for the source archive and every input bundle
(`§78.4`); identity pinning records expected IDs **and CBEF preimages** (`§78.5`);
the `§78.9` scenario harness executes YAML `operations` over `base_scenario` with
deterministic barriers and fault hooks (no sleeps); `§78.10` versioning discipline
holds (released corpus immutable; changes create a new version); unsupported
-platform fixtures gate with `PLATFORM_NOT_APPLICABLE` (darwin: non-UTF-8 path
fixtures), never silent substitution; expected values are independently reviewed and
cannot be generator-approved (`SUITE AC-G-05`, I-24).

#### Dependencies

M05 (Wave 4 complete).

#### Target Invariants

I-08, I-24; `SUITE AC-G-78 §78.4–§78.10`. Doctrine P25, P30 advance.

#### Design and Library References

`SUITE AC-G-78` (all subsections), `AC-G-05`; `RM §10` WP1; D-30 (state-digest check
staging).

#### Change Surface

##### Preflight Query

```bash
ls tests/golden/ 2>/dev/null                      # expect absent before
rg -n 'fixture-oracles' contracts/manifests/       # oracle registration mechanism
rg -n 'fixture_candidates' tooling/contracts/      # candidate/review pipeline
```

##### Known Touch (verified this session)

New `tests/golden/codefabric-golden-v1/` tree; `contracts/manifests/fixture-oracles.json`
consumers; harness module in `tests/integration/` (feature-gated, inside the single
top-level target per repo-spec §61.3); `justfile` (fixture recipes if needed).

#### Required Changes

Author workspace fixtures as exact bytes (tar-preserved mode/symlink/line-ending
fidelity); scenario YAML for the Wave-5 set (clean bootstrap, single-owner Python
edit, Rust compile failure/recovery); expected-output directories populated per
`§78.5`–`§78.8` as later packets produce them (this packet fixes structure, manifest,
harness, and input fixtures; expected outputs land with WP40's ceremony under
independent review).

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp34_behavioral_acceptance`.

harness applies scenario operations with
`expected_old_digest` verification and deterministic barriers; corpus manifest
digests recompute clean.

##### Structural

Executable oracle: `wp34_structural_acceptance`.

archive round-trip preserves exec mode, symlink target
bytes, and line endings; platform gating yields `PLATFORM_NOT_APPLICABLE`, not a
substitute fixture.

##### Negative / Zero-State

Executable oracle: `wp34_negative_zero_state`.

no generator writes into `expected/` (harness
write-path assertion + review-marker check).

##### Operational

Executable oracle: `wp34_operational_acceptance`.

`just fixture-check` extended to the corpus manifest — exit 0.

#### Edit-Local Gates

Harness-module nextest slice.

#### Packet-Local Gates

`just ci-fast`.

#### Integration Milestone

M06.

#### Replan Triggers

Scenario mechanics need a barrier/fault hook the runtime cannot expose yet → record
as discovered obligation on the packet that owns the hook (WP40/WP46), not a corpus
change.

#### Rollback or Recovery

Fixture tree is additive and immutable once released.

### WP35 — Thin rustc extractor slice (GEN AC-G-31 subset)

#### Outcome

The extractor's `--serve` no-op is replaced by the real protocol for one small crate:
`Handshake` then `Extract` streaming `CompilationAccepted → CompilationBegin →
(OwnerBegin → OwnerObservationChunk* → OwnerEnd)* → CompilationEnd` (spec event
names; SI-05) over the private endpoint with Arrow IPC chunk payloads, sequence
numbers, digests, and credit control (≤4 outstanding chunks, ≤16 MiB unacknowledged);
daemon-side launcher provides the `CODEFABRIC_EXTRACTOR_*` environment contract,
sandbox-private offline `CARGO_HOME`/target/temp, and the 10 s cancel-ack deadline
before process-group termination (`GEN AC-G-32`); `CompilationBegin` carries the
exact rustc version/commit, normalized invocation digest, and metadata/lock/config
digests; toolchain mismatch fails with `TOOLCHAIN_MISMATCH` before activation
(`GEN §0.4`); the Rust analysis-context manifest subset (`GEN AC-G-14`) derives the
immutable `analysis_context_id`; extracted facts are the D-27 minimal set — one
body's `GEN §38` families, `GEN §39.1` terminator edges, `GEN §41` direct calls plus
one deliberate `CALLS_UNKNOWN`; the default 1.x acceptance profile holds (semantic
output accepted only on successful `CompilationEnd` with every owner closed; compile
failure yields `UNAVAILABLE_COMPILE` capability withdrawal while syntax publishes);
protocol violations reject the run with `PROTOCOL_ERROR`; extraction data never
touches rustc stdout/stderr.

#### Dependencies

WP29 (job runtime), M05. From WP34 only the golden fixture crate, which may be
authored first — this is what permits `{WP34 || WP35}` in §15.

#### Target Invariants

I-06, I-10 (no compiler type escapes the callback), I-30; `GEN §96`
(session-local `DefId`/`CrateNum` never persistent); L-08 exit. Doctrine P6, P8, P23
advance.

#### Design and Library References

`GEN AC-G-31` (rules 1–9), `AC-G-32` (compiler placement), `AC-G-14`, `§7.4`, `§38`,
`§39.1`, `§41`, `§11`; `SUITE §80.6` (extractor build-matrix row incl. golden
extraction); D-27; LD-12 (dated nightly). Library ref:
`rust_mir_cpg_continuous_reference_2026-08-18.md` via `code-facts-lib-ref`.

#### Change Surface

##### Preflight Query

```bash
rg -n 'serve|identity' rustc-extractor/src/main.rs          # current stub shape
rg -n 'service RustcExtractor' contracts/rpc/rustc_extractor.proto -A8
rg -n 'CODEFABRIC_EXTRACTOR' rustc-extractor/ src/           # env contract, expect zero
just extractor-ci-fast                                       # domain baseline
```

##### Known Touch (verified this session)

`rustc-extractor/src/` (main, rustc_link, new protocol/extraction modules),
`rustc-extractor/Cargo.toml` (transport deps for the extractor side), daemon-side
launcher module behind `fact-generation`+`rpc`, `scripts/run_rustc_extractor.sh`.

#### Required Changes

Extractor-side: rustc_public driver extracting the D-27 families into normalized
observation rows, Arrow IPC encoding, tonic service over the endpoint. Daemon-side:
process launch/sandbox/env/cancel handling as a `COMPILER_GROUP` provider under
WP29's executor; context-manifest computation and `TOOLCHAIN_MISMATCH` enforcement.
The dated-nightly root never contaminates the stable root (existing target-dir
isolation preserved).

#### Legacy Disposition and Decommission

L-08: the `--serve` no-op path is deleted; negative check below.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp35_behavioral_acceptance`.

Extractor-domain tests plus golden extraction: the fixture
crate produces the exact expected observation stream (manifest sequence, owner set,
family counts, digests); compile-failure fixture yields capability withdrawal with
syntax intact.

##### Structural

Executable oracle: `wp35_structural_acceptance`.

protocol-violation fixtures (unexpected owner,
duplicate sequence, missing end, count mismatch, stale digest, early EOF) each reject
with `PROTOCOL_ERROR`; credit-control bounds enforced under a slow consumer.

##### Negative / Zero-State

Executable oracle: `wp35_negative_zero_state`.

the no-op `--serve` path is gone (`rg` zero-hit on the
stub marker + behavioral proof); no extraction bytes on stdout/stderr (captured-run
assertion); no `rustc_*` type crosses the callback boundary (extractor-local
structural rule).

##### Operational

Executable oracle: `wp35_operational_acceptance`.

run/cancel/timeout metrics; sandbox paths verified
private and offline.

#### Edit-Local Gates

`just extractor-check`, `just extractor-test`.

#### Packet-Local Gates

`just extractor-ci-fast`; `just ci-fast`; daemon↔extractor interop test in
`tests/integration/`.

#### Replan Triggers

`rustc_public` on the pinned nightly cannot expose a D-27 family without private-API
reach-through → design reopening (extractor spec issue); protocol needs a message the
generated proto lacks → contract change via Wave-1 machinery.

#### Integration Milestone

M06.

#### Rollback or Recovery

Extractor domain is process-isolated; the daemon treats absence as
`UNAVAILABLE_PROVIDER`.

### WP36 — Minimal canonical reconciliation engine (FAB AC-G-37 core)

#### Outcome

The production `ReconciliationEngine` replaces the synthetic body behind the
unchanged signature (v5 D-07 fulfilled): deterministic 8-step pipeline over N
observation streams with the canonical sort key; the `AC-G-37` source-correspondence
table for identifiers (exact byte span), declarations (exact name span + compatible
kind + enclosing owner), and call sites (exact callee span, else same enclosing
expression with overlap ≥ 0.80 and unique candidate); `GEN §81` declaration and
`GEN §83` call-target reconciliation across Tree-sitter × Ruff × extractor evidence;
`GEN §84` unknown materialization (`UNKNOWN_CALL_TARGET` for the golden unknown
fact); equal-authority conflicting exact observations produce
`CONFLICTING_EXACT_EVIDENCE` with no arbitrary winner; all evidence retained in
`fact_evidence` with conflict records; capability reporting stays separate
(`GEN §85`). DB07 exits here: zero production references to
`SyntheticCanonicalIngest`.

#### Dependencies

WP32, WP35.

#### Target Invariants

I-04, I-05, I-06, I-30; `GEN §5.3` (never silently overwrite); authority tables
`GEN §5.1`–`§5.2`. Doctrine P15, P17, P27 advance.

#### Design and Library References

`FAB AC-G-37`; `GEN §80`–`§85`; `ONT §62` certainty/resolution codes; DB07.

#### Change Surface

##### Preflight Query

```bash
rg -n 'SyntheticCanonicalIngest' src/ tests/ contracts/    # full consumer set
ast-grep run -l rust -p 'fn ingest($$$A)' src/fact_ingest.rs
```

##### Known Touch (verified this session)

`src/fact_ingest.rs` (engine body swap behind the signature), new reconciliation
module(s), `contracts/fixtures/synthetic/` (retained as engine test inputs).

#### Required Changes

Implement the engine; wire provider precedence from the generated provider registry;
migrate the conflicting-observation fixture family onto the production engine;
delete the synthetic body.

#### Legacy Disposition and Decommission

**DB07 exit.** `SyntheticCanonicalIngest` reaches zero production references; the
synthetic fixtures remain as test inputs to the production engine only.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp36_behavioral_acceptance`.

golden declaration/range/call evidence reconciles to
exact expected canonical rows; the unknown-fact fixture materializes
`UNKNOWN_CALL_TARGET`; conflict fixture yields `CONFLICTING_EXACT_EVIDENCE` +
retained evidence.

##### Structural

Executable oracle: `wp36_structural_acceptance`.

same input streams in any arrival order produce
byte-identical canonical batches (determinism under the canonical sort key).

##### Negative / Zero-State

Executable oracle: `wp36_negative_zero_state`.

**DB07 oracle**: `rg -n 'SyntheticCanonicalIngest'
src/` zero-hit outside `#[cfg(test)]`; `ast-grep` structural zero for construction
sites; green build (tier-1 confirmation).

##### Operational

Executable oracle: `wp36_operational_acceptance`.

reconciliation counters (streams, facts, conflicts,
unknowns) per run.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on the correspondence/precedence logic;
`just wave4-integration-check` (still green on the production engine).

#### Integration Milestone

M06.

#### Replan Triggers

The 0.80-overlap unique-candidate rule fails on a real fixture ambiguously → emit
multi-candidate/unknown per `GEN §5.3`, never loosen the rule silently; if that
blocks a Gate B artifact → spec issue against `FAB AC-G-37`.

#### Rollback or Recovery

Signature unchanged; the prior body is recoverable from history, but DB07 forbids
its return to production paths.

### WP37 — Minimal registered derivation (SYNTAX_TREE_V1)

#### Outcome

The first populated derivation-registry record: the `SYNTAX_TREE_V1` projection
(`ONT AC-G-74`: "syntax occurrences + ordered AST-child edges; source context only")
registered with its `FAB AC-G-42` matrix row (placement, authority, selection key
`(derivation_family, profile_id, bundle_digest)`), computed deterministically from
canonical rows with no new dependencies (D-38), materialized per its declared
placement, recorded in the derivation bundle; exactly one active authority per
snapshot for the family.

#### Dependencies

WP36.

#### Target Invariants

I-08, I-15 (mechanical derivation only); `FAB AC-G-42` single-authority rule.
Doctrine P14, P27 advance.

#### Design and Library References

`FAB AC-G-42`, `§79A` (registry); `ONT AC-G-74`; D-38; derivation registry currently
zero records (session-verified).

#### Change Surface

##### Preflight Query

```bash
rg -c 'records' contracts/registry/derivation-registry.yaml 2>/dev/null || cat contracts/registry/derivation-registry.yaml
rg -n 'derivation_bundle_version' src/fabric/publication.rs
```

##### Known Touch (verified this session)

`contracts/registry/derivation-registry.yaml` (+regeneration), derivation module
behind `fact-generation`, `contracts/bundles/derivation/`.

#### Required Changes

Registry record + regeneration; projection builder over `syntax_detail`/`relation`
rows; bundle version recording in publication pins.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp37_behavioral_acceptance`.

golden fixture → exact expected projection rows;
recomputation idempotent.

##### Structural

Executable oracle: `wp37_structural_acceptance`.

the projection's registry record round-trips through
generation; publication pins carry the derivation bundle version.

##### Negative / Zero-State

Executable oracle: `wp37_negative_zero_state`.

a second implementation for the family cannot activate
in one snapshot (selection-key uniqueness test).

##### Operational

Executable oracle: `wp37_operational_acceptance`.

Projection row/duration counters.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just contracts-verify`.

#### Integration Milestone

M06.

#### Replan Triggers

`SYNTAX_TREE_V1` turns out to require a capability the canonical rows lack → pick the
next-cheapest AC-G-42 row (implementation adaptation, recorded in state).

#### Rollback or Recovery

Additive registry append + module.

### WP38 — Minimal semantic query slice (QRY core subset)

#### Outcome

A bounded production query path: request validation against the generated
`cpg-semantic-query-request` schema (all eight forms *validate*; only three
*compile*); phrase resolution strictly through the phrase registry's
`planspec_mapping` data — the resolver never guesses; unregistered or unimplemented
phrases fail with registry error codes under `SRV AC-G-65`'s
suite-referenced-codes rule (`SEMANTIC_PHRASE_AMBIGUOUS` from its explicit list;
`SEMANTIC_PHRASE_UNRECOGNIZED` from `QRY AC-G-44`; SI-13); typed `PlanSpec`
construction (`QRY AC-G-46`:
SQL strings are not an IR) for `find code entities` (§13), `retrieve facts about
code` (§14), and `retrieve source and syntax context` (§20 incl. the `AC-G-55`
source-context wire encoding with `Utf8TextContext`/`RawBytesContext`, checksums, and
`source_context_id` CBEF recipe); lowering to DataFusion over `cpg_serving` views
only (rules `serving-projections-generated-only`, `serving-leased-providers-only`);
deterministic ordering (`QRY §33`); the `§36` response envelope with orthogonal
states and `PublicSnapshotMetadata` (`§37`); canonical logical-response bytes and
`b3:` digests per `QRY AC-G-53` `codefabric-jcs-v1` (existing JCS implementation);
freshness accepted per D-41's bounded quiescent-workspace regime.

#### Dependencies

M05 substrate. WP37's projection is consumed only by the acceptance fixtures, so
development may parallel WP37 (`{WP37 || WP38}` in §15) with acceptance gated on
it.

#### Target Invariants

I-02, I-07, I-15 (evaluative requests rejected by construction — only registered
fact phrases exist), I-33; `QRY §4` principles for the served forms. Doctrine P9,
P16, P21, P26 advance.

#### Design and Library References

`QRY §5`–`§8` (envelope, scope, freshness), `§13`, `§14`, `§20`, `§33`, `§36`–`§37`;
`QRY AC-G-44`, `AC-G-46`, `AC-G-53`, `AC-G-55`; `FAB §91`–`§93`; generated
`contracts/query/planspec.schema.json`, phrase registry (45 records,
session-verified); D-41; SI-13.

#### Change Surface

##### Preflight Query

```bash
rg -n 'planspec_mapping' contracts/registry/phrase-registry.yaml | head -5
rg -n 'cpg_source_context|serving_projection' src/generated/table_specs.rs
ast-grep outline src/fabric/serving.rs --match 'ServingQuerySession' --view expanded
```

##### Known Touch (verified this session)

New query module(s) behind `daemon`; `src/fabric/serving.rs` (session seam);
`src/contracts/jcs` consumers.

#### Required Changes

Envelope parsing/validation; phrase→PlanSpec binding from registry data; three
TypedQueryNode compilers; DataFusion lowering with the snapshot-pinned session;
response materialization with deterministic order, JCS bytes, digests; source-context
encoder over WP33's extraction API.

#### Legacy Disposition and Decommission

None (D-41 shim is bounded and exits at WP45/DB08).

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp38_behavioral_acceptance`.

golden request fixtures → byte-exact canonical
responses with pinned digests (`§78.8` file set); alias phrases compile to the same
AST and checksum.

##### Structural

Executable oracle: `wp38_structural_acceptance`.

response ordering matches `QRY §33` on adversarial
orderings; every lowering targets `cpg_serving` views only (governance rules green).

##### Negative / Zero-State

Executable oracle: `wp38_negative_zero_state`.

the five unimplemented forms and unregistered phrases
fail with precise registry error codes, never partial results; evaluative phrasings do
not exist in the registry (closed-registry assertion).

##### Operational

Executable oracle: `wp38_operational_acceptance`.

Query plan/duration/row counters via `ServingRuntimeEvidence`.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on ordering + phrase binding.

#### Integration Milestone

M06.

#### Replan Triggers

A phrase registry record lacks executable `planspec_mapping` data (AC-G-44 violation)
→ registry correction via Wave-1 machinery, packet blocked on the regenerated
registry, spec issue filed.

#### Rollback or Recovery

Additive module.

### WP39 — Minimal accepted-handle service and result artifacts (SRV Part II subset)

#### Outcome

The daemon serves `codefabric.cpgd.v1` over the private UDS for an internal test
client: `Handshake` (version/bundle negotiation), `StartQuery` (unary; returns
`daemon_query_id`, `resume_token`, `ACCEPTED` before any freshness wait),
`StreamQuery` (the closed five-variant `QueryEvent` oneof; `SnapshotPinnedEvent`
first with canonical `PublicSnapshotMetadata`; monotonic non-reused sequences; event
checksums; `TerminalEvent` exactly once), `CancelQuery` (cancels wait/execution/
artifact creation, releases leases, deletes incomplete artifacts; effective without
ack), `ReadResult`/`ReleaseResult` over the minimal immutable artifact store
(`SRV AC-G-63`: CBEF artifact ID, `codefabric-result://` URI, 0600/0700, TTL
defaults; `AC-G-67`: range reads ≤1 MiB with checksums over uncompressed canonical
bytes, 5-minute read lease, ≤4 concurrent reads, idempotent release); idempotency
pre-pin key per `SRV §15` with `IDEMPOTENCY_CONFLICT`; artifacts bound to snapshot
leases so retirement waits (`SRV §17`); `GetStatus`/`ValidateQuery`/`AttachQuery`
and all production security (AC-G-60/61/69) explicitly deferred to W17.

#### Dependencies

WP38.

#### Target Invariants

I-02, I-07, I-33 (terminal completeness record); `SRV §6` invariants for the
implemented verbs. Doctrine P16, P18, P22, P24 advance.

#### Design and Library References

`SRV §8`–`§17`, `AC-G-58` (event/handle contract), `AC-G-63`, `AC-G-67`; `SRV §68`/
`§79` (test-layer legitimacy of the internal client); existing `rpc.rs` UDS
transport + peer auth (session-verified).

#### Change Surface

##### Preflight Query

```bash
rg -n 'QueryEvent|StartQuery' src/generated/codefabric.cpgd.v1.rs | head
rg -n 'SnapshotLeaseManager|SnapshotLeaseGuard' src/snapshot_runtime.rs | head
ast-grep outline src/rpc.rs --items exports
```

##### Known Touch (verified this session)

New service module behind `daemon`+`rpc`; artifact-store module; `src/snapshot_runtime.rs`
lease consumers; `tests/integration/` service tests with the in-process test client.

#### Required Changes

Service implementation over generated stubs; event pipeline from WP38 execution;
artifact store with CBEF IDs, TTL sweep, permission bits; idempotency records in the
operational store; lease binding.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp39_behavioral_acceptance`.

start→pin→chunks→terminal for a golden query; cancel
mid-stream releases leases and deletes partial artifacts; artifact read returns
byte-exact canonical payload across chunked range reads; re-start with same
idempotency key returns the pinned record; conflicting content →
`IDEMPOTENCY_CONFLICT`.

##### Structural

Executable oracle: `wp39_structural_acceptance`.

`TerminalEvent` exactly once across success/cancel/
error paths; sequences strictly monotonic, never reused; first semantic event is
`SnapshotPinnedEvent`.

##### Negative / Zero-State

Executable oracle: `wp39_negative_zero_state`.

no progress event exposes SQL, physical table names, or
source content (`SRV §13` fixture scan); artifacts unreadable after release/expiry;
0600/0700 modes asserted.

##### Operational

Executable oracle: `wp39_operational_acceptance`.

stream/artifact/lease metrics; TTL sweeper observable.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; interop case in `tests/integration/rpc.rs` lineage.

#### Integration Milestone

M06.

#### Replan Triggers

The generated cpgd.v1 protocol lacks a required field for the five-verb slice →
contract change via Wave-1 machinery (plan revision if the service shape moves).

#### Rollback or Recovery

Additive service; daemon runs without it constructed.

### WP40 — Vertical golden slice integration and Gate B (M06)

#### Outcome

The eleven Gate B artifacts pass end-to-end through one run over the golden corpus:
bootstrap → Wave-4 providers + WP35 extractor → WP36 reconciliation (Python owner,
Rust MIR owner, unknown fact, property fact, relation fact) → WP37 projection →
durable publication (`FAB §71.1`) → single-owner mutation scenario driving a real
non-empty hot overlay (production `SnapshotOverlayProviderFactory` over
`ConsolidatedOverlay`; `EmptySnapshotOverlay` remains only the legitimate empty
case) → `FAB §71.2` activation with pin preservation for an in-flight lease → later
durable flush/rebase (`FAB AC-G-22` protocol, bounded to this scenario; the
threshold-driven continuous loop is WP46) → WP38 query → WP39 stream + artifact;
expected outputs pinned into `expected/` under independent review (I-24); each
Wave-5 scenario ends with the D-30 state-digest clean-rebuild equality check; new
recipes `wave5-integration-check` and `gate-b-check` added; **Readiness Gate B
passes exactly as `SUITE` Part V defines** — M06.

#### Dependencies

WP34–WP39 all complete.

#### Target Invariants

I-02, I-09 (scenario-scoped), I-24, I-33; `LIFE §125` pin order; `FAB §91`
("a long-running query never observes an active-pointer change"). Doctrine P18, P24,
P25, P27 advance.

#### Design and Library References

`SUITE` Part V Gate B; `FAB §71`, `AC-G-20`–`AC-G-23`, `AC-G-26`; `LIFE §100`–`§107`
(kernel semantics, this scenario's slice); D-30; `SUITE AC-G-78 §78.7`–`§78.8`.

#### Change Surface

##### Preflight Query

```bash
rg -n 'EmptySnapshotOverlay|SnapshotOverlayProviderFactory' src/
rg -n 'wave3-integration-check' justfile -A6
just plan-status                                   # packet trust before ceremony
```

##### Known Touch (verified this session)

Overlay-provider factory implementation beside `src/fabric/snapshot_catalog.rs`;
integration harness; `justfile` (+2 recipes); `tests/golden/.../expected/`.

#### Required Changes

Production overlay provider factory (overlay replacement batches + tombstone
anti-joins per `FAB §91`/`LIFE §105`); the end-to-end harness wiring scenario
operations to the real daemon components; expected-output pinning + review; recipes.

#### Legacy Disposition and Decommission

None beyond exercising WP36's cutover; DB08 remains open (freshness shim exits at
WP45).

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp40_behavioral_acceptance`.

Wraps `just gate-b-check`: the eleven artifacts in one run
with byte-exact expected outputs (responses, stream events, artifact manifest,
publication/snapshot manifests per `§78.7`–`§78.8`).

##### Structural

Executable oracle: `wp40_structural_acceptance`.

the in-flight lease observes the pre-activation
snapshot to terminal completeness while the pointer advances (pin preservation);
rebase preserves effective state digests (`AC-G-22` equality).

##### Negative / Zero-State

Executable oracle: `wp40_negative_zero_state`.

intermediate publication versions invisible through
serving views during the scenario (probe between staging and pointer CAS via fault
hook); no gate artifact satisfied by a test double.

##### Operational

Executable oracle: `wp40_operational_acceptance`.

`just wave5-integration-check` and `just gate-b-check` — exit 0 (standing gates).

#### Edit-Local Gates

Harness nextest slice.

#### Packet-Local Gates

`just ci-fast`; `just wave4-integration-check`; `just wave5-integration-check`;
`just gate-b-check`.

#### Integration Milestone

M06 (this packet is the milestone ceremony).

#### Replan Triggers

Any Gate B artifact requires a contract that does not exist → stop; file against the
owning document (`SUITE` Part V rule); never invent the gate contract in
implementation.

#### Rollback or Recovery

Ceremony is read-only over landed packets; failures route to the owning packet.

## 9. Work packets — Wave 6 (Continuous update, freshness, and core equivalence)

Wave 6 defers all gix acceleration (`RM §11`); every Git-conditional LIFE clause
degrades to the generic authoritative-inventory path, while wave/work-item records
still *carry* their `GitStateVector` fence fields as absent and `GitAccelerationStatus`
remains representable as `NOT_A_GIT_WORKTREE`/`GIT_UNAVAILABLE` so the AC-G-25
machines model-check unchanged. Three timing quantities are deliberately distinct and
named throughout (per the LIFE §28 / AC-G-29 / Appendix B split): the **filesystem
debounce timeout** (75 ms), the **wave gather quiet period** (75 ms quiet / 500 ms max
ordinary, 250 ms / 2 s bulk), and the **downstream gather window** (20 ms).

### WP41 — Watcher and event facade

#### Outcome

`notify-debouncer-full 0.7.0` (LD-24) behind the daemon feature set, wrapped by the
application event facade: the `WatchChange` vocabulary (`DirtyPath{path,seq}`,
`RemovedPath`, `Renamed{from,to}`, `GitMetadata`, `ReconcileRequired{root,reason}`,
`WatcherError{class}`) with raw `notify::EventKind` never leaking downstream
(`LIFE §29`); handler work limited to classify/sequence/normalize/enqueue
(`LIFE §27`); Appendix B starting values (timeout 75 ms, tick 20 ms, ingress 4,096);
bounded `try_send` ingress where overflow atomically sets the reconcile-required flag
(a coordinator flag, not a lifecycle state — SI-07), drops event details, and
schedules authoritative reconcile (`LIFE §30`, `§9.7`); `need_rescan` → source trust
`UNVERIFIED` + reconciliation generation + coalesced rescans (`LIFE §9.3`); backend
failure degrades health while the pinned snapshot keeps serving with freshness
warnings, with PollWatcher fallback for difficult filesystems (`LIFE §9.4`, `§9.6`);
the `LIFE §117.1` watcher failure taxonomy mapped to diagnostics.

#### Dependencies

M06.

#### Target Invariants

I-03 (events are hints, never authority — `LIFE §157` inv. 11), I-31 precursor;
`LIFE §158` inv. 1–2. Doctrine P6, P22, P23 advance.

#### Design and Library References

`LIFE §27`–`§30`, `§9.3`–`§9.7`, `§117.1`, Appendix B; LD-24 (D-35); library ref
`notify_debouncer_full_rust_reference.md` (mandatory pre-read; 0.7.0 anchor,
explicitly not 0.8.0-rc).

#### Change Surface

##### Preflight Query

```bash
rg -n 'notify' Cargo.toml Cargo.lock                 # expect zero before
rg -n 'reconcile_required|change_token' src/source_image.rs src/coordinator.rs
just stable-graph-check                               # activation boundary baseline
```

##### Known Touch (verified this session)

`Cargo.toml`/`Cargo.lock` (+watcher deps), `scripts/stable_graph_check.sh`
(family/activation extension), new watch module behind `daemon`,
`src/coordinator.rs` seam.

#### Required Changes

Dependency adoption with graph-validator coverage; debouncer construction/lifecycle
per workspace root; facade + bounded channel; overflow/rescan/backend-failure
handling; PollWatcher configuration path; failure-taxonomy diagnostics.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp41_behavioral_acceptance`.

real-FS integration — create/modify/delete/rename
sequences yield the expected `WatchChange` stream (asserting final authoritative
state, not platform event order, per `LIFE §139`); overflow injection sets the
reconcile flag and drops cleanly.

##### Structural

Executable oracle: `wp41_structural_acceptance`.

no `notify` type escapes the watch module (`ast-grep`
rule `watcher-boundary-only`, added with fixtures); handler path contains no
parse/hash/traversal calls (structural assertion).

##### Negative / Zero-State

Executable oracle: `wp41_negative_zero_state`.

parser crates and watcher crates absent from narrow
feature graphs (extended `stable-graph-check`); no unbounded channel construction in
the watch path.

##### Operational

Executable oracle: `wp41_operational_acceptance`.

ingress depth, dropped-to-reconcile, rescan, backend
-failure counters.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just stable-graph-check`.

#### Integration Milestone

M07.

#### Replan Triggers

Platform backend cannot deliver a required event class even via PollWatcher →
implementation adaptation (documented degradation to periodic reconcile) + spec
issue if LIFE assumptions break.

#### Rollback or Recovery

Additive; the daemon without a watcher serves stale-marked snapshots.

### WP42 — Dirty registry, update-wave scheduler, and source capture loop

#### Outcome

The actor-owned per-workspace coordinator loop (`LIFE §110`, `§26`): dirty registry
holding latest-generation entries per normalized path (`LIFE §31`); update classes
`U0`–`U8` with the normative broaden-when-uncertain rule (`LIFE §13`); the
`UpdateWaveState` registry **extended through the Wave-1 machinery** from today's
coarse 7-state generated vocabulary (`Collecting`…`Superseded`, session-verified)
to the full `LIFE §21` 17-state machine — new states appended under the `AC-G-06`
append-only discipline, superseded coarse states deprecated with their codes never
reassigned, `registries.rs` and transition tables regenerated, and the `AC-G-25`
model checks re-run — driving `update_wave`/`update_wave_item` rows (immutable
after `HOT_PUBLISHED`); `AC-G-29` gather windows (ordinary 75 ms/500 ms,
bulk 250 ms/2 s, 10,000-path bulk escalation), candidate-set freeze at
`SNAPSHOTTING`, and the bounded internal `BeginSourceBatch`/`EndSourceBatch`/
`AbortSourceBatch` RPC (nested batches prohibited, 5 s abandonment default,
strict-current queries wait at the batch barrier); the `LIFE §114.1` generation rule
on every work item with `STALE_RESULT` rejection; supersession and cooperative
cancellation (`§114.2`, `§10.6` — side-effect-free work may finish but cannot
commit); the `LIFE §115` overload order (coalesce → discard superseded → bulk
escalate → defer derived/durable → preserve query headroom → expose degraded
status; debounce is never the overload mechanism); `LIFE §111`–`§113` priorities,
admission, and the shared thread budget; `LIFE §32` bulk-mode triggers; the
`LIFE §33` seven-step stable capture loop driving `capture_with_fence`; the
`LIFE §36` rescan generation fence (W0/G0→W1/G1, `dirty_after_reconcile`); the
`LIFE §34.3` Merkle inventory digest as `worktree_inventory_digest`.

#### Dependencies

WP41.

#### Target Invariants

I-03, I-14, I-31; `LIFE §157` inv. 8, 11, 16–17; `LIFE §158` inv. 3–4, 7;
`LIFE AC-G-28` generation rules (persisted `source_generation`, restart never
resets, rebuilt overlay uses a new generation). Doctrine P17, P18, P20, P22 advance.

#### Design and Library References

`LIFE §13`, `§21`, `§26`, `§31`–`§36`, `§110`–`§115`, `AC-G-28`, `AC-G-29`.
Session-verified caveat: the generated `UpdateWaveState` machine currently carries
only 7 coarse states (`Collecting`, `Snapshotting`, `Running`, `Publishing`,
`Complete`, `Failed`, `Superseded`) — the `LIFE §21` 17-state extension is this
packet's contract work, covered by A-01.

#### Change Surface

##### Preflight Query

```bash
rg -n 'update_wave|update_wave_item' src/operational_store.rs src/generated/
rg -n 'UpdateWaveState' src/generated/registries.rs -A20   # current 7-state shape
ast-grep outline src/coordinator.rs --items exports
rg -n 'advance_source_generation|capture_with_fence' src/source_image.rs
```

##### Known Touch (verified this session)

State-machine registry sources under `contracts/registry/` plus regenerated
`src/generated/registries.rs` and `contracts/generated/registry/` projections;
`src/coordinator.rs` (loop ownership), scheduler module(s), `src/source_image.rs`
consumers, `src/operational_store.rs` wave-table writers, `src/rpc.rs` (batch RPC).

#### Required Changes

Extend the `UpdateWaveState` registry source to the `LIFE §21` vocabulary and
regenerate (`just contracts-gen`, confirm-gated, diff-reviewed; `contracts-verify`
+ `contracts-repro-check` before completion per §4.1); scheduler actor with command
channels; classification; wave lifecycle through the regenerated transitions; batch
RPC; capture-loop integration; backpressure/admission; bulk-mode entry; rescan
fence.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp42_behavioral_acceptance`.

repeated saves to one path collapse to one latest
generation; a newer event supersedes a running wave which cannot commit; atomic-save
and rename sequences produce single coherent waves; batch RPC holds waves and
releases at the barrier; overflow escalates to bulk with the fence intact.

##### Structural

Executable oracle: `wp42_structural_acceptance`.

every wave/work-item transition flows through the
generated table (illegal → `STATE_TRANSITION_VIOLATION`); waves immutable after
`HOT_PUBLISHED`; candidate set frozen at `SNAPSHOTTING` (late event lands in next
generation).

##### Negative / Zero-State

Executable oracle: `wp42_negative_zero_state`.

no unbounded queue in the scheduler path; stale
-generation provider output cannot commit (`STALE_RESULT` fixture); restart never
resets `source_generation` (persistence fixture).

##### Operational

Executable oracle: `wp42_operational_acceptance`.

wave counts by class/state, queue depths, coalesce/
supersede/backpressure counters, gather-window timings.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on classification + supersession decisions.

#### Integration Milestone

M07.

#### Replan Triggers

The extended 17-state machine misses a transition the scheduler needs → registry append via
Wave-1 machinery (AC-G-25 discipline), never an untabled transition.

#### Rollback or Recovery

Coordinator restart recovers from persisted operational state (WP47 proves).

### WP43 — Invalidation and operational dependency graph (LIFE AC-G-41)

#### Outcome

The SQLite-persisted operational dependency graph: the `AC-G-41` edge schema
(`upstream_fingerprint`, `strength_code EXACT|CONSERVATIVE|CONFIGURATION`,
`source_generation`, `active`) with the 11 initial edge kinds; the 7-step update
algorithm with SCC handling; invalidation as a set over {worktree, owner, fact
family, representation, provider capability, derived projection} (`LIFE §14`, 18
fact families); deterministic replacement owners and the incoming-edge ownership
rule (`LIFE §15`); the 6-step propagation with unchanged-fingerprint stop and
threshold escalation (>10,000 dirty owners or >10% context owners → partition
rebuild; >40% or context/config change → full context rebuild; rescan + unknown
inventory delta → full inventory reconcile); inactive edges removed only by owner
replacement; graph checkpointed with each durable publication and warm-restart
loaded, never reconstructed heuristically; the `InvalidationPlan` DTO (`LIFE §14.3`)
with `capability_withdrawals` computed **at admission** so invalidated semantic facts
are hidden before current syntax publishes (`LIFE §94.2` + `§99.1` + `§103` +
`§157` inv. 4).

#### Dependencies

WP42.

#### Target Invariants

I-06, I-31, I-34; `LIFE §157` inv. 4, 7, 9. Doctrine P14, P25 advance;
anti-principle "scattered dirty flags" guarded (single graph authority).

#### Design and Library References

`LIFE §14`–`§16`, `AC-G-41`; `FAB §13.10` capability rows; the Wave-6 fact families
present at this point (SOURCE, RAW_SYNTAX, TYPED_SYNTAX + the W5 slice families).

#### Change Surface

##### Preflight Query

```bash
rg -n 'operational_ddl_digest' src/operational_store.rs      # DDL change mechanism
rg -n 'InvalidationPlan' src/ contracts/                     # expect zero before
```

##### Known Touch (verified this session)

`contracts/schema/operational-store.sql` source + regeneration, invalidation
module(s), `src/operational_store.rs`, `src/coordinator.rs` admission seam.

#### Required Changes

DDL extension through the contracts pipeline; graph store + update algorithm;
propagation with escalation; admission-time withdrawal computation feeding overlay
construction; checkpoint/restore.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp43_behavioral_acceptance`.

seeded changes propagate exactly along declared edges
and stop at unchanged fingerprints; threshold fixtures escalate correctly; a callee
body edit does not rewrite incoming call-site edges absent an interface change.

##### Structural

Executable oracle: `wp43_structural_acceptance`.

warm restart loads the checkpoint and applies only the
current-source delta (no heuristic reconstruction — fixture proves identical plans);
inactive edges absent after owner replacement.

##### Negative / Zero-State

Executable oracle: `wp43_negative_zero_state`.

a wave whose invalidation plan withdraws capabilities
cannot publish syntax-current state that still exposes the invalidated semantic rows
(fence fixture — the `§157` inv. 4 oracle).

##### Operational

Executable oracle: `wp43_operational_acceptance`.

plan sizes, edge counts, escalations, checkpoint
timings.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on propagation stop/escalation logic.

#### Integration Milestone

M07.

#### Replan Triggers

An 11-kind edge vocabulary gap for a real dependency → registry append (AC-G-41 is
explicit that kinds are registry data); graph write volume breaks SQLite WAL
assumptions → performance adaptation (batch/checkpoint tuning), not schema fork.

#### Rollback or Recovery

Graph is derived state: full inventory reconcile rebuilds it.

### WP44 — Fast syntax lane

#### Outcome

The `LIFE §94` fast lane: stable source recapture → incremental Tree-sitter where
the WP30 capability applies (full parse where not) → current source/syntax facts,
parse errors, token/trivia, likely owner boundaries, source-to-owner mapping, and
capability withdrawals — publishing a syntax-current immutable snapshot **before**
semantic providers finish, which removes invalidated semantic facts from visibility,
retains unaffected owners, and marks pending/unavailable capabilities (`§94.3`,
I-34); the `LIFE §99.1` eight fast-validation checks gate publication (digest,
identity, fence, Arrow schema, span bounds, deterministic owner IDs, complete
withdrawals, no stale generation); the `LIFE §17` safe-reuse fast path with its six
conditions and `REANCHORED_UNCHANGED_SEMANTICS` labeling (its `§6.12` carve-outs
respected) for retained facts; `LIFE §132` latency stages instrumented with the
`§133` sub-second fast-visibility target measured (not SLO-gated until W19).

#### Dependencies

WP42 (including its extended `UpdateWaveState` vocabulary, which supplies this
lane's `FAST_ANALYZING`/`FAST_VALIDATING`/`FAST_PUBLISHED` states), WP43, and
WP30's incremental-parse capability.

#### Target Invariants

I-31, I-34; `LIFE §157` inv. 4–5, 7. Doctrine P17, P26 advance.

#### Design and Library References

`LIFE §93`–`§94`, `§99.1`, `§17`, `§6.12`, `§132`–`§133`, `§23` capability codes;
WP30's incremental capability + `GEN §94` equivalence.

#### Change Surface

##### Preflight Query

```bash
rg -n 'OverlayMutation|owner_tombstone' src/fabric/overlay.rs | head
rg -n 'REANCHORED' src/ contracts/                  # expect zero before
```

##### Known Touch (verified this session)

Fast-lane module(s) wiring WP30/WP31 adapters into overlay construction;
`src/fabric/overlay.rs` consumers; latency metrics module.

#### Required Changes

Lane orchestration inside the wave lifecycle (`FAST_ANALYZING`→`FAST_VALIDATING`→
`FAST_PUBLISHED`); withdrawal-bearing overlay assembly; the eight-check validator;
reuse/reanchoring; instrumentation.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp44_behavioral_acceptance`.

an edit produces a syntax-current snapshot before the
(stubbed-slow) semantic lane completes, with pending capabilities visible and stale
semantic rows hidden; parse-break then parse-fix sequences converge; trivia-only
edits reuse semantics with `REANCHORED_UNCHANGED_SEMANTICS` unless a `§6.12`
carve-out applies.

##### Structural

Executable oracle: `wp44_structural_acceptance`.

all eight §99.1 checks enforced (each has a violation
fixture that blocks publication); incremental parse path equals clean parse on the
lane's edit corpus.

##### Negative / Zero-State

Executable oracle: `wp44_negative_zero_state`.

no syntax-current snapshot ever exposes an invalidated
semantic fact as current (`§157` inv. 4 lane oracle); reuse never fires when any of
the six conditions fails (condition-matrix fixtures).

##### Operational

Executable oracle: `wp44_operational_acceptance`.

`watch_to_dirty`/`dirty_to_source`/`source_to_fast`
stage timings recorded; fast-visibility measurement captured for the report (no
stored benchmark claims in state — v5 D-21).

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on the reuse-condition logic.

#### Integration Milestone

M07.

#### Replan Triggers

Incremental parsing unsafe for a discovered edit class → fall back to full parse for
that class (implementation adaptation; equivalence check keeps it honest).

#### Rollback or Recovery

Lane failure keeps the prior snapshot active (`LIFE §159` inv. 6).

### WP45 — Freshness and barrier state machine (LIFE AC-G-24/AC-G-25)

#### Outcome

The formal freshness machine: four workspace counters plus
`owner_capability_generation[owner, context, capability]`; freshness determined by
admitted event sequence, reconciliation sequence, and generations — never elapsed
time; the five structured policies (`LIFE §124`) with `REQUIRE_CURRENT_FOR_TARGETS`
as default and its candidate-lease loop (lease after source barrier, resolve targets
inside the lease, release without exposure if pending, retry to deadline, single
final lease for binding/planning/execution/artifacts); the `LIFE §125` barrier and
normative pin order in query admission; deadline expiry →
`FRESHNESS_DEADLINE_EXCEEDED` with barrier/progress metadata; overflow blocks
strict-current completion until reconciliation reaches the barrier; `AC-G-25`
conformance — all eleven machines runtime-validated from generated tables, illegal
transitions raise `STATE_TRANSITION_VIOLATION`, model checks prove terminal
reachability and no unintended cycles; `LIFE §126` `PublicSnapshotMetadata` fully
populated; `LIFE §127` eight distinct empty-result codes (a compile failure is never
a normal empty call graph); `LIFE §128` source-fallback delivery; `LIFE §129`
bounded multi-agent fairness; D-29 executed for the `FreshnessState` vocabulary.
**DB08 exits here**: the D-41 Wave-5 freshness shim reaches zero.

#### Dependencies

WP42, WP43, WP44.

#### Target Invariants

I-02, I-31, I-33; `LIFE §157` inv. 1–3, 8; `SUITE §0.2` inv. 8. Doctrine P12, P16,
P21, P23 advance.

#### Design and Library References

`LIFE AC-G-24`, `AC-G-25`, `§19`, `§23`–`§25`, `§124`–`§129`; `ONT §62.7`–`§62.9`;
generated state machines (session-verified); D-29, D-41, DB08.

#### Change Surface

##### Preflight Query

```bash
rg -n 'FreshnessState' src/generated/registries.rs -A6       # current vocabulary
rg -n 'D-41|freshness' src/ --type rust -l                    # shim sites from WP38/39
rg -n 'model.check|terminal' tooling/ contracts/ -l           # existing machine checks
```

##### Known Touch (verified this session)

Freshness module(s); `src/coordinator.rs` + WP39 service admission seams;
`src/generated/registries.rs` (regenerated if D-29 appends); shim removal sites in
the WP38/WP39 modules.

#### Required Changes

Counters + barrier; policy evaluation incl. the candidate-lease loop over
`SnapshotLeaseManager`; admission integration; metadata population; empty-result
code mapping; model-check tests over the generated transition tables; shim removal.

#### Legacy Disposition and Decommission

**DB08 exit**: the quiescent-regime shim is deleted; negative check below.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp45_behavioral_acceptance`.

policy matrix fixtures — each of the five policies
against current/pending/stale/overflow workspace states yields the AC-G-24 required
condition outcomes; candidate-lease loop releases non-satisfying candidates without
exposure; deadline and cancellation paths.

##### Structural

Executable oracle: `wp45_structural_acceptance`.

model checks green for all eleven machines; every
freshness decision cites sequences/generations only (no time-based predicate in the
decision path — structural assertion).

##### Negative / Zero-State

Executable oracle: `wp45_negative_zero_state`.

**DB08 oracle**: the D-41 shim is absent (`rg`
zero-hit on its marker + green build); no query admission path bypasses the barrier
(governance: admission chokepoint assertion).

##### Operational

Executable oracle: `wp45_operational_acceptance`.

barrier wait histograms, policy counts, deadline
expiries, fairness counters.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just mutants-file` on policy-condition evaluation.

#### Integration Milestone

M07.

#### Replan Triggers

AC-G-24's required-condition table demands a counter the implementation cannot
maintain O(1) → performance adaptation; a needed `FreshnessState` value is
unappendable → design reopening via SI-06.

#### Rollback or Recovery

Barrier is admission-side; disable only by reverting the packet.

### WP46 — Continuous overlay rebase and durable flush (FAB AC-G-22)

#### Outcome

The threshold-driven continuous durable loop over the Wave-3 kernel: the six
`LIFE §107` flush triggers (age/rows/bytes/changed-owners/idle/shutdown-drain); the
`AC-G-22` three-snapshot protocol — `S_old = P_n + O_n`, capture `O_flush`, build
and validate `P_(n+1)` equivalent to `S_old`, accept newer waves into `O_delta`,
CAS the pointer, rebase `O_delta` onto the new base, **validate the effective digest
unchanged**, activate `S_new`; `O_flush` rows dropped in rebase only when their
logical content digest exists in the new base; failed CAS or digest mismatch aborts
and restarts from the newly current base; highest-accepted-generation-wins
consolidation with `OVERLAY_GENERATION_CONFLICT` on equal-generation payload
divergence; multi-wave merge preserving later generations (`LIFE §104`); the
`AC-G-26` ordering (durable operational transaction commits before the in-memory
swap); no durable-flush race makes a generation disappear or appear twice; overlay
consolidation (never chains) and no Delta micro-files under small updates
(`LIFE §158` inv. 9–10).

#### Dependencies

WP42 (waves feed overlays), WP44; the Wave-3 kernel and WP40's scenario-scoped
rebase.

#### Target Invariants

I-02, I-09, I-14, I-33; `LIFE §157` inv. 9–10, 19–21; `LIFE §159` inv. 2–3.
Doctrine P19, P24 advance.

#### Design and Library References

`FAB AC-G-22` (the sole home of the three-snapshot protocol), `AC-G-26`, `§71`;
`LIFE §104`, `§106`–`§107`; existing `OverlayRebaseRequest`/`execute_rebase` and
fault-point enums (session-verified).

#### Change Surface

##### Preflight Query

```bash
rg -n 'OverlayRebaseFaultPoint|execute_rebase' src/fabric/overlay.rs
rg -n 'DurablePublicationState|ServingActivationState' src/generated/registries.rs | head
```

##### Known Touch (verified this session)

Flush-policy module; `src/fabric/overlay.rs`, `src/fabric/publication.rs`,
`src/snapshot_runtime.rs` consumers; `src/coordinator.rs` scheduling seam.

#### Required Changes

Trigger evaluation; the continuous loop with watermark capture, equivalence
validation, CAS/retry, rebase, digest check; conflict surfacing; fault-point
coverage for every step (`RM §27.3`).

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp46_behavioral_acceptance`.

sustained edit streams flush at each trigger class;
concurrent waves during flush land in `O_delta` and survive rebase; effective state
before and after flush is digest-identical; generation-divergence fixture raises
`OVERLAY_GENERATION_CONFLICT`.

##### Structural

Executable oracle: `wp46_structural_acceptance`.

fault injection at every protocol step (via the
existing fault-point enums) leaves either the old or the new consistent state —
never a partial pointer/overlay mix; no generation disappears or duplicates across a
forced CAS race.

##### Negative / Zero-State

Executable oracle: `wp46_negative_zero_state`.

no overlay chain deeper than base+one-consolidated at
any observation point; no micro-file storm under the small-update fixture (Delta
version-count assertion).

##### Operational

Executable oracle: `wp46_operational_acceptance`.

flush trigger counts, rebase durations, conflict/
abort/retry counters, overlay memory.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just wave3-integration-check`; `just mutants-file` on trigger +
rebase-drop decisions.

#### Integration Milestone

M07.

#### Replan Triggers

Equivalence validation cost makes continuous flush impractical at the fixture scale
→ performance adaptation (incremental digest maintenance), protocol unchanged.

#### Rollback or Recovery

Protocol is abort-and-restart by construction; `ABANDONED` publications cleaned by
WP47 recovery.

### WP47 — Startup and crash recovery

#### Outcome

The `LIFE §5` scenario set for the non-Git-accelerated profile: cold start
(19-step, `WORKSPACE_BOOTSTRAPPING` before first snapshot), warm start unchanged
(recovered `ServingSnapshot` leased for readiness validation, inventory digest
verified, events replayed), warm start with offline changes (never assume downtime
events exist), orphan staging publications (`§5.5`: preserve the pointer, resume
only idempotent known-safe work, else mark `ABANDONED`, reconcile source
independently, clean up after pinned-version safety), crash with unflushed overlay
(`§5.6` + D-40: rebuild from source; old durable base queryable only as
`POTENTIALLY_STALE`; strict-current waits or fails explicitly; never a false
current claim — I-33); the `AC-G-28` ten public startup states with usable/current
criteria and the `BEST_AVAILABLE_STALE` gating rule (only `BEST_AVAILABLE_SNAPSHOT`
queries complete); the `LIFE §154` nine-condition readiness barrier; `LIFE §151`–
`§153` shutdown ordering, drain, and cancel; the `LIFE §11.3`–`§11.5` crash-window
behaviors re-proven under the continuous loop.

#### Dependencies

WP41–WP46.

#### Target Invariants

I-14, I-25, I-31, I-33; `LIFE §159` inv. 1–4, 6, 8–9. Doctrine P22, P23, P24
advance.

#### Design and Library References

`LIFE §5`, `AC-G-28`, `§108.1`/`§108.3` (D-40), `§151`–`§154`, `§11.3`–`§11.5`;
existing `orphan_after_restart`/`recover` seams (session-verified).

#### Change Surface

##### Preflight Query

```bash
rg -n 'orphan_after_restart|restore_and_bootstrap|recover' src/ --type rust -l
rg -n 'StoreFaultPoint|MutationFaultPoint' src/ --type rust -l   # fault hooks
```

##### Known Touch (verified this session)

`src/coordinator.rs`/`src/daemon.rs` startup paths, `src/snapshot_runtime.rs`,
`src/fabric/publication.rs` recovery consumers, startup-state surface in the status
API.

#### Required Changes

Startup state machine over recovery steps; scenario implementations; readiness
barrier; shutdown drain/cancel; crash-injection harness using the existing
fault-point enums at every persistence boundary this plan added.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp47_behavioral_acceptance`.

each §5 scenario converges to the correct advertised
state and serving behavior; offline-edit warm start detects and reconciles without
watcher events; orphan-staging fixtures resume or abandon exactly per rule.

##### Structural

Executable oracle: `wp47_structural_acceptance`.

crash injection at every publication/activation/
flush/checkpoint boundary leaves recoverable state (process-kill matrix over the
fault points); `BEST_AVAILABLE_STALE` admits only opt-in stale queries.

##### Negative / Zero-State

Executable oracle: `wp47_negative_zero_state`.

a lost overlay never yields a false current claim
(strict-current query against a post-crash workspace fails/waits until
reconciliation — the I-33 oracle); no recovery path resets `source_generation`.

##### Operational

Executable oracle: `wp47_operational_acceptance`.

startup state transitions and durations; recovery
counters; drain/cancel behavior on shutdown.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; crash-matrix suite (packet-owned recipe extension if needed).

#### Integration Milestone

M07.

#### Replan Triggers

A recovery scenario requires journaling beyond D-40's no-journal policy → design
reopening (the §108.2 option needs a decision), not silent scope creep.

#### Rollback or Recovery

This packet *is* recovery; its own failures keep prior snapshots active.

### WP48 — Clean-rebuild comparator, core edit corpus, and CORE_SOURCE_V1 (M07)

#### Outcome

The complete `SUITE AC-G-79` comparator: `ComparisonInput` domain checks
(`COMPARISON_DOMAIN_MISMATCH` before reading facts; independent source-current
verification else `SNAPSHOT_FRESHNESS_MISMATCH`); effective-state materialization
using the same overlay composition as serving (comparing only the durable base when
an overlay exists is prohibited); the ignore registry
(`contracts/comparison/comparison-ignore-registry.yaml`) as the only field-exclusion
authority; canonical row/table/state digests with the specified BLAKE3 domain
prefixes; the nine outcomes with **no numeric tolerance**; the `§79.6` difference
artifact (first 100 differing rows per table, full difference stream, CBEF
preimages, reproduction command, ACL-scoped); the `§79.7` gate — every golden
scenario freezes inventory, builds a clean snapshot in an isolated namespace,
compares, and releases only after the artifact is durable, including the
corrupted-overlay (must reject) and changed-ignored-field (must accept) conformance
cases; the core edit corpus — `LIFE §138`'s Wave-6 subset (isolated/repeated/atomic
save, add/delete/rename/move, directory removal, whitespace-only, parse break/fix,
formatter burst `§6.13`, generated burst `§6.14`, overflow/rescan, restart, crash at
publication boundaries) expressed in `AC-G-78 §78.9` mechanics as a corpus minor
version; every scenario converging incrementally equal to clean rebuild
(`LIFE §137` oracle, `§157` inv. 32); `CORE_SOURCE_V1` advertised `COMPLETE` with
per-owner coverage via `coverage_scope_fingerprint` and `capability_summaries`
(`ONT AC-G-72`: never advertised merely because some owners implement it); recipes
`rebuild-equivalence-check` and `wave6-integration-check`; the Wave-5 scenarios
re-run under the full comparator (closing D-30's staging). Gate C remains formally
open (`RM §11`). M07.

#### Dependencies

WP41–WP47 all complete.

#### Target Invariants

I-08, I-09 (the packet mechanizes it), I-24; `LIFE §137`; `SUITE AC-G-79` in full.
Doctrine P25, P27, P30 advance.

#### Design and Library References

`SUITE AC-G-79 §79.1–§79.7`, `AC-G-78 §78.9–§78.10`; `LIFE §137`–`§138`, `§6.13`–
`§6.14`, `§139`–`§140`, `§142`; `ONT AC-G-72`, `§62.4`; D-30.

#### Change Surface

##### Preflight Query

```bash
cat contracts/comparison/comparison-ignore-registry.yaml      # 32 records today, already a superset of the §79.3 initial list
rg -n 'effective_state_digest' src/fabric/snapshot_catalog.rs
ls tests/golden/codefabric-golden-v1/scenarios/               # W5 scenario set
```

##### Known Touch (verified this session)

Comparator module(s); `contracts/comparison/` registry consumers; corpus scenario
additions (minor version); `justfile` (+2 recipes); isolated-namespace build path in
`src/fabric.rs` consumers.

#### Required Changes

Comparator implementation; canonical row-projection/sort-key data in the schema
registry (Contract IR extension if absent); difference-artifact writer; corpus
scenarios; profile advertisement wiring; recipes.

#### Legacy Disposition and Decommission

Closes D-30 staging: the bounded W5 state-digest check is subsumed (removed or
delegated to the full comparator; negative check below).

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp48_behavioral_acceptance`.

Wraps `just rebuild-equivalence-check`: every core-corpus
scenario and every Wave-5 scenario ends `EQUIVALENT`; the corrupted-overlay case
rejects; the ignored-field case accepts.

##### Structural

Executable oracle: `wp48_structural_acceptance`.

comparator uses serving-identical overlay composition
(shared code path assertion); ignore-registry entries are the only excluded fields
(schema-driven diff of compared columns); difference artifact contains all nine
required contents on a forced mismatch.

##### Negative / Zero-State

Executable oracle: `wp48_negative_zero_state`.

`CORE_SOURCE_V1` cannot advertise `COMPLETE` while any
mandatory capability is missing for any supported owner (withdrawal fixture flips
the profile to `PARTIAL` with named codes); the W5 bounded check no longer exists as
an independent oracle.

##### Operational

Executable oracle: `wp48_operational_acceptance`.

`just wave6-integration-check` — exit 0; comparator run/difference metrics.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just rebuild-equivalence-check`; `just wave6-integration-check`;
`just mutants-file` on canonical row encoding.

#### Integration Milestone

M07 (ceremony packet).

#### Replan Triggers

A scenario cannot converge without Git awareness (e.g. rename fidelity) → record the
Git-conditional expectation and defer that scenario variant to Wave 7's corpus
extension — never weaken the comparator; an ignore-registry addition is required →
reviewed requirement + minor version per `§79.3`, not a silent edit.

#### Rollback or Recovery

Comparator is read-only over snapshots; corpus versions are immutable.

## 10. Work packets — Wave 7 (Git-aware lifecycle acceleration and topology)

The five packets map 1:1 to `LIFE §§88–92`'s phases — the only spec-internal phase
sequence that matches its wave (`RM §29` row 7). Authority is fixed throughout:
filesystem bytes and CodeFabric BLAKE3 digests remain source truth; gix produces
candidates, never facts (`LIFE §2.1`, `§54`, `§157` inv. 12–15); every Git-derived
candidate set is fenced by its captured `GitStateVector` and rejected when the
baseline moved (`LIFE §73`, `§157` inv. 18); gix failure degrades acceleration, never
correctness (`LIFE §80`, `§159` inv. 5/7/13). All work stays inside the `git_state`
module boundary behind `repository-state` (D-26), under the existing
`gix-boundary-only`/`gix-read-only` rules and the `LIFE §76` local-workstation trust
defaults (network/credentials/filters/hooks/mutation/checkout disabled; environment
overrides rejected; global/system Git config not consulted; ignore rules never
authorization).

### WP49 — Phase 1: Repository correctness

#### Outcome

The `GitStateAdapter` surface completed to `LIFE §156`'s five detached-DTO methods
(`open_worktree`, `capture_state`, `inventory`, `status_candidates`,
`tree_diff_candidates` — no `gix::Repository`, attached objects, references, index
handles, or thread-local provider state escapes); the full 12-field
`GitStateVector` (`LIFE §50`) with `GitStateObservations` bound from the policy
compiler and inventory boundary (the adapter binds them, never recreates them);
hash-algorithm-agnostic `GitObjectId` (`§50.1`); `GitOperationState` (`§51`) with
the `§51.1` lifecycle effects (no indefinite waits; aggressive grouping; delayed
durable flush; strict-current still answers from current worktree bytes); HEAD
states incl. unborn/detached/non-commit (`§58`); bare repositories recognized with
source capability unavailable (`§42`); discovery exclusively through repository/
worktree APIs — never string-concatenated `.git` paths (`§41`); the `§69` handle
-model decision executed and benchmarked against gix 0.86 ownership semantics
(one of the four permitted options; never a shared `Arc<gix::Repository>` — `§87`);
the `§76` trust profile constructed and fingerprinted; the LD-25 resolved
feature-graph confirmation recorded; D-33's DTO authorship (`HeadKind`,
`GitPathChangeClass`, `GitRenameEvidence`, `SubmoduleCandidate`,
`GitInventoryResult`, `GitTreeDiffOptions`, `GitTrustPolicy`, …) inside the
boundary; `LIFE §43` path forms prevail over Appendix F's lossy restatement.

#### Dependencies

M07.

#### Target Invariants

I-03, I-10, I-32; `LIFE §157` inv. 24–27, 29–31; `LIFE §159` inv. 12 (forbidden Git
behavior fails closed). Doctrine P6, P8, P13 advance.

#### Design and Library References

`LIFE §§37–44`, `§50`–`§51`, `§58`, `§69`, `§76`, `§78`, `§87`, `§156`, Appendix E/F;
D-26, D-33, D-34; LD-25. Library ref: `gix_rust_advanced_reference.md` via
`gix-notify-ref` (mandatory pre-read for handle/ownership semantics).

#### Change Surface

##### Preflight Query

```bash
ast-grep outline src/git_state.rs --items exports              # current boundary
rg -n 'GitStateVector|capture_state' src/git_state.rs | head
cargo tree -p gix --edges features | head -30                   # LD-25 verification
rg -n 'Arc<.*Repository' src/ --type rust                       # expect zero
```

##### Known Touch (verified this session)

`src/git_state.rs` (+submodules), `Cargo.toml` (gix feature list completion per
Appendix E), `rules/` (rule fixtures extended for new DTO surface),
`tests/integration/git_state.rs`.

#### Required Changes

Extend the adapter to the full vector/operation/HEAD surface; execute + record the
handle-model benchmark decision; construct the trust profile with explicit
fingerprint; complete the Appendix E feature manifest with the resolved-graph proof;
author the missing DTOs.

#### Legacy Disposition and Decommission

None; Wave-2 topology discovery is extended in place.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp49_behavioral_acceptance`.

`LIFE §145.1` topology fixtures (normal, bare, unborn
HEAD, detached, non-commit target, linked worktrees, worktree removal, common-dir
sharing) produce exact expected snapshots/vectors; operation-state fixtures
(merge/rebase/cherry-pick) map correctly with `UNKNOWN` fallback.

##### Structural

Executable oracle: `wp49_structural_acceptance`.

adapter methods return detached DTOs only (structural
rule extension of `gix-boundary-only` covering the new surface, with near-miss
fixtures); handle-model benchmark evidence recorded (command + environment, no
stored claim in state).

##### Negative / Zero-State

Executable oracle: `wp49_negative_zero_state`.

zero `Arc<gix::Repository>` sharing (`§143`-class
handle-safety test + rule); no string-built `.git`/worktree paths (`rg` + rule over
the boundary); forbidden trust capabilities (network, filters, hooks, credential
helpers) fail closed in fixture (`§145.6` "external filter configured but
forbidden").

##### Operational

Executable oracle: `wp49_operational_acceptance`.

`gix_open`/`capture` duration + error-class metrics
(`LIFE §148` subset).

#### Edit-Local Gates

Targeted nextest incl. `tests/integration/git_state.rs` extension.

#### Packet-Local Gates

`just ci-fast`; `just governance-scan`.

#### Integration Milestone

M08.

#### Replan Triggers

gix 0.86 ownership semantics make every permitted handle option fail the benchmark
→ design reopening against `LIFE §69`; the resolved feature graph cannot supply a
required capability under Appendix E's profile → LD-25 revision + spec issue.

#### Rollback or Recovery

Adapter extension is additive; `GIT_UNAVAILABLE` degrades to Wave-6 behavior.

### WP50 — Phase 2: Git-native inventory

#### Outcome

Git-native worktree inventory: the 8-class inclusion taxonomy (`LIFE §46`) with the
default table (tracked wins over later ignore; untracked-ignored excluded; `.git`
internals excluded from source but selectively watched) and the hard rule that
ignore rules are never authorization; gix dirwalk/exclude/attribute stacks for
cold-start and authoritative rescans with the `§47.1` bounds (file count, depth,
bytes, time, cancellation, symlink policy) and generic-walker fallback; attribute
evaluation without filter execution (`§48`), attribute changes triggering
`INCLUSION_OR_CONTENT_POLICY` reconciles; the `§49` inclusion-policy fingerprint
with its five-step change handling (no semantic regeneration for still-included
unchanged files); the `§52` metadata watch set (per-worktree HEAD/index/operation
paths, configs, symbolic-ref target, packed-refs indicators, excludes, attribute
sources, `.gitmodules`, worktree registrations — never the whole ODB or all refs)
resolved through repository APIs and delivered as coalesced `GitStateChange`
categories through WP41's facade (`§53`: raw metadata order is never authoritative
— each category forces a reopen/re-read); the `§45` six-level file-identity
evidence ladder; nested repositories classified per `§67` and never traversed
without explicit policy.

#### Dependencies

WP49, WP41 (facade).

#### Target Invariants

I-03, I-32; `LIFE §157` inv. 13–16; ignore ≠ authorization (`§46`). Doctrine P8,
P11 advance.

#### Design and Library References

`LIFE §§45–49`, `§52`–`§53`, `§67`, `§145.2` fixtures; `AC-G-11` path checks
("gix path reads remain advisory acceleration evidence and are revalidated").

#### Change Surface

##### Preflight Query

```bash
rg -n 'inventory' src/git_state.rs src/inventory.rs | head     # both walkers
rg -n 'GitMetadata|GitStateChange' src/ --type rust             # facade seam from WP41
rg -n 'inclusion_policy_fingerprint' src/ contracts/
```

##### Known Touch (verified this session)

`src/git_state.rs` inventory surface, `src/inventory.rs` (fallback parity seam),
watch-set integration in the WP41 module, `src/secure_path.rs` consumers (all paths
revalidated through the authorization boundary).

#### Required Changes

Dirwalk-backed inventory with bounds + fallback; inclusion classification; both
fingerprints with change-driven reconciles; metadata watch registration + event
categorization; identity-ladder plumbing into WP42's rename policy.

#### Legacy Disposition and Decommission

None — the generic walker remains the mandatory fallback, never removed.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp50_behavioral_acceptance`.

`§145.2` fixtures (nested `.gitignore`,
`info/exclude`, trusted global excludes, tracked-ignored, ignored-untracked,
CodeFabric override, pathspec include/exclude, case-only rename, non-UTF8 path
[platform-gated], directory/file transition, symlink) classify exactly;
`.gitignore`/attribute edits trigger precisely the five-step §49 handling.

##### Structural

Executable oracle: `wp50_structural_acceptance`.

gix inventory and the generic walker agree on the
included set for every fixture (fallback-parity oracle); every watched path came
from a repository API, none from string construction.

##### Negative / Zero-State

Executable oracle: `wp50_negative_zero_state`.

an ignore rule can never widen or narrow
*authorization* (fixture: ignored-but-authorized file still readable through
`secure_path`; unauthorized path never enters inventory regardless of Git state);
no `.git` internal enters the source CPG.

##### Operational

Executable oracle: `wp50_operational_acceptance`.

dirwalk durations/entry counts, watch-set size,
policy-change reconcile counters.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just governance-scan`.

#### Integration Milestone

M08.

#### Replan Triggers

gix dirwalk cannot express a required bound or classification → use the generic
walker for that case (fallback is always available; record the gap as a gix
upgrade-gate item per `LIFE §147`).

#### Rollback or Recovery

`GIT_DEGRADED` → generic inventory, identical semantic state.

### WP51 — Phase 3: Status and index acceleration

#### Outcome

Status as a bounded accelerator for warm startup, authoritative rescan, overflow
recovery, bulk storms, metadata-triggered baselines, and periodic audit — never the
final proof of currentness (`LIFE §54`); the 12 `GitPathChangeClass` candidate
classes (`§54.1`); the 9-step reconcile algorithm with G0/G1 capture and
retry-or-broaden when HEAD/index/state moved mid-scan (`§55`), unioned with
watcher-watermark dirty paths, every candidate resolved through stable
`SourceImage` capture + digest comparison before publication; the D-31 periodic
generic audit (post-bulk + configurable idle interval; divergence → authoritative
rescan + `GIT_DEGRADED`); index integration (`§56`): index-only changes update the
`GitStateVector` with **no source or semantic regeneration** (the CPG represents
the worktree, not the staging area), conflict stages surface `GIT_CONFLICTED`
while bytes remain authority and analysis proceeds if parsable, external index
writers force reopen/reload with no attached-object retention; sparse-index support
verified before reliance with authoritative-inventory fallback and CLI parity
fixtures precondition (`§57`); the `§85` prohibition holds — ordinary isolated
saves never trigger status scans (hot path stays event → stable read → digest →
local invalidation); the `§146` Git CLI parity oracle against a pinned git
executable (`rev-parse`, `status --porcelain=v2 -z`, `ls-files -z`,
`check-ignore -z`, `diff-tree --raw -z`, `worktree list --porcelain`,
`submodule status`; byte-safe NUL parsing; test oracle only, never a hot-path
dependency) behind a new `git-parity-check` recipe.

#### Dependencies

WP50.

#### Target Invariants

I-03, I-32; `LIFE §157` inv. 12, 17–18, 28; `LIFE §158` inv. 8 ("full gix status
does not run for every isolated save" — the §85 rule). Doctrine P8, P24 advance.

#### Design and Library References

`LIFE §§54–57`, `§85`, `§80`, `§145.3`, `§146`, `§148` (`candidate_pruning_ratio`);
D-31; `GitCandidateDelta` (D-33).

#### Change Surface

##### Preflight Query

```bash
rg -n 'status_candidates' src/git_state.rs                     # WP49 stub surface
git --version                                                   # parity oracle pin
rg -n 'porcelain' tests/ tooling/                               # expect zero before
```

##### Known Touch (verified this session)

`src/git_state.rs` status/index surface, reconcile integration in `src/coordinator.rs`
(WP42 seam), parity harness under `tests/integration/`, `justfile`
(+`git-parity-check`).

#### Required Changes

Bounded status jobs through the blocking executor; candidate normalization;
reconcile-with-retry; audit scheduler; index snapshot/fingerprint handling; parity
harness with pinned-git recording; pruning-ratio metric.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp51_behavioral_acceptance`.

`§145.3` fixtures (staged-only, unstaged, both,
untracked, deleted, exec-bit-only, symlink mode, conflict stages, external index
writer, corrupt/truncated index) converge to clean-rebuild-equal state; mid-scan
HEAD/index movement forces retry/broaden (`§140` fixture: stale candidate delta
cannot commit).

##### Structural

Executable oracle: `wp51_structural_acceptance`.

`just git-parity-check` — classification agreement
with the pinned Git CLI across the fixture corpus; index-only staging produces zero
source/semantic regeneration (provider-run count assertion).

##### Negative / Zero-State

Executable oracle: `wp51_negative_zero_state`.

the isolated-save hot path performs zero gix status
calls (instrumented counter oracle — the `§85`/M08 exit criterion); no candidate
delta with a mismatched baseline vector commits.

##### Operational

Executable oracle: `wp51_operational_acceptance`.

`candidate_pruning_ratio` per trigger class
(warm start, branch switch, watcher-loss, formatter burst, ordinary save); status
durations; audit outcomes.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just git-parity-check`; `just mutants-file` on the reconcile
retry/broaden decision.

#### Integration Milestone

M08.

#### Replan Triggers

gix status semantics diverge from the pinned CLI on a fixture → the CLI is the
oracle; treat as a gix defect: degrade that class to generic reconcile and record
in the `§147` upgrade gate — never ship a divergent classification.

#### Rollback or Recovery

Acceleration-off fallback preserved by construction.

### WP52 — Phase 4: Bulk HEAD-tree acceleration

#### Outcome

Branch-switch and bulk-transition acceleration: tracked baseline (`§58`); the `§59`
pipeline — old-tree→new-tree gix diff candidates (add/delete/modify/mode) unioned
with current worktree status because tree diff alone is insufficient for the five
`§59.1` reasons (local modifications, untracked files, conflict state,
filter/line-ending effects, concurrent watcher events) — resolved through
authoritative reads + digest comparison into one bulk update wave; bounded rename
detection only under the `§60` conditions with full verification before identity
transfer (similarity is evidence, not identity); mode-only/symlink/file-kind
transitions per `§61` (exec-bit metadata-only; symlink target/type → full
source-policy verification; file↔directory → subtree reconcile); the D-32-hardened
`§62` stabilization tuple (HEAD target, HEAD tree, index fingerprint, operation
state, event watermark, dirty-path set, **plus inclusion-policy and attributes
fingerprints**) gating full semantic/durable publication, with the fast syntax lane
still allowed to publish earlier with pending-semantic capability metadata; `§73`
staleness rules — attached values never cross update waves, candidate deltas with
moved baselines rejected; the `LIFE §5.4` warm start after HEAD/worktree baseline
transition.

#### Dependencies

WP51.

#### Target Invariants

I-32, I-34; `LIFE §157` inv. 18, 30; `LIFE §159` inv. 5; rename similarity is
evidence, never identity, per `LIFE §60` and the `§45` ladder. Doctrine P13, P24
advance.

#### Design and Library References

`LIFE §§58–62`, `§73`, `§5.4`, `§145.4` fixtures (incl. "HEAD moves but worktree
byte-identical"), `§140`; D-32 (SI-09), D-33.

#### Change Surface

##### Preflight Query

```bash
rg -n 'tree_diff_candidates' src/git_state.rs
rg -n 'bulk' src/coordinator.rs | head                          # WP42 bulk seam
rg -n 'rename' src/git_state.rs src/coordinator.rs | head       # identity ladder seam
```

##### Known Touch (verified this session)

`src/git_state.rs` tree-diff surface, bulk-wave orchestration in the WP42
scheduler, rename verification against WP50's identity ladder.

#### Required Changes

Tree-diff jobs; union-with-status candidate assembly; stabilization-tuple capture/
recheck; bounded rename proposals + verification; mode/symlink handlers; §5.4
startup variant.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp52_behavioral_acceptance`.

`§145.4` fixtures — branch switch with no local
changes / preserving a local modification / with untracked files; large rename set;
checkout event storm; HEAD-moves-worktree-identical (zero regeneration beyond
vector update); operation-state changes — all converging clean-rebuild-equal.

##### Structural

Executable oracle: `wp52_structural_acceptance`.

a mid-gather `.gitignore` change destabilizes the
tuple and defers full publication (D-32 oracle); an accepted rename transfers
identity only after digest/language/module verification.

##### Negative / Zero-State

Executable oracle: `wp52_negative_zero_state`.

no tree-diff candidate bypasses current-byte
verification into published state (fence fixture); no unbounded rename detection
above the limit (bound assertion).

##### Operational

Executable oracle: `wp52_operational_acceptance`.

bulk wave sizes/durations, rename candidate/accept
counts, tuple-instability retries.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just git-parity-check`; `just rebuild-equivalence-check` on the
branch-switch scenarios.

#### Integration Milestone

M08.

#### Replan Triggers

Tuple instability under real formatter/checkout storms makes bulk waves never
stabilize → adaptation via the `§51.1` aggressive-grouping allowance and bulk
gather bounds; if insufficient, spec issue against `§62`.

#### Rollback or Recovery

Bulk path degrades to `U7` full reconcile (`LIFE §13`), which Wave 6 proved.

### WP53 — Phase 5: Shared caches, topology, degradation, and Wave-7 equivalence (M08)

#### Outcome

The cache hierarchy with authority intact: L1 BLAKE3 worktree-digest (authoritative)
→ L2 blob OID + worktree-transform fingerprint + provider bundle (accelerator for
clean tracked files only, all six `§63.1` safeguards proven: index/tree cleanliness,
no clean/smudge transformation, line-ending/encoding representation, symlink/kind
match, path-sensitive identity, provider context) → L3 owner semantic fingerprint →
L4 derived projection fingerprint (`§64`); gix object/pack caches enabled only for
decode-bearing operations, sized inside the global memory budget, with
commit-graph/history caches explicitly absent (`§65`); `CommonRepoActor` owning
shared immutable repository resources while every linked worktree keeps its own
workspace_id, watcher, inventory, waves, snapshots, and pointers (`§68`, `§40.1`);
common-dir metadata changes fan out to multiple coordinators while worktree state
stays local; submodules as distinct workspace/lifecycle domains with parent-side
topology only, endpoint-only externals, `COMPOSITE_SNAPSHOT_UNSUPPORTED` for
cross-workspace traversal, no auto-fetch/auto-initialize (`§66`); execution
placement and parallelism policy (`§70`–`§72`: blocking pool over the bounded Git
semaphore, three coordination modes, process-wide thread budget including gix
internal parallelism); resource governance treating corrupt repositories as
untrusted input (`§79`); `§143` handle-safety and cache-isolation tests (common
caches never leak worktree HEAD/index state); `§144` workloads executed and
measured (10k-file branch switch, 100k-file warm reconcile, multiple linked
worktrees, large ignored tree, large rename set — cold/warm cache paths separated;
measurements reported, SLO-gated only at W19); **the Wave-7 exit oracle**:
Appendix D steps 11–12 — the full corpus re-run with gix acceleration disabled
produces the identical semantic CPG (`§157` inv. 32 in its Git-aware form, via the
WP48 comparator); `wave7-integration-check` recipe; M08.

#### Dependencies

WP49–WP52.

#### Target Invariants

I-01 (per-worktree workspace identity), I-02, I-32; `LIFE §157` inv. 15, 22–23;
`LIFE §158` inv. 6, 13–15; `LIFE §80`. Doctrine P1, P8, P22 advance.

#### Design and Library References

`LIFE §§63–68`, `§70`–`§72`, `§79`, `§40.1`, `§143`–`§145`, Appendix D; D-31
(audit interplay), `§20` `GitAccelerationStatus` persistence.

#### Change Surface

##### Preflight Query

```bash
rg -n 'CommonRepoActor|common_repository' src/ --type rust      # Wave-2 seam
rg -n 'blob_oid|transform' src/git_state.rs                     # expect zero before
rg -n 'memory_limit' src/fabric/serving.rs src/daemon.rs        # global budget seam
```

##### Known Touch (verified this session)

`src/git_state.rs` cache layer, `src/coordinator.rs` common-repo actor +
multi-coordinator fan-out, `src/daemon.rs` budget wiring, `justfile`
(+`wave7-integration-check`), workload harness (perf group, measurement-only).

#### Required Changes

L2 cache with safeguard evaluation; gix cache configuration; common-repo actor and
linked-worktree registry; submodule topology records + query-boundary error;
placement/semaphore/budget integration; degradation paths
(`GIT_DEGRADED`/`GIT_UNAVAILABLE` with generic fallback); gix-disabled equivalence
run; recipe.

#### Legacy Disposition and Decommission

None.

#### Acceptance Checks

##### Behavioral

Executable oracle: `wp53_behavioral_acceptance`.

`§145.5` linked-worktree/submodule fixtures (two
worktrees, independent pointers, shared ODB; submodule add/update/deinit;
`.gitmodules` edits) and `§145.6` failure/security fixtures (corrupt object,
missing object, cache limit, interruptions, forbidden filter, untrusted config,
lock contention) all converge or degrade exactly as specified.

##### Structural

Executable oracle: `wp53_structural_acceptance`.

L2 cache serves only when all six safeguards pass
(safeguard-matrix fixtures — each single failure forces a current-byte read);
`§143` handle-safety suite green (no attached value crosses waves; no worktree
state in common caches).

##### Negative / Zero-State

Executable oracle: `wp53_negative_zero_state`.

**Wave-7 exit oracle**: the full corpus with gix
disabled compares `EQUIVALENT` to the accelerated runs via
`just rebuild-equivalence-check`; a parent query never returns child-workspace
body facts (`COMPOSITE_SNAPSHOT_UNSUPPORTED` fixture); blob OID never substitutes
for the source digest in any canonical row (schema-level assertion).

##### Operational

Executable oracle: `wp53_operational_acceptance`.

`just wave7-integration-check` — exit 0; `§148` gix metric family incl.
`candidate_pruning_ratio` and `queries_with_git_acceleration_degraded`; `§144`
workload measurements recorded.

#### Edit-Local Gates

Targeted nextest.

#### Packet-Local Gates

`just ci-fast`; `just git-parity-check`; `just rebuild-equivalence-check`;
`just wave7-integration-check`.

#### Integration Milestone

M08 (ceremony packet).

#### Replan Triggers

The gix-disabled equivalence run finds any divergence → stop; the divergence is a
correctness defect in an earlier packet (never waived, never ignored-field-listed);
memory-budget integration shows gix caches cannot fit the global budget → cache
disablement is always legal (acceleration-only).

#### Rollback or Recovery

Every acceleration layer can be disabled independently; correctness rests on
Wave 6.

## 11. Integration milestones

### M05 — Wave 4 exit: source and syntax facts canonical and queryable internally

Packets: WP27–WP33. Evidence: `just wave4-integration-check` (capture → jobs →
adapters → encoders → publish → pinned `ServingQuerySession` SQL over the four new
tables, across valid/invalid/malformed/oversized/binary/excluded inputs);
determinism proof (repeated extraction → identical IDs and effective rows); every
raw provider kind representable; unsupported files yield explicit capability/
diagnostic outcomes (`RM §9` exit evidence, complete); `just ci-fast`,
`just governance`, `just features-each` green. Gate: no Wave-5 packet starts before
M05 (WP34 fixture authoring excepted as declared prework, `RM §4.2`-style, behind
the corpus's own review discipline).

### M06 — Wave 5 exit: Readiness Gate B

Packets: WP34–WP40. Evidence: `just gate-b-check` — the eleven `SUITE` Part V
artifacts end-to-end over the released golden corpus; `just wave5-integration-check`;
all IDs, rows, response bytes, and checksums match released golden outputs
(`RM §10` exit evidence); D-30 state-digest equivalence on every Wave-5 scenario;
`just ci-fast` + `just extractor-ci-fast` green. The gate is the suite's, not this
plan's: any missing gate contract stops and files upstream.

### M07 — Wave 6 exit: CORE_SOURCE_V1 and core equivalence

Packets: WP41–WP48. Evidence: `just wave6-integration-check`;
`just rebuild-equivalence-check` — every core edit scenario (isolated/repeated/
atomic save, add/delete/rename/move, parse break/fix, formatter burst, generated
burst, watcher overflow, restart) converges and compares `EQUIVALENT`;
strict-current queries never expose invalidated stale facts (WP43/WP44/WP45
negative oracles); `CORE_SOURCE_V1` advertised `COMPLETE` with exact per-owner
coverage; the formal all-capability Gate C remains open (`RM §11`). `just ci-fast`
green with the watcher/scheduler/freshness surface in.

### M08 — Wave 7 exit: Git-aware convergence with fallback preserved

Packets: WP49–WP53. Evidence: `just wave7-integration-check`; branch switches,
large tracked-tree transitions, index conflicts, ignore/attribute changes,
linked-worktree additions, and submodule topology changes converge to clean rebuild
(`§145` fixture families through the WP48 comparator); ordinary isolated saves
trigger zero full status scans (WP51 counter oracle); every Git candidate set
fenced by its `GitStateVector` and current bytes; gix disabled → identical semantic
state (WP53 exit oracle); `just git-parity-check` green (`RM §12` exit evidence,
complete).

---

## 12. Cross-packet decommission batches

### DB07 — Synthetic canonical-ingest zero state

Prerequisite: WP36. Exit invariant: `SyntheticCanonicalIngest` has zero production
references — `rg -n 'SyntheticCanonicalIngest' src/` matches only `#[cfg(test)]`
scopes, `ast-grep` finds zero construction sites outside tests, and the build is
green (tier-1). The synthetic observation *fixtures* remain as production-engine
test inputs. Standing oracle: `wp36_negative_zero_state` at every later milestone.

### DB08 — Wave-5 freshness-shim zero state

Prerequisite: WP45. Exit invariant: the D-41 bounded freshness plumbing is absent;
every query admission path flows through the AC-G-24 barrier. Standing oracle:
`wp45_negative_zero_state`.

---

## 13. Design assumptions, accepted deferrals, and replan ownership

- **A-01** — The generated contract machinery (Contract IR, registries, bundles) can
  express every Wave-4 extension (new tables, kinds, raw-kind registries). Owner:
  WP28. Validation: `just contracts-verify` + `contracts-repro-check`. If false:
  plan revision (Wave-1 machinery extension packet).
- **A-02** — The pinned nightly's `rustc_public` exposes the D-27 minimal families
  without private-API reach-through beyond the accepted extractor boundary. Owner:
  WP35. Validation: golden extraction. If false: design reopening.
- **A-03** — The phrase registry's 45 records carry executable `planspec_mapping`
  data for the three Wave-5 forms (`QRY AC-G-44` requires it of every released
  phrase). Owner: WP38 preflight. If false: registry correction upstream first.
- **A-04** — `notify-debouncer-full` 0.7.0 behaves per its reference on macOS and
  Linux for the `LIFE §139` scenario corpus (final-state assertions absorb platform
  event-order differences). Owner: WP41. If false: PollWatcher-first adaptation.
- **A-05** — gix 0.86's resolved feature graph supplies Appendix E's capabilities
  under `default-features = false`. Owner: WP49 (LD-25). If false: LD revision +
  spec issue.
- **A-06** — Spec-issue filings (§6.2) are dispositions, not blockers: each SI has
  a plan decision that stands unless the owning spec answers differently, which is
  then a recorded plan deviation or revision per §17.2.
- Accepted deferrals: Ubuntu evidence (user-deferred, I-27); Gate C (W14); sidecar
  activation (W9); notebooks (D-37); §108.2 journal (D-40); petgraph (D-38);
  performance SLOs (W19; measurements are collected, never gate).

---

## 14. Final gate matrix

Recipe names only. New recipes are proposed by the packet that first needs them:
`wave4-integration-check` (WP33), `wave5-integration-check` + `gate-b-check` (WP40),
`rebuild-equivalence-check` + `wave6-integration-check` (WP48), `git-parity-check`
(WP51), `wave7-integration-check` (WP53). Ubuntu clean-checkout remains outside the
matrix under the standing user deferral.

Routine and cross-domain gates:

- just artifacts-check
- just plan-status
- just tracked-target-zero-state-check
- just ci-fast
- just ci-pr
- just root-check
- just root-clippy
- just root-test
- just extractor-ci-fast
- just sidecar-ci-fast
- just adapter-ci-fast
- just stable-graph-check
- just features-each
- just features-no-default
- just deps-fast
- just policy
- just governance
- just proof-coverage-check
- just typos

Contract and schema gates:

- just contracts-tooling-lint
- just contracts-verify
- just contracts-verify-released
- just contracts-repro-check
- just schema-check
- just fixture-check
- just compilation-units-check
- just proto-check
- just proto-repro-check
- just adapter-contracts-check
- just adapter-contracts-governance
- just adapter-contracts-repro-check

Wave integration gates (standing once added):

- just wave2-integration-check
- just wave3-integration-check
- just wave4-integration-check
- just wave5-integration-check
- just gate-b-check
- just wave6-integration-check
- just rebuild-equivalence-check
- just wave7-integration-check
- just git-parity-check

Risk-triggered evidence, bound to named packet obligations (never blanket gates):
`just mutants-file` on the WP29 supersession, WP30 normalization, WP32 §80 ladder,
WP36 correspondence, WP38 ordering/binding, WP42 classification, WP43 propagation,
WP45 policy, WP46 trigger/rebase, WP48 canonical-encoding, WP51 retry/broaden, and
WP52 tuple logic; `just fuzz` remains scoped to the existing JCS target unless a
packet states a new parser/protocol risk with a named target. A performance recipe
becomes a completion gate only at W19; Waves 4–7 record measurements without
storing claims in execution state (v5 D-21).

---

## 15. Execution sequence

Declared dependency edges govern; `||` marks permitted parallelism with disjoint
write sets (parallel packets follow `subagent-orchestration.md` §3: isolated
worktrees, lead-merged integration).

~~~text
Wave 4
  M04 -> WP27 -> WP28 -> {WP29 || WP30} -> WP31 -> WP32 -> WP33 -> M05
         (WP30 needs WP28+WP29-accepted contracts only; WP31 needs WP30)

Wave 5
  M05 -> {WP34 || WP35} -> WP36 -> {WP37 || WP38} -> WP39 -> WP40 -> M06
         (WP34 fixture authoring may begin during Wave 4 as declared prework;
          WP35 depends on WP29+WP34-fixture-crate; WP38 depends on WP37 for the
          projection query but may develop against Wave-4 tables in parallel)

Wave 6
  M06 -> WP41 -> WP42 -> WP43 -> WP44 -> WP45 -> WP46 -> WP47 -> WP48 -> M07
         (strictly sequential except WP46 may overlap WP45 on disjoint modules;
          WP47 requires all of WP41–WP46; WP48 is the ceremony)

Wave 7
  M07 -> WP49 -> WP50 -> WP51 -> WP52 -> WP53 -> M08
         (LIFE phase order is normative; no intra-wave parallelism)

Decommission
  DB07 exits at WP36; DB08 exits at WP45; both re-checked at M07 and M08.
~~~

Additional direct edges: WP32 → {WP28, WP30, WP31}; WP33 → {WP29, WP32};
WP36 → {WP32, WP35}; WP39 → WP38; WP40 → {WP34–WP39}; WP43 → WP42; WP44 → {WP42,
WP43, WP30}; WP45 → {WP42, WP43, WP44}; WP46 → {WP42, WP44}; WP50 → {WP49, WP41};
WP52 → WP51; WP53 → {WP49–WP52}.

After M07, `RM §4.1` permits Wave 7 to run in parallel with the Wave-8/Wave-10
language lanes; those lanes are outside this plan, and nothing here may consume
their prework.

---

## 16. Completion checklist

- [ ] WP27–WP33 — Wave 4 packets complete with proving commits
- [ ] M05 — `wave4-integration-check` standing green
- [ ] WP34–WP39 — golden corpus, extractor slice, reconciliation, derivation,
      query, service packets complete
- [ ] DB07 — synthetic-ingest zero state green
- [ ] WP40 / M06 — **Readiness Gate B** green (`gate-b-check`)
- [ ] WP41–WP47 — continuous-update packets complete
- [ ] DB08 — freshness-shim zero state green
- [ ] WP48 / M07 — `CORE_SOURCE_V1` COMPLETE; `rebuild-equivalence-check` green;
      Gate C recorded as intentionally open
- [ ] WP49–WP52 — Git phases 1–4 complete
- [ ] WP53 / M08 — gix-disabled equivalence green; `wave7-integration-check` and
      `git-parity-check` standing
- [ ] §6.2 specification issues filed with owning documents; dispositions recorded
- [ ] Ubuntu evidence remains recorded as user-deferred; no packet blocked on it

---

## 17. Plan risks and replan policy

### 17.1 Risks

- **R-01 Spec drift in place.** The 1.3 specs were revised during Wave 0–3 and may
  move again. Mitigation: declared-input digests; `just plan-status` freshness
  derivation; section citations carry titles; any digest mismatch triggers a
  staleness probe before the affected packet, per `evidence-policy.md §4`.
- **R-02 0.0.x parser churn.** Ruff component crates are pre-1.0. Mitigation: exact
  pins (LD-23); upgrades only as provider upgrades with fixture gates.
- **R-03 Grammar/registry mismatch.** Tree-sitter raw-kind inventories may exceed
  registry assumptions. Mitigation: raw kinds are provider-registry data (D-39),
  append-only; unknown kinds recorded, never dropped (I-29).
- **R-04 Gate B breadth.** The gate needs artifacts from every Wave-5 packet; a
  late gap cascades. Mitigation: WP40 is a ceremony over landed packets; the gate
  text was planned against directly (§8 preamble); missing contracts stop and file
  upstream rather than being invented.
- **R-05 Equivalence-corpus flakiness.** Watcher scenarios are platform-sensitive.
  Mitigation: `LIFE §139` final-state assertions; `§78.9` deterministic barriers;
  retries remain diagnostic (nextest `ci` profile fails flaky tests).
- **R-06 gix behavioral gaps.** Status/dirwalk semantics may diverge from the CLI
  oracle or the reference. Mitigation: parity fixtures (WP51); generic fallback is
  always sufficient (`LIFE §80`); divergences degrade acceleration and enter the
  `§147` upgrade gate.
- **R-07 Throughput regressions from validation depth.** Eleven-check batches,
  comparator runs, and rebase digest validation add cost. Mitigation: measurements
  collected per packet; performance adaptation is in-scope, protocol weakening is
  not; SLO gating deferred to W19.
- **R-08 Integrated-plan scope creep.** Four waves in one plan invite cross-wave
  shortcuts. Mitigation: D-23 wave segmentation; milestone ceremonies; the
  completion rule that no later-wave prework reports as earlier-packet completion.

### 17.2 Replan classification

- **Implementation adaptation** (state-recorded, no plan change): module layout
  within the scope boundary, alternative pinned-library API usage, performance
  tuning, fixture additions, registry appends within declared vocabularies.
- **Plan revision** (new plan version): packet boundary/sequence changes, new
  packets, gate-matrix changes, any SI answered upstream in a way that contradicts
  a plan disposition, Contract IR/protocol shape changes.
- **Design reopening** (returns to the owning 1.3 document): target invariant
  conflicts, provider-boundary violations required to proceed, `LIFE §69`-class
  decisions failing their benchmarks, any Gate B/`CORE_SOURCE_V1` criterion that
  cannot be met as specified.

### 17.3 Standing packet triggers

Every packet: (1) a planned generated contract or registry lacks a needed record →
fix upstream through Wave-1 machinery, never hand-author; (2) the current consumer
surface is materially larger than the preflight predicted → re-scope before
editing; (3) a packet cannot remain dependency-closed → split via plan revision;
(4) evidence contradicts a session-verified "known touch" → re-verify, current tree
wins; (5) any invariant I-01…I-34 would be violated by the planned mechanism →
stop, classify per §17.2.

### 17.4 Rollback doctrine

Additive packets revert by commit; contract regenerations revert through their
derivation units; overlay/publication protocols are abort-and-restart by
construction; decommission exits (DB07, DB08) are never rolled back silently — a
regression there is a defect, and the standing negative oracles catch it at every
later milestone.
