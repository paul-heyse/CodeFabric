# Model-First, Contract-Driven, Provenance-Native Data Fabric — v2

# 0. Purpose, precedence, and what changed in v2

The target architecture should optimize not merely for code that works, but for systems whose
**meaning, authority, state, execution, and history are explicit in the design** — and whose every
assertion about itself is **produced by running something**, not by someone having written it down.

The desired system has six defining characteristics. The first five are carried forward from v1
unchanged. The sixth is what v2 adds:

1. **Model-first:** important semantics exist as explicit typed models rather than being implicit
   in procedural control flow.
2. **Contract-driven:** every important boundary states what is invariant, what may vary, who owns
   the contract, and how compliance is validated.
3. **Authority-centered:** each concept has one canonical source of truth; alternative
   representations are projections, caches, compiled forms, or views of that authority.
4. **Provenance-native:** lineage, versions, configuration, transformations, and execution identity
   are produced automatically as part of normal operations.
5. **Fabric-oriented:** storage, tables, schemas, calculations, plans, execution, and
   interoperability share common representations and lifecycle rules rather than behaving as
   disconnected subsystems.
6. **Execution-proved:** a declaration is admissible only where its referent cannot change
   independently of it. Everything else — validity, drift, capability, correctness, completeness —
   is computed from the authority at the moment it is needed, by executing something or by
   evaluating a relation over the model.

The architecture should consequently resemble a **semantic compiler and execution platform** more
than a collection of service methods. DataFusion illustrates this shape through a compiler-like
sequence from SQL/DataFrame/`LogicalPlanBuilder` into `LogicalPlan`, logical optimization, physical
planning, `ExecutionPlan`, and finally an Arrow `RecordBatch` stream.

## 0.1 Precedence

This document is the **single principles authority** for the system. It supersedes
`full_data_fabric_design_principles.md` (v1) and absorbs the load-bearing content of the retired
`semantic_design_principles_holistic.md`. Where any prior principles text conflicts with this one,
this one governs.

Both predecessors remain on disk, byte-for-byte, as frozen historical records. Neither is cited in
new work. Retiring a doctrine from guidance is a separate act from deleting it from the repository,
and only the first is required for this document to govern — see §0.1's closing note and §B.2.

Precedence against the rest of the corpus is unchanged: the authoritative design suite
(`docs/authoritative_design/`) states what the system must do; this document states how design
decisions are made and what evidence makes them true. A principle here never overrides a normative
`AC-G` contract, a domain specification, or a released interface. It governs the space those leave
open — which, in practice, is most of it.

`docs/library_ref/full_data_fabric_design_principles.md` is retained **byte-for-byte** and is not
edited to record its own supersession. Its bytes are pinned by eighteen recorded digests: twelve plan
declared-input tables and four review frontmatters carry the SHA-256, and two Gate B release
artifacts carry the BLAKE3 form, verified at `src/gate_b_release.rs:653`. Editing it — even to add
one banner line — would invalidate all eighteen at once.

That fact is not an inconvenience to work around. It is the clearest specimen in this repository of
what P26, P28 and P31 exist to prevent: a static declaration of a value that is by definition
computable, replicated across eighteen artifacts, whose only failure mode is that somebody forgets
to update all eighteen. Read it as the worked example behind those three principles.

The same constraint binds `semantic_design_principles_holistic.md`, which is a declared input of the
active plan. `tooling/ci/artifact_contracts.py:668` rejects a *missing* declared input
unconditionally, so that file cannot be deleted without also editing the plan that pins it. It is
therefore frozen rather than removed.

## 0.2 What changed from v1

v1 already argued most of this. Its P2 — "make models executable, not merely descriptive" — is the
seed of the entire v2 thesis, and the fabric, authority, provenance, and hierarchy principles are
carried forward substantially as written. The change is one of **bindingness and scope**.

v1 permitted a declared datum to stand in for a demonstrated behavior. Its P18 asked for
fingerprints of "anything whose identity matters" without separating identity from correctness; its
P17 accepted a preserved artifact where a reproducible one was possible; its P19 allowed
reproducibility to be an aspiration; its P20 stated capability conservatism as an intention rather
than a proof obligation; its P25 asked that tests "derive from contracts" without requiring that
every contract clause name the thing that decides it.

v2 closes those five gaps, generalizes the result into six new principles (P26–P31), and absorbs
five more (P32–P36) from the retired doctrine so that nothing load-bearing is lost.

The whole shift compresses to one sentence:

> **Write down only what cannot change; compute everything else, every time you need it.**

## 0.3 Relationship to the agent evidence policy

`.claude/skills/_shared/evidence-policy.md` §0 states the governing rule for **how an agent
evidences a claim**:

> **Executable beats derived beats recorded.**

That section declares itself the single normative home of that principle, and this document does
not restate it — it cites it. What v2 contributes is the *extension of the same rule from the agent
to the system*. The agent may not hand-transcribe a fact a command derives; by the same logic, the
system may not persist a declaration of a state it can compute. `evidence-policy.md` §4
("staleness is derived, never compared by hand"), §6 (the anti-ledger rule), and
`artifact-schemas.md` §8 (the validated-versus-derived contract) are the operational precedent that
P26–P31 generalize into architecture.

One existing normative sentence is the strongest warrant for the whole pivot. `AC-G-05`, in
`docs/authoritative_design/codefabric_present_state_cpg_suite_governance_and_release_manifest_v1.3.md:444-450`:

> Every governed semantic authority with more than one consumer SHALL declare at least two derived
> operations that consume its typed model rather than its rendered prose or generated bytes. […] A
> consumer that privately re-encodes a governed vocabulary, identity recipe, schema, state machine,
> or query meaning violates AC-G-05 **even when its bytes happen to compare equal**.

Byte equality is not semantic agreement. That sentence already rejects the strongest available
static proof as insufficient. v2 is the general form of it.

---

# A. The staticness test

This is the organizing lens of v2. It is normative and it is unnumbered, because it is not one
principle among thirty-six — it is the question asked before writing any of them down.

## A.1 Three classes of declaration

Every declaration in the system falls into exactly one class. Classify before writing.

### Class 1 — Intrinsically static

The referent **cannot change**, because it is a completed fact about the past or a decision that a
human accountably made.

```text
a committed Delta version and its exact contents
a published snapshot pointer that has already been superseded
a pinned dependency revision
an owner acceptance record
a recorded provenance record about an execution that already happened
an independently authored expectation of what a fixture means
a released wire contract, whose stability across time IS its semantics
```

These are **legitimately declared, and should be frozen hard**. Freezing them is not a compromise;
immutability is their entire purpose. A committed version whose bytes could change would not be a
version. Class 1 is why v2 is not an argument against static artifacts — it is an argument about
which ones earn the status.

### Class 2 — Derived-on-demand

The value is static only because it was **projected from a live authority at some instant**.

```text
schemas and schema fingerprints of current state
capability and feature lists
catalogs, censuses, inventories, registries of what currently exists
traceability from requirements to implementation
the set of consumers of a given authority
"what changed since" summaries
```

**Compute these when needed.** Materialize a copy only for one of two reasons: a wire boundary
whose stability is itself the contract, or a derivation the checking environment genuinely cannot
reproduce — a pinned foreign toolchain, a compiler that is not universally installed. When you do
materialize, the copy is a **cache that carries its own re-derivation oracle**, never an authority.
A materialized Class 2 artifact without a command that regenerates and compares it has silently
become a Class 3 artifact.

### Class 3 — Falsely static

A **hand-maintained assertion about live behavior**.

```text
a ledger of dispositions someone updates by hand
a "supported features" list maintained beside the code that implements them
a claimed capability with no prover
a status table, a conformance matrix, a coverage census
a list of the files that consume something
a stored hash of a thing that is still being edited
```

These are defects. They are not documentation debt to be tidied later — they are **second
authorities that will disagree with the first one**, and the only question is when. Delete them and
replace them with a query over the model.

## A.2 The test

Before writing any declaration, apply this:

> **Can the thing this declares change without the declaration changing?**
>
> If yes, it is not a declaration. It is a cache with no invalidation, and it must become a
> computation.

Three corollaries, each of which catches a different disguise:

- **A digest of a mutable thing is Class 3, not Class 1.** Content addressing makes an *immutable*
  artifact identifiable. Recording the digest of something still under edit records only when you
  last looked.
- **A generated file is Class 2 even though a tool wrote it.** Automation of the write does not make
  the artifact authoritative. The generator's input is the authority.
- **"It is checked in CI" does not promote a class.** If the check compares a committed copy against
  a regeneration, the committed copy is still Class 2 and the check is its re-derivation oracle. If
  no such check exists, it is Class 3 wearing a lanyard.

## A.3 The friction rule

A control whose **only** failure mode is "somebody forgot" is not a control. It is a breakage point
that also happens to produce red builds.

```text
forgot to regenerate the file after changing its source
forgot to add the new module to the list
forgot to bump the version constant
forgot to re-run the digest after editing the document
forgot to add the row to the traceability table
```

Every one of these is a synchronization point between two representations that should have been
one. Eliminate it by construction, in this order of preference:

1. **Derive on read.** The second representation does not exist; it is computed when consumed.
2. **Compute membership.** Where a list must exist, the list is the *result of a query*, not an
   enumeration — "every type implementing this trait", not "these fourteen types".
3. **Close the loop.** Where a copy must exist, a check regenerates it and compares. The copy is a
   cache; the check is the authority.

Only when all three are genuinely impossible does a hand-maintained list survive — and then it is
Class 1 by explicit human acceptance, carrying the name of the person accountable for it and the
condition under which it is revisited.

## A.4 Identity and correctness are different questions

The distinction that P18 and P30 turn on, stated once here because it is the most frequently
collapsed one in practice:

```text
"Is this the same artifact I reviewed?"      -> a fingerprint answers this. Perfectly.
"Is this artifact correct?"                  -> a fingerprint cannot answer this at all.
```

Digests, canonical identifiers, checksums, censuses and manifests **authenticate which execution and
which contract were examined**. They do not establish that the system understood anything. Only
running the artifact against an independently authored expectation does that.

A system can be byte-perfectly reproducible and completely wrong. Reproducibility is a property of
the pipeline; correctness is a property of the answer. v2 asks for both and never accepts the first
as evidence of the second.

---

# B. Reading the principles

## B.1 Citation convention

Principles are cited `DF-P1` through `DF-P36`, or bare `P1`–`P36` where the context is
unambiguous. Cite by **number and title**, never by line number or heading ordinal — titles are
stable across revisions and line numbers are not.

