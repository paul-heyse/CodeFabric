---
artifact: authoritative-design
artifact_id: CF-ONT-2.3.0
suite_id: codefabric-relational-data-fabric
suite_version: 2.3.0
artifact_tag: ONT
artifact_version: 2.3.0
authority_status: current
predecessor_path: docs/authoritative_design/code_property_graph_present_state_fact_ontology_specification_v2.2.md
---

# Code Property Graph present-state fact ontology specification v2.3

## 0. Governing contract and transition status

The stable artifact identity is `CF-ONT-2.3.0`. Together with the other current
`codefabric-relational-data-fabric` v2.3 masters, this artifact is the sole current ontology
authority. Its v2.2 predecessor is immutable historical evidence, not a coequal runtime or
authoring authority.

This successor preserves the CodeFabric fact, identity, unknown, provenance, and query
semantics while replacing generated registries, bundles, censuses, fingerprints, and
hand-maintained traceability as semantic authorities. V2.3 carries those ontology contracts
forward without importing MCP presentation, guarded-input transport, completion, or public
resource-handle mechanics into the fact vocabulary. A missing semantic query input remains a
typed QRY preparation result, not a new fact family. Publication of this master does not claim
runtime conformance.

Historical v1 wire allocations remain immutable evidence, but no v1 runtime
profile or compatibility route is part of this ontology's production target.

The authoritative ontology of a candidate epoch is assembled from exact provider `RecordBatch`
schemas, explicit typed inputs whose values cannot be derived, and typed
`ProgrammaticTransformation` values. The candidate DataFusion session derives relation, field,
schema, dependency, and provenance observations from what is actually installed. Execution reads
those batches and transformations directly. A declared output schema is an assertion checked
against the derived plan schema, never the source of that schema. This prose fixes semantics and
acceptance obligations; it does not establish a parallel current catalog.

### 0.1 Precedence

The suite governance master owns cross-artifact precedence. Within this artifact, normative
`SHALL`, `MUST`, and `MUST NOT` clauses override examples. Released public wire and identity
allocations remain immutable unless an explicit successor migration preserves or tombstones
them. A current live catalog, programmatic input/transformation relation, or exact library API beats a cached navigation
page or derived documentation.

### 0.2 Relational authority

At minimum the programmatic assembly exposes typed relations equivalent to:

```text
input.fact_family
input.entity_kind
input.property_kind
input.relationship_kind
input.type_algebra_variant
input.identity_recipe
program.authority_rule
program.derivation
input.provider_boundary
input.query_requirement
program.unknown_rule
system.programmatic_schema_observation
input.proof_obligation
```

Names above identify semantic roles, not a frozen physical table-name API. Their stable semantic
IDs, keys, dependencies, and `SchemaContract` links are constructed from admitted schemas,
explicit inputs, and typed transformations. A generated file,
Rust enum, YAML document, static census, or digest MAY materialize or diagnose these rows but
MUST NOT answer a semantic question independently of them.

### 0.3 Staticness boundary

Static declarations are limited to exact build/toolchain inputs, released wire and identity
contracts, explicit non-derivable policy/compatibility inputs, independently accepted expectations,
and historical records. Current schemas, provider surfaces, dependencies, capabilities,
traceability, validation status, and coverage are derived by assembling and inspecting the
candidate session. No bootstrap metamodel, migration log, or replayed schema registry is authority.

### 0.4 Released compatibility

Within the sole supported `codefabric.cpgd.v2` client boundary, this version does not remove the
eight public semantic request forms, public entity/fact ID encodings, released v2 error/result
meanings, v2 Protobuf field identities, accepted tombstones, or one-snapshot response semantics.
Historical v1 allocations remain immutable allocation evidence but do not create v1 client,
profile, negotiation, translation, or runtime compatibility. Internal provider payloads and
generated catalogs are not wire commitments. Every released v2 ID receives an explicit preserve,
migrate, supersede, or tombstone decision before physical deletion.

## 1. Purpose

This ontology defines language-neutral and Python/Rust-specific facts for a present-state code
property graph. It is a fact substrate, not an evaluative judgment system. Facts describe what
source says, what pinned static providers report, and what named deterministic analyses derive.
They never imply safety, quality, risk, desirability, or test impact.

## 2. Normative scope

Included domains are source and lexical structure; syntax; semantic identity; scopes,
bindings, modules, imports, exports, types, members, callables, call sites, dispatch, CFG,
dataflow, abstract memory, ownership, effects, exceptions, resources, async/concurrency,
generated/lowered code, objective graph facts, and interprocedural summaries. Git history,
runtime observations/coverage, environment inventory, and engineering recommendations are
excluded.

