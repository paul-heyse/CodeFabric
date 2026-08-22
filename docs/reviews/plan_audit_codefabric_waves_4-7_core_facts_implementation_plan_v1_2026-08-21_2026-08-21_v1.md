---
artifact: plan-audit
date: 2026-08-21
version: v1
status: complete
plan_path: docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v1_2026-08-21.md
verdict: needs-revision
---

# Plan audit — CodeFabric Waves 4–7 core facts implementation plan v1

## 1. Provenance and scope

This audit reviewed the complete plan against the current repository, the eight
normative design artifacts under `docs/upfront_design/`, the governing repository
specification, and the applicable version-pinned library references routed by
`code-facts-lib-ref`. The audit is read-only except for this report.

Current evidence:

- Baseline and current `HEAD` are both
  `63c1e9625f6c852ab918467b03cf16ee69660ab7`.
- The worktree was clean except for the untracked plan being audited.
- All 13 SHA-256 values in the plan's declared-input table independently match the
  current files.
- `just ci-fast` exited 0 at the current `HEAD`: 140 root nextest cases passed, one
  was skipped, all adapter/extractor/sidecar tests passed, and contract,
  reproduction, governance, and proof-coverage checks were green.
- `just artifacts-check` and `just plan-status` exit 0, but currently report the
  completed Waves 0–3 v5 plan, not this plan.
- Running the validator with this plan explicitly fails because
  `docs/plans/state/codefabric-waves-4-7-core-facts_state.json` does not exist.
- Official crates.io metadata reports `tree-sitter-rust 0.24.2`, grammar ABI 15,
  and exported `LANGUAGE` and `NODE_TYPES`; `tree-sitter-python 0.25.0` likewise
  exports `LANGUAGE` and `NODE_TYPES` and uses grammar ABI 15.
- Official crates.io metadata confirms `rayon 1.12.0`,
  `notify-debouncer-full 0.7.0` resolving `notify 8.2.0`, and the platform feature
  surfaces assumed by the plan.
- `Cargo.toml` already declares `gix =0.86.0` with the complete feature list the
  plan assigns to WP49.

The audit did not treat the user-deferred Ubuntu run as a blocker.

## 2. Executive summary

The plan has a good architectural spine. Its immutable source-image boundary,
application-owned provider DTOs, generation fencing, generated schema and registry
authority, canonical reconciliation cutover, clean-rebuild equivalence, and
gix-as-acceleration-only posture are appropriate for the target system. The wave
segmentation is also substantially faithful to RM Waves 4–7.

The plan is not ready to execute. Three blockers must be closed first:

1. the plan cannot be adopted and proven through the repository's current active-plan
   recipes;
2. WP35 does not define an executable Cargo compiler-wrapper topology and is
   inconsistent with the released Protobuf direction and current extractor CLI; and
3. WP35 permits real compiler execution without closing the mandatory trust-profile
   behavior in GEN AC-G-35.

Seven major corrections are also required. Most importantly, design deviations must
be accepted in their normative owners rather than deferred as non-blocking issue
filings; provider dependency decisions must be exact; Tree-sitter grammar catalogs
should be derived from library metadata instead of manually duplicated; parser
cancellation and resource limits must use the libraries' native control surfaces; the
golden corpus needs an explicit candidate/release lifecycle; packet completion edges
must be made honest; and the coarse update-wave vocabulary needs a real migration and
zero-state disposition.

These are revision-sized corrections, not a reason to discard the overall design.

## 3. Readiness verdict

**Verdict: `needs-revision`.**

Do not start WP27 from this v1 plan. Produce a v2 plan after integrating the design
corrections and closing F-001 through F-010. F-011 and F-012 should be folded into the
same revision because they are low-cost factual/proof cleanups.

The plan does not require a wholesale redesign: its data, ownership, identity,
publication, and equivalence architecture should be retained. The required redesign is
localized to provider metadata generation, compiler execution/control, and the
plan-to-design/proof boundary.

## 4. Finding index

| ID | Severity | Category | Scope | Summary |
|---|---|---|---|---|
| F-001 | blocker | proof | plan adoption; final gates | Active-plan recipes and state do not validate this plan. |
| F-002 | blocker | design | WP35; AC-G-31; rustc extractor | The compiler-wrapper/process/protocol topology is not executable as specified. |
| F-003 | blocker | operations | WP35; AC-G-35 | Real compiler execution lacks a conforming trust-profile boundary. |
| F-004 | major | doctrine | D-23–D-39; SI-01–SI-14; A-06 | Normative design changes are treated as non-blocking plan decisions. |
| F-005 | major | library | WP27/WP29/WP31/WP35 | Provider dependency decisions are incomplete or intentionally moving. |
| F-006 | major | design | D-39; WP28/WP30 | Raw-kind authority manually duplicates metadata available from pinned libraries. |
| F-007 | major | operations | WP29–WP31 | Parser cancellation, memory, and work budgets are metrics rather than enforceable contracts. |
| F-008 | major | legacy | WP34/WP40; AC-G-78 | The partial Gate-B corpus is ambiguously versioned as the released immutable v1 corpus. |
| F-009 | major | sequence | WP34–WP38; WP45/WP46 | Three parallelism claims contradict actual completion or shared-write dependencies. |
| F-010 | major | legacy | WP42/WP45 | State-vocabulary replacement has no migration, recovery, or zero-state proof. |
| F-011 | minor | factuality | LD-25; WP49 | The alleged incomplete gix feature profile is already complete. |
| F-012 | minor | proof | packet-local gates; final matrix | `just mutants-file on ...` is not an executable command. |

