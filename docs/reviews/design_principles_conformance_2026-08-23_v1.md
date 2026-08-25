---
artifact: design-principles-conformance
date: 2026-08-23
version: v1
status: complete
principles_path: docs/library_ref/full_data_fabric_design_principles.md
principles_digest: c20ba5e3f2d499fb439c9aadebf72d2fa98f795368faf7a7a168f420a64b48e1
baseline_commit: d89cc90cd2e51c4b716b2a4da2c0a8d6f79d5409
verdict: conformant-with-findings
---

# Design-principles conformance review — gap register

Assessment of the CodeFabric codebase against
`docs/library_ref/full_data_fabric_design_principles.md` (25 principles, 10 anti-patterns,
17 mandatory design questions).

**This register documents gaps. It is not a target design and proposes no architecture.**
Each finding states what is inconsistent and gives a command that proves it; the remediation
direction is one sentence and deliberately stops short of a design.

## 1. Baseline and scope

Assessed three times. **Pass 3 is the current state, and is the first with a reproducible commit.**

| | Pass 1 | Pass 2 | Pass 3 (current) |
|---|---|---|---|
| Baseline | `eb27a5b` | `eb27a5b` + uncommitted tree | **`d89cc90`** |
| Reproducible? | partly | no — digest record only | **yes — `git checkout d89cc90`** |
| Findings | 74 | 99 | **124** |

The Waves 4–7 work is now committed: `35fc632 feat(core-facts): complete waves 4-7` then
`d89cc90 docs(plan): reconcile waves 4-7 completion`. Net against `eb27a5b`: **155 files,
+20,361 / −1,514**, 52 added. Every detector below can be re-run by checking out `d89cc90`.

**Three checks that could have invalidated the register — all clear.** The yardstick is
byte-identical (`full_data_fabric_design_principles.md` still hashes to `c20ba5e3…`). The pins
held: Arrow `=58.4.0`, DataFusion `=54.1.0`, delta-rs `9f922319`; the Arrow 59 / DataFusion 55
references remain research, cited by nothing outside `docs/library_ref/`. The three
`docs/upfront_design/` edits are the same additive clarifications seen at pass 2.

Subject: ~68k lines handwritten Rust, ~5k handwritten Python, `contracts/**` authorities, `tests/`,
`rules/`, `justfile`. `src/generated/**` is evidence about its authority, not authored code.

**Not claimed.** No gate verdict. `just governance` could not be run to completion in the review
environment — `sccache` is blocked by the sandbox (`Operation not permitted`), which makes cargo
fail outright by design (`AGENTS.md`, sccache is a committed hard prerequisite). That is an
environment limit, not a repository finding, and nothing below rests on it.

## 2. How to read this register

**Class.** `CONFLICT` — built code contradicts a principle the design suite also endorses.
`FORECLOSURE` — no contradiction, but the current shape raises the cost of satisfying the
principle in its owning wave. `UNOWNED` and `DIVERGENT` are handled in §7 and §6.

**Verdict and trail.** Each finding leads with its current verdict and a compact three-pass trail.

| Verdict | Meaning | p3 count |
|---|---|---:|
| `RESOLVED` | genuinely fixed; the change that did it is cited | 1 |
| `PARTIAL` | meaningfully improved; the claim substantially still holds | 11 |
| `TOKEN` | detector no longer matches; substance unchanged | 3 |
| `STANDS` | unchanged | 70 |
| `STANDS (worse)` | unchanged and measurably larger | 14 |
| `NEW` | first raised in pass 3 | 25 |

**A detector flipping is not evidence of a fix** — the rule that has now caught the same error
three times, including one of my own. Pass 3's first reading reported DP-057 fixed because the test
name it greps for appears in *this register*; the test does not exist. That defect is filed as
DP-124 and every whole-repo detector now carries `--glob '!docs/reviews/**'`.

**Reachability.** Re-derived at `d89cc90` and **unchanged**: no code path runs from `daemon::serve`
to any provider, fact, or query. `daemon.rs:18-30` imports only `contracts`, `coordinator`,
`fabric`, `operational_store`, `registries`, `workspace_registry`; `coordinator_task` handles only
`Bootstrap`, `Status`, `Shutdown`; `StaticConfig` declares one socket, the admin IPC.

## 3. What the register found

**124 findings — 117 CONFLICT, 6 FORECLOSURE, 1 observation · 23 blocker, 82 major, 18 minor,
1 observation.** Of the 99 carried forward: **84 stand** (14 of them measurably worse), 11 are
partial, 3 are token, 1 is resolved. 25 are new.

### 3.1 The completion claim

The plan state records `status: complete`, all 27 packets `WP27`–`WP53` complete, and milestones
`M05`–`M08` complete. That is **supported at the level the plan's governance mechanically enforces,
and not at the level its own prose exit conditions require.**

Real work landed, and it is not vaporware: a live `notify-debouncer-full` watcher, a persisted
dirty registry and wave scheduler, a SQLite dependency graph, an incremental Tree-sitter fast lane
with a formal freshness barrier, overlay rebase with exhaustive fault injection, crash recovery, a
real dated-nightly `rustc_public` extractor, a real DataFusion serving-query KAT, a real UDS gRPC
service, and a real gix acceleration stack with fenced caches. **All 108 oracle names declared in
the plan resolve to real functions** — nothing is fabricated.

What is not met is the *proving*, and it is filed as DP-100 – DP-108:

- **`SUITE AC-G-79` / `RM §1` invariant 10** — incremental converges to clean rebuild — is never
  proved. The comparator re-runs the reconciler on the same captured wave (DP-100).
- **Readiness Gate B is not executed.** `gate-b-check` has no body; the eleven `expected/` files are
  contract descriptors, not outputs; none of SUITE's eleven end-to-end items runs (DP-101).
- **The 16 core edit scenarios are asserted to exist as directories.** `scenario.json` is never
  deserialized — verified, zero hits (DP-102).
- **M08's gix-disabled / cache-disabled / full-rebuild comparisons have no implementation**, and
  `git-parity-check` performs no parity comparison (DP-103).
- **Five acceptance oracles are one-line aliases** re-calling an earlier packet's body — including
  `wp50_behavioral_acceptance` calling the *structural* body (DP-104).
- **One commit closes 22 packets, 4 milestones and 3 decommission batches** against a plan whose
  DAG is declared normative and whose contract requires per-packet proof (DP-106).
- **The governance layer counts oracle names, not oracle substance** — the structural reason a
  "complete" state and an unexecuted Gate B coexist with green gates (DP-108).

The honest description of the tree is: *waves 4–7 components implemented and unit-proved; the
wave-5 vertical, wave-6 rebuild equivalence, and wave-7 parity claims are asserted but not
executed.* Daemon reachability is explicitly **not** counted against WP41–WP53 — the plan never
asks for it there, and W17 owns daemon RPC.

### 3.2 The patterns did not change

The register's central claims survived a 20,000-line change set:

- **Single authority (P3)** — all thirteen pass-1 findings stand; `registries.rs` grew 36 → 51
  public enums, the Python registry gap widened to 15 classes, and 27 *new* ad-hoc digest domains
  appeared outside `crate::identity` (DP-120).
- **Error taxonomy (P16)** — `#[error]` variants grew 375 → **439**; still **zero** carry a phase.
  The new modules built a *shadow* vocabulary: none of `LIFECYCLE_*`, `CONTINUOUS_*`,
  `DERIVATION_*` is in `PUBLIC_ERROR_IDS`, while registered codes like `QUERY_HARD_LIMIT_EXCEEDED`
  go unused for the exact conditions they name (DP-117).
- **Capability truthfulness (P20)** — six per-query result states are typed `&'static str`, so they
  *cannot* hold runtime state (DP-110); a live freshness reading is consulted only on the failure
  path and discarded on success (DP-109); an `EffectiveLimitsProfile` digest hashes a constant
  string rather than the limits (DP-111).

### 3.3 Governance moved backwards

Two findings are worse **because of governance edits made in this cycle**, which is the sharpest
signal in this pass:

- `rules/authoritative-source-read-boundary.yml` widened its `ignores` from 5 to 7, adding
  `src/golden_corpus.rs` and `src/query_service.rs`. The rule blast radius shrank while the code
  grew ~11k lines (DP-071).
- `scripts/seed_zero_state_check.sh` gained `-g '!skills/**/REFERENCE.md'`, so a governance script
  now references the untracked duplicate skills tree. Pass 2's claim that `skills/` was inert is no
  longer true (DP-094).

And DP-074 completed its own demonstration: the row registering *this artifact type* landed in the
validator and not in the documented table, so at `d89cc90` the two authorities disagree.

## 4. What genuinely improved

- **`capability_status` is written** — the one `RESOLVED`, now with a second builder in
  `core_facts.rs` (DP-013).
- **Wave selector coverage** — uncovered `wpNN` prefixes fell from ten to four (DP-061).
- **Freshness policy is read for admission** at `query_service.rs:512` — one of roughly eight
  constants became runtime (DP-080).
- **The golden corpus changed shape** from a self-hash to eleven decoded contract checks, three of
  which reach real generated authority (DP-082).
- **The extractor stub is gone**, `CpgQueryService` is implemented, and `lifecycle.rs:844` routes
  wave transitions through `generated_transition` — the correct registry-driven pattern, and the
  counter-example proving it is achievable.
- `derivation.rs` reads its whole contract from `DERIVATION_ENTRIES` and errors on drift — the
  healthiest new module, spoiled only by its hardcoded second copy (DP-097).

The pass-1 conformance record also still holds: one schema IR fanning out with digests verified on
open, crash-recoverable publication, content-addressed snapshots, descriptor-relative path opens,
and seventeen boundary rules confining provider types to their adapters.

## 5. Principle coverage matrix

`enforced` = a named executable oracle proves it. `by-convention` = it holds in code but nothing
would catch a regression — itself a finding under `artifact-schemas.md §6`.
Status is as of **pass 3** (`d89cc90`); rows that moved this cycle are marked.

| # | Principle | Status | Oracle / findings |
|---|---|---|---|
| P1 | Model semantics before behavior | **partial** | enforced for governed codes by `rules/model-no-raw-governed-code-or-flag`; DP-017, DP-045, DP-047 |
| P2 | Models executable, not descriptive | **conflict** (eased) | DP-004 partial, DP-042 partial — the derivation registry is the first compiled to a typed record Rust enforces. DP-038 worse (+2 hand-written encoders); capability and error registries still name lists |
| P3 | One authoritative owner | **conflict** (dominant, worse again) | All 13 pass-1 findings stand. `registries.rs` 36 → **51** enums; Python gap now 15 classes; **27 new** ad-hoc digest domains outside `crate::identity` (DP-120); `FreshnessState` declared twice with the generated copy dead (DP-118). DP-074 completed its own demonstration |
| P4 | Explicit conceptual hierarchies | **conflict** | DP-009 — `ProviderAdapter`'s only impl is a test double |
| P5 | Variability behind contracts | **conflict** | DP-010, DP-033 — provider set fixed in a signature; two role tables into one enum |
| P6 | Semantic meaning vs execution | **conflict** (regressed) | Still no Rust `PlanSpec`: `semantic_query.rs:322` executes a `format!`-built SQL string instead. The plan layer is now *bypassed*, not merely absent — DP-098, DP-053 partial |
| P7 | Shared canonical fabric | **conformant** | one schema IR fans out to Arrow/SQL/JSON/Rust; digests round-trip verified on open |
| P8 | Common representation as infrastructure | **conflict** | DP-011, DP-028, DP-029, DP-030, DP-032; Arrow is hop 7 of 8, wire Arrow has no decoder |
| P9 | Provenance intrinsic | **conflict** | DP-023, DP-024, DP-026, DP-050, DP-053, DP-054 |
| P10 | Provenance closure | **conflict** | 4 of 9 links exist; DP-051, DP-052, DP-055 |
| P11 | Immutable snapshots, explicit transitions | **conformant-and-enforced** | content-addressed manifests; `generated_transition` refuses undeclared edges. Caveat DP-027, and DP-079: legality is checked, guard truth is not |
| P12 | Schemas are executable contracts | **conflict** | DP-025, DP-037, DP-063 — evolution policy is an unstated hard pin; schemas meta-validated only |
| P13 | Governance at the authoritative boundary | **conformant** (in-scope part) | `secure_path.rs` descriptor-relative opens, `authoritative-source-read-boundary`. Tenancy/masking → §6 |
| P14 | Highest-level extension | **unowned** | §7 |
| P15 | Optimizer visibility | **n/a — scheduled** | no custom `ExecutionPlan`, no UDFs; nothing yet claimed |
| P16 | Lifecycle phases first-class | **conflict** (worse again) | 0 of **439** variants carry a phase. New modules built a *shadow* vocabulary outside `PUBLIC_ERROR_IDS` while registered codes go unused (DP-117). DP-016, DP-018, DP-096 |
| P17 | Intermediate artifacts inspectable | **conflict** | DP-015, DP-036 — traces discarded on error; `QueryPlanArtifact` dropped |
| P18 | Fingerprint what has identity | **partial** | strong CBEF/JCS/BLAKE3 base; DP-005, DP-046, DP-051, DP-052 |
| P19 | Reproducibility a normal mode | **conflict** | DP-012 — `result_checksum` is not reproducible. `model-repro-check` covers the model plane only |
| P20 | Conservative capability claims | **conflict** (worst area) | Result states typed `&'static str` so they cannot hold runtime state (DP-110); live freshness discarded on the success path (DP-109); a limits digest that hashes a constant (DP-111); plus DP-076..DP-081. DP-013 resolved; DP-080 improved by one field of ~eight |
| P21 | Enforced vs advisory metadata | **conflict** | DP-008, DP-021, DP-042, DP-068 — the metadata-theater cluster |
| P22 | Protocols and canonical boundaries | **conflict** | DP-048, DP-064, DP-065, DP-066, DP-067, DP-070 — four disjoint boundary families |
| P23 | State ownership local and explicit | **partial** | strong ownership doc-comments and single-writer enforcement; DP-035, DP-049, DP-072 |
| P24 | Semantic observability | **conflict** | DP-014 (false shutdown evidence), DP-036, DP-050, DP-055 |
| P25 | Tests derive from contracts | **conflict** (worst row) | Gate B not executed (DP-101); 16 scenarios never deserialized (DP-102); five acceptance oracles are aliases (DP-104); AC-G-79 unproved (DP-100); governance counts oracle names not substance (DP-108). Zero of **209** `wpNN_` tests names a contract (DP-099). Source-text oracles number 10+, not 6 (DP-062) |

