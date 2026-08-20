# Pydantic in Python — advanced technical reference / feature-category catalog

## Version / source anchors

This reference is pinned to **Pydantic 2.13.4**, released **2026-05-06**, which is the latest stable Pydantic release on PyPI as of **2026-08-19**. Stable 2.13.4 supports **Python >=3.9**. The corresponding Pydantic Core dependency is **pydantic-core 2.46.4**; Pydantic also depends on `annotated-types`, `typing-extensions`, and `typing-inspection`. Pydantic's core validation and serialization engine is implemented in Rust through `pydantic-core`, while Python-side Pydantic builds and manages schemas, models, configuration, decorators, and developer-facing APIs.

A **Pydantic 2.14.0b1** prerelease exists as of this reference date. It is deliberately excluded from the stable API contract in this document and covered separately in the prerelease-transition section. Pydantic 2.14 drops Python 3.9, adds initial Python 3.15 support, and contains model-build and core-schema changes that should not be silently assumed by 2.13 code.

`pydantic-settings` is a separately versioned package in Pydantic V2. This reference covers it as a companion subsystem and pins its examples to **pydantic-settings 2.15.0** (released 2026-08-07, Python >=3.10), rather than conflating its version with core Pydantic.

Primary anchors used throughout:

- Pydantic Validation docs: https://pydantic.dev/docs/validation/latest/get-started/
- Installation: https://pydantic.dev/docs/validation/latest/get-started/install/
- Models: https://pydantic.dev/docs/validation/latest/concepts/models/
- Fields: https://pydantic.dev/docs/validation/latest/concepts/fields/
- Configuration: https://pydantic.dev/docs/validation/latest/api/pydantic/config/
- Validators: https://pydantic.dev/docs/validation/latest/concepts/validators/
- Serialization: https://pydantic.dev/docs/validation/latest/concepts/serialization/
- TypeAdapter: https://pydantic.dev/docs/validation/latest/api/pydantic/type_adapter/
- JSON: https://pydantic.dev/docs/validation/latest/concepts/json/
- JSON Schema: https://pydantic.dev/docs/validation/latest/concepts/json_schema/
- Strict Mode: https://pydantic.dev/docs/validation/latest/concepts/strict_mode/
- Performance: https://pydantic.dev/docs/validation/latest/concepts/performance/
- Migration guide: https://pydantic.dev/docs/validation/latest/get-started/migration/
- Changelog: https://pydantic.dev/docs/validation/latest/get-started/changelog/
- 2.13 release article: https://pydantic.dev/articles/pydantic-v2-13-release
- PyPI core package: https://pypi.org/project/pydantic/
- pydantic-settings docs: https://pydantic.dev/docs/validation/latest/concepts/pydantic_settings/
- pydantic-settings PyPI: https://pypi.org/project/pydantic-settings/

## Why this reference is intentionally broader than a model tutorial

Pydantic is commonly introduced as `class User(BaseModel): ...`, but a production Pydantic boundary is an interaction among at least seven layers:

1. **Python typing layer** — annotations, generics, `Annotated`, `Literal`, unions, dataclasses, `TypedDict`, forward references.
2. **Pydantic schema-construction layer** — field discovery, `FieldInfo`, configuration, decorators, model rebuilding, generic specialization.
3. **Core-schema layer** — Pydantic translates Python types and metadata into `pydantic_core.core_schema.CoreSchema` graphs.
4. **Rust runtime layer** — `SchemaValidator` and `SchemaSerializer` execute compiled validation and serialization logic.
5. **Python customization layer** — field/model validators, functional validators, serializers, custom type hooks, context, model lifecycle hooks.
6. **Contract layer** — JSON Schema, aliases, strictness/coercion, serialization mode, error locations, API compatibility.
7. **Application layer** — FastAPI/request boundaries, settings, persistence, queues, LLM structured output, config/secrets, observability and tests.

A serious agent reference therefore needs to document not only individual methods, but also **ownership boundaries and invariants**: when Pydantic validates versus trusts data, where coercion occurs, when schemas are built, how validation and serialization differ, what static typing does and does not prove, and which configuration choices alter external contracts.

## Feature inventory: what this reference covers

The public Pydantic capability surface naturally breaks into:

- `BaseModel`, model construction, validation, copying, equality, extra/private state, and post-init hooks;
- `Field`, `FieldInfo`, `Annotated`, defaults, factories, constraints, aliases, computed fields and private attributes;
- `ConfigDict` and model/dataclass/TypedDict configuration;
- strict vs lax validation and the conversion table;
- field/model/functional validators and validation context;
- model/field/functional serializers and serialization context;
- Python-mode vs JSON-mode dumping, inclusion/exclusion, round-trip and polymorphic serialization;
- `TypeAdapter` for arbitrary type validation, serialization and schema generation without a `BaseModel` wrapper;
- `RootModel`, Pydantic dataclasses, standard dataclasses, `TypedDict`, `NamedTuple`, generics and recursive models;
- unions, smart union selection, left-to-right union mode and discriminated unions;
- dynamic models, `create_model`, forward-reference resolution and `model_rebuild`;
- custom types and `CoreSchema`/JSON-Schema extension hooks;
- built-in, standard-library, Pydantic-specific and network types;
- JSON parsing, `jiter`, string caching and partial validation;
- JSON Schema generation/customization and validation-vs-serialization schemas;
- error objects, custom errors and usage errors;
- `@validate_call`;
- `pydantic-settings` environment, dotenv, CLI, secrets and cloud secret-provider integration;
- performance, schema-build cost, memory, reuse and validation-path tuning;
- experimental APIs and stability boundaries;
- Mypy/Pyrefly/static analysis and IDE/tooling integrations;
- V1 compatibility/migration and the 2.13 → 2.14 boundary;
- production security, secrets, trust boundaries, observability and contract testing.

---

# Proposed comprehensive documentation map

## 0) Scope, versioning, and Pydantic mental model
## 1) Installation, dependencies, extras, version pinning, and project layout
## 2) First executable validation/serialization application
## 3) Architecture: Python annotations → CoreSchema → Rust validator/serializer
## 4) `BaseModel` definition and object model
## 5) Validation entry points: `__init__`, `model_validate`, JSON and strings
## 6) Trusted construction, copying, equality, extras, field-set tracking, and private state
## 7) Fields, `FieldInfo`, `Annotated`, metadata, constraints, and signatures
## 8) Defaults, `default_factory`, validated data, and default validation
## 9) `ConfigDict`: complete configuration model
## 10) Strict mode, lax coercion, and the conversion contract
## 11) Constraints, reusable annotated types, and constrained-type design
## 12) Aliases, validation aliases, serialization aliases, paths, choices, and generators
## 13) Field validators: before, after, plain, and wrap
## 14) Model validators, `ValidationInfo`, context, ordering, and inheritance
## 15) Functional validator metadata: `BeforeValidator`, `AfterValidator`, `WrapValidator`, `ValidateAs`, and related helpers
## 16) Serialization fundamentals: `model_dump` and `model_dump_json`
## 17) Field serializers, model serializers, functional serializers, and serialization context
## 18) Include/exclude semantics, `exclude_if`, unset/default/none/computed handling
## 19) Subclass and polymorphic serialization, `SerializeAsAny`, and external-contract safety
## 20) Computed fields, private attributes, properties, and model lifecycle hooks
## 21) `TypeAdapter`: arbitrary-type validation, serialization, JSON Schema, and reuse
## 22) `RootModel`
## 23) Pydantic dataclasses
## 24) `TypedDict`, standard-library dataclasses, `NamedTuple`, and model-like types
## 25) Generic models, type variables, specialization, and PEP 695 syntax
## 26) Unions: smart mode, left-to-right, discriminators, callable discriminators, and errors
## 27) Forward annotations, recursive models, cyclic input, and namespace resolution
## 28) Dynamic models, `create_model`, `model_rebuild`, and runtime schema composition
## 29) Custom types, `CoreSchema`, `__get_pydantic_core_schema__`, and annotated handlers
## 30) Built-in and standard-library type validation
## 31) Pydantic-specific types, secrets, encoded data, constraints, and `FailFast`
## 32) Network, URL, DSN, email, IP, UUID, path, and filesystem-oriented types
## 33) JSON parsing, `jiter`, string caching, and partial validation
## 34) JSON Schema fundamentals and validation-vs-serialization schemas
## 35) Advanced JSON Schema customization and `GenerateJsonSchema`
## 36) Errors: `ValidationError`, custom errors, locations, causes, and usage errors
## 37) `@validate_call`: validation of ordinary function calls
## 38) `pydantic-settings` 2.15.0 fundamentals and source priority
## 39) Advanced settings: nested env, dotenv, secrets, CLI, cloud secret managers, and custom sources
## 40) Performance, build-time cost, memory, validation hot paths, and `FailFast`
## 41) Experimental APIs and stability boundaries
## 42) Static typing, Mypy, Pyrefly, IDEs, Hypothesis, and code generation
## 43) Observability and validation instrumentation
## 44) Framework and persistence integration boundaries
## 45) Pydantic V1 compatibility and V1 → V2 migration
## 46) Stable release delta: Pydantic 2.12 → 2.13.4
## 47) Pydantic 2.14 prerelease transition and Python-version boundary
## 48) Testing, schema snapshots, round-trip checks, fuzzing, and compatibility contracts
## 49) Security, secrets, untrusted input, serialization exposure, and trust boundaries
## 50) Production architecture patterns
## 51) Dense appendices and lookup matrices

---

# Stable release delta — why 2.13.4 deserves a new reference

Pydantic 2.13 is not merely a patch train. It materially changes the serialization and model-behavior surface that coding agents must reason about:

| Area | 2.13 change | Why it matters |
|---|---|---|
| Subclass serialization | new `polymorphic_serialization` option | gives a narrower, explicit alternative to broad duck-typed `serialize_as_any` behavior |
| Conditional exclusion | `exclude_if` supported on computed fields | conditional output contracts can now cover computed values |
| String constraints | `StringConstraints(ascii_only=...)` | reusable string contracts can reject non-ASCII content without a custom validator |
| Private attributes | default factories may receive validated model data | private state can be derived from already-validated fields during construction |
| Union serialization | discriminator-selected branch no longer falls back across all members | serialized union behavior is more deterministic and mirrors discriminator intent |
| Extra-field state | extra fields assigned after initialization are tracked in `model_fields_set` | `exclude_unset` and stateful patch/update workflows can observe extras more accurately |
| `RootModel` | shallow-copy and core-metadata fixes | root models have several version-sensitive edge cases now resolved in 2.13.x |
| JSON validation | 2.13.1/2 restored `ValidationInfo.data`/`field_name` in `model_validate_json` paths | custom validators behave consistently across Python and JSON entry points |
| `from_attributes` | 2.13.3 handles `AttributeError` subclasses | ORM/object extraction is more robust |
| Pydantic V1 compatibility | bundled namespace updated to 1.10.26 | V1 compatibility inside a V2 install includes Python 3.14 support |

---

# Source-index shorthand used in the prose

This document cites source families in prose with compact labels:

- **[DOCS-HOME]** — current stable Pydantic Validation docs landing page.
- **[MODELS]** — Models concept guide.
- **[FIELDS]** — Fields concept guide / Fields API.
- **[CONFIG]** — `ConfigDict` API.
- **[VALIDATORS]** — validators concept/API.
- **[SER]** — serialization concept/API.
- **[TYPEADAPTER]** — `TypeAdapter` API.
- **[JSON]** — JSON parsing guide.
- **[JSONSCHEMA]** — JSON Schema guide/API.
- **[STRICT]** — strict-mode guide and conversion table.
- **[PERF]** — performance guide.
- **[MIGRATION]** — official V1 → V2 migration guide.
- **[CHANGELOG]** — current changelog.
- **[V213]** — Pydantic 2.13 release article/changelog.
- **[SETTINGS]** — Pydantic Settings guide.
- **[PYPI]** — PyPI release metadata.


# Pydantic Advanced — 0) Scope, versioning, and mental model

## 0.0 Version stance

Use **Pydantic 2.13.4** as the stable call-signature and behavior target for this reference. Do not copy examples from `dev` or 2.14 prerelease documentation into a 2.13 application without checking availability. Pydantic's docs explicitly identify the stable documentation as v2.13.4, while PyPI marks 2.14.0b1 as a prerelease.

The highest-confidence hierarchy for generated code is:

1. installed package / pinned source tag for exact signatures;
2. stable API reference for public members and defaults;
3. stable concept documentation for semantics and intended patterns;
4. changelog for version-specific behavior;
5. development/prerelease docs only when deliberately targeting prerelease code.

## 0.1 What Pydantic is

Pydantic is a **runtime validation, parsing, serialization, and schema-construction system driven by Python type annotations**. It guarantees that successfully produced output objects conform to the declared types and constraints; it does **not** guarantee that the original input already had those types. In lax mode, Pydantic may coerce input to produce the required output. [DOCS-HOME] [MODELS]

This distinction should shape all architecture discussions:

```text
untrusted / weakly typed input
  -> Pydantic validation + conversion
      -> typed Python output
          -> application/domain logic
              -> Pydantic serialization / schema contract
```

Agent-safe phrasing:

> Pydantic validates and transforms data into values that conform to a Python type contract; it is not merely an assertion library that checks input without changing it.

## 0.2 What Pydantic is not

Pydantic is not:

- a static type checker;
- a database ORM;
- a persistence layer;
- an API framework;
- a security sandbox;
- a general business-rule engine;
- a replacement for explicit authorization or trust-boundary design.

It integrates with all of those domains, but its responsibility is primarily **data contract execution**.

## 0.3 Canonical processing pipeline

```text
Python annotation / model class
  -> annotation resolution
  -> FieldInfo + model configuration + decorator discovery
  -> pydantic CoreSchema graph
  -> SchemaValidator + SchemaSerializer compilation
  -> validate_python / validate_json / validate_strings
  -> Python value / BaseModel instance
  -> to_python / to_json
  -> JSON Schema generation when requested
```

A `BaseModel` class exposes the compiled pieces through attributes such as:

```python
Model.__pydantic_core_schema__
Model.__pydantic_validator__
Model.__pydantic_serializer__
Model.model_fields
```

These are critical debugging/introspection surfaces, but only documented public methods should be treated as stable application APIs unless an integration explicitly depends on internals.

## 0.4 Validation and serialization are separate contracts

Pydantic V2 deliberately separates validation and serialization. A field can:

- accept multiple input forms;
- normalize to one Python representation;
- serialize to a different JSON-compatible representation;
- produce different JSON Schema depending on `mode='validation'` or `mode='serialization'`.

Example:

```python
from datetime import datetime
from pydantic import BaseModel

class Event(BaseModel):
    at: datetime

x = Event.model_validate({'at': '2026-08-19T12:30:00Z'})
assert isinstance(x.at, datetime)

python_value = x.model_dump(mode='python')
json_value = x.model_dump(mode='json')
json_bytes = x.model_dump_json()
```

Do not conflate the Python in-memory type with the JSON transport type.

## 0.5 Minimum vocabulary

| Term | Meaning | Agent use |
|---|---|---|
| `BaseModel` | class-based model with named fields and compiled validator/serializer | default object-shaped boundary |
| `FieldInfo` | metadata/configuration describing a field | inspect defaults, aliases, constraints, metadata |
| `ConfigDict` | typed configuration mapping for model behavior | model-wide validation/serialization policy |
| `CoreSchema` | schema graph consumed by pydantic-core | custom types and advanced debugging |
| `SchemaValidator` | compiled Rust-backed validator | executes validation |
| `SchemaSerializer` | compiled Rust-backed serializer | executes serialization |
| `TypeAdapter` | validator/serializer/schema wrapper for any supported type | validate lists, unions, dataclasses, TypedDicts without models |
| `RootModel[T]` | model whose payload is one root value | wrapper when object-shaped fields are artificial |
| `ValidationInfo` | request-like metadata available to validators | context, config, field name, prior data, mode |
| `SerializationInfo` | metadata available to serializers | mode, include/exclude/context and serializer state |
| JSON Schema | external machine-readable type/schema contract | OpenAPI, SDKs, codegen, contract tests |

## 0.6 Core invariants for LLM programming agents

**Invariant 1 — successful validation guarantees output, not input.**
Coercion may occur unless strict behavior is enabled.

**Invariant 2 — validation is not serialization.**
Input aliases, output aliases, validation schemas, and serialization schemas may differ.

**Invariant 3 — model construction has trusted and validated paths.**
`model_construct()` skips validation; never substitute it casually for `model_validate()`.

**Invariant 4 — optionality and default are separate concepts.**
`T | None` means the value may be `None`; it does not automatically mean the field has a default of `None` in Pydantic V2.

**Invariant 5 — schema-build cost is real.**
Models and `TypeAdapter`s compile validators/serializers. Reuse them in hot paths.

**Invariant 6 — nested model output is schema-controlled by default.**
Subclass-only attributes are not automatically dumped through a base-class field contract; opt into polymorphism deliberately.

**Invariant 7 — JSON input and Python input can have different strict behavior.**
Some values such as dates are necessarily represented as strings in JSON and can still be accepted in JSON strict mode.

**Invariant 8 — external contracts deserve explicit configuration.**
Aliases, extra handling, strictness, serialization, and JSON Schema options are API-design decisions, not cosmetic flags.

## 0.7 Anti-pattern inventory

- Treating `BaseModel` as a passive typed `dict` with no conversion semantics.
- Adding validators for constraints already expressible through type annotations or `Field` metadata.
- Calling `model_construct()` on untrusted input.
- Recreating `TypeAdapter` inside a hot loop.
- Using `serialize_as_any=True` globally without considering data exposure.
- Relying on Pydantic to enforce authorization.
- Using deprecated V1 method names in new V2 code.
- Assuming JSON Schema is identical for validation and serialization.
- Exposing `ValidationError` input values verbatim when they may contain secrets.

Sources: [DOCS-HOME], [MODELS], [SER], [STRICT], [PERF].

---

# Pydantic Advanced — 1) Installation, dependencies, extras, version pinning, and project layout

## 1.0 Stable installation

```bash
python -m pip install 'pydantic==2.13.4'
```

With `uv`:

```bash
uv add 'pydantic==2.13.4'
```

Core stable dependencies include `pydantic-core==2.46.4`, `annotated-types`, `typing-extensions`, and `typing-inspection`. `pydantic-core` contains the Rust validation/serialization implementation.

## 1.1 Optional extras

Pydantic 2.13.4 exposes lightweight extras for optional validation capabilities:

```bash
python -m pip install 'pydantic[email]==2.13.4'
python -m pip install 'pydantic[timezone]==2.13.4'
```

- `email` adds `email-validator`, used by types such as `EmailStr` and `NameEmail`.
- `timezone` provides timezone-data support where the platform needs it.

Settings are **not** a core extra in V2. Install the separate package:

```bash
python -m pip install 'pydantic-settings==2.15.0'
```

## 1.2 Recommended dependency policy

For applications and shared infrastructure libraries, pin at least the Pydantic minor line, and use exact pins when schemas or serialization outputs are treated as stable external contracts.

Example `pyproject.toml`:

```toml
[project]
requires-python = ">=3.10"
dependencies = [
  "pydantic==2.13.4",
  "pydantic-settings==2.15.0",
]
```

Why exact pins can be justified:

- JSON Schema details can change across minor versions;
- serialization behavior evolves;
- validator edge cases and union-selection behavior change;
- integrations often snapshot OpenAPI/JSON Schema output;
- Pydantic follows a version policy that permits selected behavior changes within the V2 series.

## 1.3 Do not pin `pydantic-core` independently unless you have a reason

Pydantic 2.13.4 pins the compatible core version itself. Normal application code should declare `pydantic`, not a manually chosen unrelated `pydantic-core` version.

Bad:

```toml
pydantic = "2.13.4"
pydantic-core = "2.50.0"  # do not force an arbitrary incompatible core
```

Preferred:

```toml
pydantic = "2.13.4"
```

Directly depend on `pydantic-core` only if your package intentionally builds CoreSchema/SchemaValidator functionality without Pydantic's higher-level APIs.

## 1.4 Python-version boundary

Stable Pydantic 2.13.4 supports Python >=3.9. The 2.14 prerelease line drops Python 3.9 and moves the floor to 3.10. If your library still supports 3.9, treat 2.14 as an explicit migration event.

## 1.5 Project layout

Recommended application organization:

```text
src/my_app/
  domain/
    models.py              # domain-oriented validated values/models
  api/
    schemas.py             # transport-specific request/response contracts
  settings.py              # BaseSettings + SettingsConfigDict
  adapters/
    database.py            # ORM -> Pydantic mapping boundaries
  validation/
    types.py               # reusable Annotated types
    adapters.py            # reusable TypeAdapter instances
    errors.py              # application error translation
  serialization/
    policies.py            # aliases/include/exclude/versioned output
```

Avoid a single giant `models.py` that mixes transport DTOs, persistence records, domain state, settings and external-service payloads. Pydantic makes those objects syntactically similar, but their **trust and compatibility obligations differ**.

## 1.6 Import policy

Canonical imports:

```python
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    TypeAdapter,
    ValidationError,
)
```

V1 compatibility must be visibly namespaced:

```python
from pydantic.v1 import BaseModel as BaseModelV1
```

Do not intermix V1/V2 models accidentally inside one inheritance hierarchy.

## 1.7 Environment verification

```bash
python - <<'PY'
import pydantic
import pydantic_core
print('pydantic:', pydantic.__version__)
print('pydantic-core:', pydantic_core.__version__)
PY
```

Agent checklist:

```text
[ ] Pin the Pydantic version used by the project.
[ ] Let Pydantic select its compatible pydantic-core version.
[ ] Add [email] only if email types are used.
[ ] Install pydantic-settings separately in V2.
[ ] Record Python-version support explicitly.
[ ] Keep V1 compatibility imports visibly under pydantic.v1.
[ ] Separate transport/domain/settings/persistence model responsibilities.
```

Sources: [DOCS-HOME], installation docs, [PYPI], [MIGRATION].

---

# Pydantic Advanced — 2) First executable validation/serialization application

## 2.0 Minimal model

```python
from datetime import datetime
from pydantic import BaseModel, Field

class User(BaseModel):
    id: int = Field(gt=0)
    name: str = Field(min_length=1, max_length=100)
    signup_at: datetime | None = None
    tags: list[str] = Field(default_factory=list)

payload = {
    'id': '123',
    'name': 'Ada',
    'signup_at': '2026-08-19T20:00:00Z',
    'tags': ['admin'],
}

user = User.model_validate(payload)
print(user)
print(user.model_dump())
print(user.model_dump(mode='json'))
print(user.model_dump_json(indent=2))
print(User.model_json_schema())
```

## 2.1 What happens

```text
payload dict
  -> model_validate
  -> compiled SchemaValidator
  -> int coercion ('123' -> 123)
  -> string length checks
  -> datetime parsing
  -> list element validation
  -> User instance
  -> model_dump / model_dump_json through SchemaSerializer
```

## 2.2 Validation errors are structured

```python
from pydantic import ValidationError

try:
    User.model_validate({'id': 0, 'name': ''})
except ValidationError as exc:
    print(exc.errors())
    print(exc.json())
```

Use `errors()` for machine handling. Do not parse the human-formatted `str(exc)` representation.

## 2.3 JSON-native validation

```python
raw = b'{"id":"123","name":"Ada","tags":["a","b"]}'
user = User.model_validate_json(raw)
```

For JSON payloads, prefer `model_validate_json()` over `json.loads(...)` followed by `model_validate()` in the general case; it avoids a separate Python JSON parse and lets pydantic-core validate directly. [PERF]

## 2.4 Strict validation

```python
from pydantic import ValidationError

try:
    User.model_validate(payload, strict=True)
except ValidationError as exc:
    print(exc)
```

`'123'` no longer satisfies an `int` field in Python strict mode.

## 2.5 `TypeAdapter` for non-model contracts

```python
from pydantic import TypeAdapter

UserIds = TypeAdapter(list[int])
ids = UserIds.validate_python(['1', 2, 3])
assert ids == [1, 2, 3]

print(UserIds.dump_json(ids))
print(UserIds.json_schema())
```

Use `TypeAdapter` instead of inventing a one-field wrapper model when the true contract is `list[int]`, `dict[str, T]`, a union, a `TypedDict`, a dataclass, or another supported type.

## 2.6 Test the full contract, not just field values

```python
def test_user_contract():
    user = User.model_validate({'id': '3', 'name': 'Ada'})
    assert user.id == 3
    assert user.model_dump(mode='json') == {
        'id': 3,
        'name': 'Ada',
        'signup_at': None,
        'tags': [],
    }
```

For public APIs, additionally snapshot selected JSON Schema/OpenAPI output and test serialization aliases/field exclusion.

Sources: [MODELS], [FIELDS], [TYPEADAPTER], [PERF].

---

# Pydantic Advanced — 3) Architecture: Python annotations → CoreSchema → Rust validator/serializer

## 3.0 Architectural compression

Pydantic V2 is best understood as a **schema compiler plus runtime engine**.

```text
Python type / model
  -> namespace + annotation resolution
  -> field/config/decorator metadata
  -> CoreSchema generation
  -> pydantic-core SchemaValidator / SchemaSerializer
  -> repeated runtime calls
```

The expensive schema-construction work is intentionally separated from repeated validation. This is why reusing a model class or `TypeAdapter` is usually faster than reconstructing adapters/schemas per call.

