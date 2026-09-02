---
artifact: authoritative-design
artifact_id: CF-GEN-2.3.0
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: GEN
artifact_version: 2.3.0
authority_status: current
predecessor_path: docs/authoritative_design/present_state_cpg_fact_generation_specification_python_rust_v2.2.md
---

# Present-state CPG fact generation specification for Python and Rust v2.3

## 0. Governing contract and transition status

The stable artifact identity is `CF-GEN-2.3.0`. This artifact is the current generation
authority for the `codefabric-relational-data-fabric` v2.3 suite. The v2.2 predecessor remains
immutable history; its generated registries, provider DTOs,
payloads, and fingerprints are not coequal semantic authority.

This successor preserves the high-level Python/Rust product facts, public identities, explicit
unknowns, incremental behavior, and semantic query capabilities. It
replaces the realization with exact provider-native Arrow relations, explicit application-owned
analysis producers, typed programmatic normalization/authority/proof, relation-scoped Arrow IPC,
and one immutable proved `FabricEpoch`. Until target cutover, the deployed runtime may remain
legacy; this document does not certify implementation state.

Historical v1 wire artifacts are allocation evidence only. Provider and analysis
execution never depends on a v1 profile, translator, generated binding, or
compatibility fixture. V2.3 introduces no serving-owned provider, guard, completion,
or resource producer: those are QRY/SRV control and presentation concerns, while GEN
continues to emit only provider-native and application-analysis relations.

### 0.1 Execution is authority

Exact provider batches plus explicit non-derivable typed inputs and typed
`ProgrammaticTransformation` values produce the provider boundary, authority, normalization,
derivation, query-requirement, coverage, unknown, and proof relations that execution reads.
Candidate-catalog relation/field/schema/dependency/provenance observations are derived from the
installed session to fixed point. Generated catalogs, YAML registries, censuses,
fingerprints, bundle manifests, and cached capability lists MAY diagnose or render the current
programmatic catalog but
MUST NOT provide a second semantic answer.

### 0.2 Exact-current-API rule

Adapters bind to the pinned APIs named in §2 without a defensive facade for hypothetical future
versions. Application-owned Arrow schemas and DTO ownership boundaries are required isolation,
not claims that different provider APIs are interchangeable. A provider/API change requires an
explicit migration, compile probes, fixture reacceptance, schema migration, and proof.

### 0.3 Released compatibility

The eight public semantic request forms, public entity/fact encodings, released Protobuf field
identities, public errors/results, deterministic ordering, provenance, explicit unknowns, and
one-epoch consistency remain commitments. Semantic Arrow IPC between Rust processes is an
internal versioned boundary; removing legacy opaque provider payloads does not authorize a
public wire break.

## 1. Purpose

This specification defines how immutable source bytes and exact analysis contexts become
provider-native, normalized, and mechanically derived CPG facts for Python and Rust. It fixes
provider authority, coverage semantics, process/trust boundaries, incrementality, reconciliation,
error/resource behavior, Arrow schemas, and executable acceptance.

## 2. Source basis and version anchors

The adopted provider baseline is exact:

| Surface | Adopted identity |
|---|---|
| Tree-sitter runtime | `tree-sitter = 0.26.12` |
| Python grammar | `tree-sitter-python = 0.25.0` |
| Rust grammar | `tree-sitter-rust = 0.24.2` |
| stable-root Ruff component crates | `0.0.7` |
| Pyrefly | `1.2.0`, revision `1933169ad8ee9e4d4114112eb56ef0811fb0a094` |
| rustc extractor toolchain | `nightly-2026-08-18` |
| rustc compiler identity | `1.100.0-nightly`, commit `8fa1c96cfd489e4c27654c144ae871ce2c4db6c6` |
| semantic Arrow boundary | Arrow `59.2.0` |

The Pyrefly sidecar's transitive Ruff `0.0.6` universe remains isolated inside that process and
MUST NOT exchange Ruff Rust types with the stable root's `0.0.7` universe. The exact Tree-sitter,
Ruff, Pyrefly, and Rust MIR library references are evidence for the API allocations below. Live
manifests/locks/toolchain identity and compile probes adjudicate drift.

## 3. Normative scope

This specification owns source-to-fact generation, not durable publication, public query
interpretation, or FastMCP presentation. It includes syntax and semantic providers, application
analyses, provider IPC/control, capability/provenance production, incremental invalidation, and
the fact batches consumed by an epoch builder. It excludes git history, runtime execution
observation, risk judgments, and arbitrary SQL.

## 4. Provider responsibility model

Provider responsibility is per fact family, not per language or adapter:

| Authority | Genuine responsibility |
|---|---|
| Tree-sitter | error-tolerant CST, grammar-native structure, queries/captures, `ERROR`/`MISSING`, incremental edit and changed-range evidence |
| Ruff `0.0.7` | Python tokens/trivia, typed AST, source coordinates, scopes, bindings, references, import syntax and parser/semantic diagnostics |
| Pyrefly exact hybrid | Python semantic types, calls, members, module/import meaning, selected definitions/xrefs/navigation, dependency impact |
| `rustc_public` | Rust semantic items/types/instances and typed MIR bodies/CFG/calls/access structure exposed by the pinned public API |
| narrow `rustc_private` | stable compiler keys, exact source/hygiene, exact borrow-check/loan facts, and only selected mono/vtable enrichment unavailable publicly |
| CodeFabric analysis | Python CFG/flow; Rust ownership/flow approximations; common graph, effects/resources, and interprocedural summaries |

Each provider is evidence, not universal truth. A provider may be authoritative for one family
and merely corroborating or inapplicable for another.

## 5. Authority and precedence

### 5.1 Python authority