**Strongest row:** P11. **Weakest rows:** P25 and P3.

## 6. Divergence ledger — closed, not gaps

| Principle claim | Why it does not apply | Citation |
|---|---|---|
| P13 tenancy predicates, multi-tenant access policy | The daemon is single-user local; peer identity is a `SO_PEERCRED` UID equality check, not a tenancy model | `SRV §6` inv. 4; `AC-G-61` |
| P13/P21 governance metadata: classification, retention, **masking** | Evaluative governance products the fabric explicitly refuses to emit | `FAB` App. D; `ONT` App. B; `FAB` App. C inv. 16 |
| P21 "advisory metadata: display name, precision hint" | `QRY` owns canonical human-readable statements as a *contract*, not an advisory channel | `AC-G-54` |
| P15/P16 optimizer visibility via transparent `Expr` over UDFs | Agents never see SQL or physical graph syntax; there is no user-authored expression surface to keep transparent | `SRV §6` inv. 5 |

## 7. Spec-feedback list — UNOWNED, route to the owning 1.3 specification

Per `RM §28`, a discovered ambiguity returns to the owning specification rather than being resolved
in implementation. These five principles have no normative home in the suite:

| Principle | What is unowned | Suggested owner |
|---|---|---|
| P2 | That one model must support *multiple* derived operations. `RM §27.1` requires origination in a machine source, not derivation breadth. | `SUITE` (AC-G-05) |
| P10 | Provenance *closure* as a deliberately resolvable recursive chain. Only per-artifact fingerprints are owned. | `FAB` / `LIFE` |
| P14 | The extension-level preference ladder (built-in → UDF → provider → logical → physical). | `FAB` / `QRY` |
| P19 | `Reproducibility { deterministic, inputs_pinned, environment_recorded, … }` as a **modelled status**. The suite requires convergence (`RM §1` inv. 10) but never models reproducibility itself. | `LIFE` (AC-G-79) |
| P21 | The five-way metadata semantic-class taxonomy (enforced / planner-consumed / contractual / governance / advisory). | `FAB` (AC-G-20) |


## 8. Findings

Ordered by discovery, grouped implicitly by plane. Severity, class, principle, and
reachability are on each heading line; the detector follows the evidence.

## DP-001 · CONFLICT · blocker · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Canonical CBEF identity has **live drift** between two hand-maintained implementations.
`contracts/identity/cbef-v1.yaml:38-176` declares 17 domains (last `ROOT_AUTHORIZATION`).
`src/identity.rs:28-46` has 17. `codefabric-cpg-mcp/.../contracts/identity.py:23-41` has **16**
— `ROOT_AUTHORIZATION` absent. Undetected because `identity.py` is `ArtifactRole::Ignored`
(`repository_model.rs:825-829` claims only `.json`, `/registries.py`, `/wire_models.py`).
Detector:
  test "$(sed -n '/pub enum IdentityDomain/,/^}/p' src/identity.rs | grep -cE '^\s+[A-Za-z]+ = [0-9]+,')" \
     = "$(sed -n '/^class IdentityDomain/,/^class /p' codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/identity.py | grep -cE '^\s+[A-Z_]+ = [0-9]+')"
  # currently 17 vs 16 -> non-zero exit

## DP-002 · CONFLICT · blocker · P3 · reachability: unreachable (contract-level)
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`QueryExecutionState` has two authorities that have already diverged.
`contracts/rpc/cpg_query_service.proto:49-58`: ACCEPTED=1, WAITING_FOR_FRESHNESS=2, RUNNING=3,
SUCCEEDED=4, FAILED=5, CANCELLED=6, LOST=7.
`contracts/registry/enum-registry.yaml` QUERY_EXECUTION_STATE: ACCEPTED=10, RUNNING=20,
COMPLETE=30, FAILED=40, CANCELLED=50, DEADLINE_EXCEEDED=60, NOT_EXECUTED_DEPENDENCY=70.
Different value SETS and different numbering under one domain name. The proto driver treats
`.proto` as opaque bytes; the registry driver never reads them. No cross-check exists.

## DP-003 · CONFLICT · blocker · P3 · reachability: production (Python)
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
**Pass 3 —** Gap now 15 classes: `model_registries.py` grew to 49, `registries.py` untouched at 34; still only the ungoverned copy is imported.
The Python registry module that is actually imported is ungoverned and stale.
`model_registries.py` IS one of the 74 governed outputs (`committed-tree.json`) and is imported
by nothing. `registries.py` (95 KB) is NOT a governed output — produced by nothing — and is the
only one imported (`codefabric-cpg-mcp/tests/test_registries.py:7`).
Both declare 34 classes but different 34: the imported copy lacks `OccurrenceFamily`,
`ProviderNodeFlags`, `RangeReconciliationStep`, `RawKindDisposition`; the governed copy lacks
the 4 state-machine domains. Root cause: `repository_model.rs:825-829` matches
`/registries.py`, which matches the orphan and not `model_registries.py`.

## DP-004 · CONFLICT · major · P2/P25 · reachability: production (model compiler)
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
The model's "desired tree" is defined as the current on-disk bytes, so the check that compares
them cannot fail. `src/bin/codefabric_model/desired_tree.rs:796-799`:
    let bytes = current_outputs.get(&path).cloned()
        .ok_or_else(|| DesiredTreeError::MissingCurrentOutput(...))?;
    ... content_digest: digest_bytes(&bytes)
`ModelPlan::check` asserting zero changes is therefore trivially true. Real drift detection
comes from `transaction::check_current`, and `main.rs:184-190` overwrites `plan.changes` from
that preview — but the action graph, action keys, `explain` output and affected-closure that
`just model-plan` reports remain the shadow's.

## DP-005 · CONFLICT (latent) · major · P12/P18 · reachability: production path pending
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The generated CBEF recipe path is structurally incapable of honoring a declared contract field.
`contracts/identity/cbef-v1.yaml` marks 8 fields `normalization: ASCII_LOWER`
(`workspace_kind`, `worktree_kind`, `language_slug`, `owner_kind`, `result_kind`,
`symbolic_name`, `originating_role`, `case_sensitivity_mode`).
`src/generated/model_identity_recipes.rs` carries **zero** normalization — `RecipeValue::Utf8(String)` —
and `src/identity.rs:882-885` maps every such value to `StringNormalization::None`.
Hand-written paths DO apply `AsciiLower` (`identity.rs:633,682,966,981,1713`, `analysis_context.rs:220`).
Generated recipes exist for all 17 domains, including every domain carrying an ASCII_LOWER field.
Latent only because just 2 of 17 recipes are in use today (`entity`, `relation_fact`), neither of
which has an ASCII_LOWER field — and `rules/model-no-positional-cbef-construction.yml` actively
pushes production onto the generated path, which is what triggers it.

## DP-006 · CONFLICT · major · P3/P24 · reachability: production (governance output)
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The suite manifest cannot describe 14 of its own generated siblings.
`contracts/generated/model/governance/suite-manifest.json` reports 60 outputs;
`validation.json` reports `output_count=74`; `committed-tree.json` has 74 keys.
Cause (`aggregate_driver.rs:390`): `model_outputs()` is computed before the 9 governance views,
the 4 `contracts/manifests/*`, and `validation.json` are inserted into the tree.

## DP-007 · CONFLICT · major · P3 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS → p3 STANDS (worse)
**Pass 3 —** The stale Derived copy drifted **further**. `contracts/schema/arrow-delta/table-specs.json` still claims `source_digest: b3:7e1b29fe…` while the live authority header moved `b3:580695b4…` → `b3:33ae78a6…`. The 17 `contracts/generated/registry/*.json` remain unproduced; one file did leave the family (`contracts/schema/operational-store.sql`, deleted).
19 files are classified `ArtifactRole::Derived` but are produced by nothing and are invisible
to drift detection, because `transaction::compile_sync_plan` (`transaction.rs:449-462`) computes
stale deletions only from `committed-tree.json`, never from the model's Derived claims:
`codefabric-cpg-mcp/.../contracts/registries.py`, `contracts/schema/arrow-delta/table-specs.json`,
and all 17 `contracts/generated/registry/*-registry.json`.
Provably stale: the arrow-delta copy carries `"generator_revision":"codefabric-schema-contracts-v1"`
(decommissioned) and `"source_digest":"b3:7e1b29fe…"`, while the live authority digest in
`src/generated/table_specs.rs:1` is `b3:580695b4…`.

## DP-008 · CONFLICT · major · P21/P25 · reachability: production (governance output)
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The traceability layer is metadata theater: it asserts requirement→implementation→oracle closure
while carrying no normative content.
`requirements.jsonl` and `traceability.jsonl` are **byte-identical** (both sha256 `c0ea9145a6bc…`).
All 63 records share one templated `normative_text`:
  "Released artifact <X> must retain its accepted identity and have model-derived implementation
   and executable proof closure."
`requirement_id` = `MODEL-<blake3(artifact_id)[..16]>`; `verified_by` = `["just model-repro-check"]`.
No normative document is parsed — the `AC-G-NN` corpus in `docs/upfront_design/` never enters the model.
Detector: `cmp -s contracts/generated/model/governance/{requirements,traceability}.jsonl`
   and `jq -r .normative_text …/requirements.jsonl | sed 's/artifact [^ ]*/artifact <X>/' | sort -u | wc -l` == 1

## DP-009 · CONFLICT · major · P4/P5 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The trait that would unify providers has exactly one implementor and it is a test double.
`ProviderAdapter` (`src/provider_runtime.rs:320`); sole impl
`impl ProviderAdapter for FakeAdapter` at `:1564`, inside `#[cfg(test)] mod tests` opened at `:1536`.
Neither `TreeSitterAdapter` (`tree_sitter_adapter.rs:311`) nor `RuffAdapter` (`ruff_adapter.rs:443`)
implements it.

## DP-010 · CONFLICT · major · P5 · reachability: test-only
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
Backend leakage in the ingest entry point: the provider set is fixed in the function signature.
`src/source_syntax.rs:267-273`:
    pub(crate) fn project(expected_scope, source: &SourceImage,
        tree: &TreeSitterSnapshot, ruff: Option<&RuffSnapshot>,
        runs: SourceSyntaxProviderRuns) -> ...
plus `SourceSyntaxProviderRuns { tree_sitter, ruff_python }` (`:34`).
Design test from the principles doc — "how many modules change when a new implementation is
added?" — answers: this signature, `validate_inputs`, `validate_provider_structure`, every
caller, and the hard-coded precedence at `:523-524`.

## DP-011 · CONFLICT · major · P8/P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
**Five** unrelated encodings of one concept — cancellation — with no common contract:
`git_state.rs:246` `pub struct GitCancellation(Arc<AtomicBool>)`
`provider_runtime.rs:193` `pub struct ProviderCancellation`
`tree_sitter_adapter.rs:137` `pub trait TreeSitterCancellation`
`ruff_adapter.rs:329` `pub trait RuffCancellation`
`inventory.rs:43` `pub struct InventoryCancellation(AtomicBool)`
Two traits, three structs; two wrap `Arc<AtomicBool>`, one a bare `AtomicBool`.
Normative hook: `SRV §6` invariant 10 requires cancellation to be **end-to-end** — "MCP
cancellation reaches native execution and cleanup". No single type can be threaded end to end.
Detector: `rg -c '^\s*(pub )?(trait|struct|enum) \w*Cancellation' src/`  # currently 5

## DP-012 · CONFLICT · blocker · P19/P20 · reachability: test-only (serving path)
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`result_checksum` is presented as a determinism guarantee but is not deterministic.
`src/fabric/serving.rs:373-408`: rows arrive via `execute_stream(...)` and are fed to a
**sequential** BLAKE3 hasher in stream arrival order (`for row in &rows { hasher.update(...) }`).
No ordering is imposed — `SortExec|lexsort|sort_batch|with_sort_information` occur **0 times**
in the file. `target_partitions` is caller-configurable (`:118,137,154`) and the only in-repo
construction uses 2. DataFusion 54's `execute_stream` merges a >1-partition plan through
`CoalescePartitionsExec`, which does not preserve order.
=> The same SQL over the same frozen immutable snapshot can yield a different `result_checksum`
across runs for any plan with >1 output partition and no top-level `ORDER BY`.
`QueryPlanArtifact` records it beside `output_partition_count` as though it were a function of
the plan. The only guard is `assert!(result_checksum.starts_with("b3:"))` (`:2162`).
Not fixable by configuration: the hasher is order-sensitive, so it needs an imposed total order
or a commutative accumulator.

## DP-013 · CONFLICT · blocker · P20 + repo doctrine · reachability: production
**RESOLVED** · p1 raised → p2 RESOLVED → p3 RESOLVED
**Pass 3 —** **RESOLVED**, carried from pass 2 and hardened: `capability_status` is now referenced by four non-generated files with a second builder at `core_facts.rs:835`. Residual, tracked under DP-080: `reason_code`/`diagnostic_id` (`fact_ingest.rs:80-81`) are still never populated, so *unknown* remains inexpressible, and `query_service.rs:763` still advertises `capability_statuses: Vec::new()`.
A permanently-empty table is declared a required, pinned publication member.
`src/generated/table_specs.rs` table_code 9 `capability_status`: `required_for_publication: true`,
`publication_pin_role: PublicationPinRole::PinnedData`, `dependencies: &[8]`.
The Delta table is physically created at bootstrap (`src/fabric.rs:409-418` iterates `table_specs()`)
and the publication census verifies the required flag (`src/fabric/snapshot_catalog.rs:802`).
But `rg -l 'capability_status' src/ --glob '!src/generated/**'` is **empty**, and `fact_ingest.rs`
defines row structs for exactly 8 tables (codes 100-170) — none for table 9.
=> Every owner reads as "no capability status", indistinguishable from "capability unknown".
This is the precise inversion the repo's own doctrine forbids: `ONT §4.5` (unknown is a
first-class fact), `GEN §97` (capability gaps), `RM §1` invariant 6.