## 3.1 Core schema is the internal execution IR

`pydantic_core.core_schema.CoreSchema` describes validation/serialization logic using a typed schema graph. It can represent:

- scalar parsers;
- lists/tuples/sets/dicts;
- unions and tagged unions;
- typed dictionaries;
- models/dataclasses;
- function validators;
- serialization hooks;
- references/definitions for recursion;
- defaults/nullability/constraints.

Conceptually:

```text
Annotated[int, Field(gt=0)]
  -> int_schema(gt=0)

list[Annotated[int, Field(gt=0)]]
  -> list_schema(items_schema=int_schema(gt=0))
```

Do not hand-build CoreSchema for ordinary model code; use it when implementing custom types or specialized framework integrations.

## 3.2 `SchemaValidator`

A model's compiled validator is available through:

```python
class M(BaseModel):
    x: int

validator = M.__pydantic_validator__
```

The higher-level methods delegate into this engine. Prefer `M.model_validate(...)` for application code because it preserves Pydantic's documented model lifecycle and API stability.

## 3.3 `SchemaSerializer`

Similarly:

```python
serializer = M.__pydantic_serializer__
```

`model_dump()` and `model_dump_json()` are high-level model-facing wrappers around compiled serialization behavior.

## 3.4 Annotation resolution is a first-class subsystem

Pydantic must resolve:

- forward references;
- type aliases;
- nested classes;
- generic type variables;
- `Annotated` metadata;
- inherited annotations;
- Python-version-specific typing forms.

If types are unavailable at class-build time, the model can be incomplete and later require `model_rebuild()`.

## 3.5 Model-build lifecycle

Approximate lifecycle:

```text
class body executes
  -> ModelMetaclass discovers fields/private attrs/config/decorators
  -> annotations are resolved as far as possible
  -> core schema is generated
  -> validator/serializer are built
  -> model marked complete
```

`ConfigDict(defer_build=True)` changes this by postponing validator/serializer construction until first use. This can reduce import/startup cost in applications with many rarely-used models, at the cost of moving compilation latency to the first validation/schema operation.

## 3.6 Why schema internals matter for performance

Schema complexity influences:

- model class creation time;
- memory footprint;
- validator compilation cost;
- union branch behavior;
- JSON Schema traversal;
- serialization path complexity.

A giant deeply recursive dynamically generated model graph can be expensive even if each individual validation is fast.

## 3.7 Extension boundary

For custom types, prefer this progression:

```text
1. standard annotation / Field constraints
2. Annotated + BeforeValidator/AfterValidator/serializer metadata
3. ValidateAs / InstanceOf / SkipValidation where appropriate
4. __get_pydantic_core_schema__ / GetPydanticSchema
5. direct pydantic-core usage only for truly low-level needs
```

Every step downward increases power and maintenance burden.

## 3.8 Debugging schema behavior

Useful inspection:

```python
from pprint import pprint

pprint(M.model_fields)
pprint(M.__pydantic_core_schema__)
print(M.model_json_schema())
```

Do not use private/internal schema dictionaries as long-lived serialized contracts. JSON Schema is the supported external schema representation.

Sources: Pydantic internals architecture docs, Pydantic Core API, [MODELS], [TYPEADAPTER], [PERF].

---

# Pydantic Advanced — 4) `BaseModel` definition and object model

## 4.0 Basic declaration

```python
from pydantic import BaseModel, ConfigDict, Field

class Account(BaseModel):
    model_config = ConfigDict(extra='forbid')

    account_id: int
    owner: str
    balance: float = 0.0
    labels: set[str] = Field(default_factory=set)
```

Annotated class attributes become Pydantic fields unless they are recognized as class variables, private attributes or ignored types.

## 4.1 Class-level model metadata

High-value attributes:

```python
Account.model_fields
Account.model_computed_fields
Account.__pydantic_core_schema__
Account.__pydantic_validator__
Account.__pydantic_serializer__
Account.__pydantic_complete__
```

Use `Account.model_fields`, not `account.model_fields`; instance access is deprecated in the V2 line and intended for removal in V3.

## 4.2 Instance state

Common instance attributes/properties:

```python
m.__pydantic_fields_set__
m.model_fields_set
m.__pydantic_extra__
m.__pydantic_private__
```

- `model_fields_set` identifies fields explicitly supplied or later assigned, distinct from fields populated only from defaults.
- `__pydantic_extra__` contains allowed unknown fields when `extra='allow'`.
- `__pydantic_private__` stores private-attribute state.

## 4.3 Constructor signature

Pydantic synthesizes a useful `inspect.Signature` for models. Static type checkers may also synthesize constructor behavior based on fields/config/plugin support.

```python
import inspect
print(inspect.signature(Account))
```

Aliases and `extra` policy can affect the runtime signature.

## 4.4 Mutable defaults

Pydantic can deep-copy unhashable default values per model instance, which prevents the common Python shared-list trap in many cases. Still prefer explicit factories for mutable containers because the intent is unambiguous:

```python
class Model(BaseModel):
    items: list[int] = Field(default_factory=list)
```

## 4.5 Model configuration inheritance

Configuration is inherited and merged through model inheritance, subject to Pydantic's configuration rules. Use a custom shared base class for organization-wide defaults:

```python
class ContractModel(BaseModel):
    model_config = ConfigDict(
        extra='forbid',
        validate_by_alias=True,
        validate_by_name=True,
    )
```

Avoid overusing a global base when different trust boundaries legitimately need different `extra`, strictness or alias behavior.

## 4.6 Model methods to know

| Method | Purpose |
|---|---|
| `model_validate` | validate Python/object input |
| `model_validate_json` | validate JSON bytes/string directly |
| `model_validate_strings` | validate string-oriented mappings |
| `model_construct` | construct without validation |
| `model_dump` | serialize to Python or JSON-compatible Python values |
| `model_dump_json` | serialize directly to JSON text |
| `model_copy` | shallow/deep model copy; optional unvalidated update |
| `model_json_schema` | generate JSON Schema |
| `model_rebuild` | rebuild core schema after unresolved/changed references |

## 4.7 Fields vs properties vs ClassVar

```python
from typing import ClassVar

class Example(BaseModel):
    kind: ClassVar[str] = 'example'  # not a Pydantic field
    value: int

    @property
    def double(self) -> int:
        return self.value * 2
```

A normal property is not automatically serialized. Use `@computed_field` when it belongs in the Pydantic serialization/schema surface.

## 4.8 Agent rules

- Declare every model field with an annotation.
- Prefer `Field(default_factory=...)` for mutable/runtime-generated defaults.
- Keep configuration close to the model boundary that owns the behavior.
- Use class-level `model_fields` introspection.
- Do not mutate `__pydantic_*` internals directly unless implementing framework-level behavior with tests.

Sources: [MODELS], BaseModel API, [CONFIG], [FIELDS].

---

# Pydantic Advanced — 5) Validation entry points: `__init__`, `model_validate`, JSON and strings

## 5.0 Validation entry-point matrix

| Entry point | Input | Best use |
|---|---|---|
| `Model(**kwargs)` | keyword mapping | ergonomic ordinary Python construction |
| `Model.model_validate(obj)` | mapping/model/object | explicit validation boundary, supports runtime flags/context |
| `Model.model_validate_json(data)` | JSON string/bytes/bytearray | direct JSON ingestion; usually fastest JSON path |
| `Model.model_validate_strings(obj)` | string-valued nested mapping | environment/form/string-origin data |
| `TypeAdapter.validate_*` | arbitrary type | non-model contracts |

## 5.1 `__init__`

```python
m = Account(account_id='1', owner='Ada')
```

The model initializer validates keyword arguments. It is ergonomic but less explicit than `model_validate` at system boundaries.

## 5.2 `model_validate`

Representative call surface includes runtime options such as strictness, extra behavior, attribute extraction, context, and alias policy:

```python
m = Account.model_validate(
    {'account_id': '1', 'owner': 'Ada'},
    strict=False,
    extra='forbid',
    context={'source': 'api'},
    by_alias=True,
    by_name=True,
)
```

Runtime flags can override selected model configuration. This is valuable when a reusable model must be consumed under a stricter one-off boundary.

## 5.3 `from_attributes`

To validate an object by reading attributes rather than a mapping:

```python
class UserDTO(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    id: int
    name: str

class ORMUser:
    id = 1
    name = 'Ada'

u = UserDTO.model_validate(ORMUser())
```

This replaces the common V1 `orm_mode` pattern. Pydantic is not an ORM; it simply extracts declared attributes.

## 5.4 `model_validate_json`

```python
m = Account.model_validate_json(
    '{"account_id": "1", "owner": "Ada"}'
)
```

Advantages:

- avoids `json.loads()` materialization as a separate Python step;
- pydantic-core parses and validates in one path;
- can be more memory/CPU efficient;
- provides JSON-specific coercion semantics.

The 2.13.1 and 2.13.2 patch releases fixed `ValidationInfo.data` and `ValidationInfo.field_name` availability on this path. Pinning 2.13.4 avoids those early 2.13 regressions.

## 5.5 `model_validate_strings`

This is useful when a nested mapping originates from a source where every leaf is text:

```python
class Query(BaseModel):
    count: int
    enabled: bool

q = Query.model_validate_strings({'count': '10', 'enabled': 'true'})
```

Do not use it to pretend arbitrary JSON-like objects are strings; use the entry point matching the real source format.

## 5.6 Context

```python
from pydantic import field_validator, ValidationInfo

class TenantValue(BaseModel):
    value: int

    @field_validator('value')
    @classmethod
    def enforce_limit(cls, v: int, info: ValidationInfo) -> int:
        limit = (info.context or {}).get('limit', 100)
        if v > limit:
            raise ValueError('value exceeds tenant limit')
        return v

TenantValue.model_validate({'value': 5}, context={'limit': 10})
```

Context is the supported way to pass request/runtime information to validators without global variables.

## 5.7 `extra` at call time

In 2.12+, validation methods can override `extra` behavior:

```python
Model.model_validate(payload, extra='forbid')
```

This can be valuable for migration: keep permissive model defaults for legacy internal callers but enforce strict boundaries at new APIs.

## 5.8 Strict mode caveat: Python vs JSON

Strict Python validation often requires exact Python types. Strict JSON validation can still accept string representations for types that JSON cannot natively express, such as dates. Strict means “strict with respect to the input representation,” not “JSON may only contain native Python runtime types.” [STRICT]

Sources: BaseModel API, [STRICT], [JSON], [CHANGELOG].

---

# Pydantic Advanced — 6) Trusted construction, copying, equality, extras, field-set tracking, and private state

## 6.0 `model_construct()` is a trust-boundary method

```python
m = Account.model_construct(
    account_id='not validated',
    owner=123,
)
```

`model_construct()` bypasses validation. Use it only when data has already been validated or is generated by trusted code that can uphold the model invariants.

Good use cases:

- deserializing from an internal cache that stores already validated structures;
- performance-sensitive reconstruction after an upstream validated protocol;
- tests that intentionally need invalid internal state;
- custom serializers/framework internals.

Bad use case: “validation seems slow, so use construct for API payloads.”

## 6.1 `model_copy()`

```python
m2 = m.model_copy()
m3 = m.model_copy(deep=True)
m4 = m.model_copy(update={'owner': 'Grace'})
```

The `update` data is **not validated**. Treat it as trusted. For validated patch/update flows, dump/merge/revalidate or use an explicit patch model.

Validated update pattern:

```python
data = original.model_dump()
data.update(patch)
updated = Account.model_validate(data)
```

## 6.2 Field-set tracking

```python
class Patchable(BaseModel):
    x: int = 0
    y: int = 0

m = Patchable(x=1)
assert m.model_fields_set == {'x'}
```

This powers common patch semantics:

```python
m.model_dump(exclude_unset=True)
```

Pydantic 2.13 also tracks allowed extra fields assigned after initialization in `model_fields_set`.

## 6.3 Extra fields

`ConfigDict(extra=...)` choices:

- `'ignore'` — silently discard extras; default.
- `'forbid'` — validation error.
- `'allow'` — retain extras in `__pydantic_extra__`.

For external contracts, `'forbid'` is often safer because spelling mistakes and unexpected producer changes fail visibly.

For forward-compatible event envelopes, `'allow'` or `'ignore'` can be intentional.

## 6.4 Typed extras

```python
class Extensible(BaseModel):
    __pydantic_extra__: dict[str, int] = Field(init=False)
    x: int
    model_config = ConfigDict(extra='allow')
```

Now allowed extras are validated as integers.

## 6.5 Private attributes

Private attributes are not normal fields and are not validated/serialized as model fields:

```python
from pydantic import PrivateAttr

class Cached(BaseModel):
    x: int
    _cache: dict[str, object] = PrivateAttr(default_factory=dict)
```

In Pydantic 2.13, private-attribute default factories can receive validated model data when their callable signature supports it, enabling private state derived from validated fields.

## 6.6 Equality

Pydantic V2 model equality semantics differ from V1; models are no longer simply equal to equivalent dictionaries. Equality accounts for model type and model/private/extra state under Pydantic's rules. Do not use `model == model.model_dump()` as a portable assumption.

## 6.7 Frozen models

```python
class Key(BaseModel):
    model_config = ConfigDict(frozen=True)
    tenant: str
    id: int
```

This prevents normal attribute assignment and may make instances hashable if fields are hashable. It is **faux immutability**; mutable child objects can still be mutated unless they are themselves immutable.

## 6.8 Patch-model pattern

```python
class UserPatch(BaseModel):
    name: str | None = None
    enabled: bool | None = None

patch = UserPatch.model_validate(payload)
changes = patch.model_dump(exclude_unset=True)
```

Note that `None` now has domain meaning. If “missing” must differ from “explicit null,” rely on field-set tracking or the experimental `MISSING` sentinel where appropriate, rather than conflating them.

Sources: [MODELS], BaseModel API, [CONFIG], [V213].

---

# Pydantic Advanced — 7) Fields, `FieldInfo`, `Annotated`, metadata, constraints, and signatures

## 7.0 Two declaration styles

Assignment style:

```python
class Product(BaseModel):
    name: str = Field(min_length=1, max_length=100)
    price: float = Field(gt=0)
```

Annotated style:

```python
from typing import Annotated

PositivePrice = Annotated[float, Field(gt=0)]

class Product(BaseModel):
    name: Annotated[str, Field(min_length=1, max_length=100)]
    price: PositivePrice
```

## 7.1 Why `Annotated` is powerful

`Annotated` can stack multiple metadata providers:

```python
from pydantic import AfterValidator, PlainSerializer, WithJsonSchema

NormalizedCode = Annotated[
    str,
    Field(min_length=2, max_length=16),
    AfterValidator(str.upper),
    PlainSerializer(lambda v: v.lower()),
    WithJsonSchema({'type': 'string', 'pattern': '^[A-Z0-9]+$'}),
]
```

This turns a reusable Python type alias into a validation + serialization + schema contract.

## 7.2 Static type checker caveat

Static type checkers synthesize constructor signatures more reliably from assignment-form `default`, `default_factory`, and `alias` than from arbitrary `Annotated` metadata. When constructor typing matters, keep those particular semantics visible to the type checker.

## 7.3 High-value `Field()` parameters

Identity/default:

```text
default
default_factory
alias
validation_alias
serialization_alias
alias_priority
```

Schema/documentation:

```text
title
field_title_generator
description
examples
json_schema_extra
deprecated
```

Validation/behavior:

```text
strict
validate_default
frozen
gt / ge / lt / le
multiple_of
allow_inf_nan
min_length / max_length
pattern
max_digits / decimal_places
coerce_numbers_to_str
union_mode
fail_fast
```

Serialization:

```text
exclude
exclude_if
repr
```

Dataclass integration:

```text
init
init_var
kw_only
```

Union selection:

```text
discriminator
union_mode
```

## 7.4 Constraints are schema-visible

```python
class Page(BaseModel):
    size: int = Field(ge=1, le=500)
```

This creates runtime validation and JSON Schema bounds. Prefer it to a custom validator when the rule is declarative and standardized.

## 7.5 Field introspection

```python
info = Product.model_fields['price']
print(info.annotation)
print(info.default)
print(info.metadata)
print(info.alias)
```

`FieldInfo.metadata` contains metadata derived from `Annotated` constraints and other markers.

## 7.6 Requiredness

```python
class M(BaseModel):
    required_nullable: str | None
    optional_nullable: str | None = None
```

The first field is required but may contain `None`. The second is not required because it has a default.

V1 code often assumes `Optional[T]` automatically means default `None`; audit this during migration.

## 7.7 `exclude_if`

```python
class Result(BaseModel):
    value: int
    debug: str | None = Field(default=None, exclude_if=lambda v: v is None)
```

`exclude_if` gives value-dependent serialization exclusion at the field definition. In 2.13 it also applies to computed fields.

## 7.8 `deprecated`

Marking a field deprecated affects JSON Schema/documentation and can emit access-time warnings depending on the configuration/usage. Use it to stage contract migrations rather than silently deleting fields.

## 7.9 Agent rules

- Prefer `Annotated` for reusable constrained/customized type contracts.
- Prefer `Field` constraints over validators for simple ranges/length/pattern rules.
- Preserve assignment syntax when defaults/aliases must be visible to static checkers.
- Do not infer “optional field” merely from `T | None`.
- Treat alias and serialization configuration as an external API contract.

Sources: [FIELDS], Fields API, [JSONSCHEMA], [V213].

---

# Pydantic Advanced — 8) Defaults, `default_factory`, validated data, and default validation

## 8.0 Plain defaults

```python
class M(BaseModel):
    retries: int = 3
```

By default, Pydantic generally does **not** validate default values. Enable validation when defaults need conversion/constraint enforcement.

## 8.1 `validate_default`

```python
class M(BaseModel):
    count: int = Field(default='3', validate_default=True)

assert M().count == 3
```

Or model-wide:

```python
class M(BaseModel):
    model_config = ConfigDict(validate_default=True)
```

Use this when defaults come from constants that may drift or when configuration such as `use_enum_values` needs to apply to defaults.

## 8.2 Factory without arguments

```python
from uuid import uuid4

class Job(BaseModel):
    id: str = Field(default_factory=lambda: str(uuid4()))
```

Factories run per instance.

## 8.3 Factory using already validated data

Pydantic can pass already validated field data to a one-argument factory:

```python
class User(BaseModel):
    email: str
    username: str = Field(default_factory=lambda data: data['email'].split('@')[0])
```

**Field order matters.** `email` must already have been validated when `username`'s factory executes.

This is useful for derived defaults but should not become hidden cross-field business logic. Complex invariant logic is often clearer in a model validator.

## 8.4 Factory failure semantics

If a prior field failed validation, a factory that requires validated data may not be callable. Design errors so the primary invalid field remains the actionable failure.

## 8.5 Deep-copy behavior for defaults

Pydantic deep-copies non-hashable defaults per instance rather than reusing the same object in common cases. Prefer factories anyway:

```python
items: list[str] = Field(default_factory=list)
```

This is clearer to humans, linters and agents.

## 8.6 Default vs unset

```python
class M(BaseModel):
    x: int = 10

m1 = M()
m2 = M(x=10)

assert m1.model_fields_set == set()
assert m2.model_fields_set == {'x'}
```

Serialization flags can distinguish these states:

```python
m1.model_dump(exclude_unset=True)  # {}
m2.model_dump(exclude_unset=True)  # {'x': 10}
```

This distinction is central to PATCH/event-delta/config-override semantics.

## 8.7 Default enum values and `use_enum_values`

Because enum value extraction occurs during validation, an enum default needs `validate_default=True` if `ConfigDict(use_enum_values=True)` should transform it.

## 8.8 Private-attribute factories in 2.13

Private attribute factories gained support for receiving validated model data in 2.13. Use this for internal cached/derived state only; if the value belongs to the public data contract, make it a field or computed field instead.

Sources: [FIELDS], [CONFIG], [V213].

---

# Pydantic Advanced — 9) `ConfigDict`: complete configuration model

## 9.0 Configuration syntax

```python
from pydantic import BaseModel, ConfigDict

class APIModel(BaseModel):
    model_config = ConfigDict(
        extra='forbid',
        validate_by_alias=True,
        validate_by_name=True,
        serialize_by_alias=True,
        str_strip_whitespace=True,
    )
```

Selected configuration values may also be set as class keyword arguments:

```python
class User(BaseModel, validate_assignment=True):
    name: str
```

## 9.1 Configuration category map

### String normalization

```text
str_to_lower
str_to_upper
str_strip_whitespace
str_min_length
str_max_length
```

Use model-wide string normalization only when it truly applies to every string field. Domain-specific normalization belongs in reusable annotated types or validators.

### Extra and mutability

```text
extra = 'ignore' | 'allow' | 'forbid'
frozen
validate_assignment
revalidate_instances
```

`validate_assignment` validates direct attribute assignment to the configured model. It does not make the entire nested object graph immutable or automatically revalidate arbitrary in-place mutation of mutable children.

`revalidate_instances` controls whether nested model/dataclass instances are revalidated when used as input.

### Alias/input extraction

```text
from_attributes
loc_by_alias
alias_generator
validate_by_alias
validate_by_name
populate_by_name        # discouraged in 2.11+, target deprecation in V3
```

Prefer the explicit pair `validate_by_alias` / `validate_by_name` in new code.

### Validation policy

```text
strict
validate_default
validate_return
arbitrary_types_allowed
allow_inf_nan
coerce_numbers_to_str
regex_engine
validation_error_cause
```

### Serialization / JSON representations

```text
ser_json_timedelta
ser_json_temporal
val_temporal_unit
ser_json_bytes
val_json_bytes
ser_json_inf_nan
serialize_by_alias
polymorphic_serialization
url_preserve_empty_path
```

`ser_json_timedelta` is a legacy/specialized setting being superseded by broader temporal serialization controls; avoid building new architecture around deprecated behavior.

### Schema/documentation

```text
title
model_title_generator
field_title_generator
json_schema_extra
json_schema_serialization_defaults_required
json_schema_mode_override
use_attribute_docstrings
```

### Build/performance/integration

```text
ignored_types
protected_namespaces
hide_input_in_errors
defer_build
plugin_settings
cache_strings
schema_generator  # deprecated/legacy API surface
```

## 9.2 `extra`

External request model default recommendation:

```python
ConfigDict(extra='forbid')
```

Event/forward-compatible ingestion models may deliberately choose `ignore` or `allow`. Do not choose based on convenience; choose based on producer/consumer compatibility policy.

## 9.3 `strict`

```python
class StrictContract(BaseModel):
    model_config = ConfigDict(strict=True)
    count: int
```

Can be overridden at field level. Strictness is best used at clearly defined trust boundaries because enabling it across a mature lax-coercion codebase can be breaking.

## 9.4 `validate_assignment`

```python
class AssignmentSafe(BaseModel):
    model_config = ConfigDict(validate_assignment=True)
    count: int

m = AssignmentSafe(count=1)
m.count = '2'  # validated/coerced unless field strictness forbids it
```

## 9.5 `from_attributes`

Use for ORM/domain-object extraction. It also affects discriminator lookup for tagged unions when attribute-based input is used.

## 9.6 Alias configuration

Pydantic's default behavior is asymmetric: aliases are commonly accepted for validation, while serialization by alias is disabled by default. The docs anticipate changing serialization alias defaults in V3. Set the behavior explicitly for public contracts rather than relying on defaults.

```python
ConfigDict(
    validate_by_alias=True,
    validate_by_name=True,
    serialize_by_alias=True,
)
```

## 9.7 `defer_build`

For applications importing hundreds/thousands of models where many are never validated:

```python
class Lazy(BaseModel):
    model_config = ConfigDict(defer_build=True)
    x: int
```

Tradeoff:

```text
lower import/startup cost
  vs
first-use validation/schema latency
```

Benchmark your actual model graph.

## 9.8 `cache_strings`

Pydantic/Jiter can cache repeated strings to reduce repeated allocations at a memory cost. Configurable modes allow broad caching, key-only caching or no caching. It matters most for JSON-heavy repeated-key workloads.

## 9.9 `regex_engine`

Pydantic can select regex implementation behavior. The default Rust regex engine favors linear-time behavior and safety; Python regex may be needed for features unsupported by Rust regex. Select deliberately if user-controlled patterns/inputs are involved.

## 9.10 `hide_input_in_errors`

Use when validation errors could otherwise render sensitive raw input values in logs or client-visible diagnostics.

```python
ConfigDict(hide_input_in_errors=True)
```

This is a security/logging hygiene control, not a replacement for redacting application logs.

## 9.11 `polymorphic_serialization`

New in 2.13. It provides a controlled way to serialize subclass fields for model/dataclass subclass instances. Prefer it over broad `serialize_as_any` when your goal is specifically model/dataclass polymorphism.

## 9.12 Configuration base-class pattern

```python
class ExternalContract(BaseModel):
    model_config = ConfigDict(
        extra='forbid',
        validate_by_alias=True,
        validate_by_name=False,
        serialize_by_alias=True,
        hide_input_in_errors=True,
    )
```

Then subclass per API version. Do not use one organization-wide base if internal models need fundamentally different coercion/extra/serialization behavior.

## 9.13 Anti-patterns

