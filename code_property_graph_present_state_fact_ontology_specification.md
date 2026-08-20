# Comprehensive Present-State Code Property Graph Ontology Specification

**Status:** Draft normative specification  
**Target languages:** Python and Rust  
**Primary purpose:** Present-state code-intelligence fact substrate for LLM programming agents  
**Artifact type:** Language-neutral core ontology with Python- and Rust-specific extensions  
**Scope boundary:** Facts and mechanically derived facts only; no task-specific or evaluative analysis  

---

## 1. Purpose

This document specifies a comprehensive ontology for a Code Property Graph (CPG) whose primary purpose is to provide LLM programming agents with a maximally rich, semantically precise representation of the **present state of analyzed code**.

The CPG defined here is a **fact substrate**, not an automated software-engineering decision system.

It SHALL represent:

- facts directly harvested from source text;
- facts obtained from parsers, semantic analyzers, type systems, compilers, and intermediate representations;
- facts mechanically derived from graph topology or semantic dataflow;
- deterministic summaries of larger fact sets;
- explicit unresolved or unknown semantic states.

It SHALL NOT attempt to make higher-level judgments such as:

- whether a refactor is safe;
- which tests are impacted by a change;
- whether code is risky or poorly designed;
- whether an architectural dependency should be removed;
- whether a vulnerability is exploitable;
- what code should be changed.

Those conclusions are deliberately left to downstream reasoning systems, including LLM programming agents.

The design objective is:

> **Provide the richest possible present-state semantic evidence from which an intelligent programming agent can perform its own reasoning.**

---

## 2. Normative scope

### 2.1 Included information

The ontology covers the current analyzed program, including:

- source text and lexical structure;
- complete syntax;
- declarations and semantic identities;
- lexical scopes and bindings;
- references and name resolution;
- modules, imports, exports, and code-declared dependencies;
- types and type relationships;
- members, inheritance, traits, protocols, and implementations;
- callables, call sites, arguments, and dispatch;
- control flow;
- values and computations;
- dataflow and def-use;
- state and abstract memory locations;
- aliasing and points-to relationships;
- initialization and program-point state;
- Rust ownership, borrowing, moves, copies, and drops;
- direct and transitive effects;
- exceptions, panic, unwind, and cleanup;
- resource lifetimes;
- async, generator, task, thread, channel, and lock semantics;
- closures and captures;
- generated and lowered code;
- generic specialization and Rust monomorphization;
- macros and macro expansion;
- Rust MIR;
- objective graph-derived facts;
- deterministic semantic summaries;
- explicit unresolved facts.

### 2.2 Excluded information

The ontology excludes the following as first-class CPG fact domains:

#### Historical state

- Git history;
- commit history;
- prior revisions;
- semantic diff across revisions;
- code churn;
- historical hotspots;
- blame information;
- longitudinal evolution.

#### Runtime observation

- runtime execution traces;
- code coverage;
- production profiling;
- sampled values;
- runtime-observed call edges;
- production telemetry.

#### External environment state

- active virtual environments;
- installed package inventories;
- host operating-system state;
- environment variables;
- deployment state;
- current machine-specific configuration;
- live infrastructure state.

Code that *declares or consumes* configuration remains part of the source ontology. For example:

- Rust `#[cfg(...)]` syntax is a source fact;
- Cargo feature declarations are code/project facts;
- Python import statements are code facts;
- environment-variable reads in source are code facts.

The ontology simply does not attempt to model which external environment is currently active.

#### Evaluative or task-oriented conclusions

The ontology excludes conclusions such as:

- `REFACTOR_SAFE`;
- `TEST_IMPACTED`;
- `HIGH_RISK`;
- `GOD_CLASS`;
- `VULNERABLE`;
- `ARCHITECTURALLY_BAD`;
- `SHOULD_REWRITE`;
- `RECOMMENDED_CHANGE`.

Tests themselves remain ordinary code and receive the same semantic representation as all other code.

---

## 3. Definition of a fact

A **fact** is an objective proposition about the present-state analyzed program that is either:

1. directly observable in source;
2. determined by language semantics;
3. exposed by a compiler or semantic engine;
4. mechanically derived from other program facts; or
5. a deterministic summary of another fact set.

The ontology recognizes five broad fact classes.

| Fact class | Definition | Example |
|---|---|---|
| **Source fact** | Directly observable from source text | Identifier `foo` occupies bytes 120–123 |
| **Semantic fact** | Determined by language semantics or a semantic analyzer | This occurrence of `foo` resolves to function `F` |
| **Compiler/lowered fact** | Exposed by compiler or intermediate representation | MIR block `bb7` branches to `bb9` and `bb10` |
| **Derived graph fact** | Deterministically computed from other graph facts | Basic block `B3` dominates `B14` |
| **Summary fact** | Deterministic compression of a larger fact set | Function `F` may write fields `{x, y}` |

The following are facts:

```text
CALLS_EXACT(F, G)
TRANSITIVELY_REACHES(F, G)
SAME_CALL_SCC(F, G)
DOMINATES(B1, B9)
MAY_ALIAS(X, Y)
```

The following are not facts within this specification:

```text
CHANGING_G_IS_RISKY_FOR_F
THIS_REFACTOR_IS_SAFE
THIS_TEST_IS_RELEVANT
THIS_MODULE_SHOULD_BE_SPLIT
```

---

## 4. Design principles

### 4.1 Present-state only

All facts describe one analyzed program state.

The graph MAY contain internal generation or freshness metadata required for consistency, but historical states are outside this ontology.

### 4.2 Raw and normalized representations SHALL coexist

The ontology SHALL preserve both:

- the provider-native or language-native representation; and
- a normalized semantic representation.

This is required for future completeness.

For syntax:

```text
SYNTAX_NODE
  raw_language_kind
  normalized_kind
  source_span
  frontend_fields
```

For Rust MIR:

```text
MIR_NODE
  raw_mir_variant
  normalized_semantic_kind
  owner
  source_correspondence
```

A normalized enumeration MUST NOT prevent representation of newly introduced Python grammar nodes or Rust compiler variants.

### 4.3 Syntax occurrence and semantic entity SHALL remain distinct

An identifier occurrence in source is not the same object as the semantic declaration it denotes.

Likewise:

- call syntax is not a callable;
- type syntax is not a semantic type;
- a member-access expression is not the member declaration;
- a generic declaration is not a monomorphized instance.

### 4.4 Call sites SHALL be first-class entities

The graph MUST NOT reduce call semantics solely to caller-to-callee edges.

A call site carries essential information:

- source location;
- receiver;
- arguments;
- binding of arguments to parameters;
- dispatch mechanism;
- declared target;
- exact target;
- candidate target set;
- unknown target state.

Derived caller-to-callee relations MAY be materialized for convenience.

### 4.5 Unknown is a first-class fact

Absence of a resolved relationship MUST NOT be used to imply impossibility.

For example:

```text
MAY_CALL -> UNKNOWN_CALL_TARGET
REFERS_TO -> UNKNOWN_SYMBOL
MAY_POINT_TO -> UNKNOWN_MEMORY
MAY_RESOLVE_MEMBER -> UNKNOWN_MEMBER
```

is superior to silently omitting unresolved facts.

### 4.6 Direct and transitive facts SHALL remain distinguishable

For example:

```text
DIRECTLY_WRITES
TRANSITIVELY_WRITES

DIRECTLY_CALLS
TRANSITIVELY_CALLS

DIRECT_EFFECT
TRANSITIVE_EFFECT
```

A downstream agent must be able to distinguish behavior authored in the subject itself from behavior inherited through callees.

### 4.7 Objective derivation is permitted; evaluative interpretation is not

The following are valid derived facts:

- SCC membership;
- dominance;
- post-dominance;
- control dependence;
- reaching definitions;
- liveness;
- points-to sets;
- alias sets;
- transitive reachability;
- loop structure;
- recursion;
- mechanically computed metrics.