v2 fixes a v1 navigation hazard: v1 numbered principles as `# {N}. Principle {M}` where `N = M + 1`,
so `P25` lived under heading `# 26.`. In v2 every principle is an h1 of the form `# P{n} — Title`,
so the ordinal equals the principle number by construction and `just lib-outline` renders the set
directly.

`P1`–`P25` keep their v1 identifiers. Twenty keep their v1 titles; five are retitled, because in
those five the title itself carried the defect. Every retitle is listed in Y7.

## B.2 Migration from the retired semantic design principles

`semantic_design_principles_holistic.md` is **retired and superseded**: it remains on disk as a
frozen historical record, is referenced by no skill or reference document, and is not cited in new
work. It was written for a different product — one with a workbench, a UI, and solvers — and roughly
two-thirds of its content duplicated principles already stated here. Its load-bearing remainder is
absorbed below. Citations of the form `SD-Pn` or `H-Pn` in historical artifacts resolve through this
table:

| Retired | Absorbed into | Note |
|---|---|---|
| SD-P1 Information hiding, SD-P2 Separation of concerns, SD-P3 Single responsibility, SD-P4 Cohesion/coupling | P35 | general module hygiene, restated as a dependency-structure rule |
| SD-P5 Dependency direction, SD-P7 Acyclic dependencies | **P35** | |
| SD-P6 Ports and adapters | P5 | already the substance of P5; gloss added |
| SD-P8 Trust boundaries and least privilege | P13 | |
| SD-P9 Platform-independent semantics | P6 | |
| SD-P10 Declarative single-sourcing | P3 | |
| SD-P11 Parse, don't validate; SD-P12 Illegal states unrepresentable | **P32** | |
| SD-P13 Stable semantic identity | P18 | |
| **SD-P14 Staged compilation** | **P16** | *hard-coded as the literal `H-P14`; see B.3* |
| SD-P15 Canonicalization before optimization | P6 | |
| **SD-P16 Design by contract** | **P12** | *hard-coded as the literal `H-P16`; see B.3* |
| SD-P17 Functional core, imperative shell | **P33** | |
| SD-P18 Generic runtime and reusable control plane | P5, P15 | |
| SD-P19 Durable domain truth vs temporal control truth | P11 | |
| SD-P20 Unified mutation; SD-P21 Command-query separation; SD-P24 Idempotency | **P34** | |
| SD-P22 Ownership and lifecycle of state | P23 | |
| SD-P23 Explicit failure semantics | P16 | |
| SD-P25 Reproducibility, hermeticity, incrementality | P19 | |
| SD-P26 UI as compiled projection | P3 | generalized: presentation is a projection, never a second truth |
| SD-P27 Provenance and explainability | P9, P10 | |
| SD-P28 Observability as structured data | P24 | |
| SD-P29 Declare and version public contracts | P12, P22 | its forward reference to `contract_substrate_discipline.md`, a file that does not exist, is dropped |
| SD-P30 Design for testability | P25 | |
| SD-P31 Additive extensibility and executable governance | P14, **P36** | |

Its §5 required artifact stack and §6 required pass contract are absorbed into P16. Its §8 secondary
implementation constraints are retained verbatim in Y4. Its §9 anti-principles are folded into Y3.
Its §10 conformance evidence list is superseded by P25, which is strictly stronger. Its UI,
workbench, and solver framing is dropped: this system has none of those surfaces.

## B.3 Two identifiers that code depends on

Two of the retired principles are not merely cited in prose — their identifiers appear as string
literals inside executable artifacts, as the pair `["H-P14", "H-P16"]`:

```text
contracts/registry/transformation-pass-registry.yaml        13 records
tooling/ci/design_principle_alignment.py                    the pass-namespace assertion
src/bin/codefabric_model/registry_cbef_driver.rs            the mirrored Rust check
```

Their successors are **`H-P14` → `P16`** (staged compilation and the pass contract) and
**`H-P16` → `P12`** (design by contract at every boundary).

Because the holistic document is frozen rather than deleted (§B.2), those literals still resolve and
nothing is currently broken. Remapping them to `P16` and `P12` is deferred cleanup, not a
prerequisite for this document to govern. It is called out here rather than left to discovery
because it is precisely the class of coupling P31 exists to eliminate: three files that must be
edited together, in which the only thing preventing divergence is that someone remembers all
three.

---

# P1 — Model semantics before implementing behavior

When a concept is important enough to affect multiple operations, it should exist first as an
explicit **semantic model**.

Do not allow the meaning of the system to reside primarily in sequences of function calls, nested
conditionals, scattered configuration lookups, strings constructed at runtime, conventions
understood only by callers, or duplicated special-case logic. Represent that meaning with typed
structures that can be inspected, validated, compared, serialized, versioned, and transformed.

**Prefer:**

```text
semantic intent -> typed semantic model -> validation -> binding/resolution
    -> compiled representation -> execution
```

over `request -> procedural code with embedded decisions -> execution`.

Ask before implementing substantial behavior:

> **What is the model that represents this concept independently of the code that executes it?**

## The v2 addition: the model must be what execution reads

A model that describes the system alongside code that independently implements it is not a model.
It is a second authority (P3) and a piece of decoration (P27).

The test is causal, not structural: **delete a row from the model and the corresponding behavior
must disappear.** If execution still works, the model was never load-bearing, and the code has been
carrying the semantics all along.

This is not hypothetical here. An independent implementation review of an earlier ontology design
found exactly this failure: generated rule metadata that did not drive execution, self-description
that reduced to a table census, and analysis that was bypassable. The response was to make the
causal connection an architectural property rather than a convention — see P27, which generalizes
that finding.

---

# P2 — Make models executable, not merely descriptive

A weak modeling approach creates configuration DTOs that are immediately unpacked into procedural
code. A stronger approach treats the model as a **declarative program**.

The same model should, where appropriate, support multiple derived operations:

```text
Model
 |- validate                     |- derive provenance dependencies
 |- bind to schemas/catalog      |- derive documentation
 |- compile to Expr              |- derive test fixtures
 |- compile to LogicalPlan       |- fingerprint
 |- render as SQL                |- execute
 |- derive required columns
```

This is analogous to DataFusion's `LogicalPlan`: the logical plan describes **what computation
means**, while a later stage determines **how it runs**.

If a model exists, **do not re-encode its semantics separately in every consumer**. Prefer

```text
one semantic representation -> multiple controlled interpreters/compilers
```

so as to reduce the number of places in which domain semantics can independently drift.

This principle is the seed of the whole of v2. Every one of P26–P31 is a consequence of taking it
completely seriously: if the model is genuinely executable, then validity, drift, capability, and
correctness are all things you *run*, and nothing about them needs to be written down separately.

`AC-G-05` states the operational floor: every governed semantic authority with more than one
consumer declares at least two derived operations that consume its **typed model**, not its
rendered prose or generated bytes. Two derived operations is the minimum that proves the model is a
program rather than a document.

---

# P3 — One authoritative owner for every concept

Every important semantic concept has exactly one clearly identified **authority**.

This does not mean there is only one representation. There may be cached, projected, physical,
indexed, serialized, API-facing, compiled, or materialized forms. But each such form must say what
authority it derives from.

```text
SchemaContract       = semantic authority
Arrow Schema         = canonical runtime representation
DFSchema             = planning-qualified representation
ExecutionPlan schema = physical execution representation
RecordBatch schema   = runtime realization
Parquet schema       = persisted representation
API schema document  = exposed projection
```

For every substantive design object, be able to answer:

```text
Who owns the truth?
Who may mutate it?
Who may derive from it?
How is a derived representation tied back to it?
How is stale derivation detected?
```

If two components can independently decide what the same concept means, reject the design.

## The v2 addition: staleness is answered by re-derivation

The last question — *how is stale derivation detected?* — has exactly one admissible answer in v2:
**by re-deriving and comparing**, not by consulting a stored marker.

A recorded source-digest beside a derived artifact detects staleness only if something recomputes
it. Absent that, the digest records when someone last looked, which is a different fact wearing the
same name. The digest is fine; the *check that recomputes it* is the principle.

Presentation surfaces are the special case worth naming, because they are the most common accidental
second authority. A UI, an adapter, a report, or an MCP tool response is a **projection of the
authority, never a second truth**. When presentation code begins deciding what something means, the
meaning has moved and the authority is now wrong.

---

# P4 — Use explicit conceptual hierarchies to encode shared guarantees and legal variation

The `CatalogProvider -> SchemaProvider -> TableProvider` family is the design exemplar. The
hierarchy is not organizational; it establishes **levels of semantic responsibility**:

```text
CatalogProviderList  -> owns catalog namespace
CatalogProvider      -> owns schema namespace
SchemaProvider       -> owns table namespace
TableProvider        -> owns table contract and scan/write behavior
```

A hierarchy must answer two questions unambiguously.

**What is universal?** Every `TableProvider` has a schema, a table type, a scan contract, a defined
relationship to filters/projections/limits, planning metadata, and a defined write posture.

**What may differ?** Where data resides, how scans execute, what pushes down, whether writes are
supported, what statistics exist, what authorization applies, and which backend serves it.

The consumer interacts with the **shared contract**, not with backend-specific branches. That
produces substitutability without pretending implementations are identical.

---

# P5 — Encode variability behind contracts, not throughout consumers

Once a hierarchy exists, consumers must not continually ask:

```rust
if source_is_delta { ... } else if source_is_parquet { ... } else if source_is_api { ... }
```

Prefer `consumer -> canonical contract -> backend-specific implementation`. Backend-specific
knowledge is localized to the adapter or provider that owns that variability.

The design test:

> **If we introduce another valid implementation of this concept, how many existing modules must
> change?**

The desired answer is **none outside registration/configuration and the new implementation itself**.
That is what makes the system a fabric rather than a collection of integrations.

## Ports and adapters

Stated in the boundary vocabulary absorbed from the retired doctrine: the core expresses its needs
through explicit **ports**, and technology-specific mechanisms exist as **adapters** implementing
those ports. Storage engines, compilers, parsers, transports, and external tools are adapter
concerns, never core concerns. Two consequences are load-bearing:

- tests must be able to substitute an alternate adapter with minimal friction — and if no second
  adapter has ever been substituted, the boundary is unproven (P25);
- external systems must not infect internal semantic types. No borrowed provider type escapes its
  adapter.