- `populate_by_name=True` in new code without considering newer `validate_by_*` settings.
- `arbitrary_types_allowed=True` as a global escape hatch.
- `extra='ignore'` on a contract where unknown producer fields should fail.
- global `strict=True` without auditing JSON/Python call sites.
- relying on default alias serialization when external field names matter.
- enabling broad polymorphic serialization without a data-exposure review.

Sources: [CONFIG], [STRICT], [SER], [V213].

---

# Pydantic Advanced — 10) Strict mode, lax coercion, and the conversion contract

## 10.0 Lax is the default

Pydantic's default behavior is pragmatic conversion where reasonable:

```python
class M(BaseModel):
    x: int

assert M(x='123').x == 123
```

This is a feature, not a bug. The output type contract is satisfied.

## 10.1 Strict at call level

```python
M.model_validate({'x': '123'}, strict=True)
```

This fails for a Python string-to-int conversion.

## 10.2 Strict at field level

```python
class Mixed(BaseModel):
    id: int = Field(strict=True)
    count: int
```

Useful when identifiers must not be silently converted but user-entered quantities may be.

## 10.3 Strict metadata

```python
from typing import Annotated
from pydantic import Strict

StrictInt = Annotated[int, Strict()]
```

Reusable and composable.

## 10.4 Strict at model configuration level

```python
class StrictModel(BaseModel):
    model_config = ConfigDict(strict=True)
    x: int
    y: int = Field(strict=False)
```

Fields can selectively opt out.

## 10.5 JSON strictness differs intentionally

```python
from datetime import date
from pydantic import TypeAdapter

Date = TypeAdapter(date)

# Python strict rejects the string
# Date.validate_python('2000-01-01', strict=True)

# JSON strict can accept a JSON string because JSON has no date scalar
assert Date.validate_json('"2000-01-01"', strict=True) == date(2000, 1, 1)
```

This is central to API design: strict validation of a transport encoding is not necessarily “no parsing whatsoever.”

## 10.6 Conversion table

Pydantic publishes a conversion table describing accepted input types by target type and strict/lax mode. Use it before writing a validator solely to control coercion; many desired behaviors are already built in.

## 10.7 Common surprising conversions to audit

- numeric strings to integers/floats in lax mode;
- selected strings/bytes to booleans;
- strings to dates/datetimes/UUIDs/URLs;
- lists/tuples/sets between container shapes;
- integer/float conversion where lossless rules permit it;
- enum values vs enum instances;
- JSON string encodings for non-JSON-native types.

## 10.8 Boundary policy

Recommended approach:

```text
External API with strong producer contract
  -> strict where wire representation allows it
  -> declarative constraints
  -> extra='forbid'

Human-entered config/forms
  -> selected coercion helpful
  -> explicit normalization

Internal domain state
  -> validate at ingress
  -> avoid repeatedly reparsing trusted objects
```

## 10.9 Migration strategy to stricter validation

1. instrument current validation failures;
2. identify coercions callers rely on;
3. add field-level strictness first;
4. introduce runtime `strict=True` at selected ingress points;
5. only then consider model-wide strict config;
6. version public APIs if accepted wire formats change.

## 10.10 Strictness is not semantic validation

`strict=True` can ensure `42` is an integer rather than `'42'`. It cannot tell you whether 42 is a valid customer ID, whether the caller owns that customer or whether the transaction is legal. Those remain application/domain rules.

Sources: [STRICT], conversion table, [CONFIG].


---

# Pydantic Advanced — 11) Constraints, reusable annotated types, and constrained-type design

## 11.0 Prefer the annotated pattern

The modern Pydantic pattern is to compose ordinary Python types with `typing.Annotated` metadata rather than creating large families of special wrapper classes.

```python
from typing import Annotated
from pydantic import Field, TypeAdapter

PositiveInt = Annotated[int, Field(gt=0)]
NonEmptyName = Annotated[str, Field(min_length=1, max_length=100)]

PositiveInts = TypeAdapter(list[PositiveInt])
assert PositiveInts.validate_python(['1', 2]) == [1, 2]
```

## 11.1 `annotated-types`

Pydantic understands constraints from the `annotated-types` package, such as numeric bounds and lengths. This keeps type aliases usable across libraries that understand standard `Annotated` metadata.

Conceptual examples:

```python
from typing import Annotated
from annotated_types import Ge, Le, MinLen, MaxLen

Percent = Annotated[float, Ge(0), Le(100)]
ShortList = Annotated[list[int], MinLen(1), MaxLen(10)]
```

## 11.2 String constraints

`StringConstraints` is useful for reusable string policy:

```python
from typing import Annotated
from pydantic import StringConstraints

Identifier = Annotated[
    str,
    StringConstraints(
        strip_whitespace=True,
        min_length=1,
        max_length=64,
        pattern=r'^[A-Za-z0-9_-]+$',
    ),
]
```

Pydantic 2.13 adds `ascii_only`, allowing a reusable string type to explicitly reject non-ASCII content.

## 11.3 Strict reusable types

```python
from pydantic import Strict

StrictIdentifier = Annotated[str, Strict(), StringConstraints(min_length=1)]
```

## 11.4 Numeric constraints

```python
Port = Annotated[int, Field(ge=1, le=65535)]
Probability = Annotated[float, Field(ge=0.0, le=1.0, allow_inf_nan=False)]
EvenPositive = Annotated[int, Field(gt=0, multiple_of=2)]
```

## 11.5 Decimal constraints

```python
from decimal import Decimal

Money = Annotated[
    Decimal,
    Field(gt=Decimal('0'), max_digits=12, decimal_places=2),
]
```

Remember that JSON Schema/serialization representation may not be the same as Python `Decimal`. Contract tests should inspect both validation and serialization modes if consumers care.

## 11.6 Container constraints

```python
NonEmptyTags = Annotated[list[str], Field(min_length=1, max_length=50)]
```

Item constraints belong on the item type:

```python
Tag = Annotated[str, Field(min_length=1, max_length=32)]
Tags = Annotated[list[Tag], Field(max_length=50)]
```

## 11.7 Avoid deprecated constrained factory types in new code

V1-era factories such as `constr`, `conint`, and related constrained helper types have migration/static-analysis disadvantages. Prefer ordinary types plus `Annotated` constraints in new V2 code.

## 11.8 Value-object boundary

A reusable constrained alias is ideal when the runtime value should remain the primitive:

```python
OrderId = Annotated[int, Field(gt=0)]
```

Use a `BaseModel`/dataclass/value-object class when the concept carries multiple fields, methods or state beyond the primitive value.

## 11.9 Constraint ordering

Multiple `Annotated` metadata entries form a pipeline. When mixing transformation validators and constraints, test the order explicitly. Do not assume a string is constrained before normalization unless the schema order actually produces that behavior.

## 11.10 Agent checklist

```text
[ ] Prefer Annotated + Field/annotated-types for reusable constraints.
[ ] Put item constraints on collection element types.
[ ] Use BaseModel/dataclass when the domain value needs more than one primitive.
[ ] Avoid custom validators when a declarative constraint exists.
[ ] Test normalization/constraint ordering when metadata is stacked.
[ ] Inspect generated JSON Schema for externally published aliases.
```

Sources: [FIELDS], [STRICT], types guide, [V213].

---

# Pydantic Advanced — 12) Aliases, validation aliases, serialization aliases, paths, choices, and generators

## 12.0 Three field-level alias concepts

```python
class User(BaseModel):
    first_name: str = Field(
        alias='firstName',
        validation_alias='first_name_input',
        serialization_alias='firstName',
    )
```

The fields serve different roles:

- `alias` can influence validation and serialization depending on config/runtime flags;
- `validation_alias` is specifically for accepted input keys;
- `serialization_alias` is specifically for emitted keys.

Use separate validation/serialization aliases when ingest and output contracts intentionally differ.

## 12.1 `AliasPath`

Extract a field from nested input without pre-transforming the entire payload:

```python
from pydantic import AliasPath

class User(BaseModel):
    name: str = Field(validation_alias=AliasPath('user', 'profile', 'name'))
```

Input:

```python
{'user': {'profile': {'name': 'Ada'}}}
```

## 12.2 `AliasChoices`

Support migration/fallback names:

```python
from pydantic import AliasChoices

class ConfigPayload(BaseModel):
    token: str = Field(
        validation_alias=AliasChoices('api_token', 'token', 'legacy_token')
    )
```

The first present choice wins according to Pydantic's lookup semantics. This is especially useful for config/environment migrations.

## 12.3 Alias generators

```python
from pydantic.alias_generators import to_camel

class APIModel(BaseModel):
    model_config = ConfigDict(alias_generator=to_camel)
    user_id: int
```

Input/output alias behavior still depends on validation/serialization configuration.

## 12.4 Separate generators

```python
from pydantic import AliasGenerator
from pydantic.alias_generators import to_camel, to_pascal

class M(BaseModel):
    model_config = ConfigDict(
        alias_generator=AliasGenerator(
            validation_alias=to_camel,
            serialization_alias=to_pascal,
        ),
        serialize_by_alias=True,
    )
    first_name: str
```

## 12.5 Alias precedence

Explicit field aliases and generated aliases have precedence rules influenced by `alias_priority`. When a field must preserve a legacy wire name despite a global generator, set it explicitly and test schema/output.

## 12.6 Runtime validation controls

Pydantic 2.11+ exposes granular alias input controls:

```python
ConfigDict(
    validate_by_alias=True,
    validate_by_name=False,
)
```

Per-call behavior can also be controlled on validation methods.

You cannot create a validation configuration that accepts neither aliases nor field names; at least one path must remain valid.

## 12.7 Serialization aliases

Serialization by alias is disabled by default in V2, even though alias validation commonly defaults on. The Pydantic docs call this inconsistency out and anticipate a V3 default change.

Public-contract rule:

```python
class ExternalModel(BaseModel):
    model_config = ConfigDict(serialize_by_alias=True)
```

Set the desired behavior explicitly rather than relying on version defaults.

## 12.8 Error locations

`loc_by_alias=True` controls whether validation errors point to the actual alias/key seen in input rather than the Python field name. For external APIs, alias-based locations can be more useful to callers.

## 12.9 Alias architecture patterns

**Transport DTO pattern**

```text
external camelCase
  -> validation aliases/generator
  -> Python snake_case
  -> domain logic
  -> serialization aliases/generator
  -> external camelCase
```

**Migration pattern**

```text
AliasChoices(new_name, old_name)
  -> canonical Python field
  -> serialization emits new_name only
```

This allows backward-compatible input while moving output forward.

## 12.10 Anti-patterns

- `populate_by_name=True` in new 2.13 code when `validate_by_name`/`validate_by_alias` are clearer.
- Adding a global alias generator without snapshotting JSON Schema and serialized keys.
- Accepting old and new aliases forever with no migration plan.
- Using Python field names as public API fields accidentally because `serialize_by_alias` was omitted.

Sources: alias guide, [FIELDS], [CONFIG].

---

# Pydantic Advanced — 13) Field validators: before, after, plain, and wrap

## 13.0 Decorator form

```python
from pydantic import BaseModel, field_validator

class M(BaseModel):
    x: int

    @field_validator('x')
    @classmethod
    def validate_x(cls, value: int) -> int:
        if value <= 0:
            raise ValueError('x must be positive')
        return value
```

Default mode is `'after'`.

## 13.1 After validators

After validators run after Pydantic's type validation. They receive the normalized field value and are usually the safest default for domain checks.

```python
@field_validator('x', mode='after')
@classmethod
def x_is_even(cls, value: int) -> int:
    if value % 2:
        raise ValueError('must be even')
    return value
```

Advantages:

- type is already known;
- less raw-input branching;
- type checker understands the value better.

## 13.2 Before validators

Before validators receive raw input and can normalize unusual shapes:

```python
from typing import Any

@field_validator('tags', mode='before')
@classmethod
def ensure_list(cls, value: Any) -> Any:
    if isinstance(value, str):
        return [value]
    return value
```

Before validators should accept broad input (`Any`) because they execute before Pydantic enforces the annotation.

Be careful mutating input in place; union validation may pass the same input through alternative branches.

## 13.3 Plain validators

A plain validator effectively becomes the validation implementation for that point and terminates normal inner validation.

Use when deliberately replacing Pydantic's default validation, not merely adding a check.

## 13.4 Wrap validators

Wrap validators receive a handler for downstream validation and can run logic before/after or conditionally bypass/modify the normal path.

Conceptual:

```python
from pydantic import ValidatorFunctionWrapHandler

@field_validator('value', mode='wrap')
@classmethod
def wrap_value(cls, value, handler: ValidatorFunctionWrapHandler):
    try:
        return handler(value)
    except Exception:
        # translate/retry only if this is truly the desired contract
        raise
```

Wrap validators are powerful but add Python-call overhead and complexity. Pydantic's performance guide specifically recommends avoiding wrap validators in hot paths when performance matters.

## 13.5 Multi-field validators

```python
@field_validator('first_name', 'last_name')
@classmethod
def nonempty(cls, value: str) -> str:
    if not value.strip():
        raise ValueError('must not be blank')
    return value
```

`check_fields=False` is useful for validators declared on a base class that target fields added by subclasses.

## 13.6 Field ordering and `ValidationInfo.data`

A field validator can inspect already validated fields:

```python
from pydantic import ValidationInfo

@field_validator('password_repeat')
@classmethod
def passwords_match(cls, value: str, info: ValidationInfo) -> str:
    if value != info.data.get('password'):
        raise ValueError('passwords do not match')
    return value
```

Only fields validated **before** the current field are guaranteed in `info.data`. Field definition order therefore matters. If the invariant is truly whole-model, a model validator is often clearer.

## 13.7 Raising errors

Recommended:

```python
raise ValueError('domain-friendly message')
```

or use `PydanticCustomError` for a stable custom error code/context.

Do not use `assert` for essential validation in production because Python optimization (`-O`) can remove assertions.

## 13.8 Inheritance

Field validators participate in class inheritance. Pay attention to whether a subclass overrides a validator method or adds another validator; write tests for shared model base classes.

## 13.9 Agent rules

```text
simple declarative constraint -> Field / Annotated
field-specific typed invariant -> after validator
raw-input normalization -> before validator
replace normal validation -> plain validator
intercept downstream validation -> wrap validator
whole-model invariant -> model validator
```

Sources: [VALIDATORS], functional validators API, [PERF].

---

# Pydantic Advanced — 14) Model validators, `ValidationInfo`, context, ordering, and inheritance

## 14.0 After model validator

Modern after validators are instance methods and should return the validated instance:

```python
from typing_extensions import Self
from pydantic import BaseModel, model_validator

class Interval(BaseModel):
    start: int
    end: int

    @model_validator(mode='after')
    def ordered(self) -> Self:
        if self.end < self.start:
            raise ValueError('end must be >= start')
        return self
```

Classmethod after validators are deprecated in the current V2 line; do not generate them in new code.

## 14.1 Before model validator

```python
from typing import Any

@model_validator(mode='before')
@classmethod
def normalize_payload(cls, data: Any) -> Any:
    if isinstance(data, dict) and 'legacyName' in data:
        data = dict(data)
        data['name'] = data.pop('legacyName')
    return data
```

Before model validators see raw input, which may be a dict or an arbitrary object when attribute extraction is enabled.

Prefer aliases for simple key migration. Use a before model validator when the transformation is genuinely structural.

## 14.2 Wrap model validator

Wrap model validators can surround the entire model validation process. They are useful for logging, error translation, auditing and highly specialized fallback behavior, but they are the most invasive validator mode.

## 14.3 `ValidationInfo`

Depending on validator type, `ValidationInfo` provides:

- `context` — caller-supplied context;
- `config` — effective configuration;
- `mode` — `'python'` or `'json'`;
- `field_name` — current field where meaningful;
- `data` — already validated field data where meaningful.

Do not assume `data` is available in model validators the same way as field validators.

## 14.4 Context injection

```python
class TenantScoped(BaseModel):
    plan: str

    @field_validator('plan')
    @classmethod
    def allowed_plan(cls, v: str, info: ValidationInfo) -> str:
        allowed = set((info.context or {}).get('plans', []))
        if allowed and v not in allowed:
            raise ValueError('plan not allowed')
        return v

TenantScoped.model_validate(
    {'plan': 'pro'},
    context={'plans': ['basic', 'pro']},
)
```

This keeps request-specific policy out of globals.

## 14.5 Constructor context limitation

Calling `Model(...)` directly does not expose the same explicit `context=` argument as `model_validate`. If context is core to validation, use explicit validation entry points at that boundary rather than inventing global state. Advanced ContextVar workarounds exist but increase coupling.

## 14.6 Ordering

Annotated validator ordering and decorator validators follow defined rules. In broad terms, before/wrap metadata is processed in one direction and after validators in the opposite/return direction; decorator-based validators are translated into metadata and added to the field's schema.

If correctness depends on several validators running in a particular sequence, write a focused test rather than relying on a casual mental model.

## 14.7 Inheritance behavior

A model validator defined on a base class runs for subclass instances unless overridden. Overriding the same validator method in the subclass replaces the base implementation under normal Python method inheritance semantics.

## 14.8 Cross-field invariant design

Use model validators for invariants such as:

- `end >= start`;
- one-of/mutually exclusive fields;
- fields that jointly determine validity;
- consistency between IDs and metadata;
- invariant after all conversions/defaults.

Do **not** put external I/O (database/API calls) into validation unless you have explicitly accepted the latency, retry and determinism implications. Pydantic validators work best as deterministic data-contract functions.

Sources: [VALIDATORS], [CHANGELOG].

---

# Pydantic Advanced — 15) Functional validator metadata: `BeforeValidator`, `AfterValidator`, `WrapValidator`, `ValidateAs`, and related helpers

## 15.0 Reusable validators through `Annotated`

```python
from typing import Annotated
from pydantic import AfterValidator, TypeAdapter

EvenInt = Annotated[int, AfterValidator(lambda v: v if v % 2 == 0 else (_ for _ in ()).throw(ValueError('must be even')))]
```

In real code, use named functions rather than opaque lambda tricks:

```python
def check_even(v: int) -> int:
    if v % 2:
        raise ValueError('must be even')
    return v

EvenInt = Annotated[int, AfterValidator(check_even)]
```

This reusable type works in models, `TypeAdapter`, lists, unions and nested schemas.

## 15.1 Functional validator classes

High-value metadata forms include:

- `BeforeValidator(func)`
- `AfterValidator(func)`
- `PlainValidator(func)`
- `WrapValidator(func)`

Choose modes with the same semantics as decorator validators.

## 15.2 `InstanceOf`

Require a runtime instance of an arbitrary class without fully modeling it:

```python
from pydantic import InstanceOf

class Basket(BaseModel):
    items: list[InstanceOf[Plugin]]
```

This validates identity/type, not the internal state of each arbitrary object.

## 15.3 `SkipValidation`

```python
from pydantic import SkipValidation

class Envelope(BaseModel):
    trusted_blob: SkipValidation[dict[str, object]]
```

Use only when the value is intentionally trusted or validated elsewhere. Serialization may still emit warnings if the actual value violates the annotated expectation.

## 15.4 `ValidateAs`

`ValidateAs` lets a custom runtime type be validated through a Pydantic-supported intermediary schema and then converted:

```python
from pydantic import TypeAdapter, ValidateAs

class DomainId:
    def __init__(self, value: int):
        self.value = value

DomainIdType = Annotated[
    DomainId,
    ValidateAs(int, lambda v: DomainId(v)),
]

adapter = TypeAdapter(DomainIdType)
obj = adapter.validate_python('42')
```

This can be much simpler than writing a low-level CoreSchema hook.

## 15.5 `PydanticUseDefault`

Low-level validator flows can signal that the field default should be used instead of the provided value. This is specialized behavior; prefer explicit input normalization when the semantics can be expressed more clearly.

## 15.6 `OnErrorOmit`

For selected container/typed structures, Pydantic can omit invalid items rather than failing the entire structure. This is a deliberate lossy-validation policy and should be visible in the domain contract.

## 15.7 Functional validators vs custom type hooks

Decision rule:

```text
Need to add simple behavior to an existing type?
  -> Annotated functional validator

Need to validate through another supported type?
  -> ValidateAs

Need custom schema graph / high-performance specialized parsing?
  -> __get_pydantic_core_schema__ / GetPydanticSchema
```

Sources: functional validators API, types/custom types guide, [VALIDATORS].

---

# Pydantic Advanced — 16) Serialization fundamentals: `model_dump` and `model_dump_json`

## 16.0 Serialization has two principal modes

```python
python_data = model.model_dump(mode='python')
json_data = model.model_dump(mode='json')
json_text = model.model_dump_json()
```

- `mode='python'` may preserve Python-native types such as `datetime`, `Decimal`, tuples and custom values according to serializers.
- `mode='json'` returns Python values intended to be JSON serializable.
- `model_dump_json()` serializes directly to JSON text through pydantic-core.

## 16.1 Current `model_dump` option map

High-value options in 2.13.4 include:

```text
mode
include
exclude
context
by_alias
exclude_unset
exclude_defaults
exclude_none
exclude_computed_fields
round_trip
warnings
fallback
serialize_as_any
polymorphic_serialization
```

`model_dump_json()` additionally exposes JSON formatting options such as `indent` and `ensure_ascii` alongside the corresponding filtering/serialization controls.

## 16.2 Alias output

```python
model.model_dump(by_alias=True)
```

or configure:

```python
ConfigDict(serialize_by_alias=True)
```

For published contracts, prefer a model-level explicit policy and test runtime overrides only where needed.

## 16.3 Round-trip mode

`round_trip=True` is intended for cases where the dumped representation should preserve enough information for non-idempotent/special Pydantic types to validate back accurately.

Do not assume ordinary `model_dump(mode='json')` is a lossless persistence format for every type.

## 16.4 Fallback serialization

A `fallback` callable can handle unknown values during serialization:

```python
model.model_dump(
    mode='json',
    fallback=lambda value: repr(value),
)
```

Be cautious: a generic fallback can silently turn unsupported data into a misleading string contract. Prefer explicit serializers for stable APIs.

## 16.5 Warnings

Serialization warnings can be controlled. Suppressing warnings globally may hide mismatched runtime values—especially when validation was skipped.

## 16.6 `context`

Serialization context is propagated to serializer functions:

```python
model.model_dump(context={'role': 'admin'})
```

This supports context-dependent formatting/redaction. Avoid using context to make one schema unpredictably emit fundamentally different object shapes unless consumers understand that contract.

## 16.7 Direct serializer reuse

For non-model types, use `TypeAdapter.dump_python()` / `dump_json()` instead of wrapping data into a temporary model.

## 16.8 Serialization is not `dict(model)`

`dict(model)` and iteration expose model fields in Python but do not necessarily apply full Pydantic serialization behavior. Use `model_dump()` for the Pydantic serialization contract.

Sources: [SER], BaseModel API.

---

# Pydantic Advanced — 17) Field serializers, model serializers, functional serializers, and serialization context

## 17.0 Field serializer

```python
from datetime import datetime, timezone
from pydantic import field_serializer

class Event(BaseModel):
    at: datetime

    @field_serializer('at')
    def serialize_at(self, value: datetime) -> str:
        return value.astimezone(timezone.utc).isoformat()
```

## 17.1 Plain vs wrap field serializer

Plain serializer:

```python
@field_serializer('amount', mode='plain')
def ser_amount(self, value):
    return f'{value:.2f}'
```

Wrap serializer:

```python
@field_serializer('items', mode='wrap')
def ser_items(self, value, handler, info):
    serialized = handler(value)
    return serialized
```

Wrap mode allows calling the default/next serializer and modifying the result. It is powerful but more complex.

## 17.2 Supported serializer signatures

Field serializers can be instance methods or function-style callables, with optional serialization info and wrap handler depending on mode. Common forms conceptually include:

```text
(self, value, info)
(self, value, handler, info)
(value, info)
(value, handler, info)
```

Let the decorator/API validate the signature; invalid serializer signatures fail during model construction/use.

## 17.3 Model serializer

```python
from pydantic import model_serializer

class Point(BaseModel):
    x: int
    y: int

    @model_serializer(mode='plain')
    def serialize_model(self):
        return {'coordinates': [self.x, self.y]}
```

This changes the serialized representation of the whole model. Because it can make the external output differ dramatically from field structure, pair it with explicit return typing/JSON Schema tests where consumers depend on schema.

## 17.4 Wrap model serializer

Use when you want normal serialization plus an envelope/transformation:

```python
@model_serializer(mode='wrap')
def envelope(self, handler, info):
    data = handler(self)
    return {'data': data}
```

## 17.5 Functional serializers in `Annotated`

```python
from pydantic import PlainSerializer

LowerHex = Annotated[
    bytes,
    PlainSerializer(lambda b: b.hex(), return_type=str),
]
```

Use for reusable type-level serialization behavior.

## 17.6 Serialization info/context

Serializer info can expose:

- serialization mode;
- context;
- include/exclude state;
- field name where relevant;
- alias/exclusion flags.

Example redaction:

```python
@field_serializer('email')
def ser_email(self, value: str, info):
    if (info.context or {}).get('redact'):
        return '***'
    return value
```

## 17.7 Schema alignment

If a serializer changes the output type, provide a serializer `return_type` or otherwise customize the serialization JSON Schema so generated contracts reflect real output.

## 17.8 Avoid I/O in serializers

Serializers may run in logging, API responses, caching, tests and repeated dumps. Network/database calls inside them create hidden latency and failure channels. Keep serialization deterministic and local.

Sources: [SER], functional serializers API.