Ruff is primary for valid typed source structure, tokens/trivia, scopes, bindings, references,
and import syntax. Tree-sitter is primary for damaged/incomplete CST, grammar-native structural
queries, incremental reuse, and changed ranges; it corroborates but does not replace richer Ruff
facts on complete source. Pyrefly's exact selected surface is primary for cross-module type,
member, call-target, module-resolution, and affected-module semantics. CodeFabric is sole
authority for Python CFG/evaluation order, dataflow, alias/points-to, effect/resource, and
interprocedural analyses.

### 5.2 Rust authority

Tree-sitter is primary for Rust source CST and incomplete-edit structure. `rustc_public` is
primary for public semantic/MIR facts genuinely exposed by the pinned compiler. The narrow
`rustc_private` seam is primary only for stable-key, exact source/hygiene, exact borrow-check,
and selected mono/vtable families. CodeFabric is sole authority for conservative ownership,
reaching-definitions, liveness, alias/points-to, drop/resource, async/lowering summaries, and
common graph analyses. `rustc_public` does not acquire stable identity or borrow-check authority
by supplying MIR inputs.

### 5.3 Conflict and reconciliation

Authority selection is a typed programmatic relational plan. It retains all conflicting raw
evidence, selects canonical facts only under the per-family authority rule, emits diagnostics,
and materializes multi-candidate or unknown facts when resolution is not justified. No adapter
may silently overwrite another provider or infer a negative from missing output.

## 6. End-to-end architecture

```text
immutable source images + analysis context + exact programmatic assembly inputs
  -> stable-root Tree-sitter/Ruff adapters
  -> pinned Pyrefly sidecar via control gRPC + relation-scoped Arrow IPC
  -> dated-nightly rustc extractor via trust launcher + control gRPC + Arrow IPC
  -> provider-native relations + coverage/remainder/diagnostic relations
  -> typed identity, normalization, authority, conflict and unknown transformations
  -> separately versioned application-derived analyses
  -> proof-bearing candidate FabricEpoch
```

Every stage consumes and emits immutable, keyed relations. No semantic payload crosses as
opaque JSON, debug text standing in for typed APIs, or one-row-per-fact Protobuf.

## 7. Provider isolation requirements

### 7.1 Tree-sitter

`Parser`, `Tree`, `TreeCursor`, `Node<'tree>`, `Query`, and capture views stay inside the adapter
call or revision cache. Outputs materialize owned source/digest/range/kind/field/error DTOs.
There is one parser per concurrent parse; compiled queries may be shared only as their API
permits. Numeric node/field IDs and node reuse are not cross-version identity.

### 7.2 Ruff

All Ruff `0.0.x` types remain inside the stable-root adapter. The complete `Parsed` value,
tokens, trivia, index, AST, and semantic model may be retained only within revision-bounded
adapter state. Outputs are typed Arrow rows; no `AtomicNodeIndex` becomes canonical identity.

### 7.3 Pyrefly

One long-lived pinned sidecar state per workspace/context owns Pyrefly objects. Provider binding
keys, response-local type indices, and bundled Ruff types never cross the process boundary as
application identity. `Require::Everything` is used only for files selected for bulk extraction;
dependencies needing exports use `Require::Exports`.

### 7.4 rustc

No `rustc_public` or `rustc_private` borrowed/session value escapes the compiler callback or
crosses a thread/process boundary. The dated-nightly extractor is a separate Cargo root and
target. Textual MIR is diagnostic only, never a machine interface.

### 7.5 Semantic compilation trust

Process isolation is not sandboxing. Every untrusted Rust semantic run enters through the
policy-bearing launcher in AC-G-35. A direct host Cargo/rustc launch, inherited credentials or
network, mutable workspace view, or claimed-only sandbox digest is non-conforming.

## 8. Immutable source-image contract

A `SourceImage` binds workspace, file identity, raw canonical path bytes, content digest, exact
bytes or immutable descriptor, encoding decision, source kind, language, generation, and access
authorization. Providers analyze exactly those bytes. Output for a mismatched digest/generation
is rejected as stale; providers never reread a mutable live path behind the daemon's back.

## 9. Canonical source coordinates

Canonical coordinates are half-open byte ranges attached to `(file_id, content_digest)`. Ruff
`TextRange`, Tree-sitter bytes/points, Pyrefly/LSP positions, and rustc spans convert at the
adapter boundary with checked UTF-8/UTF-16 and macro/source-hygiene rules. Line/column values are
derived presentation. Out-of-bounds, reversed, wrong-digest, or mid-code-point ranges are
protocol failures or explicit unmappable remainders.

## 10. Provider-observation metadata

Every provider relation or lossless keyed run relation carries provider/run identity, exact
release/revision/toolchain/grammar, programmatic assembly, analysis context, semantic-environment identity,
source generation/digests, requested family/scope, completed family/scope, remainder reason,
diagnostics, cancellation/resource status, trust receipt where applicable, provider-local key,
and relation `SchemaContract` identity.

Coverage is set-valued, not a success count:

```text
requested_scope
completed_scope
remainder_scope + reason
unknown_scope + cause
diagnostic/evidence
```

`requested EXCEPT (completed UNION intentional_remainder)` MUST be empty before a run may claim
closed coverage. Intentional remainder still limits capability; it does not become success.

## 11. Relation-scoped Arrow IPC boundary

Each semantic data stream carries exactly one relation schema and a bounded sequence of Arrow
IPC batches. Control messages name protocol version, run/job, relation/program/schema IDs, source
and context pins, requested scope, sequence/credit state, terminal coverage, and checksums.
Protobuf owns control only. A stream manifest may enumerate relations, but semantic rows are not
embedded in it.

The receiver validates the session-derived `SchemaContract` before accepting a batch: logical
Arrow schema, field IDs/order/types/nullability, fixed-width identifiers, nested types,
dictionary/extension metadata, source/context/run pins, sequence, batch and total byte/row
limits. Credits/backpressure are relation-aware. Deadline or cancellation stops producer work,
closes the stream, kills the process group where needed, releases reservations, and emits an
explicit incomplete remainder. Corruption never falls back to legacy JSON interpretation.