## 5. Findings

### F-001 — This plan is not wired into the active-plan proof contract

**Severity:** blocker  
**Category:** proof  
**Scope:** frontmatter `state_path`; §14; repository active-plan tooling  
**Finding:** The absence of execution state is expected while a plan is merely being
authored, but the plan contains no adoption step that makes itself the active plan.
`tooling/ci/artifact_contracts.py` hard-codes the Waves 0–3 v5 plan as
`DEFAULT_PLAN`; `just artifacts-check` also runs the Wave-0-specific reconciliation
test and invokes the validator without `--plan`; `just plan-status` likewise invokes
the default. Consequently, both final-gate recipes are green while validating the
predecessor. Explicit validation of this plan fails on its unresolved `state_path`.
This permits execution to proceed without ever proving the plan or its packet ledger.

**Required resolution:** Add a pre-execution adoption step owned by the plan. Create
the schema-2 state with exactly WP27–WP53, M05–M08, and DB07–DB08 (plus any batch added
by F-010); replace the hard-coded active-plan default with a single explicit,
reviewable active-plan pointer or parameterized just recipe; make the artifact tests
plan-generic; and prove both the default recipes and explicit `--plan` invocation
resolve this plan. State initialization is administrative adoption, not WP27 product
completion.

**Revalidation:**

```bash
just artifacts-check && just plan-status && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py artifacts-check --plan docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && env -u VIRTUAL_ENV -u UV_PROJECT_ENVIRONMENT PYTHONPATH=. uv run --frozen --project codefabric-cpg-mcp python tooling/ci/artifact_contracts.py plan-status --plan docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md
```

### F-002 — WP35 has no coherent Cargo wrapper and protocol topology

**Severity:** blocker  
**Category:** design  
**Scope:** D-27; L-08; WP35; GEN AC-G-31; `rustc_extractor.proto`  
**Finding:** GEN AC-G-31 says Cargo launches a compiler wrapper and requires probe and
version invocations to pass through. The Rust MIR reference §6 specifies Cargo's
native contract: `RUSTC_WORKSPACE_WRAPPER` receives the real rustc path followed by
all rustc arguments. Current `rustc-extractor/src/main.rs` instead accepts only
`--identity` and `--serve <endpoint>`. WP35 says to replace `--serve` with a tonic
service, but does not define who launches Cargo, who listens, who connects, how the
real rustc path and arguments reach `rustc_driver`, or how per-invocation wrappers
coordinate with a long-lived service.

The released RPC direction does not resolve the ambiguity. `Extract` accepts daemon
commands and returns extraction events, while `CompilationBegin` contains only a
normalized invocation digest—not the invocation needed to execute rustc. There is no
wrapper-mode argv contract. The extractor package also has no generated Protobuf
bindings or transport/Arrow dependencies, and WP35 does not assign their generation
to the existing single-FDS derivation unit. Finally, the committed root Cargo config
uses `sccache` as `rustc-wrapper`; Cargo nests workspace and general wrappers. The
plan does not prove that a cache hit cannot suppress the extraction callback or state
how the analysis Cargo invocation composes with or disables sccache.

**Required resolution:** Correct GEN AC-G-31 and the released RPC contract before
WP35. Specify one complete process diagram and executable argument contract. The
preferred Wave-5 shape is a daemon-launched `cargo check` using
`RUSTC_WORKSPACE_WRAPPER` for first-party targets, an extractor wrapper mode that
receives and preserves the real-rustc argv, forwards non-analysis probes exactly,
runs the `rustc_public` callback, preserves compiler exit behavior, and exchanges
events/credits with one clearly identified daemon-side endpoint. Revise the RPC
direction/messages if that topology requires it. Explicitly decide sccache composition
so extraction cannot be skipped. Extend the one committed descriptor-set derivation
unit to generate the extractor's Rust bindings, and pin the extractor domain's exact
matching prost/tonic/Arrow dependencies. Add a real Cargo workspace-wrapper fixture,
not only a direct `rustc_link` callback test.

**Revalidation:**

