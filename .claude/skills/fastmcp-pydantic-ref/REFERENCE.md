# FastMCP + Pydantic — Reference Companion

Companion to `SKILL.md` in this folder. SKILL.md is the map you read first; this file is what you
come back to once you know which document you need. Both target `docs/library_ref/`:

* **`fastmcp`** = `fastmcp_python_advanced_reference_3.4.7.md` — 15,682 lines, §0-§37
* **`pydantic`** = `pydantic_python_advanced_reference_2.13.4.md` — 7,340 lines, §0-§51

Citations are `fastmcp §N.M` / `pydantic §N.M`, matching the documents' own numbering. Line numbers
appear only in §1, because line numbers move when a document is regenerated and section numbers do
not — seek by line, cite by section, and if a line looks wrong re-derive §1 with `lib-outline`.

| Section | What it is | Reach for it when |
|---|---|---|
| **§1** | Chapter and appendix maps, with line numbers and subsection depth | you have a section number, or you need to know where to `Read` |
| **§2** | **Symbol → canonical location**, ~230 public API names | you have a name and need the definition |
| **§3** | Task → location, phrased as goals | you have a goal and no name |
| **§4** | Ten decision trees | you are choosing between library options |
| **§5** | The `fastmcp` ↔ `pydantic` seam | the answer might be in either document |
| **§6** | Fourteen navigation rules | before searching either file |

---

## §1 — Document maps

### §1.1 `fastmcp` — 38 chapters

Front matter: title (1) · **"Proposed comprehensive documentation map"** (85) · "Stable release
delta — what changed after FastMCP 3.0" (245) · "Source index used throughout this reference" (262).
Deep-dive chapters start at 306. There is **no end-of-reference marker**; §37 runs to EOF.

The **Depth** column is load-bearing: it says whether `lib-outline --view expanded` will show you
that chapter's subsections. `**###**` means it shows *nothing*; `##` + `###`×k means it shows all
but k of them (almost always just `N.0`). See Rule 2.

| § | Line | Lines | Depth | Subs | Title |
|---|---:|---:|---|---|---|
| **§0** | 306 | 213 | **`###`** | 0.1-0.8 | Scope, versioning, and mental model |
| **§1** | 519 | 495 | `##` | 1.0-1.16 | Installation, package selection, dependency policy, and project layout |
| **§2** | 1014 | 633 | `##` | 2.0-2.22 | First executable server, client, and test |
| **§3** | 1647 | 336 | `##` + `###`×1 | 3.0-3.7 | Core API map and object model |
| **§4** | 1983 | 324 | `##` + `###`×1 | 4.0-4.12 | Server construction and lifecycle |
| **§5** | 2307 | 290 | **`###`** | 5.0-5.13 | Tools: definition, registration, and execution contract |
| **§6** | 2597 | 326 | **`###`** | 6.0-6.14 | Tools: typing, validation, hidden parameters, outputs, and content blocks |
| **§7** | 2923 | 385 | **`###`** | 7.0-7.17 | Resources and resource templates |
| **§8** | 3308 | 303 | **`###`** | 8.0-8.13 | Prompts and prompt rendering |
| **§9** | 3611 | 408 | **`###`** | 9.0-9.18 | MCP Context |
| **§10** | 4019 | 313 | **`###`** | 10.0-10.16 | Dependency injection |
| **§11** | 4332 | 489 | `##` | 11.0-11.23 | Lifespans, session state, storage, and state ownership |
| **§12** | 4821 | 247 | **`###`** | 12.0-12.13 | Background tasks and long-running workflows |
| **§13** | 5068 | 443 | **`###`** | 13.0-13.25 | Middleware and the server policy layer |
| **§14** | 5511 | 392 | **`###`** | 14.0-14.24 | Providers and dynamic component sources |
| **§15** | 5903 | 499 | `##` | 15.0-15.26 | Transforms, visibility, versioning, pagination, and discovery shaping |
| **§16** | 6402 | 416 | `##` | 16.0-16.24 | Search transforms, Code Mode, composition, proxying, and gateways |
| **§17** | 6818 | 416 | `##` + `###`×1 | 17.0-17.2 | Authentication and authorization |
| **§18** | 7234 | 481 | `##` | 18.0-18.24 | Advanced security policy and identity-aware execution |
| **§19** | 7715 | 379 | **`###`** | 19.0-19.11 | Running and deploying servers |
| **§20** | 8094 | 494 | `##` | 20.0-20.26 | HTTP hardening, reverse proxies, scaling, and event delivery |
| **§21** | 8588 | 345 | **`###`** | 21.0-21.12 | Programmatic client fundamentals |
| **§22** | 8933 | 689 | `##` + `###`×2 | 22.0-22.18 | Client transports, handlers, roots, and client-side auth |
| **§23** | 9622 | 327 | `##` | 23.0-23.19 | Client-only packaging and `fastmcp-remote` |
| **§24** | 9949 | 337 | **`###`** | 24.0-24.17 | Apps and interactive UI delivery |
| **§25** | 10286 | 390 | `##` + `###`×1 | 25.0-25.13 | Prefab, built-in app providers, Generative UI, and custom renderers |
| **§26** | 10676 | 422 | `##` + `###`×1 | 26.0-26.15 | OpenAPI and FastAPI integration |
| **§27** | 11098 | 361 | `##` + `###`×8 | 27.0-27.14 | Project configuration, settings, and portable deployment contracts |
| **§28** | 11459 | 310 | `##` + `###`×1 | 28.0-28.15 | CLI and developer workflows |
| **§29** | 11769 | 194 | **`###`** | 29.0-29.11 | Observability, inspection, telemetry, and operational diagnostics |
| **§30** | 11963 | 452 | `##` + `###`×1 | 30.0-30.20 | Testing, contract verification, and tool fingerprinting |
| **§31** | 12415 | 301 | `##` + `###`×1 | 31.0-31.17 | Ecosystem and host integrations |
| **§32** | 12716 | 463 | `##` + `###`×1 | 32.0-32.24 | Security hardening and governance |
| **§33** | 13179 | 464 | `##` + `###`×1 | 33.0-33.23 | Performance, scaling, resilience, and large-catalog engineering |
| **§34** | 13643 | 445 | `##` + `###`×1 | 34.0-34.18 | Production architecture patterns |
| **§35** | 14088 | 405 | `##` + `###`×1 | 35.0-35.24 | API stability, upgrade discipline, and FastMCP 2 → 3 migration |
| **§36** | 14493 | 443 | `##` + `###`×1 | 36.0-36.22 | FastMCP 4 prerelease transition guide |
| **§37** | 14936 | 747 | `##` letters | A)-AJ) | Dense appendices and lookup matrices |

Three chapters where the depth column understates the problem:

* **§17** — only `17.0`, `17.1`, `17.2` are numbered at that level. Every symbol lives one level
  further down, at `### 17.1.1`-`### 17.2.9`: `TokenVerifier` (6833) · `RemoteAuthProvider` (6854) ·
  `OAuthProxy` (6884) · `OIDCProxy` (6920) · `OAuthProvider` (6941) · `MultiAuth` (6968) ·
  decision framework (7006) · `require_scopes(...)` (7024) · `restrict_tag(...)` (7045) ·
  AND-composition (7072) · custom/async checks (7093) · component-level authz (7117) ·
  `AuthMiddleware` (7142) · access-token-aware tools (7172).
* **§27** — `27.0`-`27.7` are `###` (the entire `fastmcp.json` schema: `source` 11140, `environment`
  11158, `deployment` 11185, JSON-schema support 11213, CLI override precedence 11229, auto-detection
  11243); only `27.8`-`27.14`, the CLI commands, are visible at `##`.
* **§22** — `22.0` and `22.1` are `###`; `22.2`-`22.18` are `##`. Also carries a third level:
  `22.2.1`-`22.2.3` (StdioTransport environment/path/session rules, 8975-9010), `22.3.1` (TLS,
  9054), `22.8.1` (9213), `22.9.1`-`22.9.3` (OAuth parameters/flow/guidance, 9248-9293),
  `22.10.1` (CIMD document requirements, 9320).

**§3** also carries a full third level — the object-model breakdown at `3.1.1`-`3.6.3`
(1671-1947), which is where `FastMCP`'s `local_provider` (1703), provider aggregation order (1728),
transform placement levels (1782), and `mount`/`import_server`/`create_proxy` (1935/1941/1947) are
actually defined.

### §1.2 `pydantic` — 52 chapters

Front matter: title (1) · **"Proposed comprehensive documentation map"** (76) · "Stable release
delta — why 2.13.4 deserves a new reference" (133) · "Source-index shorthand used in the prose"
(152). Deep-dive chapters start at 174. Tail: `# Reference source URLs` (7294).

Depth is **uniform `## N.M`** in every chapter, so `--view expanded` is complete and trustworthy
here. §51 is the only chapter that numbers from `.1` rather than `.0`.