The runtime consequence is the same rule at execution time: execute **classes** of compiled cases
through one generic path, rather than growing a per-case execution branch. One exhaustive lowerer
beats twenty special cases, because only the former can be proved total.

---

# P6 — Separate semantic meaning from execution strategy

```text
LogicalPlan   = what should happen
ExecutionPlan = how it should happen
```

A filter remains semantically a filter regardless of partitioning, object store, vectorization,
batch size, join implementation, spilling, parallelism, streaming, or physical file layout.

For any substantial subsystem, distinguish:

```text
intent -> semantic representation -> validated representation
       -> physical strategy -> runtime execution -> observed result
```

Do not contaminate the semantic model with implementation choices unless those choices actually
alter semantics. A model should say `join A to B on key K`, not
`perform an 8-way hash-partitioned HashJoinExec with 16 partitions`. This separation permits
optimization without semantic rewriting.

## Canonicalization precedes optimization

Alias resolution, unit normalization, default materialization, topology normalization, and
equivalent-expression normalization all happen **before** any optimizer-specific lowering.
Equivalent authored inputs must converge on equivalent normalized form.

This ordering is what makes P28 affordable. Difference between two states is only cheap to compute
when both have been canonicalized first; otherwise every comparison must re-implement equivalence,
and equivalence implemented twice is P3 violated.

The corresponding platform-independence rule: platform detail does not appear in authored semantics
except through declared bindings or capabilities.

---

# P7 — Build a shared canonical data fabric

"Data fabric" here does not mean a vendor product or a data catalog. It means:

> **The system possesses a small number of canonical representations through which data, schemas,
> queries, storage, and metadata compose across otherwise independent capabilities.**

The Arrow/DataFusion ecosystem exhibits this: Arrow provides the memory model, `object_store`
provides storage semantics, Parquet/IPC/Flight provide persistence and transport, and DataFusion
consumes and emits `RecordBatch` streams at the query layer.

```text
+-----------------------------------------------+
| Domain / semantic models                      |
| PlanSpec  CalculationSpec  SchemaContract     |
+----------------------+------------------------+
                       v
| Catalog / authority plane                     |
| Catalog -> Schema -> Table -> Function        |
+----------------------+------------------------+
                       v
| Logical computation plane                     |
| Expr  DFSchema  LogicalPlan                   |
+----------------------+------------------------+
                       v
| Physical execution plane                      |
| ExecutionPlan  RecordBatchStream              |
+----------------------+------------------------+
                       v
| Common data plane                             |
| Arrow Schema  Array  RecordBatch              |
+----------------------+------------------------+
                       v
| Persistence / transaction plane               |
| Parquet  object_store  Delta snapshot/log     |
+-----------------------------------------------+

            <-> provenance throughout <->
```

Each boundary uses **canonical semantic objects**, not bespoke translations unique to each pair of
components.

---

# P8 — Treat the common representation as infrastructure

Arrow makes the data representation itself a compositional primitive: raw Arrow is the
interoperability and memory substrate, and DataFusion is the relational planner and execution engine
over it.

> **Prefer a single canonical representation flowing through components over repeated conversion
> into component-specific internal DTOs.**

For tabular computation, preserve Arrow-native representations wherever practicable. More generally,
deliberately choose canonical representations for:

```text
data  schema  expressions  plans  identifiers  versions
provenance  diagnostics  policy decisions
```

This significantly reduces adapter code and semantic mismatch — and it is what makes P29 possible at
all. Validation can only be a relational query over the model if the model has a relational
representation to begin with.

---

# P9 — Make provenance intrinsic to every meaningful transformation

Provenance is not reconstructed from logs after a failure. It is an **automatic output of normal
computation**.

A derived artifact should answer:

```text
What produced me?   From what inputs?   At what versions?
Against what schema contracts?   Using what calculations?   Using what plan?
Using what configuration?   Using what software version?
Under what execution/request identity?   When?   Into what committed state?
```

Delta makes this durable at the table-transition boundary through standardized commit metadata —
`application_id`, `pipeline_name`, `source_table_versions`, `input_snapshot_pin`,
`schema_contract_version`, `request_id`, `git_sha`, `build_id` — treated explicitly as
audit/provenance data.

Provenance is designed **before** the operation is implemented. Do not accept "we can add tracing
later." Require instead:

> **What provenance record does this operation produce by construction?**

## The v2 addition: emitted, never maintained

"By construction" is load-bearing and excludes an entire family of designs. Provenance is **emitted
by the operation that produces the artifact**, in the same transaction, by the same code path. A
provenance record that a separate process assembles, that a human curates, or that a later job
back-fills is a Class 3 declaration (§A.1): it describes an execution it did not witness.

The consequence is a hard rule. If an operation cannot emit its own provenance, that is a finding
about the operation, not a licence to record the provenance elsewhere.

---

# P10 — Seek provenance closure

> Starting from any durable result, an operator can recursively resolve the material facts required
> to explain how it came into existence.

```text
output Delta version 184 -> commit metadata -> execution/request ID
  -> physical + logical planning bundle -> PlanSpec version
  -> CalculationSpec versions -> input table versions
  -> input schema fingerprints -> source objects / snapshots
```

Not every byte of the chain must be embedded in every artifact; stable references are sufficient.

## The v2 addition: closure is a resolver, not an assertion

v1 said the chain must be "deliberately resolvable." v2 requires that it be **mechanically
resolvable, and that the resolver exists and runs**.

The difference is total. "Deliberately resolvable" is a property nobody can falsify: every chain
looks resolvable until someone tries to walk it and finds the third link points at an artifact that
was garbage-collected, or a version identifier whose format changed, or a request ID that was never
persisted. A closure check that starts from a real durable result and walks to the source objects
discovers these in seconds.

So: provenance closure is proved by a check that performs the walk, on a real artifact, and fails on
a broken link. Every intermediate reference the walk cannot resolve is a defect in the chain, and
the check's coverage — which artifacts it starts from — is part of the claim.

---

# P11 — Prefer immutable snapshots and explicit state transitions

Target `state N + explicit operation + explicit inputs + explicit policy -> state N+1`, rather than
a shared mutable object that arbitrary callers modify.

Arrow reinforces this through immutable columnar data and batch-oriented transformations; Delta
through table versions and transaction-log-mediated state transitions.

This does not mean all runtime structures must literally be immutable. It means **semantically
significant change** has a before state, an operation, an after state, a version or identity,
validation, and provenance. A mutable cache is acceptable. A silently mutable authoritative table
definition is not.

## Durable domain truth and temporal control truth are different kinds

The distinction absorbed from the retired doctrine, and the runtime half of the staticness test:

```text
DURABLE DOMAIN TRUTH        facts, relations, assumptions, contracts, committed versions,
                            published snapshots, accepted artifacts
                            -> versioned, immutable, provenance-carrying

TEMPORAL CONTROL TRUTH      execution state, retry state, cancellation, leases, in-flight
                            candidates, queue depth, current pointer
                            -> owned, reconstructible, never persisted as domain history
```

Conflating them produces both of the failure modes v2 exists to prevent. Treating durable truth as
temporal loses history that cannot be recovered. Treating temporal truth as durable freezes a
snapshot of something still moving — which is precisely the Class 3 declaration of §A.1.

Committed, immutable state is the canonical **legitimate** static declaration, and the clearest case
of Class 1. A published Delta version is not a cache of the table; it *is* the table at that
version, forever. Declare it, pin it, and depend on it without hesitation. The staticness test is
not an argument against this — it is the argument for reserving this treatment for things that have
earned it.

---

# P12 — Schemas are executable contracts, not documentation

Schema is one of the strongest authorities in the architecture. A schema contract encompasses far
more than column names:

```text
field identity  name  type  nullability  ordering  nested structure
semantic annotations  units where applicable  constraints
compatibility policy  schema version  fingerprint
```

**Do not:** infer contracts from example data; use arbitrary map keys as schema; silently widen or
narrow types; silently change nullability; silently reorder columns; use expression display strings
as durable field names.

**Do:** validate schema at boundaries; make compatibility explicit; alias derived fields
deterministically; separate source schema from canonical schema; record schema version in
provenance.

Types are part of system behavior, not a compiler inconvenience.

## The v2 addition: recompute, do not trust the recorded value

v1's instruction to "fingerprint stable contracts" is retained with its meaning made exact. The
fingerprint is not the contract and storing it is not validation. The contract is enforced by
**recomputing the fingerprint from the live schema and comparing** at the boundary where a mismatch
would matter. A stored fingerprint that nothing recomputes records only the moment it was written.

## Design by contract at every stable boundary

Generalized from schemas to all boundaries — this is the successor home for the retired `H-P16`.
Contracts exist not only for schemas but for adapters, semantic commands, runtime services,
projection builders, transport protocols, and publication surfaces. Every stable boundary declares
its preconditions, postconditions, and invariants **explicitly**, and per P25 each such clause names
the executable that decides it.

Public contracts are declared and versioned deliberately. Contract evolution is intentional and
visible, never incidental.

---

# P13 — Put governance at the authoritative boundary

Security, tenancy, visibility, and policy are enforced where the relevant semantic authority lives:

```text
CatalogProvider         -> namespace visibility
SchemaProvider          -> table visibility
TableProvider::schema() -> visible columns
TableProvider::scan()   -> tenant predicates / access policy
function registry       -> callable calculation policy
logical-plan validator  -> query policy
write/transaction boundary -> mutation policy
```

This is superior to duplicating security decisions throughout arbitrary callers because it makes
enforcement **structural**. Do not claim a capability — filter pushdown among them — that is not
actually enforced at the boundary claiming it.

## Narrow authority and least privilege

Authority, trust, and privilege are explicit architectural properties. Distinguish untrusted inputs,
privileged operations, sensitive data, and trusted internal artifacts, and keep authority narrow,
centralized where necessary, and auditable.

Sensitive operations — publication, external write, privileged execution, credentialed access —
must not be ambiently available. Where a capability token, lease, or scope governs an operation, its
absence must **fail closed**.

## The v2 addition: enforcement executes, or it does not exist

A policy expressed as metadata that no code consults is not governance. It is a label.

The test is the same causal one as P27: **revoke the authority and the operation must fail**. If the
operation still succeeds, the enforcement point is somewhere else, or nowhere. This is what
separates a governance boundary from a governance *diagram*, and it is why a negative test — the
denied case — is the load-bearing half of the evidence. A test that proves the permitted case
succeeds proves nothing about enforcement.

---

# P14 — Prefer the highest-level extension that preserves the semantics

