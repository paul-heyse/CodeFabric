# Audit report — CodeFabric Waves 0–3 Foundation Implementation Plan v1 (2026-08-20)

**Target:** `docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v1_2026-08-20.md`
**Method:** Six parallel verification agents — four spec-citation verifiers (governance manifest + ontology; data fabric; lifecycle; serving/fact-gen/query/roadmap), one library-reference verifier (LD-01…LD-14 and repo dependency state), and one independent design challenger (D-01…D-09, WP08/WP25 decomposition, Wave-3 sequencing). Every claim verified against the actual spec/reference text with file:line evidence; nothing taken from the plan on faith.

---

## Overall verdict

**The plan is execution-worthy after one targeted revision cycle.** Its factual grounding is unusually strong — the large majority of its several hundred citations verified exactly, and **every one of its ~20 self-flagged assumptions (A-01…A-42 register) was confirmed as a real spec defect or genuine uncertainty**, several of them worse than the plan states. The audit found:

- **3 executability blockers** (must fix before execution begins)
- **~10 newly discovered spec defects** the plan must file upstream rather than resolve ad hoc (roadmap §28 rule)
- **~25 citation mis-attributions** to correct (mostly wrong section/document; substance usually right)
- **4 library-grounding risks**, two of which would cause hard failures mid-wave

---

## 1. Executability blockers

### B-1 — WP23 ↔ WP24 circular dependency (Wave 3 sequencing)
WP24 depends on WP23 (AC-G-19's manifest `overlay:` block needs WP23's overlay representation). But WP23's rebase protocol ends with "activate S_new", and its acceptance requires activation to swap one consolidated overlay — and activation (the AC-G-26 SQLite `BEGIN IMMEDIATE` transaction + `ArcSwap`) is owned entirely by WP24. §15's dependency note names only the WP22 pointer-CAS edge and misses this cycle.

**Fix:** reorder to `WP19 → WP20 → WP21 → WP22 → WP24 → WP23 → WP25`. WP24 can construct and activate a snapshot over a base publication with an **empty overlay** (`overlay_generation: 0` — a valid AC-G-19 manifest), which breaks the cycle; WP23 then populates the overlay block and consumes WP24's activation transaction. WP24's lease/retention half (depends only on WP13 + WP22) can run parallel to WP21–WP23. WP25's `cpg_control` operational projections (depend only on WP13 + WP19) can also be pulled earlier.

### B-2 — Two mandatory AC-G-25 state machines are never authored
AC-G-25 (LIFE:5677–5689) mandates **eleven** machines including `DurablePublicationState` and `ServingActivationState`, each requiring `from/event/guard/to/actions/idempotency_key/error_on_illegal` transition rows plus model-checked reachability. WP08 authors YAML for five machines only. Yet WP22's acceptance asserts "states follow `DurablePublicationState` exactly", and `ServingActivationState` appears **zero times in the plan** while §13.8's manifest carries `serving_activation_state_code`. §62.8 supplies enum values only, not transitions. Gate A either fails at M02 or silently under-delivers.

**Fix:** add both transition tables to WP08's mandate; they gate WP22/WP24.

### B-3 — WP08 is an unexecutable omnibus; its own contingency should be the plan
WP08's stall contingency (R-07: split by registry family as a plan revision) is pre-designed and declared sequence-neutral — deferring it to a mid-wave replan buys nothing. The 53-section phrase harvest + EBNF has **zero consumers in Waves 0–3** (first consumer is the Wave-5 query compiler) yet sits ahead of WP09/WP10/WP11 on the critical path. Roadmap §28 item 4 permits 4–8 packets per wave; the split fits.

Compounding defect: the harvest range is wrong. The Query phrase catalog is **§50–§94** (QUERY:3168–4898); **§95–§102 are Part VII worked examples** (QUERY:4924–5366) that define no phrases. WP08's counting verifier ("enumerate Query §50–§102 … fails on any gap") would demand phrase coverage for eight sections that have none.

**Fix:** pre-split into WP08a (ontology/enum/flag/error/capability/provider/derivation registries + all AC-G-25 machines — everything Waves 2–3 consume) and WP08b (phrase registry + `english-controlled-v1.ebnf` + `model-pack.schema.json`), WP08b parallel to WP09/WP10, closing before M02. Correct the verifier range to §50–§94.