## 12. Raw and normalized observation preservation

Provider-native relations retain native variants, kinds, fields, ranges, local handles, and raw
renderings where a typed field does not exist. Normalized relations are separate typed
transformations linked through provenance. New provider variants require an explicit versioned
schema/typed-input change or a typed raw escape/remainder already authorized for that family; they are never
dropped into a generic JSON blob.

## 13. Canonical semantic-identity inputs

Canonical identity is computed after provider ingestion. Python uses module/context identity,
qualified lexical path, kind, and governed anonymous-owner anchors. Rust uses private
`StableCrateId + DefPathHash` when present. If the selected private seam is unavailable, the
application qualified-name key is explicit and capability is downgraded; the system MUST NOT
claim stable compiler identity. Raw `DefId`, MIR local/block indices, provider node indices, and
response-local type indices remain provenance only.

## 14. Python pipeline overview

Python generation combines Tree-sitter's fast/error-tolerant lane, Ruff's full-file typed source
lane, Pyrefly's semantic lane, typed programmatic reconciliation, then CodeFabric owner-local and
interprocedural analyses. Syntax remains available when semantics degrade. Semantic facts are
accepted only for the exact source and semantic environment.

## 15. Python source and lexical fact generation

Ruff supplies primary byte/line coordinates, tokens, comments/trivia, continuation and string
detail, with Tree-sitter source ranges as corroboration/error-tolerant evidence. Encoding,
newline, source type (`.py`, `.pyi`, supported generated source), and unsupported/binary/oversize
policy are explicit.

## 16. Python syntax fact generation

Ruff `parse_unchecked`/typed AST supplies rich syntax and recoverable errors; Tree-sitter supplies
CST-native nodes/fields, `ERROR`/`MISSING`, structural queries, and edit-local changed ranges.
Ruff reparses a whole file; it is not presented as incremental. Extract only trustworthy spans
from damaged regions and emit explicit remainder for the rest.

## 17. Python semantic-entity generation

Declarations/entities begin from Ruff structure and bindings, then reconcile with Pyrefly
semantic evidence. Application-owned identities never embed Ruff/Pyrefly handles. Generated or
external entities retain their source/context and body-availability status.

## 18. Python scope and binding generation

Ruff is primary for lexical scopes, bindings, references, global/nonlocal, comprehension,
pattern, import, and assignment roles. Pyrefly may enrich resolved semantics but does not replace
source-role evidence. Dynamic constructs produce candidate/unknown relations.

## 19. Python module/import/export generation

Ruff supplies import syntax: module text, relative depth, imported names, aliases, and binding
occurrences. Semantic resolution comes from the pinned Pyrefly TSP/module-resolver path under the
exact module search environment. Query is not claimed to expose import resolution. Star/dynamic
imports and missing stubs/dependencies produce explicit remainder and affected-module edges.

## 20. Python type generation

The exact authority split is:

- `Query::get_type_table_in_file` for bulk inferred expression types;
- the pinned TSP seam for declared, computed, and expected distinctions where exposed;
- `Query::is_subtype` as an on-demand oracle, not a persisted all-pairs relation;
- `Query::resolve_target_from_qualified_name` for the helper's exact supported target class; and
- explicit unsupported/remainder rows for distinctions the pinned surface cannot provide.

Response-local type indices are normalized within the run and retained only as provenance.
Rendered type strings alone do not define canonical type identity.

## 21. Python object/member generation

`Query::get_attributes` is the exact Query authority for selected member/attribute facts.
TSP/module semantics and Ruff source declarations supply declared/object-model context.
Descriptors, properties, MRO candidates, dynamic attributes, and unresolved members retain
multi-candidate/unknown behavior rather than a guessed unique target.

## 22. Python callable-contract generation

Ruff provides declaration and parameter syntax; Pyrefly type surfaces provide accepted semantic
parameter/return/overload evidence. Decorator transformation and dynamic wrapper effects are
separate candidate/unknown facts. Callable contracts retain source and semantic provenance.

## 23. Python call-site and dispatch generation

Ruff/Tree-sitter emit every syntactic call site. `Query::get_callees_with_location` supplies the
exact accepted call-target family. Zero returned callees is unknown, not proof of none. Union,
overload, decorator, descriptor, and dynamic receiver behavior yields typed candidates with a
declared resolution/completeness tier.

## 24. Python CFG generation

Python CFG, evaluation order, branches, loops, exceptional edges, cleanup, and async suspension
are CodeFabric-derived from accepted Ruff structure and semantic evidence. They MUST NOT carry
`raw_ruff` or `raw_pyrefly` authority. Outputs name the Python CFG algorithm release, owner,
input rows, context, precision, remainder, and clean/incremental proof.

## 25. Python value and dataflow generation

Definitions/uses, reaching definitions, liveness, value-flow, and merge semantics are
application-owned algorithms over the CFG and binding/type evidence. Partial typing, dynamic
features, unknown calls, and damaged syntax propagate explicit unknowns.

## 26. Python memory, alias, and points-to generation

CodeFabric models locals, closure cells, globals, attributes, subscripts, and conservative heap
locations. Alias/points-to results are candidate sets with precision and unknown remainder.
Neither Ruff nor Pyrefly is credited with an analysis it did not expose.

## 27. Python effect generation

Direct syntactic/semantic evidence feeds application effect facts and summaries. Reads, writes,
calls, raises, allocation/resource operations, dynamic mutation, and unknown effects remain
separate; no risk or safety conclusion is emitted.

## 28. Python exceptional-flow generation

The application builds raise/handler/finally/with/cleanup and propagation edges under named
precision. Unknown call/decorator/context-manager behavior yields unknown exceptional successors.