| § | Line | Lines | Depth | Subs | Title |
|---|---:|---:|---|---|---|
| **§0** | 174 | 157 | `##` | 0.0-0.7 | Scope, versioning, and mental model |
| **§1** | 331 | 152 | `##` | 1.0-1.7 | Installation, dependencies, extras, version pinning, and project layout |
| **§2** | 483 | 114 | `##` | 2.0-2.6 | First executable validation/serialization application |
| **§3** | 597 | 140 | `##` | 3.0-3.8 | Architecture: Python annotations → CoreSchema → Rust validator/serializer |
| **§4** | 737 | 125 | `##` | 4.0-4.8 | `BaseModel` definition and object model |
| **§5** | 862 | 126 | `##` | 5.0-5.8 | Validation entry points: `__init__`, `model_validate`, JSON and strings |
| **§6** | 988 | 128 | `##` | 6.0-6.8 | Trusted construction, copying, equality, extras, field-set tracking, and private state |
| **§7** | 1116 | 169 | `##` | 7.0-7.9 | Fields, `FieldInfo`, `Annotated`, metadata, constraints, and signatures |
| **§8** | 1285 | 102 | `##` | 8.0-8.8 | Defaults, `default_factory`, validated data, and default validation |
| **§9** | 1387 | 234 | `##` | 9.0-9.13 | `ConfigDict`: complete configuration model |
| **§10** | 1621 | 123 | `##` | 10.0-10.10 | Strict mode, lax coercion, and the conversion contract |
| **§11** | 1744 | 127 | `##` | 11.0-11.10 | Constraints, reusable annotated types, and constrained-type design |
| **§12** | 1871 | 152 | `##` | 12.0-12.10 | Aliases, validation aliases, serialization aliases, paths, choices, and generators |
| **§13** | 2023 | 146 | `##` | 13.0-13.9 | Field validators: before, after, plain, and wrap |
| **§14** | 2169 | 109 | `##` | 14.0-14.8 | Model validators, `ValidationInfo`, context, ordering, and inheritance |
| **§15** | 2278 | 108 | `##` | 15.0-15.7 | Functional validator metadata: `BeforeValidator`, `AfterValidator`, `WrapValidator`, `ValidateAs`, and related helpers |
| **§16** | 2386 | 96 | `##` | 16.0-16.8 | Serialization fundamentals: `model_dump` and `model_dump_json` |
| **§17** | 2482 | 122 | `##` | 17.0-17.8 | Field serializers, model serializers, functional serializers, and serialization context |
| **§18** | 2604 | 81 | `##` | 18.0-18.8 | Include/exclude semantics, `exclude_if`, unset/default/none/computed handling |
| **§19** | 2685 | 99 | `##` | 19.0-19.6 | Subclass and polymorphic serialization, `SerializeAsAny`, and external-contract safety |
| **§20** | 2784 | 87 | `##` | 20.0-20.7 | Computed fields, private attributes, properties, and model lifecycle hooks |
| **§21** | 2871 | 149 | `##` | 21.0-21.10 | `TypeAdapter`: arbitrary-type validation, serialization, JSON Schema, and reuse |
| **§22** | 3020 | 73 | `##` | 22.0-22.7 | `RootModel` |
| **§23** | 3093 | 94 | `##` | 23.0-23.7 | Pydantic dataclasses |
| **§24** | 3187 | 85 | `##` | 24.0-24.6 | `TypedDict`, standard-library dataclasses, `NamedTuple`, and model-like types |
| **§25** | 3272 | 80 | `##` | 25.0-25.8 | Generic models, type variables, specialization, and PEP 695 syntax |
| **§26** | 3352 | 97 | `##` | 26.0-26.9 | Unions: smart mode, left-to-right, discriminators, callable discriminators, and errors |
| **§27** | 3449 | 84 | `##` | 27.0-27.6 | Forward annotations, recursive models, cyclic input, and namespace resolution |
| **§28** | 3533 | 82 | `##` | 28.0-28.6 | Dynamic models, `create_model`, `model_rebuild`, and runtime schema composition |
| **§29** | 3615 | 101 | `##` | 29.0-29.8 | Custom types, `CoreSchema`, `__get_pydantic_core_schema__`, and annotated handlers |
| **§30** | 3716 | 100 | `##` | 30.0-30.10 | Built-in and standard-library type validation |
| **§31** | 3816 | 130 | `##` | 31.0-31.10 | Pydantic-specific types, secrets, encoded data, constraints, and `FailFast` |
| **§32** | 3946 | 118 | `##` | 32.0-32.9 | Network, URL, DSN, email, IP, UUID, path, and filesystem-oriented types |
| **§33** | 4064 | 109 | `##` | 33.0-33.9 | JSON parsing, `jiter`, string caching, and partial validation |
| **§34** | 4173 | 109 | `##` | 34.0-34.9 | JSON Schema fundamentals and validation-vs-serialization schemas |
| **§35** | 4282 | 82 | `##` | 35.0-35.8 | Advanced JSON Schema customization and `GenerateJsonSchema` |
| **§36** | 4364 | 116 | `##` | 36.0-36.9 | Errors: `ValidationError`, custom errors, locations, causes, and usage errors |
| **§37** | 4480 | 88 | `##` | 37.0-37.7 | `@validate_call`: validation of ordinary function calls |
| **§38** | 4568 | 130 | `##` | 38.0-38.8 | `pydantic-settings` 2.15.0 fundamentals and source priority |
| **§39** | 4698 | 119 | `##` | 39.0-39.10 | Advanced settings: nested env, dotenv, secrets, CLI, cloud secret managers, and custom sources |
| **§40** | 4817 | 154 | `##` | 40.0-40.14 | Performance, build-time cost, memory, validation hot paths, and `FailFast` |
| **§41** | 4971 | 73 | `##` | 41.0-41.5 | Experimental APIs and stability boundaries |
| **§42** | 5044 | 111 | `##` | 42.0-42.7 | Static typing, Mypy, Pyrefly, IDEs, Hypothesis, and code generation |
| **§43** | 5155 | 94 | `##` | 43.0-43.6 | Observability and validation instrumentation |
| **§44** | 5249 | 98 | `##` | 44.0-44.7 | Framework and persistence integration boundaries |
| **§45** | 5347 | 152 | `##` | 45.0-45.12 | Pydantic V1 compatibility and V1 → V2 migration |
| **§46** | 5499 | 97 | `##` | 46.0-46.10 | Stable release delta: Pydantic 2.12 → 2.13.4 |
| **§47** | 5596 | 74 | `##` | 47.0-47.6 | Pydantic 2.14 prerelease transition and Python-version boundary |
| **§48** | 5670 | 132 | `##` | 48.0-48.10 | Testing, schema snapshots, round-trip checks, fuzzing, and compatibility contracts |
| **§49** | 5802 | 124 | `##` | 49.0-49.12 | Security, secrets, untrusted input, serialization exposure, and trust boundaries |
| **§50** | 5926 | 151 | `##` | 50.0-50.10 | Production architecture patterns |
| **§51** | 6077 | 1264 | `##` | 51.1-51.60 | Dense appendices and lookup matrices |

### §1.3 `fastmcp` §37 — 35 lettered lookup matrices (14936-EOF)

Letters run **`A)`-`J)`, then `L)`-`Z)`, then `AA)`-`AJ)` — `K)` does not exist.** This is the
intended fast-lookup layer; the letters are opaque, so use this table rather than opening the
chapter. Note the appendix is really **two overlapping series**: `A)`-`J)` were written against the
early chapters, `L)`-`AJ)` against the later ones, so transport appears at both `F)` and `O)`,
client auth at both `G)` and `Z)`, and providers at both `D)` and `V)`. Read both when they collide.