---

## 2. Newly discovered spec defects — file upstream, do not resolve in-plan

Per roadmap §28 (ROADMAP:1494), each of these returns to the owning 1.3 spec as a design issue:

| # | Owner | Defect |
|---|---|---|
| S-1 | Data Fabric | **§101 vacuum retention contradicts AC-G-23.** §101 (:3072–3077) omits the active snapshot and non-expired leases from its protected set; AC-G-23 (:3780–3784) includes both. Implementing §101 literally vacuums data files still pinned by a live `ServingSnapshot` or query lease. AC-G-23 must win. |
| S-2 | Data Fabric | **§91 references a `TableSpec` overlay-policy field that §11 does not define**, and names a fourth category (`query-time derived`) absent from AC-G-21's five-value enum. Three overlapping mutation taxonomies exist: §11 `OwnerReplacementPolicy` (variants undefined), §68's six prose mutation classes (no enum name), AC-G-21's five-value `OverlayMutationPolicy`. |
| S-3 | Data Fabric | **§13.1 `workspace` table has no `registration_revision` or `updated_at` column**, but AC-G-10 relink/configure mutate `root_path_bytes` and mint new revisions, and AC-G-19 pins `registration_revision`. After a relink the Delta row is silently stale with no column able to express which revision it reflects, and D-08 defines no update trigger. (Add the columns via WP09 TableSpec + file the issue.) |
| S-4 | Data Fabric | **§2.1's `deltalake` dependency entry (:312–317) is a multi-line inline table — invalid TOML.** Copying it verbatim will not parse. |
| S-5 | Lifecycle | **§154's readiness barrier (:5253) requires Git acceleration `CURRENT`, but `GitAccelerationStatus` (:1523–1532) has no `CURRENT` member** (`GIT_READY`, `GIT_DEGRADED`, …). |
| S-6 | Ontology | **§64.7 says the 256-bit digest SHOULD be retained; AC-G-13 (:3747) says SHALL.** |
| S-7 | Ontology | **§66 vs AC-G-73 divergence is worse than the plan's A-06**: §66 says SHOULD (AC-G-73 says mandatory), lists 7 kinds vs 12, and `UNKNOWN_MEMORY` is the majority spelling (10 occurrences across 2 docs) vs `UNKNOWN_MEMORY_LOCATION` (1 occurrence, AC-G-73 only). |
| S-8 | Ontology | **Three of the nine §62 "code tables" (62.7, 62.8, 62.9) carry no numeric codes** — bare enum blocks. Code assignment must be authored, not harvested. |
| S-9 | Serving | **§9's seven-RPC form conflicts with AC-G-58's nine-RPC service** (5-variant vs 7-variant `QueryEvent` oneof). AC-G-58 wins per §0.2 precedence but §9 is never annotated as superseded. |
| S-10 | Library refs | **The delta-rs reference is pinned to rev `9f922319`, not the suite's `35cfed45`** — and pins DataFusion **54.0.0** vs the fabric's =54.1.0. Also: `docs/library_ref/deltalake_rust.md` (the path the skill routes) does not exist (only the dated filename), and the reference cites `datafusion_54vs53.md` four times — a file that does not exist. A companion doc `docs/codefabric_delta_rs_9f922319_design_change_recommendations_2026-08-20.md` exists and the plan has not absorbed it. |

---

## 3. Library-grounding risks