The following are not:

- risky;
- fragile;
- safe;
- bad architecture;
- likely impacted;
- recommended.

---

# Part I — Language-Neutral Core Ontology

## 5. Source and lexical ontology

### 5.1 Core entities

#### `SOURCE_FILE`

Represents one analyzed source file.

Required conceptual properties:

- stable file identity within the analyzed codebase;
- repository-relative or workspace-relative path;
- language;
- source length;
- source digest or equivalent present-state identity.

#### `SOURCE_SPAN`

Represents a half-open byte interval in a specific source file.

Conceptual properties:

```text
file
start_byte
end_byte
```

Line and column positions MAY be derived presentation properties but byte offsets SHOULD remain canonical.

#### `TOKEN`

Represents a lexical token.

Properties:

- raw token kind;
- normalized token kind;
- span;
- lexical text or text reference;
- ordinal within file.

Specialized token categories MAY include:

```text
IDENTIFIER_TOKEN
KEYWORD_TOKEN
OPERATOR_TOKEN
PUNCTUATION_TOKEN
LITERAL_TOKEN
STRING_TOKEN
NUMBER_TOKEN
```

#### `COMMENT`

Represents a source comment, preserving exact source range and text.

#### `DOCUMENTATION`

Represents language-recognized documentation constructs such as:

- Python docstrings;
- Rust doc comments;
- documentation attributes.

#### `PRAGMA_OR_DIRECTIVE`

Represents source directives such as:

- Python type comments;
- Python `type: ignore`;
- Rust attributes;
- Rust `cfg` declarations;
- language pragmas.

#### `PARSE_ERROR`

Represents parser-recognized invalid syntax.

#### `MISSING_SYNTAX`

Represents parser-synthesized missing syntax used during error recovery.

### 5.2 Core lexical relationships

```text
CONTAINS_SPAN
TOKEN_OF
LEXICALLY_PRECEDES
DOCUMMENTS / DOCUMENTS
DIRECTIVE_APPLIES_TO
```

A provider MAY expose stronger lexical ordering relationships, but the canonical ontology SHOULD preserve source ordering deterministically.

---

## 6. Syntax ontology

The ontology SHALL be capable of representing every syntax construct supported by the language frontend.

### 6.1 Universal syntax entities

```text
SYNTAX_NODE
STATEMENT
EXPRESSION
PATTERN
DECLARATION_SYNTAX
TYPE_SYNTAX
PARAMETER_SYNTAX
ARGUMENT_SYNTAX
BLOCK
LITERAL
OPERATION
ATTRIBUTE_ACCESS
MEMBER_ACCESS
SUBSCRIPT_ACCESS
INDEX_ACCESS
CALL_EXPRESSION
ASSIGNMENT
BRANCH
LOOP
RETURN
YIELD
AWAIT
RAISE_OR_PANIC_SYNTAX
IMPORT_OR_USE_SYNTAX
```

The normalized kind hierarchy MAY be more detailed, but no provider-native node kind may become unrepresentable.

### 6.2 Required syntax-node properties

```text
language
raw_kind
normalized_kind
source_span
is_named
is_error
is_missing
```

Optional provider-specific properties MAY include:

```text
grammar_field_name
frontend_node_id
parse_state
extra/trivia flags
```

Provider-local IDs MUST NOT be treated as durable semantic identity.

### 6.3 Structural relationships

#### `AST_CHILD(parent, child, field_name, ordinal)`

Canonical ordered syntax containment.

`field_name` SHALL be retained where the language frontend provides it.

Examples:

```text
condition
body
target
value
receiver
callee
argument
decorator
return_type
pattern
guard
```

Additional structural convenience relations MAY include:

```text
PARENT_OF
ENCLOSES
LEXICAL_NEXT
```

---

## 7. Semantic identity ontology

Syntax nodes represent occurrences. Semantic nodes represent language-level entities.

### 7.1 Core semantic entity kinds

```text
MODULE
NAMESPACE
SCOPE

SYMBOL
DECLARATION
BINDING
REFERENCE

FUNCTION
METHOD
CLOSURE
LAMBDA
CONSTRUCTOR
PARAMETER

CLASS
STRUCT
ENUM
UNION
TRAIT
PROTOCOL
INTERFACE
ENUM_VARIANT
FIELD
PROPERTY
MEMBER

VARIABLE
LOCAL
GLOBAL
STATIC
CONSTANT

TYPE_ALIAS
TYPE_PARAMETER
LIFETIME_PARAMETER
CONST_PARAMETER

EXTERNAL_SYMBOL
SYNTHESIZED_SYMBOL
GENERATED_SYMBOL
```

Not every language uses every kind.

### 7.2 Common semantic properties

```text
name
qualified_name
semantic_kind
visibility
mutability
source_span
name_span

is_external
is_generated
is_synthesized
```

Language-specific modifiers MAY include:

```text
async
unsafe
const
static
final
abstract
classmethod
extern
default
```

### 7.3 Core ownership relationships

```text
DECLARES
DEFINED_IN
OWNED_BY
CONTAINS
HAS_SCOPE
ENCLOSING_SCOPE
```

---

## 8. Scope, binding, and name-resolution ontology

### 8.1 Scope facts

A `SCOPE` represents a language-recognized lexical or semantic name-resolution domain.

Scope facts include:

- scope kind;
- parent scope;
- declared bindings;
- visible bindings;
- free variables;
- captured variables;
- shadowing relationships.

### 8.2 Binding relationships

```text
BINDS
REFERS_TO
MAY_REFER_TO
SHADOWS
CAPTURES
CAPTURED_FROM
ALIASES
REBINDS
```

### 8.3 Reference classification

Identifier/reference occurrences SHOULD be classifiable as:

```text
declaration
definition
read
write
read_write
import_binding
parameter_binding
capture
type_reference
call_reference
member_reference
```

### 8.4 Unresolved references

An unresolved occurrence SHALL remain explicit:

```text
REFERS_TO -> UNKNOWN_SYMBOL
```

or, when multiple candidates remain:

```text
MAY_REFER_TO -> candidate_1
MAY_REFER_TO -> candidate_2
```

---

## 9. Module, import, export, and dependency ontology

### 9.1 Entities

```text
MODULE
PACKAGE
CRATE
IMPORT_DECLARATION
IMPORT_BINDING
EXPORT
REEXPORT
EXTERNAL_DEPENDENCY_REFERENCE
```

### 9.2 Relationships

```text
IMPORTS_MODULE
IMPORTS_SYMBOL
EXPORTS
REEXPORTS
ALIASES
DEFINED_IN_MODULE
DEPENDS_ON_MODULE
```

### 9.3 Required distinction

The following SHALL remain semantically distinct:

```text
import/use syntax
local imported binding
resolved module
resolved imported symbol
re-exported symbol
```

A single import syntax occurrence may therefore produce several semantic facts.

---

## 10. Type ontology

Types SHALL be graph entities, not merely strings.

### 10.1 Common type families

```text
UNKNOWN_TYPE
ERROR_TYPE
ANY_OR_DYNAMIC
NEVER_OR_BOTTOM
NULL_OR_NONE

PRIMITIVE_TYPE
NOMINAL_TYPE
CLASS_OBJECT_TYPE
TYPE_OBJECT

LITERAL_TYPE
UNION_TYPE
INTERSECTION_TYPE

CALLABLE_TYPE
BOUND_METHOD_TYPE

TUPLE_TYPE
ARRAY_TYPE
LIST_TYPE
SEQUENCE_TYPE
MAPPING_TYPE
STRUCTURAL_TYPE

GENERIC_TYPE
TYPE_PARAMETER
TYPE_VARIABLE
ASSOCIATED_TYPE
TYPE_ALIAS

REFERENCE_TYPE
POINTER_TYPE
```

### 10.2 Type relationships