## 29. Python resource-lifetime generation

Acquire/release/transfer/escape and context-manager cleanup are application-derived under a
versioned resource model. Conditional and exceptional cleanup is retained. No leak verdict is
produced.

## 30. Python async, generator, and concurrency generation

The application relates async functions/generators, await/yield/suspension/resume, async
iteration/context management, and statically evidenced spawn/join/synchronization candidates.
It does not claim runtime order.

## 31. Python closure and capture generation

Ruff binding evidence supplies lexical candidates; CodeFabric derives capture/environment facts
and Pyrefly enriches type/member meaning. Capture mode or escape that cannot be established is
explicit unknown.

## 32. Python generated/synthesized semantic generation

Decorators, dataclass-like synthesis, stubs, generated files, and implicit members are emitted
only from exact provider evidence or a named application transformation. Source/generated
correspondence and uncertainty remain queryable.

## 33. Python explicit-unknown generation

Every unresolved symbol/type/member/module/call/memory/effect/control/resource family produces a
typed unknown/remainder row bound to request, owner, context, reason, and evidence. An empty
provider relation without completed coverage produces unknown capability.

## 34. Rust pipeline overview

Rust generation combines Tree-sitter source CST with contained dated-nightly compilation. The
extractor emits public typed semantic/MIR relations and narrow private enrichment, after which
CodeFabric derives ownership/flow and common analyses. Compile failure removes current compiler
facts from the candidate and emits capability gaps; stale-current compiler facts are prohibited.

## 35. Rust source and lexical generation

Tree-sitter supplies Rust source CST, tokens/structure, incomplete-edit recovery, and changed
ranges. rustc spans/source scopes/hygiene enrich exact compiled correspondence. Macro-expanded
or unmappable spans retain call-site/def-site evidence and explicit remainder.

## 36. Rust semantic-definition generation

Pinned `rustc_public` supplies available items, definitions, types, visibility, ownership, and
relationships. Canonical identity is not raw `DefId`. The private seam supplies stable-key and
source/hygiene inputs only where exact compile probes prove them.

## 37. Rust type and generic generation

Typed public compiler structures supply Rust types, generic parameters, substitutions,
associated/projection/opaque forms, and available instance evidence. Debug text is supplemental
raw evidence only and cannot replace a typed field exposed by the compiler.

## 38. Rust MIR-body generation

The public seam emits typed bodies, phases, blocks, locals, places/projections, operands,
rvalues, statements, terminators, source scopes, constants/promoteds, and native variants.
Nothing is reconstructed by parsing textual MIR. Raw indices remain body/run-local coordinates.

## 39. Rust CFG generation

Public MIR terminator successors are provider-native CFG structure. CodeFabric preserves normal,
unwind, cleanup, false/unwind-unreachable, yield/resume, switch, call, drop, assert, return, and
abort-like variants as exposed. Dominance/control-dependence and conservative semantic flow are
application-derived later.

## 40. Rust place, memory, and access-event generation

The adapter losslessly projects typed MIR places/projections and statement/terminator operand
contexts into raw access observations, retaining the underlying native variant and coordinates.
Any classification requiring alias, reaching, liveness, ownership state, or effect inference is
an application-derived family.

## 41. Rust call and instance generation

Public MIR call terminators plus exact `Instance` resolution supply direct callable evidence.
Function pointers, closures, trait objects/dynamic dispatch, shims, drop glue, intrinsics, and
cross-crate generic instances remain distinct and carry precision. Uses are not silently turned
into call-graph edges.

## 42. Rust trait and dynamic-dispatch generation

Public trait/impl/instance evidence supplies available candidates. The narrow private seam may
add selected vtable facts only when the accepted boundary names and probes that exact family.
Dynamic dispatch remains a declared over-approximation with unknown remainder.

## 43. Rust macro and generated-code generation

Tree-sitter records source macro occurrences; rustc source maps/hygiene from the selected private
seam record exact expansion correspondence where available. Compilation in order to obtain this
evidence always uses the trust launcher.

## 44. Rust move, initialization, and ownership-state generation

Exact private borrow-check/loan observations remain `rustc_private` provider-native rows.
CodeFabric separately derives conservative initialization, move, ownership, borrow-state, and
unknown facts from raw MIR/access inputs. The application output MUST NOT claim exact borrowck or
compiler provenance.

## 45. Rust def-use and liveness generation

CodeFabric derives def-use, reaching definitions, kills, liveness, and value-flow over raw MIR
CFG/access facts. MIR is not assumed SSA. Results name algorithm release, owner/body, precision,
input provenance, and clean/incremental proof.

## 46. Rust alias and points-to generation

Alias/points-to is application-owned and conservative. It distinguishes references, raw
pointers, dereferences, fields, indices, address-taking, unions, casts, unsafe/FFI escape, and
unknown memory. Exact private loans are inputs/evidence, not relabeled alias results.

## 47. Rust drop and resource generation

Raw drop terminators/glue/shims and cleanup edges remain compiler evidence; resource acquire,
release, transfer, escape, and interprocedural lifetime implications are application-derived.

## 48. Rust async/coroutine generation

Raw public MIR and selected source correspondence expose generated state-machine structure where
available. CodeFabric derives higher ownership/resource/flow summaries and records uncertainty
where source-to-lowering mapping is incomplete.

## 49. Rust constants/statics/CTFE generation

Typed compiler values and errors are retained when exposed. Debug representations are
supplemental. Unavailable/evaluation-error states are explicit and never replaced by source-text
guessing.

## 50. Rust unsafe/FFI/inline-assembly generation

The application records exact source/compiler occurrences, ABIs, raw operations, foreign
symbols/calls, and conservative cross-language candidates. These are facts, not vulnerability or
safety judgments.