| # | Severity | Finding |
|---|---|---|
| L-1 | **High** | **`llvm-tools` vs `llvm-tools-preview`.** The MIR reference (the authority the extractor domain is built on) consistently writes `components = ["rustc-dev", "llvm-tools", "rust-src"]` — zero hits for `llvm-tools-preview`. The plan writes `llvm-tools-preview` (grounded in a *different* reference, the rust-dev-tooling doc). The names are not interchangeable in `rust-toolchain.toml`; the wrong one fails `rustup component add`. Reconcile before WP02. |
| L-2 | **High** | **"A version-pinned handle's commit conflicts with any concurrent commit" is unsupported.** The delta-rs reference (§9.22) describes OCC that fails only on *conflicting* changes since the read snapshot — strictly weaker. No conflict-classification matrix is documented. If the overlay/publication design is load-bearing on unconditional conflict, it needs its own probe. ("No CAS primitive" and the OCC model itself: confirmed.) |
| L-3 | Medium | **`DeltaOps` is deprecated at the pinned rev** (prefer `DeltaTable::write/create/delete/…`), and the legacy `DeltaTableProvider` type was **removed** (PR #4435). Do not copy older examples. `WriteBuilder`/`CreateBuilder` await to `DeltaTable`, not `(DeltaTable, metrics)` — the plan's flag on this is confirmed twice in the reference. |
| L-4 | Medium | **Python pins are weaker than LD-11 states.** `pydantic-settings` is transitive-only (never declared in `pyproject.toml`); declarations are `>=`, not `==` (lock currently resolves to the plan's exact versions but `uv lock --upgrade` would float); `grpcio`/`protobuf`/`orjson` are absent from `uv.lock` today (consistent with WP04 being future, but LD-11 reads present-tense). |

Confirmed probe-needs (plan already flags these — both justified): Arrow two-arg byte-capacity builder constructors are **not documented** anywhere in the Arrow reference (only single-arg `with_capacity`); programmatic `ViewTable` construction is **not documented** (SQL `CREATE VIEW` only — zero `ViewTable::` hits). **Recommendation from the design challenge: move the ViewTable/anti-join probe from WP25 (last in the wave) into WP19's preflight** — a packet-sized replan trigger should not first execute at the end of a seven-packet linear wave.

Minor: gix "HEAD requires `revision` feature" is not actually stated by the gix reference (§2's gate covers the whole revision surface; HEAD accessors carry no feature annotation) — practical impact nil since `revision` is in the recommended set, but the grounding is wrong. Carry instead the documented unborn-HEAD failure mode (`head_commit()` fails on a repo with no commits).

---

## 4. Citation corrections (substance usually right, attribution wrong)

**Governance manifest / ontology**
1. AC-G-02 never says "BLAKE3" — only `b3:<64 lowercase hex>`. BLAKE3-256 is bound in AC-G-07:384. Cite both.
2. AC-G-04's CI rule has **four** failure conditions; the plan omits "query phrases with no executable mapping".
3. **AC-G-14 is in the fact-generation spec (:4045), titled "Analysis-context discovery, identity, and selection"** — not a toolchain-bundle/handshake contract. `pyrefly_bundle_digest` (:4070) and `rust_toolchain` commit-hash identity (:4098) are context-manifest fields. The plan's toolchain-bundle framing is wrong on both document and subject.
4. AC-G-03 never names `feature-registry.yaml` (introduced in AC-G-05/Part IV); the linkage is a synthesis, not a citation.
5. AC-G-13: "rejection of out-of-order records" does not exist — :3715 states emission order only. Decoder-side rejection would be plan-invented.
6. AC-G-15 contains no interning rules (zero `intern` hits in the ontology); type interning is fact-gen §20.2 (:1074).
7. AC-G-71: "null never means unknown" is the Decision (:4030) and rule 3, not rule 2.
8. `registry/` holds 14 files of which 13 are `*-registry.yaml`; the 14th is `model-pack.schema.json`. macOS config root is state root + `/config`, not the same path. `fact_store: delta-local-filesystem` (not "delta-local").

**Data fabric**
9. Utf8View prohibition is **§65.2 (:2128)**, not §5.1. §5.1 prohibits JSON-blob/EAV/etc.
10. `enum_catalog` is not a §13 control table (it is §8:585, and only MAY); the §13 list omits `serving_snapshot_manifest`/`active_snapshot` (§13.8). A-35's bare `repository` name appears at **three** sites (:2208, :2932, :3951), not two.
11. `OwnerReplacementPolicy` is §11:656, not §68. FULL_TABLE_REPLACE partial replacement is prohibited **unless** a derivation profile formally proves a smaller stable partition (:3706) — not flat.
12. `SOURCE_SNAPSHOT_MISMATCH` and `STALE_RESULT` appear **zero times in the fabric spec** — they live in fact-gen (:4388), ontology (:2994), lifecycle, and serving.
13. §94 is "Query-planning policy" and prefers `Expr`/`LogicalPlan` **over** SQL; the read-only posture is §0.6:129 and §13.12:967. §75 lists **sixteen** integrity checks (plan names six of them); `DurablePublicationState` is §13.5:836 with **seven** values (FAILED/ABANDONED included).
14. "§9.22 OCC" is the **deltalake library reference**, not the fabric spec.

**Serving / fact-gen / query / roadmap**
15. **SERVE §79 Phase 1 registers four public tools** (:3280) — the plan says Phase 1 registers none. This changes adapter-packet scope.
16. `FreshnessPolicy` enum is §9 (:538–545), not AC-G-58. `SEMANTIC_PHRASE_UNRECOGNIZED` is not in AC-G-65's enumerated list (that has `SEMANTIC_PHRASE_AMBIGUOUS`; the code exists at QUERY:5933 and enters via the catch-all).
17. SERVE §8 is SHOULD (recommended transport); the gRPC/UDS **mandate** is AC-G-61 (:3550). §19.5 argues the full schema is *not* a Pydantic graph — the schema-closed claim belongs to §6 invariants 13/15.
18. GEN §2 writes the nightly as `` nightly `2026-08-18` `` — the toolchain string `nightly-2026-08-18` appears only in the plan. GEN names **no** rustup components (governance:1056 names `rustc-dev` alone). notify/notify-debouncer-full versions come from the library reference, not GEN §2.
19. AC-G-30: handshake field is "sidecar build and Pyrefly source digest" (not `pyrefly_bundle_digest`); sidecar stdout is **unused and MAY be closed** (protocol rides the socket) — the inverse of "STDOUT protocol-only". AC-G-32 carries 2 s/10 s cancel deadlines, not the 4-chunk/16 MiB credit constants (those are duplicated in AC-G-30:4169 and AC-G-31:4268 — A-18 confirmed). Extractor "length-delimited framing" is an inference; AC-G-31 says fd-or-UDS + owned DTOs "before framing".
20. **AC-G-33's stable read is nine steps, not seven** — a 7-step reading silently drops line-index construction and snapshot-lease issuance. (LIFE §33's capture algorithm is the 7-step one.) "Default retry 3" is an Appendix-B benchmark starting value, not normative.
21. QUERY §103/§104 are schema-artifact pointer sections; envelope fields are §6 (:517), public-ID patterns are §32 (:2381).
22. ROADMAP Wave 0 WP2 does **not** say "own lockfile"/"own rust-toolchain" for the extractor — only "separate executable/build domain" (WP3/Pyrefly has "independent lockfile"). Gate F/performance posture is §26:1425 + §24 WP5, not §29. `codefabric-contracts verify` is Wave 1 WP2, not exit evidence; "four Protobuf packages" comes from WP6's enumeration.
23. Lifecycle: `BLOCKED_PATH_COLLISION` is owned by the **ontology** (:3678), not AC-G-11. The rename/identity policy is **§35** (:1965), not §45 (§45 is the file-identity hierarchy — both support the plan's point). §130 has **no** workspace-registration, credentials, or generation-counter tables (those exist only as AC-G-27 persisted-domain prose — schema authoring falls to the plan). AC-G-25's roster is eleven machines and does **not** include the AC-G-10 registry machine (plan's five-machine set is a plan-side selection). The §43-overrides-Appendix-F precedence is a plan-side editorial resolution of a real intra-document conflict — the spec never states it. §76 sandbox: "config restricted"/"env off" are policy dimensions without assigned defaults. Bounded-blocking-execution class is §109.6/§158.5, not §78–79.

---

## 5. Design decision verdicts (independent challenge)

| Decision | Verdict | Key finding |
|---|---|---|
| D-01/D-02 three Cargo roots + uv project, no workspace | **Sound** | Roadmap mandate over-claimed for WP2 (verbatim only for WP3); "workspace shares one toolchain resolution" is wrong (only the lockfile argument is load-bearing — and sufficient). **Gap: no multi-root IDE config** — add `rust-analyzer.linkedProjects` + `rust-analyzer.rustc.source = "discover"` to WP01 or the two riskiest domains get no editor diagnostics for the entire plan. |
| D-04 commit generated artifacts + byte-identity CI | **Sound** (spec-forced) | Cost centre: same content committed to ≥2 locations (`contracts/generated/` + per-domain `src/generated/`). Recommend one location, `include!`/package-data from it, plus `.gitattributes linguist-generated` and fmt/typos/ast-grep excludes — otherwise `cargo fmt --check` and byte-identity will fight on every regeneration. |
| D-06 persisted state machines | **Questionable** | Headline says one machine; body and WP14 build two (correct per I-13 — fix the record). AC-G-28 is a projection over a **four-input tuple** (lifecycle + source-trust + capability + snapshot-usability columns), never named. Plus blocker B-2. |
| D-07 `SyntheticCanonicalIngest` | **Sound in principle, under-specified** | Transition is bounded (Wave 5 WP3 is the named replacement). But §72/§73.1's real shape is *N streams + precedence → canonical + fact_evidence + conflict records*; a single-stream ingress makes the swap a signature change at every WP21/WP22 call site. Pin the N-stream signature now; add one two-conflicting-observations fixture so the evidence leg is exercised at Wave 3. |
| D-08 SQLite vs Delta authority split | **Questionable** | Split is right and well-cited, but: S-3 (workspace-row staleness); `cpg_control` joins a publication-pinned store to a live SQLite view with no stated capture point relative to lease acquisition; no Wave-3 test asserts SQLite↔Delta agreement, and relink→publish→query is never traversed. Also WP19 depends on Wave-2 WP14 workspace rows but the Delta namespace doesn't exist until WP19 — backfill unowned. |
| D-09 extractor/sidecar deep integration at Wave 0 | **Sound decision, wrong CI tier** | §27.6 mandates Wave-0 pinning. But the plan's "§76's adoption conditions are satisfied" is **three of four** — the semantic golden corpus lands in Wave 5; record the deviation. And every-PR extractor/sidecar jobs are a Tier B/C workload in Tier A (repo-spec §49): move to path/pin-triggered + nightly-scheduled; proof preserved, per-PR cost near zero for Waves 1–3. |
| WP25 | Questionable sizing | Content coherent, but its packet-sized ViewTable probe runs last in a seven-packet linear wave — pull into WP19 preflight (see §3). |

---

## 6. Plan assumption register — all confirmed

A-01 (three names for one hash: `BLAKE3_128` ×13 across 6 docs vs two descriptive phrasings), A-06 (worse — see S-7), A-10, A-11, A-13 (`primary_key_digest`/`effective_content_digest` defined nowhere; per-table effective-content digest implies full-table scans unless incrementally maintained — nothing specifies that), A-15, A-16, A-18, A-21, A-23, A-25, A-27, A-28 (`blake3:` vs `b3:` — three fields in QUERY §36.1), A-30, A-35 (three sites), A-36 (`fact_evidence` is the only table with no `**Primary key:**` line — load-bearing for five downstream contracts), A-37, A-38, A-39, A-42 — **all verified as real**. CBEF authoring gaps confirmed: field-tag numbers unassigned suite-wide (single `field_tag` hit); container length-prefix widths for type codes 9–12 unspecified.

Verified-solid foundations (no action): §2.1 pins; AC-G-05 tree; AC-G-06/07/08; AC-G-09/10/11 (minus the one mis-attribution); AC-G-12/13 core; AC-G-18; AC-G-19/20/21/22/23/26 details; §63–§70, §91–§92, §95, §102–§114 fabric details; AC-G-27/28/62; §39 gix pin (byte-identical in both locations; `revision` feature genuinely absent); AC-G-30/31 protocol substance; AC-G-44/46/53; SERVE §54/§55/§60/§68.6/§70; all Wave 0–3 roadmap work packages and exit evidence near-verbatim.

---

## 7. Recommended revision sequence

1. Apply B-1 (re-sequence Wave 3) and B-3 (pre-split WP08, fix §50–§94 range) — pure plan edits.
2. Apply B-2 (add the two missing machines to WP08a's mandate).
3. File S-1…S-10 as spec issues with the owning documents; where execution can't wait, record the resolution the audit supports (AC-G-23 over §101; AC-G-19 over §13.8; AC-G-58 over §9) as explicit deviations.
4. Resolve L-1 (component name) and L-2 (conflict-semantics probe) before WP02/WP22 respectively; align the delta-rs rev story (S-10) or regenerate the reference at `35cfed45`.
5. Sweep the §4 citation corrections through the plan text (mostly mechanical).
6. Adopt the design-challenge recommendations judged cheap-now/expensive-later: N-stream ingress signature (D-07), workspace-row revision column + relink test (D-08), rust-analyzer multi-root config (D-01), single generated-artifact location + hygiene attrs (D-04), CI tier move (D-09), ViewTable probe into WP19.
