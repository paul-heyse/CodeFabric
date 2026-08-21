---
artifact: implementation-review
date: 2026-08-20
version: v2
status: complete
plan_path: docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v4_2026-08-20.md
verdict: changes-required
---

# Implementation review: v4 alignment after model-based remediation

## Scope and method

This read-only review evaluates the current committed implementation against plan v4 and
the accepted model-based remediation plan. It does not treat unimplemented Wave 1–3 product
behavior as a defect merely because the end-to-end plan is unfinished.

The review derived input freshness and commit ancestry, inspected the current contract/model
surfaces, used repository-wide text and structural searches for legacy authority, ran current
executable oracles, and obtained separate plan-status and design-alignment lenses. It also
honors the user's instruction to defer Ubuntu clean-checkout evidence for now.

## Verdict

**Changes required before v4 WP07 resumes.**

The model-based remediation closes the prior review's IR-001–IR-009 architecture concerns at
the intended substrate boundary. One bounded compiler-model correction remains: generated
outputs need first-class many-input compilation units instead of being owned by one source
artifact. The other blockers are repository-history hygiene, stale plan mechanics, and
untrusted schema-1 execution state. After those are corrected in a v5 successor, the original
Wave 1–3 product outcomes remain a viable implementation sequence.

## Outcome and invariant matrix

| Outcome or invariant | Assessment | Executable oracle |
|---|---|---|
| Four current build domains are locally green | conforming | `just ci-fast` |
| One typed catalog and bounded Contract IR | substantially conforming | `just contracts-verify`; IR-013 remains for generated-output derivations |
| Dual semantic/source identity and bundle projection | conforming | `normative_projection_vectors_have_exact_blake3_identities`; `bundle_projection_uses_the_closed_sorted_model_and_retains_member_identity` |
| One canonical packaged artifact index | conforming | `packaged_index_has_the_exact_source_census_and_bytes`; `just adapter-wheel-test` |
| Independent KAT vs generated corpus policy | conforming | `just fixture-check`; seeded JCS mutation test |
| One Protobuf descriptor IR/compiler path | conforming for the Wave-0 substrate | `just proto-repro-check` |
| Contract-IR-to-Pydantic/FastMCP views | conforming for the pre-runtime substrate | `just adapter-contracts-check`; `just adapter-contracts-repro-check` |
| Manual/legacy contract authority decommission | conforming in live implementation | `just governance`; remediation DB02–DB04 checks |
| Seed/PyO3/Maturin zero state | conforming | `just seed-zero-state-check` |
| Readiness Gate A | incomplete as planned | `just contracts-verify-released` fails with 49 draft warnings |
| Packet completion trust | non-conforming | schema/proving-commit `jq` check in IR-012 |
| Generated build-output zero state | non-conforming | tracked-target checks in IR-010 |
| V4 executable continuation contract | non-conforming | current recipe and legacy-mechanic checks in IR-011 |
| Multi-source generated-output provenance | non-conforming | catalog/output inspection and IR-013 focused re-test |

## Findings

### IR-010 — Unpublished history contains 2.53 GB of tracked Cargo build outputs

**Severity:** blocker

**Dimension:** legacy / operations / maintenance

**Evidence:** `git ls-files | rg '(^|/)target/'` returns 7,156 tracked files: 6,803 under
`pyrefly-sidecar/target` and 353 under `rustc-extractor/target`. `git ls-tree -r --long HEAD`
totals approximately 2,534,006,222 bytes for those paths. Both local unpublished commits
contain the same target tree, so an ordinary deletion commit would still push the large blobs.
This violates the repository's generated-target posture and turns transient compiler output
into permanent source history.

**Remediation:** before any push, rewrite the two unpublished commits to exclude all auxiliary
Cargo target outputs, add explicit nested-target ignore coverage, and add a permanent
governance check that rejects tracked build artifacts. Re-run the four-domain gates after the
rewrite. This review does not authorize the history rewrite.

**Focused re-test:**

```bash
test -z "$(git ls-files | rg '(^|/)target/' || true)" && test -z "$(git rev-list --objects origin/master..HEAD | rg ' (pyrefly-sidecar|rustc-extractor)/target/' || true)"
```

### IR-011 — Plan v4 is no longer an executable continuation contract

**Severity:** major

**Dimension:** design alignment / sequence / proof

**Evidence:** direct digest derivation finds all eight suite inputs changed since v4 planning.
The accepted changes are not incidental: v4 still describes language-neutral generated-index
source mirrors, open protocol/orjson intent, an independent adapter schema renderer, and older
Proto generation mechanics. Its final matrix names `adapter-schemas`,
`contracts-regen-check`, and `fuzz-contracts`, none of which exists in `just --list`.
Literal execution would bypass or contradict the accepted typed-catalog, one-FDS, Pydantic,
and independent-KAT design now owned by SUITE AC-G-02/05/07 and RM §6.

**Remediation:** create a v5 successor preserving packet IDs and dependency order while
integrating the remediation decisions. Treat WP07's model/KAT rules, WP09's generated
Pydantic slice, WP10's one-FDS path, and WP11's closed bundle model as standing inputs. Replace
obsolete commands with current recipes and recompute the declared-input table.

**Focused re-test:**