## 51. Rust explicit-unknown generation

Compile failure, private-seam unavailability, unsupported target/config, unmappable spans,
dynamic dispatch, indirect calls, missing dependency bodies, cancellation, limits, and protocol
failure produce typed current capability gaps and fact remainders. Last-known-good semantics may
be retained only as explicitly stale history and never published as current.

## 52. petgraph role

petgraph `0.8.3` may implement bounded graph algorithms only after native relational/
DataFusion mechanisms are evaluated. It consumes application-owned canonical IDs and produces
application-derived rows. `NodeIndex` is ephemeral and never stored or returned.

## 53. Projection construction

Graph projections are compiled from typed transformation inputs specifying relation inputs, node/edge identity,
direction, duplicates, filters, scope, unknown policy, and `SchemaContract`. Procedural filename
or static registry selection is prohibited.

## 54. Reachability generation

Reachability is deterministic, bounded when query-facing, and explicit about truncated,
disconnected, unknown-edge, and resource-exhausted results. Direct and transitive edges remain
distinguishable.

## 55. Strongly connected components and recursion

SCC/recursion facts name the exact projection and algorithm release. Results are invariant to
input row order and internal graph indices and preserve unknown-call limitations.

## 56. Dominance generation

Dominators and post-dominators operate on the selected entry/exit and edge families, including
the declared exceptional-edge policy. Unreachable nodes and multi-exit semantics are explicit.

## 57. Control-dependence generation

Control dependence derives from the declared CFG/post-dominator semantics and records exceptional
edge policy, synthetic exits, unknown successors, and completeness.

## 58. Loop generation

Loop headers, back edges, nesting, irreducible components, exits, and membership are objective
facts under the selected CFG projection. No complexity judgment is emitted.

## 59. Reaching-definitions framework

The versioned dataflow framework defines direction, lattice, transfer, join, convergence,
program-point semantics, owner scope, and unknown propagation. Provider output never acquires
this authority merely by supplying definitions and uses.

## 60. Liveness generation

Liveness uses the declared use/def and CFG semantics, including exceptional/cleanup policy. It
is application-derived, owner-scoped, and clean/incremental equivalent.

## 61. Points-to and alias analysis

Common alias/points-to algorithms state abstraction, field/index sensitivity, context policy,
soundness/precision tier, escape/unknown behavior, bounds, and language-specific inputs.

## 62. Shortest graph distance

Distance facts declare directedness, edge weights (or unit weight), bounds, unreachable
semantics, duplicates, and unknown edges. They are objective and request-bounded.

## 63. Connected components

Weak/strong/undirected component semantics are separately named. Results use canonical external
IDs and deterministic component identity independent of row order.

## 64. Transitive reduction and closure

Closure/reduction is emitted only for a named, bounded, semantically valid projection. Cycles,
duplicates, unknown edges, and resource limits have explicit behavior.

## 65. Structural metric generation

Metrics state their projection, unit, duplicate policy, and aggregation scope. They remain
descriptive measurements, never evaluative labels.

## 66. Interprocedural summary generation

Summaries use monotone, deterministic fixed points with explicit convergence/resource bounds,
unknown-callee propagation, recursion/dynamic-dispatch behavior, algorithm/precision release,
dependency/invalidation closure, and provenance. Non-convergence or exhaustion yields unknown
and blocks dependent capability proof.

## 67. Structural relationship generation

Structural edges are emitted from provider-native roles/ranges and normalized through typed
transformations without deleting raw evidence.

## 67A. Source and lexical relationship and typed-extension generation

Source/token/trivia/range/diagnostic relations use typed Arrow fields. Provider-native variants
may use a governed typed escape union; JSON and debug strings cannot substitute for exposed
structure.

## 68. Symbol and binding relationship generation

Declarations, ownership, binding, references, shadowing, capture, and unresolved candidates are
reconciled under the per-family Python/Rust authorities in §5.

## 69. Module/dependency relationship generation

Module/import/export/dependency edges preserve syntax request, semantic resolution, context,
search-root/stub decision, candidates, and remainder.

## 70. Type relationship generation

Declared/computed/expected/narrowed/subtype/instantiation relations remain distinct. Query-time
oracles such as Pyrefly subtype tests are not expanded into unbounded persisted closures.

## 71. Member relationship generation

Member declaration, inheritance/implementation, lookup, override, descriptor/property, and
candidate/unknown edges retain language-specific authority.

## 71A. Python-specific object-model relationship generation

Python MRO, descriptor, protocol, metaclass, class/instance, and dynamic attribute behavior uses
the exact selected Pyrefly/Ruff surfaces and explicit unknowns.

## 71B. Rust-specific object-model relationship generation

Rust trait/impl/associated-item/vtable/instance relationships preserve public versus private
compiler authority and dynamic-dispatch precision.

## 72. Invocation relationship generation

Every call occurrence is emitted before resolution. Resolved and candidate edges link through
the call-site entity and retain dispatch, authority, completeness, and unknown remainder.

## 73. Control-flow relationship generation

Provider-native public MIR CFG and application-derived Python CFG remain distinct relation
families. Common normalization preserves normal/exceptional/unwind/cleanup/suspend edge kinds.

## 74. Dataflow relationship generation

Dataflow relations are application-derived under §§25, 45, and 59–60. Raw provider definitions,
uses, places, or bindings remain inputs and cannot be relabeled as reaching/liveness output.

## 75. Memory relationship generation

Reads/writes/moves/copies/borrows/address-taking retain exact raw context when exposed; alias,
points-to, abstract heap, and unknown memory are separate derived families.

## 76. Ownership/lifetime relationship generation

Exact private borrow-check rows and application approximations occupy different relations,
provenance domains, capability states, and public precision projections.

## 77. Effect relationship generation

Direct evidence and propagated summaries remain separate. Unknown calls/providers/analysis
limits propagate unknown effects rather than optimistic none.