```bash
bash -lc "rg -n 'RUSTC_WORKSPACE_WRAPPER|real rustc|probe/version|probe.*pass' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md && rg -n 'sccache.*(disable|bypass|cache hit|composition)|cache hit.*extract' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'single.*descriptor|compile_fds|rustc-extractor/src/generated|wp35_wrapper_topology_acceptance' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-003 — WP35 does not enforce GEN AC-G-35's default trust model

**Severity:** blocker  
**Category:** operations  
**Scope:** WP35; GEN AC-G-35; RM §27.4; Rust MIR reference §53  
**Finding:** A real Cargo check can execute repository build scripts and proc macros.
GEN AC-G-35 therefore makes `UNTRUSTED_SANDBOXED` the default and permits compiler
execution without the full platform sandbox only under an explicit `TRUSTED_LOCAL`
grant. WP35 promises private offline Cargo/target/temp directories, but does not cite
AC-G-35, require a trust profile, remove credentials, pin a sandbox-profile digest,
or fail closed when a conforming sandbox is unavailable. “Sandbox paths verified
private and offline” is not equivalent to the required network, filesystem,
environment, descriptor, quota, child-process, and platform controls.

RM §27.4 schedules general provider sandboxing for Waves 9–11 while RM Wave 5 still
requires one real compiler slice. The plan must resolve that staging explicitly; it
cannot silently run the default untrusted profile unsandboxed.

**Required resolution:** Bound Gate B to the repository-owned golden fixture under an
explicit `TRUSTED_LOCAL` grant, while still removing credentials and network by
default and pinning the trust/sandbox-profile digest in the provider run. Until the
full W10 sandbox exists, arbitrary or ungranted repository compiler jobs must fail
closed with compiler-semantic capability unavailable while source/syntax remains
available. Add negative tests for default `UNTRUSTED_SANDBOXED` on platforms where
the conforming sandbox is absent, missing grants, attempted credential/network/home/
workspace access, and trust-profile changes invalidating context. Alternatively,
move a conforming sandbox packet before WP35.

**Revalidation:**

```bash
bash -lc "rg -n 'TRUSTED_LOCAL|UNTRUSTED_SANDBOXED|sandbox_profile_digest|trust profile' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'ungranted|fail.*closed|capability.*unavailable|wp35_.*(trust|sandbox)' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-004 — The plan improperly outranks unresolved normative design

**Severity:** major  
**Category:** doctrine  
**Scope:** §2.1; D-23–D-39; §6.2; A-06; RM §28  
**Finding:** The plan identifies real design defects, but §6.2 says none blocks a
packet and A-06 says each plan decision stands unless its owner later answers
differently. RM §28 says the opposite: a detailed plan must not introduce a new
high-level design decision; ambiguity returns to the owning specification. The plan's
precedence also places its decisions immediately below normative owners, so it cannot
legitimately override a contradictory owner while leaving that owner unchanged.

The problem is material rather than editorial. D-25 changes FAB's canonical feature
map; D-26 contradicts LIFE's “new crate” wording; D-28 creates a new cross-table
canonicalization rule; D-29 changes the public freshness vocabulary; D-31 and D-32
change lifecycle behavior; D-33 changes DTO/path contracts; D-35/D-36 add provider
pins; and D-39 adds a governed contract family/path. The plan also paraphrases RM §28
as permitting an integrated Waves 4–7 plan, while its explicit integrated-plan
exception is limited to Waves 0–3. User direction can authorize a Waves 4–7 program
plan, but that must be recorded as a deliberate exception, not attributed to text
that does not say it.

**Required resolution:** Before accepting v2, integrate every behavior- or
authority-changing SI into its normative owner and recalculate the declared-input
digests. Distinguish genuinely editorial filings from design-blocking corrections.
Replace A-06 with the rule that normative corrections must be accepted before their
consumer packet begins. Correct RM's program-plan language or explicitly record the
user-authorized deviation while preserving per-wave milestones and context loading.
Do not use the implementation plan as the permanent home of D-25, D-26, D-28,
D-29, D-31–D-36, or D-39.

**Revalidation:**

```bash
bash -lc "! rg -n 'none blocks a packet|plan decision that stands' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'fact-generation' docs/upfront_design/present_state_cpg_data_fabric_specification_rust_arrow_datafusion_deltalake_v1.3.md && rg -n 'provider-raw-kinds|provider raw-kind' docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md && rg -n 'tree-sitter-rust' docs/upfront_design/present_state_cpg_fact_generation_specification_python_rust_v1.3.md docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md && rg -n 'GitCandidateDelta|inclusion-policy fingerprint|attributes fingerprint' docs/upfront_design/codefabric_continuous_cpg_update_lifecycle_management_specification_v1.3.md"
```

### F-005 — The provider library decision set is not closed

**Severity:** major  
**Category:** library  
**Scope:** D-36; LD-22/LD-23; WP27/WP29/WP31/WP35  
**Finding:** Several load-bearing dependencies are left to execution-time discovery.
D-36 tells WP27 to run the moving `cargo search` result and select the newest
compatible `tree-sitter-rust`. Official metadata already provides an exact current
candidate: 0.24.2, grammar ABI 15, compatible with core 0.26.12 and exporting both
`LANGUAGE` and `NODE_TYPES`. Deferring the selection makes the plan and bundle
identity non-reproducible.