| Letter | Line | Answers |
|---|---:|---|
| **A)** | 14943 | Decorator-argument matrix — every argument `@mcp.tool`/`@mcp.resource`/`@mcp.prompt` accepts, plus auto-inference rules |
| **B)** | 14978 | Dependency-injection matrix — which DI primitive for which runtime value, and the rules worth memorising |
| **C)** | 15006 | Task-mode truth table — when a call runs inline vs as a background task |
| **D)** | 15027 | Provider comparison grid |
| **E)** | 15051 | Transform comparison grid, plus `add_transform(...)` vs `wrap_transform(...)` |
| **F)** | 15079 | Client transport decision matrix |
| **G)** | 15093 | Client auth decision matrix |
| **H)** | 15107 | **Output-schema and content-block conversion rules** — server-side conversion matrix (`H.1`) and the client-side result-object cheat sheet (`H.2`) |
| **I)** | 15138 | Deployment mounting invariants, with fast-path examples |
| **J)** | 15161 | **Fast lookup rules** — the document's own "what knob do I need?" table; the single best first stop |
| **L)** | 15190 | Stable version-gate matrix — which capability arrived in which release |
| **M)** | 15215 | Package / extra selection matrix (`fastmcp` vs `fastmcp-slim` vs `fastmcp-remote`) |
| **N)** | 15232 | Server-definition vs deployment matrix — what belongs in the constructor vs the run step |
| **O)** | 15246 | Transport decision matrix |
| **P)** | 15259 | HTTP deployment invariants |
| **Q)** | 15278 | Tool authoring quick matrix |
| **R)** | 15301 | Tool-result conversion quick matrix — what a return value becomes on the wire |
| **S)** | 15318 | Resource selection matrix |
| **T)** | 15341 | Prompt quick matrix |
| **U)** | 15356 | **`Context` capability matrix — stable v3**; the compact answer to "what can `ctx` do?" |
| **V)** | 15373 | Provider / transform decision matrix |
| **W)** | 15396 | Visibility / version / authorization matrix |
| **X)** | 15419 | Authentication decision matrix — the compact form of §17.1 |
| **Y)** | 15435 | Client construction matrix |
| **Z)** | 15458 | Client auth matrix |
| **AA)** | 15473 | Apps / provider quick matrix |
| **AB)** | 15490 | OpenAPI / FastAPI generation matrix |
| **AC)** | 15506 | CLI workflow matrix |
| **AD)** | 15525 | Contract-testing matrix |
| **AE)** | 15551 | Security checklist — condensed |
| **AF)** | 15575 | Performance/scaling checklist — condensed |
| **AG)** | 15595 | **V2 → V3 migration grep sheet** — literal strings to search for in v2 code |
| **AH)** | 15620 | Stable v3 vs FastMCP 4 beta boundary |
| **AI)** | 15639 | Production architecture quick chooser |
| **AJ)** | 15656 | **Source-of-truth hierarchy for LLM coding agents** — read this before trusting any example, here or on the web |

### §1.4 `pydantic` §51 — 60 appendix subsections (6077-7293)

1,264 lines, 17% of the document, numbered `51.1`-`51.60` (no `51.0`). This is the exact-signature
and matrix layer: for "what is the precise 2.13.4 surface of X?" or "which option do I want?",
**come here before the prose chapter**. Four bands:

**Version and install** — `51.1` stable version matrix (6081) · `51.2` installation matrix (6091).

**Exact signatures** (6115-6356) — `51.3` `BaseModel` primary validation signatures (6115) ·
`51.4` `model_rebuild()` (6148) · `51.5` `model_copy()` (6170) · `51.6` **`model_dump()` exact
surface** + option quick map (6188) · `51.7` `model_dump_json()` (6230) · `51.8` validation vs
construction decision (6255) · `51.9` `TypeAdapter` constructor (6268) · `51.10`
`TypeAdapter.validate_python()` (6282) · `51.11` `.validate_json()` (6302) · `51.12`
`.dump_python()` (6319) · `51.13` `TypeAdapter` method matrix (6343).

**Decision matrices** (6357-6931) — `51.14` **`Field()` parameter-category matrix** (6357, with
eight `###` category blocks) · `51.15` field declaration decision table (6440) · `51.16`
**`ConfigDict` complete 2.13.4 attribute inventory** (6455, plus deprecated-configuration notes) ·
`51.17` five ready-made `ConfigDict` profiles (6526) · `51.18` extra-field (6579) · `51.19`
strictness (6589) · `51.20` alias (6600) · `51.21` validator mode (6615) · `51.22` field vs model
validator (6624) · `51.23` validator error (6646) · `51.24` serialization-mode (6656) · `51.25`
**exclusion matrix** (6668) · `51.26` polymorphic serialization (6681) · `51.27` **`TypeAdapter` vs
`RootModel` vs `BaseModel`** (6690) · `51.28` union decision (6700) · `51.29` customization
escalation (6710) · `51.30` JSON validation decision table (6724) · `51.31` JSON Schema generation
(6734) · `51.32` error-detail lookup (6747) · `51.33` **common validation error-code categories**
(6764) · `51.34` strict vs lax API design (6790) · `51.35` standard type quick matrix (6801) ·
`51.36` network type quick matrix (6815) · `51.37` secret-handling rules (6827) · `51.38`
**`pydantic-settings` source map** (6842) · `51.39` settings extras 2.15.0 (6862) · `51.40` settings
environment controls (6872) · `51.41` settings security checklist (6892) · `51.42` performance rules
condensed (6905) · `51.43` performance architecture matrix (6919).

**Migration and checkpoints** (6932-7033) — `51.44` V1→V2 rename matrix (6932) · `51.45` V1
optionality trap (6953) · `51.46` V2 equality trap (6965) · `51.47` V2 serialization trap (6975) ·
`51.48` release 2.13.4 checkpoint (6979) · `51.49` 2.14 prerelease checkpoint (6989) · `51.50` model
contract design checklist (6998) · `51.51` agent anti-pattern checklist (7014).

**Cookbooks** (7034-7293) — copy-ready patterns, each with `###` sub-recipes. `51.52` **validation
boundary** (7034: exact external JSON · human-friendly config · extensible event envelope · ORM
projection · patch semantics) · `51.53` **serializer** (7089: always hide · conditionally hide
`None` · custom format · context redaction · public subclass safety) · `51.54` union (7128: tagged
variants · ordered coercion) · `51.55` custom type (7152: simple constraint · normalization ·
validate via intermediary · full hook) · `51.56` schema contract (7185: validation schema ·
serialization schema · arbitrary type · vendor extension · global generator policy) · `51.57` error
translation (7218) · `51.58` upgrade discipline checklist (7238) · `51.59` source-of-truth map by
question (7258) · `51.60` **final agent invariants** (7274, fifteen numbered).

---

## §2 — Symbol → canonical location

**Use this instead of grep.** Both documents use their own public API names constantly in examples,
so a literal search returns dozens of hits with no signal about which one is the definition:
`Context` appears **201** times in `fastmcp`, `TypeAdapter` **103** times in `pydantic`, `Depends`
51, `ToolResult` 40, `model_validate_json` 22. Every row below points at the subsection that
*defines* the symbol; the **Also** column lists the other places worth reading.

### §2.1 `pydantic`

**Model surface**

| Symbol | Defined at | Also |
|---|---|---|
| `BaseModel` | §4.0 | §4.6 method table · §51.3 signatures |
| `model_validate(...)` | §5.2 | §5.0 entry-point matrix · §51.3 · §51.8 |
| `model_validate_json(...)` | §5.4 | §33.0-§33.1 why it is faster · §33.7 the 2.13 fix · §51.3 |
| `model_validate_strings(...)` | §5.5 | §5.0 |
| `model_construct(...)` | §6.0 | §51.8 validation-vs-construction · §49 |
| `model_copy(...)` | §6.1 | §51.5 exact surface |
| `model_dump(...)` | §16.1 option map | §51.6 **exact 2.13.4 surface** · §18 exclusion semantics |
| `model_dump_json(...)` | §16.0 | §51.7 exact surface |
| `model_json_schema(...)` | §34.0 | §34.2 validation vs serialization mode · §51.31 |
| `model_rebuild()` | §27.2 | §51.4 · §3.4 why it is needed · §28.5 |
| `model_post_init` | §20.5 | §20.6 custom `__init__` |
| `model_fields` | §4.1 | §7.5 introspection (instance access is deprecated) |
| `model_fields_set` | §6.2 | §8.6 default-vs-unset · §18.3 |
| `model_computed_fields` | §4.1 | §20.0 |
| `__pydantic_extra__` | §4.2 | §6.3 `extra` modes · §6.4 typed extras |
| `__pydantic_private__` | §4.2 | §6.5 · §20.4 |
| `__pydantic_core_schema__` · `__pydantic_validator__` · `__pydantic_serializer__` | §4.1 | §3.2 · §3.3 · §3.8 debugging |
| `create_model(...)` | §28.0 | §28.4 build cost · §28.6 security · §49.11 |

**Fields, defaults, aliases**

| Symbol | Defined at | Also |
|---|---|---|
| `Field(...)` | §7.0 (styles), §7.3 (parameter list) | §51.14 **parameter-category matrix** · §51.15 |
| `FieldInfo` | §7.5 | §7.3 |
| `Annotated[...]` | §7.1 | §11.0 reusable types · §15.0 functional metadata · §11.9 ordering |
| `default_factory` | §8.2 | §8.3 **validated-data form** · §8.5 · §4.4 |
| `validate_default` | §8.1 | §8.7 enum defaults |
| `exclude_if` | §7.7 | §18.2 · §20.2 computed fields (new in 2.13) |
| `deprecated` | §7.8 | — |
| `computed_field` | §20.0 | §20.1 alias/title · §20.2 · §18.6 |
| `PrivateAttr(...)` | §6.5 | §20.4 · §8.8 validated-data factories (2.13) |
| `alias` · `validation_alias` · `serialization_alias` | §12.0 | §12.5 precedence · §51.20 alias matrix |
| `AliasPath(...)` | §12.1 | — |
| `AliasChoices(...)` | §12.2 | §38.4 settings aliases |
| `AliasGenerator(...)` · `alias_generator` | §12.3-§12.4 | `pydantic.alias_generators.to_camel`/`to_pascal` §12.3-§12.4 |
| `alias_priority` | §12.5 | — |
| `loc_by_alias` | §12.8 | §36.1 error locations |