## 77A. Exceptional-flow relationship generation

Normal, exception, unwind, cleanup, handler, and unknown successor relations remain typed and
distinct across language normalization.

## 77B. Resource-lifetime relationship generation

Acquire/release/transfer/escape/cleanup facts name their language abstraction, algorithm,
exception policy, and unknown remainder.

## 77C. Async and concurrency relationship generation

Source, provider-lowered, and application-derived suspension/concurrency relations remain
separate; runtime observations are excluded.

## 77D. Closure and capture relationship generation

Capture relations preserve source binding evidence, environment/lowering correspondence, mode,
escape, and unknowns.

## 77E. Program-point state relationship generation

State rows bind canonical owner/body identity to observation-local points and exact generation;
they are replaced with the owner and never range-translated between content digests.

## 78. Generated/lowered relationship generation

Expansion, synthesis, desugaring, specialization, shims, and source/hygiene correspondence are
emitted only from exact evidence or a named versioned derivation.

## 79. Derived graph relationship generation

All common graph and summary rows carry `application_derived` authority, algorithm/precision,
input/program/source/provider closure, completeness, and proof identity.

## 80. Range reconciliation algorithm

Providers join on `(file_id, content_digest, start_byte, end_byte)` plus semantic role, not on
line/column or provider node identity. Exact matches are preferred; containment/overlap is
allowed only by a typed reconciliation transformation that emits its method and ambiguity.
Conflicting or unmappable
ranges remain evidence plus unknown.

## 81. Declaration reconciliation

The plan joins source occurrences, lexical ownership, provider declarations, module/context, and
identity recipes. It never merges incompatible kinds solely because ranges match.

## 82. Type reconciliation

Raw provider type evidence is normalized into canonical structural types while retaining raw
forms. Authority selects among declared/computed/expected/narrowed propositions independently.
Unrepresentable variants become typed remainder, not erased text.

## 83. Call-target reconciliation

Call-target evidence joins through the first-class call site. Per-language authority,
multi-candidate sets, dynamic-dispatch tier, indirect uses, and explicit unknown remainder are
preserved.

## 84. Explicit unknown-materialization rules

For every requested unit, the epoch builder proves a completed fact, intentional remainder,
diagnostic failure, or typed unknown. Empty relations alone are never evidence of none. Unknown
rows carry reason categories including unsupported, not applicable, parse/type/compile failure,
missing dependency, ambiguous, cancelled, timed out, resource limit, stale, corrupt, and trust
unavailable.

## 85. Capability reporting

Capabilities begin unknown. Advertised support is a query over accepted boundary demands,
requested/completed/remainder/unknown coverage, exact provider/toolchain/schema identity,
derived-producer closure, trust posture, and passing proof in the current epoch. Boolean or
hard-coded capability lists are non-authoritative. `TRUSTED_LOCAL` is visibly degraded and never
indistinguishable from contained untrusted compilation.

## 86. Generation output boundary

The stable root receives relation-scoped Arrow IPC or in-process Arrow batches conforming to the
same session-derived schema. It accepts no provider object, borrowed tree/compiler value, semantic
JSON blob, or debug-text substitute. Every relation links to its source/context/run, coverage,
provenance, and `SchemaContract`.

## 87. Derived-analysis boundary

Derived producers consume only accepted immutable provider/canonical relations and emit new
application-owned relations. They cannot mutate raw facts, claim provider provenance, or author
their own independent acceptance expectations.

## 88. Activation boundary

Provider success does not activate facts. The epoch builder validates schema, coverage,
authority, provenance, unknown, derivation, policy, resource, and independent semantic proof for
the exact candidate. Activation selects the whole proved epoch atomically; queries never mix
syntax generation N+1 with semantic generation N.

## 89. Recommended crates and mechanism selection

Use the exact pinned crates directly at their owned boundary: Tree-sitter/Ruff in the stable
root, Pyrefly in its sidecar, rustc public/private in the dated-nightly extractor, Arrow 59.2.0
for semantic data, DataFusion 55 for typed programmatic relational work, and petgraph 0.8.3 only for
bounded graph gaps. This is not permission to add another package, native Python extension, or
compatibility facade.

## 90. Provider job interfaces

Jobs are application-owned control DTOs containing job/run ID, provider release, fabric build ID,
analysis context/semantic environment, source descriptors and digests, requested relation
families/scopes, resource/deadline/cancellation policy, trust profile, and negotiated protocol/
schema identities. Results are Arrow streams plus terminal coverage and diagnostics.

## 91. Canonical graph-projection DTO

Graph execution receives a typed programmatic projection descriptor or logical plan with
canonical node/edge IDs, relation/schema identities, duplicate/direction/filter/unknown/bound
policy, and resource envelope. It never receives persisted petgraph indices or arbitrary SQL.

## 92. Programmatic relation interface

The predecessor declarative model-pack, bootstrap, and migration-replay interfaces are superseded.
Semantics enter only through exact provider batches, explicit typed inputs, and typed
transformations compiled to DataFusion expressions/logical plans. A packaged rendering may be
transported for diagnostics or review but cannot be loaded as parallel semantic authority.

## 93. Provider fixture requirements

Independently authored fixtures cover each accepted API family with valid, empty-complete,
partial, damaged syntax, type/compile failure, ambiguity, cancellation, limit, oversize,
unsupported context/platform, stale digest/context, corruption, and trust failure cases.
Provider implementers may supply observations but cannot author the sole expected semantics.

## 94. Differential validation

For supported overlap, compare providers without erasing authority: Tree-sitter/Ruff structure,
Ruff/Pyrefly declarations and ranges, syntax/semantic call sites, Tree-sitter/rustc source
correspondence, public/private compiler seams, and incremental/clean output. Differences become
diagnostics/conflict/unknown rows, not automatic test failure or silent overwrite.