---

# Pydantic Advanced — 18) Include/exclude semantics, `exclude_if`, unset/default/none/computed handling

## 18.0 Runtime include/exclude

```python
model.model_dump(include={'id', 'name'})
model.model_dump(exclude={'secret'})
```

Nested structures can use mapping/set specifications for precise subtree selection.

## 18.1 Field-level exclusion

```python
class User(BaseModel):
    username: str
    password_hash: str = Field(exclude=True)
```

Field-level exclusion is appropriate for values that should never be part of normal serialization.

Security note: still design separate output DTOs for high-risk boundaries; do not rely on one exclusion flag as the only barrier preventing secrets from leaving a service.

## 18.2 `exclude_if`

```python
class Response(BaseModel):
    detail: str | None = Field(
        default=None,
        exclude_if=lambda value: value is None,
    )
```

The decision is value-dependent. Pydantic 2.13 supports this on computed fields as well.

## 18.3 `exclude_unset`

Use for patch/delta output:

```python
patch.model_dump(exclude_unset=True)
```

Fields explicitly set to a value equal to the default are still “set.”

## 18.4 `exclude_defaults`

Removes values equal to their defaults regardless of whether the caller explicitly supplied them. This is semantically different from `exclude_unset`.

## 18.5 `exclude_none`

Drops serialized values that are `None`. Do not use this when `null` vs omitted has different business/API meaning.

## 18.6 `exclude_computed_fields`

Useful for round-trip/persistence-style dumps where computed fields should not be re-fed as stored input.

For general round-trip serialization, Pydantic recommends considering `round_trip=True` instead of using this as a blanket approximation.

## 18.7 Precedence

Field-level and runtime include/exclude controls interact. For public DTOs, write tests for the exact shape rather than trusting an informal precedence memory.

## 18.8 Patch example

```python
class UserPatch(BaseModel):
    display_name: str | None = None
    bio: str | None = None

patch = UserPatch.model_validate({'bio': None})
assert patch.model_fields_set == {'bio'}
assert patch.model_dump(exclude_unset=True) == {'bio': None}
```

Using `exclude_none=True` would incorrectly erase the explicit “clear bio” instruction.

Sources: [SER], [FIELDS], [V213].

---

# Pydantic Advanced — 19) Subclass and polymorphic serialization, `SerializeAsAny`, and external-contract safety

## 19.0 Default V2 behavior is annotation-driven

```python
class User(BaseModel):
    name: str

class Admin(User):
    permissions: list[str]

class Envelope(BaseModel):
    user: User

x = Envelope(user=Admin(name='Ada', permissions=['all']))
```

By default, serialization through the `User`-annotated field follows the `User` schema. Subclass-only fields are not automatically exposed.

This is a security-relevant improvement over unrestricted duck typing: adding a sensitive field to a subclass does not automatically leak it through every base-typed serialization boundary.

## 19.1 `SerializeAsAny`

```python
from pydantic import SerializeAsAny

class Envelope(BaseModel):
    user: SerializeAsAny[User]
```

This asks serialization to use the runtime value's broader schema/shape. Use deliberately.

## 19.2 Runtime `serialize_as_any`

```python
model.model_dump(serialize_as_any=True)
```

This is broad runtime duck-typed serialization behavior. Because it can expose values not visible in the annotation schema, it is dangerous as a global “fix my output” switch on security-sensitive APIs.

## 19.3 `polymorphic_serialization` in 2.13

Pydantic 2.13 introduced `polymorphic_serialization` to address the use case of serializing model/dataclass subclass fields without all of the broader `serialize_as_any` semantics.

Use through model config or dump-time option depending on scope:

```python
class M(BaseModel):
    model_config = ConfigDict(polymorphic_serialization=True)
```

or:

```python
obj.model_dump(polymorphic_serialization=True)
```

Treat exact option types/semantics as version-pinned because this is new surface in 2.13.

## 19.4 Union serialization

For discriminated unions, 2.13 no longer falls back to trying every union branch if serialization through the discriminator-selected member fails. This is more deterministic and avoids surprising “another branch happened to serialize” behavior.

## 19.5 API design rule

Choose one of three explicit policies:

```text
Schema-closed output
  -> default annotation-driven serialization

Known model/dataclass polymorphism
  -> polymorphic_serialization

Full duck-typed runtime serialization
  -> SerializeAsAny / serialize_as_any
```

Do not reach for the third when the second is sufficient.

## 19.6 Security example

```python
class User(BaseModel):
    name: str

class InternalUser(User):
    password_hash: str

class PublicResponse(BaseModel):
    user: User
```

Default schema-driven output protects against accidental subclass-field exposure. Turning on broad duck typing could reintroduce the leak.

Sources: [SER], [V213], [CHANGELOG], [CONFIG].

---

# Pydantic Advanced — 20) Computed fields, private attributes, properties, and model lifecycle hooks

## 20.0 `computed_field`

```python
from pydantic import computed_field

class Rectangle(BaseModel):
    width: float
    height: float

    @computed_field
    @property
    def area(self) -> float:
        return self.width * self.height
```

Computed fields participate in serialization and serialization JSON Schema even though callers do not provide them as ordinary validated input fields.

## 20.1 Alias/title/description on computed fields

Computed fields support metadata for output contracts. Use explicit aliases when serialization field naming differs.

## 20.2 `exclude_if` for computed fields

New in 2.13, conditional exclusion can be attached to computed fields, allowing output of derived values only when a predicate is satisfied.

## 20.3 Cached properties

`@computed_field` can wrap `@cached_property` when the derived value is expensive and instance immutability/mutation behavior makes caching safe.

Remember: cached values can become stale if source fields change.

## 20.4 Private attributes

```python
from pydantic import PrivateAttr

class Document(BaseModel):
    text: str
    _tokens: list[str] | None = PrivateAttr(default=None)
```

Private attributes:

- are not ordinary model fields;
- are not part of normal validation/JSON Schema;
- are stored separately;
- can hold caches, handles and runtime-only metadata.

## 20.5 `model_post_init`

For post-validation initialization that needs the complete validated instance:

```python
class M(BaseModel):
    x: int
    _derived: int = PrivateAttr()

    def model_post_init(self, context) -> None:
        self._derived = self.x * 2
```

Use this for internal setup, not for altering public validated values in ways that should have been enforced through validators.

## 20.6 Custom `__init__`

Pydantic supports custom initialization patterns but custom `__init__` affects signature/model-build behavior and can complicate integrations. Prefer validators, defaults and post-init hooks unless an actual custom constructor contract is required.

## 20.7 Derived public value vs private cache

```text
Consumer should see / schema should describe it
  -> computed_field

Runtime implementation detail only
  -> PrivateAttr

Value is part of validated input/state
  -> normal field
```

Sources: fields/computed field API, [V213], [MODELS].


---

# Pydantic Advanced — 21) `TypeAdapter`: arbitrary-type validation, serialization, JSON Schema, and reuse

## 21.0 What `TypeAdapter` solves

`TypeAdapter` provides Pydantic validation, serialization and JSON Schema for a supported Python type **without requiring a `BaseModel` wrapper**.

```python
from pydantic import TypeAdapter

UserIds = TypeAdapter(list[int])
assert UserIds.validate_python(['1', 2, 3]) == [1, 2, 3]
```

Use it for:

- `list[T]`, `dict[K, V]`, tuples and sets;
- unions and discriminated unions;
- `TypedDict`;
- standard/Pydantic dataclasses;
- constrained/annotated aliases;
- literals/enums;
- custom types;
- validation of generic aliases.

## 21.1 Constructor

Conceptual stable signature:

```python
TypeAdapter(
    type,
    *,
    config: ConfigDict | None = None,
    _parent_depth: int = 2,
    module: str | None = None,
)
```

`config=` cannot generally override configuration for types that already own their own config, such as `BaseModel`, Pydantic dataclasses and `TypedDict` types with Pydantic config. Configure those types at their own definition boundary.

## 21.2 Validation methods

```python
adapter.validate_python(value)
adapter.validate_json(json_bytes_or_text)
adapter.validate_strings(string_mapping)
```

High-value runtime options mirror model validation where applicable:

```text
strict
extra
from_attributes
context
experimental_allow_partial
by_alias
by_name
```

Exact availability depends on the method and version; pin signatures when generating libraries.

## 21.3 Serialization methods

```python
adapter.dump_python(value, mode='python')
adapter.dump_python(value, mode='json')
adapter.dump_json(value)
```

This is the correct serializer for arbitrary validated types. Avoid creating synthetic wrapper models solely to get `.model_dump_json()`.

## 21.4 JSON Schema

```python
schema = adapter.json_schema()
```

For several adapters at once, use the multiple-schema helpers so shared definitions can be coordinated instead of independently emitted.

## 21.5 Rebuild

Forward references may require rebuilding:

```python
adapter.rebuild(force=False, raise_errors=True)
```

The API returns `None` if rebuilding was unnecessary, otherwise success/failure status.

## 21.6 Performance rule: instantiate once

Bad:

```python
def parse(values):
    return TypeAdapter(list[int]).validate_python(values)
```

Good:

```python
INT_LIST = TypeAdapter(list[int])

def parse(values):
    return INT_LIST.validate_python(values)
```

Each adapter construction builds validator and serializer machinery. The performance guide explicitly recommends reuse.

## 21.7 Type-checking caveat

For some generic/union forms, static type checkers may not infer the adapter's output type perfectly. Explicit type parameters can help:

```python
adapter = TypeAdapter[str | int](str | int)
```

## 21.8 Dataclass generic workaround

Parameterized generic dataclasses may not validate through ordinary class construction as expected. `TypeAdapter(GenericDataclass[int])` is the correct way to validate the specialized alias.

## 21.9 Adapter registry pattern

```python
# validation/adapters.py
from pydantic import TypeAdapter

STRINGS = TypeAdapter(list[str])
USER_ID_MAP = TypeAdapter(dict[str, int])
```

Centralize reused adapters for hot code paths and stable contract tests.

## 21.10 Agent checklist

```text
[ ] Use TypeAdapter when the real contract is not object-shaped.
[ ] Instantiate reusable adapters once.
[ ] Use validate_json for raw JSON.
[ ] Do not pass config to model-like types that own config.
[ ] Use TypeAdapter for generic dataclass aliases.
[ ] Snapshot json_schema for externally consumed adapter contracts.
```

Sources: [TYPEADAPTER], [PERF], [JSONSCHEMA].

---

# Pydantic Advanced — 22) `RootModel`

## 22.0 Purpose

`RootModel[T]` represents a Pydantic model whose semantic payload is one root value rather than named object fields.

```python
from pydantic import RootModel

UserIds = RootModel[list[int]]
ids = UserIds.model_validate(['1', 2, 3])
assert ids.root == [1, 2, 3]
```

Use when the model itself has identity/methods/config but the serialized wire shape should remain an array/scalar/mapping rather than `{'items': ...}`.

## 22.1 Named subclass

```python
class Tags(RootModel[list[str]]):
    def unique(self) -> set[str]:
        return set(self.root)
```

## 22.2 Validation and serialization

```python
tags = Tags.model_validate(['a', 'b'])
print(tags.root)
print(tags.model_dump())
print(tags.model_dump_json())
print(Tags.model_json_schema())
```

The dumped shape follows the root type, not an artificial `root` object property.

## 22.3 RootModel vs TypeAdapter

Choose `TypeAdapter[T]` when you only need validation/serialization/schema around a type.

Choose `RootModel[T]` when you need a **class identity** with methods, inheritance/configuration and model semantics while keeping a root-shaped contract.

## 22.4 V1 migration

V1 custom-root models often used a `__root__` field. V2 replaces that mechanism with `RootModel`.

## 22.5 2.13 patch relevance

Pydantic 2.13 includes multiple root-model fixes:

- shallow-copy handling changed/fixed in the 2.13 line;
- 2.13.4 specifically preserves `RootModel` core metadata;
- discriminated-union support includes root models with suitable `Literal` root types in newer 2.13 behavior.

This is a reason to target 2.13.4 rather than 2.13.0 exactly.

## 22.6 Arbitrary types caveat

`RootModel` does not behave exactly like a normal `BaseModel` with `arbitrary_types_allowed=True`; model custom type integration should use proper Pydantic custom-type hooks where needed.

## 22.7 API-boundary example

```python
class Coordinates(RootModel[tuple[float, float]]):
    pass

# Wire shape can remain [lat, lon] rather than {'coordinates': [...]}
```

Sources: RootModel API, [MIGRATION], [CHANGELOG].

---

# Pydantic Advanced — 23) Pydantic dataclasses

## 23.0 Decorator

```python
from pydantic.dataclasses import dataclass

@dataclass
class User:
    id: int
    name: str

u = User(id='1', name='Ada')
assert u.id == 1
```

Pydantic dataclasses preserve a dataclass-oriented programming model while applying Pydantic validation.

## 23.1 Configuration

```python
from pydantic import ConfigDict
from pydantic.dataclasses import dataclass

@dataclass(config=ConfigDict(validate_assignment=True))
class User:
    id: int
```

Pydantic dataclasses do not use the normal `model_config` class attribute in the same way as `BaseModel`.

## 23.2 Missing model methods

Pydantic dataclasses do not expose the full BaseModel method surface such as `model_validate`, `model_dump`, and `model_json_schema` directly.

Wrap the dataclass in `TypeAdapter`:

```python
from pydantic import TypeAdapter

UserAdapter = TypeAdapter(User)
obj = UserAdapter.validate_python({'id': '1', 'name': 'Ada'})
data = UserAdapter.dump_python(obj)
schema = UserAdapter.json_schema()
```

## 23.3 Standard dataclass inheritance

A Pydantic dataclass can inherit fields from standard dataclasses; Pydantic will validate inherited fields according to the final decorated class schema.

## 23.4 Extra behavior differs from BaseModel

Pydantic dataclass handling of extras is not identical to `BaseModel`:

- extra data is not included in serialization the same way;
- there is no `__pydantic_extra__` field customization equivalent for typed extras.

Do not assume every `ConfigDict` feature has identical runtime consequences across model-like types.

## 23.5 Generic dataclass caveat

Parameterized generic dataclasses are generic aliases, not new proper Pydantic model classes. Ordinary construction may not validate the type parameter:

```python
from typing import Generic, TypeVar

T = TypeVar('T')

@dataclass
class Box(Generic[T]):
    value: T

# Validate specialization through TypeAdapter:
BoxInt = TypeAdapter(Box[int])
validated = BoxInt.validate_python({'value': '1'})
```

## 23.6 Validation hooks

Pydantic dataclasses can use validators and post-init behavior. Keep lifecycle semantics explicit, especially when migrating from V1, because V2 changed dataclass validation/post-init behavior.

## 23.7 BaseModel vs dataclass decision

| Need | Prefer |
|---|---|
| Pydantic-first API DTO with model methods | `BaseModel` |
| Existing dataclass-centric domain code | Pydantic dataclass |
| Arbitrary dataclass validation without modifying class | `TypeAdapter` |
| Generic specialized dataclass validation | `TypeAdapter` |

Sources: dataclasses guide/API, [TYPEADAPTER], [MIGRATION].

---

# Pydantic Advanced — 24) `TypedDict`, standard-library dataclasses, `NamedTuple`, and model-like types

## 24.0 `TypedDict`

Pydantic can validate a `TypedDict` through `TypeAdapter`:

```python
from typing_extensions import TypedDict
from pydantic import TypeAdapter

class UserRecord(TypedDict):
    id: int
    name: str

UserRecordAdapter = TypeAdapter(UserRecord)
value = UserRecordAdapter.validate_python({'id': '1', 'name': 'Ada'})
assert value == {'id': 1, 'name': 'Ada'}
```

This returns a dict-shaped value, not a model instance.

## 24.1 Why `TypedDict` can be faster

The Pydantic performance guide recommends `TypedDict` instead of nested `BaseModel` when you only need structured mappings and performance matters. Their published illustrative benchmark shows a substantial advantage for simple nested structures.

Tradeoff:

```text
TypedDict
  + lower object overhead
  + dict output directly
  - no instance methods/private attrs/model lifecycle

BaseModel
  + rich object model
  + methods/config/model state
  - more Python object overhead
```

## 24.2 Configure `TypedDict`

Pydantic provides `with_config` for attaching Pydantic configuration to model-like types such as `TypedDict`/standard dataclasses where `model_config` is not appropriate.

Use this to enforce e.g. string normalization/extra behavior at the type definition rather than trying to override with a `TypeAdapter(config=...)` when the type owns configuration.

## 24.3 Standard dataclasses

A normal `@dataclass` can be validated by `TypeAdapter` without converting its definition to a Pydantic dataclass:

```python
from dataclasses import dataclass

@dataclass
class Point:
    x: int
    y: int

PointAdapter = TypeAdapter(Point)
point = PointAdapter.validate_python({'x': '1', 'y': 2})
```

## 24.4 Named tuples

Pydantic supports `typing.NamedTuple` validation. 2.13 includes behavior changes around inherited/non-field annotations so only actual named-tuple fields participate in its schema.

NamedTuple has positional semantics; prefer `BaseModel`/TypedDict for public JSON object contracts unless tuple semantics are genuinely part of the interface.

## 24.5 PEP 728 TypedDict extras

Newer V2 releases support newer TypedDict typing semantics such as extra-item policies where Python/typing support allows it. Pair these with Pydantic's JSON Schema and runtime `extra` behavior tests; typing-only constructs may map differently across Python versions.

## 24.6 Model-like contract decision table

| Contract shape | Best default |
|---|---|
| rich named object | `BaseModel` |
| dict with known keys, no object behavior | `TypedDict` + `TypeAdapter` |
| existing dataclass | `TypeAdapter` or Pydantic dataclass |
| tuple-shaped record | `NamedTuple` if positional semantics matter |
| one root list/scalar | `RootModel` or `TypeAdapter` |

Sources: [PERF], dataclasses guide, standard-library types, TypeAdapter API.

---

# Pydantic Advanced — 25) Generic models, type variables, specialization, and PEP 695 syntax

## 25.0 V2 generic model pattern

Pydantic V2 does not require V1's separate `GenericModel` base. Combine `BaseModel` with standard Python generics:

```python
from typing import Generic, TypeVar
from pydantic import BaseModel

T = TypeVar('T')

class Response(BaseModel, Generic[T]):
    data: T

IntResponse = Response[int]
r = IntResponse(data='1')
assert r.data == 1
```

## 25.1 PEP 695 syntax

On supported Python versions:

```python
class Response[T](BaseModel):
    data: T
```

Pydantic resolves and specializes the schema per parameterized model.

## 25.2 Specialized model classes

Unlike parameterized generic dataclasses, a parameterized generic Pydantic model is a proper Pydantic specialization with field type substitution and validation.

## 25.3 `isinstance` caveat

Avoid relying on `isinstance(x, GenericModel[int])` style checks as a general Python contract. If runtime specialization identity matters, define a named subclass:

```python
class IntResponse(Response[int]):
    pass
```

## 25.4 Generic inheritance

Generic parameters can be partially or fully fixed in subclasses. Pydantic must rebuild field/core schemas with substituted type variables; complex recursive generic inheritance is a version-sensitive stress point and deserves tests.

## 25.5 Generic schema names

Parameterized models receive generated names that can affect `$defs` keys and JSON Schema references. If generated JSON Schema is an external artifact, snapshot/test the relevant schema rather than assuming names across versions.

## 25.6 Custom generic types

Custom generic non-model types can inspect `source_type` and generic arguments in `__get_pydantic_core_schema__`. This is advanced; use ordinary generic Pydantic models or annotated aliases first.

## 25.7 Generic adapter pattern

```python
ResultList = TypeAdapter(list[Response[int]])
```

Reuse the adapter if used repeatedly.

## 25.8 V1 migration

Replace:

```python
from pydantic.generics import GenericModel

class Response(GenericModel, Generic[T]): ...
```

with standard V2 generic `BaseModel` inheritance.

Sources: [MODELS], generics section, [MIGRATION].

---

# Pydantic Advanced — 26) Unions: smart mode, left-to-right, discriminators, callable discriminators, and errors

## 26.0 Why unions are special

A normal field has one validation path. A union must determine **which member is the intended match**. This creates both performance and semantic ambiguity.

```python
class M(BaseModel):
    value: int | str
```

## 26.1 Smart union mode

Smart mode is the default for ordinary unions. Pydantic evaluates members and uses heuristics based on successful validation quality/exactness and, for structured types, valid-field counts.

Do not assume “first union member always wins” in smart mode.

## 26.2 Left-to-right mode

When deterministic first-success order is the desired API:

```python
class M(BaseModel):
    value: int | str = Field(union_mode='left_to_right')
```

Ordering becomes part of the contract. `int | str` and `str | int` may behave differently under this mode.

## 26.3 Discriminated unions

Preferred for structured variants:

```python
from typing import Literal

class Cat(BaseModel):
    kind: Literal['cat']
    meows: int

class Dog(BaseModel):
    kind: Literal['dog']
    barks: int

class PetEnvelope(BaseModel):
    pet: Cat | Dog = Field(discriminator='kind')
```

Benefits:

- faster: validate only the selected branch;
- clearer errors;
- deterministic branch selection;
- generated JSON Schema/OpenAPI includes discriminator information.

The performance guide explicitly recommends tagged/discriminated unions over broad unions when possible.

## 26.4 Multiple literal tags

A variant may support several literal values when the discriminator contract requires aliases/version tags.

## 26.5 Callable discriminator

Use when variants cannot share a simple common field:

```python
from pydantic import Discriminator, Tag

# Union members are Annotated with Tag(...),
# discriminator callable returns matching tag.
```

A callable discriminator must usually handle both raw dict-like input and model instances because it can be used during validation and serialization.

## 26.6 Nested discriminators

Complex variant trees can nest discriminated unions. Keep tag vocabularies orthogonal and test JSON Schema output.

## 26.7 Error explosion

Non-discriminated unions may report errors from multiple candidate branches. This can be valuable for diagnostics but noisy for consumers. A discriminator narrows errors to the intended variant.

## 26.8 2.13 serialization change

When a discriminator selects a union member for serialization and that branch fails, Pydantic 2.13 no longer blindly falls back to trying all other members. This avoids misleading alternative serialization and makes tagged contracts stricter.

## 26.9 Anti-patterns

- `dict | list | str | int | ...` as a lazy substitute for designing a protocol.
- left-to-right unions without documenting member order.
- discriminators whose callable has side effects.
- discriminator values that overlap ambiguously across variants.
- using union parsing to “guess” user intent when explicit tags are available.

Sources: unions guide, [PERF], [V213].

---

# Pydantic Advanced — 27) Forward annotations, recursive models, cyclic input, and namespace resolution

## 27.0 Forward annotations

```python
from __future__ import annotations
from pydantic import BaseModel

class Node(BaseModel):
    value: int
    child: Node | None = None
```

or quote the type:

```python
child: 'Node | None' = None
```

Pydantic resolves self-references during model construction where possible.

## 27.1 Mutually recursive models

```python
from __future__ import annotations

class A(BaseModel):
    b: B | None = None

class B(BaseModel):
    a: A | None = None

A.model_rebuild()
B.model_rebuild()
```

Depending on namespace availability and definition order, explicit rebuild may be required.

## 27.2 `model_rebuild`

Use when forward types were unavailable when the class was first built:

```python
Model.model_rebuild(
    force=False,
    raise_errors=True,
)
```

It replaces V1's `update_forward_refs()` workflow.

## 27.3 Recursive values vs recursive schemas

A recursive schema is supported. A cyclic **input object graph** can still be invalid for a tree-shaped model. Pydantic detects recursion loops and raises a validation error instead of recursing until Python crashes.

## 27.4 Namespace problems in dynamic code

Forward-reference resolution can fail when:

- models are created inside functions;
- aliases exist only in local scopes;
- classes are dynamically generated;
- imports are conditional;
- generic specialization moves evaluation context;
- `TypeAdapter` is built outside the defining module.

Prefer module-level type aliases and explicit namespaces for framework-generated models.

## 27.5 `TypeAdapter.rebuild`

Adapters have their own rebuild API because their namespace resolution differs subtly from `BaseModel`:

```python
adapter.rebuild(_types_namespace={'MyType': MyType})
```

## 27.6 Python-version sensitivity

Annotation resolution is one of the most Python-version-sensitive parts of Pydantic, especially around newer typing syntax. The 2.14 prerelease specifically continues model-build/annotation evaluation optimization and adds thread-safety fixes to rebuilding. Pin and test dynamic/generic/forward-heavy systems.

Sources: forward annotations guide, internals resolving annotations, TypeAdapter API, [CHANGELOG].

---

# Pydantic Advanced — 28) Dynamic models, `create_model`, `model_rebuild`, and runtime schema composition

## 28.0 `create_model`

```python
from pydantic import create_model

DynamicUser = create_model(
    'DynamicUser',
    id=(int, ...),
    name=(str, 'anonymous'),
)

u = DynamicUser.model_validate({'id': '1'})
```

Use when the set of fields is genuinely determined at runtime or generated from metadata.

## 28.1 Base-class composition

```python
class BaseContract(BaseModel):
    model_config = ConfigDict(extra='forbid')

Dynamic = create_model(
    'Dynamic',
    value=(int, ...),
    __base__=BaseContract,
)
```

This inherits configuration/validators/fields according to normal Pydantic model rules.

## 28.2 Dynamic optionalization pattern

