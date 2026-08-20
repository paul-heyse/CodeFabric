---
name: attrs-cattrs-ref
description: "TARGET DOCUMENTS ABSENT — `docs/library_ref/attrs.md` and `cattrs.md` are not written yet, so section citations do not resolve. Neither library is a direct CodeFabric dependency after the Wave 0 packaging cutover; transitive lockfile presence is not adoption. Use upstream docs if a future direct seam is proposed. Reference for attrs (declarative class definition, field metadata, validators, converters, frozen/mutable policy, introspection, extending) and cattrs (structured/unstructured boundary conversion, converter instances, hook registration, generated hooks, union handling, validation errors, preconfigured serializer integrations). Use when code involves attrs or cattrs imports, class definition with @define/@frozen, field() declarations, cattrs.structure/unstructure, Converter configuration, or boundary serialization design."
allowed-tools: Read, Grep, Glob, Bash
---

# attrs + cattrs Reference

## How the Reference Documents Are Organized

**Version anchor:** attrs 26.1.0, cattrs 26.1.0. All guidance assumes these versions.

Two deep-dive reference documents back this skill.

> **Both are currently absent from this repository.** `docs/library_ref/attrs.md` and
> `docs/library_ref/cattrs.md` have not been written, so every `§N` citation below — the section
> indexes, the task tables, the decision trees — resolves to nothing. Unlike `typer-rich-ref`,
> this skill is currently **inactive for implemented CodeFabric code**: neither library is
> a direct dependency. Treat the section numbers as a specification of what those two
> documents should contain, and fall back to the upstream docs (<https://www.attrs.org>,
> <https://catt.rs>) until they exist. The design suite in `docs/upfront_design/` never mentions
> either library, so nothing here is spec-anchored.

| Library | Path | Lines | Sections | Scope |
|---------|------|-------|----------|-------|
| attrs | `docs/library_ref/attrs.md` | ~3,900 | 21 (§0-§20) | Class definition, fields, validators, converters, mutation policy, immutability, equality, hashing, typing, inheritance, slots, serialization helpers, introspection, programmatic generation, extending, comparative framing, best practices |
| cattrs | `docs/library_ref/cattrs.md` | ~4,800 | 19 (§0-§18) | Converters, default type coverage, configuration, structuring, unstructuring, custom hooks, predicate hooks, hook factories, field overrides, unions, validation errors, fallbacks, preconfigured converters, recipes, performance, typing edge cases, migration, testing |

**Document structure pattern (both docs):** Each numbered section opens with a design-stance summary, then covers exact API semantics, deployment advisories, illustrative code, and anti-patterns. Subsections use N.M and N.M.K numbering. Both docs cite official documentation URLs as source anchors.

**Reading strategy:**
- Use the section indexes below to find the right section number and line offset.
- Read with `offset` and `limit` parameters. Sections average 150-200 lines.
- For cross-cutting questions, use the Unified Concern Cross-Reference to find sections in both docs.

---

## attrs Section Index

| § | Line | Title | Key Topics |
|---|------|-------|------------|
| 0 | 1 | Scope & mental model | Version anchor, "regular classes" stance, attrs vs cattrs boundary, doc assumptions |
| 1 | 51 | Core API names | `attrs.*` vs `attr.*`, modern vs classic, import policy, migration framing |
| 2 | 185 | Class definition: `define`, `mutable`, `frozen` | Decorator params, slots, frozen, on_setattr defaults, kw_only, match_args, deployment profiles |
| 3 | 491 | Field declaration: `field()` | default, factory, validator, converter, repr, eq, order, hash, metadata, alias, on_setattr, Attribute model |
| 4 | 815 | Initialization & lifecycle | `__attrs_pre/post_init__`, `__attrs_init__`, execution order, derived attributes, frozen post-init |
| 5 | 1096 | Defaults, factories, aliases | Literal defaults, `Factory(takes_self)`, `@x.default`, alias/private-name stripping, derived-attr patterns |
| 6 | 1380 | Validation framework | Decorator vs callable validators, composition (`and_`, `or_`, `not_`, `optional`), built-in validators, global enable/disable, `attrs.validate()` |
| 7 | 1565 | Conversion & normalization | Plain converters, `attrs.Converter`, signature effects, `pipe()`, `optional()`, `default_if_none()`, `to_bool`, assignment-time conversion |
| 8 | 1789 | Mutation policy: `on_setattr` | `setters.convert`, `validate`, `frozen`, `pipe`, `NO_OP`, class-level vs field-level, partial immutability |
| 9 | 1937 | Immutability & persistent update | `frozen=True` deep semantics, `attrs.evolve()`, `copy.replace()`, transitive immutability limits |
| 10 | 2109 | Equality & ordering | Generated `__eq__`/`__ne__`, `order=True`, per-field `eq`/`order` callables, `cmp_using` |
| 11 | 2246 | Hashing & identity | Hash rules, `frozen` + `eq` interaction, `unsafe_hash`, `cache_hash`, per-field `hash=` |
| 12 | 2420 | Type metadata & static typing | Annotation modes, Pyright/mypy support, `type=` parameter, type-checker limitations |
| 13 | 2574 | Inheritance & subclassing | Field collection across MRO, `__attrs_init_subclass__`, slotted MI caveats |
| 14 | 2755 | Slots, pickling, exceptions | `slots=True` mechanics, `weakref_slot`, `getstate/setstate`, `auto_exc`, cached_property |
| 15 | 2858 | Serialization & collection helpers | `asdict`, `astuple`, filters/value_serializer, recurse control |
| 16 | 3115 | Introspection & runtime inspection | `attrs.fields()`, `fields_dict()`, `has()`, `Attribute` members, `resolve_types()` |
| 17 | 3330 | Programmatic class generation | `attrs.make_class()`, dynamic field construction, `these=` parameter |
| 18 | 3531 | Extending attrs | `field_transformer`, metadata-driven extensions, wrapper decorators |
| 19 | 3765 | Comparative framing | attrs vs dataclasses vs pydantic vs msgspec, selection guidance |
| 20 | 3859 | Best practices & anti-patterns | Deployment rules, common failure modes, documentation standards |

---

## cattrs Section Index

| § | Line | Title | Key Topics |
|---|------|-------|------------|
| 0 | 3 | Scope & mental model | "Validation at edges" thesis, structured vs unstructured, two primitives, boundary placement, global vs private converters |
| 1 | 204 | Core API & object model | `structure`/`unstructure`, `BaseConverter`/`Converter`, `copy()`, `get_structure_hook`/`get_unstructure_hook`, fallback factories |
| 2 | 475 | Default type coverage | Primitives, enums, Path, optionals, lists, dicts, tuples, deques, sets, TypedDict, attrs/dataclasses, generics, unions, special typing forms |
| 3 | 751 | Converter configuration | `dict_factory`, `unstruct_strat`, `detailed_validation`, `prefer_attrib_converters`, `omit_if_default`, `forbid_extra_keys`, `use_alias`, `unstruct_collection_overrides`, `type_overrides` |
| 4 | 1074 | Structuring in detail | Dispatch model, registration order, scalar/collection/class/TypedDict/generic structuring, field resolution, detailed validation error trees, union disambiguation |
| 5 | 1334 | Unstructuring in detail | Runtime-class dispatch, `unstructure_as`, leaf/collection/class outbound, `AS_DICT` vs `AS_TUPLE`, `omit_if_default` mechanics |
| 6 | 1531 | Custom hooks | `register_structure_hook`, `register_unstructure_hook`, hook signatures, wrapping existing hooks via `get_*_hook` |
| 7 | 1767 | Predicate hooks & dispatch | `register_*_hook_factory`, predicate functions, `is_annotated`/`has_attrib`, dispatch priority, ordering rules |
| 8 | 2047 | Hook factories & generated hooks | `make_dict_structure_fn`, `make_dict_unstructure_fn`, `override()`, `_cattrs_*` parameters, regeneration |
| 9 | 2343 | Field-level overrides & `Annotated` | `override(rename, omit, struct_hook, unstruct_hook, omit_if_default)`, `Annotated[T, override(...)]`, per-field customization |
| 10 | 2660 | Union handling & polymorphism | Default union strategy, Literal disambiguation, unique-field disambiguation, tagged unions, `configure_tagged_union`, include/exclude strategies |
| 11 | 2967 | Validation & error surfacing | `detailed_validation`, `ClassValidationError`, `IterableValidationError`, `BaseValidationError`, ExceptionGroup topology |
| 12 | 3157 | Fallback behavior | `structure_fallback_factory`, `unstructure_fallback_factory`, converter chaining, missing-handler errors |
| 13 | 3395 | Preconfigured converters | JSON, orjson, msgpack, CBOR, TOML, YAML preconf factories, `datetime`/`date` extras, `make_converter()` |
| 14 | 3550 | Recipes & advanced patterns | Nested renaming, subclass disambiguation, optional key handling, strict/lenient boundary profiles |
| 15 | 3946 | Performance & throughput | `Converter` vs `BaseConverter` startup, generated-hook caching, benchmark patterns |
| 16 | 4225 | Typing edge cases | PEP 695 aliases, `NewType`, `Protocol`, `Self`, `Annotated`, stringized annotations |
| 17 | 4382 | Migration & version behavior | Sequence to tuple change, abstract-set to frozenset change, `GenConverter` to `Converter` rename, fallback eagerness |
| 18 | 4549 | Testing & QA | Golden-fixture testing, converter isolation, boundary-contract testing patterns |

---

## Unified Concern Cross-Reference

| Concern | attrs | cattrs |
|---------|-------|--------|
| **Getting started / mental model** | §0, §1 | §0, §1 |
| **Defining a class** | §2, §3 | -- |
| **Frozen / immutable objects** | §2.5, §9 | §2.5, §4.5, §5.6 |
| **Field defaults and factories** | §3.2-§3.4, §5 | -- |
| **Validators and invariants** | §6 | §11 |
| **Converters and normalization** | §7 | §3.3.4, §4.9 |
| **Assignment-time mutation** | §8 | -- |
| **Equality and comparison** | §10 | -- |
| **Hashing and identity** | §11 | -- |
| **Inheritance and subclassing** | §13 | -- |
| **Structuring from dicts/tuples** | -- | §4 |
| **Unstructuring to dicts/tuples** | §15 | §5 |
| **Custom hook registration** | -- | §6, §7 |
| **Generated hooks and factories** | §18 (field_transformer) | §8, §9 |
| **Per-field override at boundary** | §3.8 (metadata) | §9 |
| **Union / polymorphic types** | -- | §10 |
| **Validation error diagnostics** | §6.6-§6.7 | §11 |
| **Converter config flags** | -- | §3 |
| **Serializer integration** | §15 (asdict/astuple) | §13 |
| **Performance** | §2.3 (slot/frozen perf) | §15 |
| **Type annotations / edge cases** | §12 | §16 |
| **Testing patterns** | §20 | §18 |
| **Introspection / runtime metadata** | §16 | §1.7-§1.9 |
| **Programmatic class generation** | §17 | -- |
| **attrs vs dataclasses vs others** | §19 | §0.6 |
| **attrs + cattrs together** | §0.3-§0.4 | §0.5-§0.6, §2.5 |
| **Migration / version behavior** | §1.5-§1.9 | §17 |

---

## Decision Trees

### Tree 1: Class Definition Surface

```
Need immutable value object / cache key / config?
  -> @frozen (attrs §2, §9)
Need mutable object with invariant-preserving writes?
  -> @define (attrs §2, §8)
Need stylistic symmetry with @frozen in same module?
  -> @mutable (attrs §2.4)
Multiple inheritance or metaclass framework?
  -> @define(slots=False) (attrs §2.2.3, §14)
Public API with subclass evolution?
  -> add kw_only=True (attrs §2.2.5)
```

### Tree 2: Field Policy

```
Field needs no custom behavior?
  -> bare annotation: x: int (attrs §3.11)
Field needs any local policy?
  -> field() required (attrs §3.12)
    Invariant enforcement needed?
      -> field(validator=...) (attrs §6)
    Inbound normalization needed?
      -> field(converter=...) (attrs §7)
    Both normalization and invariant?
      -> field(converter=..., validator=...) -- converter runs first (attrs §4.3)
    Governance/extension metadata?
      -> field(metadata={...}) (attrs §3.8, §16)
    Constructor name differs from storage name?
      -> field(alias=...) (attrs §3.2, §5.6)
```

### Tree 3: Default / Factory Selection

```
Immutable scalar constant?
  -> literal default: x: int = 3 (attrs §5.2)
Fresh mutable container (list, dict, set)?
  -> factory=list / factory=dict (attrs §5.3)
Derived from earlier fields, caller can override?
  -> default=Factory(func, takes_self=True) or @x.default (attrs §5.4, §5.5)
Derived from full object, not caller-supplied?
  -> field(init=False) + __attrs_post_init__ (attrs §4.7)
Construction adapts external input / multiple modes?
  -> @classmethod factory (attrs §4.7, §5.7)
```

### Tree 4: Converter Selection

```
Long-lived service, repeated conversions?
  -> Converter() (cattrs §1.4, §15)
CLI / ultra-short-lived, startup-sensitive?
  -> evaluate BaseConverter() (cattrs §1.3, §15)
Need strict ingress validation?
  -> Converter(forbid_extra_keys=True, detailed_validation=True) (cattrs §3.4.2, §11)
Need compact egress output?
  -> Converter(omit_if_default=True) (cattrs §3.4.1)
Need aliased field keys at boundary?
  -> Converter(use_alias=True) (cattrs §3.4.3)
Need serializer-friendly collections (sets->lists)?
  -> unstruct_collection_overrides={Set: list} (cattrs §3.4.4)
Fork policy but keep registered hooks?
  -> converter.copy(...) (cattrs §1.6, §3.6)
Different fallback-factory behavior needed?
  -> fresh Converter() -- copy() cannot change fallbacks (cattrs §3.5.5)
```

### Tree 5: Structuring Strategy

```
Incoming data is a dict/mapping?
  -> dict-based structuring (default) (cattrs §4.5)
Incoming data is positional (tuple/list)?
  -> structure_attrs_fromtuple or unstruct_strat=AS_TUPLE (cattrs §4.6)
Need per-field rename at boundary?
  -> make_dict_structure_fn with override(rename=...) (cattrs §8, §9)
Need per-field custom structuring logic?
  -> override(struct_hook=...) (cattrs §9)
Need union disambiguation?
  Has Literal discriminator field?
    -> default strategy handles it (cattrs §10)
  No shared Literal?
    -> unique required fields or configure_tagged_union (cattrs §10)
Leaf hook must affect composite targets?
  -> register leaf hooks BEFORE first use of composite targets (cattrs §4.2)
```

### Tree 6: Boundary Placement

```
Leaf script or tiny app?
  -> global cattrs.structure/unstructure acceptable (cattrs §0.5)
Reusable package / service / multi-boundary system?
  -> private Converter() instances (cattrs §0.5, §1.10)
Multiple boundaries with shared hooks but different policy?
  -> base converter + copy() per boundary (cattrs §3.6)
Need layered fallback (child->parent converter)?
  -> fallback factories: parent.get_*_hook (cattrs §1.8, §12)
Test isolation required?
  -> private Converter per test or fixture (cattrs §18)
```

---

## Operating Rules

1. **attrs owns class construction; cattrs owns boundary conversion.** attrs configures at class-definition time, then is out of the picture. cattrs operates at ingress/egress boundaries. Do not conflate the two. (attrs §0.3-§0.4, cattrs §0.1)

2. **Do not dump serialization policy into attrs models.** Validation and serialization are edge concerns. Rename logic, omit-if-default, wire-format keys belong in cattrs converter configuration, not in attrs field declarations. (attrs §0.3, cattrs §0.1)

3. **Structure once at ingress, unstructure once at egress.** Do not re-structure trusted internal objects. cattrs validates during structuring, not unstructuring. (cattrs §0.4, §0.8)

4. **Use converters for normalization, validators for invariants.** attrs converters run before validators. Converters transform input representation. Validators check the normalized result. Do not use converters for semantic invariants or validators for parsing. (attrs §4.3, §7.9)

5. **Register leaf hooks before composite targets.** cattrs generated hooks capture child handlers at generation time. A late-registered int hook will not affect an already-generated list[int] hook. (cattrs §4.2)

6. **Instantiate private Converter instances; do not mutate the global.** Global converter mutations affect all call sites. Larger applications should create private instances. (cattrs §0.5, §1.2)

7. **Use frozen for value objects; use on_setattr for guarded mutables.** Do not use frozen=True merely to preserve correctness on mutable objects. attrs.define() already runs converters and validators on assignment by default. (attrs §2.4, §8.10)

8. **Prefer kw_only=True for public / subclassable constructors.** Keyword-only prevents constructor-ordering breakage when subclasses add mandatory fields after base-class defaults. (attrs §2.2.5)

9. **Prefer bare annotations for simple fields; use field() only when local policy is needed.** Mixed unannotated field() + bare annotated attributes is the highest-risk authoring error. (attrs §3.11-§3.14)

10. **Assume generated hooks snapshot converter defaults at generation time.** Changing converter policy after a type's hook was generated requires regeneration or re-registration. (cattrs §3.5, §4.2)