## 95. Algorithm validation

Every derived family has independently authored examples, property tests, row-order permutation,
addition/deletion/change cases, exceptional/dynamic/partial inputs, clean versus incremental
equivalence, causal input mutations, convergence/resource tests, and exact provenance checks.

## 96. Canonical invariants

Executable invariant plans prove unique keys, valid foreign references, source/context/epoch
pins, range bounds, identity recipes, authority uniqueness, schema closure, requested coverage,
provenance closure, derived producer uniqueness, explicit unknowns, and absence of judgment
facts. Zero violations with uncovered inputs is unknown, not pass.

## 97. Capability gaps and required treatment

A gap is current typed data. It names family/scope, provider or algorithm, exact version/context,
cause, evidence/diagnostic, trust/resource state, retryability, last-known-good history if any,
and dependent capabilities/proofs. Gaps block claims that require them; they do not delete
unrelated syntax facts.

## 98. Phase 1 — Source and syntax completeness

Acceptance proves immutable source images, coordinate conversion, Tree-sitter/Ruff exact APIs,
damaged-source behavior, raw kinds, changed-range semantics, relation schemas, and coverage/
unknown closure.

## 99. Phase 2 — Semantic identity and types

Acceptance proves application identity, exact Pyrefly hybrid authority, public/private Rust
identity inputs, types/members/modules/calls, environment/context pins, conflicts, and degraded
fallback behavior.

## 100. Phase 3 — CFG and access events

Acceptance proves public MIR CFG/access fidelity, application Python CFG ownership, edge-kind
coverage, ranges/program points, partial failure behavior, and raw-versus-derived provenance.

## 101. Phase 4 — Dataflow and ownership

Acceptance proves Python/Rust application analyses, distinct exact borrow-check observations,
precision/unknown semantics, owner replacement, and clean/incremental equality.

## 102. Phase 5 — Derived graph facts

Acceptance proves projection compilation, selected DataFusion/petgraph rung, external-ID
round-trip, SCC/reachability/dominance/control-dependence/loop behavior, bounds, cancellation, and
resource release.

## 103. Phase 6 — Effects and summaries

Acceptance proves direct/transitive separation, effect/resource/exception/async semantics,
monotone bounded fixed points, unknown propagation, invalidation closure, and independent
expectations.

## 104. Phase 7 — Full conformance

Full conformance requires exact API compile/behavior probes, all provider and derived phases,
schema lifecycle proof, semantic environment invalidation, untrusted Rust sandbox proof,
relation-scoped IPC faults, capability/provenance closure, public/internal semantic equality,
clean reconstruction, incremental equivalence, and target-route zero state for opaque payloads,
static registries, and provider-authority mislabeling.

## AC-G-14 — Analysis-context discovery, identity, and selection

An analysis context includes language version/target, build/package target, dependency and
feature selection, module/crate roots, configuration, platform, stubs/typeshed/sysroot/toolchain,
and source generation. Discovery emits candidate and selection relations with evidence. A source
file may participate in multiple contexts; context-dependent semantic facts include context in
identity. Unchanged bytes under a changed context are semantically invalidated.

## AC-G-30 — Pyrefly sidecar wire protocol and exact authority

The sidecar binds to the pinned revision and exposes these authority classes:

| Fact family | Exact selected surface |
|---|---|
| bulk inferred types | `Query::get_type_table_in_file` |
| call targets | `Query::get_callees_with_location` |
| members | `Query::get_attributes` |
| subtype helper | `Query::is_subtype` on demand |
| qualified target helper | `Query::resolve_target_from_qualified_name` |
| declared/computed/expected distinctions | pinned TSP seam where actually exposed |
| import/module resolution | pinned TSP/module-resolver seam |
| accepted bulk definitions/xrefs | deliberately selected exact-revision Glean/internal seam |
| named navigation fallback | LSP only for the accepted fallback family, never bulk authority |

`Query::add_files` rendered diagnostics are not represented as a structured diagnostic API.
Their text may be retained as raw diagnostics while unavailable structure becomes remainder.
The protocol uses relation-scoped Arrow IPC, bounded credits, cancellation, source/context/run
validation, and typed terminal coverage.

One long-lived workspace state records actual `Require::Everything` and `Require::Exports`
tiers. The semantic-environment identity includes Pyrefly version/revision, complete effective
configuration, selected Python version/platform, admitted interpreter identity without
executing repository Python, roots/search paths, typeshed/stub/dependency identities,
build-system module map, and presets/strictness. A change is global semantic invalidation.

After file changes the sidecar reports modules Pyrefly actually affected/rechecked. It MUST NOT
echo requested modules as proof of recheck. If the pinned surface cannot prove a smaller set,
the daemon conservatively refreshes reverse importers and records that policy/remainder.

## AC-G-31 — rustc extractor protocol and public/private seam

The extractor handshake binds exact nightly/compiler/target/sysroot/application/program/protocol/schema/trust
identities. `rustc_public` supplies typed public semantic/MIR families. The smallest pinned
`rustc_private` adapter supplies only stable compiler keys, exact source/hygiene, exact
borrow-check/loan, and selected mono/vtable families independently compile- and behavior-probed.
Application analyses remain separate.

Canonical Rust identity uses private stable keys when available or the explicit downgraded
application qualified-name recipe. Raw `DefId`, MIR indices, and borrowed compiler values never
cross. Data streams are relation-scoped Arrow IPC; Protobuf carries control. Toolchain mismatch,
compile failure, trust failure, corrupt stream, or private-seam loss yields typed gaps, never a
different nightly or legacy-summary fallback.

## AC-G-32 — Common asynchronous provider execution interface