A common pattern is deriving a “patch” model where every field becomes optional. Build it by inspecting `model_fields` and reconstructing field annotations/defaults. Preserve metadata carefully; aliases, validators and JSON Schema metadata can otherwise be lost.

## 28.3 `__module__` / `__qualname__`

Dynamic models used by pickling, code generation, docs or static analysis may need stable `__module__`/`__qualname__` metadata. Newer V2 releases expanded `create_model()` support for these identity fields.

## 28.4 Build cost

Dynamic model generation builds schemas/validators/serializers. Do not generate an equivalent model per request.

Bad:

```python
def handle(fields):
    Model = create_model('Request', **fields)
    return Model.model_validate(...)
```

Better:

```text
normalize schema specification
  -> cache generated model by contract/version key
  -> reuse compiled model
```

## 28.5 Model rebuild after mutation

Do not mutate `model_fields` as a casual way to change a live schema. If framework code intentionally mutates schema metadata, it generally must rebuild the model and understand internal cache/reference behavior.

Prefer `create_model()` to derive a new model class rather than mutating an existing model in place.

## 28.6 Security

If model definitions are generated from untrusted metadata, treat that metadata as code-like configuration:

- bound number of fields/depth;
- restrict allowed types/constraints;
- avoid arbitrary import paths;
- cache with bounded cardinality;
- reject pathological recursive schemas;
- do not execute arbitrary validator functions from user metadata.

Sources: dynamic model examples, `create_model` API, [MODELS], [CHANGELOG].

---

# Pydantic Advanced — 29) Custom types, `CoreSchema`, `__get_pydantic_core_schema__`, and annotated handlers

## 29.0 Escalation ladder

Before implementing a custom core schema, try:

1. standard supported type;
2. `Annotated[..., Field(...)]`;
3. `BeforeValidator` / `AfterValidator`;
4. `PlainSerializer` / `WrapSerializer`;
5. `ValidateAs`;
6. only then `__get_pydantic_core_schema__`.

## 29.1 Type-level core-schema hook

```python
from typing import Any
from pydantic import GetCoreSchemaHandler, TypeAdapter
from pydantic_core import CoreSchema, core_schema

class Username(str):
    @classmethod
    def __get_pydantic_core_schema__(
        cls,
        source_type: Any,
        handler: GetCoreSchemaHandler,
    ) -> CoreSchema:
        return core_schema.no_info_after_validator_function(
            cls,
            handler(str),
        )

adapter = TypeAdapter(Username)
value = adapter.validate_python('ada')
assert isinstance(value, Username)
```

This replaces V1's `__get_validators__` custom-type hook.

## 29.2 Handler discipline

The handler lets your custom type delegate schema generation to Pydantic for inner/source types. Avoid recursively calling the wrong handler/type in a way that creates infinite schema generation.

## 29.3 Annotation-level customization

Instead of modifying the third-party class, put a marker object in `Annotated` that implements the core-schema hook. This is ideal for integrating types you do not own.

```text
ThirdPartyType
  + Annotated metadata object
      -> custom Pydantic validation/serialization
```

## 29.4 `GetPydanticSchema`

For lightweight annotation-driven core-schema customization, `GetPydanticSchema` can reduce boilerplate compared with a full metadata marker class.

## 29.5 JSON Schema hook

A custom validation schema does not automatically imply the exact external JSON Schema you want. Implement:

```python
__get_pydantic_json_schema__(core_schema, handler)
```

or use `WithJsonSchema` in an annotated type.

V1's `__modify_schema__` is replaced by these V2 APIs.

## 29.6 Third-party object integration pattern

Typical contract:

```text
Python input:
  accept ThirdPartyType instance OR primitive representation

validated output:
  ThirdPartyType instance

JSON serialization:
  primitive representation

JSON Schema:
  primitive schema
```

Use a union/is-instance branch plus conversion and serialization schema rather than `arbitrary_types_allowed=True` if you need meaningful parsing and JSON contracts.

## 29.7 Direct pydantic-core usage

`SchemaValidator` and `SchemaSerializer` can be instantiated directly from CoreSchema for ultra-low-level libraries. This bypasses much of Pydantic's model/type metadata ecosystem. Use only if the lower-level API itself is your product boundary.

## 29.8 Stability warning

The custom core-schema API is public but lower level and more likely to evolve than ordinary annotations. Keep custom type integrations small, tested and version-pinned.

Sources: custom types guide, Pydantic Core API, [MIGRATION], [JSONSCHEMA].

---

# Pydantic Advanced — 30) Built-in and standard-library type validation

## 30.0 Scope

Pydantic supports a broad set of Python built-in and standard-library types. For each type, three questions matter:

1. what Python/JSON inputs are accepted in lax mode?
2. what changes in strict mode?
3. how does the value serialize in Python vs JSON mode?

Always consult the standard-library types guide/conversion table for exact edge cases.

## 30.1 Scalars

Common targets:

```text
bool
int
float
str
bytes
None
```

Boolean lax parsing accepts selected common integer/string/bytes forms. Do not assume arbitrary non-empty strings become `True`; the accepted vocabulary is explicit.

## 30.2 Decimal and complex

`Decimal` supports parsing from compatible numeric/string representations and serializes to a JSON-safe representation. Pydantic 2.13 added support for a three-tuple input representation for `Decimal` in relevant Python validation paths.

`complex` has dedicated parsing behavior; 2.13 standardized use of Python's `complex()` constructor for Python input conversion.

## 30.3 Dates and times

```text
datetime.datetime
datetime.date
datetime.time
datetime.timedelta
```

Pydantic parses standardized text/numeric encodings according to configuration. Temporal serialization can be configured with `ser_json_temporal` and validation numeric units with `val_temporal_unit` in newer V2 releases.

Strict JSON can still accept standardized JSON strings for dates/times because JSON has no native date scalar.

## 30.4 UUID

Pydantic validates UUID instances/strings/bytes depending on mode and can constrain versions with specialized UUID types/metadata. Use UUID parsing as format/type validation, not a claim about security or randomness quality.

## 30.5 Paths

```text
pathlib.Path
PurePath variants
FilePath
DirectoryPath
NewPath
```

Some Pydantic-specific path types can check filesystem state. These introduce I/O/state dependency into validation; do not use them for pure transport contracts if validation may occur on a machine where the path is not intended to exist.

## 30.6 Collections

```text
list[T]
tuple[...]
set[T]
frozenset[T]
dict[K, V]
deque[T]
Sequence[T]
Mapping[K, V]
```

Performance guidance: if you know the concrete input/output container, annotate the concrete type instead of broad `Sequence`/`Mapping`; Pydantic must do more work to support abstract container possibilities.

## 30.7 Enums and literals

Use `Enum` for named Python enum semantics and `Literal[...]` for fixed exact alternatives. `use_enum_values` changes stored validated values and has important default-validation implications.

## 30.8 Regex patterns

`re.Pattern` can be validated/compiled. `ConfigDict(regex_engine=...)` affects pattern execution for field constraints. Be cautious when switching to Python's regex engine with untrusted input because regex complexity can introduce denial-of-service risk.

## 30.9 Callable and type objects

Pydantic can validate callable/type/class objects in selected forms. Such fields are Python-runtime contracts and usually have no ordinary portable JSON representation.

## 30.10 `Any`

`Any` tells Pydantic to leave the value essentially unvalidated. The performance guide recommends it when validation is intentionally unnecessary.

Do not use `Any` merely because a schema is inconvenient; it eliminates a major purpose of the boundary.

Sources: standard-library types API, conversion table, [STRICT], [PERF], [V213].


---

# Pydantic Advanced — 31) Pydantic-specific types, secrets, encoded data, constraints, and `FailFast`

## 31.0 Why Pydantic-specific types exist

Pydantic ships types and metadata for contracts that are common but not represented by a sufficiently precise standard-library type. These types often bundle validation, Python representation, serialization and JSON Schema behavior.

Use them where their semantics match the domain; do not choose them merely because a name sounds close.

## 31.1 Secret types

```python
from pydantic import BaseModel, SecretStr, SecretBytes

class Credentials(BaseModel):
    username: str
    password: SecretStr
    token: SecretBytes
```

Secret values mask their representation:

```python
c = Credentials(username='ada', password='secret', token=b'abc')
print(c.password)                 # masked representation
actual = c.password.get_secret_value()
```

Important:

- `SecretStr` is a display/logging safety primitive, not encryption.
- the actual secret remains in process memory;
- serializers can intentionally reveal secrets if customized;
- avoid placing `get_secret_value()` results in error messages/logs.

The 2.14 prerelease adds constant-time equality comparison changes for secret types; stable 2.13 should not be documented as if it already had that new behavior.

## 31.2 `Json[T]`

`Json[T]` accepts a raw JSON string/bytes and then validates the decoded value as `T`.

```python
from pydantic import Json

class Payload(BaseModel):
    data: Json[list[int]]

p = Payload(data='[1, "2", 3]')
assert p.data == [1, 2, 3]
```

This is useful when a JSON document itself is nested inside another transport field.

## 31.3 Encoded bytes/strings

Pydantic includes encoders/types for base64 and encoded byte/string forms. Use these rather than ad hoc validators when the desired wire representation matches the built-in type.

Model config also controls byte JSON encoding (`ser_json_bytes`, `val_json_bytes`) for ordinary bytes fields.

## 31.4 `ImportString`

`ImportString` can resolve an import path into a Python object. It is useful in trusted configuration:

```python
from collections.abc import Callable
from pydantic import ImportString

class PluginConfig(BaseModel):
    handler: ImportString[Callable]
```

Security warning: importing an arbitrary user-controlled path can execute module import side effects. Treat it as code-execution-adjacent configuration and restrict sources.

## 31.5 `ByteSize`

`ByteSize` parses human-readable storage sizes and provides conversion helpers. Good for settings/resource limits:

```python
from pydantic import ByteSize

class Limits(BaseModel):
    max_upload: ByteSize
```

Prefer to normalize once at configuration ingress and use integer bytes internally.

## 31.6 `FiniteFloat` and strict scalar types

Pydantic provides convenience types/metadata for strictness and finite numeric behavior. In new reusable libraries, `Annotated` often composes more cleanly than a proliferation of named constrained subclasses.

## 31.7 `FailFast`

```python
from typing import Annotated
from pydantic import FailFast

class Batch(BaseModel):
    items: Annotated[list[int], FailFast()]
```

Normally, collection validation tries to report multiple item errors. `FailFast()` stops at the first failure.

Use when:

- validation speed matters more than complete error reporting;
- inputs are very large;
- callers only need pass/fail plus one example;
- validation is part of a hot ingestion/filtering path.

Avoid for interactive forms/configuration where users benefit from seeing all errors at once.

## 31.8 `OnErrorOmit`

Allows selected invalid values to be dropped rather than failing the enclosing collection. This is intentionally lossy validation. Document it visibly because consumers may otherwise assume every input element survived.

## 31.9 `MISSING`

Pydantic's experimental `MISSING` sentinel can represent absence distinctly from `None` and can interact with schema/serialization. Treat it as experimental and version-pinned; for ordinary patch models, `model_fields_set` remains the stable first tool.

## 31.10 Agent rules

- Secret types improve representation safety but do not encrypt values.
- Restrict `ImportString` to trusted configuration.
- Use `FailFast` only when partial diagnostics are acceptable.
- Prefer built-in encoded/size types when their semantics exactly match the protocol.
- Keep experimental sentinels out of public stable libraries unless deliberately accepted.

Sources: Pydantic types API, [PERF], experimental docs, [CONFIG].

---

# Pydantic Advanced — 32) Network, URL, DSN, email, IP, UUID, path, and filesystem-oriented types

## 32.0 URL model

Pydantic provides URL types implemented as structured URL objects rather than plain strings. V2 URL/DSN types no longer simply inherit from `str`, which is a V1 migration concern.

Common types include:

```text
AnyUrl
AnyHttpUrl
HttpUrl
WebsocketUrl
AnyWebsocketUrl
FileUrl
FtpUrl
```

Use `str(url)` when a library truly requires a string representation.

## 32.1 URL constraints

URL types/metadata can constrain:

- allowed schemes;
- host requirements;
- default host/port/path;
- maximum length;
- empty-path preservation/normalization behavior.

`ConfigDict(url_preserve_empty_path=True)` can preserve empty URL paths rather than normalizing them according to Pydantic defaults.

## 32.2 DSN types

Common DSN classes include:

```text
PostgresDsn
CockroachDsn
AmqpDsn
RedisDsn
MongoDsn
KafkaDsn
NatsDsn
MySQLDsn
MariaDBDsn
ClickHouseDsn
SnowflakeDsn
```

Use a DSN type to validate structural connection-string semantics, not to verify that the remote database actually exists or credentials work.

## 32.3 Multi-host URLs

Selected DSNs use a multi-host URL representation. Serialization/normalization details can be version-sensitive; avoid string-snapshot assumptions without tests.

## 32.4 Email types

Install the optional dependency:

```bash
pip install 'pydantic[email]==2.13.4'
```

Then:

```python
from pydantic import EmailStr, NameEmail

class Contact(BaseModel):
    email: EmailStr
    display: NameEmail | None = None
```

`EmailStr` validates address syntax according to the `email-validator` integration. It does not prove mailbox ownership or deliverability.

## 32.5 IP types

Use standard library `ipaddress` classes (`IPv4Address`, `IPv6Address`, network/interface types) with Pydantic validation. For constrained IP-only fields, specialized Pydantic helpers may improve schema clarity.

## 32.6 UUIDs

Use `UUID` or version-specific UUID types where protocol requires a particular version. Do not describe UUID validation as a security guarantee.

## 32.7 Filesystem types

Pydantic-specific path types can assert properties such as:

```text
existing file
existing directory
new/non-existing path expectation
socket path
```

These validations touch filesystem state and are therefore environment-dependent.

Bad API DTO:

```python
class UploadRequest(BaseModel):
    path: FilePath  # client path is on client's machine, not server's
```

Good internal job configuration when the server itself owns the filesystem path.

## 32.8 Normalization vs original text

URL/email/path parsing may normalize representations. If the literal original user text must be retained for audit/legal/display reasons, store it separately from the normalized validated value.

## 32.9 Credentials inside URLs

Connection URLs may embed usernames/passwords. Secret handling policy should avoid rendering full DSNs in logs/errors. Consider storing credentials separately or redacting before logging.

Sources: network types API, standard library types, [MIGRATION], [CONFIG].

---

# Pydantic Advanced — 33) JSON parsing, `jiter`, string caching, and partial validation

## 33.0 JSON-native path

Pydantic uses the Rust JSON parser `jiter` through pydantic-core for JSON validation. Prefer:

```python
Model.model_validate_json(raw)
TypeAdapter(T).validate_json(raw)
```

over:

```python
Model.model_validate(json.loads(raw))
```

in the general case. [PERF]

## 33.1 Why direct JSON validation helps

```text
model_validate(json.loads(raw))
  JSON bytes -> Python JSON decoder -> Python dict/list/scalars -> pydantic-core

model_validate_json(raw)
  JSON bytes -> pydantic-core JSON parser + validator pipeline
```

The second can reduce Python allocations and duplicate traversal.

## 33.2 Exception to the general performance rule

The official performance guide notes that models with some before/wrap validators can in selected cases perform better after Python-side JSON parsing. Benchmark your hot model instead of turning a rule of thumb into dogma.

## 33.3 JSON inf/nan behavior

Pydantic/Jiter can support JSON parsing/serialization behavior for non-finite floats according to relevant options. `ConfigDict(ser_json_inf_nan=...)` controls emitted representation. Public APIs should choose standards compatibility deliberately.

## 33.4 String caching

Repeated JSON object keys/short strings are common. Pydantic can cache strings to reduce repeated allocations.

Configuration:

```python
ConfigDict(cache_strings='all')
ConfigDict(cache_strings='keys')
ConfigDict(cache_strings='none')
```

Use cases:

- high-volume JSON messages with repeated field names;
- event streams with stable schemas;
- memory/CPU tuning after profiling.

Caching trades memory for allocation/hash work. Benchmark before changing defaults globally.

## 33.5 Partial validation

Experimental partial validation allows validation of incomplete/streaming data through `TypeAdapter`:

```python
adapter.validate_json(
    partial_json,
    experimental_allow_partial=True,
)
```

or selected modes such as trailing-string handling where supported.

Use case:

```text
LLM / streaming JSON tokens
  -> partial parser/validator
  -> display/use valid prefix while more data arrives
```

## 33.6 Critical partial-validation limitation

Partial validation is not “ordinary validation but stop at EOF.” Errors associated with the incomplete tail may need special handling/omission. Do not use it for final acceptance of security-sensitive or persisted data. Always run full validation when the document is complete.

## 33.7 `model_validate_json` patch fixes in 2.13

2.13.1 and 2.13.2 corrected missing `ValidationInfo.data` and `ValidationInfo.field_name` on JSON validation paths. This illustrates why validation-context-heavy code should pin patch versions and test both Python and JSON entry points.

## 33.8 JSON in queues/files

For JSONL/event processing:

```python
EVENT = TypeAdapter(Event)

for line in file:
    event = EVENT.validate_json(line)
```

Reuse the adapter/model class and validate raw bytes/lines directly.

## 33.9 Security

Pydantic validates structure/types; it does not impose a global payload-size limit. Apply byte/depth/message limits at the network/file/queue layer before allowing an attacker to force arbitrarily expensive parse/schema work.

Sources: [JSON], [PERF], experimental docs, [CHANGELOG].

---

# Pydantic Advanced — 34) JSON Schema fundamentals and validation-vs-serialization schemas

## 34.0 BaseModel schema

```python
schema = User.model_json_schema()
```

Pydantic V2 targets JSON Schema Draft 2020-12 by default, with selected OpenAPI-oriented extensions/compatibility behavior.

## 34.1 TypeAdapter schema

```python
schema = TypeAdapter(list[User]).json_schema()
```

## 34.2 Two schema modes

Validation schema:

```python
User.model_json_schema(mode='validation')
```

Serialization schema:

```python
User.model_json_schema(mode='serialization')
```

These can differ when:

- validators accept broader input than output type;
- serializers change output type;
- aliases differ;
- computed fields appear only in serialization;
- defaults/requiredness differ under schema config;
- special types have asymmetric input/output representation.

For API request bodies use validation mode; for response/serialized payload contracts use serialization mode when the framework supports/needs that distinction.

## 34.3 `$defs` and references

Nested models/type aliases are emitted into `$defs` and referenced via `$ref` to avoid duplication and represent recursion.

Do not flatten JSON Schema manually unless the consumer requires it; recursive schemas cannot always be naively inlined.

## 34.4 Field metadata

`Field` information such as:

```text
title
description
examples
deprecated
constraints
aliases where relevant
```

flows into generated schema.

## 34.5 `json_schema_extra`

Model-wide:

```python
class M(BaseModel):
    model_config = ConfigDict(
        json_schema_extra={'x-service': 'billing'}
    )
```

Field-level:

```python
id: int = Field(json_schema_extra={'x-id-kind': 'billing'})
```

Use vendor extensions/extra schema metadata for downstream tooling only if the consumers understand them.

## 34.6 Union representation

Pydantic exposes controls such as `union_format` in newer V2 releases for alternative JSON Schema representations when possible. Discriminated unions produce discriminator-aware schemas suitable for OpenAPI.

## 34.7 Optional fields

V2 JSON Schema represents nullable values explicitly when `None` is part of the type. Requiredness still follows default presence, not `Optional` alone.

## 34.8 Decimal

V2's schema/serialization behavior for `Decimal` changed from V1 and commonly represents it as a string for JSON safety/precision. Generated clients must follow the emitted schema rather than assuming JSON number.

## 34.9 Schema as contract test

For public libraries/APIs:

```python
schema = Model.model_json_schema(mode='serialization')
# Normalize intentionally unstable fields if necessary,
# then snapshot or compare semantic fragments.
```

Prefer semantic assertions for critical properties over giant fragile text snapshots alone.

Sources: [JSONSCHEMA], [MIGRATION].

---

# Pydantic Advanced — 35) Advanced JSON Schema customization and `GenerateJsonSchema`

## 35.0 Field/model metadata first

Before subclassing the schema generator, use:

- `Field(title=..., description=..., examples=..., json_schema_extra=...)`;
- `ConfigDict(json_schema_extra=...)`;
- `WithJsonSchema(...)` on an `Annotated` type;
- custom type `__get_pydantic_json_schema__`.

These local mechanisms minimize global schema divergence.

## 35.1 `WithJsonSchema`

```python
from typing import Annotated
from pydantic import WithJsonSchema

HexColor = Annotated[
    str,
    WithJsonSchema({'type': 'string', 'pattern': '^#[0-9A-Fa-f]{6}$'}),
]
```

Use when validation is defined separately but the external JSON Schema needs a targeted representation.

## 35.2 Custom type JSON Schema hook

```python
@classmethod
def __get_pydantic_json_schema__(cls, core_schema, handler):
    schema = handler(core_schema)
    schema = handler.resolve_ref_schema(schema)
    schema['format'] = 'my-format'
    return schema
```

The handler lets you preserve Pydantic's generated schema and modify it rather than reimplementing recursive generation.

## 35.3 `GenerateJsonSchema`

For application/library-wide JSON Schema policy, subclass `GenerateJsonSchema` and pass it through schema-generation APIs.

This is one of Pydantic V2's major improvements over V1: schema generation is deliberately decomposed into overridable methods instead of requiring users to copy a monolithic recursive function.

## 35.4 Reference templates

Schema generation allows customizing reference templates/definition handling. If another ecosystem requires OpenAPI-style component references or organization-specific `$id` conventions, implement the change at the generator boundary.

## 35.5 `json_schema_serialization_defaults_required`

This config can mark fields with defaults as required in **serialization** schema because serialized model instances will normally include them even though input callers do not need to supply them.

Useful when response schema should describe actual output presence rather than input construction requirements.

## 35.6 `json_schema_mode_override`

Frameworks sometimes request one schema mode but an integration needs one globally consistent representation. `json_schema_mode_override` can force validation or serialization mode.

Use carefully: hiding the real input/output distinction can misdescribe one side of the contract.

## 35.7 Determinism

2.13 includes work to make defaults from sets deterministic in JSON Schema. Even with that, schema ordering/text formatting is not necessarily a semantic API. Normalize before snapshotting if non-semantic ordering causes churn.

## 35.8 Global customization risk

A custom `GenerateJsonSchema` affects every nested type. Maintain a regression suite covering:

- recursion and refs;
- discriminated unions;
- custom serializers;
- `RootModel`;
- dataclasses/TypedDicts;
- validation vs serialization mode;
- generics.

Sources: [JSONSCHEMA], JSON schema API, [MIGRATION], [V213].

---

# Pydantic Advanced — 36) Errors: `ValidationError`, custom errors, locations, causes, and usage errors

## 36.0 ValidationError structure

```python
from pydantic import ValidationError

try:
    Model.model_validate(payload)
except ValidationError as exc:
    details = exc.errors()
```

Each error detail carries structured information such as:

```text
type
loc
msg
input
ctx (where applicable)
url (documentation link where applicable)
```

Use `errors()` or `json()` for machine processing.

## 36.1 Error locations

Nested location example:

```text
('orders', 3, 'price')
```

This can be converted to dot/bracket notation for UI/API responses. Keep the structured tuple internally.

`loc_by_alias=True` can report source aliases in locations, which is often preferable for external callers.

## 36.2 Do not parse human error strings

Bad:

```python
if 'valid integer' in str(exc): ...
```

Good:

```python
for err in exc.errors():
    if err['type'] == 'int_parsing': ...
```

Error codes are the machine-facing discriminator.

## 36.3 Custom errors

`PydanticCustomError` lets a validator emit stable custom error type/message/context:

```python
from pydantic_core import PydanticCustomError

raise PydanticCustomError(
    'not_even',
    'value must be even; got {value}',
    {'value': v},
)
```

Use stable organization-specific codes when consumers translate errors.

## 36.4 `ValueError` vs `TypeError`

Validators should generally raise `ValueError` (or supported custom errors) for invalid data.

A `TypeError` raised inside validators is no longer automatically wrapped the same way it was in V1. It often indicates programmer misuse rather than invalid user input.

## 36.5 `PydanticUserError`

Pydantic raises usage errors when the **schema/model definition itself** is invalid—for example invalid decorator signatures/configuration—not because runtime input failed validation.

Pydantic 2.13 changed `PydanticUserError` to derive from `RuntimeError` rather than `TypeError`. Do not catch broad `TypeError` as a proxy for Pydantic configuration errors.

## 36.6 Error causes

`ConfigDict(validation_error_cause=True)` can expose underlying Python exception groups/causes for debugging validator exceptions. This is useful in development but can reveal implementation details in production diagnostics.

## 36.7 Hide raw input

```python
ConfigDict(hide_input_in_errors=True)
```

Use for secrets/PII-heavy validation boundaries. `ValidationError` can otherwise include input values in text/details.

## 36.8 Error translation layer

Recommended architecture:

```text
Pydantic ValidationError
  -> application error translator
      -> stable domain/API error code + safe field path + message
          -> logs/metrics preserve internal details under access controls
```

Do not expose raw internal validation traces blindly over a public API.

## 36.9 Batch validation

By default Pydantic accumulates useful multiple errors. `FailFast` can stop collection early for selected containers when speed matters.