```text
DECLARED_TYPE
INFERRED_TYPE
COMPUTED_TYPE
EXPECTED_TYPE
TYPE_OF

PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE

TYPE_PARAMETER_OF
TYPE_ARGUMENT
INSTANTIATES

SUBTYPE_OF
SUPERTYPE_OF
BOUNDED_BY
CONSTRAINED_BY

COERCES_TO
CASTS_TO
NARROWS_TO
```

### 10.3 Distinct type concepts

The graph SHALL preserve separately, where available:

- declared type;
- inferred/computed type;
- expected/contextual type.

These facts SHALL NOT be collapsed into one `HAS_TYPE` fact unless the original distinctions remain recoverable.

---

## 11. Member and object-model ontology

### 11.1 Entities

```text
MEMBER
FIELD
METHOD
PROPERTY
DESCRIPTOR
ASSOCIATED_ITEM
```

### 11.2 Relationships

```text
DECLARES_MEMBER
HAS_MEMBER
INHERITS
IMPLEMENTS
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD
OVERRIDES
OVERRIDDEN_BY

RESOLVES_MEMBER
MAY_RESOLVE_MEMBER
```

### 11.3 Member properties

Potential objective properties include:

```text
visibility
static_or_instance_status
class_member_status
read_only
writeable
final
abstract
receiver_type
declaring_type
resolved_owner_type
```

Language-specific member resolution SHALL be permitted without forcing Python and Rust into identical mechanics.

---

## 12. Callable contract ontology

Every callable SHOULD expose a complete, objective invocation contract.

### 12.1 Callable properties

```text
name
qualified_name

parameter_count
parameter_ordering
parameter_kinds
default_values_or_default_expressions

receiver_semantics
variadic_status

generic_parameters
return_type

async_status
generator_status

ABI_or_calling_convention
unsafe_status
const_status
```

### 12.2 Relationships

```text
HAS_PARAMETER
RETURNS_TYPE
HAS_TYPE_PARAMETER
HAS_GENERIC_CONSTRAINT
CAPTURES
```

---

## 13. Call-site ontology

Call sites SHALL be first-class graph entities.

### 13.1 Entities

```text
CALL_SITE
CALLEE_EXPRESSION
RECEIVER
ARGUMENT
ARGUMENT_BINDING
```

### 13.2 Relationships

```text
CONTAINS_CALL
HAS_CALLEE_EXPRESSION
HAS_RECEIVER
HAS_ARGUMENT
ARGUMENT_BINDS_TO

CALLS_DECLARATION
CALLS_EXACT_TARGET
CALLS_INSTANCE
MAY_CALL
CALLS_UNKNOWN

REFERENCES_CALLABLE
TAKES_FUNCTION_ADDRESS
PASSES_CALLABLE
RETURNS_CALLABLE
```

### 13.3 Call-site properties

```text
source_span
call_syntax_kind
dispatch_kind
resolved_target_count
resolution_status
```

### 13.4 Derived caller/callee facts

The graph MAY materialize convenience relations:

```text
DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE
```

but these SHALL be derivable from call-site facts.

---

## 14. Dispatch ontology

Dispatch mechanism SHALL be explicit.

### 14.1 Common dispatch kinds

```text
DIRECT
STATIC_METHOD
BOUND_METHOD
CONSTRUCTOR
CLOSURE
FUNCTION_POINTER
CALLABLE_OBJECT
STATIC_TRAIT
DYNAMIC_TRAIT
VTABLE
VIRTUAL_OVERRIDE
INTRINSIC
FOREIGN
COMPILER_SHIM
DROP_GLUE
UNKNOWN_DYNAMIC
```

### 14.2 Dispatch facts

```text
dispatch_mechanism
declared_target
resolved_target
possible_target_set
receiver_type
target_instance
```

The graph SHALL distinguish:

- declared contract target;
- exact executable target;
- possible target set.

---

## 15. Control-flow ontology

### 15.1 Entities

```text
CONTROL_FLOW_GRAPH
ENTRY
EXIT
BASIC_BLOCK
INSTRUCTION
OPERATION
BRANCH
SWITCH
LOOP_HEADER
RETURN_POINT
EXCEPTIONAL_EXIT
```

### 15.2 CFG relationships

```text
CFG_NEXT
CFG_TRUE
CFG_FALSE
CFG_CASE
CFG_LOOP_BACK
CFG_BREAK
CFG_CONTINUE
CFG_RETURN
CFG_EXCEPTION
CFG_UNWIND
CFG_CALL_RETURN
```

Normal and exceptional control flow SHALL remain distinct.

---

## 16. Derived control-flow facts

The following mechanically derived facts are part of the ontology.

```text
PREDECESSOR
SUCCESSOR

REACHABLE_BLOCK
UNREACHABLE_BLOCK

DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_DOMINATOR

POST_DOMINATES
IMMEDIATE_POST_DOMINATOR

CONTROL_DEPENDENT_ON

BACK_EDGE
LOOP_MEMBER
LOOP_HEADER
LOOP_NESTING_DEPTH

CFG_SCC_MEMBER
```

### 16.1 Reachability scope

`UNREACHABLE_BLOCK` is meaningful only relative to a defined CFG root or owner. The graph MUST preserve sufficient ownership context to avoid ambiguous global claims.

---

## 17. Value and computation ontology

### 17.1 Value entities

```text
VALUE
CONSTANT_VALUE
PARAMETER_VALUE
RETURN_VALUE
TEMPORARY_VALUE
MERGED_VALUE
UNKNOWN_VALUE
```

### 17.2 Computation entities

```text
UNARY_OPERATION
BINARY_OPERATION
COMPARISON
CAST_OPERATION
COERCION_OPERATION
AGGREGATE_OR_CONSTRUCTION
INDEX_OPERATION
FIELD_ACCESS_OPERATION
```

### 17.3 Relationships

```text
PRODUCES_VALUE
CONSUMES_VALUE
OPERAND
RESULT
```

This layer allows source expressions and lowered/compiler values to participate in one common value-flow model.

---

## 18. Definition/use and dataflow ontology

### 18.1 Entities

```text
DEFINITION_EVENT
USE_EVENT
```

### 18.2 Definition categories

```text
initialization
assignment
parameter_initialization
mutation
return_assignment
merged_definition
```

### 18.3 Use categories

```text
read
argument
condition
return
receiver
index
dereference
```

### 18.4 Relationships

```text
DEFINES
USES
REACHES
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
```

### 18.5 Derived dataflow facts

```text
REACHING_DEFINITION
LIVE_AT
KILLS_DEFINITION
```

The graph MAY include SSA-like derived structures, but SHALL NOT require source languages themselves to be represented in SSA form.

---

## 19. Abstract memory and state-location ontology

### 19.1 Location entities

```text
LOCAL_LOCATION
PARAMETER_LOCATION
GLOBAL_LOCATION
STATIC_LOCATION

FIELD_LOCATION
INSTANCE_MEMBER_LOCATION
CLASS_MEMBER_LOCATION

INDEXED_LOCATION
CONTAINER_ELEMENT_LOCATION

DEREFERENCED_LOCATION
HEAP_OBJECT
UNKNOWN_MEMORY
```

### 19.2 Access paths

Memory paths SHALL be representable structurally.

Conceptual example:

```text
base
  .field
  [index]
  *
  downcast
  subslice
```

An expression such as:

```text
object.x.y[i]
```

SHOULD be representable as a structured access path rather than an opaque string.

### 19.3 Memory relationships

```text
READS
WRITES
MUTATES
INITIALIZES
DEINITIALIZES
TAKES_ADDRESS
DEREFERENCES
```

---

## 20. Alias and points-to ontology

### 20.1 Relationships

```text
MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS
POINTS_TO
MAY_POINT_TO
```

`DOES_NOT_ALIAS` SHALL only be asserted when proven under the analysis model.

### 20.2 Derived structures

```text
ALIAS_SET
POINTS_TO_SET
```

The ontology favors conservative uncertainty over false precision.