**Configuration** — `ConfigDict` as a whole is §9; the **complete 2.13.4 attribute inventory** is §51.16 and five ready-made profiles are §51.17.

| Setting | Defined at | Also |
|---|---|---|
| `extra` | §6.3, §9.2 | §51.18 matrix · §49.1 |
| `strict` | §9.3 | §10 whole chapter · §51.19 · §51.34 |
| `frozen` | §6.7 | §9.1 |
| `validate_assignment` | §9.4 | — |
| `from_attributes` | §5.3, §9.5 | §44.2 ORM boundary |
| `defer_build` | §9.7 | §40.9 · §3.5 |
| `cache_strings` | §9.8 | §33.4 |
| `regex_engine` | §9.9 | §49.7 |
| `hide_input_in_errors` | §9.10 | §36.7 · §49.9 |
| `polymorphic_serialization` | §9.11 | §19.3 (new in 2.13) · §51.26 |
| `validate_by_alias` · `validate_by_name` · `serialize_by_alias` | §9.6 | §12.6-§12.7 · §51.20 |
| `revalidate_instances` · `arbitrary_types_allowed` · `protected_namespaces` · `ignored_types` | §9.1 category map | §9.13 anti-patterns |
| `use_enum_values` | §8.7 | §30.7 |
| `str_to_lower` · `str_strip_whitespace` · `str_min_length` … | §9.1 (string normalization) | §11.2 prefer `StringConstraints` |
| `ser_json_temporal` · `ser_json_bytes` · `ser_json_inf_nan` … | §9.1 (serialization) | §30.3 |

**Validators**

| Symbol | Defined at | Also |
|---|---|---|
| `field_validator` | §13.0 | §13.1-§13.4 the four modes · §51.21 · §51.22 |
| `model_validator` | §14.0 | §14.1 before form · §14.2 wrap form · §14.6 ordering · §14.7 inheritance · §51.22 |
| `BeforeValidator` · `AfterValidator` · `WrapValidator` · `PlainValidator` | §15.1 | §15.0 via `Annotated` · §13.1-§13.4 for decorator semantics |
| `ValidationInfo` | §14.3 | §13.6 `.data` and field ordering · §5.6 context |
| validation `context=` | §5.6 | §14.4 · §14.5 constructor limitation |
| `InstanceOf` | §15.2 | §29.0 escalation ladder |
| `SkipValidation` | §15.3 | — |
| `ValidateAs` | §15.4 | §29.0 |
| `PydanticUseDefault` | §15.5 | — |
| `OnErrorOmit` | §15.6 | §31.8 |
| `@validate_call` | §37.0 | §37.3 return validation · §37.6 performance · §37.7 stability |

**Serialization**

| Symbol | Defined at | Also |
|---|---|---|
| `field_serializer` | §17.0 | §17.1 plain vs wrap · §17.2 signatures |
| `model_serializer` | §17.3 | §17.4 wrap form |
| `PlainSerializer` | §17.5 | §7.1 stacking in `Annotated` (`WrapSerializer` is named only in the §29.0 ladder) |
| `SerializationInfo` · serialization `context=` | §17.6 | §51.53 context-redaction recipe |
| `SerializeAsAny` | §19.1 | §19.2 runtime `serialize_as_any` · §19.6 security example |
| `exclude_unset` | §18.3 | §6.2 · §8.6 · §18.8 patch example |
| `exclude_defaults` | §18.4 | §51.25 exclusion matrix |
| `exclude_none` | §18.5 | §51.25 |
| `exclude_computed_fields` | §18.6 | §18.7 precedence |
| `round_trip=True` | §16.3 | §48.4 round-trip test |
| `fallback=` | §16.4 | §16.5 warnings |

**Types**

| Symbol | Defined at | Also |
|---|---|---|
| `TypeAdapter` | §21.0-§21.1 | §21.2-§21.5 methods · §21.6 **instantiate once** · §51.9-§51.13 signatures · §51.27 |
| `RootModel` | §22.0 | §22.3 vs `TypeAdapter` · §22.5 2.13 patches · §51.27 |
| `from pydantic.dataclasses import dataclass` | §23.0 | §23.2 missing model methods · §23.4 extra behavior · §23.7 decision |
| `TypedDict` | §24.0 | §24.1 why it can be faster · §24.2 config · §40.6 |
| `NamedTuple` · stdlib `dataclass` | §24.3-§24.4 | §24.6 decision table |
| `Strict()` | §10.3 | §10.2 field level · §10.4 model level |
| `StringConstraints(...)` | §11.2 | `ascii_only` is new in 2.13 (§46.2) |
| `annotated_types` — `Ge`, `Le`, `MinLen`, `MaxLen` | §11.1 | §11.4-§11.6 |
| `SecretStr` · `SecretBytes` | §31.1 | §49.2 · §51.37 · §47.5 comparison semantics in 2.14 |
| `Json[T]` | §31.2 | §33 JSON chapter |
| `ImportString` | §31.4 | §49.6 **security** |
| `ByteSize` | §31.5 | — |
| `FailFast` | §31.7 | §40.8 |
| `MISSING` | §31.9 | §41.2 (experimental) · §6.8 |
| `FiniteFloat` and strict scalars | §31.6 | §30.1 |
| `AnyUrl` and URL types | §32.0 | §32.1 constraints · §32.8 normalization · §51.36 |
| DSN types | §32.2 | §32.3 multi-host · §32.9 credentials in URLs |
| `EmailStr` · `NameEmail` | §32.4 | §1.1 needs the `[email]` extra |
| `Discriminator` · `Tag` | §26.5 | §26.3 literal discriminators · §51.28 |

**Schema, errors, settings**

| Symbol | Defined at | Also |
|---|---|---|
| `CoreSchema` | §3.1 | §29.7 direct pydantic-core use · §29.8 stability warning |
| `__get_pydantic_core_schema__` | §29.1 | §29.0 escalation ladder · §29.2 handler discipline |
| `__get_pydantic_json_schema__` | §29.5 | §35.2 |
| `GetPydanticSchema` | §29.4 | — |
| `WithJsonSchema` | §35.1 | §7.1 |
| `GenerateJsonSchema` | §35.3 | §35.4 ref templates · §35.8 global-customization risk |
| `json_schema_extra` | §34.5 | §35.0 |
| `$defs` / references | §34.3 | §35.4 |
| `ValidationError` · `.errors()` | §36.0 | §36.2 **do not parse the string form** · §51.32 · §51.33 error codes |
| `PydanticCustomError` | §36.3 | §36.4 `ValueError` vs `TypeError` |
| `PydanticUserError` | §36.5 | — |
| `BaseSettings` · `SettingsConfigDict` | §38.1 | §38.0 separate package · §38.8 lifetime |
| settings source priority | §38.2 | §39.1 priority design · §51.38 **source map** |
| `settings_customise_sources` | §39.0 | §39.2 custom source |
| `.env` / dotenv | §38.7 | §39.4 file secrets · §51.40 |
| CLI settings integration | §39.3 | §51.39 extras — the reference describes the capability but never names the class; get it from the installed package |
| cloud secret managers · TOML/YAML sources | §39.6-§39.7 | §51.39 |

> Not in this document: `JsonValue` is never mentioned. If you need it, the installed package is the
> only authority — see `pydantic` §0.0's confidence hierarchy.

### §2.2 `fastmcp`

**Server object and constructor** — the constructor's four groups are `identity` / `composition` /
`behavior` / `handlers+storage` (§3.1.2, §4.0). Each individual behavior field has its own
*unnumbered* `###` block inside §4.4, which is why they are unfindable by outline.