Every accepted fact family has exactly one authoritative producer class or an explicit
unsupported remainder. Provider observations, canonical normalized facts, and application-
derived facts remain distinct and queryable.

## 3. Definition and canonical forms of a fact

A fact is one typed proposition in one immutable epoch. It has one of three forms:

```text
entity existence     entity_id + entity_kind
relationship         fact_id + subject_id + relationship_kind + object_id
property             fact_id + subject_id + property_kind + typed_value
```

Independent propositions MUST NOT be hidden as unauditable columns on a broad entity row.
Denormalized columns are allowed only as deterministic, reconstructible projections. Fact rows
link to an executable `SchemaContract` derived from the admitted provider schema or built logical
plan; Arrow metadata alone never makes an invalid value valid.

## 4. Design principles

The ontology obeys these invariants:

1. syntax occurrence is not semantic entity;
2. provider-native and normalized facts coexist;
3. provider-local identity is observation-local;
4. a byte range is meaningful only with its file and content digest;
5. absence requires coverage proof or becomes explicit unknown;
6. authority resolves conflicts without deleting evidence;
7. derivations name algorithm, precision, inputs, completeness, and proof;
8. every fact has one replacement owner and one epoch;
9. queryability never depends on opaque JSON semantic payloads; and
10. facts never encode agent judgment.

## 5. Source and lexical ontology

Source entities include `WORKSPACE`, `ANALYSIS_CONTEXT`, `SOURCE_FILE`, `SOURCE_IMAGE`,
`SOURCE_REGION`, `TOKEN`, `TRIVIA`, `COMMENT`, `DOCSTRING`, and `DIAGNOSTIC`. Required source
properties include content digest, byte length, language, source kind, raw path bytes,
display path, encoding decision, line index identity, and source generation. Token/trivia facts
retain exact byte ranges and provider-native kinds.

## 6. Syntax ontology

Syntax occurrences include concrete nodes, typed AST occurrences, error nodes, missing nodes,
syntactic declarations, references, imports, annotations, call expressions, and binding targets.
They retain provider-native kind/field/named-extra flags beside normalized kind. A recovered
tree is usable evidence only for ranges the provider marks trustworthy; damaged regions produce
diagnostics and unknown remainders.

## 7. Semantic identity ontology

Semantic entities include modules, scopes, symbols, declarations, bindings, references,
callables, types, members, parameters, variables, constants, external entities, generated
entities, and executable instances. Common properties include name, qualified name, semantic
kind, visibility, mutability, source/name spans, external/generated/synthesized flags, and
language-specific modifiers.

Canonical identity is application-owned. Tree-sitter node IDs, Ruff node indices, Pyrefly
binding/type indices, raw `DefId`, MIR local/block ordinals, and petgraph indices MUST NOT be
canonical identity.

## 8. Scope, binding, and name-resolution ontology

A `SCOPE` is a language-recognized lexical or semantic resolution domain. Relationships include
`BINDS`, `REFERS_TO`, `MAY_REFER_TO`, `SHADOWS`, `CAPTURES`, `CAPTURED_FROM`, `ALIASES`, and
`REBINDS`. References classify declaration, definition, read, write, read-write, import,
parameter, capture, type, call, and member roles. Unresolved and multi-candidate resolution is
explicit.

## 9. Module, import, export, and dependency ontology

The ontology distinguishes import syntax from resolved module identity and declares module,
package, namespace, import request, import binding, resolved target, export, re-export, and
dependency relationships. Relative depth, aliases, star imports, conditional/type-only imports,
stub/runtime source, search-root decision, and unresolved remainder remain separate facts.

## 10. Type ontology

Types are canonical semantic entities with a closed, versioned algebra covering unknown, any,
never, primitive, literal, nominal, tuple, union, intersection where supported, callable,
overload, generic application, type variable, parameter specification, variadic tuple,
reference/pointer, array/slice, function item/pointer, trait object, projection/associated type,
opaque, and provider-native escape variants. Declared, computed, expected, and narrowed type
facts are separate propositions with independent provenance.

## 11. Member and object-model ontology

Member facts distinguish declaration, lookup candidate, resolved member, inherited member,
property/descriptor behavior, class/instance access, visibility, static/class methods, trait or
protocol requirements, and unresolved dynamic lookup. Object-model authority is language-
specific; syntax alone never proves resolution.

## 12. Callable contract ontology

Callable contracts represent parameters, parameter kinds, defaults, annotations/types,
return/yield/send types, receiver rules, variadics, generics, effects where known, and overload
alternatives. A declared callable and an executable specialization are distinct.

## 13. Call-site ontology

A call site is first-class even when no target resolves. It records the call expression,
enclosing callable/owner, syntactic callee, arguments and evaluation order, source range,
dispatch shape, candidate set, selected target when known, and unknown remainder. A call site is
not reducible to a caller-to-callee edge.

