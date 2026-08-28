# Invariants and doctrine

Eleven separate invariant lists exist across the suite, plus the roadmap's own and the
cross-cutting doctrine in `CLAUDE.md`. None of them cross-reference each other. This file is
the map: where each list lives, what it governs, and where the lists overlap or diverge.

Nothing here is normative. It is the checklist a `plan-audit` or `implementation-review` pass
runs against, with a citation for every line so the review can quote the spec rather than the
guidance file.

See [`README.md §2`](./README.md#2-citation-convention) for tags.

## 1. Where the lists are

| List | Location | Count | Governs |
|---|---|---:|---|
| Global invariants | `SUITE §0.2` | 10 | the whole release |
| Suite-wide implementation invariants | `RM §1` | 10 | every wave |
| Design principles | `ONT §4` (§4.1–§4.7) | 7 | what a fact is allowed to be |
| Canonical invariants | `GEN §96` | 16 | what provider output must preserve |
| Mandatory invariants | `FAB` Appendix C | 16 | storage, publication, operators |
| Core design principles | `QRY §4` (§4.1–§4.10) | 10 | request and response semantics |
| Hard system invariants | `SRV §6` | 20 | the daemon/adapter boundary |
| Consistency invariants | `LIFE §157` | 32 | snapshots, generations, Git |
| Performance invariants | `LIFE §158` | — | executor placement, budgets |
| Failure invariants | `LIFE §159` | 13 | what a crash may and may not do |
| Generic fallback invariant | `LIFE §80` | 1 | gix degradation |

`LIFE` Part XVII (§157–§159) is the largest single body. `ONT` Part IX (§86) and the four
"Explicit Non-Outputs" appendices (`ONT` App. B, `GEN` App. C, `FAB` App. D, `QRY` App. D) state
the same doctrine negatively — what the system must never emit.

## 2. The two ten-item lists are not the same list

`SUITE §0.2` and `RM §1` both present "ten invariants" and are frequently conflated. They share
six, and each contributes four the other omits. Together they are **fourteen distinct
invariants**, and a wave plan that quotes only one list is missing four.

| Invariant | `SUITE §0.2` | `RM §1` |
|---|:---:|:---:|
| `workspace_id` identifies exactly one authorized source instance | 1 | 1 |
| One immutable leased `ServingSnapshot` is the only query pin | 2 | 2 |
| Current filesystem bytes — not watcher events, Git objects, or prior provider output — are present-state authority | 3 | 3 |
| Provider observations are not canonical until reconciled | 5 | 4 |
| Context-sensitive facts never cross analysis-context boundaries | 4 | 5 |
| Unknown remainder is explicit; missing data does not prove absence | 6 | 6 |
| Every compatibility-sensitive artifact is versioned and fingerprinted | 10 | 9 |
| **Public source disclosure is authorization-scoped independently from fact access** | 7 | — |
| **A partial stream is not a successful logical response until its terminal completeness record is emitted** | 8 | — |
| **Operational state is not semantic program history** | 9 | — |
| **The Rust daemon owns semantic interpretation, planning, execution, snapshots, and canonical result bytes** | — | 7 |
| **The Python FastMCP process remains a thin adapter and never becomes a second query or graph engine** | — | 8 |
| **Incremental results must converge to the clean-rebuild result for identical inputs** | — | 10 |

The four `RM`-only entries are implementation obligations rather than design contracts, which
is why the manifest omits them — but they are each backed by a contract: the daemon/adapter
split by `SUITE AC-G-01`'s ownership map and `SRV §6` invariant 3, and clean-rebuild convergence by
`SUITE AC-G-79`.

## 3. Cross-cutting doctrine, traced to its normative home

`CLAUDE.md` states the project doctrine in prose. Each statement has a normative owner; cite the
spec, not the guidance file.

| Doctrine | Normative home |
|---|---|
| **Fact substrate, not judgment.** No `SAFE_TO_REFACTOR`, `HIGH_RISK`, complexity verdicts, or test-impact conclusions. The query service *rejects* evaluative requests. | `ONT §4.7 Objective derivation is permitted; evaluative interpretation is not` · `ONT §67 No evaluative ontology rule` · `ONT §86` (Part IX Non-Goals) · `FAB` App. C invariant 16 · `QRY` App. D Explicitly Rejected Output Classes · `ONT` App. B |
| **Excluded domains**: git history, runtime observation/coverage, environment inventory. | `LIFE §81 No history ontology` · `FAB` App. C invariant 15 · `RM §24` deferred-beyond-baseline list |
| **Absence is never proof of absence.** Missing provider output materializes as an explicit unknown or capability gap. | `ONT §4.5 Unknown is a first-class fact` · `ONT §66 Mandatory unknown semantics` · `ONT AC-G-73` · `QRY §4.10 Absence is not assumed from missing data` · `QRY AC-G-48` · `LIFE §157` invariant 5 · `GEN §96` (`unknown != absent`) |
| **Compile failure yields capability gaps, not stale-current compiler facts.** | `LIFE §157` invariant 6 · `GEN §97 Capability gaps and required treatment` · `GEN AC-G-36` |
| **Raw and normalized coexist.** Normalization must not block representing a new grammar or compiler variant. | `ONT §4.2 Raw and normalized representations SHALL coexist` · `GEN §12 Raw and normalized observation preservation` · `GEN §96` (`raw provider kind remains recoverable`) |
| **Syntax occurrence ≠ semantic entity.** Call syntax is not a callable; type syntax is not a type. Call sites are first-class. | `ONT §4.3` · `ONT §4.4 Call sites SHALL be first-class entities` · `ONT §65 Required separation of fact types` · `GEN §96` (`syntax occurrence != semantic entity`, `call site != callable`, `type syntax != type entity`) |
| **Canonical identity is application-owned.** Raw `DefId`, MIR indices, Tree-sitter node IDs, Ruff node indices and Pyrefly keys are never canonical. | `ONT §64 Required identity and public encoding rules` · `ONT AC-G-13` · `GEN §13 Canonical semantic-identity inputs` · `GEN §96` (`provider-local ID is never canonical identity`) · `FAB` App. C invariant 3 |
| **Provider isolation.** Every provider sits behind an application-owned adapter emitting application-owned DTOs; no long-lived borrowed provider types escape. | `GEN §7 Provider isolation requirements` (§7.1–§7.5) · `GEN AC-G-32` · `SUITE AC-G-01` |
| **Authority, never silent overwrite.** Conflicts resolve by per-fact-family authority tables; conflicting evidence is retained; unresolvable conflict emits an unknown or multi-candidate fact. | `GEN §5 Authority and precedence` · `GEN §§80–84` reconciliation · `FAB AC-G-37 Canonical reconciliation algorithm` · `FAB §16` (`fact_evidence`) |
| **Atomic present state.** Owner-scoped replacement plus manifest-pinned multi-table MVCC; intermediate versions invisible through `cpg_serving` until the pointer advances. | `FAB §69 Owner replacement protocol` · `FAB §71` · `FAB AC-G-26` · `FAB` App. C invariants 1, 2, 12 · `LIFE §100 Atomicity model` · `LIFE §106 Atomic pointer swap` · `LIFE §157` invariants 9, 10, 20 |
| **Every query pins exactly one immutable snapshot and never mixes source generations.** | `LIFE §157` invariants 1, 3 · `QRY §22 Freshness barrier and ServingSnapshot consistency` · `SRV §6` invariants 1, 2 · `FAB AC-G-19` |
| **Direct ≠ transitive; exact ≠ possible.** Never flattened, at any layer. | `ONT §4.6` · `QRY §4.7`, `§4.8` · `SRV §6` invariants 8, 9 · `GEN §96` (`direct effect != transitive effect`) · `FAB` App. C invariants 7, 8 |
| **The daemon owns semantics; the adapter is presentation only.** No independent mutable CPG state in the FastMCP process. | `SRV §6` invariant 3 · `SRV §5 Responsibility matrix` · `LIFE §122 Central daemon and workspace-registry authority` · `SUITE AC-G-01` |
| **No silent truncation.** Explicit limits, hard rejections, and unavailable facts stay distinct. | `SRV §6` invariant 6 · `QRY §34 Limits and bounded execution` · `QRY AC-G-52` · `QRY AC-G-56` · `LIFE §159` invariant 10 |

## 4. Doctrine stated negatively

Four appendices and one Part enumerate what must never be emitted. Read them together with §3 —
they are the enforceable form of the same rules.

| Location | Contents |
|---|---|
| `ONT §86` (Part IX — Non-Goals) | what the ontology refuses to model |
| `ONT` Appendix B — Explicitly Excluded Analytical Outputs | evaluative outputs excluded at the fact layer |
| `GEN` Appendix C — Explicit Non-Outputs | what generation must not produce |
| `FAB` Appendix D — Explicit Non-Outputs | what no canonical table may contain |
| `QRY` Appendix D — Explicitly Rejected Output Classes | what the query service rejects at request validation |
| `SRV §7 Non-goals` · `SRV §31 Deliberate FastMCP exclusions` | what the adapter refuses to become |
| `LIFE §§81–87` | no history ontology, no Git mutation, no checkout, no external filters, no status on every event, no blob OID as sole identity, no shared `Arc<Repository>` |

`SRV` Appendix E is an anti-pattern inventory; `ONT §87`, `QRY §121` and `GEN`'s closing
sections each state a single "governing design rule" that subsumes the rest of their document.

## 5. The invariants a lifecycle change must satisfy

`LIFE` Part XVII is the checklist for any incremental-update design. It is long enough that it
is worth knowing the shape rather than the 45 individual lines.

**`LIFE §157` — consistency (32).** Groups: snapshot pinning and generation isolation (1–3, 19),
invalidation ordering (4, 7, 9, 10, 20), negative-proof discipline (5, 6), staleness fencing
(8, 17, 18), source authority (11–16), concurrency (21, 28), worktree isolation (22, 23), Git
semantics (24–27, 29–31), and the convergence invariant (32) — *the incremental graph equals a
clean Git-aware rebuild for the same worktree source snapshot*, which is what `SUITE AC-G-79`
and Gate C test.

**`LIFE §158` — performance.** Executor placement and budget rules: lightweight event handlers,
bounded or coalescing queues, path-change collapse to one latest generation, CPU-heavy work off
Tokio workers, blocking gix work in a bounded execution class, process-wide thread budgets that
include gix's own internal parallelism.

**`LIFE §159` — failure (13).** The load-bearing shape: a crash, a failed publication, event
loss, Git-metadata corruption or storage failure may **degrade** — acceleration, durability,
source trust — but may never partially mutate an active snapshot, advance a durable pointer,
corrupt current query state, or silently relabel incomplete work as healthy. `LIFE §80` states
the same rule for gix specifically: gix failure degrades acceleration, not correctness.

`SUITE AC-G-81` turns these into the fault-injection matrix, and Gate D is where they are
proved.

## 6. Which invariants each wave must not break

Not a substitute for `RM`'s per-wave exit evidence — a pointer to the list a wave is most likely
to violate.

| Waves | Most-at-risk list |
|---|---|
| W0–W1 | `SUITE §0.2` 10 (versioning and fingerprinting) · `SUITE AC-G-02`/`AC-G-03` |
| W1 contract identity | `SUITE AC-G-02` separates semantic `canonical_digest`, exact-byte `source_digest`, and AC-G-07 `bundle_digest`; projection IDs are versioned contract data |
| W2–W3 | `FAB` App. C 1–5, 12 · `LIFE §157` 1–3, 9, 10, 20 · `LIFE §159` 1–3 |
| W4–W5 | `GEN §96` in full · `ONT §4` in full |
| W6–W7 | `LIFE §157` in full — especially 32, convergence · `LIFE §158` · `LIFE §159` |
| W8–W12 | `GEN §5` authority · `GEN §96` · `ONT §4.5`, `§4.6` · `ONT AC-G-73` |
| W13–W14 | `FAB` App. C 8, 13, 14 · `ONT §4.6` direct vs transitive |
| W15–W16 | `QRY §4` in full · `QRY AC-G-48` completeness algebra |
| W17–W18 | `SRV §6` in full — the 20 hard invariants are the adapter's whole contract |
| W19 | all of them; Gates D–G are the mechanized form |

## 5. Ontology-program amendment invariant map

The synchronized amendment strengthens the existing lists without creating a parallel doctrine:
one authored authority maps to `SUITE`/`ONT`; causal native relational execution and fail-closed
planning map to `FAB`/`QRY`; semantic self-description and candidate-bound receipts map to
`ONT`/`FAB`; one durable activation command, recovery, and lease compatibility map to
`LIFE`/`SRV`; observation-versus-decision separation maps to `SUITE`/`LIFE`. The complete
wording remains in the master documents, not this map.