LD-23 says to add four Ruff crates “plus siblings ... if required,” although the
pinned Ruff reference §1.0 gives the exact coherent Wave-4 set:
`ruff_python_parser`, `ruff_python_ast`, `ruff_python_trivia`,
`ruff_python_index`, `ruff_source_file`, and `ruff_text_size`, all `=0.0.7`.
WP31 needs the latter two crates for line/coordinate and trivia/index behavior.
The same reference §§2.1–2.2 requires retaining one complete `Parsed` value and
building trivia/index structures once per source revision, but the plan does not make
that reuse an invariant.

GEN AC-G-32 requires dedicated bounded Rayon pools, yet Rayon is absent from the
current graph and from every LD/change surface. Official metadata identifies current
`rayon 1.12.0` (MSRV 1.80). WP35 likewise introduces transport and Arrow IPC in the
separate extractor package without an exact dependency/family decision.

**Required resolution:** Resolve and record exact pins before plan acceptance:
`tree-sitter-rust =0.24.2` after the already-successful ABI-15 probe; all six Ruff
analysis crates `=0.0.7`; and `rayon =1.12.0` behind the intended activation
boundary. State exact feature choices. Add a Rayon LD and stable-graph assertions.
Make one `Parsed` + `TriviaRanges` + `Indexer` + `LineIndex` construction per source
revision the WP31 design. Pin extractor prost/tonic/Arrow families to the same
repository versions and generate their bindings from the shared FDS as required by
F-002. Remove moving “newest,” placeholder, and “if required” decisions.

**Revalidation:**

```bash
bash -lc "rg -n 'tree-sitter-rust.*=0\.24\.2' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && for crate in ruff_python_parser ruff_python_ast ruff_python_trivia ruff_python_index ruff_source_file ruff_text_size; do rg -n \"$crate.*=0\.0\.7\" docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md; done && rg -n 'rayon.*=1\.12\.0' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && ! rg -n 'D-36 choice|newest grammar|if required' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-006 — Raw-kind catalogs should be derived from the pinned providers

**Severity:** major  
**Category:** design  
**Scope:** D-39; WP28/WP30/WP31; ONT AC-G-70  
**Finding:** ONT AC-G-70 correctly separates provider-native raw kinds from canonical
ontology codes, but the plan proposes manually authoring three YAML raw-kind
registries. For Tree-sitter this duplicates exact metadata already exposed by the
pinned grammar crates and runtime. Both selected grammar crates export `NODE_TYPES`;
`Language` exposes ABI, node-kind, named/visible, field, supertype/subtype, and
parse-state inventories. The Tree-sitter reference §§22–23 explicitly recommends
generating typed catalogs and upgrade diffs from these surfaces. A manual list creates
the exact grammar-upgrade churn and forgotten-kind risk the plan is trying to avoid.

The best model is two-layered: provider-native inventory is mechanically derived;
the mapping from raw provider kinds to CodeFabric normalized kinds is the small,
owner-reviewed authority. Ruff enums should be consumed through generated exhaustive
matches so a provider upgrade fails compilation when a variant is unmapped, rather
than relying on corpus discovery or a permissive string fallback.

**Required resolution:** Extend the existing typed derivation-unit model with a
provider-grammar/raw-catalog derivation. For each Tree-sitter grammar, generate the
raw node/field catalog from `NODE_TYPES` plus runtime `Language` introspection, bind
it to package version, runtime ABI, node-types digest, and query-bundle digest, and
generate hot-path ID lookup tables. Keep only the normalized-kind mapping authored.
Generate Ruff conversion code from its mapping registry with exhaustive enum matches.
At startup and in CI, compare the full library inventory to generated catalogs,
compile all queries, and emit a reviewable grammar diff on upgrades. Record generated
outputs and provenance through the existing artifact catalog; do not add a second
generator authority.

**Revalidation:**

```bash
bash -lc "rg -n 'NODE_TYPES|node_kind_count|field_count|grammar fingerprint' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'raw-kind.*(derivation|generated)|provider.*catalog.*derivation' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md docs/upfront_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md && ! rg -n 'Author the .*raw-kind registry|Author the .*raw-kind registries|manually author.*raw-kind' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-007 — Parser resource and cancellation behavior is not enforceable

**Severity:** major  
**Category:** operations  
**Scope:** WP29–WP31; GEN AC-G-32/AC-G-43; Tree-sitter §§18/35/36/42; Ruff §4.15  
**Finding:** WP29 promises bounded pools and a two-second cancellation
acknowledgement, while WP30/WP31 mostly require counters. The plan does not require
Tree-sitter's native `ParseOptions::progress_callback`, parser reset after cancelled
parse, query progress callbacks, match/range/depth limits, bounded live tree
revisions, or worker-owned parser/cursor pools. This matters because Tree-sitter uses
a native C allocator that can abort on allocation failure, and a cancelled query may
produce a partial capture set that must not be published as complete.