---

## 21. Program-point state ontology

Objective state facts MAY include:

```text
INITIALIZED_AT
UNINITIALIZED_AT
MAY_BE_UNINITIALIZED_AT

KNOWN_CONSTANT_AT
POSSIBLE_CONSTANT_SET

NULL_AT
NON_NULL_AT
MAY_BE_NULL_AT

VARIANT_AT
POSSIBLE_VARIANTS_AT
```

These facts are relative to a program point and SHALL retain the corresponding control-flow location.

---

## 22. Effect ontology

Effects describe observable program behavior without evaluating whether that behavior is desirable.

### 22.1 Direct effect kinds

```text
READS_STATE
WRITES_STATE
MUTATES_ARGUMENT

ALLOCATES
DEALLOCATES

MAY_RAISE
MAY_PANIC
MAY_UNWIND

PERFORMS_IO
MAY_BLOCK

SPAWNS_TASK
SPAWNS_THREAD
AWAITS

ACQUIRES_LOCK
RELEASES_LOCK

CALLS_FOREIGN_CODE
USES_UNSAFE_OPERATION
USES_INLINE_ASSEMBLY
```

### 22.2 Direct versus transitive effects

The ontology SHOULD distinguish:

```text
DIRECT_EFFECT
TRANSITIVE_EFFECT
```

For example:

```text
DIRECTLY_WRITES(function, location)
TRANSITIVELY_WRITES(function, location)
```

---

## 23. Exceptional-flow ontology

### 23.1 Entities

```text
RAISE_SITE
PANIC_SITE
ASSERT_SITE
HANDLER
CATCH_CLAUSE
EXCEPT_CLAUSE
FINALLY_REGION
CLEANUP_REGION
UNWIND_EDGE
```

### 23.2 Relationships

```text
RAISES
MAY_RAISE
HANDLED_BY
MAY_BE_HANDLED_BY
PROPAGATES_TO
UNWINDS_TO
EXECUTES_CLEANUP
```

This layer represents mechanism only. It does not assign risk.

---

## 24. Resource-lifetime ontology

### 24.1 Entities

```text
RESOURCE_CREATION
RESOURCE_ACQUISITION
RESOURCE_USE
RESOURCE_RELEASE
RESOURCE_DROP
```

### 24.2 Relationships

```text
CREATES_RESOURCE
ACQUIRES_RESOURCE
OWNS_RESOURCE
TRANSFERS_RESOURCE
USES_RESOURCE
RELEASES_RESOURCE
DROPS_RESOURCE
```

No `RESOURCE_LEAK` conclusion is required by this specification.

---

## 25. Async and concurrency ontology

### 25.1 Entities

```text
COROUTINE
FUTURE
GENERATOR
TASK
THREAD
CHANNEL
LOCK
```

### 25.2 Relationships

```text
CREATES_FUTURE
SPAWNS
AWAITS
YIELDS
RESUMES
JOINS

SENDS
RECEIVES

ACQUIRES
RELEASES
```

### 25.3 Derived concurrency relationships

Where supported:

```text
MAY_RUN_CONCURRENTLY_WITH
HAPPENS_BEFORE
```

These SHALL remain mechanically justified relations, not inferred performance or correctness judgments.

---

## 26. Closure and capture ontology

### 26.1 Entities

```text
CLOSURE
CAPTURE
CAPTURED_SYMBOL
```

### 26.2 Relationships

```text
CAPTURES
CAPTURED_FROM
CAPTURES_BY_VALUE
CAPTURES_BY_REFERENCE
CAPTURES_MUTABLY
```

Language profiles define which capture modes are meaningful.

---

## 27. Generated and lowered-code ontology

### 27.1 Entities

```text
SOURCE_ENTITY
GENERATED_ENTITY
EXPANSION
LOWERED_ENTITY
COMPILER_INSTANCE
```

### 27.2 Relationships

```text
GENERATED_FROM
EXPANDED_FROM
LOWERS_TO
CORRESPONDS_TO
SPECIALIZES
MONOMORPHIZES
```

The ontology SHALL preserve the ability to map generated/lowered entities back to source-authored constructs where the provider exposes such provenance.

---

## 28. Generic and specialization ontology

### 28.1 Entities

```text
GENERIC_DECLARATION
GENERIC_PARAMETER
GENERIC_ARGUMENT
SPECIALIZATION
```

### 28.2 Relationships

```text
HAS_GENERIC_PARAMETER
TYPE_ARGUMENT
CONST_ARGUMENT
LIFETIME_ARGUMENT
INSTANTIATES
SPECIALIZES
SUBSTITUTES
```

A generic declaration and a concrete specialization SHALL remain distinct entities.

---

## 29. Objective graph-analysis facts

Mechanically computed graph structure is explicitly part of the ontology.

### 29.1 Generic graph facts

```text
IN_DEGREE
OUT_DEGREE

SCC_ID
SCC_SIZE
IS_RECURSIVE_SCC

CONNECTED_COMPONENT

TRANSITIVELY_REACHES
TRANSITIVELY_REACHED_BY

SHORTEST_GRAPH_DISTANCE
```

These SHOULD identify the graph projection on which they were computed.

### 29.2 Call-graph-specific facts

```text
DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE

CALL_SCC
RECURSIVE_FUNCTION
MUTUALLY_RECURSIVE_SET
```

### 29.3 Control-graph-specific facts

```text
DOMINATES
POST_DOMINATES
CONTROL_DEPENDENT_ON
BACK_EDGE
LOOP_MEMBER
CFG_SCC_MEMBER
```

---

## 30. Objective structural metrics

Mechanically derived scalar measurements MAY be included.

Recommended examples:

```text
statement_count
expression_count
basic_block_count
cfg_edge_count

cyclomatic_complexity
loop_count
loop_nesting_depth

direct_call_count
unique_direct_callee_count
direct_caller_count

parameter_count
generic_parameter_count

branch_count
return_count
raise_or_panic_count

read_count
write_count
```

The ontology explicitly excludes evaluative labels derived from these metrics.

For example, it MAY store:

```text
cyclomatic_complexity = 18
```

but SHALL NOT canonically infer:

```text
HIGH_COMPLEXITY = true
```

---

## 31. Interprocedural summary ontology

Interprocedural summaries are deterministic compressed facts intended to reduce repeated traversal costs.

### 31.1 Recommended callable summary contents

```text
direct_callees
may_callees

direct_reads
transitive_reads

direct_writes
transitive_writes

parameters_read
parameters_mutated

possible_return_types
possible_return_values_or_value_classes

may_allocate
may_deallocate

may_perform_io
may_block

may_raise
may_panic
may_unwind

may_spawn
may_await

may_use_unsafe
may_cross_ffi

unknown_effect
```

### 31.2 Summary rules

A summary SHALL remain attributable to the underlying fact families from which it was computed.

A summary MUST NOT replace the lower-level facts needed to explain or recompute it.

---

## 32. Explicit unknown ontology

### 32.1 Unknown entity classes

```text
UNKNOWN_SYMBOL
UNKNOWN_TYPE
UNKNOWN_CALL_TARGET
UNKNOWN_MEMBER
UNKNOWN_MODULE
UNKNOWN_MEMORY
UNKNOWN_EFFECT
UNKNOWN_EXTERNAL_IMPLEMENTATION
```

### 32.2 Unknown relationships

Examples:

```text
MAY_CALL -> UNKNOWN_CALL_TARGET
MAY_RESOLVE_MEMBER -> UNKNOWN_MEMBER
REFERS_TO -> UNKNOWN_SYMBOL
MAY_POINT_TO -> UNKNOWN_MEMORY
```

Unknown facts SHALL be preserved instead of represented as absent edges.

---

# Part II — Python Ontology Profile

## 33. Python scope ontology

Python-specific scope kinds include:

```text
MODULE_SCOPE
FUNCTION_SCOPE
CLASS_SCOPE
LAMBDA_SCOPE
COMPREHENSION_SCOPE
ANNOTATION_SCOPE
TYPE_PARAMETER_SCOPE
```

The ontology SHALL preserve Python's language-specific scoping semantics rather than approximating every scope as a generic block scope.

---

## 34. Python binding ontology

Python-specific binding kinds include:

```text
LOCAL_BINDING
PARAMETER_BINDING
GLOBAL_BINDING
NONLOCAL_BINDING

IMPORT_BINDING

CLASS_MEMBER_BINDING
INSTANCE_MEMBER_BINDING

COMPREHENSION_TARGET
LOOP_TARGET
WITH_TARGET
EXCEPTION_TARGET
MATCH_CAPTURE
WALRUS_BINDING

TYPE_PARAMETER_BINDING
TYPE_ALIAS_BINDING

FREE_VARIABLE
CELL_VARIABLE
BUILTIN_REFERENCE
```

The graph SHOULD preserve whether a binding is declaration-like, assignment-like, imported, captured, or synthesized.

---

## 35. Python type ontology extensions

Python-specific type kinds SHOULD include:

```text
ANY
UNKNOWN
NEVER
NONE_TYPE

CLASS_INSTANCE
CLASS_OBJECT
MODULE_TYPE

LITERAL_TYPE
UNION_TYPE
INTERSECTION_TYPE

CALLABLE
BOUND_METHOD
OVERLOAD

TYPE_VAR
PARAM_SPEC
TYPE_VAR_TUPLE
SELF

PROTOCOL
TYPED_DICT
TYPE_ALIAS

ANNOTATED
UNPACK
TYPE_GUARD
TYPE_IS
```

Where possible, the graph SHOULD preserve provenance distinguishing:

- explicit annotation;
- inferred type;
- contextual expected type;
- narrowing result.

---

## 36. Python object-model ontology

Python-specific relationships include:

```text
MRO_PRECEDES
METACLASS_OF

DESCRIPTOR_FOR
PROPERTY_FOR
GETTER_FOR
SETTER_FOR
DELETER_FOR

CLASS_METHOD_OF
STATIC_METHOD_OF

RESOLVES_ATTRIBUTE
MAY_RESOLVE_ATTRIBUTE
```

### 36.1 Attribute resolution

Attribute/member resolution SHOULD preserve:

- receiver type;
- declaring class;
- MRO resolution;
- descriptor/property semantics;
- instance versus class binding;
- dynamic/unknown fallback.

---

## 37. Python call ontology

Python-specific call kinds include:

```text
DIRECT_FUNCTION_CALL
BOUND_METHOD_CALL
CLASS_METHOD_CALL
STATIC_METHOD_CALL

CONSTRUCTOR_CALL
CALLABLE_OBJECT_CALL

SUPER_CALL
DECORATOR_APPLICATION

ASYNC_FUNCTION_CALL
GENERATOR_CREATION
```

Constructor call semantics MAY separately model:

```text
__new__
__init__
```

when statically resolvable.

Callable-object invocation MAY separately resolve:

```text
__call__
```

---

## 38. Python dynamic-semantics facts

The graph SHOULD explicitly represent syntax/semantics associated with dynamic behavior.

```text
USES_EVAL
USES_EXEC

USES_GETATTR
USES_SETATTR
USES_DELATTR
USES___DICT__

USES_GLOBALS
USES_LOCALS
USES_VARS

DYNAMIC_IMPORT
STAR_IMPORT

MONKEY_PATCH_WRITE
DYNAMIC_ATTRIBUTE_WRITE
```

These are factual observations.

The ontology SHALL NOT infer a generic negative quality or danger label from them.

Unknown-target relationships SHOULD be retained when these constructs prevent complete static resolution.

---

## 39. Python decorator ontology

Decorators SHALL be represented through at least two distinct semantic relationships:

```text
DECORATED_BY
DECORATOR_APPLICATION_CALL
```

The first captures the structural declaration relationship.

The second captures the executable semantics of decorator application.

Framework-generated behavior MAY be represented using synthesized semantic entities when a semantic provider can identify them objectively.

---

## 40. Python pattern-matching ontology

Entities:

```text
MATCH_STATEMENT
MATCH_CASE
PATTERN
PATTERN_BINDING
GUARD
```

Relationships SHOULD connect:

- match subject;
- case;
- pattern;
- bindings introduced by the pattern;
- guard;
- corresponding control-flow edges.

---

## 41. Python comprehension ontology

A comprehension SHOULD represent:

```text
COMPREHENSION
COMPREHENSION_SCOPE
GENERATOR_CLAUSE
COMPREHENSION_TARGET
COMPREHENSION_ITERABLE
COMPREHENSION_FILTER
COMPREHENSION_RESULT
```

Comprehension-local bindings SHALL remain distinct from surrounding-scope bindings.

---

## 42. Python context-manager ontology

Entities and relationships MAY include:

```text
CONTEXT_MANAGER
ENTER_CALL
EXIT_CALL
ASYNC_ENTER_CALL
ASYNC_EXIT_CALL
```

with statically resolved targets where available.

The graph SHOULD preserve exceptional-control relationships through context-manager exit logic where derivable.

---

## 43. Python async and generator ontology

Python SHOULD distinguish:

```text
ASYNC_FUNCTION
COROUTINE_OBJECT
AWAIT_SITE
ASYNC_ITERATOR
ASYNC_CONTEXT_MANAGER

GENERATOR_FUNCTION
GENERATOR_OBJECT
YIELD_SITE
YIELD_FROM_SITE
```

Calling an async or generator function and executing/resuming its body SHALL remain distinct facts.

---

# Part III — Rust Ontology Profile

## 44. Rust source-semantic entities

Rust-specific source entities include:

```text
CRATE
MODULE
USE_DECLARATION

FUNCTION
METHOD
CLOSURE

STRUCT
ENUM
UNION
VARIANT
FIELD

TRAIT
IMPL
ASSOCIATED_FUNCTION
ASSOCIATED_TYPE
ASSOCIATED_CONST

TYPE_ALIAS
OPAQUE_TYPE

CONST
STATIC

MACRO_DECLARATION
MACRO_INVOCATION
MACRO_EXPANSION

EXTERN_BLOCK
FOREIGN_FUNCTION
```

---

## 45. Rust declaration properties

Rust semantic declarations MAY expose:

```text
visibility
mutability
unsafe
async
const
extern
ABI
variadic
defaultness
repr
```

Attributes SHALL be represented as source facts and associated with the declaration or syntax construct to which they apply.

---

## 46. Rust generic ontology

Rust generic entities include:

```text
TYPE_PARAMETER
LIFETIME_PARAMETER
CONST_PARAMETER
WHERE_PREDICATE
TRAIT_BOUND
LIFETIME_BOUND
```

Relationships include:

```text
BOUNDED_BY
OUTLIVES
IMPLEMENTS
ASSOCIATED_WITH
```

Generic arguments SHALL remain structured:

```text
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT
```

---

## 47. Rust type ontology extensions

Rust-specific normalized type kinds SHOULD include:

```text
BOOL
CHAR
INTEGER
FLOAT
STR
NEVER

ADT
TUPLE
ARRAY
SLICE

REFERENCE
RAW_POINTER

FN_DEF
FN_POINTER

CLOSURE_TYPE
COROUTINE_TYPE

DYN_TRAIT
OPAQUE_TYPE

GENERIC_PARAMETER
ASSOCIATED_TYPE
PROJECTION_TYPE
TYPE_ALIAS
```

Additional type facts MAY include:

```text
mutability
region_or_lifetime
generic_arguments
ABI
```

### 47.1 Type-adjustment relationships

```text
AUTO_DEREF_TO
AUTO_REF_TO
UNSIZES_TO
COERCES_TO
REIFIES_FN_POINTER
```

These SHOULD be preserved where rustc exposes them reliably.

---