Extension points form a hierarchy. Do not immediately drop to the most powerful abstraction. The
preferred progression:

```text
UDF > TableProvider > SQL planner hook > LogicalPlanBuilder
    > LogicalPlan::Extension > ExecutionPlan > custom QueryPlanner
```

Equivalently for calculations: built-in SQL/`Expr` before scalar UDF; UDF before custom physical
execution; specialized aggregate/window/table abstractions rather than forcing all behavior into
scalar functions.

> Prefer **the most declarative representation that fully expresses the requirement**.

Higher-level representations preserve more semantic visibility, optimization opportunity,
validation, portability, explainability, security inspection, and testability. Drop lower only when
the semantics actually require it.

## Additive extensibility

New capability enters primarily through added semantics, passes, projections, or adapters — not
through repeated edits to the runtime core. The litmus test: if realizing new scope regularly
requires editing the core, rewriting the shell, or introducing a one-off execution branch, the
extension hierarchy is not being used and the design has regressed to P5's rejected form.

---

# P15 — Preserve optimizer visibility

A custom abstraction is not automatically a better abstraction. If introducing one hides useful
semantic structure, it may make the system worse.

```text
amount > 1000 AND status = 'paid'          -- visible to an optimizer
is_high_value_paid_order(amount, status)   -- opaque, when the UDF is opaque
```

Prefer transparent `Expr` composition; reserve UDFs for true domain kernels or behavior that cannot
be cleanly represented by built-ins.

Before introducing an abstraction, ask:

> **What semantic information becomes invisible once I wrap this?**

Good encapsulation hides implementation detail. Bad encapsulation hides information other system
components need for reasoning.

## The v2 generalization: visibility to the validator, not only the optimizer

The optimizer is one consumer of structure. The **validator** is another, and under P29 it is the
more important one.

An invariant can be evaluated as a relational predicate only if the structure it quantifies over is
still visible in the model. Wrapping a relation in an opaque function does not merely cost a
pushdown — it converts a checkable claim into an unmentionable one, and forces validation to fall
back to scanning text.

So the question generalizes: what becomes invisible **to anything that must reason about this** —
optimizer, validator, provenance resolver, or drift computation? An abstraction that improves
readability while making an invariant uncheckable has made a bad trade, and P29 makes that trade
explicit rather than silent.

---

# P16 — Treat lifecycle phases as first-class architecture

Operations move through explicit phases:

```text
declare -> resolve -> validate -> normalize -> compile -> optimize
        -> authorize -> execute -> verify -> commit -> observe
```

Not every subsystem needs all phases, but important operations must make their phase boundaries
visible. The benefits are a better error taxonomy, easier debugging, deterministic hooks, easier
testing, explicit policy gates, clean provenance, and inspectable intermediates.

## Staged compilation

The successor home for the retired `H-P14`. Execution proceeds through **staged compilation across
distinct artifact forms**, never by direct interpretation of raw authored input. At minimum,
normalization is separated from execution-oriented lowering, and projection generation is separated
from execution lowering.

Each compile or projection pass declares a **pass contract**:

```text
purpose                        invalidation effects on downstream artifacts
input artifact types           diagnostics and failure classes
output artifact types          determinism expectations
required invariants on entry   preserved semantic identities
established invariants on exit newly generated semantic identities
```

A pass without an explicit contract is an ungoverned transformation and therefore architectural
debt. Per P25, each clause of a pass contract names the executable that decides it; a declared
invariant with no oracle is not part of the contract.

## Explicit failure semantics

Failures are classified, never flattened. Distinguish at minimum authoring failures, validation
failures, reference-resolution failures, compile failures, capability mismatches, infeasibility,
transient infrastructure failures, programmer defects, cancellation, timeout, and publication
failures.

A failure identifies its phase:

```text
schema_binding  type_validation  logical_planning  policy_validation
physical_planning  execution  write_validation  commit
```

rather than returning `operation failed`. This is not a diagnostics nicety: a phase-tagged failure
taxonomy is what allows a negative test to assert *which* rejection occurred, and an assertion that
merely requires "an error" cannot distinguish correct rejection from an unrelated crash.

---

# P17 — Make intermediate artifacts reconstructible by re-execution

> *Retitled in v2. v1: "Make intermediate artifacts inspectable and reproducible."*

The plan must not disappear between "request accepted" and "result produced." For important
transformations, the input semantic spec, resolved dependencies, validated model, compiled
representation, configuration snapshot, software versions, input versions, output contract,
diagnostics, metrics, and result identity must all be available for reasoning about what occurred.

## The v2 addition: reconstruction is the norm, preservation the fallback

v1 said "preserve or make reconstructible" and treated the two as equivalent. They are not.

**Reconstruction is strictly better** and is the default. If the inputs are pinned and the pipeline
is deterministic, the intermediate artifact does not need to be stored at all — it needs to be
*regenerable on demand*, which is a stronger property, because a stored artifact can be stale, can
be lost, and can silently disagree with the code that supposedly produced it. A regenerable one
cannot: it is produced by the current code, from the pinned inputs, at the moment it is asked for.

Preserve an intermediate artifact only when reconstruction is genuinely unavailable:

- the operation is non-deterministic or depends on an external service;
- the producing environment is not reproducible where the artifact is consumed;
- the artifact is evidence about a past execution, in which case it is Class 1 by definition (§A.1)
  and preserving it is correct.

When you do preserve, the artifact carries the identity of the inputs it came from, so that
reconstruction remains possible later and disagreement remains detectable.

---

# P18 — Fingerprint for identity, never for correctness

> *Retitled in v2. v1: "Fingerprint anything whose identity matters." This is the largest single
> change from v1.*

Human-readable names are rarely enough for strong reproducibility. Deterministic fingerprints
should be considered for schema contracts, calculation specs, plan specs, logical plans, function
registries, catalog snapshots, configuration sets, source snapshots, dependency environments, and
policy sets.

A useful conceptual identity:

```text
ArtifactIdentity { semantic_id, semantic_version, fingerprint, environment_fingerprint }
```

This lets the system answer **"Is this actually the same thing?"** rather than relying on labels.

## What a fingerprint proves, and what it cannot

That question — *is this the same thing?* — is the **only** question a fingerprint answers. It is
worth answering, it is answered perfectly, and it is not the question anyone actually cares about
most of the time.

```text
"Is this the same artifact I reviewed?"   -> a fingerprint answers this. Definitively.
"Is this artifact correct?"               -> a fingerprint cannot contribute to this at all.
```

Digests, canonical identifiers, checksums, censuses and manifests **authenticate which execution and
which contract were examined**. They do not establish that the system understood anything. Two runs
producing identical bytes proves the pipeline is deterministic; it says nothing about whether the
bytes are right. A system can be byte-perfectly reproducible and completely wrong.

## Rules

- **Fingerprint immutable things.** A fingerprint of a mutable artifact is Class 3 (§A.1): it
  records when someone last looked. Content addressing is for content that cannot change.
- **Never let a fingerprint comparison stand in for a behavioral assertion.** If the claim is about
  behavior, the evidence is an execution (P25, P30). `AC-G-05` states the general form: a consumer
  that privately re-encodes a governed meaning violates the contract *even when its bytes happen to
  compare equal*.
- **Define a versioned canonicalization algorithm per fingerprint domain**, and namespace any
  fingerprint that depends on an engine's internal representation by that engine's version.
- **Use fingerprints in cache keys and provenance references, never names alone.**
- **Separate the purposes.** Identity, integrity, and cache-keying are three different jobs. A
  single digest function serving all three invites one of them to be silently weakened for another's
  convenience.

---

# P19 — Prove reproducibility by re-execution

> *Retitled in v2. v1: "Make reproducibility a normal operating mode."*

Reproducibility should not require forensic work. Given a meaningful past result, the architecture
should recover the input versions, schema versions, calculation versions, query/model spec,
configuration, software and library versions, execution environment, and output version.

Exact reproducibility is not always achievable — external services and volatile operations are
obvious exceptions — but **reproducibility status is itself modeled**:

```text
Reproducibility {
    deterministic: true,           inputs_pinned: true,
    external_dependencies_pinned: true,
    volatile_functions: false,     environment_recorded: true,
}
```

That is more useful than an undocumented assumption that a calculation "should probably reproduce."

## The v2 addition: the status is measured, not declared

A `Reproducibility` record whose fields are *asserted by the author* is a Class 3 declaration. Every
field above is a claim about behavior, and every one of them is decidable by running something:

```text
deterministic          -> execute twice, compare. This is the whole test.
inputs_pinned          -> resolve every declared input; an unpinned one is discovered, not remembered
volatile_functions     -> inspect the resolved function set for declared volatility
environment_recorded   -> attempt the reconstruction and see whether it succeeds
```

So the record is an **output of a check, not an input to one**. Where the check has not run, the
honest value is unknown (P20), not `true`.

## Incrementality follows declared semantic dependencies

Incremental recomputation is driven by the declared dependency graph of the semantic model — never
by ambient machine state, filesystem timestamps, or scattered dirty flags. Cache keys incorporate
semantic inputs, reference-context versions, and pass and tool versions.

This matters beyond performance. An incremental system whose invalidation is driven by ambient state
cannot be re-executed to check itself, which forfeits every proof in this principle.

---

# P20 — Advertise only capabilities an executable prover confirms

> *Retitled in v2. v1: "Be conservative about claimed capabilities."*

Metadata that influences execution must be truthful. This is especially important for:

```text
filter pushdown  projection pushdown  ordering  partitioning  uniqueness
constraints  statistics  nullability  determinism  function volatility  idempotency
```

A false optimization hint is worse than no hint.

> **Unknown is preferable to falsely known.**

If an implementation cannot guarantee a capability, advertise it as unavailable or uncertain. Never
invent optimizer-relevant facts to improve performance.

## The v2 addition: unknown is the default, not the fallback

v1 treated conservatism as a disposition. v2 makes it a state machine with a default:

```text
A capability is UNKNOWN until an executable prover confirms it.
Confirmation is a test that fails when the capability is absent.
A capability with no prover is reported as unavailable -- not as present-but-untested.
```

The asymmetry is the point. Claiming a capability you do not have corrupts the consumer's reasoning
silently and at a distance: a planner that trusts a false pushdown claim produces wrong answers, not
slow ones. Declining to claim a capability you do have costs only performance. The costs are not
comparable, so the default is not symmetric.

