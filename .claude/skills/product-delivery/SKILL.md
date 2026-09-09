---
name: product-delivery
description: Plan, implement, and hand off an observable product outcome with proportionate checks. Honor the requested scope.
---

# Product delivery

Read `AGENTS.md` and `STATUS.md`; use the selected design and current backlog.
Honor explicit user exclusions and preserve dirty/concurrent work.

1. Identify the observable outcome and its immediate consumers.
2. Inspect current code, inputs, and relevant local library references.
3. Make a small coherent change in the canonical tree and integrate promptly.
4. Run affected checks; exercise product behavior when making a product claim.
5. Fix relevant failures and update `STATUS.md` with what works, fails, and comes next.

For a plan request, write an editable Markdown backlog under `docs/plans/`: outcomes,
affected surfaces, actual dependencies, relevant validation, and material risks.
For a status request, report demonstrated behavior and evidence date, limitations,
and next action. Neither request activates another state machine.

Keep evidence attributable to command/revision/configuration. Empty selection is failure.
Reuse applicable evidence. Do not manufacture a universal pass from a scoped check.
Use short decisions or optional review for consequential uncertainty. Routine adaptations
update the current plan in place; no oracle quota, digest registry, proving-commit chain,
new suite issuance, or additional approval is needed for already authorized work.

Runtime actions leave compact outcomes; source changes use Git history. Preserve useful
checks and remove obsolete machinery with its working replacement. References: the
repository command list and `_shared/validation-policy.md` when choosing checks.