```bash
test -f docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md && rg -n 'adapter-contracts-check|adapter-contracts-repro-check|contracts-repro-check|fixture-check|proto-repro-check' docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md && ! rg -n 'schema fingerprints.*adapter-schemas|regeneration byte-identity.*contracts-regen-check|decoder fuzz.*fuzz-contracts|lockfile-pinned `grpcio`/`protobuf`/`orjson`' docs/plans/codefabric_waves_0-3_foundation_implementation_plan_v5_2026-08-20.md
```

### IR-012 — Execution state cannot currently prove completed packets

**Severity:** major

**Dimension:** evidence / operations

**Evidence:** both execution states use schema version 1. V4 marks six packets complete with
`proving_commit: null`; remediation marks eight complete with `proving_commit: null`. Both
retain derived check evidence and record `current_head` as `da18263` although HEAD is
`cdad126`. The remediation plan also remains `status: draft` while its state says executing.
Under the current execution-state contract, a complete packet requires a recorded proving
commit in current history and passing named checks.

**Remediation:** after history repair stabilizes commit IDs, migrate both states to schema 2,
remove derived facts, retain judgment/deviation history, record the corrected proving commits,
and update next actions. Record Ubuntu clean-checkout as user-deferred assurance rather than
as a current blocker. Do not silently rewrite the historical remediation plan; capture its
accepted/executed disposition in the successor artifacts.

**Focused re-test:**

```bash
jq -e '.schema_version == 2 and ([.packets[] | select(.status == "complete" and .proving_commit == null)] | length == 0)' docs/plans/state/codefabric-waves-0-3-foundation_v4_state.json docs/plans/state/codefabric-model-based-foundation-remediation_v1_state.json
```

### IR-013 — Generated-output ownership cannot represent a compilation unit

**Severity:** major

**Dimension:** architecture / provenance / extensibility

**Evidence:** `ArtifactDescriptor.generated_outputs` and the compiled
`OutputsByPath = Path -> (artifact_id, GeneratedOutput)` model assign each output to exactly one
source artifact. The catalog currently works around this by assigning all Proto descriptor,
census, toolchain, Rust, and Python outputs to `codefabric.manifests.suite-manifest`. The
actual Wave-0 Proto source and four production Proto authorities declare no outputs.
`tooling/proto/generate.py` independently discovers every Proto artifact and then hard-codes
`SOURCE_RELATIVE`, `wave0_probe_pb2*`, and the Wave-0 descriptor filename. The generated index
therefore reports the suite manifest—not the Proto source set—as the output authority.

This is not sufficient for WP10, where four source authorities compile together into one FDS
and several package/domain outputs. Adding more hard-coded names or assigning everything to
the suite self record would recreate the manual authority problem the remediation was meant to
remove.

**Remediation:** correct SUITE AC-G-05 and RM §6 to introduce a closed typed
compilation/derivation unit with a stable ID, producer, sorted input artifact IDs (or a closed
declared input-set selector), outputs/roles, consumers, resource profile, and generator/tool
identity. Derive source sets and destinations from the unit. Bind generated-index output
provenance to the unit and all input semantic/source identities. Use the same model for
registry and adapter generation; do not create a Proto-only exception. Reject missing inputs,
cycles, duplicate output paths, and outputs owned by multiple units.

**Focused re-test:**

```bash
cargo nextest run --no-default-features --features contracts-tooling -E 'test(compilation_unit_derives_many_inputs_and_outputs)' && ! rg -n 'SOURCE_RELATIVE|wave0_probe_pb2|wave0-probe-descriptor' tooling/proto/generate.py
```

## Remaining implementation scope

The incomplete scope is intentional and substantial:

- WP07: CBEF/public-ID/path/type-algebra implementation and normative KATs.
- WP08/WP08b: populated registries, state machines, phrase mappings, grammar, model-pack
  schema, and cross-language generated types.
- WP09: `TableSpec`s, operational DDL, snapshot/state contracts, and production JSON Schemas;
  the adapter Pydantic slice is already supplied by remediation.
- WP10: the four real protocol packages compiled through the landed one-FDS substrate.
- WP11: populated bundles, deployment profile, CF-IDs, traceability, and released Gate A.
- WP12–WP18: the entire secure daemon/workspace/source-control-plane runtime.
- WP19–WP26: the entire canonical fact-state, publication, overlay, snapshot, and serving-
  catalog runtime.

Current compatibility probes for gix, rusqlite, rustix, DataFusion, Delta, tonic, and grpcio
are dependency/API evidence, not partial completion of the production packets that consume
them.

## Design alignment conclusion

The remediation already corrected the governing semantic suite and roadmap. The one new
normative correction is limited to AC-G-05/RM §6's compiler meta-model: first-class
compilation units must represent many-source derivations and generated-output provenance. A
v5 plan should implement that correction, make the existing remediation rules executable
throughout the remaining packets, and add the tracked-target decommission as a repository-
infrastructure prerequisite. Wave 2 and Wave 3 topology can otherwise be preserved.

The accepted local resume order is:

1. repair unpublished history and add target-output zero-state governance;
2. correct and implement the compilation-unit graph;
3. produce v5 and schema-2 states with corrected proving commits and the Ubuntu deferral;
4. rerun local full/reproduction checks and locally close the remediation interlock;
5. resume WP07, then continue the original packet DAG.

## Review checks

- `just ci-fast` — passed.
- `just proto-repro-check` — passed.
- `just adapter-contracts-check` — passed.
- `just contracts-repro-check` — passed.
- `just contracts-verify-released` — failed as expected with 49 draft warnings.
- direct input-digest, commit-ancestry, state-schema, tracked-file, and recipe searches —
  produced the findings above.