## 14. Dispatch ontology

Dispatch facts classify direct, static, virtual, trait-object, function-pointer, closure,
constructor, operator, descriptor, dynamic/member, reflection-like, and unresolved dispatch.
Candidate sets carry resolution and completeness; multiple candidates are not collapsed to one
guess.

## 15. Control-flow ontology

Control-flow entities include callable entry/exit, basic blocks, program points, branch and
switch decisions, normal edges, exceptional/unwind edges, cleanup/drop edges, suspension/resume,
and unreachable structure. CFG identity is owner-local and versioned; provider-local block
ordinals remain observation coordinates.

## 16. Derived control-flow facts

Dominators, post-dominators, control dependence, loop membership/nesting, reachability, SCCs,
recursion, and bounded path/distance facts are application-derived. Each names the graph
projection, algorithm release, precision, bounds, input epoch, and completeness.

## 17. Value and computation ontology

Values and computations include literals, expressions, operands, rvalues, conversions,
operators, tuple/aggregate construction, calls, allocations, projections, and provider-native
lowering forms. A value is distinct from its memory location and from the occurrence that
produced it.

## 18. Definition/use and dataflow ontology

Facts include definitions, uses, kills, def-use/use-def, reaching definitions, liveness,
value-flow, phi-like merge semantics where an analysis introduces them, and explicit unknown
sources/sinks. MIR is not assumed to be SSA. These analyses are application-derived unless an
exact provider family explicitly supplies the claimed proposition.

## 19. Abstract memory and state-location ontology

Abstract locations cover locals, parameters, globals/statics, fields, attributes, indexed or
subscripted elements, dereferences, heap abstractions, closure cells, and unknown memory.
Location identity includes owner/context and projection semantics; it never equates a source
expression with storage.

## 20. Alias and points-to ontology

Alias and points-to facts are conservative candidate relations with an explicit precision tier,
scope, analysis release, and unknown remainder. `MAY_ALIAS` is not `MUST_ALIAS`; absence of a
candidate is meaningful only under proved complete coverage.

## 21. Program-point state ontology

Program-point state relates a point to initialization, move, borrow/loan, reaching-definition,
liveness, ownership, resource, or abstract-value state. State facts carry the exact CFG/body
identity and cannot be moved between generations by matching ordinals.

## 22. Effect ontology

Effects distinguish direct observations from transitive summaries. Families include reads,
writes, mutation, allocation, deallocation, calls, throws/raises, I/O or external interaction
when statically evidenced, synchronization, unsafe/FFI use, and unknown effect. Effects are
facts about static evidence, not risk judgments.

## 23. Exceptional-flow ontology

Facts cover raise/throw, catch/handler/finally, unwind, cleanup, propagation, exception groups,
panic-like edges where modeled, and unknown exceptional continuation. Normal and exceptional
edges MUST remain distinct.

## 24. Resource-lifetime ontology

Resource facts represent acquire, release, transfer, escape, conditional release, cleanup
owner, and unknown lifetime. Resource classification and precision are named; no leak verdict is
part of the base ontology.

## 25. Async and concurrency ontology

Facts cover async callables, futures/coroutines/generators, suspension and resume points, spawn,
join, send/receive, lock/synchronization occurrences, and only mechanically supported
happens-before candidates. Static potential is not runtime execution.

## 26. Closure and capture ontology

Closures/lambdas own capture facts including captured entity, by-value/reference/mutable mode
when known, environment field or cell, move semantics, escape, and unknown mode.

## 27. Generated and lowered-code ontology

Source, macro-expanded, synthesized, shim, stub, desugared, and lowered entities remain
distinguishable. `GENERATED_FROM`, `EXPANDS_TO`, `LOWERS_TO`, `SYNTHESIZES`, and source-hygiene
relations preserve correspondence and uncertainty.

## 28. Generic and specialization ontology

Facts cover type/lifetime/const parameters, bounds, substitutions, generic declarations,
monomorphized or specialized instances, trait/protocol implementations, and unresolved
substitutions. Declaration and executable instance identities remain separate.

## 29. Objective graph-analysis facts

Allowed graph facts include reachability, SCC membership, recursion components, dominance,
post-dominance, control dependence, connected components, bounded shortest distance, and
explicitly selected closure/reduction. They are versioned mechanical results, not judgments.

## 30. Objective structural metrics

Counts and measures such as node/edge count, in/out degree, nesting depth, block count, and path
bound are permitted when their graph projection and counting semantics are explicit. Labels such
as complex, risky, or hotspot are prohibited.

## 31. Interprocedural summary ontology

