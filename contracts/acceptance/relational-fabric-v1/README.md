# Relational-fabric independent evidence v1

This directory is WP22's separate, read-only expectation ingress. Production model,
provider, compiler, query, serving, activation, and release code must not import it or
write it. `expectations.json` contains decoded rows, accountable ownership, rationale,
limitations, causal controls, and content-bound source/API/public-contract provenance.
`acceptance-transaction.json` binds the exact schema, corpus, and comparator manifest.

The current transaction is deliberately **rejected**. It records facts that cannot be
repaired by assertion:

- this corpus was first authored after implementation consumers had begun, so WP22's
  required WP01-before-WP02 chronology has no proving commit;
- the historical source tree at `7184b86dc80adedc8a2b8d081179fa52d3dfee20` is exact,
  but that tree selected a moving root `stable` toolchain and no exact rustc/cargo
  executable bytes were captured;
- no immutable legacy comparator executable or enforcing no-network/read-only/no-write
  runtime was captured; and
- no independent numeric performance budget has been accepted.

The validators never generate or accept evidence. A valid successor must be authored
and reviewed as new content-addressed bytes; accepted files are never edited in place.
The comparator may emit length-delimited decoded rows to stdout, but must have no
filesystem write authority and legacy output remains comparison evidence only.

Focused commands (the repository's Just recipes may wrap these without changing their
semantics):

```text
python tooling/ci/relational_fabric_evidence.py independent-evidence-dag <plan>
python tooling/ci/relational_fabric_evidence.py expectation-independence
python tooling/ci/relational_fabric_evidence.py early-evidence-acceptance
python tooling/ci/relational_fabric_evidence.py comparison-engine-isolation
python tooling/ci/relational_fabric_evidence.py late-authoring-zero-state
python tooling/ci/relational_fabric_evidence.py legacy-comparator-reconstruction
```

The first two checks can pass for this candidate. The other four must fail closed until
their recorded external and chronological prerequisites genuinely exist.