## DP-014 · CONFLICT · major · P24 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Daemon shutdown returns **false evidence**. `src/daemon.rs:880-903` logs all six named steps
(`mark-stopping` … `release-singleton-lease`) in a loop, and only afterwards performs the work:
`coordinators.shutdown_all().await?`, `checkpoint()?`, `drop(listener)`, `remove_file(...)?`,
`drop(operational_store)`. The same array is then returned as `DaemonExit::shutdown_steps`.
A failure in `shutdown_all()` propagates *after* all six phases were already logged and
reported as complete. This is not a decorative phase list — it is observability that asserts
an outcome it did not observe.

## DP-015 · CONFLICT · major · P17 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Stage traces are built only on the success path, so a failure cannot name its phase.
`src/fabric/snapshot_catalog.rs:570-644`: `trace` is a local `Vec` moved into `Self` at `:640`;
every `?` on `:574-631` discards it. A failure in `WrapOverlay` is indistinguishable from one in
`ResolveVersions`. Identical shape at `src/snapshot_runtime.rs:254-399`.

## DP-016 · CONFLICT · major · P16 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
**Pass 3 —** `#[error(` variants now **439**; still zero carry a phase.
**Zero of 375 `#[error]` variants across 36 `thiserror` enums carry a phase or stage field.**
The three that look like they do carry injected *test fault seams*, not real-failure phases:
`FabricError::MutationFault` (`fabric.rs:76`), `FabricError::PublicationFault` (`:82`),
`SnapshotRuntimeError::ActivationFault` (`snapshot_runtime.rs:50`). Every real failure in those
subsystems arrives as `MutationConflict(String)` / `PublicationIntegrity(String)` /
`SnapshotProviderIntegrity(String)`.

## DP-017 · CONFLICT · major · P1/P16 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
**Pass 3 —** Code-prefixed variants hold at **51**; `PUBLIC_ERROR_IDS` still has one consumer, a length assertion. See DP-117 — the new modules built a shadow vocabulary rather than joining the registered one.
Wire-stable public error identity is a substring of a `Display` impl.
44 variants across 8 enums encode identity as `#[error("CODE:{0}")]`. ~28 of those names exist
**nowhere but a format string** (`FABRIC_TABLE_INVARIANT`, `MUTATION_CONFLICT`,
`OVERLAY_REBASE_RESTART_REQUIRED`, `SERVING_*`, `COORDINATOR_*`, `STALE_RESULT`, …).
Nothing checks they are a subset of `PUBLIC_ERROR_IDS`; a message edit silently renames a
public error. Only `SecurePathError::diagnostic()` (`secure_path.rs:74-98`) returns a structured
`{code, name}` — and it re-states strings already in the `#[error]` attributes.

## DP-018 · CONFLICT · major · P3/P16 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
**Ten** independent error-code vocabularies coexist, none derived from another. Two live in one
file with the same method name and opposite casing conventions:
`DiagnosticClass::code()` SCREAMING_SNAKE (`model_control.rs:156-169`) and
`EdgeKind::code()` kebab-case (`model_control.rs:97-110`).
`contracts/registry/error-registry.yaml`'s `grpc_status`, `mcp_mapping`, `severity`,
`retryability`, `scope`, `allowed_public_detail_fields` appear **zero times** in `src/generated/`.
`src/secure_path.rs:24-27` re-types four numeric registry codes (2000/2010/2020/2030) as the only
numeric codes anywhere in Rust, with nothing asserting the correspondence.

## DP-019 · CONFLICT · minor · P20 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Capability claims that are true only tautologically. `src/fabric/serving.rs:275-277` sets
`parquet_pruning`/`repartition_joins`/`repartition_aggregations`, then `:321-323` reads the same
values straight back into `ServingRuntimeEvidence` and `:2109-2111` asserts them. This proves the
setter ran, not that pruning occurred — and with `statistics() -> None` (`overlay.rs:725-727`) the
repartition heuristics run blind.
`TableSpec::dependencies` (`schema_registry.rs:65`, assigned `:488`) is read nowhere in `src/`.
`zorder_columns` is not even written into Delta metadata, so its drift is undetectable in principle.

## DP-020 · CONFLICT · minor · P20 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
CHECK constraints are installed but never verified, and are written before the table is
authenticated. `install_constraints` runs only on the bootstrap path (`src/fabric.rs:416`), and
`validate_open_table` (`:537-587`) never looks for `delta.constraints.*` — a query-serving open
accepts a table whose constraints were silently dropped. Constraint presence is asserted only in
a test (`src/fabric.rs:965-967`). Ordering defect: `install_constraints` (`:416`) runs *before*
`validate_open_table` (`:417`), writing to a table not yet proven to be the contracted one.

## DP-021 · CONFLICT · blocker · P21 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Referential integrity is declared in two places and enforced in neither — the doc's
*metadata theater* verbatim.
`contracts/generated/model/schema/operational-store.sql:2` sets `PRAGMA foreign_keys = ON;`
and the file then declares **0 `REFERENCES` clauses across 24 `CREATE TABLE` statements**.
The generator has no REFERENCES branch, so the pragma can never do anything.
On the Arrow side, `src/generated/table_specs.rs` carries **14** `foreign_key: Some(...)`
annotations; `grep -c foreign_key src/fact_ingest.rs` is **0** — the full validation matrix
`validate_fact_batch` never reads them. They survive only as decorative Arrow field metadata
`com.codefabric.cpg.foreign_key` (`src/schema_registry.rs:299-302`).
Detector: `test "$(grep -c REFERENCES contracts/generated/model/schema/operational-store.sql)" -gt 0`

## DP-022 · CONFLICT · blocker · P20/P9 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
`provider_run.wave_id` is a `NOT NULL` foreign key into a table that has **no writer anywhere**.
`operational-store.sql:196` `CREATE TABLE update_wave`, `:211` `update_wave_item`;
`rg -c 'INSERT INTO update_wave' --glob '*.rs' .` → **0**, including tests.
`provider_run.wave_id` is `NOT NULL` (`:220`), retention deletes these rows, and the serving
plane exposes both as control views. `wave_id` is validated only by `is_empty()`
(`src/provider_runtime.rs:749`), never a 16-byte decode — unlike `provider_run_id`.

## DP-023 · CONFLICT · blocker · P8/P9 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The one provider-run → published-fact join is defeated by a type mismatch introduced in the
same struct literal. `src/provider_runtime.rs:910-919`:
    let owner_id = spec.scopes.iter().find(...).map(|scope| scope.scope_id.as_bytes().to_vec());
`scope_id` is a prost `String`, so `provider_run.owner_id` is variable-length **UTF-8 text**,
while every fact-table `owner_id` is `LogicalType::Id16` and is length-checked
(`src/fact_ingest.rs:1052-1070`). Two lines later the same literal decodes
`provider_run_id: ids.run.to_vec()` correctly as 16 raw bytes.
=> `evidence.owner_id = provider_run.owner_id` can never match.

## DP-024 · CONFLICT · major · P9 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Provider attribution on the projection path is six constants, discarding facts the call sites
already hold. `src/source_syntax.rs:1394-1403`:
    directness_code: 10, certainty_code: 10, resolution_code: 10,
    producer_code: ProviderCode::CodefabricDerivation as i16, derivation_code: None, flags: 0
`ProviderCode::CodefabricDerivation = 50` (`src/generated/registries.rs:1528`), so every
relation is attributed to derivation even when Tree-sitter (10) or Ruff (20) produced it —
and `derive_relations` (`:1248-1352`) knows which. `derivation_code`, the field naming *which
rule* derived the fact, is always `None`.

## DP-025 · CONFLICT · major · P12/P21 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`fact_evidence.fact_form_code` is an unvalidatable column that the two ingest paths disagree on.
`src/generated/table_specs.rs:1807-1811`: `LogicalType::Code16`, `nullable: false`,
**`semantic_type: None`** — bound to no registry domain, so `validate_registered_codes` cannot
check it. The projection path hard-codes the literal `fact_form_code: 10`
(`src/source_syntax.rs:1491`) while the observation path reads it from the provider
(`src/fact_ingest.rs:1851`). Same column, one constant and one data-driven source, no oracle.

## DP-026 · CONFLICT · major · P9 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Derived relations get no evidence rows at all, so table 110 is unexplainable through table 130.
`derive_relations` (`src/source_syntax.rs:1248-1259`) takes `output: &mut Vec<RelationRow>` and
**no** evidence accumulator — unlike `project_ruff_tokens` (`:905-916`) and
`project_ruff_annotations` (`:989-1002`), which both take one. Call site `:600-611` passes none.

## DP-027 · FORECLOSURE · major · P11/P9 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The retention closure is not the provenance closure: a retained publication's explaining facts
are legally deletable. `SnapshotRetentionSet::build` (`src/snapshot_runtime.rs:923-996`) unions
five *publication-ID* sources only. It protects nothing in `provider_run`,
`table_mutation_operation`, `source_inventory`, or `source_blob`. Meanwhile
`cleanup_terminal_before` deletes `provider_run` on a bare timestamp with no publication
reachability check (`src/operational_store.rs:290-293`), and source-blob GC is gated only on
live/orphaned leases (`src/source_image.rs:708-720`), never consulting a retained publication.

## DP-028 · CONFLICT · major · P8/P22 · reachability: production
**TOKEN** · p1 raised → p2 TOKEN → p3 TOKEN
Two parallel channels carry the same provider output in two incompatible representations;
neither works. `src/provider_runtime.rs:185-190` hands the consumer both
`events: mpsc::Receiver<ProviderEvent>` (carrying `ArrowIpcChunk`) and
`observations: mpsc::Receiver<ObservationMessage>` (carrying `ObservedFact`).
`ProviderEvent::ArrowIpcChunk` (`:154-162`) has **no decoder anywhere** —
`rg 'StreamReader|arrow::ipc::reader' src/` returns nothing; the only `arrow::ipc` import in the
crate is a `StreamWriter` in a test (`src/source_syntax.rs:1795`).
`ObservationMessage::Batch` has exactly one sender in the repo — a test sending an empty vector
(`src/fact_ingest.rs:2355`).

## DP-029 · FORECLOSURE · major · P8 · reachability: test-only
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
**Pass 3 —** Asymmetry now 10-vs-4: the projection path emits `{8,9,100..170}` while `encode_selected` still emits four.
The observation stream protocol structurally cannot express half the fact tables.
`CanonicalFact` has three variants — `Entity | Relation | Property` (`src/fact_ingest.rs:1433-1437`)
— so `encode_selected` emits `{100,110,120,130}` (`:1734-1739`) while the projection path emits
`{100,110,120,130,140,150,160,170}` (`src/source_syntax.rs:638-647`).
`source_file`, `source_token`, `source_annotation`, and `syntax_detail` are unrepresentable.

## DP-030 · CONFLICT · major · P2/P8 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The two ingest paths share only the innermost validator and re-implement everything above it.
`ValidatedFactBatch::validate` is common; **not shared**: cross-table referential validation
(`validate_cross_table` exists only on the projection path, `src/source_syntax.rs:1408-1478`),
row budgets (two mechanisms, different limits), provider precedence (ad-hoc range ladder vs an
explicit `BTreeMap<i16,u16>` sort at `src/fact_ingest.rs:1817-1825`), conflict disposition,
evidence encoding, schema-fingerprint fencing (only the observation path checks it, `:1782-1788`),
and table coverage.

## DP-031 · CONFLICT · minor · P3 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The `fact_evidence` identity function is implemented twice against the same domain string.
`src/fact_ingest.rs:1708-1717` and `src/source_syntax.rs:1512-1514` both hash
`b"codefabric-fact-evidence-v1\0" || run || observation || fact` through separate blake3
harnesses. They agree only by a copied literal.
Detector: `test "$(rg -c 'codefabric-fact-evidence-v1' --glob '*.rs' src | wc -l)" -eq 1`

## DP-032 · FORECLOSURE · major · P8 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Provider-computed facts die at the adapter boundary and one is re-derived downstream.
Dropped: `RuffAstFact.evaluation_ordinal` (Ruff's evaluation order, computed by a dedicated pass —
`src/ruff_adapter.rs:957`, read only in adapter tests), `.source_ordinal`, `.line`/`.column`
(forcing downstream re-derivation from `source_file.line_start_offsets`), and
`RawSyntaxFact.depth` (`src/tree_sitter_adapter.rs:78`, read only by a fingerprint hash).
`entity.name` is re-derived by **re-slicing the source bytes** (`src/source_syntax.rs:544-548`)
rather than carried from the provider that already parsed it.
`RuffTokenClass` (11 variants) is collapsed into `TokenKind` (8) at `:1653-1669`.

## DP-033 · CONFLICT · major · P5 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The pairwise explosion is already present and already inconsistent.
`tree_field_role` (`src/source_syntax.rs:1610-1630`) and `ruff_field_role` (`:1632-1652`) are two
independently hand-maintained mappings into the identical `SyntaxFieldRole` registry — the N×M
signature. They already disagree: Ruff `Annotation → Returns` and `Clause → FinallyBody` are
semantic coercions with no Tree-sitter counterpart; Tree-sitter collapses `"left"|"target" → Target`.
Adding Pyrefly or rustc-MIR requires two more such tables.

## DP-034 · CONFLICT · major · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
IR-owned values are hard-coded as string literals in the runtime, so IR and runtime can drift
silently. `src/schema_registry.rs:435` `"1.3".to_owned()` (ontology_version) and `:463`
`"suite-major-1".to_owned()` (compatibility_mode), while the contract IR declares both and
`render_runtime_rust` (`schema_driver.rs:986-1030`) never emits them.

## DP-035 · CONFLICT · major · P23 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The one cache in the fabric plane with no identity and no invalidation.
`capture_control_schema` (`src/fabric/serving.rs:695-757`) snapshots 9 operational tables into
`MemTable`s under `BEGIN DEFERRED`, once, at `from_lease` (`:285-291`), and holds them for the
session. No digest, no generation stamp, no invalidation; divergence from live SQLite is silent.
`QueryPlanArtifact` records `source_table_versions` for Delta tables (`:421-428`) and nothing for
this capture. Compare every other cache in the plane, which carries a version or digest.

## DP-036 · CONFLICT · major · P17/P24 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The richest diagnostic artifact in the system is built and dropped.
`QueryPlanArtifact` (`src/fabric/serving.rs:197-213`) carries logical/optimized/physical plan
text, DataFusion and Arrow versions, snapshot and publication IDs, source table versions, output
schema, row counts, result checksum and 8 execution metrics — built at `:415-448`, returned in
`ServingQueryResult`, never serialized. The operational table that would hold it,
`result_artifact_lease` (`operational-store.sql:299-305`), has **zero INSERT sites** (only a
`DROP TABLE` in a migration test, `src/operational_store.rs:773`).