Callable summaries may cover inputs/outputs, direct/transitive effects, resources, exceptions,
calls, reads/writes, captures, and unknown callees. Fixed points are monotone, deterministic,
bounded, provenance-producing, and explicit about incomplete recursion or dynamic dispatch.

## 32. Explicit unknown ontology

Unknown entities include symbol, type, call target, member, module, memory, effect, control-flow
successor, ownership state, resource state, provider output, and derived remainder. Each unknown
names the requested family, scope, reason, producer/run, and whether retry or a different context
could change it.

## 33. Python scope ontology

Python scopes cover module, class, function, lambda, comprehension, annotation/type-parameter,
and pattern contexts with Python-specific lookup rules. Class-body and comprehension behavior
MUST NOT be modeled as generic lexical nesting alone.

## 34. Python binding ontology

Bindings include local/global/nonlocal, parameter, import, assignment, annotation, exception,
pattern, comprehension, walrus, and implicit/synthesized bindings. Dynamic mutation of globals,
`exec`, star imports, and uncertain attribute effects produce explicit remainder.

## 35. Python type ontology extensions

Python types include `Any`, `Unknown`, `Never`, literals, unions, overloads, protocols, typed
dictionaries, callables, descriptors/properties, generics, `Self`, parameter specifications,
variadic tuples, and declared/computed/expected/narrowed distinctions. Provider-rendered type
text is evidence, not canonical identity by itself.

## 36. Python object-model ontology

Facts cover class and metaclass relationships, MRO, protocol conformance, attributes, methods,
properties/descriptors, class/instance lookup, dynamic attributes, dataclass-like synthesis when
observed, and unknown member resolution.

## 37. Python call ontology

Python call sites retain positional/keyword/starred arguments, receiver/member form,
decorator/wrapper ambiguity, overload and union candidates, call-target provenance, and unknown
remainders. Zero Pyrefly callees does not prove no target.

## 38. Python dynamic-semantics facts

Dynamic imports, `getattr`/`setattr`, monkey-patching, metaclass hooks, decorators, `exec`, and
reflection-like constructs are represented as occurrences, static candidates, and explicit
unknown effects—never optimistic exact resolution.

## 39. Python decorator ontology

Decorator applications are first-class ordered facts connecting decorator expression, decorated
declaration, application order, statically resolved candidates, and unknown transformation.

## 40. Python pattern-matching ontology

Pattern facts distinguish value, capture, wildcard, sequence, mapping, class, OR, AS, and guard
forms. Pattern identifiers that bind are not reference reads.

## 41. Python comprehension ontology

Comprehensions own their generators, filters, evaluation order, binding context, captured names,
and async form. Their scope semantics follow the selected Python analysis context.

## 42. Python context-manager ontology

Context-manager facts connect enter/exit or async-enter/async-exit occurrences, bound targets,
exceptional cleanup, resolved candidates, resources, and unknown protocol behavior.

## 43. Python async and generator ontology

Facts cover async functions, generators, async generators, `await`, `yield`, `yield from`, async
iteration/context management, suspension/resume, send/yield/return types, and unresolved effects.

## 44. Rust source-semantic entities

Rust entities include crates, modules, items, functions, associated items, impls, traits,
structs, enums/variants, unions, fields, statics, constants, closures, generators/coroutines,
foreign items, macros, and executable instances.

## 45. Rust declaration properties

Properties include visibility, `unsafe`, `const`, `async`, ABI, externness, mutability,
attributes, generics, where clauses, trait/impl ownership, source/hygiene correspondence, and
generated status.

## 46. Rust generic ontology

Rust generics model type, lifetime, and const parameters; bounds; predicates; substitutions;
associated types/constants; opaque types; impl Trait; and monomorphized instances. Session-local
compiler handles remain provenance only.

## 47. Rust type ontology extensions

Rust types include scalar, tuple, array, slice, reference, raw pointer, ADT, foreign, function
definition, function pointer, closure, coroutine, dynamic trait object, projection, parameter,
alias, opaque, never, and error/unknown forms.

## 48. Rust MIR ontology

MIR facts retain body phase, blocks, locals, places/projections, operands, rvalues, statements,
terminators, source scopes, promoted/constant references, unwind edges, and provider-native
variants. MIR indices are body-local observation coordinates, not canonical global IDs.

## 49. Rust place and projection ontology

A place is a local plus ordered projections such as dereference, field, index, constant index,
subslice, and downcast. Place reads, writes, moves, copies, borrows, address-taking, discriminant,
drop, and unknown context remain distinct.

## 50. Rust MIR state-transition ontology

Typed transitions connect program points, operands/rvalues, places, statements/terminators,
normal/unwind successors, calls, drops, assertions, switches, yields, and returns. Raw structure
is distinct from application-derived ownership/dataflow state.