Sources: errors API, error handling docs, [CONFIG], [V213].

---

# Pydantic Advanced — 37) `@validate_call`: validation of ordinary function calls

## 37.0 Basic usage

```python
from pydantic import validate_call

@validate_call
def repeat(text: str, count: int) -> str:
    return text * count

assert repeat('a', '3') == 'aaa'
```

Arguments are validated/coerced according to annotations before the function runs.

## 37.1 Complex signatures

`@validate_call` supports many ordinary Python signature features:

- positional-only parameters;
- keyword-only parameters;
- defaults;
- `*args` / `**kwargs` where type annotations allow;
- aliases in supported configurations;
- methods and callable patterns.

This is different from schema systems like MCP tools that may forbid variadic signatures; here the contract is Python-call validation.

## 37.2 Strict configuration

```python
@validate_call(config=ConfigDict(strict=True))
def f(x: int): ...
```

## 37.3 Return validation

```python
@validate_call(validate_return=True)
def f(x: int) -> int:
    return str(x)
```

Return validation can detect implementation contract drift, at additional runtime cost.

## 37.4 Raw callable access

The decorated object preserves access to the underlying callable for selected advanced/testing use. Use carefully: bypassing the wrapper bypasses validation.

## 37.5 Async functions

```python
@validate_call
async def fetch(limit: int) -> list[str]:
    ...
```

Argument validation happens around the async call semantics as supported by the decorator.

## 37.6 Performance

Decorating a hot, tiny function means validation overhead may dominate useful work. `@validate_call` is best at **boundary functions** where runtime input assurance is valuable.

Good:

```text
CLI command handler
plugin callback
public library entry point
job/task boundary
configuration-driven callable
```

Less useful:

```text
private arithmetic helper called millions of times
```

## 37.7 API stability

Do not casually use `@validate_call` to change a library's accepted argument coercions without treating that as an API change. Callers may begin succeeding on inputs that Python would previously reject or receiving `ValidationError` instead of ordinary exceptions.

Sources: validate-call guide/API, [STRICT].

---

# Pydantic Advanced — 38) `pydantic-settings` 2.15.0 fundamentals and source priority

## 38.0 Package boundary

Pydantic V2 moved settings management out of core Pydantic. Install:

```bash
pip install 'pydantic-settings==2.15.0'
```

Import:

```python
from pydantic_settings import BaseSettings, SettingsConfigDict
```

`pydantic-settings` 2.15.0 is independently versioned and requires Python >=3.10.

## 38.1 Basic settings class

```python
from pydantic import PostgresDsn, RedisDsn, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict

class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix='APP_',
        env_file='.env',
        extra='ignore',
    )

    database_url: PostgresDsn
    redis_url: RedisDsn | None = None
    api_key: SecretStr
    debug: bool = False
```

`Settings()` pulls values from configured settings sources and validates the resulting values through Pydantic.

## 38.2 Source concept

Settings combines multiple potential sources such as:

```text
explicit __init__ arguments
environment variables
.env files
file secrets / nested secrets
CLI arguments
custom sources
cloud secret manager sources
configuration files
```

Priority is explicit and customizable. Never describe settings behavior without knowing which sources are enabled and their order.

## 38.3 Environment prefix

```python
model_config = SettingsConfigDict(env_prefix='MY_APP_')
```

Field `database_url` maps to an environment name such as `MY_APP_DATABASE_URL` unless aliases override it.

## 38.4 Aliases in settings

```python
class Settings(BaseSettings):
    auth_key: str = Field(validation_alias='MY_AUTH_KEY')
```

`validation_alias`, `alias`, `AliasChoices` and other Pydantic alias behavior integrates with settings source lookup.

This enables migration across environment variable names without renaming internal Python attributes.

## 38.5 Complex environment values

Complex types are commonly parsed from environment variable JSON strings:

```bash
export APP_DOMAINS='["example.com", "api.example.com"]'
```

```python
domains: set[str] = set()
```

If environment syntax should be comma-separated rather than JSON, implement a custom settings source/parsing policy rather than adding ad hoc parsing to every field.

## 38.6 Nested environment variables

`env_nested_delimiter` can map names such as:

```text
APP_DATABASE__HOST
APP_DATABASE__PORT
```

to nested model fields.

Use a naming convention that does not collide with legitimate field names and document case-sensitivity behavior for your operating systems.

## 38.7 `.env`

```python
SettingsConfigDict(env_file='.env', env_file_encoding='utf-8')
```

Do not commit secrets-bearing `.env` files to source control. Treat `.env` as a developer/deployment convenience, not a secret vault.

## 38.8 Settings singleton/lifetime

Application pattern:

```python
from functools import lru_cache

@lru_cache
def get_settings() -> Settings:
    return Settings()
```

or construct once during application lifespan/startup and inject it.

Do not instantiate settings repeatedly in hot request paths unless in-place reload semantics are intentionally required.

Sources: [SETTINGS], pydantic-settings PyPI 2.15.0.

---

# Pydantic Advanced — 39) Advanced settings: nested env, dotenv, secrets, CLI, cloud secret managers, and custom sources

## 39.0 Source customization

`BaseSettings.settings_customise_sources(...)` lets a settings class reorder, remove or add sources.

Conceptual signature receives source objects such as:

```text
init_settings
env_settings
dotenv_settings
file_secret_settings
```

and returns them in desired priority order.

## 39.1 Priority design

A common production policy:

```text
explicit runtime overrides
  > CLI override (for command jobs)
  > environment variables
  > mounted secret files
  > dotenv / local config
  > model defaults
```

But secrets may deserve higher priority than env in some deployments. Define the rule explicitly and test collisions.

## 39.2 Custom source

Implement a `PydanticBaseSettingsSource` subclass when values come from:

- organization config service;
- Consul/etcd;
- encrypted file;
- application metadata endpoint;
- custom environment encoding.

Keep I/O and parsing in the source layer, not field validators.

## 39.3 CLI integration

Pydantic Settings includes integrated CLI parsing. It can:

- let CLI flags override settings fields;
- generate a CLI from models;
- support subcommands/positional args/aliases/shortcuts;
- tune how required args, `None`, booleans and help text are represented;
- integrate with existing parsers.

This is a powerful package surface in its own right. If building a substantial end-user CLI, compare dedicated CLI frameworks before making a settings model own all CLI UX.

## 39.4 File secrets

A secrets directory commonly maps one file per key, useful for Docker/Kubernetes-style mounted secrets.

Security posture:

- ensure directory permissions are restrictive;
- do not log loaded values;
- limit allowed path size/count;
- avoid following unexpected symlinks;
- pin pydantic-settings to a patched version.

## 39.5 Nested secrets security

A 2026 `pydantic-settings` security fix addressed a nested-secrets path/symlink escape issue in affected prior versions. Pinning **2.15.0** incorporates the patched line. Treat secret-path resolution as a security boundary, not mere file parsing.

## 39.6 Cloud secret managers

Current pydantic-settings extras include:

```text
aws-secrets-manager
azure-key-vault
gcp-secret-manager
```

Install only the provider(s) used:

```bash
pip install 'pydantic-settings[aws-secrets-manager]==2.15.0'
```

Provider integrations bring their own identity/credential/network/retry behavior. Keep secret-manager client lifecycle and failure policy explicit.

## 39.7 TOML/YAML/pyproject sources

Optional extras support configuration formats such as TOML and YAML. Config files are appropriate for non-secret structured defaults; combine them with higher-priority secret/env sources for credentials.

## 39.8 In-place reload

Pydantic Settings supports workflows for reloading settings. Changing process-wide configuration in place can create concurrency consistency problems: one request may observe old values while another observes new.

For high-integrity systems, prefer immutable settings snapshots and atomic dependency swapping/versioning rather than mutating a globally shared object field by field.

## 39.9 Async environments

Settings construction can involve sources that are fundamentally async (e.g. network secret stores), while `BaseSettings` construction is primarily synchronous. Follow the documented async source/integration pattern rather than blocking an event loop with network calls in field validators.

## 39.10 Test source precedence

```python
def test_env_beats_default(monkeypatch):
    monkeypatch.setenv('APP_DEBUG', 'true')
    s = Settings()
    assert s.debug is True
```

Also test conflicts across env/dotenv/secrets/CLI sources because source priority is part of the application's operational contract.

Sources: [SETTINGS], pydantic-settings 2.15.0 PyPI/changelog/security history.

---

# Pydantic Advanced — 40) Performance, build-time cost, memory, validation hot paths, and `FailFast`

## 40.0 Profile first

The official performance guidance starts with a warning: Pydantic often is not the real bottleneck. Measure before replacing readable contracts with specialized schemas.

Separate metrics:

```text
model / adapter build time
validation latency
serialization latency
allocation / RSS
JSON parse latency
error-heavy validation latency
import/startup time
```

## 40.1 Prefer direct JSON validation

General rule:

```python
Model.model_validate_json(raw)
```

over:

```python
Model.model_validate(json.loads(raw))
```

because direct JSON validation avoids a separate Python parsing/materialization pass.

Benchmark if model-level before/wrap validators make the two-stage route faster for your schema.

## 40.2 Reuse `TypeAdapter`

Each new adapter constructs validators and serializers.

```python
# module scope
ITEMS_ADAPTER = TypeAdapter(list[Item])
```

not inside every invocation.

## 40.3 Prefer concrete containers

If only lists are accepted:

```python
list[int]
```

is cheaper/more direct than:

```python
Sequence[int]
```

Likewise `dict` vs broad `Mapping` when concrete semantics suffice.

## 40.4 Use `Any` when validation is intentionally unnecessary

```python
class Envelope(BaseModel):
    metadata_blob: Any
```

This preserves the value without traversing/validating it. Only use when the lack of validation is deliberate.

## 40.5 Tagged unions

Discriminated/tagged union:

```python
Field(discriminator='kind')
```

lets Pydantic select one branch instead of trying multiple union schemas. It is both clearer and faster.

## 40.6 `TypedDict` vs nested models

For deep dict-shaped structures with no model behavior, `TypedDict` + `TypeAdapter` can materially reduce Python object overhead. Pydantic's performance guide shows a simple benchmark where it is significantly faster than nested `BaseModel` objects.

## 40.7 Avoid wrap validators when possible

Wrap validators cross the Python/Rust boundary and invoke handlers; they are inherently more expensive than pure core-schema constraints/normal validators. Use only for semantics that require interception.

## 40.8 `FailFast`

For large sequences:

```python
Annotated[list[Item], FailFast()]
```

stops on the first invalid item, improving failure-path latency at the cost of complete diagnostics.

## 40.9 Defer schema build

```python
ConfigDict(defer_build=True)
```

can improve import/startup for large libraries/apps with many models that may never be used.

Measure cold-start vs first-use latency.

## 40.10 Dynamic model caches

If `create_model()` is used, cache models by a normalized contract key. Unbounded schema/model generation can become a memory leak because generated classes/validators remain referenced in caches/registries.

## 40.11 String caching

Tune `cache_strings` only for validated JSON workloads where repeated strings are common and profiling shows benefit.

## 40.12 Avoid primitive subclasses as metadata bags

The performance guide recommends against primitive subclasses carrying extra Python attributes. Use a small model/dataclass value object when you need extra state.

## 40.13 Build-time matters in serverless/CLI

A FastAPI service may build thousands of Pydantic schemas during import/OpenAPI generation. Performance tuning is not only “requests/sec”; model construction can dominate:

- CLI startup;
- Lambda cold start;
- test collection;
- worker startup;
- codegen/schema generation.

The upcoming 2.14 line contains explicit model-build/core-schema optimizations, underscoring that this is a distinct performance dimension.

## 40.14 Optimization checklist

```text
[ ] Profile validation vs model-build vs serialization separately.
[ ] Use model_validate_json for raw JSON by default.
[ ] Reuse TypeAdapter instances.
[ ] Prefer concrete containers.
[ ] Use discriminated unions.
[ ] Consider TypedDict for dict-only nested structures.
[ ] Avoid wrap validators on hot paths.
[ ] Use FailFast only when incomplete diagnostics are acceptable.
[ ] Consider defer_build for huge rarely-used model catalogs.
[ ] Cache dynamic models with bounded cardinality.
```

Sources: [PERF], [JSON], [TYPEADAPTER], [CHANGELOG].


---

# Pydantic Advanced — 41) Experimental APIs and stability boundaries

## 41.0 Experimental means version-gated

Pydantic exposes selected features under experimental modules/flags so users can test emerging capabilities before they receive normal stability guarantees.

Agent rule:

```text
If import path, parameter, sentinel or feature is explicitly experimental:
  - pin exact Pydantic version;
  - isolate it behind a small wrapper;
  - add behavior tests;
  - do not make it an unqualified public-library dependency contract.
```

## 41.1 Partial validation

Introduced experimentally in V2.10, partial validation allows a `TypeAdapter` to accept incomplete input, particularly useful for streaming LLM/JSON output.

```python
adapter.validate_json(
    partial_json,
    experimental_allow_partial=True,
)
```

This is not final-data validation. Validate the completed object again normally before persistence, authorization or side effects.

## 41.2 `MISSING` sentinel

The experimental `MISSING` sentinel expresses “not supplied” as a first-class value distinct from `None` and ordinary defaults. It can simplify some configuration/patch schemas but is still an experimental contract.

Stable alternatives:

```text
model_fields_set
exclude_unset
explicit application sentinel
separate patch DTO
```

## 41.3 Pipeline API

Pydantic has experimental pipeline-style APIs for composing validation/constraint logic. These can make complex `Annotated` pipelines more declarative but have received ongoing fixes across 2.13/2.14 prereleases.

For shared libraries, ordinary `Annotated` functional validators are the safer stable baseline unless the pipeline API offers material value.

## 41.4 Experimental surface isolation pattern

```python
# validation/experimental.py
from pydantic import TypeAdapter

_PARTIAL = TypeAdapter(MyType)

def validate_partial_json(raw: bytes):
    return _PARTIAL.validate_json(
        raw,
        experimental_allow_partial=True,
    )
```

If the API changes, only one adapter module needs migration.

## 41.5 Test both enabled and fallback paths

If an application can operate without an experimental feature, maintain a stable fallback. This is particularly useful for libraries supporting multiple Pydantic minor versions.

Sources: experimental features docs, [CHANGELOG].

---

# Pydantic Advanced — 42) Static typing, Mypy, Pyrefly, IDEs, Hypothesis, and code generation

## 42.0 Runtime validation complements static typing

Static type checker:

```text
checks source-level expectations before execution
```

Pydantic:

```text
checks/transforms runtime values entering the program
```

Neither replaces the other.

## 42.1 Mypy plugin

Enable the Pydantic plugin where appropriate:

```ini
[mypy]
plugins = pydantic.mypy
```

The plugin improves understanding of:

- model constructor signatures;
- required/optional fields;
- frozen models;
- aliases/configuration;
- untyped fields;
- dynamic model behavior in supported cases.

Additional strict plugin settings can prevent aliases/dynamic behavior that make static signatures unsound.

## 42.2 Why field declaration style matters

Static type checkers understand:

```python
x: int = Field(default=1, alias='external_x')
```

more directly than some metadata buried only inside `Annotated`, particularly for constructor synthesis. Keep defaults/factories/aliases in forms supported by your checker when constructor typing matters.

## 42.3 Pyrefly

Pydantic documents Pyrefly integration as a supported development-tool workflow. Type-checker behavior can differ from Mypy; if a project standardizes on Pyrefly, test generics, dynamic models and aliases against that checker instead of assuming plugin parity.

## 42.4 PyCharm / VS Code

Pydantic's typed model signatures and plugins/extensions can improve IDE completion and error checking. Generated/dynamic models naturally provide less static visibility.

## 42.5 Hypothesis

Property-based tests are particularly useful for validators and serializers:

```text
generate valid/invalid values
  -> validate
  -> dump
  -> revalidate where round-trip is expected
```

Focus on boundaries:

- numeric min/max;
- Unicode/string pattern cases;
- union discriminators;
- optional/missing/null;
- nested container lengths;
- date/time timezone edges.

## 42.6 datamodel-code-generator

The Pydantic docs integrate with `datamodel-code-generator`, which can generate Pydantic models from sources including:

```text
OpenAPI
JSON Schema
JSON/YAML/CSV sample data
Python dictionaries
GraphQL schemas
```

Example:

```bash
datamodel-codegen \
  --input person.json \
  --input-file-type jsonschema \
  --output models.py
```

Generated code should be reviewed and pinned to a generator/Pydantic compatibility line. Schema generation is code generation, not a substitute for architecture review.

## 42.7 Static contract anti-patterns

- `Any` everywhere to silence a type checker.
- dynamic `create_model()` for schemas that are actually static.
- aliases generated from runtime data, making constructor signatures unknowable.
- validators returning types inconsistent with annotations and relying on runtime behavior.
- custom serializers without output type/schema metadata.

Sources: Mypy integration docs, Pyrefly docs, Hypothesis integration, datamodel-code-generator integration.

---

# Pydantic Advanced — 43) Observability and validation instrumentation

## 43.0 What to observe

Pydantic boundary metrics can answer:

```text
Which models fail most?
Which fields/error codes fail?
How long does validation take?
How much input is being rejected?
Did a strictness migration increase failures?
Which serializer paths are slow?
```

## 43.1 Logfire integration

Pydantic Logfire can instrument Pydantic validation and record successful/failed validations. The Pydantic strict-mode docs specifically suggest observability as a way to safely roll out stricter validation.

Use instrumentation to understand behavior, not to log every sensitive value indiscriminately.

## 43.2 Validation telemetry pattern

At an application boundary:

```python
from time import perf_counter
from pydantic import ValidationError

start = perf_counter()
try:
    value = Model.model_validate(payload)
except ValidationError as exc:
    metrics.increment('validation.failure', tags={'model': 'Model'})
    # record safe error codes/locations, not raw secrets
    raise
finally:
    metrics.observe('validation.seconds', perf_counter() - start)
```

## 43.3 Error-cardinality control

Do not use full error message or raw input as a metric label. High-cardinality telemetry becomes expensive/useless.

Good labels:

```text
model_name
error_type
field_top_level
source
schema_version
```

Bad labels:

```text
raw email address
request ID as metric tag
entire validation message
secret token
```

## 43.4 Schema-build telemetry

Large FastAPI/agent/tooling applications should measure model/schema build time separately from runtime validation. `defer_build` may make startup faster while shifting latency into first request; only telemetry reveals the actual tradeoff.

## 43.5 Validation failures as producer-drift signal

For queues/APIs:

```text
new producer deployment
  -> spike in extra_forbidden / missing / literal_error
      -> likely schema drift
```

Validation error codes are excellent machine-readable compatibility signals.

## 43.6 Redaction

Combine:

- `SecretStr`/`SecretBytes`;
- `hide_input_in_errors=True` where appropriate;
- application-level structured redaction;
- access-controlled debug traces.

Pydantic's masking is one defense layer, not a comprehensive telemetry privacy system.

Sources: Pydantic Logfire integration, [STRICT], errors docs, [CONFIG].

---

# Pydantic Advanced — 44) Framework and persistence integration boundaries

## 44.0 FastAPI

FastAPI uses Pydantic for request validation, response serialization and OpenAPI/JSON Schema generation. Keep the ownership boundary clear:

```text
FastAPI
  owns HTTP routing/dependencies/request-response lifecycle

Pydantic
  owns data validation/serialization/schema behavior
```

A Pydantic configuration change such as aliases, strictness, `extra`, computed fields or polymorphic serialization can therefore be an HTTP API change when the model is used by FastAPI.

## 44.1 API request vs response models

Do not automatically reuse one model for database row, write request, read response and internal domain state.

Example split:

```python
class UserCreate(BaseModel):
    email: EmailStr
    password: SecretStr

class UserPublic(BaseModel):
    id: int
    email: EmailStr

class UserInternal(UserPublic):
    password_hash: str
```

This reduces reliance on exclusion flags and prevents accidental subclass serialization leaks.

## 44.2 ORM / SQLAlchemy-style objects

```python
class UserDTO(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    id: int
    name: str
```

Then:

```python
UserDTO.model_validate(orm_user)
```

Pydantic reads attributes; it does not manage sessions, lazy loading or transaction lifetime. Accessing attributes can trigger ORM lazy loads, so serialize outside/inside session boundaries deliberately.

## 44.3 Persistence models

`model_dump()` is not automatically a database persistence representation. Special types, computed fields, aliases and serializers may transform values.

Define persistence mapping explicitly:

```text
Pydantic/domain object
  -> persistence adapter
  -> database columns/document
```

## 44.4 Queue/event contracts

Pydantic is especially effective at queue ingress:

```python
EVENT = TypeAdapter(Event)

def consume(raw: bytes):
    event = EVENT.validate_json(raw)
    ...
```

Include schema/event version discriminators. Use `extra='forbid'` for strict producers or intentional forward-compatible extra policy for evolving event envelopes.

## 44.5 LLM structured output

`TypeAdapter`/JSON Schema can define structured output contracts for LLMs. Experimental partial validation can improve streaming UX, but final output must still undergo full validation.

Keep semantic checks (authorization/business legitimacy) after structural validation.

## 44.6 DataFrame/data science boundaries

Pydantic models are row/object contracts, not a high-throughput columnar dataframe representation. Avoid instantiating millions of models if a vectorized Arrow/Pandas/Polars validation path is more appropriate.

## 44.7 Plugin systems

Use `InstanceOf`, discriminated config models or `ImportString` for plugin boundaries depending on whether plugins are Python objects or config-described implementations. Restrict arbitrary import strings when configuration is untrusted.

Sources: Pydantic integrations/examples, [MODELS], [TYPEADAPTER], [MIGRATION].

---

# Pydantic Advanced — 45) Pydantic V1 compatibility and V1 → V2 migration

## 45.0 V1 compatibility namespace

Pydantic V2 includes a V1 compatibility namespace:

```python
from pydantic.v1 import BaseModel
```

Pydantic 2.13 updates this bundled namespace to match Pydantic V1.10.26, including Python 3.14 support in that compatibility implementation.

Use this for staged migrations, not as an excuse to indefinitely mix V1 and V2 idioms in new modules.

## 45.1 Method rename map

| V1 | V2 |
|---|---|
| `dict()` | `model_dump()` |
| `json()` | `model_dump_json()` |
| `parse_obj()` | `model_validate()` |
| `parse_raw()` | parse externally / `model_validate_json()` where applicable |
| `construct()` | `model_construct()` |
| `copy()` | `model_copy()` |
| `json_schema()` / `schema()` | `model_json_schema()` |
| `update_forward_refs()` | `model_rebuild()` |
| `__fields__` | `model_fields` |
| `__private_attributes__` remains conceptually private metadata | use documented V2 surfaces |

Deprecated compatibility methods may still exist, but do not generate them in new V2 code.

## 45.2 Validators

V1:

```python
@validator('x')
@root_validator
```

V2:

```python
@field_validator('x')
@model_validator(mode='after')
```

V2 validator signatures and exception behavior differ. Migrate semantics, not only decorator names.

## 45.3 `Config` → `ConfigDict`

V1:

```python
class Config:
    orm_mode = True
```

V2:

```python
model_config = ConfigDict(from_attributes=True)
```

Key rename examples:

```text
orm_mode -> from_attributes
allow_population_by_field_name -> validate_by_name / validate_by_alias strategy
allow_mutation -> inverse-ish frozen behavior
schema_extra -> json_schema_extra
```

## 45.4 Root models

V1 `__root__` custom root fields become `RootModel[T]`.

## 45.5 Settings

V1 `BaseSettings` moved to the separate `pydantic-settings` package in V2.

## 45.6 Extra types

Some V1 types moved to external packages such as `pydantic-extra-types`. Do not assume every old type remains in core Pydantic.

## 45.7 Generics

V1 `GenericModel` is no longer required. Use `BaseModel, Generic[T]` or PEP 695 syntax on supported Python versions.

## 45.8 Optional fields

Audit this carefully:

```python
x: str | None
```

In V2 this is nullable but **required** without a default. Add `= None` if omission is intended.

## 45.9 Serialization

V2 serialization changed substantially:

- new field/model serializers replace `json_encoders` as the preferred customization mechanism;
- subclass serialization is annotation-driven by default;
- JSON Schema can differ between validation and serialization modes.

## 45.10 Custom types

Replace:

```text
__get_validators__
__modify_schema__
```

with:

```text
__get_pydantic_core_schema__
__get_pydantic_json_schema__
```

or `Annotated` metadata.

## 45.11 `bump-pydantic`

Official migration tooling:

```bash
pip install bump-pydantic
bump-pydantic my_package
```

Treat it as an accelerator, not proof of semantic correctness. Run tests and audit validators/config/serialization manually.

## 45.12 Incremental migration plan

```text
1. upgrade V1 to >=1.10.17
2. change imports to pydantic.v1 namespace
3. allow Pydantic <3 / install V2
4. migrate module-by-module from pydantic.v1 -> pydantic
5. convert methods/config/validators/custom types
6. snapshot schemas/serialized output
7. remove V1 namespace
```

Sources: [MIGRATION], [CHANGELOG].

---

# Pydantic Advanced — 46) Stable release delta: Pydantic 2.12 → 2.13.4

## 46.0 Major 2.13 feature: polymorphic serialization

2.12 changed `serialize_as_any` behavior and exposed edge cases. 2.13 introduces `polymorphic_serialization` as the preferred narrower control for serializing model/dataclass subclasses.