## DP-037 · CONFLICT · major · P12 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Schema compatibility is declared as a policy and implemented as a hard pin.
`contracts/generated/model/schema/schema-validation.json` carries
`"compatibility_acceptance_generated": false` (hard-coded at `schema_driver.rs:1179`) — there is
no generated schema-compatibility acceptance suite. The driver validates only
`compatible_suite_major == 1 && schema_version == 1 && owner_bucket_count == 256`
(`schema_driver.rs:283-291`). `delta.enableTypeWidening` is pinned `"false"` (`src/fabric.rs:454`)
and any digest change is a hard `SCHEMA_DIGEST_MISMATCH` reject (`:586-590`). `FAB` App. C
invariant 11 requires schema evolution to be *explicit and versioned*; the de-facto policy is
"no evolution", which is a decision that is nowhere stated.

## DP-038 · FORECLOSURE · major · P2 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
Row encoders are hand-written against a generated schema rather than generated from it.
`src/fact_ingest.rs:373-961` holds 8 `encode_*` functions containing ~161 hand-written
`("column_name", accessor)` tuples. They are *checked* against the generated spec by name and
arity at runtime (`:345-366`) — a guard, not derivation. The `schema_driver.rs:1` doc comment
calling it a "row-encoder family driver" overstates its outputs.

## DP-039 · CONFLICT · major · P3/P8 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`NewlineKind` — one fact, six encodings, reconciled by a hand-written crosswalk.
`contracts/registry/enum-registry.yaml:75` · `src/generated/registries.rs:2118` (`Crlf`) ·
`src/generated/model_registries.rs:371` · `.../contracts/registries.py:318` ·
`.../contracts/model_registries.py:316` · and a hand-written
`src/source_image.rs:73` `#[repr(u16)] enum NewlineKind { … CrLf = 30 … }`.
Crosswalk: `src/source_syntax.rs:1556-1562` `NewlineKind::CrLf => RegistryNewlineKind::Crlf as i16`.
The same file needs `use ... NewlineKind as RegistryNewlineKind` (`:19`) to dodge the collision.

## DP-040 · CONFLICT · major · P3 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS (worse) → p3 STANDS (worse)
**Pass 3 —** `registries.rs` now **51** public enums and `model_registries.rs` **47** (36/32 at pass 1); `source_syntax.rs` still imports from both.
Two generated Rust registry modules declare 32 identically-named but nominally distinct enums,
and one file imports from both. `src/generated/registries.rs` (36 enums) and
`src/generated/model_registries.rs` (32) are emitted by the same driver from the same inputs
(`registry_cbef_driver.rs:2118` and `:2268`). `src/source_syntax.rs:16` imports
`model_generated::registries::{OccurrenceFamily, ProviderNodeFlags, RawKindDisposition}` while
`:19-22` imports `crate::registries::{…}`.

## DP-041 · CONFLICT · minor · P2/P8 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Typed generated facts are recovered by re-parsing generated SQL as text.
`src/operational_store.rs:19-20` `include_str!`s the generated DDL, then
`generated_table_names()` (`:968-975`) does `strip_prefix("CREATE TABLE ")` / `strip_suffix(" (")`,
`generated_table_ddl()` (`:977-995`) scans until `line == ") STRICT;"`, and
`generated_column_shapes()` (`:997`) parses columns — while the typed
`src/generated/table_specs.rs` and `model_schema_tables.rs` carry the same facts.
`:681-701` additionally hand-writes a `CREATE TABLE` and an 18-column `INSERT … SELECT` inside a
migration that must stay in sync with the IR by convention.

## DP-042 · CONFLICT · major · P2/P21 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
**Pass 3 —** Registry still holds one record and nothing executes the derivation (DP-075). `aggregate_driver.rs` convention clauses 78 → 77. `CAPABILITY_IDS` and `PUBLIC_ERROR_IDS` remain `&[&str]`.
The registry that should own derivation facts is empty, so those facts live in path-string
matching. `contracts/registry/derivation-registry.yaml` is `records: []`
("allocations intentionally deferred to Wave 5+"), hence
`src/generated/registries.rs:4584` `DERIVATION_IDS: &[&str] = &[];`.
Meanwhile `bundle_membership` (`aggregate_driver.rs:1145-1201`) encodes all 8 bundle memberships
as ~25 `starts_with`/`ends_with`/`contains` clauses, and `capability-registry.yaml` /
`feature-registry.yaml` each match 3 bundles by accident of clause ordering.
Sibling conventions-as-model: `contract_role` (`repository_model.rs:846-864`), `role_for`
(`:804-844`), `driver_family` (`aggregate_driver.rs:1413-1429`), `projection_profile` (`:714-740`).

## DP-043 · CONFLICT · minor · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Cross-family references in the schema IR are unvalidated and ~half do not resolve.
The IR carries 28 distinct `semantic_type: "enum:<name>"` strings; `SchemaContractIr::validate`
(`schema_driver.rs:281-555`) never inspects them, and they pass verbatim into Arrow field
metadata (`schema_registry.rs:293-297`) and generated Rust. Roughly 14 of the 28 resolve to no
registry domain. There is no digest relationship between the schema IR and the enum registry.

## DP-044 · CONFLICT · minor · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The model compiler does not use the repository's own strict canonical-JSON profile.
`src/contracts/jcs.rs` implements `codefabric-jcs-v1` — RFC 8785 plus duplicate-key rejection
(`:162`), `MAX_SAFE_INTEGER` bounds (`:219`), non-finite rejection (`:168`) and a
`failure_class()` taxonomy. The drivers instead call `serde_json_canonicalizer::to_vec` directly
with no such guards, and re-implement `reject_duplicate_json_keys` ad hoc in
`aggregate_driver.rs:954` and `schema_driver.rs:1346`.
`digest_bytes` is independently defined in **9** modules (`desired_tree.rs:1010`,
`aggregate_driver.rs:1532`, `transaction.rs:938`, `incremental.rs:594`, `model_control.rs:658`,
`driver_protocol.rs:484`, `registry_cbef_driver.rs`, `adapter_driver.rs:442`, `proto_driver.rs:697`).

## DP-045 · CONFLICT · minor · P1 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
31 KB of typed contract vocabulary is dead and shadowed by private re-declarations.
Only `DeploymentProfileDocument` (`src/contracts/models.rs:256`) has consumers. `ArtifactHeader`,
`BundleDocument`, `CbefContract`, `RequirementRecord`, `OwnerAcceptance`, `FixtureOracleRecord`,
`RegistryDocument<T>` and others are each shadowed by a private struct inside the driver that
actually decodes them. `main.rs:16-18` imports the module under
`#[allow(dead_code, clippy::enum_variant_names, clippy::struct_field_names)]`.

## DP-046 · CONFLICT · minor · P18 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Two competing action-key schemes produce different digests for the same family.
`ActionKeyMaterial` (`desired_tree.rs:235-247`) drives the shadow plan and excludes external tool
identity and the real driver descriptor; `FamilyActionIdentity` (`incremental.rs:49-59`) drives
the real render cache and includes both. `just model-plan` reports the shadow key; the cache uses
the other.

## DP-047 · CONFLICT · minor · P8/P22 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
A wire type is used as the domain type. `ProviderJobSpec` — a prost-generated proto message — is
the input to `ProviderAdapter::run` (`src/provider_runtime.rs:326`) and
`ProviderExecutor::submit` (`:1311`). `map_wire_event` (`:557-655`) additionally collapses
`ScopeBegin`/`ScopeEnd` lossily into `ProviderEvent::Progress` with a runtime-formatted string:
`phase: format!("scope-begin:{}:{}", scope.scope_kind, scope.scope_id)` (`:594`).

## DP-048 · CONFLICT · major · P22 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Three disjoint public-boundary contract families with no cross-check.
(1) `contracts/rpc/*.proto` → Rust + Python bindings.
(2) `contracts/adapter/adapter-model-ir.json` → Pydantic `wire_models.py` — **no Rust output**;
    `SnapshotSummary` has no proto counterpart.
(3) The daemon's live boundary is a newline-delimited JSON protocol over a 0600 Unix socket
    (`AdminEnvelope`/`AdminResponse`, `src/daemon.rs:250,258`) with **no schema artifact at all**.
Peer-UID policy is implemented twice: inline at `daemon.rs:832-836` and in
`rpc::SameUserInterceptor` (`src/rpc.rs:144`).

## DP-049 · CONFLICT · minor · P23 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Unbounded state growth and a documented ownership relation that does not exist.
`AdmissionController`'s `workspaces`/`contexts` maps are inserted into (`provider_runtime.rs:447,459`)
and never removed. `SourceImageStore`'s doc-comment (`src/source_image.rs:373`) reads
"Coordinator-owned source capture, lease, and garbage-collection service" — no coordinator
constructs it; only tests and `snapshot_runtime` do.

## DP-050 · CONFLICT · major · P9/P24 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Ingest diagnostics are produced and discarded. `IngestDiagnostic` and `ConflictRecord` are
returned in `CanonicalIngestOutput` (`src/fact_ingest.rs:1639-1644`, populated
`src/source_syntax.rs:653-670`) and are **never written** to the `diagnostic` table (code 10),
which has no row struct, no encoder, and no serving projection.

## DP-051 · FORECLOSURE · major · P18/P10 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`file_id` is a *location* identity used as a fact identity. `source_file_identity`
(`src/identity.rs:718-736`) hashes only `workspace_id`, `SOURCE_CONTEXT_ID` and
`path.comparison_key_bytes` — deliberately excluding `source_digest` and `source_generation`.
Table 140's primary key is `(workspace_id, file_id)` with no generation, so the row is
overwritten per generation. A fact row's `file_id` alone therefore does not identify the bytes it
was derived from; you must additionally know the pinned Delta version.
(`source_occurrence_identity` does fold in the digest — `src/source_syntax.rs:335`.)

## DP-052 · FORECLOSURE · major · P10/P18 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The snapshot's link to source bytes is excluded from the snapshot's own identity.
`ServingSnapshotCandidate.source_blob_digests` (`src/snapshot_runtime.rs:79`, populated `:197-203`)
is not a field of `ServingSnapshotManifestBody` (`src/snapshot.rs:121-134`), so it is neither
content-addressed nor persisted. It survives only as `snapshot_lease.source_blob_lease_id`, which
is NULLed on release and on expiry (`src/snapshot_runtime.rs:720,813`).

## DP-053 · CONFLICT · major · P9 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
**Pass 3 —** `semantic_request_id` now has 17 hits; `mcp_call_id` appears only in generated prost. `query_id` is still `f(sql, snapshot_id)`, and `trace_id`/`correlation_id`/`execution_id` are still absent, so two agents remain indistinguishable at the fabric layer.
No request or execution identity exists in the Rust daemon.
`semantic_request_id` and `mcp_call_id` are declared as Python output fields
(`wire_models.py:135,259`); `rg 'semantic_request_id|mcp_call_id' --glob '*.rs' src` → **zero**.
`QueryPlanArtifact.query_id` is `f(sql, snapshot_id)` (`src/fabric/serving.rs:416`) — a content
hash of the SQL text, so two identical queries from two different agents are indistinguishable.
Also zero `trace_id`, `request_id`, `correlation_id`, `execution_id` anywhere in `src/`.

## DP-054 · CONFLICT · major · P9/P11 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
A Delta commit cannot be joined to its scope except by parsing a string.
`table_mutation_operation` (`operational-store.sql:236-252`) has 14 columns and **no**
`workspace_id`, `analysis_context_id`, `source_generation`, or `wave_id`. Its
`workspace_scope: None` (`src/generated/table_specs.rs:3875`) also makes it structurally
ineligible for a control view — `capture_raw_operational_table` hard-errors on the missing scope
(`src/fabric/serving.rs:822-827`). The only scope available is a substring of `application_id`,
constructed at `src/fabric/mutation.rs:237-248`.
Additionally, three of the ten commit-metadata keys — `owner_set_fingerprint`, `input_checksum`,
`expected_output_checksum` — are digests with **no stored preimage** (`mutation.rs:172-179`),
so "what were the inputs" is unanswerable.

## DP-055 · CONFLICT · major · P11/P24 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
Commit metadata is write-only for provenance purposes. The only `history()` read outside a test is
`commit_metadata_matches` (`src/fabric/mutation.rs:448-459`), an all-keys equality probe used for
recovery (`:476,487`). There is no `explain_version(table_code, delta_version)` surface, and the
MCP server publishes zero tools — so nothing can answer "why does this row exist" for an operator.

## DP-056 · CONFLICT · major · P11 · reachability: test-only
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The query artifact under-pins the interpretation context it used.
`src/fabric/serving.rs:420` copies **1 of 7** bundle IDs (`schema_bundle_id`); the manifest holds
seven (`src/snapshot.rs:104-112`). Overlay identity (`overlay_generation`, `overlay_digest`,
`src/snapshot.rs:86-91`) is not copied at all, and `source_table_versions` (`:422-429`) reads only
`base_publication.tables` — so overlay-supplied rows are invisible in the artifact's version pin.

## DP-057 · CONFLICT · blocker · P25 · reachability: production (CI)
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
**Pass 3 —** **Correction to pass 3's own first reading.** A naive existence check reported this fixed; it is not. The test name occurs in exactly two places repo-wide — `.github/workflows/ci.yml:149` and *this register*. Zero `.rs` files define it, zero read `contracts/fixtures/proto/production_wire.json`, and `ci.yml` was last touched in `00ba7ef`, before the baseline. The gate still passes while running zero Rust tests. See DP-124 for the detector defect that produced the false reading.
**A CI gate passes while running zero tests.** `.github/workflows/ci.yml:147-151`, step
"Cross-language Protobuf round trips", runs
`cargo test … --test integration integration::rpc::rust_protobuf_matches_the_shared_wire_fixture`.
That test **does not exist anywhere in the repository**. Verified empirically at `eb27a5b`:
`running 0 tests … 7 filtered out`, `test result: ok`, **exit code 0**.
Only the Python half of the step executes, so the cross-language claim in the step's own name has
never been tested. The shared KAT `contracts/fixtures/proto/production_wire.json` is consumed by
Python only; the Rust counterpart (`tests/integration/rpc.rs:522-563`) does encode→decode→assert_eq
— a tautology that cannot detect wire drift — against hardcoded bytes at `:549` rather than the
shared fixture.