All lanes implement admission, bounded queues/credits, deadlines, cancellation, supersession,
progress, resource accounting, terminal coverage, and stale-result rejection. Cancellation is
cooperative where possible and forceful at the process group when required. Fairness and limits
are enforced by the daemon; providers cannot publish directly.

## AC-G-33 — Immutable source snapshot transport

In-process lanes borrow only revision-pinned bytes. Process lanes receive immutable,
descriptor-relative source/dependency views and validated path manifests. No lane reads mutable
workspace paths after admission. Every output range and digest is checked against the admitted
image before acceptance.

## AC-G-34 — Build and project-configuration discovery

Discovery emits typed candidates, selected context, inputs, resolution evidence, ambiguity, and
unknowns. It does not execute untrusted repository Python. Rust build metadata requiring build
scripts/procedural macros is obtained only through the trust launcher. Config/search path/
feature/target/sysroot/dependency changes invalidate semantic output even when bytes do not
change.

## AC-G-35 — Provider sandbox and Rust compilation trust model

The versioned `RustCompilationTrustPolicy` selects the launcher profile and is recorded in every
launcher receipt and dependent capability/provenance row. The default Rust profile is untrusted
and fail-closed. The launcher supplies an immutable
read-only `ProviderWorkspaceView` and `DependencyInputBundle`; offline registry/cache inputs; a
minimal allowlisted environment; no inherited credentials, proxy/agent variables, network,
home, Git metadata, mutable workspace, or unrelated file descriptors; and private validated
target/output/temp directories outside source.

It bounds wall/CPU time, memory, processes/threads, open files, file count/size, artifact bytes,
stdout/stderr, and total Arrow output. Cancel/timeout/limit kills the full process group and
releases private outputs. Build scripts and proc macros execute only inside proved containment;
if the platform cannot enforce it, extraction fails closed. `TRUSTED_LOCAL` requires a distinct
authorization input, launcher receipt, capability/provenance state, and public visibility. A
launcher claim/digest without hostile execution proof has no authority.

Hostile acceptance fixtures attempt network and credential access, source/parent/symlink/path
escape writes, inherited descriptor use, surviving child processes, output explosion, process
exhaustion, timeout, CPU and memory exhaustion. Direct host Cargo/rustc ingress is structural
failure.

## AC-G-36 — Provider capability granularity and aggregation

Capabilities are derived by joining typed program/input demand, boundary contract, installed handler/schema,
requested/completed/remainder/unknown rows, trust and resource status, derived producer closure,
and proof results. Aggregation never turns partial family support into workspace-wide support.
Every advertised capability names epoch/context/scope/precision and its limiting remainder.

## AC-G-38 — Programmatic transformation, matching, and trust

The former model-pack and migration log are not authority. Programmatic semantics are closed typed
inputs and transformations with exact application/provider releases, author/review provenance,
and independently accepted expectations. Assembly produces typed relations; structural matching
and plan compilation consume those values. Untrusted external semantic input is not executable
without parsing into an accepted typed input and completing the proof transaction.

## AC-G-39 — Derived-analysis precision profiles

Each derived producer row names algorithm release, semantic family, inputs, owner scope,
direction/lattice/transfer where applicable, graph projection, path/field/context sensitivity,
soundness/precision tier, bounds, invalidation, materialization, unknown propagation, and proof.
Exactly one producer or explicit unsupported remainder is required per accepted family.

## AC-G-40 — Generated, expanded, stub, shim, and lowered source capture

Every non-authored form carries form kind, generating provider/algorithm, source/owner/context,
correspondence evidence, range/hygiene quality, and body availability. Source and generated
entities remain distinct even when a normalized projection links them.

## AC-G-43 — Unsupported, oversized, binary, generated, and vendored files

File admission policy is explicit typed input and produces one of accepted, excluded-not-applicable,
unsupported, oversized, binary, generated-policy, vendored-policy, unreadable, or unknown.
Excluded scope is visible in capability and proof. A skipped/unreadable file cannot establish a
negative fact or a complete workspace result.

## Cross-layer integration obligations

The ontology supplies fact families, identity recipes, authority classes, and `SchemaContract`
links. The fabric registers provider-native/canonical/derived relations in one epoch and builds
normalization, proof, and semantic query plans from typed transformations over exact batches and
explicit inputs. Lifecycle owns immutable input,
invalidation, replacement, cancellation, and atomic activation. FastMCP presents daemon results
only and never reconstructs provider or analysis logic.

## Release conformance obligations

The implementation SHALL expose and pass intent-level checks equivalent to:

```text
syntax-provider-native-check
syntax-provider-exact-api-check
pyrefly-provider-native-check
pyrefly-exact-surface-matrix-check
pyrefly-semantic-environment-invalidation-check
rustc-provider-native-check
rustc-public-private-authority-check
rustc-untrusted-compilation-sandbox-check
provider-native-arrow-conformance-check
provider-protocol-check
provider-normalization-authority-check
provider-capability-proof-check
python-derived-analysis-conformance-check
rust-mir-derived-analysis-conformance-check
derived-analysis-authority-coverage-check
clean-incremental-equivalence checks
provider-legacy-json-zero-state-check
provider-static-registry-target-zero-state-check
```

Each check executes against the exact current adapter, programmatic relation observations, typed
inputs/transformations, and production path;
expectations are independently owned. Compile success alone does not prove semantics, a digest
does not prove row equality, and zero violations without input coverage is unknown.

## Provider boundary for executed programmatic transformations

Every active provider family resolves relationally from accepted demand to one exact handler,
provider-native relation/schema, authority rule, normalization plan, coverage/unknown contract,
and proof obligation. Every active derived family resolves to one algorithm producer or explicit
unsupported remainder. The resulting `SchemaContract` is used across Arrow IPC, DataFusion
logical/physical planning, batches, storage, and output restoration. A missing, duplicate,
dangling, inert, wrongly authoritative, or unproved link blocks epoch activation.
