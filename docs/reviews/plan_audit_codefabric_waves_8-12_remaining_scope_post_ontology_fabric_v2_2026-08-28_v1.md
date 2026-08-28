---
artifact: plan-audit
date: 2026-08-28
version: v1
status: complete
plan_path: docs/plans/codefabric_waves_8-12_semantic_profiles_implementation_plan_v2_2026-08-26.md
verdict: ready-with-corrections
---

# Remaining Waves 9–12 reconciliation after the ontology-compiled data-fabric cutover

## Scope and decision

This audit covers only packets that were not complete when the ontology-compiled
data-fabric successor interrupted the Waves 8–12 plan. Completed packet history is not
reopened. The immutable waves plan remains unchanged; this report and the plan's schema-v2
state `plan_deviations` are the A-3 reconciliation mechanism required by the successor
design and its WP16.

The remaining program is executable after the successor plan activates Stage 2b, but each
fact-emitting packet must consume the compiled schema/ontology surfaces that will then be
current. Provider-boundary packets whose outputs are application-owned observations rather
than canonical Arrow/Delta rows remain valid without a packet rewrite. No remaining packet
is superseded: the successor changes the common data-fabric seam, not the semantic facts
those packets own.

## Findings

### F-001 — Canonical fact emitters must use the post-cutover generated seam

**Severity:** major
**Category:** impact
**Scope:** remaining WP10–WP14, WP15, WP18–WP23, WP25–WP35, WP37
**Finding:** These packets still own valid semantic behavior, but their execution must use
the single generated row shapes, per-domain ID extensions/literals, compiled ontology codes,
and Stage-2b publication rules established by the successor plan. Reintroducing a local row
shape, generic ID type, literal governed code, or direct authority parser would recreate the
retired fabric.
**Required resolution:** Apply the per-packet `needs-revision` constraints in the matrix
below as execution-state deviations before each affected packet begins.
**Revalidation:** `just ontology-self-description-check && just id-domain-extension-check && just model-repro-check`
**Integration disposition:** `deferred` — closure belongs to each packet's future proving
commit because this audit may not edit the immutable plan or pre-implement its semantic work.

### F-002 — Stage-2b lifecycle and ontology gates become inherited entry checks

**Severity:** major
**Category:** sequence
**Scope:** all remaining fact-emitting packets and milestones M02–M05
**Finding:** Once Stage 2b activates, later publications must pin the same twenty ontology
dimension versions while authority inputs are unchanged, and must pass compiled relational
closure before pointer advancement. The waves plan's original packet order remains valid,
but its entry assumptions predate this activation barrier.
**Required resolution:** Require the active Stage-2b pointer, unchanged-dimension-version
check, and ontology relational closure at the entry of every remaining integration group.
**Revalidation:** `just ontology-stage2b-activation-check && just ontology-relational-closure-check`
**Integration disposition:** `applied-design` — FAB §6.3, §8, §9, §§81–82/93 and the
successor activation transaction now own the common lifecycle; the matrix records the
waves-side consumption rule.

### F-003 — Provider transport and private-adapter packets do not own the canonical fabric

**Severity:** minor
**Category:** impact
**Scope:** WP09, WP38, WP16, WP17, WP24
**Finding:** These packets transport immutable source/provider observations or implement
provider-private adapters. Their application-owned DTO and protocol boundaries remain
correct and do not declare canonical table, result, or ID-domain shapes.
**Required resolution:** Preserve their scope and feed accepted observations into the
current generated ingest seam; do not add fabric work to them.
**Revalidation:** `just provider-protocol-check && just provider-observation-schema-check`
**Integration disposition:** `rejected` — a packet rewrite would mix provider isolation
with canonical-fabric ownership and would be a regression rather than a correction.

### F-004 — Integrated closure must retain the GraphOperatorPlan boundary

**Severity:** major
**Category:** design
**Scope:** WP35
**Finding:** WP35's single-owner derivation registry remains correct. Its integrated gate
must interpret the amended FAB calculation sections consistently: built-in DataFusion plans
own relational calculations, while non-relational traversal/fixed-point work uses the
application-owned `GraphOperatorPlan` derived lane, never UDTFs or custom DataFusion logical
or physical nodes without an accepted extension decision.
**Required resolution:** Add the successor plan's ontology, ID-domain, result-schema, and
GraphOperatorPlan zero-state checks to WP35's future integrated closure.
**Revalidation:** `just wave12-integration-check && just ontology-dimension-check`
**Integration disposition:** `deferred` — WP35 is not yet executable and must close this at
its proving commit against the then-current integrated substrate.