## DP-058 · CONFLICT · blocker · P25 · reachability: production
**TOKEN** · p1 raised → p2 TOKEN → p3 TOKEN
**No `AC-G` contract, in any wave, is named by any executable oracle.**
`rg -o 'AC-G-[0-9]+' tests rules rule-tests justfile` → **0**.
Of the 35 contracts with wave ≤ 4, 24 appear only in free-text doc comments, and 11 have zero
reference anywhere outside `docs/` — including `AC-G-05` (required machine artifacts),
`AC-G-20`/`AC-G-21` (overlay schemas and semantics, which `src/fabric/overlay.rs` implements in
2,000+ lines), `AC-G-28`, `AC-G-43`, and `AC-G-72` (mandatory conformance profiles, whose phrase
appears nowhere outside `docs/`).
This is the repo's own `artifact-schemas.md §6` rule inverted: a matrix row with no oracle is
itself a finding, and here every row lacks one.

## DP-059 · CONFLICT · major · P25 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
A released governance artifact carries the reserved zero digest.
`contracts/manifests/fixture-oracles.json` has `status: released` and
`canonical_digest: "b3:0000000000000000000000000000000000000000000000000000000000000000"`.

## DP-060 · CONFLICT · major · P25 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The entire negative-fixture family is dead, and a committed changelog claims otherwise.
All **5 of 5** `negative-class` fixtures have no consumer in any language
(`negative/{broken-trace-edge,drifted-digest,perturbed-artifact,schema-version-drift}.json`,
`model-packs/invalid-executable-field.json`), plus `model-packs/valid-minimal.json`.
`contracts/fixtures/CHANGELOG.md:41-42` states the broken trace edge was wired so "the released
verifier prove[s] its rejection on every run" — `CF-ARCH-9999` occurs only in the fixture and the
changelog; there is no verifier. `:53-54` similarly claims a committed mutation oracle for
`SCHEMA_VERSION_NOT_ADVANCED`, a string that occurs only in the fixture and the changelog.

## DP-061 · CONFLICT · major · P25 · reachability: production
**PARTIAL** · p1 raised → p2 STANDS → p3 PARTIAL
**Pass 3 —** Selector coverage genuinely improved — uncovered `wpNN` prefixes fell from ten to four (`wp07`-`wp10`, 18 tests), and five new recipes exist. **The orphan claim stands in full:** none of the seven is invoked by `ci-fast` (`justfile:445`), `ci-pr` (`:449`), `governance` (`:441`), any `_model-profile-*` root, or `.github/workflows/ci.yml`, which contains zero occurrences of `wave`/`gate-b`/`git-parity`/`rebuild-equiv`. `gate-b-check` gave `adapter-wheel-test` its only referrer — and is itself orphaned.
Both wave acceptance oracles are orphaned and Wave 4 has none.
`justfile:121-129` defines `wave2-integration-check` and `wave3-integration-check`; neither is
invoked by `ci-fast` (`:409`), `ci-pr` (`:413`), any `_model-profile-*` root, or CI.
Their selectors cover `wp12-wp18` (37 tests) and `wp19-wp26` (36); **48 of the 96 `wpNN_*` tests
match neither** — all of `wp07`-`wp10` and all of `wp27`-`wp32`. There is no
`wave4-integration-check` although Wave 4 code has landed with `wp29`-`wp32` suites.
Also orphaned: `source-capture-race-check` (sole runner of the `#[ignore]`d race campaign) and
`adapter-wheel-test`.

## DP-062 · CONFLICT · major · P25 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS → p3 STANDS (worse)
**Pass 3 —** Undercounted. The census is **10+**, not six: add `aggregate_driver.rs:1748`, `publication.rs:1716`, `assurance.rs:686`, `serving.rs:2050`. Anchors moved (`git_state.rs:421` → `:694`; `fact_ingest.rs:2403` → `:2599`); the concat trick that keeps the test from tripping its own literals is intact.
Six acceptance tests assert on **source text** rather than behavior, via `include_str!`.
`tests/integration/coordinator.rs:208-211` counts `"bootstrap_sync("` occurrences;
`tests/integration/git_state.rs:421-431` builds its forbidden strings by concatenation
(`["edit","_reference"].concat()`) so the test's own text does not trip its own check;
`src/fact_ingest.rs:2403-2407` asserts the literal YAML of a governance rule
(`boundary_rule.contains("ignores:\n  - src/fabric.rs")`);
`src/fabric/mutation.rs:1057-1069` substring-scans the schema IR instead of decoding it;
plus `proto_driver.rs:823-865` and `incremental.rs:842-843`.
Two of these duplicate policies that already have proper structural detectors
(`rules/gix-read-only.yml`, `rules/deltalake-boundary-only.yml`) — the weaker text oracle breaks
on reformatting and passes on semantic change.

## DP-063 · CONFLICT · major · P12/P25 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
The public-schema oracle meta-validates the schemas and never validates an instance.
`tooling/model/validate_staged_schemas.py:41-68` checks there are 8 declarations, that each
`$schema` is Draft 2020-12, that `$id` matches the path, and calls
`Draft202012Validator.check_schema`. **No document is ever validated against any schema.**
Seven of the eight public schemas — including `contracts/query/planspec.schema.json` — have zero
references from any `.rs`/`.py`/`.sh`. The lone exception (`src/analysis_context.rs:317-344`)
compares only the *key set* of `schema["properties"]`, not types, `required`, or constraints.

## DP-064 · CONFLICT · major · P22/P25 · reachability: production
**PARTIAL** · p1 raised → p2 PARTIAL → p3 PARTIAL
The adapter contract suite proves a test-local probe server, not the shipped boundary.
`codefabric-cpg-mcp/tests/test_adapter_contracts.py:69-124` constructs its own
`probe_mcp = FastMCP(...)` and decorates two tools; every proof in that 319-line file — schema
modes, fingerprint policy, tool listing, `fastmcp inspect` CLI parity — runs against it.
`server.py:29-39` publishes zero tools, so none of it touches production.
The tool-manifest fingerprint has no accepted baseline: `:247` asserts only `startswith("b3:")`,
and `contracts/adapter/adapter-fingerprints.json` contains no tool-manifest fingerprint at all.

## DP-065 · CONFLICT · major · P22 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The generated public MCP boundary schemas are consumed by nothing.
`contracts/adapter/fastmcp-{input,output,public-meta}.schema.json` appear in **zero**
`.rs`/`.py`/`.sh`/`.toml`/`.yml`/`justfile` references. `tooling/model/validate_staged_adapter.py:70-78`
fixes `required_kinds` to five projection kinds, deliberately excluding `public-json-schema`.
The only property proved about the public MCP boundary is that regenerating it reproduces the same
bytes — yet `contracts/manifests/requirements.jsonl:10` asserts these artifacts have "executable
proof closure".

## DP-066 · CONFLICT · major · P22 · reachability: production
**TOKEN** · p1 raised → p2 TOKEN → p3 TOKEN
A fourth disjoint boundary family: `contracts/rpc/feature-registry.yaml` (`status: released`,
18 records across CPGD/PROVIDER/PYREFLY/RUSTC bit ranges, each with a
`requirement: optional|required|disabled`) has **no generated projection in either language**.
`src/rpc.rs:23` `negotiate_feature_bits` takes `supported` as an untyped `u64` and has **zero
callers in `src/`**. The only supported-mask value in the repo is a hand-written test literal,
`tests/integration/rpc.rs:43` `const SUPPORTED_FEATURES: u64 = 0b1111;` — the 4 CPGD bits only,
silently omitting the other three domains and encoding no `required` semantics.

## DP-067 · CONFLICT · major · P3/P22 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Two hand-written decoders of one artifact disagree on its contract, and Rust reads it across a
language boundary by path. `src/contracts/index.rs:13-16` `include_bytes!`s
`codefabric-cpg-mcp/src/codefabric_cpg_mcp/contracts/model_artifact_index.json` — a filesystem
coupling, not a protocol. Divergences: `resource_profile` is an open 4-`Option` struct in Rust
(`:24-33`) vs a closed union with `extra="forbid"` in Python (`index.py:29-37`); `authority_path`
safety is `!is_empty()` in Rust (`:132`) vs absolute/`..`/backslash rejection in Python (`:72-79`);
`consumers` is a `BTreeSet<String>` (dedups and reorders) vs an order-preserving
`tuple[str,...]` with `min_length=1`. No differential test compares them.

## DP-068 · CONFLICT · major · P21/P25 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
Three `released` registries with real semantics have zero executable consumers:
`contracts/security/security-corpus-manifest.yaml` (5 threat cases with expected status,
public fields, forbidden observations, resource bounds), `contracts/faults/fault-point-registry.yaml`
(7 fault points with allowed actions and expected invariants), and
`contracts/comparison/comparison-ignore-registry.yaml` (16+ non-semantic exclusions).
The fault registry is the sharpest: it declares 7 codes while the code declares 10 across three
unrelated enums — `OverlayRebaseFaultPoint` mirrors 3 **by naming convention only**,
`MutationFaultPoint` and `PublicationFaultPoint` are absent from the registry, and the four
`SOURCE_*` registry codes have no implementation. `RM §27.3` requires that faultability not be
retrofitted.

## DP-069 · CONFLICT · major · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
The model's declared consumer graph names a program that does not exist.
All **116** artifacts in `model_artifact_index.json` and `contracts/manifests/suite-manifest.json`
declare exactly one consumer, `"contract-verifier"` — verified: the distinct consumer set across
116 artifacts is `{'contract-verifier'}`, and the string occurs in no `[[bin]]`, recipe, script or
test. The `consumers` field therefore carries zero information, and Python's
`Field(min_length=1)` check (`index.py:49`) is trivially satisfied.
Separately, **zero** artifacts have an `authority_path` inside `codefabric-cpg-mcp/` — the adapter
ships a model index that does not describe its own generated contract artifacts.

## DP-070 · CONFLICT · minor · P22 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
A generated contract module imports an undeclared dependency the one Python rule does not cover.
`codefabric-cpg-mcp/.../contracts/fingerprints.py:6` does `from mcp.types import Tool as MCPTool`;
`mcp` is absent from `pyproject.toml` `[project].dependencies` (a transitive import via FastMCP).
`rules/no-framework-internal-contract-imports.yml:12` forbids `fastmcp.`/`pydantic._internal`
inside `contracts/**/*.py` but does not cover `mcp.*`, so the generated module legally imports a
framework type against the rule's stated intent.

## DP-071 · CONFLICT · major · P25 · reachability: production
**STANDS (worse)** · p1 raised → p2 STANDS → p3 STANDS (worse)
**Pass 3 —** **Worse, and worse by a governance edit made this cycle.** `rules/authoritative-source-read-boundary.yml` widened its `ignores` from 5 to 7, adding `src/golden_corpus.rs` and `src/query_service.rs`. The rule blast radius shrank while the code grew ~11k lines. Snapshot assertions still absent and skipped; `provider-observation-boundary-only` still scopes the nonexistent `src/providers/**`; still one Python rule and none covering `server.py`/`settings.py`/`__main__.py`/`channel.py`.
Rule-set coverage gaps behind an otherwise strong 16-rule / 16-test 1:1 harness.
Snapshot assertions are structurally absent and explicitly skipped: `sgconfig.yml:4` declares
`snapshotDir: __snapshots__`, the directory does not exist, and `justfile:246` passes
`--skip-snapshot-tests` — so rule tests assert match/no-match only, never range or message.
`rules/provider-observation-boundary-only.yml` scopes to `src/providers/**/*.rs`, **a directory
that does not exist**. Four rules are scoped to a single file each, so a new module violates them
freely. 15 of 16 rules are Rust; **zero rules cover `server.py`, `settings.py`, `__main__.py`, or
`daemon/channel.py`**, so nothing structurally enforces `SRV §6` invariants 11, 19, or 20.

## DP-072 · CONFLICT · minor · P23 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`SRV §6` invariant 20 ("settings immutable for process lifetime") holds by convention.
`settings.py:28-38` gives **per-instance** immutability (`frozen=True`), but there is no singleton
— no `@lru_cache`, no module-level instance — although sibling contract modules use
`@lru_cache(maxsize=1)` in five places (`contracts/index.py:82,89,106`, `contracts/schemas.py:15,25`).
`Settings()` is constructed inside `@lifespan` (`server.py:17-26`), i.e. per connection, reading
`os.environ` at entry — which is why `test_server.py:13-16` can `monkeypatch.setenv` immediately
before connecting. No oracle asserts one instance per process.
Adjacent undeclared module state: `MODEL_BY_NAME`/`TYPE_ADAPTERS`/`MODEL_ADAPTERS` are plain
mutable `dict`s (`wire_models.py:302-308`) where the sibling `registries.py` uses `MappingProxyType`.

## DP-073 · observation · minor · P25 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
`src/compatibility.rs` and its tests are library-capability probes, not CodeFabric contracts —
`wp27_structural_acceptance:39` asserts `language.abi_version() == 15`, a tree-sitter fact. The
module says so itself (`:5`), but it is compiled into the default profile via
`local-workstation = ["daemon", "compatibility-probes"]` (`Cargo.toml:113`). This is the clearest
instance of tests written around whichever functions exist rather than around a contract.

## DP-074 · CONFLICT · minor · P3 · reachability: production
**STANDS** · p1 raised → p2 STANDS → p3 STANDS
**Pass 3 —** **Live demonstration completed.** The row registering this very artifact type landed this cycle in `tooling/ci/artifact_contracts.py:116-121` and `:136-141` — and not in `artifact-schemas.md §7`. At `d89cc90` the validator accepts a type the documented table does not list, and nothing compares them.
Discovered while registering this review's own artifact type: the review-artifact vocabulary
itself has two authorities. `.claude/skills/_shared/artifact-schemas.md §7` carries the table of
`artifact` values and their required keys; `tooling/ci/artifact_contracts.py:109-116`
(`REVIEW_REQUIREMENTS`) and `:117-130` (`REVIEW_VERDICTS`) carry the same vocabulary again as
Python literals. Adding a type to the documented table does not register it with the validator, and
nothing compares the two. The §7 text asserting that "a new report type gets a row here before
first use" is therefore only half the registration.
Detector: compare the `artifact` column of `artifact-schemas.md §7` against the keys of
`REVIEW_REQUIREMENTS`; they must be equal.