This connects directly to a doctrine already binding elsewhere in this system: **absence is never
proof of absence**. A missing provider result materializes as an explicit unknown or a capability
gap, never as an empty result that a consumer may read as "none". The same discipline applies to
capability advertisement: silence means unknown, and unknown is a value that must be representable
in the contract.

---

# P21 — Separate enforced semantics from advisory metadata

Metadata is valuable, but it must have a defined semantic class:

```text
Enforced              types, constraints, validation rules, access policies
Planner-consumed      statistics, ordering, partitioning, pushdown support
Contractual           semantic type, units, schema version, field identity
Governance            classification, retention, masking
Lineage               producer, source version, run identity
Advisory              display name, precision hint, description
```

Never assume that writing a metadata key causes the runtime to enforce it. Metadata is not a
substitute for `DataType`, nullability, qualifiers, constraints, or runtime validation; it is an
annotation channel.

## The v2 addition: the class is discovered, not declared

The classification above is a description of **what the code actually does**, and it is decided by
one question: *does an executable enforcer read this key and fail when it is violated?*

```text
an enforcer exists and rejects violations  -> Enforced
a planner reads it and changes its plan    -> Planner-consumed
a consumer depends on its meaning          -> Contractual
nothing reads it                           -> Advisory, whatever the label says
```

Labelling a key "enforced" does not enforce it. If no enforcer exists, the key is advisory and the
label is a lie of exactly the kind P20 rejects — a claimed capability with no prover. Either write
the enforcer or reclassify the key; leaving it labelled `enforced` is the worst of the three
outcomes, because consumers will rely on it.

Every key's class is therefore verified the same way: **violate it and observe what happens.** If
nothing happens, it is advisory.

---

# P22 — Use protocols and canonical boundaries for interoperability

Interoperability happens at deliberate protocol boundaries rather than through accidental object
conversion. The Arrow stack embodies this:

```text
RecordBatch / RecordBatchReader   Arrow IPC   Parquet
C Data Interface   C Stream Interface   PyCapsule   Flight   Substrait
```

Prefer Arrow IPC/Parquet for file interchange, PyCapsule or the C Stream Interface for in-process
interoperability, and `RecordBatchReader` for streaming, rather than unnecessary row-wise or
frame-library materialization.

> **Integrate through stable semantic protocols whenever possible; write pairwise adapters only when
> no common boundary exists.**

A released wire contract is Class 1 (§A.1): its stability across time *is* its semantics, so pinning
its bytes is correct rather than a compromise. This is the clearest legitimate case for a committed
generated artifact — and it remains subject to the closure rule of §A.3, meaning a check regenerates
it and compares.

---

# P23 — Keep state ownership local and explicit

Each stateful concern declares:

```text
scope  owner  lifetime  mutability  refresh policy
concurrency policy  invalidation policy  authority relationship
```

Useful scopes: process, runtime, session, tenant, query, transaction, batch, partition, request.

Caches must never silently become authorities. A cache is conceptually

```text
CacheEntry { derived_from, source_version, fingerprint, created_at, invalidation_policy, value }
```

not `HashMap<Key, Value>`. That turns caching into a controlled optimization rather than a second
source of truth.

Every mutable thing or scarce resource has an explicit lifecycle — creation, use, handoff, shutdown,
cleanup, disposal. This covers caches, workers, transactions, temporary artifacts, locks, sessions,
handles, and live runtime graphs.

## The v2 addition: validity is established by re-derivation

`invalidation_policy` is a declared field, and per §A.1 a declared policy about live behavior is
Class 3 unless something executes it. A TTL is not validity; it is a guess about validity with a
timer attached.

The cache entry's correctness is established by **re-deriving from `derived_from` at
`source_version` and comparing**. A cache whose entries cannot be re-derived cannot be validated at
all, and has therefore already become the second authority this principle prohibits — the only
remaining question is whether anyone notices before it matters.

---

# P24 — Make observability semantic, not merely operational

Traditional observability asks how long a function ran, whether it errored, and how much memory it
used. The stronger system also asks:

```text
Which table versions were read?      Which schema was bound?
Which calculation versions ran?      Which logical plan was chosen?
Which physical strategy resulted?    Which predicates were pushed down?
Which configuration affected planning?   Which commit did the write produce?
```

`EXPLAIN`, logical and physical plans, schema snapshots, metrics, Delta history, and commit metadata
collectively demonstrate this richer notion. Observability covers both **runtime observability** and
**semantic observability**.

Observability and provenance are complementary but distinct: provenance explains derivation,
observability explains execution. Keep them separate — an observed metric must never become an input
to an accountable decision, or measurement noise acquires the authority of a contract.

---

# P25 — Every contract clause names its executable oracle

> *Retitled in v2. v1: "Make testing derive from contracts and invariants."*

Every contract suggests its own tests. If a `TableProvider` promises a schema, projection, filter
pushdown, statistics, and write semantics, tests prove those claims. If a calculation declares a
type policy, null policy, units, and determinism, tests verify them. If a plan model declares
logical semantics, tests compare the unoptimized result, the optimized result, the
serialized/deserialized result, and the physical execution result.

The test architecture follows the models. Do not write tests only around whichever functions happen
to exist.

## The v2 addition: a clause with no oracle is not a contract

v1 asked that tests derive from contracts. v2 inverts the obligation and makes it total:

> **Every clause of every contract names the executable that decides it. A clause with no named
> oracle is not part of the contract — it is a wish, and it is deleted or given one.**

This is the operational form of the whole document. It is what prevents a contract from accumulating
aspirational language that nothing enforces, and it is what makes P21's classification decidable and
P20's capability states discoverable.

An oracle must be **substantive**. The following are not oracles, and a contract naming one of them
is still unproven:

```text
a check that asserts the artifact exists
a check that counts rows or files without inspecting them
a check that passes unconditionally, or whose selector matches nothing
a check that compares the system's output against the system's own earlier output
a prose attestation that the invariant holds
```

The last two matter most. A zero-match selector silently passes forever — a test selector that
selects nothing must **fail**, not succeed. And a comparison against self-generated expectation
proves only self-consistency, which is P30.

## What the oracle must distinguish

An oracle earns its name by **failing when the claim is false**. The design question is therefore
not "does the test pass?" but "what change to the system would make this test fail?" — and if the
answer is "none I can name", the test is measuring its own existence.

The strongest available form is proof by construction (P32): an invariant the type system makes
unrepresentable needs no runtime oracle, because there is no execution in which it can be violated.
Prefer that wherever it is reachable, and name the compile-time construction as the oracle.

---

# P26 — Declare only what cannot change

The normative form of the staticness test (§A).

A value may be written down as a static declaration **only if its referent is immutable by
construction**: a completed event, a released version, a pinned revision, an accountable human
decision, or an independently authored expectation. Everything else is computed from the authority
at the moment it is needed.

Before writing any declaration, classify it (§A.1) and apply the test (§A.2):

> **Can the thing this declares change without the declaration changing?**
>
> If yes, it is not a declaration. It is a cache with no invalidation, and it must become a
> computation.

## What this rejects

```text
a registry of the current consumers of an authority
a census of what currently exists
a table of which components satisfy which requirement
a stored digest of a document still being edited
a "supported operations" list beside the code that implements them
a conformance matrix maintained by hand
a version constant that must be bumped when behavior changes
```

Each of these is a second representation of something the system already knows. The system will
change; the representation will not; and the gap will be discovered by whoever is relying on it at
the time.

## What this protects

Class 1 declarations are not merely tolerated — they are load-bearing and should be frozen hard:

```text
committed table versions and published snapshot pointers
pinned dependency revisions and toolchain identities
owner acceptance records
recorded provenance about executions that already happened
independently authored expectations (P30)
released wire contracts, whose stability across time is their semantics (P22)
```

The difference between the two lists is not "static versus dynamic". It is whether the thing
described can move underneath the description. Where it cannot, declaration is exactly right.

## The residual case

Where derivation is genuinely impossible — an external constraint, an unavailable environment, a
judgment no tool can make — the declaration survives as **Class 1 by explicit human acceptance**. It
then carries, in the artifact itself:

```text
who accepted it       under what rationale
when it is revisited  what would falsify it
```

An accepted static declaration with no named owner and no revisit condition is not accepted; it is
abandoned.

---

# P27 — Every declaration must be causally load-bearing

A declared fact must **drive the execution it describes**. Metadata that execution does not consume
is not documentation of the system; it is a parallel fiction that will diverge.

## The test

> **Change the declaration. The observable behavior must change.**
>
> If it does not, the declaration is decorative. Delete it, or wire it.

This is a stronger requirement than "the declaration is accurate", and deliberately so. An accurate
declaration that nothing reads is accurate only until the next commit, and nothing will detect the
moment it stops being so. A declaration that execution *depends on* cannot silently drift, because
drift breaks the behavior.

## Why this is stated as a principle here

This system has already paid for the absence of this rule. An independent implementation review of
an earlier ontology design found that generated rule metadata did not drive execution, that a claim
of recursive self-description reduced to a table census, that domain analysis was bypassable, and
that a critical stage depended on a test-only route. None of those was a missing feature — each was
a declaration that looked authoritative and was causally inert.

The design that replaced it did not fix them one at a time. It made the causal connection an
**architectural property**: the compiled program relations are the thing the planner reads, so a
rule that is not in the program cannot execute, and a plan that did not come from the program cannot
run.

That is the general remedy. Do not verify that the declaration matches the behavior; arrange that
the behavior has no other source.

## Corollaries

- **Generated code that nothing imports is not generated code.** It is a build artifact with a
  plausible name.
- **A registry consumed only by the check that validates the registry is circular.** The consumer
  must be production, not the validator.
- **A configuration key with no reader is a defect, not a spare part.** Delete it; the next reader
  will assume it works.
- **A bypass route defeats the whole principle.** If a second path reaches execution without passing
  through the declaration, the declaration governs nothing. Seal the ingress or abandon the claim.

---

# P28 — Compute change; never declare it

Drift, staleness, difference, impact, and freshness are **functions over two states**, evaluated on
demand. They are never recorded values that someone must remember to update.

```text
WRONG                                    RIGHT
a changelog someone maintains            a diff computed between two versions
a version constant bumped by hand        an identity derived from content
a "last synchronized" timestamp          a re-derivation that compares
a stored digest of a live document       a digest recomputed at the point of use
a list of what a change affects          the affected set computed from the dependency graph
a status field on a work item            the status derived from what is committed
```

## Why the recorded form always fails