## 48. Rust MIR ontology

MIR is a semantic/control-flow layer attached to Rust source-level definitions.

### 48.1 MIR entities

```text
MIR_BODY
MIR_LOCAL
MIR_BASIC_BLOCK
MIR_STATEMENT
MIR_TERMINATOR

PLACE
PLACE_PROJECTION

OPERAND
RVALUE

MIR_CALL_SITE
DROP_SITE
ASSERT_SITE
```

### 48.2 MIR ownership

Every MIR entity SHALL be attributable to its MIR body and source-level semantic owner where correspondence exists.

### 48.3 Raw MIR variants

Each MIR statement, terminator, operand, rvalue, and projection SHOULD preserve its provider-native/raw variant identifier in addition to normalized meaning.

---

## 49. Rust place and projection ontology

A Rust MIR `Place` SHALL be represented as:

```text
base_local
projection_1
projection_2
...
```

Projection kinds include:

```text
DEREF
FIELD
INDEX
CONSTANT_INDEX
SUBSLICE
DOWNCAST
OPAQUE_CAST
```

Thus a place such as conceptually:

```text
x.foo[i].bar
```

is a structured memory/access path rather than a serialized string.

---

## 50. Rust MIR state-transition ontology

The following SHALL remain semantically distinct:

```text
READ
WRITE

COPY
MOVE

BORROW_SHARED
BORROW_MUT
REBORROW

RAW_ADDRESS_OF

STORAGE_LIVE
STORAGE_DEAD

INIT
DEINIT

DROP
```

In particular:

> `MOVE` and `COPY` MUST NOT be collapsed.

---

## 51. Rust ownership and borrow ontology

Where compiler-semantic facts support it, the graph SHOULD represent:

```text
OWNS
MOVED_TO
COPIED_TO

BORROWS_SHARED
BORROWS_MUTABLY
REBORROWS

LOAN
LOAN_CREATED_AT
LOAN_LIVE_AT

REGION
OUTLIVES
REGION_CONTAINS

MOVE_PATH
```

### 51.1 Program-point ownership state

Derived facts MAY include:

```text
OWNED_AT
MOVED_AT
BORROWED_SHARED_AT
BORROWED_MUT_AT
UNINITIALIZED_AT
```

Such facts SHALL retain program-point identity.

---

## 52. Rust call and executable-instance ontology

### 52.1 Rust dispatch/call kinds

```text
DIRECT_FN
STATIC_TRAIT_DISPATCH
DYNAMIC_TRAIT_DISPATCH

FN_POINTER
CLOSURE

INTRINSIC
FOREIGN_CALL

DROP_GLUE
COMPILER_SHIM
COROUTINE_RESUME
UNKNOWN_INDIRECT
```

### 52.2 Definition versus instance

The ontology SHALL distinguish:

```text
DECLARED_FUNCTION
MONO_INSTANCE
```

Relationships include:

```text
MONOMORPHIZES
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT
CALLS_INSTANCE
```

A generic source body SHOULD remain represented once at the source-semantic level even when multiple executable specializations exist.

---

## 53. Rust trait and dynamic-dispatch ontology

### 53.1 Entities

```text
TRAIT
TRAIT_METHOD
IMPL
IMPL_METHOD
DYN_TRAIT_TYPE
VTABLE
VTABLE_ENTRY
```

### 53.2 Relationships

```text
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD

INVOKES_TRAIT_CONTRACT
STATICALLY_RESOLVES_TO

UNSIZES_TO_DYN
USES_VTABLE

MAY_DISPATCH_TO
```

The graph SHALL distinguish:

- static trait resolution;
- dynamic trait dispatch;
- conservative candidate targets.

---

## 54. Rust macro ontology

### 54.1 Entities

```text
MACRO_DEFINITION
MACRO_INVOCATION
EXPANSION
EXPANDED_ITEM
```

### 54.2 Relationships

```text
INVOKES_MACRO
EXPANDS_TO
GENERATED_FROM
SOURCE_CORRESPONDENCE
```

Where exposed by the compiler/frontend, hygiene and expansion-context information SHOULD be retained.

---

## 55. Rust drop and destruction ontology

### 55.1 Entities

```text
DROP_SITE
DROP_IMPL
DROP_GLUE
```

### 55.2 Relationships

```text
DROPS
INVOKES_DROP_IMPL
INVOKES_DROP_GLUE
DROPS_FIELD
```

Compiler-generated destruction is executable semantics and MUST NOT be omitted merely because no explicit source `drop()` call exists.

---

## 56. Rust async and coroutine-lowering ontology

### 56.1 Entities

```text
ASYNC_FUNCTION
FUTURE_TYPE
COROUTINE_BODY
COROUTINE_STATE
SUSPEND_POINT
RESUME_POINT
```

### 56.2 Relationships

```text
LOWERS_TO_COROUTINE
CREATES_FUTURE
HAS_STATE
SUSPENDS_AT
RESUMES_AT
```

The ontology SHALL preserve:

```text
calling async function
!=
executing async body
```

---

## 57. Rust unsafe and FFI ontology

### 57.1 Entities

```text
UNSAFE_BLOCK
UNSAFE_FUNCTION
RAW_POINTER_DEREF
RAW_ADDRESS
INLINE_ASM
FOREIGN_FUNCTION
EXTERN_BLOCK
```

### 57.2 Relationships

```text
CONTAINS_UNSAFE_OPERATION
CALLS_FOREIGN
CROSSES_FFI
```

These are objective facts only.

---

## 58. Rust constants, statics, and CTFE ontology

Rust MAY expose:

```text
CONST_ITEM
STATIC_ITEM
CONST_VALUE
CTFE_RESULT
CONST_ALLOCATION
```

Relationships:

```text
REFERENCES_CONST
REFERENCES_STATIC
EVALUATES_TO
REFERENCES_ALLOCATION
```

This information SHOULD be included where compiler APIs expose it reliably and structurally.

---

# Part IV — Derived Fact Families

## 59. Derived facts versus source facts

Derived facts are permitted when they are mechanically reproducible from lower-level facts.

Examples:

```text
DOMINATES
POST_DOMINATES
CONTROL_DEPENDENT_ON
REACHING_DEFINITION
TRANSITIVELY_REACHES
CALL_SCC
MAY_ALIAS
LOOP_NESTING_DEPTH
```

A derived fact SHOULD identify:

- the fact family from which it was computed;
- the graph projection or owner over which it was computed;
- the derivation method or analysis version where operationally useful.

This metadata is supporting provenance, not historical analysis.

---

## 60. Recommended graph projections

The same underlying CPG MAY produce multiple graph projections.

### 60.1 Syntax graph

Nodes:

- syntax nodes.

Edges:

- AST containment;
- lexical adjacency.

### 60.2 Symbol graph

Nodes:

- declarations;
- references;
- bindings;
- scopes.

Edges:

- declares;
- binds;
- refers-to;
- captures;
- shadows.

### 60.3 Type graph

Nodes:

- semantic types;
- declarations.

Edges:

- type-of;
- subtype;
- generic argument;
- implementation;
- coercion.

### 60.4 Call graph

Nodes:

- callable definitions and/or callable instances.

Edges:

- exact calls;
- possible calls.

Primary source:

- call-site facts.

### 60.5 Control-flow graph

Nodes:

- blocks/instructions.

Edges:

- normal and exceptional CFG edges.

### 60.6 Dataflow graph

Nodes:

- definition events;
- use events;
- values;
- locations.

Edges:

- reaches;
- def-use;
- value-flow;
- data dependency.

### 60.7 Memory/alias graph

Nodes:

- abstract locations;
- pointers/references;
- values.

Edges:

- points-to;
- may-alias;
- must-alias.

### 60.8 Ownership graph

Primarily Rust-specific.

Nodes:

- values;
- places;
- loans;
- regions;
- move paths.

Edges:

- owns;
- moves;
- copies;
- borrows;
- reborrows;
- outlives.