## DP-075 · CONFLICT · blocker · P1/P25 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
**~10,600 lines of new Rust landed with zero inbound edges from `daemon::serve`.**
Eight new modules are declared in `src/lib.rs` behind `feature = "daemon"` and form a closed
island: `query_service → {semantic_query, lifecycle}`, `lifecycle → core_facts`,
`core_facts → rustc_service`, `continuous → lifecycle`; `derivation` and `golden_corpus` are
referenced by nothing. Inbound references from outside the island: **zero** for `continuous`,
`query_service`, `derivation`, `golden_corpus`.
`daemon::serve` (`src/daemon.rs:771`) still imports only `coordinator`, `fabric`,
`operational_store`, `registries`, `workspace_registry`. `StaticConfig` (`daemon.rs:41-60`)
declares exactly one socket — the admin IPC — so even a started gRPC service would have no
configured address to bind.
Consequence for this register: every "resolution" living in these modules is **unproven**, and
`ContinuousWorkspaceEngine` — the component that would drive the pipeline — is constructed once,
at `src/continuous.rs:484`, inside `#[cfg(test)]`.
Detector: `rg -l --type rust 'crate::(query_service|continuous|derivation|golden_corpus)' src/ tests/` → empty

## DP-076 · CONFLICT · blocker · P20 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
The daemon advertises a bundle digest that matches no installed bundle.
`src/query_service.rs:700-703` hardcodes
`bundle_digest: "b3:f4d3d3f7fff40534c5cbd2e54a7808193cdc82b61034ef525519ba64e14d5e7b"` for
`codefabric.bundles.query-language-bundle`. The installed bundle
(`contracts/bundles/query-language-bundle.json`) is
`b3:d48d7550f96dbbd67c87bbc63d5123cd13e5ebd68f35e61e0f857a6f22fc0a48`.
Repo-wide, the advertised value occurs in exactly two places: this line and
`tests/golden/codefabric-golden-v1/corpus-manifest.json` — it is copied from a golden-corpus
*expectation*, not read from the bundle. This is a compatibility-negotiation field.
Detector: `grep -A1 'bundle_digest' src/query_service.rs` vs `jq -r .bundle_digest contracts/bundles/query-language-bundle.json`

## DP-077 · CONFLICT · blocker · P20 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
A response asserts the opposite of what its own SQL does.
`src/semantic_query.rs:322` always emits `SELECT * FROM {} ORDER BY {} LIMIT {} OFFSET {}`, while
`:365` and `:422` set `limit_state: "NOT_APPLIED"`. The generated registry carries
`LimitState::ExplicitLimitReached = 20` (`src/generated/registries.rs:681`). `SRV §6` invariant 6
requires explicit limits, hard rejections and unavailable facts to stay distinct.
Detector: `rg -n 'LIMIT \{\}|limit_state: "NOT_APPLIED"' src/semantic_query.rs`

## DP-078 · CONFLICT · blocker · P20 (security) · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
**Pass 3 —** One sub-clause **superseded**: `stream_query` now does call `authorize_workspace`. The finding otherwise stands — `opaque_bytes` is still `blake3::Hasher::new()` (unkeyed), `cancel_token` is still `resume_token.clone()` (`query_service.rs:881`), and the keyed pattern is still demonstrated 60 lines earlier at `:138`.
Tokens named "opaque" are unkeyed and therefore forgeable.
`src/query_service.rs:64-69` `fn opaque_bytes` uses `blake3::Hasher::new()` — **unkeyed** — over a
public domain string and a value. `query_id` is derived from the request checksum (`:873`),
`resume_token = opaque_bytes(…, &query_id)` (`:874`), and `cancel_token = resume_token.clone()`
(`:877`). Any peer that knows the request bytes can compute another agent's resume and cancel
tokens. The same file demonstrates the correct pattern 60 lines earlier:
`ResultArtifactStore` uses `blake3::Hasher::new_keyed(&self.lease_secret)` (`:133`) seeded from
`/dev/urandom` (`:92`). Compounding: `stream_query` (`:919-938`) checks only the resume token and
never calls `authorize_workspace`, unlike `attach_query` (`:944`).
Detector: `rg -n 'fn opaque_bytes' -A4 src/query_service.rs | rg -c 'new_keyed'` → 0

## DP-079 · CONFLICT · blocker · P20 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
Every update wave declares its semantic capabilities terminal without evaluating a guard.
`src/continuous.rs:279-290` unconditionally drives
`transition(…, "semantic-work-not-applicable", "semantic-capabilities-terminal")` and then
`(…, "wave-output-valid", "required-capabilities-terminal")`.
`contracts/registry/state-machine-registry.yaml:97` offers the opposite branch
(`semantic-work-required` / `semantic-capabilities-applicable`). `generated_transition` validates
that a transition is **legal**; nothing checks the guard is **true**. So a wave containing Rust
files that require `RUST_MIR` still declares required capabilities terminal with no semantic
provider having run — the wave-level analogue of a hardcoded `completeness_state: "COMPLETE"`,
and a direct contradiction of `RM §1` invariant 6.

## DP-080 · CONFLICT · major · P20 · reachability: unreachable · verdict: NEW
**PARTIAL** · raised p3
**Pass 3 —** One constant became runtime: `query_service.rs:512` now matches on `freshness_policy` for admission. Roughly eight others stand — the six `&'static str` result states (DP-110), `failed_query_count: 0`, `errors: Vec::new()`, unconditional `readiness: Ready` (`:721`,`:783`), and three empty capability/fingerprint vectors (`:709`,`:730`,`:763`). `FreshnessBarrier::default()` (`:353`) is still the production constructor; `with_freshness_barrier` is called only from a test.
Runtime state is advertised as compile-time constants across the new query path.
`src/semantic_query.rs:361-364` and `:419-422` set `execution_state: "SUCCEEDED"`,
`availability_state: "AVAILABLE"`, `completeness_state: "COMPLETE"`, `freshness_state: "CURRENT"`
as literals; `failed_query_count: 0` (`:424`); `errors: Vec::new()` (`:429`). No code path emits a
non-success value. `request.freshness_policy` is parsed (`:101`) and never read.
`ProductionQueryService::new` (`src/query_service.rs:335-349`) installs `FreshnessBarrier::default()`
— `admitted=0, reconciled=0`, which `src/lifecycle.rs:1417-1426` reports as `Current` forever;
`with_freshness_barrier` (`:354`) has one caller, a test. `readiness: WorkspaceReadiness::Ready` is
unconditional (`:717`, `:779`); `active_schema_fingerprints`, `capability_codes` and
`capability_statuses` are advertised as empty vectors (`:705`, `:726`, `:759`) while
`core_facts.rs` builds real `CapabilityEvidence` that is never plumbed through.

## DP-081 · CONFLICT · major · P20 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
Two advertised guarantees are never enforced. `RESULT_LEASE_SECONDS` (`src/query_service.rs:47`)
produces `lease_expires_at_unix_ms`, computed at `:143` and returned to clients at `:626`, `:1103`
— and `read_result` (`:1056-1107`) never compares it to the current time. `WorkspaceClaim.permission_claims`
is required non-empty at construction (`:184`) and then never read; `authorize_workspace` (`:236`)
checks membership only.

## DP-082 · CONFLICT · major · P25 · reachability: production (fixtures) · verdict: NEW
**PARTIAL** · raised p3
**Pass 3 —** Shape changed, substance did not. The module grew 385 → 729 and each of the eleven expectations is now decoded and checked rather than only hashed. But **8 of 11 compare Rust literals to Rust literals**; the three that reach real authority (`PROVIDER_IDS`, `table_specs()`, `DURABLE_PUBLICATION_STATE_VALUES`) use `is_subset`/`any`, so an empty array passes. Nothing executes: `scenario.json` is still never deserialized, and `core_source_v1_coverage` compares directory names. It still does not close DP-060.
The golden corpus is a self-hashing tamper check over stubs, not an oracle.
`src/golden_corpus.rs:230-324` reads `corpus-manifest.json`, walks the member roots, checks names
against three hardcoded `const` arrays (`:13-44`), and asserts the BLAKE3 of the concatenated file
bytes equals a digest recorded in the manifest. It executes nothing and compares nothing to system
output. The 11 "expected answers" are 106-186-byte descriptions of checks —
`expected/queries/gate-b.json` is `{"forms":[…],"ordering":"canonical","profile":"gate-b-v1"}` —
and the 16 scenarios are one-line stubs such as
`{"scenario_id":"000_clean_bootstrap","edits":[],"expected_terminal":"CURRENT"}`. No `scenario.json`
is ever deserialized. The three tests assert the corpus hashes to a hardcoded literal and that
mutating a file breaks that hash — a self-referential tautology. `corpus_status` is `CANDIDATE`
while the profile inside is `RELEASED`.
This is the change most likely to be mistaken for progress: it does **not** close DP-060 — the
negative fixtures remain unconsumed by any executable code.

## DP-083 · CONFLICT · major · P3 · reachability: production · verdict: NEW
**STANDS** · raised p3
A cross-process agreement algorithm is implemented twice, byte-identically, in two Cargo roots on
two different toolchains. `digest_frames` and `valid_digest` are duplicated verbatim at
`src/rustc_service.rs:55-72` and `rustc-extractor/src/wrapper.rs:92-109`. These compute the
`owner_content_digest` the daemon and extractor must agree on; there is no shared crate.
Detector: `rg -n 'fn digest_frames' src/ rustc-extractor/` → 2 hits, no common definition

## DP-084 · CONFLICT · major · P22 · reachability: production · verdict: NEW
**STANDS** · raised p3
Two wire protocols exist for the extractor seam, and the tested path is not the shipping path.
`rustc-extractor/src/main.rs:143-157` retains `--extract-json` with its own hand-written
`ExtractRequest`/`ExtractResponse` (`:48-69`), its own `protocol_version: "1.0"` string check
(`:75`) and its own digest helper (`:71`) — running the compiler and returning JSON on stdout,
bypassing `wrapper.rs`, the `.proto`, `RustcObservationService`, and the whole flow-control and
validation path. The extractor's own `wp35` determinism test exercises this path, not the gRPC one.

## DP-085 · CONFLICT · major · P3 · reachability: production · verdict: NEW
**STANDS** · raised p3
A wire code is reconstructed by arithmetic on a registry array position.
`rustc-extractor/src/wrapper.rs:43-50` computes the `RUST_MIR` capability code as
`(index_in(CAPABILITY_IDS, "RUST_MIR") + 1) * 10`. Reordering the registry silently changes the
value on the wire. This is the `CAPABILITY_IDS: &[&str]` name-list defect (DP-002 family) turned
into a live encoding rule.

## DP-086 · CONFLICT · major · P3 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
The same fingerprint is computed two different ways in one file.
`src/core_facts.rs:449-455` derives `coverage_scope_fingerprint` from
`b"codefabric-capability-scope-v1\0"` + (workspace, context, owner, generation, capability_code);
`:1197-1207` derives a field of the same name from `b"codefabric.rustc.capability-scope.v1\0"` +
(workspace, context, generation, owner, chunk_digest). Different domain separator convention
(`-` vs `.`), different field sets, different order.
Adjacent: `stable_id16` (`:1154`) mints entity ids by truncating a bare `blake3::hash`, and
`semantic_key` (`:1144`) re-implements length-framed hashing — bypassing
`crate::identity::derive_identity` over `CbefRecord`, which the file imports at `:28`.

## DP-087 · CONFLICT · major · P22 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
The adapter's cancellation path depends on an undeclared implementation detail.
`CancelQueryRequest.cancel_token` (`contracts/rpc/cpg_query_service.proto:316`) is a distinct
field, but `StartQueryResponse` (`:219-229`) returns only `resume_token`. The Python client passes
the resume token as the cancel token (`daemon/client.py:290`). This works only because
`src/query_service.rs:877` does `let cancel_token = resume_token.clone();`. Any daemon honoring the
proto's separation silently breaks cancellation — defeating `SRV §6` invariant 10
("cancellation is end-to-end").

## DP-088 · CONFLICT · blocker · P22 + `SRV §6` inv. 3/6/8/9 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
Python re-derives daemon domain state instead of presenting it.
`codefabric-cpg-mcp/.../server.py:171` hardcodes `execution_state="COMPLETE"`; `:173` hardcodes
`completeness_state="COMPLETE"`; `:142` hardcodes every clause's `state="COMPLETE"`. Lines
`:172`, `:174-176`, `:177-179` collapse the daemon's `availability_state`, `freshness_state` and
`limit_state` through Python ternaries into a narrower vocabulary — anything not `CURRENT`/`PINNED`
becomes `POTENTIALLY_STALE`. `:185` computes `truncated` in Python.
This is precisely the collapse `SRV §6` invariants 6, 8 and 9 forbid, performed in the layer that
`RM §1` invariant 8 requires to remain a thin adapter.

## DP-089 · CONFLICT · major · P22 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
The adapter is a second semantic validator, and one field is transported twice in two encodings.
`daemon/client.py:170-179` reads `request["freshness_policy"]` and maps three string literals to
proto enums; `:240` extracts `semantic_request_id` from the request dict — while `:242` also sends
the full canonical request as bytes. `SRV §7` lists a second query interpreter as an explicit
non-goal.

## DP-090 · CONFLICT · major · P23 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
Resource existence is authoritative in adapter memory rather than in the lease owner.
`daemon/client.py:68` declares `self._leased_artifacts`, mutated at `:309`, `:399`, `:137`.
`read_resource` (`:402-407`) raises "result resource is absent or already released" from this local
dict without asking the daemon. On adapter restart every outstanding lease leaks until daemon-side
expiry; a lease revoked daemon-side still reads as present locally. This is the
cache-as-second-authority pattern of DP-035, in the layer least entitled to it.

## DP-091 · CONFLICT · major · `SRV §6` inv. 7/13 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
Structured daemon diagnostics are flattened into a string.
`server.py:230-237` maps every `canonical_error_records_json` entry to
`ValidationIssue(code="SEMANTIC_QUERY_INVALID", path=(), message=str(parsed_json))` — destroying
the daemon's own error code and path, substituting one hardcoded code and an empty path, and
stuffing the original JSON into a human-readable string. "Unknown remains data" fails in the one
place the adapter sees structured diagnostics.

