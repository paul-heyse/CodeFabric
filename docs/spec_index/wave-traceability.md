# Capability-stage traceability

This derived view maps the current v2.1 roadmap to approved implementation plan v3. The plan remains
authoritative for exact dependencies, acceptance checks, proving commits, and decommission batches.
Plan v3 is approved but not active until its separate state-creation transaction.

| Stage | Packets | Current-suite boundary |
|---|---|---|
| authority and scope closure | WP28 | sole-current v2.1 routing; prior-outcome, authority, durability/cache, and L-20--L-55 ledgers |
| production composition | WP29 | real daemon programmatic epoch/command/activation/query factory; no default bootstrap backend |
| early rejected-authority cutover | WP30, DB09 | bootstrap/model/importer/generated-schema/old-epoch/migration consumer cutover and physical deletion |
| DataFusion and Delta closure | WP31, WP32 | plan-derived schemas, fixed-point observations, authorized children, bounded caches, exact Delta histories and candidate-free recovery |
| exact providers | WP33 | provider-native Arrow batches, relation-scoped IPC, exclusive admission, coverage/remainders, Rust trust |
| explicit analyses | WP34 | Python/Rust/common derived producer closure, fixed points, precision, provenance, explicit unsupported remainder |
| semantic query | WP35 | eight request forms, highest-rung graph execution, authorized catalog closure, explicit unknown/negative semantics |
| lifecycle and public delivery | WP36, DB10, DB11 | source truth, one command/resource/epoch route, real UDS delivery, presentation-only FastMCP |
| first-principles evidence | WP37 | independently authored decoded rows, causal faults, released-wire expectations, no comparator prerequisite |
| remaining physical purge | WP38, DB12, DB13 | generated/governance/tooling/feature/recipe/dependency/package zero state with exact history exclusions |
| post-purge release proof | WP39 | clean/incremental equality, security/resource/recovery/provenance and public compatibility evidence |
| fenced cutover | WP40 | predecessor restart/reboot revocation, unknown reconciliation, target mutation forward-only |
| final certification | WP41, DB14 | all 56 packet oracles, four domains, exact reconstruction, state/proving trust, independent review |

## Milestone map

| Milestone | Proved capability |
|---|---|
| M01 | successor authority, v2 outcome mapping, and full disposition/durability scope closure |
| M02 | production programmatic composition plus bootstrap/model/dual-epoch zero state |
| M03 | DataFusion/catalog/cache and Delta/durability/recovery closure |
| M04 | exact provider-to-public production delivery across all four domains |
| M05 | independent first-principles release evidence and total targeted purge |
| M06 | fenced forward-only cutover and final certification at one trusted HEAD |

No milestone is satisfied by file presence, an executor statement, a state label, a digest, or
predecessor agreement. Its exact packet oracles and decommission exits must pass at an ancestral
proving commit and again at the certification HEAD.