Migration question:

```text
Did code enable serialize_as_any only to expose subclass model fields?
  -> evaluate polymorphic_serialization instead
```

## 46.1 `exclude_if` on computed fields

2.12 added/expanded field-level `exclude_if`; 2.13 extends conditional exclusion to computed fields.

This can replace manual serializer logic for output fields such as empty summaries, optional computed metrics or conditional links.

## 46.2 `StringConstraints.ascii_only`

2.13 allows string constraints to require ASCII-only content without a custom validator.

Use for protocol identifiers where ASCII is a real requirement; do not impose it on human names/text.

## 46.3 Private attribute factory data

Private attribute default factories can receive already validated model data. This makes private cache/index initialization more expressive.

## 46.4 Extra fields and fields-set tracking

Allowed extra fields assigned after initialization now participate in `model_fields_set` tracking, improving update/delta semantics for extensible models.

## 46.5 Union serialization

Discriminator-selected serialization no longer falls back through unrelated branches if the selected branch fails. This is more deterministic but can surface errors that older code silently routed around.

## 46.6 Numeric/type changes

2.13 includes changes such as:

- unconditional Python `complex()` conversion path for Python complex validation;
- three-tuple `Decimal` support;
- named-tuple annotation handling corrections.

Edge-case heavy financial/scientific systems should include regression tests across minor upgrades.

## 46.7 Performance changes

The 2.13 release line contains multiple model/schema/runtime improvements, including:

- validator/serializer decorator processing optimizations;
- compiled regex caching in pydantic-core;
- optimized `Literal` validators;
- optimized union lookup machinery;
- optimized datetime formatting;
- JSON model data validation by iteration;
- annotation evaluation and `FieldInfo` copying improvements.

## 46.8 Patch train

### 2.13.1

Fixes missing `ValidationInfo.data` in `model_validate_json()`.

### 2.13.2

Fixes missing `ValidationInfo.field_name` in `model_validate_json()`.

### 2.13.3

Handles `AttributeError` subclasses correctly in `from_attributes` validation.

### 2.13.4

Preserves `RootModel` core metadata and includes packaging/linker maintenance.

## 46.9 Bundled V1

`pydantic.v1` matches V1.10.26 in the 2.13 release, including Python 3.14 support.

## 46.10 Upgrade checklist 2.12 → 2.13.4

```text
[ ] Search serialize_as_any usage; evaluate polymorphic_serialization.
[ ] Test discriminated union serialization failure paths.
[ ] Test model_validate_json validators using info.data/field_name.
[ ] Test from_attributes against proxy/ORM properties.
[ ] Test RootModel copy/schema behavior.
[ ] Snapshot critical serialization schema/output.
[ ] Benchmark model-build/validation if Pydantic is hot.
```

Sources: [V213], [CHANGELOG].

---

# Pydantic Advanced — 47) Pydantic 2.14 prerelease transition and Python-version boundary

## 47.0 Status

As of 2026-08-19, **2.14.0b1** is a prerelease and PyPI explicitly warns that it may not be stable for production use. This reference remains pinned to 2.13.4.

## 47.1 Python support change

The 2.14 line drops Python 3.9 and targets Python 3.10+. It also adds initial Python 3.15 support in the beta.

For libraries:

```text
support Python 3.9?
  -> stay on Pydantic 2.13.x (or conditional dependency)

move to Pydantic 2.14+
  -> raise package Python floor to >=3.10
```

## 47.2 2.14 beta notable additions

The beta includes:

- `EllipsisType` support;
- initial Python 3.15 support;
- `Fraction` support in pydantic-core;
- constant-time comparisons for `SecretStr` / `SecretBytes`;
- named-tuple core schema work;
- model class/core schema generation performance improvements;
- thread-safe model rebuilding;
- fixes across polymorphic serialization, `ValidateAs`, aliases and generic models.

## 47.3 Model-build focus

Several beta changes specifically optimize class/model build:

```text
private-attr metadata caching
annotation-evaluation fast paths
type lookup optimization
avoid exponential core-schema traversal
micro-optimizations in model class building
```

Large model catalogs may see meaningful startup improvements, but benchmark final stable 2.14 rather than assuming prerelease numbers/behavior.

## 47.4 Thread-safe rebuild

2.14 beta includes explicit work to make model rebuilding thread safe. If a 2.13 application dynamically calls `model_rebuild()` after threads/tasks are already serving requests, redesign so schema construction occurs before concurrency where possible rather than depending on unreleased fixes.

## 47.5 Secret comparison semantics

Constant-time comparison is a security-relevant improvement in the beta. Do not state that stable 2.13 secret equality is constant-time based on 2.14 release notes.

## 47.6 Migration gate

Before adopting stable 2.14 when released:

```text
[ ] raise Python floor if needed
[ ] run full type-checker matrix
[ ] snapshot schemas
[ ] test generics/forward refs/dynamic models
[ ] test secret-type semantics
[ ] test custom CoreSchema hooks
[ ] benchmark import/model build
[ ] review final 2.14 changelog, not beta notes alone
```

Sources: PyPI 2.14.0b1, [CHANGELOG].

---

# Pydantic Advanced — 48) Testing, schema snapshots, round-trip checks, fuzzing, and compatibility contracts

## 48.0 Test the boundary dimensions independently

For a public model, test:

```text
validation acceptance
validation rejection/error codes
normalized Python values
serialization Python mode
serialization JSON mode
JSON text
aliases
include/exclude
strict behavior
JSON Schema validation mode
JSON Schema serialization mode
```

## 48.1 Validation test

```python
def test_user_coercion():
    u = User.model_validate({'id': '1', 'name': 'Ada'})
    assert u.id == 1
```

## 48.2 Strict rejection test

```python
import pytest
from pydantic import ValidationError

with pytest.raises(ValidationError) as exc:
    User.model_validate({'id': '1', 'name': 'Ada'}, strict=True)

assert exc.value.errors()[0]['type'] == 'int_type'
```

Prefer checking error type/location rather than the whole human message.

## 48.3 Serialization shape test

```python
assert response.model_dump(mode='json', by_alias=True) == expected
```

For sensitive models, explicitly assert secret/internal fields are absent.

## 48.4 Round-trip test

Where the contract is intended to round trip:

```python
first = Model.model_validate(payload)
raw = first.model_dump_json(round_trip=True)
second = Model.model_validate_json(raw)
assert second == first
```

Not every serializer is intentionally invertible; only impose this property when it is part of the design.

## 48.5 Schema snapshot

```python
schema = Model.model_json_schema(mode='serialization')
```

Snapshot critical APIs, but normalize known non-semantic ordering where necessary. Combine snapshots with semantic assertions such as required fields, discriminator, formats and aliases.

## 48.6 Differential Python/JSON validation

If custom validators use `ValidationInfo` or strict mode, test both:

```python
Model.model_validate(python_payload)
Model.model_validate_json(json_payload)
```

The 2.13.1/2 patch fixes are a concrete example of why the two paths deserve separate regression coverage.

## 48.7 Hypothesis/property tests

Useful properties:

```text
valid normalized value always satisfies domain invariant
serialization output always revalidates (if round-trip expected)
no secret field appears in public dump
union discriminator always selects one intended variant
strict mode never accepts selected coercion classes
```

## 48.8 Golden migration tests

Before Pydantic upgrade:

1. record selected model outputs/schemas/errors on old version;
2. run same corpus on new version;
3. classify differences as intentional vs regression;
4. version external contracts when necessary.

## 48.9 Performance regression tests

Benchmark separately:

```text
model class creation
first validation
steady-state validation
JSON validation
serialization
error-heavy invalid input
```

## 48.10 Dynamic/custom type tests

Custom CoreSchema code requires more coverage than ordinary fields:

- Python and JSON input;
- strict/lax where applicable;
- serialization mode;
- schema generation;
- nested list/union/model context;
- forward references/generics;
- error paths.

Sources: [CHANGELOG], [PERF], testing/tool integrations.

---

# Pydantic Advanced — 49) Security, secrets, untrusted input, serialization exposure, and trust boundaries

## 49.0 Pydantic is a parser/validator, not a sandbox

Untrusted data can still consume CPU/memory before it is rejected. Bound the input before validation:

```text
HTTP body size
message size
archive/file decompression ratio
JSON nesting/depth
collection cardinality
string length
model/dynamic-schema complexity
```

Use Pydantic field constraints in addition to transport-level limits.

## 49.1 Unknown fields

For sensitive external write APIs:

```python
ConfigDict(extra='forbid')
```

prevents silently ignoring attacker/mistyped fields that callers may believe were honored.

## 49.2 Secret values

Use `SecretStr`/`SecretBytes` for masking, plus:

- `hide_input_in_errors=True` for sensitive validation boundaries;
- structured log redaction;
- secret-manager storage;
- minimal retention/lifetime.

Masking is not encryption.

## 49.3 Serialization exposure

Default annotation-driven V2 serialization is a security feature for base-class fields. Review any use of:

```text
SerializeAsAny
serialize_as_any=True
polymorphic_serialization=True
```

against sensitive subclass fields.

## 49.4 Separate public DTOs

Strongest pattern:

```text
InternalUser(password_hash, permissions, internal_notes)
  -> explicit mapping
PublicUser(id, name, avatar)
```

This is safer than one giant internal model with many `exclude=True` flags.

## 49.5 Validators and side effects

Avoid:

- network calls;
- database writes;
- filesystem mutation;
- shell commands;
- arbitrary imports;

inside validators. Validation may be retried, run during tests/schema flows or invoked unexpectedly by framework integrations.

## 49.6 `ImportString`

Treat untrusted `ImportString` as dangerous. Importing a module executes Python module code. Restrict allowlists or resolve known plugin IDs to imports in application code.

## 49.7 Regex

Rust regex's restricted feature set/linear-time design is often safer for untrusted text. Switching to Python regex for advanced features can reintroduce catastrophic-backtracking risks depending on patterns.

## 49.8 Filesystem validation

`FilePath`/`DirectoryPath` validate paths relative to the current host and can reveal existence/errors. Do not expose arbitrary server filesystem validation to untrusted users without path sandboxing.

## 49.9 Error leakage

`ValidationError` may include raw input and type information. Public APIs should translate errors into safe contract errors.

## 49.10 Settings/secrets sources

Pin pydantic-settings to patched versions. Control secret-directory permissions, symlink behavior, max sizes and cloud credential scopes.

## 49.11 Dynamic models/schema DoS

If users can request dynamically generated schemas:

```text
limit field count
limit nesting/union width
limit generated model cache entries
limit regex/pattern complexity
reject arbitrary Python callable validators
```

## 49.12 Security checklist

```text
[ ] Bound bytes/depth/cardinality before or during validation.
[ ] Forbid unexpected fields where contract requires exactness.
[ ] Hide/redact sensitive input in errors/logs.
[ ] Keep public response DTOs separate from sensitive internal models.
[ ] Review polymorphic / any-serialization flags.
[ ] Restrict ImportString/plugin imports.
[ ] Avoid I/O/side effects in validators/serializers.
[ ] Pin settings/security-sensitive companion packages.
```

Sources: [CONFIG], [SER], types/settings docs, security/release history.

---

# Pydantic Advanced — 50) Production architecture patterns

## 50.0 Pattern A — HTTP/API boundary DTOs

```text
HTTP bytes
  -> web framework limits/content parsing
  -> Pydantic request model (extra='forbid', aliases, constraints)
  -> domain command/object
  -> service logic
  -> explicit public response model
  -> Pydantic serialization
  -> HTTP response
```

Use separate input/output models when exposure and accepted data differ.

## 50.1 Pattern B — versioned event ingestion

```python
class EventV1(BaseModel):
    version: Literal[1]
    ...

class EventV2(BaseModel):
    version: Literal[2]
    ...

Event = Annotated[EventV1 | EventV2, Field(discriminator='version')]
EVENT = TypeAdapter(Event)
```

Benefits:

- deterministic branch selection;
- explicit migration logic;
- stable JSON Schema;
- fast validation.

## 50.2 Pattern C — settings snapshot

```text
process startup
  -> BaseSettings reads env/secrets/config
  -> validated immutable-ish Settings object
  -> injected into services
```

Avoid repeatedly rereading process environment during request handling.

## 50.3 Pattern D — persistence mapping

```text
DB ORM object
  -> Pydantic internal read DTO via from_attributes
  -> domain logic
  -> persistence mapper
  -> ORM/update statement
```

Do not make Pydantic own transaction/session behavior.

## 50.4 Pattern E — high-throughput JSON queue

```python
EVENT = TypeAdapter(Event)

for raw in messages:
    event = EVENT.validate_json(raw)
    process(event)
```

Add message byte limits before validation and use discriminated unions for variants.

## 50.5 Pattern F — plugin config

```text
untrusted-ish config
  -> strict Pydantic plugin configuration model
  -> known plugin ID
  -> application allowlist resolves implementation
```

Prefer this over raw `ImportString` from user data.

## 50.6 Pattern G — dynamic schema platform

```text
schema specification
  -> validate schema spec itself
  -> normalize + hash
  -> lookup bounded model cache
  -> create_model only on cache miss
  -> contract tests / JSON Schema publication
```

Keep schema generation isolated from ordinary request business logic.

## 50.7 Pattern H — LLM streaming structured output

```text
LLM token stream
  -> experimental partial TypeAdapter validation for UX
  -> completed JSON
  -> full TypeAdapter validation
  -> semantic/domain validation
  -> side effects
```

Never trigger irreversible actions from only partial validation.

## 50.8 Pattern I — shared reusable annotated types

```python
# contracts/types.py
Email = Annotated[str, ...]
TenantId = Annotated[str, ...]
PositiveMoney = Annotated[Decimal, ...]
```

Use these across request models, settings and events so the low-level format contract is consistent.

## 50.9 Pattern J — public library boundary

```text
public callable
  -> @validate_call or explicit TypeAdapter/model validation
  -> stable Python return type
  -> custom ValidationError translation if library UX requires it
```

Only add runtime validation where the library contract benefits enough to justify overhead.

## 50.10 Architecture principles

1. Validate at system boundaries, not indiscriminately at every internal function.
2. Separate transport, domain, persistence and settings contracts.
3. Prefer declarative constraints and discriminators over Python validators.
4. Make alias/serialization behavior explicit for public schemas.
5. Reuse compiled models/adapters.
6. Keep validation deterministic and side-effect free.
7. Treat schema/serialization changes as API changes.
8. Version dynamic/external contracts.
9. Instrument validation failures and model-build cost.
10. Keep security boundaries outside Pydantic too: input limits, auth, secret management, I/O sandboxing.

Sources: synthesis of the stable APIs and deployment guidance above.


---

# Pydantic Advanced — 51) Dense appendices and lookup matrices

This section is intentionally repetitive and lookup-oriented. It is designed so an LLM coding agent can retrieve a narrow answer without reconstructing semantics from narrative chapters.

## 51.1 Stable version matrix

| Component | Pinned reference version | Python floor | Status |
|---|---:|---:|---|
| `pydantic` | 2.13.4 | >=3.9 | stable |
| `pydantic-core` | 2.46.4 | follows Pydantic wheel support | exact core paired with 2.13.4 |
| bundled `pydantic.v1` | 1.10.26 behavior line | includes Python 3.14 support under Pydantic 2.13 | compatibility namespace |
| `pydantic-settings` | 2.15.0 | >=3.10 | separate stable companion |
| `pydantic` 2.14.0b1 | prerelease | >=3.10 | do not mix into stable examples |

## 51.2 Installation matrix

```bash
# Stable core
pip install 'pydantic==2.13.4'

# Email types
pip install 'pydantic[email]==2.13.4'

# Timezone-data extra
pip install 'pydantic[timezone]==2.13.4'

# Settings
pip install 'pydantic-settings==2.15.0'

# Settings cloud sources
pip install 'pydantic-settings[aws-secrets-manager]==2.15.0'
pip install 'pydantic-settings[azure-key-vault]==2.15.0'
pip install 'pydantic-settings[gcp-secret-manager]==2.15.0'

# Settings file formats
pip install 'pydantic-settings[toml,yaml]==2.15.0'
```

## 51.3 `BaseModel` primary validation signatures — 2.13.4

```python
@classmethod
def model_validate(
    cls,
    obj: Any,
    *,
    strict: bool | None = None,
    extra: ExtraValues | None = None,
    from_attributes: bool | None = None,
    context: Any | None = None,
    by_alias: bool | None = None,
    by_name: bool | None = None,
) -> Self: ...
```

```python
@classmethod
def model_validate_json(
    cls,
    json_data: str | bytes | bytearray,
    *,
    strict: bool | None = None,
    extra: ExtraValues | None = None,
    context: Any | None = None,
    by_alias: bool | None = None,
    by_name: bool | None = None,
) -> Self: ...
```

`model_validate_strings()` is the corresponding string-oriented validation entry point for nested mappings of strings. Use the stable API docs for the exact method signature when generating a wrapper library.

## 51.4 `BaseModel.model_rebuild()` — 2.13.4

```python
@classmethod
def model_rebuild(
    cls,
    *,
    force: bool = False,
    raise_errors: bool = True,
    _parent_namespace_depth: int = 2,
    _types_namespace: MappingNamespace | None = None,
) -> bool | None: ...
```

Return semantics:

```text
None   -> schema already complete; no rebuild needed
True   -> rebuild required and succeeded
False  -> rebuild required and failed, when errors not raised
```

## 51.5 `model_copy()` — 2.13.4

```python
def model_copy(
    *,
    update: Mapping[str, Any] | None = None,
    deep: bool = False,
) -> Self: ...
```

Critical invariant:

```text
update values are NOT validated
```

Use only with trusted update data.

## 51.6 `model_dump()` — exact stable 2.13.4 surface

```python
def model_dump(
    *,
    mode: Literal['json', 'python'] | str = 'python',
    include: IncEx | None = None,
    exclude: IncEx | None = None,
    context: Any | None = None,
    by_alias: bool | None = None,
    exclude_unset: bool = False,
    exclude_defaults: bool = False,
    exclude_none: bool = False,
    exclude_computed_fields: bool = False,
    round_trip: bool = False,
    warnings: bool | Literal['none', 'warn', 'error'] = True,
    fallback: Callable[[Any], Any] | None = None,
    serialize_as_any: bool = False,
    polymorphic_serialization: bool | None = None,
) -> dict[str, Any]: ...
```

### Option quick map

| Parameter | Meaning |
|---|---|
| `mode='python'` | preserve Python-compatible representation where serializers allow |
| `mode='json'` | produce JSON-compatible Python values |
| `include` | whitelist fields/subtrees |
| `exclude` | blacklist fields/subtrees |
| `context` | serializer context object |
| `by_alias` | emit aliases |
| `exclude_unset` | omit fields not explicitly supplied/set |
| `exclude_defaults` | omit values equal to defaults |
| `exclude_none` | omit `None` |
| `exclude_computed_fields` | omit computed fields |
| `round_trip` | produce representation suitable for revalidation of non-idempotent types |
| `warnings` | ignore/warn/error on serialization issues |
| `fallback` | custom handler for unknown values |
| `serialize_as_any` | broad duck-typed serialization |
| `polymorphic_serialization` | model/dataclass subclass polymorphism, new 2.13 surface |

## 51.7 `model_dump_json()` — exact stable 2.13.4 surface

```python
def model_dump_json(
    *,
    indent: int | None = None,
    ensure_ascii: bool = False,
    include: IncEx | None = None,
    exclude: IncEx | None = None,
    context: Any | None = None,
    by_alias: bool | None = None,
    exclude_unset: bool = False,
    exclude_defaults: bool = False,
    exclude_none: bool = False,
    exclude_computed_fields: bool = False,
    round_trip: bool = False,
    warnings: bool | Literal['none', 'warn', 'error'] = True,
    fallback: Callable[[Any], Any] | None = None,
    serialize_as_any: bool = False,
    polymorphic_serialization: bool | None = None,
) -> str: ...
```

`ensure_ascii=False` means non-ASCII characters are emitted as-is by default.

## 51.8 Validation vs construction decision matrix

| Situation | API |
|---|---|
| ordinary kwargs in Python | `Model(...)` |
| explicit Python boundary | `Model.model_validate(...)` |
| raw JSON | `Model.model_validate_json(...)` |
| nested string-origin mapping | `Model.model_validate_strings(...)` |
| already trusted/prevalidated fields | `Model.model_construct(...)` |
| non-model type | `TypeAdapter(T).validate_*` |

Never choose `model_construct()` solely for convenience or to suppress validation errors.

## 51.9 `TypeAdapter` constructor

```python
TypeAdapter(
    type,
    *,
    config: ConfigDict | None = None,
    _parent_depth: int = 2,
    module: str | None = None,
)
```

Do not pass `config=` to a model/dataclass/TypedDict type that already owns its config; Pydantic rejects/ignores such an override according to type semantics.

## 51.10 `TypeAdapter.validate_python()` — 2.13.4

```python
def validate_python(
    object: Any,
    /,
    *,
    strict: bool | None = None,
    extra: ExtraValues | None = None,
    from_attributes: bool | None = None,
    context: Any | None = None,
    experimental_allow_partial:
        bool | Literal['off', 'on', 'trailing-strings'] = False,
    by_alias: bool | None = None,
    by_name: bool | None = None,
) -> T: ...
```

Note: `from_attributes` is not supported when the adapter wraps a Pydantic dataclass.

## 51.11 `TypeAdapter.validate_json()` — 2.13.4

```python
def validate_json(
    data: str | bytes | bytearray,
    /,
    *,
    strict: bool | None = None,
    extra: ExtraValues | None = None,
    context: Any | None = None,
    experimental_allow_partial:
        bool | Literal['off', 'on', 'trailing-strings'] = False,
    by_alias: bool | None = None,
    by_name: bool | None = None,
) -> T: ...
```

## 51.12 `TypeAdapter.dump_python()` — 2.13.4

```python
def dump_python(
    instance: T,
    /,
    *,
    mode: Literal['json', 'python'] = 'python',
    include: IncEx | None = None,
    exclude: IncEx | None = None,
    by_alias: bool | None = None,
    exclude_unset: bool = False,
    exclude_defaults: bool = False,
    exclude_none: bool = False,
    exclude_computed_fields: bool = False,
    round_trip: bool = False,
    warnings: bool | Literal['none', 'warn', 'error'] = True,
    fallback: Callable[[Any], Any] | None = None,
    serialize_as_any: bool = False,
    polymorphic_serialization: bool | None = None,
    context: Any | None = None,
) -> Any: ...
```

## 51.13 `TypeAdapter` method matrix

| Method | Purpose |
|---|---|
| `validate_python` | Python object validation |
| `validate_json` | raw JSON validation |
| `validate_strings` | string-origin nested data |
| `get_default_value` | retrieve wrapped type default if defined |
| `dump_python` | serialize to Python/JSON-compatible values |
| `dump_json` | serialize directly to JSON bytes |
| `json_schema` | generate schema for this type |
| `json_schemas` | generate schemas for multiple adapters/modes |
| `rebuild` | rebuild forward-reference/core schema state |

## 51.14 `Field()` parameter-category matrix

### Value/default

```text
default
default_factory
```

### Aliases

```text
alias
alias_priority
validation_alias
serialization_alias
```

### Documentation/schema

```text
title
field_title_generator
description
examples
deprecated
json_schema_extra
```

### Serialization/UI

```text
exclude
exclude_if
repr
```

### Model/dataclass behavior

```text
frozen
validate_default
init
init_var
kw_only
```

### String/container

```text
pattern
min_length
max_length
coerce_numbers_to_str
```

### Numeric

```text
gt
ge
lt
le
multiple_of
allow_inf_nan
max_digits
decimal_places
```

### Union

```text
discriminator
union_mode = 'smart' | 'left_to_right'
```

### Failure behavior

```text
fail_fast
strict
```

## 51.15 Field declaration decision table

| Need | Recommended syntax |
|---|---|
| simple required field | `x: T` |
| nullable required field | `x: T | None` |
| optional/omittable field | `x: T | None = None` or other default |
| runtime-generated default | `Field(default_factory=...)` |
| numeric/string constraints | `Annotated[T, Field(...)]` |
| reusable type policy | named `Annotated` alias |
| legacy input key | `validation_alias` / `AliasChoices` |
| different output key | `serialization_alias` |
| union tag | `Field(discriminator='kind')` |
| conditional dump exclusion | `exclude_if=` |

## 51.16 `ConfigDict` complete stable 2.13.4 attribute inventory

The stable API surface documents these configuration keys:

```text
title
model_title_generator
field_title_generator
str_to_lower
str_to_upper
str_strip_whitespace
str_min_length
str_max_length
extra
frozen
populate_by_name
use_enum_values
validate_assignment
arbitrary_types_allowed
from_attributes
loc_by_alias
alias_generator
ignored_types
allow_inf_nan
json_schema_extra
json_encoders
strict
revalidate_instances
ser_json_timedelta
ser_json_temporal
val_temporal_unit
ser_json_bytes
val_json_bytes
ser_json_inf_nan
validate_default
validate_return
protected_namespaces
hide_input_in_errors
defer_build
plugin_settings
schema_generator
json_schema_serialization_defaults_required
json_schema_mode_override
coerce_numbers_to_str
regex_engine
validation_error_cause
use_attribute_docstrings
cache_strings
validate_by_alias
validate_by_name
serialize_by_alias
url_preserve_empty_path
polymorphic_serialization
```