## Per-packet disposition matrix

`needs-revision` means execution-time adaptation to the new common seam; it does not change
the packet's semantic outcome or stable identifier.

| Packet | Disposition | Required execution-time adaptation |
|---|---|---|
| WP09 | unaffected | Preserve source/view/lease transport; hand accepted observations to the generated ingest seam. |
| WP38 | unaffected | Preserve the application-owned report adapter and wire DTOs; no canonical fabric ownership. |
| WP10 | needs-revision | Resolve module/symbol codes from `CompiledOntology`; emit generated domain-typed rows. |
| WP11 | needs-revision | Use generated type/entity/fact ID domains and compiled property/type authorities. |
| WP12 | needs-revision | Use generated member/relation row shapes and compiled relation/property rules. |
| WP13 | needs-revision | Use compiled relation/certainty bindings and domain-conformant plan literals. |
| WP14 | needs-revision | Run reconciliation output through compiled relational closure and preserve ontology dimension pins. |
| WP15 | needs-revision | Use the Contract-IR operational ID/timestamp map and generated context/repository/worktree domains. |
| WP16 | unaffected | Preserve extractor process/sandbox ownership; canonical lowering remains downstream. |
| WP17 | unaffected | Preserve invocation-manifest acceptance; pass only accepted DTOs to generated lowering. |
| WP18 | needs-revision | Emit definition/type facts through generated row shapes and domain extensions. |
| WP19 | needs-revision | Emit MIR body/local/block facts through generated row shapes and domains. |
| WP20 | needs-revision | Emit CFG/call facts through generated row shapes and compiled relation codes. |
| WP21 | needs-revision | Materialize compile-failure unknowns/capability gaps through compiled fact/rule contracts. |
| WP22 | needs-revision | Use generated place/access-path row shapes and typed IDs. |
| WP23 | needs-revision | Use generated ownership-state rows and compiled enum/property codes. |
| WP24 | unaffected | Keep compiler-private enrichment inside the extractor DTO boundary. |
| WP25 | needs-revision | Use generated instance/dispatch IDs, rows, and relation codes. |
| WP26 | needs-revision | Use generated macro/generated-code rows and source-span classification. |
| WP27 | needs-revision | Use generated resource/effect/property rows and compiled one-of validation. |
| WP28 | needs-revision | Pin ontology dimensions across Rust-lane publications and use domain-conformant literals. |
| WP29 | needs-revision | Lower reconciliation checks as compiled relational plans and keep provider conflicts as evidence. |
| WP30 | needs-revision | Extend, rather than duplicate, the compiled property cardinality/one-of rule family. |
| WP31 | needs-revision | Resolve capability/state codes through ontology dimensions and typed control projections. |
| WP32 | needs-revision | Use compiled unknown/fact kinds and generated identity recipes. |
| WP33 | needs-revision | Produce coverage/completeness outputs through generated result schemas and ResultChecksumV2. |
| WP34 | needs-revision | Use analysis-context/set domains and generated typed ID-list results. |
| WP37 | needs-revision | Use generated bridge/foreign identity rows and compiled relation/property authorities. |
| WP35 | needs-revision | Add Stage-2b closure and preserve the built-in-plan/GraphOperatorPlan boundary in final integration. |

Disposition census: 5 `unaffected`, 24 `needs-revision`, 0 `superseded`.

## Resumption conditions

The waves plan may resume at WP09 only after the ontology-compiled successor has one
accepted Stage-2b dossier and active pointer. Before each `needs-revision` packet begins,
its state entry records this review path and the applicable matrix row. M02–M05 inherit
`ontology-dimension-check`; WP35 additionally inherits the result-schema and
GraphOperatorPlan zero-state checks. These are packet-entry adaptations, not permission to
change the immutable plan or completed history.

## Verdict

`ready-with-corrections` — no remaining semantic outcome is invalidated, no packet is
superseded, and the exact execution-time corrections are bounded above. Resumption remains
blocked until Stage 2b is activated and the state-file deviation is present.
