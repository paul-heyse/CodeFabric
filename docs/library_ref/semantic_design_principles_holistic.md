Below is an integrated draft suitable as the updated **semantic_design_principles** document. It preserves the semantic-compiled architecture doctrine already established in the attached semantic principles document, and incorporates the additive structural, contractual, runtime, lifecycle, observability, and security-oriented principles from the attached general design-principles document. It also retains alignment with the external architecture families previously used to frame the doctrine: OMG Model Driven Architecture for platform-independent versus platform-specific realization, SysML v2 / MBSE for lifecycle traceability and systems-model interoperability, compiler-style multi-IR architecture for staged lowering, ports-and-adapters for boundary isolation, provenance for trust and traceability, and hermeticity for reproducible execution.   ([OMG][1])

# Integrated Semantic-Compiled Platform Design Principles

## 1. Purpose

This document defines the governing design principles for a semantic-compiled engineering platform intended to support broad classes of customized analysis without repeated reinvention of execution infrastructure, UI surfaces, data-handling mechanisms, or domain-specific orchestration.

The document has two purposes. First, it defines the semantic-compiled platform architecture itself: the authored semantic model, the staged compilation pipeline, the generic runtime, the reusable control plane, the projection-oriented workbench, and the governance model that makes the platform inspectable and evolvable. Second, it defines the structural and operational disciplines necessary to keep that platform implementable in practice: modular decomposition, explicit contracts, narrow authority, lifecycle ownership, observability, and testability.  

## 2. Scope

This doctrine applies to:

* authored semantic models;
* reference datasets and environment bindings;
* intermediate representations;
* compiler and projection passes;
* runtime orchestration and control-plane protocols;
* UI/workbench projections;
* storage, solver, import, and publication boundaries;
* audit, provenance, observability, and governance mechanisms.

This doctrine does not prescribe a single programming language, solver, UI framework, storage technology, or deployment topology. Those choices belong to platform-specific realization, provided that they remain subordinate to the semantic model and to the boundary rules defined here. 

## 3. Foundational Posture

A semantic-compiled engineering platform is a platform in which authored semantics are the normative description of the problem instance, compilation normalizes and lowers those semantics through governed artifact stages, a bounded runtime executes compiled forms through reusable protocols, the workbench renders compiled semantic projections rather than maintaining a separate truth, and provenance plus reproducibility make outputs inspectable, repeatable, and governable. 

This posture remains consistent with MDA’s separation of platform-independent and platform-specific realization, with SysML’s role in specifying, analyzing, designing, and verifying complex systems, with compiler-style multi-level intermediate representations, with ports-and-adapters isolation of technology edges, and with provenance and hermeticity as trust disciplines. ([OMG][1])

## 4. Definitions

**Semantic case model**: the authored, platform-independent description of a problem instance, including topology, assumptions, scenario deltas, objectives, capabilities, and publication intent.

**Reference context**: the declared external knowledge bound to a case, including datasets, standards, schemas, libraries, and environment assumptions.

**Normalized semantic IR**: a canonicalized representation in which aliases are resolved, defaults are materialized, units are normalized, and stable semantic identities are assigned.

**Execution IR**: the representation used to drive runtime tools and solvers, including constraint graphs, tool graphs, optimizer variables, and protocol selections.

**Projection IR**: the representation used to generate tables, inspectors, results panes, comparisons, publications, and workbench views.

**Control plane**: the reusable protocol layer for validation, execution, cancellation, resume, publish/review, and batch/sweep workflows.

**Semantic identity**: the durable identifier assigned to a domain concept such as a unit, stream, assumption, constraint, scenario, artifact, or workflow instance.

**Pass contract**: the declared specification of a transformation step, including inputs, outputs, preserved identities, generated identities, diagnostics, invalidation effects, and test obligations.

**Semantic command**: the normative representation of an authored change, independent of whether the change originated from a table edit, inspector, script, batch action, import, or agent.

## 5. Required Artifact Stack

The platform shall maintain the following artifact classes:

1. **Semantic Case Model**
   The normative authored artifact.

2. **Reference Context**
   The declared external knowledge bound to the case.

3. **Normalized Semantic IR**
   The canonical semantic representation used as the stable internal input to lowering and execution.

4. **Execution IR**
   The runtime-ready representation for solver, tool, and orchestration execution.

5. **Projection IR**
   The representation used to generate interactive workbench views and publication outputs.