## 51. Rust ownership and borrow ontology

Exact compiler-private loan/region/borrow-check observations, when available, occupy a distinct
provider-native family. Application ownership, initialization, move, alias, and liveness
approximations occupy derived families with their own precision. Neither may impersonate the
other.

## 52. Rust call and executable-instance ontology

Rust calls distinguish declared callable, resolved `Instance`, direct call terminator, closure,
function pointer, virtual/dynamic candidate, shim/drop glue/intrinsic use, specialization, and
unknown target. Dynamic dispatch is an over-approximation with a declared tier.

## 53. Rust trait and dynamic-dispatch ontology

Facts cover trait declarations, impls, obligations where exposed, associated items, vtable
enrichment where selected, receiver adjustment, candidate implementations, and unresolved
dispatch.

## 54. Rust macro ontology

Macro occurrences, definitions, invocations, expansions, hygiene/source correspondences, and
generated owners are represented when exact compiler or syntax evidence exists. Unavailable
expansion detail is an explicit remainder.

## 55. Rust drop and destruction ontology

Facts distinguish explicit `Drop` calls, compiler drop terminators, drop glue/shims, cleanup
paths, resource implications, and application-derived lifetime summaries.

## 56. Rust async and coroutine-lowering ontology

Async/coroutine facts connect source entities to generated state machines, suspension/resume,
captured fields, discriminants where exposed, cleanup, and unknown lowering correspondence.

## 57. Rust unsafe and FFI ontology

Facts describe unsafe blocks/functions/operations, raw pointers, inline assembly, foreign items,
ABIs, extern calls, and cross-language candidates. They do not assert vulnerability or safety.

## 58. Rust constants, statics, and CTFE ontology

Facts cover constants/statics, mutability, initializer, allocation/reference evidence,
evaluated value when typed and exposed, and explicit CTFE unavailability or error.

## 59. Derived facts versus source facts

Every fact has one `authority_class`:

```text
source_native | provider_native | normalized | application_derived | proof_or_capability
```

Provider-native means the proposition is genuinely exposed by the named exact provider or a
lossless typed projection of its native structure. Normalized facts are authority-selected
projections. Application CFG/dataflow/alias/effect/resource/summary/graph results are never
provider-native merely because provider rows are inputs.

## 60. Recommended graph projections

Model relations define projections for syntax, symbol/binding, module dependency, type/member,
call, CFG, dataflow, memory, ownership, effect/resource, generated/lowered, and common graph
views. A projection names included entity/edge families, direction, duplicate policy, unknown
policy, owner/context scope, and `SchemaContract`. It is compiled at runtime and is not a static
registry.

## 61. Universal fact metadata

Every fact carries, directly or through lossless keyed relations:

```text
fact_id, workspace_id, analysis_context_id, fabric_epoch_id,
source_generation, owner_id, language, fact_form, fact_family_id,
subject_entity_id, object_entity_id or typed_value, program_point_id,
source_file_id, content_digest, start_byte, end_byte,
producer_id, producer_release, provider_run_id, authority_class,
certainty, resolution, directness, completeness, schema_contract_id,
supporting_fact_ids or provenance-edge identity
```

Optionality is defined by the admitted provider schema and typed transformation per fact family.
Ranges are valid only as
`(source_file_id, content_digest, start_byte, end_byte)` and use half-open byte offsets.

## 62. Canonical evidence, resolution, directness, and completeness dimensions

The dimensions remain orthogonal typed relations, not one confidence score. Released names retain
their meanings: certainty includes `SOURCE_EXACT`, `COMPILER_EXACT`, `STATIC_SEMANTIC`,
`SOUND_MAY`, `MODELLED`, `HEURISTIC`, `UNRESOLVED`; resolution includes `EXACT`,
`STATICALLY_RESOLVED`, `SOUND_POSSIBLE`, `POSSIBLE`, `MODELLED`, `HEURISTIC`, `UNRESOLVED`,
`UNAVAILABLE`, `NOT_APPLICABLE`; directness includes `DIRECT`, `TRANSITIVE`, `SUMMARY`,
`NOT_APPLICABLE`; completeness includes `COMPLETE`, `PARTIAL`, `INDETERMINATE`, `UNAVAILABLE`,
`NOT_APPLICABLE`.

Accepted numeric allocations remain immutable released compatibility inputs and history. A
generated enum or prose table may render them but cannot become current authority.

## 63. Ownership of facts

Replacement owners include source file, module, scope, callable, class/type, MIR body,
crate/build unit, and workspace-global derivation. The smallest sound owner is required.
Ownership defines current-state replacement and invalidation, not history. Global ownership is
allowed only when no smaller coherent replacement scope exists.

## 64. Required identity and public encoding rules