---

# Part V — Fact Metadata and Conformance

## 61. Universal fact metadata

Every fact SHOULD support a common metadata envelope sufficient to interpret the fact correctly.

Recommended fields:

```text
fact_id
fact_kind

subject_id
object_id          # where relation-shaped

language

source_file_id
source_span        # where applicable

producer
producer_version

certainty
resolution_status

is_derived
derivation_kind

owner_scope_or_owner_unit
```

This specification intentionally excludes historical revision lineage as an ontology domain. Implementations MAY still require an internal snapshot/generation identifier for atomic consistency, but such metadata is infrastructural rather than a semantic code fact.

---

## 62. Certainty and resolution metadata

A present-state fact SHOULD identify its semantic resolution class where ambiguity is possible.

Recommended categories:

```text
EXACT
STATIC_SEMANTIC
SOUND_MAY
POSSIBLE
MODELLED
HEURISTIC
UNRESOLVED
```

These categories are not a single probabilistic confidence scale.

For example:

```text
CALLS_EXACT_TARGET       exact
MAY_CALL                 sound/conservative possibility
UNKNOWN_CALL_TARGET      unresolved
```

---

## 63. Ownership of facts

Every fact SHOULD have an identifiable semantic owner suitable for replacement/recomputation.

Typical ownership units include:

```text
source_file
module
scope
callable
class_or_type
MIR_body
crate
```

Ownership is not a historical feature; it is required to define which present-state facts belong together.

---

## 64. Required identity rules

### 64.1 Source identity

Source locations MUST be tied to a specific current source file identity.

### 64.2 Semantic identity

Semantic entities MUST NOT use source position alone as identity.

### 64.3 Provider-local IDs

The following MUST NOT be persisted as canonical semantic identity unless wrapped in a stable application-owned identity scheme:

- transient parser node IDs;
- Pyrefly internal IDs;
- rustc session-local `DefId`;
- MIR local/block ordinal used as global identity.

### 64.4 Anonymous entities

Anonymous entities such as closures SHOULD use owner-relative structural identity sufficient for stable present-state graph construction.

---

## 65. Required separation of fact types

Implementations conforming to this ontology SHALL distinguish at least the following:

```text
syntax occurrence != semantic entity

declaration != reference

type syntax != semantic type

call expression != call site != callable

declared callable != executable specialization

value != memory location

read != write

copy != move

borrow != raw-address taking

normal CFG edge != exceptional/unwind edge

direct effect != transitive effect

resolved target set != unknown target

source entity != generated/lowered entity
```

---

## 66. Mandatory unknown semantics

A conforming implementation MUST NOT use missing edges as a universal representation of uncertainty.

At minimum, the following unknown concepts SHOULD exist:

```text
UNKNOWN_SYMBOL
UNKNOWN_TYPE
UNKNOWN_CALL_TARGET
UNKNOWN_MEMBER
UNKNOWN_MODULE
UNKNOWN_MEMORY
UNKNOWN_EFFECT
```

Where a dynamic or external construct creates an unresolved candidate space, the graph SHOULD retain an explicit unknown relation.

---

## 67. No evaluative ontology rule

The canonical ontology SHALL NOT include leaf facts whose meaning is primarily an engineering judgment rather than program semantics.

Excluded examples:

```text
SAFE_TO_REFACTOR
RISK_SCORE
TEST_IMPACT
LIKELY_BUG
POOR_DESIGN
VULNERABLE
RECOMMENDATION
HOTSPOT
GOD_OBJECT
SHOULD_INLINE
SHOULD_EXTRACT
```

A downstream analysis system MAY derive such conclusions, but they do not belong to this base CPG specification.

---

# Part VI — Canonical Layer Model

## 68. Ontology layers

The complete present-state CPG is organized into the following conceptual layers:

```text
L0  Source
    text, spans, tokens, comments, documentation

L1  Syntax
    complete raw + normalized syntax structure

L2  Semantic identity
    declarations, symbols, scopes, bindings, references

L3  Type semantics
    types, narrowing, generics, subtyping, coercions

L4  Object semantics
    members, inheritance, traits/protocols, implementation

L5  Invocation
    callable contracts, call sites, arguments, dispatch, targets

L6  Control flow
    blocks, branches, loops, normal and exceptional flow

L7  Values and dataflow
    values, definitions, uses, reaching definitions, dependencies

L8  State and memory
    locations, access paths, reads, writes, aliasing

L9  Ownership and lifetime
    moves, copies, borrows, drops, resource lifetime

L10 Effects
    state mutation, allocation, I/O, raise/panic, async, FFI, unsafe

L11 Generated and lowered semantics
    macros, expansions, MIR, coroutines, shims, specializations

L12 Graph-derived facts
    reachability, SCCs, dominance, post-dominance,
    control dependence, loops, recursion, graph metrics

L13 Semantic summaries
    per-callable reads, writes, calls, effects, returns, unknowns

L14 Explicit uncertainty
    unresolved symbols, types, calls, members, memory, effects
```

---

# Part VII — Recommended Canonical Relationship Inventory

## 69. Structural relationships

```text
CONTAINS
AST_CHILD
ENCLOSES
LEXICALLY_PRECEDES
DEFINED_IN
OWNED_BY
HAS_SCOPE
ENCLOSING_SCOPE
```

## 70. Symbol and binding relationships

```text
DECLARES
BINDS
REFERS_TO
MAY_REFER_TO
SHADOWS
CAPTURES
CAPTURED_FROM
ALIASES
REBINDS
```

## 71. Module and dependency relationships

```text
IMPORTS_MODULE
IMPORTS_SYMBOL
EXPORTS
REEXPORTS
DEFINED_IN_MODULE
DEPENDS_ON_MODULE
```

## 72. Type relationships

```text
DECLARED_TYPE
INFERRED_TYPE
COMPUTED_TYPE
EXPECTED_TYPE
TYPE_OF

PARAMETER_TYPE
RETURN_TYPE
FIELD_TYPE

TYPE_PARAMETER_OF
TYPE_ARGUMENT
LIFETIME_ARGUMENT
CONST_ARGUMENT

SUBTYPE_OF
SUPERTYPE_OF
BOUNDED_BY
CONSTRAINED_BY
OUTLIVES

INSTANTIATES
SPECIALIZES
SUBSTITUTES

COERCES_TO
CASTS_TO
NARROWS_TO
```

## 73. Member relationships

```text
DECLARES_MEMBER
HAS_MEMBER
INHERITS
IMPLEMENTS
IMPLEMENTS_TRAIT
IMPLEMENTS_METHOD
OVERRIDES
OVERRIDDEN_BY
RESOLVES_MEMBER
MAY_RESOLVE_MEMBER
```

## 74. Invocation relationships

```text
CONTAINS_CALL
HAS_CALLEE_EXPRESSION
HAS_RECEIVER
HAS_ARGUMENT
ARGUMENT_BINDS_TO

CALLS_DECLARATION
CALLS_EXACT_TARGET
CALLS_INSTANCE
MAY_CALL
CALLS_UNKNOWN

REFERENCES_CALLABLE
TAKES_FUNCTION_ADDRESS
PASSES_CALLABLE
RETURNS_CALLABLE
```

## 75. Control-flow relationships

```text
CFG_NEXT
CFG_TRUE
CFG_FALSE
CFG_CASE
CFG_LOOP_BACK
CFG_BREAK
CFG_CONTINUE
CFG_RETURN
CFG_EXCEPTION
CFG_UNWIND
CFG_CALL_RETURN
```

## 76. Dataflow relationships

```text
DEFINES
USES
REACHES
DEF_USE
DATA_DEP
VALUE_FLOWS_TO
PRODUCES_VALUE
CONSUMES_VALUE
OPERAND
RESULT
```

## 77. Memory relationships