6. **Publication Manifest**
   The declared set of artifacts, reports, and surfaces to be emitted.

7. **Provenance Graph**
   The trace record linking entities, activities, and agents involved in producing each derived artifact.

8. **Command / Scenario Delta Log**
   The durable record of authored changes expressed as semantic commands rather than as UI gestures.

This artifact stack is the minimum closure required to keep authored semantics, normalization, execution, and presentation distinct. 

## 6. Required Pass Contract

Each compile or projection pass shall declare:

* purpose;
* input artifact types;
* output artifact types;
* required invariants on entry;
* established invariants on exit;
* preserved semantic identities;
* newly generated semantic identities;
* invalidation effects on downstream artifacts;
* diagnostics and failure classes;
* determinism expectations;
* test obligations and golden fixtures.

A pass without an explicit contract shall be treated as an ungoverned transformation and therefore as architectural debt. This requirement reflects the compiler-style discipline that transformations occur over explicit intermediate forms, not over opaque runtime side effects.  ([mlir.llvm.org][2])

## 7. Normative Design Principles

### A. Structural and Boundary Principles

#### Principle 1 — Information Hiding

Each module, service, package, or subsystem shall own a bounded set of internal decisions and shall expose only stable, intention-revealing surfaces. Internal data layout, vendor quirks, wire formats, caching strategy, algorithm selection, and framework-specific mechanics shall remain local to the owner of those concerns. External callers shall not depend on those internals.

**Minimum implications**

* Blast radius of internal change shall remain local.
* Internal representation changes shall not force wide interface churn.
* Abstractions shall hide real complexity, not rename it.

#### Principle 2 — Separation of Concerns

Policy, domain rules, semantic definitions, infrastructure, orchestration, logging, parsing, metrics, and UI glue shall remain distinguishable and separately governable concerns. Semantic rules shall remain readable without traversing infrastructure mechanisms, and infrastructure mechanisms shall remain replaceable without rewriting semantic rules.

**Minimum implications**

* Domain rules and platform mechanics shall not be co-located by convenience alone.
* Parsing, IO, and orchestration code shall not become the home of semantic logic.
* View logic shall not become the home of business truth.

#### Principle 3 — Single Responsibility

Every component shall admit a primary reason to change that can be stated without conjunction. Mixed-purpose components that accumulate unrelated responsibilities shall be treated as design defects.

**Minimum implications**

* Artifact schemas, passes, adapters, runtime services, and workbench controllers shall each have bounded reasons to evolve.
* “Utility” components that become dumping grounds shall be refactored.

#### Principle 4 — High Cohesion and Low Coupling

Related concepts shall live together, and unrelated concepts shall not share a home merely because the arrangement is convenient. Communication among components shall occur through narrow, intention-revealing interfaces rather than through deep structural knowledge.

**Minimum implications**

* Components shall be understandable in relative isolation.
* Cross-component dependency shall be justified by domain need, not by incidental access.

#### Principle 5 — Dependency Direction

The most semantically important logic shall depend on the fewest details. Platform edges such as storage, solver integration, framework code, UI, and external import/export mechanisms shall depend inward on semantic core abstractions, not the reverse.

**Minimum implications**

* Core semantics shall remain technology-agnostic.
* Runtime details shall remain replaceable at the edge.

#### Principle 6 — Ports and Adapters

The core shall express its needs through explicit ports, and technology-specific mechanisms shall exist as adapters implementing those ports. Solvers, storage engines, workbook importers, GUI frameworks, CLI surfaces, and automation endpoints are all adapter concerns rather than core concerns.  ([alistair.cockburn.us][3])

**Minimum implications**

* Boundary interfaces shall be explicit.
* Tests shall be able to substitute fake or alternate adapters with minimal friction.
* External systems shall not infect internal semantic types.

#### Principle 7 — Acyclic Dependency Structure

Module, package, service, and artifact dependency graphs shall remain intentionally layered and acyclic. Cycles shall be treated as design defects rather than normal friction.

**Minimum implications**

* Layering violations shall be detectable through governance checks.
* Cycles shall require explicit remediation rather than local workarounds.

#### Principle 8 — Trust Boundaries and Least Privilege

Authority, trust, and privilege shall be explicit architectural properties. The platform shall distinguish untrusted inputs, privileged operations, sensitive data, and trusted internal artifacts. Authority shall be narrow, centralized where necessary, and auditable.

**Minimum implications**