A recorded difference is correct at exactly one instant and decays silently thereafter. Worse, its
decay is invisible: the record still parses, still looks authoritative, and still answers questions
— with an answer that was true last month. There is no failure mode that surfaces it, which is why
`evidence-policy.md` §4 already requires that staleness be *derived, never compared by hand*.

A computed difference has the opposite property. It is correct whenever it is asked, it costs
nothing when not asked, and it cannot be wrong without the computation itself being wrong — which is
a defect a test can find.

## Consequences for design

- **Invalidation is computed from the dependency graph**, not from a hand-drawn impact list. "What
  does this change affect?" is a query, and if it cannot be one, the dependency structure is not
  modeled (P1).
- **Identity is derived from content**, so that a change to content necessarily changes identity and
  no one has to remember to bump anything (P18).
- **Where a digest must be recorded** — a pinned input, an acceptance record — it is recorded once,
  as a Class 1 fact about a specific past state, and thereafter **recomputed and compared**, never
  hand-edited to match. An artifact whose recorded digest no longer matches is a derivation result
  to act on, not a table to quietly correct.

## The worked example in this repository

The v1 principles document (§0.1) is pinned by eighteen recorded digests across twelve plans, four
reviews, and two release-gate artifacts. Every one of them is a Class 3 declaration of a Class 2
fact: the digest of a file is computable at any moment, from the file. The consequence is that a
one-line edit to a document is now an eighteen-site coordinated change, and the *only* thing
preventing silent inconsistency is that a check happens to recompute them.

That check is what makes the arrangement survivable. Without it, the eighteen copies would disagree
and nothing would say so. With it, the copies are a cache and the check is the authority — which is
the closure rule of §A.3, and the minimum acceptable form when a recorded copy cannot be avoided.

---

# P29 — Validate by relational query over the model, not by scanning text

An invariant is a **predicate over the typed model** that evaluates to the set of rows violating it,
each carrying enough provenance to locate the violation.

```text
WEAKEST   a regex over file paths; a count of matching files
          -> proves a string is present. Not that a property holds.

WEAK      a structural pattern match over syntax (ast-grep and equivalents)
          -> proves a shape occurs. Not that a relation holds across the system.

STRONG    a relational predicate over the compiled/typed model
          -> returns the violating rows, with provenance, exhaustively

STRONGEST a construction in which the violation is unrepresentable (P32)
          -> no query needed; there is no execution that violates it
```

## The rule

Express every invariant at the strongest tier its subject permits, and **label the tier**. A textual
probe is admissible where nothing stronger exists — cross-language residue, generated output,
configuration, comments, anything with no AST — but it must be recorded as the weak evidence it is,
never reported as if it established a property.

The failure being prevented is specific and common: a check that greps for a symbol name and passes,
reported as though it proved an architectural invariant. It proved that a string occurs somewhere. A
rename, a re-export, a macro expansion, or a second implementation defeats it silently, and its
zero-hit result is not proof of absence unless the coverage envelope is complete.

## Why relational is the target tier

This system compiles its semantics into a relational program and executes it over a query engine. An
invariant expressed as a predicate over that model therefore gets, for free, the properties that
make validation trustworthy:

- **Exhaustiveness** — the query ranges over every row, not over the files someone remembered to
  include. Coverage is a property of the model, not of the search path.
- **Provenance** — a violating row carries what produced it, from what input, at what version (P9),
  so a failure is actionable rather than a location to start investigating.
- **Composability** — invariants combine as relations. "Every X has a Y" and "no Y outside Z"
  compose; two greps do not.
- **Non-circularity** — the predicate reads the model that execution reads (P27), so it cannot pass
  by inspecting a representation execution ignores.

## The obligation this creates

A system that asks for relational validation must make its own structure queryable. If an invariant
cannot be expressed as a predicate because the model does not represent the thing being quantified
over, the finding is about the model (P1), not about the invariant. "We had to grep for it" is
evidence that something is missing from the model.

---

# P30 — Expectations are authored independently of the system under test

The system may never generate, approve, or rewrite the expectation against which it is judged.

```text
An expectation is authoritative for the intended behavior it states
and the independently observed behavior it is compared against.
It is authoritative for nothing else.
```

## What this excludes

- **Self-generated goldens.** Recording current output as the expected output proves only that the
  system is deterministic. It converts every existing bug into a permanent requirement, and the
  first person to fix one will be told they broke a test.
- **Expectations selected by runtime identifiers.** An expectation that names entities by identifiers
  the system minted is comparing the system to itself with extra steps. Select by reviewable source
  anchors and semantic attributes that a human authored and can check.
- **Blanket acceptance.** A recipe that regenerates and accepts expectations in bulk is a mechanism
  for erasing the entire signal in one command. Acceptance is per-change, reviewed, and
  attributable.
- **Comparators that share code with the system.** An independent evaluator that imports the
  production compiler, reconciler, provider adapters, or execution path is not independent; it will
  reproduce the same misunderstanding on both sides of the comparison and report agreement.

## What it requires

```text
the expected behavior is stated BEFORE execution
the fixture is small enough that a reviewer can reason about it directly
the claim cites the governing rule and a human-readable source anchor
a known injected fault MUST make the relevant claim fail
negative claims name the searched universe and the completeness state
   that makes absence meaningful
```

The fault-injection clause is the one that does the work. An expectation suite that has never been
shown to fail is not known to be an oracle at all (P25) — it may be asserting nothing. Deliberately
breaking the system and confirming that the right claim fails is the only evidence that the suite
discriminates.

## Where staged generation is allowed

Generation may **stage candidates** for review — proposing an expectation a human then reads,
corrects, and accepts. It may never **approve** one. The boundary is the human decision, and that
decision is a Class 1 declaration under P26: it carries its owner and its rationale, and it is
immutable evidence thereafter.

---

# P31 — Eliminate synchronization points that fail only by forgetting

If the sole failure mode of a control is that somebody forgot, it is not a control. It is a breakage
point that also produces red builds.

```text
forgot to regenerate after changing the source
forgot to add the new variant to the list
forgot to bump the version constant
forgot to re-run the digest after editing
forgot to add the traceability row
forgot to update the count in the description
```

Every one of these is a synchronization point between two representations that should have been one.

## Remedies, in order of preference

1. **Derive on read.** The second representation does not exist. There is nothing to forget.
2. **Compute membership.** Where a list must exist, it is the *result of a query* — "every type
   implementing this trait", "every registered domain" — not an enumeration. Adding a member cannot
   be forgotten, because nobody adds it to a list.
3. **Close the loop.** Where a materialized copy is genuinely required (§A.1 Class 2), a check
   regenerates it and compares. The copy is a cache; the check is the authority. This is the
   *minimum* acceptable form, not a good one — it still costs a regeneration step, it still fails
   late, and it still requires the check to exist.

## The cost this is trading against

Closure checks are not free. Each one adds a mutating recipe that must be run deliberately, a diff
that must be reviewed, and a class of build failure whose remedy is mechanical rather than
intellectual. Ten of them is a workflow; a hundred is a treadmill, and the response to a treadmill
is always to automate the acceptance, which destroys the check.

So the count matters. When a closure check is added, the question is not "is this check correct?"
but **"why does the second representation exist at all?"** — and the answer must name a real
constraint: a foreign toolchain, a wire contract, a derivation the checking environment cannot
perform. "It is convenient to have it committed" is not one.

## The diagnostic

To find these, ask of any build failure: **would a competent engineer, making a correct change, hit
this?** If yes, and the fix is mechanical, the check is protecting a synchronization point rather
than an invariant. It should be removed by eliminating what it synchronizes.

---

# P32 — Validate by construction

The strongest form of dynamic validation is a **constructor that cannot produce an invalid value**.
Where an invariant can be made unrepresentable, no runtime check is needed, no oracle can be
forgotten, and no violating state can reach the code that would have to handle it.

## Parse, don't validate

Messy input is converted into a structured, typed representation **once, at the boundary**.
Validation occurs by successful construction of a well-formed value, not by scattered revalidation
at every subsequent use.

```text
WRONG   accept the loose form; check it here; check it again there; hope
        every path checked
RIGHT   parse at the boundary into a type that cannot be constructed
        wrongly; every downstream path receives a value already known good
```

The second form is also the only one whose coverage is provable: there is no "every path" to
enumerate, because the type is the path.

## Illegal states are unrepresentable

Data models prevent impossible combinations structurally wherever practical. Required fields,
mutually exclusive modes, legal state transitions, and bounded variant sets are expressed in types
rather than in comments and scattered conditionals.

```text
a bounded, closed variant set        rather than a string with known values
a state machine as a type            rather than a status field plus rules
mutually exclusive fields as a sum   rather than two options and an invariant
a non-empty collection type          rather than a length check at each use
a validated newtype                  rather than a raw string plus a convention
```

## Exhaustiveness is the mechanism

A closed variant set is only load-bearing if the compiler forces every consumer to handle every
case. Where the language provides exhaustive matching, use it and never add a catch-all arm that
silently absorbs future variants — a wildcard converts a compile error, which finds every consumer,
into a runtime surprise, which finds one.

This is the highest tier of P29's ladder, and it is the reason to reach for it: an invariant
enforced by construction has an oracle that runs on every build, over every consumer, with complete
coverage, and it cannot be forgotten (P31).

---

# P33 — Functional core, imperative shell

Deterministic transformations — semantic rewrites, canonicalization, compilation, projection
generation, derivations — form a **pure functional core**. IO, orchestration, retries, cancellation,
storage, transport, and process management form an **imperative shell** around it.

```text
SHELL   watch, read, schedule, retry, cancel, commit, serve, log
CORE    parse, normalize, compile, analyze, derive, plan, project
```

## Why this is a v2 principle rather than a style preference

Every proof obligation in this document assumes re-execution is cheap. P17 requires reconstruction
rather than preservation; P19 proves determinism by executing twice; P23 validates a cache by
re-deriving it; P28 computes difference between states; P29 evaluates predicates over the model.

All of that is affordable exactly to the extent that the computation is **pure** — no IO, no clock,
no ambient state, no ordering dependence on things outside its inputs. A core function can be run a
thousand times in a test, on a fixture, in a comparison, inside a property check. A computation
entangled with IO can be run once, carefully, in an environment someone had to set up.

So this principle is not about elegance. It is the precondition that makes execution-based
validation practical rather than aspirational.

## The rules that follow

- The core takes its inputs explicitly. No hidden reads of clock, environment, filesystem, or global
  state — each of those is an unpinned input (P19) masquerading as a constant.