| Symbol | Defined at | Also |
|---|---|---|
| `FastMCP(...)` | §3.1, §4.0-§4.5 | §3.1.3 owns a `local_provider` · §3.1.6 aggregation order · §4.1 the "always set these" rule |
| `name` · `instructions` · `version` · `website_url` · `icons` | §4.2 identity fields | §3.1.2 |
| `tools` · `auth` · `middleware` · `providers` · `transforms` · `lifespan` | §4.3 composition fields | §3.1.2 |
| `on_duplicate` | §4.4 | §35.4 v2→v3 duplicate-policy consolidation |
| `strict_input_validation` | §4.4 | **§6.3 flexible vs strict** — the semantic half |
| `mask_error_details` | §4.4 | §18.20 · §32.19 information disclosure |
| `list_page_size` | §4.4 | §15.16-§15.17 pagination |
| `tasks` | §4.4 | §12.1 · App. **C)** |
| `client_log_level` | §4.4 | §9.4 `ctx.log()` |
| `dereference_schemas` | §4.4 | §6.2 serve-time schema shaping · §33.17 its cost |
| `sampling_handler` · `sampling_handler_behavior` · `session_state_store` | §4.5 | §11.7 store boundary · §11.8-§11.9 · §22.12 the client-side handler |
| `run()` | §19.1 | §19.2 transport selection · App. **O)** |
| `http_app()` / ASGI export | §19.3 | §20.1 · §20.2 path composition · App. **I)** |
| `run_http_async()` | §19.4 | §19.10 ASGI-only knobs |
| `stateless_http` | §20.5 | §19.9 horizontal scaling · §33.12 |
| custom routes | §4.10 | §20.18 |
| `call_tool` · `read_resource` · `render_prompt` (server-side) | §3.1.4 | §8.11 definition lookup vs execution |
| `mount(...)` | §14.18, §3.6.1 | §15.3 `namespace=` · §16.10 vs import · §35.14 |
| `import_server(...)` | §3.6.2 (deprecated in v3) | §35.14 · §36.15 |
| `create_proxy(...)` / `FastMCPProxy` | §3.6.3, §14.19 | §16.11-§16.13 · §35.15 |

**Components**

| Symbol | Defined at | Also |
|---|---|---|
| `@mcp.tool` | §5.2 | §5.6 full decorator surface · App. **A)** · §35.8 v3 returns the original function |
| `mcp.add_tool(...)` | §5.3 | — |
| standalone `@tool()` | §5.4 | §35.9 method registration |
| `ToolResult` | §5.11, §6.11 | App. **R)** conversion matrix · §35.6 replaced the v2 tool serializer |
| `output_schema={...}` | §6.10 | §6.9 automatic generation from return annotations · App. **H)** |
| `Image` · `Audio` · `File` | §6.13 | §6.12 content-block conversion rules |
| `@mcp.resource(...)` | §7.1 | §7.3 `add_resource` · §7.4 standalone `@resource()` · App. **S)** |
| `ResourceResult` · `ResourceContent` | §7.6 | §7.5 return contract |
| resource templates / RFC 6570 | §7.12-§7.13 | §7.14 coercion · §7.15-§7.16 validation |
| `@mcp.prompt` | §8.1 | §8.3 decorator arguments · App. **T)** |
| `Message` | §8.7 | §8.6 return contract · §35.11 v2→v3 message-type migration |
| `PromptResult` | §8.8 | §8.9 static vs runtime metadata |

**`Context` and dependency injection**

| Symbol | Defined at | Also |
|---|---|---|
| `Context` | §9.0, §9.1 (three access patterns) | **App. `U)` capability matrix** · §9.17 vs ordinary Python · §9.16 when *not* to use it |
| `ctx.debug` · `ctx.info` · `ctx.warning` · `ctx.error` (and `log(...)`) | §9.4 | §4.4 `client_log_level` · §22.14 the client-side `log_handler` |
| `ctx.report_progress(...)` | §9.5 | §12.8 · §22.15 client-side `progress_handler` |
| resource / prompt access from a tool | §9.6-§9.7 | — |
| elicitation | §9.8 | §22.13 client-side handler · §28.7 through the CLI · §36.6 v4 changes |
| sampling | §9.9 | §22.12 client-side handler · §31.10 |
| session state | §9.10 | §11.5-§11.9 · §35.10 became async in v3 |
| per-session visibility | §9.11 | §15.11 |
| request metadata / session and request IDs | §9.12-§9.13 | §11.18 it is not lifespan state |
| `CurrentContext()` | §10.5 | §9.1 |
| `CurrentFastMCP()` | §10.6 | — |
| `CurrentRequest()` | §10.7 | HTTP-only |
| `CurrentAccessToken()` | §10.8 | §18.4 · §17.2.8 access-token-aware tools |
| `Depends(...)` | §10.9 | §10.10 nested · §10.11 caching · §10.12 async CM dependencies · App. **B)** |
| `Progress()` · `CurrentDocket()` · `CurrentWorker()` | §10.13 | §12 tasks |
| hidden-parameter contract | §10.2, §6.5 | §10.4 callers cannot override injected parameters |

**State, tasks, middleware**

| Symbol | Defined at | Also |
|---|---|---|
| `lifespan=` | §11.1 | §11.2 access from a tool · §11.3 unconditional cleanup · §11.4 ASGI composition · §20.3 forwarding |
| state ownership | §11.10 (table) | §11.13 DI vs lifespan · §11.11 never module globals · §34.13 by architecture |
| `TaskConfig` | §12.2 | §12.1 enabling · §12.3 poll interval · App. **C)** |
| task backends | §12.6 | §12.7 worker topology · §33.14 |
| `result()` · `status()` · `wait()` · `cancel()` · `on_status_change(...)` | §12.11 | §12.10 client-side task objects |
| `Middleware` · `add_middleware(...)` | §13.1 | §13.20 subclassing · §13.2 `call_next(context)` · §13.3 stack order |
| hooks: `on_message` · `on_request` · `on_call_tool` · `on_list_tools` … | §13.5 | §13.6 signature · §13.9 operation hooks · §13.11 raw `__call__` |
| `MiddlewareContext` | §13.6 | §13.22 component metadata · §13.23 storing state |
| `on_initialize` | §13.10 | carries a hard timing rule |
| built-in middleware | logging §13.13 · timing §13.14 · **caching §13.15** · rate limiting §13.16 · error handling §13.17 · retry §13.18 · ping §13.19 | §32.16 caching security · §33.15 overhead |

**Providers, transforms, discovery**