* Imported files, user-provided data, external datasets, and agent actions shall be treated according to declared trust boundaries.
* Sensitive operations such as publication, external write, privileged execution, or credentialed access shall not be ambiently available.
* The blast radius of bugs and malformed inputs shall be reduced through narrow authority.

### B. Semantic Model and Compilation Principles

#### Principle 9 — Platform-Independent Semantics

The semantic case model shall be the normative artifact of the platform. Platform details shall not be embedded in authored semantics except through declared bindings or capabilities. This is the MDA-aligned top-level separation between stable model and platform realization.  ([OMG][1])

#### Principle 10 — Declarative Knowledge Single-Sourcing

Mappings, constants, rule tables, default assumptions, naming crosswalks, and projection rules shall be declarative and single-sourced. Semantic duplication across runtime code, UI code, adapters, and tests shall be treated as a governance defect.

#### Principle 11 — Parse, Don’t Validate

Messy inputs shall be converted into structured semantic representations at the boundary. Validation shall occur primarily by successful construction of well-formed types and artifacts rather than by scattered revalidation throughout the system.

#### Principle 12 — Illegal States Shall Be Unrepresentable

Data models shall prevent impossible combinations by construction wherever practical. Required fields, mutually exclusive modes, legal state transitions, and bounded variant sets shall be expressed structurally rather than by comments and scattered checks.

#### Principle 13 — Stable Semantic Identity

Every durable domain concept shall possess a stable semantic identity that survives reordering, projection, caching, persistence, and publication. Row numbers, sheet coordinates, widget paths, and solver-local variable positions shall not act as primary identity. 

#### Principle 14 — Staged Compilation

Execution shall occur through staged compilation across distinct artifact forms rather than through direct interpretation of raw authored semantics. At minimum, normalization shall be separated from execution-oriented lowering, and projection generation shall be separated from execution lowering.

#### Principle 15 — Canonicalization Before Optimization

Alias resolution, unit normalization, default materialization, topology normalization, and equivalent-expression normalization shall precede optimizer-specific lowering. Equivalent authored cases shall converge toward equivalent normalized IR.

#### Principle 16 — Design by Contract

Contracts shall exist not only for compiler passes, but also for public adapters, semantic commands, runtime services, projection builders, control-plane protocols, and publication surfaces. Preconditions, postconditions, and invariants shall be explicit at every stable boundary. 

#### Principle 17 — Functional Core, Imperative Shell

Deterministic transformations, semantic rewrites, canonicalization, projection generation, and pure derivations shall form the functional core. IO, orchestration, retries, cancellation, UI event handling, storage, and framework interactions shall form the imperative shell. This division is a primary mechanism for testability, reproducibility, and disciplined side effects. 

### C. Runtime, Mutation, and Control Principles

#### Principle 18 — Generic Runtime and Reusable Control Plane

The runtime shall execute classes of compiled cases rather than ad hoc simulation-specific execution branches. Validation, execution, cancellation, resume, publish/review, and batch/sweep behaviors shall derive from a reusable control plane configured by case semantics. 

#### Principle 19 — Durable Domain Truth, Temporal Control Truth

Assumptions, topology, constraints, artifacts, and scenario deltas are durable domain truth. Execution state, retry state, cancellation state, suspension, and publish workflow are temporal control truth. The two shall remain distinct.

#### Principle 20 — Unified Mutation and Transaction Semantics

All authored change shall pass through one semantic command model. Table edits, inspector edits, imports, scripts, agents, and batch changes shall all resolve to the same semantic mutation path, owning validation, provenance, invalidation, and replay semantics. 

#### Principle 21 — Command–Query Separation

Operations that mutate platform truth and operations that read platform truth shall remain distinct. Queries shall not produce hidden side effects; commands shall carry explicit mutation intent.

#### Principle 22 — Ownership and Lifecycle of State and Resources

Every mutable thing or scarce resource shall have a clear owner and explicit lifecycle: creation, use, handoff, shutdown, cleanup, and disposal. This includes caches, workers, transactions, temp artifacts, locks, sessions, handles, and live runtime graphs. 

#### Principle 23 — Explicit Failure Semantics

The platform shall distinguish semantic authoring failures, validation failures, reference-resolution failures, compile failures, capability mismatches, solver infeasibility, transient infrastructure failures, programmer defects, cancellation, timeout, and publication failures. Failures shall not be flattened into a generic “run failed” category.  

#### Principle 24 — Idempotency as a Design Goal