- The core is deterministic. Given the same inputs it produces the same outputs, including the same
  ordering, which is what makes comparison meaningful.
- The shell owns everything non-reproducible and keeps it thin, because the shell is the part that
  cannot be tested by re-execution.
- Where the core must express a failure, it returns it as a typed value (P16) rather than raising
  through the shell's error machinery.

---

# P34 — One mutation path; commands idempotent and replayable

All authored change passes through **one semantic command model**. Every entry point — an API call,
an administrative command, an import, a batch operation, an agent action — resolves to the same
mutation path, which owns validation, authorization, provenance, invalidation, and replay.

## No second route

A second write path is not a convenience; it is a hole in every guarantee attached to the first one.
Whatever the primary path enforces — authorization (P13), provenance (P9), invalidation (P28),
contract validation (P12) — the secondary path does not, and the secondary path is the one that will
be used when the primary is inconvenient.

This includes routes that exist only for testing. A test-only mutation route that reaches production
state is a production route with a misleading name, and per P27 it defeats the declaration it
bypasses.

## Command-query separation

Operations that mutate truth and operations that read it stay distinct. Queries produce no hidden
side effects; commands carry explicit mutation intent. A read that mutates — a lazy initialization,
an access-time update, a cache fill that changes observable state — makes P29's validation
unreliable, because observing the system changes it.

## Idempotency

Re-running a command, a compilation step, a recovery action, or a publication with the same
effective inputs must not corrupt state or produce an inconsistent outcome. Idempotency is a
first-class property of recovery, retry, and incremental workflows — not an optimization.

The reason is that failure recovery cannot be avoided, only handled. A process dies between the
write and the acknowledgment; the caller retries; the question is whether the system converges.
Where an operation cannot be made idempotent, its outcome must be **reconcilable**: an interrupted
operation whose result is unknown must be resolvable by inspecting durable state, not by guessing.

Commands are therefore identified, so that a retry is recognizable as a retry rather than as a
second intent.

---

# P35 — Inward, acyclic dependency structure

The most semantically important logic depends on the fewest details. Platform edges — storage,
transport, compilers, external tools, presentation — depend **inward** on the semantic core, never
the reverse.

```text
edges/adapters  ---depend on--->  ports  ---defined by--->  semantic core
```

The core stays technology-agnostic; runtime details stay replaceable at the edge.

## Acyclicity

Module, package, and artifact dependency graphs are intentionally layered and **acyclic**. A cycle
is a design defect, not normal friction: it means two things that claim to be separable are not, and
it makes both untestable in isolation and unreplaceable independently.

Cycles are detected by a check over the dependency graph, not by review (P36). Layering violations
are the same class of finding.

## The module hygiene that supports it

Absorbed from the retired doctrine and retained because it is what makes the above achievable rather
than aspirational:

- **Information hiding.** Each module owns a bounded set of internal decisions and exposes only
  stable, intention-revealing surfaces. Internal layout, vendor quirks, wire formats, caching
  strategy, and algorithm selection stay local. Abstractions must hide real complexity, not rename
  it.
- **Separation of concerns.** Domain rules, infrastructure, orchestration, and presentation remain
  distinguishable and separately governable. Semantic rules must be readable without traversing
  infrastructure, and infrastructure must be replaceable without rewriting semantics. Parsing, IO,
  and orchestration are not homes for semantic logic.
- **Single responsibility.** Every component admits a primary reason to change that can be stated
  without a conjunction.
- **High cohesion, low coupling.** Related concepts live together; unrelated concepts do not share a
  home for convenience. Communication is through narrow, intention-revealing interfaces rather than
  deep structural knowledge.

## The boundary this system already enforces

Process and build isolation is the strongest available form of this principle, because it is
enforced by the toolchain rather than by discipline: a dependency that would violate it cannot
compile. Where such a boundary exists, it is the authority, and no convenience is sufficient reason
to weaken it.

---

# P36 — Governance is executable

Doctrine violations are detected by **checks that run**, not by review that remembers.

> **A principle nothing can detect is an aspiration, not doctrine.**

## The obligation

Every principle in this document that constrains the implementation must be answerable by an
executable check, or must be explicitly recorded as **currently unenforced**. Both are acceptable
positions. What is not acceptable is a principle that is treated as binding while nothing can tell
whether it holds — because that produces confident claims of conformance with no evidence behind
them, which is worse than an acknowledged gap.

The distinction to maintain, and to record:

```text
ENFORCED        a named executable oracle proves it, and fails when it is violated
BY-CONVENTION   it holds in the code today, and nothing prevents its violation tomorrow
UNENFORCED      no check exists; conformance is unknown
```

A `by-convention` finding is not a failure. It is an accurate description of the strongest thing
currently true, and it is exactly the input needed to decide whether enforcement is worth building.

## Escalation

A violation that recurs is promoted: from a review comment, to a structural governance rule with
fixtures, to a gate. The same rule applies to invariants — one asserted more than once by hand is
promoted to a check and thereafter cited by name. Re-deriving the same invariant manually in a second
artifact is a defect, not diligence.

## The check on the checks

Executable governance has its own failure mode, and it is the one this document exists to address: a
check that runs, passes, and proves nothing. Detectors that count files, match strings, or compare
the system against its own output satisfy the letter of "executable governance" while providing no
signal (P25, P29, P30).

So the governance layer is subject to its own principles. Each check names what it would catch, and
per P30 a check that has never been demonstrated to fail on a real violation is not known to work.
The strongest form of the demonstration is a **negative fixture**: a deliberate violation, committed
as a test input, which the check must reject.

---

# Y1. The overall architectural pattern

```text
                 SEMANTIC AUTHORITY
                       |
                       v
               explicit typed models
                       |
        +--------------+---------------+
        |              |               |
        v              v               v
     schemas       calculations       plans
        |              |               |
        +--------------+---------------+
                       v
              contract validation          <- executable oracles (P25)
                       v
               catalog resolution
                       v
               logical compilation         <- staged, contracted passes (P16)
                       v
                  optimization
                       v
               physical execution
                       v
              canonical Arrow data
                       v
          transactional persistence
                       v
               versioned state             <- Class 1: legitimately immutable (P26)
                       |
                       v
            provenance + diagnostics       <- emitted, not maintained (P9)
                       |
                       v
           relational validation queries   <- invariants as predicates (P29)
```

The design goal is not maximum abstraction. It is **maximum semantic coherence with minimum
duplication of authority** — and, in v2, minimum declaration of anything the system can compute
about itself.

---

# Y2. Mandatory design questions

Before implementing or materially revising a subsystem, answer the following. If several have no
clear answer, stop at the design stage.

| Question | Required design outcome |
|---|---|
| What semantic concept is being represented? | Explicit model, or an explanation why one is unnecessary |
| What is the authoritative representation? | One clearly named authority |
| What derived representations exist? | Explicit derivation relationships |
| What is invariant? | Machine-testable contracts |
| What may implementations vary? | Explicit extension points |
| What hierarchy does this concept belong to? | Parent/child responsibilities |
| What lifecycle phases does it pass through? | Phase boundaries and artifacts |
| What is logical vs physical? | Semantic and implementation concerns separated |
| What common fabric type should cross boundaries? | Canonical representation rather than bespoke DTO |
| How is provenance captured? | Automatic lineage/version/operation identity, emitted by the operation |
| How is identity/versioning represented? | IDs, versions, content-derived fingerprints |
| Where is policy enforced? | Authority-bound enforcement point, with a denied-case test |
| How are capabilities advertised? | Explicit, conservative, prover-backed |
| How is drift detected? | A computation over two states, not a recorded value |
| How is the operation explained later? | Reconstructible artifacts and diagnostics |
| Can an existing higher-level abstraction express it? | Reuse before lower-level extension |
| What proves the contract? | A named executable oracle per clause |
| **Which declarations here are Class 1, and why?** | **A staticness classification per declared artifact (§A.1)** |
| **What breaks if each declaration is wrong?** | **Causal dependence, or deletion (P27)** |
| **What would make each oracle fail?** | **A named falsifying change; otherwise the oracle is vacuous (P25)** |
| **What must a human remember for this to stay correct?** | **Nothing. If something, name the closure check (P31)** |
| **At what tier is each invariant checked?** | **Construction > relational > structural > textual, labelled (P29)** |

---

# Y3. Anti-patterns to actively reject

### Hidden semantic logic

Domain meaning exists primarily in procedural branches.
**Replace with:** explicit semantic models compiled into behavior.

### Multiple authorities

Schema defined in a Rust struct, a migration, a config file, and writer code independently.
**Replace with:** one contract with generated or derived representations.

### Backend leakage

Every consumer branches on the backend in use.
**Replace with:** a provider abstraction exposing a common contract.

### Opaque abstraction

Transparent expression logic wrapped in an opaque function for code organization.
**Replace with:** reusable expression builders that retain optimizer *and validator* visibility.

### Premature physicalization

The model hard-codes partitioning, join algorithm, concurrency, or storage mechanism.
**Replace with:** logical requirement first, physical planning later.

### Provenance afterthought

Only application logs explain where data came from.
**Replace with:** provenance emitted by the operation, in the same transaction.

### Mutable authority

Shared authoritative objects silently changed by arbitrary callers.
**Replace with:** controlled state transitions and versioned snapshots.

### Metadata theater

A metadata tag claims an invariant the runtime does not enforce.
**Replace with:** an enforcer, or an honest reclassification to advisory.

### Pairwise integration explosion

A-B, A-C, B-C each get custom data structures.
**Replace with:** a canonical fabric or protocol boundary.

### Capability overclaiming

A provider claims pushdown, statistics, or ordering it cannot guarantee.
**Replace with:** conservative reporting; unknown until proved.

### Declaration theater *(new in v2)*

A registry, manifest, or metadata table that looks authoritative and that execution never reads.
**Replace with:** wire it into the execution path, or delete it. Verify by changing it and observing
that behavior changes (P27).

### The regenerate-and-commit treadmill *(new in v2)*

A committed artifact whose only purpose is to be compared against its own regeneration, multiplied
until the workflow is dominated by mechanical resynchronization.
**Replace with:** derivation on read, or computed membership. Where a copy is unavoidable, name the
constraint that makes it unavoidable (P31).

### The hand-maintained ledger *(new in v2)*

A table of dispositions, statuses, consumers, or conformance results updated by a human.
**Replace with:** a query over the model. The ledger's rows are already facts the system holds
(P26, P29).

### Self-authored expectation *(new in v2)*