### Deprecated/discouraged configuration notes

```text
populate_by_name
  -> not recommended in 2.11+; prefer validate_by_name + validate_by_alias

json_encoders
  -> deprecated V1 carry-over; prefer serializers

schema_generator
  -> deprecated/legacy configuration surface

ser_json_timedelta
  -> newer temporal configuration offers broader replacement path
```

## 51.17 `ConfigDict` recommended profiles

### Strict external write DTO

```python
ConfigDict(
    extra='forbid',
    strict=True,
    validate_by_alias=True,
    validate_by_name=False,
    serialize_by_alias=True,
    hide_input_in_errors=True,
)
```

Only use full strictness if the wire representation is intended to be exact.

### Flexible external read/ingestion DTO

```python
ConfigDict(
    extra='ignore',
    validate_by_alias=True,
    validate_by_name=True,
)
```

### ORM read DTO

```python
ConfigDict(
    from_attributes=True,
    extra='ignore',
)
```

### Internal immutable key/value model

```python
ConfigDict(
    frozen=True,
    extra='forbid',
)
```

### Huge lazy model catalog

```python
ConfigDict(
    defer_build=True,
)
```

## 51.18 Extra-field matrix

| `extra` | Unknown input | Stored? | Typical use |
|---|---|---:|---|
| `'ignore'` | accepted/discarded | no | forward-compatible ingestion |
| `'forbid'` | `ValidationError` | no | strict API/config contract |
| `'allow'` | accepted | `__pydantic_extra__` | extensible envelope |

Per-call validation can override model `extra` in current V2.

## 51.19 Strictness matrix

| Layer | Syntax |
|---|---|
| one validation call | `Model.model_validate(data, strict=True)` |
| `TypeAdapter` call | `adapter.validate_python(data, strict=True)` |
| one field | `Field(strict=True)` |
| reusable type | `Annotated[T, Strict()]` |
| whole model | `ConfigDict(strict=True)` |
| one field opt-out from strict model | `Field(strict=False)` |

## 51.20 Alias matrix

| Requirement | Mechanism |
|---|---|
| one shared alias | `Field(alias='x')` |
| input-only alias | `validation_alias=` |
| output-only alias | `serialization_alias=` |
| nested input lookup | `AliasPath(...)` |
| multiple accepted legacy names | `AliasChoices(...)` |
| generated names | `alias_generator=` |
| different validation/output generators | `AliasGenerator(...)` |
| accept Python field name | `validate_by_name=True` |
| accept alias | `validate_by_alias=True` |
| emit aliases by default | `serialize_by_alias=True` |

## 51.21 Validator mode matrix

| Mode | Input seen | Calls inner validation? | Best use | Cost/risks |
|---|---|---|---|---|
| `after` | validated typed value | already occurred | domain constraint | safest/default |
| `before` | raw input | yes after return | normalization | must handle `Any`; union mutation risk |
| `plain` | raw/current input | no normal continuation | replace validation | can bypass annotation validation |
| `wrap` | raw/current + handler | optionally | intercept/retry/log | highest complexity/overhead |

## 51.22 Field vs model validator decision

```text
one field, declarative constraint
  -> Field / Annotated metadata

one field, typed semantic check
  -> @field_validator(..., mode='after')

one field, raw normalization
  -> mode='before'

whole model invariant
  -> @model_validator(mode='after')

whole raw object structural migration
  -> @model_validator(mode='before')

intercept complete validation
  -> model wrap validator
```

## 51.23 Validator error matrix

| Need | Raise/use |
|---|---|
| ordinary invalid value | `ValueError` |
| stable custom code + context | `PydanticCustomError` |
| assertion-style developer invariant | avoid `assert` for required runtime validation |
| programming/type bug | allow `TypeError` to surface rather than treating as user validation |
| invalid model definition/config | Pydantic raises `PydanticUserError` |

## 51.24 Serialization-mode matrix

| Need | API |
|---|---|
| Python-native mapping | `model_dump()` |
| JSON-compatible Python mapping | `model_dump(mode='json')` |
| JSON text | `model_dump_json()` |
| arbitrary type Python dump | `TypeAdapter.dump_python()` |
| arbitrary type JSON bytes | `TypeAdapter.dump_json()` |
| external key names | `by_alias=True` / `serialize_by_alias=True` |
| revalidatable special representation | `round_trip=True` |

## 51.25 Exclusion matrix

| Requirement | Option |
|---|---|
| omit never-supplied fields | `exclude_unset=True` |
| omit values equal to defaults | `exclude_defaults=True` |
| omit nulls | `exclude_none=True` |
| omit computed fields | `exclude_computed_fields=True` |
| omit one field always | `Field(exclude=True)` |
| omit based on value | `Field(exclude_if=...)` |
| whitelist fields | `include=...` |
| blacklist fields/subtrees | `exclude=...` |

## 51.26 Polymorphic serialization matrix

| Mode | Behavior | Security posture |
|---|---|---|
| default | serialize according to annotated schema | closed/safest default |
| `polymorphic_serialization` | include model/dataclass subclass fields | targeted opt-in |
| `SerializeAsAny[T]` | annotation-level duck-typed output | broad; review exposure |
| `serialize_as_any=True` | runtime broad duck typing | broadest; avoid global casual use |

## 51.27 `TypeAdapter` vs `RootModel` vs `BaseModel`

| Need | Choose |
|---|---|
| validate `list[int]` | `TypeAdapter(list[int])` |
| class with methods but root array wire shape | `RootModel[list[int]]` |
| named object fields | `BaseModel` |
| dict-shaped typed structure, low overhead | `TypedDict` + `TypeAdapter` |
| existing dataclass | `TypeAdapter` or Pydantic dataclass |

## 51.28 Union decision matrix

| Need | Choice |
|---|---|
| variants share a tag | discriminated union |
| deterministic first-success | `union_mode='left_to_right'` |
| ambiguous primitive union | smart mode, test examples |
| custom tag extraction | callable `Discriminator` + `Tag` |
| performance-sensitive structured union | discriminator |

## 51.29 Customization escalation matrix

```text
Field constraint
  -> Annotated metadata
      -> field/model validator
          -> functional validator/serializer
              -> ValidateAs / InstanceOf / SkipValidation
                  -> GetPydanticSchema / __get_pydantic_core_schema__
                      -> direct pydantic-core
```

Choose the highest-level mechanism that fully expresses the contract.

## 51.30 JSON validation decision table

| Input source | Preferred path |
|---|---|
| Python dict/object | `model_validate` / `validate_python` |
| raw JSON | `model_validate_json` / `validate_json` |
| env/form/string map | `model_validate_strings` / `validate_strings` |
| incomplete streaming JSON | `TypeAdapter.validate_json(..., experimental_allow_partial=...)` |
| already validated trusted values | `model_construct` only if model semantics needed |

## 51.31 JSON Schema generation matrix

| Need | API |
|---|---|
| model input schema | `Model.model_json_schema(mode='validation')` |
| model output schema | `Model.model_json_schema(mode='serialization')` |
| arbitrary type schema | `TypeAdapter(T).json_schema()` |
| local schema metadata | `Field(..., json_schema_extra=...)` |
| model schema metadata | `ConfigDict(json_schema_extra=...)` |
| reusable annotated override | `WithJsonSchema(...)` |
| custom type schema | `__get_pydantic_json_schema__` |
| global policy | subclass `GenerateJsonSchema` |

## 51.32 Error-detail lookup

Typical `ValidationError.errors()` item:

```python
{
    'type': 'int_parsing',
    'loc': ('items', 2, 'count'),
    'msg': 'Input should be a valid integer, unable to parse string as an integer',
    'input': 'abc',
    'ctx': {...},        # optional
    'url': '...',        # may be present
}
```

Machine logic should use `type` and `loc`, not parse `msg`.

## 51.33 Common validation error code categories

Examples include categories such as:

```text
missing
extra_forbidden
int_type
int_parsing
float_type
string_type
string_too_short
string_too_long
literal_error
union_tag_invalid
union_tag_not_found
is_instance_of
greater_than
greater_than_equal
less_than
less_than_equal
multiple_of
```

Do not hardcode a list without checking the target version; error code inventory evolves.

## 51.34 Strict vs lax API design matrix

| Boundary | Typical policy |
|---|---|
| public machine-to-machine API | strict where representation is explicit; forbid extras |
| browser form / CLI / env | selected lax conversion useful |
| internal message queue | explicit version/tag + mostly strict schema |
| ORM extraction | typed output; `from_attributes=True` |
| LLM output | lax structural normalization if intended, then semantic checks |
| already typed internal domain object | avoid repeated validation unless boundary changed |

## 51.35 Standard type quick matrix

| Type family | Examples | Important note |
|---|---|---|
| scalar | `int`, `float`, `bool`, `str`, `bytes` | lax coercions differ; inspect conversion table |
| temporal | `datetime`, `date`, `time`, `timedelta` | JSON strict can accept standardized strings |
| numeric exact | `Decimal`, `Fraction` | `Fraction` core support is 2.14 prerelease, not stable 2.13 feature |
| IDs | `UUID` | validation does not prove security/randomness |
| collections | `list`, `tuple`, `set`, `dict` | concrete types faster than broad abstract types |
| enums | `Enum`, `Literal` | `Literal` excellent for discriminators |
| paths | `Path`, `FilePath`, `DirectoryPath` | filesystem-specific types touch host state |
| patterns | `re.Pattern` | regex engine/config matters |
| arbitrary | `Any` | intentionally bypasses validation |

## 51.36 Network type quick matrix

| Need | Type family |
|---|---|
| HTTP URL | `HttpUrl` / `AnyHttpUrl` |
| generic URL | `AnyUrl` |
| websocket | `WebsocketUrl` / `AnyWebsocketUrl` |
| database connection | specific `*Dsn` type |
| email | `EmailStr` / `NameEmail` + `[email]` extra |
| IP | stdlib `ipaddress` types |
| UUID | stdlib `UUID` / specialized UUID constraints |

## 51.37 Secret-handling quick rules

```text
SecretStr/SecretBytes
  masks repr/display
  does NOT encrypt
  actual value via get_secret_value()

hide_input_in_errors=True
  reduces raw value leakage in validation error text

public DTO separation
  strongest defense against accidental sensitive serialization
```

## 51.38 `pydantic-settings` source map

Potential settings sources include:

```text
constructor kwargs
environment variables
.env files
file secrets
nested secrets
CLI arguments
TOML/YAML/pyproject/custom files
AWS Secrets Manager
Azure Key Vault
Google Cloud Secret Manager
custom PydanticBaseSettingsSource implementations
```

Actual order is determined by defaults plus `settings_customise_sources()`.

## 51.39 Settings extras — 2.15.0

```text
aws-secrets-manager
azure-key-vault
gcp-secret-manager
toml
yaml
```

## 51.40 Settings environment controls to know

Common `SettingsConfigDict` concepts include:

```text
env_prefix
env_file
env_file_encoding
env_nested_delimiter
env_nested_max_split
env_ignore_empty
env_parse_none_str
env_parse_enums
case_sensitive
secrets_dir
cli_parse_args
```

The exact settings package exposes many additional CLI/secrets source options; consult the 2.15.0 API when generating a settings framework.

## 51.41 Settings security checklist

```text
[ ] Pin pydantic-settings to a patched release.
[ ] Keep .env secrets out of source control.
[ ] Restrict secret-directory permissions.
[ ] Define source priority explicitly.
[ ] Avoid logging Settings.model_dump() when it includes credentials.
[ ] Wrap sensitive fields with SecretStr/SecretBytes where useful.
[ ] Scope cloud secret-manager identity narrowly.
[ ] Treat ImportString as code-loading capability.
```

## 51.42 Performance rules — official guide condensed

```text
1. Prefer model_validate_json over model_validate(json.loads(...)) in general.
2. Instantiate TypeAdapter once; reuse it.
3. Prefer list/tuple/dict over Sequence/Mapping when concrete types suffice.
4. Use Any when validation is intentionally unnecessary.
5. Avoid subclasses of primitives merely to carry extra state.
6. Prefer tagged/discriminated unions over broad unions.
7. Prefer TypedDict over nested models when dict output/object behavior is sufficient.
8. Avoid wrap validators in performance-sensitive paths.
9. Use FailFast when only the first collection error is needed.
```

## 51.43 Performance architecture matrix

| Bottleneck | First actions |
|---|---|
| import/startup | measure model build; `defer_build`; reduce dynamic schemas |
| raw JSON | direct JSON validation; reuse model/adapter |
| union validation | discriminator |
| nested object allocation | consider TypedDict |
| invalid huge list | `FailFast`, length limits |
| repeated adapter creation | module-level adapter cache |
| dynamic schema memory | bounded create_model cache |
| serializer Python overhead | remove unnecessary wrap/custom serializers |

## 51.44 V1 → V2 rename matrix

| V1 | V2 |
|---|---|
| `BaseModel.dict()` | `model_dump()` |
| `BaseModel.json()` | `model_dump_json()` |
| `parse_obj()` | `model_validate()` |
| `construct()` | `model_construct()` |
| `copy()` | `model_copy()` |
| `schema()` / `json_schema()` | `model_json_schema()` |
| `update_forward_refs()` | `model_rebuild()` |
| `__fields__` | `model_fields` |
| `@validator` | `@field_validator` |
| `@root_validator` | `@model_validator` |
| `Config.orm_mode` | `ConfigDict(from_attributes=True)` |
| `GenericModel` | `BaseModel` + `Generic[T]` |
| custom `__root__` | `RootModel[T]` |
| `BaseSettings` in core | `pydantic-settings.BaseSettings` |
| `__get_validators__` | `__get_pydantic_core_schema__` / annotated validators |
| `__modify_schema__` | `__get_pydantic_json_schema__` / JSON schema metadata |

## 51.45 V1 optionality migration trap

```python
# V2: required and nullable
x: str | None

# V2: omittable with default None
x: str | None = None
```

Audit every V1 `Optional[T]` declaration whose callers omitted the field.

## 51.46 V2 model equality migration trap

Do not rely on:

```python
model == {'field': value}
```

V2 model equality is model-aware and no longer treats equivalent dicts as equal in the V1 style.

## 51.47 V2 serialization migration trap

Subclass fields are no longer automatically included through base-class annotations. Review tests that depended on V1-style recursive `dict()` output from subclasses.

## 51.48 Release 2.13.4 checkpoint

```text
2.13.0  2026-04-13  main stable feature release
2.13.1  2026-04-15  ValidationInfo.data JSON fix
2.13.2  2026-04-17  ValidationInfo.field_name JSON fix
2.13.3  2026-04-20  from_attributes AttributeError subclass fix
2.13.4  2026-05-06  RootModel core metadata + packaging fixes
```

## 51.49 Release 2.14 prerelease checkpoint

```text
2.14.0a1  2026-05-22  Python 3.9 dropped
2.14.0b1  2026-08-06  first beta; initial Python 3.15; model-build improvements
```

Do not write stable 2.13 code against 2.14-only features such as new `EllipsisType`/`Fraction` core behavior or constant-time secret comparison.

## 51.50 Model contract design checklist

```text
[ ] What is the trust source: JSON/API/env/ORM/internal cache?
[ ] Is lax coercion intentional?
[ ] Should unknown fields be ignored, allowed, or rejected?
[ ] Is null different from omitted?
[ ] Are aliases part of a published wire contract?
[ ] Does serialization need base-closed or polymorphic behavior?
[ ] Is JSON Schema an external compatibility artifact?
[ ] Do validators depend on field order/context?
[ ] Can validation stay deterministic and side-effect free?
[ ] Should input values be hidden in errors?
[ ] Is this model transport, domain, persistence, or settings state?
```

## 51.51 Agent anti-pattern checklist

```text
[ ] Do not use model_construct() on untrusted input.
[ ] Do not recreate TypeAdapter in loops.
[ ] Do not use Any simply to silence errors.
[ ] Do not put database/network calls in validators.
[ ] Do not rely on Optional[T] to imply a default.
[ ] Do not use deprecated V1 methods in new V2 code.
[ ] Do not use populate_by_name in new code when validate_by_* is clearer.
[ ] Do not enable arbitrary_types_allowed globally as a shortcut.
[ ] Do not broadly enable serialize_as_any without exposure review.
[ ] Do not use one internal model as every public response DTO.
[ ] Do not parse ValidationError strings for machine logic.
[ ] Do not snapshot giant JSON Schema text without semantic assertions.
[ ] Do not create equivalent dynamic models per request.
[ ] Do not treat SecretStr as encryption.
[ ] Do not accept arbitrary ImportString from untrusted users.
```

## 51.52 Validation boundary cookbook

### Exact external JSON object

```python
class Request(BaseModel):
    model_config = ConfigDict(
        extra='forbid',
        validate_by_alias=True,
        validate_by_name=False,
        hide_input_in_errors=True,
    )
    id: Annotated[int, Field(gt=0, strict=True)]
```

### Human-friendly config

```python
class Config(BaseModel):
    model_config = ConfigDict(
        extra='forbid',
        str_strip_whitespace=True,
    )
    port: Annotated[int, Field(ge=1, le=65535)]
    enabled: bool = True
```

### Extensible event envelope

```python
class Envelope(BaseModel):
    model_config = ConfigDict(extra='allow')
    event_type: str
    version: int
```

### ORM projection

```python
class UserRead(BaseModel):
    model_config = ConfigDict(from_attributes=True)
    id: int
    name: str
```

### Patch semantics

```python
class UserPatch(BaseModel):
    name: str | None = None
    bio: str | None = None

changes = UserPatch.model_validate(raw).model_dump(exclude_unset=True)
```

## 51.53 Serializer cookbook

### Always hide field

```python
password_hash: str = Field(exclude=True)
```

### Conditionally hide `None`

```python
debug: str | None = Field(default=None, exclude_if=lambda v: v is None)
```

### Custom format

```python
@field_serializer('amount')
def serialize_amount(self, value: Decimal) -> str:
    return format(value, 'f')
```

### Context redaction

```python
@field_serializer('email')
def serialize_email(self, value: str, info):
    if (info.context or {}).get('redact'):
        return '***'
    return value
```

### Public subclass safety

```python
class PublicEnvelope(BaseModel):
    user: PublicUser  # keep default schema-closed serialization
```

## 51.54 Union cookbook

### Tagged variants

```python
class A(BaseModel):
    type: Literal['a']
    x: int

class B(BaseModel):
    type: Literal['b']
    y: str

Variant = Annotated[A | B, Field(discriminator='type')]
```

### Ordered coercion

```python
value: int | str = Field(union_mode='left_to_right')
```

Use only if “first successful member” is a documented requirement.

## 51.55 Custom type cookbook

### Simple constraint

```python
Positive = Annotated[int, Field(gt=0)]
```

### Simple normalization

```python
Upper = Annotated[str, AfterValidator(str.upper)]
```

### Validate via intermediary

```python
Domain = Annotated[DomainType, ValidateAs(int, DomainType)]
```

### Full hook

```python
class DomainType:
    @classmethod
    def __get_pydantic_core_schema__(cls, source_type, handler):
        ...

    @classmethod
    def __get_pydantic_json_schema__(cls, core_schema, handler):
        ...
```

## 51.56 Schema contract cookbook

### Validation schema

```python
request_schema = Request.model_json_schema(mode='validation')
```

### Serialization schema

```python
response_schema = Response.model_json_schema(mode='serialization')
```

### Arbitrary type

```python
schema = TypeAdapter(list[Response]).json_schema()
```

### Vendor extension

```python
Field(json_schema_extra={'x-sensitive': True})
```

### Global generator policy

```python
class MyGenerateJsonSchema(GenerateJsonSchema):
    ...
```

## 51.57 Error translation cookbook

```python
from pydantic import ValidationError

try:
    value = Request.model_validate(raw)
except ValidationError as exc:
    public_errors = [
        {
            'code': item['type'],
            'path': list(item['loc']),
            'message': item['msg'],
        }
        for item in exc.errors()
    ]
```

For sensitive systems, consider replacing `message` with application-owned safe text and never echo `item['input']`.

## 51.58 Upgrade discipline checklist

Before Pydantic minor upgrade:

```text
[ ] Read the full minor changelog and patch notes.
[ ] Confirm Python support floor/ceiling.
[ ] Run type checker/plugin tests.
[ ] Run JSON/Python validation corpus.
[ ] Compare serialization output.
[ ] Compare validation and serialization JSON Schema.
[ ] Test discriminated unions.
[ ] Test custom validators/serializers/context.
[ ] Test custom CoreSchema types.
[ ] Test RootModel/generics/forward references.
[ ] Run FastAPI/OpenAPI snapshots if applicable.
[ ] Run settings source priority/security tests.
[ ] Benchmark startup/model-build and hot validation.
```

## 51.59 Source-of-truth map by question

| Question | Primary source |
|---|---|
| exact stable method signature | stable API reference / source tag |
| what input coercions happen? | strict mode + conversion table + type docs |
| how should I write validators? | validators concept guide |
| how does output work? | serialization guide/API |
| what configuration exists? | `ConfigDict` API |
| how do aliases work? | alias guide + config API |
| how do custom types work? | custom types + Pydantic Core docs |
| what JSON Schema is generated? | JSON Schema guide/API + actual generated test |
| why did upgrade change behavior? | changelog + release article |
| how do settings sources work? | pydantic-settings docs/versioned package |
| what is experimental? | experimental docs + changelog |

## 51.60 Final agent invariants

1. **Validate at trust boundaries.** Do not revalidate every internal value without purpose.
2. **Successful Pydantic validation describes the output, not the original input.** Coercion is normal unless strictness says otherwise.
3. **Never use `model_construct()` as a substitute for validation on untrusted data.**
4. **Treat aliases, extra handling, strictness, JSON Schema and serialization as API contract decisions.**
5. **Prefer declarative metadata to Python validators.** It is faster, more schema-visible and easier to reason about.
6. **Use discriminated unions for structured variants.** They are clearer, faster and generate better schemas.
7. **Reuse `TypeAdapter`.** Adapter construction is schema compilation work.
8. **Separate public output models from sensitive internal models.** Do not rely only on exclusion flags.
9. **Keep validators and serializers deterministic and side-effect free.** I/O belongs in service/application layers.
10. **Use V2 APIs in V2 code.** Keep V1 compatibility explicitly under `pydantic.v1` during migration.
11. **Pin new/experimental behavior.** Partial validation, MISSING and prerelease 2.14 behavior are not generic stable assumptions.
12. **Test both Python and JSON entry points when custom validation depends on mode/context.**
13. **Test validation and serialization schemas separately when consumers depend on them.**
14. **Pydantic validates structure; application logic still owns authorization, resource limits and business semantics.**
15. **For stable production code as of this reference, target Pydantic 2.13.4—not 2.14 prerelease behavior.**

---

# Reference source URLs

Core stable documentation:

1. https://pydantic.dev/docs/validation/latest/get-started/
2. https://pydantic.dev/docs/validation/latest/get-started/install/
3. https://pydantic.dev/docs/validation/latest/concepts/models/
4. https://pydantic.dev/docs/validation/latest/concepts/fields/
5. https://pydantic.dev/docs/validation/latest/concepts/alias/
6. https://pydantic.dev/docs/validation/latest/api/pydantic/config/
7. https://pydantic.dev/docs/validation/latest/concepts/validators/
8. https://pydantic.dev/docs/validation/latest/concepts/serialization/
9. https://pydantic.dev/docs/validation/latest/concepts/strict_mode/
10. https://pydantic.dev/docs/validation/latest/concepts/types/
11. https://pydantic.dev/docs/validation/latest/concepts/unions/
12. https://pydantic.dev/docs/validation/latest/concepts/dataclasses/
13. https://pydantic.dev/docs/validation/latest/concepts/forward_annotations/
14. https://pydantic.dev/docs/validation/latest/concepts/json/
15. https://pydantic.dev/docs/validation/latest/concepts/json_schema/
16. https://pydantic.dev/docs/validation/latest/concepts/performance/
17. https://pydantic.dev/docs/validation/latest/concepts/experimental/
18. https://pydantic.dev/docs/validation/latest/api/pydantic/base_model/
19. https://pydantic.dev/docs/validation/latest/api/pydantic/type_adapter/
20. https://pydantic.dev/docs/validation/latest/api/pydantic/types/
21. https://pydantic.dev/docs/validation/latest/api/pydantic/standard_library_types/
22. https://pydantic.dev/docs/validation/latest/get-started/migration/
23. https://pydantic.dev/docs/validation/latest/get-started/changelog/
24. https://pydantic.dev/docs/validation/latest/get-started/version-policy/
25. https://pydantic.dev/articles/pydantic-v2-13-release
26. https://pypi.org/project/pydantic/

Companion settings sources:

27. https://pydantic.dev/docs/validation/latest/concepts/pydantic_settings/
28. https://pypi.org/project/pydantic-settings/

Development/tooling sources:

29. https://pydantic.dev/docs/validation/latest/integrations/dev-tools/mypy/
30. https://pydantic.dev/docs/validation/latest/integrations/dev-tools/datamodel_code_generator/
31. https://pydantic.dev/docs/validation/latest/internals/architecture/
32. https://pydantic.dev/docs/validation/latest/internals/resolving_annotations/

Prerelase boundary source:

33. https://pypi.org/project/pydantic/2.14.0b1/