Re-running a semantic command, compilation step, projection step, recovery action, or publication step with the same effective inputs shall not corrupt platform state or produce inconsistent outcomes. Idempotency shall be treated as a first-class property of recovery, retry, and incremental workflows. 

#### Principle 25 — Reproducibility, Hermeticity, and Semantic Incrementality

Compilation and execution shall be reproducible from declared inputs, and incrementality shall be driven by declared semantic dependencies rather than by ambient machine state or scattered dirty flags. Cache keys shall incorporate semantic inputs, reference-context versions, and pass/tool versions.   ([Bazel][4])

### D. Workbench, Contracts, and Governance Principles

#### Principle 26 — UI as Compiled Semantic Projection

The workbench shall render compiled semantic projections rather than maintaining a second truth in hand-built simulation-specific UI structures. Editors, inspectors, comparisons, tables, result panes, and publication views shall be projections of semantic artifacts. 

#### Principle 27 — Provenance and Explainability

Every significant derived artifact shall carry inspectable provenance, and every significant result shall be explainable in terms of upstream semantics, reference context, passes, runtime actions, and publication steps.  ([W3C][5])

#### Principle 28 — Observability as Consistent, Structured Data

Logs, metrics, traces, and diagnostic records shall form a coherent, structured operational narrative aligned to artifact boundaries, pass boundaries, runtime phases, and failure modes. Observability and provenance are complementary but distinct: provenance explains derivation; observability explains execution.

#### Principle 29 — Declare and Version Public Contracts

Stable schemas, protocol formats, command surfaces, publication artifacts, and stable interfaces shall be explicitly declared and versioned. Contract evolution shall be intentional and visible rather than incidental. 

The concrete substrate rules for `@compiled_semantic_contract` classes are
defined in `docs/library_ref/contract_substrate_discipline.md`: no `Any` field
positions, explicit attrs converters for local invariants, cattrs profiles for
boundary structuring, no typing introspection in the decorator chain, and no
`eval`-based annotation resolution.

#### Principle 30 — Design for Testability

Architecture shall permit low-friction testing of semantic rules, normalization, compilation, projection, runtime policy, adapters, and governance checks. Dependency injection, pure-core transformations, explicit contracts, and golden artifacts are architectural requirements rather than test-suite conveniences. 

#### Principle 31 — Additive Extensibility and Executable Governance

New capability shall enter primarily through added semantics, passes, projections, or adapters rather than through repeated edits to runtime core or shell infrastructure. Governance shall exist to detect violations of this doctrine through dependency checks, boundary checks, contract checks, determinism tests, provenance tests, and failure-taxonomy coverage. 

## 8. Secondary Implementation Constraints

The following constraints are adopted as implementation-level review rules. They are normative for code review and architecture review, even when they do not define primary platform topology.

**Prefer composition over inheritance.**
Behavior assembly shall default to composition unless inheritance provides a clear, bounded, and stable semantic fit.

**Law of Demeter.**
Code shall interact with direct collaborators rather than depending on deep structural reach-through.

**Tell, don’t ask.**
Behavior and invariants shall remain close to the abstractions that own them rather than being recreated through external raw-data inspection.

**KISS.**
Solutions shall remain as simple as possible without sacrificing required boundaries, determinism, or inspectability.

**YAGNI.**
Abstraction layers shall not be introduced without a clear, near-term second use case or a clearly identified change vector.

**Principle of least astonishment.**
APIs, schemas, commands, and runtime behaviors shall align with the expectation of a competent reader and shall minimize surprising side effects or non-obvious interpretations.

**Conceptual integrity / semantic consistency.**
The same concept shall carry the same name across the platform, and the same name shall not represent different concepts in different subsystems unless an explicit translation boundary exists. 

## 9. Anti-Principles

The following conditions are doctrine violations:

* simulation-family-specific execution paths as the default extension mechanism;
* platform truth distributed across GUI widgets, solver objects, and workbook coordinates;
* hidden duplicate rule definitions across runtime, UI, and adapters;
* workflow controllers or state machines acting as the primary repository of engineering semantics;
* durable identity derived from row order, proxy indices, or solver-local order;
* untracked side-write paths outside the semantic command model;
* privileged behavior distributed diffusely rather than governed through narrow authority.

## 10. Conformance Evidence

A platform conforming to this doctrine shall provide at minimum:

1. a published semantic case schema;
2. a published artifact taxonomy;
3. documented pass contracts;
4. explicit semantic identity rules;
5. a semantic command model;
6. provenance artifacts or equivalent evidence traces;
7. deterministic or golden tests for normalization and selected projections;
8. dependency graph checks and layering tests;
9. failure taxonomy and mapped diagnostics;
10. structured observability surfaces;
11. explicit lifecycle ownership for mutable and scarce resources;
12. at least one alternate adapter path proving boundary independence.

Absent evidence shall be treated as absence of conformance rather than as missing documentation.

## 11. Decision Framework

### 11.1 Default Architecture Selection

| Need                                                                                                          | Primary architectural selection      |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| Stable platform-independent meaning with multiple possible technical realizations                             | MDA-style separation                 |
| Rich runtime lowering, optimizer targeting, and multiple internal representations                             | Compiler-style multi-IR architecture |
| Replaceable UI, solver, storage, import, and automation edges                                                 | Ports and Adapters                   |
| Lifecycle traceability across requirements, behavior, verification, and external tool ecosystems              | MBSE / SysML v2                      |
| Direct domain-author editing of structured semantics, multiple notations over one model, non-textual notation | DSL / language-workbench approach    |
| Reviewable, governed workflow/control artifacts                                                               | Statechart / SCXML artifact path     |

### 11.2 Selection Rules

**MDA-style separation** applies when platform independence is the dominant architectural problem and the same semantics are expected to outlive implementation technology. ([OMG][1])

**Compiler-style multi-IR architecture** applies by default when the platform requires staged lowering, inspectable intermediate forms, optimizer-specific targets, and multiple downstream projections. ([mlir.llvm.org][2])

**Ports and Adapters** applies as the standard boundary discipline for all entry and exit mechanisms. ([alistair.cockburn.us][3])

**MBSE / SysML v2** applies when formal requirements, interface descriptions, verification cases, lifecycle continuity, and external model interoperability become first-class concerns. OMG defines SysML as a general-purpose language for specifying, analyzing, designing, and verifying complex systems, and notes the adoption of SysML v2 as the next-generation form. ([OMG][6])

**DSL / language-workbench approaches** apply when structured semantic authoring itself becomes a first-class product capability, especially where multiple notations over one underlying model are required. This selection remains consistent with the existing semantic doctrine’s emphasis on semantics-first authoring and compiled projection. 

**Statechart / SCXML artifacts** apply when workflow or control logic itself must become a governed artifact rather than remaining internal orchestration code. The durable domain / temporal control separation remains in force in either case. 

### 11.3 Default Stack for the Present Context

For a semantic-compiled engineering platform of the present type, the default stack is:

* MDA-style separation for the semantic/platform boundary;
* compiler-style multi-IR architecture for the runtime core;
* ports-and-adapters for all external boundaries;
* provenance and hermeticity as mandatory cross-cutting constraints;
* optional SysML v2 / MBSE overlays where lifecycle traceability and external interoperability justify them;
* optional DSL / language-workbench layers where structured semantic authoring becomes a first-class product capability.

## 12. Litmus Test

The doctrine is being followed when new scope is realized primarily by adding semantic content, manifests, passes, projections, and adapters. The doctrine is not being followed when new scope regularly requires editing the platform’s core runtime, rewriting the shell, inventing new one-off workflow controllers, or introducing simulation-specific execution branches as the default extension mechanism. 

## 13. Closing Standard

A semantic-compiled engineering platform is, under this doctrine, a platform in which semantics define the problem instance, compilation normalizes and lowers those semantics through governed artifact stages, a bounded runtime executes compiled forms through reusable protocols, the workbench renders compiled projections rather than separate truths, and provenance, observability, reproducibility, and controlled authority make every result inspectable, repeatable, and governable.

[1]: https://www.omg.org/mda/?utm_source=chatgpt.com "Model Driven Architecture (MDA) | Object Management Group"
[2]: https://mlir.llvm.org/docs/LangRef/ "MLIR Language Reference - MLIR"
[3]: https://alistair.cockburn.us/hexagonal-architecture "hexagonal-architecture"
[4]: https://bazel.build/versions/8.1.0/basics/hermeticity "Hermeticity  |  Bazel"
[5]: https://www.w3.org/TR/2013/REC-prov-dm-20130430/ "PROV-DM: The PROV Data Model"
[6]: https://www.omg.org/sysml/?utm_source=chatgpt.com "SysML® Official Specifications | Object Management Group"