The system generates the golden it is then judged against, or a comparator that shares code with the
system under test.
**Replace with:** independently authored claims, plus a fault-injection demonstration that the suite
can fail (P30).

### The textual probe standing in for a proof *(new in v2)*

A regex or file count reported as though it established an architectural property.
**Replace with:** a relational predicate over the model; where impossible, keep the probe and label
it as weak evidence with its coverage envelope (P29).

### The frozen enumeration *(new in v2)*

A hand-written list of things that exist — variants, modules, consumers, contracts — that must be
extended whenever a member is added.
**Replace with:** computed membership. If the list must exist, it is a query result (P31).

### Vacuous oracle *(new in v2)*

A check that cannot fail: a zero-match selector that passes, an existence assertion, a count
comparison, an unconditional success.
**Replace with:** an oracle with a named falsifying change, demonstrated by a negative fixture
(P25, P36).

---

# Y4. Secondary review constraints

Normative for code and architecture review, though they do not define platform topology.

**Prefer composition over inheritance.** Behavior assembly defaults to composition unless
inheritance provides a clear, bounded, and stable semantic fit.

**Law of Demeter.** Interact with direct collaborators rather than depending on deep structural
reach-through.

**Tell, don't ask.** Behavior and invariants stay close to the abstractions that own them rather
than being recreated through external raw-data inspection.

**KISS.** Solutions stay as simple as possible without sacrificing required boundaries, determinism,
or inspectability.

**YAGNI.** Abstraction layers are not introduced without a clear near-term second use case or a
clearly identified change vector.

**Principle of least astonishment.** APIs, schemas, commands, and runtime behaviors align with a
competent reader's expectation and minimize surprising side effects.

**Conceptual integrity.** The same concept carries the same name across the system, and the same
name does not represent different concepts in different subsystems unless an explicit translation
boundary exists.

---

# Y5. Compact agent design constitution

Suitable for direct inclusion in an agent's design instructions.

> **Architecture objective.** Design the system as a model-first, contract-driven,
> provenance-native, execution-proved data fabric. Important semantics exist as explicit typed
> models that are validated and then compiled into execution, rather than embedded in procedural
> control flow.
>
> **Staticness.** Write down only what cannot change. A declaration is admissible only where its
> referent is immutable by construction — a completed event, a released version, a pinned revision,
> an accountable human decision, an independently authored expectation. Everything else is computed
> from the authority at the moment it is needed. Before declaring anything, ask whether the thing it
> describes can change without it changing; if so, it is a cache with no invalidation.
>
> **Causality.** Every declaration must drive the execution it describes. Change the declaration and
> observable behavior must change. A declaration execution does not read is deleted, not documented.
>
> **Change.** Drift, staleness, difference and impact are computed between two states on demand,
> never recorded values someone maintains. Identity derives from content so that changing content
> necessarily changes identity.
>
> **Validation.** Express every invariant at the strongest tier its subject permits — unrepresentable
> by construction, else a relational predicate over the typed model, else a structural match, else a
> textual probe — and label the tier. A textual zero-hit is not proof of absence.
>
> **Oracles.** Every contract clause names the executable that decides it. A clause with no oracle is
> not a contract. An oracle that cannot fail is not an oracle: name the change that would falsify it,
> and prove it with a negative fixture.
>
> **Independence.** The system never generates, approves, or rewrites the expectation it is judged
> against, and a comparator never shares code with the system under test.
>
> **Friction.** A control whose only failure mode is that somebody forgot is a breakage point.
> Eliminate it by deriving on read, or by computing membership. Where a materialized copy is
> unavoidable, name the constraint that makes it so and close the loop with a regeneration check.
>
> **Authority.** One authoritative owner per semantic concept. Other forms are derived
> representations tied to the authority by stable identity, and staleness is detected by
> re-derivation, never by consulting a stored marker.
>
> **Contracts and hierarchy.** Define shared invariants separately from legal variation; encode them
> as typed interfaces and validate at boundaries. Consumers depend on the common contract rather than
> branching on implementation type.
>
> **Model vs execution.** Separate logical meaning from physical strategy. Canonicalize before
> optimizing.
>
> **Common fabric.** Reuse canonical representations across boundaries; prefer standard
> interoperability protocols over pairwise conversion.
>
> **Extensibility.** Use the highest-level abstraction that fully preserves the required semantics.
> New capability enters as added semantics, passes, projections, or adapters — not as edits to the
> core.
>
> **Construction.** Parse at the boundary into types that cannot be constructed wrongly; make
> illegal states unrepresentable; keep variant sets closed and matches exhaustive.
>
> **Core and shell.** Keep deterministic transformation pure and push IO, retries, and orchestration
> to a thin shell, so that re-execution stays cheap enough to be the default proof.
>
> **Mutation.** One semantic command path owns validation, authorization, provenance, and
> invalidation. No second write route, including test-only ones. Commands are identified and
> idempotent; interrupted outcomes are reconcilable from durable state.
>
> **Provenance.** Emitted by the operation that produces the artifact, in the same transaction.
> Closure is proved by a resolver that walks the chain and fails on a broken link.
>
> **State.** Prefer immutable versioned snapshots and explicit transitions. Distinguish durable
> domain truth from temporal control truth. A cache declares what it derives from and is validated by
> re-derivation.
>
> **Truthfulness.** A capability is unknown until an executable prover confirms it. Never advertise
> pushdown, ordering, statistics, constraints, determinism or idempotency without a prover. Absence
> of a provider result is an explicit unknown, never an empty result.
>
> **Governance.** Enforce policy at the layer owning the relevant contract, and prove it with the
> denied case. Keep authority narrow and fail closed.
>
> **Reproducibility.** Prove it by executing twice and comparing. Reproducibility status is an output
> of a check, not an author's assertion.
>
> **Dependencies.** Edges depend inward on the semantic core. Dependency graphs are acyclic, and
> cycles are found by a check rather than by review.
>
> **Executable governance.** A principle nothing can detect is an aspiration. Record each as
> enforced, by-convention, or unenforced — and never claim conformance without the oracle that
> establishes it.
>
> **Design review requirement.** Before coding, state the semantic model, authoritative owner,
> hierarchy, invariants, legal variation, lifecycle phases, boundary types, provenance model, policy
> enforcement point, extension level, staticness classification of every declared artifact, and the
> executable oracle for every contract clause. If these cannot be stated clearly, continue designing.

---

# Y6. Short form

> **Represent meaning explicitly, assign each meaning a single authority, compose the system through
> typed hierarchical contracts and common canonical representations, separate semantics from
> execution, and make every state transformation versioned, inspectable, reproducible and
> provenance-complete — while declaring only what cannot change and computing everything else by
> execution.**

Two maxims:

> **Model the truth once; derive behavior from it; preserve its lineage everywhere.**

> **Write down only what cannot change; compute everything else, every time you need it.**

---

# Y7. v1 to v2 delta

For migrating citations and derived material. `DF-Pn` identifiers are stable: no principle changed
number, and no number was reused.

## Retitled — five

| ID | v1 title | v2 title |
|---|---|---|
| P17 | Make intermediate artifacts inspectable and reproducible | **Make intermediate artifacts reconstructible by re-execution** |
| P18 | Fingerprint anything whose identity matters | **Fingerprint for identity, never for correctness** |
| P19 | Make reproducibility a normal operating mode | **Prove reproducibility by re-execution** |
| P20 | Be conservative about claimed capabilities | **Advertise only capabilities an executable prover confirms** |
| P25 | Make testing derive from contracts and invariants | **Every contract clause names its executable oracle** |

## Materially revised, title unchanged — fourteen

| ID | What changed |
|---|---|
| P1 | Adds the causal test: the model must be what execution reads |
| P3 | Staleness is answered by re-derivation, not a stored marker; presentation is a projection |
| P5 | Adds ports and adapters; adds the generic-runtime consequence |
| P6 | Adds canonicalization before optimization; adds platform-independence |
| P9 | Provenance is emitted by the operation, never maintained separately |
| P10 | Closure is proved by a resolver that runs, not by an assertion of resolvability |
| P11 | Adds durable domain truth vs temporal control truth; names committed state as Class 1 |
| P12 | Fingerprints are recomputed, not trusted; generalized to boundary contracts (`H-P16` successor) |
| P13 | Adds least privilege; enforcement must execute, proved by the denied case |
| P14 | Adds additive extensibility |
| P15 | Generalized from optimizer visibility to validator visibility |
| P16 | Adds staged compilation and the pass contract (`H-P14` successor); adds the failure taxonomy |
| P21 | The metadata class is discovered by whether an enforcer exists, not declared |
| P23 | Cache validity established by re-derivation, not by a declared invalidation policy |

## Carried forward substantially unchanged — six

`P2`, `P4`, `P7`, `P8`, `P22`, `P24`.

`P2` is unchanged in substance but is promoted in §0.2 as the origin of the v2 thesis.

## New — eleven

| ID | Title | Origin |
|---|---|---|
| P26 | Declare only what cannot change | new |
| P27 | Every declaration must be causally load-bearing | new |
| P28 | Compute change; never declare it | new |
| P29 | Validate by relational query over the model, not by scanning text | new |
| P30 | Expectations are authored independently of the system under test | new |
| P31 | Eliminate synchronization points that fail only by forgetting | new |
| P32 | Validate by construction | absorbed SD-P11, SD-P12 |
| P33 | Functional core, imperative shell | absorbed SD-P17 |
| P34 | One mutation path; commands idempotent and replayable | absorbed SD-P20, SD-P21, SD-P24 |
| P35 | Inward, acyclic dependency structure | absorbed SD-P1..P5, SD-P7 |
| P36 | Governance is executable | absorbed SD-P31 |

## Structural changes

- Heading scheme is `# P{n} — Title`; the v1 `N = M + 1` ordinal offset is gone.
- §A (the staticness test) and §B (reading and migration) are new front matter.
- v1 §27-§31 become Y1-Y6; Y4 is new, absorbed from the retired doctrine's §8.
- Six anti-patterns added to Y3; five questions added to Y2.

## What this obligates elsewhere

v2 does not migrate anything by itself. Anything binding to the v1 numbering must be updated
deliberately, and the principle count is no longer 25 — code asserting an exact `P1`–`P25` set will
reject a v2-aligned registry. The `["H-P14", "H-P16"]` literals described in §B.3 must be remapped
together with any removal of the holistic doctrine.

Per P36, the honest position during migration is to record each principle as enforced,
by-convention, or unenforced, rather than to assert conformance that no oracle establishes.