Application-owned IDs remain 128-bit values derived from unkeyed BLAKE3-256 over `CBEF-v1`,
retaining the full digest for collision diagnosis. CBEF uses `CFID`, version `0x01`, a
big-endian domain and field framing, typed length-prefixed payloads, deterministic container
ordering, explicit absence tags, and domain-specific normalization. Production callers use
explicit typed recipe-specific builders; arbitrary tagged-field vectors and delimiter strings are
non-conforming.

Released lowercase public encodings remain:

```text
workspace:<32-hex>  repository:<32-hex>  worktree:<32-hex>
context:<32-hex>    snapshot:<32-hex>    publication:<32-hex>
entity:<kind-slug>:<32-hex>              fact:<kind-slug>:<32-hex>
```

`context:source` is the sole symbolic exception. Decoders validate domain, kind, width, and
lowercase hexadecimal form. Unequal preimages sharing a 128-bit ID block activation with
`ID_COLLISION`; there is no silent re-key.

File identity remains present-state and path-based: same canonical workspace-relative path
preserves `file_id` across content or inode replacement; rename/move creates a new `file_id`.
Continuity evidence may transfer caches but is not semantic identity. Python semantic identity
uses module identity plus qualified lexical path and kind under its analysis context. Rust
prefers private `StableCrateId + DefPathHash`; when unavailable, a documented application
qualified-name key is used with downgraded capability. Anonymous occurrences use owner-relative
role/ordinal and digest-bound range inputs.

## 65. Required separation of fact types

Conformance distinguishes syntax occurrence/entity, declaration/reference, type syntax/type,
call expression/call site/callable, declaration/instance, value/location, read/write, copy/move,
borrow/address-taking, normal/exceptional edge, direct/transitive effect, candidate/unknown
target, and source/generated entity.

## 66. Mandatory unknown semantics

Missing edges are never a universal uncertainty encoding. `UNKNOWN_SYMBOL`, `UNKNOWN_TYPE`,
`UNKNOWN_CALL_TARGET`, `UNKNOWN_MEMBER`, `UNKNOWN_MODULE`, `UNKNOWN_MEMORY`, `UNKNOWN_EFFECT`,
and family-specific unknown/remainder rows are mandatory where applicable. A negative fact is
allowed only when requested scope, completed coverage, and an exhaustive provider/derivation
claim are all proved.

## 67. No evaluative ontology rule

The ontology MUST NOT contain `SAFE_TO_REFACTOR`, risk scores, test-impact conclusions, likely-
bug labels, vulnerability conclusions, recommendations, hotspots, design-quality labels, or
other engineering judgments. A separately governed consumer may reason from facts; its judgment
does not become a base CPG fact.

## 68. Ontology layers

The epoch registers explicit-input, provider-native, program, canonical, derived,
capability/proof, and authorized public-projection layers. Dependency direction is explicit
input/source/provider → program/canonical → derived → query/proof. A higher layer cannot rewrite
lower-layer evidence.

## 69. Structural relationships

Structural families include `CONTAINS`, `PARENT_OF`, `CHILD_OF`, `PRECEDES`, `HAS_TOKEN`,
`HAS_TRIVIA`, `HAS_RANGE`, `HAS_DIAGNOSTIC`, and provider-native field/role edges.

## 70. Symbol and binding relationships

Families include `DECLARES`, `DEFINED_IN`, `OWNED_BY`, `HAS_SCOPE`, `BINDS`, `REFERS_TO`,
`MAY_REFER_TO`, `SHADOWS`, `CAPTURES`, `ALIASES`, and `REBINDS`.

## 71. Module and dependency relationships

Families include `IMPORTS`, `RESOLVES_TO_MODULE`, `EXPORTS`, `REEXPORTS`, `DEPENDS_ON`,
`USES_STUB`, and explicit unresolved/import-remainder relations.

## 72. Type relationships

Families include `HAS_DECLARED_TYPE`, `HAS_COMPUTED_TYPE`, `HAS_EXPECTED_TYPE`,
`HAS_NARROWED_TYPE`, `SUBTYPE_OF`, `IMPLEMENTS`, `INSTANTIATES`, `SPECIALIZES`,
`HAS_TYPE_ARGUMENT`, and unknown type evidence.

## 73. Member relationships

Families include `HAS_MEMBER`, `INHERITS_MEMBER`, `OVERRIDES`, `IMPLEMENTS_MEMBER`,
`RESOLVES_MEMBER`, `MAY_RESOLVE_MEMBER`, and descriptor/property relations.

## 74. Invocation relationships

Families include `HAS_CALL_SITE`, `CALLS`, `MAY_CALL`, `RESOLVES_CALL_TO`, `DISPATCHES_TO`,
`INSTANTIATES_CALLABLE`, and explicit unknown-call remainder.