Ruff has no incremental parse API and its public parser is whole-source. The pinned
reference requires file caps, deadlines, depth-disciplined visitors, fuzzing of custom
extraction, and parse errors as data. WP31 has no explicit whole-file work/output/depth
budget or policy for acknowledging cancellation while discarding an in-flight
non-cancellable parse. GEN AC-G-43's 16 MiB ordinary and 64 MiB hard file limits are
necessary but are not complete parser work or output budgets.

**Required resolution:** Add a typed provider resource-profile authority referenced by
`ProviderJobSpec`, provider bundle, and metrics. Include file bytes, parse/query wall
and work budgets, output rows/bytes, traversal depth/nodes, parser pool size, retained
tree revisions, and retry policy. WP30 must use Tree-sitter progress callbacks,
`parser.reset()` after cancellation, query match/range/depth controls, and atomic
revision commit. WP31 must parse once, reuse its complete `Parsed` snapshot, bound
visitors and output, and acknowledge/drop stale work explicitly when the underlying
whole-file parse cannot be interrupted. Add adversarial repetitive/deep/large sources
and cancellation tests; no partial result may publish as complete.

**Revalidation:**

```bash
bash -lc "rg -n 'ParseOptions::progress_callback|progress_callback.*ParseOptions|parser\.reset' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'QueryCursorOptions|match_limit|range/depth|retained tree|parser pool' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && rg -n 'Ruff.*(whole-file|whole source|non-incremental).*(deadline|budget|cancel)|wp3[01]_.*resource' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-008 — The Gate-B corpus slice has no coherent release lifecycle

**Severity:** major  
**Category:** legacy  
**Scope:** WP34/WP40; SUITE AC-G-78  
**Finding:** AC-G-78 defines `tests/golden/codefabric-golden-v1/` as the normative
corpus with all listed Python, Rust, FFI, scenario, expected-output, query, RPC, MCP,
diagnostic, and comparator coverage. It states that a released corpus version is
immutable. WP34 creates that same v1 root with `corpus_version: "1.0"`, cites all
AC-G-78 subsections, but intentionally implements only the Wave-5 slice. WP40 then
adds expected outputs after WP34, and later waves must add most of AC-G-78's required
coverage. The plan never says whether the WP34 corpus is a draft candidate, a Gate-B
coverage profile, or the released v1 corpus.

This ambiguity creates two bad outcomes: either an incomplete corpus is incorrectly
released as AC-G-78 v1, or subsequent packets mutate an allegedly immutable released
oracle.

**Required resolution:** Give the corpus a machine-readable lifecycle. The preferred
approach is one canonical `codefabric-golden-v1` candidate with immutable exact source
inputs and append-only, named coverage profiles; `gate-b-v1` is accepted at WP40, but
the corpus itself remains unreleased until the complete AC-G-78 profile is accepted at
its roadmap-owned release wave. Expected-output candidates remain generator-write
protected and become immutable only when owner-accepted for their profile. State
which directories/files may be added at each later wave and prove that an accepted
profile cannot be rewritten. Do not claim “all AC-G-78 subsections” for the Wave-5
slice.

**Revalidation:**

```bash
bash -lc "rg -n 'corpus.*(candidate|draft)|Gate-B.*coverage profile|gate-b-v1|not.*released.*AC-G-78|AC-G-78.*release.*Wave 19' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md && ! sed -n '/^### WP34 /,/^### WP35 /p' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md | rg -n 'AC-G-78.*all subsections'"
```

### F-009 — Three parallelism claims are not dependency-closed

**Severity:** major  
**Category:** sequence  
**Scope:** WP34/WP35; WP37/WP38; WP45/WP46; §15  
**Finding:** WP35's acceptance uses the WP34 golden fixture crate but WP35 does not
depend on WP34; the plan treats a partial output from an incomplete packet as an
accepted contract. WP38's behavioral oracle consumes WP37's projection but WP38 does
not depend on WP37. In both cases the plan admits that completion is gated on the
other packet while still drawing parallel completion branches. Packet dependencies,
not prose caveats, must make completion closure explicit.

The plan also permits WP46 to overlap WP45 on “disjoint modules,” but both known-touch
sets include `src/coordinator.rs`. WP45 changes admission/freshness coordination while
WP46 changes flush scheduling in the same coordinator. The declared write sets are not
disjoint, so parallel implementation risks semantic and merge conflicts.

**Required resolution:** Serialize WP34 before WP35 and WP37 before WP38, or split the
fixture/projection subdeliverables into independently proven packets with stable
interfaces. Serialize WP45 and WP46 unless the plan first extracts and assigns truly
disjoint coordinator interfaces/files. Update the dependency lists, sequence diagram,
milestone dependencies, and state IDs consistently.

**Revalidation:**

```bash
bash -lc "sed -n '/^### WP35 /,/^### WP36 /p' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md | sed -n '/^#### Dependencies$/,/^#### Target Invariants$/p' | rg -n 'WP34' && sed -n '/^### WP38 /,/^### WP39 /p' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md | sed -n '/^#### Dependencies$/,/^#### Target Invariants$/p' | rg -n 'WP37' && ! rg -n '\{WP34 \|\| WP35\}|\{WP37 \|\| WP38\}|WP46 may overlap WP45' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-010 — Update-wave vocabulary replacement has no migration or zero state