| Symbol | Defined at | Also |
|---|---|---|
| `Provider` | §3.2, §14.1 | §3.2.2 core methods · §3.2.3 built-in categories · App. **D)**/**V)** |
| `LocalProvider` | §14.2 | §3.1.3 |
| `FastMCPProvider` | §14.3 | §16.9 |
| `ProxyProvider` | §14.4 | §14.5 session models · §14.6 feature forwarding · §32.9 credential separation |
| `FileSystemProvider` | §14.7 | §31.12 |
| `OpenAPIProvider` | §26.2 | §26.3 default route mapping · §32.10 SSRF |
| `Transform` | §3.3, §14.10 | §3.3.2 placement levels · §15.1 provider vs server level · §15.19 order is API behavior |
| `add_transform(...)` vs `wrap_transform(...)` | §14.12 | App. **E)** |
| `Namespace(...)` | §14.13 | §15.2 · §16.15 |
| `ToolTransform(...)` | §14.14 | §15.4 · §15.5 transform vs wrapper tool · §35.16 |
| `ToolSearch(...)` | §14.15 | §16.1-§16.3 · §33.6 · §16.3 **search is not security** |
| `ResourcesAsTools` · `PromptsAsTools` | §14.16-§14.17 | §15.6-§15.7 |
| Code Mode | §16.4-§16.8 | §16.21 suitability matrix · §32.12 security · §33.7 |
| visibility / `only=True` / component keys | §15.8-§15.10 | §15.22 publication is not authorization · §18.7 · §32.4 |
| versioned components | §15.12-§15.14 | §15.15 versioning is not migration · §30.10 |

**Auth** — all six provider families live under §17's hidden third level (see §1.1).

| Symbol | Defined at | Also |
|---|---|---|
| `TokenVerifier` | §17.1.1 | §18.8 verifier invariants · App. **X)** |
| `RemoteAuthProvider` | §17.1.2 | — |
| `OAuthProxy` | §17.1.3 | §18.10 trust boundary · §32.7 |
| `OIDCProxy` | §17.1.4 | — |
| `OAuthProvider` | §17.1.5 | — |
| `MultiAuth` | §17.1.6 | §17.1.7 decision framework |
| `require_scopes(...)` | §17.2.1 | §18.6 scope vs resource checks |
| `restrict_tag(...)` | §17.2.2 | §17.2.3 AND-composition |
| `AuthMiddleware` | §17.2.6 | §17.2.7 component + middleware auth · §18.3 |
| CIMD / client assertions | §18.11 | §22.10 client side · §32.8 |

**Client**

| Symbol | Defined at | Also |
|---|---|---|
| `Client(...)` | §21.1, §22.0-§22.1 | §21.2 `async with` lifecycle · App. **Y)** |
| `ping()` | §21.5 | — |
| `list_tools()` · `list_resources()` · `list_prompts()` | §21.6 | §35.7 v3 component list APIs |
| `call_tool(...)` (client) | §21.7 | §21.10 `.data` vs `.content` |
| `read_resource(...)` · `get_prompt(...)` | §21.8-§21.9 | §21.10 |
| `StdioTransport` | §22.2 | **§22.2.1 nothing is inherited from the parent environment** · §22.2.3 session reuse |
| `StreamableHttpTransport` | §22.3 | §22.3.1 TLS |
| `SSETransport` | §22.4 | backward compatibility only |
| in-memory client/server | §22.5 | §2.3 · §30.2 **the default integration-test primitive** |
| `auth="<token>"` / `BearerAuth(...)` | §22.8 | §22.8.1 |
| `auth="oauth"` / `OAuth(...)` | §22.9 | §22.9.1 parameters that matter · §22.9.2 flow and token storage |
| `message_handler` · `sampling_handler` · `elicitation_handler` · `log_handler` · `progress_handler` | §22.11-§22.15 | §22.17 notifications |
| roots | §22.16 | static and dynamic forms |
| `fastmcp-slim[client]` · `fastmcp-remote` | §23.1, §23.4 | §23.11-§23.12 vs `Client` and vs a gateway · §1.3-§1.4 · App. **M)** |

**Apps, config, CLI, testing**

| Symbol | Defined at | Also |
|---|---|---|
| `@mcp.tool(app=True)` (Prefab) | §24.2, §25.2 | §24.3 · §25.9 **pin `prefab-ui` yourself** |
| `FastMCPApp` | §3.4, §24.4, §25.3 | §3.4.2 entry vs backend tools · §24.7 why it exists |
| `@app.ui()` · `@app.tool()` | §24.5-§24.6 | §3.4.4 `get_app_tool` bypass |
| `CallTool(...)` · `result_key` | §24.8 | — |
| `GenerativeUI()` | §24.10, §25.8 | §32.13 security |
| `Approval` · `Choice` · `FormInput` · `FileUpload` | §25.4-§25.7 | §25.4 **`Approval` is UX, not a security boundary** (§32.15) |
| `AppConfig` · `ui://` resources | §24.12-§24.13 | §24.14 CSP and permissions |
| `FastMCP.from_openapi(...)` | §26.1 | §26.11 generated vs model-facing schemas |
| `FastMCP.from_fastapi(...)` | §26.9 | §26.10 conversion vs mounting |
| `RouteMap` · `MCPType` | §26.4 | §26.5 exclusion-first policy |
| `fastmcp.json` | §27.0-§27.7 (all `###`) | `source` §27.2 · `environment` §27.3 · `deployment` §27.4 · schema §27.5 · precedence §27.6 · §27.13 vs standard MCP JSON |
| `fastmcp run` | §27.8, §28.2 | §27.8.1 dependency behavior |
| `fastmcp inspect` | §27.11, §28.4 | §30.7 manifest generation · App. **AC)** |
| `fastmcp install` | §27.10, §28.9 | §27.10.1 `mcp-json` · §27.10.2 `stdio` |
| `fastmcp generate-cli` | §27.12, §28.10 | §27.12.1-§27.12.3 |
| `fastmcp dev` · `list` · `call` · `discover` | §28.3, §28.5-§28.6, §28.8 | §28.12 debugging decision tree |
| tool fingerprinting | §30.5 | §30.6 what belongs in one · §30.8 drift classification · App. **AD)** |
| OpenTelemetry instrumentation | §29.1-§29.3 | §33.20 |

---

## §3 — Task → location

The entry path when you have a goal but no symbol. Phrased the way the goal occurs to you, not the
way the documents title their chapters.

### §3.1 Shaping what comes in

| I need to… | Go to |
|---|---|
| reject unknown keys instead of silently dropping them | `pydantic` §6.3, §9.2 · §51.18 |
| keep unknown keys and read them later | `pydantic` §6.3 (`extra='allow'`, `__pydantic_extra__`) · §6.4 to type them |
| stop `"10"` from becoming `10` | `pydantic` §10.1-§10.4 (four places to set strictness) · `fastmcp` §6.3 for the tool boundary |
| accept several spellings of one input key | `pydantic` §12.2 `AliasChoices` |
| pull a field out of a nested payload without pre-transforming it | `pydantic` §12.1 `AliasPath` |
| accept `camelCase` on the wire but keep `snake_case` in Python | `pydantic` §12.3-§12.4, §12.9 |
| validate JSON bytes without a separate `json.loads` | `pydantic` §5.4, §33.0-§33.1 |
| validate a mapping whose leaves are all strings (env, form) | `pydantic` §5.5 |
| build a model from an ORM object's attributes | `pydantic` §5.3, §9.5 · §44.2 |
| pass request-scoped data into a validator | `pydantic` §5.6 context, §14.4 |
| express a constraint once and reuse it everywhere | `pydantic` §11.0-§11.6 (`Annotated` + `Field`/`annotated-types`) |
| validate a bare `list[int]`, union, `TypedDict` or dataclass — no wrapper model | `pydantic` §21.0, §2.5 · §51.27 |
| run a rule that spans two fields | `pydantic` §14.0 model validator, §13.5 · §51.22 |
| choose a union branch by a tag field | `pydantic` §26.3 · §26.5 for a callable discriminator |
| validate an ordinary function's arguments | `pydantic` §37.0 |

### §3.2 Shaping what goes out

| I need to… | Go to |
|---|---|
| emit only the fields the caller actually set | `pydantic` §18.3 `exclude_unset` · §6.2 · §18.8 |
| drop `None`s / drop values equal to their default | `pydantic` §18.5 / §18.4 · §51.25 |
| drop a field conditionally, based on its value | `pydantic` §18.2 `exclude_if` · §20.2 for computed fields |
| always hide a field from output | `pydantic` §18.1 · §51.53 |
| stop a subclass's extra attributes leaking through a base-class field | `pydantic` §19.0 (this is the default) · §19.3 `polymorphic_serialization` to opt in · §19.6 |
| emit alias names rather than field names | `pydantic` §12.7, §9.6 (`serialize_by_alias` is **off** by default) · §16.2 |
| serialize one field in a custom format | `pydantic` §17.0-§17.2 · §17.5 for the reusable `Annotated` form |
| replace the whole model's output shape | `pydantic` §17.3-§17.4 |
| redact secrets during serialization | `pydantic` §31.1 `SecretStr` · §49.2-§49.3 · §51.53 context redaction |
| produce output that can be fed straight back in | `pydantic` §16.3 round-trip mode · §48.4 |
| expose a derived value as part of the contract | `pydantic` §20.0 `computed_field` (a plain `@property` is not serialized — §4.7) |

### §3.3 Contracts and schemas

| I need to… | Go to |
|---|---|
| generate the JSON Schema a consumer will see | `pydantic` §34.0-§34.2 (**pick the mode**) · §51.31 |
| understand why validation and serialization schemas differ | `pydantic` §34.2, §0.4 · §51.24 |
| add vendor keys / examples / titles to the schema | `pydantic` §34.4-§34.5 · §35.1 `WithJsonSchema` |
| change schema generation globally | `pydantic` §35.3 `GenerateJsonSchema` · §35.8 for the risk |
| make a third-party type validate and serialize | `pydantic` §29.0 escalation ladder → §29.1, §29.6 · §51.55 |
| detect that a published contract drifted | `pydantic` §48.5 snapshot · §34.9 · `fastmcp` §30.5-§30.9 |
| freeze a server's client-visible manifest | `fastmcp` §30.5, §30.7, `fastmcp inspect --format mcp` (§27.11) |

### §3.4 Building and running a server

| I need to… | Go to |
|---|---|
| register a tool / resource / prompt | `fastmcp` §5.2 / §7.1 / §8.1 · App. **A)** |
| decide whether something should be a tool, a resource or a prompt | §4 tree 6 · `fastmcp` App. **Q)**/**S)**/**T)** |
| hide a runtime-only value from the published schema | `fastmcp` §10 (DI), §6.5, §10.14 · App. **B)** |
| get at logging, progress, elicitation or sampling from inside a handler | `fastmcp` §9.4-§9.9 · App. **U)** |
| open a database pool once for the process | `fastmcp` §11.1-§11.3 (lifespan), §11.12 concurrency-safety |
| decide where a value lives — lifespan, session, DI, `Context` | `fastmcp` §11.10 ownership table · §11.13 |
| run work that outlives one request | `fastmcp` §12.1-§12.2 · App. **C)** |
| add cross-cutting policy (logging, rate limit, retry, cache) | `fastmcp` §13.13-§13.19 built-ins · §13.20 custom |
| combine several servers into one surface | `fastmcp` §14.18 `mount` · §16.9-§16.11 · App. **I)** |
| put a remote MCP server behind this one | `fastmcp` §14.19, §16.11-§16.13 |
| stop a large catalog eating the model's context | `fastmcp` §14.15 `ToolSearch`, §15.16 pagination, §33.5-§33.6 |
| rename or reshape a tool coming from a provider | `fastmcp` §14.14 `ToolTransform` · §15.4-§15.5 |
| publish two versions of one tool name | `fastmcp` §15.12-§15.14 |
| turn an OpenAPI spec or FastAPI app into a server | `fastmcp` §26.1 / §26.9 · App. **AB)** |
| pick an auth provider | `fastmcp` §17.1.1-§17.1.7 · App. **X)** |
| authorize a specific tool | `fastmcp` §17.2.1-§17.2.5 · §18.3 |
| choose STDIO vs HTTP, and deploy it | `fastmcp` §19.2-§19.3, §20 · App. **O)**/**P)** |
| make the project reproducible for a host | `fastmcp` §27.0-§27.7 `fastmcp.json` · §27.10 install |
| test it | `fastmcp` §30.2 in-memory first · §2.12 pytest fixture · §30.3-§30.4 |
| upgrade from v2 | `fastmcp` §35.20 workflow · **§35.21 / App. `AG)` grep lists** |