## DP-092 · CONFLICT · minor · P20 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
Two adapter defects of shape. `server.py:332-334` returns
`ResourceReference(uri=f"cpg://reference/{reference}/1.3")` for six identities, while the only
registered resource template is `cpg://result/{artifact_id}` (`:200`) — those URIs resolve to
nothing, and the catalog test asserts `list_resources() == []`, so the branch is untested.
`daemon/client.py:367-386` contains `while True:` re-issuing `ReadResult` at the same offset
whenever the stream yields no chunk and `final` stays false — an unbounded RPC loop inside an MCP
tool call whose only bound is the 120 s FastMCP timeout.

## DP-093 · CONFLICT · major · P22 · reachability: production (adapter) · verdict: NEW
**STANDS** · raised p3
The adapter computes and sends a host capability profile digest that the daemon never validates.
`daemon/client.py:26-34` computes `HOST_PROFILE_DIGEST` at import from a Python-local literal dict
and sends it on `Handshake` (`:100`), `ValidateQuery` (`:194`) and `StartQuery` (`:250`); the proto
declares the fields (`cpg_query_service.proto:94,189,213`). The Rust handler never reads
`host_capabilities` — the single `profile_digest` hit in `src/query_service.rs:712` is the daemon's
own outbound limits digest — and discards the adapter's schema fingerprints
(`active_schema_fingerprints: Vec::new()`, `:705`). The digest's derivation rule is specified
nowhere in `contracts/` or `SRV`.

## DP-094 · CONFLICT · major · P3 · reachability: production · verdict: NEW
**STANDS (worse)** · raised p3
**Pass 3 —** **Worse, and the finding's own narrative is now false.** Pass 2 said `skills/` was inert because all tooling hard-codes `.claude/skills`. This cycle `scripts/seed_zero_state_check.sh` gained `-g '!skills/**/REFERENCE.md'` — a governance script has begun referencing the duplicate tree. It remains a real, untracked, non-ignored directory.
An exact, untracked duplicate of the skills tree exists at the repository root.
`diff -rq skills/ .claude/skills/` produces no output — 36 files, byte-identical, including a
copied `.DS_Store`. It is a **real directory**, not a symlink (`.codex/skills` and `.agents/skills`
are symlinks), and it is **not gitignored** (`git check-ignore` exits 1).
This contradicts `AGENTS.md §10.2`, which states that `.codex/skills` and `.agents/skills` symlink
to `.claude/skills` "so a skill is edited once and both read it". All tooling hard-codes
`.claude/skills` (`scripts/bootstrap.sh:310,321`, `scripts/seed_zero_state_check.sh:35`), so
`skills/` is inert — and one `git add -A` away from becoming a committed second source of truth.
The risk is already live: the `design-principles-conformance` schema row added this cycle exists in
both copies, kept in step by nothing.
Detector: `test ! -e skills || { test -L skills && test "$(readlink skills)" = ".claude/skills"; }`

## DP-095 · CONFLICT · major · P3 · reachability: unreachable · verdict: NEW
**STANDS** · raised p3
The generated state registries are re-emitted as bare string literals throughout the new code.
Six vocabularies exist as generated enums — `QueryExecutionState` (`src/generated/registries.rs:476`),
`QueryAvailabilityState` (`:543`), `CompletenessState` (`:589`), `FreshnessState` (`:642`),
`LimitState` (`:681`), `DependencyState` (`:720`) — and appear as literals 29 times in
`src/query_service.rs` and 12 times in `src/semantic_query.rs`.
`src/lifecycle.rs:849` demonstrates the correct pattern (`registry_state_name(...)` feeding
`generated_transition`), which is what makes the literals elsewhere a defect rather than a style.
Adjacent: `supported_language_codes: vec![10, 20]` (`query_service.rs:720`) as bare ordinals, and
`RequestForm` (`registry_models.rs:533`) → `QueryForm` (`semantic_query.rs:26`) → three string
literals (`query_service.rs:749`) — three copies of one concept.

## DP-096 · CONFLICT · major · P16 · reachability: mixed · verdict: NEW
**STANDS (worse)** · raised p3
**Pass 3 —** New-module `#[error]` variants grew 61 → **65** after the review documented the pattern; still zero phase fields, `wrapper.rs` still `Result<_, String>` at 11 sites, and "phase" appears once, in a doc comment.
The error anti-pattern was propagated into the new code at scale.
The new modules add **61 error variants across 8 enums; none carries a phase field**. The word
"phase" appears once in `src/lifecycle.rs`, in a doc comment (`:1545`). Seventeen new variants use
the `#[error("CODE:{0}")] Variant(String)` shape — `lifecycle.rs` 9, `core_facts.rs` 4,
`derivation.rs` 3, `semantic_query.rs` 1. `rustc-extractor/src/wrapper.rs` has no error type at
all, using `Result<_, String>` throughout.
This is DP-016 and DP-017 reproduced in code written after the review documented them.

## DP-097 · CONFLICT · minor · P3 · reachability: unreachable · verdict: NEW
**STANDS (worse)** · raised p3
**Pass 3 —** **Worse.** `derivation.rs:65-72` now hardcodes **8** registry fields, up from 6 — `precision_profile` and `algorithm_version` were added. The two fields the register flagged as "checked by neither" are now checked by a second verbatim copy rather than by the registry.
`src/derivation.rs:63-70` hardcodes the expected values of six registry fields it then compares the
registry against (`owner_kind`, `input_fact_families`, `output_fact_families`, `replacement_scope`,
`dependency_rule`). The registry is nominally the authority while Rust holds a verbatim second copy
and rejects any registry change; `precision_profile` is in the registry and checked by neither.

## DP-098 · CONFLICT · major · P8 · reachability: unreachable · verdict: NEW
**STANDS (worse)** · raised p3
**Pass 3 —** New modules now expose **68** public structs and 16 public enums; `semantic_query.rs:510` still synthesizes ids with the literal prefix `"entity:unknown:"` while `identity.rs:1138 encode_public_id` exists.
A new DTO family converts Arrow away rather than reusing it.
`src/semantic_query.rs:184-188` types `entities`/`facts`/`paths`/`groups`/`source_contexts` as
`BTreeMap<String, BTreeMap<String, String>>`, populated at `:337-348` with each id echoed as its
own only field — so a `RecordBatch` becomes stringly-typed nested maps carrying no fact content.
`:451` synthesizes public ids by prefixing the literal `"entity:unknown:"`, bypassing
`crate::identity::encode_public_id` (`src/identity.rs:1138`).
The new modules add 66 public structs and 16 public enums.

## DP-099 · CONFLICT · major · P25 · reachability: mixed · verdict: NEW
**STANDS** · raised p3
Not one of the 111 new test functions names a contract.
Across the ~10,600 new lines, tests are named `wpNN_*` / `waveN_*`; **zero** are named for an
`AC-G` contract. Only five `AC-G` strings appear in all the new code, every one a doc comment on a
constant (`src/rustc_service.rs:35,37`; `src/core_facts.rs:81,159,358`).
On the adapter side, `test_server.py` now drives the production `mcp` object — real progress — but
its two tests cite no contract, and both monkeypatch `CpgDaemonClient.connect/close/execute/validate/status`,
so all 421 lines of `daemon/client.py` (checksum verification, stream ordering, lease lifecycle,
cancellation) are exercised by nothing.


## DP-100 · CONFLICT · blocker · P19/P25 · reachability: test-only
**NEW** · raised p3
**`SUITE AC-G-79` / `RM §1` invariant 10 — "incremental results converge to the clean-rebuild
result" — is asserted but never proved.** WP48's own acceptance sentence requires the AC-G-79
comparator over a core edit corpus.
The comparator is `assert_matches_clean_rebuild` (`src/continuous.rs:465-490`). It clones
`result.wave` — the wave the engine just reconciled — sets `wave.state = FastAnalyzing` to bypass
the phase guard, and re-runs `FastSyntaxReconciler::default().reconcile_wave(...)` on that same
captured wave. It never re-walks inventory, never re-captures bytes, never materializes effective
state. `AC-G-79 §79.2` requires `effective = durable base − overlay tombstones/replacements +
overlay rows` and forbids comparing only the durable base when an overlay is present; no effective
state is materialized on either side. No `ComparisonInput` domain check, and
`contracts/comparison/comparison-ignore-registry.yaml` is read by nothing.
What *is* proved is real but strictly weaker: incremental Tree-sitter parsing of a captured wave
equals a stateless re-parse of the same captured bytes.
Detector: `rg -n 'fn assert_matches_clean_rebuild' -A6 src/continuous.rs | rg -c 'result\.wave\.clone'` → 1

## DP-101 · CONFLICT · blocker · P20/P25 · reachability: production (fixtures)
**NEW** · raised p3
**Readiness Gate B is not executed.** `gate-b-check` (`justfile:146`) is dependency-only, with no
body: `wave5-integration-check adapter-wheel-test model-release-census-check`. Its docstring claims
to "Prove Readiness Gate B across all eleven accepted golden artifacts".
`SUITE:2255-2257` defines Gate B as eleven things passing **end to end** — one Python owner, one
Rust MIR owner, one unknown fact, one property fact, one relation fact, one derived projection, one
hot-overlay update, one durable publication, one semantic query, one streamed result, one artifact
result. **None of the eleven executes.**
The eleven `expected/*/gate-b.json` files are contract *descriptors*, not outputs — e.g.
`expected/identities/gate-b.json` is `{"identity_algorithm":"CBEF-v1","requirements":[…],"profile":"gate-b-v1"}`
and `expected/rpc/gate-b.json` is `{"transport":"unix-domain-socket","deadline_required":true,…}`.
There are no expected IDs, rows, response bytes, or checksums anywhere under `expected/`.
`execute_artifact_contract` (`src/golden_corpus.rs:427-548`) compares them to compiled constants
and string literals; `"CBEF-v1"` is a bare literal, not sourced from `crate::identity`.
`RM §10` requires "All IDs, rows, response bytes, and checksums match released golden outputs" —
uncheckable, because no such outputs exist.
Fairly noted: `corpus_status` is honestly `CANDIDATE`, and `SUITE:964-968` permits a partial
`gate-b-v1` profile. But `SUITE:970` releases the corpus only when every expected-output plane is
"populated and verified", and M06 requires proof "over owner-accepted golden **answers**" — there
are rules, not answers.

## DP-102 · CONFLICT · blocker · P25 · reachability: production (fixtures)
**NEW** · raised p3
**M07 requires `rebuild-equivalence-check` to pass "every core edit scenario"; the 16 scenarios are
asserted to exist as directories and are never executed.**
`wp48_structural_acceptance` (`src/golden_corpus.rs:719`) asserts
`coverage.scenario_ids.len() == 16` and equality with `REQUIRED_SCENARIOS` — a directory-name
census. Each payload is a one-line descriptor:
`scenarios/010_python_local_edit/scenario.json` = `{"scenario_id":"010_python_local_edit","edits":["replace-python-body"],"expected_terminal":"CURRENT"}`.
`edits` names a verb no code implements and `expected_terminal` is never read.
Verified: `rg -c 'scenario\.json' --type rust .` → **0** — no scenario file is ever deserialized.
Missing outright from the WP48 acceptance list: overflow, multi-file logical save, context change,
capability withdrawal.

## DP-103 · CONFLICT · blocker · P20/P25 · reachability: test-only
**NEW** · raised p3
**M08 requires "forced gix-disabled, cache-disabled, and full-rebuild comparisons are semantically
identical"; no code performs any of the three.** `wave7-integration-check` (`justfile:165`) is
dependency-only — `git-parity-check rebuild-equivalence-check` — so it is the union of two recipes
neither of which compares a disabled configuration to an enabled one.
`git-parity-check`'s own docstring says "Compare Git-accelerated candidates and state with
authoritative fallback". No test in `tests/integration/git_state.rs` constructs the authoritative
`InventoryWalker` fallback and compares it to the accelerated result.
`wp50_operational_acceptance` (`:808-840`) is labelled "inventory parity fixture" and asserts only
that the gix inventory *contains* two filenames.
WP53's "gix-disabled and cache-disabled states equal the accelerated result across the WP48
comparator corpus" has no implementation.

## DP-104 · CONFLICT · blocker · P25 · reachability: production (tests)
**NEW** · raised p3
**Five acceptance oracles for completed packets are one-line aliases that re-call an earlier
packet's body, adding no assertions.** Verified at `tests/integration/git_state.rs`:
```
:744  fn wp49_behavioral_acceptance()  { wp17_behavioral_acceptance(); }
:749  fn wp49_structural_acceptance()  { wp17_structural_acceptance(); }
:754  fn wp49_negative_zero_state()    { wp17_negative_zero_state(); }
:773  fn wp50_behavioral_acceptance()  { wp17_structural_acceptance(); }   // structural body, behavioral name
:932  fn wp52_operational_acceptance() { wp52_behavioral_acceptance(); }
```
WP49's acceptance sentence requires "linked/bare/detached/unborn/non-UTF8/corrupt cases,
gix-disabled equivalence, and no direct domain leakage beyond DTOs". Three aliases to WP17 plus a
thread-pool smoke test asserting an executor returns `49` (`:759-770`) do not meet it.
`git-parity-check` selects `test(/wp(49|50|51|52|53)/) --no-tests=fail`, so the gate passes because
tests *exist*, not because they assert anything new. Any "each packet has four acceptance oracles"
check is satisfied by construction.
Detector: fail any `#[test] fn X() { Y(); }` whose body is a single call to another `#[test]` fn.

## DP-105 · CONFLICT · major · P20 · reachability: test-only
**NEW** · raised p3
`RM §11` and `§25` require `CORE_SOURCE_V1` to be "advertised `COMPLETE` for supported files" with
"exact coverage returned per owner", and the capability "continuously maintained".
`core_source_v1_coverage` (`src/golden_corpus.rs:409`) has exactly one caller — the test at `:720`.
Coverage is computed and never returned to anyone. The continuous engine that would maintain it is
constructible only from tests (see DP-075).