```text
READS
WRITES
MUTATES
INITIALIZES
DEINITIALIZES
TAKES_ADDRESS
DEREFERENCES

MUST_ALIAS
MAY_ALIAS
DOES_NOT_ALIAS
POINTS_TO
MAY_POINT_TO
```

## 78. Ownership/lifetime relationships

```text
OWNS
MOVED_TO
COPIED_TO
BORROWS_SHARED
BORROWS_MUTABLY
REBORROWS
LOAN_CREATED_AT
LOAN_LIVE_AT
REGION_CONTAINS
OUTLIVES

DROPS
DROPS_FIELD
TRANSFERS_RESOURCE
RELEASES_RESOURCE
```

## 79. Effect relationships

```text
READS_STATE
WRITES_STATE
MUTATES_ARGUMENT
ALLOCATES
DEALLOCATES
MAY_RAISE
MAY_PANIC
MAY_UNWIND
PERFORMS_IO
MAY_BLOCK
SPAWNS_TASK
SPAWNS_THREAD
AWAITS
ACQUIRES_LOCK
RELEASES_LOCK
CALLS_FOREIGN_CODE
USES_UNSAFE_OPERATION
USES_INLINE_ASSEMBLY
```

## 80. Generated/lowered relationships

```text
GENERATED_FROM
EXPANDED_FROM
EXPANDS_TO
LOWERS_TO
CORRESPONDS_TO
MONOMORPHIZES
SPECIALIZES
```

## 81. Derived graph relationships

```text
TRANSITIVELY_REACHES
TRANSITIVELY_REACHED_BY

DOMINATES
STRICTLY_DOMINATES
IMMEDIATE_DOMINATOR

POST_DOMINATES
IMMEDIATE_POST_DOMINATOR

CONTROL_DEPENDENT_ON

BACK_EDGE
LOOP_MEMBER

DIRECT_CALLER
DIRECT_CALLEE
TRANSITIVE_CALLER
TRANSITIVE_CALLEE
```

---

# Part VIII — Conformance Requirements

## 82. Core conformance

A CPG implementation conforms to the **Core Present-State Ontology** when it can represent:

1. source spans and syntax;
2. semantic declarations and references;
3. scopes and bindings;
4. semantic types;
5. call sites and call targets;
6. control flow;
7. values and def-use;
8. state reads/writes;
9. unresolved semantic facts;
10. objective derived graph facts.

## 83. Python profile conformance

A Python-conformant implementation additionally SHOULD represent:

- Python-specific scopes;
- Python binding categories;
- Python inferred and contextual types;
- MRO;
- descriptors/properties;
- constructor and callable-object semantics;
- decorators as executable application;
- comprehensions;
- pattern matching;
- async/generator semantics;
- dynamic constructs and explicit unknowns.

## 84. Rust profile conformance

A Rust-conformant implementation additionally SHOULD represent:

- crates/modules/items;
- generics and lifetimes;
- traits/impls;
- Rust semantic types;
- macros and expansion;
- MIR bodies;
- MIR places and projections;
- reads/writes/moves/copies;
- borrows and loans where available;
- static and dynamic trait dispatch;
- function pointers/closures;
- monomorphic instances;
- drop glue;
- async/coroutine lowering;
- unsafe/FFI operations.

## 85. Advanced derived-fact conformance

An advanced implementation SHOULD compute:

```text
dominators
post-dominators
control dependence
reaching definitions
def-use
liveness
loops
SCCs
recursion
transitive reachability
alias/points-to sets
objective per-callable effect summaries
```

---

# Part IX — Non-Goals

## 86. No agent reasoning inside the ontology

The ontology is deliberately designed so that a downstream LLM can answer questions such as:

- “What code could execute from here?”
- “Where can this value come from?”
- “Who writes this field?”
- “Which implementations could this call dispatch to?”
- “What code mutates this parameter?”
- “What happens during Rust drop?”
- “What conditions govern execution of this write?”

The CPG provides the underlying facts.

It does not canonically answer:

- “Should I change this?”
- “Is this refactor safe?”
- “Which test should I run?”
- “Is this architecture good?”
- “Is this vulnerability exploitable?”

Those are downstream reasoning problems.

---

# Part X — Final Specification Principle

## 87. Governing design rule

Every canonical node, property, and relationship added to the CPG SHOULD satisfy this test:

> **Does this describe an objective fact about the present-state program, or a mechanically derivable property of those facts?**

If yes, it belongs in the ontology.

If instead the proposed fact primarily answers:

> **What should an engineer conclude or do?**

then it belongs in a downstream analysis or agent-reasoning layer, not in the base CPG.

The target architecture is therefore:

```text
Present-state source
        ↓
Raw syntax facts
        ↓
Semantic facts
        ↓
Compiler / IR facts
        ↓
Derived graph facts
        ↓
Deterministic summaries
        ↓
Explicit unknowns
        ↓
Comprehensive CPG fact substrate
        ↓
LLM programming-agent reasoning
```

This specification intentionally stops at the **Comprehensive CPG fact substrate** boundary.

---

# Appendix A — Compact Ontology Checklist

```text
SOURCE
[ ] files
[ ] spans
[ ] tokens
[ ] comments
[ ] documentation
[ ] directives
[ ] parse errors

SYNTAX
[ ] every raw syntax node
[ ] normalized syntax kind
[ ] AST field names
[ ] lexical ordering

SEMANTICS
[ ] declarations
[ ] scopes
[ ] bindings
[ ] references
[ ] identity
[ ] imports/exports

TYPES
[ ] declared type
[ ] inferred/computed type
[ ] expected type
[ ] subtype relationships
[ ] generics
[ ] coercions
[ ] narrowing

OBJECT MODEL
[ ] members
[ ] inheritance
[ ] trait/protocol implementation
[ ] overrides
[ ] member resolution

CALLS
[ ] call sites
[ ] receiver
[ ] arguments
[ ] argument binding
[ ] dispatch kind
[ ] exact targets
[ ] may targets
[ ] unknown targets

CONTROL FLOW
[ ] entry/exit
[ ] basic blocks
[ ] normal CFG
[ ] exceptional CFG
[ ] dominators
[ ] post-dominators
[ ] control dependence
[ ] loops

VALUES / DATAFLOW
[ ] values
[ ] definitions
[ ] uses
[ ] reaching definitions
[ ] def-use
[ ] value flow
[ ] liveness

MEMORY
[ ] access paths
[ ] reads
[ ] writes
[ ] initialization
[ ] aliasing
[ ] points-to

OWNERSHIP / LIFETIME
[ ] moves
[ ] copies
[ ] borrows
[ ] reborrows
[ ] loans
[ ] regions
[ ] drops
[ ] resource lifecycle

EFFECTS
[ ] state mutation
[ ] allocation
[ ] I/O
[ ] blocking
[ ] exceptions
[ ] panic/unwind
[ ] spawn/await
[ ] locks
[ ] unsafe
[ ] FFI

GENERATED / LOWERED
[ ] Python synthesized semantics where objectively modelled
[ ] Rust macros
[ ] Rust MIR
[ ] Rust monomorphization
[ ] Rust shims/drop glue
[ ] async/coroutine lowering

DERIVED FACTS
[ ] reachability
[ ] SCCs
[ ] recursion
[ ] dominance
[ ] post-dominance
[ ] control dependence
[ ] loops
[ ] alias sets
[ ] effect summaries
[ ] structural metrics

UNCERTAINTY
[ ] unknown symbols
[ ] unknown types
[ ] unknown call targets
[ ] unknown members
[ ] unknown memory
[ ] unknown effects
```

---

# Appendix B — Explicitly Excluded Analytical Outputs

```text
historical change analysis
semantic diff across revisions
test-impact analysis
coverage analysis
runtime profiling
refactor-safety judgment
bug-likelihood judgment
risk scoring
architecture-quality scoring
vulnerability exploitability
recommendations
remediation plans
change prioritization
```

These may be produced by downstream systems using the facts specified here, but are not part of this ontology.