### §3.5 Configuration and secrets

| I need to… | Go to |
|---|---|
| read typed configuration from the environment | `pydantic` §38.1 `BaseSettings` · §38.3 prefix · §51.40 |
| control which source wins | `pydantic` §38.2, §39.0-§39.1 · §51.38 |
| populate a nested settings model from flat env vars | `pydantic` §38.6 · §38.5 complex values |
| load a `.env`, or deliberately not in production | `pydantic` §38.7 · §51.41 |
| read secrets from files or a cloud secret manager | `pydantic` §39.4, §39.6 · §39.5 nested-secret security |
| add TOML/YAML/`pyproject` as a source | `pydantic` §39.7 |
| build a CLI from a settings model | `pydantic` §39.3 |
| keep a token out of logs, reprs and dumps | `pydantic` §31.1 (`SecretStr` is not encryption — §51.37) |

### §3.6 When something is slow or wrong

| I need to… | Go to |
|---|---|
| find out why validation is slow | `pydantic` §40.0 profile first → §40.1-§40.8 · §51.42-§51.43 |
| stop rebuilding schemas per request | `pydantic` §21.6, §40.2 · §9.7 / §40.9 to defer instead |
| cut import/startup cost | `pydantic` §9.7 `defer_build`, §40.13 · `fastmcp` §33.1-§33.2 |
| turn a `ValidationError` into an API error body | `pydantic` §36.8, §36.0 · §51.57 · §51.33 error codes |
| stop raw input appearing in error output | `pydantic` §9.10, §36.7 · §49.9 |
| work out which of two libraries owns the bug | **§5** |
| understand why a v2 example does not work | `fastmcp` §35.2-§35.19 · App. **AG)** |
| understand why a 2.14 or v4 example does not work | `pydantic` §47 · `fastmcp` §36 |

---

## §4 — Decision trees

Ten choices the two libraries force on you. Each tree ends in a citation; the citation is the
authority, the tree is only the shortest route to it.

**1. Which validation entry point?** (`pydantic` §5.0, §51.3)

```
input is JSON text or bytes
  -> model_validate_json(...)                      §5.4  (one parse+validate pass; fastest)
input is a mapping whose every leaf is a string (env, form, query)
  -> model_validate_strings(...)                   §5.5
input is an object to read attributes from (ORM row, domain object)
  -> ConfigDict(from_attributes=True) + model_validate(obj)   §5.3
input is a mapping, and this is a trust boundary
  -> model_validate(...)                           §5.2  (runtime strict/extra/context/alias flags)
input is literal Python you wrote yourself
  -> Model(**kwargs)                               §5.1
the contract is not object-shaped at all
  -> TypeAdapter                                   §21, tree 3
data is already validated and you are reconstructing it
  -> model_construct(...)                          §6.0  -- never for external data
```

**2. Where do I set strictness?** (`pydantic` §10, §51.19)

```
one field must not be coerced
  -> Field(strict=True)                            §10.2
   or Annotated[int, Strict()] if reused           §10.3
one call site must be strict, model stays lax
  -> model_validate(..., strict=True)              §10.1
the whole model is a strict contract
  -> ConfigDict(strict=True), fields may opt out with Field(strict=False)   §10.4
the input is JSON
  -> remember strict JSON still accepts a date string; JSON has no date scalar   §10.5, §5.8
still unsure whether the coercion you fear even happens
  -> the conversion table before writing a validator   §10.6, §10.7
```

**3. `BaseModel`, `TypeAdapter`, `RootModel`, dataclass, or `TypedDict`?** (`pydantic` §51.27)

```
named object fields
  -> BaseModel                                     §4
bare container/union/alias: list[int], dict[str, T], A | B
  -> TypeAdapter(...)                              §21   (build once -- §21.6)
root value, but you want methods/a name on it
  -> RootModel[T]                                  §22
dict-shaped and allocation matters
  -> TypedDict + TypeAdapter                       §24.0-§24.1, §40.6
you already have a dataclass
  -> TypeAdapter over it, or @pydantic.dataclasses.dataclass   §23, §24.3
   ... note a Pydantic dataclass has no model_dump/model_validate   §23.2
```

**4. Which exclusion control?** (`pydantic` §51.25, §18)

```
omit fields the caller never supplied      -> exclude_unset=True          §18.3
omit values equal to their default         -> exclude_defaults=True       §18.4
omit nulls                                 -> exclude_none=True           §18.5
omit computed fields                       -> exclude_computed_fields=True §18.6
omit one field always                      -> Field(exclude=True)         §18.1
omit based on the value                    -> Field(exclude_if=...)       §18.2  (computed too, 2.13)
whitelist / blacklist at the call          -> include= / exclude=         §18.0
combining several of these
  -> §18.7 declines to state a precedence: assert the exact shape in a test
```

**5. How should a subclass serialize?** (`pydantic` §51.26, §19)

```
default: output follows the *annotation*, not the runtime class    §19.0
want subclass fields, and it is a model/dataclass
  -> ConfigDict(polymorphic_serialization=True)    §19.3   (new in 2.13; the narrow option)
want duck-typed output for one annotated position
  -> SerializeAsAny[T]                             §19.1
want it everywhere at runtime
  -> serialize_as_any=True                         §19.2   -- broadest; §19.6 shows the leak
public contract that must stay closed
  -> change nothing; the default is the safe one   §19.5
```

**6. Which union mode?** (`pydantic` §26, §51.28)

```
members carry a shared literal tag field
  -> Field(discriminator='kind')                   §26.3  (best errors, best schema, fastest)
the tag is computed, not a plain field
  -> Discriminator(callable) + Tag(...)            §26.5
members are unambiguous types
  -> smart mode, the default                       §26.1
order must decide, and first match wins
  -> union_mode='left_to_right'                    §26.2
errors are unreadable
  -> that is the symptom of an undiscriminated union   §26.7
```

**7. How far down the customization ladder?** (`pydantic` §29.0, §51.29)

```
1. a standard type + Field constraints                        §7.4, §11
2. Annotated + BeforeValidator/AfterValidator/PlainSerializer §15, §17.5
3. ValidateAs / InstanceOf / SkipValidation                   §15.2-§15.4
4. __get_pydantic_core_schema__ / GetPydanticSchema           §29.1, §29.4
5. pydantic-core directly                                     §29.7  -- §29.8 warns on stability
stop at the highest step that works; each step down costs maintenance
```

**8. Tool, resource, or prompt?** (`fastmcp` App. **Q)**/**S)**/**T)**, §5.0, §7.0, §8.0)

```
the model should be able to *do* something, with arguments
  -> tool                                          §5
the model should be able to *read* something, addressed by URI
  -> resource, or a resource template if parameterised   §7.1, §7.12
the *user* picks a reusable message scaffold
  -> prompt                                        §8.1
the client only supports tools
  -> ResourcesAsTools / PromptsAsTools             §14.16-§14.17
```

**9. Compose, mount, import, or proxy?** (`fastmcp` §3.6, §16.9-§16.13, App. **I)**)

```
components declared in this process
  -> LocalProvider, implicitly                     §14.2
another FastMCP server object, live and nested
  -> mount(child, namespace=...)                   §14.18, §15.3
a one-time copy of another server's components
  -> import_server(...)   -- deprecated in v3      §3.6.2, §35.14
a remote MCP server, projected locally
  -> create_proxy(...) / ProxyProvider             §14.19, §16.11
a whole catalog from a spec or filesystem
  -> OpenAPIProvider / FileSystemProvider          §26.2, §14.7
you only need to rename/reshape what a source already gives you
  -> a Transform, not a new provider               §14.10, §15.5
```

**10. Where does this value come from?** (`fastmcp` §11.10, §11.13, §9.16)