## DP-106 · CONFLICT · major · P25 · reachability: production (process)
**NEW** · raised p3
**One commit closes 22 packets, 4 milestones and 3 decommission batches, against a plan whose
dependency DAG is declared normative and whose packet contract requires per-packet proof.**
`plan:129` — "completion requires all packet acceptance at a proving commit that is an ancestor of
HEAD and again at current HEAD"; `plan:585-590` — "The exact direct dependency edges … are
normative". `35fc632` is 153 files, +20,163 insertions, and is the single `consolidated_proving_commit`
for WP32–WP53.
The state discloses this openly rather than minting artificial per-packet commits, which is honest —
but it means no packet's acceptance was ever demonstrated at its own boundary, and the normative DAG
went unexercised.

## DP-107 · CONFLICT · minor · P3 · reachability: production (process)
**NEW** · raised p3
Two artifacts in the tree assert contradictory completion state, and the older one was left
unamended. `docs/plans/state/codefabric-waves-4-7-core-facts_v5_state.json` records `status: complete`
with all packets and M05–M08 complete. `docs/reviews/implementation_status_…_2026-08-23_v1.md`
still records WP34/WP35 (and others) `in_progress`, M05–M08 `not_started`, WP40 `not_started`, and
lists as a blocker that the seven wave recipes "do not exist yet".
Separately, the plan's own §12 completion checkboxes remain unchecked for every wave-5/6/7 line
while the state it governs says complete.

## DP-108 · CONFLICT · major · P25 · reachability: production
**NEW** · raised p3
**The governance layer counts oracle *names*, not oracle *substance* — the structural reason a
"complete" state and an unexecuted Gate B coexist with green gates.**
`tooling/ci/artifact_contracts.py:411-420` (`_validate_oracle_catalog`) verifies only that each
packet block in the plan *text* declares four uniquely-named oracles; `plan-status` (`:819-834`)
checks baseline ancestry, input freshness and commit trust. Nothing verifies that an oracle's body
proves its acceptance sentence, and nothing runs the oracles.
Fair credit, verified: all 108 declared oracle names do resolve to real functions. The gap is
between naming and proving, not fabrication.

## DP-109 · CONFLICT · blocker · P20 · reachability: unreachable
**NEW** · raised p3
The live freshness reading is consulted only on the failure path and discarded on success.
`lifecycle.rs:1479` — `FreshnessAdmission::BestAvailable => return Ok(self.state())` can return
`PotentiallyStale`. `query_service.rs:521` discards it (`Ok(_) =>`). The success terminal
(`query_service.rs:639`) reads `executed.response.freshness_state`, which is the compile-time
literal `"CURRENT"` (`semantic_query.rs:480`). `freshness.state()` is read only at
`query_service.rs:544-547`, the error branch.
=> a `best_available_snapshot` query over a stale workspace reports `CURRENT`.

## DP-110 · CONFLICT · blocker · P20 · reachability: unreachable
**NEW** · raised p3
Six per-query result states are typed so they *cannot* hold runtime state.
`semantic_query.rs:123-128` and `:194-198` declare `execution_state`, `availability_state`,
`completeness_state`, `freshness_state`, `limit_state`, `dependency_state` as `&'static str`;
`:423-428` and `:477-481` populate them with `"SUCCEEDED"`, `"AVAILABLE"`, `"COMPLETE"`,
`"CURRENT"`, `"NOT_APPLIED"`, `"SATISFIED"`, plus `failed_query_count: 0` and
`not_executed_dependency_count: 0` (`:483-484`).
Detector: `rg -nE '(execution|availability|completeness|freshness|limit|dependency)_state: &.static str' src/` → must be 0

## DP-111 · CONFLICT · major · P20 · reachability: unreachable
**NEW** · raised p3
`EffectiveLimitsProfile.profile_digest` is the hash of a constant string, not of the limits.
`query_service.rs:716` — `profile_digest: digest(b"codefabric.local-query-limits.v1")`. The five
limit fields directly above (`:711-715`) are not inputs, so a client caching on the digest never
observes a limits change.

## DP-112 · CONFLICT · major · P20 · reachability: unreachable
**NEW** · raised p3
A declared state is unreachable in production while being advertised as a live outcome.
`lifecycle.rs:1450 mark_unavailable()` is the sole writer of the flag read at `:1457`, and its only
caller is `lifecycle.rs:2887` — a test. No production path can produce `FreshnessState::Unavailable`,
yet `query_service.rs:531,547,767` advertise it.

## DP-113 · CONFLICT · major · P20/P25 · reachability: production (fixtures)
**NEW** · raised p3
Ten manifest digest fields are declared, parsed, and never verified.
`golden_corpus.rs:59-68` declares `source_archive_digest`, `workspace_registration_digest`,
`context_manifest_digests`, `provider_bundle_digests`, `model_pack_bundle_digest`,
`ontology_bundle_digest`, `schema_bundle_digest`, `derivation_bundle_digest`, `query_bundle_digest`,
`tool_contract_bundle_digest`. None is read outside the struct definition. `corpus_status` (`:56`)
is likewise parsed and dropped — the manifest says `CANDIDATE` while `validate_profile:294` accepts
on the inner `profile_status`.

## DP-114 · CONFLICT · major · P20 · reachability: production (fixtures)
**NEW** · raised p3
The golden corpus's owner acceptance is self-satisfying.
`corpus-manifest.json` carries the identical value `b3:7801342c…` as
`coverage_profiles[0].canonical_digest`, `accepted_profile_digests[0]` **and**
`source_archive_digest`. `golden_corpus.rs:297-300` accepts the profile because
`accepted_profile_digests.contains(&profile.canonical_digest)` — the digest is compared to itself.
`accepted_by` is checked only for non-emptiness (`:295`), and `acceptance_basis` asserts "answers
are reviewed evidence and are never renderer output" with nothing enforcing it.

## DP-115 · CONFLICT · major · P25 · reachability: unreachable
**NEW** · raised p3
The only real rebuild-comparison extractor has no caller.
`CanonicalState::from_serving_session` (`lifecycle.rs:2121`) — verified: one hit repo-wide, the
definition itself. The `wp48` tests that stand in for it compare hand-built literals:
`lifecycle.rs:3144-3155` proves `vec![b"b",b"a"]` and `vec![b"a",b"b"]` hash equally, and `:3160-3168`
proves `[b"a"] != [b"b"]` — a test of `BTreeMap` ordering, not of canonical state.

## DP-116 · CONFLICT · major · P25 · reachability: production (tests)
**NEW** · raised p3
A "behavioral acceptance" test asserts a hardcoded digest of the very files it hashes.
`golden_corpus.rs:615-623 wp34_behavioral_acceptance` asserts `canonical_digest == "b3:7801342c…"`
and `files.len() == 39`, where that digest was computed by `digest_files` over those 39 files and
already compared to the manifest at `:344-350`. `wp34_operational_acceptance` (`:660-669`) then
recomputes the same value. Three copies of one number, none independently derived.

## DP-117 · CONFLICT · major · P3/P16 · reachability: production
**NEW** · raised p3
The new modules created a shadow error vocabulary while the registered one goes unused.
None of `LIFECYCLE_*`, `CONTINUOUS_*`, `DERIVATION_*`, `SEMANTIC_QUERY_INVALID`,
`GIT_STATE_UNAVAILABLE` appears in `PUBLIC_ERROR_IDS` (`src/generated/registries.rs:5600+`).
Conversely `SEMANTIC_PHRASE_AMBIGUOUS`, `SEMANTIC_PHRASE_UNRECOGNIZED`, `QUERY_HARD_LIMIT_EXCEEDED`
and `INVALID_REQUEST_SCHEMA` are all registered and appear **zero** times in
`src/semantic_query.rs` — the exact conditions they name are raised as
`SemanticQueryError::Invalid(String)` at `:264-273` and `:336-343`.
Detector: every `[A-Z][A-Z_]{4,}` prefix inside `#[error("…")]` must be a member of `PUBLIC_ERROR_IDS`

## DP-118 · CONFLICT · major · P3 · reachability: production
**NEW** · raised p3
`FreshnessState` is declared twice — generated and hand-written — and only the hand copy is used.
`src/generated/registries.rs:642` defines it with codes, `FRESHNESS_STATE_VALUES` and
`TryFrom<u16>`; `lifecycle.rs:1393` re-declares it with no codes and no registry link.
`rg 'registries::FreshnessState'` → 0 hits: the generated enum is dead. `query_service.rs:544-547`
and `:765-767` then re-spell the wire names as string literals — a fourth and fifth copy.

## DP-119 · CONFLICT · major · P3 · reachability: production
**NEW** · raised p3
The `codefabric-capability-scope-v1` fingerprint has two independent implementations, one unguarded.
`src/core_facts.rs:450-456` and `src/source_syntax.rs:731-740` hash the same five fields under the
identical domain string; only `core_facts.rs:447` validates
`CAPABILITY_IDS.contains(&capability_code)`, so `source_syntax.rs` will mint a scope fingerprint for
an unregistered capability.

## DP-120 · CONFLICT · major · P3/P18 · reachability: production
**NEW** · raised p3
27 new ad-hoc digest domains were introduced outside `crate::identity`, in two incompatible naming
conventions. `git diff eb27a5b HEAD -- src/ | grep 'b"codefabric'` yields 27 new prefixes — e.g.
`codefabric.derivation.syntax-tree.input.v1` (`derivation.rs:83`), `codefabric-golden-profile-v1`
(`golden_corpus.rs:218`), `codefabric.gate-b.execution.v1` (`:389`), `codefabric.continuous-state.v1`
(`lifecycle.rs:2190`), `codefabric.snapshot.base-table-versions.v1` (`semantic_query.rs:548`).
`src/identity.rs` owns CBEF-v1; none routes through it and none is registered.

## DP-121 · CONFLICT · major · P3 · reachability: production (fixtures)
**NEW** · raised p3
The golden corpus re-declares registry vocabularies as literal sets instead of reading the authority.
`golden_corpus.rs:496` — `BTreeSet::from(["cpg_base","cpg_control","cpg_serving"])`;
`:472` — `BTreeSet::from(["UNAVAILABLE_PARSE","UNSUPPORTED_CONTENT"])` while
`generated/registries.rs:332,5639` are authoritative; `:505-509` — the three query forms.
The same function *does* read `PROVIDER_IDS` (`:447`) and `table_specs()` (`:456`), so the file
demonstrates it knows the better pattern.
Also note the comparisons at `:460` use `is_subset`, not equality — an empty array passes.

## DP-122 · CONFLICT · minor · P3 · reachability: production
**NEW** · raised p3
The human-facing query-form vocabulary exists in six places and no registry:
`semantic_query.rs:29,31,33` (serde renames), `golden_corpus.rs:506-508`,
`expected/queries/gate-b.json`, `query_service.rs:726-728` (handshake), `:759-761` (status), and
`codefabric-cpg-mcp/tests/test_server.py:27`. `PHRASE_ENTRIES` carries only hyphenated
`plan_node_kind` values, which `semantic_query.rs:52-56` hand-maps to.

## DP-123 · CONFLICT · minor · P20 · reachability: unreachable
**NEW** · raised p3
Request DTOs advertise filters the implementation unconditionally rejects.
`QueryInput` (`semantic_query.rs:63-69`), `QueryPredicate` (`:71-77`) and `response_projection`
(`:114`) are public, documented, `deny_unknown_fields` types that `validate_request:311-322,344-352`
rejects whenever non-empty ("outside the accepted minimal subset"). The schema advertises
entity/relation-kind filtering and projection; neither exists.

## DP-124 · CONFLICT · major · P25 · reachability: production (this register)
**NEW** · raised p3
**This register contaminates its own detectors, and did so materially.**
Any detector of the form `rg <identifier> .` now also matches the register, because the register
quotes the identifier. Verified: `rust_protobuf_matches_the_shared_wire_fixture` occurs in exactly
one file repo-wide — `docs/reviews/design_principles_conformance_2026-08-23_v1.md`. A naive
existence check therefore reported DP-057 as fixed when the test does not exist; the correct verdict
is STANDS. `SCHEMA_VERSION_NOT_ADVANCED`, `CF-ARCH-9999` and `contract-verifier` each gain a
spurious hit the same way.
Remediation for this register: every whole-repo detector carries `--glob '!docs/reviews/**'`.

## 9. Method and limits

**Pass 3 method.** Established the change set with git (2 commits, 155 files, +20,361/−1,514),
confirmed the yardstick, pins and normative homes were unmoved, then ran three sweeps: re-verify all
99 carried findings at `d89cc90`; sweep the newly grown modules and the new wave/gate recipes; and
audit the Waves 4–7 completion claim against the plan's and roadmap's **quoted** exit criteria,
deliberately separating "not required by the plan" from "required and unmet".

**The token-fix guard, applied to myself.** Pass 3's first detector run reported DP-057 fixed. It
was not — the test name it searches for exists only in this register. Filed as DP-124; the
correction is that every whole-repo detector excludes `docs/reviews/**`. Three prior token flips
(DP-028, DP-058, DP-066) remain token.

**Detectors rewritten this pass.** DP-003/DP-006 (`committed-tree.json` moved into `.git/`),
DP-007 (authority digest moved `b3:580695b4…` → `b3:33ae78a6…`), DP-013 (retired by its fix;
replaced with a residual detector), DP-028, DP-053 (needs `--glob '!src/generated/**'`), DP-057,
DP-058, DP-061 (must now test *reachability from a gate*, not existence), DP-062 (census was 6,
actual 10+), DP-064/DP-065 (`adapter-fingerprints.json` no longer exists), DP-066, DP-071,
DP-078 (one sub-clause superseded), DP-094. Count baselines re-measured: 5 · 439 · 51 · 51/47 · 9 ·
34/49 · 65 · 8 · 68/16.

**What this pass did not do.** It did not establish a gate verdict — `just governance` cannot
complete in this environment because the sandbox blocks `sccache`, which cargo requires by design.
It did not assess `src/generated/**` as authored code, and proposes no remediation designs.

**Fairness note on the completion audit.** Daemon reachability is not counted against WP41–WP53:
the plan defines those packets' completion in component terms and W17 owns daemon RPC. The
completion findings (DP-100 – DP-108) are confined to criteria the plan or roadmap states in
end-to-end terms — WP39, WP40, WP48, M06, M07, M08 — and each quotes the criterion it measures
against. Explicitly deferred scope (Gate C, sidecar activation, performance SLOs, Ubuntu clean-host
evidence, licensing) is recorded as deferred, not as a gap.

**Reproducing this register.** `git checkout d89cc90`, then run any finding's detector. This is the
first pass whose baseline is a commit rather than a digest record.