## 75. Control-flow relationships

Families include entry/exit ownership, `CFG_EDGE`, `TRUE_EDGE`, `FALSE_EDGE`, `SWITCH_EDGE`,
`EXCEPTION_EDGE`, `UNWIND_EDGE`, `CLEANUP_EDGE`, `SUSPEND_EDGE`, and `RESUME_EDGE`.

## 76. Dataflow relationships

Families include `DEFINES`, `USES`, `KILLS`, `REACHES`, `DEF_USE`, `USE_DEF`, `LIVE_AT`,
`VALUE_FLOWS_TO`, and unknown source/sink.

## 77. Memory relationships

Families include `READS`, `WRITES`, `READ_WRITES`, `MOVES`, `COPIES`, `BORROWS`,
`TAKES_ADDRESS`, `PROJECTS`, `POINTS_TO`, `MAY_ALIAS`, and `MUST_ALIAS` only when proved.

## 78. Ownership/lifetime relationships

Families include initialization, move, borrow/loan, region, capture, drop, acquire, release,
transfer, escape, and owner-scoped state relations with exact/private versus derived authority.

## 79. Effect relationships

Families connect callables/program points to direct and transitive effects, exceptional effects,
resource effects, synchronization, unsafe/FFI evidence, and unknown effect.

## 80. Generated/lowered relationships

Families include `GENERATED_FROM`, `EXPANDS_TO`, `LOWERS_TO`, `SYNTHESIZES`, `DESUGARS_TO`,
`SPECIALIZES_TO`, and source/hygiene correspondence.

## 81. Derived graph relationships

Families include reachable, SCC/recursion membership, dominates/post-dominates, control/data
dependence, loop membership, bounded distance, closure/reduction, and interprocedural summary
edges, all with analysis provenance.

## 82. Core conformance

Core conformance requires programmatic schema/transformation assembly, `SchemaContract` closure,
CBEF identity, complete
universal metadata, raw/canonical separation, authority conflict retention, coverage-qualified
negative semantics, explicit unknowns, and one-epoch ownership.

## 83. Python profile conformance

Python conformance requires Tree-sitter/Ruff source evidence, the exact Pyrefly hybrid semantic
surfaces, Python-specific scopes/bindings/types/object model/calls/dynamic remainders, and
separately versioned application-derived CFG/flow analyses.

## 84. Rust profile conformance

Rust conformance requires Tree-sitter syntax, public MIR/semantic evidence, narrow private
stable-key/source-hygiene/borrow-check enrichment, exact trust receipts, Rust identity rules,
and separately versioned application ownership/flow analyses.

## 85. Advanced derived-fact conformance

Derived conformance requires exactly one producer or explicit unsupported remainder for every
accepted analysis/query family; deterministic and bounded algorithms; clean/incremental
equivalence; precision, provenance, and unknown propagation; and independently authored semantic
expectations.

## 86. No agent reasoning inside the ontology

No typed input row, provider adapter, transformation, derivation, query compiler, or proof compiler may ask
an LLM or agent to decide semantic truth. Agent-authored requests and independent reviewed
expectations may be inputs; executable deterministic logic establishes facts and proof results.

## 87. Governing design rule

If a proposed implementation cannot represent a provider-native fact, conflict, remainder,
unknown, or new compiler/grammar variant without changing an unrelated canonical fact family,
the provider/input/transformation contract or schema is too narrow. If execution does not read a
declaration, that declaration
is documentation, not authority.

## AC-G-12 — File identity across replacement, rename, and move

The file-identity behavior in §64 is released. Rename continuity is operational evidence only.
Executable conformance covers content change, atomic replacement, rename/move, delete/recreate,
case-folding collisions, and ambiguous continuity candidates.

## AC-G-13 — Canonical ID preimage serialization

`CBEF-v1`, its released domain recipes, public prefixes, recipe-specific builders, collision
behavior, and exact decode rejection rules remain wire commitments. Identity recipes are explicit
typed inputs consumed by canonical constructors; generated accessors are derived build products only.

## AC-G-15 — Canonical type algebra

The type algebra in §§10, 35, and 47 is a closed typed discriminant set extended only by a reviewed
versioned contract change. Every type row has a canonical structural key, provider raw
rendering where available, language/context identity, and explicit unknown/error variants.

## AC-G-16 — External dependency identity and body policy

External packages/crates/modules/symbols have ecosystem-canonical identity plus exact resolved
version/source/context evidence when available. Missing bodies are explicit external or
unavailable capability, never proof of no behavior.

## AC-G-17 — Cross-language and FFI linking profile