```
the model supplies it
  -> an ordinary typed parameter                   §5.7
the runtime supplies it and the model must not see it
  -> DI: Depends(...) / CurrentContext() / CurrentAccessToken()   §10.9, §10.5, §10.8
it is per-request MCP capability (log, progress, elicit, sample)
  -> Context                                       §9.1, App. U)
it is per-process and expensive to build (pool, client, config)
  -> lifespan                                      §11.1  -- make it concurrency-safe §11.12
it must survive across calls in one session
  -> session state                                 §9.10, §11.5
it must survive across processes
  -> a distributed session_state_store             §11.7, §11.9
never
  -> a module global                               §11.11
```

---

## §5 — The `fastmcp` ↔ `pydantic` seam

Each document treats the other library as background — `fastmcp` names Pydantic 37 times, mostly in
§6 — so the seam is where an agent loses the thread. Four crossings, and one rule.

| Crossing | `fastmcp` side | `pydantic` side |
|---|---|---|
| **A tool signature is a schema.** Type hints on the function become the published input schema. | §6.1 generation from hints · §6.2 serve-time shaping and `$ref` dereferencing · §6.4 `Annotated[...]`/`Field(...)` on parameters · §2.6 worked example | §7 fields and `Annotated` · §34 JSON Schema · §29 for a type that will not schematise |
| **Input coercion policy.** Flexible mode allows Pydantic-style coercion so `"10"` satisfies `int`; strict mode validates against the exact generated schema first. | §6.3 · §4.4 `strict_input_validation` | §10 strict vs lax, and the four places to set it · §10.5 for JSON-specific behavior |
| **Output shape.** What a return value becomes on the wire — content blocks, structured content, output schema. | §6.8-§6.10 · §6.11 `ToolResult` · App. **H)**, App. **R)** | §16 `model_dump` modes · §18 exclusion · §19 subclass leakage · §34.2 serialization-mode schema |
| **Server configuration.** FastMCP has `fastmcp.json` for the *project*; typed process settings are Pydantic's job. | §27.0-§27.7 `fastmcp.json` · §27.6 CLI override precedence | §38-§39 `pydantic-settings` · §51.38 source map |

**The rule.** In a traceback that spans both, split it at the schema. Anything about *whether a
value was accepted, coerced, or rejected*, and anything about *what a dumped object looks like*, is
a `pydantic` question even when the symbol in the traceback is a FastMCP one. Anything about
*whether the component was registered, visible, routed, or authorized* is a `fastmcp` question even
when the payload is a model.

Two consequences worth stating outright:

* **`pydantic` §19 applies to tool return values.** A tool annotated `-> Base` that returns a
  `Derived` will not emit `Derived`'s extra fields, because that is Pydantic's annotation-driven
  default (§19.0) — not a FastMCP bug.
* **`pydantic` §21.6 applies to tool handlers.** A `TypeAdapter` constructed inside a handler is
  recompiled on every call. Build it at module scope.

---

## §6 — Navigation rules

Fourteen rules, all verified against the two files. Rules 1-6 are about finding things; 7-10 about
reading them; 11-14 about trusting what you find.

**1. Look symbols up in §2, never by grepping.** These documents teach by example, so a public API
name appears wherever it is *used*, not only where it is defined. Measured hit counts: `Context`
**201** in `fastmcp`, `TypeAdapter` **103** in `pydantic`, `Depends` 51, `ToolResult` 40,
`model_validate_json` 22, `ToolTransform` 20, `SecretStr` 12, `mask_error_details` 8. A grep gives
you the noise and hides the definition inside it.

**2. `lib-outline --view expanded` is only trustworthy on `pydantic`.** That extractor maps `##` to
members, and `fastmcp`'s subsection depth is inconsistent:

* **14 chapters number their subsections at `###`** — §0, §5, §6, §7, §8, §9, §10, §12, §13, §14,
  §19, §21, §24, §29. Together **4,575 lines, 29.2% of the document.** Expanding them returns
  *nothing at all*, silently. That includes tools, `Context`, DI, middleware and providers — five of
  the highest-traffic chapters in the file.
* **§17, §22 and §27 return a partial list**, which is worse than nothing because it looks
  complete. §17 shows 2 entries and hides all six auth providers one level below; §27 shows 7 and
  hides the entire `fastmcp.json` schema; §22 hides `22.0`-`22.1`.
* **13 chapters** demote only `N.0` to `###`, so just their orientation subsection is missing —
  §3, §4, §17, §25, §26, §28, §30, §31, §32, §33, §34, §35, §36.
* **Only 9 chapters expand completely**: §1, §2, §11, §15, §16, §18, §20, §23, and §37 (whose
  letters are `##`, so they list correctly even though they are not `N.M`).

Use §1.1's Depth column to know in advance. For a `###`-numbered chapter, grep `^### N\.` instead.
`pydantic` has no equivalent problem: 594 `##` against 48 `###`, uniform `## N.M` in all 52 chapters.

**3. A bare `rg '^# '` is roughly half wrong on both files.** It reports **85** top-level headings in
`fastmcp` and **78** in `pydantic`; the real counts are **42** and **57**. The difference is
`#`-prefixed shell and Python comments inside fenced blocks — **43 decoys** in `fastmcp`, **21** in
`pydantic` — things like `# Recommended with uv`, `# Code Mode transform`, `# contracts/types.py`.
`lib-outline` parses markdown and is immune; a hand-rolled search needs a fence toggle.

**4. Build indexes from chapter headings, never from either document's own map.** The "Proposed
comprehensive documentation map" is an outline written before the chapters and never reconciled.
`fastmcp` diverges on **five** titles — §0, §22 ("client-side authentication" → "client-side auth"),
§25 ("custom HTML" → "custom renderers"), §27 (drops "and portable deployment contracts"), §32
(drops "checklist") — and `pydantic` on **one** (§0). The chapter heading is the real title.

**5. Neither document has an end-of-reference marker.** `gix`/`notify` end with `# End of reference`;
these do not. A chapter runs until the next `# … Advanced — N)` heading, and the final chapter runs
to EOF: `fastmcp` §37 ends in a source list, `pydantic` §51 is followed only by
`# Reference source URLs` (7294).

**6. Numbering starts in three different places.** `fastmcp` §0 has **no `0.0`** (it opens at `0.1`);
`pydantic` §51 has **no `51.0`** (it opens at `51.1`); everything else opens at `N.0`. And all 14 of
`fastmcp`'s `###`-numbered chapters open with a *bare, unnumbered* `###` line restating the chapter
title before the first real subsection (§0's restatement is worded slightly differently). Seeking to
"the first subsection" lands on that echo.

**7. Read `pydantic` by the chapter, `fastmcp` by the subsection.** `pydantic` chapters have a
median of **~112** lines (range 73-234 once §51 is set aside); reading the whole chapter usually costs less than locating
the right part of it. `fastmcp` chapters have a median of **~400** (range 194-747) — never read one
whole.

**8. Blank-line density is ~30% in both** — 31.2% in `fastmcp`, 30.3% in `pydantic` — so a
`Read(offset, limit)` window holds roughly what it appears to. (The `gix` reference in this same
directory is 53%, which is why that skill warns to double the limit; these two do not need it.)

**9. Go to the appendix chapter first for a signature or a matrix.** `pydantic` §51 (1,264 lines,
**17% of the file**) and `fastmcp` §37 (747 lines, 35 lettered matrices) are the intended lookup
layers, indexed at §1.4 and §1.3. Read the prose chapter afterwards, for *why*.

**10. `fastmcp` §37's letters skip `K`,** and the appendix is two overlapping series — `A)`-`J)`
written against the early chapters, `L)`-`AJ)` against the later ones. Transport is at both `F)` and
`O)`, client auth at both `G)` and `Z)`, providers at both `D)` and `V)`. When two letters cover one
topic, read both.

**11. A named symbol is not necessarily in the document.** `JsonValue`, for one, is never mentioned
in the `pydantic` reference. Absence from §2 means absence from the document, not absence from the
library — fall back to the installed package.

**12. Both documents rank themselves below the installed package.** `fastmcp` App. `AJ)` and
`pydantic` §0.0 give near-identical hierarchies: the project's pinned version and its actual
signatures outrank the official docs, which outrank these references. Reconcile any example against
the pin before applying it.

**13. Prerelease material is quarantined, and it looks like the rest of the document.**
`fastmcp` §36 (443 lines) is FastMCP 4; `pydantic` §47 (74 lines) is 2.14. Both are written in the
same voice as the stable chapters and both are explicitly labelled as migration research. If a
symbol only resolves to §36 or §47, it does not exist in the pinned version.

**14. Version-sensitive claims live in the delta chapters, not the feature chapters.** Behavior that
changed recently is described *twice*: once in the topical chapter and once in `pydantic` §46 /
`fastmcp` §35, and the delta chapter is usually the one that says what the old behavior was. For a
"this used to work" question, start there — `fastmcp` §35.21 and App. `AG)` are literal grep sheets
for v2 idioms, and `pydantic` §51.44-§51.47 catch the three classic V1→V2 traps.
