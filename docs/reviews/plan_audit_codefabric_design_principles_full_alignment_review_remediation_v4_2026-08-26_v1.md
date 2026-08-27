---
artifact: plan-audit
plan_id: codefabric-design-principles-full-alignment-review-remediation
plan_version: v4
version: v1
date: 2026-08-26
status: complete
verdict: ready
plan_path: docs/plans/codefabric_design_principles_full_alignment_review_remediation_implementation_plan_v4_2026-08-26.md
design_path: docs/designs/codefabric_behavioral_conformance_corpus_design_v1_2026-08-26.md
---

# Focused plan audit — behavior-first remediation v4

## Scope and verdict

This focused re-audit covers only the v3-to-v4 execution-governance corrections discovered by
the repository's active-plan assurance gate during WP09. The accepted behavior-first design,
packet outcomes, dependencies, proof obligations, legacy dispositions, and owner checkpoint are
unchanged. Verdict: **ready**.

## Evidence reviewed

- The complete v3 and v4 plan artifacts and their targeted diff.
- `tooling/ci/plan_assurance.py` dependency and exact-oracle-definition rules.
- The Rust `query_form_projection_parity` packet oracle and the independent Python presentation
  projection check in `codefabric-cpg-mcp/tests/test_proto.py`.
- The accepted behavior-first design and the prior v3 ready audit.

## Findings and dispositions

### F-001 — Dependency prose was machine-ambiguous

The WP08 dependency clause repeated its own packet identifier while explaining that milestone
exit conditions were not dependencies. The repository parser intentionally treats every packet
identifier in that clause as an edge, so v3 appeared self-dependent. V4 removes only that
explanatory identifier. The declared dependency set remains WP04, WP05, WP07, and WP09–WP12.

Disposition: applied and ready. Revalidate with `just plan-dependency-check` and
`just artifacts-check` after activation.

### F-002 — A supplemental cross-language check reused a governed oracle name

The completed WP01 Rust oracle and a supplemental Python presentation projection test had the
same normalized oracle name. The strict oracle census correctly rejects duplicate definitions.
The implementation change renames only the Python supplemental check; the Rust governed oracle,
its semantics, and the `query-form-contract-check` coverage remain intact.

Disposition: implementation naming correction required during WP09; no plan semantic change.
Revalidate with `just query-form-contract-check` and `just oracle-substance-check`.

## Readiness conclusion

V4 is dependency-closed, preserves the audited behavior-first architecture, and restores
unambiguous machine execution without weakening any acceptance criterion. No further plan change
is required before WP09 resumes.
