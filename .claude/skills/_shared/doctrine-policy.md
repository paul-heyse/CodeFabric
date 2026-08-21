# Doctrine and Project-Policy Application

The project doctrine at
`docs/library_ref/semantic_design_principles_holistic.md` is authoritative when
present. Do not reproduce all principles in every skill or artifact.

## When a full doctrine read is mandatory

Read the doctrine before designing, planning, auditing, or reviewing changes
that affect any of:

- semantic identity or domain models;
- compilation passes or artifact boundaries;
- adapter/port boundaries;
- mutation, transactions, commands, or durable state;
- runtime ownership or resource lifecycle;
- UI projection of semantic truth;
- public contracts, provenance, observability, or executable governance;
- architectural replacement or broad decommission.

For narrow tests, documentation, or isolated bug fixes, read only the relevant
project rules unless architecture may be affected.

## Required artifact behavior

- Cite relevant principles by number/name in design decisions; packet
  invariants inherit the citation through their design references.
- State `Advances`, `Maintains`, or `Risk — mitigated` for the principles a
  decision actually implicates.
- Record any temporary violation as an explicit bounded transition with an
  owner, exit invariant, and decommission packet.
- Check the anti-principles directly. A recurring anti-principle becomes a
  `rules/` governance rule with `rule-tests/` fixtures, not a per-artifact
  assertion; cite the rule id thereafter (`evidence-policy.md` §0).
- Treat silent doctrine regressions as defects.
- Mark unrelated principles `N/A`; do not produce a ceremonial 31-row table
  when most principles do not apply.

## Clean-sheet challenge

Doctrine is not a reason to preserve the current architecture. Ask:

> Would this still be the preferred design if the existing implementation did
> not exist?

If not, retain the legacy shape only when a genuine constraint justifies it:
external compatibility, persisted data, operational risk, migration economics,
or a missing target capability. Record the constraint and the exit condition.

## Material design reopening

Do not reopen an accepted design for stylistic preference. Reopen it when
current code, verified library capability, doctrine, target invariants,
migration economics, security, reliability, or performance evidence materially
invalidates the choice.