**Severity:** major  
**Category:** legacy  
**Scope:** WP42/WP45; D-29; state-machine registry  
**Finding:** The current generated `UpdateWaveState` has seven values. LIFE §21 has
17 values, four of which share names with current states while three current coarse
states (`RUNNING`, `PUBLISHING`, `COMPLETE`) do not exist in the normative machine.
WP42 says to append the 17-state vocabulary, deprecate coarse states, and drive
persisted `update_wave` rows through it, but its Legacy Disposition says “None.” It
does not specify which existing codes remain canonical, how old durable rows are
interpreted, whether pre-WP42 rows must be absent, or when emit/read support for
coarse states reaches zero. Blindly appending a second code for an already-existing
state would also violate registry uniqueness.

Adding `AWAITING_CURRENT` to `FreshnessState` is an ordinary append, but the owning
ONT/LIFE disagreement must be resolved under F-004 before WP45 consumes it.

**Required resolution:** Define an explicit registry migration: retain existing codes
for exact matching names; append only genuinely new states; deprecate non-normative
coarse states with replacement/migration semantics; prove whether any persisted rows
exist; migrate or reject them before the new scheduler activates; and prohibit new
emission of deprecated states. Add a decommission batch (recommended DB09) with
positive 17-state model evidence, negative deprecated-emission/read ambiguity zero
state, and a reintroduction guard. Update state schema/plan IDs under F-001.

**Revalidation:**

```bash
bash -lc "sed -n '/^### WP42 /,/^### WP43 /p' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md | rg -n 'migration|persisted rows|deprecated.*(emission|zero state)|DB09' && rg -n '^### DB09|DB09.*(state|vocabulary|deprecated)' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

### F-011 — WP49's gix feature-completion premise is stale

**Severity:** minor  
**Category:** factuality  
**Scope:** LD-25; A-05; WP49  
**Finding:** `Cargo.toml` already declares exact `gix =0.86.0`,
`default-features = false`, and all features WP49 says it must complete:
`sha1`, `sha256`, `index`, `status`, `attributes`, `excludes`, `dirwalk`,
`blob-diff`, `interrupt`, `parallel`, `auto-chain-error`, and `tracing` (plus the
already-required `sha256`). `cargo tree -p gix --edges features` resolves now.
Treating this as an open manifest change creates unnecessary lock/feature churn and
weakens known-touch accuracy.

**Required resolution:** Restate LD-25/WP49 as retain-and-verify. Remove Cargo.toml
from known touch unless a concrete API probe finds a missing feature. Keep the
resolved graph, handle model, and capability probes as acceptance evidence.

**Revalidation:**

```bash
bash -lc "cargo tree -p gix --edges features >/dev/null && ! sed -n '/^### WP49 /,/^### WP50 /p' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md | rg -n 'Cargo\.toml.*feature list completion|complete the Appendix E feature manifest'"
```

### F-012 — Mutation-test gates are not executable as written

**Severity:** minor  
**Category:** proof  
**Scope:** packet-local gates; §14  
**Finding:** Multiple packet gates say ``just mutants-file` on ...``. The repository
recipe requires a concrete path argument, so these are risk labels rather than
commands. The final matrix correctly calls mutation evidence risk-triggered, but the
packet sections present it as a packet-local gate. A future executor cannot determine
from the plan whether mutation is mandatory, what file is targeted, or what success
condition closes the packet.

**Required resolution:** For each mandatory mutation obligation, have the owning
packet add a named, argument-free just recipe or identify the exact post-discovery
source path in the packet's state-independent command contract. Otherwise label it a
risk trigger and remove it from packet completion gates. Preserve targeted mutation;
do not replace it with blanket repository mutation testing.

**Revalidation:**