Cross-language links are candidate facts unless exact ABI/name/export evidence proves a target.
They retain source and target language contexts, ABI, symbol spelling, resolution tier, and
unknown remainder; they never infer safety.

## AC-G-18 — Path canonicalization, display, URI, and ordering

Raw/reversible path bytes, comparison keys, display paths, and public URIs are separate.
Descriptor-relative authorization and collision checks precede ingestion. Ordering uses the
declared comparison bytes, not locale-dependent display text.

## AC-G-70 — Executed ontology model

The former machine-registry and replay contract is superseded. The current ontology is the exact
provider batches, explicit typed inputs, typed transformations, and observations derived from the
candidate catalog. Conformance joins those observations to provider, producer, query, and proof
relations and MUST fail on missing, duplicate, dangling, inert, or uncovered rows. No bootstrap,
migration replay, static registry, or suite census may replace this contract.

## AC-G-71 — Property schema, value types, cardinality, null, and storage mapping

Each property/fact family links to one session-derived `SchemaContract` that owns logical Arrow
schema, qualified `DFSchema`, storage schema, logical/physical casts, null/cardinality/key rules,
projection/filter/statistics remapping, output restoration, and validation at plan, stream,
batch, and sink boundaries. Delta `BINARY` cannot silently alter a logical fixed-width ID.

## AC-G-72 — Mandatory conformance profiles

Profiles are typed program selections with dependency closure, not hand-maintained lists. Core,
Python, Rust, advanced-derived, public-query, and wire-compatibility profiles resolve every
required fact family to one producer or explicit remainder and one named executable proof.

## AC-G-73 — Unknown entities, unknown remainder, and explicit negative facts

Every provider/derivation request materializes requested scope, completed scope, remainder,
diagnostic, and unknown relations. A negative fact requires an exhaustive completed scope and a
proof that the selected authority can establish absence. Cancellation, limits, parse/type/
compile failure, unsupported context, and corruption cannot yield an empty-negative result.

## AC-G-74 — Graph projection model

Graph projections are typed transformation inputs compiled into DataFusion-native plans or the highest bounded
graph extension justified by the operation. They define node/edge identity, duplicates,
direction, filters, unknowns, bounds, and schema. petgraph indices are never persisted or
returned.

## AC-G-75 — Interprocedural summary semantics

Each summary family names input/output relations, lattice/order, transfer, join, convergence and
resource bounds, unknown propagation, invalidation closure, algorithm release, and proof. A
non-converged or resource-exhausted summary remains explicit unknown and blocks dependent proof.

## AC-G-76 — Static concurrency and happens-before semantics

Only language- or library-semantics-backed ordering is represented. Static spawn/join/lock/
channel facts and possible happens-before edges carry evidence and completeness; runtime order is
out of scope.

## AC-G-77 — Effect and resource model semantics

Direct effect/resource observations remain separate from application propagation and summaries.
Each family names object/resource abstraction, path sensitivity, exceptional cleanup,
interprocedural policy, precision, and unknown remainder. No leak, risk, or correctness verdict
is emitted.

## Cross-layer integration obligations

The generation layer emits typed provider-native and derived relations conforming to this
ontology. The fabric registers them in one epoch and executes typed normalization/authority/proof
transformations over exact provider batches and explicit inputs. The query layer exposes only
authorized semantic projections while preserving provenance,
unknowns, bounds, deterministic ordering, and epoch pins. Lifecycle replacement is atomic by
owner and never mixes generations.

## Release conformance obligations

Before activation, executable checks SHALL prove:

1. isolated programmatic schema/transformation assembly and candidate-catalog fixed-point closure;
2. complete relation/field/key/reference/`SchemaContract` closure;
3. released ID and wire compatibility for supported `codefabric.cpgd.v2` clients, with no v1
   interoperability claim;
4. exactly one provider or application authority per fact family;
5. requested/completed/remainder/unknown coverage closure;
6. raw/canonical/derived provenance separation and conflict retention;
7. independent Python/Rust semantic expectations and causal mutants;
8. clean versus incremental equivalence under additions, changes, and deletions;
9. one-epoch query/public result equivalence; and
10. zero active reads of predecessor registries, bundles, censuses, or fingerprints.

Zero violations without proved input coverage is `unknown`, not pass.

## Relational ontology projection

The assembler emits typed Arrow contract/program relations and `SchemaContract` links from exact
provider batches, explicit typed inputs, and `ProgrammaticTransformation` values. Rendering source,
Markdown, JSON, Rust, Python, or diagrams from those relations is allowed for navigation or tooling,
but those artifacts are disposable and cannot be imported as production semantic authority.
Reassembling the same batches and typed inputs with the same exact application/provider release
vector must reproduce complete schemas and logical rows; digests locate differences but do not
prove semantic equality.