```bash
bash -lc "! rg -n 'just mutants-file` on|`just mutants-file` on' docs/plans/codefabric_waves_4-7_core_facts_implementation_plan_v2_2026-08-21.md"
```

## 6. Target-design assessment

### 6.1 Strong decisions to preserve

- **One application-owned provider boundary.** Tree-sitter, Ruff, rustc, notify, and
  gix types are kept behind adapters and DTOs. This is the correct response to
  version-sensitive and attached-lifetime APIs.
- **Immutable source truth and generation fencing.** Source bytes and line indexes
  remain authoritative; provider and gix results are evidence tied to exact source,
  context, and baseline generations.
- **One generated contract authority.** TableSpecs, state machines, registries,
  schemas, and protocol outputs flow through the existing typed Contract IR and
  derivation units. The plan generally extends rather than forks this machinery.
- **Explicit degradation.** Unsupported inputs, provider failure, watcher loss, and
  gix failure become capability/trust states instead of retaining stale facts.
- **Equivalence as correctness.** Incremental behavior, overlay rebase, Git
  acceleration, and generic fallback converge under canonical comparison. This is a
  much stronger oracle than implementation-shaped assertions.
- **Gix remains an accelerator.** Current bytes and CodeFabric digests remain
  authoritative; candidate sets are fenced and generic fallback remains mandatory.
- **Wave-scoped breadth.** The plan does not prematurely adopt Pyrefly semantics,
  broad Rust semantics, petgraph, public FastMCP serving, or performance SLO gates.

### 6.2 Design corrections required

The clean-sheet architecture would not manually transcribe grammar inventories, run a
compiler through an ambiguous long-lived service, or let an implementation plan become
the permanent owner of contract behavior. F-002, F-004, and F-006 therefore identify
real design corrections, not stylistic preferences.

The preferred corrected control flow is:

```text
accepted ProviderJobSpec + trust/resource profile
  -> daemon builds immutable ProviderWorkspaceView
  -> daemon launches cargo check with RUSTC_WORKSPACE_WRAPPER
  -> per-compilation wrapper preserves rustc argv/probes and runs rustc_public
  -> owned Arrow observation chunks cross one bounded, credited endpoint
  -> daemon verifies terminal manifest and generation
  -> staged observations enter the sole reconciliation path
```

For parsers, the preferred metadata flow is:

```text
exact grammar crate + runtime Language metadata
  -> typed derivation unit
  -> generated raw node/field catalog + grammar fingerprint + hot-path IDs
  + owner-authored raw-to-normalized mapping
  -> exhaustive adapter bindings and upgrade-diff oracle
```

These flows reduce handwritten authority and ongoing upgrade maintenance while making
failure and provenance visible.

## 7. Library capability assessment

| Decision | Disposition | Audit assessment |
|---|---|---|
| LD-21 Tree-sitter core/Python | retain with correction | Exact versions and ABI strategy are sound. Add `NODE_TYPES`/`Language`-derived catalogs, `Parser::set_language` as authoritative ABI check, progress callbacks, match/range/depth limits, and worker-owned pools. |
| LD-22 Tree-sitter Rust | revise | Adoption is sound; version must be fixed before execution. Current official candidate is `=0.24.2`, ABI 15, with `LANGUAGE` and `NODE_TYPES`. |
| LD-23 Ruff component crates | revise | Use the exact six-crate analysis set from the pinned reference. Retain one `Parsed`, then construct trivia, index, and line mapping once per revision. Do not add formatter/codegen/semantic crates in these waves. |
| Missing Rayon decision | add | GEN AC-G-32's dedicated CPU pools require a direct dependency and exact activation decision. Current official release is `=1.12.0`. |
| Inherited rustc/rustc_public | revise integration | The dated nightly isolation is correct. Use Cargo's wrapper contract and the existing single-FDS generation model; do not invent a parallel transport generator or rely on `--serve` alone. |
| LD-24 notify/debouncer | retain | Exact versions are coherent: debouncer 0.7.0 resolves notify 8.2.0. The facade, overflow-to-rescan behavior, PollWatcher fallback, final-state assertions, and no raw-event leakage are well grounded. |
| LD-25 gix | retain, correct premise | Version/features and detached-DTO boundary are already present. WP49 should prove APIs, trust, and handle ownership—not churn the manifest absent evidence. |
| LD-26 no new graph/query engine | retain | DataFusion and existing canonical tables suffice for the Wave-5 projection/query slice; petgraph deferral is appropriate. |

No additional general-purpose parsing, query, graph, or serialization dependency is
justified by this scope.

## 8. Work-packet and impact assessment

- **WP27:** correctly owns the root feature boundary and graph proof, but must close
  exact grammar/Ruff/Rayon decisions and accepted design pins first.
- **WP28:** correctly extends Contract IR before consumers. Replace manually authored
  raw catalogs with one typed provider-metadata derivation and keep only semantic
  normalization mappings authored.
- **WP29:** accepted-handle, permits, supersession, generation fences, and no-storage
  provider boundary are strong. Add typed resource profiles and direct Rayon
  dependency ownership.
- **WP30/WP31:** authority split and correspondence rules are faithful to GEN. Add
  library-native cancellation/resource controls and the one-parse/one-index reuse
  invariant.
- **WP32/WP33:** canonical encoders, dual-source parse diagnostics, capability
  outcomes, and Wave-4 integration are well shaped once upstream corrections land.
- **WP34:** fixture mechanics, exact bytes, deterministic barriers, independent oracle
  review, and generator write protection are strong; release/profile staging is not.
- **WP35:** blocks Wave 5 until topology, generated transport ownership, cache
  composition, and trust profile are corrected.
- **WP36/WP37:** production reconciliation cutover and the small registered
  `SYNTAX_TREE_V1` projection are proportionate to Gate B.
- **WP38/WP39:** three-form compilation and accepted-handle serving are appropriately
  narrow, but WP38 completion must depend on WP37.
- **WP40:** Gate-B integration, non-empty overlay provider, pin preservation, and
  bounded state-digest staging provide meaningful interaction proof.
- **WP41:** one of the strongest packets. It uses the debouncer as an event-coalescing
  hint layer without treating events as authority.
- **WP42:** comprehensive scheduler behavior, but the generated state-machine cutover
  must have migration and zero-state ownership.
- **WP43–WP48:** invalidation, fast publication, freshness, rebase, recovery, and the
  full comparator form a coherent Wave-6 closure. Serialize the shared WP45/WP46
  coordinator work.
- **WP49–WP53:** the five gix phases track LIFE accurately, preserve generic fallback,
  and include strong parity/equivalence/failure evidence. Correct only the stale
  feature-profile premise in WP49.

The broad change-surface discovery is generally sufficient. The main missing surfaces
are the shared Proto derivation unit and extractor generated bindings/dependencies,
Rayon's manifest/feature graph, active-plan tooling/state, and persisted state-code
migration.

## 9. Legacy, transition, and decommission assessment

DB07 and DB08 are well bounded and have positive target plus negative legacy evidence.
L-08 is incomplete until F-002 replaces the no-op with the actual wrapper topology,
not merely a service implementation. The state-machine transition needs a new batch
under F-010. The golden corpus needs profile-level immutability rather than treating a
partial candidate as a released oracle.

The plan otherwise correctly avoids dual reconciliation paths, placeholder serving
views, alternate schema authorities, and gix-only correctness.

## 10. Proof and validation assessment

The layered proof strategy is strong: focused named oracles, packet-local gates,
per-wave integration recipes, Gate B, clean-rebuild differential comparison, Git CLI
parity, fault hooks, and negative governance checks cover distinct risks. The current
green `ci-fast` baseline is meaningful and no pre-existing failure is masking the
findings.

Required proof improvements are narrowly identifiable:

- active-plan proof must validate this plan (F-001);
- real Cargo-wrapper, probe, status, cache, and transport behavior must replace direct
  callback plausibility (F-002);
- trust-profile denial must be executable, not inferred from private temp paths
  (F-003);
- generated provider catalogs need independent full-inventory and upgrade-diff checks
  (F-006);
- parser budget/cancellation tests must distinguish complete, cancelled, stale, and
  resource-rejected outputs (F-007);
- corpus acceptance must distinguish candidate, accepted coverage profile, and
  released corpus (F-008);
- mutation gates must resolve to actual commands (F-012).

## 11. Doctrine assessment

The plan substantially advances model authority, deterministic identity, bounded
concurrency, explicit uncertainty, regeneration, and differential proof. It avoids
many common anti-patterns: provider-native types do not become the application API;
filesystem events and gix candidates do not become facts; partial provider output
does not activate; and broad future capabilities are not claimed early.

The main doctrine regressions are exactly the findings above:

- A-06 allows plan prose to become shadow design authority.
- D-39 creates manual duplication where the libraries expose executable metadata.
- WP35 leaves a dangerous execution boundary implicit.
- WP42 introduces a second vocabulary without a bounded transition.
- the current proof recipes can report the predecessor plan healthy while this plan
  is unadopted.

Correcting those restores the plan's intended model-based character.

## 12. Top required changes

1. Correct and accept the normative design changes; regenerate declared-input digests
   in v2.
2. Define the Cargo/rustc wrapper, endpoint, Protobuf direction, shared-FDS output,
   exact dependency, and sccache behavior end to end.
3. Make Gate-B compiler execution explicitly `TRUSTED_LOCAL` and fail closed for
   untrusted/ungranted work until the full sandbox wave.
4. Make provider dependencies exact, including tree-sitter-rust 0.24.2, the six Ruff
   crates, and Rayon 1.12.0.
5. Generate Tree-sitter raw catalogs from `NODE_TYPES`/`Language`, and generate
   exhaustive Ruff mappings from the reviewed normalization model.
6. Add typed parser resource profiles and native cancellation/work bounds.
7. Clarify golden-corpus candidate/profile/release immutability.
8. Correct dependency edges and serialize shared coordinator writes.
9. Add the update-wave registry migration and decommission batch.
10. Adopt the v2 plan through schema-2 state and active-plan-generic proof recipes.
11. Correct the stale gix manifest premise and make mutation commands executable.

## 13. Re-audit scope

The follow-up audit should re-run every finding command and then focus on:

- accepted design ownership and fresh declared-input digests;
- exact dependency/feature graphs for the stable root and extractor domain;
- the final Cargo-wrapper and RPC sequence, including sccache and trust behavior;
- generated provider catalog provenance and upgrade handling;
- resource/cancellation semantics for both parser families;
- corpus profile immutability and state-code migration;
- the corrected dependency graph, active-plan state, and recipe resolution.

If those checks close without introducing a second authority or weakening the
negative/equivalence oracles, the expected next verdict is `ready` or
`ready-with-corrections`.
